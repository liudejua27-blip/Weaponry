"""Restricted retained-layer PBR compiler for :class:`SurfaceLayerProgram@1`.

This module is deliberately *not* a second geometry language.  It accepts
only the sealed ``SurfaceLayerLowering@1`` DTO produced by Rust Core and
renders its retained 2D layers into deterministic UV0 PBR textures.  There is
no SVG parser, font file, URL, path, script, shader source, or filesystem
input.  The caller is still responsible for binding the result to the stable
Material Zone selected by Rust.

The module is intentionally standalone while the Rust-to-Python protocol is
being extended.  It proves the exact five-channel bake and provenance rules;
it does not make a retained layer a persisted product mutation by itself.
"""

from __future__ import annotations

import hashlib
import json
import math
import struct
import zlib
from functools import lru_cache
from typing import Mapping, Sequence

from .agent_models import VisualTextureMap, VisualTextureSet
from .visual_texture_sets import (
    GeometryArtifactProfileId,
    PRODUCTION_TEXTURE_HEIGHT,
    PRODUCTION_TEXTURE_WIDTH,
    SUPPORTED_PBR_ROLES,
    TEXTURE_HEIGHT,
    TEXTURE_WIDTH,
    builtin_material_properties,
    builtin_visual_material_binding,
    normalize_surface_adornment_program,
)


_LOWERING_FIELDS = frozenset({
    "schema_version",
    "source_program_sha256",
    "adornments",
    "retained_layers",
    "retained_layers_sha256",
})
_RETAINED_FIELDS = frozenset({
    "vector_paths",
    "decal_layers",
    "roughness_masks",
    "emissive_masks",
    "symmetry",
    "uv_frame",
    "quality_profile",
})
_HEX = frozenset("0123456789abcdef")
_COVERAGES = frozenset({"full_zone", "center_band", "edge_band", "symmetric_pair"})
_SYMMETRIES = frozenset({"none", "mirror_u", "mirror_v", "radial_2", "radial_4"})
_COLOR_TOKENS = {
    "accent_blue": (38, 142, 255),
    "signal_red": (224, 66, 48),
    "graphite": (65, 76, 88),
    "aluminum": (196, 205, 214),
}


