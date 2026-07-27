#!/usr/bin/env python3
"""Run the task-D packaged visual-program loop.

The Rust probe owns all product assertions.  This launcher only supplies the
release-shaped environment, waits for the redacted phase reports, and checks
their cross-process identity.  The Provider is explicitly deterministic and
offline; this command is not real DeepSeek evidence.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
APP_BINARY = ROOT / "apps/desktop/src-tauri/target/release/bundle/macos/CAD 工作台.app/Contents/MacOS/wushen-forge-desktop"
SIDECAR_BINARY = ROOT / "apps/desktop/src-tauri/target/release/bundle/macos/CAD 工作台.app/Contents/MacOS/wushen-agent"
OUTPUT_ROOT = ROOT / "output/task-d-packaged-visual-program"
PHASE_SCHEMA = "ForgeCADVisualProgramPackagedProof@1"
RESUME_SCHEMA = "ForgeCADVisualProgramPackagedResumeProof@1"
EXPECTED_TOOLS = [
    "author_forge_visual_program",
    "build_candidate_geometry",
    "compile_readback_candidate",
    "render_candidate_views",
    "evaluate_candidate",
    "prepare_candidate_preview",
]


class GateFailure(RuntimeError):
    pass


def require(condition: bool, code: str) -> None:
    if not condition:
        raise GateFailure(code)


def read_report(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise GateFailure("TASK_D_REPORT_INVALID") from error
    require(isinstance(value, dict), "TASK_D_REPORT_NOT_OBJECT")
    return value


def launch(environment: dict[str, str], output: Path) -> subprocess.Popen[str]:
    child_environment = os.environ.copy()
    child_environment.update(environment)
    log = output.with_suffix(".log").open("w", encoding="utf-8")
    process = subprocess.Popen(
        [str(APP_BINARY)],
        cwd=ROOT,
        env=child_environment,
        stdout=log,
        stderr=subprocess.STDOUT,
        text=True,
    )
    process._forgecad_log = log  # type: ignore[attr-defined]
    return process


def wait_report(path: Path, process: subprocess.Popen[str], timeout: int = 900) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists():
            return read_report(path)
        if process.poll() is not None:
            break
        time.sleep(1)
    log = getattr(process, "_forgecad_log", None)
    if log is not None:
        log.flush()
    raise GateFailure(f"TASK_D_REPORT_TIMEOUT:{path}")


def stop(process: subprocess.Popen[str]) -> None:
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=30)
        except subprocess.TimeoutExpired as error:
            raise GateFailure("TASK_D_PACKAGED_PROCESS_DID_NOT_EXIT") from error
    log = getattr(process, "_forgecad_log", None)
    if log is not None:
        log.close()
    subprocess.run(
        ["pkill", "-TERM", "-f", f"{SIDECAR_BINARY} agent serve"],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def validate_phase(value: dict[str, Any]) -> dict[str, Any]:
    require(value.get("schema_version") == PHASE_SCHEMA, "TASK_D_PHASE_SCHEMA_INVALID")
    require(value.get("status") == "pass", f"TASK_D_PHASE_FAILED:{value.get('error_code')}")
    require(value.get("tools") == EXPECTED_TOOLS, "TASK_D_TOOL_SEQUENCE_INVALID")
    require(not {"infer_product_domain", "select_style_recipe", "plan_complete_concept"}.intersection(value["tools"]), "TASK_D_LEGACY_TOOL_REACHED")
    require(value.get("provider") == {
        "source_kind": "offline_deterministic",
        "internal_subrequests": 1,
        "action_loop_steps": 1,
        "product_tool_calls": 6,
        "external_network_calls": 0,
        "credential_reads": 0,
    }, "TASK_D_PROVIDER_EVIDENCE_INVALID")
    program = value.get("program")
    require(isinstance(program, dict), "TASK_D_PROGRAM_EVIDENCE_MISSING")
    require(program.get("domain_pack_id") == "pack_robotic_arm_concept", "TASK_D_PROGRAM_DOMAIN_INVALID")
    require(program.get("visual_only") is True, "TASK_D_PROGRAM_SCOPE_INVALID")
    image_evidence = value.get("image_evidence")
    require(isinstance(image_evidence, dict), "TASK_D_INPUT_IMAGE_EVIDENCE_MISSING")
    require(image_evidence.get("media_type") == "image/png", "TASK_D_INPUT_IMAGE_MEDIA_TYPE_INVALID")
    require(image_evidence.get("source_object_sha256") == image_evidence.get("content_readback_sha256"), "TASK_D_INPUT_IMAGE_HASH_DRIFT")
    renderer = value.get("renderer")
    require(isinstance(renderer, dict), "TASK_D_RENDERER_EVIDENCE_MISSING")
    require(len(renderer.get("view_ids", [])) == 8, "TASK_D_RENDER_VIEW_COUNT_INVALID")
    require(len(renderer.get("view_sha256", {})) == 8, "TASK_D_IMAGE_EVIDENCE_HASHES_MISSING")
    require(isinstance(renderer.get("renderer_id"), str) and renderer["renderer_id"], "TASK_D_SINGLE_RENDERER_MISSING")
    require(len(renderer.get("render_package_sha256", "")) == 64, "TASK_D_RENDER_PACKAGE_HASH_MISSING")
    preview = value.get("preview")
    export = value.get("export")
    require(isinstance(preview, dict) and isinstance(export, dict), "TASK_D_PREVIEW_EXPORT_MISSING")
    require(preview.get("glb_sha256") == export.get("glb_sha256"), "TASK_D_PREVIEW_EXPORT_HASH_DRIFT")
    require(80_000 <= export.get("triangle_count", 0) <= 150_000, "TASK_D_PRODUCTION_DENSITY_INVALID")
    require(value.get("confirmed_asset_version_id") == export.get("asset_version_id"), "TASK_D_ACTIVE_VERSION_INVALID")
    require(export.get("glb_sha256") == export.get("x_forgecad_glb_sha256"), "TASK_D_EXPORT_READBACK_HASH_INVALID")
    return value


def validate_resume(value: dict[str, Any], phase: dict[str, Any]) -> dict[str, Any]:
    require(value.get("schema_version") == RESUME_SCHEMA, "TASK_D_RESUME_SCHEMA_INVALID")
    require(value.get("status") == "pass", f"TASK_D_RESUME_FAILED:{value.get('error_code')}")
    expected = phase["confirmed_asset_version_id"]
    require(value.get("expected_asset_version_id") == expected, "TASK_D_RESUME_VERSION_DRIFT")
    require(value.get("active_design", {}).get("asset_version_id") == expected, "TASK_D_RESUME_ACTIVE_DRIFT")
    require(value.get("export", {}).get("glb_sha256") == phase["export"]["glb_sha256"], "TASK_D_RESUME_EXPORT_HASH_DRIFT")
    return value


def main() -> int:
    require(APP_BINARY.exists(), "TASK_D_RELEASE_BINARY_MISSING")
    with tempfile.TemporaryDirectory(prefix="forgecad-task-d-library-") as temporary:
        temporary_root = Path(temporary)
        phase_path = temporary_root / "phase.json"
        resume_path = temporary_root / "resume.json"
        base_environment = {
            "WUSHEN_AGENT_RUNTIME_MODE": "packaged-sidecar",
            "FORGECAD_DISABLE_PROVIDER_CONFIG": "1",
            "FORGECAD_MVP_OFFLINE_ARM": "1",
            "FORGECAD_MVP_VISUAL_PROGRAM_E2E": "1",
            "FORGECAD_CONCEPT_WORKER_ENABLED": "0",
            "WUSHEN_LOCAL_WORKER_ENABLED": "0",
            "WUSHEN_LIBRARY_ROOT": str(temporary_root),
            "FORGECAD_MVP_VISUAL_PROGRAM_E2E_OUTPUT": str(phase_path),
        }
        first = launch(base_environment, phase_path)
        try:
            phase = validate_phase(wait_report(phase_path, first))
        finally:
            stop(first)
        resume_environment = dict(base_environment)
        resume_environment.update({
            "FORGECAD_MVP_VISUAL_PROGRAM_E2E_RESUME": "1",
            "FORGECAD_MVP_VISUAL_PROGRAM_E2E_RESUME_INPUT": str(phase_path),
            "FORGECAD_MVP_VISUAL_PROGRAM_E2E_OUTPUT": str(resume_path),
        })
        second = launch(resume_environment, resume_path)
        try:
            resume = validate_resume(wait_report(resume_path, second), phase)
        finally:
            stop(second)
        OUTPUT_ROOT.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(phase_path, OUTPUT_ROOT / "visual-program-proof.json")
        shutil.copyfile(resume_path, OUTPUT_ROOT / "visual-program-resume-proof.json")
    print(json.dumps({"schema_version": "ForgeCADTaskDPackagedVisualProgram@1", "status": "pass", "phase": phase, "resume": resume}, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateFailure as error:
        print(str(error))
        raise SystemExit(1)
