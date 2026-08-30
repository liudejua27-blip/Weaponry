#!/usr/bin/env python3
"""Verify that the default MCP test target does not compile compatibility code.

This checker consumes Cargo dep-info produced from two isolated target
directories.  Source declarations and warnings are not accepted as proof:
the default binary/test compile graph must contain only the active Knife
adapter modules, while the explicit compatibility binary must contain the
historical registry and representative raw handlers.
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


PREFIX = "crates/forgecad-mcp/src/"
DEFAULT_EXACT = {
    "active_manifest.rs",
    "active_schema.rs",
    "active_session.rs",
    "domain_router.rs",
    "knife_curve_evaluated_mesh_tools.rs",
    "knife_curve_modifier_graph_tools.rs",
    "knife_tool_profile.rs",
    "main.rs",
    "result_adapter.rs",
    "supervisor.rs",
}
COMPAT_REQUIRED = {
    "agentic_action_tools.rs",
    "agentic_orchestrator_tools.rs",
    "agentic_tools.rs",
    "agentic_write_tools.rs",
    "agentic_write_tools_session.rs",
    "agentic_write_tools_tests.rs",
    "compat_handler.rs",
    "compat_main.rs",
    "compatibility_registry.rs",
    "native_high_durable_tools.rs",
    "production_weapon_form_art_mesh_proposal_tools.rs",
    "production_weapon_high_low_bake_tools.rs",
    "result_adapter.rs",
    "weapon_foundation_tools.rs",
}


def fail(message: str) -> None:
    raise SystemExit(f"WPN_MCP_COMPAT_ISOLATION_INVALID: {message}")


def source_set(path: Path) -> set[str]:
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        fail(f"cannot read dep-info {path}: {exc}")
    matches = re.findall(r"(?:^|\s)(?:[^\s]*?/)?" + re.escape(PREFIX) + r"([A-Za-z0-9_]+\.rs)(?=\s|$)", text)
    if not matches:
        fail(f"dep-info {path} contains no forgecad-mcp source files")
    return set(matches)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--default-dep-info", required=True, type=Path)
    parser.add_argument("--compat-dep-info", required=True, type=Path)
    args = parser.parse_args()

    default = source_set(args.default_dep_info)
    compat = source_set(args.compat_dep_info)
    if default != DEFAULT_EXACT:
        fail(
            "default compile graph drifted: "
            f"missing={sorted(DEFAULT_EXACT - default)} extra={sorted(default - DEFAULT_EXACT)}"
        )
    missing_compat = COMPAT_REQUIRED - compat
    if missing_compat:
        fail(f"compatibility compile graph lost required modules: {sorted(missing_compat)}")
    if "main.rs" in compat:
        fail("compatibility binary must not compile the default main.rs")
    if "compat_main.rs" in default or "compatibility_registry.rs" in default:
        fail("default binary compiled compatibility entry or registry")

    print(
        json.dumps(
            {
                "schema_version": "WeaponryMcpCompatibilityIsolationCheck@1",
                "status": "PASS",
                "default_source_count": len(default),
                "default_sources": sorted(default),
                "compatibility_source_count": len(compat),
                "compatibility_required_sources_present": sorted(COMPAT_REQUIRED),
                "default_compiles_compatibility_registry": False,
                "default_compiles_compatibility_entry": False,
            },
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
