# ForgeCAD 权威状态与版本真值

2026-08-15 Agentic observation cache truth：Runtime 现在在进程内按 `AgenticSceneObserveResult@1.canonical_sha256` 缓存一次完整只读观察；bound plan/critic/visual-evidence/action follow-up 优先读取同一对象，不再在正常同一 Runtime 会话中重新拆分推导。缓存不是 SQLite/CAS 用户数据；Runtime 重启后仍允许一次重新构建，但必须通过请求 hash、project/candidate scope 与 canonical hash 校验。新增 cache/ambiguity regression 和 bounded action regression 通过；没有新的视觉 receipt，`QUALITY_TARGET_NOT_MET`、`camera=MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`、人评/PBR/export-restart/360 状态不变。

2026-08-15 Primary Form composition-lineage truth：`ForgeCADPrimaryFormCompositionLineage@1` 是 CLI 层的 hash-bound orchestration projection；每一步必须消费同一 current candidate 的 consolidated observation，完成第二步起先校验 prefix lineage 再推进 candidate，并且只允许 Runtime 返回 `prepared` 且严格改善时推进 staged candidate。raw events 仅保留 transport audit，projection 不写 Runtime/CAS、不产生 version/confirm/export，也不替代 Runtime-owned QualityReport。当前无授权 PNG，真实 composition 未运行；`QUALITY_TARGET_NOT_MET`、`camera=MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 仍为权威状态。

2026-08-15 Modular Primary Form continuation truth：当前源码的 `--part-contour-sequence` 只允许 2–3 个 exact Part，按 current candidate 重新绑定 consolidated observation、camera、baseline compare 与 Runtime-owned Rig 后执行一次 `primary_form_repair_prepare`；只有严格接受的 staged candidate 才会成为下一步 source，未改善结果保持原 candidate/camera。Runtime continuation regression、full Runtime `117 passed / 0 failed / 12 ignored` 与 MCP010C Gate 通过；raw stdio probe 的工具计数来自 `ForgeCADMcpToolManifestSummary@1`，不再硬编码。Stage 0 Runtime source hash 已同步且 checker 通过；该同步不改变 `camera=MISMATCH`、`QUALITY_TARGET_NOT_MET` 或 Viewer Runtime-only QualityReport authority。没有新的授权参考视觉 receipt，真实组合为 `NOT_RUN/BLOCKED_REFERENCE_BYTES_UNAVAILABLE`，未 confirm/version/export。

2026-08-15 Hip-pair staged candidate state：r21 `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-canonical-observation-hip-pair-r21.json` 在 cohort `726153a3…42ab5c` 通过 Runtime-owned 60-evaluation fit 与 `PASS_SILHOUETTE_FIT_TO_COMPARE`；source/proposal loss `0.426350916959`/`0.426265642648`，生成 staged `candidate-2dc5728da7124288b6ff79a0fb6b4f00`。同一 candidate 的最终 visual status 仍 `QUALITY_TARGET_NOT_MET`（IoU `0.743473474034`、Boundary F1 `0.301145366407`、bbox `0.0234375`、centroid `0.021831234764`，hard gate false），未 confirm/version/export。r19/r20/r21 相互独立，不能写成已合并模型；Stage 0 truth、Viewer Runtime-only authority、human/PBR/export-restart/360 状态不变。

2026-08-15 Chest-shell staged candidate state：r20 `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-canonical-observation-chest-shell-r20.json` 在 cohort `726153a3…42ab5c` 通过 Runtime-owned 61-evaluation fit 与 `PASS_SILHOUETTE_FIT_TO_COMPARE`；source/proposal loss `0.426350916959`/`0.416406210214`，生成 staged `candidate-d9d8cb24b1c5475d85b773c3ceb032c7`。同一 candidate 的最终 visual status 仍 `QUALITY_TARGET_NOT_MET`（IoU `0.739507057324`、Boundary F1 `0.314765400977`、bbox `0.013671875`、centroid `0.020515796935`，hard gate false），未 confirm/version/export。Stage 0 truth、Viewer Runtime-only authority、human/PBR/export-restart/360 状态不变；staged candidate 不等于 immutable version。

2026-08-15 Upper-arm-right staged candidate state：r19 `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-canonical-observation-upper-arm-right-r19.json` 在 cohort `726153a3…42ab5c` 通过 Runtime-owned 63-evaluation fit 与 `PASS_SILHOUETTE_FIT_TO_COMPARE`；source/proposal loss `0.426350916959`/`0.423630597587`，生成 staged `candidate-f1123cb13b624cb69493f825398b4423`。同一 candidate 的最终 visual status 仍 `QUALITY_TARGET_NOT_MET`（IoU `0.744938326895`、Boundary F1 `0.307109823312`、bbox `0.021484375`、centroid `0.021832396893`，hard gate false），未 confirm/version/export。Stage 0 truth、Viewer Runtime-only authority、human/PBR/export-restart/360 状态不变；staged candidate 不等于 immutable version。

2026-08-15 Pelvis bounded trial / camera binding reconciliation：r16 `BLOCKED`（瞬时 `reference_get` IPC），r17 `BLOCKED`（CLI 将 `no_improvement` proposal camera 错用于 source compare）均保留为独立 supplemental receipt；probe 已修复为仅在 `primary_form_repair_prepare=prepared` 时采用 repair camera。r18 `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-canonical-observation-pelvis-r18.json` 通过同 cohort canonical observation、26 inferred Part rows、`boundary_error_get=0` 与 `PASS_CAMERA_FIT_TO_COMPARE`；pelvis 55-evaluation proposal 未改善，source/proposal loss `0.435730135362`/`0.438891599478`，`no_improvement`/`retained_source`，最终 `QUALITY_TARGET_NOT_MET`，未 confirm/version/export。Stage 0 provisional truth、Viewer Runtime-only authority、human/PBR/export-restart/360 状态不变。

2026-08-15 Canonical observation + inferred Part table 状态：Runtime `silhouette_part_error` 对无显式 Part slice 的 target 生成同一 candidate/camera-bound 的 inferred Part error table；CLI 在同一 silhouette turn 内先完成 baseline compare，再读取一次 `AgenticSceneObserveResult@1`，不再发出独立 `boundary_error_get`。真实 r15 supplemental `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-canonical-part-observation-r15.json` 使用 cohort `726153a3…42ab5c`，canonical observation 含 26 个 inferred rows，`boundary_error_get` 为 0，推荐 Part 包含 `pelvis`、`upper-arm-right`、`chest-shell`；最终仍 `QUALITY_TARGET_NOT_MET`（IoU `0.739507057324`、Boundary F1 `0.314765400977`、hard gate false），未 confirm/version/export。它证明观察聚合与 Runtime 归因链路，不能改写 Stage 0 的 provisional `camera=MISMATCH`/`BLOCKED_INCOMPLETE_BINDING` 或升级 likeness。

2026-08-15 Primary Form right-shoulder bounded trial：current packaged cohort `26354e2f…a2029` 的真实 r11 receipt `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-part-contour-trial-r11.json` 对 `shoulder-armor-right` 完成 63 次 Runtime-owned bounded fit；same-camera source/proposal loss `0.426350916959`→`0.409541676181`，因此准备 staged candidate，但全局 `silhouette_iou=0.749072115206`、`boundary_f1_4px=0.326405552646` 仍为 `QUALITY_TARGET_NOT_MET`，且 `quality_hard_gate_passed=false`。没有 confirm/version/export、持久用户数据或无关副作用；r11 作为 supplemental current-cohort evidence 保留，不能替换 Stage 0 provisional truth，也不能宣称 Primary Form 已收敛。

2026-08-15 Primary Form exact-side sink alias 状态：Runtime 的固定 Part-ID projection 现在覆盖 `*-armor-left/right` 与显式 `*-left/right` sink 的单侧别名，且 side-aware；`shoulder-armor-left` 只会 materialize 到左侧，不会因 alias 规则污染右侧。新增 Runtime regression 通过；该 source fix 没有新的真实 likeness receipt，不改变 Stage 0 的 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 或未运行门。

2026-08-15 Primary Form exact single-Part trial 状态：`primary_form_repair_prepare`/Job 现在可接受可选 `part_id`，Runtime 会从 bilateral authoring Rig 派生 exact Part Rig，仅让该 Part 的 width/height/offset controls 进入 bounded search；主流程不再依赖预先存在的 Part RenderSet，source/proposal 仍由同一 Runtime-owned camera 与 Render Worker compare 做严格接受。r10 current cohort `d2b67cd8…02a2f1` 对 `shoulder-armor-left` 完成 trial，`acceptance_status=retained_source`、`candidate_state=unchanged`、`acceptance_strict_improvement=false`，source/proposal loss `0.426350916959`/`0.425568832154`；Part contour readback 为 `proposal_ready`。该 receipt 顶层 `BLOCKED` 仅来自 probe 无关副作用分类，未写持久用户数据、未 confirm/export，quality `NOT_RUN`，Stage 0 仍是 `QUALITY_TARGET_NOT_MET`、`BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 Primary Form 单 Part contour/semantic asymmetry 状态：Runtime `part_contour_fit_prepare` 已关闭显式 bilateral Worker Part-ID 与 Rig Part 不一致的投影缺口：`hip-pair` 可读取 `hip-left`/`hip-right` 的合并 envelope，而精确 `shoulder-armor-left` 只读取左侧 mask/error；单侧 Rig 的 width/height/offset 参数保持 typed、bounded、read-only。真实 r8 receipt `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-part-contour-r8.json` 在 cohort `dd78d216…e5088` 上返回 `proposal_ready`，26 Parts、无持久化或无关副作用；它是 boundary-only 结构证据，`quality_visual_status=NOT_RUN`，不得改写 Stage 0 的 `QUALITY_TARGET_NOT_MET`、`BLOCKED_INCOMPLETE_BINDING` 或历史 benchmark。

