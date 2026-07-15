#!/usr/bin/env python3
"""Gate quality facts against the compiled GLB readback, never an estimate."""

from __future__ import annotations

import base64
import hashlib
import json
import tempfile
from pathlib import Path

from forgecad_agent.application import geometry_worker
from forgecad_agent.application.agent_asset_editing import AgentAssetEditingService, AgentAssetError
from forgecad_agent.application.agent_kernel import AgentKernelService
from forgecad_agent.application.agent_models import (
    AgentAssetQualityReport,
    BuildAgentBlockoutRequest,
    CommitAgentBlockoutRequest,
    CreateAgentThreadRequest,
    SegmentAgentBlockoutRequest,
    StartAgentTurnRequest,
)
from forgecad_agent.application.mechanical_planner import MechanicalConceptPlan
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner, SQLiteUnitOfWork


ROOT = Path(__file__).resolve().parents[1]
CASES = (
    ("future", "设计一个虚构非功能的未来概念道具", "pack_future_weapon_prop"),
    ("vehicle", "设计一辆城市探索汽车", "pack_vehicle_concept"),
    ("aircraft", "设计一架垂直起降概念飞机", "pack_aircraft_concept"),
    ("robotic", "设计一台三关节机械臂", "pack_robotic_arm_concept"),
)


def _seed(factory: SQLiteConnectionFactory) -> None:
    now = "2026-07-15T00:00:00+00:00"
    with factory.connect() as connection:
        connection.execute(
            """INSERT INTO domain_profiles(profile_id, domain_type, schema_version, pack_id, display_name, profile_json, profile_sha256, status, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)""",
            ("profile_weapon_concept_v1", "weapon_concept", "DesignDomainProfile@1", "weapon-concept-v1-reference", "Q003", "{}", "0" * 64, "active", now, now),
        )
        for suffix, _, _ in CASES:
            connection.execute(
                """INSERT INTO projects(project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?)""",
                (f"prj_q003_{suffix}", "profile_weapon_concept_v1", "weapon_concept", f"Q003 {suffix}", "active", None, now, now),
            )


def _make_asset(factory: SQLiteConnectionFactory, suffix: str, message: str, expected_pack: str):
    kernel = AgentKernelService(factory)
    assets = AgentAssetEditingService(factory)
    thread = kernel.create_thread(
        CreateAgentThreadRequest(client_request_id=f"q003-thread-{suffix}", project_id=f"prj_q003_{suffix}", title=f"Q003 {suffix}"),
        f"q003-thread-{suffix}",
    )
    turn = kernel.start_turn(
        thread.thread_id,
        StartAgentTurnRequest(client_request_id=f"q003-turn-{suffix}", message=message),
        f"q003-turn-{suffix}",
    )
    plan = MechanicalConceptPlan.model_validate(next(item.payload["result"] for item in turn.items if item.item_type == "tool_result"))
    assert plan.domain_pack_id == expected_pack, plan.domain_pack_id
    direction_id = plan.directions[0].direction_id
    built = kernel.build_blockout(
        BuildAgentBlockoutRequest(client_request_id=f"q003-build-{suffix}", plan=plan, direction_id=direction_id),
        f"q003-build-{suffix}",
    )
    segmented = kernel.segment_blockout(
        SegmentAgentBlockoutRequest(client_request_id=f"q003-segment-{suffix}", plan=plan, direction_id=direction_id, artifact_id=built.artifact_id),
        f"q003-segment-{suffix}",
    )
    return assets, assets.commit_blockout(
        CommitAgentBlockoutRequest(client_request_id=f"q003-commit-{suffix}", artifact_id=segmented.artifact_id),
        f"q003-commit-{suffix}",
    )


def _revision(factory: SQLiteConnectionFactory, project_id: str) -> int:
    with SQLiteUnitOfWork(factory) as unit:
        snapshot = unit.active_designs.get_snapshot(project_id)
        assert snapshot is not None
        return snapshot.revision


