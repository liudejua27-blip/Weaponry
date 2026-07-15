#!/usr/bin/env python3
"""FGC-M108: same-GLB visual PBR, zone mapping and studio readback gate."""

from __future__ import annotations

import argparse
import base64
import copy
import json
import struct
import warnings
from pathlib import Path

warnings.simplefilter("ignore", DeprecationWarning)

from jsonschema import Draft202012Validator, RefResolver  # noqa: E402

from forgecad_agent.application.domain_packs import domain_pack_for_message  # noqa: E402
from forgecad_agent.application.geometry_worker import (  # noqa: E402
    build_blockout,
    list_blockout_variants,
    read_shape_program_glb_facts,
)
from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner  # noqa: E402
from forgecad_agent.application.visual_texture_sets import builtin_visual_material_count  # noqa: E402


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_DIR = ROOT / "packages" / "concept-spec" / "schemas"
BRIEFS = (
    "设计一个具有多材质外观的未来概念道具，只用于游戏展示",
    "设计一辆具有多材质外观的概念探索车",
    "设计一架具有多材质外观的概念飞行器",
    "设计一台具有多材质外观的展示型机械臂",
)
PBR_ROLES = {"base_color", "metallic_roughness", "normal", "occlusion", "emissive"}
PBR_TEXTURE_FIELDS = {
    "base_color": ("pbrMetallicRoughness", "baseColorTexture"),
    "metallic_roughness": ("pbrMetallicRoughness", "metallicRoughnessTexture"),
    "normal": (None, "normalTexture"),
    "occlusion": (None, "occlusionTexture"),
    "emissive": (None, "emissiveTexture"),
}


def _schema_validator() -> Draft202012Validator:
    schema = json.loads((SCHEMA_DIR / "geometry-compile-readback.schema.json").read_text(encoding="utf-8"))
    common = json.loads((SCHEMA_DIR / "common.schema.json").read_text(encoding="utf-8"))
    resolver = RefResolver.from_schema(schema, store={common["$id"]: common})
    return Draft202012Validator(schema, resolver=resolver)


def _glb_parts(payload: bytes) -> tuple[dict, bytearray]:
    offset = 12
    document = None
    binary = bytearray()
    while offset + 8 <= len(payload):
        length, kind = struct.unpack_from("<II", payload, offset)
        offset += 8
        chunk = payload[offset:offset + length]
        if kind == 0x4E4F534A:
            document = json.loads(chunk.rstrip(b" \x00").decode("utf-8"))
        elif kind == 0x004E4942:
            binary = bytearray(chunk)
        offset += length
    assert isinstance(document, dict) and binary
    return document, binary


def _glb_payload(document: dict, binary: bytearray) -> bytes:
    encoded = json.dumps(document, separators=(",", ":")).encode("utf-8")
    encoded += b" " * ((4 - len(encoded) % 4) % 4)
    binary.extend(b"\x00" * ((4 - len(binary) % 4) % 4))
    total = 12 + 8 + len(encoded) + 8 + len(binary)
    return (
        struct.pack("<4sII", b"glTF", 2, total)
        + struct.pack("<II", len(encoded), 0x4E4F534A)
        + encoded
        + struct.pack("<II", len(binary), 0x004E4942)
        + bytes(binary)
    )


def _expect_rejected(payload: bytes, fragment: str) -> None:
    try:
        read_shape_program_glb_facts(payload)
    except ValueError as exc:
        assert fragment in str(exc), exc
        return
    raise AssertionError("corrupted PBR GLB unexpectedly passed readback")


def _material_texture_reference(document: dict, material_index: int, role: str) -> dict:
    parent_field, texture_field = PBR_TEXTURE_FIELDS[role]
    material = document["materials"][material_index]
    parent = material[parent_field] if parent_field is not None else material
    reference = parent[texture_field]
    assert isinstance(reference, dict) and isinstance(reference.get("index"), int)
    return reference


def _remove_material_texture_reference(document: dict, material_index: int, role: str) -> None:
    parent_field, texture_field = PBR_TEXTURE_FIELDS[role]
    material = document["materials"][material_index]
    parent = material[parent_field] if parent_field is not None else material
    del parent[texture_field]


