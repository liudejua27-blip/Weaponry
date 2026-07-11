from __future__ import annotations

import json
import sqlite3
import uuid
from typing import Any, Optional

from forgecad_agent.infrastructure.db import (
    CheckpointRepository,
    SQLiteConnectionFactory,
)

from ..models import JobActionResponse, utc_now
from ..providers.three_d import ThreeDProvider, ThreeDProviderError


class JobCommandError(RuntimeError):
    def __init__(self, code: str, message: str, *, recoverable: bool = False) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable


class LegacyJobCommandService:
    """Transactional cancel/retry use cases for the frozen legacy job domain."""

    def __init__(
        self,
        connection_factory: SQLiteConnectionFactory,
        three_d_provider: ThreeDProvider,
    ) -> None:
        self.connection_factory = connection_factory
        self.three_d_provider = three_d_provider

    def cancel_job(self, job_id: str) -> JobActionResponse:
        with self.connection_factory.connect() as connection:
            row = _job_action_row(connection, job_id)
            previous_status = str(row["status"])
            if previous_status == "cancelled":
                now = utc_now()
                message = "Cancel request was already applied for this job."
                action_id = _record_job_action(
                    connection,
                    job_id=job_id,
                    action_type="cancel",
                    requested_step=row["current_step"],
                    status="noop",
                    previous_status=previous_status,
                    resulting_status=previous_status,
                    event_id=None,
                    message=message,
                    metadata={"action": "cancel", "noop_reason": "already_cancelled"},
                    created_at=now,
                )
                return JobActionResponse(
                    action_id=action_id,
                    job_id=job_id,
                    status="cancelled",
                    previous_status=previous_status,
                    current_step=row["current_step"],
                    event_id=None,
                    message=message,
                    event_stream_url=f"/api/jobs/{job_id}/events",
                )
            if previous_status not in {
                "created",
                "queued",
                "running",
                "waiting_provider",
                "waiting_user",
                "retrying",
            }:
                raise JobCommandError(
                    "JOB_ACTION_CONFLICT",
                    f"Job {job_id} is {previous_status}; only active or waiting jobs can be cancelled.",
                )

            now = utc_now()
            current_step = str(row["current_step"] or "request_guard")
            provider_task_record_id = _mark_current_provider_cancel_requested(
                connection,
                job_id=job_id,
                step_name=current_step,
                requested_at=now,
            )
            provider_cancel_status = "no_active_provider_task"
            if provider_task_record_id and current_step == "rough3d_submit":
                provider_task = connection.execute(
                    "SELECT provider_task_id FROM provider_tasks WHERE task_record_id = ?",
                    (provider_task_record_id,),
                ).fetchone()
                if provider_task is not None and provider_task["provider_task_id"]:
                    self._cancel_provider_task(
                        connection,
                        task_record_id=provider_task_record_id,
                        provider_task_id=str(provider_task["provider_task_id"]),
                    )
                    provider_cancel_status = "cancel_attempted"

            _update_latest_step_attempt(
                connection,
                job_id,
                current_step,
                "cancelled",
                now,
            )
            connection.execute(
                """
                UPDATE generation_jobs
                SET status = 'cancelled',
                    current_step = ?,
                    cancel_requested_at = COALESCE(cancel_requested_at, ?),
                    cancel_provider_attempted_at = ?,
                    updated_at = ?,
                    finished_at = ?
                WHERE job_id = ?
                """,
                (
                    current_step,
                    now,
                    now if provider_task_record_id else None,
                    now,
                    now,
                    job_id,
                ),
            )
            message = f"Cancel requested for job at step {current_step}."
            event_id = _insert_job_action_event(
                connection,
                job_id=job_id,
                weapon_id=str(row["weapon_id"]),
                step=current_step,
                status="cancelled",
                level="warning",
                message=message,
                metadata={
                    "previous_status": previous_status,
                    "action": "cancel",
                    "provider_task_record_id": provider_task_record_id,
                    "provider_cancel_state": provider_cancel_status,
                },
                created_at=now,
            )
            action_id = _record_job_action(
                connection,
                job_id=job_id,
                action_type="cancel",
                requested_step=current_step,
                status="accepted",
                previous_status=previous_status,
                resulting_status="cancelled",
                event_id=event_id,
                message=message,
                metadata={
                    "action": "cancel",
                    "provider_task_record_id": provider_task_record_id,
                },
                created_at=now,
            )
            return JobActionResponse(
                action_id=action_id,
                job_id=job_id,
                status="cancelled",
                previous_status=previous_status,
                current_step=current_step,
                event_id=event_id,
                message=message,
                event_stream_url=f"/api/jobs/{job_id}/events",
            )

    def retry_job(self, job_id: str) -> JobActionResponse:
        with self.connection_factory.connect() as connection:
            row = _job_action_row(connection, job_id)
            retry_from = _default_retry_step(
                connection,
                job_id,
                row["current_step"],
            )
            return _request_retry(
                connection,
                row=row,
                retry_from=retry_from,
                action_type="retry",
            )

    def retry_job_from_step(self, job_id: str, step_name: str) -> JobActionResponse:
        with self.connection_factory.connect() as connection:
            row = _job_action_row(connection, job_id)
            exists = connection.execute(
                "SELECT 1 FROM job_steps WHERE job_id = ? AND step_name = ? LIMIT 1",
                (job_id, step_name),
            ).fetchone()
            if exists is None:
                raise JobCommandError(
                    "INVALID_REQUEST",
                    f"Cannot retry from unknown step for this job: {step_name}",
                )
            return _request_retry(
                connection,
                row=row,
                retry_from=step_name,
                action_type="retry_from_step",
            )

    def _cancel_provider_task(
        self,
        connection: sqlite3.Connection,
        *,
        task_record_id: str,
        provider_task_id: str,
    ) -> None:
        now = utc_now()
        row = connection.execute(
            "SELECT metadata_json FROM provider_tasks WHERE task_record_id = ?",
            (task_record_id,),
        ).fetchone()
        try:
            metadata = json.loads(row["metadata_json"] or "{}") if row else {}
        except json.JSONDecodeError:
            metadata = {}
        try:
            cancel = self.three_d_provider.cancel_rough_model(
                provider_task_id=provider_task_id
            )
            status = "cancelled" if cancel.status == "cancelled" else "cancel_requested"
            metadata = {
                **metadata,
                **cancel.metadata,
                "provider_cancel_status": cancel.status,
            }
        except ThreeDProviderError as exc:
            status = "cancel_requested"
            metadata = {
                **metadata,
                "provider_cancel_error": exc.code,
                "provider_cancel_message": str(exc),
            }
        connection.execute(
            """
            UPDATE provider_tasks
            SET status = ?, last_seen_at = ?, updated_at = ?, metadata_json = ?
            WHERE task_record_id = ?
            """,
            (status, now, now, _canonical_json(metadata), task_record_id),
        )


