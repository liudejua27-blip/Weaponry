from __future__ import annotations

import asyncio
import json
import os
from typing import Annotated, Optional, Union

from fastapi import FastAPI, Header, HTTPException, Request
from fastapi.exceptions import RequestValidationError
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import FileResponse
from fastapi.responses import JSONResponse
from fastapi.responses import StreamingResponse

from .asset_store import AssetStoreError, IdempotencyConflictError, SQLiteAssetStore
from .models import (
    AssetUploadRequest,
    AssetUploadResponse,
    AssetFileResponse,
    AssetRevealResponse,
    CreativeGraphResponse,
    CreativeInterpretationRequest,
    CreativeInterpretationResponse,
    CreativeRecastConfirmRequest,
    CreativeRecastConfirmResponse,
    CreateWeaponRequest,
    ExportUnityRequest,
    Generate3DRequest,
    HealthResponse,
    JobAcceptedResponse,
    JobActionListResponse,
    JobActionResponse,
    JobDetail,
    JobListResponse,
    JobRuntimeStateResponse,
    PatchWeaponRequest,
    ProviderSettingsListResponse,
    RuntimeRecoveryResponse,
    RuntimeWorkOnceResponse,
    WeaponDetail,
    WeaponListResponse,
)
from .providers.llm import LLMProviderError
from .worker import local_worker_enabled, run_local_worker_loop, stop_worker_task


DEFAULT_CORS_ORIGINS = [
    "http://127.0.0.1:5173",
    "http://127.0.0.1:5174",
    "http://localhost:5173",
    "http://localhost:5174",
]


def cors_origins_from_env() -> list[str]:
    extra = [
        origin.strip().rstrip("/")
        for origin in os.environ.get("WUSHEN_CORS_ORIGINS", "").split(",")
        if origin.strip()
    ]
    return list(dict.fromkeys([*DEFAULT_CORS_ORIGINS, *extra]))


