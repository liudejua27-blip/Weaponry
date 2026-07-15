#!/usr/bin/env python3
"""Gate the bounded ProfileSketch/section contract and ShapeProgram provenance."""

from __future__ import annotations

import copy

from pydantic import TypeAdapter

from forgecad_agent.application.profile_contracts import (
    ProfileContractValidationError,
    ProfileSectionSetPayload,
    ProfileSketchPayload,
    canonical_profile_payload,
    resample_profile_contour,
    validate_profile_section_set,
    validate_profile_sketch,
)
from forgecad_agent.application.shape_program import ShapeProgramValidationError, validate_shape_program


def rectangle(sketch_id: str = "sketch_shell", *, clockwise: bool = False, with_hole: bool = False) -> dict:
    start = [-0.8, -0.6]
    points = (
        [[-0.8, 0.6], [0.8, 0.6], [0.8, -0.6], start]
        if clockwise
        else [[0.8, -0.6], [0.8, 0.6], [-0.8, 0.6], start]
    )
    holes = []
    if with_hole:
        holes.append(
            {
                "hole_id": "hole_window",
                "winding": "clockwise",
                "start": [-0.2, -0.2],
                "segments": [
                    {"kind": "line", "to": [-0.2, 0.2]},
                    {"kind": "line", "to": [0.2, 0.2]},
                    {"kind": "line", "to": [0.2, -0.2]},
                    {"kind": "line", "to": [-0.2, -0.2]},
                ],
            }
        )
    return {
        "schema_version": "ProfileSketch@1",
        "sketch_id": sketch_id,
        "version": 1,
        "plane": "cross_section",
        "closed": True,
        "winding": "clockwise" if clockwise else "counter_clockwise",
        "start": start,
        "segments": [{"kind": "line", "to": point} for point in points],
        "holes": holes,
        "normalized_bounds": {"min": [-0.8, -0.6], "max": [0.8, 0.6]},
        "symmetry": "vertical",
        "continuity_hint": "linear",
        "resample_count": 32,
        "provenance": {"source": "agent", "source_ref": "fixture_g820"},
    }


def expect_profile_rejected(payload: dict, code: str) -> None:
    try:
        validate_profile_sketch(payload)
    except ProfileContractValidationError as exc:
        assert code in str(exc), exc
        return
    raise AssertionError(f"expected profile rejection: {code}")


def expect_sections_rejected(payload: dict, code: str) -> None:
    try:
        validate_profile_section_set(payload)
    except ProfileContractValidationError as exc:
        assert code in str(exc), exc
        return
    raise AssertionError(f"expected section rejection: {code}")


def section_set() -> dict:
    profile = rectangle()
    return {
        "schema_version": "ProfileSectionSet@1",
        "section_set_id": "sectionset_shell",
        "version": 1,
        "main_axis": "x",
        "profiles": [profile],
        "sections": [
            {"section_id": "section_start", "position": -0.75, "profile_sketch_id": profile["sketch_id"], "scale": 0.8, "twist_degrees": -5, "cap_policy": "start"},
            {"section_id": "section_middle", "position": 0, "profile_sketch_id": profile["sketch_id"], "scale": 1.0, "twist_degrees": 0, "cap_policy": "none"},
            {"section_id": "section_end", "position": 0.75, "profile_sketch_id": profile["sketch_id"], "scale": 0.7, "twist_degrees": 5, "cap_policy": "end"},
        ],
        "resample_policy": {"mode": "uniform_count", "count": 32},
        "symmetry": "vertical",
        "provenance": {"source": "component_recipe", "source_ref": "recipe_shell"},
    }


def old_shape_program() -> dict:
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": "shape_g820_legacy",
        "units": "millimeter",
        "seed": 820,
        "triangle_budget": 1000,
        "parameters": [],
        "operations": [{"operation_id": "op_box", "op": "box", "inputs": [], "args": {"size": [100, 100, 100], "part_role": "body_shell"}}],
        "outputs": [{"output_id": "output_box", "operation_id": "op_box", "kind": "mesh", "part_role": "body_shell"}],
        "non_functional_only": True,
    }


