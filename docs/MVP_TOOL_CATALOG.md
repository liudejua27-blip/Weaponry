# ForgeCAD MVP 工具、Skill 与外部项目目录

版本：2026-08-13
2026-08-17 Reference Visual Structure：现有 `reference_mask_prepare`/`reference_mask_refine_prepare` 可选接受中性 `visual_structure` draft；Runtime 补齐 policy/review/canonical hash 并随 `SilhouetteTarget@1` 进入 CAS，`silhouette_target_get` 原路回读。没有新增工具，当前仍为 41 read + 33 opt-in write = 74；该注释不会直接创建 candidate 或解锁 detail/PBR。
2026-08-17 PDK v0：`geometry_program_hash` 仍是默认只读工具，但现在可接受 `ParametricDesignKitRequest@1` 并返回六类 typed macro 的 `ParametricDesignKitProgram@1`；这是 Runtime-owned structural authoring aid，未新增 write tool、Skill 执行器或外部插件加载。receipt：`docs/evidence/mcp010f/parametric-design-kit-v0-source-gate-20260817.json`。
当前 Stage 0 覆盖（2026-08-17）：138 contracts、41 read + 33 opt-in write = 74 tools；新增 `fictional-energy-rifle-profile` source-only authoring contracts 与 `repair_intent_run_prepare`，Profile 不新增 write tool、只执行 CAS-bound bounded run 并产出 staged candidate，`repair_apply_prepare`/confirm 仍独立且未完成。
状态：MVP 功能核心目录；当前源码为 138 个 contracts、41 read + 33 opt-in write = 74 个工具、12 个 Skill（必须先读 `ponytail-preflight@0.1.0`，以及历史 Bundle + `primitive-blockout@0.2.0`、`hard-surface-detail@0.2.0`、`uv-pbr@0.2.0`）；唯一 `in_progress` 为 `FGC-MCP010F`。MCP010C source-focused fixed renderer/九 AOV/reference compare/typed visual review、MCP010D hard-surface Operator/Skill、MCP010E 离线 AssetPack/UV/PBR/MikkTSpace、MCP010F Viewer 与 contour-first Runtime target/Rig/SDF/Part/candidate compare source slice（含 `CameraCalibrationRef@1` 和 `silhouette_part_error_get`）已通过各自范围；新增 Fictional Energy Rifle Profile/Plan 仅提供 nonfunctional structural authoring aid，不代表视觉通过；新增 `primary_form_repair_prepare` 将一次 Primary Form fit→compile→readback→render→compare 收口为 Runtime-owned staged prepare/evaluate；新增 `primary_form_repair_job_prepare` 将可能超过单次 IPC deadline 的长搜索异步化，并由 `job_get`/`job_events_read`/`job_result_get` 回读终态 CAS 结果；新增 `RuntimeJobResult@1` 约束该 CAS 结果外层 envelope；新增 `design_action_run_prepare`/`design_action_run_get` 的窄范围单 Part `primary-form` action-run/readback 与 `repair_intent_run_prepare` 的 CAS-bound staged run 也已通过 focused source/package transport Gate。Agentic observe/plan projection 与 durable DesignSession/Checkpoint/RepairIntent prepare/readback 也有隔离 source/transport/restart receipt，真实 Runtime 的嵌套只读 projection producer/consumer conformance 已通过独立回执。packaged Viewer 也已有 CLI read-model、原生窗口和核心控件 smoke，但同一 provisional observation 的 package binding、PBR likeness、正式 VoiceOver、真人评审和 360 仍 `NOT_RUN/BLOCKED`；durable/reference/DesignSpec 完整 producer、通用单动作 orchestrator 和 Repair 应用仍未完成。

本文是 Luna 执行 Goal 时的“能调用什么、何时调用、什么不能声称”的单一索引。它不是新的运行时配置，也不允许绕过 MCP 合同。工具实现仍以 Rust source 和 JSON Schema 为权威；本文只提供可读的路线图和验收边界。

Stage 0 机器真值读取 `docs/evidence/mcp010f/current-benchmark-truth.json`：attempt35 只是 provisional retained observation，候选状态是 `QUALITY_TARGET_NOT_MET`，证据完整性是 `INCOMPLETE_TRUTH_BINDING`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，fit/compare camera 为 `MISMATCH`，packaged Viewer 为不同 cohort/artifact，尚未绑定该 observation。工具或 Viewer 已实现不等于这些缺口已通过，也不能提升 human/PBR/export-restart/360 状态。

