#!/usr/bin/env python3
"""Safely stop the authenticated local ForgeCAD Runtime in development.

This is an operations helper, not a product MCP tool. It never signals an
arbitrary PID and never removes Runtime data. Without ``--confirm`` it only
validates the current ready handoff and prints a sanitized status.
"""

from __future__ import annotations

import argparse
import json
import socket
import sys
import time
from pathlib import Path
from typing import Any


MAX_MESSAGE_BYTES = 1024 * 1024
SOCKET_TIMEOUT_SECONDS = 2.0
SHUTDOWN_WAIT_SECONDS = 3.0


def default_data_root() -> Path:
    return (
        Path.home()
        / "Library"
        / "Application Support"
        / "ForgeCAD Runtime"
        / "runtime-data"
    )


def read_ready(path: Path) -> tuple[str, str]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise RuntimeError("RUNTIME_NOT_READY: ready handoff is absent") from error
    except (OSError, json.JSONDecodeError) as error:
        raise RuntimeError("RUNTIME_HANDOFF_INVALID: ready handoff is unreadable") from error
    if not isinstance(value, dict) or value.get("status") != "ready":
        raise RuntimeError("RUNTIME_NOT_READY: handoff is not ready")
    endpoint = value.get("socket_path")
    token = value.get("token")
    if not isinstance(endpoint, str) or not endpoint:
        raise RuntimeError("RUNTIME_HANDOFF_INVALID: socket path is missing")
    if not isinstance(token, str) or len(token) < 16:
        raise RuntimeError("RUNTIME_HANDOFF_INVALID: authentication token is missing")
    return endpoint, token


def exchange(connection: socket.socket, payload: dict[str, Any]) -> dict[str, Any]:
    connection.sendall(
        json.dumps(payload, separators=(",", ":"), ensure_ascii=True).encode("utf-8")
        + b"\n"
    )
    buffered = bytearray()
    while b"\n" not in buffered:
        chunk = connection.recv(65536)
        if not chunk:
            raise RuntimeError("RUNTIME_IPC_CLOSED: Runtime closed before response")
        buffered.extend(chunk)
        if len(buffered) > MAX_MESSAGE_BYTES:
            raise RuntimeError("RUNTIME_IPC_LIMIT: response exceeded 1 MiB")
    line = bytes(buffered).split(b"\n", 1)[0]
    try:
        value = json.loads(line)
    except json.JSONDecodeError as error:
        raise RuntimeError("RUNTIME_IPC_INVALID: response was not JSON") from error
    if not isinstance(value, dict):
        raise RuntimeError("RUNTIME_IPC_INVALID: response was not an object")
    return value


def authenticated_shutdown(endpoint: str, token: str) -> None:
    connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    connection.settimeout(SOCKET_TIMEOUT_SECONDS)
    try:
        connection.connect(endpoint)
        authenticated = exchange(
            connection,
            {"version": 1, "token": token, "method": "authenticate", "payload": None},
        )
        if authenticated.get("ok") is not True:
            raise RuntimeError("RUNTIME_AUTH_FAILED: authenticated shutdown was rejected")
        stopped = exchange(
            connection,
            {"version": 1, "token": None, "method": "runtime_shutdown", "payload": None},
        )
        if stopped.get("ok") is not True:
            raise RuntimeError("RUNTIME_SHUTDOWN_REJECTED: Runtime refused shutdown")
    finally:
        connection.close()


def wait_for_handoff_removal(path: Path) -> None:
    deadline = time.monotonic() + SHUTDOWN_WAIT_SECONDS
    while path.exists() and time.monotonic() < deadline:
        time.sleep(0.02)
    if path.exists():
        raise RuntimeError("RUNTIME_SHUTDOWN_TIMEOUT: ready handoff still exists")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Inspect or explicitly stop the authenticated local ForgeCAD Runtime."
    )
    parser.add_argument(
        "--data-root",
        type=Path,
        default=default_data_root(),
        help="Runtime data root (default: user-level ForgeCAD Runtime data root)",
    )
    parser.add_argument(
        "--confirm",
        action="store_true",
        help="Send authenticated runtime_shutdown; without this flag the command is read-only",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    ready_path = args.data_root / "ipc" / "ready.json"
    try:
        endpoint, token = read_ready(ready_path)
        # Never echo endpoint, token, or Runtime payload into a transcript.
        if not args.confirm:
            print("RUNTIME_READY_FOR_EXPLICIT_SHUTDOWN")
            return 0
        authenticated_shutdown(endpoint, token)
        wait_for_handoff_removal(ready_path)
        print("RUNTIME_SHUTDOWN_OK")
        return 0
    except RuntimeError as error:
        print(str(error), file=sys.stderr)
        return 2
    except (OSError, ValueError) as error:
        print(f"RUNTIME_SHUTDOWN_ERROR: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
