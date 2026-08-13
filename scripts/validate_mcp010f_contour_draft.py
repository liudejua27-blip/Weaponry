#!/usr/bin/env python3
"""Validate a Viewer contour handoff before Codex prepares a local edit.

The Viewer draft is intentionally transient.  This helper does not call MCP,
read the reference image, write Runtime/CAS, or create a quality result.  It
only proves that the user-drawn points are a bounded polygon and that the
clipboard payload is bound to the same candidate/reference/render evidence
that Codex is about to revise.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
from typing import Any


MAX_POINTS = 128
MIN_AREA = 0.0005
MIN_PERIMETER = 0.05
SHA_FIELDS = (
    "reference_sha256",
    "artifact_sha256",
    "render_set_hash",
    "comparison_report_hash",
)


def canonical_sha256(value: Any) -> str:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def fail(message: str) -> None:
    raise ValueError(message)


def text(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value:
        fail(f"{field} must be a non-empty string")
    return value


def sha256(value: Any, field: str) -> str:
    value = text(value, field)
    if len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        fail(f"{field} must be a lowercase SHA-256")
    return value


def load_object(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"{label} is not valid JSON: {exc}")
    if not isinstance(value, dict):
        fail(f"{label} must be a JSON object")
    return value


def receipt_binding(receipt: dict[str, Any]) -> dict[str, str]:
    binding = {
        "project_id": text(receipt.get("project_id"), "receipt.project_id"),
        "candidate_id": text(receipt.get("candidate_id"), "receipt.candidate_id"),
        "reference_id": text(receipt.get("reference_id"), "receipt.reference_id"),
    }
    for field in SHA_FIELDS:
        binding[field] = sha256(receipt.get(field), f"receipt.{field}")
    return binding


def point(value: Any, index: int) -> tuple[float, float]:
    if not isinstance(value, dict) or set(value) != {"x", "y"}:
        fail(f"points[{index}] must contain exactly x and y")
    x = value.get("x")
    y = value.get("y")
    if not isinstance(x, (int, float)) or not math.isfinite(x) or not 0 <= x <= 1:
        fail(f"points[{index}].x must be finite and within [0,1]")
    if not isinstance(y, (int, float)) or not math.isfinite(y) or not 0 <= y <= 1:
        fail(f"points[{index}].y must be finite and within [0,1]")
    return float(x), float(y)


def orientation(a: tuple[float, float], b: tuple[float, float], c: tuple[float, float]) -> float:
    return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])


def on_segment(a: tuple[float, float], b: tuple[float, float], c: tuple[float, float]) -> bool:
    return min(a[0], c[0]) <= b[0] <= max(a[0], c[0]) and min(a[1], c[1]) <= b[1] <= max(a[1], c[1])


def segments_cross(a: tuple[float, float], b: tuple[float, float], c: tuple[float, float], d: tuple[float, float]) -> bool:
    ab_c = orientation(a, b, c)
    ab_d = orientation(a, b, d)
    cd_a = orientation(c, d, a)
    cd_b = orientation(c, d, b)
    epsilon = 1e-9
    if ((ab_c > epsilon and ab_d < -epsilon) or (ab_c < -epsilon and ab_d > epsilon)) and ((cd_a > epsilon and cd_b < -epsilon) or (cd_a < -epsilon and cd_b > epsilon)):
        return True
    return (abs(ab_c) <= epsilon and on_segment(a, c, b)) or (abs(ab_d) <= epsilon and on_segment(a, d, b)) or (abs(cd_a) <= epsilon and on_segment(c, a, d)) or (abs(cd_b) <= epsilon and on_segment(c, b, d))


def validate_polygon(raw_points: Any) -> list[tuple[float, float]]:
    if not isinstance(raw_points, list) or not 3 <= len(raw_points) <= MAX_POINTS:
        fail(f"points must contain 3..{MAX_POINTS} entries")
    points = [point(value, index) for index, value in enumerate(raw_points)]
    for index in range(len(points)):
        previous = points[index - 1]
        current = points[index]
        if math.dist(previous, current) < 0.002:
            fail(f"points[{index}] is too close to its predecessor")
    area_twice = sum(points[index][0] * points[(index + 1) % len(points)][1] - points[(index + 1) % len(points)][0] * points[index][1] for index in range(len(points)))
    if abs(area_twice) / 2 < MIN_AREA:
        fail("contour polygon area is too small")
    perimeter = sum(math.dist(points[index], points[(index + 1) % len(points)]) for index in range(len(points)))
    if perimeter < MIN_PERIMETER:
        fail("contour polygon perimeter is too small")
    for first in range(len(points)):
        first_next = (first + 1) % len(points)
        for second in range(first + 1, len(points)):
            second_next = (second + 1) % len(points)
            if first == second or first_next == second or second_next == first:
                continue
            if first == 0 and second_next == len(points) - 1:
                continue
            if segments_cross(points[first], points[first_next], points[second], points[second_next]):
                fail(f"contour polygon self-intersects at edges {first} and {second}")
    return points


def validate_draft(draft: dict[str, Any], binding: dict[str, str], receipt: dict[str, Any]) -> dict[str, Any]:
    schema = draft.get("schema_version")
    if schema not in {"ForgeCADViewerContourDraft@1", "ForgeCADViewerContourDraft@2"}:
        fail("draft.schema_version must be ForgeCADViewerContourDraft@1 or @2")
    expected_keys_v1 = {"schema_version", "coordinate_space", "points", "closed", "transient_only", "runtime_write"}
    expected_keys_v2 = expected_keys_v1 | {"project_id", "candidate_id", "reference_id", "artifact_sha256", "render_set_hash", "comparison_report_hash", "source_pass", "selected_part_id", "selected_material_zone_id"}
    allowed = expected_keys_v2 if schema.endswith("@2") else expected_keys_v1
    unknown = set(draft) - allowed
    if unknown:
        fail(f"draft contains unsupported fields: {sorted(unknown)}")
    if draft.get("coordinate_space") != "normalized_reference_image":
        fail("draft.coordinate_space must be normalized_reference_image")
    if draft.get("closed") is not True or draft.get("transient_only") is not True or draft.get("runtime_write") is not False:
        fail("draft must be closed, transient_only=true and runtime_write=false")
    points = validate_polygon(draft.get("points"))
    if schema.endswith("@2"):
        for field in ("project_id", "candidate_id", "reference_id", "source_pass"):
            text(draft.get(field), f"draft.{field}")
        if draft["project_id"] != binding["project_id"] or draft["candidate_id"] != binding["candidate_id"] or draft["reference_id"] != binding["reference_id"]:
            fail("draft project/candidate/reference binding does not match the comparison receipt")
        for field in ("artifact_sha256", "render_set_hash", "comparison_report_hash"):
            if sha256(draft.get(field), f"draft.{field}") != binding[field]:
                fail(f"draft.{field} does not match the comparison receipt")
        if draft["source_pass"] != "silhouette":
            fail("draft.source_pass must be silhouette")
        selected_part = draft.get("selected_part_id")
        if selected_part is not None and not isinstance(selected_part, str):
            fail("draft.selected_part_id must be a string or null")
        selected_zone = draft.get("selected_material_zone_id")
        if selected_zone is not None and not isinstance(selected_zone, str):
            fail("draft.selected_material_zone_id must be a string or null")
    else:
        selected_part = None
        selected_zone = None
    part_ids = receipt.get("part_ids")
    if isinstance(part_ids, list) and selected_part not in (None, "all") and selected_part not in part_ids:
        fail("draft.selected_part_id is not present in the candidate artifact")
    material_zone_ids = receipt.get("material_zone_ids")
    if isinstance(material_zone_ids, list) and selected_zone not in (None, "all") and selected_zone not in material_zone_ids:
        fail("draft.selected_material_zone_id is not present in the candidate artifact")
    return {
        "schema_version": "ForgeCADContourCorrectionIntent@1",
        "status": "READY_FOR_SINGLE_PART_CONTOUR_EDIT" if selected_part not in (None, "all") else "CONTOUR_DRAFT_BOUND_PART_SELECTION_REQUIRED",
        "binding": binding,
        "draft": {
            "schema_version": schema,
            "draft_sha256": canonical_sha256(draft),
            "point_count": len(points),
            "closed": True,
            "coordinate_space": "normalized_reference_image",
            "source_pass": "silhouette",
            "selected_part_id": selected_part,
            "selected_material_zone_id": selected_zone,
        },
        "edit_policy": {
            "allowed_scope": "one semantic Part and one contour-bearing Operator stage",
            "preferred_operations": ["profile-extrude@1", "profile-loft@1", "panel@1", "transform@2"],
            "preserve": ["reference_id", "camera_calibration", "operator_catalog_sha256", "base_candidate_hash"],
            "rerun": ["geometry_program_hash", "geometry_prepare", "artifact_readback_get", "reference_compare_prepare", "quality_get"],
            "locked_until_pass": ["landmark-structure", "semantic-part-fill", "surface-detail", "uv-pbr", "candidate_confirm", "export_confirm"],
            "hidden_or_cropped_regions": "unknown_or_inferred_only",
        },
        "runtime_write": False,
        "persistent_user_data_touched": False,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--draft", required=True, type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    draft = load_object(args.draft, "draft")
    receipt = load_object(args.receipt, "receipt")
    result = validate_draft(draft, receipt_binding(receipt), receipt)
    result["intent_sha256"] = canonical_sha256(result)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": result["status"], "intent_sha256": result["intent_sha256"], "output": str(args.output)}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
