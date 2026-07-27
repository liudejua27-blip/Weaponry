#!/usr/bin/env python3
"""Launch the explicit, live DeepSeek ForgeVisualProgram acceptance.

Without every confirmation flag this is a zero-network dry-run.  This launcher
does not read credentials, Provider configuration, or any request/response
contents.  The release app remains the only code allowed to resolve its own
Rust-owned credential store.
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
SCHEMA_VERSION = "ForgeCADDeepSeekForgeVisualAcceptanceLaunch@1"
REPORT_SCHEMA_VERSION = "ForgeCADDeepSeekForgeVisualAcceptance@1"
LIVE_CONFIRMATION = "I_UNDERSTAND_THIS_MAY_INCUR_PROVIDER_COST"
RUN_ID = re.compile(r"^live_[A-Za-z0-9_-]{7,75}$")
FORBIDDEN_REPORT_KEYS = {"api_key", "secret", "base_url", "endpoint", "model", "prompt", "response"}
SAFE_ERROR_CODES = {
    "FORGE_VISUAL_LIVE_PROVIDER_DISABLED_BY_OFFLINE_MODE",
    "FORGE_VISUAL_LIVE_RUNTIME_UNAVAILABLE",
    "FORGE_VISUAL_LIVE_PROJECT_CREATE_REJECTED",
    "FORGE_VISUAL_LIVE_PROJECT_ID_MISSING",
    "FORGE_VISUAL_LIVE_THREAD_CREATE_REJECTED",
    "FORGE_VISUAL_LIVE_THREAD_ID_MISSING",
    "FORGE_VISUAL_LIVE_TURN_START_REJECTED",
    "FORGE_VISUAL_LIVE_TURN_RESULT_MISSING",
    "FORGE_VISUAL_LIVE_TURN_ID_MISSING",
    "FORGE_VISUAL_LIVE_CANCELLATION_ID_MISSING",
    "FORGE_VISUAL_LIVE_CANCELLATION_TOKEN_MISSING",
    "FORGE_VISUAL_LIVE_ACTIVE_DESIGN_READ_REJECTED",
    "FORGE_VISUAL_LIVE_ACTIVE_DESIGN_NOT_EMPTY",
    "FORGE_VISUAL_LIVE_ACTIVE_DESIGN_REVISION_MISSING",
    "FORGE_VISUAL_LIVE_TURN_TIMEOUT",
    "FORGE_VISUAL_LIVE_TURN_NOT_COMPLETED",
    "FORGE_VISUAL_LIVE_NETWORK_EVIDENCE_MISSING",
    "FORGE_VISUAL_LIVE_AUTHOR_MISSING",
    "FORGE_VISUAL_LIVE_RUST_COMPLETION_MISSING",
    "FORGE_VISUAL_LIVE_PREVIEW_SIDE_EFFECT",
    "FORGE_VISUAL_LIVE_SINGLE_RESULT_MISSING",
    "FORGE_VISUAL_LIVE_PREVIEW_ID_MISSING",
    "FORGE_VISUAL_LIVE_PREVIEW_SHA_MISSING",
    "FORGE_VISUAL_LIVE_PREVIEW_READ_REJECTED",
    "FORGE_VISUAL_LIVE_PREVIEW_HASH_INVALID",
    "FORGE_VISUAL_LIVE_CONFIRM_REJECTED",
    "FORGE_VISUAL_LIVE_CONFIRM_VERSION_MISSING",
    "FORGE_VISUAL_LIVE_SNAPSHOT_READ_REJECTED",
    "FORGE_VISUAL_LIVE_SNAPSHOT_DRIFT",
    "FORGE_VISUAL_LIVE_EXPORT_REJECTED",
    "FORGE_VISUAL_LIVE_EXPORT_BYTES_MISSING",
    "FORGE_VISUAL_LIVE_EXPORT_SHA_MISSING",
    "FORGE_VISUAL_LIVE_EXPORT_HASH_INVALID",
}


class AcceptanceError(RuntimeError):
    pass


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run the live DeepSeek ForgeVisualProgram acceptance.")
    parser.add_argument("--confirm-live-provider", action="store_true")
    parser.add_argument("--accept-network", action="store_true")
    parser.add_argument("--confirmation")
    parser.add_argument("--run-id")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--foreground-launch", action="store_true")
    return parser


def _dry_report(reason: str) -> dict[str, object]:
    return {
        "schema_version": SCHEMA_VERSION,
        "status": "dry_run",
        "network_calls_made": 0,
        "credential_reads": 0,
        "app_launched": False,
        "reason": reason,
    }


def _validate_live(args: argparse.Namespace) -> tuple[str, Path]:
    values = (args.confirm_live_provider, args.accept_network, args.confirmation, args.run_id, args.output)
    if not any(values) or not all(values) or args.confirmation != LIVE_CONFIRMATION:
        raise AcceptanceError("FORGE_VISUAL_LIVE_CONFIRMATION_REQUIRED")
    if not isinstance(args.run_id, str) or RUN_ID.fullmatch(args.run_id) is None:
        raise AcceptanceError("FORGE_VISUAL_LIVE_RUN_ID_INVALID")
    output = args.output.expanduser()
    if not output.is_absolute() or output.suffix.lower() != ".json":
        raise AcceptanceError("FORGE_VISUAL_LIVE_OUTPUT_INVALID")
    return args.run_id, output


def _walk_keys(value: Any):
    if isinstance(value, dict):
        for key, child in value.items():
            yield str(key).lower()
            yield from _walk_keys(child)
    elif isinstance(value, list):
        for child in value:
            yield from _walk_keys(child)


def _validate_report(report: dict[str, Any], run_id: str) -> dict[str, Any]:
    if report.get("schema_version") != REPORT_SCHEMA_VERSION:
        raise AcceptanceError("FORGE_VISUAL_LIVE_REPORT_SCHEMA_INVALID")
    if report.get("run_id_sha256") != hashlib.sha256(run_id.encode()).hexdigest():
        raise AcceptanceError("FORGE_VISUAL_LIVE_REPORT_IDENTITY_DRIFT")
    if set(_walk_keys(report)) & FORBIDDEN_REPORT_KEYS:
        raise AcceptanceError("FORGE_VISUAL_LIVE_REPORT_REDACTION_INVALID")
    if report.get("no_raw_prompt_or_response") is not True or report.get("no_key_or_provider_endpoint") is not True:
        raise AcceptanceError("FORGE_VISUAL_LIVE_REDACTION_EVIDENCE_INVALID")
    if report.get("status") != "pass":
        code = report.get("error_code")
        if code not in SAFE_ERROR_CODES:
            raise AcceptanceError("FORGE_VISUAL_LIVE_REPORT_REDACTION_INVALID")
        raise AcceptanceError("FORGE_VISUAL_LIVE_RUST_PROBE_FAILED")
    expected = {
        "execution_mode": "live_explicit_opt_in",
        "provider_owner": "rust_desktop",
        "credential_source": "rust_provider_credential_store",
        "network_calls_made": 1,
    }
    if any(report.get(key) != value for key, value in expected.items()):
        raise AcceptanceError("FORGE_VISUAL_LIVE_OWNERSHIP_OR_NETWORK_INVALID")
    evidence = report.get("visual_program_turn")
    if not isinstance(evidence, dict) or evidence.get("status") != "completed":
        raise AcceptanceError("FORGE_VISUAL_LIVE_PHASE_INVALID")
    required = {
        "network_call_made",
        "author_forge_visual_program_completed",
        "rust_compile_readback_completed",
        "rust_eight_view_render_completed",
        "rust_evaluate_completed",
        "single_result_ready",
        "preview_hash_matches_bytes_and_header",
        "confirmed_asset_created",
        "snapshot_advanced",
        "export_hash_matches_bytes_json_and_header",
    }
    if any(evidence.get(key) is not True for key in required):
        raise AcceptanceError("FORGE_VISUAL_LIVE_E2E_EVIDENCE_INVALID")
    if evidence.get("author_source_mode") not in {
        "provider_authoring_ir",
        "provider_program",
        "reviewed_fallback",
    }:
        raise AcceptanceError("FORGE_VISUAL_LIVE_E2E_EVIDENCE_INVALID")
    stages = evidence.get("completed_tool_stages")
    if not isinstance(stages, list) or not {
        "author_forge_visual_program", "build_candidate_geometry", "compile_readback_candidate",
        "render_candidate_views", "evaluate_candidate", "prepare_candidate_preview",
    }.issubset(set(stages)):
        raise AcceptanceError("FORGE_VISUAL_LIVE_TOOL_SEQUENCE_INVALID")
    return report


def _read_report(output: Path) -> dict[str, Any]:
    deadline = time.monotonic() + 900
    while time.monotonic() < deadline:
        if output.is_file():
            try:
                value = json.loads(output.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError):
                time.sleep(0.2)
                continue
            if isinstance(value, dict):
                return value
        time.sleep(0.2)
    raise AcceptanceError("FORGE_VISUAL_LIVE_REPORT_TIMEOUT")


def _environment(library: Path, run_id: str, output: Path) -> dict[str, str]:
    environment = os.environ.copy()
    for name in tuple(environment):
        if name.startswith("FORGECAD_DEEPSEEK_FORGE_VISUAL_ACCEPTANCE") or name in {
            "FORGECAD_MVP_OFFLINE_ARM", "FORGECAD_DISABLE_PROVIDER_CONFIG",
            "FORGECAD_AGENT_API_KEY", "FORGECAD_AGENT_API_KEY_FILE",
            "FORGECAD_AGENT_BASE_URL", "FORGECAD_AGENT_MODEL", "FORGECAD_AGENT_PROVIDER",
        }:
            environment.pop(name, None)
    environment.update({
        "WUSHEN_LIBRARY_ROOT": str(library),
        "WUSHEN_AGENT_RUNTIME_MODE": "packaged-sidecar",
        "FORGECAD_DEEPSEEK_FORGE_VISUAL_ACCEPTANCE": "1",
        "FORGECAD_DEEPSEEK_FORGE_VISUAL_ACCEPTANCE_CONFIRM": LIVE_CONFIRMATION,
        "FORGECAD_DEEPSEEK_FORGE_VISUAL_ACCEPTANCE_RUN_ID": run_id,
        "FORGECAD_DEEPSEEK_FORGE_VISUAL_ACCEPTANCE_OUTPUT": str(output),
        # This opt-in runner retains all Rust validation and cancellation
        # limits, while avoiding a test harness ceiling that could turn a
        # legitimate ForgeVisualProgram response into a false negative.
        "FORGECAD_DEEPSEEK_FORGE_VISUAL_ACCEPTANCE_BUDGET_OVERRIDE": "1",
    })
    return environment


def _start(environment: dict[str, str], foreground_launch: bool) -> int:
    existing = _desktop_pids()
    if foreground_launch:
        process = subprocess.Popen(
            [str(APP_BINARY)], cwd=ROOT, env=environment, stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True,
        )
        for _ in range(100):
            created = _desktop_pids() - existing
            if created:
                return max(created)
            if process.poll() is not None:
                raise AcceptanceError("FORGE_VISUAL_LIVE_APP_START_FAILED")
            time.sleep(0.1)
        raise AcceptanceError("FORGE_VISUAL_LIVE_APP_START_FAILED")
    command = ["open", "-n"]
    for name in (
        "WUSHEN_LIBRARY_ROOT",
        "WUSHEN_AGENT_RUNTIME_MODE",
        "FORGECAD_DEEPSEEK_FORGE_VISUAL_ACCEPTANCE",
        "FORGECAD_DEEPSEEK_FORGE_VISUAL_ACCEPTANCE_CONFIRM",
        "FORGECAD_DEEPSEEK_FORGE_VISUAL_ACCEPTANCE_RUN_ID",
        "FORGECAD_DEEPSEEK_FORGE_VISUAL_ACCEPTANCE_OUTPUT",
        "FORGECAD_DEEPSEEK_FORGE_VISUAL_ACCEPTANCE_BUDGET_OVERRIDE",
    ):
        command.extend(["--env", f"{name}={environment[name]}"])
    command.append(str(APP_BUNDLE))
    subprocess.run(command, cwd=ROOT, env=environment, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for _ in range(100):
        created = _desktop_pids() - existing
        if created:
            return max(created)
        time.sleep(0.1)
    raise AcceptanceError("FORGE_VISUAL_LIVE_APP_START_FAILED")


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        run_id, output = _validate_live(args)
    except AcceptanceError as error:
        print(json.dumps(_dry_report(str(error)), ensure_ascii=False, sort_keys=True))
        return 0
    report: dict[str, Any] | None = None
    try:
        if not APP_BINARY.is_file():
            raise AcceptanceError("FORGE_VISUAL_LIVE_APP_NOT_BUILT")
        if _desktop_pids() or _listener_pid() is not None:
            raise AcceptanceError("FORGE_VISUAL_LIVE_CLOSE_RUNNING_APP_FIRST")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.unlink(missing_ok=True)
        with tempfile.TemporaryDirectory(prefix="forgecad_deepseek_forge_visual_") as temporary:
            desktop_pid = _start(_environment(Path(temporary) / "library", run_id, output), args.foreground_launch)
            try:
                try:
                    listener = _wait_for_native_health(desktop_pid)
                except AssertionError as error:
                    raise AcceptanceError("FORGE_VISUAL_LIVE_SIDECAR_HEALTH_INVALID") from error
                if not _is_descendant(listener, desktop_pid):
                    raise AcceptanceError("FORGE_VISUAL_LIVE_SIDECAR_OWNERSHIP_INVALID")
                report = _read_report(output)
                _validate_report(report, run_id)
            finally:
                _stop_desktop_and_listener(desktop_pid)
        print(json.dumps(report, ensure_ascii=False, sort_keys=True))
        return 0
    except AcceptanceError as error:
        payload: dict[str, object] = {
            "schema_version": SCHEMA_VERSION,
            "status": "fail",
            "network_calls_made": report.get("network_calls_made", 0) if isinstance(report, dict) else 0,
            "error_code": str(error),
        }
        if isinstance(report, dict) and report.get("error_code") in SAFE_ERROR_CODES:
            payload["safe_probe_error_code"] = report["error_code"]
        print(json.dumps(payload, ensure_ascii=False, sort_keys=True))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
