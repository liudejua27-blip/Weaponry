# ForgeCAD 当前原子任务索引

2026-08-15 F staged transport 增量已收口：`design_stage_run_prepare` 的 action entry 支持 `view_spec`，并将它纳入父批次和子 ActionRun 的 hash binding；真实授权参考回执为 `docs/evidence/mcp010f/design-action-run-real-reference-stage-view-spec-20260815-b37.json`。结果为 `PASS_ACTION_RUN_RUNTIME_AUTO_PARAMETER_PATCH_STAGE_BATCH`，子 ActionRun 完成到 evaluate 后因 `QUALITY_TARGET_NOT_MET` 阻断，exact replay/source immutability 通过，confirm/version/持久数据仍为 0。唯一 `in_progress` 仍是 `FGC-MCP010F`；完整 Critic→Repair、Repair application/promotion、组合式多动作 orchestrator、真实 likeness/PBR/human/360 仍未完成。

2026-08-15 高层 ActionRun 自动补丁原子增量：`design_action_run_prepare` 现在可接收 typed `action.parameter_changes` 与外层 `ReferenceViewSpec`，Runtime 自动物化有界 `RuntimeParameterPatch@1 / surface-control-points-v1`、`RepairIntent` 和 review candidate，并完成 `prepare → compile → readback → render → evaluate`。真实回执 `docs/evidence/mcp010f/design-action-run-real-reference-runtime-auto-parameter-patch-20260815.json` 的提案因 regression 被拒绝，source candidate/version/confirm/持久数据不变，整体仍 `QUALITY_TARGET_NOT_MET`。这完成调用方无需拼装完整 proposal 的 transport 缺口，不改变唯一 `in_progress=FGC-MCP010F`，也不宣称完整 Agentic Runtime 或 Repair 已完成。

2026-08-15 当前原子任务增量：`OptimizationJob` 已把 `SilhouetteRig@1` 的 `surface_control_point` 参数纳入 Runtime-owned grouped search。真实 v15 回执 `docs/evidence/mcp010f/optimization-job-real-reference-surface-control-groups-v15.json` 完成 39 次 `32/4/3` 评估，证明多控制点 candidate 物化、同 camera、checkpoint/readback 与同一 Part Boolean residual lane；baseline/best loss 仅小幅改善，Part strict objective 失败，`proposal_status=blocked-no-improvement`，source candidate/version/confirm/export 均不变。该原子任务已完成其 bounded search/证据收口，但不改变唯一 `in_progress=FGC-MCP010F` 或整体 `QUALITY_TARGET_NOT_MET`。

2026-08-15 当前真实参考闭环收口：`docs/evidence/mcp010f/real-reference-quality-closure-20260815.json` 是最新 additive index。统一 cohort `61e01276…bf340a` 上，CADFit `OptimizationJob` 与 ActionRun child 均完成 39 次 `32/4/3` 评估，Manifold Boolean residual 使用候选 `[1..9]` 的 bounded same-Part lane；相机在 Runtime durable RenderSet/CAS 中回读并通过 canonical normalization，优化/比较 camera hash 一致。结果仍为 `QUALITY_TARGET_NOT_MET`、strict improvement=false、proposal=`blocked-no-improvement`，无 confirm/version/export；human review/export-restart 为 `NOT_RUN`，HQ_360=`BLOCKED_REFERENCE_COVERAGE`。唯一 `in_progress` 仍为 `FGC-MCP010F`，该证据不升级 Stage 0 或历史 ledger 的视觉质量事实。
2026-08-15 当前源码 revalidation：统一 cohort `613470b6…af04a` 的 MCP/Runtime/Geometry Worker/Render Worker 已重新跑同一授权参考；v12 OptimizationJob 与 v13 ActionRun→CADFit child 均完成 39 次 `32/4/3` 评估，camera/Manifold residual/readback 绑定一致，结果仍 `QUALITY_TARGET_NOT_MET`、strict improvement=false、proposal blocked，未 confirm/version/export。该增量已完成当前源码回归验证，但不改变唯一 `in_progress`、Stage 0 `BLOCKED_INCOMPLETE_BINDING`、真实人评/PBR/export/restart/360 的未运行或阻断状态。

2026-08-15 Runtime-owned parameter patch 原子增量：`design_action_run_prepare` 现支持 `RuntimeParameterPatch@1` 的 `primitive-dimensions-v1` 与 `surface-control-points-v1`。Runtime 在单 Part `primitive@2`/`panel@1` 或唯一 `subd-cage@1`/`surface-patch@1`/`surface-shell@1` 节点上校验参数语义、单位、范围、stale-before、唯一节点、operator 和 candidate/reference/camera lineage，生成自身 `RepairIntent@1`，并完成独立 review candidate 的 `prepare → compile → readback → render → evaluate`；surface-shell 正向与控制点越界负向回归 receipt 为 `docs/evidence/mcp010f/runtime-parameter-patch-surface-control-points-20260815.json`。随后修复 Runtime-owned 自动相机的 round-trip canonicalization，并在 `RenderSet@2` 保存/回读 `camera_object_sha256`；诊断 UI 截图探针现可完整到达 evaluate，但真实质量仍 `QUALITY_TARGET_NOT_MET`。无 confirm/version/export，唯一 `in_progress` 仍为 `FGC-MCP010F`。
2026-08-15 Surface-backed chest-shell 原子增量：`docs/evidence/mcp010f/design-action-run-real-reference-surface-control-points-v14.json` 在当前源码 cohort `613470b6…af04a` 上用同一授权参考/camera 真实执行 `surface-shell@1`、16 控制点与 `RuntimeParameterPatch@1 / surface-control-points-v1`；`control-point-5-z` `0.12m→0.20m` 后五阶段到 evaluate，source/proposal IoU `0.744889/0.745584`、Boundary F1 `0.326291/0.328475`，但统一 score `5.681233147408→5.688397884520`，`strict_improvement=false`、`non_regressing=false`、promotion=`rejected-regression`。该原子任务完成真实 surface parameter transport/readback/same-camera Gate，但不完成视觉晋级；candidate 未 confirm、version count 0、整体仍 `QUALITY_TARGET_NOT_MET`。下一步是基于 SurfaceProgram 的多控制点 bounded search/Part objective，而非确认该单点提案。

2026-08-15 参考覆盖 ActionRun 原子增量：`request-reference` 已补齐 Runtime fail-closed executor，并与 `DesignSession.next_actions` 对齐为 `scope=reference,target=authorized reference_id`。单元与 ActionRun 集成回归证明它在 `prepare` 阶段直接返回 `BLOCKED_REFERENCE_COVERAGE`，不编译、不渲染、不创建 checkpoint/OptimizationJob，且 exact replay 稳定；这只完成 staged Agent pipeline 的“缺参考即停”分支，不改变真实机器人 `QUALITY_TARGET_NOT_MET`、360 `BLOCKED_REFERENCE_COVERAGE` 或唯一 `FGC-MCP010F in_progress`。

2026-08-15 多视图 promotion 原子增量：完整 coverage 的 `ReferenceCanvas@1` 现在要求每个 supplied view kind 都有对应 authored view entity；`docs/evidence/mcp010f/cross-view-promotion-positive-synthetic-20260815.json` 的六视图 synthetic fixture 已通过逐视图 compare、aggregate strict-improvement/non-regression、approval-gated `cross_view_promotion_confirm`、immutable version 与 exact replay。它只证明 Runtime/Store transaction 和 hash lineage，不改变真实参考 `QUALITY_TARGET_NOT_MET`、人评/360 未运行或唯一 `FGC-MCP010F in_progress`。

