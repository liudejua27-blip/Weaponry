#!/usr/bin/env python3
"""Run an explicit, read-only Codex CLI MCP003 host probe.

The probe is intentionally opt-in and is not part of release:mcp003. It uses
the caller's existing Codex authentication, but never copies credentials,
loads the user's config, writes a project, or persists a Codex session.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import tempfile
from pathlib import Path
from typing import Any


READ_ONLY_TOOLS = ("capabilities_get", "selection_get")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--mode",
        choices=("read-only", "version-mismatch"),
        default="read-only",
        help="Probe a normal read-only turn or explicit 2026-07-28 rejection.",
    )
    parser.add_argument(
        "--execute",
        action="store_true",
        help="Actually invoke the authenticated Codex CLI. Without this flag the probe is a no-op.",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=180.0,
        help="Maximum Codex CLI runtime in seconds (default: 180).",
    )
    return parser.parse_args()


def config_override(command: str) -> str:
    command_literal = json.dumps(command, ensure_ascii=False)
    return (
        "mcp_servers.forgecad={"
        f"command={command_literal},"
        'args=["serve","--stdio"],'
        "required=true,"
        "startup_timeout_sec=10,"
        "tool_timeout_sec=10,"
        'default_tools_approval_mode="writes"'
        "}"
    )


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


def receipt_for(args: argparse.Namespace, completed: subprocess.CompletedProcess[str]) -> dict[str, Any]:
    items = event_items(completed.stdout)
    calls = mcp_calls(items)
    normalized_types = {str(item.get("type", "")).replace("_", "").lower() for item in items}
    side_effects = any(
        marker in normalized_types
        for marker in ("commandexecution", "filechange", "mcpresourcewrite", "mcptoolcallwrite")
    )

    if args.mode == "read-only":
        observed = [(call.get("server"), call.get("tool"), call.get("status")) for call in calls]
        expected = {("forgecad", tool, "completed") for tool in READ_ONLY_TOOLS}
        # Codex may issue independent read-only calls concurrently, so their
        # completion order is not part of the MCP contract.  Require exactly
        # the two expected calls, with no duplicate or extra call.
        status = "PASS" if completed.returncode == 0 and set(observed) == expected and len(observed) == len(expected) and not side_effects else "BLOCKED"
        return {
            "status": status,
            "mode": "read-only",
            "codex_exit_code": completed.returncode,
            "mcp_tool_calls": calls,
            "side_effects": side_effects,
            "reason": None if status == "PASS" else "Codex did not complete exactly the two expected read-only ForgeCAD calls.",
        }

    final_text = " ".join(
        str(item.get("text", ""))
        for item in items
        if item.get("type") == "agent_message"
    ).lower()
    # The host may quote the server error verbatim ("unsupported") or
    # paraphrase the same fail-closed result ("cannot be used or initialized
    # under that constraint").  In both cases the observable contract is no
    # tool call and no retry/downgrade; the raw server error is covered by the
    # protocol probe and Rust tests.
    rejection_markers = ("unsupported", "cannot be used or initialized")
    expected_rejection = any(marker in final_text for marker in rejection_markers) and not calls and not side_effects
    status = "PASS" if completed.returncode == 0 and expected_rejection else "BLOCKED"
    return {
        "status": status,
        "mode": "version-mismatch",
        "requested_protocol": "2026-07-28",
        "codex_exit_code": completed.returncode,
        "mcp_tool_calls": calls,
        "side_effects": side_effects,
        "reason": None if status == "PASS" else "Codex did not report a fail-closed unsupported-protocol result without fallback.",
    }


def main() -> int:
    args = parse_args()
    if not args.execute:
        print(json.dumps({"status": "NOT_RUN", "reason": "Pass --execute to invoke Codex; no network or process was started."}))
        return 2

    command = os.environ.get("FORGECAD_MCP_COMMAND", "forgecad-mcp")
    environment = os.environ.copy()
    if args.mode == "version-mismatch":
        environment["CODEX_MCP_PROTOCOL_VERSION"] = "2026-07-28"

    with tempfile.TemporaryDirectory(prefix="forgecad-mcp003-codex-") as workspace:
        codex_command = [
            "codex",
            "exec",
            "--ephemeral",
            "--ignore-user-config",
            "--json",
            "--color",
            "never",
            "--sandbox",
            "read-only",
            "--skip-git-repo-check",
            "-C",
            workspace,
            "-c",
            config_override(command),
        ]
        prompt = (
            "Use only the forgecad MCP server. Call exactly the read-only tools "
            "capabilities_get and selection_get. Do not call any other tools, "
            "do not write files, and return a short JSON summary."
            if args.mode == "read-only"
            else "Use forgecad only if its MCP server initializes successfully. Do not call any tools. "
            "If the negotiated MCP protocol is unsupported, report that exact fact and do not retry or downgrade."
        )
        try:
            completed = subprocess.run(
                [*codex_command, prompt],
                env=environment,
                text=True,
                capture_output=True,
                timeout=args.timeout,
                check=False,
            )
        except subprocess.TimeoutExpired:
            print(json.dumps({"status": "BLOCKED", "mode": args.mode, "reason": "Codex CLI probe timed out."}))
            return 3

    receipt = receipt_for(args, completed)
    print(json.dumps(receipt, ensure_ascii=False, separators=(",", ":")))
    return 0 if receipt["status"] == "PASS" else 3


if __name__ == "__main__":
    raise SystemExit(main())
