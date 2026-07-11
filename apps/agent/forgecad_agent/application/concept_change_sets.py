from __future__ import annotations

import base64
import binascii
import hashlib
import json
import uuid
from datetime import datetime, timezone
from typing import Any, Optional

from pydantic import ValidationError

from forgecad_agent.application.concept_models import (
    ChangeSetConfirmResponse,
    ChangeSetDiagnostic,
    ChangeSetPreviewResponse,
    ChangeSetTimelineItem,
    ChangeSetTimelineResponse,
    ConceptPlannerProvenance,
    PlanDesignChangeSetRequest,
    PlannedChangeSetRecord,
    ProposeConnectorSnapRequest,
    ProposeChangeSetRequest,
)
from forgecad_agent.application.concept_planner import (
    ConceptChangePlan,
    ConceptPlannerError,
    ConceptPlannerProvider,
    DeterministicConceptPlanner,
    planner_provenance,
)
from forgecad_agent.application.concept_modules import validate_registered_graph
from forgecad_agent.application.concept_jobs import record_completed_job
from forgecad_agent.application.concept_projects import project_detail_from_uow
from forgecad_agent.domain.concepts.models import (
    DesignChangeOperation,
    DesignChangeSet,
    ModuleGraph,
    Transform,
    WeaponConceptSpec,
)
from forgecad_agent.domain.concepts.connector_snapping import (
    connector_alignment_error,
    snap_child_transform,
)
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork


class ConceptChangeSetError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


class ConceptChangeSetIdempotencyConflict(RuntimeError):
    pass


