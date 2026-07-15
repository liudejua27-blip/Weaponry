# ForgeCAD Codex 原子任务索引

版本：2026-07-15
用途：后续 Codex 一次领取一个任务并产生可验证交付

## 1. 状态定义

- `ready`：依赖已满足，可以开始；
- `in_progress`：当前唯一正在实施的原子任务；
- `blocked`：必须先完成依赖任务；
- `external`：需要真实 Provider 费用、独立 reviewer、签名账户或测试设备；
- `done`：退出条件和 Gate 全部通过；
- `superseded`：被新的任务/ADR 显式取代。

更新状态时必须附证据日期、commit 或工作区说明。不能仅把表格改成 `done`。

## 2. P0 单一状态真值

| ID | 状态 | 依赖 | 交付 | 最低 Gate |
| --- | --- | --- | --- | --- |
| FGC-S001 | done | 文档基线 | `ActiveDesignSnapshot@1` Schema、ADR、Pydantic/TS 草案 | contracts generate/check + S001 smoke |
| FGC-S002 | done | S001 | SQLite migration、repository、revision CAS | repository/unit smoke |
| FGC-S003 | done | S002 | active-design GET/select/convert API | API integration smoke |
| FGC-S004 | done | S003 | Desktop API types/client 和错误映射 | typecheck |
| FGC-S005 | done | S004 | Workbench reducer/state machine | reducer tests |
| FGC-S006 | done | S005 | Agent、视口、选择、质量、导出统一 selector | desktop build/E2E |
| FGC-S007 | done | S006 | legacy Concept 只读模式和显式转换 | legacy preservation smoke |
| FGC-S008 | done | S007 | Agent 回退/前进、核心并发竞争、重启、版本一致性 E2E | S008 navigation smoke + workbench E2E |
| FGC-Q002 | done | S008 | 收紧 Snapshot bootstrap/质量检查的幂等与读取副作用合同 | API contract + replay/stale smoke |

### FGC-S001 任务卡

状态：done（2026-07-13）。冻结 Snapshot 合同，未修改 UI、数据库 head 或导出逻辑。

证据：脏工作区内完成；`npm run contracts:types:check`、`npm run agent:s1-active-design-snapshot-smoke`、`npm run agent:check`、`npm run release:docs-walkthrough`、`npm run repository:integrity`、`npm run release:safety-scope`、`npm run release:secrets-files` 与 `git diff --check` 通过。

必须读取：

- `docs/AUTHORITATIVE_STATE.md`
- `docs/SCHEMAS.md`
- `agent-asset-version.schema.json`
- `agent_models.py`
- `concept_schema_registry.py`

交付：

- `active-design-snapshot.schema.json`；
- `ADR-0009-active-design-snapshot.md`；
- Pydantic/生成 TypeScript 类型；
- 正向、unknown field、跨 Project ID、非法 preview/quality/export 引用测试；
- API 尚未实现的明确说明。

禁止：在该任务中修改 `CadWorkbenchPanel`、数据库 head 或导出逻辑。

退出：合同生成无漂移，Schema 能表达 agent 和 legacy-read-only 两种 source，且不能同时提供冲突的活动版本。已由 `npm run contracts:types:check` 和 `npm run agent:s1-active-design-snapshot-smoke` 验证。

### FGC-S002 任务卡

状态：done（2026-07-13）。建立服务端 Snapshot 真值和 revision compare-and-swap，未添加 API 或桌面读取路径。

证据：脏工作区内完成；`npm run agent:s2-active-design-snapshot-smoke`、`npm run agent:g6-asset-editing-smoke`、`npm run agent:g7-external-glb-import-smoke`、`npm run agent:check` 与 `git diff --check` 通过。完整文档门、integrity、安全和 secret 门在 S002 文档同步后重跑。

交付：迁移、repository/UoW、事务测试、旧库升级、空库初始化和回滚说明。

不变量：一个 Project 一行 Snapshot；active/export 同链；selection 属于 active assembly；确认 ChangeSet 和 head/Snapshot 更新同事务。

退出：并发旧 revision 返回冲突，父版本不被覆盖，迁移前后旧数据 hash 不变。

### FGC-S003 任务卡

状态：done（2026-07-13）。提供稳定 API，不暴露数据库实现，也不把 legacy ModuleGraph 假装成可编辑 Agent 资产。

端点：

```text
GET  /api/v1/projects/{project_id}/active-design
POST /api/v1/projects/{project_id}/active-design:select
POST /api/v1/projects/{project_id}/active-design:convert-legacy
```

证据：`npm run agent:s3-active-design-api-smoke`、`npm run contracts:types:check`、`npm run agent:check` 和 `git diff --check` 通过。smoke 覆盖 API bootstrap、Idempotency-Key、revision/ETag、跨项目 Part 拒绝、legacy read-only、重放和 legacy 原数据 hash 不变。

行为：`convert-legacy` 返回 `ready_for_agent_rebuild` hand-off，不创建 AgentAssetVersion；真正的受控重建与 UI 只读转换留给 S007。

退出：Idempotency-Key、revision/ETag、跨项目拒绝、legacy 原数据不变和 OpenAPI/TS 类型全部通过。

### FGC-S004 任务卡

状态：done（2026-07-13）。生成的 OpenAPI TypeScript 已暴露 Snapshot/selection/conversion 类型；`ForgeApiClient` 已读取 ETag、提交 Idempotency-Key 和可选 If-Match，并提供不持有 UI 状态的错误映射。

证据：`npm run desktop:typecheck`、`npm run contracts:types:check` 和 `git diff --check` 通过。

边界：不改 `CadWorkbenchPanel`、不写 localStorage、不创建 reducer，也不将 legacy hand-off 宣传为已完成转换。

退出：客户端从生成类型编译，错误映射涵盖 stale、legacy read-only、not found、invalid 和 Idempotency conflict；后续 S005 只接收这个 client 边界。

### FGC-S005 任务卡

状态：done（2026-07-13）。新增独立 `activeDesignMachine` reducer；它只保存完整 Snapshot 与 ETag，不复制当前版本、选择、质量或导出 ID。

证据：`npm run desktop:s5-active-design-machine-smoke`、`npm run desktop:typecheck` 与 `git diff --check` 通过。smoke 覆盖 Agent/legacy selector、ETag、晚到响应拒绝和 stale 失败保留最后 Snapshot。

边界：未接入 `CadWorkbenchPanel`、未改 WebGL、未改 localStorage 或旧 UI；S006 才能以此 reducer 统一 Agent、视口、选择、质量和导出。

退出：状态机拥有唯一 Snapshot，异步请求按 request ID 防止旧响应覆盖，所有活动版本/选择读取 selector。

### FGC-S006 任务卡

状态：done（2026-07-13）。Agent 资产不再用 localStorage 版本头恢复；保存/导入/确认后从 Snapshot 刷新活动资产，分件选择使用 revision/ETag 写回，视口高亮读取选择，质量与 GLB 导出只读取活动 Agent 资产。

证据：`npm run desktop:typecheck`、`npm run desktop:build`、`npm run desktop:s5-active-design-machine-smoke`、`npm run desktop:r3-concept-workbench-smoke` 和 `git diff --check` 通过。工作台 smoke 额外断言 Snapshot source/version/selection、Agent quality asset version 和 Agent GLB 导出不回退 legacy Concept。

边界：质量报告的 Snapshot 持久化与 legacy 只读收紧由后续 S007/S008 完成；本任务本身不实现这些状态转换。

退出：Agent 资产恢复、选择、视口、质量和 GLB 导出显示/调用同一活动资产版本，旧响应不能覆盖新 Snapshot。

### FGC-S007 任务卡

状态：done（2026-07-13）。legacy Concept 工作台进入只读提示；用户必须显式请求 Agent 重建，服务端记录 source/revision 转换授权。仅在授权存在时，确认的 Agent 资产原子提升为活动 Snapshot，并删除授权；legacy source 保持不变。

证据：`npm run agent:s7-legacy-conversion-smoke`、`npm run agent:g6-asset-editing-smoke`、`npm run agent:s3-active-design-api-smoke`、`npm run desktop:r3-concept-workbench-smoke`、`npm run desktop:typecheck` 和 `git diff --check` 通过。

退出：未授权提升拒绝；授权后 source 切换为 Agent、revision 递增、intent 清理；旧 Project/ConceptVersion/ModuleGraph hash 不变；浏览器流程需先点击授权再写 Agent 资产。

### FGC-S008 任务卡

状态：done（2026-07-13，脏工作区）。已持久化 Agent 质量报告，并使其只绑定 Snapshot 当前 Agent asset；Agent ChangeSet preview/reject/confirm 也已绑定/清理 Snapshot；已实现服务端 undo/redo navigation frame：每次操作从历史内容创建新的不可变 AgentAssetVersion，并在同一事务切换 head/Snapshot、清空选择/preview/quality。`agent:s8-active-design-navigation-smoke` 覆盖 HTTP undo、redo、Idempotency replay、ETag/CAS、preview 阻断、质量写入竞争、选择写入竞争和版本 frame；浏览器 smoke 覆盖 preview、质量 ID、活动版本、undo→redo、重启恢复与 GLB 导出一致。相关 Snapshot/G6/type/build/doc/security/integrity Gate 均通过。

### FGC-Q002 任务卡

状态：done（2026-07-13，脏工作区）。已冻结兼容 bootstrap 的读取副作用，并将质量检查收紧为当前 Snapshot 的 CAS 写入和可重放请求；没有把 navigation 伪装为可独立并发写入的资源。

实现：`GET /active-design` 只会从有效 Agent head 或 legacy current version兼容初始化一行；空 Project 返回 `ACTIVE_DESIGN_NOT_FOUND` 且不写入。`GET /active-design` 和 `GET /active-design:navigation` 都发送 `Cache-Control: no-store`；navigation 是派生读模型，不提供独立 ETag，客户端必须刷新 Snapshot 后再写。`POST :quality` 在公共 HTTP 边界同时要求 `If-Match: W/\"active-design-{revision}\"` 与 `Idempotency-Key`；同键同请求重放原报告，同键不同请求返回 `IDEMPOTENCY_CONFLICT`，旧 revision 返回 `ACTIVE_DESIGN_STALE` 且不新增报告。桌面质量动作传递当前 Snapshot ETag 和一次性键，报告完成后仍刷新 Snapshot。

不得做：不得改变 ActiveDesignSnapshot 的唯一真值、放宽 legacy 只读、删除首次启动迁移测试，或通过缓存掩盖重复质量报告。

证据：`npm run agent:q002-active-design-contract-smoke`、`npm run agent:s8-active-design-navigation-smoke`、`npm run agent:g6-asset-editing-smoke`、`npm run desktop:typecheck`、`.venv/bin/ruff check apps/agent/forgecad_agent scripts/smoke_q002_active_design_contract.py`、`npm run contracts:types:generate` 与后续 `npm run contracts:types:check` 均通过。Q002 contract smoke 覆盖空库、Agent/legacy bootstrap、no-store、浏览器 CORS 的 `ETag` 暴露/`If-Match` 预检、缺少键/ETag、同键重放、同键冲突与 stale 拒绝。广泛多客户端压力和生产缓存策略仍不在本任务范围。

## 3. P0 领域澄清与操作统一

| ID | 状态 | 依赖 | 交付 | 最低 Gate |
| --- | --- | --- | --- | --- |
| FGC-D001 | done | S003,S008 | `DomainInferenceResult@1` 合同和关键词/同义词 fixture | unit/contracts |
| FGC-D002 | done | D001 | recognized/ambiguous/unsupported 服务状态 | Agent smoke |
| FGC-D003 | done | D002,S006 | clarification Item 和单问题 UI | D003 service + focused workbench E2E |
| FGC-D004 | blocked | D003 | 四领域+未知领域 truth set | evaluation gate |
| FGC-V001 | done | S006 | 统一 Agent asset 回退/前进模型 | S008 navigation smoke + workbench E2E |
| FGC-Q001 | done | S006 | Quality 绑定 Snapshot active version | G6 smoke + S008 E2E |
| FGC-X001 | done | S006 | Export 绑定 Snapshot export source | G6/G7 smoke + S008 E2E |

关键退出条件：未知输入写盘数为 0；不存在默认武器回退；UI、质量和导出显示相同资产版本。

### FGC-D001 任务卡

状态：done（2026-07-13，脏工作区）。新增严格的 `DomainInferenceResult@1` 判别合同与四领域关键词/同义词 fixture；它只能表达 `recognized`、`ambiguous` 或 `unsupported`，不属于 Project、Plan、Asset 或持久化事件。只有 `recognized` 可以携带唯一 `domain_pack_id`；另外两态没有可写入领域包。

证据：`npm run agent:d1-domain-inference-contract-smoke`、`npm run contracts:types:generate`、`npm run agent:check` 和 `git diff --check` 通过。smoke 覆盖四包 fixture、三种合法状态、错误候选/状态组合以及 JSON Schema/Pydantic 双重拒绝。

边界：D001 本身未改变 `domain_pack_for_message()` 的旧关键词回退，未创建 API、UI、plan、blockout、版本或数据库记录；D002 已替换该回退。D001 仍不包含 UI 或持久化澄清。

### FGC-D002 任务卡

状态：done（2026-07-13，脏工作区）。新增纯 `DomainInferenceService`，从只读 fixture 推断四包 `recognized`、多包 `ambiguous` 或 `unsupported`。新 Turn 只有在唯一识别后才调用 Planner；D003 在该阻断上增加了只含 clarification Turn/Item 的用户交互，但仍不会创建 Plan、Blockout、候选、版本或资产。

证据：`npm run agent:d2-domain-inference-service-smoke`、`npm run agent:g1-kernel-smoke`、`npm run agent:g4-mechanical-planner-smoke`、`npm run agent:g5-geometry-worker-smoke`、`npm run agent:g6-segmentation-smoke`、`npm run contracts:types:check`、`npm run agent:check` 与 `git diff --check` 通过。smoke 覆盖中英文四领域、组合词歧义、未知领域、零写入屏障和有效输入的正常规划。

边界：D002 本身不调用真实 Provider；澄清持久化和 UI 由 D003 负责，D002 的纯推断合同仍不得创建 Plan、Blockout 或资产。

### FGC-D003 任务卡

状态：done（2026-07-13，脏工作区）。将 `ambiguous`/`unsupported` 结果转换为 `waiting_for_clarification` Turn 和一个 `clarification` Item；迁移 `0027` 扩展 SQLite 状态约束，前端提供单问题、四个用户可读选项，并在选择后保留原始 Brief 开启新 Turn。澄清分支不创建 Plan、Blockout、AgentAssetVersion 或 ActiveDesignSnapshot 资产引用。

证据：`npm run agent:d3-domain-clarification-smoke`、`npm run desktop:d3-domain-clarification-smoke`、`npm run agent:d2-domain-inference-service-smoke`、`npm run contracts:types:check`、`npm run desktop:typecheck`、`npm run desktop:build`、`npm run desktop:r3-concept-workbench-smoke` 与 `git diff --check` 通过。r3 当前覆盖 Agent-first 资产 v2–v5；legacy 组件替换仍由独立任务负责。

退出：含糊/未知输入只显示一个问题；选择前无 Plan/Blockout/Version/Asset 写入；重复 Idempotency-Key 返回同一 clarification Turn；明确选择后正常进入三方向 Planner；focused 浏览器 smoke 断言没有 legacy Brief fallback。

## 4. P0 前端重构与 CI

| ID | 状态 | 依赖 | 交付 | 最低 Gate |
| --- | --- | --- | --- | --- |
| FGC-F001 | done | S008 | 当前工作台 characterization tests | typecheck/build + local Chrome |
| FGC-F002 | done | F001 | 拆分 AgentConversation 和步骤 Item | component smoke + typecheck/build |
| FGC-F003 | done | F001 | 拆分 SelectionCard 与动作命令 | component smoke + typecheck/build |
| FGC-F004 | done | F003 | 拆分 Component/Material/Quality/Export drawers | component smoke + typecheck/build |
| FGC-F005 | done | F002-F004 | 缩减 CadWorkbenchPanel 为组合层 | full E2E + F001–F004 smoke |
| FGC-F006 | done | F005 | 字号、点击目标、aria-live、中文 role | accessibility checks |
| FGC-F007 | done | F006,T003 | 提取工作台生命周期协调适配层 | focused state smoke + full workbench regression |
| FGC-F008 | done | F007 | 提取 Agent 会话瞬态展示状态 | focused state smoke + full workbench regression |
| FGC-F009 | done | F008 | 提取 Agent blockout 候选展示协调 | focused candidate-state smoke + full workbench regression |
| FGC-F010 | done | F009 | 提取已提交 Agent 资产工作区协调 | focused asset-display smoke + current workbench E2E |
| FGC-F011 | done | F010 | 提取 legacy 只读兼容显示边界 | focused compatibility-display smoke + current workbench E2E |
| FGC-F012 | done | F011 | 提取组件库本机偏好协调 | focused catalog-preference smoke + current workbench E2E |
| FGC-F013 | done | F012 | 提取本机视口显示偏好协调 | focused viewport-preference smoke + current workbench E2E |
| FGC-F014 | done | F013 | 提取 legacy ModuleGraph 本机工作区会话 | focused legacy-session smoke + current workbench E2E |
| FGC-F015 | done | F014 | 提取 legacy ModuleGraph 展示叠层状态 | focused graph-overlay smoke + current workbench E2E |
| FGC-F016 | done | F015 | 提取 Agent 概念图展示请求状态 | focused render-presentation smoke + current workbench E2E |
| FGC-F017 | done | F016 | 提取 Agent 组件/结构建议读取状态 | focused edit-assist-presentation smoke + current workbench E2E |
| FGC-F018 | done | F017 | 提取视觉材质目录读取状态 | focused material-catalog-presentation smoke + current workbench E2E |
| FGC-F019 | done | F018 | 提取视觉材质筛选展示状态 | focused material-filter-presentation smoke + current workbench E2E |
| FGC-F020 | done | F019 | 提取材质预选展示状态 | focused material-preselection-presentation smoke + current workbench E2E |
| FGC-F021 | done | F020 | 提取组件库目录读取状态 | focused component-catalog-presentation smoke + current workbench E2E |
| FGC-T001 | done | 文档基线 | CI 加入 G1–G7，不改变产品行为 | workflow checks |
| FGC-T002 | done | S008 | 拆分工作台 E2E 为独立场景 | all E2E |
| FGC-T003 | done | T002 | 单 WebGL、内存和 bundle 预算 | performance gate |
| FGC-T004 | done | S008 | 修复 Agent-first 资产提交版本链与 legacy 写入边界 | r3 Agent-first smoke |

`FGC-T001` 可与 S001 独立进行，但不得降低当前工作台 E2E 的 Snapshot、版本链、preview、quality、undo/redo、重启和导出断言。

### FGC-T001 任务卡

状态：done（2026-07-13，脏工作区）。`.github/workflows/forgecad-core.yml` 的 backend job 现在执行 contracts、G1–G7、D001–D003 smoke；workbench job 先执行 D003 focused UI smoke，再保留原有 r3 workbench smoke。没有删除或放宽现有 r3/打包失败门。

证据：workflow YAML 解析通过；`npm run repository:integrity`、`npm run contracts:types:check`、`npm run agent:check`、G1–G7/D001–D003 本地 smoke 与 `git diff --check` 通过。GitHub runner 的远程执行结果需以对应 CI commit 为准。

### FGC-T004 任务卡

状态：done（2026-07-13，脏工作区）。修复 `CommitAgentBlockoutRequest` 的显式 Project 绑定与跨项目校验；修正 r3 smoke 在同项目先导入 GLB 后仍硬编码 v1–v4 的断言；Agent 资产激活时禁用旧 ModuleGraph 组件替换写入，避免两套版本链混写。

证据：`npm run contracts:types:check`、`npm run agent:g6-asset-editing-smoke`、`npm run desktop:typecheck`、`npm run desktop:r3-concept-workbench-smoke`、`npm run release:docs-walkthrough`、`npm run repository:integrity` 与 `git diff --check` 通过。当前 r3 覆盖参考 GLB v1、可编辑资产 v2–v5、质量、GLB 导出、浏览器重启恢复和 legacy graph 不变。

边界：Agent 组件级替换已由 F003 的“分件候选”入口提供；旧 ModuleGraph 组件替换在 Agent 资产激活时仍保持禁写。多客户端并发、原生安装和 packaged sidecar 不由该任务解除。

## 5. P1 轻量几何

| ID | 状态 | 依赖 | 交付 | 最低 Gate |
| --- | --- | --- | --- | --- |
| FGC-G801 | done | S008,T003 | wedge/capsule runtime | deterministic GLB smoke |
| FGC-G802 | done | G801 | profile/extrude | topology/budget/readback |
| FGC-G803 | done | G802 | revolve | topology/budget/readback |
| FGC-G804 | done | G803 | mirror/array/radial_array | reference/order tests |
| FGC-G805 | done | G804 | 受限 union/subtract | manifold/failure tests |
| FGC-G806 | done | G805 | bevel_approx/surface_panel | visual/readback smoke |
| FGC-G807 | done | G806 | 四领域模板迁移与多样性矩阵 | 48 blockout gate |
| FGC-G812 | done | G807,F009 | 方向匹配的受限视觉变体链路 | build/segment/API/UI smoke |
| FGC-G813 | done | G812,F003 | 零基础“换一版外观”预览循环 | variant/API/card smoke |
| FGC-F022 | done | G813,F009 | 方向预览轮换展示状态收敛 | presentation-state smoke |
| FGC-F023 | done | F022 | 方向预览提示展示协调收敛 | presentation-state smoke |
| FGC-F024 | done | F023 | Provider/离线规划来源展示协调 | presentation-state smoke |
| FGC-E001 | done | F024 | 真实 Provider 四领域 truth-set 评测设计 | no-call evaluation contract + smoke |
| FGC-E002 | done | E001 | 外部 Provider 评测执行器与脱敏报告 | explicit-authorized live-run runner + synthetic no-call smoke |
| FGC-E003 | external | E002 | 四领域真实 Provider baseline | user-authorized, human-reviewed external evaluation |
| FGC-G814 | done | D003, E002 | 普通 Agent Turn 的概念安全预检 | scope decision contract + Planner write barrier |
| FGC-G815 | done | G814, G813 | 受限完整外观意图到视觉族投影 | safe visual-intent mapping + deterministic regression |
| FGC-R006 | done | G815,R002 | 未保存方向的完整外观概念图预览 | bounded preview render + workbench regression |

每个任务只实现一组操作。不得同时引入 Torch、CUDA、模型权重或任意代码执行。

### FGC-G801 任务卡

状态：done（2026-07-13，脏工作区）。依赖 S008、T003；完成受控 `ShapeProgram@1` 的 wedge/capsule 两种轻量 Mesh primitive 与确定性 GLB readback。

范围：只扩展 `Geometry Worker` 的自包含 JSON 执行路径；wedge 使用固定低多边形棱锥模板，capsule 使用固定 16 段、10 环的低多边形胶囊；两者均受现有 triangle budget、finite value、无代码/路径/URL 和 non-functional-only 合同约束。

不得做：不引入 profile/extrude、布尔、任意脚本、神经 3D、碰撞/运动学、真实武器结构或制造参数；不改变 ActiveDesignSnapshot、Agent 版本或导出权限。

交付：`build_glb_from_shape_program()` 现在支持 `wedge`/`capsule`，新增 `scripts/smoke_g801_wedge_capsule.py` 和 `agent:g801-shape-primitive-smoke`，并纳入 backend CI。

证据：`npm run agent:g801-shape-primitive-smoke`、`npm run agent:g5-geometry-worker-smoke`、`npm run agent:g3-shape-program-smoke`、`npm run agent:check`、`npm run contracts:types:check` 和 `git diff --check` 通过；wedge/capsule 均验证 GLB header、正 bounds、三角数、readback 和重复生成字节一致。

退出：已满足。下一项是 G802；G801 不代表复杂实体或工程级几何已完成。

### FGC-G802 任务卡

状态：done（2026-07-13，脏工作区）。依赖 G801；完成受控二维 profile 到 extrude 的 ShapeProgram 运行时与拓扑/readback 门禁。

范围：`profile` 只保存有限的二维点列，`extrude` 只接受前置 profile 操作并沿 Y 轴生成低多边形棱柱；点数最多 32，轮廓必须非退化，三角数按 `4*n-4` 计算并复用既有 GLB triangle budget、finite value 与 non-functional-only 边界。

不得做：不引入任意 Python/JavaScript、B-Rep、布尔、revolve、碰撞/运动学、制造尺寸或现实武器机构；不改变 ActiveDesignSnapshot、Agent 版本或导出权限。

交付：扩展 `shape-program.schema.json`、validator 和 Geometry Worker；新增 `scripts/smoke_g802_profile_extrude.py`、`agent:g802-profile-extrude-smoke` 并纳入 backend CI；生成类型已重新导出并通过合同检查。

证据：`npm run agent:g802-profile-extrude-smoke`、G3/G5/G801 smoke、13 个 Agent 单测、`npm run contracts:types:check`、`npm run agent:check`、ruff 和 `git diff --check` 通过；覆盖两个有效轮廓、GLB header/bounds/triangle readback、重复字节一致和退化轮廓拒绝。

退出：已满足。下一项是 G803；profile/extrude 仍是受控概念几何，不代表工程 CAD 或前端四领域多样性矩阵已完成。

### FGC-G803 任务卡

状态：done（2026-07-13，脏工作区）。依赖 G802；完成受控二维半径/高度 profile 到 revolve 旋转体的运行时与拓扑/readback 门禁。

范围：`revolve` 只接受前置 `profile`，固定绕局部 Y 轴，16 段低多边形旋转；支持完整角度和小于 360° 的概念扇面，半径必须非负，角度必须在 `(0, 2π]`，三角数和 GLB readback 可预测。

不得做：不引入 B-Rep、自动封口/实体修复、碰撞/运动学、制造尺寸、真实武器机构或任意代码执行；不改变 ActiveDesignSnapshot、Agent 版本或导出权限。

交付：扩展 `shape-program.schema.json`、validator 和 Geometry Worker；新增 `scripts/smoke_g803_revolve.py`、`agent:g803-revolve-smoke` 并纳入 backend CI；生成类型已重新导出并通过合同检查。

证据：`npm run agent:g803-revolve-smoke`、G3/G5/G801/G802 smoke、13 个 Agent 单测、`npm run contracts:types:check`、`npm run agent:check`、ruff 和 `git diff --check` 通过；覆盖完整/半角旋转和负半径拒绝。

退出：已满足。下一项是 G804；revolve 仍是受控概念几何，不代表前端领域模板、多视图或工程 CAD 已完成。

### FGC-G804 任务卡

状态：done（2026-07-13，脏工作区）。依赖 G803；完成声明式 `mirror`、`array`、`radial_array` 的引用顺序、数量和预算门禁。

