#!/usr/bin/env python3
"""VP201 Rust lowering -> restricted worker -> GLB/readback smoke."""

from __future__ import annotations

import base64
import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any

from forgecad_agent.application.restricted_geometry_executor import (
    RestrictedGeometryExecutionRequest,
    RestrictedGeometryExecutor,
)


ROOT = Path(__file__).resolve().parents[1]


def _rust_lowering() -> dict[str, Any]:
    command = [
        str(ROOT / "script" / "with_rust_toolchain.sh"),
        "cargo",
        "run",
        "--quiet",
        "--manifest-path",
        str(ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"),
        "-p",
        "forgecad-core",
        "--bin",
        "vp201_visual_program_dump",
        "--offline",
    ]
    result = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        timeout=180,
    )
    if result.returncode != 0:
        raise AssertionError(f"VP201_RUST_LOWERING_FAILED:{result.stderr[-1600:]}")
    payload = json.loads(result.stdout)
    if payload.get("schema_version") != "ForgeVisualProgramLowering@2":
        raise AssertionError("VP201_LOWERING_SCHEMA_INVALID")
    if payload.get("compiler_version") != "forgecad-core-vp201.2":
        raise AssertionError("VP201_COMPILER_VERSION_INVALID")
    source_map = payload.get("source_map")
    if not isinstance(source_map, dict) or source_map.get("schema_version") != "ForgeVisualSourceMap@1":
        raise AssertionError("VP201_SOURCE_MAP_INVALID")
    if source_map.get("source_program_sha256") != payload.get("source_program_sha256"):
        raise AssertionError("VP201_SOURCE_MAP_HASH_INVALID")
    canonical_source_map = json.dumps(
        source_map, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    if hashlib.sha256(canonical_source_map).hexdigest() != payload.get("source_map_sha256"):
        raise AssertionError("VP201_SOURCE_MAP_CANONICAL_HASH_INVALID")
    return payload


def main() -> int:
    lowering = _rust_lowering()
    shape_program = lowering.get("shape_program")
    if not isinstance(shape_program, dict) or shape_program.get("schema_version") != "ShapeProgram@1":
        raise AssertionError("VP201_SHAPE_PROGRAM_INVALID")

    request = RestrictedGeometryExecutionRequest.model_validate(
        {
            "schema_version": "RestrictedGeometryExecutionRequest@1",
            "protocol_version": "forgecad.restricted-geometry/1",
            "execution_id": "exec_vp201_minimal",
            "idempotency_key": "idem_vp201_minimal",
            "cancellation_id": "cancel_vp201_minimal",
            "cancellation_token": "token_vp201_minimal",
            "action": "compile_readback",
            "timeout_ms": 120_000,
            "artifact_profile_id": "interactive_preview",
            "shape_program": shape_program,
        }
    )
    result = RestrictedGeometryExecutor(environment={}).execute(request)
    if result.glb_base64 is None or result.readback is None:
        raise AssertionError("VP201_RESTRICTED_COMPILE_RESULT_INVALID")
    glb = base64.b64decode(result.glb_base64, validate=True)
    if hashlib.sha256(glb).hexdigest() != result.glb_sha256:
        raise AssertionError("VP201_GLB_HASH_INVALID")
    triangle_count = result.readback.get("triangle_count")
    if not isinstance(triangle_count, int) or triangle_count <= 0:
        raise AssertionError("VP201_READBACK_TRIANGLES_INVALID")
    bounds_mm = result.readback.get("bounds_mm")
    if not isinstance(bounds_mm, list) or len(bounds_mm) != 3 or bounds_mm == [120.0, 48.0, 32.0]:
        raise AssertionError("VP201_NON_ZERO_TRANSFORM_NOT_COMPILED")
    entries = lowering["source_map"].get("entries")
    if not isinstance(entries, list) or len(entries) != 1:
        raise AssertionError("VP201_SOURCE_MAP_ENTRY_INVALID")
    entry = entries[0]
    operation_id = entry.get("operation_id")
    material_zone_id = entry.get("material_zone_id")
    if not isinstance(operation_id, str) or not isinstance(material_zone_id, str):
        raise AssertionError("VP201_SOURCE_MAP_TARGET_INVALID")
    surface_provenance = result.readback.get("surface_provenance")
    zone_faces = result.readback.get("material_zone_faces")
    if not isinstance(surface_provenance, list) or not any(
        item.get("material_zone_id") == material_zone_id
        and operation_id in item.get("source_operation_ids", [])
        and item.get("part_role") == "primary_form"
        for item in surface_provenance
        if isinstance(item, dict)
    ):
        raise AssertionError("VP201_SURFACE_READBACK_LINEAGE_INVALID")
    if not isinstance(zone_faces, list) or not any(
        item.get("material_zone_id") == material_zone_id
        and item.get("material_id") == entry.get("compiled_material_id") == "mat_graphite"
        and operation_id in item.get("source_operation_ids", [])
        for item in zone_faces
        if isinstance(item, dict)
    ):
        raise AssertionError("VP201_MATERIAL_ZONE_READBACK_LINEAGE_INVALID")

    if entry.get("part_id") != "part_primary" or entry.get("material_zone_id") != "zone_primary":
        raise AssertionError("VP201_SOURCE_LINEAGE_INVALID")
    if entry.get("authored_material_id") != "mat_shell":
        raise AssertionError("VP201_AUTHORED_MATERIAL_LINEAGE_INVALID")
    if entry.get("source_node_ids") != [
        "node_primary_shell",
        "node_primary_transform",
        "node_primary_part",
        "node_primary_zone",
    ]:
        raise AssertionError("VP201_TYPED_GRAPH_SOURCE_LINEAGE_INVALID")

    print(
        "VP201 ForgeVisualProgram@2 gate passed: "
        f"triangles={triangle_count}, glb_sha256={result.glb_sha256}, "
        f"source_program_sha256={lowering['source_program_sha256']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
