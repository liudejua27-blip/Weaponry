#!/usr/bin/env python3
"""Fail closed if the production studio can flatten ForgeCAD's PBR palette.

This is intentionally a source-and-contract smoke, not a screenshot heuristic:
the GLB owns texture bytes while the desktop owns the one renderer's studio
energy and reflection calibration.  It proves those two sides remain aligned
and that the distinct automotive/composite/metal/rubber/emissive palettes are
still materially distinguishable before browser rendering.
"""

from __future__ import annotations

import re
from pathlib import Path

from forgecad_agent.application.visual_texture_sets import (
    builtin_material_properties,
    builtin_visual_material_count,
    studio_environment_manifest,
)


ROOT = Path(__file__).resolve().parents[1]
VIEWPORT = ROOT / "apps/desktop/src/features/cad-workbench/ModuleGraphViewport.tsx"
SCHEMA = "ForgeCADMaterialPaletteContrastSmoke@1"


def _luminance(rgb: object) -> float:
    red, green, blue = (float(value) for value in rgb)  # type: ignore[arg-type]
    return (0.2126 * red + 0.7152 * green + 0.0722 * blue) / 255.0


def _material(material_id: str) -> dict[str, object]:
    for index in range(builtin_visual_material_count()):
        item = dict(builtin_material_properties(index))
        if item["material_id"] == material_id:
            return item
    raise AssertionError(f"missing built-in material: {material_id}")


def main() -> None:
    manifest = studio_environment_manifest()
    legacy_manifest = studio_environment_manifest("env_forgecad_room_studio_v1")
    assert legacy_manifest["environment_sha256"] == "291b13f7d1606bd3c180a3fb9850538f5d23208086d7f8488c2214fa59061042"
    lighting = manifest["cad_neutral_lighting"]
    assert isinstance(lighting, dict)
    assert manifest["environment_id"] == "env_forgecad_room_studio_v2"
    assert manifest["tone_mapping_exposure"] == 0.62
    assert lighting["hemisphere"]["intensity"] == 0.58
    assert lighting["ambient"]["intensity"] == 0.12
    assert lighting["key"]["intensity"] == 1.35
    assert lighting["rim"]["intensity"] == 0.5
    assert lighting["warm_rim"]["intensity"] == 0.1
    total_direct_fill = sum(
        float(lighting[name]["intensity"])
        for name in ("hemisphere", "ambient", "key", "rim", "warm_rim")
    )
    assert abs(total_direct_fill - 2.65) < 1e-9
    assert total_direct_fill < 3.0

    automotive = _material("mat_automotive_paint")
    graphite = _material("mat_primary")
    aluminium = _material("mat_aluminum")
    composite = _material("mat_composite")
    rubber = _material("mat_rubber")
    emissive = _material("mat_emissive_blue")
    automotive_base = automotive["base"]
    assert isinstance(automotive_base, tuple)
    assert automotive_base[2] >= automotive_base[0] * 2.5
    assert automotive["clearcoat"] == 0.22
    assert _luminance(aluminium["base"]) - _luminance(composite["base"]) >= 0.35
    assert _luminance(composite["base"]) - _luminance(graphite["base"]) >= 0.05
    assert _luminance(graphite["base"]) - _luminance(rubber["base"]) >= 0.03
    assert float(graphite["metallic"]) - float(composite["metallic"]) >= 25
    assert float(rubber["roughness"]) - float(composite["roughness"]) >= 35
    assert emissive["emissive"] == (12, 112, 255)

    source = VIEWPORT.read_text(encoding="utf-8")
    environment_hash = str(manifest["environment_sha256"])
    assert environment_hash in source
    assert str(legacy_manifest["environment_sha256"]) in source
    assert "studioEnvironmentIdFromEmbedded" in source
    assert "applyStudioEnvironment" in source
    for token in (
        "tone_mapping_exposure: 0.62",
        "intensity: 0.58",
        "intensity: 0.12",
        "intensity: 1.35",
        "intensity: 0.5",
        "intensity: 0.1",
        "applyForgecadPbrDisplayCalibration",
        "FORGECAD_PBR_ENVIRONMENT_INTENSITY_BY_MATERIAL_ID",
        "mat_automotive_paint: 0.52",
        "mat_aluminum: 0.42",
        "mat_composite: 0.25",
        "mat_rubber: 0.14",
    ):
        assert token in source, f"desktop palette contract missing {token}"
    assert source.count("new THREE.WebGLRenderer(") == 1
    assert not re.search(r"PLACEHOLDER_RECOMPUTE", source)

    print({
        "schema_version": SCHEMA,
        "status": "pass",
        "environment_sha256": environment_hash,
        "tone_mapping_exposure": manifest["tone_mapping_exposure"],
        "direct_fill_energy": total_direct_fill,
        "palette_luminance": {
            "automotive_paint": round(_luminance(automotive["base"]), 4),
            "aluminum": round(_luminance(aluminium["base"]), 4),
            "graphite": round(_luminance(graphite["base"]), 4),
            "composite": round(_luminance(composite["base"]), 4),
            "rubber": round(_luminance(rubber["base"]), 4),
        },
        "single_renderer": True,
    })


if __name__ == "__main__":
    main()
