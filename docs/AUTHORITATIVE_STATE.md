# ForgeCAD 权威状态与版本真值

2026-08-15 SurfaceRig 多控制点 CADFit v15（additive evidence）：在用户参考 hash `b9cb687e2cb6b2646bd47236efb76edeb27ccac5f1efdf595f9342f133c1cadd` 和统一四组件 cohort `a21e448fd564910578c83e4c2798a59bf15d00713edc172b51346de44f057106` 上，把隔离的 `chest-shell` source 改为 `surface-shell@1` 的 16 控制点，并以 `SilhouetteRig@1` 的 8 个 `surface_control_point` 参数运行 v5 grouped search。39 次评估为 `32/4/3`，Runtime-owned camera hash 为 `a386776504853e316215fda23d8a5ec425d16b1ebbef6a7f36b9e4101edf76ce`；receipt 记录非 baseline 多控制点候选 `[1..21,23..31]`，baseline 与 candidate 22 未改变。final baseline/best loss 为 `0.379948070031 → 0.379344569175`，全局 silhouette IoU/F1 为 `0.749726/0.344269 → 0.750113/0.346008`，但 Part silhouette IoU 为 `0.116673 → 0.115077`，故 `strict_improvement=false`、`proposal_status=blocked-no-improvement`、`promotion_status=blocked_global_or_part_objective`；Manifold residual 仍只在候选 `[1..9]` 的同一 Part bounded lane 运行。没有 proposal、confirm、version、export 或持久用户数据写入；该 receipt 证明 typed 多控制点物化、分组搜索、readback 和 fail-closed quality gate，不是视觉 likeness 或高质量 PASS。

2026-08-15 最新真实参考闭环：当前授权参考的 additive closure index 为 `docs/evidence/mcp010f/real-reference-quality-closure-20260815.json`，参考 hash 为 `b9cb687e…c1cadd`，MCP/Runtime/Geometry Worker/Render Worker 统一 cohort 为 `61e01276…bf340a`。真实 `OptimizationJob`/ActionRun child 均成功完成 39 次 `32/4/3` 多保真评估；候选 `[1..9]` 进入 product-owned `Manifold-C-ABI` 的同一 Part residual lane。Runtime 已修复跨进程 RenderSet→CameraCalibration CAS rehydration、refined Part target fallback 及跨语言浮点 canonicalization，optimization/comparison camera hash 一致。视觉状态仍为 `QUALITY_TARGET_NOT_MET`，strict improvement 为 false、promotion/proposal 被阻断；candidate 未 confirm、version count 0、持久用户数据未触碰。human review/export-restart hash 为 `NOT_RUN`，HQ_360 为 `BLOCKED_REFERENCE_COVERAGE`。历史 current ledger 和 Stage 0 provisional observation 未被改写。
2026-08-15 当前源码同 cohort revalidation：用当前 SurfaceProgram/ActionRun 源码构建的 MCP/Runtime/Geometry Worker/Render Worker 统一 cohort 为 `613470b6…af04a`，在同一授权 PNG（reference hash `b9cb687e…c1cadd`）上重新运行 `docs/evidence/mcp010f/optimization-job-real-reference-20260815-user-reference-boolean-residual-v12.json` 与 `docs/evidence/mcp010f/design-action-run-real-reference-20260815-user-reference-with-optimization-v13.json`。两条链均完成 `32/4/3` 共 39 次评估，optimization/comparison camera hash 一致，Manifold residual lane/readback 成功；结果仍为 `QUALITY_TARGET_NOT_MET`、strict improvement=false、proposal=`blocked-no-improvement`，candidate 未 confirm、version count 0、持久用户数据未触碰。该 revalidation 只证明当前源码没有破坏真实 transport，不晋升 Stage 0 benchmark，也不改变人评/PBR/export-restart/360 的 `NOT_RUN/BLOCKED` 事实。
2026-08-15 SurfaceProgram ActionRun source/test slice：`RuntimeParameterPatch@1` 已支持 `surface-control-points-v1`，只对同一 Part 内唯一的 `subd-cage@1`、`surface-patch@1` 或 `surface-shell@1` 节点解析有限 `control-point-{index}-{x|y|z}` 变更；Runtime 生成 hash-bound `RepairIntent@1` 并物化独立 review candidate，源 candidate/version/confirm/export 不变。正向 surface-shell ActionRun 与控制点越界负向回归均通过，receipt 为 `docs/evidence/mcp010f/runtime-parameter-patch-surface-control-points-20260815.json`。这是 typed proposal/source Gate，不是 Visual Surface backend 完成或真实参考视觉 PASS。
2026-08-15 真实 SurfaceProgram ActionRun：在当前源码四组件 cohort `613470b6…af04a` 上，把用户授权机器人参考中的 `chest-shell` 隔离改为 16 控制点 `surface-shell@1`，使用同一 camera `8cd20605…a535` 运行 `RuntimeParameterPatch@1 / surface-control-points-v1` 的 `control-point-5-z`（`0.12m → 0.20m`）。`prepare → compile → readback → render → evaluate` 全部完成；source/proposal comparison 分别为 silhouette IoU `0.744889/0.745584`、Boundary F1 `0.326291/0.328475`，但 composite score `5.681233147408 → 5.688397884520`，`strict_improvement=false`、`non_regressing=false`、promotion=`rejected-regression`。source candidate 未被改写，review candidate 未 confirm，version count `0`，视觉仍 `QUALITY_TARGET_NOT_MET`；该 receipt 只证明真实 SurfaceProgram 参数补丁和同相机质量门已闭合，不升级 likeness、高质量、PBR、人评、export/restart 或 HQ_360。

