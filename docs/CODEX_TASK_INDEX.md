# ForgeCAD 当前原子任务索引

2026-08-15 `FGC-MCP010F` current-source package/live boundary：按提交 `a153dc02` 重建并安装同源码 `ForgeCAD Runtime Dev.app`，Runtime/MCP/Geometry Worker/Render Worker cohort 为 `04bbb4d6…ce81f3`；ad-hoc deep-strict、source_worktree_dirty=false、隔离 Runtime Ready/project_create/preflight 以及 packaged MCP010B raw stdio（12 个 registry Skill、37 read + 24 opt-in write = 61 工具、GeometryProgram@2、strict readback、candidate binding 与负向 Part sink）均 PASS，source/package manifest exact hash 为 `13042015…29dc1` / `aa6da7e4…852c0d`。raw probe 同步修复了先读 `ponytail-preflight@0.1.0` 的会话顺序，并改为与 checked-in registry 精确比较 Skill 集合；证据为 `dev-app-install-primary-form-recovery-20260815.json`、`dev-app-probe-primary-form-recovery-20260815.json`、`packaged-primary-form-recovery-raw-stdio-20260815.json`。当前已存在的 Codex Desktop MCP session 仍是旧 `7f9e4c…ee518` cohort，live rebind 继续 `BLOCKED_RESTART_REQUIRED`；没有新的真实参考/视觉 receipt，质量仍 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`，人评/PBR/export-restart/360 未运行或阻断。

2026-08-15 `FGC-MCP010F` Primary Form recovery coverage：当 Runtime 的联合 evidence proposal 未严格改善时，64 次硬预算不再为每个 Part 都消耗一次 group trial；现在保留 boundary-dominant Part 的一个耦合回退，再为 36-control detail Rig 保证一次 `36/36` scalar probe coverage。新增 recovery-budget regression、Primary Form integration、Agentic action regression 与 `script/test_mcp010f.sh` full Gate 均通过；证据为 `docs/evidence/mcp010f/primary-form-recovery-coverage-20260815.json`，当前源码 hash 为 `531783a0…1ae53b`。这是 source/convergence Gate，不是新的视觉 receipt；package/live MCP 仍需按当前源码 revision 重新建立，质量仍 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`，人评/PBR/export-restart/360 未运行或阻断。

2026-08-15 `FGC-MCP010F` live/package cohort alignment：按当前 `abae43f3` 源码 revision 重建并安装 `ForgeCAD Runtime Dev.app`，MCP/Runtime/Geometry Worker/Render Worker 共用 cohort `5a1f108a…e2dd2f`；ad-hoc deep-strict、资源 allowlist、隔离 Runtime Ready/project/preflight probe 均通过，且未触碰持久用户数据。新包工具面为 37 read + 24 opt-in write。当前已建立的 Codex MCP 会话仍返回旧 `7f9e4c…ee518` cohort、旧 manifest `05fca3…d4d0a` 与旧 write surface，须重启/重新建立 MCP 会话后才能做 live authoring；该对齐模块不产生新的 likeness receipt，质量仍 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`，真实视觉闭环仍未运行。

2026-08-15 `FGC-MCP010F` Primary Form resolution-consistent fit：Runtime-owned `silhouette_fit_prepare` 的初始相机邻域与 Geometry Worker proposal 评估现在统一通过隔离 Render Worker 的 512×512 fit batch；非 Primary Form 的旧 camera 粗搜仍保留 128×128。此前 Primary Form 在 128×128 上排序、最终在 512×512 same-camera acceptance 上重算，可能出现 fit winner 被最终门拒绝而不收敛。当前修复只消除 objective resolution drift，未产生新的真实 likeness receipt；质量仍 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`，人评/PBR/export-restart/360 未运行或阻断。

2026-08-15 `FGC-MCP010F` Viewer Agentic evidence binding hardening：当 Viewer 已有完整 candidate-bound visual evidence 时，`normalizeAgenticDesignProjection` 现在要求 Runtime projection 的 artifact/reference/RenderSet/comparison/QualityReport hash 全部精确匹配；缺失或漂移立即转为 `AGENTIC_EVIDENCE_BINDING_MISMATCH`，不再以 null 兼容通过。Viewer source Gate、Node exact-hash behavior check、TypeScript/build 与 MCP010F full Gate 通过。该修复不产生新的 likeness receipt，质量仍 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`，人评/PBR/export-restart/360 未运行或阻断。

2026-08-15 `FGC-MCP010F` Render Worker fail-closed test boundary：Runtime 的 `render-core` 回退不再因 `cfg(test)` 自动启用，新增独立 `test-render-worker-fallback`；legacy `test-geometry-worker-fallback` 仅显式组合该 feature。无 feature 的 Runtime product `cargo check`、Render Worker source ownership checker、Runtime `122 passed / 0 failed / 12 ignored`（显式 legacy feature）、MCP `56 passed / 0 failed`、MCP010C/F 聚合门均通过。该修复证明普通测试不会静默替代 isolated Render Worker，但不产生新的 likeness receipt；F 仍唯一 `in_progress`，质量仍 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`，人评/PBR/export-restart/360 未运行或阻断。

2026-08-15 `FGC-MCP010F` InProcess canonical observation dispatch closure：MCP 的 `InProcess` adapter 现在把 `design_stage_plan_get`、`critic_report_get`、`visual_evidence_bundle_get` 的 `observation_sha256` 转发到 Runtime bound projection；此前 stage/critic 会落到未绑定方法，visual evidence 没有对应分支。新增 stale/missing hash regression，MCP 全量 `56 passed / 0 failed`；IPC Runtime 既有 dispatch 语义保持不变。该修复只关闭开发/测试 transport 的观察碎片化回退，不产生新的 likeness receipt，F 仍唯一 `in_progress`，质量仍 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`，未 confirm/version/export。

2026-08-15 `FGC-MCP010F` Primary Form output-level offset sink：Runtime 对 `offset_x/offset_y/offset_z/scale` 不再改写 mirror/array 之前的源节点；当 Part 输出没有最终 Transform 时，Runtime 在完整 typed output graph 后追加并复用 `forgecad.geometry.transform@2`，让 camera-plane offset 作用于语义 Part 整体而不是把双侧镜像拉开。新增 mirror/direct-output materialization regression，source unchanged、output Transform、part-output lineage 均通过；该 source 修复没有新的真实 likeness receipt，F 仍唯一 `in_progress`，质量仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`，未 confirm/version/export。

2026-08-15 `FGC-MCP010F` Primary Form exact evaluation ledger：Runtime 将 64 上限的初始 camera phase 对齐到实际 15 个 deterministic variants，并把每个 winner-refit camera row 计入执行账本，即使 same-camera strict gate 拒绝该 proposal；当前分配为 `40 geometry + 15 initial camera + 9 winner refit = 64`。新增 fixture 断言不再接受历史 `63` undercount。该模块只修复 bounded search 的预算/证据一致性，不产生新的真实 likeness receipt，F 仍唯一 `in_progress`，`QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`INCOMPLETE_TRUTH_BINDING`/`BLOCKED_INCOMPLETE_BINDING` 与 human/PBR/export-restart/360 状态不变。

2026-08-15 `FGC-MCP010F` Primary Form 单 Part orchestration boundary：`scripts/probe_mcp010f_part_correction.py` 已收口为 `baseline → camera_fit → same-camera compare → scene_observe → Rig hash → primary_form_repair_prepare` 单次 Runtime-owned repair；移除 Codex/Python 的 fraction candidate loop、GeometryProgram patch、`silhouette_candidate_compare` 和本地 metric retention。`script/test_mcp010f.sh` full Gate、desktop build/typecheck 与 focused Runtime tests 通过。此模块没有新真实授权参考或 likeness receipt，F 仍唯一 `in_progress`，`QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`INCOMPLETE_TRUTH_BINDING`/`BLOCKED_INCOMPLETE_BINDING` 与 human/PBR/export-restart/360 未运行或阻断状态不变；旧单部件 receipts 保留为历史证据。

