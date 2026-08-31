# ForgeCAD MCP Runtime 合同

> 2026-08-30 Knife Brief boundary：`weaponry_knife_production_brief_prepare/get` 归属默认
> `reference_intake` façade。MCP 只消费 package-owned closed request Schema、执行 write opt-in/preflight
> 与中央领域路由；Runtime 独占 ReferenceEvidence/CAS 三元组验证、canonical hash、successor freeze、
> Store/CAS commit/replay/get。prepare 的 blocked intake 仍是成功持久化，但不是 authoring 授权；get
> 严格只读。Brief resolved engine 还需解析 version/unit/axis 选择，texture 需解析 authoring 与 shipping 双值；
> successor projection 允许 acceptance gate 状态随 resolution 更新，但 required gates/promotion policy 仍冻结。
> 当前默认面为 11 façade / 127 active operations，compatibility 仍 131/95/226。

> **Weaponry P0 override (2026-08-29):** 本文只有在与 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md` 和 ADR-0029 一致时才具有当前执行权。ForgeCAD 在本文中解释为 Weaponry 的 Rust Runtime lineage；当前唯一产品主线是由 Codex 生成、修改、验证并交付高质量穿越火线非功能性游戏武器。通用 3D、机器人和原创科幻示例仅作 fixture/历史能力，不得抢占本月主线。 本文中所有 2026-08-28 及更早的“当前”“下一原子”“唯一 `in_progress`”和工具/Schema 数量语句均按历史 cohort 解释，不得覆盖 `WPN-*` successor queue。

> 2026-08-30 current public boundary：默认 Codex 只看到 11 个 Knife façade；其 125 个 active operation 全部在 MCP 层消费 package-owned closed request Schema，blocked=0，Runtime fallback=0。历史兼容面仅由显式 `forgecad-mcp-compat` 暴露，保持 131 read + 95 write = 226 raw tools。

## Weaponry Authoring transaction contract

`authoring_mesh_transaction_get` 默认只读，`authoring_mesh_transaction_prepare` 只在显式 write opt-in 和 Ponytail preflight 后可见。MCP 仅验证 11/9 个精确 envelope 字段、1 MiB 响应上限和 closed journal，然后薄转发 Runtime；不生成 child ID、不拆分事务、不访问 SQLite/CAS。

Runtime 唯一派生 `transaction_sha256` 语义 journal hash、`transaction_object_sha256` CAS bytes hash、child revision IDs/hashes 与 result。首次 prepare 为 `prepared/committed`；exact replay 为 `replayed` 且 store/CAS/runtime write 全部 `not-touched/false`；get 为 `found`。`restart_hash_verified` 在普通返回中必须为 false，重启事实只由独立 drop/reopen 测试证明。移除 Blender task/capability 占位链后，当前 source surface 为 **131 read + 95 opt-in write = 226 tools**。

> 2026-08-29：默认只读 `production_weapon_form_art_target_occlusion_attribution_get` 已进入公共面；Runtime 对四候选 closed family、父候选、FormArt、固定相机、目标 mask 与 raster source table 逐项重验。04BE-M/O/Q/S 均证明 restart exact 与 SQLite/CAS 零写。公共面为 **131 read + 94 opt-in write = 225 tools**。

> 2026-08-28 当前 source manifest 仍为 **130 read + 94 opt-in write = 224 tools / 583 schemas**。`ProductionWeaponFormArtCompositeProposalPlan@1` 新增 4 个 `receiver-upper` closed profile；Runtime 强制 exact `primitive@2` box 参数、空 inputs、唯一 PartOutput 与非目标节点不变。用户授权只允许生成 reviewable candidates，不等于 confirm/Stage 权限；真实 L 四候选全部视觉拒绝。

> 2026-08-28 当前 source manifest 仍为 **130 read + 94 opt-in write = 224 tools / 583 schemas**。`ProductionWeaponFormArtCompositeProposalPlan@1` 的 registered profile 闭包现含 4 个真孔 `@1` 与 4 个相机映射真孔 `@2`。Runtime 除 exact Part/node/operator 外还校验完整 canonical parent parameters 与 `inputs=[]`，重新序列化规范化 profile 数值后重算 program hash；历史父候选跨 cohort 时校验其持久 CAS/readback lineage，不把当前 cohort recompile hash 冒充历史 GLB hash。J/K 视觉门失败，不授权任何晋级。

> 2026-08-28 当前 source manifest 仍为 **130 read + 94 opt-in write = 224 tools / 583 schemas**。`ProductionWeaponFormArtCompositeProposalPlan@1` 的 `registered_profile_id` 闭包新增 4 个 `side-panel-a` aperture trial 值；MCP 只转发封闭值，Runtime 校验 exact Part/node/operator/parent parameters、重算 program hash，并继续作为 candidate/Store/CAS 唯一写者。真实 04BE-I 的 4 个候选均被 CrossView/FormArt 门拒绝；它们不授权 FormQualityV2、Stage、confirm、version 或 export。

> 2026-08-28 当前 source manifest 为 **130 read + 94 opt-in write = 224 tools / 583 schemas**。`production_weapon_form_art_aperture_repair_plan_get` 是严格只读 Runtime derivation：MCP 只转发 closed IDs/hashes，Runtime 重放 visibility calibration/failure diagnostic，以 CAS 对象字节 hash 绑定 `GeometryProgram@2`，然后返回顺序双 Part 试验计划。工具本身不执行 repair、不写数据、不创建 FormQualityV2 或推进 Stage。

> 2026-08-28 当前 source manifest 为 **129 read + 94 opt-in write = 223 tools / 581 schemas**。`production_weapon_form_art_visibility_calibration_get` 是严格只读、hash-bound Runtime derivation；MCP 只转发 durable IDs/hashes，Runtime 独占 before/after GLB、CameraRig、ReferenceCanvas/AOV 回读与 Render Worker raster attribution。真实 D1 restart exact equality 和 SQLite/CAS zero-write 已 PASS；返回只授权后续 repair plan，固定禁止 geometry repair、confirm、Stage advance、version 与 export。

> 2026-08-28 当前 source manifest 为 **127 read + 94 opt-in write = 221 tools / 577 schemas**。`production_weapon_form_art_repair_plan_get` 是严格只读 Runtime derivation：MCP 只转发 closed IDs/hashes；Runtime 校验 composite evidence Store sidecar、CrossView 完整六视图/canonical/program lineage、proposal FormArt canonical 和 current GeometryProgram profile，再返回 registered next repair。它无 Store writer、无 Worker、无 candidate/Stage/confirm/version/export 副作用。

> 2026-08-28 composite evidence 公共面为默认只读 `production_weapon_form_art_composite_evidence_get` 与显式 opt-in 写入 `production_weapon_form_art_composite_evidence_prepare`。MCP 只携带 typed durable IDs/hashes；Runtime 作为唯一写者重读 final candidate/artifact/readback、同 cohort baseline、固定六视图 54 AOV、CrossView 与 proposal FormArt，并由 Store/CAS 事务持久化精确 sidecar。真实 prepare、same-key replay 与隔离重启 GET hash equality PASS；返回强制 `candidate_confirm_allowed=false`、`production_stage_advanced=false`、`QUALITY_TARGET_NOT_MET`，不得据此创建 FormQualityV2 或晋级。

> 2026-08-28 composite proposal 公共面为只读 `production_weapon_form_art_composite_proposal_get` 与显式 opt-in 写入 `production_weapon_form_art_composite_proposal_prepare`。MCP 只传 typed IDs/hashes/registered operations；Runtime 独占 original/current scope revalidation、Worker compile、strict readback、candidate/Store/CAS/receipt 写入与 restart hash 验证。真实 D1 prepare/get 已 PASS；返回强制 `candidate_confirm_allowed=false`、`production_stage_advanced=false`、`QUALITY_TARGET_NOT_MET`，54 AOV/FormArt 未运行前不得晋级。

> 2026-08-27 `production_weapon_owner_reviewed_void_calibration_get` 已作为默认只读工具进入公共面。它只接受 project/session/FormArt/baseline 的 durable identity/hash；Runtime 内部重读批准 CameraLock lineage、RigV2、Part-ID、depth 与 reviewed targets，固定解析 `rear-stock` 的 left/right/rear-three-quarter 校准。不接受 mask、transform、camera matrix、raw mesh、路径、URL 或脚本；不写 Store/CAS、不调 Worker、不改 Stage。当前 **568 schemas / 124 read + 92 opt-in write = 216 tools**。校准 eligible 只授权下一个 bounded authoring repair，不代表 strict owner-void、视觉或商业质量通过。

> 2026-08-27 `FPS-FORM-04AS`：`FormQualityV2` 新鲜基线适配器已在 Contracts、MCP、Runtime 与 Store 收口。它明确分离 source scope（当前 Stage head、CameraLock、same-cohort fresh baseline、registration lineage、RigV2）与 evaluation scope（distinct proposal candidate、proposal CrossView、proposal-side Part-ID/negative-space/line-flow）；所有调用字段均由 Runtime 从 durable evidence 重派生并由 Store 独立回读验证，legacy 模式不得夹带 proposal scope。538-schema checker、Contracts/Store/Runtime/MCP compile、四组件 same-cohort build identity 均 PASS，source cohort=`acf10c3b…173`。这只是 source/compile gate：当前真实 D1 `candidate-9127…fdc8b` 仍为 `REJECTED_REGRESSION`，未用新 adapter 重跑，Stage=`camera-calibrated`、secondary=`NOT_CREATED`、quality=`QUALITY_TARGET_NOT_MET`，无 confirm/version/export。下一原子是基于批准相机设计新的 bounded `rear-stock` art-shape，只有 proposal evidence=`READY` 且 fresh FormQualityV2 真实运行通过后才允许推进 Stage。证据：`docs/evidence/mcp010f/production-weapon-form-quality-v2-fresh-baseline-adapter-source-gate-04as-20260827.json`。

> 2026-08-27 `FPS-FORM-04AR`：fresh FormArt proposal receipt 的 camera provenance 现在是严格互斥 union。`fresh-same-cohort-baseline-rig-v2` 必须绑定 baseline parent receipt 和六个 baseline view receipts，历史 FormEvidence view receipt 字段必须为 null；`historical-form-art-camera` 反之。Store reachability 与回读按该 union 选择根对象，并重验 baseline/project/session/source candidate/artifact/lineage/RigV2/view receipt，禁止把 fresh 与 historical camera receipts 混合。真实 D1 620×680 prepare 已通过该 durable transport，但视觉门拒绝。

> 2026-08-27 `FPS-FORM-04AL` 当前增量：Runtime-owned durable fresh six-view baseline producer 已接通合同、Store、Runtime 与 MCP `prepare/get`；每个视图绑定 approved registration lineage / RigV2、fresh same-cohort 512×512 九 AOV、camera/mask/compare/quality 与完整 CAS reachability，并以单事务持久化。精确状态为 `PASS_SOURCE_COMPILE_DURABLE_PRODUCER_NOT_RUN_REAL_D1`；真实 D1、orientation approval、fresh baseline、notch、secondary、Stage/confirm/version/export 均未执行。当前公共面 **538 schemas / 118 read + 88 opt-in write = 206 tools**，视觉仍 `QUALITY_TARGET_NOT_MET`。

> 2026-08-27 `04AK`：新增默认只读 `production_weapon_form_art_baseline_preflight_get`（`Request@1/Result@1`）。Runtime 独占验证 registration lineage、RigV2、scope 和固定六视图；请求不接受 camera/hash/RenderSet/AOV/path/content。结果当前固定 fail closed 为 `FRESH_BASELINE_MATERIALIZER_UNAVAILABLE`，不写 Store/CAS、不启动 Worker。工具面现为 **117 read + 87 opt-in write = 204 tools**。

> 2026-08-27 `04AJ`：`production_weapon_form_art_mesh_proposal_get` 可对历史 durable FormArt 做只读 proposal，但 Runtime 必须逐项验证 FormArt row、CAS receipt、FormEvidence view、RenderSet canonical/profile/binding，并返回 recorded worker cohorts。若与当前 Runtime cohort 不一致，结果必须含 `prepare_eligible_by_form_art_cohort=false / BASELINE_FORM_ART_COHORT_REFRESH_REQUIRED`；对应 write prepare 必须在 child revision、candidate 或 render 写入前拒绝。不得通过复用旧 cohort 字符串伪造当前 provenance。

> 同轮 rollback 修正：CameraLock lineage 的 Store transaction 失败路径必须连同 semantic/orientation/RigV2/lineage receipt 一并释放本次新建的 ApprovalReceipt CAS。另因 notch request 当前不直接携带 `registration_lineage_id`，编排层不得把“先创建 lineage”当作消费证明；必须先生成并验证绑定该 lineage 的 fresh same-cohort FormArt，再允许 notch prepare。

> 2026-08-26 `04AI`：新增默认只读 `production_camera_lock_registration_lineage_preflight_projection_get`（`Request@1/Result@1`）。闭合请求只接受 parent scope、board rotation 与 semantic screen order；Runtime 独占派生 orbit、registered camera hash/canonical、投影/upright proof 和 projection input hash，并完整验证已有 lineage。它不写 Store/CAS、不启动 Worker、不生成 ApprovalReceipt，`projection_ready_for_user_review` 不等于 `ready_for_promotable_lineage`。该轮为 116+87；当前为 **117 read + 87 opt-in write = 204 tools**。

> 2026-08-26 `04AH`：`production_camera_lock_registration_lineage_prepare` 仍是同一 opt-in write tool，但 closed input 新增 rear3q semantic screen order 与 canonical `0/180°` camera orbit。Runtime 不接受 camera matrix；它从现有 registered camera 派生 Y-orbit 变体，投影 candidate-owned stock/muzzle source anchors并验证 world `+Y` upright。board rotation 仍是独立字段，diagnostic `rotate-180` 不得冒充用户审批；promotable 结果必须由 Runtime 创建 CAS-backed `ApprovalReceipt@1`，orientation provenance、Store 与 restart readback 均绑定其 exact object hash。

> 2026-08-26 proposal 合同更新：`production_weapon_form_art_mesh_proposal_get/prepare` 工具数不变，其 `edit` 现为 `MoveVertices@1 | OpenFrameNotch@1` 闭合 union。OpenFrameNotch 只接受 source-node/Part 绑定和 1..999 milli 的 width/height，拒绝 caller mesh/raw topology/脚本/路径；Runtime 仍是 durable revision、CAS/SQLite 和 candidate 的唯一写者。当前为 529 schemas / 202 tools。

> 2026-08-26 当前公共面：**528 schemas / 115 read + 87 opt-in write = 202 tools**。`production_weapon_form_art_mesh_proposal_get/prepare` 只接受 exact candidate/AuthoringMesh/FormArt hashes 与 bounded stable vertex moves；prepare 由 Runtime 完成 child revision、单 source-node lowering、Worker strict readback、六视图与 durable proposal FormArt receipt。Pareto review 不能 confirm、version、export 或推进 Stage；真实 D1 因 owner/open-void、negative-space、line-flow 未全通过而返回 `BLOCKED_PROPOSAL_FORM_ART_EVIDENCE`。

> `FPS-FORM-04AG` 合同边界：prepare response 只携带 `ProductionWeaponFormArtProposalEvidence@1` 引用/projection，authority 是 Runtime 写入的 CAS receipt + SQLite Store identity。真实 D1 已通过 CAS canonical readback、Store index/reachability、same-key replay 与 Runtime drop/reopen replay/readback，公共工具家族未扩展。六视图 Part-ID 已 observed；owner/open-void、negative-space、line-flow 仍有失败或 unknown/inferred，所以状态是 `BLOCKED_PROPOSAL_FORM_ART_EVIDENCE`，不得触发 secondary、Stage、confirm、version 或 export。

> 2026-08-26 当前公共面：**114 read + 86 opt-in write = 200 tools**。新增 `production_weapon_authoring_mesh_v2_source_prepare` 只接收 exact durable hashes/IDs，由 Runtime 读取 candidate GeometryProgram 与 ArtifactReadback、验证唯一 Part/source ownership、派生 stable mesh lineage 和 source-bound V2 genesis；caller 不得提交 topology/program/source binding。prepare 写 AuthoringMesh revision，不推进 Stage、confirm、version 或 export。

> 2026-08-26 `04AF` 公共面：**114 read + 85 opt-in write = 199 tools**。`authoring_mesh_v2_durable_get/prepare` 仅接受闭合 typed revision/edit 请求，Runtime 生成稳定 identity、journal/tombstone、CAS 与父子 revision；`production_camera_lock_registration_lineage_preflight_get` 只读报告父系和用户回执 blocker。真实 D1 action proposal 已走六视图合同但因回退被拒绝，这不得触发 confirm/version/export。

> 商业生产 MCP 不得接收任意几何脚本或把自动 retopo/UV/material 直接 promotion；所有写入仍由 Runtime 对 approved upstream artifact 执行 prepare/readback/approval/confirm。目标合同与 Engine sidecars 见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

> 2026-08-26 `FPS-FORM-04AD` 权威增量：当前合同面为 **518 schemas / 111 read + 83 opt-in write = 194 tools**。新增 `ProductionWeaponSemanticLandmarkOrdering@1` 只表达 Runtime-derived 的 3D source/subject-axis 顺序，明确 `target_landmark_arrays_present=false / metrics=NOT_PRESENT`，不得冒充 2D landmark；`ProductionWeaponAuthoredViewOrientation@1` 将诊断变换与用户方向回执分开；`RegisteredCameraRigCalibration@2` 只有绑定 promotable authored rear3q receipt 才能物化。定向 Contracts/Runtime/MCP compile 与 518-schema checker PASS。真实 D1 尚无 orientation-specific user receipt，因此保持 `BLOCKED_AUTHORED_REAR_THREE_QUARTER_ORIENTATION`、Stage=`camera-calibrated`、secondary=`NOT_CREATED`、quality=`QUALITY_TARGET_NOT_MET`，不 confirm/version/export。旧 `@1` 保持历史真值；durable 落点采用 CameraLock 的 additive child lineage，不复制/自动升级整张旧记录。

> 2026-08-26 商业合同补充：MCP 最终暴露的是三个组合读模型/prepare 系列——`HeroSourceAsset@1`、`FpsPresentationPackage@1`、`EngineDeliveryPackage@1`——而不是把 GLB 当作全部真值。所有 High/Low/Cage/Bake/Material/LOD/animation/engine sidecar 由 Runtime 绑定同一 lineage；MCP 不接收 caller 自报 hash、不执行第三方脚本、不把自动 draft 晋升，并将大 mesh/texture/heatmap 保存在 CAS 只返回引用。

2026-08-26 FormArt attribution 合同同步：现有 `ProductionWeaponFormArtEvidenceGetRequest@1/GetResult@1` 的可选闭合 raster diagnostic 已在 fresh durable D1 上通过 registered-camera exact replay；MCP 只读路由严格验证 zero-write、scope、hash/canonical、candidate/artifact/readback、ReferenceCanvas/DesignSpec/target、CameraLock/rig、FormEvidence view、RenderSet 与 masks，拒绝 caller camera/mask。04AB 输出 expected Part=`rear-stock`、highest source=`rear-stock/548px`、semantic match=true、repair target unique。商业晋级不得停在该 transient projection：新增 CameraLock additive child lineage，持久绑定 GeometryProgram、SubjectFrameRegistration、source semantic ordering、RegisteredCameraRig 与 authored reference orientation；旧 @1 不自动迁移、不允许进入商业 Repair/High。

2026-08-26 04AE source 实现口径：`production_camera_lock_registration_lineage_get` 是默认只读工具，`..._prepare` 仅在 MCP 显式 write opt-in 时可见。Prepare 请求只能提供父 CameraLock 期望 hash、三个输出 ID、rear3q rotation 与独立审批；Runtime 从父 lock/GeometryProgram/ReferenceCanvas/CAS 重放并派生 ordering、crop hashes、exact matrix 与 RigV2。阻断朝向不落成功 row，旧 CameraLock、Stage、candidate/version/export 不变。

2026-08-26 合同同步：Formal High 的 4 个 closed wrapper schemas、MCP `get/prepare`、Runtime adapter/IPC 与 Store scoped idempotency 已接通。prepare 只允许 source Stage transition identity/hashes、distinct High candidate ID、idempotency key、bounded response 与 caller-input hash，禁止 caller 提供 Runtime 输出 hash；`get` 严格只读，不修复 CAS、不推进 Stage、不 confirm/version/export。完整 positive/restart/cleanup 仍未证明。

2026-08-26 Formal High 输入边界补充：public prepare 不得接收 candidate-state、formal-readback、receipt 等 Runtime 输出哈希。当前 internal materializer 已采用 source Stage head + distinct High candidate identity + request hash 的无环输入，所有输出真值由 Runtime 派生；公共工具已 source-exposed，但 positive capability仍 `NOT_PROVEN`。

2026-08-26 最新合同实现说明：Formal High 的 4 个 wrapper Schema 与 public MCP `get/prepare` 已落地，当前为 **518 schemas / 111 read + 83 opt-in write = 194 tools**。prepare 只接受 source Stage transition 三个绑定、distinct High candidate、idempotency/response/writer policy 和 input hash；Runtime 派生 scope、candidate state、artifact/readback/receipt。Store 持久化 scoped idempotency 并 fail-closed 冲突。完整合法 secondary source 下的 positive restart/cleanup 尚未运行，因此该 public surface 仍只记 source/structural，不得从 monolithic High/Low/Bake prepare 产生部分成功持久化。

2026-08-26 Cage/Bake public seam 增量：Cage/Bake 与 Formal High public tools 均已 source-exposed。当前未通过的是完整 source-lineage/CAS 正向 restart/cleanup fixture；MCP 不得补造、推断或内联该真值。真实 D1 因 Form 前置未通过且无 formal positive receipt，仍 fail closed、零写，不推进 Stage、confirm、version 或 export。


`low_quad_draft_durable_get` 默认只读；`low_quad_draft_durable_prepare` 仅在 authenticated explicit write opt-in 后暴露。两者都必须先满足 Ponytail preflight，并由 MCP 原样路由到同名 Runtime 方法；MCP 不读写 SQLite/CAS。当前 Low 输入必须保持 candidate-bound exact provenance，prepare 只物化 source-only Low draft，不要求 confirm，也不能推进 Stage、创建版本或导出。Hero UV 的 `hero_uv_durable_get`（只读）与 `hero_uv_durable_prepare`（authenticated explicit write opt-in）现已完成 Store→Runtime→MCP 公共链路；该链路仍仅为 structural/source evidence。

> 2026-08-26 最新权威 source 口径（取代下方 2026-08-25 的“最新/当前”计数）：**518 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 MCP tools**。Low quad draft 仍为 candidate-bound exact provenance 的 `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false` 输入；Hero UV 的 `hero_uv_durable_get/prepare` 已完成 Store→Runtime→MCP，真实 prepare→同键重放→Runtime drop/reopen→get 为 **1/1 PASS**，四个 Hero CAS roots 已 linked 并纳入 reachability/GC。该结果仅为 structural/source pass，不是 artist-authored unwrap、visual、human、engine、commercial 或 packaged pass；`FPS-HIGH-05=NOT_PASSED`、Stage=`camera-calibrated`、visual=`QUALITY_TARGET_NOT_MET`、human/engine/distribution=`NOT_RUN`、commercial=`NOT_PROVEN`、packaged acceptance=`NOT_RUN`、HQ360=`BLOCKED_REFERENCE_COVERAGE`，不推进 Stage、confirm、version 或 export。证据：`docs/evidence/mcp010f/commercial-weapon-hero-uv-durable-restart-source-gate-20260826.json`。

> 2026-08-25 历史快照（已由上方 2026-08-26 权威口径取代）—口径：已有 7 个 Native High durable/GLB closed contracts与 2 个 Low quad draft Worker contracts，当前总数 **499 schemas**；公共 `native_high_durable_get/prepare` 使工具面保持 **107 read + 79 opt-in write = 186**，当前 cohort Runtime restart **1/1 PASS**。Low quad draft与 Hero UV仍未接 Runtime durable/MCP；这些不是 packaged/candidate-quality 能力，也不推进 Stage/confirm/version/export。

2026-08-25 `CQ-02-TYPED-TOPOLOGY-IDENTITY-LINEAGE`：`authoring_mesh_edit_preview → authoring_mesh_edit_prepare` 的 `split_edge / collapse_edge / dissolve_edge` proof 仍保持 source-element-only；下游 Runtime 现在只从 Store 的 exact candidate→idempotency response 恢复该 proof，并把 parent source identity 物化为 durable `AuthoringMeshIdentityLineage@1` child IDs、单调 tombstone 及 one-to-many/many-to-one relation，不接受 caller identity/proof arrays。真实 split/collapse/dissolve 已分别完成各自独立的完整持久化与 Runtime drop/reopen/get 重启链路，合计 **3/3 PASS**；Store `authoring_mesh_` **12/12**、MCP IdentityLineage **3/3**、490-schema checker与 Contracts/Store/Runtime/MCP 联合 compile PASS，工具数仍 **106 read + 78 write = 184**。general correspondence、evaluated retarget、完整 selection/undo history 与产品级 cross-version editor仍 `NOT_PROVEN`。Stage 保持 `camera-calibrated`，视觉=`QUALITY_TARGET_NOT_MET`，human/engine/distribution=`NOT_RUN`，HQ360=`BLOCKED_REFERENCE_COVERAGE`。新回执：`docs/evidence/mcp010f/authoring-mesh-typed-topology-identity-lineage-materialization-source-gate-20260825.json`；原 source-proof 回执继续作为上游证据。

2026-08-25 Native High Runtime contract 口径：fixed Worker/GLB sibling 已由 Runtime producer 消费，并完成 CAS/Store/replay/restart/get 与公共 MCP source-focused Gate。它仍不是 packaged/candidate-quality 或 Stage transition receipt，`FPS-HIGH-05=NOT_PASSED`，总体质量状态不变。

> 2026-08-25 商业质量合同边界：High/Low、Hero UV、Cage/Bake、Material Layer、Engine 与 Human 仍是目标能力。MCP 已同步 direct `authoring-mesh@1 + bevel@2` stack/evaluation schema，旧 Apply@1 仍闭合拒绝 bevel@2；这只证明精确 Runtime forwarding 的 read-only lowering。IdentityLineage split/collapse/dissolve 独立 full-chain **3/3 PASS**；general correspondence和完整编辑历史仍 `NOT_PROVEN`。

2026-08-22 `CandidateMaterialSurfaceQuality@1` public positive fixture：`Geometry → CandidateTopologyQuality@1 → AppearanceProgram@3 → TextureBuild@2 → SurfaceBake@1 → AppearanceSourceLineage@1 → CandidateMaterialSurfaceQuality@1` 的 `prepare → same-key replay → get → Runtime drop/reopen → restart get` 通过 **1/1（111.72s）**；Runtime focused **5/5**、Store full **74/74**、Contracts **350**。CAS inventory unchanged；stable `artifact_id` 与 GLB object SHA-256、MaterialPack CAS kind 精确区分，合法 UV/tangent rebuild 不计入 geometry-preservation 漂移。该结果仅为 `structural_only`；V2 animated-socket-particles 仍无完整 public `prepare → Store → restart get`，durable end-to-end=`NOT_RUN`/`BLOCKED_FIXTURE_CHAIN`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/candidate-material-surface-quality-public-positive-source-gate-20260822.json`。