范围：`mirror` 按主轴镜像部件中心；`array` 沿主轴按正间距复制；`radial_array` 绕固定轴按角度复制。每个操作只接受一个已出现的几何输入，最大数量仍受 Schema 64 项和整体 triangle budget 约束；不改变源操作或版本状态。

不得做：不支持任意旋转姿态、自动碰撞修复、布尔实体、B-Rep、工程阵列或制造参数；不改变 ActiveDesignSnapshot、Agent 版本或导出权限。

交付：扩展 `shape-program.schema.json`、validator 和 Geometry Worker；新增 `scripts/smoke_g804_transform_arrays.py`、`agent:g804-transform-arrays-smoke` 并纳入 backend CI；生成类型已重新导出并通过合同检查。

证据：`npm run agent:g804-transform-arrays-smoke`、G3/G5/G801/G802/G803 smoke、13 个 Agent 单测、`npm run contracts:types:check`、`npm run agent:check`、ruff 和 `git diff --check` 通过；覆盖 mirror、线性 array、radial_array、零轴拒绝和缺失引用拒绝。

退出：已满足。下一项是 G805；阵列操作仍是受控概念几何，不代表四领域模板多样性或精确碰撞已完成。

### FGC-G805 任务卡

状态：done（2026-07-13，脏工作区）。依赖 G804；完成受限 `union`/`subtract` 的 manifold/failure 边界。

范围：`union` 只接受 box/cylinder/capsule/wedge 的不重叠或相切 AABB，作为可追溯的复合 Mesh 输出；`subtract` 只接受一个轴对齐盒体减去一个完全包含且贯穿 Y/Z 的盒体，并输出左右两个盒体。所有其他布尔组合显式失败，不回退到近似工程结论。

不得做：不实现 B-Rep、任意网格布尔、自动修复、精确 manifold 证明、碰撞/强度、制造尺寸或现实武器机构；不改变 ActiveDesignSnapshot、Agent 版本或导出权限。

交付：扩展 validator 和 Geometry Worker；新增 `scripts/smoke_g805_boolean.py`、`agent:g805-boolean-smoke` 并纳入 backend CI；保持既有 G801–G804 回归门。

证据：`npm run agent:g805-boolean-smoke`、G3/G5/G801/G802/G803/G804 smoke、13 个 Agent 单测、`npm run contracts:types:check`、`npm run agent:check`、ruff 和 `git diff --check` 通过；覆盖 disjoint union、overlap rejection、合法贯穿 subtract、非法 subtract 和布尔输入数量拒绝。

退出：已满足。下一项是 G806；本任务的 union 是受限复合输出，不应在用户界面中标记为工程实体布尔。

### FGC-G806 任务卡

状态：done（2026-07-13，脏工作区）。依赖 G805；完成受控 `bevel_approx` 与 `surface_panel` 的视觉几何/readback 门禁。

范围：`bevel_approx` 只接受一个 box 或已倒角 box，半径必须小于源盒体 X/Z 面的半尺寸，支持 1–3 个低多边形圆角段；`surface_panel` 只接受一个 box 或 bevel 结果，在 ±Y 面生成贴合的薄面板，面板尺寸和 X/Z 偏移必须落在源面内。输出仍是可追溯的低多边形 Mesh，不宣称精确边缘倒角或工程实体。

不得做：不实现任意网格 bevel、B-Rep/fillet、自动 UV/纹理、碰撞/强度、制造尺寸或现实武器机构；不改变 ActiveDesignSnapshot、Agent 版本、质量或导出权限。

交付：扩展 ShapeProgram Schema/validator 和 Geometry Worker；新增 `scripts/smoke_g806_bevel_surface_panel.py`、`agent:g806-bevel-surface-panel-smoke` 并纳入 backend CI；生成类型已重新导出并通过合同检查。

证据：`npm run agent:g806-bevel-surface-panel-smoke`、G3/G5/G801/G802/G803/G804/G805 smoke、13 个 Agent 单测、`npm run contracts:types:check`、`npm run agent:check`、ruff 和 `git diff --check` 通过；覆盖 1/3 段倒角、默认/显式面板、GLB header/bounds/triangle readback、重复字节一致、越界半径、非法面方向、面板越界和缺失引用拒绝。

退出：已满足。下一项 G807 已完成并进入后续 R001；G806 本身不代表前端已经开放任意倒角或完整四领域生成。

### FGC-G807 任务卡

状态：done（2026-07-13，脏工作区）。依赖 G806 已满足；四领域 48 条确定性 blockout 多样性门禁已通过。

范围：把 ShapeProgram 的受控操作组合进未来武器概念道具、汽车、飞机和机械臂四个同级 Domain Pack；每个领域至少 12 个完整外观 blockout（共 48 个），每个结果必须有明确主体、辅助结构、视觉面板/材质区和可继续分件的输出，而不是孤立零件或只改变缩放的重复模板。

不得做：不引入本地神经 3D、Torch/CUDA/模型权重、现实武器制造结构、汽车/飞机安全结论、机械臂动力学或任意代码执行；不在同一任务中实现多视图、真实 Provider 评测、材质纹理目录或发布签名。

交付：`BLOCKOUT_VARIANT_IDS` 版本化目录、四领域显式模板、确定性 ShapeProgram/GLB 生成、结构差异与重复检测、`scripts/smoke_g807_blockout_diversity.py` 48 条 blockout gate；保持 ActiveDesignSnapshot、单一 WebGL 和现有 G801–G806 回归门。

证据：`npm run agent:g807-blockout-diversity-smoke` 通过，四个 Domain Pack 各 12 个结果、跨领域 48 个结构签名均唯一；每个结果通过 GLB header、bounds、triangle budget、readback、重复生成字节一致、AssemblyGraph 连通性和机械臂 joints 检查。`npm run agent:g5-geometry-worker-smoke`、G801、G806 与 ruff 回归通过。

退出：已满足。该 Gate 只证明后端轻量 blockout 目录和确定性生成，不代表前端已开放 48 个变体、多视图、真实 Provider 或工程级 CAD。

### FGC-G812 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 G807、F009 已满足。将既有 48 个预审、非功能性视觉 blockout 变体接入实际 Agent 方向预览与分件链路；零基础用户继续只选择三张“完整外观方向”卡，不接触变体 ID、参数、尺寸或工程选项。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/API.md`、`docs/SCHEMAS.md`、`docs/TEST_STRATEGY.md`、`apps/agent/forgecad_agent/application/geometry_worker.py`、`apps/agent/forgecad_agent/application/agent_kernel.py`、`apps/agent/forgecad_agent/application/agent_models.py`、`apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx`、F009/G807/G6 smoke。

范围：为 `BuildAgentBlockoutRequest` 和 `SegmentAgentBlockoutRequest` 添加可选的受限 `variant_id`，并在两个响应中返回实际使用的 ID。未提供时，服务端只能根据已验证 `domain_pack_id + silhouette + direction_id` 从同一 Domain Pack 的三项相近视觉变体中稳定选择一个；显式 ID 必须属于该 Domain Pack，否则两个端点都以 4xx 拒绝。构建、分件、ShapeProgram、AssemblyGraph、候选持久化和随后确认的 AgentAssetVersion 必须使用同一个实际 ID，不能出现 GLB 与部件列表不一致。工作台方向卡继续一键预览，但调用同一个受限默认选择；预览仍是临时显示，确认仍由既有 commit → ActiveDesignSnapshot 路径完成。

不得做：不得公开 48 项技术目录、自由输入变体/尺寸、制造或功能配置、任意 ShapeProgram/脚本、神经 3D、第二 renderer、localStorage 版本头，或为候选预览创建新的 Snapshot/Version/ChangeSet 真值。不得把变体解释为现实武器、汽车、飞机或机械臂的功能、安全、结构或制造差异。

交付：稳定的 server-side 变体解析器；build/segment/API/候选保存的一致 ID；生成 OpenAPI/TypeScript；`agent:g812-direction-variants-smoke` 覆盖四领域默认选择、显式选择、跨包拒绝、幂等重放、GLB/分件/ShapeProgram/AssemblyGraph 一致和提交后的版本来源；backend CI 已接入新 Agent smoke。工作台仍通过 Build 返回的实际 ID 调用 Segment；T002、T003 与 r3 验证预览不写版本、确认路径和单一 WebGL 不回归。

退出：已满足。三方向在四领域实际生成的候选都来自受限变体目录，重复请求稳定，显式错误/跨包变体被拒绝；GLB、parts、ShapeProgram、AssemblyGraph、候选 JSON 与已提交 Asset 的持久化 ShapeProgram/AssemblyGraph 都可追溯同一 `variant_id`。`agent:g812-direction-variants-smoke`、G807、G6、G809/G810、F009、T002（12/12）、T003、r3、contracts、typecheck/build、ruff 与 `git diff --check` 已通过。G812 只开放自动匹配的视觉多样性，不等于自由造型、工程 CAD 或真实 Provider 质量评测。

### FGC-G813 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 G812、F003 已满足。为零基础用户提供一个明确的“换一版外观”动作：在当前已选方向的同一三项预审视觉族中轮换下一版 blockout 预览。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/USER_GUIDE.md`、`docs/API.md`、`docs/FRONTEND.md`、`docs/TEST_STRATEGY.md`、`apps/agent/forgecad_agent/application/geometry_worker.py`、`apps/agent/forgecad_agent/application/agent_kernel.py`、`apps/agent/forgecad_agent/application/agent_models.py`、`apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx`、`apps/desktop/src/features/cad-workbench/AgentSelectionCard.tsx`、G812/F003/T002 smoke。

范围：为 build 与 segment 合同增加受限 `variation_index`（仅 `0..2`），它只在未显式提供 `variant_id` 时决定同一 silhouette family 的三项视觉外观顺序；响应返回实际 index。工作台候选卡只显示普通语言的“换一版外观”和“当前第 N / 3 版”，不显示 catalog ID、尺寸、坐标或技术参数。点击动作必须重新走既有 build → segment 候选预览链路；它清除未保存候选的局部选择/编辑展示，但不创建 `AgentAssetVersion`、`ActiveDesignSnapshot`、ChangeSet、质量报告或导出记录，也不覆盖已确认设计。Segment 必须接收 Build 的实际 `variant_id` 与 index，保证 GLB、parts、ShapeProgram、AssemblyGraph 和候选 JSON 同源。

不得做：不得公开 48 项目录或允许自由输入 ID/index/尺寸；不得新增形状操作、任意脚本、本地神经 3D、第二 renderer、localStorage 版本头、自动提交或版本覆盖；不得把视觉轮换描述为真实武器、汽车、飞机或机械臂的功能、安全、结构、材料或制造变化。

交付：确定性的三版解析与请求范围校验；build/segment/candidate 透传；选择卡的 plain-language action 与“未保存预览、不影响已保存设计”说明；`agent:g813-variant-regeneration-smoke`、F003 card callback smoke 和 backend CI；重新生成 OpenAPI/TypeScript。

退出：已满足。四领域每个方向在 index `0..2` 都只能得到其对应三项族中的一个确定性 variant，连续轮换得到不同外观且同一请求可幂等重放；越界 index 被拒绝；segment 与 build/候选始终使用同一实际 variant。工作台动作只更新临时 preview、按钮不泄露内部 ID，且提交、单 renderer、Snapshot/导出一致性不回归。`agent:g813-variant-regeneration-smoke`、G812、G807、G6、G809/G810、F009、F003、T002、T003、r3、contracts、typecheck/build、ruff、文档/安全/secret/integrity Gate 与 `git diff --check` 已通过。G813 不代表自由造型、真实 Provider、工程 CAD 或生产级外观生成。

### FGC-F022 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 G813、F009 已满足。已将当前方向、轮换 index、请求中状态、可恢复错误和“未保存预览”提示从 `CadWorkbenchPanel` 收敛为 project/request 双重屏障的纯展示状态。

范围：新增独立 presentation state/hook 和 focused smoke；项目切换、旧请求、分件失败与连续“换一版外观”必须拒绝过期结果。父层继续唯一拥有 build/segment API、候选持久化、AgentAssetVersion、Snapshot、ChangeSet、质量、导出和 renderer。

不得做：不得改变 G813 的 API/variant 解析、创建版本、写 Snapshot/localStorage、增加第二 renderer、把展示缓存当版本 head，或扩大为自由变体/几何编辑。

退出：已满足。展示状态覆盖初始方向、轮换、过期响应、分件失败、project reset 和 clear；`desktop:f009-agent-blockout-display-state-smoke`、F003、typecheck/build、T002、T003、r3、contracts、文档/安全/secret/integrity Gate 与 `git diff --check` 通过。该任务不代表新的 Agent 能力。

### FGC-F023 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F022 已满足。已将 Agent 方向预览的普通语言提示（生成中、分件暂不可用、生成失败、未保存预览）收敛为纯展示 selector，供对话区域和候选卡一致读取。

范围：新增无副作用 selector/state smoke；提示必须从 F022 的受限状态映射，不向用户暴露 `variation_index`、variant ID、API 错误码或几何技术信息。父层继续唯一拥有 API 调用、assistant turn、候选资产、Snapshot、ChangeSet、质量、导出和 renderer。

不得做：不得改变 Agent 对话事实、自动重试/自动提交、版本、Snapshot、质量、导出、领域判断或 G813 视觉选择；不得添加复杂任务中心、Mode、第二 renderer 或开发者面板。

退出：已满足。selector 仅从 F022 展示状态推导中文提示，覆盖 build、segment、失败和 ready；对话区和候选卡使用相同来源，过期/跨项目状态不显示，且不显示 index、variant ID 或错误码。`desktop:f023-agent-blockout-preview-presentation-smoke`、F002、F003、F009、T002（12/12）、T003、r3、typecheck/build、contracts、文档/安全/secret/integrity Gate 与 `git diff --check` 通过。该任务不代表新增模型生成能力。

### FGC-F024 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F023 已满足。只把当前方向计划的来源（本机离线规划或已连接 Provider）和“是否已执行真实 Provider”收敛为用户可理解的只读展示。

范围：建立 project/request 屏障下的纯展示 selector 与 focused smoke；只显示“本机离线规划”或“已连接模型服务生成”，并在前者明确说明它不能代表真实模型质量。Provider Key、Base URL、模型内部 ID、token、原始请求/响应、费用和错误细节均不进入工作台。

不得做：不得在读取展示时调用 Provider、触发费用、保存密钥、把离线结果伪装为真实 Provider、改变 Agent turn、版本、Snapshot、质量、导出或 renderer；不得新增开发者设置页或复杂 Mode。

退出：已满足。确定性 plan 显示“本机离线规划”，OpenAI-compatible plan 显示“已连接模型服务生成”，未知来源安全回退；不读取 Key、不调用网络、不显示 Provider/model 内部 ID。`desktop:f024-agent-plan-source-presentation-smoke`、F002、T002（12/12）、typecheck/build、contracts、文档/安全/secret/integrity Gate 与 `git diff --check` 通过。该任务不等于真实 Provider 质量评测或持久化运行恢复。

### FGC-E001 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F024 已满足。已冻结真实 Provider 四领域 truth-set 的显式、可计费评测合同、fixture、预算与人工批准边界；没有调用用户的 Provider。

范围：定义评测输入、结构化输出、失败与费用记录、人工批准和脱敏证据；运行必须由用户显式发起，密钥仍只在 Keychain/受限 secret file。

不得做：不得把离线结果写成真实模型评测，不得在 CI、首次启动或普通创意输入时隐式联网/收费，不得记录 API Key、原始敏感上下文或自动生产结论。

交付：新增 `evaluations/agent-provider-v1/contract.json` 和 `truth_set.json`。四个领域各由 5 条完整外观 Brief 主干与 4 条视觉修饰词展开为 20 条，共 80 条；另有 10 条含糊与 10 条越界安全停止条目。默认执行是零费用、零网络、零资产/Snapshot 写入的 dry-run；没有自动重试。真实 run 必须在未来执行器同时验证 `--confirm-live-provider`、正值 `--confirmed-budget-cny`、唯一 `--evaluation-run-id` 与人工授权记录，证据只记录 fixture hash、case ID、结构化结果、安全类别、延迟与 token，不记录 Key、URL、模型内部 ID 或原始 Prompt/Response。

退出：已满足。`npm run agent:e001-provider-evaluation-dry-run` 验证固定 100 条测试计划（80 次正常 Provider 请求 + 20 条本地安全停止）、4×20 领域分布、零默认成本与零网络；`npm run agent:e001-provider-evaluation-contract-smoke` 覆盖默认预算、CI 自动调用、fixture 截断的拒绝。真实调用仍标为 `external`，须由用户单独授权；该任务不实现 live 执行器或宣称真实模型质量。

### FGC-E002 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 E001 已满足。已实现与普通 Agent Turn 隔离的外部 Provider 评测执行器、预算闸门和脱敏 run report；默认不发起真实请求。

范围：执行器必须默认拒绝联网，只有同时提供 E001 固定的三个显式 flag、有效本机 Provider 配置和人工授权记录时才允许逐条调用。每条最多一次，达到请求/token/预算/超时限制即停止；报告只保存 E001 允许的字段，并将错误映射到固定类别。真实执行需要用户逐次批准，CI 只跑无网络合同、dry-run 与合成执行器 smoke。

不得做：不得复用 Weapon R4 live 命令或输出；不得把 Provider Key、Base URL、模型内部 ID、原始 Prompt/Response、费用账单或任何用户项目创意写入报告；不得写 AgentAssetVersion、Snapshot、质量、导出或普通 Thread/Turn。

退出：已满足。`scripts/run_agent_provider_evaluation.py` 在没有三个固定 flag 时仅输出 E001 dry-run；真实路径还要求操作者、批准时间、preflight 和有效 OpenAI-compatible 本机配置，缺配置在网络前拒绝。它把 timeout、限流、鉴权、传输、结构化输出、策略、预算与取消映射到固定类别，最大 80 次正常 Provider 请求；20 条含糊/越界输入在隔离 preflight 本地停止。`npm run agent:e002-provider-evaluation-runner-smoke` 用合成 Provider 覆盖无凭据、缺确认、零/超额预算、超时、取消、无 token usage、输出 token 超限、完整 telemetry 与脱敏报告，且 `network_calls_made=0`。真实网络调用仍是 `external`，未经用户在该次运行中明确授权不得执行。

### FGC-E003 任务卡

状态：external。依赖 E002 已满足；不是可由 CI 或普通 Codex 自动领取的代码任务。

范围：只有用户针对一次具体 run 明确授权、提供有效本机 Provider 配置并确认成本上限后，才按 [真实 Provider 四领域评测合同](AGENT_PROVIDER_EVALUATION.md) 手工执行 E002 命令。执行后由非执行者人工审阅脱敏 run report，确认完整 80+20、token usage 覆盖、所有阈值和实际 Provider 控制台账单。

不得做：不得从 E001/E002 合成测试推断模型质量；不得把 key、Base URL、model ID、原始 Prompt/Response、绝对路径或账单明细存入仓库；不得在 CI、启动、普通 Turn 或失败后自动重试。

退出：仅在用户授权的实际 run 通过、人工审阅完成、脱敏汇总保存在用户批准的位置并在能力矩阵登记 `PASS / external evidence` 后满足。未发生真实 run 时必须保持 `EXTERNAL / NOT RUN`。

### FGC-G814 任务卡

状态：done（2026-07-14，脏工作区，未提交）。已把 E002 的有限概念范围边界提升为普通 Agent Turn 的纯策略合同和 Planner 前写入屏障。

范围：新增版本化 `ConceptScopeDecision@1`，在 DomainInference 后、Planner 前明确返回 `allowed | clarification_required | unsupported`。现实武器制造、加工/材料配方/性能、车辆安全、飞行/适航、机器人控制/扭矩/认证等越界请求必须返回 `unsupported`，不调用 Provider、不创建 Plan、blockout、Thread/Turn 之外的资产、版本、Snapshot、质量或导出。普通含糊输入保持现有 D003 单问题澄清；四领域非功能外观 Brief 必须继续正常进入 Planner。

不得做：不把 E002 evaluation fixture、真实 Provider 运行编号或人工预算带进普通 Turn；不实现制造、工程、性能或安全建议；不新增第二个 Agent、UI Mode 或外部调用；不得把关键词过滤写成唯一安全机制，策略结果必须可测试、可解释并与 DomainInference 分层。

交付：Schema/Pydantic/TypeScript、纯 scope policy、Kernel Planner 前阻断、API/工作台最小可读提示、正常/含糊/越界 fixture 和 D003/G4/r3 回归；更新 `AUTHORITATIVE_STATE`、`API`、`USER_GUIDE`、能力矩阵与测试策略。

证据：`npm run agent:g814-concept-scope-smoke`、`npm run agent:g1-kernel-smoke`、`npm run agent:d2-domain-inference-service-smoke`、`npm run agent:d3-domain-clarification-smoke`、`npm run desktop:f008-agent-conversation-state-smoke`、`npm run desktop:f002-agent-conversation-smoke`、`npm run desktop:typecheck`、`npm run contracts:types:check`、`npm run agent:check`、`npm run desktop:t002-workbench-e2e-scenarios`（13/13，含 scope-stop 场景）、`npm run desktop:r3-concept-workbench-smoke`、`npm run desktop:build`、文档/安全/integrity/secrets Gate 与 `git diff --check` 均通过。

退出：已满足。四领域正常完整外观 Brief 仍只产生受限概念 plan；含糊输入只进入 D003 澄清；10 条明确越界 fixture 与一条已选领域绕过负例在任何 Provider/Planner、资产/Snapshot/质量/导出写入前停止。文档只描述已实际验证的有限预检范围，不把规则集称为完整内容安全系统。

### FGC-G815 任务卡

状态：done（2026-07-14，脏工作区，未提交）。已将已验证、非功能性的完整外观意图稳定投影到既有受限视觉族，使四领域的安全文本方向确实影响 blockout 外观，而不将自然语言变成任意几何或工程参数。

范围：冻结一份有限 `MechanicalConceptSpec@1` 视觉意图到既有 ShapeProgram/variant family 的显式映射合同；只接受已验证的轮廓、细节密度、色彩方向和姿态类别，输出仍必须使用既有 `resolve_blockout_variant()`、Domain Pack 模板和 triangle budget。四领域中相同 Pack 的不同安全视觉方向应得到可重复、可解释、不同的 ShapeProgram/GLB 指纹；无效、未知或越界意图安全回退到既有默认外观，绝不注入 ShapeProgram 操作或数值。

不得做：不得接入神经 3D、任意 Python/JavaScript/shell、自由网格、自由工程尺寸、现实制造/性能/控制建议、第二 WebGL renderer 或新的版本/Snapshot 真值；不得绕过 G814、G813、确认和质量边界。

交付：版本化视觉意图映射合同与 fixtures、Planner/Geometry Worker 的受限适配、四领域正负回归、同输入重复 fingerprint、工作台方向卡不暴露技术 ID/参数的回归，以及 `DESIGN`、`API`、`USER_GUIDE`、能力矩阵和 handoff 同步。

证据：新增 `VisualIntentMapping@1` JSON Schema/Pydantic 与 `visual_intent.py`。确定性 Planner 和 OpenAI-compatible Planner 输出均由本机受限映射归一化；Geometry Worker 只用 mapping 的 0–3 视觉族索引选择现有 catalog，旧/损坏 mapping 仍回退 G812 轮廓路径。`npm run agent:g815-visual-intent-projection-smoke` 覆盖四领域各两条安全 Brief、不同族/GLB/ShapeProgram 指纹、重复生成和损坏回退；G2/G4/G812/G813、F002、typecheck、contracts 和 agent check 均通过。完整工作台/文档 Gate 以本任务最终记录为准。

退出：已满足。四领域各两条安全完整外观 Brief 在同一领域的受限视觉族中稳定分化；未知/越界意图不会生成任意操作或绕过 G814；preview→confirm→Snapshot 版本边界和单 WebGL renderer 保持不变；确定性视觉映射和离线 Planner 未被宣传为真实 Provider 质量。

### FGC-R006 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 G815、R002 已满足。已为尚未保存的三张完整外观方向提供可读的概念图预览，让零基础用户在“保存为可编辑模型”前就能比较完整外观。

范围：复用现有受限 ShapeProgram/GLB 与 R002 软件栅格化渲染能力，针对当前内存 Plan 重建的同源 blockout 生成固定的低分辨率 iso 预览；预览绑定当前 project/request/plan/direction/variant 的临时上下文，切换方向、换一版外观、取消或项目切换后丢弃。工作台只在已有方向卡显示“软件概念图”，沿用单一主视口，不新增 WebGL renderer 或下载入口。

不得做：不得持久化未确认的 GLB、PNG、AgentAssetVersion、Snapshot、质量或导出记录；不得调用真实 Provider、旧 Concept renderer、外部图片模型或生成照片级图像；不得显示工程尺寸、部件连接、制造、安全或性能信息。

交付：候选预览 render 合同/服务、request-context 状态屏障、普通语言的工作台预览卡、取消/迟到响应/项目切换测试，以及 G815/R002/T002/T003/r3 回归。

证据：新增 `AgentBlockoutConceptPreview@1` Schema/Pydantic/OpenAPI 合同和 `POST /api/v1/agent/blockouts:concept-preview`。Kernel 只在内存中构建受限 blockout 并用既有软件栅格器输出固定 320×240 iso PNG；不会打开 UnitOfWork、写幂等、候选、资产、Snapshot、质量或导出。`agent:r006-blockout-concept-preview-smoke` 覆盖四领域、PNG/readback、重复 hash、与实际 build 的 variant/topology 同源及零写入；`desktop:r006-direction-concept-preview-state-smoke` 覆盖三卡加载、迟到响应、clear 与项目隔离；T002-04b 覆盖三图、选择清空、版本不变和单 canvas。CI 已加入后端/桌面 Gate。

退出：已满足。四领域任一方向能在保存前获得与当前 ShapeProgram/variant 同源的完整外观概念 PNG；预览不写版本或 Snapshot，过期图片不能写回，工作台仍只有一个 WebGL canvas；文档只称其为软件概念图，不称为真实渲染或工程图。

### FGC-R001 任务卡

