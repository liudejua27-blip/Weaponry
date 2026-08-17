# ADR-0026: Agentic Design Runtime

状态：Accepted as target architecture；observe/plan read-only projection、durable session/checkpoint/RepairIntent prepare/readback slice、`repair_intent_run_prepare` CAS-bound bounded run 与 MCP010F 窄范围 Primary Form 单动作 prepare/evaluate、bounded action-run/readback 已落地，通用单动作 orchestrator、Repair 应用和完整视觉 Gate 仍未完成
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
- MCP010F 另外提供窄范围 `primary_form_repair_prepare`：Codex 只传一次 target/camera/Rig/optimizer typed intent，Runtime 在同一 bounded action 内完成 fit、typed GeometryProgram、strict readback、隔离 Render Worker 九 AOV 和 candidate-bound compare，结果是 staged candidate + Runtime QualityReport；它不 confirm、不创建 version、不 export，且无严格改善时保持 source candidate 不变；
- MCP010F 另提供 `design_action_run_prepare`/`design_action_run_get` 的窄范围 action-run slice：Codex 只提交一次已批准、单 Part、`primary-form` bounded action，Runtime 绑定 session/project/candidate/reference scope 后复用上述 pipeline，并把本回合唯一 `AgenticSceneObserveResult@1.canonical_sha256` 固化为 `DesignActionRun@1.observation_sha256`，与结果一起写入 SQLite/CAS；结果锁定 confirm/export，幂等 prepare/get 不改变 candidate/version 或用户持久数据。该 slice 不是通用多阶段 orchestrator；
- `repair_intent_run_prepare` 将 Runtime-owned CAS `RepairIntent@1` 与 exact session/candidate/observation/evidence/reference/camera binding 接入同一 bounded `compile → readback → render → compare`，调用方不能覆盖 intent，只返回 staged `RepairIntentRunResult@1`。最终 Dev.app 的真实授权参考 packaged transport 已通过，但视觉在 camera evidence gate blocked；Repair apply/confirm、通用 orchestrator 与质量晋级仍未完成；
- Viewer 通过 authenticated IPC 查询 durable session，仍只展示 read model，不提供写入按钮；完整重启后的 session/checkpoint/intent receipt 为 `docs/evidence/mcp010f/agentic-runtime-session-checkpoint-20260813.json`，合同检查为 `scripts/check_agentic_runtime_receipt.py`。

这里的 `AgenticSceneObserveResult@1` 仍是可丢弃的 source transport envelope；durable session/checkpoint/RepairIntent 只在上述受限 prepare/readback/run 范围内存在。真实 Runtime 的嵌套只读 projection 已有独立 producer/consumer conformance 回执，但 durable/reference/DesignSpec 的完整 producer 尚未形成。`primary_form_repair_prepare` 与 `repair_intent_run_prepare` 都只覆盖 bounded staged/evaluate slices，尚未形成通用单动作 orchestrator、Repair 应用或视觉质量通过。这个区分是本 ADR 的强制状态边界。

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
- 限制 Codex 一次只做一个可验证设计动作。

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

1. durable/reference/DesignSpec 与剩余 Agentic contract family 的完整 producer/consumer conformance（嵌套只读 projection 已有独立 Gate）；
2. `DesignSpec@1` / `ReferenceCanvas@1` 的独立 CAS-bound producer/readback 和真实参考覆盖证据；
3. 从当前 Primary Form 专用 action-run slice 推广为通用单动作 `prepare -> compile -> readback -> render -> evaluate` orchestrator；
4. bounded RepairIntent 执行和用户批准边界；
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
