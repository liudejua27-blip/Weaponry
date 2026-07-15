#!/usr/bin/env python3
"""Smoke the controlled profile -> revolve ShapeProgram path."""

import math

from forgecad_agent.application.geometry_worker import build_glb_from_shape_program, read_shape_program_glb
from forgecad_agent.application.shape_program import ShapeProgramValidationError, validate_shape_program


def revolve_program(points: list[list[float]], suffix: str, angle: float = math.pi * 2) -> dict:
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": f"shape_g803_{suffix}",
        "units": "millimeter",
        "seed": 803,
        "triangle_budget": 100000,
        "parameters": [],
        "operations": [
            {"operation_id": "op_profile", "op": "profile", "inputs": [], "args": {"points": points}},
            {"operation_id": "op_revolve", "op": "revolve", "inputs": ["op_profile"], "args": {"position": [0, 500, 0], "angle": angle, "part_role": f"{suffix}_body"}},
        ],
        "outputs": [{"output_id": "output_body", "operation_id": "op_revolve", "kind": "mesh", "part_role": f"{suffix}_body"}],
        "non_functional_only": True,
    }


def main() -> int:
    program = revolve_program([[0, -450], [180, -360], [220, 0], [180, 360], [0, 450]], "joint")
    validate_shape_program(program)
    payload, bounds, triangles = build_glb_from_shape_program(program)
    assert payload[:4] == b"glTF"
    assert triangles == 2 * 16 * 4
    assert all(value > 0 for value in bounds), bounds
    assert read_shape_program_glb(payload) == (triangles, bounds)
    assert build_glb_from_shape_program(program)[0] == payload

    partial = revolve_program([[0, -200], [140, -120], [140, 120], [0, 200]], "partial", math.pi)
    validate_shape_program(partial)
    _, _, partial_triangles = build_glb_from_shape_program(partial)
    assert partial_triangles == 2 * 15 * 3

    invalid = revolve_program([[-10, 0], [100, 0], [100, 100]], "negative_radius")
    try:
        validate_shape_program(invalid)
    except ShapeProgramValidationError as error:
        assert "REVOLVE_RADIUS" in str(error)
    else:
        raise AssertionError("negative revolve radius must be rejected")
    print("G803 ShapeProgram smoke passed: revolve topology, partial angle and deterministic GLB readback")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