def _canonical_json(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def _sha256(value: object) -> str:
    return hashlib.sha256(_canonical_json(value).encode("utf-8")).hexdigest()


def _id(value: object, prefix: str) -> bool:
    if not isinstance(value, str) or not value.startswith(prefix):
        return False
    suffix = value[len(prefix):]
    return bool(suffix) and all(character in "abcdefghijklmnopqrstuvwxyz0123456789_-" for character in suffix)


def _sha(value: object) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(character in _HEX for character in value)


def _uv(value: object) -> tuple[float, float] | None:
    if not isinstance(value, list) or len(value) != 2:
        return None
    if any(type(item) not in {int, float} or not 0 <= float(item) <= 1 for item in value):
        return None
    return float(value[0]), float(value[1])


def _bounded_int(value: object, *, minimum: int, maximum: int) -> bool:
    return type(value) is int and minimum <= value <= maximum


def _normalise_retained_layers(value: object) -> dict[str, object]:
    if not isinstance(value, Mapping) or set(value) != _RETAINED_FIELDS:
        raise ValueError("retained surface layers must contain exactly the reviewed fields")
    vector_paths = value.get("vector_paths")
    decals = value.get("decal_layers")
    roughness = value.get("roughness_masks")
    emissive = value.get("emissive_masks")
    symmetry = value.get("symmetry")
    uv_frame = value.get("uv_frame")
    quality_profile = value.get("quality_profile")
    if not isinstance(vector_paths, list) or len(vector_paths) > 8:
        raise ValueError("retained vector path count is outside the reviewed bound")
    if not isinstance(decals, list) or len(decals) > 4:
        raise ValueError("retained decal count is outside the reviewed bound")
    if not isinstance(roughness, list) or len(roughness) > 2:
        raise ValueError("retained roughness mask count is outside the reviewed bound")
    if not isinstance(emissive, list) or len(emissive) > 2:
        raise ValueError("retained emissive mask count is outside the reviewed bound")
    if quality_profile not in {"interactive_preview", "production_concept"}:
        raise ValueError("retained surface quality profile is invalid")
    if not isinstance(symmetry, Mapping) or set(symmetry) != {"mode", "center_uv"}:
        raise ValueError("retained surface symmetry is invalid")
    if symmetry.get("mode") not in _SYMMETRIES or _uv(symmetry.get("center_uv")) is None:
        raise ValueError("retained surface symmetry is outside the reviewed vocabulary")
    if not isinstance(uv_frame, Mapping) or set(uv_frame) != {
        "frame_id", "u_min", "u_max", "v_min", "v_max", "rotation_degrees"
    }:
        raise ValueError("retained UV frame is invalid")
    numeric_frame = ("u_min", "u_max", "v_min", "v_max", "rotation_degrees")
    if (
        not _id(uv_frame.get("frame_id"), "uvframe_")
        or any(type(uv_frame.get(key)) not in {int, float} for key in numeric_frame)
        or not 0 <= float(uv_frame["u_min"]) < float(uv_frame["u_max"]) <= 1
        or not 0 <= float(uv_frame["v_min"]) < float(uv_frame["v_max"]) <= 1
        or not -180 <= float(uv_frame["rotation_degrees"]) <= 180
    ):
        raise ValueError("retained UV frame is outside the reviewed bound")

    all_ids: set[str] = set()
    total_points = 0
    for path in vector_paths:
        if not isinstance(path, Mapping) or set(path) != {"path_id", "closed", "commands"}:
            raise ValueError("retained vector path is invalid")
        commands = path.get("commands")
        if (
            not _id(path.get("path_id"), "path_")
            or not isinstance(path.get("closed"), bool)
            or not isinstance(commands, list)
            or not 2 <= len(commands) <= 32
            or not commands
            or not isinstance(commands[0], Mapping)
            or commands[0].get("kind") != "move"
            or path["path_id"] in all_ids
        ):
            raise ValueError("retained vector path is outside the reviewed bound")
        all_ids.add(path["path_id"])
        for command in commands:
            if not isinstance(command, Mapping) or set(command) != {"kind", "points"}:
                raise ValueError("retained vector command is invalid")
            expected = {"move": 1, "line": 1, "quadratic": 2, "cubic": 3}.get(command.get("kind"))
            points = command.get("points")
            if expected is None or not isinstance(points, list) or len(points) != expected or any(_uv(point) is None for point in points):
                raise ValueError("retained vector command is outside the reviewed bound")
            total_points += len(points)
    if total_points > 128:
        raise ValueError("retained vector point count exceeds the reviewed bound")

    for decal in decals:
        if not isinstance(decal, Mapping) or set(decal) != {
            "decal_id", "motif", "text_token", "color_token", "anchor_uv", "scale_milli", "opacity_milli"
        }:
            raise ValueError("retained decal is invalid")
        if (
            not _id(decal.get("decal_id"), "decal_")
            or decal["decal_id"] in all_ids
            or decal.get("motif") not in {"chevron_mark", "hex_badge", "warning_stripe", "panel_label"}
            or decal.get("text_token") not in {"none", "A-01", "SERVICE", "CAUTION", "01"}
            or decal.get("color_token") not in _COLOR_TOKENS
            or _uv(decal.get("anchor_uv")) is None
            or not _bounded_int(decal.get("scale_milli"), minimum=50, maximum=500)
            or not _bounded_int(decal.get("opacity_milli"), minimum=0, maximum=1000)
        ):
            raise ValueError("retained decal is outside the reviewed vocabulary")
        all_ids.add(decal["decal_id"])
    for mask in roughness:
        if not isinstance(mask, Mapping) or set(mask) != {"mask_id", "motif", "coverage", "intensity_milli", "seed"}:
            raise ValueError("retained roughness mask is invalid")
        if (
            not _id(mask.get("mask_id"), "rough_")
            or mask["mask_id"] in all_ids
            or mask.get("motif") not in {"linear_brush", "edge_wear", "microgrid"}
            or mask.get("coverage") not in _COVERAGES
            or not _bounded_int(mask.get("intensity_milli"), minimum=0, maximum=1000)
            or not _bounded_int(mask.get("seed"), minimum=0, maximum=2_147_483_647)
        ):
            raise ValueError("retained roughness mask is outside the reviewed vocabulary")
        all_ids.add(mask["mask_id"])
    for mask in emissive:
        if not isinstance(mask, Mapping) or set(mask) != {"mask_id", "motif", "color_token", "coverage", "intensity_milli", "seed"}:
            raise ValueError("retained emissive mask is invalid")
        if (
            not _id(mask.get("mask_id"), "emissive_")
            or mask["mask_id"] in all_ids
            or mask.get("motif") not in {"double_flowline", "dot_array", "panel_indicator"}
            or mask.get("color_token") not in {"accent_blue", "signal_red"}
            or mask.get("coverage") not in _COVERAGES
            or not _bounded_int(mask.get("intensity_milli"), minimum=0, maximum=1000)
            or not _bounded_int(mask.get("seed"), minimum=0, maximum=2_147_483_647)
        ):
            raise ValueError("retained emissive mask is outside the reviewed vocabulary")
        all_ids.add(mask["mask_id"])
    return json.loads(_canonical_json(value))


def normalize_surface_layer_lowering(value: Mapping[str, object]) -> dict[str, object]:
    """Verify the exact data-only Rust lowering at the Python capability edge."""

    if not isinstance(value, Mapping) or set(value) != _LOWERING_FIELDS:
        raise ValueError("surface layer lowering must contain exactly the Rust DTO fields")
    if value.get("schema_version") != "SurfaceLayerLowering@1" or not _sha(value.get("source_program_sha256")):
        raise ValueError("surface layer lowering identity is invalid")
    adornments_value = value.get("adornments")
    if not isinstance(adornments_value, list) or not 1 <= len(adornments_value) <= 4:
        raise ValueError("surface layer lowering must carry one to four reviewed A005 relief layers")
    adornments = [normalize_surface_adornment_program(item) for item in adornments_value]
    first = adornments[0]
    if any(
        item["target_part_id"] != first["target_part_id"]
        or item["target_zone_id"] != first["target_zone_id"]
        or item["base_material"] != first["base_material"]
        or item["kind"] != "normal_relief"
        for item in adornments
    ):
        raise ValueError("surface layer lowering A005 compatibility mapping diverges")
    if len({item["program_id"] for item in adornments}) != len(adornments):
        raise ValueError("surface layer lowering A005 program ids must be unique")
    retained = _normalise_retained_layers(value.get("retained_layers"))
    retained_sha = value.get("retained_layers_sha256")
    if not _sha(retained_sha) or retained_sha != _sha256(retained):
        raise ValueError("surface layer lowering retained hash does not match canonical payload")
    return {
        "schema_version": "SurfaceLayerLowering@1",
        "source_program_sha256": str(value["source_program_sha256"]),
        "adornments": adornments,
        "retained_layers": retained,
        "retained_layers_sha256": str(retained_sha),
    }


def surface_layer_material_id(lowering: Mapping[str, object]) -> str:
    normalized = normalize_surface_layer_lowering(lowering)
    return f"mat_surface_layer_{normalized['source_program_sha256'][:32]}"


def surface_layer_lowering_sha256(lowering: Mapping[str, object]) -> str:
    return _sha256(normalize_surface_layer_lowering(lowering))


def _png_rgb(rows: Sequence[bytes], *, width: int, height: int) -> bytes:
    raw = b"".join(b"\x00" + row for row in rows)

    def chunk(tag: bytes, payload: bytes) -> bytes:
        return struct.pack(">I", len(payload)) + tag + payload + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)

    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, level=9)) + chunk(b"IEND", b"")