最终同 cohort 修订口径：强制 build cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` 下 material focused **5/5（112.63s）**；Runtime full **310 passed / 0 failed / 20 ignored**（330 total，201.91s），且 public material fixture 明确在该 full run 内执行。此前 **111.72s** 仅为 public fixture 单测时长；两者都只支持 `structural_only`，不提升 visual/commercial、human/engine 或 stage/confirm/version/export 状态。

数值口径：当前 source 为 **518 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 tools**；`hero_uv_durable_get/prepare` 已进入公共 MCP surface，本文其余较小数值均只作 historical prior slice 保留。

2026-08-25 当前合同面新增 closed `AuthoringMeshRequest@1 → AuthoringMesh@1` 与默认只读 `authoring_mesh_get`。Runtime 从 durable candidate/program/artifact/readback 重放唯一 direct authoring Part，构建 V/E/H/C/F/loop/ring half-edge projection并 fail-closed 验证 cycle/twin/manifold/orientation；original/evaluated lineage 明确非双射。MCP 仅转发 closed request 与 ≤1 MiB structured response，不写 SQLite/CAS/Stage，不宣称 durable mesh、跨版本 stable ID、Viewer 或视觉质量。

2026-08-22 `FictionalEnergyVfxAnimatedSocketParticlesSequence@2` 双候选 source slice：Contracts **350**；Store V2 focused **2/2**、Store full **74/74**；Runtime V2 仅低层 focused **6/6**、cargo check **PASS**；MCP V2 **3/3**；同 cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` Runtime full **309 passed / 0 failed / 20 ignored**（191.06s）、MCP full **128 passed / 0 failed / 0 ignored**（1.93s），这些是全量回归，不是 V2 public `prepare → Store → restart get` 正向 fixture。V1/V2 隔离；V2 仅证明 1..16 frame、geometry/appearance 双 candidate/delivery/AnchorSet bridge 以及 Store FK/reachability/idempotence/conflict/rollback 的结构面。完整双候选 public Runtime `prepare → Store → restart get` 正向 fixture 尚不存在，durable end-to-end=`NOT_RUN` / `BLOCKED_FIXTURE_CHAIN`，不能声称正向 durable。该 slice 为 `structural_only`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/fictional-energy-vfx-animated-socket-particles-v2-dual-candidate-source-gate-20260822.json`。

2026-08-20 `GeometryProgram@2`、Agentic action/critic/repair、`GeometryModifierStackRequest@1` 与 fictional energy rifle profile 已 exact 支持 `energy-core@1`；MCP/Runtime 另对 outer/inner radius、depth、position、rotation 提供闭合 typed patch 与关系校验。MCP 只做 closed schema/canonical hash/有界补丁校验；实际拓扑、预算和 mesh 执行属于固定 Worker/Runtime。没有新增 MCP tool或任意代码入口。

2026-08-19 当前合同面新增 closed `AuthoringMeshEditPrepareRequest@1 → AuthoringMeshEditPrepare@1` 与显式 write `authoring_mesh_edit_prepare`。MCP 只做 closed schema、write opt-in/cohort 和 1 MiB wire gate；Runtime 重放 nested preview、验证 expected preview/current head、物化 exact derived artifacts；Store 原子提交 reviewable candidate、Job/event/audit/evidence/idempotency。输出明确 `approval-required`、`no-version-created` 与 `locked-until-confirm`，拒绝脚本、路径、URL、选择历史和任意插件状态。

2026-08-19 当前合同面新增 closed `AuthoringTopologyRequest@1 → AuthoringTopology@1`、`AuthoringMeshEditPreviewRequest@1 → AuthoringMeshEditPreview@1` 与默认只读 `authoring_topology_get`/`authoring_mesh_edit_preview`。MCP 仅接收公开 geometry-prepare cohort、node/Part、policy、budget 和 preview edit；Runtime 从 durable evidence 派生并验证隐藏的 GeometryProgram object/evidence canonical，读取 CAS 必须有界。Topology 只覆盖一个 direct `authoring-mesh@1` source Part；preview 只允许 sorted translate 或 boundary triangle/planar-convex-quad extrude，执行 transient derived program 双 Worker replay/strict readback，完整响应 ≤1 MiB且不持久化。MCP/Runtime 禁止 Python/path/URL/env/plugin/network，不声明 BMesh parity。

2026-08-19 当前合同面新增 closed `RenderEvidenceReplayRequest@1 → RenderEvidenceReplay@1` 与默认只读 `render_evidence_replay_get`。outer canonical hash 绑定 candidate-state 和完整 nested Integrity request；Runtime 独占 strict GLB readback、actual fixed Worker same-cohort/profile 双重放与九 AOV raw PNG/decoded RGBA8 exact 验证。Result 仅含 hash/size/status，固定 artifact-embedded-materials-only、in-memory-only、no-write 与 1 MiB；MCP 不接触 DB/CAS，不允许 engine/profile/camera override、bytes、path、URL 或 script。

2026-08-19 当前合同面新增 closed `MechanicalPoseGeometryPreviewRequest@1 → MechanicalPoseGeometryPreview@1` 与默认只读 `mechanical_pose_geometry_preview`。outer request hash 绑定完整单 tick pose request 与固定 preview policy；Runtime 从 durable evidence/CAS 重验 baseline GeometryProgram/GLB，按每 Part `posed-world × inverse(rest-world)` 追加 transient `transform@2` sinks、重新 Worker hash/compile/strict readback。result 含派生 program、per-Part delta、transient artifact hash/readback与明确 no-write/structural-only限制；完整 MCP response ≤1 MiB。

2026-08-19 当前合同面新增 `SubdivisionArtifactLineageSidecarRequest@1`、`Sidecar@1`、`Link@1`。同一 closed request 供显式 write `subdivision_artifact_lineage_prepare` 与默认 read `subdivision_artifact_lineage_sidecar_get` 使用；MCP 不接触数据库/CAS。Runtime prepare 先完成既有 exact replay，再写 canonical sidecar与独立 link；getter 仅重验 durable bindings/CAS bytes。唯一 candidate/node link 用 request hash实现精确幂等，冲突 fail closed；完整 Link MCP response ≤1 MiB。

2026-08-19 historical 合同 slice 新增 closed `SubdivisionArtifactLineageRequest@1 → SubdivisionArtifactLineageProjection@1` 和默认只读 `subdivision_artifact_lineage_get`。请求绑定 project/candidate/artifact/ArtifactReadback/node/25k/canonical；Runtime 从 durable evidence 解析其余 program/catalog/config/CAS hash，严格重验 V2 readback并让 fixed Worker 重编译 exact persisted program。只有完整 GLB bytes 与 candidate artifact 完全相同且 node 是唯一 direct source primitive，才输出 evaluated quad → source-local triangle ranges。完整 response ≤1 MiB、read path 零写入；结果明确不是 persisted sidecar、无 glTF V/E/C identity、无跨版本或视觉结论。该 slice 当时为 170 schemas、20/20 operators、47+33=80。

2026-08-19 historical Subdivision root-lineage source slice（当时 168/79 cohort）：新增 closed `SubdivisionTopologyLineageRequest@1 → SubdivisionTopologyLineage@1` 和默认只读 `subdivision_topology_lineage_preview`。请求只接受 exact canonical `GeometryProgram@2`、`subd-cage@2` node 与 1..25,000 element budget；Runtime/Worker 双层校验并在整个 MCP wire 1 MiB 前 fail closed。结果固定 `lineage_kind=control-root-to-evaluated-quad-topology@1`、`id_scope=program-and-evaluation-bound`、`cross_version_stable=false`、`artifact_binding_status=unavailable-preview-only`、`runtime_write_performed=false`；不接受脚本、路径、URL、bytes 或网络。当时为 168 schemas、20/20 operators、46+33 tools；现行口径见本文顶部。

2026-08-19 historical crease-aware contract slice 新增 closed `SubdivisionCreaseEvaluationRequest@1 → Result@1` 和 `subd-cage@2`。请求只允许 3..16 规则开放 quad grid、1..2 levels、1..128 条 interior adjacent edge、整数 sharpness 1..2；Runtime 在 input hash 前按 endpoint 归一化顺序，结果 validator 重新绑定原请求、program、Part、MaterialZone、policy、budget 与 catalog。`geometry_program_hash` 只生成 canonical `GeometryProgram@2`，不编译或写状态；实际执行继续走既有 `geometry_prepare → ArtifactReadback@2`。该 slice 当时为 166 schemas、20/20 operators、45+33 tools；旧 catalog hash 的 draft 按 cohort policy fail closed，已存 immutable artifact 不被重写。

2026-08-19 historical Boolean Operand Lineage 合同 slice 当时为 164 schemas、45 read + 33 opt-in write = 78 tools。`boolean_operand_lineage_preview` 接受 closed canonical `BooleanOperandLineageRequest@1`，其中 exact `GeometryProgram@2`、Boolean node ID 与 `max_lineage_runs=1..4096` 由 Runtime/Worker 双层验证；Runtime 从请求 program 独立重算 operation、左右输入与递归 source lineage，返回完整 MCP wire 至多 1 MiB 的 canonical `BooleanOperandLineage@1`。结果只含连续 operand/evaluated-face runs，不接受 bytes/path/URL/script，不写 SQLite/CAS/candidate/version。receipt：`docs/evidence/mcp010f/blender-boolean-operand-lineage-source-gate-20260819.json`。

2026-08-19 historical Render Evidence Integrity 合同 slice 当时为 162 schemas、44 read + 33 opt-in write = 77 tools。`render_evidence_integrity_get` 接受 closed `RenderEvidenceIntegrityRequest@1`，其 canonical request 必须携带 exact project/candidate/artifact/ArtifactReadback/program/reference/camera/RenderSet/comparison/quality hashes；Runtime 重新读取并验证所有对象与九 AOV/mask bytes，返回至多 1 MiB 的 canonical `RenderEvidenceIntegrity@1`。调用不得携带 bytes/base64/path/URL/script，且固定 `runtime_write_performed=false`。

2026-08-19 historical Mechanical Pose Sequence Preview 合同 slice 当时为 160 schemas、43 read + 33 opt-in write = 76 tools。`mechanical_pose_evaluate` 接受 closed `MechanicalPoseSequencePreviewRequest@1`，Runtime 对最多 16 个严格递增 tick 逐 sample 复用单 tick 的 candidate/artifact/readback/program/catalog/config 与 rest/action semantic validation，返回 ordered `MechanicalPoseSequencePreview@1`。sequence identity 包含完整 readback/program/catalog/config/input lineage，standalone validator 仍必须对原始 closed request 做 exact binding。MCP 文本块只返回 bounded summary，完整值只在 `structuredContent`，并对整个序列化 `tools/call` response 强制 1 MiB wire Gate。调用不写 SQLite/CAS/candidate/version，不接受 timeline、Armature、skin、脚本、路径、URL、环境变量或网络。

2026-08-18 historical Parametric Group v2 contract slice 当时为 158 schemas。`geometry_program_hash` 接受 closed `ParametricDesignKitRequest@2`，Runtime 只能从编译内置 catalog 选择三个 immutable template、校验 exact typed sockets，并 lowering 到现有 Geometry Worker 可验证的 `GeometryProgram@2`。

2026-08-18 historical Mechanical Pose source slice：新增默认只读 `mechanical_pose_evaluate`，该 slice 当时为 160 schemas、19 active operators、43 read + 33 opt-in write = 76 tools。MCP 使用 closed inline transport Schema，Runtime 再按公开合同验证 candidate/artifact/readback/program/catalog/config、完整 Part/source-node coverage、无环 parent map、关节/limit/action 与 input hash；结果仅为 canonical local/world TRS read projection，不调用 Worker、不 materialize 几何、不写 candidate/CAS/SQLite/version。

2026-08-18 historical RenderProfile slice：`RenderSet@2` 强制绑定 closed `RenderProfile@1` 及其 canonical/AOV/color-pipeline/ID-palette hashes；Render Worker 必须返回与 Runtime 共享常量完全相同的 profile，否则 fail closed。固定 `forgecad-fixed-software@2` CPU raster profile 只让 beauty 执行 linear-to-sRGB，其余八个 AOV 都是 non-color data。该 slice 当时为 152 schemas、19 active operators、42 read + 33 opt-in write = 75 tools；这不是 Blender/Cycles/EEVEE/OCIO/GPU/EXR parity，也不改变视觉 Gate。

2026-08-18 historical Modifier evaluation v2：默认只读 `geometry_program_hash` 的第四个 closed oneOf 分支为 `GeometryModifierEvaluationRequest@2`，Runtime 返回 `GeometryModifierEvaluationResult@2` 与 canonical `GeometryModifierEvaluationSignature@1`。合同明确区分完整 authoring stack、有效 evaluation、GeometryProgram output、evaluation policy、OperatorCatalog cohort 与 cache key；previous signature 仅是 caller-round-tripped comparison input，并对 project/representation/Part/material/solid fail closed。`reuse_kind=semantic-signature-only`；cache key 绑定 `output_kind=geometry-program-canonical-sha256`，不是持久 mesh cache hit。source base operator/parameters 由 11 个 paired branches 绑定。本路径不 compile、不调用写入 API；focused test 比较 candidate/version/CAS inventory，但未直接比较 SQLite 全表或 Job inventory，也不替代后续 `geometry_prepare → ArtifactReadback@2 → quality → approval`。该 slice 当时为 149 schemas、19 active operators、42 read + 33 opt-in write = 75 tools。

2026-08-18 historical Bevel/Normal slice：`GeometryProgram@2` 与 `GeometryModifierStackRequest@1` 新增 closed `bevel@1` / `normal-policy@1`。Runtime 只允许 Bevel 作为 direct solid `primitive@2` box 的首个 enabled modifier；Worker 从 source operator provenance 生成 bounded rounded box，不从 evaluated triangle diagonal 猜 semantic edge。Normal Policy 固定 corner domain、face-area × corner-angle weighting、`keep_sharp=true` 与显式 crease threshold。两者仍经现有 `geometry_prepare → strict readback → quality → approval → confirm`，Modifier hash/lowering 本身不写 candidate/CAS/SQLite/version。该 slice 当时为 146 schemas、19 active operators、42 read + 33 write = 75 tools；任意 mesh Bevel、visual/package/live 仍未通过。

2026-08-18 TopologySnapshot historical slice：新增 closed `TopologySnapshotRequest@1` / `TopologySnapshot@1` 与默认只读 `topology_snapshot_get`。Runtime 重新验证 project/candidate/artifact 与 hard-gate-passed `ArtifactReadback@2`，并绑定 readback/program/catalog/config/policy hash后，才从同一 GLB CAS bytes 投影一个 Part 的完整 evaluated triangle V/E/F/C。上限为 512 faces、1536 vertices/edges/corners、1 MiB canonical snapshot，超限 fail closed；MCP 文本块只返回 bounded summary、完整值只放 `structuredContent`，并再次对完整序列化 `tools/call` response 执行 1 MiB wire Gate。输出保留 face winding、corner normal/UV/tangent 与 edge direction，但 ID 仅 artifact-bound，source lineage 仅 partial operator-node。MCP 不重算、不持久化、不创建 candidate/version。该 slice 当时为 146 schemas、17 active operators、42 read + 33 opt-in write = 75 tools；当前 catalog 真值以上一段 19/19 为准。

2026-08-18 Modifier slice historical cohort：只读 `geometry_program_hash` 新增第三个 closed oneOf 分支 `GeometryModifierStackRequest@1`。Runtime 只将有序、可启停的 transform/mirror/array lowering 到现有 GeometryProgram@2 并返回 `GeometryModifierStackProgram@1`；MCP 不重算 hash、不持久化、不创建 candidate，也不允许 Boolean/任意脚本进入 v1 modifier 分支。该 slice 当时为 144 schemas、17 active source operators、41 read + 33 opt-in write；当前机器真值以本文顶部的 191/21/54/36/90 为准。

版本：2026-08-10
2026-08-17 Reference Visual Structure：`reference_mask_prepare`/`reference_mask_refine_prepare` 的可选 `visual_structure` 只接受归一化 visual region/line-flow draft；Runtime 强制 `visual-geometry-not-functional`、global contour authority、overlap/shared boundary、review status 与 canonical hash，并把它嵌入不可变 `SilhouetteTarget@1` CAS 对象。它不执行自动语义分类、不运行任意 Python、不生成候选，也不把区域边界变成最终模型接缝。
状态：MCP005–MCP009 MVP functional core 和真实 Codex CLI host golden path 已完成；MCP010B structural source Gate PASS 但 Darwin OS memory hard cap deferred/NOT_RUN；当前源码总 manifest 为 515 Schema、28 operator entries、111 read + 83 opt-in write = 194 个工具；MCP010C fixed renderer/九 AOV/reference comparison/review raw Gate PASS，MCP010D/E offline Operator/AssetPack/UV/PBR/MikkTSpace raw Gate PASS；MCP010F Viewer/source structural slices 与 Hero UV durable source slice 均不等于 artist unwrap、视觉、人评、引擎、商业或 packaged pass。Agentic observe/plan projection 与 durable session/checkpoint/RepairIntent prepare/readback 也已通过隔离 source/transport/restart receipt；通用单动作 orchestrator、Repair 应用、packaged/live C/D/E/F、真实用户 likeness、Viewer/PBR likeness/360仍 NOT_RUN/BLOCKED；唯一 `in_progress` 仍为 `FGC-MCP010F`。

2026-08-15 Agentic observation binding：`scene_observe_get` 生成的完整 `AgenticSceneObserveResult@1` 在 Runtime 进程内按 canonical hash bounded cache 保持；bound plan/critic/evidence/action 先读取同一 projection，cache miss 才允许重建并校验 project/candidate scope 与 canonical hash。该 cache 不进入 SQLite/CAS，不改变 projection read-only 或 durable session/checkpoint 的边界；focused tests 通过，真实视觉质量仍 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark `BLOCKED_INCOMPLETE_BINDING`。
P0 required 客户端：Codex Desktop、Codex CLI
未来兼容客户端：Codex IDE / VS Code / Cursor / Windsurf；其他 MCP Client

## 1. 合同目标

`forgecad-mcp` 将模型无关的 ForgeCAD Runtime 暴露给 Codex。它不包含 Agent、聊天、图片理解、模型 SDK、Provider、项目数据库或几何算法。所有工具输入先经公开 JSON Schema 验证，再调用本机 Runtime；所有输出都包含稳定 ID、Schema 版本、hash、lineage、能力状态和可恢复错误。

P0 使用 MCP `stdio`。Streamable HTTP、远程多租户、OAuth 和通用 MCP Client 均不在范围内。MCP Tasks/Skills 等可选协议扩展不能成为 P0 前置条件；长任务使用普通工具返回持久 `RuntimeJob`。

当前工具账本校正：源码默认 **111 个只读工具**，显式 write opt-in 后共 **194 个工具（111 read + 83 write）**；Formal High 公共面为 `production_weapon_formal_high_get/prepare`，Cage/Bake 公共面为 `production_weapon_high_low_bake_preflight_get/get/prepare`。preflight 不 materialize；全新 prepare 在七类 typed producer 未齐时保持 `runtime_write=false` 和 Store/CAS 零写入。下文旧数量仅指历史中间 receipt，不覆盖当前 manifest。

Stage 0 运行合同快照：当前共有 515 Schema、111 read + 83 opt-in write = 194 tools，唯一 `in_progress` 是 `FGC-MCP010F`，统一机器入口为 `docs/evidence/mcp010f/current-benchmark-truth.json`。Mechanical pose 与 Agentic observe/plan/critic/evidence 都是只读 projection；Hero UV `hero_uv_durable_get/prepare` 的 Store→Runtime→MCP 与真实 prepare/replay/drop/reopen/get **1/1 PASS** 仅证明四个 Hero CAS roots 的 linked/GC structural lineage，不证明 artist unwrap、visual、人评、engine、commercial 或 packaged acceptance。durable session/checkpoint/RepairIntent prepare/readback 与 CAS-bound RepairIntentRun 的隔离证据见各自 receipt，它只证明受限持久化和 staged transport，不证明 orchestrator 或 Repair 应用。attempt35 仅是 provisional retained observation，它的结果为 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`：camera-fit hash `354caf27…f95788` 与 reference-compare camera hash `8cd20605…a535` 为 `MISMATCH`；packaged Viewer binding 为 `PASS_CURRENT_COHORT_BOUND_READ_MODEL`，但不等于 attempt35 same-observation UI E2E。因此 source/raw/transport/build/AX smoke 只能证明对应合同或链路，不能声明视觉、人评或 packaged E2E PASS。


