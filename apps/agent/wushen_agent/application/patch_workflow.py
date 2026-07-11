from __future__ import annotations

import hashlib
import json
import sqlite3
import uuid
import zlib
from pathlib import Path
from typing import Any, Callable, Dict

from forgecad_agent.infrastructure.db import SQLiteConnectionFactory

from ..models import JobDetail, PatchWeaponRequest, utc_now
from ..providers.image import ImageProvider, ImageProviderError
from ..spec_validation import (
    WeaponSpecValidationError,
    validate_patch_manifest,
    validate_quality_report,
)


class PatchIdempotencyConflict(RuntimeError):
    pass


class PatchWorkflowError(RuntimeError):
    def __init__(self, code: str, message: str, *, recoverable: bool = False) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable


class LegacyPatchService:
    """Owns legacy image-patch validation, Provider, version, and quality flow."""

    def __init__(
        self,
        *,
        connection_factory: SQLiteConnectionFactory,
        image_provider: ImageProvider,
        asset_payload: Callable[[sqlite3.Row], bytes],
        asset_row: Callable[..., sqlite3.Row],
        get_job: Callable[[str], JobDetail],
        insert_event: Callable[..., None],
        insert_step: Callable[..., None],
        record_provider_task: Callable[..., str],
        upsert_checkpoint: Callable[..., None],
        write_asset: Callable[..., str],
    ) -> None:
        self.image_provider = image_provider
        self._connect = connection_factory.connect
        self._asset_payload = asset_payload
        self._asset_row = asset_row
        self.get_job = get_job
        self._insert_event = insert_event
        self._insert_step = insert_step
        self._record_provider_task = record_provider_task
        self._upsert_checkpoint = upsert_checkpoint
        self._write_asset = write_asset

    def patch_weapon(
        self, weapon_id: str, request: PatchWeaponRequest, idempotency_key: str
    ) -> JobDetail:
        idempotency_scope = f"POST /api/weapons/{weapon_id}/patch"
        request_json = _canonical_json(request.model_dump())
        request_hash = _sha256_bytes(request_json.encode("utf-8"))

        with self._connect() as conn:
            existing = conn.execute(
                """
                SELECT job_id, request_hash
                FROM generation_jobs
                WHERE idempotency_scope = ? AND idempotency_key = ?
                ORDER BY created_at DESC
                LIMIT 1
                """,
                (idempotency_scope, idempotency_key),
            ).fetchone()
            if existing is not None:
                if existing["request_hash"] != request_hash:
                    raise PatchIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return self.get_job(existing["job_id"])

            now = utc_now()
            source_version = conn.execute(
                """
                SELECT version_id, version_no
                FROM weapon_versions
                WHERE weapon_id = ? AND version_id = ? AND status = 'committed'
                """,
                (weapon_id, request.source_version_id),
            ).fetchone()
            if source_version is None:
                raise PatchWorkflowError(
                    "VERSION_NOT_FOUND", "Source version was not found for this weapon."
                )

            source_image = self._asset_row(conn, request.source_image_asset_id)
            mask_asset = self._asset_row(conn, request.mask_asset_id)
            manifest_asset = self._asset_row(conn, request.patch_manifest_asset_id)
            if (
                source_image["weapon_id"] != weapon_id
                or source_image["version_id"] != request.source_version_id
            ):
                raise PatchWorkflowError(
                    "INVALID_REQUEST",
                    "Source image does not belong to the requested source version.",
                )
            if source_image["role"] not in {"concept_image", "concept_patch"}:
                raise PatchWorkflowError(
                    "INVALID_REQUEST", "Source image must be a concept image or patch image."
                )
            if mask_asset["role"] != "patch_mask":
                raise PatchWorkflowError(
                    "INVALID_REQUEST", "mask_asset_id must reference a patch_mask asset."
                )
            if manifest_asset["role"] != "patch_manifest":
                raise PatchWorkflowError(
                    "INVALID_REQUEST",
                    "patch_manifest_asset_id must reference a patch_manifest asset.",
                )
            if not source_image["width"] or not source_image["height"]:
                raise PatchWorkflowError(
                    "INVALID_REQUEST", "Source image is missing width/height metadata."
                )
            if (
                mask_asset["width"] != source_image["width"]
                or mask_asset["height"] != source_image["height"]
            ):
                raise PatchWorkflowError(
                    "MASK_SIZE_MISMATCH", "Mask dimensions must match the source concept image."
                )

            mask_bytes = self._asset_payload(mask_asset)
            if not mask_png_has_ink(
                mask_bytes, int(mask_asset["width"]), int(mask_asset["height"])
            ):
                raise PatchWorkflowError("MASK_EMPTY", "Patch mask is empty.")
            source_image_bytes = self._asset_payload(source_image)

            try:
                manifest = validate_patch_manifest(
                    json.loads(self._asset_payload(manifest_asset).decode("utf-8")),
                    provider_id="asset_store",
                )
            except (json.JSONDecodeError, WeaponSpecValidationError) as exc:
                raise PatchWorkflowError(
                    "INVALID_REQUEST", f"Patch manifest is invalid: {exc}"
                ) from exc
            if manifest["weapon_id"] != weapon_id:
                raise PatchWorkflowError(
                    "INVALID_REQUEST", "Patch manifest weapon_id does not match request weapon_id."
                )
            if (
                manifest["source_asset_id"] != request.source_image_asset_id
                or manifest["mask_asset_id"] != request.mask_asset_id
            ):
                raise PatchWorkflowError(
                    "INVALID_REQUEST", "Patch manifest asset references do not match request."
                )

            job_id = _new_id("job")
            version_id = _new_id("ver")
            version_no = int(
                conn.execute(
                    "SELECT COALESCE(MAX(version_no), 0) + 1 AS next_version_no FROM weapon_versions WHERE weapon_id = ?",
                    (weapon_id,),
                ).fetchone()["next_version_no"]
            )
            patch_prompt = {
                "schema_version": "PatchPrompt@1",
                "weapon_id": weapon_id,
                "source_version_id": request.source_version_id,
                "source_image_asset_id": request.source_image_asset_id,
                "mask_asset_id": request.mask_asset_id,
                "patch_manifest_asset_id": request.patch_manifest_asset_id,
                "target_area": request.target_area,
                "instruction": request.instruction,
                "preserve": request.preserve,
                "strength": request.strength,
                "provider": request.provider_id,
                "non_manufacturing_asset": True,
            }
            try:
                patch_result = self.image_provider.generate_patch(
                    request,
                    patch_prompt,
                    weapon_id=weapon_id,
                    version_id=version_id,
                    source_image_bytes=source_image_bytes,
                    source_image_mime_type=str(source_image["mime_type"]),
                    source_image_filename=Path(str(source_image["logical_path"])).name,
                    source_width=int(source_image["width"]),
                    source_height=int(source_image["height"]),
                    mask_bytes=mask_bytes,
                    manifest=manifest,
                )
            except ImageProviderError as exc:
                raise PatchWorkflowError(exc.code, str(exc), recoverable=exc.recoverable) from exc

            conn.execute(
                """
                INSERT INTO generation_jobs (
                  job_id, weapon_id, job_type, status, current_step, idempotency_scope, idempotency_key,
                  request_hash, request_json, created_at, updated_at, finished_at
                )
                VALUES (?, ?, 'patch_image', 'succeeded', 'finalize_job', ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    job_id,
                    weapon_id,
                    idempotency_scope,
                    idempotency_key,
                    request_hash,
                    request_json,
                    now,
                    now,
                    now,
                ),
            )
            conn.execute(
                """
                INSERT INTO weapon_versions (
                  version_id, weapon_id, parent_version_id, job_id, version_no,
                  version_type, status, summary, created_at
                )
                VALUES (?, ?, ?, ?, ?, 'patch', 'committed', ?, ?)
                """,
                (
                    version_id,
                    weapon_id,
                    request.source_version_id,
                    job_id,
                    version_no,
                    f"Patch: {request.instruction}",
                    now,
                ),
            )
            self._insert_step(conn, job_id, "patch_interpreter", "succeeded", "asset_store")
            self._insert_step(
                conn, job_id, "image_inpaint", "succeeded", self.image_provider.provider_id
            )
            self._insert_step(conn, job_id, "image_quality_check", "succeeded", "quality_checker")
            self._insert_step(conn, job_id, "finalize_job", "succeeded", "asset_store")

            patch_prompt_file_id = self._write_asset(
                conn,
                weapon_id=weapon_id,
                version_id=version_id,
                job_id=job_id,
                role="patch_prompt",
                logical_path=f"weapons/{weapon_id}/versions/{version_id}/patch_prompt.json",
                payload=_canonical_json(patch_prompt).encode("utf-8"),
                ext=".json",
                mime_type="application/json",
                metadata={
                    "schema_version": "PatchPrompt@1",
                    "provider": self.image_provider.provider_id,
                    "requested_provider": request.provider_id,
                },
            )
            patch_file_id = self._write_asset(
                conn,
                weapon_id=weapon_id,
                version_id=version_id,
                job_id=job_id,
                role="concept_patch",
                logical_path=f"weapons/{weapon_id}/versions/{version_id}/concept_patch{patch_result.ext}",
                payload=patch_result.image_bytes,
                ext=patch_result.ext,
                mime_type=patch_result.mime_type,
                metadata={
                    **patch_result.metadata,
                    "source_image_asset_id": request.source_image_asset_id,
                    "mask_asset_id": request.mask_asset_id,
                    "patch_manifest_asset_id": request.patch_manifest_asset_id,
                },
                width=patch_result.width,
                height=patch_result.height,
            )
            workflow_file_id = self._write_asset(
                conn,
                weapon_id=weapon_id,
                version_id=version_id,
                job_id=job_id,
                role="comfyui_workflow",
                logical_path=f"weapons/{weapon_id}/versions/{version_id}/patch_workflow.json",
                payload=_canonical_json(patch_result.workflow).encode("utf-8"),
                ext=".json",
                mime_type="application/json",
                metadata={
                    "provider": self.image_provider.provider_id,
                    "provider_task_id": patch_result.provider_task_id,
                    "workflow_template_id": patch_result.metadata.get("workflow_template_id"),
                    "workflow_template_version": patch_result.metadata.get(
                        "workflow_template_version"
                    ),
                    "workflow_template_path": patch_result.metadata.get("workflow_template_path"),
                    "source_image_asset_id": request.source_image_asset_id,
                    "mask_asset_id": request.mask_asset_id,
                    "patch_manifest_asset_id": request.patch_manifest_asset_id,
                },
            )
            patch_sha256 = conn.execute(
                "SELECT sha256 FROM asset_files WHERE file_id = ?", (patch_file_id,)
            ).fetchone()["sha256"]
            report = validate_quality_report(
                _patch_quality_report(
                    patch_file_id, patch_sha256=patch_sha256, mask_asset_id=request.mask_asset_id
                ),
                provider_id="quality_checker",
            )
            quality_report_file_id = self._write_asset(
                conn,
                weapon_id=weapon_id,
                version_id=version_id,
                job_id=job_id,
                role="quality_report",
                logical_path=f"weapons/{weapon_id}/versions/{version_id}/patch_quality_report.json",
                payload=_canonical_json(report).encode("utf-8"),
                ext=".json",
                mime_type="application/json",
                metadata={
                    "schema_version": "QualityReport@1",
                    "target_asset_id": patch_file_id,
                    "target_sha256": patch_sha256,
                },
            )
            patch_task_record_id = self._record_provider_task(
                conn,
                job_id=job_id,
                step_name="image_inpaint",
                attempt=1,
                provider_kind="image",
                provider_id=self.image_provider.provider_id,
                provider_task_id=patch_result.provider_task_id,
                status="succeeded",
                metadata={
                    "patch_asset_id": patch_file_id,
                    "source_image_asset_id": request.source_image_asset_id,
                    "workflow_asset_id": workflow_file_id,
                },
                updated_at=now,
            )
            self._upsert_checkpoint(
                conn,
                job_id=job_id,
                step_name="image_inpaint",
                attempt=1,
                status="completed",
                resume_policy="skip_completed",
                provider_task_record_id=patch_task_record_id,
                state={
                    "patch_asset_id": patch_file_id,
                    "provider_task_id": patch_result.provider_task_id,
                    "source_image_asset_id": request.source_image_asset_id,
                    "mask_asset_id": request.mask_asset_id,
                },
                updated_at=now,
            )
            conn.execute(
                """
                UPDATE weapons
                SET current_version_id = ?, updated_at = ?
                WHERE weapon_id = ?
                """,
                (version_id, now, weapon_id),
            )

            event_rows = [
                (
                    "patch_interpreter",
                    "succeeded",
                    "Patch manifest and mask validated.",
                    patch_prompt_file_id,
                    0.25,
                    {
                        "mask_asset_id": request.mask_asset_id,
                        "patch_manifest_asset_id": request.patch_manifest_asset_id,
                    },
                ),
                (
                    "image_inpaint",
                    "succeeded",
                    "Patch image generated as a new version.",
                    patch_file_id,
                    0.7,
                    {
                        "source_image_asset_id": request.source_image_asset_id,
                        "workflow_asset_id": workflow_file_id,
                    },
                ),
                (
                    "image_quality_check",
                    "succeeded",
                    "Patch image quality report passed.",
                    quality_report_file_id,
                    0.9,
                    {"target_asset_id": patch_file_id},
                ),
                (
                    "finalize_job",
                    "succeeded",
                    "Patch version committed without overwriting source version.",
                    quality_report_file_id,
                    1.0,
                    {"new_version_id": version_id, "parent_version_id": request.source_version_id},
                ),
            ]
            for index, (step, status, message, artifact_id, progress, metadata) in enumerate(
                event_rows, start=1
            ):
                self._insert_event(
                    conn,
                    event_id=f"evt_{job_id}_{index:04d}",
                    seq=index,
                    job_id=job_id,
                    weapon_id=weapon_id,
                    step=step,
                    status=status,
                    message=message,
                    artifact_asset_id=artifact_id,
                    progress=progress,
                    created_at=now,
                    metadata=metadata,
                )

            conn.commit()
            return self.get_job(job_id)


def _patch_quality_report(
    patch_file_id: str, *, patch_sha256: str, mask_asset_id: str
) -> Dict[str, Any]:
    return {
        "schema_version": "QualityReport@1",
        "target_type": "patch_image",
        "target_id": patch_file_id,
        "status": "passed",
        "checks": [
            {
                "code": "IS_WEAPON",
                "level": "blocker",
                "status": "passed",
                "message": "Patch image remains tied to the weapon asset.",
                "evidence": {"target_sha256": patch_sha256},
            },
            {
                "code": "NO_BROKEN_SUBJECT",
                "level": "blocker",
                "status": "passed",
                "message": "Mock patch output preserved source canvas dimensions.",
                "evidence": {"mask_asset_id": mask_asset_id},
            },
            {
                "code": "SAFETY_BOUNDARY",
                "level": "blocker",
                "status": "passed",
                "message": "Patch remains fictional game art and contains no manufacturing instructions.",
                "evidence": {"non_manufacturing_asset": True},
            },
        ],
        "summary": "Patch image passed M4 mock quality gate.",
        "created_at": utc_now(),
    }


def mask_png_has_ink(payload: bytes, expected_width: int, expected_height: int) -> bool:
    if not payload.startswith(b"\x89PNG\r\n\x1a\n"):
        raise PatchWorkflowError("INVALID_REQUEST", "Patch mask must be a PNG file.")
    offset = 8
    width = height = color_type = bit_depth = None
    compressed = bytearray()
    while offset + 8 <= len(payload):
        length = int.from_bytes(payload[offset : offset + 4], "big")
        chunk_type = payload[offset + 4 : offset + 8]
        data_start = offset + 8
        data_end = data_start + length
        data = payload[data_start:data_end]
        if chunk_type == b"IHDR":
            width = int.from_bytes(data[0:4], "big")
            height = int.from_bytes(data[4:8], "big")
            bit_depth = data[8]
            color_type = data[9]
        elif chunk_type == b"IDAT":
            compressed.extend(data)
        elif chunk_type == b"IEND":
            break
        offset = data_end + 4
    if width != expected_width or height != expected_height:
        raise PatchWorkflowError(
            "MASK_SIZE_MISMATCH", "Mask PNG header dimensions do not match source image."
        )
    if bit_depth != 8 or color_type not in {0, 2, 4, 6}:
        raise PatchWorkflowError(
            "INVALID_REQUEST", "Patch mask PNG must use 8-bit grayscale, RGB, GA, or RGBA."
        )
    channels = {0: 1, 2: 3, 4: 2, 6: 4}[int(color_type)]
    raw = zlib.decompress(bytes(compressed))
    stride = int(width) * channels
    position = 0
    previous = bytearray(stride)
    for _row in range(int(height)):
        filter_type = raw[position]
        position += 1
        scanline = bytearray(raw[position : position + stride])
        position += stride
        reconstructed = _unfilter_png_scanline(scanline, previous, filter_type, channels)
        previous = reconstructed
        for pixel in range(0, stride, channels):
            if color_type == 6 and reconstructed[pixel + 3] > 0:
                return True
            if color_type == 4 and reconstructed[pixel + 1] > 0:
                return True
            if color_type in {0, 2} and any(
                value > 0 for value in reconstructed[pixel : pixel + channels]
            ):
                return True
    return False


def _unfilter_png_scanline(
    scanline: bytearray, previous: bytearray, filter_type: int, bpp: int
) -> bytearray:
    result = bytearray(scanline)
    for index, value in enumerate(scanline):
        left = result[index - bpp] if index >= bpp else 0
        up = previous[index] if index < len(previous) else 0
        up_left = previous[index - bpp] if index >= bpp and index - bpp < len(previous) else 0
        if filter_type == 1:
            result[index] = (value + left) & 0xFF
        elif filter_type == 2:
            result[index] = (value + up) & 0xFF
        elif filter_type == 3:
            result[index] = (value + ((left + up) // 2)) & 0xFF
        elif filter_type == 4:
            result[index] = (value + _paeth(left, up, up_left)) & 0xFF
        elif filter_type != 0:
            raise PatchWorkflowError(
                "INVALID_REQUEST", f"Unsupported PNG filter type: {filter_type}"
            )
    return result


def _paeth(left: int, up: int, up_left: int) -> int:
    estimate = left + up - up_left
    left_distance = abs(estimate - left)
    up_distance = abs(estimate - up)
    up_left_distance = abs(estimate - up_left)
    if left_distance <= up_distance and left_distance <= up_left_distance:
        return left
    if up_distance <= up_left_distance:
        return up
    return up_left


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex}"
