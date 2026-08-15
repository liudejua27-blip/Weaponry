# FGC-MCP010 高质量硬表面参考闭环计划

2026-08-15 staged Agent transport 增量：`design_stage_run_prepare` 已支持 action entry 的 `view_spec`，并将 typed `parameter_changes`、`view_spec` 和 action 一起 hash-bind 后交给 Runtime-owned `DesignActionRun`；Runtime 自动物化受限参数补丁和独立 review candidate。真实 receipt `docs/evidence/mcp010f/design-action-run-real-reference-stage-view-spec-20260815-b37.json` 在四组件同 cohort 中通过，子运行到 evaluate 后由 `QUALITY_TARGET_NOT_MET` 停止，批次 checkpoint/replay 和源 candidate immutability 通过。该项是 img2threejs 式分阶段输入/执行边界，不把 transport PASS 写成视觉质量 PASS，也不替代 Critic→Repair 或 Repair promotion。

2026-08-15 高层 ActionRun 自动参数补丁：Runtime/MCP 现在支持仅由 typed `action.parameter_changes` + 外层 `ReferenceViewSpec` 驱动的 bounded materialization；Runtime 自动选择 `surface-control-points-v1`，生成 `RuntimeParameterPatch@1`、RepairIntent 和 review candidate，完成 `prepare → compile → readback → render → evaluate`。真实授权参考回执 `docs/evidence/mcp010f/design-action-run-real-reference-runtime-auto-parameter-patch-20260815.json` 使用 cohort `a21e448f…f057106`，五阶段已完成但 proposal 为 `rejected-regression`、视觉为 `QUALITY_TARGET_NOT_MET`，没有 confirm/version/export。该项只把高层 ActionRun 接到已有 typed patch 执行器，不等于完整 Critic→Repair、likeness、高质量或通用 Manifold Boolean。

2026-08-15 SurfaceRig 多控制点 v15：`OptimizationJob` 现在把 `surface_control_point` 显式作为 `SilhouetteRig@1` typed 参数，并按相邻控制点分组进行 bounded seed/trust-region search；真实回执 `docs/evidence/mcp010f/optimization-job-real-reference-surface-control-groups-v15.json` 在同一参考、同一 camera 和 cohort `a21e448f…f057106` 上完成 39 次 `32/4/3` 评估。30 个非 baseline candidate 物化了多控制点变化，Manifold residual 仍限于同一 Part；final loss `0.379948070031 → 0.379344569175`，但 Part strict objective 未满足，故 proposal/promotion fail closed，candidate/version/export 不变，整体仍 `QUALITY_TARGET_NOT_MET`。这一步完成了局部 SurfaceProgram 搜索基础设施，不把 scalar loss 改善写成 likeness 或高质量通过。

2026-08-15 ActionRun execution increment：为把 CADFit/Manifold/VisualSurface 的诊断真正接到可执行修正，Runtime 新增 `RuntimeParameterPatch@1` 的 bounded materializer。P0 当前只允许单 Part、单源节点、`primitive@2`/`panel@1` 的 dimension/position/rotation/radius patch；Runtime 重新 hash/compile/readback/render/compare 后生成独立 review candidate 和 RepairIntent，绝不直接改写 source candidate 或 version。另已修复自动 framing 相机在跨语言 JSON round-trip 后的 canonical hash 漂移：`RenderSet@2.camera_object_sha256` 绑定 Runtime-owned 相机 CAS 对象，ActionRun 在 proposal 执行前重绑定并验证该对象。诊断 UI 截图探针现在可到达 evaluate，但仍不是机器人参考，视觉仍 `QUALITY_TARGET_NOT_MET`；真实用户参考/视觉 likeness 仍须单独验证。

2026-08-15 SurfaceProgram Operator source Gate：`surface-patch@1` 的确定性 bicubic Bézier open patch 与 `surface-shell@1` 的 bounded constant-thickness shell 已接入 `GeometryProgram@2` typed operator set；4×4 cage 的 128/320 triangle fixtures、strict readback、UV/tangent、watertight shell 与负向参数 Gate 通过，receipts 为 `docs/evidence/mcp010f/visual-surface-patch-gate-20260815.json`、`docs/evidence/mcp010f/visual-surface-shell-gate-20260815.json`。这仍只是 Visual Surface 的几何 source lane，不升级完整 surface backend、通用 Boolean、likeness 或高质量状态。

2026-08-15 contour comparator hardening：`silhouette_candidate_compare` 已与 CADFit 的可见质量优先级统一为 Boundary F1 → Silhouette IoU → bbox → centroid → SDF，复合 loss 仅用于最后 tie-break；真实同 cohort（`b37f7116fdab4ec6ea1a57b75bbb922addf3d5ef2d32c51562d5e12aa7cdfe2f`）5 候选回执 `docs/evidence/mcp010f/part-correction-real-reference-20260815-b37-priority-cohort.json` 通过，仍 `QUALITY_TARGET_NOT_MET`，不解锁后续质量门。

2026-08-15 Critic→PartError→CADFit/Manifold 增量：真实回执 `docs/evidence/mcp010f/optimization-job-real-reference-20260815-critic-part-bool-b37.json` 将显式 `target_sha256` 传入 `critic_report_get`，Runtime 重新读取同候选的 `SilhouettePartErrorResult@1` 并生成 scoped projection intent；同一 Part residual 进入 Manifold C-ABI，完成 32/4/3 多保真评估，`strict_improvement=true`、`proposal_status=proposed`。这只闭合 hash-bound 局部诊断、Boolean residual 和 CADFit proposal/readback；视觉仍 `QUALITY_TARGET_NOT_MET`，无 confirm/version，不是 likeness、高质量或通用 mesh Boolean PASS。

2026-08-15 ActionRun→CADFit 真实 handoff：`design_action_run_prepare` 已接入可选 hash-bound `OptimizationIntent@1`。Runtime 只允许受限 single-Part geometry action 创建同 `run_id` 的 child `OptimizationJob`，先校验 session/candidate/stage/Part/intent/camera hash；父 ActionRun 保持 review-only，proposal 与 optimizer intent 互斥，checkpoint/非几何/跨 Part 输入 fail closed。真实回执 `docs/evidence/mcp010f/design-action-run-cadfit-handoff-real-reference-20260815.json` 完成 32/4/2 共 38 次评估，但 `strict_improvement=false`、`proposal_status=blocked-no-improvement`；source candidate unchanged、version count 0、未触碰持久用户数据。此项是 Agentic handoff/transport PASS，不是视觉 likeness、高质量、Repair 应用或 360 PASS。

2026-08-15 CADFit/Manifold v19c 局部绑定修正：`SilhouetteTarget@1.part.region` 现在允许 Runtime 消费由参考图像标注的归一化 ROI；`target_part_region_mask` 将 Runtime-owned 全身 mask 裁剪到该 ROI，CADFit `part_region`、Part contour error 和 Part fit envelope 优先使用它，缺少 ROI 时才回退到声明的 contour slice。真实回执 `docs/evidence/mcp010f/optimization-job-real-reference-20260815-boolean-residual-v19c.json` 在同一授权参考上通过 32/4/2 共 38 次评估，`part_target_binding=PASS_IMAGE_DERIVED_REGION_BOUNDED_REFERENCE_MASK`，`chest-armor` ROI 为 `x=0.292,y=0.285,width=0.435,height=0.285`；candidate 0 无 Boolean，候选 `[1..9]` 进入 product-owned Manifold Boolean lane，baseline/best loss `0.472832854888/0.470494645212`，但 `strict_improvement=false`、`proposal_status=blocked-no-improvement`。ROI 是显式图像区域，不是语义分割或 likeness 通过；没有 confirm/version/持久用户数据写入，仍 `NO_LIKENESS_PASS_CLAIM`、`QUALITY_TARGET_NOT_MET`。

