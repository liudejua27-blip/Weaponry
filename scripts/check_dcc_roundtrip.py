#!/usr/bin/env python3
"""Preflight or execute a read-only GLB round-trip through Blender/Assimp."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path

from forgecad_agent.application.combined_glb import CombinedGlbError, read_glb
from forgecad_agent.application.combined_obj import CombinedObjError, build_combined_obj


ROOT = Path(__file__).resolve().parents[1]
BLENDER_SCRIPT = ROOT / "scripts" / "blender" / "roundtrip_glb.py"
DEFAULT_OUTPUT_ROOT = ROOT / "output" / "dcc-roundtrip"
COMMITTED_PACK_ROOT = ROOT / "assets" / "module-packs"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input-glb", type=Path)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--blender-executable", type=Path)
    parser.add_argument("--assimp-executable", type=Path)
    parser.add_argument("--require-dcc", action="store_true")
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    blender = _find_blender(args.blender_executable)
    assimp = _find_assimp(args.assimp_executable)
    available = {
        "blender": str(blender) if blender else None,
        "assimp": str(assimp) if assimp else None,
    }
    if blender is None and assimp is None:
        return _report(
            {
                "ok": not args.require_dcc,
                "status": "blocked_dcc_not_configured",
                "roundtrip_validated": False,
                "tools": available,
                "resolution": (
                    "Install Blender or Assimp and configure FORGECAD_BLENDER_EXECUTABLE "
                    "or FORGECAD_ASSIMP_EXECUTABLE."
                ),
            },
            1 if args.require_dcc else 0,
        )

    if args.input_glb is None:
        return _report(
            {
                "ok": True,
                "status": "ready_for_dcc_roundtrip",
                "roundtrip_validated": False,
                "tools": available,
                "next": "Pass --input-glb with an immutable combined GLB to execute the gate.",
            },
            0,
        )

    source = args.input_glb.expanduser().resolve()
    if not source.is_file():
        return _report(
            {"ok": False, "status": "input_glb_missing", "input_glb": str(source)}, 1
        )
    output_root = args.output_root.expanduser().resolve()
    if (
        source == output_root
        or output_root.is_relative_to(source)
        or output_root.is_relative_to(COMMITTED_PACK_ROOT)
    ):
        return _report(
            {
                "ok": False,
                "status": "unsafe_output_path",
                "input_glb": str(source),
                "output_root": str(output_root),
            },
            1,
        )
    if output_root.exists() and any(output_root.iterdir()) and not args.force:
        return _report(
            {
                "ok": False,
                "status": "output_not_empty",
                "output_root": str(output_root),
                "resolution": "Use an empty output directory or pass --force.",
            },
            1,
        )
    if args.force and output_root.exists():
        shutil.rmtree(output_root)
    output_root.mkdir(parents=True, exist_ok=True)

    source_sha256 = _sha256(source)
    try:
        source_metrics = _glb_metrics(source.read_bytes())
    except (CombinedGlbError, CombinedObjError, KeyError, ValueError) as exc:
        return _report(
            {"ok": False, "status": "input_glb_invalid", "message": str(exc)}, 1
        )

    tool_name = "blender" if blender is not None else "assimp"
    output = output_root / f"roundtrip-{tool_name}.glb"
    command = (
        [
            str(blender),
            "--background",
            "--python-exit-code",
            "1",
            "--factory-startup",
            "--python",
            str(BLENDER_SCRIPT),
            "--",
            "--input",
            str(source),
            "--output",
            str(output),
        ]
        if blender is not None
        else [str(assimp), "export", str(source), str(output)]
    )
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        timeout=300,
    )
    if completed.returncode != 0 or not output.is_file():
        return _report(
            {
                "ok": False,
                "status": "dcc_roundtrip_failed",
                "tool": tool_name,
                "returncode": completed.returncode,
                "stdout_tail": completed.stdout[-2000:],
                "stderr_tail": completed.stderr[-2000:],
            },
            1,
        )
    if _sha256(source) != source_sha256:
        return _report(
            {"ok": False, "status": "source_glb_modified", "input_glb": str(source)}, 1
        )
    try:
        output_metrics = _glb_metrics(output.read_bytes())
    except (CombinedGlbError, CombinedObjError, KeyError, ValueError) as exc:
        return _report(
            {"ok": False, "status": "roundtrip_glb_invalid", "message": str(exc)}, 1
        )
    if output_metrics != source_metrics:
        return _report(
            {
                "ok": False,
                "status": "roundtrip_geometry_mismatch",
                "tool": tool_name,
                "source_metrics": source_metrics,
                "output_metrics": output_metrics,
            },
            1,
        )
    return _report(
        {
            "ok": True,
            "status": "dcc_roundtrip_validated",
            "roundtrip_validated": True,
            "tool": tool_name,
            "input_glb": str(source),
            "source_sha256": source_sha256,
            "output_glb": str(output),
            "output_sha256": _sha256(output),
            "geometry_metrics": source_metrics,
        },
        0,
    )


def _glb_metrics(payload: bytes) -> dict[str, int]:
    document, _ = read_glb(payload)
    if document.get("asset", {}).get("version") != "2.0":
        raise ValueError("GLB asset.version must be 2.0")
    flattened = build_combined_obj(payload)
    return {
        "vertex_count": flattened.vertex_count,
        "triangle_count": flattened.triangle_count,
    }


def _find_blender(explicit: Path | None) -> Path | None:
    return _find_executable(
        explicit,
        ("FORGECAD_BLENDER_EXECUTABLE", "BLENDER_EXECUTABLE"),
        "blender",
        (
            Path("/Applications/Blender.app/Contents/MacOS/Blender"),
            Path.home() / "Applications/Blender.app/Contents/MacOS/Blender",
        ),
    )


def _find_assimp(explicit: Path | None) -> Path | None:
    return _find_executable(
        explicit,
        ("FORGECAD_ASSIMP_EXECUTABLE", "ASSIMP_EXECUTABLE"),
        "assimp",
        (Path("/opt/homebrew/bin/assimp"), Path("/usr/local/bin/assimp")),
    )


def _find_executable(
    explicit: Path | None,
    environment_names: tuple[str, ...],
    command_name: str,
    defaults: tuple[Path, ...],
) -> Path | None:
    candidates: list[Path] = []
    if explicit is not None:
        candidates.append(explicit.expanduser())
    for name in environment_names:
        if os.environ.get(name):
            candidates.append(Path(os.environ[name]).expanduser())
    discovered = shutil.which(command_name)
    if discovered:
        candidates.append(Path(discovered))
    candidates.extend(defaults)
    for candidate in candidates:
        resolved = candidate.resolve()
        if resolved.is_file() and os.access(resolved, os.X_OK):
            return resolved
    return None


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _report(payload: dict[str, object], exit_code: int) -> int:
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return exit_code


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, subprocess.SubprocessError) as exc:
        print(
            json.dumps(
                {"ok": False, "status": "dcc_preflight_failed", "message": str(exc)},
                ensure_ascii=False,
                indent=2,
            )
        )
        raise SystemExit(1) from exc