<!-- forgecad-reference-source: input=ENV_AUTHORIZED_PNG original_sha256=1964704a62ed7a841b4d49c370b8d46f4626e201daad29092a9c39a40b4c4109 intake=PASS_SOURCE_SIX_REFERENCE_EVIDENCE_CAS views=6 worker=PASS_SAME_COHORT_SIX_FIXED_VIEWS target=USER_REFINED_USER_CONFIRMED_REVIEWED_STRUCTURE user_confirmed_crop=PASS_USER_CONFIRMED_SEVEN_CROPS contour=PASS_USER_CONFIRMED_SIX_IDENTITY_CONTOURS negative_space=BOUNDING_REGIONS_CONFIRMED_EXACT_SUBTRACT_UNKNOWN line_flow=EXPECTED_ROWS_DURABLE_MATCH_NOT_PROVEN camera_lock_fixture=PASS_REAL_DURABLE_REPLAY_RESTART form_art_fixture=PASS_REAL_DURABLE_NOT_PROVEN form_quality_v2_fixture=BLOCKED_ZERO_WRITE_MISSING_LEGACY_CROSS_VIEW secondary_form_approved=NOT_CREATED fixture=PASS_REAL_1_OF_1_108.07S -->

## 2. 进程和信任边界

```text
Codex ──stdio── forgecad-mcp ──local authenticated IPC── forgecad-runtime
                       │                                  ├── SQLite V1
                       ├── short launcher election       ├── CAS
                       └── shared Runtime handoff         └── later workers
```

