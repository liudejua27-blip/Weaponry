#!/usr/bin/env python3
"""FGC-G825: single Manifold CSG, immutable feature history and promotion gate."""

from __future__ import annotations

import base64
import copy
import hashlib
import json
import tempfile
import time
from pathlib import Path

from forgecad_agent.application.agent_asset_editing import AgentAssetEditingService
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
from forgecad_agent.application.geometry_worker import compile_shape_program
from forgecad_agent.application.manifold_csg import ManifoldCsgError
from forgecad_agent.application.mechanical_planner import MechanicalConceptPlan
from forgecad_agent.application.shape_program import ShapeProgramValidationError
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner, SQLiteUnitOfWork


ROOT = Path(__file__).resolve().parents[1]


def boolean_program(
    operation: str,
    *,
    suffix: str,
    base_size: list[float] | None = None,
    tool_size: list[float] | None = None,
    tool_position: list[float] | None = None,
    material_pair: tuple[str, str] = ("mat_automotive_paint", "mat_dark_glass"),
) -> dict:
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": f"shape_g825_{suffix}",
        "units": "millimeter",
        "seed": 825,
        "triangle_budget": 100000,
        "parameters": [],
        "operations": [
            {
                "operation_id": "op_base",
                "op": "box",
                "inputs": [],
                "args": {
                    "position": [0, 0, 0],
                    "size": base_size or [1000, 700, 500],
                    "part_role": "base_shell",
                    "zone_id": "zone_shell",
                    "material_id": material_pair[0],
                },
            },
            {
                "operation_id": "op_tool",
                "op": "box",
                "inputs": [],
                "args": {
                    "position": tool_position or [260, 0, 0],
                    "size": tool_size or [520, 520, 620],
                    "part_role": "trim_tool",
                    "zone_id": "zone_trim",
                    "material_id": material_pair[1],
                },
            },
            {
                "operation_id": "op_boolean",
                "op": operation,
                "inputs": ["op_base", "op_tool"],
                "args": {"part_role": "finished_shell", "zone_id": "zone_shell"},
            },
        ],
        "outputs": [
            {
                "output_id": "output_finished",
                "operation_id": "op_boolean",
                "kind": "mesh",
                "part_role": "finished_shell",
            }
        ],
        "non_functional_only": True,
    }


def expect_code(program: dict, code: str, **kwargs: object) -> None:
    try:
        compile_shape_program(program, **kwargs)
    except (ManifoldCsgError, ShapeProgramValidationError, ValueError) as exc:
        assert code in str(exc), exc
        if isinstance(exc, ManifoldCsgError):
            assert exc.node_id.startswith("op_") and exc.code == code
        return
    raise AssertionError(f"expected {code}")


def nested_depth_program() -> dict:
    program = boolean_program("union", suffix="depth")
    operations = program["operations"][:2]
    previous = "op_base"
    for index in range(9):
        tool_id = f"op_depth_tool_{index}"
        node_id = f"op_depth_boolean_{index}"
        operations.append({
            "operation_id": tool_id,
            "op": "box",
            "inputs": [],
            "args": {
                "position": [700 + index * 600, 0, 0],
                "size": [500, 500, 500],
                "part_role": f"depth_tool_{index}",
                "zone_id": "zone_depth",
            },
        })
        operations.append({
            "operation_id": node_id,
            "op": "union",
            "inputs": [previous, tool_id],
            "args": {"part_role": "depth_result", "zone_id": "zone_depth"},
        })
        previous = node_id
    program["operations"] = operations
    program["outputs"][0]["operation_id"] = previous
    return program


def budget_program() -> dict:
    program = boolean_program("union", suffix="budget")
    program["triangle_budget"] = 100
    program["operations"].insert(1, {
        "operation_id": "op_array",
        "op": "array",
        "inputs": ["op_base"],
        "args": {"axis": [1, 0, 0], "count": 32, "spacing": 1100, "part_role": "array_shell"},
    })
    program["operations"][-1]["inputs"] = ["op_array", "op_tool"]
    return program


def open_input_program() -> dict:
    from smoke_g821_profile_solid_fidelity import program_for, revolve_profile

    program = program_for(
        revolve_profile(sketch_id="sketch_g825_open"),
        operation="extrude",
        suffix="g825_open",
        caps=False,
    )
    program["program_id"] = "shape_g825_non_manifold"
    program["operations"].extend([
        {
            "operation_id": "op_closed_tool",
            "op": "box",
            "inputs": [],
            "args": {"position": [0, 600, 0], "size": [300, 300, 300], "part_role": "closed_tool"},
        },
        {
            "operation_id": "op_boolean",
            "op": "union",
            "inputs": ["op_extrude", "op_closed_tool"],
            "args": {"part_role": "invalid_result"},
        },
    ])
    program["outputs"] = [{"output_id": "output_invalid", "operation_id": "op_boolean", "kind": "mesh", "part_role": "invalid_result"}]
    return program


