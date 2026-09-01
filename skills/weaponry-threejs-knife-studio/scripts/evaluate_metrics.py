#!/usr/bin/env python3
"""Deterministic mathematical evaluation for the Three.js knife slice.

The evaluator consumes a closed ``KnifeSceneProgram@1`` and, when available,
a closed 2-D contour reference.  It deliberately stays below the render
boundary: a program is sampled into a planar polygon, both polygons are
rasterized with a fixed scanline algorithm, and the resulting masks/point
sets are compared.  No Three.js, browser, image decoder, or Runtime write is
involved.

This is an evidence-producing measurement tool, not an approval tool.  A
successful mathematical measurement is reported as ``MEASURED_NOT_APPROVED``;
missing reference/camera evidence or failed geometry is ``NOT_RUN``.  The
script never emits visual or commercial acceptance.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import re
import sys
from pathlib import Path
from typing import Any, Iterable, Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

try:
    import search_candidates as _search
except ImportError as exc:  # pragma: no cover - only useful for a broken install
    raise RuntimeError("search_candidates.py is required beside evaluate_metrics.py") from exc


ROOT = SCRIPT_DIR.parent
DEFAULT_PROGRAM = ROOT / "references" / "dragonfang-first-slice.json"

REFERENCE_SCHEMA_VERSION = "KnifeContourReference@1"
RECEIPT_SCHEMA_VERSION = "KnifeMetricEvaluationReceipt@1"
COORDINATE_SPACE = "unit-square@1"
CAMERA_PROJECTION = "orthographic-normalized@1"
DEFAULT_GRID_SIZE = 128
DEFAULT_BOUNDARY_TOLERANCE_PX = 1
MIN_GRID_SIZE = 8
MAX_GRID_SIZE = 512
MAX_BOUNDARY_TOLERANCE_PX = 8
PROGRAM_SAMPLE_COUNT = 129
MAX_CONTOUR_POINTS = 2048
# Strict visible-view thresholds used by the current knife/weapon quality
# contract.  These are mathematical gates only; passing them never changes
# the receipt's approval status.
SILHOUETTE_IOU_MIN = 0.90
BOUNDARY_F1_MIN = 0.90
SYMMETRIC_CHAMFER_MAX = 0.03
P95_CONTOUR_DISTANCE_MAX = 0.05
LANDMARK_ERROR_MAX = 0.03
LANDMARK_IDS = ("root", "shoulder", "belly", "tip")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
ID_RE = re.compile(r"^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$")

REFERENCE_KEYS = {
    "schema_version",
    "reference_id",
    "coordinate_space",
    "outer_contour",
    "landmarks",
    "camera_frame",
    "canonical_sha256",
}
LANDMARK_KEYS = {"landmark_id", "point"}
CAMERA_FRAME_KEYS = {"frame_id", "projection", "x_min", "x_max", "y_min", "y_max"}

RECEIPT_KEYS = {
    "schema_version",
    "evaluation_id",
    "program_sha256",
    "reference_sha256",
    "grid_size",
    "boundary_tolerance_px",
    "geometry",
    "metrics",
    "hard_gates",
    "statuses",
    "provenance",
    "canonical_sha256",
}
STATUS_VALUES = {"PASS", "FAIL", "NOT_RUN"}
QUALITY_VALUES = {"NOT_RUN", "MEASURED_NOT_APPROVED"}
GEOMETRY_GATE_KEYS = {
    "finite_values",
    "independent_spine_edge_ids",
    "sections_strictly_monotonic",
    "required_four_sections_present",
    "positive_section_width_and_thickness",
    "tip_converges_without_zero_section",
    "spine_edge_separation",
    "nondegenerate_curve_samples",
    "spine_longitudinal_order",
    "edge_longitudinal_order",
    "spine_curve_no_planar_self_intersection",
    "edge_curve_no_planar_self_intersection",
    "estimated_triangle_budget",
}
GATE_CHECK_KEYS = {
    "geometry": GEOMETRY_GATE_KEYS,
    "reference": {"reference_present", "reference_schema_valid", "four_landmarks_present", "outer_contour_valid"},
    "camera_binding": {"declared_frame_present", "declared_projection_supported", "runtime_camera_identity_verified"},
    "metric_computability": {"measurement_executed", "predicted_contour_simple", "predicted_mask_nonempty", "reference_mask_nonempty", "predicted_boundary_nonempty", "reference_boundary_nonempty", "four_landmark_errors_computed"},
    "quality_thresholds": {"silhouette_iou", "boundary_f1", "symmetric_chamfer", "p95_contour_distance", "landmark_error"},
}


class EvaluationInputError(ValueError):
    """A malformed or unsupported closed evaluation input."""


def canonical_bytes(value: Any) -> bytes:
    """Return deterministic JSON bytes used by all hashes in this script."""

    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def canonical_sha256(value: Any) -> str:
    """Hash a document after blanking its own canonical field."""

    draft = copy.deepcopy(value)
    if isinstance(draft, dict) and "canonical_sha256" in draft:
        draft["canonical_sha256"] = ""
    return hashlib.sha256(canonical_bytes(draft)).hexdigest()


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise EvaluationInputError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=_reject_duplicate_keys)
    except OSError as exc:
        raise EvaluationInputError(f"cannot read {path}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise EvaluationInputError(f"invalid JSON in {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise EvaluationInputError(f"{path} must contain a JSON object")
    return value


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise EvaluationInputError(message)


def _exact_keys(value: Any, expected: set[str], label: str) -> None:
    _require(isinstance(value, dict), f"{label} must be an object")
    _require(set(value) == expected, f"{label} keys are not closed")


def _finite_number(value: Any, label: str, minimum: float | None = None, maximum: float | None = None) -> None:
    _require(isinstance(value, (int, float)) and not isinstance(value, bool), f"{label} must be numeric")
    value_float = float(value)
    _require(math.isfinite(value_float), f"{label} must be finite")
    if minimum is not None:
        _require(value_float >= minimum, f"{label} must be at least {minimum}")
    if maximum is not None:
        _require(value_float <= maximum, f"{label} must be at most {maximum}")


def _finite_unit_point(point: Any, label: str) -> None:
    _require(isinstance(point, list) and len(point) == 2, f"{label} must be a 2-D point")
    _finite_number(point[0], f"{label}[0]", 0.0, 1.0)
    _finite_number(point[1], f"{label}[1]", 0.0, 1.0)


def _id(value: Any, label: str) -> None:
    _require(isinstance(value, str) and 1 <= len(value) <= 64, f"{label} must be an ID")
    _require(ID_RE.fullmatch(value) is not None, f"{label} is not a closed ID")


def _sha(value: Any, label: str, allow_empty: bool = False) -> None:
    _require(isinstance(value, str), f"{label} must be a SHA-256 string")
    if allow_empty and value == "":
        return
    _require(SHA256_RE.fullmatch(value) is not None, f"{label} is not a lowercase SHA-256")


def _validate_contour(contour: Any, label: str = "reference.outer_contour") -> list[list[float]]:
    _require(isinstance(contour, list), f"{label} must be a list")
    _require(3 <= len(contour) <= MAX_CONTOUR_POINTS, f"{label} count is outside [3,{MAX_CONTOUR_POINTS}]")
    result: list[list[float]] = []
    for index, point in enumerate(contour):
        _finite_unit_point(point, f"{label}[{index}]")
        result.append([float(point[0]), float(point[1])])
    if len(result) > 3 and result[0] == result[-1]:
        result.pop()
    _require(len(result) >= 3, f"{label} must contain at least three unique vertices")
    _require(all(left != right for left, right in zip(result, result[1:] + result[:1])), f"{label} has duplicate adjacent vertices")
    _require(abs(_polygon_area(result)) > 1e-12, f"{label} has zero area")
    _require(not _polygon_self_intersects(result), f"{label} self-intersects")
    return result


def _validate_camera_frame(frame: Any) -> None:
    if frame is None:
        return
    _exact_keys(frame, CAMERA_FRAME_KEYS, "reference.camera_frame")
    _id(frame["frame_id"], "reference.camera_frame.frame_id")
    _require(frame["projection"] == CAMERA_PROJECTION, "reference.camera_frame.projection is unsupported")
    _finite_number(frame["x_min"], "reference.camera_frame.x_min")
    _finite_number(frame["x_max"], "reference.camera_frame.x_max")
    _finite_number(frame["y_min"], "reference.camera_frame.y_min")
    _finite_number(frame["y_max"], "reference.camera_frame.y_max")
    _require(float(frame["x_min"]) < float(frame["x_max"]), "reference.camera_frame x range is empty")
    _require(float(frame["y_min"]) < float(frame["y_max"]), "reference.camera_frame y range is empty")


def validate_reference(reference: dict[str, Any]) -> str:
    """Validate the closed contour reference and return its computed hash."""

    _exact_keys(reference, REFERENCE_KEYS, "KnifeContourReference")
    _require(reference["schema_version"] == REFERENCE_SCHEMA_VERSION, "reference schema version drifted")
    _id(reference["reference_id"], "reference.reference_id")
    _require(reference["coordinate_space"] == COORDINATE_SPACE, "reference.coordinate_space drifted")
    contour = _validate_contour(reference["outer_contour"])

    landmarks = reference["landmarks"]
    _require(isinstance(landmarks, list) and len(landmarks) == len(LANDMARK_IDS), "reference.landmarks must contain four entries")
    _require([item.get("landmark_id") for item in landmarks] == list(LANDMARK_IDS), "reference.landmarks order is not closed")
    for index, item in enumerate(landmarks):
        _exact_keys(item, LANDMARK_KEYS, f"reference.landmarks[{index}]")
        _require(item["landmark_id"] == LANDMARK_IDS[index], f"reference.landmarks[{index}] has the wrong role")
        _finite_unit_point(item["point"], f"reference.landmarks[{index}].point")

    _validate_camera_frame(reference["camera_frame"])
    _sha(reference["canonical_sha256"], "reference.canonical_sha256", allow_empty=True)
    digest = canonical_sha256(reference)
    if reference["canonical_sha256"]:
        _require(reference["canonical_sha256"] == digest, "reference canonical hash does not match canonical JSON")
    # Keep this explicit so a future edit cannot accidentally validate a
    # normalized local copy while hashing a different source shape.
    _require(len(contour) >= 3, "reference contour became empty after closure normalization")
    return digest


def _polygon_area(points: Sequence[Sequence[float]]) -> float:
    return 0.5 * sum(
        float(points[index][0]) * float(points[(index + 1) % len(points)][1])
        - float(points[(index + 1) % len(points)][0]) * float(points[index][1])
        for index in range(len(points))
    )


def _orientation(a: Sequence[float], b: Sequence[float], c: Sequence[float]) -> float:
    return (float(b[0]) - float(a[0])) * (float(c[1]) - float(a[1])) - (float(b[1]) - float(a[1])) * (float(c[0]) - float(a[0]))


def _on_segment(a: Sequence[float], b: Sequence[float], p: Sequence[float]) -> bool:
    epsilon = 1e-12
    return (
        min(float(a[0]), float(b[0])) - epsilon <= float(p[0]) <= max(float(a[0]), float(b[0])) + epsilon
        and min(float(a[1]), float(b[1])) - epsilon <= float(p[1]) <= max(float(a[1]), float(b[1])) + epsilon
        and abs(_orientation(a, b, p)) <= epsilon
    )


def _segments_intersect(a: Sequence[float], b: Sequence[float], c: Sequence[float], d: Sequence[float]) -> bool:
    ab_c = _orientation(a, b, c)
    ab_d = _orientation(a, b, d)
    cd_a = _orientation(c, d, a)
    cd_b = _orientation(c, d, b)
    epsilon = 1e-12
    if ((ab_c > epsilon and ab_d < -epsilon) or (ab_c < -epsilon and ab_d > epsilon)) and ((cd_a > epsilon and cd_b < -epsilon) or (cd_a < -epsilon and cd_b > epsilon)):
        return True
    return (
        (abs(ab_c) <= epsilon and _on_segment(a, b, c))
        or (abs(ab_d) <= epsilon and _on_segment(a, b, d))
        or (abs(cd_a) <= epsilon and _on_segment(c, d, a))
        or (abs(cd_b) <= epsilon and _on_segment(c, d, b))
    )


def _polygon_self_intersects(points: Sequence[Sequence[float]]) -> bool:
    count = len(points)
    for first in range(count):
        a = points[first]
        b = points[(first + 1) % count]
        for second in range(first + 1, count):
            # Adjacent edges and the first/last closure edge share an
            # endpoint by construction; that is not a self-intersection.
            if second == first + 1 or (first == 0 and second == count - 1):
                continue
            c = points[second]
            d = points[(second + 1) % count]
            if _segments_intersect(a, b, c, d):
                return True
    return False


def _program_outline(program: dict[str, Any]) -> tuple[list[list[float]], dict[str, list[float]], dict[str, int]]:
    """Project the program's two blade curves into a normalized 2-D outline."""

    _search.validate_program(program)
    blade = program["blade_surface"]
    longitudinal = _search._dominant_longitudinal_axis(program)
    non_longitudinal = [axis for axis in range(3) if axis != longitudinal]
    spine = _search._sample_curve(blade["spine_curve"], PROGRAM_SAMPLE_COUNT)
    edge = _search._sample_curve(blade["cutting_edge_curve"], PROGRAM_SAMPLE_COUNT)
    lateral = max(
        non_longitudinal,
        key=lambda axis: max(point[axis] for point in spine + edge) - min(point[axis] for point in spine + edge),
    )
    projected = [[float(point[longitudinal]), float(point[lateral])] for point in spine + list(reversed(edge))]
    compact: list[list[float]] = []
    for point in projected:
        if not compact or point != compact[-1]:
            compact.append(point)
    if len(compact) > 3 and compact[0] == compact[-1]:
        compact.pop()
    _require(len(compact) >= 3, "program blade outline is degenerate")

    x_min = min(point[0] for point in compact)
    x_max = max(point[0] for point in compact)
    y_min = min(point[1] for point in compact)
    y_max = max(point[1] for point in compact)
    x_span = x_max - x_min
    y_span = y_max - y_min
    _require(x_span > 1e-12 and y_span > 1e-12, "program blade outline has zero projected span")
    scale = max(x_span, y_span)
    margin = 0.06
    usable = 1.0 - 2.0 * margin

    def normalize(point: Sequence[float]) -> list[float]:
        return [margin + (float(point[0]) - x_min) / scale * usable, margin + (float(point[1]) - y_min) / scale * usable]

    contour = [normalize(point) for point in compact]
    section_by_role = {section["role"]: section for section in blade["sections"]}
    landmarks: dict[str, list[float]] = {}
    for role in LANDMARK_IDS:
        t = float(section_by_role[role]["u"])
        spine_point = _search._sample_curve(blade["spine_curve"], PROGRAM_SAMPLE_COUNT)[round(t * (PROGRAM_SAMPLE_COUNT - 1))]
        edge_point = _search._sample_curve(blade["cutting_edge_curve"], PROGRAM_SAMPLE_COUNT)[round(t * (PROGRAM_SAMPLE_COUNT - 1))]
        midpoint = [(spine_point[axis] + edge_point[axis]) / 2.0 for axis in range(3)]
        landmarks[role] = normalize([midpoint[longitudinal], midpoint[lateral]])
    return contour, landmarks, {"longitudinal": longitudinal, "lateral": lateral}


