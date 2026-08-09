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
REQUIRED_HOSTS = ("codex_desktop", "codex_cli")
STATUSES = {"PASS", "NOT_RUN", "BLOCKED", "NOT_APPLICABLE"}
SCOPES = {"REQUIRED", "OPTIONAL_NOT_IN_SCOPE", "FUTURE_NOT_IN_SCOPE", "NON_BLOCKING_FUTURE"}


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
            raise SystemExit("Codex config must use the single-user forgecad-mcp entrypoint")
        if server.get("args") != ["serve", "--stdio"]:
            raise SystemExit("MCP003 config must start forgecad-mcp in stdio mode")
        if server.get("enabled") is not True or server.get("required") is not False or server.get("default_tools_approval_mode") != "writes":
            raise SystemExit("Codex baselines must enable ForgeCAD without making it startup-required")
        if "FORGECAD_MCP_HOST_DIAGNOSTIC_FIXTURE" in text or "FORGECAD_RUNTIME_DATA_DIR" in text:
            raise SystemExit("Codex baselines must not carry diagnostic fixture or test data environment names")
        expected_env_vars = {"FORGECAD_MCP_ENABLE_MCP004_WRITES"}
        if set(server.get("env_vars", [])) != expected_env_vars:
            raise SystemExit("Codex baselines must forward only the selected host or Runtime IPC variable names")
        if os.path.isabs(str(server.get("command"))):
            raise SystemExit("absolute command path in Codex baseline")
        if any("=" in str(value) for value in server.get("env_vars", [])):
            raise SystemExit("Codex baseline contains an environment value instead of a variable name")

    matrix = json.loads((ROOT / "docs/evidence/mcp003/host-matrix.json").read_text(encoding="utf-8"))
    if matrix.get("task_id") != "FGC-MCP003":
        raise SystemExit("MCP003 host matrix task mismatch")
    if matrix.get("status") != "PASS":
        raise SystemExit("MCP003 required host matrix is not PASS")
    for host in HOSTS:
        row = matrix.get(host)
        if not isinstance(row, dict):
            raise SystemExit(f"host row missing: {host}")
        if row.get("scope") not in SCOPES:
            raise SystemExit(f"invalid {host}.scope")
        for field in ("discovery", "connection", "read_only_e2e"):
            if row.get(field) not in STATUSES:
                raise SystemExit(f"invalid {host}.{field} status")
        if any(value == "PASS" for key, value in row.items() if key != "reason") and not row.get("evidence"):
            raise SystemExit(f"PASS host row lacks evidence: {host}")

    required_gate = matrix.get("required_gate")
    if not isinstance(required_gate, dict) or required_gate.get("status") != "PASS":
        raise SystemExit("MCP003 required gate is not PASS")
    if required_gate.get("required_hosts") != list(REQUIRED_HOSTS):
        raise SystemExit("MCP003 required host scope drifted")

    protocol = matrix.get("protocol_adapter", {})
    for field in ("discovery", "initialize", "read_only_tools", "resources_list_read", "version_mismatch_fail_closed"):
        if protocol.get(field) != "PASS":
            raise SystemExit(f"protocol adapter required gate is not PASS: {field}")
    for host in REQUIRED_HOSTS:
        row = matrix[host]
        if row.get("scope") != "REQUIRED":
            raise SystemExit(f"required host is not marked REQUIRED: {host}")
        for field in ("discovery", "connection", "read_only_e2e"):
            if row.get(field) != "PASS":
                raise SystemExit(f"required host gate is not PASS: {host}.{field}")
        if row.get("write_transactions") is not False or row.get("side_effects") is not False:
            raise SystemExit(f"required host has unproven side-effect-free read-only behavior: {host}")

    desktop = matrix["codex_desktop"]
    if desktop.get("initialize_protocol_version") != "2025-06-18":
        raise SystemExit("Codex Desktop initialize protocol version is not recorded")
    if desktop.get("version_mismatch") != "NOT_APPLICABLE" or desktop.get("host_override_result") != "HOST_OVERRIDE_IGNORED":
        raise SystemExit("Codex Desktop forced mismatch must be HOST_OVERRIDE_IGNORED / NOT_APPLICABLE")
    observer = desktop.get("capture_observer", {})
    if (
        observer.get("mode") != "transparent_passthrough_logger"
        or observer.get("request_rewriting") is not False
        or observer.get("synthetic_messages") is not False
        or observer.get("write_e2e_claim") is not False
    ):
        raise SystemExit("Desktop capture observer must be transparent, non-synthetic and non-write evidence")
    mismatch_attempt = json.loads(
        (ROOT / "docs/evidence/mcp003/codex-desktop-mismatch-attempt.json").read_text(encoding="utf-8")
    )
    mismatch_observer = mismatch_attempt.get("capture_observer", {})
    if (
        mismatch_observer.get("request_rewriting") is not False
        or mismatch_observer.get("synthetic_messages") is not False
        or mismatch_observer.get("used_to_claim_write_e2e") is not False
    ):
        raise SystemExit("Desktop mismatch evidence must not depend on rewritten or synthetic requests")

    cli = matrix["codex_cli"]
    if cli.get("version_mismatch") != "PASS" or cli.get("silent_downgrade") is not False:
        raise SystemExit("Codex CLI protocol mismatch must fail closed without silent downgrade")
    if cli.get("mismatch_tool_calls") != 0 or cli.get("mismatch_side_effects") is not False:
        raise SystemExit("Codex CLI mismatch must have no tool calls or side effects")

    boundary = matrix.get("security_boundary", {})
    if boundary.get("client_name_is_authentication") is not False:
        raise SystemExit("client-name authentication boundary is missing")
    if boundary.get("runtime_contract_mismatch") != "fail_closed":
        raise SystemExit("runtime mismatch must fail closed")
    print("MCP003 Codex host baselines OK; real host rows remain explicitly accounted")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
