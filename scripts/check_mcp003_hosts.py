#!/usr/bin/env python3
"""Check MCP003 Codex host baselines and honest E2E status accounting."""

from __future__ import annotations

import json
import os
import ast
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONFIGS = [ROOT / "config/codex/desktop.toml", ROOT / "config/codex/cli.toml", ROOT / "config/codex/ide.toml"]
HOSTS = ("codex_desktop", "codex_cli", "codex_ide")
STATUSES = {"PASS", "NOT_RUN", "BLOCKED"}


def read_baseline(path: Path) -> dict:
    values: dict[str, object] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.split("#", 1)[0].strip()
        if not line or line.startswith("[") or "=" not in line:
            continue
        key, value = (part.strip() for part in line.split("=", 1))
        if value == "true":
            values[key] = True
            continue
        if value == "false":
            values[key] = False
            continue
        try:
            values[key] = ast.literal_eval(value)
        except (ValueError, SyntaxError):
            values[key] = value.strip('"')
    return values


def main() -> int:
    for path in CONFIGS:
        if not path.exists():
            raise SystemExit(f"Codex config baseline missing: {path.relative_to(ROOT)}")
        text = path.read_text(encoding="utf-8")
        if "[mcp_servers.forgecad]" not in text:
            raise SystemExit(f"forgecad server table missing: {path.relative_to(ROOT)}")
        server = read_baseline(path)
        if server.get("command") != "forgecad-mcp":
            raise SystemExit("MCP003 config must use the packaged forgecad-mcp command name")
        if server.get("args") != ["serve", "--stdio"]:
            raise SystemExit("MCP003 config must start forgecad-mcp in stdio mode")
        if server.get("required") is not True or server.get("default_tools_approval_mode") != "writes":
            raise SystemExit("Codex baselines must require the server and approve writes explicitly")
        if set(server.get("env_vars", [])) != {"FORGECAD_RUNTIME_SOCKET", "FORGECAD_RUNTIME_TOKEN"}:
            raise SystemExit("Codex baselines must forward only the Runtime IPC variable names")
        if os.path.isabs(str(server.get("command"))):
            raise SystemExit("absolute command path in Codex baseline")
        if any("=" in str(value) for value in server.get("env_vars", [])):
            raise SystemExit("Codex baseline contains an environment value instead of a variable name")

    matrix = json.loads((ROOT / "docs/evidence/mcp003/host-matrix.json").read_text(encoding="utf-8"))
    if matrix.get("task_id") != "FGC-MCP003":
        raise SystemExit("MCP003 host matrix task mismatch")
    for host in HOSTS:
        row = matrix.get(host)
        if not isinstance(row, dict):
            raise SystemExit(f"host row missing: {host}")
        for field in ("discovery", "connection", "read_only_e2e"):
            if row.get(field) not in STATUSES:
                raise SystemExit(f"invalid {host}.{field} status")
        if any(value == "PASS" for key, value in row.items() if key != "reason") and not row.get("evidence"):
            raise SystemExit(f"PASS host row lacks evidence: {host}")
    boundary = matrix.get("security_boundary", {})
    if boundary.get("client_name_is_authentication") is not False:
        raise SystemExit("client-name authentication boundary is missing")
    if boundary.get("runtime_contract_mismatch") != "fail_closed":
        raise SystemExit("runtime mismatch must fail closed")
    print("MCP003 Codex host baselines OK; real host rows remain explicitly accounted")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