def build_reference_from_program(program: dict[str, Any], reference_id: str | None = None) -> dict[str, Any]:
    """Build a deterministic mathematical smoke reference, never a visual target."""

    contour, landmarks, _ = _program_outline(program)
    asset_id = str(program["asset_id"])
    generated_id = reference_id or f"{asset_id}-math-reference"
    reference = {
        "schema_version": REFERENCE_SCHEMA_VERSION,
        "reference_id": generated_id,
        "coordinate_space": COORDINATE_SPACE,
        "outer_contour": contour,
        "landmarks": [{"landmark_id": role, "point": landmarks[role]} for role in LANDMARK_IDS],
        "camera_frame": {
            "frame_id": "synthetic-unit-square-1",
            "projection": CAMERA_PROJECTION,
            "x_min": 0.0,
            "x_max": 1.0,
            "y_min": 0.0,
            "y_max": 1.0,
        },
        "canonical_sha256": "",
    }
    validate_reference(reference)
    return reference


Mask = list[list[bool]]
Point = tuple[float, float]
Pixel = tuple[int, int]


def _validate_grid(grid_size: int) -> int:
    _require(isinstance(grid_size, int) and not isinstance(grid_size, bool), "grid_size must be an integer")
    _require(MIN_GRID_SIZE <= grid_size <= MAX_GRID_SIZE, f"grid_size must be within [{MIN_GRID_SIZE},{MAX_GRID_SIZE}]")
    return grid_size


