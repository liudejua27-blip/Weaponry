# ForgeCAD 文档地图

> 2026-08-28 `FPS-FORM-04BE-F` 已完成只读失败诊断：新增 `ProductionWeaponFormArtFailureDiagnosticGetRequest/Result@1`、Runtime exact evidence revalidation 与默认只读 `production_weapon_form_art_failure_diagnostic_get`，当前公共面为 **579 schemas / 128 read + 94 opt-in write = 222 tools**。真实 D1 与隔离重启结果 canonical=`68e838ce…6fa3` 完全一致，SQLite/CAS 不变；它证明 04BE-E 的负 Y repair 在 left boundary 退化、right owner intrusion 增加、rear3q owner attribution 冲突、side trigger aperture sealed 与 line-flow 独立失败之间不能继续盲猜。下一原子 `FPS-FORM-04BE-G` 只做 hash-bound owner attribution 与 side-aperture visibility calibration，不注册新几何 profile。证据：`docs/evidence/mcp010f/production-weapon-form-art-failure-diagnostic-real-d1-04be-f-20260828.json`。

> 2026-08-28 `FPS-FORM-04BE-D` 已完成 evidence-bound typed repair plan：新增闭合 GET request/result、Runtime-only evidence derivation 与默认只读 MCP `production_weapon_form_art_repair_plan_get`，当前公共面为 **577 schemas / 127 read + 94 opt-in write = 221 tools**。真实 D1 在 mandatory `ponytail-preflight@0.1.0` 后，从 04BE-C exact sidecar/CrossView/FormArt/GeometryProgram 派生 `rear-stock-owner-void-half-y-flat-z@1`；重启前后 canonical=`d6f74060…85fd` 完全一致，116 个业务表/2520 rows 的逻辑摘要和 1639 个 CAS objects 树均未变化。该计划只把 current quarter-Y/flat-Z 描述为下一次 registered half-Y/flat-Z repair，`repair_execution_status=NOT_RUN / QUALITY_TARGET_NOT_MET`，不创建 FormQualityV2、secondary、confirm、version、export 或 High→Low→UV→Bake。证据：`docs/evidence/mcp010f/production-weapon-form-art-repair-plan-real-d1-04be-d-20260828.json`。

> 2026-08-28 `FPS-FORM-04BE-C` 已完成真实 D1 的同 cohort 六视图证据固化：final candidate 生成 **6×9=54 AOV**，`ProductionWeaponFormArtCompositeEvidenceRecord@1` 将 parent composite proposal、CrossView、proposal FormArt 与 receipt 精确绑定，隔离 Runtime 重启 GET hash equality PASS。当前公共面为 **575 schemas / 126 read + 94 opt-in write = 220 tools**。结果仍为 `QUALITY_TARGET_NOT_MET`：CrossView=`rejected-regression`，proposal FormArt=`BLOCKED_PROPOSAL_FORM_ART_EVIDENCE`，因此未创建 FormQualityV2，Stage=`camera-calibrated`、secondary=`NOT_CREATED`、confirm/version/export=false。下一原子 `FPS-FORM-04BE-D` 只从这些 durable hashes 派生 rear-stock owner-void/left-boundary typed repair plan。证据：`docs/evidence/mcp010f/production-weapon-form-art-composite-evidence-durable-runtime-gate-04be-c-20260828.json`。

> 2026-08-28 `FPS-FORM-04BE-B`：Runtime 已在真实 D1 中精确重验 original 04AZ baseline 与 current 04BB candidate/proposal evidence，按 registered trigger-guard aperture 生成并持久化 composite reviewable candidate=`candidate-f4a7d…06fa`；Store/CAS plan/link/receipt 与隔离 Runtime 重启 GET hash equality 均 PASS。当前公共面为 **572 schemas / 125 read + 93 opt-in write = 218 tools**。六视图 × 九 AOV、FormArt、FormQualityV2、human/engine 尚未运行，Stage=`camera-calibrated`、secondary=`NOT_CREATED`、confirm/version/export=false、`QUALITY_TARGET_NOT_MET`。下一原子为 `FPS-FORM-04BE-C` exact 54-AOV evidence。证据：`docs/evidence/mcp010f/production-weapon-form-art-composite-reviewable-candidate-durable-runtime-gate-04be-b-20260828.json`。

> 2026-08-27 `FPS-FORM-04AS`：`FormQualityV2` 新鲜基线适配器已在 Contracts、MCP、Runtime 与 Store 收口。它明确分离 source scope（当前 Stage head、CameraLock、same-cohort fresh baseline、registration lineage、RigV2）与 evaluation scope（distinct proposal candidate、proposal CrossView、proposal-side Part-ID/negative-space/line-flow）；所有调用字段均由 Runtime 从 durable evidence 重派生并由 Store 独立回读验证，legacy 模式不得夹带 proposal scope。538-schema checker、Contracts/Store/Runtime/MCP compile、四组件 same-cohort build identity 均 PASS，source cohort=`acf10c3b…173`。这只是 source/compile gate：当前真实 D1 `candidate-9127…fdc8b` 仍为 `REJECTED_REGRESSION`，未用新 adapter 重跑，Stage=`camera-calibrated`、secondary=`NOT_CREATED`、quality=`QUALITY_TARGET_NOT_MET`，无 confirm/version/export。下一原子是基于批准相机设计新的 bounded `rear-stock` art-shape，只有 proposal evidence=`READY` 且 fresh FormQualityV2 真实运行通过后才允许推进 Stage。证据：`docs/evidence/mcp010f/production-weapon-form-quality-v2-fresh-baseline-adapter-source-gate-04as-20260827.json`。

