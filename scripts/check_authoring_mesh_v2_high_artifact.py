#!/usr/bin/env python3
"""Closed contract gate for WPN-KNIFE-HIGH-ARTIFACT-001.

This is a contract/fixture gate only.  It proves that a V2 High Bridge can be
described as a Runtime-derived GLB adapter with strict readback and Low
adapter identities.  It does not run a Worker, create a GLB, or prove visual,
human, engine, or commercial quality.
"""

from __future__ import annotations

import copy
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_ROOT = ROOT / "packages" / "forgecad-contracts"
SCHEMA_ROOT = CONTRACT_ROOT / "schemas"
FIXTURE_ROOT = CONTRACT_ROOT / "fixtures" / "authoring-mesh-v2-high-artifact"
MANIFEST_PATH = CONTRACT_ROOT / "manifest.json"
SOURCE_BINDING_PATH = CONTRACT_ROOT / "fixtures" / "knife-source-binding" / "positive" / "dragonfang-source-binding.json"
BRIDGE_PATH = CONTRACT_ROOT / "fixtures" / "authoring-mesh-v2-high-bridge" / "positive" / "dragonfang-high-bridge.json"

MAIN_SCHEMA = "authoring-mesh-v2-high-artifact.schema.json"
PREPARE_SCHEMA = "authoring-mesh-v2-high-artifact-prepare-request.schema.json"
GET_SCHEMA = "authoring-mesh-v2-high-artifact-get-request.schema.json"
RESULT_SCHEMA = "authoring-mesh-v2-high-artifact-result.schema.json"
MAIN_TITLE = "AuthoringMeshV2HighArtifact@1"
PREPARE_TITLE = "AuthoringMeshV2HighArtifactPrepareRequest@1"
GET_TITLE = "AuthoringMeshV2HighArtifactGetRequest@1"
RESULT_TITLE = "AuthoringMeshV2HighArtifactResult@1"

MAIN_PATH = FIXTURE_ROOT / "positive" / "dragonfang-high-artifact.json"
PREPARE_PATH = FIXTURE_ROOT / "positive" / "dragonfang-high-artifact-prepare-request.json"
GET_PATH = FIXTURE_ROOT / "positive" / "dragonfang-high-artifact-get-request.json"
RESULT_PATH = FIXTURE_ROOT / "positive" / "dragonfang-high-artifact-result-prepared.json"
NEGATIVE_PATH = FIXTURE_ROOT / "negative" / "cases.json"

MAIN_FIELD_COUNT = 106
PREPARE_FIELD_COUNT = 13
GET_FIELD_COUNT = 19
RESULT_FIELD_COUNT = 75
SOURCE_SCOPE = "single-v2-revision-part@1"
V2_KIND = "authoring-mesh-v2-high-artifact-glb@1"
V2_READBACK_KIND = "authoring-mesh-v2-high-artifact-readback@1"
LOW_KIND = "production-weapon-high-artifact-glb"
LOW_READBACK_KIND = "native-high-glb-materialize-result"
LOW_SOURCE_SCHEMA = "HighMeshArtifact@1"
POLICY = "forgecad-runtime-only-state-writer@1"
MAIN_CANONICALIZATION = "canonical-json-sha256-excluding-canonical-sha256@1"
REQUEST_CANONICALIZATION = "canonical-json-sha256-excluding-input-sha256@1"
ARTIFACT_POLICY = "authoring-mesh-v2-high-bridge-to-low-glb-adapter@1"
GLB_OPERATION = "forgecad.production.authoring-mesh-v2-high-artifact-materialize@1"
SCOPE_LIMITATIONS = [
    "SINGLE_V2_REVISION_PART_ONLY",
    "RUNTIME_DERIVES_GLTF_FROM_HIGH_BRIDGE",
    "LOW_CONSUMPTION_REQUIRES_STRICT_GLTF_READBACK",
    "NO_CALLER_SUPPLIED_GLTF_BYTES",
    "NO_STAGE_ADVANCEMENT",
    "NO_VISUAL_OR_HUMAN_ACCEPTANCE",
]
SHA256 = re.compile(r"^[0-9a-f]{64}$")
IDENTIFIER = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$")

