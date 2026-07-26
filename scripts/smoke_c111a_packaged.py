#!/usr/bin/env python3
"""Run the C111A golden-surface Product Tool/A005/restart proof."""

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
OUTPUT_ROOT = ROOT / "output/c111a-packaged-golden-path"
PHASE_SCHEMA = "ForgeCADArmMvpPackagedProtocolProof@4"
RESUME_SCHEMA = "ForgeCADArmMvpPackagedResumeProof@4"
ROOT_RECIPE = "recipe_c111_arm_golden_surface"
EXPECTED_INITIAL_GLB = "e3023bf3e4621dcbcd1f19ab3efa87d2779fb547af0c93cc841fbb1837917496"


class GateFailure(RuntimeError):
    pass


def require(condition: bool, code: str) -> None:
    if not condition:
        raise GateFailure(code)


def read_report(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise GateFailure("C111A_PACKAGED_REPORT_INVALID") from exc
    require(isinstance(value, dict), "C111A_PACKAGED_REPORT_NOT_OBJECT")
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
    raise GateFailure(f"C111A_PACKAGED_REPORT_TIMEOUT:{path}")


def stop(process: subprocess.Popen[str]) -> None:
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=30)
        except subprocess.TimeoutExpired as exc:
            raise GateFailure("C111A_PACKAGED_PROCESS_DID_NOT_EXIT") from exc
    log = getattr(process, "_forgecad_log", None)
    if log is not None:
        log.close()
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
    raise GateFailure("C111A_PACKAGED_PORT_NOT_RELEASED")


def validate_phase(value: dict[str, Any]) -> dict[str, Any]:
    require(value.get("schema_version") == PHASE_SCHEMA, "C111A_PHASE_SCHEMA_INVALID")
    require(value.get("status") == "pass", f"C111A_PHASE_FAILED:{value.get('error_code')}")
    require(value.get("root_recipe_id") == ROOT_RECIPE, "C111A_ROOT_RECIPE_INVALID")
    require(value.get("c110c") is None and value.get("c110d") is None, "C111A_UNRELATED_DELTA_PRESENT")
    preview = value.get("preview")
    require(isinstance(preview, dict), "C111A_PREVIEW_MISSING")
    require(preview.get("artifact_profile_id") == "production_concept", "C111A_PREVIEW_PROFILE_INVALID")
    require(preview.get("glb_sha256") == EXPECTED_INITIAL_GLB, "C111A_INITIAL_GLB_DRIFT")
    require(preview.get("triangle_count") == 130_244, "C111A_INITIAL_TRIANGLE_DRIFT")
    a005 = value.get("a005")
    require(isinstance(a005, dict), "C111A_A005_MISSING")
    require(a005.get("parent_asset_version_id") == value.get("v1_asset_version_id"), "C111A_A005_PARENT_DRIFT")
    program_ids = a005.get("surface_adornment_program_ids")
    require(
        a005.get("surface_adornment_count") == 6
        and isinstance(program_ids, list)
        and len(program_ids) == 6
        and "adorn_c111_base_flowline" not in program_ids
        and {
            "adorn_c111_gripper_chevron",
            "adorn_c111_gripper_microgrid",
            "adorn_c111_joint_microgrid",
            "adorn_c111_link_flowline",
            "adorn_c111_link_groove",
        }.issubset(set(program_ids)),
        "C111A_A005_PROGRAMS_MISSING",
    )
    active = value.get("active_design")
    export = value.get("export")
    require(isinstance(active, dict) and isinstance(export, dict), "C111A_ACTIVE_OR_EXPORT_MISSING")
    require(active.get("asset_version_id") == a005.get("v2_asset_version_id"), "C111A_ACTIVE_HEAD_DRIFT")
    require(export.get("asset_version_id") == a005.get("v2_asset_version_id"), "C111A_EXPORT_VERSION_DRIFT")
    require(export.get("glb_sha256") == export.get("x_forgecad_glb_sha256"), "C111A_EXPORT_HASH_DRIFT")
    require(export.get("triangle_count") == 130_244, "C111A_EXPORT_TRIANGLE_DRIFT")
    require(
        value.get("provider")
        == {
            "source_kind": "offline_deterministic",
            "internal_subrequests": 1,
            "action_loop_steps": 1,
            "product_tool_calls": 6,
            "external_network_calls": 0,
            "credential_reads": 0,
        },
        "C111A_PROVIDER_POLICY_INVALID",
    )
    return value


