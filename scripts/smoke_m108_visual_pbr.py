#!/usr/bin/env python3
"""FGC-M108: same-GLB visual PBR, zone mapping and studio readback gate."""

from __future__ import annotations

import argparse
import base64
import copy
import hashlib
import json
import math
import struct
import warnings
import zlib
from pathlib import Path

warnings.simplefilter("ignore", DeprecationWarning)

from jsonschema import Draft202012Validator, RefResolver  # noqa: E402

from forgecad_agent.application import visual_texture_sets as visual_texture_sets_module  # noqa: E402
from forgecad_agent.application.agent_models import AgentAssetQualityReport  # noqa: E402
from forgecad_agent.application.domain_packs import domain_pack_for_message  # noqa: E402
from forgecad_agent.application.geometry_models import (  # noqa: E402
    GeometryCompileReadback,
    GeometryVisualTextureSetReadback,
)
from forgecad_agent.application.geometry_worker import (  # noqa: E402
    build_blockout,
    list_blockout_variants,
    read_shape_program_glb_facts,
)
from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner  # noqa: E402
from forgecad_agent.application.visual_texture_sets import (  # noqa: E402
    builtin_visual_material_count,
    builtin_visual_texture_cache_facts,
    legacy_builtin_visual_texture_sets,
    visual_texture_png_bytes,
)


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
RADIAL_TRIANGLES_BY_OPERATION = {"cylinder": 96, "capsule": 432}
LEGACY_V1_TEXTURE_AGGREGATE_SHA256 = "0b4701fe31946dfc9572990daa5e1e9260d05ddcfcfdef640c9eac776e10b62f"


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


def _visual_aabb_components(
    document: dict,
    binary: bytearray,
    *,
    contact_tolerance_m: float = 0.001,
) -> tuple[tuple[str, ...], ...]:
    """Return final-GLB visual AABB components without implying solid joints."""

    accessors = document.get("accessors")
    views = document.get("bufferViews")
    assert isinstance(accessors, list) and isinstance(views, list)
    assert len(document.get("meshes", [])) == 1
    assert type(document.get("scene")) is int and document["scene"] == 0
    scenes = document.get("scenes")
    assert isinstance(scenes, list) and len(scenes) == 1 and isinstance(scenes[0], dict)
    scene_nodes = scenes[0].get("nodes")
    assert (
        isinstance(scene_nodes, list)
        and len(scene_nodes) == 1
        and type(scene_nodes[0]) is int
        and scene_nodes[0] == 0
    )
    nodes = document.get("nodes")
    assert (
        isinstance(nodes, list)
        and len(nodes) == 1
        and type(nodes[0].get("mesh")) is int
        and nodes[0]["mesh"] == 0
    )
    assert not any(
        key in nodes[0]
        for key in ("children", "extensions", "matrix", "rotation", "scale", "skin", "translation", "weights")
    )
    records: list[tuple[str, tuple[float, float, float], tuple[float, float, float]]] = []
    for primitive_index, primitive in enumerate(document["meshes"][0]["primitives"]):
        accessor_index = primitive["attributes"]["POSITION"]
        assert (
            type(accessor_index) is int
            and 0 <= accessor_index < len(accessors)
            and isinstance(accessors[accessor_index], dict)
        ), (primitive_index, "POSITION accessor index is invalid")
        accessor = accessors[accessor_index]
        assert accessor["componentType"] == 5126 and accessor["type"] == "VEC3"
        assert accessor.get("sparse") is None
        view_index = accessor.get("bufferView")
        assert (
            type(view_index) is int
            and 0 <= view_index < len(views)
            and isinstance(views[view_index], dict)
        ), (primitive_index, "POSITION bufferView index is invalid")
        view = views[view_index]
        assert type(view.get("buffer")) is int and view["buffer"] == 0
        count = accessor.get("count")
        accessor_offset = accessor.get("byteOffset", 0)
        view_offset = view.get("byteOffset", 0)
        view_length = view.get("byteLength")
        explicit_stride = view.get("byteStride")
        stride = 12 if explicit_stride is None else explicit_stride
        assert type(count) is int and count > 0
        assert type(accessor_offset) is int and accessor_offset >= 0
        assert type(view_offset) is int and view_offset >= 0
        assert type(view_length) is int and view_length >= 0
        if explicit_stride is not None:
            assert (
                type(explicit_stride) is int
                and 4 <= explicit_stride <= 252
                and explicit_stride % 4 == 0
            )
        assert type(stride) is int and stride >= 12 and stride % 4 == 0
        base = view_offset + accessor_offset
        end = base + (count - 1) * stride + 12
        view_end = view_offset + view_length
        assert accessor_offset % 4 == 0 and view_offset % 4 == 0 and base % 4 == 0
        assert view_end <= len(binary), (primitive_index, "POSITION bufferView exceeds BIN bytes")
        assert end <= view_end, (primitive_index, "POSITION accessor exceeds its bufferView")
        positions = [
            struct.unpack_from("<3f", binary, base + index * stride)
            for index in range(count)
        ]
        assert all(math.isfinite(value) for position in positions for value in position)
        minimum = tuple(min(position[axis] for position in positions) for axis in range(3))
        maximum = tuple(max(position[axis] for position in positions) for axis in range(3))
        assert all(
            math.isclose(float(accessor["min"][axis]), minimum[axis], rel_tol=1e-6, abs_tol=1e-7)
            and math.isclose(float(accessor["max"][axis]), maximum[axis], rel_tol=1e-6, abs_tol=1e-7)
            for axis in range(3)
        ), (primitive_index, "declared POSITION bounds diverge from BIN bytes")
        role = str(primitive["extras"]["forgecad_part_role"])
        records.append((f"{role}#{primitive_index}", minimum, maximum))

    def touches(left_index: int, right_index: int) -> bool:
        _, left_min, left_max = records[left_index]
        _, right_min, right_max = records[right_index]
        return all(
            max(left_min[axis], right_min[axis])
            <= min(left_max[axis], right_max[axis]) + contact_tolerance_m
            for axis in range(3)
        )

    remaining = set(range(len(records)))
    components: list[tuple[str, ...]] = []
    while remaining:
        seed = min(remaining)
        remaining.remove(seed)
        stack = [seed]
        members: list[int] = []
        while stack:
            current = stack.pop()
            members.append(current)
            connected = {
                candidate
                for candidate in remaining
                if touches(current, candidate)
            }
            remaining.difference_update(connected)
            stack.extend(sorted(connected, reverse=True))
        components.append(tuple(records[index][0] for index in sorted(members)))
    return tuple(components)


