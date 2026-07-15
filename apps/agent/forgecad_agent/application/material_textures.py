from __future__ import annotations

import base64
import binascii
import hashlib
import json
import struct
from typing import Optional

from forgecad_agent.application.agent_models import (
    AgentMaterialPreset,
    AgentMaterialTextureListResponse,
    AgentMaterialTextureObject,
    AgentMaterialTextureSummary,
    RegisterAgentMaterialTextureRequest,
)
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork
from forgecad_agent.infrastructure.storage.content_addressed_store import (
    ContentAddressedStore,
    ObjectStoreError,
)


MAX_TEXTURE_BYTES = 4_000_000
MAX_TEXTURE_DIMENSION = 4096
MAX_TEXTURE_PIXELS = 16_000_000


class MaterialTextureError(RuntimeError):
    def __init__(self, code: str, message: str, *, status_code: int = 400) -> None:
        super().__init__(message)
        self.code = code
        self.status_code = status_code


class MaterialTextureIdempotencyConflict(RuntimeError):
    pass


class MaterialTextureService:
    """Register and inspect visual-only texture objects without path access."""

    def __init__(
        self,
        connection_factory: SQLiteConnectionFactory,
        object_store: ContentAddressedStore,
    ) -> None:
        self.connection_factory = connection_factory
        self.object_store = object_store

    def register(
        self,
        request: RegisterAgentMaterialTextureRequest,
        idempotency_key: str,
    ) -> AgentMaterialTextureObject:
        payload = _decode_payload(request.payload_base64)
        width, height = _inspect_image(payload, request.mime_type)
        if len(payload) > MAX_TEXTURE_BYTES:
            raise MaterialTextureError("TEXTURE_TOO_LARGE", "纹理对象超过 4 MB 限制。")
        if width > MAX_TEXTURE_DIMENSION or height > MAX_TEXTURE_DIMENSION:
            raise MaterialTextureError("TEXTURE_DIMENSIONS_TOO_LARGE", "纹理尺寸超过 4096 像素限制。")
        if width * height > MAX_TEXTURE_PIXELS:
            raise MaterialTextureError("TEXTURE_PIXELS_TOO_MANY", "纹理像素数量超过限制。")

        request_hash = _hash_json(request.model_dump(mode="json"))
        scope = "POST /api/v1/agent/material-textures"
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            replay = unit.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise MaterialTextureIdempotencyConflict(
                        "Idempotency-Key was reused with a different texture request."
                    )
                return AgentMaterialTextureObject.model_validate_json(replay.response_json)

            stored = self.object_store.put(payload, extension=_extension_for_mime(request.mime_type))
            texture_asset_id = f"asset_tex_{stored.sha256[:24]}"
            existing = unit.material_textures.get_by_sha_role(stored.sha256, request.texture_role)
            if existing is not None:
                if not _metadata_matches(existing, request):
                    raise MaterialTextureError(
                        "TEXTURE_METADATA_CONFLICT",
                        "相同纹理内容已登记，但来源、许可证或用途不一致。",
                        status_code=409,
                    )
                result = self._to_model(existing)
            else:
                now = _utc_now()
                unit.material_textures.insert(
                    texture_asset_id=texture_asset_id,
                    texture_role=request.texture_role,
                    display_name=request.display_name,
                    mime_type=request.mime_type,
                    byte_size=len(payload),
                    sha256=stored.sha256,
                    object_path=stored.relative_path,
                    width=width,
                    height=height,
                    source=request.source,
                    license=request.license,
                    license_ref=request.license_ref,
                    thumbnail_asset_id=request.thumbnail_asset_id,
                    created_at=now,
                    updated_at=now,
                )
                row = unit.material_textures.get(texture_asset_id)
                if row is None:
                    raise MaterialTextureError("TEXTURE_NOT_PERSISTED", "纹理对象登记失败。", status_code=500)
                result = self._to_model(row)

            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=result.model_dump_json(),
                created_at=_utc_now(),
            )
            return result

    def get(self, texture_asset_id: str) -> AgentMaterialTextureObject:
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            row = unit.material_textures.get(texture_asset_id)
            if row is None:
                raise MaterialTextureError("TEXTURE_NOT_FOUND", "纹理对象不存在。", status_code=404)
            return self._to_model(row)

    def list(
        self,
        *,
        texture_role: Optional[str] = None,
        source: Optional[str] = None,
        query: Optional[str] = None,
        limit: int = 100,
    ) -> AgentMaterialTextureListResponse:
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            rows = unit.material_textures.list(
                texture_role=texture_role,
                source=source,
                query=query,
                limit=min(max(limit, 1), 100),
            )
            return AgentMaterialTextureListResponse(items=[self._to_model(row) for row in rows])

    def enrich_catalog(self, presets: list[AgentMaterialPreset]) -> list[AgentMaterialPreset]:
        """Add object existence/provenance summaries without exposing paths."""
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            enriched: list[AgentMaterialPreset] = []
            for preset in presets:
                texture_ids = [
                    ("base_color", preset.pbr.base_color_texture_asset_id),
                    ("metallic_roughness", preset.pbr.metallic_roughness_texture_asset_id),
                    ("normal", preset.pbr.normal_texture_asset_id),
                    ("occlusion", preset.pbr.occlusion_texture_asset_id),
                    ("emissive", preset.pbr.emissive_texture_asset_id),
                    ("thumbnail", preset.thumbnail_asset_id),
                ]
                summaries: list[AgentMaterialTextureSummary] = []
                fallback = "parameter"
                for role, texture_asset_id in texture_ids:
                    if not texture_asset_id:
                        continue
                    row = unit.material_textures.get(texture_asset_id)
                    if row is None:
                        summaries.append(
                            AgentMaterialTextureSummary(
                                texture_asset_id=texture_asset_id,
                                texture_role=role,
                                exists=False,
                            )
                        )
                        fallback = "unavailable" if role == "thumbnail" else fallback
                        continue
                    summaries.append(
                        AgentMaterialTextureSummary(
                            texture_asset_id=texture_asset_id,
                            texture_role=role,
                            exists=self._object_exists(row),
                            source=row["source"],
                            license=row["license"],
                            license_ref=row["license_ref"],
                        )
                    )
                    if role == "thumbnail":
                        fallback = "texture" if self._object_exists(row) else "unavailable"
                enriched.append(
                    preset.model_copy(
                        update={
                            "texture_summary": summaries,
                            "thumbnail_fallback": fallback,
                        }
                    )
                )
            return enriched

    def _to_model(self, row) -> AgentMaterialTextureObject:
        return AgentMaterialTextureObject(
            texture_asset_id=row["texture_asset_id"],
            texture_role=row["texture_role"],
            display_name=row["display_name"],
            mime_type=row["mime_type"],
            byte_size=int(row["byte_size"]),
            sha256=row["sha256"],
            object_path=row["object_path"],
            width=int(row["width"]),
            height=int(row["height"]),
            source=row["source"],
            license=row["license"],
            license_ref=row["license_ref"],
            thumbnail_asset_id=row["thumbnail_asset_id"],
            object_exists=self._object_exists(row),
            created_at=row["created_at"],
            updated_at=row["updated_at"],
        )

    def _object_exists(self, row) -> bool:
        try:
            self.object_store.read(row["object_path"], expected_sha256=row["sha256"])
        except ObjectStoreError:
            return False
        return True


