import pytest

from forgecad_agent.application.shape_program import ShapeProgramValidationError, validate_shape_program


VALID_PROGRAM = {
    "schema_version": "ShapeProgram@1",
    "program_id": "shape_unit_test",
    "units": "millimeter",
    "seed": 7,
    "triangle_budget": 1000,
    "parameters": [{"parameter_id": "param_length", "default": 100, "min": 50, "max": 200}],
    "operations": [
        {
            "operation_id": "op_body",
            "op": "box",
            "inputs": [],
            "args": {"size": [100, 40, 20], "parameter_id": "param_length", "part_role": "body_shell"},
        },
        {
            "operation_id": "op_panel",
            "op": "surface_panel",
            "inputs": ["op_body"],
            "args": {"zone_id": "zone_body", "part_role": "body_shell"},
        },
    ],
    "outputs": [{"output_id": "output_body", "operation_id": "op_panel", "kind": "mesh", "part_role": "body_shell"}],
    "non_functional_only": True,
}


def test_valid_program_is_returned_as_a_copy():
    result = validate_shape_program(VALID_PROGRAM)
    assert result == VALID_PROGRAM
    assert result is not VALID_PROGRAM


@pytest.mark.parametrize(
    ("mutation", "error_code"),
    [
        ({"non_functional_only": False}, "SHAPE_PROGRAM_SCHEMA_INVALID"),
        ({"operations": [{**VALID_PROGRAM["operations"][0], "inputs": ["op_missing"]}]}, "SHAPE_PROGRAM_FORWARD_OR_MISSING_REFERENCE"),
        ({"operations": [{**VALID_PROGRAM["operations"][0], "args": {"parameter_id": "param_unknown"}}]}, "SHAPE_PROGRAM_UNKNOWN_PARAMETER"),
        ({"parameters": [{"parameter_id": "param_length", "default": 300, "min": 50, "max": 200}]}, "SHAPE_PROGRAM_PARAMETER_RANGE"),
    ],
)
def test_invalid_programs_are_rejected(mutation, error_code):
    candidate = {**VALID_PROGRAM, **mutation}
    with pytest.raises(ShapeProgramValidationError, match=error_code):
        validate_shape_program(candidate)


def test_non_finite_values_are_rejected():
    candidate = {**VALID_PROGRAM, "operations": [{**VALID_PROGRAM["operations"][0], "args": {"size": [1, 2, float("inf")]}}]}
    with pytest.raises(ShapeProgramValidationError, match="SHAPE_PROGRAM_NON_FINITE"):
        validate_shape_program(candidate)
