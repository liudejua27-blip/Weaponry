#!/usr/bin/env python3
"""S007 smoke: explicit legacy-to-Agent promotion preserves legacy records."""

from __future__ import annotations

import hashlib
import json
import tempfile
from pathlib import Path

from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner, SQLiteUnitOfWork
from forgecad_agent.infrastructure.db.agent_repositories import ActiveDesignSnapshotError


ROOT = Path(__file__).resolve().parents[1]
NOW = "2026-07-13T00:00:00+00:00"


def _canonical(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _seed_legacy_project(factory: SQLiteConnectionFactory) -> tuple[str, str, str]:
    project_id = "prj_s007_legacy"
    version_id = "ver_s007_legacy"
    graph_id = "mg_s007_legacy"
    connection = factory.connect()
    try:
        connection.execute(
            """
            INSERT INTO domain_profiles(
              profile_id, domain_type, schema_version, pack_id, display_name,
              profile_json, profile_sha256, status, created_at, updated_at
            ) VALUES ('profile_s007', 'weapon_concept', 'DesignDomainProfile@1',
                      'weapon-concept-v1-reference', 'S007 fixture', '{}', ?, 'active', ?, ?)
            """,
            ("0" * 64, NOW, NOW),
        )
        connection.execute(
            """
            INSERT INTO projects(
              project_id, profile_id, domain_type, name, status,
              current_version_id, created_at, updated_at
            ) VALUES (?, 'profile_s007', 'weapon_concept', 'S007 legacy', 'active', ?, ?, ?)
            """,
            (project_id, version_id, NOW, NOW),
        )
        connection.execute(
            """
            INSERT INTO module_graphs(
              graph_id, project_id, version_id, root_node_id, schema_version,
              graph_json, graph_sha256, validation_status, created_at, updated_at
            ) VALUES (?, ?, NULL, 'node_root', 'ModuleGraph@1', ?, ?, 'valid', ?, ?)
            """,
            (graph_id, project_id, '{"schema_version":"ModuleGraph@1"}', "1" * 64, NOW, NOW),
        )
        connection.execute(
            """
            INSERT INTO project_versions(
              version_id, project_id, parent_version_id, version_no, status,
              summary, spec_schema_version, spec_json, spec_sha256,
              module_graph_id, change_set_id, created_at
            ) VALUES (?, ?, NULL, 1, 'committed', 'legacy source', 'WeaponConceptSpec@1', '{}', ?, ?, NULL, ?)
            """,
            (version_id, project_id, "2" * 64, graph_id, NOW),
        )
        connection.commit()
    finally:
        connection.close()
    return project_id, version_id, graph_id


def _legacy_hash(factory: SQLiteConnectionFactory, project_id: str) -> str:
    connection = factory.connect()
    try:
        payload = {
            table: [dict(row) for row in connection.execute(f"SELECT * FROM {table} WHERE project_id = ?", (project_id,)).fetchall()]
            for table in ("projects", "project_versions", "module_graphs")
        }
    finally:
        connection.close()
    return hashlib.sha256(_canonical(payload).encode("utf-8")).hexdigest()


def _seed_agent_version(unit: SQLiteUnitOfWork, project_id: str) -> tuple[str, str]:
    asset_version_id = "assetver_s007_rebuilt"
    graph_id = "mg_s007_rebuilt"
    graph = {
        "schema_version": "AssemblyGraph@1",
        "graph_id": graph_id,
        "parts": [{"part_id": "part_s007_body"}],
        "connections": [],
    }
    unit.agent_assets.add_version(
        asset_version_id=asset_version_id,
        project_id=project_id,
        parent_asset_version_id=None,
        version_no=1,
        status="committed",
        summary="explicit Agent rebuild",
        stage="editable_asset",
        plan_id="plan_s007",
        direction_id="direction_s007",
        domain_pack_id="pack_future_weapon_prop",
        artifact_id="artifact_s007_rebuilt",
        parts_json='[{"part_id":"part_s007_body"}]',
        shape_program_json='{"schema_version":"ShapeProgram@1","operations":[]}',
        assembly_graph_json=_canonical(graph),
        material_bindings_json="{}",
        created_at=NOW,
    )
    return asset_version_id, graph_id


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-s007-legacy-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        applied = SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        assert "0024" in applied, applied
        project_id, legacy_version_id, legacy_graph_id = _seed_legacy_project(factory)
        legacy_before = _legacy_hash(factory, project_id)

        with SQLiteUnitOfWork(factory) as unit:
            legacy = unit.active_designs.create_legacy_snapshot(
                project_id=project_id,
                legacy_version_id=legacy_version_id,
                module_graph_id=legacy_graph_id,
                updated_at=NOW,
            )
            asset_version_id, agent_graph_id = _seed_agent_version(unit, project_id)
            try:
                unit.active_designs.promote_legacy_to_agent_snapshot(
                    project_id=project_id,
                    expected_revision=legacy.revision,
                    asset_version_id=asset_version_id,
                    assembly_graph_id=agent_graph_id,
                    updated_at=NOW,
                )
            except ActiveDesignSnapshotError:
                pass
            else:
                raise AssertionError("legacy promotion must require an explicit conversion intent")

            unit.active_designs.record_legacy_conversion_intent(
                project_id=project_id,
                expected_revision=legacy.revision,
                legacy_version_id=legacy_version_id,
                module_graph_id=legacy_graph_id,
                requested_at=NOW,
            )
            promoted = unit.active_designs.promote_legacy_to_agent_snapshot(
                project_id=project_id,
                expected_revision=legacy.revision,
                asset_version_id=asset_version_id,
                assembly_graph_id=agent_graph_id,
                updated_at=NOW,
            )
            assert promoted.active_design.source == "agent_asset"
            assert promoted.active_design.asset_version_id == asset_version_id
            assert promoted.revision == legacy.revision + 1
            assert unit.active_designs.get_legacy_conversion_intent(project_id) is None

        assert _legacy_hash(factory, project_id) == legacy_before

    print("S007 explicit legacy conversion intent/promotion preservation smoke passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
