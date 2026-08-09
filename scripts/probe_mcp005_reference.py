#!/usr/bin/env python3
"""Probe MCP005 reference admission through the real MCP adapter.

Without ``--execute`` this command is intentionally NOT_RUN. With ``--execute``
it starts one authenticated Runtime, asks Codex CLI to create a project and
submit a user-authorized ``codex_local_file`` reference, and records only the
source hash and opaque Runtime evidence. The original path and image bytes are
never written to the receipt.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any


WRITE_SEQUENCE = ("project_create", "reference_import")
READ_SEQUENCE = ("reference_get",)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--reference", required=True, help="user-authorized PNG/JPEG path")
    parser.add_argument("--runtime-command", default="forgecad-runtime")
    parser.add_argument("--mcp-command", default="forgecad-mcp")
    parser.add_argument("--codex-command", default="codex")
    parser.add_argument("--timeout", type=float, default=240.0)
    return parser.parse_args()


def event_items(stdout: str) -> list[dict[str, Any]]:
    items: list[dict[str, Any]] = []
    for line in stdout.splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        item = event.get("item")
        if isinstance(item, dict):
            items.append(item)
    return items


def mcp_calls(items: list[dict[str, Any]]) -> list[dict[str, Any]]:
    grouped: dict[str, dict[str, Any]] = {}
    order: list[str] = []
    for item in items:
        if item.get("type") != "mcp_tool_call":
            continue
        key = str(item.get("id") or f"call-{len(order)}")
        if key not in grouped:
            order.append(key)
            grouped[key] = {
                "server": item.get("server"),
                "tool": item.get("tool"),
                "status": item.get("status"),
            }
        elif item.get("status"):
            grouped[key]["status"] = item["status"]
    return [grouped[key] for key in order]


def unrelated_side_effects(items: list[dict[str, Any]]) -> bool:
    normalized = {str(item.get("type", "")).replace("_", "").lower() for item in items}
    return bool(normalized & {"commandexecution", "filechange", "mcpresourcewrite"})


def config_override(command: str) -> str:
    command_literal = json.dumps(command, ensure_ascii=False)
    return (
        "mcp_servers.forgecad={"
        f"command={command_literal},"
        'args=["serve","--stdio"],'
        'env_vars=["FORGECAD_RUNTIME_SOCKET","FORGECAD_RUNTIME_TOKEN","FORGECAD_MCP_ENABLE_MCP004_WRITES","FORGECAD_ATTACHMENT_ROOTS"],'
        "enabled=true,required=true,startup_timeout_sec=20,tool_timeout_sec=60,"
        'default_tools_approval_mode="writes"}'
    )


def prompt(reference_path: str) -> str:
    return f"""Use only the forgecad MCP server. Do not use shell, browser, filesystem tools, other MCP servers, images, or arbitrary code.

