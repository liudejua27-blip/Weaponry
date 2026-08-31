#!/usr/bin/env python3
"""Closed-contract gate for the Weaponry knife production brief.

The brief is an immutable intake/design truth that may be stored by Runtime;
it never creates geometry, advances Stage, confirms a candidate, versions or
exports.  This checker keeps the source conflict ledger explicit and verifies
that fixtures and transport contracts contain only hashes and sanitized
claims, never source bytes or identity artifacts from a reference board.
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
SCHEMA_PATH = CONTRACT_ROOT / "schemas" / "weaponry-knife-production-brief.schema.json"
PREPARE_SCHEMA_PATH = CONTRACT_ROOT / "schemas" / "weaponry-knife-production-brief-prepare-request.schema.json"
GET_SCHEMA_PATH = CONTRACT_ROOT / "schemas" / "weaponry-knife-production-brief-get-request.schema.json"
RESULT_SCHEMA_PATH = CONTRACT_ROOT / "schemas" / "weaponry-knife-production-brief-result.schema.json"
FIXTURE_ROOT = CONTRACT_ROOT / "fixtures" / "weaponry-knife-production-brief"
POSITIVE_PATH = FIXTURE_ROOT / "positive" / "dragonfang-kukri-brief.json"
RESOLVED_SUCCESSOR_PATH = FIXTURE_ROOT / "positive" / "dragonfang-kukri-brief-resolved-001.json"
GENERIC_POSITIVE_PATH = FIXTURE_ROOT / "positive" / "generic-resolved-original-control.json"
NEGATIVE_PATH = FIXTURE_ROOT / "negative" / "unknown-field.json"
MANIFEST_PATH = CONTRACT_ROOT / "manifest.json"

sys.path.insert(0, str(ROOT / "scripts"))
from check_agentic_contracts import is_valid  # noqa: E402


SCHEMA_VERSION = "WeaponryKnifeProductionBrief@1"
SCHEMA_FILENAME = "weaponry-knife-production-brief.schema.json"
TRANSPORT_SCHEMA_FILENAMES = {
    "weaponry-knife-production-brief-prepare-request.schema.json",
    "weaponry-knife-production-brief-get-request.schema.json",
    "weaponry-knife-production-brief-result.schema.json",
}
IMAGE_SHA256 = "932c5ec407249678f69d1a9d61daa8f59177bf54766695e30ec3d2bbef00bf7e"
TEXT_SHA256 = "7e7c7087e47e121b2ef3808176dcbbda066550ea75ecd18d5895b16171d122e8"
CONTROL_SHA256 = "a" * 64
REQUIRED_VIEWS = [
    "front",
    "back",
    "left",
    "right",
    "front-three-quarter",
    "rear-three-quarter",
    "top",
    "bottom",
    "fps-hold",
    "fps-inspect",
]
EXPECTED_CONFLICT_KINDS = {
    "identity-label",
    "hero-triangle-budget",
    "texture-resolution",
    "engine-profile",
}
EXPECTED_PRIORITIES = [
    "kukri-core-silhouette",
    "dragon-head-guard",
    "blade-proportion",
    "dragon-relief",
    "pbr-material",
    "controlled-wear",
    "micro-detail",
]
EXPECTED_PARTS = {
    "blade",
    "cutting-edge",
    "blade-body",
    "dragon-relief",
    "guard-dragon-head",
    "dragon-eye-left",
    "dragon-eye-right",
    "grip",
    "grip-fastener",
    "gem",
    "pommel",
}
EXPECTED_ZONES = {
    "dark-red-blade",
    "antique-gold-ornament",
    "black-grip",
    "silver-cutting-edge",
    "ruby-gem",
}
EXPECTED_GATES = [
    "K0_AUTH_REFERENCE",
    "K1_AUTHORING",
    "K2_FORM",
    "K3_GRAPH_HIGH",
    "K4_LOW",
    "K5_HERO_UV",
    "K6_CAGE_BAKE",
    "K7_MATERIAL",
    "K8_FPS_LOD",
    "K9_ENGINE",
    "K10_HUMAN_EXPORT",
]
FORBIDDEN_KEYS = {
    "path",
    "url",
    "uri",
    "raw",
    "raw_bytes",
    "bytes",
    "contact",
    "signature",
    "email",
    "phone",
    "logo",
    "trademark",
    "api_key",
    "secret",
    "token",
    "password",
    "prompt",
    "script",
    "shell",
    "environment",
}
SUSPICIOUS_VALUE = re.compile(
    r"(?:https?|file|data|ftp)://|^/(?:[^/]|$)|^[A-Za-z]:[/\\]|"
    r"(?:^|[/\\])\.\.?[/\\]|[Bb][Ll][Ee][Nn][Dd][Ee][Rr]\s+--[Pp][Yy][Tt][Hh]\b|"
    r"[Pp][Ll][Uu][Gg][Ii][Nn]|[Aa][Dd][Dd](?:-?[Oo][Nn])?\b|"
    r"-----BEGIN|[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}|"
    r"\+?\d[\d ()-]{7,}\d"
)


def fail(message: str) -> None:
    raise SystemExit(f"Weaponry knife production brief violation: {message}")


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


def canonical_hash(value: dict[str, Any]) -> str:
    payload = copy.deepcopy(value)
    payload["canonical_sha256"] = ""
    return hashlib.sha256(
        json.dumps(
            payload,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    ).hexdigest()


def walk_schema(node: Any) -> list[dict[str, Any]]:
    if not isinstance(node, dict):
        return []
    found = [node] if node.get("type") == "object" else []
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
    for key in ("$defs", "items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"):
        child = node.get(key)
        if isinstance(child, list):
            for value in child:
                names.extend(walk_property_names(value))
        elif isinstance(child, dict):
            names.extend(walk_property_names(child))
    return names


def walk_values(node: Any, location: str = "$") -> list[tuple[str, str]]:
    if isinstance(node, dict):
        values: list[tuple[str, str]] = []
        for key, value in node.items():
            values.extend(walk_values(value, f"{location}.{key}"))
        return values
    if isinstance(node, list):
        values = []
        for index, value in enumerate(node):
            values.extend(walk_values(value, f"{location}[{index}]"))
        return values
    if isinstance(node, str):
        return [(location, node)]
    return []


def check_schema(schema: dict[str, Any], manifest: dict[str, Any]) -> None:
    require(schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema", "schema draft drifted")
    require(schema.get("$id") == f"https://forgecad.local/contracts/{SCHEMA_FILENAME}", "schema id drifted")
    require(schema.get("title") == "WeaponryKnifeProductionBrief", "schema title drifted")
    require(schema.get("type") == "object" and schema.get("additionalProperties") is False, "root is not closed")
    require(schema.get("properties", {}).get("schema_version", {}).get("const") == SCHEMA_VERSION, "schema version drifted")
    required = set(schema.get("required", []))
    require({"schema_version", "canonical_sha256", "parent_brief_id", "parent_brief_sha256", "freeze_policy"} <= required, "schema is not hash and parent-lineage bound")
    for object_schema in walk_schema(schema):
        require(object_schema.get("additionalProperties") is False, "nested object is open")
    forbidden = {key.lower() for key in FORBIDDEN_KEYS}
    for name in walk_property_names(schema):
        require(name.lower() not in forbidden, f"schema exposes forbidden property {name}")
    declared = manifest.get("schemas", [])
    require(SCHEMA_FILENAME in declared, "manifest does not register the brief schema")


def check_transport_schemas(manifest: dict[str, Any]) -> None:
    declared = set(manifest.get("schemas", []))
    require(TRANSPORT_SCHEMA_FILENAMES <= declared, "manifest does not register every brief transport schema")
    expected = [
        (
            PREPARE_SCHEMA_PATH,
            "WeaponryKnifeProductionBriefPrepareRequest@1",
            "weaponry_knife_production_brief_prepare",
        ),
        (
            GET_SCHEMA_PATH,
            "WeaponryKnifeProductionBriefGetRequest@1",
            "weaponry_knife_production_brief_get",
        ),
        (RESULT_SCHEMA_PATH, "WeaponryKnifeProductionBriefResult@1", None),
    ]
    for path, title, operation in expected:
        schema = load_object(path)
        filename = path.name
        require(schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema", f"{filename} draft drifted")
        require(schema.get("$id") == f"https://forgecad.local/contracts/{filename}", f"{filename} id drifted")
        require(schema.get("title") == title, f"{filename} title drifted")
        require(schema.get("type") == "object" and schema.get("additionalProperties") is False, f"{filename} root is not closed")
        for object_schema in walk_schema(schema):
            require(object_schema.get("additionalProperties") is False, f"{filename} contains an open object")
        property_names = {name.lower() for name in walk_property_names(schema)}
        require(not property_names & {name.lower() for name in FORBIDDEN_KEYS}, f"{filename} exposes forbidden transport property")
        if operation is not None:
            require(schema.get("properties", {}).get("operation", {}).get("const") == operation, f"{filename} operation drifted")
            require(schema.get("properties", {}).get("max_response_bytes", {}).get("const") == 1048576, f"{filename} response budget drifted")
            require(schema.get("properties", {}).get("runtime_write_performed", {}).get("const") is False, f"{filename} caller pretends Runtime already wrote")
    result = load_object(RESULT_SCHEMA_PATH)
    required = set(result.get("required", []))
    require({"reference_id", "reference_object_sha256", "reference_evidence_sha256", "brief_sha256", "brief_object_sha256", "parent_brief_id", "parent_brief_sha256", "freeze_policy", "authoring_eligibility", "production_stage_advanced", "candidate_confirmed", "version_created", "export_performed"} <= required, "result omits reference, parent-lineage, durable or no-promotion truth")
    require(result["properties"]["production_stage_advanced"].get("const") is False, "brief result may advance Stage")
    require(result["properties"]["candidate_confirmed"].get("const") is False, "brief result may confirm a candidate")
    require(result["properties"]["version_created"].get("const") is False, "brief result may create a version")
    require(result["properties"]["export_performed"].get("const") is False, "brief result may export")


def check_cross_field_invariants(brief: dict[str, Any]) -> None:
    require(brief["parent_brief_id"] is None and brief["parent_brief_sha256"] is None, "Dragonfang initial intake unexpectedly has a parent")
    require(brief["freeze_policy"] == "initial-intake-no-parent@1", "Dragonfang initial freeze policy drifted")
    require(brief["authorization"]["source_reference_sha256"] == brief["reference_coverage"]["source_reference_sha256"], "authorization/reference hash drifted")
    require(brief["reference_coverage"]["source_reference_sha256"] == IMAGE_SHA256, "positive fixture source hash drifted")

    identity = brief["asset_identity"]
    identity_claims = identity["identity_claims"]
    identity_by_id = {claim["claim_id"]: claim for claim in identity_claims}
    require(len(identity_by_id) == len(identity_claims), "identity claim IDs are not unique")
    require(sorted(identity["source_labels"]) == sorted(claim["label"] for claim in identity_claims), "source labels do not match identity claims")
    require(all(claim["evidence_sha256"] == (IMAGE_SHA256 if claim["source_kind"] == "image-panel" else TEXT_SHA256) for claim in identity_claims), "identity claim evidence hash drifted")

    coverage = brief["reference_coverage"]
    require(coverage["required_views"] == REQUIRED_VIEWS, "required reference view contract drifted")
    supplied = set(coverage["supplied_views"])
    missing = set(coverage["missing_views"])
    require(not supplied & missing, "supplied and missing reference views overlap")
    require(supplied | missing == set(REQUIRED_VIEWS), "reference coverage does not partition required views")
    require(coverage["coverage_status"] == "partial" and coverage["hq_360_status"] == "BLOCKED_REFERENCE_COVERAGE", "partial reference coverage was promoted")

    priorities = brief["silhouette_priorities"]
    require([item["rank"] for item in priorities] == list(range(1, 8)), "silhouette priority ranks are not ordered")
    require([item["focus"] for item in priorities] == EXPECTED_PRIORITIES, "silhouette priority order drifted")

    parts = brief["parts"]
    part_ids = [part["part_id"] for part in parts]
    require(set(part_ids) == EXPECTED_PARTS and len(part_ids) == len(EXPECTED_PARTS), "semantic part inventory drifted")
    zone_ids = {zone["zone_id"] for zone in brief["material_zones"]}
    require(zone_ids == EXPECTED_ZONES, "material-zone inventory drifted")
    require(all(set(part["material_zone_ids"]) <= zone_ids for part in parts), "part references an unknown material zone")
    require(all(part["parent_id"] is None or part["parent_id"] in set(part_ids) for part in parts), "part references an unknown parent")
    require(set(brief["presentation_constraints"]["inspect_focus_order"]) <= set(part_ids), "Dragonfang FPS inspect focus references an unknown part")
    require(sum(zone["target_share_percent"] for zone in brief["material_zones"]) == 100, "material-zone target shares do not total 100")
    ruby = next(zone for zone in brief["material_zones"] if zone["zone_id"] == "ruby-gem")
    require(ruby["emissive_allowed"] is True and "emissive" in ruby["channels"], "ruby emissive policy drifted")
    require(all(zone["emissive_allowed"] is False and "emissive" not in zone["channels"] for zone in brief["material_zones"] if zone["zone_id"] != "ruby-gem"), "emissive leaked to a non-ruby zone")

    surface = brief["surface_constraints"]
    hero = surface["hero_budget"]
    require(hero["status"] == "conflicted" and hero["resolved_min_triangles"] is None and hero["resolved_max_triangles"] is None, "hero budget was silently resolved")
    hero_claims = {claim["claim_id"]: claim for claim in hero["claims"]}
    require(set(hero_claims) == {"hero-image-exact-7836", "hero-text-range-25k-45k", "hero-text-fallback-15k-30k"}, "hero budget claims drifted")
    require(hero_claims["hero-image-exact-7836"]["min_triangles"] == 7836 and hero_claims["hero-image-exact-7836"]["max_triangles"] == 7836, "image Hero triangle claim drifted")
    require(surface["lod_levels"] == [{"level_id": "lod0", "target_percent": 100}, {"level_id": "lod1", "target_percent": 50}, {"level_id": "lod2", "target_percent": 25}, {"level_id": "lod3", "target_percent": 12.5}], "LOD targets drifted")
    texture = surface["texture_policy"]
    require(
        texture["resolved_width"] is None
        and texture["resolved_height"] is None
        and texture["shipping_width"] is None
        and texture["shipping_height"] is None
        and texture["resolution_status"] == "conflicted",
        "texture resolution was silently resolved",
    )
    claims = {(claim["width"], claim["height"], claim["usage"]) for claim in texture["resolution_claims"]}
    require((2048, 2048, "unspecified") in claims and (4096, 4096, "hero") in claims and (2048, 2048, "production") in claims, "texture claims drifted")

    engine = brief["engine_constraints"]
    require(engine["preferred_engine"] is None and engine["profile_status"] == "conflicted", "engine profile was silently selected")
    engine_claims = {claim["claim_id"]: claim for claim in engine["target_claims"]}
    require(set(engine_claims) == {"engine-image-ambiguous-532", "engine-text-unreal-56", "engine-text-unity-6"}, "engine claims drifted")
    require(engine_claims["engine-image-ambiguous-532"]["version_requirement"] == "5.3.2" and engine_claims["engine-image-ambiguous-532"]["confidence"] == "ambiguous", "ambiguous image engine claim drifted")
    require(engine["unit_status"] == "unresolved" and engine["axis_status"] == "unresolved", "engine coordinate policy was silently resolved")

    conflicts = brief["source_conflicts"]
    conflict_by_kind = {item["kind"]: item for item in conflicts}
    require(set(conflict_by_kind) == EXPECTED_CONFLICT_KINDS and len(conflicts) == len(EXPECTED_CONFLICT_KINDS), "source conflict ledger drifted")
    all_claim_ids = set(identity_by_id) | set(hero_claims) | {claim["claim_id"] for claim in texture["resolution_claims"]} | set(engine_claims)
    for conflict in conflicts:
        require(conflict["resolution_status"] == "unresolved" and conflict["blocking"] is True, f"conflict {conflict['conflict_id']} is not blocking")
        require(set(conflict["observed_claim_ids"]) <= all_claim_ids, f"conflict {conflict['conflict_id']} references an unknown claim")

    acceptance = brief["acceptance_constraints"]
    require(acceptance["status"] == "blocked" and acceptance["required_gates"] == EXPECTED_GATES, "acceptance gate contract drifted")
    statuses = {item["gate_id"]: item["status"] for item in acceptance["gate_statuses"]}
    require(list(statuses) == EXPECTED_GATES, "acceptance gate order drifted")
    require(statuses["K0_AUTH_REFERENCE"] == "blocked" and statuses["K9_ENGINE"] == "blocked" and statuses["K10_HUMAN_EXPORT"] == "blocked", "blocking gates were promoted")
    require(acceptance["runtime_sole_writer"] and acceptance["prototype_not_truth"] and acceptance["human_artist_required"] and acceptance["user_approval_required"], "approval boundary weakened")


def successor_source_claims(brief: dict[str, Any]) -> dict[str, Any]:
    """Return the exact source-claim objects that a successor must preserve."""
    claims: dict[str, Any] = {}
    claims.update({item["claim_id"]: item for item in brief["asset_identity"]["identity_claims"]})
    claims.update({item["claim_id"]: item for item in brief["surface_constraints"]["hero_budget"]["claims"]})
    claims.update({item["claim_id"]: item for item in brief["surface_constraints"]["texture_policy"]["resolution_claims"]})
    claims.update({item["claim_id"]: item for item in brief["engine_constraints"]["target_claims"]})
    return claims


def successor_conflict_identity(brief: dict[str, Any]) -> list[dict[str, Any]]:
    """Return conflict identity only; resolution fields are intentionally omitted."""
    return [
        {
            "conflict_id": item["conflict_id"],
            "kind": item["kind"],
            "observed_claim_ids": item["observed_claim_ids"],
        }
        for item in brief["source_conflicts"]
    ]


def successor_immutable_projection(brief: dict[str, Any]) -> dict[str, Any]:
    """Mirror Runtime's allowlisted successor mutation projection."""
    value = copy.deepcopy(brief)
    for field in [
        "brief_id",
        "parent_brief_id",
        "parent_brief_sha256",
        "freeze_policy",
        "authorization",
        "reference_coverage",
        "source_conflicts",
        "canonical_sha256",
        "created_at",
    ]:
        value[field] = None
    value["asset_identity"]["name_status"] = None
    value["asset_identity"]["selected_label"] = None
    hero = value["surface_constraints"]["hero_budget"]
    for field in ["status", "resolved_min_triangles", "resolved_max_triangles", "blocks"]:
        hero[field] = None
    texture = value["surface_constraints"]["texture_policy"]
    for field in ["resolution_status", "resolved_width", "resolved_height", "shipping_width", "shipping_height"]:
        texture[field] = None
    engine = value["engine_constraints"]
    for field in [
        "profile_status",
        "preferred_engine",
        "preferred_engine_version",
        "unit_status",
        "selected_unit",
        "axis_status",
        "selected_axis_profile",
    ]:
        engine[field] = None
    acceptance = value["acceptance_constraints"]
    # Gate truth is resolution-owned: a successor may remove source-conflict
    # blockers while retaining the required gate set and policy booleans.
    for field in ["status", "gate_statuses", "blocking_reasons"]:
        acceptance[field] = None
    return value


