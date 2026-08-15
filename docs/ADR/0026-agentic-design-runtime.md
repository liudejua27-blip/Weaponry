# ADR-0026: Agentic Design Runtime

状态：Accepted as target architecture；observe/plan projection、durable session/checkpoint/RepairIntent prepare/readback、显式 bounded 多视图 authoring producer/readback、单动作 geometry ActionRun、逐视图 evidence inventory、hash-bound CrossViewEvidenceBundle@1、approval-gated 有界同阶段独立动作批处理，以及带可选累计程序合并准备的 ordered composition proposal 已落地；Repair 晋级应用、用户 promotion 和正式跨视图视觉 Gate 仍未完成
日期：2026-08-13

## 背景

MCP005-MCP009 已证明 ForgeCAD 可以在 Codex host 中完成真实参考导入、typed geometry、appearance、limited quality、approval/version 和 CAS-backed GLB export。MCP010C/D/E/F 又补齐了固定 renderer、九 AOV、hard-surface Operator、离线 AssetPack、Viewer surface、silhouette target/camera/Rig/Part compare 等结构能力。

但当前真实机器人基准仍是 `QUALITY_TARGET_NOT_MET`，attempt35 也只是 `provisional retained observation`，并且存在 `INCOMPLETE_TRUTH_BINDING` 和 fit/compare camera `MISMATCH`。这说明当前问题不是单个工具或模型不够强，而是产品还没有把“看见、理解、分阶段设计、每步验证、可回滚修改”组织成主 authoring loop。

用户输入的新要求强调：

- Codex 必须能看见 Scene Graph、语义部件、尺寸、几何统计、相机、选区、多视图截图/AOV；
- 每步必须 `Plan -> Act -> Inspect -> Render -> Evaluate -> Act`，不能最后才一次性渲染；
- 主要阶段必须有 checkpoint/version/restore；
- 软件返回 `MainBody`、`LeftSensorHousing`、`symmetry_partner` 等设计语义，而不是低层 cube/mesh 列表；
- 生产流程必须先 reference/design spec/blockout/primary form，再 secondary/tertiary detail/PBR/final。

外部研究结论：

- Pi Agent 的价值是薄、线性、可观察、可配置的 harness，而不是黑盒多 Agent；
- Omniverse Kit 的价值是 extension/app 组合架构和清晰生命周期；
- FreeCAD 的价值是 Document、Transaction、Parametric Recompute、Undo/Redo；
- OpenUSD 的价值是 semantic scene graph、layer、variant、reference；
- build123d/CadQuery 的价值是 AI 友好的参数化 CAD 表达；
- BlenderMCP 的价值是 MCP bridge、scene inspect 和 screenshot feedback，但其 arbitrary Python 执行方式不进入 ForgeCAD；
- Manifold、Trimesh、MaterialX、TRELLIS/Hunyuan3D 只能按 adoption/隔离/draft 规则进入，不能绕过 Runtime 真值。

## 决策

ForgeCAD 的高质量路线改为 **Agentic 3D Design Runtime**：

```text
Codex / Pi-style Agent Harness
  -> DesignSession Orchestrator
  -> SemanticSceneGraph / ModelUnderstandingBundle
  -> ReferenceCanvas + DesignSpec
  -> Stage-gated Parametric Design Kit
  -> Runtime/Worker compile + readback
  -> Visual Evidence Engine
  -> Critic/Repair Loop
  -> Approval/version/export
```

ADR-0025 仍然有效：ForgeCAD 不内置 LLM/Provider，不执行任意 Python/JavaScript/shell，不让外部 DCC、GLB、Three.js scene、prompt 或截图成为版本真值。Rust Runtime 仍是 SQLite/CAS/Project/Candidate/Version/Job/Quality 的唯一写者，MCP 仍是薄 stdio adapter，Viewer 仍只读。

### 0.1 第一阶段实施状态

MCP010F 当前包含两个严格受限的 slice：

