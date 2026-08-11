#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/forgecad-desktop-mcp004.XXXXXX")"
trap 'rm -rf "$TEMP_ROOT"' EXIT

TARGET_DIR="$TEMP_ROOT/cargo-target"
RUNTIME_DATA="$TEMP_ROOT/runtime-data"
MISSING_RUNTIME="$TEMP_ROOT/runtime-does-not-exist"

# The MCP004 authenticated-IPC regression exercises a real geometry_prepare
# through Runtime.  Production Runtime never links a compiler fallback: it
# resolves only this same-target fixed Worker sibling.  Build it before the
# MCP test binary so a test executable in `debug/deps` can resolve the
# `debug/forgecad-geometry-worker` sibling exactly as Runtime does.
CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/geometry-worker/Cargo.toml" \
  --bin forgecad-geometry-worker --offline

test -x "$TARGET_DIR/debug/forgecad-geometry-worker"

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo test \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp --offline

CARGO_TARGET_DIR="$TARGET_DIR" \
  "$PROJECT_ROOT/script/with_rust_toolchain.sh" cargo build \
  --manifest-path "$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml" \
  -p forgecad-mcp --bin forgecad-mcp \
  -p forgecad-runtime --bin forgecad-runtime --offline

FORGECAD_TEST_ROOT="$TEMP_ROOT" \
FORGECAD_TEST_MCP="$TARGET_DIR/debug/forgecad-mcp" \
FORGECAD_TEST_RUNTIME_BINARY="$TARGET_DIR/debug/forgecad-runtime" \
FORGECAD_TEST_RUNTIME="$MISSING_RUNTIME" \
FORGECAD_TEST_DATA="$RUNTIME_DATA" \
python3 - <<'PY'
import atexit
import concurrent.futures
import hashlib
import json
import os
import selectors
import socket
import subprocess
import sys
import time

mcp = os.environ["FORGECAD_TEST_MCP"]
runtime_binary = os.environ["FORGECAD_TEST_RUNTIME_BINARY"]
missing_runtime = os.environ["FORGECAD_TEST_RUNTIME"]
runtime_data = os.environ["FORGECAD_TEST_DATA"]


def shutdown_runtime(data_root):
    """Stop only the isolated Runtime identified by its authenticated handoff."""
    ready_path = os.path.join(data_root, "ipc", "ready.json")
    try:
        ready = json.loads(open(ready_path, encoding="utf-8").read())
        endpoint = ready["socket_path"]
        token = ready["token"]
    except (FileNotFoundError, KeyError, json.JSONDecodeError, OSError):
        return
    client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    client.settimeout(2.0)
    received = b""

    def exchange(payload):
        nonlocal received
        client.sendall(json.dumps(payload, separators=(",", ":")).encode() + b"\n")
        while b"\n" not in received:
            chunk = client.recv(65536)
            if not chunk:
                raise RuntimeError("Runtime closed before cleanup response")
            received += chunk
            if len(received) > 1024 * 1024:
                raise RuntimeError("Runtime cleanup response exceeded limit")
        line, received = received.split(b"\n", 1)
        return json.loads(line)

    try:
        client.connect(endpoint)
        authenticated = exchange(
            {"version": 1, "token": token, "method": "authenticate", "payload": None}
        )
        if not authenticated.get("ok"):
            return
        stopped = exchange(
            {"version": 1, "token": None, "method": "runtime_shutdown", "payload": None}
        )
        if not stopped.get("ok"):
            raise RuntimeError("Runtime rejected isolated cleanup")
    finally:
        client.close()
    deadline = time.monotonic() + 3.0
    while os.path.exists(ready_path) and time.monotonic() < deadline:
        time.sleep(0.02)
    if os.path.exists(ready_path):
        raise RuntimeError("isolated Runtime did not remove its ready handoff")