> 2026-08-27 `FPS-FORM-04AR` 最新真实 D1 入口：orientation lineage、fresh same-cohort baseline、`rear-stock 620×680` durable notch、六视图拒绝结果与最终零晋级状态统一见 `evidence/mcp010f/production-weapon-real-d1-orientation-baseline-notch-04ar-20260827.json`。该回执替代 04AP/04AQ 的“真实 D1 尚未运行”作为当前资产真值，但不改变商业路线或质量门。

> 2026-08-27 `FPS-FORM-04AL` 当前增量：Runtime-owned durable fresh six-view baseline producer 已接通合同、Store、Runtime 与 MCP `prepare/get`；每个视图绑定 approved registration lineage / RigV2、fresh same-cohort 512×512 九 AOV、camera/mask/compare/quality 与完整 CAS reachability，并以单事务持久化。精确状态为 `PASS_SOURCE_COMPILE_DURABLE_PRODUCER_NOT_RUN_REAL_D1`；真实 D1、orientation approval、fresh baseline、notch、secondary、Stage/confirm/version/export 均未执行。当前公共面 **538 schemas / 118 read + 88 opt-in write = 206 tools**，视觉仍 `QUALITY_TARGET_NOT_MET`。

> 2026-08-27 `FPS-FORM-04AK` 最新增量：lineage-bound fresh-baseline preflight 与 proposal 原子性边界见 `evidence/mcp010f/production-weapon-form-art-lineage-baseline-preflight-source-gate-04ak-20260827.json`。公共面为 **533 schemas / 117 read + 87 write = 204 tools**；该证据明确 materializer 尚不可用、真实 D1 零写、视觉质量未晋级。

> 2026-08-27 `FPS-FORM-04AJ` 最新增量：620/680 zero-write proposal 与 cohort fail-closed 边界见 `evidence/mcp010f/production-weapon-real-d1-open-frame-notch-620-680-readonly-proposal-04aj-20260827.json`。它证明 proposal identity 和零写，不证明 notch 已执行、fresh FormArt 或 secondary/visual PASS。

> 2026-08-26 `FPS-FORM-04AI` 最新增量：Runtime-derived semantic camera 的 successor contract/source/真实 D1 zero-write 结果记录在 `evidence/mcp010f/production-weapon-real-d1-semantic-camera-preflight-04ai-20260826.json`。该轮为 531/116+87；当前由 04AK 更新为 **533 schemas / 117 read + 87 write = 204 tools**。该 receipt 只证明 projection ready，用户批准和 durable lineage 尚未发生。

> 2026-08-26 `FPS-FORM-04AH` 最新阅读增量：rear3q 的 reference rotation、semantic screen order 与 Runtime camera orbit 已分离；source Gate 见 `docs/evidence/mcp010f/production-weapon-rear3q-semantic-camera-orientation-source-gate-04ah-20260826.json`，当前权威边界仍是缺 orientation-specific 用户回执、无真实 durable lineage、无 secondary/visual PASS。

> 2026-08-26 `FPS-FORM-04AC` 最新纵切：当前公共面为 **529 schemas / 115 read + 87 opt-in write = 202 tools**（新 Schema 是既有 proposal get/prepare 的 `MoveVertices | OpenFrameNotch` typed union，不新增工具）。真实 D1 `rear-stock` 已完成闭合 U-frame 拓扑、durable child、单 source-node lower、strict GLB readback 和 fresh 六视图 FormArt；`700/700` 候选因六视图回退被拒绝，baseline 保留。真实回执见 `evidence/mcp010f/production-weapon-real-d1-open-frame-notch-rejected-04ac-20260826.json`。

> 2026-08-26 最新同步：当前公共面为 **528 schemas / 115 read + 87 opt-in write = 202 tools**。真实 D1 AuthoringMesh `MoveVertices`/lower/readback/六视图证据见 `evidence/mcp010f/production-weapon-real-d1-authoring-mesh-v2-move-vertices-six-view-20260826.json`；proposal FormArt durable replay/restart 见 `evidence/mcp010f/production-weapon-formart-proposal-owner-evidence-durable-store-04ag-20260826.json`。后者证明 transport durability，但内容仍 `BLOCKED_PROPOSAL_FORM_ART_EVIDENCE`，不允许 secondary approval。商业纵向路线仍以 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`、`FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md` 与 ADR-0027 为权威。

> `FPS-FORM-04AG` 文档入口：`ProductionWeaponFormArtProposalEvidence@1` 已注册，并完成真实 D1 CAS/Store/replay/restart readback；hash-only receipt 见 `evidence/mcp010f/production-weapon-formart-proposal-owner-evidence-durable-store-04ag-20260826.json`。该 receipt 证明 durable transport，不代表六视图内容 Gate、质量、人评、引擎或 Stage/confirm/version/export PASS；实际内容状态为 `BLOCKED_PROPOSAL_FORM_ART_EVIDENCE`。

> 2026-08-26 `FPS-AUTHORING-MESH-V2-02` 同步：当前权威口径为 **527 schemas / 114 read + 86 write = 200 tools**。新增真实资产回执 `evidence/mcp010f/production-weapon-real-d1-authoring-mesh-v2-source-restart-20260826.json`，证明真实 D1 rear-stock 的 typed source binding、8V/6Q genesis 与 Runtime restart；不证明视觉质量。代码入口为 Runtime `production_weapon_authoring_mesh_v2_source.rs` / `authoring_mesh_v2_geometry.rs`、MCP `authoring_mesh_v2_durable_tools.rs` 与 High Worker `authoring_mesh_v2.rs`。

> 2026-08-26 `04AF` 同步：当前权威口径为 **527 schemas / 114 read + 85 write = 199 tools**。新增真实资产回执 `evidence/mcp010f/production-weapon-real-d1-rear-stock-source-repair-six-view-20260826.json`（六视图回退，拒绝）与 `evidence/mcp010f/authoring-mesh-v2-real-runtime-restart-20260826.json`（持久化局部拓扑，structural-only）。商业总路线仍以 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`、`FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md` 和 ADR-0027 为权威。