def _job_action_row(connection: sqlite3.Connection, job_id: str) -> sqlite3.Row:
    row = connection.execute(
        """
        SELECT job_id, weapon_id, status, current_step
        FROM generation_jobs
        WHERE job_id = ?
        """,
        (job_id,),
    ).fetchone()
    if row is None:
        raise KeyError(job_id)
    return row


def _default_retry_step(
    connection: sqlite3.Connection,
    job_id: str,
    current_step: Optional[str],
) -> str:
    failed_step = connection.execute(
        """
        SELECT step_name
        FROM job_steps
        WHERE job_id = ? AND status = 'failed'
        ORDER BY attempt DESC, started_at DESC
        LIMIT 1
        """,
        (job_id,),
    ).fetchone()
    if failed_step:
        return str(failed_step["step_name"])
    if current_step:
        return str(current_step)
    first_step = connection.execute(
        """
        SELECT step_name
        FROM job_steps
        WHERE job_id = ?
        ORDER BY started_at ASC
        LIMIT 1
        """,
        (job_id,),
    ).fetchone()
    return str(first_step["step_name"]) if first_step else "request_guard"


def _request_retry(
    connection: sqlite3.Connection,
    *,
    row: sqlite3.Row,
    retry_from: str,
    action_type: str,
) -> JobActionResponse:
    previous_status = str(row["status"])
    job_id = str(row["job_id"])
    if previous_status not in {"failed", "partial_succeeded", "waiting_user"}:
        raise JobCommandError(
            "JOB_ACTION_CONFLICT",
            f"Job {job_id} is {previous_status}; retry requires failed, partial_succeeded, or waiting_user.",
        )
    now = utc_now()
    attempt = _insert_retry_step_attempt(connection, job_id, retry_from, now)
    connection.execute(
        """
        UPDATE generation_jobs
        SET status = 'retrying',
            current_step = ?,
            retry_count = retry_count + 1,
            error_code = NULL,
            error_message = NULL,
            updated_at = ?,
            finished_at = NULL
        WHERE job_id = ?
        """,
        (retry_from, now, job_id),
    )
    message = f"Retry requested from step {retry_from}."
    event_id = _insert_job_action_event(
        connection,
        job_id=job_id,
        weapon_id=str(row["weapon_id"]),
        step=retry_from,
        status="retrying",
        level="info",
        message=message,
        metadata={
            "previous_status": previous_status,
            "retry_from": retry_from,
            "attempt": attempt,
            "action": "retry",
        },
        created_at=now,
    )
    action_id = _record_job_action(
        connection,
        job_id=job_id,
        action_type=action_type,
        requested_step=retry_from,
        status="accepted",
        previous_status=previous_status,
        resulting_status="retrying",
        event_id=event_id,
        message=message,
        metadata={"retry_from": retry_from, "attempt": attempt, "action": "retry"},
        created_at=now,
    )
    return JobActionResponse(
        action_id=action_id,
        job_id=job_id,
        status="retrying",
        previous_status=previous_status,
        current_step=retry_from,
        event_id=event_id,
        message=message,
        retry_from=retry_from,
        event_stream_url=f"/api/jobs/{job_id}/events",
    )


