from __future__ import annotations

import asyncio
import json
from typing import Annotated, Optional

from fastapi import APIRouter, Header, HTTPException
from fastapi.responses import StreamingResponse

from ..asset_store import AssetStoreError, SQLiteAssetStore
from ..models import (
    JobActionListResponse,
    JobActionResponse,
    JobDetail,
    JobListResponse,
    JobRuntimeStateResponse,
)
from .errors import asset_store_error_status, error_response


def build_job_router(store: SQLiteAssetStore) -> APIRouter:
    router = APIRouter()

    @router.get("/api/jobs", response_model=JobListResponse)
    def list_jobs(
        query: Optional[str] = None,
        status: Optional[str] = None,
        job_type: Optional[str] = None,
        error_code: Optional[str] = None,
        cursor: Optional[str] = None,
        limit: int = 25,
    ) -> JobListResponse:
        try:
            return store.list_jobs(
                query=query,
                status=status,
                job_type=job_type,
                error_code=error_code,
                cursor=cursor,
                limit=limit,
            )
        except AssetStoreError as exc:
            return error_response(
                asset_store_error_status(exc.code),
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            )

    @router.get("/api/jobs/{job_id}", response_model=JobDetail)
    def get_job(job_id: str) -> JobDetail:
        try:
            return store.get_job(job_id)
        except KeyError as exc:
            raise HTTPException(status_code=404, detail="Job not found.") from exc

    @router.get("/api/jobs/{job_id}/runtime", response_model=JobRuntimeStateResponse)
    def get_job_runtime(job_id: str) -> JobRuntimeStateResponse:
        try:
            return store.get_job_runtime_state(job_id)
        except KeyError as exc:
            raise HTTPException(status_code=404, detail="Job not found.") from exc

    @router.get("/api/jobs/{job_id}/actions", response_model=JobActionListResponse)
    def list_job_actions(
        job_id: str,
        cursor: Optional[str] = None,
        limit: int = 50,
    ) -> JobActionListResponse:
        try:
            return store.list_job_actions(job_id, cursor=cursor, limit=limit)
        except KeyError as exc:
            raise HTTPException(status_code=404, detail="Job not found.") from exc
        except AssetStoreError as exc:
            return error_response(
                asset_store_error_status(exc.code),
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            )

    @router.get("/api/jobs/{job_id}/events")
    async def job_events(
        job_id: str,
        after: Optional[str] = None,
        last_event_id: Optional[str] = None,
        last_event_id_header: Annotated[Optional[str], Header(alias="Last-Event-ID")] = None,
    ) -> StreamingResponse:
        if not store.has_job(job_id):
            raise HTTPException(status_code=404, detail="Job not found.")
        resume_after = after or last_event_id or last_event_id_header

        async def stream():
            try:
                for event in store.iter_events(job_id, after=resume_after):
                    yield f"id: {event.id}\n"
                    yield "event: job.event\n"
                    yield f"data: {json.dumps(event.model_dump(), ensure_ascii=False)}\n\n"
                    await asyncio.sleep(0.05)
            except AssetStoreError as exc:
                yield "event: job.error\n"
                payload = {"error": {"code": exc.code, "message": str(exc)}}
                yield f"data: {json.dumps(payload, ensure_ascii=False)}\n\n"

        return StreamingResponse(stream(), media_type="text/event-stream")

    @router.post("/api/jobs/{job_id}/cancel", response_model=JobActionResponse)
    def cancel_job(job_id: str) -> JobActionResponse:
        try:
            return store.cancel_job(job_id)
        except KeyError as exc:
            raise HTTPException(status_code=404, detail="Job not found.") from exc
        except AssetStoreError as exc:
            return error_response(
                asset_store_error_status(exc.code),
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            )

    @router.post("/api/jobs/{job_id}/retry", response_model=JobActionResponse)
    def retry_job(job_id: str) -> JobActionResponse:
        try:
            return store.retry_job(job_id)
        except KeyError as exc:
            raise HTTPException(status_code=404, detail="Job not found.") from exc
        except AssetStoreError as exc:
            return error_response(
                asset_store_error_status(exc.code),
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            )

    @router.post("/api/jobs/{job_id}/retry-from/{step_name}", response_model=JobActionResponse)
    def retry_job_from_step(job_id: str, step_name: str) -> JobActionResponse:
        try:
            return store.retry_job_from_step(job_id, step_name)
        except KeyError as exc:
            raise HTTPException(status_code=404, detail="Job not found.") from exc
        except AssetStoreError as exc:
            return error_response(
                asset_store_error_status(exc.code),
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            )

    return router