def validate_resume(value: dict[str, Any], phase: dict[str, Any]) -> dict[str, Any]:
    require(value.get("schema_version") == RESUME_SCHEMA, "C111A_RESUME_SCHEMA_INVALID")
    require(value.get("status") == "pass", f"C111A_RESUME_FAILED:{value.get('error_code')}")
    expected = phase["a005"]["v2_asset_version_id"]
    require(value.get("expected_asset_version_id") == expected, "C111A_RESUME_VERSION_DRIFT")
    active = value.get("active_design")
    export = value.get("export")
    require(isinstance(active, dict) and isinstance(export, dict), "C111A_RESUME_FIELDS_MISSING")
    require(active.get("asset_version_id") == expected, "C111A_RESUME_ACTIVE_DRIFT")
    require(export.get("asset_version_id") == expected, "C111A_RESUME_EXPORT_VERSION_DRIFT")
    require(export.get("glb_sha256") == phase["export"]["glb_sha256"], "C111A_RESUME_HASH_DRIFT")
    require(export.get("glb_byte_size") == phase["export"]["glb_byte_size"], "C111A_RESUME_BYTES_DRIFT")
    require(export.get("triangle_count") == phase["export"]["triangle_count"], "C111A_RESUME_TRIANGLE_DRIFT")
    return value


def main() -> int:
    require(APP_BINARY.exists(), "C111A_RELEASE_BINARY_MISSING")
    temporary_root = Path(tempfile.mkdtemp(prefix="forgecad-c111a-library-"))
    phase_path = temporary_root / "phase.json"
    resume_path = temporary_root / "resume.json"
    environment = {
        "WUSHEN_AGENT_RUNTIME_MODE": "packaged-sidecar",
        "FORGECAD_DISABLE_PROVIDER_CONFIG": "1",
        "FORGECAD_MVP_OFFLINE_ARM": "1",
        "FORGECAD_MVP_ARM_ARCHITECTURE": "golden_surface",
        "FORGECAD_CONCEPT_WORKER_ENABLED": "0",
        "WUSHEN_LOCAL_WORKER_ENABLED": "0",
        "WUSHEN_LIBRARY_ROOT": str(temporary_root),
        "FORGECAD_MVP_ARM_PACKAGED_PROBE": "1",
        "FORGECAD_MVP_ARM_PACKAGED_PROBE_OUTPUT": str(phase_path),
    }
    first = launch(environment, phase_path)
    try:
        phase = validate_phase(wait_report(phase_path, first))
    finally:
        stop(first)
    resume_environment = dict(environment)
    resume_environment["FORGECAD_MVP_ARM_PACKAGED_RESUME"] = "1"
    resume_environment["FORGECAD_MVP_ARM_PACKAGED_RESUME_INPUT"] = str(phase_path)
    resume_environment["FORGECAD_MVP_ARM_PACKAGED_PROBE_OUTPUT"] = str(resume_path)
    second = launch(resume_environment, resume_path)
    try:
        resume = validate_resume(wait_report(resume_path, second), phase)
    finally:
        stop(second)
    OUTPUT_ROOT.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(phase_path, OUTPUT_ROOT / "packaged-protocol-proof.json")
    shutil.copyfile(resume_path, OUTPUT_ROOT / "packaged-resume-proof.json")
    print(json.dumps({
        "schema_version": "ForgeCADC111APackagedGoldenPath@1",
        "status": "pass",
        "phase": phase,
        "resume": resume,
    }, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateFailure as error:
        print(str(error))
        raise SystemExit(1)
