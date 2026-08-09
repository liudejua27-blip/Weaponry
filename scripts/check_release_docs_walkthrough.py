#!/usr/bin/env python3
"""Check that the current authority chain describes the MCP reset, not the old app."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    required = [
        "docs/DOCUMENTATION_MAP.md",
        "docs/DOCUMENTATION_STATUS.md",
        "docs/CODEX_HANDOFF.md",
        "docs/CODEX_EXECUTION_PLAN.md",
        "docs/CODEX_TASK_INDEX.md",
        "docs/MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md",
        "docs/MVP_DELIVERY_PLAN.md",
        "docs/MVP_TOOL_CATALOG.md",
        "docs/AUTHORITATIVE_STATE.md",
        "docs/DESIGN.md",
        "docs/ADR/0025-codex-only-mcp-3d-runtime.md",
        "docs/MCP_RUNTIME_CONTRACT.md",
        "docs/CODEX_INTEGRATION.md",
        "docs/COMPILER_PIPELINE.md",
        "docs/WORKBENCH_VIEWER.md",
        "docs/SKILL_PACKAGE_STANDARD.md",
        "docs/EXTERNAL_PROJECT_ADOPTION.md",
        "docs/RESET_MIGRATION_PLAN.md",
        "docs/evidence/mcp000/manifest.json",
        "docs/evidence/mcp001/manifest.json",
        "docs/evidence/mcp002/manifest.json",
        "docs/evidence/mcp003/manifest.json",
        "docs/evidence/mcp003/protocol-snapshot.json",
        "docs/evidence/mcp003/host-matrix.json",
        "docs/evidence/mcp004/manifest.json",
        "docs/evidence/mcp005/manifest.json",
        "docs/evidence/mcp006/manifest.json",
        "docs/evidence/mcp007/manifest.json",
        "docs/evidence/mcp008/manifest.json",
        "docs/evidence/mcp009/manifest.json",
    ]
    missing = [path for path in required if not (ROOT / path).exists()]
    if missing:
        raise SystemExit(f"authority docs missing: {missing}")

    forbidden = [
        "docs/ADR/0023-deepseek-qwen-only-ai-provider-policy.md",
        "docs/ADR/0024-api-first-open-world-3d-coding-agent.md",
        "docs/U004_STAGE1_HIGH_QUALITY_WORKBENCH_PLAN.md",
        "docs/AGENT_PROVIDER_EVALUATION.md",
        "docs/AGENT_CURRENT_ISSUES_AUDIT.md",
        "docs/IMPLEMENTATION_PLAN.md",
        "docs/DOMAIN_PACKS.md",
        "docs/MECHANICAL_DESIGN_OPERATIONS.md",
        "docs/MODULE_ASSET_GUIDE.md",
        "docs/MODULE_NAMING_STANDARD.md",
        "docs/AGENT_GITHUB_REFERENCE_ARCHITECTURE.md",
        "docs/AGENT_PLUGINS_SKILLS_DESIGN.md",
        "docs/COMPATIBILITY_MIGRATION.md",
        "docs/EXTERNAL_REFERENCE_AND_PRODUCT_DIFFERENTIATION.md",
        "docs/API.md",
        "docs/FRONTEND.md",
        "docs/evidence/U002_UNIVERSAL_AUTHOR_GATE.md",
        "docs/evidence/U004_W4_INTEGRATION_EVIDENCE_MANIFEST.md",
        "docs/evidence/f026",
        "docs/examples/module-pack",
        "docs/legacy",
    ]
    present = [path for path in forbidden if (ROOT / path).exists()]
    if present:
        raise SystemExit(f"superseded authority docs still present: {present}")

    task_index = (ROOT / "docs/CODEX_TASK_INDEX.md").read_text(encoding="utf-8")
    if "FGC-MCP001" not in task_index:
        raise SystemExit("MCP001 is not in the task index")
    if (
        "FGC-MCP004 | done" not in task_index
        or "FGC-MCP005 | done" not in task_index
        or "FGC-MCP006 | done" not in task_index
        or "FGC-MCP007 | done" not in task_index
        or "FGC-MCP008 | done" not in task_index
        or "FGC-MCP009 | done" not in task_index
    ):
        raise SystemExit("MVP task handoff must preserve MCP004-009 done")
    required_mcp010_rows = (
        "FGC-MCP010A | in_progress | MCP009",
        "FGC-MCP010B | blocked | MCP010A",
        "FGC-MCP010C | blocked | MCP010B",
        "FGC-MCP010D | blocked | MCP010C",
        "FGC-MCP010E | blocked | MCP010D",
        "FGC-MCP010F | blocked | MCP010E",
        "FGC-MCP011 | blocked | MCP010F",
    )
    missing_mcp010_rows = [row for row in required_mcp010_rows if row not in task_index]
    if missing_mcp010_rows:
        raise SystemExit(f"MCP010A-F task chain is incomplete: {missing_mcp010_rows}")

    mcp010_plan = (ROOT / "docs/MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md").read_text(
        encoding="utf-8"
    )
    required_mcp010_terms = (
        "PARTIAL_VISIBLE_VIEW_PASS",
        "BLOCKED_REFERENCE_COVERAGE",
        "17 read + 13 write",
        "44 个 JSON Schema",
        "MCP010F",
    )
    missing_mcp010_terms = [
        term for term in required_mcp010_terms if term not in mcp010_plan
    ]
    if missing_mcp010_terms:
        raise SystemExit(f"MCP010 authority plan is incomplete: {missing_mcp010_terms}")

    luna = (ROOT / "docs/LUNA_GOAL_EXECUTION_GUIDE.md").read_text(encoding="utf-8")
    if "MVP_DELIVERY_PLAN.md" not in luna or "FGC-MCP005" not in luna:
        raise SystemExit("Luna guide does not point to the MVP delivery plan and MCP005")

    mvp = (ROOT / "docs/MVP_DELIVERY_PLAN.md").read_text(encoding="utf-8")
    required_mvp_terms = (
        "reference_import",
        "GeometryProgram@1",
        "MCP009",
        "change_prepare",
        "mvp-glb",
        "approved-for-evaluation",
    )
    missing_terms = [term for term in required_mvp_terms if term not in mvp]
    if missing_terms:
        raise SystemExit(f"MVP delivery plan is incomplete: {missing_terms}")
    print("release docs walkthrough OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