def _assert_glb_readback_contract_self_test(document: dict, binary: bytearray) -> None:
    forged_document = copy.deepcopy(document)
    forged_binary = bytearray(binary)
    primitive = forged_document["meshes"][0]["primitives"][0]
    accessor = forged_document["accessors"][primitive["attributes"]["POSITION"]]
    view = forged_document["bufferViews"][accessor["bufferView"]]
    offset = int(view.get("byteOffset", 0)) + int(accessor.get("byteOffset", 0))
    position = list(struct.unpack_from("<3f", forged_binary, offset))
    position[0] += 100.0
    struct.pack_into("<3f", forged_binary, offset, *position)
    _expect_rejected(
        _glb_payload(forged_document, forged_binary),
        "declared position bounds do not match POSITION bytes",
    )

    truncated_document = copy.deepcopy(document)
    primitive = truncated_document["meshes"][0]["primitives"][0]
    accessor = truncated_document["accessors"][primitive["attributes"]["POSITION"]]
    truncated_document["bufferViews"][accessor["bufferView"]]["byteLength"] = 12
    _expect_rejected(
        _glb_payload(truncated_document, bytearray(binary)),
        "accessor exceeds its bufferView",
    )

    negative_index_document = copy.deepcopy(document)
    primitive = negative_index_document["meshes"][0]["primitives"][0]
    primitive["attributes"]["POSITION"] = -len(negative_index_document["accessors"])
    _expect_rejected(
        _glb_payload(negative_index_document, bytearray(binary)),
        "accessor index is invalid",
    )

    missing_buffer_document = copy.deepcopy(document)
    primitive = missing_buffer_document["meshes"][0]["primitives"][0]
    accessor = missing_buffer_document["accessors"][primitive["attributes"]["POSITION"]]
    del missing_buffer_document["bufferViews"][accessor["bufferView"]]["buffer"]
    _expect_rejected(
        _glb_payload(missing_buffer_document, bytearray(binary)),
        "accessor references an unsupported buffer",
    )

    bad_stride_document = copy.deepcopy(document)
    primitive = bad_stride_document["meshes"][0]["primitives"][0]
    index_accessor = bad_stride_document["accessors"][primitive["indices"]]
    bad_stride_document["bufferViews"][index_accessor["bufferView"]]["byteStride"] = 2
    _expect_rejected(
        _glb_payload(bad_stride_document, bytearray(binary)),
        "explicit byteStride is outside the glTF range",
    )

    for bad_stride in (6, 256):
        bad_stride_document = copy.deepcopy(document)
        primitive = bad_stride_document["meshes"][0]["primitives"][0]
        index_accessor = bad_stride_document["accessors"][primitive["indices"]]
        bad_stride_document["bufferViews"][index_accessor["bufferView"]]["byteStride"] = bad_stride
        _expect_rejected(
            _glb_payload(bad_stride_document, bytearray(binary)),
            "explicit byteStride is outside the glTF range",
        )

    misaligned_document = copy.deepcopy(document)
    primitive = misaligned_document["meshes"][0]["primitives"][0]
    accessor = misaligned_document["accessors"][primitive["attributes"]["POSITION"]]
    accessor["byteOffset"] = 2
    _expect_rejected(
        _glb_payload(misaligned_document, bytearray(binary)),
        "byte stride or alignment is invalid",
    )

    misaligned_view_document = copy.deepcopy(document)
    primitive = misaligned_view_document["meshes"][0]["primitives"][0]
    accessor = misaligned_view_document["accessors"][primitive["attributes"]["POSITION"]]
    misaligned_view_document["bufferViews"][accessor["bufferView"]]["byteOffset"] = 2
    _expect_rejected(
        _glb_payload(misaligned_view_document, bytearray(binary)),
        "byte stride or alignment is invalid",
    )

    instanced_document = copy.deepcopy(document)
    instanced_document["nodes"].append({"mesh": 0, "translation": [100.0, 0.0, 0.0]})
    instanced_document["scenes"][0]["nodes"].append(1)
    _expect_rejected(
        _glb_payload(instanced_document, bytearray(binary)),
        "single-mesh static scene contract",
    )

    transformed_document = copy.deepcopy(document)
    transformed_document["nodes"][0]["translation"] = [100.0, 0.0, 0.0]
    _expect_rejected(
        _glb_payload(transformed_document, bytearray(binary)),
        "static scene node must be untransformed and uninstanced",
    )

    bool_node_document = copy.deepcopy(document)
    bool_node_document["scenes"][0]["nodes"] = [False]
    _expect_rejected(
        _glb_payload(bool_node_document, bytearray(binary)),
        "single-mesh static scene contract",
    )

    float_node_document = copy.deepcopy(document)
    float_node_document["scenes"][0]["nodes"] = [0.0]
    _expect_rejected(
        _glb_payload(float_node_document, bytearray(binary)),
        "single-mesh static scene contract",
    )

    second_mesh_document = copy.deepcopy(document)
    second_mesh_document["meshes"].append(copy.deepcopy(second_mesh_document["meshes"][0]))
    _expect_rejected(
        _glb_payload(second_mesh_document, bytearray(binary)),
        "single-mesh static scene contract",
    )

    second_scene_document = copy.deepcopy(document)
    second_scene_document["scenes"].append({"nodes": [0]})
    _expect_rejected(
        _glb_payload(second_scene_document, bytearray(binary)),
        "single-mesh static scene contract",
    )

    missing_image_buffer_document = copy.deepcopy(document)
    image_view_index = missing_image_buffer_document["images"][0]["bufferView"]
    del missing_image_buffer_document["bufferViews"][image_view_index]["buffer"]
    _expect_rejected(
        _glb_payload(missing_image_buffer_document, bytearray(binary)),
        "image view must explicitly reference the embedded buffer",
    )

    external_image_buffer_document = copy.deepcopy(document)
    image_view_index = external_image_buffer_document["images"][0]["bufferView"]
    external_image_buffer_document["bufferViews"][image_view_index]["buffer"] = 1
    _expect_rejected(
        _glb_payload(external_image_buffer_document, bytearray(binary)),
        "image view must explicitly reference the embedded buffer",
    )

    for invalid_offset in (False, 0.0, "0"):
        invalid_image_offset_document = copy.deepcopy(document)
        image_view_index = invalid_image_offset_document["images"][0]["bufferView"]
        invalid_image_offset_document["bufferViews"][image_view_index]["byteOffset"] = invalid_offset
        _expect_rejected(
            _glb_payload(invalid_image_offset_document, bytearray(binary)),
            "PBR image exceeds its binary buffer",
        )


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


