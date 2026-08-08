#!/usr/bin/env python3
"""Static MCP003 protocol/resource contract gate."""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SNAPSHOT = ROOT / "docs/evidence/mcp003/protocol-snapshot.json"
SOURCE = ROOT / "apps/desktop/src-tauri/crates/forgecad-mcp/src/main.rs"
CONTRACTS_SOURCE = ROOT / "apps/desktop/src-tauri/crates/forgecad-contracts/src/lib.rs"


def read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def main() -> int:
    snapshot = read_json(SNAPSHOT)
    source = SOURCE.read_text(encoding="utf-8")
    if snapshot.get("task_id") != "FGC-MCP003":
        raise SystemExit("MCP003 protocol snapshot task mismatch")
    if snapshot.get("protocol_version") != "2025-11-25":
        raise SystemExit("MCP003 must pin the stable 2025-11-25 protocol")
    if snapshot.get("compatible_protocol_versions") != ["2025-06-18", "2025-11-25"]:
        raise SystemExit("MCP003 Codex legacy compatibility versions drifted")
    if snapshot.get("server_name") != "forgecad":
        raise SystemExit("MCP server name must be forgecad")
    if len(snapshot.get("source_urls", [])) < 3:
        raise SystemExit("MCP003 protocol sources are incomplete")
    for method in snapshot["methods"]:
        if f'"{method}"' not in source:
            raise SystemExit(f"MCP method missing from source: {method}")
    required_annotations = snapshot["required_tool_annotations"]
    for field, expected in required_annotations.items():
        if field not in source or (expected is True and f'"{field}":true' not in source) or (expected is False and f'"{field}":false' not in source):
            raise SystemExit(f"MCP tool annotation missing: {field}={expected}")
    for tool in snapshot["tools"]:
        if f'"{tool}"' not in source:
            raise SystemExit(f"MCP tool missing from source: {tool}")
    for template in snapshot["resource_templates"]:
        if template not in source:
            raise SystemExit(f"MCP resource template missing from source: {template}")
    if len("ForgeCAD is a local Codex-only 3D Runtime. Read capabilities and projects first; permanent writes require a prepared candidate and user approval. Long work returns a RuntimeJob. Do not send arbitrary code, URLs, secrets, or unauthorized paths.") > snapshot["server_instructions_max_bytes"]:
        raise SystemExit("server instructions exceed the first-512-byte Codex contract")
    if "clientInfo" not in source or "CONTRACT_VERSION_UNSUPPORTED" not in source:
        raise SystemExit("initialize contract/version fail-closed markers missing")
    contracts = CONTRACTS_SOURCE.read_text(encoding="utf-8")
    if "MCP_PROTOCOL_VERSIONS" not in source or "2025-06-18" not in contracts:
        raise SystemExit("Codex 2025-06-18 compatibility surface missing")
    if "client_name" in source.lower() or "clientname" in source.lower():
        raise SystemExit("client name must not be used as authentication")
    if re.search(r"(?:/Users/|/home/|[A-Za-z]:\\\\)", source):
        raise SystemExit("absolute machine path in MCP source")
    print("MCP003 protocol snapshot OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
