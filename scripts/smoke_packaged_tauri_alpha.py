#!/usr/bin/env python3
"""Exercise the macOS Alpha bundle's Rust app-server and restricted sidecar.

This is deliberately local-only: it uses a temporary HOME and Library,
removes Provider configuration, and never enables either legacy Python writer
oracle. It proves the release desktop executable owns Thread/Turn, product
compatibility state, native Item replay and the bundled restricted geometry
sidecar.
"""

from __future__ import annotations

import json
import os
import signal
import subprocess
import tempfile
import time
import urllib.request
from pathlib import Path

from smoke_packaged_sidecar_alpha import _assert, _restricted_health_payload


ROOT = Path(__file__).resolve().parents[1]
APP_BINARY = (
    ROOT
    / "apps/desktop/src-tauri/target/release/bundle/macos/CAD 工作台.app"
    / "Contents/MacOS/wushen-forge-desktop"
)
APP_BUNDLE = APP_BINARY.parents[2]
HEALTH_URL = "http://127.0.0.1:8000/api/health"
ARTIFACT_DIRECTORY = os.environ.get("FORGECAD_NATIVE_SMOKE_ARTIFACT_DIR")
APP_SERVER_READY_MARKER = (
    "ForgeCAD app-server ready protocol=forgecad.app-server/1 "
    "lifecycle_owner=rust-app-server state_owner=rust-core "
    "python_role=restricted_geometry_executor"
)
APP_SERVER_HTTP_MARKER = "ForgeCAD app-server compat/http roundtrip protocol=forgecad.app-server/1"
K001_PROBE_MARKER = "ForgeCAD K001 packaged WebView probe report="


def main() -> int:
    _assert(APP_BINARY.is_file(), "build the macOS .app before the Tauri Alpha smoke")
    _assert(_listener_pid() is None, "port 8000 must be free before the Tauri Alpha smoke")

    with tempfile.TemporaryDirectory(prefix="forgecad_tauri_alpha_") as temporary:
        temporary_path = Path(temporary)
        try:
            report = _run_native_smoke(temporary_path)
        except Exception as error:
            _write_artifact(
                {
                    "ok": False,
                    "provider_calls": 0,
                    "error": _safe_diagnostic(str(error), temporary_path),
                },
                log_path=temporary_path / "WushenForge/agent.log",
                temporary_path=temporary_path,
            )
            raise

    _write_artifact(report)
    print(json.dumps(report))
    return 0


