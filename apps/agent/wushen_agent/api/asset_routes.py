from __future__ import annotations

from typing import Annotated, Optional, Union

from fastapi import APIRouter, Header, HTTPException
from fastapi.responses import FileResponse, JSONResponse

from ..asset_store import AssetStoreError, IdempotencyConflictError, SQLiteAssetStore
from ..models import AssetFileResponse, AssetRevealResponse, AssetUploadRequest, AssetUploadResponse
from .errors import asset_store_error_status, error_response


def build_asset_router(store: SQLiteAssetStore) -> APIRouter:
    router = APIRouter()

    @router.get("/api/assets/{asset_id}", response_model=AssetFileResponse)
    def get_asset(asset_id: str) -> AssetFileResponse:
        try:
            return store.get_asset_metadata(asset_id)
        except AssetStoreError as exc:
            return error_response(
                asset_store_error_status(exc.code),
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            )

    @router.get("/api/assets/{asset_id}/file", response_model=None)
    def get_asset_file(asset_id: str):
        try:
            asset = store.resolve_asset_file(asset_id)
        except AssetStoreError as exc:
            return error_response(
                asset_store_error_status(exc.code),
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            )
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

    @router.post("/api/assets/{asset_id}/reveal", response_model=AssetRevealResponse)
    def reveal_asset_file(
        asset_id: str,
        dry_run: bool = False,
    ) -> Union[AssetRevealResponse, JSONResponse]:
        try:
            return store.reveal_asset_file(asset_id, dry_run=dry_run)
        except AssetStoreError as exc:
            return error_response(
                asset_store_error_status(exc.code),
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            )

    @router.post(
        "/api/weapons/{weapon_id}/versions/{version_id}/assets",
        response_model=AssetUploadResponse,
        status_code=201,
    )
    def upload_version_asset(
        weapon_id: str,
        version_id: str,
        request: AssetUploadRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[AssetUploadResponse, JSONResponse]:
        if not idempotency_key:
            raise HTTPException(status_code=400, detail="Idempotency-Key is required.")
        if not store.has_weapon(weapon_id):
            raise HTTPException(status_code=404, detail="Weapon not found.")
        try:
            return store.upload_asset(weapon_id, version_id, request, idempotency_key)
        except IdempotencyConflictError as exc:
            raise HTTPException(status_code=409, detail=str(exc)) from exc
        except AssetStoreError as exc:
            return error_response(
                asset_store_error_status(exc.code),
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            )

    return router