2026-08-15 img2threejs 受控学习已转为 ForgeCAD 自有 staged pipeline 计划：Visual Surface typed readback contract/negative Gate、`surface-patch@1` open-surface source Gate、`surface-shell@1` bounded watertight-shell source Gate、`subd-cage@1` bounded editable regular-quad source Gate、同一 candidate GLB 的 bounded mesh-derived curvature/feature-line analysis Gate，以及 `surface_signal_canonical_sha256` 进入 Critic/PartError/CADFit 的显式绑定 Gate 已完成；真实授权参考同 cohort 回执 `docs/evidence/mcp010f/optimization-job-real-reference-20260815-surface-signal-cohort-v4-camera-rebound.json` 已证明 39 次 CADFit、多保真、Manifold Boolean residual 与 Surface Signal hash lineage 的 proposal-only transport，并已将 comparison camera rebind 到 optimization camera。比较状态仍为 `QUALITY_TARGET_NOT_MET`（IoU `0.529445`、Boundary F1 `0.090842`），不能晋升为 likeness benchmark；同 cohort DesignActionRun v4 也已完成 fixed-camera proposal-only continuation。本轮新增 v9 bounded `chest-shell` correction 已在同一 `camera_fit_prepare` 相机下完成 5 候选比较，winner 的 IoU/F1/critical min IoU 为 `0.533249/0.079165/0.614734`，但仍 `QUALITY_TARGET_NOT_MET`，因此只记录为 proposal/transport evidence。完整 coverage synthetic promotion fixture 已通过；下一项是把该 promotion/Repair 链带到真实授权参考并独立通过视觉/人评门，不安装 upstream Skill，不把 Three.js factory 作为 Runtime 真值，当前唯一 `in_progress` 仍为 `FGC-MCP010F`。

2026-08-15 Surface Signal real cohort slice：MCP/Runtime/Geometry Worker/Render Worker 四者均绑定 cohort `889f054a706360eb0060f040f338be08f1a81379d91ca78e0a086c8098de9d2e`，39 次评估为 `32/4/3`，`0.612457433696 → 0.610821480247`，`strict_improvement=true`、`proposal_status=proposed`；`surface_signal_canonical_sha256` 与 `OptimizationResidual@1.source_visual_surface_sha256` 同为 `a4f868760eec33bd3204c3ca57e7ecf35b9a1381181d78f3c5616d22742f51e1`，optimization/comparison camera 同为 `3ed0c20c…87859`。全局 silhouette IoU `0.529445`、Boundary F1 `0.090842`，仍没有 confirm/version/export；该证据只覆盖同 cohort、same-camera Surface Signal→Critic/PartError→CADFit/Boolean transport，不覆盖 likeness/high-quality。

2026-08-15 同 cohort DesignActionRun continuation：`docs/evidence/mcp010f/design-action-run-real-reference-20260815-surface-signal-cohort-v4-camera-bound.json` 已证明四进程统一 cohort 下，先 pre-fit camera 再执行 `prepare → compile → readback → render → quality stop`；直接 Repair 在 `repair-proposal-failed` 阻断，嵌套 CADFit 完成 `32/4/3` 共 39 次评估，`0.539958348892 → 0.539031653854`，strict proposal 物化独立 review candidate，但 `visual_gate_passed=false`、`confirm_allowed=false`、`version_count=0`。父级与嵌套 CADFit comparison/optimization camera 同为 `085c199a…525f1`；当前下一步转为轮廓/PartError correction，不再扩大 Boolean 搜索。

2026-08-15 CADFit continuation 已完成当前 cohort 验证：真实 Codex probe 支持单 Part 6 参数 Job；Runtime queued-return、MCP continuous-number canonical rebind 及 nested camera/Rig hash rebind 均已重建验证。当前 cohort 为 `675df14b5e24c02a4dbf463098894ac59a612b00d4e7c6436fc3e64b80b0035f`，Runtime optimization 10/10、MCP 61/61；真实参考 receipt `docs/evidence/mcp010f/real-codex-cadfit-real-reference-20260815-rebuilt-rig-rebind.json` 完成 39 evaluation checkpoint，strict improvement/proposal 为 true，但视觉仍 `QUALITY_TARGET_NOT_MET`，没有 confirm/version/export。AssetPack smoke 因不是用户参考且无严格改善保持 BLOCKED，不计为视觉 PASS。

2026-08-15 F 局部候选排序修正：`silhouette_candidate_compare` 统一使用 Boundary F1 → Silhouette IoU → bbox → centroid → SDF 的可见轮廓优先级，composite loss 只作 tie-break；同 cohort（`b37f7116fdab4ec6ea1a57b75bbb922addf3d5ef2d32c51562d5e12aa7cdfe2f`）真实回执 `docs/evidence/mcp010f/part-correction-real-reference-20260815-b37-priority-cohort.json` 验证 5 候选真实往返。该项不改变 `QUALITY_TARGET_NOT_MET`、无 confirm/version/export 的事实。

2026-08-15 MCP010F 局部诊断回归：`docs/evidence/mcp010f/optimization-job-real-reference-20260815-critic-part-bool-b37.json` 已验证 `critic_report_get` 的显式目标哈希、Runtime PartError/scoped intent、Manifold residual 和 32/4/3 CADFit proposal/readback 同一绑定；`strict_improvement=true` 仍只代表优化提案门通过，视觉保持 `QUALITY_TARGET_NOT_MET`，没有 confirm/version/高质量声明。

2026-08-15 Agentic stage batch real stdio Gate：`docs/evidence/mcp010f/design-action-run-orchestrator-real-reference-20260815-b37.json` 已通过真实授权参考执行 `design_stage_run_prepare`；首个 ActionRun 在质量门阻断，batch checkpoint/replay/source immutability 通过。该事实只覆盖真实 dispatch/stop/replay，不把完整 orchestrator、Repair、视觉 likeness 或 360 标为完成。

2026-08-15 当前 CADFit/Boolean 真实闭环：`docs/evidence/mcp010f/design-action-run-cadfit-real-reference-20260815-v4-diverse-fixed4-b37.json` 已完成 39 次多保真 ActionRun-bound 搜索并在 render/Repair 质量门阻断；顶层 `proposal=null` 表示直接 Repair 没有 proposal，但嵌套 CADFit child 已经通过显式 continuation 物化独立 review candidate。该候选仍为 `QUALITY_TARGET_NOT_MET`，没有 confirm/version。`docs/evidence/mcp010f/optimization-job-real-reference-20260815-boolean-residual-v20-fixed.json` 已完成同一 Part 的 `[1..9]` Manifold Boolean residual 搜索，strict gate 为 proposed，但原始回执没有 build cohort 字段，因此不与当前 source cohort 合并。该增量只推进 FGC-MCP010F 的 bounded CADFit/Boolean 证据，不改变唯一 `in_progress`、Stage 0 `BLOCKED_INCOMPLETE_BINDING` 或通用 mesh Boolean 未完成事实。

2026-08-15 CADFit/Manifold v19c 局部绑定修正：`SilhouetteTarget@1.part.region` 现在允许 Runtime 消费由参考图像标注的归一化 ROI；`target_part_region_mask` 将 Runtime-owned 全身 mask 裁剪到该 ROI，CADFit `part_region`、Part contour error 和 Part fit envelope 优先使用它，缺少 ROI 时才回退到声明的 contour slice。真实回执 `docs/evidence/mcp010f/optimization-job-real-reference-20260815-boolean-residual-v19c.json` 在同一授权参考上通过 32/4/2 共 38 次评估，`part_target_binding=PASS_IMAGE_DERIVED_REGION_BOUNDED_REFERENCE_MASK`，`chest-armor` ROI 为 `x=0.292,y=0.285,width=0.435,height=0.285`；candidate 0 无 Boolean，候选 `[1..9]` 进入 product-owned Manifold Boolean lane，baseline/best loss `0.472832854888/0.470494645212`，但 `strict_improvement=false`、`proposal_status=blocked-no-improvement`。ROI 是显式图像区域，不是语义分割或 likeness 通过；没有 confirm/version/持久用户数据写入，仍 `NO_LIKENESS_PASS_CLAIM`、`QUALITY_TARGET_NOT_MET`。

2026-08-15 FGC-MCP010F CADFit residual family v3：真实参考 v18 已验证 candidate-0 unmodified baseline、9-slot `OptimizationResidual@1` family、product-owned Manifold bounded Boolean 搜索和 32/4/2 CADFit 评估；Boolean lane/readback PASS 但 winner 为无 Boolean shape-only candidate，严格多目标仍 `blocked-no-improvement`，无 proposal/confirm/version。该事实只升级 bounded search/readback，不升级 likeness 或通用 mesh Boolean。