def _self_report_corrupt_material_texture_payload(
    document: dict,
    binary: bytearray,
    material_index: int,
    role: str,
) -> None:
    """Corrupt bytes and update their self-reported hash like a hostile GLB."""

    texture_index = int(_material_texture_reference(document, material_index, role)["index"])
    image_index = int(document["textures"][texture_index]["source"])
    image = document["images"][image_index]
    view = document["bufferViews"][image["bufferView"]]
    offset = int(view.get("byteOffset", 0))
    length = int(view["byteLength"])
    binary[offset + 24] ^= 0x01
    payload = bytes(binary[offset:offset + length])
    image["extras"]["forgecad_visual_texture"]["sha256"] = hashlib.sha256(payload).hexdigest()


def _embedded_png(document: dict, binary: bytearray, material_index: int, role: str) -> bytes:
    texture_index = int(_material_texture_reference(document, material_index, role)["index"])
    image_index = int(document["textures"][texture_index]["source"])
    image = document["images"][image_index]
    view = document["bufferViews"][image["bufferView"]]
    offset = int(view.get("byteOffset", 0))
    length = int(view["byteLength"])
    return bytes(binary[offset:offset + length])


def _legacy_v1_texture_aggregate_sha256() -> str:
    digest = hashlib.sha256()
    for texture_set in legacy_builtin_visual_texture_sets():
        digest.update(texture_set.visual_texture_set_id.encode("utf-8"))
        for texture_map in texture_set.maps:
            digest.update(texture_map.texture_id.encode("utf-8"))
            digest.update(visual_texture_png_bytes(texture_map.texture_id))
    return digest.hexdigest()


