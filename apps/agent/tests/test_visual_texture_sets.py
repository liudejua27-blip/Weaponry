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
        "base_color": "56027d78d117243d74b5638af0435f445b7cc9b61752721edf95516f35810011",
        "metallic_roughness": "58e6b663cb90a88f14be2ffe35e0679582f26da92f22548ddb8cecfec0d91562",
        "normal": "e6279c8325a62e5c4d631d66cbcbad15d52a0c3b34d77923ac1744eb62e1b5eb",
        "occlusion": "73c6d56857ad25334f01e28a1101eca973f0d3ef8763ae37d441285d2edfb96c",
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