> 2026-08-26 04AE 文档口径：**525 schemas / 112 read + 84 write = 196 tools**；CameraLock、AuthoringMesh V2、Native High、strict bake/material plan 均仅 source compile，real D1/user orientation 仍 blocked。主蓝图见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

> 商业 FPS Hero Weapon 的最新跨来源研究、软件缺口、typed contracts、开源边界和纵向实施顺序统一见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`；它补充 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md` 与 ADR-0027，不替代当前资产/Stage 证据。

> 2026-08-26 `FPS-FORM-04AD` 权威增量：当前合同面为 **518 schemas / 111 read + 83 opt-in write = 194 tools**。新增 `ProductionWeaponSemanticLandmarkOrdering@1` 只表达 Runtime-derived 的 3D source/subject-axis 顺序，明确 `target_landmark_arrays_present=false / metrics=NOT_PRESENT`，不得冒充 2D landmark；`ProductionWeaponAuthoredViewOrientation@1` 将诊断变换与用户方向回执分开；`RegisteredCameraRigCalibration@2` 只有绑定 promotable authored rear3q receipt 才能物化。定向 Contracts/Runtime/MCP compile 与 518-schema checker PASS。真实 D1 尚无 orientation-specific user receipt，因此保持 `BLOCKED_AUTHORED_REAR_THREE_QUARTER_ORIENTATION`、Stage=`camera-calibrated`、secondary=`NOT_CREATED`、quality=`QUALITY_TARGET_NOT_MET`，不 confirm/version/export。旧 `@1` 保持历史真值；durable 落点采用 CameraLock 的 additive child lineage，不复制/自动升级整张旧记录。

> 2026-08-26 `FPS-FORM-04AC` source 入口：`agentic_action.rs` 已新增闭合 `rear-stock-profile-reconstruction-v1`，Runtime 只接受 5 个有界内侧型面语义量，产生三 depth stations 的 `profile-loft@2`，锁死外包络并强制唯一改变 source node=`rear-stock`。Caller GeometryProgram 与外部 RepairIntent 路径对该 strategy fail closed。Runtime/MCP compile PASS；尚无真实 proposal/RenderSet/FormArt receipt，rear-three-quarter authored semantic orientation 仍未 durable，故只记 `PASS_SOURCE_COMPILE / ACCEPTANCE_BLOCKED`，不推进 Stage/confirm/version/export。

> 2026-08-26 商业 Goal 导航：商业武器的北极星、三类交付对象、Authoring→High→Low→UV→Cage/Bake→Material→FPS→Engine→Human 路线以 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md` 为质量权威，以 ADR-0027 为架构权威；本地图及其他状态文档只引用，不再用工具/合同数量推导商业进度。

2026-08-26 最新真实 D1 semantic-camera 回执：`docs/evidence/mcp010f/production-weapon-real-d1-semantic-camera-relock-source-attribution-pass-20260826.json`，覆盖 fresh durable rebuild、registered camera exact replay、6 RenderSets/54 AOV、FormEvidence/FormArt/CrossView restart 与 live pixel→triangle→source attribution；结果为 `PASS_REAL_D1 / UNIQUE_REAR_STOCK_SOURCE`。前一 04AA mismatch 回执保留为根因证据。V2 durable semantic CameraLock/ordering/rig/Form evidence 是正式商业后续，当前 @1 PASS 不提升质量 Stage。

2026-08-26 全量文档同步：Formal High 的 4 个 closed contracts、Runtime adapter/IPC、MCP `get/prepare` 与 Store scoped idempotency 已接通；当前为 **518 schemas / 111 read + 83 opt-in write = 194 tools**。完整 positive materialize→replay→drop/reopen、失败清理和 raw current-cohort receipt 尚未取得，因此 public surface=`PASS_SOURCE`、capability positive=`NOT_PROVEN`。

2026-08-26 最新同步入口：Form Stage policy 与 Formal High internal materialization seam 的 source 真值记录在 `docs/evidence/mcp010f/commercial-weapon-form-stage-policy-formal-high-source-gate-20260826.json`。本条取代各页较早的“derived High materialization 尚未实现”表述，但不把完整 positive restart、public tool、Stage、visual 或 commercial Gate 写成 PASS。

2026-08-26 Cage/Bake 文档入口：公共 preflight/get/prepare、Store 七子表和固定 Cage/Bake Worker 已形成 source seam；High resolver 已按 `Stage source candidate + distinct derived High candidate` 的真实字段模型编译通过。AuthoringMesh source GLB、source-only Native High GLB/readback 与 Formal High internal materializer 各自存在；当前缺的是完整 source-lineage/CAS 正向 restart fixture 与独立 Formal High public prepare/get，不是“完全没有 GLB”，也不是需要再造一套重复 binding Schema。真实 D1 因 Form 前置未通过且没有 formal positive receipt，仍返回 `FORMAL_HIGH_STAGE_SOURCE_LINEAGE_UNAVAILABLE / PRODUCTION_WEAPON_HIGH_LOW_BAKE_PRODUCER_UNAVAILABLE / ZERO_WRITE`。权威状态读 `AUTHORITATIVE_STATE.md`，工程交接读 `CODEX_HANDOFF.md`，商业方法与软件缺口读 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`，合同/测试边界读 `MCP_RUNTIME_CONTRACT.md`、`SCHEMAS.md`、`TEST_STRATEGY.md`；不得提升任何质量或发布结论。


