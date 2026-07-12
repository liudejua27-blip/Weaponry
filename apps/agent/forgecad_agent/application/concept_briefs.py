from __future__ import annotations

import hashlib
import json
import uuid
from datetime import datetime, timezone
from typing import Any, Optional

from pydantic import ValidationError

from forgecad_agent.application.concept_models import (
    DesignBriefRecord,
    ConceptPlannerProvenance,
    DesignVariantListResponse,
    DesignVariantRecord,
    GenerateDesignVariantsRequest,
    InterpretDesignBriefRequest,
    SelectDesignVariantRequest,
)
from forgecad_agent.application.concept_planner import (
    ConceptPlannerError,
    ConceptPlannerProvider,
    ConceptVariantPlan,
    DeterministicConceptPlanner,
    planner_provenance,
)
from forgecad_agent.application.concept_modules import validate_registered_graph
from forgecad_agent.application.concept_jobs import record_completed_job
from forgecad_agent.domain.concepts.models import ModuleGraph, WeaponConceptSpec
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork


class ConceptBriefError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


class ConceptBriefIdempotencyConflict(RuntimeError):
    pass


class ConceptBriefService:
    """Interpret briefs and derive registry-bound A/B/C graph variants."""

    def __init__(
        self,
        connection_factory: SQLiteConnectionFactory,
        planner: Optional[ConceptPlannerProvider] = None,
    ) -> None:
        self.connection_factory = connection_factory
        self.planner = planner or DeterministicConceptPlanner()
        self.deterministic_planner = DeterministicConceptPlanner()

    def interpret(
        self,
        project_id: str,
        request: InterpretDesignBriefRequest,
        idempotency_key: str,
    ) -> DesignBriefRecord:
        scope = f"POST /api/v1/projects/{project_id}/brief:interpret"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            replay = unit_of_work.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise ConceptBriefIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return DesignBriefRecord.model_validate_json(replay.response_json)

            project = unit_of_work.concept_projects.get_active(project_id)
            if project is None:
                raise ConceptBriefError("PROJECT_NOT_FOUND", "Concept project not found.")
            current_version_id = project["current_version_id"]
            version = unit_of_work.concept_projects.get_version(
                project_id,
                str(current_version_id),
            )
            if version is None:
                raise ConceptBriefError("VERSION_NOT_FOUND", "Current project version not found.")
            for asset_id in request.reference_asset_ids:
                asset = unit_of_work.concept_assets.get_active(asset_id)
                if asset is None:
                    raise ConceptBriefError(
                        "ASSET_NOT_FOUND",
                        f"Brief reference asset was not found: {asset_id}",
                    )
                if asset["project_id"] not in {None, project_id}:
                    raise ConceptBriefError(
                        "INVALID_REQUEST",
                        "Brief reference asset belongs to another project.",
                    )

            brief_id = _new_id("brief")
            now = _utc_now()
            current_spec = WeaponConceptSpec.model_validate_json(version["spec_json"])
            module_catalog = _module_catalog(unit_of_work, str(project["profile_id"]))
            provider, spec, fallback_used, warnings, attempted_provider = (
                self._interpret_with_provider(
                    request=request,
                    current_spec=current_spec,
                    module_catalog=module_catalog,
                )
            )
            provenance = planner_provenance(
                provider,
                input_payload={
                    "source_text": request.source_text,
                    "current_spec": current_spec.model_dump(mode="json"),
                    "module_catalog": module_catalog,
                },
                output_payload=spec.model_dump(mode="json"),
                registry_module_ids=[item["module_id"] for item in module_catalog],
                attempted_provider=attempted_provider,
                fallback_used=fallback_used,
                warnings=warnings,
            )
            unit_of_work.brief_variants.add_brief(
                brief_id=brief_id,
                project_id=project_id,
                source_text=request.source_text,
                reference_asset_ids_json=_canonical_json(request.reference_asset_ids),
                interpreted_spec_json=_canonical_json(spec.model_dump(mode="json")),
                planner_provenance_json=_canonical_json(
                    provenance.model_dump(mode="json")
                ),
                status="interpreted",
                created_at=now,
            )
            job_id = record_completed_job(
                unit_of_work,
                project_id=project_id,
                version_id=str(current_version_id),
                job_type="interpret_brief",
                input_payload=request.model_dump(mode="json"),
                output_payload={
                    "brief_id": brief_id,
                    "status": "interpreted",
                    "provider_id": provenance.provider_id,
                    "generator": provenance.generator,
                    "fallback_used": provenance.fallback_used,
                },
                steps=(
                    (
                        "load_current_spec",
                        "Current WeaponConceptSpec and registry catalog loaded.",
                        0.35,
                        {
                            "version_id": current_version_id,
                            "registry_module_count": len(module_catalog),
                        },
                    ),
                    (
                        "interpret_brief",
                        "Brief interpretation completed through the configured planner boundary.",
                        1.0,
                        {
                            "brief_id": brief_id,
                            "provider_id": provenance.provider_id,
                            "generator": provenance.generator,
                            "fallback_used": provenance.fallback_used,
                        },
                    ),
                ),
            )
            response = DesignBriefRecord(
                brief_id=brief_id,
                project_id=project_id,
                source_text=request.source_text,
                reference_asset_ids=request.reference_asset_ids,
                interpreted_spec=spec,
                status="interpreted",
                planner_provenance=provenance,
                created_at=now,
                updated_at=now,
                job_id=job_id,
            )
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=now,
            )
            return response

    def get_brief(self, project_id: str, brief_id: str) -> DesignBriefRecord:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            row = unit_of_work.brief_variants.get_brief(project_id, brief_id)
            if row is None:
                raise ConceptBriefError("BRIEF_NOT_FOUND", "Design brief not found.")
            return _brief_record(row)

    def generate_variants(
        self,
        project_id: str,
        request: GenerateDesignVariantsRequest,
        idempotency_key: str,
    ) -> DesignVariantListResponse:
        scope = f"POST /api/v1/projects/{project_id}/variants"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            replay = unit_of_work.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise ConceptBriefIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return DesignVariantListResponse.model_validate_json(replay.response_json)

            project = unit_of_work.concept_projects.get_active(project_id)
            if project is None:
                raise ConceptBriefError("PROJECT_NOT_FOUND", "Concept project not found.")
            brief = unit_of_work.brief_variants.get_brief(project_id, request.brief_id)
            if brief is None:
                raise ConceptBriefError("BRIEF_NOT_FOUND", "Design brief not found.")
            existing = unit_of_work.brief_variants.list_variants(
                project_id,
                brief_id=request.brief_id,
            )
            if existing:
                job_id = record_completed_job(
                    unit_of_work,
                    project_id=project_id,
                    version_id=str(project["current_version_id"]),
                    job_type="generate_variants",
                    input_payload=request.model_dump(mode="json"),
                    output_payload={
                        "brief_id": request.brief_id,
                        "variant_ids": [row["variant_id"] for row in existing],
                        "reused": True,
                    },
                    steps=(
                        (
                            "reuse_variants",
                            "Existing persisted planner variants reused.",
                            1.0,
                            {"variant_count": len(existing)},
                        ),
                    ),
                )
                response = DesignVariantListResponse(
                    items=[_variant_record(row) for row in existing],
                    next_cursor=None,
                    job_id=job_id,
                )
                unit_of_work.idempotency.add(
                    scope=scope,
                    key=idempotency_key,
                    request_hash=request_hash,
                    response_json=_canonical_json(response.model_dump(mode="json")),
                    created_at=_utc_now(),
                )
                return response

            version = unit_of_work.concept_projects.get_version(
                project_id,
                str(project["current_version_id"]),
            )
            if version is None or version["module_graph_id"] is None:
                raise ConceptBriefError(
                    "MODULE_GRAPH_NOT_FOUND",
                    "A/B/C variants require a current version with a validated ModuleGraph.",
                )
            graph_row = unit_of_work.modules.get_graph(str(version["module_graph_id"]))
            if graph_row is None:
                raise ConceptBriefError("MODULE_GRAPH_NOT_FOUND", "Current ModuleGraph not found.")
            profile = unit_of_work.domain_profiles.get_active(str(project["profile_id"]))
            if profile is None:
                raise ConceptBriefError(
                    "DOMAIN_PROFILE_NOT_FOUND",
                    "The project domain profile is unavailable.",
                )
            base_graph = ModuleGraph.model_validate_json(graph_row["graph_json"])
            interpreted_spec = WeaponConceptSpec.model_validate_json(
                brief["interpreted_spec_json"]
            )
            module_catalog = _module_catalog(unit_of_work, str(project["profile_id"]))
            provider, plans, fallback_used, warnings, attempted_provider = (
                self._variants_with_provider(
                    request=request,
                    source_text=str(brief["source_text"]),
                    interpreted_spec=interpreted_spec,
                    base_graph=base_graph,
                    module_catalog=module_catalog,
                )
            )
            _validate_variant_plans(
                plans,
                base_graph=base_graph,
                registry_module_ids={item["module_id"] for item in module_catalog},
            )
            provenance = planner_provenance(
                provider,
                input_payload={
                    "source_text": brief["source_text"],
                    "interpreted_spec": interpreted_spec.model_dump(mode="json"),
                    "base_graph": base_graph.model_dump(mode="json"),
                    "module_catalog": module_catalog,
                },
                output_payload=[item.model_dump(mode="json") for item in plans],
                registry_module_ids=[item["module_id"] for item in module_catalog],
                attempted_provider=attempted_provider,
                fallback_used=fallback_used,
                warnings=warnings,
            )
            now = _utc_now()
            items: list[DesignVariantRecord] = []
            for plan in plans:
                graph = _planned_variant(base_graph, plan)
                issues = validate_registered_graph(
                    unit_of_work,
                    graph=graph,
                    profile_pack_id=str(profile["pack_id"]),
                )
                if issues:
                    raise ConceptBriefError(
                        "VARIANT_GENERATION_FAILED",
                        f"Planner variant {plan.rank} failed validation: {issues[0].code}",
                    )
                variant = DesignVariantRecord(
                    variant_id=_new_id("variant"),
                    project_id=project_id,
                    brief_id=request.brief_id,
                    rank=plan.rank,
                    name=plan.name,
                    summary=plan.summary,
                    module_graph=graph,
                    recommended_module_ids=plan.recommended_module_ids,
                    rationale=plan.rationale,
                    planner_provenance=provenance,
                    status="proposed",
                    created_at=now,
                )
                unit_of_work.brief_variants.add_variant(
                    variant_id=variant.variant_id,
                    project_id=project_id,
                    brief_id=request.brief_id,
                    rank=plan.rank,
                    name=plan.name,
                    summary=plan.summary,
                    module_graph_json=_canonical_json(graph.model_dump(mode="json")),
                    recommended_module_ids_json=_canonical_json(
                        plan.recommended_module_ids
                    ),
                    rationale_json=_canonical_json(plan.rationale),
                    planner_provenance_json=_canonical_json(
                        provenance.model_dump(mode="json")
                    ),
                    status="proposed",
                    created_at=now,
                )
                items.append(variant)
            job_id = record_completed_job(
                unit_of_work,
                project_id=project_id,
                version_id=str(project["current_version_id"]),
                job_type="generate_variants",
                input_payload=request.model_dump(mode="json"),
                output_payload={
                    "brief_id": request.brief_id,
                    "variant_ids": [item.variant_id for item in items],
                    "provider_id": provenance.provider_id,
                    "generator": provenance.generator,
                    "fallback_used": provenance.fallback_used,
                },
                steps=(
                    (
                        "load_module_graph",
                        "Validated base ModuleGraph loaded.",
                        0.25,
                        {"graph_id": base_graph.graph_id},
                    ),
                    (
                        "generate_variants",
                        "Three planner graph variants generated.",
                        0.75,
                        {
                            "variant_count": len(items),
                            "provider_id": provenance.provider_id,
                            "generator": provenance.generator,
                        },
                    ),
                    (
                        "validate_variants",
                        "All A/B/C variants passed connector validation.",
                        1.0,
                        {"variant_ids": [item.variant_id for item in items]},
                    ),
                ),
            )
            response = DesignVariantListResponse(
                items=items,
                next_cursor=None,
                job_id=job_id,
            )
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=now,
            )
            return response

    def list_variants(
        self,
        project_id: str,
        *,
        brief_id: Optional[str] = None,
    ) -> DesignVariantListResponse:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            if unit_of_work.concept_projects.get_active(project_id) is None:
                raise ConceptBriefError("PROJECT_NOT_FOUND", "Concept project not found.")
            rows = unit_of_work.brief_variants.list_variants(
                project_id,
                brief_id=brief_id,
            )
            return DesignVariantListResponse(
                items=[_variant_record(row) for row in rows],
                next_cursor=None,
            )

    def select_variant(
        self,
        project_id: str,
        variant_id: str,
        request: SelectDesignVariantRequest,
        idempotency_key: str,
    ) -> DesignVariantRecord:
        scope = f"POST /api/v1/projects/{project_id}/variants/{variant_id}:select"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            replay = unit_of_work.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise ConceptBriefIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return DesignVariantRecord.model_validate_json(replay.response_json)
            row = unit_of_work.brief_variants.get_variant(project_id, variant_id)
            if row is None:
                raise ConceptBriefError("VARIANT_NOT_FOUND", "Design variant not found.")
            unit_of_work.brief_variants.select_variant(
                project_id=project_id,
                brief_id=str(row["brief_id"]),
                variant_id=variant_id,
            )
            selected_row = unit_of_work.brief_variants.get_variant(project_id, variant_id)
            response = _variant_record(selected_row)
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=_utc_now(),
            )
            return response

    def _interpret_with_provider(
        self,
        *,
        request: InterpretDesignBriefRequest,
        current_spec: WeaponConceptSpec,
        module_catalog: list[dict[str, str]],
    ) -> tuple[
        ConceptPlannerProvider,
        WeaponConceptSpec,
        bool,
        list[str],
        Optional[ConceptPlannerProvider],
    ]:
        provider = self._provider_for(request.generator)
        try:
            spec = provider.interpret_brief(
                source_text=request.source_text,
                current_spec=current_spec,
                module_catalog=module_catalog,
            )
            return provider, spec, False, [], None
        except (ConceptPlannerError, ValidationError) as exc:
            if request.generator == "auto" and provider.provider_type != "deterministic":
                warning = _planner_warning(exc)
                spec = self.deterministic_planner.interpret_brief(
                    source_text=request.source_text,
                    current_spec=current_spec,
                    module_catalog=module_catalog,
                )
                return self.deterministic_planner, spec, True, [warning], provider
            raise _concept_planner_error(exc) from exc

    def _variants_with_provider(
        self,
        *,
        request: GenerateDesignVariantsRequest,
        source_text: str,
        interpreted_spec: WeaponConceptSpec,
        base_graph: ModuleGraph,
        module_catalog: list[dict[str, str]],
    ) -> tuple[
        ConceptPlannerProvider,
        list[ConceptVariantPlan],
        bool,
        list[str],
        Optional[ConceptPlannerProvider],
    ]:
        provider = self._provider_for(request.generator)
        registry_module_ids = {item["module_id"] for item in module_catalog}
        try:
            plans = provider.plan_variants(
                source_text=source_text,
                interpreted_spec=interpreted_spec,
                base_graph=base_graph,
                module_catalog=module_catalog,
            )
            _validate_variant_plans(
                plans,
                base_graph=base_graph,
                registry_module_ids=registry_module_ids,
            )
            return provider, plans, False, [], None
        except (ConceptPlannerError, ConceptBriefError, ValidationError) as exc:
            if request.generator == "auto" and provider.provider_type != "deterministic":
                warning = _planner_warning(exc)
                plans = self.deterministic_planner.plan_variants(
                    source_text=source_text,
                    interpreted_spec=interpreted_spec,
                    base_graph=base_graph,
                    module_catalog=module_catalog,
                )
                _validate_variant_plans(
                    plans,
                    base_graph=base_graph,
                    registry_module_ids=registry_module_ids,
                )
                return self.deterministic_planner, plans, True, [warning], provider
            raise _concept_planner_error(exc) from exc

    def _provider_for(self, generator: str) -> ConceptPlannerProvider:
        if generator in {"deterministic_rules", "deterministic_template"}:
            return self.deterministic_planner
        return self.planner


