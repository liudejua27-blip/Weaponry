#!/usr/bin/env python3
"""Check machine-readable contracts for the M1 skeleton."""

from __future__ import annotations

import json
import pathlib
import sqlite3
import sys
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]


def check_json_schemas() -> None:
    schema_dir = ROOT / "packages" / "weapon-spec" / "schemas"
    for path in sorted(schema_dir.glob("*.json")):
        json.loads(path.read_text(encoding="utf-8"))


def check_sqlite_migration() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        db = pathlib.Path(tmp) / "check.db"
        conn = sqlite3.connect(db)
        for migration in sorted((ROOT / "migrations").glob("*.sql")):
            conn.executescript(migration.read_text(encoding="utf-8"))
        tables = {
            row[0]
            for row in conn.execute(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
            )
        }
        required = {
            "schema_migrations",
            "library_meta",
            "idempotency_records",
            "weapons",
            "generation_jobs",
            "job_steps",
            "weapon_versions",
            "weapon_specs",
            "provider_configs",
            "asset_files",
            "agent_events",
            "models_3d",
            "export_packages",
        }
        missing = required - tables
        if missing:
            raise RuntimeError(f"missing tables: {sorted(missing)}")
        conn.close()


def main() -> int:
    try:
        check_json_schemas()
        check_sqlite_migration()
    except Exception as exc:  # noqa: BLE001 - contract checker should print all failure types.
        print(f"contract check failed: {exc}", file=sys.stderr)
        return 1
    print("contract check ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
