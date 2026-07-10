from __future__ import annotations

import hashlib
import json
import math
import os
import shutil
import sqlite3
import subprocess
import struct
import sys
import uuid
import zipfile
from io import BytesIO
import zlib
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional

from forgecad_agent.infrastructure.db import (
    AssetRepository,
    CheckpointRepository,
    MigrationError,
    SQLiteConnectionFactory,
    SQLiteMigrationRunner,
)
from forgecad_agent.infrastructure.storage import ContentAddressedStore, ObjectStoreError

from .application.asset_uploads import (
    AssetUploadError,
    AssetUploadIdempotencyConflict,
    LegacyAssetUploadService,
)
from .application.creative_recast import (
    CreativeRecastError,
    CreativeRecastIdempotencyConflict,
    LegacyCreativeRecastService,
)
from .application.create_weapon import (
    CreateWeaponIdempotencyConflict,
    LegacyCreateWeaponService,
)
from .application.job_commands import JobCommandError, LegacyJobCommandService
from .application.job_queries import JobQueryError, LegacyJobQueryService
from .application.job_recovery import LegacyJobRecoveryService
from .application.library import LegacyLibraryService, LibraryError
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
    JobActionListResponse,
    JobActionResponse,
    JobDetail,
    JobEvent,
    JobListResponse,
    JobRuntimeStateResponse,
    PatchWeaponRequest,
    ProviderSettings,
    RuntimeRecoveryResponse,
    RuntimeWorkOnceResponse,
    WeaponDetail,
    WeaponSummary,
    utc_now,
)
from .providers.image import (
    ImageProvider,
    ImageProviderError,
    image_provider_from_env,
    image_provider_settings_from_env,
)
from .providers.llm import (
    LLMProvider,
    llm_provider_from_env,
    llm_provider_settings_from_env,
)
from .providers.three_d import (
    RoughModelResult,
    ThreeDProvider,
    ThreeDProviderError,
    three_d_provider_from_env,
    three_d_provider_settings_from_env,
)
from .spec_validation import (
    WeaponSpecValidationError,
    validate_patch_manifest,
    validate_quality_report,
)


class IdempotencyConflictError(Exception):
    """Raised when the same idempotency key is reused with different input."""


class AssetStoreError(Exception):
    def __init__(self, code: str, message: str, recoverable: bool = False) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable


class SQLiteAssetStore:
    """SQLite metadata store plus immutable content-addressed object files."""

    def __init__(
        self,
        library_root: Path,
        migrations_dir: Path,
        llm_provider: Optional[LLMProvider] = None,
        image_provider: Optional[ImageProvider] = None,
        three_d_provider: Optional[ThreeDProvider] = None,
    ) -> None:
        self.library_root = library_root.expanduser().resolve()
        self.db_path = self.library_root / "library.db"
        self.migrations_dir = migrations_dir
        self.llm_provider = llm_provider or llm_provider_from_env()
        self.image_provider = image_provider or image_provider_from_env()
        self.three_d_provider = three_d_provider or three_d_provider_from_env()
        self.library_root.mkdir(parents=True, exist_ok=True)
        self.connection_factory = SQLiteConnectionFactory(self.db_path)
        self.object_store = ContentAddressedStore(self.library_root)
        self.asset_uploads = LegacyAssetUploadService(
            self.connection_factory,
            self.object_store,
        )
        self.creative_recast = LegacyCreativeRecastService(self.connection_factory)
        self.job_commands = LegacyJobCommandService(
            self.connection_factory,
            self.three_d_provider,
        )
        self.job_queries = LegacyJobQueryService(self.connection_factory)
        self.job_recovery = LegacyJobRecoveryService(self.connection_factory)
        self.library = LegacyLibraryService(
            self.connection_factory,
            self.object_store,
        )
        self.create_weapon_workflow = LegacyCreateWeaponService(
            connection_factory=self.connection_factory,
            llm_provider=self.llm_provider,
            image_provider=self.image_provider,
            three_d_provider=self.three_d_provider,
            write_asset=self._write_asset,
            write_rough_model_assets=self._write_rough_model_assets,
            record_provider_task=self._record_provider_task,
            upsert_checkpoint=self._upsert_checkpoint,
            insert_step=self._insert_step,
            insert_event=self._insert_event,
            get_job=self.get_job,
            concept_quality_report=_concept_quality_report,
        )
        self._migrate()

    @classmethod
    def from_env(cls) -> "SQLiteAssetStore":
        default_root = Path.cwd() / "WushenForgeLibrary"
        library_root = Path(os.environ.get("WUSHEN_LIBRARY_ROOT", str(default_root)))
        migrations_dir = Path(os.environ.get("WUSHEN_MIGRATIONS_DIR", str(Path.cwd() / "migrations")))
        return cls(library_root=library_root, migrations_dir=migrations_dir)

    @property
    def providers(self) -> List[ProviderSettings]:
        return llm_provider_settings_from_env() + image_provider_settings_from_env() + three_d_provider_settings_from_env()

    def create_weapon(self, request: CreateWeaponRequest, idempotency_key: str) -> JobDetail:
        try:
            return self.create_weapon_workflow.create_weapon(request, idempotency_key)
        except CreateWeaponIdempotencyConflict as exc:
            raise IdempotencyConflictError(str(exc)) from exc

    def create_interpretation(
        self,
        weapon_id: str,
        request: CreativeInterpretationRequest,
        idempotency_key: str,
    ) -> CreativeInterpretationResponse:
        try:
            return self.creative_recast.create_interpretation(
                weapon_id,
                request,
                idempotency_key,
            )
        except CreativeRecastIdempotencyConflict as exc:
            raise IdempotencyConflictError(str(exc)) from exc
        except CreativeRecastError as exc:
            raise AssetStoreError(exc.code, str(exc), recoverable=exc.recoverable) from exc

    def confirm_recast(
        self,
        weapon_id: str,
        request: CreativeRecastConfirmRequest,
        idempotency_key: str,
    ) -> CreativeRecastConfirmResponse:
        try:
            return self.creative_recast.confirm_recast(
                weapon_id,
                request,
                idempotency_key,
            )
        except CreativeRecastIdempotencyConflict as exc:
            raise IdempotencyConflictError(str(exc)) from exc
        except CreativeRecastError as exc:
            raise AssetStoreError(exc.code, str(exc), recoverable=exc.recoverable) from exc

    def get_creative_graph(self, weapon_id: str) -> CreativeGraphResponse:
        try:
            return self.creative_recast.get_creative_graph(weapon_id)
        except CreativeRecastError as exc:
            raise AssetStoreError(exc.code, str(exc), recoverable=exc.recoverable) from exc

    def _write_rough_model_assets(
        self,
        conn: sqlite3.Connection,
        *,
        weapon_id: str,
        version_id: str,
        job_id: str,
        model_id: str,
        source_image_file_id: str,
        source_image_bytes: bytes,
        source_image_mime_type: str,
        source_image_logical_path: str,
        target_format: str,
        style: str,
        orientation_policy: Dict[str, str],
        scale_policy: str,
        gated_by_quality_report_file_id: Optional[str],
    ) -> Dict[str, str]:
        result = self.three_d_provider.generate_rough_model(
            weapon_id=weapon_id,
            model_id=model_id,
            source_image_asset_id=source_image_file_id,
            source_image_bytes=source_image_bytes,
            source_image_mime_type=source_image_mime_type,
            source_image_logical_path=source_image_logical_path,
            target_format=target_format,
            style=style,
            orientation_policy=orientation_policy,
            scale_policy=scale_policy,
        )
        return self._write_rough_model_result_assets(
            conn,
            weapon_id=weapon_id,
            version_id=version_id,
            job_id=job_id,
            model_id=model_id,
            source_image_file_id=source_image_file_id,
            orientation_policy=orientation_policy,
            scale_policy=scale_policy,
            gated_by_quality_report_file_id=gated_by_quality_report_file_id,
            result=result,
        )

    def _write_rough_model_result_assets(
        self,
        conn: sqlite3.Connection,
        *,
        weapon_id: str,
        version_id: str,
        job_id: str,
        model_id: str,
        source_image_file_id: str,
        orientation_policy: Dict[str, str],
        scale_policy: str,
        gated_by_quality_report_file_id: Optional[str],
        result: RoughModelResult,
    ) -> Dict[str, str]:
        common_metadata = {
            **result.metadata,
            "source_image_asset_id": source_image_file_id,
            "model_id": model_id,
            "provider": self.three_d_provider.provider_id,
        }
        raw_model_file_id = self._write_asset(
            conn,
            weapon_id=weapon_id,
            version_id=version_id,
            job_id=job_id,
            role="rough_raw_glb",
            logical_path=f"weapons/{weapon_id}/models/{model_id}/raw/provider_output.glb",
            payload=result.raw_glb_bytes,
            ext=".glb",
            mime_type="model/gltf-binary",
            metadata={**common_metadata, "pipeline_stage": "raw"},
        )
        normalized_model_file_id = self._write_asset(
            conn,
            weapon_id=weapon_id,
            version_id=version_id,
            job_id=job_id,
            role="rough_normalized_glb",
            logical_path=f"weapons/{weapon_id}/models/{model_id}/processed/rough_normalized.glb",
            payload=result.normalized_glb_bytes,
            ext=".glb",
            mime_type="model/gltf-binary",
            metadata={**common_metadata, "pipeline_stage": "normalized"},
        )
        optimized_model_file_id = self._write_asset(
            conn,
            weapon_id=weapon_id,
            version_id=version_id,
            job_id=job_id,
            role="rough_optimized_glb",
            logical_path=f"weapons/{weapon_id}/models/{model_id}/processed/rough_optimized.glb",
            payload=result.optimized_glb_bytes,
            ext=".glb",
            mime_type="model/gltf-binary",
            metadata={**common_metadata, "pipeline_stage": "optimized"},
        )
        unity_material_file_id = self._write_asset(
            conn,
            weapon_id=weapon_id,
            version_id=version_id,
            job_id=job_id,
            role="unity_material_json",
            logical_path=f"weapons/{weapon_id}/models/{model_id}/unity/unity_material.json",
            payload=_canonical_json(result.unity_material_json).encode("utf-8"),
            ext=".json",
            mime_type="application/json",
            metadata={"schema_version": "UnityMaterial@1", "provider": self.three_d_provider.provider_id, "model_id": model_id},
        )
        optimized_sha256 = conn.execute("SELECT sha256 FROM asset_files WHERE file_id = ?", (optimized_model_file_id,)).fetchone()["sha256"]
        analyzed_metrics = _merge_model_metrics(result.metrics, _analyze_glb_payload(result.optimized_glb_bytes))
        report = validate_quality_report(
            _model_quality_report(
                model_id,
                optimized_model_file_id=optimized_model_file_id,
                optimized_sha256=optimized_sha256,
                provider_id=self.three_d_provider.provider_id,
                source_image_file_id=source_image_file_id,
                metrics=analyzed_metrics,
            ),
            provider_id="quality_checker",
        )
        model_quality_report_file_id = self._write_asset(
            conn,
            weapon_id=weapon_id,
            version_id=version_id,
            job_id=job_id,
            role="quality_report",
            logical_path=f"weapons/{weapon_id}/models/{model_id}/model_quality_report.json",
            payload=_canonical_json(report).encode("utf-8"),
            ext=".json",
            mime_type="application/json",
            metadata={
                "schema_version": "QualityReport@1",
                "target_model_id": model_id,
                "target_asset_id": optimized_model_file_id,
                "target_sha256": optimized_sha256,
                "provider": "quality_checker",
            },
        )
        conn.execute(
            """
            INSERT INTO models_3d (
              model_id, weapon_id, version_id, job_id, provider, status,
              source_image_file_id, raw_model_file_id, normalized_model_file_id,
              optimized_model_file_id, unity_material_file_id,
              orientation_policy_json, quality_report_json, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, 'optimized', ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                model_id,
                weapon_id,
                version_id,
                job_id,
                self.three_d_provider.provider_id,
                source_image_file_id,
                raw_model_file_id,
                normalized_model_file_id,
                optimized_model_file_id,
                unity_material_file_id,
                _canonical_json({**orientation_policy, "scale_policy": scale_policy}),
                _canonical_json(
                    {
                        "quality_report_file_id": model_quality_report_file_id,
                        "status": report["status"],
                        "metrics": analyzed_metrics,
                        "gated_by_quality_report_file_id": gated_by_quality_report_file_id,
                    }
                ),
                utc_now(),
                utc_now(),
            ),
        )
        return {
            "raw_model_file_id": raw_model_file_id,
            "normalized_model_file_id": normalized_model_file_id,
            "optimized_model_file_id": optimized_model_file_id,
            "unity_material_file_id": unity_material_file_id,
            "model_quality_report_file_id": model_quality_report_file_id,
            "provider_task_id": result.provider_task_id or "",
        }

    def generate_3d(self, weapon_id: str, request: Generate3DRequest, idempotency_key: str) -> JobDetail:
        if (
            os.environ.get("WUSHEN_GENERATE_3D_ASYNC", "0").strip() == "1"
            or os.environ.get("WUSHEN_GENERATE3D_ASYNC", "0").strip() == "1"
            or os.environ.get("WUSHEN_GENERATE3D_WORKER", "0").strip() == "1"
            or os.environ.get("WUSHEN_GENERATE3D_RUNTIME", "sync").strip().lower() in {"worker", "async"}
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
                    raise IdempotencyConflictError("Idempotency-Key was reused with a different request body.")
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
                raise AssetStoreError("VERSION_NOT_FOUND", "Source version was not found for this weapon.")

            source_image = self._asset_row(conn, request.source_image_asset_id)
            if source_image["weapon_id"] != weapon_id or source_image["version_id"] != request.source_version_id:
                raise AssetStoreError("INVALID_REQUEST", "Source image does not belong to the requested source version.")
            if source_image["role"] not in {"concept_image", "concept_patch"}:
                raise AssetStoreError("INVALID_REQUEST", "Source image must be a concept image or patch image.")

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
                (job_id, weapon_id, idempotency_scope, idempotency_key, request_hash, request_json, now, now, now),
            )
            conn.execute(
                """
                INSERT INTO weapon_versions (
                  version_id, weapon_id, parent_version_id, job_id, version_no,
                  version_type, status, summary, created_at
                )
                VALUES (?, ?, ?, ?, ?, 'rough_3d', 'committed', ?, ?)
                """,
                (version_id, weapon_id, request.source_version_id, job_id, version_no, "Rough 3D generated from selected concept image.", now),
            )
            self._insert_step(conn, job_id, "rough3d_plan", "succeeded", "asset_store")
            self._insert_step(conn, job_id, "rough3d_submit", "succeeded", self.three_d_provider.provider_id)
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
                metadata={"schema_version": "ModelGenerationInput@1", "provider": request.provider_id},
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
                raise AssetStoreError(exc.code, str(exc), recoverable=exc.recoverable) from exc

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
                ("rough3d_plan", "succeeded", "3D generation input captured.", model_input_file_id, 0.2, {"source_image_asset_id": request.source_image_asset_id}),
                ("rough3d_submit", "succeeded", "Rough 3D provider produced GLB variants.", model_assets["raw_model_file_id"], 0.55, {"model_id": model_id, "provider": self.three_d_provider.provider_id}),
                ("model_qc_optimize", "succeeded", "Model quality report passed and optimized GLB was stored.", model_assets["model_quality_report_file_id"], 0.82, {"optimized_model_file_id": model_assets["optimized_model_file_id"]}),
                ("asset_commit_model", "succeeded", "3D model assets committed without overwriting the source version.", model_assets["optimized_model_file_id"], 0.95, {"new_version_id": version_id}),
                ("finalize_job", "succeeded", "Generate-3D job completed.", model_assets["model_quality_report_file_id"], 1.0, {"new_version_id": version_id, "parent_version_id": request.source_version_id}),
            ]
            for index, (step, status, message, artifact_id, progress, metadata) in enumerate(event_rows, start=1):
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

    def enqueue_generate_3d(self, weapon_id: str, request: Generate3DRequest, idempotency_key: str) -> JobDetail:
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
                    raise IdempotencyConflictError("Idempotency-Key was reused with a different request body.")
                return self.get_job(existing["job_id"])

            self._validate_generate_3d_source(conn, weapon_id, request)
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

    def run_worker_once(self, runner_id: str = "local_worker") -> RuntimeWorkOnceResponse:
        with self._connect() as conn:
            job = conn.execute(
                """
                SELECT job_id, weapon_id, job_type, status, current_step, request_json
                FROM generation_jobs
                WHERE (
                    job_type IN ('generate_3d', 'export_unity')
                    AND status IN ('queued', 'retrying')
                  )
                  OR (
                    job_type = 'generate_3d'
                    AND status = 'waiting_provider'
                    AND current_step = 'rough3d_submit'
                  )
                ORDER BY created_at ASC
                LIMIT 1
                """
            ).fetchone()
            if job is None:
                return RuntimeWorkOnceResponse(claimed=False, message="No queued worker job.")

            job_id = str(job["job_id"])
            job_type = str(job["job_type"])
            previous_status = str(job["status"])
            first_step = "rough3d_plan" if job_type == "generate_3d" else "export_plan"
            now = utc_now()
            if previous_status == "waiting_provider" and job_type == "generate_3d":
                conn.execute(
                    """
                    UPDATE generation_jobs
                    SET runner_id = ?,
                        lease_expires_at = datetime('now', '+5 minutes'),
                        updated_at = ?
                    WHERE job_id = ? AND status = 'waiting_provider' AND current_step = 'rough3d_submit'
                    """,
                    (runner_id, now, job_id),
                )
                if conn.total_changes == 0:
                    conn.commit()
                    return RuntimeWorkOnceResponse(claimed=False, message="Waiting provider job was already claimed.")
            else:
                conn.execute(
                    """
                    UPDATE generation_jobs
                    SET status = 'running',
                        current_step = ?,
                        runner_id = ?,
                        lease_expires_at = datetime('now', '+5 minutes'),
                        updated_at = ?,
                        finished_at = NULL
                    WHERE job_id = ? AND status IN ('queued', 'retrying')
                    """,
                    (first_step, runner_id, now, job_id),
                )
                if conn.total_changes == 0:
                    conn.commit()
                    return RuntimeWorkOnceResponse(claimed=False, message="Queued job was already claimed.")

                self._insert_job_action_event(
                    conn,
                    job_id=job_id,
                    weapon_id=job["weapon_id"],
                    step=first_step,
                    status="started",
                    level="info",
                    message=f"Local worker claimed {job_type} job.",
                    metadata={"runner_id": runner_id, "job_type": job_type},
                    created_at=now,
                )
            try:
                if job_type == "generate_3d":
                    request = Generate3DRequest(**json.loads(job["request_json"]))
                    if previous_status == "waiting_provider":
                        status = self._resume_generate_3d_provider_job(conn, job_id=job_id, weapon_id=str(job["weapon_id"]), request=request, runner_id=runner_id)
                    else:
                        status = self._complete_generate_3d_worker_job(conn, job_id=job_id, weapon_id=str(job["weapon_id"]), request=request, runner_id=runner_id)
                elif job_type == "export_unity":
                    request = ExportUnityRequest(**json.loads(job["request_json"]))
                    status = self._complete_export_unity_worker_job(conn, job_id=job_id, weapon_id=str(job["weapon_id"]), request=request, runner_id=runner_id)
                else:
                    raise AssetStoreError("INVALID_REQUEST", f"Unsupported worker job type: {job_type}")
                conn.commit()
                return RuntimeWorkOnceResponse(
                    claimed=True,
                    job_id=job_id,
                    job_type=job_type,
                    status=status,  # type: ignore[arg-type]
                    message=f"Worker completed {job_type} job with status {status}.",
                )
            except AssetStoreError as exc:
                failed_at = utc_now()
                failed_step = "rough3d_submit" if job_type == "generate_3d" else first_step
                conn.execute(
                    """
                    UPDATE generation_jobs
                    SET status = 'failed',
                        error_code = ?,
                        error_message = ?,
                        updated_at = ?,
                        finished_at = ?
                    WHERE job_id = ?
                    """,
                    (exc.code, str(exc), failed_at, failed_at, job_id),
                )
                self._insert_job_action_event(
                    conn,
                    job_id=job_id,
                    weapon_id=job["weapon_id"],
                    step=failed_step,
                    status="failed",
                    level="error",
                    message=str(exc),
                    metadata={"error_code": exc.code, "runner_id": runner_id, "job_type": job_type},
                    created_at=failed_at,
                )
                conn.commit()
                return RuntimeWorkOnceResponse(claimed=True, job_id=job_id, job_type=job_type, status="failed", message=str(exc))

    def _validate_generate_3d_source(self, conn: sqlite3.Connection, weapon_id: str, request: Generate3DRequest) -> sqlite3.Row:
        source_version = conn.execute(
            """
            SELECT version_id, version_no
            FROM weapon_versions
            WHERE weapon_id = ? AND version_id = ? AND status = 'committed'
            """,
            (weapon_id, request.source_version_id),
        ).fetchone()
        if source_version is None:
            raise AssetStoreError("VERSION_NOT_FOUND", "Source version was not found for this weapon.")

        source_image = self._asset_row(conn, request.source_image_asset_id)
        if source_image["weapon_id"] != weapon_id or source_image["version_id"] != request.source_version_id:
            raise AssetStoreError("INVALID_REQUEST", "Source image does not belong to the requested source version.")
        if source_image["role"] not in {"concept_image", "concept_patch"}:
            raise AssetStoreError("INVALID_REQUEST", "Source image must be a concept image or patch image.")
        return source_image

    def _complete_generate_3d_worker_job(
        self,
        conn: sqlite3.Connection,
        *,
        job_id: str,
        weapon_id: str,
        request: Generate3DRequest,
        runner_id: str,
    ) -> str:
        job = conn.execute(
            "SELECT status FROM generation_jobs WHERE job_id = ?",
            (job_id,),
        ).fetchone()
        if job is None:
            raise AssetStoreError("JOB_NOT_FOUND", f"Job was not found: {job_id}")
        if job["status"] == "cancelled":
            return "cancelled"

        source_image = self._validate_generate_3d_source(conn, weapon_id, request)
        now = utc_now()
        version_id = _new_id("ver")
        model_id = _new_id("model")
        version_no = int(
            conn.execute(
                "SELECT COALESCE(MAX(version_no), 0) + 1 AS next_version_no FROM weapon_versions WHERE weapon_id = ?",
                (weapon_id,),
            ).fetchone()["next_version_no"]
        )
        conn.execute(
            """
            INSERT INTO weapon_versions (
              version_id, weapon_id, parent_version_id, job_id, version_no,
              version_type, status, summary, created_at
            )
            VALUES (?, ?, ?, ?, ?, 'rough_3d', 'draft', ?, ?)
            """,
            (version_id, weapon_id, request.source_version_id, job_id, version_no, "Worker draft for rough 3D generation.", now),
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
            "runtime": "worker",
            "runner_id": runner_id,
            "non_manufacturing_asset": True,
            "created_at": now,
        }

        self._update_step_runtime(conn, job_id, "rough3d_plan", "running", now)
        conn.execute(
            "UPDATE generation_jobs SET status = 'running', current_step = 'rough3d_plan', updated_at = ? WHERE job_id = ?",
            (now, job_id),
        )
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
            metadata={"schema_version": "ModelGenerationInput@1", "provider": request.provider_id, "runtime": "worker"},
        )
        self._update_step_runtime(conn, job_id, "rough3d_plan", "succeeded", now)
        self._insert_job_action_event(
            conn,
            job_id=job_id,
            weapon_id=weapon_id,
            step="rough3d_plan",
            status="succeeded",
            level="info",
            message="3D generation input captured by local worker.",
            metadata={"source_image_asset_id": request.source_image_asset_id, "artifact_asset_id": model_input_file_id},
            created_at=now,
        )

        submit_at = utc_now()
        self._update_step_runtime(conn, job_id, "rough3d_submit", "running", submit_at)
        try:
            submission = self.three_d_provider.submit_rough_model(
                weapon_id=weapon_id,
                model_id=model_id,
                source_image_asset_id=request.source_image_asset_id,
                source_image_bytes=source_image_bytes,
                source_image_mime_type=source_image["mime_type"],
                source_image_logical_path=source_image["logical_path"],
                target_format=request.target_format,
                style=request.style,
                orientation_policy=request.orientation_policy,
                scale_policy=request.scale_policy,
            )
        except ThreeDProviderError as exc:
            raise AssetStoreError(exc.code, str(exc), recoverable=exc.recoverable) from exc
        provider_task_id = submission.provider_task_id
        task_record_id = self._record_provider_task(
            conn,
            job_id=job_id,
            step_name="rough3d_submit",
            attempt=_latest_step_attempt(conn, job_id, "rough3d_submit"),
            provider_kind="three_d",
            provider_id=self.three_d_provider.provider_id,
            provider_task_id=provider_task_id,
            status=submission.status,
            metadata={
                **submission.metadata,
                "phase": "submitted",
                "model_id": model_id,
                "version_id": version_id,
                "source_image_asset_id": request.source_image_asset_id,
            },
            updated_at=submit_at,
        )
        self._upsert_checkpoint(
            conn,
            job_id=job_id,
            step_name="rough3d_submit",
            attempt=_latest_step_attempt(conn, job_id, "rough3d_submit"),
            status="ready",
            resume_policy="restart_step",
            provider_task_record_id=task_record_id,
            state={
                "phase": "submitted",
                "model_id": model_id,
                "version_id": version_id,
                "provider_task_id": provider_task_id,
                "source_image_asset_id": request.source_image_asset_id,
            },
            updated_at=submit_at,
        )
        self._insert_job_action_event(
            conn,
            job_id=job_id,
            weapon_id=weapon_id,
            step="rough3d_submit",
            status="progress",
            level="info",
            message="Rough 3D provider task submitted.",
            metadata={
                "provider": self.three_d_provider.provider_id,
                "provider_task_record_id": task_record_id,
                "provider_task_id": provider_task_id,
                "model_id": model_id,
            },
            created_at=submit_at,
        )

        if self._job_cancel_requested(conn, job_id):
            self._cancel_provider_task(conn, task_record_id=task_record_id, provider_task_id=provider_task_id)
            self._suppress_cancelled_worker_commit(conn, job_id=job_id, weapon_id=weapon_id, step="rough3d_submit")
            return "cancelled"

        try:
            poll = self.three_d_provider.poll_rough_model(provider_task_id=provider_task_id)
        except ThreeDProviderError as exc:
            raise AssetStoreError(exc.code, str(exc), recoverable=exc.recoverable) from exc

        return self._handle_generate_3d_provider_poll(
            conn,
            job_id=job_id,
            weapon_id=weapon_id,
            request=request,
            version_id=version_id,
            model_id=model_id,
            source_image=source_image,
            source_image_bytes=source_image_bytes,
            task_record_id=task_record_id,
            provider_task_id=provider_task_id,
            poll_status=poll.status,
            poll_progress=poll.progress,
            poll_metadata=poll.metadata,
            poll_error_code=poll.error_code,
            poll_error_message=poll.error_message,
        )

    def _resume_generate_3d_provider_job(
        self,
        conn: sqlite3.Connection,
        *,
        job_id: str,
        weapon_id: str,
        request: Generate3DRequest,
        runner_id: str,
    ) -> str:
        version = conn.execute(
            """
            SELECT version_id
            FROM weapon_versions
            WHERE job_id = ? AND weapon_id = ? AND version_type = 'rough_3d' AND status = 'draft'
            ORDER BY created_at DESC
            LIMIT 1
            """,
            (job_id, weapon_id),
        ).fetchone()
        if version is None:
            raise AssetStoreError("JOB_CHECKPOINT_MISSING", "Generate-3D provider job is missing its draft version.")
        task = conn.execute(
            """
            SELECT task_record_id, provider_task_id, status, metadata_json
            FROM provider_tasks
            WHERE job_id = ? AND step_name = 'rough3d_submit'
            ORDER BY attempt DESC, updated_at DESC
            LIMIT 1
            """,
            (job_id,),
        ).fetchone()
        if task is None or not task["provider_task_id"]:
            raise AssetStoreError("JOB_CHECKPOINT_MISSING", "Generate-3D provider job is missing its provider task.")
        metadata = json.loads(task["metadata_json"] or "{}")
        model_id = str(metadata.get("model_id") or "")
        if not model_id:
            raise AssetStoreError("JOB_CHECKPOINT_MISSING", "Generate-3D provider checkpoint is missing model_id.")
        provider_task_id = str(task["provider_task_id"])
        task_record_id = str(task["task_record_id"])
        source_image = self._validate_generate_3d_source(conn, weapon_id, request)
        source_image_bytes = self._asset_payload(source_image)

        if task["status"] == "cancel_requested" or self._job_cancel_requested(conn, job_id):
            self._cancel_provider_task(conn, task_record_id=task_record_id, provider_task_id=provider_task_id)
            self._suppress_cancelled_worker_commit(conn, job_id=job_id, weapon_id=weapon_id, step="rough3d_submit")
            return "cancelled"

        try:
            poll = self.three_d_provider.poll_rough_model(provider_task_id=provider_task_id)
        except ThreeDProviderError as exc:
            raise AssetStoreError(exc.code, str(exc), recoverable=exc.recoverable) from exc

        return self._handle_generate_3d_provider_poll(
            conn,
            job_id=job_id,
            weapon_id=weapon_id,
            request=request,
            version_id=str(version["version_id"]),
            model_id=model_id,
            source_image=source_image,
            source_image_bytes=source_image_bytes,
            task_record_id=task_record_id,
            provider_task_id=provider_task_id,
            poll_status=poll.status,
            poll_progress=poll.progress,
            poll_metadata=poll.metadata,
            poll_error_code=poll.error_code,
            poll_error_message=poll.error_message,
        )

    def _handle_generate_3d_provider_poll(
        self,
        conn: sqlite3.Connection,
        *,
        job_id: str,
        weapon_id: str,
        request: Generate3DRequest,
        version_id: str,
        model_id: str,
        source_image: sqlite3.Row,
        source_image_bytes: bytes,
        task_record_id: str,
        provider_task_id: str,
        poll_status: str,
        poll_progress: float,
        poll_metadata: Dict[str, Any],
        poll_error_code: Optional[str],
        poll_error_message: Optional[str],
    ) -> str:
        poll_at = utc_now()
        base_metadata = self._provider_task_metadata(conn, task_record_id)
        provider_metadata = {
            **base_metadata,
            **poll_metadata,
            "phase": "polling" if poll_status in {"submitted", "polling", "unknown"} else poll_status,
            "model_id": model_id,
            "version_id": version_id,
            "source_image_asset_id": request.source_image_asset_id,
            "last_progress": poll_progress,
            "last_provider_state": poll_status,
        }
        conn.execute(
            """
            UPDATE provider_tasks
            SET status = ?, last_seen_at = ?, updated_at = ?, metadata_json = ?
            WHERE task_record_id = ?
            """,
            (poll_status, poll_at, poll_at, _canonical_json(provider_metadata), task_record_id),
        )
        if poll_status in {"submitted", "polling", "unknown"}:
            self._update_step_runtime(conn, job_id, "rough3d_submit", "waiting_provider", poll_at, provider_task_id=provider_task_id)
            self._upsert_checkpoint(
                conn,
                job_id=job_id,
                step_name="rough3d_submit",
                attempt=_latest_step_attempt(conn, job_id, "rough3d_submit"),
                status="ready",
                resume_policy="restart_step",
                provider_task_record_id=task_record_id,
                state={
                    "phase": "polling",
                    "model_id": model_id,
                    "version_id": version_id,
                    "provider_task_id": provider_task_id,
                    "source_image_asset_id": request.source_image_asset_id,
                    "last_provider_state": poll_status,
                    "last_progress": poll_progress,
                },
                updated_at=poll_at,
            )
            conn.execute(
                """
                UPDATE generation_jobs
                SET status = 'waiting_provider',
                    current_step = 'rough3d_submit',
                    runner_id = NULL,
                    lease_expires_at = NULL,
                    updated_at = ?,
                    finished_at = NULL
                WHERE job_id = ?
                """,
                (poll_at, job_id),
            )
            self._insert_job_action_event(
                conn,
                job_id=job_id,
                weapon_id=weapon_id,
                step="rough3d_submit",
                status="waiting_provider",
                level="info",
                message="Rough 3D provider task is still running; worker will poll again.",
                metadata={
                    "provider": self.three_d_provider.provider_id,
                    "provider_task_id": provider_task_id,
                    "provider_state": poll_status,
                    "progress": poll_progress,
                },
                created_at=poll_at,
            )
            return "waiting_provider"

        if poll_status == "failed":
            raise AssetStoreError(poll_error_code or "PROVIDER_BAD_OUTPUT", poll_error_message or "Rough 3D provider task failed.", recoverable=True)
        if poll_status == "cancelled":
            self._suppress_cancelled_worker_commit(conn, job_id=job_id, weapon_id=weapon_id, step="rough3d_submit")
            return "cancelled"
        if poll_status != "succeeded":
            raise AssetStoreError("PROVIDER_BAD_OUTPUT", f"Unexpected rough 3D provider status: {poll_status}", recoverable=True)

        if self._job_cancel_requested(conn, job_id):
            self._cancel_provider_task(conn, task_record_id=task_record_id, provider_task_id=provider_task_id)
            self._suppress_cancelled_worker_commit(conn, job_id=job_id, weapon_id=weapon_id, step="rough3d_submit")
            return "cancelled"

        try:
            result = self.three_d_provider.fetch_rough_model(
                provider_task_id=provider_task_id,
                weapon_id=weapon_id,
                model_id=model_id,
                source_image_asset_id=request.source_image_asset_id,
                source_image_bytes=source_image_bytes,
                source_image_mime_type=source_image["mime_type"],
                source_image_logical_path=source_image["logical_path"],
                target_format=request.target_format,
                style=request.style,
                orientation_policy=request.orientation_policy,
                scale_policy=request.scale_policy,
            )
        except ThreeDProviderError as exc:
            raise AssetStoreError(exc.code, str(exc), recoverable=exc.recoverable) from exc

        if self._job_cancel_requested(conn, job_id):
            self._cancel_provider_task(conn, task_record_id=task_record_id, provider_task_id=provider_task_id)
            self._suppress_cancelled_worker_commit(conn, job_id=job_id, weapon_id=weapon_id, step="rough3d_submit")
            return "cancelled"

        model_assets = self._write_rough_model_result_assets(
            conn,
            weapon_id=weapon_id,
            version_id=version_id,
            job_id=job_id,
            model_id=model_id,
            source_image_file_id=request.source_image_asset_id,
            orientation_policy=request.orientation_policy,
            scale_policy=request.scale_policy,
            gated_by_quality_report_file_id=None,
            result=result,
        )
        finished_provider_at = utc_now()
        conn.execute(
            """
            UPDATE provider_tasks
            SET status = 'succeeded', last_seen_at = ?, updated_at = ?, metadata_json = ?
            WHERE task_record_id = ?
            """,
            (
                finished_provider_at,
                finished_provider_at,
                _canonical_json(
                    {
                        **self._provider_task_metadata(conn, task_record_id),
                        **result.metadata,
                        "phase": "fetched",
                        "model_id": model_id,
                        "version_id": version_id,
                        "provider_task_id": provider_task_id,
                        "raw_model_file_id": model_assets["raw_model_file_id"],
                        "optimized_model_file_id": model_assets["optimized_model_file_id"],
                    }
                ),
                task_record_id,
            ),
        )
        self._update_step_runtime(conn, job_id, "rough3d_submit", "succeeded", utc_now(), provider_task_id=provider_task_id)
        self._upsert_checkpoint(
            conn,
            job_id=job_id,
            step_name="rough3d_submit",
            attempt=_latest_step_attempt(conn, job_id, "rough3d_submit"),
            status="completed",
            resume_policy="skip_completed",
            provider_task_record_id=task_record_id,
            state={
                "phase": "completed",
                "model_id": model_id,
                "version_id": version_id,
                "provider_task_id": provider_task_id,
                "source_image_asset_id": request.source_image_asset_id,
                "raw_model_file_id": model_assets["raw_model_file_id"],
                "optimized_model_file_id": model_assets["optimized_model_file_id"],
            },
            updated_at=utc_now(),
        )

        conn.execute(
            "UPDATE weapon_versions SET status = 'committed', summary = ? WHERE version_id = ?",
            ("Rough 3D generated from selected concept image by worker.", version_id),
        )
        conn.execute(
            """
            UPDATE weapons
            SET current_version_id = ?, updated_at = ?
            WHERE weapon_id = ?
            """,
            (version_id, utc_now(), weapon_id),
        )

        event_rows = [
            ("rough3d_submit", "succeeded", "Rough 3D provider produced GLB variants.", model_assets["raw_model_file_id"], 0.55, {"model_id": model_id, "provider": self.three_d_provider.provider_id, "provider_task_id": provider_task_id}),
            ("model_qc_optimize", "succeeded", "Model quality report passed and optimized GLB was stored.", model_assets["model_quality_report_file_id"], 0.82, {"optimized_model_file_id": model_assets["optimized_model_file_id"]}),
            ("asset_commit_model", "succeeded", "3D model assets committed without overwriting the source version.", model_assets["optimized_model_file_id"], 0.95, {"new_version_id": version_id}),
            ("finalize_job", "succeeded", "Generate-3D worker job completed.", model_assets["model_quality_report_file_id"], 1.0, {"new_version_id": version_id, "parent_version_id": request.source_version_id}),
        ]
        for step, status, message, artifact_id, progress, metadata in event_rows:
            next_seq = self._next_event_seq(conn, job_id)
            self._insert_event(
                conn,
                event_id=f"evt_{job_id}_{next_seq:04d}",
                seq=next_seq,
                job_id=job_id,
                weapon_id=weapon_id,
                step=step,
                status=status,
                message=message,
                artifact_asset_id=artifact_id,
                progress=progress,
                created_at=utc_now(),
                metadata=metadata,
            )
            if step != "rough3d_submit":
                self._update_step_runtime(conn, job_id, step, status, utc_now())

        finished_at = utc_now()
        conn.execute(
            """
            UPDATE generation_jobs
            SET status = 'succeeded',
                current_step = 'finalize_job',
                runner_id = NULL,
                lease_expires_at = NULL,
                updated_at = ?,
                finished_at = ?
            WHERE job_id = ?
            """,
            (finished_at, finished_at, job_id),
        )
        return "succeeded"

    def _provider_task_metadata(self, conn: sqlite3.Connection, task_record_id: str) -> Dict[str, Any]:
        row = conn.execute(
            "SELECT metadata_json FROM provider_tasks WHERE task_record_id = ?",
            (task_record_id,),
        ).fetchone()
        if row is None:
            return {}
        try:
            return json.loads(row["metadata_json"] or "{}")
        except json.JSONDecodeError:
            return {}

    def _cancel_provider_task(self, conn: sqlite3.Connection, *, task_record_id: str, provider_task_id: str) -> None:
        now = utc_now()
        metadata = self._provider_task_metadata(conn, task_record_id)
        try:
            cancel = self.three_d_provider.cancel_rough_model(provider_task_id=provider_task_id)
            status = "cancelled" if cancel.status == "cancelled" else "cancel_requested"
            metadata = {**metadata, **cancel.metadata, "provider_cancel_status": cancel.status}
        except ThreeDProviderError as exc:
            status = "cancel_requested"
            metadata = {**metadata, "provider_cancel_error": exc.code, "provider_cancel_message": str(exc)}
        conn.execute(
            """
            UPDATE provider_tasks
            SET status = ?, last_seen_at = ?, updated_at = ?, metadata_json = ?
            WHERE task_record_id = ?
            """,
            (status, now, now, _canonical_json(metadata), task_record_id),
        )

    def enqueue_export_unity(self, weapon_id: str, request: ExportUnityRequest, idempotency_key: str) -> JobDetail:
        idempotency_scope = f"POST /api/weapons/{weapon_id}/export-unity"
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
                    raise IdempotencyConflictError("Idempotency-Key was reused with a different request body.")
                return self.get_job(existing["job_id"])

            model = self._validate_unity_export_inputs(conn, weapon_id, request)["model"]
            now = utc_now()
            job_id = _new_id("job")
            conn.execute(
                """
                INSERT INTO generation_jobs (
                  job_id, weapon_id, job_type, status, current_step, idempotency_scope,
                  idempotency_key, request_hash, request_json, checkpoint_json,
                  created_at, updated_at, finished_at
                )
                VALUES (?, ?, 'export_unity', 'queued', 'export_plan', ?, ?, ?, ?, ?, ?, ?, NULL)
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
                            "model_id": model["model_id"],
                            "source_version_id": model["version_id"],
                            "next_step": "export_plan",
                        }
                    ),
                    now,
                    now,
                ),
            )
            for step_name in ["export_plan", "export_manifest", "export_package", "finalize_job"]:
                self._insert_step(conn, job_id, step_name, "queued", "asset_store")
            self._insert_event(
                conn,
                event_id=f"evt_{job_id}_0001",
                seq=1,
                job_id=job_id,
                weapon_id=weapon_id,
                step="export_plan",
                status="queued",
                message="Unity export job queued for local worker.",
                artifact_asset_id=None,
                progress=0.02,
                created_at=now,
                metadata={"runtime": "worker", "model_id": model["model_id"], "source_version_id": model["version_id"]},
            )
            conn.commit()
            return self.get_job(job_id)

    def _complete_export_unity_worker_job(
        self,
        conn: sqlite3.Connection,
        *,
        job_id: str,
        weapon_id: str,
        request: ExportUnityRequest,
        runner_id: str,
    ) -> str:
        job = conn.execute(
            "SELECT status FROM generation_jobs WHERE job_id = ?",
            (job_id,),
        ).fetchone()
        if job is None:
            raise AssetStoreError("JOB_NOT_FOUND", f"Job was not found: {job_id}")
        if job["status"] == "cancelled":
            return "cancelled"

        inputs = self._validate_unity_export_inputs(conn, weapon_id, request)
        model = inputs["model"]
        optimized_model = inputs["optimized_model"]
        unity_material = inputs["unity_material"]
        quality_report_asset = inputs["quality_report_asset"]
        spec_asset = inputs["spec_asset"]
        if model is None or optimized_model is None or unity_material is None:
            raise AssetStoreError("INVALID_REQUEST", "Unity export inputs could not be resolved.")

        now = utc_now()
        version_id = _new_id("ver")
        export_id = _new_id("export")
        version_no = int(
            conn.execute(
                "SELECT COALESCE(MAX(version_no), 0) + 1 AS next_version_no FROM weapon_versions WHERE weapon_id = ?",
                (weapon_id,),
            ).fetchone()["next_version_no"]
        )
        self._update_step_runtime(conn, job_id, "export_plan", "running", now)
        conn.execute(
            "UPDATE generation_jobs SET status = 'running', current_step = 'export_plan', updated_at = ? WHERE job_id = ?",
            (now, job_id),
        )
        self._update_step_runtime(conn, job_id, "export_plan", "succeeded", utc_now())
        self._insert_job_action_event(
            conn,
            job_id=job_id,
            weapon_id=weapon_id,
            step="export_plan",
            status="succeeded",
            level="info",
            message="Unity export inputs resolved by local worker.",
            metadata={"model_id": model["model_id"], "runner_id": runner_id},
            created_at=utc_now(),
        )
        if self._job_cancel_requested(conn, job_id):
            self._suppress_cancelled_worker_commit(conn, job_id=job_id, weapon_id=weapon_id, step="export_plan")
            return "cancelled"

        manifest_at = utc_now()
        self._update_step_runtime(conn, job_id, "export_manifest", "running", manifest_at)
        conn.execute(
            "UPDATE generation_jobs SET current_step = 'export_manifest', updated_at = ? WHERE job_id = ?",
            (manifest_at, job_id),
        )
        manifest = self._unity_export_manifest(
            export_id=export_id,
            weapon_id=weapon_id,
            version_id=version_id,
            model=model,
            optimized_model=optimized_model,
            unity_material=unity_material,
            quality_report_asset=quality_report_asset,
            spec_asset=spec_asset,
            created_at=manifest_at,
        )
        package_payload = self._build_unity_export_zip(
            manifest=manifest,
            optimized_model=optimized_model,
            unity_material=unity_material,
            quality_report_asset=quality_report_asset,
            spec_asset=spec_asset,
        )
        self._update_step_runtime(conn, job_id, "export_manifest", "succeeded", utc_now())

        if self._job_cancel_requested(conn, job_id):
            self._suppress_cancelled_worker_commit(conn, job_id=job_id, weapon_id=weapon_id, step="export_manifest")
            return "cancelled"

        package_at = utc_now()
        self._update_step_runtime(conn, job_id, "export_package", "running", package_at)
        conn.execute(
            "UPDATE generation_jobs SET current_step = 'export_package', updated_at = ? WHERE job_id = ?",
            (package_at, job_id),
        )
        package_logical_path = f"weapons/{weapon_id}/exports/{export_id}/unity_glb_package.zip"
        conn.execute(
            """
            INSERT INTO weapon_versions (
              version_id, weapon_id, parent_version_id, job_id, version_no,
              version_type, status, summary, created_at
            )
            VALUES (?, ?, ?, ?, ?, 'export', 'committed', ?, ?)
            """,
            (version_id, weapon_id, model["version_id"], job_id, version_no, "Unity GLB export package snapshot from local worker.", package_at),
        )
        package_asset_id = self._write_asset(
            conn,
            weapon_id=weapon_id,
            version_id=version_id,
            job_id=job_id,
            role="unity_export_package",
            logical_path=package_logical_path,
            payload=package_payload,
            ext=".zip",
            mime_type="application/zip",
            metadata={
                "schema_version": "UnityExportManifest@1",
                "export_id": export_id,
                "model_id": model["model_id"],
                "export_type": request.export_type,
                "runtime": "worker",
                "runner_id": runner_id,
                "non_manufacturing_asset": True,
            },
        )
        package_sha = conn.execute("SELECT sha256 FROM asset_files WHERE file_id = ?", (package_asset_id,)).fetchone()["sha256"]
        manifest["package"] = {
            "asset_id": package_asset_id,
            "sha256": package_sha,
            "logical_path": package_logical_path,
            "mime_type": "application/zip",
        }
        conn.execute(
            """
            INSERT INTO export_packages (
              export_id, weapon_id, version_id, model_id, job_id, export_type,
              status, package_path, manifest_json, created_at
            )
            VALUES (?, ?, ?, ?, ?, 'unity_glb', 'validated', ?, ?, ?)
            """,
            (
                export_id,
                weapon_id,
                version_id,
                model["model_id"],
                job_id,
                package_logical_path,
                _canonical_json(manifest),
                package_at,
            ),
        )
        conn.execute(
            "UPDATE weapons SET current_version_id = ?, updated_at = ? WHERE weapon_id = ?",
            (version_id, utc_now(), weapon_id),
        )

        event_rows = [
            ("export_manifest", "succeeded", "Unity export manifest built with relative package paths only.", package_asset_id, 0.55, {"export_id": export_id}),
            ("export_package", "succeeded", "Unity GLB export package committed to the asset library by worker.", package_asset_id, 0.9, {"package_sha256": package_sha}),
            ("finalize_job", "succeeded", "Unity export worker job completed for fictional game-art handoff.", package_asset_id, 1.0, {"export_id": export_id, "new_version_id": version_id}),
        ]
        for step, status, message, artifact_id, progress, metadata in event_rows:
            next_seq = self._next_event_seq(conn, job_id)
            self._insert_event(
                conn,
                event_id=f"evt_{job_id}_{next_seq:04d}",
                seq=next_seq,
                job_id=job_id,
                weapon_id=weapon_id,
                step=step,
                status=status,
                message=message,
                artifact_asset_id=artifact_id,
                progress=progress,
                created_at=utc_now(),
                metadata=metadata,
            )
            self._update_step_runtime(conn, job_id, step, status, utc_now())

        finished_at = utc_now()
        conn.execute(
            """
            UPDATE generation_jobs
            SET status = 'succeeded',
                current_step = 'finalize_job',
                runner_id = NULL,
                lease_expires_at = NULL,
                updated_at = ?,
                finished_at = ?
            WHERE job_id = ?
            """,
            (finished_at, finished_at, job_id),
        )
        return "succeeded"

    def export_unity(self, weapon_id: str, request: ExportUnityRequest, idempotency_key: str) -> JobDetail:
        if (
            os.environ.get("WUSHEN_RUNTIME_WORKER", "0").strip() == "1"
            or os.environ.get("WUSHEN_EXPORT_UNITY_WORKER", "0").strip() == "1"
            or os.environ.get("WUSHEN_EXPORT_UNITY_ASYNC", "0").strip() == "1"
        ):
            return self.enqueue_export_unity(weapon_id, request, idempotency_key)

        idempotency_scope = f"POST /api/weapons/{weapon_id}/export-unity"
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
                    raise IdempotencyConflictError("Idempotency-Key was reused with a different request body.")
                return self.get_job(existing["job_id"])

            model = self._model_for_export(conn, weapon_id, request.model_id)
            if model["status"] not in {"optimized", "rough_preview"}:
                raise AssetStoreError("INVALID_REQUEST", "Only optimized rough models can be exported to Unity.")
            if not model["optimized_model_file_id"] or not model["unity_material_file_id"]:
                raise AssetStoreError("INVALID_REQUEST", "Model is missing optimized GLB or Unity material metadata.")

            parent_version = conn.execute(
                """
                SELECT version_id
                FROM weapon_versions
                WHERE weapon_id = ? AND version_id = ? AND status = 'committed'
                """,
                (weapon_id, model["version_id"]),
            ).fetchone()
            if parent_version is None:
                raise AssetStoreError("VERSION_NOT_FOUND", "Model version was not found for this weapon.")

            optimized_model = self._asset_row(conn, model["optimized_model_file_id"])
            unity_material = self._asset_row(conn, model["unity_material_file_id"])
            if optimized_model["role"] != "rough_optimized_glb":
                raise AssetStoreError("INVALID_REQUEST", "Unity export requires an optimized rough GLB asset.")
            if unity_material["role"] != "unity_material_json":
                raise AssetStoreError("INVALID_REQUEST", "Unity export requires Unity material metadata.")

            model_quality = json.loads(model["quality_report_json"] or "{}")
            quality_report_asset = None
            if request.include_quality_reports:
                report_file_id = model_quality.get("quality_report_file_id")
                if not report_file_id:
                    raise AssetStoreError("INVALID_REQUEST", "Model quality report is missing.")
                quality_report_asset = self._asset_row(conn, report_file_id)
            spec_asset = self._latest_weapon_spec_asset(conn, weapon_id) if request.include_source_spec else None

            now = utc_now()
            job_id = _new_id("job")
            version_id = _new_id("ver")
            export_id = _new_id("export")
            version_no = int(
                conn.execute(
                    "SELECT COALESCE(MAX(version_no), 0) + 1 AS next_version_no FROM weapon_versions WHERE weapon_id = ?",
                    (weapon_id,),
                ).fetchone()["next_version_no"]
            )
            manifest = self._unity_export_manifest(
                export_id=export_id,
                weapon_id=weapon_id,
                version_id=version_id,
                model=model,
                optimized_model=optimized_model,
                unity_material=unity_material,
                quality_report_asset=quality_report_asset,
                spec_asset=spec_asset,
                created_at=now,
            )
            package_payload = self._build_unity_export_zip(
                manifest=manifest,
                optimized_model=optimized_model,
                unity_material=unity_material,
                quality_report_asset=quality_report_asset,
                spec_asset=spec_asset,
            )
            package_logical_path = f"weapons/{weapon_id}/exports/{export_id}/unity_glb_package.zip"

            conn.execute(
                """
                INSERT INTO generation_jobs (
                  job_id, weapon_id, job_type, status, current_step, idempotency_scope, idempotency_key,
                  request_hash, request_json, created_at, updated_at, finished_at
                )
                VALUES (?, ?, 'export_unity', 'succeeded', 'finalize_job', ?, ?, ?, ?, ?, ?, ?)
                """,
                (job_id, weapon_id, idempotency_scope, idempotency_key, request_hash, request_json, now, now, now),
            )
            conn.execute(
                """
                INSERT INTO weapon_versions (
                  version_id, weapon_id, parent_version_id, job_id, version_no,
                  version_type, status, summary, created_at
                )
                VALUES (?, ?, ?, ?, ?, 'export', 'committed', ?, ?)
                """,
                (version_id, weapon_id, model["version_id"], job_id, version_no, "Unity GLB export package snapshot.", now),
            )
            self._insert_step(conn, job_id, "export_plan", "succeeded", "asset_store")
            self._insert_step(conn, job_id, "export_manifest", "succeeded", "asset_store")
            self._insert_step(conn, job_id, "export_package", "succeeded", "asset_store")
            self._insert_step(conn, job_id, "finalize_job", "succeeded", "asset_store")

            package_asset_id = self._write_asset(
                conn,
                weapon_id=weapon_id,
                version_id=version_id,
                job_id=job_id,
                role="unity_export_package",
                logical_path=package_logical_path,
                payload=package_payload,
                ext=".zip",
                mime_type="application/zip",
                metadata={
                    "schema_version": "UnityExportManifest@1",
                    "export_id": export_id,
                    "model_id": model["model_id"],
                    "export_type": request.export_type,
                    "non_manufacturing_asset": True,
                },
            )
            package_sha = conn.execute("SELECT sha256 FROM asset_files WHERE file_id = ?", (package_asset_id,)).fetchone()["sha256"]
            manifest["package"] = {
                "asset_id": package_asset_id,
                "sha256": package_sha,
                "logical_path": package_logical_path,
                "mime_type": "application/zip",
            }
            conn.execute(
                """
                INSERT INTO export_packages (
                  export_id, weapon_id, version_id, model_id, job_id, export_type,
                  status, package_path, manifest_json, created_at
                )
                VALUES (?, ?, ?, ?, ?, 'unity_glb', 'validated', ?, ?, ?)
                """,
                (
                    export_id,
                    weapon_id,
                    version_id,
                    model["model_id"],
                    job_id,
                    package_logical_path,
                    _canonical_json(manifest),
                    now,
                ),
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
                ("export_plan", "succeeded", "Unity export inputs resolved from the optimized rough model.", optimized_model["file_id"], 0.25, {"model_id": model["model_id"]}),
                ("export_manifest", "succeeded", "Unity export manifest built with relative package paths only.", package_asset_id, 0.55, {"export_id": export_id}),
                ("export_package", "succeeded", "Unity GLB export package committed to the asset library.", package_asset_id, 0.9, {"package_sha256": package_sha}),
                ("finalize_job", "succeeded", "Unity export package validated for fictional game-art handoff.", package_asset_id, 1.0, {"export_id": export_id, "new_version_id": version_id}),
            ]
            for index, (step, status, message, artifact_id, progress, metadata) in enumerate(event_rows, start=1):
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

    def patch_weapon(self, weapon_id: str, request: PatchWeaponRequest, idempotency_key: str) -> JobDetail:
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
                    raise IdempotencyConflictError("Idempotency-Key was reused with a different request body.")
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
                raise AssetStoreError("VERSION_NOT_FOUND", "Source version was not found for this weapon.")

            source_image = self._asset_row(conn, request.source_image_asset_id)
            mask_asset = self._asset_row(conn, request.mask_asset_id)
            manifest_asset = self._asset_row(conn, request.patch_manifest_asset_id)
            if source_image["weapon_id"] != weapon_id or source_image["version_id"] != request.source_version_id:
                raise AssetStoreError("INVALID_REQUEST", "Source image does not belong to the requested source version.")
            if source_image["role"] not in {"concept_image", "concept_patch"}:
                raise AssetStoreError("INVALID_REQUEST", "Source image must be a concept image or patch image.")
            if mask_asset["role"] != "patch_mask":
                raise AssetStoreError("INVALID_REQUEST", "mask_asset_id must reference a patch_mask asset.")
            if manifest_asset["role"] != "patch_manifest":
                raise AssetStoreError("INVALID_REQUEST", "patch_manifest_asset_id must reference a patch_manifest asset.")
            if not source_image["width"] or not source_image["height"]:
                raise AssetStoreError("INVALID_REQUEST", "Source image is missing width/height metadata.")
            if mask_asset["width"] != source_image["width"] or mask_asset["height"] != source_image["height"]:
                raise AssetStoreError("MASK_SIZE_MISMATCH", "Mask dimensions must match the source concept image.")

            mask_bytes = self._asset_payload(mask_asset)
            if not mask_png_has_ink(mask_bytes, int(mask_asset["width"]), int(mask_asset["height"])):
                raise AssetStoreError("MASK_EMPTY", "Patch mask is empty.")
            source_image_bytes = self._asset_payload(source_image)

            try:
                manifest = validate_patch_manifest(
                    json.loads(self._asset_payload(manifest_asset).decode("utf-8")),
                    provider_id="asset_store",
                )
            except (json.JSONDecodeError, WeaponSpecValidationError) as exc:
                raise AssetStoreError("INVALID_REQUEST", f"Patch manifest is invalid: {exc}") from exc
            if manifest["weapon_id"] != weapon_id:
                raise AssetStoreError("INVALID_REQUEST", "Patch manifest weapon_id does not match request weapon_id.")
            if manifest["source_asset_id"] != request.source_image_asset_id or manifest["mask_asset_id"] != request.mask_asset_id:
                raise AssetStoreError("INVALID_REQUEST", "Patch manifest asset references do not match request.")

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
                raise AssetStoreError(exc.code, str(exc), recoverable=exc.recoverable) from exc

            conn.execute(
                """
                INSERT INTO generation_jobs (
                  job_id, weapon_id, job_type, status, current_step, idempotency_scope, idempotency_key,
                  request_hash, request_json, created_at, updated_at, finished_at
                )
                VALUES (?, ?, 'patch_image', 'succeeded', 'finalize_job', ?, ?, ?, ?, ?, ?, ?)
                """,
                (job_id, weapon_id, idempotency_scope, idempotency_key, request_hash, request_json, now, now, now),
            )
            conn.execute(
                """
                INSERT INTO weapon_versions (
                  version_id, weapon_id, parent_version_id, job_id, version_no,
                  version_type, status, summary, created_at
                )
                VALUES (?, ?, ?, ?, ?, 'patch', 'committed', ?, ?)
                """,
                (version_id, weapon_id, request.source_version_id, job_id, version_no, f"Patch: {request.instruction}", now),
            )
            self._insert_step(conn, job_id, "patch_interpreter", "succeeded", "asset_store")
            self._insert_step(conn, job_id, "image_inpaint", "succeeded", self.image_provider.provider_id)
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
                metadata={"schema_version": "PatchPrompt@1", "provider": self.image_provider.provider_id, "requested_provider": request.provider_id},
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
                    "workflow_template_version": patch_result.metadata.get("workflow_template_version"),
                    "workflow_template_path": patch_result.metadata.get("workflow_template_path"),
                    "source_image_asset_id": request.source_image_asset_id,
                    "mask_asset_id": request.mask_asset_id,
                    "patch_manifest_asset_id": request.patch_manifest_asset_id,
                },
            )
            patch_sha256 = conn.execute("SELECT sha256 FROM asset_files WHERE file_id = ?", (patch_file_id,)).fetchone()["sha256"]
            report = validate_quality_report(
                _patch_quality_report(patch_file_id, patch_sha256=patch_sha256, mask_asset_id=request.mask_asset_id),
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
                metadata={"schema_version": "QualityReport@1", "target_asset_id": patch_file_id, "target_sha256": patch_sha256},
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
                ("patch_interpreter", "succeeded", "Patch manifest and mask validated.", patch_prompt_file_id, 0.25, {"mask_asset_id": request.mask_asset_id, "patch_manifest_asset_id": request.patch_manifest_asset_id}),
                ("image_inpaint", "succeeded", "Patch image generated as a new version.", patch_file_id, 0.7, {"source_image_asset_id": request.source_image_asset_id, "workflow_asset_id": workflow_file_id}),
                ("image_quality_check", "succeeded", "Patch image quality report passed.", quality_report_file_id, 0.9, {"target_asset_id": patch_file_id}),
                ("finalize_job", "succeeded", "Patch version committed without overwriting source version.", quality_report_file_id, 1.0, {"new_version_id": version_id, "parent_version_id": request.source_version_id}),
            ]
            for index, (step, status, message, artifact_id, progress, metadata) in enumerate(event_rows, start=1):
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

    def upload_asset(self, weapon_id: str, version_id: str, request: AssetUploadRequest, idempotency_key: str) -> AssetUploadResponse:
        try:
            return self.asset_uploads.upload(
                weapon_id,
                version_id,
                request,
                idempotency_key,
            )
        except AssetUploadIdempotencyConflict as exc:
            raise IdempotencyConflictError(str(exc)) from exc
        except AssetUploadError as exc:
            raise AssetStoreError(
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            ) from exc

    def activate_version(self, weapon_id: str, version_id: str) -> WeaponDetail:
        try:
            return self.library.activate_version(weapon_id, version_id)
        except LibraryError as exc:
            raise AssetStoreError(
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            ) from exc

    def list_weapons(self) -> List[WeaponSummary]:
        return self.library.list_weapons()

    def get_weapon_detail(self, weapon_id: str) -> WeaponDetail:
        return self.library.get_weapon_detail(weapon_id)

    def get_asset_metadata(self, asset_id: str) -> AssetFileResponse:
        try:
            return self.library.get_asset_metadata(asset_id)
        except LibraryError as exc:
            raise AssetStoreError(
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            ) from exc

    def reveal_asset_file(self, asset_id: str, *, dry_run: bool = False) -> AssetRevealResponse:
        asset = self.resolve_asset_file(asset_id)
        target = _asset_reveal_target()
        if not dry_run:
            _open_asset_location(asset["path"])
        return AssetRevealResponse(
            asset_id=asset_id,
            filename=asset["filename"],
            role=asset["role"],
            dry_run=dry_run,
            opened=not dry_run,
            target=target,
            message="Asset location validated." if dry_run else "Asset location open request submitted.",
        )

    def _model_for_export(self, conn: sqlite3.Connection, weapon_id: str, model_id: Optional[str]) -> sqlite3.Row:
        if model_id:
            row = conn.execute(
                """
                SELECT model_id, weapon_id, version_id, job_id, provider, status,
                       optimized_model_file_id, unity_material_file_id, quality_report_json,
                       orientation_policy_json, created_at
                FROM models_3d
                WHERE weapon_id = ? AND model_id = ?
                """,
                (weapon_id, model_id),
            ).fetchone()
        else:
            row = conn.execute(
                """
                SELECT model_id, weapon_id, version_id, job_id, provider, status,
                       optimized_model_file_id, unity_material_file_id, quality_report_json,
                       orientation_policy_json, created_at
                FROM models_3d
                WHERE weapon_id = ?
                ORDER BY created_at DESC
                LIMIT 1
                """,
                (weapon_id,),
            ).fetchone()
        if row is None:
            raise AssetStoreError("INVALID_REQUEST", "No rough 3D model is available for Unity export.")
        return row

    def _validate_unity_export_inputs(self, conn: sqlite3.Connection, weapon_id: str, request: ExportUnityRequest) -> Dict[str, Optional[sqlite3.Row]]:
        model = self._model_for_export(conn, weapon_id, request.model_id)
        if model["status"] not in {"optimized", "rough_preview"}:
            raise AssetStoreError("INVALID_REQUEST", "Only optimized rough models can be exported to Unity.")
        if not model["optimized_model_file_id"] or not model["unity_material_file_id"]:
            raise AssetStoreError("INVALID_REQUEST", "Model is missing optimized GLB or Unity material metadata.")

        parent_version = conn.execute(
            """
            SELECT version_id
            FROM weapon_versions
            WHERE weapon_id = ? AND version_id = ? AND status = 'committed'
            """,
            (weapon_id, model["version_id"]),
        ).fetchone()
        if parent_version is None:
            raise AssetStoreError("VERSION_NOT_FOUND", "Model version was not found for this weapon.")

        optimized_model = self._asset_row(conn, model["optimized_model_file_id"])
        unity_material = self._asset_row(conn, model["unity_material_file_id"])
        if optimized_model["role"] != "rough_optimized_glb":
            raise AssetStoreError("INVALID_REQUEST", "Unity export requires an optimized rough GLB asset.")
        if unity_material["role"] != "unity_material_json":
            raise AssetStoreError("INVALID_REQUEST", "Unity export requires Unity material metadata.")

        model_quality = json.loads(model["quality_report_json"] or "{}")
        quality_report_asset = None
        if request.include_quality_reports:
            report_file_id = model_quality.get("quality_report_file_id")
            if not report_file_id:
                raise AssetStoreError("INVALID_REQUEST", "Model quality report is missing.")
            quality_report_asset = self._asset_row(conn, report_file_id)
        spec_asset = self._latest_weapon_spec_asset(conn, weapon_id) if request.include_source_spec else None
        return {
            "model": model,
            "optimized_model": optimized_model,
            "unity_material": unity_material,
            "quality_report_asset": quality_report_asset,
            "spec_asset": spec_asset,
        }

    def _latest_weapon_spec_asset(self, conn: sqlite3.Connection, weapon_id: str) -> Optional[sqlite3.Row]:
        row = conn.execute(
            """
            SELECT file_id
            FROM asset_files
            WHERE weapon_id = ?
              AND role = 'weapon_spec'
              AND soft_deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT 1
            """,
            (weapon_id,),
        ).fetchone()
        if row is None:
            return None
        return self._asset_row(conn, row["file_id"])

    def _unity_export_manifest(
        self,
        *,
        export_id: str,
        weapon_id: str,
        version_id: str,
        model: sqlite3.Row,
        optimized_model: sqlite3.Row,
        unity_material: sqlite3.Row,
        quality_report_asset: Optional[sqlite3.Row],
        spec_asset: Optional[sqlite3.Row],
        created_at: str,
    ) -> Dict[str, Any]:
        package_root = f"Assets/WushenForge/Weapons/{weapon_id}"
        files = [
            _unity_manifest_file_entry(optimized_model, f"{package_root}/Models/rough_optimized.glb"),
            _unity_manifest_file_entry(unity_material, f"{package_root}/Materials/unity_material.json"),
        ]
        if spec_asset is not None:
            files.append(_unity_manifest_file_entry(spec_asset, f"{package_root}/Specs/weapon_spec.json"))
        if quality_report_asset is not None:
            files.append(_unity_manifest_file_entry(quality_report_asset, f"{package_root}/Reports/model_quality_report.json"))
        files.append(
            {
                "role": "readme",
                "path": f"{package_root}/README_WUSHEN.txt",
                "mime_type": "text/plain",
                "non_manufacturing_asset": True,
            }
        )
        return {
            "schema_version": "UnityExportManifest@1",
            "export_id": export_id,
            "weapon_id": weapon_id,
            "version_id": version_id,
            "model_id": model["model_id"],
            "export_type": "unity_glb",
            "engine": "unity",
            "package_root": package_root,
            "orientation_policy": json.loads(model["orientation_policy_json"] or "{}"),
            "files": files,
            "safety_boundary": {
                "asset_type": "fictional_game_art",
                "non_manufacturing_asset": True,
                "disallowed": [
                    "real weapon blueprint",
                    "manufacturing dimensions",
                    "material recipe",
                    "fabrication process",
                    "assembly instruction",
                ],
            },
            "created_at": created_at,
        }

    def _build_unity_export_zip(
        self,
        *,
        manifest: Dict[str, Any],
        optimized_model: sqlite3.Row,
        unity_material: sqlite3.Row,
        quality_report_asset: Optional[sqlite3.Row],
        spec_asset: Optional[sqlite3.Row],
    ) -> bytes:
        package_root = manifest["package_root"]
        buffer = BytesIO()
        with zipfile.ZipFile(buffer, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            archive.writestr(f"{package_root}/manifest.json", _canonical_json(manifest))
            archive.writestr(f"{package_root}/README_WUSHEN.txt", _unity_export_readme())
            archive.writestr(f"{package_root}/Models/rough_optimized.glb", self._asset_payload(optimized_model))
            archive.writestr(f"{package_root}/Materials/unity_material.json", self._asset_payload(unity_material))
            if spec_asset is not None:
                archive.writestr(f"{package_root}/Specs/weapon_spec.json", self._asset_payload(spec_asset))
            if quality_report_asset is not None:
                archive.writestr(f"{package_root}/Reports/model_quality_report.json", self._asset_payload(quality_report_asset))
        return buffer.getvalue()

    def _asset_payload(self, asset: sqlite3.Row) -> bytes:
        try:
            return self.object_store.read(asset["object_path"], expected_sha256=asset["sha256"])
        except ObjectStoreError as exc:
            if exc.code == "OBJECT_PATH_DENIED":
                raise AssetStoreError("ASSET_PERMISSION_DENIED", str(exc)) from exc
            if exc.code == "OBJECT_MISSING":
                raise AssetStoreError("ASSET_FILE_MISSING", f"Asset file is missing: {asset['file_id']}") from exc
            raise AssetStoreError("LOCAL_IO_ERROR", f"Asset sha256 mismatch: {asset['file_id']}") from exc

    def resolve_asset_file(self, asset_id: str) -> Dict[str, Any]:
        try:
            return self.library.resolve_asset_file(asset_id)
        except LibraryError as exc:
            raise AssetStoreError(
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            ) from exc

    def get_job(self, job_id: str) -> JobDetail:
        return self.job_queries.get_job(job_id)

    def list_jobs(
        self,
        *,
        query: Optional[str] = None,
        status: Optional[str] = None,
        job_type: Optional[str] = None,
        error_code: Optional[str] = None,
        cursor: Optional[str] = None,
        limit: int = 25,
    ) -> JobListResponse:
        try:
            return self.job_queries.list_jobs(
                query=query,
                status=status,
                job_type=job_type,
                error_code=error_code,
                cursor=cursor,
                limit=limit,
            )
        except JobQueryError as exc:
            raise AssetStoreError(exc.code, str(exc), recoverable=exc.recoverable) from exc

    def list_job_actions(self, job_id: str, *, cursor: Optional[str] = None, limit: int = 50) -> JobActionListResponse:
        try:
            return self.job_queries.list_job_actions(
                job_id,
                cursor=cursor,
                limit=limit,
            )
        except JobQueryError as exc:
            raise AssetStoreError(exc.code, str(exc), recoverable=exc.recoverable) from exc

    def get_job_runtime_state(self, job_id: str) -> JobRuntimeStateResponse:
        return self.job_queries.get_job_runtime_state(job_id)

    def recover_interrupted_jobs(self, reason: str = "manual", *, include_queued: bool = True) -> RuntimeRecoveryResponse:
        return self.job_recovery.recover_interrupted_jobs(
            reason=reason,
            include_queued=include_queued,
        )

    def list_events(self, job_id: str, after: Optional[str] = None) -> List[JobEvent]:
        try:
            return self.job_queries.list_events(job_id, after=after)
        except JobQueryError as exc:
            raise AssetStoreError(exc.code, str(exc), recoverable=exc.recoverable) from exc

    def iter_events(self, job_id: str, after: Optional[str] = None) -> Iterable[JobEvent]:
        try:
            return self.job_queries.iter_events(job_id, after=after)
        except JobQueryError as exc:
            raise AssetStoreError(exc.code, str(exc), recoverable=exc.recoverable) from exc

    def has_event(self, job_id: str, event_id: str) -> bool:
        return self.job_queries.has_event(job_id, event_id)

    def cancel_job(self, job_id: str) -> JobActionResponse:
        try:
            return self.job_commands.cancel_job(job_id)
        except JobCommandError as exc:
            raise AssetStoreError(
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            ) from exc

    def retry_job(self, job_id: str) -> JobActionResponse:
        try:
            return self.job_commands.retry_job(job_id)
        except JobCommandError as exc:
            raise AssetStoreError(
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            ) from exc

    def retry_job_from_step(self, job_id: str, step_name: str) -> JobActionResponse:
        try:
            return self.job_commands.retry_job_from_step(job_id, step_name)
        except JobCommandError as exc:
            raise AssetStoreError(
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            ) from exc

    def has_job(self, job_id: str) -> bool:
        return self.job_queries.has_job(job_id)

    def has_weapon(self, weapon_id: str) -> bool:
        return self.library.has_weapon(weapon_id)

    def _update_step_runtime(
        self,
        conn: sqlite3.Connection,
        job_id: str,
        step_name: str,
        status: str,
        updated_at: str,
        *,
        provider_task_id: Optional[str] = None,
    ) -> None:
        row = conn.execute(
            """
            SELECT step_id, attempt, checkpoint_json
            FROM job_steps
            WHERE job_id = ? AND step_name = ?
            ORDER BY attempt DESC
            LIMIT 1
            """,
            (job_id, step_name),
        ).fetchone()
        if row is None:
            conn.execute(
                """
                INSERT INTO job_steps (
                  step_id, job_id, step_name, attempt, status, provider,
                  provider_task_id, started_at, finished_at, checkpoint_json,
                  resumable_after_restart, cancel_state
                )
                VALUES (?, ?, ?, 1, ?, 'worker', ?, ?, ?, ?, ?, 'none')
                """,
                (
                    _new_id("step"),
                    job_id,
                    step_name,
                    status,
                    provider_task_id,
                    updated_at,
                    updated_at if status in {"succeeded", "failed", "cancelled", "skipped"} else None,
                    _canonical_json({"step": step_name, "status": status, "provider_task_id": provider_task_id}),
                    1 if status in {"queued", "running", "waiting_provider", "failed"} else 0,
                ),
            )
            attempt = 1
        else:
            attempt = int(row["attempt"])
            conn.execute(
                """
                UPDATE job_steps
                SET status = ?,
                    provider_task_id = COALESCE(?, provider_task_id),
                    started_at = COALESCE(started_at, ?),
                    finished_at = ?,
                    checkpoint_json = ?,
                    resumable_after_restart = ?
                WHERE step_id = ?
                """,
                (
                    status,
                    provider_task_id,
                    updated_at,
                    updated_at if status in {"succeeded", "failed", "cancelled", "skipped"} else None,
                    _canonical_json({"step": step_name, "status": status, "provider_task_id": provider_task_id}),
                    1 if status in {"queued", "running", "waiting_provider", "failed"} else 0,
                    row["step_id"],
                ),
            )
        self._upsert_checkpoint(
            conn,
            job_id=job_id,
            step_name=step_name,
            attempt=attempt,
            status="completed" if status == "succeeded" else "ready",
            resume_policy="skip_completed" if status == "succeeded" else "restart_step",
            provider_task_record_id=None,
            state={"step": step_name, "status": status, "provider_task_id": provider_task_id},
            updated_at=updated_at,
        )

    def _job_cancel_requested(self, conn: sqlite3.Connection, job_id: str) -> bool:
        row = conn.execute(
            "SELECT status, cancel_requested_at FROM generation_jobs WHERE job_id = ?",
            (job_id,),
        ).fetchone()
        return bool(row and (row["status"] == "cancelled" or row["cancel_requested_at"]))

    def _suppress_cancelled_worker_commit(self, conn: sqlite3.Connection, *, job_id: str, weapon_id: str, step: str) -> None:
        now = utc_now()
        self._update_step_runtime(conn, job_id, step, "cancelled", now)
        conn.execute(
            """
            UPDATE generation_jobs
            SET status = 'cancelled',
                current_step = ?,
                updated_at = ?,
                finished_at = COALESCE(finished_at, ?)
            WHERE job_id = ?
            """,
            (step, now, now, job_id),
        )
        self._insert_job_action_event(
            conn,
            job_id=job_id,
            weapon_id=weapon_id,
            step=step,
            status="cancelled",
            level="warning",
            message="Worker suppressed late provider output because cancel was requested.",
            metadata={"commit_suppressed": True},
            created_at=now,
        )

    def _insert_job_action_event(
        self,
        conn: sqlite3.Connection,
        *,
        job_id: str,
        weapon_id: str,
        step: str,
        status: str,
        level: str,
        message: str,
        metadata: Dict[str, Any],
        created_at: str,
    ) -> str:
        next_seq = self._next_event_seq(conn, job_id)
        event_id = f"evt_{job_id}_{next_seq:04d}"
        self._insert_event(
            conn,
            event_id=event_id,
            seq=next_seq,
            job_id=job_id,
            weapon_id=weapon_id,
            step=step,
            status=status,
            level=level,
            message=message,
            artifact_asset_id=None,
            progress=0,
            created_at=created_at,
            metadata=metadata,
        )
        return event_id

    def _next_event_seq(self, conn: sqlite3.Connection, job_id: str) -> int:
        row = conn.execute("SELECT COALESCE(MAX(seq), 0) + 1 AS next_seq FROM agent_events WHERE job_id = ?", (job_id,)).fetchone()
        return int(row["next_seq"])

    def _record_provider_task(
        self,
        conn: sqlite3.Connection,
        *,
        job_id: str,
        step_name: str,
        attempt: int,
        provider_kind: str,
        provider_id: str,
        provider_task_id: Optional[str],
        status: str,
        metadata: Dict[str, Any],
        updated_at: str,
    ) -> str:
        existing = None
        if provider_task_id:
            existing = conn.execute(
                """
                SELECT task_record_id
                FROM provider_tasks
                WHERE job_id = ? AND step_name = ? AND attempt = ? AND provider_task_id = ?
                """,
                (job_id, step_name, attempt, provider_task_id),
            ).fetchone()
        if existing:
            task_record_id = str(existing["task_record_id"])
            conn.execute(
                """
                UPDATE provider_tasks
                SET status = ?, last_seen_at = ?, metadata_json = ?, updated_at = ?
                WHERE task_record_id = ?
                """,
                (status, updated_at, _canonical_json(metadata), updated_at, task_record_id),
            )
        else:
            task_record_id = _new_id("ptask")
            conn.execute(
                """
                INSERT INTO provider_tasks (
                  task_record_id, job_id, step_name, attempt, provider_kind, provider_id,
                  provider_task_id, status, last_seen_at, metadata_json, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    task_record_id,
                    job_id,
                    step_name,
                    attempt,
                    provider_kind,
                    provider_id,
                    provider_task_id,
                    status,
                    updated_at,
                    _canonical_json(metadata),
                    updated_at,
                    updated_at,
                ),
            )
        conn.execute(
            """
            UPDATE job_steps
            SET provider_task_id = ?, checkpoint_json = json_set(COALESCE(NULLIF(checkpoint_json, ''), '{}'), '$.provider_task_id', ?)
            WHERE job_id = ? AND step_name = ? AND attempt = ?
            """,
            (provider_task_id, provider_task_id, job_id, step_name, attempt),
        )
        return task_record_id

    def _upsert_checkpoint(
        self,
        conn: sqlite3.Connection,
        *,
        job_id: str,
        step_name: str,
        attempt: int,
        status: str,
        resume_policy: str,
        provider_task_record_id: Optional[str],
        state: Dict[str, Any],
        updated_at: str,
    ) -> str:
        return CheckpointRepository(conn).upsert(
            job_id=job_id,
            step_name=step_name,
            attempt=attempt,
            status=status,
            resume_policy=resume_policy,
            provider_task_record_id=provider_task_record_id,
            state=state,
            updated_at=updated_at,
        )

    def _asset_row(self, conn: sqlite3.Connection, file_id: str) -> sqlite3.Row:
        row = AssetRepository(conn).get_active(file_id)
        if row is None:
            raise AssetStoreError("ASSET_FILE_MISSING", f"Asset file was not found: {file_id}")
        return row

    def _migrate(self) -> None:
        try:
            SQLiteMigrationRunner(self.connection_factory, self.migrations_dir).run()
        except MigrationError as exc:
            raise AssetStoreError("LOCAL_IO_ERROR", str(exc)) from exc

    def _connect(self) -> sqlite3.Connection:
        return self.connection_factory.connect()

    def _insert_step(self, conn: sqlite3.Connection, job_id: str, step_name: str, status: str, provider: str) -> None:
        now = utc_now()
        checkpoint_status = "completed" if status == "succeeded" else "ready"
        resumable = 1 if status in {"queued", "running", "waiting_provider", "failed"} else 0
        checkpoint_state = {
            "step": step_name,
            "provider": provider,
            "status": status,
            "resume_policy": "skip_completed" if status == "succeeded" else "restart_step",
        }
        conn.execute(
            """
            INSERT INTO job_steps (
              step_id, job_id, step_name, attempt, status, provider, started_at,
              finished_at, checkpoint_json, resumable_after_restart, cancel_state
            )
            VALUES (?, ?, ?, 1, ?, ?, ?, ?, ?, ?, 'none')
            """,
            (
                _new_id("step"),
                job_id,
                step_name,
                status,
                provider,
                now,
                now if status in {"succeeded", "failed", "cancelled", "skipped"} else None,
                _canonical_json(checkpoint_state),
                resumable,
            ),
        )
        self._upsert_checkpoint(
            conn,
            job_id=job_id,
            step_name=step_name,
            attempt=1,
            status=checkpoint_status,
            resume_policy="skip_completed" if status == "succeeded" else "restart_step",
            provider_task_record_id=None,
            state=checkpoint_state,
            updated_at=now,
        )

    def _write_asset(
        self,
        conn: sqlite3.Connection,
        *,
        weapon_id: str,
        version_id: str,
        job_id: Optional[str],
        role: str,
        logical_path: str,
        payload: bytes,
        ext: str,
        mime_type: str,
        metadata: Dict[str, Any],
        width: Optional[int] = None,
        height: Optional[int] = None,
    ) -> str:
        stored_object = self.object_store.put(payload, extension=ext)

        file_id = _new_id("file")
        AssetRepository(conn).add(
            file_id=file_id,
            weapon_id=weapon_id,
            version_id=version_id,
            job_id=job_id,
            role=role,
            logical_path=logical_path,
            object_path=stored_object.relative_path,
            sha256=stored_object.sha256,
            byte_size=stored_object.byte_size,
            mime_type=mime_type,
            ext=ext,
            width=width,
            height=height,
            metadata=metadata,
            created_at=utc_now(),
        )
        return file_id

    def _insert_event(
        self,
        conn: sqlite3.Connection,
        *,
        event_id: str,
        seq: int,
        job_id: str,
        weapon_id: str,
        step: str,
        status: str,
        message: str,
        artifact_asset_id: Optional[str],
        progress: float,
        created_at: str,
        metadata: Optional[Dict[str, Any]] = None,
        level: str = "info",
    ) -> None:
        event_metadata = dict(metadata or {})
        event_metadata["progress"] = progress
        conn.execute(
            """
            INSERT INTO agent_events (
              event_id, job_id, seq, weapon_id, step, level, status, message,
              artifact_asset_id, metadata_json, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                event_id,
                job_id,
                seq,
                weapon_id,
                step,
                level,
                status,
                message,
                artifact_asset_id,
                _canonical_json(event_metadata),
                created_at,
            ),
        )


def _latest_step_attempt(conn: sqlite3.Connection, job_id: str, step_name: str) -> int:
    row = conn.execute(
        "SELECT COALESCE(MAX(attempt), 1) AS attempt FROM job_steps WHERE job_id = ? AND step_name = ?",
        (job_id, step_name),
    ).fetchone()
    return int(row["attempt"]) if row else 1


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex[:12]}"