状态：done（2026-07-13，脏工作区）。依赖 S008、G807；已冻结并实现 Snapshot 绑定的概念相机/灯光预设。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/DESIGN.md`、`docs/FRONTEND.md`、`apps/agent/forgecad_agent/application/agent_models.py`、`apps/agent/forgecad_agent/infrastructure/db/agent_repositories.py`、`apps/desktop/src/features/cad-workbench/ModuleGraphViewport.tsx`。

范围：新增 `ActiveDesignRenderPreset@1`，至少包含项目、AgentAssetVersion、相机视图和灯光预设；服务端把它作为 `ActiveDesignSnapshot` 的可选兼容字段并在 Agent Snapshot 初始化时提供默认值；新增带 revision/ETag/Idempotency-Key 的 CAS 更新接口。Agent 资产版本切换时必须重置 preset 的 `asset_version_id`，legacy 只读 Snapshot 不得写入。前端相机/灯光选择读取 Snapshot，更新经过服务端确认后再落地；视口只更新现有 renderer 的 camera/light，不创建第二个 WebGL context。

不得做：不实现 R002 多视图 PNG/ZIP、不调用旧 Concept renderer/export、不把 preset 当成工程照明或照片级渲染结论、不新增第二个 Three.js renderer。

交付：Schema/Pydantic/TypeScript/OpenAPI 类型、迁移、ActiveDesign repository/service/API、桌面 API 客户端与视口 preset 应用、确定性 render-preset smoke、前端状态/单 renderer 回归和文档更新。

证据：`npm run agent:r001-render-preset-smoke`、`npm run agent:s2-active-design-snapshot-smoke`、`npm run agent:s3-active-design-api-smoke`、`npm run contracts:types:check`、`npm run desktop:typecheck` 和 `npm run agent:check` 通过；前端沿用现有单 renderer，preset 更新不重建 WebGL。

退出：已满足。四个相机视图和三个灯光预设通过 Pydantic/API 合法性校验；旧 revision、legacy Snapshot 和跨资产版本引用拒绝；同一 Snapshot/preset 输入得到相同 fingerprint。R001 只提供主视图相机/灯光状态，不等于多视图图片导出。

## 6. P1 概念视图

| ID | 状态 | 依赖 | 交付 | 最低 Gate |
| --- | --- | --- | --- | --- |
| FGC-R001 | done | S008,G807 | Agent asset camera/light preset | deterministic render test |
| FGC-R002 | done | R001 | 3/4、front、side、top PNG | PNG provenance/readback |
| FGC-R003 | done | R002 | 爆炸视图/透明背景候选 | viewport/render test |
| FGC-R004 | done | R002,X001 | Agent render ZIP/下载 API | export E2E |
| FGC-R005 | done | R004 | Agent 直接下载 UI 和用户指南晋级 | browser E2E + Tauri startup |

渲染必须绑定 Snapshot 和 AgentAssetVersion，不得调用旧 Concept export 作为捷径。

### FGC-R002 退出记录（2026-07-13）

已实现 Agent-only 的软件概念渲染器与只读派生 render-set：它从当前活动 AgentAssetVersion 的 GLB 生成 `iso/front/side/top` 四张 PNG，记录来源版本、尺寸、字节数、SHA-256 与 PNG readback 状态，并以稳定 fingerprint 保证重复请求一致。桌面导出抽屉提供“生成概念图”、四视图缩略图和单图下载；没有新增 WebGL renderer，也不会创建版本或修改 Snapshot。

证据：`npm run agent:r002-render-views-smoke`、`npm run contracts:types:check`、`npm run desktop:typecheck`。R002 不包含 ZIP、转台视频、独立渲染器或照片级材质。

### FGC-R003 任务卡

状态：done（2026-07-13，脏工作区）。R003 已在 R002 的同一 Agent-only 软件渲染管线上增加透明背景的爆炸概念候选；不改变资产、Snapshot、质量或导出版本真值。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/DESIGN.md`、`docs/API.md`、`apps/agent/forgecad_agent/application/agent_rendering.py`、`apps/agent/forgecad_agent/application/agent_asset_editing.py`、`apps/desktop/src/features/cad-workbench/ExportDrawer.tsx`。

范围：只为活动 AgentAssetVersion 生成可复现的派生 PNG；爆炸布局只能使用既有 AssemblyGraph 的 part 层级、稳定 Part ID 和确定性视觉间距，透明背景必须保留 PNG alpha readback。所有输出都记录来源资产版本、尺寸、哈希与 fingerprint；工作台只在既有导出抽屉显示/下载结果。

不得做：不得创建第二个 Three.js/WebGL renderer，不得改变 Part 位置或创建 ChangeSet，不得调用 legacy Concept renderer/export，不得将爆炸图解释为装配、维修、制造或工程说明，不得实现 ZIP、转台视频、OBJ/MP4 或工程渲染。

实现：Renderer 只在当前 GLB 的 primitive 几何组与 `AgentAssetVersion.parts`/AssemblyGraph 稳定 Part ID 完全一一对应、且至少有两个部件时生成 `exploded_iso`。视觉间距从现有 parent/position/size 事实确定；缺失映射、外部或扁平几何明确不生成候选，工作台显示原因，不伪造分件。所有 PNG 都有 RGBA alpha readback；爆炸视图记录 `presentation_mode=exploded`、`background_mode=transparent`、稳定 `part_ids`、来源资产、字节数、SHA-256 与 render-set fingerprint。

证据：`npm run agent:r003-exploded-views-smoke`、`npm run agent:r002-render-views-smoke`、`npm run desktop:f004-workbench-drawers-smoke`、`npm run desktop:t002-workbench-e2e-scenarios`（12/12，含导出抽屉第五张图/透明背景断言）、`npm run desktop:typecheck`、`npm run contracts:types:check` 和 ruff 通过。

### FGC-R004 任务卡

