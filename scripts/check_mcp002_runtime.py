#!/usr/bin/env python3
"""Static ownership and boundary gate for FGC-MCP002."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(relative: str) -> str:
    path = ROOT / relative
    if not path.exists():
        raise SystemExit(f"MCP002 input missing: {relative}")
    return path.read_text(encoding="utf-8")


def main() -> int:
    migration = read("migrations-runtime-v1/0001_runtime.sql")
    required_tables = (
        "schema_meta",
        "writer_lease",
        "projects",
        "snapshots",
        "candidates",
        "design_asset_versions",
        "runtime_jobs",
        "runtime_job_events",
        "objects",
        "audit_events",
    )
    missing_tables = [table for table in required_tables if f"CREATE TABLE IF NOT EXISTS {table}" not in migration]
    if missing_tables:
        raise SystemExit(f"MCP002 migration missing tables: {missing_tables}")

    store = read("apps/desktop/src-tauri/crates/forgecad-store/src/lib.rs")
    for marker in ("sync_all", "rename", "LegacyDatabaseRejected", "backup_to", "restore_from"):
        if marker not in store:
            raise SystemExit(f"MCP002 Store marker missing: {marker}")
    runtime_ipc = read("apps/desktop/src-tauri/crates/forgecad-runtime/src/ipc.rs")
    for marker in ("0o600", "constant_time_equal", "AUTH_FAILED", "token_hash"):
        if marker not in runtime_ipc:
            raise SystemExit(f"MCP002 IPC marker missing: {marker}")

    mcp_manifest = read("apps/desktop/src-tauri/crates/forgecad-mcp/Cargo.toml")
    if "rusqlite" in mcp_manifest or "TcpListener" in read("apps/desktop/src-tauri/crates/forgecad-mcp/src/main.rs"):
        raise SystemExit("MCP must not own SQLite or TCP listeners")

    evidence = json.loads(read("docs/evidence/mcp002/manifest.json"))
    if evidence.get("task_id") != "FGC-MCP002":
        raise SystemExit("MCP002 evidence task id mismatch")
    print("MCP002 runtime boundary OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
