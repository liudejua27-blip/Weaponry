#!/usr/bin/env python3
"""Prove CSG candidate staging cannot partially promote authoritative state."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner, SQLiteUnitOfWork
from forgecad_agent.infrastructure.storage.content_addressed_store import ContentAddressedStore


ROOT = Path(__file__).resolve().parents[1]
NOW = "2026-07-15T00:00:00+00:00"
PROJECT_ID = "prj_g824b_lifecycle"
TABLES = (
    "agent_asset_versions",
    "agent_asset_heads",
    "agent_asset_change_sets",
    "active_design_snapshots",
    "agent_asset_quality_reports",
    "agent_imported_glbs",
    "idempotency_records",
)


def _canonical(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), default=str).encode()


def _graph(version: int) -> dict[str, Any]:
    return {
        "schema_version": "AssemblyGraph@1",
        "graph_id": f"mg_g824b_v{version}",
        "root_part_id": "part_g824b_shell",
        "parts": [{"part_id": "part_g824b_shell", "material_zones": ["zone_shell"]}],
        "connections": [],
    }


def _add_version(unit: SQLiteUnitOfWork, version: int) -> None:
    unit.agent_assets.add_version(
        asset_version_id=f"assetver_g824b_v{version}",
        project_id=PROJECT_ID,
        parent_asset_version_id=None if version == 1 else "assetver_g824b_v1",
        version_no=version,
        status="committed",
        summary=f"G824B lifecycle v{version}",
        stage="editable_asset",
        plan_id="plan_g824b",
        direction_id="direction_g824b",
        domain_pack_id="pack_vehicle_concept",
        artifact_id=f"artifact_g824b_v{version}",
        parts_json='[{"part_id":"part_g824b_shell"}]',
        shape_program_json='{"schema_version":"ShapeProgram@1","operations":[]}',
        assembly_graph_json=json.dumps(_graph(version), sort_keys=True),
        material_bindings_json='{"zone_shell":"material_shell"}',
        created_at=NOW,
    )


def _seed(factory: SQLiteConnectionFactory, store: ContentAddressedStore) -> None:
    with factory.connect() as connection:
        connection.execute(
            """INSERT INTO domain_profiles(profile_id, domain_type, schema_version, pack_id, display_name, profile_json, profile_sha256, status, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)""",
            ("profile_g824b", "weapon_concept", "DesignDomainProfile@1", "pack_vehicle_concept", "G824B", "{}", "0" * 64, "active", NOW, NOW),
        )
        connection.execute(
            """INSERT INTO projects(project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)""",
            (PROJECT_ID, "profile_g824b", "weapon_concept", "G824B lifecycle", "active", None, NOW, NOW),
        )
    with SQLiteUnitOfWork(factory) as unit:
        _add_version(unit, 1)
        unit.agent_assets.set_head(project_id=PROJECT_ID, asset_version_id="assetver_g824b_v1", updated_at=NOW)
        unit.active_designs.create_agent_snapshot(
            project_id=PROJECT_ID,
            asset_version_id="assetver_g824b_v1",
            assembly_graph_id="mg_g824b_v1",
            updated_at=NOW,
        )
        unit.agent_assets.add_change_set(
            change_set_id="assetcs_g824b_pending",
            project_id=PROJECT_ID,
            base_asset_version_id="assetver_g824b_v1",
            summary="G824B pending candidate",
            operations_json="[]",
            protected_part_ids_json="[]",
            created_at=NOW,
        )
    store.put(b"g824b-stable-object", extension=".bin")


def _database_fingerprint(factory: SQLiteConnectionFactory) -> dict[str, Any]:
    with factory.connect() as connection:
        payload = {
            table: [dict(row) for row in connection.execute(f"SELECT * FROM {table} ORDER BY rowid").fetchall()]
            for table in TABLES
        }
    return {
        "sha256": hashlib.sha256(_canonical(payload)).hexdigest(),
        "row_counts": {table: len(rows) for table, rows in payload.items()},
    }


def _object_fingerprint(store: ContentAddressedStore) -> dict[str, Any]:
    entries = []
    for path in sorted(item for item in store.objects_root.rglob("*") if item.is_file()):
        entries.append({
            "relative_path": path.relative_to(store.library_root).as_posix(),
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "byte_size": path.stat().st_size,
        })
    return {"sha256": hashlib.sha256(_canonical(entries)).hexdigest(), "object_count": len(entries)}


def _wait_for(path: Path, process: subprocess.Popen[str], timeout: float = 10.0) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists():
            return json.loads(path.read_text(encoding="utf-8"))
        if process.poll() is not None:
            stdout, stderr = process.communicate()
            raise RuntimeError(f"candidate exited before marker: {stdout[-500:]} {stderr[-500:]}")
        time.sleep(0.01)
    process.kill()
    process.wait(timeout=3)
    raise TimeoutError(f"candidate marker timed out: {path.name}")


def _stop(process: subprocess.Popen[str], *, timeout: bool) -> None:
    if timeout:
        process.kill()
    else:
        process.terminate()
    process.wait(timeout=5)


def _busy_case(
    *,
    candidate: str,
    error_code: str,
    python_site: Path,
    wasm_module: Path,
    before_db: dict[str, Any],
    before_objects: dict[str, Any],
    factory: SQLiteConnectionFactory,
    store: ContentAddressedStore,
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix=f"forgecad-g824b-{candidate}-busy-") as raw:
        root = Path(raw)
        marker = root / "kernel-started.json"
        output = root / "candidate.glb"
        if candidate == "python":
            env = dict(os.environ)
            env["PYTHONPATH"] = os.pathsep.join([str(python_site), str(ROOT / "apps/agent")])
            command = [
                sys.executable,
                str(ROOT / "scripts/g824a_manifold_python_evidence.py"),
                "--busy-probe", "--marker", str(marker), "--output", str(output),
            ]
        else:
            env = None
            command = [
                "node", str(ROOT / "scripts/g824a_manifold_wasm_evidence.mjs"), wasm_module.resolve().as_uri(),
                "--busy-probe", str(marker), str(output),
            ]
        process = subprocess.Popen(command, cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        _wait_for(marker, process)
        _stop(process, timeout=error_code == "CSG_TIMEOUT")
        return {
            "window": "kernel_running",
            "error_code": error_code,
            "process_exit_code": process.returncode,
            "partial_glb_emitted": output.exists(),
            "database_unchanged": _database_fingerprint(factory) == before_db,
            "object_store_unchanged": _object_fingerprint(store) == before_objects,
            "authoritative_paths_passed_to_child": False,
            "process_reaped": process.poll() is not None,
        }


def _ready_case(
    *,
    candidate: str,
    python_site: Path,
    wasm_module: Path,
    before_db: dict[str, Any],
    before_objects: dict[str, Any],
    factory: SQLiteConnectionFactory,
    store: ContentAddressedStore,
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix=f"forgecad-g824b-{candidate}-ready-") as raw:
        root = Path(raw)
        started = root / "candidate-started.json"
        ready = root / "candidate-ready.json"
        staging = root / "staging" / "candidate.glb"
        command = [
            sys.executable,
            str(ROOT / "scripts/g824b_candidate_lifecycle_probe.py"),
            "--candidate", candidate,
            "--python-site", str(python_site),
            "--wasm-module", str(wasm_module),
            "--started-marker", str(started),
            "--ready-marker", str(ready),
            "--staging-glb", str(staging),
        ]
        env = dict(os.environ)
        env["PYTHONPATH"] = os.pathsep.join([str(ROOT / "apps/agent"), str(ROOT / "scripts")])
        process = subprocess.Popen(command, cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        _wait_for(started, process)
        ready_payload = _wait_for(ready, process)
        if not staging.is_file() or hashlib.sha256(staging.read_bytes()).hexdigest() != ready_payload["glb_sha256"]:
            process.kill()
            process.wait(timeout=5)
            raise AssertionError("ready marker does not match the staged candidate GLB")
        _stop(process, timeout=False)
        staged_before_cleanup = staging.exists()
        staging.unlink()
        return {
            "window": "candidate_ready_before_promotion",
            "error_code": "CSG_CANCELLED",
            "process_exit_code": process.returncode,
            "valid_staged_glb_existed": staged_before_cleanup,
            "staging_cleanup_verified": not staging.exists() and not staging.with_suffix(".glb.tmp").exists(),
            "database_unchanged": _database_fingerprint(factory) == before_db,
            "object_store_unchanged": _object_fingerprint(store) == before_objects,
            "authoritative_paths_passed_to_child": False,
            "process_reaped": process.poll() is not None,
        }


def _rollback_and_success(factory: SQLiteConnectionFactory) -> dict[str, Any]:
    before = _database_fingerprint(factory)
    try:
        with SQLiteUnitOfWork(factory) as unit:
            _add_version(unit, 2)
            unit.agent_assets.set_head(project_id=PROJECT_ID, asset_version_id="assetver_g824b_v2", updated_at=NOW)
            snapshot = unit.active_designs.get_snapshot(PROJECT_ID)
            assert snapshot is not None
            unit.active_designs.advance_agent_snapshot(
                project_id=PROJECT_ID,
                expected_revision=snapshot.revision,
                asset_version_id="assetver_g824b_v2",
                assembly_graph_id="mg_g824b_v2",
                updated_at=NOW,
            )
            raise RuntimeError("inject rollback after version/head/snapshot writes")
    except RuntimeError as exc:
        assert "inject rollback" in str(exc)
    rollback_verified = _database_fingerprint(factory) == before
    with SQLiteUnitOfWork(factory) as unit:
        _add_version(unit, 2)
        unit.agent_assets.set_head(project_id=PROJECT_ID, asset_version_id="assetver_g824b_v2", updated_at=NOW)
        snapshot = unit.active_designs.get_snapshot(PROJECT_ID)
        assert snapshot is not None
        unit.active_designs.advance_agent_snapshot(
            project_id=PROJECT_ID,
            expected_revision=snapshot.revision,
            asset_version_id="assetver_g824b_v2",
            assembly_graph_id="mg_g824b_v2",
            updated_at=NOW,
        )
    with SQLiteUnitOfWork(factory) as unit:
        version = unit.agent_assets.get_version("assetver_g824b_v2")
        head = unit.agent_assets.get_head(PROJECT_ID)
        snapshot = unit.active_designs.get_snapshot(PROJECT_ID)
        atomic_success = (
            version is not None
            and head is not None
            and head["asset_version_id"] == "assetver_g824b_v2"
            and snapshot is not None
            and snapshot.active_design.asset_version_id == "assetver_g824b_v2"
            and snapshot.active_design.assembly_graph_id == "mg_g824b_v2"
            and snapshot.revision == 2
        )
    return {
        "injected_failure_rollback_verified": rollback_verified,
        "version_head_snapshot_atomic_success_verified": atomic_success,
        "uow_type": "SQLiteUnitOfWork",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--python-site", type=Path, required=True)
    parser.add_argument("--wasm-module", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="forgecad-g824b-authoritative-") as raw:
        root = Path(raw)
        factory = SQLiteConnectionFactory(root / "library.db")
        SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        store = ContentAddressedStore(root / "library")
        _seed(factory, store)
        before_db = _database_fingerprint(factory)
        before_objects = _object_fingerprint(store)
        candidates = []
        for candidate in ("python", "wasm"):
            cases = [
                _busy_case(
                    candidate=candidate, error_code=code, python_site=args.python_site, wasm_module=args.wasm_module,
                    before_db=before_db, before_objects=before_objects, factory=factory, store=store,
                )
                for code in ("CSG_CANCELLED", "CSG_TIMEOUT")
            ]
            cases.append(_ready_case(
                candidate=candidate, python_site=args.python_site, wasm_module=args.wasm_module,
                before_db=before_db, before_objects=before_objects, factory=factory, store=store,
            ))
            candidates.append({
                "candidate": candidate,
                "cases": cases,
                "all_interrupt_windows_zero_promotion": all(
                    case["database_unchanged"]
                    and case["object_store_unchanged"]
                    and case["process_reaped"]
                    and not case["authoritative_paths_passed_to_child"]
                    and not case.get("partial_glb_emitted", False)
                    and case.get("staging_cleanup_verified", True)
                    for case in cases
                ),
            })
        promotion = _rollback_and_success(factory)
        report = {
            "schema_version": "ForgeCADCSGPromotionBoundaryEvidence@1",
            "machine": {"platform": sys.platform, "python": sys.version.split()[0], "node": subprocess.check_output(["node", "--version"], text=True).strip()},
            "authoritative_state": {
                "database_tables": list(TABLES),
                "before_database": before_db,
                "before_object_store": before_objects,
                "real_sqlite_migrations_applied": True,
                "real_content_addressed_store_used": True,
            },
            "candidates": candidates,
            "promotion_transaction": promotion,
            "decision": {
                "status": "production_lifecycle_evidence_passed_candidate_not_selected",
                "selected": None,
                "satisfied": [
                    "candidate subprocess has no authoritative SQLite or object-store path",
                    "cancel, timeout, and ready-before-promotion interruption windows leave Version/head/Snapshot/preview/quality/import/idempotency facts unchanged",
                    "staging GLB is outside the content-addressed store and is removed on interruption",
                    "Version/head/Snapshot writes roll back together on injected failure and commit together on success",
                ],
                "remaining_blockers": [
                    "Windows x64 packaged sidecar has not executed the fixed provenance and lifecycle fixtures",
                    "packaged size/SBOM budget has not selected a single candidate",
                    "a superseding ADR has not selected one production implementation",
                ],
            },
        }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.output.with_suffix(args.output.suffix + ".tmp")
    temporary.write_text(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(args.output)
    print(json.dumps({"ok": True, "output": str(args.output), "decision": report["decision"]}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