- `forgecad-mcp` 是 Codex 的唯一 MVP 入口；它拥有 stdio，会异步启动或连接同一数据根的共享 Runtime，不等待 Runtime ready。外部 `FORGECAD_RUNTIME_SOCKET`/`FORGECAD_RUNTIME_TOKEN` 仍可用于独立诊断，但普通配置不需要携带它们；
- MCP 进程无数据库写权限，无任意项目文件系统权限，不监听 TCP 端口；
- 多个 MCP 适配器没有可认证 Ready handoff 时，仅通过短时 `ipc/launcher.lock` 选出启动者；选主者复核/清理 stale handoff 并发起 Runtime spawn，spawn 成功后立即释放该锁。launcher lock 不授予 SQLite/CAS 写权限，也不是 Runtime 存活租约；
- Runtime 在打开数据库和 migration 之前取得 OS 独占 `runtime.writer.lock`，它才是最终唯一写者。MVP 不使用数据库 TTL lease、heartbeat、fencing、daemon、broker 或 stale takeover；第二个 Runtime 返回 `RUNTIME_BUSY`；
- 已经 Ready 的共享 Runtime 不属于某一个 MCP stdio 会话；正常适配器退出不终止它，只有显式 authenticated shutdown/update 流程主动停止。Runtime 跨会话存活不等于未完成 Job 已支持 checkpoint；
- Runtime 校验 project scope、base revision、idempotency key、candidate hash、approval receipt 和 tool capability；
- Worker 只接受受限内部协议，不接受 Codex 生成的 Python、JavaScript、shell、URL 或绝对文件路径；
- 工具失败时返回 typed error，不回退 legacy HTTP、Provider 或第二状态写者。

