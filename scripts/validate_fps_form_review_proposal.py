#!/usr/bin/env python3
"""Validate an unconfirmed FPS form-review proposal without writing Runtime truth.

The proposal is Codex orchestration state.  This helper checks that proposed
Part regions and negative spaces are bounded, closed polygons and that line
flows use the Runtime's finite kind vocabulary.  It deliberately emits only a
hash/statistics receipt: it does not read image bytes, call MCP, write CAS or
SQLite, confirm a target, or create FormArt/FormQuality evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
from typing import Any


SCHEMA_VERSION = "ForgeCADWeaponFormReviewProposalV4@0"
VIEW_ORDER = ("front", "back", "left", "right", "top", "rear-three-quarter")
NEGATIVE_SPACE_VIEWS = {"left", "right", "rear-three-quarter"}
RUNTIME_LINE_KINDS = {"ridge", "valley", "seam", "light-channel", "occlusion-edge"}
FORBIDDEN_KEYS = {
    "content_base64",
    "raw_image_bytes",
    "prompt",
    "script",
    "shell",
    "secret",
    "token",
    "source_png",
    "base_proposal_path",
    "base_v2_proposal_path",
    "base_v2_matrix_path",
    "overlay_path",
}
MAX_POLYGON_POINTS = 256
MAX_LINE_POINTS = 256
MIN_POLYGON_AREA = 1e-6
EPSILON = 1e-9


def fail(message: str) -> None:
    raise ValueError(message)


def canonical_sha256(value: Any) -> str:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def load_object(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"{label} is not valid JSON: {exc}")
    if not isinstance(value, dict):
        fail(f"{label} must be a JSON object")
    return value


def require_object(value: Any, field: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        fail(f"{field} must be an object")
    return value


def require_array(value: Any, field: str) -> list[Any]:
    if not isinstance(value, list):
        fail(f"{field} must be an array")
    return value


def require_text(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value:
        fail(f"{field} must be a non-empty string")
    return value


def require_sha256(value: Any, field: str) -> str:
    text = require_text(value, field)
    if len(text) != 64 or any(character not in "0123456789abcdef" for character in text):
        fail(f"{field} must be a lowercase SHA-256")
    return text


def require_false(value: Any, field: str) -> None:
    if value is not False:
        fail(f"{field} must be false")


def point(value: Any, field: str) -> tuple[float, float]:
    if not isinstance(value, list) or len(value) != 2:
        fail(f"{field} must contain exactly two coordinates")
    x, y = value
    if isinstance(x, bool) or not isinstance(x, (int, float)) or not math.isfinite(x) or not 0 <= x <= 1:
        fail(f"{field}[0] must be finite and within [0,1]")
    if isinstance(y, bool) or not isinstance(y, (int, float)) or not math.isfinite(y) or not 0 <= y <= 1:
        fail(f"{field}[1] must be finite and within [0,1]")
    return float(x), float(y)


def orientation(a: tuple[float, float], b: tuple[float, float], c: tuple[float, float]) -> float:
    return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])


def on_segment(a: tuple[float, float], b: tuple[float, float], c: tuple[float, float]) -> bool:
    return min(a[0], c[0]) - EPSILON <= b[0] <= max(a[0], c[0]) + EPSILON and min(a[1], c[1]) - EPSILON <= b[1] <= max(a[1], c[1]) + EPSILON


def segments_cross(a: tuple[float, float], b: tuple[float, float], c: tuple[float, float], d: tuple[float, float]) -> bool:
    ab_c, ab_d = orientation(a, b, c), orientation(a, b, d)
    cd_a, cd_b = orientation(c, d, a), orientation(c, d, b)
    if ((ab_c > EPSILON and ab_d < -EPSILON) or (ab_c < -EPSILON and ab_d > EPSILON)) and ((cd_a > EPSILON and cd_b < -EPSILON) or (cd_a < -EPSILON and cd_b > EPSILON)):
        return True
    return (
        (abs(ab_c) <= EPSILON and on_segment(a, c, b))
        or (abs(ab_d) <= EPSILON and on_segment(a, d, b))
        or (abs(cd_a) <= EPSILON and on_segment(c, a, d))
        or (abs(cd_b) <= EPSILON and on_segment(c, b, d))
    )


def polygon(raw: Any, field: str) -> tuple[list[tuple[float, float]], float]:
    values = require_array(raw, field)
    if not 3 <= len(values) <= MAX_POLYGON_POINTS:
        fail(f"{field} must contain 3..{MAX_POLYGON_POINTS} points")
    points = [point(value, f"{field}[{index}]") for index, value in enumerate(values)]
    for index, current in enumerate(points):
        if math.dist(current, points[index - 1]) <= EPSILON:
            fail(f"{field}[{index}] duplicates its predecessor")
    for first in range(len(points)):
        first_next = (first + 1) % len(points)
        for second in range(first + 1, len(points)):
            second_next = (second + 1) % len(points)
            if first == second or first_next == second or second_next == first:
                continue
            if segments_cross(points[first], points[first_next], points[second], points[second_next]):
                fail(f"{field} self-intersects at edges {first} and {second}")
    twice_area = sum(points[index][0] * points[(index + 1) % len(points)][1] - points[(index + 1) % len(points)][0] * points[index][1] for index in range(len(points)))
    area = abs(twice_area) / 2
    if area < MIN_POLYGON_AREA:
        fail(f"{field} area is too small")
    return points, area


def bbox(value: Any, field: str) -> list[float]:
    values = require_array(value, field)
    if len(values) != 4:
        fail(f"{field} must contain x_min,y_min,x_max,y_max")
    coordinates = []
    for index, raw in enumerate(values):
        if isinstance(raw, bool) or not isinstance(raw, (int, float)) or not math.isfinite(raw) or not 0 <= raw <= 1:
            fail(f"{field}[{index}] must be finite and within [0,1]")
        coordinates.append(float(raw))
    if coordinates[0] >= coordinates[2] or coordinates[1] >= coordinates[3]:
        fail(f"{field} is empty or inverted")
    return coordinates


def require_review_coordinate_binding(value: dict[str, Any], field: str) -> None:
    if value.get("source_crop_coordinate_space") != "normalized_crop_pixels":
        fail(f"{field}.source_crop_coordinate_space must be normalized_crop_pixels")
    if value.get("target_coordinate_space") != "normalized_aspect_fit_512":
        fail(f"{field}.target_coordinate_space must be normalized_aspect_fit_512")
    if "within_declared_bbox" in value:
        fail(f"{field}.within_declared_bbox is an unsupported cross-space truth claim")
    if value.get("cross_space_containment_status") != "not_evaluated_missing_transform":
        fail(f"{field}.cross_space_containment_status must be not_evaluated_missing_transform")


def reject_forbidden_keys(value: Any, path: str = "proposal") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if key in FORBIDDEN_KEYS:
                fail(f"{path}.{key} is forbidden in review orchestration state")
            reject_forbidden_keys(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            reject_forbidden_keys(child, f"{path}[{index}]")


def validate_proposal(proposal: dict[str, Any]) -> dict[str, Any]:
    reject_forbidden_keys(proposal)
    if proposal.get("schema_version") != SCHEMA_VERSION or proposal.get("status") != "PROPOSAL_REVIEW_PENDING":
        fail("proposal schema/status is not the bounded V3 review proposal")
    if proposal.get("proposal_only") is not True:
        fail("proposal.proposal_only must be true")
    require_text(proposal.get("source_asset_name"), "proposal.source_asset_name")
    require_sha256(proposal.get("source_png_sha256"), "proposal.source_png_sha256")
    overlay_filename = require_text(proposal.get("overlay_filename"), "proposal.overlay_filename")
    if "/" in overlay_filename or "\\" in overlay_filename:
        fail("proposal.overlay_filename must be a basename")
    require_sha256(proposal.get("overlay_sha256"), "proposal.overlay_sha256")
    for field in ("user_confirmed", "runtime_write", "worker_started", "receipt_created"):
        require_false(proposal.get(field), f"proposal.{field}")
    order = require_array(proposal.get("view_order"), "proposal.view_order")
    if tuple(order) != VIEW_ORDER:
        fail("proposal.view_order must be the exact six identity views")
    views = require_object(proposal.get("views"), "proposal.views")
    if set(views) != set(VIEW_ORDER):
        fail("proposal.views must contain exactly the six identity views")

    totals = {"outer_contours": 0, "landmarks": 0, "part_polygons": 0, "negative_polygons": 0, "line_flows": 0}
    seen_ids: set[str] = set()
    for view_kind in VIEW_ORDER:
        view = require_object(views[view_kind], f"views.{view_kind}")
        if view.get("view_kind") != view_kind:
            fail(f"views.{view_kind}.view_kind differs")
        require_false(view.get("user_confirmed"), f"views.{view_kind}.user_confirmed")
        polygon(view.get("outer_contour_points"), f"views.{view_kind}.outer_contour_points")
        totals["outer_contours"] += 1

        landmarks = require_array(view.get("landmarks"), f"views.{view_kind}.landmarks")
        for index, landmark_value in enumerate(landmarks):
            landmark = require_object(landmark_value, f"views.{view_kind}.landmarks[{index}]")
            identifier = require_text(landmark.get("landmark_id"), "landmark_id")
            if identifier in seen_ids:
                fail(f"duplicate annotation id {identifier}")
            seen_ids.add(identifier)
            point(landmark.get("point"), f"landmark {identifier}.point")
            require_false(landmark.get("user_confirmed"), f"landmark {identifier}.user_confirmed")
            if landmark.get("runtime_visibility_before_confirmation") != "unknown":
                fail(f"landmark {identifier} must remain Runtime unknown before confirmation")
        totals["landmarks"] += len(landmarks)

        parts = require_array(view.get("part_regions_v3"), f"views.{view_kind}.part_regions_v3")
        for index, part_value in enumerate(parts):
            part = require_object(part_value, f"views.{view_kind}.part_regions_v3[{index}]")
            identifier = require_text(part.get("structure_id"), "part structure_id")
            if identifier in seen_ids:
                fail(f"duplicate annotation id {identifier}")
            seen_ids.add(identifier)
            bbox(part.get("bbox"), f"part {identifier}.bbox")
            require_review_coordinate_binding(part, f"part {identifier}")
            polygon(part.get("closed_contour_points"), f"part {identifier}.closed_contour_points")
            if part.get("closed") is not True or part.get("normalized") is not True:
                fail(f"part {identifier} must be declared closed and normalized")
            if part.get("contour_status") != "CLOSED_POLYGON_PROPOSAL_NOT_AUTHORITY" or part.get("contour_provenance") != "proposal":
                fail(f"part {identifier} must remain a non-authoritative proposal")
            if part.get("proposed_visibility") != "inferred" or part.get("semantic_visibility") != "inferred" or part.get("runtime_visibility") != "unknown":
                fail(f"part {identifier} must remain inferred/Runtime-unknown")
            if part.get("requires_user_confirmation") is not True or part.get("requires_runtime_part_index_binding") is not True:
                fail(f"part {identifier} must require confirmation and Runtime Part binding")
            require_false(part.get("user_confirmed"), f"part {identifier}.user_confirmed")
        totals["part_polygons"] += len(parts)

        negative = require_object(view.get("negative_space_v2"), f"views.{view_kind}.negative_space_v2")
        require_false(negative.get("user_confirmed"), f"views.{view_kind}.negative_space_v2.user_confirmed")
        negative_regions = require_array(negative.get("regions"), f"views.{view_kind}.negative_space_v2.regions")
        if view_kind not in NEGATIVE_SPACE_VIEWS:
            if negative_regions or negative.get("status") != "not-applicable-zero-rows-unconfirmed-proposal":
                fail(f"{view_kind} must keep negative-space zero-row/not-applicable")
        else:
            if negative.get("status") != "CLOSED_POLYGON_PROPOSAL_NOT_AUTHORITY" or not negative_regions:
                fail(f"{view_kind} must contain non-authoritative negative-space polygon proposals")
        for index, region_value in enumerate(negative_regions):
            region = require_object(region_value, f"views.{view_kind}.negative_space_v2.regions[{index}]")
            identifier = require_text(region.get("structure_id"), "negative structure_id")
            if identifier in seen_ids:
                fail(f"duplicate annotation id {identifier}")
            seen_ids.add(identifier)
            bbox(region.get("bbox"), f"negative {identifier}.bbox")
            require_review_coordinate_binding(region, f"negative {identifier}")
            polygon(region.get("closed_contour_points"), f"negative {identifier}.closed_contour_points")
            if region.get("visual_role") != "open-frame" or region.get("mask_operation") != "subtract" or region.get("boundary_relationship") != "enclosed":
                fail(f"negative {identifier} semantics are not open-frame/subtract/enclosed")
            if region.get("contour_status") != "CLOSED_POLYGON_PROPOSAL_NOT_AUTHORITY" or region.get("runtime_visibility") != "unknown":
                fail(f"negative {identifier} must remain non-authoritative/Runtime-unknown")
            if region.get("requires_user_confirmation") is not True:
                fail(f"negative {identifier} must require user confirmation")
            require_false(region.get("user_confirmed"), f"negative {identifier}.user_confirmed")
        totals["negative_polygons"] += len(negative_regions)

        flows = require_array(view.get("line_flows_v2"), f"views.{view_kind}.line_flows_v2")
        for index, flow_value in enumerate(flows):
            flow = require_object(flow_value, f"views.{view_kind}.line_flows_v2[{index}]")
            identifier = require_text(flow.get("line_flow_id"), "line_flow_id")
            if identifier in seen_ids:
                fail(f"duplicate annotation id {identifier}")
            seen_ids.add(identifier)
            kind = require_text(flow.get("runtime_kind_candidate"), f"line flow {identifier}.runtime_kind_candidate")
            if kind not in RUNTIME_LINE_KINDS:
                fail(f"line flow {identifier} uses unsupported Runtime kind {kind}")
            require_text(flow.get("continuity_group_id"), f"line flow {identifier}.continuity_group_id")
            values = require_array(flow.get("points"), f"line flow {identifier}.points")
            if not 2 <= len(values) <= MAX_LINE_POINTS:
                fail(f"line flow {identifier} must contain 2..{MAX_LINE_POINTS} points")
            for point_index, value in enumerate(values):
                point(value, f"line flow {identifier}.points[{point_index}]")
            if flow.get("runtime_visibility") != "unknown" or flow.get("requires_user_confirmation") is not True:
                fail(f"line flow {identifier} must remain Runtime-unknown and require confirmation")
            require_false(flow.get("user_confirmed"), f"line flow {identifier}.user_confirmed")
        totals["line_flows"] += len(flows)

    result = {
        "schema_version": "ForgeCADFormReviewProposalValidation@1",
        "status": "READY_FOR_USER_REVIEW_NOT_RUNTIME_WRITE",
        "proposal_schema_version": SCHEMA_VERSION,
        "proposal_canonical_sha256": canonical_sha256(proposal),
        "view_order": list(VIEW_ORDER),
        "counts": totals,
        "coordinate_binding_policy": "source-bbox-normalized-crop-pixels-target-contour-normalized-aspect-fit-512-no-cross-space-containment-claim",
        "epistemic_policy": "proposal-inferred-or-unknown-user-confirmation-does-not-promote-to-observed",
        "quality_status": "QUALITY_TARGET_NOT_MET",
        "visual_quality_status": "NOT_PROVEN",
        "runtime_write": False,
        "persistent_user_data_touched": False,
        "next_required_action": "explicit user review of P polygons, N polygons and F kind/continuity mappings",
    }
    result["validation_sha256"] = canonical_sha256(result)
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--proposal", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    result = validate_proposal(load_object(args.proposal, "proposal"))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": result["status"], "validation_sha256": result["validation_sha256"], "output": str(args.output)}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