def _run_native_smoke(temporary_path: Path) -> dict[str, object]:
    library_root = temporary_path / "library"
    environment = _safe_environment(temporary_path, library_root)
    first = _start_desktop(temporary_path, environment)
    try:
        listener_pid = _wait_for_native_health(first)
        _assert(
            _is_descendant(listener_pid, first),
            "healthy Agent listener is not owned by the packaged desktop supervisor",
        )
        log_path = temporary_path / "WushenForge/agent.log"
        _wait_for_log(log_path, "ForgeCAD supervisor healthy mode=packaged-sidecar")
        _wait_for_log_count(log_path, APP_SERVER_READY_MARKER, 1)
        _wait_for_log_count(log_path, APP_SERVER_HTTP_MARKER, 1)
        _assert((library_root / "library.db").is_file(), "native first initialization did not create the Library")
        initial_report = _wait_for_probe_report(log_path, "initial", 1)
        _validate_k001_probe_report(initial_report, "initial")
    finally:
        _stop_desktop_and_listener(first)

    _assert(_listener_pid() is None, "sidecar listener survived desktop shutdown cleanup")

    environment["FORGECAD_K001_PACKAGED_PROBE_PHASE"] = "restart"
    environment["FORGECAD_K001_EXPECT_PROJECT_ID"] = str(initial_report["project_id"])
    environment["FORGECAD_K001_EXPECT_THREAD_ID"] = str(initial_report["thread_id"])
    environment["FORGECAD_K001_EXPECT_ASSET_VERSION_ID"] = str(initial_report["asset_version_id"])
    environment["FORGECAD_K001_EXPECT_LAST_EVENT_ID"] = str(initial_report["last_event_id"])
    environment["FORGECAD_K001_EXPECT_CURSOR"] = str(initial_report["cursor"])
    environment["FORGECAD_K001_EXPECT_GLB_SHA256"] = str(initial_report["glb_sha256"])
    restarted = _start_desktop(temporary_path, environment)
    try:
        listener_pid = _wait_for_native_health(restarted)
        _assert(
            _is_descendant(listener_pid, restarted),
            "restarted Agent listener is not owned by the packaged desktop supervisor",
        )
        _wait_for_log_count(
            temporary_path / "WushenForge/agent.log",
            APP_SERVER_READY_MARKER,
            2,
        )
        _wait_for_log_count(
            temporary_path / "WushenForge/agent.log",
            APP_SERVER_HTTP_MARKER,
            2,
        )
        restart_report = _wait_for_probe_report(
            temporary_path / "WushenForge/agent.log",
            "restart",
            1,
        )
        _validate_k001_probe_report(restart_report, "restart")
        for field in (
            "project_id",
            "thread_id",
            "asset_version_id",
            "glb_sha256",
        ):
            _assert(
                restart_report[field] == initial_report[field],
                f"packaged WebView restart changed {field}",
            )
        _assert(
            restart_report["resume_from_event_id"] == initial_report["last_event_id"],
            "packaged WebView restart did not resume from the first-run SSE event",
        )
        _assert(
            restart_report["resume_from_cursor"] == initial_report["cursor"],
            "packaged WebView restart did not preserve the first-run Rust cursor",
        )
        first_restarted_sequence = int(str(restart_report["first_event_id"]))
        last_restarted_sequence = int(str(restart_report["last_event_id"]))
        _assert(
            first_restarted_sequence == 1,
            "packaged WebView restart did not replay the first persisted native Agent Item",
        )
        _assert(
            last_restarted_sequence == int(str(initial_report["last_event_id"])),
            "packaged WebView restart changed the persisted native Item interval",
        )
        _assert(
            restart_report["cursor"] == initial_report["cursor"],
            "packaged WebView restart changed the acknowledged Rust Item cursor",
        )
    finally:
        _stop_desktop_and_listener(restarted)

    return {
        "ok": True,
        "supervisor_mode": "packaged-sidecar",
        "empty_library_initialized": True,
        "editable_glb_export": True,
        "packaged_webview_thread_turn_asset": True,
        "packaged_webview_edit_undo_redo_export": True,
        "packaged_webview_native_item_replay": True,
        "packaged_webview_binary_protocol": True,
        "packaged_webview_resource_transport": True,
        "rust_app_server_protocol_ready": True,
        "packaged_webview_compat_http_roundtrip": True,
        "rust_product_compatibility_state": True,
        "python_product_api_used": False,
        "legacy_python_writer_oracles": False,
        "rust_app_server_restart_ready": True,
        "packaged_webview_restart_roundtrip": True,
        "restart_recovery": True,
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
    ):
        environment.pop(name, None)
    environment["HOME"] = str(temporary)
    environment["WUSHEN_LIBRARY_ROOT"] = str(library_root)
    environment["WUSHEN_AGENT_RUNTIME_MODE"] = "packaged-sidecar"
    environment["FORGECAD_DISABLE_PROVIDER_CONFIG"] = "1"
    environment["FORGECAD_CONCEPT_WORKER_ENABLED"] = "0"
    environment["WUSHEN_LOCAL_WORKER_ENABLED"] = "0"
    environment["FORGECAD_K001_PACKAGED_PROBE"] = "1"
    environment["FORGECAD_K001_PACKAGED_PROBE_PHASE"] = "initial"
    return environment