<!-- forgecad-stage0: schemas=147 schema_set_sha256=8b292d3ea73475b9ad6c8ebe1eb6521d3dd8834a9410441f113524cef79d4759 read_tools=41 write_tools=33 total_tools=74 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260815-b37-complete-auto-v3.json latest_completed=real-codex-cli-current-20260815-b37-complete-auto-v3.json -->

## 1. MVP 运行边界

```text
Codex Desktop / CLI
        │ MCP stdio
        ▼
forgecad-mcp  ── authenticated local IPC ── forgecad-runtime
                                                │
                                                ├─ SQLite V1 + CAS（唯一写者）
                                                └─ bounded typed geometry/appearance/render
        ▲
        │ read-only IPC
ForgeCAD Viewer（可选）
```

- 当前源码的默认连接暴露 41 个只读工具；只有 authenticated IPC、Runtime handoff 和 `FORGECAD_MCP_ENABLE_MCP004_WRITES=1` 同时满足时，才暴露完整 74 个工具（41 read + 33 opt-in write）。其中 `operator_catalog_get`、`geometry_program_hash`、`silhouette_rig_hash`、`material_pack_get`、`render_pass_get`、`silhouette_target_get`、`camera_fit_prepare`、`silhouette_fit_prepare`、`part_contour_fit_prepare`、`silhouette_part_error_get`、`silhouette_candidate_compare`、`boundary_error_get`、`session_get`、`checkpoint_get`、`job_result_get` 和 Agentic 的 `scene_observe_get`、`design_stage_plan_get`、`critic_report_get`、`visual_evidence_bundle_get` 是 Runtime-owned 只读工具；`reference_mask_prepare`/`reference_mask_refine_prepare`/`primary_form_repair_prepare`/`primary_form_repair_job_prepare`/`repair_intent_run_prepare`、`session_create_or_resume`、`checkpoint_prepare`、`checkpoint_restore_prepare` 需要显式 write opt-in。C source raw receipt 证明九 AOV、comparison、review 和 image block 的绑定链，不证明用户图片 likeness；E raw receipt 另证明 AssetPack、嵌入纹理、UV/tangent readback 和 PBR lowering；F contour-first slice 证明 target/camera/Rig/Rig-hash/SDF/Part/candidate boundary source dispatch；Agentic receipts证明 observe/plan projection、durable session/checkpoint/RepairIntent prepare/readback 与 CAS-bound staged RepairIntent run 的隔离 transport，不证明 orchestrator、Repair 应用或机器人 likeness。MCP010A/010B 的 Dev.app receipts继续按历史结构证据保留。
- Codex 可在临时目录调用 `scripts/make_mcp010f_comparison_sheet.py`，把同一参考图、`beauty`、`silhouette` 和一个诊断 AOV 打包成固定 2×2 review sheet。它只做标准库 PNG 重采样/哈希清单，不评分、不写 Runtime/CAS；原图字节不得进入仓库或 evidence，`QualityReport@2` 仍是唯一质量真值。
- Codex 也可在临时目录调用 `scripts/build_mcp010f_fit_plan.py`，把已绑定的 comparison/view/catalog JSON 转成最多五轮、按 `reference-canvas → silhouette-blockout → landmark-structure → semantic-part-fill → surface-detail → uv-pbr → final` 门控的单部件修正队列。轮廓门未通过时只返回 silhouette 动作，并锁定后续 landmark/form/material；它只验证输入 hash 和整理 metric/landmark/region 证据，并为已知 region 输出一个 `primary_part_id`、只读 supporting Parts、material-zone hints 和按 Part 分组的 Operator hints；未知 region 不会被猜成部件。它不生成 GeometryProgram、不调用 Operator、不写 Runtime/CAS；缺少 live OperatorCatalog 时不会伪造可执行提示。
- 本机 Codex 另提供 `forgecad-material-surface-design` 编排 Skill，专门把 live AssetPack、MaterialZone、profile/panel/vent/joint/sweep 线条、UV/PBR 通道和九 AOV 复核串成一条短路径。它不是 Runtime Skill Bundle，不改变 `skill_list` 的产品真值，也不安装第三方插件；缺失 AssetPack 或 `AppearanceProgram@2` 时必须报告 `MATERIAL_ROUTE_UNAVAILABLE`。
- 最新安装的 `d9c23b…ac0bd` package 在 Skill 知识分支修正后通过隔离 raw/real-Codex V2 structural Gate；用户完整 Desktop 重启后已成为 live Skill overlay，当前 live cohort为 d9。
- `forgecad-mcp` 不打开 SQLite/CAS，不执行模型调用，不接受任意 Python、JavaScript、shell、URL 或未授权路径。
- Runtime 启动前取得 `runtime.writer.lock`；MVP 不使用 TTL lease、heartbeat、broker、远程 transport 或插件市场。
- Viewer 只读 Runtime projection；关闭 Viewer 不删除已确认数据，但 MVP 不承诺 Codex 断线后未完成 Job 继续。
- `functional-core PASS` 只证明 focused 本地实现；当前已有真实 Codex CLI 十二调用 host golden-path receipt。真人视觉评分、外部分发和签名仍必须有独立 receipt。