def check_dragonfang_resolved_successor(
    parent: dict[str, Any], successor: dict[str, Any], *, runtime_bound_probe: bool = False
) -> None:
    """Check the frozen resolution or its in-memory live-binding probe shape."""
    require(parent["brief_id"] == "dragonfang-kukri-brief", "Dragonfang parent brief ID drifted")
    require(parent["canonical_sha256"] == "27c8893f4ff982ff90ad62b21df18e00b30db691b8d9d169ddb8cbff2235d514", "Dragonfang parent canonical hash drifted")
    require(successor["project_id"] == parent["project_id"], "successor changed project scope")
    require(successor["brief_id"] != parent["brief_id"], "successor brief ID is not new")
    require(successor["parent_brief_id"] == parent["brief_id"], "successor parent ID is not exact")
    require(successor["parent_brief_sha256"] == parent["canonical_sha256"], "successor parent hash is not exact")
    require(successor["freeze_policy"] == "immutable-successor-preserve-source-claims@1", "successor freeze policy drifted")
    require(successor["canonical_sha256"] == canonical_hash(successor), "successor canonical hash is stale")
    require(successor["brief_id"] == "dragonfang-kukri-brief-resolved-001", "successor identity drifted")

    # The parent source claims and conflict identity are immutable.  Resolution
    # status, selected values, authorization state, and coverage are separate.
    require(successor_source_claims(successor) == successor_source_claims(parent), "successor changed source claim objects")
    require(successor_conflict_identity(successor) == successor_conflict_identity(parent), "successor changed conflict identity")
    require(successor_immutable_projection(successor) == successor_immutable_projection(parent), "successor changed a non-allowlisted field")
    require(successor["reference_coverage"] == parent["reference_coverage"], "successor silently changed reference coverage")
    require(successor["authorization"]["source_reference_sha256"] == parent["authorization"]["source_reference_sha256"], "successor authorization source hash drifted")

    identity = successor["asset_identity"]
    require(identity["working_asset_id"] == "dragonfang-kukri", "working asset ID drifted")
    require(identity["name_status"] == "resolved", "successor identity is not resolved")
    require(identity["selected_label"] == "尼泊尔-屠龙", "formal Chinese name was not frozen")
    require(identity["source_labels"] == parent["asset_identity"]["source_labels"], "resolved successor dropped source labels")
    require(len(identity["source_labels"]) == 3, "resolved successor must preserve all three source labels")

    authorization = successor["authorization"]
    require(authorization["status"] == "user-confirmed", "successor authorization status was not user-confirmed")
    expected_evidence = "runtime-bound" if runtime_bound_probe else "source-asserted-not-runtime-bound"
    require(authorization["evidence_status"] == expected_evidence, "successor authorization binding status drifted")
    require(authorization["user_confirmation_required"] is False, "user-confirmed successor retained a confirmation prompt")

    hero = successor["surface_constraints"]["hero_budget"]
    require(hero["status"] == "resolved", "Hero budget is not resolved")
    require(hero["resolved_min_triangles"] == 25000 and hero["resolved_max_triangles"] == 45000, "Hero budget does not match 25k-45k freeze")
    require(hero["blocks"] == [], "resolved Hero budget retains blockers")
    require(any(claim["claim_id"] == "hero-text-range-25k-45k" and claim["min_triangles"] == 25000 and claim["max_triangles"] == 45000 for claim in hero["claims"]), "resolved Hero budget dropped the retained 25k-45k claim")

    texture = successor["surface_constraints"]["texture_policy"]
    require(texture["resolution_status"] == "resolved", "texture policy is not resolved")
    require(texture["resolved_width"] == 4096 and texture["resolved_height"] == 4096, "4K authoring master was not selected")
    require(texture["shipping_width"] == 2048 and texture["shipping_height"] == 2048, "2K shipping texture was not selected")
    require(any(claim["claim_id"] == "texture-text-hero-4096" and claim["width"] == 4096 and claim["height"] == 4096 and claim["usage"] == "hero" for claim in texture["resolution_claims"]), "4K authoring claim was dropped")
    require(any(claim["claim_id"] == "texture-text-production-2048" and claim["width"] == 2048 and claim["height"] == 2048 and claim["usage"] == "production" for claim in texture["resolution_claims"]), "2K shipping claim was dropped")
    require(
        any(
            claim["usage"] == "hero"
            and claim["width"] == texture["resolved_width"]
            and claim["height"] == texture["resolved_height"]
            for claim in texture["resolution_claims"]
        ),
        "authoring master does not match a retained hero claim",
    )
    require(
        any(
            claim["usage"] == "production"
            and claim["width"] == texture["shipping_width"]
            and claim["height"] == texture["shipping_height"]
            for claim in texture["resolution_claims"]
        ),
        "shipping resolution does not match a retained production claim",
    )

    engine = successor["engine_constraints"]
    require(engine["profile_status"] == "resolved" and engine["preferred_engine"] == "unreal", "Unreal target was not selected")
    require(engine["preferred_engine_version"] == "5.6-or-later", "Unreal 5.6-or-later version was not selected")
    require(engine["unit_status"] == "resolved" and engine["axis_status"] == "resolved", "engine unit/axis resolution was not frozen")
    require(engine["selected_unit"] == "centimeter" and engine["selected_unit"] in engine["unit_options"], "centimeter unit selection is not retained in engine options")
    require(engine["selected_axis_profile"] == "unreal-z-up-x-forward-right-handed", "Unreal axis profile was not selected")
    require(any(claim["claim_id"] == "engine-text-unreal-56" and claim["engine_family"] == "unreal" and claim["version_requirement"] == "5.6-or-later" for claim in engine["target_claims"]), "Unreal 5.6 claim was dropped")

    conflicts = successor["source_conflicts"]
    require(len(conflicts) == len(EXPECTED_CONFLICT_KINDS), "successor conflict count drifted")
    require(all(item["resolution_status"] == "resolved" and item["blocking"] is False for item in conflicts), "resolved successor retained a blocking conflict")

    acceptance = successor["acceptance_constraints"]
    require(acceptance["status"] == "blocked", "successor acceptance was promoted")
    expected_blockers = [
        "missing-reference-views",
        "engine-validation-not-run",
        "independent-human-review-missing",
    ] if runtime_bound_probe else [
        "authorization-not-runtime-bound",
        "missing-reference-views",
        "engine-validation-not-run",
        "independent-human-review-missing",
    ]
    require(
        acceptance["blocking_reasons"] == expected_blockers,
        "successor acceptance blockers do not reflect remaining gates",
    )
    statuses = {item["gate_id"]: item["status"] for item in acceptance["gate_statuses"]}
    require(statuses["K0_AUTH_REFERENCE"] == ("pass" if runtime_bound_probe else "blocked"), "authorization gate status is inconsistent with binding")
    require(statuses["K1_AUTHORING"] == "not-run", "authoring gate was fabricated")
    require(statuses["K9_ENGINE"] == "blocked" and statuses["K10_HUMAN_EXPORT"] == "blocked", "remaining engine/human gates were promoted")
    require(acceptance["required_gates"] == parent["acceptance_constraints"]["required_gates"], "successor changed required acceptance gates")
    require(acceptance["promotion_labels"] == parent["acceptance_constraints"]["promotion_labels"], "successor changed acceptance promotion labels")
    require(acceptance["runtime_sole_writer"] and acceptance["prototype_not_truth"] and acceptance["human_artist_required"] and acceptance["user_approval_required"], "successor approval boundary weakened")


