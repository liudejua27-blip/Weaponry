# ForgeCAD MVP 开发快速开始

> **Weaponry P0 override (2026-08-29):** 本文只有在与 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md` 和 ADR-0029 一致时才具有当前执行权。ForgeCAD 在本文中解释为 Weaponry 的 Rust Runtime lineage；当前唯一产品主线是由 Codex 生成、修改、验证并交付高质量穿越火线非功能性游戏武器。通用 3D、机器人和原创科幻示例仅作 fixture/历史能力，不得抢占本月主线。 本文中所有 2026-08-28 及更早的“当前”“下一原子”“唯一 `in_progress`”和工具/Schema 数量语句均按历史 cohort 解释，不得覆盖 `WPN-*` successor queue。

开始开发前先确认当前任务为 `WPN-AUTH-001` 或其 successor，并使用原创/仓库 fixture；不得把
合作方私有资产、绝对路径或原图字节放入命令、日志、git 或文档。运行成功只报告 source、
runtime、package、visual、human 和 release 中实际通过的层级。

版本：2026-08-09

## 普通用户

当前 MCP001–009 已完成单用户事务、真实 Codex image attachment/PNG/JPEG reference admission、first-party Skill Bundle、有界 geometry/GLB、UV/tangent/PBR、固定 render、GLB Viewer、limited quality、stable-Part change、immutable version/restore 和 CAS-backed mvp-glb receipt；真实 Codex CLI 十二调用 host golden path 已 PASS，MCP010A Dev.app 已通过第二次 Desktop 激活 Gate。工具、Skill 和 GitHub 候选的当前清单见 `MVP_TOOL_CATALOG.md`。像素级参考比较、真人评分、完整 Desktop 3D write 和 packaged release 仍单独标记。正式可用条件见 `MVP_DELIVERY_PLAN.md`、`USER_GUIDE.md` 和 `DOCUMENTATION_STATUS.md`。

## Luna / 开发者

1. 进入仓库 `/Users/liuchongjiang/Documents/武神`；
2. 完整阅读根 `AGENTS.md`；
3. 按 `DOCUMENTATION_MAP.md` 的顺序阅读权威链；
4. 运行只读基线：

```bash
git status -sb
git diff --check
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run release:license-sbom
npm run mvp:functional-core
npm run desktop:typecheck
npm run desktop:build
npm run desktop:tauri-check
```

5. 阅读 `MVP_DELIVERY_PLAN.md`、`MVP_TOOL_CATALOG.md` 和 `LUNA_GOAL_EXECUTION_GUIDE.md`；
6. 如果要继续产品化，按任务索引领取 `FGC-MCP010`；如果要复核模型链路，运行带 `--image` 的真实 Codex host probe，不重新实现 MCP008/009；
7. 不清理未跟踪文件，不安装 BlenderMCP/Python CAD/远程 Provider，不引入 heartbeat；
8. 任何新的破坏性操作都必须建立恢复门，且不得删除 `WushenForgeLibrary`。

## 当前开发起点

严格按 `CODEX_TASK_INDEX.md`：MCP005 → MCP006 → MCP007 → MCP008 → MCP009 已是 MVP host golden path 完成线；visual/human/packaged gates 继续单独记录，MCP010–013 是可选产品化/发布线。

不要启动旧 Provider、端口 8000、FastAPI Agent 或旧工作台来验证新方向。