def _validate_tolerance(tolerance_px: int) -> int:
    _require(isinstance(tolerance_px, int) and not isinstance(tolerance_px, bool), "boundary_tolerance_px must be an integer")
    _require(0 <= tolerance_px <= MAX_BOUNDARY_TOLERANCE_PX, f"boundary_tolerance_px must be within [0,{MAX_BOUNDARY_TOLERANCE_PX}]")
    return tolerance_px


def _rasterize_polygon(contour: Sequence[Sequence[float]], width: int, height: int) -> Mask:
    """Rasterize with a deterministic half-open scanline fill.

    Pixel centers are used for inclusion.  The half-open edge rule avoids
    vertex double-counting and keeps adjacent horizontal runs deterministic.
    """

    _require(width > 0 and height > 0, "raster dimensions must be positive")
    mask: Mask = [[False for _ in range(width)] for _ in range(height)]
    count = len(contour)
    for pixel_y in range(height):
        y = (pixel_y + 0.5) / float(height)
        intersections: list[float] = []
        for index in range(count):
            left = contour[index]
            right = contour[(index + 1) % count]
            y_left = float(left[1])
            y_right = float(right[1])
            if y_left == y_right:
                continue
            if not ((y_left <= y < y_right) or (y_right <= y < y_left)):
                continue
            fraction = (y - y_left) / (y_right - y_left)
            intersections.append(float(left[0]) + fraction * (float(right[0]) - float(left[0])))
        intersections.sort()
        for index in range(0, len(intersections) - 1, 2):
            x_left = intersections[index]
            x_right = intersections[index + 1]
            if x_left > x_right:
                x_left, x_right = x_right, x_left
            first = max(0, int(math.ceil(x_left * width - 0.5)))
            last_exclusive = min(width, int(math.ceil(x_right * width - 0.5)))
            for pixel_x in range(first, last_exclusive):
                center_x = (pixel_x + 0.5) / float(width)
                if x_left <= center_x < x_right:
                    mask[pixel_y][pixel_x] = True
    return mask


