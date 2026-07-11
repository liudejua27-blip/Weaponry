#!/usr/bin/env python3
"""Validate the non-approving Markdown handoff for an integrity-locked review draft."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

from formal_module_review import generate_review_draft
from smoke_formal_module_review import _create_candidate_pack


ROOT = Path(__file__).resolve().parents[1]
HANDOFF_SCRIPT = ROOT / "scripts" / "render_formal_module_review_handoff.py"


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad_review_handoff_") as temporary:
        root = Path(temporary)
        pack_root = root / "candidate-pack"
        source_root = root / "candidate-sources"
        _create_candidate_pack(pack_root, source_root, formal_like=True)
        review = root / "review.json"
        generate_review_draft(pack_root, source_root, review, scope="first_three")
        output = root / "review-handoff.md"

        created = _run(pack_root, source_root, review, output)
        _assert(created.returncode == 0, created.stderr or created.stdout)
        report = json.loads(created.stdout)
        _assert(report["approval_granted"] is False, "handoff grants approval")
        text = output.read_text(encoding="utf-8")
        _assert("DRAFT ONLY" in text, "handoff draft boundary missing")
        _assert("module_core_shell_01" in text, "module checklist missing")
        _assert(str(root) not in text, "handoff leaked an absolute path")

        overwrite = _run(pack_root, source_root, review, output)
        _assert(overwrite.returncode == 1, "handoff overwrote an existing file")
        _assert(
            "must not overwrite" in overwrite.stdout, "overwrite diagnostic mismatch"
        )

        (source_root / "module_front_shell_01.blend").write_bytes(b"stale")
        stale = _run(pack_root, source_root, review, root / "stale-handoff.md")
        _assert(stale.returncode == 1, "stale source was accepted")
        _assert(
            "review_handoff_failed" in stale.stdout, "stale source diagnostic missing"
        )

    print(
        json.dumps(
            {
                "ok": True,
                "draft_boundary_verified": True,
                "absolute_paths_excluded": True,
                "overwrite_rejected": True,
                "stale_artifact_rejected": True,
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


def _run(
    pack_root: Path, source_root: Path, review: Path, output: Path
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(HANDOFF_SCRIPT),
            "--pack-root",
            str(pack_root),
            "--source-root",
            str(source_root),
            "--review",
            str(review),
            "--output",
            str(output),
        ],
        cwd=ROOT,
        env={**__import__("os").environ, "PYTHONPATH": "apps/agent:scripts"},
        text=True,
        capture_output=True,
        check=False,
    )


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    raise SystemExit(main())