sys.path.insert(0, str(ROOT / "scripts"))
from check_agentic_contracts import is_valid, load_schema_registry  # noqa: E402


class ContractViolation(Exception):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ContractViolation(message)


def load(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise ContractViolation(f"cannot load {path.relative_to(ROOT)}: {exc}") from exc


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def sha256(value: Any) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def canonical_hash(value: dict[str, Any]) -> str:
    preimage = copy.deepcopy(value)
    preimage["canonical_sha256"] = ""
    return sha256(preimage)


def input_hash(value: dict[str, Any]) -> str:
    preimage = copy.deepcopy(value)
    preimage["input_sha256"] = ""
    return sha256(preimage)


def obj(path: Path) -> dict[str, Any]:
    value = load(path)
    require(isinstance(value, dict), f"{path.relative_to(ROOT)} is not an object")
    return value


def walk_objects(node: Any) -> list[dict[str, Any]]:
    if not isinstance(node, dict):
        return []
    found: list[dict[str, Any]] = []
    if node.get("type") == "object":
        found.append(node)
    for key in ("properties", "$defs", "definitions"):
        child = node.get(key)
        if isinstance(child, dict):
            for value in child.values():
                found.extend(walk_objects(value))
    for key in ("items", "prefixItems", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"):
        child = node.get(key)
        if isinstance(child, list):
            for value in child:
                found.extend(walk_objects(value))
        elif isinstance(child, dict):
            found.extend(walk_objects(child))
    return found


def walk_property_names(node: Any) -> list[str]:
    if not isinstance(node, dict):
        return []
    names: list[str] = []
    props = node.get("properties")
    if isinstance(props, dict):
        names.extend(props)
        for value in props.values():
            names.extend(walk_property_names(value))
    for key in ("$defs", "definitions", "items", "prefixItems", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"):
        child = node.get(key)
        if isinstance(child, list):
            for value in child:
                names.extend(walk_property_names(value))
        elif isinstance(child, dict):
            names.extend(walk_property_names(child))
    return names


def schema_shell(schema: dict[str, Any], filename: str, title: str) -> None:
    require(schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema", f"{filename} draft drifted")
    require(schema.get("$id") == f"https://forgecad.local/contracts/{filename}", f"{filename} id drifted")
    require(schema.get("title") == title, f"{filename} title drifted")
    require(schema.get("type") == "object" and schema.get("additionalProperties") is False, f"{filename} root is open")
    properties = schema.get("properties", {})
    require(set(schema.get("required", [])) == set(properties), f"{filename} required/properties drifted")
    require(properties.get("schema_version", {}).get("const") == title, f"{filename} schema version drifted")
    for local in walk_objects(schema):
        require(local.get("additionalProperties") is False, f"{filename} contains an open local object")
    forbidden = {"path", "url", "uri", "raw", "raw_bytes", "bytes", "secret", "token", "password", "api_key", "prompt", "script", "shell", "environment", "executor", "topology", "steps", "revision"}
    require(not ({name.lower() for name in walk_property_names(schema)} & forbidden), f"{filename} exposes raw/evaluator property")


def check_schemas(manifest: dict[str, Any], registry: dict[str, dict[str, Any]]) -> dict[str, dict[str, Any]]:
    entries = [(MAIN_SCHEMA, MAIN_TITLE, MAIN_FIELD_COUNT), (PREPARE_SCHEMA, PREPARE_TITLE, PREPARE_FIELD_COUNT), (GET_SCHEMA, GET_TITLE, GET_FIELD_COUNT), (RESULT_SCHEMA, RESULT_TITLE, RESULT_FIELD_COUNT)]
    checked: dict[str, dict[str, Any]] = {}
    for filename, title, count in entries:
        require(filename in manifest.get("schemas", []), f"manifest omits {filename}")
        schema = obj(SCHEMA_ROOT / filename)
        schema_shell(schema, filename, title)
        require(len(schema["required"]) == count, f"{filename} field count drifted")
        require(registry.get(schema["$id"]) == schema, f"{filename} is not registry-bound")
        checked[filename] = schema
    result = checked[RESULT_SCHEMA]
    require(result["properties"]["high_artifact"].get("$ref") == f"https://forgecad.local/contracts/{MAIN_SCHEMA}", "Result does not bind Main")
    require(checked[MAIN_SCHEMA]["properties"]["high_worker_build_cohort_sha256"].get("$ref") == "#/$defs/sha256", "Worker cohort is not SHA-bound")
    return checked


def check_main(main: dict[str, Any], bridge: dict[str, Any], source_binding: dict[str, Any], schema: dict[str, Any], registry: dict[str, dict[str, Any]]) -> None:
    require(set(main) == set(schema["properties"]), "Main fixture fields differ from closed schema")
    require(is_valid(schema, main, registry), "positive Main is schema-invalid")
    require(main["schema_version"] == MAIN_TITLE and main["source_scope"] == SOURCE_SCOPE, "Main version/scope drifted")
    require(main["project_id"] == bridge["project_id"] == source_binding["project_id"], "Main project binding drifted")
    for field, expected in {
        "high_bridge_id": bridge["bridge_id"], "high_bridge_sha256": bridge["canonical_sha256"], "high_bridge_object_sha256": sha256(bridge),
        "source_binding_id": source_binding["source_binding_id"], "source_binding_sha256": source_binding["canonical_sha256"], "source_binding_object_sha256": sha256(source_binding),
        "source_revision_id": bridge["revision_id"], "source_revision_index": bridge["revision_index"], "source_revision_sha256": bridge["revision_sha256"], "source_revision_object_sha256": bridge["revision_object_sha256"],
        "high_artifact_kind": V2_KIND, "high_artifact_readback_kind": V2_READBACK_KIND, "low_compatibility_artifact_kind": LOW_KIND, "low_compatibility_readback_kind": LOW_READBACK_KIND, "low_compatibility_source_schema_version": LOW_SOURCE_SCHEMA,
        "high_artifact_policy": ARTIFACT_POLICY, "glb_materialization_operation": GLB_OPERATION, "scope_limitations": SCOPE_LIMITATIONS,
    }.items():
        require(main[field] == expected, f"Main {field} is not bound to its source or policy")
    require(main["high_artifact_sha256"] == main["high_artifact_object_sha256"] == main["glb_sha256"] == main["glb_object_sha256"], "GLB semantic/object aliases drifted")
    for field in (
        "high_execution_request_sha256", "high_evaluation_sha256", "high_result_sha256",
        "high_result_object_sha256", "high_readback_sha256", "high_readback_object_sha256",
        "high_worker_algorithm_sha256", "high_worker_build_cohort_sha256",
    ):
        require(main[field] == bridge[field], f"Main {field} is not High Bridge Worker-bound")
    require(main["source_part_id"] == bridge["part_id"] and main["source_node_id"] == bridge["source_node_id"], "Main source Part/node drifted")
    require(main["high_part_ids"] == [bridge["part_id"]] and main["high_material_zone_ids"] == [bridge["material_zone_id"]], "Main exceeds the current single-Part High scope")
    require(main["high_artifact_readback_schema_version"] == "AuthoringMeshV2HighArtifactStoreReadback@1", "durable Store readback identity drifted")
    strict = main["strict_readback"]
    require(strict["glb_sha256"] == main["glb_sha256"] and strict["glb_object_sha256"] == main["glb_object_sha256"], "readback GLB binding drifted")
    require(strict["source_artifact_id"] == main["high_mesh_artifact_id"] and strict["source_artifact_sha256"] == main["high_mesh_artifact_sha256"], "readback source evaluator binding drifted")
    require(strict["part_ids"] == main["high_part_ids"] and strict["material_zone_ids"] == main["high_material_zone_ids"], "readback Part/material inventory drifted")
    require(strict["source_node_ids"] == [binding["source_node_id"] for binding in strict["part_bindings"]], "readback source nodes drifted")
    require(main["high_part_inventory_sha256"] == sha256({"part_ids": main["high_part_ids"], "material_zone_ids": main["high_material_zone_ids"], "part_bindings": strict["part_bindings"]}), "Part inventory hash drifted")
    require(main["glb_source_schema_version"] == LOW_SOURCE_SCHEMA and main["glb_source_artifact_id"] == main["high_mesh_artifact_id"] and main["glb_source_artifact_sha256"] == main["high_mesh_artifact_sha256"], "GLB source extras identity drifted")
    require(main["high_replay_count"] == 2 and main["high_replay_byte_exact"] is True and main["high_non_destructive"] is True, "Worker replay proof drifted")
    require(main["high_artifact_status"] == main["structural_status"] == "PASS_SOURCE_STRUCTURAL" and main["quality_status"] == "structural_only", "structural status drifted")
    require(main["high_artifact_hard_gate_passed"] is True and main["high_mesh_created"] is True, "persisted GLB artifact gate drifted")
    for field in ("high_stage_unlocked", "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed"):
        require(main[field] is False, f"Main falsely advances {field}")
    require(main["visual_status"] == main["human_status"] == main["engine_status"] == main["distribution_status"] == "NOT_RUN", "Main falsely promotes downstream status")
    require(main["runtime_write_performed"] is True and main["persistent_user_data_touched"] is True, "Main is not Runtime durable")
    require(main["canonical_sha256"] == canonical_hash(main), "Main canonical hash drifted")
    for field in ("high_bridge_sha256", "high_bridge_object_sha256", "source_binding_sha256", "source_binding_object_sha256", "high_worker_build_cohort_sha256", "high_artifact_sha256", "high_artifact_object_sha256", "high_artifact_readback_sha256", "high_artifact_readback_object_sha256"):
        require(SHA256.fullmatch(main[field]) is not None, f"Main {field} is not a SHA-256 identity")
    require(main["writer_policy"] == POLICY and main["canonicalization_policy"] == MAIN_CANONICALIZATION, "Main policy drifted")


def check_prepare(prepare: dict[str, Any], main: dict[str, Any], schema: dict[str, Any], registry: dict[str, dict[str, Any]]) -> None:
    require(set(prepare) == set(schema["properties"]), "Prepare fixture fields differ from closed schema")
    require(is_valid(schema, prepare, registry), "positive Prepare is schema-invalid")
    require(prepare["operation"] == "authoring_mesh_v2_high_artifact_prepare", "Prepare operation drifted")
    for field in ("project_id", "high_artifact_id", "high_bridge_id", "high_bridge_sha256", "high_bridge_object_sha256"):
        require(prepare[field] == main[field], f"Prepare {field} is not Main-bound")
    require(prepare["runtime_write_performed"] is False and prepare["max_response_bytes"] == 1048576, "Prepare write/size gate drifted")
    require(prepare["canonicalization_policy"] == REQUEST_CANONICALIZATION and prepare["input_sha256"] == input_hash(prepare), "Prepare input hash/policy drifted")


def check_get(get: dict[str, Any], main: dict[str, Any], schema: dict[str, Any], registry: dict[str, dict[str, Any]]) -> None:
    require(set(get) == set(schema["properties"]), "Get fixture fields differ from closed schema")
    require(is_valid(schema, get, registry), "positive Get is schema-invalid")
    require(get["operation"] == "authoring_mesh_v2_high_artifact_get", "Get operation drifted")
    for field in ("project_id", "high_artifact_id", "high_artifact_sha256", "high_artifact_object_sha256", "high_artifact_readback_sha256", "high_artifact_readback_object_sha256", "high_artifact_receipt_sha256", "high_artifact_receipt_object_sha256", "high_bridge_id", "high_bridge_sha256", "high_bridge_object_sha256"):
        require(get[field] == main[field], f"Get {field} is not Main-bound")
    require(get["runtime_write_performed"] is False and get["persistent_user_data_touched"] is False and get["input_sha256"] == input_hash(get), "Get read-only/input gate drifted")


def check_result(result: dict[str, Any], main: dict[str, Any], prepare: dict[str, Any], schema: dict[str, Any], registry: dict[str, dict[str, Any]]) -> None:
    require(set(result) == set(schema["properties"]), "Result fixture fields differ from closed schema")
    require(is_valid(schema, result, registry), "positive Result is schema-invalid")
    require(result["operation"] == "authoring_mesh_v2_high_artifact_prepare" and result["request_kind"] == "prepare" and result["status"] == "prepared", "Result prepared branch drifted")
    require(result["high_artifact"] == main, "Result nested Main drifted")
    for field in ("project_id", "high_artifact_id", "high_artifact_sha256", "high_artifact_object_sha256", "high_artifact_readback_sha256", "high_artifact_readback_object_sha256", "high_artifact_receipt_sha256", "high_artifact_receipt_object_sha256", "high_bridge_id", "high_bridge_sha256", "high_bridge_object_sha256", "high_mesh_artifact_id", "high_mesh_artifact_sha256", "high_mesh_artifact_object_sha256", "high_worker_build_cohort_sha256", "high_part_ids", "high_material_zone_ids"):
        require(result[field] == main[field], f"Result {field} is not Main-bound")
    require(result["request_input_sha256"] == prepare["input_sha256"] and result["replayed"] is False and result["idempotency_key"] == prepare["idempotency_key"], "Result request/replay binding drifted")
    require(result["store_effect"] == result["cas_effect"] == "inserted" and result["atomicity_status"] == result["store_commit_status"] == result["cas_commit_status"] == "committed", "Result commit proof drifted")
    require(result["runtime_write_performed"] is True and result["persistent_user_data_touched"] is True and result["high_mesh_created"] is True, "Result durable artifact proof drifted")
    require(result["canonical_sha256"] == canonical_hash(result), "Result canonical hash drifted")
    # Both read-only branches must be accepted by the closed result contract.
    replayed = copy.deepcopy(result)
    replayed.update({"status": "replayed", "idempotency_key": None, "replayed": True, "store_effect": "not-touched", "cas_effect": "not-touched", "atomicity_status": "not-touched", "store_commit_status": "not-touched", "cas_commit_status": "not-touched", "runtime_write_performed": False, "persistent_user_data_touched": False})
    replayed["canonical_sha256"] = canonical_hash(replayed)
    require(is_valid(schema, replayed, registry), "derived replayed Result is schema-invalid")
    found = copy.deepcopy(replayed)
    found.update({"operation": "authoring_mesh_v2_high_artifact_get", "request_kind": "get", "status": "found", "replayed": False})
    found["canonical_sha256"] = canonical_hash(found)
    require(is_valid(schema, found, registry), "derived found Result is schema-invalid")


def mutate(value: dict[str, Any], mutation: str) -> dict[str, Any]:
    candidate = copy.deepcopy(value)
    if mutation == "extra-field": candidate["unexpected"] = True
    elif mutation == "kind": candidate["high_artifact_kind"] = LOW_KIND
    elif mutation == "low-kind-drift": candidate["low_compatibility_artifact_kind"] = "wrong-low-kind"
    elif mutation == "bridge-hash": candidate["high_bridge_sha256"] = "0" * 64
    elif mutation == "readback-binding": candidate["strict_readback"]["source_artifact_sha256"] = "0" * 64
    elif mutation == "canonical-hash": candidate["canonical_sha256"] = "0" * 64
    elif mutation == "operation": candidate["operation"] = "authoring_mesh_v2_high_artifact_get"
    elif mutation == "input-hash": candidate["input_sha256"] = "0" * 64
    elif mutation == "artifact-hash": candidate["high_artifact_sha256"] = "0" * 64
    elif mutation == "write-claim": candidate["runtime_write_performed"] = True
    elif mutation == "status": candidate["status"] = "replayed"
    elif mutation == "promotion": candidate["production_stage_advanced"] = True
    else: raise ContractViolation(f"unknown negative mutation {mutation}")
    return candidate


def check_negative(descriptor: dict[str, Any], main: dict[str, Any], prepare: dict[str, Any], get: dict[str, Any], result: dict[str, Any], schemas: dict[str, dict[str, Any]], registry: dict[str, dict[str, Any]], bridge: dict[str, Any], source_binding: dict[str, Any]) -> None:
    require(descriptor.get("schema_version") == "AuthoringMeshV2HighArtifactNegativeCases@1", "negative inventory version drifted")
    cases = descriptor.get("cases", [])
    expected = {"main-extra-field", "main-kind", "main-low-kind-drift", "main-bridge-hash", "main-readback-binding", "main-canonical-hash", "prepare-extra-field", "prepare-operation", "prepare-bridge-hash", "prepare-input-hash", "get-extra-field", "get-artifact-hash", "get-write-claim", "get-input-hash", "result-extra-field", "result-status", "result-promotion", "result-canonical-hash"}
    require({case.get("id") for case in cases} == expected, "negative case inventory drifted")
    targets = {"main": (main, schemas[MAIN_SCHEMA]), "prepare": (prepare, schemas[PREPARE_SCHEMA]), "get": (get, schemas[GET_SCHEMA]), "result": (result, schemas[RESULT_SCHEMA])}
    for case in cases:
        target = case.get("target")
        require(target in targets, f"negative target drifted: {case.get('id')}")
        base, schema = targets[target]
        candidate = mutate(base, case["mutation"])
        if case["mutation"] == "canonical-hash":
            require(is_valid(schema, candidate, registry) and canonical_hash(candidate) != candidate["canonical_sha256"], f"canonical negative is not a schema-valid hash drift: {case['id']}")
            continue
        if case["mutation"] == "input-hash":
            require(is_valid(schema, candidate, registry) and input_hash(candidate) != candidate["input_sha256"], f"input negative is not a schema-valid hash drift: {case['id']}")
            continue
        if is_valid(schema, candidate, registry):
            try:
                if target == "main": check_main(candidate, bridge, source_binding, schema, registry)
                elif target == "prepare": check_prepare(candidate, main, schema, registry)
                elif target == "get": check_get(candidate, main, schema, registry)
                else: check_result(candidate, main, prepare, schema, registry)
            except ContractViolation:
                continue
            raise ContractViolation(f"schema-valid negative unexpectedly passed: {case['id']}")


def run() -> None:
    manifest = obj(MANIFEST_PATH)
    registry = load_schema_registry(manifest)
    schemas = check_schemas(manifest, registry)
    main, prepare, get, result = obj(MAIN_PATH), obj(PREPARE_PATH), obj(GET_PATH), obj(RESULT_PATH)
    bridge, source_binding = obj(BRIDGE_PATH), obj(SOURCE_BINDING_PATH)
    check_main(main, bridge, source_binding, schemas[MAIN_SCHEMA], registry)
    check_prepare(prepare, main, schemas[PREPARE_SCHEMA], registry)
    check_get(get, main, schemas[GET_SCHEMA], registry)
    check_result(result, main, prepare, schemas[RESULT_SCHEMA], registry)
    check_negative(obj(NEGATIVE_PATH), main, prepare, get, result, schemas, registry, bridge, source_binding)
    print("AuthoringMeshV2 High artifact contracts OK: closed Main/Prepare/Get/Result, V2 GLB/readback, Low adapter identities, source/Bridge/Worker bindings and structural-only gates")


if __name__ == "__main__":
    try:
        run()
    except ContractViolation as exc:
        raise SystemExit(f"AuthoringMeshV2 High artifact contract violation: {exc}") from exc