2026-08-15 residual family v3 增量：真实回执 `docs/evidence/mcp010f/optimization-job-real-reference-20260815-boolean-residual-v18.json` 采用 `seed-then-adaptive-trust-region-v3-residual-family`，把 candidate 0 锁定为未修改 baseline，仅为后续 9 个候选启用确定性的 same-Part Boolean residual family，其余预算仍用于 Rig 探索；旧 strategy checkpoint 会 fail closed。该 Job 完成 32/4/2 评估，baseline/best loss `0.480812392901/0.478756028517`，候选池含 `residual-chest-sphere-boolean` 与 `product-owned-Manifold-C-ABI` readback；最终 winner 是无 Boolean 的 shape-only 候选，`strict_improvement=false`、`proposal_status=blocked-no-improvement`，没有 confirm/version/持久数据写入。该增量把“Boolean 可搜索但必须经过锁定多目标门”的语义补齐，仍不是语义 Part likeness、通用 mesh Boolean 或视觉高质量 PASS；当前残差仍限于单 `chest-shell` lane。

2026-08-15 最新源代码覆盖：当前为 123 个 JSON Schema、40 read + 30 opt-in write = 70 tools。新增 `VisualSurfaceReadback@1` 的同 candidate mask/edge/ROI/AOV CAS readback，以及 `DesignCriticReport@1.visual_surface` → `OptimizationResidual@1` 的 readback canonical hash/lineage revalidation；新增 `SilhouetteEvaluationObjective@1`、`silhouette_evaluation_objective_prepare` 与 `silhouette_objective_compare`，把 global/Part target、PartError、camera 和 promotion policy 固定为只读 objective；另有 `RepairApplyConfirmRequest@1`/`RepairApplyConfirmResult@1` 与 `repair_apply_confirm` 单视图消费路径；`design_action_optimization_proposal_prepare` 只读取已完成的 ActionRun-bound CADFit 结果，重新编译独立 review candidate 并绑定显式 ViewSpec，不自动 Repair 或 Confirm。Runtime 会在 fresh approval 后重新验证 source/run/proposal/RepairIntent/artifact/visual lineage，并交给 Store 创建 immutable version；多视图仍必须走 `cross_view_promotion_confirm`。合同/Stage0 静态检查、Visual Surface readback/negative Gate、Repair focused test 与 workspace test 已通过。该增量不改变 `QUALITY_TARGET_NOT_MET`、`BLOCKED_INCOMPLETE_BINDING`、camera `MISMATCH`、PBR、人评、export/restart 或 360 状态。

2026-08-15 v11f OptimizationJob 退出 Gate：`OptimizationIntent@1.evaluation_objective_sha256` 让统一 objective 成为 Job 的显式输入；Runtime 对 checkpoint、resume、final ranking 和 result policy 做 hash/readback 校验，Global 采用 non-regression，Part 采用 strict-improvement，二者任一失败即 `blocked_global_or_part_objective`。真实四组件同 cohort receipt `docs/evidence/mcp010f/optimization-job-unified-objective-real-reference-20260815-cohort-v12.json` 完成 39 次 `32/4/3` 搜索，objective 内部门为 `ready`，但 fixed-camera likeness 比较为 `QUALITY_TARGET_NOT_MET`；Manifold Boolean residual 只证明 bounded same-Part operator lane，候选仍 proposal-only，不允许 confirm/version/export。

计数校正：下方早期 reconciliation 段落中仍出现的 115/118 Schema、37 read + 28/30 opt-in write = 65/67，均是历史快照；当前以本段与 Stage 0 marker 的 123 / 40 + 30 = 70 为准。

版本：2026-08-15
状态：`FGC-MCP010A done`；`FGC-MCP010B blocked/deferred（Darwin OS memory hard cap NOT_RUN）`；`FGC-MCP010C source-focused PASS_WITH_UNRUN_VISUAL_GATES`；`FGC-MCP010D source-focused PASS_WITH_BOUNDED_BOOLEAN_AND_VISUAL_GATES（product-owned Manifold union/difference/intersection raw Worker PASS，current packaged/视觉门仍 NOT_RUN）`；`FGC-MCP010E source-focused PASS_WITH_DEFERRED_EXTERNAL_GATES（当前 packaged E 结构性探针 PASS，但视觉/人评/导出仍 NOT_RUN）`；唯一 `in_progress` 为 `FGC-MCP010F`（Viewer source、packaged CLI read-model、原生窗口与核心控件 smoke PASS；同一 provisional observation 的 packaged Viewer 绑定、正式 VoiceOver、人评和 360 仍 `NOT_RUN/BLOCKED`）。ADR-0026 已新增 Agentic Design Runtime 目标架构；它不改变当前 F 状态。
依赖：`FGC-MCP009 done（MVP host golden path）`

当前账本校正：源码合同为 123 个 JSON Schema，工具面为 40 个默认只读工具和 30 个显式 opt-in write 工具（共 70）。新增 `BooleanRequest@1`/`BooleanResult@1` 绑定 product-owned Manifold Boolean Worker；`boolean@1` 当前支持同一 Part scope 的 union/difference/intersection，不开放通用 mesh Boolean。`SilhouetteEvaluationObjective@1` 把 automatic global target、refined Part target parent lineage、PartError canonical hash、Runtime-owned camera ref 和 promotion policy 固定为同一只读 objective；真实 objective compare 仍在全局与局部改善冲突时 fail closed。`silhouette_part_error_get` 提供 hash-bound 的多 Part 局部轮廓误差表，`design_stage_run_prepare` 提供 approval-gated 有界同阶段独立动作 batch，`design_composition_prepare` 提供显式 2–6 步线性 composition proposal，`repair_apply_prepare` 提供 source/proposal/RepairIntent/quality/evidence revalidation 后的 CAS-backed apply-intent，`repair_apply_confirm` 已通过 focused fixture/runtime transaction；`design_action_optimization_proposal_prepare` 只物化严格且 non-regressing 的独立优化候选，视觉门仍需单独通过且不自动 Confirm；`CrossViewEvidenceBundle@1` 与 `cross_view_promotion_confirm` 提供 bounded Repair proposal 的逐视图 aggregate evidence、hash/approval 校验与 Promotion fail-closed boundary；Agentic projection、逐视图 evidence inventory 与 durable session/checkpoint/RepairIntent prepare/readback 另有独立 receipt；它们仍是结构/编排能力，不是 likeness 通过。

Stage 0 机器真值唯一入口为 `docs/evidence/mcp010f/current-benchmark-truth.json`。attempt35 只是 provisional retained observation，不是已成立 benchmark：它是 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，fit/compare camera 为 `MISMATCH`；当前 packaged Viewer CLI read-model 已完成 current-cohort lineage binding，但正式 UI/accessibility E2E 仍未运行。因此 source/read-model/window/control smoke 均不能补写为同一 candidate 的 packaged visual E2E，更不能越过 PBR likeness、独立真人、export/restart 或 360 门。

