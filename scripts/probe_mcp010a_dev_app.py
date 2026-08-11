#!/usr/bin/env python3
"""Verify the MCP010A Dev.app and its sibling Runtime using isolated data."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import plistlib
import selectors
import socket
import stat
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_APP = Path.home() / "Applications" / "ForgeCAD Runtime Dev.app"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, default=DEFAULT_APP)
    parser.add_argument("--verify-only", action="store_true")
    parser.add_argument("--evidence", type=Path)
    parser.add_argument(
        "--task-id",
        default="FGC-MCP010A",
        help="Task recorded in an optional probe receipt.",
    )
    parser.add_argument("--timeout", type=float, default=20.0)
    return parser.parse_args()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def component_paths(app: Path) -> dict[str, Path]:
    return {
        "forgecad-mcp": app / "Contents" / "Resources" / "forgecad-mcp",
        "forgecad-runtime": app / "Contents" / "Resources" / "forgecad-runtime",
        "forgecad-geometry-worker": app
        / "Contents"
        / "Resources"
        / "forgecad-geometry-worker",
        "forgecad-viewer": app / "Contents" / "MacOS" / "forgecad-desktop",
    }


def build_identity(path: Path) -> dict[str, Any]:
    completed = subprocess.run(
        [str(path), "--build-identity"],
        text=True,
        capture_output=True,
        timeout=20,
        check=True,
    )
    value = json.loads(completed.stdout)
    if not isinstance(value, dict):
        raise SystemExit("component build identity was not an object")
    return value


def verify_app(app: Path) -> tuple[dict[str, Any], dict[str, Path]]:
    if not app.is_dir():
        raise SystemExit("ForgeCAD Runtime Dev.app is not installed")
    paths = component_paths(app)
    for name, path in paths.items():
        if not path.is_file() or not os.access(path, os.X_OK):
            raise SystemExit(f"missing packaged executable: {name}")
    resources = app / "Contents" / "Resources"
    executable_resources = {
        path.name
        for path in resources.iterdir()
        if path.is_file() and path.stat().st_mode & stat.S_IXUSR
    }
    expected_resources = {
        "forgecad-mcp",
        "forgecad-runtime",
        "forgecad-geometry-worker",
    }
    if executable_resources != expected_resources:
        raise SystemExit(f"unexpected executable Resources: {sorted(executable_resources)}")
    if (resources / "forgecad-mcp-host").exists():
        raise SystemExit("obsolete forgecad-mcp-host is present")
    with (app / "Contents" / "Info.plist").open("rb") as source:
        info = plistlib.load(source)
    if info.get("CFBundleIdentifier") != "local.forgecad.runtime.dev":
        raise SystemExit("development bundle identifier mismatch")
    manifest_path = resources / "forgecad-dev-build-manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    cohort = manifest.get("build_cohort_sha256")
    if not isinstance(cohort, str) or len(cohort) != 64:
        raise SystemExit("development build cohort is invalid")
    for name, expected in manifest.get("resource_sha256", {}).items():
        path = paths.get(name)
        if path is None or sha256_file(path) != expected:
            raise SystemExit(f"packaged resource hash mismatch: {name}")
    for name, path in paths.items():
        if build_identity(path).get("build_cohort_sha256") != cohort:
            raise SystemExit(f"component cohort mismatch: {name}")
    subprocess.run(
        ["codesign", "--verify", "--deep", "--strict", "--verbose=2", str(app)],
        text=True,
        capture_output=True,
        timeout=20,
        check=True,
    )
    return manifest, paths


class McpClient:
    def __init__(self, command: Path, environment: dict[str, str], timeout: float) -> None:
        self.timeout = timeout
        self.next_id = 1
        self.process = subprocess.Popen(
            [str(command), "serve", "--stdio"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            env=environment,
            bufsize=1,
        )
        self.selector = selectors.DefaultSelector()
        assert self.process.stdout is not None
        self.selector.register(self.process.stdout, selectors.EVENT_READ)

    def notify(self, method: str) -> None:
        assert self.process.stdin is not None
        self.process.stdin.write(json.dumps({"jsonrpc": "2.0", "method": method}) + "\n")
        self.process.stdin.flush()

    def request(self, method: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
        identifier = self.next_id
        self.next_id += 1
        payload: dict[str, Any] = {"jsonrpc": "2.0", "id": identifier, "method": method}
        if params is not None:
            payload["params"] = params
        assert self.process.stdin is not None
        self.process.stdin.write(json.dumps(payload, separators=(",", ":")) + "\n")
        self.process.stdin.flush()
        deadline = time.monotonic() + self.timeout
        while time.monotonic() < deadline:
            events = self.selector.select(max(0.0, deadline - time.monotonic()))
            if not events:
                break
            assert self.process.stdout is not None
            line = self.process.stdout.readline()
            if not line:
                break
            response = json.loads(line)
            if response.get("id") == identifier:
                return response
        raise SystemExit(f"MCP response timed out: {method}")

    def tool(self, name: str, arguments: dict[str, Any] | None = None) -> Any:
        response = self.request(
            "tools/call", {"name": name, "arguments": arguments or {}}
        )
        if "error" in response:
            raise SystemExit(f"MCP tool protocol error: {name}")
        result = response.get("result", {})
        if result.get("isError"):
            raise SystemExit(f"MCP tool returned an error: {name}")
        return result.get("structuredContent")

    def close(self) -> None:
        if self.process.stdin is not None:
            self.process.stdin.close()
        try:
            self.process.wait(timeout=20)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait(timeout=5)
            raise SystemExit("MCP did not stop after stdio EOF")
        if self.process.returncode != 0:
            assert self.process.stderr is not None
            detail = " ".join(self.process.stderr.read().split())[:256]
            raise SystemExit(f"MCP exited {self.process.returncode}: {detail}")


def shutdown_isolated_runtime(data_root: Path) -> None:
    """Authenticate to the probe-owned Runtime and stop it before temp cleanup."""
    ready_path = data_root / "ipc" / "ready.json"
    try:
        ready = json.loads(ready_path.read_text(encoding="utf-8"))
        endpoint = ready["socket_path"]
        token = ready["token"]
    except (FileNotFoundError, KeyError, json.JSONDecodeError, OSError):
        return

    connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    connection.settimeout(2.0)
    buffered = b""

    def exchange(payload: dict[str, Any]) -> dict[str, Any]:
        nonlocal buffered
        connection.sendall(
            json.dumps(payload, separators=(",", ":")).encode("utf-8") + b"\n"
        )
        while b"\n" not in buffered:
            chunk = connection.recv(65536)
            if not chunk:
                raise SystemExit("isolated Runtime closed before cleanup response")
            buffered += chunk
            if len(buffered) > 1024 * 1024:
                raise SystemExit("isolated Runtime cleanup response exceeded limit")
        line, buffered = buffered.split(b"\n", 1)
        value = json.loads(line)
        if not isinstance(value, dict):
            raise SystemExit("isolated Runtime cleanup response was not an object")
        return value

    try:
        connection.connect(endpoint)
        authenticated = exchange(
            {"version": 1, "token": token, "method": "authenticate", "payload": None}
        )
        if authenticated.get("ok") is not True:
            raise SystemExit("isolated Runtime cleanup authentication failed")
        stopped = exchange(
            {
                "version": 1,
                "token": None,
                "method": "runtime_shutdown",
                "payload": None,
            }
        )
        if stopped.get("ok") is not True:
            raise SystemExit("isolated Runtime cleanup was rejected")
    finally:
        connection.close()

    deadline = time.monotonic() + 3.0
    while ready_path.exists() and time.monotonic() < deadline:
        time.sleep(0.02)
    if ready_path.exists():
        raise SystemExit("isolated Runtime did not stop before temporary data cleanup")


def write_receipt(path: Path | None, receipt: dict[str, Any]) -> None:
    if path is None:
        return
    resolved = path if path.is_absolute() else ROOT / path
    resolved.parent.mkdir(parents=True, exist_ok=True)
    resolved.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    manifest, paths = verify_app(args.app)
    cohort = str(manifest["build_cohort_sha256"])
    if args.verify_only:
        receipt = {
            "schema_version": (
                "ForgeCADMCP010ADevPackageVerify@1"
                if args.task_id == "FGC-MCP010A"
                else "ForgeCADDevAppPackageVerify@1"
            ),
            "task_id": args.task_id,
            "status": "PASS",
            "build_cohort_sha256": cohort,
            "resource_allowlist": [
                "forgecad-geometry-worker",
                "forgecad-mcp",
                "forgecad-runtime"
            ],
            "codesign": "ad-hoc deep strict PASS",
            "runtime_probe": "NOT_RUN",
        }
        write_receipt(args.evidence, receipt)
        print(json.dumps(receipt, sort_keys=True))
        return 0

    environment = os.environ.copy()
    for key in (
        "FORGECAD_RUNTIME_COMMAND",
        "FORGECAD_RUNTIME_SOCKET",
        "FORGECAD_RUNTIME_TOKEN",
        "FORGECAD_RUNTIME_READY_FILE",
        "FORGECAD_RUNTIME_STATUS_FILE",
    ):
        environment.pop(key, None)
    environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
    with tempfile.TemporaryDirectory(prefix="forgecad-mcp010a-") as temporary:
        runtime_data = Path(temporary) / "runtime-data"
        environment["FORGECAD_RUNTIME_DATA_DIR"] = str(runtime_data)
        client = McpClient(paths["forgecad-mcp"], environment, args.timeout)
        try:
            initialized = client.request(
                "initialize",
                {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {"name": "forgecad-mcp010a-probe", "version": "1"},
                },
            )
            if initialized.get("result", {}).get("protocolVersion") != "2025-06-18":
                raise SystemExit("Codex compatibility initialize failed")
            client.notify("notifications/initialized")
            state = "Starting"
            deadline = time.monotonic() + args.timeout
            while time.monotonic() < deadline:
                status = client.tool("runtime_status")
                state = status.get("state") if isinstance(status, dict) else None
                if state == "Ready":
                    break
                if state in {"Degraded", "Busy"}:
                    raise SystemExit(f"sibling Runtime failed to become Ready: {state}")
                time.sleep(0.1)
            if state != "Ready":
                raise SystemExit("sibling Runtime did not become Ready")
            capabilities = client.tool("capabilities_get")
            if not isinstance(capabilities, dict):
                raise SystemExit("capabilities_get did not return an object")
            if capabilities.get("build_cohort_match") is not True:
                raise SystemExit("MCP and Runtime build cohorts do not match")
            if capabilities.get("build_cohort_sha256") != cohort:
                raise SystemExit("Runtime build cohort differs from installed manifest")
            if capabilities.get("mcp_build_cohort_sha256") != cohort:
                raise SystemExit("MCP build cohort differs from installed manifest")
            project = client.tool(
                "project_create",
                {"name": "MCP010A isolated activation probe", "policy": {"profile": "mvp"}},
            )
            if not isinstance(project, dict) or not project.get("project_id"):
                raise SystemExit("isolated project_create did not return a project")
            projects = client.tool("project_list")
            if not isinstance(projects, list) or len(projects) != 1:
                raise SystemExit("isolated project was not readable")
        finally:
            try:
                shutdown_isolated_runtime(runtime_data)
            finally:
                client.close()

    receipt = {
        "schema_version": (
            "ForgeCADMCP010ADevProbe@1"
            if args.task_id == "FGC-MCP010A"
            else "ForgeCADDevAppProbe@1"
        ),
        "task_id": args.task_id,
        "status": "PASS",
        "protocol_version": "2025-06-18",
        "runtime_state": "Ready",
        "build_cohort_sha256": cohort,
        "build_cohort_match": True,
        "isolated_project_create": "PASS",
        "persistent_user_data_touched": False,
        "codex_desktop_restart_gate": "NOT_RUN",
    }
    write_receipt(args.evidence, receipt)
    print(json.dumps(receipt, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
