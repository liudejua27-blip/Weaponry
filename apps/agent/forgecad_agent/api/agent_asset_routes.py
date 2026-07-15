from __future__ import annotations

import re
from typing import Annotated, Optional, Union

from fastapi import APIRouter, Header, Query
from fastapi.responses import JSONResponse, Response

from forgecad_agent.application.agent_asset_editing import (
    AgentAssetEditingService,
    AgentAssetError,
    AgentAssetIdempotencyConflict,
)
from forgecad_agent.application.agent_models import (
    AgentAssetChangeSet,
    AgentAssetChangeSetConfirmResponse,
    AgentAssetQualityReport,
    AgentAssetRenderSet,
    AgentAssetExportResponse,
    AgentAssetVersion,
    AgentComponentCandidate,
    AgentComponentRecord,
    AgentStructureSuggestionList,
    CommitAgentBlockoutRequest,
    ImportAgentGlbRequest,
    ImportAgentGlbResponse,
    ProposeAgentAssetChangeSetRequest,
    SaveAgentComponentRequest,
)


def build_agent_asset_router(service: AgentAssetEditingService) -> APIRouter:
    router = APIRouter(prefix="/api/v1/agent", tags=["agent-assets"])

    @router.post("/blockouts:commit", response_model=AgentAssetVersion, status_code=201)
    def commit_blockout(
        request: CommitAgentBlockoutRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[AgentAssetVersion, JSONResponse]:
        try:
            return service.commit_blockout(request, _require_idempotency_key(idempotency_key))
        except AgentAssetIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except AgentAssetError as exc:
            return _agent_error(exc)

    # Register the colon-suffixed GET before the generic asset-version path;
    # otherwise FastAPI would treat ``assetver_x:render`` as an asset id.
    @router.get("/asset-versions/{asset_version_id}:render", response_model=AgentAssetRenderSet)
    def render_asset_views(
        asset_version_id: str,
        width: int = Query(default=640, ge=64, le=2048),
        height: int = Query(default=640, ge=64, le=2048),
    ) -> Union[AgentAssetRenderSet, JSONResponse]:
        try:
            return service.render_views(asset_version_id, width=width, height=height)
        except AgentAssetError as exc:
            return _agent_error(exc)

    @router.get(
        "/asset-versions/{asset_version_id}:render-package",
        response_class=Response,
        response_model=None,
        responses={
            200: {
                "description": "A presentation-only ZIP containing current PNG concept views and manifest.json.",
                "content": {"application/zip": {}},
                "headers": {
                    "Content-Disposition": {"schema": {"type": "string"}},
                    "X-ForgeCAD-Render-Set-SHA256": {"schema": {"type": "string"}},
                },
            }
        },
    )
    def download_render_package(
        asset_version_id: str,
        render_set_sha256: str = Query(min_length=64, max_length=64, pattern=r"^[a-f0-9]{64}$"),
        width: int = Query(default=640, ge=64, le=2048),
        height: int = Query(default=640, ge=64, le=2048),
    ) -> Union[Response, JSONResponse]:
        try:
            payload, manifest = service.render_view_package(
                asset_version_id,
                width=width,
                height=height,
                expected_render_set_sha256=render_set_sha256,
            )
            return Response(
                content=payload,
                media_type="application/zip",
                headers={
                    "Cache-Control": "no-store",
                    "Content-Disposition": f'attachment; filename="{asset_version_id}-concept-views.zip"',
                    "X-ForgeCAD-Render-Set-SHA256": manifest.render_set_sha256,
                },
            )
        except AgentAssetError as exc:
            return _agent_error(exc)

    @router.get("/asset-versions/{asset_version_id}", response_model=AgentAssetVersion)
    def get_asset_version(asset_version_id: str) -> Union[AgentAssetVersion, JSONResponse]:
        try:
            return service.get_version(asset_version_id)
        except AgentAssetError as exc:
            return _agent_error(exc)

    @router.post("/imports:glb", response_model=ImportAgentGlbResponse, status_code=201)
    def import_glb(
        request: ImportAgentGlbRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[ImportAgentGlbResponse, JSONResponse]:
        try:
            return service.import_glb(request, _require_idempotency_key(idempotency_key))
        except AgentAssetIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except AgentAssetError as exc:
            return _agent_error(exc)

    @router.post("/asset-versions/{asset_version_id}/components", response_model=AgentComponentRecord, status_code=201)
    def save_component(
        asset_version_id: str,
        request: SaveAgentComponentRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[AgentComponentRecord, JSONResponse]:
        try:
            return service.save_component(asset_version_id, request, _require_idempotency_key(idempotency_key))
        except AgentAssetIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except AgentAssetError as exc:
            return _agent_error(exc)

    @router.get("/components", response_model=list[AgentComponentRecord])
    def list_components(
        project_id: str = Query(min_length=1),
        domain_pack_id: Optional[str] = None,
        role: Optional[str] = None,
        q: Optional[str] = None,
    ) -> list[AgentComponentRecord]:
        return service.list_components(project_id, domain_pack_id=domain_pack_id, role=role, query=q)

    @router.get("/asset-versions/{asset_version_id}/components:compatible", response_model=list[AgentComponentCandidate])
    def list_component_candidates(
        asset_version_id: str,
        part_id: str = Query(min_length=1),
    ) -> Union[list[AgentComponentCandidate], JSONResponse]:
        try:
            return service.list_component_candidates(asset_version_id, part_id=part_id)
        except AgentAssetError as exc:
            return _agent_error(exc)

    @router.get("/asset-versions/{asset_version_id}/structure-suggestions", response_model=AgentStructureSuggestionList)
    def list_structure_suggestions(asset_version_id: str) -> Union[AgentStructureSuggestionList, JSONResponse]:
        try:
            return service.list_structure_suggestions(asset_version_id)
        except AgentAssetError as exc:
            return _agent_error(exc)

    @router.post("/asset-versions/{asset_version_id}:quality", response_model=AgentAssetQualityReport)
    def quality_asset_version(
        asset_version_id: str,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
        if_match: Annotated[Optional[str], Header(alias="If-Match")] = None,
    ) -> Union[AgentAssetQualityReport, JSONResponse]:
        try:
            return service.quality(
                asset_version_id,
                expected_revision=_resolve_active_design_revision(if_match),
                idempotency_key=_require_idempotency_key(idempotency_key),
            )
        except AgentAssetIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except AgentAssetError as exc:
            return _agent_error(exc)

    @router.get("/quality-reports/{quality_report_id}", response_model=AgentAssetQualityReport)
    def get_quality_report(quality_report_id: str) -> Union[AgentAssetQualityReport, JSONResponse]:
        try:
            return service.get_quality_report(quality_report_id)
        except AgentAssetError as exc:
            return _agent_error(exc)

    @router.post("/asset-versions/{asset_version_id}:export", response_model=AgentAssetExportResponse)
    def export_asset(asset_version_id: str) -> Union[AgentAssetExportResponse, JSONResponse]:
        try:
            return service.export_glb(asset_version_id)
        except AgentAssetError as exc:
            return _agent_error(exc)

    @router.post(
        "/asset-versions/{asset_version_id}/change-sets",
        response_model=AgentAssetChangeSet,
        status_code=201,
    )
    def propose_change_set(
        asset_version_id: str,
        request: ProposeAgentAssetChangeSetRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[AgentAssetChangeSet, JSONResponse]:
        try:
            return service.propose_change_set(asset_version_id, request, _require_idempotency_key(idempotency_key))
        except AgentAssetIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except AgentAssetError as exc:
            return _agent_error(exc)

    @router.post("/change-sets/{change_set_id}:preview", response_model=AgentAssetChangeSet)
    def preview_change_set(
        change_set_id: str,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[AgentAssetChangeSet, JSONResponse]:
        try:
            return service.preview_change_set(change_set_id, _require_idempotency_key(idempotency_key))
        except AgentAssetIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except AgentAssetError as exc:
            return _agent_error(exc)

    @router.post("/change-sets/{change_set_id}:confirm", response_model=AgentAssetChangeSetConfirmResponse)
    def confirm_change_set(
        change_set_id: str,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[AgentAssetChangeSetConfirmResponse, JSONResponse]:
        try:
            return service.confirm_change_set(change_set_id, _require_idempotency_key(idempotency_key))
        except AgentAssetIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except AgentAssetError as exc:
            return _agent_error(exc)

    @router.post("/change-sets/{change_set_id}:reject", response_model=AgentAssetChangeSet)
    def reject_change_set(
        change_set_id: str,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[AgentAssetChangeSet, JSONResponse]:
        try:
            return service.reject_change_set(change_set_id, _require_idempotency_key(idempotency_key))
        except AgentAssetIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except AgentAssetError as exc:
            return _agent_error(exc)

    return router


def _require_idempotency_key(value: Optional[str]) -> str:
    if not value:
        raise AgentAssetError("IDEMPOTENCY_KEY_REQUIRED", "Idempotency-Key is required.")
    return value


_ACTIVE_DESIGN_ETAG_PATTERN = re.compile(r'^W/"active-design-([1-9][0-9]*)"$')


def _resolve_active_design_revision(if_match: Optional[str]) -> int:
    if not if_match:
        raise AgentAssetError("ACTIVE_DESIGN_REVISION_REQUIRED", "质量检查需要当前 ActiveDesignSnapshot 的 If-Match。")
    match = _ACTIVE_DESIGN_ETAG_PATTERN.fullmatch(if_match.strip())
    if match is None:
        raise AgentAssetError("ACTIVE_DESIGN_ETAG_INVALID", 'If-Match must be formatted as W/"active-design-{revision}".')
    return int(match.group(1))


def _agent_error(exc: AgentAssetError) -> JSONResponse:
    return _error(exc.status_code, exc.code, str(exc))


def _error(status_code: int, code: str, message: str) -> JSONResponse:
    return JSONResponse(
        status_code=status_code,
        content={
            "error": {
                "code": code,
                "message": message,
                "recoverable": status_code >= 500 or status_code == 409,
                "details": {},
            }
        },
    )
