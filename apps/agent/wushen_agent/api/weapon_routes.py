from __future__ import annotations

from typing import Annotated, Optional, Union

from fastapi import APIRouter, Header, HTTPException
from fastapi.responses import JSONResponse

from ..asset_store import AssetStoreError, IdempotencyConflictError, SQLiteAssetStore
from ..models import (
    CreativeGraphResponse,
    CreativeInterpretationRequest,
    CreativeInterpretationResponse,
    CreativeRecastConfirmRequest,
    CreativeRecastConfirmResponse,
    CreateWeaponRequest,
    ExportUnityRequest,
    Generate3DRequest,
    JobAcceptedResponse,
    JobDetail,
    PatchWeaponRequest,
    WeaponDetail,
    WeaponListResponse,
)
from ..providers.llm import LLMProviderError
from .errors import asset_store_error_status, error_response, provider_error_status


def build_weapon_router(store: SQLiteAssetStore) -> APIRouter:
    router = APIRouter()

    @router.post("/api/weapons", response_model=JobAcceptedResponse, status_code=202)
    def create_weapon(
        request: CreateWeaponRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> JobAcceptedResponse:
        key = require_idempotency_key(idempotency_key)
        try:
            job = store.create_weapon(request, key)
        except IdempotencyConflictError as exc:
            raise HTTPException(status_code=409, detail=str(exc)) from exc
        except LLMProviderError as exc:
            return error_response(
                provider_error_status(exc.code),
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            )
        return accepted_job(job)

    @router.get("/api/weapons", response_model=WeaponListResponse)
    def list_weapons() -> WeaponListResponse:
        return WeaponListResponse(items=store.list_weapons(), next_cursor=None)

    @router.get("/api/weapons/{weapon_id}", response_model=WeaponDetail)
    def get_weapon(weapon_id: str) -> WeaponDetail:
        try:
            return store.get_weapon_detail(weapon_id)
        except KeyError as exc:
            raise HTTPException(status_code=404, detail="Weapon not found.") from exc

    @router.post(
        "/api/weapons/{weapon_id}/interpretation",
        response_model=CreativeInterpretationResponse,
    )
    def create_interpretation(
        weapon_id: str,
        request: CreativeInterpretationRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[CreativeInterpretationResponse, JSONResponse]:
        key = require_idempotency_key(idempotency_key)
        try:
            return store.create_interpretation(weapon_id, request, key)
        except IdempotencyConflictError as exc:
            raise HTTPException(status_code=409, detail=str(exc)) from exc
        except AssetStoreError as exc:
            return store_error_response(exc)

    @router.post(
        "/api/weapons/{weapon_id}/recast/confirm",
        response_model=CreativeRecastConfirmResponse,
    )
    def confirm_recast(
        weapon_id: str,
        request: CreativeRecastConfirmRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[CreativeRecastConfirmResponse, JSONResponse]:
        key = require_idempotency_key(idempotency_key)
        try:
            return store.confirm_recast(weapon_id, request, key)
        except IdempotencyConflictError as exc:
            raise HTTPException(status_code=409, detail=str(exc)) from exc
        except AssetStoreError as exc:
            return store_error_response(exc)

    @router.get(
        "/api/weapons/{weapon_id}/creative-graph",
        response_model=CreativeGraphResponse,
    )
    def get_creative_graph(
        weapon_id: str,
    ) -> Union[CreativeGraphResponse, JSONResponse]:
        try:
            return store.get_creative_graph(weapon_id)
        except AssetStoreError as exc:
            return store_error_response(exc)

    @router.post(
        "/api/weapons/{weapon_id}/versions/{version_id}/activate",
        response_model=WeaponDetail,
    )
    def activate_weapon_version(
        weapon_id: str,
        version_id: str,
    ) -> Union[WeaponDetail, JSONResponse]:
        require_weapon(store, weapon_id)
        try:
            return store.activate_version(weapon_id, version_id)
        except AssetStoreError as exc:
            return store_error_response(exc)

    @router.post(
        "/api/weapons/{weapon_id}/patch",
        response_model=JobAcceptedResponse,
        status_code=202,
    )
    def patch_weapon(
        weapon_id: str,
        request: PatchWeaponRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> JobAcceptedResponse:
        require_weapon(store, weapon_id)
        key = require_idempotency_key(idempotency_key)
        try:
            return accepted_job(store.patch_weapon(weapon_id, request, key))
        except IdempotencyConflictError as exc:
            raise HTTPException(status_code=409, detail=str(exc)) from exc
        except AssetStoreError as exc:
            return store_error_response(exc)

    @router.post(
        "/api/weapons/{weapon_id}/generate-3d",
        response_model=JobAcceptedResponse,
        status_code=202,
    )
    def generate_3d(
        weapon_id: str,
        request: Generate3DRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> JobAcceptedResponse:
        require_weapon(store, weapon_id)
        key = require_idempotency_key(idempotency_key)
        try:
            return accepted_job(store.generate_3d(weapon_id, request, key))
        except IdempotencyConflictError as exc:
            raise HTTPException(status_code=409, detail=str(exc)) from exc
        except AssetStoreError as exc:
            return store_error_response(exc)

    @router.post(
        "/api/weapons/{weapon_id}/export-unity",
        response_model=JobAcceptedResponse,
        status_code=202,
    )
    def export_unity(
        weapon_id: str,
        request: ExportUnityRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> JobAcceptedResponse:
        require_weapon(store, weapon_id)
        key = require_idempotency_key(idempotency_key)
        try:
            return accepted_job(store.export_unity(weapon_id, request, key))
        except IdempotencyConflictError as exc:
            raise HTTPException(status_code=409, detail=str(exc)) from exc
        except AssetStoreError as exc:
            return store_error_response(exc)

    return router


def require_idempotency_key(value: Optional[str]) -> str:
    if not value:
        raise HTTPException(status_code=400, detail="Idempotency-Key is required.")
    return value


def require_weapon(store: SQLiteAssetStore, weapon_id: str) -> None:
    if not store.has_weapon(weapon_id):
        raise HTTPException(status_code=404, detail="Weapon not found.")


def accepted_job(job: JobDetail) -> JobAcceptedResponse:
    return JobAcceptedResponse(
        weapon_id=job.weapon_id,
        job_id=job.job_id,
        status=job.status,
        event_stream_url=f"/api/jobs/{job.job_id}/events",
    )


def store_error_response(exc: AssetStoreError) -> JSONResponse:
    return error_response(
        asset_store_error_status(exc.code),
        exc.code,
        str(exc),
        recoverable=exc.recoverable,
    )