def check_generic_resolved_probe(brief: dict[str, Any]) -> None:
    """Prove the contract remains reusable after the Dragonfang fixture is resolved."""
    require(brief["parent_brief_id"] is None and brief["parent_brief_sha256"] is None, "generic initial control unexpectedly has a parent")
    require(brief["freeze_policy"] == "initial-intake-no-parent@1", "generic initial freeze policy drifted")
    require(brief["subject"] == "original-control-knife", "generic control probe subject drifted")
    identity = brief["asset_identity"]
    require(identity["name_status"] == "resolved" and identity["selected_label"] is not None, "resolved name state is inconsistent")
    require(len(identity["source_labels"]) == 1, "resolved identity must select one source label")
    identity_claims = identity["identity_claims"]
    identity_claim_ids = {claim["claim_id"] for claim in identity_claims}
    require(len(identity_claim_ids) == len(identity_claims), "generic identity claim IDs are not unique")
    require(identity["source_labels"] == [claim["label"] for claim in identity_claims], "generic source labels do not match claims")

    coverage = brief["reference_coverage"]
    required_views = set(coverage["required_views"])
    require(coverage["coverage_status"] == "complete", "complete generic coverage was not retained")
    require(set(coverage["supplied_views"]) == required_views, "generic coverage does not include every required view")
    require(coverage["missing_views"] == [] and coverage["hq_360_status"] == "eligible", "complete coverage did not become HQ360 eligible")

    parts = brief["parts"]
    part_ids = [part["part_id"] for part in parts]
    require(len(part_ids) == len(set(part_ids)), "duplicate part IDs were accepted")
    zones = brief["material_zones"]
    zone_ids = [zone["zone_id"] for zone in zones]
    zone_id_set = set(zone_ids)
    require(len(zone_ids) == len(zone_id_set), "duplicate material zone IDs were accepted")
    for part in parts:
        require(part["parent_id"] is None or part["parent_id"] in set(part_ids), f"part {part['part_id']} references an unknown parent")
        require(set(part["material_zone_ids"]) <= zone_id_set, f"part {part['part_id']} references an unknown material zone")
    require(sum(zone["target_share_percent"] for zone in zones) == 100, "generic material-zone shares do not total 100")
    for zone in zones:
        interval = zone["roughness_range"]
        require(interval["min"] <= interval["max"], f"material range for {zone['zone_id']} is inverted")

    presentation = brief["presentation_constraints"]
    require(set(presentation["inspect_focus_order"]) <= set(part_ids), "generic FPS focus references an unknown part")
    require(presentation["inspect_focus_order"], "generic FPS focus order is empty")

    surface = brief["surface_constraints"]
    hero = surface["hero_budget"]
    require(hero["status"] == "resolved", "generic hero budget was not resolved")
    require(hero["resolved_min_triangles"] <= hero["resolved_max_triangles"], "generic hero triangle range is inverted")
    require(hero["claims"] and hero["blocks"] == [], "resolved hero budget retained blockers")
    lod_levels = surface["lod_levels"]
    lod_ids = [level["level_id"] for level in lod_levels]
    require(len(lod_ids) == len(set(lod_ids)), "duplicate LOD IDs were accepted")
    lod_targets = [level["target_percent"] for level in lod_levels]
    require(lod_targets[0] == 100 and all(previous >= current for previous, current in zip(lod_targets, lod_targets[1:])), "LOD targets are not a monotonic reduction from hero")
    texture = surface["texture_policy"]
    require(
        texture["resolution_status"] == "resolved"
        and texture["resolved_width"]
        and texture["resolved_height"]
        and texture["shipping_width"]
        and texture["shipping_height"],
        "generic texture resolution was not resolved",
    )
    require(texture["resolution_claims"], "resolved texture policy has no claim")
    require(
        any(
            claim["usage"] == "production"
            and claim["width"] == texture["shipping_width"]
            and claim["height"] == texture["shipping_height"]
            for claim in texture["resolution_claims"]
        ),
        "generic shipping resolution does not match a retained production claim",
    )

    engine = brief["engine_constraints"]
    require(engine["profile_status"] == "resolved" and engine["preferred_engine"] is not None, "generic engine profile was not resolved")
    require(engine["preferred_engine_version"] == "6-or-later", "generic engine version selection drifted")
    require(engine["selected_unit"] == "meter" and engine["selected_unit"] in engine["unit_options"], "generic engine unit selection is inconsistent")
    require(engine["selected_axis_profile"] == "generic-meter-axis", "generic engine axis selection drifted")
    require(engine["target_claims"], "resolved engine profile has no claim")

    conflicts = brief["source_conflicts"]
    require(any(conflict["resolution_status"] == "resolved" and conflict["blocking"] is False for conflict in conflicts), "resolved non-blocking conflict record is missing")
    all_claim_ids = identity_claim_ids | {claim["claim_id"] for claim in hero["claims"]} | {claim["claim_id"] for claim in texture["resolution_claims"]} | {claim["claim_id"] for claim in engine["target_claims"]}
    for conflict in conflicts:
        require(set(conflict["observed_claim_ids"]) <= all_claim_ids, f"conflict {conflict['conflict_id']} references an unknown claim")
        require(conflict["blocking"] is (conflict["resolution_status"] == "unresolved"), f"conflict {conflict['conflict_id']} has inconsistent blocking state")

    acceptance = brief["acceptance_constraints"]
    required_gates = acceptance["required_gates"]
    statuses = acceptance["gate_statuses"]
    status_ids = [item["gate_id"] for item in statuses]
    require(status_ids == required_gates and len(status_ids) == len(set(status_ids)), "acceptance gate dependency/order is inconsistent")
    if acceptance["status"] == "ready":
        require(acceptance["blocking_reasons"] == [] and all(item["status"] == "pass" for item in statuses), "ready acceptance has a failing or unrun dependency")
    if acceptance["status"] == "blocked":
        require(acceptance["blocking_reasons"] and any(item["status"] in {"blocked", "fail"} for item in statuses), "blocked acceptance has no blocking dependency")


