#!/usr/bin/env python3
"""Closed contract gate for the Runtime source-to-AuthoringMeshV2 bridge.

This is a Contracts-only gate.  It proves that the package contract can carry
the current Runtime envelope and that hash/policy/promotion mutations fail
closed.  It does not claim High, visual, human, engine, approval, version, or
export quality.
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
FIXTURE_ROOT = (
    CONTRACT_ROOT / "fixtures" / "production-weapon-authoring-mesh-v2-source"
)
REQUEST_PATH = FIXTURE_ROOT / "positive" / "dragonfang-source-prepare-request.json"
RESULT_PATH = FIXTURE_ROOT / "positive" / "dragonfang-source-prepare-result.json"

sys.path.insert(0, str(ROOT / "scripts"))
from check_agentic_contracts import is_valid, load_schema_registry  # noqa: E402

REQUEST_SCHEMA = "production-weapon-authoring-mesh-v2-source-prepare-request.schema.json"
RESULT_SCHEMA = "production-weapon-authoring-mesh-v2-source-prepare-result.schema.json"
REQUEST_TITLE = "ProductionWeaponAuthoringMeshV2SourcePrepareRequest@1"
RESULT_TITLE = "ProductionWeaponAuthoringMeshV2SourcePrepareResult@1"
REQUEST_FIELDS = (
    "schema_version",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "geometry_program_sha256",
    "artifact_sha256",
    "artifact_readback_sha256",
    "part_id",
    "source_node_id",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
)
RESULT_FIELDS = (
    "schema_version",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "geometry_program_sha256",
    "artifact_sha256",
    "artifact_readback_sha256",
    "part_id",
    "source_node_id",
    "source_operator_id",
    "source_parameters_sha256",
    "source_position_m",
    "source_rotation_rad",
    "material_zone_id",
    "solid",
    "source_binding_sha256",
    "mesh_id",
    "lineage_id",
    "revision_id",
    "revision_sha256",
    "revision_object_sha256",
    "authoring_mesh_v2",
    "request_input_sha256",
    "idempotency_key",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "stage_advanced",
    "candidate_confirmed",
    "version_created",
    "export_performed",
    "quality_status",
    "limitations",
    "canonical_sha256",
)
NESTED_FIELDS = (
    "schema_version",
    "project_id",
    "mesh_id",
    "lineage_id",
    "revision_id",
    "revision_index",
    "parent_revision_ids",
    "revision_sha256",
    "revision_object_sha256",
    "operation",
    "revision",
    "durable_record",
    "request_input_sha256",
    "idempotency_key",
    "replayed",
    "restart_hash_verified",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "stage_advanced",
    "candidate_confirmed",
    "version_created",
    "export_performed",
    "quality_status",
    "limitations",
    "canonicalization_policy",
    "canonical_sha256",
)
LIMITATIONS = [
    "REAL_CANDIDATE_SOURCE_BOUND",
    "PROFILE_EXTRUDE_OR_PRIMITIVE_SOURCE",
    "NO_ART_EDIT_APPLIED",
    "NO_STAGE_ADVANCEMENT",
    "NO_VISUAL_QUALITY_CLAIM",
]
NESTED_LIMITATIONS = [
    "RUNTIME_SOLE_WRITER",
    "NO_STAGE_ADVANCEMENT",
    "NO_CANDIDATE_CONFIRM",
    "NO_VERSION_CREATED",
    "NO_EXPORT",
    "STRUCTURAL_ONLY_NOT_COMMERCIAL_QUALITY",
]
POLICY = "forgecad-runtime-only-state-writer@1"
CANONICALIZATION_POLICY = "canonical-json-sha256-excluding-canonical-sha256@1"


def fail(message: str) -> None:
    raise SystemExit(
        f"Production weapon AuthoringMeshV2 source contract violation: {message}"
    )


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def load(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        fail(f"cannot load {path.relative_to(ROOT)}: {exc}")


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


def input_hash(value: dict[str, Any]) -> str:
    payload = copy.deepcopy(value)
    payload["input_sha256"] = ""
    return sha256(payload)


def require_exact_fields(value: Any, expected: tuple[str, ...], label: str) -> None:
    require(isinstance(value, dict), f"{label} must be an object")
    require(set(value) == set(expected), f"{label} fields differ from the Runtime envelope")


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
    require(set(schema.get("required", [])) == set(properties), f"{filename} required/properties drifted")
    require(properties.get("schema_version", {}).get("const") == title, f"{filename} version drifted")
    forbidden = {
        "path", "url", "uri", "raw", "raw_bytes", "bytes", "secret", "token",
        "password", "api_key", "prompt", "script", "shell", "environment", "executor",
    }
    require(not ({name.lower() for name in properties} & forbidden), f"{filename} exposes a forbidden field")


def load_contracts() -> tuple[dict[str, Any], dict[str, Any], dict[str, dict[str, Any]]]:
    manifest = load(MANIFEST_PATH)
    require(isinstance(manifest, dict), "manifest must be an object")
    for filename in (REQUEST_SCHEMA, RESULT_SCHEMA):
        require(filename in manifest.get("schemas", []), f"manifest omits {filename}")
    request_schema = load(SCHEMA_ROOT / REQUEST_SCHEMA)
    result_schema = load(SCHEMA_ROOT / RESULT_SCHEMA)
    require(isinstance(request_schema, dict), "request schema must be an object")
    require(isinstance(result_schema, dict), "result schema must be an object")
    check_schema_shell(request_schema, REQUEST_SCHEMA, REQUEST_TITLE)
    check_schema_shell(result_schema, RESULT_SCHEMA, RESULT_TITLE)
    require(set(request_schema["required"]) == set(REQUEST_FIELDS), "request fields drifted")
    require(set(result_schema["required"]) == set(RESULT_FIELDS), "result fields drifted")
    nested = result_schema["$defs"]["authoring_mesh_v2_result"]
    require(nested.get("additionalProperties") is False, "nested durable result is open")
    require(set(nested["required"]) == set(NESTED_FIELDS), "nested durable result fields drifted")
    registry = load_schema_registry(manifest)
    return request_schema, result_schema, registry


def check_positive(request_schema: dict[str, Any], result_schema: dict[str, Any], registry: dict[str, dict[str, Any]]) -> None:
    request = load(REQUEST_PATH)
    result = load(RESULT_PATH)
    require_exact_fields(request, REQUEST_FIELDS, "positive request")
    require_exact_fields(result, RESULT_FIELDS, "positive result")
    require(is_valid(request_schema, request, registry), "positive request is not schema-valid")
    require(is_valid(result_schema, result, registry), "positive result is not schema-valid")
    require(request["schema_version"] == REQUEST_TITLE, "request title drifted")
    require(result["schema_version"] == RESULT_TITLE, "result title drifted")
    require(input_hash(request) == request["input_sha256"], "request input hash drifted")
    require(result["request_input_sha256"] == request["input_sha256"], "result input hash is not request-bound")
    require(canonical_hash(result) == result["canonical_sha256"], "result canonical hash drifted")
    nested = result["authoring_mesh_v2"]
    require_exact_fields(nested, NESTED_FIELDS, "nested durable result")
    require(nested["project_id"] == result["project_id"], "nested project binding drifted")
    for field in ("mesh_id", "lineage_id", "revision_id", "revision_sha256", "revision_object_sha256"):
        require(nested[field] == result[field], f"nested {field} binding drifted")
    require(nested["request_input_sha256"] == result["request_input_sha256"], "nested request hash drifted")
    require(canonical_hash(nested) == nested["canonical_sha256"], "nested canonical hash drifted")
    require(result["source_operator_id"] in {"forgecad.geometry.primitive@2", "forgecad.geometry.profile-extrude@1"}, "source operator is not enabled")
    require(result["limitations"] == LIMITATIONS, "source limitations drifted")
    require(nested["limitations"] == NESTED_LIMITATIONS, "durable limitations drifted")
    require(result["runtime_write_performed"] is True and result["persistent_user_data_touched"] is True, "source result is not marked as Runtime-written")
    for field in ("stage_advanced", "candidate_confirmed", "version_created", "export_performed"):
        require(result[field] is False, f"source result promoted {field}")
    require(result["quality_status"] == "structural_source_bound_not_visually_evaluated", "source quality status drifted")
    require(result["canonical_sha256"] != "0" * 64, "result canonical hash is empty")


def check_negative_fixtures(request_schema: dict[str, Any], result_schema: dict[str, Any], registry: dict[str, dict[str, Any]]) -> None:
    request = load(REQUEST_PATH)
    result = load(RESULT_PATH)
    expected = {
        "request-extra-field.json": (request_schema, {**request, "unexpected": True}),
        "request-input-hash-drift.json": (request_schema, {**request, "input_sha256": "0" * 64}),
        "request-illegal-policy.json": (request_schema, {**request, "writer_policy": "caller-supplied@1"}),
        "result-extra-field.json": (result_schema, {**result, "unexpected": True}),
        "result-canonical-hash-drift.json": (result_schema, {**result, "canonical_sha256": "0" * 64}),
        "result-promotion.json": (result_schema, {**result, "candidate_confirmed": True}),
    }
    paths = sorted((FIXTURE_ROOT / "negative").glob("*.json"))
    require({path.name for path in paths} == set(expected), "negative fixture set drifted")
    for path in paths:
        marker = load(path)
        require(isinstance(marker, dict) and marker.get("mutation") == path.stem, f"{path.name} marker drifted")
        schema, mutated = expected[path.name]
        if "input-hash-drift" in path.name or "canonical-hash-drift" in path.name:
            require(is_valid(schema, mutated, registry), f"{path.name} is not structurally valid")
        else:
            require(not is_valid(schema, mutated, registry), f"{path.name} mutation was accepted")
        if "input-hash-drift" in path.name:
            require(input_hash(mutated) != mutated["input_sha256"], f"{path.name} is not a hash drift")
        if "canonical-hash-drift" in path.name:
            require(canonical_hash(mutated) != mutated["canonical_sha256"], f"{path.name} is not a hash drift")


def run_checks() -> None:
    request_schema, result_schema, registry = load_contracts()
    check_positive(request_schema, result_schema, registry)
    check_negative_fixtures(request_schema, result_schema, registry)
    print("Production Weapon AuthoringMeshV2 source contracts PASS")


if __name__ == "__main__":
    run_checks()
