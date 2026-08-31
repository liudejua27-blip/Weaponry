#!/usr/bin/env python3
"""Focused closed-contract gate for WPN-AUTH-MATERIALIZE-001.

This checker covers only the public prepare request/result envelope for the
Runtime-only AuthoringMeshV2 candidate materializer.  Candidate and
ArtifactReadback are reused from their existing contracts.  There is
intentionally no materializer Main record or Get contract: the operation does
not own a second durable record; its durable output is the existing candidate,
geometry evidence and Job projection.

The gate proves schema closure, identity/hash bindings and negative cases.  It
does not claim visual, High, human, engine, approval, version or export
quality, and it does not pretend that a fixture is live Runtime evidence.
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
MANIFEST_PATH = CONTRACT_ROOT / "manifest.json"
FIXTURE_ROOT = (
    CONTRACT_ROOT / "fixtures" / "authoring-mesh-v2-candidate-materializer"
)
REQUEST_PATH = FIXTURE_ROOT / "positive" / "dragonfang-materialize-request.json"
RESULT_PATH = FIXTURE_ROOT / "positive" / "dragonfang-materialize-result.json"
NEGATIVE_PATH = FIXTURE_ROOT / "negative" / "cases.json"

sys.path.insert(0, str(ROOT / "scripts"))
from check_agentic_contracts import is_valid, load_schema_registry  # noqa: E402

REQUEST_SCHEMA = "authoring-mesh-v2-candidate-materialize-prepare-request.schema.json"
RESULT_SCHEMA = "authoring-mesh-v2-candidate-materialize-result.schema.json"
REQUEST_TITLE = "AuthoringMeshV2CandidateMaterializeRequest@1"
RESULT_TITLE = "AuthoringMeshV2CandidateMaterializeResult@1"

REQUEST_FIELDS = (
    "schema_version",
    "operation",
    "project_id",
    "mesh_id",
    "lineage_id",
    "revision_id",
    "revision_index",
    "revision_sha256",
    "revision_object_sha256",
    "source_binding_id",
    "source_binding_sha256",
    "source_binding_object_sha256",
    "base_version_id",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
)

RESULT_FIELDS = (
    "schema_version",
    "operation",
    "request_kind",
    "status",
    "project_id",
    "mesh_id",
    "lineage_id",
    "revision_id",
    "revision_index",
    "revision_sha256",
    "revision_object_sha256",
    "source_binding_id",
    "source_binding_sha256",
    "source_binding_object_sha256",
    "representation_plan_sha256",
    "materialization_mode",
    "source_candidate_id",
    "source_candidate_state_sha256",
    "source_artifact_sha256",
    "source_artifact_readback_sha256",
    "source_program_sha256",
    "source_program_object_sha256",
    "source_node_id",
    "source_part_id",
    "source_material_zone_id",
    "source_solid",
    "source_part_output_sha256",
    "replacement_node_id",
    "preserved_part_ids",
    "geometry_idempotency_key",
    "candidate",
    "artifact",
    "job",
    "replayed",
    "restart_hash_verified",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "quality_status",
    "visual_status",
    "human_status",
    "engine_status",
    "writer_policy",
    "canonicalization_policy",
    "request_input_sha256",
    "idempotency_key",
    "canonical_sha256",
)

IDENTIFIER = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
POLICY = "forgecad-runtime-only-state-writer@1"
REQUEST_CANONICALIZATION_POLICY = "canonical-json-sha256-excluding-input-sha256@1"
MAX_RESPONSE_BYTES = 1_048_576
OPERATION = "authoring_mesh_v2_candidate_materialize"


def fail(message: str) -> None:
    raise SystemExit(f"AuthoringMeshV2 candidate materializer contract violation: {message}")


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
    preimage = copy.deepcopy(value)
    preimage["canonical_sha256"] = ""
    return sha256(preimage)


def input_hash(value: dict[str, Any]) -> str:
    preimage = copy.deepcopy(value)
    preimage["input_sha256"] = ""
    return sha256(preimage)


def require_exact_fields(
    value: Any, expected: tuple[str, ...], label: str
) -> None:
    require(isinstance(value, dict), f"{label} must be an object")
    require(set(value) == set(expected), f"{label} fields differ from the closed envelope")


def walk_schema(node: Any) -> list[dict[str, Any]]:
    if not isinstance(node, dict):
        return []
    found: list[dict[str, Any]] = []
    if node.get("type") == "object":
        found.append(node)
    for key, child in node.items():
        if key in {"properties", "$defs", "definitions"} and isinstance(child, dict):
            for value in child.values():
                found.extend(walk_schema(value))
        elif key in {"items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"}:
            if isinstance(child, list):
                for value in child:
                    found.extend(walk_schema(value))
            else:
                found.extend(walk_schema(child))
    return found


def walk_property_names(node: Any) -> list[str]:
    if not isinstance(node, dict):
        return []
    names: list[str] = []
    properties = node.get("properties")
    if isinstance(properties, dict):
        names.extend(properties)
        for value in properties.values():
            names.extend(walk_property_names(value))
    for key in {"$defs", "definitions", "items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"}:
        child = node.get(key)
        if isinstance(child, list):
            for value in child:
                names.extend(walk_property_names(value))
        elif isinstance(child, dict):
            names.extend(walk_property_names(child))
    return names


def check_schema_shell(schema: dict[str, Any], filename: str, title: str) -> None:
    require(
        schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema",
        f"{filename} draft drifted",
    )
    require(
        schema.get("$id") == f"https://forgecad.local/contracts/{filename}",
        f"{filename} id drifted",
    )
    require(schema.get("title") == title, f"{filename} title drifted")
    require(
        schema.get("type") == "object" and schema.get("additionalProperties") is False,
        f"{filename} root is not closed",
    )
    properties = schema.get("properties", {})
    require(
        set(schema.get("required", [])) == set(properties),
        f"{filename} required/properties drifted",
    )
    require(
        properties.get("schema_version", {}).get("const") == title,
        f"{filename} version drifted",
    )
    for object_schema in walk_schema(schema):
        require(
            object_schema.get("additionalProperties") is False,
            f"{filename} contains an open local object",
        )
    forbidden = {
        "path",
        "url",
        "uri",
        "raw",
        "raw_bytes",
        "bytes",
        "secret",
        "token",
        "password",
        "api_key",
        "prompt",
        "script",
        "shell",
        "environment",
        "executor",
    }
    require(
        not ({name.lower() for name in walk_property_names(schema)} & forbidden),
        f"{filename} exposes a forbidden property",
    )


def check_schemas(
    manifest: dict[str, Any], registry: dict[str, dict[str, Any]]
) -> tuple[dict[str, Any], dict[str, Any]]:
    require(REQUEST_SCHEMA in manifest.get("schemas", []), f"manifest omits {REQUEST_SCHEMA}")
    require(RESULT_SCHEMA in manifest.get("schemas", []), f"manifest omits {RESULT_SCHEMA}")
    request_schema = object_at(SCHEMA_ROOT / REQUEST_SCHEMA)
    result_schema = object_at(SCHEMA_ROOT / RESULT_SCHEMA)
    check_schema_shell(request_schema, REQUEST_SCHEMA, REQUEST_TITLE)
    check_schema_shell(result_schema, RESULT_SCHEMA, RESULT_TITLE)
    require(set(request_schema["required"]) == set(REQUEST_FIELDS), "request fields drifted")
    require(set(result_schema["required"]) == set(RESULT_FIELDS), "result fields drifted")
    require(
        request_schema["properties"]["source_binding_id"] == {"$ref": "#/$defs/nullable_identifier"},
        "request source binding ID is not nullable",
    )
    require(
        result_schema["properties"]["artifact"].get("$ref")
        == "https://forgecad.local/contracts/artifact-readback-v2.schema.json",
        "result does not reuse ArtifactReadback@2",
    )
    require(
        result_schema["properties"]["candidate"].get("$ref")
        == "https://forgecad.local/contracts/candidate.schema.json",
        "result does not reuse Candidate@1",
    )
    for field in [
        "source_candidate_id",
        "source_node_id",
        "source_part_id",
        "source_material_zone_id",
    ]:
        require(
            result_schema["properties"][field].get("$ref")
            == "#/$defs/nullable_identifier",
            f"result {field} is not nullable Runtime identity",
        )
    for field in [
        "source_candidate_state_sha256",
        "source_artifact_sha256",
        "source_artifact_readback_sha256",
        "source_program_sha256",
        "source_program_object_sha256",
        "source_part_output_sha256",
    ]:
        require(
            result_schema["properties"][field].get("$ref")
            == "#/$defs/nullable_sha256",
            f"result {field} is not nullable SHA-256",
        )
    require(
        result_schema["properties"]["replacement_node_id"].get("$ref")
        == "#/$defs/identifier",
        "result replacement node identity is not required",
    )
    require(registry.get(request_schema["$id"]) == request_schema, "request schema is not registry-bound")
    require(registry.get(result_schema["$id"]) == result_schema, "result schema is not registry-bound")
    return request_schema, result_schema


def check_source_binding_triple(value: dict[str, Any], label: str) -> None:
    fields = [value.get("source_binding_id"), value.get("source_binding_sha256"), value.get("source_binding_object_sha256")]
    require(all(item is None for item in fields) or all(item is not None for item in fields), f"{label} source binding triple is partial")


def check_positive(
    request_schema: dict[str, Any],
    result_schema: dict[str, Any],
    registry: dict[str, dict[str, Any]],
) -> None:
    request = object_at(REQUEST_PATH)
    result = object_at(RESULT_PATH)
    require_exact_fields(request, REQUEST_FIELDS, "positive request")
    require_exact_fields(result, RESULT_FIELDS, "positive result")
    require(is_valid(request_schema, request, registry), "positive request is schema-invalid")
    require(is_valid(result_schema, result, registry), "positive result is schema-invalid")
    require(request["schema_version"] == REQUEST_TITLE, "request title drifted")
    require(result["schema_version"] == RESULT_TITLE, "result title drifted")
    require(request["operation"] == OPERATION, "request operation drifted")
    require(result["operation"] == OPERATION, "result operation drifted")
    require(result["request_kind"] == "prepare", "result request kind drifted")
    require(result["status"] == "prepared", "positive result status drifted")
    require(
        result["materialization_mode"] == "standalone_revision",
        "positive result materialization mode drifted",
    )
    require(
        all(
            result[field] is None
            for field in (
                "source_candidate_id",
                "source_candidate_state_sha256",
                "source_artifact_sha256",
                "source_artifact_readback_sha256",
                "source_program_sha256",
                "source_program_object_sha256",
                "source_node_id",
                "source_part_id",
                "source_material_zone_id",
                "source_solid",
                "source_part_output_sha256",
            )
        ),
        "standalone result unexpectedly contains source proof",
    )
    require(
        isinstance(result["replacement_node_id"], str)
        and IDENTIFIER.fullmatch(result["replacement_node_id"]),
        "replacement node identity drifted",
    )
    require(
        result["preserved_part_ids"] == [],
        "standalone result has preserved source parts",
    )
    source_result = copy.deepcopy(result)
    source_result.update(
        {
            "source_binding_id": "source-binding-001",
            "source_binding_sha256": "a" * 64,
            "source_binding_object_sha256": "b" * 64,
            "materialization_mode": "source_binding_part_replacement",
            "source_candidate_id": "source-candidate-001",
            "source_candidate_state_sha256": "c" * 64,
            "source_artifact_sha256": "d" * 64,
            "source_artifact_readback_sha256": "e" * 64,
            "source_program_sha256": "f" * 64,
            "source_program_object_sha256": "0" * 64,
            "source_node_id": "blade-source-node",
            "source_part_id": "blade-part",
            "source_material_zone_id": "blade-metal",
            "source_solid": True,
            "source_part_output_sha256": "1" * 64,
            "preserved_part_ids": ["guard-part"],
        }
    )
    source_result["canonical_sha256"] = canonical_hash(source_result)
    require(
        is_valid(result_schema, source_result, registry),
        "source-bound result mode is schema-invalid",
    )
    require(input_hash(request) == request["input_sha256"], "request input hash drifted")
    require(result["request_input_sha256"] == request["input_sha256"], "result is not request-bound")
    check_source_binding_triple(request, "positive request")
    check_source_binding_triple(result, "positive result")
    require(
        all(result[field] == request[field] for field in (
            "project_id",
            "mesh_id",
            "lineage_id",
            "revision_id",
            "revision_index",
            "revision_sha256",
            "revision_object_sha256",
            "source_binding_id",
            "source_binding_sha256",
            "source_binding_object_sha256",
        )),
        "result revision/source identity is not request-bound",
    )
    require(
        result["geometry_idempotency_key"]
        == "authoring-v2-"
        + hashlib.sha256(request["idempotency_key"].encode("utf-8")).hexdigest()[:48],
        "geometry idempotency derivation drifted",
    )
    candidate = result["candidate"]
    artifact = result["artifact"]
    job = result["job"]
    require(candidate["project_id"] == result["project_id"], "candidate project binding drifted")
    require(candidate["state"] == "reviewable", "candidate is not reviewable")
    require(candidate["quality_hard_gate_passed"] is True, "candidate structural gate drifted")
    require(artifact["candidate_id"] == candidate["candidate_id"], "artifact candidate binding drifted")
    require(artifact["object_sha256"] == candidate["prepared_object_sha256"], "artifact object binding drifted")
    require(SHA256.fullmatch(artifact["program_sha256"]), "derived GeometryProgram hash is missing")
    require(job["project_id"] == result["project_id"], "Job project binding drifted")
    require(result["replayed"] is False, "positive fixture is not first prepare")
    require(result["restart_hash_verified"] is False, "prepare fixture claims restart verification")
    require(result["runtime_write_performed"] is True, "positive prepare is not Runtime-written")
    require(result["persistent_user_data_touched"] is True, "positive prepare is not persistent")
    require(result["quality_status"] == "structural_only", "quality status drifted")
    require(
        result["visual_status"] == result["human_status"] == result["engine_status"] == "NOT_RUN",
        "positive fixture promoted an unrun downstream gate",
    )
    require(result["canonical_sha256"] == canonical_hash(result), "result canonical hash drifted")


def check_negative(
    request_schema: dict[str, Any],
    result_schema: dict[str, Any],
    registry: dict[str, dict[str, Any]],
) -> None:
    descriptor = object_at(NEGATIVE_PATH)
    require(descriptor.get("schema_version") == "AuthoringMeshV2CandidateMaterializeNegativeFixtures@1", "negative fixture version drifted")
    cases = descriptor.get("cases")
    require(isinstance(cases, list) and cases, "negative fixture cases are empty")
    request = object_at(REQUEST_PATH)
    result = object_at(RESULT_PATH)
    expected_ids = {
        "request-extra-field",
        "request-operation-drift",
        "request-input-hash-drift",
        "request-source-binding-partial",
        "result-extra-field",
        "result-operation-drift",
        "result-canonical-hash-drift",
        "result-source-binding-partial",
        "result-source-proof-partial",
        "result-materialization-mode-drift",
        "result-status-drift",
        "result-restart-claim",
        "result-visual-promotion",
    }
    require({case.get("id") for case in cases} == expected_ids, "negative case inventory drifted")
    for case in cases:
        require(isinstance(case, dict), "negative case must be an object")
        value = copy.deepcopy(request if case["target"] == "request" else result)
        mutation = case.get("mutation")
        if mutation == "extra_field":
            value["unexpected"] = True
        elif mutation == "operation_drift":
            value["operation"] = "authoring_mesh_v2_candidate_materialize_legacy"
        elif mutation == "status_drift":
            value["status"] = "replayed"
        elif mutation == "input_hash_drift":
            value["input_sha256"] = "0" * 64
            require(is_valid(request_schema, value, registry), "input-hash negative left schema-validity unexpectedly")
            require(input_hash(value) != value["input_sha256"], "input-hash negative did not drift")
            continue
        elif mutation == "source_binding_partial":
            value["source_binding_id"] = "source-binding-1"
        elif mutation == "source_proof_partial":
            value["source_candidate_id"] = "source-candidate-1"
        elif mutation == "materialization_mode_drift":
            value["materialization_mode"] = "source_binding_part_replacement"
        elif mutation == "canonical_hash_drift":
            value["canonical_sha256"] = "0" * 64
            require(is_valid(result_schema, value, registry), "canonical negative left schema-validity unexpectedly")
            require(canonical_hash(value) != value["canonical_sha256"], "canonical negative did not drift")
            continue
        elif mutation == "restart_claim":
            value["restart_hash_verified"] = True
        elif mutation == "visual_promotion":
            value["visual_status"] = "PASS"
        else:
            fail(f"unknown negative mutation: {mutation}")
        schema = request_schema if case["target"] == "request" else result_schema
        require(not is_valid(schema, value, registry), f"negative fixture unexpectedly passed: {case['id']}")


def run_checks() -> None:
    manifest = object_at(MANIFEST_PATH)
    registry = load_schema_registry(manifest)
    request_schema, result_schema = check_schemas(manifest, registry)
    check_positive(request_schema, result_schema, registry)
    check_negative(request_schema, result_schema, registry)
    print(
        "AuthoringMeshV2 candidate materializer contracts OK: "
        "closed Prepare/Result, Candidate/Artifact reuse, source triple, replay/restart gates and negatives"
    )


if __name__ == "__main__":
    run_checks()
