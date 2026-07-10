#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import shutil
import sqlite3
import struct
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path
from typing import Any

from concept_module_pack import (
    ModulePackValidationError,
    import_module_pack,
    validate_module_pack,
)
from smoke_r2_concept_projects import (
    _assert,
    _free_port,
    _json_request,
    _start_agent,
    _stop_agent,
    _wait_for_health,
)


ROOT = Path(__file__).resolve().parents[1]
CATEGORIES = [
    "core_shell",
    "front_shell",
    "rear_shell",
    "grip_shell",
    "top_accessory",
    "side_accessory",
    "lower_structure",
    "storage_visual",
    "armor_panel",
]


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad_r3_module_pack_") as temporary_directory:
        temporary_root = Path(temporary_directory)
        pack_root = temporary_root / "weapon-concept-v1"
        _create_pack(pack_root)

        validated = validate_module_pack(pack_root, release=True)
        _assert(len(validated.modules) == 9, "release pack module count mismatch")
        dry_run = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts" / "concept_module_pack.py"),
                str(pack_root),
                "--release",
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        _assert(dry_run.returncode == 0, f"module pack CLI dry-run failed: {dry_run.stderr}")
        dry_run_report = json.loads(dry_run.stdout)
        _assert(dry_run_report["mode"] == "dry-run", "CLI was not dry-run by default")

        negative_results = {
            "hash_mismatch": _negative_case(pack_root, temporary_root, "hash_mismatch"),
            "unsafe_path": _negative_case(pack_root, temporary_root, "unsafe_path"),
            "missing_license": _negative_case(pack_root, temporary_root, "missing_license"),
            "duplicate_connector": _negative_case(
                pack_root, temporary_root, "duplicate_connector"
            ),
            "pack_mismatch": _negative_case(pack_root, temporary_root, "pack_mismatch"),
        }
        _assert(all(negative_results.values()), "one or more invalid packs were accepted")

        library_root = temporary_root / "ForgeCADLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(library_root, port)
        try:
            _wait_for_health(base_url, process)
            imported = import_module_pack(validated, base_url)
            replay = import_module_pack(validated, base_url)
            _assert(len(imported) == 9, "pack import count mismatch")
            _assert(
                [item["object_path"] for item in imported]
                == [item["object_path"] for item in replay],
                "pack replay was not idempotent",
            )
            listed = _json_request(
                base_url,
                "/api/v1/module-assets?pack_id=pack_weapon_concept_v1",
                method="GET",
            )
            _assert(len(listed["items"]) == 9, "registry did not contain the full pack")
        finally:
            _stop_agent(process)

        with sqlite3.connect(library_root / "library.db") as connection:
            module_count = connection.execute("SELECT COUNT(*) FROM module_assets").fetchone()[0]
            connector_count = connection.execute(
                "SELECT COUNT(*) FROM module_connectors"
            ).fetchone()[0]
        _assert(module_count == 9, "module pack replay created duplicate records")
        _assert(connector_count == 9, "connector import count mismatch")

        restart_port = _free_port()
        restart_url = f"http://127.0.0.1:{restart_port}"
        restarted = _start_agent(library_root, restart_port)
        try:
            _wait_for_health(restart_url, restarted)
            restored = _json_request(
                restart_url,
                "/api/v1/module-assets?pack_id=pack_weapon_concept_v1",
                method="GET",
            )
            _assert(len(restored["items"]) == 9, "restart lost imported module pack")
        finally:
            _stop_agent(restarted)

        print(
            json.dumps(
                {
                    "ok": True,
                    "module_count": module_count,
                    "connector_count": connector_count,
                    "dry_run_default": True,
                    "release_categories": CATEGORIES,
                    "negative_cases": negative_results,
                    "idempotent_replay": True,
                    "restart_restored": True,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    return 0


def _create_pack(root: Path) -> None:
    (root / "LICENSES").mkdir(parents=True)
    (root / "LICENSES" / "PACK.txt").write_text(
        "SPDX-License-Identifier: CC-BY-4.0\nConcept fixtures only.\n",
        encoding="utf-8",
    )
    entries: list[dict[str, Any]] = []
    for index, category in enumerate(CATEGORIES, start=1):
        module_id = f"module_{category}_{index:02d}"
        module_root = root / "modules" / module_id
        module_root.mkdir(parents=True)
        material_name = f"mat_{category}"
        payload = _triangle_glb(material_name)
        slot_prefix = category.removesuffix("_shell").removesuffix("_accessory").removesuffix(
            "_structure"
        ).removesuffix("_visual").removesuffix("_panel")
        manifest = {
            "schema_version": "ModuleAssetManifest@1",
            "module_id": module_id,
            "pack_id": "pack_weapon_concept_v1",
            "category": category,
            "asset_id": f"asset_{category}_{index:02d}",
            "sha256": hashlib.sha256(payload).hexdigest(),
            "bounds_mm": [100.0, 50.0, 10.0],
            "triangle_count": 1,
            "material_slots": [material_name],
            "connectors": [
                {
                    "connector_id": f"connector_{category}_{index:02d}",
                    "slot": f"{slot_prefix}.core",
                    "connector_type": "visual_mount",
                    "transform": {
                        "position": [0, 0, 0],
                        "rotation": [0, 0, 0],
                        "scale": [1, 1, 1],
                    },
                    "scale_range": [1, 1],
                    "exclusive": True,
                }
            ],
        }
        _write_json(module_root / "module.json", manifest)
        (module_root / "model.glb").write_bytes(payload)
        (module_root / "thumbnail.png").write_bytes(_png_512())
        (module_root / "LICENSE.txt").write_text(
            "SPDX-License-Identifier: CC-BY-4.0\nGenerated smoke fixture.\n",
            encoding="utf-8",
        )
        entries.append(
            {
                "module_id": module_id,
                "manifest_path": f"modules/{module_id}/module.json",
                "glb_path": f"modules/{module_id}/model.glb",
                "thumbnail_path": f"modules/{module_id}/thumbnail.png",
                "license_path": f"modules/{module_id}/LICENSE.txt",
                "lod": "LOD0",
            }
        )
    _write_json(
        root / "pack.json",
        {
            "schema_version": "ModulePackManifest@1",
            "pack_id": "pack_weapon_concept_v1",
            "profile_id": "profile_weapon_concept_v1",
            "name": "Weapon Concept Pack smoke fixture",
            "version": "0.1.0",
            "description": "Non-functional concept, game and film-prop module validation fixture.",
            "intended_uses": ["visual_asset", "game_asset", "film_prop"],
            "non_functional_only": True,
            "units": "millimeter",
            "up_axis": "Y",
            "forward_axis": "-Z",
            "handedness": "right",
            "license": {
                "spdx_expression": "CC-BY-4.0",
                "license_path": "LICENSES/PACK.txt",
            },
            "modules": entries,
        },
    )


def _negative_case(source: Path, temporary_root: Path, case: str) -> bool:
    target = temporary_root / f"invalid_{case}"
    shutil.copytree(source, target)
    pack_path = target / "pack.json"
    pack = json.loads(pack_path.read_text(encoding="utf-8"))
    first_entry = pack["modules"][0]
    if case == "hash_mismatch":
        (target / first_entry["glb_path"]).write_bytes(b"invalid")
    elif case == "unsafe_path":
        first_entry["thumbnail_path"] = "../outside.png"
        _write_json(pack_path, pack)
    elif case == "missing_license":
        first_entry["license_path"] = "modules/missing/LICENSE.txt"
        _write_json(pack_path, pack)
    elif case == "duplicate_connector":
        second_manifest_path = target / pack["modules"][1]["manifest_path"]
        second_manifest = json.loads(second_manifest_path.read_text(encoding="utf-8"))
        second_manifest["connectors"][0]["connector_id"] = "connector_core_shell_01"
        _write_json(second_manifest_path, second_manifest)
    elif case == "pack_mismatch":
        manifest_path = target / first_entry["manifest_path"]
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["pack_id"] = "pack_wrong_v1"
        _write_json(manifest_path, manifest)
    else:
        raise AssertionError(f"unknown negative case: {case}")
    try:
        validate_module_pack(target, release=True)
    except ModulePackValidationError:
        return True
    return False


def _triangle_glb(material_name: str) -> bytes:
    positions = struct.pack(
        "<9f",
        -0.05,
        -0.025,
        0.0,
        0.05,
        -0.025,
        0.0,
        0.0,
        0.025,
        0.01,
    )
    texcoords = struct.pack("<6f", 0.0, 0.0, 1.0, 0.0, 0.5, 1.0)
    indices = struct.pack("<3H", 0, 1, 2)
    binary = positions + texcoords + indices
    binary += b"\x00" * ((4 - len(binary) % 4) % 4)
    gltf = {
        "asset": {"version": "2.0", "generator": "ForgeCAD module-pack smoke"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": "module_root", "mesh": 0}],
        "meshes": [
            {
                "name": "module_mesh",
                "primitives": [
                    {
                        "attributes": {"POSITION": 0, "TEXCOORD_0": 1},
                        "indices": 2,
                        "material": 0,
                    }
                ],
            }
        ],
        "materials": [{"name": material_name}],
        "buffers": [{"byteLength": len(binary)}],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": len(positions), "target": 34962},
            {
                "buffer": 0,
                "byteOffset": len(positions),
                "byteLength": len(texcoords),
                "target": 34962,
            },
            {
                "buffer": 0,
                "byteOffset": len(positions) + len(texcoords),
                "byteLength": len(indices),
                "target": 34963,
            },
        ],
        "accessors": [
            {
                "bufferView": 0,
                "componentType": 5126,
                "count": 3,
                "type": "VEC3",
                "min": [-0.05, -0.025, 0.0],
                "max": [0.05, 0.025, 0.01],
            },
            {"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC2"},
            {"bufferView": 2, "componentType": 5123, "count": 3, "type": "SCALAR"},
        ],
    }
    json_chunk = json.dumps(gltf, separators=(",", ":")).encode("utf-8")
    json_chunk += b" " * ((4 - len(json_chunk) % 4) % 4)
    total_length = 12 + 8 + len(json_chunk) + 8 + len(binary)
    return (
        struct.pack("<4sII", b"glTF", 2, total_length)
        + struct.pack("<II", len(json_chunk), 0x4E4F534A)
        + json_chunk
        + struct.pack("<II", len(binary), 0x004E4942)
        + binary
    )


def _png_512() -> bytes:
    signature = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", 512, 512, 8, 6, 0, 0, 0)
    raw_scanline = b"\x00" + (b"\x30\x38\x44\xff" * 512)
    image_data = zlib.compress(raw_scanline * 512, level=9)
    return signature + _png_chunk(b"IHDR", ihdr) + _png_chunk(b"IDAT", image_data) + _png_chunk(b"IEND", b"")


def _png_chunk(chunk_type: bytes, payload: bytes) -> bytes:
    return (
        struct.pack(">I", len(payload))
        + chunk_type
        + payload
        + struct.pack(">I", zlib.crc32(chunk_type + payload) & 0xFFFFFFFF)
    )


def _write_json(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.exit(main())
