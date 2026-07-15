#!/usr/bin/env python3
"""Validate the lightweight visual-only material catalog."""

from forgecad_agent.application.material_catalog import list_material_presets
from forgecad_agent.application.agent_models import AgentMaterialPreset
from pydantic import ValidationError


def main() -> int:
    presets = list_material_presets()
    assert len(presets) == 13, "M102 catalog must expose the 7 baseline plus 6 category presets"
    ids = [preset.material_id for preset in presets]
    assert len(ids) == len(set(ids))
    assert {preset.category for preset in presets} >= {"metal", "polymer", "rubber", "composite", "glass", "coating"}
    assert all(preset.visual_only is True for preset in presets)
    assert all(0 <= preset.pbr.metallic <= 1 for preset in presets)
    assert all(0 < preset.pbr.opacity <= 1 for preset in presets)
    assert all(preset.source == preset.provenance and preset.version == "1" for preset in presets)
    assert all(preset.license == "not_applicable" for preset in presets)
    assert all("density" not in preset.pbr.model_dump() for preset in presets)
    legacy = AgentMaterialPreset.model_validate({
        "schema_version": "MaterialPreset@1",
        "material_id": "mat_legacy",
        "display_name": "兼容旧材质",
        "category": "metal",
        "pbr": {"base_color": "#112233", "metallic": 0.4, "roughness": 0.5, "opacity": 1},
        "visual_only": True,
        "allowed_domains": ["vehicle_concept"],
        "provenance": "forgecad_builtin",
    })
    assert legacy.source == "forgecad_builtin" and legacy.version == "1" and legacy.visual_tags == ["metal"]
    try:
        AgentMaterialPreset.model_validate({**legacy.model_dump(mode="json"), "pbr": {**legacy.pbr.model_dump(), "ior": 0.5}})
    except ValidationError:
        pass
    else:
        raise AssertionError("invalid material IOR was accepted")
    print(f"G6 material catalog smoke passed: {len(presets)} visual-only presets")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
