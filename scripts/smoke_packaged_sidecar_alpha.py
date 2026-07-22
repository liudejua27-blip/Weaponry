#!/usr/bin/env python3
"""Verify the frozen Python sidecar is a restricted geometry facet only.

K003 moved every product and Agent lifecycle write into the Rust app-server.
This direct-sidecar smoke therefore checks boot, the capability-gated ownership
contract, stable product tombstones, zero product database creation and restart.
It never enables either legacy Python writer oracle.
"""

from __future__ import annotations

import json
import os
import signal
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SIDECAR = ROOT / "apps/desktop/src-tauri/binaries/wushen-agent-aarch64-apple-darwin"
CAPABILITY_ENV = "FORGECAD_RESTRICTED_GEOMETRY_CAPABILITY_TOKEN"
CAPABILITY_HEADER = "X-ForgeCAD-Restricted-Geometry-Capability"
OWNERSHIP_PATH = "/api/v1/internal/geometry/capability/ownership"
PRODUCT_TOMBSTONES = (
    "/api/v1/projects",
    "/api/v1/agent/threads",
    "/api/v1/internal/k002/lifecycle",
    "/api/v1/internal/k002/product-tools/execute",
)


def main() -> int:
    _assert(
        SIDECAR.is_file() and SIDECAR.stat().st_size > 0,
        "frozen macOS arm64 sidecar is required before the packaged Alpha smoke",
    )
    capability = "7a" * 32
    with tempfile.TemporaryDirectory(prefix="forgecad_packaged_geometry_") as temporary:
        temporary_path = Path(temporary)
        environment = _safe_environment(temporary_path, capability)
        for phase in ("initial", "restart"):
            port = _free_port()
            process = _start_sidecar(temporary, environment, port)
            try:
                payload = _wait_for_health(port, process)
                _assert(
                    payload == _restricted_health_payload(),
                    f"unexpected {phase} health payload",
                )
                _assert_restricted_ownership(port, capability)
                _assert_product_tombstones(port)
                _assert(
                    not any(temporary_path.rglob("library.db")),
                    "restricted Python sidecar created a product database",
                )
            finally:
                _stop_sidecar(process)

    print(
        json.dumps(
            {
                "ok": True,
                "packaged_sidecar_health": True,
                "restricted_geometry_ownership": True,
                "python_product_routes_status": 410,
                "python_lifecycle_routes_status": 410,
                "python_product_database_created": False,
                "legacy_python_writer_oracles": False,
                "restart_recovery": True,
                "provider_calls": 0,
            },
            separators=(",", ":"),
        )
    )
    return 0


def _safe_environment(home: Path, capability: str) -> dict[str, str]:
    environment: dict[str, str] = {}
    for name in ("PATH", "LANG", "LC_ALL", "LC_CTYPE", "TMPDIR"):
        value = os.environ.get(name)
        if value:
            environment[name] = value
    environment.update(
        {
            "HOME": str(home),
            "WUSHEN_AGENT_RUNTIME_MODE": "packaged-sidecar",
            "FORGECAD_DISABLE_PROVIDER_CONFIG": "1",
            "FORGECAD_CONCEPT_WORKER_ENABLED": "0",
            "WUSHEN_LOCAL_WORKER_ENABLED": "0",
            CAPABILITY_ENV: capability,
        }
    )
    return environment


def _start_sidecar(
    temporary: str,
    environment: dict[str, str],
    port: int,
) -> subprocess.Popen[str]:
    return subprocess.Popen(
        [str(SIDECAR), "agent", "serve", "--host", "127.0.0.1", "--port", str(port)],
        cwd=temporary,
        env=environment,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )


def _stop_sidecar(process: subprocess.Popen[str]) -> None:
    if process.poll() is not None:
        return
    os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=10)


def _assert_restricted_ownership(port: int, capability: str) -> None:
    status, payload = _request_json(
        port,
        OWNERSHIP_PATH,
        headers={CAPABILITY_HEADER: capability},
    )
    _assert(status == 200, "restricted ownership endpoint did not return HTTP 200")
    expected = {
        "schema_version": "RestrictedGeometryCapabilityOwnership@1",
        "protocol_version": "forgecad.restricted-geometry/1",
        "capability_owner": "rust_forgecad_core",
        "python_role": "restricted_geometry_executor",
        "database_access": False,
        "object_store_access": False,
        "provider_access": False,
        "thread_session_access": False,
        "snapshot_write": False,
        "accepts_caller_glb": False,
        "persistent_artifacts": False,
        "actions": ["compile_readback", "render"],
    }
    _assert(payload == expected, "restricted geometry ownership payload diverged")


def _restricted_health_payload() -> dict[str, object]:
    return {
        "status": "ok",
        "service": "forgecad-restricted-geometry-executor",
        "mode": "restricted_geometry_executor",
        "schema_version": "RestrictedGeometryExecutorHealth@1",
        "python_role": "restricted_geometry_executor",
        "database_access": False,
        "object_store_access": False,
        "provider_access": False,
        "snapshot_write": False,
        "persistent_state_writer": False,
    }


def _assert_product_tombstones(port: int) -> None:
    for path in PRODUCT_TOMBSTONES:
        status, payload = _request_json(port, path, method="POST", body={})
        _assert(status == 410, f"restricted Python route {path} did not return HTTP 410")
        _assert(isinstance(payload, dict), f"restricted Python route {path} returned invalid JSON")
        error = payload.get("error")
        _assert(isinstance(error, dict), f"restricted Python route {path} omitted its error")
        _assert(error.get("code") == "PRODUCT_STATE_RUST_OWNED", f"wrong tombstone for {path}")
        _assert(error.get("recoverable") is False, f"tombstone for {path} is recoverable")


def _request_json(
    port: int,
    path: str,
    *,
    method: str = "GET",
    body: dict[str, Any] | None = None,
    headers: dict[str, str] | None = None,
) -> tuple[int, Any]:
    data = json.dumps(body, separators=(",", ":")).encode("utf-8") if body is not None else None
    request_headers = dict(headers or {})
    if data is not None:
        request_headers["Content-Type"] = "application/json"
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}{path}",
        data=data,
        headers=request_headers,
        method=method,
    )
    try:
        with urllib.request.urlopen(request, timeout=3) as response:
            return response.status, json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as error:
        return error.code, json.loads(error.read().decode("utf-8"))


def _wait_for_health(port: int, process: subprocess.Popen[str]) -> dict[str, str]:
    endpoint = f"http://127.0.0.1:{port}/api/health"
    # PyInstaller/one-file extraction can take over 30 seconds on a cold
    # macOS launch. Keep this bounded, but do not race the real supervisor.
    for _ in range(900):
        if process.poll() is not None:
            stderr = process.stderr.read() if process.stderr else ""
            raise AssertionError(f"packaged sidecar exited before health check: {stderr[-2000:]}")
        try:
            with urllib.request.urlopen(endpoint, timeout=0.25) as response:
                return json.loads(response.read().decode("utf-8"))
        except OSError:
            time.sleep(0.1)
    raise AssertionError("packaged sidecar did not become healthy within 90 seconds")


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as socket_handle:
        socket_handle.bind(("127.0.0.1", 0))
        return int(socket_handle.getsockname()[1])


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    raise SystemExit(main())