2026-08-15 Primary Form 36-control Rig 真实复放状态：packaged cohort `106a2889…67b08e` 的 r7 supplemental receipt 记录 56 次 Runtime-owned bounded fit；新增 hip width/height、hip/pelvis/chest x-offset 与 hip y-offset 后，source loss `0.40710507361`、proposal loss `0.413276643194`，严格 acceptance 仍为 `retained_source`，`candidate_state=unchanged`，无持久用户数据写入。current-cohort camera binding、Render Worker 与 Viewer read-model 结构绑定通过，但 final `silhouette_iou=0.746739614479`、`boundary_f1_4px=0.34284083431`，`quality_visual_status=QUALITY_TARGET_NOT_MET`；r7 顶层 `BLOCKED` 只代表无关 side-effect 账本，不是质量或 transport PASS。

2026-08-15 Primary Form same-camera retention 真实验证：Runtime geometry-winner refit 现在先在同一 Runtime-owned final camera 上渲染 source GLB 与 proposal，再执行严格 `proposal_loss < source_loss` acceptance；search-only comparator 仅用于 bounded local-minimum exploration，最终 candidate/evidence 仍使用严格 acceptance。packaged cohort `e7e466f6…b7cc` 的 supplemental r6 receipt 记录 source loss `0.40710507361`、proposal loss `0.412782572291`、`acceptance_status=retained_source`、`acceptance_strict_improvement=false`、`candidate_state=unchanged`；顶层 `BLOCKED` 来自无关 side-effect 账本，未产生 candidate/version/confirm/export。真实 compare、camera binding、Render Worker cohort 与 Viewer read-model 结构门通过，视觉质量仍为 `QUALITY_TARGET_NOT_MET`。