MCP010A 第一次真实 Desktop 重启暴露了 stale handoff、多适配器监督和单客户端 IPC 阻塞问题；失败 receipt 保持原样。共享 Runtime/IPC 修复的 focused/aggregate tests、同 cohort Dev.app 重建、package verify、隔离 probe 与第二次真实 Desktop 重启后的 30 工具/Ready/cohort/project readback Gate 均已 PASS；成功 receipt 为 `docs/evidence/mcp010a/codex-desktop-post-restart-success.json`。本节仍不宣称 MCP010B–F 的视觉质量能力。

## 3. Server 信息与能力协商

Server 名：`forgecad`。Server instructions 的前 512 字符必须自包含地说明：这是本地 3D Runtime；任何设计 tool 或其他 Skill 前先读取 `ponytail-preflight@0.1.0`；永久写入需候选与用户批准；长任务返回 job；禁止发送任意代码和未授权路径。

MCP003 使用 2025-era 的有状态 stdio 生命周期：`2025-11-25` 是 ForgeCAD 的规范版本，同时明确兼容 Codex 当前 stdio 默认发送的 `2025-06-18`。初始化必须包含 `protocolVersion`、`capabilities`、`clientInfo`；只接受这两个版本，并在响应中返回实际协商的版本。不匹配的版本、缺失参数或 Runtime 合同会 fail closed。初始化后，诊断可先调用 `capabilities_get`；进入设计链路时必须先 `skill_get(ponytail-preflight@0.1.0)`，随后才可调用其余 tool/Skill。返回：

- `runtime_version`、`contract_versions`、`tool_manifest_hash`；
- Viewer/Runtime/Worker/Skill 状态；
- 支持的几何、材质、纹理、渲染、质量、导入/导出格式；
- 图片导入模式及尺寸/MIME 上限；
- 每个工具 `available | unavailable | degraded` 与 limitation；
- 当前项目授权范围和写审批策略。

若 Server/Runtime/合同版本不兼容，所有写工具 fail closed；只允许诊断和导出备份。

## 4. 资源

| URI | 内容 | 约束 |
|---|---|---|
| `forgecad://capabilities` | 当前能力快照 | 无目标能力伪装 |
| `forgecad://projects/{project_id}/snapshot` | `ActiveDesignSnapshot` | 单一当前投影 |
| `forgecad://projects/{project_id}/selection` | Viewer 当前临时选择 | 非版本真值 |
| `forgecad://candidates/{candidate_id}` | 候选、readback、quality 摘要 | hash-bound |
| `forgecad://jobs/{job_id}` | Job 与最近事件 | 可重启读取 |
| `forgecad://versions/{version_id}` | 不可变版本和工件 manifest | 只读 |
| `forgecad://renders/{render_set_id}/{pass}` | 固定视图/AOV 图像 | MCP008 生成四个 bounded PNG pass；binary 仍走受保护的 Viewer IPC |
| `forgecad://skills/{skill_id}/{version}` | first-party Skill manifest + checked-in knowledge | MCP006 development-only registry；只读 metadata/knowledge，不含可执行 payload |
| `forgecad://artifacts/{artifact_id}` | hash-bound 工件元数据 | MCP007 通过 `artifact_readback_get` 读取；binary blob 仍不内联 |
| `forgecad://operators/catalog` | Runtime-owned `OperatorCatalog@1` | MCP010B V2 authoring catalog；必须与 `operator_catalog_get`、capabilities 和 V2 artifact/readback digest 相同 |

MCP003 当前已实现 capabilities、项目 snapshot/selection、candidate、job、version 的 JSON projection 和对应 resource templates；MCP005 增加 references，MCP006 增加 first-party Skill manifest resources，MCP007 增加 artifact metadata/readback，MCP008 增加 RenderSet metadata，MCP010B 新增 operator catalog resource 的可调用镜像。MCP raw tool 不内联原始 GLB/PNG bytes；可选 Viewer 通过 authenticated `artifact_bytes_get` 读 CAS bytes。资源 URI 只接受 `forgecad://` 和受限 opaque ID，不接受文件路径、URL、查询串或 `..`。

大二进制不内联到日志或事件；通过 MCP resource link 或受限 blob 读取，并声明 MIME、字节数和 SHA-256。

## 5. 工具目录

### 5.1 只读工具

- `capabilities_get`
- `runtime_status`
- `doctor`
- `operator_catalog_get`（与 `forgecad://operators/catalog` 同一 Runtime-owned `OperatorCatalog@1`）
- `geometry_program_hash`（只校验 hash-free `GeometryProgram@2` draft 并返回 compiler-owned hash；零持久化副作用）
- `project_list`
- `project_get`
- `snapshot_get`
- `selection_get`
- `candidate_get`
- `job_get`
- `job_events_read`
- `version_list`
- `version_diff`
- `skill_list`
- `skill_get`
- `quality_get`（结构/PBR/fixed-render checks；reference compare 明确 limited）
- `artifact_readback_get`
- `reference_get`
- `hero_uv_durable_get`（candidate-bound Low exact provenance；只读 durable Hero UV receipt）

