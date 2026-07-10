from __future__ import annotations

import copy
import json
import math
import struct
from dataclasses import dataclass
from typing import Any, Iterable

from forgecad_agent.domain.concepts.models import ModuleGraphNode


class CombinedGlbError(ValueError):
    pass


@dataclass(frozen=True)
class CombinedGlbSource:
    node: ModuleGraphNode
    payload: bytes


def build_combined_glb(sources: Iterable[CombinedGlbSource]) -> bytes:
    output: dict[str, Any] = {
        "asset": {"version": "2.0", "generator": "ForgeCAD combined-glb/1"},
        "scene": 0,
        "scenes": [{"name": "ForgeCAD Combined Scene", "nodes": []}],
        "nodes": [],
        "meshes": [],
        "materials": [],
        "buffers": [{"byteLength": 0}],
        "bufferViews": [],
        "accessors": [],
    }
    binary = bytearray()
    material_lookup: dict[str, int] = {}
    source_count = 0
    for source in sources:
        source_count += 1
        document, source_binary = read_glb(source.payload)
        _assert_static_source(document, source.node.module_id)
        binary.extend(b"\x00" * ((4 - len(binary) % 4) % 4))
        binary_offset = len(binary)
        binary.extend(source_binary)

        view_offset = len(output["bufferViews"])
        accessor_offset = len(output["accessors"])
        mesh_offset = len(output["meshes"])
        node_offset = len(output["nodes"])
        material_map: dict[int, int] = {}
        for index, material in enumerate(document.get("materials", [])):
            canonical = json.dumps(material, sort_keys=True, separators=(",", ":"))
            if canonical not in material_lookup:
                material_lookup[canonical] = len(output["materials"])
                output["materials"].append(copy.deepcopy(material))
            material_map[index] = material_lookup[canonical]

        for view in document.get("bufferViews", []):
            copied = copy.deepcopy(view)
            if copied.get("buffer", 0) != 0:
                raise CombinedGlbError(f"{source.node.module_id}: only one GLB buffer is supported")
            copied["buffer"] = 0
            copied["byteOffset"] = int(copied.get("byteOffset", 0)) + binary_offset
            output["bufferViews"].append(copied)
        for accessor in document.get("accessors", []):
            copied = copy.deepcopy(accessor)
            if "bufferView" in copied:
                copied["bufferView"] += view_offset
            sparse = copied.get("sparse")
            if sparse:
                sparse["indices"]["bufferView"] += view_offset
                sparse["values"]["bufferView"] += view_offset
            output["accessors"].append(copied)
        for mesh in document.get("meshes", []):
            copied = copy.deepcopy(mesh)
            for primitive in copied.get("primitives", []):
                if "extensions" in primitive:
                    raise CombinedGlbError(
                        f"{source.node.module_id}: compressed/extended mesh primitives are not supported"
                    )
                primitive["attributes"] = {
                    semantic: accessor + accessor_offset
                    for semantic, accessor in primitive.get("attributes", {}).items()
                }
                if "indices" in primitive:
                    primitive["indices"] += accessor_offset
                if "material" in primitive:
                    primitive["material"] = material_map[primitive["material"]]
                for target in primitive.get("targets", []):
                    for semantic in list(target):
                        target[semantic] += accessor_offset
            output["meshes"].append(copied)

        source_nodes = document.get("nodes", [])
        for node in source_nodes:
            copied = copy.deepcopy(node)
            if "mesh" in copied:
                copied["mesh"] += mesh_offset
            if "children" in copied:
                copied["children"] = [child + node_offset for child in copied["children"]]
            output["nodes"].append(copied)
        scene_index = int(document.get("scene", 0))
        scenes = document.get("scenes", [])
        if not 0 <= scene_index < len(scenes):
            raise CombinedGlbError(f"{source.node.module_id}: active scene is missing")
        source_roots = [index + node_offset for index in scenes[scene_index].get("nodes", [])]
        wrapper_index = len(output["nodes"])
        wrapper = {
            "name": f"NODE_{source.node.node_id}__{source.node.module_id}",
            "children": source_roots,
            "translation": [value / 1000 for value in source.node.transform.position],
            "rotation": list(_quaternion_from_euler(source.node.transform.rotation)),
            "scale": _signed_scale(source.node.transform.scale, source.node.mirror_axis),
            "extras": {
                "forgecad_node_id": source.node.node_id,
                "forgecad_module_id": source.node.module_id,
                "forgecad_mirror_axis": source.node.mirror_axis,
            },
        }
        output["nodes"].append(wrapper)
        output["scenes"][0]["nodes"].append(wrapper_index)

    if source_count == 0:
        raise CombinedGlbError("combined GLB requires at least one module source")
    binary.extend(b"\x00" * ((4 - len(binary) % 4) % 4))
    output["buffers"][0]["byteLength"] = len(binary)
    return write_glb(output, bytes(binary))