2026-08-15 `FGC-MCP010F` Primary Form profile-loft height sink correction：Runtime `materialize_rig_geometry_program` 现在把 `height` 正确映射到 `forgecad.geometry.profile-loft@1` 的 `profiles[*].height_m` 跨度；不再把高度提案错误写入截面点的局部深度坐标，`profile-extrude` 既有行为保持不变。新增回归与全部 7 个 Rig materialization tests、Runtime `122 passed / 0 failed / 12 ignored`、MCP010F full Gate 通过；Stage 0 truth hash 已同步。该 source Gate 不产生新的真实 likeness receipt，`QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 与人评/PBR/export-restart/360 状态保持不变。

2026-08-15 `FGC-MCP010F` Durable Agentic observation lineage hardening：`session_create_or_resume`、`session_get`、`checkpoint_prepare` 与 bounded action 现在要求观察中的 candidate/reference ID、candidate canonical hash、reference object hash 与 reference canonical hash 全部等于当前 Runtime 记录；缺失或漂移均返回明确的 fail-closed binding error。新增 exact-lineage regression、Agentic projection 回归和 bounded action 回归通过。该修复不新增 Schema/tool/CAS 用户数据，不执行 Repair/confirm/version/export，也不改变 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 或人评/PBR/export-restart/360 未运行状态。

2026-08-15 `FGC-MCP010F` Agentic observation cache：Runtime 按 `AgenticSceneObserveResult@1.canonical_sha256` 保留同一进程内的完整只读观察；bound plan/critic/evidence/action follow-up 优先消费原对象，避免 Codex 在一次观察后重新拼接多个投影。Runtime 重启时 cache 不持久化，只有重新构建并通过 canonical hash/scope 校验才可继续；cache ambiguity regression 与 bounded action regression 通过。该修复不改变唯一 `in_progress`、工具/Schema 数或真实视觉真值：仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 `FGC-MCP010F` Primary Form composition lineage projection：CLI 序列现在生成 `ForgeCADPrimaryFormCompositionLineage@1`，以 canonical hash 固定 2–3 个 serial steps；完成第二步起，下一步 candidate 必须来自已校验的 prefix lineage，并 fail closed 检查 candidate、consolidated observation、target、camera、Rig 与 intent 链。Codex 不再从散乱 raw events 重建连续搜索状态。该投影不等于 Runtime durable producer、视觉 likeness 或高质量 PASS；唯一 `in_progress` 仍为 F，授权 PNG 缺失，真实组合为 `NOT_RUN/BLOCKED_REFERENCE_BYTES_UNAVAILABLE`。

2026-08-15 `FGC-MCP010F` Primary Form bilateral landmark projection：Runtime 修复 landmark Part-ID 解码的 alias gap；语义 `knee-pair`/`hand-pair` 会聚合 Render Worker concrete `*-left`/`*-right` masks，随后再计算 left/right anchor coverage 与 NME。新增双侧 landmark regression；这是 observation/convergence boundary 修复，不改变唯一 `in_progress=FGC-MCP010F`，没有新的真实 likeness receipt，Stage 0 仍明确 `camera=MISMATCH`、`QUALITY_TARGET_NOT_MET`、`BLOCKED_INCOMPLETE_BINDING`，人评/PBR/export-restart/360 未运行或阻断。

2026-08-15 `FGC-MCP010F` modular Primary Form composition boundary：`scripts/probe_mcp010c_codex_cli.py` 新增 `--part-contour-sequence`，仅接受 2–3 个不重复的 exact Part ID；每一步都重新绑定同一 project/target/current candidate 的 `target → camera_fit → baseline compare → scene_observe → SilhouetteRig` consolidated observation，然后只发出一次 Runtime-owned `primary_form_repair_prepare`。只有 `prepared` 才把 staged candidate 作为下一步 source，`no_improvement` 保留 source candidate 与 source camera；Codex 不接收连续参数轨迹。Runtime focused continuation regression、Runtime 全量 `117 passed / 0 failed / 12 ignored` 与 MCP010C Gate 通过；raw stdio probe 改为读取 Runtime-owned tool manifest summary，receipt 动态记录 read/write/total 数。Stage 0 Runtime source hash 已与当前源码同步，`check_mcp010f_stage0_truth.py` 通过；它仍明确 `camera=MISMATCH`、`QUALITY_TARGET_NOT_MET`、`BLOCKED_INCOMPLETE_BINDING`。未取得用户授权 PNG，真实组合 sequence 仍 `NOT_RUN/BLOCKED_REFERENCE_BYTES_UNAVAILABLE`，未 confirm/version/export。

2026-08-15 `FGC-MCP010F` hip-pair bounded candidate trial：r21 `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-canonical-observation-hip-pair-r21.json` 在 cohort `726153a3…42ab5c` 完成 60 次 Runtime-owned bounded fit，`PASS_SILHOUETTE_FIT_TO_COMPARE`，source/proposal loss `0.426350916959`→`0.426265642648`，严格接受并生成 staged candidate `candidate-2dc5728da7124288b6ff79a0fb6b4f00`。全局 compare 为 IoU `0.743473474034`、Boundary F1 `0.301145366407`、bbox `0.0234375`、centroid `0.021831234764`，仍 `QUALITY_TARGET_NOT_MET`、hard gate false；未 confirm/version/export，PBR、人评和 360 仍未运行或阻断。该单 Part receipt 与 r19/r20 是相互独立的 candidate trial，不代表跨项目合并；下一步须先选择最佳 baseline 再继续。

2026-08-15 `FGC-MCP010F` chest-shell bounded candidate trial：r20 `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-canonical-observation-chest-shell-r20.json` 在 cohort `726153a3…42ab5c` 完成 61 次 Runtime-owned bounded fit，`PASS_SILHOUETTE_FIT_TO_COMPARE`，source/proposal loss `0.426350916959`→`0.416406210214`，严格接受并生成 staged candidate `candidate-d9d8cb24b1c5475d85b773c3ceb032c7`。全局 compare 为 IoU `0.739507057324`、Boundary F1 `0.314765400977`、bbox `0.013671875`、centroid `0.020515796935`，仍 `QUALITY_TARGET_NOT_MET`、hard gate false；未 confirm/version/export，PBR、人评和 360 仍未运行或阻断。F 仍唯一 `in_progress`，下一步评估 `hip-pair` 单个 bounded trial。

2026-08-15 `FGC-MCP010F` upper-arm-right bounded candidate trial：r19 `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-canonical-observation-upper-arm-right-r19.json` 在 cohort `726153a3…42ab5c` 完成 63 次 Runtime-owned bounded fit，`PASS_SILHOUETTE_FIT_TO_COMPARE`，source/proposal loss `0.426350916959`→`0.423630597587`，严格接受并生成 staged candidate `candidate-f1123cb13b624cb69493f825398b4423`。全局 compare 仍为 IoU `0.744938326895`、Boundary F1 `0.307109823312`、bbox `0.021484375`、centroid `0.021832396893`，`QUALITY_TARGET_NOT_MET`、hard gate false；未 confirm/version/export，PBR、人评和 360 仍未运行或阻断。F 仍唯一 `in_progress`，下一步继续按推荐 Part 做单个 bounded trial。

2026-08-15 `FGC-MCP010F` pelvis bounded trial and camera-handoff repair：真实 r16 receipt `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-canonical-observation-pelvis-r16.json` 因首次 `reference_get` 瞬时 IPC 失败而按规则 `BLOCKED`，随后重试 r17 暴露 CLI 在 `primary_form_repair_prepare=no_improvement` 时把未采用的 proposal camera 错当成 source compare camera，receipt 保留为 `BLOCKED`。`scripts/probe_mcp010c_codex_cli.py` 已修复为仅在 `prepared` 产生新候选时切换 repair camera；r18 `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-canonical-observation-pelvis-r18.json` 通过 `PASS_CAMERA_FIT_TO_COMPARE`、canonical observation 26 inferred Parts、`boundary_error_get=0`，但 pelvis proposal loss `0.435730135362`→`0.438891599478` 变差，Runtime 返回 `no_improvement`/`retained_source`，最终 `QUALITY_TARGET_NOT_MET`，未 confirm/version/export。F 仍唯一 `in_progress`，下一步按推荐 Part 继续单个 bounded trial。

2026-08-15 `FGC-MCP010F` canonical observation Part attribution closure：Runtime 对 `SilhouetteTarget.parts=[]` 生成 candidate-bound inferred Part error rows；CLI silhouette turn 先执行同 camera baseline `reference_compare_prepare`，再执行唯一 canonical `scene_observe_get`，后续不再调用 `boundary_error_get`。真实 r15 receipt `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-canonical-part-observation-r15.json` 在 cohort `726153a3…42ab5c` 返回 26 个 inferred Parts、`boundary_error_get=0`、`boundary_error.source=canonical_observation`；最终 `QUALITY_TARGET_NOT_MET`，F 仍唯一 `in_progress`，下一原子任务是按推荐 Part 继续单个 bounded repair，不能放宽质量门或把连续搜索交给 Codex。

2026-08-15 `FGC-MCP010F` right-shoulder bounded candidate trial：current packaged cohort `26354e2f…a2029` 的真实 r11 supplemental receipt 对 `shoulder-armor-right` 完成 exact Part contour、63 次 Runtime-owned bounded fit、strict same-camera source/proposal acceptance；loss `0.426350916959`→`0.409541676181`，生成 `staged_new_candidate`，但全局质量仍 `QUALITY_TARGET_NOT_MET`（IoU `0.749072115206`、Boundary F1 `0.326405552646`），`visual_review=needs_revision`，未 confirm/version/export。该原子增量完成右肩 trial evidence，F 仍唯一 `in_progress`；下一项为剩余高误差 Part 的 canonical-observation 归因与模块化修复，不得放宽质量门。

2026-08-15 `FGC-MCP010F` exact-side sink alias hardening：Runtime `materialize_rig_geometry_program` 增加固定、side-aware 的 `*-armor-left/right ↔ *-left/right` alias projection，避免 exact single-Part trial 因 detail sink 命名差异而产生零应用改动；新增 asymmetric alias regression、既有 bilateral materialization 与 exact-side scope regressions 全部通过。无新真实视觉 receipt，F 仍唯一 `in_progress`，质量仍 `QUALITY_TARGET_NOT_MET`。

2026-08-15 `FGC-MCP010F` bounded single-Part candidate trial：已在既有 `primary_form_repair_prepare`/Job 上增加可选 `part_id`，Runtime 从 bilateral Rig 派生 exact Part scope，避免其他 Part 进入连续参数搜索；同一 source/proposal camera acceptance 仍是唯一候选接受门。真实 r10 supplemental receipt `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-part-contour-trial-r10.json` 在 cohort `d2b67cd8…02a2f1` 对 `shoulder-armor-left` 实际执行，结果 `retained_source`/`no_improvement`，未创建 candidate/version，Part contour readback `proposal_ready`；顶层因无关 side-effect ledger 为 `BLOCKED`，quality `NOT_RUN`。该原子增量完成 source/transport/strict-retention 验证，但 F 仍唯一 `in_progress`，真实 likeness 仍未通过。

2026-08-15 `FGC-MCP010F` 单 Part contour/semantic asymmetry 增量：Runtime 已支持 bilateral alias-aware boundary projection 与 exact single-Part `SilhouetteRig@1` scope；真实 r8 receipt `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-part-contour-r8.json` 在 cohort `dd78d216…e5088` 通过 `silhouette_rig_hash → part_contour_fit_prepare`，26 Parts、`proposal_ready`、`PASS_BOUNDARY_ONLY`、无持久化/无关副作用。该增量只完成 typed/read-only 归因与提案链路，未运行视觉质量，不改变 `QUALITY_TARGET_NOT_MET`、`BLOCKED_INCOMPLETE_BINDING` 或 F 的唯一 `in_progress` 状态；下一原子任务为 bounded single-Part candidate trial 与严格 same-camera non-regression gate。

2026-08-15 `FGC-MCP010F` Primary Form 36-control Rig 真实复放：packaged cohort `106a2889…67b08e` 的 supplemental r7 完成 56 次 bounded fit；新增 hip width/height、hip/pelvis/chest offset 控制后仍为 `acceptance_status=retained_source`，source/proposal loss `0.40710507361`/`0.413276643194`，未创建 candidate/version，`persistent_user_data_touched=false`。Viewer current-cohort read-model 与 camera/Render Worker 结构绑定通过，但 final IoU `0.746739614479`、boundary F1 `0.34284083431`，质量仍 `QUALITY_TARGET_NOT_MET`；F 继续唯一 `in_progress`。

2026-08-15 `FGC-MCP010F` Primary Form same-camera acceptance 真实复放：新 packaged cohort `e7e466f6…b7cc` 的 supplemental r6 经过 56 次 bounded fit evaluations；最终 paired acceptance 比较 source loss `0.40710507361` 与 proposal loss `0.412782572291`，Runtime 保留 source、未创建 candidate/version。r6 顶层 `BLOCKED` 由无关 side-effect 账本触发，不能作为 transport PASS；camera binding、Render Worker cohort 与 Viewer read-model 绑定仍为结构 PASS，质量继续 `QUALITY_TARGET_NOT_MET`，F 仍唯一 `in_progress`。

版本：2026-08-15
状态：唯一任务状态表；MVP host golden path 与 FGC-MCP010A 已收口；FGC-MCP010B 结构实现已通过、Darwin OS 总内存硬门 deferred；FGC-MCP010C source-focused 已完成；FGC-MCP010D/E source-focused 已通过；FGC-MCP010F source-focused in_progress，packaged/人评/360 子门保留。ADR-0026 的 Agentic Design Runtime 已完成 observe/plan projection、嵌套只读 projection conformance 与 durable session/checkpoint/RepairIntent prepare/readback slice；durable/reference/DesignSpec 完整 producer、单动作 orchestrator 和 Repair 应用 backlog 尚未改变当前唯一任务状态。

2026-08-15 `FGC-MCP010F` QualityReport/packaged cohort consistency：`AppearancePrepare` 的 `visual_status=not-run` 现在与 `hard_gate_passed=false` 严格绑定；同一 packaged cohort `fee79807…` 的 raw F probe 通过 26 parts/4704 triangles、7 material zones、九 AOV、embedded textures、Render Worker binding 和 Viewer read-model。该结构门不等于视觉 likeness 门，任务继续 `in_progress`，真实质量仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 `FGC-MCP010F` Primary Form 多尺度 geometry convergence：Runtime-owned `GeometryProgram` 搜索在 `max_evaluations=64` 时改为 `40 geometry + 15 initial camera + 9 winner refit`，并对 coordinate probe 使用 `1.0 → 0.5 → 0.25` deterministic refinement scales；总预算仍硬封顶，Codex 不接收连续参数轨迹。focused budget/scale regressions 通过；没有新的真实 likeness receipt，任务仍保持 `in_progress`，质量仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 `FGC-MCP010F` Primary Form bounded continuation：Runtime action/job 的 `max_iterations` 上限由 1 提升为 2，但 `max_evaluations` 仍固定封顶 64；第二轮只在 Runtime 内部围绕第一轮 incumbent 继续 camera/geometry coupling，不向 Codex 返回连续轨迹。端到端 fixture 要求 63–64 evaluations 且 `iterations=2`；没有新的真实 likeness receipt，当前历史质量仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 `FGC-MCP010F` camera-fit→compare automatic handoff：Runtime 在同一 `project_id/candidate_id/target_sha256` 下复用最近一次 `camera_fit_prepare` 的 Runtime-owned `selected_camera`；`reference_compare_prepare` 未显式携带 camera 时不再回到 default camera 或重新推导另一套 framing。没有 exact cache 或 target 时仍保持 bounded default fallback；显式 `CameraCalibrationRef@1` 与完整 camera 路径不变。新增自动复用回归，目标是关闭 fit/compare camera hash 漂移；当前历史真实 receipt 仍保持 `camera=MISMATCH`，必须用新同 cohort transport 重新验证后才能更新质量账本。

2026-08-15 `FGC-MCP010F` Primary Form asynchronous convergence Job：新增 `primary_form_repair_job_prepare`，将一次可能超过 MCP IPC deadline 的 Runtime-owned 64-evaluation search 变为 queued `RuntimeJob@1`；后台 execution context 复用现有 Geometry/Render Worker、strict readback 和 same-camera acceptance，终态结果进入 CAS，由 `job_get`/`job_events_read`/`job_result_get` 读取。Store 使用原子 queued/terminal event，取消不会被后台结果覆盖；当前只保证进程内异步解耦，不提供跨重启续跑、不执行 confirm/version/export，也没有新的 likeness receipt。MCP/Runtime/Store tests 与 Stage 0 通过，工具面更新为 103 Schema、37 read + 24 opt-in write = 61；质量仍 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 `FGC-MCP010F` Render Worker cohort-to-RenderSet evidence：Runtime sibling launcher 现在把 isolated `forgecad-render-worker` 返回的 `build_cohort_sha256` 保留给 Render adapter；perspective `RenderSet@2` 写入 `render_worker_build_cohort_sha256` 与 `render_worker_binding_status`，Runtime 对 cohort/status 漂移 fail closed。source build 没有 cohort 时显式为 `cohort_unavailable`，packaged same-cohort build 才能写 `same_cohort_verified`；Viewer 只读显示该状态，不重算 QualityReport 门。Runtime C regression、contracts、Viewer typecheck、MCP010C/E probe syntax 通过；该模块仍不产生新的 likeness、人评、PBR、export/restart 或 360 证据，F 继续为唯一 `in_progress`。

2026-08-15 `FGC-MCP010F` canonical observation follow-up binding：`design_stage_plan_get`、`critic_report_get` 与 `visual_evidence_bundle_get` 现在必须携带先前 `scene_observe_get` 返回的 `observation_sha256`；Runtime 重新校验 hash 后只切片同一个 `AgenticSceneObserveResult@1`，不再独立重算观察。Agentic evidence tool 统一返回 `VisualEvidenceBundle@1`，Viewer 的 `ViewerVisualEvidence@1` 保留在 Viewer 专用路径。新增 stale-hash/同 observation slice regression；该模块不改变 schema/tool count 以外的质量事实，Stage 0 仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 未运行或阻断。

2026-08-15 `FGC-MCP010F` canonical observation durable handoff：`DesignSession@1` 与 `DesignCheckpoint@1` 现在都持久化同一 `AgenticSceneObserveResult@1.canonical_sha256`，Runtime 在 session get、checkpoint prepare/get/restore 和 action run 前重新生成当前 observation 并对 hash 做 fail-closed binding；跨重启 receipt 验证 session、checkpoint 与 nested projection 使用同一 canonical hash。focused/full Runtime、contracts、receipt checker 与隔离 Runtime/MCP restart probe 通过。该模块修复 Codex observation 被拆散后的 hash 粒度不一致，不等于 durable/reference/DesignSpec 完整 producer、通用 orchestrator、Repair 应用或视觉质量 PASS；Stage 0 仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 未运行或阻断。

2026-08-15 `FGC-MCP010F` RepairIntent observation binding：`RepairIntent@1.observation_sha256` 现在是必填并且必须等于 durable session 的同一 `AgenticSceneObserveResult@1.canonical_sha256`；`checkpoint_restore_prepare` 由 Runtime 生成绑定字段，receipt checker 对 session/checkpoint/RepairIntent 三者做一致性校验。隔离重启 probe 与 contracts/receipt/projection checker 通过；这只补齐 RepairIntent 的 observation lineage，不代表 Repair 执行、完整 orchestrator 或视觉质量 PASS，Stage 0 状态不变。

2026-08-15 `FGC-MCP010F` Primary Form local-group convergence：当 Runtime-owned joint proposal 未改善时，几何预算先按 boundary-priority 将同一 Part 的 width/height/offset 组成一个局部候选，再把剩余预算交给单坐标细化；成功的 Part 候选可组合到当前 Runtime incumbent，仍只经过 typed Geometry Worker 与 Render Worker。新增 group ordering/composition regression；没有新增真实 likeness receipt，Stage 0 仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 未运行或阻断。

2026-08-15 `FGC-MCP010F` Primary Form ranking snapshot：Runtime 现在为 camera batch、geometry probes 和 geometry-winner camera refit 同时保存 public contour metrics 与 transient candidate-bound landmark ranking snapshot；winner、baseline、strict-improvement 都使用同一 snapshot，避免“候选有 landmark、baseline 没有”时默认 coverage=0 造成排序漂移。`SilhouetteFitResult@1` 的公开五项 metrics、schema/tool manifest 和 Viewer 权威边界不变；新增 baseline/candidate snapshot regression，Runtime 全量 `105 passed / 12 ignored`。该模块只修复 Runtime 内部收敛判定，没有新的真实 likeness receipt，Stage 0 仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 未运行或阻断。

2026-08-15 `FGC-MCP010F` observation/camera handoff：`design_action_run_prepare` 现在要求 Codex 提交它实际消费的 `observation_sha256`；`input_sha256` 将该 hash 纳入 action binding，Runtime 重新生成并校验当前 `AgenticSceneObserveResult@1`，过期或不匹配立即 fail closed。Primary Form 的 `base_camera` 不再无条件回退 default camera，而是按 session 的 `camera_hash` 从同一 candidate/target-bound `camera_fit_prepare` 证据解析；找不到精确 camera binding 也 fail closed。MCP schema/负向 fixture、Runtime stale-observation 与 action round-trip focused tests 通过；该模块不改变视觉质量，Stage 0 仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 未运行或阻断。

2026-08-15 `FGC-MCP010F` Runtime quality-authority projection：Runtime 将 visible-view gate 的 metric、方向、阈值和 pass/fail/not-run 状态集中在 `visible_view_gate_checks`；`visible_view_gate_passes` 与 Agentic critic 共用同一检查记录，删除 Agentic 层重复的 0.90/0.02/0.85 等阈值判断。新增一致性 regression；focused/full Runtime、MCP010F source Gate 通过。该模块不改变 schema/tool manifest、Viewer 仍只读消费 Runtime QualityReport，真实结果继续为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 仍未运行或阻断。

2026-08-15 `FGC-MCP010F` Primary Form metric-priority retention：Runtime 的 camera fit、SilhouetteFit Rig 搜索、geometry/refit 选择、same-camera acceptance 和 candidate compare 现在共享 `boundary_f1_4px → silhouette_iou → bbox/centroid → landmarks → regions` 的确定性优先排序；旧 scalar loss 的 Chamfer/IoU 主导顺序已修正为 boundary-first，低优先级 IoU/Chamfer 不能再交换掉边界改善。新增 regression 证明边界改善即使伴随 IoU 下降也保留；Primary Form focused 14 passed，完整 Runtime 预期为 `104 passed / 12 ignored`。该模块不改变 schema/tool manifest、真实 likeness receipt、Viewer quality authority 或 Stage 0 的 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 仍未运行或阻断。

2026-08-15 `FGC-MCP010F` Primary Form 双向边界证据：Runtime 的 candidate-bound boundary observation 现在在固定上限内同时采样 target→model 与 model→target 两个方向；model-owned excess/displaced edge 也保留同一 candidate/camera/Part-ID 绑定，再由既有 per-Part coverage 与最多 64 条 deterministic selection 投影给 bounded Rig sweep。新增 regression 证明模型独有边缘不会被单向最近点投影丢失；`forgecad-runtime` 全量 `103 passed / 12 ignored`。该模块不改变 schema/tool manifest、Render Worker typed boundary、Viewer read-only quality authority 或真实 likeness receipt；Stage 0 仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 仍未运行或阻断。

2026-08-15 `FGC-MCP010F` Render Worker ownership refactor：Runtime 的通用 sibling transport 已从 `geometry_worker.rs` 抽成 `execute_sibling_worker`；`forgecad-render-worker` 二进制身份和 `execute_render_worker` adapter 现在只由 `render_worker.rs` 持有，Geometry Worker 不再知道 Render Worker。新增两个要求同 cohort sibling 的隔离 conformance（九 AOV/512px/PNG/重复确定性、拒绝 GeometryProgram 载荷）；当前 `forgecad-runtime` 全量 `102 passed / 12 ignored`，source boundary checker 与 `git diff --check` 通过。该模块不改变 schema/tool manifest、真实 likeness receipt、Viewer 质量权威或当前 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`、human/PBR/export-restart/360 未运行状态。