2026-08-15 ActionRun reference boundary：Runtime 已把 `request-reference` 收口为绑定 session authorized `reference_id` 的 `BLOCKED_REFERENCE_COVERAGE` prepare-only 分支；后续 pipeline、checkpoint、OptimizationJob 和候选/version/export 均不执行。`DesignSession.next_actions` 的 reference request 已与该合同一致。该增量只证明 Agent pipeline 的缺参考 fail-closed 分支，不升级当前机器人 `QUALITY_TARGET_NOT_MET` 或 360 证据。

2026-08-15 多视图 promotion 正向 synthetic fixture：`docs/evidence/mcp010f/cross-view-promotion-positive-synthetic-20260815.json` 已通过 Runtime focused conformance。完整 `ReferenceCanvas@1` 的六个 supplied view kind 必须对应六个 authored view entity；同一 candidate 的六组 RenderSet/Comparison/Quality 通过 aggregate strict-improvement/non-regression 后，`cross_view_promotion_confirm` 在 fresh approval 下创建一条 immutable version，重复请求 exact replay 且不重复写版本。该 fixture 使用 Runtime 生成的同一 silhouette/reference/camera，只证明事务、hash lineage、current-head base-version 与幂等边界，不证明真实参考 likeness、PBR、人评、export/restart 或 HQ_360。

2026-08-15 v11f OptimizationJob unified-objective 状态：真实回执 `docs/evidence/mcp010f/optimization-job-unified-objective-real-reference-20260815-cohort-v12.json` 绑定四组件 cohort `4ac1ea60…9279a6`，完成 39 次 `32/4/3` 评估；`OptimizationIntent`、checkpoint、resume、final compare 与 result 都绑定同一 `SilhouetteEvaluationObjective@1`，Global non-regression 与 Part strict-improvement 共同满足时内部 promotion 为 `ready`。本次 `0.534967881136 → 0.534672666853` 且 proposal 为 `proposed`，但 fixed-camera compare 仍 `QUALITY_TARGET_NOT_MET`（IoU `0.529445`、Boundary F1 `0.090842`），没有 confirm/version/export；Manifold 仅为 bounded same-Part Boolean residual lane，不是通用 mesh Boolean 或 likeness PASS。

2026-08-15 SurfaceProgram Worker source Gate：`forgecad.geometry.surface-patch@1` 的 128-triangle open patch、`forgecad.geometry.surface-shell@1` 的 320-triangle constant-thickness shell 与 `forgecad.geometry.subd-cage@1` 的 3×3 regular quad cage（0/1/2 级为 8/32/128 triangles）已接入 `GeometryProgram@2`、Runtime/Worker catalog 和 bounded Geometry Worker；重复编译、strict readback、UV/tangent 与负向参数 Gate 通过，shell 另通过 watertight/non-manifold readback，SubD cage 另通过 control-point edit/determinism/readback Gate。receipts 为 `docs/evidence/mcp010f/visual-surface-patch-gate-20260815.json`、`docs/evidence/mcp010f/visual-surface-shell-gate-20260815.json`、`docs/evidence/mcp010f/visual-surface-subd-cage-gate-20260815.json`；完整 Visual Surface backend 仍 `NOT_RUN`，不改变视觉/benchmark/PBR/human/export/360 事实。

2026-08-15 CADFit continuation 已完成当前 cohort 验证：真实 Codex CLI 探针新增可选 `--cadfit-optimization`，在同一 candidate/target/refined camera 上提交单 `chest-shell` 的 6 参数 `OptimizationIntent@1`，并读取 32/4/3 多保真 Job 终态；Runtime prepare/resume 先返回 queued snapshot 再 spawn Worker，MCP optimization wire 对连续数值执行受限规范化，并重绑定 nested camera/Rig hash。rustup stable toolchain 重建的 MCP/Runtime/Worker cohort 为 `675df14b5e24c02a4dbf463098894ac59a612b00d4e7c6436fc3e64b80b0035f`，Runtime optimization 10/10、MCP 61/61。真实授权参考回执 `docs/evidence/mcp010f/real-codex-cadfit-real-reference-20260815-rebuilt-rig-rebind.json` 的 loss 为 `0.466875204210 → 0.462318253808`，strict improvement/proposal 为 true；但视觉仍 `PASS_WITH_QUALITY_TARGET_NOT_MET`，没有 confirm/version/export。该增量不改变 Stage 0 `BLOCKED_INCOMPLETE_BINDING`、PBR/人评/restart/360 未完成事实。

