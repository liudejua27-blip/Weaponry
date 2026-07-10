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

from smoke_r2_change_sets import _change_set
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
    with tempfile.TemporaryDirectory(
        prefix="forgecad_r3_change_audit_"
    ) as temporary_directory:
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
                idempotency_key="r3-audit-project",
            )
            project_id = project["project_id"]
            _register_modules(base_url)
            graph = _graph(project_id)
            _json_request(
                base_url,
                f"/api/v1/module-graphs/{graph['graph_id']}/validate",
                method="POST",
                body={
                    "client_request_id": "r3-audit-graph",
                    "graph": graph,
                    "persist": True,
                },
                idempotency_key="r3-audit-graph",
            )
            bound = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/versions",
                method="POST",
                body={
                    "client_request_id": "r3-audit-bind",
                    "parent_version_id": project["current_version_id"],
                    "summary": "绑定审计导出测试 Graph。",
                    "spec": project["current_spec"],
                    "module_graph_id": graph["graph_id"],
                },
                idempotency_key="r3-audit-bind",
            )
            version_id = bound["current_version_id"]

            user_change = _change_set(
                project_id, version_id, "change_audit_user_detail"
            )
            user_change["summary"] = "=1+1 audit formula probe"
            _json_request(
                base_url,
                f"/api/v1/versions/{version_id}/change-sets",
                method="POST",
                body={
                    "client_request_id": "r3-audit-user-change",
                    "change_set": user_change,
                },
                idempotency_key="r3-audit-user-change",
            )
            planned = _json_request(
                base_url,
                f"/api/v1/versions/{version_id}/change-sets:plan",
                method="POST",
                body={
                    "client_request_id": "r3-audit-planner-change",
                    "instruction": "整体长度调整为 226 mm。",
                    "generator": "deterministic_rules",
                },
                idempotency_key="r3-audit-planner-change",
            )

            request_body = {
                "client_request_id": "r3-audit-export",
                "include_jsonl": True,
                "include_csv": True,
                "retention_class": "project_lifetime",
                "max_records": 5000,
            }
            created = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/change-set-audit-exports",
                method="POST",
                body=request_body,
                idempotency_key="r3-audit-export",
            )
            _assert(created["status"] == "validated", "audit status mismatch")
            _assert(created["record_count"] == 2, "audit record count mismatch")
            _assert(
                created["retention_class"] == "project_lifetime",
                "audit retention class mismatch",
            )
            replay = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/change-set-audit-exports",
                method="POST",
                body=request_body,
                idempotency_key="r3-audit-export",
            )
            _assert(
                replay["audit_export_id"] == created["audit_export_id"],
                "audit idempotency replay mismatch",
            )
            limit_status, limit_body = _json_request_allow_error(
                base_url,
                f"/api/v1/projects/{project_id}/change-set-audit-exports",
                method="POST",
                body={
                    **request_body,
                    "client_request_id": "r3-audit-limit",
                    "max_records": 1,
                },
                idempotency_key="r3-audit-limit",
            )
            _assert(
                limit_status == 409
                and limit_body["error"]["code"] == "AUDIT_EXPORT_LIMIT_EXCEEDED",
                "audit max-record guard mismatch",
            )

            package, header_hash = _download(base_url, created["audit_export_id"])
            package_hash = hashlib.sha256(package).hexdigest()
            _assert(package[:2] == b"PK", "audit download is not a ZIP")
            _assert(
                package_hash == created["package_sha256"] == header_hash,
                "audit package hash mismatch",
            )
            with zipfile.ZipFile(io.BytesIO(package)) as archive:
                names = set(archive.namelist())
                _assert(
                    names
                    == {
                        "Manifest/change-set-audit-export.json",
                        "README.txt",
                        "Records/change-sets.csv",
                        "Records/change-sets.jsonl",
                    },
                    "audit archive entries mismatch",
                )
                manifest = json.loads(
                    archive.read("Manifest/change-set-audit-export.json")
                )
                records = [
                    json.loads(line)
                    for line in archive.read("Records/change-sets.jsonl")
                    .decode("utf-8")
                    .splitlines()
                    if line
                ]
                _assert(
                    manifest["schema_version"] == "ChangeSetAuditExportManifest@1"
                    and manifest["record_count"] == 2
                    and manifest["ordering"] == "updated_at_desc_change_set_id_desc",
                    "audit manifest contract mismatch",
                )
                for entry in manifest["files"]:
                    payload = archive.read(entry["path"])
                    _assert(
                        hashlib.sha256(payload).hexdigest() == entry["sha256"]
                        and len(payload) == entry["byte_size"],
                        f"audit entry hash mismatch: {entry['path']}",
                    )
                actors = {record["actor_type"] for record in records}
                _assert(actors == {"user", "planner"}, "audit actors mismatch")
                planner_record = next(
                    record for record in records if record["actor_type"] == "planner"
                )
                _assert(
                    planner_record["planner_provenance"]["provider_id"]
                    == "deterministic_concept_rules"
                    and planner_record["planner_job_id"] == planned["job_id"],
                    "audit planner provenance mismatch",
                )
                csv_text = archive.read("Records/change-sets.csv").decode("utf-8-sig")
                _assert(
                    "'=1+1 audit formula probe" in csv_text,
                    "audit CSV formula neutralization mismatch",
                )
            filtered = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/change-set-audit-exports",
                method="POST",
                body={
                    **request_body,
                    "client_request_id": "r3-audit-filtered",
                    "operation": "set_parameter",
                    "include_csv": False,
                },
                idempotency_key="r3-audit-filtered",
            )
            _assert(
                filtered["record_count"] == 1
                and filtered["filters"]["operation"] == "set_parameter"
                and {entry["path"] for entry in filtered["manifest"]["files"]}
                == {"README.txt", "Records/change-sets.jsonl"},
                "filtered JSONL-only audit mismatch",
            )
        except Exception as exc:
            _stop_agent(process)
            agent_output = process.stdout.read() if process.stdout else ""
            raise RuntimeError(
                f"ChangeSet audit export smoke failed: {exc}\nAgent output:\n{agent_output}"
            ) from exc
        finally:
            if process.poll() is None:
                _stop_agent(process)

        restart_port = _free_port()
        restart_url = f"http://127.0.0.1:{restart_port}"
        restart_process = _start_agent(library_root, restart_port)
        try:
            _wait_for_health(restart_url, restart_process)
            restored = _json_request(
                restart_url,
                f"/api/v1/projects/{project_id}/change-set-audit-exports",
                method="GET",
            )
            restored_main = next(
                (
                    item
                    for item in restored["items"]
                    if item["audit_export_id"] == created["audit_export_id"]
                ),
                None,
            )
            _assert(
                len(restored["items"]) == 2
                and restored_main is not None
                and restored_main["package_sha256"] == package_hash,
                "audit archive did not survive restart",
            )
            restored_package, restored_header_hash = _download(
                restart_url, created["audit_export_id"]
            )
            _assert(
                restored_package == package and restored_header_hash == package_hash,
                "restart audit download mismatch",
            )
        finally:
            _stop_agent(restart_process)

        with sqlite3.connect(library_root / "library.db") as connection:
            audit_row = connection.execute(
                """
                SELECT record_count, retention_class, status
                FROM change_set_audit_exports
                WHERE audit_export_id = ?
                """,
                (created["audit_export_id"],),
            ).fetchone()
            asset_row = connection.execute(
                """
                SELECT role, sha256 FROM concept_assets WHERE asset_id = ?
                """,
                (created["package_asset_id"],),
            ).fetchone()
            link_row = connection.execute(
                """
                SELECT version_id, relation FROM artifact_links WHERE asset_id = ?
                """,
                (created["package_asset_id"],),
            ).fetchone()
        _assert(
            audit_row == (2, "project_lifetime", "validated"),
            "audit persistence mismatch",
        )
        _assert(asset_row == ("project_report", package_hash), "audit asset mismatch")
        _assert(
            link_row == (None, "change_set_audit_package"),
            "audit artifact link mismatch",
        )
        print(
            json.dumps(
                {
                    "ok": True,
                    "project_id": project_id,
                    "audit_export_id": created["audit_export_id"],
                    "record_count": created["record_count"],
                    "package_sha256": package_hash,
                    "restart_restored": True,
                    "retention_class": created["retention_class"],
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    return 0


def _register_modules(base_url: str) -> None:
    modules = (
        (
            "module_core_shell_01",
            "asset_core_shell_01",
            "core_shell",
            "connector_core_front",
            "core.front",
            "core-shell-01.glb",
        ),
        (
            "module_front_shell_01",
            "asset_front_shell_01",
            "front_shell",
            "connector_front_core",
            "front.core",
            "front-shell-01.glb",
        ),
    )
    for index, (
        module_id,
        asset_id,
        category,
        connector_id,
        slot,
        filename,
    ) in enumerate(modules, start=1):
        payload = _minimal_glb(module_id)
        _register(
            base_url,
            _manifest(
                module_id=module_id,
                asset_id=asset_id,
                category=category,
                payload=payload,
                connectors=[_connector(connector_id, slot, "shell_mount")],
            ),
            payload,
            f"packs/weapon-concept/{filename}",
            f"r3-audit-register-{index}",
        )


def _download(base_url: str, audit_export_id: str) -> tuple[bytes, str]:
    request = urllib.request.Request(
        f"{base_url}/api/v1/change-set-audit-exports/{audit_export_id}/file"
    )
    with urllib.request.urlopen(request, timeout=20) as response:
        return response.read(), response.headers["X-Content-SHA256"]


if __name__ == "__main__":
    raise SystemExit(main())
