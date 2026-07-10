from __future__ import annotations

import hashlib
import json
from datetime import datetime, timezone
from typing import Any, Optional
from uuid import uuid4

from forgecad_agent.application.concept_models import (
    CreateQualityRunRequest,
    InspectConceptVersionRequest,
    QualityRunRecord,
)
from forgecad_agent.application.concept_jobs import record_completed_job
from forgecad_agent.application.mesh_quality_inspector import (
    ModuleInspectionSource,
    inspect_concept_geometry,
)
from forgecad_agent.domain.concepts.models import (
    ModelQualityReport,
    ModuleAssetManifest,
    ModuleGraph,
    WeaponConceptSpec,
)
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork
from forgecad_agent.infrastructure.storage import ContentAddressedStore, ObjectStoreError


class ConceptQualityError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


class ConceptQualityIdempotencyConflict(RuntimeError):
    pass


class ConceptQualityService:
    """Persist reports and inspect immutable version-scoped concept geometry."""

    def __init__(
        self,
        connection_factory: SQLiteConnectionFactory,
        object_store: Optional[ContentAddressedStore] = None,
    ) -> None:
        self.connection_factory = connection_factory
        self.object_store = object_store

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
                            exclude={
                                "node_ids",
                                "geometry_refs",
                                "measured_value",
                                "threshold",
                            },
                        ),
                        "node_ids_json": _canonical_json(finding.node_ids),
                        "geometry_refs_json": _canonical_json(
                            [item.model_dump(mode="json") for item in finding.geometry_refs]
                        ),
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

    def inspect_version(
        self,
        version_id: str,
        request: InspectConceptVersionRequest,
        idempotency_key: str,
    ) -> QualityRunRecord:
        scope = f"POST /api/v1/versions/{version_id}/quality-runs:inspect"
        request_hash = _hash_json(request.model_dump(mode="json"))
        if self.object_store is None:
            raise ConceptQualityError(
                "QUALITY_INSPECTOR_UNAVAILABLE",
                "The quality inspector requires the immutable object store.",
            )
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
            graph_id = version["module_graph_id"]
            if graph_id is None:
                raise ConceptQualityError(
                    "MODULE_GRAPH_NOT_FOUND",
                    "Quality runs require a version with a validated ModuleGraph.",
                )
            graph_row = unit_of_work.modules.get_graph(str(graph_id))
            if graph_row is None:
                raise ConceptQualityError(
                    "MODULE_GRAPH_NOT_FOUND",
                    "The version ModuleGraph is unavailable.",
                )
            graph = ModuleGraph.model_validate_json(graph_row["graph_json"])
            spec = WeaponConceptSpec.model_validate_json(version["spec_json"])
            sources: list[ModuleInspectionSource] = []
            for node in graph.nodes:
                module_row = unit_of_work.modules.get_manifest(node.module_id)
                if module_row is None:
                    raise ConceptQualityError(
                        "MODULE_ASSET_UNAVAILABLE",
                        f"Module asset is unavailable: {node.module_id}",
                    )
                try:
                    payload = self.object_store.read(
                        str(module_row["object_path"]),
                        expected_sha256=str(module_row["sha256"]),
                    )
                except ObjectStoreError as exc:
                    raise ConceptQualityError(
                        "MODULE_ASSET_UNAVAILABLE",
                        f"Module GLB is unavailable: {node.module_id}: {exc}",
                    ) from exc
                sources.append(
                    ModuleInspectionSource(
                        node_id=node.node_id,
                        manifest=ModuleAssetManifest.model_validate_json(
                            module_row["manifest_json"]
                        ),
                        payload=payload,
                    )
                )

            now = _utc_now()
            findings = [
                finding.model_copy(update={"finding_id": f"finding_{uuid4().hex}"})
                for finding in inspect_concept_geometry(
                    graph=graph, sources=sources, spec=spec
                )
            ]
            finding_statuses = {finding.status for finding in findings}
            status = (
                "failed"
                if "failed" in finding_statuses
                else "warning"
                if "warning" in finding_statuses
                else "passed"
            )
            report = ModelQualityReport(
                report_id=f"quality_{uuid4().hex}",
                project_id=str(version["project_id"]),
                version_id=version_id,
                ruleset_version=request.ruleset_version,
                status=status,
                findings=findings,
                created_at=now,
            )
            _add_report(unit_of_work, report, now)
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
                    "source": "server_geometry_inspector",
                },
                output_payload={
                    "quality_run_id": report.report_id,
                    "status": report.status,
                    "finding_count": len(report.findings),
                    "geometry_reference_count": sum(
                        len(finding.geometry_refs) for finding in report.findings
                    ),
                },
                steps=[
                    (
                        "load_immutable_assets",
                        "Loaded version-scoped ModuleGraph and content-addressed GLBs.",
                        0.25,
                        {"node_count": len(graph.nodes)},
                    ),
                    (
                        "inspect_meshes",
                        "Checked indices, triangles, normals, UV0, topology, hidden geometry, density, LOD0, and bounds.",
                        0.62,
                        {
                            "module_count": len(sources),
                            "triangle_budget": spec.constraints.max_triangle_count,
                        },
                    ),
                    (
                        "inspect_assembly",
                        "Checked symmetry, Connector alignment, connected surface gaps, and exact unconnected intersections.",
                        0.86,
                        {
                            "edge_count": len(graph.edges),
                            "geometry_reference_count": sum(
                                len(finding.geometry_refs) for finding in findings
                            ),
                        },
                    ),
                    (
                        "finalize_report",
                        "Stored ModelQualityReport@1.",
                        1.0,
                        {"status": report.status, "finding_count": len(findings)},
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


def _add_report(
    unit_of_work: SQLiteUnitOfWork,
    report: ModelQualityReport,
    created_at: str,
) -> None:
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
                    exclude={
                        "node_ids",
                        "geometry_refs",
                        "measured_value",
                        "threshold",
                    },
                ),
                "node_ids_json": _canonical_json(finding.node_ids),
                "geometry_refs_json": _canonical_json(
                    [item.model_dump(mode="json") for item in finding.geometry_refs]
                ),
                "measured_value_json": _optional_json(finding.measured_value),
                "threshold_json": _optional_json(finding.threshold),
            }
            for finding in report.findings
        ],
        created_at=created_at,
    )
