#!/usr/bin/env python3
"""Release docs walkthrough gate for Wushen Forge.

The gate verifies that a new open-source user can find the startup, provider
configuration, Concept asset loop, legacy compatibility references, and release
commands in committed docs, and that referenced npm scripts exist. QUICKSTART is
intentionally an index into OPERATIONS rather than a duplicate legacy manual.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]


REQUIRED_SCRIPTS = [
    "desktop:dev",
    "desktop:build",
    "agent:check",
    "m5:gate",
    "unity:preflight",
    "unity:import:gate",
    "release:safety-scope",
    "release:secrets-files",
    "release:prompt-quality",
    "release:docs-walkthrough",
    "release:packaging-readiness",
    "release:license-sbom",
    "release:gate",
    "r4:planner-gate",
    "agent:r4-change-planner-smoke",
    "agent:r4-change-planner-api-smoke",
    "agent:r4-evaluation-preflight",
    "agent:r4-evaluation-preflight-smoke",
    "assets:formal-review-handoff",
    "assets:formal-review-handoff-smoke",
    "assets:blender-full-candidate-recovery-drill",
    "assets:blender-full-candidate-connector-matrix",
]

DOC_REQUIREMENTS: dict[str, list[str]] = {
    "README.md": [
        "docs/QUICKSTART.md",
        "docs/OPERATIONS.md",
        "npm run r4:planner-gate",
        "npm run release:gate",
        "虚构游戏美术资产",
        "非制造说明",
        "不输出可用于现实制造武器的精确图纸",
    ],
    "docs/QUICKSTART.md": [
        "npm install",
        "python3 -m venv .venv",
        ".venv/bin/pip install -e \"apps/agent[dev]\"",
        ".venv/bin/python -m uvicorn wushen_agent.main:create_app",
        "--factory --host 127.0.0.1 --port 8000",
        "npm run desktop:dev",
        "VITE_FORGE_API_BASE_URL",
        "OPERATIONS.md",
        "MODULE_ASSET_GUIDE.md",
        "npm run r2:gate",
        "npm run r3:workbench-gate",
        "npm run r4:planner-gate",
        "npm run release:safety-scope",
        "npm run release:secrets-files",
        "npm run release:prompt-quality",
        "npm run release:docs-walkthrough",
        "npm run release:packaging-readiness",
        "npm run release:license-sbom",
        "npm run release:gate",
        "半透明 ghost",
        "真实模型 AI 指标",
        "agent:r4-evaluation-preflight",
    ],
    "docs/API.md": [
        "POST /api/weapons",
        "POST /api/weapons/{weapon_id}/patch",
        "POST /api/weapons/{weapon_id}/generate-3d",
        "POST /api/weapons/{weapon_id}/export-unity",
        "GET /api/provider-settings",
        "GET /api/health",
        "Idempotency-Key",
        "WUSHEN_3D_PROVIDER=local_http",
        "WUSHEN_EXPORT_UNITY_ASYNC=1",
    ],
    "docs/M3_DESKTOP_SUPERVISOR.md": [
        ".venv/bin/python -m uvicorn wushen_agent.main:create_app --factory --host 127.0.0.1 --port 8000",
        "WUSHEN_LIBRARY_ROOT",
        "WUSHEN_MIGRATIONS_DIR",
        "WUSHEN_REPO_ROOT",
        "WUSHEN_AGENT_PYTHON",
    ],
    "docs/LOCAL_3D_RUNTIME.md": [
        "WUSHEN_3D_PROVIDER=local_http",
        "scripts/wushen_local_3d_runtime.py",
        "agent:p0-local-3d-runtime-sf3d-manual",
        "agent:p0-local-3d-runtime-triposr-manual",
    ],
    "docs/UNITY_IMPORT_SMOKE.md": [
        "npm run unity:preflight",
        "npm run unity:import:gate",
        "WUSHEN_UNITY_EXECUTABLE",
        "blocked_unity_not_configured",
    ],
    "workflows/comfyui/README.md": [
        "WUSHEN_COMFYUI_WORKFLOW_TEMPLATE",
        "WUSHEN_COMFYUI_PATCH_WORKFLOW_TEMPLATE",
        "WUSHEN_COMFYUI_BASE_URL",
        "WUSHEN_COMFYUI_CHECKPOINT",
    ],
}


def main() -> int:
    blockers: list[dict[str, Any]] = []
    summaries: dict[str, Any] = {}

    package = _read_json(ROOT / "package.json")
    scripts = package.get("scripts") if isinstance(package.get("scripts"), dict) else {}
    missing_scripts = [script for script in REQUIRED_SCRIPTS if script not in scripts]
    if missing_scripts:
        blockers.append(
            {
                "severity": "blocker",
                "code": "MISSING_NPM_SCRIPT",
                "message": "Release walkthrough references npm scripts that are not defined.",
                "details": {"scripts": missing_scripts},
            }
        )
    summaries["scripts"] = {"required": len(REQUIRED_SCRIPTS), "missing": missing_scripts}

    docs_summary: dict[str, Any] = {}
    for rel_path, phrases in DOC_REQUIREMENTS.items():
        path = ROOT / rel_path
        if not path.exists():
            blockers.append(
                {
                    "severity": "blocker",
                    "code": "MISSING_DOC",
                    "message": f"{rel_path} is required by the release walkthrough.",
                }
            )
            docs_summary[rel_path] = {"exists": False, "missing_phrases": phrases}
            continue
        text = path.read_text(encoding="utf-8")
        missing = [phrase for phrase in phrases if phrase not in text]
        if missing:
            blockers.append(
                {
                    "severity": "blocker",
                    "code": "DOC_WALKTHROUGH_GAP",
                    "message": f"{rel_path} is missing required walkthrough phrase(s).",
                    "details": {"phrases": missing},
                }
            )
        docs_summary[rel_path] = {"exists": True, "missing_phrases": missing}
    summaries["docs"] = docs_summary

    endpoint_mismatches = _check_api_endpoint_consistency()
    if endpoint_mismatches:
        blockers.append(
            {
                "severity": "blocker",
                "code": "API_ENDPOINT_DOC_MISMATCH",
                "message": "Quickstart endpoint list is not covered by API.md.",
                "details": {"endpoints": endpoint_mismatches},
            }
        )
    summaries["api_endpoint_mismatches"] = endpoint_mismatches

    quickstart_script_refs = _extract_npm_script_refs((ROOT / "docs" / "QUICKSTART.md").read_text(encoding="utf-8"))
    missing_quickstart_scripts = sorted(script for script in quickstart_script_refs if script not in scripts)
    if missing_quickstart_scripts:
        blockers.append(
            {
                "severity": "blocker",
                "code": "QUICKSTART_SCRIPT_REF_MISSING",
                "message": "Quickstart references npm scripts that are not defined.",
                "details": {"scripts": missing_quickstart_scripts},
            }
        )
    summaries["quickstart_script_refs"] = {
        "referenced": sorted(quickstart_script_refs),
        "missing": missing_quickstart_scripts,
    }

    report = {"ok": not blockers, "summaries": summaries, "blockers": blockers}
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if not blockers else 1


def _check_api_endpoint_consistency() -> list[str]:
    quickstart = (ROOT / "docs" / "QUICKSTART.md").read_text(encoding="utf-8")
    api = (ROOT / "docs" / "API.md").read_text(encoding="utf-8")
    endpoints = sorted(set(re.findall(r"(?:GET|POST|PUT|DELETE) /api/[A-Za-z0-9_/{}/-]+", quickstart)))
    return [endpoint for endpoint in endpoints if endpoint not in api]


def _extract_npm_script_refs(text: str) -> set[str]:
    return set(re.findall(r"npm run ([A-Za-z0-9:_-]+)", text))


def _read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    raise SystemExit(main())
