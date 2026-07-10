from __future__ import annotations

import hashlib
import json
from datetime import datetime, timezone
from typing import Any, Optional

from forgecad_agent.application.concept_models import (
    CreateQualityRunRequest,
    QualityRunRecord,
)
from forgecad_agent.application.concept_jobs import record_completed_job
from forgecad_agent.domain.concepts.models import ModelQualityReport
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork


class ConceptQualityError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


class ConceptQualityIdempotencyConflict(RuntimeError):
    pass


class ConceptQualityService:
    """Persist version-scoped ModelQualityReport contracts for R2."""

    def __init__(self, connection_factory: SQLiteConnectionFactory) -> None:
        self.connection_factory = connection_factory

    def create_run(
        self,
        version_id: str,
        request: CreateQualityRunRequest,
        idempotency_key: str,
    ) -> QualityRunRecord:
        scope = f"POST /api/v1/versions/{version_id}/quality-runs"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            replay = unit_of_work.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise ConceptQualityIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return QualityRunRecord.model_validate_json(replay.response_json)

            version = unit_of_work.concept_projects.find_version(version_id)
            if version is None:
                raise ConceptQualityError("VERSION_NOT_FOUND", "Concept version not found.")
            report = request.report
            if report.version_id != version_id:
                raise ConceptQualityError(
                    "INVALID_REQUEST",
                    "ModelQualityReport version_id does not match the route version_id.",
                )
            if report.project_id != version["project_id"]:
                raise ConceptQualityError(
                    "INVALID_REQUEST",
                    "ModelQualityReport project_id does not match the version project.",
                )
            if version["module_graph_id"] is None:
                raise ConceptQualityError(
                    "MODULE_GRAPH_NOT_FOUND",
                    "Quality runs require a version with a validated ModuleGraph.",
                )
            if unit_of_work.quality.get_report(report.report_id) is not None:
                raise ConceptQualityError(
                    "QUALITY_RUN_CONFLICT",
                    "Quality run ID is already registered.",
                )

            now = _utc_now()
            report_json = _canonical_json(report.model_dump(mode="json"))
            unit_of_work.quality.add_report(
                quality_run_id=report.report_id,
                project_id=report.project_id,
                version_id=report.version_id,
                ruleset_version=report.ruleset_version,
                status=report.status,
                report_json=report_json,
                findings=[
                    {
                        **finding.model_dump(
                            mode="json",
                            exclude={"node_ids", "measured_value", "threshold"},
                        ),
                        "node_ids_json": _canonical_json(finding.node_ids),
                        "measured_value_json": _optional_json(finding.measured_value),
                        "threshold_json": _optional_json(finding.threshold),
                    }
                    for finding in report.findings
                ],
                created_at=now,
            )
            response = QualityRunRecord(
                quality_run_id=report.report_id,
                project_id=report.project_id,
                version_id=report.version_id,
                report=report,
                created_at=now,
            )
            job_id = record_completed_job(
                unit_of_work,
                project_id=report.project_id,
                version_id=report.version_id,
                job_type="quality_run",
                input_payload={
                    "quality_run_id": report.report_id,
                    "ruleset_version": report.ruleset_version,
                },
                output_payload={
                    "quality_run_id": report.report_id,
                    "report_id": report.report_id,
                    "status": report.status,
                    "finding_count": len(report.findings),
                },
                steps=[
                    (
                        "load_version_graph",
                        "Loaded version-scoped ModuleGraph context.",
                        0.3,
                        {"version_id": report.version_id},
                    ),
                    (
                        "persist_findings",
                        "Persisted quality findings.",
                        0.75,
                        {"finding_count": len(report.findings)},
                    ),
                    (
                        "finalize_report",
                        "Stored ModelQualityReport@1.",
                        1.0,
                        {"status": report.status},
                    ),
                ],
            )
            response = response.model_copy(update={"job_id": job_id})
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=now,
            )
            return response

    def get_run(self, quality_run_id: str) -> QualityRunRecord:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            row = unit_of_work.quality.get_report(quality_run_id)
            if row is None:
                raise ConceptQualityError("QUALITY_RUN_NOT_FOUND", "Quality run not found.")
            return QualityRunRecord(
                quality_run_id=row["quality_run_id"],
                project_id=row["project_id"],
                version_id=row["version_id"],
                report=ModelQualityReport.model_validate_json(row["report_json"]),
                created_at=row["created_at"],
            )


def _optional_json(value: Any) -> Optional[str]:
    return None if value is None else _canonical_json(value)


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _hash_json(value: Any) -> str:
    return hashlib.sha256(_canonical_json(value).encode("utf-8")).hexdigest()


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()