def _decode_payload(value: str) -> bytes:
    try:
        payload = base64.b64decode(value, validate=True)
    except (ValueError, binascii.Error) as exc:
        raise MaterialTextureError("TEXTURE_BASE64_INVALID", "纹理内容不是有效的 base64。") from exc
    if not payload:
        raise MaterialTextureError("TEXTURE_EMPTY", "纹理内容不能为空。")
    return payload


def _inspect_image(payload: bytes, mime_type: str) -> tuple[int, int]:
    if mime_type == "image/png":
        if len(payload) < 24 or payload[:8] != b"\x89PNG\r\n\x1a\n" or payload[12:16] != b"IHDR":
            raise MaterialTextureError("TEXTURE_FORMAT_INVALID", "PNG 内容签名或头部无效。")
        width, height = struct.unpack(">II", payload[16:24])
        return _validate_dimensions(width, height)
    if mime_type == "image/jpeg":
        if len(payload) < 4 or payload[:2] != b"\xff\xd8":
            raise MaterialTextureError("TEXTURE_FORMAT_INVALID", "JPEG 内容签名无效。")
        return _jpeg_dimensions(payload)
    if mime_type == "image/webp":
        if len(payload) < 30 or payload[:4] != b"RIFF" or payload[8:12] != b"WEBP" or payload[12:16] != b"VP8X":
            raise MaterialTextureError("TEXTURE_FORMAT_INVALID", "仅支持带 VP8X 头的 WebP 纹理。")
        width = 1 + int.from_bytes(payload[24:27], "little")
        height = 1 + int.from_bytes(payload[27:30], "little")
        return _validate_dimensions(width, height)
    raise MaterialTextureError("TEXTURE_MIME_UNSUPPORTED", "不支持的纹理媒体类型。")


