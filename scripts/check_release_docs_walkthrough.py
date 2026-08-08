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
    ]
    missing = [path for path in required if not (ROOT / path).exists()]
    if missing:
        raise SystemExit(f"authority docs missing: {missing}")

    forbidden = [
        "docs/ADR/0023-deepseek-qwen-only-ai-provider-policy.md",
        "docs/ADR/0024-api-first-open-world-3d-coding-agent.md",
        "docs/AGENT_GITHUB_REFERENCE_ARCHITECTURE.md",
        "docs/AGENT_PLUGINS_SKILLS_DESIGN.md",
        "docs/COMPATIBILITY_MIGRATION.md",
    ]
    present = [path for path in forbidden if (ROOT / path).exists()]
    if present:
        raise SystemExit(f"superseded authority docs still present: {present}")

    task_index = (ROOT / "docs/CODEX_TASK_INDEX.md").read_text(encoding="utf-8")
    if "FGC-MCP001" not in task_index:
        raise SystemExit("MCP001 is not in the task index")
    print("release docs walkthrough OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
