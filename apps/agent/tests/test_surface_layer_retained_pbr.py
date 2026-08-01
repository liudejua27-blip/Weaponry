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
    assert first.version == ("4" if profile == "production_concept" else "3")
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


def test_vectorized_retained_production_bake_preserves_frozen_bytes() -> None:
    lowering = _lowering()
    texture_set = surface_layer_visual_texture_set(
        lowering,
        artifact_profile_id="production_concept",
    )
    expected = {
        "base_color": "7e7baadf247d617b7b401b420d33dfaf3fec195825622ff454380a3ead118bc5",
        "metallic_roughness": "ccc0553798f5c729b2ad573b9e5c6a4f5cccd3b57a5118990deae13aeba58cd9",
        "normal": "a548d30122c6d7c007091d7bedfc80d74919290c423a82c52e6d2245a5e65628",
        "occlusion": "893c9201f20d5af18d0aedfb9daba0e756bedef820ff17639ee6a3580e6177ea",
        "emissive": "92106c39944909ea830b01f50f01897600402ffe19b3e09513efe50c8219eb12",
    }

    assert {
        item.texture_role: hashlib.sha256(
            surface_layer_visual_texture_png_bytes(
                lowering,
                artifact_profile_id="production_concept",
                texture_role=item.texture_role,
            )
        ).hexdigest()
        for item in texture_set.maps
    } == expected


def test_retained_surface_layer_renders_feature_driven_roughness_and_emissive_motifs() -> None:
    baseline = _lowering()
    feature_driven = copy.deepcopy(baseline)
    feature_driven["retained_layers"]["roughness_masks"][0]["motif"] = "microgrid"
    feature_driven["retained_layers"]["emissive_masks"][0]["motif"] = "panel_indicator"
    feature_driven["retained_layers_sha256"] = hashlib.sha256(
        _canonical(feature_driven["retained_layers"]).encode("utf-8")
    ).hexdigest()

    normalize_surface_layer_lowering(feature_driven)
    baseline_maps = {
        item.texture_role: surface_layer_visual_texture_png_bytes(
            baseline,
            artifact_profile_id="interactive_preview",
            texture_role=item.texture_role,
        )
        for item in surface_layer_visual_texture_set(
            baseline,
            artifact_profile_id="interactive_preview",
        ).maps
    }
    feature_maps = {
        item.texture_role: surface_layer_visual_texture_png_bytes(
            feature_driven,
            artifact_profile_id="interactive_preview",
            texture_role=item.texture_role,
        )
        for item in surface_layer_visual_texture_set(
            feature_driven,
            artifact_profile_id="interactive_preview",
        ).maps
    }
    assert feature_maps["metallic_roughness"] != baseline_maps["metallic_roughness"]
    assert feature_maps["emissive"] != baseline_maps["emissive"]


def test_retained_surface_layer_renders_reviewed_decal_motifs_into_base_color() -> None:
    baseline = _lowering()
    feature_driven = copy.deepcopy(baseline)
    feature_driven["retained_layers"]["decal_layers"][0]["motif"] = "warning_stripe"
    feature_driven["retained_layers"]["decal_layers"][0]["text_token"] = "CAUTION"
    feature_driven["retained_layers"]["decal_layers"][0]["color_token"] = "signal_red"
    feature_driven["retained_layers_sha256"] = hashlib.sha256(
        _canonical(feature_driven["retained_layers"]).encode("utf-8")
    ).hexdigest()

    normalize_surface_layer_lowering(feature_driven)
    baseline_base_color = surface_layer_visual_texture_png_bytes(
        baseline,
        artifact_profile_id="interactive_preview",
        texture_role="base_color",
    )
    feature_base_color = surface_layer_visual_texture_png_bytes(
        feature_driven,
        artifact_profile_id="interactive_preview",
        texture_role="base_color",
    )
    assert feature_base_color != baseline_base_color


def test_retained_surface_layer_renders_evidence_conditioned_silver_color_token() -> None:
    baseline = _lowering()
    silver = copy.deepcopy(baseline)
    silver["retained_layers"]["base_color_token"] = "silver"
    silver["retained_layers_sha256"] = hashlib.sha256(
        _canonical(silver["retained_layers"]).encode("utf-8")
    ).hexdigest()

    normalize_surface_layer_lowering(silver)
    assert surface_layer_visual_texture_png_bytes(
        silver,
        artifact_profile_id="interactive_preview",
        texture_role="base_color",
    ) != surface_layer_visual_texture_png_bytes(
        baseline,
        artifact_profile_id="interactive_preview",
        texture_role="base_color",
    )

    forged = copy.deepcopy(silver)
    forged["retained_layers"]["base_color_token"] = "rgb(255,255,255)"
    forged["retained_layers_sha256"] = hashlib.sha256(
        _canonical(forged["retained_layers"]).encode("utf-8")
    ).hexdigest()
    with pytest.raises(ValueError, match="base color token"):
        normalize_surface_layer_lowering(forged)


