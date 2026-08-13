#!/usr/bin/env python3
"""Build one deterministic reference/render review sheet for Codex.

This is an evidence/review helper, not a Runtime renderer and not a quality
scorer. It deliberately uses only Python's standard library so a Codex turn
does not need Pillow, NumPy, Playwright, or a network service. The reference
and render PNGs are read from caller-provided paths, resized into a fixed
2x2 sheet, and the output manifest stores hashes rather than local paths or
image bytes.
"""

from __future__ import annotations

import argparse
import binascii
import hashlib
import json
import struct
import zlib
from pathlib import Path
from typing import Iterable


PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
DEFAULT_PASSES = ("beauty", "silhouette", "material-id")
PANEL_ORDER = ("reference", "beauty", "silhouette", "material-id")
LABELS = {
    "reference": "REFERENCE",
    "beauty": "BEAUTY",
    "silhouette": "SILHOUETTE",
    "material-id": "MATERIAL",
}


class SheetError(ValueError):
    """A caller or PNG input violated the bounded sheet contract."""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--render-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--tile-size", type=int, default=512)
    parser.add_argument(
        "--passes",
        nargs="+",
        default=list(DEFAULT_PASSES),
        help="Three render pass names; the output keeps reference in the first panel.",
    )
    return parser.parse_args()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SheetError(message)