def _planned_variant(base_graph: ModuleGraph, plan: ConceptVariantPlan) -> ModuleGraph:
    payload = json.loads(_canonical_json(base_graph.model_dump(mode="json")))
    payload["graph_id"] = _new_id("mg")
    candidate = next(
        (node for node in payload["nodes"] if node["node_id"] == plan.target_node_id),
        None,
    )
    if candidate is None:
        raise ConceptBriefError(
            "VARIANT_GENERATION_FAILED",
            f"Planner target node does not exist: {plan.target_node_id}",
        )
    candidate["transform"]["scale"] = plan.scale
    for adjustment in plan.node_transforms:
        node = next(
            (item for item in payload["nodes"] if item["node_id"] == adjustment.node_id),
            None,
        )
        if node is None:
            raise ConceptBriefError(
                "VARIANT_GENERATION_FAILED",
                f"Planner transform target does not exist: {adjustment.node_id}",
            )
        if adjustment.position is not None:
            node["transform"]["position"] = adjustment.position
        if adjustment.rotation is not None:
            node["transform"]["rotation"] = adjustment.rotation
        if adjustment.scale is not None:
            node["transform"]["scale"] = adjustment.scale
        if adjustment.mirror_axis is not None:
            node["mirror_axis"] = adjustment.mirror_axis
    return ModuleGraph.model_validate(payload)