2026-08-15 最新增量：真实 Codex v3 已完成当前源码 cohort 的 camera-fit→compare binding、9 AOV typed image transport/hash 和非持久化 image blocks；11 个 Codex turn 全部 exit code 0，26 parts/4704 triangles 的 artifact/readback 通过。比较 IoU `0.529998`、Boundary F1 `0.073554`、bbox `0.203125`、centroid `0.031885`、landmark coverage `0`、region median `0`、critical min `1.0`，仍未过严格视觉门。随后单 Part contour probe 用同一授权 PNG 读取 `chest-shell` 局部误差，编译/回读/比较 5 个候选，复合 loss winner IoU `0.745135`、Boundary F1 `0.340045`，未晋级。CADFit synthetic 与真实授权 PNG 进程均完成多保真 coarse/mid/final 评估；修正 OptimizationJob 与 `reference_compare_prepare` 的 Boundary F1 物理容差后，真实 PNG 的 final baseline loss `0.388631`→final proposal loss `0.384008`，coarse-8 overall best `0.371270`，proposal compare IoU `0.744929`、Boundary F1 `0.339688`，仍 `QUALITY_TARGET_NOT_MET`；proposal program/GLB hash 经隔离 Runtime 重启读回。Manifold same-Part bounded union/difference/intersection raw Worker 继续通过；这些都是 transport/结构/局部 proposal 证据，不解锁 likeness、PBR、人评、confirm/export 或 360。

2026-08-15 CADFit 相机交接增量：真实参考 CADFit 探针已补齐 `camera_fit_prepare → silhouette_fit_prepare → OptimizationJob` 的 hash-bound 顺序。receipt `docs/evidence/mcp010f/optimization-job-real-reference-20260815-v8.json` 记录初始相机 `8cd20605…a535`、silhouette-fit winner `27d180d2…c0c` 和 `PASS_SILHOUETTE_FIT_TO_OPTIMIZATION`；38 次评估全部使用 winner camera，final baseline/best loss 为 `0.381100/0.378275`。由于严格多目标条件未通过，Runtime 正确返回 `blocked-no-improvement`，没有 proposal/version/confirm；这修复了优化入口的相机漂移风险，但不改变机器人 `QUALITY_TARGET_NOT_MET`、Stage 0 provisional camera mismatch、PBR、人评、导出或 360 状态。

2026-08-15 Boolean residual 增量：真实参考 v14 将 `critic_report_get` 与 `silhouette_part_error_get` 的 hash 绑定输入编译为同一 `chest-shell` Part 的 `OptimizationResidual@1`，由 product-owned Manifold C-ABI Worker 执行 union，再进入 32/4/2 的 CADFit 多保真评估。`boolean_node_ids`、最终 program/readback、camera binding 和无 confirm/version 均有 receipt；严格多目标 gate 仍拒绝 `proposal_status=blocked-no-improvement`，因此该增量是 bounded transport/readback PASS，不是视觉 likeness、高质量或通用 mesh Boolean PASS。MCP camera canonical rebind 以及 profile-extrude Boolean union/difference/intersection raw、thin-surface tangent regression 已通过；xatlas、Khronos Validator、PBR likeness、人评、export/restart 和 360 仍不变。

<!-- forgecad-stage0: schemas=123 schema_set_sha256=583fd0d2615f09e66d16c58fca8d4ab60f1856d1de427b5b9e390c8c8b137f67 read_tools=40 write_tools=30 total_tools=70 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260815-b37-complete-auto-v3.json latest_completed=real-codex-cli-current-20260815-b37-complete-auto-v3.json -->

本文是 MCP010A–F 的唯一详细执行合同。它不改写 MCP005–009 的历史 evidence，也不把目标 Schema、工具、Skill、库或素材写成当前能力。

ADR-0026 补充本计划的方向：MCP010F 之后不能继续只靠堆 operator/detail/material 追求高质量，必须把 `ReferenceCanvas → DesignSpec → SemanticSceneGraph → stage gates → Visual Evidence Bundle → Critic/Repair` 变成产品化 authoring loop。在对应 Schema、Runtime producer、MCP tools 和真实 Codex evidence 完成前，这些仍是目标设计。

当前已完成的 Agentic slice 包含 Runtime-owned read-only projection 与受批准的 durable prepare/readback：`scene_observe_get`、`design_stage_plan_get`、`critic_report_get`、`visual_evidence_bundle_get` 通过 source/runtime/MCP/Viewer Gate；`session_create_or_resume`、`session_get`、`checkpoint_prepare`、`checkpoint_get`、`checkpoint_restore_prepare` 通过合同/重启 receipt。后者只持久化 session/checkpoint 并生成 CAS-only RepairIntent，不执行 Repair，不解锁视觉/PBR/confirm/export；后续质量任务必须继续读取本计划的 candidate/camera/quality truth。

## 1. 目标和声明边界

把现有 primitive blockout 升级为 Codex 可驱动、可回读、可比较、可局部修改和可回退的首个白色硬表面机器人质量闭环：

```text
ReferenceEvidence
→ typed detail inventory
→ GeometryProgram/AppearanceProgram
→ mesh + UV/PBR + self-contained GLB
→ fixed RenderSet/AOV
→ reference metrics + typed visual review
→ user review/approval
→ immutable version → restore → CAS export
```

当前只有一张裁切腿脚、正面三分之四参考。因此：

- 本轨道首先验收可见视图，最高状态是 `PARTIAL_VISIBLE_VIEW_PASS`；
- 用户补充 front、back、left、right、rear-three-quarter 五张同设计全身参考之前，`HQ_360_PASS` 固定为 `BLOCKED_REFERENCE_COVERAGE`；
- 隐藏结构必须标记 `unknown/inferred`，不能以对称或想象伪装成参考事实；
- 本轨道不承诺骨骼动画、制造 CAD、工程安全、跨类别通用重建或公开发行。

当前高质量主路径固定为 `GeometryProgram@2` + active detail Operators → `ArtifactReadback@2` strict readback →（轮廓门解锁后）`AppearanceProgram@2` → `RenderSet@2` 九 AOV → candidate-bound strict `reference_compare_prepare` → typed `visual_review_submit` / `QualityReport@2`。`[transition-v1]` `GeometryProgram@1` + primitive-only + `RenderSet@1` 四 pass 只保留 MCP007–009 兼容/结构导出，不得作为当前高质量、reference likeness、PBR 或 360 路径。

未来 Agentic high-quality 主路径在上述硬门之前增加设计理解层：

```text
ReferenceCanvas@1
→ DesignSpec@1
→ SemanticSceneGraph@1 / ModelUnderstandingBundle@1
→ DesignStagePlan@1
→ single bounded action
→ strict readback / AOV / compare
→ DesignCriticReport@1
```

该路径的新增对象只能从 Runtime/Worker/Render/Quality evidence 派生；不得让自然语言、截图或外部 DCC 状态成为真值。

## 2. 当前事实与目标分离

### MCP010B/C current source reconciliation

最新校正：`d9c23b…ac0bd` 是当前安装并已由用户完整重启加载的 Skill-overlay Dev.app，已通过 package/raw/real-Codex V2 structural probes 和 live Desktop structural activation。`5143ac3b…6e61`、bfa56 与更早 cohort 保留为历史 receipt；当前 live Desktop 为 d9。