def _rewrite_visual_textures_as_legacy_v1(
    document: dict,
    binary: bytearray,
    *,
    material_indices: tuple[int, ...] | None = None,
) -> tuple[dict, bytearray]:
    """Rewrite selected current GLB materials with exact immutable v1 payloads."""

    legacy_sets = legacy_builtin_visual_texture_sets()
    selected_indices = (
        tuple(range(len(legacy_sets)))
        if material_indices is None
        else material_indices
    )
    assert selected_indices and len(selected_indices) == len(set(selected_indices))
    assert all(0 <= index < len(legacy_sets) for index in selected_indices)
    for material_index in selected_indices:
        texture_set = legacy_sets[material_index]
        material = document["materials"][material_index]
        material["extras"]["forgecad_visual_texture_set_id"] = texture_set.visual_texture_set_id
        for texture_map in texture_set.maps:
            texture_index = int(
                _material_texture_reference(
                    document,
                    material_index,
                    texture_map.texture_role,
                )["index"]
            )
            texture = document["textures"][texture_index]
            image = document["images"][int(texture["source"])]
            view = document["bufferViews"][int(image["bufferView"])]
            payload = visual_texture_png_bytes(texture_map.texture_id)
            binary.extend(b"\x00" * ((4 - len(binary) % 4) % 4))
            view["byteOffset"] = len(binary)
            view["byteLength"] = len(payload)
            binary.extend(payload)
            texture["name"] = texture_map.texture_id
            image["name"] = texture_map.texture_id
            image["extras"]["forgecad_visual_texture"] = texture_map.model_dump(mode="json")
    binary.extend(b"\x00" * ((4 - len(binary) % 4) % 4))
    document["buffers"][0]["byteLength"] = len(binary)
    return document, binary


def _texture_cache_entry_count() -> int:
    """Inspect the bounded byte cache without eagerly resolving both versions."""

    return visual_texture_sets_module._cached_texture_set_bytes.cache_info().currsize