2026-08-15 最新源代码覆盖：当前为 123 Schema、40 read + 30 opt-in write = 70 tools。新增只读 `visual_surface_get`、`silhouette_evaluation_objective_prepare`、`silhouette_objective_compare` 及对应 objective contracts；`design_action_optimization_proposal_prepare` 只将严格且 non-regressing 的 ActionRun-bound CADFit 结果物化为独立 review candidate，要求显式 ViewSpec，不自动 Repair/Confirm。首个 `forgecad.geometry.surface-patch@1` open-surface Worker source Gate 已通过（4×4 Bézier cage、128-triangle fixture、strict readback、negative segment Gate）；它仍不是完整 Visual Surface backend。它要求 fresh approval、精确 source/run/proposal/RepairIntent/artifact/visual lineage 和幂等确认后才进入 immutable-version Store path，多视图仍必须使用 `cross_view_promotion_confirm`。合同/negative gate、Stage 0、`cargo check` 与 focused tests 已通过，唯一 `in_progress` 仍为 `FGC-MCP010F`；surface quality backend 与视觉质量仍未通过。

版本：2026-08-15
状态：唯一任务状态表；MVP host golden path 与 FGC-MCP010A 已收口；FGC-MCP010B 结构实现已通过、Darwin OS 总内存硬门 deferred；FGC-MCP010C source-focused 已完成；FGC-MCP010D/E source-focused 已通过；FGC-MCP010F source-focused in_progress，packaged/人评/360 子门保留。ADR-0026 的 Agentic Design Runtime 已完成 observe/plan projection、嵌套只读 projection conformance、durable session/checkpoint/RepairIntent prepare/readback、显式 bounded authoring_context producer/readback、逐视图 evidence inventory、bounded cross-view evidence bundle、primary-form/secondary-structure/tertiary-detail single-Part geometry proposal slice、有界同阶段独立动作 batch，以及带父程序哈希链校验的 cumulative-program composition merge prepare；完整 coverage synthetic cross-view promotion/replay fixture 也已通过，但真实参考上的完整 orchestrator、Repair 应用、候选持久变更和视觉门仍未完成，唯一任务状态不变。

