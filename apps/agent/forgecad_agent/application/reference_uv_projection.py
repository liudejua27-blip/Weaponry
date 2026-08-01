"""Bounded local UV evidence bakes for sealed reference PNGs.

This module deliberately does *not* claim to solve a camera or reconstruct
hidden material.  It composites pixels from a sealed PNG into one declared
observed UV rectangle and emits a matching unobserved-texel mask.  The caller
must have already established the image/camera/zone lineage in Rust.

It is a small executable building block for the Appearance Compiler: the
subsequent camera-space rasterizer may supply a richer observed mask without
changing the PBR truth boundary.  No paths, URLs, shaders, scripts, arbitrary
image decoders, or model calls are accepted here.

``ReferenceUvEvidenceBake@1`` is deliberately only the older bounded UV
rectangle compatibility path.  ``ReferenceCameraUvRasterBake@2`` is a real
local camera-space path: a reviewed world-to-clip matrix, all occluding
triangles, and one target material zone are depth-rasterized before source
pixels are written into UV texels.  It does not solve a camera, unwrap a mesh,
de-light a photo, or infer hidden texture.
"""

from __future__ import annotations

import base64
import hashlib
import json
import math
import struct
import zlib
from dataclasses import dataclass
from typing import Mapping, Sequence

import numpy as np


SCHEMA_VERSION = "ReferenceUvEvidenceBake@1"
ALGORITHM_ID = "forgecad.reference_uv_evidence_bake"
ALGORITHM_VERSION = "1"
CAMERA_RASTER_SCHEMA_VERSION = "ReferenceCameraUvRasterBake@2"
CAMERA_RASTER_ALGORITHM_ID = "forgecad.reference_camera_uv_raster"
CAMERA_RASTER_ALGORITHM_VERSION = "1"
MAX_SOURCE_PNG_BYTES = 8 * 1024 * 1024
MAX_TEXTURE_EDGE_PX = 1024
MAX_CAMERA_RASTER_SOURCE_PIXELS = 1024 * 1024
MAX_CAMERA_RASTER_TRIANGLES = 120_000
_PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
_HEX = frozenset("0123456789abcdef")


class ReferenceUvEvidenceBakeError(ValueError):
    """A stable error at the restricted texture-bake boundary."""


