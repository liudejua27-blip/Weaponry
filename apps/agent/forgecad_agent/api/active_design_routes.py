from __future__ import annotations

import re
from typing import Annotated, Optional, Union

from fastapi import APIRouter, Header, Response
from fastapi.responses import JSONResponse

from forgecad_agent.application.active_design import (
    ActiveDesignApiError,
    ActiveDesignIdempotencyConflict,
    ActiveDesignService,
)
from forgecad_agent.application.agent_models import (
    ActiveDesignSnapshot,
    ActiveDesignNavigation,
    ConvertLegacyActiveDesignRequest,
    LegacyActiveDesignConversionResponse,
    NavigateActiveDesignRequest,
    SelectActiveDesignRequest,
    SetActiveDesignPartDisplayRequest,
    SetActiveDesignRenderPresetRequest,
)


def build_active_design_router(service: ActiveDesignService) -> APIRouter:
    router = APIRouter(prefix="/api/v1/projects", tags=["active-design"])

    @router.get("/{project_id}/active-design", response_model=ActiveDesignSnapshot)
    def get_active_design(project_id: str, response: Response) -> Union[ActiveDesignSnapshot, JSONResponse]:
        try:
            snapshot = service.get_snapshot(project_id)
            _set_etag(response, snapshot.revision)
            response.headers["Cache-Control"] = "no-store"
            return snapshot
        except ActiveDesignApiError as exc:
            return _active_design_error(exc)

    @router.get("/{project_id}/active-design:navigation", response_model=ActiveDesignNavigation)
    def get_active_design_navigation(project_id: str, response: Response) -> Union[ActiveDesignNavigation, JSONResponse]:
        try:
            # Navigation is a derived read model. It intentionally has no
            # independent ETag; a caller must refresh the Snapshot before a
            # CAS write, so an intermediary must not cache this result.
            response.headers["Cache-Control"] = "no-store"
            return service.get_navigation(project_id)
        except ActiveDesignApiError as exc:
            return _active_design_error(exc)

    @router.post("/{project_id}/active-design:select", response_model=ActiveDesignSnapshot)
    def select_active_design_part(
        project_id: str,
        request: SelectActiveDesignRequest,
        response: Response,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
        if_match: Annotated[Optional[str], Header(alias="If-Match")] = None,
    ) -> Union[ActiveDesignSnapshot, JSONResponse]:
        try:
            snapshot = service.select_part(
                project_id,
                request,
                expected_revision=_resolve_expected_revision(request.snapshot_revision, if_match),
                idempotency_key=_require_idempotency_key(idempotency_key),
            )
            _set_etag(response, snapshot.revision)
            return snapshot
        except ActiveDesignIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ActiveDesignApiError as exc:
            return _active_design_error(exc)

    @router.post("/{project_id}/active-design:render-preset", response_model=ActiveDesignSnapshot)
    def set_active_design_render_preset(
        project_id: str,
        request: SetActiveDesignRenderPresetRequest,
        response: Response,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
        if_match: Annotated[Optional[str], Header(alias="If-Match")] = None,
    ) -> Union[ActiveDesignSnapshot, JSONResponse]:
        try:
            snapshot = service.set_render_preset(
                project_id,
                request,
                expected_revision=_resolve_expected_revision(request.snapshot_revision, if_match),
                idempotency_key=_require_idempotency_key(idempotency_key),
            )
            _set_etag(response, snapshot.revision)
            return snapshot
        except ActiveDesignIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ActiveDesignApiError as exc:
            return _active_design_error(exc)

    @router.post("/{project_id}/active-design:part-display", response_model=ActiveDesignSnapshot)
    def set_active_design_part_display(
        project_id: str,
        request: SetActiveDesignPartDisplayRequest,
        response: Response,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
        if_match: Annotated[Optional[str], Header(alias="If-Match")] = None,
    ) -> Union[ActiveDesignSnapshot, JSONResponse]:
        try:
            snapshot = service.set_part_display(
                project_id,
                request,
                expected_revision=_resolve_expected_revision(request.snapshot_revision, if_match),
                idempotency_key=_require_idempotency_key(idempotency_key),
            )
            _set_etag(response, snapshot.revision)
            return snapshot
        except ActiveDesignIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ActiveDesignApiError as exc:
            return _active_design_error(exc)

    @router.post(
        "/{project_id}/active-design:convert-legacy",
        response_model=LegacyActiveDesignConversionResponse,
    )
    def convert_legacy_active_design(
        project_id: str,
        request: ConvertLegacyActiveDesignRequest,
        response: Response,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
        if_match: Annotated[Optional[str], Header(alias="If-Match")] = None,
    ) -> Union[LegacyActiveDesignConversionResponse, JSONResponse]:
        try:
            result = service.convert_legacy(
                project_id,
                request,
                expected_revision=_resolve_expected_revision(request.snapshot_revision, if_match),
                idempotency_key=_require_idempotency_key(idempotency_key),
            )
            _set_etag(response, result.snapshot_revision)
            return result
        except ActiveDesignIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ActiveDesignApiError as exc:
            return _active_design_error(exc)

    @router.post("/{project_id}/active-design:undo", response_model=ActiveDesignSnapshot)
    def undo_active_design(
        project_id: str,
        request: NavigateActiveDesignRequest,
        response: Response,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
        if_match: Annotated[Optional[str], Header(alias="If-Match")] = None,
    ) -> Union[ActiveDesignSnapshot, JSONResponse]:
        try:
            snapshot = service.navigate_asset(
                project_id,
                request,
                expected_revision=_resolve_expected_revision(request.snapshot_revision, if_match),
                idempotency_key=_require_idempotency_key(idempotency_key),
                action="undo",
            )
            _set_etag(response, snapshot.revision)
            return snapshot
        except ActiveDesignIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ActiveDesignApiError as exc:
            return _active_design_error(exc)

    @router.post("/{project_id}/active-design:redo", response_model=ActiveDesignSnapshot)
    def redo_active_design(
        project_id: str,
        request: NavigateActiveDesignRequest,
        response: Response,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
        if_match: Annotated[Optional[str], Header(alias="If-Match")] = None,
    ) -> Union[ActiveDesignSnapshot, JSONResponse]:
        try:
            snapshot = service.navigate_asset(
                project_id,
                request,
                expected_revision=_resolve_expected_revision(request.snapshot_revision, if_match),
                idempotency_key=_require_idempotency_key(idempotency_key),
                action="redo",
            )
            _set_etag(response, snapshot.revision)
            return snapshot
        except ActiveDesignIdempotencyConflict as exc:
            return _error(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ActiveDesignApiError as exc:
            return _active_design_error(exc)

    return router


_ETAG_PATTERN = re.compile(r'^W/"active-design-([1-9][0-9]*)"$')


def _resolve_expected_revision(snapshot_revision: Optional[int], if_match: Optional[str]) -> int:
    etag_revision: Optional[int] = None
    if if_match:
        match = _ETAG_PATTERN.fullmatch(if_match.strip())
        if match is None:
            raise ActiveDesignApiError(
                "ACTIVE_DESIGN_ETAG_INVALID",
                'If-Match must be formatted as W/"active-design-{revision}".',
                status_code=400,
            )
        etag_revision = int(match.group(1))
    if snapshot_revision is None and etag_revision is None:
        raise ActiveDesignApiError(
            "ACTIVE_DESIGN_REVISION_REQUIRED",
            "snapshot_revision 或 If-Match 是必填项。",
            status_code=400,
        )
    if snapshot_revision is not None and etag_revision is not None and snapshot_revision != etag_revision:
        raise ActiveDesignApiError(
            "ACTIVE_DESIGN_STALE",
            "snapshot_revision 与 If-Match 不一致，请刷新后重试。",
            status_code=409,
        )
    return snapshot_revision if snapshot_revision is not None else etag_revision  # type: ignore[return-value]


def _set_etag(response: Response, revision: int) -> None:
    response.headers["ETag"] = f'W/"active-design-{revision}"'


def _require_idempotency_key(value: Optional[str]) -> str:
    if not value:
        raise ActiveDesignApiError("IDEMPOTENCY_KEY_REQUIRED", "Idempotency-Key is required.")
    return value


def _active_design_error(exc: ActiveDesignApiError) -> JSONResponse:
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
