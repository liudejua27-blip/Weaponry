#!/usr/bin/env python3
"""Smoke the restricted bevel approximation and attached surface panel paths."""

from forgecad_agent.application.geometry_worker import build_glb_from_shape_program, read_shape_program_glb
from forgecad_agent.application.shape_program import ShapeProgramValidationError, validate_shape_program


def program(*, bevel_segments: int = 1, radius: float = 40, panel_size: list[float] | None = None, offset: list[float] | None = None, suffix: str = "valid") -> dict:
    panel_args: dict = {"part_role": "body_panel", "axis": [0, 1, 0]}
    if panel_size is not None:
        panel_args["size"] = panel_size
    if offset is not None:
        panel_args["position"] = [offset[0], 0, offset[1]]
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": f"shape_g806_{suffix}",
        "units": "millimeter",
        "seed": 806,
        "triangle_budget": 100000,
        "parameters": [],
        "operations": [
            {"operation_id": "op_body", "op": "box", "inputs": [], "args": {"position": [0, 300, 0], "size": [1000, 600, 700], "part_role": "body_shell"}},
            {"operation_id": "op_bevel", "op": "bevel_approx", "inputs": ["op_body"], "args": {"radius": radius, "segments": bevel_segments, "part_role": "body_shell"}},
            {"operation_id": "op_panel", "op": "surface_panel", "inputs": ["op_bevel"], "args": panel_args},
        ],
        "outputs": [{"output_id": "output_body", "operation_id": "op_panel", "kind": "mesh", "part_role": "body_shell"}],
        "non_functional_only": True,
    }


def main() -> int:
    low = program(bevel_segments=1, suffix="low")
    validate_shape_program(low)
    payload, bounds, triangles = build_glb_from_shape_program(low)
    assert triangles == 28 + 12, triangles
    assert bounds == [1000.0, 620.0, 700.0], bounds
    assert read_shape_program_glb(payload) == (triangles, bounds)
    assert build_glb_from_shape_program(low)[0] == payload

    smooth = program(bevel_segments=3, radius=60, panel_size=[400, 30, 260], offset=[100, -80], suffix="smooth")
    validate_shape_program(smooth)
    smooth_payload, smooth_bounds, smooth_triangles = build_glb_from_shape_program(smooth)
    assert smooth_triangles == 60 + 12, smooth_triangles
    assert smooth_bounds == [1000.0, 630.0, 700.0], smooth_bounds
    assert read_shape_program_glb(smooth_payload) == (smooth_triangles, smooth_bounds)

    too_large = program(radius=500, suffix="too_large")
    validate_shape_program(too_large)
    try:
        build_glb_from_shape_program(too_large)
    except ValueError as error:
        assert "half-size" in str(error)
    else:
        raise AssertionError("bevel radius larger than the source face must fail")

    bad_axis = program(suffix="bad_axis")
    bad_axis["operations"][2]["args"]["axis"] = [1, 0, 0]
    try:
        validate_shape_program(bad_axis)
    except ShapeProgramValidationError as error:
        assert "SURFACE_PANEL_AXIS" in str(error)
    else:
        raise AssertionError("surface panel must reject unsupported face axes")

    bad_fit = program(panel_size=[1200, 20, 200], suffix="bad_fit")
    validate_shape_program(bad_fit)
    try:
        build_glb_from_shape_program(bad_fit)
    except ValueError as error:
        assert "fit within" in str(error)
    else:
        raise AssertionError("surface panel outside the source face must fail")

    bad_reference = program(suffix="bad_reference")
    bad_reference["operations"][1]["inputs"] = ["op_missing"]
    try:
        validate_shape_program(bad_reference)
    except ShapeProgramValidationError as error:
        assert "FORWARD_OR_MISSING" in str(error) or "BEVEL_INPUT" in str(error)
    else:
        raise AssertionError("bevel input reference must be validated")

    print("G806 ShapeProgram smoke passed: bevel approximation, surface panel fit and deterministic readback")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