Stage 0 机器真值入口为 `docs/evidence/mcp010f/current-benchmark-truth.json`。当前为 123 Schema、40 read + 30 opt-in write = 70 tools，唯一 `in_progress` 为 `FGC-MCP010F`。attempt35 仅是 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING` 的 provisional retained observation，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，camera 绑定 `MISMATCH`；最新 real-Codex receipt 已完成当前 cohort 的 Viewer exact lineage binding，但不改变 attempt35 的 provisional 状态。新增跨视图 evidence bundle、`design_composition_prepare` 的独立动作与 cumulative-program merge-chain 校验、`repair_apply_prepare` 的 CAS-backed apply-intent boundary、`design_action_optimization_proposal_prepare` 的 CADFit review-candidate continuation、`repair_apply_confirm` fixture/runtime transaction、统一 `SilhouetteEvaluationObjective@1`，以及 `cross_view_promotion_confirm` 的 negative boundary 与 complete-coverage synthetic promotion/replay fixture，均只改变各自 source/readback/transaction slice，未改变视觉质量事实。

<!-- forgecad-stage0: schemas=123 schema_set_sha256=583fd0d2615f09e66d16c58fca8d4ab60f1856d1de427b5b9e390c8c8b137f67 read_tools=40 write_tools=30 total_tools=70 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260815-b37-complete-auto-v3.json latest_completed=real-codex-cli-current-20260815-b37-complete-auto-v3.json -->

2026-08-15 最新真实 Codex transport（未晋升 provisional benchmark）：`docs/evidence/mcp010f/real-codex-cli-current-20260815-b37-complete-auto-v3.json` 使用当前源码构建 cohort `b37f7116fdab4ec6ea1a57b75bbb922addf3d5ef2d32c51562d5e12aa7cdfe2f`，11 个 Codex turn 全部 exit code 0；26 parts/4704 triangles 的 artifact/readback 通过，silhouette-fit→reference-compare 相机绑定为 `PASS_SILHOUETTE_FIT_TO_COMPARE`，9 个 AOV 的 typed image block/readback 与 hash/dimension 记录通过。比较结果 IoU `0.529998`、Boundary F1 `0.073554`、bbox `0.203125`、centroid `0.031885`、landmark coverage `0`、region median `0`、critical min `1.0`，仍为 `QUALITY_TARGET_NOT_MET`、typed review `needs_revision`；脱敏 CLI 事件仅归类为只读 Skill 读取，未触碰用户持久数据。该 receipt 证明当前 cohort 的 transport 绑定，不改写 attempt35 的 `MISMATCH + BLOCKED_INCOMPLETE_BINDING` provisional 事实，也不证明 likeness/high-quality；人评、PBR likeness、confirm/export、restart hash、packaged accessibility 和 360 仍未运行或受阻。

2026-08-14 同 cohort 真实复跑已验证 Primary Form 合同修复：Runtime 将 authored baseline 的完整 Rig definition 在 `SilhouetteFitResult@1` 输出边界压缩为 `parameter_id/part_id/value`，focused test 与 `script/test_mcp010c.sh`、`script/test_mcp010f.sh` 均通过。receipt `docs/evidence/mcp010f/real-codex-cli-current-20260814-same-cohort.json` 的 44 个 typed MCP 调用完成，`scene_observe_get` 为单次聚合上下文，fit `evaluations=16`；fit/compare camera hash 与 canonical hash 一致，但 IoU `0.690952`、Boundary F1 `0.256758` 仍为 `QUALITY_TARGET_NOT_MET`。一个 Codex turn 返回码为 1，探针因此保守记为 `BLOCKED`，不把该 receipt 晋升为 benchmark 或高质量 PASS。

2026-08-14 Primary Form 相机绑定加固：Runtime 仍以 `128×128` 做有界粗搜，但对排序后的最多五个候选加 authored base camera 通过同一个隔离 Render Worker 批量执行固定 `512×512` 验证，再用完整内部损失选择结果；对外 `CameraFitResult@1` 仍只返回合同规定的四项 silhouette metrics，避免低分辨率 aliasing 直接污染 compare camera。`script/test_mcp010c.sh`、`script/test_mcp010f.sh`、Runtime focused tests 和四个最新同 cohort build identity 均通过（cohort `d11e83cc…e07264`）。此前两次停在 `reference_get` 的重试保留为历史阻断记录。

2026-08-14 当前真实 Codex 复跑：`docs/evidence/mcp010f/real-codex-cli-current-20260814-viewer-bound.json` 在同一 cohort 完成 `project_create → reference_import → reference_get`、单次 `scene_observe_get`、Primary Form fit、compare、九 AOV、typed review、quality readback，并验证 packaged Viewer exact project/candidate/artifact/reference/RenderSet/comparison binding。11 个 Codex turn 全部退出码 0，fit→compare camera binding 为 `PASS_SILHOUETTE_FIT_TO_COMPARE`；compare IoU `0.688698`、Boundary F1 `0.248825`、bbox `0.035156`、centroid `0.042175`、landmark coverage `0.517270`、NME `0.153089`、region median `0.759490`、critical min `0.389423`，仍为 `QUALITY_TARGET_NOT_MET`。该 receipt 已进入 inventory/Stage 0 latest pointer，但未晋升 provisional benchmark；脱敏 CLI 事件仍未观察到 image-block consumption，PBR、人评、confirm/export、restart hash 和 360 继续未运行/阻断。

## 1. 状态规则

状态只允许 `ready | in_progress | blocked | done | superseded`。同一时刻最多一个 `in_progress`；用户启动 Goal 后，Luna 才把唯一 `ready` 项改为 `in_progress`。依赖未完成时保持 `blocked`。

历史 evidence 的状态描述当时验收范围；任务索引描述当前权威范围。改变范围时不得改写原始 receipt，只能说明“现阶段退出条件”和仍留给后续任务的 Gate。

FGC-MCP010F 最新增量：隔离 Vite browser DOM smoke 已验证 Viewer 的 9 个 AOV、3 种比较模式、轮廓画布、差异热图/flicker 控件，以及无 candidate-bound metrics 时保持 `reference-canvas` 且 correction queue 为空；Runtime 另已收紧 `SilhouetteTarget.parts` 的唯一非重叠 contour slice 和 Part-ID 局部边界归因；同 cohort Dev.app 又完成 frontmost native-window smoke，但 macOS System Events 未暴露 WebKit 子树，因此这不改变 packaged Tauri UI/accessibility 仍 `NOT_RUN` 的状态。

2026-08-13 F 增量：单部件修正探针现在在 `project_create`、参考导入、Operator/Geometry 或其他设计调用前读取并校验 `ponytail-preflight@0.1.0`；顺序回归已加入 `script/test_mcp010f.sh`。隔离探针根据真实 `chest-shell` Part 误差表执行五个有界候选比较，receipt 为 `docs/evidence/mcp010f/part-correction-source-20260813-followup.json`；最高 silhouette IoU `0.745895`、Boundary F1 `0.330265`，Runtime loss winner 的 IoU `0.745135`，全部 `QUALITY_TARGET_NOT_MET`。该结果只证明 preflight 顺序和局部候选 transport，未 confirm/export、未写入用户持久数据，不能改变 F 的 `in_progress`、camera `MISMATCH` 或 `BLOCKED_INCOMPLETE_BINDING`。

同轮继续推进了多 Part 路径：探针新增 `shoulder-armor-left/right` 的 bounded patch route，并使用 `shoulder-contour-mild` 的 `shoulder-armor-right` sink 完成 Part-error、proposal 和五候选比较。receipt `docs/evidence/mcp010f/part-correction-source-20260813-shoulder-right.json` 的最佳 silhouette IoU `0.744471`、Boundary F1 `0.327606`，仍为 `QUALITY_TARGET_NOT_MET`，未 confirm/export。它只证明单肩 Part 的 Runtime 归因和 transport 已可执行，不改变 current baseline 或 F 的 `in_progress`。

随后同一探针选择 `shoulder-armor-left` 并使用图像派生左肩 contour 完成五候选比较。receipt `docs/evidence/mcp010f/part-correction-source-20260813-shoulder-left.json` 的最佳 silhouette IoU `0.742468`、Boundary F1 `0.327530`，未改善肩甲基线，仍为 `QUALITY_TARGET_NOT_MET`；无 confirm/export、无持久用户数据写入。该结果只扩展了左右肩 Part 的可归因 transport，不改变 F 的 `in_progress`。

2026-08-13 模块化修复增量：`apps/render-worker` 已落地为隔离一次性 JSONL Render Worker，Runtime render 路径不再在主进程内直接执行；`silhouette_fit_prepare` 现在由 Runtime 持有按证据排序的 bounded Primary Form 坐标搜索，Codex 只提交 typed proposal；Viewer 删除本地阈值/工作流推导，改为消费 Runtime Agentic projection 与 candidate-bound QualityReport。`script/test_mcp010c.sh`、`script/test_mcp010f.sh` 和真实 C raw stdio 均通过；这些是模块/结构证据，仍不改变 F 的 `QUALITY_TARGET_NOT_MET`、`MISMATCH`、packaged same-observation `NOT_RUN`、人评/VoiceOver/360 未运行。

2026-08-14 F 回归修复：Primary Form 保留 authored baseline，只有真实 Worker probe 严格改善时才接受 Runtime proposal；每个 camera-fit probe 的 silhouette/Part-ID transient evidence 直接随 winner 复用，避免 selected camera 产生未计入预算的重复渲染。Runtime focused test 进一步证明同一 fresh Runtime 中，`silhouette_fit_prepare` 的 winner `CameraCalibrationRef@1` 交给 `reference_compare_prepare` 后，`camera_hash` 与 `canonical_sha256` 完全一致。该修复只证明新路径的有界收敛保底和相机 handoff，不晋升 attempt35 的旧 `MISMATCH`、`QUALITY_TARGET_NOT_MET` 或 `BLOCKED_INCOMPLETE_BINDING` 事实。

同轮将 canonical observation 接入 silhouette-first Codex source 编排：每个视觉回合先读取一次同一 candidate-bound `scene_observe_get`，再进入 camera/Rig；脚本校验 `AgenticSceneObserveResult@1` 的 project/candidate/read-only/canonical hash。source check 已通过，但新 observation sequence 尚未用新的授权机器人参考重跑，不改变旧 receipt 的质量和完整性状态。

ADR-0026 重规划规则：`scene_observe_get`、`design_stage_plan_get`、`critic_report_get`、`visual_evidence_bundle_get` 标为 `source/read-only projection PASS`；真实 Runtime 的 scene/stage 嵌套只读 projection 另标为 `nested projection conformance PASS`；`session_create_or_resume`、`session_get`、`checkpoint_prepare`、`checkpoint_get`、`checkpoint_restore_prepare` 另标为 `durable prepare/readback PASS`；`design_stage_run_prepare` 标为 `bounded independent stage batch PASS`，其 RuntimeJob/event stop gate 和 exact replay 已通过 focused Runtime/MCP test。`design_composition_prepare` 另标为 `ordered composition proposal + cumulative merge-chain PASS_WITH_POSITIVE_DETERMINISTIC_FIXTURE_AND_FAIL_CLOSED_PROMOTION`：负向 fixture 在首个质量门停止，正向 fixture 已由两步 reviewable ActionRun 编译独立 merge candidate，且 source candidate 未变、回放一致、confirm 仍锁定；真实参考上的完整 Plan→Act→Inspect→Evaluate→Checkpoint orchestrator、Repair 应用、用户批准后的 candidate/version mutation 和完整 Visual Evidence conformance 仍未完成。`CrossViewEvidenceBundle@1` boundary 现为 bounded cross-view evidence PASS，完整 coverage synthetic fixture 已进一步证明专用 `cross_view_promotion_confirm` 的 immutable-version/replay 边界；真实参考仍必须独立完成 promotion/Repair/人评。证据为 `docs/evidence/mcp010f/agentic-composition-proposal-20260814.json`、`docs/evidence/mcp010f/agentic-runtime-observe-plan-20260813.json`、`docs/evidence/mcp010f/agentic-runtime-projection-conformance-20260813.json`、`docs/evidence/mcp010f/agentic-runtime-session-checkpoint-20260813.json`、`docs/evidence/mcp010f/cross-view-evidence-boundary-20260814.json` 和 `docs/evidence/mcp010f/cross-view-promotion-positive-synthetic-20260815.json`，校验器分别为 `scripts/check_agentic_projection_receipt.py` 与 `scripts/check_agentic_runtime_receipt.py`。显式 authoring_context、single-Part geometry-stage proposal、逐视图 evidence inventory 与 CADFit checkpoint/resume 另有独立 evidence；不得把 fixture-only promotion 当作真实参考视觉质量或完整 Agentic Runtime/done。

## 2. 当前任务链

| Task ID | 状态 | 依赖 | 当前原子结果 |
|---|---|---|---|
| FGC-MCP000 | done | 无 | ADR-0025、权威链、重置和执行任务链 |
| FGC-MCP001 | done | MCP000 | 可恢复硬切；新 Viewer/Runtime/contracts 骨架 |
| FGC-MCP002 | done | MCP001 | SQLite V1/CAS、Runtime OS 文件锁单写者、authenticated IPC |
| FGC-MCP003 | done | MCP002 | MCP stdio/resources/只读 tools；Codex Desktop/CLI P0 只读宿主证据 |
| FGC-MCP004 | done | MCP003 | 单用户事务基座：candidate/Job/approval/confirm/reject/restore/diagnostic export、MCP 内置 Runtime supervisor、一次有界 restart、Codex CLI diagnostic write、Viewer read model |
| FGC-MCP005 | done | MCP004 | 真实 Codex PNG/JPEG 附件字节 → CAS → `ReferenceEvidence@1`；CLI E2E PASS |
| FGC-MCP006 | done | MCP005 | MVP typed design/geometry/appearance 合同 + 10 个 first-party 声明式 Skill Bundle |
| FGC-MCP007 | done | MCP006 | 有界多 Part 硬表面机器人几何、Assembly/Part/source-map、GLB readback、Viewer read model |
| FGC-MCP008 | done | MCP007 | bounded UV/tangent/PBR、固定 beauty/silhouette/normal/part-ID、真实 GLB Viewer |
| FGC-MCP009 | done | MCP008 | limited quality projection、stable-Part `change_prepare`、拒绝/批准/版本/restore、CAS-backed MVP GLB export；功能核心收口 |
| FGC-MCP010A | done | MCP009 | 权威重排、可恢复旧代码清理、同 revision 用户级开发 App 激活、真实 Codex capability/build-hash Gate；Desktop attempt 1 FAIL 保留，attempt 2 PASS（30 工具、Ready、cohort match、临时项目 readback） |
| FGC-MCP010B | blocked | MCP010A | V2 geometry/readback/Worker isolation source Gate 已通过；Darwin 512 MiB OS 总内存硬门 deferred 为 NOT_RUN，不阻塞当前 C source implementation；历史 package/live receipts 保留 |

最新 d9c23b…ac0bd Skill-overlay Dev.app 已完成 package/raw/real-Codex V2 structural 子门；Bundle 知识现在明确区分视觉质量停止与用户明确批准的 `STRUCTURAL_BLOCKOUT` MVP 路径。该包已完成用户完整 Desktop restart，并通过 live Desktop structural activation；仍不得把它写成视觉/PBR能力。
| FGC-MCP010C | done | MCP010B | source-focused：固定 512×512 perspective/z-buffer renderer、九 AOV、candidate-bound reference comparison、Codex/human review 与 MCP image block；默认 camera auto-fit、视觉指标 CAS round-trip 与同一 candidate 五次 MCP determinism 已通过 source regression；Viewer compare source implementation/local IPC-build tests PASS；真实 Codex CLI 已完成六 turn/32-call C transport；机器人 likeness threshold 仍 FAIL_QUALITY_TARGET_NOT_MET，packaged/人评/360 子门另行保留 |
| FGC-MCP010D | done | MCP010C | 12 个真实高细节 Operator、13 项 catalog、`hard-surface-detail@0.2.0` active overlay、隔离 Worker 和 strict lineage/readback；Manifold C API 已进入 product-owned isolated Worker，`boolean@1` 为同一 Part bounded union/difference/intersection active；current packaged D rebuild、视觉门另行保留 |
| FGC-MCP010E | done | MCP010D | source-focused：first-party 离线硬表面 AssetPack、UV atlas、MikkTSpace、纹理/PBR provenance；xatlas/Validator/packaged/视觉子门 deferred |
| FGC-MCP010F | in_progress | MCP010E | Viewer compare/AOV/Part/MaterialZone/explosion source surface + contour-first Runtime target/Rig/Part compare slice；新增 hash-bound `SilhouetteTarget@1`、唯一非重叠 Part contour slices、64-render coarse-to-local camera fit、Runtime-owned bounded Primary Form search、`SilhouetteRig@1` bounded fit、SDF/Chamfer、single-Part proposal、candidate compare、directional boundary errors 和 MCP dispatch，source/aggregate tests PASS；Render Worker `--isolated-once` 严格单请求、Primary Form X/Y 轴 proposal、Viewer 直接消费 Runtime `QualityReport@2.hard_gate_passed` 已加入回归；新增 bounded RepairIntent proposal、CADFit OptimizationJob CAS checkpoint/resume 与真实 38-evaluation process Gate；image-derived `chest-shell` Part error→5-candidate correction transport 也已通过，但 winner IoU 0.7451、Boundary F1 0.3400 仍 `QUALITY_TARGET_NOT_MET`；真实参考 v18 已完成 candidate-0 unmodified baseline、9-slot `OptimizationResidual@1` residual family、product-owned Manifold bounded Boolean 搜索和 32/4/2 CADFit 评估，Boolean lane/readback PASS 但 winner 为无 Boolean shape-only candidate，严格多目标仍 `blocked-no-improvement`，无 proposal/confirm/version；新增 `surface-patch@1` open-surface 与 `surface-shell@1` constant-thickness watertight source Gate（4×4 Bézier/128 与 320 triangles/strict readback/negative Gate）已通过，但完整 surface backend、真实视觉 likeness、Viewer packaged UI/人评/360 仍独立未运行 |
| FGC-MCP011 | blocked | MCP010F | Job checkpoint/并发/崩溃恢复/配额/GC/全局性能 |
| FGC-MCP012 | blocked | MCP011 | 第三方 Skill 生命周期、外部项目深度治理、分发签名/撤销 |
| FGC-MCP013 | blocked | MCP012 | Developer ID/notarization、clean install、升级回滚、packaged Desktop/CLI、跨类别真人门 |

2026-08-15 ActionRun→CADFit 真实绑定增量：父 `DesignActionRun@1` 现在可以在受限 geometry action 中携带 `OptimizationIntent@1`，Runtime 创建同 run-bound child Job 并回写 child id/hash；真实回执 `docs/evidence/mcp010f/design-action-run-cadfit-handoff-real-reference-20260815.json` 的 38 次搜索已完成，但 strict gate 阻止 proposal（`blocked-no-improvement`），candidate/version/用户持久数据未变。该增量仍属于 MCP010F 的 handoff/transport 子门，不能把它标为完整 Agentic Runtime、Repair 执行或视觉质量完成。

2026-08-15 F contour correction 原子任务结果：`scripts/probe_mcp010f_part_correction.py` 已补齐四组件 build-cohort receipt、Runtime camera-fit target binding 和所有候选的同相机比较；真实回执 `docs/evidence/mcp010f/part-correction-real-reference-20260815-v9-same-camera.json` 使用授权参考 hash `946b1be7…f51f80`，5 个 `chest-shell` 候选共享 camera `35e938f5…7fdd`，winner 是 `chest-height +0.25`，IoU `0.523518 → 0.533249`、Boundary F1 `0.077158 → 0.079165`、critical-region min IoU `0.587601 → 0.614734`。该结果严格改善但全部 comparison 仍 `QUALITY_TARGET_NOT_MET`，仅为 proposal/transport evidence；无 confirm/version/export。账本 row 与 `scripts/check_mcp010f_current_quality_evidence.py` 已通过，下一步是全局/局部 target 一致性与多视图 promotion，不是直接确认或继续盲目扩大 Boolean 搜索。

2026-08-15 F target-consistency 原子任务结果：最新回执 `docs/evidence/mcp010f/part-correction-real-reference-20260815-v10-target-consistency.json` 在同一 camera 和同一 5 候选集上分别比较 automatic target/refined Part target；global winner 为 `chest-height +0.25`，但 Part-bound winner 为 baseline，`part_strict_improvement=false`、`target_ranking_consistent=false`。该差异以 `BLOCKED_GLOBAL_PART_WINNER_DIVERGENCE` 留在 ledger，禁止 promotion/Repair/Confirm；下一步是统一 target/ROI/PartError objective 后再做多视图 promotion。

2026-08-15 F unified-objective 原子任务结果：新增 `SilhouetteEvaluationObjective@1`、prepare/compare contracts 与两个 MCP read-only tool，统一 automatic global target、refined Part target parent lineage、PartError canonical hash、Runtime-owned camera ref 和 promotion policy。真实回执 `docs/evidence/mcp010f/part-correction-real-reference-20260815-v11e-unified-objective.json` 使用 cohort `752e1ad39233543749308a6e7c2b10d37156aa0812a3d6eb9d97afcc2de60274`，5 个 candidate 共用 camera `35e938…7fdd`；global-only improvement 与 Part-only improvement 发生冲突，objective compare 返回 `blocked_global_or_part_objective`、`winner=null`。`target_consistency_status=PASS_UNIFIED_GLOBAL_PART_OBJECTIVE` 仅代表协议一致性，候选仍全部 `QUALITY_TARGET_NOT_MET`，没有 confirm/version/export；v10 divergence 仍保留为历史阻断证据。下一项是让 OptimizationJob 读取并复用该 objective/promotion policy，再做多视图 promotion。

2026-08-15 F unified-objective → OptimizationJob 原子任务结果：`OptimizationIntent@1`、所有 checkpoint evaluation、best-so-far、final compare 和 `OptimizationResult@1` 现在绑定同一 `SilhouetteEvaluationObjective@1`；Global non-regression 与 Part strict-improvement 共同决定 unified promotion，Boolean residual 仍限制在 product-owned Manifold same-Part lane。真实回执 `docs/evidence/mcp010f/optimization-job-unified-objective-real-reference-20260815-cohort-v12.json` 以 cohort `4ac1ea60…9279a6` 完成 39 次 `32/4/3` 评估，`0.534967881136 → 0.534672666853`，内部 `promotion_status=ready`、结果 `proposal_status=proposed`；comparison 仍 `QUALITY_TARGET_NOT_MET`（IoU `0.529445`、Boundary F1 `0.090842`），没有 confirm/version/export。新增 ledger/checker row 已通过；这不构成 likeness/high-quality PASS，下一步仍是多视图 promotion/真实视觉门，而不是直接确认。

## 2.1 Agentic Design Runtime 后续 backlog（未领取）

以下条目来自 ADR-0026 和 `FORGECAD_AGENTIC_DESIGN_RUNTIME_PLAN.md`。它们不是当前 `ready` 任务，只有用户明确领取并拆成原子任务后才能进入主任务链：

| Backlog ID | 状态 | 依赖 | 目标 |
|---|---|---|---|
| FGC-ADR026-01 | blocked | MCP010F truth freeze | `SemanticSceneGraph@1` / `ModelUnderstandingBundle@1` Schema 与只读 producer |
| FGC-ADR026-02 | blocked | FGC-ADR026-01 | `ReferenceCanvas@1` / `DesignSpec@1`，记录 coverage、observed/inferred/unknown 和 stage criteria |
| FGC-ADR026-03 | blocked | FGC-ADR026-02 | `DesignSession@1` / `DesignCheckpoint@1` / `DesignStagePlan@1`，约束阶段门和 rollback |
| FGC-ADR026-04 | blocked | FGC-ADR026-03 | `scene_observe_get` / `visual_evidence_bundle_get`，一次返回 Codex 可判断的 hash-bound 设计现场 |
| FGC-ADR026-05 | blocked | FGC-ADR026-04 | Parametric Design Kit v0，将 Housing/Panel/Vent/Joint/Sensor/Frame 等 intent 展开为 typed bounded programs |
| FGC-ADR026-06 | blocked | FGC-ADR026-05 | `DesignCriticReport@1` / `RepairIntent@1`，只输出 evidence-bound single-Part/MaterialZone repair |
| FGC-ADR026-07 | blocked | FGC-ADR026-06 | 真实机器人 stage-gated visible-view loop + human/export/restart hash |

最近领取任务：

```text
`FGC-MCP010C`：实现固定 renderer、九 AOV、参考比较、Codex/human visual review 和 MCP image block。合成/raw Gate 与首次真实机器人 reference→compare→review transport 已通过；默认 camera auto-fit 与视觉指标 CAS round-trip 的最新 raw source regression 也已通过（IoU 0.6623，仍 `QUALITY_TARGET_NOT_MET`）；Viewer compare source implementation/local IPC-build tests 也已通过；当前最新真实 Codex receipt 已完成 current-cohort Viewer exact project/candidate/artifact/reference/render-set/comparison lineage read-model binding，但 packaged Viewer UI/accessibility、PBR/纹理、export/restart hash 和完整 360°仍必须保持独立状态。

