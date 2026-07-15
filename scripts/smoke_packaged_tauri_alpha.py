#!/usr/bin/env python3
"""Exercise the macOS Alpha bundle's real Tauri-to-sidecar lifecycle.

This is deliberately local-only: it uses a temporary HOME and Library,
removes Provider configuration, and drives the deterministic robot-arm path.
It proves the release desktop executable, rather than a repository Python
process, launches its bundled frozen Agent sidecar.
"""

from __future__ import annotations

import json
import os
import signal
import subprocess
import tempfile
import time
import urllib.request
from base64 import b64decode
from pathlib import Path

from smoke_packaged_sidecar_alpha import _assert, _create_and_export_editable_asset, _request


ROOT = Path(__file__).resolve().parents[1]
APP_BINARY = (
    ROOT
    / "apps/desktop/src-tauri/target/release/bundle/macos/CAD 工作台.app"
    / "Contents/MacOS/wushen-forge-desktop"
)
APP_BUNDLE = APP_BINARY.parents[2]
HEALTH_URL = "http://127.0.0.1:8000/api/health"
ARTIFACT_DIRECTORY = os.environ.get("FORGECAD_NATIVE_SMOKE_ARTIFACT_DIR")


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
        _assert((library_root / "library.db").is_file(), "native first initialization did not create the Library")
        asset_version_id = _create_and_export_editable_asset(8000, library_root)
    finally:
        _stop_desktop_and_listener(first)

    _assert(_listener_pid() is None, "sidecar listener survived desktop shutdown cleanup")

    restarted = _start_desktop(temporary_path, environment)
    try:
        listener_pid = _wait_for_native_health(restarted)
        _assert(
            _is_descendant(listener_pid, restarted),
            "restarted Agent listener is not owned by the packaged desktop supervisor",
        )
        recovered = _request(8000, f"/api/v1/agent/asset-versions/{asset_version_id}")
        exported = _request(8000, f"/api/v1/agent/asset-versions/{asset_version_id}:export", method="POST")
        _assert(recovered["asset_version_id"] == asset_version_id, "desktop restart did not restore the editable asset")
        _assert(b64decode(exported["glb_base64"])[:4] == b"glTF", "recovered native GLB export is invalid")
    finally:
        _stop_desktop_and_listener(restarted)

    return {
        "ok": True,
        "supervisor_mode": "packaged-sidecar",
        "empty_library_initialized": True,
        "editable_glb_export": True,
        "packaged_manifold_csg": True,
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
    ):
        environment.pop(name, None)
    environment["HOME"] = str(temporary)
    environment["WUSHEN_LIBRARY_ROOT"] = str(library_root)
    environment["WUSHEN_AGENT_RUNTIME_MODE"] = "packaged-sidecar"
    environment["FORGECAD_DISABLE_PROVIDER_CONFIG"] = "1"
    environment["FORGECAD_CONCEPT_WORKER_ENABLED"] = "0"
    environment["WUSHEN_LOCAL_WORKER_ENABLED"] = "0"
    return environment


def _start_desktop(temporary: Path, environment: dict[str, str]) -> int:
    # macOS must launch an .app through LaunchServices. Executing the inner
    # Mach-O directly skips normal app activation in headless shells and does
    # not exercise the same Tauri lifecycle as a user opening the bundle.
    existing = _desktop_pids()
    subprocess.run(["open", "-n", str(APP_BUNDLE)], cwd=temporary, env=environment, check=True)
    for _ in range(100):
        created = _desktop_pids() - existing
        if created:
            return max(created)
        time.sleep(0.1)
    raise AssertionError("LaunchServices did not start the packaged desktop app")


def _wait_for_native_health(desktop_pid: int) -> int:
    for _ in range(350):
        if not _process_exists(desktop_pid):
            raise AssertionError("packaged desktop exited before sidecar health")
        try:
            with urllib.request.urlopen(HEALTH_URL, timeout=0.25) as response:
                payload = json.loads(response.read().decode("utf-8"))
            _assert(payload == {"status": "ok", "service": "wushen-agent", "mode": "sqlite_mock"}, "unexpected native health payload")
            listener_pid = _listener_pid()
            _assert(listener_pid is not None, "healthy sidecar did not expose a listening process")
            return listener_pid
        except OSError:
            time.sleep(0.1)
    raise AssertionError("packaged desktop did not make its sidecar healthy within 35 seconds")


def _wait_for_log(path: Path, expected: str) -> None:
    for _ in range(50):
        if path.is_file() and expected in path.read_text(encoding="utf-8"):
            return
        time.sleep(0.1)
    raise AssertionError(f"native supervisor did not report {expected!r}")


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
