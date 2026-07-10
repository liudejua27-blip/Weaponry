from __future__ import annotations

import hashlib
import json
import uuid
from datetime import datetime, timezone
from typing import Any, Iterable, Optional

from forgecad_agent.application.concept_models import (
    ConceptJobEventListResponse,
    ConceptJobRecord,
)
from forgecad_agent.domain.concepts.models import JobEventV2
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork


class ConceptJobError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


class ConceptJobService:
    def __init__(self, connection_factory: SQLiteConnectionFactory) -> None:
        self.connection_factory = connection_factory

    def get_job(self, job_id: str) -> ConceptJobRecord:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            row = unit_of_work.concept_jobs.get_job(job_id)
            if row is None:
                raise ConceptJobError("CONCEPT_JOB_NOT_FOUND", "Concept job not found.")
            events = [_event_from_row(event) for event in unit_of_work.concept_jobs.events(job_id)]
            return _job_from_row(row, events=events)

    def list_events(
        self,
        job_id: str,
        *,
        after: Optional[str] = None,
    ) -> ConceptJobEventListResponse:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            if unit_of_work.concept_jobs.get_job(job_id) is None:
                raise ConceptJobError("CONCEPT_JOB_NOT_FOUND", "Concept job not found.")
            after_seq: Optional[int] = None
            if after:
                after_seq = unit_of_work.concept_jobs.event_seq(job_id, after)
                if after_seq is None:
                    raise ConceptJobError(
                        "INVALID_EVENT_CURSOR",
                        "Concept job event cursor does not belong to this job.",
                    )
            rows = unit_of_work.concept_jobs.events(job_id, after_seq=after_seq)
            return ConceptJobEventListResponse(
                items=[_event_from_row(row) for row in rows],
                next_cursor=None,
            )


def record_completed_job(
    unit_of_work: SQLiteUnitOfWork,
    *,
    project_id: str,
    version_id: Optional[str],
    job_type: str,
    input_payload: dict[str, Any],
    output_payload: dict[str, Any],
    steps: Iterable[tuple[str, str, float, dict[str, Any]]],
) -> str:
    job_id = _new_id("job")
    now = _utc_now()
    input_json = _canonical_json(input_payload)
    unit_of_work.concept_jobs.add_job(
        job_id=job_id,
        project_id=project_id,
        version_id=version_id,
        job_type=job_type,
        status="succeeded",
        current_step="finalize",
        input_hash=hashlib.sha256(input_json.encode("utf-8")).hexdigest(),
        input_json=input_json,
        output_json=_canonical_json(output_payload),
        created_at=now,
        finished_at=now,
    )
    step_list = list(steps)
    for seq, (step, message, progress, metadata) in enumerate(step_list, start=1):
        event = JobEventV2(
            event_id=f"evt_{job_id}_{seq:04d}",
            job_id=job_id,
            seq=seq,
            project_id=project_id,
            version_id=version_id,
            step=step,
            level="info",
            status="succeeded",
            message=message,
            progress=progress,
            artifact_asset_id=None,
            metadata=metadata,
            created_at=now,
        )
        unit_of_work.concept_jobs.add_event(
            event_id=event.event_id,
            job_id=job_id,
            seq=seq,
            project_id=project_id,
            version_id=version_id,
            step=step,
            level=event.level,
            status=event.status,
            message=message,
            progress=progress,
            artifact_asset_id=None,
            metadata_json=_canonical_json(metadata),
            created_at=now,
        )
    return job_id


def _job_from_row(row: Any, *, events: list[JobEventV2]) -> ConceptJobRecord:
    return ConceptJobRecord(
        job_id=row["job_id"],
        project_id=row["project_id"],
        version_id=row["version_id"],
        type=row["job_type"],
        status=row["status"],
        current_step=row["current_step"],
        input_hash=row["input_hash"],
        input=json.loads(row["input_json"]),
        outputs=json.loads(row["output_json"]),
        error_code=row["error_code"],
        error_message=row["error_message"],
        created_at=row["created_at"],
        updated_at=row["updated_at"],
        finished_at=row["finished_at"],
        events=events,
    )


def _event_from_row(row: Any) -> JobEventV2:
    return JobEventV2(
        event_id=row["event_id"],
        job_id=row["job_id"],
        seq=row["seq"],
        project_id=row["project_id"],
        version_id=row["version_id"],
        step=row["step"],
        level=row["level"],
        status=row["status"],
        message=row["message"],
        progress=row["progress"],
        artifact_asset_id=row["artifact_asset_id"],
        metadata=json.loads(row["metadata_json"]),
        created_at=row["created_at"],
    )


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex[:12]}"


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()
