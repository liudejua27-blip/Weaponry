#!/usr/bin/env python3
"""Smoke the restricted bevel approximation and attached surface panel paths."""

from forgecad_agent.application.geometry_worker import (
    build_glb_from_shape_program,
    read_shape_program_glb,
    read_shape_program_glb_facts,
)
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


def assert_glb_readback(payload: bytes, bounds: list[float], triangles: int, *, bevel_segments: int) -> None:
    expected_bevel_triangles = 16 * (bevel_segments + 1)
    expected_triangles = expected_bevel_triangles + 12
    assert triangles == expected_triangles
    assert read_shape_program_glb(payload) == (triangles, bounds)
    facts = read_shape_program_glb_facts(payload)
    assert facts.triangle_count == expected_triangles
    assert facts.bounds_mm == bounds
    assert facts.mesh_count == 1
    assert facts.primitive_count == 2
    assert facts.uv0_primitive_count == facts.primitive_count
    assert facts.normal_primitive_count == facts.primitive_count
    assert facts.tangent_primitive_count == facts.primitive_count
    assert sum(item["face_count"] for item in facts.material_zone_faces) == triangles
    surfaces = {item["part_role"]: item for item in facts.surface_provenance}
    assert set(surfaces) == {"body_shell", "body_panel"}
    bevel = surfaces["body_shell"]
    panel = surfaces["body_panel"]
    for surface in surfaces.values():
        assert surface["closed"] is True
        assert surface["boundary_edge_count"] == 0
        assert surface["non_manifold_edge_count"] == 0
        assert surface["degenerate_triangle_count"] == 0
        assert surface["texture_ready"] is True
    assert bevel["surface_ranges"] == [
        {"surface_role": "surface", "first_triangle": 0, "triangle_count": expected_bevel_triangles}
    ]
    assert panel["surface_ranges"] == [
        {"surface_role": "trim", "first_triangle": 0, "triangle_count": 12}
    ]
    finished = bevel["edge_finish"]
    assert finished["edge_set"] == "xz_perimeter"
    assert finished["selected_edge_count"] == 4
    assert finished["subdivision_count"] == bevel_segments
    features = {item["node_id"]: item for item in facts.feature_history}
    assert features["op_bevel"]["result_triangle_count"] == expected_bevel_triangles
    assert features["op_bevel"]["result_closed"] is True
    assert features["op_panel"]["result_triangle_count"] == expected_triangles
    assert features["op_panel"]["result_closed"] is True


def main() -> int:
    low = program(bevel_segments=1, suffix="low")
    validate_shape_program(low)
    payload, bounds, triangles = build_glb_from_shape_program(low)
    assert bounds == [1000.0, 620.0, 700.0], bounds
    assert_glb_readback(payload, bounds, triangles, bevel_segments=1)
    assert build_glb_from_shape_program(low)[0] == payload

    smooth = program(bevel_segments=3, radius=60, panel_size=[400, 30, 260], offset=[100, -80], suffix="smooth")
    validate_shape_program(smooth)
    smooth_payload, smooth_bounds, smooth_triangles = build_glb_from_shape_program(smooth)
    assert smooth_triangles > triangles
    assert smooth_bounds == [1000.0, 630.0, 700.0], smooth_bounds
    assert_glb_readback(smooth_payload, smooth_bounds, smooth_triangles, bevel_segments=3)

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
