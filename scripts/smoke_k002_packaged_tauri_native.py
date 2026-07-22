#!/usr/bin/env python3
"""Prove the packaged WebView uses the Rust-owned K002 Agent lifecycle.

This smoke is intentionally offline and isolated from the user's credentials.
It launches the real macOS ``.app`` twice through LaunchServices with a
temporary HOME/Library and Provider configuration disabled.  The opt-in
WebView probe must use the native app-server transport, persist a failed Turn
for an unconfigured Provider without a network call, replay ordered Items after
restart, and prove the removed Python lifecycle POST remains HTTP 410. Neither
legacy Python writer oracle is enabled.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import tempfile
import time
from pathlib import Path

from smoke_packaged_tauri_alpha import (
    APP_BINARY,
    APP_BUNDLE,
    _assert,
    _desktop_pids,
    _is_descendant,
    _listener_pid,
    _stop_desktop_and_listener,
    _wait_for_log_count,
    _wait_for_native_health,
)


ROOT = Path(__file__).resolve().parents[1]
PROBE_MARKER = "ForgeCAD K002 packaged WebView probe report="
PROBE_SCHEMA = "ForgeCADK002PackagedProbe@1"
APP_SERVER_READY_MARKER = "ForgeCAD app-server ready protocol=forgecad.app-server/1"
ARTIFACT_DIRECTORY = os.environ.get(
    "FORGECAD_K002_PACKAGED_SMOKE_ARTIFACT_DIR",
    os.environ.get("FORGECAD_NATIVE_SMOKE_ARTIFACT_DIR", ""),
)
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")


def main() -> int:
    _assert(APP_BINARY.is_file(), "build the macOS .app before the K002 packaged smoke")
    _assert(_listener_pid() is None, "port 8000 must be free before the K002 packaged smoke")

    with tempfile.TemporaryDirectory(prefix="forgecad_k002_packaged_") as temporary:
        temporary_path = Path(temporary)
        log_path = temporary_path / "WushenForge/agent.log"
        try:
            report = _run_native_smoke(temporary_path)
        except Exception as error:
            _write_artifact(
                {
                    "schema_version": PROBE_SCHEMA,
                    "ok": False,
                    "provider_calls": 0,
                    "error": _safe_diagnostic(str(error), temporary_path),
                },
                log_path=log_path,
                temporary_path=temporary_path,
            )
            raise

    _write_artifact(report)
    print(json.dumps(report, separators=(",", ":")))
    return 0


def _run_native_smoke(temporary_path: Path) -> dict[str, object]:
    library_root = temporary_path / "library"
    log_path = temporary_path / "WushenForge/agent.log"
    environment = _safe_environment(temporary_path, library_root)

    initial_desktop = _start_desktop(temporary_path, environment)
    try:
        listener_pid = _wait_for_native_health(initial_desktop)
        _assert(
            _is_descendant(listener_pid, initial_desktop),
            "healthy Agent listener is not owned by the packaged desktop supervisor",
        )
        _wait_for_log_count(log_path, APP_SERVER_READY_MARKER, 1)
        _assert((library_root / "library.db").is_file(), "K002 first launch did not create the Library")
        initial_report = _wait_for_probe_report(log_path, "initial")
        _validate_probe_report(initial_report, "initial")
    finally:
        _stop_desktop_and_listener(initial_desktop)

    _assert(_listener_pid() is None, "sidecar listener survived K002 first-launch cleanup")

    environment["FORGECAD_K002_PACKAGED_PROBE_PHASE"] = "restart"
    environment["FORGECAD_K002_EXPECT_THREAD_ID"] = str(initial_report["thread_id"])
    environment["FORGECAD_K002_EXPECT_TURN_ID"] = str(initial_report["turn_id"])
    environment["FORGECAD_K002_EXPECT_ITEMS_SHA256"] = str(initial_report["items_sha256"])
    environment["FORGECAD_K002_EXPECT_ITEM_COUNT"] = str(initial_report["item_count"])
    environment["FORGECAD_K002_EXPECT_LAST_SEQUENCE"] = str(initial_report["last_sequence"])
    environment["FORGECAD_K002_EXPECT_TURN_ERROR_CODE"] = str(initial_report["turn_error_code"])

    restarted_desktop = _start_desktop(temporary_path, environment)
    try:
        listener_pid = _wait_for_native_health(restarted_desktop)
        _assert(
            _is_descendant(listener_pid, restarted_desktop),
            "restarted Agent listener is not owned by the packaged desktop supervisor",
        )
        _wait_for_log_count(log_path, APP_SERVER_READY_MARKER, 2)
        restart_report = _wait_for_probe_report(log_path, "restart")
        _validate_probe_report(restart_report, "restart")
    finally:
        _stop_desktop_and_listener(restarted_desktop)

    _assert(_listener_pid() is None, "sidecar listener survived K002 restart cleanup")
    for field in (
        "thread_id",
        "turn_id",
        "turn_status",
        "turn_error_code",
        "item_count",
        "last_sequence",
        "item_sequences",
        "item_ids",
        "item_types",
        "items_sha256",
        "replay_items_sha256",
    ):
        _assert(
            restart_report[field] == initial_report[field],
            f"packaged K002 restart changed {field}",
        )

    return {
        "schema_version": PROBE_SCHEMA,
        "ok": True,
        "supervisor_mode": "packaged-sidecar",
        "packaged_webview_native_transport": True,
        "capability_ownership_verified": True,
        "supervisor_running": True,
        "supervisor_state": "running",
        "supervisor_managed_by_desktop": True,
        "rust_thread_turn_item_lifecycle": True,
        "provider_status": "unconfigured",
        "provider_network_call_made": False,
        "agent_service_status_capability_handshake": True,
        "turn_status": "failed",
        "turn_error_code": initial_report["turn_error_code"],
        "ordered_item_replay": True,
        "restart_replay_consistent": True,
        "legacy_lifecycle_post_status": 410,
        "reasoning_content_present": False,
        "legacy_python_writer_oracles": False,
        "item_count": initial_report["item_count"],
        "last_sequence": initial_report["last_sequence"],
        "items_sha256": initial_report["items_sha256"],
        "provider_calls": 0,
    }


def _safe_environment(temporary: Path, library_root: Path) -> dict[str, str]:
    environment = os.environ.copy()
    for name in (
        "FORGECAD_AGENT_PROVIDER",
        "FORGECAD_AGENT_BASE_URL",
        "FORGECAD_AGENT_MODEL",
        "FORGECAD_AGENT_API_KEY",
        "FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE",
        "FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE",
        "FORGECAD_K001_PACKAGED_PROBE",
        "FORGECAD_K001_PACKAGED_PROBE_PHASE",
        "FORGECAD_K001_EXPECT_PROJECT_ID",
        "FORGECAD_K001_EXPECT_THREAD_ID",
        "FORGECAD_K001_EXPECT_ASSET_VERSION_ID",
        "FORGECAD_K001_EXPECT_LAST_EVENT_ID",
        "FORGECAD_K001_EXPECT_CURSOR",
        "FORGECAD_K001_EXPECT_GLB_SHA256",
        "FORGECAD_K002_EXPECT_THREAD_ID",
        "FORGECAD_K002_EXPECT_TURN_ID",
        "FORGECAD_K002_EXPECT_ITEMS_SHA256",
        "FORGECAD_K002_EXPECT_ITEM_COUNT",
        "FORGECAD_K002_EXPECT_LAST_SEQUENCE",
        "FORGECAD_K002_EXPECT_TURN_ERROR_CODE",
    ):
        environment.pop(name, None)
    environment["HOME"] = str(temporary)
    environment["WUSHEN_LIBRARY_ROOT"] = str(library_root)
    environment["WUSHEN_AGENT_RUNTIME_MODE"] = "packaged-sidecar"
    environment["FORGECAD_DISABLE_PROVIDER_CONFIG"] = "1"
    environment["FORGECAD_CONCEPT_WORKER_ENABLED"] = "0"
    environment["WUSHEN_LOCAL_WORKER_ENABLED"] = "0"
    environment["FORGECAD_K002_PACKAGED_PROBE"] = "1"
    environment["FORGECAD_K002_PACKAGED_PROBE_PHASE"] = "initial"
    return environment


def _start_desktop(temporary: Path, environment: dict[str, str]) -> int:
    existing = _desktop_pids()
    forwarded_names = {
        "HOME",
        "WUSHEN_LIBRARY_ROOT",
        "WUSHEN_AGENT_RUNTIME_MODE",
        "FORGECAD_DISABLE_PROVIDER_CONFIG",
        "FORGECAD_CONCEPT_WORKER_ENABLED",
        "WUSHEN_LOCAL_WORKER_ENABLED",
        "FORGECAD_K002_PACKAGED_PROBE",
        "FORGECAD_K002_PACKAGED_PROBE_PHASE",
        "FORGECAD_K002_EXPECT_THREAD_ID",
        "FORGECAD_K002_EXPECT_TURN_ID",
        "FORGECAD_K002_EXPECT_ITEMS_SHA256",
        "FORGECAD_K002_EXPECT_ITEM_COUNT",
        "FORGECAD_K002_EXPECT_LAST_SEQUENCE",
        "FORGECAD_K002_EXPECT_TURN_ERROR_CODE",
    }
    command = ["open", "-n"]
    for name in sorted(forwarded_names):
        if name in environment:
            command.extend(["--env", f"{name}={environment[name]}"])
    command.append(str(APP_BUNDLE))
    subprocess.run(command, cwd=temporary, env=environment, check=True)
    for _ in range(100):
        created = _desktop_pids() - existing
        if created:
            return max(created)
        time.sleep(0.1)
    raise AssertionError("LaunchServices did not start the packaged desktop app for K002")


def _wait_for_probe_report(path: Path, phase: str) -> dict[str, object]:
    for _ in range(1_900):
        if path.is_file():
            reports: list[dict[str, object]] = []
            for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
                if not line.startswith(PROBE_MARKER):
                    continue
                try:
                    value = json.loads(line[len(PROBE_MARKER) :])
                except (TypeError, ValueError):
                    continue
                if isinstance(value, dict) and value.get("phase") == phase:
                    reports.append(value)
            if reports:
                report = reports[-1]
                _assert(
                    report.get("schema_version") == PROBE_SCHEMA,
                    "packaged K002 WebView probe returned the wrong schema",
                )
                if report.get("ok") is not True:
                    raise AssertionError(
                        f"packaged K002 WebView {phase} probe failed with "
                        f"{report.get('error_code', 'UNKNOWN')}"
                    )
                return report
        time.sleep(0.1)
    raise AssertionError(f"packaged K002 WebView {phase} probe did not finish within 190 seconds")


def _validate_probe_report(report: dict[str, object], phase: str) -> None:
    _assert(report.get("schema_version") == PROBE_SCHEMA, "unexpected K002 probe schema")
    _assert(report.get("phase") == phase, f"unexpected K002 probe phase for {phase}")
    _assert(report.get("ok") is True, f"K002 {phase} probe did not report success")
    _assert(_stable_identifier(report.get("thread_id")), "K002 probe returned an invalid Thread ID")
    _assert(_stable_identifier(report.get("turn_id")), "K002 probe returned an invalid Turn ID")
    _assert(report.get("turn_status") == "failed", "unconfigured K002 Turn did not fail closed")
    _assert(
        report.get("turn_error_code") == "PROVIDER_NOT_CONFIGURED",
        "unconfigured K002 Turn returned the wrong error code",
    )
    _assert(report.get("provider_status") == "unconfigured", "Provider preflight was not unconfigured")
    _assert(report.get("provider_configured") is False, "Provider unexpectedly reported configured")
    _assert(
        report.get("provider_network_call_made") is False,
        "unconfigured Provider preflight reported a network call",
    )
    _assert(report.get("supervisor_running") is True, "agent_service_status did not report running")
    _assert(
        report.get("supervisor_state") == "running",
        "agent_service_status did not report the running state",
    )
    _assert(
        report.get("supervisor_managed_by_desktop") is True,
        "agent_service_status did not verify desktop capability ownership",
    )
    _assert(report.get("supervisor_running") is True, "K002 supervisor was not running")
    _assert(report.get("supervisor_state") == "running", "K002 supervisor state was not running")
    _assert(
        report.get("supervisor_managed_by_desktop") is True,
        "K002 sidecar was not owned by the current desktop supervisor",
    )
    _assert(report.get("reasoning_content_present") is False, "K002 persisted reasoning_content")
    _assert(
        report.get("legacy_lifecycle_post_status") == 410,
        "legacy Python lifecycle POST did not remain HTTP 410",
    )
    _assert(
        type(report.get("provider_calls")) is int and report.get("provider_calls") == 0,
        "offline K002 probe made a Provider call",
    )

    item_sequences = _integer_list(report.get("item_sequences"), "item_sequences")
    item_ids = _string_list(report.get("item_ids"), "item_ids")
    item_types = _string_list(report.get("item_types"), "item_types")
    _assert(2 <= len(item_sequences) <= 128, "K002 replay returned an invalid Item count")
    _assert(
        item_sequences == list(range(1, len(item_sequences) + 1)),
        "K002 replay Item sequences are not contiguous and ordered",
    )
    _assert(len(item_ids) == len(item_sequences), "K002 replay Item ID count changed")
    _assert(len(item_types) == len(item_sequences), "K002 replay Item type count changed")
    _assert(len(set(item_ids)) == len(item_ids), "K002 replay returned duplicate Item IDs")
    _assert(all(_stable_identifier(item_id) for item_id in item_ids), "K002 replay Item IDs are invalid")
    _assert(
        item_types[:2] == ["user_message", "tool_result"],
        "K002 unconfigured lifecycle did not preserve user_message -> provider_gateway Item order",
    )

    item_count = report.get("item_count")
    last_sequence = report.get("last_sequence")
    _assert(type(item_count) is int and item_count == len(item_sequences), "K002 Item count is inconsistent")
    _assert(
        type(last_sequence) is int and last_sequence == item_sequences[-1],
        "K002 last Item sequence is inconsistent",
    )
    items_sha256 = report.get("items_sha256")
    replay_items_sha256 = report.get("replay_items_sha256")
    _assert(
        isinstance(items_sha256, str) and SHA256_PATTERN.fullmatch(items_sha256) is not None,
        "K002 Item replay hash is invalid",
    )
    _assert(replay_items_sha256 == items_sha256, "K002 immediate Item replay changed its hash")


def _stable_identifier(value: object) -> bool:
    return (
        isinstance(value, str)
        and 1 <= len(value) <= 128
        and value == value.strip()
        and all(character.isalnum() or character in "-_.:" for character in value)
    )


def _integer_list(value: object, field: str) -> list[int]:
    _assert(isinstance(value, list), f"K002 {field} must be a list")
    _assert(all(type(item) is int and item >= 0 for item in value), f"K002 {field} is invalid")
    return value


def _string_list(value: object, field: str) -> list[str]:
    _assert(isinstance(value, list), f"K002 {field} must be a list")
    _assert(
        all(isinstance(item, str) and 1 <= len(item) <= 128 for item in value),
        f"K002 {field} is invalid",
    )
    return value


def _write_artifact(
    report: dict[str, object],
    *,
    log_path: Path | None = None,
    temporary_path: Path | None = None,
) -> None:
    if not ARTIFACT_DIRECTORY:
        return
    directory = Path(ARTIFACT_DIRECTORY)
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "k002-packaged-report.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    if log_path is not None and log_path.is_file():
        log_contents = log_path.read_text(encoding="utf-8", errors="replace")[-16_384:]
        (directory / "k002-packaged-supervisor.log").write_text(
            _safe_diagnostic(log_contents, temporary_path) + "\n",
            encoding="utf-8",
        )


def _safe_diagnostic(value: str, temporary_path: Path | None) -> str:
    sanitized = value.replace(str(ROOT), "<workspace>")
    if temporary_path is not None:
        sanitized = sanitized.replace(str(temporary_path), "<temporary>")
    return sanitized


if __name__ == "__main__":
    raise SystemExit(main())
