from __future__ import annotations

import json
from typing import Annotated, List, Optional, Union

from fastapi import APIRouter, Header, Query
from fastapi.responses import JSONResponse, StreamingResponse

from forgecad_agent.application.agent_kernel import (
    AgentKernelError,
    AgentKernelIdempotencyConflict,
    AgentKernelService,
)
from forgecad_agent.application.agent_models import (
    AgentApproval,
    AgentApprovalResolution,
    AgentThreadDetail,
    AgentThreadListResponse,
    AgentTurn,
    CreateAgentApprovalRequest,
    CreateAgentThreadRequest,
    ResolveAgentApprovalRequest,
    StartAgentTurnRequest,
    BuildAgentBlockoutRequest,
    BuildAgentBlockoutResponse,
    RenderAgentBlockoutConceptPreviewRequest,
    AgentBlockoutConceptPreview,
    SegmentAgentBlockoutRequest,
    SegmentAgentBlockoutResponse,
    AgentMaterialPreset,
    AgentProviderCheckResponse,
    AgentMaterialTextureListResponse,
    AgentMaterialTextureObject,
    RegisterAgentMaterialTextureRequest,
)
from forgecad_agent.application.material_textures import (
    MaterialTextureError,
    MaterialTextureIdempotencyConflict,
    MaterialTextureService,
)
from forgecad_agent.application.domain_packs import DomainPackManifest, list_domain_packs
from forgecad_agent.application.material_catalog import list_material_presets