def _validate_mask(mask: Any, label: str = "mask") -> Mask:
    _require(isinstance(mask, list) and mask, f"{label} must be a non-empty 2-D list")
    width: int | None = None
    result: Mask = []
    for row_index, row in enumerate(mask):
        _require(isinstance(row, list) and row, f"{label}[{row_index}] must be a non-empty row")
        if width is None:
            width = len(row)
        _require(len(row) == width, f"{label} rows must have equal width")
        converted: list[bool] = []
        for column_index, value in enumerate(row):
            _require(isinstance(value, bool) or (isinstance(value, int) and not isinstance(value, bool) and value in (0, 1)), f"{label}[{row_index}][{column_index}] must be boolean or binary integer")
            converted.append(bool(value))
        result.append(converted)
    _require(width is not None, f"{label} has no rows")
    return result


def _mask_shape(mask: Mask) -> tuple[int, int]:
    return len(mask[0]), len(mask)


def _assert_same_mask_shape(left: Mask, right: Mask) -> tuple[int, int]:
    left_width, left_height = _mask_shape(left)
    right_width, right_height = _mask_shape(right)
    _require((left_width, left_height) == (right_width, right_height), "masks must have identical dimensions")
    return left_width, left_height


def silhouette_iou(predicted: Mask, reference: Mask) -> float:
    """Return the binary-mask intersection over union."""

    predicted = _validate_mask(predicted, "predicted_mask")
    reference = _validate_mask(reference, "reference_mask")
    _assert_same_mask_shape(predicted, reference)
    intersection = sum(1 for pred_row, ref_row in zip(predicted, reference) for pred, ref in zip(pred_row, ref_row) if pred and ref)
    union = sum(1 for pred_row, ref_row in zip(predicted, reference) for pred, ref in zip(pred_row, ref_row) if pred or ref)
    _require(union > 0, "silhouette IoU is undefined for two empty masks")
    return intersection / float(union)


def _boundary_pixels(mask: Mask) -> list[Pixel]:
    width, height = _mask_shape(mask)
    pixels: list[Pixel] = []
    for y in range(height):
        for x in range(width):
            if not mask[y][x]:
                continue
            if x == 0 or x == width - 1 or y == 0 or y == height - 1:
                pixels.append((x, y))
                continue
            if not (mask[y][x - 1] and mask[y][x + 1] and mask[y - 1][x] and mask[y + 1][x]):
                pixels.append((x, y))
    return pixels


def _within_tolerance(point: Pixel, candidates: Sequence[Pixel], tolerance_px: int) -> bool:
    limit = tolerance_px * tolerance_px
    return any((point[0] - other[0]) ** 2 + (point[1] - other[1]) ** 2 <= limit for other in candidates)


def _boundary_f1_stats(predicted: Mask, reference: Mask, tolerance_px: int) -> tuple[float, float, float, list[Pixel], list[Pixel]]:
    _assert_same_mask_shape(predicted, reference)
    _validate_tolerance(tolerance_px)
    predicted_boundary = _boundary_pixels(predicted)
    reference_boundary = _boundary_pixels(reference)
    _require(predicted_boundary and reference_boundary, "Boundary F1 is undefined when a boundary is empty")
    true_positive_predicted = sum(1 for point in predicted_boundary if _within_tolerance(point, reference_boundary, tolerance_px))
    true_positive_reference = sum(1 for point in reference_boundary if _within_tolerance(point, predicted_boundary, tolerance_px))
    precision = true_positive_predicted / float(len(predicted_boundary))
    recall = true_positive_reference / float(len(reference_boundary))
    f1 = 0.0 if precision + recall == 0.0 else 2.0 * precision * recall / (precision + recall)
    return precision, recall, f1, predicted_boundary, reference_boundary


def boundary_f1(predicted: Mask, reference: Mask, tolerance_px: int = DEFAULT_BOUNDARY_TOLERANCE_PX) -> float:
    """Return contour Boundary F1 using symmetric pixel tolerance."""

    predicted = _validate_mask(predicted, "predicted_mask")
    reference = _validate_mask(reference, "reference_mask")
    return _boundary_f1_stats(predicted, reference, tolerance_px)[2]