def _assert_legacy_persisted_quality_hydration(
    result,
    legacy_facts,
    legacy_glb: bytes,
) -> None:
    """Hydrate a pre-field v1 compile readback and its persisted quality owner."""

    compile_payload = result.compile_readback.model_dump(mode="json")
    compile_payload["glb_sha256"] = hashlib.sha256(legacy_glb).hexdigest()
    compile_payload["glb_byte_size"] = len(legacy_glb)
    compile_payload["visual_texture_sets"] = copy.deepcopy(
        legacy_facts.visual_texture_sets
    )
    for texture_set in compile_payload["visual_texture_sets"]:
        assert texture_set["visual_texture_set_id"].endswith("_builtin")
        texture_set.pop("texture_material_id")

    migrated_compile = GeometryCompileReadback.model_validate(compile_payload)
    assert all(
        texture_set.texture_material_id.startswith("mat_")
        for texture_set in migrated_compile.visual_texture_sets
    )
    quality_payload = {
        "schema_version": "AgentAssetQualityReport@1",
        "quality_report_id": "quality_m108_legacy_v1_persisted",
        "asset_version_id": "assetver_m108_legacy_v1_persisted",
        "status": "passed",
        "triangle_count": migrated_compile.triangle_count,
        "bounds_mm": migrated_compile.bounds_mm,
        "evidence_source": "geometry_compile_readback",
        "compile_readback": compile_payload,
        "findings": [],
        "checked_at": "2026-07-16T00:00:00Z",
    }
    migrated_quality = AgentAssetQualityReport.model_validate_json(
        json.dumps(quality_payload, separators=(",", ":"))
    )
    assert migrated_quality.compile_readback is not None
    assert migrated_quality.compile_readback.glb_sha256 == hashlib.sha256(
        legacy_glb
    ).hexdigest()
    assert all(
        texture_set.visual_texture_set_id.endswith("_builtin")
        and texture_set.texture_material_id.startswith("mat_")
        for texture_set in migrated_quality.compile_readback.visual_texture_sets
    )


def _png_rgb_pixels(payload: bytes) -> tuple[int, int, tuple[tuple[int, int, int], ...]]:
    assert payload[:8] == b"\x89PNG\r\n\x1a\n"
    offset = 8
    width = height = 0
    compressed = bytearray()
    while offset + 12 <= len(payload):
        length = struct.unpack_from(">I", payload, offset)[0]
        kind = payload[offset + 4:offset + 8]
        data = payload[offset + 8:offset + 8 + length]
        if kind == b"IHDR":
            width, height, bit_depth, color_type, compression, filtering, interlace = struct.unpack(
                ">IIBBBBB", data
            )
            assert (bit_depth, color_type, compression, filtering, interlace) == (8, 2, 0, 0, 0)
        elif kind == b"IDAT":
            compressed.extend(data)
        elif kind == b"IEND":
            break
        offset += 12 + length
    raw = zlib.decompress(bytes(compressed))
    stride = width * 3
    assert len(raw) == height * (stride + 1)
    pixels: list[tuple[int, int, int]] = []
    for row_index in range(height):
        row_offset = row_index * (stride + 1)
        assert raw[row_offset] == 0
        row = raw[row_offset + 1:row_offset + 1 + stride]
        pixels.extend(tuple(row[index:index + 3]) for index in range(0, len(row), 3))
    return width, height, tuple(pixels)


def _texture_grid_artifact_status(
    width: int,
    height: int,
    pixels: tuple[tuple[int, int, int], ...],
) -> str | None:
    """Reject hard periodic cell boundaries at any pixel phase."""

    def pixel_delta(left: tuple[int, int, int], right: tuple[int, int, int]) -> float:
        return sum(abs(left[channel] - right[channel]) for channel in range(3)) / 3

    edge_profiles = (
        tuple(
            sum(
                pixel_delta(
                    pixels[y * width + x - 1],
                    pixels[y * width + x],
                )
                for y in range(height)
            )
            / height
            for x in range(1, width)
        ),
        tuple(
            sum(
                pixel_delta(
                    pixels[(y - 1) * width + x],
                    pixels[y * width + x],
                )
                for x in range(width)
            )
            / width
            for y in range(1, height)
        ),
    )
    for grid_size in (8, 12, 16, 18, 28, 32):
        for profile in edge_profiles:
            extent = len(profile) + 1
            for phase in range(grid_size):
                coordinates = tuple(
                    coordinate
                    for coordinate in range(1, extent)
                    if coordinate % grid_size == phase
                )
                if not coordinates:
                    continue
                on_grid = tuple(profile[coordinate - 1] for coordinate in coordinates)
                adjacent = tuple(
                    profile[nearby - 1]
                    for coordinate in coordinates
                    for nearby in (coordinate - 1, coordinate + 1)
                    if 1 <= nearby < extent
                )
                if not adjacent:
                    continue
                on_mean = sum(on_grid) / len(on_grid)
                adjacent_mean = sum(adjacent) / len(adjacent)
                # A one-level quantized step is still visible when repeated,
                # while smooth periodic gradients spread into adjacent edges.
                if (
                    on_mean - adjacent_mean > 0.75
                    and on_mean / max(adjacent_mean, 0.001) > 1.5
                ):
                    return "PBR_TEXTURE_GRID_ARTIFACT"
    return None


