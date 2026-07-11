from __future__ import annotations

import csv
import hashlib
import io
import json
import uuid
import zipfile
from datetime import datetime, timezone
from typing import Any

from forgecad_agent.application.concept_change_sets import (
    change_set_timeline_item_from_row,
)
from forgecad_agent.application.concept_jobs import record_completed_job
from forgecad_agent.application.concept_models import (
    ChangeSetAuditExportFileEntry,
    ChangeSetAuditExportListResponse,
    ChangeSetAuditExportManifest,
    ChangeSetAuditExportRecord,
    CreateChangeSetAuditExportRequest,
)
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork
from forgecad_agent.infrastructure.storage import ContentAddressedStore, ObjectStoreError


class ChangeSetAuditExportError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


class ChangeSetAuditExportIdempotencyConflict(RuntimeError):
    pass


class ConceptChangeSetAuditService:
    """Create and retrieve immutable project-lifetime ChangeSet audit packages."""

    def __init__(
        self,
        connection_factory: SQLiteConnectionFactory,
        object_store: ContentAddressedStore,
    ) -> None:
        self.connection_factory = connection_factory
        self.object_store = object_store

    def create_export(
        self,
        project_id: str,
        request: CreateChangeSetAuditExportRequest,
        idempotency_key: str,
    ) -> ChangeSetAuditExportRecord:
        scope = f"POST /api/v1/projects/{project_id}/change-set-audit-exports"
        request_hash = _hash_json(request.model_dump(mode="json"))
        normalized_query = (
            request.query.strip().lower()
            if request.query is not None and request.query.strip()
            else None
        )
        filters = {
            "query": normalized_query,
            "status": request.status,
            "operation": request.operation,
        }
        created_at = _utc_now()
        audit_export_id = _new_id("csaudit")

        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            replay = unit_of_work.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise ChangeSetAuditExportIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return ChangeSetAuditExportRecord.model_validate_json(replay.response_json)

            # Freeze project existence and the ordered ChangeSet query in the
            # same local write transaction as the archive metadata.
            unit_of_work.require_connection().execute("BEGIN IMMEDIATE")
            if unit_of_work.concept_projects.get_active(project_id) is None:
                raise ChangeSetAuditExportError("PROJECT_NOT_FOUND", "Concept project not found.")
            rows = unit_of_work.change_sets.list_for_project(
                project_id,
                limit=request.max_records + 1,
                query=normalized_query,
                status=request.status,
                operation=request.operation,
            )
            if len(rows) > request.max_records:
                raise ChangeSetAuditExportError(
                    "AUDIT_EXPORT_LIMIT_EXCEEDED",
                    (
                        "Matching ChangeSets exceed max_records; narrow the filters "
                        "or raise max_records within the 10,000 record limit."
                    ),
                )

            records = [_audit_record_from_row(row) for row in rows]
            jsonl_payload = (
                "\n".join(_canonical_json(record) for record in records) + ("\n" if records else "")
            ).encode("utf-8")
            files: dict[str, tuple[bytes, str]] = {
                "README.txt": (_readme_payload(), "text/plain; charset=utf-8"),
                "Records/change-sets.jsonl": (
                    jsonl_payload,
                    "application/x-ndjson",
                ),
            }
            if request.include_csv:
                files["Records/change-sets.csv"] = (
                    _csv_payload(records),
                    "text/csv; charset=utf-8",
                )

            manifest = ChangeSetAuditExportManifest(
                audit_export_id=audit_export_id,
                project_id=project_id,
                filters=filters,
                record_count=len(records),
                retention_class=request.retention_class,
                files=[
                    ChangeSetAuditExportFileEntry(
                        path=path,
                        sha256=hashlib.sha256(payload).hexdigest(),
                        byte_size=len(payload),
                        mime_type=mime_type,
                    )
                    for path, (payload, mime_type) in sorted(files.items())
                ],
                created_at=created_at,
            )
            manifest_json = _canonical_json(manifest.model_dump(mode="json"))
            files["Manifest/change-set-audit-export.json"] = (
                manifest_json.encode("utf-8"),
                "application/json",
            )
            package = _build_zip(files)
            stored = self.object_store.put(package, extension=".zip")
            package_asset_id = _new_id("asset")
            unit_of_work.concept_assets.add(
                asset_id=package_asset_id,
                project_id=project_id,
                version_id=None,
                role="project_report",
                logical_path=f"Audit/{audit_export_id}.zip",
                object_path=stored.relative_path,
                sha256=stored.sha256,
                byte_size=stored.byte_size,
                mime_type="application/zip",
                metadata_json=_canonical_json(
                    {
                        "schema_version": manifest.schema_version,
                        "audit_export_id": audit_export_id,
                        "record_count": len(records),
                        "retention_class": request.retention_class,
                    }
                ),
                created_at=created_at,
            )
            unit_of_work.change_set_audit_exports.add(
                audit_export_id=audit_export_id,
                project_id=project_id,
                package_asset_id=package_asset_id,
                filters_json=_canonical_json(filters),
                manifest_json=manifest_json,
                record_count=len(records),
                retention_class=request.retention_class,
                created_at=created_at,
            )
            unit_of_work.exports.add_artifact_link(
                project_id=project_id,
                version_id=None,
                asset_id=package_asset_id,
                relation="change_set_audit_package",
                created_at=created_at,
            )
            job_id = record_completed_job(
                unit_of_work,
                project_id=project_id,
                version_id=None,
                job_type="export_package",
                input_payload=request.model_dump(mode="json"),
                output_payload={
                    "audit_export_id": audit_export_id,
                    "package_asset_id": package_asset_id,
                    "package_sha256": stored.sha256,
                    "record_count": len(records),
                },
                steps=(
                    (
                        "collect",
                        "Collected the filtered ChangeSet audit snapshot.",
                        0.35,
                        {"record_count": len(records), "filters": filters},
                    ),
                    (
                        "manifest",
                        "Hashed audit records and validated the archive manifest.",
                        0.72,
                        {"schema_version": manifest.schema_version},
                    ),
                    (
                        "package",
                        "Stored the immutable project-lifetime audit package.",
                        1.0,
                        {"audit_export_id": audit_export_id},
                    ),
                ),
                artifact_asset_id=package_asset_id,
            )
            response = ChangeSetAuditExportRecord(
                audit_export_id=audit_export_id,
                project_id=project_id,
                status="validated",
                retention_class=request.retention_class,
                record_count=len(records),
                filters=filters,
                job_id=job_id,
                package_asset_id=package_asset_id,
                package_sha256=stored.sha256,
                package_byte_size=stored.byte_size,
                manifest=manifest,
                created_at=created_at,
            )
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=created_at,
            )
            return response

    def get_export(self, audit_export_id: str) -> ChangeSetAuditExportRecord:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            row = unit_of_work.change_set_audit_exports.get(audit_export_id)
            if row is None:
                raise ChangeSetAuditExportError(
                    "AUDIT_EXPORT_NOT_FOUND", "ChangeSet audit export not found."
                )
            return _export_record_from_row(row)

    def list_for_project(
        self,
        project_id: str,
        *,
        limit: int = 50,
    ) -> ChangeSetAuditExportListResponse:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            if unit_of_work.concept_projects.get_active(project_id) is None:
                raise ChangeSetAuditExportError("PROJECT_NOT_FOUND", "Concept project not found.")
            rows = unit_of_work.change_set_audit_exports.list_for_project(project_id, limit=limit)
            return ChangeSetAuditExportListResponse(
                items=[_export_record_from_row(row) for row in rows]
            )

    def read_export(self, audit_export_id: str) -> tuple[bytes, str, str]:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            row = unit_of_work.change_set_audit_exports.get(audit_export_id)
            if row is None:
                raise ChangeSetAuditExportError(
                    "AUDIT_EXPORT_NOT_FOUND", "ChangeSet audit export not found."
                )
            try:
                payload = self.object_store.read(
                    str(row["object_path"]),
                    expected_sha256=str(row["package_sha256"]),
                )
            except ObjectStoreError as exc:
                raise ChangeSetAuditExportError(
                    "AUDIT_EXPORT_PACKAGE_UNAVAILABLE",
                    f"ChangeSet audit package is unavailable: {exc}",
                ) from exc
            return (
                payload,
                f"{audit_export_id}.zip",
                str(row["package_sha256"]),
            )