def _corrupt_material_texture_payload(
    document: dict,
    binary: bytearray,
    material_index: int,
    role: str,
) -> None:
    texture_index = int(_material_texture_reference(document, material_index, role)["index"])
    image_index = int(document["textures"][texture_index]["source"])
    view = document["bufferViews"][document["images"][image_index]["bufferView"]]
    binary[int(view.get("byteOffset", 0)) + 24] ^= 0x01


def _assert_asset(result, plan, validator: Draft202012Validator) -> None:
    again = build_blockout(
        plan,
        result.direction_id,
        variant_id=result.variant_id,
        presentation_profile="showcase",
    )
    assert result.glb_bytes == again.glb_bytes
    assert result.compile_readback.glb_sha256 == again.compile_readback.glb_sha256
    readback = result.compile_readback
    validator.validate(readback.model_dump(mode="json"))
    assert readback.glb_byte_size <= 2_000_000
    assert readback.visual_environment.environment_id == "env_forgecad_room_studio_v1"
    assert readback.visual_environment.color_workflow == "linear_srgb"
    assert readback.visual_environment.tone_mapping == "aces_filmic"
    assert readback.visual_environment.contact_shadows is True
    assert len(readback.material_zone_faces) >= 3
    assert len({item.material_zone_id for item in readback.material_zone_faces}) >= 3
    assert len(readback.visual_texture_sets) >= 3
    assert len({item.material_index for item in readback.visual_texture_sets}) >= 5
    assert any(item.edge_finish.mode == "bevel_approximation" for item in readback.surface_provenance)
    zones_by_material: dict[str, set[str]] = {}
    for zone in readback.material_zone_faces:
        zones_by_material.setdefault(zone.material_id, set()).add(zone.material_zone_id)
    for texture_set in readback.visual_texture_sets:
        assert set(texture_set.material_zone_ids) == zones_by_material[texture_set.material_id]
        assert {item.texture_role for item in texture_set.maps} == PBR_ROLES
        assert texture_set.texture_byte_size == sum(item.byte_size for item in texture_set.maps)
        assert all(item.fallback == "none" and item.byte_size <= 4_000_000 for item in texture_set.maps)
        assert {(item.width, item.height) for item in texture_set.maps} == {(128, 128)}
        assert {item.color_space for item in texture_set.maps if item.texture_role in {"base_color", "emissive"}} == {"srgb"}
        assert {item.color_space for item in texture_set.maps if item.texture_role not in {"base_color", "emissive"}} == {"linear"}

    document, binary = _glb_parts(result.glb_bytes)
    assert len(document["images"]) == len(document["textures"]) == builtin_visual_material_count() * len(PBR_ROLES)
    assert not any("uri" in image for image in document["images"])
    assert {"KHR_materials_clearcoat", "KHR_materials_transmission", "KHR_materials_ior"} <= set(document["extensionsUsed"])
    assert document["extras"]["forgecad_visual_environment"]["environment_sha256"] == readback.visual_environment.environment_sha256
    primitives = document["meshes"][0]["primitives"]
    assert {float(item["extras"].get("forgecad_visual_uv_repeat_mm", 0)) for item in primitives} == {320.0}
    used_material_indices = {int(item["material"]) for item in primitives}
    assert len(used_material_indices) >= 5
    material_by_role = {
        str(item["extras"]["forgecad_part_role"]): str(item["extras"]["forgecad_material_id"])
        for item in primitives
    }
    used_extensions = {
        extension
        for material_index in used_material_indices
        for extension in document["materials"][material_index].get("extensions", {})
    }
    if "mat_dark_glass" in material_by_role.values():
        assert {"KHR_materials_transmission", "KHR_materials_ior"} <= used_extensions
    if "mat_signal_red" in material_by_role.values():
        assert "KHR_materials_clearcoat" in used_extensions
    material_index_by_id = {
        str(material["extras"]["forgecad_texture_material_id"]): index
        for index, material in enumerate(document["materials"])
    }
    automotive_index = material_index_by_id["mat_automotive_paint"]
    aluminum_index = material_index_by_id["mat_aluminum"]
    automotive_material = document["materials"][automotive_index]
    aluminum_material = document["materials"][aluminum_index]
    assert automotive_index == 7 and automotive_index != aluminum_index
    assert automotive_material["extras"]["forgecad_visual_texture_set_id"] != aluminum_material["extras"]["forgecad_visual_texture_set_id"]
    assert automotive_material["pbrMetallicRoughness"]["baseColorTexture"]["index"] != aluminum_material["pbrMetallicRoughness"]["baseColorTexture"]["index"]
    assert automotive_material["extensions"]["KHR_materials_clearcoat"]["clearcoatFactor"] == 0.86
    assert "KHR_materials_clearcoat" not in aluminum_material.get("extensions", {})
    semantic_aliases = (
        (("wheel", "track", "tire", "grip", "foot"), "mat_rubber"),
        (("canopy", "cockpit", "glass", "window", "transparent"), "mat_dark_glass"),
        (("light", "lamp", "emissive"), "mat_emissive_blue"),
        (("joint", "rotor", "nacelle", "ring", "pivot", "wrist", "turntable"), "mat_aluminum"),
    )
    matched_aliases = 0
    for role, material_id in material_by_role.items():
        for tokens, expected_material_id in semantic_aliases:
            if any(token in role.lower() for token in tokens):
                assert material_id == expected_material_id, (role, material_id, expected_material_id)
                matched_aliases += 1
                break
    assert matched_aliases > 0
    if plan.domain_pack_id == "pack_future_weapon_prop":
        assert any(role.startswith("prop_") and material_id == "mat_signal_red" for role, material_id in material_by_role.items())
    elif plan.domain_pack_id == "pack_vehicle_concept":
        assert "mat_automotive_paint" in material_by_role.values()
        assert any(item.material_id == "mat_automotive_paint" and item.material_index == 7 for item in readback.visual_texture_sets)
        wheel_roles = [role for role in material_by_role if any(token in role for token in ("wheel", "track", "tire"))]
        if wheel_roles:
            assert all(material_by_role[role] == "mat_rubber" for role in wheel_roles)
    elif plan.domain_pack_id == "pack_aircraft_concept":
        canopy_roles = [role for role in material_by_role if any(token in role for token in ("canopy", "cockpit", "glass"))]
        if canopy_roles:
            assert all(material_by_role[role] == "mat_dark_glass" for role in canopy_roles)
    else:
        joint_roles = [role for role in material_by_role if any(token in role for token in ("joint", "pivot", "wrist", "turntable"))]
        assert joint_roles and all(material_by_role[role] == "mat_aluminum" for role in joint_roles)

    target_material_index = min(used_material_indices)
    for role in PBR_TEXTURE_FIELDS:
        missing_texture = copy.deepcopy(document)
        _remove_material_texture_reference(missing_texture, target_material_index, role)
        _expect_rejected(
            _glb_payload(missing_texture, bytearray(binary)),
            "PBR texture reference is invalid",
        )

        corrupt_texture = copy.deepcopy(document)
        corrupt_binary = bytearray(binary)
        _corrupt_material_texture_payload(corrupt_texture, corrupt_binary, target_material_index, role)
        _expect_rejected(
            _glb_payload(corrupt_texture, corrupt_binary),
            "hash or mime metadata is invalid",
        )

    wrong_normal_scale = copy.deepcopy(document)
    wrong_normal_scale["materials"][target_material_index]["normalTexture"]["scale"] = 0.99
    _expect_rejected(_glb_payload(wrong_normal_scale, bytearray(binary)), "built-in PBR truth")

    used_material_ids = {
        index: str(document["materials"][index]["extras"]["forgecad_texture_material_id"])
        for index in used_material_indices
    }
    dark_glass_indices = [index for index, material_id in used_material_ids.items() if material_id == "mat_dark_glass"]
    for dark_glass_index in dark_glass_indices:
        missing_ior = copy.deepcopy(document)
        del missing_ior["materials"][dark_glass_index]["extensions"]["KHR_materials_ior"]
        _expect_rejected(
            _glb_payload(missing_ior, bytearray(binary)),
            "transmission parameters do not match",
        )

        double_transparency = copy.deepcopy(document)
        double_transparency["materials"][dark_glass_index]["alphaMode"] = "BLEND"
        _expect_rejected(_glb_payload(double_transparency, bytearray(binary)), "transparent material compatibility")

    clearcoat_indices = [
        index
        for index in used_material_indices
        if "KHR_materials_clearcoat" in document["materials"][index].get("extensions", {})
    ]
    for clearcoat_index in clearcoat_indices:
        missing_clearcoat = copy.deepcopy(document)
        del missing_clearcoat["materials"][clearcoat_index]["extensions"]["KHR_materials_clearcoat"]
        _expect_rejected(
            _glb_payload(missing_clearcoat, bytearray(binary)),
            "clearcoat parameters do not match",
        )

        wrong_clearcoat = copy.deepcopy(document)
        clearcoat = wrong_clearcoat["materials"][clearcoat_index]["extensions"]["KHR_materials_clearcoat"]
        clearcoat["clearcoatFactor"] = float(clearcoat["clearcoatFactor"]) + 0.01
        _expect_rejected(
            _glb_payload(wrong_clearcoat, bytearray(binary)),
            "clearcoat parameters do not match",
        )