def _texture_variation_status(
    width: int,
    height: int,
    pixels: tuple[tuple[int, int, int], ...],
) -> str | None:
    """Require microdetail only for channels whose role needs it."""

    del width, height
    if len(set(pixels)) < 3:
        return "PBR_TEXTURE_FLAT"
    return None


def _assert_texture_microdetail_gate_self_test() -> None:
    width = height = 64
    flat = tuple((80, 80, 80) for _ in range(width * height))
    smooth = tuple(
        (80 + round(3 * math.sin(2 * math.pi * x / 16)),) * 3
        for y in range(height)
        for x in range(width)
    )
    assert _texture_grid_artifact_status(width, height, flat) is None
    assert _texture_variation_status(width, height, flat) == "PBR_TEXTURE_FLAT"
    assert _texture_grid_artifact_status(width, height, smooth) is None
    assert _texture_variation_status(width, height, smooth) is None
    for grid_size in (8, 12, 16, 18, 28, 32):
        for phase in range(grid_size):
            stepped = tuple(
                (
                    40
                    if (
                        (x + phase) // grid_size
                        + (y + phase) // grid_size
                    )
                    % 2
                    else 80,
                )
                * 3
                for y in range(height)
                for x in range(width)
            )
            assert _texture_grid_artifact_status(
                width,
                height,
                stepped,
            ) == "PBR_TEXTURE_GRID_ARTIFACT", (grid_size, phase)

    for axis in (0, 1):
        stepped = tuple(
            (
                79
                if (((x, y)[axis] + 5) // 16) % 2
                else 80,
            )
            * 3
            for y in range(height)
            for x in range(width)
        )
        assert _texture_grid_artifact_status(
            width,
            height,
            stepped,
        ) == "PBR_TEXTURE_GRID_ARTIFACT", axis


def _assert_asset(result, plan, validator: Draft202012Validator) -> set[int]:
    legacy_readback_indices: set[int] = set()
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
    incomplete_v2_texture_set = readback.visual_texture_sets[0].model_dump(mode="json")
    incomplete_v2_texture_set.pop("texture_material_id")
    try:
        GeometryVisualTextureSetReadback.model_validate(incomplete_v2_texture_set)
    except ValueError:
        pass
    else:
        raise AssertionError("current v2 texture readback accepted a missing canonical material field")
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

    # The same GLB readback that owns quality facts must prove the fixed M108
    # radial-surface baseline.  This prevents a renderer-only smoothing trick
    # or an unchanged 16-sided asset from masquerading as a fidelity upgrade.
    radial_operation_by_role = {
        str(item["args"]["part_role"]): str(item["op"])
        for item in result.shape_program["operations"]
        if item["op"] in RADIAL_TRIANGLES_BY_OPERATION
    }
    readback_triangles_by_role = {
        item.part_role: sum(surface_range.triangle_count for surface_range in item.surface_ranges)
        for item in readback.surface_provenance
    }
    assert radial_operation_by_role
    assert all(
        readback_triangles_by_role[role] == RADIAL_TRIANGLES_BY_OPERATION[operation]
        for role, operation in radial_operation_by_role.items()
    )

    document, binary = _glb_parts(result.glb_bytes)
    visual_components = _visual_aabb_components(document, binary)
    assert len(visual_components) == 1, (
        plan.domain_pack_id,
        result.variant_id,
        "showcase contains visually disconnected final-GLB AABB components",
        visual_components,
    )
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
    connection_tokens = (
        "_mount_collar",
        "_side_bridge_",
        "_rotor_pylon_",
        "_shoulder_bridge",
        "_tilt_pod_bridge_",
        "_wrist_bridge",
        "_rail_bridge",
        "_carriage_bridge",
    )
    connection_roles = {
        role for role in material_by_role
        if any(token in role for token in connection_tokens)
    }
    if connection_roles:
        connection_surfaces = {
            item.part_role: item
            for item in readback.surface_provenance
            if item.part_role in connection_roles
        }
        assert set(connection_surfaces) == connection_roles
        assert all(
            item.closed
            and item.boundary_edge_count == 0
            and item.non_manifold_edge_count == 0
            and item.degenerate_triangle_count == 0
            and item.uv_degenerate_triangle_count == 0
            and item.tangent_fallback_triangle_count == 0
            and item.texture_ready
            for item in connection_surfaces.values()
        )
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
    assert automotive_material["extras"]["forgecad_visual_texture_set_id"].endswith("_builtin_v2")
    assert all(
        str(material["extras"]["forgecad_visual_texture_set_id"]).endswith("_builtin_v2")
        for material in document["materials"]
    )
    assert all(
        item.texture_material_id
        == document["materials"][item.material_index]["extras"]["forgecad_texture_material_id"]
        for item in readback.visual_texture_sets
    )
    for material_index in range(builtin_visual_material_count()):
        for texture_role in PBR_TEXTURE_FIELDS:
            width, height, pixels = _png_rgb_pixels(
                _embedded_png(document, binary, material_index, texture_role)
            )
            assert _texture_grid_artifact_status(width, height, pixels) is None, (
                material_index,
                texture_role,
            )
            if texture_role in {"metallic_roughness", "normal"}:
                assert _texture_variation_status(width, height, pixels) is None, (
                    material_index,
                    texture_role,
                )
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

    legacy_fixture = (
        plan.domain_pack_id == "pack_future_weapon_prop"
        and result.variant_id == "compact_prop_a"
    ) or (
        plan.domain_pack_id == "pack_vehicle_concept"
        and result.variant_id == "urban_scout_a"
    )
    if legacy_fixture:
        if plan.domain_pack_id == "pack_future_weapon_prop":
            assert (
                _legacy_v1_texture_aggregate_sha256()
                == LEGACY_V1_TEXTURE_AGGREGATE_SHA256
            )
        legacy_document, legacy_binary = _rewrite_visual_textures_as_legacy_v1(
            copy.deepcopy(document),
            bytearray(binary),
        )
        legacy_glb = _glb_payload(legacy_document, legacy_binary)
        legacy_facts = read_shape_program_glb_facts(legacy_glb)
        assert legacy_facts.visual_texture_sets
        fixture_legacy_indices = {
            int(item["material_index"])
            for item in legacy_facts.visual_texture_sets
        }
        assert fixture_legacy_indices == used_material_indices
        legacy_readback_indices.update(fixture_legacy_indices)
        assert all(
            item["visual_texture_set_id"].endswith("_builtin")
            and not item["visual_texture_set_id"].endswith("_builtin_v2")
            for item in legacy_facts.visual_texture_sets
        )
        assert all(
            "_v2_" not in texture_map["texture_id"]
            for item in legacy_facts.visual_texture_sets
            for texture_map in item["maps"]
        )
        if plan.domain_pack_id == "pack_future_weapon_prop":
            legacy_texture_set = copy.deepcopy(legacy_facts.visual_texture_sets[0])
            legacy_texture_set.pop("texture_material_id")
            migrated_texture_set = GeometryVisualTextureSetReadback.model_validate(
                legacy_texture_set
            )
            assert (
                migrated_texture_set.texture_material_id
                == legacy_facts.visual_texture_sets[0]["texture_material_id"]
            )
            forged_legacy_binding = copy.deepcopy(legacy_texture_set)
            forged_legacy_binding["material_id"] = (
                "mat_rubber"
                if migrated_texture_set.texture_material_id != "mat_rubber"
                else "mat_aluminum"
            )
            try:
                GeometryVisualTextureSetReadback.model_validate(
                    forged_legacy_binding
                )
            except ValueError:
                pass
            else:
                raise AssertionError(
                    "forged legacy authored-to-canonical material binding was migrated"
                )

            mixed_index = min(used_material_indices)
            mixed_document, mixed_binary = _rewrite_visual_textures_as_legacy_v1(
                copy.deepcopy(document),
                bytearray(binary),
                material_indices=(mixed_index,),
            )
            _expect_rejected(
                _glb_payload(mixed_document, mixed_binary),
                "mix incompatible built-in visual texture contract versions",
            )
        else:
            assert {5, 7}.issubset(fixture_legacy_indices)
            _assert_legacy_persisted_quality_hydration(
                result,
                legacy_facts,
                legacy_glb,
            )

    if plan.domain_pack_id == "pack_aircraft_concept" and result.variant_id == "vertical_takeoff_b":
        _assert_glb_readback_contract_self_test(document, binary)

    target_material_index = min(used_material_indices)
    wrong_texture_set = copy.deepcopy(document)
    wrong_texture_set["materials"][target_material_index]["extras"][
        "forgecad_visual_texture_set_id"
    ] += "_forged"
    _expect_rejected(
        _glb_payload(wrong_texture_set, bytearray(binary)),
        "VisualTextureSet identity",
    )

    forged_primitive_material = copy.deepcopy(document)
    forged_primitive_material["meshes"][0]["primitives"][0]["extras"][
        "forgecad_material_id"
    ] = "mat_forged_alias"
    _expect_rejected(
        _glb_payload(forged_primitive_material, bytearray(binary)),
        "outside the reviewed visual catalog",
    )

    known_material_wrong_index = copy.deepcopy(document)
    known_primitive = known_material_wrong_index["meshes"][0]["primitives"][0]
    known_material_index = int(known_primitive["material"])
    known_primitive["extras"]["forgecad_material_id"] = (
        "mat_dark_glass" if known_material_index != 5 else "mat_aluminum"
    )
    _expect_rejected(
        _glb_payload(known_material_wrong_index, bytearray(binary)),
        "canonical texture material",
    )

    sampled_texture_index = int(
        _material_texture_reference(
            document,
            target_material_index,
            "base_color",
        )["index"]
    )
    wrong_sampler = copy.deepcopy(document)
    wrong_sampler["samplers"] = [{"magFilter": 9728, "minFilter": 9728, "wrapS": 33071, "wrapT": 33071}]
    wrong_sampler["textures"][sampled_texture_index]["sampler"] = 0
    _expect_rejected(
        _glb_payload(wrong_sampler, bytearray(binary)),
        "sampling state",
    )

    wrong_texture_transform = copy.deepcopy(document)
    _material_texture_reference(
        wrong_texture_transform,
        target_material_index,
        "base_color",
    )["extensions"] = {"KHR_texture_transform": {"offset": [0.25, 0]}}
    _expect_rejected(
        _glb_payload(wrong_texture_transform, bytearray(binary)),
        "sampling state",
    )

    boolean_texture_index = copy.deepcopy(document)
    _material_texture_reference(
        boolean_texture_index,
        target_material_index,
        "base_color",
    )["index"] = False
    _expect_rejected(
        _glb_payload(boolean_texture_index, bytearray(binary)),
        "texture reference is invalid",
    )
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

        self_report_corrupt = copy.deepcopy(document)
        self_report_binary = bytearray(binary)
        _self_report_corrupt_material_texture_payload(
            self_report_corrupt,
            self_report_binary,
            target_material_index,
            role,
        )
        _expect_rejected(
            _glb_payload(self_report_corrupt, self_report_binary),
            "built-in texture truth",
        )

    nonzero_target_material_index = (
        automotive_index if automotive_index in used_material_indices else max(used_material_indices)
    )
    assert nonzero_target_material_index != target_material_index
    nonzero_self_report_corrupt = copy.deepcopy(document)
    nonzero_self_report_binary = bytearray(binary)
    _self_report_corrupt_material_texture_payload(
        nonzero_self_report_corrupt,
        nonzero_self_report_binary,
        nonzero_target_material_index,
        "base_color",
    )
    _expect_rejected(
        _glb_payload(nonzero_self_report_corrupt, nonzero_self_report_binary),
        "built-in texture truth",
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

    return legacy_readback_indices


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
    _assert_texture_microdetail_gate_self_test()
    assert _texture_cache_entry_count() == 0
    assets = _build_showcase_assets()
    assert _texture_cache_entry_count() == builtin_visual_material_count() == 8
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
    legacy_v1_readback_indices: set[int] = set()
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
        legacy_v1_readback_indices.update(_assert_asset(result, plan, validator))
        asset_count += 1
    assert asset_count == 12
    assert legacy_v1_readback_indices == set(range(builtin_visual_material_count()))
    assert _texture_cache_entry_count() == 16
    cache_facts = builtin_visual_texture_cache_facts()
    assert cache_facts["entry_count"] == cache_facts["max_entries"] == 16, cache_facts
    assert cache_facts["png_byte_size"] <= 4_000_000, cache_facts
    print("M108 visual PBR smoke passed: 12 multi-zone assets across four domains use one embedded GLB/readback texture truth")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
