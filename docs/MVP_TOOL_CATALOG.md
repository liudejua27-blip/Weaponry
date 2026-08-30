# ForgeCAD MVP 工具、Skill 与外部项目目录

> **Weaponry P0 override (2026-08-29):** 本文只有在与 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md` 和 ADR-0029 一致时才具有当前执行权。ForgeCAD 在本文中解释为 Weaponry 的 Rust Runtime lineage；当前唯一产品主线是由 Codex 生成、修改、验证并交付高质量穿越火线非功能性游戏武器。通用 3D、机器人和原创科幻示例仅作 fixture/历史能力，不得抢占本月主线。 本文中所有 2026-08-28 及更早的“当前”“下一原子”“唯一 `in_progress`”和工具/Schema 数量语句均按历史 cohort 解释，不得覆盖 `WPN-*` successor queue。

> 2026-08-30 current catalog：默认 Action Space 是 11 个 Knife façade、125 个 active operation、125/125 closed request Schema；兼容面是显式 226 raw tools。当前下一步是 MCP/Runtime/Store 物理拆分，不以继续增加工具数量作为能力或质量指标。

## Weaponry public Tool profile

新公共能力优先复用和扩展五个稳定任务面，而不是新增主题化工具：

| Surface | 责任 | 示例目标 |
| --- | --- | --- |
| Observe/Query | 授权、参考、拓扑、选择、Modifier、候选、证据 | `authoring_document_get`, `selection_query` |
| Author | 原子 journal preview/prepare | 扩展现有 `authoring_mesh_edit_preview/prepare` |
| Evaluate | Modifier、High/Low/UV/Bake/PBR、固定渲染 | `evaluation_prepare/get` family |
| Review | AOV、compare、critic、human/engine receipt | candidate-bound read surfaces |
| Version/Deliver | checkpoint、confirm、restore、export | 现有事务面收敛 |

`fictional_energy_*`、`production_weapon_*`、`game_weapon_*` 和 V1/V2/V3 surface 先进入
legacy/replay profile；只有完成 persisted-record reachability 和 successor migration 才能删除。

> 2026-08-29 移除 Blender task/capability 占位链后的 current-source 为 **583 schemas / 131 read + 95 opt-in write = 226 tools**。新的稳定任务面是默认只读 `authoring_mesh_transaction_get` 和显式写 `authoring_mesh_transaction_prepare`。它们提供通用原子 journal，不绑定某把武器/candidate/session；上层候选、审批和质量流程必须通过独立编排组合，不得塞进几何原语以缩小 action space。
本月不得用 Tool 数量增长证明 Action Space 或质量提升。

> 2026-08-29 当前公共面仍为 **585 schemas / 131 default-read + 94 opt-in write = 225 tools**。本轮没有新工具或 Schema；仅将 `production_weapon_form_art_composite_evidence_prepare` 内部的 secondary Form 评审 policy 升级为 `@2`，对 core 512px raster metrics 允许绝对 tradeoff `0.01`。semantic/topology/hash/UV/Bake/human 门不变，因此该变更不新增商业质量能力计数。

> 2026-08-29 当前公共面：**585 schemas / 131 default-read + 94 opt-in write = 225 tools**。新增默认只读 `production_weapon_form_art_target_occlusion_attribution_get`；它只从 hash-bound candidate/FormArt/camera/target/AOV 派生像素遮挡归因，不写 Runtime 状态、不授权 confirm 或外观阶段。

> 2026-08-28 当前目录仍为 **583 schemas / 130 read + 94 opt-in write = 224 tools**。现有 composite proposal write tool 新增 4 个 `receiver-upper` registered profile；它们只接受 canonical D1 box 父节点与空 inputs，分别回撤 min/max-X 20/40mm。真实 L 四候选均视觉失败，因此只证明 bounded Part mutation 和拒绝闭环，不证明孔洞质量能力。

> 2026-08-28 当前目录仍为 **583 schemas / 130 read + 94 opt-in write = 224 tools**。`production_weapon_form_art_composite_proposal_prepare` 的闭合 profile 集新增 4 个 `side-panel-a` 真孔 `@1` 与 4 个相机映射真孔 `@2`；Runtime 只接受完整 canonical parent node、空 inputs 与注册参数，并以 product-owned `multi-loop-profile-loft` 生成真实内外环。真实 J/K 8 个候选均未打开 approved target，因此这些 profile 是结构能力而非视觉质量能力。

> 2026-08-28 当前目录仍为 **583 schemas / 130 read + 94 opt-in write = 224 tools**。`production_weapon_form_art_composite_proposal_prepare` 的闭合操作集现包含 4 个 `side-panel-a` aperture sensitivity profile：min/max-X 各 20/40mm。它们只能改变 exact `side-panel-a` panel node，必须在同源 baseline 上运行 strict readback 和 54 AOV。真实 04BE-I 四变体全部退化且目标孔洞仍 sealed，因此没有新增可保留的网格能力结论。

> 2026-08-28 当前目录为 **583 schemas / 130 read + 94 opt-in write = 224 tools**。新增默认只读 `production_weapon_form_art_aperture_repair_plan_get`：请求只接受 04BE-G/F 与 durable proposal 的 exact IDs/hashes；Runtime 自行重放并从 CAS 验证 GeometryProgram 节点。返回 2 个严格顺序步骤和 8 个 bounded 变体，但不调 geometry worker 执行变体、不写 Store/CAS、不推进生产门。

> 2026-08-28 当前目录为 **581 schemas / 129 read + 94 opt-in write = 223 tools**。新增默认只读 `production_weapon_form_art_visibility_calibration_get`：请求只接受 04BE-F diagnostic 与 04BE-E durable evidence 的 exact IDs/hashes；Runtime 重读 before/after GLB、ReferenceCanvas、CameraRig/AOV 并由隔离 Render Worker 派生 raster attribution。工具不接受 raw mesh、mask、AOV bytes、camera、脚本、路径或 URL，不写 Store/CAS；它只授权后续 typed plan，不创建 FormQualityV2 或推进 Stage。

> 2026-08-28 当前目录为 **577 schemas / 127 read + 94 opt-in write = 221 tools**。新增默认只读 `production_weapon_form_art_repair_plan_get`：请求只含 durable IDs/hashes 和 input hash；Runtime 重读 04BE-C sidecar、CrossView、proposal FormArt 与 composed GeometryProgram，派生固定 `rear-stock-owner-void-half-y-flat-z@1` 计划。该工具不调 Worker、不写 SQLite/CAS、不执行 repair、不推进 Stage/confirm/version/export，也不接受 raw mesh、vertex selection、camera、script、path 或 URL。真实 D1 restart equality PASS，但质量仍 `QUALITY_TARGET_NOT_MET`。

> 2026-08-28 当前目录为 **575 schemas / 126 read + 94 opt-in write = 220 tools**。新增默认只读 `production_weapon_form_art_composite_evidence_get` 与显式 write opt-in `production_weapon_form_art_composite_evidence_prepare`；MCP 只传 durable IDs/hashes，54 AOV、相机、CrossView、FormArt 与 receipt 均由 Runtime 重读并派生。prepare 只固化不可变、non-promoting 证据，不允许调用方提交 mesh/AOV bytes/camera/script/path/URL，也不确认候选或推进 Stage。真实 D1 prepare/restart GET 已 PASS，但质量仍为 `QUALITY_TARGET_NOT_MET`。

> 2026-08-28 当前目录为 **572 schemas / 125 read + 93 opt-in write = 218 tools**。新增默认只读 `production_weapon_form_art_composite_proposal_get` 与显式 write opt-in `production_weapon_form_art_composite_proposal_prepare`；prepare 只接受 durable IDs/hashes 和 1–8 个产品注册、source-node/Part 互斥 replacement，不接受 raw mesh、profile points、任意 GeometryProgram patch、operator、script、path 或 URL。真实 D1 prepare/restart GET 已 PASS，但六视图/FormArt/visual/human/engine 未过，不能确认或晋级。

> 2026-08-27 当前目录为 **568 schemas / 124 read + 92 opt-in write = 216 tools**。新增默认只读 `production_weapon_owner_reviewed_void_calibration_get`；它只从 Runtime durable FormArt/fresh baseline/RigV2/Part-ID/depth/批准相机派生 left/right/rear-three-quarter 的 `rear-stock` 身份校准，不接受 mask/transform/camera matrix/raw mesh/路径/URL/脚本，不写 Store/CAS、不调 Worker。返回 calibration eligible 仅允许下一次 bounded art-shape，不替代 strict owner-void、视觉、secondary 或商业质量门。

> 2026-08-27 `FPS-FORM-04AL` 当前增量：Runtime-owned durable fresh six-view baseline producer 已接通合同、Store、Runtime 与 MCP `prepare/get`；每个视图绑定 approved registration lineage / RigV2、fresh same-cohort 512×512 九 AOV、camera/mask/compare/quality 与完整 CAS reachability，并以单事务持久化。精确状态为 `PASS_SOURCE_COMPILE_DURABLE_PRODUCER_NOT_RUN_REAL_D1`；真实 D1、orientation approval、fresh baseline、notch、secondary、Stage/confirm/version/export 均未执行。当前公共面 **538 schemas / 118 read + 88 opt-in write = 206 tools**，视觉仍 `QUALITY_TARGET_NOT_MET`。

> 2026-08-27 `04AK`：新增默认只读 `production_weapon_form_art_baseline_preflight_get`；当前目录为 **533 schemas / 117 read + 87 opt-in write = 204 tools**。该工具只验证 approved lineage、RigV2、scope 与固定六视图并返回 producer blocker，不启动 Worker、不写 Runtime、不接受任意脚本或路径。

> 2026-08-26 `04AI`：新增一个默认只读工具 `production_camera_lock_registration_lineage_preflight_projection_get`，该轮目录为 531/116+87；当前已由 04AK 更新为 **533 schemas / 117 read + 87 opt-in write = 204 tools**。该工具只做 Runtime-derived semantic camera 投影和完整 lineage 验证；无 Worker、无持久化、无任意 camera/mesh/script 输入。

> 2026-08-26 `04AH`：工具面不变（115 read +87 opt-in write=202）。现有 CameraLock lineage prepare 的闭合参数已区分 reference rotation、rear3q stock/muzzle screen order 与 Runtime camera orbit；无新增泛化 camera/script tool。

> 2026-08-26 目录增量：不新增工具。既有 `production_weapon_form_art_mesh_proposal_get/prepare` 的 typed edit 变体新增 `AuthoringMeshOpenFrameNotch@1`，故工具面仍为 **115 read + 87 opt-in write = 202**；Schema 面增至 529。真实 D1 证据为执行 PASS / 视觉拒绝，不改能力晋级状态。

> 2026-08-26 当前目录：**527 schemas / 28 operators / 115 read + 87 opt-in write = 202 tools**。新工具为 `production_weapon_form_art_mesh_proposal_get/prepare`；它将真实 D1 stable-ID `MoveVertices` 接入 durable revision、单 source-node lowering和六视图，不接受 caller mesh/program。当前派生方案只为 reviewable tradeoff，因缺 proposal-side FormArt owner 证据而 blocked；新工具数不构成商业质量进展。

> 2026-08-26 当前目录：**527 schemas / 28 operators / 114 read + 86 opt-in write = 200 tools**。新增 `production_weapon_authoring_mesh_v2_source_prepare`：仅由 Runtime 从 exact candidate/program/artifact/readback/Part/source 绑定生成真实武器 `AuthoringMeshRevision@2` genesis；不接受 caller topology，不推进 Stage。真实 D1 rear-stock prepare→restart get 已 PASS，视觉/High/商业仍未通过。

> 2026-08-26 `04AF` 现行目录：**527 schemas / 28 operators / 114 read + 85 opt-in write = 199 tools**。新增公共面为 `authoring_mesh_v2_durable_get/prepare` 和只读 `production_camera_lock_registration_lineage_preflight_get`。前者已有真实 Runtime restart 结构回执，后者在没有 orientation-specific 用户回执时 fail closed；两者都不代表商业资产通过。

> 2026-08-26 04AE 现行目录：**525 schemas / 28 operators / 112 read + 84 opt-in write = 196 tools**。新增两个 MaterialLayerGraph plan schema 与专用 Worker entry，不新增 MCP 工具；真实 durable/texture instance 仍 blocked，旧数量只是历史 cohort。

> 2026-08-26 目录约束：后续商业工具必须服务纵向 stage，不新增可越过 Form 的独立“自动商业化”入口。自动 retopo/UV/LOD/material generation 只能返回 draft；promotion 必须读取上游 approved artifact、同 hash diagnostics 和人审状态。详见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

> 2026-08-26 `FPS-FORM-04AD` 权威增量：当前合同面为 **518 schemas / 111 read + 83 opt-in write = 194 tools**。新增 `ProductionWeaponSemanticLandmarkOrdering@1` 只表达 Runtime-derived 的 3D source/subject-axis 顺序，明确 `target_landmark_arrays_present=false / metrics=NOT_PRESENT`，不得冒充 2D landmark；`ProductionWeaponAuthoredViewOrientation@1` 将诊断变换与用户方向回执分开；`RegisteredCameraRigCalibration@2` 只有绑定 promotable authored rear3q receipt 才能物化。定向 Contracts/Runtime/MCP compile 与 518-schema checker PASS。真实 D1 尚无 orientation-specific user receipt，因此保持 `BLOCKED_AUTHORED_REAR_THREE_QUARTER_ORIENTATION`、Stage=`camera-calibrated`、secondary=`NOT_CREATED`、quality=`QUALITY_TARGET_NOT_MET`，不 confirm/version/export。旧 `@1` 保持历史真值；durable 落点采用 CameraLock 的 additive child lineage，不复制/自动升级整张旧记录。

> 2026-08-26 内部策略增量：`rear-stock-profile-reconstruction-v1` 是既有 DesignActionRun/RuntimeParameterPatch 内的 product-owned materializer，不新增 MCP tool，也不进入 public assembly parameter registry。它只接受 5 个 meter-bounded inner-profile controls，拒绝 raw point/JSON/script/path/URL/caller GeometryProgram，并只产生 review-only proposal。当前仅 source compile PASS，工具数与商业能力状态不变。

> 2026-08-26 目录解释：工具是否注册只表示 callable surface，不表示能产出商业武器。未来新增工具必须映射到唯一生产链与 typed artifact；优先补齐 AuthoringMesh 局部编辑、High/Low correspondence、per-Part Cage/Bake、MaterialLayerGraph、FPS presentation 和 engine package，不再以工具总数作为路线 KPI。

2026-08-26 transport 补充：Formal High raw stdio 已验证默认 read-only、Ponytail preflight、write disabled typed rejection、显式 opt-in 与 missing-stage fail-closed；reservation cleanup 1/1 PASS。positive prepare/replay/drop-reopen get 仍 `NOT_RUN`。Render Worker raster attribution 仍是内部 typed operation；现有公共 `production_weapon_form_art_evidence_get` 只增加可选闭合诊断输入/输出，不新增工具，故计数保持 **515 schemas / 111 read + 83 write = 194 tools**。该请求只接受 evidence/hash identity，不接受 camera、mask、路径、URL 或脚本；真实 D1 已运行并由 semantic repair gate 阻断错误 source。

2026-08-26 目录同步：合同 manifest 为 515 schemas；MCP 已增加默认只读 `production_weapon_formal_high_get` 与显式 opt-in write `production_weapon_formal_high_prepare`，当前为 **111/83（总计 194）**。两者严格走 Ponytail preflight、closed request、1 MiB wire budget、Runtime-only writer 和 output validation。Store scoped idempotency 已完成；完整 positive/restart/cleanup、visual/human/engine/package 仍未通过。

2026-08-26 最新 source 增量：Form Stage policy 与 Formal High factory/Store/MCP/Runtime seam 已落地；当前为 **518 schemas / 111 read + 83 opt-in write = 194 tools**。public prepare/get 可调用但尚无合法 positive restart/cleanup receipt，所以目录只标 source-exposed，不标 production capability。真实 D1 未写入或晋级。

2026-08-26 Cage/Bake public seam 增量：公共面为 `production_weapon_high_low_bake_preflight_get/get/prepare`。Store 七子表和固定 Cage/Bake Worker 已 source PASS；Runtime High resolver 与 Formal High internal materializer 已完成 compile/focused 范围。当前缺的是完整 source-lineage/CAS 正向 restart fixture 与独立 Formal High public prepare/get，而非缺少另一套 binding Schema。真实 D1 因 Form 前置未通过且没有 formal positive receipt，新建 prepare 仍首先报告 `FORMAL_HIGH_STAGE_SOURCE_LINEAGE_UNAVAILABLE`，整体 `PRODUCTION_WEAPON_HIGH_LOW_BAKE_PRODUCER_UNAVAILABLE` 且零写。工具存在不等于 formal High capability、正向 Cage/Bake、Stage、视觉、人评、引擎、分发或商业 PASS。证据：`docs/evidence/mcp010f/commercial-weapon-form-stage-policy-formal-high-source-gate-20260826.json` 与 `docs/evidence/mcp010f/commercial-weapon-cage-bake-public-seam-source-gate-20260826.json`。


> 2026-08-26 最新权威 source 口径（取代下方 2026-08-25 的“最新/当前”计数）：**518 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 MCP tools**。Low quad draft 已接入 Contracts、Store、Runtime 与公共 `low_quad_draft_durable_get/prepare`，并保留 candidate-bound current Low provenance；仍为 `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`。同键 prepare replay → Runtime drop/reopen → get 当前 cohort **1/1 PASS**，六个 Low durable CAS roots 已纳入 linked/GC 判定。Hero UV 现已接入 Store、Runtime 与 MCP public `hero_uv_durable_get/prepare`，四个 Hero UV CAS roots 已纳入 linked/GC；仅为 structural/source，artist UV review 与 packaged same-cohort 仍 `NOT_RUN`，visual/human/engine/commercial 仍 `NOT_RUN/NOT_PROVEN`。以上均不推进 Stage、confirm、version 或 export；`FPS-HIGH-05=NOT_PASSED`、Stage=`camera-calibrated`、`secondary-form-approved=NOT_CREATED`、visual=`QUALITY_TARGET_NOT_MET`、`HQ360=BLOCKED_REFERENCE_COVERAGE`、proposal=`registered=false`。证据：`docs/evidence/mcp010f/commercial-weapon-hero-uv-durable-restart-source-gate-20260826.json`；`FGC-MCP010F` 是唯一 `in_progress`。

> 2026-08-25 历史快照（已由上方 2026-08-26 权威口径取代）—目录口径：**499 schemas / 28 operator entries / 107 read + 79 opt-in write = 186 tools**。新增的 Low quad draft 合同与 Hero UV Worker operation不增加 MCP 工具；公共 `native_high_durable_get/prepare` 仍是唯一新增 live source surface。High proposal未注册，Low/UV未 durable，不构成 packaged/live Commercial High/Low/UV。

## 商业级 FPS 资产能力映射（目录声明，不是能力激活）

工具/Schema/Skill 数量只是可审计的协议表面，不是商业质量 KPI。商业 Hero Weapon 的目录消费顺序必须是 `Art Direction/design language → silhouette/primary/negative space → secondary/tertiary/bevel → AuthoringMesh/edge flow → Native High → Low/Retopo → Hero UV → Cage/Bake → Material Layer/PBR → FPS/world model → LOD/collision/socket → commercial engine/performance → independent Art Director → export/restart`；前一门未通过时，目录工具只能返回诊断或 `NOT_PROVEN`，不得暗示后续门可用。

| 能力簇 | 目录可展示的证据 | 当前商业状态 |
|---|---|---|
| Design language / silhouette | `reference_*`、CameraLock、`silhouette_*`、`boundary_error_get`、九 AOV 与 candidate-bound metrics | 当前 Stage=`camera-calibrated`、CrossView=`QUALITY_TARGET_NOT_MET`、`secondary-form-approved=NOT_CREATED`；不能从工具返回推导视觉 PASS |
| Secondary / tertiary / bevel | `operator_catalog_get`、bounded panel/vent/groove/joint/bevel 与 Part/source lineage | source structural only；二三级曲面节奏、倒角密度、高光连续与艺术接受 `NOT_PROVEN` |
| Authoring topology | `authoring_mesh_*`、IdentityLineage 和 durable edit/restart receipts | split/collapse/dissolve 独立 `3/3 PASS`；general correspondence、evaluated retarget、quad/edge-flow/editor `NOT_PROVEN` |
| Native High → Low → UV → Cage/Bake | High durable MCP、candidate-bound Low provenance、Hero UV public durable get/prepare、Cage/Bake formal get/prepare 与 diagnostic metadata | High/Low/Hero UV 仍 structural/source；Cage/Bake get/prepare 已公开，fixed Worker与七记录 Store seam source PASS，但全新 prepare 在 producer 不完整时零写失败。当前 D1 无 formal receipt，旧失败 bake 指标不能晋级 |
| Material Layer / PBR | `appearance_prepare`、`material_pack_get`、`render_pass_get`、embedded texture/readback | fixed-formula/embedded PBR 仅 structural consumer evidence；商业 Layer/Mask/Wear/Microdetail `NOT_PROVEN` |
| FPS/world + engine/performance | 当前只读 Viewer/AOV/selection/explosion surface；目标为 presentation/LOD/collision/socket/engine receipts | first-person/world model、商业引擎往返和性能预算 `NOT_RUN`；Three.js/Viewer 不代替引擎验收 |
| Human/export/restart | `human_visual_review_submit` 与现有 confirm/version/export 仅在前门通过后消费 | independent Art Director=`NOT_RUN`，`PASS_HUMAN_ART_REVIEW` 不存在；export/restart 同 hash=`NOT_RUN` |

Native High 的目录声明必须保持 **source-only structural/durable slice**：Worker/GLB/Runtime CAS/Store/restart 与公共 MCP source-focused 证据通过；`packages/forgecad-skills/proposals/native-high/0.1.0` 继续 `registered=false`，不进入 `registry.json`、active Skill root 或 Runtime Skill 选择面。packaged/candidate quality 仍 pending/`NOT_RUN`；因此目录不得把 High 标为 active/PASS，也不得解锁后续商业门。

## 商业模块合同与工具暴露（目标/排队，不激活能力）

商业缺口必须先变成 closed typed contract，再变成 Runtime-owned module；工具数量、Schema 数量或 source compile 不能单独激活模块。以下是预期边界，当前未通过的模块必须返回 `CAPABILITY_UNAVAILABLE`、`NOT_PROVEN` 或 `NOT_RUN`，不得把目标名称当成 live tool：

| 目标模块 | 目标合同 / 受限 Worker | 当前状态与退出条件 |
|---|---|---|
| Authoring Mesh | `AuthoringMesh@1`、`AuthoringMeshIdentityLineage@1`；原生 half-edge/element lineage kernel | 当前为 partial structural；split/collapse/dissolve 独立 full-chain 3/3 PASS，但 general correspondence、evaluated retarget、cross-version editor、完整 undo/selection 仍 `NOT_PROVEN` |
| Native High | `HighMeshArtifact@1`、`DetailGraph@1`；Native High Worker | source durable/GLB/readback 已有边界证据；proposal `registered=false`、`FPS-HIGH-05=NOT_PASSED`，candidate-quality、packaged、视觉和人审仍 `NOT_RUN` |
| Retopology / Low | `LowMeshArtifact@1`、`RetopologyConstraintSet@1`；editable Low/Retopology Worker | current Low 仅 `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`，prepare replay/drop-reopen/get 1/1 PASS；artist edge-flow/promotion 未运行 |
| Hero UV | `HeroUvLayout@1`、`HeroUvDurable*`；Hero UV Worker | `hero_uv_durable_get/prepare` 的 Store→Runtime→MCP 与真实 replay/drop-reopen/get 1/1 PASS；仅 structural/source，artist review、packaged same-cohort、engine tangent 和 commercial 门 `NOT_RUN/NOT_PROVEN` |
| Cage / Bake | `CageArtifact@1`、`HighLowBakeReceipt@1`；Cage-Bake Worker；public `production_weapon_high_low_bake_get/prepare` | Worker、8-map/dilation、七记录 atomic Store/replay/get source PASS；formal producer unavailable，当前 D1 positive receipt=`NOT_RUN`，quality=`NOT_PASSED` |
| Surface | `MaterialLayerGraph@1`、`HeroMaterialPack@1`；Surface/Texture Worker | fixed-formula/embedded PBR 仅 structural preview；Layer/Mask/Wear/Microdetail 与同 export hash `NOT_PROVEN` |
| LOD / delivery | `HeroLodSet@1`、`CollisionSet@1`、`SocketSet@1`；LOD/Collision/Socket Worker | `NOT_RUN`；必须通过 Part/material/UV/tangent/silhouette 和 commercial-engine readback |
| Engine / human | `EngineValidationReceipt@1`、`HeroArtReviewReceipt@1`；engine harness / independent review adapter | 两者均 target/future；Unity/Unreal round-trip、独立资深武器艺术家盲审、IP 与同 export hash 均 `NOT_RUN` |

所有上述 Worker 预期由 `ForgeCadModule@1` 封装。其最小 typed manifest 必须同时绑定 `schema_refs`、`operator_refs`、`budget`、`fixture_refs`、`LICENSE`/`NOTICE` hashes、`sbom_sha256`、`provenance`、`signature`、`module_sha256`、`contract_set_sha256` 与 `worker_build_cohort_sha256`；输入/输出和 CAS lineage 另带 `input_sha256`/`output_sha256`。模块不得声明 network、dynamic_plugin、script 或 direct_db/cas_write capability。没有同 cohort 正/负 fixture、资源预算、许可证/SBOM/provenance、签名和 hash receipt 时，模块保持 `queued`，不进入 active catalog、Skill registry 或 confirm/version/export 路径。

2026-08-25 `CQ-02-TYPED-TOPOLOGY-IDENTITY-LINEAGE`：`authoring_mesh_edit_preview → authoring_mesh_edit_prepare` 的 `split_edge / collapse_edge / dissolve_edge` proof 仍保持 source-element-only；下游 Runtime 现在只从 Store 的 exact candidate→idempotency response 恢复该 proof，并把 parent source identity 物化为 durable `AuthoringMeshIdentityLineage@1` child IDs、单调 tombstone 及 one-to-many/many-to-one relation，不接受 caller identity/proof arrays。真实 split/collapse/dissolve 已分别完成各自独立的完整持久化与 Runtime drop/reopen/get 重启链路，合计 **3/3 PASS**；Store `authoring_mesh_` **12/12**、MCP IdentityLineage **3/3**、490-schema checker与 Contracts/Store/Runtime/MCP 联合 compile PASS，工具数仍 **106 read + 78 write = 184**。general correspondence、evaluated retarget、完整 selection/undo history 与产品级 cross-version editor仍 `NOT_PROVEN`。Stage 保持 `camera-calibrated`，视觉=`QUALITY_TARGET_NOT_MET`，human/engine/distribution=`NOT_RUN`，HQ360=`BLOCKED_REFERENCE_COVERAGE`。新回执：`docs/evidence/mcp010f/authoring-mesh-typed-topology-identity-lineage-materialization-source-gate-20260825.json`；原 source-proof 回执继续作为上游证据。

2026-08-25 Native High 较早 source/transport 快照：6/6/bridge-only 已由当前 source durable/MCP receipt 取代。High Worker 仍不作为 active Skill 暴露，proposal `registered=false`，`FPS-HIGH-05=NOT_PASSED`；Low/Modifier 与质量状态不变。

2026-08-25 解释边界：工具数量、Schema 数量、active operator 数量和 Skill 数量不是商业质量 KPI。typed split/collapse/dissolve 复用既有 edit tools，独立 full-chain **3/3 PASS**；general correspondence、完整编辑历史与商业视觉未证明。High Worker 仍不在 live tool catalog 中。

2026-08-24 `FPS-FORM-EVIDENCE-04A`：真实六视图 CameraLock/FormEvidence/reviewed-structure FormArt、同 candidate CrossView 与 structural-only legacy FormQuality durable replay/restart已通过；FormArt仍 `NOT_PROVEN`。open-frame bbox不等于精确 subtract contour，negative-space=`unknown`；line-flow rows durable但匹配不预设。FormQuality@2 preflight 五项 ready，仅 CrossView hard gate 与 FormArt target observation zero-write blocked，Stage停在`camera-calibrated`。当前103 read/76 write工具和Low/Cage source bundle均不能替代formal High/Low/Cage、Hero UV、Bake或真人美术门。

2026-08-22 `CandidateAnimationVfxQuality@2` source/structural gate：Contracts **402**；Store focused **3/3**、Store full **112/112**；Runtime focused **6/6**、同源码同 cohort Runtime full **354 passed / 0 failed / 22 ignored**（376 total，115.40s）；MCP focused **4/4**、full **152 passed / 0 failed / 0 ignored**（2.49s）；contracts/runtime/store/MCP joint cargo check **PASS**。旧 `GEOMETRY_WORKER_PROTOCOL` 报告来自 stale Worker binary，已由同源码 Geometry/Render Worker 重建后清除。尚无真实 `Attachment@3 + Quality@2` public full-chain positive fixture，durable end-to-end=`NOT_RUN`/`BLOCKED_FIXTURE_CHAIN`；当前仅 `structural_only`，visual/artistic/commercial FPS=`NOT_PROVEN`，human/engine=`NOT_RUN`，不推进 stage/confirm/version/export。证据：`docs/evidence/mcp010f/candidate-animation-vfx-quality-v2-durable-source-gate-20260822.json`。

2026-08-22 `CandidateMaterialSurfaceQuality@1` public positive fixture：`Geometry → CandidateTopologyQuality@1 → AppearanceProgram@3 → TextureBuild@2 → SurfaceBake@1 → AppearanceSourceLineage@1 → CandidateMaterialSurfaceQuality@1` 的 `prepare → same-key replay → get → Runtime drop/reopen → restart get` 通过 **1/1（111.72s）**；Runtime focused **5/5**、Store full **74/74**、Contracts **350**。CAS inventory unchanged；stable `artifact_id` 与 GLB object SHA-256、MaterialPack CAS kind 精确区分，合法 UV/tangent rebuild 不计入 geometry-preservation 漂移。该结果仅为 `structural_only`；V2 animated-socket-particles 仍无完整 public `prepare → Store → restart get`，durable end-to-end=`NOT_RUN`/`BLOCKED_FIXTURE_CHAIN`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/candidate-material-surface-quality-public-positive-source-gate-20260822.json`。