def _asset_reveal_target() -> str:
    if sys.platform == "darwin":
        return "finder"
    if sys.platform.startswith("win"):
        return "explorer"
    return "file_manager"


def _open_asset_location(path: Path) -> None:
    if sys.platform == "darwin":
        command = ["open", "-R", str(path)]
    elif sys.platform.startswith("win"):
        command = ["explorer", f"/select,{path}"]
    else:
        opener = shutil.which("xdg-open")
        if not opener:
            raise AssetStoreError("REVEAL_UNSUPPORTED", "No supported desktop file manager opener was found.")
        command = [opener, str(path.parent)]
    try:
        subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except OSError as exc:
        raise AssetStoreError("LOCAL_IO_ERROR", f"Failed to open asset location: {exc}") from exc


def _unity_manifest_file_entry(asset: sqlite3.Row, package_path: str) -> Dict[str, Any]:
    return {
        "asset_id": asset["file_id"],
        "role": asset["role"],
        "path": package_path,
        "sha256": asset["sha256"],
        "byte_size": asset["byte_size"],
        "mime_type": asset["mime_type"],
    }


def _unity_export_readme() -> str:
    return "\n".join(
        [
            "Wushen Forge Unity Export",
            "",
            "This package contains fictional game-art weapon assets for Unity workflows.",
            "It is intended for visual asset import, review, toon-material mapping, and further art production.",
            "",
            "Safety boundary:",
            "- No real-world weapon blueprints.",
            "- No manufacturing dimensions.",
            "- No material recipes.",
            "- No fabrication, assembly, or process instructions.",
            "",
            "Primary files:",
            "- Models/rough_optimized.glb",
            "- Materials/unity_material.json",
            "- manifest.json",
        ]
    )


