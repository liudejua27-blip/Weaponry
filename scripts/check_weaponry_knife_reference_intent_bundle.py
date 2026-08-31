#!/usr/bin/env python3
"""Closed-contract gate for WPN-KNIFE-HIGH-001 Slice A.

This gate owns only the Contract slice.  It intentionally does not inspect or
write Runtime, Store or MCP implementation state.  The bundle is an
observation/quality input: an eligible immutable production brief and sealed
reference hashes are required, while route, exactness and unknown regions stay
explicit and independent.
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
FIXTURE_ROOT = CONTRACT_ROOT / "fixtures" / "weaponry-knife-reference-intent-bundle"
POSITIVE_PATH = FIXTURE_ROOT / "positive" / "dragonfang-reference-intent-bundle.json"
THRESHOLD_FIXTURE_PATH = FIXTURE_ROOT / "positive" / "quality-threshold-pending-policy.json"
BRIEF_PATH = CONTRACT_ROOT / "fixtures" / "weaponry-knife-production-brief" / "positive" / "dragonfang-kukri-brief-resolved-001.json"
PENDING_THRESHOLD_FIXTURE_SHA256 = "cc99e2c26c147b27e59bb8544334ec24adfbd83f4e723d0c21599ebaf7304b4b"
NEGATIVE_PATHS = [
    FIXTURE_ROOT / "negative" / "unknown-field.json",
    FIXTURE_ROOT / "negative" / "multiple-references.json",
    FIXTURE_ROOT / "negative" / "malformed-evidence-region-id.json",
    FIXTURE_ROOT / "negative" / "path-injection.json",
]

sys.path.insert(0, str(ROOT / "scripts"))
from check_agentic_contracts import is_valid, load_schema_registry  # noqa: E402

MAIN = "knife-reference-intent-bundle.schema.json"
PREPARE = "knife-reference-intent-bundle-prepare-request.schema.json"
GET = "knife-reference-intent-bundle-get-request.schema.json"
RESULT = "knife-reference-intent-bundle-result.schema.json"
SCHEMA_VERSION = "KnifeReferenceIntentBundle@1"
H = "0123456789abcdef" * 4
FORBIDDEN_NAMES = {
    "path", "url", "uri", "raw", "raw_bytes", "bytes", "secret", "token",
    "password", "api_key", "prompt", "script", "shell", "environment", "output",
}
SUSPICIOUS_VALUE = re.compile(
    r"(?:https?|file|data|ftp)://|^/(?:[^/]|$)|^[A-Za-z]:[/\\]|"
    r"(?:^|[/\\])\.\.?[/\\]|[Bb][Ll][Ee][Nn][Dd][Ee][Rr]\s+--[Pp][Yy][Tt][Hh]|"
    r"[Pp][Ll][Uu][Gg][Ii][Nn]|[Aa][Dd][Dd](?:-?[Oo][Nn])?\b|"
    r"[Pp][Aa][Ss][Ss][Ww][Oo][Rr][Dd]\s*[:=]|[Aa][Pp][Ii][-_]?[Kk][Ee][Yy]\s*[:=]|"
    r"[Ss][Ee][Cc][Rr][Ee][Tt]\s*[:=]|[Tt][Oo][Kk][Ee][Nn]\s*[:=]|"
    r"[Oo][Uu][Tt][Pp][Uu][Tt]\s*[:=]"
)
EXPECTED_DETAIL_IDS = {
    "kukri-spine-contour",
    "kukri-belly-contour",
    "kukri-tip-sweep",
    "cutting-edge-bevel",
    "blade-thickness-taper",
    "gold-spine-armor",
    "dragon-relief-scale-flow",
    "guard-dragon-muzzle",
    "guard-jaw-choil-negative-space",
    "guard-horns",
    "guard-eye-setting",
    "grip-curvature",
    "grip-segment-breaks",
    "grip-gold-border",
    "grip-fasteners-3-to-5",
    "pommel-hook",
    "controlled-edge-wear",
    "dragon-engraving",
}
EXPECTED_CRITICAL_FEATURE_IDS = {
    "feature-kukri-silhouette",
    "feature-blade-section",
    "feature-guard-attachment",
    "feature-guard-negative-space",
    "feature-grip-contour",
    "feature-pommel-hook",
    "feature-dragon-relief",
    "feature-material-lock",
    "feature-micro-engraving-lock",
}


def fail(message: str) -> None:
    raise SystemExit(f"Weaponry knife reference intent contract violation: {message}")


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


def object_hash(value: dict[str, Any], field: str = "canonical_sha256") -> str:
    payload = copy.deepcopy(value)
    payload[field] = ""
    return hashlib.sha256(canonical_bytes(payload)).hexdigest()


def walk(node: Any) -> list[dict[str, Any]]:
    if not isinstance(node, dict):
        return []
    found = [node] if node.get("type") == "object" else []
    for key, child in node.items():
        if key in {"properties", "$defs", "definitions"} and isinstance(child, dict):
            for value in child.values():
                found.extend(walk(value))
        elif key in {"items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"}:
            if isinstance(child, list):
                for value in child:
                    found.extend(walk(value))
            else:
                found.extend(walk(child))
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


def string_values(node: Any, location: str = "$") -> list[tuple[str, str]]:
    if isinstance(node, dict):
        values: list[tuple[str, str]] = []
        for key, value in node.items():
            values.extend(string_values(value, f"{location}.{key}"))
        return values
    if isinstance(node, list):
        values = []
        for index, value in enumerate(node):
            values.extend(string_values(value, f"{location}[{index}]"))
        return values
    if isinstance(node, str):
        return [(location, node)]
    return []


def check_schema_shell(schema: dict[str, Any], filename: str, title: str, version: str) -> None:
    require(schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema", f"{filename} draft drifted")
    require(schema.get("$id") == f"https://forgecad.local/contracts/{filename}", f"{filename} id drifted")
    require(schema.get("title") == title, f"{filename} title drifted")
    require(schema.get("type") == "object" and schema.get("additionalProperties") is False, f"{filename} root is open")
    require(schema.get("properties", {}).get("schema_version", {}).get("const") == version, f"{filename} version drifted")
    require("schema_version" in schema.get("required", []) and "canonical_sha256" in schema.get("required", []) or filename != MAIN, f"{filename} is not version/hash bound")
    for object_schema in walk(schema):
        require(object_schema.get("additionalProperties") is False, f"{filename} has an open object")
    require(not {name.lower() for name in property_names(schema)} & FORBIDDEN_NAMES, f"{filename} exposes a forbidden property")


def check_schemas(manifest: dict[str, Any], registry: dict[str, dict[str, Any]]) -> dict[str, dict[str, Any]]:
    expected = {
        MAIN: ("KnifeReferenceIntentBundle@1", SCHEMA_VERSION),
        PREPARE: ("KnifeReferenceIntentBundlePrepareRequest@1", "KnifeReferenceIntentBundlePrepareRequest@1"),
        GET: ("KnifeReferenceIntentBundleGetRequest@1", "KnifeReferenceIntentBundleGetRequest@1"),
        RESULT: ("KnifeReferenceIntentBundleResult@1", "KnifeReferenceIntentBundleResult@1"),
    }
    declared = set(manifest.get("schemas", []))
    require(set(expected) <= declared, "manifest does not register every intent bundle schema")
    schemas: dict[str, dict[str, Any]] = {}
    for filename, (title, version) in expected.items():
        schema = object_at(SCHEMA_ROOT / filename)
        check_schema_shell(schema, filename, title, version)
        schemas[filename] = schema
    require(registry.get(schemas[MAIN].get("$id")) == schemas[MAIN], "main schema is not registry-bound")
    require(set(schemas[MAIN]["properties"]["route"]["enum"]) == {"reference-projection", "authored-texture", "procedural-finish"}, "route enum drifted")
    require(set(schemas[MAIN]["properties"]["exactness"]["enum"]) == {"image-only", "metadata-assisted", "exact-texture"}, "exactness enum drifted")
    intake_records = schemas[MAIN]["$defs"]["intake_manifest"]["properties"]["records"]
    require(intake_records.get("minItems") == 1 and intake_records.get("maxItems") == 1, "KnifeIntakeManifest@1 must be single-reference closed")
    quality_properties = schemas[MAIN]["$defs"]["quality_contract"]["properties"]
    require(quality_properties["threshold_fixture_sha256"].get("const") == PENDING_THRESHOLD_FIXTURE_SHA256, "quality threshold hash is not locked to the checked-in pending fixture")
    require(quality_properties["threshold_status"].get("const") == "CALIBRATION_PENDING", "quality threshold status is not locked pending")
    unknown_schema = schemas[MAIN]["$defs"]["unknown"]
    require("view" in unknown_schema.get("required", []), "unknown view binding is not required")
    evidence_items = schemas[MAIN]["$defs"]["critical_feature"]["properties"]["evidence_region_ids"]["items"]
    require(evidence_items.get("$ref") == "#/$defs/evidence_region_id", "critical feature evidence IDs are not bound to the closed region-ID definition")
    require(schemas[MAIN]["$defs"]["evidence_region_id"].get("pattern", "").endswith("(front|back|left|right|front-three-quarter|rear-three-quarter|top|bottom|fps-hold|fps-inspect)$"), "evidence region ID does not require a recognized view suffix")
    return schemas


def check_values_are_safe(value: Any) -> None:
    for location, text in string_values(value):
        require(not SUSPICIOUS_VALUE.search(text), f"suspicious locator/script/secret/output value at {location}")


def load_brief_targets() -> tuple[set[str], set[str]]:
    brief = object_at(BRIEF_PATH)
    require(brief.get("schema_version") == "WeaponryKnifeProductionBrief@1", "eligible brief fixture schema version drifted")
    parts = brief.get("parts")
    zones = brief.get("material_zones")
    require(isinstance(parts, list) and isinstance(zones, list), "eligible brief fixture has no parts/material zones")
    part_ids = {part.get("part_id") for part in parts if isinstance(part, dict)}
    zone_ids = {zone.get("zone_id") for zone in zones if isinstance(zone, dict)}
    require(None not in part_ids and None not in zone_ids, "eligible brief fixture contains an unidentifiable target")
    return part_ids, zone_ids


def check_detail_target_mappings(
    details: list[dict[str, Any]], part_ids: set[str], zone_ids: set[str]
) -> None:
    """Keep every typed detail target inside the eligible Brief vocabulary.

    ``edge-role`` and ``surface-finish`` are semantic roles, not a second
    namespace.  They must resolve to a real Brief part/material zone before a
    Runtime producer can use them.  Critical features use the same explicit
    part-or-zone union; they must not smuggle arbitrary labels into the source
    binding.
    """
    for detail in details:
        target = detail["target"]
        kind = target["target_kind"]
        target_id = target["target_id"]
        if kind == "part":
            require(target_id in part_ids, f"detail part target is not declared by eligible brief: {target_id}")
        elif kind == "material-zone":
            require(target_id in zone_ids, f"detail material target is not declared by eligible brief: {target_id}")
        elif kind == "edge-role":
            require(target["mapping_status"] == "mapped", f"edge-role target is not a mapped Brief part: {target_id}")
            require(target_id in part_ids, f"edge-role target is not a Brief part: {target_id}")
        elif kind == "surface-finish":
            require(target["mapping_status"] == "mapped", f"surface-finish target is not a mapped Brief material zone: {target_id}")
            require(target_id in zone_ids, f"surface-finish target is not a Brief material zone: {target_id}")


def check_main(bundle: dict[str, Any], schema: dict[str, Any], registry: dict[str, dict[str, Any]]) -> None:
    require(is_valid(schema, bundle, registry), "positive bundle is schema-invalid")
    check_values_are_safe(bundle)
    require(bundle["canonical_sha256"] == object_hash(bundle), "bundle canonical hash is stale")
    brief = bundle["brief_binding"]
    brief_fixture = object_at(BRIEF_PATH)
    reference = bundle["reference_binding"]
    require(brief["brief_schema_version"] == "WeaponryKnifeProductionBrief@1", "brief schema binding drifted")
    require(brief["authoring_eligibility"] == "ELIGIBLE", "bundle is not bound to an eligible brief")
    require(brief["authorization_binding_status"] == "runtime-bound", "brief authorization is not Runtime-bound")
    part_ids, zone_ids = load_brief_targets()
    require(brief["brief_id"] == brief_fixture["brief_id"], "bundle is bound to a different production brief fixture")
    require(reference["binding_status"] == "runtime-bound", "reference binding is not Runtime-bound")
    require(bundle["route"] != bundle["exactness"], "route and exactness were conflated")

    intake = bundle["intake_manifest"]
    require(intake["canonical_sha256"] == object_hash(intake), "intake manifest canonical hash is stale")
    records = intake["records"]
    require(len(records) == 1, "KnifeIntakeManifest@1 must contain exactly one Runtime-bound primary record")
    record_ids = {record["reference_id"] for record in records}
    require(len(record_ids) == len(records), "intake reference IDs are not unique")
    primary = [record for record in records if record["reference_id"] == reference["reference_id"]]
    require(len(primary) == 1 and primary[0]["role"] == "primary", "reference binding has no exact primary intake record")
    require(primary[0]["reference_object_sha256"] == reference["reference_object_sha256"], "reference object hash drifted")
    require(primary[0]["reference_evidence_sha256"] == reference["reference_evidence_sha256"], "reference evidence hash drifted")
    require(all(record["decode_status"] == "decoded" and record["duplicate_status"] == "unique" and record["admission_status"] == "admitted" for record in records), "intake record was silently admitted without decode/duplicate gates")
    for record in records:
        coverage = {item["view"] for item in record["visible_coverage"]}
        require(len(coverage) == len(record["visible_coverage"]), "visible coverage contains duplicate views")

    inventory = bundle["detail_inventory"]
    require(inventory["canonical_sha256"] == object_hash(inventory), "detail inventory canonical hash is stale")
    details = inventory["details"]
    detail_ids = {detail["detail_id"] for detail in details}
    require(len(detail_ids) == len(details), "detail IDs are not unique")
    require(len(details) == 18 and detail_ids == EXPECTED_DETAIL_IDS, "Dragonfang detail inventory must contain the locked 18-detail coverage set")
    require(any(detail["target"]["target_kind"] == "part" and detail["target"]["mapping_status"] == "mapped" for detail in details), "detail inventory has no mapped real part feature")
    require(any(detail["target"]["target_kind"] == "material-zone" for detail in details), "detail inventory has no material-local target")
    require(any(detail["observation_status"] == "unknown" for detail in details), "detail inventory erased unknown observation state")
    supplied_views = {item["view"] for item in primary[0]["visible_coverage"] if item["status"] == "observed"}
    families = {detail["family"] for detail in details}
    require({"silhouette", "cross-section", "attachment", "negative-space", "identity", "surface", "wear"} <= families, "detail inventory does not cover all required feature families")
    check_detail_target_mappings(details, part_ids, zone_ids)
    for detail in details:
        target = detail["target"]
        require(all(region["reference_id"] == reference["reference_id"] for region in detail["evidence_regions"]), f"detail {detail['detail_id']} introduced a second reference")
        require(all(region["view"] in supplied_views for region in detail["evidence_regions"]), f"detail {detail['detail_id']} cites an unobserved view")
        if detail["scale"] == "micro" and detail["family"] in {"surface", "wear"}:
            require(detail["high_action"] != "geometry", f"micro material detail is not locked: {detail['detail_id']}")
    later_stage_details = {"controlled-edge-wear", "dragon-engraving"}
    for detail in details:
        if detail["detail_id"] in later_stage_details:
            require(detail["scale"] == "micro", f"later-stage detail scale drifted: {detail['detail_id']}")
            require(detail["observation_status"] in {"unknown", "inferred"}, f"later-stage detail was overclaimed: {detail['detail_id']}")
            require(detail["high_action"] in {"material-override", "later-normal-bake"}, f"later-stage detail action drifted: {detail['detail_id']}")

    quality = bundle["quality_contract"]
    require(quality["canonical_sha256"] == object_hash(quality), "quality contract canonical hash is stale")
    require(quality["stage_order"] == ["camera-lock", "silhouette-blockout", "structural-form", "secondary-form", "high-geometry"], "High stage order drifted")
    require(quality["threshold_status"] == "CALIBRATION_PENDING", "uncalibrated threshold fixture was promoted")
    threshold_fixture = object_at(THRESHOLD_FIXTURE_PATH)
    require(threshold_fixture["schema_version"] == "KnifeQualityThresholdCalibrationFixture@1", "threshold fixture schema version drifted")
    require(threshold_fixture["status"] == "CALIBRATION_PENDING" and threshold_fixture["calibration_required"] is True, "threshold fixture is not an explicit pending-policy fixture")
    require(threshold_fixture["canonical_sha256"] == object_hash(threshold_fixture), "threshold fixture canonical hash is stale")
    require(quality["threshold_fixture_sha256"] == threshold_fixture["canonical_sha256"], "quality contract threshold fixture binding drifted")
    require(quality["promotion_state"] == "HIGH_LOCKED_UNTIL_CALIBRATED_AND_REVIEWED@1", "High promotion lock weakened")
    policy = quality["correction_policy"]
    require(policy == {"max_iterations_per_pass": 3, "max_iterations_total": 6, "one_changed_scope_per_iteration": True, "baseline_preserved": True}, "correction policy drifted")
    critical_features = quality["critical_features"]
    critical_ids = {feature["feature_id"] for feature in critical_features}
    require(EXPECTED_CRITICAL_FEATURE_IDS <= critical_ids, "quality contract is missing a required critical feature lock")
    critical_target_union = part_ids | zone_ids
    require(
        all(feature["target_id"] in critical_target_union for feature in critical_features),
        "quality critical feature target must be a declared Brief part or material zone",
    )
    critical_kinds = {feature["feature_kind"] for feature in critical_features}
    require({"silhouette", "cross-section", "attachment", "negative-space", "identity", "material"} <= critical_kinds, "quality critical features do not cover silhouette, blade section, guard, grip/pommel, relief and material locks")
    critical_by_id = {feature["feature_id"]: feature for feature in critical_features}
    require(critical_by_id["feature-blade-section"]["target_id"] == "blade-body", "blade-section critical target drifted")
    require(critical_by_id["feature-guard-attachment"]["target_id"] == "guard-dragon-head", "guard attachment critical target drifted")
    require(critical_by_id["feature-guard-negative-space"]["target_id"] == "guard-dragon-head", "guard negative-space critical target drifted")
    require(critical_by_id["feature-grip-contour"]["target_id"] == "grip" and critical_by_id["feature-pommel-hook"]["target_id"] == "pommel", "grip/pommel critical target drifted")
    require(critical_by_id["feature-dragon-relief"]["target_id"] == "dragon-relief", "dragon relief critical target drifted")
    require(
        critical_by_id["feature-micro-engraving-lock"]["feature_kind"] == "identity"
        and critical_by_id["feature-micro-engraving-lock"]["target_id"] == "antique-gold-ornament",
        "micro-engraving identity must remain bound to the legal Brief material-zone union",
    )
    for feature_id in ("feature-material-lock", "feature-micro-engraving-lock"):
        require(critical_by_id[feature_id]["source_status"] == "unknown" and not critical_by_id[feature_id]["blocking"], f"locked material/micro feature was promoted: {feature_id}")
    require(any(view["comparison_role"] == "primary-reference" and view["reference_required"] for view in quality["fixed_views"]), "quality contract has no primary reference view")
    orbit_views = [view for view in quality["fixed_views"] if view["comparison_role"] == "orbit-nonreference" and not view["reference_required"]]
    require(len(orbit_views) >= 2, "quality contract needs at least two non-reference orbit views")
    require(len({view["view"] for view in orbit_views}) == len(orbit_views), "quality contract orbit views are degenerate duplicates")
    require(all(failure["blocks_promotion"] for failure in quality["blocking_failures"]), "a blocking failure does not block promotion")

    unknowns = bundle["unknowns"]
    unknown_ids = {item["unknown_id"] for item in unknowns}
    require(len(unknown_ids) == len(unknowns), "unknown IDs are not unique")
    require(all(item["resolution_status"] == "open" for item in unknowns), "unknowns were silently resolved")
    missing_views = set(brief_fixture["reference_coverage"]["missing_views"])
    reference_unknowns = [item for item in unknowns if item["topic"] == "reference-view"]
    require(len(unknowns) == len(missing_views), "Dragonfang unknown count does not match eligible Brief missing_views")
    require(len(reference_unknowns) == len(missing_views), "reference-view unknown count does not match eligible Brief missing_views")
    require({item["view"] for item in reference_unknowns} == missing_views, "unknown reference views do not exactly preserve eligible Brief missing_views")
    require(all(item["view"] is not None and item["impact"] == "blocking" for item in reference_unknowns), "missing reference view was not retained as a blocking, view-bound unknown")


def prepare_fixture(bundle: dict[str, Any]) -> dict[str, Any]:
    binding = bundle["brief_binding"]
    ref = bundle["reference_binding"]
    value = {
        "schema_version": "KnifeReferenceIntentBundlePrepareRequest@1",
        "operation": "knife_reference_intent_bundle_prepare",
        "project_id": bundle["project_id"],
        "brief_id": binding["brief_id"],
        "brief_sha256": binding["brief_sha256"],
        "brief_object_sha256": binding["brief_object_sha256"],
        "reference_id": ref["reference_id"],
        "reference_object_sha256": ref["reference_object_sha256"],
        "reference_evidence_sha256": ref["reference_evidence_sha256"],
        "brief_authoring_eligibility": "ELIGIBLE",
        "intent_bundle": bundle,
        "idempotency_key": "dragonfang-intent-prepare-001",
        "max_response_bytes": 1048576,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    }
    value["input_sha256"] = object_hash(value, "input_sha256")
    return value


def get_fixture(bundle: dict[str, Any]) -> dict[str, Any]:
    binding = bundle["brief_binding"]
    ref = bundle["reference_binding"]
    value = {
        "schema_version": "KnifeReferenceIntentBundleGetRequest@1",
        "operation": "knife_reference_intent_bundle_get",
        "project_id": bundle["project_id"],
        "brief_id": binding["brief_id"],
        "brief_sha256": binding["brief_sha256"],
        "brief_object_sha256": binding["brief_object_sha256"],
        "reference_id": ref["reference_id"],
        "reference_object_sha256": ref["reference_object_sha256"],
        "reference_evidence_sha256": ref["reference_evidence_sha256"],
        "brief_authoring_eligibility": "ELIGIBLE",
        "intent_bundle_id": bundle["intent_bundle_id"],
        "intent_bundle_sha256": bundle["canonical_sha256"],
        "intent_bundle_object_sha256": H,
        "max_response_bytes": 1048576,
        "runtime_write_performed": False,
        "persistent_user_data_touched": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
        "input_sha256": "",
    }
    value["input_sha256"] = object_hash(value, "input_sha256")
    return value


def result_fixture(bundle: dict[str, Any], request_kind: str, status: str) -> dict[str, Any]:
    binding = bundle["brief_binding"]
    ref = bundle["reference_binding"]
    prepare = request_kind == "prepare"
    value = {
        "schema_version": "KnifeReferenceIntentBundleResult@1",
        "operation": "knife_reference_intent_bundle_prepare" if prepare else "knife_reference_intent_bundle_get",
        "request_kind": request_kind,
        "status": status,
        "project_id": bundle["project_id"],
        "brief_id": binding["brief_id"],
        "brief_sha256": binding["brief_sha256"],
        "brief_object_sha256": binding["brief_object_sha256"],
        "brief_authoring_eligibility": "ELIGIBLE",
        "reference_id": ref["reference_id"],
        "reference_object_sha256": ref["reference_object_sha256"],
        "reference_evidence_sha256": ref["reference_evidence_sha256"],
        "intent_bundle_id": bundle["intent_bundle_id"],
        "intent_bundle_sha256": bundle["canonical_sha256"],
        "intent_bundle_object_sha256": H,
        "intent_bundle": bundle,
        "idempotency_key": None if not prepare else "dragonfang-intent-prepare-001",
        "replayed": status == "replayed",
        "store_effect": "inserted" if status == "stored" else "not-touched",
        "cas_effect": "inserted" if status == "stored" else "not-touched",
        "runtime_write_performed": status == "stored",
        "persistent_user_data_touched": status == "stored",
        "production_stage_advanced": False,
        "candidate_confirmed": False,
        "version_created": False,
        "export_performed": False,
        "high_mesh_created": False,
        "high_stage_unlocked": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
        "canonical_sha256": "",
    }
    value["canonical_sha256"] = object_hash(value)
    return value


def check_transports(schemas: dict[str, dict[str, Any]], bundle: dict[str, Any], registry: dict[str, dict[str, Any]]) -> None:
    prepare = prepare_fixture(bundle)
    get = get_fixture(bundle)
    require(is_valid(schemas[PREPARE], prepare, registry), "prepare fixture is schema-invalid")
    require(is_valid(schemas[GET], get, registry), "get fixture is schema-invalid")
    for request in (prepare, get):
        check_values_are_safe(request)
        require(request["input_sha256"] == object_hash(request, "input_sha256"), "transport input hash is stale")
    for kind, status in (("prepare", "stored"), ("prepare", "replayed"), ("get", "found")):
        result = result_fixture(bundle, kind, status)
        require(is_valid(schemas[RESULT], result, registry), f"{kind}/{status} result fixture is schema-invalid")
        check_values_are_safe(result)
        require(result["canonical_sha256"] == object_hash(result), f"{kind}/{status} result canonical hash is stale")


def check_negative_fixtures(schema: dict[str, Any], registry: dict[str, dict[str, Any]], bundle: dict[str, Any]) -> None:
    for path in NEGATIVE_PATHS:
        value = object_at(path)
        require(not is_valid(schema, value, registry), f"negative fixture unexpectedly passed: {path.name}")
    mutations = []
    value = copy.deepcopy(bundle)
    value["output"] = "forbidden"
    mutations.append(("root output field", value))
    value = copy.deepcopy(bundle)
    value["detail_inventory"]["details"][0]["label"] = "output: /tmp/unsafe"
    mutations.append(("output injection", value))
    value = copy.deepcopy(bundle)
    value["route"] = "file:///unsafe"
    mutations.append(("route locator", value))
    value = copy.deepcopy(bundle)
    value["brief_binding"]["authoring_eligibility"] = "BLOCKED"
    mutations.append(("ineligible brief", value))
    value = copy.deepcopy(bundle)
    secondary = copy.deepcopy(value["intake_manifest"]["records"][0])
    secondary["reference_id"] = "reference-secondary"
    secondary["reference_object_sha256"] = "5" * 64
    secondary["reference_evidence_sha256"] = "6" * 64
    secondary["role"] = "secondary"
    value["intake_manifest"]["records"].append(secondary)
    mutations.append(("multiple intake references", value))
    value = copy.deepcopy(bundle)
    value["unknowns"][0]["view"] = None
    mutations.append(("reference unknown without view", value))
    value = copy.deepcopy(bundle)
    value["unknowns"][0]["topic"] = "other"
    mutations.append(("non-reference unknown with view", value))
    value = copy.deepcopy(bundle)
    value["quality_contract"]["threshold_fixture_sha256"] = "a" * 64
    mutations.append(("random threshold fixture hash", value))
    value = copy.deepcopy(bundle)
    value["quality_contract"]["threshold_status"] = "CALIBRATED"
    mutations.append(("caller-asserted calibrated threshold", value))
    value = copy.deepcopy(bundle)
    value["quality_contract"]["critical_features"][0]["evidence_region_ids"] = ["malformed-region-id"]
    mutations.append(("malformed evidence region ID", value))
    value = copy.deepcopy(bundle)
    value["intent_bundle_id"] = "intent:colon"
    mutations.append(("colon in Runtime opaque identifier", value))
    for label, mutated in mutations:
        require(not is_valid(schema, mutated, registry), f"negative mutation unexpectedly passed: {label}")

    part_ids, zone_ids = load_brief_targets()
    value = copy.deepcopy(bundle)
    value["detail_inventory"]["details"][0]["target"] = {
        "target_kind": "edge-role", "target_id": "antique-gold-ornament", "mapping_status": "mapped"
    }
    try:
        check_detail_target_mappings(value["detail_inventory"]["details"], part_ids, zone_ids)
    except SystemExit:
        pass
    else:
        fail("edge-role target outside Brief part union unexpectedly passed")
    value = copy.deepcopy(bundle)
    value["detail_inventory"]["details"][0]["target"] = {
        "target_kind": "surface-finish", "target_id": "blade", "mapping_status": "mapped"
    }
    try:
        check_detail_target_mappings(value["detail_inventory"]["details"], part_ids, zone_ids)
    except SystemExit:
        pass
    else:
        fail("surface-finish target outside Brief material-zone union unexpectedly passed")


def run_checks() -> None:
    manifest = object_at(MANIFEST_PATH)
    registry = load_schema_registry(manifest)
    schemas = check_schemas(manifest, registry)
    bundle = object_at(POSITIVE_PATH)
    check_main(bundle, schemas[MAIN], registry)
    check_transports(schemas, bundle, registry)
    check_negative_fixtures(schemas[MAIN], registry, bundle)
    print("Weaponry knife reference intent contracts OK: main/prepare/get/result + positive/negative fixtures")


if __name__ == "__main__":
    run_checks()
