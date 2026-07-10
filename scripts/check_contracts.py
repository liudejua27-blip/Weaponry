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
    schema_dirs = (
        ROOT / "packages" / "weapon-spec" / "schemas",
        ROOT / "packages" / "concept-spec" / "schemas",
    )
    required_concept_schemas = {
        "common.schema.json",
        "design-domain-profile.schema.json",
        "weapon-concept-spec.schema.json",
        "module-asset-manifest.schema.json",
        "module-graph.schema.json",
        "design-change-set.schema.json",
        "model-quality-report.schema.json",
        "job-event-v2.schema.json",
    }
    for schema_dir in schema_dirs:
        names = set()
        for path in sorted(schema_dir.glob("*.json")):
            json.loads(path.read_text(encoding="utf-8"))
            names.add(path.name)
        if schema_dir.name == "schemas" and schema_dir.parent.name == "concept-spec":
            missing = required_concept_schemas - names
            if missing:
                raise RuntimeError(f"missing concept schemas: {sorted(missing)}")


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
            "domain_profiles",
            "projects",
            "project_versions",
            "concept_assets",
            "module_assets",
            "module_connectors",
            "module_graphs",
            "module_graph_nodes",
            "module_graph_edges",
            "design_briefs",
            "design_variants",
            "design_change_sets",
            "quality_runs",
            "quality_findings",
            "export_packages_v2",
            "artifact_links",
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