def create_app() -> FastAPI:
    app = FastAPI(title="Wushen Forge Agent", version="0.1.0")
    app.add_middleware(
        CORSMiddleware,
        allow_origins=cors_origins_from_env(),
        allow_credentials=False,
        allow_methods=["GET", "POST", "OPTIONS"],
        allow_headers=["Content-Type", "Idempotency-Key", "Last-Event-ID", "X-Wushen-Client-Version"],
    )
    store = SQLiteAssetStore.from_env()

    @app.on_event("startup")
    async def recover_interrupted_jobs_on_startup() -> None:
        worker_enabled = local_worker_enabled()
        if os.environ.get("WUSHEN_RECOVER_ON_STARTUP", "1").strip() != "0":
            store.recover_interrupted_jobs(reason="startup", include_queued=not worker_enabled)
        if worker_enabled:
            app.state.local_worker_task = asyncio.create_task(
                run_local_worker_loop(
                    store,
                    runner_id=os.environ.get(
                        "WUSHEN_LOCAL_WORKER_ID",
                        os.environ.get("WUSHEN_GENERATE3D_WORKER_ID", "local_asset_worker"),
                    ),
                )
            )

    @app.on_event("shutdown")
    async def stop_generate3d_worker_on_shutdown() -> None:
        task = getattr(app.state, "local_worker_task", None)
        if task is not None:
            await stop_worker_task(task)

    @app.exception_handler(HTTPException)
    async def http_error_handler(_request: Request, exc: HTTPException) -> JSONResponse:
        detail = exc.detail if isinstance(exc.detail, str) else "Request failed."
        code = "INVALID_REQUEST"
        if exc.status_code == 404:
            code = "JOB_NOT_FOUND"
        elif exc.status_code == 409:
            code = "IDEMPOTENCY_CONFLICT"
        return error_response(exc.status_code, code, detail)

    @app.exception_handler(RequestValidationError)
    async def validation_error_handler(_request: Request, exc: RequestValidationError) -> JSONResponse:
        return error_response(
            422,
            "INVALID_REQUEST",
            "Request validation failed.",
            details={"errors": exc.errors()},
        )

    @app.get("/api/health", response_model=HealthResponse)
    def health() -> HealthResponse:
        return HealthResponse(status="ok", service="wushen-agent", mode="sqlite_mock")

    @app.get("/api/health/providers", response_model=ProviderSettingsListResponse)
    def provider_health() -> ProviderSettingsListResponse:
        return ProviderSettingsListResponse(providers=store.providers)

    @app.post("/api/weapons", response_model=JobAcceptedResponse, status_code=202)
    def create_weapon(
        _request: CreateWeaponRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> JobAcceptedResponse:
        if not idempotency_key:
            raise HTTPException(status_code=400, detail="Idempotency-Key is required.")
        try:
            job = store.create_weapon(_request, idempotency_key)
        except IdempotencyConflictError as exc:
            raise HTTPException(status_code=409, detail=str(exc)) from exc
        except LLMProviderError as exc:
            status_code = provider_error_status(exc.code)
            return error_response(status_code, exc.code, str(exc), recoverable=exc.recoverable)
        return JobAcceptedResponse(
            weapon_id=job.weapon_id,
            job_id=job.job_id,
            status=job.status,
            event_stream_url=f"/api/jobs/{job.job_id}/events",
        )

    @app.get("/api/weapons", response_model=WeaponListResponse)
    def list_weapons() -> WeaponListResponse:
        return WeaponListResponse(items=store.list_weapons(), next_cursor=None)

    @app.get("/api/weapons/{weapon_id}", response_model=WeaponDetail)
    def get_weapon(weapon_id: str) -> WeaponDetail:
        try:
            return store.get_weapon_detail(weapon_id)
        except KeyError as exc:
            raise HTTPException(status_code=404, detail="Weapon not found.") from exc

    @app.post("/api/weapons/{weapon_id}/interpretation", response_model=CreativeInterpretationResponse)
    def create_interpretation(
        weapon_id: str,
        _request: CreativeInterpretationRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[CreativeInterpretationResponse, JSONResponse]:
        if not idempotency_key:
            raise HTTPException(status_code=400, detail="Idempotency-Key is required.")
        try:
            return store.create_interpretation(weapon_id, _request, idempotency_key)
        except IdempotencyConflictError as exc:
            raise HTTPException(status_code=409, detail=str(exc)) from exc
        except AssetStoreError as exc:
            return error_response(asset_store_error_status(exc.code), exc.code, str(exc), recoverable=exc.recoverable)

    @app.post("/api/weapons/{weapon_id}/recast/confirm", response_model=CreativeRecastConfirmResponse)
    def confirm_recast(
        weapon_id: str,
        _request: CreativeRecastConfirmRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[CreativeRecastConfirmResponse, JSONResponse]:
        if not idempotency_key:
            raise HTTPException(status_code=400, detail="Idempotency-Key is required.")
        try:
            return store.confirm_recast(weapon_id, _request, idempotency_key)
        except IdempotencyConflictError as exc:
            raise HTTPException(status_code=409, detail=str(exc)) from exc
        except AssetStoreError as exc:
            return error_response(asset_store_error_status(exc.code), exc.code, str(exc), recoverable=exc.recoverable)

    @app.get("/api/weapons/{weapon_id}/creative-graph", response_model=CreativeGraphResponse)
    def get_creative_graph(weapon_id: str) -> Union[CreativeGraphResponse, JSONResponse]:
        try:
            return store.get_creative_graph(weapon_id)
        except AssetStoreError as exc:
            return error_response(asset_store_error_status(exc.code), exc.code, str(exc), recoverable=exc.recoverable)

    @app.get("/api/jobs", response_model=JobListResponse)
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
            return error_response(asset_store_error_status(exc.code), exc.code, str(exc), recoverable=exc.recoverable)

    @app.get("/api/jobs/{job_id}", response_model=JobDetail)
    def get_job(job_id: str) -> JobDetail:
        try:
            return store.get_job(job_id)
        except KeyError as exc:
            raise HTTPException(status_code=404, detail="Job not found.") from exc

    @app.get("/api/jobs/{job_id}/runtime", response_model=JobRuntimeStateResponse)
    def get_job_runtime(job_id: str) -> JobRuntimeStateResponse:
        try:
            return store.get_job_runtime_state(job_id)
        except KeyError as exc:
            raise HTTPException(status_code=404, detail="Job not found.") from exc

    @app.get("/api/jobs/{job_id}/actions", response_model=JobActionListResponse)
    def list_job_actions(job_id: str, cursor: Optional[str] = None, limit: int = 50) -> JobActionListResponse:
        try:
            return store.list_job_actions(job_id, cursor=cursor, limit=limit)
        except KeyError as exc:
            raise HTTPException(status_code=404, detail="Job not found.") from exc
        except AssetStoreError as exc:
            return error_response(asset_store_error_status(exc.code), exc.code, str(exc), recoverable=exc.recoverable)

    @app.get("/api/jobs/{job_id}/events")
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
                yield f"data: {json.dumps({'error': {'code': exc.code, 'message': str(exc)}}, ensure_ascii=False)}\n\n"

        return StreamingResponse(stream(), media_type="text/event-stream")

    @app.get("/api/assets/{asset_id}", response_model=AssetFileResponse)
    def get_asset(asset_id: str) -> AssetFileResponse:
        try:
            return store.get_asset_metadata(asset_id)
        except AssetStoreError as exc:
            return error_response(asset_store_error_status(exc.code), exc.code, str(exc), recoverable=exc.recoverable)

    @app.get("/api/assets/{asset_id}/file", response_model=None)
    def get_asset_file(asset_id: str):
        try:
            asset = store.resolve_asset_file(asset_id)
        except AssetStoreError as exc:
            return error_response(asset_store_error_status(exc.code), exc.code, str(exc), recoverable=exc.recoverable)
        return FileResponse(
            asset["path"],
            media_type=asset["mime_type"],
            filename=asset["filename"],
            headers={
                "Cache-Control": "private, max-age=3600",
                "X-Wushen-Asset-Id": asset_id,
                "X-Wushen-Asset-Sha256": asset["sha256"],
            },
        )

    @app.post("/api/assets/{asset_id}/reveal", response_model=AssetRevealResponse)
    def reveal_asset_file(asset_id: str, dry_run: bool = False) -> Union[AssetRevealResponse, JSONResponse]:
        try:
            return store.reveal_asset_file(asset_id, dry_run=dry_run)
        except AssetStoreError as exc:
            return error_response(asset_store_error_status(exc.code), exc.code, str(exc), recoverable=exc.recoverable)

    @app.post("/api/weapons/{weapon_id}/versions/{version_id}/assets", response_model=AssetUploadResponse, status_code=201)
    def upload_version_asset(
        weapon_id: str,
        version_id: str,
        _request: AssetUploadRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[AssetUploadResponse, JSONResponse]:
        if not idempotency_key:
            raise HTTPException(status_code=400, detail="Idempotency-Key is required.")
        if not store.has_weapon(weapon_id):
            raise HTTPException(status_code=404, detail="Weapon not found.")
        try:
            return store.upload_asset(weapon_id, version_id, _request, idempotency_key)
        except IdempotencyConflictError as exc:
            raise HTTPException(status_code=409, detail=str(exc)) from exc
        except AssetStoreError as exc:
            return error_response(asset_store_error_status(exc.code), exc.code, str(exc), recoverable=exc.recoverable)

    @app.post("/api/weapons/{weapon_id}/versions/{version_id}/activate", response_model=WeaponDetail)
    def activate_weapon_version(weapon_id: str, version_id: str) -> Union[WeaponDetail, JSONResponse]:
        if not store.has_weapon(weapon_id):
            raise HTTPException(status_code=404, detail="Weapon not found.")
        try:
            return store.activate_version(weapon_id, version_id)
        except AssetStoreError as exc:
            return error_response(asset_store_error_status(exc.code), exc.code, str(exc), recoverable=exc.recoverable)

    @app.post("/api/weapons/{weapon_id}/patch", response_model=JobAcceptedResponse, status_code=202)
    def patch_weapon(
        weapon_id: str,
        _request: PatchWeaponRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> JobAcceptedResponse:
        if not idempotency_key:
            raise HTTPException(status_code=400, detail="Idempotency-Key is required.")
        if not store.has_weapon(weapon_id):
            raise HTTPException(status_code=404, detail="Weapon not found.")
        try:
            job = store.patch_weapon(weapon_id, _request, idempotency_key)
        except IdempotencyConflictError as exc:
            raise HTTPException(status_code=409, detail=str(exc)) from exc
        except AssetStoreError as exc:
            return error_response(asset_store_error_status(exc.code), exc.code, str(exc), recoverable=exc.recoverable)
        return JobAcceptedResponse(
            weapon_id=job.weapon_id,
            job_id=job.job_id,
            status=job.status,
            event_stream_url=f"/api/jobs/{job.job_id}/events",
        )

    @app.post("/api/weapons/{weapon_id}/generate-3d", response_model=JobAcceptedResponse, status_code=202)
    def generate_3d(
        weapon_id: str,
        _request: Generate3DRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> JobAcceptedResponse:
        if not idempotency_key:
            raise HTTPException(status_code=400, detail="Idempotency-Key is required.")
        if not store.has_weapon(weapon_id):
            raise HTTPException(status_code=404, detail="Weapon not found.")
        try:
            job = store.generate_3d(weapon_id, _request, idempotency_key)
        except IdempotencyConflictError as exc:
            raise HTTPException(status_code=409, detail=str(exc)) from exc
        except AssetStoreError as exc:
            return error_response(asset_store_error_status(exc.code), exc.code, str(exc), recoverable=exc.recoverable)
        return JobAcceptedResponse(
            weapon_id=job.weapon_id,
            job_id=job.job_id,
            status=job.status,
            event_stream_url=f"/api/jobs/{job.job_id}/events",
        )

    @app.post("/api/weapons/{weapon_id}/export-unity", response_model=JobAcceptedResponse, status_code=202)
    def export_unity(
        weapon_id: str,
        _request: ExportUnityRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> JobAcceptedResponse:
        if not idempotency_key:
            raise HTTPException(status_code=400, detail="Idempotency-Key is required.")
        if not store.has_weapon(weapon_id):
            raise HTTPException(status_code=404, detail="Weapon not found.")
        try:
            job = store.export_unity(weapon_id, _request, idempotency_key)
        except IdempotencyConflictError as exc:
            raise HTTPException(status_code=409, detail=str(exc)) from exc
        except AssetStoreError as exc:
            return error_response(asset_store_error_status(exc.code), exc.code, str(exc), recoverable=exc.recoverable)
        return JobAcceptedResponse(
            weapon_id=job.weapon_id,
            job_id=job.job_id,
            status=job.status,
            event_stream_url=f"/api/jobs/{job.job_id}/events",
        )

    @app.post("/api/jobs/{job_id}/cancel", response_model=JobActionResponse)
    def cancel_job(job_id: str) -> JobActionResponse:
        try:
            return store.cancel_job(job_id)
        except KeyError as exc:
            raise HTTPException(status_code=404, detail="Job not found.") from exc
        except AssetStoreError as exc:
            return error_response(asset_store_error_status(exc.code), exc.code, str(exc), recoverable=exc.recoverable)

    @app.post("/api/jobs/{job_id}/retry", response_model=JobActionResponse)
    def retry_job(job_id: str) -> JobActionResponse:
        try:
            return store.retry_job(job_id)
        except KeyError as exc:
            raise HTTPException(status_code=404, detail="Job not found.") from exc
        except AssetStoreError as exc:
            return error_response(asset_store_error_status(exc.code), exc.code, str(exc), recoverable=exc.recoverable)

    @app.post("/api/jobs/{job_id}/retry-from/{step_name}", response_model=JobActionResponse)
    def retry_job_from_step(job_id: str, step_name: str) -> JobActionResponse:
        try:
            return store.retry_job_from_step(job_id, step_name)
        except KeyError as exc:
            raise HTTPException(status_code=404, detail="Job not found.") from exc
        except AssetStoreError as exc:
            return error_response(asset_store_error_status(exc.code), exc.code, str(exc), recoverable=exc.recoverable)

    @app.post("/api/runtime/recover", response_model=RuntimeRecoveryResponse)
    def recover_runtime() -> RuntimeRecoveryResponse:
        return store.recover_interrupted_jobs(reason="manual")

    @app.post("/api/runtime/work-once", response_model=RuntimeWorkOnceResponse)
    def runtime_work_once() -> RuntimeWorkOnceResponse:
        return store.run_worker_once()

    @app.get("/api/provider-settings", response_model=ProviderSettingsListResponse)
    def list_provider_settings() -> ProviderSettingsListResponse:
        return ProviderSettingsListResponse(providers=store.providers)

    return app


app = create_app()


def provider_error_status(code: str) -> int:
    if code == "PROVIDER_UNCONFIGURED":
        return 400
    if code == "PROVIDER_AUTH_FAILED":
        return 401
    if code == "RATE_LIMITED":
        return 429
    if code in {"INVALID_LLM_JSON", "PROVIDER_BAD_OUTPUT"}:
        return 502
    return 504


def asset_store_error_status(code: str) -> int:
    if code in {"WEAPON_NOT_FOUND", "VERSION_NOT_FOUND", "ASSET_FILE_MISSING", "INVALID_INTERPRETATION_ID"}:
        return 404
    if code in {"MASK_EMPTY", "MASK_SIZE_MISMATCH", "INVALID_REQUEST", "REVEAL_UNSUPPORTED", "INVALID_INTERPRETATION_CANDIDATE"}:
        return 400
    if code == "INVALID_EVENT_CURSOR":
        return 400
    if code in {"JOB_ACTION_CONFLICT", "INTERPRETATION_NOT_CONFIRMED"}:
        return 409
    if code == "PROVIDER_UNCONFIGURED":
        return 400
    if code in {"PROVIDER_BAD_OUTPUT", "GLB_INVALID", "QUALITY_CHECK_FAILED"}:
        return 502
    if code == "PROVIDER_TIMEOUT":
        return 504
    return 500


def error_response(
    status_code: int,
    code: str,
    message: str,
    details: Optional[dict[str, object]] = None,
    recoverable: Optional[bool] = None,
) -> JSONResponse:
    return JSONResponse(
        status_code=status_code,
        content={
            "error": {
                "code": code,
                "message": message,
                "recoverable": recoverable if recoverable is not None else status_code >= 500 or status_code in {409, 429},
                "details": details or {},
            }
        },
    )
