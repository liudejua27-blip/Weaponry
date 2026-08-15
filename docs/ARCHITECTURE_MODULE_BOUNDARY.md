# ForgeCAD 架构与模块边界

2026-08-15 Stage Batch 边界补充：`design_stage_run_prepare` 只允许每个 action entry 传递受限 `action`、可选 proposal/optimization intent 和可选 object `view_spec`；Runtime 将 entry 的 `view_spec` 纳入父批次与子 `DesignActionRun` 输入哈希，并交给现有 RuntimeParameterPatch materializer。MCP 仍是薄 stdio 适配器，Runtime 仍是唯一写者，子 proposal 仍是独立 review candidate，批次不 confirm/version/export；真实同 cohort receipt 为 `docs/evidence/mcp010f/design-action-run-real-reference-stage-view-spec-20260815-b37.json`。这闭合 staged transport，不改变 Viewer read-only、Worker typed/no-script 或质量门约束。

2026-08-15 高层 ActionRun 输入边界：MCP `design_action_run_prepare` 可传递外层 `ReferenceViewSpec` 与 typed `action.parameter_changes`；Runtime 负责策略选择、单 Part/单节点约束、`RuntimeParameterPatch@1`/`RepairIntent`/review candidate 物化和五阶段回读。调用方不必提交完整 proposal，但仍不能绕过 Runtime 唯一写者、质量门、用户批准、confirm/version/export；真实回执 `docs/evidence/mcp010f/design-action-run-real-reference-runtime-auto-parameter-patch-20260815.json` 的 proposal 因 regression 被拒绝，整体仍 `QUALITY_TARGET_NOT_MET`。

2026-08-15 Desktop 入口边界已收口：`apps/desktop/src/App.tsx` 只挂载只读 `RuntimeViewer`；Desktop 不再承担图片上传、聊天式意图输入、模型生成按钮或 Agent 决策。Codex Desktop/CLI 仍是 P0 authoring 入口，ForgeCAD Desktop 只显示 Runtime-owned project/candidate/reference/AOV/quality/Job/version 投影。该修正不改变 Runtime 唯一写者和视觉质量 Gate。
2026-08-15 CADFit 模块边界增量：`OptimizationJob` 现在消费 Runtime-owned `SilhouetteRig@1` 的 `surface_control_point` typed 参数，并在同一 Part 的 `surface-shell@1` 内按控制点组执行候选生成；MCP 只传递/校验合同，Geometry Worker 只编译受限 SurfaceProgram，Runtime 负责 candidate/CAS/checkpoint/result/quality lineage，Viewer 只读投影。真实 v15 回执虽证明 39 次分阶段搜索和多控制点 readback，Part strict objective 失败时仍不生成 proposal，故没有 source candidate/version/confirm/export 变更；这条边界不把 CADFit 变成任意 optimizer，也不把 scalar loss 改善视为视觉质量。

版本：2026-08-15
状态：模块权责文档；描述当前已实现边界与 ADR-0026 目标模块。Agentic observe/plan projection、嵌套只读 projection conformance、受批准的 durable session/checkpoint/RepairIntent prepare/readback、显式 bounded authoring_context（含多视图 ReferenceCanvas/DesignSpec）CAS producer/readback、evidence-bound single-Part geometry-stage proposal、逐视图 evidence inventory、hash-bound CrossViewEvidenceBundle@1、RuntimeJob 驱动的有界同阶段独立动作批处理、带可选 `cumulative-program` 合并准备的 ordered composition proposal、source/proposal/RepairIntent/quality/evidence revalidated `repair_apply_prepare` CAS apply-intent boundary、受限 ActionRun→CADFit child handoff，以及完整覆盖 synthetic fixture 的 `cross_view_promotion_confirm` immutable-version transaction 已进入 Runtime/MCP；真实参考 Repair 实际应用/晋级、完整 orchestrator 和完整视觉闭环仍未进入当前边界。

## 1. 总体边界

