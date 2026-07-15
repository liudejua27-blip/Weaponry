"""Deterministic software rendering for Agent concept views.

This module is deliberately independent from the legacy Concept renderer and
export routes.  It consumes the immutable GLB produced by the Agent asset
pipeline, projects its triangles in software, and returns four lightweight
PNG views.  It is visual concept evidence only; it is not engineering or
photo-realistic rendering.
"""

from __future__ import annotations

import binascii
import math
import struct
import zlib
from dataclasses import dataclass
from typing import Iterable, Sequence

from forgecad_agent.application.combined_obj import CombinedObjError, CombinedObjResult, build_combined_obj


class AgentRenderError(ValueError):
    """Raised when an Agent GLB cannot be rendered as concept imagery."""


Vector3 = tuple[float, float, float]
Color4 = tuple[float, float, float, float]
VIEW_ORDER = ("iso", "front", "side", "top")
VIEW_CAMERAS: dict[str, tuple[Vector3, Vector3]] = {
    "iso": ((1.0, 0.72, 1.0), (0.0, 1.0, 0.0)),
    "front": ((0.0, 0.0, 1.0), (0.0, 1.0, 0.0)),
    "side": ((1.0, 0.0, 0.0), (0.0, 1.0, 0.0)),
    "top": ((0.0, 1.0, 0.0), (0.0, 0.0, -1.0)),
}


@dataclass(frozen=True)
class AgentRenderResult:
    views: dict[str, bytes]
    width: int
    height: int
    exploded_part_ids: tuple[str, ...] = ()
    exploded_unavailable_reason: str | None = None


@dataclass(frozen=True)
class ExplodedPartOffset:
    """A deterministic, visual-only offset for one stable Agent part."""

    part_id: str
    offset: Vector3


@dataclass(frozen=True)
class _Triangle:
    points: tuple[Vector3, Vector3, Vector3]
    color: Color4
    group_index: int


def render_agent_views(
    glb: bytes,
    *,
    width: int = 640,
    height: int = 640,
    exploded_parts: Sequence[ExplodedPartOffset] = (),
) -> AgentRenderResult:
    """Render immutable Agent views, with an optional facts-bound exploded candidate.

    The exploded view is deliberately conditional: it is only rendered when
    every supplied stable Part maps one-to-one to an existing GLB primitive
    group.  This avoids inventing part separation for imported or flattened
    geometry that has no trustworthy AssemblyGraph-to-mesh correspondence.
    """
    if not 64 <= width <= 2048 or not 64 <= height <= 2048:
        raise AgentRenderError("render dimensions must be between 64 and 2048")
    try:
        obj = build_combined_obj(glb)
        triangles = _triangles_from_obj(obj)
        views: dict[str, bytes] = {
            view_id: _render_png(
                triangles,
                width,
                height,
                camera_vector=VIEW_CAMERAS[view_id][0],
                up_hint=VIEW_CAMERAS[view_id][1],
            )
            for view_id in VIEW_ORDER
        }
    except (CombinedObjError, KeyError, IndexError, TypeError, ValueError) as exc:
        if isinstance(exc, AgentRenderError):
            raise
        raise AgentRenderError(f"agent concept render failed: {exc}") from exc
    for view_id in VIEW_ORDER:
        _readback_png(views[view_id], width=width, height=height, require_transparent_background=True)

    exploded_part_ids: tuple[str, ...] = ()
    exploded_unavailable_reason: str | None = None
    if exploded_parts:
        try:
            if len(exploded_parts) < 2:
                raise AgentRenderError("至少需要两个已映射部件才能生成爆炸概念图")
            if len({item.part_id for item in exploded_parts}) != len(exploded_parts):
                raise AgentRenderError("爆炸概念图的部件标识必须唯一")
            exploded_triangles = _translate_triangle_groups(triangles, exploded_parts)
            views["exploded_iso"] = _render_png(
                exploded_triangles,
                width,
                height,
                camera_vector=VIEW_CAMERAS["iso"][0],
                up_hint=VIEW_CAMERAS["iso"][1],
            )
            _readback_png(
                views["exploded_iso"],
                width=width,
                height=height,
                require_transparent_background=True,
            )
            exploded_part_ids = tuple(item.part_id for item in exploded_parts)
        except AgentRenderError as exc:
            exploded_unavailable_reason = str(exc)
    return AgentRenderResult(
        views=views,
        width=width,
        height=height,
        exploded_part_ids=exploded_part_ids,
        exploded_unavailable_reason=exploded_unavailable_reason,
    )