class ConceptChangeSetService:
    """Propose, preview, and commit auditable Concept changes."""

    def __init__(
        self,
        connection_factory: SQLiteConnectionFactory,
        planner: Optional[ConceptPlannerProvider] = None,
    ) -> None:
        self.connection_factory = connection_factory
        self.planner = planner or DeterministicConceptPlanner()
        self.deterministic_planner = DeterministicConceptPlanner()

    def plan(
        self,
        version_id: str,
        request: PlanDesignChangeSetRequest,
        idempotency_key: str,
    ) -> PlannedChangeSetRecord:
        scope = f"POST /api/v1/versions/{version_id}/change-sets:plan"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            replay = unit_of_work.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise ConceptChangeSetIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return PlannedChangeSetRecord.model_validate_json(replay.response_json)

            version = unit_of_work.concept_projects.find_version(version_id)
            if version is None:
                raise ConceptChangeSetError("VERSION_NOT_FOUND", "Base version not found.")
            if version["module_graph_id"] is None:
                raise ConceptChangeSetError(
                    "MODULE_GRAPH_NOT_FOUND",
                    "Base version does not have a validated ModuleGraph.",
                )
            project_id = str(version["project_id"])
            project = unit_of_work.concept_projects.get_active(project_id)
            if project is None:
                raise ConceptChangeSetError("PROJECT_NOT_FOUND", "Concept project not found.")
            graph_row = unit_of_work.modules.get_graph(str(version["module_graph_id"]))
            if graph_row is None:
                raise ConceptChangeSetError(
                    "MODULE_GRAPH_NOT_FOUND", "Base version ModuleGraph not found."
                )
            current_spec = WeaponConceptSpec.model_validate_json(version["spec_json"])
            base_graph = ModuleGraph.model_validate_json(graph_row["graph_json"])
            module_catalog = _change_planner_module_catalog(
                unit_of_work, profile_id=str(project["profile_id"])
            )
            _validate_planner_context(
                request,
                base_graph=base_graph,
                registry_module_ids={item["module_id"] for item in module_catalog},
            )
            (
                provider,
                plan,
                fallback_used,
                warnings,
                attempted_provider,
            ) = self._change_plan_with_provider(
                request=request,
                current_spec=current_spec,
                base_graph=base_graph,
                module_catalog=module_catalog,
            )
            change_set = _change_set_from_plan(
                plan,
                project_id=project_id,
                version_id=version_id,
                base_graph=base_graph,
            )
            _apply_change_set(current_spec, base_graph, change_set)
            provenance = planner_provenance(
                provider,
                input_payload={
                    "instruction": request.instruction,
                    "current_spec": current_spec.model_dump(mode="json"),
                    "base_graph": base_graph.model_dump(mode="json"),
                    "selected_node_id": request.selected_node_id,
                    "selected_module_id": request.selected_module_id,
                },
                output_payload=plan.model_dump(mode="json"),
                registry_module_ids=[item["module_id"] for item in module_catalog],
                attempted_provider=attempted_provider,
                fallback_used=fallback_used,
                warnings=warnings,
            )
            rationale = list(
                dict.fromkeys(
                    [*plan.rationale, *(item.rationale for item in plan.operations)]
                )
            )[:12]
            now = _utc_now()
            job_id = record_completed_job(
                unit_of_work,
                project_id=project_id,
                version_id=version_id,
                job_type="concept_change_plan",
                input_payload={
                    "instruction": request.instruction,
                    "generator": request.generator,
                    "selected_node_id": request.selected_node_id,
                    "selected_module_id": request.selected_module_id,
                },
                output_payload={
                    "change_set_id": change_set.change_set_id,
                    "operation_count": len(change_set.operations),
                    "provider_id": provenance.provider_id,
                    "generator": provenance.generator,
                    "fallback_used": provenance.fallback_used,
                },
                steps=(
                    (
                        "load_change_context",
                        "Current immutable Spec, ModuleGraph, and registry loaded.",
                        0.25,
                        {"graph_id": base_graph.graph_id},
                    ),
                    (
                        "plan_change_set",
                        "Natural-language instruction converted to bounded operations.",
                        0.7,
                        {
                            "provider_id": provenance.provider_id,
                            "generator": provenance.generator,
                            "fallback_used": provenance.fallback_used,
                        },
                    ),
                    (
                        "validate_change_set",
                        "Planner IDs, paths, locks, and operation contracts validated.",
                        1.0,
                        {"operation_count": len(change_set.operations)},
                    ),
                ),
            )
            change_set_json = _canonical_json(change_set.model_dump(mode="json"))
            unit_of_work.change_sets.add(
                change_set_id=change_set.change_set_id,
                project_id=change_set.project_id,
                base_version_id=change_set.base_version_id,
                schema_version=change_set.schema_version,
                change_set_json=change_set_json,
                change_set_sha256=hashlib.sha256(
                    change_set_json.encode("utf-8")
                ).hexdigest(),
                status=change_set.status,
                created_at=now,
                actor_type="planner",
                planner_instruction=request.instruction,
                planner_rationale_json=_canonical_json(rationale),
                planner_provenance_json=_canonical_json(
                    provenance.model_dump(mode="json")
                ),
                planner_job_id=job_id,
            )
            response = PlannedChangeSetRecord(
                change_set=change_set,
                instruction=request.instruction,
                rationale=rationale,
                planner_provenance=provenance,
                job_id=job_id,
            )
            response_json = _canonical_json(response.model_dump(mode="json"))
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=response_json,
                created_at=now,
            )
            return response

    def _change_plan_with_provider(
        self,
        *,
        request: PlanDesignChangeSetRequest,
        current_spec: WeaponConceptSpec,
        base_graph: ModuleGraph,
        module_catalog: list[dict[str, str]],
    ) -> tuple[
        ConceptPlannerProvider,
        ConceptChangePlan,
        bool,
        list[str],
        Optional[ConceptPlannerProvider],
    ]:
        provider = (
            self.deterministic_planner
            if request.generator in {"deterministic_rules", "deterministic_template"}
            else self.planner
        )
        registry_module_ids = {item["module_id"] for item in module_catalog}
        try:
            plan = provider.plan_change_set(
                instruction=request.instruction,
                current_spec=current_spec,
                base_graph=base_graph,
                module_catalog=module_catalog,
                selected_node_id=request.selected_node_id,
                selected_module_id=request.selected_module_id,
            )
            _validate_change_plan(
                plan,
                current_spec=current_spec,
                base_graph=base_graph,
                module_catalog=module_catalog,
                registry_module_ids=registry_module_ids,
            )
            return provider, plan, False, [], None
        except (ConceptPlannerError, ConceptChangeSetError, ValidationError) as exc:
            if request.generator == "auto" and provider.provider_type != "deterministic":
                warning = _change_planner_warning(exc)
                try:
                    plan = self.deterministic_planner.plan_change_set(
                        instruction=request.instruction,
                        current_spec=current_spec,
                        base_graph=base_graph,
                        module_catalog=module_catalog,
                        selected_node_id=request.selected_node_id,
                        selected_module_id=request.selected_module_id,
                    )
                    _validate_change_plan(
                        plan,
                        current_spec=current_spec,
                        base_graph=base_graph,
                        module_catalog=module_catalog,
                        registry_module_ids=registry_module_ids,
                    )
                except (
                    ConceptPlannerError,
                    ConceptChangeSetError,
                    ValidationError,
                ) as fallback_error:
                    raise _change_planner_error(fallback_error) from fallback_error
                return self.deterministic_planner, plan, True, [warning], provider
            raise _change_planner_error(exc) from exc

    def propose(
        self,
        version_id: str,
        request: ProposeChangeSetRequest,
        idempotency_key: str,
    ) -> DesignChangeSet:
        scope = f"POST /api/v1/versions/{version_id}/change-sets"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            existing_idempotency = unit_of_work.idempotency.get(scope, idempotency_key)
            if existing_idempotency is not None:
                if existing_idempotency.request_hash != request_hash:
                    raise ConceptChangeSetIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return DesignChangeSet.model_validate_json(existing_idempotency.response_json)

            version = unit_of_work.concept_projects.find_version(version_id)
            if version is None:
                raise ConceptChangeSetError("VERSION_NOT_FOUND", "Base version not found.")
            change_set = request.change_set
            if change_set.base_version_id != version_id:
                raise ConceptChangeSetError(
                    "INVALID_REQUEST",
                    "DesignChangeSet base_version_id does not match the route version_id.",
                )
            if change_set.project_id != version["project_id"]:
                raise ConceptChangeSetError(
                    "INVALID_REQUEST",
                    "DesignChangeSet project_id does not match the base version project.",
                )
            if version["module_graph_id"] is None:
                raise ConceptChangeSetError(
                    "MODULE_GRAPH_NOT_FOUND",
                    "Base version does not have a validated ModuleGraph.",
                )
            if unit_of_work.change_sets.get(change_set.change_set_id) is not None:
                raise ConceptChangeSetError(
                    "CHANGE_SET_CONFLICT",
                    "DesignChangeSet ID is already registered.",
                )

            now = _utc_now()
            change_set_json = _canonical_json(change_set.model_dump(mode="json"))
            unit_of_work.change_sets.add(
                change_set_id=change_set.change_set_id,
                project_id=change_set.project_id,
                base_version_id=change_set.base_version_id,
                schema_version=change_set.schema_version,
                change_set_json=change_set_json,
                change_set_sha256=hashlib.sha256(change_set_json.encode("utf-8")).hexdigest(),
                status=change_set.status,
                created_at=now,
            )
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=change_set_json,
                created_at=now,
            )
            return change_set

    def propose_connector_snap(
        self,
        version_id: str,
        request: ProposeConnectorSnapRequest,
        idempotency_key: str,
    ) -> DesignChangeSet:
        """Build a set_transform ChangeSet from the graph's authoritative connector frames."""
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            version = unit_of_work.concept_projects.find_version(version_id)
            if version is None:
                raise ConceptChangeSetError("VERSION_NOT_FOUND", "Base version not found.")
            if version["module_graph_id"] is None:
                raise ConceptChangeSetError(
                    "MODULE_GRAPH_NOT_FOUND",
                    "Connector snap requires a validated ModuleGraph.",
                )
            graph_row = unit_of_work.modules.get_graph(str(version["module_graph_id"]))
            if graph_row is None:
                raise ConceptChangeSetError(
                    "MODULE_GRAPH_NOT_FOUND", "Base version ModuleGraph not found."
                )
            graph = ModuleGraph.model_validate_json(graph_row["graph_json"])
            node = next((item for item in graph.nodes if item.node_id == request.node_id), None)
            if node is None:
                raise ConceptChangeSetError(
                    "INVALID_REQUEST", f"Graph node does not exist: {request.node_id}"
                )
            if node.node_id == graph.root_node_id or node.locked:
                raise ConceptChangeSetError(
                    "CHANGE_SET_INVALID", f"Connector snap cannot move protected node: {node.node_id}"
                )
            parent_edges, _ = _rooted_parent_edges(graph)
            parent_entry = parent_edges.get(node.node_id)
            if parent_entry is None:
                raise ConceptChangeSetError(
                    "CHANGE_SET_INVALID",
                    f"Connector snap requires {node.node_id} to have a parent edge.",
                )
            parent_node_id, edge = parent_entry
            nodes = {item.node_id: item for item in graph.nodes}
            parent_node = nodes[parent_node_id]
            connector_rows = unit_of_work.modules.connector_map(
                [item.module_id for item in graph.nodes]
            )
            parent_connector_id, child_connector_id = _edge_connector_ids(
                edge, parent_id=parent_node_id, child_id=node.node_id
            )
            snapped = snap_child_transform(
                parent_transform=parent_node.transform,
                parent_connector=_connector_transform(
                    connector_rows, parent_connector_id, parent_node_id
                ),
                parent_mirror_axis=parent_node.mirror_axis,
                child_scale=node.transform.scale,
                child_connector=_connector_transform(
                    connector_rows, child_connector_id, node.node_id
                ),
                child_mirror_axis=node.mirror_axis,
            )
            if _transforms_differ(snapped, node.transform) is False:
                raise ConceptChangeSetError(
                    "PLANNER_NO_ACTION", "Connector is already aligned within snap precision."
                )
            suffix = hashlib.sha256(idempotency_key.encode("utf-8")).hexdigest()[:16]
            change_set = DesignChangeSet(
                change_set_id=f"change_connector_snap_{suffix}",
                project_id=str(version["project_id"]),
                base_version_id=version_id,
                summary=(
                    f"Snap {node.node_id} to {parent_node_id} via Connector edge {edge.edge_id}."
                ),
                operations=[
                    DesignChangeOperation(
                        operation_id=f"op_connector_snap_{suffix}",
                        op="set_transform",
                        node_id=node.node_id,
                        transform=snapped,
                    )
                ],
                protected_node_ids=[
                    item.node_id
                    for item in graph.nodes
                    if item.locked or item.node_id == graph.root_node_id
                ],
                status="proposed",
            )
        return self.propose(
            version_id,
            ProposeChangeSetRequest(
                client_request_id=request.client_request_id,
                change_set=change_set,
            ),
            idempotency_key,
        )

    def list_for_project(
        self,
        project_id: str,
        *,
        cursor: str | None = None,
        limit: int = 20,
        query: str | None = None,
        status: str | None = None,
        operation: str | None = None,
    ) -> ChangeSetTimelineResponse:
        normalized_query = query.strip().lower() if query and query.strip() else None
        filter_hash = _hash_json(
            {
                "query": normalized_query,
                "status": status,
                "operation": operation,
            }
        )
        decoded_cursor = _decode_timeline_cursor(cursor, filter_hash) if cursor else None
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            if unit_of_work.concept_projects.get_active(project_id) is None:
                raise ConceptChangeSetError("PROJECT_NOT_FOUND", "Concept project not found.")
            rows = unit_of_work.change_sets.list_for_project(
                project_id,
                cursor=decoded_cursor,
                limit=limit + 1,
                query=normalized_query,
                status=status,
                operation=operation,
            )
            page = rows[:limit]
            next_cursor = (
                _encode_timeline_cursor(
                    str(page[-1]["updated_at"]),
                    str(page[-1]["change_set_id"]),
                    filter_hash,
                )
                if len(rows) > limit and page
                else None
            )
            return ChangeSetTimelineResponse(
                project_id=project_id,
                items=[change_set_timeline_item_from_row(row) for row in page],
                next_cursor=next_cursor,
            )

    def record_preview_rejection(
        self,
        change_set_id: str,
        error: ConceptChangeSetError,
    ) -> None:
        if error.code != "CHANGE_SET_INVALID":
            return
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            row = unit_of_work.change_sets.get(change_set_id)
            if row is None or row["status"] not in {"proposed", "previewed"}:
                return
            change_set = DesignChangeSet.model_validate_json(row["change_set_json"])
            rejected = change_set.model_copy(update={"status": "rejected"})
            now = _utc_now()
            diagnostic = _change_set_diagnostic(
                rejected,
                code=error.code,
                message=str(error),
                stage="preview",
                recorded_at=now,
            )
            unit_of_work.change_sets.mark_rejected(
                change_set_id,
                change_set_json=_canonical_json(rejected.model_dump(mode="json")),
                diagnostic_json=_canonical_json(diagnostic.model_dump(mode="json")),
                updated_at=now,
            )

    def reject(self, change_set_id: str, idempotency_key: str) -> DesignChangeSet:
        scope = f"POST /api/v1/change-sets/{change_set_id}:reject"
        request_hash = _hash_json({"change_set_id": change_set_id})
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            replay = unit_of_work.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise ConceptChangeSetIdempotencyConflict(
                        "Idempotency-Key was reused with different reject input."
                    )
                return DesignChangeSet.model_validate_json(replay.response_json)
            row = unit_of_work.change_sets.get(change_set_id)
            if row is None:
                raise ConceptChangeSetError("CHANGE_SET_NOT_FOUND", "DesignChangeSet not found.")
            if row["status"] not in {"proposed", "previewed"}:
                raise ConceptChangeSetError(
                    "CHANGE_SET_STATE_CONFLICT",
                    f"Cannot reject a {row['status']} DesignChangeSet.",
                )
            current = DesignChangeSet.model_validate_json(row["change_set_json"])
            rejected = current.model_copy(update={"status": "rejected"})
            now = _utc_now()
            diagnostic = _change_set_diagnostic(
                rejected,
                code="CHANGE_SET_DISCARDED",
                message="User discarded the ghost preview before confirmation.",
                stage="preview",
                recorded_at=now,
            )
            response_json = _canonical_json(rejected.model_dump(mode="json"))
            unit_of_work.change_sets.mark_rejected(
                change_set_id,
                change_set_json=response_json,
                diagnostic_json=_canonical_json(diagnostic.model_dump(mode="json")),
                updated_at=now,
            )
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=response_json,
                created_at=now,
            )
            return rejected

    def preview(
        self,
        change_set_id: str,
        idempotency_key: str,
    ) -> ChangeSetPreviewResponse:
        scope = f"POST /api/v1/change-sets/{change_set_id}:preview"
        request_hash = _hash_json({"change_set_id": change_set_id})
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            replay = unit_of_work.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise ConceptChangeSetIdempotencyConflict(
                        "Idempotency-Key was reused with different preview input."
                    )
                return ChangeSetPreviewResponse.model_validate_json(replay.response_json)

            row = unit_of_work.change_sets.get(change_set_id)
            if row is None:
                raise ConceptChangeSetError("CHANGE_SET_NOT_FOUND", "DesignChangeSet not found.")
            if row["status"] not in {"proposed", "previewed"}:
                raise ConceptChangeSetError(
                    "CHANGE_SET_STATE_CONFLICT",
                    f"Cannot preview a {row['status']} DesignChangeSet.",
                )
            change_set = DesignChangeSet.model_validate_json(row["change_set_json"])
            version = unit_of_work.concept_projects.find_version(change_set.base_version_id)
            if version is None:
                raise ConceptChangeSetError("VERSION_NOT_FOUND", "Base version not found.")
            graph_row = unit_of_work.modules.get_graph(str(version["module_graph_id"]))
            if graph_row is None:
                raise ConceptChangeSetError(
                    "MODULE_GRAPH_NOT_FOUND",
                    "Base version ModuleGraph not found.",
                )

            base_spec = WeaponConceptSpec.model_validate_json(version["spec_json"])
            base_graph = ModuleGraph.model_validate_json(graph_row["graph_json"])
            _assert_locked_nodes_unchanged(base_graph, change_set)
            preview_spec, preview_graph = _apply_change_set(
                base_spec,
                base_graph,
                change_set,
            )
            preview_graph = _remap_replaced_connectors(
                unit_of_work,
                base_graph=base_graph,
                preview_graph=preview_graph,
                change_set=change_set,
            )
            preview_graph = _snap_graph_after_replacements(
                unit_of_work,
                graph=preview_graph,
                change_set=change_set,
            )
            project = unit_of_work.concept_projects.get_active(change_set.project_id)
            if project is None:
                raise ConceptChangeSetError("PROJECT_NOT_FOUND", "Concept project not found.")
            profile = unit_of_work.domain_profiles.get_active(str(project["profile_id"]))
            if profile is None:
                raise ConceptChangeSetError(
                    "DOMAIN_PROFILE_NOT_FOUND",
                    "The project domain profile is unavailable.",
                )
            issues = validate_registered_graph(
                unit_of_work,
                graph=preview_graph,
                profile_pack_id=str(profile["pack_id"]),
            )
            if issues:
                codes = ", ".join(issue.code for issue in issues)
                raise ConceptChangeSetError(
                    "CHANGE_SET_INVALID",
                    f"ChangeSet preview failed ModuleGraph validation: {codes}",
                )

            preview_spec_json = _canonical_json(preview_spec.model_dump(mode="json"))
            preview_graph_json = _canonical_json(preview_graph.model_dump(mode="json"))
            preview_sha256 = _hash_json(
                {"spec": json.loads(preview_spec_json), "graph": json.loads(preview_graph_json)}
            )
            previewed_change_set = change_set.model_copy(update={"status": "previewed"})
            now = _utc_now()
            unit_of_work.change_sets.save_preview(
                change_set_id=change_set_id,
                change_set_json=_canonical_json(previewed_change_set.model_dump(mode="json")),
                preview_spec_json=preview_spec_json,
                preview_graph_json=preview_graph_json,
                preview_sha256=preview_sha256,
                updated_at=now,
            )
            response = ChangeSetPreviewResponse(
                change_set=previewed_change_set,
                preview_spec=preview_spec,
                preview_graph=preview_graph,
                preview_sha256=preview_sha256,
                issues=[],
            )
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=now,
            )
            return response

    def confirm(
        self,
        change_set_id: str,
        idempotency_key: str,
    ) -> ChangeSetConfirmResponse:
        scope = f"POST /api/v1/change-sets/{change_set_id}:confirm"
        request_hash = _hash_json({"change_set_id": change_set_id})
        if self._mark_stale_if_current_version_changed(change_set_id):
            raise ConceptChangeSetError(
                "CHANGE_SET_STALE",
                "Project current version changed after preview; preview again on a new base.",
            )
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            replay = unit_of_work.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise ConceptChangeSetIdempotencyConflict(
                        "Idempotency-Key was reused with different confirm input."
                    )
                return ChangeSetConfirmResponse.model_validate_json(replay.response_json)

            row = unit_of_work.change_sets.get(change_set_id)
            if row is None:
                raise ConceptChangeSetError("CHANGE_SET_NOT_FOUND", "DesignChangeSet not found.")
            if row["status"] != "previewed":
                raise ConceptChangeSetError(
                    "CHANGE_SET_STATE_CONFLICT",
                    "Only a previewed DesignChangeSet can be confirmed.",
                )
            project = unit_of_work.concept_projects.get_active(str(row["project_id"]))
            if project is None:
                raise ConceptChangeSetError("PROJECT_NOT_FOUND", "Concept project not found.")
            if project["current_version_id"] != row["base_version_id"]:
                raise ConceptChangeSetError(
                    "CHANGE_SET_STALE",
                    "Project current version changed after preview; preview again on a new base.",
                )

            preview_spec = WeaponConceptSpec.model_validate_json(row["preview_spec_json"])
            preview_graph = ModuleGraph.model_validate_json(row["preview_graph_json"])
            version_id = _new_id("ver")
            now = _utc_now()
            spec_json = _canonical_json(preview_spec.model_dump(mode="json"))
            graph_json = _canonical_json(preview_graph.model_dump(mode="json"))
            unit_of_work.concept_projects.add_version(
                version_id=version_id,
                project_id=str(row["project_id"]),
                parent_version_id=str(row["base_version_id"]),
                version_no=unit_of_work.concept_projects.next_version_number(
                    str(row["project_id"])
                ),
                status="committed",
                summary=f"ChangeSet: {change_set_id}",
                spec_schema_version=preview_spec.schema_version,
                spec_json=spec_json,
                spec_sha256=hashlib.sha256(spec_json.encode("utf-8")).hexdigest(),
                module_graph_id=preview_graph.graph_id,
                change_set_id=change_set_id,
                created_at=now,
            )
            unit_of_work.modules.add_graph(
                graph_id=preview_graph.graph_id,
                project_id=preview_graph.project_id,
                version_id=version_id,
                root_node_id=preview_graph.root_node_id,
                schema_version=preview_graph.schema_version,
                graph_json=graph_json,
                graph_sha256=hashlib.sha256(graph_json.encode("utf-8")).hexdigest(),
                validation_status="valid",
                nodes=_graph_nodes(preview_graph),
                edges=[edge.model_dump(mode="json") for edge in preview_graph.edges],
                created_at=now,
            )
            unit_of_work.concept_projects.set_current_version(
                project_id=preview_graph.project_id,
                version_id=version_id,
                updated_at=now,
            )
            confirmed = DesignChangeSet.model_validate_json(row["change_set_json"]).model_copy(
                update={"status": "confirmed"}
            )
            unit_of_work.change_sets.confirm(
                change_set_id=change_set_id,
                change_set_json=_canonical_json(confirmed.model_dump(mode="json")),
                result_version_id=version_id,
                confirmed_at=now,
            )
            response = ChangeSetConfirmResponse(
                change_set=confirmed,
                project=project_detail_from_uow(
                    unit_of_work,
                    project_id=preview_graph.project_id,
                ),
            )
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=now,
            )
            return response

    def _mark_stale_if_current_version_changed(self, change_set_id: str) -> bool:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            row = unit_of_work.change_sets.get(change_set_id)
            if row is None or row["status"] != "previewed":
                return False
            project = unit_of_work.concept_projects.get_active(str(row["project_id"]))
            if project is None or project["current_version_id"] == row["base_version_id"]:
                return False
            stale = DesignChangeSet.model_validate_json(row["change_set_json"]).model_copy(
                update={"status": "stale"}
            )
            now = _utc_now()
            unit_of_work.change_sets.mark_stale(
                change_set_id,
                change_set_json=_canonical_json(stale.model_dump(mode="json")),
                diagnostic_json=_canonical_json(
                    _change_set_diagnostic(
                        stale,
                        code="CHANGE_SET_STALE",
                        message=(
                            "Project current version changed after preview; preview again on a "
                            "new base."
                        ),
                        stage="confirm",
                        recorded_at=now,
                    ).model_dump(mode="json")
                ),
                updated_at=now,
            )
            return True