2026-08-15 `FGC-MCP010F` Primary Form evidence-magnitude handoff：Runtime 的 bounded coordinate probe 在第一轮只对当前值尚未达到同一 Runtime boundary proposal 的坐标复用 evidence magnitude，并将单步限制为不超过该 Rig authored span 的 50%；已处于 proposal 的坐标和后续反向 pass 继续使用原有小步。这样 secondary Part 不会只被固定 `step_fraction` 轻微试探，连续搜索仍完全由 Runtime/Workers 承担。新增 magnitude regression，`forgecad-runtime` 全量 102 passed、10 ignored；没有新的真实 likeness receipt，仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 未运行或阻断。

2026-08-15 `FGC-MCP010F` Primary Form boundary evidence coverage：Runtime 不再用全局距离 top-N 直接截断 boundary observation；在同一 `max_segments<=64` 上限内，先为每个有 `part_id` 的可见 Part 保留一条最高误差 segment，再按距离补齐，并保持确定性的距离降序输出。这样后续 bounded multi-Part sweep 能同时看到 shin/head/hand 等部件，不把连续搜索交回 Codex，也不改变 MCP schema/tool count。新增 coverage regression，`forgecad-runtime` 全量 101 passed、10 ignored；没有新的真实 likeness receipt，仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 未运行或阻断。