def _normalized_boundary_points(pixels: Sequence[Pixel], width: int, height: int) -> list[Point]:
    return [((x + 0.5) / float(width), (y + 0.5) / float(height)) for x, y in pixels]


def _validate_points(points: Sequence[Sequence[float]], label: str) -> list[Point]:
    _require(isinstance(points, (list, tuple)) and points, f"{label} must be a non-empty point sequence")
    result: list[Point] = []
    for index, point in enumerate(points):
        _require(isinstance(point, (list, tuple)) and len(point) == 2, f"{label}[{index}] must be a 2-D point")
        _finite_number(point[0], f"{label}[{index}][0]")
        _finite_number(point[1], f"{label}[{index}][1]")
        result.append((float(point[0]), float(point[1])))
    return result


def _nearest_distance(point: Point, candidates: Sequence[Point]) -> float:
    return math.sqrt(min((point[0] - other[0]) ** 2 + (point[1] - other[1]) ** 2 for other in candidates))


def _symmetric_distances(predicted_points: Sequence[Sequence[float]], reference_points: Sequence[Sequence[float]]) -> list[float]:
    predicted = _validate_points(predicted_points, "predicted_boundary_points")
    reference = _validate_points(reference_points, "reference_boundary_points")
    return [_nearest_distance(point, reference) for point in predicted] + [_nearest_distance(point, predicted) for point in reference]


def symmetric_chamfer(predicted_points: Sequence[Sequence[float]], reference_points: Sequence[Sequence[float]]) -> float:
    """Return the symmetric mean nearest-point distance in input coordinates."""

    distances = _symmetric_distances(predicted_points, reference_points)
    return sum(distances) / float(len(distances))


def _quantile(values: Sequence[float], probability: float) -> float:
    _require(values, "cannot calculate a quantile of an empty sequence")
    _require(0.0 <= probability <= 1.0, "quantile probability is outside [0,1]")
    ordered = sorted(float(value) for value in values)
    position = (len(ordered) - 1) * probability
    lower = int(math.floor(position))
    upper = min(lower + 1, len(ordered) - 1)
    fraction = position - lower
    return ordered[lower] * (1.0 - fraction) + ordered[upper] * fraction


def p95_contour_distance(predicted_points: Sequence[Sequence[float]], reference_points: Sequence[Sequence[float]]) -> float:
    """Return the symmetric 95th-percentile nearest contour distance."""

    return _quantile(_symmetric_distances(predicted_points, reference_points), 0.95)


def landmark_error(predicted: dict[str, Sequence[float]], reference: dict[str, Sequence[float]]) -> float:
    """Return mean normalized Euclidean error over common semantic landmarks."""

    _require(set(predicted) == set(LANDMARK_IDS), "predicted landmark roles are not closed")
    _require(set(reference) == set(LANDMARK_IDS), "reference landmark roles are not closed")
    errors = []
    for role in LANDMARK_IDS:
        left = _validate_points([predicted[role]], f"predicted.{role}")[0]
        right = _validate_points([reference[role]], f"reference.{role}")[0]
        errors.append(math.sqrt((left[0] - right[0]) ** 2 + (left[1] - right[1]) ** 2))
    return sum(errors) / float(len(errors))


def _round_metric(value: float | None) -> float | None:
    if value is None:
        return None
    rounded = round(float(value), 12)
    return 0.0 if rounded == 0.0 else rounded


def _gate(status: str, checks: dict[str, bool], basis: str) -> dict[str, Any]:
    _require(status in STATUS_VALUES, f"unsupported gate status: {status}")
    passed = status == "PASS" and all(checks.values())
    return {"status": status, "passed": passed, "checks": checks, "basis": basis}


def _null_metrics() -> dict[str, Any]:
    return {
        "silhouette_iou": None,
        "boundary_f1": None,
        "boundary_precision": None,
        "boundary_recall": None,
        "symmetric_chamfer": None,
        "p95_contour_distance": None,
        "landmark_error": None,
        "landmark_errors": [{"landmark_id": role, "error": None} for role in LANDMARK_IDS],
        "predicted_area_px": None,
        "reference_area_px": None,
        "grid_area_px": None,
    }


