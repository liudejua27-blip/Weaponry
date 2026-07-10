from __future__ import annotations

import hashlib
import io
import json
import uuid
import zipfile
from datetime import datetime, timezone
from typing import Any, Optional

from forgecad_agent.application.concept_jobs import record_completed_job
from forgecad_agent.application.concept_models import (
    ConceptExportRecord,
    CreateConceptExportRequest,
)
from forgecad_agent.domain.concepts.models import (
    ConceptExportManifest,
    ExportFileEntry,
    ExportModuleEntry,
    ModuleAssetManifest,
    ModuleGraph,
)
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork
from forgecad_agent.infrastructure.storage import ContentAddressedStore, ObjectStoreError


class ConceptExportError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


class ConceptExportIdempotencyConflict(RuntimeError):
    pass


class ConceptExportService:
    """Create immutable, non-functional concept delivery packages."""

    def __init__(
        self,
        connection_factory: SQLiteConnectionFactory,
        object_store: ContentAddressedStore,
    ) -> None:
        self.connection_factory = connection_factory
        self.object_store = object_store

    def create_export(
        self,
        version_id: str,
        request: CreateConceptExportRequest,
        idempotency_key: str,
    ) -> ConceptExportRecord:
        scope = f"POST /api/v1/versions/{version_id}/exports"
        request_hash = _hash_json(request.model_dump(mode="json"))

        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            replay = unit_of_work.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise ConceptExportIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return ConceptExportRecord.model_validate_json(replay.response_json)

            version = unit_of_work.concept_projects.find_version(version_id)
            if version is None:
                raise ConceptExportError("VERSION_NOT_FOUND", "Concept version not found.")
            graph_id = version["module_graph_id"]
            if graph_id is None:
                raise ConceptExportError(
                    "MODULE_GRAPH_NOT_FOUND",
                    "Concept export requires a version with a validated ModuleGraph.",
                )
            graph_row = unit_of_work.modules.get_graph(str(graph_id))
            if graph_row is None or graph_row["validation_status"] != "valid":
                raise ConceptExportError(
                    "MODULE_GRAPH_NOT_FOUND",
                    "The version ModuleGraph is missing or invalid.",
                )

            project_id = str(version["project_id"])
            graph = ModuleGraph.model_validate_json(graph_row["graph_json"])
            export_id = _new_id("export")
            package_asset_id = _new_id("asset")
            created_at = _utc_now()
            files: dict[str, tuple[bytes, str]] = {}

            spec_payload = _canonical_json_bytes(json.loads(version["spec_json"]))
            graph_payload = _canonical_json_bytes(graph.model_dump(mode="json"))
            files["Specs/weapon-concept-spec.json"] = (spec_payload, "application/json")
            files["Graphs/module-graph.json"] = (graph_payload, "application/json")

            module_entries: list[ExportModuleEntry] = []
            for node in graph.nodes:
                module_row = unit_of_work.modules.get_manifest(node.module_id)
                if module_row is None:
                    raise ConceptExportError(
                        "MODULE_NOT_FOUND",
                        f"ModuleGraph references an unavailable module: {node.module_id}",
                    )
                manifest = ModuleAssetManifest.model_validate_json(module_row["manifest_json"])
                asset_row = unit_of_work.concept_assets.get_active(manifest.asset_id)
                if asset_row is None:
                    raise ConceptExportError(
                        "MODULE_ASSET_NOT_FOUND",
                        f"Module asset is unavailable: {manifest.asset_id}",
                    )
                try:
                    payload = self.object_store.read(
                        str(asset_row["object_path"]),
                        expected_sha256=str(asset_row["sha256"]),
                    )
                except ObjectStoreError as exc:
                    raise ConceptExportError(
                        "EXPORT_SOURCE_UNAVAILABLE",
                        f"Cannot read immutable module source {manifest.asset_id}: {exc}",
                    ) from exc
                logical_path = f"Modules/{node.node_id}.glb"
                files[logical_path] = (payload, "model/gltf-binary")
                module_entries.append(
                    ExportModuleEntry(
                        node_id=node.node_id,
                        module_id=node.module_id,
                        asset_id=manifest.asset_id,
                        sha256=str(asset_row["sha256"]),
                        logical_path=logical_path,
                        transform=node.transform,
                    )
                )

            quality_report_id: Optional[str] = None
            if request.include_quality_report:
                quality_row = unit_of_work.quality.latest_report(version_id)
                if quality_row is not None:
                    quality_report_id = str(quality_row["quality_run_id"])
                    files["Quality/model-quality-report.json"] = (
                        _canonical_json_bytes(json.loads(quality_row["report_json"])),
                        "application/json",
                    )

            readme_payload = (
                "ForgeCAD Weapon Concept Pack\n"
                "\n"
                "This package contains a future-weapon concept for game assets, film props, "
                "or non-functional display use. It is not a manufacturing package and does not "
                "contain functional engineering or fabrication instructions.\n"
            ).encode("utf-8")
            files["README.txt"] = (readme_payload, "text/plain; charset=utf-8")

            file_entries = [
                ExportFileEntry(
                    path=path,
                    sha256=hashlib.sha256(payload).hexdigest(),
                    byte_size=len(payload),
                    mime_type=mime_type,
                )
                for path, (payload, mime_type) in files.items()
            ]
            manifest = ConceptExportManifest(
                export_id=export_id,
                project_id=project_id,
                version_id=version_id,
                profile=request.profile,
                spec_sha256=str(version["spec_sha256"]),
                graph_sha256=str(graph_row["graph_sha256"]),
                modules=module_entries,
                quality_report_id=quality_report_id,
                files=file_entries,
                created_at=created_at,
            )
            manifest_json = _canonical_json(manifest.model_dump(mode="json"))
            files["Manifest/concept-export-manifest.json"] = (
                manifest_json.encode("utf-8"),
                "application/json",
            )
            package_payload = _build_zip(files)
            stored = self.object_store.put(package_payload, extension=".zip")

            unit_of_work.concept_assets.add(
                asset_id=package_asset_id,
                project_id=project_id,
                version_id=version_id,
                role="export_package",
                logical_path=f"Exports/{export_id}.zip",
                object_path=stored.relative_path,
                sha256=stored.sha256,
                byte_size=stored.byte_size,
                mime_type="application/zip",
                metadata_json=_canonical_json(
                    {
                        "export_id": export_id,
                        "profile": request.profile,
                        "non_functional_only": True,
                    }
                ),
                created_at=created_at,
            )
            unit_of_work.exports.add(
                export_id=export_id,
                project_id=project_id,
                version_id=version_id,
                profile=request.profile,
                package_asset_id=package_asset_id,
                manifest_json=manifest_json,
                status="validated",
                created_at=created_at,
            )
            unit_of_work.exports.add_artifact_link(
                project_id=project_id,
                version_id=version_id,
                asset_id=package_asset_id,
                relation="concept_export_package",
                created_at=created_at,
            )
            job_id = record_completed_job(
                unit_of_work,
                project_id=project_id,
                version_id=version_id,
                job_type="export_package",
                input_payload=request.model_dump(mode="json"),
                output_payload={
                    "export_id": export_id,
                    "package_asset_id": package_asset_id,
                    "package_sha256": stored.sha256,
                },
                steps=[
                    ("collect", "Collected immutable concept sources.", 0.35, {}),
                    ("manifest", "Validated ConceptExportManifest@1.", 0.7, {}),
                    ("package", "Stored concept export package.", 1.0, {"export_id": export_id}),
                ],
                artifact_asset_id=package_asset_id,
            )
            response = ConceptExportRecord(
                export_id=export_id,
                project_id=project_id,
                version_id=version_id,
                profile=request.profile,
                status="validated",
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

    def get_export(self, export_id: str) -> ConceptExportRecord:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            row = unit_of_work.exports.get(export_id)
            if row is None:
                raise ConceptExportError("EXPORT_NOT_FOUND", "Concept export not found.")
            return _record_from_row(row)

    def read_export(self, export_id: str) -> tuple[bytes, str, str]:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            row = unit_of_work.exports.get(export_id)
            if row is None:
                raise ConceptExportError("EXPORT_NOT_FOUND", "Concept export not found.")
            try:
                payload = self.object_store.read(
                    str(row["object_path"]),
                    expected_sha256=str(row["package_sha256"]),
                )
            except ObjectStoreError as exc:
                raise ConceptExportError(
                    "EXPORT_PACKAGE_UNAVAILABLE",
                    f"Concept export package is unavailable: {exc}",
                ) from exc
            return payload, f"{export_id}.zip", str(row["package_sha256"])


def _record_from_row(row: Any) -> ConceptExportRecord:
    return ConceptExportRecord(
        export_id=row["export_id"],
        project_id=row["project_id"],
        version_id=row["version_id"],
        profile=row["profile"],
        status=row["status"],
        job_id=row["job_id"],
        package_asset_id=row["package_asset_id"],
        package_sha256=row["package_sha256"],
        package_byte_size=row["package_byte_size"],
        manifest=ConceptExportManifest.model_validate_json(row["manifest_json"]),
        created_at=row["created_at"],
    )


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


def _canonical_json_bytes(value: Any) -> bytes:
    return _canonical_json(value).encode("utf-8")


def _hash_json(value: Any) -> str:
    return hashlib.sha256(_canonical_json(value).encode("utf-8")).hexdigest()


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex[:12]}"


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()
