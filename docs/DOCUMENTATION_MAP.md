# ForgeCAD 文档地图与生命周期

版本：2026-07-17
状态：文档唯一入口

本文件回答三个问题：先读什么、哪份文档拥有哪类真相、哪些内容只是历史记录。后续 Codex 不得通过文件名猜测权威性。

## 1. 按读者进入

| 读者 | 必读入口 | 继续阅读 |
| --- | --- | --- |
| 零基础测试用户 | [QUICKSTART](QUICKSTART.md)、[USER_GUIDE](USER_GUIDE.md) | [OPERATIONS](OPERATIONS.md) |
| 产品与设计 | [PRODUCT_DEFINITION](PRODUCT_DEFINITION.md)、[FRONTEND](FRONTEND.md) | [MECHANICAL_DESIGN_OPERATIONS](MECHANICAL_DESIGN_OPERATIONS.md)、[DOMAIN_PACKS](DOMAIN_PACKS.md)、[MATERIAL_SYSTEM](MATERIAL_SYSTEM.md) |
| 后端与合同开发 | [CODEX_HANDOFF](CODEX_HANDOFF.md)、[AUTHORITATIVE_STATE](AUTHORITATIVE_STATE.md) | [DESIGN](DESIGN.md)、[API](API.md)、[SCHEMAS](SCHEMAS.md)、[DATABASE](DATABASE.md) |
| 前端开发 | [CODEX_HANDOFF](CODEX_HANDOFF.md)、[FRONTEND](FRONTEND.md) | [TEST_STRATEGY](TEST_STRATEGY.md)、[DEVELOPMENT](DEVELOPMENT.md) |
| 资产作者 | [ASSET_AUTHORING](ASSET_AUTHORING.md) | [MODULE_ASSET_GUIDE](MODULE_ASSET_GUIDE.md)、[MODULE_NAMING_STANDARD](MODULE_NAMING_STANDARD.md) |
| 发布维护 | [RELEASE_MAINTENANCE](RELEASE_MAINTENANCE.md) | [PRODUCTION_RELEASE_CHECKLIST](PRODUCTION_RELEASE_CHECKLIST.md)、[PACKAGING](PACKAGING.md)、[DISASTER_RECOVERY](DISASTER_RECOVERY.md) |
| 后续 Codex | [AGENTS](../AGENTS.md)、[CODEX_HANDOFF](CODEX_HANDOFF.md)、[DOCUMENTATION_STATUS](DOCUMENTATION_STATUS.md) | [CODEX_EXECUTION_PLAN](CODEX_EXECUTION_PLAN.md)、[CODEX_TASK_INDEX](CODEX_TASK_INDEX.md)、[CODEX_DEFINITION_OF_DONE](CODEX_DEFINITION_OF_DONE.md) |

## 2. 唯一权威归属

| 主题 | 唯一权威文档 | 不得替代它的材料 |
| --- | --- | --- |
| 产品范围、四领域和非目标 | [PRODUCT_DEFINITION](PRODUCT_DEFINITION.md) | 截图、旧 Weapon 文档、计划草稿 |
| 当前用户可用功能 | [USER_GUIDE](USER_GUIDE.md) | DESIGN 的目标能力、历史 evidence |
| 目标系统架构 | [DESIGN](DESIGN.md) | GitHub 参考项目自身架构 |
| 目标 3D 机械设计操作流程 | [MECHANICAL_DESIGN_OPERATIONS](MECHANICAL_DESIGN_OPERATIONS.md) | 当前 USER_GUIDE、HTML/SVG demo、聊天建议 |
| Design Surface Compiler 分层、编译边界与实施顺序 | [ADR-0016](ADR/0016-design-surface-compiler.md) | HTML/CSS 折面 demo、单次概念图、旧多候选方案 |
| Project/Version/Selection/Quality/Export 真值 | [AUTHORITATIVE_STATE](AUTHORITATIVE_STATE.md) | 前端 localStorage、旧 Concept hook |
| 当前桌面 app-server 协议与 Agent HTTP compatibility API | [API](API.md)、[ADR-0014](ADR/0014-rust-first-codex-app-server.md) | legacy API、生成类型文件、仅有目标架构的旧说明 |
| 真实 Provider 四领域评测合同 | [AGENT_PROVIDER_EVALUATION](AGENT_PROVIDER_EVALUATION.md) | legacy R4/Weapon 评测、离线 smoke |
| 实施顺序 | [CODEX_EXECUTION_PLAN](CODEX_EXECUTION_PLAN.md) | 旧里程碑证据 |
| 原子任务状态 | [CODEX_TASK_INDEX](CODEX_TASK_INDEX.md) | 聊天中的口头进度 |
| 文档当前状态、标签和同步规则 | [DOCUMENTATION_STATUS](DOCUMENTATION_STATUS.md) | 单次 smoke、旧 handoff、未重跑 evidence |
| 测试与生产退出条件 | [TEST_STRATEGY](TEST_STRATEGY.md)、[PRODUCTION_RELEASE_CHECKLIST](PRODUCTION_RELEASE_CHECKLIST.md) | 单次 smoke 结果 |
| 生产概念工件与视觉验收拆分 | [ADR-0015](ADR/0015-split-production-artifact-and-visual-acceptance.md)、[CODEX_TASK_INDEX](CODEX_TASK_INDEX.md) 的 M108A/M108B 任务卡 | 旧 M108 状态、固定 showcase 截图、单次高分辨率 GLB |
| M108B 独立视觉评分操作 | [M108_VISUAL_BENCHMARK_PROTOCOL](evidence/M108_VISUAL_BENCHMARK_PROTOCOL.md) | Codex 代理评分、自动截图、M108A production profile 名称 |
| 开源参考与采用边界 | [AGENT_GITHUB_REFERENCE_ARCHITECTURE](AGENT_GITHUB_REFERENCE_ARCHITECTURE.md) | 仓库 star、营销页 |
| 开发插件、Skill 与产品内 Skill | [AGENT_PLUGINS_SKILLS_DESIGN](AGENT_PLUGINS_SKILLS_DESIGN.md) | 用户安装包内的插件市场 |