MCP003 的工具清单固定排序并声明 `readOnlyHint=true`、`destructiveHint=false`、`idempotentHint=true`、`openWorldHint=false`。当前 MCP010B–F 源码默认有 **110 个只读工具**，显式 write opt-in 后为 **82 个写工具（总计 192）**；其中 `hero_uv_durable_get` 为默认读，`hero_uv_durable_prepare` 仅在 authenticated explicit write opt-in 后出现。V2 authoring、AssetPack 查询、轮廓目标读取、相机拟合证据、边界误差、多 Part 误差表、render image 和 durable session/checkpoint readback 工具只校验或读取，绝不把 MCP 适配器变成状态写者。`artifact_readback_get` 已读取 GLB header/lineage/part/triangle/UV/tangent metadata，`material_pack_get` 返回离线 AssetPack manifest，`quality_get`、`silhouette_target_get`、`camera_fit_prepare`、`boundary_error_get`、`session_get` 和 `checkpoint_get` 已可用但质量比较保守标记 limited；不得以自然语言把 limited、fixture 或 unavailable 伪装成视觉 PASS。`CameraCalibrationRef@1` 是 `SilhouetteFitIntent@1` 的闭合只读引用，只携带 Runtime-owned camera hash pair，Runtime 按 candidate/target evidence 解析完整 calibration，拒绝 hash 漂移或跨 candidate 引用。MCP010F Viewer 的 AOV、对比、部件/材质区筛选、爆炸图、热图和轮廓画布仅为只读 ephemeral projection；Viewer durable lookup 同样不提供写入入口。MCP010A Dev.app 的 30-tool activation receipt、MCP003 历史的 17-read snapshot，以及 MCP010B 的 3c/f488 package receipts 都必须保留为历史。当前总源合同为 515（历史合同 + MCP010B/C/D/E/F、Agentic contract family、Runtime Job/Primary Form、RepairIntentRun、Low durable 与 Hero UV durable contracts），B/C/D/E/F source-focused Gate 均已按各自范围通过；本段仍仅为 structural/source evidence，不等于 artist unwrap、visual、人评、engine、commercial 或 packaged pass；唯一 `in_progress` 仍为 `FGC-MCP010F`。

这些工具必须声明 read-only annotation，且不能以“读取”为名创建项目、下载网络资产、运行编译或改变 GC 生命周期。`reference_get` 只返回 ReferenceEvidence 元数据，不返回原始路径或字节；当前不提供原始图片 MCP blob 读取。

### 5.2 候选/任务工具

- `project_create`（MCP004；创建项目元数据）
- `candidate_prepare`（MCP004；diagnostic 或已入 CAS 的 typed object）
- `reference_import`（MCP005；只写 CAS/ReferenceEvidence，不创建版本）
- `geometry_prepare`（MCP007 V1 compatibility；MCP010B 也接受由 `geometry_program_hash` 生成 canonical hash 的 V2 program，且 program `project_id` 必须等于 outer target project）
- `appearance_prepare`（MCP008；bounded AppearanceProgram@1）
- `quality_get`（只读；Runtime-owned hard checks + limited reference evidence）
- `change_prepare`
- `restore_prepare`
- `export_prepare`
- `job_cancel`
- `hero_uv_durable_prepare`（explicit authenticated write opt-in；不 confirm/version/export）

它们可写临时 Job/CAS/candidate 状态，但不能创建永久资产版本。由于会读取附件、占用计算或创建临时工件，Codex 配置中按 write 工具处理。`quality_get` 是只读工具，不在此组重复列出；MVP 尚未提供 `visual_review_submit`、`exploded_view_prepare` 或通用 `candidate_render` 工具。

### 5.3 MCP004 已完成基座边界

MCP004 当前已在 Runtime 和 authenticated local IPC 实现并测试以下 typed 方法：`project_create`、`candidate_prepare`、`candidate_confirm`、`candidate_reject`、`restore_prepare`、`restore_confirm`、`export_prepare`、`export_confirm`、`job_cancel`。MCP007 增加 `geometry_prepare`，MCP008 增加 `appearance_prepare`，MCP009 增加 `change_prepare`。`candidate_prepare` 接受已经存在于 Runtime CAS 的 prepared object hash，或受限的 `request.typed=diagnostic` 非视觉合同对象；两条路径都不接受图片路径、任意代码或网络 URL。`quality_get` 现在执行 Runtime-owned geometry/GLB/UV/tangent/PBR/fixed-render hard checks，并可返回明确 `limited` 的 reference aspect comparison。`restore_prepare` 只接受 project 内已 confirmed 且 quality-passing 的历史 version，并以当前 head 绑定新 candidate；`restore_confirm` 在单一 SQLite 事务中创建当前 head 的新子版本，历史版本不被覆盖。`export_prepare/export_confirm` 支持 `manifest-json/diagnostic` 和 `glb/mvp-glb`；GLB 只允许 confirmed quality-passing Runtime GLB，confirm 返回 CAS output hash/receipt，不写任意本机路径。

当前 `forgecad-mcp` 源码的默认 stdio tool manifest 包含 **110 个只读工具**；显式 authenticated IPC + `FORGECAD_MCP_ENABLE_MCP004_WRITES=1` 时列出 **192 个工具，即 110 个只读 + 82 个写工具**（MCP004/005/007/008/009/010C/F、Agentic projection/durable prepare/staged RepairIntent run、Low durable 与 Hero UV durable）。`hero_uv_durable_get`/`hero_uv_durable_prepare` 均要求 candidate-bound current Low exact provenance；prepare/replay/drop/reopen/get 的真实 current-cohort receipt 为 **1/1 PASS**，四个 Hero CAS roots linked/GC，但该结果仍仅为 structural/source evidence，不是 artist unwrap、visual、human、engine、commercial 或 packaged pass。`subdivision_topology_lineage_preview` 是默认只读 structural projection：它调用 fixed Geometry Worker 并由 Runtime 独立重验 root coverage，不持久化谱系或改写 artifact。Agentic 的四个 projection 工具、`session_get`、`checkpoint_get`、`job_result_get` 是只读 Runtime-owned surface；`session_create_or_resume`、`checkpoint_prepare`、`checkpoint_restore_prepare` 与 `repair_intent_run_prepare` 是显式 approval-gated prepare，不直接 confirm candidate/version。`operator_catalog_get` 与 resource 完全镜像；`geometry_program_hash` 拒绝预填 hash、unknown/V1/catalog mismatch 和无效 draft，且没有 Store/CAS/Job/event 写入。`silhouette_target_get`、`camera_fit_prepare`、`silhouette_fit_prepare`、`part_contour_fit_prepare`、`silhouette_part_error_get`、`silhouette_candidate_compare`、`boundary_error_get` 只读 target/RenderSet 或运行有界相机、Rig、Part 和候选搜索；`reference_mask_prepare` 与 `reference_mask_refine_prepare` 才写入不可变 CAS target。`primary_form_repair_job_prepare` 只排队 Runtime-owned bounded search，终态通过 `job_get`/`job_result_get` 回读 CAS，不直接 confirm/version；`repair_intent_run_prepare` 只接受 Runtime-owned CAS `RepairIntent@1` 并返回 staged run，不应用 Repair 或创建版本。`material_pack_get` 只读取并校验 first-party manifest；`render_pass_get` 只 CAS 读取 RenderSet@2 并返回 PNG image block；`reference_compare_prepare` 生成 candidate/reference-bound camera、九 AOV、mask、metrics、diff，不创建版本；两个 review 工具只保存 typed evidence。V2 physical contract 为 position ±10 m、dimension/height ≤10 m、radius/radii ≤5 m。`runtime_status` 和 `doctor` 只读取生命周期状态，不运行 fixture、confirm、签名或完整验收。视觉证据工具声明 `requiresConfirmation`/write boundary，Runtime 不把 receipt当密码学身份认证。Runtime contract/version 由同版本 launcher 和 Runtime 事务合同保证，不把 client name 或一段 status 字符串当成安全边界。MCP/Runtime 不可用时 initialize 仍成功，依赖 Runtime 的调用返回结构化 `RUNTIME_UNAVAILABLE`、`retryable=true`。

`RenderSet@2` 还必须携带 `render_worker_build_cohort_sha256` 与 `render_worker_binding_status`。Runtime 只接受当前同 cohort 的 Worker hash；普通 source build 没有 cohort 时必须明确写 `cohort_unavailable`，不能伪装成 `same_cohort_verified`。Viewer 只读显示该绑定状态，质量门仍只来自 Runtime `QualityReport@2`。

MCP004 可按当前任务范围标为 done；其历史 evidence 中 reference/Geometry/GLB/signing 的 NOT_RUN/BLOCKED 保持不变，并分别转到 MCP005、MCP007–009 和 MCP013。

MCP010F 的 `silhouette_rig_hash` 是默认只读的 Runtime-owned authoring helper：它只接收候选绑定、无 `canonical_sha256` 的 `SilhouetteRig@1` draft，返回唯一 Rig hash，不创建 candidate、Job、版本或 CAS 对象。Codex/Luna 应先调用它，再把返回 hash 放回不变的 Rig draft，避免在自然语言、脚本或客户端中复制 canonical JSON 算法。

### 5.4 MVP 工具开放顺序

| Task | 新工具/能力 | 永久版本写入 |
|---|---|---|
| MCP005 | `reference_import`、`reference_get` | 否；只创建 ReferenceEvidence/CAS |
| MCP006 | `skill_list/get`、Skill resource 可用实现；development-only Bundle metadata | 否；创建 typed plan/candidate |
| MCP007 | `geometry_prepare`、`artifact_readback_get`、Viewer candidate/artifact read model | 否 |
| MCP008 | `appearance_prepare`、四 pass fixed render、Viewer artifact bytes | 否 |
| MCP009 | `quality_get`、`version_diff`、`change_prepare`、`glb/mvp-glb` export | confirm/export 依赖现有 approval 事务；reference compare limited |
| MCP010B | `operator_catalog_get`、`geometry_program_hash`、V2 `geometry_prepare` / `ArtifactReadback@2` | hash/catalog 读取零永久版本；prepare 仍只创建候选，需严格 readback 才可继续 |
| MCP010F | Viewer read model、AOV/compare/Part/MaterialZone/explosion/heatmap controls | 只读 Runtime projection；Viewer 不启动 Runtime、不写 SQLite/CAS/候选/版本 |

只有 producer、Runtime validator、negative tests、capability 状态和真实 evidence 同任务完成后，工具才可从 unavailable 变 available。不能先把空工具列出再用自然语言结果伪装实现。

