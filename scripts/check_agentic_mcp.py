#!/usr/bin/env python3
"""Check the Agentic Design Runtime MCP adapter boundary."""

import argparse
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MAIN = ROOT / "apps/desktop/src-tauri/crates/forgecad-mcp/src/main.rs"
AGENTIC = ROOT / "apps/desktop/src-tauri/crates/forgecad-mcp/src/agentic_tools.rs"
AGENTIC_WRITE = ROOT / "apps/desktop/src-tauri/crates/forgecad-mcp/src/agentic_write_tools.rs"

TOOLS = {
    "scene_observe_get": True,
    "design_stage_plan_get": True,
    "critic_report_get": True,
    "visual_evidence_bundle_get": True,
    "session_get": True,
    "checkpoint_get": True,
}

WRITE_TOOLS = {
    "session_create_or_resume",
    "checkpoint_prepare",
    "checkpoint_restore_prepare",
}


def fail(message: str) -> "NoReturn":
    raise SystemExit(f"agentic MCP check failed: {message}")


def load_manifest(path: Path) -> list[dict]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"manifest could not be read: {error}")
    if isinstance(payload, dict) and isinstance(payload.get("result"), dict):
        payload = payload["result"]
    if isinstance(payload, dict):
        payload = payload.get("tools")
    if not isinstance(payload, list) or not all(isinstance(tool, dict) for tool in payload):
        fail("manifest must be a tools array or a tools/list response")
    return payload


def check_manifest(tools: list[dict], write_enabled: bool) -> None:
    by_name = {tool.get("name"): tool for tool in tools}
    if len(by_name) != len(tools):
        fail("manifest contains duplicate tool names")
    for name, available in TOOLS.items():
        tool = by_name.get(name)
        if tool is None:
            fail(f"missing {name} from read manifest")
        annotations = tool.get("annotations", {})
        if annotations.get("readOnlyHint") is not True:
            fail(f"{name} is not read-only")
        if annotations.get("destructiveHint") is not False:
            fail(f"{name} is destructive")
        if annotations.get("idempotentHint") is not True:
            fail(f"{name} is not idempotent")
        if annotations.get("openWorldHint") is not False:
            fail(f"{name} is open-world")
        forgecad = tool.get("_meta", {}).get("forgecad", {})
        expected = "available" if available else "unavailable"
        if forgecad.get("availability") != expected:
            fail(f"{name} availability is not {expected}")
        if name in {"session_get", "checkpoint_get"}:
            if forgecad.get("runtime_method") != name:
                fail(f"{name} is not bound to its assumed Runtime method")
            if annotations.get("writeIntent") is not False:
                fail(f"{name} has write intent")
            if annotations.get("approvalRequired") is not False:
                fail(f"{name} requires approval")

    present_writes = {name for name in by_name if name in WRITE_TOOLS}
    if write_enabled and present_writes != WRITE_TOOLS:
        fail(f"write-enabled manifest is missing Agentic tools: {WRITE_TOOLS - present_writes}")
    if not write_enabled and present_writes:
        fail("write tools are visible without explicit opt-in")
    for name in present_writes:
        tool = by_name[name]
        annotations = tool.get("annotations", {})
        if annotations.get("readOnlyHint") is not False:
            fail(f"{name} is marked read-only")
        if annotations.get("destructiveHint") is not False:
            fail(f"{name} must be non-destructive at prepare boundary")
        if annotations.get("writeIntent") is not True:
            fail(f"{name} is missing writeIntent=true")
        if annotations.get("approvalRequired") is not True:
            fail(f"{name} is missing approvalRequired=true")
        forgecad = tool.get("_meta", {}).get("forgecad", {})
        if forgecad.get("runtime_method") != name:
            fail(f"{name} is not bound to its assumed Runtime method")


def check_sources() -> None:
    if not MAIN.is_file() or not AGENTIC.is_file() or not AGENTIC_WRITE.is_file():
        fail("agentic adapter source is missing")
    main = MAIN.read_text(encoding="utf-8")
    agentic = AGENTIC.read_text(encoding="utf-8")
    agentic_write = AGENTIC_WRITE.read_text(encoding="utf-8")

    for marker in (
        "mod agentic_tools;",
        "mod agentic_write_tools;",
        "tools.extend(agentic_tools::read_tools());",
        "tools.extend(agentic_write_tools::read_tools());",
        "agentic_write_tools::write_tools()",
        "if agentic_tools::is_tool(name)",
        "agentic_write_tools::validate_call",
        "agentic_tools::runtime_method(name)",
        "requires_ponytail_preflight",
    ):
        if marker not in main:
            fail(f"main.rs is missing boundary marker: {marker}")

    for name in TOOLS:
        if f'"{name}"' not in agentic:
            if f'"{name}"' not in agentic_write:
                fail(f"Agentic adapter is missing {name}")
    if 'Some("agentic_critic_projection")' not in agentic:
        fail("critic projection is not bound to the Runtime projection method")
    if 'Some("visual_evidence_bundle_get")' not in agentic:
        fail("evidence projection is not bound to the existing Runtime read method")
    for method in ("agentic_scene_observe", "agentic_stage_plan"):
        if f'"{method}"' not in agentic:
            fail(f"unimplemented Runtime method is not recorded: {method}")
    if "forgecad_runtime" in agentic or "rusqlite" in agentic or "sqlite" in agentic:
        fail("agentic_tools.rs must not acquire a database or Runtime dependency")
    if "write_tool" in agentic or "candidate_confirm" in agentic or "version_confirm" in agentic:
        fail("agentic_tools.rs contains a write/confirm surface")
    if main.count("agentic_tools::read_tools()") != 1:
        fail("agentic read definitions must be added only to the read manifest")
    for marker in (
        "session_create_or_resume",
        "session_get",
        "checkpoint_prepare",
        "checkpoint_get",
        "checkpoint_restore_prepare",
        "writeIntent",
        "approvalRequired",
        "AGENTIC_SCOPE_MISMATCH",
        "AGENTIC_VISUAL_STATE_UNKNOWN",
        "AGENTIC_RUNTIME_METHOD_UNAVAILABLE",
    ):
        if marker not in agentic_write:
            fail(f"agentic_write_tools.rs is missing boundary marker: {marker}")
    forbidden_adapter_tokens = (
        "forgecad_runtime",
        "rusqlite",
        "sqlite",
        "cas",
        "reqwest",
        "ureq",
        "std::process",
        "Command::",
        "invokeModel",
        "fetch(",
    )
    if any(token in agentic_write for token in forbidden_adapter_tokens):
        fail("agentic_write_tools.rs must remain a thin adapter")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--manifest",
        type=Path,
        help="optional tools/list JSON response or tools array to validate",
    )
    parser.add_argument(
        "--write-enabled",
        action="store_true",
        help="expect the explicit write-enabled manifest",
    )
    args = parser.parse_args()
    check_sources()
    if args.manifest:
        check_manifest(load_manifest(args.manifest), args.write_enabled)
    print("ForgeCAD Agentic MCP adapter boundary OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
