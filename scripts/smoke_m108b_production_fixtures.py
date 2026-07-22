#!/usr/bin/env python3
"""Compile all frozen M108B production Recipe roots through Rust then Python.

No Provider is configured or contacted.  The Rust helper emits only expanded
ShapePrograms; the restricted executor receives no registry, project, object
store or Snapshot context.
"""
from __future__ import annotations

import base64, hashlib, json, subprocess, sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps/agent"))
from forgecad_agent.application.restricted_geometry_executor import RestrictedGeometryExecutionRequest, RestrictedGeometryExecutor

ROOT_IDS = {
    "recipe_prop_scout_compact", "recipe_prop_ceremonial_heavy", "recipe_prop_racing_streamlined",
    "recipe_vehicle_compact_coupe", "recipe_vehicle_utility_crossover", "recipe_vehicle_track_concept",
    "recipe_aircraft_streamlined_personal", "recipe_aircraft_explorer_tilt", "recipe_aircraft_cargo_display",
    "recipe_arm_desktop_assistant", "recipe_arm_gallery_industrial", "recipe_arm_service_display",
}
LOCK_PATH = ROOT / "packages/concept-spec/fixtures/component-recipes/locks/m108b-production-v1.lock.json"

def fail(code: str, detail: str) -> None: raise AssertionError(f"{code}: {detail}")

def expanded() -> dict:
    run = subprocess.run([str(ROOT / "script/with_rust_toolchain.sh"), "cargo", "run", "--quiet", "--manifest-path", str(ROOT / "apps/desktop/src-tauri/Cargo.toml"), "-p", "forgecad-core", "--bin", "m108b_recipe_dump", "--offline"], cwd=ROOT, text=True, capture_output=True, timeout=600)
    if run.returncode: fail("M108B_RUST_EXPANSION_FAILED", run.stderr[-2000:])
    try: value = json.loads(run.stdout)
    except json.JSONDecodeError as exc: fail("M108B_RUST_EXPANSION_INVALID", str(exc))
    if value.get("schema_version") != "M108BProductionRecipeExpansion@1": fail("M108B_RUST_EXPANSION_INVALID", "schema")
    return value

def main() -> int:
    lock = json.loads(LOCK_PATH.read_text(encoding="utf-8"))
    expected_parts = lock.get("expected_parts_by_root", {})
    expected_zones = lock.get("expected_unique_zones_by_root", {})
    if set(expected_parts) != ROOT_IDS or set(expected_zones) != ROOT_IDS:
        fail("M108B_EXPANDED_FACT_LOCK_INVALID", str(LOCK_PATH))
    fixture = expanded(); candidates = fixture.get("candidates", [])
    if fixture.get("registry_sha256") != lock.get("registry_sha256"):
        fail("M108B_REGISTRY_LOCK_DRIFT", str(fixture.get("registry_sha256")))
    if len(candidates) != 12: fail("M108B_ROOT_COUNT_INVALID", str(len(candidates)))
    executor = RestrictedGeometryExecutor(environment={}); hashes: set[str] = set(); reports = []
    for index, candidate in enumerate(candidates):
        root = candidate.get("recipe", {}).get("recipe_id")
        if root not in ROOT_IDS: fail("M108B_ROOT_ID_INVALID", str(root))
        graph = candidate.get("expanded_assembly_graph", {}); parts = graph.get("parts", [])
        if len(parts) != expected_parts[root]: fail("M108B_PART_COUNT_INVALID", f"{root}:{len(parts)}!={expected_parts[root]}")
        if len(candidate.get("component_recipe_instances", [])) != len(parts): fail("M108B_INSTANCE_DRIFT", str(root))
        unique_zones = {z for part in parts for z in part.get("material_zone_ids", [])}
        if len(unique_zones) != expected_zones[root]: fail("M108B_ZONE_COUNT_INVALID", f"{root}:{len(unique_zones)}!={expected_zones[root]}:{sorted(unique_zones)}")
        request = RestrictedGeometryExecutionRequest.model_validate({"schema_version":"RestrictedGeometryExecutionRequest@1","protocol_version":"forgecad.restricted-geometry/1","execution_id":f"m108b_{index}","idempotency_key":f"m108b_idem_{index}","cancellation_id":f"m108b_cancel_{index}","cancellation_token":f"m108b_token_{index}","action":"compile_readback","timeout_ms":120000,"artifact_profile_id":"production_concept","shape_program":candidate["expanded_shape_program"]})
        try:
            result = executor.execute(request)
        except Exception as exc:
            fail("M108B_COMPILE_FAILED", f"{root}: {exc}")
        if not result.glb_base64 or not result.readback: fail("M108B_READBACK_MISSING", str(root))
        glb = base64.b64decode(result.glb_base64)
        if hashlib.sha256(glb).hexdigest() != result.glb_sha256 or result.glb_sha256 in hashes: fail("M108B_GLB_HASH_INVALID", str(root))
        hashes.add(result.glb_sha256)
        readback = result.readback
        surfaces = readback.get("surface_provenance")
        if not isinstance(surfaces, list) or not surfaces:
            fail("M108B_SURFACE_PROVENANCE_MISSING", str(root))
        for surface in surfaces:
            if surface.get("closed") is not True or any(int(surface.get(key, 1)) != 0 for key in ("boundary_edge_count", "non_manifold_edge_count", "degenerate_triangle_count")):
                fail("M108B_TOPOLOGY_INVALID", f"{root}:{surface}")
        primitive_count = int(readback.get("primitive_count", 0))
        if primitive_count < 1 or any(int(readback.get(key, 0)) != primitive_count for key in ("normal_primitive_count", "uv0_primitive_count", "tangent_primitive_count")):
            fail("M108B_VERTEX_FACTS_INVALID", str(root))
        if primitive_count > 96 or int(readback.get("mesh_count", 0)) > 72:
            fail("M108B_RENDERER_BUDGET_INVALID", f"{root}:meshes={readback.get('mesh_count')}:primitives={primitive_count}")
        if readback.get("artifact_profile", {}).get("artifact_profile_id") != "production_concept": fail("M108B_PROFILE_INVALID", str(root))
        triangles = int(result.triangle_count)
        if not 8000 <= triangles <= 24000: fail("M108B_TRIANGLE_BUDGET_INVALID", f"{root}:{triangles}")
        observed = {x.get("material_zone_id") for x in readback.get("material_zone_faces", [])}
        if not unique_zones.issubset(observed): fail("M108B_ZONE_READBACK_DRIFT", f"{root}:{observed}")
        reports.append({"recipe_id":root,"parts":len(parts),"zones":len(unique_zones),"triangles":triangles,"glb_sha256":result.glb_sha256})
    print("M108B production fixtures passed: roots=12, provider_calls=0, reports=" + json.dumps(reports, ensure_ascii=False, separators=(",",":")), file=sys.stderr, flush=True)
    return 0

if __name__ == "__main__": raise SystemExit(main())