def _audit_record_from_row(row: Any) -> dict[str, Any]:
    item = change_set_timeline_item_from_row(row)
    return {
        "schema_version": "ChangeSetAuditRecord@1",
        **item.model_dump(mode="json"),
        "change_set_sha256": str(row["change_set_sha256"]),
    }


def _export_record_from_row(row: Any) -> ChangeSetAuditExportRecord:
    return ChangeSetAuditExportRecord(
        audit_export_id=str(row["audit_export_id"]),
        project_id=str(row["project_id"]),
        status=str(row["status"]),
        retention_class=str(row["retention_class"]),
        record_count=int(row["record_count"]),
        filters=json.loads(row["filters_json"]),
        job_id=str(row["job_id"]) if row["job_id"] is not None else None,
        package_asset_id=str(row["package_asset_id"]),
        package_sha256=str(row["package_sha256"]),
        package_byte_size=int(row["package_byte_size"]),
        manifest=ChangeSetAuditExportManifest.model_validate_json(row["manifest_json"]),
        created_at=str(row["created_at"]),
    )


def _csv_payload(records: list[dict[str, Any]]) -> bytes:
    output = io.StringIO(newline="")
    fields = [
        "change_set_id",
        "base_version_id",
        "result_version_id",
        "status",
        "actor_type",
        "summary",
        "operations",
        "node_ids",
        "provider_id",
        "provider_type",
        "model",
        "fallback_used",
        "planner_instruction",
        "diagnostic_code",
        "created_at",
        "updated_at",
        "confirmed_at",
        "change_set_sha256",
    ]
    writer = csv.DictWriter(output, fieldnames=fields, lineterminator="\n")
    writer.writeheader()
    for record in records:
        change_set = record["change_set"]
        operations = change_set["operations"]
        provenance = record.get("planner_provenance") or {}
        diagnostic = record.get("diagnostic") or {}
        node_ids = sorted(
            {
                node_id
                for operation in operations
                for node_id in (
                    operation.get("node_id"),
                    operation.get("from_node_id"),
                    operation.get("to_node_id"),
                )
                if node_id
            }
        )
        row = {
            "change_set_id": change_set["change_set_id"],
            "base_version_id": record["base_version_id"],
            "result_version_id": record.get("result_version_id") or "",
            "status": record["status"],
            "actor_type": record["actor_type"],
            "summary": change_set["summary"],
            "operations": "|".join(item["op"] for item in operations),
            "node_ids": "|".join(node_ids),
            "provider_id": provenance.get("provider_id", ""),
            "provider_type": provenance.get("provider_type", ""),
            "model": provenance.get("model") or "",
            "fallback_used": provenance.get("fallback_used", ""),
            "planner_instruction": record.get("planner_instruction") or "",
            "diagnostic_code": diagnostic.get("code", ""),
            "created_at": record["created_at"],
            "updated_at": record["updated_at"],
            "confirmed_at": record.get("confirmed_at") or "",
            "change_set_sha256": record["change_set_sha256"],
        }
        writer.writerow({key: _csv_safe_cell(value) for key, value in row.items()})
    return output.getvalue().encode("utf-8-sig")