> 2026-08-26 最新权威 source 口径（取代下方 2026-08-25 的“最新/当前”计数）：**518 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 MCP tools**。Low quad draft 的 current provenance 为 candidate-bound，仍为 `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`；其 prepare→同键重放→Runtime drop/reopen→get 在隔离 current-cohort fixture **1/1 PASS**。Hero UV 已有 **7 个 registered contracts**，并已接入 Store/Runtime/MCP public `hero_uv_durable_get/prepare`；其 prepare→同键重放→Runtime drop/reopen→get **1/1 PASS**，四个 Hero CAS roots 已纳入 linked/GC 判定。该 slice 仅为 `structural_only`，不是 artist unwrap、visual、human、engine、commercial 或 packaged PASS；proposal=`registered=false`。当前不推进 Stage、confirm、version 或 export；Stage=`camera-calibrated`、`secondary-form-approved=NOT_CREATED`、`FPS-HIGH-05=NOT_PASSED`、visual=`QUALITY_TARGET_NOT_MET`、HQ360=`BLOCKED_REFERENCE_COVERAGE`。证据：`docs/evidence/mcp010f/commercial-weapon-hero-uv-durable-restart-source-gate-20260826.json`。

> 2026-08-25 历史商业武器研究与当时 source 证据入口：总路线见 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`，综合 receipt 见 `docs/evidence/mcp010f/commercial-weapon-authoring-slices-source-gate-20260825.json`，Native High durable细节见 `native-high-glb-durable-source-implementation-gate-20260825.json`。当前是 **499 schemas / 107 read + 79 opt-in write = 186 MCP tools**；High、Low draft、Hero UV与 Art Director matrix增量不改变视觉、人审、引擎、分发和 Stage 状态。

2026-08-25 历史生产作者链对账（当前口径见顶部）：公共合同集合为 **499 schemas**，公共工具面为 **107 read + 79 opt-in write = 186**；`native_high_durable_get/prepare` 当前 cohort restart **1/1 PASS**，Low quad draft与 Hero UV仅 Worker producer，Art Director matrix仅 Viewer read model。这仍是 source-only structural receipt，不是视觉或产品验收：`FPS-HIGH-05=NOT_PASSED`、visual=`QUALITY_TARGET_NOT_MET`、human/engine/distribution=`NOT_RUN`、Stage=`camera-calibrated`，不推进 stage/confirm/version/export，HQ360=`BLOCKED_REFERENCE_COVERAGE`。既有 490-schema Stage0 receipt 保持历史原样。

2026-08-25 `CQ-02-TYPED-TOPOLOGY-IDENTITY-LINEAGE`：真实 split/collapse/dissolve 已分别完成 `edit preview/prepare → reviewable candidate → durable AuthoringMesh → IdentityLineage → Runtime drop/reopen/get` 独立完整链路，合计 **3/3 PASS**。proof 仍保持 source-element-only；Runtime 只从 Store 的 exact candidate→idempotency response 恢复 proof，并派生 child IDs、单调 tombstone及 one-to-many/many-to-one relation，不接受 caller identity/proof arrays。Store `authoring_mesh_` **12/12**、MCP IdentityLineage **3/3**、490-schema checker与 Contracts/Store/Runtime/MCP 联合 compile PASS，工具数仍 **106 read + 78 write = 184**。general correspondence、evaluated retarget、完整 selection/undo history 与产品级 cross-version editor仍 `NOT_PROVEN`。Stage 保持 `camera-calibrated`，视觉=`QUALITY_TARGET_NOT_MET`，human/engine/distribution=`NOT_RUN`，HQ360=`BLOCKED_REFERENCE_COVERAGE`。

2026-08-25 AuthoringMesh 同步入口：当前公共合同集合为 **499 schemas**；该 AuthoringMesh切片当时工具面为 **106 read + 78 write = 184 MCP tools**，当前总工具面见顶部。既有 `authoring_mesh_get` 继续提供 candidate-bound、零写入 half-edge structural projection；7 个 `AuthoringMeshCanonical/Artifact/Link/Prepare/Get@1` durable 合同已接 Runtime 三对象 CAS producer/get、Store durable record与 MCP 默认读/显式写，公共三对象 Runtime drop/reopen restart fixture已通过 **1/1**。IdentityLineage 现从 Store-owned typed proof 派生 operation-derived child IDs、单调 tombstone和闭合 relation；split/collapse/dissolve 三条真实 staged candidate→durable mesh→identity→restart 独立链路 **3/3 PASS**。完整编辑历史、evaluated retarget和产品级 cross-version editor仍 `NOT_PROVEN`。商业 canonical 决策见 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md` §5.2，合同事实见 `SCHEMAS.md`；视觉质量与 Stage 晋级仍未完成。此前 AuthoringMesh/Stage0 receipt 中的 490-schema 数值继续按历史快照解释。

2026-08-25 最新 supplemental Form 诊断入口：`docs/evidence/mcp010f/production-weapon-real-d1-hash-only-registration-preflight-20260825.json` 与 `docs/evidence/mcp010f/production-weapon-real-d1-private-stock-upper-profile-station-isolation-20260825.json`。它们分别记录唯一 registration identity 与两个 station 方向停止；聚合 fixture 被中止，所以不构成完整 fixture、FormQuality、secondary Stage 或商业质量 PASS。

