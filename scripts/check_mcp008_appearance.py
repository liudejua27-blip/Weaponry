#!/usr/bin/env python3
"""Exercise the bounded AppearanceProgram, GLB attributes, and fixed passes."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
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
    completed = subprocess.run([worker], input=json.dumps(payload) + "\n", text=True, capture_output=True, check=False, timeout=20)
    lines = [line for line in completed.stdout.splitlines() if line.strip()]
    if not lines:
        raise RuntimeError(f"worker returned no response: {completed.stderr[-400:]}")
    return json.loads(lines[-1])


def inspect_glb(encoded: str) -> tuple[str, dict[str, Any]]:
    raw = base64.b64decode(encoded, validate=True)
    if len(raw) < 28 or raw[:4] != b"glTF" or struct.unpack_from("<I", raw, 4)[0] != 2:
        raise RuntimeError("GLB header failed")
    if struct.unpack_from("<I", raw, 8)[0] != len(raw) or raw[16:20] != b"JSON":
        raise RuntimeError("GLB length/JSON chunk failed")
    json_len = struct.unpack_from("<I", raw, 12)[0]
    root = json.loads(raw[20 : 20 + json_len].decode())
    primitive = root["meshes"][0]["primitives"][0]
    attributes = primitive["attributes"]
    if "TEXCOORD_0" not in attributes or "TANGENT" not in attributes:
        raise RuntimeError("UV/tangent attributes are missing")
    lineage = root.get("extras", {}).get("forgecad", {})
    if lineage.get("uv_status") != "passed" or lineage.get("tangent_status") != "passed":
        raise RuntimeError("UV/tangent lineage failed")
    return hashlib.sha256(raw).hexdigest(), {"size_bytes": len(raw), "part_ids": lineage["part_ids"], "triangle_count": lineage["triangle_count"], "uv_status": lineage["uv_status"], "tangent_status": lineage["tangent_status"]}


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
