# ForgeCAD Runtime Schema 规范

> 2026-08-30 Brief addendum：manifest=`662 schemas`；新增
> `WeaponryKnifeProductionBrief@1`、prepare/get/result 四个 package-owned closed Schema。Brief 明确
> 区分无父的 `initial-intake-no-parent@1` 与绑定 exact parent ID/hash 的
> `immutable-successor-preserve-source-claims@1`；successor 不得增删改 source claims 或 conflict
> identity；resolved successor 的 `resolved_width/height` 与 `shipping_width/height` 分别冻结 authoring master
> 和 shipping 尺寸，并要求匹配 retained `hero`/`production` claims。默认 Knife profile active request closure 为 127/127。MCP validator 本轮补了 `not`
> 关键字，但仍不能宣称实现完整 JSON Schema 标准；复杂 regex/组合、循环 `$ref` 和预算门继续单列。

> **Weaponry P0 override (2026-08-29):** 本文只有在与 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md` 和 ADR-0029 一致时才具有当前执行权。ForgeCAD 在本文中解释为 Weaponry 的 Rust Runtime lineage；当前唯一产品主线是由 Codex 生成、修改、验证并交付高质量穿越火线非功能性游戏武器。通用 3D、机器人和原创科幻示例仅作 fixture/历史能力，不得抢占本月主线。 本文中所有 2026-08-28 及更早的“当前”“下一原子”“唯一 `in_progress`”和工具/Schema 数量语句均按历史 cohort 解释，不得覆盖 `WPN-*` successor queue。

> 2026-08-30 current source：manifest=`658 schemas`；默认 Knife profile 的 125 个 active operation 已全部绑定 package-owned closed request Schema，blocked=0，Runtime fallback=0。MCP validator 已执行 `minProperties/maxProperties`；复杂组合关键字、循环 `$ref` 与预算耗尽仍需继续负向覆盖。以下旧数量均为历史 cohort。

## Successor Schema families

优先新增或收敛为：`WeaponAssetAuthorization@1`、`AuthoringTransaction@1`、
`SelectionQuery@1`、`ModifierGraph@1`、`EvaluationReceipt@1`、`HighLowCorrespondence@1`、
`HeroUvCageBake@1`、`CrossFireFpsPresentation@1`、`CrossFireEngineDelivery@1` 和
`WeaponArtAcceptance@1`。Schema 只在有真实 producer/consumer、负向 fixture 和 replay 路径时
注册；禁止为每个武器部件或试验阶段继续创建平行版本。

> 2026-08-29 移除 Blender task/capability 六个占位 Schema 后，当前 manifest 为 **583 schemas / 131 read + 95 opt-in write = 226 tools**。`AuthoringMeshTransaction@1`、`AuthoringMeshTransactionPrepareRequest@1`、`AuthoringMeshTransactionGetRequest@1` 和 `AuthoringMeshTransactionResult@1` 继续保留。journal 限制 1–32 个连续 command_index，只含 SplitEdge/MoveVertices/FaceExtrude，generated ref 只可指向更早 command 且类型匹配。Result 将 first commit、exact replay 和 read-only found 分成三个互斥分支，禁止把 replay/get 伪报成持久写入。

> 2026-08-29 当前 contract manifest 为 **585 schemas**。新增 `ProductionWeaponFormArtTargetOcclusionAttributionGetRequest/Result@1`，并扩展 composite proposal 的 closed registered profile enum 以承载 camera-target receiver U topology；调用方仍不能提交任意点、mesh、脚本、相机或 mask。

> 2026-08-28 manifest 数量仍为 **583 schemas / 130 read + 94 opt-in write = 224 tools**，content-set=`514e8500d130…dda76e`。本阶段没有新增 Schema；`ProductionWeaponFormArtCompositeProposalPlan@1.operation` 的 closed oneOf 增加 `receiver-upper` 分支及 4 个 20/40mm min/max-X registered profile。调用者不能提交任意尺寸、mesh、script、path 或多 Part payload。

> 2026-08-28 manifest 数量仍为 **583 schemas / 130 read + 94 opt-in write = 224 tools**，content-set=`ec0c0a695826…99bb27`。本阶段没有新增 Schema；仅扩展 `ProductionWeaponFormArtCompositeProposalPlan@1.registered_profile_id` 的 closed oneOf，加入 4 个 product-owned true-aperture `@1` 与 4 个 camera-mapped-aperture `@2`。`@2` 用于区分已产生不可变候选的早期试验语义，不能静默复用旧 ID；调用者仍不能提交任意 profile、mesh、script、camera 或 path。

> 2026-08-28 manifest 仍为 **583 schemas / 130 read + 94 opt-in write = 224 tools**，content-set=`5521e44711ba…e36157`。本阶段没有新增 Schema；仅扩展现有 `ProductionWeaponFormArtCompositeProposalPlan@1` 的 closed oneOf，添加 `side-panel-a-retract-min-x-20mm@1`、`max-x-20mm@1`、`min-x-40mm@1`、`max-x-40mm@1`。变体仍必须绑定唯一 `side-panel-a` Part/node，不允许任意 profile/script/mesh 输入。

> 2026-08-28 manifest 为 **583 schemas / 130 read + 94 opt-in write = 224 tools**，content-set=`262a56faa10f…e52013`。新增 `ProductionWeaponFormArtApertureRepairPlanGetRequest@1` 与 `ProductionWeaponFormArtApertureRepairPlanGetResult@1`；结果闭合 calibrated source bindings、当前节点 canonical、两个顺序 step、8 个 bounded 变体、依赖门和六视图重验条件。Schema 强制 trial=`NOT_RUN`、`repair_execution_allowed_by_this_tool=false`、`form_quality_v2_status=NOT_CREATED`。

> 2026-08-28 manifest 为 **581 schemas / 129 read + 94 opt-in write = 223 tools**，content-set=`e0116e2f7ee0…8516a`。新增 `ProductionWeaponFormArtVisibilityCalibrationGetRequest@1` 与 `ProductionWeaponFormArtVisibilityCalibrationGetResult@1`；结果精确绑定 04BE-F diagnostic、before/after FormArt/GLB、CameraRig 与六个 reviewed structures，输出 reference/ranked masks、source counts、winner/depth/Part-ID/silhouette delta 及左右 calibrated source。Schema 强制 `geometry_repair_authorized=false`、`runtime_write_performed=false`、`form_quality_v2_status=NOT_CREATED`。

> 2026-08-28 manifest 为 **577 schemas / 127 read + 94 opt-in write = 221 tools**，content-set=`22332f4de4c3…b81639`。新增 `ProductionWeaponFormArtRepairPlanGetRequest@1` 与 `ProductionWeaponFormArtRepairPlanGetResult@1`；输出绑定 exact composite evidence、CrossView/FormArt canonical、composed GeometryProgram、current/target 五站 profile、四个 failure issues、7 个 mandatory revalidation gates 与 8 个 preserved invariants。Schema 明确 `repair_execution_status=NOT_RUN`、`runtime_write_performed=false`、`candidate_confirm_allowed=false`，不能把 plan 当成 mesh mutation、FormQualityV2、secondary 或商业质量通过。

> 2026-08-28 manifest 为 **575 schemas / 126 read + 94 opt-in write = 220 tools**，content-set=`cc60273466b0…f89e2`。新增 composite evidence prepare/get/Store record 三个闭合合同，只接受 durable identity/hash，并由 Runtime 派生、Store 重验 parent proposal、CrossView、FormArt、receipt 与 CAS roots；不接受 AOV bytes、相机矩阵、raw mesh、脚本、路径或 URL。真实 54-AOV/restart transport PASS，但 CrossView 与 owner-void 质量门失败，未创建 FormQualityV2，也不构成 secondary、视觉或商业质量 PASS。

> 2026-08-28 manifest 为 **572 schemas / 125 read + 93 opt-in write = 218 tools**，content-set=`1467f283b013…e62a0`。Composite proposal 现由 plan、prepare request、get request、Store record 四个闭合合同覆盖 original/current/final lineage、注册 replacement、Runtime-only writer、non-promoting 状态与 restart readback。Schema/持久化 PASS 不代表 54 AOV、FormArt、FormQualityV2 或商业美术质量通过。

> 2026-08-27 manifest 为 **568 schemas / 124 read + 92 opt-in write = 216 tools**，content-set=`ccb206f67756…fdc0`。新增 `ProductionWeaponOwnerReviewedVoidCalibrationProjection@1`、GET request 与 result 三个闭合合同；请求只携带 durable identity/hash，输出将三视图 `rear-stock` 身份/相机/深度 calibration 与 strict owner-void 结果分开。Schema 和 source compile PASS 不代表真实 D1 已校准、新形体已创作或商业美术质量通过。

> 2026-08-27 `FPS-FORM-04AU`：manifest 仍为 **538 schemas / 118 read + 88 opt-in write = 206 tools**。`ProductionWeaponFormArtMeshProposalEdit@1` 的闭合 union 已加入 `AuthoringMeshRearStockVoidRailBow@1`，`RenderSet@2` 明确要求 `view_id`；调用方仍不能提交 vertex IDs、raw mesh、camera matrix、任意脚本或路径。current content-set=`24be71a5bdf1…3c6c7`。真实 D1 RailBow 已完成 durable child/strict readback/restart，但六视图回归与 owner binding 门失败，故 schema 同步不改变 `QUALITY_TARGET_NOT_MET`。

> 2026-08-27 `FPS-FORM-04AS`：`FormQualityV2` 新鲜基线适配器已在 Contracts、MCP、Runtime 与 Store 收口。它明确分离 source scope（当前 Stage head、CameraLock、same-cohort fresh baseline、registration lineage、RigV2）与 evaluation scope（distinct proposal candidate、proposal CrossView、proposal-side Part-ID/negative-space/line-flow）；所有调用字段均由 Runtime 从 durable evidence 重派生并由 Store 独立回读验证，legacy 模式不得夹带 proposal scope。538-schema checker、Contracts/Store/Runtime/MCP compile、四组件 same-cohort build identity 均 PASS，source cohort=`acf10c3b…173`。这只是 source/compile gate：当前真实 D1 `candidate-9127…fdc8b` 仍为 `REJECTED_REGRESSION`，未用新 adapter 重跑，Stage=`camera-calibrated`、secondary=`NOT_CREATED`、quality=`QUALITY_TARGET_NOT_MET`，无 confirm/version/export。下一原子是基于批准相机设计新的 bounded `rear-stock` art-shape，只有 proposal evidence=`READY` 且 fresh FormQualityV2 真实运行通过后才允许推进 Stage。证据：`docs/evidence/mcp010f/production-weapon-form-quality-v2-fresh-baseline-adapter-source-gate-04as-20260827.json`。

> 2026-08-27 `FPS-FORM-04AL` 当前增量：Runtime-owned durable fresh six-view baseline producer 已接通合同、Store、Runtime 与 MCP `prepare/get`；每个视图绑定 approved registration lineage / RigV2、fresh same-cohort 512×512 九 AOV、camera/mask/compare/quality 与完整 CAS reachability，并以单事务持久化。精确状态为 `PASS_SOURCE_COMPILE_DURABLE_PRODUCER_NOT_RUN_REAL_D1`；真实 D1、orientation approval、fresh baseline、notch、secondary、Stage/confirm/version/export 均未执行。当前公共面 **538 schemas / 118 read + 88 opt-in write = 206 tools**，视觉仍 `QUALITY_TARGET_NOT_MET`。

> 2026-08-27 `04AK`：manifest 现为 **533 schemas**，新增 `ProductionWeaponFormArtBaselinePreflightRequest@1/Result@1`。Request 绑定 lineage/session/project/candidate/artifact 和零写策略，不接受 caller-owned camera/render/path；Result 投影 RigV2、固定六视图与 blockers。content-set=`e81ccfc851f6…4713ddaa`，materializer 仍 `UNAVAILABLE`。

> 2026-08-26 `04AI`：该轮 manifest 为 531 schemas，新增 `ProductionCameraLockRegistrationLineagePreflightProjectionGetRequest@1/Result@1`。Request 明确不含 orbit/matrix/mesh；Result 区分 Runtime-derived camera、闭合 upright proof、projection readiness、existing-lineage authority 与 blockers。当前已由 04AK 更新为 533 schemas、content-set=`e81ccfc851f6…4713ddaa`。

> 2026-08-26 `04AH`：`ProductionCameraLockRegistrationLineagePrepareRequest@1`、`ProductionWeaponAuthoredViewOrientation@1` 与 `RegisteredCameraRigCalibration@2` 已补齐 rear3q `subject_screen_order`、canonical closed `registered_camera_orbit`（仅 `0/180`）及 `runtime-projected-stock-muzzle-screen-order-and-world-y-upright@1` proof；promotable lineage 还强制绑定 CAS-backed `ApprovalReceipt@1`。schema 数仍为 529；content-set=`78c3edd1…d3ff`。

> 2026-08-26 当前 manifest 为 **529 schemas**。新增 `ProductionWeaponFormArtMeshProposalEdit@1` 是 `AuthoringMeshMoveVertices@1 | AuthoringMeshOpenFrameNotch@1` 闭合 union；后者仅有 `source_node_id/part_id/source-local/selection_policy/opening_width_milli/opening_height_milli/canonical_sha256`。它复用现有 proposal get/prepare，不增加 MCP tool，也不把真实 D1 的视觉拒绝改写成 PASS。

> 2026-08-26 当前 manifest 为 **528 schemas**，公共面仍为 **115 read + 87 write = 202 tools**。`ProductionWeaponFormArtProposalEvidence@1` 已注册并由 Runtime 物化为独立 CAS receipt + SQLite durable index；同输入 replay 与 Runtime drop/reopen replay/readback 均保持 receipt/canonical/identity hash 不变。真实 D1 六视图 Part-ID 全部 `observed`，但 negative-space、line-flow 与三处 `rear-stock` owner/open-void 严格门未全部通过，故 evidence=`BLOCKED_PROPOSAL_FORM_ART_EVIDENCE`、secondary=`NOT_CREATED`，不推进 Stage/confirm/version/export；`REVIEWABLE_TRADEOFF` 永远不是 PASS。

> 2026-08-26 `AuthoringMeshRevision@2` 增加 Runtime-derived optional `AuthoringMeshV2SourceBinding@1`：绑定 project/candidate/Part/source/material/operator、program/artifact/readback hashes 与原 transform；通用 durable MCP 不允许 caller 注入该字段。`production_weapon_authoring_mesh_v2_source_prepare` 使用闭合 inline request schema，从真实 candidate-owned box 生成 source-bound genesis。manifest 仍为 **527 schemas**；公共工具面为 **114 read + 86 write = 200**。

> 2026-08-26 现行 manifest 为 **527 schemas**。本轮新增 `AuthoringMesh@2` durable genesis/split/restart 的闭合 MCP wrapper，并增加 CameraLock registration lineage 只读 preflight 表面；真实 D1 方案的六视图拒绝结果是资产证据，不是新 Schema 可以改写的质量状态。

> 2026-08-26 商业合同方向新增研究约束：AuthoringMesh V2 的 Rust typed seam 已具备 corner/half-edge/stable identity/revision DAG 和一个局部 split-edge；`MaterialLayerGraph@1` 已进入 manifest，但仅是 plan validator，尚无纹理求值。`HighLowCorrespondence@1`、`AssetFramePolicy@1`、`SkeletalHierarchy@1`、`SocketSet@1`、`AnimationClipSet@1`、`GeometryTangentPolicy@1`、`TexturePackingPolicy@1`、`EngineImportProfile@1`、`ShaderParityReceipt@1`、`FirstPersonPerformanceBudget@1` 仍是目标合同，未进入 manifest 前不得宣称可用。详见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

> 2026-08-26 `FPS-FORM-04AE` 当前权威增量：合同面为 **525 schemas / 112 read + 84 opt-in write = 196 tools**。新增两个闭合合同 `MaterialLayerGraph@1` 与 `MaterialLayerGraphPlanResult@1`；Worker 只返回 `VALIDATED_PLAN_NOT_EVALUATED`，不能声明已经生成纹理。CameraLock registration lineage 仍保持 Runtime 唯一派生；真实 D1 尚无 orientation-specific user receipt，因此 positive/restart instance 仍 `NOT_RUN/NOT_CREATED`；Stage=`camera-calibrated`、secondary=`NOT_CREATED`、quality=`QUALITY_TARGET_NOT_MET`，不 confirm/version/export。

2026-08-26 FormArt attribution Schema 同步：`ProductionWeaponFormArtEvidenceGetRequest@1/GetResult@1` 的闭合诊断对象已在 fresh durable D1 上返回唯一 `rear-stock/548px` source，semantic match=true，所有 side-effect flags=false。正式商业增量不放宽 @1，也不复制整张 CameraLock：source ordering、authored orientation 与 `RegisteredCameraRigCalibration@2` 由 additive child lineage 持久绑定。manifest 当前为 525；新增材质图合同不改变真实 D1 positive/restart 的 `NOT_RUN/NOT_CREATED`。

2026-08-26 Schema 同步：manifest 现有 **518** 个 schema。4 个 Formal High wrapper 为 `production-weapon-formal-high-{prepare,get}-{request,result}`，复用 `ProductionWeaponHighArtifact@1`；MCP 已暴露对应 get/prepare，当前 194 tools。Store scoped idempotency 已实现；完整 positive/restart/cleanup 和质量 acceptance 仍 `NOT_RUN/NOT_PROVEN`，不能从合同与工具数量推导 High 通过。

2026-08-26 最新 Schema 口径：CameraLock registration lineage 的 5 个合同保持不变，并新增 `MaterialLayerGraph@1`、`MaterialLayerGraphPlanResult@1`，manifest 为 525。材质图 Worker 仅输出未求值执行计划，不允许将 plan 当作 PBR 纹理或商业材质结果。真实 D1 positive restart 仍因 orientation approval `NOT_RUN/NOT_PROVEN`。

2026-08-26 Cage/Bake 合同边界：现有 AuthoringMesh durable 可记录 source artifact/GLB lineage，Native High durable 可记录 High GLB/readback，既有 `ProductionWeaponHighArtifact@1` 已足以承载 Stage source candidate → distinct derived High candidate/High GLB 的正式记录；不新增第二套重复 binding Schema。Runtime resolver 已从 Stage transition/head 的真实 source candidate 读取 AuthoringMesh/NativeHigh，Formal High internal materializer 负责派生 distinct candidate、readback 与 receipt。当前未通过的是完整 source-lineage/CAS 正向 restart fixture 与独立 public request/result；真实 D1 仍以 `FORMAL_HIGH_STAGE_SOURCE_LINEAGE_UNAVAILABLE` fail closed、零写。不得用请求字段、相同 candidate 或语义 hash 冒充正式 High lineage。


新增正式合同分组：Low durable 5 个（prepare request/result、get request/result、link）；Hero UV 7 个（layout request/result、durable prepare request/result、get request/result、link）。Low 5 个已有 Contracts/Store/Runtime/MCP consumer；Hero UV 7 个现已形成 `hero_uv_durable_get/prepare` 的 Store→Runtime→MCP public producer/consumer。Hero UV durable 仍只证明 candidate-bound current Low exact provenance 的 structural/source lineage，不等于 artist-authored unwrap 或任何视觉/发布门。

> 2026-08-26 最新权威 source 口径（取代下方 2026-08-25 与本日早先计数）：**525 schemas / 28 operator entries / 112 read + 84 opt-in write = 196 MCP tools**。CameraLock registration lineage 只达到 source compile，真实 D1 positive/restart 尚未运行；Low quad draft 仍为 `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`；MaterialLayerGraph 仅验证 closed DAG plan，不求值纹理。AuthoringMesh V2/Native High/High↔Low↔Cage/bake 新 seam 也均未形成 visual、human、engine、commercial 或 packaged pass；Stage=`camera-calibrated`、visual=`QUALITY_TARGET_NOT_MET`、HQ360=`BLOCKED_REFERENCE_COVERAGE`，不推进 Stage、confirm、version 或 export。

> 2026-08-25 历史快照（已由上方 2026-08-26 权威口径取代）—合同口径：manifest 为 **499 schemas**。除 7 个 Native High durable/GLB 合同外，新增 `LowQuadDraftWorkerRequest@1` / `LowQuadDraftWorkerResult@1`；Hero UV 目前只有 Worker protocol/source producer，尚未注册 JSON Schema。Native High 当前 cohort restart **1/1 PASS**，Low/UV 不具备 Runtime durable truth；均不是 Stage/visual/human/engine/distribution PASS。

2026-08-25 `CQ-02-TYPED-TOPOLOGY-IDENTITY-LINEAGE`：`authoring_mesh_edit_preview → authoring_mesh_edit_prepare` 的 `split_edge / collapse_edge / dissolve_edge` proof 仍保持 source-element-only；下游 Runtime 现在只从 Store 的 exact candidate→idempotency response 恢复该 proof，并把 parent source identity 物化为 durable `AuthoringMeshIdentityLineage@1` child IDs、单调 tombstone 及 one-to-many/many-to-one relation，不接受 caller identity/proof arrays。真实 split/collapse/dissolve 已分别完成各自独立的完整持久化与 Runtime drop/reopen/get 重启链路，合计 **3/3 PASS**；Store `authoring_mesh_` **12/12**、MCP IdentityLineage **3/3**、490-schema checker与 Contracts/Store/Runtime/MCP 联合 compile PASS，工具数仍 **106 read + 78 write = 184**。general correspondence、evaluated retarget、完整 selection/undo history 与产品级 cross-version editor仍 `NOT_PROVEN`。Stage 保持 `camera-calibrated`，视觉=`QUALITY_TARGET_NOT_MET`，human/engine/distribution=`NOT_RUN`，HQ360=`BLOCKED_REFERENCE_COVERAGE`。新回执：`docs/evidence/mcp010f/authoring-mesh-typed-topology-identity-lineage-materialization-source-gate-20260825.json`；原 source-proof 回执继续作为上游证据。

2026-08-25 Native High schema/transport 口径：proposal 的公共 Schema 副本已 byte-exact 同步；Worker/GLB/Runtime CAS/Store/restart 与公共 MCP source-focused receipt 通过。这些 Schema 仍只证明 source durable boundary，proposal `registered=false`、`FPS-HIGH-05=NOT_PASSED`；总体 Stage/visual/human/engine/distribution/HQ360 状态不变。

> 2026-08-25 商业资产 Schema 规划历史快照（当前口径见顶部）：当时 manifest 为 **499 schemas**。split/collapse/dissolve 的 durable full-chain restart **3/3 PASS**。新 `HighMeshWorkerRequest@1` / `HighMeshArtifact@1` 定义 standalone structural prototype；Worker Protocol closed envelope与未注册 proposal bundle 已有 source evidence，但仍不等于 Runtime/CAS/GLB/package 产品能力。

目标合同与现有结构性合同不是别名，也不能互相冒充：

| 商业目标 | 当前可关联的 source contract / operator | 当前解释 |
|---|---|---|
| `WeaponArtBrief@1` / `ArtDecisionReceipt@1` | 无完整 live producer/consumer 链 | target / unavailable |
| `AuthoringMesh@1` / `AuthoringMeshIdentityLineage@1` | read-only projection + three-object restart + typed proof→durable identity | partial structural；split/collapse/dissolve 独立 full-chain 3/3 PASS；general/evaluated/完整编辑与商业 half-edge editor未证明 |
| `HighMeshArtifact@1` | `ProductionWeaponHighArtifact@1` | provisional source artifact；无正式 detail approval |
| `LowMeshArtifact@1` | `ProductionWeaponLowArtifact@1` + `LowQuadDraftWorkerResult@1` + `LowQuadDraftDurable*` | explicit quad draft 已接 Runtime/Store/MCP source durable；当前 Low 保持 candidate-bound exact provenance，仍 `DRAFT_UNREVIEWED`，artist edge-flow review/promotion 未通过 |
| `HeroUvLayout@1` | `HeroUvLayoutRequest@1` / `HeroUvLayout@1` + `HeroUvDurable*` | 7 个合同已注册并由 `hero_uv_durable_get/prepare` 完成 Store→Runtime→MCP；真实 replay/drop/reopen/get **1/1 PASS**，四个 CAS roots linked/GC；仅 structural/source，不是 artist unwrap、visual、人评、engine、commercial 或 packaged PASS |
| `CageArtifact@1` / `HighLowBakeReceipt@1` | `ProductionWeaponCageArtifact@1` 与 prefixed High/Low/Bake contracts | diagnostic/source-only；当前 bake quality failed |
| `MaterialLayerGraph@1` / `HeroMaterialPack@1` | `MaterialLayerStack@1`、`MaterialPackManifest@2` | 固定结构/公式预览；无完整 Layer/Mask/Generator/Wear 图 |
| `EngineValidationReceipt@1` | `GameEngineImportReadiness@1` 与历史 consumer receipts | engine-neutral/readiness 或历史结构回读；不能代替目标商业引擎 Gate |
| `HeroArtReviewReceipt@1` | `HumanVisualReviewReceipt@1` | 简单人评结构；无独立资深武器艺术家盲审、原创/IP 与同 export hash 绑定 |

不得通过复制目标名称新增空 Schema；只有 Schema、Runtime producer、Store/restart、MCP、Viewer、negative Gate 与同 cohort receipt 闭环后，目标能力才可从 `unavailable` 晋级。

2026-08-23 新增 FORM evidence 事实：`ProductionWeaponFormEvidence@1` parent + 六个 ordered `ProductionWeaponFormEvidenceView@1` children 已有真实 same-cohort source receipt，绑定 project/candidate/artifact/reference/view/camera/render-set hashes；6 views、54 AOV、7 owned receipt objects、same-key replay 0 new、restart verified、retarget zero-write。Part-ID 只能标 `observed`，negative-space 只能标 `inferred|unknown`，line-flow 标 `unknown`；`quality_status=NOT_PROVEN`、`QUALITY_TARGET_NOT_MET`、不推进 Stage。下一任务为独立 `FPS-HIGH-LOW-CAGE-05` contracts/gates；不得把 GLB/readback 或该 evidence 合同解释为 Hero Asset topology、PBR 或商业质量。

2026-08-22 `CandidateMaterialSurfaceQuality@1` public positive fixture：`Geometry → CandidateTopologyQuality@1 → AppearanceProgram@3 → TextureBuild@2 → SurfaceBake@1 → AppearanceSourceLineage@1 → CandidateMaterialSurfaceQuality@1` 的 `prepare → same-key replay → get → Runtime drop/reopen → restart get` 通过 **1/1（111.72s）**；Runtime focused **5/5**、Store full **74/74**、Contracts **350**。CAS inventory unchanged；stable `artifact_id` 与 GLB object SHA-256、MaterialPack CAS kind 精确区分，合法 UV/tangent rebuild 不计入 geometry-preservation 漂移。该结果仅为 `structural_only`；V2 animated-socket-particles 仍无完整 public `prepare → Store → restart get`，durable end-to-end=`NOT_RUN`/`BLOCKED_FIXTURE_CHAIN`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/candidate-material-surface-quality-public-positive-source-gate-20260822.json`。

