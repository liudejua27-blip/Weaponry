#!/usr/bin/env python3
"""Focused contract gate for the bounded knife curve EvaluatedMesh façade.

The contract is intentionally independent from Runtime/MCP implementation.  It
proves that the curve graph source binding is carried into a closed sweep/loft
plan and into a disposable EvaluatedMesh identity/link, while no mesh buffers
or arbitrary executor can cross the façade boundary.  The existing
KnifeCurveModifierGraph@1 schemas are compatibility replay contracts and are
hash-checked here without changing their meaning.
"""

from __future__ import annotations

import copy
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_ROOT = ROOT / "packages" / "forgecad-contracts" / "schemas"
PROFILE_ROOT = ROOT / "packages" / "forgecad-contracts" / "profiles"
MANIFEST_PATH = ROOT / "packages" / "forgecad-contracts" / "manifest.json"
SOURCE_SUMMARY_PATH = ROOT / "docs" / "evidence" / "mcp010f" / "source-tool-manifest-summary.json"

sys.path.insert(0, str(ROOT / "scripts"))
from check_agentic_contracts import is_valid, load_schema_registry  # noqa: E402

PREPARE = "knife-curve-evaluated-mesh-prepare-request.schema.json"
GET = "knife-curve-evaluated-mesh-get-request.schema.json"
RESULT = "knife-curve-evaluated-mesh-result.schema.json"
LEGACY_SCHEMA_BYTES_SHA256 = {
    "knife-curve-modifier-graph-prepare-request.schema.json": "0c4401f6d9505072fb6adcd374f3c3a353c3e8b6c8aea246e6903920a289755d",
    "knife-curve-modifier-graph-get-request.schema.json": "32f87e380c159863680c146a91950a56f47d0ab1bf5d4c22e4cc39c5d82bd9ee",
    "knife-curve-modifier-graph-result.schema.json": "8ca5fa15d0e27d85cd81cb4696df338f1b17410939e392b0e8fb72b940fb3375",
}
NATIVE_OPERATIONS = {
    "knife_curve_modifier_graph_prepare": ("write", "KnifeCurveModifierGraphPrepareRequest@1", "KnifeCurveModifierGraphResult@1"),
    "knife_curve_modifier_graph_get": ("read", "KnifeCurveModifierGraphGetRequest@1", "KnifeCurveModifierGraphResult@1"),
    "knife_curve_evaluated_mesh_prepare": ("write", "KnifeCurveEvaluatedMeshPrepareRequest@1", "KnifeCurveEvaluatedMeshResult@1"),
    "knife_curve_evaluated_mesh_get": ("read", "KnifeCurveEvaluatedMeshGetRequest@1", "KnifeCurveEvaluatedMeshResult@1"),
}
FACADES = [
    "weapon_preflight", "reference_intake", "observe", "authoring_transaction",
    "surface_pipeline", "fps_presentation", "quality_review", "delivery",
    "approval", "recovery", "job",
]
FORBIDDEN_PROPERTY_NAMES = {
    "path", "url", "uri", "raw", "raw_bytes", "bytes", "secret", "token",
    "password", "api_key", "prompt", "script", "shell", "environment",
    "executor", "operator_code", "python", "javascript", "vertices", "faces",
    "indices", "mesh_buffer",
}
H = "0123456789abcdef" * 4


