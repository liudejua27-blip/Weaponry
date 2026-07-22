"""Deterministic built-in visual PBR texture sets for ShapeProgram GLBs.

The textures are deliberately generated in-process from small reviewed colour
palettes.  They are visual-only, self-contained PNGs: no network request,
filesystem path, user prompt, or hidden third-party asset participates in the
GLB result.  Their metadata is the same object consumed by GLB write and
readback validation.
"""

from __future__ import annotations

import hashlib
import json
import math
import struct
import zlib
from functools import lru_cache
from typing import Iterable, Literal, Mapping

import numpy as np

from .agent_models import VisualTextureMap, VisualTextureSet


TEXTURE_WIDTH = 128
TEXTURE_HEIGHT = 128
PRODUCTION_TEXTURE_WIDTH = 1024
PRODUCTION_TEXTURE_HEIGHT = 1024
GeometryArtifactProfileId = Literal["interactive_preview", "production_concept"]
SurfaceAdornmentKind = Literal[
    "normal_relief",
    "pattern",
    "flowline",
    "micro_surface",
]
SurfaceAdornmentMotif = Literal[
    "parallel_groove",
    "chevron_relief",
    "double_flowline",
    "hex_microgrid",
]
SurfaceAdornmentIntensity = Literal["subtle", "balanced", "pronounced"]
SurfaceAdornmentCoverage = Literal[
    "full_zone",
    "center_band",
    "edge_band",
    "symmetric_pair",
]
SUPPORTED_PBR_ROLES = (
    "base_color",
    "metallic_roughness",
    "normal",
    "occlusion",
    "emissive",
)