2026-08-15 `FGC-MCP010F` Primary Form proposal-direction retention：dominant Part 仍只作为 probe zero 的局部 seed；Runtime 现在并行保留完整 boundary projection，后续 secondary-Part coordinate probe 使用各自有界 evidence direction，不再用参数索引交替方向猜测。新增 `primary_form_proposal_direction` regression，显式处理 `-1/0/+1`，避免正零 `signum` 误判为正向；`forgecad-runtime` 全部 100 passed、10 ignored。没有新的真实 likeness receipt，仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 未运行或阻断。

2026-08-15 `FGC-MCP010F` Primary Form bounded multi-Part sweep：保留 candidate-bound 最大边界误差 Part 作为首个局部 seed，但移除后续坐标 schedule 对该 Part 的过滤；Runtime 现在按固定优先级和同一 bounded budget 继续遍历完整 typed Rig，头部、胸部、手部、前臂、膝部等 secondary Part 不再被单次 observation 静默漏掉。新增 seed/sweep coverage regression，`forgecad-runtime` 全部 100 passed、10 ignored；没有新的真实 likeness receipt，仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 未运行或阻断。Agentic action-run 仍保持现有单 Part 合同，未扩成通用 orchestrator。

2026-08-15 `FGC-MCP010F` Primary Form local Part offset calibration：修复 Runtime 局部 Part envelope proposal 的两处收敛缺口：米制 `offset_x/offset_y` 现在必须使用同一 `CameraCalibration@1` 的 camera-plane world scale；图像 Y 向下与 camera-plane up 的符号统一，避免显式 Part target 的纵向 proposal 反向。缺少 camera 时 legacy `part_contour_fit_prepare` 对米制 offset 保持 neutral，不再用 Rig step 伪造世界位移；新增 Runtime focused regression 与完整 `forgecad-runtime` 单测通过。该模块没有 Schema/工具数变化、没有真实视觉 receipt，不改变当前 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING` 或 human/PBR/export-restart/360 未运行状态。

2026-08-15 `FGC-MCP010F` Primary Form same-camera retention gate：Runtime 在 staged candidate 之前，用最终选定 camera、同一 512px Render Worker、同一 reference target 和同一 weighted loss 对 authored source GLB 与 proposed GeometryProgram 做非持久化比较；只有 proposal 严格优于 source 才进入 `geometry_prepare`。新增 `PrimaryFormAcceptance@1` 结果绑定 source/proposal program/camera/loss/status；相机补偿或 full-resolution 回归会返回 `retained_source`、`no_improvement`，不写 candidate/VisualEvidence。合同与 Runtime focused regression 已通过；该模块仍不改变真实机器人 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 或 human/PBR/export-restart/360 未运行事实。

2026-08-15 `FGC-MCP010F` Agentic observation→action receipt binding：`design_action_run_prepare` 将同一回合唯一 `AgenticSceneObserveResult@1.canonical_sha256` 写入不可变 `DesignActionRun@1.observation_sha256`，并由 Store 校验/回读；Runtime focused round-trip 与 Agentic contract 缺字段负向回归已加入。该模块收口观察、候选、证据与 action receipt 的同一 hash 链，不改变 Primary Form 的搜索预算、Viewer 权威边界、`QUALITY_TARGET_NOT_MET`、`MISMATCH` 或 `BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 `FGC-MCP010F` Primary Form convergence evidence alignment：`SilhouetteFitResult@1` 现在包含 Runtime-owned baseline/selected loss 与 metrics、camera/geometry evaluation breakdown 和 strict-improvement retention；action-run evaluate stage 通过 Primary Form result hash 指向完整收敛证据。`DesignActionRun@1` stage contract 与 Store 真实字段对齐，并新增 skipped/legacy-field negative checks。该模块通过 contracts、Agentic/MCP、Stage 0、Runtime action/silhouette focused tests，仍不改变 `QUALITY_TARGET_NOT_MET`、`MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`，F 继续为唯一 `in_progress`。

