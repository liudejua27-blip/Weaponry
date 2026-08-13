#!/usr/bin/env python3
"""Build a hash-only, contour-first correction plan from a Codex CLI receipt.

This helper is deliberately orchestration-only.  It never reads the reference
image, calls MCP, writes Runtime/CAS, or invents landmarks/regions.  A receipt
with no declared landmarks or regions is reported as an incomplete visual
intake, not as evidence that those parts of the model are wrong.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


THRESHOLDS = {
    "boundary_f1_4px": 0.90,
    "silhouette_iou": 0.90,
    "bbox_edge_error": 0.02,
    "centroid_error": 0.02,
    "landmark_coverage": 0.80,
    "landmark_nme": 0.03,
    "region_median_iou": 0.85,
    "critical_region_min_iou": 0.85,
}


def sha256_json(value: Any) -> str:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


def require_text(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(f"{field} must be a non-empty string")
    return value


def load_receipt(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError("receipt must be a JSON object")
    metrics = value.get("comparison_metrics")
    if not isinstance(metrics, dict):
        raise ValueError("receipt.comparison_metrics is required")
    for field in ("candidate_id", "reference_sha256", "artifact_sha256", "program_sha256", "catalog_sha256", "comparison_report_hash", "render_set_hash"):
        require_text(value.get(field), field)
    for field in THRESHOLDS:
        if not isinstance(metrics.get(field), (int, float)):
            raise ValueError(f"comparison_metrics.{field} must be numeric")
    return value


def build_plan(receipt: dict[str, Any], receipt_file: str) -> dict[str, Any]:
    metrics = receipt["comparison_metrics"]
    failed = [
        {
            "metric": field,
            "current": metrics[field],
            "target": target,
            "direction": "at_least" if field.endswith(("f1_4px", "iou", "coverage")) else "at_most",
        }
        for field, target in THRESHOLDS.items()
        if (metrics[field] < target if field.endswith(("f1_4px", "iou", "coverage")) else metrics[field] > target)
    ]
    contour_failed = [item for item in failed if item["metric"] in {"boundary_f1_4px", "silhouette_iou", "bbox_edge_error", "centroid_error"}]
    landmark_or_region_data = metrics["landmark_coverage"] > 0 or metrics["region_median_iou"] > 0
    intake_status = "complete_enough_for_visual_fit" if landmark_or_region_data else "incomplete_visual_intake"

    actions: list[dict[str, Any]] = []
    if contour_failed:
        actions.append(
            {
                "round": 1,
                "stage": "silhouette-blockout",
                "status": "unlocked",
                "primary_metrics": [item["metric"] for item in contour_failed],
                "allowed_change": "one visible contour-bearing Part or one active contour Operator stage",
                "must_preserve": ["project_id", "reference_id", "camera calibration", "catalog_sha256"],
                "locked_stages": ["landmark-structure", "semantic-part-fill", "surface-detail", "uv-pbr"],
            }
        )
    if not landmark_or_region_data:
        actions.append(
            {
                "round": len(actions) + 1,
                "stage": "visual-intake",
                "status": "required_before_landmarks_or_regions",
                "required": ["normalized visible landmarks", "declared visible regions", "observed/inferred/unknown labels"],
                "forbidden": ["inventing landmarks", "guessing hidden regions", "using zero coverage as a geometry diagnosis"],
            }
        )
    else:
        if metrics["landmark_coverage"] < THRESHOLDS["landmark_coverage"] or metrics["landmark_nme"] > THRESHOLDS["landmark_nme"]:
            actions.append(
                {
                    "round": len(actions) + 1,
                    "stage": "landmark-structure",
                    "status": "locked_until_contour_pass",
                    "primary_metrics": ["landmark_coverage", "landmark_nme"],
                    "allowed_change": "one proportion-bearing Part after the contour gate passes",
                }
            )
        if metrics["region_median_iou"] < THRESHOLDS["region_median_iou"] or metrics["critical_region_min_iou"] < THRESHOLDS["critical_region_min_iou"]:
            actions.append(
                {
                    "round": len(actions) + 1,
                    "stage": "semantic-part-fill",
                    "status": "locked_until_contour_and_landmark_pass",
                    "primary_metrics": ["region_median_iou", "critical_region_min_iou"],
                    "allowed_change": "one observed region mapped to one semantic Part",
                }
            )

    plan = {
        "schema_version": "ForgeCADMCP010FContourCorrectionPlan@1",
        "status": "READY_FOR_CONTOUR_FIRST_REVIEW" if contour_failed else "NO_CONTOUR_FAILURE_RECORDED",
        "source_receipt": {
            "receipt_file": receipt_file,
            "candidate_id": receipt["candidate_id"],
            "reference_sha256": receipt["reference_sha256"],
            "artifact_sha256": receipt["artifact_sha256"],
            "program_sha256": receipt["program_sha256"],
            "catalog_sha256": receipt["catalog_sha256"],
            "comparison_report_hash": receipt["comparison_report_hash"],
            "render_set_hash": receipt["render_set_hash"],
        },
        "metric_priority": ["boundary_f1_4px", "silhouette_iou", "bbox_edge_error", "centroid_error", "landmark_coverage", "landmark_nme", "region_median_iou", "critical_region_min_iou"],
        "failed_metrics": failed,
        "visual_intake": {
            "status": intake_status,
            "landmarks_declared": metrics["landmark_coverage"] > 0,
            "regions_declared": metrics["region_median_iou"] > 0,
            "reason": "The source receipt declares no landmarks or regions; collect image-derived normalized annotations before diagnosing those metrics." if not landmark_or_region_data else "Receipt contains visible landmark/region evidence; keep hidden areas explicitly unknown or inferred.",
        },
        "actions": actions[:5],
        "surface_material_policy": {
            "status": "locked_until_contour_and_form_pass",
            "asset_pack": receipt.get("pbr_material_pack", "NOT_RUN"),
            "rule": "Do not use material zones, emissive effects, or extra triangles to mask a failed silhouette boundary.",
        },
        "quality_claim": "ORCHESTRATION_ONLY_NO_NEW_QUALITY_RESULT",
        "persistent_user_data_touched": False,
    }
    plan["plan_sha256"] = sha256_json(plan)
    return plan


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--receipt-file", default=None)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    receipt = load_receipt(args.receipt)
    receipt_file = args.receipt_file or args.receipt.as_posix()
    output = build_plan(receipt, receipt_file)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": output["status"], "plan_sha256": output["plan_sha256"], "output": str(args.output)}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
