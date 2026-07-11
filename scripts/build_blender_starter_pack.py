#!/usr/bin/env python3
"""Run the three-module Blender authoring starter and validate its exported pack."""

from __future__ import annotations

import argparse
import json
import os
import py_compile
import shutil
import subprocess
from pathlib import Path

from concept_module_pack import ModulePackValidationError, validate_module_pack


ROOT = Path(__file__).resolve().parents[1]
AUTHORING_SCRIPT = ROOT / "scripts" / "blender" / "weapon_concept_starter.py"
DEFAULT_OUTPUT_ROOT = ROOT / "output" / "blender" / "weapon-concept-v1-starter"
COMMITTED_PACK_ROOT = ROOT / "assets" / "module-packs"
REQUIRED_MODULE_IDS = (
    "module_core_shell_01",
    "module_front_shell_01",
    "module_front_shell_02",
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--blender-executable", type=Path)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--require-blender", action="store_true")
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    source_check = _check_authoring_source()
    output_root = args.output_root.expanduser().resolve()
    if output_root.is_relative_to(COMMITTED_PACK_ROOT):
        print(
            json.dumps(
                {
                    "ok": False,
                    "status": "committed_pack_output_denied",
                    "output_root": str(output_root),
                },
                ensure_ascii=False,
                indent=2,
            )
        )
        return 1
    if output_root.exists() and any(output_root.iterdir()) and not args.force:
        print(
            json.dumps(
                {
                    "ok": False,
                    "status": "output_not_empty",
                    "output_root": str(output_root),
                    "resolution": "Choose an empty directory or pass --force for a deliberate rebuild.",
                },
                ensure_ascii=False,
                indent=2,
            )
        )
        return 1
    blender = _find_blender(args.blender_executable)
    if blender is None:
        report = {
            "ok": not args.require_blender,
            "status": "blocked_blender_not_configured",
            "build_ready": False,
            "source_check": source_check,
            "required_module_ids": list(REQUIRED_MODULE_IDS),
            "resolution": (
                "Install Blender and set FORGECAD_BLENDER_EXECUTABLE, or pass "
                "--blender-executable."
            ),
        }
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 1 if args.require_blender else 0

    command = [
        str(blender),
        "--background",
        "--python-exit-code",
        "1",
        "--factory-startup",
        "--python",
        str(AUTHORING_SCRIPT),
        "--",
        "--output-root",
        str(output_root),
    ]
    if args.force:
        command.append("--force")
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        print(
            json.dumps(
                {
                    "ok": False,
                    "status": "blender_build_failed",
                    "command": command,
                    "returncode": completed.returncode,
                    "stdout_tail": completed.stdout[-4000:],
                    "stderr_tail": completed.stderr[-4000:],
                },
                ensure_ascii=False,
                indent=2,
            )
        )
        return 1

    try:
        validated = validate_module_pack(output_root, release=False)
    except ModulePackValidationError as exc:
        print(
            json.dumps(
                {
                    "ok": False,
                    "status": "export_validation_failed",
                    "errors": exc.errors,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
        return 1

    actual_ids = tuple(module.manifest.module_id for module in validated.modules)
    if actual_ids != REQUIRED_MODULE_IDS:
        print(
            json.dumps(
                {
                    "ok": False,
                    "status": "starter_module_set_mismatch",
                    "expected": list(REQUIRED_MODULE_IDS),
                    "actual": list(actual_ids),
                },
                ensure_ascii=False,
                indent=2,
            )
        )
        return 1

    sources = [output_root / "sources" / f"{module_id}.blend" for module_id in actual_ids]
    missing_sources = [str(path) for path in sources if not path.is_file()]
    if missing_sources:
        print(
            json.dumps(
                {
                    "ok": False,
                    "status": "blend_sources_missing",
                    "missing": missing_sources,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
        return 1

    print(
        json.dumps(
            {
                "ok": True,
                "status": "built_and_validated",
                "build_ready": True,
                "blender_executable": str(blender),
                "output_root": str(output_root),
                "module_ids": list(actual_ids),
                "warnings": list(validated.warnings),
                "blend_sources": [str(path) for path in sources],
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


def _check_authoring_source() -> dict[str, object]:
    py_compile.compile(str(AUTHORING_SCRIPT), doraise=True)
    source = AUTHORING_SCRIPT.read_text(encoding="utf-8")
    missing_ids = [module_id for module_id in REQUIRED_MODULE_IDS if module_id not in source]
    required_tokens = (
        "bpy.ops.wm.save_as_mainfile",
        "bpy.ops.export_scene.gltf",
        '"UV0"',
        '"MAT_primary"',
        '"MAT_secondary"',
        '"MAT_accent"',
        "forgecad_authoring_metadata",
        'bpy.data.worlds.new("ForgeCAD_World")',
        "_business_position_mm_to_blender_m",
        "_business_size_mm_to_blender_m",
        '(14, -24, 0)',
        '(0, 24, 0)',
    )
    missing_tokens = [token for token in required_tokens if token not in source]
    if missing_ids or missing_tokens:
        raise RuntimeError(
            f"authoring source is incomplete: ids={missing_ids}, tokens={missing_tokens}"
        )
    return {
        "syntax_valid": True,
        "required_module_ids_present": True,
        "export_contract_present": True,
    }


def _find_blender(explicit: Path | None) -> Path | None:
    candidates: list[Path] = []
    if explicit is not None:
        candidates.append(explicit.expanduser())
    for name in ("FORGECAD_BLENDER_EXECUTABLE", "BLENDER_EXECUTABLE"):
        value = os.environ.get(name)
        if value:
            candidates.append(Path(value).expanduser())
    command = shutil.which("blender")
    if command:
        candidates.append(Path(command))
    candidates.extend(
        (
            Path("/Applications/Blender.app/Contents/MacOS/Blender"),
            Path.home() / "Applications/Blender.app/Contents/MacOS/Blender",
        )
    )
    for candidate in candidates:
        resolved = candidate.resolve()
        if resolved.is_file() and os.access(resolved, os.X_OK):
            return resolved
    return None


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, py_compile.PyCompileError) as exc:
        print(
            json.dumps(
                {"ok": False, "status": "preflight_failed", "message": str(exc)},
                ensure_ascii=False,
                indent=2,
            )
        )
        raise SystemExit(1) from exc
