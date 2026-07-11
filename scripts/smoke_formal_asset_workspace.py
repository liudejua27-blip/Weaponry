#!/usr/bin/env python3
"""Smoke-test the non-promoting formal asset workspace scaffold."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCAFFOLD = ROOT / "scripts" / "scaffold_formal_asset_workspace.py"
REFERENCE_PACK = ROOT / "assets" / "module-packs" / "weapon-concept-v1-reference"


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad_formal_workspace_") as temporary:
        root = Path(temporary)
        sources = root / "sources"
        sources.mkdir()
        pack = json.loads((REFERENCE_PACK / "pack.json").read_text(encoding="utf-8"))
        for entry in pack["modules"]:
            (sources / f"{entry['module_id']}.blend").write_bytes(b"BLENDER-v420")
        output = root / "workspace"
        created = _run(REFERENCE_PACK, sources, output)
        _assert(created.returncode == 0, created.stderr or created.stdout)
        report = json.loads(created.stdout)
        _assert(report["promotion_granted"] is False, "scaffold granted promotion")
        _assert(report["module_count"] == 10, "scaffold module count mismatch")
        _assert((output / "sources").is_dir(), "sources were not copied")
        _assert((output / "staging-pack" / "pack.json").is_file(), "Pack was not copied")
        _assert(
            "final-art declaration" in (output / "LICENSE_DECISION_REQUIRED.md").read_text(),
            "license boundary template missing",
        )
        manifest = (output / "INPUT_MANIFEST.json").read_text(encoding="utf-8")
        _assert(str(root) not in manifest, "input manifest leaked absolute paths")

        overwrite = _run(REFERENCE_PACK, sources, output)
        _assert(overwrite.returncode == 1, "scaffold overwrote an existing workspace")
        _assert("must not already exist" in overwrite.stderr, "overwrite guard missing")

    print(
        json.dumps(
            {
                "ok": True,
                "release_shaped_workspace_created": True,
                "promotion_boundary_verified": True,
                "absolute_paths_excluded": True,
                "overwrite_rejected": True,
            },
            indent=2,
        )
    )
    return 0


def _run(pack: Path, sources: Path, output: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(SCAFFOLD),
            "--pack-root",
            str(pack),
            "--source-root",
            str(sources),
            "--output-root",
            str(output),
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    raise SystemExit(main())