def assert_compile_contract() -> None:
    fixtures = [
        boolean_program("union", suffix="housing_union", tool_position=[320, 0, 0]),
        boolean_program("subtract", suffix="window_subtract", tool_position=[120, 80, 0]),
        boolean_program("subtract", suffix="wheel_arch", tool_position=[350, -260, 0], tool_size=[500, 500, 700]),
        boolean_program("subtract", suffix="groove", tool_position=[0, 0, 230], tool_size=[700, 300, 100]),
        boolean_program("subtract", suffix="coplanar", tool_position=[290, 0, 0], tool_size=[420, 520, 620]),
    ]
    for program in fixtures:
        compiled = compile_shape_program(program)
        repeated = compile_shape_program(program)
        assert compiled.glb_bytes == repeated.glb_bytes
        assert compiled.readback.glb_sha256 == repeated.readback.glb_sha256
        assert compiled.readback.feature_history == repeated.readback.feature_history
        assert [item.node_id for item in compiled.readback.feature_history] == [
            item["operation_id"] for item in program["operations"]
        ]
        feature = compiled.readback.feature_history[-1]
        assert feature.kernel_id == "manifold3d" and feature.kernel_version == "3.5.2"
        assert feature.input_node_ids == ["op_base", "op_tool"]
        assert feature.input_hashes == [
            compiled.readback.feature_history[0].result_sha256,
            compiled.readback.feature_history[1].result_sha256,
        ]
        assert feature.result_closed and feature.result_triangle_count == compiled.readback.triangle_count
        assert feature.material_ids and feature.material_zone_ids
        assert feature.surface_provenance_sha256 != "0" * 64
        if program["operations"][-1]["op"] == "subtract":
            assert "boolean_cut" in feature.surface_roles
            assert any(item.boolean_backside is True for item in compiled.readback.surface_provenance)
            assert any(item.material_zone_id == "zone_trim" for item in compiled.readback.surface_provenance)

    expect_code(
        boolean_program(
            "subtract",
            suffix="near_degenerate",
            tool_position=[289.999999, 0, 0],
            tool_size=[420, 520, 620],
        ),
        "CSG_DEGENERATE_OUTPUT",
    )
    expect_code(open_input_program(), "CSG_NON_MANIFOLD_INPUT")
    expect_code(nested_depth_program(), "CSG_DEPTH_EXCEEDED")
    expect_code(budget_program(), "CSG_INPUT_BUDGET_EXCEEDED")

    cancelled = boolean_program("union", suffix="cancelled", base_size=[4000, 3000, 2000], tool_size=[3000, 2500, 2200])
    started = time.monotonic()
    expect_code(cancelled, "CSG_CANCELLED", cancel_check=lambda: time.monotonic() - started > 0.02)
    expect_code(cancelled, "CSG_TIMEOUT", csg_timeout_seconds=0.001)


def _seed_project(factory: SQLiteConnectionFactory) -> None:
    now = "2026-07-15T00:00:00+00:00"
    with factory.connect() as connection:
        connection.execute(
            """INSERT INTO domain_profiles(profile_id, domain_type, schema_version, pack_id, display_name, profile_json, profile_sha256, status, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)""",
            ("profile_g825", "weapon_concept", "DesignDomainProfile@1", "weapon-concept-v1-reference", "G825", "{}", "0" * 64, "active", now, now),
        )
        connection.execute(
            """INSERT INTO projects(project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at)
               VALUES (?, ?, ?, ?, 'active', NULL, ?, ?)""",
            ("prj_g825", "profile_g825", "weapon_concept", "G825 immutable feature", now, now),
        )


