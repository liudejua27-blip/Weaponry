from __future__ import annotations

import hashlib
import json
import os
import sqlite3
import uuid
from typing import Any, Callable, Dict

from forgecad_agent.infrastructure.db import SQLiteConnectionFactory

from ..models import Generate3DRequest, JobDetail, utc_now
from ..providers.three_d import ThreeDProvider, ThreeDProviderError


class Generate3DIdempotencyConflict(RuntimeError):
    pass


class Generate3DError(RuntimeError):
    def __init__(self, code: str, message: str, *, recoverable: bool = False) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable


class LegacyGenerate3DService:
    """Owns legacy sync/queue generate-3D entry orchestration."""

    def __init__(
        self,
        *,
        connection_factory: SQLiteConnectionFactory,
        three_d_provider: ThreeDProvider,
        asset_row: Callable[..., sqlite3.Row],
        asset_payload: Callable[[sqlite3.Row], bytes],
        write_asset: Callable[..., str],
        write_rough_model_assets: Callable[..., Dict[str, str]],
        record_provider_task: Callable[..., str],
        upsert_checkpoint: Callable[..., None],
        insert_step: Callable[..., None],
        insert_event: Callable[..., None],
        get_job: Callable[[str], JobDetail],
    ) -> None:
        self.three_d_provider = three_d_provider
        self._connect = connection_factory.connect
        self._asset_row = asset_row
        self._asset_payload = asset_payload
        self._write_asset = write_asset
        self._write_rough_model_assets = write_rough_model_assets
        self._record_provider_task = record_provider_task
        self._upsert_checkpoint = upsert_checkpoint
        self._insert_step = insert_step
        self._insert_event = insert_event
        self.get_job = get_job

    def _validate_source(
        self,
        conn: sqlite3.Connection,
        weapon_id: str,
        request: Generate3DRequest,
    ) -> sqlite3.Row:
        source_version = conn.execute(
            """
            SELECT version_id, version_no
            FROM weapon_versions
            WHERE weapon_id = ? AND version_id = ? AND status = 'committed'
            """,
            (weapon_id, request.source_version_id),
        ).fetchone()
        if source_version is None:
            raise Generate3DError(
                "VERSION_NOT_FOUND",
                "Source version was not found for this weapon.",
            )
        source_image = self._asset_row(conn, request.source_image_asset_id)
        if (
            source_image["weapon_id"] != weapon_id
            or source_image["version_id"] != request.source_version_id
        ):
            raise Generate3DError(
                "INVALID_REQUEST",
                "Source image does not belong to the requested source version.",
            )
        if source_image["role"] not in {"concept_image", "concept_patch"}:
            raise Generate3DError(
                "INVALID_REQUEST",
                "Source image must be a concept image or patch image.",
            )
        return source_image

    def generate_3d(
        self, weapon_id: str, request: Generate3DRequest, idempotency_key: str
    ) -> JobDetail:
        if (
            os.environ.get("WUSHEN_GENERATE_3D_ASYNC", "0").strip() == "1"
            or os.environ.get("WUSHEN_GENERATE3D_ASYNC", "0").strip() == "1"
            or os.environ.get("WUSHEN_GENERATE3D_WORKER", "0").strip() == "1"
            or os.environ.get("WUSHEN_GENERATE3D_RUNTIME", "sync").strip().lower()
            in {"worker", "async"}
        ):
            return self.enqueue_generate_3d(weapon_id, request, idempotency_key)

        idempotency_scope = f"POST /api/weapons/{weapon_id}/generate-3d"
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
                    raise Generate3DIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return self.get_job(existing["job_id"])

            source_version = conn.execute(
                """
                SELECT version_id, version_no
                FROM weapon_versions
                WHERE weapon_id = ? AND version_id = ? AND status = 'committed'
                """,
                (weapon_id, request.source_version_id),
            ).fetchone()
            if source_version is None:
                raise Generate3DError(
                    "VERSION_NOT_FOUND", "Source version was not found for this weapon."
                )

            source_image = self._asset_row(conn, request.source_image_asset_id)
            if (
                source_image["weapon_id"] != weapon_id
                or source_image["version_id"] != request.source_version_id
            ):
                raise Generate3DError(
                    "INVALID_REQUEST",
                    "Source image does not belong to the requested source version.",
                )
            if source_image["role"] not in {"concept_image", "concept_patch"}:
                raise Generate3DError(
                    "INVALID_REQUEST", "Source image must be a concept image or patch image."
                )

            now = utc_now()
            job_id = _new_id("job")
            version_id = _new_id("ver")
            model_id = _new_id("model")
            version_no = int(
                conn.execute(
                    "SELECT COALESCE(MAX(version_no), 0) + 1 AS next_version_no FROM weapon_versions WHERE weapon_id = ?",
                    (weapon_id,),
                ).fetchone()["next_version_no"]
            )
            source_image_bytes = self._asset_payload(source_image)
            model_input = {
                "schema_version": "ModelGenerationInput@1",
                "weapon_id": weapon_id,
                "source_version_id": request.source_version_id,
                "source_image_asset_id": request.source_image_asset_id,
                "source_image": source_image["logical_path"],
                "provider": request.provider_id,
                "target_format": request.target_format,
                "style": request.style,
                "orientation_policy": request.orientation_policy,
                "scale_policy": request.scale_policy,
                "build_unity_export": request.build_unity_export,
                "non_manufacturing_asset": True,
                "created_at": now,
            }

            conn.execute(
                """
                INSERT INTO generation_jobs (
                  job_id, weapon_id, job_type, status, current_step, idempotency_scope, idempotency_key,
                  request_hash, request_json, created_at, updated_at, finished_at
                )
                VALUES (?, ?, 'generate_3d', 'succeeded', 'finalize_job', ?, ?, ?, ?, ?, ?, ?)
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
                VALUES (?, ?, ?, ?, ?, 'rough_3d', 'committed', ?, ?)
                """,
                (
                    version_id,
                    weapon_id,
                    request.source_version_id,
                    job_id,
                    version_no,
                    "Rough 3D generated from selected concept image.",
                    now,
                ),
            )
            self._insert_step(conn, job_id, "rough3d_plan", "succeeded", "asset_store")
            self._insert_step(
                conn, job_id, "rough3d_submit", "succeeded", self.three_d_provider.provider_id
            )
            self._insert_step(conn, job_id, "model_qc_optimize", "succeeded", "quality_checker")
            self._insert_step(conn, job_id, "asset_commit_model", "succeeded", "asset_store")
            self._insert_step(conn, job_id, "finalize_job", "succeeded", "asset_store")

            model_input_file_id = self._write_asset(
                conn,
                weapon_id=weapon_id,
                version_id=version_id,
                job_id=job_id,
                role="other",
                logical_path=f"weapons/{weapon_id}/models/{model_id}/model_generation_input.json",
                payload=_canonical_json(model_input).encode("utf-8"),
                ext=".json",
                mime_type="application/json",
                metadata={
                    "schema_version": "ModelGenerationInput@1",
                    "provider": request.provider_id,
                },
            )
            try:
                model_assets = self._write_rough_model_assets(
                    conn,
                    weapon_id=weapon_id,
                    version_id=version_id,
                    job_id=job_id,
                    model_id=model_id,
                    source_image_file_id=request.source_image_asset_id,
                    source_image_bytes=source_image_bytes,
                    source_image_mime_type=source_image["mime_type"],
                    source_image_logical_path=source_image["logical_path"],
                    target_format=request.target_format,
                    style=request.style,
                    orientation_policy=request.orientation_policy,
                    scale_policy=request.scale_policy,
                    gated_by_quality_report_file_id=None,
                )
            except ThreeDProviderError as exc:
                raise Generate3DError(exc.code, str(exc), recoverable=exc.recoverable) from exc

            model_task_record_id = self._record_provider_task(
                conn,
                job_id=job_id,
                step_name="rough3d_submit",
                attempt=1,
                provider_kind="three_d",
                provider_id=self.three_d_provider.provider_id,
                provider_task_id=model_assets.get("provider_task_id") or None,
                status="succeeded",
                metadata={
                    "model_id": model_id,
                    "source_image_asset_id": request.source_image_asset_id,
                    "raw_model_file_id": model_assets["raw_model_file_id"],
                },
                updated_at=now,
            )
            self._upsert_checkpoint(
                conn,
                job_id=job_id,
                step_name="rough3d_submit",
                attempt=1,
                status="completed",
                resume_policy="skip_completed",
                provider_task_record_id=model_task_record_id,
                state={
                    "model_id": model_id,
                    "provider_task_id": model_assets.get("provider_task_id") or None,
                    "source_image_asset_id": request.source_image_asset_id,
                    "raw_model_file_id": model_assets["raw_model_file_id"],
                    "optimized_model_file_id": model_assets["optimized_model_file_id"],
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
                    "rough3d_plan",
                    "succeeded",
                    "3D generation input captured.",
                    model_input_file_id,
                    0.2,
                    {"source_image_asset_id": request.source_image_asset_id},
                ),
                (
                    "rough3d_submit",
                    "succeeded",
                    "Rough 3D provider produced GLB variants.",
                    model_assets["raw_model_file_id"],
                    0.55,
                    {"model_id": model_id, "provider": self.three_d_provider.provider_id},
                ),
                (
                    "model_qc_optimize",
                    "succeeded",
                    "Model quality report passed and optimized GLB was stored.",
                    model_assets["model_quality_report_file_id"],
                    0.82,
                    {"optimized_model_file_id": model_assets["optimized_model_file_id"]},
                ),
                (
                    "asset_commit_model",
                    "succeeded",
                    "3D model assets committed without overwriting the source version.",
                    model_assets["optimized_model_file_id"],
                    0.95,
                    {"new_version_id": version_id},
                ),
                (
                    "finalize_job",
                    "succeeded",
                    "Generate-3D job completed.",
                    model_assets["model_quality_report_file_id"],
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

    def enqueue_generate_3d(
        self, weapon_id: str, request: Generate3DRequest, idempotency_key: str
    ) -> JobDetail:
        idempotency_scope = f"POST /api/weapons/{weapon_id}/generate-3d"
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
                    raise Generate3DIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return self.get_job(existing["job_id"])

            self._validate_source(conn, weapon_id, request)
            now = utc_now()
            job_id = _new_id("job")
            conn.execute(
                """
                INSERT INTO generation_jobs (
                  job_id, weapon_id, job_type, status, current_step, idempotency_scope,
                  idempotency_key, request_hash, request_json, checkpoint_json,
                  created_at, updated_at, finished_at
                )
                VALUES (?, ?, 'generate_3d', 'queued', 'rough3d_plan', ?, ?, ?, ?, ?, ?, ?, NULL)
                """,
                (
                    job_id,
                    weapon_id,
                    idempotency_scope,
                    idempotency_key,
                    request_hash,
                    request_json,
                    _canonical_json(
                        {
                            "runtime": "worker",
                            "source_version_id": request.source_version_id,
                            "source_image_asset_id": request.source_image_asset_id,
                            "next_step": "rough3d_plan",
                        }
                    ),
                    now,
                    now,
                ),
            )
            for step_name, provider in [
                ("rough3d_plan", "asset_store"),
                ("rough3d_submit", self.three_d_provider.provider_id),
                ("model_qc_optimize", "quality_checker"),
                ("asset_commit_model", "asset_store"),
                ("finalize_job", "asset_store"),
            ]:
                self._insert_step(conn, job_id, step_name, "queued", provider)
            self._insert_event(
                conn,
                event_id=f"evt_{job_id}_0001",
                seq=1,
                job_id=job_id,
                weapon_id=weapon_id,
                step="rough3d_plan",
                status="queued",
                message="Generate-3D job queued for local worker.",
                artifact_asset_id=None,
                progress=0.02,
                created_at=now,
                metadata={
                    "runtime": "worker",
                    "source_version_id": request.source_version_id,
                    "source_image_asset_id": request.source_image_asset_id,
                },
            )
            conn.commit()
            return self.get_job(job_id)


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex}"