状态：done（2026-07-13，脏工作区）。R002/R003 的单图 PNG 已是绑定当前 AgentAssetVersion 的只读派生结果；R004 只把同一 render-set 的当前 PNG 与机器可读 manifest 打包为一次下载，不改变模型或重新定义导出真值。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/API.md`、`docs/USER_GUIDE.md`、`apps/agent/forgecad_agent/application/agent_asset_editing.py`、`apps/agent/forgecad_agent/api/agent_asset_routes.py`、`apps/desktop/src/features/cad-workbench/ExportDrawer.tsx`。

范围：仅接受当前活动 AgentAssetVersion；ZIP 内必须包含 render-set manifest、来源版本、视图 SHA-256、尺寸、展示模式、背景模式与爆炸候选的 `part_ids`。下载不创建 Version、Snapshot、Quality 或 Export 记录；桌面只在现有导出抽屉提供明确的“下载概念图包”动作。

不得做：不得包含 legacy Concept render、OBJ/MP4、任意源文件、工程图、装配/维修说明或制造信息；不得让 ZIP 的存在暗示 Agent 已支持多格式资产导出或照片级渲染。

最低 Gate：ZIP manifest/hash/readback/repeatability/API stale 拒绝 smoke、导出抽屉 E2E 下载断言、单 WebGL 回归、合同/typecheck 与用户指南只描述已验证能力。

实现与证据：新增 `AgentAssetRenderPackage@1`，ZIP 固定只含 `manifest.json`、`iso.png`、`front.png`、`side.png`、`top.png` 和在安全映射成立时的 `exploded_iso.png`。manifest 记录来源 asset version、render-set fingerprint、视图 SHA-256、尺寸、展示/背景模式、稳定 `part_ids` 与概念非工程声明；ZIP member 使用固定顺序、时间戳和权限以保证同一输入字节一致。`GET /api/v1/agent/asset-versions/{id}:render-package` 强制携带当前 render-set fingerprint，并拒绝已切换的活动资产或指纹不匹配的旧预览；不写入 Version、Snapshot、Quality、Export 或对象库。导出抽屉仅在已生成当前概念图后显示“下载概念图包”。`npm run agent:r004-render-package-smoke` 覆盖 ZIP member/manifest、PNG hash/readback、透明背景、重复字节一致、缺 fingerprint 与 stale 指纹拒绝；`npm run desktop:t002-workbench-e2e-scenarios` 覆盖浏览器 ZIP 下载，T003 保留单 canvas/context 回归。

### FGC-R005 任务卡

状态：done（2026-07-13，脏工作区）。这是本机 Alpha UX/原生验证任务，不是签名、公证或外部发布任务。

必须读取：`docs/USER_GUIDE.md`、`docs/FRONTEND.md`、`docs/AUTHORITATIVE_STATE.md`、`docs/API.md`、`apps/desktop/src/features/cad-workbench/ExportDrawer.tsx`、`apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx`、`apps/desktop/src-tauri/`。

范围：只重整 Agent 资产导出抽屉的零基础用途文案和可用动作，使 Agent 路径只显示已实现的 GLB、单张概念 PNG 与概念图包；legacy Concept 选项必须仍处于只读兼容边界，不能被 Agent UI 误导为可用能力。对本机 Tauri 开发包执行真实下载验收，确认 ZIP 的浏览器/原生 WebView 下载不会改变 Snapshot、版本、质量、选择或单一 WebGL context。

不得做：不得新增 OBJ、MP4、源包、STEP/3MF、工程图、批量导出、云存储、签名/公证或绕过 R004 fingerprint；不得将概念图包称为模型资产包；不得为下载新增 renderer、持久化 Export 记录或 legacy 回退。

最低 Gate：Agent-only 导出抽屉组件/工作台 E2E、T003 单 renderer 回归、可重复的本机 Tauri download E2E（或记录可复现的环境阻断）、typecheck/build/contracts/docs/security Gate。用户指南只可描述通过的本机行为。

实现与证据：Agent 资产激活时，抽屉只显示“下载 3D 模型 (GLB)”、生成/下载单张概念 PNG 和已生成概念图的受限 PNG/manifest 图包；旧用途选择、OBJ 与源包仅保留在 legacy Concept 只读兼容分支。`desktop:f004-workbench-drawers-smoke`、`desktop:t002-workbench-e2e-scenarios`（12/12，含直接 GLB 与图包下载）和 `desktop:r3-concept-workbench-smoke` 已通过，T003 单 renderer 回归保持通过。`FORGECAD_LOCAL_VISUAL_PACK=0 ./script/build_and_run.sh --verify` 已完成 `.app` 构建、原生进程启动和 `local-dev-python` Agent 健康检查；随后自动化原生 WebView 下载时，macOS 返回“osascript 不允许辅助访问”。这是已记录、可复现的环境阻断，不得表述为原生下载点击已通过。获得辅助功能权限后必须按 `DEVELOPMENT.md` 重复 GLB、单 PNG、图包下载和 Snapshot 不变量验收。

## 7. P1 材质与组件

| ID | 状态 | 依赖 | 交付 | 最低 Gate |
| --- | --- | --- | --- | --- |
| FGC-M101 | done | S008 | 完整 MaterialPreset 字段和兼容迁移 | contracts |
| FGC-M102 | done | M101 | 金属/塑料/橡胶/复合/透明/涂层目录 | catalog smoke |
| FGC-M103 | done | M102 | 纹理对象、来源、许可证和缩略图 | license/object tests |
| FGC-M104 | done | M103 | Material Zone UI 与检索 | workbench E2E |
| FGC-M105 | done | M104 | Material Zone 与部件槽绑定 | ChangeSet/E2E |
| FGC-M106 | done | M105 | 领域兼容材质筛选 | catalog/workbench |
| FGC-M107 | done | M106 | Material Zone 选择持久化 | Snapshot/restart |
| FGC-C101 | done | S008 | part role 中文与稳定词典 | localization tests |
| FGC-C102 | done | C101 | 可解释的项目内组件替换结论 | service/UI tests |
| FGC-C103 | done | C102 | 候选 split/merge（不直接写版本） | graph/change tests |
| FGC-C104 | done | C103 | 部件锁定、隐藏、隔离和统一回退 | API/service/UI/E2E |
| FGC-G808 | done | C104 | 受限可编辑参数映射合同 | schema/service tests |
| FGC-G809 | done | G808 | 参数映射与 ChangeSet 校验绑定 | service/version tests |
| FGC-G810 | done | G809 | 为确定性 blockout 声明受限参数 | worker/version tests |
| FGC-G811 | done | G810 | 零基础参数控件读取真实声明 | focused UI/E2E |

### FGC-C101 任务卡

状态：done（2026-07-13，本机脏工作区）。为四领域 Agent 部件提供稳定、可读的中文 role 词典和缺省回退，不改变 `part_id`、AssemblyGraph、Snapshot 或版本链。

范围：建立只读 TypeScript 词典，将 `primary_body`、`fuselage`、`cockpit_canopy`、`main_wing`、`tail_surface`、`shoulder_joint`、`joint_elbow` 等稳定 role 映射为零基础用户可理解的中文名称；未知 role 显示安全的“未命名部件”，不得凭名称推断领域或功能。词典仅位于显示边界，不修改 API、Pydantic、AssemblyGraph 或持久化数据；候选选择、材质上下文和组件保存名称都复用同一映射。

不得做：改写稳定 role、自动推断未知领域、引入制造/安全语义、复制材质到 Snapshot、增加第二 renderer 或在同一任务中实现 compatibility score。

证据：`npm run desktop:c101-part-role-labels-smoke` 覆盖四领域稳定 role、已知关节与未知回退；`npm run desktop:f003-agent-selection-card-smoke` 覆盖组件显示不泄露 `joint_elbow`；并已通过 `desktop:f004-workbench-drawers-smoke`、M104–M106、T002（12/12）、r3、typecheck 与 build。完整文档/安全/Agent Gate 记录在本轮交接。

### FGC-C102 任务卡

状态：done（2026-07-13，本机脏工作区）。为已有项目内组件替换候选补充可解释的兼容性结论；它不是工程性能、结构安全、制造适配评分或正式资产审阅。

范围：新增 `AgentComponentCompatibility@1`/`AgentComponentCandidate@1` 和 `components:compatible` HTTP 读取路径。候选只读取实际存在的 AgentComponent 启用状态、当前 Agent asset 的 `domain_pack_id`、稳定 role、来源资产最新质量，以及替换保留当前目标 AssemblyGraph 连接的事实。来源 `passed`/`warning` 可进入 preview；`failed`/`unavailable`、停用、跨领域或不同 role 必须不可替换。UI 只显示中文理由；所有永久替换仍先 preview，再 confirm，并由 ChangeSet 与 ActiveDesignSnapshot 保持版本真值。

边界：`AgentComponent` 是项目内快照，不带正式 Module Asset 的 creator/reviewer/review_status；不得把该资产目录的审阅状态复制或伪装到 AgentComponent。组件自身不保存重复质量字段，`source_quality_status` 每次从来源 asset 最新报告计算。外部 GLB 参考不得查询或替换组件。

不得做：从显示中文名称反推 role、捏造连接器或质量数据、给出结构/安全/制造评分、为未知领域自动放行、修改 C101 词典、绕过 ChangeSet 或增加第二 renderer。

证据：`npm run agent:c102-component-compatibility-smoke` 覆盖 HTTP 端点、来源 `unavailable`/`failed` 拒绝、质量恢复、停用、领域/role 不匹配、目标连接保留和 preview-first ChangeSet；`npm run desktop:f003-agent-selection-card-smoke` 覆盖中文解释；G6、typecheck、contracts、T002、r3、build 与文档/安全 Gate 在本轮交接中重跑。下一项为 `FGC-C103`，只能提出 split/merge 候选，不得直接写版本。

### FGC-C103 任务卡

状态：done（2026-07-13，本机脏工作区）。为零基础用户设计“建议拆分/合并部件”的候选流程，不把自由网格建模或任意代码暴露到工作台。

范围：从当前 Agent asset 的 AssemblyGraph、稳定 role、ShapeProgram 和已有连接事实生成只读 split/merge 建议；用户只能查看影响范围和选择“创建预览”，最终永久变化必须通过新的受限 ChangeSet、质量重跑和 Snapshot 版本链。没有足够结构事实时明确说明“暂不能建议”，不猜测。

不得做：直接写 AgentAssetVersion、自动合并确认、任意网格布尔、现实武器功能结构、工程连接/强度结论、绕过 C102 质量边界或增加第二 renderer。

最低 Gate：图/ChangeSet 正负例、预览无写入、确认创建子版本、quality/export/Snapshot 一致、工作台 E2E、typecheck、build、`agent:check`、`contracts:types:check` 和 `git diff --check`。

证据：新增 `AgentStructureSuggestion@1` 与 `GET /asset-versions/{id}/structure-suggestions`。候选仅来自当前 `AssemblyGraph`、稳定 role、受限 ShapeProgram primitive output 和已有连接事实；无事实、外部 GLB、锁定、关节或过期 suggestion ID 均不允许写入。`npm run agent:c103-structure-suggestions-smoke` 覆盖 split/merge 正例、伪造 suggestion ID 拒绝、HTTP 读取、preview 无写入、confirm 子版本以及确认后的 quality/export/Snapshot 一致；`desktop:f003-agent-selection-card-smoke` 覆盖中文动作，r3 覆盖事实不足时明确不建议，T002/T003/typecheck/build/contract Gate 均在本轮重跑。后续任务须由新的原子任务卡定义，不能把 C104 的显示状态扩展成工程装配或任意版本浏览。

### FGC-C104 任务卡

状态：done（2026-07-13，本机脏工作区）。将部件锁定、隐藏、单独查看和显示全部收敛为 `ActiveDesignPartDisplay@1`，并由唯一 `ActiveDesignSnapshot` 的 revision/ETag/Idempotency-Key CAS 持久化；不创建几何版本，不增加第二个 WebGL renderer。

范围：新增 `part_display` 可选合同与 SQLite 迁移，旧 Snapshot 缺少该字段时仍可安全加载。`locked_part_ids` 必须同时驱动工作台禁用状态和后端 ChangeSet 拒绝；`hidden_part_ids` 与 `isolated_part_id` 只控制现有主视口。隐藏或隔离导致已选部件不可见时，服务端原子清空选择；资产版本推进、撤销/重做时只保留下一版本仍有的稳定 part ID。新 API 为 `POST /api/v1/projects/{project_id}/active-design:part-display`，支持 `lock|unlock|hide|show|isolate|clear_isolation|show_all`。

不得做：把锁定描述成工程、制造或安全约束；把隐藏误写成删除；让 localStorage 或组件 state 成为第二份显示真值；在 preview 存在时绕过统一 Snapshot；为单独查看创建新的 Three.js renderer；扩展自由 split/merge、任意网格编辑或现实武器功能。

证据：`npm run agent:c104-part-display-smoke` 覆盖 HTTP/CAS/幂等、锁定后服务端 ChangeSet 拒绝、隐藏/隔离选择保护及资产推进状态归一化；`npm run desktop:f003-agent-selection-card-smoke` 覆盖零基础动作与锁定禁用；`npm run desktop:r3-concept-workbench-smoke` 覆盖锁定重启恢复、单独查看、隐藏清选择和显示全部；`npm run desktop:t003-performance-smoke` 确认仍为一个 canvas/context。`agent:check`、ruff、contracts、typecheck、build、文档/安全/secret Gate 与 `git diff --check` 在本轮重跑。下一项为 `FGC-G808`，先冻结受限参数映射合同，不在同一任务中新增几何操作或 UI。

### FGC-G808 任务卡

状态：done（2026-07-13，本机脏工作区）。依赖 C104；为既定顺序中“扩展轻量 ShapeProgram/Geometry Worker”先冻结了不执行的输入合同，没有改动 UI、Geometry Worker 或 ChangeSet 白名单。

范围：新增 `EditableParameterBinding@1`，使 `BlockoutPartCandidate` 可选声明最多六个既有 position/scale 数值路径的稳定 ID、用户显示名、单位、默认值、范围和步长。JSON Schema/Pydantic 分别拒绝未知字段/路径与无效有限值、范围、单位、边界和同 Part 重复 ID/路径。旧 Asset 缺少 `editable_parameter_bindings` 时仍默认为空并可加载。

不得做：增加自由尺寸输入、B-Rep、任意 ShapeProgram 操作、现实武器功能参数、工程/安全/制造结论、前端参数面板、第二 renderer，或绕过 preview→confirm/ChangeSet/ActiveDesignSnapshot。锁定部件仍必须由 C104 的服务端保护先行拒绝。

证据：新增 `npm run agent:g808-editable-parameter-bindings-smoke`，覆盖 JSON Schema/Pydantic、旧资产兼容、路径、单位、有限范围、步长和唯一性；`contracts:types:generate/check` 已生成 Concept TypeScript、Python registry 与 OpenAPI 类型；G3/G6 回归通过。CI 的 backend contract job 已接入该 smoke。下一项为 `FGC-G809`，只将已声明 mapping 绑定到既有 ChangeSet 校验和不可变版本链，不新增 UI 或几何操作。

### FGC-G809 任务卡

状态：done（2026-07-13，本机脏工作区）。依赖 G808；将已冻结的 Part 参数声明接入既有 ChangeSet，而没有新增 UI、几何操作或自由参数。

交付：`set_part_parameter` 在 Part 有非空 `EditableParameterBinding@1` 列表时只接受该 Part 的已声明路径，并按声明范围和步长拒绝越界或非步进值；C104 锁定仍在声明检查前拒绝。历史 `AgentAssetVersion@1` 缺少 bindings 时明确冻结为“原六条 position/scale 路径 + 原全局概念边界”的兼容策略，空列表绝不意味着任意路径开放。组件替换和结构拆分会保留其 JSON 绑定声明；preview 仍不写版本，confirm 仍创建不可变子版本并原子推进 Agent head/Snapshot。

不得做：增加 UI、自然语言自由参数、任意路径或表达式、单位转换、ShapeProgram 新操作、直接覆盖父版本、工程/制造/武器功能参数，或改写锁定/质量/导出/选择真值。

证据：新增 `npm run agent:g809-parameter-binding-changesets-smoke`，覆盖声明/未声明、范围/步长、C104 锁定优先、preview 无版本写入、confirm 子版本/绑定保留、Snapshot/export/head 一致，以及旧资产的固定六路径兼容和任意路径拒绝；CI backend job 已接入。`contracts:types:check`、G6、C104、G808 与完整任务 Gate 在本轮通过。零基础参数面板必须另立前端任务。

### FGC-G810 任务卡

状态：done（2026-07-13，本机脏工作区）。依赖 G809；为新生成的四领域确定性 blockout 提供真实、受限的 Part 参数声明，没有增加 UI 或新的 ShapeProgram 操作。

交付：Geometry Worker 只为唯一 role 对应的单一 `box`/`wedge` ShapeProgram 输出生成 `scale.x/y/z` 的 `EditableParameterBinding@1`（ratio、默认 1、`0.6..1.4`、步长 `0.1` 与零基础可读名称）。重复 role（如成对车轮/双 nacelle）与当前 cylinder/capsule 适配器保持空声明，避免一个“单部件”操作实际修改多个或非独立的几何参数；`editable_parameters` 同步反映真实声明。所有四领域默认生成链均至少包含一个非空声明，G809 的真实新资产 preview→confirm 已覆盖子版本、Snapshot/head/export 一致；独立 legacy fixture 仍证明空声明只走固定六路径兼容。

不得做：参数 UI、任意尺寸输入、单位转换、ShapeProgram 新操作、自由代码/表达式、工程/制造/武器功能参数、修改 Snapshot/锁定/质量/导出真值或重写已有不可变资产。

证据：新增 `npm run agent:g810-generated-parameter-bindings-smoke`，覆盖四领域默认生成、重复确定性、每条声明与真实唯一 `box`/`wedge` `args.size` 对应、范围/步长/单位和重复 role/cylinder/capsule 的负边界；G6 segmentation、G6 asset editing、G809、G807/C104、contracts 与完整任务 Gate 本轮重跑。CI backend job 已接入。下一项 `FGC-G811` 才能为零基础用户读取真实绑定制作参数控件。

### FGC-G811 任务卡

状态：done（2026-07-13，本机脏工作区）。依赖 G810。开始前读取 G808–G810、`AgentSelectionCard.tsx`、`activeDesignMachine.ts`、`agent_asset_editing.py`、G809/G810 smoke、工作台 E2E 与 `AUTHORITATIVE_STATE.md`。

目标：将当前硬编码的“缩短/放大”动作替换为零基础用户能理解的、由当前选中 Part 的真实 `editable_parameter_bindings` 驱动的紧凑参数控件；用户只可选择声明值的步长，并且每次修改仍只创建 ChangeSet preview，确认才创建版本。无声明、锁定、legacy 或外部 GLB 时必须显示可理解的不可编辑状态，不得猜测控件。

不得做：自由数字输入、任意路径/单位换算、批量编辑、第二 renderer、localStorage 参数真值、绕过 preview→confirm/ETag/Snapshot、工程尺寸/制造参数或现实武器功能。

退出：控件只显示真实绑定的显示名、当前值/范围/步长和 ratio；声明为空时不显示错误控件；锁定禁用；点击生成正确 `set_part_parameter` ChangeSet；预览/取消/确认、刷新与版本切换状态一致；单一 WebGL 保持；focused component、workbench E2E、G809/G810、typecheck/build、文档/安全 Gate 通过。

交付：新增独立 `AgentParameterControls`，不保存本地参数草稿；只从当前 `AgentAssetVersion.assembly_graph` 的 position/scale 读取当前值（缺失时才使用该绑定的声明默认值），并显示中文“比例（ratio）”、范围和步长。每次“减少/增加”只创建一个以该绑定 `path` 与 `step` 计算的 `set_part_parameter` preview；范围边界、锁定和已有 preview 均禁用控件。空 bindings 显示“暂不支持单独调整比例”，外部 GLB 仍不进入部件编辑区，legacy 保持只读转换路径。候选卡不再硬编码“可调比例”，而从当前资产的真实声明显示“可调整/暂不可调”。

证据：`npm run desktop:f003-agent-selection-card-smoke` 覆盖声明显示、实际 `path/value`、空声明和锁定；`npm run desktop:t002-workbench-e2e-scenarios` 12/12 通过，覆盖 preview 取消、确认、质量、导出、刷新和单一 canvas；`npm run desktop:r3-concept-workbench-smoke` 通过 Agent-first 的 preview→confirm、锁定和重启链；G809、G810、C104、T003、typecheck、build、agent check 及文档/安全 Gate 本轮重跑。未新增自由输入、单位换算、工程尺寸或 ShapeProgram 操作。

### FGC-M101 任务卡

状态：done（2026-07-13，本机脏工作区）。依赖 S008；本任务只冻结并实现 `MaterialPreset@1` 的完整视觉字段，不引入纹理文件或工程材料数据库。

范围：保留现有 `material_id`、七个内置预设、`provenance` 和四领域兼容值；扩展 PBR 的颜色、金属度、粗糙度、不透明度、内容寻址纹理 ID、法线强度、自发光、透射、IOR、清漆和纹理缩放；增加 `visual_tags`、`source`、`license`、`version` 元数据；旧 payload 缺少新增字段时补齐安全默认值，不改变 ID、材质绑定或 AgentAssetVersion。

不得做：纹理对象上传、外部许可证推断、真实材料牌号/强度/密度、Material Zone UI 检索、第二渲染器或修改 Snapshot/版本链。

退出：JSON Schema、Pydantic、OpenAPI/TypeScript 生成类型一致；旧 payload 和完整 payload 均通过，越界 PBR/纹理字段被拒绝；七个内置材质目录 smoke 与 G2 合同 smoke 通过。证据：`npm run agent:m101-material-contract-smoke`、`npm run agent:g2-contracts-smoke`、`npm run agent:g6-material-catalog-smoke`、`npm run contracts:types:check`、`npm run agent:check`、`ruff check`、`npm run desktop:typecheck`、`npm run desktop:build` 和 `npm run desktop:f004-workbench-drawers-smoke`。下一项是 `FGC-M102`，只扩充视觉材质目录数量。

### FGC-M102 任务卡

状态：done（2026-07-13，本机脏工作区）。依赖 M101；扩充六类轻量视觉材质，不上传纹理、不引入工程材料数据库。

范围：在保留七个既有材质 ID 和旧 API 行为的前提下，增加喷涂钢板、哑光工程塑料、轮胎橡胶、碳纤复合外观、透明玻璃和粉末涂层六个稳定预设；所有预设都使用 `visual_only=true`、四领域兼容白名单和参数 PBR 默认值。

不得做：材质纹理上传、供应商/牌号/密度/强度/温度等工程字段、外部许可证自动判断、Material Zone 检索 UI 或改变 ChangeSet/Snapshot。

退出：目录共 13 个唯一 ID，覆盖 metal/polymer/rubber/composite/glass/coating 六类；目录、来源、视觉边界和旧七个预设回归通过。证据：`npm run agent:m102-material-catalog-smoke`、`npm run agent:g6-material-catalog-smoke`、`ruff check`、`agent:check`、`npm run agent:unit`、`npm run contracts:types:check`、`npm run desktop:typecheck`、`npm run desktop:f004-workbench-drawers-smoke`、`npm run desktop:r3-concept-workbench-smoke` 和 `npm run desktop:build`。下一项 `M103` 需先定义纹理对象、来源和许可证边界。

### FGC-M103 任务卡

状态：done（2026-07-13，本机脏工作区）。依赖 M102；实现受控的视觉纹理对象、来源/许可证元数据和缩略图引用，不把外部纹理自动变成可信资产。

范围：新增 `MaterialTextureObject@1` 合同与内部内容寻址对象元数据；规定允许的纹理用途（base color、normal、thumbnail）、媒体类型、尺寸/字节/哈希校验、对象存储边界、`MaterialSource`/`MaterialLicense` 映射和缺失对象时的安全降级。`MaterialPreset@1` 只能引用 `asset_...` ID；目录 API 返回对象存在性与 provenance 摘要，不返回绝对路径。为当前 13 个内置预设补充无纹理时的缩略图/参数回退契约，并保留 `visual_only=true`。显式纹理登记只接受原始 base64，不自动下载或读取路径。

不得做：自动下载 URL、任意本地路径读取、外部许可证推断、把第三方纹理标为本人原创、工程材料字段、第二渲染器、Material Zone UI 检索、改变 Snapshot/ChangeSet/版本链或在无人工确认时替换材质。

实现前必须阅读：`docs/MATERIAL_SYSTEM.md`、`docs/ASSET_AUTHORING.md`、`docs/THIRD_PARTY_LICENSES.md`、`docs/AUTHORITATIVE_STATE.md`、对象库/迁移入口和 M101 生成合同。若来源或许可证无法验证，状态必须是 `unknown`，不得猜测。

退出：JSON Schema、Pydantic、OpenAPI/TypeScript 生成物一致；旧 MaterialPreset payload 继续可加载；合法内部纹理 ID、非法路径/URL/媒体类型/尺寸/哈希/许可证组合均有测试；对象存储拒绝绝对路径并能安全回退到参数材质；目录 API 和 13 个预设回归通过；用户指南没有增加未实现的纹理编辑承诺。证据：`npm run agent:m103-material-texture-smoke`、`npm run agent:m102-material-catalog-smoke`、`npm run agent:g6-material-catalog-smoke`、`npm run contracts:types:check`、`npm run agent:check`、`ruff check`、`npm run desktop:typecheck`、`npm run desktop:f004-workbench-drawers-smoke`、`npm run release:secrets-files`、`npm run release:safety-scope` 和 `git diff --check`。

### FGC-M104 任务卡

状态：done（2026-07-13，本机脏工作区）。依赖 M103；只改现有 `MaterialDrawer`/工作台状态接线，不创建第二渲染器或第二个材质真值。

范围：把当前视觉材质目录接入零基础用户可理解的 Material Zone 检视：按部件/区域显示当前材质、六类筛选、关键词搜索、缩略图/参数回退状态、来源/许可证摘要和“预览材质”动作；保留 ChangeSet preview → confirm，所有永久替换仍由父层 Snapshot/ETag/版本链拥有。缺失纹理必须显示“使用参数外观”，不可伪造“已加载”。

不得做：自由 PBR 参数编辑、任意 URL/本地路径导入、把 `third_party`/`unknown` 写成原创、第二 Three.js 预览器、直接修改 AgentAssetVersion、改变部件几何或新增工程材料字段。

退出：Material Zone 与选中 Part 一致；搜索和分类筛选可叠加；对象缺失安全回退；单一 WebGL canvas 不重建。实际交付了当前部件/材质区上下文、六类中文筛选、名称/标签搜索、参数/纹理回退、来源/许可证摘要和单一预览动作；永久变更仍由父层 ChangeSet/Snapshot 拥有。兼容领域筛选尚未接入，不能从本任务推断。证据：`npm run desktop:m104-material-zone-smoke`、`npm run desktop:f004-workbench-drawers-smoke`、`npm run desktop:typecheck`。完整 T002、r3、build 和合同回归在任务交接中记录。

### FGC-M105 任务卡

状态：done（2026-07-13，本机脏工作区）。依赖 M104；把 Material Zone 检视接入真实部件材质区选择和 ChangeSet 路径，不改变视觉材质与工程材料边界。

范围：为 Agent 部件的每个 `material_zone_id` 提供零基础可读名称和选择入口；选择区域后只显示该区域的当前预设、纹理摘要和兼容材质；点击“预览材质”必须生成带 `part_id`/`material_zone_id` 的 ChangeSet preview，确认/取消沿用现有 Snapshot/ETag/版本链。没有稳定区域映射时继续显示“主材质区”，并要求 Agent 澄清，不得猜测。

不得做：自由 PBR 编辑、任意 URL/路径导入、真实材料性能、第二 renderer、直接写 AgentAssetVersion、把未登记纹理标为已加载或把 unknown 标为原创。

退出：已为 MaterialDrawer 增加稳定 zone 选择、中文 zone 标签和显式“预览材质”动作；动作携带 `part_id`/`material_zone_id`，服务端拒绝不属于当前部件的 zone，仍由 ChangeSet preview → confirm 和 Snapshot/ETag 拥有永久写入。`npm run desktop:m105-material-zone-binding-smoke`、`npm run desktop:m104-material-zone-smoke`、`npm run desktop:f004-workbench-drawers-smoke`、`npm run agent:g6-asset-editing-smoke` 和 `npm run desktop:typecheck` 已通过。T002、r3、build、contracts 和 diff check 在交接回归中继续运行。当前确定性 blockout 多数只有一个 zone；多 zone 资产可通过同一合同接入，仍不允许猜测 zone 映射。

### FGC-M106 任务卡

状态：done（2026-07-13，本机脏工作区）。依赖 M105；把 `AgentMaterialPreset.allowed_domains` 接入当前 Domain Pack 的可组合筛选，避免零基础用户在汽车、飞机、机械臂或未来道具工作流中看到不适配的视觉预设。

范围：从当前 ActiveDesign/AgentAsset 的 `domain_pack_id` 得到领域上下文，在 MaterialDrawer 中增加“适合当前设计”默认筛选和“全部视觉材质”显式切换；兼容判断只能读取真实 `allowed_domains`，未知领域不猜测；搜索、六类分类和兼容筛选可叠加；选中 zone、来源/许可证和参数回退状态必须保持一致。

不得做：自动修改材质白名单、凭显示名称推断兼容性、工程材料结论、任意纹理下载、第二 renderer、绕过 ChangeSet/Snapshot，或把领域筛选当作质量/安全认证。

退出：已实现真实 `allowed_domains` 兼容筛选、四领域正/负组件 smoke、当前选中材质保留、全部材质显式切换和未知领域不猜测。`npm run desktop:m106-material-domain-filter-smoke`、M105、M104、F004、F006、T003、T002、r3、typecheck 和 build 已通过。Material Zone 的 Snapshot 持久化由已完成的 M107 接手。

### FGC-M107 任务卡

状态：done（2026-07-13，脏工作区）。依赖 M106；已把当前选中的 Material Zone 纳入 ActiveDesignSnapshot 的选择真值，使多 zone 编辑在刷新、撤销/重做和重启后仍与选中 Part 一致。

范围：扩展 Snapshot 合同、迁移、API 和 desktop reducer，增加可选 `selected_material_zone_id`；服务端验证该 zone 属于活动 Agent Part，legacy/外部 GLB 保持 null；MaterialDrawer 读取 Snapshot 选择并通过 CAS 更新；确认/取消/undo/redo/reload 后恢复相同 Part/zone。没有 zone 或 zone 不存在时安全回退到 null/首个真实 zone，不猜测。

不得做：把材质预设本身复制进 Snapshot、绕过 ETag、把 localStorage 当版本头、为 legacy 写入 zone、自动创建工程材料字段或改变第二 renderer。

交付：`selected_material_zone_id` 合同、0030 SQLite 迁移、API/CAS 所有权校验、前端 Snapshot reducer/MaterialDrawer 回写；版本切换在目标部件仍存在时保留部件和 zone，否则回退到首个真实 zone 或 null；legacy 仍为 null。

证据：`npm run agent:m107-active-zone-smoke`、`npm run agent:s8-active-design-navigation-smoke`、`npm run agent:s7-legacy-conversion-smoke`、`npm run desktop:m106-material-domain-filter-smoke`、`npm run desktop:m105-material-zone-binding-smoke`、S5、T002、r3、contracts、typecheck、build 和 diff check 通过。

退出：Snapshot/TS/Pydantic/迁移一致；四领域选区、stale revision、重启和 undo/redo 有测试；Material Zone 预览继续携带 part/zone。下一项可领取任务由索引中唯一 `ready` 项决定。

## 8. P2 数据、发布与外部事项

| ID | 状态 | 依赖 | 交付 | 最低 Gate |
| --- | --- | --- | --- | --- |
| FGC-B001 | done | 文档基线 | backup 枚举 `agent_imported_glbs.object_path` | backup/restore smoke |
| FGC-B002 | done | S002,B001 | Snapshot/Agent 资产完整恢复演练 | recovery gate |
| FGC-P001 | done | S008 | Python/Rust 单元测试与漏洞审计 | CI audit |
| FGC-P007 | done | P001 | 修复高危 Python 依赖并重跑 P001 审计 | pip-audit + CI audit |
| FGC-P008 | done | R006,P001 | packaged sidecar 输入与本机预检合同 | no-secret inventory + deterministic blocked/readiness report |
| FGC-P002 | done | P001,P008 | 目标平台非空 packaged sidecar | packaged Alpha launch + recovery |
| FGC-P003 | blocked | P002 | 全新机器安装/升级/卸载 | installer E2E |
| FGC-P009 | ready | P002 | macOS packaged Alpha 持续回归 | native supervisor smoke in macOS CI |
| FGC-P004 | external | D004,G807 | 真实 Provider 四领域评测 | live evaluation |
| FGC-P005 | external | M103 | 刘邦正式资产审阅和 attestation | formal review validate |
| FGC-P006 | external | P003-P005 | macOS/Windows 签名与发布 | release checklist |

### FGC-B001 任务卡

状态：done（2026-07-13，脏工作区）。备份对象枚举已加入 `agent_imported_glbs.object_path`，并在备份 manifest、校验、恢复和容量统计中保持来源表与内容哈希一致。

证据：`npm run agent:r3-library-backup-restore-smoke` 通过，覆盖 Agent imported GLB 对象复制、恢复、哈希校验、重复引用去重、篡改检测、额外对象检测、密钥/临时文件排除和 SQLite 外键检查。

`FGC-B001` 可以独立提前修复，因为它不改变活动设计状态合同。

### FGC-B002 任务卡

状态：done（2026-07-13，脏工作区）。在 B001 的 imported GLB 备份枚举之上，恢复 smoke 增加了 Agent head 和 `ActiveDesignSnapshot` 真值回读：恢复到新目录后通过 `/active-design` 校验 active asset 与 export source/version 同链，并保留 GLB 哈希/外键/失败路径保护。

必须读取：

- `docs/DISASTER_RECOVERY.md`
- `docs/AUTHORITATIVE_STATE.md`
- `scripts/library_backup.py`
- `scripts/smoke_library_backup_restore.py`

文件范围：恢复脚本、备份 smoke、灾难恢复文档和证据矩阵；不得改变 ActiveDesignSnapshot 合同或用户界面。

退出：恢复后的 Project、AgentAssetVersion、Snapshot、质量/选择引用和 imported GLB 对象哈希与源一致；损坏备份、跨项目对象和目标目录非空时安全失败；恢复不会覆盖源库。`npm run agent:r3-library-backup-restore-smoke` 已通过并输出 `restored_active_design_snapshot_verified: true`。

### FGC-P001 任务卡

状态：done（2026-07-13，脏工作区）。已补齐第一批 Python/Rust 单元测试、CI audit job 和 JSON 报告归档；依赖升级由 P007 完成并重新审计通过。

必须读取：

- `docs/TEST_STRATEGY.md`
- `docs/PRODUCTION_RELEASE_CHECKLIST.md`
- `docs/CODEX_DEFINITION_OF_DONE.md`

退出：关键应用服务与迁移至少有失败路径单元测试；`cargo audit`/等价 Rust 审计和 Python 依赖审计结果进入 CI artifact；发现高危项必须阻断，不得只记录警告。已由 `npm run agent:unit`、Rust `cargo test`、`pip-audit`、`cargo audit` 和 CI workflow 配置验证。

### FGC-P007 任务卡

状态：done（2026-07-13，脏工作区）。将最低 Python 版本提升到 3.10，锁定 FastAPI 0.139.0、Starlette 1.3.1、Uvicorn 0.51.0，重建本机运行环境并完成 Python/Rust smoke 与审计。不得降低审计等级、忽略 CVE、删除依赖条目，亦未扩展几何、Provider 或 UI。

必须读取：

- `docs/TEST_STRATEGY.md`
- `docs/PRODUCTION_RELEASE_CHECKLIST.md`
- `apps/agent/pyproject.toml`
- `apps/agent/requirements-release.lock`

退出：审计 JSON artifact 仍生成；Python 高危漏洞数为 0，依赖解析与 G1–G7 smoke 通过。已验证 Python 0 vulnerabilities、Rust 0 vulnerabilities、npm audit 通过。

### FGC-P008 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 R006、P001 已满足。已冻结并验证将 P002 从“empty sidecar”推进到本机 packaged Alpha 所需的输入清单和无密钥预检报告。

范围：读取现有 Tauri 配置、sidecar 路径约定、Rust/Cargo 工具链和 packaging gate；产出版本化 readiness contract，明确目标二进制、架构、健康检查、首次初始化、恢复和不含 Provider Key 的 secret-file 边界。预检必须能稳定区分 `ready_for_local_alpha` 与 `blocked_missing_sidecar`，并作为 P002 的前置证据。

不得做：不得构建或下载未知二进制、提交 Key、调用 Provider、签名、公证、上传、修改 Project/AgentAsset/Snapshot 真值，或把预检绿色说成安装包可对外发布。

交付：新增 `ForgeCADPackagedSidecarInput@1`、`release:packaged-sidecar-preflight` 与 smoke，并接入 backend CI 和既有 packaging readiness smoke。预检只读合同/二进制前 4 KiB；当前空 sidecar 稳定返回 `blocked_missing_sidecar`，临时正确 Mach-O arm64 输入返回 `ready_for_local_alpha`，错误 CPU 或 secret-like 合同输入拒绝。它不读取 Provider secret、不联网、不执行二进制或写入项目状态。

证据：`npm run release:packaged-sidecar-preflight-smoke`、`npm run release:packaged-sidecar-preflight`、`npm run release:packaging-readiness`（预期仍以空 sidecar 阻断）和 `git diff --check`。当前预检报告的目标为 `aarch64-apple-darwin`、`binaries/wushen-agent-aarch64-apple-darwin`、`agent serve` 与 `GET /api/health`。

退出：已满足。P002 现可领取，但不得仅复制文件或 header；必须产出真实冻结 runtime，并真实启动、健康检查、首次初始化、GLB 导出和重启恢复。预检绿色不等于签名、公证或外部发布。

### FGC-P002 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 P001、P008 已满足。当前 macOS arm64 frozen sidecar 已构建；真实 `.app` 会以 `packaged-sidecar` 方式启动 bundle 内的 sidecar。该结论仅限本机 macOS Alpha，不是安装、签名、公证或外部分发结论。

必须读取：

- `docs/PACKAGING.md`
- `docs/RELEASE_MAINTENANCE.md`
- `apps/desktop/src-tauri/binaries/sidecar-inputs.json`
- `scripts/packaged_sidecar_preflight.py`
- `apps/desktop/src-tauri/src/main.rs`

不得做：不得复制占位 header、把仓库/`.venv` 作为普通用户依赖、读取或提交 Provider Key、自动调用 Provider、宣称已签名/公证/可外部分发，或修改 Project/AgentAsset/Snapshot 真值。

证据：`npm run desktop:packaged-sidecar-build` 生成 arm64 Mach-O；`npm run release:packaged-sidecar-preflight -- --require-ready` 返回 `ready_for_local_alpha`；`npm run desktop:packaged-sidecar-alpha-smoke` 覆盖无 Provider 环境的 health、空资料库初始化、确定性机械臂可编辑 GLB 导出和重启读取。`npm --workspace apps/desktop run tauri -- build --bundles app` 生成 `.app`；`npm run desktop:packaged-tauri-alpha-smoke` 通过 LaunchServices 启动实际 bundle，验证 `mode=packaged-sidecar`、sidecar 是桌面进程后代、首次 Library 初始化、GLB 导出和重启恢复，输出 `provider_calls: 0`。另已通过真实工作台启动/关闭检查，确认正常窗口关闭不会遗留端口 8000 sidecar。

退出：已满足。`release:packaging-readiness` 仍会因未构建的其他发布目标 sidecar 而失败；该跨平台发布条件属于 P003/P006，不得由 P002 隐藏或删除。不得把 P002 Alpha smoke 说成完整安装、签名、公证或外部分发。

### FGC-P009 任务卡

状态：ready（2026-07-14，脏工作区，未提交）。依赖 P002 已满足；macOS arm64 CI job 已配置，但尚未获得远端 runner 的实际结果。

范围：将已有 macOS LaunchServices native supervisor smoke 固定为可复现的 macOS CI/专用构建机回归，收集不含密钥的失败日志和结果 JSON；不得改动 sidecar、Project/AgentAsset/Snapshot 真值、Provider 调用、签名或安装器。

最低 Gate：`npm run desktop:packaged-tauri-alpha-smoke`、`release:secrets-files`、`git diff --check`；失败时保留可读的无密钥证据。

退出：目标 macOS runner 稳定执行 bundle 构建和 native smoke；不将其误称为全新机器安装、签名、公证或跨平台发布。

### FGC-F001 任务卡

状态：done（2026-07-13，脏工作区）。`scripts/smoke_workbench_characterization_ui.mjs` 已在本机 Chrome 通过并登记到 CI；测试覆盖首次初始化、legacy 显式重建 hand-off、含糊输入澄清、预览不写盘、Agent 资产提交、Snapshot/导出一致、重启恢复和单 WebGL canvas。legacy starter 在未完成显式转换时保存仍返回 `ACTIVE_DESIGN_INVALID`，测试验证了该写入屏障和显式 hand-off，不得改成自动覆盖。不得把本任务的确定性 smoke 当作真实 Provider 质量评测。

必须读取：

- `docs/TEST_STRATEGY.md`
- `docs/AUTHORITATIVE_STATE.md`
- `apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx`
- `scripts/smoke_r3_concept_workbench_ui.mjs`

证据：`npm run desktop:f001-workbench-characterization`（本机 Chrome）通过，输出断言包括 `single_canvas`、`ambiguous_clarification_write_barrier`、`preview_does_not_write_version`、`agent_commit_snapshot_export_alignment`、`reload_restores_agent_head`、`legacy_rebuild_requires_explicit_handoff`。

退出：测试能在 CI 浏览器环境运行，失败路径有明确断言；当前 r3 Agent-first smoke 的版本链、单 WebGL 和 legacy 写入阻断断言不得被删除或放宽。已满足。

### FGC-F002 任务卡

状态：done（2026-07-13，脏工作区）。从 `CadWorkbenchPanel.tsx` 提取 `AgentConversation` 和独立 `AgentStepItem`，只通过 props 接收项目、Provider、输入、澄清、Kernel 步骤和方向预览状态；没有复制 Snapshot、版本或选择真值，也没有改动视口、几何、导出或 legacy 数据路径。

交付：

- `apps/desktop/src/features/cad-workbench/AgentConversation.tsx`：Agent 输入、Provider 配置、澄清、步骤和方向结果的视图边界；
- `apps/desktop/src/features/cad-workbench/AgentStepItem.tsx`：单个 Kernel Item 的类型标签和安全文本摘要；
- `apps/desktop/src/features/cad-workbench/AgentConversation.smoke.tsx`：无浏览器副作用的组件树 smoke；
- `scripts/smoke_agent_conversation_component.mjs` 与 `desktop:f002-agent-conversation-smoke`：临时 TypeScript 编译并检查澄清、步骤、方向、可访问输入标签。

证据：`npm run desktop:f002-agent-conversation-smoke`、`npm run desktop:typecheck`、`npm run desktop:build`、`npm run desktop:f001-workbench-characterization` 和 `npm run desktop:r3-concept-workbench-smoke` 均通过。Chrome/IAB 直接打开本机 Vite 页面被浏览器策略以 `ERR_BLOCKED_BY_CLIENT` 拒绝；因此仍以仓库 Playwright smoke 作为本机渲染证据，并在交接中保留该限制。

边界：SelectionCard、动作命令已由后续 F003 独立交付；Component/Material/Quality/Export drawers 仍由 F004 负责；不得在本任务中顺便重写视觉系统或状态机。

退出：Agent 对话与步骤 Item 有独立组件边界和可执行 smoke，F001/r3 行为断言保持通过。已满足。

### FGC-F003 任务卡

状态：done（2026-07-13，脏工作区）。从 `CadWorkbenchPanel.tsx` 提取 `AgentSelectionCard`，由组件负责分件候选列表、当前部件选择、受限比例/关节/材质动作、兼容组件替换、保存可复用部件和 ChangeSet 预览确认按钮；永久修改仍通过父层回调进入既有 API/ChangeSet，不复制 Snapshot、ETag 或版本头。

交付：

- `apps/desktop/src/features/cad-workbench/AgentSelectionCard.tsx`：分件选择卡与部件动作命令视图；
- `apps/desktop/src/features/cad-workbench/AgentSelectionCard.smoke.tsx`：覆盖角色选择、比例、关节、组件替换和检查入口的组件树 smoke；
- `scripts/smoke_agent_selection_card_component.mjs` 与 `desktop:f003-agent-selection-card-smoke`：临时 TypeScript 编译和 React 元素树验证。

证据：`npm run desktop:f003-agent-selection-card-smoke`、`npm run desktop:typecheck`、`npm run desktop:build`、`npm run desktop:f001-workbench-characterization` 和 `npm run desktop:r3-concept-workbench-smoke` 均通过。

边界：视觉材质目录、组件/材质/质量/导出抽屉由后续 F004 负责；本任务未增加自由参数、任意 split/merge 或后端能力。

退出：选择卡和动作命令有独立组件边界，所有动作仍遵循 preview → confirm → immutable version，F001/r3 行为基线保持通过。已满足。

### FGC-F004 任务卡

状态：done（2026-07-13，脏工作区）。从 `CadWorkbenchPanel.tsx` 提取 `ComponentDrawer`、`MaterialDrawer`、`QualityDrawer` 和 `ExportDrawer`。抽屉只接收当前 Snapshot 派生状态和父层回调；版本、ETag、ChangeSet、质量检查和导出副作用仍由父层拥有。

交付：

- `apps/desktop/src/features/cad-workbench/ComponentDrawer.tsx`：组件目录、筛选、缩略图、审阅/质量元数据、适配和替换预览入口；
- `apps/desktop/src/features/cad-workbench/MaterialDrawer.tsx`：视觉材质预设和细节密度控制，明确视觉材质边界；
- `apps/desktop/src/features/cad-workbench/QualityDrawer.tsx`：Agent/legacy 检查摘要、真实发现和检查动作；
- `apps/desktop/src/features/cad-workbench/ExportDrawer.tsx`：按用途选择导出目标，保持 Agent 资产只导出 GLB 的限制；
- `apps/desktop/src/features/cad-workbench/WorkbenchDrawers.smoke.tsx` 与 `scripts/smoke_workbench_drawers_component.mjs`：四类抽屉组件树 smoke；
- `desktop:f004-workbench-drawers-smoke`：F004 最低 Gate。

证据：`npm run desktop:f004-workbench-drawers-smoke`、`npm run desktop:typecheck`、`npm run desktop:build`、`npm run desktop:f001-workbench-characterization` 和 `npm run desktop:r3-concept-workbench-smoke` 均通过。首次 r3 回归发现并修复了 API URL 方法上下文丢失问题，修复后回归通过。

边界：本任务未实现自由 split/merge、复杂几何、真实 Provider 评测、任意格式 Agent 导出或前端状态机；F005 才负责进一步缩减 `CadWorkbenchPanel` 为组合层。

退出：四类抽屉有独立组件和 smoke，所有永久动作仍由父层进入既有 Snapshot/ChangeSet/版本链，F001/r3 行为基线保持通过。已满足。

### FGC-F005 任务卡

状态：done（2026-07-13，脏工作区）。依赖 F002、F003、F004。

目标：把 `CadWorkbenchPanel.tsx` 进一步缩减为组合层和生命周期协调层，统一由明确的 selector/props 连接 AgentConversation、AgentSelectionCard、四类抽屉、视口和确认条，不改变 Snapshot、ETag、ChangeSet、版本、质量或导出真值。

范围：

- 先运行 F001 characterization、F002/F003/F004 component smoke 和 r3 Agent-first smoke；
- 只移动已有状态/副作用的组合调用，不新增第二套状态机或 localStorage 版本头；
- 把重复的 `activeDesignSnapshot` 派生值收敛为稳定 selector/adapter；
- 使用现有 F001 characterization 与 r3 Agent-first E2E 作为组合层回归证据，覆盖已实现的抽屉打开/关闭、部件选择、检查、导出和重启恢复；未新增一套重复的端到端脚本；
- 保留单 WebGL canvas、legacy 只读/显式转换和 Agent 资产仅 GLB 导出边界。

明确不做：复杂 ShapeProgram、自由 split/merge、多视图、真实 Provider 评测、签名/公证和 packaged sidecar。

退出：已由 `WorkbenchDrawerStack.tsx` 集中组合四类抽屉，抽屉组件只接收父层提供的 Snapshot 派生 props 和副作用回调；`CadWorkbenchPanel` 仍保留 Agent/legacy 生命周期与状态，不能宣称已经是最终状态机。F001–F004 smoke、typecheck/build、现有工作台 characterization/r3 E2E 均通过；失败路径、取消、重启和 Snapshot 版本一致性仍由既有回归证据覆盖。已满足。

### FGC-F006 任务卡

状态：done（2026-07-13，脏工作区）。依赖 F005；完成零基础主流程的可访问性收敛，不改变 Snapshot、版本、选择、质量或导出真值。

目标：在不改变 CAD 深色视觉语言和单 WebGL 约束的前提下，完成零基础用户可用的可访问性收敛：最小字号、点击目标、键盘焦点、中文 `aria-label`、`aria-live` 状态和抽屉 Escape/焦点返回。

范围：先建立可执行 accessibility checks，再修正 Agent 对话、选择卡、四类抽屉、确认条和状态栏；不得顺便增加几何、导出格式或 Provider 能力。

交付：工作台最小窗口和控件尺寸基线、11px 辅助文字下限、可见键盘焦点、中文 `aria-label`/`aria-pressed`、Agent/错误/澄清状态 `aria-live`、抽屉 `role=dialog`、初始焦点、Escape 关闭与触发控件焦点返回、键盘调整组件库高度。

证据：`npm run desktop:f006-accessibility-smoke`、`npm run desktop:f004-workbench-drawers-smoke`、`npm run desktop:typecheck`、`npm run desktop:build`、`npm run desktop:r3-concept-workbench-smoke` 和 `git diff --check` 通过。r3 额外覆盖质量/组件抽屉初始焦点、Escape 关闭和导出关闭后的焦点返回。

边界：未实现完整屏幕阅读器人工验收、多客户端压力、复杂几何或最终状态机；这些仍由后续任务负责。

退出：键盘路径、焦点可见性、屏幕阅读器标签、加载/错误/澄清状态通知和最小点击目标均有自动化证据；F001–F005 回归保持通过。已满足。

### FGC-F007 任务卡

状态：done（2026-07-13，脏工作区）。依赖 F006、T003 均已满足。`useWorkbenchLifecycle` 现集中请求编号、取消/乱序响应屏障、既有错误映射和单一抽屉/焦点状态；`CadWorkbenchPanel` 仍拥有 API 调用、Snapshot hydration、ETag、ChangeSet、质量和下载副作用。

目标：从 `CadWorkbenchPanel.tsx` 提取一个只负责工作台生命周期协调的适配层，明确管理异步加载、取消、错误映射与抽屉触发后的派生 UI 状态；不移动 `ActiveDesignSnapshot`、ETag、ChangeSet、版本、选择、质量或下载副作用的真值归属。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/FRONTEND.md`、`docs/TEST_STRATEGY.md`、`apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx`、`apps/desktop/src/features/cad-workbench/activeDesignMachine.ts`、F001/T002/T003 smoke。

