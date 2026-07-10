#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import io
import json
import sqlite3
import tempfile
import urllib.request
import zipfile
from pathlib import Path

from smoke_r2_concept_projects import (
    _assert,
    _create_body,
    _free_port,
    _json_request,
    _json_request_allow_error,
    _start_agent,
    _stop_agent,
    _wait_for_health,
)
from smoke_r2_module_registry import (
    _connector,
    _graph,
    _manifest,
    _minimal_glb,
    _register,
)


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad_r2_exports_") as temporary_directory:
        library_root = Path(temporary_directory) / "ForgeCADLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(library_root, port)
        try:
            _wait_for_health(base_url, process)
            project = _json_request(
                base_url,
                "/api/v1/projects",
                method="POST",
                body=_create_body(),
                idempotency_key="r2-export-project",
            )
            project_id = project["project_id"]
            core_payload = _minimal_glb("export_core")
            front_payload = _minimal_glb("export_front")
            _register(
                base_url,
                _manifest(
                    module_id="module_core_shell_01",
                    asset_id="asset_core_shell_01",
                    category="core_shell",
                    payload=core_payload,
                    connectors=[
                        _connector("connector_core_front", "core.front", "shell_mount")
                    ],
                ),
                core_payload,
                "packs/weapon-concept/core-shell-01.glb",
                "r2-export-core",
            )
            _register(
                base_url,
                _manifest(
                    module_id="module_front_shell_01",
                    asset_id="asset_front_shell_01",
                    category="front_shell",
                    payload=front_payload,
                    connectors=[
                        _connector("connector_front_core", "front.core", "shell_mount")
                    ],
                ),
                front_payload,
                "packs/weapon-concept/front-shell-01.glb",
                "r2-export-front",
            )
            graph = _graph(project_id)
            _json_request(
                base_url,
                f"/api/v1/module-graphs/{graph['graph_id']}/validate",
                method="POST",
                body={"client_request_id": "r2-export-graph", "graph": graph, "persist": True},
                idempotency_key="r2-export-graph",
            )
            bound = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/versions",
                method="POST",
                body={
                    "client_request_id": "r2-export-bind",
                    "parent_version_id": project["current_version_id"],
                    "summary": "绑定用于概念导出的模块图。",
                    "spec": project["current_spec"],
                    "module_graph_id": graph["graph_id"],
                },
                idempotency_key="r2-export-bind",
            )
            version_id = bound["current_version_id"]
            quality_report = _quality_report(project_id, version_id)
            _json_request(
                base_url,
                f"/api/v1/versions/{version_id}/quality-runs",
                method="POST",
                body={"client_request_id": "r2-export-quality", "report": quality_report},
                idempotency_key="r2-export-quality",
            )

            export_body = {
                "client_request_id": "r2-export-create",
                "profile": "game_asset",
                "include_modules": True,
                "include_quality_report": True,
            }
            created = _json_request(
                base_url,
                f"/api/v1/versions/{version_id}/exports",
                method="POST",
                body=export_body,
                idempotency_key="r2-export-create",
            )
            _assert(created["status"] == "validated", "export status mismatch")
            _assert(created["manifest"]["non_functional_only"] is True, "export boundary missing")
            _assert(len(created["manifest"]["modules"]) == 2, "module manifest count mismatch")
            _assert(
                created["manifest"]["quality_report_id"] == quality_report["report_id"],
                "quality report was not linked",
            )
            job = _json_request(base_url, f"/api/v1/jobs/{created['job_id']}", method="GET")
            _assert(job["status"] == "succeeded", "export job status mismatch")
            _assert(
                job["events"][-1]["artifact_asset_id"] == created["package_asset_id"],
                "export artifact was not linked from JobEvent@2",
            )

            package_payload = _download(base_url, created["export_id"])
            _assert(
                hashlib.sha256(package_payload).hexdigest() == created["package_sha256"],
                "downloaded export hash mismatch",
            )
            _assert(len(package_payload) == created["package_byte_size"], "export size mismatch")
            with zipfile.ZipFile(io.BytesIO(package_payload)) as archive:
                names = set(archive.namelist())
                required = {
                    "Manifest/concept-export-manifest.json",
                    "Specs/weapon-concept-spec.json",
                    "Graphs/module-graph.json",
                    "Modules/node_core.glb",
                    "Modules/node_front.glb",
                    "Quality/model-quality-report.json",
                    "README.txt",
                }
                _assert(required <= names, f"export package files missing: {sorted(required - names)}")
                manifest = json.loads(archive.read("Manifest/concept-export-manifest.json"))
                _assert(manifest == created["manifest"], "ZIP manifest mismatch")
                _assert(archive.read("Modules/node_core.glb") == core_payload, "core GLB changed")
                _assert(archive.read("Modules/node_front.glb") == front_payload, "front GLB changed")
                for entry in manifest["files"]:
                    payload = archive.read(entry["path"])
                    _assert(hashlib.sha256(payload).hexdigest() == entry["sha256"], "file hash mismatch")

            replay = _json_request(
                base_url,
                f"/api/v1/versions/{version_id}/exports",
                method="POST",
                body=export_body,
                idempotency_key="r2-export-create",
            )
            _assert(replay["export_id"] == created["export_id"], "export replay mismatch")
            conflict_status, conflict = _json_request_allow_error(
                base_url,
                f"/api/v1/versions/{version_id}/exports",
                method="POST",
                body={**export_body, "profile": "film_prop"},
                idempotency_key="r2-export-create",
            )
            _assert(
                conflict_status == 409 and conflict["error"]["code"] == "IDEMPOTENCY_CONFLICT",
                "export idempotency conflict was not rejected",
            )
        finally:
            _stop_agent(process)

        database_path = library_root / "library.db"
        with sqlite3.connect(database_path) as connection:
            export_count = connection.execute("SELECT COUNT(*) FROM export_packages_v2").fetchone()[0]
            asset_count = connection.execute(
                "SELECT COUNT(*) FROM concept_assets WHERE role = 'export_package'"
            ).fetchone()[0]
            link_count = connection.execute(
                "SELECT COUNT(*) FROM artifact_links WHERE relation = 'concept_export_package'"
            ).fetchone()[0]
        _assert((export_count, asset_count, link_count) == (1, 1, 1), "export persistence mismatch")

        restart_port = _free_port()
        restart_url = f"http://127.0.0.1:{restart_port}"
        restarted_process = _start_agent(library_root, restart_port)
        try:
            _wait_for_health(restart_url, restarted_process)
            restored = _json_request(
                restart_url,
                f"/api/v1/exports/{created['export_id']}",
                method="GET",
            )
            _assert(restored["package_sha256"] == created["package_sha256"], "restart export mismatch")
            _assert(_download(restart_url, created["export_id"]) == package_payload, "restart download mismatch")
        finally:
            _stop_agent(restarted_process)

        print(
            json.dumps(
                {
                    "ok": True,
                    "export_id": created["export_id"],
                    "job_id": created["job_id"],
                    "package_sha256": created["package_sha256"],
                    "module_count": len(created["manifest"]["modules"]),
                    "quality_report_included": True,
                    "restart_restored": True,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    return 0


def _quality_report(project_id: str, version_id: str) -> dict:
    return {
        "schema_version": "ModelQualityReport@1",
        "report_id": "quality_export_v2",
        "project_id": project_id,
        "version_id": version_id,
        "ruleset_version": "weapon-concept-quality/1.0",
        "status": "passed",
        "findings": [],
        "created_at": "2026-07-10T12:00:00+00:00",
    }


def _download(base_url: str, export_id: str) -> bytes:
    with urllib.request.urlopen(f"{base_url}/api/v1/exports/{export_id}/file", timeout=10) as response:
        _assert(response.status == 200, "export download failed")
        _assert(response.headers.get_content_type() == "application/zip", "export MIME mismatch")
        return response.read()


if __name__ == "__main__":
    raise SystemExit(main())
