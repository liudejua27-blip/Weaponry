# ADR-0026: Agentic Design Runtime

状态：Accepted as target architecture；当前为目标设计，尚未落地为 Schema/tool/runtime Gate
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

ADR-0026 不改变当前唯一 `in_progress`：`FGC-MCP010F`。建议把后续工作拆成 MCP010F 的重构子门或 MCP014 之后的新任务：

1. `SemanticSceneGraph@1` / `ModelUnderstandingBundle@1` Schema 与 read tool；
2. `DesignSpec@1` / `ReferenceCanvas@1`；
3. `DesignSession@1` / `DesignCheckpoint@1`；
4. `scene_observe_get`：一次返回 Codex 设计判断所需现场；
5. `design_stage_plan_get`：只读返回下一步允许动作和失败门；
6. Parametric Design Kit v0；
7. Visual Evidence Bundle；
8. Critic/Repair schema；
9. 真实机器人 visible-view loop + human gate + export/restart hash。

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
