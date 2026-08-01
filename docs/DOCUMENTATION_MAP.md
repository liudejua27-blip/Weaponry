# ForgeCAD 文档地图

版本：2026-08-01
状态：当前文档唯一入口

2026-07-29 的 U001A 已删除被取代的阶段报告、旧操作说明和重复决策；2026-08-01 当前树为 53 份 Markdown，新增一份 U004 第一阶段执行总图，继续保留 0 份独立 legacy 手册。历史原文由 Git 保存，不再与当前产品文档并列。

## 1. 按读者进入

| 读者 | 先读 | 再读 |
| --- | --- | --- |
| 测试用户 | [QUICKSTART](QUICKSTART.md)、[USER_GUIDE](USER_GUIDE.md) | [OPERATIONS](OPERATIONS.md) |
| 产品/设计 | [PRODUCT_DEFINITION](PRODUCT_DEFINITION.md)、[AGENT_CURRENT_ISSUES_AUDIT](AGENT_CURRENT_ISSUES_AUDIT.md) | [U004 第一阶段高质量工作台总图](U004_STAGE1_HIGH_QUALITY_WORKBENCH_PLAN.md)、[DESIGN](DESIGN.md)、[FRONTEND](FRONTEND.md) |
| 后端/合同 | [CODEX_HANDOFF](CODEX_HANDOFF.md)、[AUTHORITATIVE_STATE](AUTHORITATIVE_STATE.md) | [API](API.md)、[SCHEMAS](SCHEMAS.md)、[DATABASE](DATABASE.md) |
| 前端 | [CODEX_HANDOFF](CODEX_HANDOFF.md)、[FRONTEND](FRONTEND.md) | [TEST_STRATEGY](TEST_STRATEGY.md) |
| 资产/材质 | [ASSET_AUTHORING](ASSET_AUTHORING.md)、[MATERIAL_SYSTEM](MATERIAL_SYSTEM.md) | [MODULE_ASSET_GUIDE](MODULE_ASSET_GUIDE.md) |
| 发布维护 | [RELEASE_MAINTENANCE](RELEASE_MAINTENANCE.md) | [PRODUCTION_RELEASE_CHECKLIST](PRODUCTION_RELEASE_CHECKLIST.md)、[PACKAGING](PACKAGING.md)、[DISASTER_RECOVERY](DISASTER_RECOVERY.md) |
| Codex/Luna | [AGENTS](../AGENTS.md)、[DOCUMENTATION_STATUS](DOCUMENTATION_STATUS.md)、[CODEX_HANDOFF](CODEX_HANDOFF.md) | [U004 第一阶段高质量工作台总图](U004_STAGE1_HIGH_QUALITY_WORKBENCH_PLAN.md)、[CODEX_EXECUTION_PLAN](CODEX_EXECUTION_PLAN.md)、[CODEX_TASK_INDEX](CODEX_TASK_INDEX.md)、[LUNA_GOAL_EXECUTION_GUIDE](LUNA_GOAL_EXECUTION_GUIDE.md) |

## 2. 唯一权威归属

| 主题 | 唯一权威 |
| --- | --- |
| 产品范围、类别开放和非目标 | [PRODUCT_DEFINITION](PRODUCT_DEFINITION.md)、[ADR-0022](ADR/0022-universal-reference-conditioned-3d-agent.md) |
| AI Provider 主权边界 | [ADR-0023](ADR/0023-deepseek-qwen-only-ai-provider-policy.md) |
| 当前用户能力 | [USER_GUIDE](USER_GUIDE.md) |
| 当前状态和阻断 | [DOCUMENTATION_STATUS](DOCUMENTATION_STATUS.md) |
| 当前任务 | [CODEX_TASK_INDEX](CODEX_TASK_INDEX.md) |
| U004 第一阶段产品链、DeepSeek 会话/缓存、工作台目标与四 Luna 分工 | [U004_STAGE1_HIGH_QUALITY_WORKBENCH_PLAN](U004_STAGE1_HIGH_QUALITY_WORKBENCH_PLAN.md) |
| 当前工作区和验证 | [CODEX_HANDOFF](CODEX_HANDOFF.md) |
| 目标架构 | [DESIGN](DESIGN.md) 与 [当前 ADR 索引](ADR/README.md) |
| 版本、选择、质量和导出真值 | [AUTHORITATIVE_STATE](AUTHORITATIVE_STATE.md) |
| API/Schema/数据库 | [API](API.md)、[SCHEMAS](SCHEMAS.md)、[DATABASE](DATABASE.md) |
| 测试、真人门和 Provider 授权 | [TEST_STRATEGY](TEST_STRATEGY.md)、[AGENT_PROVIDER_EVALUATION](AGENT_PROVIDER_EVALUATION.md) |
| 能力与 Gate | [CAPABILITY_GATE_MATRIX](evidence/CAPABILITY_GATE_MATRIX.md) |
| Weapon/Concept 兼容 | [COMPATIBILITY_MIGRATION](COMPATIBILITY_MIGRATION.md) |
| 发布 | [PRODUCTION_RELEASE_CHECKLIST](PRODUCTION_RELEASE_CHECKLIST.md)、[RELEASE_MAINTENANCE](RELEASE_MAINTENANCE.md) |
| 开源参考与采用边界 | [AGENT_GITHUB_REFERENCE_ARCHITECTURE](AGENT_GITHUB_REFERENCE_ARCHITECTURE.md) |

## 3. 生命周期

### 当前权威

根 README、产品/用户/设计/API/Schema/操作/测试/发布文档、任务与 handoff，以及 `ADR/README.md` 列出的有效决策。修改能力时同步更新用户指南、状态账本和能力矩阵。

### 当前证据

`docs/evidence/` 只保留能力矩阵、F026 视觉规格和冻结概念图。新证据优先由可重复 Gate 生成到 `output/`，再把结论和命令摘要写入能力矩阵；不再为每个阶段创建一份 Markdown 报告。

### 历史证据

R0–R6、旧 Weapon/Unity/Planner、M108 阶段报告和旧 readiness audit 已从当前树删除。Git 历史可追溯原文，但历史 PASS 不能承诺当前能力或当前脏工作区。

### 兼容资料

旧 Weapon/Concept API、表、命令和退出顺序已合并到 `COMPATIBILITY_MIGRATION.md`。不再维护 `docs/legacy/`，也不得恢复独立旧产品操作手册。

### 已删除

除上述历史文档外，已拒绝或合并的 `docs/LOCAL_3D_RUNTIME.md`、ComfyUI、Unity、旧 M1–M5、旧 prompt set、旧 Blender starter 和 `design-qa.md` 继续保持缺失。详细文件名由文档 walkthrough Gate 冻结。

## 4. 维护规则

1. 同一事实只保留一个权威定义，其他文档使用链接；
2. 用户指南只写当前 Gate 支持的能力；
3. 目标合同明确写 `尚未实现`，历史结果明确写 `historical/regression`；
4. 旧文档清理后不得因排查方便恢复到主树；需要原文时使用 Git；
5. 外部项目采用前记录版本、许可证、体积、平台、成本和退出方案；
6. 完成文档变更前运行 `npm run release:docs-walkthrough`、`npm run repository:integrity`、安全/密钥 Gate 和 `git diff --check`。
