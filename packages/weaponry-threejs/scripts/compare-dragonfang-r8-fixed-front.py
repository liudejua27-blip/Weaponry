#!/usr/bin/env python3
"""Compare r7 and r8 FRONT blade masks under one immutable camera/reference fit."""

from __future__ import annotations

import copy
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

from PIL import Image, ImageDraw, ImageFont


REPO = Path(__file__).resolve().parents[3]
SKILL = REPO / "skills/weaponry-threejs-knife-studio"
sys.path.insert(0, str(SKILL / "scripts"))
import calibrate_browser_reference as calibration  # noqa: E402


R7_DIR = REPO / "packages/weaponry-threejs/evidence/rendered-r7-fixed-views"
R8_DIR = REPO / "packages/weaponry-threejs/evidence/rendered-r8-fixed-views"
OUTPUT_DIR = REPO / "packages/weaponry-threejs/evidence/dragonfang-r8-front-comparison"
REFERENCE_PATH = SKILL / "references/dragonfang-front-blade-reference.json"
REFERENCE_CROP_PATH = REPO / "packages/weaponry-threejs/evidence/reference-crops/dragonfang-front.png"
MAPPING_PATH = SKILL / "references/dragonfang-r8-reference-view-mapping.json"
SOURCE_CALIBRATION_SHA256 = "7e1bd17f939b0aa288bcf010e4eb6ff3156d1e504d52d8755b1c8c25b8f953ec"
FRONT_CAMERA_SHA256 = "6f92f25e1a01cda6fd7e0b78806a0351b753358b4eeb582393b5bc4c783b1104"
R7_PROGRAM_SHA256 = "24b4f6e558f59c825daf1127cb5751a79204e44bd2b3b116420ef8112f8a113f"
R8_PROGRAM_SHA256 = "0c495db1c8ff2c0079cd5dafc3270eaafa9eae0ae1d0f41b6099d65db8ec51e1"
WORKER_COHORT_SHA256 = "92f8d4ca303851e9eead9cd337f5bf6821980fd76eb8026d617d744b0c7f5044"
ALLOWED_PART_IDS = (1, 2)
FROZEN_FIT = {
    "algorithm": "aspect-preserving-centered-reference-bbox-to-baseline-mask-bbox@1",
    "source_coordinate_space": "unit-square@1",
    "target_coordinate_space": "capture-pixel-normalized-top-left@1",
    "axis_conversion": "reference-bottom-left-to-image-top-left@1",
    "source_bbox": [0.06, 0.773019391, 0.94, 0.938815443],
    "target_bbox": [0.02734375, 0.390625, 0.80859375, 0.62109375],
    "source_center": [0.5, 0.855917417],
    "target_center": [0.41796875, 0.505859375],
    "scale": 0.887784090909,
    "translation": [-0.025923295455, -0.254010490945],
}


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def canonical_sha256(value: dict[str, Any]) -> str:
    draft = copy.deepcopy(value)
    draft["canonical_sha256"] = ""
    return hashlib.sha256(canonical_bytes(draft)).hexdigest()


def file_sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(f"WPN_THREE_R8_FIXED_COMPARE_INVALID: {message}")


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    require(isinstance(value, dict), f"{path.name} must be an object")
    return value


def load_blade_mask(path: Path) -> tuple[list[list[bool]], dict[str, Any]]:
    payload = path.read_bytes()
    with Image.open(path) as source:
        require(source.format == "PNG" and source.size == (512, 512), f"{path.name} must be a fixed 512 PNG")
        rgba = source.convert("RGBA").transpose(Image.Transpose.FLIP_LEFT_RIGHT)
        pixels = rgba.load()
        mask: list[list[bool]] = []
        observed: set[int] = set()
        for y in range(512):
            row: list[bool] = []
            for x in range(512):
                red, green, blue, alpha = pixels[x, y]
                part_id = (red << 16) | (green << 8) | blue
                if alpha > 0 and part_id > 0:
                    observed.add(part_id)
                row.append(alpha > 0 and part_id in ALLOWED_PART_IDS)
            mask.append(row)
    bounds = calibration._mask_bounds(mask)
    require(bounds is not None, f"{path.name} contains no blade-body/cutting-edge mask")
    return mask, {
        "png_sha256": hashlib.sha256(payload).hexdigest(),
        "mask_sha256": calibration._hash_binary_mask(mask),
        "pixel_count": calibration._mask_count(mask),
        "bounds_px": list(bounds),
        "bbox": calibration._normalized_pixel_bbox(bounds, 512, 512),
        "observed_part_ids": sorted(observed),
        "handedness_transform": "mirror-render-x-to-reference@1",
    }


def metric_delta(before: dict[str, Any], after: dict[str, Any]) -> dict[str, float]:
    keys = ("silhouette_iou", "boundary_f1", "symmetric_chamfer", "p95_contour_distance", "landmark_error")
    return {key: round(float(after[key]) - float(before[key]), 12) for key in keys}