## 3. 生命周期分类

### 当前权威

根 README、上表所列产品/架构/操作/执行文档，以及状态为 Accepted 且未被后续 ADR 取代的当前决策；本轮 M108 拆分以 ADR-0015 为准。修改功能时必须同步更新相关当前权威文档。

### 历史决策

`docs/ADR/` 保存为什么改变方向。已被取代的 ADR 继续保留，但必须标记 `Superseded`，不能作为当前范围。

### 历史证据

`docs/evidence/` 保存过去命令、fixture 和审计结果。它们不可单独承诺当前能力，也不得进入零基础用户默认阅读路径。`M108_VISUAL_BENCHMARK_PROTOCOL.md` 是 M108B 的受控人工操作协议，但只有 Recipe-backed 正式 kit 和有效真人评分才构成退出证据；当前固定 showcase kit 只是 preflight。新的当前能力要同时出现在能力—Gate 矩阵和当前测试结果中。

### 兼容资料

`docs/legacy/` 只描述迁移期间仍存在的旧 Weapon/Concept 数据和入口。主文档不得复制 legacy 命令；需要调试时由迁移维护者显式进入。

### 已删除

以下文档已从工作树删除，因为其内容被合并或路线已被否决；历史仍可通过 Git 查看：

- `docs/M1_SKELETON.md`、`docs/M2_ASSETSTORE.md`：旧阶段快照，现状由 README、CODEX_HANDOFF 和能力矩阵承载；
- `docs/M3_COMFYUI_ADAPTER.md`、`docs/M3_LLM_AND_CONTRACTS.md`、`docs/M4_PATCH_ASSETSTORE.md`、`docs/M5_ROUGH3D_PREVIEW.md`：上一代 Weapon/ComfyUI/神经 3D 里程碑，不是当前 Agent 计划；
- `docs/PROMPT_QUALITY_SET.md`：旧 Weapon prompt 集，四领域 truth set 将按 TEST_STRATEGY 重建；
- `docs/AGENT_FIRST_WORKBENCH.md`：已合并到 FRONTEND、PRODUCT_DEFINITION 和 USER_GUIDE；
- `docs/BLENDER_AUTHORING_STARTER.md`：零基础主路径不依赖 Blender，专业资产流程由 ASSET_AUTHORING 和 MODULE_ASSET_GUIDE 承载；
- `docs/LOCAL_3D_RUNTIME.md`：本地 TripoSR/SF3D/Hunyuan 路线已否决；
- `docs/UNITY_IMPORT_SMOKE.md`：Unity 不再是通用机械 Agent 的产品交付路径；
- `docs/M3_DESKTOP_SUPERVISOR.md`：开发 supervisor 的运行说明已并入 DEVELOPMENT、PACKAGING 与 RELEASE_MAINTENANCE，避免同一 sidecar 合同出现两份互相漂移的描述；
- `design-qa.md`：旧 Weapon/TripoSR 截图审查，不是当前工作台 QA。
- `workflows/comfyui/README.md`：旧外部 ComfyUI 操作说明；实现仍待兼容迁移，但不再提供产品级操作路径。

## 4. 文档维护规则

1. 新文档创建前先确认现有权威文档不能容纳该内容；
2. 同一事实只保留一个权威定义，其他文档使用链接；
3. 用户指南只写当前通过验证的功能；目标能力必须明确标为目标；
4. 外部开源项目只能作为参考或候选依赖，采用前必须记录版本、许可证、体积、平台和退出方案；
5. 真实 Provider 评测的 fixture、预算、授权、脱敏记录和通过口径只能在 `AGENT_PROVIDER_EVALUATION.md` 与其引用的 evaluation contract 定义；
6. 删除文档时同步修复 README、OPERATIONS、legacy、Gate 和相对链接；
7. 运行 `npm run release:docs-walkthrough`、`npm run repository:integrity` 和 `git diff --check` 后才能交接。