2026-08-15 `FGC-MCP010F` Agentic bounded action-run slice：新增 Runtime-owned `design_action_run_prepare`/`design_action_run_get`。当前只允许已批准、单 Part、`primary-form` bounded repair；一次运行复用 Runtime 的 Rig、Geometry/Render Worker、strict readback 和 candidate-bound compare，并写入不可变 SQLite/CAS action receipt。MCP 要求 session/project/candidate/reference scope 与显式 write opt-in；结果锁定 confirm/export，不改变 candidate/version。Runtime focused idempotent round-trip、MCP boundary/manifest、Stage 0 通过；通用多阶段 orchestrator、Repair 应用、durable/reference/DesignSpec 完整 producer、真实 Codex action loop 和视觉质量仍未完成，F 继续是唯一 `in_progress`。

2026-08-15 `FGC-MCP010F` automatic-target Part correction：Runtime `part_contour_fit_prepare` 对无显式 Part contour 的 automatic target 复用 candidate-bound silhouette/Part-ID boundary projection；`shin-pair` bounded source patch、同 cohort real receipt 与 baseline retention 已通过。没有候选被接受或持久化，F 仍 `in_progress`，质量仍 `QUALITY_TARGET_NOT_MET`。

2026-08-15 `FGC-MCP010F` Primary Form action budget 修复：真实修复前 receipt 发现 detail action 请求 64 次却被 Runtime 外层 cap 截断为 24；源码恢复同一 bounded ceiling `64`，当前 action/job 只允许最多 2 个 Runtime continuation iterations，并增加 optimizer normalization 与 `primary_form_repair_prepare` 端到端消费预算回归（fixture 实际 63–64 次）。Dev.app/package cohort `c521bf28…c4a5` 通过安装校验；修复后真实 Codex 复跑在 geometry hash/prepare authoring sequence 阻断，未产生新的视觉结果。该原子模块只修正 Runtime action budget，不改变 Stage 0 的 `QUALITY_TARGET_NOT_MET`、`camera=MISMATCH`、`benchmark=BLOCKED_INCOMPLETE_BINDING`，F 仍为唯一 `in_progress`。

