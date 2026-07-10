#!/usr/bin/env python3
"""Generate the deterministic non-functional Arctic Patrol S1 reference Module Pack."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
import zlib
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
PACK_ROOT = ROOT / "assets" / "module-packs" / "weapon-concept-v1-reference"
MATERIALS = {
    "MAT_primary": ([0.10, 0.13, 0.17, 1.0], 0.78, 0.30),
    "MAT_secondary": ([0.22, 0.27, 0.32, 1.0], 0.64, 0.38),
    "MAT_accent": ([0.72, 0.08, 0.05, 1.0], 0.48, 0.32),
}


@dataclass(frozen=True)
class Box:
    center_mm: tuple[float, float, float]
    size_mm: tuple[float, float, float]
    material: str


@dataclass(frozen=True)
class ModuleDefinition:
    module_id: str
    category: str
    boxes: tuple[Box, ...]
    connectors: tuple[dict[str, Any], ...]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail if generated assets are stale")
    args = parser.parse_args()
    outputs = generate_outputs()
    if args.check:
        stale = [path for path, payload in outputs.items() if not path.is_file() or path.read_bytes() != payload]
        if stale:
            for path in stale:
                print(f"stale reference asset: {path.relative_to(ROOT)}", file=sys.stderr)
            return 1
        print(f"reference module pack assets ok: {len(MODULES)} modules")
        return 0
    for path, payload in outputs.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
        print(f"wrote {path.relative_to(ROOT)}")
    return 0


def generate_outputs() -> dict[Path, bytes]:
    outputs: dict[Path, bytes] = {}
    pack_modules: list[dict[str, Any]] = []
    pack_license = (
        "SPDX-License-Identifier: LicenseRef-ForgeCAD-Reference-Assets\n"
        "Copyright (c) 2026 ForgeCAD project contributors.\n"
        "Non-functional reference assets for project development and evaluation.\n"
    ).encode()
    outputs[PACK_ROOT / "LICENSES" / "PACK.txt"] = pack_license
    for index, definition in enumerate(MODULES, start=1):
        module_root = PACK_ROOT / "modules" / definition.module_id
        glb, bounds_mm, triangle_count = build_glb(definition)
        manifest = {
            "schema_version": "ModuleAssetManifest@1",
            "module_id": definition.module_id,
            "pack_id": "pack_weapon_concept_v1",
            "category": definition.category,
            "asset_id": f"asset_{definition.module_id.removeprefix('module_')}",
            "sha256": hashlib.sha256(glb).hexdigest(),
            "bounds_mm": bounds_mm,
            "triangle_count": triangle_count,
            "material_slots": list(MATERIALS),
            "connectors": list(definition.connectors),
        }
        outputs[module_root / "model.glb"] = glb
        outputs[module_root / "module.json"] = json_bytes(manifest)
        outputs[module_root / "thumbnail.png"] = thumbnail_png(index)
        outputs[module_root / "LICENSE.txt"] = pack_license
        relative_root = f"modules/{definition.module_id}"
        pack_modules.append(
            {
                "module_id": definition.module_id,
                "manifest_path": f"{relative_root}/module.json",
                "glb_path": f"{relative_root}/model.glb",
                "thumbnail_path": f"{relative_root}/thumbnail.png",
                "license_path": f"{relative_root}/LICENSE.txt",
                "lod": "LOD0",
            }
        )
    outputs[PACK_ROOT / "pack.json"] = json_bytes(
        {
            "schema_version": "ModulePackManifest@1",
            "pack_id": "pack_weapon_concept_v1",
            "profile_id": "profile_weapon_concept_v1",
            "name": "Arctic Patrol S1 Reference Module Pack",
            "version": "0.1.0",
            "description": (
                "Deterministic hard-surface reference modules for concept, game, film-prop "
                "and non-functional display workflow validation; not final art."
            ),
            "intended_uses": ["visual_asset", "game_asset", "film_prop", "non_functional_display"],
            "non_functional_only": True,
            "units": "millimeter",
            "up_axis": "Y",
            "forward_axis": "-Z",
            "handedness": "right",
            "license": {
                "spdx_expression": "LicenseRef-ForgeCAD-Reference-Assets",
                "license_path": "LICENSES/PACK.txt",
            },
            "modules": pack_modules,
        }
    )
    return outputs


def build_glb(definition: ModuleDefinition) -> tuple[bytes, list[float], int]:
    binary = bytearray()
    buffer_views: list[dict[str, Any]] = []
    accessors: list[dict[str, Any]] = []
    primitives: list[dict[str, Any]] = []
    overall_min = [float("inf")] * 3
    overall_max = [float("-inf")] * 3
    for box in definition.boxes:
        positions, normals, uvs, indices, minimum, maximum = box_geometry(box)
        for axis in range(3):
            overall_min[axis] = min(overall_min[axis], minimum[axis])
            overall_max[axis] = max(overall_max[axis], maximum[axis])
        position_accessor = add_accessor(binary, buffer_views, accessors, positions, 5126, 24, "VEC3", minimum, maximum)
        normal_accessor = add_accessor(binary, buffer_views, accessors, normals, 5126, 24, "VEC3")
        uv_accessor = add_accessor(binary, buffer_views, accessors, uvs, 5126, 24, "VEC2")
        index_accessor = add_accessor(binary, buffer_views, accessors, indices, 5123, 36, "SCALAR", target=34963)
        primitives.append(
            {
                "attributes": {
                    "POSITION": position_accessor,
                    "NORMAL": normal_accessor,
                    "TEXCOORD_0": uv_accessor,
                },
                "indices": index_accessor,
                "material": list(MATERIALS).index(box.material),
                "mode": 4,
            }
        )
    document = {
        "asset": {"version": "2.0", "generator": "ForgeCAD deterministic reference pack/1"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": f"GEO_{definition.module_id}_LOD0", "mesh": 0}],
        "meshes": [{"name": f"MESH_{definition.module_id}_LOD0", "primitives": primitives}],
        "materials": [
            {
                "name": name,
                "pbrMetallicRoughness": {
                    "baseColorFactor": values[0],
                    "metallicFactor": values[1],
                    "roughnessFactor": values[2],
                },
            }
            for name, values in MATERIALS.items()
        ],
        "buffers": [{"byteLength": len(binary)}],
        "bufferViews": buffer_views,
        "accessors": accessors,
    }
    json_chunk = json.dumps(document, separators=(",", ":")).encode()
    json_chunk += b" " * ((4 - len(json_chunk) % 4) % 4)
    binary.extend(b"\x00" * ((4 - len(binary) % 4) % 4))
    total_length = 12 + 8 + len(json_chunk) + 8 + len(binary)
    glb = (
        struct.pack("<4sII", b"glTF", 2, total_length)
        + struct.pack("<II", len(json_chunk), 0x4E4F534A)
        + json_chunk
        + struct.pack("<II", len(binary), 0x004E4942)
        + bytes(binary)
    )
    bounds_mm = [round((overall_max[i] - overall_min[i]) * 1000, 4) for i in range(3)]
    return glb, bounds_mm, len(definition.boxes) * 12


def box_geometry(box: Box) -> tuple[bytes, bytes, bytes, bytes, list[float], list[float]]:
    cx, cy, cz = (value / 1000 for value in box.center_mm)
    hx, hy, hz = (value / 2000 for value in box.size_mm)
    faces = (
        ((1, 0, 0), ((hx, -hy, -hz), (hx, -hy, hz), (hx, hy, hz), (hx, hy, -hz))),
        ((-1, 0, 0), ((-hx, -hy, hz), (-hx, -hy, -hz), (-hx, hy, -hz), (-hx, hy, hz))),
        ((0, 1, 0), ((-hx, hy, -hz), (hx, hy, -hz), (hx, hy, hz), (-hx, hy, hz))),
        ((0, -1, 0), ((-hx, -hy, hz), (hx, -hy, hz), (hx, -hy, -hz), (-hx, -hy, -hz))),
        ((0, 0, 1), ((hx, -hy, hz), (-hx, -hy, hz), (-hx, hy, hz), (hx, hy, hz))),
        ((0, 0, -1), ((-hx, -hy, -hz), (hx, -hy, -hz), (hx, hy, -hz), (-hx, hy, -hz))),
    )
    position_values: list[float] = []
    normal_values: list[float] = []
    uv_values: list[float] = []
    index_values: list[int] = []
    for face_index, (normal, vertices) in enumerate(faces):
        base = face_index * 4
        for x, y, z in vertices:
            position_values.extend((cx + x, cy + y, cz + z))
            normal_values.extend(normal)
        uv_values.extend((0, 0, 1, 0, 1, 1, 0, 1))
        index_values.extend((base, base + 1, base + 2, base, base + 2, base + 3))
    minimum = [cx - hx, cy - hy, cz - hz]
    maximum = [cx + hx, cy + hy, cz + hz]
    return (
        struct.pack(f"<{len(position_values)}f", *position_values),
        struct.pack(f"<{len(normal_values)}f", *normal_values),
        struct.pack(f"<{len(uv_values)}f", *uv_values),
        struct.pack(f"<{len(index_values)}H", *index_values),
        minimum,
        maximum,
    )


def add_accessor(
    binary: bytearray,
    buffer_views: list[dict[str, Any]],
    accessors: list[dict[str, Any]],
    payload: bytes,
    component_type: int,
    count: int,
    value_type: str,
    minimum: list[float] | None = None,
    maximum: list[float] | None = None,
    *,
    target: int = 34962,
) -> int:
    binary.extend(b"\x00" * ((4 - len(binary) % 4) % 4))
    offset = len(binary)
    binary.extend(payload)
    view_index = len(buffer_views)
    buffer_views.append({"buffer": 0, "byteOffset": offset, "byteLength": len(payload), "target": target})
    accessor: dict[str, Any] = {
        "bufferView": view_index,
        "componentType": component_type,
        "count": count,
        "type": value_type,
    }
    if minimum is not None:
        accessor["min"] = minimum
    if maximum is not None:
        accessor["max"] = maximum
    accessors.append(accessor)
    return len(accessors) - 1


def thumbnail_png(index: int) -> bytes:
    width = height = 512
    palette = ((17, 25, 36, 255), (48, 60, 75, 255), (184, 27, 18, 255))
    rows = bytearray()
    for y in range(height):
        rows.append(0)
        for x in range(width):
            accent = abs((x - 256) * 0.62) + abs((y - 250) * 0.9) < 85 + index * 2
            stripe = ((x + y + index * 19) // 54) % 6 == 0
            color = palette[2] if accent and stripe else palette[1] if accent else palette[0]
            rows.extend(color)
    return png_chunked(width, height, bytes(rows))


def png_chunked(width: int, height: int, raw: bytes) -> bytes:
    def chunk(kind: bytes, payload: bytes) -> bytes:
        return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def connector(connector_id: str, slot: str, connector_type: str, position: tuple[float, float, float]) -> dict[str, Any]:
    return {
        "connector_id": connector_id,
        "slot": slot,
        "connector_type": connector_type,
        "transform": {"position": list(position), "rotation": [0, 0, 0], "scale": [1, 1, 1]},
        "scale_range": [0.9, 1.1],
        "exclusive": True,
    }


def b(center: tuple[float, float, float], size: tuple[float, float, float], material: str) -> Box:
    return Box(center, size, material)


def json_bytes(value: dict[str, Any]) -> bytes:
    return (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode()


MODULES = (
    ModuleDefinition("module_core_shell_01", "core_shell", (
        b((0, 0, 0), (100, 48, 40), "MAT_primary"), b((-5, 20, 0), (72, 12, 34), "MAT_secondary"),
        b((-18, -20, 0), (36, 10, 36), "MAT_secondary"), b((28, 0, 19), (22, 28, 4), "MAT_accent"),
        b((-28, 0, -19), (18, 24, 4), "MAT_accent"),
    ), (
        connector("connector_core_front", "core.front", "shell_mount", (-50, 0, 0)),
        connector("connector_core_rear", "core.rear", "rear_mount", (50, 0, 0)),
        connector("connector_core_grip", "core.grip", "grip_mount", (14, -24, 0)),
        connector("connector_core_top", "core.top", "top_mount", (0, 24, 0)),
        connector("connector_core_side", "core.side", "side_mount", (0, 0, 20)),
        connector("connector_core_lower", "core.lower", "lower_mount", (-12, -24, 0)),
        connector("connector_core_storage", "core.storage", "storage_mount", (30, -24, 0)),
        connector("connector_core_armor", "core.armor", "armor_mount", (0, 0, -20)),
    )),
    ModuleDefinition("module_front_shell_01", "front_shell", (
        b((-32, 0, 0), (64, 34, 34), "MAT_primary"), b((-58, 10, 0), (18, 12, 30), "MAT_secondary"),
        b((-28, 17, 0), (28, 6, 28), "MAT_secondary"), b((-45, 0, 17), (18, 16, 3), "MAT_accent"),
    ), (connector("connector_front_01_core", "front.core", "shell_mount", (0, 0, 0)),)),
    ModuleDefinition("module_front_shell_02", "front_shell", (
        b((-38, 0, 0), (76, 28, 40), "MAT_primary"), b((-65, 8, 0), (20, 12, 34), "MAT_secondary"),
        b((-34, 15, 0), (38, 8, 36), "MAT_secondary"), b((-20, -14, 0), (30, 6, 32), "MAT_accent"),
        b((-56, 0, -21), (14, 12, 3), "MAT_accent"),
    ), (connector("connector_front_02_core", "front.core", "shell_mount", (0, 0, 0)),)),
    ModuleDefinition("module_rear_shell_01", "rear_shell", (
        b((20, 0, 0), (40, 36, 36), "MAT_primary"), b((37, 8, 0), (12, 18, 30), "MAT_secondary"),
        b((18, 18, 0), (24, 6, 30), "MAT_secondary"), b((30, -12, 18), (14, 7, 3), "MAT_accent"),
    ), (connector("connector_rear_core", "rear.core", "rear_mount", (0, 0, 0)),)),
    ModuleDefinition("module_grip_shell_01", "grip_shell", (
        b((0, -32, 0), (28, 64, 30), "MAT_primary"), b((0, -61, 0), (34, 12, 34), "MAT_secondary"),
        b((0, -28, 16), (20, 38, 3), "MAT_secondary"), b((0, -46, -16), (18, 8, 3), "MAT_accent"),
    ), (connector("connector_grip_core", "grip.core", "grip_mount", (0, 0, 0)),)),
    ModuleDefinition("module_top_accessory_01", "top_accessory", (
        b((0, 7, 0), (42, 14, 18), "MAT_primary"), b((-12, 16, 0), (16, 8, 14), "MAT_secondary"),
        b((14, 15, 0), (10, 6, 12), "MAT_accent"),
    ), (connector("connector_top_core", "top.core", "top_mount", (0, 0, 0)),)),
    ModuleDefinition("module_side_accessory_01", "side_accessory", (
        b((0, 0, 7), (38, 24, 14), "MAT_primary"), b((-12, 0, 16), (12, 16, 6), "MAT_secondary"),
        b((13, 0, 16), (8, 12, 6), "MAT_accent"),
    ), (connector("connector_side_core", "side.core", "side_mount", (0, 0, 0)),)),
    ModuleDefinition("module_lower_structure_01", "lower_structure", (
        b((0, -8, 0), (46, 16, 24), "MAT_primary"), b((-15, -18, 0), (12, 10, 20), "MAT_secondary"),
        b((14, -18, 0), (16, 8, 18), "MAT_secondary"), b((0, -13, 13), (12, 6, 3), "MAT_accent"),
    ), (connector("connector_lower_core", "lower.core", "lower_mount", (0, 0, 0)),)),
    ModuleDefinition("module_storage_visual_01", "storage_visual", (
        b((0, -18, 0), (34, 36, 28), "MAT_primary"), b((0, -38, 0), (28, 10, 24), "MAT_secondary"),
        b((0, -16, 15), (20, 18, 3), "MAT_secondary"), b((0, -30, -15), (14, 6, 3), "MAT_accent"),
    ), (connector("connector_storage_core", "storage.core", "storage_mount", (0, 0, 0)),)),
    ModuleDefinition("module_armor_panel_01", "armor_panel", (
        b((0, 0, -4), (52, 30, 8), "MAT_primary"), b((-12, 0, -10), (18, 22, 4), "MAT_secondary"),
        b((16, 0, -10), (10, 16, 4), "MAT_accent"),
    ), (connector("connector_armor_core", "armor.core", "armor_mount", (0, 0, 0)),)),
)


if __name__ == "__main__":
    raise SystemExit(main())