def _validate_receipt(receipt: dict[str, Any]) -> str:
    """Validate the closed output shape before it is printed or written."""

    _exact_keys(receipt, RECEIPT_KEYS, "KnifeMetricEvaluationReceipt")
    _require(receipt["schema_version"] == RECEIPT_SCHEMA_VERSION, "receipt schema version drifted")
    _id(receipt["evaluation_id"], "receipt.evaluation_id")
    _sha(receipt["program_sha256"], "receipt.program_sha256")
    _require(receipt["reference_sha256"] is None or (isinstance(receipt["reference_sha256"], str) and SHA256_RE.fullmatch(receipt["reference_sha256"]) is not None), "receipt.reference_sha256 is invalid")
    _require(isinstance(receipt["grid_size"], int) and MIN_GRID_SIZE <= receipt["grid_size"] <= MAX_GRID_SIZE, "receipt.grid_size is invalid")
    _require(isinstance(receipt["boundary_tolerance_px"], int) and 0 <= receipt["boundary_tolerance_px"] <= MAX_BOUNDARY_TOLERANCE_PX, "receipt.boundary_tolerance_px is invalid")

    _exact_keys(receipt["geometry"], {"hard_gate_pass", "coordinate_axes", "sample_count", "contour_vertex_count", "landmark_ids"}, "receipt.geometry")
    _require(isinstance(receipt["geometry"]["hard_gate_pass"], bool), "receipt.geometry.hard_gate_pass is invalid")
    _exact_keys(receipt["geometry"]["coordinate_axes"], {"longitudinal", "lateral"}, "receipt.geometry.coordinate_axes")
    for axis in receipt["geometry"]["coordinate_axes"].values():
        _require(isinstance(axis, int) and 0 <= axis <= 2, "receipt.geometry axis is invalid")
    for field in ("sample_count", "contour_vertex_count"):
        _require(isinstance(receipt["geometry"][field], int) and receipt["geometry"][field] >= 3, f"receipt.geometry.{field} is invalid")
    _require(receipt["geometry"]["landmark_ids"] == list(LANDMARK_IDS), "receipt.geometry.landmark_ids are not closed")

    metric_keys = {
        "silhouette_iou",
        "boundary_f1",
        "boundary_precision",
        "boundary_recall",
        "symmetric_chamfer",
        "p95_contour_distance",
        "landmark_error",
        "landmark_errors",
        "predicted_area_px",
        "reference_area_px",
        "grid_area_px",
    }
    _exact_keys(receipt["metrics"], metric_keys, "receipt.metrics")
    _require(isinstance(receipt["metrics"]["landmark_errors"], list) and len(receipt["metrics"]["landmark_errors"]) == len(LANDMARK_IDS), "receipt.metrics.landmark_errors is invalid")
    for index, item in enumerate(receipt["metrics"]["landmark_errors"]):
        _exact_keys(item, {"landmark_id", "error"}, f"receipt.metrics.landmark_errors[{index}]")
        _require(item["landmark_id"] == LANDMARK_IDS[index], f"receipt.metrics.landmark_errors[{index}] role drifted")

    gate_keys = {"status", "passed", "checks", "basis"}
    _exact_keys(receipt["hard_gates"], {"geometry", "reference", "camera_binding", "metric_computability", "quality_thresholds", "all_pass"}, "receipt.hard_gates")
    for gate_name in ("geometry", "reference", "camera_binding", "metric_computability", "quality_thresholds"):
        _exact_keys(receipt["hard_gates"][gate_name], gate_keys, f"receipt.hard_gates.{gate_name}")
        _require(receipt["hard_gates"][gate_name]["status"] in STATUS_VALUES, f"receipt.hard_gates.{gate_name}.status is invalid")
        _require(isinstance(receipt["hard_gates"][gate_name]["passed"], bool), f"receipt.hard_gates.{gate_name}.passed is invalid")
        _require(isinstance(receipt["hard_gates"][gate_name]["checks"], dict), f"receipt.hard_gates.{gate_name}.checks is invalid")
        _require(set(receipt["hard_gates"][gate_name]["checks"]) == GATE_CHECK_KEYS[gate_name], f"receipt.hard_gates.{gate_name}.checks are not closed")
        _require(all(isinstance(value, bool) for value in receipt["hard_gates"][gate_name]["checks"].values()), f"receipt.hard_gates.{gate_name}.checks must be boolean")
        _require(isinstance(receipt["hard_gates"][gate_name]["basis"], str), f"receipt.hard_gates.{gate_name}.basis is invalid")
    _require(isinstance(receipt["hard_gates"]["all_pass"], bool), "receipt.hard_gates.all_pass is invalid")

    _exact_keys(receipt["statuses"], {"quality_status", "render_status", "visual_review_status", "human_review_status", "commercial_status"}, "receipt.statuses")
    _require(receipt["statuses"]["quality_status"] in QUALITY_VALUES, "receipt quality status is invalid")
    for key in ("render_status", "visual_review_status", "human_review_status", "commercial_status"):
        _require(receipt["statuses"][key] == "NOT_RUN", f"receipt.{key} must remain NOT_RUN")
    _exact_keys(receipt["provenance"], {"renderer_used", "runtime_write", "evaluation_mode", "reference_basis", "approval_boundary"}, "receipt.provenance")
    _require(receipt["provenance"]["renderer_used"] is False and receipt["provenance"]["runtime_write"] is False, "receipt provenance crossed the write/render boundary")
    _require(receipt["provenance"]["approval_boundary"] == "math-measurement-only", "receipt approval boundary drifted")
    _sha(receipt["canonical_sha256"], "receipt.canonical_sha256", allow_empty=True)
    digest = canonical_sha256(receipt)
    if receipt["canonical_sha256"]:
        _require(receipt["canonical_sha256"] == digest, "receipt canonical hash does not match canonical JSON")
    return digest


def validate_receipt(receipt: dict[str, Any]) -> str:
    """Public closed-output validator used by focused callers and smoke tests."""

    return _validate_receipt(receipt)


