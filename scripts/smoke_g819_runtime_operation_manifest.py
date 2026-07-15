#!/usr/bin/env python3
"""Gate the single ShapeProgram runtime manifest and fail-closed asset paths."""

from __future__ import annotations

import json
import tempfile
from pathlib import Path

from forgecad_agent.application import geometry_worker
from forgecad_agent.application.agent_asset_editing import AgentAssetEditingService, AgentAssetError
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
from forgecad_agent.application.geometry_worker import build_glb_from_shape_program
from forgecad_agent.application.mechanical_planner import MechanicalConceptPlan
from forgecad_agent.application.profile_contracts import canonical_profile_payload
from forgecad_agent.application.shape_program import validate_shape_program
from forgecad_agent.application.shape_program_runtime import (
    UnsupportedRuntimeOperationError,
    assert_schema_consumes_runtime_manifest,
    runtime_operation_names,
)
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner, SQLiteUnitOfWork


ROOT = Path(__file__).resolve().parents[1]


def _program(operations: list[dict], output: str, suffix: str) -> dict:
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": f"shape_g819_{suffix}",
        "units": "millimeter",
        "seed": 819,
        "triangle_budget": 100000,
        "parameters": [],
        "operations": operations,
        "outputs": [{"output_id": "output_g819", "operation_id": output, "kind": "mesh", "part_role": "g819_part"}],
        "non_functional_only": True,
    }


