#!/usr/bin/env python3
"""Run the explicit packaged DeepSeek arm-continuation acceptance.

The command is dry-run by default.  A live run performs three bounded phases
against one temporary Rust Library: deterministic arm seed, one DeepSeek
AssemblyDelta continuation, then a fresh-process resume/export readback.  The
script never reads or prints a provider key; the packaged Rust app resolves the
credential from its existing Keychain entry.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time
from typing import Any, Sequence

from smoke_packaged_tauri_alpha import (
    APP_BINARY,
    APP_BUNDLE,
    _desktop_pids,
    _is_descendant,
    _listener_pid,
    _stop_desktop_and_listener,
    _wait_for_native_health,
)


ROOT = Path(__file__).resolve().parents[1]
LIVE_CONFIRMATION = "I_UNDERSTAND_THIS_MAY_INCUR_PROVIDER_COST"
RUN_ID = re.compile(r"^live_[A-Za-z0-9_-]{7,75}$")
DELTA_SCHEMA = "ForgeCADDeepSeekDeltaAcceptance@1"
PACKAGED_SCHEMA = "ForgeCADArmMvpPackagedProtocolProof@3"
FORBIDDEN_KEYS = {"api_key", "secret", "base_url", "model", "brief", "prompt", "response"}


class AcceptanceError(RuntimeError):
    pass


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run the packaged DeepSeek AssemblyDelta acceptance.")
    parser.add_argument("--confirm-live-provider", action="store_true")
    parser.add_argument("--accept-network", action="store_true")
    parser.add_argument("--confirmation")
    parser.add_argument("--run-id")
    parser.add_argument("--output", type=Path)
    parser.add_argument(
        "--foreground-launch",
        action="store_true",
        help="launch the release binary directly so macOS Keychain authorization is visible",
    )
    return parser


def _validate_live(args: argparse.Namespace) -> tuple[str, Path]:
    values = (args.confirm_live_provider, args.accept_network, args.confirmation, args.run_id, args.output)
    if not any(values):
        raise AcceptanceError("LIVE_CONFIRMATION_REQUIRED")
    if not all(values) or args.confirmation != LIVE_CONFIRMATION:
        raise AcceptanceError("LIVE_CONFIRMATION_REQUIRED")
    if not isinstance(args.run_id, str) or RUN_ID.fullmatch(args.run_id) is None:
        raise AcceptanceError("LIVE_RUN_ID_INVALID")
    output = args.output.expanduser()
    if not output.is_absolute() or output.suffix.lower() != ".json":
        raise AcceptanceError("LIVE_OUTPUT_INVALID")
    return args.run_id, output


def _dry_report(reason: str) -> dict[str, object]:
    return {
        "schema_version": "ForgeCADDeepSeekDeltaAcceptanceLaunch@1",
        "status": "dry_run",
        "network_calls_made": 0,
        "credential_reads": 0,
        "app_launched": False,
        "reason": reason,
    }


def _clean_environment() -> dict[str, str]:
    environment = os.environ.copy()
    for name in tuple(environment):
        if name.startswith("FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE") or name.startswith("FORGECAD_MVP_ARM_PACKAGED"):
            environment.pop(name, None)
        if name in {
            "FORGECAD_MVP_OFFLINE_ARM",
            "FORGECAD_DISABLE_PROVIDER_CONFIG",
            "FORGECAD_AGENT_API_KEY",
            "FORGECAD_AGENT_API_KEY_FILE",
            "FORGECAD_AGENT_BASE_URL",
            "FORGECAD_AGENT_MODEL",
            "FORGECAD_AGENT_PROVIDER",
            "FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE",
            "FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE",
        }:
            environment.pop(name, None)
    environment["WUSHEN_AGENT_RUNTIME_MODE"] = "packaged-sidecar"
    environment["FORGECAD_CONCEPT_WORKER_ENABLED"] = "0"
    environment["WUSHEN_LOCAL_WORKER_ENABLED"] = "0"
    return environment


def _start(environment: dict[str, str], forwarded: set[str], foreground_launch: bool) -> int:
    existing = _desktop_pids()
    if foreground_launch:
        process = subprocess.Popen(
            [str(APP_BINARY)],
            cwd=ROOT,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        for _ in range(100):
            created = _desktop_pids() - existing
            if created:
                return max(created)
            if process.poll() is not None:
                raise AcceptanceError("LIVE_APP_START_FAILED")
            time.sleep(0.1)
        raise AcceptanceError("LIVE_APP_START_FAILED")
    command = ["open", "-n"]
    for name in sorted(forwarded):
        if name in environment:
            command.extend(["--env", f"{name}={environment[name]}"])
    command.append(str(APP_BUNDLE))
    subprocess.run(command, cwd=ROOT, env=environment, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for _ in range(100):
        created = _desktop_pids() - existing
        if created:
            return max(created)
        time.sleep(0.1)
    raise AcceptanceError("LIVE_APP_START_FAILED")


def _read_report(path: Path, timeout: float = 1500.0) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.is_file():
            try:
                report = json.loads(path.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError):
                time.sleep(0.2)
                continue
            if isinstance(report, dict):
                return report
        time.sleep(0.2)
    raise AcceptanceError("LIVE_REPORT_TIMEOUT")


def _launch_and_read(
    environment: dict[str, str],
    forwarded: set[str],
    report_path: Path,
    foreground_launch: bool,
) -> dict[str, Any]:
    if _listener_pid() is not None:
        raise AcceptanceError("LIVE_PORT_8000_OCCUPIED")
    desktop_pid = _start(environment, forwarded, foreground_launch)
    try:
        listener = _wait_for_native_health(desktop_pid)
        if not _is_descendant(listener, desktop_pid):
            raise AcceptanceError("LIVE_SIDECAR_OWNERSHIP_INVALID")
        return _read_report(report_path)
    finally:
        _stop_desktop_and_listener(desktop_pid)


def _validate_seed(report: dict[str, Any]) -> None:
    if report.get("schema_version") != PACKAGED_SCHEMA or report.get("status") != "pass":
        raise AcceptanceError("OFFLINE_ARM_SEED_FAILED")
    project_id = report.get("project_id")
    asset_id = report.get("c110d", {}).get("v4_asset_version_id") if isinstance(report.get("c110d"), dict) else None
    if not isinstance(project_id, str) or not project_id or not isinstance(asset_id, str) or not asset_id:
        raise AcceptanceError("OFFLINE_ARM_SEED_INVALID")


def _validate_delta(report: dict[str, Any], run_id: str) -> None:
    encoded = json.dumps(report, ensure_ascii=False, sort_keys=True)
    if report.get("schema_version") != DELTA_SCHEMA or report.get("status") != "pass":
        raise AcceptanceError("LIVE_DELTA_PROBE_FAILED")
    if report.get("run_id_sha256") != hashlib.sha256(run_id.encode("utf-8")).hexdigest():
        raise AcceptanceError("LIVE_REPORT_IDENTITY_DRIFT")
    if report.get("provider_owner") != "rust_desktop" or report.get("credential_source") != "rust_provider_credential_store":
        raise AcceptanceError("LIVE_RUST_OWNERSHIP_DRIFT")
    if report.get("network_calls_made") != 1:
        raise AcceptanceError("LIVE_NETWORK_EVIDENCE_INVALID")
    delta = report.get("delta")
    if not isinstance(delta, dict) or delta.get("status") != "completed":
        raise AcceptanceError("LIVE_DELTA_PHASE_INVALID")
    if delta.get("network_call_made") is not True or delta.get("delta_bound") is not True:
        raise AcceptanceError("LIVE_DELTA_NOT_BOUND")
    if delta.get("confirmed") is not True or delta.get("asset_or_snapshot_writes") != 1:
        raise AcceptanceError("LIVE_DELTA_CONFIRM_INVALID")
    if not isinstance(delta.get("preview_glb_sha256"), str) or len(delta["preview_glb_sha256"]) != 64:
        raise AcceptanceError("LIVE_DELTA_GLB_INVALID")
    if not isinstance(delta.get("preview_triangle_count"), int) or delta["preview_triangle_count"] <= 0:
        raise AcceptanceError("LIVE_DELTA_GLB_INVALID")
    if not isinstance(report.get("base_asset_version_id"), str) or not isinstance(report.get("new_asset_version_id"), str):
        raise AcceptanceError("LIVE_DELTA_LINEAGE_INVALID")
    if report.get("base_asset_version_id") == report.get("new_asset_version_id"):
        raise AcceptanceError("LIVE_DELTA_LINEAGE_INVALID")
    if report.get("no_raw_prompt_or_response") is not True or report.get("no_key_or_provider_endpoint") is not True:
        raise AcceptanceError("LIVE_REDACTION_EVIDENCE_INVALID")
    if any(f'"{key}"' in encoded for key in FORBIDDEN_KEYS):
        raise AcceptanceError("LIVE_REPORT_REDACTION_INVALID")


def _validate_resume(report: dict[str, Any], run_id: str, expected_asset: str) -> None:
    encoded = json.dumps(report, ensure_ascii=False, sort_keys=True)
    if report.get("schema_version") != DELTA_SCHEMA or report.get("status") != "pass":
        raise AcceptanceError("LIVE_RESUME_PROBE_FAILED")
    if report.get("run_id_sha256") != hashlib.sha256(run_id.encode("utf-8")).hexdigest():
        raise AcceptanceError("LIVE_REPORT_IDENTITY_DRIFT")
    restart = report.get("restart")
    if not isinstance(restart, dict) or restart.get("restarted") is not True or restart.get("confirmed") is not True:
        raise AcceptanceError("LIVE_RESUME_INVALID")
    if report.get("new_asset_version_id") != expected_asset or restart.get("network_call_made") is not False:
        raise AcceptanceError("LIVE_RESUME_LINEAGE_INVALID")
    if report.get("no_raw_prompt_or_response") is not True or report.get("no_key_or_provider_endpoint") is not True:
        raise AcceptanceError("LIVE_REDACTION_EVIDENCE_INVALID")
    if any(f'"{key}"' in encoded for key in FORBIDDEN_KEYS):
        raise AcceptanceError("LIVE_REPORT_REDACTION_INVALID")


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        run_id, output = _validate_live(args)
    except AcceptanceError as error:
        print(json.dumps(_dry_report(str(error)), ensure_ascii=False, sort_keys=True))
        return 0
    if not APP_BINARY.is_file():
        raise AcceptanceError("LIVE_APP_NOT_BUILT")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="forgecad_deepseek_delta_") as temporary:
        library = Path(temporary) / "library"
        seed_path = Path(temporary) / "packaged-seed.json"
        seed_environment = _clean_environment()
        seed_environment.update({
            "WUSHEN_LIBRARY_ROOT": str(library),
            "FORGECAD_DISABLE_PROVIDER_CONFIG": "1",
            "FORGECAD_MVP_OFFLINE_ARM": "1",
            "FORGECAD_MVP_ARM_PACKAGED_PROBE": "1",
            "FORGECAD_MVP_ARM_PACKAGED_PROBE_OUTPUT": str(seed_path),
        })
        seed = _launch_and_read(seed_environment, {
            "WUSHEN_LIBRARY_ROOT", "WUSHEN_AGENT_RUNTIME_MODE", "FORGECAD_DISABLE_PROVIDER_CONFIG",
            "FORGECAD_MVP_OFFLINE_ARM", "FORGECAD_CONCEPT_WORKER_ENABLED", "WUSHEN_LOCAL_WORKER_ENABLED",
            "FORGECAD_MVP_ARM_PACKAGED_PROBE", "FORGECAD_MVP_ARM_PACKAGED_PROBE_OUTPUT",
        }, seed_path, args.foreground_launch)
        _validate_seed(seed)
        seed_path.write_text(json.dumps(seed, ensure_ascii=False, sort_keys=True) + "\n", encoding="utf-8")

        delta_environment = _clean_environment()
        delta_environment.update({
            "WUSHEN_LIBRARY_ROOT": str(library),
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE": "1",
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_CONFIRM": LIVE_CONFIRMATION,
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_RUN_ID": run_id,
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_INPUT": str(seed_path),
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_OUTPUT": str(output),
            # The live continuation is an explicit cost opt-in.  This removes
            # the ordinary 256K/100K Turn ceilings while retaining the Rust
            # Action Loop's reviewed wall-time, tool-count, cancellation, and
            # per-request maximums.  The packaged bridge also requires the
            # exact confirmation token before accepting this override.
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_BUDGET_OVERRIDE": "1",
        })
        delta = _launch_and_read(delta_environment, {
            "WUSHEN_LIBRARY_ROOT", "WUSHEN_AGENT_RUNTIME_MODE", "FORGECAD_CONCEPT_WORKER_ENABLED",
            "WUSHEN_LOCAL_WORKER_ENABLED", "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE",
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_CONFIRM", "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_RUN_ID",
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_INPUT", "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_OUTPUT",
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_BUDGET_OVERRIDE",
        }, output, args.foreground_launch)
        _validate_delta(delta, run_id)
        resume_path = Path(temporary) / "resume.json"
        resume_environment = _clean_environment()
        resume_environment.update({
            "WUSHEN_LIBRARY_ROOT": str(library),
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE": "1",
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_CONFIRM": LIVE_CONFIRMATION,
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_RUN_ID": run_id,
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_INPUT": str(output),
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_OUTPUT": str(resume_path),
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_RESUME": "1",
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_BUDGET_OVERRIDE": "1",
        })
        resumed = _launch_and_read(resume_environment, {
            "WUSHEN_LIBRARY_ROOT", "WUSHEN_AGENT_RUNTIME_MODE", "FORGECAD_CONCEPT_WORKER_ENABLED",
            "WUSHEN_LOCAL_WORKER_ENABLED", "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE",
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_CONFIRM", "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_RUN_ID",
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_INPUT", "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_OUTPUT",
            "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_RESUME", "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_BUDGET_OVERRIDE",
        }, resume_path, args.foreground_launch)
        _validate_resume(resumed, run_id, str(delta["new_asset_version_id"]))
    print(json.dumps({"schema_version": DELTA_SCHEMA, "status": "pass", "delta": delta, "resume": resumed}, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except AcceptanceError as error:
        print(json.dumps({"schema_version": "ForgeCADDeepSeekDeltaAcceptanceLaunch@1", "status": "fail", "error": str(error)}, ensure_ascii=False, sort_keys=True))
        raise SystemExit(2) from None
    except KeyboardInterrupt:
        print(json.dumps({"schema_version": "ForgeCADDeepSeekDeltaAcceptanceLaunch@1", "status": "cancelled", "error": "LIVE_RUN_CANCELLED"}, ensure_ascii=False, sort_keys=True))
        raise SystemExit(130) from None
