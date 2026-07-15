#!/usr/bin/env python3
"""Gate restricted ProfileSectionSet loft and exact GLB readback facts."""

from __future__ import annotations

import copy

from forgecad_agent.application.geometry_worker import compile_shape_program, read_shape_program_glb_facts
from forgecad_agent.application.profile_contracts import canonical_profile_payload
from forgecad_agent.application.shape_program import ShapeProgramValidationError, validate_shape_program
from forgecad_agent.application.shape_program_runtime import UnsupportedRuntimeOperationError


def profile(sketch_id: str, width: float, height: float, *, curved: bool = False) -> dict:
    start = [-width, -height]
    if curved:
        segments = [
            {"kind": "line", "to": [width * 0.65, -height]},
            {"kind": "quadratic", "control": [width, -height], "to": [width, 0]},
            {"kind": "quadratic", "control": [width, height], "to": [0, height]},
            {"kind": "line", "to": [-width, height]},
            {"kind": "line", "to": start},
        ]
    else:
        segments = [
            {"kind": "line", "to": [width, -height]},
            {"kind": "line", "to": [width, height]},
            {"kind": "line", "to": [-width, height]},
            {"kind": "line", "to": start},
        ]
    return {
        "schema_version": "ProfileSketch@1",
        "sketch_id": sketch_id,
        "version": 1,
        "plane": "cross_section",
        "closed": True,
        "winding": "counter_clockwise",
        "start": start,
        "segments": segments,
        "holes": [],
        "normalized_bounds": {"min": [-width, -height], "max": [width, height]},
        "symmetry": "vertical",
        "continuity_hint": "tangent" if curved else "linear",
        "resample_count": 24,
        "provenance": {"source": "component_recipe", "source_ref": "g822_shell_recipe"},
    }


def section_set(suffix: str, axis: str, *, twist: float, curved: bool) -> dict:
    profiles = [
        profile(f"sketch_{suffix}_start", 0.55, 0.38, curved=curved),
        profile(f"sketch_{suffix}_middle", 0.88, 0.58, curved=curved),
        profile(f"sketch_{suffix}_end", 0.48, 0.32, curved=curved),
    ]
    return {
        "schema_version": "ProfileSectionSet@1",
        "section_set_id": f"sectionset_{suffix}",
        "version": 1,
        "main_axis": axis,
        "profiles": profiles,
        "sections": [
            {"section_id": f"section_{suffix}_start", "position": -0.9, "profile_sketch_id": profiles[0]["sketch_id"], "scale": 0.8, "twist_degrees": -twist, "cap_policy": "start"},
            {"section_id": f"section_{suffix}_middle", "position": 0, "profile_sketch_id": profiles[1]["sketch_id"], "scale": 1.1, "twist_degrees": 0, "cap_policy": "none"},
            {"section_id": f"section_{suffix}_end", "position": 0.9, "profile_sketch_id": profiles[2]["sketch_id"], "scale": 0.72, "twist_degrees": twist, "cap_policy": "end"},
        ],
        "resample_policy": {"mode": "uniform_count", "count": 24},
        "symmetry": "vertical",
        "provenance": {"source": "component_recipe", "source_ref": f"recipe_{suffix}"},
    }


def program(suffix: str, axis: str, *, twist: float = 8, curved: bool = False) -> dict:
    canonical, _json, digest = canonical_profile_payload(section_set(suffix, axis, twist=twist, curved=curved))
    input_id = f"profileinput_{suffix}"
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": f"shape_g822_{suffix}",
        "units": "millimeter",
        "seed": 822,
        "triangle_budget": 100000,
        "parameters": [],
        "profile_inputs": [{
            "input_id": input_id,
            "input_kind": "profile_section_set",
            "contract_version": "ProfileSectionSet@1",
            "input_sha256": digest,
            "canonical_payload": canonical,
        }],
        "operations": [{
            "operation_id": "op_loft",
            "op": "loft",
            "inputs": [],
            "args": {
                "section_set_input_id": input_id,
                "cross_section_scale": [720, 520],
                "axis_length": 1800,
                "continuity": "linear",
                "position": [0, 700, 0],
                "part_role": f"{suffix}_shell",
                "material_id": "mat_aluminum",
            },
        }],
        "outputs": [{"output_id": "output_loft", "operation_id": "op_loft", "kind": "mesh", "part_role": f"{suffix}_shell"}],
        "non_functional_only": True,
    }