补充当前事实：Dev.app packaged C 的安装/包验证/隔离探针、九 AOV raw renderer 和 Codex CLI compare/review transport，packaged D 的同 cohort Operator/strict readback raw probe，以及 packaged E 的同 cohort 用户参考结构传输已通过；当前 cohort packaged Viewer CLI read-model 已完成 exact lineage binding；packaged Viewer UI/accessibility、真实人评、PBR likeness、export/restart hash 和 360°仍独立保持 `NOT_RUN/BLOCKED`。
```

FGC-MCP010D 已完成 source-focused 退出条件，并新增 product Worker Boolean adoption receipt：12 个真实高细节 Operator、13 项 catalog（13 active，其中 Boolean 为同一 Part bounded union/difference/intersection）、`hard-surface-detail@0.2.0` active、strict lineage/readback、固定同级 Worker 隔离和负向回归均通过；current packaged D rebuild、真实视觉门仍为 `NOT_RUN/BLOCKED`。证据位于 `docs/evidence/mcp010d/`。

FGC-MCP010E 已完成 source-focused 退出条件，并新增 packaged structural 退出证据：65 个合同、`forgecad-hard-surface-robot@1.0.0` 离线 AssetPack、`uv-pbr@0.2.0`、512px bounded UV atlas、固定 `mikktspace@0.3.0`、嵌入式 PNG PBR 通道、九 AOV、raw stdio 和同 cohort 用户参考结构探针均通过。xatlas adoption、Khronos Validator、真实视觉/PBR likeness、独立人评、export/restart hash 与 360°仍 `NOT_RUN/BLOCKED`。证据位于 `docs/evidence/mcp010e/`。

2026-08-15 增量：真实独立进程回归进一步证明 synthetic fixture 的 `OptimizationJob` 38 次 coarse/mid/final 多保真评估、checkpoint/readback、最高已完成 fidelity 的 best-so-far 结果和无 confirm/version/export 边界；证据为 `docs/evidence/mcp010f/optimization-job-real-process-20260815-multiobjective.json`。同日真实授权机器人 PNG 的 `chest-shell` 单 Part CADFit 也完成 38 次评估；在统一 Boundary F1 物理容差后，final baseline loss `0.388631`→final proposal loss `0.384008`，coarse-8 overall best `0.371270`，proposal compare IoU `0.744929`、Boundary F1 `0.339688`，仍 `QUALITY_TARGET_NOT_MET`。proposal program/GLB 对象 hash 经 Runtime 重启读回；证据为 `docs/evidence/mcp010f/optimization-job-real-reference-20260815-metric-aligned.json`。两者均是 bounded proposal，不是正式 candidate 晋级或视觉质量 PASS。Manifold 当前 product-owned isolated Worker 的 union/difference/intersection 与 unsupported-shape negative Gate 通过；通用 mesh Boolean 仍不开放，current packaged、视觉/PBR/export-restart/人评仍需独立 Gate。

FGC-MCP010F 当前 source-focused in_progress：Viewer 已接入只读 Runtime projection，支持九 AOV、reference/render split/overlay/flicker、显式轮廓画布、与 Runtime `mask-2` 同源的 ephemeral border-connected flood-fill reference-contour aid、Part/MaterialZone 筛选、临时爆炸图、差异热图辅助、contour-first 阶段/累计门提示和 Codex correction queue；TypeScript/Vite/Tauri source Gate 已通过；comparison-sheet 与 hash-bound fit-plan 只在临时目录整理现有视觉证据，不写 Runtime/CAS。轮廓画布只是选择同一 candidate 的 silhouette AOV 与 overlay，reference-contour aid 只用于 Viewer 视觉提示，不创建第二套 Runtime mask；视觉解锁只信任 candidate-bound `QualityReport@2.visual_status + hard_gate_passed`，结构 candidate 的 `quality_hard_gate_passed` 不会清空视觉队列。fit-plan 已实行 `reference-canvas → silhouette-blockout → landmark-structure → semantic-part-fill → surface-detail → uv-pbr → final` 门控，轮廓未过时不输出 landmark/form/material 修改。Runtime 新增 `silhouette_part_error_get` 多 Part 误差表，供 Luna 按局部 boundary error 选出一个修正 Part；多 Part source regression 与真实 chest-shell transport 已通过。用户机器人 PNG 的 attempt35 记录 unrotated surface-linework + armor-shell-zones，26 Parts/4704 triangles，silhouette IoU 0.741047、boundary F1 0.328765、landmark coverage 0.733333、NME 0.134536，仍 `QUALITY_TARGET_NOT_MET`；它只是 benchmark eligibility `BLOCKED_INCOMPLETE_BINDING` 的 provisional retained observation。当前最新 receipt `real-codex-cli-current-20260814-viewer-bound.json` 已完成同 cohort transport 与 packaged Viewer exact project/candidate/artifact/reference/RenderSet/comparison binding，但 compare IoU 0.688698、Boundary F1 0.248825，仍未晋级质量门；image-block consumption、VoiceOver、PBR、人评、export/restart hash 和 HQ_360 仍 `NOT_RUN/BLOCKED`。证据位于 `docs/evidence/mcp010f/`。

自动 mask→contour 现在改为有向栅格边界追踪，确定性选择最大外环并避免分离组件污染主轮廓；`automatic_contour_points_are_ordered_and_follow_outer_mask_boundary` 重建回归 IoU > 0.94。带 15 个 intake landmarks 的同 cohort Codex 回合已完成 target/camera/Rig/fit/compare/九 AOV/review/quality transport，但最终 IoU 0.685417、boundary F1 0.272115、landmark coverage 0.666667、NME 0.134407，低于 attempt35，故不晋级基线；这证明地标进入 Runtime 排序输入，不证明视觉门通过。

本轮 contour-first source slice 已将 Viewer 的临时草图升级为 Runtime 可验证 target：`reference_mask_prepare`/`reference_mask_refine_prepare` 生成不可变 CAS target，`silhouette_target_get` 只读回 target，`camera_fit_prepare` 运行 37 个覆盖 yaw/pitch/FOV/distance/roll/target-offset/global-scale 的粗候选并对前三名各做 9 个局部探针（硬预算 64 次真实渲染）；`silhouette_fit_prepare` 使用最多 64 个 128×128 transient batch 评估并归一化到 512×512 指标，扩展到 roll/FOV/distance/target-offset/scale 与 Rig 参数，并返回 SDF/Chamfer；`part_contour_fit_prepare` 和 `silhouette_candidate_compare` 只读输出 bounded proposal/排序；`boundary_error_get` 输出最多 64 个方向误差段。target round-trip 与真实渲染 camera-fit/fit batch 单测通过；调用说明和 Luna 停止规则见 `docs/CODEX_SILHOUETTE_FIT_WORKFLOW.md`。该增量仍不改变当前机器人质量状态，也不把单张参考扩展为 360°证据。
其中 `silhouette_fit_prepare` 已补上 Part-aware Rig proposal：target 有 typed Part slice 时，在选定相机进行一次 bounded Part-ID readback，匹配 `part_id` 的宽高/缩放/偏移使用局部 envelope 与质心，未标注参数继续使用全身 fallback；无 slice 不增加渲染开销。该实现与合成局部优先回归已通过，真实胸甲 probe 仍仅记录 proposal/compare，不代表 likeness 通过。

010A 的旧代码清理与恢复 Gate 已 PASS：旧 Provider/Agent/standalone Host 入口、旧评估和孤儿运行残留已移除或隔离，两份 Host receipt 仅作为 `SUPERSEDED` 历史归档，用户 `output/`、`WushenForgeLibrary`、Runtime V1 与 Codex 历史均保留。MCP010A 的 30-tool Desktop receipt、MCP010B 的 3c/f488 Dev.app receipts和 884/896-triangle structural probes均为历史，原样保留。MCP010B 的范围内 subtotal 为 52 contracts（44 历史 + 8；含 `GeometryQualityReport@2`、`GeometryCandidateEvidence@1`），并已通过 V2 geometry/readback、Skill integrity、Worker isolation、MCP004/MCP007/MCP008/MCP009 回归、V2 restore hardening 和 closed GLB profile focused Gate；Darwin 512 MiB OS 总内存硬门仍 deferred/NOT_RUN。MCP010C 的 source subtotal 新增 7 个合同；`script/test_mcp010c.sh` 已通过固定 512×512 perspective/z-buffer renderer、九 AOV、candidate-bound reference comparison、MCP image block、Codex/human review 和确定性 raw stdio Gate；同 cohort 真实 Codex CLI 另完成六 turn/32-call C transport，receipt 为 `docs/evidence/mcp010c/real-codex-cli-current-20260812-attempt2.json`。当前 E source 之后仓库总合同为 65（另加 E 的 6 个合同）。C 的 synthetic/reference/CLI structural evidence 不等于用户机器人 likeness 或高质量视觉 PASS；Viewer compare、packaged C、真实用户评分、PBR/纹理、export/restart hash 和 HQ_360 仍未运行，证据账本位于 `docs/evidence/mcp010c/manifest.json`。

## 3. MCP004 为什么现在可以 done

MCP004 的 MVP 责任是提供后续 3D 能力可复用的单用户事务与生命周期基座，不再把生产签名、真实附件、GLB exporter 和 packaged Desktop write 塞入同一前置任务。当前证据已覆盖：

- Runtime/IPC candidate prepare、quality/approval-bound confirm/reject、idempotency、stale/hash/expiry fail-closed；
- restore-as-new-version 和 path-free diagnostic manifest export；
- `forgecad-mcp` 拥有 stdio、异步启动/连接 Runtime、Runtime 缺失或崩溃时 stdio 继续、一次有界 restart；MCP010A 已补齐共享 Runtime 的启动选主、适配器退出存活和异常客户端隔离回归并通过最终源码 Gate；
- OS 文件锁单写者、进程退出自动释放、第二 Runtime `RUNTIME_BUSY`；
- 默认只读和显式 opt-in write 边界；
- 真实 Codex CLI diagnostic write E2E 与 Viewer authenticated read model；
- `npm run release:mcp004` PASS。

`docs/evidence/mcp004/manifest.json` 保留当时 `in_progress` 的历史现场与 `BLOCKED/NOT_RUN`，不篡改原始证据。未完成项已经转移：reference import 属于 MCP005，几何/外观/质量/GLB 属于 MCP007–009，distribution signing/packaged Desktop 属于 MCP013。这个范围调整不把它们写成 PASS。

## 4. MVP 任务退出条件

### FGC-MCP005 — Reference Intake（已完成）

Owned：Reference Schema、图片 admission、Runtime/CAS、MCP tool/probe、MCP005 evidence。

必须：

- `reference_import` 和 `reference_get` 使用公开 Schema；
- P0 只接受 PNG/JPEG，有限 byte/pixel/dimension/frame/decode memory；
- canonicalize、authorized root、symlink/目录/设备文件/MIME/魔数检查；
- CAS hash 与真实源字节一致，永久状态丢弃绝对路径；
- 真实 Codex CLI attachment-byte E2E；Desktop 不可传时记录 unavailable；
- success + truncated/oversize/decompression-bomb/path escape/symlink/hash mismatch tests；
- 更新 capabilities、合同、用户文档和 `docs/evidence/mcp005/manifest.json`。

已完成证据：PNG/JPEG Runtime admission、authorized-root/outside-root/symlink/hash/MIME negative tests、CAS readback、authenticated MCP adapter、真实 Codex CLI `project_create → reference_import → reference_get`。Codex Desktop 当前 bridge 仍记录 `NOT_RUN / unavailable`，不影响 CLI 任务退出；原图路径和字节没有进入仓库、DB、receipt 或日志。MCP010C 另有一份隔离真实机器人九-AOV compare/review receipt，但当前视觉阈值未通过。

禁止：Geometry/Render、远程 image-to-3D、Blender/Python、将图片复制进 Git。

### FGC-MCP006 — Contracts + MVP Skills

Owned：SubjectProfile/RepresentationPlan/AssemblyGraph/GeometryProgram/AppearanceProgram/RecipePlan Schema，first-party Skill 包、validator/benchmark、Registry 开发模式。

必须：

- 10 个核心 Skill 的知识、Schema、Recipe、operator lock、validator、fixture、LICENSE/NOTICE、SPDX SBOM、provenance；
- first-party canonical hash/trust root；分发签名延后但完整性不可省略；
- DAG cycle、unknown operator、错误单位、non-finite、预算、license/SBOM/hash drift fail closed；
- Bundle 不携带 executable、不访问网络/环境变量/任意路径；
- 同输入 canonical plan hash 可重复。

已完成：44 个 contracts schema；Runtime 内置 development-only Skill Registry 的十个 first-party Skill；`skill_list`/`skill_get` 与 `forgecad://skills/{skill_id}/{version}` 只读资源；十个独立 `bundles/<skill_id>/0.1.0` 标准目录；每项 Recipe、operator lock、validator subset、synthetic adversarial fixtures、benchmark receipt、LICENSE/NOTICE、SPDX SBOM、provenance、development trust manifest 和明确延期签名占位；registry/bundle hash、DAG cycle、单位、finite、预算、未知 operator、路径/脚本/网络 capability 等 fail-closed Gate；真实 Codex CLI `capabilities_get → skill_list → skill_get` 和 Runtime/MCP focused tests。MCP006 不把 Skill metadata 误写成 3D 结果，后续 geometry/appearance consumer 已分别在 MCP007–009 通过 focused Gate。