2026-08-15 SurfaceProgram 增量：Geometry Worker 已加入受限 `forgecad.geometry.surface-patch@1` open patch、`forgecad.geometry.surface-shell@1` constant-thickness shell 与 `forgecad.geometry.subd-cage@1` regular quad control-cage typed operators。它们属于产品自有 Worker 的 source/focused Gate，不是完整 Visual Surface backend，也不改变 Runtime 唯一写者、candidate/version promotion 或视觉质量状态。
2026-08-15 SurfaceProgram real ActionRun boundary：真实授权参考的 `chest-shell` 已在隔离 source candidate 中以 16 控制点 `surface-shell@1` 进入 Runtime-owned `RuntimeParameterPatch@1 / surface-control-points-v1`，并完成 `prepare → compile → readback → render → evaluate` 的同 camera 质量回路；`control-point-5-z` 单点 proposal 因 composite gate regression 被拒绝，未 confirm/version。该证据说明 SurfaceProgram 已接入 staged authoring boundary，不改变 Runtime 唯一写者、MCP thin adapter、Viewer read-only 或 `QUALITY_TARGET_NOT_MET` 事实。

ForgeCAD 的边界固定为：

```text
External Agent Harness
  Codex Desktop / Codex CLI / future Pi-style harness
        |
        | MCP stdio
        v
forgecad-mcp
        |
        | authenticated local IPC
        v
forgecad-runtime
        |
        +-- SQLite V1 + CAS
        +-- Geometry / Appearance / Render Worker
        +-- CADFit OptimizationJob（proposal-only）
        +-- Quality / Evidence / Versioning
        |
        v
Read-only Viewer
```

Codex/Agent 负责理解、规划、设计判断、选择工具和迭代。ForgeCAD 负责有界 typed 几何/约束、单位、材质、渲染、版本、撤销、回读和质量证据；Manifold 已以固定 revision vendored 到 Geometry Worker，`boolean@1` 当前只开放同一 Part 的 bounded union/difference/intersection，仍不是通用布尔或视觉质量门。

## 2. 当前已实现模块

