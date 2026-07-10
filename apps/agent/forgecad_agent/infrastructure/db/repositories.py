from __future__ import annotations

import json
import sqlite3
import uuid
from dataclasses import dataclass
from typing import Any, Mapping, Optional


@dataclass(frozen=True)
class IdempotencyRecord:
    request_hash: str
    response_json: str


class IdempotencyRepository:
    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def get(self, scope: str, key: str) -> Optional[IdempotencyRecord]:
        row = self.connection.execute(
            """
            SELECT request_hash, response_json
            FROM idempotency_records
            WHERE scope = ? AND idempotency_key = ?
            """,
            (scope, key),
        ).fetchone()
        if row is None:
            return None
        return IdempotencyRecord(
            request_hash=str(row["request_hash"]),
            response_json=str(row["response_json"]),
        )

    def add(
        self,
        *,
        scope: str,
        key: str,
        request_hash: str,
        response_json: str,
        created_at: str,
    ) -> None:
        self.connection.execute(
            """
            INSERT INTO idempotency_records (
              scope, idempotency_key, request_hash, response_json, created_at
            )
            VALUES (?, ?, ?, ?, ?)
            """,
            (scope, key, request_hash, response_json, created_at),
        )


class AssetRepository:
    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def get_active(self, file_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT file_id, weapon_id, version_id, job_id, role, logical_path, object_path,
                   sha256, byte_size, mime_type, ext, width, height, metadata_json, created_at
            FROM asset_files
            WHERE file_id = ? AND soft_deleted_at IS NULL
            """,
            (file_id,),
        ).fetchone()


class JobRepository:
    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def exists(self, job_id: str) -> bool:
        return self.connection.execute(
            "SELECT 1 FROM generation_jobs WHERE job_id = ?",
            (job_id,),
        ).fetchone() is not None

    def get(self, job_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT job_id, weapon_id, job_type, status, current_step,
                   created_at, updated_at, error_code, error_message
            FROM generation_jobs
            WHERE job_id = ?
            """,
            (job_id,),
        ).fetchone()

    def assets(self, job_id: str) -> list[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT file_id, role
            FROM asset_files
            WHERE job_id = ? AND soft_deleted_at IS NULL
            ORDER BY created_at ASC
            """,
            (job_id,),
        ).fetchall()

    def latest_version(self, job_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT version_id
            FROM weapon_versions
            WHERE job_id = ?
            ORDER BY version_no DESC
            LIMIT 1
            """,
            (job_id,),
        ).fetchone()

    def latest_model(self, job_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT model_id
            FROM models_3d
            WHERE job_id = ?
            ORDER BY created_at DESC
            LIMIT 1
            """,
            (job_id,),
        ).fetchone()

    def list_summaries(
        self,
        *,
        query: Optional[str],
        status: Optional[str],
        job_type: Optional[str],
        error_code: Optional[str],
        cursor: Optional[tuple[str, str]],
        limit: int,
    ) -> list[sqlite3.Row]:
        clauses = ["1 = 1"]
        params: list[Any] = []
        if status:
            clauses.append("j.status = ?")
            params.append(status)
        if job_type:
            clauses.append("j.job_type = ?")
            params.append(job_type)
        if error_code:
            clauses.append("j.error_code = ?")
            params.append(error_code)
        if query:
            needle = f"%{query.strip()}%"
            clauses.append(
                """
                (
                  j.job_id LIKE ?
                  OR j.weapon_id LIKE ?
                  OR j.current_step LIKE ?
                  OR j.error_code LIKE ?
                  OR j.error_message LIKE ?
                  OR w.name LIKE ?
                )
                """
            )
            params.extend([needle, needle, needle, needle, needle, needle])
        if cursor is not None:
            cursor_updated_at, cursor_job_id = cursor
            clauses.append("(j.updated_at < ? OR (j.updated_at = ? AND j.job_id < ?))")
            params.extend([cursor_updated_at, cursor_updated_at, cursor_job_id])

        where_sql = " AND ".join(clauses)
        return self.connection.execute(
            f"""
            WITH event_rollup AS (
              SELECT
                e.job_id,
                COUNT(*) AS event_count,
                (
                  SELECT e2.status
                  FROM agent_events e2
                  WHERE e2.job_id = e.job_id
                  ORDER BY e2.seq DESC
                  LIMIT 1
                ) AS latest_event_status,
                (
                  SELECT e2.message
                  FROM agent_events e2
                  WHERE e2.job_id = e.job_id
                  ORDER BY e2.seq DESC
                  LIMIT 1
                ) AS latest_event_message,
                (
                  SELECT e2.created_at
                  FROM agent_events e2
                  WHERE e2.job_id = e.job_id
                  ORDER BY e2.seq DESC
                  LIMIT 1
                ) AS latest_event_created_at
              FROM agent_events e
              GROUP BY e.job_id
            ),
            action_rollup AS (
              SELECT job_id, COUNT(*) AS action_count
              FROM job_actions
              GROUP BY job_id
            ),
            latest_versions AS (
              SELECT job_id, version_id
              FROM weapon_versions
              WHERE job_id IS NOT NULL
                AND version_id IN (
                  SELECT version_id
                  FROM weapon_versions vv
                  WHERE vv.job_id = weapon_versions.job_id
                  ORDER BY version_no DESC
                  LIMIT 1
                )
            ),
            latest_models AS (
              SELECT job_id, model_id
              FROM models_3d
              WHERE job_id IS NOT NULL
                AND model_id IN (
                  SELECT model_id
                  FROM models_3d mm
                  WHERE mm.job_id = models_3d.job_id
                  ORDER BY created_at DESC, model_id DESC
                  LIMIT 1
                )
            )
            SELECT
              j.job_id, j.weapon_id, w.name AS weapon_name, j.job_type, j.status,
              j.current_step, j.error_code, j.error_message, j.created_at,
              j.updated_at, j.finished_at,
              COALESCE(er.event_count, 0) AS event_count,
              COALESCE(ar.action_count, 0) AS action_count,
              er.latest_event_status, er.latest_event_message, er.latest_event_created_at,
              lv.version_id AS output_version_id,
              lm.model_id AS output_model_id
            FROM generation_jobs j
            LEFT JOIN weapons w ON w.weapon_id = j.weapon_id
            LEFT JOIN event_rollup er ON er.job_id = j.job_id
            LEFT JOIN action_rollup ar ON ar.job_id = j.job_id
            LEFT JOIN latest_versions lv ON lv.job_id = j.job_id
            LEFT JOIN latest_models lm ON lm.job_id = j.job_id
            WHERE {where_sql}
            ORDER BY j.updated_at DESC, j.job_id DESC
            LIMIT ?
            """,
            (*params, limit),
        ).fetchall()

    def list_actions(
        self,
        job_id: str,
        *,
        cursor: Optional[tuple[str, str]],
        limit: int,
    ) -> list[sqlite3.Row]:
        clauses = ["job_id = ?"]
        params: list[Any] = [job_id]
        if cursor is not None:
            cursor_created_at, cursor_action_id = cursor
            clauses.append("(created_at < ? OR (created_at = ? AND action_id < ?))")
            params.extend([cursor_created_at, cursor_created_at, cursor_action_id])
        return self.connection.execute(
            f"""
            SELECT action_id, job_id, action_type, requested_step, status,
                   previous_job_status, resulting_job_status, event_id,
                   message, metadata_json, created_at
            FROM job_actions
            WHERE {" AND ".join(clauses)}
            ORDER BY created_at DESC, action_id DESC
            LIMIT ?
            """,
            (*params, limit),
        ).fetchall()

    def runtime_header(self, job_id: str) -> Optional[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT job_id, status, current_step
            FROM generation_jobs
            WHERE job_id = ?
            """,
            (job_id,),
        ).fetchone()

    def provider_tasks(self, job_id: str) -> list[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT task_record_id, job_id, step_name, attempt, provider_kind,
                   provider_id, provider_task_id, status, cancel_requested_at,
                   last_seen_at, metadata_json, created_at, updated_at
            FROM provider_tasks
            WHERE job_id = ?
            ORDER BY created_at ASC, attempt ASC
            """,
            (job_id,),
        ).fetchall()

    def checkpoints(self, job_id: str) -> list[sqlite3.Row]:
        return self.connection.execute(
            """
            SELECT checkpoint_id, job_id, step_name, attempt, status, resume_policy,
                   provider_task_record_id, state_json, created_at, updated_at
            FROM job_checkpoints
            WHERE job_id = ?
            ORDER BY attempt ASC, created_at ASC
            """,
            (job_id,),
        ).fetchall()

    def event_cursor_seq(self, job_id: str, event_id: str) -> Optional[int]:
        row = self.connection.execute(
            "SELECT seq FROM agent_events WHERE job_id = ? AND event_id = ?",
            (job_id, event_id),
        ).fetchone()
        return int(row["seq"]) if row is not None else None

    def events(self, job_id: str, *, after_seq: Optional[int] = None) -> list[sqlite3.Row]:
        clauses = ["job_id = ?"]
        params: list[Any] = [job_id]
        if after_seq is not None:
            clauses.append("seq > ?")
            params.append(after_seq)
        return self.connection.execute(
            f"""
            SELECT event_id, seq, job_id, weapon_id, step, level, status, message,
                   artifact_asset_id, metadata_json, created_at
            FROM agent_events
            WHERE {" AND ".join(clauses)}
            ORDER BY seq ASC
            """,
            params,
        ).fetchall()

    def has_event(self, job_id: str, event_id: str) -> bool:
        return self.connection.execute(
            "SELECT 1 FROM agent_events WHERE job_id = ? AND event_id = ?",
            (job_id, event_id),
        ).fetchone() is not None


class CheckpointRepository:
    def __init__(self, connection: sqlite3.Connection) -> None:
        self.connection = connection

    def upsert(
        self,
        *,
        job_id: str,
        step_name: str,
        attempt: int,
        status: str,
        resume_policy: str,
        provider_task_record_id: Optional[str],
        state: Mapping[str, Any],
        updated_at: str,
    ) -> str:
        existing = self.connection.execute(
            """
            SELECT checkpoint_id
            FROM job_checkpoints
            WHERE job_id = ? AND step_name = ? AND attempt = ?
            """,
            (job_id, step_name, attempt),
        ).fetchone()
        state_json = json.dumps(state, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        if existing is not None:
            checkpoint_id = str(existing["checkpoint_id"])
            self.connection.execute(
                """
                UPDATE job_checkpoints
                SET status = ?, resume_policy = ?, provider_task_record_id = ?,
                    state_json = ?, updated_at = ?
                WHERE checkpoint_id = ?
                """,
                (
                    status,
                    resume_policy,
                    provider_task_record_id,
                    state_json,
                    updated_at,
                    checkpoint_id,
                ),
            )
            return checkpoint_id

        checkpoint_id = f"ckpt_{uuid.uuid4().hex[:12]}"
        self.connection.execute(
            """
            INSERT INTO job_checkpoints (
              checkpoint_id, job_id, step_name, attempt, status, resume_policy,
              provider_task_record_id, state_json, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                checkpoint_id,
                job_id,
                step_name,
                attempt,
                status,
                resume_policy,
                provider_task_record_id,
                state_json,
                updated_at,
                updated_at,
            ),
        )
        return checkpoint_id
