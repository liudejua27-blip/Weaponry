"""Deterministic built-in visual PBR texture sets for ShapeProgram GLBs.

The textures are deliberately generated in-process from small reviewed colour
palettes.  They are visual-only, self-contained PNGs: no network request,
filesystem path, user prompt, or hidden third-party asset participates in the
GLB result.  Their metadata is the same object consumed by GLB write and
readback validation.
"""

from __future__ import annotations

import hashlib
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


def _clamp(value: int) -> int:
    return max(0, min(255, value))


def _noise(x: int, y: int, seed: int) -> int:
    value = (x * 374761393 + y * 668265263 + seed * 2246822519) & 0xFFFFFFFF
    value = ((value ^ (value >> 13)) * 1274126177) & 0xFFFFFFFF
    return int((value ^ (value >> 16)) % 17) - 8


def _surface_height(pattern: str, x: int, y: int, seed: int) -> int:
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


def _texture_bytes(role: str, material: Mapping[str, object]) -> bytes:
    base = tuple(int(value) for value in material["base"])
    metallic = int(material["metallic"])
    roughness = int(material["roughness"])
    emissive = tuple(int(value) for value in material["emissive"])
    pattern = str(material["pattern"])
    seed = sum(base) + metallic * 3 + roughness * 5
    rows: list[bytes] = []
    for y in range(TEXTURE_HEIGHT):
        row = bytearray()
        for x in range(TEXTURE_WIDTH):
            height = _surface_height(pattern, x, y, seed)
            variation = height + _noise(x // 8, y // 8, seed + 53) // 4
            if role == "base_color":
                pixel = tuple(_clamp(value + variation) for value in base)
            elif role == "metallic_roughness":
                pixel = (255, _clamp(roughness + variation), _clamp(metallic + height // 2))
            elif role == "normal":
                dx = _surface_height(pattern, (x + 1) % TEXTURE_WIDTH, y, seed) - _surface_height(pattern, (x - 1) % TEXTURE_WIDTH, y, seed)
                dy = _surface_height(pattern, x, (y + 1) % TEXTURE_HEIGHT, seed) - _surface_height(pattern, x, (y - 1) % TEXTURE_HEIGHT, seed)
                pixel = (_clamp(128 - dx * 2), _clamp(128 + dy * 2), 254)
            elif role == "occlusion":
                ambient = _clamp(248 - max(0, -height) * 2)
                pixel = (ambient, ambient, ambient)
            elif role == "emissive":
                pixel = tuple(_clamp(value + variation * 2) if value else 0 for value in emissive)
            else:  # Defensive: all callers use the fixed contract above.
                raise ValueError(f"unsupported visual texture role: {role}")
            row.extend(pixel)
        rows.append(bytes(row))
    return _png_rgb(rows)


def _map(role: str, material: Mapping[str, object]) -> VisualTextureMap:
    payload = _texture_bytes(role, material)
    slug = str(material["material_id"]).removeprefix("mat_")
    return VisualTextureMap(
        texture_id=f"vtex_{slug}_{role}",
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
            visual_texture_set_id=f"vtexset_{str(material['material_id']).removeprefix('mat_')}_builtin",
            material_id=str(material["material_id"]),
            display_name=str(material["name"]),
            maps=[_map(role, material) for role in SUPPORTED_PBR_ROLES],
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


def builtin_visual_material_count() -> int:
    """Return the fixed built-in GLB material-table size."""

    return len(_MATERIALS)


def visual_texture_png_bytes(texture_id: str) -> bytes:
    for material, texture_set in zip(_MATERIALS, builtin_visual_texture_sets()):
        for item in texture_set.maps:
            if item.texture_id == texture_id:
                return _texture_bytes(item.texture_role, material)
    raise ValueError(f"unknown built-in visual texture id: {texture_id}")


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
