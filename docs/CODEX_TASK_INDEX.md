# ForgeCAD 当前原子任务索引

版本：2026-08-14
状态：唯一任务状态表；MVP host golden path 与 FGC-MCP010A 已收口；FGC-MCP010B 结构实现已通过、Darwin OS 总内存硬门 deferred；FGC-MCP010C source-focused 已完成；FGC-MCP010D/E source-focused 已通过；FGC-MCP010F source-focused in_progress，packaged/人评/360 子门保留。ADR-0026 的 Agentic Design Runtime 已完成 observe/plan projection、嵌套只读 projection conformance 与 durable session/checkpoint/RepairIntent prepare/readback slice；durable/reference/DesignSpec 完整 producer、单动作 orchestrator 和 Repair 应用 backlog 尚未改变当前唯一任务状态。

Stage 0 机器真值入口为 `docs/evidence/mcp010f/current-benchmark-truth.json`。当前为 101 Schema、35 read + 21 opt-in write = 56 tools，唯一 `in_progress` 为 `FGC-MCP010F`。attempt35 仅是 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING` 的 provisional retained observation，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，camera 绑定 `MISMATCH`；补充 receipt `docs/evidence/mcp010f/primary-form-cas-rerun-20260814.json` 已完成 current-cohort fit/compare/AOV/review 与 packaged Viewer exact lineage binding，但不改变 Stage 0 latest pointer、attempt35 provisional 状态或 benchmark 资格。

2026-08-14 Primary Form objective alignment follow-up：Runtime Geometry trial 统一复用 camera 的 landmark/coverage-aware loss，避免几何搜索用 contour-only loss 绕过观测证据；focused 与完整 Runtime 回归通过。新 cohort 真实 Codex 复跑在 authoring sequence 阻断，未产出新的视觉指标，故不升级当前任务的视觉质量或 benchmark 状态。

2026-08-14 Primary Form proposal handoff follow-up：严格优于 authored baseline 的 Geometry Worker 试算现在随 `SilhouetteFitResult@1.selected_geometry_program` 返回；Runtime 校验其 `GeometryProgram@2` project/canonical hash，未改善时返回 `null`。它仍不自动写 candidate/version 或确认，后续必须在用户批准后调用 `geometry_prepare`；合同/Runtime focused/source Gate 已通过，未产生新的 likeness receipt。

<!-- forgecad-stage0: schemas=101 schema_set_sha256=2b22ee5bc28d3cfa332bd8f2677d07a4eb2405d8f545fcad9d137115101d763d read_tools=35 write_tools=21 total_tools=56 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260814-primary-form-framing-bound-viewer.json latest_completed=real-codex-cli-current-20260814-primary-form-coverage-bound-viewer.json -->

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