最终同 cohort 修订口径：强制 build cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` 下 material focused **5/5（112.63s）**；Runtime full **310 passed / 0 failed / 20 ignored**（330 total，201.91s），且 public material fixture 明确在该 full run 内执行。此前 **111.72s** 仅为 public fixture 单测时长；两者都只支持 `structural_only`，不提升 visual/commercial、human/engine 或 stage/confirm/version/export 状态。

数值口径：当前 source 为 **518 schemas / 28 operator catalog entries / 111 read + 83 opt-in write = 194 tools**；Low quad draft 公开 `low_quad_draft_durable_get/prepare`，六个 Low durable CAS roots已纳入 linked/GC；Hero UV 公开 `hero_uv_durable_get/prepare`，四个 Hero UV CAS roots已纳入 linked/GC。这些能力仍仅 structural/source，不解锁 artist、packaged、visual、human、engine、commercial 或 confirm/version/export。本文其余较小数值均只作 historical prior slice 保留。

2026-08-22 `FictionalEnergyVfxAnimatedSocketParticlesSequence@2` 双候选 source slice：Contracts **350**；Store V2 focused **2/2**、Store full **74/74**；Runtime V2 仅低层 focused **6/6**、cargo check **PASS**；MCP V2 **3/3**；同 cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` Runtime full **309 passed / 0 failed / 20 ignored**（191.06s）、MCP full **128 passed / 0 failed / 0 ignored**（1.93s），这些是全量回归，不是 V2 public `prepare → Store → restart get` 正向 fixture。V1/V2 隔离；V2 仅证明 1..16 frame、geometry/appearance 双 candidate/delivery/AnchorSet bridge 以及 Store FK/reachability/idempotence/conflict/rollback 的结构面。完整双候选 public Runtime `prepare → Store → restart get` 正向 fixture 尚不存在，durable end-to-end=`NOT_RUN` / `BLOCKED_FIXTURE_CHAIN`，不能声称正向 durable。该 slice 为 `structural_only`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/fictional-energy-vfx-animated-socket-particles-v2-dual-candidate-source-gate-20260822.json`。

2026-08-20 Operator Catalog 新增 active product-owned `forgecad.geometry.bevel@2`，当前为 26/26 active；MCP 工具数不变。它只接受 direct `authoring-mesh@1`、单个 stable source edge 与 closed convex valence-3 P0 grammar，不接收 Python、脚本、路径、URL、网络或动态插件，也不进入 Modifier/Agentic/Repair 或冒充成品视觉质量。此前 `energy-core@1` slice 的 25/25 是历史计数。

2026-08-20 新增的 `viewer_provenance_graph` 是 Tauri Viewer 专用只读命令，不进入 MCP tool manifest，因此工具总数不变。它要求 exact project/candidate/candidate-state/artifact，固定 64 nodes / 128 edges / 1 MiB，完整或失败地显示 Geometry、Operator DAG、Artifact/readback/quality 与可验证的 Visual/AOV/MechanicalAnimation 分支；不派生质量、不执行 Blender/Python、不触发 Runtime 写入，并明确列出尚无 dedicated durable history 的分支。

2026-08-20 `viewer_mechanical_animation_frame_preview` 同样是 Tauri Viewer 专用只读命令，不进入 MCP tool manifest。它构造 closed `MechanicalAnimationClipPreviewRequest@1` 并调用既有 Runtime read surface；Viewer 只接收 scheduled single tick、same-cohort 双 Worker exact replay 和 transient artifact hash 完整绑定的 rigid Part delta。它不新增 MCP 工具、动画写入、自动播放、Armature/skin 或 Blender/Python runtime。

2026-08-19 工具面新增显式 opt-in write `authoring_mesh_edit_prepare`。MCP 只转发 closed request；Runtime/Store 独占 edit 重放、CAS 物化、strict evidence、原子 reviewable candidate/Job/idempotency 写入。工具 metadata 固定 `requiresConfirmation=true`，不接受 Python/BMesh/plugin payload，也不 confirm/version/export。

2026-08-19 historical Render Evidence Replay slice 新增默认只读 `render_evidence_replay_get`，该 slice 的 source manifest 当时为 50 read + 34 write = 84；现行口径见本文顶部。MCP 仅转发 closed nested integrity request 并执行整个 response 1 MiB Gate；Runtime 独占 strict GLB readback、actual fixed Render Worker 两次重放、同 cohort/profile 和九 AOV byte/pixel exact 验证。该工具不接受 PNG/GLB bytes、path、URL、script、engine 或动态插件，不写产品状态。

2026-08-19 historical 工具面新增默认只读 `mechanical_pose_geometry_preview`，source manifest 当时为 49 read + 34 write = 83；该历史阶段后续曾到 54+36=90，现行总量见本文顶部 110+82=192。MCP 只转发 closed request；Runtime 执行 exact cohort/rest/action 校验、per-Part delta lowering、transient fixed-Worker compile 与 strict readback，不产生 CAS/SQLite/candidate/version 写入，也不接受 Python、Blender runtime、路径、URL 或动态插件。

2026-08-19 historical 工具面 slice 新增默认只读 `subdivision_artifact_lineage_sidecar_get` 与显式 opt-in write `subdivision_artifact_lineage_prepare`，source manifest 为 48 read + 34 write = 82。MCP 仍是薄适配器；CAS/SQLite sidecar/link 只由 Runtime 写，getter 不懒写，不接受脚本、路径、URL、program bytes 或 Blender runtime。

2026-08-19 historical 工具面 slice 新增默认只读 `subdivision_artifact_lineage_get`，source manifest 为 47 read + 33 opt-in write = 80。它不接受 program 或 GLB bytes 作为调用方真值，而是由 Runtime 从 durable evidence/CAS 读取 exact program/artifact/readback，完成 strict revalidation 和 fixed-Worker full-GLB byte replay，再返回 direct SubD primitive-local triangle mapping；不新增 write tool、Operator、Skill、Blender/OpenSubdiv/Python 依赖或持久 sidecar。

2026-08-19 historical Subdivision root-lineage tool slice（当时 168/79 cohort）：新增默认只读 `subdivision_topology_lineage_preview`，当时 source manifest 为 46 read + 33 opt-in write = 79；现行口径见本文顶部。它只调用 allowlist fixed Geometry Worker，并由 Runtime 独立重验 control root → evaluated quad topology 映射；没有下载或安装 Blender addon、OpenSubdiv library、Python package、动态插件或外部 MCP。结果不持久化、不绑定 artifact/GLB，不提供 corner/per-level child path/influence weights，也不改变视觉质量。

2026-08-19 historical crease-aware slice 工具数量不变：现有只读 `geometry_program_hash` 增加 crease request branch，existing write `geometry_prepare` 执行新 active `subd-cage@2`。`hard-surface-detail@0.2.0` Skill/operator lock 已绑定该 product-owned operator；没有下载或安装 Blender addon、OpenSubdiv library、Python package、动态插件或新 MCP tool。Modifier Stack v1 仍只允许原 base set，不把 crease 隐式改造成 modifier。

2026-08-19 historical Boolean Operand Lineage slice 当时工具面为 45 read + 33 opt-in write = 78，OperatorCatalog 为 19/19 active，合同为 164 schemas。新增默认只读 `boolean_operand_lineage_preview`；它调用已有 allowlist fixed Geometry Worker，Runtime 独立重算 Boolean program 语义，未增加网络、脚本、Provider、Blender 插件或依赖。receipt：`docs/evidence/mcp010f/blender-boolean-operand-lineage-source-gate-20260819.json`。

2026-08-19 historical Render Evidence Integrity slice 当时工具面为 44 read + 33 opt-in write = 77，OperatorCatalog 为 19/19 active，合同为 162 schemas。新增默认只读 `render_evidence_integrity_get`；它只消费 Runtime/CAS 已有的 candidate-bound ArtifactReadback、camera、RenderSet、comparison、quality、AOV/mask，不运行 Blender/Cycles/EEVEE、脚本、插件、URL 或网络，也不写产品状态。

2026-08-19 historical Mechanical Pose Sequence slice 当时工具面为 43 read + 33 opt-in write = 76，OperatorCatalog 为 19/19 active，合同为 160 schemas。现有只读 `mechanical_pose_evaluate` 新增 `MechanicalPoseSequencePreviewRequest@1` branch，最多接受 16 个严格递增 tick；未新增 executable plugin、Skill、tool、operator、Worker 或 dependency。receipt：`docs/evidence/mcp010f/blender-mechanical-pose-sequence-preview-source-gate-20260819.json`。

2026-08-18 historical Parametric Group v2：该 slice 当时为 158 schemas。现有只读 `geometry_program_hash` 新增 `ParametricDesignKitRequest@2` branch，只允许 rounded-box、mirrored-box、arrayed-cylinder 三个编译内置 group template；未新增 executable plugin、Skill、tool、operator 或 dependency。

2026-08-18 historical Mechanical pose：新增默认只读 `mechanical_pose_evaluate`；该 slice 当时为 156 schemas、19 active operators、43 read + 33 opt-in write = 76 tools。工具要求 candidate/artifact/readback/program/catalog/config 与 RestFrame/Part/source-node exact binding，返回有界 local/world TRS read projection；不调用 Worker、不 materialize 几何、不写状态，也不提供 Armature/skin/完整 animation。

2026-08-18 historical RenderProfile/AOV lineage：未新增 MCP tool；`RenderSet@2`、Render Worker 与 Runtime 两个 producer 绑定同一 closed `RenderProfile@1` 及 canonical/AOV/color-pipeline/ID-palette hashes。该 slice 当时为 152 schemas、19 active operators、42 read + 33 opt-in write = 75 tools；该固定 CPU software profile 不提供 Blender/Cycles/EEVEE/OCIO/GPU/EXR parity，也不改变视觉质量事实。

2026-08-18 historical Modifier evaluation v2：未新增 MCP tool；`geometry_program_hash` 的第四个 closed 分支接受 `GeometryModifierEvaluationRequest@2`，返回 canonical previous-comparison signature、dirty reason 与 deterministic `initial-miss/reusable/invalidated` 判定。它不编译、不创建持久 mesh cache、不写 candidate/CAS/SQLite/version。该 slice 当时为 149 schemas、19 active operators、42 read + 33 opt-in write = 75 tools；后续仍须显式调用 `geometry_prepare` 才进入真实 Worker compile/readback/quality/approval 链。

2026-08-18 historical Bevel/Normal v1：未新增 MCP tool；现有 `geometry_program_hash` 的 Modifier Stack 分支可 lowering direct solid box 的首个 `bevel@1`，以及其后的 corner-domain `normal-policy@1`。该 slice 当时为 146 schemas、19 active operators、42 read + 33 opt-in write = 75 tools。Worker 真实执行、strict readback 与负向 Gate PASS；任意网格、package/live/视觉仍未通过。

2026-08-18 historical TopologySnapshot v1：新增默认只读 `topology_snapshot_get`，要求先绑定 `ArtifactReadback@2` 的 project/candidate/artifact/readback/program/catalog/config/policy hash，按 Part 返回最多 512 faces 的完整 evaluated GLB V/E/F/C 与 corner normal/UV/tangent；超限不截断，不写 Store/CAS/candidate/version。该 slice 当时为 146 schemas、42 read + 33 opt-in write = 75 tools；ID 不跨 artifact 稳定，source lineage 只到 operator node。

2026-08-18 Modifier Stack v1：`geometry_program_hash` 新增第三个只读分支，可把 closed、hash-bound、有序 transform/mirror/array stack lowering 为 `GeometryProgram@2` 和逐 stage evaluation trace；不新增工具数量，不写 Runtime 状态。receipt：`docs/evidence/mcp010f/blender-modifier-stack-source-gate-20260818.json`。

历史快照版本：2026-08-13（其中的“当前”计数只描述当时 cohort；现行真值以本文顶部的 515 schemas / 28 operator entries / 110 read + 82 opt-in write / 192 tools 为准）
2026-08-17 Reference Visual Structure：现有 `reference_mask_prepare`/`reference_mask_refine_prepare` 可选接受中性 `visual_structure` draft；Runtime 补齐 policy/review/canonical hash 并随 `SilhouetteTarget@1` 进入 CAS，`silhouette_target_get` 原路回读。没有新增工具；该 slice 当时仍为 41 read + 33 opt-in write = 74；该注释不会直接创建 candidate 或解锁 detail/PBR。
2026-08-17 PDK v0：`geometry_program_hash` 仍是默认只读工具，但现在可接受 `ParametricDesignKitRequest@1` 并返回六类 typed macro 的 `ParametricDesignKitProgram@1`；这是 Runtime-owned structural authoring aid，未新增 write tool、Skill 执行器或外部插件加载。receipt：`docs/evidence/mcp010f/parametric-design-kit-v0-source-gate-20260817.json`。
历史 Stage 0 覆盖（2026-08-17）：138 contracts、41 read + 33 opt-in write = 74 tools；新增 `fictional-energy-rifle-profile` source-only authoring contracts 与 `repair_intent_run_prepare`，Profile 不新增 write tool、只执行 CAS-bound bounded run 并产出 staged candidate，`repair_apply_prepare`/confirm 仍独立且未完成。
状态：MVP 功能核心目录；该历史段源码为 138 个 contracts、41 read + 33 opt-in write = 74 个工具、12 个 Skill（必须先读 `ponytail-preflight@0.1.0`，以及历史 Bundle + `primitive-blockout@0.2.0`、`hard-surface-detail@0.2.0`、`uv-pbr@0.2.0`）；唯一 `in_progress` 为 `FGC-MCP010F`。MCP010C source-focused fixed renderer/九 AOV/reference compare/typed visual review、MCP010D hard-surface Operator/Skill、MCP010E 离线 AssetPack/UV/PBR/MikkTSpace、MCP010F Viewer 与 contour-first Runtime target/Rig/SDF/Part/candidate compare source slice（含 `CameraCalibrationRef@1` 和 `silhouette_part_error_get`）已通过各自范围；新增 Fictional Energy Rifle Profile/Plan 仅提供 nonfunctional structural authoring aid，不代表视觉通过；新增 `primary_form_repair_prepare` 将一次 Primary Form fit→compile→readback→render→compare 收口为 Runtime-owned staged prepare/evaluate；新增 `primary_form_repair_job_prepare` 将可能超过单次 IPC deadline 的长搜索异步化，并由 `job_get`/`job_events_read`/`job_result_get` 回读终态 CAS 结果；新增 `RuntimeJobResult@1` 约束该 CAS 结果外层 envelope；新增 `design_action_run_prepare`/`design_action_run_get` 的窄范围单 Part `primary-form` action-run/readback 与 `repair_intent_run_prepare` 的 CAS-bound staged run 也已通过 focused source/package transport Gate。Agentic observe/plan projection 与 durable DesignSession/Checkpoint/RepairIntent prepare/readback 也有隔离 source/transport/restart receipt，真实 Runtime 的嵌套只读 projection producer/consumer conformance 已通过独立回执。packaged Viewer 也已有 CLI read-model、原生窗口和核心控件 smoke，但同一 provisional observation 的 package binding、PBR likeness、正式 VoiceOver、真人评审和 360 仍 `NOT_RUN/BLOCKED`；durable/reference/DesignSpec 完整 producer、通用单动作 orchestrator 和 Repair 应用仍未完成。

本文是 Luna 执行 Goal 时的“能调用什么、何时调用、什么不能声称”的单一索引。它不是新的运行时配置，也不允许绕过 MCP 合同。工具实现仍以 Rust source 和 JSON Schema 为权威；本文只提供可读的路线图和验收边界。

Stage 0 机器真值读取 `docs/evidence/mcp010f/current-benchmark-truth.json`：attempt35 只是 provisional retained observation，候选状态是 `QUALITY_TARGET_NOT_MET`，证据完整性是 `INCOMPLETE_TRUTH_BINDING`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，fit/compare camera 为 `MISMATCH`，packaged Viewer 为不同 cohort/artifact，尚未绑定该 observation。工具或 Viewer 已实现不等于这些缺口已通过，也不能提升 human/PBR/export-restart/360 状态。


<!-- forgecad-reference-source: input=ENV_AUTHORIZED_PNG original_sha256=1964704a62ed7a841b4d49c370b8d46f4626e201daad29092a9c39a40b4c4109 intake=PASS_SOURCE_SIX_REFERENCE_EVIDENCE_CAS views=6 worker=PASS_SAME_COHORT_SIX_FIXED_VIEWS target=USER_REFINED_USER_CONFIRMED_REVIEWED_STRUCTURE user_confirmed_crop=PASS_USER_CONFIRMED_SEVEN_CROPS contour=PASS_USER_CONFIRMED_SIX_IDENTITY_CONTOURS negative_space=BOUNDING_REGIONS_CONFIRMED_EXACT_SUBTRACT_UNKNOWN line_flow=EXPECTED_ROWS_DURABLE_MATCH_NOT_PROVEN camera_lock_fixture=PASS_REAL_DURABLE_REPLAY_RESTART form_art_fixture=PASS_REAL_DURABLE_NOT_PROVEN form_quality_v2_fixture=BLOCKED_ZERO_WRITE_MISSING_LEGACY_CROSS_VIEW secondary_form_approved=NOT_CREATED fixture=PASS_REAL_1_OF_1_108.07S -->

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

- 当前源码的默认连接暴露 111 个只读工具；只有 authenticated IPC、Runtime handoff 和 `FORGECAD_MCP_ENABLE_MCP004_WRITES=1` 同时满足时，才暴露完整 194 个工具（111 read + 83 opt-in write）。Cage/Bake 公共面为 `production_weapon_high_low_bake_preflight_get`（read）、`production_weapon_high_low_bake_get`（read）和 `production_weapon_high_low_bake_prepare`（write）。preflight 只返回阻断原因；get 只读已有正式回执；prepare 在七类 typed producer 未齐时不写 Store/CAS，也不推进 Stage。工具数量不构成视觉、真人、引擎或商业质量 PASS。
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

## 2. 历史 MVP 基础只读目录（37 个；现行完整数为 110）

本节逐项表格是早期 MVP 基础目录，不是 2026-08-26 完整 manifest。新增工具以 `source-tool-manifest-summary.json` 和本文件顶部当前口径为准。

| 工具 | 用途 | 当前 MVP 证据/限制 |
|---|---|---|
| `artifact_readback_get` | 读取候选绑定的 GLB header、Part、triangle、UV/tangent、PBR readback | MCP007/008 focused PASS；不返回任意文件路径 |
| `candidate_get` | 读取 candidate、hash、Job、quality 摘要 | 只读；未确认 candidate 可回收 |
| `capabilities_get` | 读取 Runtime/MCP/Worker/Skill 能力和 limitation | 必须在写入前调用；不以空字段伪装能力 |
| `doctor` | 读取 bounded health/contract/lock 诊断 | 不启动 fixture、不 confirm、不签名 |
| `geometry_program_hash` | 校验无 hash 的 `GeometryProgram@2` draft、展开 bounded PDK，或 lowering bounded Modifier Stack，返回 Runtime/Worker-owned canonical hash | 不编译、不创建 candidate/Job、不写 SQLite/CAS；Modifier v1 支持 transform/mirror/array、direct-box bevel 与 corner normal-policy；draft 不能预填 hash |
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

## 3. 历史 MVP 基础写目录（18 个；现行完整数为 82）

本节逐项表格保留基础事务语义；现行 opt-in write 完整数为 82，不能由本节行数反推工具总量。

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

`quality_get` 现可读回 candidate-bound `QualityReport@2`；attempt35 返回 `QUALITY_TARGET_NOT_MET`，不得 confirm/export。当前工具数为 111 read + 83 opt-in write = 194；Hero UV public `hero_uv_durable_get/prepare` 与 Low candidate-bound provenance 的 source/restart receipt 仅为 structural/source，artist UV review、packaged same-cohort、visual/human/engine/commercial 仍 `NOT_RUN/NOT_PROVEN`。Mechanical pose 与 Agentic projection 都只读、可重建，durable session/checkpoint/RepairIntent 及 `repair_intent_run_prepare` 仍不替代 QualityReport 或已确认 version。Stage=`camera-calibrated`、`secondary-form-approved=NOT_CREATED`、`FPS-HIGH-05=NOT_PASSED`、`HQ360=BLOCKED_REFERENCE_COVERAGE`；无 confirm/version/export。证据：`docs/evidence/mcp010f/commercial-weapon-hero-uv-durable-restart-source-gate-20260826.json`。

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
| `hard-surface-detail@0.2.0` | MCP010D | profile-extrude/profile-loft/revolve/tube-sweep、transform/mirror/array、panel/vent-array/joint-stack/part-output/boolean | 固定 revision Manifold 的 bounded same-Part `boolean@1` 已在隔离 Worker accepted；通用 arbitrary-mesh Boolean 仍 unavailable，且该 Skill 不提供商业纹理/PBR/视觉相似度 |
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
| MCP010D | `hard-surface-detail@0.2.0`（`primitive-blockout@0.2.0` 继续 active） | 已通过 V2 Schema、真实 Operator consumer、validator/benchmark/receipt、strict readback/lineage 和同 cohort packaged D raw structural probe；bounded same-Part Manifold `boolean@1` 已 accepted，通用 mesh Boolean与视觉门仍 `NOT_RUN/unavailable` |
| MCP010E | `uv-pbr@0.2.0`、`render-evidence@0.2.0`、`reference-compare@0.2.0` + `forgecad-hard-surface-robot@1.0.0` | 离线 AssetPack、512px UV atlas、固定 `mikktspace@0.3.0`、嵌入式纹理/PBR/Render producer 和逐资产 provenance |

其他历史 Skill 保持 `0.1.0`，其中未实现 Operator 继续返回 `partial/unavailable`。`primitive-blockout@0.2.0`、`hard-surface-detail@0.2.0` 与 `uv-pbr@0.2.0` 是当前 active 的 V2 Skills；它们不是插件市场或任意执行插件，而是 Runtime 预注册 Operator 的声明式调用说明。AssetPack 仍是独立资产合同；缺少 producer/operator/asset/benchmark 时不得把 MCP010E Bundle 标为 active。

## 6. GitHub/外部工具决策

本文引用 GitHub 不自动表示已安装。当前仅有固定 revision、bounded same-Part Manifold `boolean@1` 与 `mikktspace@0.3.0` tangent Worker 是受限 `accepted` 产品切片；它们不等于通用 3D compiler、商业 UV/PBR 或 renderer。其余候选均未采用。

| 项目 | 状态 | 允许的下一步 | 禁止行为 |
|---|---|---|---|
| `image-rs/image` | approved-for-evaluation | 隔离图片 decoder benchmark | 未固定 revision 就改 lockfile |
| `gltf-rs/gltf` | approved-for-evaluation | GLB strict readback benchmark | 接受外部 URI/任意 buffer |
| `elalish/manifold` | **accepted restricted** | 固定 revision、隔离 Worker 中的 bounded same-Part union/difference/intersection；继续做通用约束研究 | 扩大为任意 mesh、动态插件或第二 Runtime 真值 |
| `jpcy/xatlas` | research-authorized / not adopted | UV seam/overlap/determinism benchmark | 未审计就替换 product-owned UV |
| `gltf-rs/mikktspace@0.3.0` | **accepted restricted** | 固定 tangent Worker 与 handedness/determinism receipt | 漂移时静默改变 PBR，或把 tangent PASS 写成商业 UV/PBR PASS |
| `KhronosGroup/glTF-Validator` | research-authorized / not adopted | 隔离 GLB validator receipt | 用外部报告替代 Runtime strict readback 或 EngineValidation |
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
## 2026-08-26 CameraLock child tools

- `production_camera_lock_registration_lineage_get`：默认可见的只读 exact-scope/restart-verified lookup。
- `production_camera_lock_registration_lineage_prepare`：只在显式 write opt-in 可见；接受用户 rear-three-quarter rotation 与审批原语，不接受 caller-authored program/ordering/orientation/RigV2 对象或输出 hash。

<!-- forgecad-stage0: schemas=658 schema_set_sha256=29784beef684ae4334bfc2983f19fec25694c632ed11e0840bd12b0e9838f0f1 read_tools=131 write_tools=95 total_tools=226 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260815-b37-complete-auto-v3.json latest_completed=real-codex-cli-current-20260815-b37-complete-auto-v3.json -->