隔离 source-built real Codex CLI 已完成 `project_create → reference_import → capabilities_get → operator_catalog_get → geometry_program_hash → geometry_prepare → artifact_readback_get` 的 V2 structural Gate；attempt 1 保持 `BLOCKED`，attempt 2 为历史 pre-semantic-Part-sink 的 `PASS`，且 candidate 未确认。固定同级 Worker 的 timeout/crash/FD isolation 和 accepted-result peak-RSS gate 另已通过，但 Darwin 512 MiB OS 总内存硬门为 `NOT_RUN`。3c/f488 Dev.app 的 V2 raw probe、packaged Worker structural E2E 和授权参考 CLI 链也都是历史 package receipt；f488 的候选未确认、为 12 Part/896 triangle，且 `chest-shell` 按顺序绑定 chest-shell/chest-panel。历史 `bfa56ac…de9` Dev.app receipt保留；当前 `d9c23b…ac0bd` Dev.app则通过 fresh package/Worker/raw/real-Codex structural Gate和 live Desktop structural activation，并产生相同结构规模的未确认 12 Part/896 triangle/161104-byte candidate；这些 receipt 都不证明参考相似度、材质/PBR V2、export/restore、Viewer hash 或 360°。

MCP005 已满足上述条件：Runtime `supports_reference_import=true`，`reference_import` 在显式 authenticated IPC opt-in 下可用，`reference_get` 为只读工具；真实 Codex CLI evidence 见 `docs/evidence/mcp005/codex-cli-reference-e2e.json`。Codex Desktop 当前 bridge 仍是 `NOT_RUN / unavailable`，不得写成 Desktop PASS。MCP005 的成功只证明真实图片字节进入 CAS，不证明视觉理解、几何或 GLB。

MCP006 已完成 historical development-only Bundle Gate：Runtime 已加载历史 first-party registry，`supports_skill_registry=true`，并通过 `skill_list`、`skill_get` 与 Skill resource 只读暴露 manifest。当前 registry 有 12 个 Bundle；`ponytail-preflight@0.1.0` 的 `skill_get` 同时返回 checked-in `SkillKnowledge@1`，MCP adapter 在新 session 中会先要求读取它。MCP010B 当前源码另外加载并验证 `primitive-blockout@0.2.0`，其 `forgecad.geometry.primitive@2` 是当前唯一 active V2 Skill consumer；历史 Bundle 和新 Bundle 均包含本地合同 schema、Recipe、operator/validator allowlist、合成正/负 fixture、benchmark receipt、许可证、SBOM、provenance 和 development trust manifest。`scripts/check_mcp006_skills.py` 校验 DAG、单位、finite、预算、canonical hash、路径/脚本/网络 capability，并 fail closed。它们不是“已签名安装包”，不执行任意代码，不替代 Geometry/Render 结果；distribution signing/revocation 延后 MCP012–013。

MCP007 已完成 geometry Gate：`geometry_prepare` 只接受 canonical `GeometryProgram@1`，当前 allowlist 为 product-owned box/cylinder/sphere primitive；Runtime 写入 geometry GLB CAS，创建 reviewable candidate/quality report，返回 `GeometryPrepareResult@1` 与 strict `ArtifactReadback@1`。MCP008 已在其上完成 bounded Appearance/Render；MCP009 已完成 limited quality/change/version/export functional core。真实 Codex CLI geometry/readback slice 已 PASS（14 parts/516 triangles，见 `docs/evidence/mcp007/codex-cli-geometry.json`）；`docs/evidence/mcp009/codex-cli-appearance-export.json` 另记录真实图片附件到 appearance、quality、confirm、version 和 CAS-only GLB export 的十二调用 host golden path。MCP010A 已通过最小 Desktop activation write probe；完整 Desktop 3D write、packaged、像素/视觉 gates 仍保持 `BLOCKED/NOT_RUN`，不得把有限主链路扩展成通用质量结论。

### 5.5 永久写工具

- `project_create`（MVP 直接创建项目元数据；完整 prepare/confirm 项目策略仍是后续合同）
- `candidate_confirm`
- `candidate_reject`
- `restore_confirm`
- `export_confirm`

MVP 不暴露 `skill_install_confirm` 或 `skill_disable_confirm`；Skill Bundle 只能读取 first-party development registry，第三方安装留 MCP012。

永久工具必须绑定：

```json
{
  "project_id": "...",
  "base_version_id": "...",
  "prepared_object_id": "...",
  "prepared_object_sha256": "...",
  "quality_report_id": "...",
  "approval_receipt_id": "...",
  "idempotency_key": "..."
}
```

Runtime 必须确认审批未过期、范围和 hash 完全一致、基线未漂移、硬质量门通过。确认结果是不可变版本；同一幂等键重复调用返回同一结果。

请求中的 `approval_receipt_id` 在 MVP 中只是 Codex approval context 的 opaque id；Runtime 不信任它作为最终凭证。confirm/reject/export 成功或记录过期审批时，由 Runtime 在事务内生成 `receipt-...` 的最终持久化 receipt，并在结果中返回该 ID。它是宿主审批流程证据，不是密码学人类签名。

### 5.6 MCP010C 当前工具（source raw 已验证；真实/packaged 视觉门未运行）

| 工具 | Annotation/确认 | 目标合同 |
|---|---|---|
| `render_pass_get` | read-only/idempotent | 只返回已持久化且 hash-bound 的 PNG image block |
| `reference_compare_prepare` | write/temporary | 生成 camera、mask、metrics、diff；不创建版本 |
| `visual_review_submit` | write/evidence | 保存绑定 candidate/render/pass/region 的 typed review |
| `human_visual_review_submit` | write/evidence + confirmation | 保存用户评分；Runtime receipt 不作为密码学身份认证 |

`quality_get` 保持既有只读名称，现可读回 candidate-bound `QualityReport@2`；source synthetic/raw PASS 不等于用户图片 likeness PASS。当前 source manifest 为 111 read + 83 opt-in write = 194；Hero UV durable receipt 仍仅为 structural/source evidence，不是 artist unwrap、visual、human、engine、commercial 或 packaged pass。Agentic projection 只读、可重建，durable session/checkpoint/RepairIntent 与 CAS-bound RepairIntentRun 只代表已验证的 prepare/readback/staged-run receipt，不是 QualityReport 或 confirmed version 真值；空工具、自然语言结果或 target Schema不能改变 capability 状态。同一 provisional observation 绑定的 packaged Viewer E2E、live C/D/E/F、人评阈值、真实 PBR likeness 和 HQ_360 仍必须独立记录；Stage=`camera-calibrated`、visual=`QUALITY_TARGET_NOT_MET`，不推进 Stage/confirm/version/export。

MCP010E 的 first-party 离线 AssetPack 由应用资源和 Runtime CAS bootstrap 提供，不新增通用 `material_pack_install` 工具；publisher/install/disable/upgrade/revoke 属 MCP012。

### 5.7 商业模块 MCP/Runtime 边界（目标/排队）

商业模块不通过“新增一个万能工具”实现。每个目标模块必须先有 `ForgeCadModule@1` manifest，再由 Runtime 选择同版本、同 cohort 的预注册 Worker；MCP 只转发 closed request/result，不能读取 SQLite/CAS，也不能加载外部插件。目标模块与当前状态如下：

| 模块 | 目标合同 | 预期 MCP/Runtime 形态 | 当前状态 |
|---|---|---|---|
| Authoring Mesh | `AuthoringMesh@1` + `AuthoringMeshIdentityLineage@1` | candidate-bound read projection 与显式 edit prepare；原生 half-edge kernel | split/collapse/dissolve 3/3 PASS；general correspondence、evaluated retarget、跨版本 editor `NOT_PROVEN` |
| Native High | `HighMeshArtifact@1` + `DetailGraph@1` | Runtime-owned prepare/job/readback；不进入 live Skill 选择面 | source durable/GLB slice；proposal `registered=false`、`FPS-HIGH-05=NOT_PASSED` |
| Retopology / Low | `LowMeshArtifact@1` + `RetopologyConstraintSet@1` | bounded draft/prepare/get；必须保留 candidate/High/Part lineage | Low 当前 `DRAFT_UNREVIEWED / structural_only`，artist promotion `NOT_RUN` |
| Hero UV | `HeroUvLayout@1` + `HeroUvDurable*` | `hero_uv_durable_get` 只读、`hero_uv_durable_prepare` explicit write opt-in | public Store→Runtime→MCP 与 replay/drop-reopen/get 1/1 PASS；仍非 artist/commercial pass |
| Cage-Bake / Surface / LOD | `CageArtifact@1`、`HighLowBakeReceipt@1`、`MaterialLayerGraph@1`、`HeroMaterialPack@1`、`HeroLodSet@1` | Cage/Bake public get/prepare 已存在；其余按 typed workers/jobs，所有结果 candidate/export-hash bound | Formal High internal materializer 与 Cage/Bake Worker/Store/MCP seam 仅 source/compile/focused；完整 positive restart/public surface/current-D1 receipt 缺失且 quality failed；Surface/LOD `NOT_RUN/NOT_PROVEN` |
| Engine / Art Review | `EngineValidationReceipt@1`、`HeroArtReviewReceipt@1` | future readback/review receipts；不能由 Codex 自评生成 PASS | commercial engine 与独立 Hero Art Review `NOT_RUN` |

`ForgeCadModule@1` 的 closed manifest 至少包含：`schema_refs`、`operator_refs`、`budget`、`fixture_refs`、`license_text_sha256`、`notice_sha256`、`sbom_sha256`、`provenance`（source revision/toolchain/build cohort）、`signature`、`module_sha256`、`contract_set_sha256`、`input_sha256`/`output_sha256` lineage，以及显式 `network=false`、`dynamic_plugin=false`、`script=false`、`direct_db_write=false`。预算字段只接受有限的 CPU/time、peak RSS、输入/输出 bytes、element/triangle/texture/CAS 上限；未有 benchmark receipt 的数值保持 `queued`，不得自行推导硬门。所有正/负 fixture 必须是可公开、无用户图片/绝对路径/secret 的 deterministic fixture。模块未同时通过合同 checker、资源/恶意输入 Gate、LICENSE/NOTICE、SBOM、provenance、签名和 hash 校验时，Runtime 返回 `CAPABILITY_UNAVAILABLE`，不推进 Stage、confirm、version 或 export。

## 6. 参考图片导入

`reference_import` 只接受以下二选一来源：

1. `inline_content`：受合同限制的 MIME、尺寸和字节数；
2. `codex_local_file`：Codex 提供的本地附件路径，但必须位于启动时显式授权的 attachment roots 或 OS 单文件授权内。

路径处理顺序固定：canonicalize → 拒绝 symlink/目录/设备文件 → 检查 root → MIME sniff → size/dimension/decompression-bomb 检查 → 计算 hash → 复制到 CAS → 丢弃原始路径。日志和永久对象不得保存用户名或绝对路径。

P0 Gate 必须分别在 Codex Desktop 和 Codex CLI 上证明实际附件字节能进入 CAS；客户端只让 Codex“看见图片”不算通过。Codex IDE/VS Code/Cursor/Windsurf 的附件传输保留为未来兼容 Gate，不阻塞当前 MCP003/MCP004。若某客户端不能传附件，能力快照必须明确 `unavailable`，不得静默用语言描述替代原图。

## 7. Job 合同

预计超过 10 秒的操作必须在 2 秒内返回：

```json
{
  "schema_version": "RuntimeJob@1",
  "job_id": "job_...",
  "kind": "candidate_compile",
  "state": "queued",
  "project_id": "...",
  "request_sha256": "...",
  "created_at": "...",
  "poll_after_ms": 1000,
  "cancel_supported": true
}
```