### 1.1 Agentic Design Runtime projection 与 durable prepare（Phase 1）

以下四个工具已进入当前 source manifest。它们只读 Runtime 现有证据，返回可重建 projection；不创建 candidate/version/job，也不替代 durable producer。隔离证据：`docs/evidence/mcp010f/agentic-runtime-observe-plan-20260813.json`。

以下 durable 工具也已进入当前 MCP manifest；action-run 工具目前只覆盖 Primary Form：

| 目标工具 | 类型 | 预期行为 |
|---|---|---|
| `scene_observe_get` | read | 返回 Runtime-owned semantic scene/understanding/reference/quality projection；字段明确区分 observed/inferred/unknown |
| `visual_evidence_bundle_get` | read | 读取现有 candidate-bound Viewer evidence；缺失或跨 candidate evidence fail closed，不创建 render |
| `design_stage_plan_get` | read | 根据现有 evidence 返回 current stage、失败门、允许动作和 blocked actions，不推进 stage |
| `critic_report_get` | read | 返回 evidence-bound critic projection 和 bounded repair suggestion；不执行 RepairIntent |
| `session_get` | read | 按 project/session/candidate binding 读取 Runtime 持久化 `DesignSession@1` |
| `checkpoint_get` | read | 读取不可变 `DesignCheckpoint@1` 及 session/checkpoint hash binding |
| `session_create_or_resume` | write/approval | 创建或恢复同一 reference/candidate/evidence lineage 的 session；需要显式 opt-in |
| `checkpoint_prepare` | write/approval | 保存阶段/失败检查点；只接受已观察 evidence，确认状态仍由既有事务控制 |
| `checkpoint_restore_prepare` | write/approval | 只生成 CAS-bound `RepairIntent@1`；不修改 candidate/version/history |
| `design_action_run_get` | read | 回读 Runtime-owned、candidate/session/reference/observation-bound `DesignActionRun@1`；包含本回合 `observation_sha256`，不重算质量、不推进 stage |
| `design_action_run_prepare` | write/approval | 只接受一个已批准、单 Part、`primary-form` bounded action；复用 Runtime compile/readback/render/evaluate，锁定 confirm/export，不修改 candidate/version/user data |

这些工具已有公开 Schema、negative tests、Runtime producer 和隔离 Viewer/Runtime evidence；真实 Runtime 的 scene/stage 嵌套只读 projection 已通过 `scripts/check_agentic_projection_receipt.py`，durable 工具与 Primary Form action-run 仍只覆盖 prepare/readback。durable/reference/DesignSpec 完整 producer、通用单动作 design orchestrator、Repair 实际应用、完整 Visual Evidence contract conformance 和 real Codex quality loop 仍未实现。Codex 仍必须先读取 `ponytail-preflight@0.1.0`，并在写工具前提交显式 approval。

## 2. 只读工具（默认可见，37 个）