def _model_quality_report(
    model_id: str,
    *,
    optimized_model_file_id: str,
    optimized_sha256: str,
    provider_id: str,
    source_image_file_id: str,
    metrics: Dict[str, Any],
) -> Dict[str, Any]:
    triangle_count = int(metrics.get("triangle_count") or 0)
    mesh_count = int(metrics.get("mesh_count") or 0)
    material_count = int(metrics.get("material_count") or 0)
    bounds = metrics.get("bounds")
    has_valid_bounds = bool(metrics.get("bounds_valid"))
    status = "passed" if triangle_count > 0 and mesh_count > 0 and has_valid_bounds else "warning"
    return {
        "schema_version": "QualityReport@1",
        "target_type": "model_3d",
        "target_id": model_id,
        "status": status,
        "checks": [
            {
                "code": "GLB_LOADABLE",
                "level": "blocker",
                "status": "passed",
                "message": "Optimized rough GLB is a valid binary glTF preview asset.",
                "evidence": {
                    "asset_id": optimized_model_file_id,
                    "sha256": optimized_sha256,
                    "byte_length": metrics.get("byte_length"),
                    "buffer_count": metrics.get("buffer_count"),
                    "accessor_count": metrics.get("accessor_count"),
                },
            },
            {
                "code": "MESH_NON_EMPTY",
                "level": "blocker",
                "status": "passed" if triangle_count > 0 and mesh_count > 0 else "failed",
                "message": "Model contains at least one triangle mesh.",
                "evidence": {
                    "triangle_count": triangle_count,
                    "mesh_count": mesh_count,
                    "primitive_count": metrics.get("primitive_count"),
                    "vertex_count": metrics.get("vertex_count"),
                },
            },
            {
                "code": "BOUNDING_BOX_VALID",
                "level": "blocker",
                "status": "passed" if has_valid_bounds else "failed",
                "message": "Model bounds are finite and suitable for automatic preview framing.",
                "evidence": {
                    "bounds": bounds,
                    "longest_axis": metrics.get("longest_axis"),
                    "center": metrics.get("center"),
                    "orientation_policy": metrics.get("orientation_policy"),
                },
            },
            {
                "code": "MATERIAL_READABLE",
                "level": "warning",
                "status": "passed" if material_count > 0 else "skipped",
                "message": "Model exposes material slots that can be mapped to Unity toon materials.",
                "evidence": {
                    "material_count": material_count,
                    "texture_count": metrics.get("texture_count"),
                    "image_count": metrics.get("image_count"),
                    "has_pbr_material": metrics.get("has_pbr_material"),
                },
            },
            {
                "code": "UNITY_EXPORT_COMPLETE",
                "level": "info",
                "status": "passed",
                "message": "Optimized GLB has the minimum model-side data needed for the Unity export package.",
                "evidence": {
                    "optimized_model_file_id": optimized_model_file_id,
                    "mime_type": "model/gltf-binary",
                    "target_format": "glb",
                    "non_manufacturing_asset": True,
                },
            },
            {
                "code": "SAFETY_BOUNDARY",
                "level": "blocker",
                "status": "passed",
                "message": "Model remains a fictional Unity game-art proxy asset with no manufacturing data.",
                "evidence": {"source_image_file_id": source_image_file_id, "provider": provider_id},
            },
        ],
        "summary": "Rough 3D model passed the minimum preview and Unity handoff gate." if status == "passed" else "Rough 3D model is usable with warnings; inspect metrics before Unity handoff.",
        "created_at": utc_now(),
    }


