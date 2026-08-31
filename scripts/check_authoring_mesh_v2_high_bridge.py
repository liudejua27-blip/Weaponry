#!/usr/bin/env python3
"""Focused closed-contract gate for WPN-KNIFE-HIGH-V2-BRIDGE-001.

This gate describes the narrow bridge around the existing direct V2 High
Worker evaluator.  It binds the complete ordered materialized
AuthoringMeshRevision@2 Part set to the source
binding and to the materialized candidate/program/artifact/readback proof,
then records only the Worker identities and same-cohort replay evidence.

The bridge deliberately does not expose the direct Worker's raw revision or
steps to callers.  Runtime must resolve those from durable truth and construct
the fixed CPU stitched request.  Candidate@1 is not AuthoringMeshCanonical@1,
and an artifact/GLB hash is not topology proof.  The gate is structural
contract evidence only: it does not run a Worker and does not claim High,
visual, human, engine or commercial quality.
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
FIXTURE_ROOT = CONTRACT_ROOT / "fixtures" / "authoring-mesh-v2-high-bridge"
MAIN_PATH = FIXTURE_ROOT / "positive" / "dragonfang-high-bridge.json"
PREPARE_PATH = FIXTURE_ROOT / "positive" / "dragonfang-high-bridge-prepare-request.json"
GET_PATH = FIXTURE_ROOT / "positive" / "dragonfang-high-bridge-get-request.json"
RESULT_PATH = FIXTURE_ROOT / "positive" / "dragonfang-high-bridge-result-prepared.json"
NEGATIVE_PATH = FIXTURE_ROOT / "negative" / "cases.json"
SOURCE_BINDING_PATH = (
    CONTRACT_ROOT
    / "fixtures"
    / "knife-source-binding"
    / "positive"
    / "dragonfang-source-binding.json"
)

sys.path.insert(0, str(ROOT / "scripts"))
from check_agentic_contracts import is_valid, load_schema_registry  # noqa: E402

MAIN_SCHEMA = "authoring-mesh-v2-high-bridge.schema.json"
PREPARE_SCHEMA = "authoring-mesh-v2-high-bridge-prepare-request.schema.json"
GET_SCHEMA = "authoring-mesh-v2-high-bridge-get-request.schema.json"
RESULT_SCHEMA = "authoring-mesh-v2-high-bridge-result.schema.json"
MAIN_TITLE = "AuthoringMeshV2HighBridge@1"
PREPARE_TITLE = "AuthoringMeshV2HighBridgePrepareRequest@1"
GET_TITLE = "AuthoringMeshV2HighBridgeGetRequest@1"
RESULT_TITLE = "AuthoringMeshV2HighBridgeResult@1"

MAIN_FIELDS = (
    "schema_version", "bridge_id", "project_id", "source_scope",
    "source_revision_schema_version", "mesh_id", "lineage_id", "revision_id",
    "revision_index", "revision_sha256", "revision_object_sha256", "source_binding_id",
    "source_binding_sha256", "source_binding_object_sha256", "materialized_candidate_id",
    "materialized_candidate_state_sha256", "materialized_program_sha256",
    "materialized_program_object_sha256", "materialized_artifact_id",
    "materialized_artifact_sha256", "materialized_artifact_object_sha256",
    "materialized_artifact_readback_sha256", "materialized_artifact_readback_object_sha256",
    "representation_plan_sha256", "source_node_id", "part_id", "material_zone_id", "solid",
    "source_part_output_sha256", "preserved_part_ids", "materialized_artifact_hash_policy",
    "high_execution_request_schema_version", "high_execution_operation", "high_operation", "high_result_schema_version",
    "high_readback_schema_version", "high_evaluator_contract", "high_subdivision_backend",
    "high_subdivision_levels", "high_max_triangles_per_face", "high_max_output_vertices",
    "high_max_output_triangles", "high_execution_request_sha256", "high_evaluation_sha256", "high_result_sha256",
    "high_result_object_sha256", "high_readback_sha256", "high_readback_object_sha256",
    "high_worker_algorithm_sha256", "high_worker_build_cohort_sha256", "high_replay_count",
    "high_replay_byte_exact", "high_non_destructive", "high_projected_source_mesh_sha256",
    "high_source_vertex_count", "high_source_triangle_count", "high_evaluated_part_count",
    "high_evaluated_triangle_count", "cohort_policy", "scope_limitations",
    "high_structural_status", "high_status", "quality_status", "visual_status", "human_status",
    "engine_status", "high_mesh_created", "high_stage_unlocked", "production_stage_advanced",
    "candidate_confirmed", "version_created", "export_performed", "runtime_write_performed",
    "persistent_user_data_touched", "writer_policy", "canonicalization_policy", "canonical_sha256",
    "created_at",
)

PREPARE_FIELDS = (
    "schema_version", "operation", "project_id", "bridge_id", "source_scope",
    "source_revision_schema_version", "mesh_id", "lineage_id", "revision_id", "revision_index",
    "revision_sha256", "revision_object_sha256", "source_binding_id", "source_binding_sha256",
    "source_binding_object_sha256", "materialized_candidate_id", "materialized_candidate_state_sha256",
    "materialized_program_sha256", "materialized_program_object_sha256", "materialized_artifact_id",
    "materialized_artifact_sha256", "materialized_artifact_object_sha256",
    "materialized_artifact_readback_sha256", "materialized_artifact_readback_object_sha256",
    "representation_plan_sha256", "source_node_id", "part_id", "material_zone_id", "solid",
    "source_part_output_sha256", "preserved_part_ids", "materialized_artifact_hash_policy",
    "high_execution_request_schema_version", "high_execution_operation", "high_operation", "high_result_schema_version",
    "high_readback_schema_version", "high_evaluator_contract", "high_subdivision_backend",
    "high_subdivision_levels", "high_max_triangles_per_face", "high_max_output_vertices",
    "high_max_output_triangles", "scope_limitations", "idempotency_key", "max_response_bytes",
    "runtime_write_performed", "writer_policy", "canonicalization_policy", "input_sha256",
)

GET_FIELDS = (
    "schema_version", "operation", "project_id", "bridge_id", "bridge_sha256",
    "bridge_object_sha256", "source_scope", "source_revision_schema_version", "mesh_id",
    "lineage_id", "revision_id", "revision_index", "revision_sha256", "revision_object_sha256",
    "source_binding_id", "source_binding_sha256", "source_binding_object_sha256",
    "materialized_candidate_id", "materialized_candidate_state_sha256", "materialized_program_sha256",
    "materialized_program_object_sha256", "materialized_artifact_id", "materialized_artifact_sha256",
    "materialized_artifact_object_sha256", "materialized_artifact_readback_sha256",
    "materialized_artifact_readback_object_sha256", "representation_plan_sha256", "source_node_id",
    "part_id", "material_zone_id", "solid", "source_part_output_sha256", "preserved_part_ids",
    "materialized_artifact_hash_policy", "high_execution_request_schema_version", "high_execution_operation", "high_operation",
    "high_result_schema_version", "high_readback_schema_version", "high_evaluator_contract",
    "high_subdivision_backend", "high_subdivision_levels", "high_max_triangles_per_face",
    "high_max_output_vertices", "high_max_output_triangles", "high_execution_request_sha256", "high_evaluation_sha256",
    "high_result_sha256", "high_result_object_sha256", "high_readback_sha256",
    "high_readback_object_sha256", "high_worker_algorithm_sha256", "high_worker_build_cohort_sha256",
    "high_replay_count", "high_replay_byte_exact", "high_non_destructive",
    "high_projected_source_mesh_sha256", "high_source_vertex_count", "high_source_triangle_count",
    "high_evaluated_part_count", "high_evaluated_triangle_count", "scope_limitations",
    "max_response_bytes", "runtime_write_performed", "persistent_user_data_touched", "writer_policy",
    "canonicalization_policy", "input_sha256",
)

RESULT_FIELDS = (
    "schema_version", "operation", "request_kind", "status", "project_id", "bridge_id",
    "bridge_sha256", "bridge_object_sha256", "bridge", "source_scope", "mesh_id", "lineage_id",
    "revision_id", "revision_index", "revision_sha256", "revision_object_sha256", "source_binding_id",
    "source_binding_sha256", "source_binding_object_sha256", "materialized_candidate_id",
    "materialized_candidate_state_sha256", "materialized_program_sha256",
    "materialized_program_object_sha256", "materialized_artifact_id", "materialized_artifact_sha256",
    "materialized_artifact_object_sha256", "materialized_artifact_readback_sha256",
    "materialized_artifact_readback_object_sha256", "representation_plan_sha256", "source_node_id",
    "part_id", "material_zone_id", "solid", "source_part_output_sha256", "preserved_part_ids",
    "high_execution_operation", "high_execution_request_sha256", "high_evaluation_sha256", "high_result_sha256", "high_result_object_sha256", "high_readback_sha256",
    "high_readback_object_sha256", "high_worker_algorithm_sha256", "high_worker_build_cohort_sha256",
    "high_replay_count", "high_replay_byte_exact", "high_non_destructive", "high_structural_status",
    "request_input_sha256", "idempotency_key", "replayed", "store_effect", "cas_effect",
    "atomicity_status", "store_commit_status", "cas_commit_status", "runtime_write_performed",
    "persistent_user_data_touched", "partial_result_exposed", "high_status", "quality_status",
    "visual_status", "human_status", "engine_status", "high_mesh_created", "high_stage_unlocked",
    "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed",
    "writer_policy", "canonicalization_policy", "canonical_sha256",
)

IDENTIFIER = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
POLICY = "forgecad-runtime-only-state-writer@1"
MAIN_CANONICALIZATION = "canonical-json-sha256-excluding-canonical-sha256@1"
REQUEST_CANONICALIZATION = "canonical-json-sha256-excluding-input-sha256@1"
MAX_RESPONSE_BYTES = 1_048_576
SOURCE_SCOPE = "materialized-v2-revision-part-set@1"
HIGH_OPERATION = "forgecad.production.authoring-mesh-v2-high-evaluate@1"
HIGH_EXECUTION_OPERATION = "forgecad.production.authoring-mesh-v2-high-execute@1"
HIGH_EXECUTION_SCHEMA_VERSION = "AuthoringMeshV2HighExecutionRequest@2"
HIGH_EVALUATOR_CONTRACT = "forgecad-owned-cpu-catmull-clark-stitched-polygon@2"
ARTIFACT_POLICY = "artifact-sha256-equals-object-sha256-until-semantic-artifact-contract@1"
SCOPE_LIMITATIONS = [
    "RUNTIME_DERIVES_COMPLETE_ORDERED_PART_INPUTS",
    "RUNTIME_CONSTRUCTS_CPU_STITCHED_STEPS",
    "NO_CALLER_SUPPLIED_REVISION_TOPOLOGY",
    "NO_OPEN_SUBDIVISION_BACKEND",
    "VERIFIED_PRESERVED_PARTS_FROM_MATERIALIZED_GLB",
]

# These are deliberately recorded by the checker, not exposed as new schema
# capabilities.  The direct Worker currently does not support these scopes.
CURRENT_SCOPE_UNSUPPORTED = (
    "caller_supplied_steps",
    "open_subdivision_backend",
    "boolean_steps",
    "raw_revision_payload",
)


class ContractViolation(Exception):
    pass


def fail(message: str) -> None:
    raise ContractViolation(message)


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
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False
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


def require_exact_fields(value: Any, expected: tuple[str, ...], label: str) -> None:
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
        elif key in {"items", "prefixItems", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"}:
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
    for key in {"$defs", "definitions", "items", "prefixItems", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"}:
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
    require(set(schema.get("required", [])) == set(properties), f"{filename} required/properties drifted")
    require(properties.get("schema_version", {}).get("const") == title, f"{filename} version drifted")
    for object_schema in walk_schema(schema):
        require(object_schema.get("additionalProperties") is False, f"{filename} contains an open local object")
    forbidden = {
        "path", "url", "uri", "raw", "raw_bytes", "bytes", "secret", "token", "password",
        "api_key", "prompt", "script", "shell", "environment", "executor", "topology",
        "steps", "revision",
    }
    require(
        not ({name.lower() for name in walk_property_names(schema)} & forbidden),
        f"{filename} exposes raw/evaluator property",
    )


def check_schemas(manifest: dict[str, Any], registry: dict[str, dict[str, Any]]) -> dict[str, dict[str, Any]]:
    schemas = {
        MAIN_SCHEMA: (MAIN_TITLE, MAIN_FIELDS),
        PREPARE_SCHEMA: (PREPARE_TITLE, PREPARE_FIELDS),
        GET_SCHEMA: (GET_TITLE, GET_FIELDS),
        RESULT_SCHEMA: (RESULT_TITLE, RESULT_FIELDS),
    }
    checked: dict[str, dict[str, Any]] = {}
    for filename, (title, fields) in schemas.items():
        require(filename in manifest.get("schemas", []), f"manifest omits {filename}")
        schema = object_at(SCHEMA_ROOT / filename)
        check_schema_shell(schema, filename, title)
        require(set(schema["required"]) == set(fields), f"{filename} fields drifted")
        require(registry.get(schema["$id"]) == schema, f"{filename} is not registry-bound")
        checked[filename] = schema
    main = checked[MAIN_SCHEMA]
    prepare = checked[PREPARE_SCHEMA]
    get = checked[GET_SCHEMA]
    result = checked[RESULT_SCHEMA]
    require(main["properties"]["scope_limitations"]["const"] == SCOPE_LIMITATIONS, "Main scope limitations drifted")
    require(prepare["properties"]["scope_limitations"]["const"] == SCOPE_LIMITATIONS, "Prepare scope limitations drifted")
    require(get["properties"]["scope_limitations"]["const"] == SCOPE_LIMITATIONS, "Get scope limitations drifted")
    require(
        result["properties"]["bridge"].get("$ref")
        == f"https://forgecad.local/contracts/{MAIN_SCHEMA}",
        "Result does not bind the Main bridge",
    )
    require(
        main["properties"]["materialized_artifact_hash_policy"].get("const") == ARTIFACT_POLICY,
        "artifact topology policy drifted",
    )
    require(
        main["properties"]["high_worker_build_cohort_sha256"].get("$ref") == "#/$defs/sha256",
        "Worker cohort is not required as a SHA-256 identity",
    )
    return checked


def check_identity_hashes(main: dict[str, Any], label: str) -> None:
    for field in (
        "revision_sha256", "revision_object_sha256", "source_binding_sha256",
        "source_binding_object_sha256", "materialized_candidate_state_sha256",
        "materialized_program_sha256", "materialized_program_object_sha256",
        "materialized_artifact_sha256", "materialized_artifact_object_sha256",
        "materialized_artifact_readback_sha256", "materialized_artifact_readback_object_sha256",
        "representation_plan_sha256", "source_part_output_sha256", "high_execution_request_sha256", "high_evaluation_sha256",
        "high_result_sha256", "high_result_object_sha256", "high_readback_sha256",
        "high_readback_object_sha256", "high_worker_algorithm_sha256", "high_worker_build_cohort_sha256",
        "high_projected_source_mesh_sha256",
    ):
        require(SHA256.fullmatch(main[field]) is not None, f"{label} {field} is not a SHA-256 identity")
    for field in ("bridge_id", "project_id", "mesh_id", "lineage_id", "revision_id", "source_binding_id", "materialized_candidate_id", "materialized_artifact_id", "source_node_id", "part_id", "material_zone_id"):
        require(IDENTIFIER.fullmatch(main[field]) is not None, f"{label} {field} is not an opaque identifier")


def check_main_semantics(main: dict[str, Any], source_binding: dict[str, Any]) -> None:
    require_exact_fields(main, MAIN_FIELDS, "Main")
    require(main["schema_version"] == MAIN_TITLE, "Main schema version drifted")
    require(main["source_scope"] == SOURCE_SCOPE, "Main source scope is not complete materialized Part set")
    require(main["source_revision_schema_version"] == "AuthoringMeshRevision@2", "Main revision schema drifted")
    require(main["materialized_candidate_id"] != source_binding["source_candidate_id"], "Candidate@1 was used as the materialized V2 identity")
    require(main["project_id"] == source_binding["project_id"], "Main project is not SourceBinding-bound")
    for field, source_field in (
        ("source_binding_id", "source_binding_id"),
        ("source_binding_sha256", "canonical_sha256"),
        ("mesh_id", "authoring_mesh_id"),
        ("lineage_id", "authoring_mesh_lineage_id"),
        ("revision_id", "authoring_mesh_revision_id"),
        ("revision_index", "authoring_mesh_revision_index"),
        ("revision_sha256", "authoring_mesh_revision_sha256"),
        ("revision_object_sha256", "authoring_mesh_revision_object_sha256"),
    ):
        require(main[field] == source_binding[source_field], f"Main {field} is not bound to SourceBinding")
    require(main["source_binding_object_sha256"] == sha256(source_binding), "SourceBinding object hash drifted")
    require(main["materialized_artifact_sha256"] == main["materialized_artifact_object_sha256"], "artifact hash policy is not obeyed")
    require(main["preserved_part_ids"], "source-bound bridge does not record preserved Parts")
    require(len(main["preserved_part_ids"]) == len(set(main["preserved_part_ids"])), "preserved Part IDs are not unique")
    require(main["part_id"] not in main["preserved_part_ids"], "target Part appears among preserved Parts")
    require(main["solid"] is True, "source Part solid proof drifted")
    require(main["scope_limitations"] == SCOPE_LIMITATIONS, "Main scope limitation record drifted")
    require(main["high_operation"] == HIGH_OPERATION, "direct Worker operation drifted")
    require(main["high_execution_operation"] == HIGH_EXECUTION_OPERATION, "Worker execution operation drifted")
    require(main["high_execution_request_schema_version"] == HIGH_EXECUTION_SCHEMA_VERSION, "Worker execution schema version drifted")
    require(main["high_evaluator_contract"] == HIGH_EVALUATOR_CONTRACT, "direct Worker evaluator contract drifted")
    require(main["high_replay_count"] == 2 and main["high_replay_byte_exact"] is True, "Worker replay proof drifted")
    require(main["high_non_destructive"] is True, "direct High evaluator became destructive")
    require(main["high_structural_status"] == "PASS_SOURCE_STRUCTURAL", "structural status drifted")
    require(main["high_status"] == "NOT_RUN", "Main claims a production High status")
    require(main["quality_status"] == "structural_only", "Main quality status drifted")
    require(main["visual_status"] == main["human_status"] == main["engine_status"] == "NOT_RUN", "Main promoted an unrun gate")
    for field in ("high_mesh_created", "high_stage_unlocked", "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed"):
        require(main[field] is False, f"Main {field} claims an unimplemented promotion")
    require(main["runtime_write_performed"] is True and main["persistent_user_data_touched"] is True, "durable Main is not Runtime-written")
    require(main["writer_policy"] == POLICY, "Main writer policy drifted")
    require(main["canonicalization_policy"] == MAIN_CANONICALIZATION, "Main canonical policy drifted")
    require(main["canonical_sha256"] == canonical_hash(main), "Main semantic canonical hash drifted")
    check_identity_hashes(main, "Main")
    require(not any(key in main for key in ("revision", "steps", "raw", "topology", "candidate", "artifact")), "Main exposes raw revision or Candidate/GLB payload")


def check_prepare(prepare: dict[str, Any], main: dict[str, Any], schema: dict[str, Any], registry: dict[str, dict[str, Any]]) -> None:
    require_exact_fields(prepare, PREPARE_FIELDS, "Prepare")
    require(is_valid(schema, prepare, registry), "positive Prepare is schema-invalid")
    require(prepare["operation"] == "authoring_mesh_v2_high_bridge_prepare", "Prepare operation drifted")
    require(prepare["project_id"] == main["project_id"] and prepare["bridge_id"] == main["bridge_id"], "Prepare bridge identity drifted")
    for field in PREPARE_FIELDS:
        if field not in {"schema_version", "operation", "idempotency_key", "max_response_bytes", "runtime_write_performed", "writer_policy", "canonicalization_policy", "input_sha256"}:
            require(prepare[field] == main[field], f"Prepare {field} is not Main-bound")
    require(prepare["max_response_bytes"] == MAX_RESPONSE_BYTES, "Prepare response bound drifted")
    require(prepare["runtime_write_performed"] is False, "Prepare is not write-gated")
    require(prepare["canonicalization_policy"] == REQUEST_CANONICALIZATION, "Prepare input canonical policy drifted")
    require(prepare["input_sha256"] == input_hash(prepare), "Prepare input hash drifted")
    require(not any(key in prepare for key in ("revision", "steps", "raw", "topology")), "Prepare exposes raw revision/evaluator steps")


def check_get(get: dict[str, Any], main: dict[str, Any], schema: dict[str, Any], registry: dict[str, dict[str, Any]]) -> None:
    require_exact_fields(get, GET_FIELDS, "Get")
    require(is_valid(schema, get, registry), "positive Get is schema-invalid")
    require(get["operation"] == "authoring_mesh_v2_high_bridge_get", "Get operation drifted")
    require(get["bridge_sha256"] == main["canonical_sha256"], "Get semantic bridge hash drifted")
    require(get["bridge_object_sha256"] == sha256(main), "Get bridge object hash drifted")
    for field in GET_FIELDS:
        if field not in {"schema_version", "operation", "bridge_sha256", "bridge_object_sha256", "max_response_bytes", "runtime_write_performed", "persistent_user_data_touched", "writer_policy", "canonicalization_policy", "input_sha256"}:
            require(get[field] == main[field], f"Get {field} is not Main-bound")
    require(get["runtime_write_performed"] is False and get["persistent_user_data_touched"] is False, "Get claims a write")
    require(get["canonicalization_policy"] == REQUEST_CANONICALIZATION, "Get input canonical policy drifted")
    require(get["input_sha256"] == input_hash(get), "Get input hash drifted")


def check_result(result: dict[str, Any], main: dict[str, Any], prepare: dict[str, Any], schema: dict[str, Any], registry: dict[str, dict[str, Any]]) -> None:
    require_exact_fields(result, RESULT_FIELDS, "Result")
    require(is_valid(schema, result, registry), "positive Result is schema-invalid")
    require(result["operation"] == "authoring_mesh_v2_high_bridge_prepare", "Result operation drifted")
    require(result["request_kind"] == "prepare" and result["status"] == "prepared", "positive Result branch drifted")
    require(result["bridge"] == main, "Result nested Main bridge drifted")
    require(result["bridge_sha256"] == main["canonical_sha256"], "Result semantic bridge hash drifted")
    require(result["bridge_object_sha256"] == sha256(main), "Result bridge object hash drifted")
    top_level_bound = (
        "project_id", "source_scope", "mesh_id", "lineage_id", "revision_id", "revision_index",
        "revision_sha256", "revision_object_sha256", "source_binding_id", "source_binding_sha256",
        "source_binding_object_sha256", "materialized_candidate_id", "materialized_candidate_state_sha256",
        "materialized_program_sha256", "materialized_program_object_sha256", "materialized_artifact_id",
        "materialized_artifact_sha256", "materialized_artifact_object_sha256", "materialized_artifact_readback_sha256",
        "materialized_artifact_readback_object_sha256", "representation_plan_sha256", "source_node_id",
        "part_id", "material_zone_id", "solid", "source_part_output_sha256", "preserved_part_ids",
        "high_execution_operation", "high_execution_request_sha256", "high_evaluation_sha256", "high_result_sha256", "high_result_object_sha256", "high_readback_sha256",
        "high_readback_object_sha256", "high_worker_algorithm_sha256", "high_worker_build_cohort_sha256",
        "high_replay_count", "high_replay_byte_exact", "high_non_destructive", "high_structural_status",
    )
    for field in top_level_bound:
        require(result[field] == main[field], f"Result {field} is not Main-bound")
    require(result["request_input_sha256"] == prepare["input_sha256"], "Result is not Prepare-bound")
    require(result["replayed"] is False and result["idempotency_key"] == prepare["idempotency_key"], "prepared Result replay identity drifted")
    require(result["store_effect"] == result["cas_effect"] == "inserted", "prepared Result effects drifted")
    require(result["atomicity_status"] == result["store_commit_status"] == result["cas_commit_status"] == "committed", "prepared Result commit proof drifted")
    require(result["runtime_write_performed"] is True and result["persistent_user_data_touched"] is True, "prepared Result is not durable")
    require(result["partial_result_exposed"] is False, "prepared Result exposes a partial result")
    for field in ("high_mesh_created", "high_stage_unlocked", "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed"):
        require(result[field] is False, f"Result {field} claims an unimplemented promotion")
    require(result["high_status"] == "NOT_RUN" and result["quality_status"] == "structural_only", "Result quality status drifted")
    require(result["visual_status"] == result["human_status"] == result["engine_status"] == "NOT_RUN", "Result promoted unrun downstream gate")
    require(result["canonical_sha256"] == canonical_hash(result), "Result canonical hash drifted")

    # Exercise the two read/replay result branches from the same durable
    # identity.  These are derived in-memory probes, not claims of live
    # Runtime/Store evidence.
    replayed = copy.deepcopy(result)
    replayed.update(
        {
            "status": "replayed",
            "idempotency_key": None,
            "replayed": True,
            "store_effect": "not-touched",
            "cas_effect": "not-touched",
            "atomicity_status": "not-touched",
            "store_commit_status": "not-touched",
            "cas_commit_status": "not-touched",
            "runtime_write_performed": False,
            "persistent_user_data_touched": False,
            "canonical_sha256": "",
        }
    )
    replayed["canonical_sha256"] = canonical_hash(replayed)
    require(is_valid(schema, replayed, registry), "derived replayed Result branch is schema-invalid")

    found = copy.deepcopy(replayed)
    found.update(
        {
            "operation": "authoring_mesh_v2_high_bridge_get",
            "request_kind": "get",
            "status": "found",
            "replayed": False,
            "canonical_sha256": "",
        }
    )
    found["canonical_sha256"] = canonical_hash(found)
    require(is_valid(schema, found, registry), "derived found Result branch is schema-invalid")


def mutate(value: dict[str, Any], mutation: str) -> dict[str, Any]:
    result = copy.deepcopy(value)
    if mutation == "extra_field":
        result["unexpected"] = True
    elif mutation == "source_scope":
        result["source_scope"] = "multi-v2-revision"
    elif mutation == "candidate_as_source":
        result["materialized_candidate_id"] = "dragonfang-blockout-candidate-001"
    elif mutation == "artifact_topology_policy":
        result["materialized_artifact_hash_policy"] = "artifact-contains-topology"
    elif mutation == "artifact_hash_mismatch":
        result["materialized_artifact_sha256"] = "a" * 64
    elif mutation == "high_promotion":
        result["high_status"] = "PASS"
    elif mutation == "worker_cohort_missing":
        result.pop("high_worker_build_cohort_sha256", None)
    elif mutation == "canonical_hash":
        result["canonical_sha256"] = "0" * 64
    elif mutation == "raw_revision":
        result["revision"] = {"vertices": []}
    elif mutation == "input_hash":
        result["input_sha256"] = "0" * 64
    elif mutation == "write_claim":
        result["runtime_write_performed"] = True
    elif mutation == "result_status":
        result["status"] = "replayed"
    elif mutation == "result_promotion":
        result["production_stage_advanced"] = True
    else:
        fail(f"unknown negative mutation {mutation}")
    return result


def check_negative(
    descriptor: dict[str, Any], main: dict[str, Any], prepare: dict[str, Any], get: dict[str, Any], result: dict[str, Any],
    schemas: dict[str, dict[str, Any]], registry: dict[str, dict[str, Any]], source_binding: dict[str, Any],
) -> None:
    require(descriptor.get("schema_version") == "AuthoringMeshV2HighBridgeNegativeCases@1", "negative fixture version drifted")
    cases = descriptor.get("cases")
    require(isinstance(cases, list) and cases, "negative fixture cases are empty")
    expected = {
        "main-extra-field", "main-source-scope", "main-candidate-as-source", "main-artifact-topology-policy",
        "main-artifact-hash-mismatch", "main-high-promotion", "main-worker-cohort-missing", "main-canonical-hash",
        "prepare-extra-field", "prepare-raw-revision", "prepare-input-hash", "get-write-claim",
        "result-extra-field", "result-status", "result-promotion", "result-canonical-hash",
    }
    require({case.get("id") for case in cases} == expected, "negative case inventory drifted")
    base = {"main": main, "prepare": prepare, "get": get, "result": result}
    schema_by_target = {"main": schemas[MAIN_SCHEMA], "prepare": schemas[PREPARE_SCHEMA], "get": schemas[GET_SCHEMA], "result": schemas[RESULT_SCHEMA]}
    for case in cases:
        require(isinstance(case, dict), "negative case must be an object")
        target = case.get("target")
        require(target in base, f"negative case target drifted: {case.get('id')}")
        candidate = mutate(base[target], case["mutation"])
        schema = schema_by_target[target]
        if case["mutation"] == "canonical_hash":
            require(is_valid(schema, candidate, registry), f"canonical negative is not schema-valid: {case['id']}")
            require(canonical_hash(candidate) != candidate["canonical_sha256"], f"canonical negative did not drift: {case['id']}")
            continue
        if case["mutation"] == "input_hash":
            require(is_valid(schema, candidate, registry), f"input negative is not schema-valid: {case['id']}")
            require(input_hash(candidate) != candidate["input_sha256"], f"input negative did not drift: {case['id']}")
            continue
        if is_valid(schema, candidate, registry):
            try:
                if target == "main":
                    check_main_semantics(candidate, source_binding)
                elif target == "prepare":
                    check_prepare(candidate, main, schema, registry)
                elif target == "get":
                    check_get(candidate, main, schema, registry)
                else:
                    check_result(candidate, main, prepare, schema, registry)
            except ContractViolation:
                continue
            fail(f"schema-valid negative unexpectedly passed semantic gate: {case['id']}")


def run_checks() -> None:
    manifest = object_at(MANIFEST_PATH)
    registry = load_schema_registry(manifest)
    schemas = check_schemas(manifest, registry)
    main = object_at(MAIN_PATH)
    prepare = object_at(PREPARE_PATH)
    get = object_at(GET_PATH)
    result = object_at(RESULT_PATH)
    source_binding = object_at(SOURCE_BINDING_PATH)
    check_main_semantics(main, source_binding)
    check_prepare(prepare, main, schemas[PREPARE_SCHEMA], registry)
    check_get(get, main, schemas[GET_SCHEMA], registry)
    check_result(result, main, prepare, schemas[RESULT_SCHEMA], registry)
    check_negative(object_at(NEGATIVE_PATH), main, prepare, get, result, schemas, registry, source_binding)
    print(
        "AuthoringMeshV2 High bridge contracts OK: closed Main/Prepare/Get/Result, "
        "SourceBinding/materialized identity, direct Worker replay/cohort and structural-only gates; "
        "unsupported current scope remains checker-recorded"
    )


if __name__ == "__main__":
    try:
        run_checks()
    except ContractViolation as exc:
        raise SystemExit(f"AuthoringMeshV2 High bridge contract violation: {exc}") from exc