def _clamp(value: float) -> int:
    return max(0, min(255, round(value)))


def _coverage(coverage: str, u: float, v: float) -> float:
    if coverage == "full_zone":
        return 1.0
    if coverage == "center_band":
        return max(0.0, min(1.0, 1 - (abs(u - 0.5) - 0.14) / 0.20))
    if coverage == "edge_band":
        return max(0.0, min(1.0, (0.22 - min(u, 1 - u, v, 1 - v)) / 0.14))
    pair = max(0.0, 1 - abs(u - 0.27) / 0.16, 1 - abs(u - 0.73) / 0.16)
    return min(1.0, pair)


def _apply_frame(u: float, v: float, frame: Mapping[str, object]) -> tuple[float, float]:
    x = (u - float(frame["u_min"])) / (float(frame["u_max"]) - float(frame["u_min"]))
    y = (v - float(frame["v_min"])) / (float(frame["v_max"]) - float(frame["v_min"]))
    radians = math.radians(float(frame["rotation_degrees"]))
    dx, dy = x - 0.5, y - 0.5
    return 0.5 + dx * math.cos(radians) - dy * math.sin(radians), 0.5 + dx * math.sin(radians) + dy * math.cos(radians)


def _symmetry_samples(u: float, v: float, symmetry: Mapping[str, object]) -> tuple[tuple[float, float], ...]:
    center = symmetry["center_uv"]
    cx, cy = float(center[0]), float(center[1])
    mode = symmetry["mode"]
    samples = [(u, v)]
    if mode == "mirror_u":
        samples.append((2 * cx - u, v))
    elif mode == "mirror_v":
        samples.append((u, 2 * cy - v))
    elif mode in {"radial_2", "radial_4"}:
        samples.append((2 * cx - u, 2 * cy - v))
        if mode == "radial_4":
            samples.extend(((cx - (v - cy), cy + (u - cx)), (cx + (v - cy), cy - (u - cx))))
    return tuple(samples)