| 工具 | 用途 | 当前 MVP 证据/限制 |
|---|---|---|
| `artifact_readback_get` | 读取候选绑定的 GLB header、Part、triangle、UV/tangent、PBR readback | MCP007/008 focused PASS；不返回任意文件路径 |
| `candidate_get` | 读取 candidate、hash、Job、quality 摘要 | 只读；未确认 candidate 可回收 |
| `capabilities_get` | 读取 Runtime/MCP/Worker/Skill 能力和 limitation | 必须在写入前调用；不以空字段伪装能力 |
| `doctor` | 读取 bounded health/contract/lock 诊断 | 不启动 fixture、不 confirm、不签名 |
| `geometry_program_hash` | 校验无 hash 的 `GeometryProgram@2` draft，返回 compiler-owned canonical hash | 不编译、不创建 candidate/Job、不写 SQLite/CAS；draft 不能预填 hash |
| `silhouette_rig_hash` | 校验候选绑定的无 hash `SilhouetteRig@1` draft，返回 Runtime-owned canonical hash | 只读、零持久化副作用；Codex 不应在本地重算 Rig canonical JSON |
| `silhouette_target_get` | 读取 `SilhouetteTarget@1` 及 reference/mask hash | 只读 CAS；不会返回原图路径/字节，也不会让 Viewer 拥有第二套 mask 真值 |
| `camera_fit_prepare` | 对现有 candidate 运行 37 个覆盖 yaw/pitch/FOV/distance/roll/target-offset/global-scale 的粗候选 + 前三名各 9 个局部探针 | 只返回 `CameraFitResult@1`，总预算不超过 64 次真实渲染；不修改 candidate、不创建版本；只接受真实渲染后有改善的 camera |
| `silhouette_fit_prepare` | 对 `SilhouetteRig@1` 运行有界 camera/参数搜索 | 最多 64 次 128×128 transient batch、8 次迭代并归一化到 512×512 指标；返回 SDF/Chamfer、阈值和 bounded proposal，不修改 candidate |
| `part_contour_fit_prepare` | 针对单一 semantic Part 计算局部轮廓 proposal | 读取同一 candidate 的 part-ID/RenderSet，只返回 bounded adjustment；不写 GeometryProgram/CAS |
| `silhouette_candidate_compare` | 在同一 target 下比较 2–8 个 candidate | 返回 candidate-bound metrics、loss、winner/tie；拒绝跨项目、重复或未绑定候选 |
| `boundary_error_get` | 读取 target 与 candidate RenderSet，返回方向化边界误差段 | 最多 64 段；径向 inward/outward 是诊断近似，必须由下一轮 compare 验证 |
| `silhouette_part_error_get` | 返回每个声明 Part 的局部 envelope、质心/宽高比、边界误差和推荐修正 Part | 只读 hash-bound 多 Part 归因；不创建 candidate/version；缺失 Part 或 unknown slice 必须 fail closed |
| `job_events_read` | 读取 durable Job events | MVP 支持读取/取消；checkpoint 续跑属 MCP011 |
| `job_get` | 读取 Job 状态 | 非终态重启可转 typed failure |
| `job_result_get` | 读取已完成 Job 的 CAS-backed 结果 | 仅返回终态 event 绑定的 JSON；排队/运行中返回 `JOB_RESULT_PENDING`；不恢复跨重启执行 |
| `operator_catalog_get` | 读取 Runtime-owned `OperatorCatalog@1` | 返回值必须与 `forgecad://operators/catalog`、capability 和 V2 artifact/readback digest 一致；不是第二套 catalog 真值 |
| `project_get` | 读取项目元数据和 head | 不创建项目 |
| `project_list` | 列出当前 Runtime 项目 | 不读取旧 Library |
| `reference_get` | 读取 ReferenceEvidence hash/MIME/尺寸/授权 | 不返回原始绝对路径或图片字节 |
| `quality_get` | 读取 Runtime-owned quality report | 可读取 candidate-bound `QualityReport@2`；attempt35 为 `QUALITY_TARGET_NOT_MET`，不能 confirm/export |
| `selection_get` | 读取 Viewer 临时 selection | ephemeral，不是版本真值；当前可为 unavailable |
| `runtime_status` | 读取 Runtime 生命周期 | `Starting/Ready/Degraded/Restarting/Busy` 只做状态投影 |
| `skill_get` | 读取 first-party Skill manifest 与完整 checked-in knowledge | 首次设计调用必须为 `ponytail-preflight@0.1.0`；未满足时其他 tool/Skill 返回 `PONYTAIL_PREFLIGHT_REQUIRED`；不等于结果质量 |
| `skill_list` | 列出当前 12 个 first-party Skill | 先读取 `ponytail-preflight@0.1.0`；`primitive-blockout@0.2.0`、`hard-surface-detail@0.2.0`、`uv-pbr@0.2.0` 有 active consumer；不安装第三方 Bundle |
| `snapshot_get` | 读取 `ActiveDesignSnapshot` | 单一当前投影，不复制资产状态 |
| `version_diff` | 读取两个不可变版本的结构化差异 | MCP009 focused PASS；不提供通用 mesh diff |
| `version_list` | 列出项目版本 DAG | 历史不可变；restore 创建新子版本 |

只读工具必须带 `readOnlyHint=true`、`destructiveHint=false`、`idempotentHint=true`、`openWorldHint=false`。如果 Runtime 不可用，调用返回 `RUNTIME_UNAVAILABLE`；Runtime 已连接但拒绝请求时返回 `INVALID_INPUT`、`STORE_ERROR`、`RUNTIME_BUSY` 等 typed code。不能因为 stdio initialize 成功就声称 Runtime ready。

