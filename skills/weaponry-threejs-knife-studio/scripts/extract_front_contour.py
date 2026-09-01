#!/usr/bin/env python3
"""Extract one deterministic blade-only front contour from an authorized image.

This is a development/reference-intake helper, not a Runtime operation.  It
uses a caller-confirmed crop, fixed pixel thresholds and the largest connected
foreground component.  The result is a closed KnifeContourReference@1; no
visual, human or commercial approval is inferred.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from collections import deque
from pathlib import Path
from typing import Iterable

from PIL import Image, ImageFilter


MARGIN = 0.06
SAMPLES = 129


def canonical_bytes(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def canonical_sha256(value: dict[str, object]) -> str:
    draft = dict(value)
    draft["canonical_sha256"] = ""
    return hashlib.sha256(canonical_bytes(draft)).hexdigest()


def parse_crop(text: str) -> tuple[int, int, int, int]:
    try:
        values = tuple(int(value) for value in text.split(","))
    except ValueError as exc:
        raise argparse.ArgumentTypeError("crop must be x0,y0,x1,y1 integers") from exc
    if len(values) != 4 or values[0] < 0 or values[1] < 0 or values[2] <= values[0] or values[3] <= values[1]:
        raise argparse.ArgumentTypeError("crop must be a non-empty x0,y0,x1,y1 rectangle")
    return values


def threshold_mask(image: Image.Image) -> Image.Image:
    rgb = image.convert("RGB")
    mask = Image.new("L", rgb.size, 0)
    source = rgb.load()
    target = mask.load()
    for y in range(rgb.height):
        for x in range(rgb.width):
            red, green, blue = source[x, y]
            value = max(red, green, blue)
            chroma = value - min(red, green, blue)
            if (value >= 32 and chroma >= 9) or value >= 66:
                target[x, y] = 255
    # Close narrow gaps in the gold/red silhouette without inventing remote
    # components.  Fixed 5 px morphology is part of the extraction cohort.
    return mask.filter(ImageFilter.MaxFilter(5)).filter(ImageFilter.MinFilter(5))


def largest_component(mask: Image.Image) -> set[tuple[int, int]]:
    pixels = mask.load()
    visited: set[tuple[int, int]] = set()
    best: set[tuple[int, int]] = set()
    for y in range(mask.height):
        for x in range(mask.width):
            start = (x, y)
            if start in visited or pixels[x, y] == 0:
                continue
            queue = deque([start])
            visited.add(start)
            component: set[tuple[int, int]] = set()
            while queue:
                current_x, current_y = queue.popleft()
                component.add((current_x, current_y))
                for next_x, next_y in (
                    (current_x - 1, current_y),
                    (current_x + 1, current_y),
                    (current_x, current_y - 1),
                    (current_x, current_y + 1),
                ):
                    point = (next_x, next_y)
                    if 0 <= next_x < mask.width and 0 <= next_y < mask.height and point not in visited:
                        visited.add(point)
                        if pixels[next_x, next_y] != 0:
                            queue.append(point)
            if len(component) > len(best):
                best = component
    if len(best) < 64:
        raise ValueError("foreground component is too small for a knife contour")
    return best


def interpolate_column(bounds: dict[int, tuple[float, float]], x: float) -> tuple[float, float]:
    left = int(x)
    right = min(left + 1, max(bounds))
    while left not in bounds and left >= min(bounds):
        left -= 1
    while right not in bounds and right <= max(bounds):
        right += 1
    if left not in bounds or right not in bounds:
        raise ValueError("foreground component has an unbounded column gap")
    if left == right:
        return bounds[left]
    fraction = (x - left) / (right - left)
    return (
        bounds[left][0] * (1 - fraction) + bounds[right][0] * fraction,
        bounds[left][1] * (1 - fraction) + bounds[right][1] * fraction,
    )


def smooth(values: list[float], radius: int = 2) -> list[float]:
    return [
        sum(values[max(0, index - radius) : min(len(values), index + radius + 1)])
        / len(values[max(0, index - radius) : min(len(values), index + radius + 1)])
        for index in range(len(values))
    ]


def extract_reference(image_path: Path, crop: tuple[int, int, int, int], reference_id: str) -> tuple[dict[str, object], dict[str, object]]:
    image_bytes = image_path.read_bytes()
    with Image.open(image_path) as image:
        if crop[2] > image.width or crop[3] > image.height:
            raise ValueError("crop exceeds source image bounds")
        cropped = image.convert("RGB").crop(crop)
    component = largest_component(threshold_mask(cropped))
    xs = [point[0] for point in component]
    ys = [point[1] for point in component]
    x_min, x_max = min(xs), max(xs)
    y_min, y_max = min(ys), max(ys)
    if x_max - x_min < 128 or y_max - y_min < 8:
        raise ValueError("foreground component does not have a blade-like horizontal span")

    bounds: dict[int, tuple[float, float]] = {}
    for x, y in component:
        if x not in bounds:
            bounds[x] = (float(y), float(y))
        else:
            bounds[x] = (min(bounds[x][0], y), max(bounds[x][1], y))

    # Source blade points tip-left/root-right. Reverse x so the closed program
    # convention remains root at u=0 and tip at u=1. Image y is also inverted
    # to map the visible spine upward in unit-square coordinates.
    source_x = [x_max - (x_max - x_min) * index / (SAMPLES - 1) for index in range(SAMPLES)]
    raw = [interpolate_column(bounds, x) for x in source_x]
    top = smooth([pair[0] for pair in raw])
    bottom = smooth([pair[1] for pair in raw])
    scale = float(max(x_max - x_min, y_max - y_min))
    usable = 1.0 - 2.0 * MARGIN

    def normalized(index: int, y: float) -> list[float]:
        x = source_x[index]
        return [
            round(MARGIN + (x_max - x) / scale * usable, 9),
            round(MARGIN + (y_max - y) / scale * usable, 9),
        ]

    spine = [normalized(index, top[index]) for index in range(SAMPLES)]
    edge = [normalized(index, bottom[index]) for index in range(SAMPLES)]
    contour = spine + list(reversed(edge))

    landmarks = []
    for role, u in (("root", 0.0), ("shoulder", 0.28), ("belly", 0.68), ("tip", 1.0)):
        index = round(u * (SAMPLES - 1))
        landmarks.append(
            {
                "landmark_id": role,
                "point": [
                    spine[index][0],
                    round((spine[index][1] + edge[index][1]) * 0.5, 9),
                ],
            }
        )

    reference: dict[str, object] = {
        "schema_version": "KnifeContourReference@1",
        "reference_id": reference_id,
        "coordinate_space": "unit-square@1",
        "outer_contour": contour,
        "landmarks": landmarks,
        "camera_frame": {
            "frame_id": "authorized-front-crop-normalized-1",
            "projection": "orthographic-normalized@1",
            "x_min": 0.0,
            "x_max": 1.0,
            "y_min": 0.0,
            "y_max": 1.0,
        },
        "canonical_sha256": "",
    }
    reference["canonical_sha256"] = canonical_sha256(reference)
    receipt = {
        "schema_version": "KnifeContourExtractionReceipt@1",
        "source_image_sha256": hashlib.sha256(image_bytes).hexdigest(),
        "source_image_bytes": len(image_bytes),
        "crop": list(crop),
        "algorithm": "fixed-threshold-largest-component-column-envelope@1",
        "component_pixel_count": len(component),
        "component_bounds": [x_min, y_min, x_max, y_max],
        "contour_point_count": len(contour),
        "reference_sha256": reference["canonical_sha256"],
        "visual_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "commercial_status": "NOT_RUN",
    }
    return reference, receipt


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--crop", required=True, type=parse_crop)
    parser.add_argument("--reference-id", default="dragonfang-front-blade-reference")
    args = parser.parse_args()
    reference, receipt = extract_reference(args.input, args.crop, args.reference_id)
    args.output.write_text(json.dumps(reference, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(receipt, ensure_ascii=False, sort_keys=True))


if __name__ == "__main__":
    main()