class LiveMcp:
    def __init__(self, child_env, label):
        self.label = label
        self.next_id = 1
        self.selector = selectors.DefaultSelector()
        self.process = None
        try:
            self.process = subprocess.Popen(
                [mcp, "serve", "--stdio"],
                env=child_env,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1,
            )
            self.selector.register(self.process.stdout, selectors.EVENT_READ)
        except BaseException:
            self.selector.close()
            if self.process is not None and self.process.poll() is None:
                self.process.kill()
                self.process.wait(timeout=5)
            raise

    def request(self, method, params=None, timeout=3.0):
        identifier = self.next_id
        self.next_id += 1
        payload = {"jsonrpc": "2.0", "id": identifier, "method": method}
        if params is not None:
            payload["params"] = params
        self.process.stdin.write(json.dumps(payload, separators=(",", ":")) + "\n")
        self.process.stdin.flush()
        events = self.selector.select(timeout)
        if not events:
            raise RuntimeError(f"{self.label} timed out waiting for {method}")
        response = json.loads(self.process.stdout.readline())
        if response.get("id") != identifier:
            raise RuntimeError(f"{self.label} returned an unexpected response id")
        return response

    def initialize(self):
        response = self.request(
            "initialize",
            {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": self.label, "version": "1"},
            },
        )
        if response.get("error") is not None:
            raise RuntimeError(f"{self.label} initialize failed: {response}")

    def tool(self, name, arguments=None, timeout=3.0):
        response = self.request(
            "tools/call",
            {"name": name, "arguments": arguments or {}},
            timeout=timeout,
        )
        result = response.get("result", {})
        if response.get("error") is not None or result.get("isError"):
            return None
        return result.get("structuredContent")

    def close(self):
        forced = False
        try:
            if self.process.poll() is None:
                try:
                    self.process.stdin.close()
                except BrokenPipeError:
                    pass
                try:
                    self.process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    forced = True
                    self.process.kill()
                    try:
                        self.process.wait(timeout=5)
                    except subprocess.TimeoutExpired as error:
                        raise RuntimeError(
                            f"{self.label} remained alive after targeted kill"
                        ) from error
            if forced:
                raise RuntimeError(f"{self.label} required targeted kill after stdio EOF")
            if self.process.returncode != 0:
                detail = self.process.stderr.read()[:256]
                raise RuntimeError(f"{self.label} exited unexpectedly: {detail!r}")
        finally:
            self.selector.close()


def close_clients(clients):
    errors = []
    for client in reversed(clients):
        try:
            client.close()
        except BaseException as error:
            errors.append(error)
    return errors


def wait_for_ready_document(data_root, timeout=8.0):
    ready_path = os.path.join(data_root, "ipc", "ready.json")
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            ready_bytes = open(ready_path, "rb").read()
            ready = json.loads(ready_bytes)
            if (
                ready.get("status") == "ready"
                and isinstance(ready.get("socket_path"), str)
                and isinstance(ready.get("token"), str)
            ):
                return ready_bytes, ready
        except (FileNotFoundError, json.JSONDecodeError, OSError):
            pass
        time.sleep(0.02)
    raise RuntimeError("isolated Runtime did not publish a ready handoff")


