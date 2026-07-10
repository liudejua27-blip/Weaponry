from __future__ import annotations

import hashlib
import json
import uuid
from datetime import datetime, timezone
from typing import Any

from forgecad_agent.application.concept_models import (
    AppendConceptVersionRequest,
    ConceptProjectDetail,
    ConceptProjectListResponse,
    ConceptProjectSummary,
    ConceptVersionSummary,
    CreateConceptProjectRequest,
)
from forgecad_agent.domain.concepts.models import (
    DesignDomainProfile,
    WeaponConceptSpec,
)
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork


class ConceptProjectError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


class ConceptProjectIdempotencyConflict(RuntimeError):
    pass


class ConceptProjectService:
    """Project/version use cases for the new Concept domain."""

    def __init__(self, connection_factory: SQLiteConnectionFactory) -> None:
        self.connection_factory = connection_factory
        self.ensure_default_profile()

    def ensure_default_profile(self) -> DesignDomainProfile:
        profile = default_weapon_concept_profile()
        profile_json = _canonical_json(profile.model_dump(mode="json"))
        profile_sha256 = _sha256_text(profile_json)
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            existing = unit_of_work.domain_profiles.get_active(profile.profile_id)
            if existing is not None and existing["profile_sha256"] == profile_sha256:
                return profile
            now = _utc_now()
            unit_of_work.domain_profiles.upsert(
                profile_id=profile.profile_id,
                domain_type=profile.domain_type,
                schema_version=profile.schema_version,
                pack_id=profile.pack_id,
                display_name=profile.display_name,
                profile_json=profile_json,
                profile_sha256=profile_sha256,
                status="active",
                created_at=now,
                updated_at=now,
            )
        return profile

    def create_project(
        self,
        request: CreateConceptProjectRequest,
        idempotency_key: str,
    ) -> ConceptProjectDetail:
        scope = "POST /api/v1/projects"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            existing = unit_of_work.idempotency.get(scope, idempotency_key)
            if existing is not None:
                if existing.request_hash != request_hash:
                    raise ConceptProjectIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return ConceptProjectDetail.model_validate_json(existing.response_json)

            profile_row = unit_of_work.domain_profiles.get_active(request.profile_id)
            if profile_row is None:
                raise ConceptProjectError(
                    "DOMAIN_PROFILE_NOT_FOUND",
                    f"Active domain profile was not found: {request.profile_id}",
                )

            project_id = _new_id("prj")
            version_id = _new_id("ver")
            now = _utc_now()
            spec = WeaponConceptSpec(
                project_id=project_id,
                profile_id=request.profile_id,
                name=request.name,
                intended_uses=request.intended_uses,
                style=request.style,
                proportions=request.proportions,
                required_slots=request.required_slots,
                optional_slots=request.optional_slots,
                constraints=request.constraints,
                assumptions=request.assumptions,
            )
            spec_json = _canonical_json(spec.model_dump(mode="json"))
            unit_of_work.concept_projects.add(
                project_id=project_id,
                profile_id=request.profile_id,
                domain_type="weapon_concept",
                name=request.name,
                status="active",
                created_at=now,
                updated_at=now,
            )
            unit_of_work.concept_projects.add_version(
                version_id=version_id,
                project_id=project_id,
                parent_version_id=None,
                version_no=1,
                status="committed",
                summary="Initial Weapon Concept specification.",
                spec_schema_version=spec.schema_version,
                spec_json=spec_json,
                spec_sha256=_sha256_text(spec_json),
                module_graph_id=None,
                change_set_id=None,
                created_at=now,
            )
            unit_of_work.concept_projects.set_current_version(
                project_id=project_id,
                version_id=version_id,
                updated_at=now,
            )
            response = project_detail_from_uow(
                unit_of_work,
                project_id=project_id,
            )
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=now,
            )
            return response

    def list_projects(self) -> ConceptProjectListResponse:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            rows = unit_of_work.concept_projects.list_active()
            items = [ConceptProjectSummary(**dict(row)) for row in rows]
        return ConceptProjectListResponse(items=items, next_cursor=None)

    def get_project(self, project_id: str) -> ConceptProjectDetail:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            return project_detail_from_uow(unit_of_work, project_id=project_id)

    def append_version(
        self,
        project_id: str,
        request: AppendConceptVersionRequest,
        idempotency_key: str,
    ) -> ConceptProjectDetail:
        scope = f"POST /api/v1/projects/{project_id}/versions"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            existing = unit_of_work.idempotency.get(scope, idempotency_key)
            if existing is not None:
                if existing.request_hash != request_hash:
                    raise ConceptProjectIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return ConceptProjectDetail.model_validate_json(existing.response_json)

            project = unit_of_work.concept_projects.get_active(project_id)
            if project is None:
                raise ConceptProjectError("PROJECT_NOT_FOUND", "Concept project not found.")
            parent = unit_of_work.concept_projects.get_version(
                project_id,
                request.parent_version_id,
            )
            if parent is None:
                raise ConceptProjectError(
                    "VERSION_NOT_FOUND",
                    "Parent version was not found in this project.",
                )
            if request.spec.project_id != project_id:
                raise ConceptProjectError(
                    "INVALID_REQUEST",
                    "WeaponConceptSpec project_id does not match the route project_id.",
                )
            if request.spec.profile_id != project["profile_id"]:
                raise ConceptProjectError(
                    "INVALID_REQUEST",
                    "WeaponConceptSpec profile_id cannot change inside a project.",
                )
            if request.module_graph_id:
                graph = unit_of_work.modules.get_graph(request.module_graph_id)
                if graph is None or graph["project_id"] != project_id:
                    raise ConceptProjectError(
                        "MODULE_GRAPH_NOT_FOUND",
                        "Validated ModuleGraph was not found in this project.",
                    )
                if graph["validation_status"] != "valid":
                    raise ConceptProjectError(
                        "INVALID_REQUEST",
                        "Only a valid ModuleGraph can be attached to a version.",
                    )

            now = _utc_now()
            version_id = _new_id("ver")
            spec_json = _canonical_json(request.spec.model_dump(mode="json"))
            unit_of_work.concept_projects.add_version(
                version_id=version_id,
                project_id=project_id,
                parent_version_id=request.parent_version_id,
                version_no=unit_of_work.concept_projects.next_version_number(project_id),
                status="committed",
                summary=request.summary,
                spec_schema_version=request.spec.schema_version,
                spec_json=spec_json,
                spec_sha256=_sha256_text(spec_json),
                module_graph_id=request.module_graph_id,
                change_set_id=None,
                created_at=now,
            )
            unit_of_work.concept_projects.set_current_version(
                project_id=project_id,
                version_id=version_id,
                updated_at=now,
            )
            if request.module_graph_id:
                unit_of_work.modules.bind_graph_to_version(
                    request.module_graph_id,
                    version_id,
                    updated_at=now,
                )
            response = project_detail_from_uow(unit_of_work, project_id=project_id)
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=now,
            )
            return response