2026-08-15 QualityReport 质量权威一致性与 packaged F 状态：`AppearancePrepare` 的结构性产物在 `visual_status=not-run` 时现在明确写入 `hard_gate_passed=false`，与 Runtime `QualityReport@2` validator 和 Viewer 只读消费规则一致；同一 packaged cohort `fee79807…` 的隔离 F probe 已验证 26 parts/4704 triangles、7 material zones、九 AOV、embedded textures、Render Worker cohort binding 与 Viewer read-model PASS。该证据仍是 structural/read-model PASS，不是 likeness 或视觉质量 PASS；真实质量继续为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，人评/PBR likeness/export-restart/360 未运行或阻断。

2026-08-15 Primary Form 多尺度几何收敛状态：Runtime-owned `GeometryProgram` 搜索在固定 `max_evaluations=64` 上限内调整为 `geometry=40`、初始 camera=16、geometry-winner refit=8；较小预算按同一有界比例分配，三者总和仍不超过调用者上限。coordinate probe 的后续 pass 使用确定性 `1.0 → 0.5 → 0.25` 步长，形成一次完整 evidence-directed pass 后的 reverse/fine refinement，不把连续参数轨迹交给 Codex。该 source/focused 修复不改变 Schema、Worker/readback、same-camera acceptance、Viewer quality authority 或历史 receipt；当前仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，人评/PBR/export-restart/360 未运行或阻断。

2026-08-15 camera-fit→compare handoff 状态：`reference_compare_prepare` 在没有显式 camera、但存在同一 `project_id/candidate_id/target_sha256` 的 Runtime camera-fit cache 时，复用该 `selected_camera`，不再重新从 default camera 推导 framing；无 cache/target 仍走原有 bounded fallback。新增自动复用 regression 通过。该源码 Gate 尚未替换历史真实 Codex receipt，因此当前账本仍保留 `camera=MISMATCH`、`QUALITY_TARGET_NOT_MET` 与 `BLOCKED_INCOMPLETE_BINDING`，不得写成视觉质量通过。

2026-08-15 Primary Form bounded continuation 状态：`primary_form_repair_prepare` 与异步 Job 的 Runtime optimizer 现在最多允许 2 个 continuation iterations；总 `max_evaluations` 仍为 64，第二轮只复用第一轮 Runtime incumbent 的相机/几何上下文。focused fixture 已验证 63–64 次 bounded evaluations 与 `iterations=2`，但没有新真实视觉 receipt，因此历史 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH` 和 benchmark incomplete binding 不变。

2026-08-15 Render Worker cohort-to-RenderSet 状态：Runtime sibling launcher 保留 isolated `forgecad-render-worker` 响应的 `build_cohort_sha256`，perspective `RenderSet@2` 写入 `render_worker_build_cohort_sha256` 与 `render_worker_binding_status`；Runtime 当前 cohort 不匹配、字段缺失或 status/值不一致时 fail closed。source build 的 null cohort 只能标记 `cohort_unavailable`，packaged same-cohort 才能标记 `same_cohort_verified`；Viewer 只读展示该信息，不推导 `QualityReport@2` 门。该结构/证据修复不产生新的 likeness、PBR、人评、export/restart 或 360 结论，Stage 0 仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 canonical observation durable binding 状态：`DesignSession@1.observation_sha256` 与 `DesignCheckpoint@1.observation_sha256` 必须等于同一 Runtime 生成的 `AgenticSceneObserveResult@1.canonical_sha256`；`session_get`、checkpoint prepare/get/restore 和 `design_action_run_prepare` 会重新生成当前 observation，过期或跨 candidate/session 的 hash fail closed。Store payload 校验、Runtime focused/full tests、contracts、隔离 Runtime/MCP restart receipt 和 nested projection receipt 通过。该状态只证明观察证据的 durable lineage，不改变 Runtime 唯一写者、Viewer read-only authority、完整 durable/reference/DesignSpec producer 或 Repair 执行状态；Stage 0 继续为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，人评/PBR/export-restart/360 未运行或阻断。

2026-08-15 RepairIntent durable binding 状态：`RepairIntent@1` 的 `observation_sha256` 由 Runtime 在 checkpoint restore prepare 时写入，并且必须等于 durable session 的 canonical observation hash；Store/CAS readback、schema positive/negative fixture、receipt checker 与隔离重启 probe 均通过。该字段只证明未来 Repair intent 使用哪一次观察，Repair 本身仍未执行；完整 orchestrator、真实 likeness、`QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING` 和人评/PBR/export-restart/360 状态均不变。

2026-08-15 Primary Form local-group convergence 状态：Runtime 的 bounded geometry search 在 joint proposal 未改善时，不再直接把所有剩余预算拆成互相独立的单坐标试探；它按 Runtime boundary ranking 形成同 Part 的耦合 width/height/offset candidate，并只在 candidate 严格改善时合并到当前 geometry incumbent。该修复没有改变 Schema、工具数、Viewer quality authority 或真实 receipt；`QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 和人评/PBR/export-restart/360 保持原状。