MCP006 的 44-contract、MCP010B 已保存的 50/52-contract aggregate，以及 3c/f488/bfa56/d9 Dev.app/raw/CLI receipt 都是历史或结构事实，原样保留。当前源码总计 **115 个 JSON Schema**：历史合同、MCP010B/C/D/E/F 当前合同，以及 Agentic contract family，其中新增 `BooleanRequest@1`/`BooleanResult@1`。当前 source Gate 已通过 B 的 V2 geometry/readback/Worker isolation、C 的 fixed renderer/九 AOV/reference compare/review raw path、D 的真实 Operator、E 的离线 AssetPack/UV/PBR/MikkTSpace raw path，以及 F 的哈希绑定轮廓目标、扩展相机搜索、Runtime-owned camera reference、受限 Rig/SDF fit、Runtime-owned bounded Primary Form search、单/多 Part contour proposal、Part error table 和 candidate compare Runtime path；Agentic projection、bounded multi-view authoring/readback、逐视图 evidence inventory、single-action geometry ActionRun、bounded independent stage batch、ordered composition proposal、hash-linked cumulative merge prepare、`repair_apply_prepare` apply-intent 与 durable prepare/readback 也通过合同/Viewer/隔离重启/focused probe；当前工具面为 37 read + 28 opt-in write。C/E/F receipt 使用 synthetic 或结构性 reference，只证明绑定、传输、持久化和 deterministic bytes，不证明 PBR likeness、用户 robot likeness、Viewer/package/live、人评或 360。当前 ActionRun/stage batch/composition proposal/merge-chain validation/Repair apply prepare 仍不证明 positive composition merge、Repair execution、跨视图 render/compare 或视觉 PASS。下文任何未特别注明的旧“50/52/65-contract/current Dev.app”叙述都应按本段分层。

| 项目 | 当前事实 | MCP010 目标 |
| 说明 | 上一段中的 `5143ac3b…6e61` 与 bfa56 仅是历史 package/live receipts；当前安装包与完整重启后的 live cohort 以 `d9c23b…ac0bd` 为准 | live 证据仍只证明结构工具链，不提前写成视觉/PBR能力 |
|---|---|---|
| 合同 | MCP006 历史为 44 个 JSON Schema；当前 MCP010B/C/D/E/F source contracts 与 Agentic contract family 使当前 manifest 为 115 个 JSON Schema（含 `CameraCalibrationRef@1`、`SilhouettePartErrorResult@1`、`BooleanRequest@1`、`BooleanResult@1`、`DesignSession@1`、`DesignCheckpoint@1`、`RepairIntent@1`、`DesignActionBatchResult@1`、`RepairApplyPrepareResult@1`） | 维持版本化合同；后续任务只可新增有证据的 Viewer/闭环合同 |
| MCP | 当前源码为 37 read + 28 opt-in write（65）；F 新增 Runtime-owned `silhouette_rig_hash` 以避免 Codex 本地重算 Rig canonical hash，以及 `silhouette_part_error_get` 多 Part 误差表；Agentic 新增 projection tools、session/checkpoint readback、approval-gated prepare tools、`design_stage_run_prepare` 独立 stage batch、`design_composition_prepare` ordered proposal 和 `repair_apply_prepare` CAS-backed apply-intent；C/E/F/Agentic source raw/restart/focused Gate 已按各自范围通过，历史 Dev.app receipts仍按 cohort 保存 | 真实用户 likeness、同一 candidate 的 packaged Viewer、人评/PBR/360证据仍需独立 Gate；不得用 synthetic/raw、prepare receipt、stage batch、composition proposal 或 projection 直接宣传高质量 |
| Skill | 十个历史 first-party `0.1.0` declarative Bundle + 当前 `primitive-blockout@0.2.0`、`hard-surface-detail@0.2.0`、`uv-pbr@0.2.0` active overlay；AssetPack 独立于 Skill | 仅在真实 consumer、bundle integrity、AssetPack provenance 和 benchmark 都通过后保持 active |
| 几何 | MCP010D source 已提供 primitive、profile/extrude、loft、revolve、tube-sweep、transform、mirror、array、boolean（同一 Part bounded union/difference/intersection）、panel、vent-array、joint-stack、part-output；product-owned Manifold C API Worker adoption/raw Gate 已通过 | current packaged D rebuild、真实用户视觉阈值仍未运行 |
| Render | `[transition-v1]` MCP008/009 保留四个 compatibility pass；MCP010C source 已有 512×512 perspective/z-buffer + 九 AOV + local metrics；MCP010F source Viewer 与 packaged CLI read-model 可读取这些 AOV并做临时对比 | 核心控件 smoke 已运行；同一 provisional observation 的 packaged binding、正式 VoiceOver、真实用户视觉阈值、export/restart hash仍未运行 |
| 材质 | bounded glTF PBR：embedded baseColor/normal/metallic-roughness/AO/emissive channel sampling | first-party 离线 AssetPack、纹理、clearcoat/emissive strength |
| Viewer | 只读 GLB canvas/read model；compare/AOV/diff/Part/MaterialZone/explosion/heatmap source surface、packaged CLI read-model、原生窗口与核心控件 smoke 已通过 | 同一 provisional observation 的 package binding、正式 VoiceOver、真人视觉门仍独立验收 |

只有对应 producer、consumer、negative/focused/真实 Codex evidence 全部通过后，能力才能从 `planned/unavailable` 变为 `available`。

### ADR-0026 Agentic Design Loop target

下列能力是 MCP010F 后续重构目标，不属于当前 MCP010F source PASS：

| 目标能力 | 当前状态 | MCP010 后续退出要求 |
|---|---|---|
| `SemanticSceneGraph@1` / `ModelUnderstandingBundle@1` | 目标设计 | 从 candidate/readback/RenderSet/Quality 派生，返回 Part roles、dimensions、symmetry、source map、MaterialZone、camera、selection、uncertainty |
| `ReferenceCanvas@1` / `DesignSpec@1` | 目标设计 | 绑定 reference CAS hash、视图 coverage、observed/inferred/unknown、primary/secondary/tertiary goals |
| `DesignSession@1` / `DesignCheckpoint@1` | 目标设计 | Runtime-owned stage/checkpoint/rollback projection；永久写仍走 candidate/version |
| `scene_observe_get` / `visual_evidence_bundle_get` | 目标设计 | 一次返回 Codex 设计判断需要的 hash-bound 现场；默认只读 |
| Parametric Design Kit | 目标设计 | Housing/Panel/Vent/Joint/Sensor/Frame 等 intent 展开为 typed bounded Geometry/Appearance programs |
| `DesignCriticReport@1` / `RepairIntent@1` | 目标设计 | 只输出 evidence-bound 单 Part/MaterialZone repair，不直接写几何 |

这些能力完成前，当前可见视图仍按已有 `reference_mask_prepare → silhouette_target_get → scene_observe_get → camera_fit_prepare → silhouette_rig_hash → silhouette_fit_prepare → reference_compare_prepare → render_pass_get → visual_review_submit → quality_get` 链路执行；其中 `scene_observe_get` 是同一视觉回合的一次 canonical Runtime projection，不能被拆成跨轮次的零散观察。

## 3. 原子任务链

### 3.1 FGC-MCP010A — 权威重排与开发激活

Owned：权威文档、文档 checker、用户级开发 App 构建/激活、原始 stdio/CLI/真实 Codex capability evidence。

MCP010B 的 authoring 可用性补口已经在当前源码完成：公开但只读的 `operator_catalog_get` 返回与 `forgecad://operators/catalog` 完全相同的 Runtime-owned catalog，`geometry_program_hash` 由 Runtime/Worker 的同一 canonical JSON 实现校验无 hash V2 draft 并返回 compiler-owned hash。后者不编译、不创建 candidate/Job，也不写 SQLite/CAS；它已由 `catalog → hash → prepare` raw stdio、隔离 source-focused V2 structural Gate 和完整重启后的 live Desktop hash 调用验证。它不属于 010A 已安装 Dev.app 的 30-tool 或 MCP010B 早期 `3c6f59…7140` pre-graph 历史 receipt；`f4885b11…6bc1` 所记录的 package/isolated V2 semantic-Part graph raw activation 与 exact packaged Worker structural E2E 也是历史安装 cohort receipt。当前 `d9c23b…ac0bd` Dev.app 已通过 ad-hoc/package/Worker、isolated V2 raw、real-Codex structural 和用户完整重启后的 live Desktop structural 路径；live 证据仍不宣称视觉/PBR。

