#!/usr/bin/env python3
"""Layout/ownership smoke for the hard-cut Codex Runtime."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    required = [
        "apps/desktop/src-tauri/crates/forgecad-contracts",
        "apps/desktop/src-tauri/crates/forgecad-core",
        "apps/desktop/src-tauri/crates/forgecad-store",
        "apps/desktop/src-tauri/crates/forgecad-runtime",
        "apps/desktop/src-tauri/crates/forgecad-mcp",
        "apps/desktop/src-tauri/crates/forgecad-worker-protocol",
        "apps/geometry-worker",
        "apps/render-worker",
        "packages/forgecad-contracts",
        "migrations-runtime-v1/0001_runtime.sql",
    ]
    missing = [path for path in required if not (ROOT / path).exists()]
    if missing:
        raise SystemExit(f"runtime layout missing: {missing}")

    forbidden = [
        "apps/agent",
        "apps/desktop/src/features/cad-workbench",
        "apps/desktop/src-tauri/crates/forgecad-app-server",
        "apps/desktop/src-tauri/crates/forgecad-app-server-protocol",
        "apps/desktop/src-tauri/crates/forgecad-core/src/api_first_provider.rs",
        "packages/concept-spec",
        "packages/weapon-spec",
        "packages/agent-skills",
        "evaluations/agent-provider-v1",
        "evaluations/r4",
        "evaluations/csg-g824",
        "evaluations/csg-g824a",
        "evaluations/csg-g824b",
        "evaluations/csg-g824c",
        "evaluations/csg-g824d",
        "script/build_packaged_agent_sidecar.sh",
        "smoke-gate07.env.example",
    ]
    present = [path for path in forbidden if (ROOT / path).exists()]
    if present:
        raise SystemExit(f"legacy runtime paths still exist: {present}")

    package = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))
    scripts = package.get("scripts", {})
    if any(token in key.lower() or token in value.lower() for key, value in scripts.items() for token in ("deepseek", "qwen", "provider", "api-first", "workbench")):
        raise SystemExit("legacy model/provider/workbench script remains")
    print("ForgeCAD runtime layout OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
