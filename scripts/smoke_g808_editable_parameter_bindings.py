#!/usr/bin/env python3
"""Smoke G808's non-executable, bounded Agent part parameter declarations."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator
from pydantic import ValidationError

from forgecad_agent.application.agent_models import BlockoutPartCandidate, EditableParameterBinding


ROOT = Path(__file__).resolve().parents[1]
COMMON_SCHEMA = json.loads((ROOT / "packages" / "concept-spec" / "schemas" / "common.schema.json").read_text(encoding="utf-8"))
BINDING_SCHEMA = COMMON_SCHEMA["$defs"]["editable_parameter_binding"]


def binding(**overrides: object) -> dict[str, object]:
    return {
        "schema_version": "EditableParameterBinding@1",
        "parameter_id": "editparam_body_length_ratio",
        "path": "transform.scale.x",
        "display_name": "主体长度比例",
        "unit": "ratio",
        "default": 1.0,
        "min": 0.6,
        "max": 1.6,
        "step": 0.05,
        **overrides,
    }


def part(**overrides: object) -> BlockoutPartCandidate:
    return BlockoutPartCandidate.model_validate({
        "part_id": "part_g808_body",
        "role": "primary_body",
        "parent_part_id": None,
        "position_mm": [0, 0, 0],
        "size_mm": [180, 40, 30],
        "material_zone_ids": ["zone_body"],
        "editable_parameters": ["transform.scale.x", "transform.position.x"],
        "locked": False,
        "provenance": "agent_generated",
        **overrides,
    })


def expect_invalid(**overrides: object) -> None:
    try:
        EditableParameterBinding.model_validate(binding(**overrides))
    except ValidationError:
        return
    raise AssertionError(f"binding unexpectedly accepted: {overrides}")


def main() -> int:
    # Old Agent assets have no bindings and must continue to load.
    assert part().editable_parameter_bindings == []

    declared = part(editable_parameter_bindings=[
        binding(),
        binding(
            parameter_id="editparam_body_offset_x",
            path="transform.position.x",
            display_name="主体前后位置",
            unit="millimeter",
            default=0,
            min=-200,
            max=200,
            step=10,
        ),
    ])
    assert [item.path for item in declared.editable_parameter_bindings] == [
        "transform.scale.x",
        "transform.position.x",
    ]

    # The independently generated JSON contract rejects undeclared paths and
    # unknown fields before an API payload reaches Pydantic.
    schema_validator = Draft202012Validator(BINDING_SCHEMA)
    assert not list(schema_validator.iter_errors(binding()))
    assert list(schema_validator.iter_errors(binding(path="transform.rotation.x")))
    assert list(schema_validator.iter_errors(binding(unexpected=True)))

    expect_invalid(path="transform.rotation.x")
    expect_invalid(unit="millimeter")
    expect_invalid(min=1.2, max=1.0)
    expect_invalid(default=2.0)
    expect_invalid(step=2.0)
    expect_invalid(min=0.01)
    expect_invalid(parameter_id="editparam_bad", path="transform.position.x", unit="ratio", min=-10, max=10, default=0, step=1)

    try:
        part(editable_parameter_bindings=[binding(), binding(parameter_id="editparam_duplicate")])
    except ValidationError:
        pass
    else:
        raise AssertionError("duplicate editable binding paths must be rejected")

    print("G808 editable parameter bindings smoke passed: JSON/Pydantic compatibility, finite ranges, units, paths and uniqueness")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