必须：

1. 把任务索引重排为 010A–F；同一时刻只允许一个原子任务处于 `in_progress`；
2. 从同一源码 revision 构建 `forgecad-mcp`、`forgecad-runtime`、Worker 和 Viewer；
3. 安装到 `~/Applications/ForgeCAD Runtime Dev.app`，开发期只允许本机 ad-hoc 签名；
4. Codex 用户配置指向 App Resources 中的 `forgecad-mcp`，不再引用 `forgecad-mcp-host`；仓库配置不写 token、fixture data dir、用户名或用户绝对路径；
5. 原始 MCP/CLI Gate 通过后，由用户重启 Codex；真实调用证明 `capabilities_get`、临时 `project_create`、Runtime `Ready` 和 MCP/Runtime 相同 build hash。

退出：用户重启后的真实证据必须证明工具、Runtime Ready、能力 cohort 和临时项目读回；本次已满足并将 010A 标记 `done`。不得自动领取 010B。ad-hoc 开发 App 不是 MCP013 的签名安装包。

当前进度：010A/010B 历史与 source structural evidence 见 `docs/evidence/mcp010a/`、`docs/evidence/mcp010b/`；C source evidence 见 `docs/evidence/mcp010c/`；D/E/F source evidence 见 `docs/evidence/mcp010d/`、`docs/evidence/mcp010e/`、`docs/evidence/mcp010f/`。当前源码总计 115 个 JSON Schema、37 read + 28 opt-in write = 65 个工具。用户第一次 Desktop restart 的 FAIL receipt和后续结构 PASS receipt均保持原样。C 当前源码已在隔离 raw stdio 中证明固定 renderer/九 AOV、candidate-bound comparison、MCP image block、Codex typed visual review 与 deterministic bytes；D 已证明 13-entry/13-active catalog 和 product-owned Manifold bounded Boolean raw Worker；E raw stdio 已证明 AssetPack manifest/provenance、embedded PNG textures、512px UV atlas、固定 mikktspace、PBR bindings 和同一九 AOV render path；F source Gate 已新增 hash-bound silhouette target、扩展 camera search、`CameraCalibrationRef@1`、Runtime-owned bounded Primary Form/SilhouetteRig/SDF fit、单 Part contour proposal、candidate compare、只读 Viewer 的九 AOV、reference/render split/overlay/flicker、Part/MaterialZone 筛选、爆炸图和热图辅助及 TypeScript/Vite/Tauri 构建，另有 packaged CLI read-model、原生窗口与核心控件 smoke；Agentic projection、逐视图 evidence inventory、promotion transaction 与 durable prepare/readback 另通过 preflight 顺序、空 reference fail closed、合同 checker、Runtime/MCP 重启和 bounded stage batch focused probe。C/D/E/F 结构/传输证据不是用户机器人 likeness；同一 provisional observation 的 packaged Viewer 绑定、正式 VoiceOver、独立人评阈值、xatlas/Validator、真实 PBR likeness、export/restart hash 和 360仍 `NOT_RUN/BLOCKED`。短时 launcher flock 只用于启动选主，Runtime `runtime.writer.lock` 才是最终唯一写者。

### 3.2 FGC-MCP010B — V2 合同与几何真值

兼容界限：`[transition-v1]` `GeometryProgram@1` 继续服务已存在的 MCP007–009 primitive-only appearance/export MVP 路径，且 Runtime 现会对其 GLB 作物理回读；它不是 MCP010B 的 V2 high-quality 写路径，也不得借此获得 V2 catalog、strict `ArtifactReadback@2`、九 AOV strict compare 或材质声明。历史对象不迁移、不改写。V1 新写入口的最终移除须与 MCP010E 的 `AppearanceProgram@2` 迁移一并设计，不能在 B 中让当前已验收的 V1 appearance/restore/export 链静默断裂。

Owned：`GeometryProgram@2`、`OperatorCatalog@1`、`ArtifactReadback@2`、GLB/accessor validator、primitive 修复及负向 fixture。

当前 B source Gate 与 C/D/E/F source Gate 已通过：B 覆盖 V2 geometry/readback/Worker isolation/restore；C 覆盖当前合同 checker、固定 renderer、九 AOV、local mask/metrics、candidate-bound review、MCP image block 和 deterministic raw stdio；D 覆盖 13-entry/13-active catalog、product-owned Manifold C API isolated Worker 的同一 Part bounded union/difference/intersection 和 strict lineage/readback；E 覆盖离线 AssetPack、512px UV atlas、fixed mikktspace、embedded PBR/九 AOV；F 覆盖哈希绑定轮廓目标、37 个覆盖全局尺度的粗候选加 9 个局部探针、扩展 Rig/SDF 搜索、单 Part proposal、2–8 候选比较、方向性边界误差、只读 Viewer 的 AOV/compare/Part/MaterialZone/explosion/heatmap source surface、TypeScript/Vite/Tauri 构建和 write-boundary negative check。Agentic projection、逐视图 evidence inventory 与 durable session/checkpoint/RepairIntent prepare/readback 另有独立合同/重启 Gate；bounded independent stage batch 与 ordered composition proposal 另有 Runtime/MCP focused test。当前 source tool manifest 为 37 read + 28 opt-in write = 65。历史 package/CLI/live receipts保持原样；C/D/E/F/Agentic synthetic/raw/source不等于真实 robot likeness、PBR likeness、同一 candidate 的 packaged Viewer、人评或 360。Darwin OS total-memory hard cap、xatlas、Khronos Validator仍 `NOT_RUN`；授权单图仍只能产生 `PARTIAL_VISIBLE_VIEW_PASS`，HQ_360 仍 `BLOCKED_REFERENCE_COVERAGE`。

必须：

- `GeometryProgram@2` 按 `operator_id` 使用封闭参数 Schema，真实 DAG inputs，米/弧度单位，显式 Part outputs、operator catalog hash 和完整预算；
- Codex 必须先用 `operator_catalog_get` 读取 live `OperatorCatalog@1`，把同一 digest 写入 hash-free draft，再调用 `geometry_program_hash`；返回 hash 填回 program 后才可 `geometry_prepare`。`GeometryProgram@2.project_id` 必须与外层 target project 完全相同：hash/catalog mismatch 由 hash/compile validation 拒绝，project mismatch 由 `geometry_prepare` 在编译和持久化前拒绝；
- V2 物理 envelope 固定为：position 各轴 `[-10, 10]` m，box `size_m` 与 cylinder `height_m` 为 `(0, 10]` m，sphere/cylinder radius 与 ellipsoid radii 为 `(0, 5]` m，rotation 各轴为 `[-2π, 2π]`；
- 修复 sphere 极点退化、cylinder 端盖法线、ellipsoid 法线、UV/tangent 假 PASS；
- Runtime 遍历 GLB BIN/accessor，真实计算 index/non-finite/degenerate/boundary/non-manifold/winding/Part/Material/source coverage；
- 删除 hard-coded validator PASS；损坏 index、source map、hash、winding 或 UV 时 fail closed；
- `[transition-v1]` `@1` 不迁移或改写已确认版本；MCP007–009 的 primitive-only 兼容链只保留历史结构/导出用途，不能产生 V2、detail、九 AOV strict compare 或高质量结论。