范围：先为当前父层的生命周期/请求状态写 focused state smoke，再抽出无副作用的 selector、adapter 或 reducer；父层仍是唯一 API 调用和下载副作用入口。保持 legacy 只读/显式转换、单 WebGL renderer、R005 Agent-only 下载 UI 和当前浏览器行为不变。

不得做：不得新增领域、几何、Provider、导出格式、localStorage 版本头、第二状态真值、第二 renderer 或重写完整布局；不得删除 r3、T002、T003 或 F001 断言。

证据：新增 `desktop:f007-workbench-lifecycle-smoke`，覆盖单一抽屉、关闭和请求新旧判定；`desktop:s5-active-design-machine-smoke` 补充取消后保留 Snapshot、迟到响应拒绝和后续真实错误边界。`desktop:typecheck`、`desktop:build`、F001、F002–F007、T002、T003、r3、`contracts:types:check`、`agent:check`、文档/安全 Gate 与 `git diff --check` 全部通过。CI desktop job 已接入 F007。F006 同轮发现并修正一处 10px 辅助文字为 11px，未放宽可访问性断言。

退出：focused 生命周期状态、取消、乱序响应和抽屉互斥均有自动化证据；父层 API/下载归属、单 renderer 和既有浏览器行为保持不变。已满足。

### FGC-F008 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F007 已满足。`useAgentConversationPresentation` 现集中输入草稿、模式、提示、项目内 thread、Kernel items、澄清和方向卡的短生命周期展示状态；项目切换会原子清空这些状态，并用 project/request 双重屏障拒绝迟到响应。父层仍拥有 Agent API/SSE 调用、legacy fallback、blockout/分件、确认、ChangeSet、质量、导出与 `ActiveDesignSnapshot` 真值。

目标：从 `CadWorkbenchPanel.tsx` 提取 Agent 会话的瞬态展示状态（输入草稿、方向预览和澄清呈现），使项目切换可原子复位且 UI 状态可独立测试；不移动 Agent 请求、`ActiveDesignSnapshot`、版本、选择、质量或导出的真值。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/FRONTEND.md`、`docs/TEST_STRATEGY.md`、`apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx`、`apps/desktop/src/features/cad-workbench/AgentConversation.tsx`、`apps/desktop/src/features/cad-workbench/useWorkbenchLifecycle.ts`、D003/F002/F007/T002 smoke。

范围：先为输入、澄清和方向预览的 project-reset/未写入边界建立 focused state smoke；再抽出小型 reducer、selector 或 hook。父层继续是唯一的 API、SSE、确认、ChangeSet、质量和下载副作用入口；未知/含糊领域继续在用户选择前零 Plan/Blockout/Version/Asset 写入。

不得做：不得新增 Provider、领域、几何、第二对话线程真值、localStorage 版本头、自动确认、自由参数、第二 renderer 或布局重写；不得改变 D003 零写入、r3、T002、T003、F001–F007 断言。

证据：新增 `desktop:f008-agent-conversation-state-smoke`，覆盖输入/线程/方向/澄清的 project-reset、旧项目迟到 Turn 和同项目已取消 Turn 拒绝，以及会话 reducer 不拥有 asset-version 字段；CI desktop job 已接入。`desktop:typecheck`、`desktop:build`、F001、F002、F007、D003、T002（12/12）、T003、r3 通过；T003 仍确认仅一个 canvas/context、renderer generation 稳定且 bundle 在既有 Alpha 预算内。F008 不改变 D003 写入屏障、单 WebGL renderer 或 Agent 资产版本链。

退出：项目切换、澄清、方向呈现、取消和迟到响应均有 focused 与浏览器回归证据；API/SSE、Snapshot、版本、质量和下载归属保持不变。已满足。

### FGC-F009 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F008 已满足。`useAgentBlockoutDisplay` 现集中 GLB、ShapeProgram、分件候选和方向预览加载的显示缓冲；它只承载当前候选或已 hydration 资产的视口投影，不保存 AgentAssetVersion、Snapshot、ChangeSet、质量或导出 ID。项目切换、方向重选和 project/request 过期边界均会拒绝旧显示结果。

目标：从 `CadWorkbenchPanel.tsx` 提取 Agent blockout 候选的短生命周期展示协调（GLB、ShapeProgram、分件候选与方向预览加载状态），确保项目切换、方向重选和迟到响应不会污染当前项目；不移动 `AgentAssetVersion`、`ActiveDesignSnapshot`、版本提交或 ChangeSet 真值。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/FRONTEND.md`、`docs/TEST_STRATEGY.md`、`apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx`、`apps/desktop/src/features/cad-workbench/useAgentConversationPresentation.ts`、F008/T002/T003/r3 smoke。

范围：先为 blockout 候选清空、方向重选、分件失败和迟到 build/segment 响应建立 focused state smoke；再抽出纯 reducer/adapter。父层继续是唯一 `buildAgentBlockout`/`segmentAgentBlockout`/`commitAgentBlockout` API 调用和持久写入入口；确认后的 AgentAssetVersion 继续只从现有 Snapshot/asset 路径读取。

不得做：不得把候选 blockout 冒充已提交资产，不得增加几何操作、Provider、自由参数、导出格式、第二 renderer、localStorage 版本头或自动确认；不得改变 D003、F001–F008、T002、T003、r3 的写入/版本/单 canvas 断言。

证据：新增 `desktop:f009-agent-blockout-display-state-smoke`，覆盖方向重选清空旧候选、旧 segmentation/旧项目 build 拒绝、分件失败只保留未提交外观且状态不拥有 asset-version/Snapshot 字段；CI desktop job 已接入。`desktop:typecheck`、`desktop:build`、F001、D003、T002（12/12）和 T003 通过，且 T003 继续确认单 canvas/context、renderer 稳定和 Alpha bundle 预算通过。后续 F010 修复了 r3 在 C104 重启后的 UI hydration/动作就绪竞态：r3 现保持锁定重启、隔离、隐藏/恢复与单 canvas 断言，不删除或放宽持久化断言。父层仍是 `buildAgentBlockout`、`segmentAgentBlockout`、`commitAgentBlockout` API/持久写入唯一入口。

退出：候选清空、方向重选、分件失败、取消和迟到响应均有 focused 与浏览器回归证据；候选不冒充资产，Snapshot/版本/质量/导出真值保持不变。已满足。

### FGC-F010 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F009 已满足。新增 `agentAssetWorkspaceState` 与 `useAgentAssetWorkspace`，将当前 Snapshot 已选 Agent 资产的读取投影、选中部件、质量摘要和导航摘要从 `CadWorkbenchPanel` 提取；project/source/request 过期读取不会写回。该 Hook 是缓存，不拥有 asset head、Snapshot revision、ETag、ChangeSet、质量写入或导出身份。

目标：从 `CadWorkbenchPanel.tsx` 提取已提交 Agent 资产的工作区协调（读取的 asset projection、选中部件、质量摘要和导航摘要），使 hydration 与项目切换可测试；不把这些投影变成独立版本真值，也不移动 Snapshot/ETag/CAS、ChangeSet、质量检查或导出 API。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/FRONTEND.md`、`docs/TEST_STRATEGY.md`、`apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx`、`apps/desktop/src/features/cad-workbench/useAgentBlockoutDisplay.ts`、S005/F007/F009/T002/r3 smoke。

范围：先为 active asset hydration、Snapshot source 切换、选中部件复位、质量摘要清空和迟到读取响应建立 focused state smoke；再抽出纯 reducer/adapter。父层继续发起 `getAgentAssetVersion`、quality/navigation 读取、Snapshot CAS、ChangeSet、undo/redo 和导出；Hook 不保存 head、revision 或 ETag。

不得做：不得把 `AgentAssetVersion` cache 当作版本 head，不得恢复 localStorage 版本头、增加 API/几何/Provider/导出格式或第二 renderer；不得改变 S005、D003、F001–F009、T002、T003、r3 的版本链与单 canvas 断言。

证据：新增 `desktop:f010-agent-asset-workspace-state-smoke`，覆盖当前 Snapshot hydration、资产 source 切换、旧资产 selection 拒绝、项目切换清空和质量/导航迟到读取拒绝，并断言 reducer 不拥有 Snapshot revision 或 asset-version head 字段；CI desktop job 已接入。`desktop:f003-agent-selection-card-smoke`、F008、F009、F010、`desktop:typecheck`、`desktop:build`、T002（12/12）、T003 和 r3 通过。r3 进一步验证 hydration 后动作只在 Snapshot 已选资产一致时启用，锁定重启、隔离、隐藏/恢复和 Agent asset v5 重启恢复均保持通过。文档、合同和安全 Gate 记录于本轮交接。最低 Gate 已满足。

### FGC-F011 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F010 已满足。新增 `legacyCompatibilityDisplay` 与 `LegacyCompatibilityNotice`，把 legacy source 的只读说明、重建引导与动作就绪状态从 `CadWorkbenchPanel` 的 Agent 会话显示中提取。显示模型只由当前 Snapshot source 与 operation 派生，不保存转换授权、版本头、revision、ETag 或任何写入能力。

目标：从 `CadWorkbenchPanel.tsx` 提取 legacy Concept 的只读兼容显示边界（来源说明、显式“让 Agent 重建可编辑资产”引导与旧格式不可用于 Agent 的提示），使 Agent-first 主路径不再混杂 legacy 展示判断；不改变 legacy 迁移授权、Snapshot、版本、质量、导出或任何 API 写入。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/COMPATIBILITY_MIGRATION.md`、`docs/FRONTEND.md`、`docs/TEST_STRATEGY.md`、`apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx`、`activeDesignMachine.ts`、S007/S008/F001/F010/T002/r3 smoke。

范围：先为 legacy source、显式转换可用性、Agent source 隔离和项目切换建立 focused compatibility-display smoke；再提取纯显示组件或 adapter。父层继续唯一发起 Snapshot GET、legacy conversion authorization、CAS、质量、导出和所有写入；组件不得保存版本头、ETag、转换授权或 localStorage 状态。

不得做：不得恢复旧武神导航、任务中心、Mode 或独立资产库；不得让 legacy Concept 重新可写、让 Agent 导出回退旧格式、增加转换 API、删除 legacy 写入屏障或改变单 WebGL renderer；不得借此增加几何、Provider 或导出格式。

证据：新增 `desktop:f011-legacy-compatibility-display-smoke`，覆盖空 Snapshot、legacy 只读提示、转换动作就绪、Agent source 隔离以及显示模型不拥有 revision/asset-version 字段；CI desktop job 已接入。F002、F011、`desktop:typecheck`、`desktop:build`、F001、T002（12/12）、T003 和 r3 通过；浏览器回归继续验证 legacy 显式 hand-off 写入屏障、Agent-first 版本/导出一致、重启恢复和单 WebGL canvas。合同/文档/安全 Gate 记录于本轮交接。最低 Gate 已满足。

### FGC-F012 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F011 已满足。新增 `componentLibraryPreferencesState` 与 `useComponentLibraryPreferences`，将组件库的分类、关键词、审阅状态筛选、收藏、最近使用、抽屉模式和高度提取为按 Project+Domain Pack 隔离的本机偏好。它只过滤真实资产元数据；不保存资产、审阅、质量、Snapshot、ETag、ChangeSet 或导出真值。

目标：从 `CadWorkbenchPanel.tsx` 提取组件库的本机偏好协调（目录分类、关键词、审阅状态筛选、收藏、最近使用和抽屉高度），使项目切换与偏好 key 的读取/保存可测试；不将偏好写入 Asset Pack、Project、AgentAssetVersion、Snapshot、审计或导出。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/FRONTEND.md`、`docs/TEST_STRATEGY.md`、`docs/ASSET_AUTHORING.md`、`apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx`、`ComponentDrawer.tsx`、F004/F006/F011/T002/T003/r3 smoke。

范围：先为 project/pack preference key 切换、收藏/最近使用的有界去重、目录与状态组合筛选、抽屉尺寸边界和 localStorage 缺失/损坏回退建立 focused smoke；再抽出纯 reducer/adapter。父层继续读取真实资产元数据、质量摘要与缩略图，并继续唯一拥有组件替换 ChangeSet、Snapshot CAS 和所有 API。

不得做：不得把收藏/最近使用同步进资产 Pack 或版本审计；不得伪造审阅、许可证、质量或兼容结论；不得恢复独立资产库页面、增加第二 renderer、改动 Asset Catalog API、组件替换语义、导出、Provider 或几何能力。

证据：新增 `desktop:f012-component-library-preferences-smoke`，覆盖 Project+Domain Pack preference key 隔离、损坏 localStorage 回退、收藏/最近使用去重和长度边界、抽屉高度边界、持久化 round-trip，以及目录/关键词/审阅状态组合筛选。CI desktop job 已接入。F004、F006、F012、`desktop:typecheck`、`desktop:build`、T002（12/12）、T003 和 r3 通过；目录、审阅和质量仍由现有资产元数据/质量读取，不因本机偏好改变。合同/文档/安全 Gate 记录于本轮交接。最低 Gate 已满足。

### FGC-F013 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F012 已满足。

目标：从 `CadWorkbenchPanel.tsx` 提取本机视口显示偏好协调（工具选择、网格、线框、X 光、Connector 显示、爆炸系数与截面偏移），使安全读取/恢复、边界和项目切换可测试；不移动 Snapshot 绑定的相机/灯光预设、测量标注、选择、质量、版本或导出。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/FRONTEND.md`、`docs/TEST_STRATEGY.md`、`apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx`、`ModuleGraphViewport.tsx`、`activeDesignMachine.ts`、R001/F006/F012/T002/T003/r3 smoke。

范围：先为 localStorage 缺失/损坏回退、布尔开关、工具白名单、爆炸/截面数值边界和项目切换不创建版本建立 focused smoke；再抽出纯 reducer/adapter。`cameraView` 和 `lightPreset` 继续通过现有 Snapshot/CAS 路径保存；测量标注继续使用既有项目/版本 key；父层继续拥有视口 props、Snapshot API、所有 ChangeSet 与导出。

不得做：不得把本机偏好写入 Snapshot、版本或导出；不得新增第二 WebGL renderer、自由几何、工程测量/尺寸、相机/灯光 API、Provider 或导出格式；不得把显示开关伪装为质量、审阅、制造或安全结论。

交付：新增 `viewportDisplayPreferencesState` 与 `useViewportDisplayPreferences`。`CadWorkbenchPanel` 的 v6 通用 session 不再保存视口显示、相机或灯光；项目切换只读取 `forgecad.viewport-display.preferences.v1.<project_id>` 的本机显示偏好。相机与灯光继续在既有 `refreshActiveDesign`/`updateRenderPreset` Snapshot/CAS 路径中读取和写入。

证据：新增 `desktop:f013-viewport-display-preferences-smoke` 并接入 desktop CI，覆盖项目隔离、缺失/损坏 localStorage 回退、工具白名单、布尔显示开关、爆炸/截面边界、round-trip，以及本机状态不含资产、版本、选择、质量、导出、相机或灯光字段。`desktop:f013-viewport-display-preferences-smoke`、`desktop:typecheck` 与 `git diff --check` 已通过；完整回归、R001/F006/F012/T002/T003/r3、合同/文档/安全 Gate 记录于本轮交接。

退出：满足后才可领取下一项任务；相机/灯光仍由 Snapshot CAS 读取，local preference 切换不改变 asset version、selection、quality、export 或 renderer 数量。

### FGC-F014 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F013 已满足。

目标：将旧 `ModuleGraph` 兼容路径的本机工作区会话（inspector tab、legacy 图节点/模块定位、变换坐标/吸附偏好与测量模式）从 `CadWorkbenchPanel.tsx` 提取，使安全读取、项目隔离、图变更后的失效选择和损坏 localStorage 回退可测试。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/COMPATIBILITY_MIGRATION.md`、`docs/FRONTEND.md`、`docs/TEST_STRATEGY.md`、`apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx`、`activeDesignMachine.ts`、F010/F011/F013/T002/T003/r3 smoke。

范围：先为缺失/损坏 localStorage 回退、Project 隔离、图节点失效时安全清空、变换/吸附/测量模式白名单，以及 Agent source 不读取或写入 legacy session 建立 focused smoke；再抽出纯 reducer/adapter。既有项目/版本 measurement annotation key 保持原位；Agent 资产选择、Snapshot、ETag、质量、版本、ChangeSet、导出、相机/灯光与 renderer 仍由既有服务端/父层路径拥有。

不得做：不得把 legacy session 提升为 Snapshot 或 asset head；不得更改 legacy→Agent 显式转换授权、测量标注格式、ChangeSet、导出、Provider、几何、相机/灯光 API 或增加第二 renderer；不得让 Agent source 使用本机 legacy 节点选择替代 Snapshot part selection。

交付：新增 `legacyModuleGraphWorkspaceState` 与 `useLegacyModuleGraphWorkspace`。旧全局 `forgecad.cad.session.v6` 读写已删除；legacy ModuleGraph 的 inspector tab、节点/模块定位、变换坐标/吸附与测量模式按 Project 保存。读取到图后 reducer 会只从现有节点中恢复有效选择；Agent source 打开 `null` preference context，既不读取也不写入该 legacy session。

证据：新增 `desktop:f014-legacy-module-graph-workspace-smoke` 并接入 desktop CI，覆盖 Project 隔离、缺失/损坏 localStorage 回退、节点失效后的安全回退、变换/吸附/测量模式、round-trip、Agent source 不读写 legacy session，以及状态字段不含资产、版本、Snapshot、质量、导出、相机或测量标注。F010、F011、F013、F014、F006、`desktop:typecheck`、`desktop:build`、T002（12/12）、T003、r3、contracts、agent check 与 `git diff --check` 已通过；合同/文档/安全 Gate 记录于本轮交接。

退出：满足；Agent source 下的当前资产、选择、质量和导出仍从同一 Snapshot 读取。

### FGC-F015 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F014 已满足。

目标：从 `CadWorkbenchPanel.tsx` 提取 legacy ModuleGraph 的纯展示叠层状态（本机隐藏节点、聚焦节点、质量高亮节点/几何引用和缩略图失败记录），使项目/图/质量变更后的清空、过期节点过滤和单视口复用可测试。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/COMPATIBILITY_MIGRATION.md`、`docs/FRONTEND.md`、`docs/TEST_STRATEGY.md`、`apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx`、`ModuleGraphViewport.tsx`、F010/F011/F013/F014/T002/T003/r3 smoke。

范围：先建立 focused smoke，覆盖隐藏/聚焦/质量叠层的纯显示边界、图节点失效过滤、项目或 legacy graph 改变后的清空、缩略图失败的本机短暂性和 Agent Snapshot part display 隔离；再抽出纯 reducer/adapter。父层继续唯一拥有 Quality API、Snapshot/CAS、Agent part display、ChangeSet、导出和 renderer props 的装配。

不得做：不得把隐藏/聚焦/质量叠层写入 Agent Snapshot 或版本；不得把 legacy `hiddenNodeIds` 与 Agent `part_display` 合并；不得伪造质量结果、增加 renderer、改变测量、导出、Provider、几何或 legacy 转换授权。

交付：新增 `legacyModuleGraphOverlayState` 与 `useLegacyModuleGraphOverlay`。该 reducer 仅持有 legacy project+graph context 的隐藏节点、聚焦节点、质量高亮/几何引用和有界缩略图失败记录；它不写入 localStorage。项目、旧图或 Agent source context 改变时会清空临时叠层；图重载只保留当前图仍存在的节点。Agent source 的空 context 会拒绝 legacy 叠层动作，`ModuleGraphViewport` 继续从同一个 renderer 接收 legacy overlays 与独立的 Snapshot `part_display` props。

证据：新增 `desktop:f015-legacy-module-graph-overlay-smoke` 并接入 desktop CI，覆盖 project/graph context 隔离、默认隐藏节点、失效节点/几何引用过滤、质量叠层清理、图切换清空缩略图失败，以及 Agent source 不接收 legacy 隐藏/聚焦状态。F010、F011、F013、F014、F015、F006、`desktop:typecheck`、`desktop:build`、T002（12/12）、T003、r3、contracts、agent check 与文档/安全 Gate 通过；质量结果、Snapshot、版本、导出和 Agent `part_display` 仍未进入该状态层。

退出：满足；legacy 图叠层只改变当前单视口的短暂显示，不创建版本或质量结论，且 Agent source 的显示仍以同一 Snapshot 为准。

### FGC-F016 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F015 已满足。

目标：从 `CadWorkbenchPanel.tsx` 提取 Agent 四视图/概念图包的短暂请求与展示状态（当前 render-set、加载中、下载包请求中），使 asset version/fingerprint 改变、迟到结果和抽屉关闭后的显示边界可测试。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/FRONTEND.md`、`docs/TEST_STRATEGY.md`、`docs/API.md` 的 Agent render API、`CadWorkbenchPanel.tsx`、`ExportDrawer.tsx`、R002–R005/F010/F015/T002/T003/r3 smoke。

范围：先建立 focused smoke，覆盖只接受当前 project、Agent asset version 与 render fingerprint 的结果；asset/source 切换清空旧图；加载状态不成为导出身份；概念图包只展示服务端已返回的 PNG/manifest。再抽出纯 reducer/adapter。父层继续唯一拥有 render API、Snapshot/CAS、直接 GLB 下载、浏览器下载副作用、质量、ChangeSet、导出与 renderer。

不得做：不得把 render-set、下载包、loading 或图片 URL 写入 Snapshot、版本、localStorage 或导出审计；不得把 PNG 当成工程渲染、模型源文件或质量结论；不得增加第二 WebGL renderer、Provider、几何、导出格式或原生下载授权。

交付：新增 `agentRenderPresentationState` 与 `useAgentRenderPresentation`。该内存层以 project + 当前 Agent asset version 为 context，只保存当前 `AgentAssetRenderSet`、渲染/图包请求状态与递增 request ID；切换资产或 source 时清空旧图，关闭抽屉会取消未完成请求并拒绝迟到响应。图包请求必须使用当前 render-set fingerprint；PNG/ZIP 下载、Render API、Snapshot/CAS、GLB 导出、质量和导出审计仍由父层/服务端拥有。

证据：新增 `desktop:f016-agent-render-presentation-smoke` 并接入 desktop CI，覆盖 project/asset context、错误资产结果拒绝、当前 fingerprint 图包约束、资产切换清空、图包迟到响应拒绝、关闭抽屉取消渲染，以及状态不含 Snapshot、quality、ChangeSet、export、renderer、图片 URL 或 asset head。R002–R004、F010、F015、F016、F006、`desktop:typecheck`、`desktop:build`、T002（12/12）、T003、r3、contracts、agent check 与文档/安全 Gate 通过；R005 的原生 WebView 下载人工验收边界未改变。

退出：满足；概念图只作为当前 Agent 资产的只读派生展示，当前 fingerprint 不一致或资产切换时不能下载，且不创建版本、质量或导出记录。

### FGC-F017 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F016 已满足。

目标：从 `CadWorkbenchPanel.tsx` 提取当前 Agent 资产的组件替换候选与事实驱动结构建议的只读加载状态，使 project、asset version、选中 Part 和迟到读取的边界可测试。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/FRONTEND.md`、`docs/TEST_STRATEGY.md`、`docs/API.md` 的 `components:compatible`/`structure-suggestions`、`CadWorkbenchPanel.tsx`、`AgentSelectionCard.tsx`、C102/C103/F010/F016/T002/T003/r3 smoke。