Create a project with project_create, name=\"MCP005 reference acceptance\", policy={{\"profile\":\"mvp\"}}. Save its project_id.
Then call reference_import exactly once with:
project_id=<saved project_id>
source={{\"kind\":\"codex_local_file\",\"path\":{json.dumps(reference_path, ensure_ascii=False)}}}
authorization={{\"user_authorized\":true,\"declaration\":\"The user supplied and authorized this reference for the local ForgeCAD MVP.\"}}
Do not alter the path or invent a different source. Save the returned reference_id and call reference_get exactly once with it.
Stop after reference_get. Report only whether the three MCP calls completed and the returned MIME, dimensions, reference_id and object_sha256; do not claim geometry, rendering, similarity or a finished 3D model.
"""


def not_run(reason: str, source_sha256: str, size: int) -> dict[str, Any]:
    return {
        "status": "NOT_RUN",
        "reason": reason,
        "scope": "MCP005 reference admission",
        "source_sha256": source_sha256,
        "source_size_bytes": size,
        "reference_path_recorded": False,
        "image_bytes_recorded": False,
        "geometry": "NOT_RUN",
        "render": "NOT_RUN",
    }


def main() -> int:
    args = parse_args()
    source = Path(args.reference)
    if not source.is_file() or source.is_symlink():
        print(json.dumps({"status": "BLOCKED", "reason": "reference is not a regular file"}))
        return 3
    source_bytes = source.read_bytes()
    source_sha256 = hashlib.sha256(source_bytes).hexdigest()
    if not args.execute:
        print(json.dumps(not_run("Pass --execute to run the local Runtime and Codex CLI.", source_sha256, len(source_bytes))))
        return 2

    environment = os.environ.copy()
    environment.pop("CODEX_MCP_PROTOCOL_VERSION", None)
    environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
    environment["FORGECAD_ATTACHMENT_ROOTS"] = str(source.parent)

    # macOS AF_UNIX paths are short; keep the temporary root under /tmp so the
    # authenticated Runtime socket remains below the platform path limit.
    with tempfile.TemporaryDirectory(dir="/tmp", prefix="fc5-") as temporary:
        root = Path(temporary)
        ready = root / "ready.json"
        runtime = subprocess.Popen(
            [
                args.runtime_command,
                "serve",
                "--database",
                str(root / "runtime.sqlite"),
                "--cas-root",
                str(root / "cas"),
                "--endpoint-dir",
                str(root / "ipc"),
                "--ready-file",
                str(ready),
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
            env=environment,
        )
        try:
            deadline = time.monotonic() + 30
            while not ready.exists() and time.monotonic() < deadline:
                if runtime.poll() is not None:
                    break
                time.sleep(0.05)
            if not ready.exists():
                print(json.dumps(not_run("Runtime did not publish a ready handoff.", source_sha256, len(source_bytes))))
                return 3
            handoff = json.loads(ready.read_text(encoding="utf-8"))
            environment.update(
                {
                    "FORGECAD_RUNTIME_SOCKET": str(handoff["socket_path"]),
                    "FORGECAD_RUNTIME_TOKEN": str(handoff["token"]),
                }
            )
            with tempfile.TemporaryDirectory(dir="/tmp", prefix="fc5-codex-") as workspace:
                completed = subprocess.run(
                    [
                        args.codex_command,
                        "exec",
                        "--ephemeral",
                        "--ignore-user-config",
                        "--json",
                        "--color",
                        "never",
                        "--approve-for-me",
                        "--skip-git-repo-check",
                        "-C",
                        workspace,
                        "-c",
                        config_override(args.mcp_command),
                        "--image",
                        str(source),
                    ],
                    input=prompt(str(source)) + "\n",
                    env=environment,
                    text=True,
                    capture_output=True,
                    timeout=args.timeout,
                    check=False,
                )
            calls = mcp_calls(event_items(completed.stdout))
            tools = [call.get("tool") for call in calls if call.get("server") == "forgecad"]
            statuses_ok = all(call.get("status") == "completed" for call in calls)
            status = "PASS" if completed.returncode == 0 and tools == [*WRITE_SEQUENCE, *READ_SEQUENCE] and statuses_ok and not unrelated_side_effects(event_items(completed.stdout)) else "BLOCKED"
            receipt = {
                "status": status,
                "mode": "codex-cli-mcp005-reference",
                "codex_exit_code": completed.returncode,
                "mcp_tool_calls": calls,
                "expected_sequence": [*WRITE_SEQUENCE, *READ_SEQUENCE],
                "source_sha256": source_sha256,
                "source_size_bytes": len(source_bytes),
                "reference_path_recorded": False,
                "image_bytes_recorded": False,
                "geometry": "NOT_RUN",
                "render": "NOT_RUN",
                "reason": None if status == "PASS" else "Codex did not complete the exact MCP005 admission sequence.",
            }
            print(json.dumps(receipt, ensure_ascii=False, separators=(",", ":")))
            return 0 if status == "PASS" else 3
        finally:
            if runtime.poll() is None:
                runtime.terminate()
                try:
                    runtime.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    runtime.kill()
                    runtime.wait(timeout=5)


if __name__ == "__main__":
    raise SystemExit(main())
