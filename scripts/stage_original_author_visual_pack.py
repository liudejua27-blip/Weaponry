#!/usr/bin/env python3
"""Stage a local Blender visual Pack as self-declared original art.

This is deliberately a staging tool, not a promotion tool.  It converts a
validated Blender candidate into a local Pack with the author's declared
license, while keeping independent review pending in the product metadata.
It never creates a FormalModulePromotionReport or marks assets approved.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import uuid
from pathlib import Path

from concept_module_pack import validate_module_pack


ROOT = Path(__file__).resolve().parents[1]
COMMITTED_PACK_ROOT = ROOT / "assets" / "module-packs"
ORIGINAL_AUTHOR_LICENSE = "LicenseRef-ForgeCAD-Original-Author"
BLENDER_HEADER = b"BLENDER"
ZSTD_MAGIC = b"\x28\xb5\x2f\xfd"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Stage a local original-author Pack without granting review approval."
    )
    parser.add_argument("--candidate-root", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    args = parser.parse_args()

    candidate = args.candidate_root.expanduser().resolve()
    output = args.output_root.expanduser()
    if not output.is_absolute():
        raise ValueError("--output-root must be an absolute path")
    output = output.resolve()
    if output.exists() or output.is_symlink():
        raise ValueError("--output-root must not already exist")
    if output.is_relative_to(COMMITTED_PACK_ROOT):
        raise ValueError("--output-root cannot be inside a committed Module Pack")
    if candidate == output or output.is_relative_to(candidate):
        raise ValueError("--output-root must be outside --candidate-root")

    pack = validate_module_pack(candidate, release=True)
    if not 10 <= len(pack.modules) <= 12:
        raise ValueError("original-author staging requires 10-12 modules")
    sources = candidate / "sources"
    _validate_sources(sources, pack)
    _reject_symlinks(candidate)

    temporary = output.parent / f".{output.name}.staging-{uuid.uuid4().hex}"
    try:
        shutil.copytree(candidate, temporary)
        _write_original_author_declaration(temporary, pack)
        validate_module_pack(temporary, release=True)
        output.parent.mkdir(parents=True, exist_ok=True)
        os.replace(temporary, output)
    except Exception:
        shutil.rmtree(temporary, ignore_errors=True)
        raise

    print(
        json.dumps(
            {
                "ok": True,
                "status": "staged_original_author_pending_review",
                "pack_root": str(output),
                "module_count": len(pack.modules),
                "origin_claim": "self_declared_original",
                "review_status": "pending_review",
                "promotion_granted": False,
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


def _validate_sources(sources: Path, pack) -> None:
    expected = {f"{module.manifest.module_id}.blend" for module in pack.modules}
    actual = {path.name for path in sources.glob("*.blend")} if sources.is_dir() else set()
    if actual != expected:
        raise ValueError(f"source set mismatch: expected {sorted(expected)}, found {sorted(actual)}")
    for path in sources.glob("*.blend"):
        # Blender 5 may write compressed .blend files with a Zstandard stream
        # header instead of exposing the historical ``BLENDER`` bytes first.
        # Both formats remain editable source; the Pack itself is separately
        # validated before staging.
        if not path.is_file() or path.is_symlink():
            raise ValueError(f"invalid Blender source: {path.name}")
        header = path.read_bytes()[:7]
        if not (
            header.startswith(BLENDER_HEADER) or header.startswith(ZSTD_MAGIC)
        ):
            raise ValueError(f"invalid Blender source: {path.name}")


def _reject_symlinks(root: Path) -> None:
    if any(path.is_symlink() for path in root.rglob("*")):
        raise ValueError("candidate Pack cannot contain symlinks")


def _write_original_author_declaration(root: Path, pack) -> None:
    pack_path = root / "pack.json"
    raw = json.loads(pack_path.read_text(encoding="utf-8"))
    raw["name"] = "Weapon Concept v1 original-author visual Pack"
    raw["version"] = "1.1.0"
    raw["description"] = (
        "Self-declared original non-functional concept/game/film-prop/display "
        "asset Pack. Independent visual review remains pending."
    )
    raw["license"] = {
        "spdx_expression": ORIGINAL_AUTHOR_LICENSE,
        "license_path": "LICENSES/PACK.txt",
    }
    pack_path.write_text(json.dumps(raw, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    license_text = (
        f"SPDX-License-Identifier: {ORIGINAL_AUTHOR_LICENSE}\n"
        "Self-declared original non-functional concept, game asset, film prop, or display asset.\n"
        "Independent visual review is pending. This is not manufacturing, structural, or safety documentation.\n"
    )
    (root / "LICENSES" / "PACK.txt").write_text(license_text, encoding="utf-8")
    for entry in pack.manifest.modules:
        (root / entry.license_path).write_text(license_text, encoding="utf-8")


if __name__ == "__main__":
    raise SystemExit(main())
