#!/usr/bin/env python3
"""Rust→restricted-Python temporary source-pack smoke for M108B future props."""
from __future__ import annotations

import base64
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACK = ROOT / "packages/concept-spec/fixtures/component-recipes/production-packs/future-weapon-prop.json"
sys.path.insert(0, str(ROOT / "apps/agent"))
from forgecad_agent.application.restricted_geometry_executor import RestrictedGeometryExecutionRequest, RestrictedGeometryExecutor
from forgecad_agent.application.shape_program import validate_shape_program


def fail(code: str, detail: str) -> None:
    raise AssertionError(f"{code}: {detail}")


def main() -> int:
    pack = json.loads(PACK.read_text(encoding="utf-8"))
    registry = {
        "schema_version": "EditableComponentRecipeRegistry@1",
        "registry_id": "registry_m108b_future_prop_source_check_v1",
        "policy_version": "ComponentRecipePolicy@1",
        "recipes": pack["recipes"],
    }
    with tempfile.TemporaryDirectory(prefix="forgecad_m108b_future_prop_") as directory:
        source_registry = Path(directory) / "registry.json"
        source_registry.write_text(json.dumps(registry, ensure_ascii=False), encoding="utf-8")
        run = subprocess.run(
            [str(ROOT / "script/with_rust_toolchain.sh"), "cargo", "run", "--quiet", "--manifest-path", str(ROOT / "apps/desktop/src-tauri/Cargo.toml"), "-p", "forgecad-core", "--bin", "m108b_future_prop_recipe_dump", "--offline", "--", str(source_registry)],
            cwd=ROOT, text=True, capture_output=True, timeout=600,
        )
        if run.returncode:
            fail("M108B_PROP_RUST_SOURCE_PACK_INVALID", run.stderr[-3000:])
        try:
            expansion = json.loads(run.stdout)
        except json.JSONDecodeError as error:
            fail("M108B_PROP_RUST_SOURCE_PACK_INVALID", str(error))
    if expansion.get("schema_version") != "M108BFuturePropSourcePackExpansion@1":
        fail("M108B_PROP_EXPANSION_INVALID", "schema")
    candidates = expansion.get("candidates")
    if not isinstance(candidates, list) or len(candidates) != 3:
        fail("M108B_PROP_EXPANSION_INVALID", "root count")
    executor = RestrictedGeometryExecutor(environment={})
    reports = []
    for index, candidate in enumerate(candidates):
        graph = candidate.get("expanded_assembly_graph", {})
        parts = graph.get("parts", [])
        zones = {zone for part in parts for zone in part.get("material_zone_ids", [])}
        instance_recipes = {
            instance.get("recipe", {}).get("recipe_id")
            for instance in candidate.get("component_recipe_instances", [])
            if isinstance(instance, dict)
        }
        operations = candidate.get("expanded_shape_program", {}).get("operations", [])
        op_names = {operation.get("op") for operation in operations if isinstance(operation, dict)}
        inputs = candidate.get("expanded_shape_program", {}).get("profile_inputs", [])
        if len(parts) not in range(7, 10) or len(zones) not in range(4, 6):
            fail("M108B_PROP_PART_ZONE_CONTRACT", f"parts={len(parts)} zones={len(zones)}")
        if not {"recipe_m108b_prop_accent", "recipe_m108b_prop_fin", "recipe_m108b_prop_rib"}.issubset(instance_recipes):
            fail("M108B_PROP_CHILD_VARIETY_MISSING", str(sorted(instance_recipes)))
        if not {"loft", "sweep", "revolve", "surface_panel"}.issubset(op_names):
            fail("M108B_PROP_OPERATION_CONTRACT", str(sorted(op_names)))
        if not any(item.get("input_kind") == "profile_section_set" for item in inputs if isinstance(item, dict)):
            fail("M108B_PROP_SECTION_SET_MISSING", "no ProfileSectionSet input")
        request = RestrictedGeometryExecutionRequest.model_validate({
            "schema_version": "RestrictedGeometryExecutionRequest@1", "protocol_version": "forgecad.restricted-geometry/1",
            "execution_id": f"m108b_prop_source_{index}", "idempotency_key": f"m108b_prop_source_idem_{index}",
            "cancellation_id": f"m108b_prop_source_cancel_{index}", "cancellation_token": f"m108b_prop_source_token_{index}",
            "action": "compile_readback", "timeout_ms": 120000, "artifact_profile_id": "production_concept",
            "shape_program": candidate["expanded_shape_program"],
        })
        try:
            validate_shape_program(candidate["expanded_shape_program"])
        except Exception as error:
            fail("M108B_PROP_SHAPE_PROGRAM_INVALID", str(error))
        result = executor.execute(request)
        glb = base64.b64decode(result.glb_base64)
        if hashlib.sha256(glb).hexdigest() != result.glb_sha256:
            fail("M108B_PROP_GLB_HASH_INVALID", str(index))
        triangles = int(result.triangle_count)
        if not 8_000 <= triangles <= 20_000:
            fail("M108B_PROP_TRIANGLE_BUDGET_INVALID", str(triangles))
        reports.append({"recipe_id": candidate["recipe"]["recipe_id"], "parts": len(parts), "zones": len(zones), "triangles": triangles, "glb_sha256": result.glb_sha256})
    print("M108B future-prop source-pack smoke passed: " + json.dumps(reports, ensure_ascii=False, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
