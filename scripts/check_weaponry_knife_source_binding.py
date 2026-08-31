#!/usr/bin/env python3
"""Focused closed-contract gate for WPN-KNIFE-SOURCE-BINDING-001.

This gate is deliberately Contracts-only.  It verifies the exact source
lineage envelope that a future Runtime/Store implementation must consume, but
it does not claim that the binding has been produced by a live Runtime or that
any High/visual/human/engine gate has passed.
"""

from __future__ import annotations

import copy
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_ROOT = ROOT / "packages" / "forgecad-contracts"
SCHEMA_ROOT = CONTRACT_ROOT / "schemas"
MANIFEST_PATH = CONTRACT_ROOT / "manifest.json"
FIXTURE_ROOT = CONTRACT_ROOT / "fixtures" / "knife-source-binding"
POSITIVE_PATH = FIXTURE_ROOT / "positive" / "dragonfang-source-binding.json"
INTENT_PATH = (
    CONTRACT_ROOT
    / "fixtures"
    / "weaponry-knife-reference-intent-bundle"
    / "positive"
    / "dragonfang-reference-intent-bundle.json"
)

sys.path.insert(0, str(ROOT / "scripts"))
from check_agentic_contracts import is_valid, load_schema_registry  # noqa: E402

MAIN = "knife-source-binding.schema.json"
PREPARE = "knife-source-binding-prepare-request.schema.json"
GET = "knife-source-binding-get-request.schema.json"
RESULT = "knife-source-binding-result.schema.json"
SCHEMA_VERSION = "KnifeSourceBinding@1"
PREPARE_VERSION = "KnifeSourceBindingPrepareRequest@1"
GET_VERSION = "KnifeSourceBindingGetRequest@1"
RESULT_VERSION = "KnifeSourceBindingResult@1"
POLICY = "intent-brief-reference-quality-to-authoring-mesh-exact@1"
DOWNSTREAM_POLICY = "must-inherit-source-binding-sha256@1"
HASH_FIELDS = (
    "intent_bundle_sha256",
    "intent_bundle_object_sha256",
    "brief_sha256",
    "brief_object_sha256",
    "reference_object_sha256",
    "reference_evidence_sha256",
    "quality_contract_sha256",
    "quality_contract_object_sha256",
    "source_candidate_state_sha256",
    "authoring_mesh_revision_sha256",
    "authoring_mesh_revision_object_sha256",
    "authoring_mesh_identity_sha256",
)
DOWNSTREAM_KEYS = ("curve_modifier_graph", "curve_evaluated_mesh", "high", "render")
NEGATIVE_PATHS = sorted((FIXTURE_ROOT / "negative").glob("*.json"))


