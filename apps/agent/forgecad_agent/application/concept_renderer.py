from __future__ import annotations

import binascii
import hashlib
import math
import struct
import zlib
from dataclasses import dataclass
from typing import Iterable, Sequence

from forgecad_agent.application.combined_glb import read_glb, write_glb
from forgecad_agent.application.combined_obj import (
    CombinedObjError,
    CombinedObjResult,
    build_combined_obj,
)


class ConceptRenderError(ValueError):
    pass


Vector3 = tuple[float, float, float]
Color4 = tuple[float, float, float, float]
TURNTABLE_FRAME_COUNT = 8
ORTHOGRAPHIC_VIEWS: dict[str, tuple[Vector3, Vector3]] = {
    "front": ((0.0, 0.0, 1.0), (0.0, 1.0, 0.0)),
    "side": ((1.0, 0.0, 0.0), (0.0, 1.0, 0.0)),
    "top": ((0.0, 1.0, 0.0), (0.0, 0.0, -1.0)),
}


@dataclass(frozen=True)
class ConceptRenderResult:
    preview_png: bytes
    exploded_png: bytes
    orthographic_pngs: dict[str, bytes]
    turntable_frames: tuple[bytes, ...]
    width: int
    height: int
    triangle_count: int
    exploded_distance_m: float


@dataclass(frozen=True)
class _RenderTriangle:
    points: tuple[Vector3, Vector3, Vector3]
    color: Color4


def render_concept_pngs(
    combined_glb: bytes,
    *,
    width: int = 640,
    height: int = 640,
) -> ConceptRenderResult:
    if not 64 <= width <= 2048 or not 64 <= height <= 2048:
        raise ConceptRenderError("render dimensions must be between 64 and 2048")
    try:
        preview_obj = build_combined_obj(combined_glb)
        exploded_glb, exploded_distance = _build_exploded_glb(combined_glb)
        exploded_obj = build_combined_obj(exploded_glb)
        preview_triangles = _triangles_from_obj(preview_obj)
        exploded_triangles = _triangles_from_obj(exploded_obj)
        orthographic_pngs = {
            name: _render_png(
                preview_triangles,
                width,
                height,
                camera_vector=camera,
                up_hint=up,
            )
            for name, (camera, up) in ORTHOGRAPHIC_VIEWS.items()
        }
        turntable_frames = tuple(
            _render_png(
                preview_triangles,
                width,
                height,
                camera_vector=(
                    math.cos(2 * math.pi * index / TURNTABLE_FRAME_COUNT),
                    0.42,
                    math.sin(2 * math.pi * index / TURNTABLE_FRAME_COUNT),
                ),
                up_hint=(0.0, 1.0, 0.0),
            )
            for index in range(TURNTABLE_FRAME_COUNT)
        )
        return ConceptRenderResult(
            preview_png=_render_png(
                preview_triangles,
                width,
                height,
                camera_vector=(1.0, 0.72, 1.0),
                up_hint=(0.0, 1.0, 0.0),
            ),
            exploded_png=_render_png(
                exploded_triangles,
                width,
                height,
                camera_vector=(1.0, 0.72, 1.0),
                up_hint=(0.0, 1.0, 0.0),
            ),
            orthographic_pngs=orthographic_pngs,
            turntable_frames=turntable_frames,
            width=width,
            height=height,
            triangle_count=len(preview_triangles),
            exploded_distance_m=exploded_distance,
        )
    except (CombinedObjError, KeyError, IndexError, TypeError, ValueError) as exc:
        if isinstance(exc, ConceptRenderError):
            raise
        raise ConceptRenderError(f"concept render failed: {exc}") from exc


def _build_exploded_glb(combined_glb: bytes) -> tuple[bytes, float]:
    document, binary = read_glb(combined_glb)
    wrappers = [
        node for node in document.get("nodes", []) if node.get("extras", {}).get("forgecad_node_id")
    ]
    if len(wrappers) <= 1:
        return combined_glb, 0.0
    positions = [
        tuple(float(value) for value in node.get("translation", [0, 0, 0])) for node in wrappers
    ]
    center = tuple(sum(point[axis] for point in positions) / len(positions) for axis in range(3))
    span = math.sqrt(
        sum(
            (max(point[axis] for point in positions) - min(point[axis] for point in positions)) ** 2
            for axis in range(3)
        )
    )
    distance = min(0.15, max(0.025, span * 0.35))
    for node, position in zip(wrappers, positions):
        direction = tuple(position[axis] - center[axis] for axis in range(3))
        if _length(direction) <= 1e-9:
            direction = _stable_direction(str(node.get("extras", {}).get("forgecad_node_id")))
        direction = _normalize(direction)
        node["translation"] = [position[axis] + direction[axis] * distance for axis in range(3)]
    return write_glb(document, binary), distance


