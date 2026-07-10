from __future__ import annotations

import hashlib
import json
import uuid
from datetime import datetime, timezone
from typing import Any, Optional

from forgecad_agent.application.concept_models import (
    DesignBriefRecord,
    DesignVariantListResponse,
    DesignVariantRecord,
    GenerateDesignVariantsRequest,
    InterpretDesignBriefRequest,
    SelectDesignVariantRequest,
)
from forgecad_agent.application.concept_modules import validate_registered_graph
from forgecad_agent.domain.concepts.models import ModuleGraph, WeaponConceptSpec
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork


class ConceptBriefError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


class ConceptBriefIdempotencyConflict(RuntimeError):
    pass


class ConceptBriefService:
    """Persist a brief and derive deterministic A/B/C graph variants for R2."""

    def __init__(self, connection_factory: SQLiteConnectionFactory) -> None:
        self.connection_factory = connection_factory

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
            spec = WeaponConceptSpec.model_validate_json(version["spec_json"])
            unit_of_work.brief_variants.add_brief(
                brief_id=brief_id,
                project_id=project_id,
                source_text=request.source_text,
                reference_asset_ids_json=_canonical_json(request.reference_asset_ids),
                interpreted_spec_json=_canonical_json(spec.model_dump(mode="json")),
                status="interpreted",
                created_at=now,
            )
            response = DesignBriefRecord(
                brief_id=brief_id,
                project_id=project_id,
                source_text=request.source_text,
                reference_asset_ids=request.reference_asset_ids,
                interpreted_spec=spec,
                status="interpreted",
                created_at=now,
                updated_at=now,
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
                response = DesignVariantListResponse(
                    items=[_variant_record(row) for row in existing],
                    next_cursor=None,
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
            now = _utc_now()
            items: list[DesignVariantRecord] = []
            for rank, (name, summary, scale_factor) in enumerate(
                (
                    ("A · 紧凑巡逻型", "压缩非核心前部轮廓，强调紧凑比例。", 0.9),
                    ("B · 均衡基准型", "保持当前模块比例，作为比较基准。", 1.0),
                    ("C · 延展展示型", "延长非核心前部轮廓，强调展示张力。", 1.1),
                ),
                start=1,
            ):
                graph = _scaled_variant(base_graph, scale_factor)
                issues = validate_registered_graph(
                    unit_of_work,
                    graph=graph,
                    profile_pack_id=str(profile["pack_id"]),
                )
                if issues:
                    raise ConceptBriefError(
                        "VARIANT_GENERATION_FAILED",
                        f"Deterministic variant {rank} failed validation: {issues[0].code}",
                    )
                variant = DesignVariantRecord(
                    variant_id=_new_id("variant"),
                    project_id=project_id,
                    brief_id=request.brief_id,
                    rank=rank,
                    name=name,
                    summary=summary,
                    module_graph=graph,
                    status="proposed",
                    created_at=now,
                )
                unit_of_work.brief_variants.add_variant(
                    variant_id=variant.variant_id,
                    project_id=project_id,
                    brief_id=request.brief_id,
                    rank=rank,
                    name=name,
                    summary=summary,
                    module_graph_json=_canonical_json(graph.model_dump(mode="json")),
                    status="proposed",
                    created_at=now,
                )
                items.append(variant)
            response = DesignVariantListResponse(items=items, next_cursor=None)
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


def _scaled_variant(base_graph: ModuleGraph, scale_factor: float) -> ModuleGraph:
    payload = json.loads(_canonical_json(base_graph.model_dump(mode="json")))
    payload["graph_id"] = _new_id("mg")
    candidate = next(
        (
            node
            for node in payload["nodes"]
            if node["node_id"] != payload["root_node_id"] and not node["locked"]
        ),
        None,
    )
    if candidate is None:
        raise ConceptBriefError(
            "VARIANT_GENERATION_FAILED",
            "Current ModuleGraph has no editable non-root module.",
        )
    candidate["transform"]["scale"][0] = scale_factor
    return ModuleGraph.model_validate(payload)


def _brief_record(row: Any) -> DesignBriefRecord:
    return DesignBriefRecord(
        brief_id=row["brief_id"],
        project_id=row["project_id"],
        source_text=row["source_text"],
        reference_asset_ids=json.loads(row["reference_asset_ids_json"]),
        interpreted_spec=WeaponConceptSpec.model_validate_json(row["interpreted_spec_json"]),
        status=row["status"],
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