def _module_catalog(
    unit_of_work: SQLiteUnitOfWork, profile_id: str
) -> list[dict[str, str]]:
    profile = unit_of_work.domain_profiles.get_active(profile_id)
    if profile is None:
        raise ConceptBriefError(
            "DOMAIN_PROFILE_NOT_FOUND", "The project domain profile is unavailable."
        )
    return [
        {"module_id": str(row["module_id"]), "category": str(row["category"])}
        for row in unit_of_work.modules.list_manifests(
            pack_id=str(profile["pack_id"]), limit=500
        )
    ]


def _validate_variant_plans(
    plans: list[ConceptVariantPlan],
    *,
    base_graph: ModuleGraph,
    registry_module_ids: set[str],
) -> None:
    if len(plans) != 3 or sorted(item.rank for item in plans) != [1, 2, 3]:
        raise ConceptBriefError(
            "PLANNER_BAD_OUTPUT", "Planner must return exactly ranks 1, 2, and 3."
        )
    nodes = {node.node_id: node for node in base_graph.nodes}
    signatures: set[str] = set()
    for plan in plans:
        node = nodes.get(plan.target_node_id)
        if node is None or node.node_id == base_graph.root_node_id or node.locked:
            raise ConceptBriefError(
                "PLANNER_BAD_OUTPUT",
                f"Planner target must be an editable non-root node: {plan.target_node_id}",
            )
        unknown = sorted(set(plan.recommended_module_ids) - registry_module_ids)
        if unknown:
            raise ConceptBriefError(
                "PLANNER_BAD_OUTPUT",
                f"Planner referenced unregistered modules: {unknown}",
            )
        for adjustment in plan.node_transforms:
            adjusted_node = nodes.get(adjustment.node_id)
            if (
                adjusted_node is None
                or adjusted_node.node_id == base_graph.root_node_id
                or adjusted_node.locked
            ):
                raise ConceptBriefError(
                    "PLANNER_BAD_OUTPUT",
                    f"Planner transform target must be an editable non-root node: {adjustment.node_id}",
                )
        signature = _canonical_json(
            {
                "target_node_id": plan.target_node_id,
                "scale": plan.scale,
                "node_transforms": [
                    item.model_dump(mode="json") for item in plan.node_transforms
                ],
            }
        )
        if signature in signatures:
            raise ConceptBriefError(
                "PLANNER_BAD_OUTPUT", "Planner variants must be structurally distinct."
            )
        signatures.add(signature)