2026-08-15 Primary Form ranking snapshot 状态：Runtime 将 camera/geometry/refit 的 public contour metrics 与 transient landmark ranking metrics 分离保存，但所有 winner、baseline 和 strict-improvement 比较均使用同一完整 ranking snapshot；geometry trial 以 selected camera 的 snapshot 为局部 incumbent，最终 fit 仍对 authored base camera 做严格比较。该修复不改变 `SilhouetteFitResult@1` 五项公开 metrics、Runtime 唯一写者或 Viewer read-only authority；新增 snapshot regression，Runtime 全量 `105 passed / 12 ignored`。没有新的真实视觉 receipt，Stage 0 继续为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 observation/camera binding 状态：Runtime action 只接受由 Codex 明确提交的 `observation_sha256`，并把它纳入 `DesignActionRun@1.input_sha256`；当前 Runtime observation canonical hash 不一致时返回 `AGENTIC_OBSERVATION_STALE`，不创建 action receipt。Primary Form 的 camera handoff 使用 session `camera_hash` 在同一 candidate/target-bound fit projection 中解析完整 `CameraCalibration`，未命中时返回 `AGENTIC_CAMERA_BINDING_MISMATCH`，不使用 default fallback。MCP/Runtime focused tests 通过；这只是观察和相机的证据绑定修复，不改变 Runtime 唯一写者、Viewer read-only authority 或 Stage 0 的 `QUALITY_TARGET_NOT_MET`、`MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 Runtime quality-authority 状态：`visible_view_gate_checks` 集中持有 visible-view metric 的方向/阈值/状态，`visible_view_gate_passes` 与 Agentic critic projection 共享它；Agentic 不再自带第二套质量门。该修复保持 Runtime 唯一质量权威与 Viewer read-only projection boundary，focused/full Runtime 和 MCP010F source Gate 通过。没有新的真实视觉 receipt，Stage 0 仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，人评/PBR/export-restart/360 仍未运行或阻断。

2026-08-15 Primary Form metric-priority 状态：Runtime 的 bounded fit 不再用 Chamfer/IoU 加权和单独决定 winner。`primary_form_metric_ordering` 以 boundary F1、silhouette IoU、bbox、centroid、landmark coverage/NME、region 顺序逐层比较；同一优先级完全相等时才使用 scalar loss 作为稳定 tie-break。该排序已接入 camera fit、Rig geometry/refit、same-camera acceptance 与 candidate compare，并新增 focused regression；完整 Runtime 预期 `104 passed / 12 ignored`。没有新的真实机器人视觉 receipt，Stage 0 仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，人评/PBR/export-restart/360 仍未运行或阻断。

2026-08-15 Primary Form 双向边界证据状态：Runtime-owned `boundary_error_segments_for_masks` 现在对同一 target/model mask 以固定、有限采样同时建立 reference→model 与 model→reference correspondence；model-only excess edge 不再因单向 nearest projection 被漏掉。每条记录继续绑定同一 camera 与 Part-ID，并交给既有每部件覆盖和最多 64 条 deterministic selection；新增 regression，`forgecad-runtime` 全量 `103 passed / 12 ignored`。该 source/convergence Gate 没有新的真实机器人视觉 receipt，不改变 Runtime 唯一写者、Worker typed boundary、Viewer read-only authority 或 Stage 0 的 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`；人评/PBR/export-restart/360 仍未运行或阻断。

2026-08-15 Render Worker ownership 状态：Runtime 通用 sibling launcher 现在由 `geometry_worker.rs` 的 `execute_sibling_worker` 提供，Render Worker binary identity、typed request projection 和 response adapter 全部归属 `render_worker.rs`；Geometry Worker 不再保留 `RENDER_WORKER_BINARY` 或 `execute_render_worker`。新增两个 macOS isolated tests 要求 source-built same-cohort `forgecad-render-worker`，覆盖九 AOV/512px PNG/确定性和 GeometryProgram 载荷拒绝；普通 Runtime 全量 `102 passed / 12 ignored`，boundary checker PASS。该结构 Gate 不生成新的视觉质量证据，Stage 0 仍保持 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，人评/PBR/export-restart/360 未运行或阻断。

2026-08-15 Agentic observation single-snapshot 状态：`design_stage_plan_get`、`critic_report_get` 和 `visual_evidence_bundle_get` 现在只能使用 `scene_observe_get` 返回的 canonical observation hash；Runtime 校验后直接切片同一 `AgenticSceneObserveResult@1`，不允许派生读工具重建第二份现场。Agentic evidence 统一为 `VisualEvidenceBundle@1`，Viewer 专用 `ViewerVisualEvidence@1` 不变。focused stale-hash/slice regression 通过；该边界只减少 Codex 观察漂移，不推进视觉质量、confirm/export、人评/PBR/restart 或 360。

2026-08-15 Primary Form evidence-magnitude handoff 状态：Runtime 第一轮坐标 probe 在当前值与 candidate-bound evidence proposal 不同的时候，复用该坐标的 evidence-derived magnitude；单步上限为 authored Rig span 的 50%，proposal 已到达或后续反向 pass 则使用原有小步。该模块只改变 Runtime 内部 bounded search 的步长，不改 schema、tool manifest、Runtime 唯一写者、Worker 协议或 Viewer quality authority。`forgecad-runtime` 全量 102 passed、10 ignored；没有新的真实视觉 receipt，Stage 0 仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 未运行或阻断。

2026-08-15 Primary Form boundary evidence coverage 状态：Runtime-owned `boundary_error_segments_for_masks` 现在在固定 64 条上限内先覆盖每个有 Part-ID 的可见部件，再按最大距离填充；输出仍按 distance descending，未归属 segment 只参与填充阶段。该修复让同一观察能同时提供 shin/head/hand 等部件的局部方向证据，供后续 bounded Rig sweep 使用，不改变 schema、tool manifest、Runtime 唯一写者、Viewer read-only authority 或 Agentic 单 Part action scope。coverage regression 与 `forgecad-runtime` 全量 101 passed、10 ignored；没有新的真实视觉 receipt，Stage 0 仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 未运行或阻断。

