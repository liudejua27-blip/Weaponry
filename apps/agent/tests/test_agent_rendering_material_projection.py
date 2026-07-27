from __future__ import annotations

import struct
import zlib

import pytest

from forgecad_agent.application.agent_rendering import _parse_mtl
from forgecad_agent.application.combined_obj import _png_average_linear_rgb


def _rgba_png(red: int, green: int, blue: int, alpha: int = 255) -> bytes:
    signature = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0)

    def chunk(kind: bytes, payload: bytes) -> bytes:
        return (
            struct.pack(">I", len(payload))
            + kind
            + payload
            + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)
        )

    scanline = bytes((0, red, green, blue, alpha))
    return (
        signature
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(scanline))
        + chunk(b"IEND", b"")
    )


def test_embedded_base_color_png_projects_to_linear_material_color() -> None:
    color = _png_average_linear_rgb(_rgba_png(32, 96, 224))
    assert color is not None
    assert color[0] == pytest.approx(0.01444, abs=0.0001)
    assert color[1] == pytest.approx(0.11697, abs=0.0001)
    assert color[2] == pytest.approx(0.74540, abs=0.0001)


def test_software_renderer_projects_emissive_energy_into_concept_color() -> None:
    materials = _parse_mtl(
        "\n".join(
            (
                "newmtl mat_blue",
                "Kd 0.01 0.04 0.20",
                "Ke 0.00 0.10 0.60",
                "d 1.0",
            )
        )
    )
    assert materials["mat_blue"][0] == pytest.approx(0.01)
    assert materials["mat_blue"][1] == pytest.approx(0.12)
    assert materials["mat_blue"][2] == pytest.approx(0.68)
    assert materials["mat_blue"][3] == pytest.approx(1.0)
