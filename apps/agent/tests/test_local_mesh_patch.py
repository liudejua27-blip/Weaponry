"""U004 mesh-seed slice: bounded local patch over reviewed worker geometry."""

from __future__ import annotations

import copy

import pytest

from forgecad_agent.application.geometry_worker import compile_shape_program
from forgecad_agent.application.shape_program import ShapeProgramValidationError, validate_shape_program


def _program() -> dict:
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": "shape_u004_local_mesh_patch",
        "units": "millimeter",
        "seed": 43,
        "triangle_budget": 1000,
        "parameters": [],
        "profile_inputs": [],
        "operations": [
            {
                "operation_id": "op_shell",
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
                "operation_id": "op_shell_patch",
                "op": "local_mesh_patch",
                "inputs": ["op_shell"],
                "args": {
                    "patch_center": [0.0, 0.0, 0.0],
                    "patch_radius": 0.2,
                    "patch_offset": [0.1, 0.0, 0.0],
                },
            },
        ],
        "outputs": [
            {
                "output_id": "output_shell",
                "operation_id": "op_shell_patch",
                "kind": "mesh",
                "part_role": "armor_shell",
            }
        ],
        "non_functional_only": True,
    }


def test_local_mesh_patch_is_deterministic_topology_preserving_and_readback_visible() -> None:
    program = _program()
    first = compile_shape_program(program, artifact_profile_id="production_concept")
    second = compile_shape_program(copy.deepcopy(program), artifact_profile_id="production_concept")

    assert first.readback.triangle_count == second.readback.triangle_count == 12
    assert first.glb_bytes == second.glb_bytes
    feature = first.readback.feature_history[-1]
    assert feature.node_id == "op_shell_patch"
    assert feature.operation == "local_mesh_patch"
    assert feature.result_triangle_count == 12
    assert first.readback.bounds_mm[0] > -120.0


@pytest.mark.parametrize(
    "mutate, code",
    [
        (lambda program: program["operations"][1]["args"].update({"patch_radius": 0.01}), "SHAPE_PROGRAM_SCHEMA_INVALID"),
        (lambda program: program["operations"][1]["args"].update({"patch_offset": [0.0, 0.0, 0.0]}), "SHAPE_PROGRAM_LOCAL_PATCH_BOUNDS"),
        (lambda program: program["operations"][1].update({"inputs": ["op_missing"]}), "SHAPE_PROGRAM_FORWARD_OR_MISSING_REFERENCE"),
    ],
)
def test_local_mesh_patch_fails_closed_for_invalid_bounds_or_lineage(mutate, code: str) -> None:
    program = _program()
    mutate(program)
    with pytest.raises(ShapeProgramValidationError, match=code):
        validate_shape_program(program)