2026-08-15 Primary Form proposal-direction retention 状态：Runtime 现在将完整 candidate-bound boundary proposal 与 dominant-Part 的局部 probe-zero seed 分离；secondary Part coordinate probe 使用其 evidence-derived bounded direction，零改变量才回退到稳定方向。新增显式 `-1/0/+1` 回归，避免正零被误当成正向。全量 Runtime 为 100 passed、10 ignored；没有新的真实视觉 receipt，Stage 0 继续为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，Agentic action-run 单 Part scope 和 Viewer read-only authority 不变。

2026-08-15 Primary Form bounded multi-Part sweep 状态：Runtime 仍让 candidate-bound dominant Part 只负责初始化局部 seed，以避免一次联合 proposal 耦合多个轮廓误差；后续 deterministic coordinate schedule 已移除 dominant-Part filter，会在固定 Geometry/Render Worker 预算内覆盖完整 typed Rig。新增 regression 验证 secondary Part 在 seed 后仍进入排序和 probe schedule；`forgecad-runtime` 全量为 100 passed、10 ignored。没有新的真实机器人视觉 receipt，Stage 0 继续为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`；Agentic action-run 的单 Part scope、Viewer read-only authority 和未运行的人评/PBR/export-restart/360 门不变。

2026-08-15 Primary Form local Part offset calibration 状态：Runtime 局部 Part envelope proposal 对米制 `offset_x/offset_y` 现在使用同一 camera calibration 的 world-per-normalized-screen scale；`offset_y` 按 image-Y 向下、camera-plane up 向上的坐标约定换算。`fit_rig_parameters_with_landmark_context` 将完整 camera 传入该 proposal；没有 camera 的 legacy `part_contour_fit_prepare` 不再猜测米制世界位移。相机标定/符号 focused regression 和 `forgecad-runtime` 全单测通过；没有 Schema、tool manifest 或真实视觉状态变化，Stage 0 仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 Primary Form same-camera retention 状态：`primary_form_repair_prepare` 现在在创建 staged candidate 前，使用最终 Runtime-owned camera、同一 reference target 和同一 512px Render Worker 对 source GLB 与 proposed GeometryProgram 做 non-persisted full-resolution compare；只有 `proposal_loss < source_loss` 才进入正常 candidate/evidence transaction。`PrimaryFormAcceptance@1` 固化 source/proposal program hash、camera hash、两侧 loss 和 `accepted`/`retained_source`；未通过时保持 authored source，不覆盖 source VisualEvidence。该 Gate 只修复相机补偿/局部回归导致的错误晋级，未产生新的授权机器人视觉 receipt，当前仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 Agentic observation→action binding 状态：`design_action_run_prepare` 只把当前 Runtime 生成并校验过的 `AgenticSceneObserveResult@1.canonical_sha256` 写入 `DesignActionRun@1.observation_sha256`；SQLite/CAS immutable readback 与合同均要求该 SHA-256。focused action round-trip 验证 receipt 与同 candidate 的 Runtime observation 相等，缺字段 fixture fail closed。该模块只修复 observation 被拆散后无法证明 action 使用哪一次观察的问题，不等于多阶段 orchestrator、Repair 应用或视觉质量通过；Stage 0 继续保留 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 Primary Form 收敛证据状态：Runtime 现对每次 bounded fit 固化 authored baseline 与 selected winner 的 camera/metrics/loss、strict-improvement 和 camera/geometry evaluation 计数；action-run evaluate stage 通过 `result_sha256`、`output_sha256`、`summary_sha256` 形成 Primary Form→QualityReport→comparison 的单一证据链。`DesignActionRun@1` Schema 已对齐 Store 的实际 stage object，并通过 `skipped`/旧 `hash/reason` 负向回归。该修复不改变 candidate/version 写入边界，不把 Runtime source/focused PASS 写成视觉质量 PASS；Stage 0 仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 Render Worker Runtime adapter 状态：Runtime 侧新增 `render_worker.rs` typed adapter，Geometry Worker 只保留编译/worker transport，Primary Form 的初始 512px framing、full-resolution camera ranking、Geometry winner refit 和九 AOV render 均通过该 adapter 调用同一个 isolated sibling Render Worker。新增 source ownership checker 与 Runtime focused regressions 通过；该模块化修复不改写现有真实 reference，视觉仍为 `QUALITY_TARGET_NOT_MET`，`camera MISMATCH`、benchmark incomplete binding、human/PBR/export-restart/360 状态保持原账本。

2026-08-15 Agentic bounded action-run 状态：`design_action_run_prepare`/`design_action_run_get` 已进入当前 Runtime/MCP source slice。该切片只允许 approved、session/project/candidate/reference-bound、单 Part `primary-form` action；Runtime 在一次 action run 内复用既有 Primary Form bounded repair pipeline，写入 SQLite/CAS 的 `AgenticActionRunRecord` 并提供 immutable readback。结果锁定 `confirm`/`export`，`runtime_write=false`、`persistent_user_data_touched=false`，不会修改 candidate/version。focused Runtime idempotency/readback、MCP boundary/manifest 和 Stage 0 已通过；它不等于通用 orchestrator、Repair 应用、durable/reference/DesignSpec 完整 producer 或视觉质量通过。

2026-08-15 Primary Form 自动目标单 Part 状态：Runtime 已把没有显式 `SilhouetteTarget.parts` 的 automatic silhouette target 交给同一 Render Worker Part-ID boundary projection，仅对请求的 semantic Part 生成局部 envelope/error 和 bounded proposal；没有 hidden-side inference 或额外 Codex 参数搜索。current Dev.app cohort `77ccce85…b9d4` 的 `shin-pair` 隔离 receipt 已通过 preflight、MCP/Runtime cohort binding、Geometry/Render Worker transport 和五候选 compare，但严格单 Part retention 门保留 authored baseline（IoU `0.741047`、Boundary F1 `0.328765`），没有应用 candidate，仍为 `QUALITY_TARGET_NOT_MET`。该 supplemental receipt 不改写 Stage 0 provisional observation、camera `MISMATCH` 或 benchmark eligibility。

2026-08-15 Primary Form action budget truth：修复前真实 receipt `docs/evidence/mcp010f/primary-form-budget-pre-fix-real-codex-20260815.json` 暴露了 `max_evaluations=64 → fit_evaluations=24` 的 Runtime 外层截断；当前源码 cap 已恢复为 64，Runtime continuation 上限为 2，端到端 focused fixture 证明 `primary_form_repair_prepare` 在 64 请求下完成 63–64 bounded evaluations 且 `iterations=2`。Dev.app cohort `c521bf28…c4a5` 已安装；修复后 real-Codex receipt 在 authoring/hash/prepare 阶段阻断，没有新的视觉比较或质量结果。该源码修复不改写 retained observation，仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 未运行或阻断。

2026-08-15 packaged Render Worker 状态：Dev.app 当前 cohort `aa5eaaa2…5827` 已将 MCP、Runtime、Geometry Worker、Render Worker 四个资源一起安装；Resource allowlist、ad-hoc deep-strict 签名、同 cohort identity 与 packaged raw stdio 均通过。Runtime 在 sibling Render Worker 进程边界内完成九 AOV、固定 renderer、两次 deterministic hash、compare 与 image-block transport，且未写持久用户数据。该状态只把 packaged resource/process/protocol boundary 记为 PASS；raw 输入为 synthetic reference，`QUALITY_TARGET_NOT_MET`、`structural_visual_claim=NOT_CLAIMED`、human/PBR/export-restart/360 未运行事实保持不变。

2026-08-14 Primary Form Part-priority 修复：Runtime 不再只按 Rig 提案改变量排序 bounded geometry probes；在已有同 candidate 的 Part-ID boundary evidence 时，先按聚合的 Part contour distance 优先覆盖主导可见误差 Part，再按参数 delta 与稳定 ID tie-break。无 Part evidence 仍使用原排序，所有值/边界/Worker 调用保持 Runtime-owned。focused/full Runtime 与 MCP010F source Gate PASS；新的授权参考视觉复验因原图字节不在当前 workspace 而阻断，不能升级 `QUALITY_TARGET_NOT_MET` 或 benchmark 状态。

2026-08-14 Primary Form 首轮全控制覆盖修订：detail route 的 26-control `SilhouetteRig@1` 现在通过 `max_evaluations=64` 进入 Runtime；GeometryProgram 路径的确定性预算为 `32 geometry + 16 initial-camera + 16 geometry-winner-camera-refit`，首轮几何试探覆盖初始证据提案和全部 26 个控制。该 source/convergence 修订有 Runtime focused/full regression，但尚未重新运行授权机器人 reference，因此不改变 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 或人评/PBR/export-restart/360 状态。

2026-08-14 Primary Form 单动作真实 transport：同 cohort receipt `docs/evidence/mcp010f/real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json` 已验证 Codex 在一次 observation/camera/Rig 回合后只调用一次 `primary_form_repair_prepare`；Runtime 在该动作内完成 24 次 bounded fit、Geometry Worker 编译、严格 GLB 回读、隔离 Render Worker、candidate-bound compare 和九 AOV，随后 CLI 只读取 Runtime 返回的 visual evidence，不再重复发起 Codex compare。相机绑定为 `PASS_SILHOUETTE_FIT_TO_COMPARE`，compare IoU `0.749122`、Boundary F1 `0.347623`，仍为 `QUALITY_TARGET_NOT_MET`；这证明模块边界和证据链收口，不证明 likeness/high-quality PASS。

版本：2026-08-13
状态：MCP005–009 functional truth 已实现；FGC-MCP010A done；MCP010B structural truth source Gate 已通过但 Darwin OS memory hard cap deferred/NOT_RUN；MCP010C source-focused renderer/compare/review Gate、MCP010D hard-surface Operator/Skill Gate 与 MCP010E 离线 AssetPack/UV/PBR/MikkTSpace Gate 已通过，首次真实机器人 compare/review transport 已运行但 likeness threshold `FAIL_QUALITY_TARGET_NOT_MET`；MCP010F Viewer source Gate 已通过，当前 cohort packaged Viewer CLI read-model 已完成 exact project/candidate/artifact/reference/render-set/comparison lineage binding，正式 UI/accessibility、独立人评、360 仍 NOT_RUN/BLOCKED。当前 F 轮廓 slice 另提供 Runtime-owned `silhouette_part_error_get`：按 hash 绑定的多 Part table 归因局部边界误差，source focused PASS；它不改变当前真实图片 likeness 失败事实。ADR-0026 的 observe/plan projection 与受批准 durable `DesignSession`/`DesignCheckpoint`/`RepairIntent` prepare/readback 已成为当前 source/runtime/MCP/Viewer 证据层；单动作 orchestrator、Repair 应用、完整 producer/consumer conformance 与完整视觉门仍未完成。

Stage 0 权威快照：当前为 102 Schema、36 read + 23 opt-in write = 59 tools，唯一 `in_progress` 为 `FGC-MCP010F`；机器可读事实入口是 `docs/evidence/mcp010f/current-benchmark-truth.json`。attempt35 只称 `provisional retained observation`，视觉状态为 `QUALITY_TARGET_NOT_MET`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`：camera-fit 选中相机 `354caf27…f95788`，reference-compare 相机为 `8cd20605…a535`，判定 `MISMATCH`。packaged Viewer receipt 已将其自身 current-cohort read-model 精确绑定到一个 project/candidate/artifact/reference/RenderSet/comparison，但 UI/accessibility、image-block consumption、真实人评和 packaged E2E 仍未运行；attempt35 与 r3 staged candidate 都不是 best 或合格 benchmark，source、raw transport、build、窗口结构或 AX smoke 也不构成视觉、人评或 packaged UI PASS。Runtime 的 `primary_form_repair_prepare` 只证明窄范围 staged prepare/evaluate，不改写上述质量事实。Agentic observe/plan receipt 与 durable session/checkpoint/RepairIntent receipt 只证明各自隔离 source/readback slice，不改写上述质量事实。