def _insert_retry_step_attempt(
    connection: sqlite3.Connection,
    job_id: str,
    step_name: str,
    created_at: str,
) -> int:
    row = connection.execute(
        """
        SELECT COALESCE(MAX(attempt), 0) + 1 AS next_attempt
        FROM job_steps
        WHERE job_id = ? AND step_name = ?
        """,
        (job_id, step_name),
    ).fetchone()
    attempt = int(row["next_attempt"])
    connection.execute(
        """
        INSERT INTO job_steps (
          step_id, job_id, step_name, attempt, status, provider, started_at,
          checkpoint_json, resumable_after_restart, cancel_state
        )
        VALUES (?, ?, ?, ?, 'queued', 'job_retry', ?, ?, 1, 'none')
        """,
        (
            _new_id("step"),
            job_id,
            step_name,
            attempt,
            created_at,
            _canonical_json(
                {"step": step_name, "attempt": attempt, "resume_policy": "restart_step"}
            ),
        ),
    )
    CheckpointRepository(connection).upsert(
        job_id=job_id,
        step_name=step_name,
        attempt=attempt,
        status="ready",
        resume_policy="restart_step",
        provider_task_record_id=None,
        state={"step": step_name, "attempt": attempt, "retry_requested": True},
        updated_at=created_at,
    )
    return attempt


