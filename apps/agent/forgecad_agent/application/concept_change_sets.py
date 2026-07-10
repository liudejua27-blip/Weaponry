from __future__ import annotations

import hashlib
import json
import uuid
from datetime import datetime, timezone
from typing import Any

from pydantic import ValidationError

from forgecad_agent.application.concept_models import (
    ChangeSetConfirmResponse,
    ChangeSetPreviewResponse,
    ProposeChangeSetRequest,
)
from forgecad_agent.application.concept_modules import validate_registered_graph
from forgecad_agent.application.concept_projects import project_detail_from_uow
from forgecad_agent.domain.concepts.models import (
    DesignChangeOperation,
    DesignChangeSet,
    ModuleGraph,
    WeaponConceptSpec,
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

    def __init__(self, connection_factory: SQLiteConnectionFactory) -> None:
        self.connection_factory = connection_factory

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
            preview_spec, preview_graph = _apply_change_set(
                base_spec,
                base_graph,
                change_set,
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
            unit_of_work.change_sets.mark_stale(
                change_set_id,
                change_set_json=_canonical_json(stale.model_dump(mode="json")),
                updated_at=_utc_now(),
            )
            return True


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


def _hash_json(value: Any) -> str:
    return hashlib.sha256(_canonical_json(value).encode("utf-8")).hexdigest()


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex[:12]}"


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()