def fit_image(source: Image.Image, size: tuple[int, int]) -> Image.Image:
    canvas = Image.new("RGB", size, "#0b1015")
    image = source.convert("RGB")
    image.thumbnail((size[0] - 32, size[1] - 56), Image.Resampling.LANCZOS)
    canvas.paste(image, ((size[0] - image.width) // 2, (size[1] - image.height) // 2 + 18))
    return canvas


def make_sheet(reference_contour: list[list[float]], r7_mask: list[list[bool]], r8_mask: list[list[bool]]) -> None:
    labels = ("AUTHORIZED FRONT REFERENCE", "R7 BASELINE / SAME CAMERA", "R8 CANDIDATE / SAME CAMERA")
    with Image.open(REFERENCE_CROP_PATH) as ref, Image.open(R7_DIR / "front-beauty.png") as r7_beauty, Image.open(R8_DIR / "front-beauty.png") as r8_beauty:
        panels = [fit_image(ref, (640, 420)), fit_image(r7_beauty.transpose(Image.Transpose.FLIP_LEFT_RIGHT), (640, 420)), fit_image(r8_beauty.transpose(Image.Transpose.FLIP_LEFT_RIGHT), (640, 420))]
    sheet = Image.new("RGB", (1920, 840), "#080d12")
    draw = ImageDraw.Draw(sheet)
    font = ImageFont.load_default()
    for index, (label, panel) in enumerate(zip(labels, panels)):
        x = index * 640
        sheet.paste(panel, (x, 0))
        draw.text((x + 18, 14), label, fill="#f1f5f9", font=font)
    for index, (label, mask) in enumerate((("R7 MASK + FROZEN REFERENCE", r7_mask), ("R8 MASK + FROZEN REFERENCE", r8_mask))):
        panel = Image.new("RGB", (512, 360), "#0b1015")
        overlay = Image.new("RGB", (512, 512), "#0b1015")
        pixels = overlay.load()
        for y in range(512):
            for x in range(512):
                if mask[y][x]:
                    pixels[x, y] = (128, 42, 34)
        line = [(round(point[0] * 512), round(point[1] * 512)) for point in reference_contour]
        ImageDraw.Draw(overlay).line(line + [line[0]], fill=(245, 200, 72), width=2)
        overlay.thumbnail((500, 320), Image.Resampling.NEAREST)
        panel.paste(overlay, ((512 - overlay.width) // 2, 30))
        x = 224 + index * 960
        sheet.paste(panel, (x, 460))
        draw.text((x + 10, 474), label, fill="#f1f5f9", font=font)
    sheet.save(OUTPUT_DIR / "r7-r8-front-reference-comparison.png")


def make_eight_view_sheet() -> None:
    view_ids = ("front", "back", "top", "bottom", "left", "right", "rear_three_quarter", "fps_hold")
    sheet = Image.new("RGB", (2048, 1024), "#080d12")
    draw = ImageDraw.Draw(sheet)
    font = ImageFont.load_default()
    for index, view_id in enumerate(view_ids):
        with Image.open(R8_DIR / f"{view_id}-beauty.png") as source:
            panel = fit_image(source, (512, 512))
        x = (index % 4) * 512
        y = (index // 4) * 512
        sheet.paste(panel, (x, y))
        draw.text((x + 16, y + 14), view_id.upper(), fill="#f1f5f9", font=font)
    sheet.save(OUTPUT_DIR / "r8-fixed-eight-view-beauty.png")


def main() -> int:
    reference = load_json(REFERENCE_PATH)
    reference_sha = calibration._validate_reference(reference)
    r7_manifest = load_json(R7_DIR / "manifest.json")
    r8_manifest = load_json(R8_DIR / "manifest.json")
    mapping = load_json(MAPPING_PATH)
    require(reference_sha == "d471982953841212d5b90906f9fb5627a6bd9593d754353f3f476c8702da57d2", "reference contour identity drifted")
    require(r7_manifest.get("program_sha256") == R7_PROGRAM_SHA256 and r8_manifest.get("program_sha256") == R8_PROGRAM_SHA256, "program lineage drifted")
    require(r7_manifest.get("preview_worker_cohort_sha256") == WORKER_COHORT_SHA256 == r8_manifest.get("preview_worker_cohort_sha256"), "baseline/candidate Worker cohort differs")
    front_mapping = next(item for item in mapping["mappings"] if item["reference_view_kind"] == "front")
    require(front_mapping.get("camera_sha256") == FRONT_CAMERA_SHA256, "r8 mapping FRONT camera differs")
    r8_front_camera = next(item["camera_sha256"] for item in r8_manifest["cameras"] if item["view_id"] == "FRONT")
    require(r8_front_camera == FRONT_CAMERA_SHA256, "r8 render FRONT camera differs")
    require(r7_manifest.get("worker_result_sha256") == "ce2ea661551e075748230fe9d6cd4f746d2256bb1fe5eba122e23aa31baf832c", "r7 baseline Worker result drifted")
    calibration._validate_fit(FROZEN_FIT)

    reference_contour, reference_landmarks = calibration._reference_contour_and_landmarks(reference)
    transformed_contour = calibration._transform_points(reference_contour, FROZEN_FIT)
    transformed_landmarks = {key: calibration._transform_point(value, FROZEN_FIT) for key, value in reference_landmarks.items()}
    r7_mask, r7_mask_info = load_blade_mask(R7_DIR / "front-semantic-id.png")
    r8_mask, r8_mask_info = load_blade_mask(R8_DIR / "front-semantic-id.png")
    r7_metrics = calibration._metric_values(r7_mask, transformed_contour, transformed_landmarks, 2)
    r8_metrics = calibration._metric_values(r8_mask, transformed_contour, transformed_landmarks, 2)
    r7_gate = calibration._quality_gate(r7_metrics)
    r8_gate = calibration._quality_gate(r8_metrics)

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    make_sheet(transformed_contour, r7_mask, r8_mask)
    make_eight_view_sheet()
    receipt = {
        "schema_version": "WeaponryThreeJsR8FixedFrontComparison@1",
        "task_id": "WPN-THREE-R8-RENDER-COMPARE-013",
        "asset_id": "Dragonfang Kukri",
        "reference_id": reference["reference_id"],
        "reference_sha256": reference_sha,
        "reference_image_sha256": mapping["reference_sha256"],
        "reference_view_mapping_sha256": mapping["canonical_sha256"],
        "source_calibration_sha256": SOURCE_CALIBRATION_SHA256,
        "calibration_policy": "reuse-normalized-r7-fit-at-512-no-refit@1",
        "fit": FROZEN_FIT,
        "view_id": "FRONT",
        "camera_sha256": FRONT_CAMERA_SHA256,
        "handedness_transform": "mirror-render-x-to-reference@1",
        "allowed_part_ids": list(ALLOWED_PART_IDS),
        "preview_worker_cohort_sha256": WORKER_COHORT_SHA256,
        "baseline": {
            "program_sha256": R7_PROGRAM_SHA256,
            "manifest_sha256": file_sha256(R7_DIR / "manifest.json"),
            "worker_result_sha256": r7_manifest["worker_result_sha256"],
            "mask": r7_mask_info,
            "metrics": {key: value for key, value in r7_metrics.items() if key != "predicted_landmarks"},
            "deterministic_gate": r7_gate,
        },
        "candidate": {
            "program_sha256": R8_PROGRAM_SHA256,
            "manifest_sha256": file_sha256(R8_DIR / "manifest.json"),
            "worker_result_sha256": r8_manifest["worker_result_sha256"],
            "mask": r8_mask_info,
            "metrics": {key: value for key, value in r8_metrics.items() if key != "predicted_landmarks"},
            "deterministic_gate": r8_gate,
        },
        "candidate_minus_baseline": metric_delta(r7_metrics, r8_metrics),
        "agent_visual_review": {
            "status": "REVIEWED_NOT_APPROVED",
            "review_scope": ["blade-body", "cutting-edge"],
            "reviewed_views": ["FRONT", "BACK", "TOP", "BOTTOM", "LEFT", "RIGHT", "REAR_THREE_QUARTER", "FPS_HOLD"],
            "global_fidelity_estimate": 0.4,
            "decision": "refine-spec",
            "improvements": [
                "r8 modestly improves FRONT belly placement and the shoulder-to-belly transition over r7",
                "r8 improves average contour distance and mean landmark error under the frozen reference fit",
            ],
            "blocking_mismatches": [
                "blade-to-handle length ratio remains substantially too short and compact relative to the authorized reference",
                "tip taper remains too blunt and does not reproduce the reference's long converging point",
                "spine arch and lower belly inflection positions remain materially different from the reference contour",
                "REAR_THREE_QUARTER still reads as a thin procedural slab rather than a resolved grind and asymmetric blade volume",
                "p95 contour distance regressed slightly, so the worst local mismatch was not improved",
            ],
            "frozen_systems": ["dragon-relief", "guard", "grip", "pommel", "material-zones"],
            "limit": "deterministic FRONT mask and agent image review do not constitute human, engine, or commercial acceptance",
        },
        "comparison_image": {
            "path": "r7-r8-front-reference-comparison.png",
            "sha256": file_sha256(OUTPUT_DIR / "r7-r8-front-reference-comparison.png"),
        },
        "eight_view_image": {
            "path": "r8-fixed-eight-view-beauty.png",
            "sha256": file_sha256(OUTPUT_DIR / "r8-fixed-eight-view-beauty.png"),
        },
        "comparison_status": "MEASURED_NOT_APPROVED",
        "geometry_modified": False,
        "visual_status": "NOT_APPROVED",
        "human_status": "NOT_RUN",
        "commercial_status": "NOT_RUN",
        "canonical_sha256": "",
    }
    receipt["canonical_sha256"] = canonical_sha256(receipt)
    (OUTPUT_DIR / "comparison.receipt.json").write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({
        "status": "PASS_MEASURED_NOT_APPROVED",
        "camera_sha256": FRONT_CAMERA_SHA256,
        "worker_cohort_sha256": WORKER_COHORT_SHA256,
        "r7_gate": r7_gate["status"],
        "r8_gate": r8_gate["status"],
        "candidate_minus_baseline": receipt["candidate_minus_baseline"],
        "receipt_sha256": receipt["canonical_sha256"],
    }, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