def test_retained_surface_finish_token_changes_metallic_roughness_channel() -> None:
    baseline = _lowering()
    polished = copy.deepcopy(baseline)
    polished["retained_layers"]["surface_finish_token"] = "polished_metal"
    polished["retained_layers_sha256"] = hashlib.sha256(
        _canonical(polished["retained_layers"]).encode("utf-8")
    ).hexdigest()
    ceramic = copy.deepcopy(baseline)
    ceramic["retained_layers"]["surface_finish_token"] = "ceramic_coat"
    ceramic["retained_layers_sha256"] = hashlib.sha256(
        _canonical(ceramic["retained_layers"]).encode("utf-8")
    ).hexdigest()

    normalize_surface_layer_lowering(polished)
    normalize_surface_layer_lowering(ceramic)
    baseline_map = surface_layer_visual_texture_png_bytes(
        baseline,
        artifact_profile_id="interactive_preview",
        texture_role="metallic_roughness",
    )
    polished_map = surface_layer_visual_texture_png_bytes(
        polished,
        artifact_profile_id="interactive_preview",
        texture_role="metallic_roughness",
    )
    ceramic_map = surface_layer_visual_texture_png_bytes(
        ceramic,
        artifact_profile_id="interactive_preview",
        texture_role="metallic_roughness",
    )
    assert polished_map != baseline_map
    assert ceramic_map != polished_map

    forged = copy.deepcopy(polished)
    forged["retained_layers"]["surface_finish_token"] = "freeform_finish"
    forged["retained_layers_sha256"] = hashlib.sha256(
        _canonical(forged["retained_layers"]).encode("utf-8")
    ).hexdigest()
    with pytest.raises(ValueError, match="surface finish token"):
        normalize_surface_layer_lowering(forged)


def test_category_open_natural_finish_tokens_change_all_relevant_pbr_fields() -> None:
    baseline = _lowering()
    wood = copy.deepcopy(baseline)
    wood["retained_layers"]["base_color_token"] = "wood_warm"
    wood["retained_layers"]["surface_finish_token"] = "wood_grain"
    wood["retained_layers_sha256"] = hashlib.sha256(
        _canonical(wood["retained_layers"]).encode("utf-8")
    ).hexdigest()
    fur = copy.deepcopy(baseline)
    fur["retained_layers"]["base_color_token"] = "fur_warm"
    fur["retained_layers"]["surface_finish_token"] = "fur_soft"
    fur["retained_layers_sha256"] = hashlib.sha256(
        _canonical(fur["retained_layers"]).encode("utf-8")
    ).hexdigest()

    for lowering in (wood, fur):
        normalize_surface_layer_lowering(lowering)

    for role in ("base_color", "metallic_roughness", "normal"):
        baseline_map = surface_layer_visual_texture_png_bytes(
            baseline,
            artifact_profile_id="interactive_preview",
            texture_role=role,
        )
        natural_map = surface_layer_visual_texture_png_bytes(
            wood,
            artifact_profile_id="interactive_preview",
            texture_role=role,
        )
        assert natural_map != baseline_map

    assert surface_layer_visual_texture_png_bytes(
        wood,
        artifact_profile_id="interactive_preview",
        texture_role="base_color",
    ) != surface_layer_visual_texture_png_bytes(
        fur,
        artifact_profile_id="interactive_preview",
        texture_role="base_color",
    )


def test_retained_surface_layer_renders_reviewed_vector_paths_into_pbr_channels() -> None:
    with_path = _lowering()
    without_path = copy.deepcopy(with_path)
    without_path["retained_layers"]["vector_paths"] = []
    without_path["retained_layers_sha256"] = hashlib.sha256(
        _canonical(without_path["retained_layers"]).encode("utf-8")
    ).hexdigest()

    normalize_surface_layer_lowering(with_path)
    normalize_surface_layer_lowering(without_path)
    for role in ("base_color", "metallic_roughness", "occlusion"):
        assert surface_layer_visual_texture_png_bytes(
            with_path,
            artifact_profile_id="interactive_preview",
            texture_role=role,
        ) != surface_layer_visual_texture_png_bytes(
            without_path,
            artifact_profile_id="interactive_preview",
            texture_role=role,
        )


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
