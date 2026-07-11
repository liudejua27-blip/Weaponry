#!/usr/bin/env python3
"""Preflight or execute read-only re-export of edited Blender module sources."""

from __future__ import annotations

import argparse
import hashlib
import json
import py_compile
import subprocess
from pathlib import Path

from build_blender_starter_pack import (
    COMMITTED_PACK_ROOT,
    DEFAULT_OUTPUT_ROOT as STARTER_OUTPUT_ROOT,
    REQUIRED_MODULE_IDS,
    _find_blender,
)
from concept_module_pack import ModulePackValidationError, validate_module_pack


ROOT = Path(__file__).resolve().parents[1]
EXPORT_SCRIPT = ROOT / "scripts" / "blender" / "export_weapon_concept_sources.py"
DEFAULT_SOURCE_ROOT = STARTER_OUTPUT_ROOT / "sources"
DEFAULT_EXPORT_ROOT = ROOT / "output" / "blender" / "weapon-concept-v1-edited-export"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--blender-executable", type=Path)
    parser.add_argument("--source-root", type=Path, default=DEFAULT_SOURCE_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_EXPORT_ROOT)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--require-blender", action="store_true")
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    script_check = _check_export_source()
    source_root = args.source_root.expanduser().resolve()
    output_root = args.output_root.expanduser().resolve()
    safety_error = _output_safety_error(source_root, output_root)
    if safety_error:
        return _print_failure(safety_error, source_root=source_root, output_root=output_root)

    source_paths = tuple(source_root / f"{module_id}.blend" for module_id in REQUIRED_MODULE_IDS)
    source_errors = _source_errors(source_root, source_paths)
    blender = _find_blender(args.blender_executable)
    blender_ready = blender is not None
    sources_ready = not source_errors
    output_ready = not (
        output_root.exists() and any(output_root.iterdir()) and not args.force
    )

    if not args.execute:
        status = (
            "ready_for_read_only_export"
            if blender_ready and sources_ready and output_ready
            else "blocked_blender_and_sources_not_ready"
            if not blender_ready and not sources_ready
            else "blocked_blender_not_configured"
            if not blender_ready
            else "blocked_sources_not_ready"
            if not sources_ready
            else "blocked_output_not_empty"
        )
        print(
            json.dumps(
                {
                    "ok": True,
                    "status": status,
                    "execute": False,
                    "blender_ready": blender_ready,
                    "sources_ready": sources_ready,
                    "output_ready": output_ready,
                    "source_errors": source_errors,
                    "source_root": str(source_root),
                    "output_root": str(output_root),
                    "script_check": script_check,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
        return 0

    if args.require_blender and not blender_ready:
        return _print_failure("blender_not_configured", source_root=source_root)
    if not blender_ready:
        return _print_failure("blender_not_configured", source_root=source_root)
    if source_errors:
        return _print_failure(
            "sources_not_ready",
            source_root=source_root,
            details=source_errors,
        )
    if not output_ready:
        return _print_failure("output_not_empty", output_root=output_root)

    before_hashes = {path.name: _sha256(path) for path in source_paths}
    command = [
        str(blender),
        "--background",
        "--python-exit-code",
        "1",
        "--factory-startup",
        "--python",
        str(EXPORT_SCRIPT),
        "--",
        "--source-root",
        str(source_root),
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
        return _print_failure(
            "blender_edited_export_failed",
            details={
                "returncode": completed.returncode,
                "stdout_tail": completed.stdout[-4000:],
                "stderr_tail": completed.stderr[-4000:],
            },
        )

    after_hashes = {path.name: _sha256(path) for path in source_paths}
    if after_hashes != before_hashes:
        return _print_failure(
            "source_blend_changed_during_export",
            details={"before": before_hashes, "after": after_hashes},
        )
    try:
        validated = validate_module_pack(output_root, release=False)
    except ModulePackValidationError as exc:
        return _print_failure("edited_export_validation_failed", details=exc.errors)
    module_ids = tuple(module.manifest.module_id for module in validated.modules)
    if module_ids != REQUIRED_MODULE_IDS:
        return _print_failure(
            "edited_export_module_set_mismatch",
            details={"expected": list(REQUIRED_MODULE_IDS), "actual": list(module_ids)},
        )
    print(
        json.dumps(
            {
                "ok": True,
                "status": "edited_sources_exported_and_validated",
                "source_unchanged": True,
                "source_root": str(source_root),
                "output_root": str(output_root),
                "module_ids": list(module_ids),
                "warnings": list(validated.warnings),
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


def _check_export_source() -> dict[str, bool]:
    py_compile.compile(str(EXPORT_SCRIPT), doraise=True)
    source = EXPORT_SCRIPT.read_text(encoding="utf-8")
    required = (
        "bpy.ops.wm.open_mainfile",
        "bpy.ops.export_scene.gltf",
        "forgecad_authoring_metadata",
        "apply location/rotation/scale",
        "apply modifiers before export",
        "_blender_position_m_to_business_mm",
        "_blender_rotation_to_business_euler",
    )
    missing = [token for token in required if token not in source]
    if "save_as_mainfile" in source or missing:
        raise RuntimeError(
            f"read-only exporter contract failed: save_call={'save_as_mainfile' in source}, "
            f"missing={missing}"
        )
    return {
        "syntax_valid": True,
        "opens_existing_sources": True,
        "does_not_save_sources": True,
        "export_validation_present": True,
    }


def _output_safety_error(source_root: Path, output_root: Path) -> str | None:
    if output_root.is_relative_to(COMMITTED_PACK_ROOT):
        return "committed_pack_output_denied"
    if output_root == source_root:
        return "source_output_overlap_denied"
    if output_root.is_relative_to(source_root) or source_root.is_relative_to(output_root):
        return "source_output_overlap_denied"
    return None


def _source_errors(source_root: Path, paths: tuple[Path, ...]) -> list[str]:
    if not source_root.is_dir():
        return [f"source directory does not exist: {source_root}"]
    expected = {path.name for path in paths}
    actual = {path.name for path in source_root.glob("*.blend")}
    errors = []
    if actual != expected:
        errors.append(f"expected exactly {sorted(expected)}, found {sorted(actual)}")
    for path in paths:
        if not path.is_file():
            errors.append(f"missing source: {path.name}")
        elif not _has_blender_header(path):
            errors.append(f"invalid Blender header: {path.name}")
    return errors


def _has_blender_header(path: Path) -> bool:
    with path.open("rb") as handle:
        return handle.read(7) == b"BLENDER"


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _print_failure(
    status: str,
    *,
    source_root: Path | None = None,
    output_root: Path | None = None,
    details=None,
) -> int:
    print(
        json.dumps(
            {
                "ok": False,
                "status": status,
                "source_root": str(source_root) if source_root else None,
                "output_root": str(output_root) if output_root else None,
                "details": details,
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, py_compile.PyCompileError) as exc:
        print(
            json.dumps(
                {"ok": False, "status": "edited_export_preflight_failed", "message": str(exc)},
                ensure_ascii=False,
                indent=2,
            )
        )
        raise SystemExit(1) from exc
