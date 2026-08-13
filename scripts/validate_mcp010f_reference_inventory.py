#!/usr/bin/env python3
"""Validate the Codex-side single-reference detail inventory.

This is an orchestration check, not a Runtime or image-understanding step. It
reads typed metadata only, rejects raw image/path material, and optionally
checks every referenced Operator against a live-catalog receipt. It never
calls MCP and never writes SQLite/CAS.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import sys
from pathlib import Path
from typing import Any


HEX64 = re.compile(r"^[0-9a-f]{64}$")
IDENTIFIER = re.compile(r"^[A-Za-z0-9_.:@-]{1,128}$")
FORBIDDEN_KEYS = {
    "bytes",
    "bytes_base64",
    "image_bytes",
    "image_base64",
    "local_path",
    "absolute_path",
    "file_path",
    "prompt",
    "secret",
}
EVIDENCE = {"observed", "inferred", "unknown"}
CRITICALITY = {"identity", "major", "supporting"}
STATUS = {"planned", "tested", "retained", "rejected"}
VISIBILITY = {"visible", "partial", "visible_or_partial", "unknown", "inferred", "cropped_or_unknown"}
REVIEW_SIGNALS = {"boundary", "silhouette", "landmark", "normal", "material-id", "uv-stretch", "part-id", "depth", "ao"}
STAGES = {"reference-canvas", "silhouette", "structure", "form", "material-surface", "final"}
TOPOLOGY = {
    "primitive",
    "box",
    "cylinder",
    "ellipsoid",
    "sphere",
    "profile-extrude",
    "profile-loft",
    "revolve",
    "tube-sweep",
    "panel",
    "vent-array",
    "joint-stack",
    "transform",
    "mirror",
    "array",
    "part-output",
}


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def digest(value: Any) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def short_operator(value: str) -> str:
    prefix = "forgecad.geometry."
    return value[len(prefix) :] if value.startswith(prefix) else value


def add_error(errors: list[str], path: str, message: str) -> None:
    errors.append(f"{path}: {message}")


def walk_forbidden(value: Any, path: str, errors: list[str]) -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            key_text = str(key).lower()
            if key_text in FORBIDDEN_KEYS:
                add_error(errors, f"{path}.{key}", "raw image/path/prompt/secret field is not allowed")
            walk_forbidden(child, f"{path}.{key}", errors)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            walk_forbidden(child, f"{path}[{index}]", errors)


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def active_operators(catalog: Any, errors: list[str]) -> set[str]:
    if not isinstance(catalog, dict):
        add_error(errors, "operator_catalog", "catalog must be an object")
        return set()
    entries = catalog.get("operators")
    if not isinstance(entries, list) or not entries:
        add_error(errors, "operator_catalog.operators", "catalog must contain a non-empty operators list")
        return set()
    active: set[str] = set()
    for index, entry in enumerate(entries):
        if isinstance(entry, str):
            active.add(short_operator(entry))
            continue
        if not isinstance(entry, dict) or not isinstance(entry.get("operator_id"), str):
            add_error(errors, f"operator_catalog.operators[{index}]", "operator entry must have operator_id")
            continue
        status = entry.get("status", "active")
        if status == "active":
            active.add(short_operator(entry["operator_id"]))
    return active


def finite_number(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(float(value))


def assetpack_material_ids(assetpack: Any, errors: list[str]) -> set[str]:
    if not isinstance(assetpack, dict):
        add_error(errors, "assetpack_manifest", "asset pack must be an object")
        return set()
    definitions = assetpack.get("material_definitions")
    if not isinstance(definitions, list) or not definitions:
        add_error(errors, "assetpack_manifest.material_definitions", "asset pack must contain material_definitions")
        return set()
    material_ids: set[str] = set()
    for index, definition in enumerate(definitions):
        if not isinstance(definition, dict) or not isinstance(definition.get("material_id"), str):
            add_error(errors, f"assetpack_manifest.material_definitions[{index}]", "material_id is required")
            continue
        material_ids.add(definition["material_id"])
    return material_ids


def validate_inventory(inventory: Any, catalog: Any | None, assetpack: Any | None) -> dict[str, Any]:
    errors: list[str] = []
    warnings: list[str] = []
    walk_forbidden(inventory, "$", errors)
    if not isinstance(inventory, dict):
        add_error(errors, "$", "inventory must be an object")
        return result(inventory, errors, warnings, None)
    if inventory.get("schema_version") != "ForgeCADCodexReferenceInventory@1":
        add_error(errors, "$.schema_version", "expected ForgeCADCodexReferenceInventory@1")

    reference = inventory.get("reference")
    if not isinstance(reference, dict):
        add_error(errors, "$.reference", "reference object is required")
        reference = {}
    reference_sha = reference.get("reference_sha256")
    if not isinstance(reference_sha, str) or not HEX64.fullmatch(reference_sha):
        add_error(errors, "$.reference.reference_sha256", "must be a lowercase SHA-256")
    if reference.get("source") != "user-authorized-reference":
        add_error(errors, "$.reference.source", "must be user-authorized-reference")
    for field in ("width", "height"):
        value = reference.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or not 1 <= value <= 16384:
            add_error(errors, f"$.reference.{field}", "must be an integer in 1..16384")
    coverage = reference.get("coverage")
    if not isinstance(coverage, dict):
        add_error(errors, "$.reference.coverage", "coverage object is required")
    else:
        required_coverage = {"front", "back", "left", "right", "feet"}
        missing = sorted(required_coverage - set(coverage))
        if missing:
            add_error(errors, "$.reference.coverage", f"missing keys: {', '.join(missing)}")
        for key, value in coverage.items():
            if key in required_coverage and value not in VISIBILITY:
                add_error(errors, f"$.reference.coverage.{key}", "unknown visibility state")

    contract = inventory.get("quality_contract")
    if not isinstance(contract, dict):
        add_error(errors, "$.quality_contract", "quality_contract object is required")
        contract = {}
    if contract.get("target") != "PARTIAL_VISIBLE_VIEW_PASS":
        add_error(errors, "$.quality_contract.target", "single-image target must be PARTIAL_VISIBLE_VIEW_PASS")
    max_rounds = contract.get("max_correction_rounds")
    if not isinstance(max_rounds, int) or isinstance(max_rounds, bool) or not 1 <= max_rounds <= 5:
        add_error(errors, "$.quality_contract.max_correction_rounds", "must be an integer in 1..5")
        max_rounds = 5
    priority = contract.get("metric_priority")
    expected_priority = [
        "boundary_f1_4px",
        "silhouette_iou",
        "bbox_edge_error",
        "centroid_error",
        "landmark_coverage_and_nme",
        "region_iou",
    ]
    if priority != expected_priority:
        add_error(errors, "$.quality_contract.metric_priority", "must preserve boundary-first metric order")
    if contract.get("hq_360_status") != "BLOCKED_REFERENCE_COVERAGE":
        add_error(errors, "$.quality_contract.hq_360_status", "must remain BLOCKED_REFERENCE_COVERAGE for one image")

    inventory_items = inventory.get("detail_inventory")
    if not isinstance(inventory_items, list) or not inventory_items:
        add_error(errors, "$.detail_inventory", "at least one detail is required")
        inventory_items = []
    if len(inventory_items) > 128:
        add_error(errors, "$.detail_inventory", "maximum 128 details")
    detail_ids: set[str] = set()
    referenced_ops: set[str] = set()
    referenced_materials: set[str] = set()
    counts = {"observed": 0, "inferred": 0, "unknown": 0}
    for index, item in enumerate(inventory_items):
        path = f"$.detail_inventory[{index}]"
        if not isinstance(item, dict):
            add_error(errors, path, "detail must be an object")
            continue
        detail_id = item.get("detail_id")
        if not isinstance(detail_id, str) or not IDENTIFIER.fullmatch(detail_id):
            add_error(errors, f"{path}.detail_id", "must be a bounded identifier")
        elif detail_id in detail_ids:
            add_error(errors, f"{path}.detail_id", "duplicate detail_id")
        else:
            detail_ids.add(detail_id)
        for field in ("semantic_part_id", "feature", "line_flow", "material_zone_id"):
            value = item.get(field)
            if not isinstance(value, str) or not value.strip():
                add_error(errors, f"{path}.{field}", "non-empty string is required")
        material_id = item.get("material_zone_id")
        if isinstance(material_id, str) and material_id.strip():
            referenced_materials.add(material_id)
        evidence = item.get("evidence")
        if evidence not in EVIDENCE:
            add_error(errors, f"{path}.evidence", "must be observed, inferred or unknown")
        else:
            counts[evidence] += 1
        if item.get("criticality") not in CRITICALITY:
            add_error(errors, f"{path}.criticality", "unknown criticality")
        topology = item.get("topology_strategy")
        if topology not in TOPOLOGY:
            add_error(errors, f"{path}.topology_strategy", "operator strategy is not in the first-party allowlist")
        operators = item.get("operator_ids")
        if not isinstance(operators, list) or not operators or any(not isinstance(op, str) or not op.strip() for op in operators):
            add_error(errors, f"{path}.operator_ids", "at least one typed operator is required")
        else:
            referenced_ops.update(short_operator(op) for op in operators)
        confidence = item.get("confidence")
        if not finite_number(confidence) or not 0.0 <= float(confidence) <= 1.0:
            add_error(errors, f"{path}.confidence", "must be finite and within 0..1")
        if item.get("review_signal") not in REVIEW_SIGNALS:
            add_error(errors, f"{path}.review_signal", "unknown review signal")
        if item.get("status") not in STATUS:
            add_error(errors, f"{path}.status", "unknown detail status")

    catalog_status = "NOT_RUN_LIVE_CATALOG"
    catalog_active: set[str] | None = None
    if catalog is not None:
        catalog_errors: list[str] = []
        catalog_active = active_operators(catalog, catalog_errors)
        if catalog_errors:
            errors.extend(catalog_errors)
            catalog_status = "FAIL"
        else:
            missing = sorted(referenced_ops - catalog_active)
            if missing:
                add_error(errors, "$.detail_inventory.operator_ids", f"not active in supplied catalog: {', '.join(missing)}")
                catalog_status = "FAIL"
            else:
                catalog_status = "PASS_LIVE_ACTIVE_OPERATORS"
    else:
        warnings.append("live OperatorCatalog was not supplied; run the helper again with current catalog before geometry_prepare")

    assetpack_status = "NOT_RUN_ASSETPACK_MANIFEST"
    if assetpack is not None:
        assetpack_errors: list[str] = []
        material_ids = assetpack_material_ids(assetpack, assetpack_errors)
        if assetpack_errors:
            errors.extend(assetpack_errors)
            assetpack_status = "FAIL"
        else:
            missing_materials = sorted(referenced_materials - material_ids)
            if missing_materials:
                add_error(errors, "$.detail_inventory.material_zone_id", f"not present in supplied AssetPack: {', '.join(missing_materials)}")
                assetpack_status = "FAIL"
            else:
                assetpack_status = "PASS_ASSETPACK_MATERIALS"
    else:
        warnings.append("AssetPack manifest was not supplied; run the helper again with the current offline pack before appearance_prepare")

    correction = inventory.get("correction_state")
    if not isinstance(correction, dict):
        add_error(errors, "$.correction_state", "correction_state object is required")
        correction = {}
    rounds_used = correction.get("rounds_used")
    rounds_remaining = correction.get("rounds_remaining")
    if not isinstance(rounds_used, int) or not 0 <= rounds_used <= max_rounds:
        add_error(errors, "$.correction_state.rounds_used", "must be within max_correction_rounds")
    if not isinstance(rounds_remaining, int) or rounds_remaining < 0:
        add_error(errors, "$.correction_state.rounds_remaining", "must be a non-negative integer")
    if isinstance(rounds_used, int) and isinstance(rounds_remaining, int) and rounds_used + rounds_remaining != max_rounds:
        add_error(errors, "$.correction_state", "rounds_used + rounds_remaining must equal max_correction_rounds")
    if correction.get("current_stage") not in STAGES:
        add_error(errors, "$.correction_state.current_stage", "unknown workflow stage")
    if correction.get("confirmation_allowed") is not False:
        add_error(errors, "$.correction_state.confirmation_allowed", "inventory cannot unlock confirmation")
    if correction.get("human_review") != "NOT_RUN":
        add_error(errors, "$.correction_state.human_review", "inventory cannot fabricate human review")
    if correction.get("full_360") != "BLOCKED_REFERENCE_COVERAGE":
        add_error(errors, "$.correction_state.full_360", "single-image 360 status must remain blocked")
    if inventory.get("persistent_user_data_touched") is not False:
        add_error(errors, "$.persistent_user_data_touched", "must be false")

    return result(inventory, errors, warnings, catalog_status, assetpack_status=assetpack_status, counts=counts, detail_count=len(inventory_items), operator_count=len(referenced_ops))


def result(
    inventory: Any,
    errors: list[str],
    warnings: list[str],
    catalog_status: str | None,
    *,
    assetpack_status: str = "NOT_RUN_ASSETPACK_MANIFEST",
    counts: dict[str, int] | None = None,
    detail_count: int = 0,
    operator_count: int = 0,
) -> dict[str, Any]:
    return {
        "schema_version": "ForgeCADReferenceInventoryValidation@1",
        "status": "PASS" if not errors else "FAIL",
        "validation_status": "passed" if not errors else "rejected",
        "inventory_sha256": digest(inventory) if inventory is not None else None,
        "detail_count": detail_count,
        "evidence_counts": counts or {"observed": 0, "inferred": 0, "unknown": 0},
        "referenced_operator_count": operator_count,
        "operator_catalog": catalog_status or "NOT_RUN_LIVE_CATALOG",
        "assetpack_manifest": assetpack_status,
        "errors": errors,
        "warnings": warnings,
        "next_action": "construct_or_update_hash_bound_geometry_draft" if not errors else "stop_and_fix_inventory_before_geometry_write",
        "persistent_user_data_touched": False,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inventory", type=Path, required=True)
    parser.add_argument("--operator-catalog", type=Path)
    parser.add_argument("--assetpack-manifest", type=Path)
    parser.add_argument("--output", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        inventory = load_json(args.inventory)
        catalog = load_json(args.operator_catalog) if args.operator_catalog else None
        assetpack = load_json(args.assetpack_manifest) if args.assetpack_manifest else None
        receipt = validate_inventory(inventory, catalog, assetpack)
    except (OSError, json.JSONDecodeError, TypeError, ValueError) as exc:
        receipt = result(None, [f"input: {exc}"], [], "FAIL")
    serialized = json.dumps(receipt, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(serialized, encoding="utf-8")
    sys.stdout.write(serialized)
    return 0 if receipt["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