| 模块 | Owned state | 允许 | 禁止 |
|---|---|---|---|
| `forgecad-mcp` | 无数据库状态 | MCP initialize、tool/resource manifest、typed request validation、连接 Runtime | 打开 SQLite/CAS、执行模型、运行脚本、保存 Provider/API Key |
| `forgecad-runtime` | SQLite/CAS/Project/Candidate/Version/Job/Quality | 唯一写者、candidate/version/approval/export、Skill registry、QualityReport | 让 MCP/Viewer/Worker 写库、接受任意路径/URL/脚本 |
| `CADFit OptimizationJob` | Runtime Job + CAS 中不可变 `OptimizationIntent`、候选 program/GLB、evaluation/checkpoint/result | 对单一 candidate Part 先做小型参数感知 seed 探索，再围绕最佳 seed 做收缩的局部 trust-region coarse probes，随后按 `coarse→mid→final` 多保真 compile/readback/render/metrics 晋级；同 fidelity 按锁定多目标排序，best/proposal 对象 hash 分离；可选 hash-bound `OptimizationResidual@1` 只编译同一 Part 的 typed Boolean DAG；每个阶段、候选替换和评估持久化 next-stage cursor，取消后从已校验 CAS 对象继续，最多输出 proposal | 直接修改 candidate/version/export、把连续优化交给 Codex、接受任意 optimizer/script/外部 solver、把局部 proposal 当全局最优或 confirm |
| `ActionRun → CADFit handoff` | 父 `DesignActionRun` receipt + Runtime Job 子记录 + hash-bound `OptimizationIntent` | 受限 geometry ActionRun 校验 session/candidate/stage/Part/intent/camera scope，创建同 `run_id` 的 child OptimizationJob；父回执暴露 child id/hash，正常 ActionRun 的 optimizer 字段为 null，所有路径 proposal-only | proposal 与 optimizer intent 冲突、checkpoint/非几何/跨 Part 输入、外层或 nested camera hash 漂移；不自动 confirm/version，不把 `blocked-no-improvement` 写成视觉 PASS |
| Contracts | JSON Schema + canonical hash | 定义跨进程对象、版本、negative gates | 空 Schema 冒充能力、未实现 producer 就宣传 PASS |
| Geometry Worker | 临时 worker process | bounded typed Operator、GLB lowering、strict readback；当前 catalog 激活 16 个 hard-surface Operator，其中 `surface-patch@1` 输出显式 `solid=false` 的开放 Bézier patch，`surface-shell@1` 输出受限 constant-thickness watertight shell，`subd-cage@1` 输出 regular rectangular Catmull-Clark-style open surface，Boolean 支持同一 Part 的 union/difference/intersection | 网络监听、任意 Python/JS/shell、下载资产、写 Runtime DB、把开放 patch/cage 当 watertight solid、开放任意 mesh Boolean |
| Manifold adoption boundary | accepted fixed-revision source；product-owned isolated Worker slice | C API/FFI、MeshGL64 topology/readback、预算/确定性/资源/移除 Gate 已接入 `apps/geometry-worker` | 自动 CMake/clone、Python/JS/WASM binding、网络、动态 lib；不把 Boolean 变成 Runtime 写者或通用 solver |
| Appearance/Render path | Worker/Runtime evidence | MaterialZone、UV/tangent、PBR、九 AOV、reference compare | 用 beauty/截图替代 QualityReport |
| Visual Surface diagnostics | Runtime-owned read-only projection；MCP `visual_surface_get` | 绑定同一 project/candidate/reference/artifact/RenderSet/camera/compare/quality lineage；从 CAS 回读九个 512×512 AOV、参考/候选 mask、4px edge/SDF、Part-ID ROI，并从同一已通过 ArtifactReadback@2 的 candidate GLB 提供 bounded mesh-derived curvature/feature-line summary；`VisualSurfaceReadback@1` hash 投影给 Critic/CADFit | 不执行 SubD/NURBS principal curvature、zebra 或任意 mesh analysis；不写 SQLite/CAS、candidate/version，不设置 `quality_hard_gate_passed`；summary 不是视觉质量 PASS |
| SurfaceProgram operator slice | Geometry Worker + `GeometryProgram@2` | `surface-patch@1` 接收 16 个 typed Bézier control points、受限 u/v segments 和 transform；`surface-shell@1` 复用同一 cage 并以显式 thickness 生成封闭 shell；`subd-cage@1` 接收 2–16×2–16 rectangular quad cage，执行 0–2 级 bounded Catmull-Clark-style subdivision；source focused receipts 证明重复编译、strict readback 与负向边界 | 不实现 arbitrary-topology SubD、crease/extraordinary vertex、NURBS、variable thickness、trim/self-intersection、自动质量晋级或 Manifold 调用；patch/cage 仍 `solid=false`，shell source fixture 才可 `solid=true`；mesh-derived signal 另见 Visual Surface analysis receipt |
| Agentic projection | Runtime 按需派生的临时 projection；不持久化 | `scene_observe_get`、`design_stage_plan_get`、`critic_report_get`、`visual_evidence_bundle_get`；输出 observed/inferred/unknown、stage、gate、action 和 hash binding | 写 SQLite/CAS/candidate/version/checkpoint；把 projection 当 durable DesignSession 或视觉 PASS |
| Agentic durable prepare + bounded Repair | Runtime-owned SQLite/CAS session/checkpoint/ReferenceCanvas/DesignSpec/RepairIntent 与 proposal candidate | session result 先验证默认或显式 `authoring_context`；`design_action_run_prepare` 当前执行 checkpoint，以及 primary-blockout/primary-form-adjustment/secondary-structure/tertiary-detail 的严格 single-Part geometry proposal；Repair proposal 可携带 2–8 个已绑定视图，逐视图真实编译、GLB 回读、同相机渲染/比较，并写入 `CrossViewEvidenceBundle@1`；完整 coverage synthetic fixture 已通过 `cross_view_promotion_confirm` 创建不可变版本并验证 exact replay；`design_stage_run_prepare` 以 approval-gated RuntimeJob/event 驱动最多 6 个同阶段独立动作，逐个复用 ActionRun、在首个质量阻断处停止并支持精确输入 hash 重放；`design_composition_prepare` 以 approval-gated 2–6 项线性依赖封装 ActionRun，并可用 `merge.mode=cumulative-program` 校验父程序哈希链、在批次完成后编译一个独立合并候选；`repair_apply_prepare` 重新校验 source head、RepairIntent、proposal candidate、QualityReport 和可选 cross-view bundle，只写 CAS apply-intent；所有路径都返回 reviewable proposal，未实现的 action kind fail closed，`candidate_confirm` 对跨视图 bundle 保持 promotion fail-closed；MaterialZone/UV-PBR 仍是合同/目标设计，当前 executor 不执行 | 任意脚本/全局 mesh delta、非累计或未绑定哈希的合并、Repair 应用到 source candidate、直接改 source candidate/version/history、真实参考 proposal promotion/confirm/version/export、把 synthetic fixture 或 proposal receipt 当视觉 PASS |
| Viewer | ephemeral UI state | 只读 GLB/AOV/compare/selection/explosion/heatmap | 创建版本、写 SQLite/CAS、保存产品状态到 localStorage |
| Skills/AssetPack | first-party manifests + receipts | 声明式 recipe、operator lock、validator、SBOM/provenance | 可执行插件、第三方仓库直接安装、模型权重 |
| Evidence | hash-only receipts | PASS/FAIL/BLOCKED/NOT_RUN 分层记录 | 用历史 receipt 证明当前 binary，或用结构 PASS 证明视觉 PASS |