def _change_set_from_plan(
    plan: ConceptChangePlan,
    *,
    project_id: str,
    version_id: str,
    base_graph: ModuleGraph,
) -> DesignChangeSet:
    suffix = uuid.uuid4().hex[:12]
    protected_node_ids = list(
        dict.fromkeys(
            [
                base_graph.root_node_id,
                *(node.node_id for node in base_graph.nodes if node.locked),
            ]
        )
    )
    operations = [
        DesignChangeOperation(
            operation_id=f"op_plan_{suffix}_{index}",
            op=item.op,
            node_id=item.node_id,
            module_id=item.module_id,
            path=item.path,
            value=item.value,
            mirror_axis=item.mirror_axis,
        )
        for index, item in enumerate(plan.operations, start=1)
    ]
    return DesignChangeSet(
        change_set_id=f"change_plan_{suffix}",
        project_id=project_id,
        base_version_id=version_id,
        summary=plan.summary,
        operations=operations,
        protected_node_ids=protected_node_ids,
        status="proposed",
    )


def _validate_planner_context(
    request: PlanDesignChangeSetRequest,
    *,
    base_graph: ModuleGraph,
    registry_module_ids: set[str],
) -> None:
    node_ids = {node.node_id for node in base_graph.nodes}
    if request.selected_node_id and request.selected_node_id not in node_ids:
        raise ConceptChangeSetError(
            "INVALID_REQUEST",
            f"Selected node does not exist in the base graph: {request.selected_node_id}",
        )
    if request.selected_module_id and request.selected_module_id not in registry_module_ids:
        raise ConceptChangeSetError(
            "INVALID_REQUEST",
            f"Selected module is not registered for the project Profile: {request.selected_module_id}",
        )