def main() -> int:
    ccw = rectangle(with_hole=True)
    cw = rectangle(clockwise=True)
    assert validate_profile_sketch(ccw)["winding"] == "counter_clockwise"
    TypeAdapter(ProfileSketchPayload).validate_python(ccw)
    assert len(resample_profile_contour(ccw)) == 32

    _, ccw_json, ccw_hash = canonical_profile_payload(rectangle())
    cw_payload, cw_json, cw_hash = canonical_profile_payload(cw)
    assert cw_payload["winding"] == "counter_clockwise"
    assert (ccw_json, ccw_hash) == (cw_json, cw_hash)

    open_curve = {
        **rectangle("sketch_open"),
        "plane": "side",
        "closed": False,
        "winding": "open",
        "start": [-0.8, -0.4],
        "segments": [
            {"kind": "quadratic", "control": [0, 0.8], "to": [0.4, 0.1]},
            {"kind": "cubic", "control_1": [0.55, 0], "control_2": [0.7, -0.3], "to": [0.8, -0.4]},
        ],
        "holes": [],
        "normalized_bounds": {"min": [-0.8, -0.4], "max": [0.8, 0.8]},
        "resample_count": 24,
    }
    validate_profile_sketch(open_curve)
    assert len(resample_profile_contour(open_curve)) == 24

    bowtie = rectangle("sketch_bowtie")
    bowtie["segments"] = [
        {"kind": "line", "to": [0.8, 0.6]},
        {"kind": "line", "to": [-0.8, 0.6]},
        {"kind": "line", "to": [0.8, -0.6]},
        {"kind": "line", "to": [-0.8, -0.6]},
    ]
    expect_profile_rejected(bowtie, "SELF_INTERSECTION")

    outside_hole = rectangle("sketch_outside", with_hole=True)
    outside_hole["holes"][0]["start"] = [0.7, -0.2]
    outside_hole["holes"][0]["segments"] = [
        {"kind": "line", "to": [0.7, 0.2]},
        {"kind": "line", "to": [0.95, 0.2]},
        {"kind": "line", "to": [0.95, -0.2]},
        {"kind": "line", "to": [0.7, -0.2]},
    ]
    outside_hole["normalized_bounds"]["max"][0] = 0.95
    expect_profile_rejected(outside_hole, "HOLE_OUTSIDE")

    wrong_hole_winding = rectangle("sketch_hole_winding", with_hole=True)
    wrong_hole_winding["holes"][0]["winding"] = "counter_clockwise"
    expect_profile_rejected(wrong_hole_winding, "HOLE_WINDING")

    degenerate = rectangle("sketch_degenerate")
    degenerate["start"] = [0, 0]
    degenerate["segments"] = [{"kind": "line", "to": [0, 0]}] * 3
    degenerate["normalized_bounds"] = {"min": [0, 0], "max": [0, 0]}
    expect_profile_rejected(degenerate, "DEGENERATE")

    non_finite = rectangle("sketch_nonfinite")
    non_finite["segments"][0]["to"][0] = float("inf")
    expect_profile_rejected(non_finite, "SCHEMA_INVALID")

    not_closed = rectangle("sketch_not_closed")
    not_closed["segments"][-1]["to"] = [-0.7, -0.6]
    expect_profile_rejected(not_closed, "CLOSED_CONTOUR")

    too_many = rectangle("sketch_budget")
    too_many["segments"] = [{"kind": "line", "to": [0, 0]} for _ in range(65)]
    expect_profile_rejected(too_many, "SCHEMA_INVALID")
    too_few_samples = rectangle("sketch_samples")
    too_few_samples["resample_count"] = 7
    expect_profile_rejected(too_few_samples, "SCHEMA_INVALID")

    sections = section_set()
    validate_profile_section_set(sections)
    TypeAdapter(ProfileSectionSetPayload).validate_python(sections)
    canonical_sections, canonical_json, section_hash = canonical_profile_payload(sections)
    assert len(canonical_json) > 100 and len(section_hash) == 64
    assert canonical_sections["sections"] == sections["sections"]

    unordered = copy.deepcopy(sections)
    unordered["sections"][1]["position"] = -0.8
    expect_sections_rejected(unordered, "ORDER_INVALID")
    duplicate = copy.deepcopy(sections)
    duplicate["sections"][1]["position"] = duplicate["sections"][0]["position"]
    expect_sections_rejected(duplicate, "ORDER_INVALID")
    too_many_sections = copy.deepcopy(sections)
    too_many_sections["sections"] = [
        {"section_id": f"section_{index}", "position": -1 + index / 6, "profile_sketch_id": "sketch_shell", "scale": 1, "twist_degrees": 0, "cap_policy": "none"}
        for index in range(13)
    ]
    expect_sections_rejected(too_many_sections, "SCHEMA_INVALID")
    mismatch = copy.deepcopy(sections)
    mismatch["resample_policy"]["count"] = 64
    expect_sections_rejected(mismatch, "RESAMPLE_MISMATCH")

    legacy = old_shape_program()
    assert validate_shape_program(legacy) == legacy
    program = copy.deepcopy(legacy)
    program["program_id"] = "shape_g820_provenance"
    program["profile_inputs"] = [
        {
            "input_id": "profileinput_sections",
            "input_kind": "profile_section_set",
            "contract_version": "ProfileSectionSet@1",
            "input_sha256": section_hash,
            "canonical_payload": canonical_sections,
        }
    ]
    validate_shape_program(program)
    corrupted = copy.deepcopy(program)
    corrupted["profile_inputs"][0]["input_sha256"] = "0" * 64
    try:
        validate_shape_program(corrupted)
    except ShapeProgramValidationError as exc:
        assert "PROFILE_INPUT_HASH_MISMATCH" in str(exc), exc
    else:
        raise AssertionError("corrupted profile provenance hash must be rejected")

    print("G820 profile contract smoke passed: bounded curves, holes, sections, canonical hashes and legacy ShapeProgram compatibility")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
