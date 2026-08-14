# ForgeCAD 文档地图

版本：2026-08-13 · 状态：MCP005–MCP009 MVP functional core 已收口；FGC-MCP010A done；FGC-MCP010B blocked/deferred（Darwin OS memory hard cap NOT_RUN）；FGC-MCP010C source-focused PASS_WITH_UNRUN_VISUAL_GATES；FGC-MCP010D/E source-focused PASS；FGC-MCP010F source + current-cohort packaged CLI read-model in_progress（Tauri packaged UI/人评/360 子门 NOT_RUN/BLOCKED）。ADR-0026 / Agentic Design Runtime 已完成 observe/plan projection 与 durable session/checkpoint/RepairIntent prepare/readback source slice；架构模块边界和废弃隔离计划继续作为治理文档。当前为 102 Schema、57 tools，完整 orchestrator/Repair/视觉闭环和 `QUALITY_TARGET_NOT_MET` 事实仍需分层记录。

## 阅读顺序

1. `DOCUMENTATION_STATUS.md`：当前事实和能力标签
2. `CODEX_HANDOFF.md`：本分支证据、命令和剩余阻断
3. `ADR/0025-codex-only-mcp-3d-runtime.md`：产品断代决策
4. `ADR/0026-agentic-design-runtime.md`：Agentic Design Runtime 目标架构；定义 DesignSession、SemanticSceneGraph、ReferenceCanvas、阶段门、Visual Evidence 和 Critic/Repair loop
5. `FORGECAD_AGENTIC_DESIGN_RUNTIME_PLAN.md`：本次大调整的执行规划；包含已实现的 projection 与 durable prepare/readback slice 和未完成的 orchestrator/Repair backlog
6. `ARCHITECTURE_MODULE_BOUNDARY.md`：当前模块权责、目标模块和 active/archive 目录边界
7. `DEPRECATED_ISOLATION_PLAN.md`：废弃文档、代码、模块的隔离位置、状态定义和验证流程
8. `RESET_MIGRATION_PLAN.md`：删除/迁移/升级清单
9. `CODEX_EXECUTION_PLAN.md`：阶段与退出门
10. `CODEX_TASK_INDEX.md`：Luna 唯一任务队列
11. `MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`：MCP010A–F 的唯一质量升级执行合同
12. `CODEX_GEOMETRY_V2_WORKFLOW.md`：MCP010B 期间 Codex/Luna 发现 live OperatorCatalog、构造 GeometryProgram@2、判读 ArtifactReadback@2、固定同级 Worker 隔离证据和 V1 过渡的受限工作流
13. `CODEX_SINGLE_REFERENCE_OPERATING_GUIDE.md`：Codex 使用单张授权参考图的可执行调用顺序、停止条件、失败映射和 C–F 交接边界
14. `CODEX_REFERENCE_DETAIL_INVENTORY.md`：Codex 单图参考的授权 intake、可见/推断/未知细节清单、分阶段修正队列、质量合同和停止规则；它是编排模板，不是 Runtime 合同
15. `CODEX_SILHOUETTE_FIT_WORKFLOW.md`：轮廓目标、相机搜索、边界误差和 Codex/Luna 单 Part 修正纪律；当前 source slice 已接入 Runtime/MCP，但真实用户 likeness 仍需独立证据
16. `MCP010C_READINESS_AUDIT.md`：固定渲染/参考比较的当前实现审计、C1–C4 证据、未运行视觉门和 Luna 检查清单；当前 C source Gate 已通过但不代表真实用户 likeness 或 packaged/live 完成
17. `AUTHORITATIVE_STATE.md`：Runtime 数据真值
18. `MVP_DELIVERY_PLAN.md`：MVP 范围、MCP005–009 退出门、工具采用决策和当前证据边界
19. `LUNA_GOAL_EXECUTION_GUIDE.md`：Goal 执行协议、当前可调用工具和真实 host 验收动作
20. `LUNA_GITHUB_REPLICATION_PLAYBOOK.md`：Luna 研究 build123d、BlenderMCP、CadQuery、Manifold、MaterialX 的冻结 revision、选择性源文件复刻、quarantine、审查和接受流程
21. `EXTERNAL_PROJECT_ADOPTION.md`：第三方采用状态、research receipt 和 accepted 入口
22. `CODEX_PONYTAIL_PREFLIGHT_WORKFLOW.md`：Codex 经 MCP 进入 3D 设计前必须读取的 first-party preflight Skill、会话顺序、边界和维护规则
23. 任务相关合同：`MCP_RUNTIME_CONTRACT.md`、`CODEX_INTEGRATION.md`、`COMPILER_PIPELINE.md`、`WORKBENCH_VIEWER.md`、`SKILL_PACKAGE_STANDARD.md`、`SCHEMAS.md`、`DATABASE.md`
24. `MVP_ARCHITECTURE.md`：单用户启动、文件锁和最小运行边界
25. `MVP_TOOL_CATALOG.md`：当前源码的 35 个只读/21 个写工具（56 个，写工具仍需显式 opt-in），12 个 Skill（含必须先读取的 `ponytail-preflight@0.1.0`，以及 active `primitive-blockout@0.2.0`、`hard-surface-detail@0.2.0` 与 `uv-pbr@0.2.0`）；新增轮廓 target/camera/Rig/Rig-hash/SDF/Part/candidate compare、Agentic observe/plan/critic/evidence projection 和 durable session/checkpoint readback 工具的调用顺序见相关合同。C/D/E/F 的结构 Gate、真实 likeness、人评/PBR/纹理和 360 仍必须另标 planned/unavailable
## 生命周期