def _triangles_from_obj(result: CombinedObjResult) -> list[_RenderTriangle]:
    materials = _parse_mtl(result.mtl.decode("utf-8"))
    vertices: list[Vector3] = []
    triangles: list[_RenderTriangle] = []
    current_material = "mat_default"
    for raw_line in result.obj.decode("utf-8").splitlines():
        line = raw_line.strip()
        if line.startswith("v "):
            values = tuple(float(value) for value in line.split()[1:])
            if len(values) != 3 or not all(math.isfinite(value) for value in values):
                raise ConceptRenderError("OBJ vertex is invalid")
            vertices.append(values)  # type: ignore[arg-type]
        elif line.startswith("usemtl "):
            current_material = line.split(maxsplit=1)[1]
        elif line.startswith("f "):
            tokens = line.split()[1:]
            if len(tokens) != 3:
                raise ConceptRenderError("software renderer requires triangle OBJ faces")
            indices = [int(token.split("/", 1)[0]) - 1 for token in tokens]
            if any(index < 0 or index >= len(vertices) for index in indices):
                raise ConceptRenderError("OBJ face references an invalid vertex")
            triangles.append(
                _RenderTriangle(
                    points=tuple(vertices[index] for index in indices),  # type: ignore[arg-type]
                    color=materials.get(current_material, (0.8, 0.8, 0.8, 1.0)),
                )
            )
    if not vertices or not triangles:
        raise ConceptRenderError("OBJ contains no renderable triangles")
    return triangles


def _parse_mtl(text: str) -> dict[str, Color4]:
    values: dict[str, list[float]] = {}
    current: str | None = None
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if line.startswith("newmtl "):
            current = line.split(maxsplit=1)[1]
            values[current] = [0.8, 0.8, 0.8, 1.0]
        elif current is not None and line.startswith("Kd "):
            color = [float(value) for value in line.split()[1:]]
            if len(color) == 3:
                values[current][:3] = color
        elif current is not None and line.startswith("d "):
            values[current][3] = float(line.split()[1])
    return {
        name: tuple(min(1.0, max(0.0, component)) for component in color)  # type: ignore[misc]
        for name, color in values.items()
    }


def _render_png(
    triangles: Sequence[_RenderTriangle],
    width: int,
    height: int,
    *,
    camera_vector: Vector3,
    up_hint: Vector3,
) -> bytes:
    camera = _normalize(camera_vector)
    right = _normalize(_cross(up_hint, camera))
    screen_up = _normalize(_cross(camera, right))
    projected_points = [
        (
            _dot(point, right),
            _dot(point, screen_up),
            _dot(point, camera),
        )
        for triangle in triangles
        for point in triangle.points
    ]
    minimum_x = min(point[0] for point in projected_points)
    maximum_x = max(point[0] for point in projected_points)
    minimum_y = min(point[1] for point in projected_points)
    maximum_y = max(point[1] for point in projected_points)
    span_x = maximum_x - minimum_x
    span_y = maximum_y - minimum_y
    if span_x <= 1e-12 and span_y <= 1e-12:
        raise ConceptRenderError("render bounds have no visible extent")
    scale = min(
        width * 0.84 / max(span_x, 1e-12),
        height * 0.84 / max(span_y, 1e-12),
    )
    center_x = (minimum_x + maximum_x) * 0.5
    center_y = (minimum_y + maximum_y) * 0.5
    pixels = bytearray(width * height * 4)
    depths = [float("-inf")] * (width * height)
    light = _normalize((0.35, 0.9, 0.45))

    for triangle in triangles:
        world_a, world_b, world_c = triangle.points
        normal_raw = _cross(_subtract(world_b, world_a), _subtract(world_c, world_a))
        if _length(normal_raw) <= 1e-15:
            continue
        normal = _normalize(normal_raw)
        diffuse = abs(_dot(normal, light))
        facing = abs(_dot(normal, camera))
        shade = min(1.15, 0.46 + diffuse * 0.5 + facing * 0.12)
        red, green, blue = (
            _linear_to_srgb(min(1.0, max(0.0, component * shade)))
            for component in triangle.color[:3]
        )
        rgba = (
            round(red * 255),
            round(green * 255),
            round(blue * 255),
            round(triangle.color[3] * 255),
        )
        screen = []
        for point in triangle.points:
            projected_x = _dot(point, right)
            projected_y = _dot(point, screen_up)
            screen.append(
                (
                    (projected_x - center_x) * scale + width * 0.5,
                    height * 0.5 - (projected_y - center_y) * scale,
                    _dot(point, camera),
                )
            )
        _rasterize_triangle(pixels, depths, width, height, screen, rgba)
    return _encode_png_rgba(width, height, pixels)


