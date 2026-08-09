# ForgeCAD 文档地图

版本：2026-08-09 · 状态：MCP005–MCP009 MVP functional core 已收口；FGC-MCP010A 权威重排/开发激活进行中

## 阅读顺序

1. `DOCUMENTATION_STATUS.md`：当前事实和能力标签
2. `CODEX_HANDOFF.md`：本分支证据、命令和剩余阻断
3. `ADR/0025-codex-only-mcp-3d-runtime.md`：产品断代决策
4. `RESET_MIGRATION_PLAN.md`：删除/迁移/升级清单
5. `CODEX_EXECUTION_PLAN.md`：阶段与退出门
6. `CODEX_TASK_INDEX.md`：Luna 唯一任务队列
7. `MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`：MCP010A–F 的唯一质量升级执行合同
8. `AUTHORITATIVE_STATE.md`：Runtime 数据真值
9. `MVP_DELIVERY_PLAN.md`：MVP 范围、MCP005–009 退出门、工具采用决策和当前证据边界
10. `LUNA_GOAL_EXECUTION_GUIDE.md`：Goal 执行协议、当前可调用工具和真实 host 验收动作
11. 任务相关合同：`MCP_RUNTIME_CONTRACT.md`、`CODEX_INTEGRATION.md`、`COMPILER_PIPELINE.md`、`WORKBENCH_VIEWER.md`、`SKILL_PACKAGE_STANDARD.md`、`SCHEMAS.md`、`DATABASE.md`
12. `MVP_ARCHITECTURE.md`：单用户启动、文件锁和最小运行边界
13. `MVP_TOOL_CATALOG.md`：当前 17 个只读/13 个写工具、10 个 Skill、调用顺序和 GitHub 采用门；MCP010 目标能力必须另标 planned/unavailable

## 生命周期

- `已实现`：当前代码和对应 Gate 通过；
- `部分实现`：已实现与缺口必须分开；
- `目标设计`：没有当前代码证据；
- `迁移中`：旧代码已删除，新能力尚未完成；
- `blocked`：退出条件因环境、授权或外部事实失败；
- `superseded`：不再属于当前产品。

目标设计不能覆盖事实，历史 Git 内容不能证明当前能力。每个任务结束必须同步状态账本、任务索引、能力矩阵、handoff 和受影响合同；用户指南只能写已实现或当前 Viewer 能力。`functional-core PASS` 不能升级成 `high-quality/reference PASS`。

## 当前权威文件

产品/决策：`PRODUCT_DEFINITION.md`、`ADR/0025-codex-only-mcp-3d-runtime.md`。

架构/合同：`DESIGN.md`、`MVP_ARCHITECTURE.md`、`AUTHORITATIVE_STATE.md`、`MCP_RUNTIME_CONTRACT.md`、`CODEX_INTEGRATION.md`、`COMPILER_PIPELINE.md`、`WORKBENCH_VIEWER.md`、`SKILL_PACKAGE_STANDARD.md`、`SCHEMAS.md`、`DATABASE.md`；MCP003 快照和宿主矩阵位于 `evidence/mcp003/`。

执行/质量：`RESET_MIGRATION_PLAN.md`、`MVP_DELIVERY_PLAN.md`、`MVP_TOOL_CATALOG.md`、`MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`、`CODEX_EXECUTION_PLAN.md`、`CODEX_TASK_INDEX.md`、`LUNA_GOAL_EXECUTION_GUIDE.md`、`CODEX_DEFINITION_OF_DONE.md`、`TEST_STRATEGY.md`、`evidence/CAPABILITY_GATE_MATRIX.md`。

运维/供应链：`DEVELOPMENT.md`、`OPERATIONS.md`、`PACKAGING.md`、`PRODUCTION_RELEASE_CHECKLIST.md`、`RELEASE_MAINTENANCE.md`、`DISASTER_RECOVERY.md`、`THIRD_PARTY_LICENSES.md`、`EXTERNAL_PROJECT_ADOPTION.md`。

当前树已删除旧 Provider、App Server、Python Agent、standalone Host、旧工作台、旧 Concept/Weapon/Module 产品入口、合同和评估。恢复材料只存在于受控 reset/cleanup 归档和 Git 历史；少量解释架构迁移所需的历史 receipt 只能以 `SUPERSEDED` 状态放在 `evidence/archive/`，不能由当前 manifest 引用为 PASS，也不得重新链接或恢复为产品入口。