- `已实现`：当前代码和对应 Gate 通过；
- `部分实现`：已实现与缺口必须分开；
- `目标设计`：没有当前代码证据；
- `迁移中`：旧代码已删除，新能力尚未完成；
- `blocked`：退出条件因环境、授权或外部事实失败；
- `superseded`：不再属于当前产品。

目标设计不能覆盖事实，历史 Git 内容不能证明当前能力。`scene_observe_get` 等 projection 已有 source/runtime/MCP/Viewer 证据，`session_create_or_resume`、`session_get`、`checkpoint_prepare`、`checkpoint_get`、`checkpoint_restore_prepare` 已有 durable prepare/readback 与重启 receipt；后者仍不能等同于单动作 orchestrator、Critic/Repair 执行或完整 schema conformance。每个任务结束必须同步状态账本、任务索引、能力矩阵、handoff 和受影响合同；用户指南只能写已实现或当前 Viewer 能力。`functional-core PASS` 不能升级成 `high-quality/reference PASS`。

## 当前权威文件

产品/决策：`PRODUCT_DEFINITION.md`、`ADR/0025-codex-only-mcp-3d-runtime.md`、`ADR/0026-agentic-design-runtime.md`。

架构/合同：`DESIGN.md`、`MVP_ARCHITECTURE.md`、`ARCHITECTURE_MODULE_BOUNDARY.md`、`AUTHORITATIVE_STATE.md`、`MCP_RUNTIME_CONTRACT.md`、`CODEX_INTEGRATION.md`、`COMPILER_PIPELINE.md`、`WORKBENCH_VIEWER.md`、`SKILL_PACKAGE_STANDARD.md`、`SCHEMAS.md`、`DATABASE.md`；MCP003 快照和宿主矩阵位于 `evidence/mcp003/`。

执行/质量：`RESET_MIGRATION_PLAN.md`、`MVP_DELIVERY_PLAN.md`、`MVP_TOOL_CATALOG.md`、`CODEX_PONYTAIL_PREFLIGHT_WORKFLOW.md`、`FORGECAD_AGENTIC_DESIGN_RUNTIME_PLAN.md`、`MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`、`CODEX_GEOMETRY_V2_WORKFLOW.md`、`CODEX_REFERENCE_DETAIL_INVENTORY.md`、`CODEX_SINGLE_REFERENCE_OPERATING_GUIDE.md`、`CODEX_EXECUTION_PLAN.md`、`CODEX_TASK_INDEX.md`、`LUNA_GOAL_EXECUTION_GUIDE.md`、`LUNA_GITHUB_REPLICATION_PLAYBOOK.md`、`CODEX_DEFINITION_OF_DONE.md`、`TEST_STRATEGY.md`、`evidence/CAPABILITY_GATE_MATRIX.md`。

运维/供应链：`DEVELOPMENT.md`、`OPERATIONS.md`、`PACKAGING.md`、`PRODUCTION_RELEASE_CHECKLIST.md`、`RELEASE_MAINTENANCE.md`、`DISASTER_RECOVERY.md`、`THIRD_PARTY_LICENSES.md`、`EXTERNAL_PROJECT_ADOPTION.md`、`LUNA_GITHUB_REPLICATION_PLAYBOOK.md`、`DEPRECATED_ISOLATION_PLAN.md`。

当前树已删除旧 Provider、App Server、Python Agent、standalone Host、旧工作台、旧 Concept/Weapon/Module 产品入口、合同和评估。恢复材料只存在于受控 reset/cleanup 归档和 Git 历史；少量解释架构迁移所需的历史 receipt 只能以 `SUPERSEDED` 状态放在 `evidence/archive/`，不能由当前 manifest 引用为 PASS，也不得重新链接或恢复为产品入口。
