#!/usr/bin/env python3
"""Exercise the standalone bounded geometry worker with deterministic fixtures."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import struct
import subprocess
from typing import Any


def canonical(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def program() -> dict[str, Any]:
    value: dict[str, Any] = {
        "schema_version": "GeometryProgram@1",
        "project_id": "mcp007-worker-fixture",
        "representation_plan_sha256": "e" * 64,
        "nodes": [
            {"node_id": "torso", "operator_id": "forgecad.geometry.primitive@1", "part_id": "torso", "parameters": {"shape": "box", "size": [1.2, 1.6, 0.55], "position": [0, 1.7, 0], "material_zone_id": "zone-white-shell"}},
            {"node_id": "core", "operator_id": "forgecad.geometry.primitive@1", "part_id": "core", "parameters": {"shape": "cylinder", "size": [0.55, 1.2, 0.55], "position": [0, 1.5, 0], "material_zone_id": "zone-black-mechanical", "segments": 16}},
            {"node_id": "head", "operator_id": "forgecad.geometry.primitive@1", "part_id": "head", "parameters": {"shape": "sphere", "size": [0.85, 0.9, 0.85], "position": [0, 2.75, 0], "material_zone_id": "zone-white-shell", "segments": 16}},
        ],
        "budgets": {"max_nodes": 8, "max_triangles": 20000, "max_runtime_ms": 1000},
    }
    value["canonical_sha256"] = hashlib.sha256(canonical(value)).hexdigest()
    return value


def request(value: dict[str, Any]) -> dict[str, Any]:
    return {"protocol": "forgecad-worker-protocol@1", "request_id": "mcp007-fixture", "operation": "compile_geometry", "payload": {"geometry_program": value}}


def run(worker: str, payload: dict[str, Any]) -> dict[str, Any]:
    completed = subprocess.run([worker], input=json.dumps(payload) + "\n", text=True, capture_output=True, check=False, timeout=20)
    lines = [line for line in completed.stdout.splitlines() if line.strip()]
    if not lines:
        raise RuntimeError("geometry worker returned no response")
    return json.loads(lines[-1])


def validate_glb(encoded: str) -> tuple[str, dict[str, Any]]:
    raw = base64.b64decode(encoded, validate=True)
    if len(raw) < 28 or raw[:4] != b"glTF" or struct.unpack_from("<I", raw, 4)[0] != 2:
        raise RuntimeError("GLB header failed")
    if struct.unpack_from("<I", raw, 8)[0] != len(raw) or raw[16:20] != b"JSON":
        raise RuntimeError("GLB length/JSON chunk failed")
    json_len = struct.unpack_from("<I", raw, 12)[0]
    root = json.loads(raw[20 : 20 + json_len].decode("utf-8"))
    lineage = root.get("extras", {}).get("forgecad", {})
    if lineage.get("schema_version") != "ArtifactReadback@1" or not lineage.get("part_ids") or not lineage.get("triangle_count"):
        raise RuntimeError("GLB lineage failed")
    return hashlib.sha256(raw).hexdigest(), {"size_bytes": len(raw), "part_ids": lineage["part_ids"], "triangle_count": lineage["triangle_count"]}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--worker", required=True)
    options = parser.parse_args()
    first = run(options.worker, request(program()))
    second = run(options.worker, request(program()))
    if not first.get("ok") or first.get("result") != second.get("result"):
        raise SystemExit("deterministic geometry worker fixture failed")
    artifact_hash, readback = validate_glb(first["result"]["glb_base64"])
    if readback["part_ids"] != first["result"]["part_ids"] or readback["triangle_count"] != first["result"]["triangle_count"]:
        raise SystemExit("GLB readback does not match worker metadata")
    invalid = program()
    invalid["nodes"][0]["operator_id"] = "forgecad.geometry.python@1"
    invalid["canonical_sha256"] = hashlib.sha256(canonical({key: value for key, value in invalid.items() if key != "canonical_sha256"})).hexdigest()
    negative = run(options.worker, request(invalid))
    if negative.get("ok") or negative.get("error", {}).get("code") != "GEOMETRY_REJECTED":
        raise SystemExit("unsupported operator did not fail closed")
    print(json.dumps({"status": "PASS", "artifact_sha256": artifact_hash, "program_sha256": first["result"]["program_sha256"], **readback, "negative_operator": "PASS"}, ensure_ascii=False, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