def _triangles_from_obj(result: CombinedObjResult) -> list[_Triangle]:
    materials = _parse_mtl(result.mtl.decode("utf-8"))
    vertices: list[Vector3] = []
    triangles: list[_Triangle] = []
    current_material = "mat_default"
    current_group_index = -1
    for raw_line in result.obj.decode("utf-8").splitlines():
        line = raw_line.strip()
        if line.startswith("v "):
            values = tuple(float(value) for value in line.split()[1:])
            if len(values) != 3 or not all(math.isfinite(value) for value in values):
                raise AgentRenderError("OBJ vertex is invalid")
            vertices.append(values)  # type: ignore[arg-type]
        elif line.startswith("usemtl "):
            current_material = line.split(maxsplit=1)[1]
        elif line.startswith("g "):
            current_group_index += 1
        elif line.startswith("f "):
            tokens = line.split()[1:]
            if len(tokens) != 3:
                raise AgentRenderError("software renderer requires triangle OBJ faces")
            indices = [int(token.split("/", 1)[0]) - 1 for token in tokens]
            if any(index < 0 or index >= len(vertices) for index in indices):
                raise AgentRenderError("OBJ face references an invalid vertex")
            triangles.append(
                _Triangle(
                    points=tuple(vertices[index] for index in indices),  # type: ignore[arg-type]
                    color=materials.get(current_material, (0.8, 0.8, 0.8, 1.0)),
                    group_index=max(0, current_group_index),
                )
            )
    if not vertices or not triangles:
        raise AgentRenderError("OBJ contains no renderable triangles")
    return triangles


def _translate_triangle_groups(
    triangles: Sequence[_Triangle],
    exploded_parts: Sequence[ExplodedPartOffset],
) -> list[_Triangle]:
    group_count = max((item.group_index for item in triangles), default=-1) + 1
    if group_count != len(exploded_parts):
        raise AgentRenderError(
            "当前模型的几何分组与可编辑部件不一一对应，无法安全生成爆炸概念图"
        )
    offsets = [item.offset for item in exploded_parts]
    if any(len(offset) != 3 or not all(math.isfinite(value) for value in offset) for offset in offsets):
        raise AgentRenderError("爆炸概念图的视觉间距无效")
    return [
        _Triangle(
            points=tuple(_add(point, offsets[triangle.group_index]) for point in triangle.points),  # type: ignore[arg-type]
            color=triangle.color,
            group_index=triangle.group_index,
        )
        for triangle in triangles
    ]


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
    triangles: Sequence[_Triangle],
    width: int,
    height: int,
    *,
    camera_vector: Vector3,
    up_hint: Vector3,
) -> bytes:
    camera = _normalize(camera_vector)
    right = _normalize(_cross(up_hint, camera))
    screen_up = _normalize(_cross(camera, right))
    projected = [
        (_dot(point, right), _dot(point, screen_up), _dot(point, camera))
        for triangle in triangles
        for point in triangle.points
    ]
    min_x = min(point[0] for point in projected)
    max_x = max(point[0] for point in projected)
    min_y = min(point[1] for point in projected)
    max_y = max(point[1] for point in projected)
    span_x = max_x - min_x
    span_y = max_y - min_y
    if span_x <= 1e-12 and span_y <= 1e-12:
        raise AgentRenderError("render bounds have no visible extent")
    scale = min(width * 0.84 / max(span_x, 1e-12), height * 0.84 / max(span_y, 1e-12))
    center_x = (min_x + max_x) * 0.5
    center_y = (min_y + max_y) * 0.5
    pixels = bytearray(width * height * 4)
    depths = [float("-inf")] * (width * height)
    light = _normalize((0.35, 0.9, 0.45))
    if abs(camera[1]) < 0.94:
        _draw_contact_shadow(
            pixels,
            width,
            height,
            center_x=width * 0.5,
            center_y=height * 0.5 - (min_y - center_y) * scale + height * 0.012,
            radius_x=max(width * 0.08, span_x * scale * 0.38),
            radius_y=max(height * 0.012, span_y * scale * 0.055),
        )
    for triangle in triangles:
        first, second, third = triangle.points
        normal_raw = _cross(_subtract(second, first), _subtract(third, first))
        if _length(normal_raw) <= 1e-15:
            continue
        normal = _normalize(normal_raw)
        shade = min(1.15, 0.46 + abs(_dot(normal, light)) * 0.5 + abs(_dot(normal, camera)) * 0.12)
        rgb = [_linear_to_srgb(min(1.0, max(0.0, component * shade))) for component in triangle.color[:3]]
        rgba = (round(rgb[0] * 255), round(rgb[1] * 255), round(rgb[2] * 255), round(triangle.color[3] * 255))
        screen = [
            (
                (_dot(point, right) - center_x) * scale + width * 0.5,
                height * 0.5 - (_dot(point, screen_up) - center_y) * scale,
                _dot(point, camera),
            )
            for point in triangle.points
        ]
        _rasterize_triangle(pixels, depths, width, height, screen, rgba)
    return _encode_png_rgba(width, height, _antialias_transparent_edges(pixels, width, height))