- contracts manifest 新增本 ADR 的 10 个目标合同，并通过 `scripts/check_agentic_contracts.py` 的正向/负向 fixture 检查；
- Runtime 新增按需重建的 `projection/read-only` observation、stage plan 和 critic projection；该投影只读现有 project/snapshot/candidate/evidence，不创建 candidate/version/job，也不把投影当作持久真值；
- MCP 新增四个 read-only tool：`scene_observe_get`、`design_stage_plan_get`、`critic_report_get`、`visual_evidence_bundle_get`；每个工具均声明 read-only、非 destructive、非 open-world；
- Viewer 新增 projection normalizer，只显示 Runtime 已返回的 stage/gate/action/hash，不在本地生成质量或设计状态；
- 隔离探针先执行 Ponytail preflight，随后验证空参考 fail closed、动作锁定、project binding 和无用户持久数据写入：`docs/evidence/mcp010f/agentic-runtime-observe-plan-20260813.json`。
- Runtime 另外提供受批准的 `session_create_or_resume`、`checkpoint_prepare`、`checkpoint_restore_prepare` 写入准备，以及 `session_get`、`checkpoint_get` durable readback；session/checkpoint 记录由 Runtime 写入 SQLite/CAS，恢复只生成 CAS-bound `RepairIntent@1`，不修改 candidate/version/history；
- Viewer 通过 authenticated IPC 查询 durable session，仍只展示 read model，不提供写入按钮；完整重启后的 session/checkpoint/intent receipt 为 `docs/evidence/mcp010f/agentic-runtime-session-checkpoint-20260813.json`，合同检查为 `scripts/check_agentic_runtime_receipt.py`。
- `session_create_or_resume` 现在还接受严格 bounded 的显式 `authoring_context`：Runtime 校验多个授权 `ReferenceEvidence` 组成的 `ReferenceCanvas@1`、CAS/object hash、view/coverage/unknown/claim 状态，以及绑定的 `DesignSpec@1`；缺省输入仍生成保守 single-reference unknown 对象。
- `design_action_run_prepare` 已形成单动作 `prepare -> compile -> readback -> render -> evaluate` 执行 slice：checkpoint 和 primary-blockout/primary-form-adjustment/secondary-structure/tertiary-detail 的 single-Part geometry proposal 可执行，返回独立 reviewable candidate；未支持 action kind fail closed，永不 confirm/version/export。
- `design_stage_run_prepare` 已形成 bounded stage batch：Runtime 要求显式批准、完整 ordered input hash 和 session/stage scope，最多执行 6 个同阶段独立 ActionRun，按 RuntimeJob/event 记录进度，在首个 action/quality gate 阻断处停止，并对同一 `batch_id + input_sha256` 精确重放；batch 不合并 proposal、不提升 candidate、不 confirm/version/export。
- `design_composition_prepare` 已形成显式 ordered composition proposal：要求 2–6 个 typed geometry action、线性 `depends_on`、完整 composition hash 和用户批准；每一步复用独立 ActionRun，记录 step/aggregate/replay lineage，在首个质量门停止。可选 `merge.mode=cumulative-program` 要求每个完整 GeometryProgram@2 的 `parent_program_sha256` 链接上一步程序，并在动作批次完整通过后编译一个独立 review candidate；当前真实 focused fixture 在首个视觉门阻断，因此 merge 结果为 blocked，`confirm_allowed=false`，不应用 Repair、不改变 source candidate/version/history，也不导出。
- `bounded-repair` proposal 已支持 2–8 个经 session `ReferenceCanvas@1` 精确绑定的 `view_evaluations`；Runtime 对 source/proposal candidate 分别执行每个 view 的 RenderSet@2、ReferenceComparisonReport@1 和 QualityReport@2，写入逐 view evidence 与 `CrossViewEvidenceBundle@1`，聚合 coverage/pass/non-regression/strict-improvement；跨视图 bundle 仍由 `candidate_confirm` 以 `CROSS_VIEW_PROMOTION_REQUIRED` fail closed，尚无用户批准 promotion transaction。

这里的 `AgenticSceneObserveResult@1` 仍是可丢弃的 source transport envelope；durable session/checkpoint/RepairIntent、bounded authoring producer/readback、单动作 ActionRun、独立 stage batch 和 bounded cross-view evidence 的边界已经形成，但它们仍不等于完整组合式多动作 orchestrator、用户批准的 proposal promotion、source candidate/version mutation 或最终视觉 PASS。这个区分是本 ADR 的强制状态边界。

### 1. Agent Harness

Agent harness 只负责可观察的线性循环：

```text
Observe -> Plan -> Act -> Inspect -> Render -> Evaluate -> Checkpoint
```

它不是产品真值，不保存状态，不替代 Runtime 质量门。Codex Desktop/CLI 是 P0 harness；未来可以兼容 Pi Agent-style CLI/RPC/SDK，但必须走同一 MCP/Runtime contract。

### 2. DesignSession

新增目标概念 `DesignSession@1`。它是设计过程状态机，不直接等同于 candidate/version：

- 绑定 project/reference/candidate/version/evidence hash；
- 记录当前 stage、失败门、下一步允许动作；
- 管理 stage checkpoint 和 rollback intent；
- 限制每个 Runtime ActionRun 为一个可验证设计动作；同阶段 batch 与 composition proposal 只串联独立 receipt 并记录显式 lineage，不形成隐式组合事务。

### 3. SemanticSceneGraph / ModelUnderstandingBundle

新增目标读模型：

- part tree、roles、dimensions、bbox、symmetry、dependency、source operator、editable parameters；
- material zones、surface language、detail level、visibility；
- camera、selection、RenderSet/AOV、QualityReport、evidence hashes；
- observed/inferred/unknown 与 confidence；
- 每个自然语言解释必须能回指 `part_id`、`face_id`、`render_id`、`feature_id` 或 evidence hash。

Codex 不再只接收 object position 或 mesh count，而是接收足以做设计判断的 3D 现场。

### 4. ReferenceCanvas + DesignSpec

新增目标合同：

- `ReferenceCanvas@1`：front/side/top/perspective/material/detail references、coverage、unknowns、camera claims；
- `DesignSpec@1`：category、style、primary forms、proportion、semantic parts、material language、stage goals、risk/unknown。

