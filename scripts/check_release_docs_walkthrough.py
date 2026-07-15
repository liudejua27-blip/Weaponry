#!/usr/bin/env python3
"""Verify ForgeCAD's current documentation contract.

The gate protects three boundaries:
1. current Agent capabilities are discoverable and backed by existing commands;
2. user-facing docs do not promise known-unimplemented operations;
3. rejected Weapon/Unity/ComfyUI/neural-3D operation guides stay deleted from the current tree.
"""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]

REQUIRED_SCRIPTS = [
    "agent:check",
    "contracts:types:check",
    "desktop:typecheck",
    "desktop:build",
    "desktop:tauri-check",
    "desktop:r3-concept-workbench-smoke",
    "desktop:d3-domain-clarification-smoke",
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
    "agent:d3-domain-clarification-smoke",
    "agent:g3-shape-program-smoke",
    "agent:g4-mechanical-planner-smoke",
    "agent:g5-geometry-worker-smoke",
    "agent:g6-segmentation-smoke",
    "agent:g6-material-catalog-smoke",
    "agent:g6-asset-editing-smoke",
    "agent:g6-component-registry-smoke",
    "agent:g7-external-glb-import-smoke",
    "repository:integrity",
    "release:safety-scope",
    "release:secrets-files",
    "release:docs-walkthrough",
    "release:packaging-readiness",
    "release:license-sbom",
    "library:backup",
    "library:verify-backup",
    "library:restore",
]

