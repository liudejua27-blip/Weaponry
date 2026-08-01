from __future__ import annotations

import base64
import hashlib

import numpy as np
import pytest

from forgecad_agent.application.reference_uv_projection import (
    ALGORITHM_ID,
    ALGORITHM_VERSION,
    CAMERA_RASTER_ALGORITHM_ID,
    CameraUvRasterTriangle,
    ReferenceCameraUvRasterBake,
    ReferenceUvEvidenceBakeError,
    _decode_png_rgb,
    _encode_png_rgb,
    bake_reference_camera_uv_raster,
    bake_reference_uv_evidence,
    fuse_reference_camera_uv_raster,
)


def _projection(source_png: bytes, *, rect: list[int] | None = None) -> dict[str, object]:
    return {
        "schema_version": "ReferenceUvEvidenceBake@1",
        "projection_id": "projection_reference_front",
        "source_evidence_id": "evidence_reference_front",
        "source_image_sha256": hashlib.sha256(source_png).hexdigest(),
        "source_png_base64": base64.b64encode(source_png).decode("ascii"),
        "camera_hypothesis_id": "camera_reference_front",
        "camera_provenance_sha256": "a" * 64,
        "target_material_zone_id": "zone_shell",
        "texture_width": 128,
        "texture_height": 128,
        "observed_uv_rect_bps": rect or [2500, 2500, 7500, 7500],
    }


def _camera_projection(source_png: bytes) -> dict[str, object]:
    return {
        "schema_version": "ReferenceCameraUvRasterBake@2",
        "projection_id": "projection_reference_front",
        "source_evidence_id": "evidence_reference_front",
        "source_image_sha256": hashlib.sha256(source_png).hexdigest(),
        "source_png_base64": base64.b64encode(source_png).decode("ascii"),
        "camera_hypothesis_id": "camera_reference_front",
        "camera_provenance_sha256": "a" * 64,
        "target_material_zone_id": "zone_shell",
        "texture_width": 128,
        "texture_height": 128,
        # Explicit row-major identity world-to-clip for the deterministic test scene.
        "world_to_clip_row_major": [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ],
    }


def _square_triangles(zone_id: str, *, z: float) -> list[CameraUvRasterTriangle]:
    return [
        CameraUvRasterTriangle(
            material_zone_id=zone_id,
            positions=((-0.75, -0.75, z), (0.75, -0.75, z), (0.75, 0.75, z)),
            uvs=((0.0, 1.0), (1.0, 1.0), (1.0, 0.0)),
        ),
        CameraUvRasterTriangle(
            material_zone_id=zone_id,
            positions=((-0.75, -0.75, z), (0.75, 0.75, z), (-0.75, 0.75, z)),
            uvs=((0.0, 1.0), (1.0, 0.0), (0.0, 0.0)),
        ),
    ]


def test_reference_uv_evidence_overlays_only_declared_texels_and_records_inverse_mask() -> None:
    source = np.zeros((2, 2, 3), dtype=np.uint8)
    source[0, :, :] = (240, 30, 20)
    source[1, :, :] = (30, 90, 240)
    base = np.full((128, 128, 3), (17, 21, 27), dtype=np.uint8)
    source_png = _encode_png_rgb(source)

    result = bake_reference_uv_evidence(_encode_png_rgb(base), _projection(source_png))

    baked = _decode_png_rgb(result.base_color_png)
    mask = _decode_png_rgb(result.unobserved_texel_mask_png)
    assert tuple(baked[10, 10]) == (17, 21, 27)
    assert tuple(baked[64, 64]) != (17, 21, 27)
    assert tuple(mask[10, 10]) == (255, 255, 255)
    assert tuple(mask[64, 64]) == (0, 0, 0)
    assert result.observed_texel_count == 64 * 64
    assert result.unobserved_texel_count == 128 * 128 - 64 * 64
    assert result.base_color_sha256 == hashlib.sha256(result.base_color_png).hexdigest()
    assert result.unobserved_texel_mask_sha256 == hashlib.sha256(result.unobserved_texel_mask_png).hexdigest()
    assert ALGORITHM_ID == "forgecad.reference_uv_evidence_bake"
    assert ALGORITHM_VERSION == "1"


def test_reference_uv_evidence_rejects_unsealed_or_mismatched_source_payload() -> None:
    source_png = _encode_png_rgb(np.zeros((2, 2, 3), dtype=np.uint8))
    base_png = _encode_png_rgb(np.zeros((128, 128, 3), dtype=np.uint8))
    invalid = _projection(source_png)
    invalid["source_image_sha256"] = "b" * 64
    with pytest.raises(ReferenceUvEvidenceBakeError, match="hash"):
        bake_reference_uv_evidence(base_png, invalid)

    invalid = _projection(source_png)
    invalid["observed_uv_rect_bps"] = [0, 0, 0, 10_000]
    with pytest.raises(ReferenceUvEvidenceBakeError, match="rectangle"):
        bake_reference_uv_evidence(base_png, invalid)