def evaluate_program(program: dict[str, Any], reference: dict[str, Any] | None = None, grid_size: int = DEFAULT_GRID_SIZE, boundary_tolerance_px: int = DEFAULT_BOUNDARY_TOLERANCE_PX) -> dict[str, Any]:
    """Evaluate one program against one closed contour reference.

    ``reference=None`` is intentional and produces a valid ``NOT_RUN``
    receipt.  A malformed non-null reference raises before any output is
    emitted, preserving fail-closed input semantics.
    """

    grid_size = _validate_grid(grid_size)
    boundary_tolerance_px = _validate_tolerance(boundary_tolerance_px)
    program_sha = _search.validate_program(program)
    geometry_evaluation = _search.evaluate_geometry(program)
    contour, predicted_landmarks, axes = _program_outline(program)

    reference_sha: str | None = None
    reference_valid = reference is not None
    reference_landmarks: dict[str, list[float]] | None = None
    reference_contour: list[list[float]] | None = None
    camera_present = False
    if reference is not None:
        reference_sha = validate_reference(reference)
        reference_contour = _validate_contour(reference["outer_contour"])
        reference_landmarks = {item["landmark_id"]: [float(item["point"][0]), float(item["point"][1])] for item in reference["landmarks"]}
        camera_present = reference["camera_frame"] is not None

    metric_values = _null_metrics()
    predicted_contour_simple = not _polygon_self_intersects(contour)
    metrics_ran = bool(reference_valid and geometry_evaluation["hard_gate_pass"] and predicted_contour_simple)
    predicted_mask: Mask | None = None
    reference_mask: Mask | None = None
    if metrics_ran and reference_contour is not None and reference_landmarks is not None:
        predicted_mask = _rasterize_polygon(contour, grid_size, grid_size)
        reference_mask = _rasterize_polygon(reference_contour, grid_size, grid_size)
        predicted_area = sum(1 for row in predicted_mask for value in row if value)
        reference_area = sum(1 for row in reference_mask for value in row if value)
        predicted_boundary = _boundary_pixels(predicted_mask)
        reference_boundary = _boundary_pixels(reference_mask)
        if predicted_area > 0 and reference_area > 0 and predicted_boundary and reference_boundary:
            precision, recall, f1, _, _ = _boundary_f1_stats(predicted_mask, reference_mask, boundary_tolerance_px)
            width, height = _mask_shape(predicted_mask)
            predicted_boundary_points = _normalized_boundary_points(predicted_boundary, width, height)
            reference_boundary_points = _normalized_boundary_points(reference_boundary, width, height)
            distances = _symmetric_distances(predicted_boundary_points, reference_boundary_points)
            per_landmark_errors = {
                role: math.sqrt(
                    (predicted_landmarks[role][0] - reference_landmarks[role][0]) ** 2
                    + (predicted_landmarks[role][1] - reference_landmarks[role][1]) ** 2
                )
                for role in LANDMARK_IDS
            }
            metric_values = {
                "silhouette_iou": _round_metric(silhouette_iou(predicted_mask, reference_mask)),
                "boundary_f1": _round_metric(f1),
                "boundary_precision": _round_metric(precision),
                "boundary_recall": _round_metric(recall),
                "symmetric_chamfer": _round_metric(sum(distances) / len(distances)),
                "p95_contour_distance": _round_metric(_quantile(distances, 0.95)),
                "landmark_error": _round_metric(sum(per_landmark_errors.values()) / len(per_landmark_errors)),
                "landmark_errors": [{"landmark_id": role, "error": _round_metric(per_landmark_errors[role])} for role in LANDMARK_IDS],
                "predicted_area_px": predicted_area,
                "reference_area_px": reference_area,
                "grid_area_px": grid_size * grid_size,
            }

    geometry_gate = _gate(
        "PASS" if geometry_evaluation["hard_gate_pass"] else "FAIL",
        {name: bool(value) for name, value in geometry_evaluation["hard_gates"].items()},
        "KnifeSceneProgram intrinsic no-render geometry gates",
    )
    reference_checks = {
        "reference_present": reference_valid,
        "reference_schema_valid": reference_valid,
        "four_landmarks_present": reference_valid and reference_landmarks is not None and set(reference_landmarks) == set(LANDMARK_IDS),
        "outer_contour_valid": reference_valid and reference_contour is not None,
    }
    reference_gate = _gate("PASS" if reference_valid else "NOT_RUN", reference_checks, "closed contour reference and four semantic landmarks")
    camera_checks = {
        "declared_frame_present": camera_present,
        "declared_projection_supported": camera_present,
        # A JSON declaration is not a Runtime camera hash or a rendered
        # fixed-view identity.  Keep this false by design.
        "runtime_camera_identity_verified": False,
    }
    camera_gate = _gate("NOT_RUN", camera_checks, "no-render evaluator cannot verify Runtime camera identity")
    metric_checks = {
        "measurement_executed": metrics_ran,
        "predicted_contour_simple": predicted_contour_simple,
        "predicted_mask_nonempty": bool(predicted_mask and any(any(row) for row in predicted_mask)),
        "reference_mask_nonempty": bool(reference_mask and any(any(row) for row in reference_mask)),
        "predicted_boundary_nonempty": bool(predicted_mask and _boundary_pixels(predicted_mask)),
        "reference_boundary_nonempty": bool(reference_mask and _boundary_pixels(reference_mask)),
        "four_landmark_errors_computed": all(item["error"] is not None for item in metric_values["landmark_errors"]),
    }
    if not reference_valid or not metrics_ran:
        metric_status = "NOT_RUN"
    else:
        metric_status = "PASS" if all(metric_checks.values()) else "FAIL"
    metric_gate = _gate(metric_status, metric_checks, "fixed-grid mask and boundary/landmark metric computability")

    threshold_checks = {
        "silhouette_iou": metric_values["silhouette_iou"] is not None and metric_values["silhouette_iou"] >= SILHOUETTE_IOU_MIN,
        "boundary_f1": metric_values["boundary_f1"] is not None and metric_values["boundary_f1"] >= BOUNDARY_F1_MIN,
        "symmetric_chamfer": metric_values["symmetric_chamfer"] is not None and metric_values["symmetric_chamfer"] <= SYMMETRIC_CHAMFER_MAX,
        "p95_contour_distance": metric_values["p95_contour_distance"] is not None and metric_values["p95_contour_distance"] <= P95_CONTOUR_DISTANCE_MAX,
        "landmark_error": metric_values["landmark_error"] is not None and metric_values["landmark_error"] <= LANDMARK_ERROR_MAX,
    }
    if not metrics_ran or metric_gate["status"] != "PASS":
        threshold_status = "NOT_RUN"
    else:
        threshold_status = "PASS" if all(threshold_checks.values()) else "FAIL"
    threshold_gate = _gate(
        threshold_status,
        threshold_checks,
        "strict mathematical thresholds: IoU/F1 >= 0.90; Chamfer <= 0.03; P95 <= 0.05; landmark error <= 0.03",
    )

    all_pass = all(gate["passed"] for gate in (geometry_gate, reference_gate, camera_gate, metric_gate, threshold_gate))
    quality_status = "MEASURED_NOT_APPROVED" if metric_gate["status"] == "PASS" else "NOT_RUN"
    evaluation_seed = {
        "program_sha256": program_sha,
        "reference_sha256": reference_sha,
        "grid_size": grid_size,
        "boundary_tolerance_px": boundary_tolerance_px,
    }
    evaluation_id = "eval-" + hashlib.sha256(canonical_bytes(evaluation_seed)).hexdigest()[:24]
    receipt = {
        "schema_version": RECEIPT_SCHEMA_VERSION,
        "evaluation_id": evaluation_id,
        "program_sha256": program_sha,
        "reference_sha256": reference_sha,
        "grid_size": grid_size,
        "boundary_tolerance_px": boundary_tolerance_px,
        "geometry": {
            "hard_gate_pass": bool(geometry_evaluation["hard_gate_pass"]),
            "coordinate_axes": axes,
            "sample_count": PROGRAM_SAMPLE_COUNT,
            "contour_vertex_count": len(contour),
            "landmark_ids": list(LANDMARK_IDS),
        },
        "metrics": metric_values,
        "hard_gates": {
            "geometry": geometry_gate,
            "reference": reference_gate,
            "camera_binding": camera_gate,
            "metric_computability": metric_gate,
            "quality_thresholds": threshold_gate,
            "all_pass": all_pass,
        },
        "statuses": {
            "quality_status": quality_status,
            "render_status": "NOT_RUN",
            "visual_review_status": "NOT_RUN",
            "human_review_status": "NOT_RUN",
            "commercial_status": "NOT_RUN",
        },
        "provenance": {
            "renderer_used": False,
            "runtime_write": False,
            "evaluation_mode": "deterministic-planar-contour-raster@1",
            "reference_basis": "closed-user-contour-or-synthetic-smoke-contour",
            "approval_boundary": "math-measurement-only",
        },
        "canonical_sha256": "",
    }
    receipt["canonical_sha256"] = canonical_sha256(receipt)
    _validate_receipt(receipt)
    return receipt


