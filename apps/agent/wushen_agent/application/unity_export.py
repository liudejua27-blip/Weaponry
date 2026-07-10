from __future__ import annotations

import hashlib
import json
import os
import sqlite3
import uuid
import zipfile
from io import BytesIO
from typing import Any, Callable, Dict, Optional

from forgecad_agent.infrastructure.db import SQLiteConnectionFactory

from ..models import ExportUnityRequest, JobDetail, utc_now


class UnityExportIdempotencyConflict(RuntimeError):
    pass


class UnityExportError(RuntimeError):
    def __init__(self, code: str, message: str, *, recoverable: bool = False) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable


class LegacyUnityExportService:
    """Owns legacy Unity sync, queue, worker, manifest, and ZIP workflows."""

    def __init__(
        self,
        *,
        connection_factory: SQLiteConnectionFactory,
        asset_payload: Callable[[sqlite3.Row], bytes],
        asset_row: Callable[..., sqlite3.Row],
        get_job: Callable[[str], JobDetail],
        insert_event: Callable[..., None],
        insert_job_action_event: Callable[..., None],
        insert_step: Callable[..., None],
        job_cancel_requested: Callable[..., bool],
        next_event_seq: Callable[..., int],
        suppress_cancelled_worker_commit: Callable[..., None],
        update_step_runtime: Callable[..., None],
        write_asset: Callable[..., str],
    ) -> None:
        self._connect = connection_factory.connect
        self._asset_payload = asset_payload
        self._asset_row = asset_row
        self.get_job = get_job
        self._insert_event = insert_event
        self._insert_job_action_event = insert_job_action_event
        self._insert_step = insert_step
        self._job_cancel_requested = job_cancel_requested
        self._next_event_seq = next_event_seq
        self._suppress_cancelled_worker_commit = suppress_cancelled_worker_commit
        self._update_step_runtime = update_step_runtime
        self._write_asset = write_asset

    def enqueue_export_unity(
        self, weapon_id: str, request: ExportUnityRequest, idempotency_key: str
    ) -> JobDetail:
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
                    raise UnityExportIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
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
                metadata={
                    "runtime": "worker",
                    "model_id": model["model_id"],
                    "source_version_id": model["version_id"],
                },
            )
            conn.commit()
            return self.get_job(job_id)

    def complete_worker_job(
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
            raise UnityExportError("JOB_NOT_FOUND", f"Job was not found: {job_id}")
        if job["status"] == "cancelled":
            return "cancelled"

        inputs = self._validate_unity_export_inputs(conn, weapon_id, request)
        model = inputs["model"]
        optimized_model = inputs["optimized_model"]
        unity_material = inputs["unity_material"]
        quality_report_asset = inputs["quality_report_asset"]
        spec_asset = inputs["spec_asset"]
        if model is None or optimized_model is None or unity_material is None:
            raise UnityExportError("INVALID_REQUEST", "Unity export inputs could not be resolved.")

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
            self._suppress_cancelled_worker_commit(
                conn, job_id=job_id, weapon_id=weapon_id, step="export_plan"
            )
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
            self._suppress_cancelled_worker_commit(
                conn, job_id=job_id, weapon_id=weapon_id, step="export_manifest"
            )
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
            (
                version_id,
                weapon_id,
                model["version_id"],
                job_id,
                version_no,
                "Unity GLB export package snapshot from local worker.",
                package_at,
            ),
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
        package_sha = conn.execute(
            "SELECT sha256 FROM asset_files WHERE file_id = ?", (package_asset_id,)
        ).fetchone()["sha256"]
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
            (
                "export_manifest",
                "succeeded",
                "Unity export manifest built with relative package paths only.",
                package_asset_id,
                0.55,
                {"export_id": export_id},
            ),
            (
                "export_package",
                "succeeded",
                "Unity GLB export package committed to the asset library by worker.",
                package_asset_id,
                0.9,
                {"package_sha256": package_sha},
            ),
            (
                "finalize_job",
                "succeeded",
                "Unity export worker job completed for fictional game-art handoff.",
                package_asset_id,
                1.0,
                {"export_id": export_id, "new_version_id": version_id},
            ),
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

    def export_unity(
        self, weapon_id: str, request: ExportUnityRequest, idempotency_key: str
    ) -> JobDetail:
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
                    raise UnityExportIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return self.get_job(existing["job_id"])

            model = self._model_for_export(conn, weapon_id, request.model_id)
            if model["status"] not in {"optimized", "rough_preview"}:
                raise UnityExportError(
                    "INVALID_REQUEST", "Only optimized rough models can be exported to Unity."
                )
            if not model["optimized_model_file_id"] or not model["unity_material_file_id"]:
                raise UnityExportError(
                    "INVALID_REQUEST", "Model is missing optimized GLB or Unity material metadata."
                )

            parent_version = conn.execute(
                """
                SELECT version_id
                FROM weapon_versions
                WHERE weapon_id = ? AND version_id = ? AND status = 'committed'
                """,
                (weapon_id, model["version_id"]),
            ).fetchone()
            if parent_version is None:
                raise UnityExportError(
                    "VERSION_NOT_FOUND", "Model version was not found for this weapon."
                )

            optimized_model = self._asset_row(conn, model["optimized_model_file_id"])
            unity_material = self._asset_row(conn, model["unity_material_file_id"])
            if optimized_model["role"] != "rough_optimized_glb":
                raise UnityExportError(
                    "INVALID_REQUEST", "Unity export requires an optimized rough GLB asset."
                )
            if unity_material["role"] != "unity_material_json":
                raise UnityExportError(
                    "INVALID_REQUEST", "Unity export requires Unity material metadata."
                )

            model_quality = json.loads(model["quality_report_json"] or "{}")
            quality_report_asset = None
            if request.include_quality_reports:
                report_file_id = model_quality.get("quality_report_file_id")
                if not report_file_id:
                    raise UnityExportError("INVALID_REQUEST", "Model quality report is missing.")
                quality_report_asset = self._asset_row(conn, report_file_id)
            spec_asset = (
                self._latest_weapon_spec_asset(conn, weapon_id)
                if request.include_source_spec
                else None
            )

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
                VALUES (?, ?, ?, ?, ?, 'export', 'committed', ?, ?)
                """,
                (
                    version_id,
                    weapon_id,
                    model["version_id"],
                    job_id,
                    version_no,
                    "Unity GLB export package snapshot.",
                    now,
                ),
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
            package_sha = conn.execute(
                "SELECT sha256 FROM asset_files WHERE file_id = ?", (package_asset_id,)
            ).fetchone()["sha256"]
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
                (
                    "export_plan",
                    "succeeded",
                    "Unity export inputs resolved from the optimized rough model.",
                    optimized_model["file_id"],
                    0.25,
                    {"model_id": model["model_id"]},
                ),
                (
                    "export_manifest",
                    "succeeded",
                    "Unity export manifest built with relative package paths only.",
                    package_asset_id,
                    0.55,
                    {"export_id": export_id},
                ),
                (
                    "export_package",
                    "succeeded",
                    "Unity GLB export package committed to the asset library.",
                    package_asset_id,
                    0.9,
                    {"package_sha256": package_sha},
                ),
                (
                    "finalize_job",
                    "succeeded",
                    "Unity export package validated for fictional game-art handoff.",
                    package_asset_id,
                    1.0,
                    {"export_id": export_id, "new_version_id": version_id},
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

    def _model_for_export(
        self, conn: sqlite3.Connection, weapon_id: str, model_id: Optional[str]
    ) -> sqlite3.Row:
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
            raise UnityExportError(
                "INVALID_REQUEST", "No rough 3D model is available for Unity export."
            )
        return row

    def _validate_unity_export_inputs(
        self, conn: sqlite3.Connection, weapon_id: str, request: ExportUnityRequest
    ) -> Dict[str, Optional[sqlite3.Row]]:
        model = self._model_for_export(conn, weapon_id, request.model_id)
        if model["status"] not in {"optimized", "rough_preview"}:
            raise UnityExportError(
                "INVALID_REQUEST", "Only optimized rough models can be exported to Unity."
            )
        if not model["optimized_model_file_id"] or not model["unity_material_file_id"]:
            raise UnityExportError(
                "INVALID_REQUEST", "Model is missing optimized GLB or Unity material metadata."
            )

        parent_version = conn.execute(
            """
            SELECT version_id
            FROM weapon_versions
            WHERE weapon_id = ? AND version_id = ? AND status = 'committed'
            """,
            (weapon_id, model["version_id"]),
        ).fetchone()
        if parent_version is None:
            raise UnityExportError(
                "VERSION_NOT_FOUND", "Model version was not found for this weapon."
            )

        optimized_model = self._asset_row(conn, model["optimized_model_file_id"])
        unity_material = self._asset_row(conn, model["unity_material_file_id"])
        if optimized_model["role"] != "rough_optimized_glb":
            raise UnityExportError(
                "INVALID_REQUEST", "Unity export requires an optimized rough GLB asset."
            )
        if unity_material["role"] != "unity_material_json":
            raise UnityExportError(
                "INVALID_REQUEST", "Unity export requires Unity material metadata."
            )

        model_quality = json.loads(model["quality_report_json"] or "{}")
        quality_report_asset = None
        if request.include_quality_reports:
            report_file_id = model_quality.get("quality_report_file_id")
            if not report_file_id:
                raise UnityExportError("INVALID_REQUEST", "Model quality report is missing.")
            quality_report_asset = self._asset_row(conn, report_file_id)
        spec_asset = (
            self._latest_weapon_spec_asset(conn, weapon_id) if request.include_source_spec else None
        )
        return {
            "model": model,
            "optimized_model": optimized_model,
            "unity_material": unity_material,
            "quality_report_asset": quality_report_asset,
            "spec_asset": spec_asset,
        }

    def _latest_weapon_spec_asset(
        self, conn: sqlite3.Connection, weapon_id: str
    ) -> Optional[sqlite3.Row]:
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
            _unity_manifest_file_entry(
                optimized_model, f"{package_root}/Models/rough_optimized.glb"
            ),
            _unity_manifest_file_entry(
                unity_material, f"{package_root}/Materials/unity_material.json"
            ),
        ]
        if spec_asset is not None:
            files.append(
                _unity_manifest_file_entry(spec_asset, f"{package_root}/Specs/weapon_spec.json")
            )
        if quality_report_asset is not None:
            files.append(
                _unity_manifest_file_entry(
                    quality_report_asset, f"{package_root}/Reports/model_quality_report.json"
                )
            )
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
            archive.writestr(
                f"{package_root}/Models/rough_optimized.glb", self._asset_payload(optimized_model)
            )
            archive.writestr(
                f"{package_root}/Materials/unity_material.json", self._asset_payload(unity_material)
            )
            if spec_asset is not None:
                archive.writestr(
                    f"{package_root}/Specs/weapon_spec.json", self._asset_payload(spec_asset)
                )
            if quality_report_asset is not None:
                archive.writestr(
                    f"{package_root}/Reports/model_quality_report.json",
                    self._asset_payload(quality_report_asset),
                )
        return buffer.getvalue()


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


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex}"