范围：先建立 focused smoke，覆盖仅接收当前 project、Agent asset version 与选中 Part 的候选；source/project/asset/selection 切换清空旧读取；结构建议不可用说明不伪造建议；迟到成功或失败均不覆盖新上下文。再抽出纯 reducer/adapter。父层继续唯一拥有候选/建议 API、preview→confirm ChangeSet、Snapshot/CAS、质量、导出、组件保存和 renderer。

不得做：不得把候选、建议、不可用说明或加载状态写入 Snapshot、版本、localStorage、组件目录或审计；不得把事实驱动候选描述为工程结构、功能、制造或安全结论；不得修改替换条件、拆分/合并算法、Provider、几何、导出或增加 renderer。

最低 Gate：新增 focused edit-assist-presentation smoke；`desktop:typecheck`、`desktop:build`、C102、C103、F010、F016、F006、T002、T003、r3、合同/文档/安全 Gate 全部通过；候选/建议仍只读，永久操作仍严格 preview→confirm。

交付：新增 `agentEditAssistPresentationState` 与 `useAgentEditAssistPresentation`。该内存层以 project + 当前 Agent asset version + 当前选中 Part 为 context，只保存经过当前 asset/part 过滤的组件候选、事实驱动结构建议、服务端不可用说明、loading 与递增 request ID；切换 source/project/asset/selection 时清空，迟到成功或失败均被忽略。父层继续唯一拥有候选/建议 API、组件保存、preview→confirm ChangeSet、Snapshot/CAS、质量、导出和 renderer。

证据：新增 `desktop:f017-agent-edit-assist-presentation-smoke` 并接入 desktop CI，覆盖 asset/part 过滤、selection 切换清空、迟到读取拒绝、失败只显示“暂时无法读取”而不伪造建议，以及状态不含 Snapshot、quality、ChangeSet、export、asset head 或 renderer。`agent:c102-component-compatibility-smoke`、`agent:c103-structure-suggestions-smoke`、F010、F016、F006、F003、`desktop:typecheck`、`desktop:build`、T002（12/12）、T003、r3、contracts、agent check 与文档/安全 Gate 通过。

退出：满足；候选和建议始终是当前 Agent 资产与当前 Part 的只读辅助信息，永久替换、拆分或合并仍只能从父层发起 preview→confirm。

### FGC-F018 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F017 已满足。

目标：从 `CadWorkbenchPanel.tsx` 提取视觉材质目录的只读读取与加载/失败展示状态，使 project、domain pack、当前 Agent asset 和迟到目录响应边界可测试。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/FRONTEND.md`、`docs/TEST_STRATEGY.md`、`docs/API.md` 的视觉材质接口、`CadWorkbenchPanel.tsx`、`MaterialDrawer.tsx`、M101–M107/F010/F017/T002/T003/r3 smoke。

范围：先建立 focused smoke，覆盖仅接收当前 project/domain pack 的目录结果、source/project/asset/domain 切换清空旧结果、迟到成功或失败不覆盖新 context、无目录时仅显示真实不可用状态。再抽出纯 reducer/adapter。父层继续唯一拥有 Material Zone 选择、preview→confirm ChangeSet、Snapshot/CAS、质量、导出和 renderer。

不得做：不得把材质目录、loading、失败说明或筛选结果写入 Snapshot、版本、localStorage、审计或质量报告；不得新增工程材料属性、真实制造建议、自由 PBR 编辑、外部 URL/文件路径、几何、Provider、导出或第二 renderer。

最低 Gate：新增 focused material-catalog-presentation smoke；`desktop:typecheck`、`desktop:build`、M101–M107、F010、F017、F006、T002、T003、r3、合同/文档/安全 Gate 全部通过；永久材质变更继续严格 preview→confirm。

交付：新增 `agentMaterialCatalogPresentationState` 与 `useAgentMaterialCatalogPresentation`。该内存层以 project + 当前 Agent asset version + domain pack + source 为 context，只保存当前视觉材质目录、loading、真实目录说明与递增 request ID；project/asset/domain/source 切换即清空，迟到结果不能写回。服务目录读取失败时只展示已提供的本机内置视觉预设及真实回退说明；没有回退预设时才显示目录不可用。父层继续唯一拥有 Material Zone、preview→confirm ChangeSet、Snapshot/CAS、质量、导出和 renderer。

证据：新增 `desktop:f018-agent-material-catalog-presentation-smoke` 并接入 desktop CI，覆盖 context 切换清空、迟到结果拒绝、已提供内置预设的事实回退、状态字段不含 Snapshot/选择/质量/ChangeSet/导出/asset head/renderer。M101–M107、F010、F017、F006、`desktop:typecheck`、`desktop:build`、T002（12/12）、T003、r3、contracts、agent check 与文档/安全 Gate 通过。

退出：满足；材质目录始终是当前工作台 context 的只读视觉资料，选区、预览和确认型材质修改继续严格受 Snapshot 与 preview→confirm 约束。

### FGC-F019 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F018 已满足。

目标：从 `CadWorkbenchPanel.tsx` 提取视觉材质抽屉的查询、分类与“适合当前设计”筛选展示状态，使 Project/domain/source 切换和不兼容选项的显示边界可测试。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/FRONTEND.md`、`docs/TEST_STRATEGY.md`、`MaterialDrawer.tsx`、M104–M107/F012/F018/T002/T003/r3 smoke。

范围：先建立 focused smoke，覆盖新 context 清空筛选、分类/关键词/适配筛选组合、不兼容当前领域只影响展示而不重写选中 Material Zone、外部 GLB/legacy 的禁用边界。再抽出纯 reducer/adapter。父层继续唯一拥有选中材质、Material Zone、preview→confirm ChangeSet、Snapshot/CAS、质量、导出和 renderer。

不得做：不得把筛选状态写入 Snapshot、版本、审计或质量报告；不得改变领域兼容规则、材质目录、Material Zone、工程材料字段、外部 URL/文件路径、Provider、几何、导出或第二 renderer。

最低 Gate：新增 focused material-filter-presentation smoke；`desktop:typecheck`、`desktop:build`、M104–M107、F012、F018、F006、T002、T003、r3、合同/文档/安全 Gate 全部通过；永久材质变更继续严格 preview→confirm。

交付：新增 `agentMaterialFilterPresentationState` 与 `useAgentMaterialFilterPresentation`。该内存层以 project + domain pack + source 为 context，只保存关键词、分类与适配筛选开关；切换 context 即恢复默认筛选，不保存 selected material 或 Material Zone。外部 GLB 和 legacy source 的材质抽屉保持只读禁用，领域不兼容只影响显示筛选，不重写当前选区或版本。

证据：新增 `desktop:f019-agent-material-filter-presentation-smoke` 并接入 desktop CI，覆盖筛选组合、context 切换清空、同一 context 保持、状态字段不含 selected material/Material Zone/Snapshot/version/quality/ChangeSet/export/renderer。M104–M107、F012、F018、F006、`desktop:typecheck`、`desktop:build`、T002（12/12）、T003、r3、contracts、agent check 与文档/安全 Gate 通过。

退出：满足；筛选状态只改变当前抽屉可见的视觉材质，不能写入或覆盖 Material Zone、Snapshot、资产版本或预览确认路径。

### FGC-F020 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F019 已满足。

