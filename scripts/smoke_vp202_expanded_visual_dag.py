#!/usr/bin/env python3
"""VP202 composition -> ExpandedVisualDAG -> VP201 -> GLB/readback smoke."""

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


def _rust_result() -> dict[str, Any]:
    result = subprocess.run(
        [
            str(ROOT / "script" / "with_rust_toolchain.sh"),
            "cargo", "run", "--quiet", "--manifest-path",
            str(ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"),
            "-p", "forgecad-core", "--bin", "vp202_expanded_visual_dag_dump", "--offline",
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        timeout=180,
    )
    if result.returncode != 0:
        raise AssertionError(f"VP202_RUST_EXPANSION_FAILED:{result.stderr[-2000:]}")
    return json.loads(result.stdout)


def main() -> int:
    result = _rust_result()
    dag = result.get("expanded_dag")
    lowering = result.get("lowering")
    if not isinstance(dag, dict) or dag.get("schema_version") != "ExpandedVisualDAG@1":
        raise AssertionError("VP202_EXPANDED_DAG_INVALID")
    if dag.get("compiler_version") != "forgecad-core-vp202.1":
        raise AssertionError("VP202_COMPILER_VERSION_INVALID")
    if dag.get("id_algorithm_version") != "expanded-path-v1":
        raise AssertionError("VP202_ID_ALGORITHM_INVALID")
    if not isinstance(lowering, dict) or lowering.get("source_program_sha256") != dag.get("expanded_program_sha256"):
        raise AssertionError("VP202_VP201_HASH_JOIN_INVALID")
    lineage = dag.get("lineage")
    source_entries = lowering.get("source_map", {}).get("entries")
    if not isinstance(lineage, list) or len(lineage) != 3 or not isinstance(source_entries, list) or len(source_entries) != 3:
        raise AssertionError("VP202_LINEAGE_CARDINALITY_INVALID")
    lineage_by_output = {entry["expanded_output_id"]: entry for entry in lineage}
    for entry in source_entries:
        expanded = lineage_by_output.get(entry.get("output_id"))
        if not expanded:
            raise AssertionError("VP202_SOURCE_MAP_JOIN_MISSING")
        if entry.get("part_id") != expanded.get("expanded_part_id"):
            raise AssertionError("VP202_PART_LINEAGE_INVALID")
        if entry.get("material_zone_id") != expanded.get("expanded_material_zone_id"):
            raise AssertionError("VP202_ZONE_LINEAGE_INVALID")
        if entry.get("source_node_ids") != expanded.get("expanded_node_ids"):
            raise AssertionError("VP202_NODE_LINEAGE_INVALID")

    shape_program = lowering.get("shape_program")
    request = RestrictedGeometryExecutionRequest.model_validate({
        "schema_version": "RestrictedGeometryExecutionRequest@1",
        "protocol_version": "forgecad.restricted-geometry/1",
        "execution_id": "exec_vp202_repeat",
        "idempotency_key": "idem_vp202_repeat",
        "cancellation_id": "cancel_vp202_repeat",
        "cancellation_token": "token_vp202_repeat",
        "action": "compile_readback",
        "timeout_ms": 120_000,
        "artifact_profile_id": "interactive_preview",
        "shape_program": shape_program,
    })
    compiled = RestrictedGeometryExecutor(environment={}).execute(request)
    if compiled.glb_base64 is None or compiled.readback is None:
        raise AssertionError("VP202_RESTRICTED_COMPILE_INVALID")
    glb = base64.b64decode(compiled.glb_base64, validate=True)
    if hashlib.sha256(glb).hexdigest() != compiled.glb_sha256:
        raise AssertionError("VP202_GLB_HASH_INVALID")
    zone_faces = compiled.readback.get("material_zone_faces")
    if not isinstance(zone_faces, list):
        raise AssertionError("VP202_ZONE_READBACK_INVALID")
    for source_entry in source_entries:
        if not any(
            item.get("material_zone_id") == source_entry.get("material_zone_id")
            and source_entry.get("operation_id") in item.get("source_operation_ids", [])
            for item in zone_faces if isinstance(item, dict)
        ):
            raise AssertionError("VP202_GLB_DYNAMIC_JOIN_INVALID")
    triangles = compiled.readback.get("triangle_count")
    if not isinstance(triangles, int) or triangles <= 0:
        raise AssertionError("VP202_TRIANGLE_READBACK_INVALID")
    print(
        "VP202 ExpandedVisualDAG gate passed: "
        f"outputs={len(source_entries)}, triangles={triangles}, "
        f"source_sha256={dag['source_program_sha256']}, "
        f"expanded_sha256={dag['expanded_program_sha256']}, glb_sha256={compiled.glb_sha256}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