2026-08-15 Runtime parameter patch boundary：`Agentic durable prepare + bounded Repair` 现在另有一条不依赖 Codex 重发完整程序的 `RuntimeParameterPatch@1` 路径。MCP 只负责传递单 Part 参数意图；Runtime 在当前 candidate 的 GeometryProgram 中解析唯一 `primitive@2`/`panel@1` 或 `subd-cage@1`/`surface-patch@1`/`surface-shell@1` 源节点，分别支持 `primitive-dimensions-v1` 与 `surface-control-points-v1`，负责范围/单位/stale-before/lineage 校验、canonical hash、RepairIntent 生成和 review-candidate 执行。该 materializer 仍属于 Runtime 唯一写者边界，源 candidate/version 不改写；surface-shell 正向和控制点越界负向 focused tests PASS，真实用户参考视觉门仍独立阻断。

相机对象边界：自动 framing 产生的 `CameraCalibration@1` 也由 Runtime 负责 canonical round-trip、CAS 持久化和 hash 绑定；`RenderSet@2.camera_object_sha256`、session `camera_hash` 与 proposal camera 形成同一 visual lineage。MCP/Viewer 不重新序列化或写相机真值，ActionRun 只消费 Runtime 回读的 source VisualBindings camera。该边界修复的是跨进程浮点 JSON 表示漂移，不是质量门或视觉 likeness 门。

Reference coverage boundary：`request-reference` 是 ActionRun 中唯一不进入 Geometry Worker 的 orchestration branch。Runtime 只接受当前 session 绑定的 `reference_id`，在 `prepare` 阶段生成 `BLOCKED_REFERENCE_COVERAGE` receipt，并将后续 compile/readback/render/evaluate、checkpoint、OptimizationJob 和持久候选变更全部保持关闭；`DesignSession.next_actions` 以同一 reference-scoped typed action 暴露该下一步。

## 3. ADR-0026 目标模块

以下模块仍是目标设计或后续 durable work；当前 Agentic projection 与 durable prepare/readback slice 已实现，不得把它扩展为完整 orchestrator：

