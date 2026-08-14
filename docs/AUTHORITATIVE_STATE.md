# ForgeCAD 权威状态与版本真值

2026-08-15 Primary Form same-camera retention 状态：`primary_form_repair_prepare` 现在在创建 staged candidate 前，使用最终 Runtime-owned camera、同一 reference target 和同一 512px Render Worker 对 source GLB 与 proposed GeometryProgram 做 non-persisted full-resolution compare；只有 `proposal_loss < source_loss` 才进入正常 candidate/evidence transaction。`PrimaryFormAcceptance@1` 固化 source/proposal program hash、camera hash、两侧 loss 和 `accepted`/`retained_source`；未通过时保持 authored source，不覆盖 source VisualEvidence。该 Gate 只修复相机补偿/局部回归导致的错误晋级，未产生新的授权机器人视觉 receipt，当前仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 Primary Form 收敛证据状态：Runtime 现对每次 bounded fit 固化 authored baseline 与 selected winner 的 camera/metrics/loss、strict-improvement 和 camera/geometry evaluation 计数；action-run evaluate stage 通过 `result_sha256`、`output_sha256`、`summary_sha256` 形成 Primary Form→QualityReport→comparison 的单一证据链。`DesignActionRun@1` Schema 已对齐 Store 的实际 stage object，并通过 `skipped`/旧 `hash/reason` 负向回归。该修复不改变 candidate/version 写入边界，不把 Runtime source/focused PASS 写成视觉质量 PASS；Stage 0 仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 Render Worker Runtime adapter 状态：Runtime 侧新增 `render_worker.rs` typed adapter，Geometry Worker 只保留编译/worker transport，Primary Form 的初始 512px framing、full-resolution camera ranking、Geometry winner refit 和九 AOV render 均通过该 adapter 调用同一个 isolated sibling Render Worker。新增 source ownership checker 与 Runtime focused regressions 通过；该模块化修复不改写现有真实 reference，视觉仍为 `QUALITY_TARGET_NOT_MET`，`camera MISMATCH`、benchmark incomplete binding、human/PBR/export-restart/360 状态保持原账本。

2026-08-15 Agentic bounded action-run 状态：`design_action_run_prepare`/`design_action_run_get` 已进入当前 Runtime/MCP source slice。该切片只允许 approved、session/project/candidate/reference-bound、单 Part `primary-form` action；Runtime 在一次 action run 内复用既有 Primary Form bounded repair pipeline，写入 SQLite/CAS 的 `AgenticActionRunRecord` 并提供 immutable readback。结果锁定 `confirm`/`export`，`runtime_write=false`、`persistent_user_data_touched=false`，不会修改 candidate/version。focused Runtime idempotency/readback、MCP boundary/manifest 和 Stage 0 已通过；它不等于通用 orchestrator、Repair 应用、durable/reference/DesignSpec 完整 producer 或视觉质量通过。

2026-08-15 Primary Form 自动目标单 Part 状态：Runtime 已把没有显式 `SilhouetteTarget.parts` 的 automatic silhouette target 交给同一 Render Worker Part-ID boundary projection，仅对请求的 semantic Part 生成局部 envelope/error 和 bounded proposal；没有 hidden-side inference 或额外 Codex 参数搜索。current Dev.app cohort `77ccce85…b9d4` 的 `shin-pair` 隔离 receipt 已通过 preflight、MCP/Runtime cohort binding、Geometry/Render Worker transport 和五候选 compare，但严格单 Part retention 门保留 authored baseline（IoU `0.741047`、Boundary F1 `0.328765`），没有应用 candidate，仍为 `QUALITY_TARGET_NOT_MET`。该 supplemental receipt 不改写 Stage 0 provisional observation、camera `MISMATCH` 或 benchmark eligibility。

2026-08-15 Primary Form action budget truth：修复前真实 receipt `docs/evidence/mcp010f/primary-form-budget-pre-fix-real-codex-20260815.json` 暴露了 `max_evaluations=64 → fit_evaluations=24` 的 Runtime 外层截断；当前源码 cap 已恢复为 64，端到端 focused fixture 证明 `primary_form_repair_prepare` 在 64 请求下完成 63–64 bounded evaluations，`max_iterations` 仍为 1。Dev.app cohort `c521bf28…c4a5` 已安装；修复后 real-Codex receipt 在 authoring/hash/prepare 阶段阻断，没有新的视觉比较或质量结果。该源码修复不改写 retained observation，仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 未运行或阻断。

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

<!-- forgecad-stage0: schemas=102 schema_set_sha256=8d3644fb8169157584cf844a21588d4b6a49c7852600de4698205a7da6050cdb read_tools=36 write_tools=23 total_tools=59 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json latest_completed=real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json -->

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