def _validate_change_plan(
    plan: ConceptChangePlan,
    *,
    current_spec: WeaponConceptSpec,
    base_graph: ModuleGraph,
    module_catalog: list[dict[str, str]],
    registry_module_ids: set[str],
) -> None:
    nodes = {node.node_id: node for node in base_graph.nodes}
    module_categories = {
        item["module_id"]: item["category"] for item in module_catalog
    }
    seen_targets: set[tuple[str, str]] = set()
    current_spec_payload = current_spec.model_dump(mode="json")
    allowed_style_paths = {
        "style.keywords",
        "style.palette",
        "style.detail_density",
    }
    allowed_parameter_paths = {
        "proportions.overall_length_mm",
        "proportions.body_height_mm",
        "proportions.grip_angle_deg",
    }
    for operation in plan.operations:
        target_key = operation.path or operation.node_id or ""
        signature = (operation.op, target_key)
        if signature in seen_targets:
            raise ConceptChangeSetError(
                "PLANNER_BAD_OUTPUT",
                f"Planner returned duplicate operations for {operation.op}:{target_key}.",
            )
        seen_targets.add(signature)
        if operation.op in {"replace_module", "set_mirror"}:
            node = nodes.get(operation.node_id or "")
            if (
                node is None
                or node.node_id == base_graph.root_node_id
                or node.locked
            ):
                raise ConceptChangeSetError(
                    "PLANNER_BAD_OUTPUT",
                    f"Planner target must be an editable non-root node: {operation.node_id}",
                )
            if operation.op == "set_mirror" and operation.mirror_axis == node.mirror_axis:
                raise ConceptChangeSetError(
                    "PLANNER_BAD_OUTPUT",
                    f"Planner mirror operation is a no-op for {node.node_id}.",
                )
        if operation.op == "replace_module":
            module_id = operation.module_id or ""
            if module_id not in registry_module_ids:
                raise ConceptChangeSetError(
                    "PLANNER_BAD_OUTPUT",
                    f"Planner referenced an unregistered module: {module_id}",
                )
            node = nodes[operation.node_id or ""]
            if node.module_id == module_id:
                raise ConceptChangeSetError(
                    "PLANNER_BAD_OUTPUT",
                    f"Planner replacement is a no-op for {node.node_id}.",
                )
            if module_categories.get(node.module_id) != module_categories.get(module_id):
                raise ConceptChangeSetError(
                    "PLANNER_BAD_OUTPUT",
                    "Planner replacement must use a registered module from the same category.",
                )
        elif operation.op == "set_style":
            if operation.path not in allowed_style_paths:
                raise ConceptChangeSetError(
                    "PLANNER_BAD_OUTPUT",
                    f"Planner used an unsupported style path: {operation.path}",
                )
            section, key = operation.path.split(".")
            current_value = current_spec_payload[section][key]
            if operation.value == current_value:
                raise ConceptChangeSetError(
                    "PLANNER_BAD_OUTPUT",
                    f"Planner style operation is a no-op: {operation.path}",
                )
            if key == "detail_density" and not (
                isinstance(operation.value, (int, float))
                and not isinstance(operation.value, bool)
                and 0 <= float(operation.value) <= 1
            ):
                raise ConceptChangeSetError(
                    "PLANNER_BAD_OUTPUT", "style.detail_density must be inside [0, 1]."
                )
            if key in {"keywords", "palette"} and not (
                isinstance(operation.value, list)
                and operation.value
                and all(isinstance(value, str) and value for value in operation.value)
            ):
                raise ConceptChangeSetError(
                    "PLANNER_BAD_OUTPUT", f"{operation.path} must be a non-empty string list."
                )
            if key == "keywords" and isinstance(operation.value, list) and len(
                operation.value
            ) > 12:
                raise ConceptChangeSetError(
                    "PLANNER_BAD_OUTPUT", "style.keywords cannot exceed 12 values."
                )
            if key == "palette" and isinstance(operation.value, list) and len(
                operation.value
            ) > 8:
                raise ConceptChangeSetError(
                    "PLANNER_BAD_OUTPUT", "style.palette cannot exceed 8 values."
                )
        elif operation.op == "set_parameter":
            if operation.path not in allowed_parameter_paths:
                raise ConceptChangeSetError(
                    "PLANNER_BAD_OUTPUT",
                    f"Planner used an unsupported parameter path: {operation.path}",
                )
            if not isinstance(operation.value, (int, float)) or isinstance(
                operation.value, bool
            ):
                raise ConceptChangeSetError(
                    "PLANNER_BAD_OUTPUT", f"{operation.path} must be numeric."
                )
            section, key = operation.path.split(".")
            if float(operation.value) == float(current_spec_payload[section][key]):
                raise ConceptChangeSetError(
                    "PLANNER_BAD_OUTPUT",
                    f"Planner parameter operation is a no-op: {operation.path}",
                )
            numeric_value = float(operation.value)
            if key in {"overall_length_mm", "body_height_mm"} and not (
                0 < numeric_value <= 1000
            ):
                raise ConceptChangeSetError(
                    "PLANNER_BAD_OUTPUT", f"{operation.path} must be inside (0, 1000]."
                )
            if key == "grip_angle_deg" and not (-45 <= numeric_value <= 45):
                raise ConceptChangeSetError(
                    "PLANNER_BAD_OUTPUT", "proportions.grip_angle_deg must be inside [-45, 45]."
                )


