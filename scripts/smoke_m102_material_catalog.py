#!/usr/bin/env python3
"""FGC-M102: verify the expanded, visual-only built-in material catalog."""

from forgecad_agent.application.material_catalog import list_material_presets


def main() -> int:
    presets = list_material_presets()
    assert len(presets) == 13
    ids = {preset.material_id for preset in presets}
    assert len(ids) == len(presets)
    assert {preset.category for preset in presets} >= {"metal", "polymer", "rubber", "composite", "glass", "coating"}
    assert all(preset.visual_only is True for preset in presets)
    assert all(preset.provenance == "forgecad_builtin" for preset in presets)
    assert all(preset.source == "forgecad_builtin" and preset.license == "not_applicable" for preset in presets)
    assert all(not any(key in preset.pbr.model_dump() for key in ("density", "strength", "temperature", "supplier")) for preset in presets)
    print(f"FGC-M102 material catalog smoke passed: {len(presets)} visual-only presets across 6 categories")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
