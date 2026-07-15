#!/usr/bin/env python3
"""Compatibility smoke for the G805 fixtures on the selected G825 CSG kernel."""

from forgecad_agent.application.geometry_worker import (
    build_glb_from_shape_program,
    compile_shape_program,
    read_shape_program_glb,
)
from forgecad_agent.application.shape_program import ShapeProgramValidationError, validate_shape_program


def boolean_program(op: str, second_position: list[float], suffix: str) -> dict:
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": f"shape_g805_{suffix}",
        "units": "millimeter",
        "seed": 805,
        "triangle_budget": 100000,
        "parameters": [],
        "operations": [
            {"operation_id": "op_base", "op": "box", "inputs": [], "args": {"position": [0, 300, 0], "size": [1000, 600, 600], "part_role": "base"}},
            {"operation_id": "op_tool", "op": "box", "inputs": [], "args": {"position": second_position, "size": [200, 600, 600], "part_role": "tool"}},
            {"operation_id": "op_boolean", "op": op, "inputs": ["op_base", "op_tool"], "args": {"part_role": f"{suffix}_result"}},
        ],
        "outputs": [{"output_id": "output_result", "operation_id": "op_boolean", "kind": "mesh", "part_role": f"{suffix}_result"}],
        "non_functional_only": True,
    }


def main() -> int:
    union = boolean_program("union", [0, 300, 0], "union_overlap")
    validate_shape_program(union)
    compiled_union = compile_shape_program(union)
    union_feature = compiled_union.readback.feature_history[-1]
    assert union_feature.kernel_id == "manifold3d" and union_feature.kernel_version == "3.5.2"
    assert union_feature.result_closed and union_feature.result_triangle_count > 0

    disjoint = boolean_program("union", [700, 300, 0], "union_disjoint")
    validate_shape_program(disjoint)
    payload, bounds, triangles = build_glb_from_shape_program(disjoint)
    assert triangles == 24
    assert read_shape_program_glb(payload) == (triangles, bounds)

    subtract = boolean_program("subtract", [0, 300, 0], "subtract_slot")
    validate_shape_program(subtract)
    payload, bounds, triangles = build_glb_from_shape_program(subtract)
    assert triangles == 24
    assert read_shape_program_glb(payload) == (triangles, bounds)

    unsupported = boolean_program("subtract", [0, 300, 0], "subtract_partial")
    unsupported["operations"][1]["args"]["size"] = [200, 500, 600]
    validate_shape_program(unsupported)
    compiled_partial = compile_shape_program(unsupported)
    assert compiled_partial.readback.feature_history[-1].result_closed
    assert any("boolean_cut" in item.surface_roles for item in compiled_partial.readback.surface_provenance)

    invalid = boolean_program("union", [700, 300, 0], "missing_input")
    invalid["operations"][2]["inputs"] = ["op_base"]
    try:
        validate_shape_program(invalid)
    except ShapeProgramValidationError as error:
        assert "UNION_INPUT" in str(error)
    else:
        raise AssertionError("boolean arity must be rejected")
    print("G805 fixture migration passed: legacy boxes compile through the single Manifold CSG handler")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