def _concept_planner_error(
    exc: ConceptPlannerError | ConceptBriefError | ValidationError,
) -> ConceptBriefError:
    if isinstance(exc, ConceptBriefError):
        return exc
    if isinstance(exc, ConceptPlannerError):
        return ConceptBriefError(exc.code, str(exc))
    return ConceptBriefError(
        "PLANNER_BAD_OUTPUT",
        f"Concept Planner output failed validation: {exc.errors()[0]['msg']}",
    )


def _planner_warning(
    exc: ConceptPlannerError | ConceptBriefError | ValidationError,
) -> str:
    if isinstance(exc, (ConceptPlannerError, ConceptBriefError)):
        return f"{exc.code}: {exc}"
    return f"PLANNER_BAD_OUTPUT: {exc.errors()[0]['msg']}"


def _brief_record(row: Any) -> DesignBriefRecord:
    return DesignBriefRecord(
        brief_id=row["brief_id"],
        project_id=row["project_id"],
        source_text=row["source_text"],
        reference_asset_ids=json.loads(row["reference_asset_ids_json"]),
        interpreted_spec=WeaponConceptSpec.model_validate_json(row["interpreted_spec_json"]),
        status=row["status"],
        planner_provenance=ConceptPlannerProvenance.model_validate_json(
            row["planner_provenance_json"]
        ),
        created_at=row["created_at"],
        updated_at=row["updated_at"],
    )


def _variant_record(row: Any) -> DesignVariantRecord:
    if row is None:
        raise ConceptBriefError("VARIANT_NOT_FOUND", "Design variant not found.")
    return DesignVariantRecord(
        variant_id=row["variant_id"],
        project_id=row["project_id"],
        brief_id=row["brief_id"],
        rank=row["rank"],
        name=row["name"],
        summary=row["summary"],
        module_graph=ModuleGraph.model_validate_json(row["module_graph_json"]),
        recommended_module_ids=json.loads(row["recommended_module_ids_json"]),
        rationale=json.loads(row["rationale_json"]),
        planner_provenance=ConceptPlannerProvenance.model_validate_json(
            row["planner_provenance_json"]
        ),
        status=row["status"],
        created_at=row["created_at"],
    )


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _hash_json(value: Any) -> str:
    return hashlib.sha256(_canonical_json(value).encode("utf-8")).hexdigest()


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex[:12]}"


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()