def _rasterize_triangle(
    pixels: bytearray,
    depths: list[float],
    width: int,
    height: int,
    points: Sequence[tuple[float, float, float]],
    rgba: tuple[int, int, int, int],
) -> None:
    first, second, third = points
    area = _edge(first, second, third[0], third[1])
    if abs(area) <= 1e-12:
        return
    minimum_x = max(0, math.floor(min(point[0] for point in points)))
    maximum_x = min(width - 1, math.ceil(max(point[0] for point in points)))
    minimum_y = max(0, math.floor(min(point[1] for point in points)))
    maximum_y = min(height - 1, math.ceil(max(point[1] for point in points)))
    tolerance = -1e-8
    for y in range(minimum_y, maximum_y + 1):
        sample_y = y + 0.5
        for x in range(minimum_x, maximum_x + 1):
            sample_x = x + 0.5
            weight_a = _edge(second, third, sample_x, sample_y) / area
            weight_b = _edge(third, first, sample_x, sample_y) / area
            weight_c = 1.0 - weight_a - weight_b
            if weight_a < tolerance or weight_b < tolerance or weight_c < tolerance:
                continue
            depth = weight_a * first[2] + weight_b * second[2] + weight_c * third[2]
            pixel_index = y * width + x
            if depth <= depths[pixel_index] + 1e-9:
                continue
            depths[pixel_index] = depth
            offset = pixel_index * 4
            pixels[offset : offset + 4] = bytes(rgba)


def _encode_png_rgba(width: int, height: int, pixels: bytes) -> bytes:
    scanlines = b"".join(
        b"\x00" + pixels[row * width * 4 : (row + 1) * width * 4] for row in range(height)
    )
    signature = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return (
        signature
        + _png_chunk(b"IHDR", ihdr)
        + _png_chunk(b"sRGB", b"\x00")
        + _png_chunk(b"IDAT", zlib.compress(scanlines, level=9))
        + _png_chunk(b"IEND", b"")
    )


def _png_chunk(kind: bytes, payload: bytes) -> bytes:
    return (
        struct.pack(">I", len(payload))
        + kind
        + payload
        + struct.pack(">I", binascii.crc32(kind + payload) & 0xFFFFFFFF)
    )


def _stable_direction(value: str) -> Vector3:
    digest = hashlib.sha256(value.encode("utf-8")).digest()
    rendered = tuple((digest[index] / 255.0) * 2.0 - 1.0 for index in range(3))
    return rendered if _length(rendered) > 1e-9 else (1.0, 0.0, 0.0)


def _linear_to_srgb(value: float) -> float:
    if value <= 0.0031308:
        return value * 12.92
    return 1.055 * value ** (1 / 2.4) - 0.055


def _edge(
    first: Sequence[float],
    second: Sequence[float],
    x: float,
    y: float,
) -> float:
    return (x - first[0]) * (second[1] - first[1]) - (y - first[1]) * (second[0] - first[0])


def _dot(first: Sequence[float], second: Sequence[float]) -> float:
    return sum(a * b for a, b in zip(first, second))


def _cross(first: Sequence[float], second: Sequence[float]) -> Vector3:
    return (
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    )


def _subtract(first: Sequence[float], second: Sequence[float]) -> Vector3:
    return tuple(a - b for a, b in zip(first, second))  # type: ignore[return-value]


def _length(value: Sequence[float]) -> float:
    return math.sqrt(sum(component * component for component in value))


def _normalize(value: Iterable[float]) -> Vector3:
    rendered = tuple(float(component) for component in value)
    length = _length(rendered)
    if len(rendered) != 3 or length <= 1e-15 or not math.isfinite(length):
        raise ConceptRenderError("cannot normalize a degenerate vector")
    return tuple(component / length for component in rendered)  # type: ignore[return-value]