def _bezier(points: Sequence[tuple[float, float]], t: float) -> tuple[float, float]:
    if len(points) == 2:
        return (points[0][0] * (1 - t) + points[1][0] * t, points[0][1] * (1 - t) + points[1][1] * t)
    if len(points) == 3:
        a = _bezier(points[:2], t)
        b = _bezier(points[1:], t)
        return _bezier((a, b), t)
    a = _bezier(points[:3], t)
    b = _bezier(points[1:], t)
    return _bezier((a, b), t)


def _path_segments(paths: Sequence[Mapping[str, object]]) -> tuple[tuple[tuple[float, float], tuple[float, float]], ...]:
    segments: list[tuple[tuple[float, float], tuple[float, float]]] = []
    for path in paths:
        current: tuple[float, float] | None = None
        first: tuple[float, float] | None = None
        for command in path["commands"]:
            points = tuple((float(point[0]), float(point[1])) for point in command["points"])
            if command["kind"] == "move":
                current = points[0]
                first = current
                continue
            if current is None:
                continue
            curve_points = (current,) + points
            previous = current
            for index in range(1, 13):
                next_point = _bezier(curve_points, index / 12)
                segments.append((previous, next_point))
                previous = next_point
            current = points[-1]
        if path["closed"] and current is not None and first is not None:
            segments.append((current, first))
    return tuple(segments)


def _segment_distance(u: float, v: float, segment: tuple[tuple[float, float], tuple[float, float]]) -> float:
    (ax, ay), (bx, by) = segment
    dx, dy = bx - ax, by - ay
    length_squared = dx * dx + dy * dy
    if length_squared <= 1e-12:
        return math.hypot(u - ax, v - ay)
    t = max(0.0, min(1.0, ((u - ax) * dx + (v - ay) * dy) / length_squared))
    return math.hypot(u - (ax + t * dx), v - (ay + t * dy))


def _vector_mask(u: float, v: float, segments: Sequence[tuple[tuple[float, float], tuple[float, float]]]) -> float:
    if not segments:
        return 0.0
    distance = min(_segment_distance(u, v, segment) for segment in segments)
    return max(0.0, min(1.0, 1 - distance / 0.018))


def _decal_mask(decal: Mapping[str, object], u: float, v: float) -> float:
    anchor = decal["anchor_uv"]
    scale = float(decal["scale_milli"]) / 1000.0
    x = (u - float(anchor[0])) / scale
    y = (v - float(anchor[1])) / scale
    motif = decal["motif"]
    if motif == "chevron_mark":
        present = abs(abs(x) - (0.28 + abs(y) * 0.55)) < 0.09 and abs(y) < 0.42
    elif motif == "hex_badge":
        present = max(abs(x), abs(y) * 1.15 + abs(x) * 0.5) < 0.42
    elif motif == "warning_stripe":
        present = abs(x) < 0.46 and abs(y) < 0.25 and int((x - y) * 11) % 2 == 0
    else:  # reviewed panel_label token, not arbitrary text rendering
        token_hash = sum(ord(character) for character in str(decal["text_token"]))
        present = abs(x) < 0.46 and abs(y) < 0.22 and (int((x + 0.5) * 9) + token_hash) % 3 != 0
    return float(present) * float(decal["opacity_milli"]) / 1000.0