def _jpeg_dimensions(payload: bytes) -> tuple[int, int]:
    index = 2
    sof_markers = set(range(0xC0, 0xC4)) | set(range(0xC5, 0xC8)) | set(range(0xC9, 0xCC)) | set(range(0xCD, 0xD0))
    while index + 3 < len(payload):
        if payload[index] != 0xFF:
            index += 1
            continue
        while index < len(payload) and payload[index] == 0xFF:
            index += 1
        if index >= len(payload):
            break
        marker = payload[index]
        index += 1
        if marker in (0xD8, 0xD9):
            continue
        if index + 2 > len(payload):
            break
        segment_length = int.from_bytes(payload[index:index + 2], "big")
        if segment_length < 2 or index + segment_length > len(payload):
            break
        if marker in sof_markers and segment_length >= 7:
            height = int.from_bytes(payload[index + 3:index + 5], "big")
            width = int.from_bytes(payload[index + 5:index + 7], "big")
            return _validate_dimensions(width, height)
        index += segment_length
    raise MaterialTextureError("TEXTURE_DIMENSIONS_MISSING", "无法读取 JPEG 尺寸。")


def _validate_dimensions(width: int, height: int) -> tuple[int, int]:
    if width <= 0 or height <= 0:
        raise MaterialTextureError("TEXTURE_DIMENSIONS_INVALID", "纹理尺寸无效。")
    if width > MAX_TEXTURE_DIMENSION or height > MAX_TEXTURE_DIMENSION or width * height > MAX_TEXTURE_PIXELS:
        raise MaterialTextureError("TEXTURE_DIMENSIONS_TOO_LARGE", "纹理尺寸超过限制。")
    return width, height


def _extension_for_mime(mime_type: str) -> str:
    return {"image/png": ".png", "image/jpeg": ".jpg", "image/webp": ".webp"}[mime_type]


def _metadata_matches(row, request: RegisterAgentMaterialTextureRequest) -> bool:
    return all(
        row[field] == value
        for field, value in (
            ("texture_role", request.texture_role),
            ("mime_type", request.mime_type),
            ("source", request.source),
            ("license", request.license),
            ("license_ref", request.license_ref),
            ("thumbnail_asset_id", request.thumbnail_asset_id),
        )
    )


def _hash_json(value: object) -> str:
    return hashlib.sha256(
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()


def _utc_now() -> str:
    from datetime import datetime, timezone

    return datetime.now(timezone.utc).isoformat()
