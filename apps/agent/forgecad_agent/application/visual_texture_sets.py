"""Deterministic built-in visual PBR texture sets for ShapeProgram GLBs.

The textures are deliberately generated in-process from small reviewed colour
palettes.  They are visual-only, self-contained PNGs: no network request,
filesystem path, user prompt, or hidden third-party asset participates in the
GLB result.  Their metadata is the same object consumed by GLB write and
readback validation.
"""

from __future__ import annotations

import hashlib
import math
import struct
import zlib
from functools import lru_cache
from typing import Iterable, Mapping

from .agent_models import VisualTextureMap, VisualTextureSet


TEXTURE_WIDTH = 128
TEXTURE_HEIGHT = 128
SUPPORTED_PBR_ROLES = (
    "base_color",
    "metallic_roughness",
    "normal",
    "occlusion",
    "emissive",
)


_MATERIALS: tuple[Mapping[str, object], ...] = (
    {"material_id": "mat_primary", "name": "深石墨金属外观", "pattern": "machined", "base": (96, 110, 124), "metallic": 96, "roughness": 142, "normal_scale": 0.3, "clearcoat": 0.0, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_aluminum", "name": "拉丝金属外观", "pattern": "brushed", "base": (176, 187, 198), "metallic": 232, "roughness": 82, "normal_scale": 0.55, "clearcoat": 0.0, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_signal_red", "name": "信号红涂层外观", "pattern": "coated", "base": (196, 55, 43), "metallic": 72, "roughness": 88, "normal_scale": 0.25, "clearcoat": 0.34, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_composite", "name": "哑光复合外观", "pattern": "composite", "base": (74, 87, 101), "metallic": 18, "roughness": 182, "normal_scale": 0.4, "clearcoat": 0.0, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_rubber", "name": "橡胶外观", "pattern": "rubber", "base": (34, 39, 46), "metallic": 0, "roughness": 226, "normal_scale": 0.65, "clearcoat": 0.0, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_dark_glass", "name": "深色透明外观", "pattern": "glass", "base": (45, 70, 94), "metallic": 8, "roughness": 48, "normal_scale": 0.12, "clearcoat": 0.18, "transmission": 0.54, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_emissive_blue", "name": "蓝色发光饰条外观", "pattern": "emissive", "base": (30, 92, 174), "metallic": 28, "roughness": 76, "normal_scale": 0.2, "clearcoat": 0.0, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (16, 116, 255)},
    {"material_id": "mat_automotive_paint", "name": "蓝色汽车漆外观", "pattern": "coated", "base": (61, 120, 184), "metallic": 97, "roughness": 51, "normal_scale": 0.18, "clearcoat": 0.86, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
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


def _render_texture_set_bytes(
    material: Mapping[str, object],
    *,
    version: str,
) -> Mapping[str, bytes]:
    """Render all five maps in one bounded pass without a per-pixel LRU."""

    base = tuple(int(value) for value in material["base"])
    metallic = int(material["metallic"])
    roughness = int(material["roughness"])
    emissive = tuple(int(value) for value in material["emissive"])
    pattern = str(material["pattern"])
    seed = sum(base) + metallic * 3 + roughness * 5
    height_function = {
        "1": _legacy_surface_height,
        "2": _v2_surface_height,
        "3": _v3_surface_height,
    }.get(version)
    if height_function is None:
        raise ValueError(f"unsupported built-in visual texture version: {version}")
    heights = tuple(
        height_function(pattern, x, y, seed)
        for y in range(TEXTURE_HEIGHT)
        for x in range(TEXTURE_WIDTH)
    )
    rows_by_role: dict[str, list[bytes]] = {role: [] for role in SUPPORTED_PBR_ROLES}
    for y in range(TEXTURE_HEIGHT):
        row_by_role = {role: bytearray() for role in SUPPORTED_PBR_ROLES}
        for x in range(TEXTURE_WIDTH):
            height = heights[y * TEXTURE_WIDTH + x]
            if version == "1":
                variation = height + _noise(x // 8, y // 8, seed + 53) // 4
            elif version == "2":
                variation = (
                    height
                    + _smooth_periodic_noise(x, y, seed + 53, 8) // 4
                )
            else:
                variation = round(
                    height
                    + _smooth_periodic_noise(x, y, seed + 53, 4) * 0.18
                    + _smooth_periodic_noise(x, y, seed + 71, 16) * 0.08
                )
            if version == "1":
                color_variation = variation
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
                (255, _clamp(roughness + variation), _clamp(metallic + height // 2))
            )
            left = heights[y * TEXTURE_WIDTH + (x - 1) % TEXTURE_WIDTH]
            right = heights[y * TEXTURE_WIDTH + (x + 1) % TEXTURE_WIDTH]
            above = heights[((y - 1) % TEXTURE_HEIGHT) * TEXTURE_WIDTH + x]
            below = heights[((y + 1) % TEXTURE_HEIGHT) * TEXTURE_WIDTH + x]
            row_by_role["normal"].extend(
                (_clamp(128 - (right - left) * 2), _clamp(128 + (below - above) * 2), 254)
            )
            ambient = _clamp(
                248 - max(0, -height) * (1 if version == "3" else 2)
            )
            row_by_role["occlusion"].extend((ambient, ambient, ambient))
            row_by_role["emissive"].extend(
                tuple(_clamp(value + variation * 2) if value else 0 for value in emissive)
            )
        for role in SUPPORTED_PBR_ROLES:
            rows_by_role[role].append(bytes(row_by_role[role]))
    return {role: _png_rgb(rows_by_role[role]) for role in SUPPORTED_PBR_ROLES}


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


def _texture_bytes(role: str, material: Mapping[str, object], *, version: str = "2") -> bytes:
    try:
        return _cached_texture_set_bytes(version, str(material["material_id"]))[role]
    except KeyError as exc:
        raise ValueError(f"unsupported visual texture role: {role}") from exc


def _map(role: str, material: Mapping[str, object], *, version: str) -> VisualTextureMap:
    payload = _texture_bytes(role, material, version=version)
    slug = str(material["material_id"]).removeprefix("mat_")
    version_segment = "" if version == "1" else f"_v{version}"
    return VisualTextureMap(
        texture_id=f"vtex_{slug}{version_segment}_{role}",
        texture_role=role,
        mime_type="image/png",
        byte_size=len(payload),
        sha256=hashlib.sha256(payload).hexdigest(),
        color_space="srgb" if role in {"base_color", "emissive"} else "linear",
        width=TEXTURE_WIDTH,
        height=TEXTURE_HEIGHT,
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


def builtin_visual_texture_set_for_material_index(index: int) -> VisualTextureSet:
    try:
        return builtin_visual_texture_sets()[index]
    except IndexError as exc:
        raise ValueError(f"unsupported GLB material index for visual texture set: {index}") from exc


def builtin_visual_texture_set_for_readback(
    index: int,
    visual_texture_set_id: str,
) -> VisualTextureSet:
    """Resolve only an exact current or immutable legacy built-in identity."""

    try:
        if visual_texture_set_id.endswith("_builtin_v3"):
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
    if "_v3_" in texture_id:
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
        "tone_mapping_exposure": 1.18,
        "contact_shadows": True,
        "pmrem": {"near": 0.04, "cube_size": 128},
        "cad_neutral_lighting": {
            "background": "#0b1420",
            "hemisphere": {"sky": "#eef6ff", "ground": "#111820", "intensity": 3.2},
            "ambient": {"color": "#8aa0b8", "intensity": 0.58},
            "key": {"color": "#f7fbff", "intensity": 4.8, "position": [150, 210, 160]},
            "rim": {"color": "#91b6d9", "intensity": 1.35, "position": [-160, 110, -120]},
            "warm_rim": {"color": "#ffd0b5", "intensity": 0.45, "position": [110, 20, -190]},
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