2026-08-14 Worker 边界修订：`render_fixed` 的 GeometryProgram 编译已从 Render Worker 输入移回 Geometry Worker；Render Worker 只接受编译后 GLB 的 fixed/perspective/batch render façade，geometry compile payload 有隔离负向回归。无状态 renderer implementation 已拆到 `apps/render-core`，Render Worker 不再依赖 `forgecad-geometry-worker` crate，source ownership checker 与 MCP010C 回归已通过。该状态只把 process/protocol/source ownership boundary 记为 PASS，不升级 MCP010F、`QUALITY_TARGET_NOT_MET`、camera `MISMATCH` 或其他未运行的视觉/发布门。

2026-08-14 Primary Form evidence projection 修订：Runtime 在选中 camera 的同一 Part-ID readback 上消费固定 landmark→Part anchor，将投影误差映射为相机平面米制 offset，再以 typed transform/source geometry 试算；detail 编排使用 20 参数和 `32 → 21 geometry + 11 camera` 的 Runtime-owned bounded budget。该修复有 landmark ownership、camera-plane materialization 和完整 Runtime 回归证据，但尚未重跑授权机器人 reference，因此只改善收敛路径，不改写当前真实质量、camera binding 或 benchmark eligibility。

运行时间线以 `docs/evidence/mcp010f/real-codex-run-inventory.json` 为准：attempt5 是历史 CameraCalibrationRef 里程碑；当前最新完成 transport 与最新 attempt 均为 `docs/evidence/mcp010f/real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json`，状态为 `PASS_WITH_QUALITY_TARGET_NOT_MET`，fit/compare camera binding 为 `PASS_SILHOUETTE_FIT_TO_COMPARE`，未晋升为 attempt35 provisional benchmark。历史 boundary-projection、semantic-landmark-compare 与 semantic-aligned-fast receipts 按原始状态保留。

