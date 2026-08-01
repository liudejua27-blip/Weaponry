"""U004 generic hard-surface: bounded closed box shell evidence."""

from __future__ import annotations

import copy

import pytest

from forgecad_agent.application.geometry_worker import compile_shape_program
from forgecad_agent.application.shape_program import ShapeProgramValidationError, validate_shape_program


def _program(thickness: float = 20.0) -> dict:
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": "shape_u004_closed_shell",
        "units": "millimeter",
        "seed": 43,
        "triangle_budget": 1000,
        "parameters": [],
        "profile_inputs": [],
        "operations": [
            {
                "operation_id": "op_shell_base",
                "op": "box",
                "inputs": [],
                "args": {
                    "size": [240.0, 120.0, 80.0],
                    "part_role": "armor_shell",
                    "material_id": "mat_aluminum",
                    "zone_id": "zone_shell",
                },
            },
            {
                "operation_id": "op_shell",
                "op": "shell",
                "inputs": ["op_shell_base"],
                "args": {
                    "thickness": thickness,
                    "part_role": "armor_shell",
                    "material_id": "mat_aluminum",
                    "zone_id": "zone_shell",
                },
            },
        ],
        "outputs": [
            {
                "output_id": "output_shell",
                "operation_id": "op_shell",
                "kind": "mesh",
                "part_role": "armor_shell",
            }
        ],
        "non_functional_only": True,
    }


def test_closed_shell_is_deterministic_hollow_geometry_with_readback() -> None:
    first = compile_shape_program(_program(), artifact_profile_id="production_concept")
    second = compile_shape_program(copy.deepcopy(_program()), artifact_profile_id="production_concept")

    assert first.glb_bytes == second.glb_bytes
    assert first.readback.triangle_count == second.readback.triangle_count
    assert first.readback.triangle_count > 12
    assert first.readback.bounds_mm == pytest.approx([240.0, 120.0, 80.0], abs=0.02)
    feature = first.readback.feature_history[-1]
    assert feature.node_id == "op_shell"
    assert feature.operation == "shell"
    assert feature.result_triangle_count == first.readback.triangle_count


def test_beveled_source_compiles_to_a_closed_shell_with_real_readback() -> None:
    program = _program()
    program["program_id"] = "shape_u004_beveled_shell"
    program["operations"][1]["inputs"] = ["op_bevel"]
    program["operations"].insert(
        1,
        {
            "operation_id": "op_bevel",
            "op": "bevel_approx",
            "inputs": ["op_shell_base"],
            "args": {
                "radius": 8.0,
                "segments": 2,
                "part_role": "armor_shell",
                "material_id": "mat_aluminum",
                "zone_id": "zone_shell",
            },
        },
    )

    result = compile_shape_program(program, artifact_profile_id="production_concept")
    assert result.readback.triangle_count > 12
    assert result.readback.bounds_mm == pytest.approx([240.0, 120.0, 80.0], abs=0.02)
    assert result.readback.feature_history[-1].operation == "shell"


def test_face_groove_lowering_compiles_to_real_recessed_glb_readback() -> None:
    program = {
        "schema_version": "ShapeProgram@1",
        "program_id": "shape_u004_face_groove",
        "units": "millimeter",
        "seed": 19,
        "triangle_budget": 2000,
        "parameters": [],
        "profile_inputs": [],
        "operations": [
            {
                "operation_id": "op_shell",
                "op": "box",
                "inputs": [],
                "args": {
                    "size": [200.0, 100.0, 80.0],
                    "position": [10.0, 20.0, 30.0],
                    "part_role": "armor_shell",
                    "material_id": "mat_aluminum",
                    "zone_id": "zone_shell",
                },
            },
            {
                "operation_id": "op_groove",
                "op": "groove",
                "inputs": ["op_shell"],
                "args": {
                    "face_size": [120.0, 30.0],
                    "position": [8.0, 0.0, -6.0],
                    "axis": [0, 1, 0],
                    "depth": 8.0,
                    "part_role": "armor_shell",
                    "material_id": "mat_aluminum",
                    "zone_id": "zone_shell",
                },
            },
        ],
        "outputs": [
            {
                "output_id": "output_shell",
                "operation_id": "op_groove",
                "kind": "mesh",
                "part_role": "armor_shell",
            }
        ],
        "non_functional_only": True,
    }

    result = compile_shape_program(program, artifact_profile_id="production_concept")
    assert result.readback.triangle_count > 12
    assert result.readback.bounds_mm == pytest.approx([200.0, 100.0, 80.0], abs=0.02)
    assert result.readback.feature_history[-1].node_id == "op_groove"
    assert result.readback.feature_history[-1].operation == "groove"


def test_face_groove_fails_closed_for_wrong_face_or_depth() -> None:
    program = {
        "schema_version": "ShapeProgram@1",
        "program_id": "shape_u004_invalid_face_groove",
        "units": "millimeter",
        "seed": 20,
        "triangle_budget": 2000,
        "parameters": [],
        "profile_inputs": [],
        "operations": [
            {
                "operation_id": "op_shell",
                "op": "box",
                "inputs": [],
                "args": {"size": [200.0, 100.0, 80.0], "part_role": "armor_shell"},
            },
            {
                "operation_id": "op_groove",
                "op": "groove",
                "inputs": ["op_shell"],
                "args": {
                    "face_size": [120.0, 30.0],
                    "position": [0.0, 1.0, 0.0],
                    "axis": [0, 1, 0],
                    "depth": 8.0,
                    "part_role": "armor_shell",
                },
            },
        ],
        "outputs": [{"output_id": "output_shell", "operation_id": "op_groove", "kind": "mesh", "part_role": "armor_shell"}],
        "non_functional_only": True,
    }
    with pytest.raises(ShapeProgramValidationError, match="SHAPE_PROGRAM_GROOVE_ARGUMENTS"):
        validate_shape_program(program)
    program["operations"][1]["args"]["position"] = [0.0, 0.0, 0.0]
    program["operations"][1]["args"]["depth"] = 26.0
    with pytest.raises(ShapeProgramValidationError, match="SHAPE_PROGRAM_GROOVE_ARGUMENTS"):
        validate_shape_program(program)


@pytest.mark.parametrize(
    "mutate, code",
    [
        (lambda program: program["operations"][1]["args"].update({"thickness": 0}), "SHAPE_PROGRAM_SCHEMA_INVALID"),
        (lambda program: program["operations"][1]["args"].update({"thickness": 21}), "SHAPE_PROGRAM_SHELL_ARGUMENTS"),
        (lambda program: program["operations"][1].update({"inputs": ["op_missing"]}), "SHAPE_PROGRAM_FORWARD_OR_MISSING_REFERENCE"),
        (lambda program: program["operations"][0].update({"op": "cylinder"}), "SHAPE_PROGRAM_SHELL_SOURCE"),
        (lambda program: (program["operations"].insert(1, {"operation_id": "op_bevel", "op": "bevel_approx", "inputs": ["op_shell_base"], "args": {"radius": 20.0, "segments": 2}}), program["operations"][2].update({"inputs": ["op_bevel"]})), "SHAPE_PROGRAM_SHELL_ARGUMENTS"),
    ],
)
def test_shell_fails_closed_for_unbounded_or_wrong_lineage(mutate, code: str) -> None:
    program = _program()
    mutate(program)
    with pytest.raises(ShapeProgramValidationError, match=code):
        validate_shape_program(program)