## 3. 写工具（显式 opt-in 可见，18 个）

### MCP004：事务基座（9 个）

| 工具 | 用途 | 永久版本 |
|---|---|---|
| `project_create` | 创建项目元数据 | 是项目记录，但不创建资产版本 |
| `candidate_prepare` | 准备 diagnostic 或已入 CAS 的 typed candidate | 否 |
| `candidate_confirm` | 对已批准 candidate 创建版本 | 是；hash/head/quality/approval/idempotency 必须重新校验 |
| `candidate_reject` | 拒绝 candidate | 否 |
| `restore_prepare` | 以历史 confirmed version 为内容准备新 candidate | 否 |
| `restore_confirm` | 确认 restore candidate | 是新子版本，不改写历史 |
| `export_prepare` | 准备 path-free manifest 或 `glb/mvp-glb` | 否 |
| `export_confirm` | 确认导出并生成 CAS receipt | 不写任意本机路径；返回 `output_sha256` |
| `job_cancel` | 请求取消 Job | 否 |

### MCP005–MCP009：3D vertical slice（各 1 个）

| 工具 | 任务 | 当前行为 |
|---|---|---|
| `reference_import` | MCP005 | 仅 PNG/JPEG；真实字节经授权 root/inline admission 进入 CAS，返回 `ReferenceEvidence@1` |
| `geometry_prepare` | MCP007 + MCP010B | `[transition-v1]` 保留 canonical `GeometryProgram@1` primitive-only 兼容链；当前 high-quality 路径接受已由 `geometry_program_hash` 补齐 hash 的 `GeometryProgram@2` detail program。V2 必须 project-bound、catalog-bound，输出 `ArtifactReadback@2` |
| `appearance_prepare` | MCP008 + MCP010E | `[transition-v1]` `AppearanceProgram@1` 只输出 bounded UV/tangent/PBR MaterialZone 和四个 fixed pass；当前 `AppearanceProgram@2` 绑定离线 AssetPack，并进入九 AOV strict compare，但须等待轮廓/结构门解锁 |
| `change_prepare` | MCP009 | 需要当前 `base_version_id`、稳定 `part_id`、allowlisted operation 和 typed programs；生成新 candidate，不改历史 |

所有写工具都声明 `readOnlyHint=false`。需要用户批准的 `candidate_confirm`、`restore_confirm`、`export_confirm` 以及由写流程生成的永久版本都必须绑定 approval context；MVP receipt 是宿主流程证据，不是密码学人类签名。

### 3.1 MCP010C 当前工具（source-focused 已实现；provisional observation 不具备 benchmark 资格，packaged Viewer 绑定未通过）

| 工具 | 目标类型 | 目标行为 |
|---|---|---|
| `render_pass_get` | read | 返回已经持久化、hash-bound 的真实 PNG image block，不隐式生成 render |
| `reference_compare_prepare` | write/temporary | 生成 camera/mask/metrics/diff，不创建版本；synthetic 与首次真实机器人 transport PASS，但首轮 likeness target 未通过 |
| `visual_review_submit` | write/evidence | 保存绑定 pass/region/candidate hash 的 Codex typed issue |
| `human_visual_review_submit` | write/evidence + confirmation | 保存用户评分；不作为密码学身份认证；真人阈值门仍 NOT_RUN |

`quality_get` 现可读回 candidate-bound `QualityReport@2`；attempt35 返回 `QUALITY_TARGET_NOT_MET`，不得 confirm/export。当前工具数为 41 read + 33 opt-in write = 74；Agentic projection 只读、可重建，durable session/checkpoint/RepairIntent 及 `repair_intent_run_prepare` 只覆盖已记录的 prepare/readback/staged-run receipt，不替代 QualityReport 或已确认 version。Viewer source 与 packaged read-model/window/core-control smoke 已实现，但 attempt35 camera 绑定不一致，且 package 未绑定同一 provisional observation。真实 PBR likeness、正式 VoiceOver、人评、export/restart hash 和 360 仍不在 source Gate。

### 3.2 MCP010F contour-first 工具