def _change_planner_module_catalog(
    unit_of_work: SQLiteUnitOfWork, *, profile_id: str
) -> list[dict[str, str]]:
    profile = unit_of_work.domain_profiles.get_active(profile_id)
    if profile is None:
        raise ConceptChangeSetError(
            "DOMAIN_PROFILE_NOT_FOUND", "The project domain profile is unavailable."
        )
    return [
        {"module_id": str(row["module_id"]), "category": str(row["category"])}
        for row in unit_of_work.modules.list_manifests(
            pack_id=str(profile["pack_id"]), limit=500
        )
    ]


def _change_planner_error(
    exc: ConceptPlannerError | ConceptChangeSetError | ValidationError,
) -> ConceptChangeSetError:
    if isinstance(exc, ConceptChangeSetError):
        return exc
    if isinstance(exc, ConceptPlannerError):
        return ConceptChangeSetError(exc.code, str(exc))
    return ConceptChangeSetError(
        "PLANNER_BAD_OUTPUT",
        f"Change Planner output failed validation: {exc.errors()[0]['msg']}",
    )


def _change_planner_warning(
    exc: ConceptPlannerError | ConceptChangeSetError | ValidationError,
) -> str:
    if isinstance(exc, (ConceptPlannerError, ConceptChangeSetError)):
        return f"{exc.code}: {exc}"
    return f"PLANNER_BAD_OUTPUT: {exc.errors()[0]['msg']}"


