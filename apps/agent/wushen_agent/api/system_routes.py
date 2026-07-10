from __future__ import annotations

from fastapi import APIRouter

from ..asset_store import SQLiteAssetStore
from ..models import (
    HealthResponse,
    ProviderSettingsListResponse,
    RuntimeRecoveryResponse,
    RuntimeWorkOnceResponse,
)


def build_system_router(store: SQLiteAssetStore) -> APIRouter:
    router = APIRouter()

    @router.get("/api/health", response_model=HealthResponse)
    def health() -> HealthResponse:
        return HealthResponse(status="ok", service="wushen-agent", mode="sqlite_mock")

    @router.get("/api/health/providers", response_model=ProviderSettingsListResponse)
    def provider_health() -> ProviderSettingsListResponse:
        return ProviderSettingsListResponse(providers=store.providers)

    @router.post("/api/runtime/recover", response_model=RuntimeRecoveryResponse)
    def recover_runtime() -> RuntimeRecoveryResponse:
        return store.recover_interrupted_jobs(reason="manual")

    @router.post("/api/runtime/work-once", response_model=RuntimeWorkOnceResponse)
    def runtime_work_once() -> RuntimeWorkOnceResponse:
        return store.run_worker_once()

    @router.get("/api/provider-settings", response_model=ProviderSettingsListResponse)
    def list_provider_settings() -> ProviderSettingsListResponse:
        return ProviderSettingsListResponse(providers=store.providers)

    return router
