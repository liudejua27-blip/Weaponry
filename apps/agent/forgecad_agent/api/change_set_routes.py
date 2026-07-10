from __future__ import annotations

from typing import Annotated, Optional, Union

from fastapi import APIRouter, Header, HTTPException
from fastapi.responses import JSONResponse

from forgecad_agent.application.concept_change_sets import (
    ConceptChangeSetError,
    ConceptChangeSetIdempotencyConflict,
    ConceptChangeSetService,
)
from forgecad_agent.application.concept_models import (
    ChangeSetConfirmResponse,
    ChangeSetPreviewResponse,
    ChangeSetTimelineResponse,
    ProposeChangeSetRequest,
)
from forgecad_agent.domain.concepts.models import DesignChangeSet


def build_change_set_router(service: ConceptChangeSetService) -> APIRouter:
    router = APIRouter(prefix="/api/v1", tags=["concept-change-sets"])

    @router.get(
        "/projects/{project_id}/change-sets",
        response_model=ChangeSetTimelineResponse,
    )
    def list_change_sets(
        project_id: str,
    ) -> Union[ChangeSetTimelineResponse, JSONResponse]:
        try:
            return service.list_for_project(project_id)
        except ConceptChangeSetError as exc:
            return _change_set_error_response(exc)

    @router.post(
        "/versions/{version_id}/change-sets",
        response_model=DesignChangeSet,
        status_code=201,
    )
    def propose_change_set(
        version_id: str,
        request: ProposeChangeSetRequest,
        idempotency_key: Annotated[
            Optional[str],
            Header(alias="Idempotency-Key"),
        ] = None,
    ) -> Union[DesignChangeSet, JSONResponse]:
        key = _require_idempotency_key(idempotency_key)
        try:
            return service.propose(version_id, request, key)
        except ConceptChangeSetIdempotencyConflict as exc:
            return _error_response(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ConceptChangeSetError as exc:
            return _change_set_error_response(exc)

    @router.post(
        "/change-sets/{change_set_id}:preview",
        response_model=ChangeSetPreviewResponse,
    )
    def preview_change_set(
        change_set_id: str,
        idempotency_key: Annotated[
            Optional[str],
            Header(alias="Idempotency-Key"),
        ] = None,
    ) -> Union[ChangeSetPreviewResponse, JSONResponse]:
        key = _require_idempotency_key(idempotency_key)
        try:
            return service.preview(change_set_id, key)
        except ConceptChangeSetIdempotencyConflict as exc:
            return _error_response(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ConceptChangeSetError as exc:
            return _change_set_error_response(exc)

    @router.post(
        "/change-sets/{change_set_id}:confirm",
        response_model=ChangeSetConfirmResponse,
    )
    def confirm_change_set(
        change_set_id: str,
        idempotency_key: Annotated[
            Optional[str],
            Header(alias="Idempotency-Key"),
        ] = None,
    ) -> Union[ChangeSetConfirmResponse, JSONResponse]:
        key = _require_idempotency_key(idempotency_key)
        try:
            return service.confirm(change_set_id, key)
        except ConceptChangeSetIdempotencyConflict as exc:
            return _error_response(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ConceptChangeSetError as exc:
            return _change_set_error_response(exc)

    return router


def _require_idempotency_key(value: Optional[str]) -> str:
    if not value:
        raise HTTPException(status_code=400, detail="Idempotency-Key is required.")
    return value


def _change_set_error_response(exc: ConceptChangeSetError) -> JSONResponse:
    if exc.code in {
        "PROJECT_NOT_FOUND",
        "VERSION_NOT_FOUND",
        "MODULE_GRAPH_NOT_FOUND",
        "CHANGE_SET_NOT_FOUND",
        "DOMAIN_PROFILE_NOT_FOUND",
    }:
        status_code = 404
    elif exc.code in {
        "CHANGE_SET_CONFLICT",
        "CHANGE_SET_STATE_CONFLICT",
        "CHANGE_SET_STALE",
    }:
        status_code = 409
    elif exc.code in {"INVALID_REQUEST", "CHANGE_SET_INVALID"}:
        status_code = 400
    else:
        status_code = 500
    return _error_response(status_code, exc.code, str(exc))


def _error_response(status_code: int, code: str, message: str) -> JSONResponse:
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
