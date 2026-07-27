"""End-to-end evidence for ordered n-ary ShapeProgram booleans."""

from __future__ import annotations

import pytest

from forgecad_agent.application.geometry_worker import compile_shape_program


def _program(operation: str, sources: list[dict]) -> dict:
    boolean = {
        "operation_id": "op_boolean",
        "op": operation,
        "inputs": [source["operation_id"] for source in sources],
        "args": {
            "part_role": "boolean_result",
            "material_id": "mat_aluminum",
            "zone_id": "zone_boolean",
        },
    }
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": f"shape_nary_{operation}_test",
        "units": "millimeter",
        "seed": 23,
        "triangle_budget": 10_000,
        "parameters": [],
        "profile_inputs": [],
        "operations": [*sources, boolean],
        "outputs": [
            {
                "output_id": "output_boolean",
                "operation_id": "op_boolean",
                "kind": "mesh",
                "part_role": "boolean_result",
            }
        ],
        "non_functional_only": True,
    }


def _box(operation_id: str, position: list[float], size: list[float]) -> dict:
    return {
        "operation_id": operation_id,
        "op": "box",
        "inputs": [],
        "args": {
            "position": position,
            "size": size,
            "part_role": "boolean_source",
            "material_id": "mat_aluminum",
            "zone_id": f"zone_{operation_id.removeprefix('op_')}",
        },
    }


def test_three_input_union_compiles_every_operand() -> None:
    result = compile_shape_program(
        _program(
            "union",
            [
                _box("op_left", [0, 0, 0], [100, 100, 100]),
                _box("op_middle", [80, 0, 0], [100, 100, 100]),
                _box("op_right", [160, 0, 0], [100, 100, 100]),
            ],
        ),
        artifact_profile_id="production_concept",
    )

    assert result.readback.bounds_mm == pytest.approx([260.0, 100.0, 100.0], abs=0.02)
    assert result.readback.triangle_count > 0


def test_three_input_subtract_applies_every_cutter() -> None:
    result = compile_shape_program(
        _program(
            "subtract",
            [
                _box("op_base", [0, 0, 0], [300, 100, 100]),
                _box("op_inner_cutter", [-50, 0, 0], [40, 160, 160]),
                _box("op_edge_cutter", [140, 0, 0], [80, 160, 160]),
            ],
        ),
        artifact_profile_id="production_concept",
    )

    # Ignoring the third input leaves the X extent at 300 mm.  A real n-ary
    # subtract removes the positive edge and reduces the result to 250 mm.
    assert result.readback.bounds_mm == pytest.approx([250.0, 100.0, 100.0], abs=0.02)
    assert result.readback.triangle_count > 0