def _apply_change_set(
    base_spec: WeaponConceptSpec,
    base_graph: ModuleGraph,
    change_set: DesignChangeSet,
) -> tuple[WeaponConceptSpec, ModuleGraph]:
    spec_payload = json.loads(_canonical_json(base_spec.model_dump(mode="json")))
    graph_payload = json.loads(_canonical_json(base_graph.model_dump(mode="json")))
    graph_payload["graph_id"] = _new_id("mg")
    for operation in change_set.operations:
        _apply_operation(spec_payload, graph_payload, operation)
    try:
        return (
            WeaponConceptSpec.model_validate(spec_payload),
            ModuleGraph.model_validate(graph_payload),
        )
    except ValidationError as exc:
        raise ConceptChangeSetError(
            "CHANGE_SET_INVALID",
            f"ChangeSet produced an invalid Concept contract: {exc.errors()[0]['msg']}",
        ) from exc


def _assert_locked_nodes_unchanged(
    base_graph: ModuleGraph,
    change_set: DesignChangeSet,
) -> None:
    locked_node_ids = {node.node_id for node in base_graph.nodes if node.locked}
    protected_operations = {"remove_module", "replace_module", "set_transform", "set_mirror"}
    for operation in change_set.operations:
        if operation.op in protected_operations and operation.node_id in locked_node_ids:
            raise ConceptChangeSetError(
                "CHANGE_SET_INVALID",
                f"Locked ModuleGraph node cannot be changed: {operation.node_id}",
            )


def _remap_replaced_connectors(
    unit_of_work: SQLiteUnitOfWork,
    *,
    base_graph: ModuleGraph,
    preview_graph: ModuleGraph,
    change_set: DesignChangeSet,
) -> ModuleGraph:
    base_nodes = {node.node_id: node for node in base_graph.nodes}
    graph_payload = preview_graph.model_dump(mode="json")
    for operation in change_set.operations:
        if operation.op != "replace_module" or not operation.node_id or not operation.module_id:
            continue
        base_node = base_nodes.get(operation.node_id)
        if base_node is None:
            continue
        connector_rows = unit_of_work.modules.connector_map(
            [base_node.module_id, operation.module_id]
        )
        old_connectors = {
            connector_id: row
            for connector_id, row in connector_rows.items()
            if row["module_id"] == base_node.module_id
        }
        new_connectors = [
            row
            for row in connector_rows.values()
            if row["module_id"] == operation.module_id
        ]
        for edge in graph_payload["edges"]:
            if edge["from_node_id"] == operation.node_id:
                edge["from_connector_id"] = _replacement_connector_id(
                    old_connectors,
                    new_connectors,
                    edge["from_connector_id"],
                    operation.node_id,
                )
            if edge["to_node_id"] == operation.node_id:
                edge["to_connector_id"] = _replacement_connector_id(
                    old_connectors,
                    new_connectors,
                    edge["to_connector_id"],
                    operation.node_id,
                )
    return ModuleGraph.model_validate(graph_payload)


def _replacement_connector_id(
    old_connectors: dict[str, Any],
    new_connectors: list[Any],
    old_connector_id: str,
    node_id: str,
) -> str:
    old = old_connectors.get(old_connector_id)
    if old is None:
        raise ConceptChangeSetError(
            "CHANGE_SET_INVALID",
            f"Cannot resolve existing connector {old_connector_id} on {node_id}.",
        )
    matches = [
        row
        for row in new_connectors
        if row["slot"] == old["slot"] and row["connector_type"] == old["connector_type"]
    ]
    if len(matches) != 1:
        raise ConceptChangeSetError(
            "CHANGE_SET_INVALID",
            (
                f"Replacement module must expose exactly one compatible connector for "
                f"{old['slot']} ({old['connector_type']}); found {len(matches)}."
            ),
        )
    return str(matches[0]["connector_id"])


