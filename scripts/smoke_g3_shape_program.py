#!/usr/bin/env python3
"""Smoke the ShapeProgram contract and non-execution safety boundary."""

from __future__ import annotations

from forgecad_agent.application.shape_program import ShapeProgramValidationError, validate_shape_program


VALID = {
    "schema_version": "ShapeProgram@1",
    "program_id": "shape_vehicle_blockout_demo",
    "units": "millimeter",
    "seed": 7,
    "triangle_budget": 24000,
    "parameters": [{"parameter_id": "param_body_length", "default": 2400, "min": 1200, "max": 3600}],
    "operations": [
        {"operation_id": "op_body", "op": "box", "inputs": [], "args": {"size": [2400, 1500, 600], "part_role": "body_shell"}},
        {"operation_id": "op_panel", "op": "surface_panel", "inputs": ["op_body"], "args": {"zone_id": "zone_body_shell", "part_role": "body_shell"}},
        {"operation_id": "op_panel_mirror", "op": "mirror", "inputs": ["op_panel"], "args": {"axis": [1, 0, 0], "part_role": "body_shell"}},
    ],
    "outputs": [{"output_id": "output_body", "operation_id": "op_panel_mirror", "kind": "mesh", "part_role": "body_shell"}],
    "non_functional_only": True,
}


def expect_invalid(value: dict, code: str) -> None:
    try:
        validate_shape_program(value)
    except ShapeProgramValidationError as exc:
        assert code in str(exc), (code, exc)
        return
    raise AssertionError(f"expected {code}")


def main() -> int:
    result = validate_shape_program(VALID)
    assert result["program_id"] == "shape_vehicle_blockout_demo"
    expect_invalid({**VALID, "non_functional_only": False}, "SHAPE_PROGRAM_SCHEMA_INVALID")
    expect_invalid({**VALID, "operations": [{**VALID["operations"][0], "inputs": ["op_missing"]}]}, "SHAPE_PROGRAM_FORWARD_OR_MISSING_REFERENCE")
    expect_invalid({**VALID, "operations": [{**VALID["operations"][0], "args": {"parameter_id": "param_unknown"}}]}, "SHAPE_PROGRAM_UNKNOWN_PARAMETER")
    expect_invalid({**VALID, "operations": [{**VALID["operations"][0], "args": {"size": [1, 2, float("inf")]}}]}, "SHAPE_PROGRAM_NON_FINITE")
    unsafe = {**VALID, "brief": "run python"}
    expect_invalid(unsafe, "SHAPE_PROGRAM_SCHEMA_INVALID")
    print("G3 ShapeProgram smoke passed: schema, ordered references, parameters, finite values, non-execution boundary")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
