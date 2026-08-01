#!/usr/bin/env python3
"""VP203 typed geometry -> ShapeProgram -> restricted GLB/readback lineage Gate."""

from __future__ import annotations

import base64
import hashlib
import json
import subprocess
import warnings
from pathlib import Path
from typing import Any

warnings.simplefilter("ignore", DeprecationWarning)
from jsonschema import Draft202012Validator

from forgecad_agent.application.restricted_geometry_executor import (
    RestrictedGeometryExecutionRequest,
    RestrictedGeometryExecutor,
)
from forgecad_agent.application.geometry_worker import compile_shape_program


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_ROOT = ROOT / "packages" / "concept-spec" / "schemas"
FIXTURE_ROOT = ROOT / "packages" / "concept-spec" / "fixtures"
SOURCE_SCHEMA = json.loads((SCHEMA_ROOT / "forge-visual-geometry-program-v2.schema.json").read_text())
EXPANDED_SCHEMA = json.loads((SCHEMA_ROOT / "expanded-visual-geometry-dag-v1.schema.json").read_text())
SOURCE_VALIDATOR = Draft202012Validator(SOURCE_SCHEMA)
EXPANDED_GATE_SCHEMA = json.loads(json.dumps(EXPANDED_SCHEMA))
EXPANDED_GATE_SCHEMA["properties"]["expanded_program"] = {}
EXPANDED_VALIDATOR = Draft202012Validator(EXPANDED_GATE_SCHEMA)


def validate_source_fixture(fixture_id: str, fixture: dict[str, Any]) -> None:
    errors = sorted(SOURCE_VALIDATOR.iter_errors(fixture), key=lambda item: list(item.path))
    if errors:
        path = ".".join(str(item) for item in errors[0].path)
        raise AssertionError(
            f"VP203_SOURCE_SCHEMA_INVALID:{fixture_id}:{path}:{errors[0].message}"
        )