def _relief_height(adornments: Sequence[Mapping[str, object]], u: float, v: float) -> float:
    total = 0.0
    for adornment in adornments:
        phase = (int(adornment["seed"]) % 4096) / 4096 * math.tau
        motif = adornment["motif"]
        if motif == "parallel_groove":
            wave = math.sin(math.tau * (14 * u + 0.9 * v) + phase)
        else:
            wave = math.sin(math.tau * (11 * v + 5 * abs((u - 0.5) * 2)) + phase)
        scale = {"subtle": 1.6, "balanced": 3.2, "pronounced": 5.4}[str(adornment["intensity"])]
        total += wave * scale * _coverage(str(adornment["coverage"]), u, v)
    return total


def _roughness_mask(mask: Mapping[str, object], u: float, v: float) -> float:
    phase = (int(mask["seed"]) % 4096) / 4096 * math.tau
    if mask["motif"] == "linear_brush":
        pattern = (math.sin(math.tau * (34 * v + 1.2 * u) + phase) + 1) / 2
    elif mask["motif"] == "edge_wear":
        pattern = 1 - min(u, 1 - u, v, 1 - v) / 0.16
    else:
        pattern = (math.sin(math.tau * 16 * u + phase) * math.sin(math.tau * 16 * v + phase) + 1) / 2
    return max(0.0, min(1.0, pattern)) * _coverage(str(mask["coverage"]), u, v) * float(mask["intensity_milli"]) / 1000


def _emissive_mask(mask: Mapping[str, object], u: float, v: float) -> float:
    phase = (int(mask["seed"]) % 4096) / 4096 * math.tau
    if mask["motif"] == "double_flowline":
        a = abs(v - (0.32 + 0.10 * math.sin(math.tau * u + phase)))
        b = abs(v - (0.68 - 0.10 * math.sin(math.tau * u + phase)))
        pattern = max(0.0, 1 - min(a, b) / 0.028)
    elif mask["motif"] == "dot_array":
        pattern = max(0.0, math.cos(math.tau * 9 * u) * math.cos(math.tau * 6 * v))
    else:
        pattern = float(abs(u - 0.5) < 0.35 and abs(v - 0.5) < 0.10)
    return pattern * _coverage(str(mask["coverage"]), u, v) * float(mask["intensity_milli"]) / 1000


def _render_bytes(lowering: Mapping[str, object], *, artifact_profile_id: GeometryArtifactProfileId) -> Mapping[str, bytes]:
    normalized = normalize_surface_layer_lowering(lowering)
    retained = normalized["retained_layers"]
    adornments = normalized["adornments"]
    base_index, _ = builtin_visual_material_binding(str(adornments[0]["base_material"]))
    material = builtin_material_properties(base_index)
    width = PRODUCTION_TEXTURE_WIDTH if artifact_profile_id == "production_concept" else TEXTURE_WIDTH
    height = PRODUCTION_TEXTURE_HEIGHT if artifact_profile_id == "production_concept" else TEXTURE_HEIGHT
    paths = _path_segments(retained["vector_paths"])
    rows = {role: [] for role in SUPPORTED_PBR_ROLES}
    base = tuple(int(value) for value in material["base"])
    base_roughness, base_metallic = int(material["roughness"]), int(material["metallic"])
    for y in range(height):
        encoded = {role: bytearray() for role in SUPPORTED_PBR_ROLES}
        for x in range(width):
            raw_u, raw_v = x / width, y / height
            u, v = _apply_frame(raw_u, raw_v, retained["uv_frame"])
            samples = _symmetry_samples(u, v, retained["symmetry"])
            vector = max(_vector_mask(sample_u, sample_v, paths) for sample_u, sample_v in samples)
            relief = sum(_relief_height(adornments, sample_u, sample_v) for sample_u, sample_v in samples) / len(samples)
            decals = [(decal, max(_decal_mask(decal, sample_u, sample_v) for sample_u, sample_v in samples)) for decal in retained["decal_layers"]]
            roughness = sum(_roughness_mask(mask, u, v) for mask in retained["roughness_masks"])
            emissions = [(mask, _emissive_mask(mask, u, v)) for mask in retained["emissive_masks"]]
            accent = [0.0, 0.0, 0.0]
            for decal, amount in decals:
                color = _COLOR_TOKENS[str(decal["color_token"])]
                for channel in range(3):
                    accent[channel] += (color[channel] - base[channel]) * amount
            colour_shift = relief * 0.42 + vector * 18
            encoded["base_color"].extend(_clamp(base[channel] + accent[channel] + colour_shift) for channel in range(3))
            encoded["metallic_roughness"].extend((255, _clamp(base_roughness - roughness * 65 + relief * 1.5), _clamp(base_metallic + vector * 5)))
            # A scalar field yields a valid tangent-space normal without arbitrary shaders.
            # The scalar field is expressed in normalized UV space, so its
            # derivative is independent of output resolution.  Do not scale
            # it down by a texel step: doing so makes the production map look
            # flat even though its 128 px preview has readable relief.
            encoded["normal"].extend((_clamp(128 - math.cos(math.tau * 14 * u) * relief * 3.2), _clamp(128 + math.cos(math.tau * 11 * v) * relief * 2.6), 254))
            occlusion = _clamp(252 - max(0, -relief) * 1.2 - vector * 20)
            encoded["occlusion"].extend((occlusion, occlusion, occlusion))
            emission = [0.0, 0.0, 0.0]
            for mask, amount in emissions:
                color = _COLOR_TOKENS[str(mask["color_token"])]
                for channel in range(3):
                    emission[channel] += color[channel] * amount
            encoded["emissive"].extend(_clamp(channel) for channel in emission)
        for role in SUPPORTED_PBR_ROLES:
            rows[role].append(bytes(encoded[role]))
    return {role: _png_rgb(rows[role], width=width, height=height) for role in SUPPORTED_PBR_ROLES}


