#!/usr/bin/env python3
"""No-network checks for the Rust-native DeepSeek MVP acceptance launcher."""

from __future__ import annotations

import json
import hashlib
from pathlib import Path
import subprocess
import sys

from run_deepseek_mvp_acceptance import (
    AcceptanceError,
    LIVE_CONFIRMATION,
    REPORT_SCHEMA_VERSION,
    _parser,
    _safe_failure_summary,
    _validate_live,
    _validate_report,
)


ROOT = Path(__file__).resolve().parents[1]


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def _reject(arguments: list[str], expected: str) -> None:
    try:
        _validate_live(_parser().parse_args(arguments))
    except AcceptanceError as error:
        _assert(str(error) == expected, f"expected {expected}, got {error}")
        return
    raise AssertionError(f"expected {expected}")


def main() -> int:
    source = (ROOT / "scripts" / "run_deepseek_mvp_acceptance.py").read_text(encoding="utf-8")
    for forbidden in ("find-generic-password", "security find", 'get("FORGECAD_AGENT_API_KEY")'):
        _assert(forbidden not in source, f"launcher must not read credentials: {forbidden}")

    _reject([], "LIVE_CONFIRMATION_REQUIRED")
    _reject(["--confirm-live-provider"], "LIVE_CONFIRMATION_REQUIRED")
    _reject(
        [
            "--confirm-live-provider", "--accept-network",
            "--confirmation", LIVE_CONFIRMATION,
            "--run-id", "not_live",
            "--output", "/tmp/forgecad.json",
        ],
        "LIVE_RUN_ID_INVALID",
    )
    _reject(
        [
            "--confirm-live-provider", "--accept-network",
            "--confirmation", LIVE_CONFIRMATION,
            "--run-id", "live_acceptance_20260719",
            "--output", "relative.json",
        ],
        "LIVE_OUTPUT_INVALID",
    )

    dry = subprocess.run(
        [sys.executable, "scripts/run_deepseek_mvp_acceptance.py"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    _assert(dry.returncode == 0, "default launcher must be a successful dry-run")
    report = json.loads(dry.stdout)
    _assert(
        report == {
            "app_launched": False,
            "credential_reads": 0,
            "network_calls_made": 0,
            "reason": "LIVE_CONFIRMATION_REQUIRED",
            "schema_version": "ForgeCADDeepSeekMvpAcceptanceLaunch@1",
            "status": "dry_run",
        },
        "dry-run must be strictly zero-network and zero-credential",
    )
    _assert(REPORT_SCHEMA_VERSION == "ForgeCADDeepSeekMvpAcceptance@1", "report schema drift")
    failed_report = {
        "schema_version": REPORT_SCHEMA_VERSION,
        "status": "fail",
        "execution_mode": "live_explicit_opt_in",
        "run_id_sha256": hashlib.sha256(b"live_acceptance_20260719").hexdigest(),
        "provider_owner": "rust_desktop",
        "credential_source": "rust_provider_credential_store",
        "network_calls_made": 1,
        "live_turn": {
            "status": "failed",
            "network_call_made": True,
            "asset_or_snapshot_writes": 0,
            "arm_intent_bound": False,
            "input_tokens": 12,
            "output_tokens": 0,
            "error_code": "PROVIDER_AUTHENTICATION_FAILED",
            "failure_category": "provider_execution",
        },
        "cancellation": {"status": "not_run", "network_call_made": False, "asset_or_snapshot_writes": 0, "arm_intent_bound": False},
        "local_failure": {"status": "not_run", "network_call_made": False, "asset_or_snapshot_writes": 0, "arm_intent_bound": False},
        "no_raw_prompt_or_response": True,
        "no_key_or_provider_endpoint": True,
        "error_phase": "live_turn",
        "error_code": "LIVE_TURN_NOT_EPHEMERAL_COMPLETION",
    }
    try:
        _validate_report(failed_report, "live_acceptance_20260719")
    except AcceptanceError as error:
        _assert(str(error) == "LIVE_RUST_PROBE_FAILED", "failed reports must preserve safe diagnostics")
    else:
        raise AssertionError("failed report unexpectedly accepted")
    # The summary only emits fixed redacted categories and terminal facts.
    summary = _safe_failure_summary(failed_report)
    _assert(summary == {
        "error_code": "LIVE_TURN_NOT_EPHEMERAL_COMPLETION",
        "error_phase": "live_turn",
        "network_call_made": True,
        "phase_error_code": "PROVIDER_AUTHENTICATION_FAILED",
        "phase_failure_category": "provider_execution",
        "phase_status": "failed",
    }, "failed report summary must stay redacted and useful")
    rejected_phase_code = json.loads(json.dumps(failed_report))
    rejected_phase_code["live_turn"]["error_code"] = "PROVIDER_LOOKS_SAFE_BUT_IS_NOT_REVIEWED"
    try:
        _validate_report(rejected_phase_code, "live_acceptance_20260719")
    except AcceptanceError as error:
        _assert(str(error) == "LIVE_REPORT_REDACTION_INVALID", "unknown phase code must fail closed")
    else:
        raise AssertionError("unknown phase code unexpectedly accepted")
    _assert(
        "report = _read_report(output)\n                _validate_report(report, run_id)" in source,
        "launcher must preserve a failed Rust report before validation",
    )
    print("Rust-native DeepSeek MVP acceptance launcher smoke passed (no network calls)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