最终同 cohort 修订口径：强制 build cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` 下 material focused **5/5（112.63s）**；Runtime full **310 passed / 0 failed / 20 ignored**（330 total，201.91s），且 public material fixture 明确在该 full run 内执行。此前 **111.72s** 仅为 public fixture 单测时长；两者都只支持 `structural_only`，不提升 visual/commercial、human/engine 或 stage/confirm/version/export 状态。

数值口径：当前 manifest 为 **515 schemas**；MCP surface 为 **111 read + 83 opt-in write = 194 tools**。7 个 Hero UV durable contracts 由公共 `hero_uv_durable_get` 与显式写 `hero_uv_durable_prepare` 承载，已完成 Store→Runtime→MCP 与真实 prepare/replay/drop/reopen/get **1/1 PASS**；四个 Hero CAS roots 已 linked/GC。当前 Low 输入保持 candidate-bound exact provenance；该 slice 仅为 structural/source evidence，不是 artist unwrap、visual、人评、engine、commercial 或 packaged pass，不推进 Stage/confirm/version/export。本文其余较小数值均只作 historical prior slice 保留。

2026-08-22 `FictionalEnergyVfxAnimatedSocketParticlesSequence@2` 双候选 source slice：Contracts **350**；Store V2 focused **2/2**、Store full **74/74**；Runtime V2 仅低层 focused **6/6**、cargo check **PASS**；MCP V2 **3/3**；同 cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` Runtime full **309 passed / 0 failed / 20 ignored**（191.06s）、MCP full **128 passed / 0 failed / 0 ignored**（1.93s），这些是全量回归，不是 V2 public `prepare → Store → restart get` 正向 fixture。V1/V2 隔离；V2 仅证明 1..16 frame、geometry/appearance 双 candidate/delivery/AnchorSet bridge 以及 Store FK/reachability/idempotence/conflict/rollback 的结构面。完整双候选 public Runtime `prepare → Store → restart get` 正向 fixture 尚不存在，durable end-to-end=`NOT_RUN` / `BLOCKED_FIXTURE_CHAIN`，不能声称正向 durable。该 slice 为 `structural_only`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/fictional-energy-vfx-animated-socket-particles-v2-dual-candidate-source-gate-20260822.json`。

2026-08-21 `GodotGameWeaponImportReceipt@1`：新增一个 closed 聚合合同，绑定 delivery manifest、static socket key 与 3 个 LOD derived hashes、animated socket key 与 derived hash、CollisionProxySet canonical sidecar hash、Godot binary/version/build、fixed harness hash 和 4 个 imported scene projections。真实 Godot `4.7.2.stable.official.ed1daf0bf` headless evidence 已通过：LOD triangles `304 > 176 > 112`，每 scene 5 meshes/相同 materials/6 non-rendering sockets parent/local TRS exact，10 glTF channels→2 semantic tracks、cross-loader t0/half-duration TRS exact、两个 named socket follow、5 个 `BoxShape3D` sidecar readback exact。static/animated 来自独立 Runtime source cohorts；没有 Runtime durable Godot link。commercial engine=false；Unity/Unreal、physics/hitbox、visual/human、confirm/export `NOT_RUN/false`，quality `structural_only`。当前合同总数 **291**，工具仍 **69 read + 49 write = 118**。

2026-08-20 `geometry-program-v2.schema.json` 的 closed `$defs` 新增 `energy_core_parameters`，并以 exact operator→parameter branch 绑定 `forgecad.geometry.energy-core@1`；component 限于 guard-ring、mechanical-ring、emitter-core、mechanical-backplate，schema 文件总数仍为 195。ActionRun/Critic/RepairIntent、Modifier Stack、PDK 和 fictional-energy-rifle profile 的 operator 枚举同步，禁止跨分支参数与未知可执行字段。

2026-08-20 Candidate-bound Viewer Provenance Graph 新增 `viewer-provenance-graph-request.schema.json` 与 `viewer-provenance-graph.schema.json`。它只读绑定 exact project/candidate/candidate-state/artifact，完整或失败地投影 Geometry evidence、Operator DAG、GLB、strict readback、geometry quality，以及可验证时的单个 visual/AOV 和 MechanicalAnimationClip 分支；固定 64 nodes / 128 edges / 1 MiB，不内嵌 GLB/PNG，不写 Runtime 状态。缺少独立 durable 历史的 Modifier Apply、Boolean Preview、Subdivision sidecar 与 DesignSession 必须作为 omitted/unknown 明示，结构证据不能提升视觉或 360°质量。该 slice 当时 manifest 为 195 schemas，MCP 工具为 54 read + 36 opt-in write = 90。

2026-08-19 candidate-bound Modifier Apply slice 新增 `geometry-modifier-apply-request.schema.json` 与 `geometry-modifier-apply-result.schema.json`。Request 只接受公开可读的 durable candidate/artifact/readback/program/catalog/config、一个 stable Part、闭合 modifier stack、current head/idempotency/input hash 与 1 MiB 边界；Result 绑定 source/new candidate、source/derived Worker cohort、program/artifact/readback/evidence、target terminal、preserved Part IDs、immutable CAS sidecar 与 `structural_only`。它不允许 Python/plugin/reference override，不 confirm/version/export，不代表视觉质量。该 slice 当时 manifest 为 191 schemas。

2026-08-19 Authoring Mesh Edit Prepare slice 新增 `authoring-mesh-edit-prepare-request.schema.json` 与 `authoring-mesh-edit-prepare.schema.json`。Request 封闭 project/source/current base、完整 preview、expected preview hash、幂等键、1 MiB 和 input hash；Result 封闭 source/new candidate、program/artifact/readback/evidence/Worker cohort/edit lineage、Job、审批锁、无 version/export、structural-only limitations 与 canonical hash。该 slice 当时 manifest 为 191 schemas。

2026-08-19 historical Authoring Topology/Edit Preview slice 新增 `authoring-topology-request.schema.json`、`authoring-topology.schema.json`、`authoring-mesh-edit-preview-request.schema.json` 与 `authoring-mesh-edit-preview.schema.json`。Topology request 只接受公开 `geometry_prepare` 可获得的 candidate/artifact/readback/program/catalog/config/node/Part/policy/budget 绑定；Runtime 内部从 durable evidence 派生并重验 GeometryProgram object/canonical。Topology 输出 exact source V/E/Loop/Face 与 node/Part lineage；Edit Preview 仅允许 sorted translate 或 boundary triangle/planar-convex-quad single-face extrude，并绑定 source/derived program/topology/replay hashes、no-write、structural-only 与 1 MiB。该 slice 当时 manifest 为 187 schemas，现行总数见本文顶部 195。

2026-08-19 historical Render Evidence Replay slice 新增 `render-evidence-replay-request.schema.json` 与 `render-evidence-replay.schema.json`。Request 封闭 candidate-state hash、完整 Integrity request 和固定 read-only replay policy；Result 封闭 exact candidate/artifact/camera/RenderSet/request/integrity hashes、artifact-only appearance 限制、同 cohort/profile 绑定、有序九 AOV source/first/repeat raw-byte 与 decoded-pixel hashes/sizes、no-write、1 MiB 与诚实 limitations。该 slice 当时 manifest 为 177 schemas，现行总数见本文顶部 195。

2026-08-19 historical manifest slice 新增 `mechanical-pose-geometry-preview-request.schema.json` 与 `mechanical-pose-geometry-preview.schema.json`。Request 封闭嵌套 single-tick pose request、固定 transient preview policy 与 outer input hash；Result 封闭 exact source lineage、application policy、per-Part rest/posed/delta、derived GeometryProgram、transient artifact strict readback、no-write、structural-only、limitations 与 canonical hash。该 slice 当时 manifest 为 175 schemas；现行总数为 195。

2026-08-19 historical manifest slice 新增 `subdivision-artifact-lineage-sidecar-request.schema.json`、`subdivision-artifact-lineage-sidecar.schema.json` 与 `subdivision-artifact-lineage-link.schema.json`。Request 封闭 prepare/get 共同输入；Sidecar 继承并收紧 exact artifact lineage payload，固定 immutable CAS/structural-only/no-cross-version；Link 绑定 request/candidate/artifact/readback/evidence/node/sidecar/lineage/artifact-binding hashes并嵌入完整 sidecar，无时间戳以保持幂等。该 slice 当时 manifest 为 173 schemas。

2026-08-19 historical manifest slice 新增 `subdivision-artifact-lineage-request.schema.json` 与 `subdivision-artifact-lineage-projection.schema.json`。Request 封闭 candidate/artifact/readback/node/budget/canonical；Projection 复用完整 root-lineage schema，并封闭 durable evidence/program/catalog/config hash、唯一 direct source primitive、full-GLB replay identity、primitive-local triangle ranges、no-write、not-persisted、structural-only 和 canonical。该 slice 当时 manifest 为 170 schemas。

2026-08-19 当前 manifest 新增 `subdivision-topology-lineage-request.schema.json` 与 `subdivision-topology-lineage.schema.json`。Request 封闭 exact `GeometryProgram@2`、target `subd-cage@2` node、1..25,000 element budget 和 canonical；Result 封闭完整作用域内的 control vertex/edge/quad root → evaluated quad topology 数组，固定 program/evaluation-bound ID、no artifact/readback/GLB binding、no corner/child-path/weight、no-write、`structural_only` 与 canonical/lineage hash。该 historical slice manifest 为 168 schemas。

2026-08-19 historical crease-aware manifest slice 新增 `subdivision-crease-evaluation-request.schema.json` 与 `subdivision-crease-evaluation-result.schema.json`。Request 是 closed、normalized-input-hash-bound 的 3..16 grid / 1..2 level / integer edge sharpness policy；Result 绑定 control-cage/crease/policy/topology/program/catalog/canonical hash并明确 `structural_only`、read-only projection 未编译、无 visual claim。`geometry-program-v2.schema.json` 另新增独立 `subd_cage_crease_parameters`，保持旧 `subd_cage_parameters` 不变。该 slice 当时为 166 schemas。

2026-08-19 historical Boolean Operand Lineage slice 当时 manifest 为 164 schemas。新增 closed `BooleanOperandLineageRequest@1` / `BooleanOperandLineage@1`：请求完整复用 `GeometryProgram@2` 并绑定 Boolean node、1..4096 run budget 与 canonical；结果固定两项 ordered operand、1..4096 连续 runs、lineage hash、no-write 和四项限制，Runtime 会从请求重算 operation/operand/source lineage，明确 evaluated face ID 不是原始 authoring face、跨 program 不稳定且未持久化到 GLB。receipt：`docs/evidence/mcp010f/blender-boolean-operand-lineage-source-gate-20260819.json`。

2026-08-19 historical Render Evidence Integrity slice 当时 manifest 为 162 schemas。新增 closed `RenderEvidenceIntegrityRequest@1` / `RenderEvidenceIntegrity@1`：request 绑定 13 项 exact scope/hash 与自身 canonical；result 分离 ArtifactReadback/RenderSet/comparison/quality 的 object/canonical hash、AOV `cas_object_sha256`/`bytes_sha256`、comparison mask、same-camera identity、RenderProfile 和 8 项 threshold gate lineage，固定 read-only/no-write/1 MiB 与历史 receipt 不修复限制。

2026-08-19 historical Mechanical Pose Sequence Preview slice 当时 manifest 为 160 schemas。新增 closed `MechanicalPoseSequencePreviewRequest@1` / `MechanicalPoseSequencePreview@1`：请求绑定 existing RestFrame/PoseAction draft 和 1..16 个 tick；Runtime 强制严格递增/唯一/时长范围、逐 sample semantic recomputation、完整 lineage-bound ordered sequence hash、1 MiB canonical response cap 与 `structural_only` / no-write；MCP 另对 summary + `structuredContent` 的整个 `tools/call` response 执行 1 MiB wire Gate。receipt：`docs/evidence/mcp010f/blender-mechanical-pose-sequence-preview-source-gate-20260819.json`。

2026-08-18 historical Parametric Group v2：该 slice 当时 manifest 为 158 schemas。新增 closed `ParametricDesignKitRequest@2` / `ParametricDesignKitProgram@2`，绑定 immutable template/catalog、parameters、instance、operator catalog、GeometryProgram、evaluation order、source map 与 canonical hash，并强制 `structural_only` / no-write。

2026-08-18 historical Mechanical pose：新增 `MechanicalRestFrame@1`、`MechanicalPoseAction@1`、`MechanicalPoseEvaluationRequest@1` 与 `MechanicalPoseEvaluationResult@1`，该 slice 当时 manifest 为 156 schemas。合同限定 64-link 无环机械层级、fixed/revolute/prismatic 单自由度关节、1000 Hz 整数 tick linear/clamp/rest action、candidate/artifact/readback/program/catalog/config 与 exact Part/source-node lineage；结果固定 `geometry_materialization=not-materialized`、`worker_evaluation=not-run-runtime-read-only-projection`。Schema 不声明 Armature、skin、IK/NLA/F-Curve、动画资产、package/live 或视觉 PASS。

2026-08-18 historical RenderProfile slice：新增 closed `RenderProfile@1`，该 slice 当时 manifest 为 152 schemas。它固定 ForgeCAD 自有 `forgecad-fixed-software@2` CPU raster backend、512×512、deterministic sampling、linear Rec709 D65 → fixed sRGB beauty transform、opaque film 与九个有序 AOV，并绑定 canonical/AOV/color-pipeline/ID-palette hashes；`RenderSet@2` 强制内嵌并复述这些 hashes。Schema 不声明 Blender/Cycles/EEVEE/OCIO/GPU/EXR/custom AOV、package/live 或视觉 PASS。前一 Subdivision v2 的 151-schema 状态保持历史 slice。

2026-08-18 historical Modifier evaluation v2 slice：新增 `GeometryModifierEvaluationRequest@2`、`GeometryModifierEvaluationSignature@1` 与 `GeometryModifierEvaluationResult@2`，该 slice 当时 manifest 为 149 schemas。Request 复用 closed Modifier Stack base/modifier 定义并只接受 null 或 canonical previous signature；source base operator/parameters 由 11 个 paired branches 绑定。Signature 绑定 ordered stage definition/input/output/cache chain，cache key 额外绑定 GeometryProgram canonical hash；Result 明示 `reuse_kind=semantic-signature-only` 与 `output_kind=geometry-program-canonical-sha256`。Schema 不声明真实 mesh cache、GLB readback、build cohort、视觉质量或用户批准。

2026-08-18 historical Bevel/Normal slice：`GeometryProgram@2` 的 closed operator/parameter set 与 `GeometryModifierStackRequest@1` 的 oneOf 已扩展 `bevel@1` / `normal-policy@1`，该 slice 当时 schema 总数为 146。Bevel 参数固定 direct source box、width/segments/profile/edge-scope/clamp；Normal 参数固定 corner-domain face-area × corner-angle、crease threshold、`keep_sharp=true`。Schema 接纳不单独构成执行 PASS；执行证据见 `docs/evidence/mcp010f/blender-bevel-normal-source-gate-20260818.json`。

2026-08-18 historical TopologySnapshot slice：新增 `TopologySnapshotRequest@1` 与 `TopologySnapshot@1`。请求是 closed、project/candidate/artifact/readback/program/catalog/config/policy 全绑定的单 Part readback；结果固定 `scope=part`、`complete=true`、`topology_space=evaluated-glb-triangle-mesh@1`、`id_scope=artifact-bound`、`cross_version_stable=false`、`quality_status=structural_only`，并保存有界 V/E/F/C、邻接、拓扑计数、lineage hash 与 corner normal/UV/tangent。该 slice 当时 manifest 为 146 schemas；下文 136/138/139/144 是更早 historical 数字。

2026-08-18 Modifier slice historical cohort：新增 `GeometryModifierStackRequest@1` 与 `GeometryModifierStackProgram@1`。前者是 closed、1..8 项、有序且 input-hash-bound 的只读 authoring request；v1 只允许 active unary transform/mirror/array。后者严格 `$ref` 完整 `GeometryProgram@2`，并返回 program/stack/canonical hash 与逐 stage effective evaluation hash，固定 `quality_status=structural_only`。它不表示已编译 mesh、candidate、视觉质量或用户批准。该 slice 当时 manifest 为 144 schemas；现行值以本文顶部的 499 为准。

版本：2026-08-13
2026-08-17 Reference Visual Structure：新增 `reference-visual-structure.schema.json`，作为 `SilhouetteTarget@1.visual_structure` 的可选嵌套合同。它保存重叠/共享视觉区域、连续曲面组、层级、深度证据和开放 line-flow；全局 silhouette 保持最高权威并禁止把视觉区域声明为功能部件。该 slice 当时 manifest 为 139 Schema；现行总数为 195 Schema。
2026-08-17 PDK v0：新增 `parametric-design-kit-request.schema.json` 与 `parametric-design-kit-program.schema.json`，由只读 `geometry_program_hash` 分支消费/生成。两者只描述 Runtime-owned typed macro 的输入、单节点 GeometryProgram 展开、Part/MaterialZone/parameter source map 与 structural-only 限制；不声明 candidate、视觉相似、PBR 或用户批准。
2026-08-17 历史 Stage 0 覆盖先后为 138 与 144 个 Schema；144-schema slice 当时为 41 read + 33 opt-in write = 74 tools。`fictional-energy-rifle-profile.schema.json` 与 `fictional-energy-rifle-plan.schema.json` 已进入 manifest，Profile 仍是 nonfunctional、source-only authoring aid；`repair-intent-run-request/result.schema.json` 已进入 manifest，结果保持 `confirm_allowed=false`，不能代替视觉质量或完整 orchestrator。当前机器真值为 515 schemas、111 read + 83 opt-in write = 194 tools。
状态：MCP005–MCP009 functional core 已落地；MCP006 历史 receipt 为 44 个 JSON Schema；MCP010B/C/D/E/F source 合同、Agentic Design Runtime contract family、weapon joint-multiview V2 contract family、fictional energy-rifle Profile/Plan、Modifier Stack、Modifier evaluation v2、TopologySnapshot、Subdivision evaluation v2、RenderProfile、Mechanical pose、Subdivision crease、Authoring Mesh Edit Prepare、typed particles、typed trails、静态 GLB sockets、动画 GLB sockets 与 Hero UV durable contracts 已进入当前 manifest，当前源合同总数为 515。公共 `hero_uv_durable_get/prepare` 已完成 Store→Runtime→MCP，但四根 CAS root 与真实 replay 只证明 structural/source lineage，不是 artist unwrap、visual、人评、engine、commercial 或 packaged pass；不推进 Stage/confirm/version/export。`RuntimeJobResult@1`、`repair-intent-run-request/result.schema.json`、Primary Form async Job 及 weapon coordinate/camera-rig/optimization 相关合同为长时间 Job、CAS-bound staged run 和多视图 promotion 的 hash-bound 边界。唯一 `in_progress` 为 `FGC-MCP010F`；历史 package/live receipt 仍按 cohort 单独保存。Agentic observe/plan 的真实 Runtime 嵌套只读 projection 已通过 producer/consumer conformance checker；durable session/checkpoint/RepairIntent prepare/readback、CAS-bound RepairIntentRun、窄范围 Primary Form action-run/readback 与本轮 joint source Gate 已各自有 source/runtime/MCP/隔离证据，但不等于 durable/reference/DesignSpec 完整 producer conformance、完整 Visual Evidence conformance、通用 orchestrator、Repair 应用或视觉 likeness。

Stage 0 机器真值为 `docs/evidence/mcp010f/current-benchmark-truth.json`；当前源码口径固定为 518 Schema、111 read + 83 opt-in write = 194，并绑定当前 Schema 文件内容集合哈希。attempt35 只是 provisional retained observation，为 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，fit/compare camera 为 `MISMATCH`，packaged Viewer binding 为 `PASS_CURRENT_COHORT_BOUND_READ_MODEL`（不等于 attempt35 same-observation E2E）。Hero UV durable source receipt 只增加 Store→Runtime→MCP structural lineage，不补齐 artist unwrap、PBR likeness、正式真人、engine/commercial、packaged 或 export/restart 门；不推进 Stage/confirm/version/export。Schema/producer 已实现不能补齐缺失 receipt 字段，也不能越过 360 门。

`ReferenceCanvas@1` 的 view 项现可选绑定 `view_spec`、`target_sha256`、`mask_sha256` 与 `camera_claim.camera_canonical_sha256`；target/mask 必须成对出现，Runtime 还会检查它们与同一 `reference_id/reference_sha256`、CAS、相机和 evidence 的 lineage。`VisualEvidenceBundleProjection@1` 会投影这些 per-view hash，跨视图 compare 不得使用另一视图的 target，RepairIntent 的 evaluation kind 集合必须与 `coverage.supplied_views` 一一对应。旧 unbound 单视图仍显式使用 null，不能将兼容字段缺失解释为质量通过。


<!-- forgecad-reference-source: input=ENV_AUTHORIZED_PNG original_sha256=1964704a62ed7a841b4d49c370b8d46f4626e201daad29092a9c39a40b4c4109 intake=PASS_SOURCE_SIX_REFERENCE_EVIDENCE_CAS views=6 worker=PASS_SAME_COHORT_SIX_FIXED_VIEWS target=USER_REFINED_USER_CONFIRMED_REVIEWED_STRUCTURE user_confirmed_crop=PASS_USER_CONFIRMED_SEVEN_CROPS contour=PASS_USER_CONFIRMED_SIX_IDENTITY_CONTOURS negative_space=BOUNDING_REGIONS_CONFIRMED_EXACT_SUBTRACT_UNKNOWN line_flow=EXPECTED_ROWS_DURABLE_MATCH_NOT_PROVEN camera_lock_fixture=PASS_REAL_DURABLE_REPLAY_RESTART form_art_fixture=PASS_REAL_DURABLE_NOT_PROVEN form_quality_v2_fixture=BLOCKED_ZERO_WRITE_MISSING_LEGACY_CROSS_VIEW secondary_form_approved=NOT_CREATED fixture=PASS_REAL_1_OF_1_108.07S -->

## 1. 唯一来源

新合同源位于 `packages/forgecad-contracts/schemas/**`。MCP003 已验证首批 15 个 JSON Schema；MCP004 增加审批、候选、restore 和 diagnostic export records；MCP005 增加 reference admission/get records；`[transition-v1]` MCP006–009 已落地 `SubjectProfile@1`、`RepresentationPlan@1`、`AssemblyGraph@1`、`GeometryProgram@1`、`AppearanceProgram@1`、`RecipePlan@1`、`ArtifactReadback@1`、`RenderSet@1`、`QualityReport@1`、`ChangePrepareResult@1`、GLB export profiles 和 Skill manifest/list/get/receipt/eval records，共 44 个历史 JSON Schema。MCP010B/C/D/E/F、Agentic contract family 与 weapon joint-multiview V2 family 继续增加当前 V2/evidence/target/camera/Rig/fit/Part/candidate compare/session/checkpoint/RepairIntent/action-run/Job result/RepairIntentRun/Modifier Stack/TopologySnapshot/Subdivision evaluation/RenderProfile/Mechanical pose/Subdivision crease/Hero UV durable Schema，当前 manifest 共 518；`RenderSet@2` 另绑定 Render Worker cohort/status 与完整 RenderProfile lineage。这不改写 44-contract 历史 receipt。全部 Schema 均须可解析、带 draft/id、contract manifest 为 `forgecad-runtime-contracts@1` 且声明 `model_calls=false`，manifest 与目录无漂移。Rust records 由 `forgecad-contracts` 维护；完整生成器、TypeScript 生成和额外 transport/未来宿主 conformance 仍未完成。旧 Concept/Weapon/Provider/Agent Schema 已删除。

## 2. 首批 Schema

### Runtime/MCP

`RuntimeCapabilities@1`、`RuntimeTool@1`、`RuntimeProject@1`、`RuntimeSnapshot@1`、`RuntimeJob@1`、`RuntimeJobResult@1`、`RuntimeError@1`、`RuntimeJobEvent@1`、`RuntimeResourceContents@1`、`Selection@1` 已落地；MCP annotations/resources Schema snapshot 在 `docs/evidence/mcp003/`。

### Project/Version

`Project@1`、`ActiveDesignSnapshot@1`、`Candidate@1`、`DesignAssetVersion@1`、`CasObject@1`、`AuditEvent@1`、`SemanticChangeSet@1`、`ApprovalReceipt@1`、`ExportManifest@1`。

### Reference/Design/Geometry/Appearance（分阶段落地）

MCP005 已落地 `ReferenceEvidence@1` 和四个 reference import/get request/result 合同；MCP006 已落地 `SubjectProfile@1`、`RepresentationPlan@1`、`AssemblyGraph@1`、`GeometryProgram@1`、`AppearanceProgram@1` 和 `RecipePlan@1`。`[transition-v1]` 这些 `@1` 几何/外观合同只保留历史结构兼容；当前 high-quality authoring 采用 `GeometryProgram@2` detail、`ArtifactReadback@2` strict readback、`AppearanceProgram@2`、`RenderSet@2` 九 AOV 和 candidate-bound strict compare。Agentic 的 `DesignSession@1`、`DesignCheckpoint@1`、`RepairIntent@1` 等公开合同由 Runtime 受限 prepare/readback slice 使用，内部 SQLite 记录不作为新的几何真值。

### Evidence/Skill（MCP006–009 已落地合同，执行证据仍分层）

MCP006 已加入 `ArtifactReadback@1`、`RenderSet@1`、`QualityReport@1`、`SkillBundleManifest@1`、`SkillListResult@1`、`SkillGetResult@1`、`SkillExecutionReceipt@1`、`SkillEvalReport@1`，MCP009 加入 `ChangePrepareResult@1`、GLB export profile 和 limited quality projection；`RecipePlan@1` 的单位/坐标/确定性顺序/max_edges 是显式合同。`SkillGetResult@1` 现内联 hash-bound `SkillKnowledge@1`（overview/constraints/examples），使 Codex 可在不读本机路径的情况下读取 first-party `ponytail-preflight@0.1.0`。MCP010C 已实现 `VisualReviewReport@1`、landmark/region metrics、九 AOV compare 及 Codex/human review 合同/工具接口；attempt35 的 Codex typed review 已运行但需要修订，独立真人 receipt 仍 `NOT_RUN`。完整生产 export/restart 与发布仍是后续工作，不得用空 Schema 或已存在接口代替。

## 2.1 MVP 落地顺序

| Task | 新增合同 |
|---|---|
| MCP005 | `ReferenceEvidence@1`、`ReferenceImportRequest/Result@1`、`ReferenceGetResult@1`（已完成） |
| MCP006 | `SubjectProfile@1`、`RepresentationPlan@1`、`AssemblyGraph@1`、`GeometryProgram@1`、`AppearanceProgram@1`、`RecipePlan@1`、MVP Skill manifest/receipt |
| MCP007 | `GeometryProgram@1`、`GeometryPrepareResult@1`、`ArtifactReadback@1`、Part/source map、worker compile request/result |
| MCP008 | `AppearancePrepareResult@1`、`RenderSet@1`、GLB UV/tangent/PBR readback |
| MCP009 | `QualityReport@1`、`ChangePrepareResult@1`、`VersionDiff@1` projection、`ExportManifest@1` `mvp-glb` profile |
| MCP010B | `GeometryProgram@2`、`OperatorCatalog@1`、`GeometryProgramHashRequest@1`、`GeometryProgramHashResult@1`、`ArtifactReadback@2`、`GeometryPrepareResult@2`、`GeometryQualityReport@2`、`GeometryCandidateEvidence@1` |

不能一次加入全部空 Schema 再宣称能力存在；每项必须与 validator、negative tests 和实际 producer/consumer 同任务落地。

## 2.2 MCP010 与 Agentic 合同（当前源合同 518；历史 44 receipt 不改写）

| Task | 目标合同 | 激活条件 |
|---|---|---|
| MCP010B | `GeometryProgram@2`、`OperatorCatalog@1`、`GeometryProgramHashRequest@1`、`GeometryProgramHashResult@1`、`ArtifactReadback@2`、`GeometryPrepareResult@2`、`GeometryQualityReport@2`、`GeometryCandidateEvidence@1` | 当前源码的 producer/consumer、真实 GLB JSON/BIN/accessor readback、closed GLB profile、V2 restore hardening 和损坏输入负向 Gate 已通过；当前 `d9c23b…ac0bd` Dev.app 已通过 ad-hoc/package、隔离/raw、真实 Codex CLI structural 和完整重启后的 live Desktop structural Gate |
| MCP010C | `ReferenceViewSpec@1`、`CameraCalibration@1`、`RenderSet@2`、`ReferenceComparisonReport@1`、`VisualReviewReport@1`、`HumanVisualReviewReceipt@1`、`QualityReport@2` | perspective/z-buffer renderer、九 AOV、metric/review persistence、tool E2E |
| MCP010E | `MaterialPackManifest@1`、`MaterialDefinition@1`、`TextureSet@1`、`TextureBuildReceipt@1`、`AppearanceProgram@2`、`AppearancePrepareResult@2` | AssetPack/UV/tangent/PBR producer、逐资产 provenance、GLB readback |
| MCP010F | `ReferenceMaskPrepareResult@1`、`SilhouetteTarget@1`、`CameraFitResult@1`、`CameraCalibrationRef@1`、`SilhouetteRig@1`、`SilhouetteRigHashRequest@1`、`SilhouetteRigHashResult@1`、`SilhouetteFitIntent@1`、`SilhouetteFitResult@1`、`PartContourFitResult@1`、`SilhouettePartErrorResult@1`、`SilhouetteCandidateCompareResult@1`、`BoundaryErrorResult@1`、`PrimaryFormAcceptance@1`（嵌入 `PrimaryFormRepairPrepareResult@1`） | Runtime-owned target/mask、bounded camera/Rig fit、hash-only calibration reference、single/multi-Part contour attribution、same-camera source/proposal retention and candidate compare |
| MCP010F Hero UV durable | `HeroUvLayoutRequest@1`、`HeroUvLayout@1`、`HeroUvDurablePrepareRequest@1`、`HeroUvDurablePrepareResult@1`、`HeroUvDurableGetRequest@1`、`HeroUvDurableGetResult@1`、`HeroUvDurableLink@1` | `hero_uv_durable_get/prepare` 已完成 Store→Runtime→MCP；candidate-bound current Low exact provenance、四个 CAS roots linked/GC、真实 prepare/replay/drop/reopen/get **1/1 PASS**；仅 structural/source，非 artist unwrap/visual/human/engine/commercial/packaged pass |

工具 request/result Schema 随各自 producer 同任务增加；实际 manifest 数量只能从目录和 contract manifest 计算，不能把上表简单相加后提前写成当前总数。`@1` 历史版本继续只读，破坏性变化不得回填旧对象。

当前 high-quality contract path 固定为 `GeometryProgram@2` detail → `ArtifactReadback@2` strict readback → `AppearanceProgram@2`（受前序门控制）→ `RenderSet@2` 九 AOV + Render Worker cohort binding → `ReferenceComparisonReport@1` strict compare → `VisualReviewReport@1` / `QualityReport@2`。`[transition-v1]` `GeometryProgram@1` primitive-only、`AppearanceProgram@1` 与 `RenderSet@1` 四 pass 只属于历史兼容，不得提升为当前 high-quality contract path。

## 2.3 ADR-0026 合同与当前落地层级

Agentic contract family 已进入当前 manifest 并通过正/负 fixture checker，但必须区分“合同定义”和“producer conformance”。当前 Runtime 同时提供 `AgenticSceneObserveResult@1` 可重建只读 envelope，以及受批准的 `DesignSession@1`/`DesignCheckpoint@1`/`RepairIntent@1` prepare/readback slice。真实 Runtime 产生的 `AgenticSceneObserveResult@1` 与 `DesignStagePlanProjection@1` 嵌套只读投影已由 `scripts/check_agentic_projection_receipt.py` 对隔离回执完成 producer/consumer 校验；durable 对象已经在 Runtime SQLite/CAS 持久化并经 Runtime/MCP 重启 receipt 校验，但不代表 durable/reference/DesignSpec 完整 producer conformance、单动作 orchestrator 或 Repair 应用。隔离证据见 `docs/evidence/mcp010f/agentic-runtime-observe-plan-20260813.json`、`docs/evidence/mcp010f/agentic-runtime-projection-conformance-20260813.json` 与 `docs/evidence/mcp010f/agentic-runtime-session-checkpoint-20260813.json`。

| 目标合同 | 用途 | 激活条件 |
|---|---|---|
| `DesignSession@1` | 当前设计会话、stage、candidate/checkpoint binding、失败门 | Runtime producer、MCP read surface、negative tests 和真实 Codex evidence |
| `DesignCheckpoint@1` | stage checkpoint、rollback/restore intent、candidate/version refs | 不移动 confirmed head；必须绑定 CAS/quality hash |
| `DesignStagePlan@1` | 当前允许动作、禁止动作、下一步单 Part/MaterialZone intent | 只读工具先行；不得创建 geometry |
| `ReferenceCanvas@1` | multi-view reference coverage、observed/inferred/unknown、camera claims | 绑定 `ReferenceEvidence` CAS hash；缺视图阻断 360 |
| `DesignSpec@1` | category/style/primary forms/semantic parts/material language/stage criteria | Codex 生成草案，Runtime 校验范围和 hash |
| `SemanticSceneGraph@1` | part tree、role、dimensions、symmetry、source map、editability | 从 candidate/readback/source map 派生 |
| `ModelUnderstandingBundle@1` | SceneGraph + geometry stats + material zones + cameras + AOV/quality evidence + uncertainty | `scene_observe_get` producer 完成后才可用 |
| `VisualEvidenceBundle@1` | multi-view AOV、metrics、failed gate、hash-only manifest | 不保存原图路径或截图作为版本真值 |
| `DesignCriticReport@1` | evidence-bound issue、metric、threshold、part/material target | Codex typed critic 或 deterministic critic 输出，必须引用 evidence hash |
| `RepairIntent@1` | bounded single-Part/MaterialZone repair proposal | 只能进入 prepare/recompile/readback/compare；不得直接写版本 |
| `ParametricDesignKitManifest@1` | Housing/Panel/Vent/Joint/Sensor/Frame 等 macro catalog | 每个 macro 展开为 typed Geometry/Appearance program，并有 validator/benchmark |

新增这些合同前必须更新 contract manifest、Schema checker、producer/consumer tests、MCP tool docs、Viewer docs 和 evidence；不能只创建空 Schema。

`GeometryPrepareResult@2` 是闭合的短生命周期 MCP 返回，只包含 candidate、job、operator catalog 与 `ArtifactReadback@2`；它不应额外暴露持久 evidence。`GeometryQualityReport@2` 只表示 strict hard gate 已通过的 quality CAS receipt，失败走 typed rejection 而不是伪造 `hard_gate_passed=false` 的该 Schema。`GeometryCandidateEvidence@1` 是 Runtime/Store 的 candidate-bound durable provenance：它绑定 program、artifact、readback、quality、catalog/readback-config 和可选 reference hash，并由 confirm/restore reread 使用。当前 source-focused PASS 不等于新的安装包、Desktop live、PBR、reference similarity、human review 或 360°证据。

## 2.4 商业资产目标合同与模块清单（queued / future）

目标合同以创作真值为中心，而不是继续给 GLB 增加互不关联的 sidecar。最低闭合集合为：

- Design：`WeaponArtBrief@1`、`WeaponDesignLanguage@1`、`ReferenceViewSet@1`、`DesignDecisionLog@1`；
- Authoring：`AuthoringMesh@2`、`TopologyMutationJournal@1`、`StableElementIdentity@2`、`SelectionConstraintSet@1`、`ModifierDetailGraph@1`；
- Authoring revision/promotion：`AuthoringMeshRevision@1`、`TopologyEditProgram@1`、`TopologyEditReceipt@1`、`StableElementLineage@1`、`LowRetopoDraft@1`、`LowAuthoringMesh@1`；
- Production：`HighMeshArtifact@2`、`LowMeshArtifact@2`、`HighLowCorrespondence@2`、`HeroUvLayout@2`、`CageField@1`、`BakeSet@1`；
- Production diagnostics：`HighRecipe@1`、`HighEvaluatedArtifact@1`、`CageArtifact@1`、`BakeRayDiagnostics@1`、`HighLowBakeReceipt@1`；
- Surface：`MaterialLayerGraph@1`、`MaterialMaskSet@1`、`TextureBuild@2`、`SurfaceResponseReport@1`；
- Presentation：`WeaponPresentationRig@1`、`FirstPersonPresentationSet@1`、`AnimationClipSet@1`、`GameplayBeatSet@1`、`VfxCueSet@1`、`AudioCueSet@1`；
- Delivery：`HeroLodSet@1`、`CollisionSet@1`、`SocketSet@1`、`EngineDeliveryPackage@1`、`EngineValidationReceipt@1`、`EnginePerformanceReceipt@1`；
- Texture delivery：`TextureBuildReceipt@3`、`TexturePackageReceipt@1`，分别保存逻辑 source/master raster/decoded mip-chain/compressed bytes hash、encoder revision/thread/platform/profile；
- Approval：`HeroArtReviewReceipt@1` 与同 export hash 的 confirm/version/export/restart receipt。

所有 draft 类合同必须有 `promotion_eligible=false` 默认值、producer cohort、source/evaluated identity、review state 和 blocker codes。自动 retopo/UV/cage/material/LOD 结果只能由独立 review/prepare 转成 approved artifact，不能因 Worker 返回成功自动晋级。

以下合同用于把商业级缺口转换成可验证边界；它们是目标合同，不代表当前 manifest 已新增、也不代表 producer/consumer 已完成。当前 manifest 仍为 **518 schemas**，工具仍为 **111 read + 83 opt-in write = 194**；Low durable 与 Hero UV durable 的 structural/source 事实保持不变。

| 目标合同 / 模块 | 必须绑定的 typed 内容 | 当前状态 |
|---|---|---|
| `AuthoringMesh@1` | original half-edge V/E/H/C/F、loop/ring/boundary、stable element lineage、original/evaluated namespace 与 edit operation | partial structural；split/collapse/dissolve 3/3 PASS，general/evaluated/cross-version editor `NOT_PROVEN` |
| `Native High` / `HighMeshArtifact@1` | `DetailGraph@1`、Part/source map、non-destructive high artifact、strict GLB/readback、support-loop/crease/normal policy | source-only durable slice；proposal `registered=false`、`FPS-HIGH-05=NOT_PASSED`，commercial quality `NOT_RUN` |
| `Retopology` / `LowMeshArtifact@1` | `RetopologyConstraintSet@1`、editable quad/edge-flow、Part/hard-edge/UV seam correspondence、High↔Low lineage | current Low 为 `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`，artist promotion `NOT_RUN` |
| `HeroUvLayout@1` | `HeroUvLayoutRequest@1`、UV0/UV1、visibility-weighted density、seam/stretch/overlap/OOB/padding/Mikk evidence | `hero_uv_durable_get/prepare` public chain 与 replay/drop-reopen/get 1/1 PASS；artist/packaged/engine/commercial `NOT_RUN/NOT_PROVEN` |
| `CageArtifact@1` / `HighLowBakeReceipt@1` | topology-correspondent cage、per-region offsets、ray hit/miss/fallback/cross-part/skew、dilation 与 map hashes | diagnostic/source target；当前 bake quality failed，正式 Gate `NOT_RUN` |
| `MaterialLayerGraph@1` / `HeroMaterialPack@1` | layer/mask/generator/anchor/filter/blend、channel/color-space/mip policy、texture CAS/provenance | fixed formula/embedded preview only，commercial surface `NOT_PROVEN` |
| `HeroLodSet@1` / `CollisionSet@1` / `SocketSet@1` | LOD identity/triangle and silhouette budgets、collision/socket readback、material/UV/tangent continuity | future/queued；`NOT_RUN` |
| `EngineValidationReceipt@1` | engine/version/build/harness hash、export hash、imported scene/material/LOD/socket/animation/collision projections、metrics | future；Unity/Unreal round-trip `NOT_RUN` |
| `HeroArtReviewReceipt@1` | independent reviewer scope/role、candidate/export hash、blind review rubric、IP/originality and issue evidence | future；独立资深武器艺术家审核 `NOT_RUN` |

### `ForgeCadModule@1` 最小闭合 manifest

每一个 Native High、Retopology、Hero UV、Cage-Bake、Surface 或 LOD Worker，以及 Engine/Review adapter，都必须由一个产品内建的 `ForgeCadModule@1` wrapper 描述。最小字段为：

```text
module_id, module_kind, schema_refs[], operator_refs[], budget,
fixture_refs[], license_text_sha256, notice_sha256, sbom_sha256,
provenance{source_revision, toolchain_hash, build_cohort_sha256},
signature{algorithm, key_id, signature_sha256}, module_sha256,
contract_set_sha256, input_sha256, output_sha256,
capabilities{network:false, dynamic_plugin:false, script:false,
             direct_db_write:false, direct_cas_write:false}
```

`budget` 至少声明 CPU/time、peak RSS、输入/输出 bytes、element/triangle/texture/CAS 上限；具体数值必须来自同 cohort deterministic benchmark，未有 receipt 只能是 `queued`。`fixture_refs` 必须同时覆盖正向、unknown、损坏/恶意、超预算、重放和跨平台 hash case，且不得含用户图片、绝对路径、prompt 或 secret。LICENSE/NOTICE、SPDX SBOM、source/build provenance、signature、contract/module/input/output hash 缺一项，Schema 只能标 `target`，不能进入 active registry、Runtime allowlist、Stage、confirm/version/export。

## 3. 通用字段

每个持久/跨进程对象必须有：

```text
schema_version
id
project_id (适用时)
created_at
canonical_sha256
parent_refs / lineage
```

永久写请求增加 `base_version_id`、`prepared_object_id`、`prepared_object_sha256`、`approval_receipt_id`、`idempotency_key`。所有 ID opaque 且不能含用户名/路径。

## 4. 规范化与 hash

- UTF-8、明确 key 排序、数值/单位 canonicalization、禁止 NaN/Infinity/负零歧义；
- 时间使用 UTC RFC3339，hash 使用 SHA-256；
- 二进制只保存 CAS hash/MIME/size，不内联；
- 缺失与 `null` 语义不同，Schema 必须明确；
- unknown property 默认拒绝；
- enum 扩展按版本处理，不能静默映射；
- 任何 renderer/worker/platform 影响结果的配置进入输入 hash。

## 5. 几何和单位

默认米、右手坐标系、Y-up（最终 GLB lowering 明确转换）。每个长度/角度/颜色/纹理字段声明单位、范围和精度。Geometry Operator 只允许命名 typed 参数，不接受 JSON pointer、代码、URL 或路径。

## 6. 版本策略

同一 `@1` 只允许向后兼容的 optional 增加，且 validator/consumer 已知默认；破坏性变化创建 `@2` 和显式迁移。Runtime/MCP/Viewer/Worker 协商 contract set digest；不兼容时写路径关闭。

## 7. 负向 Gate

每个 Schema 至少测试 unknown fields、超长字符串/数组、深嵌套、非有限数、错误单位/ID/hash、路径/URL/secret-like 字段、循环 DAG、预算溢出、stale base、重复 key 和版本不兼容。

<!-- forgecad-stage0: schemas=662 schema_set_sha256=202e080ec378ddb294eb9c880079dcec5c910b27a1c679034ca34c5a880dcec6 read_tools=131 write_tools=95 total_tools=226 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260815-b37-complete-auto-v3.json latest_completed=real-codex-cli-current-20260815-b37-complete-auto-v3.json -->