2026-08-15 `FGC-MCP010F` packaged Render Worker landing：同 cohort Dev.app 已实际携带并校验 `forgecad-render-worker` Resource；packaged Runtime raw stdio probe 通过 sibling Worker 完成九 AOV、固定 renderer、determinism、compare 和 image-block transport。三份证据已纳入 MCP010F manifest。该模块只关闭 packaged Worker resource/process/protocol 的结构性缺口；synthetic reference 的质量仍 `QUALITY_TARGET_NOT_MET`，不推进人评/PBR/export-restart/360，也不把当前 `in_progress` F 任务标为 done。

2026-08-14 Primary Form 联合 proposal 回退：Runtime 在一次 candidate-bound dominant Part 的联合 width/height/offset proposal 未严格改善时，按固定 `1.0 → 0.5 → 0.25` 比例回退到 authored baseline，再继续 bounded coordinate probes；所有值仍受 Rig min/max、总评估预算和 Worker/readback Gate 约束。新增插值边界 regression 与 focused/full Runtime 通过；无新视觉 receipt，不改变 `QUALITY_TARGET_NOT_MET`、`MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 或未运行门。

2026-08-14 Primary Form bilateral Part-ID projection：修复 Runtime boundary proposal 只按精确 Part-ID 查找的问题；`shin-left/right`、`shoulder-armor-left/right` 等 Worker 输出现在会聚合到 `shin-pair`、`shoulder-armor-pair` Rig Part，局部 envelope 才能驱动同一 Part 的宽度/高度/偏移 proposal。pair focused/full Runtime、MCP010C/F source Gate 通过；无新视觉 receipt，`QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING` 和未运行门状态不变。

2026-08-14 Primary Form 单 Part action scope：Runtime 将主导 Part 的 candidate-bound boundary evidence 作为本次 bounded repair 的唯一可变范围，非聚焦 Part 的参数恢复 authored baseline；critic projection 的 repair operation 改为直接指向 `primary_form_repair_prepare`，Codex 只提交一次 typed intent。focused/full Runtime、Agentic contracts、MCP010C/F source 与 Render Worker boundary Gate 通过；无新视觉 receipt，质量、camera、benchmark 和未运行门状态保持不变。

2026-08-14 Primary Form 误差归因排序修复：Runtime 在同一候选的 Part-ID boundary evidence 上聚合每个 Part 的固定距离分数，并让 bounded Rig coordinate schedule 先覆盖主导可见误差 Part，再按参数提案改变量排序；无 Part evidence 时保持原有 deterministic fallback。新增 Runtime regression 验证 `shin-pair` 即使提案改变量较小也先于 `chest-shell`，full Runtime/MCP010F source Gate 通过。新的同 cohort 视觉复验因 r3 receipt 不保留原图字节且当前 workspace 无授权参考文件而 `BLOCKED_REFERENCE_BYTES_NOT_AVAILABLE`，不改变 `QUALITY_TARGET_NOT_MET`、`MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 或未运行的人评/PBR/export-restart/360。

Stage 0 机器真值入口为 `docs/evidence/mcp010f/current-benchmark-truth.json`。当前为 102 Schema、36 read + 23 opt-in write = 59 tools，唯一 `in_progress` 为 `FGC-MCP010F`。attempt35 仅是 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING` 的 provisional retained observation，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，camera 绑定 `MISMATCH`；`primary-form-cas-rerun-20260814.json` 等较早补充 receipts 按历史状态保留，当前 latest pointer 由下方 r3 transport 记录。

2026-08-14 Primary Form 当前 latest transport：`real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json` 已进入 inventory/Stage 0 latest attempt 与 latest completed pointer。它验证 Codex 单次 observation/camera/Rig 后只提交一个 `primary_form_repair_prepare`，Runtime 内部完成 bounded fit、Geometry Worker、Render Worker 和 candidate-bound compare；CLI 消费 Runtime visual evidence，不重复 compare。该 transport 为 `QUALITY_TARGET_NOT_MET`（IoU `0.749122`、Boundary F1 `0.347623`），不替换 attempt35 provisional observation，不解锁 confirm/export/PBR、人评或 360。

2026-08-14 Primary Form 肢段尺度/装配控制扩展：detail probe 的 `SilhouetteRig@1` 从 20 个控制扩展为 26 个，新增上臂/前臂/大腿/小腿高度和肘/膝垂直位置；Runtime 复用已有 typed `height`/`offset_y` DAG materialization 与 bounded search，Codex 不再承接连续参数搜索。同步修正 raw stdio probe 的工具数期望为当前 `36 read + 23 opt-in write = 59`。focused/full Runtime、MCP010C 与干净 worktree 的 MCP010F source Gate 通过；没有新的授权机器人视觉 receipt，当前仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，人评/PBR/export-restart/360 未运行或阻断。

2026-08-14 Primary Form 首轮全控制覆盖 follow-up：detail probe 的 26-control Rig 现在以 `max_evaluations=64` 调用 Runtime；在 GeometryProgram 路径，Runtime 的 `32/16/16` 三段预算让初始证据提案之后完整覆盖 26 个控制，再执行剩余方向试探和相机重拟合。Codex 仍只提交一次 Rig，连续参数搜索不回到 Codex。26-control schedule regression 与 full Runtime 通过，未产生新的授权机器人视觉 receipt；F 仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`，人评/PBR/export-restart/360 未运行或阻断。

