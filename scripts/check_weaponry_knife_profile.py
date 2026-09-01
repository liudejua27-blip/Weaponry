#!/usr/bin/env python3
"""Focused contract gate for the closed Weaponry knife Tool profile.

The profile is deliberately a small, separate manifest.  It groups the current
MCP routes behind eleven public workflow façades without changing the legacy
226-route compatibility manifest.  This checker binds the profile to the current
source tool summary and keeps legacy replay explicit, including the one bounded
source-genesis bridge that is exposed natively and retained for raw replay.
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
PROFILE_ROOT = CONTRACT_ROOT / "profiles"
MANIFEST_PATH = CONTRACT_ROOT / "manifest.json"
PROFILE_SCHEMA_PATH = PROFILE_ROOT / "weaponry-knife-tool-profile.schema.json"
PROFILE_PATH = PROFILE_ROOT / "weaponry-knife-p0.json"
SOURCE_TOOL_SUMMARY_PATH = (
    ROOT / "docs" / "evidence" / "mcp010f" / "source-tool-manifest-summary.json"
)

sys.path.insert(0, str(ROOT / "scripts"))
from check_agentic_contracts import is_valid  # noqa: E402


FACADES = [
    "weapon_preflight",
    "reference_intake",
    "observe",
    "authoring_transaction",
    "surface_pipeline",
    "fps_presentation",
    "quality_review",
    "delivery",
    "approval",
    "recovery",
    "job",
]

NATIVE_OPERATION_NAMES = {
    "knife_reference_intent_bundle_prepare",
    "knife_reference_intent_bundle_get",
    "weaponry_knife_production_brief_prepare",
    "weaponry_knife_production_brief_get",
    "knife_curve_modifier_graph_prepare",
    "knife_curve_modifier_graph_get",
    "knife_curve_evaluated_mesh_prepare",
    "knife_curve_evaluated_mesh_get",
    "authoring_mesh_v2_candidate_materialize",
    "authoring_mesh_v2_high_bridge_prepare",
    "authoring_mesh_v2_high_bridge_get",
    "authoring_mesh_v2_high_artifact_prepare",
    "authoring_mesh_v2_high_artifact_get",
    "production_knife_uv_bake_v2_prepare",
    "production_knife_uv_bake_v2_get",
    "high_artifact_reference_compare_prepare",
    "knife_source_binding_prepare",
    "knife_source_binding_get",
    "production_weapon_authoring_mesh_v2_source_prepare",
    "knife_pass_state_get",
    "knife_pass_state_prepare",
    "weaponry_threejs_knife_design_get",
    "weaponry_threejs_knife_design_prepare",
    "weaponry_threejs_knife_design_execute",
    "weaponry_threejs_knife_comparison_get",
    "weaponry_threejs_knife_comparison_prepare",
}
NATIVE_COMPATIBILITY_OPERATION_NAMES = {
    "production_weapon_authoring_mesh_v2_source_prepare",
}
NATIVE_OPERATIONS_BY_FACADE = {
    "reference_intake": {
        "knife_reference_intent_bundle_prepare",
        "knife_reference_intent_bundle_get",
        "weaponry_knife_production_brief_prepare",
        "weaponry_knife_production_brief_get",
    },
    "authoring_transaction": NATIVE_OPERATION_NAMES
    - {
        "knife_reference_intent_bundle_prepare",
        "knife_reference_intent_bundle_get",
        "weaponry_knife_production_brief_prepare",
        "weaponry_knife_production_brief_get",
        "knife_pass_state_get",
        "knife_pass_state_prepare",
        "authoring_mesh_v2_high_artifact_get",
        "authoring_mesh_v2_high_artifact_prepare",
        "production_knife_uv_bake_v2_get",
        "production_knife_uv_bake_v2_prepare",
        "high_artifact_reference_compare_prepare",
    },
    "quality_review": {
        "high_artifact_reference_compare_prepare",
        "knife_pass_state_get",
        "knife_pass_state_prepare",
    },
    "surface_pipeline": {
        "authoring_mesh_v2_high_artifact_get",
        "authoring_mesh_v2_high_artifact_prepare",
        "production_knife_uv_bake_v2_get",
        "production_knife_uv_bake_v2_prepare",
    },
}

FORBIDDEN_PROPERTY_NAMES = {
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
}


def fail(message: str) -> None:
    raise SystemExit(f"Weaponry knife profile violation: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"cannot load {path.relative_to(ROOT)}: {exc}")


def load_object(path: Path) -> dict[str, Any]:
    value = load_json(path)
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


def canonical_hash(value: dict[str, Any]) -> str:
    payload = copy.deepcopy(value)
    payload["canonical_sha256"] = ""
    return hashlib.sha256(canonical_bytes(payload)).hexdigest()


def operation_hash(facade: dict[str, Any]) -> str:
    return hashlib.sha256(
        canonical_bytes(
            {
                "read_tools": facade["read_tools"],
                "write_tools": facade["write_tools"],
                "underlying_operations": facade["underlying_operations"],
            }
        )
    ).hexdigest()


def all_operation_hash(profile: dict[str, Any]) -> str:
    payload = {
        name: {
            "classification": profile["facades"][name]["classification"],
            "read_tools": profile["facades"][name]["read_tools"],
            "write_tools": profile["facades"][name]["write_tools"],
            "underlying_operations": profile["facades"][name]["underlying_operations"],
        }
        for name in FACADES
    }
    return hashlib.sha256(canonical_bytes(payload)).hexdigest()


def legacy_operation_hash(legacy: dict[str, Any]) -> str:
    return hashlib.sha256(
        canonical_bytes(
            {
                "read_tools": legacy["read_tools"],
                "write_tools": legacy["write_tools"],
            }
        )
    ).hexdigest()


def native_operation_hash(native: dict[str, Any]) -> str:
    return hashlib.sha256(canonical_bytes(native)).hexdigest()


def walk_property_names(node: Any) -> list[str]:
    if not isinstance(node, dict):
        return []
    names: list[str] = []
    properties = node.get("properties")
    if isinstance(properties, dict):
        names.extend(properties)
        for child in properties.values():
            names.extend(walk_property_names(child))
    for key in ("$defs", "items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"):
        child = node.get(key)
        if isinstance(child, list):
            for value in child:
                names.extend(walk_property_names(value))
        elif isinstance(child, dict):
            names.extend(walk_property_names(child))
    return names


def walk_values(node: Any, location: str = "$") -> list[tuple[str, str]]:
    found: list[tuple[str, str]] = []
    if isinstance(node, dict):
        for key, value in node.items():
            found.extend(walk_values(value, f"{location}.{key}"))
    elif isinstance(node, list):
        for index, value in enumerate(node):
            found.extend(walk_values(value, f"{location}[{index}]"))
    elif isinstance(node, str):
        found.append((location, node))
    return found


def check_schema_shape(schema: dict[str, Any]) -> None:
    require(
        schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema",
        "profile schema must use draft 2020-12",
    )
    require(
        schema.get("$id")
        == "https://forgecad.local/contracts/profiles/weaponry-knife-tool-profile.schema.json",
        "profile schema id drifted",
    )
    require(
        schema.get("type") == "object" and schema.get("additionalProperties") is False,
        "profile schema root must be closed",
    )
    require(
        schema.get("properties", {}).get("schema_version", {}).get("const")
        == "WeaponryKnifeToolProfile@1",
        "profile schema version drifted",
    )
    for name in walk_property_names(schema):
        require(
            name.lower() not in FORBIDDEN_PROPERTY_NAMES,
            f"profile schema exposes forbidden property {name}",
        )
    # The profile is a contract, but it is not one of the 583 Runtime data
    # schemas.  Every object in this separate manifest must nevertheless be
    # closed so future fields cannot silently widen the public surface.
    def inspect_objects(node: Any, location: str = "$") -> None:
        if not isinstance(node, dict):
            return
        if node.get("type") == "object":
            require(
                node.get("additionalProperties") is False,
                f"profile schema object is open at {location}",
            )
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


def check_profile_shape(
    profile: dict[str, Any],
    schema: dict[str, Any],
    source_summary: dict[str, Any],
) -> None:
    require(is_valid(schema, profile), "profile document is schema-invalid")
    require(
        profile.get("canonical_sha256") == canonical_hash(profile),
        "profile canonical_sha256 is stale",
    )
    require(profile.get("profile_id") == "weaponry-knife-p0@1", "profile id drifted")
    require(profile.get("profile_status") == "development-only", "profile must remain development-only")
    require(profile.get("product") == "weaponry", "profile product drifted")
    require(profile.get("subject") == "crossfire-knife", "profile subject drifted")

    facades = profile.get("facades")
    require(isinstance(facades, dict), "facades must be an object")
    require(list(facades) == FACADES, "facade names/order must be the exact 11-name public set")

    source_reads = source_summary.get("read_names")
    source_writes = source_summary.get("write_names")
    require(isinstance(source_reads, list), "current source read manifest is missing")
    require(isinstance(source_writes, list), "current source write manifest is missing")
    read_set = set(source_reads)
    write_set = set(source_writes)
    require(not read_set & write_set, "current source manifest overlaps read/write routes")
    require(
        source_summary.get("read_count") == len(source_reads)
        and source_summary.get("write_count") == len(source_writes)
        and source_summary.get("total_count") == len(read_set | write_set),
        "current source manifest counts are inconsistent",
    )

    legacy_operations = profile.get("legacy_operations")
    require(isinstance(legacy_operations, dict), "legacy_operations binding missing")
    require(
        legacy_operations.get("schema_version") == source_summary.get("schema_version")
        and legacy_operations.get("read_tools") == source_reads
        and legacy_operations.get("write_tools") == source_writes
        and legacy_operations.get("read_count") == source_summary.get("read_count")
        and legacy_operations.get("write_count") == source_summary.get("write_count")
        and legacy_operations.get("total_count") == source_summary.get("total_count")
        and legacy_operations.get("read_manifest_sha256") == source_summary.get("read_manifest_sha256")
        and legacy_operations.get("write_enabled_manifest_sha256") == source_summary.get("write_enabled_manifest_sha256")
        and legacy_operations.get("canonical_sha256") == source_summary.get("canonical_sha256"),
        "legacy_operations must bind every current source read/write route",
    )
    require(
        legacy_operations.get("operation_allowlist_sha256") == legacy_operation_hash(legacy_operations)
        and profile.get("legacy_operation_allowlist_sha256") == legacy_operations.get("operation_allowlist_sha256"),
        "legacy operation allowlist hash is stale",
    )

    operation_owners: dict[str, list[str]] = {}
    for name in FACADES:
        facade = facades[name]
        require(facade.get("facade_name") == name, f"{name} facade_name drifted")
        read_tools = facade.get("read_tools")
        write_tools = facade.get("write_tools")
        underlying = facade.get("underlying_operations")
        require(isinstance(read_tools, list), f"{name} read_tools missing")
        require(isinstance(write_tools, list), f"{name} write_tools missing")
        require(isinstance(underlying, list), f"{name} underlying_operations missing")
        require(not set(read_tools) & set(write_tools), f"{name} read/write routes overlap")
        require(set(read_tools) <= read_set, f"{name} contains a route absent from current read manifest")
        require(set(write_tools) <= write_set, f"{name} contains a route absent from current write manifest")
        facade_native = NATIVE_OPERATIONS_BY_FACADE.get(name, set())
        require(
            underlying == sorted(set(read_tools) | set(write_tools) | facade_native),
            f"{name} underlying operation allowlist is not the sorted read/write/native union",
        )
        require(
            facade.get("classification")
            == ("read-only" if not write_tools else "read-write"),
            f"{name} read/write classification drifted",
        )
        require(facade.get("default_enabled") is True, f"{name} is not default-enabled")
        require(
            facade.get("underlying_operation_allowlist_sha256") == operation_hash(facade),
            f"{name} operation allowlist hash is stale",
        )
        for operation in underlying:
            operation_owners.setdefault(operation, []).append(name)

    duplicate_owners = {
        operation: owners
        for operation, owners in operation_owners.items()
        if len(owners) != 1
    }
    require(
        not duplicate_owners,
        "every active operation must have exactly one public facade owner: "
        + json.dumps(duplicate_owners, sort_keys=True, separators=(",", ":")),
    )

    require(
        profile.get("underlying_operation_allowlist_sha256") == all_operation_hash(profile),
        "profile underlying operation allowlist hash is stale",
    )

    native_operations = profile.get("native_operations")
    expected_native = {
        "knife_reference_intent_bundle_prepare": {
            "operation_name": "knife_reference_intent_bundle_prepare",
            "classification": "write",
            "facade_name": "reference_intake",
            "request_schema": "KnifeReferenceIntentBundlePrepareRequest@1",
            "result_schema": "KnifeReferenceIntentBundleResult@1",
            "status": "native-development-only",
        },
        "knife_reference_intent_bundle_get": {
            "operation_name": "knife_reference_intent_bundle_get",
            "classification": "read",
            "facade_name": "reference_intake",
            "request_schema": "KnifeReferenceIntentBundleGetRequest@1",
            "result_schema": "KnifeReferenceIntentBundleResult@1",
            "status": "native-development-only",
        },
        "weaponry_knife_production_brief_prepare": {
            "operation_name": "weaponry_knife_production_brief_prepare",
            "classification": "write",
            "facade_name": "reference_intake",
            "request_schema": "WeaponryKnifeProductionBriefPrepareRequest@1",
            "result_schema": "WeaponryKnifeProductionBriefResult@1",
            "status": "native-development-only",
        },
        "weaponry_knife_production_brief_get": {
            "operation_name": "weaponry_knife_production_brief_get",
            "classification": "read",
            "facade_name": "reference_intake",
            "request_schema": "WeaponryKnifeProductionBriefGetRequest@1",
            "result_schema": "WeaponryKnifeProductionBriefResult@1",
            "status": "native-development-only",
        },
        "knife_curve_modifier_graph_prepare": {
            "operation_name": "knife_curve_modifier_graph_prepare",
            "classification": "write",
            "facade_name": "authoring_transaction",
            "request_schema": "KnifeCurveModifierGraphPrepareRequest@1",
            "result_schema": "KnifeCurveModifierGraphResult@1",
            "status": "native-development-only",
        },
        "knife_curve_modifier_graph_get": {
            "operation_name": "knife_curve_modifier_graph_get",
            "classification": "read",
            "facade_name": "authoring_transaction",
            "request_schema": "KnifeCurveModifierGraphGetRequest@1",
            "result_schema": "KnifeCurveModifierGraphResult@1",
            "status": "native-development-only",
        },
        "knife_curve_evaluated_mesh_prepare": {
            "operation_name": "knife_curve_evaluated_mesh_prepare",
            "classification": "write",
            "facade_name": "authoring_transaction",
            "request_schema": "KnifeCurveEvaluatedMeshPrepareRequest@1",
            "result_schema": "KnifeCurveEvaluatedMeshResult@1",
            "status": "native-development-only",
        },
        "authoring_mesh_v2_candidate_materialize": {
            "operation_name": "authoring_mesh_v2_candidate_materialize",
            "classification": "write",
            "facade_name": "authoring_transaction",
            "request_schema": "AuthoringMeshV2CandidateMaterializeRequest@1",
            "result_schema": "AuthoringMeshV2CandidateMaterializeResult@1",
            "status": "native-development-only",
        },
        "authoring_mesh_v2_high_bridge_prepare": {
            "operation_name": "authoring_mesh_v2_high_bridge_prepare",
            "classification": "write",
            "facade_name": "authoring_transaction",
            "request_schema": "AuthoringMeshV2HighBridgePrepareRequest@1",
            "result_schema": "AuthoringMeshV2HighBridgeResult@1",
            "status": "native-development-only",
        },
        "authoring_mesh_v2_high_bridge_get": {
            "operation_name": "authoring_mesh_v2_high_bridge_get",
            "classification": "read",
            "facade_name": "authoring_transaction",
            "request_schema": "AuthoringMeshV2HighBridgeGetRequest@1",
            "result_schema": "AuthoringMeshV2HighBridgeResult@1",
            "status": "native-development-only",
        },
        "knife_curve_evaluated_mesh_get": {
            "operation_name": "knife_curve_evaluated_mesh_get",
            "classification": "read",
            "facade_name": "authoring_transaction",
            "request_schema": "KnifeCurveEvaluatedMeshGetRequest@1",
            "result_schema": "KnifeCurveEvaluatedMeshResult@1",
            "status": "native-development-only",
        },
        "knife_source_binding_prepare": {
            "operation_name": "knife_source_binding_prepare",
            "classification": "write",
            "facade_name": "authoring_transaction",
            "request_schema": "KnifeSourceBindingPrepareRequest@1",
            "result_schema": "KnifeSourceBindingResult@1",
            "status": "native-development-only",
        },
        "knife_source_binding_get": {
            "operation_name": "knife_source_binding_get",
            "classification": "read",
            "facade_name": "authoring_transaction",
            "request_schema": "KnifeSourceBindingGetRequest@1",
            "result_schema": "KnifeSourceBindingResult@1",
            "status": "native-development-only",
        },
        "production_weapon_authoring_mesh_v2_source_prepare": {
            "operation_name": "production_weapon_authoring_mesh_v2_source_prepare",
            "classification": "write",
            "facade_name": "authoring_transaction",
            "request_schema": "ProductionWeaponAuthoringMeshV2SourcePrepareRequest@1",
            "result_schema": "ProductionWeaponAuthoringMeshV2SourcePrepareResult@1",
            "status": "native-development-only",
        },
        "weaponry_threejs_knife_design_execute": {
            "operation_name": "weaponry_threejs_knife_design_execute",
            "classification": "write",
            "facade_name": "authoring_transaction",
            "request_schema": "WeaponryThreeJsKnifeDesignExecuteRequest@1",
            "result_schema": "WeaponryThreeJsKnifeDesignExecutionResult@1",
            "status": "native-development-only",
        },
        "weaponry_threejs_knife_design_get": {
            "operation_name": "weaponry_threejs_knife_design_get",
            "classification": "read",
            "facade_name": "authoring_transaction",
            "request_schema": "WeaponryThreeJsKnifeDesignGetRequest@1",
            "result_schema": "WeaponryThreeJsKnifeDesignResult@1",
            "status": "native-development-only",
        },
        "weaponry_threejs_knife_design_prepare": {
            "operation_name": "weaponry_threejs_knife_design_prepare",
            "classification": "write",
            "facade_name": "authoring_transaction",
            "request_schema": "WeaponryThreeJsKnifeDesignPrepareRequest@1",
            "result_schema": "WeaponryThreeJsKnifeDesignResult@1",
            "status": "native-development-only",
        },
        "weaponry_threejs_knife_comparison_prepare": {
            "operation_name": "weaponry_threejs_knife_comparison_prepare",
            "classification": "write",
            "facade_name": "authoring_transaction",
            "request_schema": "WeaponryThreeJsKnifeComparisonPrepareRequest@1",
            "result_schema": "WeaponryThreeJsKnifeComparisonResult@1",
            "status": "native-development-only",
        },
        "weaponry_threejs_knife_comparison_get": {
            "operation_name": "weaponry_threejs_knife_comparison_get",
            "classification": "read",
            "facade_name": "authoring_transaction",
            "request_schema": "WeaponryThreeJsKnifeComparisonGetRequest@1",
            "result_schema": "WeaponryThreeJsKnifeComparisonResult@1",
            "status": "native-development-only",
        },
        "knife_pass_state_get": {
            "operation_name": "knife_pass_state_get",
            "classification": "read",
            "facade_name": "quality_review",
            "request_schema": "KnifePassStateGetRequest@1",
            "result_schema": "KnifePassStateResult@1",
            "status": "native-development-only",
        },
        "knife_pass_state_prepare": {
            "operation_name": "knife_pass_state_prepare",
            "classification": "write",
            "facade_name": "quality_review",
            "request_schema": "KnifePassStatePrepareRequest@1",
            "result_schema": "KnifePassStateResult@1",
            "status": "native-development-only",
        },
        "authoring_mesh_v2_high_artifact_prepare": {
            "operation_name": "authoring_mesh_v2_high_artifact_prepare",
            "classification": "write",
            "facade_name": "surface_pipeline",
            "request_schema": "AuthoringMeshV2HighArtifactPrepareRequest@1",
            "result_schema": "AuthoringMeshV2HighArtifactResult@1",
            "status": "native-development-only",
        },
        "authoring_mesh_v2_high_artifact_get": {
            "operation_name": "authoring_mesh_v2_high_artifact_get",
            "classification": "read",
            "facade_name": "surface_pipeline",
            "request_schema": "AuthoringMeshV2HighArtifactGetRequest@1",
            "result_schema": "AuthoringMeshV2HighArtifactResult@1",
            "status": "native-development-only",
        },
        "production_knife_uv_bake_v2_prepare": {
            "operation_name": "production_knife_uv_bake_v2_prepare",
            "classification": "write",
            "facade_name": "surface_pipeline",
            "request_schema": "WeaponryKnifeUvBakeV2PrepareRequest@1",
            "result_schema": "WeaponryKnifeUvBakeV2Result@1",
            "status": "native-development-only",
        },
        "production_knife_uv_bake_v2_get": {
            "operation_name": "production_knife_uv_bake_v2_get",
            "classification": "read",
            "facade_name": "surface_pipeline",
            "request_schema": "WeaponryKnifeUvBakeV2GetRequest@1",
            "result_schema": "WeaponryKnifeUvBakeV2Result@1",
            "status": "native-development-only",
        },
        "high_artifact_reference_compare_prepare": {
            "operation_name": "high_artifact_reference_compare_prepare",
            "classification": "write",
            "facade_name": "quality_review",
            "request_schema": "HighArtifactReferenceComparePrepareRequest@1",
            "result_schema": "HighArtifactReferenceComparisonPrepareResult@1",
            "status": "native-development-only",
        },
    }
    require(native_operations == expected_native, "native operation binding drifted")
    require(
        profile.get("native_operation_allowlist_sha256") == native_operation_hash(native_operations),
        "native operation allowlist hash is stale",
    )
    legacy_native_overlap = set(native_operations) & (read_set | write_set)
    require(
        legacy_native_overlap == NATIVE_COMPATIBILITY_OPERATION_NAMES
        and NATIVE_COMPATIBILITY_OPERATION_NAMES <= write_set,
        "native/legacy compatibility bridge drifted",
    )

    default_profile = profile.get("default_profile")
    require(isinstance(default_profile, dict), "default profile missing")
    require(default_profile.get("mode") == "default-knife", "default profile mode drifted")
    require(default_profile.get("public_surface") == "knife-facades", "default public surface drifted")
    require(default_profile.get("facade_names") == FACADES, "default profile must expose exactly the 11 façades")

    compatibility = profile.get("compatibility_profile")
    require(isinstance(compatibility, dict), "compatibility profile missing")
    require(
        compatibility.get("mode") == "explicit-compatibility-only",
        "compatibility profile must be explicit-only",
    )
    require(
        compatibility.get("public_surface") == "legacy-raw-tools",
        "compatibility profile must explicitly restore the legacy raw surface",
    )
    require(
        compatibility.get("facade_names") == [],
        "legacy compatibility must not masquerade as the 11-façade default surface",
    )
    legacy = compatibility.get("legacy_manifest")
    require(isinstance(legacy, dict), "legacy raw manifest binding missing")
    expected_legacy = {
        "schema_version": source_summary.get("schema_version"),
        "read_count": source_summary.get("read_count"),
        "write_count": source_summary.get("write_count"),
        "total_count": source_summary.get("total_count"),
        "read_manifest_sha256": source_summary.get("read_manifest_sha256"),
        "write_enabled_manifest_sha256": source_summary.get("write_enabled_manifest_sha256"),
        "canonical_sha256": source_summary.get("canonical_sha256"),
    }
    require(legacy == expected_legacy, "legacy compatibility is not bound to the current source manifest")
    require(
        compatibility.get("legacy_operation_allowlist_sha256")
        == legacy_operations.get("operation_allowlist_sha256"),
        "legacy compatibility operation binding is stale",
    )

    for property_name in walk_property_names(schema):
        require(
            property_name.lower() not in FORBIDDEN_PROPERTY_NAMES,
            f"profile schema exposes forbidden property {property_name}",
        )
    suspicious_value = re.compile(r"(?:https?|file)://|^/(?:[^/]|$)|-----BEGIN|sk-[A-Za-z0-9]")
    for location, value in walk_values(profile):
        require(
            suspicious_value.search(value) is None,
            f"profile contains an untyped external/sensitive value at {location}",
        )


def check_manifest(manifest: dict[str, Any]) -> None:
    profiles = manifest.get("profiles")
    require(isinstance(profiles, list) and len(profiles) == 1, "manifest must register one knife profile")
    entry = profiles[0]
    require(isinstance(entry, dict), "profile manifest entry must be an object")
    require(
        entry == {
            "profile_id": "weaponry-knife-p0@1",
            "schema": "profiles/weaponry-knife-tool-profile.schema.json",
            "document": "profiles/weaponry-knife-p0.json",
            "status": "development-only",
        },
        "manifest knife profile entry drifted",
    )


def expect_rejection(
    profile: dict[str, Any],
    schema: dict[str, Any],
    source_summary: dict[str, Any],
    mutate: Any,
    label: str,
) -> None:
    candidate = copy.deepcopy(profile)
    mutate(candidate)
    try:
        check_profile_shape(candidate, schema, source_summary)
    except SystemExit:
        return
    fail(f"negative profile case unexpectedly accepted: {label}")


def check_negative_cases(
    profile: dict[str, Any],
    schema: dict[str, Any],
    source_summary: dict[str, Any],
) -> None:
    def duplicate_route_with_valid_hashes(value: dict[str, Any]) -> None:
        facade = value["facades"]["reference_intake"]
        facade["read_tools"].append("project_get")
        facade["read_tools"].sort()
        facade["underlying_operations"].append("project_get")
        facade["underlying_operations"].sort()
        facade["underlying_operation_allowlist_sha256"] = operation_hash(facade)
        value["underlying_operation_allowlist_sha256"] = all_operation_hash(value)
        value["canonical_sha256"] = canonical_hash(value)

    expect_rejection(
        profile,
        schema,
        source_summary,
        lambda value: value["facades"]["observe"]["underlying_operations"].append("unknown_route"),
        "unknown underlying route",
    )
    expect_rejection(
        profile,
        schema,
        source_summary,
        lambda value: value["facades"]["weapon_preflight"].__setitem__("url", "https://invalid.example"),
        "forbidden URL field",
    )
    expect_rejection(
        profile,
        schema,
        source_summary,
        lambda value: value["facades"]["delivery"]["write_tools"].remove("export_prepare"),
        "read/write allowlist drift",
    )
    expect_rejection(
        profile,
        schema,
        source_summary,
        lambda value: value["compatibility_profile"].__setitem__("facade_names", FACADES),
        "legacy profile pretending to be façade surface",
    )
    expect_rejection(
        profile,
        schema,
        source_summary,
        lambda value: value["native_operations"]["knife_curve_modifier_graph_get"].__setitem__("classification", "write"),
        "native classification drift",
    )
    expect_rejection(
        profile,
        schema,
        source_summary,
        lambda value: value["legacy_operations"]["read_tools"].append("knife_curve_modifier_graph_get"),
        "native route leaking into legacy manifest",
    )
    expect_rejection(
        profile,
        schema,
        source_summary,
        duplicate_route_with_valid_hashes,
        "one operation owned by two public facades",
    )
    expect_rejection(
        profile,
        schema,
        source_summary,
        lambda value: value.__setitem__("canonical_sha256", "0" * 64),
        "stale canonical hash",
    )


def run_checks() -> None:
    manifest = load_object(MANIFEST_PATH)
    schema = load_object(PROFILE_SCHEMA_PATH)
    profile = load_object(PROFILE_PATH)
    source_summary = load_object(SOURCE_TOOL_SUMMARY_PATH)
    check_manifest(manifest)
    check_schema_shape(schema)
    check_profile_shape(profile, schema, source_summary)
    check_negative_cases(profile, schema, source_summary)


def main() -> int:
    run_checks()
    print(
        "Weaponry knife profile PASS: 11 exact façades, current read/write route binding, "
        "single facade ownership, legacy raw manifest binding, operation hashes, and negative cases"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