2026-08-15 ActionRun→CADFit 真实 handoff：`design_action_run_prepare` 现支持可选 hash-bound `OptimizationIntent@1`；Runtime 在父 ActionRun 边界校验 project/candidate/session/action/stage/Part scope、外层 intent hash 与 nested camera canonical hash，再创建同一 `run_id` 的 child `OptimizationJob`。父回执保持 review-only，proposal 与 optimizer intent 互斥，checkpoint/非几何/跨 Part 输入 fail closed。真实授权 PNG 回执 `docs/evidence/mcp010f/design-action-run-cadfit-handoff-real-reference-20260815.json` 为 `PASS_ACTION_RUN_CADFIT_HANDOFF`：38 次评估（32/4/2）完成，但 `strict_improvement=false`、`proposal_status=blocked-no-improvement`；candidate 未变、version count 0、未写持久用户数据。该证据只证明绑定和搜索传输，不改变 `QUALITY_TARGET_NOT_MET`、`BLOCKED_INCOMPLETE_BINDING`、Repair/视觉/360 未完成事实。

2026-08-15 CADFit/Manifold v19c 局部绑定修正：`SilhouetteTarget@1.part.region` 现在允许 Runtime 消费由参考图像标注的归一化 ROI；`target_part_region_mask` 将 Runtime-owned 全身 mask 裁剪到该 ROI，CADFit `part_region`、Part contour error 和 Part fit envelope 优先使用它，缺少 ROI 时才回退到声明的 contour slice。真实回执 `docs/evidence/mcp010f/optimization-job-real-reference-20260815-boolean-residual-v19c.json` 在同一授权参考上通过 32/4/2 共 38 次评估，`part_target_binding=PASS_IMAGE_DERIVED_REGION_BOUNDED_REFERENCE_MASK`，`chest-armor` ROI 为 `x=0.292,y=0.285,width=0.435,height=0.285`；candidate 0 无 Boolean，候选 `[1..9]` 进入 product-owned Manifold Boolean lane，baseline/best loss `0.472832854888/0.470494645212`，但 `strict_improvement=false`、`proposal_status=blocked-no-improvement`。ROI 是显式图像区域，不是语义分割或 likeness 通过；没有 confirm/version/持久用户数据写入，仍 `NO_LIKENESS_PASS_CLAIM`、`QUALITY_TARGET_NOT_MET`。

2026-08-15 residual family v3 状态：真实回执 `docs/evidence/mcp010f/optimization-job-real-reference-20260815-boolean-residual-v18.json` 固定 candidate 0 不含 residual，并在候选 `[1..9]` 搜索 `residual-chest-sphere-boolean`；完成 32/4/2 多保真评估，baseline/best loss 为 `0.480812392901/0.478756028517`。最终最佳程序不含 Boolean，`strict_improvement=false`、`proposal_status=blocked-no-improvement`，所以不能把 Boolean 搜索存在误写成视觉改善或候选晋级。该 receipt 固化 bounded same-Part residual search、Manifold transport、hash-bound result 和 fail-closed proposal gate；语义 Part residual、通用 mesh Boolean、visual/PBR/human/export/restart/360 仍未完成。

2026-08-15 最新源代码覆盖：当前为 123 Schema、40 read + 30 opt-in write = 70 tools。新增只读 `visual_surface_get`/VisualSurfaceResult@1 candidate-bound diagnostic projection，内含 `VisualSurfaceReadback@1` 的九 AOV/mask/edge/Part-ID ROI CAS readback，surface program backend 为 `NOT_RUN`；`SilhouetteEvaluationObjective@1` 现在可由 `OptimizationIntent@1` hash-bound 引用，并由 Runtime 在 checkpoint/resume/final compare/result 中统一 Global non-regression 与 Part strict-improvement；`DesignCriticReport@1.visual_surface` 绑定 readback canonical hash，`OptimizationResidual@1` 在 Manifold/CADFit 前重新校验 ready/readback lineage；`repair_apply_confirm` 仅消费单视图、fresh-approval 且重新校验 source/run/proposal/RepairIntent/artifact/visual lineage 的 apply intent，并通过 Runtime Store 创建不可变版本；`design_action_optimization_proposal_prepare` 只把严格且 non-regressing 的 ActionRun-bound CADFit 结果物化为独立 review candidate，要求显式 ViewSpec，不自动 Repair/Confirm；多视图仍必须走 `cross_view_promotion_confirm`。该路径已完成合同/Runtime/MCP/Store 源码接入，Visual Surface readback/negative Gate、Stage 0、`cargo check --workspace`、Runtime full test 与 Repair focused test 已通过；真实 v11f receipt 已完成 39 次 proposal-only 评估，但视觉仍为 `QUALITY_TARGET_NOT_MET`，benchmark 仍 `BLOCKED_INCOMPLETE_BINDING`。

