#!/usr/bin/env python3
"""Run the packaged C110G parallel-link preview/delta/restart proof.

The probe is Rust-owned and offline by design.  This script only launches the
exact release binary, validates its redacted report, and copies the two reports
into the caller-visible output directory.
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
OUTPUT_ROOT = ROOT / "output/c110g-packaged-golden-path"
SCHEMA = "ForgeCADC110GPackagedProtocolProof@1"
RESUME_SCHEMA = "ForgeCADC110GPackagedResumeProof@1"
ROOT_RECIPE = "recipe_c110g_parallel_link_root"


class GateFailure(RuntimeError):
    pass


def require(condition: bool, code: str) -> None:
    if not condition:
        raise GateFailure(code)


def report(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise GateFailure("C110G_REPORT_INVALID") from exc
    require(isinstance(value, dict), "C110G_REPORT_OBJECT_INVALID")
    return value


def launch(env: dict[str, str], output: Path) -> subprocess.Popen[str]:
    child_env = os.environ.copy()
    child_env.update(env)
    log = output.with_suffix(".log").open("w", encoding="utf-8")
    process = subprocess.Popen(
        [str(APP_BINARY)],
        cwd=ROOT,
        env=child_env,
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
            return report(path)
        if process.poll() is not None:
            break
        time.sleep(1)
    log = getattr(process, "_forgecad_log", None)
    if log is not None:
        log.flush()
    raise GateFailure(f"C110G_REPORT_TIMEOUT:{path}")


def stop(process: subprocess.Popen[str]) -> None:
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=30)
        except subprocess.TimeoutExpired as exc:
            raise GateFailure("C110G_PROCESS_DID_NOT_EXIT") from exc
    log = getattr(process, "_forgecad_log", None)
    if log is not None:
        log.close()
    # The app supervisor normally owns its sidecar shutdown.  If the app
    # exits during a probe failure, finish only this exact bundled sidecar so
    # the restart phase cannot accidentally attach to an older session.
    subprocess.run(
        ["pkill", "-TERM", "-f", f"{SIDECAR_BINARY} agent serve"],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    for _ in range(20):
        listener = subprocess.run(
            ["lsof", "-nP", "-iTCP:8000", "-sTCP:LISTEN"],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        if listener.returncode != 0:
            return
        time.sleep(1)
    raise GateFailure("C110G_PACKAGED_PORT_NOT_RELEASED")


def validate_phase(report_value: dict[str, Any]) -> dict[str, Any]:
    require(report_value.get("schema_version") == SCHEMA, "C110G_SCHEMA_INVALID")
    require(report_value.get("status") == "pass", "C110G_PHASE_FAILED")
    require(report_value.get("architecture") == "parallel_link", "C110G_ARCHITECTURE_INVALID")
    require(report_value.get("root_recipe_id") == ROOT_RECIPE, "C110G_ROOT_RECIPE_INVALID")
    preview = report_value.get("initial_preview")
    require(isinstance(preview, dict), "C110G_INITIAL_PREVIEW_MISSING")
    require(preview.get("artifact_profile_id") == "production_concept", "C110G_INITIAL_PROFILE_INVALID")
    require(isinstance(preview.get("glb_sha256"), str) and len(preview["glb_sha256"]) == 64, "C110G_INITIAL_HASH_INVALID")
    require(isinstance(preview.get("triangle_count"), int) and preview["triangle_count"] > 0, "C110G_INITIAL_TRIANGLES_INVALID")
    delta = report_value.get("delta")
    require(isinstance(delta, dict), "C110G_DELTA_MISSING")
    require(delta.get("parent_asset_version_id") == report_value.get("initial_asset_version_id"), "C110G_DELTA_PARENT_INVALID")
    require(delta.get("asset_version_id"), "C110G_DELTA_VERSION_MISSING")
    require(delta.get("added_part_id") == "part_c110g_added_link", "C110G_DELTA_PART_INVALID")
    require(delta.get("recipe_id") == "recipe_c110g_parallel_link", "C110G_DELTA_RECIPE_INVALID")
    require(delta.get("slot_id") == "slot_c110g_parallel_link", "C110G_DELTA_SLOT_INVALID")
    require(delta.get("operation_count") == 1, "C110G_DELTA_OPERATION_COUNT_INVALID")
    delta_preview = delta.get("preview")
    require(isinstance(delta_preview, dict), "C110G_DELTA_PREVIEW_MISSING")
    # The delta preview is interactive_preview while the initial result is
    # production_concept; cross-profile triangle counts are not comparable.
    require(delta_preview.get("triangle_count", 0) > 0, "C110G_DELTA_TRIANGLES_INVALID")
    active = report_value.get("active_design")
    export = report_value.get("export")
    require(isinstance(active, dict), "C110G_ACTIVE_MISSING")
    require(isinstance(export, dict), "C110G_EXPORT_MISSING")
    require(active.get("asset_version_id") == delta.get("asset_version_id"), "C110G_ACTIVE_DRIFT")
    require(export.get("asset_version_id") == delta.get("asset_version_id"), "C110G_EXPORT_VERSION_DRIFT")
    require(export.get("glb_sha256") == export.get("x_forgecad_glb_sha256"), "C110G_EXPORT_HASH_DRIFT")
    require(export.get("triangle_count", 0) >= delta_preview["triangle_count"], "C110G_EXPORT_TRIANGLES_INVALID")
    require(
        report_value.get("provider")
        == {
            "source_kind": "offline_deterministic",
            "internal_subrequests": 8,
            "action_loop_steps": 8,
            "product_tool_calls": 7,
            "external_network_calls": 0,
            "credential_reads": 0,
        },
        "C110G_PROVIDER_POLICY_INVALID",
    )
    return report_value


def validate_resume(resume: dict[str, Any], phase: dict[str, Any]) -> dict[str, Any]:
    require(resume.get("schema_version") == RESUME_SCHEMA, "C110G_RESUME_SCHEMA_INVALID")
    require(resume.get("status") == "pass", "C110G_RESUME_FAILED")
    delta = phase["delta"]
    require(resume.get("expected_asset_version_id") == delta["asset_version_id"], "C110G_RESUME_VERSION_DRIFT")
    active = resume.get("active_design")
    export = resume.get("export")
    require(isinstance(active, dict) and isinstance(export, dict), "C110G_RESUME_FIELDS_MISSING")
    require(active.get("asset_version_id") == delta["asset_version_id"], "C110G_RESUME_ACTIVE_DRIFT")
    require(export.get("asset_version_id") == phase["export"]["asset_version_id"], "C110G_RESUME_EXPORT_VERSION_DRIFT")
    require(export.get("glb_sha256") == phase["export"]["glb_sha256"], "C110G_RESUME_EXPORT_HASH_DRIFT")
    require(export.get("glb_byte_size") == phase["export"]["glb_byte_size"], "C110G_RESUME_EXPORT_BYTES_DRIFT")
    require(export.get("triangle_count") == phase["export"]["triangle_count"], "C110G_RESUME_EXPORT_TRIANGLES_DRIFT")
    return {
        "active_asset_version_id": active["asset_version_id"],
        "snapshot_revision": active.get("snapshot_revision"),
        "export_glb_sha256": export["glb_sha256"],
        "export_glb_byte_size": export["glb_byte_size"],
        "export_triangle_count": export["triangle_count"],
    }


def main() -> int:
    require(APP_BINARY.exists(), "C110G_RELEASE_BINARY_MISSING")
    temp_root = Path(tempfile.mkdtemp(prefix="forgecad-c110g-library-"))
    phase_path = temp_root / "phase.json"
    resume_path = temp_root / "resume.json"
    base_env = {
        "WUSHEN_AGENT_RUNTIME_MODE": "packaged-sidecar",
        "FORGECAD_DISABLE_PROVIDER_CONFIG": "1",
        "FORGECAD_MVP_OFFLINE_ARM": "1",
        "FORGECAD_MVP_ARM_ARCHITECTURE": "parallel_link",
        "FORGECAD_CONCEPT_WORKER_ENABLED": "0",
        "WUSHEN_LOCAL_WORKER_ENABLED": "0",
        "WUSHEN_LIBRARY_ROOT": str(temp_root),
        "FORGECAD_C110G_PACKAGED_PROBE": "1",
        "FORGECAD_C110G_PACKAGED_PROBE_OUTPUT": str(phase_path),
    }
    first = launch(base_env, phase_path)
    try:
        phase = validate_phase(wait_report(phase_path, first))
    finally:
        stop(first)
    second_env = dict(base_env)
    second_env.pop("FORGECAD_C110G_PACKAGED_PROBE_OUTPUT")
    second_env["FORGECAD_C110G_PACKAGED_RESUME"] = "1"
    second_env["FORGECAD_C110G_PACKAGED_RESUME_INPUT"] = str(phase_path)
    second_env["FORGECAD_C110G_PACKAGED_PROBE_OUTPUT"] = str(resume_path)
    second = launch(second_env, resume_path)
    try:
        resume = validate_resume(wait_report(resume_path, second), phase)
    finally:
        stop(second)
    OUTPUT_ROOT.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(phase_path, OUTPUT_ROOT / "packaged-protocol-proof.json")
    shutil.copyfile(resume_path, OUTPUT_ROOT / "packaged-resume-proof.json")
    print(json.dumps({"schema_version": "ForgeCADC110GPackagedGoldenPath@1", "status": "pass", "phase": phase, "resume": resume}, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateFailure as exc:
        print(str(exc))
        raise SystemExit(1)
