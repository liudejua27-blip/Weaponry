"""Focused truth-preservation coverage for M108B GLB/readback comparison."""

from __future__ import annotations

import copy
import hashlib

import pytest

from forgecad_agent.application.geometry_worker import compile_shape_program
from prepare_m108b_asset_preflight import PreflightError, _assert_readback_matches_glb


def _program() -> dict:
    return {
        "schema_version": "ShapeProgram@1",
        "program_id": "shape_m108b_preflight_readback",
        "units": "millimeter",
        "seed": 106,
        "triangle_budget": 2_000,
        "parameters": [],
        "operations": [
            {
                "operation_id": "op_shell",
                "op": "box",
                "inputs": [],
                "args": {
                    "position": [0, 100, 0],
                    "size": [240, 180, 160],
                    "part_role": "shell",
                    "material_id": "mat_primary",
                    "zone_id": "zone_shell",
                },
            }
        ],
        "outputs": [
            {"output_id": "output_shell", "operation_id": "op_shell", "kind": "mesh", "part_role": "shell"}
        ],
        "non_functional_only": True,
    }


def _adornment() -> dict:
    return {
        "schema_version": "SurfaceAdornmentProgram@1",
        "program_id": "adorn_shell",
        "target_part_id": "part_shell",
        "target_zone_id": "zone_shell",
        "kind": "flowline",
        "motif": "double_flowline",
        "intensity": "balanced",
        "coverage": "center_band",
        "seed": 106,
        "base_material": "mat_primary",
        "execution": "texture_bake",
        "skill_id": "skill_surface_finish",
        "skill_version": 1,
        "skill_sha256": "a" * 64,
        "generator": "a005_v1",
        "non_functional_only": True,
    }


def _assert_compiled_readback(*, adornment: dict | None = None) -> dict:
    result = compile_shape_program(
        _program(),
        artifact_profile_id="production_concept",
        surface_adornment_programs=[adornment] if adornment else [],
    )
    return _assert_readback_matches_glb(
        glb=result.glb_bytes,
        readback=result.readback.model_dump(mode="json"),
        expected_glb_sha256=hashlib.sha256(result.glb_bytes).hexdigest(),
    )


def test_preflight_omits_only_absent_adornment_facts() -> None:
    readback = _assert_compiled_readback()
    texture_set = readback["visual_texture_sets"][0]
    assert "surface_adornment" not in texture_set
    assert "surface_adornment_sha256" not in texture_set


def test_preflight_preserves_present_adornment_provenance_and_rejects_drift() -> None:
    adornment = _adornment()
    readback = _assert_compiled_readback(adornment=adornment)
    texture_set = next(
        item for item in readback["visual_texture_sets"] if item.get("surface_adornment")
    )
    assert texture_set["surface_adornment"] == adornment
    assert isinstance(texture_set["surface_adornment_sha256"], str)

    tampered = copy.deepcopy(readback)
    tampered["visual_texture_sets"][-1]["surface_adornment_sha256"] = "0" * 64
    result = compile_shape_program(
        _program(), artifact_profile_id="production_concept", surface_adornment_programs=[adornment]
    )
    with pytest.raises(PreflightError, match="M108B_PREFLIGHT_GLB_READBACK_FACT_DRIFT:visual_texture_sets"):
        _assert_readback_matches_glb(
            glb=result.glb_bytes,
            readback=tampered,
            expected_glb_sha256=hashlib.sha256(result.glb_bytes).hexdigest(),
        )