def build_agent_router(
    service: AgentKernelService,
    material_texture_service: Optional[MaterialTextureService] = None,
) -> APIRouter:
    router = APIRouter(prefix="/api/v1/agent", tags=["agent-kernel"])

    @router.post("/threads", response_model=AgentThreadDetail, status_code=201)
    def create_thread(
        request: CreateAgentThreadRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[AgentThreadDetail, JSONResponse]:
        try:
            return service.create_thread(request, _require_idempotency_key(idempotency_key))
        except AgentKernelIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except AgentKernelError as exc:
            return _agent_error(exc)

    @router.get("/threads", response_model=AgentThreadListResponse)
    def list_threads() -> AgentThreadListResponse:
        return service.list_threads()

    @router.get("/domain-packs", response_model=List[DomainPackManifest])
    def get_domain_packs() -> List[DomainPackManifest]:
        return list_domain_packs()

    @router.get("/materials", response_model=List[AgentMaterialPreset])
    def get_materials() -> List[AgentMaterialPreset]:
        presets = list_material_presets()
        if material_texture_service is not None:
            return material_texture_service.enrich_catalog(presets)
        return presets

    @router.post("/material-textures", response_model=AgentMaterialTextureObject, status_code=201)
    def register_material_texture(
        request: RegisterAgentMaterialTextureRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[AgentMaterialTextureObject, JSONResponse]:
        if material_texture_service is None:
            return _error(503, "TEXTURE_SERVICE_UNAVAILABLE", "纹理对象服务未启用。")
        try:
            return material_texture_service.register(request, _require_idempotency_key(idempotency_key))
        except MaterialTextureIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except MaterialTextureError as exc:
            return _error(exc.status_code, exc.code, str(exc))

    @router.get("/material-textures", response_model=AgentMaterialTextureListResponse)
    def list_material_textures(
        texture_role: Optional[str] = Query(default=None),
        source: Optional[str] = Query(default=None),
        q: Optional[str] = Query(default=None, max_length=120),
        limit: int = Query(default=100, ge=1, le=100),
    ) -> Union[AgentMaterialTextureListResponse, JSONResponse]:
        if material_texture_service is None:
            return _error(503, "TEXTURE_SERVICE_UNAVAILABLE", "纹理对象服务未启用。")
        try:
            return material_texture_service.list(texture_role=texture_role, source=source, query=q, limit=limit)
        except MaterialTextureError as exc:
            return _error(exc.status_code, exc.code, str(exc))

    @router.get("/material-textures/{texture_asset_id}", response_model=AgentMaterialTextureObject)
    def get_material_texture(texture_asset_id: str) -> Union[AgentMaterialTextureObject, JSONResponse]:
        if material_texture_service is None:
            return _error(503, "TEXTURE_SERVICE_UNAVAILABLE", "纹理对象服务未启用。")
        try:
            return material_texture_service.get(texture_asset_id)
        except MaterialTextureError as exc:
            return _error(exc.status_code, exc.code, str(exc))

    @router.post("/provider:check", response_model=AgentProviderCheckResponse)
    def check_provider() -> AgentProviderCheckResponse:
        return service.check_provider()

    @router.post("/blockouts", response_model=BuildAgentBlockoutResponse, status_code=201)
    def build_blockout(
        request: BuildAgentBlockoutRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[BuildAgentBlockoutResponse, JSONResponse]:
        try:
            return service.build_blockout(request, _require_idempotency_key(idempotency_key))
        except AgentKernelIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except AgentKernelError as exc:
            return _agent_error(exc)

    @router.post("/blockouts:concept-preview", response_model=AgentBlockoutConceptPreview)
    def render_blockout_concept_preview(
        request: RenderAgentBlockoutConceptPreviewRequest,
    ) -> Union[AgentBlockoutConceptPreview, JSONResponse]:
        try:
            return service.render_blockout_concept_preview(request)
        except AgentKernelError as exc:
            return _agent_error(exc)

    @router.post("/blockouts:segment", response_model=SegmentAgentBlockoutResponse, status_code=201)
    def segment_blockout(
        request: SegmentAgentBlockoutRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[SegmentAgentBlockoutResponse, JSONResponse]:
        try:
            return service.segment_blockout(request, _require_idempotency_key(idempotency_key))
        except AgentKernelIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except AgentKernelError as exc:
            return _agent_error(exc)

    @router.get("/threads/{thread_id}", response_model=AgentThreadDetail)
    def get_thread(thread_id: str) -> Union[AgentThreadDetail, JSONResponse]:
        try:
            return service.get_thread(thread_id)
        except AgentKernelError as exc:
            return _agent_error(exc)

    @router.post("/threads/{thread_id}/turns", response_model=AgentTurn, status_code=201)
    def start_turn(
        thread_id: str,
        request: StartAgentTurnRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[AgentTurn, JSONResponse]:
        try:
            return service.start_turn(thread_id, request, _require_idempotency_key(idempotency_key))
        except AgentKernelIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except AgentKernelError as exc:
            return _agent_error(exc)

    @router.post("/turns/{turn_id}/cancel", response_model=AgentTurn)
    def cancel_turn(
        turn_id: str,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[AgentTurn, JSONResponse]:
        try:
            return service.cancel_turn(turn_id, _require_idempotency_key(idempotency_key))
        except AgentKernelIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except AgentKernelError as exc:
            return _agent_error(exc)

    @router.post("/threads/{thread_id}/approvals", response_model=AgentApproval, status_code=201)
    def create_approval(
        thread_id: str,
        request: CreateAgentApprovalRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[AgentApproval, JSONResponse]:
        try:
            return service.create_approval(thread_id, request, _require_idempotency_key(idempotency_key))
        except AgentKernelIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except AgentKernelError as exc:
            return _agent_error(exc)

    @router.post("/approvals/{approval_id}/resolve", response_model=AgentApprovalResolution)
    def resolve_approval(
        approval_id: str,
        request: ResolveAgentApprovalRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[AgentApprovalResolution, JSONResponse]:
        try:
            return service.resolve_approval(
                approval_id,
                request,
                _require_idempotency_key(idempotency_key),
            )
        except AgentKernelIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except AgentKernelError as exc:
            return _agent_error(exc)

    @router.get("/threads/{thread_id}/events", response_model=None)
    def stream_events(
        thread_id: str,
        after: Annotated[int, Query(ge=0)] = 0,
        last_event_id: Annotated[Optional[str], Header(alias="Last-Event-ID")] = None,
    ) -> Union[StreamingResponse, JSONResponse]:
        try:
            cursor = after
            if last_event_id and last_event_id.isdigit():
                cursor = max(cursor, int(last_event_id))
            events = service.events(thread_id, after=cursor)
        except AgentKernelError as exc:
            return _agent_error(exc)

        def body():
            for event in events:
                yield f"id: {event.sequence}\n"
                yield "event: agent.item\n"
                yield f"data: {json.dumps(event.model_dump(mode='json'), ensure_ascii=False)}\n\n"
            yield "event: agent.replay.complete\n"
            yield "data: {}\n\n"

        return StreamingResponse(
            body(),
            media_type="text/event-stream",
            headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
        )

    return router


def _require_idempotency_key(value: Optional[str]) -> str:
    if not value:
        raise AgentKernelError("IDEMPOTENCY_KEY_REQUIRED", "Idempotency-Key is required.")
    return value


def _agent_error(exc: AgentKernelError) -> JSONResponse:
    return _error(exc.status_code, exc.code, str(exc), details=exc.details)


def _error(status_code: int, code: str, message: str, *, details: Optional[dict] = None) -> JSONResponse:
    return JSONResponse(
        status_code=status_code,
        content={
            "error": {
                "code": code,
                "message": message,
                "recoverable": status_code >= 500 or status_code == 409,
                "details": details or {},
            }
        },
    )