状态：`queued | running | waiting_for_input | succeeded | failed | cancelled`。事件为单调序列、可分页和重放；事件只引用 CAS，不含 prompt、图片字节、密钥和绝对路径。`job_result_get` 返回 `RuntimeJobResult@1`，其中 `result_sha256` 必须来自终态 `completed` event，结果 payload 再由 Job kind 的合同（当前 Primary Form 为 `PrimaryFormRepairPrepareResult@1`）约束。Runtime 重启后，终态可读取；非终态只能按已提交 checkpoint 续接，否则转为 typed failure，不能假装继续。当前 Primary Form asynchronous Job 仅是单进程内 IPC 解耦，不宣称跨重启续跑。

## 8. 错误合同

至少支持：

- `CAPABILITY_UNAVAILABLE`
- `CONTRACT_VERSION_UNSUPPORTED`
- `PROJECT_SCOPE_DENIED`
- `REFERENCE_TRANSFER_UNAVAILABLE`
- `REFERENCE_REJECTED`
- `STALE_BASE_VERSION`
- `CANDIDATE_HASH_MISMATCH`
- `APPROVAL_REQUIRED`
- `APPROVAL_EXPIRED`
- `QUALITY_HARD_GATE_FAILED`
- `SKILL_UNTRUSTED`
- `WORKER_BUDGET_EXCEEDED`
- `JOB_CANCELLED`
- `RUNTIME_RECOVERY_REQUIRED`

错误包含机器可读 code、safe message、retryable、next action 和 evidence IDs；不得返回 stack trace、原始请求、密钥或本机绝对路径。Runtime 已连接但拒绝请求时使用 `INVALID_INPUT`、`STORE_ERROR`、`RUNTIME_BUSY` 或 `IPC_ERROR` 等非 retryable typed code；只有连接/启动/ready handoff 故障使用 `RUNTIME_UNAVAILABLE`。MCP 会丢弃 IPC 错误细节中的路径和用户输入，避免把本机路径泄露给 Codex。

## 9. Codex 配置基线

开发期基线位于 `config/codex/desktop.toml`、`config/codex/cli.toml`、`config/codex/ide.toml`。基线不设置 `CODEX_MCP_PROTOCOL_VERSION`，因此使用 Codex 的 2025-era 默认兼容路径：

```toml
[mcp_servers.forgecad]
      command = "forgecad-mcp"
args = ["serve", "--stdio"]
enabled = true
startup_timeout_sec = 20
tool_timeout_sec = 60
required = false
default_tools_approval_mode = "writes"
```

项目级 `.codex/config.toml` 只允许稳定 `forgecad-mcp` 入口；发布安装器负责把它解析到 `/Applications/ForgeCAD Runtime.app/Contents/Resources/forgecad-mcp` 或等价稳定路径。`required=false` 是生产容错边界：ForgeCAD 故障不得阻断 Codex startup/resume。fixture、临时 Runtime data dir、token 值和用户 Library 路径不能进入正式配置。

## 10. MCP002 已通过的合同 Gate

- 首批 Project/Candidate/Version/Snapshot/Job/Event/Audit/CAS Schema、Rust records 和 manifest 无漂移检查；
- Runtime V1 migration、legacy database rejection、WAL/foreign keys/busy timeout、事务回滚、重启和 backup/restore；
- CAS SHA-256、容量限制、临时文件 + fsync + 原子 rename、missing/corrupt/hash mismatch；
- Runtime migration 前 OS 文件锁、第二 Runtime `RUNTIME_BUSY`、进程退出自动释放锁；
- Unix socket 0600、token hash + constant-time comparison、错误 token fail closed；
- MCP crate 不依赖 SQLite，IPC read dispatch 只返回结构化 Runtime projection。

## 11. MCP003 已完成的本地合同 Gate

本节的 17/19/33/54 等工具数量均是历史 MCP003–MCP010F 中间 cohort；2026-08-26 现行 manifest 固定为 111 read + 83 opt-in write = 194 tools。

- `docs/evidence/mcp003/protocol-snapshot.json` 固定 MCP `2025-11-25`、Codex `2025-06-18` 兼容版本、initialize 字段、method、tools、annotations、resource templates 和 1 MiB projection 上限；
- MCP003 历史 `resources/list`、`resources/read`、`resources/templates/list` 与 17 个只读工具（含 `runtime_status`/`doctor`/`reference_get`）由当时 Rust 单元测试及静态合同检查覆盖；MCP010B 曾在此基础上增加两个默认 read tool，形成历史 19-read 中间 source receipt，不改写原 receipt；其后历史 C/D/E/F snapshot 扩展为 33 read + 18 write；当前 source manifest 为 54 read + 35 opt-in write。
- `npm run mcp003:stdio` 的历史 receipt 校验四个响应、17 个只读工具、能力资源和协议不兼容 fail-closed。历史 19-read MCP010B raw probe 仍只作为中间 receipt 保留；该历史 source snapshot 由 MCP010C–F raw probes 校验 36 read + 23 write；当前 source manifest 为 54 read + 35 opt-in write。两者都是传输层证据，不等于 required Codex Desktop/CLI 宿主 E2E，也不把 IDE 变成当前 P0 Gate；MCP005 的 reference CLI admission 另由 `script/test_mcp005.sh` 和对应 Codex receipt 覆盖。
- 官方 `@modelcontextprotocol/sdk` 的 `StdioClientTransport` 历史 MCP003 独立探测列出 17 个只读工具、1 个资源并读回 capabilities；该历史 MCP010B–F source probe 验证 33 read + 18 write；当前 source manifest 为 54 read + 35 opt-in write，并只对当前实际 probe 到的资源数量作出声明。历史 14-tool receipt 仍只描述旧会话，不覆盖当前 manifest；
- Server/Runtime contract mismatch、协议版本不支持、非法 URI、非法 opaque ID 和未实现能力均 fail closed；
- Desktop/CLI/IDE 配置基线不含 secret、绝对路径或现代协议 opt-in；所有基线使用 `forgecad-mcp` 单入口，`docs/evidence/mcp003/host-matrix.json` 记录 required protocol adapter、Codex CLI、Codex Desktop PASS，Desktop 实际 `initialize.protocolVersion=2025-06-18`，forced mismatch 为 `HOST_OVERRIDE_IGNORED / NOT_APPLICABLE`；IDE 保持 `OPTIONAL_NOT_IN_SCOPE`。

## 12. 协议版本边界与宿主诊断

MCP `2026-07-28` 是另一种协议时代：它移除了 `initialize`/`notifications/initialized` 和会话状态，使用 `server/discover`、每请求 `_meta` 与 `requestState`。ForgeCAD MCP003 不宣称支持该现代 wire mode；把它和 2025-era 状态机混在一个未标注的进程中会造成错误的安全和生命周期假设。若配置了 `CODEX_MCP_PROTOCOL_VERSION=2026-07-28`，MCP003 应明确失败，而不是静默降级。待 Codex 宿主和 ForgeCAD 分别完成现代 stdio adapter 合同后，再以独立任务引入。

本地 Codex app-server 诊断和真实 Desktop handshake 已证明当前宿主发送 `2025-06-18`，ForgeCAD 返回相同值；此前只接受 `2025-11-25` 会返回 `CONTRACT_VERSION_UNSUPPORTED`。Desktop 的 `launchctl CODEX_MCP_PROTOCOL_VERSION=2026-07-28` override 被宿主忽略，不能产生真实 mismatch，因此 Desktop 记录 `HOST_OVERRIDE_IGNORED / NOT_APPLICABLE`，禁止代理改写请求伪造 PASS。一次真实、认证的 `codex exec` 只读模型回合完成了 `capabilities_get` 和 `selection_get`，未发生写事务；设置 `CODEX_MCP_PROTOCOL_VERSION=2026-07-28` 的第二次真实回合明确返回 unsupported-protocol、没有工具调用、静默降级或副作用。因此 MCP003 将 2025-06-18 作为显式兼容版本保留，protocol adapter/raw probe 与真实 CLI 共同承担协议负面测试。

## 13. 完整合同 Gate（MVP + release）

- 当前 P0 required：protocol adapter 与 Codex Desktop/CLI smoke；Codex IDE 不属于当前 MCP003/MCP004 发布阻断；
- tools/list、resources/list、Schema、annotations 和 server instructions snapshot；
- 每个工具成功、非法输入、重复请求、stale base、越权、取消、重启和超时测试；
- Viewer 或普通 MCP 适配器关闭不破坏、也不主动终止已经 Ready 的共享 Runtime；显式 authenticated shutdown/update 才停止 Runtime。已确认数据必须可重启读取；Runtime 崩溃或缺少兼容 checkpoint 时，非终态 Job 明确失败；

注意：截至本版本，[官方 `@modelcontextprotocol/conformance`](https://github.com/modelcontextprotocol/conformance) 的 server 命令以 Streamable HTTP URL 为入口，而 MCP003 产品合同固定为本地 stdio。不能把一个临时 HTTP 代理的结果写成 stdio Server 已通过官方 conformance；要关闭这一项，必须另立 transport adapter 合同并分别验证，或采用官方支持 stdio 的 runner。当前证据诚实保留为 `NOT_RUN`，但它不是当前 P0 required host Gate，也不能反向阻塞 MCP003/MCP004。
- Runtime 关闭时 MCP 明确失败且不启动 legacy sidecar；
- 任何永久版本都能回溯到请求 hash、候选、质量、审批、Skill 和工件 hash。

## 14. 版本参考

- [OpenAI Codex MCP 文档](https://developers.openai.com/codex/mcp/)和 [Codex MCP connection manager](https://github.com/openai/codex/blob/main/codex-rs/codex-mcp/src/connection_manager.rs)决定 P0 实际 Codex 配置、默认 legacy 版本和显式 modern opt-in；
- [MCP lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle)和 [MCP resources](https://modelcontextprotocol.io/specification/2025-11-25/server/resources)用于协议/资源设计；
- [MCP 2026-07-28 发布说明](https://blog.modelcontextprotocol.io/posts/2026-07-28/)只用于规划未来无握手/无会话 adapter，不代表 MCP003 已实现；
- [MCP Tasks extension](https://modelcontextprotocol.io/extensions/tasks/overview)和 Skills-over-MCP 仍需按客户端能力协商，P0 不依赖它们。

MCP 规范与 Codex 已发布行为可能不同步。`FGC-MCP003` 已 pin 协议版本和配置基线，protocol adapter、认证 CLI 只读/负面回合和真实 Desktop handshake/read-only 回合已有证据；IDE、其他 MCP Client 与官方 transport conformance 仍按未来/非阻塞范围记录。不得把本地适配器或无模型回合 app-server 诊断扩大成未执行的宿主能力，也不得依赖已废弃 Roots/Sampling/Logging。

<!-- forgecad-stage0: schemas=662 schema_set_sha256=202e080ec378ddb294eb9c880079dcec5c910b27a1c679034ca34c5a880dcec6 read_tools=131 write_tools=95 total_tools=226 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260815-b37-complete-auto-v3.json latest_completed=real-codex-cli-current-20260815-b37-complete-auto-v3.json -->