def _draw_contact_shadow(pixels: bytearray, width: int, height: int, *, center_x: float, center_y: float, radius_x: float, radius_y: float) -> None:
    min_x = max(0, math.floor(center_x - radius_x))
    max_x = min(width - 1, math.ceil(center_x + radius_x))
    min_y = max(0, math.floor(center_y - radius_y))
    max_y = min(height - 1, math.ceil(center_y + radius_y))
    for y in range(min_y, max_y + 1):
        normalized_y = (y + 0.5 - center_y) / radius_y
        for x in range(min_x, max_x + 1):
            normalized_x = (x + 0.5 - center_x) / radius_x
            distance_squared = normalized_x * normalized_x + normalized_y * normalized_y
            if distance_squared >= 1.0:
                continue
            alpha = round(66 * (1.0 - distance_squared) ** 2)
            if alpha:
                offset = (y * width + x) * 4
                pixels[offset : offset + 4] = bytes((12, 18, 28, alpha))


def _antialias_transparent_edges(pixels: bytearray, width: int, height: int) -> bytes:
    source = bytes(pixels)
    output = bytearray(source)
    stride = width * 4
    for y in range(1, height - 1):
        for x in range(1, width - 1):
            offset = y * stride + x * 4
            if source[offset + 3] != 0:
                continue
            neighbors = (offset - 4, offset + 4, offset - stride, offset + stride)
            opaque = [candidate for candidate in neighbors if source[candidate + 3] >= 224]
            if not opaque:
                continue
            output[offset] = sum(source[candidate] for candidate in opaque) // len(opaque)
            output[offset + 1] = sum(source[candidate + 1] for candidate in opaque) // len(opaque)
            output[offset + 2] = sum(source[candidate + 2] for candidate in opaque) // len(opaque)
            output[offset + 3] = min(128, 48 + len(opaque) * 20)
    return bytes(output)


def _rasterize_triangle(pixels: bytearray, depths: list[float], width: int, height: int, points: Sequence[tuple[float, float, float]], rgba: tuple[int, int, int, int]) -> None:
    first, second, third = points
    area = _edge(first, second, third[0], third[1])
    if abs(area) <= 1e-12:
        return
    min_x = max(0, math.floor(min(point[0] for point in points)))
    max_x = min(width - 1, math.ceil(max(point[0] for point in points)))
    min_y = max(0, math.floor(min(point[1] for point in points)))
    max_y = min(height - 1, math.ceil(max(point[1] for point in points)))
    for y in range(min_y, max_y + 1):
        for x in range(min_x, max_x + 1):
            sample_x, sample_y = x + 0.5, y + 0.5
            weight_a = _edge(second, third, sample_x, sample_y) / area
            weight_b = _edge(third, first, sample_x, sample_y) / area
            weight_c = 1.0 - weight_a - weight_b
            if min(weight_a, weight_b, weight_c) < -1e-8:
                continue
            depth = weight_a * first[2] + weight_b * second[2] + weight_c * third[2]
            pixel_index = y * width + x
            if depth <= depths[pixel_index] + 1e-9:
                continue
            depths[pixel_index] = depth
            offset = pixel_index * 4
            pixels[offset : offset + 4] = bytes(rgba)


