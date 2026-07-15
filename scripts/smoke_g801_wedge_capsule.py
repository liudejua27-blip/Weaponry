#!/usr/bin/env python3
"""Smoke the first two post-G3 ShapeProgram primitives: wedge and capsule."""

from forgecad_agent.application.geometry_worker import build_glb_from_shape_program, read_shape_program_glb


def program(primitive: dict, suffix: str) -> dict:
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": f"shape_g801_{suffix}",
        "units": "millimeter",
        "seed": 801,
        "triangle_budget": 100000,
        "parameters": [],
        "operations": [{"operation_id": f"op_{suffix}", "op": primitive["op"], "inputs": [], "args": {**primitive["args"], "part_role": f"{suffix}_part"}}],
        "outputs": [{"output_id": f"output_{suffix}", "operation_id": f"op_{suffix}", "kind": "mesh", "part_role": f"{suffix}_part"}],
        "non_functional_only": True,
    }


def main() -> int:
    fixtures = [
        ("wedge", {"op": "wedge", "args": {"position": [0, 400, 0], "size": [800, 500, 600]}}),
        ("capsule", {"op": "capsule", "args": {"position": [0, 600, 0], "radius": 180, "height": 900, "axis": [0, 1, 0]}}),
    ]
    for suffix, primitive in fixtures:
        payload, bounds, triangles = build_glb_from_shape_program(program(primitive, suffix))
        assert payload[:4] == b"glTF"
        assert triangles > 0
        assert all(value > 0 for value in bounds), (suffix, bounds)
        repeat_payload, repeat_bounds, repeat_triangles = build_glb_from_shape_program(program(primitive, suffix))
        assert payload == repeat_payload
        assert bounds == repeat_bounds
        assert triangles == repeat_triangles
        readback_triangles, readback_bounds = read_shape_program_glb(payload)
        assert readback_triangles == triangles
        assert readback_bounds == bounds
    print("G801 ShapeProgram smoke passed: deterministic wedge/capsule GLB readback")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