def run_smoke() -> dict[str, Any]:
    """Run one small deterministic smoke against Dragonfang's first slice."""

    program = load_json(DEFAULT_PROGRAM)
    source_snapshot = copy.deepcopy(program)
    reference = build_reference_from_program(program)
    first = evaluate_program(program, reference, grid_size=64, boundary_tolerance_px=1)
    second = evaluate_program(copy.deepcopy(program), copy.deepcopy(reference), grid_size=64, boundary_tolerance_px=1)
    _require(first == second, "smoke output is not deterministic")
    _require(program == source_snapshot, "smoke mutated the source program")
    _require(first["statuses"]["quality_status"] == "MEASURED_NOT_APPROVED", "smoke quality status crossed the approval boundary")
    _require(first["statuses"]["visual_review_status"] == "NOT_RUN", "smoke visual status must remain NOT_RUN")
    _require(first["statuses"]["commercial_status"] == "NOT_RUN", "smoke commercial status must remain NOT_RUN")
    _require(first["hard_gates"]["geometry"]["passed"] is True, "Dragonfang smoke geometry gate failed")
    _require(first["hard_gates"]["metric_computability"]["passed"] is True, "Dragonfang smoke metric gate failed")
    _require(first["hard_gates"]["camera_binding"]["status"] == "NOT_RUN", "smoke camera gate must remain NOT_RUN")
    _require(first["metrics"]["silhouette_iou"] == 1.0, "identical smoke contours must have IoU 1")
    _require(first["metrics"]["boundary_f1"] == 1.0, "identical smoke contours must have Boundary F1 1")
    _require(first["metrics"]["symmetric_chamfer"] == 0.0, "identical smoke contours must have zero Chamfer")
    _require(first["metrics"]["p95_contour_distance"] == 0.0, "identical smoke contours must have zero P95")
    _require(first["metrics"]["landmark_error"] == 0.0, "identical smoke landmarks must have zero error")
    return first


def _write_output(value: dict[str, Any], output: Path | None, pretty: bool) -> None:
    text = json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2 if pretty else None, separators=None if pretty else (",", ":")) + "\n"
    if output is None:
        sys.stdout.write(text)
        return
    output = output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(text, encoding="utf-8")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--program", type=Path, default=DEFAULT_PROGRAM, help="closed KnifeSceneProgram@1 JSON")
    parser.add_argument("--reference", "--target", dest="reference", type=Path, help="closed KnifeContourReference@1 JSON; omit to produce NOT_RUN")
    parser.add_argument("--grid-size", type=int, default=DEFAULT_GRID_SIZE, help=f"bounded square evaluation grid [{MIN_GRID_SIZE},{MAX_GRID_SIZE}]")
    parser.add_argument("--boundary-tolerance-px", type=int, default=DEFAULT_BOUNDARY_TOLERANCE_PX, help=f"boundary matching tolerance [{0},{MAX_BOUNDARY_TOLERANCE_PX}]")
    parser.add_argument("--output", type=Path, help="optional receipt path")
    parser.add_argument("--pretty", action="store_true", help="pretty-print the closed receipt")
    parser.add_argument("--smoke", action="store_true", help="run the Dragonfang self-contained deterministic smoke")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        protected = {args.program.resolve(), DEFAULT_PROGRAM.resolve() if args.smoke else args.program.resolve()}
        if args.reference:
            protected.add(args.reference.resolve())
        if args.output and args.output.resolve() in protected:
            raise EvaluationInputError("output path must not overwrite a closed input document")
        if args.smoke:
            receipt = run_smoke()
        else:
            program = load_json(args.program)
            reference = load_json(args.reference) if args.reference else None
            receipt = evaluate_program(program, reference, args.grid_size, args.boundary_tolerance_px)
        _write_output(receipt, args.output, args.pretty)
        return 0
    except (EvaluationInputError, _search.InputError) as exc:
        print(f"evaluation input error: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
