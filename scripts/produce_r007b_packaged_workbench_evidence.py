#!/usr/bin/env python3
"""Produce fresh three-class R007B evidence from the packaged macOS workbench.

This is deliberately an *orchestrator*, not a substitute renderer.  For each
reference class it starts the real signed-shaped Tauri bundle in a new HOME,
waits for the WebView's sealed success report, and copies only the PNG/GLB
bytes whose Rust receipts appear in that report.  The final manifest is then
validated by :mod:`validate_r007b_workbench_visual_evidence` before it is
published atomically.

There is no headless browser, dev server, accessibility automation, proxy,
fixture input, provider credential, or fallback to a previous report.  A
locked console, occupied sidecar port, missing receipt, malformed capture,
timeout, or any lineage drift leaves no passing manifest behind.
"""

from __future__ import annotations

import argparse
import copy
from datetime import datetime, timedelta, timezone
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time
from typing import Any, Mapping, NoReturn

from smoke_arm_webview_packaged_tauri import (
    APP_BINARY,
    MARKER,
    REPORT_TIMEOUT_SECONDS,
    SCHEMA as WEBVIEW_SCHEMA,
    _assert,
    _desktop_pids,
    _is_descendant,
    _listener_pid,
    _macos_console_screen_locked,
    _stop_desktop_and_listener,
    _validate_initial,
    _wait_for_native_health,
    _wait_report,
)
from validate_r007b_workbench_visual_evidence import (
    REFERENCE_CLASSES,
    SCHEMA as MANIFEST_SCHEMA,
    EvidenceFailure,
    png_pixels,
    validate_manifest,
)


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / "output" / "r007b-packaged-workbench-evidence"
PRODUCER_SCHEMA = "ForgeCADR007BPackagedWorkbenchProducer@1"
RUN_SCHEMA_KEY = "r007b_visual_run"
RUN_ENV = "FORGECAD_R007B_PACKAGED_REFERENCE_CLASS"
ARTIFACT_ENV = "FORGECAD_R007B_PACKAGED_ARTIFACT_DIR"
STABLE = re.compile(r"^[A-Za-z0-9_.-]{1,160}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")


class ProducerFailure(RuntimeError):
    """A bounded, non-sensitive reason no fresh packaged evidence was made."""


def fail(code: str) -> NoReturn:
    raise ProducerFailure(code)


def stable_rejection_code(reference_class: str, error: BaseException) -> str:
    """Retain only a bounded QA code or progress marker from child failures."""

    prefix = f"R007B_PACKAGED_{reference_class.upper()}_REJECTED"
    detail = str(error)
    qa_code = re.search(r"\b(QA_[A-Z0-9_]{3,96})\b", detail)
    if qa_code is not None:
        return f"{prefix}_{qa_code.group(1)}"
    progress = re.search(r"\blast_progress=([A-Za-z0-9_.-]{1,160})\b", detail)
    if progress is not None:
        return f"{prefix}_LAST_PROGRESS_{progress.group(1).upper()}"
    return prefix


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


def safe_relative(value: object, suffix: str, *, code: str) -> str:
    if not isinstance(value, str) or not value:
        fail(code)
    path = PurePosixPath(value)
    if (
        path.is_absolute()
        or "\\" in value
        or ".." in path.parts
        or value.startswith("./")
        or len(path.parts) < 2
        or not value.endswith(suffix)
    ):
        fail(code)
    return value


