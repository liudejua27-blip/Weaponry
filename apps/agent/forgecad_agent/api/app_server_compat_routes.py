from __future__ import annotations

from typing import Any, Optional

from fastapi import APIRouter, Query, Request
from fastapi.responses import JSONResponse
from pydantic import BaseModel, ConfigDict, Field

from forgecad_agent.application.app_server_compat import (
    AppServerCompatibilityAdapter,
    ProtocolFailure,
)


class _StrictTransportModel(BaseModel):
    model_config = ConfigDict(extra="forbid")


class OpenCompatibilityConnectionRequest(_StrictTransportModel):
    queue_capacity: Optional[int] = Field(default=None, ge=1, le=4096)


class SendCompatibilityFrameRequest(_StrictTransportModel):
    frame: dict[str, Any]


def build_app_server_compat_router(adapter: AppServerCompatibilityAdapter) -> APIRouter:
    """Expose the restricted loopback development transport.

    The packaged desktop uses the same JSON-RPC frame contract through Tauri.
    These routes are a compatibility transport only; they do not create a
    second product API or persistent state source.
    """

    # The app-server wire contract is Rust/manifest-owned and intentionally
    # separate from the legacy product HTTP OpenAPI during K001 migration.
    router = APIRouter(
        prefix="/api/v1/app-server",
        tags=["app-server-compatibility"],
        include_in_schema=False,
    )

    @router.post("/connections")
    async def open_connection(
        request: Request,
        body: Optional[OpenCompatibilityConnectionRequest] = None,
    ) -> Any:
        denied = _loopback_only(request)
        if denied is not None:
            return denied
        try:
            connection_id = await adapter.open_connection(body.queue_capacity if body is not None else None)
        except ProtocolFailure as exc:
            return _transport_error(exc)
        return {"connection_id": connection_id}

    @router.post("/connections/{connection_id}/frames")
    async def send_frame(
        connection_id: str,
        body: SendCompatibilityFrameRequest,
        request: Request,
    ) -> Any:
        denied = _loopback_only(request)
        if denied is not None:
            return denied
        try:
            return await adapter.send_frame(connection_id, body.frame)
        except ProtocolFailure as exc:
            return _transport_error(exc)

    @router.get("/connections/{connection_id}/frames")
    async def replay_frames(
        connection_id: str,
        request: Request,
        after: int = Query(default=0, ge=0),
    ) -> Any:
        denied = _loopback_only(request)
        if denied is not None:
            return denied
        try:
            return await adapter.replay_transport(connection_id, after)
        except ProtocolFailure as exc:
            return _transport_error(exc)

    @router.post("/connections/{connection_id}:close")
    async def close_connection(connection_id: str, request: Request) -> Any:
        denied = _loopback_only(request)
        if denied is not None:
            return denied
        if not await adapter.close_connection(connection_id):
            return _transport_error(
                ProtocolFailure(-32006, "CONNECTION_NOT_FOUND", "The app-server connection was not found.")
            )
        return {"closed": True}

    return router


def _loopback_only(request: Request) -> Optional[JSONResponse]:
    host = request.client.host if request.client is not None else ""
    if host in {"127.0.0.1", "::1", "localhost", "testclient"}:
        return None
    return JSONResponse(
        status_code=403,
        content={
            "error": {
                "code": "LOOPBACK_REQUIRED",
                "message": "The app-server compatibility transport is loopback-only.",
                "recoverable": False,
                "details": {},
            }
        },
    )


def _transport_error(error: ProtocolFailure) -> JSONResponse:
    if error.application_code == "CONNECTION_NOT_FOUND":
        status = 404
    elif error.application_code == "CURSOR_RESYNC_REQUIRED":
        status = 409
    else:
        status = 400
    return JSONResponse(status_code=status, content={"error": error.envelope()})