<!-- forgecad-stage0: schemas=103 schema_set_sha256=01218d921dd05574835d5762c8b64c72332b61a58eba6cdb20d0190d4b658a47 read_tools=37 write_tools=24 total_tools=61 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json latest_completed=real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json -->

2026-08-14 Primary Form / observation module state：Runtime-owned fit now honors the declared bounded budget with geometry priority (`24 → 16 geometry + 8 camera`) and covers the full ranked Rig coordinate set before reverse-direction probes. The MCP010C/F source and worker-boundary gates pass, while current real-reference quality remains `QUALITY_TARGET_NOT_MET`; canonical observation is recorded as one bound `AgenticSceneObserveResult@1` stage, not a source of new likeness evidence.

2026-08-14 Primary Form proposal handoff state：`silhouette_fit_prepare` now exposes an optional `selected_geometry_program` only when a Runtime Geometry Worker trial strictly improves the authored baseline. The returned `GeometryProgram@2` is project/hash validated at the result boundary and remains read-only; it is intended for a later user-approved `geometry_prepare` call, not automatic candidate mutation or confirmation. Contracts and focused/source gates pass; this does not change `QUALITY_TARGET_NOT_MET`, camera `MISMATCH`, benchmark eligibility, or the unrun human/PBR/export/360 gates.

2026-08-14 Primary Form bounded schedule state：camera coordinate-descent no longer aborts when one local batch fails to improve the incumbent. It now consumes the remaining Runtime-owned bounded schedule so later roll/FOV/distance/target-offset/global-scale axes are evaluated before the declared budget is exhausted; a focused 8-camera fixture verifies all 8 evaluations are consumed. This repairs a real convergence truncation, but adds no visual-quality evidence and does not change `QUALITY_TARGET_NOT_MET`, camera `MISMATCH`, `BLOCKED_INCOMPLETE_BINDING`, or the unrun human/PBR/export/360 gates.

2026-08-14 Agentic visual evidence binding state：`VisualEvidenceRecord` now persists the nullable `target_sha256` used by a target-bound `reference_compare_prepare`, with an additive SQLite migration and project/reference validation on readback. `ViewerVisualEvidence@1` and the read-only `VisualEvidenceBundle@1` projection expose that same hash；`DesignCriticReport@1` now emits one fixed-priority `primary_form_directive` whose continuous-search owner and repair owner are both Runtime. Target-bound Runtime regression proves the hash survives SQLite/CAS → Viewer → `AgenticSceneObserveResult@1`；default legacy comparisons remain nullable. This reduces duplicated Codex observation/search context without executing Repair or changing Viewer quality authority；full Runtime 87 passed/10 ignored, Store 8 passed, contracts/Agentic contracts and `script/test_mcp010f.sh` passed. Real likeness remains `QUALITY_TARGET_NOT_MET`, camera `MISMATCH`, and benchmark eligibility `BLOCKED_INCOMPLETE_BINDING`.

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

