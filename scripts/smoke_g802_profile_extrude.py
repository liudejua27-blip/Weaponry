#!/usr/bin/env python3
"""Smoke the controlled profile -> extrude ShapeProgram path."""

from forgecad_agent.application.geometry_worker import build_glb_from_shape_program, read_shape_program_glb
from forgecad_agent.application.shape_program import ShapeProgramValidationError, validate_shape_program


def profile_program(points: list[list[float]], height: float, suffix: str) -> dict:
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": f"shape_g802_{suffix}",
        "units": "millimeter",
        "seed": 802,
        "triangle_budget": 100000,
        "parameters": [],
        "operations": [
            {"operation_id": "op_profile", "op": "profile", "inputs": [], "args": {"points": points}},
            {"operation_id": "op_extrude", "op": "extrude", "inputs": ["op_profile"], "args": {"position": [0, 600, 0], "height": height, "part_role": f"{suffix}_shell"}},
        ],
        "outputs": [{"output_id": "output_shell", "operation_id": "op_extrude", "kind": "mesh", "part_role": f"{suffix}_shell"}],
        "non_functional_only": True,
    }


def main() -> int:
    fixtures = [
        ("wing", [[-900, -260], [700, -260], [1000, 0], [700, 260], [-900, 260]], 220),
        ("panel", [[-500, -350], [500, -350], [500, 350], [-500, 350]], 480),
    ]
    for suffix, points, height in fixtures:
        program = profile_program(points, height, suffix)
        validate_shape_program(program)
        payload, bounds, triangles = build_glb_from_shape_program(program)
        assert payload[:4] == b"glTF"
        assert triangles == 4 * len(points) - 4
        assert all(value > 0 for value in bounds), (suffix, bounds)
        readback_triangles, readback_bounds = read_shape_program_glb(payload)
        assert (readback_triangles, readback_bounds) == (triangles, bounds)
        repeat = build_glb_from_shape_program(program)
        assert repeat[0] == payload
    invalid = profile_program([[0, 0], [100, 0], [200, 0]], 100, "degenerate")
    try:
        validate_shape_program(invalid)
    except ShapeProgramValidationError as error:
        assert "PROFILE_DEGENERATE" in str(error)
    else:
        raise AssertionError("degenerate profile must be rejected")
    print("G802 ShapeProgram smoke passed: profile/extrude validation and deterministic GLB readback")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