| 工具 | 目标类型 | 目标行为 |
|---|---|---|
| `reference_mask_prepare` | write/CAS target | 用 reference 或 Codex normalized contour 创建 `SilhouetteTarget@1` 和 PNG mask；不创建 candidate/version |
| `reference_mask_refine_prepare` | write/CAS target | 基于旧 target 创建新不可变 target；旧 hash 不覆盖 |
| `primary_form_repair_job_prepare` | write/approval | 当 64-evaluation Primary Form search 可能超过一次 IPC window 时，创建 queued Runtime Job；后台仍复用同一 Geometry/Render Worker/strict readback/same-camera acceptance，不 confirm/version/export |
| `repair_intent_run_prepare` | write/approval | 读取 Runtime-owned `RepairIntent@1`，执行 candidate/session/observation/reference/camera exact-bound 的窄范围 staged run；只返回 reviewable/blocked，不 confirm/version/export |

轮廓 target 未通过前，Luna 只允许一个 contour-bearing Part 的 geometry 修正；不得跳到材质堆叠。完整调用纪律见 `docs/CODEX_SILHOUETTE_FIT_WORKFLOW.md`。

## 4. Luna 推荐调用顺序

```text
capabilities_get
→ runtime_status（Ready）
→ project_create
→ reference_import（真实用户授权附件）
→ reference_get
→ skill_list / skill_get（选择 first-party Bundle）
→ 当前高质量链：operator_catalog_get → geometry_program_hash（hash-free、project-bound `GeometryProgram@2` detail draft）→ geometry_prepare
  `[transition-v1]` 或仅为 MCP007–009 结构/导出兼容的 `GeometryProgram@1` primitive-only → geometry_prepare
→ artifact_readback_get / candidate_get
→ reference_mask_prepare（建立 hash-bound SilhouetteTarget@1；若 Viewer/用户细化则 reference_mask_refine_prepare）
→ silhouette_target_get
→ camera_fit_prepare（最多 64 个有界候选，只接受真实渲染改善）
→ silhouette_rig_hash（hash-free SilhouetteRig@1；Runtime 返回 canonical hash）
→ silhouette_fit_prepare（最多 8 轮/64 次 transient 评估；保留 best-so-far）
→ reference_compare_prepare（同一 candidate/reference/camera 的九 AOV strict compare；fit/compare camera 不一致即停止）
→ render_pass_get（按 pass 返回 MCP image block）
→ boundary_error_get（最多 64 个方向段）
→ visual_review_submit（Codex typed review）
→ quality_get
→ 轮廓/结构全部通过后 appearance_prepare(AppearanceProgram@2) → artifact_readback_get → 九 AOV strict compare
→ human_visual_review_submit（仅 strict visible-view 通过后；当前正式真人门 NOT_RUN）
→ candidate_reject（验证拒绝不写版本）
→ change_prepare（稳定 Part 的一次有界修改）
→ candidate_confirm（用户批准）
→ version_list / version_diff
→ restore_prepare → restore_confirm
→ export_prepare(format=glb, profile=mvp-glb)
→ export_confirm
```

每一步都记录 `project_id`、candidate/version/artifact hash、Job 状态、MIME/size、quality limitation 和 receipt。任何一步失败都停止写链路并记录 `FAIL`、`BLOCKED` 或 `NOT_RUN`，不要自动退回旧 Provider 或手工 GLB。

## 5. First-party Skill Bundle（当前 12 个）

Skill Bundle 是声明式 metadata + typed Recipe；Runtime 只解析已注册 Operator，Bundle 自身不携带可执行脚本。当前 Registry 为 `development-only`，每个 Bundle 均有 Schema、Recipe、operator lock、validator、fixture、benchmark receipt、LICENSE/NOTICE、SPDX SBOM、provenance 和 canonical trust manifest。