def _programs_covering_every_manifest_operation() -> dict[str, dict]:
    box = {"operation_id": "op_box", "op": "box", "inputs": [], "args": {"position": [0, 100, 0], "size": [200, 200, 200], "part_role": "g819_part"}}
    profile = {"operation_id": "op_profile", "op": "profile", "inputs": [], "args": {"points": [[-80, -50], [80, -50], [100, 50], [-80, 50]]}}
    loft_profile = {
        "schema_version": "ProfileSketch@1",
        "sketch_id": "sketch_g819_loft",
        "version": 1,
        "plane": "cross_section",
        "closed": True,
        "winding": "counter_clockwise",
        "start": [-0.8, -0.6],
        "segments": [
            {"kind": "line", "to": [0.8, -0.6]},
            {"kind": "line", "to": [0.8, 0.6]},
            {"kind": "line", "to": [-0.8, 0.6]},
            {"kind": "line", "to": [-0.8, -0.6]},
        ],
        "holes": [],
        "normalized_bounds": {"min": [-0.8, -0.6], "max": [0.8, 0.6]},
        "symmetry": "vertical",
        "continuity_hint": "linear",
        "resample_count": 16,
        "provenance": {"source": "agent", "source_ref": "g819_loft"},
    }
    loft_sections = {
        "schema_version": "ProfileSectionSet@1",
        "section_set_id": "sectionset_g819_loft",
        "version": 1,
        "main_axis": "x",
        "profiles": [loft_profile],
        "sections": [
            {"section_id": "section_g819_start", "position": -0.8, "profile_sketch_id": loft_profile["sketch_id"], "scale": 0.8, "twist_degrees": 0, "cap_policy": "start"},
            {"section_id": "section_g819_end", "position": 0.8, "profile_sketch_id": loft_profile["sketch_id"], "scale": 1, "twist_degrees": 4, "cap_policy": "end"},
        ],
        "resample_policy": {"mode": "uniform_count", "count": 16},
        "symmetry": "vertical",
        "provenance": {"source": "agent", "source_ref": "g819_loft"},
    }
    canonical, _payload, digest = canonical_profile_payload(loft_sections)
    loft_program = _program([{
        "operation_id": "op_loft",
        "op": "loft",
        "inputs": [],
        "args": {
            "section_set_input_id": "profileinput_g819_loft",
            "cross_section_scale": [160, 120],
            "axis_length": 300,
            "continuity": "linear",
            "position": [0, 100, 0],
            "part_role": "g819_part",
        },
    }], "op_loft", "loft")
    loft_program["profile_inputs"] = [{
        "input_id": "profileinput_g819_loft",
        "input_kind": "profile_section_set",
        "contract_version": "ProfileSectionSet@1",
        "input_sha256": digest,
        "canonical_payload": canonical,
    }]
    sweep_profile, _sweep_payload, sweep_digest = canonical_profile_payload(loft_profile)
    sweep_program = _program([{
        "operation_id": "op_sweep",
        "op": "sweep",
        "inputs": [],
        "args": {
            "profile_input_id": "profileinput_g819_sweep",
            "profile_scale": [40, 30],
            "path_points": [[0, 0, 0], [300, 0, 0]],
            "path_closed": False,
            "path_twist_degrees": 0,
            "cap_start": True,
            "cap_end": True,
            "position": [0, 100, 0],
            "part_role": "g819_part",
        },
    }], "op_sweep", "sweep")
    sweep_program["profile_inputs"] = [{
        "input_id": "profileinput_g819_sweep",
        "input_kind": "profile_sketch",
        "contract_version": "ProfileSketch@1",
        "input_sha256": sweep_digest,
        "canonical_payload": sweep_profile,
    }]
    return {
        "box": _program([box], "op_box", "box"),
        "cylinder": _program([{"operation_id": "op_cylinder", "op": "cylinder", "inputs": [], "args": {"position": [0, 100, 0], "radius": 80, "height": 200, "part_role": "g819_part"}}], "op_cylinder", "cylinder"),
        "capsule": _program([{"operation_id": "op_capsule", "op": "capsule", "inputs": [], "args": {"position": [0, 100, 0], "radius": 70, "height": 200, "part_role": "g819_part"}}], "op_capsule", "capsule"),
        "wedge": _program([{"operation_id": "op_wedge", "op": "wedge", "inputs": [], "args": {"position": [0, 100, 0], "size": [200, 200, 200], "part_role": "g819_part"}}], "op_wedge", "wedge"),
        "profile": _program([profile, {"operation_id": "op_extrude", "op": "extrude", "inputs": ["op_profile"], "args": {"position": [0, 100, 0], "height": 120, "part_role": "g819_part"}}], "op_extrude", "profile_extrude"),
        "extrude": _program([profile, {"operation_id": "op_extrude", "op": "extrude", "inputs": ["op_profile"], "args": {"position": [0, 100, 0], "height": 120, "part_role": "g819_part"}}], "op_extrude", "extrude"),
        "revolve": _program([{"operation_id": "op_profile", "op": "profile", "inputs": [], "args": {"points": [[0, -80], [80, -40], [80, 40], [0, 80]]}}, {"operation_id": "op_revolve", "op": "revolve", "inputs": ["op_profile"], "args": {"position": [0, 100, 0], "part_role": "g819_part"}}], "op_revolve", "revolve"),
        "loft": loft_program,
        "sweep": sweep_program,
        "mirror": _program([box, {"operation_id": "op_mirror", "op": "mirror", "inputs": ["op_box"], "args": {"axis": [1, 0, 0], "part_role": "g819_part"}}], "op_mirror", "mirror"),
        "array": _program([box, {"operation_id": "op_array", "op": "array", "inputs": ["op_box"], "args": {"axis": [1, 0, 0], "count": 2, "spacing": 300, "part_role": "g819_part"}}], "op_array", "array"),
        "radial_array": _program([box, {"operation_id": "op_radial", "op": "radial_array", "inputs": ["op_box"], "args": {"axis": [0, 1, 0], "count": 3, "radius": 300, "part_role": "g819_part"}}], "op_radial", "radial"),
        "union": _program([box, {"operation_id": "op_tool", "op": "box", "inputs": [], "args": {"position": [300, 100, 0], "size": [100, 200, 200], "part_role": "g819_part"}}, {"operation_id": "op_union", "op": "union", "inputs": ["op_box", "op_tool"], "args": {"part_role": "g819_part"}}], "op_union", "union"),
        "subtract": _program([box, {"operation_id": "op_tool", "op": "box", "inputs": [], "args": {"position": [0, 100, 0], "size": [100, 200, 200], "part_role": "g819_part"}}, {"operation_id": "op_subtract", "op": "subtract", "inputs": ["op_box", "op_tool"], "args": {"part_role": "g819_part"}}], "op_subtract", "subtract"),
        "bevel_approx": _program([box, {"operation_id": "op_bevel", "op": "bevel_approx", "inputs": ["op_box"], "args": {"radius": 20, "segments": 1, "part_role": "g819_part"}}], "op_bevel", "bevel"),
        "surface_panel": _program([box, {"operation_id": "op_panel", "op": "surface_panel", "inputs": ["op_box"], "args": {"part_role": "g819_part"}}], "op_panel", "surface_panel"),
    }