2026-08-14 Viewer evidence lineage follow-up：Runtime `visual_evidence` 现在在 Viewer read model 出口统一验证 candidate artifact、RenderSet、comparison report、QualityReport、reference/target 和 camera 的 hash-bound lineage；comparison 缺失、artifact/target/reference/camera 不一致均 fail closed。新增合法 RenderSet 重绑错误 artifact 的 Runtime 负向回归；并修正 `primary_form_repair_prepare` optimizer schema 与 MCP 有界 schema validator 的不兼容。该模块只收口 Viewer 真值边界，不改变 `QUALITY_TARGET_NOT_MET`、`MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 或未运行的 human/PBR/export-restart/360 门。

2026-08-14 Primary Form objective alignment follow-up：Runtime Geometry trial 统一复用 camera 的 landmark/coverage-aware loss，避免几何搜索用 contour-only loss 绕过观测证据；focused 与完整 Runtime 回归通过。新 cohort 真实 Codex 复跑在 authoring sequence 阻断，未产出新的视觉指标，故不升级当前任务的视觉质量或 benchmark 状态。

2026-08-14 Primary Form proposal handoff follow-up：严格优于 authored baseline 的 Geometry Worker 试算现在随 `SilhouetteFitResult@1.selected_geometry_program` 返回；Runtime 校验其 `GeometryProgram@2` project/canonical hash，未改善时返回 `null`。它仍不自动写 candidate/version 或确认，后续必须在用户批准后调用 `geometry_prepare`；合同/Runtime focused/source Gate 已通过，未产生新的 likeness receipt。

2026-08-14 Primary Form bounded schedule follow-up：修复 camera coordinate-descent 在首个不改善批次提前停止的问题；Runtime 现在会继续执行声明的有界 schedule，覆盖后续 roll/FOV/distance/target-offset/global-scale 轴，focused fixture 验证 `max_evaluations=8` 实际完成 8 次评估。该修复只改善 Primary Form 收敛路径，不改变唯一 `in_progress` 任务、Viewer 质量权威边界或当前 `QUALITY_TARGET_NOT_MET` / `MISMATCH` / `BLOCKED_INCOMPLETE_BINDING` 真值。

2026-08-14 Agentic observation consolidation follow-up：Runtime 现在把 target-bound `SilhouetteTarget@1` 的 `target_sha256` 持久化到 visual evidence，并在 Viewer 与 `AgenticSceneObserveResult@1` 中回读同一 hash；`DesignCriticReport@1.primary_form_directive` 聚合固定 F1→IoU→bbox/centroid→landmark→region 优先级，明确 `continuous_search_owner=runtime`，Primary Form 失败只生成一个合并的 Runtime bounded RepairIntent。target-bound Viewer/Agentic round-trip、full Runtime 87/10、Store 8、contracts/Agentic contracts 和 `script/test_mcp010f.sh` 通过；没有执行 Repair、没有改变 Viewer 质量门，也没有新的 likeness receipt，F 仍 `in_progress`。

2026-08-14 Primary Form single-action prepare/evaluate follow-up：新增 Runtime-owned `primary_form_repair_prepare`，Codex 只提交一次 target/camera/Rig/optimizer typed intent；Runtime 在一个有界动作内完成 fit、GeometryProgram 编译、strict GLB readback、隔离 Render Worker 九 AOV 和 candidate-bound compare，返回 `PrimaryFormRepairPrepareResult@1`。成功只产生 staged candidate 和 Runtime `QualityReport@2`，不 confirm、不创建 version、不 export；无严格改善则保持 candidate unchanged。Runtime focused regression、MCP dispatch/manifest 和 Stage 0 truth 通过，真实 likeness 状态不变。
2026-08-14 Primary Form 联合收敛 follow-up：`silhouette_fit_prepare` 的 bounded budget 现在显式保留 geometry-winner camera refit 段；Runtime 在几何试探严格改善后，使用同一最终 Geometry Worker GLB 通过隔离 Render Worker 重新比较局部相机邻域，再把最终 camera/metrics 交给 `primary_form_repair_prepare` 的 candidate-bound compare。三段预算总和不超过 `max_evaluations`，新增预算不透明化为 Codex 参数搜索；Runtime focused budget/refit tests 通过。该修复没有新的授权机器人 receipt，不改变 `QUALITY_TARGET_NOT_MET`、`camera=MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 或未运行的人评/PBR/export-restart/360。

<!-- forgecad-stage0: schemas=103 schema_set_sha256=01218d921dd05574835d5762c8b64c72332b61a58eba6cdb20d0190d4b658a47 read_tools=37 write_tools=24 total_tools=61 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json latest_completed=real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json -->

2026-08-14 Primary Form convergence follow-up：Runtime 修复了 `max_evaluations=24` 实际只消费 16 次的预算截断，并让有界坐标 schedule 在反向试探前覆盖当前 12 个 Rig 参数；CLI receipt 新增 canonical observation 的 target/observation/fit 分阶段 projection。`script/test_mcp010f.sh` 当前 source Gate PASS，Stage 0 仍保留 `QUALITY_TARGET_NOT_MET`、`camera=MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`，没有新增视觉 benchmark。

2026-08-14 Primary Form landmark projection follow-up：Runtime 现在把已知 landmark→Part anchor 的相机平面误差写入有限 `offset_x/offset_y` Rig proposal，并沿 camera basis materialize；detail probe 的 20 参数和 `32 → 21 geometry + 11 camera` 分配仍由 Runtime bounded schedule 执行。新增 Runtime landmark/materialization tests、完整 Runtime 回归、MCP010F source Gate 和 Viewer build 通过；因未重新取得授权机器人端到端 receipt，本任务仍不能宣称视觉收敛或 benchmark PASS。

2026-08-14 同 cohort 真实复跑已验证 Primary Form 合同修复：Runtime 将 authored baseline 的完整 Rig definition 在 `SilhouetteFitResult@1` 输出边界压缩为 `parameter_id/part_id/value`，focused test 与 `script/test_mcp010c.sh`、`script/test_mcp010f.sh` 均通过。receipt `docs/evidence/mcp010f/real-codex-cli-current-20260814-same-cohort.json` 的 44 个 typed MCP 调用完成，`scene_observe_get` 为单次聚合上下文，fit `evaluations=16`；fit/compare camera hash 与 canonical hash 一致，但 IoU `0.690952`、Boundary F1 `0.256758` 仍为 `QUALITY_TARGET_NOT_MET`。一个 Codex turn 返回码为 1，探针因此保守记为 `BLOCKED`，不把该 receipt 晋升为 benchmark 或高质量 PASS。

2026-08-14 Primary Form 相机绑定加固：Runtime 仍以 `128×128` 做有界粗搜，但对排序后的最多五个候选加 authored base camera 通过同一个隔离 Render Worker 批量执行固定 `512×512` 验证，再用完整内部损失选择结果；对外 `CameraFitResult@1` 仍只返回合同规定的四项 silhouette metrics，避免低分辨率 aliasing 直接污染 compare camera。`script/test_mcp010c.sh`、`script/test_mcp010f.sh`、Runtime focused tests 和四个最新同 cohort build identity 均通过（cohort `d11e83cc…e07264`）。此前两次停在 `reference_get` 的重试保留为历史阻断记录。

2026-08-14 Render Worker process/source boundary 修复：Runtime `render_fixed` 先经 Geometry Worker 生成编译 GLB，Render Worker 的窄 façade 拒绝 geometry compile payload；无状态 renderer 已抽到 `apps/render-core`，`forgecad-render-worker` 不再依赖 geometry-worker crate。隔离单测、compiled-GLB 负向 raw probe、source ownership checker、MCP010C focused gate 和 Runtime fixed-render regression 均通过。该原子修复只把 process/protocol/source ownership 记为 PASS；packaged/live C/F、真实视觉、人评、PBR、export/restart 和 360 仍保持 OPEN/NOT_RUN。

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

