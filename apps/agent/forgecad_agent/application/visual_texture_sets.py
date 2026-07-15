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


TEXTURE_WIDTH = 32
TEXTURE_HEIGHT = 32
SUPPORTED_PBR_ROLES = (
    "base_color",
    "metallic_roughness",
    "normal",
    "occlusion",
    "emissive",
)


_MATERIALS: tuple[Mapping[str, object], ...] = (
    {"material_id": "mat_primary", "name": "深石墨金属外观", "base": (26, 33, 42), "metallic": 198, "roughness": 72, "clearcoat": 0.0, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_aluminum", "name": "拉丝金属外观", "base": (138, 154, 168), "metallic": 224, "roughness": 58, "clearcoat": 0.0, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_signal_red", "name": "信号红涂层外观", "base": (184, 35, 27), "metallic": 122, "roughness": 78, "clearcoat": 0.28, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_composite", "name": "哑光复合外观", "base": (35, 52, 65), "metallic": 46, "roughness": 142, "clearcoat": 0.0, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_rubber", "name": "橡胶外观", "base": (14, 18, 23), "metallic": 5, "roughness": 210, "clearcoat": 0.0, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (0, 0, 0)},
    {"material_id": "mat_dark_glass", "name": "深色透明外观", "base": (10, 26, 41), "metallic": 20, "roughness": 34, "clearcoat": 0.12, "transmission": 0.48, "ior": 1.5, "alpha": 0.72, "emissive": (0, 0, 0)},
    {"material_id": "mat_emissive_blue", "name": "蓝色发光饰条外观", "base": (13, 70, 209), "metallic": 41, "roughness": 61, "clearcoat": 0.0, "transmission": 0.0, "ior": 1.5, "alpha": 1.0, "emissive": (8, 106, 255)},
)


def _clamp(value: int) -> int:
    return max(0, min(255, value))


def _png_rgb(rows: Iterable[bytes], *, width: int = TEXTURE_WIDTH, height: int = TEXTURE_HEIGHT) -> bytes:
    raw = b"".join(b"\x00" + row for row in rows)
    chunk = lambda tag, payload: struct.pack(
        ">I", len(payload)
    ) + tag + payload + struct.pack(
        ">I", zlib.crc32(tag + payload) & 0xFFFFFFFF
    )
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(
        ">IIBBBBB", width, height, 8, 2, 0, 0, 0
    )) + chunk(b"IDAT", zlib.compress(raw, level=9)) + chunk(b"IEND", b"")


def _texture_bytes(role: str, material: Mapping[str, object]) -> bytes:
    base = tuple(int(value) for value in material["base"])
    metallic = int(material["metallic"])
    roughness = int(material["roughness"])
    emissive = tuple(int(value) for value in material["emissive"])
    seed = sum(base) + metallic * 3 + roughness * 5
    rows: list[bytes] = []
    for y in range(TEXTURE_HEIGHT):
        row = bytearray()
        for x in range(TEXTURE_WIDTH):
            grain = ((x * 17 + y * 31 + seed) % 19) - 9
            weave = 7 if ((x // 4 + y // 4 + seed) % 2) else -5
            if role == "base_color":
                pixel = (_clamp(base[0] + grain + weave), _clamp(base[1] + grain), _clamp(base[2] + grain - weave))
            elif role == "metallic_roughness":
                pixel = (255, _clamp(roughness + grain), _clamp(metallic + weave))
            elif role == "normal":
                pixel = (_clamp(128 + grain), _clamp(128 + weave), 250)
            elif role == "occlusion":
                pixel = (_clamp(238 - abs(grain) * 2),) * 3
            elif role == "emissive":
                pixel = tuple(_clamp(value + (grain if value else 0)) for value in emissive)
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
    }
    payload["environment_sha256"] = hashlib.sha256(
        json_canonical(payload).encode("utf-8")
    ).hexdigest()
    return payload


def json_canonical(value: object) -> str:
    # Kept local to avoid a geometry-worker import cycle.
    import json

    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
