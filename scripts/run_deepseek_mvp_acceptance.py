#!/usr/bin/env python3
"""Launch the explicit Rust-native DeepSeek mechanical-arm acceptance probe.

No argument, missing confirmation, or invalid caller-owned output path is a
zero-network dry-run.  A live run only launches the already-built macOS app;
the app itself resolves the existing credential through Rust's Keychain store.
This script never reads a Keychain, Provider configuration, secret file, or
environment API key, and it never prints the process environment.
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
SCHEMA_VERSION = "ForgeCADDeepSeekMvpAcceptanceLaunch@1"
REPORT_SCHEMA_VERSION = "ForgeCADDeepSeekMvpAcceptance@1"
LIVE_CONFIRMATION = "I_UNDERSTAND_THIS_MAY_INCUR_PROVIDER_COST"
RUN_ID = re.compile(r"^live_[A-Za-z0-9_-]{7,75}$")
FORBIDDEN_REPORT_KEYS = {"api_key", "secret", "base_url", "model", "brief", "prompt", "response"}
SAFE_PHASE_STATUSES = {"not_run", "completed", "cancelled", "failed", "failed_before_terminal", "terminal_unknown"}
SAFE_FAILURE_CATEGORIES = {
    "native_protocol", "provider_execution", "provider_preflight", "provider_configuration",
    "product_tool", "product_tool_schema", "product_tool_budget", "token_budget", "cost_budget",
    "wall_time_budget", "cancelled", "duplicate_tool_call", "permanent_write_rejected",
    "item_event_persistence", "native_runtime", "unclassified_terminal_failure",
}
SAFE_PROBE_ERROR_CODES = {
    "LIVE_PROVIDER_DISABLED_BY_OFFLINE_MODE", "LIVE_RUNTIME_UNAVAILABLE",
    "LIVE_PROJECT_CREATE_REJECTED", "LIVE_PROJECT_ID_MISSING", "LIVE_THREAD_CREATE_REJECTED",
    "LIVE_THREAD_ID_MISSING", "LIVE_TURN_START_REJECTED", "LIVE_TURN_START_RESULT_MISSING",
    "LIVE_TURN_ID_MISSING", "LIVE_CANCELLATION_ID_MISSING", "LIVE_CANCELLATION_TOKEN_MISSING",
    "LIVE_ACTIVE_DESIGN_READ_REJECTED", "LIVE_ACTIVE_ASSET_SIDE_EFFECT",
    "LIVE_ACTIVE_DESIGN_REVISION_MISSING", "LIVE_TURN_TIMEOUT", "LIVE_TURN_NOT_EPHEMERAL_COMPLETION",
    "LIVE_TURN_SNAPSHOT_SIDE_EFFECT", "LIVE_TURN_NETWORK_ATTEMPT_MISSING", "LIVE_CANCEL_REJECTED",
    "LIVE_CANCEL_NOT_ACCEPTED", "LIVE_CANCEL_TIMEOUT", "LIVE_CANCEL_TERMINAL_DRIFT",
    "LIVE_CANCEL_SNAPSHOT_SIDE_EFFECT", "LIVE_TURN_ARM_INTENT_MISSING",
    "LIVE_FAILURE_PROBE_REJECTED", "LIVE_FAILURE_NOT_FAIL_CLOSED",
}
SAFE_PHASE_ERROR_CODES = {
    "UNSUPPORTED_PROVIDER_REJECTED",
    "PROVIDER_INVALID_REQUEST", "PROVIDER_AUTHENTICATION_FAILED", "PROVIDER_BALANCE_REQUIRED",
    "PROVIDER_RATE_LIMITED", "PROVIDER_SERVER_UNAVAILABLE", "PROVIDER_TIMEOUT",
    "PROVIDER_TRANSPORT_FAILED", "PROVIDER_EMPTY_CONTENT", "PROVIDER_EMPTY_JSON",
    "PROVIDER_INVALID_JSON", "PROVIDER_SCHEMA_MISMATCH", "PROVIDER_OUTPUT_TRUNCATED",
    "PROVIDER_CONTENT_FILTERED", "PROVIDER_SYSTEM_RESOURCE_UNAVAILABLE", "PROVIDER_CANCELLED",
    "PROVIDER_SCHEMA_RESPONSE_NOT_SSE", "PROVIDER_SCHEMA_MISSING_CHOICES",
    "PROVIDER_SCHEMA_CHOICE_INVALID", "PROVIDER_SCHEMA_MISSING_DELTA",
    "PROVIDER_SCHEMA_TOOL_DELTA_ARRAY", "PROVIDER_SCHEMA_TOOL_DELTA_INVALID",
    "PROVIDER_SCHEMA_TOOL_INDEX_INVALID", "PROVIDER_SCHEMA_TOOL_FUNCTION_INVALID",
    "PROVIDER_SCHEMA_TOOL_REQUIRED_FIELD", "PROVIDER_SCHEMA_TOOL_ARGUMENTS_OBJECT",
    "PROVIDER_SCHEMA_TOOL_ARGUMENTS_INVALID_JSON",
    "PROVIDER_SCHEMA_USAGE_MISSING", "PROVIDER_SCHEMA_FINISH_MISSING",
    "PROVIDER_SCHEMA_REASONING_MISSING", "PROVIDER_SCHEMA_STOP_WITH_TOOLS",
    "PROVIDER_SCHEMA_RESPONSE_TOO_LARGE", "PROVIDER_SCHEMA_SSE_LINE_TOO_LARGE",
    "PROVIDER_SCHEMA_SSE_EVENT_TOO_LARGE", "PROVIDER_SCHEMA_SSE_FIELD_INVALID",
    "PROVIDER_SCHEMA_SSE_DUPLICATE_DONE", "PROVIDER_SCHEMA_SSE_DATA_AFTER_DONE",
    "PROVIDER_SCHEMA_SSE_OBJECT_INVALID", "PROVIDER_SCHEMA_MULTI_CHOICE",
    "PROVIDER_SCHEMA_USAGE_ORDER", "PROVIDER_SCHEMA_DATA_AFTER_USAGE",
    "PROVIDER_SCHEMA_FINISH_TYPE", "PROVIDER_SCHEMA_FINISH_UNSUPPORTED",
    "PROVIDER_SCHEMA_FINISH_CONFLICT", "PROVIDER_SCHEMA_CONTENT_TOO_LARGE",
    "PROVIDER_SCHEMA_REASONING_TOO_LARGE", "PROVIDER_SCHEMA_TOOL_DELTA_TOO_MANY",
    "PROVIDER_SCHEMA_TOOL_TYPE", "PROVIDER_SCHEMA_TOOL_ID_TOO_LARGE",
    "PROVIDER_SCHEMA_TOOL_NAME_TOO_LARGE", "PROVIDER_SCHEMA_TOOL_ARGUMENTS_TOO_LARGE",
    "PROVIDER_SCHEMA_TOOL_TOO_MANY", "PROVIDER_SCHEMA_DONE_MISSING",
    "PROVIDER_SCHEMA_USAGE_OBJECT", "PROVIDER_SCHEMA_USAGE_PROMPT_MISSING",
    "PROVIDER_SCHEMA_USAGE_COMPLETION_MISSING", "PROVIDER_SCHEMA_USAGE_TOO_LARGE",
    "PROVIDER_SCHEMA_USAGE_TOTAL_MISMATCH", "PROVIDER_SCHEMA_USAGE_CACHE_MISMATCH",
    "PROVIDER_SCHEMA_USAGE_TYPE",
    "ACTION_LOOP_DOMAIN_UNRESOLVED", "ACTION_LOOP_DOMAIN_CONFLICT",
    "CONCEPT_PLAN_MISSING", "CONCEPT_PLAN_DOMAIN_MISSING",
    "CONCEPT_PLAN_DOMAIN_UNSUPPORTED", "CONCEPT_PLAN_BRIEF_MISSING",
    "CONCEPT_PLAN_GEOMETRY_STRATEGY_INVALID", "ARM_DESIGN_INTENT_INVALID",
    "ARM_DESIGN_INTENT_REQUIRED",
    "ARM_INTENT_ARCHITECTURE_UNSUPPORTED", "ARM_RECIPE_LOWERING_SERIALIZATION_FAILED",
    "ASSEMBLY_DELTA_INVALID", "ASSEMBLY_DELTA_NOT_ALLOWED_ON_INITIAL_SYNTHESIS",
    "ASSEMBLY_DELTA_BASE_STALE",
    "NATIVE_PRODUCT_TOOL_UNSUPPORTED", "NATIVE_PRODUCT_TOOL_RESULT_INVALID",
    "NATIVE_PRODUCT_TOOL_ARGUMENT_SCHEMA_INVALID", "NATIVE_PRODUCT_TOOL_CALL_LIMIT",
    "PRODUCT_TOOL_EXECUTION_FAILED", "PRODUCT_TOOL_OUTPUT_SERIALIZATION_FAILED",
    "REVIEWED_CATALOG_DOMAIN_UNAVAILABLE", "REVIEWED_RECIPE_EXPANSION_FAILED",
    "ARM_GEOMETRY_FAMILY_INVALID",
}


class AcceptanceError(RuntimeError):
    pass


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run the explicit Rust-native DeepSeek MVP acceptance probe.")
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


def _start(environment: dict[str, str], foreground_launch: bool) -> int:
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
    names = (
        "WUSHEN_LIBRARY_ROOT",
        "WUSHEN_AGENT_RUNTIME_MODE",
        "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE",
        "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE_CONFIRM",
        "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE_RUN_ID",
        "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE_OUTPUT",
        "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE_BUDGET_OVERRIDE",
    )
    for name in names:
        command.extend(["--env", f"{name}={environment[name]}"])
    command.append(str(APP_BUNDLE))
    subprocess.run(command, cwd=ROOT, env=environment, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for _ in range(100):
        created = _desktop_pids() - existing
        if created:
            return max(created)
        time.sleep(0.1)
    raise AcceptanceError("LIVE_APP_START_FAILED")


def _read_report(path: Path) -> dict[str, Any]:
    deadline = time.monotonic() + 240.0
    while time.monotonic() < deadline:
        if path.is_file():
            try:
                value = json.loads(path.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError):
                time.sleep(0.1)
                continue
            if isinstance(value, dict):
                return value
        time.sleep(0.1)
    raise AcceptanceError("LIVE_REPORT_TIMEOUT")


def _validate_report(report: dict[str, Any], run_id: str) -> dict[str, Any]:
    encoded = json.dumps(report, ensure_ascii=False, sort_keys=True)
    if report.get("schema_version") != REPORT_SCHEMA_VERSION or report.get("status") not in {"pass", "fail"}:
        raise AcceptanceError("LIVE_RUST_PROBE_FAILED")
    if report.get("run_id_sha256") != hashlib.sha256(run_id.encode("utf-8")).hexdigest():
        raise AcceptanceError("LIVE_REPORT_IDENTITY_DRIFT")
    if report.get("provider_owner") != "rust_desktop" or report.get("credential_source") != "rust_provider_credential_store":
        raise AcceptanceError("LIVE_RUST_OWNERSHIP_DRIFT")
    if type(report.get("network_calls_made")) is not int or report["network_calls_made"] not in {0, 1, 2}:
        raise AcceptanceError("LIVE_NETWORK_EVIDENCE_INVALID")
    for phase in ("live_turn", "cancellation", "local_failure"):
        value = report.get(phase)
        if not isinstance(value, dict):
            raise AcceptanceError("LIVE_PHASE_EVIDENCE_INVALID")
        status = value.get("status")
        if phase == "local_failure" and report.get("status") == "pass":
            valid_status = status == "failed_closed"
        else:
            valid_status = status in SAFE_PHASE_STATUSES
        if not valid_status or not isinstance(value.get("network_call_made"), bool):
            raise AcceptanceError("LIVE_PHASE_EVIDENCE_INVALID")
        if type(value.get("asset_or_snapshot_writes")) is not int or value["asset_or_snapshot_writes"] < 0:
            raise AcceptanceError("LIVE_PHASE_EVIDENCE_INVALID")
        if not isinstance(value.get("arm_intent_bound"), bool):
            raise AcceptanceError("LIVE_PHASE_EVIDENCE_INVALID")
        category = value.get("failure_category")
        if category is not None and category not in SAFE_FAILURE_CATEGORIES:
            raise AcceptanceError("LIVE_REPORT_REDACTION_INVALID")
        phase_error = value.get("error_code")
        if phase_error is not None and phase_error not in SAFE_PHASE_ERROR_CODES:
            raise AcceptanceError("LIVE_REPORT_REDACTION_INVALID")
    if report.get("no_raw_prompt_or_response") is not True or report.get("no_key_or_provider_endpoint") is not True:
        raise AcceptanceError("LIVE_REDACTION_EVIDENCE_INVALID")
    if any(f'"{key}"' in encoded for key in FORBIDDEN_REPORT_KEYS):
        raise AcceptanceError("LIVE_REPORT_REDACTION_INVALID")
    if report.get("error_phase") not in {None, "initialization", "live_turn", "cancellation", "local_failure"}:
        raise AcceptanceError("LIVE_REPORT_REDACTION_INVALID")
    if report.get("error_code") not in {None, *SAFE_PROBE_ERROR_CODES}:
        raise AcceptanceError("LIVE_REPORT_REDACTION_INVALID")
    if report.get("status") != "pass":
        raise AcceptanceError("LIVE_RUST_PROBE_FAILED")
    if report.get("network_calls_made") not in {1, 2}:
        raise AcceptanceError("LIVE_NETWORK_EVIDENCE_INVALID")
    for phase, expected in (("live_turn", "completed"), ("cancellation", "cancelled"), ("local_failure", "failed_closed")):
        value = report[phase]
        if value.get("status") != expected or value.get("asset_or_snapshot_writes") != 0:
            raise AcceptanceError("LIVE_PHASE_EVIDENCE_INVALID")
    if report["live_turn"].get("arm_intent_bound") is not True:
        raise AcceptanceError("LIVE_TURN_ARM_INTENT_MISSING")
    return report


def _safe_failure_summary(report: dict[str, Any] | None) -> dict[str, object] | None:
    """Return only fixed, already-whitelisted diagnostic facts for the CLI."""
    if not isinstance(report, dict) or report.get("schema_version") != REPORT_SCHEMA_VERSION:
        return None
    phase = report.get("error_phase")
    evidence = report.get(phase) if phase in {"live_turn", "cancellation", "local_failure"} else None
    if not isinstance(evidence, dict):
        return None
    category = evidence.get("failure_category")
    status = evidence.get("status")
    if phase not in {"initialization", "live_turn", "cancellation", "local_failure"}:
        return None
    if status not in SAFE_PHASE_STATUSES or category not in SAFE_FAILURE_CATEGORIES | {None}:
        return None
    return {
        "error_code": report.get("error_code") if report.get("error_code") in SAFE_PROBE_ERROR_CODES else None,
        "error_phase": phase,
        "network_call_made": evidence.get("network_call_made") is True,
        "phase_error_code": evidence.get("error_code") if evidence.get("error_code") in SAFE_PHASE_ERROR_CODES else None,
        "phase_failure_category": category,
        "phase_status": status,
    }


def _live_environment(library: Path, run_id: str, output: Path) -> dict[str, str]:
    environment = os.environ.copy()
    for name in tuple(environment):
        if name.startswith("FORGECAD_DEEPSEEK_MVP_ACCEPTANCE") or name in {
            "FORGECAD_MVP_OFFLINE_ARM",
            "FORGECAD_DISABLE_PROVIDER_CONFIG",
            "FORGECAD_AGENT_API_KEY",
            "FORGECAD_AGENT_API_KEY_FILE",
            "FORGECAD_AGENT_BASE_URL",
            "FORGECAD_AGENT_MODEL",
            "FORGECAD_AGENT_PROVIDER",
        }:
            environment.pop(name, None)
    environment.update({
        "WUSHEN_LIBRARY_ROOT": str(library),
        "WUSHEN_AGENT_RUNTIME_MODE": "packaged-sidecar",
        "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE": "1",
        "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE_CONFIRM": LIVE_CONFIRMATION,
        "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE_RUN_ID": run_id,
        "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE_OUTPUT": str(output),
        # Explicit live acceptance only: remove the ordinary cumulative
        # 256K/100K Turn ceilings while retaining Rust's reviewed per-request,
        # tool-count, wall-time, cancellation, and recovery protections.
        "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_BUDGET_OVERRIDE": "1",
        "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE_BUDGET_OVERRIDE": "1",
    })
    return environment


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    report: dict[str, Any] | None = None
    try:
        run_id, output = _validate_live(args)
    except AcceptanceError as error:
        print(json.dumps(_dry_report(str(error)), ensure_ascii=False, sort_keys=True))
        return 0
    try:
        if not APP_BINARY.is_file():
            raise AcceptanceError("LIVE_APP_NOT_BUILT")
        if _listener_pid() is not None:
            raise AcceptanceError("LIVE_PORT_8000_OCCUPIED")
        output.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="forgecad_deepseek_mvp_") as temporary:
            desktop_pid = _start(
                _live_environment(Path(temporary) / "library", run_id, output),
                args.foreground_launch,
            )
            try:
                listener = _wait_for_native_health(desktop_pid)
                if not _is_descendant(listener, desktop_pid):
                    raise AcceptanceError("LIVE_SIDECAR_OWNERSHIP_INVALID")
                report = _read_report(output)
                _validate_report(report, run_id)
            finally:
                _stop_desktop_and_listener(desktop_pid)
        print(json.dumps(report, ensure_ascii=False, sort_keys=True))
        return 0
    except AcceptanceError as error:
        result: dict[str, object] = {
            "schema_version": SCHEMA_VERSION,
            "status": "fail",
            "network_calls_made": report.get("network_calls_made", 0) if isinstance(report, dict) else 0,
            "error": str(error),
        }
        summary = _safe_failure_summary(report)
        if summary is not None:
            result["safe_failure_summary"] = summary
        print(json.dumps(result, ensure_ascii=False, sort_keys=True))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
