#!/usr/bin/env python3
"""Focused closed-contract gate for WPN-KNIFE-PASS-STATE-001.

This is a contracts-only gate.  It checks the immutable, hash-bound pass
snapshot and its prepare/get/result envelopes, but it does not claim that a
Runtime, Store, MCP operation, human review or engine gate has run.
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
FIXTURE_ROOT = CONTRACT_ROOT / "fixtures" / "knife-pass-state"
POSITIVE_PATH = FIXTURE_ROOT / "positive" / "dragonfang-pass-state.json"
SOURCE_PATH = (
    CONTRACT_ROOT
    / "fixtures"
    / "knife-source-binding"
    / "positive"
    / "dragonfang-source-binding.json"
)

sys.path.insert(0, str(ROOT / "scripts"))
from check_agentic_contracts import is_valid, load_schema_registry  # noqa: E402

MAIN = "knife-pass-state.schema.json"
PREPARE = "knife-pass-state-prepare-request.schema.json"
GET = "knife-pass-state-get-request.schema.json"
RESULT = "knife-pass-state-result.schema.json"
SCHEMA_VERSION = "KnifePassState@1"
PREPARE_VERSION = "KnifePassStatePrepareRequest@1"
GET_VERSION = "KnifePassStateGetRequest@1"
RESULT_VERSION = "KnifePassStateResult@1"
CANONICAL_POLICY = "canonical-json-sha256-excluding-canonical-sha256@1"
INPUT_POLICY = "canonical-json-sha256-excluding-input-sha256@1"
WRITER_POLICY = "forgecad-runtime-only-state-writer@1"
ALLOWED_QUALITY_STATUS = {"NOT_RUN", "QUALITY_TARGET_NOT_MET", "BLOCKED_REFERENCE_COVERAGE"}

MAIN_FIELDS = {
    "schema_version", "pass_id", "parent_pass_id", "parent_pass_sha256", "project_id", "stage",
    "source_binding_id", "source_binding_sha256", "source_binding_object_sha256",
    "intent_bundle_id", "intent_bundle_sha256", "intent_bundle_object_sha256",
    "brief_id", "brief_sha256", "brief_object_sha256",
    "reference_id", "reference_object_sha256", "reference_evidence_sha256",
    "source_candidate_id", "source_candidate_state_sha256",
    "baseline_candidate_id", "baseline_candidate_state_sha256", "baseline_artifact_sha256",
    "baseline_geometry_program_sha256", "baseline_geometry_program_object_sha256",
    "baseline_artifact_readback_object_sha256", "baseline_representation_plan_sha256",
    "attempt_candidate_id", "attempt_candidate_state_sha256", "attempt_artifact_sha256",
    "attempt_geometry_program_sha256", "attempt_geometry_program_object_sha256",
    "attempt_artifact_readback_object_sha256", "attempt_representation_plan_sha256",
    "authoring_mesh_id", "authoring_mesh_lineage_id", "authoring_mesh_revision_id",
    "authoring_mesh_revision_index", "authoring_mesh_revision_sha256",
    "authoring_mesh_revision_object_sha256", "authoring_mesh_identity_sha256", "authoring_mesh_sha256",
    "modifier_graph_id", "modifier_graph_sha256", "evaluated_mesh_id", "evaluated_mesh_sha256",
    "high_artifact_id", "high_artifact_sha256", "fixed_view", "camera_set_sha256",
    "render_set_id", "render_set_sha256", "render_set_object_sha256",
    "reference_comparison_id", "reference_comparison_sha256", "reference_comparison_object_sha256",
    "quality_report_id", "quality_report_sha256", "quality_report_object_sha256", "evidence_bundle_sha256",
    "hard_gate_status", "visual_gate_status", "quality_status", "high_status", "human_status", "engine_status",
    "unknowns", "unlocked_successor", "high_mesh_created", "high_stage_unlocked",
    "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed",
    "canonicalization_policy", "canonical_sha256", "created_at",
}

PREPARE_FIELDS = {
    "schema_version", "operation", "project_id", "pass_state",
    "idempotency_key", "max_response_bytes", "runtime_write_performed", "writer_policy",
    "canonicalization_policy", "input_sha256",
}

GET_FIELDS = {
    "schema_version", "operation", "project_id", "pass_id", "pass_state_sha256", "pass_state_object_sha256",
    "source_binding_id", "source_binding_sha256", "source_binding_object_sha256",
    "intent_bundle_id", "intent_bundle_sha256", "intent_bundle_object_sha256",
    "brief_id", "brief_sha256", "brief_object_sha256", "reference_id", "reference_object_sha256",
    "reference_evidence_sha256", "source_candidate_id", "source_candidate_state_sha256",
    "baseline_candidate_id", "baseline_candidate_state_sha256", "baseline_artifact_sha256",
    "baseline_geometry_program_sha256", "baseline_geometry_program_object_sha256",
    "baseline_artifact_readback_object_sha256", "baseline_representation_plan_sha256",
    "attempt_candidate_id", "attempt_candidate_state_sha256", "attempt_artifact_sha256",
    "attempt_geometry_program_sha256", "attempt_geometry_program_object_sha256",
    "attempt_artifact_readback_object_sha256", "attempt_representation_plan_sha256",
    "authoring_mesh_id", "authoring_mesh_lineage_id", "authoring_mesh_revision_id",
    "authoring_mesh_revision_index", "authoring_mesh_revision_sha256", "authoring_mesh_revision_object_sha256",
    "authoring_mesh_identity_sha256", "authoring_mesh_sha256", "fixed_view_id", "camera_set_sha256",
    "render_set_id", "render_set_sha256", "render_set_object_sha256",
    "reference_comparison_id", "reference_comparison_sha256", "reference_comparison_object_sha256",
    "quality_report_id", "quality_report_sha256", "quality_report_object_sha256", "evidence_bundle_sha256",
    "max_response_bytes", "runtime_write_performed", "persistent_user_data_touched", "writer_policy",
    "canonicalization_policy", "input_sha256",
}

RESULT_FIELDS = {
    "schema_version", "operation", "request_kind", "status", "project_id", "pass_id", "pass_state_sha256",
    "pass_state_object_sha256", "pass_state", "source_binding_id", "source_binding_sha256",
    "source_binding_object_sha256", "intent_bundle_id", "intent_bundle_sha256", "intent_bundle_object_sha256",
    "brief_id", "brief_sha256", "brief_object_sha256", "reference_id", "reference_object_sha256",
    "reference_evidence_sha256", "source_candidate_id", "source_candidate_state_sha256",
    "baseline_candidate_id", "baseline_candidate_state_sha256", "baseline_artifact_sha256",
    "baseline_geometry_program_sha256", "baseline_geometry_program_object_sha256",
    "baseline_artifact_readback_object_sha256", "baseline_representation_plan_sha256",
    "attempt_candidate_id", "attempt_candidate_state_sha256", "attempt_artifact_sha256",
    "attempt_geometry_program_sha256", "attempt_geometry_program_object_sha256",
    "attempt_artifact_readback_object_sha256", "attempt_representation_plan_sha256",
    "authoring_mesh_id", "authoring_mesh_lineage_id", "authoring_mesh_revision_id",
    "authoring_mesh_revision_index", "authoring_mesh_revision_sha256", "authoring_mesh_revision_object_sha256",
    "authoring_mesh_identity_sha256", "authoring_mesh_sha256", "fixed_view_id", "camera_set_sha256",
    "render_set_id", "render_set_sha256", "render_set_object_sha256",
    "reference_comparison_id", "reference_comparison_sha256", "reference_comparison_object_sha256",
    "quality_report_id", "quality_report_sha256", "quality_report_object_sha256", "evidence_bundle_sha256",
    "hard_gate_status", "visual_gate_status", "quality_status", "high_status", "human_status", "engine_status",
    "high_mesh_created", "high_stage_unlocked", "production_stage_advanced", "candidate_confirmed",
    "version_created", "export_performed", "idempotency_key", "replayed", "store_effect", "cas_effect",
    "atomicity_status", "store_commit_status", "cas_commit_status", "runtime_write_performed",
    "persistent_user_data_touched", "partial_result_exposed", "writer_policy",
    "canonicalization_policy", "canonical_sha256",
}

SOURCE_BINDING_FIELDS = (
    "source_binding_id", "intent_bundle_id", "intent_bundle_sha256", "intent_bundle_object_sha256",
    "brief_id", "brief_sha256", "brief_object_sha256", "reference_id", "reference_object_sha256",
    "reference_evidence_sha256", "source_candidate_id", "source_candidate_state_sha256", "authoring_mesh_id",
    "authoring_mesh_lineage_id", "authoring_mesh_revision_id", "authoring_mesh_revision_index",
    "authoring_mesh_revision_sha256", "authoring_mesh_revision_object_sha256", "authoring_mesh_identity_sha256",
)

NEGATIVE_PATHS = sorted((FIXTURE_ROOT / "negative").glob("*.json"))


def fail(message: str) -> None:
    raise SystemExit(f"Weaponry knife pass-state contract violation: {message}")


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
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


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


def check_schema_shell(schema: dict[str, Any], filename: str, title: str, fields: set[str]) -> None:
    require(schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema", f"{filename} draft drifted")
    require(schema.get("$id") == f"https://forgecad.local/contracts/{filename}", f"{filename} id drifted")
    require(schema.get("title") == title, f"{filename} title drifted")
    require(schema.get("type") == "object" and schema.get("additionalProperties") is False, f"{filename} root is open")
    require(schema.get("properties", {}).get("schema_version", {}).get("const") == title, f"{filename} version drifted")
    require(set(schema.get("required", [])) == fields, f"{filename} required/properties field set drifted")
    require(set(schema.get("properties", {})) == fields, f"{filename} properties field set drifted")
    for object_schema in walk_objects(schema):
        require(object_schema.get("additionalProperties") is False, f"{filename} contains an open object")
    forbidden = {"path", "url", "uri", "raw", "raw_bytes", "bytes", "secret", "token", "password", "api_key", "prompt", "script", "shell", "environment", "executor"}
    require(not ({name.lower() for name in property_names(schema)} & forbidden), f"{filename} exposes a forbidden property")
    require(schema.get("$defs", {}).get("identifier", {}).get("pattern") == "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$", f"{filename} identifier is not strict")
    require(schema.get("$defs", {}).get("sha256", {}).get("pattern") == "^[0-9a-f]{64}$", f"{filename} SHA-256 is not strict")


def check_schemas(manifest: dict[str, Any], registry: dict[str, dict[str, Any]]) -> dict[str, dict[str, Any]]:
    expected = {
        MAIN: (SCHEMA_VERSION, MAIN_FIELDS),
        PREPARE: (PREPARE_VERSION, PREPARE_FIELDS),
        GET: (GET_VERSION, GET_FIELDS),
        RESULT: (RESULT_VERSION, RESULT_FIELDS),
    }
    declared = set(manifest.get("schemas", []))
    require(set(expected) <= declared, "manifest does not register every pass-state schema")
    schemas: dict[str, dict[str, Any]] = {}
    for filename, (title, fields) in expected.items():
        schema = object_at(SCHEMA_ROOT / filename)
        check_schema_shell(schema, filename, title, fields)
        schemas[filename] = schema
    require(registry.get(schemas[MAIN]["$id"]) == schemas[MAIN], "Main schema is not registry-bound")
    require(schemas[MAIN]["properties"]["canonicalization_policy"].get("const") == CANONICAL_POLICY, "Main hash policy is not non-self-referential")
    require("pass_state_object_sha256" not in schemas[MAIN]["properties"], "Main embeds its own external object hash")
    require(schemas[MAIN]["properties"]["fixed_view"].get("$ref") == "#/$defs/fixed_view", "Main fixed view is not closed")
    require(schemas[MAIN]["$defs"]["fixed_view"]["properties"]["reference_required"].get("const") is True, "fixed view is not reference-bound")
    require(schemas[RESULT]["properties"]["pass_state"].get("$ref") == "https://forgecad.local/contracts/knife-pass-state.schema.json", "result does not return Main")
    return schemas


def is_sha256(value: Any) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(char in "0123456789abcdef" for char in value)


def check_hash_fields(state: dict[str, Any]) -> None:
    for field in MAIN_FIELDS:
        if field.endswith("_sha256") and field not in {"canonical_sha256", "parent_pass_sha256", "modifier_graph_sha256", "evaluated_mesh_sha256", "high_artifact_sha256"}:
            require(is_sha256(state[field]), f"{field} is not a lowercase SHA-256")
    for field in ("parent_pass_sha256", "modifier_graph_sha256", "evaluated_mesh_sha256", "high_artifact_sha256"):
        require(state[field] is None or is_sha256(state[field]), f"{field} nullable SHA-256 is malformed")


def expected_camera_set(state: dict[str, Any]) -> str:
    return sha256({"schema_version": "KnifeCameraSet@1", "fixed_views": [state["fixed_view"]], "fixed_view_count": 1})


def check_fixed_view(state: dict[str, Any]) -> None:
    view = state["fixed_view"]
    require(set(view) == {
        "view_id", "view_kind", "comparison_role", "reference_required", "camera_id",
        "camera_sha256", "reference_view_id", "reference_view_sha256", "fixed_view_policy",
    }, "fixed view is not the closed nine-field envelope")
    require(view["view_kind"] == "front", "positive fixture is not the bounded front view")
    require(view["comparison_role"] == "primary-reference" and view["reference_required"] is True, "fixed view is not reference-bound")
    require(view["reference_view_id"] == view["view_id"], "fixed view reference id drifted")
    require(view["fixed_view_policy"] == "single-runtime-bound-primary-reference-view@1", "fixed view policy drifted")
    require(is_sha256(view["camera_sha256"]), "fixed view camera hash is malformed")
    require(is_sha256(view["reference_view_sha256"]), "fixed view reference mask hash is malformed")
    require(view["reference_view_sha256"] != state["reference_object_sha256"], "fixed view embeds raw reference object instead of comparison mask")


def check_main(state: dict[str, Any], schema: dict[str, Any], registry: dict[str, dict[str, Any]], source: dict[str, Any]) -> str:
    require(is_valid(schema, state, registry), "positive Main fixture is schema-invalid")
    require(state["project_id"] == source["project_id"], "pass project is not source-bound")
    check_hash_fields(state)
    require(state["source_binding_id"] == source["source_binding_id"], "source binding id drifted")
    require(state["source_binding_sha256"] == source["canonical_sha256"], "source binding semantic hash drifted")
    require(state["source_binding_object_sha256"] == sha256(source), "source binding object hash drifted")
    for field in SOURCE_BINDING_FIELDS[1:]:
        require(state[field] == source[field], f"source lineage field drifted: {field}")
    require(state["source_candidate_id"] == source["source_candidate_id"], "source candidate id drifted")
    require(state["source_candidate_state_sha256"] == source["source_candidate_state_sha256"], "source candidate state drifted")
    for role in ("baseline", "attempt"):
        require(isinstance(state[f"{role}_candidate_id"], str), f"{role} candidate id is malformed")
        require(is_sha256(state[f"{role}_candidate_state_sha256"]), f"{role} candidate state hash is malformed")
        for field in (
            f"{role}_artifact_sha256", f"{role}_geometry_program_sha256",
            f"{role}_geometry_program_object_sha256", f"{role}_artifact_readback_object_sha256",
            f"{role}_representation_plan_sha256",
        ):
            require(is_sha256(state[field]), f"{field} lineage hash is malformed")
    for field in (
        "authoring_mesh_id", "authoring_mesh_lineage_id", "authoring_mesh_revision_id",
        "authoring_mesh_revision_index", "authoring_mesh_revision_sha256",
        "authoring_mesh_revision_object_sha256", "authoring_mesh_identity_sha256",
    ):
        require(state[field] == source[field], f"AuthoringMesh revision field drifted: {field}")
    require(state["authoring_mesh_sha256"] == source["authoring_mesh_revision_sha256"], "AuthoringMesh semantic hash is not revision-bound")
    require((state["modifier_graph_id"] is None) == (state["modifier_graph_sha256"] is None), "modifier graph nullable pair is inconsistent")
    require((state["evaluated_mesh_id"] is None) == (state["evaluated_mesh_sha256"] is None), "evaluated mesh nullable pair is inconsistent")
    require(state["high_artifact_id"] is None and state["high_artifact_sha256"] is None, "High artifact was materialized in a pre-High pass")
    check_fixed_view(state)
    require(state["camera_set_sha256"] == expected_camera_set(state), "camera set hash is not derived from the fixed view")
    for prefix in ("render_set", "reference_comparison", "quality_report"):
        require(isinstance(state[f"{prefix}_id"], str), f"{prefix} id is malformed")
        require(is_sha256(state[f"{prefix}_sha256"]) and is_sha256(state[f"{prefix}_object_sha256"]), f"{prefix} semantic/object identity is malformed")
    require(state["evidence_bundle_sha256"] == sha256({
        "schema_version": "KnifeEvidenceBundle@1",
        "render_set_sha256": state["render_set_sha256"],
        "reference_comparison_sha256": state["reference_comparison_sha256"],
        "quality_report_sha256": state["quality_report_sha256"],
        "camera_set_sha256": state["camera_set_sha256"],
    }), "evidence bundle hash drifted")
    require(state["hard_gate_status"] in {"NOT_RUN", "BLOCKED", "FAIL", "PASS_SOURCE_STRUCTURAL"}, "hard gate status is invalid")
    require(state["visual_gate_status"] in ALLOWED_QUALITY_STATUS, "visual status is not conservative")
    require(state["quality_status"] == state["visual_gate_status"], "quality and visual status diverged")
    require(state["quality_status"] == "BLOCKED_REFERENCE_COVERAGE", "positive fixture must preserve reference coverage block")
    require(state["high_status"] in {"NOT_RUN", "BLOCKED"}, "High status was promoted")
    require(state["human_status"] == "NOT_RUN" and state["engine_status"] == "NOT_RUN", "human/engine status was promoted")
    require({item["view_kind"] for item in state["unknowns"]} == {"front-three-quarter", "top", "bottom", "fps-inspect"}, "reference unknowns are not the bounded missing views")
    require(state["unlocked_successor"] == "none", "blocked pass exposes an unlocked successor")
    for field in ("high_mesh_created", "high_stage_unlocked", "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed"):
        require(state[field] is False, f"pass state promoted {field}")
    require(state["canonicalization_policy"] == CANONICAL_POLICY, "Main canonicalization policy drifted")
    require(state["canonical_sha256"] == canonical_hash(state), "Main canonical hash is stale")
    return sha256(state)


def prepare_fixture(state: dict[str, Any], object_hash: str) -> dict[str, Any]:
    value = {
        "schema_version": PREPARE_VERSION,
        "operation": "knife_pass_state_prepare",
        "project_id": state["project_id"],
        "pass_state": state,
        "idempotency_key": "dragonfang-pass-state-prepare-001",
        "max_response_bytes": 1048576,
        "runtime_write_performed": False,
        "writer_policy": WRITER_POLICY,
        "canonicalization_policy": INPUT_POLICY,
        "input_sha256": "",
    }
    value["input_sha256"] = input_hash(value)
    return value


def get_fixture(state: dict[str, Any], object_hash: str) -> dict[str, Any]:
    fields = (
        "source_binding_id", "source_binding_sha256", "source_binding_object_sha256",
        "intent_bundle_id", "intent_bundle_sha256", "intent_bundle_object_sha256", "brief_id", "brief_sha256",
        "brief_object_sha256", "reference_id", "reference_object_sha256", "reference_evidence_sha256",
        "source_candidate_id", "source_candidate_state_sha256", "baseline_candidate_id", "baseline_candidate_state_sha256",
        "baseline_artifact_sha256", "baseline_geometry_program_sha256", "baseline_geometry_program_object_sha256",
        "baseline_artifact_readback_object_sha256", "baseline_representation_plan_sha256",
        "attempt_candidate_id", "attempt_candidate_state_sha256", "attempt_artifact_sha256",
        "attempt_geometry_program_sha256", "attempt_geometry_program_object_sha256",
        "attempt_artifact_readback_object_sha256", "attempt_representation_plan_sha256",
        "authoring_mesh_id", "authoring_mesh_lineage_id", "authoring_mesh_revision_id", "authoring_mesh_revision_index",
        "authoring_mesh_revision_sha256", "authoring_mesh_revision_object_sha256", "authoring_mesh_identity_sha256",
        "authoring_mesh_sha256", "camera_set_sha256", "render_set_id", "render_set_sha256", "render_set_object_sha256",
        "reference_comparison_id", "reference_comparison_sha256", "reference_comparison_object_sha256",
        "quality_report_id", "quality_report_sha256", "quality_report_object_sha256", "evidence_bundle_sha256",
    )
    value = {
        "schema_version": GET_VERSION,
        "operation": "knife_pass_state_get",
        "project_id": state["project_id"],
        "pass_id": state["pass_id"],
        "pass_state_sha256": state["canonical_sha256"],
        "pass_state_object_sha256": object_hash,
    }
    value.update({field: state[field] for field in fields})
    value["fixed_view_id"] = state["fixed_view"]["view_id"]
    value.update({
        "max_response_bytes": 1048576,
        "runtime_write_performed": False,
        "persistent_user_data_touched": False,
        "writer_policy": WRITER_POLICY,
        "canonicalization_policy": INPUT_POLICY,
        "input_sha256": "",
    })
    value["input_sha256"] = input_hash(value)
    return value


def result_fixture(state: dict[str, Any], object_hash: str, request_kind: str, status: str) -> dict[str, Any]:
    prepare = request_kind == "prepare"
    value: dict[str, Any] = {
        "schema_version": RESULT_VERSION,
        "operation": "knife_pass_state_prepare" if prepare else "knife_pass_state_get",
        "request_kind": request_kind,
        "status": status,
        "project_id": state["project_id"],
        "pass_id": state["pass_id"],
        "pass_state_sha256": state["canonical_sha256"],
        "pass_state_object_sha256": object_hash,
        "pass_state": state,
        "idempotency_key": "dragonfang-pass-state-prepare-001" if prepare and status == "prepared" else None,
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
        "writer_policy": WRITER_POLICY,
        "canonicalization_policy": CANONICAL_POLICY,
        "canonical_sha256": "",
    }
    for field in (
        "source_binding_id", "source_binding_sha256", "source_binding_object_sha256", "intent_bundle_id",
        "intent_bundle_sha256", "intent_bundle_object_sha256", "brief_id", "brief_sha256", "brief_object_sha256",
        "reference_id", "reference_object_sha256", "reference_evidence_sha256", "source_candidate_id",
        "source_candidate_state_sha256", "baseline_candidate_id", "baseline_candidate_state_sha256",
        "baseline_artifact_sha256", "baseline_geometry_program_sha256", "baseline_geometry_program_object_sha256",
        "baseline_artifact_readback_object_sha256", "baseline_representation_plan_sha256",
        "attempt_candidate_id", "attempt_candidate_state_sha256", "attempt_artifact_sha256",
        "attempt_geometry_program_sha256", "attempt_geometry_program_object_sha256",
        "attempt_artifact_readback_object_sha256", "attempt_representation_plan_sha256",
        "authoring_mesh_id", "authoring_mesh_lineage_id", "authoring_mesh_revision_id", "authoring_mesh_revision_index",
        "authoring_mesh_revision_sha256", "authoring_mesh_revision_object_sha256", "authoring_mesh_identity_sha256",
        "authoring_mesh_sha256", "camera_set_sha256", "render_set_id", "render_set_sha256", "render_set_object_sha256",
        "reference_comparison_id", "reference_comparison_sha256", "reference_comparison_object_sha256", "quality_report_id",
        "quality_report_sha256", "quality_report_object_sha256", "evidence_bundle_sha256", "hard_gate_status",
        "visual_gate_status", "quality_status", "high_status", "human_status", "engine_status",
    ):
        value[field] = state[field]
    value["fixed_view_id"] = state["fixed_view"]["view_id"]
    value["canonical_sha256"] = canonical_hash(value)
    return value


def check_transports(schemas: dict[str, dict[str, Any]], state: dict[str, Any], registry: dict[str, dict[str, Any]], object_hash: str) -> None:
    prepare = prepare_fixture(state, object_hash)
    get = get_fixture(state, object_hash)
    require(is_valid(schemas[PREPARE], prepare, registry), "prepare fixture is schema-invalid")
    require(is_valid(schemas[GET], get, registry), "get fixture is schema-invalid")
    require(prepare["input_sha256"] == input_hash(prepare), "prepare input hash is stale")
    require(get["input_sha256"] == input_hash(get), "get input hash is stale")
    require(prepare["pass_state"]["canonical_sha256"] == state["canonical_sha256"], "prepare Main hash drifted")
    require(get["pass_state_sha256"] == state["canonical_sha256"], "get Main hash drifted")
    for request_kind, status in (("prepare", "prepared"), ("prepare", "replayed"), ("get", "found")):
        result = result_fixture(state, object_hash, request_kind, status)
        require(is_valid(schemas[RESULT], result, registry), f"{request_kind}/{status} result is schema-invalid")
        require(result["pass_state_sha256"] == state["canonical_sha256"], f"{request_kind}/{status} Main semantic hash drifted")
        require(result["pass_state_object_sha256"] == object_hash, f"{request_kind}/{status} Main object hash drifted")
        require(result["pass_state"]["canonical_sha256"] == state["canonical_sha256"], f"{request_kind}/{status} nested Main drifted")
        require(result["canonical_sha256"] == canonical_hash(result), f"{request_kind}/{status} result hash is stale")


def check_negative_fixtures(schema: dict[str, Any], registry: dict[str, dict[str, Any]], state: dict[str, Any], source: dict[str, Any]) -> None:
    require(NEGATIVE_PATHS, "pass-state negative fixture directory is empty")
    for path in NEGATIVE_PATHS:
        value = object_at(path)
        if path.name in {"source-binding-hash-mismatch.json", "evidence-hash-mismatch.json"}:
            require(is_valid(schema, value, registry), f"semantic negative fixture is not schema-valid: {path.name}")
            try:
                check_main(value, schema, registry, source)
            except SystemExit:
                pass
            else:
                fail(f"semantic negative fixture unexpectedly passed: {path.name}")
        else:
            require(not is_valid(schema, value, registry), f"negative fixture unexpectedly passed: {path.name}")
    require(
        object_at(FIXTURE_ROOT / "negative" / "source-binding-hash-mismatch.json")["source_binding_sha256"]
        != state["source_binding_sha256"],
        "source-binding semantic negative was not mutated",
    )
    require(
        object_at(FIXTURE_ROOT / "negative" / "evidence-hash-mismatch.json")["render_set_sha256"]
        != state["render_set_sha256"],
        "evidence semantic negative was not mutated",
    )


def run_checks() -> None:
    manifest = object_at(MANIFEST_PATH)
    registry = load_schema_registry(manifest)
    schemas = check_schemas(manifest, registry)
    state = object_at(POSITIVE_PATH)
    source = object_at(SOURCE_PATH)
    object_hash = check_main(state, schemas[MAIN], registry, source)
    check_transports(schemas, state, registry, object_hash)
    check_negative_fixtures(schemas[MAIN], registry, state, source)
    print("Weaponry knife pass-state contracts OK: Main/prepare/get/result + exact lineage/evidence hashes + conservative gates + negatives")


if __name__ == "__main__":
    run_checks()