@dataclass(frozen=True)
class ReferenceUvEvidenceBake:
    schema_version: str
    projection_id: str
    source_evidence_id: str
    source_image_sha256: str
    source_png_base64: str
    camera_hypothesis_id: str
    camera_provenance_sha256: str
    target_material_zone_id: str
    texture_width: int
    texture_height: int
    # [u_min, v_min, u_max, v_max] in basis points.  It is intentionally a
    # UV evidence crop, not a 3D camera matrix.
    observed_uv_rect_bps: tuple[int, int, int, int]

    @classmethod
    def from_value(cls, value: Mapping[str, object]) -> "ReferenceUvEvidenceBake":
        expected = {
            "schema_version",
            "projection_id",
            "source_evidence_id",
            "source_image_sha256",
            "source_png_base64",
            "camera_hypothesis_id",
            "camera_provenance_sha256",
            "target_material_zone_id",
            "texture_width",
            "texture_height",
            "observed_uv_rect_bps",
        }
        if not isinstance(value, Mapping) or set(value) != expected:
            raise ReferenceUvEvidenceBakeError("reference UV evidence input must contain exactly the sealed fields")
        schema_version = value.get("schema_version")
        if schema_version != SCHEMA_VERSION:
            raise ReferenceUvEvidenceBakeError("reference UV evidence schema is unsupported")
        fields = (
            "projection_id",
            "source_evidence_id",
            "camera_hypothesis_id",
            "target_material_zone_id",
        )
        if any(not _stable_id(value.get(field)) for field in fields):
            raise ReferenceUvEvidenceBakeError("reference UV evidence identity is invalid")
        source_hash = value.get("source_image_sha256")
        camera_hash = value.get("camera_provenance_sha256")
        if not _sha(source_hash) or not _sha(camera_hash):
            raise ReferenceUvEvidenceBakeError("reference UV evidence hashes are invalid")
        source_base64 = value.get("source_png_base64")
        if not isinstance(source_base64, str) or len(source_base64) > ((MAX_SOURCE_PNG_BYTES * 4) // 3) + 8:
            raise ReferenceUvEvidenceBakeError("reference PNG payload exceeds the bounded transport budget")
        try:
            source_bytes = base64.b64decode(source_base64, validate=True)
        except Exception as exc:
            raise ReferenceUvEvidenceBakeError("reference PNG payload is not canonical base64") from exc
        if not source_bytes or len(source_bytes) > MAX_SOURCE_PNG_BYTES:
            raise ReferenceUvEvidenceBakeError("reference PNG byte size is invalid")
        if hashlib.sha256(source_bytes).hexdigest() != source_hash:
            raise ReferenceUvEvidenceBakeError("reference PNG hash does not match sealed evidence")
        _decode_png_rgb(source_bytes)
        width = value.get("texture_width")
        height = value.get("texture_height")
        if type(width) is not int or type(height) is not int or (width, height) not in {(128, 128), (1024, 1024)}:
            raise ReferenceUvEvidenceBakeError("reference UV evidence texture dimensions are outside the reviewed profile")
        rect = value.get("observed_uv_rect_bps")
        if (
            not isinstance(rect, (list, tuple))
            or len(rect) != 4
            or any(type(component) is not int for component in rect)
        ):
            raise ReferenceUvEvidenceBakeError("reference UV evidence rectangle is invalid")
        u0, v0, u1, v1 = tuple(int(component) for component in rect)
        if not (0 <= u0 < u1 <= 10_000 and 0 <= v0 < v1 <= 10_000):
            raise ReferenceUvEvidenceBakeError("reference UV evidence rectangle is outside normalized UV bounds")
        return cls(
            schema_version=SCHEMA_VERSION,
            projection_id=str(value["projection_id"]),
            source_evidence_id=str(value["source_evidence_id"]),
            source_image_sha256=str(source_hash),
            source_png_base64=source_base64,
            camera_hypothesis_id=str(value["camera_hypothesis_id"]),
            camera_provenance_sha256=str(camera_hash),
            target_material_zone_id=str(value["target_material_zone_id"]),
            texture_width=width,
            texture_height=height,
            observed_uv_rect_bps=(u0, v0, u1, v1),
        )

    @property
    def source_png_bytes(self) -> bytes:
        return base64.b64decode(self.source_png_base64, validate=True)

    def as_dict(self, *, include_source_bytes: bool = True) -> dict[str, object]:
        value: dict[str, object] = {
            "schema_version": self.schema_version,
            "projection_id": self.projection_id,
            "source_evidence_id": self.source_evidence_id,
            "source_image_sha256": self.source_image_sha256,
            "camera_hypothesis_id": self.camera_hypothesis_id,
            "camera_provenance_sha256": self.camera_provenance_sha256,
            "target_material_zone_id": self.target_material_zone_id,
            "texture_width": self.texture_width,
            "texture_height": self.texture_height,
            "observed_uv_rect_bps": list(self.observed_uv_rect_bps),
        }
        if include_source_bytes:
            value["source_png_base64"] = self.source_png_base64
        return value

    def canonical_sha256(self) -> str:
        return hashlib.sha256(
            json.dumps(
                {
                    **self.as_dict(include_source_bytes=False),
                    "algorithm_id": ALGORITHM_ID,
                    "algorithm_version": ALGORITHM_VERSION,
                },
                separators=(",", ":"),
                sort_keys=True,
            ).encode("utf-8")
        ).hexdigest()


@dataclass(frozen=True)
class ReferenceUvEvidenceBakeResult:
    base_color_png: bytes
    unobserved_texel_mask_png: bytes
    projection_sha256: str
    base_color_sha256: str
    unobserved_texel_mask_sha256: str
    observed_texel_count: int
    unobserved_texel_count: int


def bake_reference_uv_evidence(
    base_color_png: bytes,
    projection_input: Mapping[str, object] | ReferenceUvEvidenceBake,
) -> ReferenceUvEvidenceBakeResult:
    """Overlay the exact observed UV crop and emit its inverse coverage mask.

    The source image is bilinearly resampled only into the rectangle declared
    by Rust.  Every texel outside the rectangle is kept from the existing
    generated PBR base-color map; the mask records that it was not observed.
    """

    projection = (
        projection_input
        if isinstance(projection_input, ReferenceUvEvidenceBake)
        else ReferenceUvEvidenceBake.from_value(projection_input)
    )
    base = _decode_png_rgb(base_color_png)
    if base.shape[:2] != (projection.texture_height, projection.texture_width):
        raise ReferenceUvEvidenceBakeError("base PBR texture dimensions do not match the sealed projection profile")
    source = _decode_png_rgb(projection.source_png_bytes)
    u0, v0, u1, v1 = projection.observed_uv_rect_bps
    x0 = (u0 * projection.texture_width) // 10_000
    x1 = ((u1 * projection.texture_width) + 9_999) // 10_000
    y0 = (v0 * projection.texture_height) // 10_000
    y1 = ((v1 * projection.texture_height) + 9_999) // 10_000
    x0, x1 = max(0, x0), min(projection.texture_width, x1)
    y0, y1 = max(0, y0), min(projection.texture_height, y1)
    if x0 >= x1 or y0 >= y1:
        raise ReferenceUvEvidenceBakeError("reference UV evidence crop resolves to no texture texels")
    output = base.copy()
    output[y0:y1, x0:x1] = _resize_bilinear(source, x1 - x0, y1 - y0)
    observed = np.zeros((projection.texture_height, projection.texture_width), dtype=np.uint8)
    observed[y0:y1, x0:x1] = 255
    mask = np.repeat((255 - observed)[:, :, None], 3, axis=2)
    base_color_png = _encode_png_rgb(output)
    mask_png = _encode_png_rgb(mask)
    observed_count = int(np.count_nonzero(observed))
    return ReferenceUvEvidenceBakeResult(
        base_color_png=base_color_png,
        unobserved_texel_mask_png=mask_png,
        projection_sha256=projection.canonical_sha256(),
        base_color_sha256=hashlib.sha256(base_color_png).hexdigest(),
        unobserved_texel_mask_sha256=hashlib.sha256(mask_png).hexdigest(),
        observed_texel_count=observed_count,
        unobserved_texel_count=(projection.texture_width * projection.texture_height) - observed_count,
    )


@dataclass(frozen=True)
class CameraUvRasterTriangle:
    """One compiled triangle in asset-local world coordinates.

    The rasterizer never receives a mesh path, arbitrary GLB, shader, or
    source code.  The restricted geometry worker must derive these records
    from its in-memory Rust-sealed compilation payload.  Every zone is
    included because a non-target triangle can occlude a target-zone texel.
    """

    material_zone_id: str
    positions: tuple[
        tuple[float, float, float],
        tuple[float, float, float],
        tuple[float, float, float],
    ]
    uvs: tuple[tuple[float, float], tuple[float, float], tuple[float, float]]


@dataclass(frozen=True)
class ReferenceCameraUvRasterBake:
    """A sealed, bounded camera-space reference-to-UV raster request.

    ``world_to_clip_row_major`` follows the explicit row-major convention.
    It is a compiler input Rust must bind to exact evidence and candidate
    lineage before invoking this sidecar; a browser-supplied matrix is never
    sufficient product truth.
    """

    projection_id: str
    source_evidence_id: str
    source_image_sha256: str
    source_png_base64: str
    camera_hypothesis_id: str
    camera_provenance_sha256: str
    target_material_zone_id: str
    texture_width: int
    texture_height: int
    world_to_clip_row_major: tuple[float, ...]

    @classmethod
    def from_value(cls, value: Mapping[str, object]) -> "ReferenceCameraUvRasterBake":
        expected = {
            "schema_version",
            "projection_id",
            "source_evidence_id",
            "source_image_sha256",
            "source_png_base64",
            "camera_hypothesis_id",
            "camera_provenance_sha256",
            "target_material_zone_id",
            "texture_width",
            "texture_height",
            "world_to_clip_row_major",
        }
        if not isinstance(value, Mapping) or set(value) != expected:
            raise ReferenceUvEvidenceBakeError("camera UV raster input must contain exactly the sealed fields")
        if value.get("schema_version") != CAMERA_RASTER_SCHEMA_VERSION:
            raise ReferenceUvEvidenceBakeError("camera UV raster schema is unsupported")
        for field in (
            "projection_id",
            "source_evidence_id",
            "camera_hypothesis_id",
            "target_material_zone_id",
        ):
            if not _stable_id(value.get(field)):
                raise ReferenceUvEvidenceBakeError("camera UV raster identity is invalid")
        source_hash = value.get("source_image_sha256")
        camera_hash = value.get("camera_provenance_sha256")
        if not _sha(source_hash) or not _sha(camera_hash):
            raise ReferenceUvEvidenceBakeError("camera UV raster hashes are invalid")
        source_base64 = value.get("source_png_base64")
        if not isinstance(source_base64, str) or len(source_base64) > ((MAX_SOURCE_PNG_BYTES * 4) // 3) + 8:
            raise ReferenceUvEvidenceBakeError("camera UV raster PNG payload exceeds the bounded transport budget")
        try:
            source_bytes = base64.b64decode(source_base64, validate=True)
        except Exception as exc:
            raise ReferenceUvEvidenceBakeError("camera UV raster PNG payload is not canonical base64") from exc
        if not source_bytes or len(source_bytes) > MAX_SOURCE_PNG_BYTES:
            raise ReferenceUvEvidenceBakeError("camera UV raster PNG byte size is invalid")
        if hashlib.sha256(source_bytes).hexdigest() != source_hash:
            raise ReferenceUvEvidenceBakeError("camera UV raster PNG hash does not match sealed evidence")
        _decode_png_rgb(source_bytes)
        width = value.get("texture_width")
        height = value.get("texture_height")
        if type(width) is not int or type(height) is not int or (width, height) not in {(128, 128), (1024, 1024)}:
            raise ReferenceUvEvidenceBakeError("camera UV raster texture dimensions are outside the reviewed profile")
        matrix = value.get("world_to_clip_row_major")
        if (
            not isinstance(matrix, (list, tuple))
            or len(matrix) != 16
            or not all(isinstance(item, (int, float)) and math.isfinite(float(item)) for item in matrix)
        ):
            raise ReferenceUvEvidenceBakeError("camera UV raster world-to-clip matrix is invalid")
        normalized_matrix = tuple(float(item) for item in matrix)
        if max(abs(item) for item in normalized_matrix) < 1e-9:
            raise ReferenceUvEvidenceBakeError("camera UV raster world-to-clip matrix is degenerate")
        return cls(
            projection_id=str(value["projection_id"]),
            source_evidence_id=str(value["source_evidence_id"]),
            source_image_sha256=str(source_hash),
            source_png_base64=source_base64,
            camera_hypothesis_id=str(value["camera_hypothesis_id"]),
            camera_provenance_sha256=str(camera_hash),
            target_material_zone_id=str(value["target_material_zone_id"]),
            texture_width=width,
            texture_height=height,
            world_to_clip_row_major=normalized_matrix,
        )

    @property
    def source_png_bytes(self) -> bytes:
        return base64.b64decode(self.source_png_base64, validate=True)

    def as_dict(self, *, include_source_bytes: bool = True) -> dict[str, object]:
        value: dict[str, object] = {
            "schema_version": CAMERA_RASTER_SCHEMA_VERSION,
            "projection_id": self.projection_id,
            "source_evidence_id": self.source_evidence_id,
            "source_image_sha256": self.source_image_sha256,
            "camera_hypothesis_id": self.camera_hypothesis_id,
            "camera_provenance_sha256": self.camera_provenance_sha256,
            "target_material_zone_id": self.target_material_zone_id,
            "texture_width": self.texture_width,
            "texture_height": self.texture_height,
            "world_to_clip_row_major": list(self.world_to_clip_row_major),
        }
        if include_source_bytes:
            value["source_png_base64"] = self.source_png_base64
        return value

    def canonical_sha256(self) -> str:
        return hashlib.sha256(
            json.dumps(
                {
                    "schema_version": CAMERA_RASTER_SCHEMA_VERSION,
                    "algorithm_id": CAMERA_RASTER_ALGORITHM_ID,
                    "projection_id": self.projection_id,
                    "source_evidence_id": self.source_evidence_id,
                    "source_image_sha256": self.source_image_sha256,
                    "camera_hypothesis_id": self.camera_hypothesis_id,
                    "camera_provenance_sha256": self.camera_provenance_sha256,
                    "target_material_zone_id": self.target_material_zone_id,
                    "texture_width": self.texture_width,
                    "texture_height": self.texture_height,
                    "world_to_clip_row_major": list(self.world_to_clip_row_major),
                },
                separators=(",", ":"),
                sort_keys=True,
            ).encode("utf-8")
        ).hexdigest()


@dataclass(frozen=True)
class ReferenceCameraUvRasterBakeResult:
    base_color_png: bytes
    unobserved_texel_mask_png: bytes
    projection_sha256: str
    base_color_sha256: str
    unobserved_texel_mask_sha256: str
    observed_texel_count: int
    unobserved_texel_count: int
    visible_target_source_pixel_count: int
    raster_source_width: int
    raster_source_height: int


def bake_reference_camera_uv_raster(
    base_color_png: bytes,
    projection_input: Mapping[str, object] | ReferenceCameraUvRasterBake,
    triangles: Sequence[CameraUvRasterTriangle],
) -> ReferenceCameraUvRasterBakeResult:
    """Depth-rasterize observed reference pixels into target-zone UV texels.

    Only triangles entirely inside the reviewed clip volume are accepted.
    This intentionally omits partially clipped edge triangles rather than
    guessing a clip polygon.  All zones participate in the depth buffer while
    only the requested zone is written into UV texels, preventing foreground
    geometry from leaking a background reference region into a material.
    """

    projection = (
        projection_input
        if isinstance(projection_input, ReferenceCameraUvRasterBake)
        else ReferenceCameraUvRasterBake.from_value(projection_input)
    )
    if len(triangles) > MAX_CAMERA_RASTER_TRIANGLES:
        raise ReferenceUvEvidenceBakeError("camera UV raster triangle budget is exceeded")
    base = _decode_png_rgb(base_color_png)
    if base.shape[:2] != (projection.texture_height, projection.texture_width):
        raise ReferenceUvEvidenceBakeError("base PBR texture dimensions do not match the sealed projection profile")
    source = _bounded_raster_source(_decode_png_rgb(projection.source_png_bytes))
    source_height, source_width, _ = source.shape
    depth = np.full((source_height, source_width), np.inf, dtype=np.float64)
    winning_triangles = np.full((source_height, source_width), -1, dtype=np.int32)
    projected: list[tuple[np.ndarray, np.ndarray] | None] = []
    for index, triangle in enumerate(triangles):
        screen = _project_triangle_to_screen(
            triangle.positions,
            projection.world_to_clip_row_major,
            source_width,
            source_height,
        )
        projected.append(screen)
        if screen is not None:
            _rasterize_depth(screen, depth, winning_triangles, index)

    sums = np.zeros((projection.texture_height, projection.texture_width, 3), dtype=np.uint64)
    counts = np.zeros((projection.texture_height, projection.texture_width), dtype=np.uint32)
    visible_target_pixels = 0
    for tri_index, triangle in enumerate(triangles):
        if triangle.material_zone_id != projection.target_material_zone_id:
            continue
        screen = projected[tri_index]
        if screen is not None:
            visible_target_pixels += _splat_visible_triangle_to_uv(
                screen,
                triangle.uvs,
                tri_index,
                source,
                winning_triangles,
                sums,
                counts,
            )
    observed = counts > 0
    output = base.copy()
    if np.any(observed):
        output[observed] = (sums[observed] // counts[observed, None]).astype(np.uint8)
    mask = np.repeat((~observed).astype(np.uint8)[:, :, None] * 255, 3, axis=2)
    output_png = _encode_png_rgb(output)
    mask_png = _encode_png_rgb(mask)
    observed_count = int(np.count_nonzero(observed))
    return ReferenceCameraUvRasterBakeResult(
        base_color_png=output_png,
        unobserved_texel_mask_png=mask_png,
        projection_sha256=projection.canonical_sha256(),
        base_color_sha256=hashlib.sha256(output_png).hexdigest(),
        unobserved_texel_mask_sha256=hashlib.sha256(mask_png).hexdigest(),
        observed_texel_count=observed_count,
        unobserved_texel_count=(projection.texture_width * projection.texture_height) - observed_count,
        visible_target_source_pixel_count=visible_target_pixels,
        raster_source_width=source_width,
        raster_source_height=source_height,
    )


def fuse_reference_camera_uv_raster(
    base_color_png: bytes,
    projections: Sequence[ReferenceCameraUvRasterBake],
    triangles: Sequence[CameraUvRasterTriangle],
) -> ReferenceCameraUvRasterBakeResult:
    """Fuse at most two reviewed camera rasters into one retained UV map.

    Each view is rasterized independently against the same immutable triangle
    set. Observed texels are averaged with equal weight; unobserved texels keep
    the generated PBR base and the output mask is the union of both inverse
    masks. The function accepts no arbitrary confidence, priority, image
    path, or camera value, so a second photo cannot silently overwrite the
    first one or claim coverage for an unseen surface.
    """

    if len(projections) != 2:
        raise ReferenceUvEvidenceBakeError("reference UV fusion requires exactly two camera raster views")
    ordered = tuple(sorted(projections, key=lambda item: item.canonical_sha256()))
    base = _decode_png_rgb(base_color_png)
    if base.shape[:2] != (ordered[0].texture_height, ordered[0].texture_width):
        raise ReferenceUvEvidenceBakeError("reference UV fusion base dimensions are invalid")
    sums = np.zeros_like(base, dtype=np.uint64)
    counts = np.zeros(base.shape[:2], dtype=np.uint32)
    visible_target_pixels = 0
    projection_hashes: list[str] = []
    source_width = source_height = 0
    for projection in ordered:
        if (
            projection.texture_width != ordered[0].texture_width
            or projection.texture_height != ordered[0].texture_height
        ):
            raise ReferenceUvEvidenceBakeError("reference UV fusion profiles must match")
        result = bake_reference_camera_uv_raster(
            base_color_png,
            projection,
            triangles,
        )
        projected = _decode_png_rgb(result.base_color_png)
        observed = _decode_png_rgb(result.unobserved_texel_mask_png)[:, :, 0] == 0
        sums[observed] += projected[observed].astype(np.uint64)
        counts[observed] += 1
        visible_target_pixels += result.visible_target_source_pixel_count
        projection_hashes.append(projection.canonical_sha256())
        source_width = result.raster_source_width
        source_height = result.raster_source_height
    observed = counts > 0
    output = base.copy()
    output[observed] = (sums[observed] // counts[observed, None]).astype(np.uint8)
    mask = np.repeat((~observed).astype(np.uint8)[:, :, None] * 255, 3, axis=2)
    output_png = _encode_png_rgb(output)
    mask_png = _encode_png_rgb(mask)
    projection_sha256 = hashlib.sha256(
        json.dumps(
            {
                "schema_version": "ReferenceCameraUvRasterFusion@1",
                "algorithm_id": CAMERA_RASTER_ALGORITHM_ID,
                "algorithm_version": CAMERA_RASTER_ALGORITHM_VERSION,
                "projection_sha256s": projection_hashes,
            },
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
    ).hexdigest()
    observed_count = int(np.count_nonzero(observed))
    return ReferenceCameraUvRasterBakeResult(
        base_color_png=output_png,
        unobserved_texel_mask_png=mask_png,
        projection_sha256=projection_sha256,
        base_color_sha256=hashlib.sha256(output_png).hexdigest(),
        unobserved_texel_mask_sha256=hashlib.sha256(mask_png).hexdigest(),
        observed_texel_count=observed_count,
        unobserved_texel_count=(ordered[0].texture_width * ordered[0].texture_height) - observed_count,
        visible_target_source_pixel_count=visible_target_pixels,
        raster_source_width=source_width,
        raster_source_height=source_height,
    )


def _bounded_raster_source(source: np.ndarray) -> np.ndarray:
    """Downsample only when a sealed source exceeds the reviewed work budget."""

    height, width, channels = source.shape
    if channels != 3:
        raise ReferenceUvEvidenceBakeError("camera UV raster source must be RGB")
    if width * height <= MAX_CAMERA_RASTER_SOURCE_PIXELS:
        return source
    scale = math.sqrt(MAX_CAMERA_RASTER_SOURCE_PIXELS / float(width * height))
    target_width = max(1, min(width, int(math.floor(width * scale))))
    target_height = max(1, min(height, int(math.floor(height * scale))))
    return _resize_bilinear(source, target_width, target_height)


def _project_triangle_to_screen(
    positions: tuple[
        tuple[float, float, float],
        tuple[float, float, float],
        tuple[float, float, float],
    ],
    matrix: tuple[float, ...],
    width: int,
    height: int,
) -> tuple[np.ndarray, np.ndarray] | None:
    """Return screen XY and NDC depth, or omit an invalid/partially clipped triangle."""

    clip_points: list[tuple[float, float, float, float]] = []
    for position in positions:
        if len(position) != 3 or not all(math.isfinite(float(item)) for item in position):
            raise ReferenceUvEvidenceBakeError("camera UV raster triangle position is invalid")
        x, y, z = (float(item) for item in position)
        clip = (
            matrix[0] * x + matrix[1] * y + matrix[2] * z + matrix[3],
            matrix[4] * x + matrix[5] * y + matrix[6] * z + matrix[7],
            matrix[8] * x + matrix[9] * y + matrix[10] * z + matrix[11],
            matrix[12] * x + matrix[13] * y + matrix[14] * z + matrix[15],
        )
        if not all(math.isfinite(item) for item in clip) or clip[3] <= 1e-9:
            return None
        clip_points.append(clip)
    ndc = np.asarray(
        [[point[0] / point[3], point[1] / point[3], point[2] / point[3]] for point in clip_points],
        dtype=np.float64,
    )
    # Proper homogeneous triangle clipping belongs to the future compiler pass.
    # Omitting a partially framed triangle is conservative: no source pixel is
    # written unless its entire face has reviewed camera coverage.
    if np.any(ndc[:, 0] < -1.0) or np.any(ndc[:, 0] > 1.0) or np.any(ndc[:, 1] < -1.0) or np.any(ndc[:, 1] > 1.0) or np.any(ndc[:, 2] < -1.0) or np.any(ndc[:, 2] > 1.0):
        return None
    screen = np.empty((3, 2), dtype=np.float64)
    screen[:, 0] = (ndc[:, 0] * 0.5 + 0.5) * (width - 1)
    screen[:, 1] = (1.0 - (ndc[:, 1] * 0.5 + 0.5)) * (height - 1)
    area = _edge(screen[0], screen[1], screen[2])
    if not math.isfinite(area) or abs(area) <= 1e-9:
        return None
    return screen, ndc[:, 2]


def _raster_bounds(screen: np.ndarray, width: int, height: int) -> tuple[int, int, int, int] | None:
    x0 = max(0, int(math.floor(float(np.min(screen[:, 0])))))
    x1 = min(width - 1, int(math.ceil(float(np.max(screen[:, 0])))))
    y0 = max(0, int(math.floor(float(np.min(screen[:, 1])))))
    y1 = min(height - 1, int(math.ceil(float(np.max(screen[:, 1])))))
    if x0 > x1 or y0 > y1:
        return None
    return x0, x1, y0, y1


def _edge(a: np.ndarray, b: np.ndarray, point: np.ndarray) -> float:
    return float((point[0] - a[0]) * (b[1] - a[1]) - (point[1] - a[1]) * (b[0] - a[0]))


def _barycentric(screen: np.ndarray, point: np.ndarray) -> tuple[float, float, float] | None:
    area = _edge(screen[0], screen[1], screen[2])
    if abs(area) <= 1e-9:
        return None
    first = _edge(screen[1], screen[2], point) / area
    second = _edge(screen[2], screen[0], point) / area
    third = 1.0 - first - second
    epsilon = 1e-9
    if first < -epsilon or second < -epsilon or third < -epsilon:
        return None
    return first, second, third


def _rasterize_depth(
    projected: tuple[np.ndarray, np.ndarray],
    depth: np.ndarray,
    winning_triangles: np.ndarray,
    triangle_index: int,
) -> None:
    screen, ndc_depth = projected
    bounds = _raster_bounds(screen, depth.shape[1], depth.shape[0])
    if bounds is None:
        return
    x0, x1, y0, y1 = bounds
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            barycentric = _barycentric(screen, np.asarray((x + 0.5, y + 0.5), dtype=np.float64))
            if barycentric is None:
                continue
            sample_depth = float(np.dot(np.asarray(barycentric, dtype=np.float64), ndc_depth))
            if sample_depth < depth[y, x] - 1e-9:
                depth[y, x] = sample_depth
                winning_triangles[y, x] = triangle_index


def _splat_visible_triangle_to_uv(
    projected: tuple[np.ndarray, np.ndarray],
    uvs: tuple[tuple[float, float], tuple[float, float], tuple[float, float]],
    triangle_index: int,
    source: np.ndarray,
    winning_triangles: np.ndarray,
    sums: np.ndarray,
    counts: np.ndarray,
) -> int:
    screen, _ = projected
    bounds = _raster_bounds(screen, source.shape[1], source.shape[0])
    if bounds is None:
        return 0
    uv_array = np.asarray(uvs, dtype=np.float64)
    if uv_array.shape != (3, 2) or not np.isfinite(uv_array).all():
        raise ReferenceUvEvidenceBakeError("camera UV raster triangle UVs are invalid")
    x0, x1, y0, y1 = bounds
    visible = 0
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            if winning_triangles[y, x] != triangle_index:
                continue
            barycentric = _barycentric(screen, np.asarray((x + 0.5, y + 0.5), dtype=np.float64))
            if barycentric is None:
                continue
            uv = np.dot(np.asarray(barycentric, dtype=np.float64), uv_array)
            if np.any(uv < 0.0) or np.any(uv > 1.0):
                # A wrapped/repeated UV is not a single observed texel.  Leave
                # it unknown until a future explicit wrap policy is approved.
                continue
            tex_x = min(counts.shape[1] - 1, max(0, int(round(float(uv[0]) * (counts.shape[1] - 1)))))
            tex_y = min(counts.shape[0] - 1, max(0, int(round(float(uv[1]) * (counts.shape[0] - 1)))))
            sums[tex_y, tex_x] += source[y, x].astype(np.uint64)
            counts[tex_y, tex_x] += 1
            visible += 1
    return visible


def _stable_id(value: object) -> bool:
    return isinstance(value, str) and 1 <= len(value) <= 128 and all(
        character.isascii() and (character.isalnum() or character in "_-") for character in value
    )


def _sha(value: object) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(character in _HEX for character in value)


def _decode_png_rgb(payload: bytes) -> np.ndarray:
    if not isinstance(payload, bytes) or len(payload) < 33 or not payload.startswith(_PNG_SIGNATURE):
        raise ReferenceUvEvidenceBakeError("only a bounded PNG source is accepted")
    offset = len(_PNG_SIGNATURE)
    width = height = bit_depth = color_type = None
    compressed: list[bytes] = []
    while offset + 12 <= len(payload):
        length = struct.unpack(">I", payload[offset:offset + 4])[0]
        chunk_end = offset + 12 + length
        if length > MAX_SOURCE_PNG_BYTES or chunk_end > len(payload):
            raise ReferenceUvEvidenceBakeError("PNG chunk bounds are invalid")
        kind = payload[offset + 4:offset + 8]
        data = payload[offset + 8:offset + 8 + length]
        crc = struct.unpack(">I", payload[offset + 8 + length:chunk_end])[0]
        if zlib.crc32(kind + data) & 0xFFFFFFFF != crc:
            raise ReferenceUvEvidenceBakeError("PNG chunk checksum is invalid")
        offset = chunk_end
        if kind == b"IHDR":
            if width is not None or length != 13:
                raise ReferenceUvEvidenceBakeError("PNG header is invalid")
            width, height, bit_depth, color_type, compression, filtering, interlace = struct.unpack(">IIBBBBB", data)
            if (
                not (1 <= width <= 4096 and 1 <= height <= 4096)
                or bit_depth != 8
                or color_type not in {2, 6}
                or compression != 0
                or filtering != 0
                or interlace != 0
            ):
                raise ReferenceUvEvidenceBakeError("PNG pixel layout is outside the restricted projection profile")
        elif kind == b"IDAT":
            compressed.append(data)
        elif kind == b"IEND":
            if length != 0 or offset != len(payload):
                raise ReferenceUvEvidenceBakeError("PNG end marker is invalid")
            break
    else:
        raise ReferenceUvEvidenceBakeError("PNG end marker is missing")
    if width is None or height is None or not compressed:
        raise ReferenceUvEvidenceBakeError("PNG image payload is incomplete")
    channels = 4 if color_type == 6 else 3
    try:
        raw = zlib.decompress(b"".join(compressed))
    except zlib.error as exc:
        raise ReferenceUvEvidenceBakeError("PNG image payload cannot be decompressed") from exc
    stride = width * channels
    if len(raw) != (stride + 1) * height:
        raise ReferenceUvEvidenceBakeError("PNG scanline length is invalid")
    decoded = np.empty((height, width, channels), dtype=np.uint8)
    previous = np.zeros(stride, dtype=np.uint8)
    cursor = 0
    for row_index in range(height):
        filter_kind = raw[cursor]
        cursor += 1
        row = np.frombuffer(raw[cursor:cursor + stride], dtype=np.uint8).copy()
        cursor += stride
        if filter_kind == 1:
            for index in range(channels, stride):
                row[index] = (int(row[index]) + int(row[index - channels])) & 0xFF
        elif filter_kind == 2:
            row = (row.astype(np.uint16) + previous.astype(np.uint16)).astype(np.uint8)
        elif filter_kind == 3:
            for index in range(stride):
                left = int(row[index - channels]) if index >= channels else 0
                above = int(previous[index])
                row[index] = (int(row[index]) + ((left + above) // 2)) & 0xFF
        elif filter_kind == 4:
            for index in range(stride):
                left = int(row[index - channels]) if index >= channels else 0
                above = int(previous[index])
                upper_left = int(previous[index - channels]) if index >= channels else 0
                row[index] = (int(row[index]) + _paeth(left, above, upper_left)) & 0xFF
        elif filter_kind != 0:
            raise ReferenceUvEvidenceBakeError("PNG filter is unsupported")
        decoded[row_index] = row.reshape((width, channels))
        previous = row
    if channels == 4:
        alpha = decoded[:, :, 3:4].astype(np.uint16)
        rgb = decoded[:, :, :3].astype(np.uint16)
        # Composite transparent photo pixels over black deterministically; the
        # output GLB base-color channel is RGB and has no user-controlled alpha.
        return ((rgb * alpha + 127) // 255).astype(np.uint8)
    return decoded


def _paeth(left: int, above: int, upper_left: int) -> int:
    estimate = left + above - upper_left
    left_delta = abs(estimate - left)
    above_delta = abs(estimate - above)
    upper_left_delta = abs(estimate - upper_left)
    if left_delta <= above_delta and left_delta <= upper_left_delta:
        return left
    if above_delta <= upper_left_delta:
        return above
    return upper_left


def _resize_bilinear(source: np.ndarray, width: int, height: int) -> np.ndarray:
    source_height, source_width, channels = source.shape
    if channels != 3 or width <= 0 or height <= 0:
        raise ReferenceUvEvidenceBakeError("reference projection resize dimensions are invalid")
    x = (np.arange(width, dtype=np.float64) + 0.5) * source_width / width - 0.5
    y = (np.arange(height, dtype=np.float64) + 0.5) * source_height / height - 0.5
    x0 = np.clip(np.floor(x).astype(np.int64), 0, source_width - 1)
    y0 = np.clip(np.floor(y).astype(np.int64), 0, source_height - 1)
    x1 = np.clip(x0 + 1, 0, source_width - 1)
    y1 = np.clip(y0 + 1, 0, source_height - 1)
    wx = (x - np.floor(x))[:, None]
    wy = (y - np.floor(y))[:, None, None]
    top = source[y0[:, None], x0[None, :], :] * (1.0 - wx) + source[y0[:, None], x1[None, :], :] * wx
    bottom = source[y1[:, None], x0[None, :], :] * (1.0 - wx) + source[y1[:, None], x1[None, :], :] * wx
    return np.rint(top * (1.0 - wy) + bottom * wy).clip(0, 255).astype(np.uint8)


def _encode_png_rgb(rgb: np.ndarray) -> bytes:
    if rgb.ndim != 3 or rgb.shape[2] != 3 or rgb.dtype != np.uint8:
        raise ReferenceUvEvidenceBakeError("projection output must be an RGB image")
    height, width, _ = rgb.shape
    raw = b"".join(b"\x00" + row.tobytes() for row in rgb)

    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    return _PNG_SIGNATURE + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, level=9)) + chunk(b"IEND", b"")