def read_glb(payload: bytes) -> tuple[dict[str, Any], bytes]:
    if len(payload) < 20:
        raise CombinedGlbError("GLB is too short")
    magic, version, declared_length = struct.unpack_from("<4sII", payload, 0)
    if magic != b"glTF" or version != 2 or declared_length != len(payload):
        raise CombinedGlbError("expected a complete glTF 2.0 binary")
    offset = 12
    json_chunk: bytes | None = None
    binary_chunk = b""
    while offset + 8 <= len(payload):
        length, chunk_type = struct.unpack_from("<II", payload, offset)
        offset += 8
        end = offset + length
        if end > len(payload):
            raise CombinedGlbError("GLB chunk exceeds file length")
        if chunk_type == 0x4E4F534A and json_chunk is None:
            json_chunk = payload[offset:end]
        elif chunk_type == 0x004E4942 and not binary_chunk:
            binary_chunk = payload[offset:end]
        offset = end
    if offset != len(payload) or json_chunk is None:
        raise CombinedGlbError("GLB chunks are malformed")
    try:
        document = json.loads(json_chunk.rstrip(b" \x00").decode())
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise CombinedGlbError(f"GLB JSON is invalid: {exc}") from exc
    return document, binary_chunk


def write_glb(document: dict[str, Any], binary: bytes) -> bytes:
    json_chunk = json.dumps(document, ensure_ascii=False, separators=(",", ":")).encode()
    json_chunk += b" " * ((4 - len(json_chunk) % 4) % 4)
    binary += b"\x00" * ((4 - len(binary) % 4) % 4)
    total = 12 + 8 + len(json_chunk) + 8 + len(binary)
    return (
        struct.pack("<4sII", b"glTF", 2, total)
        + struct.pack("<II", len(json_chunk), 0x4E4F534A)
        + json_chunk
        + struct.pack("<II", len(binary), 0x004E4942)
        + binary
    )


def _assert_static_source(document: dict[str, Any], module_id: str) -> None:
    if str(document.get("asset", {}).get("version")) != "2.0":
        raise CombinedGlbError(f"{module_id}: asset.version must be 2.0")
    unsupported = [name for name in ("animations", "skins", "cameras", "textures", "images", "samplers") if document.get(name)]
    if unsupported:
        raise CombinedGlbError(f"{module_id}: unsupported combined GLB features: {', '.join(unsupported)}")
    if document.get("extensionsUsed") or document.get("extensionsRequired"):
        raise CombinedGlbError(f"{module_id}: glTF extensions are not supported by combined-glb/1")
    buffer_count = len(document.get("buffers", []))
    if buffer_count not in {0, 1} or (buffer_count == 0 and document.get("bufferViews")):
        raise CombinedGlbError(f"{module_id}: source must contain zero or one embedded buffer")


def _quaternion_from_euler(rotation: Iterable[float]) -> tuple[float, float, float, float]:
    x, y, z = (float(value) for value in rotation)
    c1, c2, c3 = math.cos(x / 2), math.cos(y / 2), math.cos(z / 2)
    s1, s2, s3 = math.sin(x / 2), math.sin(y / 2), math.sin(z / 2)
    return (
        s1 * c2 * c3 + c1 * s2 * s3,
        c1 * s2 * c3 - s1 * c2 * s3,
        c1 * c2 * s3 + s1 * s2 * c3,
        c1 * c2 * c3 - s1 * s2 * s3,
    )


def _signed_scale(scale: Iterable[float], mirror_axis: str) -> list[float]:
    result = [float(value) for value in scale]
    if mirror_axis != "none":
        result[{"x": 0, "y": 1, "z": 2}[mirror_axis]] *= -1
    return result