def check_sanitized_values(brief: dict[str, Any]) -> None:
    forbidden = {key.lower() for key in FORBIDDEN_KEYS}

    def walk(node: Any, location: str = "$") -> None:
        if isinstance(node, dict):
            for key, value in node.items():
                require(key.lower() not in forbidden, f"forbidden key {key} at {location}")
                if key.lower().endswith("_sha256"):
                    require(
                        (key == "parent_brief_sha256" and value is None)
                        or (isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None),
                        f"invalid hash at {location}.{key}",
                    )
                elif key.lower().endswith("_at"):
                    require(isinstance(value, str) and re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", value) is not None, f"invalid timestamp at {location}.{key}")
                else:
                    walk(value, f"{location}.{key}")
        elif isinstance(node, list):
            for index, value in enumerate(node):
                walk(value, f"{location}[{index}]")
        elif isinstance(node, str):
            require(SUSPICIOUS_VALUE.search(node) is None, f"suspicious value at {location}")

    walk(brief)


def run_checks() -> None:
    schema = load_object(SCHEMA_PATH)
    manifest = load_object(MANIFEST_PATH)
    positive = load_object(POSITIVE_PATH)
    resolved_successor = load_object(RESOLVED_SUCCESSOR_PATH)
    generic = load_object(GENERIC_POSITIVE_PATH)
    negative = load_object(NEGATIVE_PATH)
    check_schema(schema, manifest)
    check_transport_schemas(manifest)
    require(is_valid(schema, positive), "positive fixture is schema-invalid")
    require(positive["canonical_sha256"] == canonical_hash(positive), "positive canonical hash is stale")
    require(is_valid(schema, resolved_successor), "resolved successor fixture is schema-invalid")
    require(resolved_successor["canonical_sha256"] == canonical_hash(resolved_successor), "resolved successor canonical hash is stale")
    require(is_valid(schema, generic), "generic resolved probe is schema-invalid")
    require(generic["canonical_sha256"] == canonical_hash(generic), "generic probe canonical hash is stale")
    require(not is_valid(schema, negative), "negative fixture unexpectedly validates")

    unknown = copy.deepcopy(positive)
    unknown["unexpected_field"] = "closed"
    require(not is_valid(schema, unknown), "top-level unknown field was accepted")
    conflict = copy.deepcopy(positive)
    conflict["source_conflicts"][0]["resolution_status"] = "resolved"
    require(not is_valid(schema, conflict), "resolved conflict bypassed the closed brief")
    conflict["source_conflicts"][0]["resolution_status"] = "unresolved"
    conflict["canonical_sha256"] = canonical_hash(conflict)
    require(is_valid(schema, conflict), "canonical hash mutation made a valid brief invalid")

    invalid_initial_parent = copy.deepcopy(positive)
    invalid_initial_parent["parent_brief_id"] = "older-brief"
    invalid_initial_parent["parent_brief_sha256"] = "b" * 64
    require(not is_valid(schema, invalid_initial_parent), "initial intake accepted a parent")

    invalid_successor_parent = copy.deepcopy(positive)
    invalid_successor_parent["freeze_policy"] = "immutable-successor-preserve-source-claims@1"
    require(not is_valid(schema, invalid_successor_parent), "successor freeze accepted a missing parent")

    structural_successor = copy.deepcopy(generic)
    structural_successor["brief_id"] = "original-control-knife-brief-v2"
    structural_successor["parent_brief_id"] = generic["brief_id"]
    structural_successor["parent_brief_sha256"] = generic["canonical_sha256"]
    structural_successor["freeze_policy"] = "immutable-successor-preserve-source-claims@1"
    structural_successor["canonical_sha256"] = canonical_hash(structural_successor)
    require(is_valid(schema, structural_successor), "well-formed immutable successor was rejected")

    check_dragonfang_resolved_successor(positive, resolved_successor)

    # A resolved successor must not discard an alternate source label merely
    # because a selected display name was frozen.
    dropped_source_label = copy.deepcopy(resolved_successor)
    dropped_source_label["asset_identity"]["source_labels"] = ["尼泊尔-屠龙"]
    dropped_source_label["canonical_sha256"] = canonical_hash(dropped_source_label)
    require(is_valid(schema, dropped_source_label), "source-label preservation negative did not reach cross-field checking")
    try:
        check_dragonfang_resolved_successor(positive, dropped_source_label)
    except SystemExit:
        pass
    else:
        fail("resolved successor accepted a dropped source label")

    # A user-confirmed authorization must not carry the unresolved confirmation
    # prompt, even when the canonical hash is otherwise valid.
    confirmation_mismatch = copy.deepcopy(resolved_successor)
    confirmation_mismatch["authorization"]["user_confirmation_required"] = True
    confirmation_mismatch["canonical_sha256"] = canonical_hash(confirmation_mismatch)
    require(not is_valid(schema, confirmation_mismatch), "user-confirmed authorization accepted a confirmation prompt")

    # Resolved engine status is not enough: the concrete unit selection is part
    # of the frozen target and must be present in the closed schema.
    engine_selection_missing = copy.deepcopy(resolved_successor)
    engine_selection_missing["engine_constraints"]["selected_unit"] = None
    engine_selection_missing["canonical_sha256"] = canonical_hash(engine_selection_missing)
    require(not is_valid(schema, engine_selection_missing), "resolved engine accepted a missing selected unit")

    shipping_resolution_mismatch = copy.deepcopy(resolved_successor)
    shipping_resolution_mismatch["surface_constraints"]["texture_policy"]["shipping_width"] = 4096
    shipping_resolution_mismatch["canonical_sha256"] = canonical_hash(shipping_resolution_mismatch)
    require(is_valid(schema, shipping_resolution_mismatch), "shipping-resolution negative did not reach cross-field checking")
    try:
        check_dragonfang_resolved_successor(positive, shipping_resolution_mismatch)
    except SystemExit:
        pass
    else:
        fail("resolved successor accepted a shipping size without a retained production claim")

    shipping_resolution_missing = copy.deepcopy(resolved_successor)
    shipping_resolution_missing["surface_constraints"]["texture_policy"]["shipping_width"] = None
    shipping_resolution_missing["canonical_sha256"] = canonical_hash(shipping_resolution_missing)
    require(not is_valid(schema, shipping_resolution_missing), "resolved texture accepted a missing shipping width")

    # The live probe may bind authorization in memory.  It must promote only
    # K0 and remove only the authorization blocker; this is not persisted here.
    runtime_bound_probe = copy.deepcopy(resolved_successor)
    runtime_bound_probe["authorization"]["evidence_status"] = "runtime-bound"
    runtime_bound_probe["acceptance_constraints"]["gate_statuses"][0]["status"] = "pass"
    runtime_bound_probe["acceptance_constraints"]["blocking_reasons"] = [
        "missing-reference-views",
        "engine-validation-not-run",
        "independent-human-review-missing",
    ]
    runtime_bound_probe["canonical_sha256"] = canonical_hash(runtime_bound_probe)
    require(is_valid(schema, runtime_bound_probe), "runtime-bound probe template is schema-invalid")
    check_dragonfang_resolved_successor(positive, runtime_bound_probe, runtime_bound_probe=True)

    inconsistent_runtime_probe = copy.deepcopy(runtime_bound_probe)
    inconsistent_runtime_probe["acceptance_constraints"]["gate_statuses"][0]["status"] = "blocked"
    inconsistent_runtime_probe["canonical_sha256"] = canonical_hash(inconsistent_runtime_probe)
    require(is_valid(schema, inconsistent_runtime_probe), "runtime-bound consistency negative did not reach checker")
    try:
        check_dragonfang_resolved_successor(positive, inconsistent_runtime_probe, runtime_bound_probe=True)
    except SystemExit:
        pass
    else:
        fail("runtime-bound probe retained a blocked K0 gate")

    # Acceptance gate truth may advance in a successor, but its required gate
    # set and promotion policy remain immutable.
    acceptance_policy_mutation = copy.deepcopy(resolved_successor)
    acceptance_policy_mutation["acceptance_constraints"]["required_gates"].append("K11_POLICY")
    acceptance_policy_mutation["canonical_sha256"] = canonical_hash(acceptance_policy_mutation)
    require(is_valid(schema, acceptance_policy_mutation), "acceptance policy negative did not reach successor checking")
    try:
        check_dragonfang_resolved_successor(positive, acceptance_policy_mutation)
    except SystemExit:
        pass
    else:
        fail("resolved successor accepted a changed required gate policy")

    # A resolved brief may have no conflicts at all; conflict records are an
    # evidence ledger, not a fixture-specific minimum.
    no_conflicts = copy.deepcopy(generic)
    no_conflicts["source_conflicts"] = []
    no_conflicts["canonical_sha256"] = canonical_hash(no_conflicts)
    require(is_valid(schema, no_conflicts), "zero-conflict generic brief was rejected")

    resolved_mismatch = copy.deepcopy(generic)
    resolved_mismatch["surface_constraints"]["hero_budget"]["resolved_min_triangles"] = None
    require(not is_valid(schema, resolved_mismatch), "resolved hero status accepted a null resolution")

    complete_coverage_mismatch = copy.deepcopy(generic)
    complete_coverage_mismatch["reference_coverage"]["missing_views"] = ["back"]
    require(not is_valid(schema, complete_coverage_mismatch), "complete coverage accepted a missing view")

    relative_path = copy.deepcopy(generic)
    relative_path["engine_constraints"]["target_claims"][0]["version_requirement"] = "../profiles/engine.json"
    require(not is_valid(schema, relative_path), "relative path entered the brief")

    executable_instruction = copy.deepcopy(generic)
    executable_instruction["engine_constraints"]["target_claims"][0]["version_requirement"] = "Blender --python addon.py"
    require(not is_valid(schema, executable_instruction), "Blender/plugin instruction entered the brief")

    duplicate_part_id = copy.deepcopy(generic)
    duplicate_part_id["parts"].append({**duplicate_part_id["parts"][0], "role": "component"})
    require(is_valid(schema, duplicate_part_id), "duplicate-part mutation was not structurally schema-valid")
    try:
        check_generic_resolved_probe(duplicate_part_id)
    except SystemExit:
        pass
    else:
        fail("duplicate part ID bypassed cross-field checking")

    unknown_parent = copy.deepcopy(generic)
    unknown_parent["parts"][0]["parent_id"] = "missing-parent"
    require(is_valid(schema, unknown_parent), "unknown-parent mutation should reach cross-field checking")
    try:
        check_generic_resolved_probe(unknown_parent)
    except SystemExit:
        pass
    else:
        fail("unknown part parent bypassed cross-field checking")

    unknown_zone = copy.deepcopy(generic)
    unknown_zone["parts"][0]["material_zone_ids"] = ["missing-zone"]
    require(is_valid(schema, unknown_zone), "unknown-zone mutation should reach cross-field checking")
    try:
        check_generic_resolved_probe(unknown_zone)
    except SystemExit:
        pass
    else:
        fail("unknown material zone bypassed cross-field checking")

    unknown_claim = copy.deepcopy(generic)
    unknown_claim["source_conflicts"][0]["observed_claim_ids"] = ["control-identity", "missing-claim"]
    require(is_valid(schema, unknown_claim), "unknown-claim mutation should reach cross-field checking")
    try:
        check_generic_resolved_probe(unknown_claim)
    except SystemExit:
        pass
    else:
        fail("unknown conflict claim bypassed cross-field checking")

    inverted_range = copy.deepcopy(generic)
    inverted_range["material_zones"][0]["roughness_range"] = {"min": 0.9, "max": 0.1}
    require(is_valid(schema, inverted_range), "range mutation should reach cross-field checking")
    try:
        check_generic_resolved_probe(inverted_range)
    except SystemExit:
        pass
    else:
        fail("inverted material range bypassed cross-field checking")

    lod_order = copy.deepcopy(generic)
    lod_order["surface_constraints"]["lod_levels"][2]["target_percent"] = 80
    require(is_valid(schema, lod_order), "LOD mutation should reach cross-field checking")
    try:
        check_generic_resolved_probe(lod_order)
    except SystemExit:
        pass
    else:
        fail("invalid LOD reduction bypassed cross-field checking")

    material_share = copy.deepcopy(generic)
    material_share["material_zones"][0]["target_share_percent"] = 71
    require(is_valid(schema, material_share), "material-share mutation should reach cross-field checking")
    try:
        check_generic_resolved_probe(material_share)
    except SystemExit:
        pass
    else:
        fail("material shares not totaling 100 bypassed cross-field checking")

    fps_focus = copy.deepcopy(generic)
    fps_focus["presentation_constraints"]["inspect_focus_order"] = ["missing-focus"]
    require(is_valid(schema, fps_focus), "FPS focus mutation should reach cross-field checking")
    try:
        check_generic_resolved_probe(fps_focus)
    except SystemExit:
        pass
    else:
        fail("unknown FPS focus part bypassed cross-field checking")

    gate_dependency = copy.deepcopy(generic)
    gate_dependency["acceptance_constraints"]["status"] = "ready"
    gate_dependency["acceptance_constraints"]["blocking_reasons"] = []
    require(is_valid(schema, gate_dependency), "gate dependency mutation should reach cross-field checking")
    try:
        check_generic_resolved_probe(gate_dependency)
    except SystemExit:
        pass
    else:
        fail("ready acceptance bypassed an unrun gate dependency")

    check_cross_field_invariants(positive)
    check_sanitized_values(resolved_successor)
    check_dragonfang_resolved_successor(positive, resolved_successor)
    check_generic_resolved_probe(generic)
    check_sanitized_values(positive)
    check_sanitized_values(generic)


def main() -> int:
    run_checks()
    print(
        "WeaponryKnifeProductionBrief PASS: closed schema, sanitized fixture, "
        "authorization/coverage/design constraints, explicit source conflicts, "
        "and blocked acceptance gates"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
