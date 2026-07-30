from __future__ import annotations

import hashlib

from forgecad_agent.application.visual_texture_sets import (
    builtin_visual_texture_set_for_material_index,
    visual_texture_png_bytes,
)


def test_vectorized_production_texture_renderer_preserves_frozen_v4_bytes() -> None:
    texture_set = builtin_visual_texture_set_for_material_index(
        0,
        artifact_profile_id="production_concept",
    )
    expected = {
        "base_color": "ba4488fe38591986dd7c17f1d160b269b5c6b560fe83a17157075c289b9cc3fa",
        "metallic_roughness": "41aa990e98e6dee267feb4e99a68c61b457102792a95589d82e90701d46d9ca0",
        "normal": "3078d36e905da94f6c2435201a5391d09de1f7fca28cfd6a569fb56e86ca7767",
        "occlusion": "4d3af78bf011ae17d894cc142fc1825cabc685290685b3f37b51da961b1be05e",
        "emissive": "87658cfaf8e619d7f15fe7179e5874c38663d3254302928d9c1e7eaacee0a9f4",
    }

    assert texture_set.version == "4"
    assert {(item.width, item.height) for item in texture_set.maps} == {(1024, 1024)}
    assert {
        item.texture_role: hashlib.sha256(
            visual_texture_png_bytes(item.texture_id)
        ).hexdigest()
        for item in texture_set.maps
    } == expected
