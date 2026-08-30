#!/usr/bin/env python3
"""Focused negative/positive gate for the façade-native knife curve slice.

This checker is intentionally independent of MCP and Runtime code.  It proves
that the three closed schemas describe the Rust ``KnifeCurve`` and
``ModifierGraph`` vocabulary, bind the source revision and hashes, and cannot
silently grow into a mesh/artifact or arbitrary-executor contract.
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
MANIFEST_PATH = ROOT / "packages" / "forgecad-contracts" / "manifest.json"
SOURCE_SUMMARY_PATH = ROOT / "docs" / "evidence" / "mcp010f" / "source-tool-manifest-summary.json"

sys.path.insert(0, str(ROOT / "scripts"))
from check_agentic_contracts import is_valid, load_schema_registry  # noqa: E402

PREPARE = "knife-curve-modifier-graph-prepare-request.schema.json"
GET = "knife-curve-modifier-graph-get-request.schema.json"
RESULT = "knife-curve-modifier-graph-result.schema.json"
NATIVE_OPERATIONS = {
    "knife_curve_modifier_graph_prepare": ("write", "KnifeCurveModifierGraphPrepareRequest@1"),
    "knife_curve_modifier_graph_get": ("read", "KnifeCurveModifierGraphGetRequest@1"),
}
H = "0123456789abcdef" * 4


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"Knife curve/modifier graph contract violation: {message}")


def load(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise SystemExit(f"Knife curve/modifier graph contract violation: cannot load {path}: {exc}")
    require(isinstance(value, dict), f"{path.name} must be an object")
    return value


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False
    ).encode("utf-8")


def sha256(value: Any) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def canonical_object_hash(value: dict[str, Any]) -> str:
    payload = copy.deepcopy(value)
    payload.pop("canonical_sha256", None)
    return sha256(payload)


def check_schema_shell(schema: dict[str, Any], expected_version: str, label: str) -> None:
    require(schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema", f"{label} draft drifted")
    require(schema.get("$id") == f"https://forgecad.local/contracts/{label}.schema.json", f"{label} id drifted")
    require(schema.get("type") == "object" and schema.get("additionalProperties") is False, f"{label} root is not closed")
    require(schema.get("properties", {}).get("schema_version", {}).get("const") == expected_version, f"{label} version drifted")
    require("schema_version" in schema.get("required", []), f"{label} is not version-bound")
    forbidden = {"path", "url", "uri", "script", "shell", "secret", "token", "password", "api_key", "raw_bytes"}

    def walk(node: Any) -> None:
        if not isinstance(node, dict):
            if isinstance(node, list):
                for child in node:
                    walk(child)
            return
        properties = node.get("properties")
        if isinstance(properties, dict):
            require(not {name.lower() for name in properties} & forbidden, f"{label} exposes a forbidden property")
            for child in properties.values():
                walk(child)
        for key in ("$defs", "items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"):
            child = node.get(key)
            if isinstance(child, dict):
                walk(child)
            elif isinstance(child, list):
                for value in child:
                    walk(value)

    walk(schema)


def curve_fixture() -> dict[str, Any]:
    curve = {
        "curve_id": "blade-spine-1",
        "role": "blade_spine",
        "basis": "bezier",
        "degree": 2,
        "control_points_m": [[0.0, 0.0, 0.0], [0.1, 0.0, 0.2], [0.2, 0.0, 0.4]],
        "weights": [],
        "knots": [],
        "closed": False,
    }
    curve["canonical_sha256"] = canonical_object_hash(curve)
    return curve


def modifier_graph_fixture(curve: dict[str, Any]) -> dict[str, Any]:
    graph = {
        "graph_id": "knife-graph-1",
        "source_revision_id": "authoring-revision-7",
        "source_revision_sha256": H,
        "nodes": [
            {
                "node_id": "curve-node",
                "operator": {"operator": "curve_profile", "curve_id": curve["curve_id"], "curve_sha256": curve["canonical_sha256"]},
                "input_node_ids": [],
                "selection_query_sha256": None,
                "enabled": True,
            },
            {
                "node_id": "transform-node",
                "operator": {"operator": "transform", "translation_m": [0.0, 0.0, 0.0], "rotation_rad": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0]},
                "input_node_ids": ["curve-node"],
                "selection_query_sha256": None,
                "enabled": True,
            },
        ],
        "output_node_ids": ["transform-node"],
    }
    graph["canonical_sha256"] = canonical_object_hash(graph)
    return graph


def source_fields() -> dict[str, Any]:
    return {
        "project_id": "project-knife-1",
        "source_candidate_id": "candidate-knife-1",
        "source_candidate_state_sha256": H,
        "source_authoring_mesh_id": "authoring-mesh-1",
        "source_authoring_mesh_lineage_id": "authoring-lineage-1",
        "source_authoring_mesh_revision_id": "authoring-revision-7",
        "source_authoring_mesh_revision_index": 7,
        "source_authoring_mesh_revision_sha256": H,
        "source_authoring_mesh_identity_sha256": H,
    }


def prepare_fixture() -> dict[str, Any]:
    curve = curve_fixture()
    graph = modifier_graph_fixture(curve)
    request = {
        "schema_version": "KnifeCurveModifierGraphPrepareRequest@1",
        "operation": "knife_curve_modifier_graph_prepare",
        **source_fields(),
        "curves": [curve],
        "modifier_graph": graph,
        "dirty_seeds": ["curve-node"],
        "recompute_policy": "dirty-seed-dependency-closure-recompute@1",
        "evaluation_policy": "original-authoring-mesh-modifier-graph-deterministic@1",
        "idempotency_key": "knife-curve-prepare-1",
        "max_response_bytes": 1048576,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
    }
    request["input_sha256"] = canonical_object_hash(request)
    return request


def derived_hashes(request: dict[str, Any]) -> dict[str, Any]:
    curves = request["curves"]
    graph = request["modifier_graph"]
    curve_set_object = {"curves": curves}
    curve_set_semantic = [{"curve_id": c["curve_id"], "role": c["role"], "basis": c["basis"], "degree": c["degree"]} for c in curves]
    sample_set_object = {"curve_set_sha256": sha256(curve_set_object), "sample_count": 4, "sampling_policy": "bounded-linear-parameter-sampling@1"}
    sample_set_semantic = {"curve_ids": [c["curve_id"] for c in curves], "sample_count": 4}
    dependency_object = {"source_revision_id": graph["source_revision_id"], "node_ids": ["__source_revision__", "__curve-" + curves[0]["canonical_sha256"], "curve-node", "transform-node"]}
    dependency_semantic = {"node_ids": ["curve-node", "transform-node"], "source_revision_sha256": graph["source_revision_sha256"]}
    recompute_object = {"dirty_seed_node_ids": request["dirty_seeds"], "recomputed_node_ids": ["curve-node", "transform-node"]}
    recompute_semantic = {"dirty_seed_node_ids": request["dirty_seeds"], "recompute_order": ["curve-node", "transform-node"]}
    return {
        "curve_set_object_sha256": sha256(curve_set_object),
        "curve_set_semantic_sha256": sha256(curve_set_semantic),
        "sample_set_object_sha256": sha256(sample_set_object),
        "sample_set_semantic_sha256": sha256(sample_set_semantic),
        "modifier_graph_object_sha256": sha256(graph),
        "modifier_graph_semantic_sha256": sha256({"graph_id": graph["graph_id"], "source_revision_sha256": graph["source_revision_sha256"], "node_ids": [n["node_id"] for n in graph["nodes"]]}),
        "dependency_graph_object_sha256": sha256(dependency_object),
        "dependency_graph_semantic_sha256": sha256(dependency_semantic),
        "recompute_plan_object_sha256": sha256(recompute_object),
        "recompute_plan_semantic_sha256": sha256(recompute_semantic),
    }


def get_fixture(request: dict[str, Any], derived: dict[str, Any]) -> dict[str, Any]:
    result = {
        "schema_version": "KnifeCurveModifierGraphGetRequest@1",
        "operation": "knife_curve_modifier_graph_get",
        **{key: request[key] for key in source_fields()},
        "curve_set_semantic_sha256": derived["curve_set_semantic_sha256"],
        "sample_set_semantic_sha256": derived["sample_set_semantic_sha256"],
        "modifier_graph_semantic_sha256": derived["modifier_graph_semantic_sha256"],
        "dependency_graph_semantic_sha256": derived["dependency_graph_semantic_sha256"],
        "recompute_plan_semantic_sha256": derived["recompute_plan_semantic_sha256"],
        "lookup_key_sha256": H,
        "idempotency_key": "knife-curve-get-1",
        "max_response_bytes": 1048576,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
    }
    result["input_sha256"] = canonical_object_hash(result)
    return result


def result_fixture(request: dict[str, Any], derived: dict[str, Any]) -> dict[str, Any]:
    result = {
        "schema_version": "KnifeCurveModifierGraphResult@1",
        "operation": "knife_curve_modifier_graph_prepare",
        "request_kind": "prepare",
        "status": "prepared",
        **{key: request[key] for key in source_fields()},
        **derived,
        "dirty_seed_node_ids": ["curve-node"],
        "recomputed_node_ids": ["curve-node", "transform-node"],
        "reused_node_ids": [],
        "evaluation_status": "curve-sampled-modifier-recompute-planned-no-mesh@1",
        "evaluated_mesh_created": False,
        "geometry_artifact_created": False,
        "replayed": False,
        "deterministic_replay": True,
        "byte_exact_replay": True,
        "restart_hash_verified": False,
        "idempotency_key": request["idempotency_key"],
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
        "visual_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
    }
    result["canonical_sha256"] = canonical_object_hash(result)
    return result


def check_core_relationships(request: dict[str, Any]) -> None:
    curves = request["curves"]
    graph = request["modifier_graph"]
    require(len({curve["curve_id"] for curve in curves}) == len(curves), "curve IDs must be unique")
    for curve in curves:
        require(curve["canonical_sha256"] == canonical_object_hash(curve), f"curve hash drifted for {curve['curve_id']}")
        points = curve["control_points_m"]
        require(1 <= len(points) <= 64, "curve control point bound drifted")
        if curve["basis"] == "bezier":
            require(len(points) == curve["degree"] + 1 and curve["weights"] == [] and curve["knots"] == [], "Bezier core invariants drifted")
        else:
            require(len(curve["weights"]) == len(points), "NURBS-like weights must match points")
            require(len(curve["knots"]) == len(points) + curve["degree"] + 1, "NURBS-like knot count drifted")
            require(curve["knots"] == sorted(curve["knots"]), "NURBS-like knots must be non-decreasing")
            require(curve["knots"][: curve["degree"] + 1] == [0.0] * (curve["degree"] + 1), "NURBS-like zero clamp missing")
            require(curve["knots"][-(curve["degree"] + 1):] == [1.0] * (curve["degree"] + 1), "NURBS-like one clamp missing")
    require(graph["source_revision_id"] == request["source_authoring_mesh_revision_id"], "graph/source revision id mismatch")
    require(graph["source_revision_sha256"] == request["source_authoring_mesh_revision_sha256"], "graph/source revision hash mismatch")
    require(graph["canonical_sha256"] == canonical_object_hash(graph), "modifier graph hash drifted")
    node_ids = [node["node_id"] for node in graph["nodes"]]
    node_set = set(node_ids)
    require(len(node_ids) == len(node_set), "modifier node IDs must be unique")
    require(all(node_id != "__source_revision__" and not node_id.startswith(("__selection-", "__curve-")) for node_id in node_ids), "reserved dependency node id was accepted")
    require(set(graph["output_node_ids"]) <= node_set, "graph output references an unknown node")
    curve_by_id = {curve["curve_id"]: curve for curve in curves}
    for node in graph["nodes"]:
        require(set(node["input_node_ids"]) <= node_set, f"{node['node_id']} references an unknown input")
        operator = node["operator"]
        require(operator["operator"] in {"transform", "mirror", "array", "bevel", "normal_policy", "curve_profile"}, "unsupported ModifierKind was accepted")
        if operator["operator"] == "curve_profile":
            require(operator["curve_id"] in curve_by_id, "CurveProfile references an undeclared curve")
            require(operator["curve_sha256"] == curve_by_id[operator["curve_id"]]["canonical_sha256"], "CurveProfile hash is not curve-bound")
    require(set(request["dirty_seeds"]) <= node_set, "dirty seed is not a direct stable graph node ID")
    pending = {node["node_id"]: set(node["input_node_ids"]) for node in graph["nodes"]}
    order: list[str] = []
    while pending:
        ready = sorted(node_id for node_id, inputs in pending.items() if not inputs)
        require(ready, "modifier graph cycle was accepted")
        order.extend(ready)
        for node_id in ready:
            pending.pop(node_id)
        for inputs in pending.values():
            inputs.difference_update(ready)


def check_result_relationships(result: dict[str, Any], request: dict[str, Any], derived: dict[str, Any]) -> None:
    require(result["source_authoring_mesh_revision_id"] == request["source_authoring_mesh_revision_id"], "result source revision id drifted")
    for key, value in derived.items():
        require(result[key] == value, f"result {key} is not bound to its source object")
    require(set(result["recomputed_node_ids"]).isdisjoint(result["reused_node_ids"]), "recomputed/reused node sets overlap")
    graph_ids = {node["node_id"] for node in request["modifier_graph"]["nodes"]}
    require(set(result["dirty_seed_node_ids"]) <= graph_ids, "result dirty seed is not a graph node")
    require(set(result["recomputed_node_ids"]) | set(result["reused_node_ids"]) <= graph_ids, "result node partition contains an unknown node")
    require(result["evaluated_mesh_created"] is False and result["geometry_artifact_created"] is False, "no-mesh result flags drifted")
    require(result["evaluation_status"] == "curve-sampled-modifier-recompute-planned-no-mesh@1", "evaluation status overclaims execution")
    require(result["canonical_sha256"] == canonical_object_hash(result), "result canonical hash drifted")


def negative_cases(schemas: dict[str, dict[str, Any]], registry: dict[str, dict[str, Any]], request: dict[str, Any], result: dict[str, Any]) -> None:
    prepare_schema = schemas[PREPARE]
    result_schema = schemas[RESULT]
    candidate = copy.deepcopy(request)
    candidate["path"] = "not-allowed"
    require(not is_valid(prepare_schema, candidate, registry), "path field was accepted")
    candidate = copy.deepcopy(request)
    candidate["curves"][0]["role"] = "fuller_groove"
    require(not is_valid(prepare_schema, candidate, registry), "non-core curve role was accepted")
    candidate = copy.deepcopy(request)
    candidate["modifier_graph"]["nodes"][0]["operator"]["operator"] = "boolean"
    require(not is_valid(prepare_schema, candidate, registry), "non-core ModifierKind was accepted")
    candidate = copy.deepcopy(request)
    candidate["dirty_seeds"] = ["unknown-node"]
    require(is_valid(prepare_schema, candidate, registry), "dirty-seed negative fixture no longer reaches semantic checker")
    try:
        check_core_relationships(candidate)
    except SystemExit:
        pass
    else:
        raise SystemExit("Knife curve/modifier graph contract violation: semantic checker accepted an unknown dirty seed")
    candidate = copy.deepcopy(result)
    candidate["evaluated_mesh_created"] = True
    require(not is_valid(result_schema, candidate, registry), "result accepted a created evaluated mesh")
    candidate = copy.deepcopy(result)
    candidate["canonical_sha256"] = "0" * 64
    require(is_valid(result_schema, candidate, registry), "result canonical negative fixture no longer reaches semantic checker")
    try:
        check_result_relationships(candidate, request, derived_hashes(request))
    except SystemExit:
        pass
    else:
        raise SystemExit("Knife curve/modifier graph contract violation: semantic checker accepted a stale canonical hash")


def check_native_operations() -> None:
    manifest = load(MANIFEST_PATH)
    profile = load(ROOT / "packages" / "forgecad-contracts" / "profiles" / "weaponry-knife-p0.json")
    source = load(SOURCE_SUMMARY_PATH)
    native = profile.get("native_operations")
    # The structural replay operations remain the compatibility subset.  The
    # knife profile may add façade-native successors (such as EvaluatedMesh),
    # but this checker deliberately validates only the original two and their
    # unchanged schema bindings.
    require(set(NATIVE_OPERATIONS) <= set(native or {}), "profile structural native operation set drifted")
    legacy = set(source["read_names"]) | set(source["write_names"])
    require(not legacy & set(NATIVE_OPERATIONS), "native operation leaked into legacy 226 source manifest")
    for operation, (classification, request_schema_version) in NATIVE_OPERATIONS.items():
        metadata = native[operation]
        require(metadata["operation_name"] == operation and metadata["classification"] == classification, f"native operation metadata drifted: {operation}")
        require(metadata["facade_name"] == "authoring_transaction", f"native operation facade drifted: {operation}")
        require(metadata["request_schema"] == request_schema_version and metadata["result_schema"] == "KnifeCurveModifierGraphResult@1", f"native schema binding drifted: {operation}")
        require(metadata["status"] == "native-development-only", f"native status drifted: {operation}")
        schema_file = {"knife_curve_modifier_graph_prepare": PREPARE, "knife_curve_modifier_graph_get": GET}[operation]
        require(schema_file in manifest["schemas"], f"native schema is absent from core manifest: {schema_file}")
        schema = load(SCHEMA_ROOT / schema_file)
        require(schema["properties"]["schema_version"]["const"] == request_schema_version, f"native request schema version drifted: {operation}")
    result_schema = load(SCHEMA_ROOT / RESULT)
    require(RESULT in manifest["schemas"] and result_schema["properties"]["schema_version"]["const"] == "KnifeCurveModifierGraphResult@1", "native result schema binding missing")


def run_checks() -> None:
    manifest = load(MANIFEST_PATH)
    registry = load_schema_registry(manifest)
    schemas = {name: load(SCHEMA_ROOT / name) for name in (PREPARE, GET, RESULT)}
    check_schema_shell(schemas[PREPARE], "KnifeCurveModifierGraphPrepareRequest@1", PREPARE[:-len(".schema.json")])
    check_schema_shell(schemas[GET], "KnifeCurveModifierGraphGetRequest@1", GET[:-len(".schema.json")])
    check_schema_shell(schemas[RESULT], "KnifeCurveModifierGraphResult@1", RESULT[:-len(".schema.json")])
    prepare = prepare_fixture()
    check_core_relationships(prepare)
    require(is_valid(schemas[PREPARE], prepare, registry), "valid prepare fixture rejected")
    derived = derived_hashes(prepare)
    get_request = get_fixture(prepare, derived)
    require(is_valid(schemas[GET], get_request, registry), "valid get fixture rejected")
    result = result_fixture(prepare, derived)
    require(is_valid(schemas[RESULT], result, registry), "valid result fixture rejected")
    check_result_relationships(result, prepare, derived)
    negative_cases(schemas, registry, prepare, result)
    check_native_operations()


def main() -> int:
    run_checks()
    print("Knife Curve+ModifierGraph contracts PASS: core-typed curves, six ModifierKind operators, direct dirty seeds, no-mesh truthful result, native/legacy bindings, and negative cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