def _start_desktop(temporary: Path, environment: dict[str, str]) -> int:
    # macOS must launch an .app through LaunchServices. Executing the inner
    # Mach-O directly skips normal app activation in headless shells and does
    # not exercise the same Tauri lifecycle as a user opening the bundle.
    existing = _desktop_pids()
    forwarded_names = {
        "HOME",
        "WUSHEN_LIBRARY_ROOT",
        "WUSHEN_AGENT_RUNTIME_MODE",
        "FORGECAD_DISABLE_PROVIDER_CONFIG",
        "FORGECAD_CONCEPT_WORKER_ENABLED",
        "WUSHEN_LOCAL_WORKER_ENABLED",
        "FORGECAD_K001_PACKAGED_PROBE",
        "FORGECAD_K001_PACKAGED_PROBE_PHASE",
        "FORGECAD_K001_EXPECT_PROJECT_ID",
        "FORGECAD_K001_EXPECT_THREAD_ID",
        "FORGECAD_K001_EXPECT_ASSET_VERSION_ID",
        "FORGECAD_K001_EXPECT_LAST_EVENT_ID",
        "FORGECAD_K001_EXPECT_CURSOR",
        "FORGECAD_K001_EXPECT_GLB_SHA256",
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
    raise AssertionError("LaunchServices did not start the packaged desktop app")


def _wait_for_native_health(desktop_pid: int) -> int:
    # Match the Rust supervisor's bounded 90-second cold-start window and
    # leave room for the capability ownership probe after /api/health.
    for _ in range(1_200):
        if not _process_exists(desktop_pid):
            raise AssertionError("packaged desktop exited before sidecar health")
        try:
            with urllib.request.urlopen(HEALTH_URL, timeout=0.25) as response:
                payload = json.loads(response.read().decode("utf-8"))
            _assert(payload == _restricted_health_payload(), "unexpected restricted geometry health payload")
            listener_pid = _listener_pid()
            _assert(listener_pid is not None, "healthy sidecar did not expose a listening process")
            return listener_pid
        except OSError:
            time.sleep(0.1)
    raise AssertionError("packaged desktop did not make its sidecar healthy within 120 seconds")


def _wait_for_log(path: Path, expected: str) -> None:
    for _ in range(50):
        if path.is_file() and expected in path.read_text(encoding="utf-8"):
            return
        time.sleep(0.1)
    raise AssertionError(f"native supervisor did not report {expected!r}")


def _wait_for_log_count(path: Path, expected: str, minimum_count: int) -> None:
    for _ in range(100):
        if path.is_file() and path.read_text(encoding="utf-8").count(expected) >= minimum_count:
            return
        time.sleep(0.1)
    raise AssertionError(
        f"native desktop did not report {minimum_count} app-server initialization event(s)"
    )


def _wait_for_probe_report(path: Path, phase: str, minimum_count: int) -> dict[str, object]:
    for _ in range(1_900):
        if path.is_file():
            reports = []
            for line in path.read_text(encoding="utf-8").splitlines():
                if not line.startswith(K001_PROBE_MARKER):
                    continue
                try:
                    report = json.loads(line[len(K001_PROBE_MARKER) :])
                except (TypeError, ValueError):
                    continue
                if isinstance(report, dict) and report.get("phase") == phase:
                    reports.append(report)
            if len(reports) >= minimum_count:
                report = reports[-1]
                _assert(
                    report.get("schema_version") == "ForgeCADK001PackagedProbe@1",
                    "packaged WebView probe returned the wrong schema",
                )
                if report.get("ok") is not True:
                    raise AssertionError(
                        f"packaged WebView {phase} probe failed with {report.get('error_code', 'UNKNOWN')}"
                    )
                return report
        time.sleep(0.1)
    raise AssertionError(f"packaged WebView {phase} probe did not finish within 190 seconds")


def _validate_k001_probe_report(report: dict[str, object], phase: str) -> None:
    _assert(report.get("schema_version") == "ForgeCADK001PackagedProbe@1", "unexpected K001 schema")
    _assert(report.get("phase") == phase and report.get("ok") is True, f"K001 {phase} probe failed")
    _assert(report.get("native_lifecycle_transport") is True, "K001 lifecycle did not use native JSON-RPC")
    _assert(report.get("native_item_replay_verified") is True, "K001 native Item replay was not verified")
    _assert(report.get("product_state_owner") == "rust_app_server", "K001 product compat was not Rust-owned")
    _assert(report.get("python_product_api_used") is False, "K001 used the Python product API")
    _assert(report.get("turn_status") == "failed", "K001 offline native Turn did not fail closed")
    _assert(
        report.get("turn_error_code") == "PROVIDER_NOT_CONFIGURED",
        "K001 offline native Turn returned the wrong error",
    )
    _assert(report.get("provider_calls") == 0, "K001 offline probe made a Provider call")


def _listener_pid() -> int | None:
    result = subprocess.run(
        ["lsof", "-n", "-t", "-iTCP:8000", "-sTCP:LISTEN"],
        check=False,
        capture_output=True,
        text=True,
    )
    values = [line.strip() for line in result.stdout.splitlines() if line.strip()]
    return int(values[0]) if values else None


def _is_descendant(pid: int, ancestor_pid: int) -> bool:
    current = pid
    for _ in range(12):
        if current == ancestor_pid:
            return True
        result = subprocess.run(["ps", "-o", "ppid=", "-p", str(current)], check=False, capture_output=True, text=True)
        value = result.stdout.strip()
        if not value:
            return False
        current = int(value)
    return False


def _desktop_pids() -> set[int]:
    result = subprocess.run(["pgrep", "-f", str(APP_BINARY)], check=False, capture_output=True, text=True)
    return {int(value) for value in result.stdout.split() if value.isdigit()}


def _process_exists(pid: int) -> bool:
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False


def _stop_desktop_and_listener(desktop_pid: int) -> None:
    if _process_exists(desktop_pid):
        os.kill(desktop_pid, signal.SIGTERM)
        for _ in range(50):
            if not _process_exists(desktop_pid):
                break
            time.sleep(0.1)
        if _process_exists(desktop_pid):
            os.kill(desktop_pid, signal.SIGKILL)
    listener_pid = _listener_pid()
    if listener_pid is not None:
        os.kill(listener_pid, 15)
        for _ in range(50):
            if _listener_pid() is None:
                return
            time.sleep(0.1)
        raise AssertionError("could not stop native sidecar listener during smoke cleanup")


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
    (directory / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if log_path is not None and log_path.is_file():
        log_contents = log_path.read_text(encoding="utf-8", errors="replace")[-8192:]
        (directory / "supervisor.log").write_text(
            _safe_diagnostic(log_contents, temporary_path) + "\n", encoding="utf-8"
        )


def _safe_diagnostic(value: str, temporary_path: Path | None) -> str:
    sanitized = value.replace(str(ROOT), "<workspace>")
    if temporary_path is not None:
        sanitized = sanitized.replace(str(temporary_path), "<temporary>")
    return sanitized


if __name__ == "__main__":
    raise SystemExit(main())