def _build_showcase_assets() -> list[tuple[str, bytes]]:
    planner = DeterministicMechanicalPlanner()
    assets: list[tuple[str, bytes]] = []
    for brief in BRIEFS:
        pack = domain_pack_for_message(brief)
        plan = planner.plan_complete_concept(
            brief=brief,
            pack=pack,
            project_id="prj_m108_smoke",
            action_loop_enabled=False,
        )
        for variant_id in ("a", "b", "c"):
            candidates = [item for item in list_blockout_variants(pack.pack_id) if item.endswith(f"_{variant_id}")]
            assert len(candidates) == 4, (pack.pack_id, variant_id, candidates)
            result = build_blockout(plan, plan.directions[0].direction_id, variant_id=candidates[0], presentation_profile="showcase")
            assets.append((f"{pack.pack_id}:{candidates[0]}", result.glb_bytes))
    return assets


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--emit-validator-fixtures",
        action="store_true",
        help="write one deterministic showcase GLB per domain as base64 JSON for the external standard validator smoke",
    )
    args = parser.parse_args()
    assets = _build_showcase_assets()
    if args.emit_validator_fixtures:
        # Keep the external validator independent of ForgeCAD readback: it
        # receives raw bytes for one representative asset in each domain.
        fixtures = [
            {"fixture_id": fixture_id, "glb_base64": base64.b64encode(glb_bytes).decode("ascii")}
            for fixture_id, glb_bytes in assets[::3]
        ]
        assert len(fixtures) == 4
        print(json.dumps(fixtures, separators=(",", ":")))
        return 0

    validator = _schema_validator()
    asset_count = 0
    for fixture_id, glb_bytes in assets:
        # The plan is deterministic and this smoke exercises all domain
        # variants above; reconstruct only the result needed by the existing
        # assertion helper without introducing a second compile path.
        pack_id, candidate = fixture_id.split(":", 1)
        brief = next(item for item in BRIEFS if domain_pack_for_message(item).pack_id == pack_id)
        pack = domain_pack_for_message(brief)
        plan = DeterministicMechanicalPlanner().plan_complete_concept(
            brief=brief,
            pack=pack,
            project_id="prj_m108_smoke",
            action_loop_enabled=False,
        )
        result = build_blockout(plan, plan.directions[0].direction_id, variant_id=candidate, presentation_profile="showcase")
        assert result.glb_bytes == glb_bytes
        _assert_asset(result, plan, validator)
        asset_count += 1
    assert asset_count == 12
    print("M108 visual PBR smoke passed: 12 multi-zone assets across four domains use one embedded GLB/readback texture truth")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