ADR-0026 重规划规则：`scene_observe_get`、`design_stage_plan_get`、`critic_report_get`、`visual_evidence_bundle_get` 标为 `source/read-only projection PASS`；真实 Runtime 的 scene/stage 嵌套只读 projection 另标为 `nested projection conformance PASS`；`session_create_or_resume`、`session_get`、`checkpoint_prepare`、`checkpoint_get`、`checkpoint_restore_prepare` 另标为 `durable prepare/readback PASS`。证据为 `docs/evidence/mcp010f/agentic-runtime-observe-plan-20260813.json`、`docs/evidence/mcp010f/agentic-runtime-projection-conformance-20260813.json` 和 `docs/evidence/mcp010f/agentic-runtime-session-checkpoint-20260813.json`，校验器分别为 `scripts/check_agentic_projection_receipt.py` 与 `scripts/check_agentic_runtime_receipt.py`。后者不等于 durable/reference/DesignSpec 完整 producer、真正的 Plan→Act→Inspect→Evaluate→Checkpoint orchestrator、Repair 执行、candidate/version mutation 或完整 Visual Evidence conformance；不得把当前切片标为完整 Agentic Runtime/done，也不得把它升级为视觉质量证据。

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
| FGC-MCP010D | done | MCP010C | 11 个真实高细节 Operator、13 项 catalog、`hard-surface-detail@0.2.0` active overlay、隔离 Worker 和 strict lineage/readback；同 cohort packaged D raw structural probe 已通过；boolean/Manifold 与视觉门保留为 NOT_RUN |
| FGC-MCP010E | done | MCP010D | source-focused：first-party 离线硬表面 AssetPack、UV atlas、MikkTSpace、纹理/PBR provenance；xatlas/Validator/packaged/视觉子门 deferred |
| FGC-MCP010F | in_progress | MCP010E | Viewer compare/AOV/Part/MaterialZone/explosion source surface + contour-first Runtime target/Rig/Part compare slice；新增 hash-bound `SilhouetteTarget@1`、唯一非重叠 Part contour slices、64-render coarse-to-local camera fit、Runtime-owned bounded Primary Form search、`SilhouetteRig@1` bounded fit、SDF/Chamfer、single-Part proposal、candidate compare、directional boundary errors 和 MCP dispatch，source/aggregate tests PASS；Render Worker `--isolated-once` 严格单请求、Primary Form X/Y 轴 proposal、Viewer 直接消费 Runtime `QualityReport@2.hard_gate_passed` 已加入回归；已知机器人 landmark 现在通过固定 Part-ID anchor 与 camera/Rig/geometry trial/candidate compare/reference comparison 共用同一瞬时损失；真实机器人基线仍 silhouette IoU 0.7410、boundary F1 0.3288、QUALITY_TARGET_NOT_MET，Viewer packaged/人评/360 仍独立未运行 |

当前 Agentic observation 增量：Runtime 在候选已有 target、RenderSet、Part-ID AOV 和比较证据时，于同一次只读 `scene_observe_get`/`critic_report_get` 内复用 `silhouette_part_error_get` 计算，并将完整 hash-bound `SilhouettePartErrorResult@1`、Runtime 推荐的 `focus_part_id`/visibility 和单一 RepairIntent scope 一并返回。无 typed Part slice 时保持 `part_error=null`、焦点 `unknown`，不推断不可见归属；该增量减少 Codex 的额外诊断调用，不改变 Primary Form 的 bounded search、质量阈值或当前 `QUALITY_TARGET_NOT_MET`。
| FGC-MCP011 | blocked | MCP010F | Job checkpoint/并发/崩溃恢复/配额/GC/全局性能 |
| FGC-MCP012 | blocked | MCP011 | 第三方 Skill 生命周期、外部项目深度治理、分发签名/撤销 |
| FGC-MCP013 | blocked | MCP012 | Developer ID/notarization、clean install、升级回滚、packaged Desktop/CLI、跨类别真人门 |

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

FGC-MCP010D 已完成 source-focused 退出条件，并新增同 cohort packaged D raw structural receipt：11 个真实高细节 Operator、13 项 catalog（12 active，boolean unavailable）、`hard-surface-detail@0.2.0` active、strict lineage/readback、固定同级 Worker 隔离和负向回归均通过；Manifold boolean 与视觉门仍为 `NOT_RUN/BLOCKED`。证据位于 `docs/evidence/mcp010d/`。

FGC-MCP010E 已完成 source-focused 退出条件，并新增 packaged structural 退出证据：65 个合同、`forgecad-hard-surface-robot@1.0.0` 离线 AssetPack、`uv-pbr@0.2.0`、512px bounded UV atlas、固定 `mikktspace@0.3.0`、嵌入式 PNG PBR 通道、九 AOV、raw stdio 和同 cohort 用户参考结构探针均通过。xatlas adoption、Khronos Validator、真实视觉/PBR likeness、独立人评、export/restart hash 与 360°仍 `NOT_RUN/BLOCKED`。证据位于 `docs/evidence/mcp010e/`。

FGC-MCP010F 当前 source-focused in_progress：Viewer 已接入只读 Runtime projection，支持九 AOV、reference/render split/overlay/flicker、显式轮廓画布、与 Runtime `mask-2` 同源的 ephemeral border-connected flood-fill reference-contour aid、Part/MaterialZone 筛选、临时爆炸图、差异热图辅助、contour-first 阶段/累计门提示和 Codex correction queue；TypeScript/Vite/Tauri source Gate 已通过；comparison-sheet 与 hash-bound fit-plan 只在临时目录整理现有视觉证据，不写 Runtime/CAS。轮廓画布只是选择同一 candidate 的 silhouette AOV 与 overlay，reference-contour aid 只用于 Viewer 视觉提示，不创建第二套 Runtime mask；视觉解锁只信任 candidate-bound `QualityReport@2.visual_status + hard_gate_passed`，结构 candidate 的 `quality_hard_gate_passed` 不会清空视觉队列。fit-plan 已实行 `reference-canvas → silhouette-blockout → landmark-structure → semantic-part-fill → surface-detail → uv-pbr → final` 门控，轮廓未过时不输出 landmark/form/material 修改。Runtime 新增 `silhouette_part_error_get` 多 Part 误差表，供 Luna 按局部 boundary error 选出一个修正 Part；多 Part source regression 与真实 chest-shell transport 已通过。用户机器人 PNG 的 attempt35 记录 unrotated surface-linework + armor-shell-zones，26 Parts/4704 triangles，silhouette IoU 0.741047、boundary F1 0.328765、landmark coverage 0.733333、NME 0.134536，仍 `QUALITY_TARGET_NOT_MET`；它只是 benchmark eligibility `BLOCKED_INCOMPLETE_BINDING` 的 provisional retained observation。packaged Viewer 的 exact lineage receipt 仍单独保留；当前最新 r3 transport 已完成 Runtime-owned Primary Form compare，IoU 0.749122、Boundary F1 0.347623，仍未晋级质量门；image-block consumption、VoiceOver、PBR、人评、export/restart hash 和 HQ_360 仍 `NOT_RUN/BLOCKED`。证据位于 `docs/evidence/mcp010f/`。

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

2026-08-14 F Agentic observation binding：no-active-snapshot candidate selection now permits only zero/one-candidate states；multiple candidates fail closed until Codex supplies an explicit candidate ID or Runtime exposes an active snapshot. Focused Runtime regression 2/2 PASS；receipt `docs/evidence/mcp010f/agentic-observation-binding-20260814.json`。该修复只收口 canonical observation lineage，不等于 Agentic orchestrator、Repair execution 或视觉质量通过。
