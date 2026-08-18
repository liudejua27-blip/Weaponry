#!/usr/bin/env python3
"""Build a deterministic, local-only correction queue for one reference view.

This is a Codex orchestration aid, not a Runtime quality source.  It reads
already persisted/hash-bound JSON reports, never reads image bytes, never calls
the network, and never writes SQLite/CAS.  The output deliberately contains
bounded *intent* suggestions rather than geometry parameters or visual claims.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import sys
from pathlib import Path
from typing import Any


MAX_ACTIONS = 5
METRIC_THRESHOLDS = {
    "silhouette_iou": (">=", 0.90),
    "boundary_f1_4px": (">=", 0.90),
    "bbox_edge_error": ("<=", 0.02),
    "centroid_error": ("<=", 0.02),
    "landmark_coverage": (">=", 0.80),
    "landmark_nme": ("<=", 0.03),
    "region_median_iou": (">=", 0.85),
    "critical_region_min_iou": (">=", 0.85),
}

# The correction queue is intentionally silhouette-first.  These are image
# space gates used by Codex orchestration only; Runtime QualityReport remains
# the product truth and this helper never creates or edits one.
SILHOUETTE_METRICS = ("silhouette_iou", "boundary_f1_4px", "bbox_edge_error", "centroid_error")
STRUCTURE_METRICS = ("landmark_coverage", "landmark_nme")
FORM_METRICS = ("region_median_iou", "critical_region_min_iou")

OPERATOR_HINTS = {
    "whole-body": [
        "forgecad.geometry.transform@2",
        "forgecad.geometry.mirror@1",
        "forgecad.geometry.part-output@1",
    ],
    "head-visor": [
        "forgecad.geometry.panel@1",
        "forgecad.geometry.profile-extrude@1",
        "forgecad.geometry.profile-loft@1",
        "forgecad.geometry.transform@2",
    ],
    "neck-mechanism": [
        "forgecad.geometry.joint-stack@1",
        "forgecad.geometry.revolve@1",
        "forgecad.geometry.tube-sweep@1",
    ],
    "chest-armor": [
        "forgecad.geometry.panel@1",
        "forgecad.geometry.profile-loft@1",
        "forgecad.geometry.vent-array@1",
        "forgecad.geometry.mirror@1",
    ],
    "left-shoulder-arm": [
        "forgecad.geometry.panel@1",
        "forgecad.geometry.joint-stack@1",
        "forgecad.geometry.tube-sweep@1",
        "forgecad.geometry.mirror@1",
    ],
    "right-shoulder-arm": [
        "forgecad.geometry.panel@1",
        "forgecad.geometry.joint-stack@1",
        "forgecad.geometry.tube-sweep@1",
        "forgecad.geometry.mirror@1",
    ],
    "pelvis-core": [
        "forgecad.geometry.panel@1",
        "forgecad.geometry.profile-loft@1",
        "forgecad.geometry.joint-stack@1",
    ],
    "left-thigh-knee": [
        "forgecad.geometry.panel@1",
        "forgecad.geometry.joint-stack@1",
        "forgecad.geometry.profile-extrude@1",
    ],
    "right-thigh-knee": [
        "forgecad.geometry.panel@1",
        "forgecad.geometry.joint-stack@1",
        "forgecad.geometry.profile-extrude@1",
    ],
}

# A comparison region is an image-space observation, not a Runtime Part.  Keep
# this bridge explicit and bounded so Codex can select a stable Part without
# guessing from a free-form region name.  ``primary`` is the one-Part change
# target for the next round; ``supporting`` parts are read-only context for
# lineage/material/AOV inspection and must not be changed in the same round.
REGION_PART_TARGETS = {
    "head-visor": {
        "primary": ["head-shell"],
        "supporting": ["visor", "visor-edge"],
    },
    "neck-mechanism": {
        "primary": ["neck-ring"],
        "supporting": ["cable-pair", "chest-core"],
    },
    "chest-armor": {
        "primary": ["chest-shell"],
        "supporting": ["chest-ridge", "chest-vent", "chest-core"],
    },
    "left-shoulder-arm": {
        "primary": ["shoulder-armor-pair"],
        "supporting": ["shoulder-pair", "upper-arm-pair", "elbow-pair", "forearm-pair", "hand-pair"],
    },
    "right-shoulder-arm": {
        "primary": ["shoulder-armor-pair"],
        "supporting": ["shoulder-pair", "upper-arm-pair", "elbow-pair", "forearm-pair", "hand-pair"],
    },
    "pelvis-core": {
        "primary": ["pelvis-shell"],
        "supporting": ["hip-pair", "core-ribs", "chest-core"],
    },
    "left-thigh-knee": {
        "primary": ["thigh-pair"],
        "supporting": ["knee-pair", "shin-pair", "knee-cap-pair"],
    },
    "right-thigh-knee": {
        "primary": ["thigh-pair"],
        "supporting": ["knee-pair", "shin-pair", "knee-cap-pair"],
    },
}

PART_MATERIAL_HINTS = {
    "head-shell": ["zone-white-shell"],
    "visor": ["zone-black-anodized"],
    "visor-edge": ["zone-emissive-amber"],
    "neck-ring": ["zone-brushed-steel"],
    "chest-shell": ["zone-white-shell"],
    "chest-ridge": ["zone-micro-scratch"],
    "chest-vent": ["zone-black-anodized"],
    "chest-core": ["zone-black-anodized", "zone-emissive-amber"],
    "shoulder-armor-pair": ["zone-white-shell"],
    "upper-arm-pair": ["zone-white-shell", "zone-dark-painted"],
    "forearm-pair": ["zone-white-shell", "zone-dark-painted"],
    "pelvis-shell": ["zone-white-shell"],
    "thigh-pair": ["zone-white-shell"],
    "knee-pair": ["zone-brushed-steel"],
    "shin-pair": ["zone-white-shell"],
}


class InputError(ValueError):
    pass


def fail(message: str) -> None:
    raise InputError(message)


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read JSON {path}: {error}")
    if not isinstance(value, dict):
        fail(f"{path} must contain a JSON object")
    return value


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def canonical_hash(value: dict[str, Any], hash_key: str = "canonical_sha256") -> str:
    draft = copy.deepcopy(value)
    if hash_key in draft:
        draft[hash_key] = ""
    return hashlib.sha256(canonical_bytes(draft)).hexdigest()


def require_identifier(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value or len(value) > 128:
        fail(f"{label} is not a bounded identifier")
    if any(character not in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_.-" for character in value):
        fail(f"{label} contains an unsupported character")
    return value


def require_sha(value: Any, label: str) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        fail(f"{label} is not a lowercase SHA-256")
    return value


def require_operator_id(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value or len(value) > 128:
        fail(f"{label} is not a bounded operator id")
    if any(character not in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_.-@" for character in value):
        fail(f"{label} contains an unsupported character")
    if value.count("@") != 1:
        fail(f"{label} must contain one version separator")
    return value


def number(value: Any, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        fail(f"{label} is not numeric")
    result = float(value)
    if result != result or result in (float("inf"), float("-inf")) or not 0.0 <= result <= 1.0:
        fail(f"{label} is outside [0, 1]")
    return result


def validate_comparison(value: dict[str, Any]) -> dict[str, Any]:
    if value.get("schema_version") != "ReferenceComparisonReport@1":
        fail("comparison report must be ReferenceComparisonReport@1")
    require_identifier(value.get("report_id"), "comparison.report_id")
    require_identifier(value.get("candidate_id"), "comparison.candidate_id")
    require_identifier(value.get("reference_id"), "comparison.reference_id")
    require_sha(value.get("artifact_sha256"), "comparison.artifact_sha256")
    require_sha(value.get("reference_sha256"), "comparison.reference_sha256")
    require_sha(value.get("render_set_hash"), "comparison.render_set_hash")
    require_sha(value.get("camera_hash"), "comparison.camera_hash")
    require_sha(value.get("canonical_sha256"), "comparison.canonical_sha256")
    if canonical_hash(value) != value["canonical_sha256"]:
        fail("comparison report canonical_sha256 does not match its bytes")
    metrics = value.get("metrics")
    if not isinstance(metrics, dict):
        fail("comparison.metrics must be an object")
    for key in METRIC_THRESHOLDS:
        number(metrics.get(key), f"comparison.metrics.{key}")
    if value.get("status") not in {"PARTIAL_VISIBLE_VIEW_PASS", "QUALITY_TARGET_NOT_MET", "BLOCKED_REFERENCE_COVERAGE"}:
        fail("comparison.status is invalid")
    return value


def validate_view_spec(value: dict[str, Any]) -> dict[str, Any]:
    if value.get("schema_version") != "ReferenceViewSpec@1":
        fail("view spec must be ReferenceViewSpec@1")
    reference_id = require_identifier(value.get("reference_id"), "view.reference_id")
    require_sha(value.get("reference_sha256"), "view.reference_sha256")
    require_identifier(value.get("view_id"), "view.view_id")
    if value.get("source_view") not in {
        "front",
        "back",
        "left",
        "right",
        "top",
        "bottom",
        "front-three-quarter",
        "rear-three-quarter",
        "detail",
        "three-quarter",
        "unknown",
    }:
        fail("view.source_view is invalid")
    require_sha(value.get("canonical_sha256"), "view.canonical_sha256")
    if canonical_hash(value) != value["canonical_sha256"]:
        fail("view spec canonical_sha256 does not match its bytes")
    image = value.get("image")
    if not isinstance(image, dict) or not isinstance(image.get("width"), int) or not isinstance(image.get("height"), int):
        fail("view.image dimensions are invalid")
    for field in ("landmarks", "regions"):
        if not isinstance(value.get(field), list):
            fail(f"view.{field} must be an array")
    landmark_ids: set[str] = set()
    for item in value["landmarks"]:
        if not isinstance(item, dict):
            fail("view landmark must be an object")
        landmark_id = require_identifier(item.get("landmark_id"), "view.landmark_id")
        if landmark_id in landmark_ids:
            fail(f"duplicate landmark {landmark_id}")
        landmark_ids.add(landmark_id)
        number(item.get("x"), f"landmark {landmark_id}.x")
        number(item.get("y"), f"landmark {landmark_id}.y")
        number(item.get("confidence"), f"landmark {landmark_id}.confidence")
        if item.get("visibility") not in {"observed", "inferred", "unknown"}:
            fail(f"landmark {landmark_id}.visibility is invalid")
    region_ids: set[str] = set()
    for item in value["regions"]:
        if not isinstance(item, dict):
            fail("view region must be an object")
        region_id = require_identifier(item.get("region_id"), "view.region_id")
        if region_id in region_ids:
            fail(f"duplicate region {region_id}")
        region_ids.add(region_id)
        for field in ("x", "y", "width", "height", "confidence"):
            number(item.get(field), f"region {region_id}.{field}")
        if item["width"] <= 0 or item["height"] <= 0:
            fail(f"region {region_id} dimensions must be positive")
        if item.get("visibility") not in {"observed", "inferred", "unknown"}:
            fail(f"region {region_id}.visibility is invalid")
    if reference_id != value["reference_id"]:
        fail("reference id mismatch")
    return value


def validate_review(value: dict[str, Any]) -> dict[str, Any]:
    if value.get("schema_version") != "VisualReviewReport@1":
        fail("review must be VisualReviewReport@1")
    require_identifier(value.get("review_id"), "review.review_id")
    require_identifier(value.get("candidate_id"), "review.candidate_id")
    require_identifier(value.get("reference_id"), "review.reference_id")
    require_sha(value.get("render_set_hash"), "review.render_set_hash")
    require_sha(value.get("comparison_report_hash"), "review.comparison_report_hash")
    if not isinstance(value.get("issues"), list):
        fail("review.issues must be an array")
    return value


def active_catalog_ids(value: dict[str, Any] | None) -> set[str] | None:
    if value is None:
        return None
    if value.get("schema_version") != "OperatorCatalog@1":
        fail("catalog must be OperatorCatalog@1")
    require_sha(value.get("canonical_sha256"), "catalog.canonical_sha256")
    operators = value.get("operators")
    if not isinstance(operators, list):
        fail("catalog.operators must be an array")
    result: set[str] = set()
    for item in operators:
        if not isinstance(item, dict):
            fail("catalog operator must be an object")
        operator_id = require_operator_id(item.get("operator_id"), "catalog.operator_id")
        if item.get("status") == "active":
            result.add(operator_id)
    return result


def sorted_regions(view: dict[str, Any]) -> list[dict[str, Any]]:
    return sorted(
        [item for item in view["regions"] if item.get("visibility") != "unknown"],
        key=lambda item: (
            -(float(item["confidence"]) * float(item["width"]) * float(item["height"])),
            item["region_id"],
        ),
    )


def sorted_landmarks(view: dict[str, Any]) -> list[dict[str, Any]]:
    return sorted(
        [item for item in view["landmarks"] if item.get("visibility") != "unknown"],
        key=lambda item: (-float(item["confidence"]), item["landmark_id"]),
    )


def failed(metrics: dict[str, Any], key: str) -> bool:
    operator, threshold = METRIC_THRESHOLDS[key]
    value = float(metrics[key])
    return value < threshold if operator == ">=" else value > threshold


def gate_snapshot(metrics: dict[str, Any]) -> dict[str, Any]:
    """Return the ordered visual gates used to unlock later passes.

    The canvas/overlay is a transient Codex review surface.  It must never
    turn a material issue into a geometry pass or allow PBR work to mask a
    silhouette error, so later gates are cumulative and fail closed.
    """
    silhouette_failed = [key for key in SILHOUETTE_METRICS if failed(metrics, key)]
    structure_failed = [key for key in STRUCTURE_METRICS if failed(metrics, key)]
    form_failed = [key for key in FORM_METRICS if failed(metrics, key)]
    silhouette_passed = not silhouette_failed
    structure_passed = silhouette_passed and not structure_failed
    form_passed = structure_passed and not form_failed
    return {
        "silhouette": {
            "passed": silhouette_passed,
            "failed_metrics": silhouette_failed,
            "priority": 1,
        },
        "structure": {
            "passed": structure_passed,
            "failed_metrics": structure_failed,
            "priority": 2,
        },
        "form": {
            "passed": form_passed,
            "failed_metrics": form_failed,
            "priority": 3,
        },
        "surface_material_unlocked": form_passed,
    }


def basis(metrics: dict[str, Any], keys: list[str]) -> list[dict[str, Any]]:
    return [
        {
            "metric": key,
            "value": metrics[key],
            "operator": METRIC_THRESHOLDS[key][0],
            "threshold": METRIC_THRESHOLDS[key][1],
        }
        for key in keys
    ]


def hints(region_ids: list[str], active_ids: set[str] | None) -> list[str]:
    candidates: list[str] = []
    for region_id in region_ids or ["whole-body"]:
        for operator_id in OPERATOR_HINTS.get(region_id, OPERATOR_HINTS["whole-body"]):
            if operator_id not in candidates and (active_ids is None or operator_id in active_ids):
                candidates.append(operator_id)
    return candidates[:6]


def part_targets(region_ids: list[str]) -> dict[str, list[str]]:
    """Resolve image regions to bounded semantic Part targets.

    Unknown region IDs are surfaced rather than converted into invented Part
    names.  Ordering is stable and follows the region priority from the
    comparison report/view spec.
    """
    primary_candidates: list[str] = []
    supporting: list[str] = []
    unmapped: list[str] = []
    for region_id in region_ids:
        binding = REGION_PART_TARGETS.get(region_id)
        if binding is None:
            unmapped.append(region_id)
            continue
        for part_id in binding["primary"] + binding["supporting"]:
            target = primary_candidates if part_id in binding["primary"] else supporting
            if part_id not in target and part_id not in primary_candidates and part_id not in supporting:
                target.append(part_id)
    # A correction round is intentionally single-Part.  Keep the first
    # high-priority region target writable and demote other overlapping
    # candidates to context so the plan cannot silently request a multi-Part
    # edit.
    primary = primary_candidates[:1]
    for part_id in primary_candidates[1:]:
        if part_id not in supporting:
            supporting.append(part_id)
    material_zones: list[str] = []
    for part_id in primary + supporting:
        for zone_id in PART_MATERIAL_HINTS.get(part_id, []):
            if zone_id not in material_zones:
                material_zones.append(zone_id)
    return {
        "primary_part_ids": primary[:4],
        "supporting_part_ids": supporting[:12],
        "material_zone_hints": material_zones[:8],
        "unmapped_region_ids": unmapped[:8],
    }


def part_operator_hints(region_ids: list[str], active_ids: set[str] | None) -> dict[str, list[str]]:
    result: dict[str, list[str]] = {}
    for region_id in region_ids:
        binding = REGION_PART_TARGETS.get(region_id)
        if binding is None:
            continue
        region_hints = OPERATOR_HINTS.get(region_id, OPERATOR_HINTS["whole-body"])
        for part_id in binding["primary"] + binding["supporting"]:
            bucket = result.setdefault(part_id, [])
            for operator_id in region_hints:
                if operator_id not in bucket and (active_ids is None or operator_id in active_ids):
                    bucket.append(operator_id)
            result[part_id] = bucket[:6]
    return result


def make_action(
    action_id: str,
    round_number: int,
    stage: str,
    region_ids: list[str],
    landmark_ids: list[str],
    visibility: str,
    evidence: list[dict[str, Any]],
    active_ids: set[str] | None,
    instruction: str,
) -> dict[str, Any]:
    targets = part_targets(region_ids)
    return {
        "action_id": action_id,
        "round": round_number,
        "stage": stage,
        "priority": round_number,
        "target_region_ids": region_ids[:4],
        "target_landmark_ids": landmark_ids[:6],
        "primary_part_ids": targets["primary_part_ids"],
        "supporting_part_ids": targets["supporting_part_ids"],
        "material_zone_hints": targets["material_zone_hints"],
        "unmapped_region_ids": targets["unmapped_region_ids"],
        "part_operator_hints": part_operator_hints(region_ids, active_ids),
        "primary_failure_metric": evidence[0]["metric"] if evidence and "metric" in evidence[0] else None,
        "visibility": visibility,
        "evidence": evidence,
        "intent": "one_stable_part_change",
        "operator_hints": hints(region_ids, active_ids),
        "instruction": instruction,
        "constraints": [
            "keep camera and reference binding unchanged",
            "change one semantic Part or one material zone only",
            "rerun geometry/readback when geometry changes",
            "rerun reference_compare_prepare and quality_get",
            "retain the prior candidate if the metric worsens",
        ],
    }


def build_plan(comparison: dict[str, Any], view: dict[str, Any], review: dict[str, Any] | None, active_ids: set[str] | None) -> dict[str, Any]:
    if comparison["reference_id"] != view["reference_id"]:
        fail("comparison and view reference_id do not match")
    metrics = comparison["metrics"]
    regions = sorted_regions(view)
    landmarks = sorted_landmarks(view)
    region_ids = [item["region_id"] for item in regions]
    landmark_ids = [item["landmark_id"] for item in landmarks]
    actions: list[dict[str, Any]] = []
    gates = gate_snapshot(metrics)

    silhouette_keys = [key for key in ("silhouette_iou", "boundary_f1_4px", "bbox_edge_error", "centroid_error") if failed(metrics, key)]
    if silhouette_keys and len(actions) < MAX_ACTIONS:
        silhouette_regions = region_ids[:4] or ["whole-body"]
        actions.append(make_action(
            "fit-silhouette",
            len(actions) + 1,
            "silhouette",
            silhouette_regions,
            [],
            "observed" if regions and all(item["visibility"] == "observed" for item in regions[:4]) else "inferred",
            basis(metrics, silhouette_keys),
            active_ids,
            "Match the projected body envelope first; select one high-confidence visible region and adjust only its width, height, or placement.",
        ))

    landmark_keys = [key for key in STRUCTURE_METRICS if failed(metrics, key)]
    if gates["silhouette"]["passed"] and landmark_keys and len(actions) < MAX_ACTIONS:
        actions.append(make_action(
            "fit-landmarks",
            len(actions) + 1,
            "structure",
            region_ids,
            landmark_ids,
            "observed" if landmarks and all(item["visibility"] == "observed" for item in landmarks[:6]) else "inferred",
            basis(metrics, landmark_keys),
            active_ids,
            "Use the listed visible landmarks as projection constraints; change the single Part that controls the most landmarks and do not move the camera in the same round.",
        ))

    region_keys = [key for key in FORM_METRICS if failed(metrics, key)]
    if gates["structure"]["passed"] and region_keys and len(actions) < MAX_ACTIONS:
        actions.append(make_action(
            "fit-regions",
            len(actions) + 1,
            "form",
            region_ids,
            [],
            "observed" if regions and all(item["visibility"] == "observed" for item in regions[:4]) else "inferred",
            basis(metrics, region_keys),
            active_ids,
            "Refine one visible semantic region at a time with panel/profile/joint detail; keep hidden or cropped geometry explicitly unknown.",
        ))

    if review is not None:
        material_issues = [
            issue for issue in review.get("issues", [])
            if isinstance(issue, dict) and issue.get("pass") in {"beauty", "material-id", "normal", "ao", "uv-stretch"}
        ]
        if material_issues and len(actions) < MAX_ACTIONS and gates["surface_material_unlocked"]:
            issue_regions = [
                item["region_id"] for item in material_issues
                if isinstance(item.get("region_id"), str)
            ]
            actions.append(make_action(
                "fit-material-surface",
                len(actions) + 1,
                "material-surface",
                issue_regions or region_ids,
                [],
                "observed" if issue_regions else "inferred",
                [{"review_issue_id": item.get("issue_id"), "pass": item.get("pass"), "confidence": item.get("confidence")} for item in material_issues[:8]],
                active_ids,
                "Change one MaterialZone or surface recipe only; preserve the geometry and compare the same fixed AOV set again.",
            ))

    if not actions and comparison["status"] == "PARTIAL_VISIBLE_VIEW_PASS":
        decision = "ready_for_human_review"
    elif comparison["status"] == "BLOCKED_REFERENCE_COVERAGE":
        decision = "blocked"
    else:
        decision = "revise"

    blocked_reasons = []
    if comparison["status"] == "BLOCKED_REFERENCE_COVERAGE":
        blocked_reasons.append("reference coverage is insufficient for this view")
    if any(item.get("visibility") in {"inferred", "unknown"} for item in view["regions"] + view["landmarks"]):
        blocked_reasons.append("some targets are inferred or unknown and cannot be treated as observed geometry")
    if active_ids is None:
        blocked_reasons.append("operator catalog was not supplied; operator_hints are intentionally empty")
    if review is not None and any(
        isinstance(issue, dict) and issue.get("pass") in {"beauty", "material-id", "normal", "ao", "uv-stretch"}
        for issue in review.get("issues", [])
    ) and not gates["surface_material_unlocked"]:
        blocked_reasons.append("surface/material pass is locked until silhouette, structure and form gates pass")

    if not gates["silhouette"]["passed"]:
        current_stage = "silhouette"
    elif not gates["structure"]["passed"]:
        current_stage = "structure"
    elif not gates["form"]["passed"]:
        current_stage = "form"
    elif review is not None and any(
        isinstance(issue, dict) and issue.get("pass") in {"beauty", "material-id", "normal", "ao", "uv-stretch"}
        for issue in review.get("issues", [])
    ):
        current_stage = "material-surface"
    else:
        current_stage = "final"

    plan = {
        "schema_version": "ForgeCADReferenceFitPlan@1",
        "candidate_id": comparison["candidate_id"],
        "reference_id": comparison["reference_id"],
        "comparison_report_canonical_sha256": comparison["canonical_sha256"],
        "comparison_report_id": comparison["report_id"],
        "baseline_candidate_id": comparison["candidate_id"],
        "view_spec_canonical_sha256": view["canonical_sha256"],
        "quality_status": comparison["status"],
        "decision": decision,
        "workflow": {
            "sequence": [
                "reference-canvas",
                "silhouette-blockout",
                "landmark-structure",
                "semantic-part-fill",
                "surface-detail",
                "uv-pbr",
                "fixed-render-review",
            ],
            "current_stage": current_stage,
            "canvas": {
                "mode": "reference-overlay-flicker",
                "primary_pass": "silhouette",
                "diagnostic_pass": "part-id",
                "camera_locked": True,
                "transient_only": True,
                "runtime_truth": "ReferenceComparisonReport@1",
                "image_bytes_recorded": False,
            },
            "gates": gates,
            "surface_material_policy": "locked_until_form_gate",
        },
        "max_rounds": MAX_ACTIONS,
        "actions": actions[:MAX_ACTIONS],
        "blocked_reasons": blocked_reasons,
        "source_counts": {
            "observed_landmarks": sum(item["visibility"] == "observed" for item in view["landmarks"]),
            "observed_regions": sum(item["visibility"] == "observed" for item in view["regions"]),
            "inferred_or_unknown_targets": sum(item["visibility"] != "observed" for item in view["landmarks"] + view["regions"]),
        },
        "persistent_user_data_touched": False,
        "canonical_sha256": "",
    }
    plan["canonical_sha256"] = canonical_hash(plan)
    return plan


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--comparison", type=Path, required=True)
    parser.add_argument("--view-spec", type=Path, required=True)
    parser.add_argument("--review", type=Path)
    parser.add_argument("--operator-catalog", type=Path)
    parser.add_argument("--output", type=Path)
    return parser.parse_args()


def main() -> int:
    try:
        args = parse_args()
        comparison = validate_comparison(load_json(args.comparison))
        view = validate_view_spec(load_json(args.view_spec))
        review = validate_review(load_json(args.review)) if args.review else None
        active_ids = active_catalog_ids(load_json(args.operator_catalog)) if args.operator_catalog else None
        plan = build_plan(comparison, view, review, active_ids)
        encoded = json.dumps(plan, ensure_ascii=False, indent=2) + "\n"
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(encoded, encoding="utf-8")
        else:
            sys.stdout.write(encoded)
        return 0
    except InputError as error:
        print(f"ForgeCAD fit plan rejected: {error}", file=sys.stderr)
        return 2
    except OSError as error:
        print(f"ForgeCAD fit plan I/O error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
