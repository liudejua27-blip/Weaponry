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


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_DIR = ROOT / "packages" / "concept-spec" / "schemas"
BRIEFS = (
    "设计一个具有多材质外观的未来概念道具，只用于游戏展示",
    "设计一辆具有多材质外观的概念探索车",
    "设计一架具有多材质外观的概念飞行器",
    "设计一台具有多材质外观的展示型机械臂",
)
PBR_ROLES = {"base_color", "metallic_roughness", "normal", "occlusion", "emissive"}


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
    assert len(readback.visual_texture_sets) >= 3
    zones_by_material: dict[str, set[str]] = {}
    for zone in readback.material_zone_faces:
        zones_by_material.setdefault(zone.material_id, set()).add(zone.material_zone_id)
    for texture_set in readback.visual_texture_sets:
        assert set(texture_set.material_zone_ids) == zones_by_material[texture_set.material_id]
        assert {item.texture_role for item in texture_set.maps} == PBR_ROLES
        assert texture_set.texture_byte_size == sum(item.byte_size for item in texture_set.maps)
        assert all(item.fallback == "none" and item.byte_size <= 4_000_000 for item in texture_set.maps)
        assert {item.color_space for item in texture_set.maps if item.texture_role in {"base_color", "emissive"}} == {"srgb"}
        assert {item.color_space for item in texture_set.maps if item.texture_role not in {"base_color", "emissive"}} == {"linear"}

    document, binary = _glb_parts(result.glb_bytes)
    assert len(document["images"]) == len(document["textures"]) == 35
    assert not any("uri" in image for image in document["images"])
    assert {"KHR_materials_clearcoat", "KHR_materials_transmission", "KHR_materials_ior"} <= set(document["extensionsUsed"])
    assert document["extras"]["forgecad_visual_environment"]["environment_sha256"] == readback.visual_environment.environment_sha256

    missing_normal = copy.deepcopy(document)
    del missing_normal["materials"][0]["normalTexture"]
    _expect_rejected(_glb_payload(missing_normal, bytearray(binary)), "PBR texture reference is invalid")

    corrupt_document = copy.deepcopy(document)
    view = corrupt_document["bufferViews"][corrupt_document["images"][0]["bufferView"]]
    corrupt_binary = bytearray(binary)
    corrupt_binary[int(view.get("byteOffset", 0)) + 24] ^= 0x01
    _expect_rejected(_glb_payload(corrupt_document, corrupt_binary), "hash or mime metadata is invalid")


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