当前计数校正（2026-08-15）：以 Stage 0 marker 和机器真值为准，源码为 123 Schema、40 read + 30 opt-in write = 70 tools；下方早期 105/106/107、118/120 与 61/62/63/67/68 叙述仅保留历史快照，不得作为当前能力计数。`design_stage_run_prepare` 是有界同阶段独立批处理，首个质量门即停止；`design_composition_prepare` 记录 2–6 项线性 ordered proposal，并可在显式 `cumulative-program` 哈希链通过且动作批次完成后编译独立 merge candidate；正向确定性 fixture 已证明两步批次、父哈希链和独立 review candidate prepare，真实参考/完整 orchestrator 仍需单独验证。`repair_apply_prepare` 只生成 source/proposal/RepairIntent/quality/evidence 绑定的 CAS apply intent，仍 fail closed；`design_action_optimization_proposal_prepare` 只物化严格且 non-regressing 的独立优化候选。`CrossViewEvidenceBundle@1` 与 `cross_view_promotion_confirm` 已有完整覆盖 synthetic promotion/replay receipt，但真实参考仍必须独立通过跨视图质量、人评与审批边界。MCP010D 已通过固定 Manifold C API 的 product-owned isolated Worker adoption，`boolean@1` 当前 active 为同一 Part bounded union/difference/intersection；通用 mesh Boolean、视觉/PBR/human/export/360 仍未通过。

2026-08-15 真实质量闭环增量：`optimization-job-real-reference-20260815-boolean-residual-v14.json` 证明授权参考的 `critic_report_get → silhouette_part_error_get → OptimizationResidual@1 → Manifold boolean → OptimizationJob` 传输与 readback 已完成；38 次评估按 32/4/2 运行，Boolean residual node 进入 best program，Runtime result 为 succeeded，但严格多目标 gate 仍 `blocked-no-improvement`，没有 proposal、confirm 或 version。MCP 优化请求的 camera canonical rebind 已将连续数值规范化后重新绑定 camera/intent hash，避免跨语言浮点序列化造成的假漂移。该证据只升级 bounded residual transport 和 Manifold Worker 的可验证性，不升级真实机器人 likeness、高质量视觉、通用 mesh Boolean、PBR、人评、export/restart 或 360。

补充校正：下方若出现 115/65，是 `repair_apply_confirm` 接入前的历史 reconciliation 快照；当前能力计数仍以本段 118 / 37 + 30 = 67 为准。

版本：2026-08-15
状态：MCP005–009 functional truth 已实现；FGC-MCP010A done；MCP010B structural truth source Gate 已通过但 Darwin OS memory hard cap deferred/NOT_RUN；MCP010C source-focused renderer/compare/review Gate、MCP010D hard-surface Operator/Skill Gate 与 MCP010E 离线 AssetPack/UV/PBR/MikkTSpace Gate 已通过，首次真实机器人 compare/review transport 已运行但 likeness threshold `FAIL_QUALITY_TARGET_NOT_MET`；MCP010F Viewer source Gate 已通过，当前 cohort packaged Viewer CLI read-model 已完成 exact project/candidate/artifact/reference/render-set/comparison lineage binding，正式 UI/accessibility、独立人评、360 仍 NOT_RUN/BLOCKED。当前 F 轮廓 slice 另提供 Runtime-owned `silhouette_part_error_get`：按 hash 绑定的多 Part table 归因局部边界误差，source focused PASS；它不改变当前真实图片 likeness 失败事实。ADR-0026 的 observe/plan projection、受批准 durable `DesignSession`/`DesignCheckpoint`/`RepairIntent` prepare/readback、显式 bounded `authoring_context` producer/readback、逐视图 evidence inventory、primary-form/secondary-structure/tertiary-detail single-Part proposal executor、bounded independent stage batch、确定性 positive cumulative merge-candidate prepare 和完整覆盖 synthetic cross-view promotion/replay fixture 已成为当前 source/runtime/MCP/Viewer 证据层；真实参考上的完整多动作 orchestrator、跨视图质量/人评、MaterialZone/UV-PBR executor、Repair 应用、真实候选 promotion 和完整视觉门仍未完成。