def assert_preview_confirm() -> None:
    with tempfile.TemporaryDirectory(prefix="forgecad-g825-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        _seed_project(factory)
        kernel = AgentKernelService(factory)
        assets = AgentAssetEditingService(factory)
        thread = kernel.create_thread(
            CreateAgentThreadRequest(client_request_id="g825-thread", project_id="prj_g825", title="G825"),
            "g825-thread",
        )
        turn = kernel.start_turn(
            thread.thread_id,
            StartAgentTurnRequest(client_request_id="g825-turn", message="设计一台三关节机械臂"),
            "g825-turn",
        )
        plan = MechanicalConceptPlan.model_validate(
            next(item.payload["result"] for item in turn.items if item.item_type == "tool_result")
        )
        direction_id = plan.directions[0].direction_id
        built = kernel.build_blockout(
            BuildAgentBlockoutRequest(client_request_id="g825-build", plan=plan, direction_id=direction_id),
            "g825-build",
        )
        segmented = kernel.segment_blockout(
            SegmentAgentBlockoutRequest(
                client_request_id="g825-segment",
                plan=plan,
                direction_id=direction_id,
                artifact_id=built.artifact_id,
            ),
            "g825-segment",
        )
        version = assets.commit_blockout(
            CommitAgentBlockoutRequest(client_request_id="g825-commit", artifact_id=segmented.artifact_id),
            "g825-commit",
        )
        target = next(item for item in version.parts if item.editable_parameter_bindings)
        tool = next(item for item in version.parts if item.part_id != target.part_id)
        program = boolean_program("subtract", suffix="versioned")
        program["operations"][0]["args"].update({
            "part_role": target.role,
            "position": list(target.position_mm),
            "size": list(target.size_mm),
        })
        program["operations"][1]["args"].update({
            "part_role": tool.role,
            "position": list(target.position_mm),
            "size": [max(10, target.size_mm[0] * 0.25), target.size_mm[1] * 1.2, target.size_mm[2] * 0.4],
        })
        program["operations"][2]["args"]["part_role"] = target.role
        program["outputs"][0]["part_role"] = target.role
        compile_shape_program(program)
        canonical = json.dumps(program, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        with factory.connect() as connection:
            connection.execute(
                "UPDATE agent_asset_versions SET shape_program_json = ? WHERE asset_version_id = ?",
                (canonical, version.asset_version_id),
            )
            connection.commit()
        base = assets.get_version(version.asset_version_id)
        base_program = copy.deepcopy(base.shape_program)
        before_versions = 1
        with SQLiteUnitOfWork(factory) as unit:
            assert len(unit.agent_assets.list_versions(base.project_id)) == before_versions
        binding = target.editable_parameter_bindings[0]
        next_value = min(binding.max, binding.default + binding.step)
        change = assets.propose_change_set(
            base.asset_version_id,
            ProposeAgentAssetChangeSetRequest(
                client_request_id="g825-propose",
                summary="G825 immutable CSG feature edit",
                operations=[
                    AgentPartEditOperation(
                        operation_id="op_g825_parameter",
                        op="set_part_parameter",
                        part_id=target.part_id,
                        path=binding.path,
                        value=next_value,
                    )
                ],
            ),
            "g825-propose",
        )
        preview = assets.preview_change_set(change.change_set_id, "g825-preview")
        assert preview.preview is not None
        with SQLiteUnitOfWork(factory) as unit:
            assert len(unit.agent_assets.list_versions(base.project_id)) == before_versions
            snapshot = unit.active_designs.get_snapshot(base.project_id)
            assert snapshot is not None and snapshot.preview is not None
        assert assets.get_version(base.asset_version_id).shape_program == base_program

        confirmed = assets.confirm_change_set(change.change_set_id, "g825-confirm")
        child = confirmed.asset_version
        assert child.parent_asset_version_id == base.asset_version_id
        assert child.asset_version_id != base.asset_version_id
        assert assets.get_version(base.asset_version_id).shape_program == base_program
        compiled = compile_shape_program(child.shape_program)
        assert compiled.readback.feature_history[-1].kernel_id == "manifold3d"
        with SQLiteUnitOfWork(factory) as unit:
            assert len(unit.agent_assets.list_versions(base.project_id)) == before_versions + 1
            snapshot = unit.active_designs.get_snapshot(base.project_id)
            assert snapshot is not None
            assert snapshot.active_design.asset_version_id == child.asset_version_id
            assert snapshot.preview is None
        report = assets.quality(child.asset_version_id)
        assert report.compile_readback is not None
        assert report.compile_readback.feature_history[-1].result_sha256 == compiled.readback.feature_history[-1].result_sha256
        exported = base64.b64decode(assets.export_glb(child.asset_version_id).glb_base64, validate=True)
        assert hashlib.sha256(exported).hexdigest() == report.compile_readback.glb_sha256


def main() -> int:
    assert_compile_contract()
    assert_preview_confirm()
    print(
        "G825 feature CSG smoke passed: one Manifold handler, immutable node/input hashes, "
        "provenance, cancellation and preview-confirm promotion"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
