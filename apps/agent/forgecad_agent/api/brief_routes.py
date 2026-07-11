from __future__ import annotations

from typing import Annotated, Optional, Union

from fastapi import APIRouter, Header, HTTPException, Query
from fastapi.responses import JSONResponse

from forgecad_agent.application.concept_briefs import (
    ConceptBriefError,
    ConceptBriefIdempotencyConflict,
    ConceptBriefService,
)
from forgecad_agent.application.concept_models import (
    DesignBriefRecord,
    DesignVariantListResponse,
    DesignVariantRecord,
    GenerateDesignVariantsRequest,
    InterpretDesignBriefRequest,
    SelectDesignVariantRequest,
)


def build_brief_router(service: ConceptBriefService) -> APIRouter:
    router = APIRouter(prefix="/api/v1", tags=["concept-briefs"])

    @router.post(
        "/projects/{project_id}/brief:interpret",
        response_model=DesignBriefRecord,
        status_code=201,
    )
    def interpret_brief(
        project_id: str,
        request: InterpretDesignBriefRequest,
        idempotency_key: Annotated[
            Optional[str],
            Header(alias="Idempotency-Key"),
        ] = None,
    ) -> Union[DesignBriefRecord, JSONResponse]:
        key = _require_idempotency_key(idempotency_key)
        try:
            return service.interpret(project_id, request, key)
        except ConceptBriefIdempotencyConflict as exc:
            return _error_response(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ConceptBriefError as exc:
            return _brief_error_response(exc)

    @router.get(
        "/projects/{project_id}/briefs/{brief_id}",
        response_model=DesignBriefRecord,
    )
    def get_brief(
        project_id: str,
        brief_id: str,
    ) -> Union[DesignBriefRecord, JSONResponse]:
        try:
            return service.get_brief(project_id, brief_id)
        except ConceptBriefError as exc:
            return _brief_error_response(exc)

    @router.post(
        "/projects/{project_id}/variants",
        response_model=DesignVariantListResponse,
        status_code=201,
    )
    def generate_variants(
        project_id: str,
        request: GenerateDesignVariantsRequest,
        idempotency_key: Annotated[
            Optional[str],
            Header(alias="Idempotency-Key"),
        ] = None,
    ) -> Union[DesignVariantListResponse, JSONResponse]:
        key = _require_idempotency_key(idempotency_key)
        try:
            return service.generate_variants(project_id, request, key)
        except ConceptBriefIdempotencyConflict as exc:
            return _error_response(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ConceptBriefError as exc:
            return _brief_error_response(exc)

    @router.get(
        "/projects/{project_id}/variants",
        response_model=DesignVariantListResponse,
    )
    def list_variants(
        project_id: str,
        brief_id: Optional[str] = Query(default=None),
    ) -> Union[DesignVariantListResponse, JSONResponse]:
        try:
            return service.list_variants(project_id, brief_id=brief_id)
        except ConceptBriefError as exc:
            return _brief_error_response(exc)

    @router.post(
        "/projects/{project_id}/variants/{variant_id}:select",
        response_model=DesignVariantRecord,
    )
    def select_variant(
        project_id: str,
        variant_id: str,
        request: SelectDesignVariantRequest,
        idempotency_key: Annotated[
            Optional[str],
            Header(alias="Idempotency-Key"),
        ] = None,
    ) -> Union[DesignVariantRecord, JSONResponse]:
        key = _require_idempotency_key(idempotency_key)
        try:
            return service.select_variant(project_id, variant_id, request, key)
        except ConceptBriefIdempotencyConflict as exc:
            return _error_response(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ConceptBriefError as exc:
            return _brief_error_response(exc)

    return router


def _require_idempotency_key(value: Optional[str]) -> str:
    if not value:
        raise HTTPException(status_code=400, detail="Idempotency-Key is required.")
    return value


def _brief_error_response(exc: ConceptBriefError) -> JSONResponse:
    if exc.code in {
        "PROJECT_NOT_FOUND",
        "VERSION_NOT_FOUND",
        "MODULE_GRAPH_NOT_FOUND",
        "BRIEF_NOT_FOUND",
        "VARIANT_NOT_FOUND",
        "DOMAIN_PROFILE_NOT_FOUND",
        "ASSET_NOT_FOUND",
    }:
        status_code = 404
    elif exc.code == "IDEMPOTENCY_CONFLICT":
        status_code = 409
    elif exc.code in {"INVALID_REQUEST", "VARIANT_GENERATION_FAILED"}:
        status_code = 400
    elif exc.code in {
        "PLANNER_BAD_OUTPUT",
        "PLANNER_UNCONFIGURED",
        "PLANNER_AUTH_FAILED",
        "PLANNER_RATE_LIMITED",
        "PLANNER_HTTP_ERROR",
        "PLANNER_TIMEOUT",
        "PLANNER_NO_EDITABLE_NODE",
    }:
        status_code = 502
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
                "recoverable": (
                    (status_code >= 500 or status_code == 409)
                    and code != "PLANNER_AUTH_FAILED"
                ),
                "details": {},
            }
        },
    )
