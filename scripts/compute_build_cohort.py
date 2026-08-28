#!/usr/bin/env python3
"""Compute the deterministic source cohort used by local ForgeCAD builds.

The cohort intentionally covers the source inputs that are shipped or compiled
into the local Runtime, MCP, and worker components. Build outputs and other
generated directories are excluded so rerunning the command for the same source
tree produces the same value.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
EXCLUDED_PARTS = {".git", "target", "node_modules", "dist", "__pycache__"}
SOURCE_INPUTS = (
    ROOT / "apps" / "desktop" / "src",
    ROOT / "apps" / "desktop" / "package.json",
    ROOT / "apps" / "desktop" / "src-tauri",
    ROOT / "apps" / "geometry-worker" / "Cargo.toml",
    ROOT / "apps" / "geometry-worker" / "Cargo.lock",
    ROOT / "apps" / "geometry-worker" / "src",
    ROOT / "apps" / "render-worker" / "Cargo.toml",
    ROOT / "apps" / "render-worker" / "Cargo.lock",
    ROOT / "apps" / "render-worker" / "src",
    ROOT / "packages" / "forgecad-contracts",
    ROOT / "packages" / "forgecad-skills",
    ROOT / "package.json",
    ROOT / "package-lock.json",
    ROOT / "script" / "with_rust_toolchain.sh",
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def source_files() -> list[Path]:
    files: set[Path] = set()
    for source in SOURCE_INPUTS:
        if not source.exists():
            continue
        candidates = [source] if source.is_file() else source.rglob("*")
        for candidate in candidates:
            if any(part in EXCLUDED_PARTS for part in candidate.parts):
                continue
            if candidate.is_symlink():
                raise SystemExit(f"source cohort refuses symlink: {candidate.relative_to(ROOT)}")
            if candidate.is_file():
                files.add(candidate)
    return sorted(files, key=lambda path: path.relative_to(ROOT).as_posix())


def source_cohort() -> tuple[str, int]:
    digest = hashlib.sha256()
    files = source_files()
    for path in files:
        relative = path.relative_to(ROOT).as_posix().encode("utf-8")
        digest.update(len(relative).to_bytes(4, "big"))
        digest.update(relative)
        digest.update(bytes.fromhex(sha256_file(path)))
    return digest.hexdigest(), len(files)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--json",
        action="store_true",
        help="Print the cohort and source file count as a JSON object.",
    )
    args = parser.parse_args()
    cohort, file_count = source_cohort()
    if args.json:
        print(json.dumps({"build_cohort_sha256": cohort, "source_file_count": file_count}))
    else:
        print(cohort)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
