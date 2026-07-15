#!/usr/bin/env python3
"""Gate G820 profile consumption by Extrude/Revolve and real GLB facts."""

from __future__ import annotations

import copy
import math

from forgecad_agent.application.geometry_worker import (
    compile_shape_program,
    read_shape_program_glb_facts,
)
from forgecad_agent.application.profile_contracts import canonical_profile_payload
from forgecad_agent.application.shape_program import ShapeProgramValidationError, validate_shape_program
from forgecad_agent.application.shape_program_runtime import UnsupportedRuntimeOperationError


def shell_profile(*, sketch_id: str = "sketch_g821_shell", hole: bool = False) -> dict:
    holes = []
    if hole:
        holes.append(
            {
                "hole_id": "hole_g821_window",
                "winding": "clockwise",
                "start": [-0.22, -0.18],
                "segments": [
                    {"kind": "line", "to": [-0.22, 0.18]},
                    {"kind": "quadratic", "control": [0, 0.24], "to": [0.22, 0.18]},
                    {"kind": "line", "to": [0.22, -0.18]},
                    {"kind": "quadratic", "control": [0, -0.24], "to": [-0.22, -0.18]},
                ],
            }
        )
    return {
        "schema_version": "ProfileSketch@1",
        "sketch_id": sketch_id,
        "version": 1,
        "plane": "cross_section",
        "closed": True,
        "winding": "counter_clockwise",
        "start": [-0.8, -0.5],
        "segments": [
            {"kind": "line", "to": [0.5, -0.5]},
            {"kind": "quadratic", "control": [0.8, -0.5], "to": [0.8, -0.2]},
            {"kind": "cubic", "control_1": [0.8, 0.2], "control_2": [0.5, 0.5], "to": [0, 0.5]},
            {"kind": "line", "to": [-0.8, 0.5]},
            {"kind": "line", "to": [-0.8, -0.5]},
        ],
        "holes": holes,
        "normalized_bounds": {"min": [-0.8, -0.5], "max": [0.8, 0.5]},
        "symmetry": "horizontal",
        "continuity_hint": "tangent",
        "resample_count": 24,
        "provenance": {"source": "agent", "source_ref": "g821_shell_fixture"},
    }


def revolve_profile(*, sketch_id: str = "sketch_g821_revolve") -> dict:
    return {
        "schema_version": "ProfileSketch@1",
        "sketch_id": sketch_id,
        "version": 1,
        "plane": "side",
        "closed": False,
        "winding": "open",
        "start": [0, -0.8],
        "segments": [
            {"kind": "quadratic", "control": [0.55, -0.75], "to": [0.7, -0.25]},
            {"kind": "cubic", "control_1": [0.75, 0], "control_2": [0.55, 0.55], "to": [0.35, 0.7]},
            {"kind": "line", "to": [0, 0.8]},
        ],
        "holes": [],
        "normalized_bounds": {"min": [0, -0.8], "max": [0.75, 0.8]},
        "symmetry": "radial",
        "continuity_hint": "smooth",
        "resample_count": 20,
        "provenance": {"source": "component_recipe", "source_ref": "g821_joint_fixture"},
    }


def program_for(profile: dict, *, operation: str, suffix: str, angle: float | None = None, caps: bool = True) -> dict:
    canonical, _json, digest = canonical_profile_payload(profile)
    input_id = f"profileinput_{suffix}"
    solid_args = {
        "position": [0, 600, 0],
        "part_role": f"{suffix}_shell",
        "material_id": "mat_aluminum",
    }
    if operation == "extrude":
        solid_args.update({"height": 420, "cap_start": caps, "cap_end": caps})
    else:
        solid_args.update({
            "angle": 2 * math.pi if angle is None else angle,
            "radial_segments": 24,
            "cap_start": caps if angle is not None and angle < 2 * math.pi else False,
            "cap_end": caps if angle is not None and angle < 2 * math.pi else False,
        })
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": f"shape_g821_{suffix}",
        "units": "millimeter",
        "seed": 821,
        "triangle_budget": 100000,
        "parameters": [],
        "profile_inputs": [
            {
                "input_id": input_id,
                "input_kind": "profile_sketch",
                "contract_version": "ProfileSketch@1",
                "input_sha256": digest,
                "canonical_payload": canonical,
            }
        ],
        "operations": [
            {
                "operation_id": "op_profile",
                "op": "profile",
                "inputs": [],
                "args": {"profile_input_id": input_id, "profile_scale": [900, 650]},
            },
            {"operation_id": f"op_{operation}", "op": operation, "inputs": ["op_profile"], "args": solid_args},
        ],
        "outputs": [{"output_id": "output_shell", "operation_id": f"op_{operation}", "kind": "mesh", "part_role": f"{suffix}_shell"}],
        "non_functional_only": True,
    }


