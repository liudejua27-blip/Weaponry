"""Golden coverage for the restricted retained SurfaceLayer PBR compiler."""

from __future__ import annotations

import copy
import hashlib
import json
import struct
import zlib
from pathlib import Path

import pytest

from forgecad_agent.application.surface_layer_pbr import (
    normalize_surface_layer_lowering,
    surface_layer_lowering_sha256,
    surface_layer_material_id,
    surface_layer_visual_texture_png_bytes,
    surface_layer_visual_texture_set,
)


FIXTURE = Path(__file__).resolve().parents[3] / "packages" / "concept-spec" / "fixtures" / "surface-layer-program-fixture.json"


def _canonical(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def _lowering() -> dict:
    source = json.loads(FIXTURE.read_text(encoding="utf-8"))
    source_sha = hashlib.sha256(_canonical(source).encode("utf-8")).hexdigest()
    retained = {
        key: source[key]
        for key in (
            "vector_paths",
            "decal_layers",
            "roughness_masks",
            "emissive_masks",
            "symmetry",
            "uv_frame",
            "quality_profile",
        )
    }
    return {
        "schema_version": "SurfaceLayerLowering@1",
        "source_program_sha256": source_sha,
        "adornments": [
            {
                "schema_version": "SurfaceAdornmentProgram@1",
                "program_id": f"adorn_{source_sha[:40]}_1",
                "target_part_id": source["target_part_id"],
                "target_zone_id": source["target_zone_id"],
                "kind": "normal_relief",
                "motif": "parallel_groove",
                "intensity": "balanced",
                "coverage": "center_band",
                "seed": 106,
                "base_material": source["base_material"],
                "execution": "texture_bake",
                "skill_id": source["skill_id"],
                "skill_version": source["skill_version"],
                "skill_sha256": source["skill_sha256"],
                "generator": "a005_v1",
                "non_functional_only": True,
            }
        ],
        "retained_layers": retained,
        "retained_layers_sha256": hashlib.sha256(_canonical(retained).encode("utf-8")).hexdigest(),
    }


def _png_pixels(payload: bytes) -> tuple[int, int, tuple[tuple[int, int, int], ...]]:
    assert payload[:8] == b"\x89PNG\r\n\x1a\n"
    width, height = struct.unpack(">II", payload[16:24])
    offset = 8
    encoded = bytearray()
    while offset < len(payload):
        length = struct.unpack(">I", payload[offset:offset + 4])[0]
        tag = payload[offset + 4:offset + 8]
        if tag == b"IDAT":
            encoded.extend(payload[offset + 8:offset + 8 + length])
        offset += 12 + length
    raw = zlib.decompress(bytes(encoded))
    rows = [raw[row * (width * 3 + 1) + 1:(row + 1) * (width * 3 + 1)] for row in range(height)]
    return width, height, tuple(
        tuple(row[index:index + 3])
        for row in rows
        for index in range(0, width * 3, 3)
    )


@pytest.mark.parametrize(("profile", "extent"), [("interactive_preview", 128), ("production_concept", 1024)])
def test_retained_surface_layers_bake_all_five_pbr_channels_deterministically(profile: str, extent: int) -> None:
    lowering = _lowering()
    first = surface_layer_visual_texture_set(lowering, artifact_profile_id=profile)
    second = surface_layer_visual_texture_set(copy.deepcopy(lowering), artifact_profile_id=profile)
    assert first.model_dump(mode="json") == second.model_dump(mode="json")
    assert first.material_id == surface_layer_material_id(lowering)
    assert first.version == "4"
    assert {item.texture_role for item in first.maps} == {
        "base_color", "metallic_roughness", "normal", "occlusion", "emissive"
    }
    assert {(item.width, item.height) for item in first.maps} == {(extent, extent)}
    payloads = {
        item.texture_role: surface_layer_visual_texture_png_bytes(
            lowering,
            artifact_profile_id=profile,
            texture_role=item.texture_role,
        )
        for item in first.maps
    }
    assert {role: hashlib.sha256(payload).hexdigest() for role, payload in payloads.items()} == {
        item.texture_role: item.sha256 for item in first.maps
    }
    assert all(_png_pixels(payload)[0:2] == (extent, extent) for payload in payloads.values())
    assert len(set(_png_pixels(payloads["base_color"])[2])) > 1
    assert len({pixel[1] for pixel in _png_pixels(payloads["metallic_roughness"])[2]}) > 1
    assert len(set(_png_pixels(payloads["normal"])[2])) > 1
    assert max(pixel[2] for pixel in _png_pixels(payloads["emissive"])[2]) > 0


def test_retained_lowering_rejects_untrusted_or_noncanonical_input() -> None:
    lowering = _lowering()
    assert normalize_surface_layer_lowering(lowering)["retained_layers_sha256"] == lowering["retained_layers_sha256"]
    assert len(surface_layer_lowering_sha256(lowering)) == 64

    forged = copy.deepcopy(lowering)
    forged["retained_layers"]["vector_paths"][0]["commands"][0]["points"][0][0] = 2
    with pytest.raises(ValueError, match="retained vector command"):
        normalize_surface_layer_lowering(forged)

    forged = copy.deepcopy(lowering)
    forged["retained_layers"]["svg"] = "<path d='M 0 0'/>"
    with pytest.raises(ValueError, match="exactly the reviewed fields"):
        normalize_surface_layer_lowering(forged)

    forged = copy.deepcopy(lowering)
    forged["retained_layers_sha256"] = "0" * 64
    with pytest.raises(ValueError, match="retained hash"):
        normalize_surface_layer_lowering(forged)

    forged = copy.deepcopy(lowering)
    forged["adornments"][0]["execution"] = "shell"
    with pytest.raises(ValueError, match="execution"):
        normalize_surface_layer_lowering(forged)