def fail(message: str) -> None:
    raise SystemExit(f"Weaponry knife source binding contract violation: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def load(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        fail(f"cannot load {path.relative_to(ROOT)}: {exc}")


def object_at(path: Path) -> dict[str, Any]:
    value = load(path)
    require(isinstance(value, dict), f"{path.relative_to(ROOT)} must be an object")
    return value


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def sha256(value: Any) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def canonical_hash(value: dict[str, Any]) -> str:
    payload = copy.deepcopy(value)
    payload["canonical_sha256"] = ""
    return sha256(payload)


def source_binding_object_hash(value: dict[str, Any]) -> str:
    """Hash the exact CAS payload: the complete Main with canonical filled."""
    return sha256(value)


def input_hash(value: dict[str, Any]) -> str:
    payload = copy.deepcopy(value)
    payload["input_sha256"] = ""
    return sha256(payload)


def walk_objects(node: Any) -> list[dict[str, Any]]:
    if not isinstance(node, dict):
        return []
    found = [node] if node.get("type") == "object" else []
    for key, child in node.items():
        if key in {"properties", "$defs"} and isinstance(child, dict):
            for value in child.values():
                found.extend(walk_objects(value))
        elif key in {"items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"}:
            if isinstance(child, list):
                for value in child:
                    found.extend(walk_objects(value))
            else:
                found.extend(walk_objects(child))
    return found


def property_names(node: Any) -> list[str]:
    if not isinstance(node, dict):
        return []
    names: list[str] = []
    properties = node.get("properties")
    if isinstance(properties, dict):
        names.extend(properties)
        for value in properties.values():
            names.extend(property_names(value))
    for key in ("$defs", "items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"):
        child = node.get(key)
        if isinstance(child, list):
            for value in child:
                names.extend(property_names(value))
        elif isinstance(child, dict):
            names.extend(property_names(child))
    return names


def check_schema_shell(schema: dict[str, Any], filename: str, title: str) -> None:
    require(schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema", f"{filename} draft drifted")
    require(schema.get("$id") == f"https://forgecad.local/contracts/{filename}", f"{filename} id drifted")
    require(schema.get("title") == title, f"{filename} title drifted")
    require(schema.get("type") == "object" and schema.get("additionalProperties") is False, f"{filename} root is open")
    require(schema.get("properties", {}).get("schema_version", {}).get("const") == title, f"{filename} version drifted")
    require(set(schema.get("required", [])) == set(schema.get("properties", {})), f"{filename} required/properties are not exact")
    for object_schema in walk_objects(schema):
        require(object_schema.get("additionalProperties") is False, f"{filename} contains an open object")
    forbidden = {"path", "url", "uri", "raw", "raw_bytes", "bytes", "secret", "token", "password", "api_key", "prompt", "script", "shell", "environment", "executor"}
    require(not ({name.lower() for name in property_names(schema)} & forbidden), f"{filename} exposes a forbidden property")


def check_schemas(manifest: dict[str, Any], registry: dict[str, dict[str, Any]]) -> dict[str, dict[str, Any]]:
    expected = {
        MAIN: SCHEMA_VERSION,
        PREPARE: PREPARE_VERSION,
        GET: GET_VERSION,
        RESULT: RESULT_VERSION,
    }
    declared = set(manifest.get("schemas", []))
    require(set(expected) <= declared, "manifest does not register every source binding schema")
    schemas: dict[str, dict[str, Any]] = {}
    for filename, title in expected.items():
        schema = object_at(SCHEMA_ROOT / filename)
        check_schema_shell(schema, filename, title)
        schemas[filename] = schema
    require(registry.get(schemas[MAIN]["$id"]) == schemas[MAIN], "Main schema is not registry-bound")
    require(
        schemas[MAIN]["$defs"]["identifier"]["pattern"] == "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$",
        "Runtime opaque identifier still permits colon",
    )
    require(
        schemas[MAIN]["properties"]["canonicalization_policy"]["const"]
        == "canonical-json-sha256-excluding-canonical-sha256@1",
        "Main hash policy is not the non-self-referential canonical policy",
    )
    downstream = schemas[MAIN]["$defs"]["downstream_binding_requirements"]
    require(set(downstream["required"]) == set(DOWNSTREAM_KEYS), "downstream binding requirement keys drifted")
    for key in DOWNSTREAM_KEYS:
        require(downstream["properties"][key].get("const") == DOWNSTREAM_POLICY, f"downstream {key} policy drifted")
    return schemas


def expected_candidate_state_hash(binding: dict[str, Any]) -> str:
    return sha256({
        "schema_version": "KnifeSourceCandidateState@1",
        "project_id": binding["project_id"],
        "source_candidate_id": binding["source_candidate_id"],
    })


def expected_revision_hash(binding: dict[str, Any]) -> str:
    return sha256({
        "schema_version": "AuthoringMeshRevision@1",
        "authoring_mesh_id": binding["authoring_mesh_id"],
        "authoring_mesh_lineage_id": binding["authoring_mesh_lineage_id"],
        "authoring_mesh_revision_id": binding["authoring_mesh_revision_id"],
        "authoring_mesh_revision_index": binding["authoring_mesh_revision_index"],
    })


def expected_revision_object_hash(binding: dict[str, Any]) -> str:
    return sha256({
        "schema_version": "AuthoringMeshRevisionObject@1",
        "authoring_mesh_id": binding["authoring_mesh_id"],
        "authoring_mesh_lineage_id": binding["authoring_mesh_lineage_id"],
        "authoring_mesh_revision_id": binding["authoring_mesh_revision_id"],
        "authoring_mesh_revision_index": binding["authoring_mesh_revision_index"],
        "authoring_mesh_revision_sha256": binding["authoring_mesh_revision_sha256"],
    })


def expected_identity_hash(binding: dict[str, Any]) -> str:
    return sha256({
        "schema_version": "AuthoringMeshIdentity@1",
        "authoring_mesh_id": binding["authoring_mesh_id"],
        "authoring_mesh_lineage_id": binding["authoring_mesh_lineage_id"],
        "authoring_mesh_revision_id": binding["authoring_mesh_revision_id"],
        "authoring_mesh_revision_index": binding["authoring_mesh_revision_index"],
    })


def check_main(binding: dict[str, Any], schema: dict[str, Any], registry: dict[str, dict[str, Any]]) -> str:
    require(is_valid(schema, binding, registry), "positive Main fixture is schema-invalid")
    intent = object_at(INTENT_PATH)
    brief = intent["brief_binding"]
    reference = intent["reference_binding"]
    quality = intent["quality_contract"]
    require(binding["project_id"] == intent["project_id"], "source binding project is not intent-bound")
    require(binding["intent_bundle_id"] == intent["intent_bundle_id"], "intent bundle id drifted")
    require(binding["intent_bundle_sha256"] == intent["canonical_sha256"], "intent semantic hash drifted")
    require(binding["intent_bundle_object_sha256"] == sha256(intent), "intent object hash drifted")
    require(binding["brief_id"] == brief["brief_id"], "brief id drifted")
    require(binding["brief_sha256"] == brief["brief_sha256"], "brief semantic hash drifted")
    require(binding["brief_object_sha256"] == brief["brief_object_sha256"], "brief object hash drifted")
    require(binding["reference_id"] == reference["reference_id"], "reference id drifted")
    require(binding["reference_object_sha256"] == reference["reference_object_sha256"], "reference object hash drifted")
    require(binding["reference_evidence_sha256"] == reference["reference_evidence_sha256"], "reference evidence hash drifted")
    require(binding["quality_contract_id"] == quality["contract_id"], "quality contract id drifted")
    require(binding["quality_contract_sha256"] == quality["canonical_sha256"], "quality semantic hash drifted")
    require(binding["quality_contract_object_sha256"] == sha256(quality), "quality object hash drifted")
    require(binding["binding_status"] == "runtime-bound" and binding["authoring_eligibility"] == "ELIGIBLE", "source eligibility is not explicit")
    require(binding["source_candidate_state_sha256"] == expected_candidate_state_hash(binding), "candidate state hash does not bind candidate identity")
    require(binding["authoring_mesh_revision_sha256"] == expected_revision_hash(binding), "AuthoringMesh revision hash does not bind revision identity")
    require(binding["authoring_mesh_revision_object_sha256"] == expected_revision_object_hash(binding), "AuthoringMesh revision object hash drifted")
    require(binding["authoring_mesh_identity_sha256"] == expected_identity_hash(binding), "AuthoringMesh identity hash drifted")
    require(set(binding["downstream_binding_requirements"]) == set(DOWNSTREAM_KEYS), "downstream requirement set is incomplete")
    require(all(value == DOWNSTREAM_POLICY for value in binding["downstream_binding_requirements"].values()), "downstream binding policy weakened")
    for field in ("high_mesh_created", "high_stage_unlocked", "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed"):
        require(binding[field] is False, f"source binding promoted {field}")
    require(binding["quality_status"] == "source_binding_only", "source binding quality state drifted")
    require(binding["visual_status"] == "NOT_RUN" and binding["human_status"] == "NOT_RUN" and binding["engine_status"] == "NOT_RUN", "source binding promoted a downstream gate")
    require(binding["canonical_sha256"] == canonical_hash(binding), "Main semantic canonical hash is stale")
    require(source_binding_object_hash(binding) == sha256(binding), "CAS object preimage is not the complete Main with canonical filled")
    return sha256(binding)


def prepare_fixture(binding: dict[str, Any]) -> dict[str, Any]:
    value = {
        "schema_version": PREPARE_VERSION,
        "operation": "knife_source_binding_prepare",
        "project_id": binding["project_id"],
        "source_binding": binding,
        "idempotency_key": "dragonfang-source-binding-prepare-001",
        "max_response_bytes": 1048576,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    }
    value["input_sha256"] = input_hash(value)
    return value


def get_fixture(binding: dict[str, Any], object_hash_value: str) -> dict[str, Any]:
    fields = (
        "intent_bundle_id", "intent_bundle_sha256", "intent_bundle_object_sha256",
        "brief_id", "brief_sha256", "brief_object_sha256", "reference_id",
        "reference_object_sha256", "reference_evidence_sha256", "quality_contract_id",
        "quality_contract_sha256", "quality_contract_object_sha256", "source_candidate_id",
        "source_candidate_state_sha256", "authoring_mesh_id", "authoring_mesh_lineage_id",
        "authoring_mesh_revision_id", "authoring_mesh_revision_index",
        "authoring_mesh_revision_sha256", "authoring_mesh_revision_object_sha256",
        "authoring_mesh_identity_sha256",
    )
    value = {
        "schema_version": GET_VERSION,
        "operation": "knife_source_binding_get",
        "project_id": binding["project_id"],
        "source_binding_id": binding["source_binding_id"],
        "source_binding_sha256": binding["canonical_sha256"],
        "source_binding_object_sha256": object_hash_value,
    }
    value.update({field: binding[field] for field in fields})
    value.update({
        "max_response_bytes": 1048576,
        "runtime_write_performed": False,
        "persistent_user_data_touched": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    })
    value["input_sha256"] = input_hash(value)
    return value


def result_fixture(binding: dict[str, Any], object_hash_value: str, request_kind: str, status: str) -> dict[str, Any]:
    prepare = request_kind == "prepare"
    value = {
        "schema_version": RESULT_VERSION,
        "operation": "knife_source_binding_prepare" if prepare else "knife_source_binding_get",
        "request_kind": request_kind,
        "status": status,
        "project_id": binding["project_id"],
        "source_binding_id": binding["source_binding_id"],
        "source_binding_sha256": binding["canonical_sha256"],
        "source_binding_object_sha256": object_hash_value,
        "binding_status": binding["binding_status"],
        "authoring_eligibility": binding["authoring_eligibility"],
        "source_binding": binding,
        "idempotency_key": "dragonfang-source-binding-prepare-001" if prepare else None,
        "replayed": status == "replayed",
        "store_effect": "inserted" if status == "prepared" else "not-touched",
        "cas_effect": "inserted" if status == "prepared" else "not-touched",
        "atomicity_status": "committed" if status == "prepared" else "not-touched",
        "store_commit_status": "committed" if status == "prepared" else "not-touched",
        "cas_commit_status": "committed" if status == "prepared" else "not-touched",
        "runtime_write_performed": status == "prepared",
        "persistent_user_data_touched": status == "prepared",
        "partial_result_exposed": False,
        "high_mesh_created": False,
        "high_stage_unlocked": False,
        "production_stage_advanced": False,
        "candidate_confirmed": False,
        "version_created": False,
        "export_performed": False,
        "quality_status": "source_binding_only",
        "visual_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
        "canonical_sha256": "",
    }
    value.update({field: binding[field] for field in (
        "intent_bundle_id", "intent_bundle_sha256", "intent_bundle_object_sha256",
        "brief_id", "brief_sha256", "brief_object_sha256", "reference_id",
        "reference_object_sha256", "reference_evidence_sha256", "quality_contract_id",
        "quality_contract_sha256", "quality_contract_object_sha256", "source_candidate_id",
        "source_candidate_state_sha256", "authoring_mesh_id", "authoring_mesh_lineage_id",
        "authoring_mesh_revision_id", "authoring_mesh_revision_index",
        "authoring_mesh_revision_sha256", "authoring_mesh_revision_object_sha256",
        "authoring_mesh_identity_sha256", "downstream_binding_requirements",
    )})
    value["canonical_sha256"] = canonical_hash(value)
    return value


def check_transports(schemas: dict[str, dict[str, Any]], binding: dict[str, Any], registry: dict[str, dict[str, Any]], object_hash_value: str) -> None:
    prepare = prepare_fixture(binding)
    get = get_fixture(binding, object_hash_value)
    require(is_valid(schemas[PREPARE], prepare, registry), "prepare fixture is schema-invalid")
    require(is_valid(schemas[GET], get, registry), "get fixture is schema-invalid")
    require(prepare["input_sha256"] == input_hash(prepare), "prepare input hash is stale")
    require(get["input_sha256"] == input_hash(get), "get input hash is stale")
    require(prepare["project_id"] == binding["project_id"], "prepare project is not binding project")
    for request_kind, status in (("prepare", "prepared"), ("prepare", "replayed"), ("get", "found")):
        result = result_fixture(binding, object_hash_value, request_kind, status)
        require(is_valid(schemas[RESULT], result, registry), f"{request_kind}/{status} result is schema-invalid")
        require(result["source_binding_sha256"] == binding["canonical_sha256"], "result semantic binding hash drifted")
        require(result["source_binding_object_sha256"] == object_hash_value, "result object hash drifted")
        require(result["canonical_sha256"] == canonical_hash(result), f"{request_kind}/{status} result hash is stale")


def check_negative_fixtures(schema: dict[str, Any], registry: dict[str, dict[str, Any]], binding: dict[str, Any]) -> None:
    require(NEGATIVE_PATHS, "source binding negative fixture directory is empty")
    for path in NEGATIVE_PATHS:
        value = object_at(path)
        if path.name in {"intent-hash-mismatch.json", "mesh-identity-mismatch.json"}:
            require(is_valid(schema, value, registry), f"semantic negative fixture is not schema-valid: {path.name}")
            try:
                check_main(value, schema, registry)
            except SystemExit:
                # These fixtures deliberately remain JSON-schema-valid while
                # violating a cross-contract binding.  The semantic checker
                # must reject them; schema validation alone is insufficient.
                pass
            else:
                fail(f"semantic negative fixture unexpectedly passed: {path.name}")
        else:
            require(not is_valid(schema, value, registry), f"negative fixture unexpectedly passed: {path.name}")
    for path in (FIXTURE_ROOT / "negative" / "intent-hash-mismatch.json", FIXTURE_ROOT / "negative" / "mesh-identity-mismatch.json"):
        value = object_at(path)
        if path.name == "intent-hash-mismatch.json":
            require(value["intent_bundle_sha256"] != binding["intent_bundle_sha256"], "intent semantic negative was not mutated")
        else:
            require(value["authoring_mesh_identity_sha256"] != expected_identity_hash(value), "mesh identity negative did not break identity binding")


def run_checks() -> None:
    manifest = object_at(MANIFEST_PATH)
    registry = load_schema_registry(manifest)
    schemas = check_schemas(manifest, registry)
    binding = object_at(POSITIVE_PATH)
    object_hash_value = check_main(binding, schemas[MAIN], registry)
    check_transports(schemas, binding, registry, object_hash_value)
    check_negative_fixtures(schemas[MAIN], registry, binding)
    print(
        "Weaponry knife source binding contracts OK: "
        "Main/prepare/get/result + exact intent/brief/reference/quality/AuthoringMesh hashes + negatives"
    )


if __name__ == "__main__":
    run_checks()