预算：512 nodes、250k triangles、64 MiB candidate GLB、512 MiB Worker memory；单次编译目标 10 秒以内。当前 macOS 实证已证明 10 秒墙钟超时/回收、受限 Rust allocator guard 和 `wait4` 峰值 RSS 后验拒绝；但 `RLIMIT_AS`/`RLIMIT_RSS`/`RLIMIT_DATA` 不能提供本机可证明的预防式硬上限，不能把 512 MiB 写成 OS 总内存硬上限；该子门保持 `NOT_RUN`，测量失败不能转移为 MCP011 全局性能实现。

### 3.3 FGC-MCP010C — 固定渲染与参考比较

Owned：`ReferenceViewSpec@1`、`CameraCalibration@1`、`RenderSet@2`、`ReferenceComparisonReport@1`、`VisualReviewReport@1`、`HumanVisualReviewReceipt@1`、`QualityReport@2` 和四个 MCP 工具变化。

Renderer 必须提供 512×512 perspective、真实 camera transform、z-buffer、确定性抗锯齿、固定 GGX 直接光和显式色彩管理；同一 candidate hash 输出：

1. beauty；
2. silhouette；
3. depth；
4. normal；
5. AO；
6. part-ID；
7. material-ID；
8. wireframe；
9. UV-stretch。

目标工具：

| 工具 | 类型 | 行为 |
|---|---|---|
| `render_pass_get` | read | 返回 hash-bound PNG image block；不生成新 render |
| `reference_compare_prepare` | write/temporary | 生成 camera、mask、metrics 和 diff，不创建版本 |
| `visual_review_submit` | write/evidence | 保存 Codex 对具体 pass/region 的 typed issue |
| `human_visual_review_submit` | write/evidence + confirmation | 保存用户评分，不作为密码学身份认证 |
| `quality_get` | existing read | 只读取 Runtime 已持久化且绑定当前 hash 的报告 |

当前源码工具数量为 37 read + 27 opt-in write = 64。MCP010A/010B 的 30/32-tool Dev.app receipts均为历史 structural cohort；C source raw 已证明 `render_pass_get` image block 和三项视觉证据工具，D 当前 packaged raw 已证明同 cohort Operator/strict readback transport，E raw 及当前 packaged E 已证明 `material_pack_get`、embedded texture 和九 AOV render path，F source 已证明轮廓目标、37 个覆盖全局尺度的粗候选加局部探针相机拟合、`CameraCalibrationRef@1`、Rig/SDF/Part/candidate compare 和边界误差读取；Agentic projection、逐视图 evidence inventory 与 durable prepare/readback 已通过合同 checker、preflight 顺序、空 reference fail closed 和 Runtime/MCP 重启 probe，bounded independent stage batch 与 ordered composition proposal 通过 Runtime/MCP focused test。packaged Viewer 已有 read-model/window/core-control smoke，但与 attempt35 provisional observation 的 package binding、正式 VoiceOver、真实用户 likeness/PBR likeness、人评阈值和所有 360° evidence仍 `NOT_RUN/BLOCKED`。

参考 mask 使用产品内确定性 border flood-fill/morphology；Codex 提交 normalized landmarks、region、visibility 和 unknown/inferred。每个 candidate 最多五轮 `silhouette → structure → form → material/surface → final` 修正；未达标返回 `QUALITY_TARGET_NOT_MET`，不能自动 confirm。

区域质量门必须在同一 declared region 内比较 reference/model mask；不得把整个模型 mask 与区域矩形直接做 IoU。Runtime 的 `region-mask-iou-v2` 实现已采用该定义，并排除 `unknown` 区域；它修正指标真值，但不放宽 silhouette、boundary、landmark 或 human gate。

为减少每轮上下文和截图噪声，C/F 允许使用 `scripts/make_mcp010f_comparison_sheet.py` 将同一 reference、beauty、silhouette 和一个 diagnostic AOV 打包为固定 2×2 review sheet。它是标准库 review helper，只保存 hash-only manifest，不计算质量、不写 Runtime/CAS；原图含用户字节时必须留在临时目录。Runtime `QualityReport@2` 与 candidate-bound comparison 仍是唯一质量真值。

F 还提供 `scripts/build_mcp010f_fit_plan.py` 作为本地 Codex 编排辅助器。它验证 `ReferenceComparisonReport@1`、`ReferenceViewSpec@1` 和可选 `OperatorCatalog@1` 的 canonical hash，按五个有序阶段生成最多五条单一 Part/MaterialZone 修正意图，并保留每条意图的 metric、landmark、region 与 observed/inferred/unknown 来源。当前输出还为已知 region 提供稳定的 `primary_part_ids`、只读 `supporting_part_ids`、`material_zone_hints` 和按 Part 分组的 `part_operator_hints`；每轮只保留一个主 Part，未知 region 进入 `unmapped_region_ids`，不会被猜成可执行部件。它不写 Runtime/CAS、不生成几何参数、不调用 Operator、不替代 `QualityReport@2`；缺少活动目录时会清空 `operator_hints` 并明确记录阻断原因。该输出只能留在临时目录或脱敏后作为编排证据。

### 3.4 FGC-MCP010D — 受限高细节几何（source-focused PASS）

Owned：真实 Operator consumer、Worker 隔离/预算、Operator catalog、geometry Skills `0.2.0` 和 Manifold adoption。

目标 Operator：

- `primitive@2`：rounded-box、正确 cylinder/sphere；
- `profile-extrude@1`、`profile-loft@1`、`revolve@1`、`tube-sweep@1`；
- `transform@2`、`mirror@1`、`array@1`；
- `boolean@1`：只允许同一 Part scope 的 union/difference/intersection；
- `panel@1`、`vent-array@1`、`joint-stack@1`；
- `part-output@1`：一个语义 Part 可由多个细节节点组成。

Manifold 固定目标为 v3.5.2，仅 C API 静态进入隔离 geometry worker，关闭 Python/JS binding、自动下载和不受控并行。当前 fixed revision 已完成 source/LICENSE/SBOM、恶意输入/时间/内存/确定性/source-ID 与 removal 评估，并以 product-owned isolated Worker 形式接受；产品能力开放同一 Part scope 的 union/difference/intersection，通用 mesh Boolean 保持 unavailable。

当前实现结果：`operator_catalog_get` 返回 16 项，16 项均 active（primitive@2、15 个 hard-surface/SurfaceProgram Operator）；`boolean@1` 支持同一 Part scope 的 union/difference/intersection，`subd-cage@1` 支持 bounded regular rectangular control-cage subdivision。`script/test_mcp010d.sh` 的 current source/raw suite 覆盖 contracts、source-built Worker/Runtime/MCP、raw stdio `catalog → hash → prepare → readback`、strict lineage、determinism、future-input/unknown-parameter/Boolean negative；SurfaceProgram source receipts 另行证明 open patch、watertight shell 与 editable SubD cage。`hard-surface-detail@0.2.0` 的 manifest、recipe、operator lock、benchmark fixture、LICENSE/NOTICE、SPDX SBOM、provenance 和 development trust 均通过 Runtime integrity 后才返回 `active`。证据：`docs/evidence/mcp010d/manifest.json`、`focused-gates.json`、`raw-stdio-subd-cage-20260815.json`、`manifold-boolean-adoption-gate-20260814.json` 及 MCP010F Surface receipts。

本次 source-focused 退出已包含 Manifold 的 product-owned isolated Worker adoption；但 current packaged D rebuild、Viewer presentation、真实 Codex Desktop D、视觉 likeness、真实 PBR likeness/纹理审美、第三方 Validator 和 360°不由本节推导，均继续记录为 `NOT_RUN/BLOCKED`。

