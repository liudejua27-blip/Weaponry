from __future__ import annotations

import json
import sqlite3
import uuid
from typing import Any, Callable, Dict, Optional

from forgecad_agent.infrastructure.db import SQLiteConnectionFactory

from ..models import (
    ExportUnityRequest,
    Generate3DRequest,
    RuntimeWorkOnceResponse,
    utc_now,
)
from ..providers.three_d import ThreeDProvider, ThreeDProviderError


class WorkerServiceError(RuntimeError):
    def __init__(self, code: str, message: str, *, recoverable: bool = False) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable


class LegacyWorkerService:
    """Claims legacy jobs and owns Generate-3D provider execution."""

    def __init__(
        self,
        *,
        connection_factory: SQLiteConnectionFactory,
        three_d_provider: ThreeDProvider,
        asset_payload: Callable[[sqlite3.Row], bytes],
        asset_row: Callable[..., sqlite3.Row],
        complete_export_unity_worker_job: Callable[..., str],
        insert_event: Callable[..., None],
        insert_job_action_event: Callable[..., None],
        job_cancel_requested: Callable[..., bool],
        next_event_seq: Callable[..., int],
        record_provider_task: Callable[..., str],
        suppress_cancelled_worker_commit: Callable[..., None],
        update_step_runtime: Callable[..., None],
        upsert_checkpoint: Callable[..., None],
        write_asset: Callable[..., str],
        write_rough_model_result_assets: Callable[..., Dict[str, str]],
    ) -> None:
        self.three_d_provider = three_d_provider
        self._connect = connection_factory.connect
        self._asset_payload = asset_payload
        self._asset_row = asset_row
        self._complete_export_unity_worker_job = complete_export_unity_worker_job
        self._insert_event = insert_event
        self._insert_job_action_event = insert_job_action_event
        self._job_cancel_requested = job_cancel_requested
        self._next_event_seq = next_event_seq
        self._record_provider_task = record_provider_task
        self._suppress_cancelled_worker_commit = suppress_cancelled_worker_commit
        self._update_step_runtime = update_step_runtime
        self._upsert_checkpoint = upsert_checkpoint
        self._write_asset = write_asset
        self._write_rough_model_result_assets = write_rough_model_result_assets

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
                    return RuntimeWorkOnceResponse(
                        claimed=False, message="Waiting provider job was already claimed."
                    )
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
                    return RuntimeWorkOnceResponse(
                        claimed=False, message="Queued job was already claimed."
                    )

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
                        status = self._resume_generate_3d_provider_job(
                            conn,
                            job_id=job_id,
                            weapon_id=str(job["weapon_id"]),
                            request=request,
                            runner_id=runner_id,
                        )
                    else:
                        status = self._complete_generate_3d_worker_job(
                            conn,
                            job_id=job_id,
                            weapon_id=str(job["weapon_id"]),
                            request=request,
                            runner_id=runner_id,
                        )
                elif job_type == "export_unity":
                    request = ExportUnityRequest(**json.loads(job["request_json"]))
                    status = self._complete_export_unity_worker_job(
                        conn,
                        job_id=job_id,
                        weapon_id=str(job["weapon_id"]),
                        request=request,
                        runner_id=runner_id,
                    )
                else:
                    raise WorkerServiceError(
                        "INVALID_REQUEST", f"Unsupported worker job type: {job_type}"
                    )
                conn.commit()
                return RuntimeWorkOnceResponse(
                    claimed=True,
                    job_id=job_id,
                    job_type=job_type,
                    status=status,  # type: ignore[arg-type]
                    message=f"Worker completed {job_type} job with status {status}.",
                )
            except Exception as caught:
                exc = _coerce_worker_error(caught)
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
                return RuntimeWorkOnceResponse(
                    claimed=True,
                    job_id=job_id,
                    job_type=job_type,
                    status="failed",
                    message=str(exc),
                )

    def _validate_generate_3d_source(
        self, conn: sqlite3.Connection, weapon_id: str, request: Generate3DRequest
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
            raise WorkerServiceError(
                "VERSION_NOT_FOUND", "Source version was not found for this weapon."
            )

        source_image = self._asset_row(conn, request.source_image_asset_id)
        if (
            source_image["weapon_id"] != weapon_id
            or source_image["version_id"] != request.source_version_id
        ):
            raise WorkerServiceError(
                "INVALID_REQUEST", "Source image does not belong to the requested source version."
            )
        if source_image["role"] not in {"concept_image", "concept_patch"}:
            raise WorkerServiceError(
                "INVALID_REQUEST", "Source image must be a concept image or patch image."
            )
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
            raise WorkerServiceError("JOB_NOT_FOUND", f"Job was not found: {job_id}")
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
            (
                version_id,
                weapon_id,
                request.source_version_id,
                job_id,
                version_no,
                "Worker draft for rough 3D generation.",
                now,
            ),
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
            metadata={
                "schema_version": "ModelGenerationInput@1",
                "provider": request.provider_id,
                "runtime": "worker",
            },
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
            metadata={
                "source_image_asset_id": request.source_image_asset_id,
                "artifact_asset_id": model_input_file_id,
            },
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
            raise WorkerServiceError(exc.code, str(exc), recoverable=exc.recoverable) from exc
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
            self._cancel_provider_task(
                conn, task_record_id=task_record_id, provider_task_id=provider_task_id
            )
            self._suppress_cancelled_worker_commit(
                conn, job_id=job_id, weapon_id=weapon_id, step="rough3d_submit"
            )
            return "cancelled"

        try:
            poll = self.three_d_provider.poll_rough_model(provider_task_id=provider_task_id)
        except ThreeDProviderError as exc:
            raise WorkerServiceError(exc.code, str(exc), recoverable=exc.recoverable) from exc

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
            raise WorkerServiceError(
                "JOB_CHECKPOINT_MISSING", "Generate-3D provider job is missing its draft version."
            )
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
            raise WorkerServiceError(
                "JOB_CHECKPOINT_MISSING", "Generate-3D provider job is missing its provider task."
            )
        metadata = json.loads(task["metadata_json"] or "{}")
        model_id = str(metadata.get("model_id") or "")
        if not model_id:
            raise WorkerServiceError(
                "JOB_CHECKPOINT_MISSING", "Generate-3D provider checkpoint is missing model_id."
            )
        provider_task_id = str(task["provider_task_id"])
        task_record_id = str(task["task_record_id"])
        source_image = self._validate_generate_3d_source(conn, weapon_id, request)
        source_image_bytes = self._asset_payload(source_image)

        if task["status"] == "cancel_requested" or self._job_cancel_requested(conn, job_id):
            self._cancel_provider_task(
                conn, task_record_id=task_record_id, provider_task_id=provider_task_id
            )
            self._suppress_cancelled_worker_commit(
                conn, job_id=job_id, weapon_id=weapon_id, step="rough3d_submit"
            )
            return "cancelled"

        try:
            poll = self.three_d_provider.poll_rough_model(provider_task_id=provider_task_id)
        except ThreeDProviderError as exc:
            raise WorkerServiceError(exc.code, str(exc), recoverable=exc.recoverable) from exc

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
            "phase": "polling"
            if poll_status in {"submitted", "polling", "unknown"}
            else poll_status,
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
            self._update_step_runtime(
                conn,
                job_id,
                "rough3d_submit",
                "waiting_provider",
                poll_at,
                provider_task_id=provider_task_id,
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
            raise WorkerServiceError(
                poll_error_code or "PROVIDER_BAD_OUTPUT",
                poll_error_message or "Rough 3D provider task failed.",
                recoverable=True,
            )
        if poll_status == "cancelled":
            self._suppress_cancelled_worker_commit(
                conn, job_id=job_id, weapon_id=weapon_id, step="rough3d_submit"
            )
            return "cancelled"
        if poll_status != "succeeded":
            raise WorkerServiceError(
                "PROVIDER_BAD_OUTPUT",
                f"Unexpected rough 3D provider status: {poll_status}",
                recoverable=True,
            )

        if self._job_cancel_requested(conn, job_id):
            self._cancel_provider_task(
                conn, task_record_id=task_record_id, provider_task_id=provider_task_id
            )
            self._suppress_cancelled_worker_commit(
                conn, job_id=job_id, weapon_id=weapon_id, step="rough3d_submit"
            )
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
            raise WorkerServiceError(exc.code, str(exc), recoverable=exc.recoverable) from exc

        if self._job_cancel_requested(conn, job_id):
            self._cancel_provider_task(
                conn, task_record_id=task_record_id, provider_task_id=provider_task_id
            )
            self._suppress_cancelled_worker_commit(
                conn, job_id=job_id, weapon_id=weapon_id, step="rough3d_submit"
            )
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
        self._update_step_runtime(
            conn,
            job_id,
            "rough3d_submit",
            "succeeded",
            utc_now(),
            provider_task_id=provider_task_id,
        )
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
            (
                "rough3d_submit",
                "succeeded",
                "Rough 3D provider produced GLB variants.",
                model_assets["raw_model_file_id"],
                0.55,
                {
                    "model_id": model_id,
                    "provider": self.three_d_provider.provider_id,
                    "provider_task_id": provider_task_id,
                },
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
                "Generate-3D worker job completed.",
                model_assets["model_quality_report_file_id"],
                1.0,
                {"new_version_id": version_id, "parent_version_id": request.source_version_id},
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

    def _provider_task_metadata(
        self, conn: sqlite3.Connection, task_record_id: str
    ) -> Dict[str, Any]:
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

    def _cancel_provider_task(
        self, conn: sqlite3.Connection, *, task_record_id: str, provider_task_id: str
    ) -> None:
        now = utc_now()
        metadata = self._provider_task_metadata(conn, task_record_id)
        try:
            cancel = self.three_d_provider.cancel_rough_model(provider_task_id=provider_task_id)
            status = "cancelled" if cancel.status == "cancelled" else "cancel_requested"
            metadata = {**metadata, **cancel.metadata, "provider_cancel_status": cancel.status}
        except ThreeDProviderError as exc:
            status = "cancel_requested"
            metadata = {
                **metadata,
                "provider_cancel_error": exc.code,
                "provider_cancel_message": str(exc),
            }
        conn.execute(
            """
            UPDATE provider_tasks
            SET status = ?, last_seen_at = ?, updated_at = ?, metadata_json = ?
            WHERE task_record_id = ?
            """,
            (status, now, now, _canonical_json(metadata), task_record_id),
        )


def _coerce_worker_error(error: Exception) -> WorkerServiceError:
    if isinstance(error, WorkerServiceError):
        return error
    code = getattr(error, "code", None)
    if isinstance(code, str) and code:
        return WorkerServiceError(
            code,
            str(error),
            recoverable=bool(getattr(error, "recoverable", False)),
        )
    raise error


def _latest_step_attempt(
    conn: sqlite3.Connection,
    job_id: str,
    step_name: str,
) -> int:
    row = conn.execute(
        """
        SELECT COALESCE(MAX(attempt), 1) AS attempt
        FROM job_steps
        WHERE job_id = ? AND step_name = ?
        """,
        (job_id, step_name),
    ).fetchone()
    return int(row["attempt"] if row else 1)


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex}"
