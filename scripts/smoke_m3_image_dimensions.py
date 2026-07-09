#!/usr/bin/env python3
"""Smoke checks for image header dimension parsing."""

from __future__ import annotations

import json
import struct
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))

from wushen_agent.providers.image import image_dimensions  # noqa: E402


def main() -> int:
    cases = {
        "png": (
            b"\x89PNG\r\n\x1a\n"
            + struct.pack(">I", 13)
            + b"IHDR"
            + struct.pack(">II", 64, 32)
            + b"\x08\x06\x00\x00\x00"
            + b"\x00\x00\x00\x00"
        ),
        "jpeg": b"\xff\xd8\xff\xc0\x00\x11\x08\x00\x18\x00\x28\x03\x01\x11\x00\x02\x11\x00\x03\x11\x00\xff\xd9",
        "webp_vp8x": b"RIFF" + struct.pack("<I", 22) + b"WEBPVP8X" + struct.pack("<I", 10) + b"\x00\x00\x00\x00" + (79).to_bytes(3, "little") + (44).to_bytes(3, "little"),
    }
    assert image_dimensions(cases["png"], mime_type="image/png", filename="x.png") == (64, 32)
    assert image_dimensions(cases["jpeg"], mime_type="image/jpeg", filename="x.jpg") == (40, 24)
    assert image_dimensions(cases["webp_vp8x"], mime_type="image/webp", filename="x.webp") == (80, 45)
    print(json.dumps({"ok": True, "formats": sorted(cases)}, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
