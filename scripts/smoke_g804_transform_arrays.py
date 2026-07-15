#!/usr/bin/env python3
"""Smoke declaration-order-safe mirror, linear array and radial array geometry."""

import math

from forgecad_agent.application.geometry_worker import build_glb_from_shape_program, read_shape_program_glb
from forgecad_agent.application.shape_program import ShapeProgramValidationError, validate_shape_program


def transform_program(op: str, args: dict, suffix: str) -> dict:
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": f"shape_g804_{suffix}",
        "units": "millimeter",
        "seed": 804,
        "triangle_budget": 100000,
        "parameters": [],
        "operations": [
            {"operation_id": "op_source", "op": "box", "inputs": [], "args": {"position": [120, 300, 0], "size": [120, 160, 200], "part_role": "source_part"}},
            {"operation_id": "op_transform", "op": op, "inputs": ["op_source"], "args": {**args, "part_role": f"{suffix}_result"}},
        ],
        "outputs": [{"output_id": "output_result", "operation_id": "op_transform", "kind": "mesh", "part_role": f"{suffix}_result"}],
        "non_functional_only": True,
    }


def main() -> int:
    fixtures = [
        ("mirror", {"axis": [1, 0, 0]}, 12),
        ("array", {"axis": [1, 0, 0], "count": 3, "spacing": 260}, 36),
        ("radial_array", {"axis": [0, 1, 0], "count": 4, "radius": 300, "angle": math.pi * 2}, 48),
    ]
    for op, args, expected_triangles in fixtures:
        program = transform_program(op, args, op)
        validate_shape_program(program)
        payload, bounds, triangles = build_glb_from_shape_program(program)
        assert triangles == expected_triangles, (op, triangles)
        assert read_shape_program_glb(payload) == (triangles, bounds)
        assert build_glb_from_shape_program(program)[0] == payload
    invalid = transform_program("array", {"axis": [0, 0, 0], "count": 3, "spacing": 10}, "bad_axis")
    try:
        validate_shape_program(invalid)
    except ShapeProgramValidationError as error:
        assert "ARRAY_AXIS" in str(error)
    else:
        raise AssertionError("zero array axis must be rejected")
    forward = transform_program("mirror", {"axis": [1, 0, 0]}, "forward")
    forward["operations"][1]["inputs"] = ["op_missing"]
    try:
        validate_shape_program(forward)
    except ShapeProgramValidationError as error:
        assert "FORWARD_OR_MISSING" in str(error) or "MIRROR_INPUT" in str(error)
    else:
        raise AssertionError("missing transform reference must be rejected")
    print("G804 ShapeProgram smoke passed: mirror/array/radial_array order, budget and deterministic readback")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
