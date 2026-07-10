#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import os
import sqlite3
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

from smoke_r2_concept_projects import (
    _assert,
    _create_body,
    _free_port,
    _json_request,
    _start_agent,
    _stop_agent,
    _wait_for_health,
)
from smoke_r2_module_registry import _graph
from smoke_r3_change_set_audit_export import _download, _register_modules

ROOT = Path(__file__).resolve().parents[1]
BACKUP_CLI = ROOT / "scripts" / "library_backup.py"


def main() -> int:
    with tempfile.TemporaryDirectory(
        prefix="forgecad_library_backup_"
    ) as temporary_directory:
        temporary_root = Path(temporary_directory)
        source_library = temporary_root / "SourceLibrary"
        backup_root = temporary_root / "Backup" / "snapshot-001"
        restored_library = temporary_root / "RestoredLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(source_library, port)
        try:
            _wait_for_health(base_url, process)
            seeded = _seed_audit_archive(base_url)
        finally:
            _stop_agent(process)

        legacy_payload = b"legacy-backup-asset"
        legacy_sha256 = hashlib.sha256(legacy_payload).hexdigest()
        legacy_relative_path = (
            Path("objects")
            / "sha256"
            / legacy_sha256[:2]
            / legacy_sha256[2:4]
            / f"{legacy_sha256}.bin"
        )
        legacy_object = source_library / legacy_relative_path
        legacy_object.parent.mkdir(parents=True, exist_ok=True)
        legacy_object.write_bytes(legacy_payload)
        with sqlite3.connect(source_library / "library.db") as connection:
            source_asset = connection.execute(
                """
                SELECT object_path, sha256, byte_size, mime_type
                FROM concept_assets
                WHERE role = 'module_glb'
                ORDER BY asset_id
                LIMIT 1
                """
            ).fetchone()
            connection.execute(
                """
                INSERT INTO concept_assets (
                  asset_id, project_id, version_id, role, logical_path,
                  object_path, sha256, byte_size, mime_type, metadata_json,
                  created_at, soft_deleted_at
                )
                VALUES (?, ?, NULL, 'other', ?, ?, ?, ?, ?, '{}', ?, NULL)
                """,
                (
                    "asset_backup_duplicate_reference",
                    seeded["project_id"],
                    "backup/duplicate-reference.glb",
                    source_asset[0],
                    source_asset[1],
                    source_asset[2],
                    source_asset[3],
                    "2026-07-10T15:00:00+00:00",
                ),
            )
            connection.execute(
                """
                INSERT INTO asset_files (
                  file_id, weapon_id, version_id, job_id, role, logical_path,
                  object_path, sha256, byte_size, mime_type, ext, metadata_json,
                  created_at, soft_deleted_at
                )
                VALUES (?, NULL, NULL, NULL, 'other', ?, ?, ?, ?, ?, '.bin', '{}', ?, NULL)
                """,
                (
                    "file_backup_legacy_reference",
                    "backup/legacy-reference.bin",
                    legacy_relative_path.as_posix(),
                    legacy_sha256,
                    len(legacy_payload),
                    "application/octet-stream",
                    "2026-07-10T15:00:00+00:00",
                ),
            )
            connection.commit()
            duplicate_reference_bytes = int(source_asset[2])

        secret_file = source_library / "config" / "provider-secret.txt"
        secret_file.parent.mkdir(parents=True, exist_ok=True)
        secret_like_value = "".join(("s", "k", "-", "backup-smoke-must-not-copy\n"))
        secret_file.write_text(secret_like_value, encoding="utf-8")
        orphan_payload = b"unreferenced-backup-candidate"
        orphan_sha256 = hashlib.sha256(orphan_payload).hexdigest()
        orphan_path = (
            source_library
            / "objects"
            / "sha256"
            / orphan_sha256[:2]
            / orphan_sha256[2:4]
            / f"{orphan_sha256}.bin"
        )
        orphan_path.parent.mkdir(parents=True, exist_ok=True)
        orphan_path.write_bytes(orphan_payload)

        nested_backup = _run_cli(
            "backup",
            "--library-root",
            str(source_library),
            "--output",
            str(source_library / "backups" / "unsafe"),
            expected_exit=2,
        )
        _assert(
            nested_backup["error"]["code"] == "BACKUP_DESTINATION_INSIDE_LIBRARY",
            "nested backup destination guard mismatch",
        )

        backup = _run_cli(
            "backup",
            "--library-root",
            str(source_library),
            "--output",
            str(backup_root),
        )
        capacity = backup["capacity"]
        _assert(capacity["unique_object_count"] == 4, "backup object count mismatch")
        _assert(capacity["reference_rows"] == 5, "backup reference count mismatch")
        _assert(
            capacity["deduplicated_bytes"] == duplicate_reference_bytes,
            "backup content-addressed deduplication baseline mismatch",
        )
        _assert(
            capacity["unreferenced_candidate_count"] == 1
            and capacity["unreferenced_candidate_bytes"] == len(orphan_payload),
            "backup orphan capacity baseline mismatch",
        )
        _assert(
            capacity["source_object_store_file_count"] == 5,
            "backup source object store baseline mismatch",
        )
        _assert(
            not (backup_root / orphan_path.relative_to(source_library)).exists(),
            "backup copied an unreferenced object candidate",
        )
        _assert(
            not (backup_root / "config").exists(),
            "backup copied provider config or secret files",
        )
        _assert(
            not (backup_root / "library.db-wal").exists()
            and not (backup_root / "library.db-shm").exists(),
            "backup copied SQLite transient files",
        )

        verified = _run_cli("verify", "--backup", str(backup_root))
        _assert(verified["integrity_check"] == "ok", "backup integrity mismatch")
        _assert(
            verified["journal_mode"] == "delete",
            "backup is not a standalone SQLite snapshot",
        )
        _assert(
            verified["capacity"] == capacity,
            "backup capacity baseline did not round-trip",
        )
        manifest = json.loads(
            (backup_root / "backup-manifest.json").read_text(encoding="utf-8")
        )
        _assert(
            {table for item in manifest["objects"] for table in item["source_tables"]}
            == {"asset_files", "concept_assets"},
            "backup did not cover both legacy and Concept object tables",
        )
        audit_entry = next(
            item
            for item in manifest["objects"]
            if item["sha256"] == seeded["audit_package_sha256"]
        )
        audit_object = backup_root / audit_entry["path"]
        original_audit_payload = audit_object.read_bytes()
        audit_object.write_bytes(original_audit_payload + b"tamper")
        tamper = _run_cli("verify", "--backup", str(backup_root), expected_exit=2)
        _assert(
            tamper["error"]["code"] == "BACKUP_SIZE_MISMATCH",
            "backup tamper detection mismatch",
        )
        audit_object.write_bytes(original_audit_payload)
        _run_cli("verify", "--backup", str(backup_root))
        extra_payload = b"unexpected-backup-object"
        extra_sha256 = hashlib.sha256(extra_payload).hexdigest()
        extra_object = (
            backup_root
            / "objects"
            / "sha256"
            / extra_sha256[:2]
            / extra_sha256[2:4]
            / f"{extra_sha256}.bin"
        )
        extra_object.parent.mkdir(parents=True, exist_ok=True)
        extra_object.write_bytes(extra_payload)
        extra_result = _run_cli("verify", "--backup", str(backup_root), expected_exit=2)
        _assert(
            extra_result["error"]["code"] == "BACKUP_OBJECT_FILE_SET_MISMATCH",
            "backup extra-object detection mismatch",
        )
        extra_object.unlink()
        _run_cli("verify", "--backup", str(backup_root))

        with sqlite3.connect(source_library / "library.db") as connection:
            connection.execute(
                "CREATE TABLE future_object_refs (reference_id TEXT PRIMARY KEY, object_path TEXT)"
            )
        unsupported_table = _run_cli(
            "backup",
            "--library-root",
            str(source_library),
            "--output",
            str(temporary_root / "Backup" / "unsupported-object-table"),
            expected_exit=2,
        )
        _assert(
            unsupported_table["error"]["code"] == "UNSUPPORTED_OBJECT_REFERENCE_TABLE",
            "future object table guard mismatch",
        )
        with sqlite3.connect(source_library / "library.db") as connection:
            connection.execute("DROP TABLE future_object_refs")

        nested_restore = _run_cli(
            "restore",
            "--backup",
            str(backup_root),
            "--destination",
            str(backup_root / "unsafe-restored-library"),
            expected_exit=2,
        )
        _assert(
            nested_restore["error"]["code"] == "RESTORE_DESTINATION_INSIDE_BACKUP",
            "nested restore destination guard mismatch",
        )

        restored_library.mkdir()
        existing_destination = _run_cli(
            "restore",
            "--backup",
            str(backup_root),
            "--destination",
            str(restored_library),
            expected_exit=2,
        )
        _assert(
            existing_destination["error"]["code"] == "RESTORE_DESTINATION_EXISTS",
            "restore overwrite guard mismatch",
        )
        restored_library.rmdir()
        restored = _run_cli(
            "restore",
            "--backup",
            str(backup_root),
            "--destination",
            str(restored_library),
        )
        _assert(
            restored["restored_verification"]["integrity_check"] == "ok"
            and restored["restored_verification"]["journal_mode"] == "delete"
            and restored["restored_verification"]["object_count"] == 4,
            "restored library verification mismatch",
        )
        provenance = (
            restored_library / "backups" / "manifests" / f"{manifest['backup_id']}.json"
        )
        _assert(provenance.is_file(), "restore provenance manifest missing")
        _assert(
            not (restored_library / "config").exists()
            and not (restored_library / "library.db-wal").exists()
            and not (restored_library / "library.db-shm").exists(),
            "restore recreated excluded transient or secret files",
        )
        _assert(
            (restored_library / legacy_relative_path).read_bytes() == legacy_payload,
            "restored legacy asset payload mismatch",
        )

        restored_port = _free_port()
        restored_url = f"http://127.0.0.1:{restored_port}"
        restored_process = _start_agent(restored_library, restored_port)
        try:
            _wait_for_health(restored_url, restored_process)
            project = _json_request(
                restored_url,
                f"/api/v1/projects/{seeded['project_id']}",
                method="GET",
            )
            audits = _json_request(
                restored_url,
                f"/api/v1/projects/{seeded['project_id']}/change-set-audit-exports",
                method="GET",
            )
            modules = _json_request(
                restored_url,
                "/api/v1/module-assets?pack_id=pack_weapon_concept_v1",
                method="GET",
            )
            restored_package, restored_header_hash = _download(
                restored_url, seeded["audit_export_id"]
            )
            _assert(
                project["current_version_id"] == seeded["version_id"],
                "restored Project current version mismatch",
            )
            _assert(
                len(audits["items"]) == 1 and audits["items"][0]["record_count"] == 1,
                "restored ChangeSet audit metadata mismatch",
            )
            _assert(len(modules["items"]) == 2, "restored module registry mismatch")
            _assert(
                hashlib.sha256(restored_package).hexdigest()
                == restored_header_hash
                == seeded["audit_package_sha256"],
                "restored audit package hash mismatch",
            )
            planner_job = _json_request(
                restored_url,
                f"/api/v1/jobs/{seeded['planner_job_id']}",
                method="GET",
            )
            _assert(
                planner_job["type"] == "concept_change_plan"
                and planner_job["status"] == "succeeded",
                "restored planner JobEvent trace mismatch",
            )
        finally:
            _stop_agent(restored_process)

        with sqlite3.connect(restored_library / "library.db") as connection:
            integrity = connection.execute("PRAGMA integrity_check").fetchone()[0]
            foreign_keys = connection.execute("PRAGMA foreign_key_check").fetchall()
            audit_count = connection.execute(
                "SELECT COUNT(*) FROM change_set_audit_exports"
            ).fetchone()[0]
        _assert(integrity == "ok" and not foreign_keys, "restored SQLite check failed")
        _assert(audit_count == 1, "restored audit table count mismatch")
        print(
            json.dumps(
                {
                    "ok": True,
                    "backup_id": manifest["backup_id"],
                    "project_id": seeded["project_id"],
                    "audit_export_id": seeded["audit_export_id"],
                    "audit_package_sha256": seeded["audit_package_sha256"],
                    "schema_versions": verified["schema_versions"],
                    "capacity": capacity,
                    "tamper_detected": True,
                    "extra_object_detected": True,
                    "future_object_table_guard_verified": True,
                    "nested_destination_guard_verified": True,
                    "overwrite_guard_verified": True,
                    "secret_and_transient_exclusions_verified": True,
                    "restored_agent_verified": True,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    return 0


def _seed_audit_archive(base_url: str) -> dict[str, Any]:
    project = _json_request(
        base_url,
        "/api/v1/projects",
        method="POST",
        body=_create_body(),
        idempotency_key="backup-project",
    )
    project_id = project["project_id"]
    _register_modules(base_url)
    graph = _graph(project_id)
    _json_request(
        base_url,
        f"/api/v1/module-graphs/{graph['graph_id']}/validate",
        method="POST",
        body={
            "client_request_id": "backup-graph",
            "graph": graph,
            "persist": True,
        },
        idempotency_key="backup-graph",
    )
    bound = _json_request(
        base_url,
        f"/api/v1/projects/{project_id}/versions",
        method="POST",
        body={
            "client_request_id": "backup-bind",
            "parent_version_id": project["current_version_id"],
            "summary": "绑定备份恢复演练 Graph。",
            "spec": project["current_spec"],
            "module_graph_id": graph["graph_id"],
        },
        idempotency_key="backup-bind",
    )
    version_id = bound["current_version_id"]
    planned = _json_request(
        base_url,
        f"/api/v1/versions/{version_id}/change-sets:plan",
        method="POST",
        body={
            "client_request_id": "backup-change-plan",
            "instruction": "整体长度调整为 226 mm。",
            "generator": "deterministic_rules",
        },
        idempotency_key="backup-change-plan",
    )
    audit = _json_request(
        base_url,
        f"/api/v1/projects/{project_id}/change-set-audit-exports",
        method="POST",
        body={
            "client_request_id": "backup-audit-export",
            "include_jsonl": True,
            "include_csv": True,
            "retention_class": "project_lifetime",
            "max_records": 5000,
        },
        idempotency_key="backup-audit-export",
    )
    package, header_hash = _download(base_url, audit["audit_export_id"])
    package_sha256 = hashlib.sha256(package).hexdigest()
    _assert(package_sha256 == header_hash, "seed audit package hash mismatch")
    return {
        "project_id": project_id,
        "version_id": version_id,
        "planner_job_id": planned["job_id"],
        "audit_export_id": audit["audit_export_id"],
        "audit_package_sha256": package_sha256,
    }


def _run_cli(*arguments: str, expected_exit: int = 0) -> dict[str, Any]:
    result = subprocess.run(
        [sys.executable, str(BACKUP_CLI), *arguments],
        cwd=ROOT,
        env={**os.environ, "PYTHONPATH": str(ROOT / "apps" / "agent")},
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != expected_exit:
        raise AssertionError(
            f"backup CLI exit mismatch for {arguments}: expected {expected_exit}, "
            f"got {result.returncode}\nstdout={result.stdout}\nstderr={result.stderr}"
        )
    payload = result.stdout if result.returncode == 0 else result.stderr
    try:
        return json.loads(payload)
    except json.JSONDecodeError as exc:
        raise AssertionError(
            f"backup CLI returned invalid JSON for {arguments}: {payload}"
        ) from exc


if __name__ == "__main__":
    raise SystemExit(main())
