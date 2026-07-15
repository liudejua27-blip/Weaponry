#!/usr/bin/env python3
"""Fail fast when the documented ForgeCAD product tree is only partially merged."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REQUIRED_PATHS = (
    "README.md",
    "AGENTS.md",
    "apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx",
    "apps/desktop/src/features/cad-workbench/ModuleGraphViewport.tsx",
    "apps/agent/forgecad_agent/api/module_routes.py",
    "apps/agent/forgecad_agent/application/concept_modules.py",
    "packages/concept-spec/schemas/module-graph.schema.json",
    "migrations/0009_r2_concept_domain.sql",
    "migrations/0016_change_set_audit_exports.sql",
    "docs/OPERATIONS.md",
    "docs/ADR/0009-active-design-snapshot.md",
    "docs/DOCUMENTATION_MAP.md",
    "docs/AGENT_GITHUB_REFERENCE_ARCHITECTURE.md",
    "docs/AGENT_PLUGINS_SKILLS_DESIGN.md",
    "docs/USER_GUIDE.md",
    "docs/API.md",
    "docs/DEVELOPMENT.md",
    "docs/ASSET_AUTHORING.md",
    "docs/RELEASE_MAINTENANCE.md",
    "docs/AUTHORITATIVE_STATE.md",
    "docs/TEST_STRATEGY.md",
    "docs/COMPATIBILITY_MIGRATION.md",
    "docs/PRODUCTION_RELEASE_CHECKLIST.md",
    "docs/DISASTER_RECOVERY.md",
    "docs/legacy/README.md",
    "docs/CODEX_HANDOFF.md",
    "docs/CODEX_EXECUTION_PLAN.md",
    "docs/CODEX_TASK_INDEX.md",
    "docs/CODEX_DEFINITION_OF_DONE.md",
    "docs/MODULE_ASSET_GUIDE.md",
    "docs/evidence/README.md",
    "docs/evidence/CAPABILITY_GATE_MATRIX.md",
    "packages/concept-spec/schemas/active-design-snapshot.schema.json",
    "packages/concept-spec/schemas/domain-inference-result.schema.json",
    "packages/concept-spec/fixtures/domain-inference-keywords.json",
    "migrations/0023_active_design_snapshots.sql",
    "migrations/0024_legacy_agent_conversion_intents.sql",
    "migrations/0025_agent_asset_quality_reports.sql",
    "migrations/0026_agent_asset_navigation_frames.sql",
    ".github/workflows/repository-integrity.yml",
    ".github/workflows/forgecad-core.yml",
    ".github/workflows/tauri-preflight.yml",
    ".github/workflows/security-baseline.yml",
)
REQUIRED_SCRIPTS = (
    "r1:gate",
    "r2:gate",
    "r3:workbench-gate",
    "r4:planner-gate",
    "desktop:r3-concept-workbench-smoke",
    "contracts:types:check",
    "release:secrets-files",
    "agent:g1-kernel-smoke",
    "agent:s1-active-design-snapshot-smoke",
    "agent:s2-active-design-snapshot-smoke",
    "agent:s3-active-design-api-smoke",
    "agent:s7-legacy-conversion-smoke",
    "agent:s8-active-design-navigation-smoke",
    "desktop:s5-active-design-machine-smoke",
    "agent:g2-contracts-smoke",
    "agent:d1-domain-inference-contract-smoke",
    "agent:d2-domain-inference-service-smoke",
    "agent:g3-shape-program-smoke",
    "agent:g4-mechanical-planner-smoke",
    "agent:g5-geometry-worker-smoke",
    "agent:g6-segmentation-smoke",
    "agent:g6-material-catalog-smoke",
    "agent:g6-asset-editing-smoke",
    "agent:g6-component-registry-smoke",
    "agent:g7-external-glb-import-smoke",
)
REQUIRED_CODE_MARKERS = {
    "apps/desktop/src/App.tsx": ("CadWorkbenchPanel", "cad"),
    "apps/agent/wushen_agent/main.py": ("build_module_router", "build_concept_project_router"),
}
LINK_PATTERN = re.compile(r"(?<!!)\[[^\]]*\]\(([^)]+)\)")


def main() -> int:
    failures: list[str] = []
    for relative_path in REQUIRED_PATHS:
        if not (ROOT / relative_path).is_file():
            failures.append(f"missing required path: {relative_path}")

    package = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))
    scripts = package.get("scripts", {})
    for script_name in REQUIRED_SCRIPTS:
        if script_name not in scripts:
            failures.append(f"missing required package script: {script_name}")

    for relative_path, markers in REQUIRED_CODE_MARKERS.items():
        path = ROOT / relative_path
        if not path.is_file():
            continue
        content = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in content:
                failures.append(f"missing {marker!r} marker in {relative_path}")

    for markdown_path in [ROOT / "README.md", *(ROOT / "docs").rglob("*.md")]:
        for target in local_markdown_targets(markdown_path):
            resolved = (markdown_path.parent / target).resolve()
            if not resolved.exists():
                failures.append(
                    f"broken local documentation link in {markdown_path.relative_to(ROOT)}: {target}"
                )

    if failures:
        print("Repository integrity check failed:", file=sys.stderr)
        print("\n".join(f"- {failure}" for failure in failures), file=sys.stderr)
        return 1
    print(
        json.dumps(
            {
                "ok": True,
                "required_paths": len(REQUIRED_PATHS),
                "required_scripts": len(REQUIRED_SCRIPTS),
            },
            ensure_ascii=False,
        )
    )
    return 0


def local_markdown_targets(markdown_path: Path) -> list[str]:
    targets: list[str] = []
    content = markdown_path.read_text(encoding="utf-8")
    for match in LINK_PATTERN.finditer(content):
        target = match.group(1).strip().strip("<>").split("#", 1)[0]
        if not target or "://" in target or target.startswith(("mailto:", "#")):
            continue
        targets.append(target)
    return targets


if __name__ == "__main__":
    raise SystemExit(main())