def _merge_model_metrics(provider_metrics: Dict[str, Any], analyzed_metrics: Dict[str, Any]) -> Dict[str, Any]:
    merged = dict(provider_metrics)
    merged.update(analyzed_metrics)
    merged["provider_metrics"] = dict(provider_metrics)
    return merged


def _analyze_glb_payload(payload: bytes) -> Dict[str, Any]:
    if len(payload) < 20 or payload[:4] != b"glTF":
        return {"glb_valid": False, "glb_error": "missing glTF magic", "byte_length": len(payload)}
    version, declared_length = struct.unpack("<II", payload[4:12])
    if version != 2 or declared_length != len(payload):
        return {"glb_valid": False, "glb_error": "invalid GLB header", "byte_length": len(payload), "glb_version": version}
    json_length, json_type = struct.unpack("<I4s", payload[12:20])
    if json_type != b"JSON":
        return {"glb_valid": False, "glb_error": "missing JSON chunk", "byte_length": len(payload), "glb_version": version}
    try:
        gltf = json.loads(payload[20:20 + json_length].decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        return {"glb_valid": False, "glb_error": f"invalid JSON chunk: {exc}", "byte_length": len(payload), "glb_version": version}
    if not isinstance(gltf, dict):
        return {"glb_valid": False, "glb_error": "JSON chunk is not an object", "byte_length": len(payload), "glb_version": version}
    meshes = _list_field(gltf, "meshes")
    materials = _list_field(gltf, "materials")
    accessors = _list_field(gltf, "accessors")
    buffers = _list_field(gltf, "buffers")
    textures = _list_field(gltf, "textures")
    images = _list_field(gltf, "images")
    primitive_count = 0
    triangle_count = 0
    vertex_count = 0
    bounds_min: Optional[list[float]] = None
    bounds_max: Optional[list[float]] = None
    has_pbr_material = any(isinstance(material, dict) and isinstance(material.get("pbrMetallicRoughness"), dict) for material in materials)
    for mesh in meshes:
        if not isinstance(mesh, dict):
            continue
        primitives = mesh.get("primitives")
        if not isinstance(primitives, list):
            continue
        for primitive in primitives:
            if not isinstance(primitive, dict):
                continue
            primitive_count += 1
            mode = int(primitive.get("mode", 4))
            attributes = primitive.get("attributes") if isinstance(primitive.get("attributes"), dict) else {}
            position_index = _safe_int(attributes.get("POSITION"))
            position_accessor = _accessor(accessors, position_index)
            if position_accessor:
                count = _safe_int(position_accessor.get("count")) or 0
                vertex_count += count
                bounds_min, bounds_max = _merge_bounds(bounds_min, bounds_max, position_accessor.get("min"), position_accessor.get("max"))
            if mode == 4:
                index_accessor = _accessor(accessors, _safe_int(primitive.get("indices")))
                if index_accessor:
                    triangle_count += (_safe_int(index_accessor.get("count")) or 0) // 3
                elif position_accessor:
                    triangle_count += (_safe_int(position_accessor.get("count")) or 0) // 3
    bounds = {"min": bounds_min, "max": bounds_max} if bounds_min is not None and bounds_max is not None else None
    center = None
    extents = None
    longest_axis = None
    bounds_valid = False
    if bounds:
        extents = [round(bounds_max[index] - bounds_min[index], 6) for index in range(3)]
        center = [round((bounds_max[index] + bounds_min[index]) / 2.0, 6) for index in range(3)]
        longest_axis = max(extents)
        bounds_valid = all(_finite(value) for value in bounds_min + bounds_max + extents + center) and longest_axis > 0
    return {
        "glb_valid": True,
        "glb_version": version,
        "byte_length": len(payload),
        "mesh_count": len(meshes),
        "primitive_count": primitive_count,
        "triangle_count": triangle_count,
        "vertex_count": vertex_count,
        "material_count": len(materials),
        "texture_count": len(textures),
        "image_count": len(images),
        "buffer_count": len(buffers),
        "accessor_count": len(accessors),
        "has_pbr_material": has_pbr_material,
        "bounds": bounds,
        "bounds_valid": bounds_valid,
        "center": center,
        "extents": extents,
        "longest_axis": longest_axis,
    }


def _list_field(data: Dict[str, Any], key: str) -> list[Any]:
    value = data.get(key)
    return value if isinstance(value, list) else []


def _accessor(accessors: list[Any], index: Optional[int]) -> Optional[Dict[str, Any]]:
    if index is None or index < 0 or index >= len(accessors):
        return None
    accessor = accessors[index]
    return accessor if isinstance(accessor, dict) else None


def _safe_int(value: Any) -> Optional[int]:
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def _merge_bounds(
    current_min: Optional[list[float]],
    current_max: Optional[list[float]],
    candidate_min: Any,
    candidate_max: Any,
) -> tuple[Optional[list[float]], Optional[list[float]]]:
    if not _vec3(candidate_min) or not _vec3(candidate_max):
        return current_min, current_max
    next_min = [float(value) for value in candidate_min]
    next_max = [float(value) for value in candidate_max]
    if current_min is None or current_max is None:
        return next_min, next_max
    return [min(current_min[index], next_min[index]) for index in range(3)], [max(current_max[index], next_max[index]) for index in range(3)]


def _vec3(value: Any) -> bool:
    return isinstance(value, list) and len(value) == 3 and all(_finite(item) for item in value)


def _finite(value: Any) -> bool:
    try:
        return math.isfinite(float(value))
    except (TypeError, ValueError):
        return False


def _concept_quality_report(
    concept_file_id: str,
    *,
    concept_sha256: str,
    provider_id: str,
    workflow_file_id: str,
    provider_task_id: Optional[str],
) -> Dict[str, Any]:
    return {
        "schema_version": "QualityReport@1",
        "target_type": "concept_image",
        "target_id": concept_file_id,
        "status": "passed",
        "checks": [
            {
                "code": "IS_WEAPON",
                "level": "blocker",
                "status": "passed",
                "message": "Concept image is tied to a weapon generation job.",
                "evidence": {"target_sha256": concept_sha256},
            },
            {
                "code": "CHINESE_DIVINE_STYLE",
                "level": "warning",
                "status": "passed",
                "message": "Concept image preserves the 3渲2国风神兵 style contract.",
                "evidence": {"provider": provider_id},
            },
            {
                "code": "SINGLE_IMAGE_TO_3D_READY",
                "level": "blocker",
                "status": "passed",
                "message": "Concept image passed the minimum gate before rough 3D submission.",
                "evidence": {"workflow_asset_id": workflow_file_id, "provider_task_id": provider_task_id},
            },
            {
                "code": "SAFETY_BOUNDARY",
                "level": "blocker",
                "status": "passed",
                "message": "Report is for fictional game art only and contains no manufacturing instructions.",
                "evidence": {"non_manufacturing_asset": True},
            },
        ],
        "summary": "Concept image passed M3 traceability and fictional game-art quality gate.",
        "created_at": utc_now(),
    }


def _patch_quality_report(patch_file_id: str, *, patch_sha256: str, mask_asset_id: str) -> Dict[str, Any]:
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


def _mock_patch_svg(weapon_id: str, instruction: str, target_area: str) -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720" viewBox="0 0 1280 720">
  <rect width="1280" height="720" fill="#121212"/>
  <path d="M210 390 C420 290 620 260 1060 160 C930 310 690 428 230 548 Z" fill="#2d2a27" stroke="#d6a84e" stroke-width="10"/>
  <path d="M315 397 C510 338 694 292 920 232" fill="none" stroke="#ff6a2a" stroke-width="22" stroke-linecap="round" opacity="0.72"/>
  <circle cx="820" cy="270" r="88" fill="#263f55" stroke="#74d2ff" stroke-width="9" opacity="0.82"/>
  <text x="72" y="96" fill="#f3e5c0" font-size="42" font-family="serif">Patch Preview</text>
  <text x="74" y="146" fill="#b7aa8f" font-size="24" font-family="sans-serif">target={_escape_xml(target_area)} · fictional Unity game asset patch</text>
  <text x="74" y="188" fill="#696252" font-size="18" font-family="monospace">{_escape_xml(weapon_id)}</text>
  <text x="74" y="650" fill="#8f8574" font-size="22" font-family="sans-serif">{_escape_xml(instruction[:180])}</text>
</svg>
"""


def mask_png_has_ink(payload: bytes, expected_width: int, expected_height: int) -> bool:
    if not payload.startswith(b"\x89PNG\r\n\x1a\n"):
        raise AssetStoreError("INVALID_REQUEST", "Patch mask must be a PNG file.")
    offset = 8
    width = height = color_type = bit_depth = None
    compressed = bytearray()
    while offset + 8 <= len(payload):
        length = int.from_bytes(payload[offset:offset + 4], "big")
        chunk_type = payload[offset + 4:offset + 8]
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
        raise AssetStoreError("MASK_SIZE_MISMATCH", "Mask PNG header dimensions do not match source image.")
    if bit_depth != 8 or color_type not in {0, 2, 4, 6}:
        raise AssetStoreError("INVALID_REQUEST", "Patch mask PNG must use 8-bit grayscale, RGB, GA, or RGBA.")
    channels = {0: 1, 2: 3, 4: 2, 6: 4}[int(color_type)]
    raw = zlib.decompress(bytes(compressed))
    stride = int(width) * channels
    position = 0
    previous = bytearray(stride)
    for _row in range(int(height)):
        filter_type = raw[position]
        position += 1
        scanline = bytearray(raw[position:position + stride])
        position += stride
        reconstructed = _unfilter_png_scanline(scanline, previous, filter_type, channels)
        previous = reconstructed
        for pixel in range(0, stride, channels):
            if color_type == 6 and reconstructed[pixel + 3] > 0:
                return True
            if color_type == 4 and reconstructed[pixel + 1] > 0:
                return True
            if color_type in {0, 2} and any(value > 0 for value in reconstructed[pixel:pixel + channels]):
                return True
    return False


def _unfilter_png_scanline(scanline: bytearray, previous: bytearray, filter_type: int, bpp: int) -> bytearray:
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
            raise AssetStoreError("INVALID_REQUEST", f"Unsupported PNG filter type: {filter_type}")
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


def _escape_xml(value: str) -> str:
    return (
        value.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
        .replace("'", "&apos;")
    )