目标：从 `CadWorkbenchPanel.tsx` 提取材质预选的短暂展示状态，使当前预览、选中 Part、source 切换和 preview/confirm 边界可测试。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/FRONTEND.md`、`docs/TEST_STRATEGY.md`、`MaterialDrawer.tsx`、M104–M107/F018/F019/T002/T003/r3 smoke。

范围：先建立 focused smoke，覆盖仅为当前 Agent 或 blockout 预览保存材质预选、Part/source/asset 切换清空、外部 GLB/legacy 禁用、预选不写 Material Zone 或 Snapshot。再抽出纯 reducer/adapter。父层继续唯一拥有 Material Zone、preview→confirm ChangeSet、Snapshot/CAS、质量、导出和 renderer。

不得做：不得把预选状态写入 Snapshot、版本、审计或质量报告；不得改变 Material Zone、领域兼容规则、工程材料字段、外部 URL/文件路径、Provider、几何、导出或第二 renderer。

最低 Gate：新增 focused material-preselection-presentation smoke；`desktop:typecheck`、`desktop:build`、M104–M107、F018、F019、F006、T002、T003、r3、合同/文档/安全 Gate 全部通过；永久材质变更继续严格 preview→confirm。

交付：新增 `agentMaterialPreselectionPresentationState` 与 `useAgentMaterialPreselectionPresentation`。该内存层以 project + asset version + selected Part + source 为 context，只保存当前视觉预选材质；project/asset/Part/source 切换恢复默认，外部 GLB/legacy source 拒绝预选。父层继续唯一拥有 Material Zone、preview→confirm ChangeSet、Snapshot/CAS、质量、导出和 renderer。

证据：新增 `desktop:f020-agent-material-preselection-presentation-smoke` 并接入 desktop CI，覆盖当前预选、Part 切换清空、外部 GLB 禁用和状态字段不含 Material Zone/Snapshot/version/quality/ChangeSet/export/renderer。M104–M107、F018、F019、F006、`desktop:typecheck`、`desktop:build`、T002（12/12）、T003、r3、contracts、agent check 与文档/安全 Gate 通过。

退出：满足；预选只影响当前视觉预览，不能自身创建、确认或恢复材质修改，也不覆盖任何 Snapshot 选择。

### FGC-F021 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 F020 已满足。

目标：从 `CadWorkbenchPanel.tsx` 提取组件库目录记录、加载、失败与当前 project/domain/source 请求屏障，使旧目录响应不覆盖当前工作台。

必须读取：`docs/AUTHORITATIVE_STATE.md`、`docs/FRONTEND.md`、`docs/TEST_STRATEGY.md`、`ComponentDrawer.tsx`、F012/C102/F017/F020/T002/T003/r3 smoke。

范围：先建立 focused smoke，覆盖 project/domain/source 切换清空、迟到成功/失败拒绝、真实空目录/失败说明与不持有资产版本真值。再抽出纯 reducer/adapter。父层继续唯一拥有目录 API、组件保存、替换 preview→confirm ChangeSet、Snapshot/CAS、质量、导出和 renderer。

不得做：不得把组件目录、loading 或错误说明写入 Snapshot、版本、localStorage、审计或质量报告；不得改变兼容规则、审阅/质量结论、组件保存、Provider、几何、导出或第二 renderer。

最低 Gate：新增 focused component-catalog-presentation smoke；`desktop:typecheck`、`desktop:build`、F012、C102、F017、F020、F006、T002、T003、r3、合同/文档/安全 Gate 全部通过；替换继续严格 preview→confirm。

交付：新增 `componentCatalogPresentationState` 与 `useComponentCatalogPresentation`；目录读取从 `useConceptWorkbench` 的项目 hydration 脱离，按 project+pack+source context 读取，切换 context 清空并拒绝迟到响应。该层不持有 Agent asset head、Snapshot、版本、质量、ChangeSet、导出或 renderer 真值。

证据：新增 `desktop:f021-component-catalog-presentation-smoke` 并接入 desktop CI；C102、F012、F017、F020、typecheck、build、T002（12/12）、T003、r3、contracts、agent check 与 diff check 通过。

退出：满足；组件目录仅用于浏览与选择候选，实际替换继续使用既有 preview→confirm 边界。

### FGC-T002 任务卡

状态：done（2026-07-13，脏工作区）。依赖 S008；在不改变运行时真值的前提下完成工作台 E2E 场景拆分。

目标：把当前单一 `desktop:r3-concept-workbench-smoke` 拆成可定位的独立工作台场景，同时保留 r3 作为 Agent-first 组合回归，不改变运行时行为。

范围：新建/首次初始化、四领域明确 Brief、未知领域澄清、方向预览、资产提交、部件 ChangeSet、材质 Zone、组件替换、只读 GLB 参考、质量与 GLB 导出、重启恢复、单 WebGL canvas；每个场景必须记录 Project、AgentAssetVersion、Snapshot revision 和失败原因。

不得做：重写工作台状态机、增加几何操作、增加导出格式、接入真实 Provider、改变 legacy 转换授权或删除现有 r3 断言。

交付：新增 `scripts/smoke_workbench_e2e_scenarios.mjs`，生成 `output/playwright/fgt002-scenarios/report.json` 及每个场景的独立 JSON 报告；新增 `desktop:t002-workbench-e2e-scenarios` npm 命令并纳入 CI。场景覆盖启动/单 Canvas、legacy 显式 hand-off、未知领域澄清、汽车、飞机、机械臂、未来武器概念道具、预览不写版本、可编辑资产提交、分件/材质、ChangeSet 取消、确认/质量/GLB/重启恢复。

证据：`npm run desktop:t002-workbench-e2e-scenarios` 通过，12/12 场景 `passed`；同时保留 `desktop:f001-workbench-characterization`、F002/F003/F004/F006 smoke、`desktop:r3-concept-workbench-smoke`、`desktop:typecheck` 和 `desktop:build` 作为回归门。

退出：至少上述 12 个场景具有独立可读的成功/失败报告，r3、F001 和组件/可访问性 smoke 仍通过；未知输入零写入、预览不创建版本、质量/导出引用同一 Snapshot、重启保持单 canvas。已满足。

### FGC-T003 任务卡

状态：done（2026-07-13，脏工作区）。依赖 T002；已完成单 WebGL、资源生命周期、内存和 bundle 预算门禁。

目标：测量并约束工作台的单 WebGL 生命周期、页面重载后的 renderer 数量、浏览器内存趋势和前端 bundle 预算；不得借此重写状态机或增加几何能力。

范围：在 T002 场景报告基础上增加 renderer/context 计数、重复打开/关闭抽屉和重载的资源释放观察、长会话内存采样、构建产物预算；记录 macOS Chrome 与 CI 可执行的阈值和已知差异。

不得做：引入第二个 Three.js renderer、把内存采样写入业务状态、删除现有 r3/T002 断言、以放宽 bundle warning 代替拆包或懒加载。

基线：`desktop:t002-workbench-e2e-scenarios`、`desktop:r3-concept-workbench-smoke`、`desktop:typecheck`、`desktop:build`。

交付：新增 `scripts/smoke_workbench_performance.mjs`，生成 `output/playwright/fgt003-performance.json`；新增 `desktop:t003-performance-smoke` npm 命令并纳入 workbench-e2e CI。报告记录 10 轮抽屉操作、3 轮重载、CDP GC 后堆、renderer generation、几何/纹理和构建产物字节数。当前 bundle 仍有 Vite 的 >500 kB 提示，但在明确的 1.2 MB 最大 JS、1.4 MB 总 JS、150 kB CSS 预算内。

证据：`npm run desktop:t003-performance-smoke` 通过；single canvas/context、generation 稳定、GC 后堆增长 0、重载后 geometries=3/textures=3、bundle budget 全部 PASS。当前本机抽屉期间几何峰值 165 是可观察的临时组件资源，重载后回落，不将峰值伪装为零；CI runner 结果仍以对应 commit 为准。

退出：浏览器重载和连续抽屉操作始终只有一个 canvas/context；内存与 bundle 阈值有自动化报告；超预算在 CI 中可定位失败，并且 T002/F001/F004/F006/r3 回归保持通过。已满足。

## 9. Codex 领取任务模板

开始时记录：

```text
Task ID:
Dependencies checked:
Files in scope:
Files explicitly out of scope:
Baseline commands:
Expected failing gates:
```

结束时记录：

```text
Implemented:
Contracts/migrations:
Tests added:
Commands passed:
Commands failed/not run:
Docs updated:
Worktree/commit:
Next unblocked task IDs:
Remaining blockers:
```

### FGC-G816 任务卡

状态：done（2026-07-14，脏工作区，未提交）。用户已明确将“展示模型质量包”提升为当前优先级；依赖 G801、G806、G807、T003 和单视口工作台 Gate 已满足。

目标：让当前 Agent ShapeProgram 预览完整显示已获支持的 `box`、`cylinder`、`wedge`、`capsule` 形体，并在同一 WebGL context 内提供轻量的展示级轮廓、柔化边缘和工作室灯光表现；修复“后端已生成但视口静默忽略 wedge/capsule”的质量缺陷。

范围：只改 `ModuleGraphViewport` 的临时预览解释与视觉呈现；复用已有 ShapeProgram 和 PBR 材质预设，不新建 Provider、版本字段、导出格式、第二 renderer 或本地神经 3D。所有形体仍是非功能性概念展示。需要有 focused preview smoke，覆盖 wedge/capsule、选择/隐藏状态、单一 canvas/context 与资源释放。

不得做：不增加现实武器功能细节、制造尺寸、工程材料、自由网格、任意脚本、外部纹理下载、localStorage 资产真值或第二 WebGL renderer；不将视觉倒角称为工程 fillet。

证据：新增 `shapeProgramPreview.ts`，只解释受限 `box`/`cylinder`/`wedge`/`capsule` 和已存在的 `bevel_approx`/`surface_panel` 显示来源；它是 display-only Three.js 适配器，不写 ShapeProgram、AgentAssetVersion 或 Snapshot。`npm run desktop:g816-shape-program-preview-smoke` 覆盖四类形体、选择、隐藏、PBR 外观与释放；`desktop:typecheck`、`desktop:build`、`desktop:r3-concept-workbench-smoke`、`desktop:t003-performance-smoke` 与 G801/G806/G807 通过。T003 仍断言单 canvas/context，重启后 renderer 资源稳定。

退出：满足。视觉圆角、工作室环境和高光仅改善当前主视口展示，不是工程 fillet、真实材料或新的几何真值。

### FGC-G817 任务卡

状态：done（2026-07-14，脏工作区，未提交）。用户要求把概念模型的细节质量做得更立体、更易懂；依赖 G807、G812、G813、G815、G816、G6、T002、T003 已满足。

目标：提供仅面向零基础用户的两档外观生成质量：默认“展示模型”以有限、可重复的非功能性外观面板/点缀提升层次；“快速草图”保留轻量体量以便先看大致方向。两档都必须生成同源 ShapeProgram、GLB、AssemblyGraph 与分件候选，仍走 preview → confirm。

范围：新增版本化、有限的 `presentation_profile` 请求/响应字段与 Geometry Worker 受控展示细节语法；工作台在 Agent 输入旁显示两个普通语言选择。展示档只增加本机固定规则的外观壳层、面板和点缀，不从 Brief 解释尺寸、功能、制造或工程结构；不调用 Provider，不增加纹理下载、第二 renderer、自由参数或自由网格。

不得做：不把展示细节叫作真实武器/车辆/航空/机械臂功能，不加入现实制造、性能、结构、安全或认证信息；不将 UI 偏好写入 Snapshot 或 localStorage 版本真值；不让 build/segment/commit 或导出链使用不同 profile。

证据：`BuildAgentBlockoutRequest`、`SegmentAgentBlockoutRequest` 与响应新增受限 `presentation_profile=quick_sketch|showcase`；服务端将同一 profile 传给 build/segment，候选 JSON、ShapeProgram、GLB、AssemblyGraph 和分件保持同源。`showcase` 只在既有 blockout 上追加有限概念外观部件，`quick_sketch` 保持旧输出；这些部件无可编辑参数。工作台默认选择“展示模型”，并允许用户在“快速草图 / 展示模型”间切换，若已有未保存预览则重建该预览。

退出：满足。`npm run agent:g817-showcase-quality-smoke` 覆盖四领域、确定性、GLB readback、build/segment/candidate 同源、无编辑绑定和 profile 合同拒绝；G6、G801、G806、G807、G812、G813、G815、G816、F002、typecheck/build、T002、r3 与 T003 通过。两档只生成概念级外观层次，不表示真实材料、功能、工程细节或照片级模型。

### FGC-G818 任务卡

状态：done（2026-07-14，脏工作区，未提交）。用户继续要求“展示模型”具备更明显的三级外观细节和视觉材质层次；G817 已完成且其 preview→confirm、单视口和同源链为本任务前置。

目标：把展示档由少量面板提升为一套有限、可重复的视觉细节语言：外观面板、分缝/凹槽视觉线、护板、孔洞视觉件、紧固件、发光点缀和线缆槽视觉线；为这些展示部件及现有 blockout 指定可验证的轻量 PBR 视觉材料。快速草图必须保持轻量输出。

范围：仅在 `geometry_worker.py` 的本机 showcase 规则、GLB material table 和 `shapeProgramPreview.ts` 的 display-only PBR 映射中实现。所有细节必须是当前 ShapeProgram/GLB/AssemblyGraph/segmentation/candidate 同源的非功能性概念部件，数量有上限且不接受 Brief 中的尺寸或可执行几何。展示部件不生成参数绑定，不创建机械臂 Joint，不改变 Snapshot 或已确认资产。

不得做：不实现现实机械/武器功能、气动/散热/固定/电气设计，不称视觉线为真实凹槽或开孔，不下载外部纹理，不增加本地神经 3D、第二 renderer、自由网格或额外用户模式；不把视觉 PBR 叫作工程材料。

证据：`geometry_worker.py` 为展示档确定性追加 `visual_panel_*`、`visual_groove_*`、`visual_guard_*`、`visual_light_strip_*`、`visual_cable_slot_*`、`visual_vent_*` 与 `visual_fastener_*`；它们带有石墨、复合外观、金属外观或受限灯带发光 PBR 映射，全部仍进入同一 ShapeProgram、GLB、AssemblyGraph、segmentation 和候选 JSON。视觉部件无参数绑定、没有机器人 Joint；快速草图不含这些部件。`shapeProgramPreview.ts` 在唯一 WebGL 视口中读取相同材质 ID，`partRoleLabels.ts` 只把内部 role 显示为普通中文名称。`smoke_g818_visual_detail_grammar.py` 已接入 CI；T002 改为显式选择服务端声明可调的部件，避免把展示点缀误当可调对象。

退出：满足。`npm run agent:g818-visual-detail-grammar-smoke`、G817、G6、G801/G806/G807、G812/G813/G815、G816、F002/F003/C101、typecheck/build、T002（14/14）、r3、T003、agent/contracts、文档/完整性/安全/密钥门禁均通过。展示层只提高概念观察的层次，不表示真实材料、槽/孔、散热、电气、固定或工程质量。

### FGC-A001 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 S008、D003、G1、G4、G814 已满足。

目标：把 Agent Thread 的持久化记录接入真实的、受限的多轮 Provider 上下文，并使用 DeepSeek 公开 usage 字段记录缓存命中和本机预算；不增加 Tool Calls、自由代码、真实制造能力或第二渲染器。

交付：新增 `ForgeCADProviderConversation@1` 编译器与 `ThreadMemorySummary@1` 持久化表；固定系统边界/领域包/Schema 前缀后只追加近期用户和助手消息、当前 Snapshot 摘要与新请求。普通语言微调复用持久化领域绑定；明确跨领域改为要求新会话。Provider HTTP 调用移出 SQLite 事务，Thread 只允许一个 in-flight Turn，超时标为结果未知且不自动重试。DeepSeek telemetry 记录缓存命中/未命中 token，20 元日预算先预留后结算，usage 缺失阻止当日后续联网请求。Tool Call 一律拒绝，不保存 reasoning content。

证据：`apps/agent/tests/test_provider_conversation.py` 覆盖历史拼接、Snapshot 摘要、领域绑定、DeepSeek cache usage 与日预算结算；`agent:unit`（16 passed）、G1、G4、`agent:check`、`contracts:types:check` 通过。fake/离线 Gate 不构成真实 Provider 质量或费用证据。

退出：满足；Provider 上下文辅助记录不成为 Project、AgentAssetVersion、Selection、Quality、Export 或 Snapshot 真值，且无 Key、完整 prompt、URL 或 reasoning content 被持久化。

### FGC-A002 任务卡

状态：done（2026-07-14，脏工作区，未提交）。依赖 A001、E001、E002 已满足。此任务只补齐真实评测的安全本机配置桥接；它不执行 E003 的真实 Provider 调用。

范围：为隔离评测器增加显式 `--provider-config-source macos-keychain`。它只读取 ForgeCAD Tauri 已保存的非敏感 metadata 与同一 `ForgeCAD Agent Provider/default` Keychain 项，把密钥仅保留在评测进程内存，继续使用 E002 的预算、timeout、token、单次调用、运行编号和脱敏报告边界。默认 `environment` 仍只服务浏览器开发的 0600 secret file。

不得做：不得把 Keychain 值写进环境、普通 Agent Turn、SQLite、run ledger、报告、日志或 UI；不得因本机配置可读而自动开始 E003、减少人工预算确认、改变 A001 的预算/范围屏障/Tool Call 禁止，或把无网络 smoke 写成真实模型质量。

证据：`npm run agent:e002-provider-evaluation-runner-smoke` 覆盖成功的内存 Keychain adapter、非 macOS 的零读取拒绝、缺失 Keychain 的联网前拒绝、现有授权/预算/限额/脱敏回归；本机使用缺失配置运行 `--provider-config-source macos-keychain` 返回 `E002_PROVIDER_UNCONFIGURED` 且 `network_calls_made=0`。真实 E003 仍需用户在该次运行重新保存已轮换的 Keychain 密钥、确认金额、执行并由非执行者审阅。

退出：已满足。评测器可安全复用本机桌面配置而不复制密钥；未配置或不受支持的目标在网络前停止。该任务不等于 E003 真实四领域 baseline。

下一项外部任务：`FGC-E003`（external）只在用户针对具体 run 明确授权、有效本机 Provider 配置、成本上限和独立审阅条件都满足后执行。R006 已由当前文档状态账本和实现证据标为 done；R005 的原生 WebView 点击仍受当前自动化会话的 macOS 辅助功能授权阻断；D004、P004–P006 仍分别依赖真实评测、正式审阅和平台账户。

## 10. 用户优先：CAD 设计能力闭环

2026-07-14，用户先建立 `G819 → Q003 → F025 → D005 → V002`，随后明确取消“三方向让用户选择”的产品目标，并要求 DeepSeek/Codex/Claude 式 Agent、Codex 式简洁工作台、专属 Skill、真实纹理、多材质、参考引导重建和通用生活机械扩展。ADR-0010 因此将 V002 标记为 `superseded`。2026-07-15，用户进一步确认不采用 HTML 六面或单一 box 雕刻，而采用 Profile/Loft/Sweep/Revolve/CSG/Recipe 的 3D 机械设计系统；ADR-0011 将几何与外观主链调整为：`G819 → Q003 → G820 → G821 → G822 → G823 → G824 → G824A → G824B → G824C → G824D → G825 → G826 → A003 → F025 → D005 → A004 → M108 → C105 → V003 → F026 → A005 → R007 → D006`。G819、Q003、G820–G826、A003、F025、D005 与 A004 已完成；A004 将受限产品工具、真实 readback/渲染和 DeepSeek thinking Tool Call 收束到同一 Turn。M108 现在是唯一主链 `ready`；一次只领取一个原子任务。P009 仍是独立发布回归任务。

| Task | 状态 | 前置 | 当前退出边界 |
|---|---|---|---|
| FGC-G819 | done | FGC-G815、FGC-Q002、FGC-T003 | `ShapeProgramRuntimeManifest@1` 成为 Schema、Pydantic、Worker、质量入口和导出的唯一操作真值；未实现/缺失执行器明确拒绝 |
| FGC-Q003 | done | G819 | 质量报告读取同次真实编译/GLB readback，而非重复估算 |
| FGC-G820 | done | Q003 | `ProfileSketch@1`/`ProfileSectionSet@1` 合同、规范化、重采样和失败边界 |
| FGC-G821 | done | G820 | 增强 Profile/Extrude/Revolve 的曲线、孔洞、封盖、UV 与表面 provenance |
| FGC-G822 | done | G821 | 受限多截面 Loft runtime 与真实 GLB readback |
| FGC-G823 | done | G822 | 受限 Sweep 路径/frame runtime 与真实 GLB readback |
| FGC-G824 | done | G823 | 现有 Worker、Manifold Python/WASM 布尔 benchmark 与采用 ADR |
| FGC-G824A | done | G824 | 候选 provenance、GLB readback、近退化拒绝与隔离取消补证 |
| FGC-G824B | done | G824A | 生产式 staging、真实 SQLite/对象库与权威状态原子提升边界 |
| FGC-G824C | done | G824B | macOS packaged candidate、预算、许可证/SBOM 与执行宿主选择建议 |
| FGC-G824D | done | G824C | Windows x64 frozen sidecar artifact、provenance/readback、生命周期和原子提升证据通过 |
| FGC-G825 | done | G824D | 唯一 Manifold Python CSG、不可变 feature node/input/result hash、surface/material provenance 与失败零部分 GLB 已通过 |
| FGC-G826 | done | G825 | 受控 edge finish、法线、UV0、tangent 与稳定 Material Zone 面事实 |
| FGC-A003 | done | G826 | Provider metadata/Keychain/supervisor/capability preflight、SSE 生命周期、取消、用量与稳定错误分类已通过 |
| FGC-F025 | done | A003 | Agent 资产主流程与 legacy 参数、旧导出、Graph Inspector 已隔离；父层继续拆薄 |
| FGC-D005 | done | F025、G811、G826 | 四领域非工程语义比例/Style Token 配方与受限参数绑定 |
| FGC-A004 | done | D005、A003、G819、G826 | 受限 Agent Action Loop、建模 Recipe 工具生命周期与 DeepSeek thinking/tool-call 续传 |
| FGC-V002 | superseded | — | 由 ADR-0010/FGC-V003 取代；不再实现三方向用户选择 |
| FGC-M108 | ready | A004、G826、Q003、D005 | 稳定多材质区、完整 PBR 纹理与展示环境，消费真实 UV/tangent/zone facts |
| FGC-C105 | blocked | M108、C104、G826、D005 | 可编辑组件配方与轮廓/特征/装配/比例/材质受限组合 |
| FGC-V003 | blocked | A004、C105、M108、D005、Q003、R006 | 自动选择建模语法和内部候选，只展示一个最佳结果 |
| FGC-F026 | blocked | F025、V003 | Codex 式工作台；3D 左上 mini，点击后同一 canvas 中央 focus |
| FGC-A005 | blocked | A004、F026 | 可设计、版本化、可评测的产品专属 Skill |
| FGC-R007 | blocked | C105、G7、A005 | 只读参考模型证据驱动的受限重建，不原地编辑导入 GLB |
| FGC-D006 | blocked | R007、A005 | 家用/工具/工程/农业/服务机械领域包晋级机制 |

### FGC-G819 任务卡

状态：done（2026-07-15）。依赖 G818、Q002、T003 已完成；Gate 已通过，下一项可领取任务为 Q003。

目标：建立版本化的 `ShapeProgramRuntimeManifest@1`（或等价的单一合同产物），作为 Schema、Pydantic 输入校验、Geometry Worker 执行器、GLB 编译/readback 与质量检查共同读取的运行时操作白名单。一个操作只有在该清单中声明、具备当前真实执行/验证路径并能被测试证明时，才可被接受。

范围：消除 ShapeProgram 允许集合、Pydantic 模型、Worker 分支和质量估算之间的漂移；在 preview、confirm、质量检查和导出前，对未声明、声明但未实现、参数不完整或运行时无法执行的操作返回稳定、可定位的 `UNSUPPORTED_RUNTIME_OPERATION`（或等价）错误。禁止任何 `else: continue`、跳过节点、删去未知节点后继续成功，或把部分编译结果标为完整成功。清单应同时驱动生成类型或有一致性 Gate，避免只更新一个入口。

不得做：不借此扩展新的几何操作、自由变换、任意脚本、文件/URL 输入、工程尺寸或第二 renderer；不以文档注释、前端隐藏或 hard-code 重复列表代替单一真值；不改变现有 preview → confirm → 不可变子版本、Snapshot CAS 和导出身份边界。

验收：已新增 `agent:g819-runtime-operation-manifest-smoke`，逐项验证 manifest 的 14 个操作能走真实受限编译路径，且 Schema/Pydantic/Worker/质量的接受集合一致；未知 `pivot` 和故意移除执行器分别覆盖 preview、confirm、quality、export 的 `UNSUPPORTED_RUNTIME_OPERATION` 与零副作用。已保留 G3/G801–G806、G6、G817/G818、`agent:check` 与 `contracts:types:check`。

退出：已满足。JSON manifest 位于 `packages/concept-spec/fixtures/shape-program-runtime-manifest.json`，Schema enum 由 `contracts:types:generate` 生成；未知、未实现、缺执行器或非法参数不能产生部分 preview、质量报告、版本或导出。Q003 可领取。

### FGC-Q003 任务卡

状态：done（2026-07-15）。

目标：让 Agent 资产质量报告只消费同一份真实 ShapeProgram 编译及 GLB readback 产物中的 triangles、bounds、part/material 事实和失败原因；禁止用 `box`、`cylinder` 等常数或平行遍历再次估算并冒充已验证质量。

范围：定义受限的 `GeometryCompileReadback@1`（或等价只读结果），由 G819 白名单守卫的编译器一次生成；质量服务引用该结果并保存质量事实与来源版本。编译/readback 失败、readback 不完整、白名单拒绝或资产版本不一致时，质量必须显式失败/不可用，不能回退为推测分数、旧报告或可导出状态。质量/导出仍各自保有授权边界，但使用相同运行时事实。

不得做：不把 GLB readback 说成工程、结构、适航、材料或安全结论；不额外创建导出版本链、不读取 legacy 质量报告、不为通过 Gate 虚构 GLB 指标、不削弱 Q002 的 CAS/重放边界。

验收：新增 `agent:q003-compile-readback-quality-smoke` 覆盖四领域的真实 GLB readback 与质量一致性、编译失败、未知/未实现操作、readback 损坏、旧报告隔离和重启/幂等；断言质量数字逐项来自编译结果而非平行常数。G819、G6、G801–G818、r3、T002/T003、`agent:check`、`contracts:types:check` 和相关质量/导出 smoke 必须继续通过。

退出：质量结论可追溯到当前同一资产版本的真实受限编译/readback，失败不再被静默估算替代；在 G819 完成前不得领取。

证据：`GeometryCompileReadback@1` 由同一次 ShapeProgram 编译后的 GLB readback 生成，质量报告保存 program/GLB hash、triangle、bounds、operation、output role 和 material 事实；导出消费同一编译结果。损坏 readback 产生 `compile_failure/unavailable`，未知操作继续拒绝，旧估算报告读取时隔离为 unavailable。`agent:q003-compile-readback-quality-smoke` 覆盖四领域、导出一致、损坏/未知失败、旧报告和重启幂等。下一项可领取任务为 G820。

### FGC-G820 任务卡

状态：done（2026-07-15，Q003 前置已完成）。

目标：冻结 `ProfileSketch@1` 与 `ProfileSectionSet@1`，为 Extrude、Revolve、Loft 和 Sweep 提供同一套受限二维轮廓、截面排序、规范化与重采样真值；不在本任务增加新的 3D operation 或用户 UI。

范围：Schema/Pydantic/生成类型必须表达正视、侧视、顶视或横截面平面，受限 line/quadratic/cubic segment、闭合与绕序、孔洞、对称/连续性提示、normalized bounds、resample count，以及按主轴排序的 section position/scale/twist/cap policy。前端 SVG path 只能转换为该合同，不能直接进入 Worker。canonical JSON、版本和输入 hash 进入 ShapeProgram provenance。

不得做：不实现 Loft/Sweep runtime，不允许自由 SVG 命令、文本脚本、URL、路径、绝对工程尺寸或六个独立面拼接；不把 Three.js 前端 geometry 变成资产真值。

验收：新增 `agent:g820-profile-sketch-contract-smoke`，覆盖闭合/开放、顺逆绕序、孔洞内外、自交、退化曲线、非有限数、点数/重采样/截面数量预算、section 顺序、重复位置、canonical hash 和旧 ShapeProgram 不受影响。contracts、G819/Q003、G802/G803、agent check 与文档 Gate 通过。

退出：已满足。`ProfileSketch@1` 与 `ProfileSectionSet@1` 已有 JSON Schema、Pydantic、生成 TypeScript/Python registry、规范化/统一重采样和 canonical SHA-256；ShapeProgram 可选 `profile_inputs` 保存 canonical payload、版本和 hash，旧程序保持兼容。闭合/开放、顺逆绕序、孔洞、自交、退化、非有限、预算、截面排序/重复位置和损坏 hash 均在 Worker 前拒绝。证据：`npm run agent:g820-profile-sketch-contract-smoke`、`npm run contracts:types:check`、G819/Q003/G802/G803、`agent:unit`、`agent:check` 与文档 Gate。下一项唯一 ready 为 G821；G820 没有新增 3D operation、Worker 执行器或用户 UI。

### FGC-G821 任务卡

状态：done（2026-07-15，G820 前置已完成）。

目标：让现有 Profile/Extrude/Revolve 真正消费 G820 合同，并补齐曲线重采样、孔洞、封盖、绕序、UV0 基线和表面 provenance，为 Loft/Sweep 提供可靠截面基础。

范围：增强现有执行分支，不增加前端自由绘图。Extrude 必须区分 side/start_cap/end_cap 和 hole wall；Revolve 必须处理轴上点、完整/部分角度封盖、seam 和表面角色。preview/final 允许不同细分预算，但相同输入、runtime version 和 profile 必须得到确定性 topology/readback hash。

不得做：不实现 Loft、Sweep、自由 fillet、工程尺寸或任意 mesh；不让不闭合、自交或孔洞非法的轮廓进入三角化；不以 Three.js `ExtrudeGeometry` 输出替代服务端真值。

验收：新增 `agent:g821-profile-solid-fidelity-smoke`，覆盖带孔/无孔 Extrude、完整/部分 Revolve、封盖、seam、bounds、triangle、UV0、surface provenance、重复生成和各类退化失败；G802/G803、G819/Q003、G820、GLB readback 与预算 Gate 继续通过。

退出：已满足。现有 `profile` operation 可通过 `profile_input_id`/`profile_scale` 消费 G820 canonical payload，旧 `args.points` 保持兼容。Extrude 支持 line/quadratic/cubic 重采样、0–8 孔洞、独立首尾封盖或明确开放 ribbon；Revolve 支持轴上点、完整/部分角度、有限 radial segments 与部分角 seam cap。GLB readback 逐 primitive 验证 NORMAL/UV0 accessor、UV 范围、退化三角、boundary/non-manifold edge、closed 状态及 side/hole_wall/start_cap/end_cap/seam 的连续三角区间。证据：`agent:g821-profile-solid-fidelity-smoke` 与 G802/G803/G819/Q003/G820、contracts、Agent 和文档 Gate。下一项唯一 ready 为 G822；本任务未增加 Loft/Sweep/UI。

### FGC-G822 任务卡

状态：done（2026-07-15）。

目标：新增单一受限 `loft` operation，以沿主轴排序的 `ProfileSectionSet@1` 构建汽车、飞机、家电和罩体主壳，不开放自由曲面 CAD。

范围：Loft 只接受 2–12 个已验证截面、统一重采样数量、有限 scale/twist、明确 start/end cap、固定 seam 对齐和受限 continuity policy；输出保留每段 side/cap 的 surface provenance、UV 基线、法线和真实 readback。自交、截面翻转、零面积、跨度/bounds/triangle 超限必须在候选写入前失败。

不得做：不实现 NURBS、B-Rep、任意控制网格、工程曲率或适航/气动结论；不在一个任务同时实现 Sweep、Manifold 或 PBR。

验收：新增 `agent:g822-loft-smoke`，至少覆盖四领域各一组壳体 fixture、不同截面形状/大小/位置/有限扭转、封盖、重复生成、GLB triangle/bounds/normals/UV/surface readback，以及排序、点数、翻转、自交、退化和预算失败。G819/Q003/G820/G821、G807/G818 与 Agent asset 提交回归通过。

退出：已满足。`loft` 已进入唯一 runtime manifest，并且只消费 canonical `ProfileSectionSet@1`；支持 2–12 个统一采样闭合无孔截面、严格轴向顺序、有限 scale/twist、固定采样 seam、linear continuity 与首尾 cap。Worker 在 GLB 写出前检查三角预算，GLB readback 再验证 triangle/bounds/NORMAL/UV0、closed/boundary/non-manifold/degenerate 及 `loft_side/seam/start_cap/end_cap` 连续三角范围。四领域壳体、重复字节和排序/采样/翻转/自交/退化/bounds/预算失败由 `agent:g822-loft-smoke` 覆盖，G819/Q003/G820/G821/G807/G818、G1–G7、Agent asset 提交、contracts 与 Agent Gate 回归通过。当前 Planner/UI 不自动产生 Loft，未增加孔洞 Loft、Sweep、NURBS/B-Rep 或自由曲面。下一项唯一 ready 为 G823。

### FGC-G823 任务卡

状态：done（2026-07-15）。

目标：新增单一受限 `sweep` operation，为扶手、框架、管路和线缆外观生成沿路径延伸的可编辑概念几何。

范围：Sweep 只接受一个已验证 ProfileSketch 和一条有界 polyline/受限 curve path；固定 parallel-transport 或等价确定性 frame 规则，限制 path 点数、最小曲率半径视觉比、twist、封盖、截面/路径预算和 bounds。输出区分 side/cap/seam surface provenance，并提供 UV0、法线和真实 GLB readback。

不得做：不生成真实管径、流体、电气、承压或结构结论；不允许任意 3D 脚本路径、不自动修复明显自交、不在本任务引入布尔内核。

验收：新增 `agent:g823-sweep-smoke`，覆盖直线、折线、平滑弯曲、有限扭转、开/闭路径、封盖、frame 连续、重复生成和 GLB readback；零长度段、180° 翻转、明显自交、曲率/点数/bounds/triangle 超限必须稳定拒绝。G819/Q003/G820–G822 和相关资产 Gate 通过。

退出：已满足。`sweep` 已进入唯一 runtime manifest，只消费 canonical 闭合无孔 `ProfileSketch@1` 与 2–32 点有界 path；使用确定性 parallel-transport frame、有限开放路径 twist、开/闭路径、显式首尾 cap 和固定 sample seam。运行边界拒绝零长度、短于截面视觉比的段、接近 180° frame 翻转、明显路径自交、闭合路径 cap/twist、点数/bounds/triangle 超限；GLB readback 验证 `sweep_side/seam/start_cap/end_cap`、UV0、normal 和 topology。证据：`agent:g823-sweep-smoke` 与 G819/Q003/G820–G822、G1–G7、contracts 和 Agent Gate。当前 Planner/UI 不自动产生 Sweep，不表达真实管径、承压、流体、电气或结构结论。下一项唯一 ready 为 G824。

### FGC-G824 任务卡

状态：done（2026-07-15；ADR-0012 决定暂不采用候选）。

目标：在不改变生产运行时的前提下，对现有有限 CSG、Manifold Python 与 Manifold JS/WASM 做隔离 benchmark，并用 ADR 选择唯一生产实现或明确记录无候选达标。

范围：固定 tag/commit、许可证/NOTICE、安装和 packaged 增量体积、冷启动、峰值内存、macOS arm64/Windows 打包、确定性、取消、失败诊断、四领域 union/subtract fixture、coplanar/near-degenerate 输入、材质与 surface provenance 保留、GLB readback 和移除方案。benchmark 不读取用户密钥、不联网获取运行时资产，也不修改 Snapshot/版本。

不得做：不在同一任务接入生产 handler、不同时保留 Python/WASM 两套默认真值、不因官方宣传或单个成功案例直接采用、不降低现有 G805 失败边界。

验收：新增可复现 benchmark 脚本、机器/版本/命令/JSON 报告和 ADR；结果可独立比较每个候选。若没有候选满足预算、确定性、provenance 与打包要求，本任务仍可完成研究结论，但 G825 标记 blocked 并写明解除条件，不能硬选一个实现。

退出：已满足。可复现脚本固定比较当前 handler、`manifold3d==3.5.2` 与 `manifold-3d@3.5.1`，JSON 报告记录 macOS arm64 机器/命令、固定 commit、Apache-2.0、包增量、冷/热时间、峰值内存、四领域 union/subtract、coplanar/near-degenerate 与确定性。ADR-0012 明确“不采用”：两种 Manifold 候选虽在本机完成几何 fixture 且 hash 一致，但尚未证明 ForgeCAD material/surface/zone provenance、取消、稳定错误码和 Windows 实机 packaged runtime；当前 handler 又不是稳健 CSG。生产依赖、锁文件、manifest 和 handler 均未改变。证据：`agent:g824-csg-benchmark-smoke`、`evaluations/csg-g824/report.json`、ADR-0012。G825 保持 blocked，解除条件见 ADR。

### FGC-G824A 任务卡

状态：done（2026-07-15；只补证，不采用候选、不接入生产）。

目标：在保持 G824 生产隔离的前提下，补齐当前 macOS 机器能够真实证明的 CSG 采用证据，缩小 ADR-0012 的阻断范围。

范围：为 Manifold Python/WASM 输入写入不同 source/material/zone property channel；覆盖四领域 union/subtract、coplanar 与 near-degenerate；经过 `simplify` 后按 original ID、face ID 和 backside 建立逐三角 provenance；写出确定性临时 GLB，并由 ForgeCAD readback 验证 triangle、material、surface range 与自定义 provenance；使用隔离候选进程验证 cancel/timeout 回收、稳定错误码、零候选 GLB 和测试 sentinel 状态不变。

不得做：不修改生产依赖、锁文件、runtime manifest、Worker handler、Snapshot、Version 或 cache head；不把隔离 sentinel 冒充生产事务验证；不把 macOS 结果冒充 Windows packaged runtime；不选择生产内核。

验收：`agent:g824a-csg-adoption-evidence-smoke` 验证 `evaluations/csg-g824a/report.json`。两个候选的有效 fixture 必须通过 property→simplify→GLB→ForgeCAD readback，近退化输出必须以 `CSG_DEGENERATE_OUTPUT` 在部分 GLB 前拒绝，cancel/timeout 必须分别映射 `CSG_CANCELLED`/`CSG_TIMEOUT`、回收进程并保持隔离状态不变；生产依赖继续不含候选。

退出：已满足。任务完成时的历史边界是：Python/WASM 的六组 fixture 均保留 deterministic source/material/zone/face/backside 事实；五组有效结果生成相同 GLB hash 并通过 ForgeCAD readback，near-degenerate 均在 GLB 写出前稳定拒绝；两个候选的 cancel/timeout 隔离进程均被回收且没有候选 GLB。当时尚未验证的生产 Worker/Version/Snapshot/cache 与 Windows x64 packaged 边界，现已由 G824B–G825 后续任务补齐。

### FGC-G824B 任务卡

状态：done（2026-07-15；生产式生命周期补证，不接入默认 handler）。

目标：证明候选 CSG 可以在权威状态之外完成编译/readback/staging，并且任何取消、超时、进程终止或提升事务失败都不会留下部分 Version、head、Snapshot、preview、quality、import、幂等记录或内容寻址对象。

范围：使用全量迁移建立真实临时 SQLite 和 `ContentAddressedStore`，保存一个活动 Agent v1、head、Snapshot 与 proposed ChangeSet；候选子进程不得获得数据库或对象库路径，只能写独立 staging。分别在 kernel running cancel、kernel running timeout、valid GLB ready before promotion 三个窗口终止 Python/WASM 候选并比较权威表/对象 fingerprint；随后在同一 `SQLiteUnitOfWork` 内注入 Version/head/Snapshot 提升失败验证整体回滚，再验证成功时三者一次提交。

不得做：不修改生产 runtime manifest、默认 CSG handler、依赖或锁文件；不把临时库证明写成 packaged Windows 证据；不在候选进程中打开真实权威路径；不允许 staging GLB 进入内容寻址对象库后再靠孤儿清理冒充零副作用。

验收：`agent:g824b-csg-promotion-boundary-smoke` 验证 `evaluations/csg-g824b/report.json`。两个候选的三个中断窗口必须全部保持七张权威表和对象库 fingerprint 不变、回收进程、清理 staging；注入失败必须同时回滚 Version/head/Snapshot，成功路径必须同时读到 v2/head v2/Snapshot v2 revision 2。

退出：已满足。Python/WASM 六个中断 case 均零权威提升，候选子进程没有接收 SQLite/对象库路径；ready-before-promotion 窗口存在 hash 已验证的 GLB，但终止后 staging 被移除且对象库不变。真实 `SQLiteUnitOfWork` 的 Version/head/Snapshot 注入失败整体回滚，成功整体提交。该证据满足 ADR-0012 的生产式生命周期设计门，但没有选择/接入 handler；后续 G824C 已补齐 macOS packaged 预算/许可证并建议 Python，Windows x64 packaged runtime 与 superseding ADR 仍缺，G825 继续 blocked。

### FGC-G824C 任务卡

状态：done（2026-07-15；macOS packaged candidate 与采用建议，不接入生产依赖）。

目标：在当前真实 macOS arm64 sidecar 入口中冻结并启动唯一候选，固定可执行包体、相对冷启动、完整进程树峰值内存和许可证文件预算，同时判断 WASM 是否适配现有 Worker 执行宿主。

范围：在隔离临时目录使用当前 `sidecar_entry.py`、全量迁移和候选 `manifold3d==3.5.2`/NumPy 构建 PyInstaller onefile；通过 runtime hook 强制导入候选，检查 archive、Mach-O 架构与真实 `/api/health`；同一轮测量当前 sidecar 基线与候选；记录 Manifold/NumPy 许可证文件及 hash，并比较 WASM payload 与 Python sidecar 执行宿主边界。

不得做：不覆盖仓库内 sidecar，不修改生产依赖、lock、runtime manifest 或默认 handler；不把 macOS 结果冒充 Windows x64；不因 WASM 体积更小而新增第二个 JS/WASM host 或把权威几何迁入 WebView；不在 superseding ADR 前领取 G825。

验收：`agent:g824c-packaged-candidate-smoke` 校验 `evaluations/csg-g824c/report.json`。候选必须是 arm64 Mach-O，archive 和 runtime 均真实加载 Manifold/NumPy，健康检查通过；总包体不超过 48 MiB、相对当前基线增量不超过 28 MiB、相对冷启动回归不超过 5 秒、完整进程树峰值 RSS 不超过 300 MiB；许可证文件必须存在且生产依赖保持不含候选。

退出：已满足。当前基线 19,445,536 bytes，候选 24,207,728 bytes，增量 4,762,192 bytes；同轮冷启动 18,250.329/19,243.281 ms，回归 992.951 ms；候选进程树峰值 87,376 KiB。`manifold3d` Apache-2.0 与 NumPy BSD-3-Clause/捆绑许可证文件均记录 hash。PyInstaller 需要显式 hidden import `numpy._core._exceptions`。WASM 不适配当前 Python sidecar host，因此建议唯一候选为 Python，状态为 `recommended_pending_windows_runtime`；Windows x64 实机和 superseding ADR 仍阻断正式采用，生产依赖没有变化。

### FGC-G824D 任务卡

状态：done（2026-07-15；GitHub Actions run `29383382978` 的 Windows x64 artifact 已下载并通过独立校验）。

目标：让 Windows x64 runner 构建当前真实 sidecar 入口，并在 frozen executable 内执行与 macOS 同源的 Manifold Python provenance/readback 和候选生命周期证据，消除 ADR-0012 最后一个平台事实缺口。

范围：使用 Windows 2022 x64、Python 3.11、PyInstaller 6.16.0、`manifold3d==3.5.2` 与 NumPy 2.4.6；runtime hook 正常路径只验证 frozen import，证据模式只写 CI 临时 staging。frozen binary 必须执行六组 provenance fixture、确定性重复、ForgeCAD GLB readback 和 near-degenerate 写出前拒绝；同一 binary 在 busy cancel、busy timeout、valid GLB ready-before-promotion 三个窗口被回收，且真实临时 SQLite/对象库 fingerprint 不变；真实 UnitOfWork 注入失败整体回滚、成功整体提交。

不得做：不把静态 workflow、macOS hook smoke 或未执行 YAML 写成 Windows 已通过；不传数据库/对象库路径给候选进程，不修改生产依赖、默认 handler、Snapshot/Version 真值或仓库 sidecar；不读取 Provider Key、不调用 Provider；没有远端 artifact 时不得新增采用 ADR 或领取 G825。

验收：Windows job 生成 `ForgeCADCSGWindowsPackagedEvidence@1` 报告并由 `check_g824d_windows_packaged_candidate.py` 验证；报告必须证明真实 x64、frozen health、六组 fixture、三个中断窗口、零权威提升、事务回滚/提交、`provider_calls: 0` 和 `production_dependency_added: false`。无论成功或失败都上传 `g824d-windows-packaged-candidate` artifact。

当前证据：`evaluations/csg-g824d/windows-report.json` 来自 run `29383382978` 的真实 `windows-2022` frozen executable。五组有效 fixture 通过 provenance/GLB readback，near-degenerate 以 `CSG_DEGENERATE_OUTPUT` 在写出前拒绝；cancel/timeout/ready-before-promotion 均回收进程且保持 SQLite、对象库和部分 GLB 不变，Version/head/Snapshot 原子回滚/提交通过，Provider 调用为零。

退出：已满足。artifact 经 `check_g824d_windows_packaged_candidate.py` 通过，ADR-0013 已取代 ADR-0012 的不采用结论并选择 Python；任务完成时只允许下一原子任务 G825 开始集成，现已由 G825 完成生产依赖与默认 handler 接入。

### FGC-G825 任务卡

状态：done（2026-07-15；ADR-0013 的唯一生产内核、Feature History、失败边界与版本链 Gate 已完成）。

目标：只接入 G824 选定的一种生产 CSG，实现可靠的受限 union/subtract，并将每次建模保存为不可变 feature node/input hash，而不是破坏性改写旧顶点。

范围：ShapeProgram 的有序节点必须记录 operation、输入节点、规范参数、runtime/kernel version、surface/material provenance 和结果 hash。布尔只接受已验证封闭输入、有限深度/输入数和预算；失败返回稳定 node ID/错误码且不产生部分 GLB。现有受限 box fixture 必须兼容或显式迁移，不能删除历史数据。

不得做：不引入第二 Project/Version/FeatureGraph 真值，不允许任意 mesh 修复脚本、隐藏 fallback 或把非 manifold 输入自动当成功；不在本任务增加 UI、材质纹理或工程实体承诺。

验收：新增 `agent:g825-feature-csg-smoke`，覆盖多种闭合壳体 union、窗洞/轮拱/凹槽 subtract、coplanar/退化/非封闭/超深度/超预算失败、取消、重复 hash、surface provenance、旧 G805 fixture、preview 零版本副作用、confirm 不可变子版本和 GLB readback。G819/Q003/G820–G824、G6、r3 继续通过。

证据：新增 `manifold_csg.py`，生产依赖精确锁定 `manifold3d==3.5.2`/NumPy 2.4.6；`GeometryCompileReadback@1` 与 GLB extras 保存有序 node、input/result/parameter/provenance hash、runtime/kernel version 和布尔表面来源。`npm run agent:g825-feature-csg-smoke` 覆盖壳体 union、窗洞/轮拱/凹槽 subtract、coplanar、退化/非封闭/深度/预算拒绝、取消/超时、重复 hash、旧 G805、preview 零版本副作用、confirm 不可变子版本、质量及导出 GLB 同源 readback。

退出：已满足。默认运行时只有一个 CSG handler，特征历史和派生 GLB 可追溯，任何失败不输出部分成功模型；它不增加 UI、纹理或工程实体承诺。

### FGC-G826 任务卡

状态：done（2026-07-15；G825 依赖、G826 Gate 与兼容回归均通过）。

目标：建立纹理前的真实表面完成事实：受控 edge finish、法线、UV0、tangent 和稳定 Material Zone face provenance，使 M108 不再从颜色或前端猜测区域。

范围：定义受限 edge set/半径比例和细分预算；为 Profile/Extrude/Revolve/Loft/Sweep/CSG 输出可复现的 split/weighted normals、UV0、tangent 及 face→part/zone 映射。优化、重索引和 GLB 写出后必须保留该映射；无 UV/tangent、空 zone、重叠 zone、布尔后 provenance 丢失或纹理前置条件不满足时明确失败/降级。

不得做：不在本任务导入完整纹理、HDRI、clearcoat 或工程材质；不把 `bevel_approx` 冒充精确 fillet，不允许 UI 按颜色/朝向猜 zone，不用 glTF 字段存在代替 readback。

验收：新增 `agent:g826-surface-readback-smoke`，覆盖所有已启用主形体操作的 edge/normals/UV/tangent/zone，seam 和 cap/side/trim 区域，镜像/阵列/CSG 后映射，GLB validator/readback、重复 hash、缺失/损坏/预算失败，以及 M101–M107 兼容。G819/Q003/G821–G825、T003、r3 与导出 Gate 继续通过。

证据：`GeometryCompileReadback@1` 新增 `tangent_primitive_count`、逐 primitive 的 surface completion 与 `material_zone_faces`；GLB 写出 `TANGENT`、`_FORGECAD_FACE_ID`、`_FORGECAD_SOURCE_FACE_ID` 和稳定 part-instance/zone extras。`agent:g826-surface-readback-smoke` 覆盖 primitive、Extrude/Revolve/Loft/Sweep、edge finish/trim、mirror/array、Manifold CSG、重复 hash、损坏 tangent/UV/face/zone 以及半径/细分/三角预算失败。

退出：已满足。当前 GLB 的表面、UV、tangent 和 zone 均可从同次 compile/readback 证明，M108 可安全消费这些几何前置事实；G826 没有实现完整纹理、HDRI、clearcoat 或工程 fillet。

### FGC-A003 任务卡

状态：done（2026-07-15；G826/Q003 依赖、A003 Gate 与兼容回归均通过）。

目标：把 DeepSeek 接入从“进程启动时可选环境变量 + 同步请求 + 泛化文案”升级为可观察、可取消、可诊断的 Provider Gateway；用户能明确知道当前是否配置、是否真的发起网络请求、请求进行到哪一步、为何失败以及已保存资产是否安全。

范围：定义 `ProviderConnectionState@1` 与脱敏 `ProviderExecutionTrace@1`。Tauri 保存后依次验证 metadata、Keychain、supervisor restart 和新 Agent capability；普通 Turn/`provider:check` 产生 start/progress/completed/failed/cancelled Item，记录 latency、usage、cache tokens、attempt 和 `network_call_made`。固定映射 DeepSeek 400 invalid request、401 auth、402 balance、422 invalid parameters、429 rate limit、500/503 server、网络/timeout、空 `content`、无效 JSON 和 Schema 不符。JSON 模式 prompt 必须明确要求 JSON 并包含版本化输出示例；失败不得静默回退为 deterministic success。

不得做：不读取或记录 Key 明文、完整 prompt/response、`reasoning_content`、内部 Base URL 或模型秘密；不自动执行 E003 真实评测；不在普通网络/鉴权/余额/超时失败后自动重试；不修改 ShapeProgram、资产版本、Snapshot、质量或导出；不以 UI 文案代替进程级 Provider capability 验证。

验收：新增 A003 Rust/Python/desktop Gate，覆盖 metadata 缺失、Keychain 缺失、保存成功但重启失败、离线明确状态、ready、取消、所有 DeepSeek 错误类别、空 JSON、Schema 不符、usage/cache telemetry、日志脱敏、重启恢复和“选择真实 Provider 后不静默离线成功”。本机无配置 fixture 必须稳定报告 `unconfigured + network_call_made=false`。`agent:check`、contracts、desktop typecheck/build、T002/T003、r3、secrets 和安全 Gate 继续通过。

证据：新增 `ProviderConnectionState@1` 与 `ProviderExecutionTrace@1` Schema/生成类型；Tauri 保存/清除 Provider 后依次回读 metadata、Keychain、受管 supervisor 与 Agent capability。普通 Turn 和显式连接测试都支持取消，SSE 只组装结构化 JSON，记录脱敏 phase、latency、usage/cache、attempt、`network_call_made` 与 `fallback_used=false`。DeepSeek 400/401/402/422/429/500/503、网络、timeout、空 content、无效 JSON、Schema 不符和不支持 Tool Calls 均有稳定错误且不自动重试、不静默离线成功。`agent:a003-provider-gateway-smoke`、`desktop:a003-provider-connection-smoke`、Rust 6 项测试、G4、Agent 18 项单测、contracts、typecheck/build 与任务回归通过；所有联网路径使用本机 fake Provider，没有执行 E003 真实评测。

退出：已满足。Provider 的配置、网络调用、生命周期和失败均有可读、可审计、无密钥的事实；已保存资产在失败/取消时不改变。下一唯一可领取任务为 F025；A003 不证明本机已配置 DeepSeek、真实模型质量或真实费用。

### FGC-F025 任务卡

状态：done（2026-07-15；A003/Q003 依赖与 F025 Gate 已通过）。

目标：将 legacy 参数、旧导出和 Graph Inspector 从 AgentAssetVersion/ActiveDesignSnapshot 主流程隔离，同时继续将 `CadWorkbenchPanel` 拆为可验证的 Agent 主编排与显式 legacy 只读兼容边界。

范围：Agent-active 路径只能读取当前 Agent 资产、受限 `AgentParameterControls`、当前质量/readback、预览确认和 Agent 导出身份；legacy `WeaponParameters`/旧参数、旧格式导出、legacy Graph Inspector 仅可在用户显式进入的 legacy 只读/转换表面出现，并不得由 Agent 编辑、质量、选择或导出流程隐式调用。将父层的兼容装配、抽屉组合和 Agent 主流程继续抽到有单独输入/状态边界的模块或 hooks，保持一个 WebGL canvas/context 和既有状态机。

不得做：不删除旧数据、迁移、fixture 或现有兼容读取来让测试通过；不把 legacy 与 Agent 的 `vN` 合并显示，不按导出格式切换版本链，不将 legacy 参数映射成自由 Agent 参数，不重写整套视觉布局或增加第二 renderer。

验收：新增 F025 前端/E2E Gate，至少覆盖 Agent-active 时 Graph Inspector、旧参数和旧导出均不可见且无网络/API 调用；显式 legacy 只读入口仍可查看兼容信息但不能污染 Agent Snapshot/quality/export。覆盖切换项目、重启、迟到响应、undo/redo 与无 Agent 资产状态。`desktop:typecheck`、`desktop:build`、F001/F006、T002/T003、r3 与相关 Agent 合同 Gate 必须通过，且有父层职责/行数下降或模块责任清单的可审计证据。

证据：`useConceptWorkbench` 首次只读取 Project shell，只有用户点击“查看旧版只读信息”后才读取旧版本、ChangeSet、审计与 ModuleGraph；关闭、切换项目和迟到响应均由 request guard 清理。`WorkbenchInspectorRail` 将 Graph Inspector、旧参数、旧质量摘要和旧格式说明限制在显式只读表面；Agent 导出/质量抽屉不再接收 legacy props，Agent Turn/修改意图不再调用 legacy Planner。F025 Gate 按文本边界记录 `CadWorkbenchPanel.tsx` 从 3,032 行降至 1,872 行，且仍只装配一个 `ModuleGraphViewport`。新增 `desktop:f025-legacy-isolation-smoke`，F001/F006/T002（14/14）/T003/r3、typecheck/build 与相关合同 Gate 通过。

退出：已满足。Agent 主流程不承载 legacy 控制真值；兼容表面仍显式可见、只读、可测试。下一唯一可领取任务为 `FGC-D005`。

### FGC-D005 任务卡

状态：done（2026-07-15；F025、G811、G819、Q003、G826 与 D005 Gate 已通过）。

目标：为未来武器概念道具、汽车、飞机和机械臂四个 Domain Pack 提供版本化的非工程语义比例配方，并把其中可编辑项绑定到当前真实可执行的、范围/步长/单位/显示名均被冻结的受限参数路径。

范围：定义 `DomainSemanticProportionRecipe@1` 与 `MechanicalStyleToken@1`（或等价版本化域包数据），把“更紧凑/更修长/更厚重/更简洁”等普通语言意图映射为有限比例档位和已存在的参数绑定。每项绑定都必须由 G819/G826 的运行时与表面事实允许、由 G808 的冻结元数据约束，并走 preview → ChangeSet → confirm；四领域均须有可读中文名称、保守默认、上下界、步长、越界拒绝和无可用绑定时的明确回退。

不得做：不出现 mm、厚度、载荷、推力、气动、安全、适航、认证或制造建议；不创建自由滑杆、自由坐标/旋转/缩放、任意 ShapeProgram 节点、机械臂工程 Joint 或跨领域自动推断；不把配方偏好写入 localStorage/Snapshot 真值或绕过版本/CAS。

验收：新增 D005 合同/服务/UI Gate，覆盖四领域的配方目录、允许绑定、范围/步长/单位显示、preview 取消/确认、越界/锁定拒绝、无绑定回退、重启与 undo/redo；断言所有实际执行操作均在 G819 白名单中，质量来自 Q003 真实 readback。F025、G808/G811、G819/Q003、G6、T002/T003、r3、typecheck/build 与文档 Gate 必须通过。

证据：新增 `MechanicalStyleToken@1`、`DomainSemanticProportionRecipe@1` 与 `ResolvedSemanticProportionOptions@1` JSON/Pydantic/OpenAPI 合同；四领域各 4 个普通语言配方通过稳定语义部件槽解析到当前变体的真实 Part，再同时核对 G808 ratio binding 与 G826 GLB `surface_provenance/source_operation_ids`。只读解析 API 不写版本；桌面卡片只把命中的 `path + target_value` 交给既有 `set_part_parameter` preview。锁定、越界、步长、不存在绑定、编译/readback 失败均拒绝或明确回退。`agent:d005-semantic-proportions-smoke` 覆盖四领域、preview 取消/确认、重启、undo/redo 与 Q003；`desktop:d005-semantic-proportions-smoke` 覆盖中文、范围/步长、非工程提示和单次 preview。

退出：已满足。四领域都有受限、普通语言可解释、非工程的比例/Style Token 配方，并且只能修改当前已验证的绑定。下一唯一可领取任务为 `FGC-A004`。

### FGC-V002 任务卡

状态：superseded（2026-07-14，由 ADR-0010 与 FGC-V003 取代）。

原因：用户明确要求不要出现三个方向供选择，由 Agent 自己评审并展示最佳结果。保留本卡只用于解释历史任务 ID；不得继续实现三方向解释、方向卡或单维重混 UI。当前 Alpha 的三方向仍是现状，只有 V003 Gate 完成后才能从当前用户流程移除。

### FGC-A004 任务卡

状态：done（2026-07-15；A004、A003、G819/Q003、D005、G1、T002/T003、r3 与安全/文档 Gate 已通过）。

目标：建立 Codex/Claude Code 式但仅面向 ForgeCAD 产品工具的 `AgentActionLoop@1`，让 DeepSeek 可以在一个 Turn 内规划、调用受限工具、读取工具结果、继续判断并停止，而不是一次请求后由前端串接另一套 legacy API。

范围：Action Loop 只允许 Product Tool Registry 中的 domain inference、reference research、Style Token/Recipe 选择、ProfileSketch author/validate、ShapeProgram author/validate、build、compile/readback、concept render、candidate evaluate 和 preview 工具。每个 Tool Item 有 stable ID、input/output Schema、状态、耗时、父 Turn、幂等 key、失败类别和 approval policy。DeepSeek thinking + Tool Calls 的同一轮子请求按官方合同回传短生命周期 `reasoning_content`；用户只看到 reasoning summary。限制最大 12 次 tool call、最大 wall time、token/费用和单次并发；取消传播到 Provider 与 Worker。

不得做：不开放 shell、Python、JavaScript、任意 URL/路径、通用 MCP、直接数据库或绕过 Snapshot/ChangeSet；不让模型动态注册工具；不持久化原始隐藏推理；不通过多 Agent 复制 Project/Version 真值；不自动确认永久修改。

验收：A004 smoke 覆盖正常 plan→build→readback→render→evaluate、工具 Schema 拒绝、G819 未实现操作拒绝、Tool Call 上限、取消/timeout、Provider 断线、重复 tool ID、stale Snapshot、`reasoning_content` 续传与不落盘、approval 前零永久副作用、重启后 completed/failed Turn 可读。A003、G819/Q003、D005、G1、T002/T003、r3 和安全 Gate 继续通过。

证据：新增 `AgentActionLoop@1`、代码所有的 `ForgeCADProductToolRegistry@1` 与只读 `/api/v1/agent/product-tools`。13 个工具只有 domain/reference/style/profile/shape/build/readback/render/evaluate/preview 能力；工具参数与结果均经过 JSON Schema，永久修改工具不能注册。离线 Planner 与 DeepSeek 均执行 plan→build→真实 GLB readback→四视图→硬门→临时 preview；DeepSeek 每次工具结果回送下一子请求并在内存续传 `reasoning_content`，持久化 Item 只含 stable tool ID、父 Turn、状态、耗时、幂等键、失败类别和审批策略。桌面不再在 Turn 完成后自动并发三次 concept-preview API。`agent:a004-action-loop-smoke` 覆盖成功链、Schema/G819、12 次上限、取消/timeout/断线、重复 ID、stale Snapshot、推理不落盘、零永久副作用和 completed/failed 重启读取。

退出：已满足。一个 Turn 的模型、工具、检查和停止形成可恢复的单一生命周期；所有工具都由代码白名单/Schema/权限验证，前端不再拼接第二套隐式 Agent 流程。下一唯一主链任务为 `FGC-M108`。

### FGC-V003 任务卡

状态：blocked（等待 A004、C105、M108、D005、Q003、R006）。

目标：让 Agent 在内部生成、构建和评审多个完整外观候选，默认只向用户展示一个通过硬门、综合证据最好的结果；彻底取消三方向选择责任。

范围：定义 `CandidateEvaluation@1`、`BestCandidateDecision@1` 和版本化 evaluation profile。Agent 先按 Domain Pack、Style Token、part role、C105 Recipe 和 G819 manifest 选择 Profile/Extrude/Revolve/Loft/Sweep/CSG 语法；每个内部候选再经过范围/Schema/G819 runtime/Q003/G826 readback/完整外观硬门，并按 Brief 覆盖、语义比例、领域角色完整、材质/纹理覆盖、可编辑性、R006 概念渲染一致性与复杂度评分。所有候选失败时 Turn 明确失败；获胜者只作为未保存 best-candidate preview，用户确认后才创建 AgentAssetVersion。用户“换一个思路”创建新 Turn，不展开被淘汰方向。

不得做：不显示三张方向卡、`N/3`、内部 variant ID、原始评分 JSON或隐藏推理；不把得分称为真实审美、工程质量或安全；不选取 readback/完整外观硬门失败候选；不自动覆盖已确认资产；不增加第二 renderer。

验收：四领域每个固定 Brief 至少内部生成 3 个候选，覆盖产品结构到建模语法/Recipe 的可解释路由、硬门淘汰、确定性 tie-break、唯一 winner、无 winner、Brief 正/负覆盖、比例/材质/编辑性分数来源、候选零版本副作用、确认/取消、迟到结果、项目切换、重启不持久化 preview、离线/DeepSeek 来源标记。桌面 E2E 断言默认可见方向卡数为 0、最佳结果卡数为 1。A004、C105、M108、D005、Q003、R006、F024、T002/T003、r3、typecheck/build 继续通过。

退出：用户只需要判断结果是否满足目标，不再替 Agent 在三个方向中做筛选；最佳结果可追溯到同 Turn 的真实编译/readback/渲染和评审证据。

### FGC-F026 任务卡

状态：blocked（等待 F025、V003）。

目标：实施 ADR-0010 的 Codex 式简洁工作台：Agent 会话/步骤/单一最佳结果成为中央主线，3D 默认缩到左上 mini viewport，点击后把同一个 canvas 移到中央 focus。

范围：新增 `ViewportDockState = mini | focus`、`BestCandidateResultCard` 和简化 shell。左列固定约 304px，mini viewport 约 280×180；中央是连续 Item、结果摘要和固定输入区；右侧不常驻属性面板。focus 模式保持同一场景、相机、选择、材质和 Snapshot render preset，关闭/Escape 返回左上。Provider 顶栏只显示未配置/连接中/已连接/需处理四态。

不得做：不 mount 第二个 `ModuleGraphViewport`/renderer/canvas，不在 F026 改 Snapshot/API/几何/材质/导出，不重新引入 Mode/任务中心/终端，不把 legacy 控件放回 Agent 壳，不以 CSS 隐藏代替 F025 隔离。

验收：1180×760 与常用桌面尺寸视觉/E2E；mini→focus→mini、快速重复点击、Escape/焦点返回、窗口缩放、项目切换、抽屉、模型重载、选择/隐藏/隔离；renderer/context 始终 1，资源计数不增长。默认首屏没有三方向、Graph Inspector、旧参数、旧导出、技术 Skill/Tool/Schema；F001/F006/F025/V003/T002/T003/r3/typecheck/build 通过。

退出：工作台的默认阅读顺序是“目标→步骤→一个结果→下一动作”，3D 可随时放大但始终只有一个 renderer，父层职责继续下降。

### FGC-A005 任务卡

状态：blocked（等待 A004、F026）。

目标：允许用户为重复的机械外观设计任务创建、测试、版本化和启用 ForgeCAD 专属 Skill，同时保持零代码、严格工具边界和可追溯结果。

范围：定义 `AgentSkillManifest@1`，目录包含 `SKILL.md`、manifest、tool policy、input/output Schema、references、examples 和 evals。引导式设计器收集用途、适用领域、输入/输出、允许的只读/候选工具、至少 3 个成功示例和 3 个失败/停止示例；先 dry-run，再 eval，最后显式启用。有效工具是 Skill、全局 Registry、G819 manifest 和当前权限的交集。AgentAssetVersion/provenance 记录 Skill ID/version/hash。

不得做：不允许 shell/脚本、任意 URL/文件路径、动态工具、第三方二进制、绕过确认、写 Snapshot 真值、隐藏付费调用或自动发布失败 Skill；不让 Skill 自定义可执行 ShapeProgram operation。

验收：创建/修改/版本/禁用、Schema/工具越权拒绝、引用 hash、示例/eval、零副作用 dry-run、启用后自动触发、冲突优先级、旧资产保留旧 hash、删除保护、导入来源/许可证、重启恢复和 UI 可访问性。A004、G819、F026、安全/密钥/文档 Gate 通过。

退出：专属 Skill 可被非开发者安全设计和复用，但不能扩大 ForgeCAD 的几何、权限、安全或版本边界。

### FGC-M108 任务卡

状态：ready（A004、G826、Q003、D005 已完成；这是下一唯一主链任务）。

目标：消费 G826 已回读的稳定 zone/UV/tangent，把当前单区参数材质提升为完整 PBR 纹理和可复现展示环境，使模型外观显著接近真实产品，而不把视觉材质冒充工程材料。

范围：定义 `VisualTextureSet@1` 并复用 G826 多 zone readback；支持 baseColor、metallicRoughness、normal、occlusion、emissive，兼容时支持 clearcoat 与受限 transmission/IOR。每个纹理记录 hash、色彩空间、尺寸、来源、许可证和回退；每个 zone 绑定真实面集合、稳定 ID 和默认/允许材质。加入 HDRI/工作室环境、线性色彩、统一 tone mapping、接触阴影；评估 KTX2/BasisU 与 glTF Transform inspect/validate/dedup/prune/压缩，必须保留 Part/zone/material 映射，不能另算一套 UV/tangent 真值。

不得做：不自动抓取网站资产、不伪造许可证、不使用绝对路径或外部 URL、不让 UI 猜 zone、不在纹理缺失时声称已加载、不输出工程材料性能、不增加第二 renderer。

验收：四领域各至少 3 个多 zone 资产；UV/tangent/readback、色彩空间、纹理缺失/损坏、clearcoat/透明兼容、GPU/文件预算、环境 hash、GLB Validator、优化前后 Part/zone/material 映射、重启/undo/redo/导出一致、人工视觉基准中比例/材质/细节中位数 ≥4/5。M101–M107、Q003、D005、T003、r3 继续通过。

退出：纹理与多材质真正进入同源 GLB/视口/readback；“更真实”有资产和视觉基准证据，不只是参数或文案。

### FGC-C105 任务卡

状态：blocked（等待 M108、C104、G826、D005）。

目标：建立可编辑组件配方，把领域角色、语义比例、连接、材质区和受限参数组合为可复用的完整产品部件，而不是每次从 primitive 临时拼装。

范围：定义 `EditableComponentRecipe@1`，引用已审阅的 role、ProfileSketch/ProfileSectionSet、ShapeProgram feature template、G808/D005 binding、connector/pivot、G826 Material Zone/M108 纹理集合、child slot、允许领域、版本、来源和质量。配方实例化只创建候选；替换/调整仍走 C102/C104 与 preview→confirm。父子配方必须无环并有预算。

不得做：不引入工程装配、公差/紧固/载荷结论，不接受任意代码/路径/URL，不自动覆盖锁定部件，不把项目内未审阅组件晋级为正式默认。

验收：四领域关键角色配方、实例化/替换/比例/材质预览、connector 保留、锁定/跨领域/质量失败/循环/预算拒绝、版本升级、旧资产 hash、重启/undo/redo、Q003/M108 readback 一致和组件目录来源说明。

退出：Agent 能用可验证、可继续编辑的组件配方形成更完整的产品外观，同时保持 AssemblyGraph 与版本真值唯一。

### FGC-R007 任务卡

状态：blocked（等待 C105、G7、A005）。

目标：让用户导入的只读 GLB/图片参考成为比例、轮廓、分区和设计语言证据，指导 Agent 重建一个新的受限可编辑资产；绝不原地编辑或冒充解析出真实工程结构。

范围：定义 `ReferenceEvidence@1` 与 `ReferenceGuidedRebuildPlan@1`。对 GLB 使用现有安全导入和真实 readback；对图片只保存用户授权的内容寻址对象、视角/来源/许可证声明。Agent 提取普通语言轮廓、比例区间、主要色块/材质区和可见部件假设，展示差异与不确定性；重建只使用 G819 operation、D005 配方和 C105 recipe，产生新候选/新 provenance。

不得做：不联网反向搜索未授权图片、不抓取或复制第三方模型、不原地修改 imported GLB、不声称恢复隐藏结构/精确尺寸/材料/功能，不把相似度当原创或许可证证明。

验收：GLB/多视图图片、单图不确定性、来源/许可证、缺失视角、比例/轮廓差异、reference hash、只读原对象不变、新候选 preview/confirm、拒绝/取消、跨项目隔离、重启、删除引用保护和视觉相似度人工审阅。G7、G819/Q003、C105、M108、T002/T003/r3 继续通过。

退出：参考只作为可追溯证据，新模型仍由 ForgeCAD 受限运行时重建并可编辑；原参考和新资产身份清楚分离。

### FGC-D006 任务卡

状态：blocked（等待 R007、A005）。

目标：把产品从首批四领域扩展到家用、工具、工程、农业、服务等生活机械外观，同时用 Domain Pack 晋级机制防止“万能 fallback”降低质量或越过安全边界。

范围：定义 `DomainPackAuthoring@1` 与 `draft → evaluated → enabled` 生命周期。每个新包必须有角色、比例配方、组件配方、材质/zone、20 条正常 Brief、10 条含糊/越界输入、至少 4 个模板/12 个 fixture、视觉 benchmark、失败回退、来源/许可证和非功能边界。先交付家用/桌面机械一包，再逐包扩展；用户仍不选 Mode。

不得做：不创建一个接受所有名词的 default pack，不因互联网参考存在就自动启用，不提供医疗/交通/飞行/工业安全或认证结论，不复制工作台/Agent/版本链，不允许 pack 带可执行代码。

验收：首个新领域包的推断/澄清/scope、内部候选最佳选择、配方/材质/参考重建、失败/越界、重启/导出和视觉基准全链；旧四包回归、跨包混淆率、未知领域零默认武器回退、A005 Skill 兼容和统一工作台 E2E。

退出：至少一个非首批领域通过完整晋级 Gate，证明 ForgeCAD 可扩展到生活机械；“通用”仍由逐包证据支持，而不是口号。

完整完成标准见 [CODEX_DEFINITION_OF_DONE.md](CODEX_DEFINITION_OF_DONE.md)。