def test_reference_uv_evidence_rejects_pbr_dimension_drift() -> None:
    source_png = _encode_png_rgb(np.zeros((2, 2, 3), dtype=np.uint8))
    wrong_base = _encode_png_rgb(np.zeros((64, 64, 3), dtype=np.uint8))
    with pytest.raises(ReferenceUvEvidenceBakeError, match="dimensions"):
        bake_reference_uv_evidence(wrong_base, _projection(source_png))


def test_camera_uv_raster_writes_only_visible_target_uv_texels() -> None:
    source = np.zeros((8, 8, 3), dtype=np.uint8)
    for y in range(8):
        for x in range(8):
            source[y, x] = (20 + x * 20, 30 + y * 20, 90)
    source_png = _encode_png_rgb(source)
    base = np.full((128, 128, 3), (7, 11, 13), dtype=np.uint8)

    result = bake_reference_camera_uv_raster(
        _encode_png_rgb(base),
        _camera_projection(source_png),
        _square_triangles("zone_shell", z=0.0),
    )

    baked = _decode_png_rgb(result.base_color_png)
    mask = _decode_png_rgb(result.unobserved_texel_mask_png)
    assert 0 < result.observed_texel_count < 128 * 128
    assert result.visible_target_source_pixel_count > 0
    assert result.raster_source_width == 8
    assert result.raster_source_height == 8
    assert np.any(np.any(baked != (7, 11, 13), axis=2))
    assert np.any(np.all(mask == (0, 0, 0), axis=2))
    assert np.any(np.all(mask == (255, 255, 255), axis=2))
    assert result.projection_sha256 != hashlib.sha256(source_png).hexdigest()
    assert CAMERA_RASTER_ALGORITHM_ID == "forgecad.reference_camera_uv_raster"


def test_camera_uv_raster_depth_rejects_target_pixels_hidden_by_other_zone() -> None:
    source_png = _encode_png_rgb(np.full((8, 8, 3), (230, 40, 30), dtype=np.uint8))
    base_png = _encode_png_rgb(np.full((128, 128, 3), (7, 11, 13), dtype=np.uint8))
    triangles = _square_triangles("zone_shell", z=0.5) + _square_triangles("zone_occluder", z=0.0)

    result = bake_reference_camera_uv_raster(base_png, _camera_projection(source_png), triangles)

    assert result.observed_texel_count == 0
    assert result.visible_target_source_pixel_count == 0
    assert np.all(_decode_png_rgb(result.base_color_png) == (7, 11, 13))
    assert np.all(_decode_png_rgb(result.unobserved_texel_mask_png) == (255, 255, 255))


def test_camera_uv_raster_rejects_degenerate_camera_matrix() -> None:
    source_png = _encode_png_rgb(np.zeros((8, 8, 3), dtype=np.uint8))
    projection = _camera_projection(source_png)
    projection["world_to_clip_row_major"] = [0.0] * 16
    with pytest.raises(ReferenceUvEvidenceBakeError, match="degenerate"):
        bake_reference_camera_uv_raster(
            _encode_png_rgb(np.zeros((128, 128, 3), dtype=np.uint8)),
            projection,
            _square_triangles("zone_shell", z=0.0),
        )


def test_camera_uv_raster_fusion_is_bounded_deterministic_and_averages_observed_texels() -> None:
    first_source = np.full((8, 8, 3), (220, 30, 20), dtype=np.uint8)
    second_source = np.full((8, 8, 3), (20, 40, 230), dtype=np.uint8)
    first = ReferenceCameraUvRasterBake.from_value(
        _camera_projection(_encode_png_rgb(first_source))
    )
    second_value = _camera_projection(_encode_png_rgb(second_source))
    second_value.update(
        projection_id="projection_reference_side",
        source_evidence_id="evidence_reference_side",
        camera_hypothesis_id="camera_reference_side",
        camera_provenance_sha256="b" * 64,
    )
    second = ReferenceCameraUvRasterBake.from_value(second_value)
    base_png = _encode_png_rgb(np.full((128, 128, 3), (7, 11, 13), dtype=np.uint8))
    triangles = _square_triangles("zone_shell", z=0.0)

    result = fuse_reference_camera_uv_raster(base_png, (first, second), triangles)
    reversed_result = fuse_reference_camera_uv_raster(base_png, (second, first), triangles)

    assert result.projection_sha256 == reversed_result.projection_sha256
    assert result.base_color_png == reversed_result.base_color_png
    assert result.unobserved_texel_mask_png == reversed_result.unobserved_texel_mask_png
    assert 0 < result.observed_texel_count < 128 * 128
    assert result.observed_texel_count + result.unobserved_texel_count == 128 * 128
    fused = _decode_png_rgb(result.base_color_png)
    observed = _decode_png_rgb(result.unobserved_texel_mask_png)[:, :, 0] == 0
    assert np.all(fused[observed] == (120, 35, 125))
    assert tuple(fused[0, 0]) == (7, 11, 13)

    with pytest.raises(ReferenceUvEvidenceBakeError, match="exactly two"):
        fuse_reference_camera_uv_raster(base_png, (first,), triangles)