def _encode_png_rgba(width: int, height: int, pixels: bytes) -> bytes:
    scanlines = b"".join(b"\x00" + pixels[row * width * 4 : (row + 1) * width * 4] for row in range(height))
    signature = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return signature + _png_chunk(b"IHDR", ihdr) + _png_chunk(b"sRGB", b"\x00") + _png_chunk(b"IDAT", zlib.compress(scanlines, level=9)) + _png_chunk(b"IEND", b"")


def _png_chunk(kind: bytes, payload: bytes) -> bytes:
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", binascii.crc32(kind + payload) & 0xFFFFFFFF)


def _readback_png(
    payload: bytes,
    *,
    width: int,
    height: int,
    require_transparent_background: bool = False,
) -> None:
    if len(payload) < 33 or payload[:8] != b"\x89PNG\r\n\x1a\n" or payload[12:16] != b"IHDR":
        raise AgentRenderError("render output is not a PNG with an IHDR chunk")
    actual_width, actual_height, bit_depth, color_type, compression, filtering, interlace = struct.unpack(">IIBBBBB", payload[16:29])
    if (actual_width, actual_height) != (width, height) or (bit_depth, color_type, compression, filtering, interlace) != (8, 6, 0, 0, 0):
        raise AgentRenderError("render PNG metadata does not match the requested RGBA dimensions")
    if require_transparent_background:
        _assert_png_has_transparent_background(payload, width=width, height=height)


def _assert_png_has_transparent_background(payload: bytes, *, width: int, height: int) -> None:
    position = 8
    compressed: list[bytes] = []
    while position + 12 <= len(payload):
        length = struct.unpack(">I", payload[position : position + 4])[0]
        kind = payload[position + 4 : position + 8]
        end = position + 12 + length
        if end > len(payload):
            raise AgentRenderError("render PNG chunk exceeds payload")
        if kind == b"IDAT":
            compressed.append(payload[position + 8 : position + 8 + length])
        if kind == b"IEND":
            break
        position = end
    try:
        decoded = zlib.decompress(b"".join(compressed))
    except zlib.error as exc:
        raise AgentRenderError("render PNG alpha readback failed") from exc
    stride = width * 4
    expected = height * (stride + 1)
    if len(decoded) != expected:
        raise AgentRenderError("render PNG scanlines do not match dimensions")
    alphas = [decoded[row * (stride + 1) + 1 + pixel * 4 + 3] for row in range(height) for pixel in range(width)]
    if not any(alpha == 0 for alpha in alphas) or not any(alpha > 0 for alpha in alphas):
        raise AgentRenderError("render PNG does not retain a transparent background")


def _linear_to_srgb(value: float) -> float:
    return value * 12.92 if value <= 0.0031308 else 1.055 * value ** (1 / 2.4) - 0.055


def _edge(first: Sequence[float], second: Sequence[float], x: float, y: float) -> float:
    return (x - first[0]) * (second[1] - first[1]) - (y - first[1]) * (second[0] - first[0])


def _dot(first: Sequence[float], second: Sequence[float]) -> float:
    return sum(a * b for a, b in zip(first, second))


def _cross(first: Sequence[float], second: Sequence[float]) -> Vector3:
    return (first[1] * second[2] - first[2] * second[1], first[2] * second[0] - first[0] * second[2], first[0] * second[1] - first[1] * second[0])


def _subtract(first: Sequence[float], second: Sequence[float]) -> Vector3:
    return tuple(a - b for a, b in zip(first, second))  # type: ignore[return-value]


def _add(first: Sequence[float], second: Sequence[float]) -> Vector3:
    return tuple(a + b for a, b in zip(first, second))  # type: ignore[return-value]


def _length(value: Sequence[float]) -> float:
    return math.sqrt(sum(component * component for component in value))


def _normalize(value: Iterable[float]) -> Vector3:
    rendered = tuple(float(component) for component in value)
    length = _length(rendered)
    if len(rendered) != 3 or length <= 1e-15 or not math.isfinite(length):
        raise AgentRenderError("cannot normalize a degenerate vector")
    return tuple(component / length for component in rendered)  # type: ignore[return-value]
