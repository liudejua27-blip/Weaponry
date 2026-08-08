# ForgeCAD 文档地图

版本：2026-08-08 · 状态：MCP003 Codex-only MCP 只读层进行中，分支唯一权威链

## 阅读顺序

1. `DOCUMENTATION_STATUS.md`：当前事实和能力标签
2. `CODEX_HANDOFF.md`：本分支证据、命令和剩余阻断
3. `ADR/0025-codex-only-mcp-3d-runtime.md`：产品断代决策
4. `RESET_MIGRATION_PLAN.md`：删除/迁移/升级清单
5. `CODEX_EXECUTION_PLAN.md`：阶段与退出门
6. `CODEX_TASK_INDEX.md`：Luna 唯一任务队列
7. `AUTHORITATIVE_STATE.md`：Runtime 数据真值
8. `LUNA_GOAL_EXECUTION_GUIDE.md`：执行协议
9. 任务相关合同：`MCP_RUNTIME_CONTRACT.md`、`CODEX_INTEGRATION.md`、`COMPILER_PIPELINE.md`、`WORKBENCH_VIEWER.md`、`SKILL_PACKAGE_STANDARD.md`、`SCHEMAS.md`、`DATABASE.md`

## 生命周期

- `已实现`：当前代码和对应 Gate 通过；
- `部分实现`：已实现与缺口必须分开；
- `目标设计`：没有当前代码证据；
- `迁移中`：旧代码已删除，新能力尚未完成；
- `blocked`：退出条件因环境、授权或外部事实失败；
- `superseded`：不再属于当前产品。

目标设计不能覆盖事实，历史 Git 内容不能证明当前能力。每个任务结束必须同步状态账本、任务索引、能力矩阵、handoff 和受影响合同；用户指南只能写已实现或当前 Viewer 能力。

## 当前权威文件

产品/决策：`PRODUCT_DEFINITION.md`、`ADR/0025-codex-only-mcp-3d-runtime.md`。

架构/合同：`DESIGN.md`、`AUTHORITATIVE_STATE.md`、`MCP_RUNTIME_CONTRACT.md`、`CODEX_INTEGRATION.md`、`COMPILER_PIPELINE.md`、`WORKBENCH_VIEWER.md`、`SKILL_PACKAGE_STANDARD.md`、`SCHEMAS.md`、`DATABASE.md`；MCP003 快照和宿主矩阵位于 `evidence/mcp003/`。

执行/质量：`RESET_MIGRATION_PLAN.md`、`CODEX_EXECUTION_PLAN.md`、`CODEX_TASK_INDEX.md`、`LUNA_GOAL_EXECUTION_GUIDE.md`、`CODEX_DEFINITION_OF_DONE.md`、`TEST_STRATEGY.md`、`evidence/CAPABILITY_GATE_MATRIX.md`。

运维/供应链：`DEVELOPMENT.md`、`OPERATIONS.md`、`PACKAGING.md`、`PRODUCTION_RELEASE_CHECKLIST.md`、`RELEASE_MAINTENANCE.md`、`DISASTER_RECOVERY.md`、`THIRD_PARTY_LICENSES.md`、`EXTERNAL_PROJECT_ADOPTION.md`。

当前树已删除旧 Provider、App Server、Python Agent、旧工作台、旧 Concept/Weapon/Module 合同及其历史证据；这些内容只存在于 reset 归档和 Git 历史，不得重新链接或恢复为产品入口。