def assert_compile(program: dict, expected_roles: set[str]) -> tuple[bytes, int, list[float]]:
    validate_shape_program(program)
    compiled = compile_shape_program(program)
    facts = read_shape_program_glb_facts(compiled.glb_bytes)
    assert facts.triangle_count == compiled.readback.triangle_count > 0
    assert facts.bounds_mm == compiled.readback.bounds_mm
    assert facts.uv0_primitive_count == facts.normal_primitive_count == facts.primitive_count == 1
    assert set(facts.surface_provenance[0]["surface_roles"]) == expected_roles
    assert facts.surface_provenance[0]["profile_input_id"] == program["profile_inputs"][0]["input_id"]
    ranges = facts.surface_provenance[0]["surface_ranges"]
    assert {item["surface_role"] for item in ranges} == expected_roles
    assert sum(item["triangle_count"] for item in ranges) == facts.triangle_count
    assert facts.surface_provenance[0]["closed"] is True
    assert facts.surface_provenance[0]["boundary_edge_count"] == 0
    assert facts.surface_provenance[0]["non_manifold_edge_count"] == 0
    assert facts.surface_provenance[0]["degenerate_triangle_count"] == 0
    assert all(0 <= value <= 1 for value in [*facts.surface_provenance[0]["uv0_min"], *facts.surface_provenance[0]["uv0_max"]])
    repeated = compile_shape_program(program)
    assert repeated.glb_bytes == compiled.glb_bytes
    assert repeated.readback.model_dump() == compiled.readback.model_dump()
    return compiled.glb_bytes, facts.triangle_count, facts.bounds_mm


def expect_rejected(program: dict, code: str) -> None:
    try:
        compile_shape_program(program)
    except (ShapeProgramValidationError, UnsupportedRuntimeOperationError, ValueError) as exc:
        assert code in str(exc), exc
        return
    raise AssertionError(f"expected rejection containing {code}")


def main() -> int:
    plain = program_for(shell_profile(), operation="extrude", suffix="plain")
    _plain_glb, plain_triangles, plain_bounds = assert_compile(plain, {"side", "start_cap", "end_cap"})
    assert all(value > 0 for value in plain_bounds)

    holed = program_for(shell_profile(sketch_id="sketch_g821_holed", hole=True), operation="extrude", suffix="holed")
    _hole_glb, hole_triangles, hole_bounds = assert_compile(holed, {"side", "hole_wall", "start_cap", "end_cap"})
    assert hole_triangles > plain_triangles and hole_bounds == plain_bounds

    full = program_for(revolve_profile(), operation="revolve", suffix="full", caps=False)
    _full_glb, full_triangles, full_bounds = assert_compile(full, {"side", "seam"})
    assert all(value > 0 for value in full_bounds)

    partial = program_for(revolve_profile(sketch_id="sketch_g821_partial"), operation="revolve", suffix="partial", angle=math.pi, caps=True)
    _partial_glb, partial_triangles, partial_bounds = assert_compile(partial, {"side", "seam", "start_cap", "end_cap"})
    assert partial_triangles > full_triangles and partial_bounds[2] < full_bounds[2]

    open_extrude = program_for(revolve_profile(sketch_id="sketch_g821_open_extrude"), operation="extrude", suffix="open_extrude", caps=True)
    expect_rejected(open_extrude, "OPEN_EXTRUDE_CAP_FORBIDDEN")
    open_ribbon = program_for(revolve_profile(sketch_id="sketch_g821_open_ribbon"), operation="extrude", suffix="open_ribbon", caps=False)
    open_compiled = compile_shape_program(open_ribbon)
    open_surface = open_compiled.readback.surface_provenance[0]
    assert open_surface.surface_roles == ["side"]
    assert open_surface.closed is False and open_surface.boundary_edge_count > 0

    holed_revolve = program_for(shell_profile(sketch_id="sketch_g821_hole_revolve", hole=True), operation="revolve", suffix="hole_revolve", caps=False)
    expect_rejected(holed_revolve, "REVOLVE_HOLES_FORBIDDEN")

    negative = revolve_profile(sketch_id="sketch_g821_negative")
    negative["start"] = [-0.1, -0.8]
    negative["normalized_bounds"]["min"][0] = -0.1
    negative_program = program_for(negative, operation="revolve", suffix="negative", caps=False)
    expect_rejected(negative_program, "REVOLVE_RADIUS")

    damaged_hash = copy.deepcopy(plain)
    damaged_hash["profile_inputs"][0]["input_sha256"] = "0" * 64
    expect_rejected(damaged_hash, "PROFILE_INPUT_HASH_MISMATCH")

    over_budget = copy.deepcopy(holed)
    over_budget["triangle_budget"] = 100
    expect_rejected(over_budget, "triangle count")

    print("G821 profile solid fidelity smoke passed: curves, holes, caps, seams, UV0, provenance and deterministic GLB readback")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