def require_mapping(value: object, code: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        fail(code)
    return value


def require_sha(value: object, code: str) -> str:
    if not isinstance(value, str) or SHA256.fullmatch(value) is None:
        fail(code)
    return value


def child_path(root: Path, relative: str, code: str) -> Path:
    candidate = root.joinpath(*PurePosixPath(relative).parts)
    try:
        candidate.relative_to(root)
    except ValueError:
        fail(code)
    if candidate.is_symlink() or not candidate.is_file():
        fail(code)
    return candidate


def safe_directory_under_home(home: Path, candidate: Path, code: str) -> Path:
    home_resolved = home.resolve(strict=True)
    candidate = candidate.resolve(strict=False)
    try:
        candidate.relative_to(home_resolved)
    except ValueError:
        fail(code)
    candidate.mkdir(parents=True, exist_ok=False)
    resolved = candidate.resolve(strict=True)
    try:
        resolved.relative_to(home_resolved)
    except ValueError:
        fail(code)
    if resolved != candidate:
        fail(code)
    return resolved


def fresh_environment(home: Path, library: Path, reference_class: str, artifact_root: Path) -> dict[str, str]:
    """Return the only environment permitted to enter the packaged QA child."""

    if reference_class not in REFERENCE_CLASSES:
        fail("R007B_PACKAGED_REFERENCE_CLASS_INVALID")
    resolved_home = home.resolve(strict=True)
    resolved_artifacts = artifact_root.resolve(strict=True)
    try:
        resolved_artifacts.relative_to(resolved_home)
    except ValueError:
        fail("R007B_PACKAGED_ARTIFACT_ROOT_ESCAPES_HOME")
    environment = os.environ.copy()
    sensitive_or_qa = {
        "FORGECAD_AGENT_API_KEY",
        "FORGECAD_AGENT_BASE_URL",
        "FORGECAD_AGENT_MODEL",
        "FORGECAD_K001_PACKAGED_PROBE",
        "FORGECAD_K002_PACKAGED_PROBE",
    }
    for name in tuple(environment):
        if name.startswith("FORGECAD_ARM_WEBVIEW_QA") or name.startswith("FORGECAD_R007B_PACKAGED_") or name in sensitive_or_qa:
            environment.pop(name, None)
    environment.update({
        "HOME": str(resolved_home),
        "WUSHEN_LIBRARY_ROOT": str(library),
        "WUSHEN_AGENT_RUNTIME_MODE": "packaged-sidecar",
        "FORGECAD_DISABLE_PROVIDER_CONFIG": "1",
        "FORGECAD_MVP_OFFLINE_ARM": "1",
        "FORGECAD_CONCEPT_WORKER_ENABLED": "0",
        "WUSHEN_LOCAL_WORKER_ENABLED": "0",
        "FORGECAD_ARM_WEBVIEW_QA": "1",
        "FORGECAD_ARM_WEBVIEW_QA_PHASE": "initial",
        RUN_ENV: reference_class,
        ARTIFACT_ENV: str(resolved_artifacts),
    })
    return environment


def start_packaged(home: Path, environment: Mapping[str, str]) -> int:
    """Launch one actual bundle with a fixed, reviewable QA environment."""

    existing = _desktop_pids()
    permitted = {
        "HOME",
        "WUSHEN_LIBRARY_ROOT",
        "WUSHEN_AGENT_RUNTIME_MODE",
        "FORGECAD_DISABLE_PROVIDER_CONFIG",
        "FORGECAD_MVP_OFFLINE_ARM",
        "FORGECAD_CONCEPT_WORKER_ENABLED",
        "WUSHEN_LOCAL_WORKER_ENABLED",
        "FORGECAD_ARM_WEBVIEW_QA",
        "FORGECAD_ARM_WEBVIEW_QA_PHASE",
        RUN_ENV,
        ARTIFACT_ENV,
    }
    command = ["open", "-n"]
    for name in sorted(permitted):
        value = environment.get(name)
        if value is not None:
            command.extend(["--env", f"{name}={value}"])
    command.append(str(APP_BINARY.parent.parent.parent))
    try:
        subprocess.run(command, cwd=home, env=dict(environment), check=True, timeout=30)
    except (OSError, subprocess.SubprocessError) as exc:
        raise ProducerFailure("R007B_PACKAGED_APP_LAUNCH_FAILED") from exc
    for _ in range(100):
        created = _desktop_pids() - existing
        if created:
            return max(created)
        time.sleep(0.1)
    fail("R007B_PACKAGED_APP_PID_MISSING")


def verify_png_receipt(artifact_root: Path, capture: Mapping[str, Any], *, expected_relative: str) -> Path:
    relative = safe_relative(capture.get("relative_path"), ".png", code="R007B_PACKAGED_CAPTURE_PATH_INVALID")
    if relative != expected_relative:
        fail("R007B_PACKAGED_CAPTURE_PATH_DRIFT")
    path = child_path(artifact_root, relative, "R007B_PACKAGED_CAPTURE_MISSING")
    width, height, raw, _ = png_pixels(path, "R007B_PACKAGED_CAPTURE_PNG_INVALID")
    if (
        capture.get("width") != width
        or capture.get("height") != height
        or capture.get("byte_size") != len(raw)
        or require_sha(capture.get("sha256"), "R007B_PACKAGED_CAPTURE_HASH_INVALID") != hashlib.sha256(raw).hexdigest()
    ):
        fail("R007B_PACKAGED_CAPTURE_RECEIPT_DRIFT")
    return path


def verify_glb_receipt(home: Path, report: Mapping[str, Any], run: Mapping[str, Any]) -> tuple[Path, dict[str, Any]]:
    receipt = require_mapping(report.get("v3_production_glb"), "R007B_PACKAGED_RESULT_GLB_RECEIPT_MISSING")
    relative = safe_relative(receipt.get("relative_path"), ".glb", code="R007B_PACKAGED_RESULT_GLB_PATH_INVALID")
    source = child_path(home / "WushenForge", relative, "R007B_PACKAGED_RESULT_GLB_MISSING")
    raw = source.read_bytes()
    if len(raw) < 20 or raw[:4] != b"glTF" or struct.unpack_from("<I", raw, 8)[0] != len(raw):
        fail("R007B_PACKAGED_RESULT_GLB_INVALID")
    reported_sha = require_sha(receipt.get("sha256"), "R007B_PACKAGED_RESULT_GLB_HASH_INVALID")
    run_sha = require_sha(run.get("result_glb_sha256"), "R007B_PACKAGED_RESULT_GLB_HASH_INVALID")
    if (
        receipt.get("byte_size") != len(raw)
        or hashlib.sha256(raw).hexdigest() != reported_sha
        or reported_sha != run_sha
    ):
        fail("R007B_PACKAGED_RESULT_GLB_RECEIPT_DRIFT")
    return source, copy.deepcopy(receipt)


def copy_verified(source: Path, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_suffix(f"{destination.suffix}.tmp")
    shutil.copyfile(source, temporary)
    temporary.replace(destination)


def normalize_run(
    *,
    home: Path,
    artifact_root: Path,
    staging: Path,
    report: Mapping[str, Any],
    reference_class: str,
) -> dict[str, Any]:
    """Copy a Rust-sealed class run into staging without inventing any lineage."""

    if report.get("schema_version") != WEBVIEW_SCHEMA or report.get("phase") != "initial" or report.get("ok") is not True:
        fail("R007B_PACKAGED_WEBVIEW_REPORT_INVALID")
    run = require_mapping(report.get(RUN_SCHEMA_KEY), "R007B_PACKAGED_VISUAL_RUN_MISSING")
    if run.get("reference_class") != reference_class:
        fail("R007B_PACKAGED_REFERENCE_CLASS_DRIFT")
    screenshots = require_mapping(run.get("screenshots"), "R007B_PACKAGED_SCREENSHOTS_INVALID")
    reference = require_mapping(screenshots.get("reference"), "R007B_PACKAGED_REFERENCE_CAPTURE_MISSING")
    result = require_mapping(screenshots.get("result"), "R007B_PACKAGED_RESULT_CAPTURE_MISSING")
    reference_relative = f"captures/{reference_class}/reference.png"
    result_relative = f"captures/{reference_class}/result.png"
    reference_source = verify_png_receipt(artifact_root, reference, expected_relative=reference_relative)
    result_source = verify_png_receipt(artifact_root, result, expected_relative=result_relative)
    glb_source, glb_receipt = verify_glb_receipt(home, report, run)
    copy_verified(reference_source, staging / reference_relative)
    copy_verified(result_source, staging / result_relative)
    glb_relative = f"artifacts/{reference_class}/result.glb"
    copy_verified(glb_source, staging / glb_relative)
    copied_glb = staging / glb_relative
    if hashlib.sha256(copied_glb.read_bytes()).hexdigest() != glb_receipt["sha256"]:
        fail("R007B_PACKAGED_RESULT_GLB_COPY_DRIFT")
    normalized = copy.deepcopy(run)
    normalized["result_glb"] = {
        "relative_path": glb_relative,
        "sha256": glb_receipt["sha256"],
        "byte_size": glb_receipt["byte_size"],
        "triangle_count": glb_receipt.get("triangle_count"),
        "complete_pbr_material_count": glb_receipt.get("complete_pbr_material_count"),
    }
    return normalized


def run_one_class(reference_class: str, staging: Path) -> dict[str, Any]:
    if _listener_pid() is not None:
        fail("R007B_PACKAGED_PORT_8000_OCCUPIED")
    with tempfile.TemporaryDirectory(prefix=f"forgecad_r007b_{reference_class}_") as raw:
        home = Path(raw)
        artifacts = safe_directory_under_home(home, home / "r007b-artifacts", "R007B_PACKAGED_ARTIFACT_ROOT_INVALID")
        environment = fresh_environment(home, home / "library", reference_class, artifacts)
        log = home / "WushenForge" / "agent.log"
        desktop_pid: int | None = None
        try:
            desktop_pid = start_packaged(home, environment)
            listener = _wait_for_native_health(desktop_pid)
            _assert(_is_descendant(listener, desktop_pid), "packaged sidecar is not desktop-owned")
            report = _wait_report(log, "initial")
            # Keep the pre-existing V003 → V1 → A005 V2 → R007B V3 contract
            # mandatory. The R007B visual manifest is additive evidence, not
            # a shortcut around the user-visible golden path.
            _validate_initial(report)
            return normalize_run(
                home=home,
                artifact_root=artifacts,
                staging=staging,
                report=report,
                reference_class=reference_class,
            )
        except ProducerFailure:
            raise
        except (AssertionError, EvidenceFailure) as exc:
            raise ProducerFailure(stable_rejection_code(reference_class, exc)) from exc
        finally:
            if desktop_pid is not None:
                _stop_desktop_and_listener(desktop_pid)
            if _listener_pid() is not None:
                fail("R007B_PACKAGED_SIDECAR_SURVIVED_SHUTDOWN")


def write_json(path: Path, value: Mapping[str, Any]) -> None:
    temporary = path.with_suffix(f"{path.suffix}.tmp")
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(path)


def produce(output: Path) -> dict[str, Any]:
    if output.exists() or output.is_symlink():
        fail("R007B_PACKAGED_OUTPUT_ALREADY_EXISTS")
    if not APP_BINARY.is_file():
        fail("R007B_PACKAGED_APP_NOT_BUILT")
    if _listener_pid() is not None:
        fail("R007B_PACKAGED_PORT_8000_OCCUPIED")
    if _macos_console_screen_locked():
        fail("R007B_PACKAGED_MACOS_SCREEN_LOCKED")
    output.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix=f".{output.name}.staging-", dir=output.parent))
    try:
        runs = [run_one_class(reference_class, staging) for reference_class in REFERENCE_CLASSES]
        manifest = {
            "schema_version": MANIFEST_SCHEMA,
            "status": "pass",
            "generated_at": utc_now(),
            "producer": {
                "schema_version": PRODUCER_SCHEMA,
                "runtime_kind": "packaged_tauri_webview",
                "fixture_or_proxy_used": False,
                "provider_network_calls": 0,
                "credential_reads": 0,
                "isolated_home_per_reference_class": True,
            },
            "visual_fidelity_validated": False,
            "formal_eligible": False,
            "m108b_status": "blocked",
            "runs": runs,
        }
        write_json(staging / "manifest.json", manifest)
        # Invoke the exact in-process validator only after all three raw Rust
        # receipts have been copied. It validates the bytes again, freshness,
        # distinct explanations/ceilings/results and the no-placeholder rules.
        validate_manifest(staging, manifest, now=datetime.now(timezone.utc), max_age=timedelta(hours=1))
        staging.replace(output)
        return {
            "schema_version": PRODUCER_SCHEMA,
            "status": "pass",
            "evidence": str(output.relative_to(ROOT)),
            "reference_classes": sorted(REFERENCE_CLASSES),
            "visual_fidelity_validated": False,
            "formal_eligible": False,
            "m108b_status": "blocked",
        }
    except BaseException:
        shutil.rmtree(staging, ignore_errors=True)
        raise