机器人需要稳定 head/visor/neck、chest/core/shoulder、arm/hand、pelvis/hip、thigh/knee/shin/ankle/foot，并包含可追踪 panel、vent、joint ring、cable、emissive housing；左右结构使用 mirror，不维护两套漂移参数。

当前用户单图 source benchmark 的最新几何/表面改进包括四层：panel@1 的固定四段圆角 profile 把面板从平面八点倒角升级为可重复的圆角轮廓；在其上使用 panel/mirror/vent/joint 组成 visor-edge、chest-ridge、shoulder-trim、forearm-rail、hip-flank、knee-cap 六类表面线流层；AppearanceProgram@2 将这些 semantic Parts 绑定到离线 AssetPack MaterialZone，当前保留两个可审计配方：8-zone surface-zones 用全套材质族，7-zone armor-shell-zones 保留可见上臂/前臂白色外壳并把深色限定在内构、凹槽、线缆和发光通道；profile-loft/revolve/tube-sweep/joint-stack 对重合且法线相容的曲面顶点启用受限平滑法线，但保留 panel/cap 锐边。当前 material-zoned linework receipt 为 26 Parts/4704 triangles，silhouette IoU 0.7410、boundary F1 0.3288、region median IoU 0.8694、critical-region minimum 0.6663；相对 rounded-panel baseline 的变化指标全部改善或持平，但仍低于整体视觉门，不能 confirm/export。rounded-panel、linework、surface-zones 和 armor-shell-zones receipts 保留在 `docs/evidence/mcp010f/`，旧 3368-triangle receipt 仍为历史对照；后续几何修正仍必须先做单一 Part 变更再跑同一 candidate 的 comparison，材质区增多不能替代轮廓修正。

### 3.5 FGC-MCP010E — 离线 AssetPack、UV 与 PBR

Owned：`MaterialPackManifest@1`、`MaterialDefinition@1`、`TextureSet@1`、`TextureBuildReceipt@1`、`AppearanceProgram@2`、first-party AssetPack、bounded UV atlas、固定 `mikktspace@0.3.0` tangent producer、glTF Validator deferred adoption 与材质 Skills `0.2.0`。

当前 E source 与 packaged structural 事实：固定 beauty renderer 已读取嵌入 baseColor/normal/metallic-roughness/AO/emissive 纹理，并以固定 key/fill/rim GGX-like 光照、clearcoat 与 emissive strength 参与采样；最新 PBR renderer Dev.app cohort `77d4bff5…f2a73` 已完成 ad-hoc/package/隔离用户参考图探针，但本次未重启 Desktop，且比较结果仍 `QUALITY_TARGET_NOT_MET`。这证明通道接线和离线 provenance，不证明机器人 PBR likeness、色彩审美、人评、export/restart hash 或 360°。最新胸甲浅斜切实验也只作为负向视觉证据：boundary F1/全局 silhouette 小幅改善，但 landmark/region 覆盖下降；单独增加胸甲上缘 cap 仍未改善全局 silhouette，retained baseline 未改变。

`forgecad-hard-surface-robot@1.0.0` 必须是 AssetPack，不是 Skill，包含：白色 dielectric clearcoat、深灰喷涂金属、黑色阳极氧化金属、拉丝钢、工程塑料、关节橡胶、暖橙 emissive 和微划痕 normal/roughness 层。

实施期只允许 Codex 一次性下载以下免费 CC0 文件到本机 adoption cache，不调用 API：

- ambientCG `Metal010` 2K PNG；
- ambientCG `Plastic006` 2K PNG；
- Poly Haven `Studio Small 03` HDRI。

每个原文件记录 source URL/ID、retrieved_at、SHA-256、作者、SPDX、license text hash、通道/色彩空间和处理 Recipe。原 ZIP 不进入 Git；派生 `.forgecad-material-pack` 经 manifest 校验后进入开发 App Resources，Runtime 首次启动写 CAS，运行时永不联网。

UV/tangent/GLB 规则：

- UV atlas 当前使用 ForgeCAD 自有的 512px deterministic triangle-chart grid（每 chart 4 texel padding、无重叠、finite/zero-area 回读）；xatlas 仍只保留 approved-for-evaluation，未进入产品依赖；
- `mikktspace@0.3.0` 固定 crates.io checksum 与 GitHub revision，通过 `docs/evidence/adoption/mikktspace/0.3.0.yaml` 的 license/SBOM/恶意输入/确定性 receipt 后进入受限 Geometry Worker；Runtime 不直接链接该库；
- baseColor/emissive 为 sRGB，normal/metallic/roughness/AO 为 linear，normal 固定 OpenGL `+Y`；
- GLB 内嵌 PNG、禁止 external URI、按材质 hash 去重；candidate texture ≤64 MiB，export ≤128 MiB；
- 支持 ratified `KHR_materials_clearcoat`、`KHR_materials_emissive_strength`；KTX2/LOD/通用 pack installer 延后；
- glTF Validator 是开发 Gate，不能替代 Runtime readback。

升级 `uv-pbr@0.2.0`、`render-evidence@0.2.0`、`reference-compare@0.2.0`；当前 115-contract checker、AssetPack manifest/provenance、Worker/MCP raw Gate 和同 cohort packaged E 结构性用户参考探针已通过。Khronos Validator、真实 PBR likeness、export/restart hash、同一 provisional observation 的 packaged Viewer binding 和 Viewer 人评仍 `NOT_RUN`；未来若第三方 adoption 失败，必须回退到 product-owned strict readback，而不能制造 Validator/mikktspace PASS。

### 3.6 FGC-MCP010F — Viewer 与真实机器人闭环

Owned：reference/render split、overlay、flicker、diff heatmap、九 AOV、camera lock、Part/MaterialZone selection/isolate/explosion、candidate undo/redo、Viewer a11y 和真实 Codex/human evidence。

当前 source slice：`script/test_mcp010f.sh` 已通过 Viewer source checker、TypeScript/Vite build、Tauri workspace compile、read-only IPC/write-boundary negative check；同 cohort Dev.app 的 `--viewer-read-model` 也已在隔离用户参考 candidate 活跃期间返回 `ForgeCADViewerReadModel@1` Ready 投影，artifact/quality/reference 结构映射通过。另有一次隔离 Vite browser DOM smoke 实际点击并验证了 9 个 AOV、3 种比较模式、轮廓画布、差异热图和 flicker；无 Runtime 数据时阶段保持 `reference-canvas` 且 correction queue 为空；同 cohort Dev.app 的 frontmost native-window smoke 观察到 1440×891 的 `ForgeCAD Runtime Viewer`，而本轮 Computer Use 又从打包 WebKit AX 树实际操作 AOV、Home/End、overlay/flicker、轮廓画布、差异热图和爆炸图。System Events 仍未暴露 WebKit 子树，故正式 VoiceOver/无障碍与人评仍保持独立 NOT_RUN。`RuntimeViewer.tsx` 的选择、材质区筛选、临时爆炸图、差异热图、显式 contour canvas、ephemeral reference-contour aid、contour-first 阶段门和 Codex correction queue 都只修改 ephemeral UI state；轮廓画布现在复制 candidate-bound `ForgeCADViewerContourDraft@2`，由 `scripts/validate_mcp010f_contour_draft.py` 生成单 Part `ForgeCADContourCorrectionIntent@1`，并拒绝 stale hash、自交/越界点和未选 Part 的写入意图。轮廓画布只是一键选择既有 silhouette AOV 与 overlay，reference-contour aid 使用与 Runtime `mask-2` 同源的受限边界连通 flood-fill/局部颜色差规则，仍只是视觉辅助，阶段门与 correction queue 仅从 candidate-bound metrics 生成提示，二者都不进入 QualityReport。视觉解锁只读取 candidate-bound `QualityReport@2.visual_status + hard_gate_passed`，不会把结构 candidate 的 `quality_hard_gate_passed` 当成视觉通过。Correction queue 不携带几何参数，只允许下一轮单 Part/单 MaterialZone 的受限意图，且要求保持 camera/reference/hash 不变并重新回读。当前仍未运行正式 VoiceOver accessibility、真人评分、export/restart hash 和五视图 360 门，因此 F 仍为 `in_progress`，不能宣称视觉质量完成。