DOC_REQUIREMENTS: dict[str, list[str]] = {
    "AGENTS.md": [
        "docs/CODEX_HANDOFF.md",
        "docs/DOCUMENTATION_STATUS.md",
        "ActiveDesignSnapshot",
        "FGC-S001",
        "不得跳过 1–4",
        "除非用户明确要求，不提交、不合并、不 push",
    ],
    "README.md": [
        "本机 Alpha",
        "box`/`cylinder",
        "ActiveDesignSnapshot",
        "广泛多客户端并发 E2E",
        "docs/legacy/README.md",
        "docs/DOCUMENTATION_MAP.md",
        "docs/DOCUMENTATION_STATUS.md",
    ],
    "docs/QUICKSTART.md": [
        "script/build_and_run.sh --verify",
        "agent_mode: local-dev-python",
        "agent:g1-kernel-smoke",
        "agent:g7-external-glb-import-smoke",
        "agent:s8-active-design-navigation-smoke",
        "概念图包",
    ],
    "docs/USER_GUIDE.md": [
        "本机 Alpha 当前功能说明",
        "当前不支持自由输入任意尺寸",
        "当前 Agent 资产正式支持的用户导出目标是 GLB",
        "已知限制",
    ],
    "docs/PRODUCT_DEFINITION.md": [
        "当前实现是本机 Alpha",
        "工作台提供四个面向零基础用户的选项",
        "目标状态下",
        "legacy/README.md",
    ],
    "docs/DESIGN.md": [
        "ActiveDesignSnapshot",
        "生产级广泛并发",
        "当前前端已展示 Planner 的三个方向",
        "以下是目标工作流",
    ],
    "docs/IMPLEMENTATION_PLAN.md": [
        "当前状态：部分实现但未退出",
        "完成 [ActiveDesignSnapshot]",
        "未知或含糊输入进入单问题澄清",
        "把 G1–G7 加入 CI",
    ],
    "docs/FRONTEND.md": [
        "PNG/manifest 图包",
        "ActiveDesignSnapshot",
        "正文不低于 12px",
        "当前只提供少量固定比例",
    ],
    "docs/API.md": [
        "POST /api/v1/agent/threads",
        "POST /api/v1/agent/blockouts:commit",
        "POST /api/v1/agent/asset-versions/{asset_version_id}:export",
        "Idempotency-Key",
        "legacy/API_WEAPON_COMPATIBILITY.md",
    ],
    "docs/OPERATIONS.md": [
        "USER_GUIDE.md",
        "DEVELOPMENT.md",
        "ASSET_AUTHORING.md",
        "RELEASE_MAINTENANCE.md",
        "DISASTER_RECOVERY.md",
        "DOCUMENTATION_MAP.md",
        "DOCUMENTATION_STATUS.md",
        "AGENT_GITHUB_REFERENCE_ARCHITECTURE.md",
        "AGENT_PLUGINS_SKILLS_DESIGN.md",
    ],
    "docs/DEVELOPMENT.md": [
        "local-dev-python",
        "desktop:r3-concept-workbench-smoke",
        "contracts:types:generate",
        "Agent-first 路径",
    ],
    "docs/ASSET_AUTHORING.md": [
        "self_declared_original",
        "刘邦",
        "pending_review",
        "assets:formal-review-validate",
    ],
    "docs/RELEASE_MAINTENANCE.md": [
        "packaged-sidecar",
        "工作台核心 Snapshot E2E 当前 Agent-first 路径已通过",
        "release:packaging-readiness",
        "真实 Provider 评测",
    ],
    "docs/AUTHORITATIVE_STATE.md": [
        "ActiveDesignSnapshot@1",
        "asset_version_id",
        "selected_part_id",
        "source_version_id",
        "ACTIVE_DESIGN_STALE",
    ],
    "docs/TEST_STRATEGY.md": [
        "G1–G7",
        "常规 `tests/` 单元测试套件",
        "工作台 E2E",
        "安装与升级",
    ],
    "docs/COMPATIBILITY_MIGRATION.md": [
        "M0：文档和能力冻结",
        "M6：删除 legacy 运行时代码",
        "不原地改写历史 Weapon 数据",
    ],
    "docs/PRODUCTION_RELEASE_CHECKLIST.md": [
        "当前结论：阻断",
        "ActiveDesignSnapshot",
        "sidecar",
        "任何必需项未勾选",
    ],
    "docs/DISASTER_RECOVERY.md": [
        "library:backup",
        "library:verify-backup",
        "library:restore",
        "停止写入",
        "agent_imported_glbs.object_path",
    ],
    "docs/DATABASE.md": [
        "agent_asset_heads",
        "ActiveDesignSnapshot",
        "agent_imported_glbs.object_path",
        "不能保证复制外部导入 GLB 对象",
    ],
    "docs/legacy/README.md": [
        "不是 ForgeCAD 通用机械 Agent 的主产品路径",
        "API_WEAPON_COMPATIBILITY.md",
    ],
    "docs/CODEX_HANDOFF.md": [
        "先刷新，不盲信快照",
        "FGC-S001",
        "desktop:r3-concept-workbench-smoke",
        "release:packaging-readiness",
        "agent_imported_glbs.object_path",
        "DOCUMENTATION_STATUS.md",
    ],
    "docs/DOCUMENTATION_STATUS.md": [
        "当前一句话结论",
        "FGC-R002",
        "能力与阻断账本",
        "FGC-Q002",
        "必跑文档门",
    ],
    "docs/CODEX_EXECUTION_PLAN.md": [
        "S1 ActiveDesignSnapshot",
        "S2 领域澄清",
        "G8 轻量几何扩展",
        "V1 多视图概念渲染",
        "R1 sidecar、恢复、安装和发布",
    ],
    "docs/CODEX_TASK_INDEX.md": [
        "FGC-S001",
        "FGC-D003",
        "FGC-T001",
        "FGC-B001",
        "一次领取一个任务",
        "Next unblocked task IDs",
    ],
    "docs/ADR/0009-active-design-snapshot.md": [
        "ActiveDesignSnapshot@1",
        "legacy_concept_read_only",
        "S008",
    ],
    "docs/CODEX_DEFINITION_OF_DONE.md": [
        "所有任务共同条件",
        "数据库任务",
        "几何任务",
        "前端任务",
        "只要一个必需项失败",
    ],
    "docs/AGENT_CURRENT_ISSUES_AUDIT.md": [
        "活动设计曾有两套状态真值",
        "FGC-S001",
        "recognized | ambiguous | unsupported",
        "主结构已解决，状态措辞仍需持续同步",
    ],
    "docs/DOCUMENTATION_MAP.md": [
        "唯一权威归属",
        "历史证据",
        "已删除",
        "docs/LOCAL_3D_RUNTIME.md",
    ],
    "docs/AGENT_GITHUB_REFERENCE_ARCHITECTURE.md": [
        "OpenAI Codex",
        "OpenCode",
        "Zoo Design Studio",
        "Manifold",
        "glTF-Validator",
        "采用否决门",
    ],
    "docs/AGENT_PLUGINS_SKILLS_DESIGN.md": [
        "Codex 开发插件/Skill",
        "@github",
        "@product-design",
        "产品内 Skill",
        "P0 永远禁止",
        "P0 不安装",
    ],
}