def wait_for_live_runtime(client, timeout=8.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        capabilities = client.tool("capabilities_get")
        if isinstance(capabilities, dict) and capabilities.get("status") == "alpha-mcp004":
            return capabilities
        time.sleep(0.05)
    raise RuntimeError(f"{client.label} did not reach the live Runtime")

env = os.environ.copy()
env.update(
    {
        "FORGECAD_RUNTIME_COMMAND": missing_runtime,
        "FORGECAD_RUNTIME_DATA_DIR": runtime_data,
        "FORGECAD_MCP_ENABLE_MCP004_WRITES": "1",
    }
)
for key in (
    "FORGECAD_RUNTIME_SOCKET",
    "FORGECAD_RUNTIME_TOKEN",
    "FORGECAD_RUNTIME_READY_FILE",
    "FORGECAD_RUNTIME_STATUS_FILE",
):
    env.pop(key, None)

requests = [
    {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "mcp004-local-regression", "version": "1"},
        },
    },
    {"jsonrpc": "2.0", "method": "notifications/initialized"},
    {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
    {
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {"name": "runtime_status", "arguments": {}},
    },
    {
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {"name": "project_list", "arguments": {}},
    },
    {
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        # Keep this request schema-valid so the lifecycle regression reaches
        # the missing Runtime path.  Malformed tool envelopes are separately
        # covered by MCP's fail-closed input-validation tests.
        "params": {
            "name": "candidate_prepare",
            "arguments": {
                "project_id": "project-mcp004",
                "request": {"typed": "diagnostic", "label": "lifecycle"},
            },
        },
    },
    {
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/call",
        "params": {"name": "doctor", "arguments": {}},
    },
]