### FGC-MCP007 — Geometry Vertical Slice

Owned：geometry contracts/core/worker、Runtime compile orchestration、GLB lowering/readback、Viewer artifact display、MCP007 evidence。

MVP 当前退出 Gate 只要求已经实现的 bounded primitive/transform 子集；其余 Operator
不得被 Skill metadata 伪装成可执行能力：

- 当前 allowlist：`forgecad.geometry.primitive@1` 的 box/cylinder/sphere 与有界 transform；
- 声明式但延期：profile/extrude/revolve/sweep/loft/mirror/array/bounded boolean/bevel/hard-surface macros；
- 机器人由多个稳定语义 Part 组成，不是单 mesh 占位；
- 真实非空 mesh/GLB，finite/index/normal/degenerate/manifold/budget Gate；
- Part/MaterialZone/source Operator lineage strict readback；
- deterministic fixture、恶意参数、timeout、Worker crash 和 no-version-on-failure；
- Viewer 显示同一 candidate hash；提交 GLB/readback/wireframe/part-ID evidence。

已完成（MCP007 evidence）：product-owned geometry worker library/binary 只接受 canonical `GeometryProgram@1`，当前实现 box/cylinder/sphere、有限预算、finite/unique ID/allowlist 校验和确定性 glTF 2.0 GLB lowering；Runtime 在候选事务中写入 GLB CAS、生成 `GeometryQualityReport@1`、返回 `ArtifactReadback@1`，并提供 authenticated `artifact_readback_get`。机器人 fixture 有 14 个语义部件、516 triangles；worker fixture 有 3 部件、332 triangles，重复输出 hash 相同。Viewer read model 通过 authenticated IPC 读取候选和 artifact readback 元数据，不启动 Runtime、不写数据库。focused worker/Runtime/MCP/Viewer Gate 与 `npm run mcp007:test` PASS；真实 Codex CLI 已使用用户授权 PNG 完成同一 typed geometry slice，14 parts/516 triangles/validator passed，见 `docs/evidence/mcp007/codex-cli-geometry.json`。MCP009 真实 Codex receipt 另证明该 geometry slice 可继续进入 Appearance/Render/Quality/Confirm/Export。当前实现刻意没有宣称 profile/extrude/revolve/sweep/loft/boolean/bevel 全量、参考相似度或通用质量；后续任务只在真实合同下扩展。

