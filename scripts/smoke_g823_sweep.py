#!/usr/bin/env python3
"""Gate restricted profile sweep, deterministic frames and GLB readback."""

from __future__ import annotations

import copy

from forgecad_agent.application.geometry_worker import compile_shape_program
from forgecad_agent.application.profile_contracts import canonical_profile_payload
from forgecad_agent.application.shape_program import ShapeProgramValidationError
from forgecad_agent.application.shape_program_runtime import UnsupportedRuntimeOperationError


def profile() -> dict:
    return {
        "schema_version": "ProfileSketch@1",
        "sketch_id": "sketch_g823_tube",
        "version": 1,
        "plane": "cross_section",
        "closed": True,
        "winding": "counter_clockwise",
        "start": [-0.55, -0.4],
        "segments": [
            {"kind": "line", "to": [0.55, -0.4]},
            {"kind": "quadratic", "control": [0.7, 0], "to": [0.55, 0.4]},
            {"kind": "line", "to": [-0.55, 0.4]},
            {"kind": "quadratic", "control": [-0.7, 0], "to": [-0.55, -0.4]},
        ],
        "holes": [],
        "normalized_bounds": {"min": [-0.7, -0.4], "max": [0.7, 0.4]},
        "symmetry": "horizontal",
        "continuity_hint": "tangent",
        "resample_count": 16,
        "provenance": {"source": "component_recipe", "source_ref": "g823_handle"},
    }


def program(suffix: str, path: list[list[float]], *, closed: bool = False, twist: float = 0, caps: bool = True) -> dict:
    canonical, _json, digest = canonical_profile_payload(profile())
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": f"shape_g823_{suffix}",
        "units": "millimeter",
        "seed": 823,
        "triangle_budget": 100000,
        "parameters": [],
        "profile_inputs": [{
            "input_id": "profileinput_g823_tube",
            "input_kind": "profile_sketch",
            "contract_version": "ProfileSketch@1",
            "input_sha256": digest,
            "canonical_payload": canonical,
        }],
        "operations": [{
            "operation_id": "op_sweep",
            "op": "sweep",
            "inputs": [],
            "args": {
                "profile_input_id": "profileinput_g823_tube",
                "profile_scale": [90, 70],
                "path_points": path,
                "path_closed": closed,
                "path_twist_degrees": twist,
                "cap_start": caps,
                "cap_end": caps,
                "position": [0, 600, 0],
                "part_role": f"{suffix}_frame",
                "material_id": "mat_aluminum",
            },
        }],
        "outputs": [{"output_id": "output_sweep", "operation_id": "op_sweep", "kind": "mesh", "part_role": f"{suffix}_frame"}],
        "non_functional_only": True,
    }


def expect_rejected(candidate: dict, code: str) -> None:
    try:
        compile_shape_program(candidate)
    except (ShapeProgramValidationError, UnsupportedRuntimeOperationError, ValueError) as exc:
        assert code in str(exc), exc
        return
    raise AssertionError(f"expected rejection containing {code}")


def assert_compiles(candidate: dict, roles: list[str], closed: bool) -> None:
    compiled = compile_shape_program(candidate)
    repeated = compile_shape_program(candidate)
    assert compiled.glb_bytes == repeated.glb_bytes
    assert compiled.readback.model_dump() == repeated.readback.model_dump()
    surface = compiled.readback.surface_provenance[0]
    assert surface.surface_roles == roles
    assert surface.closed is closed
    assert surface.non_manifold_edge_count == surface.degenerate_triangle_count == 0
    assert surface.boundary_edge_count == 0 if closed else surface.boundary_edge_count > 0
    assert all(0 <= value <= 1 for value in [*surface.uv0_min, *surface.uv0_max])


def main() -> int:
    assert_compiles(program("straight", [[0, 0, 0], [1000, 0, 0]], twist=15), ["sweep_side", "seam", "start_cap", "end_cap"], True)
    assert_compiles(program("polyline", [[0, 0, 0], [500, 0, 0], [800, 250, 0]], twist=-20), ["sweep_side", "seam", "start_cap", "end_cap"], True)
    assert_compiles(program("smooth", [[0, 0, 0], [300, 30, 0], [580, 130, 30], [800, 320, 80], [950, 560, 120]], twist=25), ["sweep_side", "seam", "start_cap", "end_cap"], True)
    assert_compiles(program("closed", [[-400, -300, 0], [400, -300, 0], [400, 300, 0], [-400, 300, 0]], closed=True, caps=False), ["sweep_side", "seam"], True)

    open_ribbon = program("open", [[0, 0, 0], [500, 0, 0]], caps=False)
    assert_compiles(open_ribbon, ["sweep_side", "seam"], False)
    zero = program("zero", [[0, 0, 0], [0, 0, 0]])
    expect_rejected(zero, "ZERO_LENGTH")
    flip = program("flip", [[0, 0, 0], [500, 0, 0], [10, 1, 0]])
    expect_rejected(flip, "FRAME_FLIP")
    tight = program("tight", [[0, 0, 0], [100, 0, 0], [100, 100, 0]])
    expect_rejected(tight, "CURVATURE_RADIUS")
    crossing = program("crossing", [[-400, -300, 0], [400, 300, 0], [-400, 300, 0], [400, -300, 0]])
    expect_rejected(crossing, "SELF_INTERSECTION")
    closed_cap = program("closed_cap", [[-400, -300, 0], [400, -300, 0], [400, 300, 0], [-400, 300, 0]], closed=True)
    expect_rejected(closed_cap, "CLOSED_CAP_FORBIDDEN")
    over_points = program("points", [[index * 100, 0, 0] for index in range(33)])
    expect_rejected(over_points, "SCHEMA_INVALID")
    over_bounds = program("bounds", [[0, 0, 0], [100001, 0, 0]])
    expect_rejected(over_bounds, "SCHEMA_INVALID")
    over_budget = copy.deepcopy(program("budget", [[0, 0, 0], [400, 0, 0], [750, 150, 0], [1000, 400, 0]]))
    over_budget["triangle_budget"] = 100
    expect_rejected(over_budget, "triangle count")
    print("G823 sweep smoke passed: deterministic frames, open/closed paths, twist, caps, topology and failure budgets")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
