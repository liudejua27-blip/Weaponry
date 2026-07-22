#!/usr/bin/env python3
"""Create a non-formal M108B asset preflight from Rust-owned Recipe output.

This tool is intentionally *not* the M108B benchmark-kit builder.  It proves
the development hand-off that must precede it:

Rust production Recipe expansion -> capability-gated RestrictedGeometryExecutor
-> exact GLB + GeometryCompileReadback@2 -> three distinct automatic reports.

It never starts a Provider, never receives a credential, database, object-store
path, or Snapshot authority, and writes a source *draft* which cannot be used
as a formal M108B visual benchmark.  In particular the draft deliberately says
``frozen_before_scoring=false``, ``formal_eligible=false`` and leaves the
Workbench renderer capture pending.  A later, separately-reviewed freeze step
must create the formal source manifest after real captures exist.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from dataclasses import asdict
from pathlib import Path
from typing import Any, Mapping

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps/agent"))

from forgecad_agent.application.geometry_models import GeometryCompileReadback
from forgecad_agent.application.geometry_worker import read_shape_program_glb_facts
from forgecad_agent.application.restricted_geometry_executor import (
    RestrictedGeometryExecutionRequest,
    RestrictedGeometryExecutor,
)


DOMAINS = (
    "pack_future_weapon_prop",
    "pack_vehicle_concept",
    "pack_aircraft_concept",
    "pack_robotic_arm_concept",
)
GATES = {
    "m108a": "FGC-M108A",
    "q003": "FGC-Q003",
    "g826": "FGC-G826",
}
ROOT_IDS = {
    "recipe_prop_scout_compact",
    "recipe_prop_ceremonial_heavy",
    "recipe_prop_racing_streamlined",
    "recipe_vehicle_compact_coupe",
    "recipe_vehicle_utility_crossover",
    "recipe_vehicle_track_concept",
    "recipe_aircraft_streamlined_personal",
    "recipe_aircraft_explorer_tilt",
    "recipe_aircraft_cargo_display",
    "recipe_arm_desktop_assistant",
    "recipe_arm_gallery_industrial",
    "recipe_arm_service_display",
}
PBR_ROLES = {
    "base_color",
    "metallic_roughness",
    "normal",
    "occlusion",
    "emissive",
}


class PreflightError(ValueError):
    """A deterministic preflight fact is missing or has drifted."""


def _canonical_bytes(value: object) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False
    ).encode("utf-8")


def _sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def _read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise PreflightError(f"M108B_PREFLIGHT_INVALID_JSON:{path.name}") from error
    if not isinstance(value, dict):
        raise PreflightError(f"M108B_PREFLIGHT_INVALID_OBJECT:{path.name}")
    return value


def _safe_relative(value: object, *, suffix: str, field: str) -> Path:
    if not isinstance(value, str) or not value:
        raise PreflightError(f"M108B_PREFLIGHT_PATH_INVALID:{field}")
    path = Path(value)
    if path.is_absolute() or ".." in path.parts or "\\" in value or path.suffix != suffix:
        raise PreflightError(f"M108B_PREFLIGHT_PATH_INVALID:{field}")
    return path


def _run_rust_expansion() -> dict[str, Any]:
    command = [
        str(ROOT / "script/with_rust_toolchain.sh"),
        "cargo",
        "run",
        "--quiet",
        "--manifest-path",
        str(ROOT / "apps/desktop/src-tauri/Cargo.toml"),
        "-p",
        "forgecad-core",
        "--bin",
        "m108b_recipe_dump",
        "--offline",
    ]
    completed = subprocess.run(
        command, cwd=ROOT, text=True, capture_output=True, timeout=600, check=False
    )
    if completed.returncode:
        raise PreflightError(
            "M108B_PREFLIGHT_RUST_EXPANSION_FAILED:" + completed.stderr[-1600:]
        )
    try:
        value = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise PreflightError("M108B_PREFLIGHT_RUST_EXPANSION_INVALID") from error
    if not isinstance(value, dict) or value.get("schema_version") != "M108BProductionRecipeExpansion@1":
        raise PreflightError("M108B_PREFLIGHT_RUST_EXPANSION_INVALID")
    candidates = value.get("candidates")
    if not isinstance(candidates, list) or len(candidates) != 12:
        raise PreflightError("M108B_PREFLIGHT_ROOT_COUNT_INVALID")
    return value


def _slug(recipe_id: str) -> str:
    if not recipe_id.startswith("recipe_"):
        raise PreflightError("M108B_PREFLIGHT_RECIPE_ID_INVALID")
    return recipe_id.removeprefix("recipe_")


def _readback_hash(readback: Mapping[str, Any]) -> str:
    return _sha256(_canonical_bytes(dict(readback)))


def _assert_readback_matches_glb(
    *, glb: bytes, readback: Mapping[str, Any], expected_glb_sha256: str
) -> dict[str, Any]:
    # The compatibility port currently appends two legacy convenience flags.
    # Persist only the strict GeometryCompileReadback@2 contract; otherwise a
    # later schema reader would be forced to accept executor-only extras.
    typed_payload = {
        field: value
        for field, value in readback.items()
        if field in GeometryCompileReadback.model_fields
    }
    validated = GeometryCompileReadback.model_validate(typed_payload)
    # Optional A005 provenance is present only for a material that actually
    # carries a surface-adornment program.  The GLB reader intentionally omits
    # the pair for ordinary built-in materials; canonicalizing absent values
    # here keeps the comparison exact for present provenance without inventing
    # nullable facts that the GLB does not contain.
    raw = validated.model_dump(mode="json", exclude_none=True)
    if raw.get("schema_version") != "GeometryCompileReadback@2":
        raise PreflightError("M108B_PREFLIGHT_READBACK_VERSION_INVALID")
    if raw.get("glb_sha256") != expected_glb_sha256 or _sha256(glb) != expected_glb_sha256:
        raise PreflightError("M108B_PREFLIGHT_GLB_READBACK_HASH_DRIFT")
    if raw.get("glb_byte_size") != len(glb):
        raise PreflightError("M108B_PREFLIGHT_GLB_READBACK_SIZE_DRIFT")
    facts = asdict(read_shape_program_glb_facts(glb))
    for field in (
        "triangle_count",
        "bounds_mm",
        "mesh_count",
        "primitive_count",
        "material_count",
        "uv0_primitive_count",
        "normal_primitive_count",
        "tangent_primitive_count",
        "material_zone_faces",
        "visual_texture_sets",
        "visual_environment",
        "artifact_profile",
        "feature_history",
    ):
        if raw.get(field) != facts.get(field):
            raise PreflightError(f"M108B_PREFLIGHT_GLB_READBACK_FACT_DRIFT:{field}")
    # The typed readback carries nullable convenience fields which the GLB
    # reader intentionally omits when absent.  Every shared, present fact must
    # still agree, rather than comparing JSON key presence.
    for raw_surface, fact_surface in zip(raw["surface_provenance"], facts["surface_provenance"]):
        for field in set(raw_surface).intersection(fact_surface):
            if raw_surface[field] != fact_surface[field]:
                raise PreflightError(f"M108B_PREFLIGHT_GLB_SURFACE_FACT_DRIFT:{field}")
    return raw


def _m108a_checks(readback: Mapping[str, Any]) -> dict[str, Any]:
    profile = readback.get("artifact_profile")
    if not isinstance(profile, dict) or profile.get("artifact_profile_id") != "production_concept":
        raise PreflightError("M108B_PREFLIGHT_M108A_PROFILE_INVALID")
    checked_sets: list[str] = []
    for texture_set in readback.get("visual_texture_sets", []):
        if not isinstance(texture_set, dict):
            raise PreflightError("M108B_PREFLIGHT_M108A_PBR_INVALID")
        maps = texture_set.get("maps", [])
        roles = {item.get("texture_role") for item in maps if isinstance(item, dict)}
        if (
            not isinstance(texture_set.get("visual_texture_set_id"), str)
            or not texture_set["visual_texture_set_id"].endswith("_builtin_v4")
            or len(maps) != 5
            or roles != PBR_ROLES
            or any(item.get("width") != 1024 or item.get("height") != 1024 for item in maps)
        ):
            raise PreflightError("M108B_PREFLIGHT_M108A_PBR_INVALID")
        checked_sets.append(texture_set["visual_texture_set_id"])
    if not checked_sets:
        raise PreflightError("M108B_PREFLIGHT_M108A_PBR_INVALID")
    return {"artifact_profile_id": "production_concept", "texture_sets": sorted(checked_sets)}


def _q003_checks(readback: Mapping[str, Any]) -> dict[str, Any]:
    primitives = int(readback.get("primitive_count", 0))
    if primitives < 1 or any(
        int(readback.get(field, 0)) != primitives
        for field in ("normal_primitive_count", "uv0_primitive_count", "tangent_primitive_count")
    ):
        raise PreflightError("M108B_PREFLIGHT_Q003_VERTEX_FACTS_INVALID")
    surfaces = readback.get("surface_provenance")
    if not isinstance(surfaces, list) or len(surfaces) != primitives:
        raise PreflightError("M108B_PREFLIGHT_Q003_SURFACE_FACTS_INVALID")
    for surface in surfaces:
        if not isinstance(surface, dict) or surface.get("closed") is not True:
            raise PreflightError("M108B_PREFLIGHT_Q003_TOPOLOGY_INVALID")
        if any(int(surface.get(field, 1)) != 0 for field in (
            "boundary_edge_count", "non_manifold_edge_count", "degenerate_triangle_count",
            "uv_degenerate_triangle_count", "tangent_fallback_triangle_count",
        )):
            raise PreflightError("M108B_PREFLIGHT_Q003_TOPOLOGY_INVALID")
    return {"primitive_count": primitives, "triangle_count": int(readback["triangle_count"])}


def _g826_checks(readback: Mapping[str, Any]) -> dict[str, Any]:
    surfaces = readback.get("surface_provenance")
    zones = readback.get("material_zone_faces")
    features = readback.get("feature_history")
    if not isinstance(surfaces, list) or not isinstance(zones, list) or not isinstance(features, list):
        raise PreflightError("M108B_PREFLIGHT_G826_FACTS_INVALID")
    if not surfaces or len(surfaces) != len(zones) or not features:
        raise PreflightError("M108B_PREFLIGHT_G826_FACTS_INVALID")
    surface_ids = [item.get("primitive_id") for item in surfaces if isinstance(item, dict)]
    zone_ids = [item.get("primitive_id") for item in zones if isinstance(item, dict)]
    if surface_ids != zone_ids or len(surface_ids) != len(set(surface_ids)):
        raise PreflightError("M108B_PREFLIGHT_G826_PROVENANCE_DRIFT")
    if any(not item.get("texture_ready") for item in [*surfaces, *zones] if isinstance(item, dict)):
        raise PreflightError("M108B_PREFLIGHT_G826_TEXTURE_READINESS_INVALID")
    return {
        "surface_provenance_count": len(surfaces),
        "material_zone_face_count": len(zones),
        "feature_history_count": len(features),
    }


def _semantic_facts(candidate: Mapping[str, Any]) -> dict[str, list[str]]:
    graph = candidate.get("expanded_assembly_graph")
    program = candidate.get("expanded_shape_program")
    if not isinstance(graph, dict) or not isinstance(program, dict):
        raise PreflightError("M108B_PREFLIGHT_CANDIDATE_INVALID")
    parts = graph.get("parts", [])
    operations = program.get("operations", [])
    instances = candidate.get("component_recipe_instances", [])
    if not isinstance(parts, list) or not isinstance(operations, list) or not isinstance(instances, list):
        raise PreflightError("M108B_PREFLIGHT_CANDIDATE_INVALID")
    return {
        "roles": sorted({str(item["role"]) for item in parts if isinstance(item, dict) and item.get("role")}),
        "profiles": [str(item["operation_id"]) for item in operations if isinstance(item, dict) and item.get("op") == "profile"],
        "sections": [str(item["operation_id"]) for item in operations if isinstance(item, dict) and item.get("op") in {"loft", "sweep", "revolve"}],
        "features": [str(item["operation_id"]) for item in operations if isinstance(item, dict) and item.get("operation_id")],
        "child_slots": [str(item["parent_slot_id"]) for item in instances if isinstance(item, dict) and item.get("parent_slot_id")],
        "connectors": [str(item["connection_id"]) for item in graph.get("connections", []) if isinstance(item, dict) and item.get("connection_id")],
        "pivots": [str(item["part_id"]) for item in parts if isinstance(item, dict) and item.get("part_id")],
        "bindings": [str(binding["parameter_id"]) for item in parts if isinstance(item, dict) for binding in item.get("editable_parameter_bindings", []) if isinstance(binding, dict) and binding.get("parameter_id")],
    }


def _renderer_preflight(readback: Mapping[str, Any]) -> dict[str, object]:
    texture_sets = readback["visual_texture_sets"]
    return {
        "status": "pending",
        "formal_eligible": False,
        "human_benchmark_evidence": False,
        "provider_calls": 0,
        "reason": "requires same-canvas ForgeCADWorkbenchRenderer@1 capture; GLB facts are not a renderer capture",
        "glb_readback_estimate": {
            "geometry_count": int(readback["mesh_count"]),
            "texture_count": sum(len(item["maps"]) for item in texture_sets),
            "draw_calls": int(readback["primitive_count"]),
            "triangle_count": int(readback["triangle_count"]),
            "embedded_pbr_texture_count": sum(len(item["maps"]) for item in texture_sets),
            "texture_memory_bytes": sum(int(item["texture_byte_size"]) for item in texture_sets),
        },
    }


def _gate_report(
    *, gate_key: str, recipe_id: str, glb_sha256: str, readback_sha256: str, checks: Mapping[str, Any]
) -> dict[str, object]:
    execution_id = f"m108b-preflight-{_slug(recipe_id)}-{gate_key}-{glb_sha256[:12]}"
    return {
        "schema_version": "M108BGateEvidence@1",
        "evidence_origin": "development_preflight",
        "formal_eligible": False,
        "human_benchmark_evidence": False,
        "provider_calls": 0,
        "score_status": "not_scored",
        "gate_id": GATES[gate_key],
        "status": "passed",
        "execution_id": execution_id,
        "source_glb_sha256": glb_sha256,
        "readback_sha256": readback_sha256,
        "checks": dict(checks),
    }


def _registry_lock_bytes() -> bytes:
    path = ROOT / "packages/concept-spec/fixtures/component-recipes/locks/m108b-production-v1.lock.json"
    if not path.is_file():
        raise PreflightError("M108B_PREFLIGHT_REGISTRY_LOCK_MISSING")
    return path.read_bytes()


def build(output: Path) -> dict[str, Any]:
    """Create an all-or-nothing development preflight directory."""

    output.parent.mkdir(parents=True, exist_ok=True)
    if output.is_symlink():
        raise PreflightError(f"M108B_PREFLIGHT_OUTPUT_SYMLINK_REFUSED:{output}")
    if output.exists() and not output.is_dir():
        raise PreflightError(f"M108B_PREFLIGHT_OUTPUT_NOT_DIRECTORY:{output}")
    if output.exists() and any(output.iterdir()):
        raise PreflightError(f"M108B_PREFLIGHT_OUTPUT_NOT_EMPTY:{output}")
    lock_bytes = _registry_lock_bytes()
    lock_document = _read_json(
        ROOT / "packages/concept-spec/fixtures/component-recipes/locks/m108b-production-v1.lock.json"
    )
    registry_source_path = ROOT / "packages/concept-spec/fixtures/production-component-recipe-registry.json"
    registry_document = _read_json(registry_source_path)
    registry_file_sha256 = _sha256(registry_source_path.read_bytes())
    expansion = _run_rust_expansion()
    registry_sha256 = expansion.get("registry_sha256")
    if not isinstance(registry_sha256, str) or len(registry_sha256) != 64:
        raise PreflightError("M108B_PREFLIGHT_REGISTRY_INVALID")
    if (
        _sha256(_canonical_bytes(registry_document)) != registry_sha256
        or lock_document.get("registry_sha256") != registry_sha256
        or lock_document.get("registry_id") != expansion.get("registry_id")
    ):
        raise PreflightError("M108B_PREFLIGHT_REGISTRY_LOCK_DRIFT")
    candidates = expansion["candidates"]
    seen_roots: set[str] = set()
    seen_hashes: set[str] = set()
    domain_counts = {domain: 0 for domain in DOMAINS}
    with tempfile.TemporaryDirectory(prefix="m108b-preflight-", dir=output.parent) as temp_dir:
        stage = Path(temp_dir) / "preflight"
        stage.mkdir()
        (stage / "registry").mkdir()
        (stage / "registry" / "m108b-production-v1.lock.json").write_bytes(lock_bytes)
        shutil.copyfile(registry_source_path, stage / "registry" / registry_source_path.name)
        fixtures: list[dict[str, Any]] = []
        # An empty environment is intentional: the executor rejects provider,
        # storage, secret and Snapshot context before the child process starts.
        executor = RestrictedGeometryExecutor(environment={})
        for index, candidate_value in enumerate(candidates):
            if not isinstance(candidate_value, dict):
                raise PreflightError("M108B_PREFLIGHT_CANDIDATE_INVALID")
            candidate = candidate_value
            recipe = candidate.get("recipe")
            if not isinstance(recipe, dict) or not isinstance(recipe.get("recipe_id"), str):
                raise PreflightError("M108B_PREFLIGHT_CANDIDATE_INVALID")
            recipe_id = recipe["recipe_id"]
            if recipe_id not in ROOT_IDS or recipe_id in seen_roots:
                raise PreflightError("M108B_PREFLIGHT_ROOT_ID_INVALID")
            seen_roots.add(recipe_id)
            if candidate.get("registry_sha256") != registry_sha256:
                raise PreflightError("M108B_PREFLIGHT_CANDIDATE_REGISTRY_DRIFT")
            instances = candidate.get("component_recipe_instances")
            if not isinstance(instances, list) or not instances:
                raise PreflightError("M108B_PREFLIGHT_CANDIDATE_INVALID")
            if any(
                not isinstance(instance, dict) or instance.get("registry_sha256") != registry_sha256
                for instance in instances
            ):
                raise PreflightError("M108B_PREFLIGHT_PROVENANCE_REGISTRY_DRIFT")
            root_instances = [instance for instance in instances if instance.get("parent_instance_id") is None]
            if len(root_instances) != 1 or root_instances[0].get("recipe") != recipe:
                raise PreflightError("M108B_PREFLIGHT_ROOT_PROVENANCE_DRIFT")
            domain = candidate.get("component_recipe_instances", [{}])[0].get("domain_pack_id") if isinstance(candidate.get("component_recipe_instances"), list) and candidate["component_recipe_instances"] else None
            if domain not in domain_counts:
                raise PreflightError("M108B_PREFLIGHT_DOMAIN_INVALID")
            domain_counts[domain] += 1
            slug = _slug(recipe_id)
            request = RestrictedGeometryExecutionRequest.model_validate({
                "schema_version": "RestrictedGeometryExecutionRequest@1",
                "protocol_version": "forgecad.restricted-geometry/1",
                "execution_id": f"m108b-preflight-{slug}-compile",
                "idempotency_key": f"m108b-preflight-{slug}-idem",
                "cancellation_id": f"m108b-preflight-{slug}-cancel",
                "cancellation_token": f"m108b-preflight-{slug}-token",
                "action": "compile_readback",
                "timeout_ms": 120000,
                "artifact_profile_id": "production_concept",
                "shape_program": candidate.get("expanded_shape_program"),
            })
            result = executor.execute(request)
            if result.readback is None or result.glb_base64 is None:
                raise PreflightError("M108B_PREFLIGHT_EXECUTOR_RESULT_INVALID")
            glb = base64.b64decode(result.glb_base64, validate=True)
            if result.glb_sha256 in seen_hashes:
                raise PreflightError("M108B_PREFLIGHT_DUPLICATE_GLB_HASH")
            seen_hashes.add(result.glb_sha256)
            readback = _assert_readback_matches_glb(
                glb=glb, readback=result.readback, expected_glb_sha256=result.glb_sha256
            )
            if result.triangle_count != readback["triangle_count"]:
                raise PreflightError("M108B_PREFLIGHT_TRIANGLE_DRIFT")
            readback_sha256 = _readback_hash(readback)
            checks = {
                "m108a": _m108a_checks(readback),
                "q003": _q003_checks(readback),
                "g826": _g826_checks(readback),
            }
            glb_file = Path("fixtures") / f"{slug}.glb"
            readback_file = Path("readbacks") / f"{slug}.json"
            candidate_file = Path("candidates") / f"{slug}.json"
            (stage / glb_file).parent.mkdir(parents=True, exist_ok=True)
            (stage / glb_file).write_bytes(glb)
            _write_json(stage / readback_file, readback)
            _write_json(stage / candidate_file, candidate)
            gate_evidence: dict[str, Any] = {"readback_sha256": readback_sha256}
            for gate_key, gate_checks in checks.items():
                report = _gate_report(
                    gate_key=gate_key,
                    recipe_id=recipe_id,
                    glb_sha256=result.glb_sha256,
                    readback_sha256=readback_sha256,
                    checks=gate_checks,
                )
                report_file = Path("evidence") / slug / f"{gate_key}.json"
                _write_json(stage / report_file, report)
                gate_evidence[gate_key] = {
                    "status": "passed",
                    "execution_id": report["execution_id"],
                    "report_file": str(report_file),
                    "report_sha256": _sha256((stage / report_file).read_bytes()),
                    "source_glb_sha256": result.glb_sha256,
                }
            parts = candidate["expanded_assembly_graph"].get("parts", [])
            zones = sorted({zone for part in parts if isinstance(part, dict) for zone in part.get("material_zone_ids", [])})
            fixtures.append({
                "fixture_id": f"m108b:{slug}",
                "domain_pack_id": domain,
                "source_glb": str(glb_file),
                "glb_sha256": result.glb_sha256,
                "glb_byte_size": len(glb),
                "readback_file": str(readback_file),
                "readback_sha256": readback_sha256,
                "candidate_file": str(candidate_file),
                "candidate_file_sha256": _sha256((stage / candidate_file).read_bytes()),
                "recipe": recipe,
                "registry": {
                    "registry_id": expansion.get("registry_id"),
                    "registry_sha256": registry_sha256,
                    "registry_file": f"registry/{registry_source_path.name}",
                    "registry_file_sha256": registry_file_sha256,
                    "registry_lock_file": "registry/m108b-production-v1.lock.json",
                    "registry_lock_sha256": _sha256(lock_bytes),
                },
                "candidate": {"candidate_id": candidate.get("candidate_id"), "candidate_sha256": candidate.get("candidate_sha256")},
                "provenance": {"component_recipe_instances": candidate.get("component_recipe_instances")},
                "semantic": _semantic_facts(candidate),
                "material_zones": zones,
                "production_pbr": {"texture_sets": readback["visual_texture_sets"]},
                "gate_evidence": gate_evidence,
                "renderer_capture": _renderer_preflight(readback),
            })
        if seen_roots != ROOT_IDS or any(count != 3 for count in domain_counts.values()):
            raise PreflightError("M108B_PREFLIGHT_DOMAIN_COVERAGE_INVALID")
        draft: dict[str, Any] = {
            "schema_version": "M108BFormalFixtureSourceManifest@1",
            "fixture_origin": "recipe_backed_production",
            "selection_status": "not_scored",
            "score_status": "not_scored",
            "frozen_before_scoring": False,
            "formal_eligible": False,
            "human_benchmark_evidence": False,
            "formal_blockers": [
                "fixture selection has not been frozen before human scoring",
                "same-canvas ForgeCADWorkbenchRenderer@1 evidence is pending",
                "development preflight is not an independent human visual review",
            ],
            "provider_calls": 0,
            "fixtures": fixtures,
        }
        _write_json(stage / "m108b-formal-source-draft.json", draft)
        summary = {
            "schema_version": "M108BAssetPreflightSummary@1",
            "provider_calls": 0,
            "fixture_count": len(fixtures),
            "unique_glb_hash_count": len(seen_hashes),
            "formal_eligible": False,
            "human_benchmark_evidence": False,
            "renderer_capture_status": "pending",
            "source_manifest": "m108b-formal-source-draft.json",
        }
        _write_json(stage / "summary.json", summary)
        verify(stage)
        if output.exists():
            output.rmdir()
        shutil.move(str(stage), str(output))
        return draft


def verify(root: Path) -> dict[str, Any]:
    """Verify every development fact without upgrading it to a formal kit."""

    draft_path = root / "m108b-formal-source-draft.json"
    draft = _read_json(draft_path)
    if (
        draft.get("schema_version") != "M108BFormalFixtureSourceManifest@1"
        or draft.get("frozen_before_scoring") is not False
        or draft.get("formal_eligible") is not False
        or draft.get("human_benchmark_evidence") is not False
        or draft.get("selection_status") != "not_scored"
        or draft.get("score_status") != "not_scored"
        or draft.get("fixture_origin") != "recipe_backed_production"
        or draft.get("provider_calls") != 0
    ):
        raise PreflightError("M108B_PREFLIGHT_DRAFT_STATUS_INVALID")
    fixtures = draft.get("fixtures")
    if not isinstance(fixtures, list) or len(fixtures) != 12:
        raise PreflightError("M108B_PREFLIGHT_FIXTURE_COUNT_INVALID")
    # Validate the manifest's declared asset identities before opening a GLB.
    # This makes duplicate fixture mutations deterministic and preserves a
    # distinct diagnostic from a later byte/hash mismatch on a file path.
    fixture_ids: set[str] = set()
    root_recipe_ids: set[str] = set()
    declared_hashes: set[str] = set()
    for fixture in fixtures:
        if not isinstance(fixture, dict):
            raise PreflightError("M108B_PREFLIGHT_FIXTURE_INVALID")
        fixture_id = fixture.get("fixture_id")
        if not isinstance(fixture_id, str) or not fixture_id.startswith("m108b:") or fixture_id in fixture_ids:
            raise PreflightError("M108B_PREFLIGHT_FIXTURE_ID_INVALID")
        fixture_ids.add(fixture_id)
        recipe = fixture.get("recipe")
        recipe_id = recipe.get("recipe_id") if isinstance(recipe, dict) else None
        if not isinstance(recipe_id, str) or recipe_id not in ROOT_IDS or recipe_id in root_recipe_ids:
            raise PreflightError("M108B_PREFLIGHT_ROOT_ID_INVALID")
        root_recipe_ids.add(recipe_id)
        declared_glb_sha256 = fixture.get("glb_sha256")
        if (
            not isinstance(declared_glb_sha256, str)
            or len(declared_glb_sha256) != 64
            or any(character not in "0123456789abcdef" for character in declared_glb_sha256)
        ):
            raise PreflightError("M108B_PREFLIGHT_GLB_SHA_INVALID")
        if declared_glb_sha256 in declared_hashes:
            raise PreflightError("M108B_PREFLIGHT_DUPLICATE_GLB_HASH")
        declared_hashes.add(declared_glb_sha256)
    domains = {domain: 0 for domain in DOMAINS}
    for fixture in fixtures:
        if not isinstance(fixture, dict):
            raise PreflightError("M108B_PREFLIGHT_FIXTURE_INVALID")
        domain = fixture.get("domain_pack_id")
        if domain not in domains:
            raise PreflightError("M108B_PREFLIGHT_DOMAIN_INVALID")
        domains[domain] += 1
        glb_path = root / _safe_relative(fixture.get("source_glb"), suffix=".glb", field="source_glb")
        readback_path = root / _safe_relative(fixture.get("readback_file"), suffix=".json", field="readback_file")
        candidate_path = root / _safe_relative(fixture.get("candidate_file"), suffix=".json", field="candidate_file")
        expected_glb_sha256 = fixture.get("glb_sha256")
        assert isinstance(expected_glb_sha256, str)  # validated in the manifest identity preflight
        glb = glb_path.read_bytes()
        if _sha256(glb) != expected_glb_sha256 or len(glb) != fixture.get("glb_byte_size"):
            raise PreflightError("M108B_PREFLIGHT_GLB_FILE_DRIFT")
        readback = _read_json(readback_path)
        if _readback_hash(readback) != fixture.get("readback_sha256"):
            raise PreflightError("M108B_PREFLIGHT_READBACK_FILE_DRIFT")
        _assert_readback_matches_glb(glb=glb, readback=readback, expected_glb_sha256=fixture["glb_sha256"])
        if _sha256(candidate_path.read_bytes()) != fixture.get("candidate_file_sha256"):
            raise PreflightError("M108B_PREFLIGHT_CANDIDATE_FILE_DRIFT")
        candidate = _read_json(candidate_path)
        if candidate.get("candidate_id") != fixture.get("candidate", {}).get("candidate_id") or candidate.get("candidate_sha256") != fixture.get("candidate", {}).get("candidate_sha256"):
            raise PreflightError("M108B_PREFLIGHT_CANDIDATE_DRIFT")
        if candidate.get("component_recipe_instances") != fixture.get("provenance", {}).get("component_recipe_instances"):
            raise PreflightError("M108B_PREFLIGHT_PROVENANCE_DRIFT")
        registry = fixture.get("registry")
        if not isinstance(registry, dict):
            raise PreflightError("M108B_PREFLIGHT_REGISTRY_INVALID")
        registry_path = root / _safe_relative(registry.get("registry_file"), suffix=".json", field="registry_file")
        lock_path = root / _safe_relative(registry.get("registry_lock_file"), suffix=".json", field="registry_lock_file")
        registry_document = _read_json(registry_path)
        lock_document = _read_json(lock_path)
        if (
            _sha256(registry_path.read_bytes()) != registry.get("registry_file_sha256")
            or _sha256(_canonical_bytes(registry_document)) != registry.get("registry_sha256")
            or lock_document.get("registry_sha256") != registry.get("registry_sha256")
            or _sha256(lock_path.read_bytes()) != registry.get("registry_lock_sha256")
            or candidate.get("registry_sha256") != registry.get("registry_sha256")
        ):
            raise PreflightError("M108B_PREFLIGHT_REGISTRY_LOCK_DRIFT")
        gate_evidence = fixture.get("gate_evidence")
        if not isinstance(gate_evidence, dict) or gate_evidence.get("readback_sha256") != fixture["readback_sha256"]:
            raise PreflightError("M108B_PREFLIGHT_GATE_READBACK_DRIFT")
        seen_reports: set[str] = set()
        seen_report_hashes: set[str] = set()
        seen_execution_ids: set[str] = set()
        for gate_key, gate_id in GATES.items():
            link = gate_evidence.get(gate_key)
            if not isinstance(link, dict):
                raise PreflightError("M108B_PREFLIGHT_GATE_REPORT_MISSING")
            report_path = root / _safe_relative(link.get("report_file"), suffix=".json", field=f"{gate_key}.report_file")
            if (
                _sha256(report_path.read_bytes()) != link.get("report_sha256")
                or str(report_path) in seen_reports
                or link.get("report_sha256") in seen_report_hashes
                or link.get("execution_id") in seen_execution_ids
            ):
                raise PreflightError("M108B_PREFLIGHT_GATE_REPORT_DRIFT")
            seen_reports.add(str(report_path))
            seen_report_hashes.add(str(link.get("report_sha256")))
            seen_execution_ids.add(str(link.get("execution_id")))
            report = _read_json(report_path)
            if (
                report.get("schema_version") != "M108BGateEvidence@1"
                or report.get("evidence_origin") != "development_preflight"
                or report.get("formal_eligible") is not False
                or report.get("human_benchmark_evidence") is not False
                or report.get("provider_calls") != 0
                or report.get("score_status") != "not_scored"
                or report.get("gate_id") != gate_id
                or report.get("status") != "passed"
                or report.get("execution_id") != link.get("execution_id")
                or report.get("source_glb_sha256") != fixture["glb_sha256"]
                or report.get("readback_sha256") != fixture["readback_sha256"]
            ):
                raise PreflightError("M108B_PREFLIGHT_GATE_REPORT_INVALID")
        capture = fixture.get("renderer_capture")
        if (
            not isinstance(capture, dict)
            or capture.get("status") != "pending"
            or capture.get("formal_eligible") is not False
            or capture.get("human_benchmark_evidence") is not False
            or capture.get("provider_calls") != 0
        ):
            raise PreflightError("M108B_PREFLIGHT_CAPTURE_STATUS_INVALID")
    if any(count != 3 for count in domains.values()) or len(declared_hashes) != 12:
        raise PreflightError("M108B_PREFLIGHT_DOMAIN_COVERAGE_INVALID")
    return draft


def _tree_hashes(root: Path) -> dict[str, str]:
    return {
        str(path.relative_to(root)): _sha256(path.read_bytes())
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def self_test() -> None:
    """Exercise determinism, tamper, duplicate-hash and formal rejection paths."""

    from prepare_m108b_formal_visual_benchmark import FormalKitBlockedError, validate_source_manifest
    from unittest.mock import patch

    registry_document = _read_json(
        ROOT / "packages/concept-spec/fixtures/production-component-recipe-registry.json"
    )
    registry_sha256 = _sha256(_canonical_bytes(registry_document))

    def controlled_expansion() -> dict[str, Any]:
        """Valid, in-memory Rust-dump-shaped candidates for factory tests.

        This avoids treating a concurrently edited production catalog or its
        worker topology as a test fixture for the factory's own determinism
        and tamper boundaries.  The real command still consumes only the Rust
        dump and fails closed when the shipped registry/lock are inconsistent.
        """
        candidates: list[dict[str, Any]] = []
        for index, recipe_id in enumerate(sorted(ROOT_IDS)):
            if recipe_id.startswith("recipe_prop_"):
                domain = "pack_future_weapon_prop"
            elif recipe_id.startswith("recipe_vehicle_"):
                domain = "pack_vehicle_concept"
            elif recipe_id.startswith("recipe_aircraft_"):
                domain = "pack_aircraft_concept"
            else:
                domain = "pack_robotic_arm_concept"
            recipe = {
                "schema_version": "ComponentRecipeRef@1",
                "recipe_id": recipe_id,
                "version": 1,
                "recipe_sha256": _sha256(recipe_id.encode("utf-8")),
            }
            instance = {
                "schema_version": "ComponentRecipeInstanceProvenance@1",
                "instance_id": f"recipeinst_controlled_{index}",
                "parent_instance_id": None,
                "parent_slot_id": None,
                "domain_pack_id": domain,
                "registry_sha256": registry_sha256,
                "recipe": recipe,
            }
            operation_id = f"op_controlled_{index}"
            zone_id = f"zone_controlled_{index}"
            shape_program = {
                "schema_version": "ShapeProgram@1",
                "program_id": f"shape_controlled_{index}",
                "units": "millimeter",
                "seed": index + 1,
                "triangle_budget": 7000,
                "parameters": [],
                "profile_inputs": [],
                "operations": [{
                    "operation_id": operation_id,
                    "op": "box",
                    "inputs": [],
                    "args": {
                        "position": [index * 35, 0, 0],
                        "size": [300 + index, 180, 120],
                        "part_role": "primary_form",
                        "material_id": "mat_graphite",
                        "zone_id": zone_id,
                    },
                }],
                "outputs": [{
                    "output_id": f"output_controlled_{index}",
                    "operation_id": operation_id,
                    "kind": "mesh",
                    "part_role": "primary_form",
                }],
                "non_functional_only": True,
            }
            candidates.append({
                "schema_version": "ComponentRecipeCandidate@1",
                "candidate_id": f"candidate_controlled_{index}",
                "candidate_sha256": _sha256(f"candidate:{index}".encode("utf-8")),
                "registry_sha256": registry_sha256,
                "recipe": recipe,
                "component_recipe_instances": [instance],
                "expanded_assembly_graph": {
                    "parts": [{
                        "part_id": f"part_controlled_{index}",
                        "role": "primary_form",
                        "material_zone_ids": [zone_id],
                        "editable_parameter_bindings": [],
                    }],
                    "connections": [],
                },
                "expanded_shape_program": shape_program,
            })
        return {
            "schema_version": "M108BProductionRecipeExpansion@1",
            "registry_id": registry_document["registry_id"],
            "registry_sha256": registry_sha256,
            "candidates": candidates,
        }

    with tempfile.TemporaryDirectory(prefix="m108b-preflight-test-") as temp_dir:
        root = Path(temp_dir)
        first, second = root / "first", root / "second"
        with patch(f"{__name__}._run_rust_expansion", side_effect=controlled_expansion):
            draft = build(first)
            build(second)
        if _tree_hashes(first) != _tree_hashes(second):
            raise AssertionError("M108B preflight is not deterministic")
        try:
            validate_source_manifest(draft)
        except FormalKitBlockedError:
            pass
        else:
            raise AssertionError("unfrozen M108B preflight draft was accepted as a formal kit source")
        manifest = _read_json(first / "m108b-formal-source-draft.json")
        duplicate = json.loads(json.dumps(manifest))
        duplicate["fixtures"][1]["glb_sha256"] = duplicate["fixtures"][0]["glb_sha256"]
        _write_json(first / "m108b-formal-source-draft.json", duplicate)
        try:
            verify(first)
        except PreflightError as error:
            if "DUPLICATE_GLB_HASH" not in str(error):
                raise
        else:
            raise AssertionError("duplicate GLB hash was accepted")
        _write_json(first / "m108b-formal-source-draft.json", manifest)
        copied_report = json.loads(json.dumps(manifest))
        copied_report["fixtures"][0]["gate_evidence"]["q003"]["report_sha256"] = copied_report["fixtures"][0]["gate_evidence"]["m108a"]["report_sha256"]
        _write_json(first / "m108b-formal-source-draft.json", copied_report)
        try:
            verify(first)
        except PreflightError as error:
            if "GATE_REPORT_DRIFT" not in str(error):
                raise
        else:
            raise AssertionError("copied gate report hash was accepted")
        _write_json(first / "m108b-formal-source-draft.json", manifest)
        glb_path = first / manifest["fixtures"][0]["source_glb"]
        glb_path.write_bytes(glb_path.read_bytes()[:-1] + b"x")
        try:
            verify(first)
        except PreflightError as error:
            if "GLB_FILE_DRIFT" not in str(error):
                raise
        else:
            raise AssertionError("tampered GLB was accepted")
        semantic = _semantic_facts({
            "expanded_assembly_graph": {
                "parts": [{"part_id": "part_binding", "role": "primary_form", "editable_parameter_bindings": [{"parameter_id": "editparam_checked"}]}],
                "connections": [],
            },
            "expanded_shape_program": {"operations": []},
            "component_recipe_instances": [],
        })
        if semantic["bindings"] != ["editparam_checked"]:
            raise AssertionError("recipe binding semantic extraction drifted")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT / "output/m108b-asset-preflight")
    parser.add_argument("--verify", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        print("M108B asset preflight self-test passed: deterministic/tamper/duplicate/formal-rejection/human_benchmark_evidence=false/provider_calls=0")
        return 0
    if args.verify:
        verify(args.output)
        print(f"M108B asset preflight verified: {args.output} (formal_eligible=false, human_benchmark_evidence=false, renderer_capture=pending, provider_calls=0)")
        return 0
    draft = build(args.output)
    print(f"M108B asset preflight built: fixtures={len(draft['fixtures'])}, formal_eligible=false, human_benchmark_evidence=false, renderer_capture=pending, provider_calls=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