2026-08-25 商业游戏武器质量研究已收口到 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`。该文件现在是“如何设计商业级游戏武器、ForgeCAD 缺什么、为什么当前结果仍是高级灰模、外部算法如何进入原生 Worker、各阶段如何验收”的唯一跨文档质量计划。它不改变当前 `QUALITY_TARGET_NOT_MET`、`human/engine=NOT_RUN` 或 `HQ_360=BLOCKED_REFERENCE_COVERAGE`，也不把研究项目升级为产品采用。

2026-08-25 Native High 同步入口：早期 transport receipt 保留为历史；最新 source durable/MCP receipt 为 `docs/evidence/mcp010f/native-high-glb-durable-source-implementation-gate-20260825.json`。权威状态见 `AUTHORITATIVE_STATE.md`，测试边界见 `TEST_STRATEGY.md`，合同见 `SCHEMAS.md` / `MCP_RUNTIME_CONTRACT.md`，proposal 晋级边界见 `SKILL_PACKAGE_STANDARD.md`；`FPS-HIGH-05` 仍 `NOT_PASSED`。

2026-08-25 `FPS-FORM-04X` 最新证据：`docs/evidence/mcp010f/production-weapon-real-d1-private-stock-upper-profile-boundary-translation-screen-20260825.json`。真实 D1 screen execution **1/1 PASS（780.85s）**，但 coupled boundary-translation geometry direction 已停止：`+0.020/+0.040 m` 均保持三视图 identity，却分别保留 intrusion `302/472/587`、`296/416/587`，rear3q adjacency 均为 `248<250`，formal owner gate=false。quality/secondary 仍 blocked，不构成 FormQuality/public-sink/stage/confirm/version/export PASS；log sha256=`19f4935b4a1078cb435773740455ac997bc92679494986a2c219b529e21082fb`。

2026-08-25 `FPS-FORM-04W` 最新证据：`docs/evidence/mcp010f/production-weapon-real-d1-private-stock-upper-profile-lip-continuation-screen-20260825.json`。它记录 intrusion 改善与 adjacency 回退的冲突，并正式停止 lip_y 正向外推；不构成 FormQuality/secondary/public-sink PASS。

2026-08-25 `FPS-FORM-04V` 最新证据：`docs/evidence/mcp010f/production-weapon-real-d1-private-stock-upper-profile-lip-extrapolation-screen-20260825.json`。它记录三视图 strong signal 与 formal owner gate 阻断，不构成 FormQuality/secondary/public-sink PASS。

2026-08-25 `FPS-FORM-04U` 最新证据：`docs/evidence/mcp010f/production-weapon-real-d1-stock-upper-pixel-owner-audit-20260825.json`。它记录逐像素 owner-mask/expected-void/boundary-band/depth/silhouette 归属，只支持 04V 私有有界诊断，不构成 FormQuality/secondary/public-sink PASS。

2026-08-25 `FPS-FORM-04T` 最新证据：`docs/evidence/mcp010f/production-weapon-real-d1-private-stock-upper-profile-cap-lip-screen-20260825.json`。两个第二坐标候选均无像素级变化并 blocked，不构成 FormQuality/secondary/public-sink PASS。

2026-08-25 `FPS-FORM-04S` 最新证据入口：`docs/evidence/mcp010f/production-weapon-real-d1-private-stock-upper-profile-shoulder-screen-20260825.json`。它记录两档 outer-shoulder 真实三视图 screen；两候选均 blocked，不构成 FormQuality/secondary/public-sink PASS。

2026-08-25 `FPS-FORM-04R` 最新证据入口：`docs/evidence/mcp010f/production-weapon-real-d1-private-stock-upper-profile-lip-screen-20260825.json`。它记录固定 `0.85 m` profile 的两档 lip 真实三视图 screen；两候选均 blocked，不构成 FormQuality/secondary/public-sink PASS。

2026-08-25 `FPS-FORM-04Q` 最新证据入口：`docs/evidence/mcp010f/production-weapon-real-d1-private-stock-upper-profile-screen-20260825.json`。它记录 active `profile-loft@2` 两个完整 profile 的真实三视图 owner screen；两候选均 blocked，不构成 FormQuality/secondary/public-sink PASS。

2026-08-25 `FPS-FORM-04P` 最新证据入口：`docs/evidence/mcp010f/production-weapon-real-d1-private-stock-upper-inner-span-screen-20260825.json`。它记录 upper-inner-span `0.85/0.75 m` 的真实三视图 owner screen；两候选均因 adjacency/rear3q identity 阻断，不构成 visual/secondary/public-sink PASS。

2026-08-25 `FPS-FORM-04O` real-D1 stock source attribution：`docs/evidence/mcp010f/production-weapon-real-d1-stock-source-attribution-20260825.json`。三视图 exact split 证明全部 open-stock owner intrusion 来自 upper beam；仅支持下一轮 private upper-only bounded diagnostic，不构成 visual/secondary/public-sink PASS。

2026-08-25 `FPS-FORM-04N` 最新证据入口：`docs/evidence/mcp010f/production-weapon-real-d1-private-stock-plane-screen-20260825.json`。它记录 private-only `stock-plane-position ±0.10 m` 的真实三视图 owner screen 与 fail-closed invariants；两个候选均 blocked，未运行六视图，也未开放 public sink或推进任何阶段。

2026-08-25 `FPS-FORM-04M` 最新证据入口：`docs/evidence/mcp010f/production-weapon-real-d1-owner-ranked-directional-diagnostic-20260825.json`。它证明 strict binding uniqueness 已正确落到 unique ranked identity，并记录三视图有符号 owner-vs-expected-void 偏差；质量仍因非零 intrusion 与 rear3q adjacency 阻断。它不证明 world-X 方向、public sink、FormQuality、secondary、depth 或视觉通过。

2026-08-25 `FPS-FORM-04K` 最新证据入口：`docs/evidence/mcp010f/production-weapon-real-d1-registered-owner-void-diagnostic-20260825.json`。它证明 rear3q authored reference rotation 使 direct registered-camera Part-ID identity，同时证明三视图仍有非零 owner intrusion、严格质量阻断；不证明 landmark/region rotation、FormQuality、secondary、depth 或视觉通过。

2026-08-25 `FPS-FORM-04J` 最新证据入口：`docs/evidence/mcp010f/production-weapon-real-d1-registered-camera-lock-readonly-link-20260825.json`。它只证明真实 D1 durable CameraLock 到 transient registered rig 的同候选只读 lineage PASS，不证明 FormQuality、secondary stage、depth、rear3q reference registration 或视觉通过。

2026-08-25 `FPS-FORM-04I`：当前源码为 **474 schemas、28 operator catalog entries、103 read + 76 opt-in write = 179 tools**。新增公开、只读、非持久化的 subject-frame registration 与 registered-camera lineage contracts；不等于 CameraLock/FormQuality 重放或视觉通过。

2026-08-25 `FPS-FORM-04H-SUBJECT-FRAME-REGISTRATION-SOURCE` supplemental source evidence：`docs/evidence/mcp010f/production-weapon-subject-frame-registration-source-supplemental-20260825.json` 记录正确 D1 的 exact semantic axes 与闭集 `yaw-180-y` GeometryProgram→SubjectFrame 注册；synthetic 7/7、真实 D1 1/1（0.05s），7 sinks 不变，zero Runtime/Worker/CAS/SQLite write。该文件只登记 source truth，不新增 schema manifest/MCP tool/current durable truth，也不改变旧 04G receipt。real registered CameraLock/FormQuality、rear-three-quarter reference registration、owner-void acceptance、secondary/confirm/version/export 均未完成。

2026-08-25 `FPS-FORM-04G-REGION-PART-ID-BINDING` supplemental diagnostic：回执 `docs/evidence/mcp010f/production-weapon-form-region-part-binding-supplemental-20260824.json` 记录 `BLOCKED_AUTHORED_ORIENTATION_OR_REGISTRATION`。同一真实 D1 baseline/trial 的唯一 image-space 候选稳定一致：left/right=`horizontal-flip`、rear-three-quarter=`vertical-flip`；但当前没有 authored orientation/registration contract，且 discovery 门允许 owner/expected-void overlap，因此仅为 `EPHEMERAL_TRANSFORM_CANDIDATE`，不得晋级 binding 或 FormQuality。FormArt canonical、user-confirmed contours 和 depth=`UNKNOWN` 不变；无 stage/confirm/version/export，sidecar 不改 schema、MCP public contracts、manifest、current truth 或 Stage0 hashes。下一原子：`FPS-FORM-04H-AUTHORED-VIEW-ORIENTATION-REGISTRATION`。

2026-08-24 `FPS-FORM-PART-ID-AUDIT-04F` supplemental diagnostic：回执 `docs/evidence/mcp010f/production-weapon-form-part-id-audit-supplemental-20260824.json` 记录 `BLOCKED_OWNER_BINDING`；mutator 改变 `rear-stock` Part-ID mask，但 left/right/rear-three-quarter reviewed region↔Part binding 未就绪。FormArt canonical unchanged、depth=`UNKNOWN`、无 stage/confirm/version/export；该 sidecar 不改 schema、MCP public contracts、manifest、current truth 或 Stage0 hashes。其后续 04G 已由上段记录，仍未形成 authored binding。

版本：2026-08-26 · 状态：MCP005–MCP009 MVP functional core 已收口；FGC-MCP010F 仍 `in_progress`。当前公共合同集合为 **518 schemas**，28 个 operator catalog entries、111 read + 83 opt-in write = 194 tools；Stage0 490-schema receipt 保留为历史快照。Hero UV durable public `get/prepare` 与正向 restart **1/1 PASS** 只增加 structural/source truth，不改变下述 Form 与视觉阻断。真实用户概念板已通过六身份视图/七相机 CameraLock、`camera-calibrated` Stage@3、FormEvidence、reviewed-structure FormArt、同 candidate CrossView 与 structural-only legacy FormQuality 的 durable prepare/replay/get/restart；六个视觉结构为 user-confirmed、depth=unknown，25 条 line-flow 类型/连续组和左/右/后三分之四 exact subtract contours 已绑定。frozen-camera open-stock evaluator 已拒绝 `clearance=0.22 m` 和 `angle=0.12 rad`：两者都未改善三视图内孔指标且回退外轮廓，baseline retained；下一原子是 reviewed region/Part-ID 绑定审计。FormArt/整体仍 `NOT_PROVEN` / `QUALITY_TARGET_NOT_MET`，FormQuality@2 仍被 CrossView hard gate 与 FormArt target observation 阻断；Stage head 仍 `camera-calibrated`，未创建 `secondary-form-approved`。`FPS-HIGH-LOW-CAGE-05` 仍缺 Stage head binding、artist quad topology、edge-flow、formal High/Low/Cage 和 Bake 证据。Blender 官方 source 只作 clean-room/reference 研究，不进入产品 Runtime。

## 商业级职责索引

ForgeCAD 当前仍是可验证高级灰模/技术管线，不是商业级资产生产软件。上游资产真值与艺术生产能力尚未闭合，具体缺口和落地顺序由 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`、ADR-0027 与模块边界共同约束；不能用研究 receipt、工具数量或可打开 GLB 覆盖缺口。