def read_png(path: Path) -> tuple[int, int, bytes]:
    """Read a non-interlaced 8-bit RGB/RGBA PNG into RGBA bytes."""

    require(path.is_file() and not path.is_symlink(), f"PNG input is not a regular file: {path}")
    data = path.read_bytes()
    require(data.startswith(PNG_SIGNATURE), f"not a PNG: {path}")
    offset = len(PNG_SIGNATURE)
    width = height = bit_depth = color_type = interlace = None
    idat = bytearray()
    while offset < len(data):
        require(offset + 12 <= len(data), f"truncated PNG chunk: {path}")
        length = struct.unpack(">I", data[offset : offset + 4])[0]
        kind = data[offset + 4 : offset + 8]
        end = offset + 12 + length
        require(end <= len(data), f"PNG chunk exceeds file: {path}")
        payload = data[offset + 8 : offset + 8 + length]
        expected_crc = struct.unpack(">I", data[offset + 8 + length : end])[0]
        require(binascii.crc32(kind + payload) & 0xFFFFFFFF == expected_crc, f"PNG CRC mismatch: {path}")
        if kind == b"IHDR":
            require(len(payload) == 13, f"invalid PNG IHDR: {path}")
            width, height, bit_depth, color_type, compression, filter_method, interlace = struct.unpack(
                ">IIBBBBB", payload
            )
            require(compression == 0 and filter_method == 0, f"unsupported PNG compression/filter: {path}")
        elif kind == b"IDAT":
            idat.extend(payload)
        elif kind == b"IEND":
            break
        offset = end
    require(width is not None and height is not None, f"PNG IHDR missing: {path}")
    require(bit_depth == 8 and color_type in (0, 2, 4, 6) and interlace == 0, f"unsupported PNG profile: {path}")
    channels = {0: 1, 2: 3, 4: 2, 6: 4}[color_type]
    row_bytes = width * channels
    decoded = zlib.decompress(bytes(idat))
    expected = height * (row_bytes + 1)
    require(len(decoded) == expected, f"PNG scanline size mismatch: {path}")
    rows: list[bytearray] = []
    cursor = 0
    previous = bytearray(row_bytes)
    for _ in range(height):
        filter_type = decoded[cursor]
        cursor += 1
        current = bytearray(decoded[cursor : cursor + row_bytes])
        cursor += row_bytes
        for index in range(row_bytes):
            left = current[index - channels] if index >= channels else 0
            up = previous[index]
            up_left = previous[index - channels] if index >= channels else 0
            if filter_type == 1:
                current[index] = (current[index] + left) & 0xFF
            elif filter_type == 2:
                current[index] = (current[index] + up) & 0xFF
            elif filter_type == 3:
                current[index] = (current[index] + ((left + up) // 2)) & 0xFF
            elif filter_type == 4:
                predictor = left + up - up_left
                distance_left = abs(predictor - left)
                distance_up = abs(predictor - up)
                distance_up_left = abs(predictor - up_left)
                nearest = left
                if distance_up < distance_left and distance_up <= distance_up_left:
                    nearest = up
                elif distance_up_left < distance_left and distance_up_left < distance_up:
                    nearest = up_left
                current[index] = (current[index] + nearest) & 0xFF
            elif filter_type != 0:
                raise SheetError(f"unsupported PNG filter {filter_type}: {path}")
        rows.append(current)
        previous = current
    rgba = bytearray(width * height * 4)
    out = 0
    for row in rows:
        for index in range(0, row_bytes, channels):
            if channels == 1:
                rgba[out : out + 3] = row[index : index + 1] * 3
                rgba[out + 3] = 255
            elif channels == 2:
                rgba[out : out + 3] = row[index : index + 1] * 3
                rgba[out + 3] = row[index + 1]
            else:
                rgba[out : out + 3] = row[index : index + 3]
                rgba[out + 3] = row[index + 3] if channels == 4 else 255
            out += 4
    return int(width), int(height), bytes(rgba)


def fit_to_tile(width: int, height: int, pixels: bytes, tile_size: int) -> bytes:
    """Letterbox-resize RGBA pixels with deterministic nearest sampling."""

    require(width > 0 and height > 0, "PNG dimensions must be positive")
    scale = min(tile_size / width, tile_size / height)
    scaled_width = max(1, min(tile_size, round(width * scale)))
    scaled_height = max(1, min(tile_size, round(height * scale)))
    canvas = bytearray(tile_size * tile_size * 4)
    for index in range(0, len(canvas), 4):
        canvas[index : index + 4] = b"\x0b\x0f\x16\xff"
    x_offset = (tile_size - scaled_width) // 2
    y_offset = (tile_size - scaled_height) // 2
    for y in range(scaled_height):
        source_y = min(height - 1, int((y + 0.5) * height / scaled_height))
        for x in range(scaled_width):
            source_x = min(width - 1, int((x + 0.5) * width / scaled_width))
            source = (source_y * width + source_x) * 4
            target = ((y + y_offset) * tile_size + x + x_offset) * 4
            canvas[target : target + 4] = pixels[source : source + 4]
    return bytes(canvas)


FONT = {
    "A": ("01110", "10001", "10001", "11111", "10001", "10001", "10001"),
    "B": ("11110", "10001", "10001", "11110", "10001", "10001", "11110"),
    "C": ("01111", "10000", "10000", "10000", "10000", "10000", "01111"),
    "D": ("11110", "10001", "10001", "10001", "10001", "10001", "11110"),
    "E": ("11111", "10000", "10000", "11110", "10000", "10000", "11111"),
    "F": ("11111", "10000", "10000", "11110", "10000", "10000", "10000"),
    "H": ("10001", "10001", "10001", "11111", "10001", "10001", "10001"),
    "I": ("11111", "00100", "00100", "00100", "00100", "00100", "11111"),
    "L": ("10000", "10000", "10000", "10000", "10000", "10000", "11111"),
    "M": ("10001", "11011", "10101", "10101", "10001", "10001", "10001"),
    "N": ("10001", "11001", "10101", "10011", "10001", "10001", "10001"),
    "O": ("01110", "10001", "10001", "10001", "10001", "10001", "01110"),
    "R": ("11110", "10001", "10001", "11110", "10100", "10010", "10001"),
    "S": ("01111", "10000", "10000", "01110", "00001", "00001", "11110"),
    "T": ("11111", "00100", "00100", "00100", "00100", "00100", "00100"),
    "U": ("10001", "10001", "10001", "10001", "10001", "10001", "01110"),
    "Y": ("10001", "10001", "01010", "00100", "00100", "00100", "00100"),
}


def draw_label(canvas: bytearray, tile_size: int, label: str) -> None:
    """Draw a tiny white 5x7 label in the tile's top-left corner."""

    scale = 2
    cursor_x = 10
    cursor_y = 8
    for char in label:
        glyph = FONT.get(char)
        if glyph is None:
            cursor_x += 6 * scale
            continue
        for row, bits in enumerate(glyph):
            for column, bit in enumerate(bits):
                if bit != "1":
                    continue
                for dy in range(scale):
                    for dx in range(scale):
                        x = cursor_x + column * scale + dx
                        y = cursor_y + row * scale + dy
                        if 0 <= x < tile_size and 0 <= y < tile_size:
                            offset = (y * tile_size + x) * 4
                            canvas[offset : offset + 4] = b"\xff\xff\xff\xff"
        cursor_x += 6 * scale


def compose_sheet(tiles: dict[str, bytes], tile_size: int) -> bytes:
    panel_height = tile_size + 28
    sheet = bytearray(tile_size * 2 * panel_height * 2 * 4)
    for index, name in enumerate(PANEL_ORDER):
        tile = bytearray(tiles[name])
        draw_label(tile, tile_size, LABELS[name])
        panel_x = (index % 2) * tile_size
        panel_y = (index // 2) * panel_height
        for y in range(tile_size):
            target = ((panel_y + 28 + y) * tile_size * 2 + panel_x) * 4
            source = y * tile_size * 4
            sheet[target : target + tile_size * 4] = tile[source : source + tile_size * 4]
        for y in range(panel_y, panel_y + 28):
            target = (y * tile_size * 2 + panel_x) * 4
            sheet[target : target + tile_size * 4] = b"\x18\x22\x2e\xff" * tile_size
    return bytes(sheet)


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", binascii.crc32(kind + payload) & 0xFFFFFFFF)


def write_png(path: Path, width: int, height: int, rgba: bytes) -> None:
    require(len(rgba) == width * height * 4, "RGBA buffer size does not match output dimensions")
    scanlines = bytearray()
    row_bytes = width * 4
    for row in range(height):
        scanlines.append(0)
        start = row * row_bytes
        scanlines.extend(rgba[start : start + row_bytes])
    payload = PNG_SIGNATURE
    payload += png_chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    payload += png_chunk(b"IDAT", zlib.compress(bytes(scanlines), level=9))
    payload += png_chunk(b"IEND", b"")
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    path.write_bytes(payload)


def main() -> int:
    args = parse_args()
    require(128 <= args.tile_size <= 1024, "tile-size must be between 128 and 1024")
    require(len(args.passes) == 3, "exactly three render passes are required")
    require(len(set(args.passes)) == 3, "render passes must be unique")
    require(args.passes[0] == "beauty" and args.passes[1] == "silhouette", "passes must start with beauty silhouette")
    reference = args.reference.expanduser().resolve()
    render_dir = args.render_dir.expanduser().resolve()
    output = args.output.expanduser().resolve()
    manifest_path = args.manifest.expanduser().resolve()
    require(not output.is_symlink() and not manifest_path.is_symlink(), "output paths must not be symlinks")
    ref_width, ref_height, ref_pixels = read_png(reference)
    tiles = {"reference": fit_to_tile(ref_width, ref_height, ref_pixels, args.tile_size)}
    panel_hashes = {"reference": sha256_bytes(reference.read_bytes())}
    for pass_name in args.passes:
        source = render_dir / f"{pass_name}.png"
        width, height, pixels = read_png(source)
        tiles[pass_name] = fit_to_tile(width, height, pixels, args.tile_size)
        panel_hashes[pass_name] = sha256_bytes(source.read_bytes())
    # The sheet always has the same four semantic panels; the third pass can
    # be any persisted AOV, but its title is derived from the requested name.
    tiles["material-id"] = tiles.pop(args.passes[2])
    LABELS["material-id"] = args.passes[2].upper().replace("-", " ")
    sheet_width = args.tile_size * 2
    sheet_height = (args.tile_size + 28) * 2
    sheet = compose_sheet(tiles, args.tile_size)
    write_png(output, sheet_width, sheet_height, sheet)
    manifest = {
        "schema_version": "ForgeCADMCP010FComparisonSheet@1",
        "status": "PASS",
        "sheet_sha256": sha256_bytes(output.read_bytes()),
        "sheet_dimensions": {"width": sheet_width, "height": sheet_height},
        "tile_size": args.tile_size,
        "panel_order": ["reference", "beauty", "silhouette", args.passes[2]],
        "panel_sha256": {"reference": panel_hashes["reference"], "beauty": panel_hashes["beauty"], "silhouette": panel_hashes["silhouette"], args.passes[2]: panel_hashes[args.passes[2]]},
        "quality_authority": "Runtime QualityReport metrics; sheet is review aid only",
        "persistent_user_data_touched": False,
    }
    manifest_path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(manifest, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
