#!/usr/bin/env python3
"""FGC-A005 deterministic texture-bake -> GLB/readback smoke."""

from __future__ import annotations

import copy
import hashlib

from forgecad_agent.application.geometry_worker import compile_shape_program
from forgecad_agent.application.visual_texture_sets import (
    builtin_visual_material_count,
    surface_adornment_texture_cache_facts,
)


def _program() -> dict:
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": "shape_a005_smoke",
        "units": "millimeter",
        "seed": 5005,
        "triangle_budget": 10_000,
        "parameters": [],
        "operations": [{
            "operation_id": "op_shell",
            "op": "box",
            "inputs": [],
            "args": {
                "position": [0, 250, 0], "size": [600, 360, 400],
                "part_role": "shell", "material_id": "mat_primary", "zone_id": "zone_shell",
            },
        }],
        "outputs": [{
            "output_id": "output_shell", "operation_id": "op_shell", "kind": "mesh", "part_role": "shell",
        }],
        "non_functional_only": True,
    }


def _adornment(kind: str, motif: str, coverage: str, seed: int) -> dict:
    return {
        "schema_version": "SurfaceAdornmentProgram@1",
        "program_id": f"adorn_{kind}",
        "target_part_id": "part_shell",
        "target_zone_id": "zone_shell",
        "kind": kind,
        "motif": motif,
        "intensity": "balanced",
        "coverage": coverage,
        "seed": seed,
        "base_material": "mat_primary",
        "execution": "texture_bake",
        "skill_id": "skill_surface_finish",
        "skill_version": 1,
        "skill_sha256": "a" * 64,
        "generator": "a005_v1",
        "non_functional_only": True,
    }


def main() -> int:
    fixtures = (
        ("normal_relief", "parallel_groove", "full_zone"),
        ("pattern", "chevron_relief", "center_band"),
        ("flowline", "double_flowline", "symmetric_pair"),
        ("micro_surface", "hex_microgrid", "edge_band"),
    )
    for profile, extent in (("interactive_preview", 128), ("production_concept", 1024)):
        for offset, (kind, motif, coverage) in enumerate(fixtures):
            adornment = _adornment(kind, motif, coverage, 5005 + offset)
            first = compile_shape_program(
                _program(), artifact_profile_id=profile, surface_adornment_programs=[adornment]
            )
            second = compile_shape_program(
                _program(), artifact_profile_id=profile, surface_adornment_programs=[copy.deepcopy(adornment)]
            )
            assert first.glb_bytes == second.glb_bytes
            assert first.readback.model_dump(mode="json") == second.readback.model_dump(mode="json")
            dynamic = next(item for item in first.readback.visual_texture_sets if item.surface_adornment)
            assert first.readback.material_count == builtin_visual_material_count() + 1
            assert dynamic.surface_adornment == adornment
            assert dynamic.surface_adornment_sha256 == hashlib.sha256(
                __import__("json").dumps(adornment, sort_keys=True, separators=(",", ":")).encode()
            ).hexdigest()
            assert {item.texture_role for item in dynamic.maps} == {
                "base_color", "metallic_roughness", "normal", "occlusion", "emissive",
            }
            assert {(item.width, item.height) for item in dynamic.maps} == {(extent, extent)}
            assert next(item for item in dynamic.maps if item.texture_role == "normal").sha256 != next(
                item for item in dynamic.maps if item.texture_role == "metallic_roughness"
            ).sha256
    assert builtin_visual_material_count() == 8
    assert surface_adornment_texture_cache_facts()["entry_count"] <= 32
    print("A005 surface-adornment PBR smoke passed: four kinds, two profiles, five maps, dynamic GLB/readback provenance")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
