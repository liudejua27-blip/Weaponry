#!/usr/bin/env python3
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / "scripts" / "export_blender_starter_pack.py"
MODULE_IDS = (
    "module_core_shell_01",
    "module_front_shell_01",
    "module_front_shell_02",
)


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad_blender_reexport_") as temporary:
        temp_root = Path(temporary)
        source_root = temp_root / "sources"
        output_root = temp_root / "exports"
        source_root.mkdir()
        for module_id in MODULE_IDS:
            (source_root / f"{module_id}.blend").write_bytes(b"BLENDER-v-test-fixture")

        ready = _run("--source-root", str(source_root), "--output-root", str(output_root))
        _assert(ready.returncode == 0, ready.stderr or ready.stdout)
        ready_report = json.loads(ready.stdout)
        _assert(ready_report["sources_ready"] is True, "valid source headers were rejected")
        blender_ready = ready_report["blender_ready"] is True
        expected_status = (
            "ready_for_read_only_export" if blender_ready else "blocked_blender_not_configured"
        )
        _assert(ready_report["status"] == expected_status, "preflight readiness is inconsistent")

        overlap = _run("--source-root", str(source_root), "--output-root", str(source_root))
        _assert(overlap.returncode == 1, "source/output overlap was accepted")
        _assert(
            json.loads(overlap.stdout)["status"] == "source_output_overlap_denied",
            "source/output overlap returned the wrong diagnostic",
        )

        committed = _run(
            "--source-root",
            str(source_root),
            "--output-root",
            str(ROOT / "assets" / "module-packs" / "weapon-concept-v1-reference"),
        )
        _assert(committed.returncode == 1, "committed pack output was accepted")
        _assert(
            json.loads(committed.stdout)["status"] == "committed_pack_output_denied",
            "committed pack output returned the wrong diagnostic",
        )

        invalid_source = temp_root / "invalid-sources"
        invalid_source.mkdir()
        for module_id in MODULE_IDS:
            (invalid_source / f"{module_id}.blend").write_bytes(b"not-a-blender-file")
        invalid = _run(
            "--source-root",
            str(invalid_source),
            "--output-root",
            str(output_root),
        )
        _assert(invalid.returncode == 0, "preflight diagnostics should remain inspectable")
        invalid_report = json.loads(invalid.stdout)
        _assert(invalid_report["sources_ready"] is False, "invalid Blender headers passed")
        _assert(
            len(invalid_report["source_errors"]) == len(MODULE_IDS),
            "invalid header diagnostics were incomplete",
        )

        if not blender_ready:
            execute = _run(
                "--source-root",
                str(source_root),
                "--output-root",
                str(output_root),
                "--execute",
                "--require-blender",
            )
            _assert(execute.returncode == 1, "execute succeeded without Blender")
            _assert(
                json.loads(execute.stdout)["status"] == "blender_not_configured",
                "missing Blender returned the wrong execute diagnostic",
            )

    print(
        json.dumps(
            {
                "ok": True,
                "read_only_source_contract": True,
                "valid_blend_headers_detected": True,
                "invalid_blend_headers_rejected": True,
                "source_output_overlap_rejected": True,
                "committed_pack_output_rejected": True,
                "execute_without_blender_rejected": not blender_ready,
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


def _run(*arguments: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(RUNNER), *arguments],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AssertionError, OSError, json.JSONDecodeError) as exc:
        raise SystemExit(f"Blender re-export preflight smoke failed: {exc}") from exc
