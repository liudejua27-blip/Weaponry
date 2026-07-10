from __future__ import annotations

from typing import Annotated, Optional, Union

from fastapi import APIRouter, Header, HTTPException
from fastapi.responses import JSONResponse

from forgecad_agent.application import (
    ConceptProjectError,
    ConceptProjectIdempotencyConflict,
    ConceptProjectService,
)
from forgecad_agent.application.concept_models import (
    AppendConceptVersionRequest,
    ConceptProjectDetail,
    ConceptProjectListResponse,
    ConceptVersionDetail,
    CreateConceptProjectRequest,
)


def build_concept_project_router(service: ConceptProjectService) -> APIRouter:
    router = APIRouter(prefix="/api/v1", tags=["concept-projects"])

    @router.post("/projects", response_model=ConceptProjectDetail, status_code=201)
    def create_project(
        request: CreateConceptProjectRequest,
        idempotency_key: Annotated[
            Optional[str],
            Header(alias="Idempotency-Key"),
        ] = None,
    ) -> Union[ConceptProjectDetail, JSONResponse]:
        key = _require_idempotency_key(idempotency_key)
        try:
            return service.create_project(request, key)
        except ConceptProjectIdempotencyConflict as exc:
            return _error_response(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ConceptProjectError as exc:
            return _concept_error_response(exc)

    @router.get("/projects", response_model=ConceptProjectListResponse)
    def list_projects() -> ConceptProjectListResponse:
        return service.list_projects()

    @router.get("/projects/{project_id}", response_model=ConceptProjectDetail)
    def get_project(
        project_id: str,
    ) -> Union[ConceptProjectDetail, JSONResponse]:
        try:
            return service.get_project(project_id)
        except ConceptProjectError as exc:
            return _concept_error_response(exc)

    @router.get("/versions/{version_id}", response_model=ConceptVersionDetail)
    def get_version(
        version_id: str,
    ) -> Union[ConceptVersionDetail, JSONResponse]:
        try:
            return service.get_version(version_id)
        except ConceptProjectError as exc:
            return _concept_error_response(exc)

    @router.post(
        "/projects/{project_id}/versions",
        response_model=ConceptProjectDetail,
        status_code=201,
    )
    def append_project_version(
        project_id: str,
        request: AppendConceptVersionRequest,
        idempotency_key: Annotated[
            Optional[str],
            Header(alias="Idempotency-Key"),
        ] = None,
    ) -> Union[ConceptProjectDetail, JSONResponse]:
        key = _require_idempotency_key(idempotency_key)
        try:
            return service.append_version(project_id, request, key)
        except ConceptProjectIdempotencyConflict as exc:
            return _error_response(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ConceptProjectError as exc:
            return _concept_error_response(exc)

    return router


def _require_idempotency_key(value: Optional[str]) -> str:
    if not value:
        raise HTTPException(status_code=400, detail="Idempotency-Key is required.")
    return value


def _concept_error_response(exc: ConceptProjectError) -> JSONResponse:
    if exc.code in {
        "PROJECT_NOT_FOUND",
        "VERSION_NOT_FOUND",
        "DOMAIN_PROFILE_NOT_FOUND",
        "MODULE_GRAPH_NOT_FOUND",
    }:
        status_code = 404
    elif exc.code == "INVALID_REQUEST":
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