| 能力 | ForgeCAD-only 责任 | 解锁门/当前口径 |
| --- | --- | --- |
| Art Direction / ReferenceViewSet | Brief、五核心视图、CameraLock、silhouette/negative-space/landmark | `camera-calibrated`；CrossView=`QUALITY_TARGET_NOT_MET`；HQ360=`BLOCKED_REFERENCE_COVERAGE` |
| AuthoringMesh | original/evaluated topology、Part/source map、stable identity lineage | split/collapse/dissolve durable/restart **3/3 PASS**；correspondence、edge-flow、evaluated retarget/editor `NOT_PROVEN` |
| Native High | 非破坏 High detail graph、derived High candidate、strict GLB/readback | source/durable structural PASS；two-candidate resolver compile PASS；formal High positive=`NOT_RUN`，`FPS-HIGH-05=NOT_PASSED` |
| Low / Retopo | artist-editable quad flow、Part/seam/hard-edge locks、High↔Low correspondence | `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`；artist review/package `NOT_RUN` |
| Hero UV | density/seam/stretch/padding/UV0/UV1/tangent | public durable restart **1/1 PASS** 仅 structural；artist unwrap/package/engine `NOT_RUN/NOT_PROVEN` |
| Cage / Bake | topology-correspondent cage、逐 Part ray/miss/skew/cross-hit/padding、8 maps | fixed Worker/Store/MCP seam=`PASS_SOURCE`；真实 D1 zero-write，formal positive=`NOT_RUN` |
| Material / Surface | layer/mask/wear/microdetail/PBR provenance | 当前仅 4 zones/6 fixed-formula textures；commercial PBR=`NOT_PROVEN` |
| LOD / Collision / Socket | authored LOD0/1/2、属性/轮廓误差和平台预算 | 分散 structural slices；商业 gate=`NOT_RUN` |
| Art Director Viewer / FPS | 只读 AOV/compare/Part/MaterialZone、animation/VFX/audio/accessibility | source/read-model smoke；正式 FPS/VoiceOver/human viewing=`NOT_RUN` |
| EngineValidation | 真实 Unreal/Unity importer round-trip 与性能预算 | `engine=NOT_RUN`，Three.js/Godot 不能替代 |
| HeroArtReview | 独立艺术家/IP/同 export hash 审核 | `human=NOT_RUN`，Codex typed review 不能替代 |