| 目标模块 | 责任 | 落地要求 |
|---|---|---|
| Agent Harness Adapter | 线性 `Observe -> Plan -> Act -> Inspect -> Evaluate -> Checkpoint` 编排 | 不保存产品状态；所有动作仍走 MCP/Runtime |
| DesignSession | stage、checkpoint、失败门、下一步允许动作 | 当前已由 Runtime 受批准写入 SQLite/CAS 并可跨重启 readback；session 创建可接收严格 hash-bound 的显式多视图 `ReferenceCanvas@1`/`DesignSpec@1` payload，也保留单参考 unknown 默认值；bounded Repair proposal 已可按 session 精确绑定多视图并生成 aggregate evidence，完整覆盖 synthetic fixture 已验证 promotion/replay/immutable version，真实参考上的完整多动作 orchestrator、Repair 和 candidate/version mutation 仍未完成 |
| SemanticSceneGraph | Part tree、role、dimensions、symmetry、source map、editable parameters | 从 readback/RenderSet/Quality 派生，不由 Codex 本地猜 |
| ReferenceCanvas | reference coverage、views、observed/inferred/unknown | session 创建时由 Runtime 写入；可从已导入的多个授权 `ReferenceEvidence` 组成显式 bounded view set，并在每次 session/action readback 校验 CAS/hash/project/authorization/view binding；缺失视图仍阻断 360 |
| DesignSpec | category、style、primary/secondary/tertiary goals、material language | 当前可由 Runtime 生成保守最小对象，也可接收经 bounded state/stage-gate 校验的显式 DesignSpec；是设计合同，不是 prompt |
| Visual Evidence Bundle | 多视图 AOV、camera、selection、metrics、failed gate | 当前从 durable ReferenceCanvas 与 candidate-bound evidence 生成逐视图 inventory；bounded Repair proposal 可对 2–8 个已绑定视图真实执行 render/compare，并持久化 `CrossViewEvidenceBundle@1`；complete coverage 还要求 supplied view kind 与实际 authored view entity 一致；synthetic fixture 已验证 promotion transaction，缺失 view binding 显式 fail closed；同 cohort packaged、human 和最终视觉门仍未完成 |
| Critic/Repair Loop | evidence-bound Part/MaterialZone issue 与 bounded repair | 当前对已有明确局部作用域的 single-Part proposal 执行 Repair，并支持多视图 aggregate evaluation；`DesignCriticReport@1` 绑定 VisualSurfaceReadback canonical hash，`OptimizationResidual@1` 在 Boolean/CADFit 前重新校验 surface readback 与同一 lineage；`repair_apply_prepare` 已提供源候选/RepairIntent/提议候选/质量/证据的可重放校验边界，single-view fixture/runtime confirm 与 complete-coverage synthetic cross-view promotion fixture 已通过，但真实参考仍只写 proposal/apply intent；全局视觉指标只生成诊断，不生成 null-scope RepairIntent，需先通过 candidate-bound Part error 取得局部证据；仍需完整 durable producer/orchestrator、真实参考 Repair、MaterialZone executor、真人视觉门和用户批准后的真实 promotion/confirm 流程 |
| Parametric Design Kit | Housing/Panel/Vent/Joint/Sensor/Frame 等 intent | 展开为 typed bounded program，保留 source map |
| Isolated Boolean Worker | Manifold C API 的 bounded union/difference/intersection 与 MeshGL 输入输出 | 固定 revision、静态/隔离进程、source-ID/topology/readback、确定性、资源/崩溃/移除 evidence 已通过；通用 mesh、视觉/PBR/human/export/360 仍 deferred |

## 4. 模块化目录原则

活动产品目录只放当前能力：

- `apps/desktop/src-tauri/crates/forgecad-runtime/**`
- `apps/desktop/src-tauri/crates/forgecad-mcp/**`
- `apps/geometry-worker/**`
- `apps/desktop/src/features/runtime-viewer/**`
- `packages/forgecad-contracts/schemas/**`
- `packages/forgecad-skills/bundles/**`
- `packages/forgecad-assets/**`
- `docs/evidence/mcp*/**`

隔离目录只放历史或废弃材料：

- `docs/evidence/archive/**`
- `packages/forgecad-skills/archive/**`
- reset/private archive 路径，例如 `/tmp/forgecad-mcp001-20260807`

任何废弃代码、文档或模块不得继续留在活动目录根部；必须移动到 archive/quarantine，或删除前保留可恢复 receipt。当前脏工作树不得无证据删除用户数据或未提交修改。

## 5. 清晰架构验收

每个新增模块必须在文档里回答：

1. 谁是唯一写者；
2. 输入/输出 Schema 是什么；
3. 是否持久化；
4. 是否可重建；
5. 是否允许网络、脚本、路径、模型调用；
6. 对应 Gate 和 evidence 在哪里；
7. 与旧模块的隔离关系是什么。

如果回答不清楚，不允许进入 active capability。
