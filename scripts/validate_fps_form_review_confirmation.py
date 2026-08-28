#!/usr/bin/env python3
"""Validate a user-authorized FPS form-review confirmation without Runtime writes.

This validator binds an immutable V4 proposal to an explicit user decision:
all line-flow kind/continuity mappings are accepted and one conservative,
visible two-dimensional negative-space contour is supplied.  It does not read
image bytes, call Runtime/MCP, infer depth, or create quality/stage evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

from validate_fps_form_review_proposal import (
    VIEW_ORDER,
    canonical_sha256,
    fail,
    load_object,
    polygon,
    require_array,
    require_false,
    require_object,
    require_sha256,
    require_text,
    validate_proposal,
)


SCHEMA_VERSION = "ForgeCADWeaponFormReviewConfirmation@1"
ALLOWED_KEYS = {
    "schema_version",
    "status",
    "source_reference_sha256",
    "source_proposal_file_sha256",
    "source_proposal_canonical_sha256",
    "source_proposal_overlay_sha256",
    "confirmation_scope",
    "user_confirmed",
    "line_flow_confirmation",
    "outer_contour_correction",
    "negative_space_correction",
    "runtime_write",
    "worker_started",
    "candidate_match_status",
    "depth_status",
    "visual_quality_status",
    "human_visual_review_status",
}
OUTER_CORRECTION_KEYS = {
    "view_kind",
    "coordinate_space",
    "source_board_size",
    "source_crop_box_xyxy",
    "source_crop_size",
    "source_crop_sha256",
    "runtime_crop_png_sha256",
    "contour_points",
    "contour_bbox",
    "contour_source",
    "depth_status",
    "user_confirmed",
}
LINE_CONFIRMATION_KEYS = {
    "accepted_line_flow_ids",
    "accepted_mapping_count",
    "accepted_mapping_canonical_sha256",
    "mapping_semantics",
    "user_confirmed",
}
NEGATIVE_CORRECTION_KEYS = {
    "structure_id",
    "view_kind",
    "visual_role",
    "mask_operation",
    "boundary_relationship",
    "visibility",
    "depth_policy",
    "profile_policy",
    "coordinate_space",
    "source_board_size",
    "source_crop_box_xyxy",
    "source_crop_size",
    "source_crop_sha256",
    "runtime_crop_png_sha256",
    "source_overlay_sha256",
    "source_mask_sha256",
    "contour_points",
    "contour_bbox",
    "contour_source",
    "containment_status",
    "user_confirmed",
}


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def exact_keys(value: dict[str, Any], allowed: set[str], field: str) -> None:
    if set(value) != allowed:
        missing = sorted(allowed - set(value))
        unexpected = sorted(set(value) - allowed)
        fail(f"{field} keys differ: missing={missing}, unexpected={unexpected}")


def require_true(value: Any, field: str) -> None:
    if value is not True:
        fail(f"{field} must be true")


def require_positive_integer(value: Any, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        fail(f"{field} must be a positive integer")
    return value


def line_flow_mapping(proposal: dict[str, Any]) -> list[dict[str, Any]]:
    views = require_object(proposal.get("views"), "proposal.views")
    rows: list[dict[str, Any]] = []
    for view_kind in VIEW_ORDER:
        view = require_object(views.get(view_kind), f"proposal.views.{view_kind}")
        for flow_value in require_array(view.get("line_flows_v2"), f"proposal.views.{view_kind}.line_flows_v2"):
            flow = require_object(flow_value, f"proposal.views.{view_kind}.line_flows_v2[]")
            rows.append({
                "view_kind": view_kind,
                "line_flow_id": require_text(flow.get("line_flow_id"), "line_flow_id"),
                "runtime_kind": require_text(flow.get("runtime_kind_candidate"), "runtime_kind_candidate"),
                "continuity_group_id": require_text(flow.get("continuity_group_id"), "continuity_group_id"),
                "points": require_array(flow.get("points"), "line_flow points"),
            })
    return rows


def numeric_pair(value: Any, field: str) -> tuple[int, int]:
    values = require_array(value, field)
    if len(values) != 2:
        fail(f"{field} must contain two integers")
    return (
        require_positive_integer(values[0], f"{field}[0]"),
        require_positive_integer(values[1], f"{field}[1]"),
    )


def crop_box(value: Any, field: str) -> tuple[int, int, int, int]:
    values = require_array(value, field)
    if len(values) != 4 or any(isinstance(item, bool) or not isinstance(item, int) for item in values):
        fail(f"{field} must contain four integers")
    x0, y0, x1, y1 = values
    if x0 < 0 or y0 < 0 or x0 >= x1 or y0 >= y1:
        fail(f"{field} is empty, reversed, or negative")
    return x0, y0, x1, y1


def point_inside(point: tuple[float, float], outer: list[tuple[float, float]]) -> bool:
    inside = False
    x, y = point
    for index, start in enumerate(outer):
        end = outer[(index + 1) % len(outer)]
        if (start[1] > y) != (end[1] > y):
            crossing = (end[0] - start[0]) * (y - start[1]) / (end[1] - start[1]) + start[0]
            if x < crossing:
                inside = not inside
    return inside


def validate_confirmation(
    proposal: dict[str, Any],
    confirmation: dict[str, Any],
    proposal_file_sha256: str,
) -> dict[str, Any]:
    proposal_validation = validate_proposal(proposal)
    exact_keys(confirmation, ALLOWED_KEYS, "confirmation")
    if confirmation.get("schema_version") != SCHEMA_VERSION or confirmation.get("status") != "USER_CONFIRMED_2D_REVIEW_INPUT":
        fail("confirmation schema/status differs")
    require_true(confirmation.get("user_confirmed"), "confirmation.user_confirmed")
    require_false(confirmation.get("runtime_write"), "confirmation.runtime_write")
    require_false(confirmation.get("worker_started"), "confirmation.worker_started")
    if confirmation.get("confirmation_scope") != "visible-2d-reference-annotations-only":
        fail("confirmation scope must remain visible 2D reference annotations only")
    if confirmation.get("candidate_match_status") != "NOT_RUN" or confirmation.get("depth_status") != "UNKNOWN":
        fail("confirmation cannot claim candidate matching or depth")
    if confirmation.get("visual_quality_status") != "NOT_PROVEN" or confirmation.get("human_visual_review_status") != "NOT_RUN":
        fail("confirmation cannot claim visual or human review quality")

    require_sha256(confirmation.get("source_reference_sha256"), "source_reference_sha256")
    if confirmation["source_reference_sha256"] != proposal.get("source_png_sha256"):
        fail("source reference SHA-256 differs from proposal")
    if require_sha256(confirmation.get("source_proposal_file_sha256"), "source_proposal_file_sha256") != proposal_file_sha256:
        fail("source proposal file SHA-256 differs")
    if require_sha256(confirmation.get("source_proposal_canonical_sha256"), "source_proposal_canonical_sha256") != canonical_sha256(proposal):
        fail("source proposal canonical SHA-256 differs")
    if require_sha256(confirmation.get("source_proposal_overlay_sha256"), "source_proposal_overlay_sha256") != proposal.get("overlay_sha256"):
        fail("source proposal overlay SHA-256 differs")

    line_confirmation = require_object(confirmation.get("line_flow_confirmation"), "line_flow_confirmation")
    exact_keys(line_confirmation, LINE_CONFIRMATION_KEYS, "line_flow_confirmation")
    require_true(line_confirmation.get("user_confirmed"), "line_flow_confirmation.user_confirmed")
    if line_confirmation.get("mapping_semantics") != "visual-only-nonfunctional-depth-unknown":
        fail("line-flow mapping semantics must stay visual-only/nonfunctional/depth-unknown")
    mapping = line_flow_mapping(proposal)
    expected_ids = [row["line_flow_id"] for row in mapping]
    accepted_ids = require_array(line_confirmation.get("accepted_line_flow_ids"), "accepted_line_flow_ids")
    if accepted_ids != expected_ids or len(set(accepted_ids)) != len(accepted_ids):
        fail("accepted line-flow IDs must exactly match proposal order")
    if line_confirmation.get("accepted_mapping_count") != len(mapping):
        fail("accepted line-flow mapping count differs")
    if require_sha256(line_confirmation.get("accepted_mapping_canonical_sha256"), "accepted_mapping_canonical_sha256") != canonical_sha256(mapping):
        fail("accepted line-flow mapping canonical SHA-256 differs")

    outer_correction = require_object(confirmation.get("outer_contour_correction"), "outer_contour_correction")
    exact_keys(outer_correction, OUTER_CORRECTION_KEYS, "outer_contour_correction")
    require_true(outer_correction.get("user_confirmed"), "outer_contour_correction.user_confirmed")
    if outer_correction.get("view_kind") != "rear-three-quarter" or outer_correction.get("coordinate_space") != "normalized_expanded_reference_crop":
        fail("outer contour correction must bind the expanded rear-three-quarter crop")
    if outer_correction.get("depth_status") != "UNKNOWN":
        fail("outer contour correction cannot claim depth")
    if outer_correction.get("contour_source") != "codex-designed-deterministic-largest-visible-foreground-boundary-user-delegated":
        fail("outer contour source does not record delegated deterministic design")
    outer_board_width, outer_board_height = numeric_pair(outer_correction.get("source_board_size"), "outer_contour_correction.source_board_size")
    outer_crop_width, outer_crop_height = numeric_pair(outer_correction.get("source_crop_size"), "outer_contour_correction.source_crop_size")
    outer_x0, outer_y0, outer_x1, outer_y1 = crop_box(outer_correction.get("source_crop_box_xyxy"), "outer_contour_correction.source_crop_box_xyxy")
    if (outer_x1 - outer_x0, outer_y1 - outer_y0) != (outer_crop_width, outer_crop_height) or outer_x1 > outer_board_width or outer_y1 > outer_board_height:
        fail("expanded outer crop binding differs from source board bounds")
    require_sha256(outer_correction.get("source_crop_sha256"), "outer_contour_correction.source_crop_sha256")
    require_sha256(outer_correction.get("runtime_crop_png_sha256"), "outer_contour_correction.runtime_crop_png_sha256")
    outer_points, outer_area = polygon(outer_correction.get("contour_points"), "outer_contour_correction.contour_points")
    outer_declared_bbox = require_array(outer_correction.get("contour_bbox"), "outer_contour_correction.contour_bbox")
    outer_calculated_bbox = [min(item[0] for item in outer_points), min(item[1] for item in outer_points), max(item[0] for item in outer_points), max(item[1] for item in outer_points)]
    if len(outer_declared_bbox) != 4 or any(isinstance(value, bool) or not isinstance(value, (int, float)) for value in outer_declared_bbox) or any(abs(float(actual) - expected) > 1e-6 for actual, expected in zip(outer_declared_bbox, outer_calculated_bbox)):
        fail("outer contour bbox differs from points")

    correction = require_object(confirmation.get("negative_space_correction"), "negative_space_correction")
    exact_keys(correction, NEGATIVE_CORRECTION_KEYS, "negative_space_correction")
    require_true(correction.get("user_confirmed"), "negative_space_correction.user_confirmed")
    expected_constants = {
        "structure_id": "rear3q.open-stock-void",
        "view_kind": "rear-three-quarter",
        "visual_role": "open-frame",
        "mask_operation": "subtract",
        "boundary_relationship": "enclosed",
        "visibility": "observed",
        "depth_policy": "unknown",
        "profile_policy": "material-only",
        "coordinate_space": "normalized_expanded_reference_crop",
        "containment_status": "PENDING_RUNTIME_TARGET_CONTAINMENT_VALIDATION",
    }
    for field, expected in expected_constants.items():
        if correction.get(field) != expected:
            fail(f"negative_space_correction.{field} must be {expected}")
    if correction.get("contour_source") != "codex-designed-conservative-inset-user-delegated":
        fail("negative-space contour source does not record delegated conservative design")
    board_width, board_height = numeric_pair(correction.get("source_board_size"), "source_board_size")
    crop_width, crop_height = numeric_pair(correction.get("source_crop_size"), "source_crop_size")
    x0, y0, x1, y1 = crop_box(correction.get("source_crop_box_xyxy"), "source_crop_box_xyxy")
    if (x1 - x0, y1 - y0) != (crop_width, crop_height) or x1 > board_width or y1 > board_height:
        fail("expanded crop binding differs from source board bounds")
    for field in ("source_crop_sha256", "runtime_crop_png_sha256", "source_overlay_sha256", "source_mask_sha256"):
        require_sha256(correction.get(field), field)
    points, area = polygon(correction.get("contour_points"), "negative_space_correction.contour_points")
    declared_bbox = require_array(correction.get("contour_bbox"), "negative_space_correction.contour_bbox")
    if len(declared_bbox) != 4:
        fail("negative_space_correction.contour_bbox must contain four coordinates")
    calculated_bbox = [min(point[0] for point in points), min(point[1] for point in points), max(point[0] for point in points), max(point[1] for point in points)]
    if any(isinstance(value, bool) or not isinstance(value, (int, float)) for value in declared_bbox) or any(abs(float(actual) - expected) > 1e-6 for actual, expected in zip(declared_bbox, calculated_bbox)):
        fail("negative-space contour bbox differs from points")
    if correction["source_board_size"] != outer_correction["source_board_size"] or correction["source_crop_box_xyxy"] != outer_correction["source_crop_box_xyxy"] or correction["source_crop_size"] != outer_correction["source_crop_size"] or correction["source_crop_sha256"] != outer_correction["source_crop_sha256"] or correction["runtime_crop_png_sha256"] != outer_correction["runtime_crop_png_sha256"]:
        fail("negative-space correction and outer contour correction use different expanded crop bindings")
    if any(not point_inside(item, outer_points) for item in points):
        fail("negative-space contour is not strictly inside the delegated outer contour")

    result = {
        "schema_version": "ForgeCADFormReviewConfirmationValidation@1",
        "status": "READY_FOR_RUNTIME_TARGET_PREPARE",
        "proposal_file_sha256": proposal_file_sha256,
        "proposal_canonical_sha256": canonical_sha256(proposal),
        "proposal_validation_sha256": proposal_validation["validation_sha256"],
        "confirmation_canonical_sha256": canonical_sha256(confirmation),
        "accepted_line_flow_count": len(mapping),
        "accepted_line_flow_mapping_sha256": canonical_sha256(mapping),
        "negative_space_structure_id": correction["structure_id"],
        "negative_space_contour_point_count": len(points),
        "negative_space_contour_area": round(area, 12),
        "outer_contour_point_count": len(outer_points),
        "outer_contour_area": round(outer_area, 12),
        "depth_status": "UNKNOWN",
        "candidate_match_status": "NOT_RUN",
        "visual_quality_status": "NOT_PROVEN",
        "runtime_write": False,
        "worker_started": False,
        "next_required_action": "Runtime-owned target prepare, FormArt projection, and same-candidate quality evaluation",
    }
    result["validation_sha256"] = canonical_sha256(result)
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--proposal", required=True, type=Path)
    parser.add_argument("--confirmation", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    proposal_bytes = args.proposal.read_bytes()
    proposal = load_object(args.proposal, "proposal")
    confirmation = load_object(args.confirmation, "confirmation")
    result = validate_confirmation(proposal, confirmation, sha256_bytes(proposal_bytes))
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
