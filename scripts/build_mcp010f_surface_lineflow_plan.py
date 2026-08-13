#!/usr/bin/env python3
"""Build a bounded surface-lineflow plan for Codex orchestration.

This helper is deliberately not a Runtime compiler.  It consumes an already
validated reference inventory, the live OperatorCatalog receipt and the
offline AssetPack manifest, then emits hash-bound *intent* actions.  It never
reads image bytes, calls MCP, writes SQLite/CAS, or invents geometry
parameters.  A failed silhouette gate keeps structure/detail/material work
locked so surface language cannot hide a proportion error.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
from pathlib import Path
from typing import Any


MAX_ACTIONS = 5
SILHOUETTE_METRICS = ("silhouette_iou", "boundary_f1_4px", "bbox_edge_error", "centroid_error")
STRUCTURE_METRICS = ("landmark_coverage", "landmark_nme")
FORM_METRICS = ("region_median_iou", "critical_region_min_iou")
THRESHOLDS: dict[str, tuple[str, float]] = {
    "silhouette_iou": (">=", 0.90),
    "boundary_f1_4px": (">=", 0.90),
    "bbox_edge_error": ("<=", 0.02),
    "centroid_error": ("<=", 0.02),
    "landmark_coverage": (">=", 0.80),
    "landmark_nme": ("<=", 0.03),
    "region_median_iou": (">=", 0.85),
    "critical_region_min_iou": (">=", 0.85),
}
ALLOWED_EVIDENCE = {"observed", "inferred", "unknown"}
ALLOWED_SIGNAL = {"silhouette", "boundary", "normal", "part-id", "material-id"}
ALLOWED_CRITICALITY = {"identity": 0, "major": 1, "supporting": 2}


class PlanError(ValueError):
    pass


def fail(message: str) -> None:
    raise PlanError(message)


def load_json(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {label}: {error}")
    if not isinstance(value, dict):
        fail(f"{label} must be a JSON object")
    return value


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def sha256_json(value: Any) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def sha256(value: Any, label: str) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(char not in "0123456789abcdef" for char in value):
        fail(f"{label} must be a lowercase SHA-256")
    return value


def identifier(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value or len(value) > 128:
        fail(f"{label} must be a bounded identifier")
    allowed = set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_.-@→")
    if any(char not in allowed for char in value):
        fail(f"{label} contains an unsupported character")
    return value


def normalize_operator(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        fail(f"{label} must be an operator id")
    if "@" not in value:
        fail(f"{label} has no version")
    if value.startswith("forgecad.geometry."):
        return value
    return f"forgecad.geometry.{value}"


def active_operator_ids(catalog: dict[str, Any]) -> tuple[set[str], str]:
    if catalog.get("schema_version") != "OperatorCatalog@1":
        fail("operator catalog must be OperatorCatalog@1")
    entries = catalog.get("operators")
    if not isinstance(entries, list) or not entries:
        fail("operator catalog must contain operators")
    active: set[str] = set()
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            fail(f"operator catalog entry {index} is invalid")
        operator = normalize_operator(entry.get("operator_id"), f"operator catalog entry {index}")
        if entry.get("status") == "active":
            active.add(operator)
    catalog_hash = catalog.get("canonical_sha256")
    if not isinstance(catalog_hash, str):
        catalog_hash = sha256_json(catalog)
    return active, sha256(catalog_hash, "operator_catalog_sha256")


def asset_material_ids(assetpack: dict[str, Any]) -> tuple[set[str], str]:
    if assetpack.get("schema_version") != "MaterialPackManifest@1":
        fail("AssetPack must be MaterialPackManifest@1")
    materials = assetpack.get("material_definitions")
    if not isinstance(materials, list) or not materials:
        fail("AssetPack has no material definitions")
    ids: set[str] = set()
    for index, material in enumerate(materials):
        if not isinstance(material, dict) or not isinstance(material.get("material_id"), str):
            fail(f"AssetPack material {index} is invalid")
        ids.add(material["material_id"])
    manifest_hash = assetpack.get("canonical_sha256")
    if not isinstance(manifest_hash, str):
        manifest_hash = sha256_json(assetpack)
    return ids, sha256(manifest_hash, "assetpack_manifest_sha256")


def metric_failed(metrics: dict[str, Any], key: str) -> bool:
    value = metrics.get(key)
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        return True
    operator, threshold = THRESHOLDS[key]
    return float(value) < threshold if operator == ">=" else float(value) > threshold


def failed_metrics(metrics: dict[str, Any], keys: tuple[str, ...]) -> list[str]:
    return [key for key in keys if metric_failed(metrics, key)]


def validate_inputs(
    inventory: dict[str, Any],
    catalog: dict[str, Any],
    assetpack: dict[str, Any],
    validation: dict[str, Any],
) -> tuple[set[str], set[str], str, str, str]:
    if inventory.get("schema_version") != "ForgeCADCodexReferenceInventory@1":
        fail("inventory must be ForgeCADCodexReferenceInventory@1")
    if inventory.get("reference", {}).get("source") != "user-authorized-reference":
        fail("inventory reference is not explicitly user-authorized")
    reference_hash = sha256(inventory.get("reference", {}).get("reference_sha256"), "reference_sha256")
    if validation.get("schema_version") != "ForgeCADReferenceInventoryValidation@1":
        fail("validation receipt has the wrong schema")
    if validation.get("status") != "PASS" or validation.get("operator_catalog") != "PASS_LIVE_ACTIVE_OPERATORS":
        fail("reference inventory validation did not pass with a live active catalog")
    if validation.get("assetpack_manifest") != "PASS_ASSETPACK_MATERIALS":
        fail("reference inventory validation did not pass with the AssetPack")
    active, catalog_hash = active_operator_ids(catalog)
    materials, assetpack_hash = asset_material_ids(assetpack)
    return active, materials, reference_hash, catalog_hash, assetpack_hash


def detail_sort_key(item: dict[str, Any]) -> tuple[int, int, float, str]:
    return (
        ALLOWED_CRITICALITY.get(str(item.get("criticality")), 9),
        0 if item.get("review_signal") in {"silhouette", "boundary"} else 1,
        -float(item.get("confidence", 0.0)),
        str(item.get("detail_id", "")),
    )


def build_plan(
    inventory: dict[str, Any],
    catalog: dict[str, Any],
    assetpack: dict[str, Any],
    validation: dict[str, Any],
    inventory_file: str,
    catalog_file: str,
    assetpack_file: str,
) -> dict[str, Any]:
    active, materials, reference_hash, catalog_hash, assetpack_hash = validate_inputs(
        inventory, catalog, assetpack, validation
    )
    details = inventory.get("detail_inventory")
    if not isinstance(details, list) or not details:
        fail("inventory detail_inventory must be a non-empty list")
    correction = inventory.get("correction_state")
    if not isinstance(correction, dict):
        fail("inventory correction_state is required")
    baseline = correction.get("current_source_baseline")
    if not isinstance(baseline, dict):
        fail("inventory current_source_baseline is required")
    metrics = {key: baseline.get(key) for key in THRESHOLDS}
    contour_failures = failed_metrics(metrics, SILHOUETTE_METRICS)
    structure_failures = failed_metrics(metrics, STRUCTURE_METRICS)
    form_failures = failed_metrics(metrics, FORM_METRICS)

    usable: list[dict[str, Any]] = []
    deferred: list[dict[str, str]] = []
    for index, raw in enumerate(details):
        if not isinstance(raw, dict):
            fail(f"detail_inventory[{index}] must be an object")
        detail_id = identifier(raw.get("detail_id"), f"detail_inventory[{index}].detail_id")
        evidence = raw.get("evidence")
        signal = raw.get("review_signal")
        material_id = raw.get("material_zone_id")
        if evidence not in ALLOWED_EVIDENCE:
            fail(f"{detail_id}.evidence is invalid")
        if signal not in ALLOWED_SIGNAL:
            fail(f"{detail_id}.review_signal is invalid")
        if not isinstance(material_id, str) or material_id not in materials:
            fail(f"{detail_id}.material_zone_id is not in the AssetPack")
        operators = raw.get("operator_ids")
        if not isinstance(operators, list) or not operators:
            fail(f"{detail_id}.operator_ids must be non-empty")
        normalized = [normalize_operator(value, f"{detail_id}.operator_ids") for value in operators]
        missing = [value for value in normalized if value not in active]
        if missing:
            fail(f"{detail_id} references inactive operators: {', '.join(missing)}")
        confidence = raw.get("confidence")
        if isinstance(confidence, bool) or not isinstance(confidence, (int, float)) or not 0.0 <= float(confidence) <= 1.0:
            fail(f"{detail_id}.confidence is outside [0, 1]")
        if evidence == "unknown":
            deferred.append({"detail_id": detail_id, "reason": "reference coverage is unknown"})
            continue
        usable.append({**raw, "detail_id": detail_id, "normalized_operator_ids": normalized})

    if contour_failures:
        eligible = [item for item in usable if item["review_signal"] in {"silhouette", "boundary"}]
        stage = "silhouette-blockout"
        locked_stages = ["landmark-structure", "semantic-part-fill", "surface-detail", "uv-pbr"]
        blocked_reasons = ["silhouette_gate_failed", "surface_material_locked_until_contour_and_form_pass"]
    elif structure_failures:
        eligible = [item for item in usable if item["review_signal"] in {"silhouette", "boundary", "normal", "part-id"}]
        stage = "landmark-structure"
        locked_stages = ["semantic-part-fill", "surface-detail", "uv-pbr"]
        blocked_reasons = ["structure_gate_failed", "surface_material_locked_until_form_pass"]
    elif form_failures:
        eligible = usable
        stage = "semantic-part-fill"
        locked_stages = ["surface-detail", "uv-pbr"]
        blocked_reasons = ["form_gate_failed", "surface_material_locked_until_form_pass"]
    else:
        eligible = [item for item in usable if item["review_signal"] == "material-id"] or usable
        stage = "surface-detail"
        locked_stages = ["uv-pbr"]
        blocked_reasons = []
    eligible.sort(key=detail_sort_key)

    actions: list[dict[str, Any]] = []
    for round_number, item in enumerate(eligible[:MAX_ACTIONS], start=1):
        actions.append(
            {
                "round": round_number,
                "stage": stage,
                "detail_id": item["detail_id"],
                "semantic_part_id": item.get("semantic_part_id"),
                "feature": item.get("feature"),
                "line_flow": item.get("line_flow"),
                "evidence": item["evidence"],
                "confidence": item["confidence"],
                "review_signal": item["review_signal"],
                "operator_ids": item["normalized_operator_ids"],
                "material_zone_id": item["material_zone_id"],
                "change_policy": "one_semantic_part_one_operator_stage; preserve_camera_reference_and_catalog_hash",
            }
        )

    if not actions:
        blocked_reasons.append("no_observed_or_inferred_details_are_eligible_for_current_gate")

    plan: dict[str, Any] = {
        "schema_version": "ForgeCADSurfaceLineFlowPlan@1",
        "status": "READY_FOR_SINGLE_PART_FLOW_REVIEW" if actions else "BLOCKED_NO_ELIGIBLE_FLOW_ACTION",
        "source": "codex-orchestration-only",
        "reference_sha256": reference_hash,
        "inventory_sha256": sha256_json(inventory),
        "operator_catalog_sha256": catalog_hash,
        "assetpack_manifest_sha256": assetpack_hash,
        "inputs": {
            "inventory_file": inventory_file,
            "operator_catalog_file": catalog_file,
            "assetpack_manifest_file": assetpack_file,
            "validation_status": validation["status"],
        },
        "metric_priority": ["boundary_f1_4px", "silhouette_iou", "bbox_edge_error", "centroid_error", "landmark_coverage", "landmark_nme", "region_median_iou", "critical_region_min_iou"],
        "current_gate": {
            "stage": stage,
            "contour_failed_metrics": contour_failures,
            "structure_failed_metrics": structure_failures,
            "form_failed_metrics": form_failures,
            "surface_material_unlocked": not contour_failures and not structure_failures and not form_failures,
        },
        "actions": actions,
        "deferred_unknown_details": deferred,
        "locked_stages": locked_stages,
        "blocked_reasons": blocked_reasons,
        "quality_claim": "ORCHESTRATION_ONLY_NO_NEW_QUALITY_RESULT",
        "runtime_write": False,
        "persistent_user_data_touched": False,
        "plan_sha256": "",
    }
    plan["plan_sha256"] = sha256_json(plan)
    return plan


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--inventory", required=True, type=Path)
    parser.add_argument("--operator-catalog", required=True, type=Path)
    parser.add_argument("--assetpack-manifest", required=True, type=Path)
    parser.add_argument("--validation", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    plan = build_plan(
        load_json(args.inventory, "inventory"),
        load_json(args.operator_catalog, "operator catalog"),
        load_json(args.assetpack_manifest, "AssetPack manifest"),
        load_json(args.validation, "validation receipt"),
        args.inventory.as_posix(),
        args.operator_catalog.as_posix(),
        args.assetpack_manifest.as_posix(),
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(plan, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": plan["status"], "action_count": len(plan["actions"]), "plan_sha256": plan["plan_sha256"]}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except PlanError as error:
        raise SystemExit(f"surface line-flow plan rejected: {error}")