def _update_latest_step_attempt(
    connection: sqlite3.Connection,
    job_id: str,
    step_name: str,
    status: str,
    finished_at: str,
) -> None:
    row = connection.execute(
        """
        SELECT step_id, attempt
        FROM job_steps
        WHERE job_id = ? AND step_name = ?
        ORDER BY attempt DESC
        LIMIT 1
        """,
        (job_id, step_name),
    ).fetchone()
    if row is None:
        step_id = _new_id("step")
        attempt = 1
        connection.execute(
            """
            INSERT INTO job_steps (
              step_id, job_id, step_name, attempt, status, provider, started_at,
              finished_at, checkpoint_json, resumable_after_restart, cancel_state
            )
            VALUES (?, ?, ?, 1, ?, 'job_action', ?, ?, ?, 0, ?)
            """,
            (
                step_id,
                job_id,
                step_name,
                status,
                finished_at,
                finished_at,
                _canonical_json(
                    {"step": step_name, "status": status, "updated_by": "job_action"}
                ),
                "cancel_requested" if status == "cancelled" else "none",
            ),
        )
    else:
        step_id = str(row["step_id"])
        attempt = int(row["attempt"])
        connection.execute(
            """
            UPDATE job_steps
            SET status = ?,
                finished_at = ?,
                resumable_after_restart = ?,
                cancel_state = CASE
                  WHEN ? = 'cancelled' THEN 'cancel_requested'
                  ELSE cancel_state
                END,
                checkpoint_json = ?
            WHERE step_id = ?
            """,
            (
                status,
                finished_at,
                0 if status == "cancelled" else 1,
                status,
                _canonical_json(
                    {"step": step_name, "status": status, "updated_by": "job_action"}
                ),
                step_id,
            ),
        )
    CheckpointRepository(connection).upsert(
        job_id=job_id,
        step_name=step_name,
        attempt=attempt,
        status="cancelled" if status == "cancelled" else "ready",
        resume_policy="manual_review" if status == "cancelled" else "restart_step",
        provider_task_record_id=None,
        state={"step": step_name, "status": status, "updated_by": "job_action"},
        updated_at=finished_at,
    )


def _mark_current_provider_cancel_requested(
    connection: sqlite3.Connection,
    *,
    job_id: str,
    step_name: str,
    requested_at: str,
) -> Optional[str]:
    task = connection.execute(
        """
        SELECT task_record_id
        FROM provider_tasks
        WHERE job_id = ? AND step_name = ?
          AND status IN ('submitted', 'polling', 'unknown')
        ORDER BY attempt DESC, updated_at DESC
        LIMIT 1
        """,
        (job_id, step_name),
    ).fetchone()
    connection.execute(
        """
        UPDATE job_steps
        SET cancel_state = 'cancel_requested'
        WHERE job_id = ? AND step_name = ?
          AND attempt = (
            SELECT MAX(attempt)
            FROM job_steps
            WHERE job_id = ? AND step_name = ?
          )
        """,
        (job_id, step_name, job_id, step_name),
    )
    if task is None:
        return None
    connection.execute(
        """
        UPDATE provider_tasks
        SET status = 'cancel_requested',
            cancel_requested_at = ?,
            updated_at = ?
        WHERE task_record_id = ?
        """,
        (requested_at, requested_at, task["task_record_id"]),
    )
    return str(task["task_record_id"])


def _insert_job_action_event(
    connection: sqlite3.Connection,
    *,
    job_id: str,
    weapon_id: str,
    step: str,
    status: str,
    level: str,
    message: str,
    metadata: dict[str, Any],
    created_at: str,
) -> str:
    row = connection.execute(
        """
        SELECT COALESCE(MAX(seq), 0) + 1 AS next_seq
        FROM agent_events
        WHERE job_id = ?
        """,
        (job_id,),
    ).fetchone()
    next_seq = int(row["next_seq"])
    event_id = f"evt_{job_id}_{next_seq:04d}"
    connection.execute(
        """
        INSERT INTO agent_events (
          event_id, job_id, seq, weapon_id, step, level, status, message,
          artifact_asset_id, metadata_json, created_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
        """,
        (
            event_id,
            job_id,
            next_seq,
            weapon_id,
            step,
            level,
            status,
            message,
            _canonical_json({**metadata, "progress": 0}),
            created_at,
        ),
    )
    return event_id


def _record_job_action(
    connection: sqlite3.Connection,
    *,
    job_id: str,
    action_type: str,
    requested_step: Optional[str],
    status: str,
    previous_status: str,
    resulting_status: str,
    event_id: Optional[str],
    message: str,
    metadata: dict[str, Any],
    created_at: str,
) -> str:
    action_id = _new_id("action")
    connection.execute(
        """
        INSERT INTO job_actions (
          action_id, job_id, action_type, requested_step, status,
          previous_job_status, resulting_job_status, event_id, message,
          metadata_json, created_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            action_id,
            job_id,
            action_type,
            requested_step,
            status,
            previous_status,
            resulting_status,
            event_id,
            message,
            _canonical_json(metadata),
            created_at,
        ),
    )
    return action_id


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex[:12]}"