def _assert_error(action, code: str) -> None:
    try:
        action()
    except AgentAssetError as exc:
        assert exc.code == code, exc
        return
    raise AssertionError(f"expected {code}")


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-q003-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        _seed(factory)
        versions = []

        for suffix, message, pack in CASES:
            assets, version = _make_asset(factory, suffix, message, pack)
            compiled = geometry_worker.compile_shape_program(version.shape_program)
            revision = _revision(factory, version.project_id)
            report = assets.quality(
                version.asset_version_id,
                expected_revision=revision,
                idempotency_key=f"q003-quality-{suffix}",
            )
            assert report.evidence_source == "geometry_compile_readback"
            assert report.compile_readback == compiled.readback
            assert report.triangle_count == compiled.readback.triangle_count
            assert report.bounds_mm == compiled.readback.bounds_mm

            exported = assets.export_glb(version.asset_version_id)
            exported_glb = base64.b64decode(exported.glb_base64, validate=True)
            assert exported.triangle_count == report.triangle_count
            assert exported.bounds_mm == report.bounds_mm
            assert hashlib.sha256(exported_glb).hexdigest() == report.compile_readback.glb_sha256

            # A process restart must replay the immutable report before compiling.
            restarted = AgentAssetEditingService(factory)
            original_compile = __import__(
                "forgecad_agent.application.agent_asset_editing", fromlist=["compile_shape_program"]
            ).compile_shape_program
            editing_module = __import__("forgecad_agent.application.agent_asset_editing", fromlist=["compile_shape_program"])
            editing_module.compile_shape_program = lambda _program: (_ for _ in ()).throw(AssertionError("replay recompiled geometry"))
            try:
                assert restarted.quality(
                    version.asset_version_id,
                    expected_revision=revision,
                    idempotency_key=f"q003-quality-{suffix}",
                ) == report
            finally:
                editing_module.compile_shape_program = original_compile
            versions.append((assets, version, report))

        # A pre-Q003 estimate remains readable only as unavailable evidence and
        # cannot replace a current readback-backed report.
        assets, version, current = versions[0]
        legacy = AgentAssetQualityReport(
            quality_report_id="quality_q003_legacy",
            asset_version_id=version.asset_version_id,
            status="passed",
            triangle_count=999999,
            findings=[],
            checked_at="2026-07-14T00:00:00+00:00",
        )
        with SQLiteUnitOfWork(factory) as unit:
            unit.agent_assets.add_quality_report(
                quality_report_id=legacy.quality_report_id,
                project_id=version.project_id,
                asset_version_id=version.asset_version_id,
                report_json=json.dumps(legacy.model_dump(mode="json"), ensure_ascii=False, sort_keys=True),
                status=legacy.status,
                created_at=legacy.checked_at,
            )
        isolated = assets.get_quality_report(legacy.quality_report_id)
        assert isolated.status == "unavailable" and isolated.triangle_count == 0
        assert isolated.evidence_source == "legacy_estimate"
        assert assets.get_quality_report(current.quality_report_id) == current

        # Damaged GLB readback produces explicit unavailable quality and blocks export.
        assets, version, _ = versions[1]
        original_readback = geometry_worker.read_shape_program_glb_facts
        geometry_worker.read_shape_program_glb_facts = lambda _payload: (_ for _ in ()).throw(ValueError("damaged GLB"))
        try:
            failure = assets.quality(
                version.asset_version_id,
                expected_revision=_revision(factory, version.project_id),
                idempotency_key="q003-damaged-readback",
            )
            assert failure.status == "unavailable" and failure.evidence_source == "compile_failure"
            assert failure.triangle_count == 0 and failure.bounds_mm is None and failure.compile_readback is None
            _assert_error(lambda: assets.export_glb(version.asset_version_id), "GEOMETRY_READBACK_FAILED")
        finally:
            geometry_worker.read_shape_program_glb_facts = original_readback

        # Persisted unknown operations remain fail-closed and write no report.
        assets, version, _ = versions[-1]
        unknown = json.loads(json.dumps(version.shape_program))
        unknown["operations"][0]["op"] = "pivot"
        with factory.connect() as connection:
            before = connection.execute("SELECT COUNT(*) FROM agent_asset_quality_reports").fetchone()[0]
            connection.execute(
                "UPDATE agent_asset_versions SET shape_program_json = ? WHERE asset_version_id = ?",
                (json.dumps(unknown, ensure_ascii=False, sort_keys=True), version.asset_version_id),
            )
        _assert_error(lambda: assets.quality(version.asset_version_id), "UNSUPPORTED_RUNTIME_OPERATION")
        with factory.connect() as connection:
            after = connection.execute("SELECT COUNT(*) FROM agent_asset_quality_reports").fetchone()[0]
        assert after == before

    print("Q003 compile/readback quality smoke passed: four domains share immutable GLB facts; failures and legacy estimates are isolated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