### FGC-MCP008 — Appearance + Render + Viewer（done）

Owned：UV/tangent/PBR contracts/compiler、render worker、Viewer 3D/part/material UI、MCP008 evidence。

已通过（功能核心）：

- UV unwrap/pack、MikkTSpace、glTF metallic-roughness + AO/Normal/Emissive；
- 白外壳/黑机械/橙 emissive typed MaterialZone；
- GLB validator 0 errors、Runtime strict readback；
- Viewer 只消费 Runtime artifact，选择/隔离不写版本；
- Viewer 关闭时 headless beauty/silhouette/normal/part-ID 仍生成；
- 固定 camera/light/resolution/renderer version/hash receipt。

证据：`docs/evidence/mcp008/manifest.json`、`worker-fixture.json`，以及 `npm run mcp008:test`；真实 Codex appearance/readback 由 `docs/evidence/mcp009/codex-cli-appearance-export.json` 的十二调用 receipt 覆盖。限制：没有把合成 fixture、单张 beauty 或 Skill metadata 写成参考相似度 PASS；packaged Viewer、像素/区域指标和真人评分仍是 `NOT_RUN`。

### FGC-MCP009 — MVP Golden Path（功能核心 done）

Owned：reference compare/Quality、SemanticChangeSet、production GLB export、Codex probe、完整 evidence pack、MVP 文档。