所有永久修改仍由 Runtime 唯一写者按 `prepare → compile/readback → evaluate → approval → confirm` 管理；MCP/Viewer/Worker 无任意脚本、DCC 或第二真值依赖。

## 阅读顺序

1. `DOCUMENTATION_STATUS.md`：当前事实和能力标签
2. `CODEX_HANDOFF.md`：本分支证据、命令和剩余阻断
3. `ADR/0025-codex-only-mcp-3d-runtime.md`：产品断代决策
4. `ADR/0026-agentic-design-runtime.md`：Agentic Design Runtime 目标架构；定义 DesignSession、SemanticSceneGraph、ReferenceCanvas、阶段门、Visual Evidence 和 Critic/Repair loop
5. `ADR/0027-native-fps-weapon-production-executor.md`：无 Blender 运行时的 ForgeCAD 原生 FPS 武器生产架构；定义 High/Low/Cage Bake、Hero UV、精细 Production Stage 与 Artistic/Engine Gate；`ADR/0028-blender-headless-worker-evaluation.md` 仅是 additive evaluation addendum
6. `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`：商业级武器的 Brief→Form→High/Low→UV/Cage/Bake→Material→FPS→Engine→Human 完整能力合同、当前证据缺口和原生 Worker 采用路线
7. `FORGECAD_AGENTIC_DESIGN_RUNTIME_PLAN.md`：本次大调整的执行规划；包含已实现的 projection 与 durable prepare/readback slice 和未完成的 orchestrator/Repair backlog
8. `ARCHITECTURE_MODULE_BOUNDARY.md`：当前模块权责、目标模块和 active/archive 目录边界
9. `DEPRECATED_ISOLATION_PLAN.md`：废弃文档、代码、模块的隔离位置、状态定义和验证流程
10. `RESET_MIGRATION_PLAN.md`：删除/迁移/升级清单
11. `CODEX_EXECUTION_PLAN.md`：阶段与退出门
12. `CODEX_TASK_INDEX.md`：Luna 唯一任务队列
13. `MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`：MCP010A–F 的唯一质量升级执行合同
14. `CODEX_GEOMETRY_V2_WORKFLOW.md`：MCP010B 期间 Codex/Luna 发现 live OperatorCatalog、构造 GeometryProgram@2、判读 ArtifactReadback@2、固定同级 Worker 隔离证据和 V1 过渡的受限工作流
15. `CODEX_SINGLE_REFERENCE_OPERATING_GUIDE.md`：Codex 使用单张授权参考图的可执行调用顺序、停止条件、失败映射和 C–F 交接边界
16. `CODEX_REFERENCE_DETAIL_INVENTORY.md`：Codex 单图参考的授权 intake、可见/推断/未知细节清单、分阶段修正队列、质量合同和停止规则；它是编排模板，不是 Runtime 合同
17. `CODEX_SILHOUETTE_FIT_WORKFLOW.md`：轮廓目标、相机搜索、边界误差和 Codex/Luna 单 Part 修正纪律；当前 source slice 已接入 Runtime/MCP，但真实用户 likeness 仍需独立证据
18. `MCP010C_READINESS_AUDIT.md`：固定渲染/参考比较的当前实现审计、C1–C4 证据、未运行视觉门和 Luna 检查清单；当前 C source Gate 已通过但不代表真实用户 likeness 或 packaged/live 完成
19. `AUTHORITATIVE_STATE.md`：Runtime 数据真值
20. `MVP_DELIVERY_PLAN.md`：MVP 范围、MCP005–009 退出门、工具采用决策和当前证据边界
21. `LUNA_GOAL_EXECUTION_GUIDE.md`：Goal 执行协议、当前可调用工具和真实 host 验收动作
22. `LUNA_GITHUB_REPLICATION_PLAYBOOK.md`：Luna 研究 build123d、BlenderMCP、CadQuery、Manifold、MaterialX 的冻结 revision、选择性源文件复刻、quarantine、审查和接受流程
23. `BLENDER_CAPABILITY_ADAPTATION_PLAN.md`：Blender 官方 frozen revision/许可证研究、ForgeCAD clean-room Mesh/Modifier/Subdivision/Render/rigid animation 能力映射与分期路线；官方 reference-only receipt 位于 `evidence/adoption/blender/`
24. `EXTERNAL_PROJECT_ADOPTION.md`：第三方采用状态、research receipt 和 accepted 入口
25. `CODEX_PONYTAIL_PREFLIGHT_WORKFLOW.md`：Codex 经 MCP 进入 3D 设计前必须读取的 first-party preflight Skill、会话顺序、边界和维护规则
26. 任务相关合同：`MCP_RUNTIME_CONTRACT.md`、`CODEX_INTEGRATION.md`、`COMPILER_PIPELINE.md`、`WORKBENCH_VIEWER.md`、`SKILL_PACKAGE_STANDARD.md`、`SCHEMAS.md`、`DATABASE.md`
27. `MVP_ARCHITECTURE.md`：单用户启动、文件锁和最小运行边界
28. `MVP_TOOL_CATALOG.md`：当前源码的 live 工具数量以 `evidence/mcp010f/current-benchmark-truth.json` 为准；C/D/E/F 的结构 Gate、真实 likeness、人评/PBR/纹理和 360 仍必须另标 planned/unavailable
## 生命周期

