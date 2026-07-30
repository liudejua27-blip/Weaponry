"""U004 local deformable representation: bounded 2x2x2 lattice evidence."""

from __future__ import annotations

import copy

import pytest

from forgecad_agent.application.geometry_worker import compile_shape_program
from forgecad_agent.application.shape_program import ShapeProgramValidationError, validate_shape_program


def _program(offsets: list[list[float]]) -> dict:
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": "shape_u004_lattice_shell",
        "units": "millimeter",
        "seed": 41,
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
                "operation_id": "op_shell_lattice",
                "op": "lattice_deform",
                "inputs": ["op_shell"],
                "args": {"corner_offsets": offsets},
            },
        ],
        "outputs": [
            {
                "output_id": "output_shell",
                "operation_id": "op_shell_lattice",
                "kind": "mesh",
                "part_role": "armor_shell",
            }
        ],
        "non_functional_only": True,
    }


DEFORMED_OFFSETS = [
    [0.0, 0.0, 0.0], [0.08, 0.0, 0.0],
    [0.0, 0.04, 0.0], [0.08, 0.04, 0.0],
    [0.0, 0.0, -0.10], [0.08, 0.0, -0.10],
    [0.0, 0.04, -0.10], [0.08, 0.04, -0.10],
]


def test_lattice_deform_is_deterministic_topology_preserving_and_readback_visible() -> None:
    program = _program(DEFORMED_OFFSETS)
    first = compile_shape_program(program, artifact_profile_id="production_concept")
    second = compile_shape_program(copy.deepcopy(program), artifact_profile_id="production_concept")

    assert first.readback.triangle_count == second.readback.triangle_count == 12
    assert first.glb_bytes == second.glb_bytes
    feature = first.readback.feature_history[-1]
    assert feature.node_id == "op_shell_lattice"
    assert feature.operation == "lattice_deform"
    assert feature.result_triangle_count == 12
    assert first.readback.bounds_mm[2] == pytest.approx(72.0, abs=0.02)


@pytest.mark.parametrize(
    "mutate, code",
    [
        (lambda program: program["operations"][1]["args"].update({"corner_offsets": [[0.0, 0.0, 0.0]] * 8}), "SHAPE_PROGRAM_LATTICE_NO_EFFECT"),
        (lambda program: program["operations"][1]["args"].update({"corner_offsets": [[0.3, 0.0, 0.0]] * 8}), "SHAPE_PROGRAM_SCHEMA_INVALID"),
        (lambda program: program["operations"][1].update({"inputs": ["op_missing"]}), "SHAPE_PROGRAM_FORWARD_OR_MISSING_REFERENCE"),
    ],
)
def test_lattice_deform_fails_closed_for_invalid_cage_or_lineage(mutate, code: str) -> None:
    program = _program(DEFORMED_OFFSETS)
    mutate(program)
    with pytest.raises(ShapeProgramValidationError, match=code):
        validate_shape_program(program)
