#!/usr/bin/env python3
"""No-network contract checks for the DeepSeek ForgeVisual acceptance launcher."""

from __future__ import annotations

import hashlib
import io
import json
from contextlib import redirect_stdout

from run_deepseek_forge_visual_acceptance import (
    AcceptanceError,
    LIVE_CONFIRMATION,
    REPORT_SCHEMA_VERSION,
    _parser,
    _validate_live,
    _validate_report,
    main,
)


def rejects(arguments: list[str], expected: str) -> None:
    try:
        _validate_live(_parser().parse_args(arguments))
    except AcceptanceError as error:
        assert str(error) == expected
        return
    raise AssertionError(f"expected {expected}")


def main_smoke() -> int:
    stdout = io.StringIO()
    with redirect_stdout(stdout):
        assert main([]) == 0
    assert json.loads(stdout.getvalue()) == {
        "app_launched": False,
        "credential_reads": 0,
        "network_calls_made": 0,
        "reason": "FORGE_VISUAL_LIVE_CONFIRMATION_REQUIRED",
        "schema_version": "ForgeCADDeepSeekForgeVisualAcceptanceLaunch@1",
        "status": "dry_run",
    }
    rejects(["--confirm-live-provider"], "FORGE_VISUAL_LIVE_CONFIRMATION_REQUIRED")
    rejects([
        "--confirm-live-provider", "--accept-network", "--confirmation", LIVE_CONFIRMATION,
        "--run-id", "wrong", "--output", "/tmp/report.json",
    ], "FORGE_VISUAL_LIVE_RUN_ID_INVALID")
    rejects([
        "--confirm-live-provider", "--accept-network", "--confirmation", LIVE_CONFIRMATION,
        "--run-id", "live_forge_visual_smoke", "--output", "relative.json",
    ], "FORGE_VISUAL_LIVE_OUTPUT_INVALID")

    run_id = "live_forge_visual_smoke"
    evidence = {
        "status": "completed",
        "network_call_made": True,
        "author_forge_visual_program_completed": True,
        "author_source_mode": "provider_authoring_ir",
        "rust_compile_readback_completed": True,
        "rust_eight_view_render_completed": True,
        "rust_evaluate_completed": True,
        "single_result_ready": True,
        "preview_hash_matches_bytes_and_header": True,
        "confirmed_asset_created": True,
        "snapshot_advanced": True,
        "export_hash_matches_bytes_json_and_header": True,
        "completed_tool_stages": [
            "author_forge_visual_program", "build_candidate_geometry", "compile_readback_candidate",
            "render_candidate_views", "evaluate_candidate", "prepare_candidate_preview",
        ],
    }
    report = {
        "schema_version": REPORT_SCHEMA_VERSION,
        "status": "pass",
        "execution_mode": "live_explicit_opt_in",
        "run_id_sha256": hashlib.sha256(run_id.encode()).hexdigest(),
        "provider_owner": "rust_desktop",
        "credential_source": "rust_provider_credential_store",
        "network_calls_made": 1,
        "visual_program_turn": evidence,
        "no_raw_prompt_or_response": True,
        "no_key_or_provider_endpoint": True,
    }
    assert _validate_report(report, run_id) == report
    try:
        _validate_report(dict(report, prompt="forbidden"), run_id)
    except AcceptanceError as error:
        assert str(error) == "FORGE_VISUAL_LIVE_REPORT_REDACTION_INVALID"
    else:
        raise AssertionError("prompt must be rejected")
    incomplete = json.loads(json.dumps(report))
    incomplete["visual_program_turn"]["snapshot_advanced"] = False
    try:
        _validate_report(incomplete, run_id)
    except AcceptanceError as error:
        assert str(error) == "FORGE_VISUAL_LIVE_E2E_EVIDENCE_INVALID"
    else:
        raise AssertionError("incomplete live flow must not pass")
    print("DeepSeek ForgeVisual acceptance launcher smoke passed (no network calls)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main_smoke())
