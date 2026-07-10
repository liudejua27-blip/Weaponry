from __future__ import annotations

import json
import sqlite3
from typing import Any

from forgecad_agent.infrastructure.db import (
    CheckpointRepository,
    SQLiteConnectionFactory,
    SQLiteUnitOfWork,
)

from ..models import RuntimeRecoveryItem, RuntimeRecoveryResponse, utc_now


class LegacyJobRecoveryService:
    """Pause interrupted legacy jobs at a deterministic review checkpoint."""

    _RECOVERABLE_STATUSES = {
        "created",
        "queued",
        "running",
        "waiting_provider",
        "retrying",
    }

    def __init__(self, connection_factory: SQLiteConnectionFactory) -> None:
        self.connection_factory = connection_factory

    def recover_interrupted_jobs(
        self,
        reason: str = "manual",
        *,
        include_queued: bool = True,
    ) -> RuntimeRecoveryResponse:
        items: list[RuntimeRecoveryItem] = []
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            connection = unit_of_work.require_connection()
            rows = connection.execute(
                """
                SELECT job_id, weapon_id, status, current_step, provider_task_id
                FROM generation_jobs
                WHERE status IN ('created', 'queued', 'running', 'waiting_provider', 'retrying')
                ORDER BY updated_at ASC
                """
            ).fetchall()
            now = utc_now()
            for row in rows:
                previous_status = str(row["status"])
                if previous_status not in self._RECOVERABLE_STATUSES:
                    continue
                if not include_queued and previous_status in {"queued", "retrying"}:
                    continue

                job_id = str(row["job_id"])
                current_step = str(row["current_step"] or "request_guard")
                provider_task = _latest_provider_task(
                    connection,
                    job_id=job_id,
                    step_name=current_step,
                )
                provider_task_id = (
                    provider_task["provider_task_id"]
                    if provider_task is not None
                    else row["provider_task_id"]
                )
                if provider_task is not None:
                    _mark_provider_task_unknown(
                        connection,
                        task_record_id=str(provider_task["task_record_id"]),
                        updated_at=now,
                    )

                state = {
                    "recovery_reason": reason,
                    "previous_status": previous_status,
                    "resume_from": current_step,
                    "provider_task_id": provider_task_id,
                }
                message = f"Agent restart recovery paused job at step {current_step}."
                event_id = _insert_recovery_event(
                    connection,
                    job_id=job_id,
                    weapon_id=str(row["weapon_id"]),
                    step=current_step,
                    message=message,
                    metadata=state,
                    created_at=now,
                )
                connection.execute(
                    """
                    UPDATE generation_jobs
                    SET status = 'waiting_user',
                        current_step = ?,
                        checkpoint_json = ?,
                        updated_at = ?,
                        finished_at = NULL
                    WHERE job_id = ?
                    """,
                    (
                        current_step,
                        _canonical_json(state),
                        now,
                        job_id,
                    ),
                )
                CheckpointRepository(connection).upsert(
                    job_id=job_id,
                    step_name=current_step,
                    attempt=_latest_step_attempt(connection, job_id, current_step),
                    status="ready",
                    resume_policy="manual_review",
                    provider_task_record_id=(
                        str(provider_task["task_record_id"])
                        if provider_task is not None
                        else None
                    ),
                    state=state,
                    updated_at=now,
                )
                items.append(
                    RuntimeRecoveryItem(
                        job_id=job_id,
                        weapon_id=row["weapon_id"],
                        previous_status=previous_status,  # type: ignore[arg-type]
                        status="waiting_user",
                        resume_from_step=current_step,
                        provider_task_id=provider_task_id,
                        event_id=event_id,
                        message=message,
                    )
                )
        return RuntimeRecoveryResponse(recovered_count=len(items), items=items)


def _latest_provider_task(
    connection: sqlite3.Connection,
    *,
    job_id: str,
    step_name: str,
) -> sqlite3.Row | None:
    return connection.execute(
        """
        SELECT task_record_id, provider_task_id
        FROM provider_tasks
        WHERE job_id = ? AND step_name = ?
        ORDER BY attempt DESC, updated_at DESC
        LIMIT 1
        """,
        (job_id, step_name),
    ).fetchone()


def _mark_provider_task_unknown(
    connection: sqlite3.Connection,
    *,
    task_record_id: str,
    updated_at: str,
) -> None:
    connection.execute(
        """
        UPDATE provider_tasks
        SET status = CASE
              WHEN status IN ('submitted', 'polling') THEN 'unknown'
              ELSE status
            END,
            updated_at = ?
        WHERE task_record_id = ?
        """,
        (updated_at, task_record_id),
    )


def _insert_recovery_event(
    connection: sqlite3.Connection,
    *,
    job_id: str,
    weapon_id: str,
    step: str,
    message: str,
    metadata: dict[str, Any],
    created_at: str,
) -> str:
    next_seq_row = connection.execute(
        """
        SELECT COALESCE(MAX(seq), 0) + 1 AS next_seq
        FROM agent_events
        WHERE job_id = ?
        """,
        (job_id,),
    ).fetchone()
    next_seq = int(next_seq_row["next_seq"])
    event_id = f"evt_{job_id}_{next_seq:04d}"
    connection.execute(
        """
        INSERT INTO agent_events (
          event_id, job_id, seq, weapon_id, step, level, status, message,
          artifact_asset_id, metadata_json, created_at
        )
        VALUES (?, ?, ?, ?, ?, 'warning', 'waiting_user', ?, NULL, ?, ?)
        """,
        (
            event_id,
            job_id,
            next_seq,
            weapon_id,
            step,
            message,
            _canonical_json({**metadata, "progress": 0}),
            created_at,
        ),
    )
    return event_id


def _latest_step_attempt(
    connection: sqlite3.Connection,
    job_id: str,
    step_name: str,
) -> int:
    row = connection.execute(
        """
        SELECT COALESCE(MAX(attempt), 1) AS attempt
        FROM job_steps
        WHERE job_id = ? AND step_name = ?
        """,
        (job_id, step_name),
    ).fetchone()
    return int(row["attempt"])


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
