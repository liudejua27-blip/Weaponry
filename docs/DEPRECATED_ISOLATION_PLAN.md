# ForgeCAD 废弃文档、代码与模块隔离计划

> **Weaponry P0 override (2026-08-29):** 本文只有在与 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md` 和 ADR-0029 一致时才具有当前执行权。ForgeCAD 在本文中解释为 Weaponry 的 Rust Runtime lineage；当前唯一产品主线是由 Codex 生成、修改、验证并交付高质量穿越火线非功能性游戏武器。通用 3D、机器人和原创科幻示例仅作 fixture/历史能力，不得抢占本月主线。 本文中所有 2026-08-28 及更早的“当前”“下一原子”“唯一 `in_progress`”和工具/Schema 数量语句均按历史 cohort 解释，不得覆盖 `WPN-*` successor queue。

> 2026-08-25 商业质量路线只增加 ForgeCAD-owned typed contracts/executors 的目标，不恢复 archived Provider、旧 workbench、BlenderMCP、任意 Python/JavaScript/shell 或 superseded Skill。研究缓存、教程笔记和候选第三方库均不得进入 active capability count；只有 accepted adoption receipt 才能改变产品依赖。

版本：2026-08-13
状态：隔离规则已更新；本轮已将三个 superseded Skill Bundle 移出 active `bundles/**`，放入 `packages/forgecad-skills/archive/superseded/`

## 1. 隔离目标

让架构、模块和文档边界清晰：

- active 目录只放当前产品能力；
- archive/quarantine 只放历史证据、superseded 模块和迁移材料；
- 废弃材料不能被 registry、manifest、build、runtime、viewer 或 docs walkthrough 当作当前 PASS；
- 所有隔离动作可追溯、可恢复、不误删用户数据。

## 2. 状态定义

| 状态 | 含义 | 允许位置 |
|---|---|---|
| active | 当前产品能力，有 Schema/tool/runtime/evidence | active source/docs/evidence |
| target-design | 目标架构，尚无实现证据 | docs authority/plan |
| superseded | 被新架构替代，仅保留历史 provenance | archive/quarantine |
| rejected | 明确不采用 | docs adoption/archive |
| research-authorized | 用户允许在冻结 revision 下研究精确上游文件；不是 adopted dependency | 不入 Git 的 adoption cache/quarantine + `docs/evidence/adoption/**` receipt |
| private-backup | 可能含用户数据或旧运行状态 | 不入 Git 的私有归档路径 |

## 3. 隔离目录

| 类型 | 位置 | 规则 |
|---|---|---|
| 废弃 evidence | `docs/evidence/archive/**` | 必须标 `SUPERSEDED`，不得被当前 manifest 引用为 PASS |
| 废弃 Skill | `packages/forgecad-skills/archive/**` | 不进入 `registry.json`，不在 `bundles/**`，不被 `skill_list` 暴露 |
| 旧重置备份 | `/tmp/forgecad-mcp001-20260807` 等 | 不入 Git，含用户数据时不得复制到仓库 |
| 外部项目评估 | `docs/evidence/adoption/**` | `research-authorized` 只允许隔离研究缓存；只有 `approval: accepted` 才能进入 lockfile/package |

## 4. 当前隔离清单

| 对象 | 当前处理 | 备注 |
|---|---|---|
| 旧 Provider/Agent/workbench 文档 | 已从 active docs 删除，旧路径由 docs walkthrough 禁止 | 不从 Git 历史恢复为权威 |
| 旧 Provider/App Server/Python Agent/workbench 代码 | MCP010A 已清理或私有隔离 | `scripts/check_repository_integrity.py` 继续检查禁止路径 |
| standalone Host receipts | `docs/evidence/archive/mcp004-standalone-host/**` | 只能解释历史，不能恢复 host |
| `reference-to-typed-plan@0.1.0` | 已移动到 `packages/forgecad-skills/archive/superseded/reference-to-typed-plan/0.1.0` | 不在 active registry；仅保留 superseded provenance |
| `hard-surface-detail@0.1.0` | 本轮移动到 `packages/forgecad-skills/archive/superseded/hard-surface-detail/0.1.0` | 被当前已注册的 `hard-surface-detail@0.2.0` 替代；原 Bundle hash 在 archive manifest 中保留 |
| `uv-pbr@0.1.0` | 本轮移动到 `packages/forgecad-skills/archive/superseded/uv-pbr/0.1.0` | 被当前已注册的 `uv-pbr@0.2.0` 替代；原 Bundle hash 在 archive manifest 中保留 |
| `GeometryProgram@1` / `AppearanceProgram@1` / `RenderSet@1` | legacy-compatible active compatibility | 不是废弃代码；只服务 MCP007-MCP009 历史兼容，不得用于 high-quality 结论 |
| 代码注释或测试中的 `legacy` 字样 | 需要逐项判别 | compatibility/test guard 不等于废弃模块 |

## 5. 新废弃项处理流程

1. 记录候选对象、路径、状态、为什么废弃；
2. 确认它不在 active registry、manifest、tool list、build target 或 runtime import 中；
3. 若含用户数据、绝对路径、原图或大文件，移动到私有备份，不进入 Git；
4. 若需保留 provenance，移动到 archive/quarantine 并保留 README；
5. 更新 `DOCUMENTATION_MAP.md`、`DOCUMENTATION_STATUS.md`、`CODEX_HANDOFF.md` 和相关 checker；
6. 运行 docs/integrity/secrets/license gates；
7. 未跑的门写 `NOT_RUN`，失败写 `FAIL`，不要用删除后的绿色替代证据。

## 6. 禁止

- 在脏工作树里直接删除不明来源文件；
- 把 archive 中的模块重新加入 active registry；
- 让 docs/evidence/archive 的 receipt 计入当前能力；
- 用 “legacy” 搜索结果一刀切删除兼容代码；
- 把外部 GitHub 项目整仓复制到 active product tree；
- 在没有 adoption receipt 时移动第三方代码到 package/installer。
- 将 `research-authorized` 缓存、原样源码或其构建产物提交为 active 模块、Skill、Runtime 或当前 PASS 证据。

## 7. 验证

最小验证：

```bash
python3 scripts/check_repository_integrity.py
python3 scripts/check_mcp006_skills.py
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:secrets-files
git diff --check
```

这些命令只证明隔离边界和文档 walk-through；不能证明视觉质量或发布包。

2026-08-13 审计结论：除已归档的三个 Skill provenance 外，当前活动目录没有可移除的旧 Provider、Agent、workbench、standalone Host、Concept/Weapon/Module 产品模块或废弃文档；`GeometryProgram@1`、`AppearanceProgram@1`、`RenderSet@1` 仍是受约束的 MCP007-MCP009 兼容链，保留在 active source。
