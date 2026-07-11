from __future__ import annotations

import hashlib
import json
import uuid
from datetime import datetime, timedelta, timezone
from typing import Any, Iterable, Optional

from forgecad_agent.application.concept_models import (
    ConceptJobEventListResponse,
    ConceptJobRecord,
    InspectConceptVersionRequest,
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

    def enqueue_quality_inspection(
        self,
        version_id: str,
        request: InspectConceptVersionRequest,
        idempotency_key: str,
    ) -> ConceptJobRecord:
        scope = f"POST /api/v1/versions/{version_id}/quality-runs:inspect:enqueue"
        input_payload = {
            "version_id": version_id,
            "ruleset_version": request.ruleset_version,
        }
        input_json = _canonical_json(input_payload)
        input_hash = hashlib.sha256(input_json.encode("utf-8")).hexdigest()
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            replay = unit_of_work.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != input_hash:
                    raise ConceptJobError("IDEMPOTENCY_CONFLICT", "Idempotency-Key was reused with a different request body.")
                return ConceptJobRecord.model_validate_json(replay.response_json)
            version = unit_of_work.concept_projects.find_version(version_id)
            if version is None:
                raise ConceptJobError("VERSION_NOT_FOUND", "Concept version not found.")
            if version["module_graph_id"] is None:
                raise ConceptJobError("MODULE_GRAPH_NOT_FOUND", "Quality runs require a version with a validated ModuleGraph.")
            now = _utc_now()
            job_id = _new_id("job")
            unit_of_work.concept_jobs.add_job(
                job_id=job_id,
                project_id=str(version["project_id"]),
                version_id=version_id,
                job_type="quality_run",
                status="queued",
                current_step="queued",
                input_hash=input_hash,
                input_json=input_json,
                output_json="{}",
                created_at=now,
                finished_at=None,
            )
            unit_of_work.concept_jobs.add_work_item(
                job_id=job_id,
                task_type="inspect_quality",
                payload_json=input_json,
                created_at=now,
            )
            unit_of_work.concept_jobs.append_event(
                event_id=_new_id("evt"), job_id=job_id, project_id=str(version["project_id"]),
                version_id=version_id, step="queued", level="info", status="queued",
                message="Quality inspection queued for the local Concept worker.", progress=0.0,
                metadata_json=_canonical_json({"task_type": "inspect_quality"}), created_at=now,
            )
            response = _job_from_row(unit_of_work.concept_jobs.get_job(job_id), events=[])
            unit_of_work.idempotency.add(
                scope=scope, key=idempotency_key, request_hash=input_hash,
                response_json=_canonical_json(response.model_dump(mode="json")), created_at=now,
            )
            return response

    def cancel_queued_job(self, job_id: str) -> ConceptJobRecord:
        now = _utc_now()
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            before = unit_of_work.concept_jobs.get_job(job_id)
            if before is None:
                raise ConceptJobError("CONCEPT_JOB_NOT_FOUND", "Concept job not found.")
            row = unit_of_work.concept_jobs.cancel_queued_work_item(job_id, now=now)
            if row is None:
                raise ConceptJobError("JOB_ACTION_CONFLICT", "Only queued Concept worker jobs can be cancelled.")
            unit_of_work.concept_jobs.append_event(
                event_id=_new_id("evt"), job_id=job_id, project_id=str(row["project_id"]),
                version_id=row["version_id"], step="cancelled", level="warning", status="cancelled",
                message="Queued Concept worker job cancelled before execution.", progress=0.0,
                metadata_json=_canonical_json({"previous_status": before["status"], "action": "cancel"}), created_at=now,
            )
            return _job_from_row(row, events=[_event_from_row(item) for item in unit_of_work.concept_jobs.events(job_id)])

    def retry_job(self, job_id: str) -> ConceptJobRecord:
        now = _utc_now()
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            before = unit_of_work.concept_jobs.get_job(job_id)
            if before is None:
                raise ConceptJobError("CONCEPT_JOB_NOT_FOUND", "Concept job not found.")
            row = unit_of_work.concept_jobs.retry_work_item(job_id, now=now)
            if row is None:
                raise ConceptJobError("JOB_ACTION_CONFLICT", "Only failed or cancelled Concept worker jobs can be retried.")
            unit_of_work.concept_jobs.append_event(
                event_id=_new_id("evt"), job_id=job_id, project_id=str(row["project_id"]),
                version_id=row["version_id"], step="retry_requested", level="info", status="retrying",
                message="Concept worker retry requested.", progress=0.0,
                metadata_json=_canonical_json({"previous_status": before["status"], "action": "retry"}), created_at=now,
            )
            return _job_from_row(row, events=[_event_from_row(item) for item in unit_of_work.concept_jobs.events(job_id)])

    def recover_interrupted_work(self, *, force: bool = False) -> list[str]:
        now = _utc_now()
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            rows = unit_of_work.concept_jobs.recover_expired_work_items(now=now, force=force)
            for row in rows:
                unit_of_work.concept_jobs.append_event(
                    event_id=_new_id("evt"), job_id=str(row["job_id"]), project_id=str(row["project_id"]),
                    version_id=row["version_id"], step="recovery", level="warning", status="queued",
                    message="Interrupted Concept worker job requeued for recovery.", progress=0.0,
                    metadata_json=_canonical_json({"previous_step": row["current_step"]}), created_at=now,
                )
            return [str(row["job_id"]) for row in rows]

    def run_next_quality_inspection(self, quality_service: Any, *, runner_id: str) -> Optional[ConceptJobRecord]:
        now = _utc_now()
        lease_expires_at = (datetime.now(timezone.utc) + timedelta(minutes=5)).isoformat()
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            row = unit_of_work.concept_jobs.claim_next_work_item(
                runner_id=runner_id, now=now, lease_expires_at=lease_expires_at,
            )
            if row is None:
                return None
            unit_of_work.concept_jobs.append_event(
                event_id=_new_id("evt"), job_id=str(row["job_id"]), project_id=str(row["project_id"]),
                version_id=row["version_id"], step="claim", level="info", status="running",
                message="Local Concept worker claimed quality inspection.", progress=0.1,
                metadata_json=_canonical_json({"runner_id": runner_id}), created_at=now,
            )
        payload = json.loads(row["payload_json"])
        job_id = str(row["job_id"])
        try:
            if row["task_type"] != "inspect_quality":
                raise ConceptJobError("UNSUPPORTED_CONCEPT_TASK", f"Unsupported task: {row['task_type']}")
            result = quality_service.inspect_version(
                str(payload["version_id"]),
                InspectConceptVersionRequest(
                    client_request_id=f"concept-quality-worker-{job_id}",
                    ruleset_version=str(payload["ruleset_version"]),
                ),
                f"concept-quality-worker-{job_id}",
                record_job=False,
                report_id=f"quality_{job_id}",
            )
        except Exception as exc:
            code = exc.code if isinstance(exc, ConceptJobError) else getattr(exc, "code", "CONCEPT_WORKER_FAILED")
            return self._finish_quality_work(job_id, status="failed", output={}, error_code=str(code), error_message=str(exc))
        return self._finish_quality_work(
            job_id, status="succeeded",
            output={"quality_run_id": result.quality_run_id, "status": result.report.status,
                    "finding_count": len(result.report.findings)},
            error_code=None, error_message=None,
        )

    def _finish_quality_work(self, job_id: str, *, status: str, output: dict[str, Any], error_code: Optional[str], error_message: Optional[str]) -> ConceptJobRecord:
        now = _utc_now()
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            row = unit_of_work.concept_jobs.get_job(job_id)
            if row is None:
                raise ConceptJobError("CONCEPT_JOB_NOT_FOUND", "Concept job not found.")
            unit_of_work.concept_jobs.finish_work_item(
                job_id=job_id, status=status, current_step="finalize" if status == "succeeded" else "failed",
                output_json=_canonical_json(output), error_code=error_code, error_message=error_message,
                finished_at=now, updated_at=now,
            )
            unit_of_work.concept_jobs.append_event(
                event_id=_new_id("evt"), job_id=job_id, project_id=str(row["project_id"]),
                version_id=row["version_id"], step="finalize", level="info" if status == "succeeded" else "error",
                status=status, message="Quality inspection completed." if status == "succeeded" else "Quality inspection failed.",
                progress=1.0 if status == "succeeded" else 0.1,
                metadata_json=_canonical_json(output if status == "succeeded" else {"error_code": error_code}), created_at=now,
            )
            updated = unit_of_work.concept_jobs.get_job(job_id)
            return _job_from_row(updated, events=[_event_from_row(item) for item in unit_of_work.concept_jobs.events(job_id)])


def record_completed_job(
    unit_of_work: SQLiteUnitOfWork,
    *,
    project_id: str,
    version_id: Optional[str],
    job_type: str,
    input_payload: dict[str, Any],
    output_payload: dict[str, Any],
    steps: Iterable[tuple[str, str, float, dict[str, Any]]],
    artifact_asset_id: Optional[str] = None,
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
        event_artifact_asset_id = artifact_asset_id if seq == len(step_list) else None
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
            artifact_asset_id=event_artifact_asset_id,
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
            artifact_asset_id=event_artifact_asset_id,
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