必须：

- `quality_get` 输出 Runtime-owned QualityReport；当前 reference compare 是明确标记 `limited` 的 aspect-ratio evidence，不冒充 silhouette IoU/landmark/region 完成；
- Codex visual review 绑定具体 render/pass/region，不能覆盖硬门；
- `change_prepare` 对稳定 Part ID 做一次有界局部修改，要求 base version 和新 typed programs；MVP 不实现通用 mesh-delta/DAG 复用；
- reject 不写版本；approve 只写一个不可变子版本；restore 创建新版本；
- export 绑定 confirmed version/artifact/quality lineage，`mvp-glb` 返回 CAS GLB hash 和 manifest receipt；MVP 不写任意本机路径；
- Runtime focused golden path、reject/restore/idempotency 和 24 个 Runtime tests、16 个 MCP tests 已通过；真实 Codex CLI 已使用授权图片完成十二调用 `project_create → reference_import → geometry_prepare → artifact_readback_get → appearance_prepare → artifact_readback_get → quality_get → candidate_confirm → version_list → export_prepare → export_confirm → version_list`，并取得 geometry/appearance/readback/fixed-render/quality/CAS GLB receipt；
- 因此可以声明“单用户 MVP host golden path 可供开发评估”，但不能宣称通用高质量、像素级参考相似度、真人验收或 packaged release。`change_prepare`、restore、restart 同 hash 和 Viewer 交互仍是独立后续 host Gate。

证据：`docs/evidence/mcp009/manifest.json`、`codex-cli-appearance-export.json`。下一步不是继续堆基础设施；若继续 Goal，先做 Viewer 同 hash、局部修改/restore 的真实 host 验证，再做独立真人评审。不得把 CAS receipt 或 aspect-ratio limited 比较升级为产品质量结论。

## 5. MCP010 质量升级与后续退出边界

MCP010A–F 的详细要求见 `MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`。它们不改写 MCP005–009 的历史 PASS：当前单图目标最多是 `PARTIAL_VISIBLE_VIEW_PASS`，补齐五张全身参考前 `HQ_360_PASS=BLOCKED_REFERENCE_COVERAGE`。Job checkpoint/GC/通用并发属于 MCP011；通用第三方 Skill/AssetPack 安装、publisher、签名和撤销属于 MCP012；外部分发、自动安装、Developer ID/notarization、filesystem/package export、packaged E2E 和跨类别质量宣传属于 MCP013。

## 6. 每任务证据

每个任务新建 `docs/evidence/mcpXXX/manifest.json`；MCP010 原子任务使用 `docs/evidence/mcp010a/` 至 `mcp010f/`。至少记录 worktree/base、命令和 exit code、artifact/contract/dependency hash、focused/aggregate/real Codex、FAIL/BLOCKED/NOT_RUN、无绝对路径/secret 检查。视觉任务另含 ReferenceEvidence、program、GLB/readback、RenderSet、QualityReport 和人工评分。

旧 U/C/K/F/E/VP/U004 任务均为 `superseded` 或历史，不重新进入依赖链。

2026-08-13 F Viewer binding 子任务已完成 source-focused 退出检查：同 candidate/project 的 artifact、reference、RenderSet 和 visual evidence 为唯一可显示/比较路径；cross-candidate、missing-evidence 和 payload hash mismatch 均 fail closed。receipt：`docs/evidence/mcp010f/viewer-candidate-binding-source-20260813.json`。authenticated IPC geometry fixture 因 `GEOMETRY_WORKER_UNAVAILABLE` 为 `BLOCKED`，不得升级为 Viewer packaged 或视觉质量 PASS。

合同复核补充：`QualityReport@2` 没有 `project_id`，Viewer 不得假设该字段存在；项目范围由 Runtime `ViewerVisualEvidence@1` envelope 加上 read model/reference、RenderSet、ComparisonReport 和 artifact hash 的同 candidate 校验建立。该补充已由 source checker 的正向/负向 fixtures、receipt 和 Stage 0 truth 固化，仍不推进 packaged、人评、PBR、export/restart 或 360 子门。
