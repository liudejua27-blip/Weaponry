#!/usr/bin/env python3
"""G809 smoke: declared parameter bindings constrain preview-first ChangeSets."""

from __future__ import annotations

import tempfile
from pathlib import Path

from forgecad_agent.application.agent_asset_editing import (
    AgentAssetEditingService,
    AgentAssetError,
    _version_row,
)
from forgecad_agent.application.agent_kernel import AgentKernelService
from forgecad_agent.application.agent_models import (
    AgentPartEditOperation,
    BuildAgentBlockoutRequest,
    CommitAgentBlockoutRequest,
    CreateAgentThreadRequest,
    ProposeAgentAssetChangeSetRequest,
    SegmentAgentBlockoutRequest,
    StartAgentTurnRequest,
)
from forgecad_agent.application.mechanical_planner import MechanicalConceptPlan
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner, SQLiteUnitOfWork


ROOT = Path(__file__).resolve().parents[1]
NOW = "2026-07-13T00:00:00+00:00"
PROJECT_ID = "prj_g809_parameter_binding"
LEGACY_PROJECT_ID = "prj_g809_parameter_binding_legacy"


def _seed_project(factory: SQLiteConnectionFactory, project_id: str) -> None:
    connection = factory.connect()
    try:
        connection.execute(
            """
            INSERT OR IGNORE INTO domain_profiles(
              profile_id, domain_type, schema_version, pack_id, display_name,
              profile_json, profile_sha256, status, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                "profile_g809",
                "weapon_concept",
                "DesignDomainProfile@1",
                "weapon-concept-v1-reference",
                "G809 fixture",
                "{}",
                "0" * 64,
                "active",
                NOW,
                NOW,
            ),
        )
        connection.execute(
            """
            INSERT INTO projects(project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at)
            VALUES (?, ?, ?, ?, 'active', NULL, ?, ?)
            """,
            (project_id, "profile_g809", "weapon_concept", f"G809 parameter binding {project_id}", NOW, NOW),
        )
        connection.commit()
    finally:
        connection.close()


def _operation(operation_id: str, part_id: str, path: str, value: float) -> AgentPartEditOperation:
    return AgentPartEditOperation(
        operation_id=operation_id,
        op="set_part_parameter",
        part_id=part_id,
        path=path,
        value=value,
    )


def _proposal(client_request_id: str, operation: AgentPartEditOperation) -> ProposeAgentAssetChangeSetRequest:
    return ProposeAgentAssetChangeSetRequest(
        client_request_id=client_request_id,
        summary="G809 有界参数调整",
        operations=[operation],
    )


def _expect_error(action: object, expected_code: str) -> None:
    try:
        assert callable(action)
        action()
    except AgentAssetError as exc:
        assert exc.code == expected_code, exc.code
        return
    raise AssertionError(f"expected {expected_code}")


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-g809-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        applied = SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        assert "0031" in applied, applied
        _seed_project(factory, PROJECT_ID)
        _seed_project(factory, LEGACY_PROJECT_ID)
        kernel = AgentKernelService(factory)
        assets = AgentAssetEditingService(factory)

        # Create a post-G810 candidate through the real generation path.
        thread = kernel.create_thread(
            CreateAgentThreadRequest(
                client_request_id="g809-thread",
                project_id=PROJECT_ID,
                title="G809 机械臂参数 smoke",
            ),
            "g809-thread",
        )
        turn = kernel.start_turn(
            thread.thread_id,
            StartAgentTurnRequest(client_request_id="g809-turn", message="设计一台三关节机械臂"),
            "g809-turn",
        )
        plan = MechanicalConceptPlan.model_validate(
            next(item.payload["result"] for item in turn.items if item.item_type == "tool_result" and "result" in item.payload)
        )
        direction_id = plan.directions[0].direction_id
        built = kernel.build_blockout(
            BuildAgentBlockoutRequest(client_request_id="g809-build", plan=plan, direction_id=direction_id),
            "g809-build",
        )
        segmented = kernel.segment_blockout(
            SegmentAgentBlockoutRequest(
                client_request_id="g809-segment",
                plan=plan,
                direction_id=direction_id,
                artifact_id=built.artifact_id,
            ),
            "g809-segment",
        )
        generated_version = assets.commit_blockout(
            CommitAgentBlockoutRequest(client_request_id="g809-commit", artifact_id=segmented.artifact_id),
            "g809-commit",
        )
        target_part = next(item for item in generated_version.parts if item.editable_parameter_bindings)
        assert [item.path for item in target_part.editable_parameter_bindings] == [
            "transform.scale.x",
            "transform.scale.y",
            "transform.scale.z",
        ]

        # An independent immutable fixture represents a pre-G808 asset.  The
        # current generator must not be used as evidence that old rows are safe.
        legacy_version = generated_version.model_copy(
            update={
                "asset_version_id": "assetver_g809_legacy_v1",
                "project_id": LEGACY_PROJECT_ID,
                "parent_asset_version_id": None,
                "version_no": 1,
                "summary": "G809 legacy empty-binding fixture",
                "parts": [item.model_copy(update={"editable_parameter_bindings": []}) for item in generated_version.parts],
                "created_at": NOW,
            }
        )
        with SQLiteUnitOfWork(factory) as unit:
            unit.agent_assets.add_version(**_version_row(legacy_version))
            unit.agent_assets.set_head(project_id=LEGACY_PROJECT_ID, asset_version_id=legacy_version.asset_version_id, updated_at=NOW)
            unit.active_designs.create_agent_snapshot(
                project_id=LEGACY_PROJECT_ID,
                asset_version_id=legacy_version.asset_version_id,
                assembly_graph_id=str(legacy_version.assembly_graph["graph_id"]),
                updated_at=NOW,
            )
        legacy_target_part = legacy_version.parts[0]
        assert legacy_target_part.editable_parameter_bindings == []

        # Empty legacy bindings retain only the fixed historic six-path policy.
        legacy_change = assets.propose_change_set(
            legacy_version.asset_version_id,
            _proposal("g809-legacy-propose", _operation("op_g809_legacy", legacy_target_part.part_id, "transform.scale.x", 0.8)),
            "g809-legacy-propose",
        )
        legacy_preview = assets.preview_change_set(legacy_change.change_set_id, "g809-legacy-preview")
        assert legacy_preview.preview is not None
        with SQLiteUnitOfWork(factory) as unit:
            snapshot = unit.active_designs.get_snapshot(LEGACY_PROJECT_ID)
            assert snapshot is not None
            assert snapshot.active_design.asset_version_id == legacy_version.asset_version_id
            assert len(unit.agent_assets.list_versions(LEGACY_PROJECT_ID)) == 1, "preview must not write an asset version"
        assets.reject_change_set(legacy_change.change_set_id, "g809-legacy-reject")
        _expect_error(
            lambda: assets.propose_change_set(
                legacy_version.asset_version_id,
                _proposal("g809-legacy-arbitrary", _operation("op_g809_legacy_arbitrary", legacy_target_part.part_id, "transform.rotation.x", 1.0)),
                "g809-legacy-arbitrary",
            ),
            "PARAMETER_NOT_ALLOWED",
        )

        # The actual newly generated asset now exercises the declared branch.
        bound_version = generated_version
        with SQLiteUnitOfWork(factory) as unit:
            snapshot = unit.active_designs.get_snapshot(PROJECT_ID)
            assert snapshot is not None
            active = unit.active_designs.set_part_display(
                project_id=PROJECT_ID,
                expected_revision=snapshot.revision,
                action="lock",
                part_id=target_part.part_id,
                updated_at=NOW,
            )
            assert active.active_design.asset_version_id == bound_version.asset_version_id
        _expect_error(
            lambda: assets.propose_change_set(
                bound_version.asset_version_id,
                _proposal("g809-locked", _operation("op_g809_locked", target_part.part_id, "transform.scale.y", 1.0)),
                "g809-locked",
            ),
            "PART_PROTECTED",
        )
        with SQLiteUnitOfWork(factory) as unit:
            unlocked = unit.active_designs.set_part_display(
                project_id=PROJECT_ID,
                expected_revision=active.revision,
                action="unlock",
                part_id=target_part.part_id,
                updated_at=NOW,
            )
            assert target_part.part_id not in unlocked.part_display.locked_part_ids

        _expect_error(
            lambda: assets.propose_change_set(
                bound_version.asset_version_id,
                _proposal("g809-undeclared", _operation("op_g809_undeclared", target_part.part_id, "transform.position.x", 1.0)),
                "g809-undeclared",
            ),
            "PARAMETER_NOT_DECLARED",
        )
        _expect_error(
            lambda: assets.propose_change_set(
                bound_version.asset_version_id,
                _proposal("g809-range", _operation("op_g809_range", target_part.part_id, "transform.scale.x", 1.5)),
                "g809-range",
            ),
            "PARAMETER_OUT_OF_RANGE",
        )
        _expect_error(
            lambda: assets.propose_change_set(
                bound_version.asset_version_id,
                _proposal("g809-step", _operation("op_g809_step", target_part.part_id, "transform.scale.x", 0.85)),
                "g809-step",
            ),
            "PARAMETER_STEP_MISMATCH",
        )

        change_set = assets.propose_change_set(
            bound_version.asset_version_id,
            _proposal("g809-valid", _operation("op_g809_valid", target_part.part_id, "transform.scale.x", 1.2)),
            "g809-valid",
        )
        preview = assets.preview_change_set(change_set.change_set_id, "g809-preview")
        assert preview.preview is not None
        with SQLiteUnitOfWork(factory) as unit:
            snapshot = unit.active_designs.get_snapshot(PROJECT_ID)
            assert snapshot is not None and snapshot.preview is not None
            assert snapshot.active_design.asset_version_id == bound_version.asset_version_id
            assert len(unit.agent_assets.list_versions(PROJECT_ID)) == 1, "preview must not create a child version"

        confirmed = assets.confirm_change_set(change_set.change_set_id, "g809-confirm")
        assert confirmed.asset_version.parent_asset_version_id == bound_version.asset_version_id
        assert confirmed.asset_version.version_no == 2
        confirmed_part = next(item for item in confirmed.asset_version.parts if item.part_id == target_part.part_id)
        assert confirmed_part.editable_parameter_bindings == target_part.editable_parameter_bindings
        with SQLiteUnitOfWork(factory) as unit:
            snapshot = unit.active_designs.get_snapshot(PROJECT_ID)
            head = unit.agent_assets.get_head(PROJECT_ID)
            assert snapshot is not None and head is not None
            assert snapshot.active_design.asset_version_id == confirmed.asset_version.asset_version_id
            assert snapshot.export.source_version_id == confirmed.asset_version.asset_version_id
            assert snapshot.preview is None
            assert head["asset_version_id"] == confirmed.asset_version.asset_version_id
            assert len(unit.agent_assets.list_versions(PROJECT_ID)) == 2

    print("G809 parameter binding ChangeSet smoke passed: declarations, step/range, lock priority, legacy fixed paths, preview and immutable child version")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
