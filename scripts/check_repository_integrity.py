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
    "apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx",
    "apps/desktop/src/features/cad-workbench/ModuleGraphViewport.tsx",
    "apps/agent/forgecad_agent/api/module_routes.py",
    "apps/agent/forgecad_agent/application/concept_modules.py",
    "packages/concept-spec/schemas/module-graph.schema.json",
    "migrations/0009_r2_concept_domain.sql",
    "migrations/0016_change_set_audit_exports.sql",
    "docs/OPERATIONS.md",
    "docs/MODULE_ASSET_GUIDE.md",
    "docs/evidence/README.md",
    "docs/evidence/CAPABILITY_GATE_MATRIX.md",
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
