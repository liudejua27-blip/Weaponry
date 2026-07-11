from __future__ import annotations

from typing import Annotated, Optional, Union

from fastapi import APIRouter, Header, HTTPException
from fastapi.responses import JSONResponse

from forgecad_agent.application.concept_models import (
    CreateQualityRunRequest,
    InspectConceptVersionRequest,
    QualityRunRecord,
)
from forgecad_agent.application.concept_quality import (
    ConceptQualityError,
    ConceptQualityIdempotencyConflict,
    ConceptQualityService,
)
from forgecad_agent.application.concept_jobs import ConceptJobError, ConceptJobService
from forgecad_agent.application.concept_models import ConceptJobRecord


def build_quality_router(service: ConceptQualityService, jobs: ConceptJobService | None = None) -> APIRouter:
    router = APIRouter(prefix="/api/v1", tags=["concept-quality"])

    @router.post(
        "/versions/{version_id}/quality-runs",
        response_model=QualityRunRecord,
        status_code=201,
    )
    def create_quality_run(
        version_id: str,
        request: CreateQualityRunRequest,
        idempotency_key: Annotated[
            Optional[str],
            Header(alias="Idempotency-Key"),
        ] = None,
    ) -> Union[QualityRunRecord, JSONResponse]:
        key = _require_idempotency_key(idempotency_key)
        try:
            return service.create_run(version_id, request, key)
        except ConceptQualityIdempotencyConflict as exc:
            return _error_response(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ConceptQualityError as exc:
            return _quality_error_response(exc)

    @router.post(
        "/versions/{version_id}/quality-runs:inspect",
        response_model=QualityRunRecord,
        status_code=201,
    )
    def inspect_concept_version(
        version_id: str,
        request: InspectConceptVersionRequest,
        idempotency_key: Annotated[
            Optional[str],
            Header(alias="Idempotency-Key"),
        ] = None,
    ) -> Union[QualityRunRecord, JSONResponse]:
        key = _require_idempotency_key(idempotency_key)
        try:
            return service.inspect_version(version_id, request, key)
        except ConceptQualityIdempotencyConflict as exc:
            return _error_response(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ConceptQualityError as exc:
            return _quality_error_response(exc)

    @router.post(
        "/versions/{version_id}/quality-runs:inspect:enqueue",
        response_model=ConceptJobRecord,
        status_code=202,
    )
    def enqueue_concept_inspection(
        version_id: str,
        request: InspectConceptVersionRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[ConceptJobRecord, JSONResponse]:
        if jobs is None:
            return _error_response(503, "CONCEPT_WORKER_UNAVAILABLE", "Concept quality worker is unavailable.")
        try:
            return jobs.enqueue_quality_inspection(version_id, request, _require_idempotency_key(idempotency_key))
        except ConceptJobError as exc:
            return _error_response(409 if exc.code == "IDEMPOTENCY_CONFLICT" else 404 if exc.code in {"VERSION_NOT_FOUND", "MODULE_GRAPH_NOT_FOUND"} else 500, exc.code, str(exc))

    @router.get("/quality-runs/{quality_run_id}", response_model=QualityRunRecord)
    def get_quality_run(
        quality_run_id: str,
    ) -> Union[QualityRunRecord, JSONResponse]:
        try:
            return service.get_run(quality_run_id)
        except ConceptQualityError as exc:
            return _quality_error_response(exc)

    return router


def _require_idempotency_key(value: Optional[str]) -> str:
    if not value:
        raise HTTPException(status_code=400, detail="Idempotency-Key is required.")
    return value


def _quality_error_response(exc: ConceptQualityError) -> JSONResponse:
    if exc.code in {"VERSION_NOT_FOUND", "MODULE_GRAPH_NOT_FOUND", "QUALITY_RUN_NOT_FOUND"}:
        status_code = 404
    elif exc.code == "QUALITY_RUN_CONFLICT":
        status_code = 409
    elif exc.code == "INVALID_REQUEST":
        status_code = 400
    elif exc.code in {"MODULE_ASSET_UNAVAILABLE", "QUALITY_INSPECTOR_UNAVAILABLE"}:
        status_code = 503
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