Stage 0 权威快照：当前为 123 Schema、40 read + 30 opt-in write = 70 tools，唯一 `in_progress` 为 `FGC-MCP010F`；机器可读事实入口是 `docs/evidence/mcp010f/current-benchmark-truth.json`。attempt35 只称 `provisional retained observation`，视觉状态为 `QUALITY_TARGET_NOT_MET`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`：camera-fit 选中相机 `354caf27…f95788`，reference-compare 相机为 `8cd20605…a535`，判定 `MISMATCH`。最新真实 Codex receipt 已将当前 cohort packaged Viewer CLI read-model 精确绑定到同一 project/candidate/artifact/reference/RenderSet/comparison，但 UI/accessibility、image-block consumption、真实人评和 packaged E2E 仍未运行；attempt35 不是 best 或合格 benchmark，source、raw transport、build、窗口结构或 AX smoke 也不构成视觉、人评或 packaged UI PASS。Agentic observe/plan receipt、durable session/checkpoint/RepairIntent receipt、bounded stage batch、composition merge-chain、Repair apply prepare/confirm fixture-runtime transaction、CADFit review-candidate continuation、统一 objective 的 proposal-only OptimizationJob、新的跨视图 boundary test 和完整覆盖 synthetic promotion/replay fixture 只证明各自隔离 source/readback/transaction slice，不改写上述质量事实。

运行时间线以 `docs/evidence/mcp010f/real-codex-run-inventory.json` 为准：attempt5 是历史 CameraCalibrationRef 里程碑；最新完成传输与最新尝试均为 `docs/evidence/mcp010f/real-codex-cli-current-20260815-threshold-bound-v4.json`，使用 v4 构建时的 source cohort `95af4cd10b4b9e5003ce24f0182935db4b7965b19897ca368908d6ae538667dd`，状态仍为 `PASS_WITH_QUALITY_TARGET_NOT_MET`，未晋升为 benchmark。随后 b367 cohort 只用于 Boundary F1 容差修正后的 CADFit/MCP010F 回归；真实参考 CADFit 的 38 次评估、proposal visual compare 与重启 hash 读回见 `docs/evidence/mcp010f/optimization-job-real-reference-20260815-metric-aligned.json`，仍未 confirm/version/export。

<!-- forgecad-stage0: schemas=123 schema_set_sha256=583fd0d2615f09e66d16c58fca8d4ab60f1856d1de427b5b9e390c8c8b137f67 read_tools=40 write_tools=30 total_tools=70 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260815-b37-complete-auto-v3.json latest_completed=real-codex-cli-current-20260815-b37-complete-auto-v3.json -->

2026-08-15 最新真实 Codex transport（未晋升 provisional benchmark）：`docs/evidence/mcp010f/real-codex-cli-current-20260815-threshold-bound-v4.json` 使用 v4 构建时的 source cohort `95af4cd10b4b9e5003ce24f0182935db4b7965b19897ca368908d6ae538667dd`，10 个 Codex turn 全部 exit code 0；26 parts/4704 triangles 的 artifact/readback 通过，silhouette-fit→reference-compare 相机绑定通过，9 个 AOV 的 typed image block/readback 通过。比较结果 IoU `0.746740`、Boundary F1 `0.342841`，仍为 `QUALITY_TARGET_NOT_MET`、typed review `needs_revision`；这只更新 latest transport，不改写 attempt35 的 `MISMATCH + BLOCKED_INCOMPLETE_BINDING` provisional 事实，也不证明 likeness/high-quality。b367 cohort 后续只用于 Boundary F1 容差修正后的 CADFit/MCP010F 回归；人评、PBR likeness、confirm/export、restart hash、packaged accessibility 和 360 仍未运行或受阻。

2026-08-15 CADFit 相机绑定事实：`docs/evidence/mcp010f/optimization-job-real-reference-20260815-v8.json` 在真实授权参考上执行 `camera_fit_prepare → silhouette_fit_prepare → OptimizationJob`，将初始相机 `8cd20605…a535` 绑定到 refined camera `27d180d2…c0c`，receipt 明确为 `PASS_SILHOUETTE_FIT_TO_OPTIMIZATION`。38 次多保真评估全部完成，但严格多目标改善为 false，Runtime 返回 `blocked-no-improvement`，未创建 proposal、未 confirm、version count 0；这是 CADFit 相机交接和拒绝边界的 transport 证据，不改变 Stage 0 视觉真值或 likeness 状态。

## 1. 真值层级

1. **Runtime V1 SQLite + CAS**：项目、候选、版本、Job、Skill、审批和工件唯一持久真值；
2. **公开 JSON Schema + canonical serialization**：对象合法性和 hash 规则；
3. **`ActiveDesignSnapshot`**：当前项目状态的单一只读投影；
4. **Worker receipts/readback**：对特定输入和工件 hash 的事实；
5. **Render/Quality evidence**：对特定 candidate/version 的检查；
6. **MCP/Viewer projection**：可丢弃、可重建的展示；
7. **Codex 对话/自然语言**：意图与解释，不是产品状态。

GLB、图片、`.blend`、Three.js scene、prompt、Skill 文档和 Codex 评价都不能单独成为版本头。

MVP 具体规则：Reference truth 是 CAS 原始字节 + `ReferenceEvidence`，不是本机路径；Geometry truth 是 canonical `GeometryProgram` + worker receipt + mesh/GLB readback，不是 `.blend` 或 Viewer scene；Appearance truth 是 typed MaterialZone/AppearanceProgram；Render/Quality 只证明同一 candidate hash；导出是 confirmed version 的派生物，不反向成为版本头。

### 1.0.1 Agentic Design Runtime 目标真值

ADR-0026 引入的目标对象不改变现有真值层级。当前 observe/plan projection 由 Runtime 按需从现有 read model 派生，可丢弃、可重建；durable session/checkpoint slice 则由 Runtime 经过审批校验后写入 SQLite/CAS。两者仍不能替代 candidate/version 的唯一写者边界，也不把下列目标能力伪装成完整实现：

- `DesignSession@1` 当前已能在同一 project/candidate/reference/evidence lineage 下持久化 stage、失败门、下一步允许动作和 checkpoint 指针；Runtime 已有 bounded single-action geometry ActionRun，但候选晋级/confirm/version 变更仍必须另行落到 Runtime candidate/version/job；
- `DesignCheckpoint@1` 当前已能持久化不可变失败/阶段检查点并在重启后读回；`checkpoint_restore_prepare` 只生成 CAS-bound `RepairIntent@1`，不执行 Repair、不改写历史；
- `SemanticSceneGraph@1` / `ModelUnderstandingBundle@1` 未来是只读理解投影，必须由 Runtime candidate、readback、RenderSet、QualityReport 和 source map 派生；
- `ReferenceCanvas@1` / `DesignSpec@1` 当前可由授权 `ReferenceEvidence` 生成保守默认对象，或由受限 `authoring_context` 提供显式 hash-bound view/spec；覆盖不足和 unknown 会明确阻断；跨视图渲染评估与完整 producer/consumer conformance 仍未完成；
- Critic/Repair report 未来只记录 evidence-bound issue 和 bounded intent，不能跳过 compile/readback/render/compare；
- Parametric Design Kit 未来必须展开为 typed Geometry/Appearance contracts，不允许成为任意脚本或第二几何真值。

上述 durable 对象目前只在受限 prepare/readback receipt 的范围内计入当前能力账本；有界 stage batch 仍只执行独立 ActionRun 并在首个质量门停止；`design_composition_prepare` 的确定性 positive fixture 已生成独立 review candidate，但不代表真实参考上的完整多动作 orchestrator；`repair_apply_prepare` 也只生成 CAS apply intent；Repair 实际应用、完整 Visual Evidence conformance、packaged same-cohort 与视觉质量门仍不得宣称已实现。

### 1.1 MCP010 当前与目标真值

2026-08-14 当前真值修订：manifest/目录计数已为 115 个 JSON Schema；最新真实 Codex receipt `docs/evidence/mcp010f/real-codex-cli-current-20260814-viewer-bound.json` 已在 cohort `d11e83cc…07264` 下完成 packaged Viewer CLI read-model 的 exact project/candidate/artifact/reference/render-set/comparison lineage binding。该 receipt 仍为 `QUALITY_TARGET_NOT_MET`、`quality_hard_gate_passed=false`，且 sanitized Codex events 未观察到 image-block consumption；UI/accessibility、真人评审、PBR likeness、export/restart hash 和 360°仍 `NOT_RUN/BLOCKED`。该绑定只补齐读模型 lineage，不把 attempt35 或当前视觉结果晋升为 likeness/high-quality PASS。

MCP010B 当前源码增加 8 个合同，MCP010C 再增加 7 个合同，MCP010E 再增加 6 个合同，MCP010F 轮廓求解器新增 12 个合同（含 `CameraCalibrationRef@1`）及其余 CameraCalibration/target binding 合同，Agentic contract family 与 D 的 Boolean request/result 继续补充合同，当前共 115 个 JSON Schema（MCP006 历史为 44）。B 的 `GeometryProgram@2`/strict readback/restore evidence source Gate 已通过；Darwin 512 MiB OS memory hard cap 仍 deferred/NOT_RUN。C 的 reference/renderer/review 合同、D 的 bounded Boolean 合同/Worker、E 的 AssetPack/Appearance V2 合同与 F 的 silhouette target/camera/Rig/SDF/Part/candidate compare 合同已由 Runtime/MCP producer/consumer 使用；Agentic projection 与 durable session/checkpoint/RepairIntent prepare/readback、composition merge-chain validation、Repair apply prepare 也已通过合同/隔离重启 Gate；固定 512×512 perspective/z-buffer renderer、九 AOV、local mask/metrics、MCP image block、Codex/human review、离线 AssetPack、512px UV atlas、固定 mikktspace、embedded PBR texture、哈希绑定轮廓目标、扩展相机搜索、受限 Rig/SDF fit 和候选比较的 source raw Gate 已通过。历史真实 Codex CLI C receipt 已完成六 turn/32-call transport；带 15 landmark/8 region intake 的 source-built silhouette-first attempt35 已完成 11-turn detail reference→mask/target→camera/Rig/fit→compare→boundary→九 AOV→typed review/quality transport，但只保留为 provisional retained observation，结果为 `QUALITY_TARGET_NOT_MET`（26 Parts/4704 triangles，IoU `0.741047`、boundary F1 `0.328765`、bbox edge error `0.007813`、landmark coverage `0.733333`、region median `0.869403`），不是 likeness PASS；其 camera-fit `354caf27…f95788` 与 reference-compare `8cd20605…a535` 为 `MISMATCH`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`。Runtime fit proposal 为 `no_improvement`、IoU `0.698340`，仍是 read-only 建议，没有新 candidate mutation/confirm。attempt33/34 的完整相机 payload hash 漂移失败保留为负向证据；`CameraCalibrationRef@1` 已消除跨轮次浮点复制阻断，但不补齐 attempt35 的 compare 真值绑定。当前单 Part proposal 已改为使用选中 Part 的本地边界宽高和质心误差，而不是全身包围盒，且通过独立单测；这只改善编排信号，不证明新模型质量。脱敏证据见 `docs/evidence/mcp010f/real-codex-cli-silhouette-first-20260813-attempt35-detail-camera-ref.json`；same-observation packaged Viewer/human/PBR/export/360 仍未运行。MCP010A/010B 的历史 Dev.app receipts仍原样保留，不能替代 C/D/E/F packaged/live/Viewer evidence。C/E/F synthetic/raw/CLI transport 与 Agentic prepare/readback receipt 都不证明用户机器人 likeness、PBR likeness、独立人评、Repair execution 或 360°。

