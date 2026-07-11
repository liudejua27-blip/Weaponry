from __future__ import annotations

import base64
import binascii
import hashlib
import json
import uuid
from pathlib import Path
from typing import Any, Optional

from forgecad_agent.infrastructure.db import (
    AssetRepository,
    SQLiteConnectionFactory,
    SQLiteUnitOfWork,
)
from forgecad_agent.infrastructure.storage import ContentAddressedStore

from ..models import AssetUploadRequest, AssetUploadResponse, utc_now
from ..providers.image import ImageProviderError, image_dimensions
from ..spec_validation import WeaponSpecValidationError, validate_patch_manifest


class AssetUploadError(RuntimeError):
    def __init__(self, code: str, message: str, *, recoverable: bool = False) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable


class AssetUploadIdempotencyConflict(RuntimeError):
    pass


class LegacyAssetUploadService:
    """Validated legacy mask/manifest uploads behind the AssetStore facade."""

    def __init__(
        self,
        connection_factory: SQLiteConnectionFactory,
        object_store: ContentAddressedStore,
    ) -> None:
        self.connection_factory = connection_factory
        self.object_store = object_store

    def upload(
        self,
        weapon_id: str,
        version_id: str,
        request: AssetUploadRequest,
        idempotency_key: str,
    ) -> AssetUploadResponse:
        scope = f"POST /api/weapons/{weapon_id}/versions/{version_id}/assets"
        request_hash = _hash_json(request.model_dump())

        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            connection = unit_of_work.require_connection()
            existing = unit_of_work.idempotency.get(scope, idempotency_key)
            if existing is not None:
                if existing.request_hash != request_hash:
                    raise AssetUploadIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return AssetUploadResponse(**json.loads(existing.response_json))

            repository = AssetRepository(connection)
            if not repository.committed_version_exists(weapon_id, version_id):
                raise AssetUploadError(
                    "VERSION_NOT_FOUND",
                    "Target version was not found for this weapon.",
                )

            payload = _decode_base64_upload(request.data_base64)
            if len(payload) > 32 * 1024 * 1024:
                raise AssetUploadError(
                    "INVALID_REQUEST",
                    "Uploaded asset is too large for the M4 local API.",
                )

            filename = _safe_filename(request.filename)
            width: Optional[int] = None
            height: Optional[int] = None
            metadata: dict[str, Any] = dict(request.metadata)
            metadata["uploaded_via"] = "local_agent_api"

            if request.role == "patch_mask":
                if request.mime_type != "image/png":
                    raise AssetUploadError(
                        "INVALID_REQUEST",
                        "patch_mask upload must use image/png.",
                    )
                try:
                    width, height = image_dimensions(
                        payload,
                        mime_type=request.mime_type,
                        filename=filename,
                    )
                except ImageProviderError as exc:
                    raise AssetUploadError(
                        exc.code,
                        str(exc),
                        recoverable=exc.recoverable,
                    ) from exc
                metadata["mask_policy"] = "white_repaint_black_preserve"
                extension = ".png"
            elif request.role == "patch_manifest":
                if request.mime_type != "application/json":
                    raise AssetUploadError(
                        "INVALID_REQUEST",
                        "patch_manifest upload must use application/json.",
                    )
                try:
                    manifest = validate_patch_manifest(
                        json.loads(payload.decode("utf-8")),
                        provider_id="asset_upload",
                    )
                except (
                    UnicodeDecodeError,
                    json.JSONDecodeError,
                    WeaponSpecValidationError,
                ) as exc:
                    raise AssetUploadError(
                        "INVALID_REQUEST",
                        f"Patch manifest is invalid: {exc}",
                    ) from exc
                if manifest["weapon_id"] != weapon_id:
                    raise AssetUploadError(
                        "INVALID_REQUEST",
                        "Patch manifest weapon_id does not match upload weapon_id.",
                    )
                metadata["schema_version"] = "PatchManifest@1"
                extension = ".json"
            else:
                raise AssetUploadError(
                    "INVALID_REQUEST",
                    f"Unsupported upload role: {request.role}",
                )

            stored_object = self.object_store.put(payload, extension=extension)
            asset_id = _new_id("file")
            repository.add(
                file_id=asset_id,
                weapon_id=weapon_id,
                version_id=version_id,
                job_id=None,
                role=request.role,
                logical_path=(
                    f"weapons/{weapon_id}/versions/{version_id}/uploads/{filename}"
                ),
                object_path=stored_object.relative_path,
                sha256=stored_object.sha256,
                byte_size=stored_object.byte_size,
                mime_type=request.mime_type,
                ext=extension,
                width=width,
                height=height,
                metadata=metadata,
                created_at=utc_now(),
            )
            row = repository.get_active(asset_id)
            if row is None:
                raise AssetUploadError(
                    "ASSET_FILE_MISSING",
                    f"Asset file was not found after upload: {asset_id}",
                )
            response = AssetUploadResponse(
                weapon_id=weapon_id,
                version_id=version_id,
                asset_id=asset_id,
                role=request.role,
                logical_path=row["logical_path"],
                sha256=row["sha256"],
                byte_size=row["byte_size"],
                mime_type=row["mime_type"],
                width=row["width"],
                height=row["height"],
            )
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump()),
                created_at=utc_now(),
            )
            return response


def _decode_base64_upload(value: str) -> bytes:
    try:
        return base64.b64decode(value, validate=True)
    except binascii.Error as exc:
        raise AssetUploadError(
            "INVALID_REQUEST",
            "Uploaded asset data_base64 is not valid base64.",
        ) from exc


def _safe_filename(value: str) -> str:
    name = Path(value).name.strip()
    safe = "".join(
        char for char in name if char.isalnum() or char in {"-", "_", "."}
    ).strip(".")
    if not safe:
        raise AssetUploadError(
            "INVALID_REQUEST",
            "Uploaded asset filename is invalid.",
        )
    return safe[:120]


def _hash_json(value: Any) -> str:
    return hashlib.sha256(_canonical_json(value).encode("utf-8")).hexdigest()


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex[:12]}"