def self_test() -> int:
    """Exercise no-launch safety boundaries used before a real desktop run."""

    with tempfile.TemporaryDirectory(prefix="forgecad-r007b-producer-self-test-") as raw:
        home = Path(raw)
        artifact = safe_directory_under_home(home, home / "artifacts", "SELF_TEST_ARTIFACT")
        environment = fresh_environment(home, home / "library", "single_image", artifact)
        if environment[RUN_ENV] != "single_image" or environment[ARTIFACT_ENV] != str(artifact):
            fail("R007B_PACKAGED_SELF_TEST_ENV_INVALID")
        if any(name in environment for name in ("FORGECAD_AGENT_API_KEY", "FORGECAD_AGENT_BASE_URL", "FORGECAD_AGENT_MODEL")):
            fail("R007B_PACKAGED_SELF_TEST_SECRET_ENV_LEAK")
        try:
            safe_relative("../escape.png", ".png", code="SELF_TEST_ESCAPE")
        except ProducerFailure as exc:
            if str(exc) != "SELF_TEST_ESCAPE":
                raise
        else:
            fail("R007B_PACKAGED_SELF_TEST_ESCAPE_ACCEPTED")
        try:
            safe_directory_under_home(home, home.parent / "outside", "SELF_TEST_OUTSIDE")
        except ProducerFailure as exc:
            if str(exc) != "SELF_TEST_OUTSIDE":
                raise
        else:
            fail("R007B_PACKAGED_SELF_TEST_OUTSIDE_ACCEPTED")
    print(json.dumps({
        "schema_version": PRODUCER_SCHEMA,
        "status": "pass",
        "mode": "self_test",
        "reference_classes": sorted(REFERENCE_CLASSES),
        "real_packaged_app_started": False,
        "visual_fidelity_validated": False,
        "formal_eligible": False,
        "m108b_status": "blocked",
    }, ensure_ascii=False, sort_keys=True))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Produce fresh three-class R007B packaged WebView evidence.")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--self-test", action="store_true")
    arguments = parser.parse_args()
    try:
        if arguments.self_test:
            return self_test()
        output = arguments.output.resolve(strict=False)
        try:
            output.relative_to(ROOT / "output")
        except ValueError:
            fail("R007B_PACKAGED_OUTPUT_OUTSIDE_OUTPUT_DIRECTORY")
        report = produce(output)
        print(json.dumps(report, ensure_ascii=False, sort_keys=True))
        return 0
    except ProducerFailure as exc:
        print(json.dumps({
            "schema_version": PRODUCER_SCHEMA,
            "status": "fail",
            "error": str(exc),
            "visual_fidelity_validated": False,
            "formal_eligible": False,
            "m108b_status": "blocked",
        }, ensure_ascii=False, sort_keys=True), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