DELETED_DOCS = [
    "design-qa.md",
    "docs/AGENT_FIRST_WORKBENCH.md",
    "docs/BLENDER_AUTHORING_STARTER.md",
    "docs/LOCAL_3D_RUNTIME.md",
    "docs/M1_SKELETON.md",
    "docs/M2_ASSETSTORE.md",
    "docs/M3_COMFYUI_ADAPTER.md",
    "docs/M3_LLM_AND_CONTRACTS.md",
    "docs/M4_PATCH_ASSETSTORE.md",
    "docs/M5_ROUGH3D_PREVIEW.md",
    "docs/PROMPT_QUALITY_SET.md",
    "docs/UNITY_IMPORT_SMOKE.md",
    "workflows/comfyui/README.md",
]

FORBIDDEN_USER_GUIDE_CLAIMS = [
    "主视图显示完整 3D blockout 和三分之四/正面/侧面/顶部概念图",
    "合并选中部件",
    "把选中区域单独拆出",
    "顶栏“撤销”回到上一版本",
    "展示图片      多视图 PNG / 转台图",
    "通用 3D 模型  GLB，可选 OBJ",
]

LEGACY_COMMAND_MARKERS = [
    "npm run r1:create-weapon-gate",
    "WUSHEN_3D_PROVIDER=local_http",
    "npm run unity:import:gate",
    "WUSHEN_COMFYUI_BASE_URL",
]

CURRENT_DOCS_THAT_MUST_NOT_ROUTE_LEGACY = [
    "docs/USER_GUIDE.md",
    "docs/API.md",
    "docs/OPERATIONS.md",
    "docs/QUICKSTART.md",
]


def main() -> int:
    blockers: list[dict[str, Any]] = []
    summaries: dict[str, Any] = {}
    package = _read_json(ROOT / "package.json")
    scripts = package.get("scripts") if isinstance(package.get("scripts"), dict) else {}

    missing_scripts = [name for name in REQUIRED_SCRIPTS if name not in scripts]
    if missing_scripts:
        blockers.append(_blocker("MISSING_NPM_SCRIPT", "Required documentation command is missing.", {"scripts": missing_scripts}))
    summaries["scripts"] = {"required": len(REQUIRED_SCRIPTS), "missing": missing_scripts}

    docs_summary: dict[str, Any] = {}
    for rel_path, phrases in DOC_REQUIREMENTS.items():
        path = ROOT / rel_path
        if not path.is_file():
            blockers.append(_blocker("MISSING_DOC", f"{rel_path} is required."))
            docs_summary[rel_path] = {"exists": False, "missing_phrases": phrases}
            continue
        text = path.read_text(encoding="utf-8")
        missing = [phrase for phrase in phrases if phrase not in text]
        if missing:
            blockers.append(_blocker("DOC_WALKTHROUGH_GAP", f"{rel_path} is incomplete.", {"phrases": missing}))
        docs_summary[rel_path] = {"exists": True, "missing_phrases": missing}
    summaries["docs"] = docs_summary

    restored_deleted_docs = [rel_path for rel_path in DELETED_DOCS if (ROOT / rel_path).exists()]
    if restored_deleted_docs:
        blockers.append(_blocker("REJECTED_DOC_RESTORED", "Rejected or merged documentation returned to the active tree.", {"paths": restored_deleted_docs}))
    summaries["deleted_docs"] = {"expected_absent": DELETED_DOCS, "restored": restored_deleted_docs}

    user_guide = (ROOT / "docs/USER_GUIDE.md").read_text(encoding="utf-8")
    forbidden_claims = [claim for claim in FORBIDDEN_USER_GUIDE_CLAIMS if claim in user_guide]
    if forbidden_claims:
        blockers.append(_blocker("USER_GUIDE_OVERCLAIM", "User guide promises unimplemented operations.", {"claims": forbidden_claims}))
    summaries["user_guide_overclaims"] = forbidden_claims

    legacy_leaks: dict[str, list[str]] = {}
    for rel_path in CURRENT_DOCS_THAT_MUST_NOT_ROUTE_LEGACY:
        text = (ROOT / rel_path).read_text(encoding="utf-8")
        matches = [marker for marker in LEGACY_COMMAND_MARKERS if marker in text]
        if matches:
            legacy_leaks[rel_path] = matches
    if legacy_leaks:
        blockers.append(_blocker("LEGACY_COMMAND_IN_CURRENT_DOC", "Legacy commands leaked into current user/API docs.", legacy_leaks))
    summaries["legacy_command_leaks"] = legacy_leaks

    task_index_report = _check_codex_task_index()
    task_count = task_index_report.pop("task_count", 0)
    if task_index_report:
        blockers.append(_blocker("CODEX_TASK_INDEX_INVALID", "Codex task index is not internally consistent.", task_index_report))
    summaries["codex_task_index"] = {
        "task_count": task_count,
        "issues": task_index_report,
    }

    script_refs: dict[str, list[str]] = {}
    missing_refs: dict[str, list[str]] = {}
    for rel_path in ["README.md", *DOC_REQUIREMENTS]:
        path = ROOT / rel_path
        if not path.is_file():
            continue
        refs = sorted(_extract_npm_script_refs(path.read_text(encoding="utf-8")))
        script_refs[rel_path] = refs
        missing = [name for name in refs if name not in scripts]
        if missing:
            missing_refs[rel_path] = missing
    if missing_refs:
        blockers.append(_blocker("DOC_SCRIPT_REF_MISSING", "Documentation references undefined npm scripts.", missing_refs))
    summaries["script_references"] = {"files": script_refs, "missing": missing_refs}

    report = {"ok": not blockers, "summaries": summaries, "blockers": blockers}
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if not blockers else 1


