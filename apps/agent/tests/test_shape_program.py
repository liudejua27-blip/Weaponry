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


@pytest.mark.parametrize("axis", [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]])
def test_surface_panel_accepts_each_axis_aligned_source_face(axis):
    candidate = {
        **VALID_PROGRAM,
        "operations": [
            {**VALID_PROGRAM["operations"][0], "args": {"size": [100, 80, 60], "part_role": "body_shell"}},
            {
                **VALID_PROGRAM["operations"][1],
                "args": {"size": [8, 30, 20], "axis": axis, "position": [0, 0, 0], "part_role": "body_shell"},
            },
        ],
    }

    validate_shape_program(candidate)


def test_surface_panel_rejects_diagonal_axis_and_normal_offset():
    diagonal = {
        **VALID_PROGRAM,
        "operations": [
            VALID_PROGRAM["operations"][0],
            {**VALID_PROGRAM["operations"][1], "args": {"axis": [1, 1, 0], "part_role": "body_shell"}},
        ],
    }
    with pytest.raises(ShapeProgramValidationError, match="SHAPE_PROGRAM_SURFACE_PANEL_AXIS"):
        validate_shape_program(diagonal)

    offset = {
        **VALID_PROGRAM,
        "operations": [
            VALID_PROGRAM["operations"][0],
            {**VALID_PROGRAM["operations"][1], "args": {"axis": [1, 0, 0], "position": [1, 0, 0], "part_role": "body_shell"}},
        ],
    }
    with pytest.raises(ShapeProgramValidationError, match="SHAPE_PROGRAM_SURFACE_PANEL_OFFSET"):
        validate_shape_program(offset)


@pytest.mark.parametrize("operation_name", ["union", "subtract"])
def test_boolean_operations_accept_two_to_eight_ordered_inputs(operation_name):
    sources = [
        {
            "operation_id": f"op_source_{index}",
            "op": "box",
            "inputs": [],
            "args": {"position": [index * 20, 0, 0], "size": [10, 10, 10], "part_role": "source"},
        }
        for index in range(3)
    ]
    boolean = {
        "operation_id": "op_boolean",
        "op": operation_name,
        "inputs": [source["operation_id"] for source in sources],
        "args": {"part_role": "boolean_result"},
    }
    candidate = {
        **VALID_PROGRAM,
        "operations": [*sources, boolean],
        "outputs": [{**VALID_PROGRAM["outputs"][0], "operation_id": "op_boolean"}],
    }

    assert validate_shape_program(candidate)["operations"][-1]["inputs"] == [
        "op_source_0",
        "op_source_1",
        "op_source_2",
    ]


@pytest.mark.parametrize("operation_name", ["union", "subtract"])
def test_boolean_operations_reject_fewer_than_two_inputs(operation_name):
    source = {**VALID_PROGRAM["operations"][0], "operation_id": "op_source"}
    boolean = {
        "operation_id": "op_boolean",
        "op": operation_name,
        "inputs": ["op_source"],
        "args": {"part_role": "boolean_result"},
    }
    candidate = {
        **VALID_PROGRAM,
        "operations": [source, boolean],
        "outputs": [{**VALID_PROGRAM["outputs"][0], "operation_id": "op_boolean"}],
    }

    with pytest.raises(ShapeProgramValidationError, match=f"SHAPE_PROGRAM_{operation_name.upper()}_INPUT"):
        validate_shape_program(candidate)