- `DesignSession@1` 当前已能在同一 project/candidate/reference/evidence lineage 下持久化 stage、失败门、下一步允许动作和 checkpoint 指针；单动作 orchestrator 与候选变更仍必须另行落到 Runtime candidate/version/job；
- `DesignCheckpoint@1` 当前已能持久化不可变失败/阶段检查点并在重启后读回；`checkpoint_restore_prepare` 只生成 CAS-bound `RepairIntent@1`，不执行 Repair、不改写历史；
- `SemanticSceneGraph@1` / `ModelUnderstandingBundle@1` 未来是只读理解投影，必须由 Runtime candidate、readback、RenderSet、QualityReport 和 source map 派生；
- `ReferenceCanvas@1` / `DesignSpec@1` 当前由授权 `ReferenceEvidence` 生成受限、hash-bound 的 session read model，覆盖不足和 unknown 会明确阻断；完整独立 producer/consumer conformance 仍未完成；
- Critic/Repair report 未来只记录 evidence-bound issue 和 bounded intent，不能跳过 compile/readback/render/compare；
- Parametric Design Kit 未来必须展开为 typed Geometry/Appearance contracts，不允许成为任意脚本或第二几何真值。

上述 durable 对象目前只在受限 prepare/readback receipt 的范围内计入当前能力账本；单动作 orchestrator、Repair 应用、完整 Visual Evidence conformance、packaged same-cohort 与视觉质量门仍不得宣称已实现。

### 1.1 MCP010 当前与目标真值

2026-08-14 当前真值修订：manifest/目录计数已为 102 个 JSON Schema；packaged Viewer receipt `docs/evidence/mcp010f/real-codex-cli-current-20260814-primary-form-coverage-bound-viewer.json` 已在其 cohort 下完成 packaged Viewer CLI read-model 的 exact project/candidate/artifact/reference/render-set/comparison lineage binding；全局最新 r3 transport 另见 `docs/evidence/mcp010f/real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json`。两者均为 `QUALITY_TARGET_NOT_MET`、`quality_hard_gate_passed=false`，且 sanitized Codex events 未观察到 image-block consumption；UI/accessibility、真人评审、PBR likeness、export/restart hash 和 360°仍 `NOT_RUN/BLOCKED`。该绑定只补齐读模型 lineage，不把 attempt35 或 r3 staged candidate 晋升为 likeness/high-quality PASS。

MCP010B 当前源码增加 8 个合同，MCP010C 再增加 7 个合同，MCP010E 再增加 6 个合同，MCP010F 轮廓求解器新增 12 个合同（含 `CameraCalibrationRef@1`）及其余 CameraCalibration/target binding 合同，Agentic contract family 继续补充 projection/session/checkpoint/RepairIntent 合同，当前共 102 个 JSON Schema（MCP006 历史为 44）。B 的 `GeometryProgram@2`/strict readback/restore evidence source Gate 已通过；Darwin 512 MiB OS memory hard cap 仍 deferred/NOT_RUN。C 的 reference/renderer/review 合同、E 的 AssetPack/Appearance V2 合同与 F 的 silhouette target/camera/Rig/SDF/Part/candidate compare 合同已由 Runtime/MCP producer/consumer 使用；Agentic projection 与 durable session/checkpoint/RepairIntent prepare/readback 也已通过合同/隔离重启 Gate；固定 512×512 perspective/z-buffer renderer、九 AOV、local mask/metrics、MCP image block、Codex/human review、离线 AssetPack、512px UV atlas、固定 mikktspace、embedded PBR texture、哈希绑定轮廓目标、扩展相机搜索、受限 Rig/SDF fit 和候选比较的 source raw Gate 已通过。历史真实 Codex CLI C receipt 已完成六 turn/32-call transport；带 15 landmark/8 region intake 的 source-built silhouette-first attempt35 已完成 11-turn detail reference→mask/target→camera/Rig/fit→compare→boundary→九 AOV→typed review/quality transport，但只保留为 provisional retained observation，结果为 `QUALITY_TARGET_NOT_MET`（26 Parts/4704 triangles，IoU `0.741047`、boundary F1 `0.328765`、bbox edge error `0.007813`、landmark coverage `0.733333`、region median `0.869403`），不是 likeness PASS；其 camera-fit `354caf27…f95788` 与 reference-compare `8cd20605…a535` 为 `MISMATCH`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`。Runtime fit proposal 为 `no_improvement`、IoU `0.698340`，仍是 read-only 建议；`primary_form_repair_prepare` 另有 staged prepare/evaluate，但没有 confirm/version/export。attempt33/34 的完整相机 payload hash 漂移失败保留为负向证据；`CameraCalibrationRef@1` 已消除跨轮次浮点复制阻断，但不补齐 attempt35 的 compare 真值绑定。当前单 Part proposal 已改为使用选中 Part 的本地边界宽高和质心误差，而不是全身包围盒，且通过独立单测；这只改善编排信号，不证明新模型质量。脱敏证据见 `docs/evidence/mcp010f/real-codex-cli-silhouette-first-20260813-attempt35-detail-camera-ref.json`；same-observation packaged Viewer/human/PBR/export/360 仍未运行。MCP010A/010B 的历史 Dev.app receipts仍原样保留，不能替代 C/D/E/F packaged/live/Viewer evidence。C/E/F synthetic/raw/CLI transport 与 Agentic prepare/readback receipt 都不证明用户机器人 likeness、PBR likeness、独立人评、Repair execution 或 360°。

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

2026-08-14 Agentic observation binding：没有 active snapshot 时，Runtime 仅在项目没有候选或只有一个候选时保留兼容读取；多个候选必须显式绑定 candidate ID，否则 `scene_observe_get` fail closed。该规则只保护 canonical observation lineage，不代表 Agentic orchestrator、Repair execution 或视觉质量通过；证据为 `docs/evidence/mcp010f/agentic-observation-binding-20260814.json`。
