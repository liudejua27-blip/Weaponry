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