def _seed_project(factory: SQLiteConnectionFactory) -> None:
    now = "2026-07-15T00:00:00+00:00"
    with factory.connect() as connection:
        connection.execute(
            """INSERT INTO domain_profiles(profile_id, domain_type, schema_version, pack_id, display_name, profile_json, profile_sha256, status, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)""",
            ("profile_weapon_concept_v1", "weapon_concept", "DesignDomainProfile@1", "weapon-concept-v1-reference", "G819", "{}", "0" * 64, "active", now, now),
        )
        connection.execute(
            """INSERT INTO projects(project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)""",
            ("prj_g819_smoke", "profile_weapon_concept_v1", "weapon_concept", "G819 smoke", "active", None, now, now),
        )


def _make_asset(factory: SQLiteConnectionFactory) -> tuple[AgentAssetEditingService, object]:
    kernel = AgentKernelService(factory)
    assets = AgentAssetEditingService(factory)
    thread = kernel.create_thread(CreateAgentThreadRequest(client_request_id="g819-thread", project_id="prj_g819_smoke", title="G819"), "g819-thread")
    turn = kernel.start_turn(thread.thread_id, StartAgentTurnRequest(client_request_id="g819-turn", message="设计一台展示型机械臂"), "g819-turn")
    plan = MechanicalConceptPlan.model_validate(
        next(item.payload["result"] for item in turn.items if item.item_type == "tool_result" and "result" in item.payload)
    )
    direction_id = plan.directions[0].direction_id
    built = kernel.build_blockout(BuildAgentBlockoutRequest(client_request_id="g819-build", plan=plan, direction_id=direction_id), "g819-build")
    segmented = kernel.segment_blockout(SegmentAgentBlockoutRequest(client_request_id="g819-segment", plan=plan, direction_id=direction_id, artifact_id=built.artifact_id), "g819-segment")
    version = assets.commit_blockout(CommitAgentBlockoutRequest(client_request_id="g819-commit", artifact_id=segmented.artifact_id), "g819-commit")
    return assets, version


def _propose(assets: AgentAssetEditingService, version: object, suffix: str):
    part = next(item for item in version.parts if item.editable_parameter_bindings)
    return assets.propose_change_set(
        version.asset_version_id,
        ProposeAgentAssetChangeSetRequest(
            client_request_id=f"g819-propose-{suffix}",
            summary="G819 运行时拒绝边界",
            operations=[AgentPartEditOperation(operation_id=f"op_g819_{suffix}", op="set_part_parameter", part_id=part.part_id, path="transform.scale.x", value=1.0)],
        ),
        f"g819-propose-{suffix}",
    )


def _assert_rejected(action, *, expected_code: str = "UNSUPPORTED_RUNTIME_OPERATION") -> None:
    try:
        action()
    except AgentAssetError as exc:
        assert exc.code == expected_code, exc
        return
    raise AssertionError(f"expected {expected_code}")


