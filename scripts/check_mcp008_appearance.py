#!/usr/bin/env python3
"""Exercise the bounded AppearanceProgram, GLB attributes, and fixed passes."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import math
import struct
import subprocess
from typing import Any


def canonical(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()


def geometry() -> dict[str, Any]:
    value: dict[str, Any] = {
        "schema_version": "GeometryProgram@1",
        "project_id": "mcp008-worker-fixture",
        "representation_plan_sha256": "f" * 64,
        "nodes": [
            {"node_id": "shell", "operator_id": "forgecad.geometry.primitive@1", "part_id": "shell", "parameters": {"shape": "box", "size": [1.2, 1.6, 0.55], "position": [0, 0.8, 0], "material_zone_id": "zone-white-shell"}},
            {"node_id": "core", "operator_id": "forgecad.geometry.primitive@1", "part_id": "core", "parameters": {"shape": "cylinder", "size": [0.55, 1.2, 0.55], "position": [0, 0.7, 0], "material_zone_id": "zone-black-mechanical", "segments": 16}},
            {"node_id": "light", "operator_id": "forgecad.geometry.primitive@1", "part_id": "light", "parameters": {"shape": "box", "size": [0.12, 0.24, 0.03], "position": [0, 1.05, 0.3], "material_zone_id": "zone-amber-emissive"}},
        ],
        "budgets": {"max_nodes": 8, "max_triangles": 20000, "max_runtime_ms": 1000},
    }
    value["canonical_sha256"] = hashlib.sha256(canonical(value)).hexdigest()
    return value


def appearance(program: dict[str, Any]) -> dict[str, Any]:
    value: dict[str, Any] = {
        "schema_version": "AppearanceProgram@1",
        "project_id": program["project_id"],
        "geometry_program_sha256": program["canonical_sha256"],
        "material_zones": [
            {"zone_id": "zone-white-shell", "part_ids": ["shell"], "base_color": [0.78, 0.82, 0.86, 1.0], "metallic": 0.72, "roughness": 0.28, "emissive": [0.0, 0.0, 0.0]},
            {"zone_id": "zone-black-mechanical", "part_ids": ["core"], "base_color": [0.03, 0.04, 0.05, 1.0], "metallic": 0.75, "roughness": 0.3, "emissive": [0.0, 0.0, 0.0]},
            {"zone_id": "zone-amber-emissive", "part_ids": ["light"], "base_color": [0.16, 0.06, 0.01, 1.0], "metallic": 0.2, "roughness": 0.25, "emissive": [1.0, 0.12, 0.01]},
        ],
    }
    value["canonical_sha256"] = hashlib.sha256(canonical(value)).hexdigest()
    return value


def request(operation: str, program: dict[str, Any], material: dict[str, Any]) -> dict[str, Any]:
    return {"protocol": "forgecad-worker-protocol@1", "request_id": "mcp008-fixture", "operation": operation, "payload": {"geometry_program": program, "appearance_program": material}}


def run(worker: str, payload: dict[str, Any]) -> dict[str, Any]:
    completed = subprocess.run([worker, "--isolated-once"], input=json.dumps(payload) + "\n", text=True, capture_output=True, check=False, timeout=20)
    lines = [line for line in completed.stdout.splitlines() if line.strip()]
    if not lines:
        raise RuntimeError(f"worker returned no response: {completed.stderr[-400:]}")
    return json.loads(lines[-1])


def non_negative_int(value: Any, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise RuntimeError(f"{label} is invalid")
    return value


def decode_glb(raw: bytes) -> tuple[dict[str, Any], bytes, int]:
    if len(raw) < 28 or raw[:4] != b"glTF" or struct.unpack_from("<I", raw, 4)[0] != 2:
        raise RuntimeError("GLB header failed")
    if struct.unpack_from("<I", raw, 8)[0] != len(raw) or raw[16:20] != b"JSON":
        raise RuntimeError("GLB length/JSON chunk failed")
    json_length = struct.unpack_from("<I", raw, 12)[0]
    json_end = 20 + json_length
    if json_end + 8 > len(raw) or raw[json_end + 4 : json_end + 8] != b"BIN\x00":
        raise RuntimeError("GLB BIN chunk failed")
    binary_length = struct.unpack_from("<I", raw, json_end)[0]
    binary_start = json_end + 8
    if binary_start + binary_length != len(raw):
        raise RuntimeError("GLB BIN length failed")
    try:
        root = json.loads(raw[20:json_end].decode())
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RuntimeError("GLB JSON decode failed") from error
    if not isinstance(root, dict):
        raise RuntimeError("GLB root is invalid")
    buffers = root.get("buffers")
    if not isinstance(buffers, list) or len(buffers) != 1 or not isinstance(buffers[0], dict):
        raise RuntimeError("GLB buffer declaration is invalid")
    if buffers[0].get("uri") is not None or non_negative_int(buffers[0].get("byteLength"), "GLB buffer byteLength") != binary_length:
        raise RuntimeError("GLB must contain exactly one embedded BIN buffer")
    return root, raw[binary_start:], binary_start


def accessor_window(
    root: dict[str, Any],
    binary: bytes,
    accessor_index: Any,
    expected_type: str,
    expected_component_type: int,
    element_size: int,
) -> tuple[int, int, int]:
    accessors = root.get("accessors")
    views = root.get("bufferViews")
    if not isinstance(accessors, list) or not isinstance(views, list):
        raise RuntimeError("GLB accessors or buffer views are missing")
    index = non_negative_int(accessor_index, "GLB accessor index")
    if index >= len(accessors) or not isinstance(accessors[index], dict):
        raise RuntimeError("GLB accessor index is invalid")
    accessor = accessors[index]
    if accessor.get("type") != expected_type or accessor.get("componentType") != expected_component_type or "sparse" in accessor:
        raise RuntimeError("GLB accessor layout is invalid")
    count = non_negative_int(accessor.get("count"), "GLB accessor count")
    view_index = non_negative_int(accessor.get("bufferView"), "GLB buffer view index")
    if view_index >= len(views) or not isinstance(views[view_index], dict):
        raise RuntimeError("GLB buffer view index is invalid")
    view = views[view_index]
    if view.get("buffer") != 0:
        raise RuntimeError("GLB uses an unsupported buffer")
    view_offset = non_negative_int(view.get("byteOffset", 0), "GLB buffer view offset")
    view_length = non_negative_int(view.get("byteLength"), "GLB buffer view length")
    accessor_offset = non_negative_int(accessor.get("byteOffset", 0), "GLB accessor offset")
    stride = non_negative_int(view.get("byteStride", element_size), "GLB byte stride")
    if stride < element_size or view_offset + view_length > len(binary):
        raise RuntimeError("GLB buffer view range is invalid")
    start = view_offset + accessor_offset
    byte_length = 0 if count == 0 else (count - 1) * stride + element_size
    if start + byte_length > view_offset + view_length or start + byte_length > len(binary):
        raise RuntimeError("GLB accessor range is invalid")
    return start, count, stride


def float_accessor(
    root: dict[str, Any], binary: bytes, accessor_index: Any, expected_type: str, dimensions: int
) -> tuple[list[tuple[float, ...]], int]:
    start, count, stride = accessor_window(root, binary, accessor_index, expected_type, 5126, dimensions * 4)
    values = [struct.unpack_from("<" + "f" * dimensions, binary, start + item * stride) for item in range(count)]
    return values, start


def index_accessor(root: dict[str, Any], binary: bytes, accessor_index: Any) -> list[int]:
    accessors = root.get("accessors")
    if not isinstance(accessors, list):
        raise RuntimeError("GLB accessors are missing")
    index = non_negative_int(accessor_index, "GLB index accessor")
    if index >= len(accessors) or not isinstance(accessors[index], dict):
        raise RuntimeError("GLB index accessor is invalid")
    component_type = accessors[index].get("componentType")
    layouts = {5121: (1, "<B"), 5123: (2, "<H"), 5125: (4, "<I")}
    if component_type not in layouts:
        raise RuntimeError("GLB index component type is invalid")
    element_size, fmt = layouts[component_type]
    start, count, stride = accessor_window(root, binary, index, "SCALAR", component_type, element_size)
    return [struct.unpack_from(fmt, binary, start + item * stride)[0] for item in range(count)]


def normalize(vector: tuple[float, float, float]) -> tuple[float, float, float]:
    length = math.sqrt(sum(component * component for component in vector))
    if not math.isfinite(length) or length <= 1.0e-8:
        raise RuntimeError("normal or tangent has zero length")
    return tuple(component / length for component in vector)


def physical_uv_tangent_readback(root: dict[str, Any], binary: bytes) -> dict[str, Any]:
    meshes = root.get("meshes")
    if not isinstance(meshes, list) or not meshes:
        raise RuntimeError("GLB meshes are missing")
    part_ids: list[str] = []
    triangle_count = 0
    first_tangent_offset: int | None = None
    for mesh in meshes:
        if not isinstance(mesh, dict) or not isinstance(mesh.get("name"), str) or not mesh["name"]:
            raise RuntimeError("GLB mesh name is invalid")
        part_ids.append(mesh["name"])
        primitives = mesh.get("primitives")
        if not isinstance(primitives, list) or not primitives:
            raise RuntimeError("GLB mesh primitives are missing")
        for primitive in primitives:
            if not isinstance(primitive, dict) or not isinstance(primitive.get("attributes"), dict):
                raise RuntimeError("GLB primitive attributes are invalid")
            attributes = primitive["attributes"]
            positions, _ = float_accessor(root, binary, attributes.get("POSITION"), "VEC3", 3)
            normals, _ = float_accessor(root, binary, attributes.get("NORMAL"), "VEC3", 3)
            uvs, _ = float_accessor(root, binary, attributes.get("TEXCOORD_0"), "VEC2", 2)
            tangents, tangent_offset = float_accessor(root, binary, attributes.get("TANGENT"), "VEC4", 4)
            indices = index_accessor(root, binary, primitive.get("indices"))
            if not positions or len(positions) != len(normals) or len(positions) != len(uvs) or len(positions) != len(tangents):
                raise RuntimeError("GLB UV/tangent attribute counts are invalid")
            if first_tangent_offset is None:
                first_tangent_offset = tangent_offset
            for uv in uvs:
                if not all(math.isfinite(component) for component in uv):
                    raise RuntimeError("GLB UV contains a non-finite value")
            for normal, tangent in zip(normals, tangents):
                if not all(math.isfinite(component) for component in normal + tangent):
                    raise RuntimeError("GLB normal or tangent contains a non-finite value")
                normal3 = normalize(normal)
                tangent3 = normalize(tangent[:3])
                if abs(sum(left * right for left, right in zip(normal3, tangent3))) > 1.0e-3:
                    raise RuntimeError("GLB tangent is not orthogonal to its normal")
                if abs(abs(tangent[3]) - 1.0) > 1.0e-3:
                    raise RuntimeError("GLB tangent handedness is invalid")
            if len(indices) % 3:
                raise RuntimeError("GLB indices are not triangles")
            for triangle in range(0, len(indices), 3):
                a, b, c = indices[triangle : triangle + 3]
                if any(index >= len(positions) for index in (a, b, c)):
                    raise RuntimeError("GLB index is out of bounds")
                uv_a, uv_b, uv_c = uvs[a], uvs[b], uvs[c]
                uv_area = (uv_b[0] - uv_a[0]) * (uv_c[1] - uv_a[1]) - (uv_b[1] - uv_a[1]) * (uv_c[0] - uv_a[0])
                if not math.isfinite(uv_area) or abs(uv_area) <= 1.0e-8:
                    raise RuntimeError("GLB UV triangle has zero area")
                triangle_count += 1
    if triangle_count == 0 or first_tangent_offset is None:
        raise RuntimeError("GLB has no UV/tangent triangles")
    return {"part_ids": part_ids, "triangle_count": triangle_count, "first_tangent_offset": first_tangent_offset}


def assert_tangent_tamper_fails_closed(raw: bytes) -> None:
    root, binary, binary_start = decode_glb(raw)
    readback = physical_uv_tangent_readback(root, binary)
    tampered = bytearray(raw)
    struct.pack_into("<f", tampered, binary_start + readback["first_tangent_offset"], float("nan"))
    tampered_root, tampered_binary, _ = decode_glb(bytes(tampered))
    try:
        physical_uv_tangent_readback(tampered_root, tampered_binary)
    except RuntimeError:
        return
    raise RuntimeError("tampered tangent bytes were accepted")


def inspect_glb(encoded: str) -> tuple[str, dict[str, Any]]:
    raw = base64.b64decode(encoded, validate=True)
    root, binary, _ = decode_glb(raw)
    # Do not trust ForgeCAD root extras for UV/tangent status. This gate reads
    # the actual accessors and triangle payload returned by the Worker.
    physical = physical_uv_tangent_readback(root, binary)
    assert_tangent_tamper_fails_closed(raw)
    return hashlib.sha256(raw).hexdigest(), {
        "size_bytes": len(raw),
        "part_ids": physical["part_ids"],
        "triangle_count": physical["triangle_count"],
        "uv_status": "physical-bin-passed",
        "tangent_status": "physical-bin-passed",
        "negative_tangent_byte_tamper": "PASS",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--worker", required=True)
    options = parser.parse_args()
    program = geometry()
    material = appearance(program)
    first = run(options.worker, request("compile_geometry", program, material))
    second = run(options.worker, request("compile_geometry", program, material))
    if not first.get("ok") or first.get("result") != second.get("result"):
        raise SystemExit("deterministic appearance compile failed")
    artifact_hash, readback = inspect_glb(first["result"]["glb_base64"])
    rendered = run(options.worker, request("render_fixed", program, material))
    if not rendered.get("ok") or len(rendered.get("result", {}).get("passes", [])) != 4:
        raise SystemExit("fixed render passes failed")
    if any(not base64.b64decode(item["png_base64"], validate=True).startswith(b"\x89PNG") for item in rendered["result"]["passes"]):
        raise SystemExit("fixed render PNG readback failed")
    invalid = dict(material)
    invalid["geometry_program_sha256"] = "0" * 64
    invalid["canonical_sha256"] = hashlib.sha256(canonical({key: value for key, value in invalid.items() if key != "canonical_sha256"})).hexdigest()
    negative = run(options.worker, request("compile_geometry", program, invalid))
    if negative.get("ok") or negative.get("error", {}).get("code") != "GEOMETRY_REJECTED":
        raise SystemExit("appearance geometry hash mismatch did not fail closed")
    print(json.dumps({"status": "PASS", "artifact_sha256": artifact_hash, **readback, "fixed_passes": [item["pass"] for item in rendered["result"]["passes"]], "negative_geometry_binding": "PASS"}, ensure_ascii=False, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