- `已实现`：当前代码和对应 Gate 通过；
- `部分实现`：已实现与缺口必须分开；
- `目标设计`：没有当前代码证据；
- `迁移中`：旧代码已删除，新能力尚未完成；
- `blocked`：退出条件因环境、授权或外部事实失败；
- `superseded`：不再属于当前产品。

目标设计不能覆盖事实，历史 Git 内容不能证明当前能力。`scene_observe_get` 等 projection 已有 source/runtime/MCP/Viewer 证据，`session_create_or_resume`、`session_get`、`checkpoint_prepare`、`checkpoint_get`、`checkpoint_restore_prepare` 已有 durable prepare/readback 与重启 receipt；后者仍不能等同于单动作 orchestrator、Critic/Repair 执行或完整 schema conformance。每个任务结束必须同步状态账本、任务索引、能力矩阵、handoff 和受影响合同；用户指南只能写已实现或当前 Viewer 能力。`functional-core PASS` 不能升级成 `high-quality/reference PASS`。

## 当前权威文件

产品/决策：`PRODUCT_DEFINITION.md`、`ADR/0025-codex-only-mcp-3d-runtime.md`、`ADR/0026-agentic-design-runtime.md`、`ADR/0027-native-fps-weapon-production-executor.md`、`ADR/0028-blender-headless-worker-evaluation.md`（evaluation-only）。

架构/合同：`DESIGN.md`、`MVP_ARCHITECTURE.md`、`ARCHITECTURE_MODULE_BOUNDARY.md`、`AUTHORITATIVE_STATE.md`、`MCP_RUNTIME_CONTRACT.md`、`CODEX_INTEGRATION.md`、`COMPILER_PIPELINE.md`、`WORKBENCH_VIEWER.md`、`SKILL_PACKAGE_STANDARD.md`、`SCHEMAS.md`、`DATABASE.md`；MCP003 快照和宿主矩阵位于 `evidence/mcp003/`。

执行/质量：`RESET_MIGRATION_PLAN.md`、`MVP_DELIVERY_PLAN.md`、`MVP_TOOL_CATALOG.md`、`CODEX_PONYTAIL_PREFLIGHT_WORKFLOW.md`、`FORGECAD_AGENTIC_DESIGN_RUNTIME_PLAN.md`、`COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`、`MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`、`BLENDER_CAPABILITY_ADAPTATION_PLAN.md`、`CODEX_GEOMETRY_V2_WORKFLOW.md`、`CODEX_REFERENCE_DETAIL_INVENTORY.md`、`CODEX_SINGLE_REFERENCE_OPERATING_GUIDE.md`、`CODEX_EXECUTION_PLAN.md`、`CODEX_TASK_INDEX.md`、`LUNA_GOAL_EXECUTION_GUIDE.md`、`LUNA_GITHUB_REPLICATION_PLAYBOOK.md`、`CODEX_DEFINITION_OF_DONE.md`、`TEST_STRATEGY.md`、`evidence/CAPABILITY_GATE_MATRIX.md`。

运维/供应链：`DEVELOPMENT.md`、`OPERATIONS.md`、`PACKAGING.md`、`PRODUCTION_RELEASE_CHECKLIST.md`、`RELEASE_MAINTENANCE.md`、`DISASTER_RECOVERY.md`、`THIRD_PARTY_LICENSES.md`、`EXTERNAL_PROJECT_ADOPTION.md`、`LUNA_GITHUB_REPLICATION_PLAYBOOK.md`、`DEPRECATED_ISOLATION_PLAN.md`。

当前树已删除旧 Provider、App Server、Python Agent、standalone Host、旧工作台、旧 Concept/Weapon/Module 产品入口、合同和评估。恢复材料只存在于受控 reset/cleanup 归档和 Git 历史；少量解释架构迁移所需的历史 receipt 只能以 `SUPERSEDED` 状态放在 `evidence/archive/`，不能由当前 manifest 引用为 PASS，也不得重新链接或恢复为产品入口。
2026-08-25 `FPS-FORM-04L` 证据入口：`docs/evidence/mcp010f/production-weapon-real-d1-stock-clearance-trials-20260825.json`。它记录三个真实 D1 clearance 试验、完整程序不变量与同 cohort lineage，并证明三组均因非零 owner intrusion 而 blocked；不是视觉、FormQuality 或 secondary PASS。