单张三分之四图最多支持 `PARTIAL_VISIBLE_VIEW_PASS`；缺少 front/back/left/right/rear-three-quarter 时 `HQ_360_PASS` 固定 `BLOCKED_REFERENCE_COVERAGE`。

### 5. Stage Gates

高质量 authoring 必须按阶段门执行：

1. `reference-canvas`：参考、授权、coverage、unknowns；
2. `primary-form`：整体轮廓、比例、主形体；
3. `secondary-structure`：模块、关节、传感器、结构层级；
4. `tertiary-detail`：panel、vent、groove、fastener、linework；
5. `uv-pbr`：UV/tangent、MaterialZone、PBR/texture；
6. `final-review`：九 AOV、strict compare、Codex typed review、human review、export/restart hash。

Primary 未通过时禁止 tertiary detail；visible-view 未通过时禁止 PBR 解锁；`QUALITY_TARGET_NOT_MET` 禁止 confirm/export。

### 6. Parametric Design Kit

把当前 OperatorCatalog 上的低层 Operator 组织成 AI 友好的 typed macro：

- Housing、Panel、Vent、Joint、Frame、Sensor、Handle、Foot、Wheel、Fastener、Cable、Light；
- Codex 表达设计意图，ForgeCAD 展开为 bounded `GeometryProgram@2`/后续版本；
- macro 必须保留 source map、Part/MaterialZone lineage、validator 和 benchmark。

### 7. Visual Evidence Engine

每个 stage 必须返回同一 candidate/reference/camera hash 下的：

- beauty、silhouette、depth、normal、AO、part-ID、material-ID、wireframe、UV-stretch；
- front/side/top/perspective 或可用视图；
- dimensions、geometry stats、part stats、selection；
- compare metrics、diff/heatmap、failed gate；
- hash-only manifest，不保存用户原图字节或绝对路径。

### 8. Critic / Repair Loop

Critic 输出不能是笼统自然语言。必须是：

```text
issue_id
stage
part_id / material_zone_id
metric_name + threshold + observed
evidence_hash
proposed_bounded_action
risk
pass/fail/unknown
```

修正只允许单 Part 或单 MaterialZone 的 bounded action，然后重新 compile/readback/render/compare。

## 外部项目边界

- Omniverse Kit、OpenUSD、FreeCAD、build123d/CadQuery、BlenderMCP、MaterialX、TRELLIS/Hunyuan3D 等当前均不因此成为 adopted dependency；
- 允许学习文档、架构、工作流和 schema 思想；
- 禁止直接复制 skill、插件、任意 Python bridge、模型权重、远程 provider、`.blend` 状态或 GitHub 仓库作为 Runtime 真值；
- 任何代码/库/资产采用仍按 `EXTERNAL_PROJECT_ADOPTION.md` 的 accepted receipt、许可证、SBOM、恶意输入、确定性和 removal plan 执行。

## 实施顺序

ADR-0026 不改变当前唯一 `in_progress`：`FGC-MCP010F`。observe/plan projection 与 durable session/checkpoint/RepairIntent prepare/readback 已完成各自 source/isolated receipt；后续工作必须拆成独立、可验证的子任务：

1. 完成剩余 Agentic contract family 的独立 producer/consumer conformance（当前 authoring_context 已有 bounded multi-view producer/readback）；
2. 把 `ReferenceCanvas@1` 的多视图事实接入真实跨视图 render/compare 与 reference coverage evidence；
3. 单动作 `prepare -> compile -> readback -> render -> evaluate`、有界 stage batch 和带累计程序链的 composition merge prepare 已落地；下一步是完整 positive merge conformance、失败恢复细化与 Repair/promotion transaction；
4. bounded RepairIntent 已能生成 reviewable proposal；下一步是用户批准后的 candidate promotion，仍不得绕过 confirm/version/export gate；
5. Parametric Design Kit v0 的 typed macro producer；
6. 完整 Visual Evidence Bundle 与 critic evidence hash；
7. 真实 Codex observe→plan→bounded action→inspect loop；
8. 真实机器人 visible-view loop + human gate + export/restart hash。

## 结果

正面影响：

- Codex 能“看得见”模型，而不是盲调工具；
- 高质量被拆成可验证的阶段门；
- 局部修改和回滚成为工作流的一部分；
- 外部项目可以系统吸收思想，而不破坏 Runtime 真值。

代价：

- 需要新增合同、读模型、orchestrator 和 Viewer 阶段面板；
- 短期不能通过继续堆 detail/材质快速宣称高质量；
- 文档和 Gate 必须持续区分目标设计、source PASS、visual PASS、human PASS 和 packaged PASS。

## 非目标

- 不恢复旧 Provider、旧 Agent、旧 Workbench 或 U004；
- 不把 Pi Agent、BlenderMCP、FreeCAD、build123d/CadQuery MCP 作为 P0 runtime dependency；
- 不下载或内置 TRELLIS/Hunyuan3D 权重；
- 不用外部图生 3D mesh 直接 confirm/export；
- 不把单张参考升级为 360 高质量。
