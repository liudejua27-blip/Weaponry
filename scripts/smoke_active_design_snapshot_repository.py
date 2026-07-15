#!/usr/bin/env python3
"""S002 migration/repository smoke for ActiveDesignSnapshot persistence and CAS."""

from __future__ import annotations

import hashlib
import json
import shutil
import tempfile
from pathlib import Path

from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner, SQLiteUnitOfWork
from forgecad_agent.infrastructure.db.agent_repositories import (
    ActiveDesignSnapshotConflict,
    ActiveDesignSnapshotError,
)


ROOT = Path(__file__).resolve().parents[1]
NOW = "2026-07-13T00:00:00+00:00"


def _canonical(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _seed_project(factory: SQLiteConnectionFactory, project_id: str) -> None:
    connection = factory.connect()
    try:
        connection.execute(
            """
            INSERT OR IGNORE INTO domain_profiles(
              profile_id, domain_type, schema_version, pack_id, display_name,
              profile_json, profile_sha256, status, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)
            """,
            (
                "profile_weapon_concept_v1",
                "weapon_concept",
                "DesignDomainProfile@1",
                "weapon-concept-v1-reference",
                "S002 fixture",
                "{}",
                "0" * 64,
                NOW,
                NOW,
            ),
        )
        connection.execute(
            """
            INSERT INTO projects(
              project_id, profile_id, domain_type, name, status,
              current_version_id, created_at, updated_at
            ) VALUES (?, 'profile_weapon_concept_v1', 'weapon_concept', ?, 'active', NULL, ?, ?)
            """,
            (project_id, f"S002 {project_id}", NOW, NOW),
        )
        connection.commit()
    finally:
        connection.close()


def _seed_legacy_version(factory: SQLiteConnectionFactory, project_id: str) -> tuple[str, str]:
    graph_id = "mg_s002_legacy"
    version_id = "ver_s002_legacy"
    connection = factory.connect()
    try:
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
            ) VALUES (?, ?, NULL, 1, 'committed', 'legacy fixture', 'WeaponConceptSpec@1', '{}', ?, ?, NULL, ?)
            """,
            (version_id, project_id, "2" * 64, graph_id, NOW),
        )
        connection.commit()
    finally:
        connection.close()
    return version_id, graph_id


def _legacy_hash(factory: SQLiteConnectionFactory, project_id: str) -> str:
    connection = factory.connect()
    try:
        tables = ("projects", "project_versions", "module_graphs")
        payload = {
            table: [dict(row) for row in connection.execute(f"SELECT * FROM {table} WHERE project_id = ?", (project_id,)).fetchall()]
            for table in tables
        }
    finally:
        connection.close()
    return hashlib.sha256(_canonical(payload).encode("utf-8")).hexdigest()


def _seed_agent_version(
    unit: SQLiteUnitOfWork,
    *,
    project_id: str,
    asset_version_id: str,
    version_no: int,
    graph_id: str,
) -> None:
    graph = {
        "schema_version": "AssemblyGraph@1",
        "graph_id": graph_id,
        "parts": [{"part_id": "part_s002_body"}],
        "connections": [],
    }
    unit.agent_assets.add_version(
        asset_version_id=asset_version_id,
        project_id=project_id,
        parent_asset_version_id=None,
        version_no=version_no,
        status="committed",
        summary="S002 fixture asset",
        stage="editable_asset",
        plan_id="plan_s002",
        direction_id="direction_s002",
        domain_pack_id="pack_vehicle_concept",
        artifact_id=f"artifact_{asset_version_id}",
        parts_json='[{"part_id":"part_s002_body"}]',
        shape_program_json='{"schema_version":"ShapeProgram@1","operations":[]}',
        assembly_graph_json=_canonical(graph),
        material_bindings_json="{}",
        created_at=NOW,
    )


def _expect_error(callback: object, expected: type[Exception]) -> None:
    try:
        callback()  # type: ignore[operator]
    except expected:
        return
    raise AssertionError(f"expected {expected.__name__}")


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-s002-snapshot-") as raw:
        root = Path(raw)
        old_migrations = root / "migrations-before-s002"
        s002_migrations = root / "migrations-through-s002"
        old_migrations.mkdir()
        s002_migrations.mkdir()
        for migration in sorted((ROOT / "migrations").glob("*.sql")):
            if migration.name < "0023_active_design_snapshots.sql":
                shutil.copy2(migration, old_migrations / migration.name)
            if migration.name <= "0023_active_design_snapshots.sql":
                shutil.copy2(migration, s002_migrations / migration.name)

        factory = SQLiteConnectionFactory(root / "library.db")
        applied_old = SQLiteMigrationRunner(factory, old_migrations).run()
        assert "0022" in applied_old and "0023" not in applied_old
        _seed_project(factory, "prj_s002_agent")
        _seed_project(factory, "prj_s002_other")
        _seed_project(factory, "prj_s002_legacy")
        legacy_version_id, legacy_graph_id = _seed_legacy_version(factory, "prj_s002_legacy")
        legacy_before = _legacy_hash(factory, "prj_s002_legacy")

        applied_upgrade = SQLiteMigrationRunner(factory, s002_migrations).run()
        # 0017 is a pre-existing idempotent migration without a ledger insert;
        # the upgrade contract is that S002 is applied and legacy rows remain
        # unchanged even when the runner safely replays that older migration.
        assert "0023" in applied_upgrade, applied_upgrade
        assert set(applied_upgrade).issubset({"0017", "0023"}), applied_upgrade
        assert _legacy_hash(factory, "prj_s002_legacy") == legacy_before
        assert SQLiteMigrationRunner(factory, s002_migrations).run() == []

        with SQLiteUnitOfWork(factory) as unit:
            _seed_agent_version(
                unit,
                project_id="prj_s002_agent",
                asset_version_id="assetver_s002_v1",
                version_no=1,
                graph_id="mg_s002_v1",
            )
            _seed_agent_version(
                unit,
                project_id="prj_s002_agent",
                asset_version_id="assetver_s002_v2",
                version_no=2,
                graph_id="mg_s002_v2",
            )
            _seed_agent_version(
                unit,
                project_id="prj_s002_other",
                asset_version_id="assetver_s002_other",
                version_no=1,
                graph_id="mg_s002_other",
            )
            unit.agent_assets.add_change_set(
                change_set_id="assetcs_s002_v1",
                project_id="prj_s002_agent",
                base_asset_version_id="assetver_s002_v1",
                summary="S002 preview fixture",
                operations_json="[]",
                protected_part_ids_json="[]",
                created_at=NOW,
            )

            snapshot = unit.active_designs.create_agent_snapshot(
                project_id="prj_s002_agent",
                asset_version_id="assetver_s002_v1",
                assembly_graph_id="mg_s002_v1",
                updated_at=NOW,
            )
            assert snapshot.revision == 1
            assert snapshot.export.source_version_id == "assetver_s002_v1"
            assert unit.active_designs.get_snapshot("prj_s002_agent") is not None

            selected = unit.active_designs.select_agent_part(
                project_id="prj_s002_agent",
                expected_revision=1,
                part_id="part_s002_body",
                updated_at=NOW,
            )
            assert selected.revision == 2 and selected.selected_part_id == "part_s002_body"
            _expect_error(
                lambda: unit.active_designs.select_agent_part(
                    project_id="prj_s002_agent",
                    expected_revision=1,
                    part_id=None,
                    updated_at=NOW,
                ),
                ActiveDesignSnapshotConflict,
            )
            _expect_error(
                lambda: unit.active_designs.select_agent_part(
                    project_id="prj_s002_agent",
                    expected_revision=2,
                    part_id="part_missing",
                    updated_at=NOW,
                ),
                ActiveDesignSnapshotError,
            )

            preview = unit.active_designs.set_preview(
                project_id="prj_s002_agent",
                expected_revision=2,
                change_set_id="assetcs_s002_v1",
                base_asset_version_id="assetver_s002_v1",
                updated_at=NOW,
            )
            assert preview.revision == 3 and preview.preview is not None
            quality = unit.active_designs.set_quality(
                project_id="prj_s002_agent",
                expected_revision=3,
                quality_report_id="quality_s002_v1",
                asset_version_id="assetver_s002_v1",
                updated_at=NOW,
            )
            assert quality.revision == 4 and quality.quality is not None

            advanced = unit.active_designs.advance_agent_snapshot(
                project_id="prj_s002_agent",
                expected_revision=4,
                asset_version_id="assetver_s002_v2",
                assembly_graph_id="mg_s002_v2",
                updated_at=NOW,
            )
            assert advanced.revision == 5
            assert advanced.selected_part_id is None and advanced.preview is None and advanced.quality is None
            assert advanced.export.source_version_id == "assetver_s002_v2"
            _expect_error(
                lambda: unit.active_designs.advance_agent_snapshot(
                    project_id="prj_s002_agent",
                    expected_revision=5,
                    asset_version_id="assetver_s002_other",
                    assembly_graph_id="mg_s002_other",
                    updated_at=NOW,
                ),
                ActiveDesignSnapshotError,
            )

            legacy = unit.active_designs.create_legacy_snapshot(
                project_id="prj_s002_legacy",
                legacy_version_id=legacy_version_id,
                module_graph_id=legacy_graph_id,
                updated_at=NOW,
            )
            assert legacy.revision == 1
            _expect_error(
                lambda: unit.active_designs.select_agent_part(
                    project_id="prj_s002_legacy",
                    expected_revision=1,
                    part_id="part_s002_body",
                    updated_at=NOW,
                ),
                ActiveDesignSnapshotError,
            )

        empty_factory = SQLiteConnectionFactory(root / "empty.db")
        applied_empty = SQLiteMigrationRunner(empty_factory, ROOT / "migrations").run()
        assert "0023" in applied_empty
        with SQLiteUnitOfWork(empty_factory) as unit:
            assert unit.active_designs.get_snapshot("prj_not_created") is None

    print("S002 ActiveDesignSnapshot migration/repository/CAS smoke passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