| Skill | 当前 consumer | MVP 作用 | 限制 |
|---|---|---|---|
| `ponytail-preflight@0.1.0` | MCP session adapter | 设计前的必要性/现有能力/最小 typed action 检查；`skill_get` 返回知识文本且先读才可调用其他设计工具/Skill | 无 executable operator，不生成几何或质量 PASS；上游 Ponytail package/hook/server 不安装、不执行 |
| `reference-intake` | MCP005/006 | 参考 hash/claims 边界；保留 staged detail inventory、可见/遮挡区和 unknowns | 不执行图片理解，不调用模型；Codex 负责语义判断 |
| `subject-profile` | MCP006/009 | typed subject/profile 草案、每区域 confidence 与“不确定而非猜测”记录 | 由 Codex 产生语义，Runtime 只校验范围和 hash |
| `semantic-assembly` | MCP006/007 | 稳定 Part/Assembly 图 | 不生成任意 mesh |
| `silhouette-blockout` | MCP007 | `[transition-v1]` 有界 primitive-only blockout | 只接受 box/cylinder/sphere；不是当前 high-quality detail 路径 |
| `hard-surface-detail@0.2.0` | MCP010D | profile-extrude/profile-loft/revolve/tube-sweep、transform/mirror/array、panel/vent-array/joint-stack/part-output | 11 个 Operator 已由固定 Worker 实际消费；boolean/Manifold unavailable，仍不提供纹理/PBR/视觉相似度 |
| `mesh-integrity` | MCP007/008 | finite/index/degenerate/readback 硬门 | 不是视觉相似度 |
| `uv-pbr@0.2.0` | MCP010E | 512px UV atlas、fixed mikktspace、MaterialZone、embedded glTF PBR/纹理颜色空间 | xatlas/UDIM/完整色彩管理/packaged/视觉 PBR 仍未运行 |
| `render-evidence` | MCP008/010C | `[transition-v1]` MCP008 四 pass compatibility；当前 MCP010C `RenderSet@2` 九 AOV、fixed camera/z-buffer、PNG/CAS/image block | source-focused deterministic path；provisional observation 的 packaged binding 和真实视觉阈值仍未通过 |
| `reference-compare` | MCP009/010C | `[transition-v1]` MCP009 limited metadata compare；当前 MCP010C local mask、silhouette/bbox/centroid/landmark/region typed metrics 与 diff evidence | synthetic/raw 只证明单位/绑定；不把颜色/CSS preview 当 likeness |
| `local-edit-and-export` | MCP009 | stable-Part change、approval、CAS `mvp-glb` | 不支持通用 mesh delta 或任意路径导出 |
| `primitive-blockout@0.2.0` | MCP010B | 当前 Runtime 可执行的 `GeometryProgram@2` primitive/hash/readback 结构 blockout；支持 ordered semantic-Part sink | 只有 box/cylinder/ellipsoid/sphere；不提供纹理、PBR、视觉相似度或 360° |

Skill metadata 的 operator ID 不等于当前全部 operator 已实现。当前可执行能力以 `geometry_prepare`/`appearance_prepare` 的 Runtime allowlist 和 `capabilities_get` 为准。

### 5.1 MCP010 Skill 版本（D/E source consumer 已实现，外部门仍 deferred）

| 任务 | 目标版本 | 激活前置条件 |
|---|---|---|
| MCP010D | `hard-surface-detail@0.2.0`（`primitive-blockout@0.2.0` 继续 active） | 已通过 V2 Schema、真实 Operator consumer、validator/benchmark/receipt、strict readback/lineage 和同 cohort packaged D raw structural probe；Manifold boolean、视觉门 NOT_RUN |
| MCP010E | `uv-pbr@0.2.0`、`render-evidence@0.2.0`、`reference-compare@0.2.0` + `forgecad-hard-surface-robot@1.0.0` | 离线 AssetPack、512px UV atlas、固定 `mikktspace@0.3.0`、嵌入式纹理/PBR/Render producer 和逐资产 provenance |

其他历史 Skill 保持 `0.1.0`，其中未实现 Operator 继续返回 `partial/unavailable`。`primitive-blockout@0.2.0`、`hard-surface-detail@0.2.0` 与 `uv-pbr@0.2.0` 是当前 active 的 V2 Skills；它们不是插件市场或任意执行插件，而是 Runtime 预注册 Operator 的声明式调用说明。AssetPack 仍是独立资产合同；缺少 producer/operator/asset/benchmark 时不得把 MCP010E Bundle 标为 active。

## 6. GitHub/外部工具决策

本文引用 GitHub 只表示研究候选，不表示已安装。当前没有 `accepted` 第三方 3D compiler、UV、tangent 或 renderer dependency。

