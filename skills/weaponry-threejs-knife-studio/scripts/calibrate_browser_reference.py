#!/usr/bin/env python3
"""Calibrate and replay a browser FRONT blade mask against a closed reference.

This script is deliberately a small, closed mathematical boundary between a
browser capture and the no-render contour metrics.  It consumes a
``KnifeContourReference@1``, a browser ``part-id`` PNG, and a closed
``WeaponryThreeJsCaptureManifest@1``.  A baseline run fits the reference
bounds to the *baseline* extracted blade bounds once.  A replay run consumes
that immutable calibration and reuses the stored fit without looking at the
candidate bounds.  The latter rule is important: a candidate cannot improve
its score by changing the reference frame.

The output is evidence of measurement only.  ``MEASURED_NOT_APPROVED`` is the
highest quality state emitted here; visual, human, engine, and commercial
acceptance stay ``NOT_RUN``.  No Runtime write, Three.js render, or image
comparison outside the selected FRONT part-id mask is performed.
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
from typing import Any, Sequence

from PIL import Image


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

try:
    import evaluate_metrics as _metrics
except ImportError as exc:  # pragma: no cover - broken skill installation
    raise RuntimeError("evaluate_metrics.py is required beside this script") from exc


REFERENCE_SCHEMA_VERSION = "KnifeContourReference@1"
MANIFEST_SCHEMA_VERSION = "WeaponryThreeJsCaptureManifest@1"
CALIBRATION_SCHEMA_VERSION = "KnifeReferenceBrowserCalibration@1"
RECEIPT_SCHEMA_VERSION = "KnifeBrowserMetricReceipt@1"
VIEW_ID = "FRONT"
VIEW_IDS = ("FRONT", "BACK", "TOP", "BOTTOM", "LEFT", "RIGHT", "REAR_THREE_QUARTER", "FPS_HOLD")
REQUIRED_AOV_IDS = ("beauty", "silhouette", "depth", "normal", "part-id", "material-id", "wireframe")
CAMERA_PROJECTION = "orthographic"
FIT_ALGORITHM = "aspect-preserving-centered-reference-bbox-to-baseline-mask-bbox@1"
REFERENCE_COORDINATE_SPACE = "unit-square@1"
TARGET_COORDINATE_SPACE = "capture-pixel-normalized-top-left@1"
REPLAY_POLICY = "reuse-frozen-fit-no-refit@1"
LANDMARK_IDS = tuple(_metrics.LANDMARK_IDS)
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
FINGERPRINT_RE = re.compile(r"^[0-9a-f]{16,128}$")
ID_RE = re.compile(r"^[a-zA-Z][a-zA-Z0-9_.@-]{0,63}$")
MAX_FRAME_SIZE = 2048
MIN_FRAME_SIZE = 16
MAX_BOUNDARY_TOLERANCE_PX = 8


class CalibrationInputError(ValueError):
    """A malformed, drifting, or untrusted calibration input."""


def canonical_bytes(value: Any) -> bytes:
    """Return the repository's deterministic JSON byte representation."""

    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def canonical_sha256(value: Any) -> str:
    """Hash a document after blanking its own canonical hash field."""

    draft = copy.deepcopy(value)
    if isinstance(draft, dict) and "canonical_sha256" in draft:
        draft["canonical_sha256"] = ""
    return hashlib.sha256(canonical_bytes(draft)).hexdigest()


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise CalibrationInputError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=_reject_duplicate_keys)
    except OSError as exc:
        raise CalibrationInputError(f"cannot read {path}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise CalibrationInputError(f"invalid JSON in {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise CalibrationInputError(f"{path} must contain a JSON object")
    return value


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise CalibrationInputError(message)


def _exact_keys(value: Any, expected: set[str], label: str) -> None:
    _require(isinstance(value, dict), f"{label} must be an object")
    _require(set(value) == expected, f"{label} keys are not closed")


def _finite_number(value: Any, label: str, minimum: float | None = None, maximum: float | None = None) -> None:
    _require(isinstance(value, (int, float)) and not isinstance(value, bool), f"{label} must be numeric")
    number = float(value)
    _require(math.isfinite(number), f"{label} must be finite")
    if minimum is not None:
        _require(number >= minimum, f"{label} must be at least {minimum}")
    if maximum is not None:
        _require(number <= maximum, f"{label} must be at most {maximum}")


def _sha(value: Any, label: str, allow_empty: bool = False) -> None:
    _require(isinstance(value, str), f"{label} must be a lowercase SHA-256 string")
    if allow_empty and value == "":
        return
    _require(SHA256_RE.fullmatch(value) is not None, f"{label} is not a lowercase SHA-256")


def _id(value: Any, label: str) -> None:
    _require(isinstance(value, str) and ID_RE.fullmatch(value) is not None, f"{label} is not a closed ID")


def _finite_vector(value: Any, length: int, label: str) -> None:
    _require(isinstance(value, list) and len(value) == length, f"{label} must contain {length} values")
    for index, item in enumerate(value):
        _finite_number(item, f"{label}[{index}]")


def _round(value: float) -> float:
    rounded = round(float(value), 12)
    return 0.0 if rounded == 0.0 else rounded


def _hash_binary_mask(mask: list[list[bool]]) -> str:
    width = len(mask[0])
    height = len(mask)
    payload = bytearray()
    payload.extend(width.to_bytes(4, "big"))
    payload.extend(height.to_bytes(4, "big"))
    payload.extend(1 if value else 0 for row in mask for value in row)
    return hashlib.sha256(bytes(payload)).hexdigest()


def _mask_bounds(mask: list[list[bool]]) -> tuple[int, int, int, int] | None:
    width = len(mask[0])
    height = len(mask)
    occupied = [(x, y) for y in range(height) for x in range(width) if mask[y][x]]
    if not occupied:
        return None
    xs = [point[0] for point in occupied]
    ys = [point[1] for point in occupied]
    return min(xs), min(ys), max(xs), max(ys)


def _mask_count(mask: list[list[bool]]) -> int:
    return sum(1 for row in mask for value in row if value)


def _normalized_pixel_bbox(bounds: tuple[int, int, int, int], width: int, height: int) -> list[float]:
    x_min, y_min, x_max, y_max = bounds
    return [x_min / float(width), y_min / float(height), (x_max + 1) / float(width), (y_max + 1) / float(height)]


def _reference_contour_and_landmarks(reference: dict[str, Any]) -> tuple[list[list[float]], dict[str, list[float]]]:
    contour = []
    for point in reference["outer_contour"]:
        # The authorized contour reference uses a bottom-left unit square,
        # while a browser image is top-left.  Keep this conversion explicit
        # and deterministic; it is not a camera or candidate adjustment.
        contour.append([float(point[0]), 1.0 - float(point[1])])
    landmarks = {
        item["landmark_id"]: [float(item["point"][0]), 1.0 - float(item["point"][1])]
        for item in reference["landmarks"]
    }
    return contour, landmarks


def _bbox_from_points(points: Sequence[Sequence[float]]) -> list[float]:
    _require(points, "reference contour cannot be empty")
    xs = [float(point[0]) for point in points]
    ys = [float(point[1]) for point in points]
    return [min(xs), min(ys), max(xs), max(ys)]


def _transform_point(point: Sequence[float], fit: dict[str, Any]) -> list[float]:
    scale = float(fit["scale"])
    translation = fit["translation"]
    return [_round(float(point[0]) * scale + float(translation[0])), _round(float(point[1]) * scale + float(translation[1]))]


def _transform_points(points: Sequence[Sequence[float]], fit: dict[str, Any]) -> list[list[float]]:
    return [_transform_point(point, fit) for point in points]


def _validate_reference(reference: dict[str, Any]) -> str:
    digest = _metrics.validate_reference(reference)
    _require(reference["schema_version"] == REFERENCE_SCHEMA_VERSION, "reference schema version drifted")
    _require(reference["camera_frame"] is not None, "reference camera frame is required")
    return digest


def _camera_fingerprint(camera: dict[str, Any]) -> str:
    material = {key: camera[key] for key in ("view_id", "projection", "matrix_world", "matrix_world_inverse", "projection_matrix")}
    return hashlib.sha256(canonical_bytes(material)).hexdigest()


def _validate_camera(camera: Any, expected_view_id: str) -> str:
    expected = {"view_id", "projection", "matrix_world", "matrix_world_inverse", "projection_matrix", "camera_fingerprint"}
    _exact_keys(camera, expected, f"capture.views[{expected_view_id}].camera")
    _require(camera["view_id"] == expected_view_id, f"camera view_id drifted for {expected_view_id}")
    _require(camera["projection"] in {"orthographic", "perspective"}, f"camera projection is unsupported for {expected_view_id}")
    for key in ("matrix_world", "matrix_world_inverse", "projection_matrix"):
        _finite_vector(camera[key], 16, f"capture.views[{expected_view_id}].camera.{key}")
    _sha(camera["camera_fingerprint"], f"capture.views[{expected_view_id}].camera.camera_fingerprint")
    computed = _camera_fingerprint(camera)
    _require(camera["camera_fingerprint"] == computed, f"camera fingerprint does not match {expected_view_id} camera matrix")
    return computed


def _validate_aov(aov: Any, view_id: str, aov_id: str, frame_width: int, frame_height: int) -> None:
    _exact_keys(aov, {"aov_id", "mime_type", "width", "height", "png_sha256", "png_size_bytes"}, f"capture.views[{view_id}].aovs[{aov_id}]")
    _require(aov["aov_id"] == aov_id, f"AOV id drifted in {view_id}")
    _require(aov["mime_type"] == "image/png", f"AOV {view_id}/{aov_id} is not PNG")
    _require(aov["width"] == frame_width and aov["height"] == frame_height, f"AOV {view_id}/{aov_id} dimensions drifted")
    _sha(aov["png_sha256"], f"AOV {view_id}/{aov_id}.png_sha256")
    _require(isinstance(aov["png_size_bytes"], int) and aov["png_size_bytes"] > 0, f"AOV {view_id}/{aov_id} size is invalid")


def _validate_manifest(manifest: dict[str, Any], view_id: str) -> tuple[str, dict[str, Any]]:
    expected = {
        "schema_version", "manifest_id", "rig_id", "rig_fingerprint", "rig_margin",
        "program_fingerprint", "scene_fingerprint", "frame_width", "frame_height",
        "view_ids", "aov_ids", "views", "renderer", "capture_mode", "renderer_invoked",
        "render_status", "quality_status", "visual_status", "human_status", "engine_status",
        "commercial_status", "canonical_sha256",
    }
    _exact_keys(manifest, expected, "WeaponryThreeJsCaptureManifest")
    _require(manifest["schema_version"] == MANIFEST_SCHEMA_VERSION, "capture manifest schema version drifted")
    _id(manifest["manifest_id"], "capture manifest.manifest_id")
    _id(manifest["rig_id"], "capture manifest.rig_id")
    # The current Three.js rig fingerprint is a short cohort fingerprint, so
    # permit the documented hexadecimal 16..64 form while still rejecting
    # arbitrary text.
    _require(isinstance(manifest["rig_fingerprint"], str) and FINGERPRINT_RE.fullmatch(manifest["rig_fingerprint"]) is not None, "capture manifest.rig_fingerprint is not a closed cohort fingerprint")
    _finite_number(manifest["rig_margin"], "capture manifest.rig_margin", 0.0, 0.5)
    _require(isinstance(manifest["program_fingerprint"], str) and FINGERPRINT_RE.fullmatch(manifest["program_fingerprint"]) is not None, "capture manifest.program_fingerprint is not a closed fingerprint")
    _sha(manifest["scene_fingerprint"], "capture manifest.scene_fingerprint")
    for key in ("frame_width", "frame_height"):
        _require(isinstance(manifest[key], int) and not isinstance(manifest[key], bool) and MIN_FRAME_SIZE <= manifest[key] <= MAX_FRAME_SIZE, f"capture manifest.{key} is outside the bounded frame range")
    frame_width = manifest["frame_width"]
    frame_height = manifest["frame_height"]
    _require(manifest["view_ids"] == list(VIEW_IDS), "capture manifest view order is not the fixed eight-view rig")
    _require(isinstance(manifest["aov_ids"], list) and all(isinstance(item, str) for item in manifest["aov_ids"]), "capture manifest aov_ids is invalid")
    _require(list(manifest["aov_ids"][: len(REQUIRED_AOV_IDS)]) == list(REQUIRED_AOV_IDS), "capture manifest required AOV order drifted")
    _require(len(set(manifest["aov_ids"])) == len(manifest["aov_ids"]), "capture manifest contains duplicate AOV ids")
    _require(isinstance(manifest["views"], list) and len(manifest["views"]) == len(VIEW_IDS), "capture manifest must contain exactly eight views")
    views_by_id: dict[str, Any] = {}
    for index, view in enumerate(manifest["views"]):
        _exact_keys(view, {"view_id", "camera", "aovs"}, f"capture.views[{index}]")
        current_id = view["view_id"]
        _require(current_id == VIEW_IDS[index], f"capture view order drifted at index {index}")
        _validate_camera(view["camera"], current_id)
        _require(isinstance(view["aovs"], list) and len(view["aovs"]) == len(manifest["aov_ids"]), f"capture view {current_id} AOV count drifted")
        for aov_index, aov_id in enumerate(manifest["aov_ids"]):
            _require(isinstance(view["aovs"][aov_index], dict), f"capture view {current_id} AOV entry is not an object")
            _require(view["aovs"][aov_index].get("aov_id") == aov_id, f"capture view {current_id} AOV order drifted")
            _validate_aov(view["aovs"][aov_index], current_id, aov_id, frame_width, frame_height)
        views_by_id[current_id] = view
    _require(manifest["renderer"] == "browser-webgl@1", "capture manifest renderer is not the closed browser renderer")
    _require(manifest["capture_mode"] == "browser-canvas-to-png@1", "capture manifest capture mode drifted")
    _require(manifest["renderer_invoked"] is True, "capture manifest does not prove renderer invocation")
    _require(manifest["render_status"] == "RENDERED", "capture manifest render status is not RENDERED")
    _require(manifest["quality_status"] == "RENDERED_NOT_APPROVED", "capture manifest quality status is not RENDERED_NOT_APPROVED")
    for key in ("visual_status", "human_status", "engine_status", "commercial_status"):
        _require(manifest[key] == "NOT_RUN", f"capture manifest.{key} must remain NOT_RUN")
    _sha(manifest["canonical_sha256"], "capture manifest.canonical_sha256")
    _require(manifest["canonical_sha256"] == canonical_sha256(manifest), "capture manifest canonical hash does not match canonical JSON")
    _require(view_id in views_by_id, f"target view {view_id} is absent from capture manifest")
    _require(view_id == VIEW_ID, "only FRONT calibration is supported")
    return manifest["canonical_sha256"], views_by_id[view_id]


def validate_manifest(manifest: dict[str, Any]) -> str:
    """Validate a closed browser capture manifest and return its hash."""

    digest, _ = _validate_manifest(manifest, VIEW_ID)
    return digest


def _load_part_id_mask(path: Path, manifest_view: dict[str, Any], frame_width: int, frame_height: int, allowed_ids: tuple[int, ...]) -> tuple[list[list[bool]], dict[str, Any]]:
    try:
        payload = path.read_bytes()
    except OSError as exc:
        raise CalibrationInputError(f"cannot read part-id PNG {path}: {exc}") from exc
    png_sha = hashlib.sha256(payload).hexdigest()
    aov = next((item for item in manifest_view["aovs"] if item["aov_id"] == "part-id"), None)
    _require(aov is not None, "target FRONT view has no part-id AOV")
    _require(png_sha == aov["png_sha256"], "part-id PNG SHA does not match capture manifest")
    _require(len(payload) == aov["png_size_bytes"], "part-id PNG size does not match capture manifest")
    try:
        with Image.open(path) as image:
            _require(image.format == "PNG", "part-id input is not a PNG")
            _require(image.width == frame_width and image.height == frame_height, "part-id PNG dimensions do not match capture manifest")
            rgba = image.convert("RGBA")
            pixels = rgba.load()
            mask = []
            observed_ids: set[int] = set()
            for y in range(frame_height):
                row: list[bool] = []
                for x in range(frame_width):
                    red, green, blue, alpha = pixels[x, y]
                    part_id = (int(red) << 16) | (int(green) << 8) | int(blue)
                    if alpha > 0:
                        observed_ids.add(part_id)
                    row.append(alpha > 0 and part_id in allowed_ids)
                mask.append(row)
    except (OSError, ValueError) as exc:
        raise CalibrationInputError(f"cannot decode part-id PNG {path}: {exc}") from exc
    bounds = _mask_bounds(mask)
    _require(bounds is not None, "allowed part-id set extracted an empty blade mask")
    count = _mask_count(mask)
    _require(count >= 8, "extracted blade mask is too small")
    return mask, {
        "png_sha256": png_sha,
        "png_size_bytes": len(payload),
        "observed_part_ids": sorted(observed_ids),
        "allowed_part_ids": list(allowed_ids),
        "pixel_count": count,
        "bounds_px": list(bounds),
        "bbox": _normalized_pixel_bbox(bounds, frame_width, frame_height),
        "mask_sha256": _hash_binary_mask(mask),
    }


def parse_allowed_part_ids(text: str) -> tuple[int, ...]:
    values = [item.strip() for item in text.split(",") if item.strip()]
    _require(values, "--allowed-part-ids must contain at least one numeric ID")
    parsed: list[int] = []
    for item in values:
        _require(re.fullmatch(r"(?:0|[1-9][0-9]{0,7})", item) is not None, f"part-id is not a canonical decimal integer: {item}")
        number = int(item)
        _require(0 <= number <= 0xFFFFFF, f"part-id is outside 24-bit range: {item}")
        _require(number != 0, "background part-id 0 cannot be used as blade geometry")
        parsed.append(number)
    _require(len(set(parsed)) == len(parsed), "--allowed-part-ids contains duplicates")
    return tuple(sorted(parsed))


def _make_fit(reference_contour: Sequence[Sequence[float]], target_bbox: Sequence[float]) -> dict[str, Any]:
    source_bbox = _bbox_from_points(reference_contour)
    source_width = source_bbox[2] - source_bbox[0]
    source_height = source_bbox[3] - source_bbox[1]
    target_width = float(target_bbox[2]) - float(target_bbox[0])
    target_height = float(target_bbox[3]) - float(target_bbox[1])
    _require(source_width > 0.0 and source_height > 0.0, "reference contour bbox is degenerate")
    _require(target_width > 0.0 and target_height > 0.0, "baseline blade mask bbox is degenerate")
    scale = min(target_width / source_width, target_height / source_height)
    source_center = [(source_bbox[0] + source_bbox[2]) * 0.5, (source_bbox[1] + source_bbox[3]) * 0.5]
    target_center = [(float(target_bbox[0]) + float(target_bbox[2])) * 0.5, (float(target_bbox[1]) + float(target_bbox[3])) * 0.5]
    translation = [target_center[0] - source_center[0] * scale, target_center[1] - source_center[1] * scale]
    return {
        "algorithm": FIT_ALGORITHM,
        "source_coordinate_space": REFERENCE_COORDINATE_SPACE,
        "target_coordinate_space": TARGET_COORDINATE_SPACE,
        "axis_conversion": "reference-bottom-left-to-image-top-left@1",
        "source_bbox": [_round(value) for value in source_bbox],
        "target_bbox": [_round(float(value)) for value in target_bbox],
        "source_center": [_round(value) for value in source_center],
        "target_center": [_round(value) for value in target_center],
        "scale": _round(scale),
        "translation": [_round(value) for value in translation],
    }


def _validate_fit(fit: Any) -> None:
    _exact_keys(fit, {"algorithm", "source_coordinate_space", "target_coordinate_space", "axis_conversion", "source_bbox", "target_bbox", "source_center", "target_center", "scale", "translation"}, "calibration.fit")
    _require(fit["algorithm"] == FIT_ALGORITHM, "calibration fit algorithm drifted")
    _require(fit["source_coordinate_space"] == REFERENCE_COORDINATE_SPACE, "calibration fit source coordinate space drifted")
    _require(fit["target_coordinate_space"] == TARGET_COORDINATE_SPACE, "calibration fit target coordinate space drifted")
    _require(fit["axis_conversion"] == "reference-bottom-left-to-image-top-left@1", "calibration fit axis conversion drifted")
    for key in ("source_bbox", "target_bbox"):
        _finite_vector(fit[key], 4, f"calibration.fit.{key}")
        _require(fit[key][0] < fit[key][2] and fit[key][1] < fit[key][3], f"calibration.fit.{key} is degenerate")
    for key in ("source_center", "target_center", "translation"):
        _finite_vector(fit[key], 2, f"calibration.fit.{key}")
    _finite_number(fit["scale"], "calibration.fit.scale", 1e-12)


def _part_id_set(value: Any, label: str) -> tuple[int, ...]:
    _require(isinstance(value, list) and value and all(isinstance(item, int) and not isinstance(item, bool) for item in value), f"{label} must be a non-empty integer list")
    _require(value == sorted(set(value)), f"{label} must be sorted and unique")
    for item in value:
        _require(1 <= item <= 0xFFFFFF, f"{label} contains an invalid non-background 24-bit ID")
    return tuple(value)


def _validate_calibration(calibration: dict[str, Any]) -> str:
    expected = {
        "schema_version", "calibration_id", "calibration_mode", "reference_id", "reference_sha256",
        "capture_manifest_sha256", "view_id", "camera_fingerprint", "rig_id", "rig_fingerprint",
        "program_fingerprint", "scene_fingerprint", "frame_width", "frame_height", "allowed_part_ids",
        "part_id_png_sha256", "part_id_png_size_bytes", "fit", "baseline_mask", "replay_policy", "calibration_status", "quality_status", "canonical_sha256",
    }
    _exact_keys(calibration, expected, "KnifeReferenceBrowserCalibration")
    _require(calibration["schema_version"] == CALIBRATION_SCHEMA_VERSION, "calibration schema version drifted")
    _id(calibration["calibration_id"], "calibration.calibration_id")
    _require(calibration["calibration_mode"] == "baseline", "calibration must be a baseline calibration")
    _id(calibration["reference_id"], "calibration.reference_id")
    for key in ("reference_sha256", "capture_manifest_sha256", "camera_fingerprint", "scene_fingerprint"):
        _sha(calibration[key], f"calibration.{key}")
    _id(calibration["rig_id"], "calibration.rig_id")
    _require(isinstance(calibration["rig_fingerprint"], str) and FINGERPRINT_RE.fullmatch(calibration["rig_fingerprint"]) is not None, "calibration.rig_fingerprint is invalid")
    _require(isinstance(calibration["program_fingerprint"], str) and FINGERPRINT_RE.fullmatch(calibration["program_fingerprint"]) is not None, "calibration.program_fingerprint is invalid")
    _require(calibration["view_id"] == VIEW_ID, "calibration view must be FRONT")
    _sha(calibration["camera_fingerprint"], "calibration.camera_fingerprint")
    for key in ("frame_width", "frame_height"):
        _require(isinstance(calibration[key], int) and MIN_FRAME_SIZE <= calibration[key] <= MAX_FRAME_SIZE, f"calibration.{key} is invalid")
    allowed = _part_id_set(calibration["allowed_part_ids"], "calibration.allowed_part_ids")
    _sha(calibration["part_id_png_sha256"], "calibration.part_id_png_sha256")
    _require(isinstance(calibration["part_id_png_size_bytes"], int) and calibration["part_id_png_size_bytes"] > 0, "calibration.part_id_png_size_bytes is invalid")
    _validate_fit(calibration["fit"])
    _exact_keys(calibration["baseline_mask"], {"pixel_count", "bounds_px", "bbox", "mask_sha256"}, "calibration.baseline_mask")
    _require(isinstance(calibration["baseline_mask"]["pixel_count"], int) and calibration["baseline_mask"]["pixel_count"] >= 8, "calibration baseline mask count is invalid")
    _finite_vector(calibration["baseline_mask"]["bounds_px"], 4, "calibration.baseline_mask.bounds_px")
    _require(all(isinstance(item, int) for item in calibration["baseline_mask"]["bounds_px"]), "calibration baseline pixel bounds must be integers")
    _require(calibration["baseline_mask"]["bounds_px"][0] <= calibration["baseline_mask"]["bounds_px"][2] and calibration["baseline_mask"]["bounds_px"][1] <= calibration["baseline_mask"]["bounds_px"][3], "calibration baseline pixel bounds are invalid")
    _finite_vector(calibration["baseline_mask"]["bbox"], 4, "calibration.baseline_mask.bbox")
    _require(calibration["baseline_mask"]["bbox"][0] < calibration["baseline_mask"]["bbox"][2] and calibration["baseline_mask"]["bbox"][1] < calibration["baseline_mask"]["bbox"][3], "calibration baseline bbox is invalid")
    _sha(calibration["baseline_mask"]["mask_sha256"], "calibration.baseline_mask.mask_sha256")
    _require(calibration["replay_policy"] == REPLAY_POLICY, "calibration replay policy drifted")
    _require(calibration["calibration_status"] == "BASELINE_CALIBRATED_NOT_APPROVED", "calibration status must remain not approved")
    _require(calibration["quality_status"] == "MEASURED_NOT_APPROVED", "calibration quality status must remain measured not approved")
    _sha(calibration["canonical_sha256"], "calibration.canonical_sha256")
    _require(calibration["canonical_sha256"] == canonical_sha256(calibration), "calibration canonical hash does not match canonical JSON")
    # Keep the variable use explicit for callers reading the validation path.
    _require(allowed, "calibration allowed part-id set is empty")
    return calibration["canonical_sha256"]


def validate_calibration(calibration: dict[str, Any]) -> str:
    """Validate a frozen ``KnifeReferenceBrowserCalibration@1``."""

    return _validate_calibration(calibration)


def _column_midpoint(mask: list[list[bool]], normalized_x: float) -> list[float]:
    width = len(mask[0])
    height = len(mask)
    requested_x = min(width - 1, max(0, int(round(float(normalized_x) * width - 0.5))))
    occupied_columns = [x for x in range(width) if any(mask[y][x] for y in range(height))]
    _require(occupied_columns, "cannot derive landmarks from an empty blade mask")
    column = min(occupied_columns, key=lambda x: (abs(x - requested_x), x))
    rows = [y for y in range(height) if mask[y][column]]
    _require(rows, "selected landmark column has no blade pixels")
    return [(column + 0.5) / float(width), ((min(rows) + max(rows)) * 0.5 + 0.5) / float(height)]


def _metric_values(mask: list[list[bool]], reference_contour: list[list[float]], reference_landmarks: dict[str, list[float]], tolerance_px: int) -> dict[str, Any]:
    width = len(mask[0])
    height = len(mask)
    reference_mask = _metrics._rasterize_polygon(reference_contour, width, height)
    reference_area = _mask_count(reference_mask)
    predicted_area = _mask_count(mask)
    predicted_boundary = _metrics._boundary_pixels(mask)
    reference_boundary = _metrics._boundary_pixels(reference_mask)
    _require(predicted_area > 0 and reference_area > 0, "metric masks are empty")
    _require(predicted_boundary and reference_boundary, "metric boundary is empty")
    _require(0 <= tolerance_px <= MAX_BOUNDARY_TOLERANCE_PX, "boundary tolerance is outside the bounded range")
    precision, recall, f1, _, _ = _metrics._boundary_f1_stats(mask, reference_mask, tolerance_px)
    predicted_boundary_points = _metrics._normalized_boundary_points(predicted_boundary, width, height)
    reference_boundary_points = _metrics._normalized_boundary_points(reference_boundary, width, height)
    distances = _metrics._symmetric_distances(predicted_boundary_points, reference_boundary_points)
    predicted_landmarks = {role: _column_midpoint(mask, reference_landmarks[role][0]) for role in LANDMARK_IDS}
    per_landmark_errors = {
        role: math.hypot(predicted_landmarks[role][0] - reference_landmarks[role][0], predicted_landmarks[role][1] - reference_landmarks[role][1])
        for role in LANDMARK_IDS
    }
    return {
        "silhouette_iou": _round(_metrics.silhouette_iou(mask, reference_mask)),
        "boundary_f1": _round(f1),
        "boundary_precision": _round(precision),
        "boundary_recall": _round(recall),
        "symmetric_chamfer": _round(sum(distances) / len(distances)),
        "p95_contour_distance": _round(_metrics._quantile(distances, 0.95)),
        "landmark_error": _round(sum(per_landmark_errors.values()) / len(per_landmark_errors)),
        "landmark_errors": [{"landmark_id": role, "error": _round(per_landmark_errors[role])} for role in LANDMARK_IDS],
        "predicted_landmarks": [{"landmark_id": role, "point": [_round(value) for value in predicted_landmarks[role]]} for role in LANDMARK_IDS],
        "reference_area_px": reference_area,
        "predicted_area_px": predicted_area,
        "grid_width": width,
        "grid_height": height,
    }


def _quality_gate(metrics: dict[str, Any]) -> dict[str, Any]:
    checks = {
        "silhouette_iou": float(metrics["silhouette_iou"]) >= float(_metrics.SILHOUETTE_IOU_MIN),
        "boundary_f1": float(metrics["boundary_f1"]) >= float(_metrics.BOUNDARY_F1_MIN),
        "symmetric_chamfer": float(metrics["symmetric_chamfer"]) <= float(_metrics.SYMMETRIC_CHAMFER_MAX),
        "p95_contour_distance": float(metrics["p95_contour_distance"]) <= float(_metrics.P95_CONTOUR_DISTANCE_MAX),
        "landmark_error": float(metrics["landmark_error"]) <= float(_metrics.LANDMARK_ERROR_MAX),
    }
    return {
        "status": "PASS" if all(checks.values()) else "FAIL",
        "passed": all(checks.values()),
        "checks": checks,
        "thresholds": {
            "silhouette_iou_min": _metrics.SILHOUETTE_IOU_MIN,
            "boundary_f1_min": _metrics.BOUNDARY_F1_MIN,
            "symmetric_chamfer_max": _metrics.SYMMETRIC_CHAMFER_MAX,
            "p95_contour_distance_max": _metrics.P95_CONTOUR_DISTANCE_MAX,
            "landmark_error_max": _metrics.LANDMARK_ERROR_MAX,
        },
        "basis": "deterministic FRONT blade-mask math only; PASS here never approves the asset",
    }


def _make_calibration(reference: dict[str, Any], reference_sha: str, manifest: dict[str, Any], manifest_sha: str, camera: dict[str, Any], camera_sha: str, mask_info: dict[str, Any], allowed_ids: tuple[int, ...], fit: dict[str, Any]) -> dict[str, Any]:
    calibration_identity = hashlib.sha256(
        canonical_bytes(
            {
                "reference_sha256": reference_sha,
                "capture_manifest_sha256": manifest_sha,
                "camera_fingerprint": camera_sha,
                "part_id_png_sha256": mask_info["png_sha256"],
            }
        )
    ).hexdigest()
    calibration = {
        "schema_version": CALIBRATION_SCHEMA_VERSION,
        "calibration_id": f"cal-{calibration_identity[:32]}",
        "calibration_mode": "baseline",
        "reference_id": reference["reference_id"],
        "reference_sha256": reference_sha,
        "capture_manifest_sha256": manifest_sha,
        "view_id": VIEW_ID,
        "camera_fingerprint": camera_sha,
        "rig_id": manifest["rig_id"],
        "rig_fingerprint": manifest["rig_fingerprint"],
        "program_fingerprint": manifest["program_fingerprint"],
        "scene_fingerprint": manifest["scene_fingerprint"],
        "frame_width": manifest["frame_width"],
        "frame_height": manifest["frame_height"],
        "allowed_part_ids": list(allowed_ids),
        "part_id_png_sha256": mask_info["png_sha256"],
        "part_id_png_size_bytes": mask_info["png_size_bytes"],
        "fit": fit,
        "baseline_mask": {
            "pixel_count": mask_info["pixel_count"],
            "bounds_px": mask_info["bounds_px"],
            "bbox": mask_info["bbox"],
            "mask_sha256": mask_info["mask_sha256"],
        },
        "replay_policy": REPLAY_POLICY,
        "calibration_status": "BASELINE_CALIBRATED_NOT_APPROVED",
        "quality_status": "MEASURED_NOT_APPROVED",
        "canonical_sha256": "",
    }
    calibration["canonical_sha256"] = canonical_sha256(calibration)
    _validate_calibration(calibration)
    return calibration


def _validate_replay_bindings(calibration: dict[str, Any], reference: dict[str, Any], reference_sha: str, manifest: dict[str, Any], manifest_sha: str, camera_sha: str, allowed_ids: tuple[int, ...]) -> None:
    _require(calibration["reference_id"] == reference["reference_id"], "replay reference_id differs from frozen calibration")
    _require(calibration["reference_sha256"] == reference_sha, "replay reference hash differs from frozen calibration")
    _require(calibration["view_id"] == VIEW_ID, "replay view differs from frozen FRONT calibration")
    _require(calibration["camera_fingerprint"] == camera_sha, "replay camera fingerprint differs from frozen calibration")
    _require(calibration["rig_id"] == manifest["rig_id"] and calibration["rig_fingerprint"] == manifest["rig_fingerprint"], "replay rig differs from frozen calibration")
    _require(calibration["frame_width"] == manifest["frame_width"] and calibration["frame_height"] == manifest["frame_height"], "replay frame dimensions differ from frozen calibration")
    _require(tuple(calibration["allowed_part_ids"]) == allowed_ids, "replay allowed part-id set differs from frozen calibration")


def _make_receipt(reference: dict[str, Any], reference_sha: str, manifest: dict[str, Any], manifest_sha: str, camera_sha: str, mask_info: dict[str, Any], allowed_ids: tuple[int, ...], calibration: dict[str, Any], calibration_sha: str, metrics: dict[str, Any], mode: str, tolerance_px: int) -> dict[str, Any]:
    metric_values = {key: value for key, value in metrics.items() if key != "predicted_landmarks"}
    metric_values["landmarks"] = {
        "reference": [
            {
                "landmark_id": role,
                "point": _transform_point(
                    [float(reference["landmarks"][index]["point"][0]), 1.0 - float(reference["landmarks"][index]["point"][1])],
                    calibration["fit"],
                ),
            }
            for index, role in enumerate(LANDMARK_IDS)
        ],
        "predicted": metrics["predicted_landmarks"],
    }
    receipt = {
        "schema_version": RECEIPT_SCHEMA_VERSION,
        "evaluation_id": f"eval-{mode}-{manifest_sha[:24]}",
        "evaluation_mode": mode,
        "calibration_sha256": calibration_sha,
        "reference_id": reference["reference_id"],
        "reference_sha256": reference_sha,
        "capture_manifest_sha256": manifest_sha,
        "baseline_capture_manifest_sha256": calibration["capture_manifest_sha256"],
        "view_id": VIEW_ID,
        "camera_fingerprint": camera_sha,
        "rig_id": manifest["rig_id"],
        "rig_fingerprint": manifest["rig_fingerprint"],
        "program_fingerprint": manifest["program_fingerprint"],
        "scene_fingerprint": manifest["scene_fingerprint"],
        "baseline_program_fingerprint": calibration["program_fingerprint"],
        "baseline_scene_fingerprint": calibration["scene_fingerprint"],
        "frame_width": manifest["frame_width"],
        "frame_height": manifest["frame_height"],
        "allowed_part_ids": list(allowed_ids),
        "part_id_png_sha256": mask_info["png_sha256"],
        "part_id_png_size_bytes": mask_info["png_size_bytes"],
        "baseline_mask_sha256": calibration["baseline_mask"]["mask_sha256"],
        "boundary_tolerance_px": tolerance_px,
        "fit_reused": mode == "frozen_replay",
        "refit_performed": mode == "baseline_calibration",
        "fit_source": "new-baseline-mask" if mode == "baseline_calibration" else "frozen-calibration-input",
        "metrics": metric_values,
        "hard_gates": {
            "input_integrity": {
                "status": "PASS",
                "passed": True,
                "checks": {
                    "reference_hash_bound": True,
                    "capture_manifest_hash_bound": True,
                    "camera_hash_bound": True,
                    "rig_hash_bound": True,
                    "program_hash_bound": True,
                    "scene_hash_bound": True,
                    "part_id_png_sha_bound": True,
                    "part_id_png_size_bound": True,
                },
                "basis": "closed reference, manifest, camera and PNG bindings",
            },
            "mask_extraction": {
                "status": "PASS",
                "passed": True,
                "checks": {
                    "part_id_aov_present": True,
                    "allowed_ids_nonempty": True,
                    "mask_nonempty": True,
                    "mask_bounds_nonempty": True,
                },
                "basis": "decoded PNG RGB 24-bit part IDs with alpha > 0",
            },
            "calibration": {
                "status": "PASS",
                "passed": True,
                "checks": {
                    "calibration_hash_bound": True,
                    "fit_algorithm_closed": True,
                    "aspect_preserving_centered": True,
                    "candidate_refit_forbidden": True,
                },
                "basis": REPLAY_POLICY,
            },
            "quality_thresholds": _quality_gate(metrics),
        },
        "statuses": {
            "quality_status": "MEASURED_NOT_APPROVED",
            "render_status": "RENDERED",
            "visual_status": "NOT_RUN",
            "human_status": "NOT_RUN",
            "engine_status": "NOT_RUN",
            "commercial_status": "NOT_RUN",
        },
        "provenance": {
            "renderer_used": False,
            "runtime_write": False,
            "reference_basis": "authorized KnifeContourReference@1 plus frozen browser part-id mask calibration",
            "evaluation_boundary": "blade-only FRONT part-id math; no visual or commercial approval",
        },
        "canonical_sha256": "",
    }
    receipt["canonical_sha256"] = canonical_sha256(receipt)
    _validate_receipt(receipt)
    return receipt


def _validate_receipt(receipt: dict[str, Any]) -> str:
    expected = {
        "schema_version", "evaluation_id", "evaluation_mode", "calibration_sha256", "reference_id", "reference_sha256", "capture_manifest_sha256", "baseline_capture_manifest_sha256", "view_id", "camera_fingerprint", "rig_id", "rig_fingerprint", "program_fingerprint", "scene_fingerprint", "baseline_program_fingerprint", "baseline_scene_fingerprint", "frame_width", "frame_height", "allowed_part_ids", "part_id_png_sha256", "part_id_png_size_bytes", "baseline_mask_sha256", "boundary_tolerance_px", "fit_reused", "refit_performed", "fit_source", "metrics", "hard_gates", "statuses", "provenance", "canonical_sha256",
    }
    _exact_keys(receipt, expected, "KnifeBrowserMetricReceipt")
    _require(receipt["schema_version"] == RECEIPT_SCHEMA_VERSION, "metric receipt schema version drifted")
    _id(receipt["evaluation_id"], "receipt.evaluation_id")
    _require(receipt["evaluation_mode"] in {"baseline_calibration", "frozen_replay"}, "receipt evaluation mode is invalid")
    for key in ("calibration_sha256", "reference_sha256", "capture_manifest_sha256", "baseline_capture_manifest_sha256", "camera_fingerprint", "baseline_mask_sha256"):
        _sha(receipt[key], f"receipt.{key}")
    _id(receipt["reference_id"], "receipt.reference_id")
    _require(receipt["view_id"] == VIEW_ID, "receipt view must be FRONT")
    _id(receipt["rig_id"], "receipt.rig_id")
    _require(isinstance(receipt["rig_fingerprint"], str) and FINGERPRINT_RE.fullmatch(receipt["rig_fingerprint"]) is not None, "receipt.rig_fingerprint is invalid")
    for key in ("program_fingerprint", "baseline_program_fingerprint"):
        _require(isinstance(receipt[key], str) and FINGERPRINT_RE.fullmatch(receipt[key]) is not None, f"receipt.{key} is invalid")
    for key in ("scene_fingerprint", "baseline_scene_fingerprint"):
        _sha(receipt[key], f"receipt.{key}")
    for key in ("frame_width", "frame_height"):
        _require(isinstance(receipt[key], int) and MIN_FRAME_SIZE <= receipt[key] <= MAX_FRAME_SIZE, f"receipt.{key} is invalid")
    _part_id_set(receipt["allowed_part_ids"], "receipt.allowed_part_ids")
    _sha(receipt["part_id_png_sha256"], "receipt.part_id_png_sha256")
    _require(isinstance(receipt["part_id_png_size_bytes"], int) and receipt["part_id_png_size_bytes"] > 0, "receipt.part_id_png_size_bytes is invalid")
    _require(isinstance(receipt["boundary_tolerance_px"], int) and 0 <= receipt["boundary_tolerance_px"] <= MAX_BOUNDARY_TOLERANCE_PX, "receipt boundary tolerance is invalid")
    _require(isinstance(receipt["fit_reused"], bool) and isinstance(receipt["refit_performed"], bool), "receipt fit flags are invalid")
    if receipt["evaluation_mode"] == "baseline_calibration":
        _require(receipt["fit_reused"] is False and receipt["refit_performed"] is True and receipt["fit_source"] == "new-baseline-mask", "baseline receipt fit flags drifted")
    else:
        _require(receipt["fit_reused"] is True and receipt["refit_performed"] is False and receipt["fit_source"] == "frozen-calibration-input", "replay receipt fit flags drifted")
    metric_keys = {"silhouette_iou", "boundary_f1", "boundary_precision", "boundary_recall", "symmetric_chamfer", "p95_contour_distance", "landmark_error", "landmark_errors", "landmarks", "reference_area_px", "predicted_area_px", "grid_width", "grid_height"}
    _exact_keys(receipt["metrics"], metric_keys, "receipt.metrics")
    for key in metric_keys - {"landmark_errors", "landmarks", "reference_area_px", "predicted_area_px", "grid_width", "grid_height"}:
        _finite_number(receipt["metrics"][key], f"receipt.metrics.{key}", 0.0)
    _require(isinstance(receipt["metrics"]["landmark_errors"], list) and len(receipt["metrics"]["landmark_errors"]) == len(LANDMARK_IDS), "receipt landmark error list is invalid")
    for index, item in enumerate(receipt["metrics"]["landmark_errors"]):
        _exact_keys(item, {"landmark_id", "error"}, f"receipt.metrics.landmark_errors[{index}]")
        _require(item["landmark_id"] == LANDMARK_IDS[index], "receipt landmark role order drifted")
        _finite_number(item["error"], f"receipt.metrics.landmark_errors[{index}].error", 0.0)
    _exact_keys(receipt["metrics"]["landmarks"], {"reference", "predicted"}, "receipt.metrics.landmarks")
    for label in ("reference", "predicted"):
        values = receipt["metrics"]["landmarks"][label]
        _require(isinstance(values, list) and len(values) == len(LANDMARK_IDS), f"receipt.metrics.landmarks.{label} is invalid")
        for index, item in enumerate(values):
            _exact_keys(item, {"landmark_id", "point"}, f"receipt.metrics.landmarks.{label}[{index}]")
            _require(item["landmark_id"] == LANDMARK_IDS[index], f"receipt.metrics.landmarks.{label} role order drifted")
            _finite_vector(item["point"], 2, f"receipt.metrics.landmarks.{label}[{index}].point")
    for key in ("reference_area_px", "predicted_area_px", "grid_width", "grid_height"):
        _require(isinstance(receipt["metrics"][key], int) and receipt["metrics"][key] > 0, f"receipt.metrics.{key} is invalid")
    _exact_keys(receipt["hard_gates"], {"input_integrity", "mask_extraction", "calibration", "quality_thresholds"}, "receipt.hard_gates")
    for gate_name in ("input_integrity", "mask_extraction", "calibration", "quality_thresholds"):
        _exact_keys(receipt["hard_gates"][gate_name], {"status", "passed", "checks", "basis"} if gate_name != "quality_thresholds" else {"status", "passed", "checks", "thresholds", "basis"}, f"receipt.hard_gates.{gate_name}")
        gate = receipt["hard_gates"][gate_name]
        _require(gate["status"] in {"PASS", "FAIL", "NOT_RUN"}, f"receipt.hard_gates.{gate_name}.status is invalid")
        _require(isinstance(gate["passed"], bool) and isinstance(gate["checks"], dict) and all(isinstance(value, bool) for value in gate["checks"].values()), f"receipt.hard_gates.{gate_name} is invalid")
        _require(isinstance(gate["basis"], str) and gate["basis"], f"receipt.hard_gates.{gate_name}.basis is invalid")
    _exact_keys(receipt["statuses"], {"quality_status", "render_status", "visual_status", "human_status", "engine_status", "commercial_status"}, "receipt.statuses")
    _require(receipt["statuses"]["quality_status"] == "MEASURED_NOT_APPROVED", "receipt quality status crossed approval boundary")
    _require(receipt["statuses"]["render_status"] == "RENDERED", "receipt render status must reflect browser capture evidence")
    for key in ("visual_status", "human_status", "engine_status", "commercial_status"):
        _require(receipt["statuses"][key] == "NOT_RUN", f"receipt.{key} must remain NOT_RUN")
    _exact_keys(receipt["provenance"], {"renderer_used", "runtime_write", "reference_basis", "evaluation_boundary"}, "receipt.provenance")
    _require(receipt["provenance"]["renderer_used"] is False and receipt["provenance"]["runtime_write"] is False, "receipt provenance crossed render/write boundary")
    _require(receipt["provenance"]["evaluation_boundary"] == "blade-only FRONT part-id math; no visual or commercial approval", "receipt evaluation boundary drifted")
    _sha(receipt["canonical_sha256"], "receipt.canonical_sha256")
    _require(receipt["canonical_sha256"] == canonical_sha256(receipt), "receipt canonical hash does not match canonical JSON")
    return receipt["canonical_sha256"]


def validate_receipt(receipt: dict[str, Any]) -> str:
    """Validate a closed ``KnifeBrowserMetricReceipt@1``."""

    return _validate_receipt(receipt)


def evaluate(reference: dict[str, Any], manifest: dict[str, Any], part_id_png: Path, allowed_ids: tuple[int, ...], mode: str, calibration: dict[str, Any] | None = None, tolerance_px: int = 1) -> tuple[dict[str, Any] | None, dict[str, Any]]:
    _require(mode in {"baseline", "replay"}, "mode must be baseline or replay")
    _require(0 <= tolerance_px <= MAX_BOUNDARY_TOLERANCE_PX, "boundary tolerance is outside the bounded range")
    reference_sha = _validate_reference(reference)
    manifest_sha, manifest_view = _validate_manifest(manifest, VIEW_ID)
    camera_sha = _validate_camera(manifest_view["camera"], VIEW_ID)
    mask, mask_info = _load_part_id_mask(part_id_png, manifest_view, manifest["frame_width"], manifest["frame_height"], allowed_ids)
    reference_contour, reference_landmarks = _reference_contour_and_landmarks(reference)
    if mode == "baseline":
        target_bbox = mask_info["bbox"]
        fit = _make_fit(reference_contour, target_bbox)
        calibration_out = _make_calibration(reference, reference_sha, manifest, manifest_sha, manifest_view["camera"], camera_sha, mask_info, allowed_ids, fit)
        calibration_sha = calibration_out["canonical_sha256"]
    else:
        _require(calibration is not None, "replay requires --calibration")
        calibration_sha = _validate_calibration(calibration)
        _validate_replay_bindings(calibration, reference, reference_sha, manifest, manifest_sha, camera_sha, allowed_ids)
        calibration_out = None
        fit = calibration["fit"]
    transformed_contour = _transform_points(reference_contour, fit)
    transformed_landmarks = {role: _transform_point(reference_landmarks[role], fit) for role in LANDMARK_IDS}
    metrics = _metric_values(mask, transformed_contour, transformed_landmarks, tolerance_px)
    receipt = _make_receipt(reference, reference_sha, manifest, manifest_sha, camera_sha, mask_info, allowed_ids, calibration_out if calibration_out is not None else calibration, calibration_sha, metrics, "baseline_calibration" if mode == "baseline" else "frozen_replay", tolerance_px)
    return calibration_out, receipt


def _write_json(path: Path, value: dict[str, Any]) -> None:
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    except OSError as exc:
        raise CalibrationInputError(f"cannot write output {path}: {exc}") from exc


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("baseline", "replay"), required=True)
    parser.add_argument("--reference", required=True, type=Path)
    parser.add_argument("--capture-manifest", required=True, type=Path)
    parser.add_argument("--part-id-png", required=True, type=Path)
    parser.add_argument("--allowed-part-ids", required=True, help="comma-separated non-background 24-bit part IDs, for example 1,2")
    parser.add_argument("--calibration", type=Path, help="frozen KnifeReferenceBrowserCalibration@1 for replay")
    parser.add_argument("--calibration-output", type=Path, help="baseline calibration JSON output")
    parser.add_argument("--receipt-output", type=Path, required=True)
    parser.add_argument("--boundary-tolerance-px", type=int, default=1)
    args = parser.parse_args(argv)
    try:
        if args.mode == "baseline":
            _require(args.calibration is None, "baseline must not accept a calibration input")
        else:
            _require(args.calibration is not None, "replay requires --calibration")
            _require(args.calibration_output is None, "replay cannot overwrite or emit a calibration")
        allowed_ids = parse_allowed_part_ids(args.allowed_part_ids)
        reference = load_json(args.reference)
        manifest = load_json(args.capture_manifest)
        calibration = load_json(args.calibration) if args.calibration is not None else None
        calibration_out, receipt = evaluate(reference, manifest, args.part_id_png, allowed_ids, args.mode, calibration, args.boundary_tolerance_px)
        if calibration_out is not None:
            _require(args.calibration_output is not None, "baseline requires --calibration-output")
            _write_json(args.calibration_output, calibration_out)
        _write_json(args.receipt_output, receipt)
        print(json.dumps({
            "schema_version": RECEIPT_SCHEMA_VERSION,
            "evaluation_mode": receipt["evaluation_mode"],
            "calibration_sha256": receipt["calibration_sha256"],
            "capture_manifest_sha256": receipt["capture_manifest_sha256"],
            "part_id_png_sha256": receipt["part_id_png_sha256"],
            "fit_reused": receipt["fit_reused"],
            "refit_performed": receipt["refit_performed"],
            "metrics": receipt["metrics"],
            "quality_status": receipt["statuses"]["quality_status"],
            "canonical_sha256": receipt["canonical_sha256"],
        }, ensure_ascii=False, sort_keys=True))
        return 0
    except (CalibrationInputError, ValueError, OSError) as exc:
        print(f"CALIBRATION_NOT_RUN: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
