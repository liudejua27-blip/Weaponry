from __future__ import annotations

import json
import sqlite3
from typing import Iterable, List, Optional

from forgecad_agent.infrastructure.db import JobRepository, SQLiteConnectionFactory

from ..models import (
    ErrorEnvelope,
    JobActionAuditEntry,
    JobActionListResponse,
    JobCheckpointSummary,
    JobDetail,
    JobEvent,
    JobListResponse,
    JobRuntimeStateResponse,
    JobSummary,
    ProviderTaskSummary,
)


class JobQueryError(RuntimeError):
    def __init__(self, code: str, message: str, *, recoverable: bool = False) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable


class LegacyJobQueryService:
    """Read-only legacy job use cases backed by the shared JobRepository."""

    def __init__(self, connection_factory: SQLiteConnectionFactory) -> None:
        self.connection_factory = connection_factory

    def get_job(self, job_id: str) -> JobDetail:
        with self.connection_factory.connect() as connection:
            repository = JobRepository(connection)
            row = repository.get(job_id)
            if row is None:
                raise KeyError(job_id)
            assets = repository.assets(job_id)
            version = repository.latest_version(job_id)
            model = repository.latest_model(job_id)

        return JobDetail(
            job_id=row["job_id"],
            weapon_id=row["weapon_id"],
            type=row["job_type"],
            status=row["status"],
            current_step=row["current_step"],
            created_at=row["created_at"],
            updated_at=row["updated_at"],
            outputs={
                "current_version_id": version["version_id"] if version else None,
                "current_model_id": model["model_id"] if model else None,
                "asset_ids": [asset["file_id"] for asset in assets],
                "asset_roles": {asset["file_id"]: asset["role"] for asset in assets},
            },
            error=(
                ErrorEnvelope(
                    code=row["error_code"],
                    message=row["error_message"] or row["error_code"],
                    recoverable=True,
                )
                if row["error_code"]
                else None
            ),
            events=self.list_events(job_id),
        )

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
        page_size = max(1, min(limit, 100))
        parsed_cursor = self._parse_cursor(cursor, "Invalid jobs cursor.")
        with self.connection_factory.connect() as connection:
            rows = JobRepository(connection).list_summaries(
                query=query,
                status=status,
                job_type=job_type,
                error_code=error_code,
                cursor=parsed_cursor,
                limit=page_size + 1,
            )
        page_rows = rows[:page_size]
        next_cursor = None
        if len(rows) > page_size and page_rows:
            last = page_rows[-1]
            next_cursor = f"{last['updated_at']}|{last['job_id']}"
        return JobListResponse(
            items=[
                JobSummary(
                    job_id=row["job_id"],
                    weapon_id=row["weapon_id"],
                    weapon_name=row["weapon_name"],
                    type=row["job_type"],
                    status=row["status"],  # type: ignore[arg-type]
                    current_step=row["current_step"],
                    error_code=row["error_code"],
                    error_message=row["error_message"],
                    event_count=int(row["event_count"]),
                    action_count=int(row["action_count"]),
                    latest_event_status=row["latest_event_status"],
                    latest_event_message=row["latest_event_message"],
                    latest_event_created_at=row["latest_event_created_at"],
                    output_version_id=row["output_version_id"],
                    output_model_id=row["output_model_id"],
                    created_at=row["created_at"],
                    updated_at=row["updated_at"],
                    finished_at=row["finished_at"],
                )
                for row in page_rows
            ],
            next_cursor=next_cursor,
        )

    def list_job_actions(
        self,
        job_id: str,
        *,
        cursor: Optional[str] = None,
        limit: int = 50,
    ) -> JobActionListResponse:
        page_size = max(1, min(limit, 100))
        parsed_cursor = self._parse_cursor(cursor, "Invalid job actions cursor.")
        with self.connection_factory.connect() as connection:
            repository = JobRepository(connection)
            if not repository.exists(job_id):
                raise KeyError(job_id)
            rows = repository.list_actions(job_id, cursor=parsed_cursor, limit=page_size + 1)
        page_rows = rows[:page_size]
        next_cursor = None
        if len(rows) > page_size and page_rows:
            last = page_rows[-1]
            next_cursor = f"{last['created_at']}|{last['action_id']}"
        return JobActionListResponse(
            items=[
                JobActionAuditEntry(
                    action_id=row["action_id"],
                    job_id=row["job_id"],
                    action_type=row["action_type"],  # type: ignore[arg-type]
                    requested_step=row["requested_step"],
                    status=row["status"],  # type: ignore[arg-type]
                    previous_job_status=row["previous_job_status"],
                    resulting_job_status=row["resulting_job_status"],
                    event_id=row["event_id"],
                    message=row["message"],
                    metadata=json.loads(row["metadata_json"] or "{}"),
                    created_at=row["created_at"],
                )
                for row in page_rows
            ],
            next_cursor=next_cursor,
        )

    def get_job_runtime_state(self, job_id: str) -> JobRuntimeStateResponse:
        with self.connection_factory.connect() as connection:
            repository = JobRepository(connection)
            job = repository.runtime_header(job_id)
            if job is None:
                raise KeyError(job_id)
            task_rows = repository.provider_tasks(job_id)
            checkpoint_rows = repository.checkpoints(job_id)
        status = str(job["status"])
        return JobRuntimeStateResponse(
            job_id=str(job["job_id"]),
            status=status,  # type: ignore[arg-type]
            current_step=job["current_step"],
            resumable=status in {"failed", "partial_succeeded", "waiting_user"},
            cancellable=status
            in {"created", "queued", "running", "waiting_provider", "waiting_user", "retrying"},
            provider_tasks=[self._provider_task_from_row(row) for row in task_rows],
            checkpoints=[self._checkpoint_from_row(row) for row in checkpoint_rows],
        )

    def list_events(self, job_id: str, after: Optional[str] = None) -> List[JobEvent]:
        with self.connection_factory.connect() as connection:
            repository = JobRepository(connection)
            after_seq: Optional[int] = None
            if after:
                after_seq = repository.event_cursor_seq(job_id, after)
                if after_seq is None:
                    raise JobQueryError(
                        "INVALID_EVENT_CURSOR",
                        f"Unknown event cursor for this job: {after}",
                    )
            rows = repository.events(job_id, after_seq=after_seq)
        return [self._event_from_row(row) for row in rows]

    def iter_events(self, job_id: str, after: Optional[str] = None) -> Iterable[JobEvent]:
        return iter(self.list_events(job_id, after=after))

    def has_event(self, job_id: str, event_id: str) -> bool:
        with self.connection_factory.connect() as connection:
            return JobRepository(connection).has_event(job_id, event_id)

    def has_job(self, job_id: str) -> bool:
        with self.connection_factory.connect() as connection:
            return JobRepository(connection).exists(job_id)

    @staticmethod
    def _parse_cursor(cursor: Optional[str], message: str) -> Optional[tuple[str, str]]:
        if not cursor:
            return None
        parts = cursor.split("|", 1)
        if len(parts) != 2:
            raise JobQueryError("INVALID_REQUEST", message)
        return (parts[0], parts[1])

    @staticmethod
    def _event_from_row(row: sqlite3.Row) -> JobEvent:
        metadata = json.loads(row["metadata_json"] or "{}")
        return JobEvent(
            id=row["event_id"],
            seq=row["seq"],
            job_id=row["job_id"],
            weapon_id=row["weapon_id"],
            step=row["step"],
            level=row["level"],
            status=row["status"],
            message=row["message"],
            artifact_asset_id=row["artifact_asset_id"],
            progress=metadata.get("progress"),
            metadata=metadata,
            created_at=row["created_at"],
        )

    @staticmethod
    def _provider_task_from_row(row: sqlite3.Row) -> ProviderTaskSummary:
        return ProviderTaskSummary(
            task_record_id=row["task_record_id"],
            job_id=row["job_id"],
            step=row["step_name"],
            attempt=row["attempt"],
            provider_kind=row["provider_kind"],
            provider_id=row["provider_id"],
            provider_task_id=row["provider_task_id"],
            status=row["status"],
            cancel_requested_at=row["cancel_requested_at"],
            last_seen_at=row["last_seen_at"],
            metadata=json.loads(row["metadata_json"] or "{}"),
            created_at=row["created_at"],
            updated_at=row["updated_at"],
        )

    @staticmethod
    def _checkpoint_from_row(row: sqlite3.Row) -> JobCheckpointSummary:
        return JobCheckpointSummary(
            checkpoint_id=row["checkpoint_id"],
            job_id=row["job_id"],
            step=row["step_name"],
            attempt=row["attempt"],
            status=row["status"],
            resume_policy=row["resume_policy"],
            provider_task_record_id=row["provider_task_record_id"],
            state=json.loads(row["state_json"] or "{}"),
            created_at=row["created_at"],
            updated_at=row["updated_at"],
        )