def expect_rejected(candidate: dict, code: str) -> None:
    try:
        compile_shape_program(candidate)
    except (ShapeProgramValidationError, UnsupportedRuntimeOperationError, ValueError) as exc:
        assert code in str(exc), exc
        return
    raise AssertionError(f"expected rejection containing {code}")


def main() -> int:
    fixtures = (
        program("vehicle", "x", twist=6, curved=True),
        program("aircraft", "x", twist=10, curved=True),
        program("appliance", "z", twist=4),
        program("robot_arm", "y", twist=8),
    )
    for candidate in fixtures:
        validate_shape_program(candidate)
        compiled = compile_shape_program(candidate)
        repeated = compile_shape_program(candidate)
        assert compiled.glb_bytes == repeated.glb_bytes
        assert compiled.readback.model_dump() == repeated.readback.model_dump()
        facts = read_shape_program_glb_facts(compiled.glb_bytes)
        assert facts.triangle_count == compiled.readback.triangle_count == 140
        assert all(value > 0 for value in facts.bounds_mm)
        provenance = facts.surface_provenance[0]
        assert provenance["surface_roles"] == ["loft_side", "seam", "start_cap", "end_cap"]
        assert sum(item["triangle_count"] for item in provenance["surface_ranges"]) == facts.triangle_count
        assert provenance["closed"] is True
        assert provenance["boundary_edge_count"] == provenance["non_manifold_edge_count"] == provenance["degenerate_triangle_count"] == 0
        assert all(0 <= value <= 1 for value in [*provenance["uv0_min"], *provenance["uv0_max"]])

    unordered = copy.deepcopy(fixtures[0])
    unordered["profile_inputs"][0]["canonical_payload"]["sections"][1]["position"] = -0.95
    expect_rejected(unordered, "ORDER_INVALID")

    flip = program("flip", "x", twist=0)
    payload = flip["profile_inputs"][0]["canonical_payload"]
    payload["sections"][0]["twist_degrees"] = -45
    payload["sections"][1]["twist_degrees"] = 45
    _normalized, _canonical, digest = canonical_profile_payload(payload)
    flip["profile_inputs"][0]["input_sha256"] = digest
    expect_rejected(flip, "TWIST_FLIP_RISK")

    self_intersect = copy.deepcopy(section_set("self_intersect", "x", twist=0, curved=False))
    sketch = self_intersect["profiles"][0]
    sketch["segments"] = [
        {"kind": "line", "to": [0.55, 0.38]},
        {"kind": "line", "to": [-0.55, 0.38]},
        {"kind": "line", "to": [0.55, -0.38]},
        {"kind": "line", "to": [-0.55, -0.38]},
    ]
    try:
        canonical_profile_payload(self_intersect)
    except ValueError as exc:
        assert "SELF_INTERSECTION" in str(exc), exc
    else:
        raise AssertionError("self-intersecting loft section must be rejected")

    point_mismatch = copy.deepcopy(section_set("point_mismatch", "x", twist=0, curved=False))
    point_mismatch["profiles"][1]["resample_count"] = 16
    try:
        canonical_profile_payload(point_mismatch)
    except ValueError as exc:
        assert "RESAMPLE_MISMATCH" in str(exc), exc
    else:
        raise AssertionError("mixed loft resample counts must be rejected")

    degenerate = copy.deepcopy(section_set("degenerate", "x", twist=0, curved=False))
    sketch = degenerate["profiles"][0]
    sketch["start"] = [0, 0]
    sketch["segments"] = [{"kind": "line", "to": [0, 0]}] * 3
    sketch["normalized_bounds"] = {"min": [0, 0], "max": [0, 0]}
    try:
        canonical_profile_payload(degenerate)
    except ValueError as exc:
        assert "DEGENERATE" in str(exc), exc
    else:
        raise AssertionError("zero-area loft section must be rejected")

    over_bounds = copy.deepcopy(fixtures[0])
    over_bounds["operations"][0]["args"]["axis_length"] = 100001
    expect_rejected(over_bounds, "SCHEMA_INVALID")

    damaged = copy.deepcopy(fixtures[0])
    damaged["profile_inputs"][0]["input_sha256"] = "0" * 64
    expect_rejected(damaged, "PROFILE_INPUT_HASH_MISMATCH")
    over_budget = copy.deepcopy(fixtures[0])
    over_budget["triangle_budget"] = 100
    expect_rejected(over_budget, "triangle count")

    print("G822 loft smoke passed: four shell domains, caps, twist, UV0, topology, deterministic readback and fail-closed bounds")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
