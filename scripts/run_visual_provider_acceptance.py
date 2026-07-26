#!/usr/bin/env python3
"""Run the explicit Rust-owned Forge Studio visual-provider acceptance.

Without the complete live confirmation this is a zero-network dry-run. The
launcher never reads a Fal/DeepSeek credential, provider response, PNG, or GLB;
it only validates the redacted report written by the desktop Rust process.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
from typing import Any

from smoke_packaged_tauri_alpha import (
    APP_BINARY,
    _desktop_pids,
    _listener_pid,
    _stop_desktop_and_listener,
)


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_VERSION = "ForgeStudioVisualProviderAcceptanceLaunch@1"
REPORT_SCHEMA_VERSION = "ForgeStudioVisualProviderAcceptance@1"
LIVE_CONFIRMATION = "I_UNDERSTAND_THIS_MAY_INCUR_VISUAL_PROVIDER_COST"
RUN_ID = re.compile(r"^live_[A-Za-z0-9_-]{7,75}$")
REQUIRED_PBR = {"base_color", "normal", "roughness", "metallic"}
FORBIDDEN_REPORT_KEYS = {
    "api_key", "secret", "base_url", "endpoint", "prompt", "response",
    "concept_png_base64", "glb_base64",
}


class AcceptanceError(RuntimeError):
    pass


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run the Forge Studio live visual-provider acceptance.")
    parser.add_argument("--confirm-live-provider", action="store_true")
    parser.add_argument("--accept-network", action="store_true")
    parser.add_argument("--confirmation")
    parser.add_argument("--run-id")
    parser.add_argument("--output", type=Path)
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
    values = (
        args.confirm_live_provider,
        args.accept_network,
        args.confirmation,
        args.run_id,
        args.output,
    )
    if not any(values) or not all(values) or args.confirmation != LIVE_CONFIRMATION:
        raise AcceptanceError("VISUAL_ACCEPTANCE_CONFIRMATION_REQUIRED")
    if not isinstance(args.run_id, str) or RUN_ID.fullmatch(args.run_id) is None:
        raise AcceptanceError("VISUAL_ACCEPTANCE_RUN_ID_INVALID")
    output = args.output.expanduser()
    if not output.is_absolute() or output.suffix.lower() != ".json":
        raise AcceptanceError("VISUAL_ACCEPTANCE_OUTPUT_INVALID")
    return args.run_id, output


def _validate_report(report: dict[str, Any], run_id: str) -> dict[str, Any]:
    if report.get("schema_version") != REPORT_SCHEMA_VERSION:
        raise AcceptanceError("VISUAL_ACCEPTANCE_REPORT_SCHEMA_INVALID")
    if report.get("run_id_sha256") != hashlib.sha256(run_id.encode()).hexdigest():
        raise AcceptanceError("VISUAL_ACCEPTANCE_REPORT_IDENTITY_DRIFT")
    if report.get("status") != "pass":
        raise AcceptanceError(str(report.get("error_code") or "VISUAL_ACCEPTANCE_PROVIDER_FAILED"))
    expected = {
        "execution_mode": "live_explicit_opt_in",
        "provider_owner": "rust_desktop",
        "credential_source": "private_visual_secret_file",
        "concept_provider_completed": True,
        "neural_provider_completed": True,
        "remote_job_completed": True,
        "no_raw_prompt_or_response": True,
        "no_key_or_provider_endpoint": True,
    }
    if any(report.get(key) != value for key, value in expected.items()):
        raise AcceptanceError("VISUAL_ACCEPTANCE_RUST_OWNERSHIP_INVALID")
    for key in ("concept_png_sha256", "glb_sha256"):
        value = report.get(key)
        if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
            raise AcceptanceError("VISUAL_ACCEPTANCE_HASH_INVALID")
    for key in ("glb_byte_size", "triangle_count", "mesh_count", "material_count"):
        if type(report.get(key)) is not int or report[key] <= 0:
            raise AcceptanceError("VISUAL_ACCEPTANCE_READBACK_INVALID")
    channels = report.get("pbr_channels")
    if not isinstance(channels, list) or not REQUIRED_PBR.issubset(set(channels)):
        raise AcceptanceError("VISUAL_ACCEPTANCE_PBR_INCOMPLETE")
    if report.get("every_primitive_has_uv0") is not True:
        raise AcceptanceError("VISUAL_ACCEPTANCE_UV0_INCOMPLETE")
    encoded_keys = set(_walk_keys(report))
    if encoded_keys & FORBIDDEN_REPORT_KEYS:
        raise AcceptanceError("VISUAL_ACCEPTANCE_REPORT_REDACTION_INVALID")
    return report


def _walk_keys(value: Any):
    if isinstance(value, dict):
        for key, child in value.items():
            yield str(key).lower()
            yield from _walk_keys(child)
    elif isinstance(value, list):
        for child in value:
            yield from _walk_keys(child)


def _read_report(path: Path) -> dict[str, Any]:
    deadline = time.monotonic() + 1_500
    while time.monotonic() < deadline:
        if path.is_file():
            try:
                value = json.loads(path.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError):
                time.sleep(0.2)
                continue
            if isinstance(value, dict):
                return value
        time.sleep(0.2)
    raise AcceptanceError("VISUAL_ACCEPTANCE_REPORT_TIMEOUT")


def _run_live(run_id: str, output: Path) -> dict[str, Any]:
    if not APP_BINARY.is_file():
        raise AcceptanceError("VISUAL_ACCEPTANCE_PACKAGED_APP_REQUIRED")
    if _desktop_pids() or _listener_pid() is not None:
        raise AcceptanceError("VISUAL_ACCEPTANCE_CLOSE_RUNNING_APP_FIRST")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.unlink(missing_ok=True)
    environment = os.environ.copy()
    environment.pop("FORGECAD_MVP_OFFLINE_ARM", None)
    environment.update({
        "FORGESTUDIO_VISUAL_ACCEPTANCE": "1",
        "FORGESTUDIO_VISUAL_ACCEPTANCE_CONFIRM": LIVE_CONFIRMATION,
        "FORGESTUDIO_VISUAL_ACCEPTANCE_RUN_ID": run_id,
        "FORGESTUDIO_VISUAL_ACCEPTANCE_OUTPUT": str(output),
    })
    process = subprocess.Popen(
        [str(APP_BINARY)],
        cwd=ROOT,
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    try:
        report = _read_report(output)
        return _validate_report(report, run_id)
    finally:
        _stop_desktop_and_listener(process.pid)


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        run_id, output = _validate_live(args)
    except AcceptanceError as error:
        if str(error) == "VISUAL_ACCEPTANCE_CONFIRMATION_REQUIRED":
            print(json.dumps(_dry_report(str(error)), ensure_ascii=False, sort_keys=True))
            return 0
        print(json.dumps({"status": "rejected", "error_code": str(error)}, sort_keys=True))
        return 2
    try:
        report = _run_live(run_id, output)
    except AcceptanceError as error:
        print(json.dumps({"status": "fail", "error_code": str(error)}, sort_keys=True))
        return 1
    print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