def run_requests(child_env, child_requests):
    process = subprocess.Popen(
        [mcp, "serve", "--stdio"],
        env=child_env,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    payload = "\n".join(
        json.dumps(request, separators=(",", ":")) for request in child_requests
    ) + "\n"
    process.stdin.write(payload)
    process.stdin.close()
    stdout = process.stdout.read()
    stderr = process.stderr.read()
    return_code = process.wait(timeout=10)
    if return_code != 0:
        raise SystemExit(
            f"MCP exited unexpectedly: {return_code}; stderr={stderr[:512]!r}"
        )
    return [json.loads(line) for line in stdout.splitlines() if line.strip()]


def run_requests_stream(child_env, child_requests, delays):
    process = subprocess.Popen(
        [mcp, "serve", "--stdio"],
        env=child_env,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,
    )
    responses = []
    for index, request in enumerate(child_requests):
        process.stdin.write(json.dumps(request, separators=(",", ":")) + "\n")
        process.stdin.flush()
        if "id" in request:
            line = process.stdout.readline()
            if line:
                responses.append(json.loads(line))
        if index in delays:
            time.sleep(delays[index])
    process.stdin.close()
    process.stdout.read()
    stderr = process.stderr.read()
    return_code = process.wait(timeout=10)
    if return_code != 0:
        raise SystemExit(
            f"MCP exited unexpectedly: {return_code}; stderr={stderr[:512]!r}"
        )
    return responses


responses = run_requests(env, requests)

by_id = {item.get("id"): item for item in responses if "id" in item}
if by_id[1].get("error") is not None:
    raise SystemExit(f"initialize failed while Runtime was absent: {by_id[1]}")
if by_id[3]["result"]["structuredContent"]["state"] != "Degraded":
    raise SystemExit(f"Runtime status was not Degraded: {by_id[3]}")
project_list = by_id[4]["result"]["structuredContent"]
if project_list.get("code") != "RUNTIME_UNAVAILABLE" or project_list.get("retryable") is not True:
    raise SystemExit(f"missing Runtime did not produce retryable structured error: {by_id[4]}")
tools = by_id[2]["result"]["tools"]
candidate_prepare = next(tool for tool in tools if tool["name"] == "candidate_prepare")
if candidate_prepare["_meta"]["forgecad"]["requiresConfirmation"] is not True:
    raise SystemExit("MCP004 write approval metadata was lost")
write_error = by_id[5]["result"]["structuredContent"]
if write_error.get("code") != "RUNTIME_UNAVAILABLE" or write_error.get("retryable") is not True:
    raise SystemExit(f"failed Runtime write did not produce structured error: {by_id[5]}")
if by_id[6]["result"]["structuredContent"].get("state") != "Degraded":
    raise SystemExit(f"doctor did not report degraded Runtime: {by_id[6]}")
if not os.path.isdir(runtime_data):
    raise SystemExit("test data root was not allocated")

# Three MCP processes must recover a stale handoff, share one live Runtime,
# perform real IPC calls without an idle client starving the server, and keep
# the shared Runtime available when owner/passive adapter sessions close.
shared_data = os.path.join(os.environ["FORGECAD_TEST_ROOT"], "shared-runtime-data")
shared_env = env.copy()
shared_env.update(
    {
        "FORGECAD_RUNTIME_COMMAND": runtime_binary,
        "FORGECAD_RUNTIME_DATA_DIR": shared_data,
    }
)
os.makedirs(os.path.join(shared_data, "ipc"), exist_ok=True)
stale_socket = os.path.join(os.environ["FORGECAD_TEST_ROOT"], "stale.sock")
stale_listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
stale_listener.bind(stale_socket)
stale_listener.close()
with open(os.path.join(shared_data, "ipc", "ready.json"), "w", encoding="utf-8") as file:
    json.dump(
        {
            "schema_version": "ForgeCADRuntimeLauncherReady@1",
            "status": "ready",
            "socket_path": stale_socket,
            "token": "stale-token",
            "runtime_capabilities": {"build_cohort_sha256": "stale"},
        },
        file,
    )
with open(os.path.join(shared_data, "ipc", "status.json"), "w", encoding="utf-8") as file:
    json.dump(
        {
            "schema_version": "ForgeCADRuntimeSupervisorStatus@1",
            "state": "Ready",
            "last_exit_code": 1,
            "restart_count": 1,
        },
        file,
    )

clients = []
atexit.register(shutdown_runtime, shared_data)
try:
    for index in range(1, 4):
        client = LiveMcp(shared_env, f"mcp004-shared-{index}")
        clients.append(client)
        client.initialize()
    capabilities = wait_for_live_runtime(clients[0])
    if capabilities.get("mcp_write_tools_enabled") is not True:
        raise SystemExit("explicit write-tool opt-in was not reflected by capabilities")
    tools_response = clients[0].request("tools/list")
    tool_names = {
        tool["name"] for tool in tools_response.get("result", {}).get("tools", [])
    }
    if (
        len(tool_names) != 32
        or "project_create" not in tool_names
        or "geometry_program_hash" not in tool_names
        or "operator_catalog_get" not in tool_names
    ):
        raise SystemExit(f"shared MCP did not expose the expected 32 tools: {sorted(tool_names)}")

    ready_path = os.path.join(shared_data, "ipc", "ready.json")
    ready_bytes = open(ready_path, "rb").read()
    ready = json.loads(ready_bytes)
    if ready.get("socket_path") == stale_socket:
        raise SystemExit("stale ready handoff was not replaced")
    ready_hash = hashlib.sha256(ready_bytes).hexdigest()
    created = clients[0].tool(
        "project_create",
        {"name": "MCP004 shared Runtime fixture", "policy": {"profile": "test"}},
    )
    if not isinstance(created, dict) or not created.get("project_id"):
        raise SystemExit("shared Runtime project_create failed")

    rogue_idle = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        rogue_idle.connect(ready["socket_path"])
        with concurrent.futures.ThreadPoolExecutor(max_workers=3) as executor:
            project_lists = list(
                executor.map(lambda item: item.tool("project_list"), clients)
            )
    finally:
        rogue_idle.close()
    for projects in project_lists:
        if not isinstance(projects, list) or len(projects) != 1:
            raise SystemExit(f"a shared MCP did not read the same Runtime project: {projects!r}")

    malformed = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        malformed.connect(ready["socket_path"])
        malformed.sendall(b"{not-json}\n")
    finally:
        malformed.close()
    if len(clients[2].tool("project_list") or []) != 1:
        raise SystemExit("malformed local IPC peer stopped the shared Runtime")

    busy_probe = subprocess.run(
        [
            runtime_binary,
            "serve",
            "--database",
            os.path.join(shared_data, "runtime.sqlite"),
            "--cas-root",
            os.path.join(shared_data, "cas"),
            "--endpoint-dir",
            os.path.join(shared_data, "second-ipc"),
            "--ready-file",
            os.path.join(shared_data, "second-ready.json"),
        ],
        capture_output=True,
        text=True,
        timeout=10,
    )
    if busy_probe.returncode != 2 or "RUNTIME_BUSY" not in busy_probe.stderr:
        raise SystemExit(
            f"second Runtime did not fail with RUNTIME_BUSY: code={busy_probe.returncode}, stderr={busy_probe.stderr!r}"
        )

    clients[0].close()
    if hashlib.sha256(open(ready_path, "rb").read()).hexdigest() != ready_hash:
        raise SystemExit("closing the launcher MCP replaced or removed the live handoff")
    if len(clients[1].tool("project_list") or []) != 1:
        raise SystemExit("passive MCP lost Runtime after launcher MCP closed")
    clients[1].close()
    if hashlib.sha256(open(ready_path, "rb").read()).hexdigest() != ready_hash:
        raise SystemExit("closing a passive MCP replaced or removed the live handoff")
    if len(clients[2].tool("project_list") or []) != 1:
        raise SystemExit("last MCP lost Runtime after passive MCP closed")
finally:
    active_failure = sys.exc_info()[0] is not None
    cleanup_errors = close_clients(clients)
    try:
        shutdown_runtime(shared_data)
    except BaseException as error:
        cleanup_errors.append(error)
    if cleanup_errors and not active_failure:
        raise cleanup_errors[0]

# Regression for launcher election lifetime: the MCP that launched the first
# Runtime initializes once and then remains completely idle. After that Runtime
# is stopped through its authenticated local IPC, only the passive MCP is polled;
# it must acquire launcher election and restore real Runtime calls.
idle_owner_data = os.path.join(
    os.environ["FORGECAD_TEST_ROOT"], "idle-owner-runtime-data"
)
idle_owner_env = env.copy()
idle_owner_env.update(
    {
        "FORGECAD_RUNTIME_COMMAND": runtime_binary,
        "FORGECAD_RUNTIME_DATA_DIR": idle_owner_data,
    }
)
idle_clients = []
atexit.register(shutdown_runtime, idle_owner_data)
try:
    owner = LiveMcp(idle_owner_env, "mcp004-idle-owner")
    idle_clients.append(owner)
    owner.initialize()
    first_ready_bytes, _ = wait_for_ready_document(idle_owner_data)

    passive = LiveMcp(idle_owner_env, "mcp004-idle-passive")
    idle_clients.append(passive)
    passive.initialize()

    # Do not issue another request to owner after initialize. This authenticated
    # shutdown simulates the shared Runtime disappearing while its launcher MCP
    # remains alive but idle.
    shutdown_runtime(idle_owner_data)
    if owner.process.poll() is not None:
        raise SystemExit("idle launcher MCP exited with its shared Runtime")

    recovered = wait_for_live_runtime(passive, timeout=10.0)
    if recovered.get("mcp_write_tools_enabled") is not True:
        raise SystemExit("passive takeover lost explicit write-tool configuration")
    second_ready_bytes, _ = wait_for_ready_document(idle_owner_data)
    if hashlib.sha256(second_ready_bytes).digest() == hashlib.sha256(
        first_ready_bytes
    ).digest():
        raise SystemExit("passive MCP did not publish a fresh Runtime handoff")
    recovered_projects = passive.tool("project_list")
    if not isinstance(recovered_projects, list):
        raise SystemExit("passive MCP did not restore real project_list IPC")
finally:
    active_failure = sys.exc_info()[0] is not None
    cleanup_errors = close_clients(idle_clients)
    try:
        shutdown_runtime(idle_owner_data)
    except BaseException as error:
        cleanup_errors.append(error)
    if cleanup_errors and not active_failure:
        raise cleanup_errors[0]

# A real Runtime is launched behind a test-only wrapper, becomes ready, and
# is then crashed. The MCP adapter must stay alive while its small supervisor
# performs its single bounded restart and settles on Degraded.
crash_wrapper = os.path.join(os.environ["FORGECAD_TEST_DATA"], "..", "runtime-crash-after-ready")
with open(crash_wrapper, "w", encoding="utf-8") as file:
    file.write(
        "#!/bin/sh\n"
        "set -eu\n"
        "ready_file=''\n"
        "previous=''\n"
        "for arg in \"$@\"; do\n"
        "  if [ \"$previous\" = '--ready-file' ]; then ready_file=\"$arg\"; fi\n"
        "  previous=\"$arg\"\n"
        "done\n"
        f"'{runtime_binary}' \"$@\" &\n"
        "runtime_pid=$!\n"
        "for attempt in $(seq 1 200); do\n"
        "  if [ -n \"$ready_file\" ] && [ -f \"$ready_file\" ]; then\n"
        "    kill \"$runtime_pid\" 2>/dev/null || true\n"
        "    wait \"$runtime_pid\" 2>/dev/null || true\n"
        "    exit 0\n"
        "  fi\n"
        "  if ! kill -0 \"$runtime_pid\" 2>/dev/null; then exit 1; fi\n"
        "  sleep 0.01\n"
        "done\n"
        "kill \"$runtime_pid\" 2>/dev/null || true\n"
        "wait \"$runtime_pid\" 2>/dev/null || true\n"
        "exit 1\n"
    )
os.chmod(crash_wrapper, 0o700)

crash_env = env.copy()
crash_env.update(
    {
        "FORGECAD_RUNTIME_COMMAND": crash_wrapper,
        "FORGECAD_RUNTIME_DATA_DIR": os.path.join(
            os.environ["FORGECAD_TEST_ROOT"], "crash-runtime-data"
        ),
    }
)
crash_client = LiveMcp(crash_env, "mcp004-crash-regression")
crash_data = crash_env["FORGECAD_RUNTIME_DATA_DIR"]
crash_status = None
crash_projects = None
try:
    crash_client.initialize()
    # Runtime startup and the wrapper's ready-file observation are asynchronous.
    # Poll the bounded status surface rather than sampling once at a fixed delay;
    # this keeps the crash/restart assertion deterministic under a cold build or
    # a busy CI host while still enforcing a short test deadline.
    deadline = time.monotonic() + 6.0
    while time.monotonic() < deadline:
        crash_status = crash_client.tool("runtime_status", timeout=1.0)
        if isinstance(crash_status, dict) and crash_status.get("restart_count") == 1:
            break
        time.sleep(0.1)
    if not isinstance(crash_status, dict) or crash_status.get("restart_count") != 1:
        raise SystemExit(
            f"Runtime crash did not trigger one bounded restart before deadline: {crash_status}"
        )
    if crash_status.get("state") not in {"Restarting", "Degraded", "Ready"}:
        raise SystemExit(f"Runtime crash settled on an invalid state: {crash_status}")
    crash_projects_response = crash_client.request(
        "tools/call",
        {"name": "project_list", "arguments": {}},
        timeout=1.0,
    )
    crash_projects = crash_projects_response.get("result", {}).get(
        "structuredContent"
    )
finally:
    close_errors = []
    try:
        crash_client.close()
    except BaseException as error:
        close_errors.append(error)
    try:
        shutdown_runtime(crash_data)
    except BaseException as error:
        close_errors.append(error)
    if close_errors:
        raise close_errors[0]
if not isinstance(crash_projects, dict) or crash_projects.get("code") != "RUNTIME_UNAVAILABLE":
    raise SystemExit(f"stdio did not remain alive after Runtime crash: {crash_projects}")
print("MCP004 local lifecycle regressions PASS: stale recovery, three MCP sessions, rogue IPC isolation, idle-owner passive takeover, shared Runtime lifetime, missing Runtime, child crash, bounded restart, retryable calls, write approval")
PY
