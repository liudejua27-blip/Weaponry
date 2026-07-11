from __future__ import annotations

import asyncio
import json
from typing import Annotated, Any, Optional, Union

from fastapi import APIRouter, Header
from fastapi.responses import JSONResponse, StreamingResponse

from forgecad_agent.application.concept_jobs import ConceptJobError, ConceptJobService
from forgecad_agent.application.concept_models import (
    ConceptJobEventListResponse,
    ConceptJobRecord,
)


def build_concept_job_router(service: ConceptJobService, quality_service: Any | None = None) -> APIRouter:
    router = APIRouter(prefix="/api/v1", tags=["concept-jobs"])

    @router.get("/jobs/{job_id}", response_model=ConceptJobRecord)
    def get_job(job_id: str) -> Union[ConceptJobRecord, JSONResponse]:
        try:
            return service.get_job(job_id)
        except ConceptJobError as exc:
            return _job_error_response(exc)

    @router.post("/jobs/{job_id}:cancel", response_model=ConceptJobRecord)
    def cancel_job(job_id: str) -> Union[ConceptJobRecord, JSONResponse]:
        try:
            return service.cancel_queued_job(job_id)
        except ConceptJobError as exc:
            return _job_error_response(exc)

    @router.post("/jobs/{job_id}:retry", response_model=ConceptJobRecord)
    def retry_job(job_id: str) -> Union[ConceptJobRecord, JSONResponse]:
        try:
            return service.retry_job(job_id)
        except ConceptJobError as exc:
            return _job_error_response(exc)

    @router.post("/concept-jobs/work-once", response_model=Optional[ConceptJobRecord])
    def work_once() -> Union[ConceptJobRecord, None, JSONResponse]:
        if quality_service is None:
            return _job_error_response(ConceptJobError("CONCEPT_WORKER_UNAVAILABLE", "Concept quality worker is unavailable."))
        try:
            return service.run_next_quality_inspection(quality_service, runner_id="manual_api_worker")
        except ConceptJobError as exc:
            return _job_error_response(exc)

    @router.get(
        "/jobs/{job_id}/events.json",
        response_model=ConceptJobEventListResponse,
    )
    def list_job_events(
        job_id: str,
        after: Optional[str] = None,
    ) -> Union[ConceptJobEventListResponse, JSONResponse]:
        try:
            return service.list_events(job_id, after=after)
        except ConceptJobError as exc:
            return _job_error_response(exc)

    @router.get("/jobs/{job_id}/events", response_model=None)
    async def stream_job_events(
        job_id: str,
        after: Optional[str] = None,
        last_event_id: Annotated[
            Optional[str],
            Header(alias="Last-Event-ID"),
        ] = None,
    ) -> Union[StreamingResponse, JSONResponse]:
        try:
            events = service.list_events(job_id, after=after or last_event_id)
        except ConceptJobError as exc:
            return _job_error_response(exc)

        async def stream():
            for event in events.items:
                yield f"id: {event.event_id}\n"
                yield "event: concept.job.event\n"
                yield f"data: {json.dumps(event.model_dump(mode='json'), ensure_ascii=False)}\n\n"
                await asyncio.sleep(0)

        return StreamingResponse(stream(), media_type="text/event-stream")

    return router


def _job_error_response(exc: ConceptJobError) -> JSONResponse:
    if exc.code in {"CONCEPT_JOB_NOT_FOUND", "VERSION_NOT_FOUND", "MODULE_GRAPH_NOT_FOUND"}:
        status_code = 404
    elif exc.code == "INVALID_EVENT_CURSOR":
        status_code = 400
    elif exc.code in {"JOB_ACTION_CONFLICT", "IDEMPOTENCY_CONFLICT"}:
        status_code = 409
    else:
        status_code = 500
    return JSONResponse(
        status_code=status_code,
        content={
            "error": {
                "code": exc.code,
                "message": str(exc),
                "recoverable": status_code >= 500,
                "details": {},
            }
        },
    )