历史 CameraCalibrationRef 里程碑 `docs/evidence/mcp010f/real-codex-cli-camera-ref-20260813-attempt5.json` 使用同一授权 PNG、同 cohort `e968c9ef…6980`、26 Parts/4704 triangles、九 AOV 和 typed review，Runtime 通过 `camera_hash + canonical_sha256 + target_sha256` 解析精确相机。其 comparison 仍为 `QUALITY_TARGET_NOT_MET`（IoU `0.698465`、boundary F1 `0.281074`、bbox edge error `0.037109`、centroid `0.049908`、landmark coverage `0.666667`、landmark NME `0.201432`、region median `0.771619`、critical region min `0.675106`）。attempt5 不是当前最新完成传输，也不是 benchmark，不解锁材质、confirm/export、human review 或 360；用户持久数据未改变。

`AppearanceProgram@2`、PBR V2/纹理、Viewer 的 Part/MaterialZone/selection/explosion/heatmap、packaged E2E 和 first-party AssetPack 已由 MCP010E/F source Gate 部分落地。C 的 RenderSet@2/比较/评审已进入 Runtime-owned producer、CAS artifact、严格 readback、固定 render、QualityReport 和 evidence lineage；E 的 AssetPack/manifest/provenance、embedded textures、UV/tangent 和九 AOV 也已进入同一 Runtime/Worker source path；F 的 Viewer compare source surface 已通过本地 IPC/构建测试，但不等于 packaged/live Viewer Gate。当前 Runtime Skill registry 的 11 项中，`primitive-blockout@0.2.0`、`hard-surface-detail@0.2.0` 与 `uv-pbr@0.2.0` 在真实 consumer 和 immutable bundle 校验后 active；这不产生视觉 likeness 或 360°结论。Darwin 512 MiB OS memory hard cap、人评阈值、Viewer/package/live C/D/E/F 和 360 更不能由结构/PBR source PASS 推导。