def fail(message: str) -> None:
    raise SystemExit(f"Knife curve EvaluatedMesh contract violation: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def load(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"cannot load {path.relative_to(ROOT)}: {exc}")
    require(isinstance(value, dict), f"{path.name} must be an object")
    return value


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def sha256(value: Any) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def canonical_object_hash(value: dict[str, Any]) -> str:
    payload = copy.deepcopy(value)
    payload.pop("canonical_sha256", None)
    payload.pop("input_sha256", None)
    return sha256(payload)


def input_hash(value: dict[str, Any]) -> str:
    payload = copy.deepcopy(value)
    payload.pop("input_sha256", None)
    return sha256(payload)


def walk_schema_properties(node: Any) -> list[str]:
    if not isinstance(node, dict):
        return []
    names: list[str] = []
    properties = node.get("properties")
    if isinstance(properties, dict):
        names.extend(properties)
        for child in properties.values():
            names.extend(walk_schema_properties(child))
    for key in ("$defs", "items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"):
        child = node.get(key)
        if isinstance(child, dict):
            names.extend(walk_schema_properties(child))
        elif isinstance(child, list):
            for value in child:
                names.extend(walk_schema_properties(value))
    return names


def check_schema_shell(schema: dict[str, Any], expected_version: str, label: str) -> None:
    require(schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema", f"{label} draft drifted")
    require(schema.get("$id") == f"https://forgecad.local/contracts/{label}.schema.json", f"{label} id drifted")
    require(schema.get("title") == expected_version, f"{label} title drifted")
    require(schema.get("type") == "object" and schema.get("additionalProperties") is False, f"{label} root is not closed")
    require(schema.get("properties", {}).get("schema_version", {}).get("const") == expected_version, f"{label} version drifted")
    properties = {name.lower() for name in walk_schema_properties(schema)}
    require(not properties & FORBIDDEN_PROPERTY_NAMES, f"{label} exposes raw mesh or executor property")

    def inspect_objects(node: Any, location: str = "$") -> None:
        if not isinstance(node, dict):
            return
        if node.get("type") == "object":
            require(node.get("additionalProperties") is False, f"{label} object is open at {location}")
        for key, value in node.items():
            if key in {"properties", "$defs"} and isinstance(value, dict):
                for child_name, child in value.items():
                    inspect_objects(child, f"{location}.{key}.{child_name}")
            elif key in {"items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"}:
                if isinstance(value, list):
                    for index, child in enumerate(value):
                        inspect_objects(child, f"{location}.{key}[{index}]")
                else:
                    inspect_objects(value, f"{location}.{key}")

    inspect_objects(schema)


def source_fields() -> dict[str, Any]:
    revision_sha = sha256({"revision_id": "authoring-revision-7", "kind": "AuthoringMeshRevision@1"})
    modifier_sha = sha256({"graph_id": "knife-graph-1", "kind": "ModifierGraph@1"})
    return {
        "project_id": "project-knife-1",
        "source_candidate_id": "candidate-knife-1",
        "source_candidate_state_sha256": H,
        "source_authoring_mesh_id": "authoring-mesh-1",
        "source_authoring_mesh_lineage_id": "authoring-lineage-1",
        "source_authoring_mesh_revision_id": "authoring-revision-7",
        "source_authoring_mesh_revision_index": 7,
        "source_authoring_mesh_revision_sha256": revision_sha,
        "source_authoring_mesh_identity_sha256": sha256({"mesh_id": "authoring-mesh-1", "lineage_id": "authoring-lineage-1"}),
        "source_modifier_graph_id": "knife-graph-1",
        "source_modifier_graph_sha256": modifier_sha,
    }


def plan_fixture(source: dict[str, Any]) -> dict[str, Any]:
    plan = {
        "schema_version": "KnifeBladeProfileSweepLoftPlan@1",
        "evaluation_id": "knife-evaluation-1",
        "spine_curve_id": "blade-spine-1",
        "spine_curve_sha256": sha256({"curve_id": "blade-spine-1", "role": "blade_spine"}),
        "edge_curve_id": "blade-edge-1",
        "edge_curve_sha256": sha256({"curve_id": "blade-edge-1", "role": "blade_edge"}),
        "station_count": 32,
        "thickness_axis": "local_normal",
        "thickness_m": 0.012,
        "root_cap": True,
        "tip_cap": True,
        "stable_triangulation": "station-ring-fixed-diagonal@1",
        "stable_lineage_policy": "source-curve-modifier-graph-evaluated-mesh@1",
    }
    plan["canonical_sha256"] = canonical_object_hash(plan)
    return plan


def source_hashes(source: dict[str, Any]) -> dict[str, str]:
    curve_set = sha256({"curve_set": "durable-curve-set-1"})
    sample_set = sha256({"sample_set": "durable-sample-set-1"})
    graph = sha256({"graph_id": source["source_modifier_graph_id"], "revision": source["source_authoring_mesh_revision_id"]})
    dependency = sha256({"dependency_graph": "knife-dependency-1", "source_revision_sha256": source["source_authoring_mesh_revision_sha256"]})
    recompute = sha256({"recompute_plan": "knife-recompute-1", "modifier_graph_sha256": source["source_modifier_graph_sha256"]})
    values = {
        "curve_set_semantic_sha256": curve_set,
        "sample_set_semantic_sha256": sample_set,
        "modifier_graph_semantic_sha256": graph,
        "dependency_graph_semantic_sha256": dependency,
        "recompute_plan_semantic_sha256": recompute,
    }
    values["curve_graph_lookup_key_sha256"] = sha256({"source": values, "kind": "KnifeCurveModifierGraphLookupKey@1"})
    return values


def mesh_hashes(source: dict[str, Any], plan: dict[str, Any]) -> dict[str, Any]:
    mesh_id = "evaluated-mesh-1"
    vertex_count = 256
    triangle_count = 512
    mesh_object = {
        "schema_version": "EvaluatedMeshObject@1",
        "evaluated_mesh_id": mesh_id,
        "evaluation_id": plan["evaluation_id"],
        "vertex_count": vertex_count,
        "triangle_count": triangle_count,
        "closed_two_manifold": True,
        "zero_degenerate_triangles": True,
    }
    mesh_semantic = {
        "evaluated_mesh_id": mesh_id,
        "source_revision_id": source["source_authoring_mesh_revision_id"],
        "source_revision_sha256": source["source_authoring_mesh_revision_sha256"],
        "modifier_graph_id": source["source_modifier_graph_id"],
        "modifier_graph_sha256": source["source_modifier_graph_sha256"],
        "evaluation_id": plan["evaluation_id"],
        "station_count": plan["station_count"],
        "thickness_axis": plan["thickness_axis"],
        "thickness_m": plan["thickness_m"],
    }
    return {
        "evaluated_mesh_id": mesh_id,
        "evaluated_mesh_object_sha256": sha256(mesh_object),
        "evaluated_mesh_semantic_sha256": sha256(mesh_semantic),
        "vertex_count": vertex_count,
        "triangle_count": triangle_count,
    }


def identity_hash(source: dict[str, Any], plan: dict[str, Any], mesh: dict[str, Any]) -> str:
    return sha256({
        "schema_version": "EvaluatedMeshIdentity@1",
        "evaluated_mesh_id": mesh["evaluated_mesh_id"],
        "evaluation_id": plan["evaluation_id"],
        "source_authoring_mesh_id": source["source_authoring_mesh_id"],
        "source_revision_id": source["source_authoring_mesh_revision_id"],
        "source_revision_sha256": source["source_authoring_mesh_revision_sha256"],
        "modifier_graph_id": source["source_modifier_graph_id"],
        "modifier_graph_sha256": source["source_modifier_graph_sha256"],
    })


def link_hash(source: dict[str, Any], plan: dict[str, Any], mesh: dict[str, Any], identity_sha: str) -> str:
    return sha256({
        "schema_version": "EvaluatedMeshLink@1",
        "evaluation_id": plan["evaluation_id"],
        "evaluated_mesh_id": mesh["evaluated_mesh_id"],
        "evaluated_mesh_identity_sha256": identity_sha,
        "evaluated_mesh_object_sha256": mesh["evaluated_mesh_object_sha256"],
        "evaluated_mesh_semantic_sha256": mesh["evaluated_mesh_semantic_sha256"],
        "source_revision_sha256": source["source_authoring_mesh_revision_sha256"],
        "modifier_graph_sha256": source["source_modifier_graph_sha256"],
    })


def evaluation_lookup(source: dict[str, Any], source_hash: dict[str, str], plan: dict[str, Any], mesh: dict[str, Any], identity_sha: str, link_sha: str) -> str:
    return sha256({
        "schema_version": "KnifeCurveEvaluatedMeshLookupKey@1",
        "source_candidate_id": source["source_candidate_id"],
        "source_authoring_mesh_revision_sha256": source["source_authoring_mesh_revision_sha256"],
        "source_modifier_graph_sha256": source["source_modifier_graph_sha256"],
        "curve_graph_lookup_key_sha256": source_hash["curve_graph_lookup_key_sha256"],
        "evaluation_id": plan["evaluation_id"],
        "evaluation_plan_semantic_sha256": sha256(plan_semantic(plan)),
        "evaluated_mesh_identity_sha256": identity_sha,
        "evaluated_mesh_link_sha256": link_sha,
        "evaluated_mesh_object_sha256": mesh["evaluated_mesh_object_sha256"],
        "evaluated_mesh_semantic_sha256": mesh["evaluated_mesh_semantic_sha256"],
    })


def plan_semantic(plan: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": plan["schema_version"],
        "evaluation_id": plan["evaluation_id"],
        "spine_curve_id": plan["spine_curve_id"],
        "spine_curve_sha256": plan["spine_curve_sha256"],
        "edge_curve_id": plan["edge_curve_id"],
        "edge_curve_sha256": plan["edge_curve_sha256"],
        "station_count": plan["station_count"],
        "thickness_axis": plan["thickness_axis"],
        "thickness_m": plan["thickness_m"],
        "root_cap": plan["root_cap"],
        "tip_cap": plan["tip_cap"],
        "stable_triangulation": plan["stable_triangulation"],
        "stable_lineage_policy": plan["stable_lineage_policy"],
    }


def prepare_fixture(source: dict[str, Any], source_hash: dict[str, str], plan: dict[str, Any]) -> dict[str, Any]:
    caller_source_hashes = {
        key: value
        for key, value in source_hash.items()
        if key != "curve_graph_lookup_key_sha256"
    }
    request = {
        "schema_version": "KnifeCurveEvaluatedMeshPrepareRequest@1",
        "operation": "knife_curve_evaluated_mesh_prepare",
        **source,
        **caller_source_hashes,
        "evaluation_plan": plan,
        "idempotency_key": "knife-evaluated-mesh-prepare-1",
        "max_response_bytes": 1048576,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
    }
    request["input_sha256"] = input_hash(request)
    return request


def result_fixture(source: dict[str, Any], source_hash: dict[str, str], plan: dict[str, Any], *, operation: str = "knife_curve_evaluated_mesh_prepare", status: str = "prepared") -> dict[str, Any]:
    mesh = mesh_hashes(source, plan)
    identity_sha = identity_hash(source, plan, mesh)
    link_sha = link_hash(source, plan, mesh, identity_sha)
    result = {
        "schema_version": "KnifeCurveEvaluatedMeshResult@1",
        "operation": operation,
        "request_kind": "get" if operation.endswith("_get") else "prepare",
        "status": status,
        **source,
        **source_hash,
        "evaluated_mesh_lookup_key_sha256": evaluation_lookup(source, source_hash, plan, mesh, identity_sha, link_sha),
        "evaluation_plan": plan,
        "evaluation_plan_object_sha256": sha256(plan),
        "evaluation_plan_semantic_sha256": sha256(plan_semantic(plan)),
        **mesh,
        "evaluated_mesh_identity_sha256": identity_sha,
        "evaluated_mesh_link_sha256": link_sha,
        "closed_two_manifold": True,
        "zero_degenerate_triangles": True,
        "mesh_readback_status": "strict-evaluated-mesh-readback@1",
        "evaluation_status": "curve-sweep-loft-evaluated-mesh-created-no-geometry-artifact@1",
        "evaluated_mesh_created": True,
        "geometry_artifact_created": False,
        "replayed": False,
        "deterministic_replay": True,
        "byte_exact_replay": True,
        "restart_hash_verified": True,
        "idempotency_key": "knife-evaluated-mesh-prepare-1",
        "atomicity_status": "committed",
        "store_commit_status": "committed",
        "cas_commit_status": "committed",
        "runtime_write_performed": True,
        "persistent_user_data_touched": True,
        "partial_result_exposed": False,
        "stage_advanced": False,
        "candidate_confirmed": False,
        "version_created": False,
        "export_performed": False,
        "quality_status": "structural_only",
        "high_status": "NOT_RUN",
        "uv_status": "NOT_RUN",
        "bake_status": "NOT_RUN",
        "visual_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
    }
    if operation.endswith("_get"):
        result.update({
            "request_kind": "get", "status": "found", "replayed": False,
            "atomicity_status": "not-touched", "store_commit_status": "not-touched",
            "cas_commit_status": "not-touched", "runtime_write_performed": False,
            "persistent_user_data_touched": False, "idempotency_key": "knife-evaluated-mesh-get-1",
        })
    result["canonical_sha256"] = canonical_object_hash(result)
    return result


def get_fixture(source: dict[str, Any], source_hash: dict[str, str], result: dict[str, Any]) -> dict[str, Any]:
    fields = [
        "project_id", "source_candidate_id", "source_candidate_state_sha256",
        "source_authoring_mesh_id", "source_authoring_mesh_lineage_id",
        "source_authoring_mesh_revision_id", "source_authoring_mesh_revision_index",
        "source_authoring_mesh_revision_sha256", "source_authoring_mesh_identity_sha256",
        "source_modifier_graph_id", "source_modifier_graph_sha256",
    ]
    request = {
        "schema_version": "KnifeCurveEvaluatedMeshGetRequest@1",
        "operation": "knife_curve_evaluated_mesh_get",
        **{key: source[key] for key in fields},
        **{key: source_hash[key] for key in (
            "curve_set_semantic_sha256", "sample_set_semantic_sha256",
            "modifier_graph_semantic_sha256", "dependency_graph_semantic_sha256",
            "recompute_plan_semantic_sha256",
        )},
        "evaluated_mesh_lookup_key_sha256": result["evaluated_mesh_lookup_key_sha256"],
        "evaluation_id": result["evaluation_plan"]["evaluation_id"],
        "evaluation_plan_object_sha256": result["evaluation_plan_object_sha256"],
        "evaluation_plan_semantic_sha256": result["evaluation_plan_semantic_sha256"],
        "evaluated_mesh_id": result["evaluated_mesh_id"],
        "evaluated_mesh_object_sha256": result["evaluated_mesh_object_sha256"],
        "evaluated_mesh_semantic_sha256": result["evaluated_mesh_semantic_sha256"],
        "evaluated_mesh_identity_sha256": result["evaluated_mesh_identity_sha256"],
        "evaluated_mesh_link_sha256": result["evaluated_mesh_link_sha256"],
        "vertex_count": result["vertex_count"],
        "triangle_count": result["triangle_count"],
        "idempotency_key": "knife-evaluated-mesh-get-1",
        "max_response_bytes": 1048576,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
    }
    request["input_sha256"] = input_hash(request)
    return request


def check_semantics(source: dict[str, Any], source_hash: dict[str, str], plan: dict[str, Any]) -> None:
    require(plan["canonical_sha256"] == canonical_object_hash(plan), "evaluation plan canonical hash drifted")
    require(plan["spine_curve_id"] != plan["edge_curve_id"], "spine and edge curves must be distinct")
    require(plan["station_count"] == 32, "station count must remain the fixed 32-station slice")
    require(plan["thickness_axis"] in {"local_normal", "world_x", "world_y", "world_z"}, "thickness axis is outside the Core closed set")
    require(0.0001 < plan["thickness_m"] <= 0.25, "thickness bound drifted")
    require(plan["root_cap"] is True and plan["tip_cap"] is True, "sweep must cap root and tip")
    require(plan["stable_triangulation"] == "station-ring-fixed-diagonal@1", "stable triangulation policy drifted")
    require(plan["stable_lineage_policy"] == "source-curve-modifier-graph-evaluated-mesh@1", "stable lineage policy drifted")
    require(source["source_authoring_mesh_revision_sha256"] != source["source_modifier_graph_sha256"], "revision and ModifierGraph hashes must be independently bound")
    require(source_hash["modifier_graph_semantic_sha256"] != source_hash["curve_set_semantic_sha256"], "semantic graph hash must not alias curve hash")


def check_result(result: dict[str, Any], source: dict[str, Any], source_hash: dict[str, str], plan: dict[str, Any]) -> None:
    check_semantics(source, source_hash, plan)
    require(result["evaluation_plan_object_sha256"] == sha256(plan), "result plan object hash is stale")
    require(result["evaluation_plan_semantic_sha256"] == sha256(plan_semantic(plan)), "result plan semantic hash is stale")
    mesh = mesh_hashes(source, plan)
    for key in mesh:
        require(result[key] == mesh[key], f"result mesh binding drifted: {key}")
    identity_sha = identity_hash(source, plan, mesh)
    link_sha = link_hash(source, plan, mesh, identity_sha)
    require(result["evaluated_mesh_identity_sha256"] == identity_sha, "EvaluatedMeshIdentity does not bind real source revision/ModifierGraph")
    require(result["evaluated_mesh_link_sha256"] == link_sha, "EvaluatedMeshLink hash drifted")
    require(result["evaluated_mesh_lookup_key_sha256"] == evaluation_lookup(source, source_hash, plan, mesh, identity_sha, link_sha), "evaluated mesh lookup key drifted")
    require(result["evaluated_mesh_created"] is True and result["geometry_artifact_created"] is False, "evaluated/geometry artifact truth drifted")
    require(result["closed_two_manifold"] is True and result["zero_degenerate_triangles"] is True, "mesh topology truth drifted")
    require(result["high_status"] == "NOT_RUN" and result["uv_status"] == "NOT_RUN" and result["bake_status"] == "NOT_RUN", "downstream High/UV/Bake status overclaims")
    require(result["visual_status"] == "NOT_RUN" and result["human_status"] == "NOT_RUN" and result["engine_status"] == "NOT_RUN", "downstream review status overclaims")
    require(result["deterministic_replay"] is True and result["byte_exact_replay"] is True, "deterministic replay truth drifted")
    require(result["canonical_sha256"] == canonical_object_hash(result), "result canonical hash drifted")


def check_get(request: dict[str, Any], result: dict[str, Any]) -> None:
    require(request["runtime_write_performed"] is False, "get request must be read-only")
    for key in (
        "evaluated_mesh_lookup_key_sha256", "evaluation_id", "evaluation_plan_object_sha256",
        "evaluation_plan_semantic_sha256", "evaluated_mesh_id", "evaluated_mesh_object_sha256",
        "evaluated_mesh_semantic_sha256", "evaluated_mesh_identity_sha256",
        "evaluated_mesh_link_sha256", "vertex_count", "triangle_count",
    ):
        expected = result[key] if key in result else result["evaluation_plan"][key]
        require(request[key] == expected, f"get exact binding drifted: {key}")


def check_negative_cases(schemas: dict[str, dict[str, Any]], registry: dict[str, dict[str, Any]], prepare: dict[str, Any], result: dict[str, Any], get: dict[str, Any], source: dict[str, Any], source_hash: dict[str, str], plan: dict[str, Any]) -> None:
    candidate = copy.deepcopy(prepare)
    candidate["curve_graph_lookup_key_sha256"] = source_hash["curve_graph_lookup_key_sha256"]
    require(not is_valid(schemas[PREPARE], candidate, registry), "caller-supplied structural lookup key was accepted")
    candidate = copy.deepcopy(prepare)
    candidate["vertices"] = []
    require(not is_valid(schemas[PREPARE], candidate, registry), "raw vertices were accepted")
    candidate = copy.deepcopy(prepare)
    candidate["faces"] = []
    require(not is_valid(schemas[PREPARE], candidate, registry), "raw faces were accepted")
    candidate = copy.deepcopy(prepare)
    candidate["executor"] = "blender"
    require(not is_valid(schemas[PREPARE], candidate, registry), "arbitrary executor was accepted")
    candidate = copy.deepcopy(prepare)
    candidate["evaluation_plan"]["station_count"] = 33
    require(not is_valid(schemas[PREPARE], candidate, registry), "unbounded station count was accepted")
    candidate = copy.deepcopy(prepare)
    candidate["evaluation_plan"]["thickness_axis"] = "x"
    require(not is_valid(schemas[PREPARE], candidate, registry), "ambiguous thickness axis was accepted")
    candidate = copy.deepcopy(result)
    candidate["geometry_artifact_created"] = True
    require(not is_valid(schemas[RESULT], candidate, registry), "geometry artifact overclaim was accepted")
    candidate = copy.deepcopy(get)
    candidate["runtime_write_performed"] = True
    require(not is_valid(schemas[GET], candidate, registry), "get write was accepted")
    candidate = copy.deepcopy(result)
    candidate["evaluated_mesh_identity_sha256"] = candidate["evaluation_plan_object_sha256"]
    require(is_valid(schemas[RESULT], candidate, registry), "identity negative fixture no longer reaches semantic checker")
    try:
        check_result(candidate, source, source_hash, plan)
    except SystemExit:
        pass
    else:
        fail("EvaluatedMeshIdentity accepted an evaluation-plan hash alias")


def check_native_bindings() -> None:
    manifest = load(MANIFEST_PATH)
    source = load(SOURCE_SUMMARY_PATH)
    profile = load(PROFILE_ROOT / "weaponry-knife-p0.json")
    require(set(profile.get("native_operations", {})) == set(NATIVE_OPERATIONS), "profile native operation set drifted")
    legacy = set(source["read_names"]) | set(source["write_names"])
    require(not legacy & set(NATIVE_OPERATIONS), "new façade-native operations leaked into legacy raw tools/list")
    for operation, (classification, request_schema, result_schema) in NATIVE_OPERATIONS.items():
        metadata = profile["native_operations"][operation]
        require(metadata == {
            "operation_name": operation,
            "classification": classification,
            "facade_name": "authoring_transaction",
            "request_schema": request_schema,
            "result_schema": result_schema,
            "status": "native-development-only",
        }, f"native metadata drifted: {operation}")
    for name in (PREPARE, GET, RESULT):
        require(name in manifest["schemas"], f"new schema absent from core manifest: {name}")
    require(set(profile["facades"]) == set(FACADES) and list(profile["facades"]) == FACADES, "default façade set/order is not exactly 11")
    underlying = profile["facades"]["authoring_transaction"]["underlying_operations"]
    require(set(NATIVE_OPERATIONS) <= set(underlying), "native operations absent from authoring_transaction allowlist")


def check_legacy_compatibility() -> None:
    for name, expected in LEGACY_SCHEMA_BYTES_SHA256.items():
        actual = hashlib.sha256((SCHEMA_ROOT / name).read_bytes()).hexdigest()
        require(actual == expected, f"legacy @1 schema changed: {name}")


def run_checks() -> None:
    registry = load_schema_registry(load(MANIFEST_PATH))
    schemas = {name: load(SCHEMA_ROOT / name) for name in (PREPARE, GET, RESULT)}
    check_schema_shell(schemas[PREPARE], "KnifeCurveEvaluatedMeshPrepareRequest@1", PREPARE[:-len(".schema.json")])
    check_schema_shell(schemas[GET], "KnifeCurveEvaluatedMeshGetRequest@1", GET[:-len(".schema.json")])
    check_schema_shell(schemas[RESULT], "KnifeCurveEvaluatedMeshResult@1", RESULT[:-len(".schema.json")])
    require(schemas[RESULT]["properties"]["evaluated_mesh_created"].get("const") is True, "result must create evaluated mesh")
    require(schemas[RESULT]["properties"]["geometry_artifact_created"].get("const") is False, "result must not create geometry artifact")
    source = source_fields()
    source_hash = source_hashes(source)
    plan = plan_fixture(source)
    prepare = prepare_fixture(source, source_hash, plan)
    require(is_valid(schemas[PREPARE], prepare, registry), "valid prepare fixture rejected")
    result = result_fixture(source, source_hash, plan)
    require(is_valid(schemas[RESULT], result, registry), "valid result fixture rejected")
    check_result(result, source, source_hash, plan)
    get = get_fixture(source, source_hash, result)
    require(is_valid(schemas[GET], get, registry), "valid get fixture rejected")
    check_get(get, result)
    check_negative_cases(schemas, registry, prepare, result, get, source, source_hash, plan)
    check_native_bindings()
    check_legacy_compatibility()


def main() -> int:
    run_checks()
    print("Knife curve EvaluatedMesh contracts PASS: closed sweep-loft plan, real revision/ModifierGraph identity binding, disposable mesh truth, exact read lookup, native/legacy separation, and negative cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