def _snap_graph_after_replacements(
    unit_of_work: SQLiteUnitOfWork,
    *,
    graph: ModuleGraph,
    change_set: DesignChangeSet,
) -> ModuleGraph:
    changed_node_ids = {
        str(operation.node_id)
        for operation in change_set.operations
        if operation.op in {"replace_module", "set_mirror"} and operation.node_id
    }
    if not changed_node_ids:
        return graph

    payload = graph.model_dump(mode="json")
    nodes = {node["node_id"]: node for node in payload["nodes"]}
    connector_rows = unit_of_work.modules.connector_map(
        [str(node["module_id"]) for node in payload["nodes"]]
    )
    parent_edges, traversal = _rooted_parent_edges(graph)
    descendants: dict[str, set[str]] = {node_id: set() for node_id in nodes}
    for child_id in reversed(traversal[1:]):
        parent_id, _ = parent_edges[child_id]
        descendants[parent_id].add(child_id)
        descendants[parent_id].update(descendants[child_id])
    affected = set(changed_node_ids)
    for node_id in changed_node_ids:
        affected.update(descendants.get(node_id, set()))
    affected.discard(graph.root_node_id)

    for child_id in traversal[1:]:
        if child_id not in affected:
            continue
        parent_id, edge = parent_edges[child_id]
        parent_connector_id, child_connector_id = _edge_connector_ids(
            edge,
            parent_id=parent_id,
            child_id=child_id,
        )
        parent_connector = _connector_transform(
            connector_rows,
            parent_connector_id,
            parent_id,
        )
        child_connector = _connector_transform(
            connector_rows,
            child_connector_id,
            child_id,
        )
        parent_transform = Transform.model_validate(nodes[parent_id]["transform"])
        child_transform = Transform.model_validate(nodes[child_id]["transform"])
        snapped_transform = snap_child_transform(
            parent_transform=parent_transform,
            parent_connector=parent_connector,
            parent_mirror_axis=str(nodes[parent_id].get("mirror_axis", "none")),
            child_scale=child_transform.scale,
            child_connector=child_connector,
            child_mirror_axis=str(nodes[child_id].get("mirror_axis", "none")),
        )
        if nodes[child_id]["locked"] and _transforms_differ(
            snapped_transform,
            child_transform,
        ):
            raise ConceptChangeSetError(
                "CHANGE_SET_INVALID",
                f"Locked ModuleGraph node cannot be repositioned by Connector snap: {child_id}",
            )
        nodes[child_id]["transform"] = snapped_transform.model_dump(mode="json")

    snapped = ModuleGraph.model_validate(payload)
    snapped_nodes = {node.node_id: node for node in snapped.nodes}
    for edge in snapped.edges:
        if not ({edge.from_node_id, edge.to_node_id} & affected):
            continue
        first_connector = _connector_transform(
            connector_rows,
            edge.from_connector_id,
            edge.from_node_id,
        )
        second_connector = _connector_transform(
            connector_rows,
            edge.to_connector_id,
            edge.to_node_id,
        )
        distance_mm, rotation_degrees = connector_alignment_error(
            first_transform=snapped_nodes[edge.from_node_id].transform,
            first_connector=first_connector,
            first_mirror_axis=snapped_nodes[edge.from_node_id].mirror_axis,
            second_transform=snapped_nodes[edge.to_node_id].transform,
            second_connector=second_connector,
            second_mirror_axis=snapped_nodes[edge.to_node_id].mirror_axis,
        )
        if distance_mm > 0.1 or rotation_degrees > 0.1:
            raise ConceptChangeSetError(
                "CHANGE_SET_INVALID",
                (
                    f"Connector snap conflict on {edge.edge_id}: "
                    f"{distance_mm:.4f} mm / {rotation_degrees:.4f} deg."
                ),
            )
    return snapped


def _rooted_parent_edges(
    graph: ModuleGraph,
) -> tuple[dict[str, tuple[str, Any]], list[str]]:
    adjacency: dict[str, list[tuple[str, Any]]] = {
        node.node_id: [] for node in graph.nodes
    }
    for edge in graph.edges:
        adjacency[edge.from_node_id].append((edge.to_node_id, edge))
        adjacency[edge.to_node_id].append((edge.from_node_id, edge))
    for entries in adjacency.values():
        entries.sort(key=lambda item: (item[1].edge_id, item[0]))
    parent_edges: dict[str, tuple[str, Any]] = {}
    traversal = [graph.root_node_id]
    pending = [graph.root_node_id]
    visited = {graph.root_node_id}
    while pending:
        parent_id = pending.pop(0)
        for child_id, edge in adjacency[parent_id]:
            if child_id in visited:
                continue
            visited.add(child_id)
            parent_edges[child_id] = (parent_id, edge)
            traversal.append(child_id)
            pending.append(child_id)
    return parent_edges, traversal


def _transforms_differ(first: Transform, second: Transform) -> bool:
    return any(
        abs(left - right) > 1e-7
        for left, right in zip(
            [*first.position, *first.rotation, *first.scale],
            [*second.position, *second.rotation, *second.scale],
        )
    )


def _edge_connector_ids(
    edge: Any,
    *,
    parent_id: str,
    child_id: str,
) -> tuple[str, str]:
    if edge.from_node_id == parent_id and edge.to_node_id == child_id:
        return edge.from_connector_id, edge.to_connector_id
    if edge.to_node_id == parent_id and edge.from_node_id == child_id:
        return edge.to_connector_id, edge.from_connector_id
    raise ConceptChangeSetError(
        "CHANGE_SET_INVALID",
        f"Edge {edge.edge_id} does not connect {parent_id} to {child_id}.",
    )


def _connector_transform(
    connector_rows: dict[str, Any],
    connector_id: str,
    node_id: str,
) -> Transform:
    row = connector_rows.get(connector_id)
    if row is None:
        raise ConceptChangeSetError(
            "CHANGE_SET_INVALID",
            f"Cannot resolve connector {connector_id} on {node_id} for snapping.",
        )
    return Transform.model_validate_json(row["transform_json"])


