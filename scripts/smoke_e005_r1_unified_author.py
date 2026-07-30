#!/usr/bin/env python3
"""E005-R1 compact author -> Rust lowering -> restricted GLB/readback Gate."""

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
from referencing import Registry, Resource

from forgecad_agent.application.geometry_worker import compile_shape_program
from forgecad_agent.application.restricted_geometry_executor import (
    RestrictedGeometryExecutionRequest,
    RestrictedGeometryExecutor,
)


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_ROOT = ROOT / "packages" / "concept-spec" / "schemas"
FIXTURE = ROOT / "packages" / "concept-spec" / "fixtures" / "e005-r1-unified-service-console.json"
AUTHOR_SCHEMA = json.loads((SCHEMA_ROOT / "forge-visual-author-source-v1.schema.json").read_text())
GEOMETRY_SCHEMA = json.loads((SCHEMA_ROOT / "forge-visual-geometry-program-v2.schema.json").read_text())
SCHEMA_REGISTRY = Registry().with_resources([
    (AUTHOR_SCHEMA["$id"], Resource.from_contents(AUTHOR_SCHEMA)),
    (GEOMETRY_SCHEMA["$id"], Resource.from_contents(GEOMETRY_SCHEMA)),
])
AUTHOR_VALIDATOR = Draft202012Validator(AUTHOR_SCHEMA, registry=SCHEMA_REGISTRY)


def canonical_sha256(value: Any) -> str:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


def rust_lowering() -> tuple[dict[str, Any], dict[str, Any]]:
    source = json.loads(FIXTURE.read_text())
    errors = sorted(AUTHOR_VALIDATOR.iter_errors(source), key=lambda item: list(item.path))
    if errors:
        raise AssertionError(f"E005_R1_SOURCE_SCHEMA_INVALID:{list(errors[0].path)}:{errors[0].message}")
    result = subprocess.run(
        [
            str(ROOT / "script" / "with_rust_toolchain.sh"),
            "cargo", "run", "--quiet", "--manifest-path",
            str(ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"),
            "-p", "forgecad-core", "--bin", "e005_r1_unified_author_dump", "--offline",
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        timeout=180,
    )
    if result.returncode != 0:
        raise AssertionError(f"E005_R1_RUST_LOWERING_FAILED:{result.stderr[-3000:]}")
    payload = json.loads(result.stdout)
    if payload.get("source") != source or not isinstance(payload.get("lowering"), dict):
        raise AssertionError("E005_R1_RUST_SOURCE_JOIN_INVALID")
    return source, payload["lowering"]


def main() -> int:
    source, lowering = rust_lowering()
    if lowering.get("schema_version") != "ForgeVisualAuthorLowering@1":
        raise AssertionError("E005_R1_LOWERING_SCHEMA_INVALID")
    source_hash = lowering.get("source_program_sha256")
    if not isinstance(source_hash, str) or len(source_hash) != 64:
        raise AssertionError("E005_R1_SOURCE_HASH_INVALID")
    density = lowering.get("semantic_density")
    if not isinstance(density, dict):
        raise AssertionError("E005_R1_DENSITY_MISSING")
    if density.get("expanded_output_count") != 11 or density.get("detail_motif_instance_count") != 10:
        raise AssertionError("E005_R1_MOTIF_EXPANSION_INVALID")
    if density.get("node_expansion_ratio_bps", 0) <= 10_000:
        raise AssertionError("E005_R1_COMPACT_EXPANSION_NOT_PROVEN")

    shape_program = lowering.get("shape_program")
    if not isinstance(shape_program, dict) or lowering.get("shape_program_sha256") != canonical_sha256(shape_program):
        raise AssertionError("E005_R1_SHAPE_HASH_INVALID")
    compile_shape_program(shape_program)
    request = RestrictedGeometryExecutionRequest.model_validate({
        "schema_version": "RestrictedGeometryExecutionRequest@1",
        "protocol_version": "forgecad.restricted-geometry/1",
        "execution_id": "exec_e005_r1_unified_author",
        "idempotency_key": "idem_e005_r1_unified_author",
        "cancellation_id": "cancel_e005_r1_unified_author",
        "cancellation_token": "token_e005_r1_unified_author",
        "action": "compile_readback",
        "timeout_ms": 120_000,
        "artifact_profile_id": "interactive_preview",
        "shape_program": shape_program,
    })
    compiled = RestrictedGeometryExecutor(environment={}).execute(request)
    if compiled.glb_base64 is None or compiled.readback is None:
        raise AssertionError("E005_R1_RESTRICTED_COMPILE_INVALID")
    glb = base64.b64decode(compiled.glb_base64, validate=True)
    if hashlib.sha256(glb).hexdigest() != compiled.glb_sha256:
        raise AssertionError("E005_R1_GLB_HASH_INVALID")

    lineage = lowering.get("lineage")
    assembly = lowering.get("assembly_graph")
    surfaces = lowering.get("surface_plan", {}).get("bindings")
    if not isinstance(lineage, list) or not isinstance(assembly, dict) or not isinstance(surfaces, list):
        raise AssertionError("E005_R1_PRODUCT_TRUTH_MISSING")
    parts = assembly.get("parts")
    if not isinstance(parts, list) or len(parts) != len(lineage) or len(surfaces) != len(lineage):
        raise AssertionError("E005_R1_PRODUCT_CARDINALITY_INVALID")
    part_ids = {item.get("part_id") for item in parts if isinstance(item, dict)}
    output_ids = {item.get("expanded_output_id") for item in lineage if isinstance(item, dict)}
    if {item.get("part_id") for item in surfaces} != part_ids:
        raise AssertionError("E005_R1_SURFACE_PART_JOIN_INVALID")
    if {item.get("expanded_output_id") for item in surfaces} != output_ids:
        raise AssertionError("E005_R1_SURFACE_OUTPUT_JOIN_INVALID")

    readback = compiled.readback
    triangles = readback.get("triangle_count")
    primitives = readback.get("primitive_count")
    history_ids = {item.get("node_id") for item in readback.get("feature_history", []) if isinstance(item, dict)}
    operation_ids = {item.get("operation_id") for item in shape_program.get("operations", []) if isinstance(item, dict)}
    if not isinstance(triangles, int) or triangles <= 0 or not isinstance(primitives, int) or primitives < 11:
        raise AssertionError("E005_R1_GEOMETRY_READBACK_INVALID")
    if not operation_ids.issubset(history_ids):
        raise AssertionError("E005_R1_OPERATION_READBACK_JOIN_INVALID")
    print(
        "E005-R1 unified author gate passed: "
        f"{density['source_json_bytes']}B -> {density['expanded_node_count']} nodes / "
        f"{len(parts)} parts / {triangles} triangles / {primitives} primitives / {compiled.glb_sha256}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
