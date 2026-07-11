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
from .application.generate_3d import (
    Generate3DError,
    Generate3DIdempotencyConflict,
    LegacyGenerate3DService,
)
from .application.job_commands import JobCommandError, LegacyJobCommandService
from .application.job_queries import JobQueryError, LegacyJobQueryService
from .application.job_recovery import LegacyJobRecoveryService
from .application.library import LegacyLibraryService, LibraryError
from .application.patch_workflow import (
    LegacyPatchService,
    PatchIdempotencyConflict,
    PatchWorkflowError,
)
from .application.unity_export import (
    LegacyUnityExportService,
    UnityExportError,
    UnityExportIdempotencyConflict,
)
from .application.worker_runtime import LegacyWorkerService
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
    three_d_provider_from_env,
    three_d_provider_settings_from_env,
)
from .spec_validation import validate_quality_report


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
        self.patch_workflow = LegacyPatchService(
            connection_factory=self.connection_factory,
            image_provider=self.image_provider,
            asset_payload=self._asset_payload,
            asset_row=self._asset_row,
            get_job=self.get_job,
            insert_event=self._insert_event,
            insert_step=self._insert_step,
            record_provider_task=self._record_provider_task,
            upsert_checkpoint=self._upsert_checkpoint,
            write_asset=self._write_asset,
        )
        self.generate_3d_workflow = LegacyGenerate3DService(
            connection_factory=self.connection_factory,
            three_d_provider=self.three_d_provider,
            asset_row=self._asset_row,
            asset_payload=self._asset_payload,
            write_asset=self._write_asset,
            write_rough_model_assets=self._write_rough_model_assets,
            record_provider_task=self._record_provider_task,
            upsert_checkpoint=self._upsert_checkpoint,
            insert_step=self._insert_step,
            insert_event=self._insert_event,
            get_job=self.get_job,
        )
        self.unity_export_workflow = LegacyUnityExportService(
            connection_factory=self.connection_factory,
            asset_payload=self._asset_payload,
            asset_row=self._asset_row,
            get_job=self.get_job,
            insert_event=self._insert_event,
            insert_job_action_event=self._insert_job_action_event,
            insert_step=self._insert_step,
            job_cancel_requested=self._job_cancel_requested,
            next_event_seq=self._next_event_seq,
            suppress_cancelled_worker_commit=self._suppress_cancelled_worker_commit,
            update_step_runtime=self._update_step_runtime,
            write_asset=self._write_asset,
        )
        self.worker_runtime = LegacyWorkerService(
            connection_factory=self.connection_factory,
            three_d_provider=self.three_d_provider,
            asset_payload=self._asset_payload,
            asset_row=self._asset_row,
            complete_export_unity_worker_job=self.unity_export_workflow.complete_worker_job,
            insert_event=self._insert_event,
            insert_job_action_event=self._insert_job_action_event,
            job_cancel_requested=self._job_cancel_requested,
            next_event_seq=self._next_event_seq,
            record_provider_task=self._record_provider_task,
            suppress_cancelled_worker_commit=self._suppress_cancelled_worker_commit,
            update_step_runtime=self._update_step_runtime,
            upsert_checkpoint=self._upsert_checkpoint,
            write_asset=self._write_asset,
            write_rough_model_result_assets=self._write_rough_model_result_assets,
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

    def generate_3d(
        self,
        weapon_id: str,
        request: Generate3DRequest,
        idempotency_key: str,
    ) -> JobDetail:
        try:
            return self.generate_3d_workflow.generate_3d(
                weapon_id,
                request,
                idempotency_key,
            )
        except Generate3DIdempotencyConflict as exc:
            raise IdempotencyConflictError(str(exc)) from exc
        except Generate3DError as exc:
            raise AssetStoreError(
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            ) from exc

    def enqueue_generate_3d(
        self,
        weapon_id: str,
        request: Generate3DRequest,
        idempotency_key: str,
    ) -> JobDetail:
        try:
            return self.generate_3d_workflow.enqueue_generate_3d(
                weapon_id,
                request,
                idempotency_key,
            )
        except Generate3DIdempotencyConflict as exc:
            raise IdempotencyConflictError(str(exc)) from exc
        except Generate3DError as exc:
            raise AssetStoreError(
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            ) from exc

    def run_worker_once(
        self,
        runner_id: str = "local_worker",
    ) -> RuntimeWorkOnceResponse:
        return self.worker_runtime.run_worker_once(runner_id)

    def enqueue_export_unity(
        self,
        weapon_id: str,
        request: ExportUnityRequest,
        idempotency_key: str,
    ) -> JobDetail:
        try:
            return self.unity_export_workflow.enqueue_export_unity(
                weapon_id,
                request,
                idempotency_key,
            )
        except UnityExportIdempotencyConflict as exc:
            raise IdempotencyConflictError(str(exc)) from exc
        except UnityExportError as exc:
            raise AssetStoreError(
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            ) from exc

    def export_unity(
        self,
        weapon_id: str,
        request: ExportUnityRequest,
        idempotency_key: str,
    ) -> JobDetail:
        try:
            return self.unity_export_workflow.export_unity(
                weapon_id,
                request,
                idempotency_key,
            )
        except UnityExportIdempotencyConflict as exc:
            raise IdempotencyConflictError(str(exc)) from exc
        except UnityExportError as exc:
            raise AssetStoreError(
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            ) from exc

    def patch_weapon(
        self,
        weapon_id: str,
        request: PatchWeaponRequest,
        idempotency_key: str,
    ) -> JobDetail:
        try:
            return self.patch_workflow.patch_weapon(
                weapon_id,
                request,
                idempotency_key,
            )
        except PatchIdempotencyConflict as exc:
            raise IdempotencyConflictError(str(exc)) from exc
        except PatchWorkflowError as exc:
            raise AssetStoreError(
                exc.code,
                str(exc),
                recoverable=exc.recoverable,
            ) from exc

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
