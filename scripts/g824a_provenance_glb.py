#!/usr/bin/env python3
"""Build and read back temporary GLBs for isolated G824A CSG evidence."""

from __future__ import annotations

import hashlib
import json
import math
import struct
from collections import defaultdict
from pathlib import Path
from typing import Any

from forgecad_agent.application.geometry_worker import read_shape_program_glb_facts


def _append_view(binary: bytearray, views: list[dict[str, Any]], payload: bytes, target: int | None = None) -> int:
    binary.extend(b"\x00" * ((4 - len(binary) % 4) % 4))
    view: dict[str, Any] = {"buffer": 0, "byteOffset": len(binary), "byteLength": len(payload)}
    if target is not None:
        view["target"] = target
    views.append(view)
    binary.extend(payload)
    return len(views) - 1


def _accessor(
    binary: bytearray,
    views: list[dict[str, Any]],
    accessors: list[dict[str, Any]],
    payload: bytes,
    component_type: int,
    count: int,
    value_type: str,
    *,
    minimum: list[float] | None = None,
    maximum: list[float] | None = None,
    target: int | None = None,
) -> int:
    item: dict[str, Any] = {
        "bufferView": _append_view(binary, views, payload, target),
        "componentType": component_type,
        "count": count,
        "type": value_type,
    }
    if minimum is not None:
        item["min"] = minimum
    if maximum is not None:
        item["max"] = maximum
    accessors.append(item)
    return len(accessors) - 1


def _normal(vertices: list[list[float]]) -> list[float]:
    a, b, c = vertices
    ab = [b[index] - a[index] for index in range(3)]
    ac = [c[index] - a[index] for index in range(3)]
    cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ]
    length = math.sqrt(sum(value * value for value in cross))
    if length <= 1e-12:
        raise AssertionError("candidate produced a degenerate triangle before GLB write")
    return [value / length for value in cross]


def _parse_document(payload: bytes) -> dict[str, Any]:
    magic, version, total = struct.unpack_from("<4sII", payload, 0)
    if magic != b"glTF" or version != 2 or total != len(payload):
        raise AssertionError("temporary provenance GLB header is invalid")
    json_length, chunk_type = struct.unpack_from("<II", payload, 12)
    if chunk_type != 0x4E4F534A:
        raise AssertionError("temporary provenance GLB is missing JSON first")
    return json.loads(payload[20:20 + json_length].rstrip(b" \x00").decode("utf-8"))