def rust_results() -> list[dict[str, Any]]:
    for name in ("bracket", "rotor", "duct"):
        fixture = json.loads((FIXTURE_ROOT / f"forge-visual-geometry-v2-{name}.json").read_text())
        validate_source_fixture(name, fixture)
    result = subprocess.run(
        [
            str(ROOT / "script" / "with_rust_toolchain.sh"),
            "cargo", "run", "--quiet", "--manifest-path",
            str(ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"),
            "-p", "forgecad-core", "--bin", "vp203_high_level_geometry_dump", "--offline",
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        timeout=180,
    )
    if result.returncode != 0:
        raise AssertionError(f"VP203_RUST_LOWERING_FAILED:{result.stderr[-3000:]}")
    payload = json.loads(result.stdout)
    results = payload.get("results")
    if not isinstance(results, list) or len(results) != 3:
        raise AssertionError("VP203_FIXTURE_CARDINALITY_INVALID")
    return results


def compile_fixture(fixture_id: str, lowering: dict[str, Any]) -> dict[str, Any]:
    dag = lowering.get("expanded_dag")
    if not isinstance(dag, dict) or dag.get("schema_version") != "ExpandedVisualGeometryDAG@1":
        raise AssertionError("VP203_EXPANDED_DAG_INVALID")
    errors = sorted(EXPANDED_VALIDATOR.iter_errors(dag), key=lambda item: list(item.path))
    if errors:
        raise AssertionError(f"VP203_EXPANDED_SCHEMA_INVALID:{fixture_id}:{errors[0].message}")
    source_errors = sorted(SOURCE_VALIDATOR.iter_errors(dag.get("expanded_program")), key=lambda item: list(item.path))
    if source_errors:
        raise AssertionError(f"VP203_EXPANDED_SOURCE_INVALID:{fixture_id}:{source_errors[0].message}")
    if dag.get("compiler_version") != "forgecad-core-vp203.1" or dag.get("id_algorithm_version") != "geometry-source-path-v1":
        raise AssertionError("VP203_COMPILER_OR_ID_VERSION_INVALID")
    if lowering.get("source_program_sha256") != dag.get("source_program_sha256"):
        raise AssertionError("VP203_SOURCE_HASH_JOIN_INVALID")
    if dag.get("source_program_sha256") != dag.get("expanded_program_sha256"):
        raise AssertionError("VP203_IDENTITY_EXPANSION_HASH_INVALID")
    source_map = lowering.get("source_map")
    if not isinstance(source_map, list) or not source_map:
        raise AssertionError("VP203_SOURCE_MAP_INVALID")
    lineage = dag.get("lineage")
    if not isinstance(lineage, list):
        raise AssertionError("VP203_NODE_LINEAGE_INVALID")
    lineage_pairs = {(item.get("source_node_id"), item.get("expanded_node_id")) for item in lineage if isinstance(item, dict)}
    for entry in source_map:
        if entry.get("source_node_ids") != entry.get("expanded_node_ids"):
            raise AssertionError("VP203_SOURCE_EXPANDED_JOIN_INVALID")
        if any((node_id, node_id) not in lineage_pairs for node_id in entry.get("source_node_ids", [])):
            raise AssertionError("VP203_EXPANDED_NODE_LINEAGE_MISSING")

    try:
        compile_shape_program(lowering["shape_program"])
    except Exception as exc:
        raise AssertionError(f"VP203_DIRECT_WORKER_REJECTED:{fixture_id}:{type(exc).__name__}:{exc}") from exc
    request = RestrictedGeometryExecutionRequest.model_validate({
        "schema_version": "RestrictedGeometryExecutionRequest@1",
        "protocol_version": "forgecad.restricted-geometry/1",
        "execution_id": f"exec_vp203_{fixture_id}",
        "idempotency_key": f"idem_vp203_{fixture_id}",
        "cancellation_id": f"cancel_vp203_{fixture_id}",
        "cancellation_token": f"token_vp203_{fixture_id}",
        "action": "compile_readback",
        "timeout_ms": 120_000,
        "artifact_profile_id": "interactive_preview",
        "shape_program": lowering.get("shape_program"),
    })
    compiled = RestrictedGeometryExecutor(environment={}).execute(request)
    if compiled.glb_base64 is None or compiled.readback is None:
        raise AssertionError("VP203_RESTRICTED_COMPILE_INVALID")
    glb = base64.b64decode(compiled.glb_base64, validate=True)
    if hashlib.sha256(glb).hexdigest() != compiled.glb_sha256:
        raise AssertionError("VP203_GLB_HASH_INVALID")
    if lowering.get("shape_program_sha256") != hashlib.sha256(
        json.dumps(lowering["shape_program"], ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest():
        raise AssertionError("VP203_SHAPE_PROGRAM_HASH_INVALID")
    feature_history = compiled.readback.get("feature_history")
    if not isinstance(feature_history, list):
        raise AssertionError("VP203_FEATURE_HISTORY_MISSING")
    history_ids = {item.get("node_id") for item in feature_history if isinstance(item, dict)}
    zone_faces = compiled.readback.get("material_zone_faces")
    if not isinstance(zone_faces, list):
        raise AssertionError("VP203_ZONE_READBACK_MISSING")
    for entry in source_map:
        if not set(entry.get("shape_operation_ids", [])).issubset(history_ids):
            raise AssertionError("VP203_SHAPE_OPERATION_LINEAGE_MISSING")
        if not any(
            face.get("material_zone_id") == entry.get("material_zone_id")
            and set(face.get("source_operation_ids", []))
            & set(entry.get("shape_operation_ids", []))
            for face in zone_faces if isinstance(face, dict)
        ):
            raise AssertionError(
                f"VP203_DYNAMIC_ZONE_JOIN_MISSING:{fixture_id}:{entry.get('output_id')}:"
                f"{entry.get('terminal_operation_id')}:{entry.get('material_zone_id')}:"
                f"{[(item.get('material_zone_id'), item.get('source_operation_ids')) for item in zone_faces if isinstance(item, dict)]}"
            )
    triangles = compiled.readback.get("triangle_count")
    primitives = compiled.readback.get("primitive_count")
    if not isinstance(triangles, int) or triangles <= 0 or not isinstance(primitives, int) or primitives <= 0:
        raise AssertionError("VP203_GEOMETRY_READBACK_INVALID")
    return {
        "fixture_id": fixture_id,
        "triangles": triangles,
        "primitives": primitives,
        "parts": tuple(sorted(entry["part_id"] for entry in source_map)),
        "zones": tuple(sorted(entry["material_zone_id"] for entry in source_map)),
        "operations": tuple(item["op"] for item in lowering["shape_program"]["operations"]),
        "glb_sha256": compiled.glb_sha256,
    }


def main() -> int:
    coverage_fixture = json.loads(
        (FIXTURE_ROOT / "forge-visual-geometry-v2-operation-coverage.json").read_text()
    )
    validate_source_fixture("operation_coverage", coverage_fixture)
    fingerprints = [compile_fixture(item["fixture_id"], item["lowering"]) for item in rust_results()]
    if len({item["operations"] for item in fingerprints}) != 3:
        raise AssertionError("VP203_TOPOLOGY_FINGERPRINTS_NOT_DISTINCT")
    if len({item["parts"] for item in fingerprints}) != 3 or len({item["zones"] for item in fingerprints}) != 3:
        raise AssertionError("VP203_PART_OR_MATERIAL_FINGERPRINTS_NOT_DISTINCT")
    if len({item["glb_sha256"] for item in fingerprints}) != 3:
        raise AssertionError("VP203_GLB_FINGERPRINTS_NOT_DISTINCT")
    print("VP203 high-level geometry gate passed: " + "; ".join(
        f"{item['fixture_id']}={item['triangles']}t/{item['primitives']}p/{item['glb_sha256']}"
        for item in fingerprints
    ))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