最新 `d9c23b…ac0bd` 开发包记录了 Bundle 知识分支的校正：`limited` 只阻断视觉质量声明，`STRUCTURAL_BLOCKOUT` 仍需用户明确选择并经过相同 Runtime geometry/readback/approval 硬门。该包的 isolated raw/real-Codex V2 structural 通过；用户完整重启后它已成为当前 live Desktop cohort，并完成 32 工具、Ready/doctor、cohort/catalog/hash 与项目只读回读结构激活。

2026-08-13 的 F 轮廓优先增量已通过隔离 source transport probe：真实 Viewer `chest-shell` 草图被绑定为局部 target，`part_contour_fit_prepare` 生成建议，四个有界单 Part 变体由固定比较器筛选。最高 IoU 为 `0.745895`（provisional observation 对照值 `0.741047`），loss winner 为 IoU `0.745135`、Boundary F1 `0.340045`；两者都未达到严格 `0.90` 轮廓门，也未创建 candidate version/export。该证据只证明轻量纠偏编排和 bounded candidate 选择，不能产生合格 benchmark 或改写当前机器人 `QUALITY_TARGET_NOT_MET`；fresh source binary 的 build identity 为 null，故不宣称 cohort PASS。
Runtime 的 `silhouette_fit_prepare` 现在会在有 typed Part slice 时做一次选定相机 Part-ID readback，并对匹配参数使用局部 target/model envelope 与 centroid proposal；无 slice 时保持全身 fallback。合成局部优先单测、完整 Runtime 和新的 source receipt 均通过，但它仍只是 bounded reviewable proposal，不放宽质量门或确认/导出边界。

目标 `HumanVisualReviewReceipt` 只证明用户评分绑定到特定 reference/camera/render/candidate hash，不证明模型身份，也不能覆盖 Geometry/UV/PBR 硬门。当前单张参考只能产生 `PARTIAL_VISIBLE_VIEW_PASS`；`HQ_360_PASS` 在多视图完整前固定 blocked。

## 2. 核心对象

### Project

包含 project ID、名称、创建时间、policy/profile、active snapshot revision。无绝对本机路径和模型/Provider 信息。

### ReferenceEvidence

保存 CAS hash、MIME、尺寸、用户授权声明、导入方式、视图/相机 claims 和派生证据 lineage。原始绝对附件路径入 CAS 后丢弃。

### Candidate

未确认、可 GC 的完整构建单元，引用：base version、SubjectProfile、RepresentationPlan、AssemblyGraph、Geometry/Appearance programs、Skill receipts、artifacts/readback、RenderSet、QualityReport、SemanticChangeSet 和状态。

