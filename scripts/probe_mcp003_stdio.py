#!/usr/bin/env python3
"""Run the bounded MCP003 stdio smoke without changing Codex configuration.

This is a transport-level probe for the packaged ``forgecad-mcp`` process.  It
does not claim that a Codex Desktop/CLI/IDE host connected successfully; those
rows remain owned by the host matrix and must be run in the user-facing host.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess


PROTOCOL_VERSION = "2025-11-25"
CODEX_COMPAT_PROTOCOL_VERSION = "2025-06-18"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--command",
        default=os.environ.get("FORGECAD_MCP_COMMAND", "forgecad-mcp"),
        help="forgecad-mcp executable; defaults to FORGECAD_MCP_COMMAND or PATH",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=10.0,
        help="maximum seconds for each process probe",
    )
    parser.add_argument(
        "command_args",
        nargs="*",
        help="optional executable arguments; defaults to serve --stdio",
    )
    return parser.parse_args()


def run_probe(command: list[str], requests: list[dict], timeout: float) -> list[dict]:
    process = subprocess.Popen(
        command,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )
    payload = "".join(json.dumps(request, separators=(",", ":")) + "\n" for request in requests)
    try:
        stdout, stderr = process.communicate(payload, timeout=timeout)
    except subprocess.TimeoutExpired as error:
        process.kill()
        process.communicate()
        raise SystemExit(f"MCP003 stdio probe timed out: {command[0]}") from error
    if process.returncode != 0:
        detail = " ".join(stderr.split())[:512]
        raise SystemExit(f"MCP003 stdio process exited {process.returncode}: {detail}")
    responses: list[dict] = []
    for line in stdout.splitlines():
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as error:
            raise SystemExit("MCP003 stdio emitted a non-JSON response") from error
        if not isinstance(value, dict):
            raise SystemExit("MCP003 stdio emitted a non-object response")
        responses.append(value)
    return responses


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def main() -> int:
    args = parse_args()
    command = [args.command, *(args.command_args or ["serve", "--stdio"])]
    responses = run_probe(
        command,
        [
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {"name": "forgecad-mcp003-probe", "version": "1"},
                },
            },
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
            {"jsonrpc": "2.0", "id": 3, "method": "resources/list"},
            {
                "jsonrpc": "2.0",
                "id": 4,
                "method": "resources/read",
                "params": {"uri": "forgecad://capabilities"},
            },
        ],
        args.timeout,
    )
    require(len(responses) == 4, f"expected 4 responses, got {len(responses)}")
    require(responses[0].get("result", {}).get("protocolVersion") == PROTOCOL_VERSION, "initialize protocol mismatch")
    tools = responses[1].get("result", {}).get("tools", [])
    require(len(tools) == 17, f"expected 17 tools, got {len(tools)}")
    require(
        all(tool.get("annotations", {}).get("readOnlyHint") is True for tool in tools),
        "a discovered tool is not read-only",
    )
    resources = responses[2].get("result", {}).get("resources", [])
    require(any(item.get("uri") == "forgecad://capabilities" for item in resources), "capabilities resource missing")
    contents = responses[3].get("result", {}).get("contents", [])
    require(contents and contents[0].get("mimeType") == "application/json", "capabilities resource is not JSON")
    json.loads(contents[0].get("text", "{}"))

    compatibility = run_probe(
        command,
        [
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": CODEX_COMPAT_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {"name": "codex-host-probe", "version": "0.147.0"},
                },
            },
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
        ],
        args.timeout,
    )
    require(
        compatibility
        and compatibility[0].get("result", {}).get("protocolVersion")
        == CODEX_COMPAT_PROTOCOL_VERSION,
        "Codex 2025-06-18 compatibility negotiation failed",
    )
    require(
        len(compatibility[1].get("result", {}).get("tools", [])) == 17,
        "Codex compatibility tools/list did not expose 17 tools",
    )

    mismatch = run_probe(
        command,
        [
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "0.0.0",
                    "capabilities": {},
                    "clientInfo": {"name": "forgecad-mcp003-probe", "version": "1"},
                },
            }
        ],
        args.timeout,
    )
    require(
        mismatch and mismatch[0].get("error", {}).get("data", {}).get("code") == "CONTRACT_VERSION_UNSUPPORTED",
        "incompatible protocol did not fail closed",
    )
    modern = run_probe(
        command,
        [{"jsonrpc": "2.0", "id": 1, "method": "server/discover", "params": {}}],
        args.timeout,
    )
    require(
        modern
        and modern[0].get("error", {}).get("data", {}).get("modern_protocol")
        == "2026-07-28",
        "modern 2026-07-28 discovery did not fail closed explicitly",
    )
    print(
        json.dumps(
            {
                "status": "PASS",
                "protocol_version": PROTOCOL_VERSION,
                "codex_compat_protocol_version": CODEX_COMPAT_PROTOCOL_VERSION,
                "responses": len(responses),
                "tools": len(tools),
                "resources": len(resources),
                "version_mismatch": "CONTRACT_VERSION_UNSUPPORTED",
                "modern_protocol": "EXPLICITLY_UNSUPPORTED",
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