实际闭环演练：在打包 Viewer 选择 `chest-shell` 并绘制 candidate-bound 轮廓后，Codex 将草图限制为单 Part `chest-wedge-mild`，在隔离 Runtime 中重新执行几何、材质、参考比较和质量读取。该次传输/回读通过，但全局质量门仍失败（silhouette IoU 0.7399、boundary F1 0.3263、landmark coverage 0.6667、region median IoU 0.8487），相对 26-Part/4704-triangle 保留基线退化，未晋级、未 confirm/export。证据 `docs/evidence/mcp010f/contour-execution-actual-20260812.json`；此处证明的是 Viewer→Codex→Runtime 的实际单部件迭代链路，不是视觉相似度通过。

2026-08-13 F 顺序与局部修正证据：每个新的设计 MCP session 先读取并校验 `ponytail-preflight@0.1.0`，再进入 project/reference/operator/geometry 调用；`scripts/probe_mcp010f_part_correction.py` 的静态顺序 Gate 已覆盖该要求。真实隔离探针随后读取 `chest-shell` Part 误差表，执行五个有界单部件候选并完成 candidate-bound comparison；receipt `docs/evidence/mcp010f/part-correction-source-20260813-followup.json` 的最佳 silhouette IoU 为 `0.745895`、Boundary F1 为 `0.330265`，仍为 `QUALITY_TARGET_NOT_MET`。这只是 ordered transport/局部修正证据，不是 likeness、confirm、export 或 packaged Viewer 质量证据。

同轮又把 probe 的受限路由扩展到 `shoulder-armor-left/right`，并以 `shoulder-contour-mild` 的 `shoulder-armor-right` 和用户授权参考右肩 contour 执行五候选比较。Runtime 成功返回局部 Part-error 与 `shoulder-width/height/offset` proposal，但最佳全局 silhouette IoU `0.744471`、Boundary F1 `0.327606` 仍未达到门；receipt `docs/evidence/mcp010f/part-correction-source-20260813-shoulder-right.json` 只证明多 Part attribution/ordered transport，不是 likeness、confirm、export 或 packaged Viewer 质量证据。

随后同一闭环选择 `shoulder-armor-left`，使用图像派生左肩 contour 完成局部 proposal 与五候选 compare；最佳 silhouette IoU `0.742468`、Boundary F1 `0.327530`，未改善肩甲基线，receipt `docs/evidence/mcp010f/part-correction-source-20260813-shoulder-left.json` 只作为 `QUALITY_TARGET_NOT_MET` 的负向单 Part 设计证据保留。不得因此进入材质、确认、导出或 360 门。

Viewer 只有一个 WebGL context，继续只读 Runtime projection。永久 geometry/material/restore/export 仍回到 Codex 的 prepare/approval/confirm。

真实链路固定为：

```text
reference_import → operator_catalog_get → geometry_program_hash(GeometryProgram@2/detail)
→ geometry_prepare → artifact_readback_get(ArtifactReadback@2 strict)
→ reference_mask_prepare → camera_fit_prepare / silhouette_fit_prepare
→ appearance_prepare(AppearanceProgram@2，仅在前序门解锁后)
→ reference_compare_prepare(同一 camera/reference/candidate，九 AOV strict compare)
→ render_pass_get × 9 → visual_review_submit → quality_get
→ 最多五轮单 Part change_prepare / strict compare
→ human_visual_review_submit（独立真人门）
→ candidate_confirm → version_diff
→ restore_prepare/restore_confirm
→ export_prepare/export_confirm
```

输出必须有 self-contained GLB、固定视图、diff、QualityReport、human receipt、immutable version、restore/export receipts，以及 Viewer/export/restart 同 hash 证据。

## 4. 质量门

### 4.1 几何/UV/PBR 硬门

- invalid index、non-finite、超阈值 degenerate triangle 为 0；
- 声明 solid 的 Part boundary/non-manifold edge 为 0；
- triangle 100% 绑定 `part_id + source_node_id + material_zone_id`；
- 同机器重复五次 program/mesh/GLB/report hash 一致；
- MaterialZone binding 完整，无 unused/unknown；
- UV finite、零面积 UV triangle 为 0，padding/density 满足 Recipe；
- tangent orthogonality/handedness/normal convention 通过；
- GLB 无 external URI，Runtime readback 与 Khronos Validator 0 error；
- restart/restore/export 后 pack/material/texture/GLB hash 不变。

### 4.2 当前可见视图门

- silhouette IoU ≥0.90；
- boundary F1（4 px）≥0.90；
- bbox 边缘平均误差 ≤2%，centroid 误差 ≤2%；
- 可见 landmark coverage ≥80%，weighted NME ≤3%；
- region median IoU ≥0.85，critical region 不低于 0.85；
- 用户对 likeness、geometry detail、material fidelity、editability 各评分 ≥4/5。

指标必须记录 reference/camera/mask/render/toolchain hash、阈值、实测值和 limitation。通过这些门只允许 `PARTIAL_VISIBLE_VIEW_PASS`。

## 5. MCP011–013 保留边界

- MCP011：持久 Job checkpoint、复杂并发/cancel race、kill-9、GC/reachability、全局配额和性能；
- MCP012：通用第三方 Skill/AssetPack publisher、安装/禁用/升级/回滚、签名和撤销；
- MCP013：Developer ID、hardened runtime、notarization、clean install、正式 Codex 配置、packaged Desktop/CLI E2E、filesystem/package export、升级失败回滚和跨类别真人质量。

MCP010 的单操作预算、first-party 固定 AssetPack、ad-hoc 开发 App 和当前机器人用户评分不得替代上述任务。

## 6. 每任务验证与证据

每个子任务先记录 dirty baseline，再按 `Schema/negative → Core/Worker → Runtime/MCP/Viewer → focused → aggregate → real Codex/visual/human` 顺序运行适用 Gate。共同命令：

```bash
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run release:license-sbom
npm run contracts:check
git diff --check
```

Evidence 目录为 `docs/evidence/mcp010a/` 至 `mcp010f/`。每个 manifest 分别记录 PASS、FAIL、BLOCKED、NOT_RUN、命令 exit、contract/build/dependency/artifact hash 和脱敏检查；不得修改 MCP005–009 原始 receipt。

## 7. 禁止项

- 不内置模型、Provider、付费 API、远程 image-to-3D 或素材 API；
- 不安装 BlenderMCP、FreeCAD MCP、任意 Python/JavaScript/shader 插件或 GitHub Skill pack；
- 不让 `.blend`、Three.js scene、外部 validator、截图或自然语言成为产品真值；
- 不在 010A 提前增加当前 Schema/tool/Skill 数量；
- 不在缺少多视图时宣称 360，不在单个机器人通过后宣称通用高质量；
- 不 commit、merge 或 push，除非用户另行明确要求。