def _blocker(code: str, message: str, details: Any | None = None) -> dict[str, Any]:
    item: dict[str, Any] = {"severity": "blocker", "code": code, "message": message}
    if details is not None:
        item["details"] = details
    return item


def _extract_npm_script_refs(text: str) -> set[str]:
    return set(re.findall(r"npm run ([A-Za-z0-9:_-]+)", text))


def _check_codex_task_index() -> dict[str, Any]:
    text = (ROOT / "docs/CODEX_TASK_INDEX.md").read_text(encoding="utf-8")
    rows = re.findall(
        r"^\| (FGC-([A-Z][0-9]{3})) \| ([a-z_]+) \| ([^|]+) \|",
        text,
        flags=re.MULTILINE,
    )
    issues: dict[str, Any] = {"task_count": len(rows)}
    ids = [task_id for task_id, _, _, _ in rows]
    duplicates = sorted({task_id for task_id in ids if ids.count(task_id) > 1})
    if duplicates:
        issues["duplicate_ids"] = duplicates
    allowed_statuses = {"ready", "in_progress", "blocked", "external", "done", "superseded"}
    invalid_statuses = sorted({status for _, _, status, _ in rows if status not in allowed_statuses})
    if invalid_statuses:
        issues["invalid_statuses"] = invalid_statuses
    short_statuses = {short_id: status for _, short_id, status, _ in rows}
    short_ids = set(short_statuses)
    missing_dependencies: dict[str, list[str]] = {}
    for task_id, _, status, dependency_cell in rows:
        dependencies = sorted(set(re.findall(r"\b([A-Z][0-9]{3})\b", dependency_cell)))
        missing = [dependency for dependency in dependencies if dependency not in short_ids]
        if missing:
            missing_dependencies[task_id] = missing
        if status == "ready":
            unresolved = [dependency for dependency in dependencies if short_statuses.get(dependency) != "done"]
            if unresolved:
                issues.setdefault("ready_with_unresolved_dependencies", {})[task_id] = unresolved
    if missing_dependencies:
        issues["missing_dependencies"] = missing_dependencies
    actual_ready = {task_id for task_id, _, status, _ in rows if status == "ready"}
    if not actual_ready:
        issues["no_ready_tasks"] = True
    if len(rows) < 40:
        issues["task_count_too_small"] = len(rows)
    return issues


def _read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    raise SystemExit(main())