def _csv_safe_cell(value: Any) -> Any:
    if isinstance(value, str) and value.lstrip().startswith(
        ("=", "+", "-", "@", "\t", "\r")
    ):
        return f"'{value}"
    return value


def _readme_payload() -> bytes:
    return (
        "ForgeCAD ChangeSet audit export\n"
        "\n"
        "Records/change-sets.jsonl is the canonical machine-readable snapshot.\n"
        "Records/change-sets.csv is an optional review-friendly projection.\n"
        "Manifest/change-set-audit-export.json records filters, ordering, hashes, and sizes.\n"
        "Retention class project_lifetime means the application exposes no delete operation;\n"
        "the package logically belongs to its Project until that Project is removed. Object\n"
        "bytes may remain until reference-aware garbage collection. This is not a regulatory\n"
        "WORM archive, legal hold, or independent disaster-recovery copy.\n"
    ).encode("utf-8")


def _build_zip(files: dict[str, tuple[bytes, str]]) -> bytes:
    output = io.BytesIO()
    with zipfile.ZipFile(output, mode="w", compression=zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(files):
            info = zipfile.ZipInfo(path, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, files[path][0])
    return output.getvalue()


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _hash_json(value: Any) -> str:
    return hashlib.sha256(_canonical_json(value).encode("utf-8")).hexdigest()


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex[:12]}"


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()
