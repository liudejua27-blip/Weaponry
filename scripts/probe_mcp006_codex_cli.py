#!/usr/bin/env python3
"""Probe the MCP006 Skill registry through the real Codex CLI.

The probe is read-only. It records tool names and the registry hash/count, not
prompts, local paths or arbitrary process output.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any


SEQUENCE = ("capabilities_get", "skill_list", "skill_get")


def args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--runtime-command", default="forgecad-runtime")
    parser.add_argument("--mcp-command", default="forgecad-mcp")
    parser.add_argument("--codex-command", default="codex")
    parser.add_argument("--timeout", type=float, default=180.0)
    return parser.parse_args()


def calls(stdout: str) -> list[dict[str, Any]]:
    grouped: dict[str, dict[str, Any]] = {}
    order: list[str] = []
    for line in stdout.splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        item = event.get("item")
        if not isinstance(item, dict) or item.get("type") != "mcp_tool_call":
            continue
        key = str(item.get("id") or f"call-{len(order)}")
        if key not in grouped:
            order.append(key)
            grouped[key] = {"server": item.get("server"), "tool": item.get("tool"), "status": item.get("status")}
        elif item.get("status"):
            grouped[key]["status"] = item["status"]
    return [grouped[key] for key in order]


def config(command: str) -> str:
    return (
        f"mcp_servers.forgecad={{command={json.dumps(command)},args=[\"serve\",\"--stdio\"],enabled=true,"
        'env_vars=["FORGECAD_RUNTIME_SOCKET","FORGECAD_RUNTIME_TOKEN"],'
        "required=true,startup_timeout_sec=20,tool_timeout_sec=60,"
        'default_tools_approval_mode="writes"}'
    )


def prompt() -> str:
    return """Use only the forgecad MCP server. Do not use shell, browser, filesystem, other MCP servers, or arbitrary code. Call capabilities_get, then skill_list, then skill_get with skill_id=reference-intake and version=0.1.0. Stop after those three read-only calls. Report only the supports_skill_registry value, number of skills, and returned canonical hash for reference-intake. Do not claim geometry or a finished 3D model."""


def main() -> int:
    options = args()
    if not options.execute:
        print(json.dumps({"status": "NOT_RUN", "reason": "Pass --execute to run the real Codex CLI Skill registry probe."}))
        return 2
    environment = os.environ.copy()
    for key in ("FORGECAD_MCP_ENABLE_MCP004_WRITES", "FORGECAD_RUNTIME_DATA_DIR"):
        environment.pop(key, None)
    with tempfile.TemporaryDirectory(dir="/tmp", prefix="fc6-") as temporary:
        root = Path(temporary)
        ready = root / "ready.json"
        runtime = subprocess.Popen([
            options.runtime_command, "serve", "--database", str(root / "runtime.sqlite"),
            "--cas-root", str(root / "cas"), "--endpoint-dir", str(root / "ipc"),
            "--ready-file", str(ready),
        ], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True, env=environment)
        try:
            deadline = time.monotonic() + 30
            while not ready.exists() and time.monotonic() < deadline:
                if runtime.poll() is not None:
                    break
                time.sleep(0.05)
            if not ready.exists():
                print(json.dumps({"status": "BLOCKED", "reason": "Runtime did not publish a ready handoff."}))
                return 3
            handoff = json.loads(ready.read_text(encoding="utf-8"))
            environment.update({"FORGECAD_RUNTIME_SOCKET": handoff["socket_path"], "FORGECAD_RUNTIME_TOKEN": handoff["token"]})
            with tempfile.TemporaryDirectory(dir="/tmp", prefix="fc6-codex-") as workspace:
                completed = subprocess.run([
                    options.codex_command, "exec", "--ephemeral", "--ignore-user-config", "--json",
                    "--color", "never", "--approve-for-me", "--skip-git-repo-check", "-C", workspace,
                    "-c", config(options.mcp_command), prompt(),
                ], env=environment, text=True, capture_output=True, timeout=options.timeout, check=False)
            tool_calls = calls(completed.stdout)
            tools = [call.get("tool") for call in tool_calls if call.get("server") == "forgecad"]
            completed_ok = all(call.get("status") == "completed" for call in tool_calls)
            status = "PASS" if completed.returncode == 0 and tools == list(SEQUENCE) and completed_ok else "BLOCKED"
            receipt = {
                "status": status,
                "mode": "codex-cli-mcp006-skill-registry",
                "codex_exit_code": completed.returncode,
                "mcp_tool_calls": tool_calls,
                "expected_sequence": list(SEQUENCE),
                "reference_path_recorded": False,
                "prompt_recorded": False,
                "geometry": "NOT_RUN",
                "render": "NOT_RUN",
                "reason": None if status == "PASS" else "Codex did not complete the exact read-only Skill sequence.",
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