def main() -> int:
    assert_schema_consumes_runtime_manifest()
    fixtures = _programs_covering_every_manifest_operation()
    assert tuple(fixtures) == runtime_operation_names(), (tuple(fixtures), runtime_operation_names())
    for operation, program in fixtures.items():
        validate_shape_program(program)
        payload, _, triangles = build_glb_from_shape_program(program)
        assert payload[:4] == b"glTF" and triangles > 0, operation

    unsupported = fixtures["box"].copy()
    unsupported["operations"] = [{**unsupported["operations"][0], "op": "pivot"}]
    try:
        validate_shape_program(unsupported)
    except UnsupportedRuntimeOperationError as exc:
        assert exc.code == "UNSUPPORTED_RUNTIME_OPERATION"
    else:
        raise AssertionError("unknown Schema operation must fail closed")

    with tempfile.TemporaryDirectory(prefix="forgecad-g819-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        _seed_project(factory)
        assets, version = _make_asset(factory)

        # A declared executor disappearing after startup must stop preview,
        # confirm, quality and export before their own writes.
        proposed = _propose(assets, version, "missing")
        original = geometry_worker._WORKER_EXECUTOR_IDS
        geometry_worker._WORKER_EXECUTOR_IDS = frozenset(item for item in original if item != "primitive_box")
        try:
            _assert_rejected(lambda: assets.preview_change_set(proposed.change_set_id, "g819-preview-missing"))
            _assert_rejected(lambda: assets.quality(version.asset_version_id))
            _assert_rejected(lambda: assets.export_glb(version.asset_version_id))
            with SQLiteUnitOfWork(factory) as unit:
                row = unit.agent_assets.get_change_set(proposed.change_set_id)
                snapshot = unit.active_designs.get_snapshot("prj_g819_smoke")
                assert row["status"] == "proposed" and row["preview_json"] is None
                assert snapshot is not None and snapshot.preview is None and snapshot.quality is None

            previewable = _propose(assets, version, "confirm")
        finally:
            geometry_worker._WORKER_EXECUTOR_IDS = original
        assets.preview_change_set(previewable.change_set_id, "g819-preview-confirm")
        geometry_worker._WORKER_EXECUTOR_IDS = frozenset(item for item in original if item != "primitive_box")
        try:
            _assert_rejected(lambda: assets.confirm_change_set(previewable.change_set_id, "g819-confirm-missing"))
            with SQLiteUnitOfWork(factory) as unit:
                row = unit.agent_assets.get_change_set(previewable.change_set_id)
                head = unit.agent_assets.get_head("prj_g819_smoke")
                assert row["status"] == "previewed" and head["asset_version_id"] == version.asset_version_id
        finally:
            geometry_worker._WORKER_EXECUTOR_IDS = original

        # Simulate a persisted pre-G819/externally-corrupted unsupported node.
        invalid_proposal = _propose(assets, version, "unknown")
        unknown_program = json.loads(json.dumps(version.shape_program))
        unknown_program["operations"][0]["op"] = "pivot"
        with factory.connect() as connection:
            connection.execute(
                "UPDATE agent_asset_versions SET shape_program_json = ? WHERE asset_version_id = ?",
                (json.dumps(unknown_program, ensure_ascii=False, sort_keys=True), version.asset_version_id),
            )
        _assert_rejected(lambda: assets.preview_change_set(invalid_proposal.change_set_id, "g819-preview-unknown"))
        _assert_rejected(lambda: assets.quality(version.asset_version_id))
        _assert_rejected(lambda: assets.export_glb(version.asset_version_id))
        with SQLiteUnitOfWork(factory) as unit:
            row = unit.agent_assets.get_change_set(invalid_proposal.change_set_id)
            report_count = unit.connection.execute("SELECT COUNT(*) FROM agent_asset_quality_reports").fetchone()[0]
            assert row["status"] == "proposed" and row["preview_json"] is None and report_count == 0

    print("G819 runtime manifest smoke passed: Schema/Pydantic/Worker/quality agree and all rejected paths are side-effect free")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