def default_weapon_concept_profile() -> DesignDomainProfile:
    return DesignDomainProfile(
        profile_id="profile_weapon_concept_v1",
        display_name="Weapon Concept Pack",
        pack_id="pack_weapon_concept_v1",
        intended_uses=["visual_asset", "game_asset", "film_prop", "non_functional_display"],
        module_categories=[
            "core_shell",
            "front_shell",
            "rear_shell",
            "grip_shell",
            "top_accessory",
            "side_accessory",
            "lower_structure",
            "storage_visual",
            "armor_panel",
        ],
        required_connectors=["core.front", "core.rear", "core.grip"],
        optional_connectors=[
            "core.top",
            "core.bottom",
            "core.left",
            "core.right",
            "core.side_panel_left",
            "core.side_panel_right",
        ],
        export_profiles=["visual_asset", "game_asset", "film_prop", "non_functional_display"],
    )


def project_detail_from_uow(
    unit_of_work: SQLiteUnitOfWork,
    *,
    project_id: str,
) -> ConceptProjectDetail:
    project = unit_of_work.concept_projects.get_active(project_id)
    if project is None:
        raise ConceptProjectError("PROJECT_NOT_FOUND", "Concept project not found.")
    profile = unit_of_work.domain_profiles.get_active(str(project["profile_id"]))
    if profile is None:
        raise ConceptProjectError(
            "DOMAIN_PROFILE_NOT_FOUND",
            "The project domain profile is unavailable.",
        )
    current_version_id = project["current_version_id"]
    if not current_version_id:
        raise ConceptProjectError(
            "VERSION_NOT_FOUND",
            "The project has no current version.",
        )
    current_version = unit_of_work.concept_projects.get_version(
        project_id,
        str(current_version_id),
    )
    if current_version is None:
        raise ConceptProjectError(
            "VERSION_NOT_FOUND",
            "The current project version is unavailable.",
        )
    versions = [
        ConceptVersionSummary(**dict(row))
        for row in unit_of_work.concept_projects.list_versions(project_id)
    ]
    return ConceptProjectDetail(
        **dict(project),
        profile=DesignDomainProfile.model_validate_json(profile["profile_json"]),
        current_spec=WeaponConceptSpec.model_validate_json(current_version["spec_json"]),
        versions=versions,
    )


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _hash_json(value: Any) -> str:
    return _sha256_text(_canonical_json(value))


def _sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex[:12]}"


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()