def geometry_artifact_profile_manifest(
    artifact_profile_id: GeometryArtifactProfileId,
) -> dict[str, object]:
    """Return the immutable compile/delivery contract for one derived GLB.

    The profile is not an asset version or user-editable geometry setting. It
    is a code-owned recipe for rebuilding the same ShapeProgram as either a
    responsive workbench preview or an on-demand production concept artifact.
    """

    if artifact_profile_id == "interactive_preview":
        payload: dict[str, object] = {
            "schema_version": "GeometryArtifactProfile@1",
            "artifact_profile_id": artifact_profile_id,
            "radial_segments": 24,
            "capsule_hemisphere_segments": 5,
            "smooth_loft_normals": False,
            "texture_width": TEXTURE_WIDTH,
            "texture_height": TEXTURE_HEIGHT,
            "texture_mime_type": "image/png",
            "texture_compression": "png_deflate",
            "delivery": "interactive",
            "triangle_budget_multiplier": 1,
            "max_triangle_count": 100_000,
        }
    elif artifact_profile_id == "production_concept":
        payload = {
            "schema_version": "GeometryArtifactProfile@1",
            "artifact_profile_id": artifact_profile_id,
            "radial_segments": 64,
            "capsule_hemisphere_segments": 14,
            "smooth_loft_normals": True,
            "texture_width": PRODUCTION_TEXTURE_WIDTH,
            "texture_height": PRODUCTION_TEXTURE_HEIGHT,
            "texture_mime_type": "image/png",
            "texture_compression": "png_deflate",
            "delivery": "on_demand",
            "triangle_budget_multiplier": 6,
            "max_triangle_count": 250_000,
        }
    else:
        raise ValueError(f"unsupported geometry artifact profile: {artifact_profile_id}")
    payload["profile_sha256"] = hashlib.sha256(
        json.dumps(
            payload,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
    ).hexdigest()
    return payload


_MATERIALS: tuple[Mapping[str, object], ...] = (
    {"material_id": "mat_primary", "name": "深石墨金属外观", "pattern": "machined", "base": (50, 58, 68), "metallic": 150, "roughness": 105, "normal_scale": 0.42, "clearcoat": 0.0, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_aluminum", "name": "拉丝金属外观", "pattern": "brushed", "base": (145, 154, 164), "metallic": 240, "roughness": 72, "normal_scale": 0.62, "clearcoat": 0.0, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_signal_red", "name": "信号红涂层外观", "pattern": "coated", "base": (196, 55, 43), "metallic": 72, "roughness": 88, "normal_scale": 0.25, "clearcoat": 0.34, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_composite", "name": "哑光复合外观", "pattern": "composite", "base": (35, 42, 50), "metallic": 35, "roughness": 145, "normal_scale": 0.5, "clearcoat": 0.0, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_rubber", "name": "橡胶外观", "pattern": "rubber", "base": (18, 22, 28), "metallic": 0, "roughness": 226, "normal_scale": 0.72, "clearcoat": 0.0, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_dark_glass", "name": "深色透明外观", "pattern": "glass", "base": (45, 70, 94), "metallic": 8, "roughness": 48, "normal_scale": 0.12, "clearcoat": 0.18, "transmission": 0.54, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_emissive_blue", "name": "蓝色发光饰条外观", "pattern": "emissive", "base": (18, 55, 120), "metallic": 38, "roughness": 68, "normal_scale": 0.22, "clearcoat": 0.0, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (12, 112, 255)},
    {"material_id": "mat_automotive_paint", "name": "蓝色汽车漆外观", "pattern": "coated", "base": (22, 75, 160), "metallic": 140, "roughness": 45, "normal_scale": 0.22, "clearcoat": 0.9, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
)


# ShapeProgram and the visual material catalog keep the authored appearance id
# in immutable zone facts.  The fixed eight-entry GLB material table is a
# separate canonical texture identity.  Keep this bridge explicit and
# exhaustive: an unknown authored id must never silently fall back to index 0.
_AUTHORED_TO_TEXTURE_MATERIAL: Mapping[str, str] = {
    "mat_primary": "mat_primary",
    "mat_graphite": "mat_primary",
    "mat_painted_steel": "mat_primary",
    "mat_powder_coat": "mat_primary",
    "mat_aluminum": "mat_aluminum",
    "mat_signal_red": "mat_signal_red",
    "mat_composite": "mat_composite",
    "mat_abs_matte": "mat_composite",
    "mat_carbon_composite": "mat_composite",
    "mat_rubber": "mat_rubber",
    "mat_rubber_tire": "mat_rubber",
    "mat_dark_glass": "mat_dark_glass",
    "mat_clear_glass": "mat_dark_glass",
    "mat_emissive_blue": "mat_emissive_blue",
    "mat_automotive_paint": "mat_automotive_paint",
}


_SURFACE_ADORNMENT_FIELDS = frozenset({
    "schema_version",
    "program_id",
    "target_part_id",
    "target_zone_id",
    "kind",
    "motif",
    "intensity",
    "coverage",
    "seed",
    "base_material",
    "execution",
    "skill_id",
    "skill_version",
    "skill_sha256",
    "generator",
    "non_functional_only",
})
_SURFACE_ADORNMENT_KINDS = frozenset({
    "normal_relief", "pattern", "flowline", "micro_surface",
})
_SURFACE_ADORNMENT_MOTIFS = frozenset({
    "parallel_groove", "chevron_relief", "double_flowline", "hex_microgrid",
})
_SURFACE_ADORNMENT_INTENSITIES = frozenset({"subtle", "balanced", "pronounced"})
_SURFACE_ADORNMENT_COVERAGES = frozenset({
    "full_zone", "center_band", "edge_band", "symmetric_pair",
})


def _is_safe_surface_adornment_identifier(value: object, prefix: str) -> bool:
    if not isinstance(value, str) or not value.startswith(prefix):
        return False
    suffix = value[len(prefix):]
    return bool(suffix) and all(
        character in "abcdefghijklmnopqrstuvwxyz0123456789_-"
        for character in suffix
    )


def normalize_surface_adornment_program(program: Mapping[str, object]) -> dict[str, object]:
    """Validate and canonically normalize the A005 texture-bake input.

    This is intentionally a small, closed vocabulary.  The program is visual
    provenance, never a ShapeProgram operation, a path, an URL, or executable
    source.  Product-state/selection validation remains Rust-owned; this
    restricted geometry boundary only accepts an already selected part/zone
    identity and proves the exact visual bytes produced from it.
    """

    if not isinstance(program, Mapping) or set(program) != _SURFACE_ADORNMENT_FIELDS:
        raise ValueError("surface adornment program must contain exactly the A005 schema fields")
    if program.get("schema_version") != "SurfaceAdornmentProgram@1":
        raise ValueError("surface adornment program schema version is invalid")
    if not _is_safe_surface_adornment_identifier(program.get("program_id"), "adorn_"):
        raise ValueError("surface adornment program id is invalid")
    if not _is_safe_surface_adornment_identifier(program.get("target_part_id"), "part_"):
        raise ValueError("surface adornment target part id is invalid")
    if not _is_safe_surface_adornment_identifier(program.get("target_zone_id"), "zone_"):
        raise ValueError("surface adornment target zone id is invalid")
    if program.get("kind") not in _SURFACE_ADORNMENT_KINDS:
        raise ValueError("surface adornment kind is invalid")
    if program.get("motif") not in _SURFACE_ADORNMENT_MOTIFS:
        raise ValueError("surface adornment motif is invalid")
    if program.get("intensity") not in _SURFACE_ADORNMENT_INTENSITIES:
        raise ValueError("surface adornment intensity is invalid")
    if program.get("coverage") not in _SURFACE_ADORNMENT_COVERAGES:
        raise ValueError("surface adornment coverage is invalid")
    seed = program.get("seed")
    if type(seed) is not int or not 0 <= seed <= 2_147_483_647:
        raise ValueError("surface adornment seed is outside the reviewed bound")
    base_material = program.get("base_material")
    if not isinstance(base_material, str) or base_material not in _AUTHORED_TO_TEXTURE_MATERIAL:
        raise ValueError("surface adornment base material is outside the reviewed visual catalog")
    if program.get("execution") != "texture_bake":
        raise ValueError("surface adornment execution is invalid")
    if not _is_safe_surface_adornment_identifier(program.get("skill_id"), "skill_"):
        raise ValueError("surface adornment skill id is invalid")
    skill_version = program.get("skill_version")
    if type(skill_version) is not int or skill_version < 1:
        raise ValueError("surface adornment skill version is invalid")
    skill_sha256 = program.get("skill_sha256")
    if not isinstance(skill_sha256, str) or len(skill_sha256) != 64 or any(
        character not in "0123456789abcdef" for character in skill_sha256
    ):
        raise ValueError("surface adornment skill hash is invalid")
    if program.get("generator") != "a005_v1" or program.get("non_functional_only") is not True:
        raise ValueError("surface adornment must use the reviewed visual-only generator")
    return {key: program[key] for key in sorted(_SURFACE_ADORNMENT_FIELDS)}


def surface_adornment_program_sha256(program: Mapping[str, object]) -> str:
    normalized = normalize_surface_adornment_program(program)
    return hashlib.sha256(json_canonical(normalized).encode("utf-8")).hexdigest()


def surface_adornment_material_id(program: Mapping[str, object]) -> str:
    """Return a stable dynamic material identity without growing the builtin table."""

    return f"mat_a005_{surface_adornment_program_sha256(program)[:32]}"


def _surface_adornment_texture_set_id(
    program: Mapping[str, object],
    artifact_profile_id: GeometryArtifactProfileId,
) -> str:
    profile_version = "v4" if artifact_profile_id == "production_concept" else "v3"
    return f"vtexset_a005_{surface_adornment_program_sha256(program)[:32]}_{profile_version}"


def _clamp(value: int) -> int:
    return max(0, min(255, value))


def _noise(x: int, y: int, seed: int) -> int:
    value = (x * 374761393 + y * 668265263 + seed * 2246822519) & 0xFFFFFFFF
    value = ((value ^ (value >> 13)) * 1274126177) & 0xFFFFFFFF
    return int((value ^ (value >> 16)) % 17) - 8


def _smooth_periodic_noise(x: int, y: int, seed: int, cell_size: int) -> int:
    """Return deterministic value noise without visible integer-cell steps.

    The reviewed cell sizes divide the fixed 128 px texture extent, so the
    bilinear field also tiles continuously at each PNG edge.
    """

    if cell_size <= 0 or TEXTURE_WIDTH % cell_size or TEXTURE_HEIGHT % cell_size:
        raise ValueError("smooth visual noise cell size must divide the texture extent")
    cells_x = TEXTURE_WIDTH // cell_size
    cells_y = TEXTURE_HEIGHT // cell_size
    grid_x = x // cell_size
    grid_y = y // cell_size
    local_x = (x % cell_size) / cell_size
    local_y = (y % cell_size) / cell_size
    smooth_x = local_x * local_x * (3.0 - 2.0 * local_x)
    smooth_y = local_y * local_y * (3.0 - 2.0 * local_y)

    def sample(offset_x: int, offset_y: int) -> int:
        return _noise(
            (grid_x + offset_x) % cells_x,
            (grid_y + offset_y) % cells_y,
            seed,
        )

    top = sample(0, 0) * (1.0 - smooth_x) + sample(1, 0) * smooth_x
    bottom = sample(0, 1) * (1.0 - smooth_x) + sample(1, 1) * smooth_x
    return round(top * (1.0 - smooth_y) + bottom * smooth_y)


def _smooth_periodic_noise_extent(
    x: int,
    y: int,
    seed: int,
    cell_size: int,
    *,
    width: int,
    height: int,
) -> int:
    if cell_size <= 0 or width % cell_size or height % cell_size:
        raise ValueError("smooth visual noise cell size must divide the texture extent")
    cells_x = width // cell_size
    cells_y = height // cell_size
    grid_x = x // cell_size
    grid_y = y // cell_size
    local_x = (x % cell_size) / cell_size
    local_y = (y % cell_size) / cell_size
    smooth_x = local_x * local_x * (3.0 - 2.0 * local_x)
    smooth_y = local_y * local_y * (3.0 - 2.0 * local_y)

    def sample(offset_x: int, offset_y: int) -> int:
        return _noise(
            (grid_x + offset_x) % cells_x,
            (grid_y + offset_y) % cells_y,
            seed,
        )

    top = sample(0, 0) * (1.0 - smooth_x) + sample(1, 0) * smooth_x
    bottom = sample(0, 1) * (1.0 - smooth_x) + sample(1, 1) * smooth_x
    return round(top * (1.0 - smooth_y) + bottom * smooth_y)


def _v2_surface_height(pattern: str, x: int, y: int, seed: int) -> int:
    """Reproduce the immutable builtin v2 texture bytes for readback."""

    micro = _noise(x, y, seed)
    broad = _smooth_periodic_noise(x, y, seed + 19, 16)
    if pattern == "brushed":
        return _smooth_periodic_noise(0, y, seed + 31, 8) // 2 + micro // 4
    if pattern == "composite":
        # A smooth, tileable weave avoids the old 18 px hard-edged checker.
        warp = round(2.5 * math.sin(2.0 * math.pi * x / 16.0))
        weft = round(2.5 * math.sin(2.0 * math.pi * y / 16.0))
        return warp + weft + micro // 5
    if pattern == "rubber":
        return micro // 2 + broad // 4
    if pattern == "coated":
        return broad // 3 + micro // 6
    if pattern == "glass":
        return broad // 5
    if pattern == "emissive":
        return 2 + broad // 4
    return micro // 3 + round(1.5 * math.sin(2.0 * math.pi * x / 32.0))


def _v3_surface_height(pattern: str, x: int, y: int, seed: int) -> int:
    """Return bounded multi-scale detail without broad horizontal banding.

    Frequencies use integer cycles over the fixed 128 px extent, so every
    directional field remains tileable at both PNG edges.  The amplitude stays
    intentionally small: the maps provide readable micro-surface response
    while the real G826 geometry and Material Zone facts remain authoritative
    for silhouettes, seams, and part boundaries.
    """

    fine = _smooth_periodic_noise(x, y, seed + 7, 4)
    medium = _smooth_periodic_noise(x, y, seed + 19, 8)
    broad = _smooth_periodic_noise(x, y, seed + 37, 16)
    phase_x = 2.0 * math.pi * x / TEXTURE_WIDTH
    phase_y = 2.0 * math.pi * y / TEXTURE_HEIGHT
    if pattern == "brushed":
        # Sub-pixel-to-few-pixel scratches avoid corrugated macro bands.
        grain = (
            0.9 * math.sin(47.0 * phase_y + 1.2 * math.sin(3.0 * phase_x))
            + 0.55 * math.sin(31.0 * phase_y + 5.0 * phase_x)
        )
        return round(grain + fine * 0.1 + medium * 0.04)
    if pattern == "composite":
        # Fine twill response without a visible cloth-sized checker.
        warp = 0.9 * math.sin(24.0 * phase_x + 24.0 * phase_y)
        weft = 0.7 * math.sin(24.0 * phase_x - 24.0 * phase_y)
        interlace = (
            0.4
            * math.sin(48.0 * phase_x)
            * math.sin(48.0 * phase_y)
        )
        return round(warp + weft + interlace + fine * 0.06)
    if pattern == "rubber":
        # Fine stipple and shallow dimples, kept below geometric feature scale.
        dimples = (
            0.9
            * math.sin(28.0 * phase_x)
            * math.sin(28.0 * phase_y)
        )
        return round(dimples + fine * 0.22 + medium * 0.08)
    if pattern == "coated":
        # Low-amplitude orange peel; colour remains mostly uniform.
        return round(fine * 0.28 + medium * 0.18 + broad * 0.08)
    if pattern == "glass":
        return round(broad * 0.08 + medium * 0.05)
    if pattern == "emissive":
        diffuser = 0.8 * math.sin(6.0 * phase_x) * math.sin(6.0 * phase_y)
        return round(diffuser + medium * 0.1)
    # Cross-hatched machining marks avoid the v2 single-axis wide bands.
    machining = (
        0.9 * math.sin(29.0 * phase_x + 13.0 * phase_y)
        + 0.55 * math.sin(17.0 * phase_x - 23.0 * phase_y)
    )
    return round(machining + fine * 0.08 + medium * 0.04)


def _v4_surface_height(
    pattern: str,
    x: int,
    y: int,
    seed: int,
    *,
    width: int,
    height: int,
) -> float:
    """Production-concept detail that survives the workbench mip chain.

    The v3 texture contract intentionally kept only very fine microstructure.
    Simply rendering the same frequencies at 512 px caused most energy to
    disappear when a 320 mm tile occupied roughly 70-190 screen pixels.  v4
    therefore mixes reviewed 12-48 cycle mid-scale response with a smaller
    high-frequency layer and keeps floating-point height until final encoding.
    """

    fine = _smooth_periodic_noise_extent(
        x, y, seed + 7, 4, width=width, height=height
    )
    medium = _smooth_periodic_noise_extent(
        x, y, seed + 19, 16, width=width, height=height
    )
    broad = _smooth_periodic_noise_extent(
        x, y, seed + 37, 64, width=width, height=height
    )
    phase_x = 2.0 * math.pi * x / width
    phase_y = 2.0 * math.pi * y / height
    if pattern == "brushed":
        grain = (
            3.2 * math.sin(24.0 * phase_y + 0.8 * math.sin(3.0 * phase_x))
            + 1.8 * math.sin(48.0 * phase_y + 7.0 * phase_x)
            + 0.55 * math.sin(127.0 * phase_y + 17.0 * phase_x)
        )
        return grain + fine * 0.16 + medium * 0.12
    if pattern == "composite":
        warp = 3.0 * math.sin(24.0 * phase_x + 24.0 * phase_y)
        weft = 2.4 * math.sin(24.0 * phase_x - 24.0 * phase_y)
        interlace = (
            1.2
            * math.sin(48.0 * phase_x)
            * math.sin(48.0 * phase_y)
        )
        return warp + weft + interlace + fine * 0.2 + medium * 0.08
    if pattern == "rubber":
        dimples = (
            4.0
            * math.sin(20.0 * phase_x)
            * math.sin(20.0 * phase_y)
        )
        return dimples + fine * 0.48 + medium * 0.24
    if pattern == "coated":
        orange_peel = (
            1.8
            * math.sin(16.0 * phase_x)
            * math.sin(18.0 * phase_y)
        )
        return orange_peel + fine * 0.4 + medium * 0.24 + broad * 0.1
    if pattern == "glass":
        return broad * 0.1 + medium * 0.06 + fine * 0.025
    if pattern == "emissive":
        diffuser = (
            1.5
            * math.sin(12.0 * phase_x)
            * math.sin(12.0 * phase_y)
        )
        return diffuser + medium * 0.12 + fine * 0.04
    machining = (
        2.8 * math.sin(23.0 * phase_x + 11.0 * phase_y)
        + 1.8 * math.sin(37.0 * phase_x - 29.0 * phase_y)
        + 0.55 * math.sin(109.0 * phase_x + 53.0 * phase_y)
    )
    return machining + fine * 0.16 + medium * 0.1


def _legacy_surface_height(pattern: str, x: int, y: int, seed: int) -> int:
    """Reproduce the immutable builtin v1 texture bytes for readback only."""

    micro = _noise(x, y, seed)
    broad = _noise(x // 12, y // 12, seed + 19)
    if pattern == "brushed":
        return _noise(0, y, seed + 31) // 2 + micro // 4
    if pattern == "composite":
        warp = 3 if x % 18 in {0, 1} else -1 if x % 18 in {9, 10} else 0
        weft = 3 if y % 18 in {0, 1} else -1 if y % 18 in {9, 10} else 0
        return warp + weft + micro // 5
    if pattern == "rubber":
        return micro // 2 + broad // 4
    if pattern == "coated":
        return broad // 3 + micro // 6
    if pattern == "glass":
        return broad // 5
    if pattern == "emissive":
        return 2 + broad // 4
    return micro // 3 + (2 if x % 28 == 0 else 0)


def _png_rgb(rows: Iterable[bytes], *, width: int = TEXTURE_WIDTH, height: int = TEXTURE_HEIGHT) -> bytes:
    raw = b"".join(b"\x00" + row for row in rows)

    def chunk(tag: bytes, payload: bytes) -> bytes:
        return (
            struct.pack(">I", len(payload))
            + tag
            + payload
            + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)
        )

    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(
        ">IIBBBBB", width, height, 8, 2, 0, 0, 0
    )) + chunk(b"IDAT", zlib.compress(raw, level=9)) + chunk(b"IEND", b"")


def _render_texture_set_bytes_scalar(
    material: Mapping[str, object],
    *,
    version: str,
) -> Mapping[str, bytes]:
    """Reference renderer retained for preview contracts and byte-equivalence tests."""

    base = tuple(int(value) for value in material["base"])
    metallic = int(material["metallic"])
    roughness = int(material["roughness"])
    emissive = tuple(int(value) for value in material["emissive"])
    pattern = str(material["pattern"])
    seed = sum(base) + metallic * 3 + roughness * 5
    if version == "4":
        width = PRODUCTION_TEXTURE_WIDTH
        height = PRODUCTION_TEXTURE_HEIGHT
        heights = tuple(
            _v4_surface_height(
                pattern,
                x,
                y,
                seed,
                width=width,
                height=height,
            )
            for y in range(height)
            for x in range(width)
        )
    else:
        width = TEXTURE_WIDTH
        height = TEXTURE_HEIGHT
        height_function = {
            "1": _legacy_surface_height,
            "2": _v2_surface_height,
            "3": _v3_surface_height,
        }.get(version)
        if height_function is None:
            raise ValueError(f"unsupported built-in visual texture version: {version}")
        heights = tuple(
            height_function(pattern, x, y, seed)
            for y in range(height)
            for x in range(width)
        )
    if version not in {"1", "2", "3", "4"}:
        raise ValueError(f"unsupported built-in visual texture version: {version}")
    rows_by_role: dict[str, list[bytes]] = {role: [] for role in SUPPORTED_PBR_ROLES}
    for y in range(height):
        row_by_role = {role: bytearray() for role in SUPPORTED_PBR_ROLES}
        for x in range(width):
            surface_height = heights[y * width + x]
            if version == "1":
                variation = surface_height + _noise(x // 8, y // 8, seed + 53) // 4
            elif version == "2":
                variation = (
                    surface_height
                    + _smooth_periodic_noise(x, y, seed + 53, 8) // 4
                )
            elif version == "4":
                variation = (
                    surface_height
                    + _smooth_periodic_noise_extent(
                        x,
                        y,
                        seed + 53,
                        8,
                        width=width,
                        height=height,
                    )
                    * 0.24
                    + _smooth_periodic_noise_extent(
                        x,
                        y,
                        seed + 71,
                        64,
                        width=width,
                        height=height,
                    )
                    * 0.1
                )
            else:
                variation = round(
                    surface_height
                    + _smooth_periodic_noise(x, y, seed + 53, 4) * 0.18
                    + _smooth_periodic_noise(x, y, seed + 71, 16) * 0.08
                )
            if version == "1":
                color_variation = variation
            elif version == "4":
                color_scale = {
                    "coated": 0.18,
                    "brushed": 0.25,
                    "glass": 0.16,
                    "composite": 0.32,
                    "rubber": 0.3,
                    "emissive": 0.2,
                }.get(pattern, 0.28)
                color_variation = round(variation * color_scale)
            elif pattern == "coated":
                color_variation = round(variation * 0.25)
            elif pattern in {"brushed", "glass"}:
                color_variation = round(variation * 0.5)
            else:
                color_variation = variation
            row_by_role["base_color"].extend(
                tuple(_clamp(value + color_variation) for value in base)
            )
            row_by_role["metallic_roughness"].extend(
                (
                    255,
                    _clamp(round(roughness + variation)),
                    _clamp(
                        round(
                            metallic
                            + (
                                surface_height * 0.08
                                if version == "4"
                                else surface_height // 2
                            )
                        )
                    ),
                )
            )
            left = heights[y * width + (x - 1) % width]
            right = heights[y * width + (x + 1) % width]
            above = heights[((y - 1) % height) * width + x]
            below = heights[((y + 1) % height) * width + x]
            row_by_role["normal"].extend(
                (
                    _clamp(
                        round(
                            128
                            - (right - left) * (4.2 if version == "4" else 2)
                        )
                    ),
                    _clamp(
                        round(
                            128
                            + (below - above) * (4.2 if version == "4" else 2)
                        )
                    ),
                    254,
                )
            )
            ambient = _clamp(
                round(
                    248
                    - max(0, -surface_height)
                    * (1 if version in {"3", "4"} else 2)
                )
            )
            row_by_role["occlusion"].extend((ambient, ambient, ambient))
            row_by_role["emissive"].extend(
                tuple(
                    _clamp(round(value + variation * 2))
                    if value
                    else 0
                    for value in emissive
                )
            )
        for role in SUPPORTED_PBR_ROLES:
            rows_by_role[role].append(bytes(row_by_role[role]))
    return {
        role: _png_rgb(rows_by_role[role], width=width, height=height)
        for role in SUPPORTED_PBR_ROLES
    }


def _noise_grid(x: np.ndarray, y: np.ndarray, seed: int) -> np.ndarray:
    """Vectorized form of :func:`_noise` with identical unsigned-32-bit math."""

    mask = np.uint64(0xFFFFFFFF)
    x_u = x.astype(np.uint64, copy=False)
    y_u = y.astype(np.uint64, copy=False)
    value = (
        x_u * np.uint64(374761393)
        + y_u * np.uint64(668265263)
        + np.uint64(seed & 0xFFFFFFFF) * np.uint64(2246822519)
    ) & mask
    value = ((value ^ (value >> np.uint64(13))) * np.uint64(1274126177)) & mask
    return ((value ^ (value >> np.uint64(16))) % np.uint64(17)).astype(np.float64) - 8.0


def _smooth_periodic_noise_extent_grid(
    x: np.ndarray,
    y: np.ndarray,
    seed: int,
    cell_size: int,
    *,
    width: int,
    height: int,
) -> np.ndarray:
    if cell_size <= 0 or width % cell_size or height % cell_size:
        raise ValueError("smooth visual noise cell size must divide the texture extent")
    cells_x = width // cell_size
    cells_y = height // cell_size
    grid_x = x // cell_size
    grid_y = y // cell_size
    local_x = (x % cell_size) / cell_size
    local_y = (y % cell_size) / cell_size
    smooth_x = local_x * local_x * (3.0 - 2.0 * local_x)
    smooth_y = local_y * local_y * (3.0 - 2.0 * local_y)

    def sample(offset_x: int, offset_y: int) -> np.ndarray:
        return _noise_grid(
            (grid_x + offset_x) % cells_x,
            (grid_y + offset_y) % cells_y,
            seed,
        )

    top = sample(0, 0) * (1.0 - smooth_x) + sample(1, 0) * smooth_x
    bottom = sample(0, 1) * (1.0 - smooth_x) + sample(1, 1) * smooth_x
    return np.rint(top * (1.0 - smooth_y) + bottom * smooth_y)


def _v4_surface_height_grid(
    pattern: str,
    x: np.ndarray,
    y: np.ndarray,
    seed: int,
    *,
    width: int,
    height: int,
) -> np.ndarray:
    fine = _smooth_periodic_noise_extent_grid(
        x, y, seed + 7, 4, width=width, height=height
    )
    medium = _smooth_periodic_noise_extent_grid(
        x, y, seed + 19, 16, width=width, height=height
    )
    broad = _smooth_periodic_noise_extent_grid(
        x, y, seed + 37, 64, width=width, height=height
    )
    phase_x = 2.0 * math.pi * x / width
    phase_y = 2.0 * math.pi * y / height
    if pattern == "brushed":
        grain = (
            3.2 * np.sin(24.0 * phase_y + 0.8 * np.sin(3.0 * phase_x))
            + 1.8 * np.sin(48.0 * phase_y + 7.0 * phase_x)
            + 0.55 * np.sin(127.0 * phase_y + 17.0 * phase_x)
        )
        return grain + fine * 0.16 + medium * 0.12
    if pattern == "composite":
        warp = 3.0 * np.sin(24.0 * phase_x + 24.0 * phase_y)
        weft = 2.4 * np.sin(24.0 * phase_x - 24.0 * phase_y)
        interlace = 1.2 * np.sin(48.0 * phase_x) * np.sin(48.0 * phase_y)
        return warp + weft + interlace + fine * 0.2 + medium * 0.08
    if pattern == "rubber":
        dimples = 4.0 * np.sin(20.0 * phase_x) * np.sin(20.0 * phase_y)
        return dimples + fine * 0.48 + medium * 0.24
    if pattern == "coated":
        orange_peel = 1.8 * np.sin(16.0 * phase_x) * np.sin(18.0 * phase_y)
        return orange_peel + fine * 0.4 + medium * 0.24 + broad * 0.1
    if pattern == "glass":
        return broad * 0.1 + medium * 0.06 + fine * 0.025
    if pattern == "emissive":
        diffuser = 1.5 * np.sin(12.0 * phase_x) * np.sin(12.0 * phase_y)
        return diffuser + medium * 0.12 + fine * 0.04
    machining = (
        2.8 * np.sin(23.0 * phase_x + 11.0 * phase_y)
        + 1.8 * np.sin(37.0 * phase_x - 29.0 * phase_y)
        + 0.55 * np.sin(109.0 * phase_x + 53.0 * phase_y)
    )
    return machining + fine * 0.16 + medium * 0.1


def _uint8_channel(value: np.ndarray | float) -> np.ndarray:
    return np.clip(np.rint(value), 0, 255).astype(np.uint8)


def _png_rgb_array(value: np.ndarray) -> bytes:
    height, width, channels = value.shape
    if channels != 3 or value.dtype != np.uint8:
        raise ValueError("PBR texture array must be uint8 RGB")
    return _png_rgb(
        (value[row].tobytes() for row in range(height)),
        width=width,
        height=height,
    )


def _render_v4_texture_set_bytes(material: Mapping[str, object]) -> Mapping[str, bytes]:
    """Render the 1K production maps in vectorized bounded passes.

    The former nested Python loop performed the same deterministic equations
    more than seven million times for a normal mechanical-arm asset.  NumPy is
    already a pinned sidecar dependency for Manifold, so this keeps the exact
    data-only texture contract while returning quality/readback inside the
    workbench Gate rather than blocking the single Agent worker for minutes.
    """

    width = PRODUCTION_TEXTURE_WIDTH
    height = PRODUCTION_TEXTURE_HEIGHT
    x = np.arange(width, dtype=np.int64)[None, :]
    y = np.arange(height, dtype=np.int64)[:, None]
    base = np.asarray(tuple(int(value) for value in material["base"]), dtype=np.float64)
    metallic = int(material["metallic"])
    roughness = int(material["roughness"])
    emissive = np.asarray(
        tuple(int(value) for value in material["emissive"]), dtype=np.float64
    )
    pattern = str(material["pattern"])
    seed = int(base.sum()) + metallic * 3 + roughness * 5
    heights = _v4_surface_height_grid(
        pattern,
        x,
        y,
        seed,
        width=width,
        height=height,
    )
    variation = (
        heights
        + _smooth_periodic_noise_extent_grid(
            x, y, seed + 53, 8, width=width, height=height
        )
        * 0.24
        + _smooth_periodic_noise_extent_grid(
            x, y, seed + 71, 64, width=width, height=height
        )
        * 0.1
    )
    color_scale = {
        "coated": 0.18,
        "brushed": 0.25,
        "glass": 0.16,
        "composite": 0.32,
        "rubber": 0.3,
        "emissive": 0.2,
    }.get(pattern, 0.28)
    color_variation = np.rint(variation * color_scale)
    base_color = np.stack(
        [_uint8_channel(base[channel] + color_variation) for channel in range(3)],
        axis=-1,
    )
    metallic_roughness = np.stack(
        (
            np.full((height, width), 255, dtype=np.uint8),
            _uint8_channel(roughness + variation),
            _uint8_channel(metallic + heights * 0.08),
        ),
        axis=-1,
    )
    left = np.roll(heights, 1, axis=1)
    right = np.roll(heights, -1, axis=1)
    above = np.roll(heights, 1, axis=0)
    below = np.roll(heights, -1, axis=0)
    normal = np.stack(
        (
            _uint8_channel(128 - (right - left) * 4.2),
            _uint8_channel(128 + (below - above) * 4.2),
            np.full((height, width), 254, dtype=np.uint8),
        ),
        axis=-1,
    )
    ambient = _uint8_channel(248 - np.maximum(0.0, -heights))
    occlusion = np.repeat(ambient[:, :, None], 3, axis=2)
    emissive_map = np.stack(
        [
            _uint8_channel(emissive[channel] + variation * 2)
            if emissive[channel]
            else np.zeros((height, width), dtype=np.uint8)
            for channel in range(3)
        ],
        axis=-1,
    )
    arrays = {
        "base_color": base_color,
        "metallic_roughness": metallic_roughness,
        "normal": normal,
        "occlusion": occlusion,
        "emissive": emissive_map,
    }
    return {role: _png_rgb_array(arrays[role]) for role in SUPPORTED_PBR_ROLES}


def _render_texture_set_bytes(
    material: Mapping[str, object],
    *,
    version: str,
) -> Mapping[str, bytes]:
    if version == "4":
        return _render_v4_texture_set_bytes(material)
    return _render_texture_set_bytes_scalar(material, version=version)


def _smooth_step(edge0: float, edge1: float, value: float) -> float:
    if edge0 >= edge1:
        raise ValueError("surface adornment mask edges are invalid")
    amount = max(0.0, min(1.0, (value - edge0) / (edge1 - edge0)))
    return amount * amount * (3.0 - 2.0 * amount)


def _surface_adornment_coverage_mask(coverage: str, u: float, v: float) -> float:
    """Return a softened closed-region mask in UV0 texture space."""

    if coverage == "full_zone":
        return 1.0
    if coverage == "center_band":
        return 1.0 - _smooth_step(0.18, 0.34, abs(u - 0.5))
    if coverage == "edge_band":
        distance = min(u, 1.0 - u, v, 1.0 - v)
        return 1.0 - _smooth_step(0.08, 0.22, distance)
    if coverage == "symmetric_pair":
        left = 1.0 - _smooth_step(0.08, 0.22, abs(u - 0.27))
        right = 1.0 - _smooth_step(0.08, 0.22, abs(u - 0.73))
        return max(left, right)
    raise ValueError("surface adornment coverage is invalid")


def _surface_adornment_height(
    program: Mapping[str, object],
    x: int,
    y: int,
    *,
    width: int,
    height: int,
) -> float:
    """A deterministic, tileable, visual-only height field for A005 maps."""

    u = x / width
    v = y / height
    seed = int(program["seed"])
    phase = (seed % 4096) / 4096.0 * math.tau
    motif = str(program["motif"])
    if motif == "parallel_groove":
        base = math.sin(math.tau * (14.0 * u + 0.9 * v) + phase)
        base += 0.26 * math.sin(math.tau * (28.0 * u + 1.8 * v) + phase * 1.7)
    elif motif == "chevron_relief":
        diagonal = abs((u - 0.5) * 2.0)
        base = math.sin(math.tau * (11.0 * v + 5.0 * diagonal) + phase)
        base += 0.18 * math.sin(math.tau * (22.0 * v - 7.0 * diagonal) + phase * 0.7)
    elif motif == "double_flowline":
        curve_a = v - (0.31 + 0.12 * math.sin(math.tau * u + phase))
        curve_b = v - (0.69 - 0.12 * math.sin(math.tau * u + phase))
        base = math.cos(math.tau * 16.0 * curve_a) + math.cos(math.tau * 16.0 * curve_b)
        base *= 0.52
    elif motif == "hex_microgrid":
        base = (
            math.cos(math.tau * 13.0 * u + phase)
            + math.cos(math.tau * (6.5 * u + 11.258 * v) + phase)
            + math.cos(math.tau * (6.5 * u - 11.258 * v) + phase)
        ) / 3.0
    else:
        raise ValueError("surface adornment motif is invalid")
    kind = str(program["kind"])
    if kind == "normal_relief":
        kind_scale = 1.25
    elif kind == "pattern":
        kind_scale = 0.78
    elif kind == "flowline":
        kind_scale = 0.94
    elif kind == "micro_surface":
        kind_scale = 0.48
        base += _smooth_periodic_noise_extent(
            x, y, seed + 131, 8, width=width, height=height
        ) * 0.16
    else:
        raise ValueError("surface adornment kind is invalid")
    intensity_scale = {"subtle": 1.6, "balanced": 3.2, "pronounced": 5.4}[str(program["intensity"])]
    return base * kind_scale * intensity_scale * _surface_adornment_coverage_mask(
        str(program["coverage"]), u, v
    )


def _render_surface_adornment_texture_set_bytes(
    program: Mapping[str, object],
    *,
    artifact_profile_id: GeometryArtifactProfileId,
) -> Mapping[str, bytes]:
    """Bake A005's closed detail grammar into five complete PBR map bytes."""

    normalized = normalize_surface_adornment_program(program)
    base_index, _canonical_material = builtin_visual_material_binding(
        str(normalized["base_material"])
    )
    material = builtin_material_properties(base_index)
    width = (
        PRODUCTION_TEXTURE_WIDTH
        if artifact_profile_id == "production_concept"
        else TEXTURE_WIDTH
    )
    height = (
        PRODUCTION_TEXTURE_HEIGHT
        if artifact_profile_id == "production_concept"
        else TEXTURE_HEIGHT
    )
    heights = tuple(
        _surface_adornment_height(normalized, x, y, width=width, height=height)
        for y in range(height)
        for x in range(width)
    )
    rows_by_role: dict[str, list[bytes]] = {role: [] for role in SUPPORTED_PBR_ROLES}
    base = tuple(int(value) for value in material["base"])
    emissive = tuple(int(value) for value in material["emissive"])
    base_metallic = int(material["metallic"])
    base_roughness = int(material["roughness"])
    kind = str(normalized["kind"])
    intensity = str(normalized["intensity"])
    normal_scale = {"subtle": 2.2, "balanced": 3.6, "pronounced": 5.2}[intensity]
    for y in range(height):
        row_by_role = {role: bytearray() for role in SUPPORTED_PBR_ROLES}
        for x in range(width):
            current = heights[y * width + x]
            left = heights[y * width + (x - 1) % width]
            right = heights[y * width + (x + 1) % width]
            above = heights[((y - 1) % height) * width + x]
            below = heights[((y + 1) % height) * width + x]
            colour_shift = (
                round(current * (1.3 if kind == "pattern" else 0.42))
                if kind != "micro_surface"
                else round(current * 0.2)
            )
            roughness_shift = round(current * (2.2 if kind == "micro_surface" else 1.7))
            metallic_shift = round(current * 0.25)
            row_by_role["base_color"].extend(
                tuple(_clamp(value + colour_shift) for value in base)
            )
            row_by_role["metallic_roughness"].extend((
                255,
                _clamp(base_roughness + roughness_shift),
                _clamp(base_metallic + metallic_shift),
            ))
            row_by_role["normal"].extend((
                _clamp(round(128 - (right - left) * normal_scale)),
                _clamp(round(128 + (below - above) * normal_scale)),
                254,
            ))
            occlusion = _clamp(round(250 - max(0.0, -current) * 1.1))
            row_by_role["occlusion"].extend((occlusion, occlusion, occlusion))
            row_by_role["emissive"].extend(tuple(
                _clamp(round(value + current * 0.4)) if value else 0
                for value in emissive
            ))
        for role in SUPPORTED_PBR_ROLES:
            rows_by_role[role].append(bytes(row_by_role[role]))
    return {
        role: _png_rgb(rows_by_role[role], width=width, height=height)
        for role in SUPPORTED_PBR_ROLES
    }


@lru_cache(maxsize=32)
def _cached_surface_adornment_texture_set_bytes(
    artifact_profile_id: GeometryArtifactProfileId,
    canonical_program: str,
) -> Mapping[str, bytes]:
    return _render_surface_adornment_texture_set_bytes(
        json.loads(canonical_program),
        artifact_profile_id=artifact_profile_id,
    )


def surface_adornment_visual_texture_set(
    program: Mapping[str, object],
    *,
    artifact_profile_id: GeometryArtifactProfileId,
) -> VisualTextureSet:
    """Return a content-addressed five-map set for one normalized A005 input."""

    normalized = normalize_surface_adornment_program(program)
    canonical_program = json_canonical(normalized)
    payloads = _cached_surface_adornment_texture_set_bytes(
        artifact_profile_id,
        canonical_program,
    )
    suffix = surface_adornment_program_sha256(normalized)[:32]
    version = "4" if artifact_profile_id == "production_concept" else "3"
    width = PRODUCTION_TEXTURE_WIDTH if artifact_profile_id == "production_concept" else TEXTURE_WIDTH
    height = PRODUCTION_TEXTURE_HEIGHT if artifact_profile_id == "production_concept" else TEXTURE_HEIGHT
    return VisualTextureSet(
        visual_texture_set_id=_surface_adornment_texture_set_id(normalized, artifact_profile_id),
        material_id=surface_adornment_material_id(normalized),
        display_name=f"A005 {normalized['motif']}",
        maps=[
            VisualTextureMap(
                texture_id=f"vtex_a005_{suffix}_v{version}_{role}",
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
        version=version,
    )


def surface_adornment_visual_texture_png_bytes(
    program: Mapping[str, object],
    *,
    artifact_profile_id: GeometryArtifactProfileId,
    texture_role: str,
) -> bytes:
    if texture_role not in SUPPORTED_PBR_ROLES:
        raise ValueError("surface adornment texture role is invalid")
    normalized = normalize_surface_adornment_program(program)
    return _cached_surface_adornment_texture_set_bytes(
        artifact_profile_id,
        json_canonical(normalized),
    )[texture_role]


def surface_adornment_texture_cache_facts() -> Mapping[str, int]:
    """Expose the bounded A005 byte cache without retaining individual pixels."""

    info = _cached_surface_adornment_texture_set_bytes.cache_info()
    return {
        "entry_count": info.currsize,
        "max_entries": int(info.maxsize or 0),
    }


@lru_cache(maxsize=len(_MATERIALS) * 3)
def _cached_texture_set_bytes(version: str, material_id: str) -> Mapping[str, bytes]:
    material = next(
        (item for item in _MATERIALS if item["material_id"] == material_id),
        None,
    )
    if material is None:
        raise ValueError(f"unknown built-in visual material id: {material_id}")
    if version not in {"1", "2", "3"}:
        raise ValueError(f"unsupported built-in visual texture version: {version}")
    return _render_texture_set_bytes(material, version=version)


@lru_cache(maxsize=len(_MATERIALS))
def _cached_production_texture_set_bytes(material_id: str) -> Mapping[str, bytes]:
    material = next(
        (item for item in _MATERIALS if item["material_id"] == material_id),
        None,
    )
    if material is None:
        raise ValueError(f"unknown built-in visual material id: {material_id}")
    return _render_texture_set_bytes(material, version="4")


def _texture_bytes(role: str, material: Mapping[str, object], *, version: str = "2") -> bytes:
    try:
        if version == "4":
            return _cached_production_texture_set_bytes(str(material["material_id"]))[role]
        return _cached_texture_set_bytes(version, str(material["material_id"]))[role]
    except KeyError as exc:
        raise ValueError(f"unsupported visual texture role: {role}") from exc


def _map(role: str, material: Mapping[str, object], *, version: str) -> VisualTextureMap:
    payload = _texture_bytes(role, material, version=version)
    slug = str(material["material_id"]).removeprefix("mat_")
    version_segment = "" if version == "1" else f"_v{version}"
    width = PRODUCTION_TEXTURE_WIDTH if version == "4" else TEXTURE_WIDTH
    height = PRODUCTION_TEXTURE_HEIGHT if version == "4" else TEXTURE_HEIGHT
    return VisualTextureMap(
        texture_id=f"vtex_{slug}{version_segment}_{role}",
        texture_role=role,
        mime_type="image/png",
        byte_size=len(payload),
        sha256=hashlib.sha256(payload).hexdigest(),
        color_space="srgb" if role in {"base_color", "emissive"} else "linear",
        width=width,
        height=height,
        source="forgecad_builtin",
        license="not_applicable",
        fallback="none",
    )


@lru_cache(maxsize=1)
def builtin_visual_texture_sets() -> tuple[VisualTextureSet, ...]:
    return tuple(
        VisualTextureSet(
            visual_texture_set_id=f"vtexset_{str(material['material_id']).removeprefix('mat_')}_builtin_v3",
            material_id=str(material["material_id"]),
            display_name=str(material["name"]),
            maps=[_map(role, material, version="3") for role in SUPPORTED_PBR_ROLES],
            source="forgecad_builtin",
            license="not_applicable",
            version="3",
        )
        for material in _MATERIALS
    )


@lru_cache(maxsize=1)
def production_visual_texture_sets() -> tuple[VisualTextureSet, ...]:
    return tuple(
        production_visual_texture_set_for_material_index(index)
        for index in range(len(_MATERIALS))
    )


@lru_cache(maxsize=len(_MATERIALS))
def production_visual_texture_set_for_material_index(index: int) -> VisualTextureSet:
    try:
        material = _MATERIALS[index]
    except IndexError as exc:
        raise ValueError(
            f"unsupported GLB material index for production visual texture set: {index}"
        ) from exc
    return VisualTextureSet(
        visual_texture_set_id=f"vtexset_{str(material['material_id']).removeprefix('mat_')}_builtin_v4",
        material_id=str(material["material_id"]),
        display_name=str(material["name"]),
        maps=[_map(role, material, version="4") for role in SUPPORTED_PBR_ROLES],
        source="forgecad_builtin",
        license="not_applicable",
        version="4",
    )


@lru_cache(maxsize=1)
def legacy_builtin_visual_texture_sets_v2() -> tuple[VisualTextureSet, ...]:
    """Return exact v2 manifests so immutable historical GLBs still read."""

    return tuple(
        VisualTextureSet(
            visual_texture_set_id=f"vtexset_{str(material['material_id']).removeprefix('mat_')}_builtin_v2",
            material_id=str(material["material_id"]),
            display_name=str(material["name"]),
            maps=[_map(role, material, version="2") for role in SUPPORTED_PBR_ROLES],
            source="forgecad_builtin",
            license="not_applicable",
            version="2",
        )
        for material in _MATERIALS
    )


@lru_cache(maxsize=1)
def legacy_builtin_visual_texture_sets() -> tuple[VisualTextureSet, ...]:
    """Return exact v1 manifests only so immutable historical GLBs still read."""

    return tuple(
        VisualTextureSet(
            visual_texture_set_id=f"vtexset_{str(material['material_id']).removeprefix('mat_')}_builtin",
            material_id=str(material["material_id"]),
            display_name=str(material["name"]),
            maps=[_map(role, material, version="1") for role in SUPPORTED_PBR_ROLES],
            source="forgecad_builtin",
            license="not_applicable",
            version="1",
        )
        for material in _MATERIALS
    )


def builtin_visual_texture_set_for_material_index(
    index: int,
    *,
    artifact_profile_id: GeometryArtifactProfileId = "interactive_preview",
) -> VisualTextureSet:
    try:
        texture_sets = (
            None
            if artifact_profile_id == "production_concept"
            else builtin_visual_texture_sets()
        )
        if artifact_profile_id == "production_concept":
            return production_visual_texture_set_for_material_index(index)
        assert texture_sets is not None
        return texture_sets[index]
    except IndexError as exc:
        raise ValueError(f"unsupported GLB material index for visual texture set: {index}") from exc


def builtin_visual_texture_set_for_readback(
    index: int,
    visual_texture_set_id: str,
) -> VisualTextureSet:
    """Resolve only an exact current or immutable legacy built-in identity."""

    try:
        if visual_texture_set_id.endswith("_builtin_v4"):
            candidate = production_visual_texture_set_for_material_index(index)
        elif visual_texture_set_id.endswith("_builtin_v3"):
            candidate = builtin_visual_texture_sets()[index]
        elif visual_texture_set_id.endswith("_builtin_v2"):
            candidate = legacy_builtin_visual_texture_sets_v2()[index]
        elif visual_texture_set_id.endswith("_builtin"):
            candidate = legacy_builtin_visual_texture_sets()[index]
        else:
            raise ValueError(
                "GLB material does not match a supported built-in VisualTextureSet identity"
            )
    except IndexError as exc:
        raise ValueError(f"unsupported GLB material index for visual texture set: {index}") from exc
    if candidate.visual_texture_set_id == visual_texture_set_id:
        return candidate
    raise ValueError("GLB material does not match a supported built-in VisualTextureSet identity")


def builtin_visual_material_count() -> int:
    """Return the fixed built-in GLB material-table size."""

    return len(_MATERIALS)


def visual_texture_png_bytes(texture_id: str) -> bytes:
    # Versioned map ids select one exact immutable manifest lazily so current
    # compilation does not generate either historical PNG set merely to reject
    # it.
    if "_v4_" in texture_id:
        version = "4"
        for material in _MATERIALS:
            slug = str(material["material_id"]).removeprefix("mat_")
            prefix = f"vtex_{slug}_v4_"
            if texture_id.startswith(prefix):
                role = texture_id.removeprefix(prefix)
                if role in SUPPORTED_PBR_ROLES:
                    return _texture_bytes(role, material, version=version)
        raise ValueError(f"unknown built-in visual texture id: {texture_id}")
    elif "_v3_" in texture_id:
        version = "3"
        texture_sets = builtin_visual_texture_sets()
    elif "_v2_" in texture_id:
        version = "2"
        texture_sets = legacy_builtin_visual_texture_sets_v2()
    else:
        version = "1"
        texture_sets = legacy_builtin_visual_texture_sets()
    for material, texture_set in zip(_MATERIALS, texture_sets):
        for item in texture_set.maps:
            if item.texture_id == texture_id:
                return _texture_bytes(item.texture_role, material, version=version)
    raise ValueError(f"unknown built-in visual texture id: {texture_id}")


def builtin_visual_material_binding(material_id: str | None) -> tuple[int, str]:
    """Return the canonical GLB index/id for one reviewed authored material."""

    authored_material_id = material_id or "mat_primary"
    canonical_material_id = _AUTHORED_TO_TEXTURE_MATERIAL.get(authored_material_id)
    if canonical_material_id is None:
        raise ValueError(f"unsupported authored visual material id: {authored_material_id}")
    for index, material in enumerate(_MATERIALS):
        if material["material_id"] == canonical_material_id:
            return index, canonical_material_id
    raise ValueError(f"built-in visual material binding is incomplete: {canonical_material_id}")


def builtin_visual_texture_cache_facts() -> Mapping[str, int]:
    """Expose bounded cache facts for the M108 sidecar regression gate."""

    cached_sets = tuple(
        _cached_texture_set_bytes(version, str(material["material_id"]))
        for version in ("1", "2", "3")
        for material in _MATERIALS
    )
    # Resolve all immutable versions: only forty compact PNG byte strings per
    # version are retained, never
    # 128x128 Python cache entries per channel/pixel.
    return {
        "entry_count": _cached_texture_set_bytes.cache_info().currsize,
        "max_entries": int(_cached_texture_set_bytes.cache_info().maxsize or 0),
        "png_byte_size": sum(
            len(payload)
            for texture_set in cached_sets
            for payload in texture_set.values()
        ),
        "production_entry_count": _cached_production_texture_set_bytes.cache_info().currsize,
        "production_max_entries": int(
            _cached_production_texture_set_bytes.cache_info().maxsize or 0
        ),
    }


def builtin_material_properties(index: int) -> Mapping[str, object]:
    try:
        return _MATERIALS[index]
    except IndexError as exc:
        raise ValueError(f"unsupported GLB material index: {index}") from exc


def studio_environment_manifest() -> dict[str, object]:
    """The renderer's fixed RoomEnvironment/PMREM display contract.

    This is a procedural studio profile rather than a claimed third-party
    HDRI.  Its canonical hash travels with the GLB so the viewport cannot
    silently substitute a different tone-mapping or contact-shadow setup.
    """

    payload: dict[str, object] = {
        "schema_version": "ForgeCADVisualEnvironment@1",
        "environment_id": "env_forgecad_room_studio_v1",
        "environment_kind": "procedural_studio",
        "source": "forgecad_builtin",
        "license": "not_applicable",
        "color_workflow": "linear_srgb",
        "output_color_space": "srgb",
        "tone_mapping": "aces_filmic",
        # This studio is deliberately contrast-preserving.  The previous
        # high-energy fill caused clearcoat paint, aluminium and graphite to
        # converge to the same pale display value in the workbench even though
        # their embedded PBR maps remained distinct.  Keep direct light for
        # form readability, but let the material palettes carry their own
        # luminance and hue hierarchy.
        "tone_mapping_exposure": 0.86,
        "contact_shadows": True,
        "pmrem": {"near": 0.04, "cube_size": 128},
        "cad_neutral_lighting": {
            "background": "#0b1420",
            "hemisphere": {"sky": "#eef6ff", "ground": "#111820", "intensity": 1.45},
            "ambient": {"color": "#8aa0b8", "intensity": 0.24},
            "key": {"color": "#f7fbff", "intensity": 3.6, "position": [150, 210, 160]},
            "rim": {"color": "#91b6d9", "intensity": 0.95, "position": [-160, 110, -120]},
            "warm_rim": {"color": "#ffd0b5", "intensity": 0.28, "position": [110, 20, -190]},
            "floor": {"kind": "shadow_catcher", "color": "#000000", "opacity": 0.16, "radius_ratio": 1.1},
        },
        "camera_views": {
            "iso": {"direction": [-0.9, 0.85, 1.55], "distance_ratio": 0.98, "fov_degrees": 38},
        },
    }
    payload["environment_sha256"] = hashlib.sha256(
        json_canonical(payload).encode("utf-8")
    ).hexdigest()
    return payload


def json_canonical(value: object) -> str:
    # Kept local to avoid a geometry-worker import cycle.
    import json

    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