@lru_cache(maxsize=16)
def _cached_bytes(artifact_profile_id: GeometryArtifactProfileId, canonical: str) -> Mapping[str, bytes]:
    return _render_bytes(json.loads(canonical), artifact_profile_id=artifact_profile_id)


def surface_layer_visual_texture_set(lowering: Mapping[str, object], *, artifact_profile_id: GeometryArtifactProfileId) -> VisualTextureSet:
    normalized = normalize_surface_layer_lowering(lowering)
    canonical = _canonical_json(normalized)
    payloads = _cached_bytes(artifact_profile_id, canonical)
    suffix = normalized["source_program_sha256"][:32]
    version = "sl1p" if artifact_profile_id == "interactive_preview" else "sl1d"
    width = TEXTURE_WIDTH if artifact_profile_id == "interactive_preview" else PRODUCTION_TEXTURE_WIDTH
    height = TEXTURE_HEIGHT if artifact_profile_id == "interactive_preview" else PRODUCTION_TEXTURE_HEIGHT
    material_id = surface_layer_material_id(normalized)
    return VisualTextureSet(
        visual_texture_set_id=f"vtexset_surface_layer_{suffix}_{version}",
        material_id=material_id,
        display_name="SurfaceLayer retained PBR",
        maps=[
            VisualTextureMap(
                texture_id=f"vtex_surface_layer_{suffix}_{version}_{role}",
                texture_role=role,
                mime_type="image/png",
                byte_size=len(payloads[role]),
                sha256=hashlib.sha256(payloads[role]).hexdigest(),
                color_space="srgb" if role in {"base_color", "emissive"} else "linear",
                width=width,
                height=height,
                source="forgecad_builtin",
                license="not_applicable",
                fallback="none",
            )
            for role in SUPPORTED_PBR_ROLES
        ],
        source="forgecad_builtin",
        license="not_applicable",
        version="4",
    )


def surface_layer_visual_texture_png_bytes(lowering: Mapping[str, object], *, artifact_profile_id: GeometryArtifactProfileId, texture_role: str) -> bytes:
    if texture_role not in SUPPORTED_PBR_ROLES:
        raise ValueError("surface layer PBR texture role is invalid")
    normalized = normalize_surface_layer_lowering(lowering)
    return _cached_bytes(artifact_profile_id, _canonical_json(normalized))[texture_role]


def surface_layer_texture_cache_facts() -> Mapping[str, int]:
    info = _cached_bytes.cache_info()
    return {"entry_count": info.currsize, "max_entries": int(info.maxsize or 0)}
