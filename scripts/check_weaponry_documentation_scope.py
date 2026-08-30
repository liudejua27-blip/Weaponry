#!/usr/bin/env python3
"""Fail when a repository Markdown file escapes Weaponry scope classification."""

from __future__ import annotations

from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
IGNORED_PARTS = {"node_modules", "output", ".forgecad-target"}
CURRENT_MARKERS = (
    "Weaponry P0 override (2026-08-29)",
    "Status: reference-only historical document (2026-08-29)",
    "Status: partially superseded by ADR-0029 (2026-08-29)",
    "Status: superseded / no current execution authority (2026-08-29)",
)
NATIVE_WEAPONRY_DOCS = {
    "docs/AUTHORITATIVE_STATE.md",
    "docs/CODEX_HANDOFF.md",
    "docs/DOCUMENTATION_MAP.md",
    "docs/DOCUMENTATION_STATUS.md",
    "docs/WEAPONRY_ARCHITECTURE_TOOL_SKILL_AUDIT_20260829.md",
    "docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md",
    "docs/WEAPONRY_DELETION_MANIFEST_20260829.md",
    "docs/WEAPONRY_DOCUMENTATION_COVERAGE_20260829.md",
    "docs/WEAPONRY_KNIFE_10_DAY_DELIVERY_PLAN.md",
    "docs/WEAPONRY_MODULE_EVALUATION_20260830.md",
    "docs/WEAPONRY_ONE_MONTH_DELIVERY_PLAN.md",
    "docs/WEAPONRY_RUNTIME_FIVE_DOMAIN_REFACTOR_20260829.md",
    "docs/ADR/0030-weaponry-knife-ten-day-hybrid-dcc.md",
    "skills/weaponry-crossfire-agent-native-dcc/SKILL.md",
    "skills/weaponry-crossfire-agent-native-dcc/references/action-space-and-gates.md",
}


def relative(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


def category(path: Path) -> str:
    rel = relative(path)
    parts = set(path.relative_to(ROOT).parts)
    if "docs/evidence/" in rel:
        return "immutable-evidence"
    if rel.startswith("packages/forgecad-skills/archive/"):
        return "immutable-archive"
    if rel.startswith("packages/forgecad-skills/bundles/") or rel.startswith(
        "packages/forgecad-skills/knowledge/"
    ):
        return "hash-bound-active-skill"
    if rel in NATIVE_WEAPONRY_DOCS:
        return "native-weaponry"
    text = path.read_text(encoding="utf-8")
    if any(marker in text for marker in CURRENT_MARKERS):
        return "classified-narrative"
    return "unclassified"


def main() -> None:
    markdown = sorted(
        path
        for path in ROOT.rglob("*.md")
        if not (set(path.relative_to(ROOT).parts) & IGNORED_PARTS)
    )
    classified = [(path, category(path)) for path in markdown]
    unclassified = [relative(path) for path, kind in classified if kind == "unclassified"]
    if unclassified:
        raise SystemExit(
            "Weaponry documentation scope violation; unclassified Markdown:\n- "
            + "\n- ".join(unclassified)
        )
    counts = Counter(kind for _, kind in classified)
    summary = " ".join(f"{kind}={counts[kind]}" for kind in sorted(counts))
    print(f"Weaponry documentation scope OK: total={len(markdown)} {summary}")


if __name__ == "__main__":
    main()