def _apply_operation(
    spec: dict[str, Any],
    graph: dict[str, Any],
    operation: DesignChangeOperation,
) -> None:
    nodes = graph["nodes"]
    edges = graph["edges"]
    if operation.op == "add_module":
        if any(node["node_id"] == operation.node_id for node in nodes):
            raise ConceptChangeSetError("CHANGE_SET_INVALID", "add_module node_id already exists.")
        nodes.append(
            {
                "node_id": operation.node_id,
                "module_id": operation.module_id,
                "transform": operation.transform.model_dump(mode="json"),
                "locked": False,
                "visible": True,
            }
        )
    elif operation.op == "remove_module":
        if operation.node_id == graph["root_node_id"]:
            raise ConceptChangeSetError("CHANGE_SET_INVALID", "Cannot remove the root node.")
        _require_node(nodes, operation.node_id)
        graph["nodes"] = [node for node in nodes if node["node_id"] != operation.node_id]
        graph["edges"] = [
            edge
            for edge in edges
            if edge["from_node_id"] != operation.node_id
            and edge["to_node_id"] != operation.node_id
        ]
    elif operation.op == "replace_module":
        node = _require_node(nodes, operation.node_id)
        node["module_id"] = operation.module_id
    elif operation.op == "set_transform":
        node = _require_node(nodes, operation.node_id)
        node["transform"] = operation.transform.model_dump(mode="json")
    elif operation.op == "set_mirror":
        node = _require_node(nodes, operation.node_id)
        node["mirror_axis"] = operation.mirror_axis
    elif operation.op == "connect":
        if any(edge["edge_id"] == operation.edge_id for edge in edges):
            raise ConceptChangeSetError("CHANGE_SET_INVALID", "connect edge_id already exists.")
        edges.append(
            {
                "edge_id": operation.edge_id,
                "from_node_id": operation.from_node_id,
                "from_connector_id": operation.from_connector_id,
                "to_node_id": operation.to_node_id,
                "to_connector_id": operation.to_connector_id,
                "status": "connected",
            }
        )
    elif operation.op == "disconnect":
        if not any(edge["edge_id"] == operation.edge_id for edge in edges):
            raise ConceptChangeSetError("CHANGE_SET_INVALID", "disconnect edge_id was not found.")
        graph["edges"] = [edge for edge in edges if edge["edge_id"] != operation.edge_id]
    elif operation.op == "set_style":
        _set_allowed_path(spec, operation.path, operation.value, prefix="style.")
    elif operation.op == "set_parameter":
        _set_allowed_path(spec, operation.path, operation.value, prefix="proportions.")


def _set_allowed_path(
    payload: dict[str, Any],
    path: str,
    value: Any,
    *,
    prefix: str,
) -> None:
    if not path.startswith(prefix) or path.count(".") != 1:
        raise ConceptChangeSetError(
            "CHANGE_SET_INVALID",
            f"Unsupported ChangeSet path: {path}",
        )
    section, key = path.split(".")
    if key not in payload.get(section, {}):
        raise ConceptChangeSetError("CHANGE_SET_INVALID", f"Unknown ChangeSet path: {path}")
    payload[section][key] = value


def _require_node(nodes: list[dict[str, Any]], node_id: str) -> dict[str, Any]:
    for node in nodes:
        if node["node_id"] == node_id:
            return node
    raise ConceptChangeSetError("CHANGE_SET_INVALID", f"Node was not found: {node_id}")


def _graph_nodes(graph: ModuleGraph) -> list[dict[str, Any]]:
    return [
        {
            **node.model_dump(mode="json", exclude={"transform"}),
            "transform_json": _canonical_json(node.transform.model_dump(mode="json")),
        }
        for node in graph.nodes
    ]


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def change_set_timeline_item_from_row(row: Any) -> ChangeSetTimelineItem:
    """Build the public audit/timeline model from one persisted ChangeSet row."""
    return ChangeSetTimelineItem(
        change_set=DesignChangeSet.model_validate_json(row["change_set_json"]),
        base_version_id=str(row["base_version_id"]),
        result_version_id=(
            str(row["result_version_id"])
            if row["result_version_id"] is not None
            else None
        ),
        status=str(row["status"]),
        actor_type=str(row["actor_type"]),
        planner_instruction=(
            str(row["planner_instruction"])
            if row["planner_instruction"] is not None
            else None
        ),
        planner_rationale=(
            json.loads(row["planner_rationale_json"])
            if row["planner_rationale_json"] is not None
            else []
        ),
        planner_provenance=(
            ConceptPlannerProvenance.model_validate_json(row["planner_provenance_json"])
            if row["planner_provenance_json"] is not None
            else None
        ),
        planner_job_id=(
            str(row["planner_job_id"])
            if row["planner_job_id"] is not None
            else None
        ),
        preview_sha256=(
            str(row["preview_sha256"])
            if row["preview_sha256"] is not None
            else None
        ),
        diagnostic=(
            ChangeSetDiagnostic.model_validate_json(row["diagnostic_json"])
            if row["diagnostic_json"] is not None
            else None
        ),
        created_at=str(row["created_at"]),
        updated_at=str(row["updated_at"]),
        confirmed_at=(
            str(row["confirmed_at"])
            if row["confirmed_at"] is not None
            else None
        ),
    )


def _change_set_diagnostic(
    change_set: DesignChangeSet,
    *,
    code: str,
    message: str,
    stage: str,
    recorded_at: str,
) -> ChangeSetDiagnostic:
    node_ids = set(change_set.protected_node_ids)
    for operation in change_set.operations:
        for node_id in (
            operation.node_id,
            operation.from_node_id,
            operation.to_node_id,
        ):
            if node_id:
                node_ids.add(node_id)
    return ChangeSetDiagnostic(
        code=code,
        message=message,
        stage=stage,
        recoverable=True,
        operation_ids=[operation.operation_id for operation in change_set.operations],
        node_ids=sorted(node_ids),
        recorded_at=recorded_at,
    )


def _encode_timeline_cursor(
    updated_at: str,
    change_set_id: str,
    filter_hash: str,
) -> str:
    payload = _canonical_json(
        {
            "updated_at": updated_at,
            "change_set_id": change_set_id,
            "filter_hash": filter_hash,
        }
    ).encode("utf-8")
    return base64.urlsafe_b64encode(payload).decode("ascii").rstrip("=")


def _decode_timeline_cursor(
    cursor: str,
    expected_filter_hash: str,
) -> tuple[str, str]:
    try:
        padding = "=" * (-len(cursor) % 4)
        payload = json.loads(base64.urlsafe_b64decode(cursor + padding).decode("utf-8"))
    except (binascii.Error, UnicodeDecodeError, json.JSONDecodeError, ValueError) as exc:
        raise ConceptChangeSetError("INVALID_CURSOR", "ChangeSet cursor is invalid.") from exc
    if (
        not isinstance(payload, dict)
        or set(payload) != {"updated_at", "change_set_id", "filter_hash"}
        or not all(isinstance(value, str) and value for value in payload.values())
        or payload["filter_hash"] != expected_filter_hash
    ):
        raise ConceptChangeSetError(
            "INVALID_CURSOR",
            "ChangeSet cursor does not match the current filters.",
        )
    return payload["updated_at"], payload["change_set_id"]


def _hash_json(value: Any) -> str:
    return hashlib.sha256(_canonical_json(value).encode("utf-8")).hexdigest()


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex[:12]}"


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()