def build_and_readback_glb(fixture: dict[str, Any], *, staging_output: Path | None = None) -> dict[str, Any]:
    grouped: dict[tuple[Any, ...], list[dict[str, Any]]] = defaultdict(list)
    for triangle in fixture["triangles"]:
        key = (
            triangle["source_id"],
            triangle["material_id"],
            triangle["zone_id"],
            int(triangle["source_face_id"]),
            bool(triangle["backside"]),
            triangle["surface_role"],
        )
        grouped[key].append(triangle)
    material_ids = sorted({key[1] for key in grouped})
    material_index = {value: index for index, value in enumerate(material_ids)}
    binary = bytearray()
    views: list[dict[str, Any]] = []
    accessors: list[dict[str, Any]] = []
    primitives: list[dict[str, Any]] = []
    expected_provenance: list[dict[str, Any]] = []
    for key in sorted(grouped):
        source_id, material_id, zone_id, source_face_id, backside, surface_role = key
        positions: list[float] = []
        normals: list[float] = []
        uvs: list[float] = []
        indices: list[int] = []
        for triangle in grouped[key]:
            vertices = [[float(value) / 1000 for value in vertex] for vertex in triangle["vertices_mm"]]
            try:
                normal = _normal(vertices)
            except AssertionError:
                return {
                    "status": "rejected",
                    "error_code": "CSG_DEGENERATE_OUTPUT",
                    "triangle_count": fixture["triangle_count"],
                    "partial_glb_emitted": False,
                    "forgecad_readback_verified": False,
                    "glb_provenance_verified": False,
                }
            base = len(positions) // 3
            for vertex in vertices:
                positions.extend(vertex)
                normals.extend(normal)
            uvs.extend((0, 0, 1, 0, 0, 1))
            indices.extend((base, base + 1, base + 2))
        axis_values = [[positions[offset + axis] for offset in range(0, len(positions), 3)] for axis in range(3)]
        lower = [min(values) for values in axis_values]
        upper = [max(values) for values in axis_values]
        position_accessor = _accessor(
            binary, views, accessors, struct.pack(f"<{len(positions)}f", *positions), 5126, len(positions) // 3, "VEC3",
            minimum=lower, maximum=upper, target=34962,
        )
        normal_accessor = _accessor(
            binary, views, accessors, struct.pack(f"<{len(normals)}f", *normals), 5126, len(normals) // 3, "VEC3", target=34962,
        )
        uv_accessor = _accessor(
            binary, views, accessors, struct.pack(f"<{len(uvs)}f", *uvs), 5126, len(uvs) // 2, "VEC2", target=34962,
        )
        index_accessor = _accessor(
            binary, views, accessors, struct.pack(f"<{len(indices)}I", *indices), 5125, len(indices), "SCALAR", target=34963,
        )
        provenance = {
            "source_id": source_id,
            "material_id": material_id,
            "material_zone_id": zone_id,
            "source_face_id": source_face_id,
            "boolean_backside": backside,
        }
        expected_provenance.append(provenance)
        primitives.append({
            "attributes": {"POSITION": position_accessor, "NORMAL": normal_accessor, "TEXCOORD_0": uv_accessor},
            "indices": index_accessor,
            "material": material_index[material_id],
            "mode": 4,
            "extras": {
                "forgecad_part_role": source_id,
                "forgecad_surface_roles": [surface_role],
                "forgecad_surface_ranges": [{"surface_role": surface_role, "first_triangle": 0, "triangle_count": len(indices) // 3}],
                "forgecad_csg_provenance": provenance,
            },
        })
    document = {
        "asset": {"version": "2.0", "generator": "ForgeCAD G824A isolated provenance evidence"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0, "name": "FORGECAD_G824A_CSG_EVIDENCE"}],
        "meshes": [{"name": "FORGECAD_G824A_CSG", "primitives": primitives}],
        "materials": [
            {"name": value, "extras": {"forgecad_material_id": value}, "pbrMetallicRoughness": {"baseColorFactor": [0.5, 0.5, 0.5, 1]}}
            for value in material_ids
        ],
        "buffers": [{"byteLength": len(binary)}],
        "bufferViews": views,
        "accessors": accessors,
    }
    json_chunk = json.dumps(document, sort_keys=True, separators=(",", ":")).encode()
    json_chunk += b" " * ((4 - len(json_chunk) % 4) % 4)
    binary.extend(b"\x00" * ((4 - len(binary) % 4) % 4))
    total = 12 + 8 + len(json_chunk) + 8 + len(binary)
    payload = (
        struct.pack("<4sII", b"glTF", 2, total)
        + struct.pack("<II", len(json_chunk), 0x4E4F534A)
        + json_chunk
        + struct.pack("<II", len(binary), 0x004E4942)
        + binary
    )
    facts = read_shape_program_glb_facts(bytes(payload))
    read_document = _parse_document(bytes(payload))
    actual_provenance = sorted(
        [primitive["extras"]["forgecad_csg_provenance"] for primitive in read_document["meshes"][0]["primitives"]],
        key=lambda item: json.dumps(item, sort_keys=True),
    )
    expected_provenance = sorted(expected_provenance, key=lambda item: json.dumps(item, sort_keys=True))
    if actual_provenance != expected_provenance:
        raise AssertionError("GLB readback changed CSG provenance")
    if facts.triangle_count != fixture["triangle_count"]:
        raise AssertionError("ForgeCAD readback triangle count differs from CSG output")
    if staging_output is not None:
        staging_output.parent.mkdir(parents=True, exist_ok=True)
        temporary = staging_output.with_suffix(staging_output.suffix + ".tmp")
        temporary.write_bytes(payload)
        temporary.replace(staging_output)
    return {
        "status": "passed",
        "glb_sha256": hashlib.sha256(payload).hexdigest(),
        "triangle_count": facts.triangle_count,
        "primitive_count": facts.primitive_count,
        "material_count": facts.material_count,
        "source_ids": sorted({item["source_id"] for item in actual_provenance}),
        "material_ids": sorted({item["material_id"] for item in actual_provenance}),
        "zone_ids": sorted({item["material_zone_id"] for item in actual_provenance}),
        "source_face_ids": sorted({item["source_face_id"] for item in actual_provenance}),
        "has_backside_cut_surface": any(item["boolean_backside"] for item in actual_provenance),
        "forgecad_readback_verified": True,
        "glb_provenance_verified": True,
    }