Candidate 状态：`prepared → compiling → evaluating → reviewable → confirmed | rejected | failed | expired`。只有 `reviewable` 且 hard gates 通过者可 confirm。

### DesignAssetVersion

不可变提交，至少包含：version ID、project、parent version、confirmed candidate hash、assembly/program/material/texture/artifact manifests、quality、approval、created_at 和 canonical digest。任何修改都创建新子版本。

### ActiveDesignSnapshot

```text
project_id
snapshot_revision
confirmed_version_id | null
review_candidate_id | null
runtime_capabilities_digest
selection_projection_revision | null
updated_at
```

Snapshot 不复制完整模型，不合并两套 `vN`，不按导出格式切换版本链。Viewer/localStorage 不能写它。

### RuntimeJob

持久 job ID、kind、project/candidate scope、request hash、state、event cursor、checkpoint/result/error refs 和取消状态。事件只追加、可重放；大内容只引用 CAS。

### SkillExecutionReceipt

绑定 Bundle/Recipe/Operator/Validator/asset/SBOM/signature hash、canonical input/output、预算和结果。不记录模型 prompt 或任意执行环境。

### ApprovalReceipt

由 Runtime 接收 Codex write approval 后创建，绑定 user-visible summary、tool、project、base version、prepared object ID/hash、quality report、expiry、decision 和 session。它不证明模型身份，只证明本地审批事务。

## 3. 写入不变量

- 只有 Runtime 进程持有 SQLite/CAS 写权限；启动时先取得 OS 独占 writer 文件锁，第二实例返回 `RUNTIME_BUSY`；
- `prepare` 不移动 confirmed head；
- `confirm` 在单一 SQLite 事务中校验 base/hash/quality/approval/idempotency、写版本、更新 snapshot、追加 audit；
- 同一 idempotency key + request hash 返回同一结果；同 key 不同 hash 拒绝；
- stale base 不自动 rebase 或 last-write-wins；
- rejected/failed/expired candidate 永不确认；
- 质量报告只能附着其 input artifact/candidate hash；
- export 只能引用 confirmed version，或明确标记为 unconfirmed diagnostic；
- CAS 对象以内容 hash 寻址，DB 事务提交前验证存在/尺寸/hash；
- GC 只能删除无 reachability 的临时候选工件，不能删除已确认版本、审批、audit 或其依赖。

## 4. 局部修改

`SemanticChangeSet` 必须引用 base version、Part/MaterialZone/source-map 稳定 ID 和 allowlisted operation。Runtime 校验 scope 后生成新 candidate，重新编译受影响 DAG 并复用未影响 hash。不能接受任意 JSON pointer、vertex buffer patch、脚本或路径。

Viewer selection 只是提示；prepare 时必须重新绑定当前 snapshot/part。Part 已不存在或版本漂移时返回 typed conflict。

## 5. Undo、Reject、Restore

- `undo/redo`：只作用于同一未确认 candidate 的 typed change stack；
- `reject`：终止 candidate，不改 confirmed version；
- `restore_prepare(version_id)`：从历史内容产生基于当前头的新 candidate；
- `restore_confirm`：批准后创建当前头的子版本，历史版本保持不变；
- 禁止移动数据库指针伪装新版本，禁止覆盖旧 GLB/CAS 对象。

## 6. 爆炸图

默认爆炸图是由 confirmed AssemblyGraph 派生的 `ExplodedViewPlan`。临时距离只存在 Viewer；保存计划必须产生 candidate/change/approval/version。Plan 引用稳定 Part ID，不能以渲染 primitive 顺序作为唯一身份。

## 7. 导出

`export_prepare` 生成 manifest 与 CAS-backed artifact reference，绑定 confirmed version、format/profile、artifact hashes、validator/readback、license/provenance 和 toolchain。MVP `glb/mvp-glb` 的 `export_confirm` 原子确认 receipt 并返回 `output_sha256`，不写任意本机路径；filesystem/package target 属 MCP013。导出目录不得成为版本真值。

如果 Viewer、candidate、quality、export 的 version/hash 不一致，导出 fail closed。导出包不包含绝对本机路径、secret、prompt、原始 Codex attachment path 或未授权资产。

## 8. 旧数据

旧 `ConceptVersion`、`ModuleGraph`、`AgentAssetVersion`、Thread/Turn/Item、Provider 和 migrations 仅属于只读归档。新 Runtime V1 不自动打开旧 DB，也不把旧 `vN` 投影为当前 snapshot。

一次性离线工具可以读取备份、校验旧工件、生成中立 export manifest，再由用户显式导入新项目。失败不修改旧库或新库。用户数据删除需要独立明确授权。

## 9. 重启与灾难恢复

重启时 Runtime：取得 OS writer 文件锁 → 验证 DB migration/version → CAS reachability → snapshot/version hashes。MVP 不使用 TTL lease、heartbeat 或 stale takeover；未完成 Job 的跨 MCP 会话恢复暂不承诺，无法安全恢复的 Job 转为 typed failure。已确认版本必须在 MCP/Viewer 不可用时仍可离线备份和校验。