| 项目 | 状态 | 允许的下一步 | 禁止行为 |
|---|---|---|---|
| `image-rs/image` | approved-for-evaluation | 隔离图片 decoder benchmark | 未固定 revision 就改 lockfile |
| `gltf-rs/gltf` | approved-for-evaluation | GLB strict readback benchmark | 接受外部 URI/任意 buffer |
| `elalish/manifold` | approved-for-evaluation | bounded boolean worker benchmark | 直接把 FFI 变 Runtime 真值 |
| `jpcy/xatlas` | approved-for-evaluation | UV seam/overlap/determinism benchmark | 未审计就替换 product-owned UV |
| `gltf-rs/mikktspace` | approved-for-evaluation | tangent golden benchmark | 漂移时静默改变 PBR |
| `KhronosGroup/glTF-Validator` | approved-for-evaluation | 外部 GLB validator receipt | 用外部报告替代 Runtime readback |
| `donmccurdy/glTF-Transform` | approved-for-evaluation-as-dev-tool | dev-only inspect/optimize | Node 进程写 SQLite/CAS 真值 |
| `img2threejs/img2threejs` | approved-for-evaluation / first-party reimplementation | staged passes、detail inventory、per-region confidence、side-by-side compare | Apache-2.0；不安装其 Python/TypeScript/Three.js skill，不把 JS 变 Runtime 真值 |
| `javierbyte/img2css` | reference-only visualizer idea | bounded 低分辨率颜色/区域预览，帮助 Codex 形成材质区和轮廓草图 | BSD-3-Clause；不执行其 JS，不保存 CSS/base64，不进入 GeometryProgram |
| Blender / BlenderMCP / FreeCAD MCP / CadQuery | reference-only/rejected for MVP | 只学习交互/算法 | 任意 Python、socket、网络资产、`.blend` 真值 |
| TripoSR/Hunyuan3D/远程 image-to-3D | rejected for MVP | 另立 ADR 后再评估 | 下载权重、远程 Provider、绕过 typed compiler |

ADR-0026 额外研究项目的当前口径：Pi Agent、NVIDIA Omniverse Kit、OpenUSD、FreeCAD、build123d/CadQuery、BlenderMCP、Trimesh、MaterialX、TRELLIS.2/Hunyuan3D 均不因文档重规划而变为 adopted dependency。用户已授权 Luna 对 build123d、BlenderMCP、CadQuery、Manifold、MaterialX 做选择性源文件研究，具体冻结 revision 和隔离流程见 `LUNA_GITHUB_REPLICATION_PLAYBOOK.md`；其 `research-authorized` receipt 仍不是 accepted adoption。其余任何“直接复制 skill、工作流、代码或权重”都必须先拆为 reference-only 学习或 accepted adoption receipt。

采用任何外部项目之前，Luna 必须新增 `docs/evidence/adoption/<project>/<full-revision>.yaml`，包含精确 revision、许可证文件 hash、transitive SBOM、恶意输入/资源测试、determinism benchmark、平台结果和 removal plan；只有 `approval: accepted` 才能改 lockfile 或打包。

## 7. 当前 MVP 状态和下一步

- `MCP005–MCP009 functional core`：focused tests/evidence PASS；可运行 `npm run mvp:functional-core`（包含 MCP005 本地 admission 回归；真实 Codex attachment probe 仍单独记录）。
- 真实 Codex MVP host golden path（参考附件 → geometry → appearance → quality → confirm → version → CAS export）：已由用户授权图片的 Codex CLI receipt 证明；MCP010A 另有第二次 Desktop 激活 Gate PASS。`reject → change → restore`、完整 Desktop 3D write、Viewer 同 hash、重启后的模型恢复和 packaged write 仍 `NOT_RUN/BLOCKED`，不能用 fixture 冒充。
- glTF Validator、独立真人视觉评分、provisional observation 的 packaged Viewer binding、Developer ID/notarization：当前 `NOT_RUN/BLOCKED`；像素级 silhouette/landmark/region compare 已实现但 attempt35 未达阈值且 truth binding 不完整，均不属于本地 functional-core 命令的隐含 PASS。
- 2026-08-10 的真实单图实验还比较了 23-Part 与 51-Part primitive blockout：两者 GLB/readback 均通过，但 limited aspect proxy 分别为 `0.5466` 与 `0.4604`，说明 Part/triangle 数量不能替代固定 camera、silhouette、region 和材质比较；详情见 `docs/evidence/mcp010b/real-reference-robot-detail-blockout.json`。
- `FGC-MCP010F` 是唯一 `in_progress`；当前 source fixed renderer/九 AOV/strict reference compare/typed visual review、D hard-surface Operator、E AssetPack/UV/PBR/MikkTSpace 和 F Viewer AOV/compare/Part/MaterialZone/explosion/heatmap Gate PASS，packaged Viewer read-model/window/core-control smoke 也已运行。attempt35 仍为 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`；B Darwin 512 MiB OS 总内存硬门、同一 provisional observation 的 packaged Viewer binding、真实 PBR likeness、正式 VoiceOver、人评阈值、export/restart hash 和 360 仍 `NOT_RUN/BLOCKED`。不得用 source/raw/package smoke 替代用户图片视觉门，也不得提前建设 heartbeat、broker、通用 pack installer 或插件市场。
