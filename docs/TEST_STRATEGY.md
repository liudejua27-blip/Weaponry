# ForgeCAD 测试策略

> 2026-08-30 final combined architecture gate：Authoring、Evaluation、Surface、Presentation、Delivery 五域均必须走 direct typed Router/service；物理抽取仍 partial。Runtime/Store/MCP root 为 `52,542 / 79,841 / 1,081` 行，MCP `agentic_write_tools.rs=22,800`，Runtime root modules=`92`。Delivery active profile 为 **11 operations = 4 read / 7 write**，中央 9 个 capability mapping 为 `Partial`，仅 `version_diff` request schema closed（1/11）；Store 新增 `GameAssetDeliveryLinkRecord` repository，ApprovalLifecycle、socket/anchor、ReadModel/QualityEvidence 与其余 Presentation repository 仍未抽取。
> 当前源码元数据：`schema_count=658`、`schema_set_sha256=29784beef684ae4334bfc2983f19fec25694c632ed11e0840bd12b0e9838f0f1`、`runtime_source_sha256=085714f60445ed831809564d5324424aed9a734f7dec3c782a90876fa1c5d708`、`truth_canonical_sha256=6f90c5fcb2fb2218b04c871d260964623729da6ba4adf9b7cfe1d5082c154cc3`；source-only compatibility summary `cohort=null`、`131/95/226`、SHA-256=`1eb6cf5125e4d72aa2e8eef0139ff11de8c69b615d47cb66f70b666fb83377ca`。
> active request Schema=`125/125`，blocked=0，Runtime fallback=0；226 legacy registry 已 feature 隔离但大量 compatibility handler 仍编译。最终 architecture-fast receipt：cohort=`265914b6699d101eb69030947c2419e26e7a99ceef52a63a3c834989af88f28c`，`87 passed / 0 failed / 0 ignored`，`182s≤900s`，`source_drift=false`，local SHA-256=`6487663b3aed0a0c80a63ebad7ff6c344f1fd0ccc283f4a52f7ed3e703fc74f8`。本轮未重跑 full Runtime qualification，前一完整 `554 passed / 0 failed / 37 ignored` 基线保留。
> 本轮无用户数据变更、无视觉/商业 promotion；没有提升 High→Low→UV→Bake、材质、FPS、引擎、人审或商业质量。不可变历史 receipts 未改写；mutable Stage0 current-source truth 已按最终源码重建。下一原子严格为 `WPN-ARCH-MCP-SPLIT-001 → WPN-ARCH-RETIRE-001`。

> 2026-08-30 `WPN-ARCH-COMPAT-001`：兼容隔离必须由 fresh Cargo dep-info 而非 feature 名称证明。
> 当前默认 MCP 编译 6 个本 crate 源文件，显式 compat 编译 39 个；默认/兼容单测为 22/22、230/230，
> 历史 manifest 131/95/226。默认 profile 另强制报告 active=125、closed request schema=12、
> Runtime-validated=113、closure=`PARTIAL`。下一 Gate 必须先将 active request closure 提升为 125/125，
> 且不得让 legacy handlers 回到默认编译图；这仍不替代 Runtime full、视觉、人审或商业质量测试。

> **Weaponry P0 override (2026-08-29):** 本文只有在与 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md` 和 ADR-0029 一致时才具有当前执行权。ForgeCAD 在本文中解释为 Weaponry 的 Rust Runtime lineage；当前唯一产品主线是由 Codex 生成、修改、验证并交付高质量穿越火线非功能性游戏武器。通用 3D、机器人和原创科幻示例仅作 fixture/历史能力，不得抢占本月主线。 本文中所有 2026-08-28 及更早的“当前”“下一原子”“唯一 `in_progress`”和工具/Schema 数量语句均按历史 cohort 解释，不得覆盖 `WPN-*` successor queue。

> 2026-08-30 架构回归分层：`npm run runtime:architecture-fast` 是 fresh-target 四身份快速门；前序 Surface
> 快照为 53 passed / 0 failed / 0 ignored / 194s，作为历史 receipt 保留。最终 combined architecture-fast 为
> `87 passed / 0 failed / 0 ignored / 182s≤900s`，same-cohort=`265914b6699d101eb69030947c2419e26e7a99ceef52a63a3c834989af88f28c`；默认 `npm run runtime:full` 继续执行
> 554/0/37 的高成本 Runtime 枚举。37 ignored 的源码 marker 与主分类由
> `forgecad-runtime/ignored-tests.json` 和全源码树 checker 闭合，但 current-cohort execution 为 0，
> 状态是 `NOT_PROVEN`。快速门不得替代 ignored、animation/GLB/2K、视觉、历史数据库或真人/引擎资格门。

> `WPN-ARCH-SURFACE-001` focused gate：Surface active profile 必须精确为 15（8 read / 7 write），默认调用经
> direct typed `surface_service`，两个 `production_weapon_retopology_cage_source_bundle_*` 只能作为
> compatibility alias；Contract capability `formal_high_low_cage_bake` 必须保留 `Partial` mapping。验证应覆盖
> profile operation set、中央 map→Runtime service→Store record/repository→MCP façade 的唯一归属、Surface router
> unknown/out-of-bound fail-closed、borrowed `SurfaceRepository` 不复制 connection/migration/CAS owner、formal
> bake exact replay/readback 和 producer-unavailable 零写路径，以及五域 direct typed service 的 routing 与仍 partial 的物理抽取。
> 最终 combined receipt 为 `87/0/0`、182s，local SHA-256=`6487663b3aed0a0c80a63ebad7ff6c344f1fd0ccc283f4a52f7ed3e703fc74f8`；
> active schema closure 为 125/125、blocked=0、Runtime fallback=0。该 gate
> 不产生视觉或商业 promotion，不触碰用户数据，历史 receipts 不改写。

### WPN-ARCH-DELIVERY-001 / combined five-domain architecture matrix

| 范围 | 必测 | 当前证据 |
|---|---|---|
| Five-domain Router | Authoring/Evaluation/Surface/Presentation/Delivery 的 direct typed Router/service、domain mismatch 与无 service→legacy re-entry | 五域 direct typed 接线；物理抽取仍 partial |
| Delivery Contract mapping | 11 operations（4 read / 7 write）、9 capability mapping、唯一 owner 与 fail-closed dispatch | 9 mappings=`Partial`；`version_diff` closed，Delivery request closure=`1/11` |
| Delivery Store seam | borrowed `DeliveryRepository<'store>`；`GameAssetDeliveryLinkRecord` record/get/list/commit；单一 migration/CAS owner | source seam PASS；ApprovalLifecycle、socket/anchor 与其余 records 未抽取 |
| MCP compatibility | active request schema、legacy registry 与默认 Action Space 隔离 | active schema=`125/125`、blocked=0、Runtime fallback=0；legacy=`131 read + 95 write = 226`，feature isolated |
| Fast aggregate | fresh four-worker identities、source drift、预算和 focused architecture tests | cohort=`265914b6699d101eb69030947c2419e26e7a99ceef52a63a3c834989af88f28c`；`87/0/0`、182s、`source_drift=false` |

该矩阵只证明架构 source/fast regression；本轮未重跑 full Runtime qualification，前一完整 `554/0/37` 基线保留。它不提升
High→Low→UV→Bake、视觉、引擎、人审或商业质量，且不改写 Stage0 truth/hash 或历史 evidence。

## Weaponry acceptance test pyramid

1. Kernel：topology invariants、generated-ID references、atomic rollback、deterministic journal；
2. Evaluation：modifier order、dirty closure、provider determinism、original/evaluated separation；
3. Game-ready：High/Low correspondence、quad editability、UV/seam/stretch、Cage/ray diagnostics、PBR channels；
4. Presentation：first-person/inspect/ADS occlusion、socket/clip/LOD/collision；
5. Visual：固定视图/AOV/reference compare，禁止相机补偿；
6. Delivery：target-engine import/re-export/readback；
7. Acceptance：授权 CrossFire weapon 与 original control 的独立武器美术人审。

每层均绑定同一 candidate lineage；后一层不能覆盖前一层 FAIL。

> 2026-08-28 `FPS-FORM-04BE-L` 只运行用户授权修改所需的 focused 验证：合同检查、Runtime/MCP/Geometry/Render Worker 同 cohort build、4 个真实 `receiver-upper` profile strict GLB、4×54 AOV、CrossView/FormArt 和 restart exact。4 个候选质量失败原样入账，没有运行宽泛测试或放宽选择门。

> 2026-08-28 `FPS-FORM-04BE-J/K` 继续只运行直接决定产品真值的 focused 验证：Runtime/MCP compile、合同枚举、8 个真实 registered profile 的 strict GLB readback、8×六视图×9 AOV、Runtime restart exact readback，以及父节点完整参数/inputs fail-closed。没有用宽泛测试掩盖结果；8 个候选视觉失败均原样入账并拒绝晋级。

> 2026-08-28 `FPS-FORM-04BE-I` 只运行与产品判定直接相关的 focused 验证：Runtime/MCP/Geometry/Render Worker compile，583-schema 合同检查，4 个真实 registered profile 的 strict GLB readback，4×六视图×9 AOV，以及 Runtime restart 后 proposal/evidence exact hash 回读。未运行无关宽泛测试；四个候选的质量失败被如实保留，不用结构 PASS 覆盖视觉失败。

> 2026-08-28 `FPS-FORM-04BE-H` 仅运行高价值 focused 验证：583-schema 合同检查、Runtime/MCP/Geometry/Render Worker compile、compiled tool manifest、mandatory preflight、真实 D1 双 Runtime 会话 canonical equality、SQLite/CAS 零写与 Stage0 真值门。未为追求绿色而执行无关宽泛测试；证据只证明 typed plan，不证明几何修复或视觉质量。

> 2026-08-28 `FPS-FORM-04BE-G` 只运行高价值 focused 验证：581-schema 合同检查、Runtime/MCP/Render Worker compile、同 cohort 默认只读工具清单、每次 Runtime session 的 mandatory Ponytail preflight、真实 D1 两次 GET/restart exact canonical equality、SQLite 全业务表与 CAS inventory 前后比较、Stage 0 真值漂移门。结果 PASS；未新增几何候选、未重复宽泛测试，视觉仍 `QUALITY_TARGET_NOT_MET`。

> 2026-08-28 `FPS-FORM-04BE-D` 只运行高价值 focused 验证：577-schema checker、Runtime/MCP compile、同 cohort build identity、mandatory preflight、真实 D1 read-only GET、Runtime restart exact result/canonical equality、全部业务表逻辑摘要与 CAS tree 前后比较。结果 PASS，但物理 SQLite file hash 因 open/close 元数据不同而保留为 false；业务 116 tables/2520 rows 与 1639 CAS objects 未变。未执行 repair、未跑宽泛测试，质量失败不被结构证据覆盖。

> 2026-08-28 `FPS-FORM-04BE-C` 采用高价值最小验证：575-schema checker；Store/Runtime/MCP 与 Geometry/Render Worker 同 cohort build；真实 D1 fresh baseline 54 AOV；composite evidence prepare/replay；SQLite/CAS row；隔离 Runtime 重启 GET hash equality；发布文档、安全、secret、license/SBOM Gate。结构与持久化均 PASS，但测试必须保留 CrossView=`rejected-regression`、FormArt=`BLOCKED_PROPOSAL_FORM_ART_EVIDENCE` 和未创建 FormQualityV2 的真实失败，不以宽泛回归或结构 PASS 替代视觉、人评、引擎门。

> 2026-08-28 `FPS-FORM-04BE-B` 采用高价值最小验证：572-schema checker；Store/Runtime/MCP compile；3 个 exact composite delta focused tests；同 cohort Runtime/Geometry Worker build identity；真实 D1 typed prepare；SQLite/CAS row；隔离 Runtime restart GET hash equality。首轮 cohort mismatch 必须保留为 fail-closed 证据。未重复运行宽泛测试，也未用结构 PASS 替代 54-AOV/FormArt/真人/引擎门。

> 2026-08-27 `FPS-FORM-04AS`：`FormQualityV2` 新鲜基线适配器已在 Contracts、MCP、Runtime 与 Store 收口。它明确分离 source scope（当前 Stage head、CameraLock、same-cohort fresh baseline、registration lineage、RigV2）与 evaluation scope（distinct proposal candidate、proposal CrossView、proposal-side Part-ID/negative-space/line-flow）；所有调用字段均由 Runtime 从 durable evidence 重派生并由 Store 独立回读验证，legacy 模式不得夹带 proposal scope。538-schema checker、Contracts/Store/Runtime/MCP compile、四组件 same-cohort build identity 均 PASS，source cohort=`acf10c3b…173`。这只是 source/compile gate：当前真实 D1 `candidate-9127…fdc8b` 仍为 `REJECTED_REGRESSION`，未用新 adapter 重跑，Stage=`camera-calibrated`、secondary=`NOT_CREATED`、quality=`QUALITY_TARGET_NOT_MET`，无 confirm/version/export。下一原子是基于批准相机设计新的 bounded `rear-stock` art-shape，只有 proposal evidence=`READY` 且 fresh FormQualityV2 真实运行通过后才允许推进 Stage。证据：`docs/evidence/mcp010f/production-weapon-form-quality-v2-fresh-baseline-adapter-source-gate-04as-20260827.json`。

> 2026-08-27 `FPS-FORM-04AR` 当前证据不是合成测试：四组件 same-cohort build 后，先在两个隔离 D1 副本暴露并修复 fresh receipt Store closure，再在全新副本完成 baseline→620×680 prepare；随后真实 D1 完成 pre-write backup、lineage/baseline restart、实际 repair、Store readback、Runtime restart与 SQLite integrity。结果明确保留视觉失败，未把 structural/durable PASS 写成 secondary 或商业质量 PASS。

> 2026-08-27 `FPS-FORM-04AL` 当前增量：Runtime-owned durable fresh six-view baseline producer 已接通合同、Store、Runtime 与 MCP `prepare/get`；每个视图绑定 approved registration lineage / RigV2、fresh same-cohort 512×512 九 AOV、camera/mask/compare/quality 与完整 CAS reachability，并以单事务持久化。精确状态为 `PASS_SOURCE_COMPILE_DURABLE_PRODUCER_NOT_RUN_REAL_D1`；真实 D1、orientation approval、fresh baseline、notch、secondary、Stage/confirm/version/export 均未执行。当前公共面 **538 schemas / 118 read + 88 opt-in write = 206 tools**，视觉仍 `QUALITY_TARGET_NOT_MET`。

> 2026-08-27 `04AK` 高价值验证：compile/build、533-schema contract checker、Stage0 truth 和 source-hash receipt 为本轮充分基线；重点验证 preflight 零写、固定六视图、lineage/RigV2/scope fail closed，以及 proposal CAS cleanup。materializer 和真实 D1 lineage 尚未运行，不用重复大测试掩盖这些明确缺口。

> 2026-08-27 `04AJ` 高价值验证：只运行 compile/build、合同/Stage0 truth 与真实 D1 isolated zero-write proposal。必须验证 620/680 typed canonical、proposal/operation/proposed-child identity、固定六视图名称、CAS `1509→1509`、lineage `0→0`，以及跨 cohort prepare 在 durable 写入前 fail closed；不以重复大测试代替真实边界。实际 notch 后另按 fresh same-cohort 六视图门验收。

> 2026-08-26 `04AI` 高价值验证：compile/contracts 后只跑真实 D1 隔离只读调用。验证请求无 orbit、Runtime 派生 `180°`、camera/canonical/proof hash 精确、screen order 与 world `+Y` upright PASS、CAS `1509→1509`、lineage `0→0`、无 Worker/Stage/confirm/version/export；未增加重复大测试。

> 2026-08-26 `04AH` 定向验证：本轮只要求 529-schema checker、Contracts/Runtime/MCP compile 和 truth/docs Gate；不扩写批量测试。真实验收必须在用户 orientation receipt 后执行同一 D1 prepare→get→Runtime restart，并验证最终 rear3q camera hash、投影 screen order、world `+Y` upright 及零 Stage/confirm/version/export 副作用。

> 2026-08-26 04AC 仍只做高价值纵向验证：529-schema checker、Runtime/MCP compile、1 个 OpenFrameNotch 闭合流形聚焦验证、真实 D1 prepare 和同键 Runtime restart replay。实跑证明拓扑/durable/GLB/FormArt 执行 PASS，同时以 `REJECTED_VISUAL_REGRESSION` 阻止错误候选。不用无关大测试数量代替资产证据。

> 2026-08-26 本纵切仅运行 compile-first 与真实 D1 必要重放：Contracts/Store/Runtime/MCP compile，`MoveVertices → child → one-node lower → strict readback → six-view proposal FormArt durable replay/restart readback`。没有用大量 fixture 代替艺术证据。当前门结果是 `PASS_DURABLE_TRANSPORT / REVIEWABLE_TRADEOFF / BLOCKED_PROPOSAL_FORM_ART_EVIDENCE / QUALITY_TARGET_NOT_MET`，不是 PASS_ASSET。

> 04AG 聚焦验证：真实 D1 已覆盖 CAS canonical readback、SQLite Store identity/reachability、same-key replay、Runtime drop/reopen replay/readback，以及六视图 candidate/artifact/camera/reference/AOV 精确绑定；hash 稳定。实际内容同时验证 fail-closed：Part-ID 全 observed，但 owner/open-void=false、negative-space/line-flow 含 unknown/inferred 时 eligibility=false，`secondary=NOT_CREATED`、Stage/confirm/version/export=false。tamper/conflict/GC 独立破坏性探针仍未运行，不由本轮 durable positive 冒充。

> 2026-08-26 本轮按 compile-first 最小验证：Contracts/Store/Runtime/MCP 联合 compile、High Worker compile，以及一个真实 D1 source prepare→Runtime restart→durable get。回执验证 `rear-stock` 8V/6Q、source binding 与 transform 保留；未运行与本纵切无关的大量测试。后续只为 `MoveVertices → proposal → six-view` 增加必要门，不把 fixture 数量当作商业质量。

> 2026-08-26 `04AF` 聚焦原则：不继续堆横向 fixture。本轮高价值证据是真实 D1 六视图拒绝回执与 `AuthoringMesh@2` Runtime restart exact readback；验证只保留聚焦编译、合同一致性、文档/安全门和下一纵向资产阶段的必需回归。

> 2026-08-26 本轮只运行必要的 contract checker、focused compile/link 和文档/安全 Gate；现行面为 **525 schemas / 112 read + 84 write = 196 tools**。没有用大规模测试数量替代真实 D1、视觉、引擎或人评证据。

> 商业路线的验证重心是少量高价值纵向 evidence：真实 candidate、同 hash、fresh render/bake、engine round-trip、human review。避免用大量 fixture 数量制造虚假进度；compile/source Gate 只证明代码面。质量门见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

> 2026-08-26 `FPS-FORM-04AD` 权威增量：当前合同面为 **518 schemas / 111 read + 83 opt-in write = 194 tools**。新增 `ProductionWeaponSemanticLandmarkOrdering@1` 只表达 Runtime-derived 的 3D source/subject-axis 顺序，明确 `target_landmark_arrays_present=false / metrics=NOT_PRESENT`，不得冒充 2D landmark；`ProductionWeaponAuthoredViewOrientation@1` 将诊断变换与用户方向回执分开；`RegisteredCameraRigCalibration@2` 只有绑定 promotable authored rear3q receipt 才能物化。定向 Contracts/Runtime/MCP compile 与 518-schema checker PASS。真实 D1 尚无 orientation-specific user receipt，因此保持 `BLOCKED_AUTHORED_REAR_THREE_QUARTER_ORIENTATION`、Stage=`camera-calibrated`、secondary=`NOT_CREATED`、quality=`QUALITY_TARGET_NOT_MET`，不 confirm/version/export。旧 `@1` 保持历史真值；durable 落点采用 CameraLock 的 additive child lineage，不复制/自动升级整张旧记录。

> 2026-08-26 `FPS-FORM-04AC` 最小验收：不扩展无关测试矩阵。当前只记 Runtime/MCP compile PASS。下一次昂贵运行必须同时证明：唯一改变 node=`rear-stock`、外包络/其他 nodes/PartOutputs exact unchanged；六视图 same registered cameras 下的 silhouette/boundary/bbox/centroid/landmark 全部 non-regressing；owner trio overlap=0 且 adjacency `>=32px`/`>=250 milli`；fresh FormArt lineage；zero confirm/version/export。rear3q durable authored orientation 缺失时必须返回 `BLOCKED_AUTHORED_REAR_THREE_QUARTER_ORIENTATION`。

> 2026-08-26 新增边界：Render Worker raster attribution 必须验证固定 512×512 triangle raster 与 formal Part-ID AOV pixel-center alignment、triangle/source-table hashes、source lineage/material zone/mesh/primitive、cohort 与零写 flags；FormArt get 诊断必须绑定 durable FormArt、candidate/artifact/readback、reference target、CameraLock/rig、FormEvidence view、RenderSet 与派生 masks，caller camera/mask fail closed。真实 D1 04AA 已证明 semantic mismatch 阻断，04AB 又证明 subject→registered exact replay 后唯一最高源为 rear-stock；两条证据都保留。下一 focused asset test 只允许一次 rear-stock repair 的三视图 strict owner-void、六视图 non-regression、restart/tamper 和 zero-confirm/version/export；不要扩成无关测试矩阵。V2 semantic contracts还需覆盖旧 @1 拒绝、ordering缺失/冲突、authored rotation缺失与 CAS parent tamper。

2026-08-26 Formal High 最小验收矩阵：合同 checker 与 Contracts/Runtime compile 只记 source PASS。正式 public Gate 必须依次证明合法 secondary-form-approved fixture 的首次 prepare、exact replay、same-key different-input conflict、CAS object tamper、SQLite payload tamper、失败 reservation/row cleanup、Runtime drop/reopen get、MCP preflight/opt-in/raw response，以及真实 D1 在任何 CAS/SQLite 写入前 fail closed。未完成这些窄门前不运行也不宣称长链视觉/商业套件通过。

2026-08-26 Cage/Bake 测试边界：当前 Store 七子表、固定 exact-topology Cage、8-map Bake、8-texel dilation、固定 2K launcher 与 blocker 分类只能记 source PASS。两候选 High resolver 已用真实字段完成 compile/focused PASS；正向 fixture 仍必须创建 Runtime-owned distinct derived High candidate，原子持久化既有 `ProductionWeaponHighArtifact@1` 并在 Runtime drop/reopen 后复验，再验证 High→Low→Hero UV→Cage→Bake 七记录、CAS reachability、双 Worker same-cohort replay 与严格输出 readback。该 materialization 前置未满足时，真实 D1 只允许断言 `FORMAL_HIGH_STAGE_SOURCE_LINEAGE_UNAVAILABLE`、`PRODUCTION_WEAPON_HIGH_LOW_BAKE_PRODUCER_UNAVAILABLE`、零写；不得写 formal High/Cage-Bake、Stage 或质量 PASS。


本轮最小 Gate：Contracts 515/515；FormArt attribution 的 Runtime request/ranking 2/2、MCP closed optional input 1/1、Render Core Part-ID alignment 1/1、Render Worker zero-write transport 1/1；Low MCP definition 1/1、manifest/write-opt-in/Runtime routing 1/1、Runtime policy/closed fields 2/2、Store additive table 1/1；Hero UV compiled core 4/4、Store→Runtime→MCP public `hero_uv_durable_get/prepare` 与真实 prepare→replay→drop/reopen→get **1/1 PASS**；四个 Hero CAS roots linked/GC；Runtime/MCP cargo check 与 MCP build PASS。仍未运行/未通过项必须继续单列：Form attribution real-D1 execution、artist-authored unwrap、visual、human、engine、commercial、packaged；以上仅为 structural/source pass，不推进 Stage、confirm、version 或 export。

> 2026-08-26 最新权威 source 口径（取代下方 2026-08-25 的“最新/当前”计数）：**518 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 MCP tools**。Low quad draft 已接入 Contracts、Store、Runtime 与公共 `low_quad_draft_durable_get/prepare`，但仍为 candidate-bound exact provenance 的 `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`；其重放不改变本轮 Hero UV 边界。Hero UV 的 `hero_uv_durable_get/prepare` 已完成 Store→Runtime→MCP，真实 prepare→同键重放→Runtime drop/reopen→get 为 **1/1 PASS**，四个 Hero CAS roots 已 linked/GC。该结果仅为 structural/source pass，不是 artist-authored unwrap、visual、human、engine、commercial 或 packaged pass；`FPS-HIGH-05=NOT_PASSED`、Stage=`camera-calibrated`、visual=`QUALITY_TARGET_NOT_MET`、human/engine/distribution=`NOT_RUN`、commercial=`NOT_PROVEN`、packaged acceptance=`NOT_RUN`、HQ360=`BLOCKED_REFERENCE_COVERAGE`，不推进 Stage、confirm、version 或 export。证据：`docs/evidence/mcp010f/commercial-weapon-hero-uv-durable-restart-source-gate-20260826.json`。

> 2026-08-25 历史快照（已由上方 2026-08-26 权威口径取代）—测试基线：Contracts **499/499 PASS**；High Worker **6/6 library + 7/7 transport PASS**；当前 cohort Runtime prepare→CAS/Store→drop/reopen→get **1/1 PASS**；Low retopology focused **10/10**（explicit quad **2/2**）；Hero UV **3/3**；Worker Protocol strict envelope **1/1**；Viewer typecheck/production build PASS。Low/UV durable、package、candidate visual、human、engine、distribution继续 `NOT_RUN/NOT_PROVEN`。

2026-08-25 `CQ-02-TYPED-TOPOLOGY-IDENTITY-LINEAGE`：`authoring_mesh_edit_preview → authoring_mesh_edit_prepare` 的 `split_edge / collapse_edge / dissolve_edge` proof 仍保持 source-element-only；下游 Runtime 现在只从 Store 的 exact candidate→idempotency response 恢复该 proof，并把 parent source identity 物化为 durable `AuthoringMeshIdentityLineage@1` child IDs、单调 tombstone 及 one-to-many/many-to-one relation，不接受 caller identity/proof arrays。真实 split/collapse/dissolve 已分别完成各自独立的完整持久化与 Runtime drop/reopen/get 重启链路，合计 **3/3 PASS**；Store `authoring_mesh_` **12/12**、MCP IdentityLineage **3/3**、490-schema checker与 Contracts/Store/Runtime/MCP 联合 compile PASS，工具数仍 **106 read + 78 write = 184**。general correspondence、evaluated retarget、完整 selection/undo history 与产品级 cross-version editor仍 `NOT_PROVEN`。Stage 保持 `camera-calibrated`，视觉=`QUALITY_TARGET_NOT_MET`，human/engine/distribution=`NOT_RUN`，HQ360=`BLOCKED_REFERENCE_COVERAGE`。新回执：`docs/evidence/mcp010f/authoring-mesh-typed-topology-identity-lineage-materialization-source-gate-20260825.json`；原 source-proof 回执继续作为上游证据。

2026-08-25 Native High 较早测试快照：3/3 + 3/3/bridge-only 已由顶部 5/5 + 7/7 + Runtime restart 1/1 + MCP source-focused 最新基线取代。package/candidate visual/human/engine/distribution仍不得记 PASS，`FPS-HIGH-05=NOT_PASSED`。

> 2026-08-25 商业质量测试总门：同一 export hash 必须分别通过 Form、Authoring Topology、High、Low/Retopo、Hero UV、Cage/Bake、Material、FPS Presentation、LOD/Collision/Socket、Commercial Engine、Independent Human Art Review 和 Restart/Export。当前 bake coverage `0.3625035285949707`、ray miss `45,386`、nearest fallback `107,063`、cross-part `3,982`、padding `0`，故 Bake/Material/Commercial Gate均未通过。AuthoringMesh typed proof→durable IdentityLineage 的 split/collapse/dissolve 独立 full-chain **3/3 PASS**，但 general correspondence、closed-manifold/ngon、完整编辑历史、evaluated retarget 及商业视觉证据仍需完成。

### 商业级 11 阶段测试矩阵（唯一顺序）

商业开发遵循“少而硬”的验证纪律：先让 Contracts/Worker/Runtime/MCP 编译通过，再立即运行一个真实 Hero candidate 的关键链。只新增能覆盖 crash/resource/determinism、hash/lineage、transaction/restart、malicious input 或商业退出门的测试；不为提高数量复制相同 happy-path fixture，也不反复运行无关全量套件代替资产审核。

每个 CQ 阶段最少只有四类证据：

1. `PASS_COMPILE`：受影响 crates/apps 能编译；
2. `PASS_BOUNDARY`：closed schema、资源上限、确定性、hash/lineage、失败零写；
3. `PASS_ASSET_STAGE`：同一真实 candidate 的 artifact/readback/固定视图/质量门；
4. `PASS_ENGINE/PASS_HUMAN`：目标引擎 packaged build 与独立 Art Director。

前两项不能提升视觉状态，第三项不能替代后两项。长时间测试只在当前原子任务的真实 Worker/engine/human gate 需要时运行一次；文档变更仅运行文档、安全、许可证和 diff Gate。

测试必须按同一 `candidate_hash → export_hash` 逐阶段运行；前一阶段失败时，后续测试只保留诊断，不得把结果写成 Stage/confirm/version/export：

1. `Art Direction/ReferenceViewSet`：验证 `WeaponArtBrief@1`、五核心视图/CameraLock、silhouette/negative-space/landmark、授权与预算；当前 CrossView=`QUALITY_TARGET_NOT_MET`、`secondary-form-approved=NOT_CREATED`、`HQ360=BLOCKED_REFERENCE_COVERAGE`。
2. `AuthoringMesh`：验证 original/evaluated、稳定 V/E/H/C/F/loop/ring/boundary、可编辑历史和 High↔Low correspondence；split/collapse/dissolve **3/3 PASS** 仅结构，商业 correspondence/editor `NOT_PROVEN`。
3. `High`：验证 DetailGraph/High artifact、support/crease/weighted normal/Subdivision、strict GLB readback；当前 source-only，`FPS-HIGH-05=NOT_PASSED`、proposal=`registered=false`。
4. `Low`：验证 artist-authored quad、hard-edge/seam/Part 边界、High↔Low correspondence、bake-ready；当前 `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`，durable replay/drop/reopen/get **1/1 PASS** 不等于商业通过。
5. `UV`：验证 2K/4K density、seam/stretch/overlap/OOB/padding、UV0/UV1、tangent/Mikk；Hero UV 7 contracts/public get/prepare **1/1 PASS**、4 CAS roots linked/GC 仍 structural/source，artist/package/engine `NOT_RUN/NOT_PROVEN`。
6. `Cage/Bake`：分别验证 exact Low topology/order Cage、8 类 maps、8-texel dilation、七记录原子 commit/replay/restart/GC、producer fail-closed 零写，以及未来 current-candidate positive receipt；现有 source seam PASS 不覆盖最后一项，正式门=`NOT_PASSED`。
7. `Material`：验证 `MaterialLayerGraph@1`、Layer/Mask/Generator/Decal/Wear/Microdetail、roughness/color-space/provenance；当前只有 **4 MaterialZones / 6 formula textures**，fixed-formula preview，commercial PBR=`NOT_PROVEN`。
8. `LOD`：验证 authored LOD0/1/2、collision/socket、误差与平台预算；commercial LOD/performance `NOT_RUN`。
9. `Viewer/animation/VFX/audio validation`：验证同 hash read model、first/third-person 相机、动画/VFX/audio/readability/accessibility；Three.js 仅结构消费，animation/VFX/audio/VoiceOver/human viewing `NOT_RUN`。
10. `Engine`：验证 Unreal 或 Unity importer/material/tangent/LOD/collision/socket/animation round-trip 与预算；**Unreal/Unity 均未运行**，Three.js 不能替代引擎验收。
11. `Independent Hero Art Review`：验证独立资深艺术家盲审/修订闭合、同 hash restart/export；human=`NOT_RUN`，无 `PASS_HUMAN_ART_REVIEW`。

当前源面仍为 **518 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 MCP tools**。测试数量、Three.js readback 或旧 bake 诊断不构成商业质量 PASS；本轮只同步文档，不运行长测试。

> 2026-08-25 04Y/04Z 诊断边界：04Y 的 registration-only preflight 可单独记录唯一 identity registration，但不能升级 owner-void/FormQuality；04Z 的 station isolation 可记录方向停止，但不能升级 secondary form。聚合 ignored fixture 在输出两段 JSON 后以 exit 130 中止，因此本轮只保存 `OUTPUT_CAPTURED_BEFORE_AGGREGATE_TEST_INTERRUPTION` supplemental receipts，不记 aggregate PASS。后续验收必须从 source/triangle attribution 开始，禁止用更多盲参数扫描代替形体设计。

2026-08-25 `FPS-FORM-04H-SUBJECT-FRAME-REGISTRATION-SOURCE`：camera registration focused tests **7/7 PASS**，覆盖 canonical identity、D1-style yaw-180、内容 hash 篡改、PartOutput 篡改、混合轴、closed transform、camera rehash 和 top/side/front materialization；正确 D1 轻量测试 **1/1 PASS（0.05s）**，source object hash 精确匹配，7 sinks 前后稳定，program 不变且无 Runtime/Worker/CAS/SQLite。下一测试原子不能直接把 transient registered cameras 写进 `CameraRigCalibration@1`；必须先新增正式 lineage 表达，再分别验证 rear-three-quarter reference registration 与 strict owner-void acceptance，最后才允许昂贵 real D1 FormQuality 重跑。

2026-08-25 `FPS-FORM-04G-REGION-PART-ID-BINDING`：真实 D1 fixture **1/1 PASS（371.07s）**，并新增昂贵运行前的精确七 typed-sink 轻量断言。receipt `docs/evidence/mcp010f/production-weapon-form-region-part-binding-supplemental-20260824.json` 记录 left/right horizontal flip、rear-three-quarter vertical flip 的 baseline/trial 一致唯一候选；因缺 authored orientation/registration contract，测试结论严格为 `BLOCKED_AUTHORED_ORIENTATION_OR_REGISTRATION`、diagnostic-only、non-promoting。下一测试原子为 `FPS-FORM-04H-AUTHORED-VIEW-ORIENTATION-REGISTRATION`；不得把 permissive discovery threshold 当作 FormQuality acceptance。

2026-08-24 `FPS-FORM-PART-ID-AUDIT-04F` supplemental diagnostic：真实 D1 1/1（361.83s）链路的 Part-ID sidecar 已记录 `BLOCKED_OWNER_BINDING`；`stock-open-frame-angle=0.12` mutator 改变 `rear-stock` mask，但 left/right/rear-three-quarter owner binding 均未 ready。FormArt canonical unchanged、depth=`UNKNOWN`，baseline retained；该回执仅 evidence-only，不产生 stage/confirm/version/export，且本轮不重跑或修改 Runtime。下一测试原子为 `FPS-FORM-04G-REGION-PART-ID-BINDING`，需在同一 frozen camera/candidate/reference/Render Worker cohort 下补齐 region↔Part binding，再重新评估。

2026-08-24 FORM fixture：真实六视图 CameraLock/FormEvidence/reviewed-structure FormArt、同 candidate CrossView 与 structural-only legacy FormQuality durable prepare/replay/get/restart **1/1 PASS（324.32s）**；54 AOV、same-key replay 0 new、restart verified、retarget zero-write。测试还证明 proposal/overlay hash绑定、bbox open-frame不得冒充subtract contour；negative-space保持unknown、line-flow期望行存在但候选匹配不预设。FormQuality@2 preflight 五项 ready，仅 CrossView hard gate 与 FormArt target observation 两项零写阻断，Stage仍camera-calibrated。Store focused `production_weapon_form_quality_` **8/8** 覆盖 QualityReport@2 `min/max` 词汇与 CAS receipt 空自引用正负边界；V2 继续独立严格验证 FormArt 三类证据。后续High/Low测试必须绑定真实secondary head并补formal three-artifact/rollback，不能复用source bundle冒充Hero资产。

历史 source-intake 与七 crop/六 contour focused fixtures 分别通过 **1/1（2.50s）**、**3/3（6.95s）**，现已被同一真实用户参考的 reviewed-structure durable fixture 覆盖：六个 identity views 的 contour/visual structure 为 user-confirmed，深度保持 unknown；bbox open-frame 不伪装成 subtract polygon，negative-space 保持 unknown；line-flow expectation 已持久化但 candidate match 不预设。FormArt 与 structural-only legacy FormQuality receipts 已产生并通过 replay/restart；FormQuality@2 仍因真实 CrossView hard gate=false 与 FormArt target observation blocked 在写入前阻断，因此 `secondary-form-approved` 仍 `BLOCKED`。

2026-08-22 `CandidateMaterialSurfaceQuality@1` public positive fixture：`Geometry → CandidateTopologyQuality@1 → AppearanceProgram@3 → TextureBuild@2 → SurfaceBake@1 → AppearanceSourceLineage@1 → CandidateMaterialSurfaceQuality@1` 的 `prepare → same-key replay → get → Runtime drop/reopen → restart get` 通过 **1/1（111.72s）**；Runtime focused **5/5**、Store full **74/74**、Contracts **350**。CAS inventory unchanged；stable `artifact_id` 与 GLB object SHA-256、MaterialPack CAS kind 精确区分，合法 UV/tangent rebuild 不计入 geometry-preservation 漂移。该结果仅为 `structural_only`；V2 animated-socket-particles 仍无完整 public `prepare → Store → restart get`，durable end-to-end=`NOT_RUN`/`BLOCKED_FIXTURE_CHAIN`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/candidate-material-surface-quality-public-positive-source-gate-20260822.json`。

最终同 cohort 修订口径：强制 build cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` 下 material focused **5/5（112.63s）**；Runtime full **310 passed / 0 failed / 20 ignored**（330 total，201.91s），且 public material fixture 明确在该 full run 内执行。此前 **111.72s** 仅为 public fixture 单测时长；两者都只支持 `structural_only`，不提升 visual/commercial、human/engine 或 stage/confirm/version/export 状态。

数值口径：当前 source 为 **518 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 tools**；typed split/collapse/dissolve 3/3、High 6/6 + 7/7 + restart 1/1、Low exact provenance 与 Hero UV durable replay/drop/reopen/get **1/1** 均为各自窄门。Hero UV 仅为 structural/source pass，artist unwrap、visual、human、engine、commercial、packaged 仍未通过；Stage/quality truth 不变，且不 confirm/version/export。

2026-08-22 `FictionalEnergyVfxAnimatedSocketParticlesSequence@2` 双候选 source slice：Contracts **350**；Store V2 focused **2/2**、Store full **74/74**；Runtime V2 仅低层 focused **6/6**、cargo check **PASS**；MCP V2 **3/3**；同 cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` Runtime full **309 passed / 0 failed / 20 ignored**（191.06s）、MCP full **128 passed / 0 failed / 0 ignored**（1.93s），这些是全量回归，不是 V2 public `prepare → Store → restart get` 正向 fixture。V1/V2 隔离；V2 仅证明 1..16 frame、geometry/appearance 双 candidate/delivery/AnchorSet bridge 以及 Store FK/reachability/idempotence/conflict/rollback 的结构面。完整双候选 public Runtime `prepare → Store → restart get` 正向 fixture 尚不存在，durable end-to-end=`NOT_RUN` / `BLOCKED_FIXTURE_CHAIN`，不能声称正向 durable。该 slice 为 `structural_only`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/fictional-energy-vfx-animated-socket-particles-v2-dual-candidate-source-gate-20260822.json`。

2026-08-20 `bevel@2` focused matrix：Worker 全量 61/61 覆盖 direct authoring mesh、单稳定 edge、segments/profile/width/edge variation、16/24-triangle strict readback、solid/determinism，以及 boundary/non-manifold/multi-edge/oversize/forbidden executable fields fail-closed；Runtime 233/0/12 ignored、MCP 86/86、contracts 195、Skills 12 与 raw stdio 26/26 catalog PASS。它是 source structural matrix，不证明当前候选、visual/PBR/human/package/live/export-restart/HQ360。

2026-08-20 `energy-core@1` focused matrix：Worker 3/3 与全量 60/60 覆盖四组件、4 Parts/768 triangles、Part/source/material exact mapping、deterministic strict GLB/lineage、boundary/non-manifold/winding、负/微小非零 inner radius、solid exact-zero、relationship/unknown field/budget/hash drift；Runtime 233/0/12 ignored、MCP 86/86、contracts 195、Runtime Skill/Profile/Modifier/PDK 与 raw stdio 25/25 PASS。它是 source structural matrix，不证明当前候选、visual/PBR/human/package/live/export-restart/HQ360。

2026-08-19 candidate-bound Modifier Apply focused matrix：contracts 191；Store 24 tests 覆盖 30 秒 bounded same-key single-flight、CAS reservation/shared temporary cleanup、current-head/source/derived evidence 与最终 Part binding 二次校验、late SQL rollback；Runtime 240 tests 通过（另 12 个显式 isolated ignored gate），覆盖 source/derived same-cohort 双回放、target terminal 替换、非目标 Part/source/material/solid/triangle 保持、stale/foreign/hash/Python/reference 负门；MCP 86 tests 通过，raw stdio 另验证 identical replay、Job→CAS sidecar 与 Runtime/MCP 重启回读。这些仍只是 structural/durable 证据，不证明 visual/PBR/human/package/HQ360。

2026-08-19 Authoring Mesh Edit Prepare focused matrix：contracts 189；Store 覆盖原子 candidate/Job/event/audit/evidence/idempotency、current-head/scope、CAS kind/mime/size/reachability、精确 replay、key reuse 和失败零行；Runtime 覆盖 preview TOCTOU 重算、same-cohort Worker、strict GLB/readback/quality/evidence、reviewable-only、no version/confirm/export、临时 CAS 回滚；MCP 覆盖默认隐藏、显式 opt-in、closed nested schema、requiresConfirmation、bounded summary/structuredContent 和 forbidden Python 字段。该矩阵不证明 visual/PBR/human/package/live/HQ360。

2026-08-19 Authoring Topology/Edit Preview focused matrix：contracts 187；Runtime 覆盖 exact durable evidence canonical、bounded CAS、source V/E/Loop/Face、single direct Part、双 Worker replay、strict GLB readback、deterministic canonical hashes与 project/candidate/version/CAS no-write；负测覆盖 stale/cross-project、evidence tamper、unknown/unsorted/zero translate、unknown/interior/non-planar/concave extrude 与 executable fields。MCP 覆盖两个默认只读 closed tools、公开 prepare 字段可调用、完整 structuredContent、自哈希、1 MiB、compiled manifest 54+35=89；真实 raw stdio 在 isolated setup 的 11 Parts/622 triangles 场景完成 topology、translate、extrude，setup 写入与 read slice no-write 分开记录。该矩阵不证明 persistent mesh editor、BMesh/Python/plugin、package/live/visual/human/PBR/export-restart/HQ360。

2026-08-19 historical Render Evidence Replay focused matrix：该历史切片为 contracts 177，现行合同总数为 191；Runtime 覆盖 restart 后 exact integrity reread、current strict GLB/ArtifactReadback 对齐、actual fixed Geometry/Render Worker 同 cohort、persisted/first/repeat 两次重放的九 AOV raw PNG + decoded RGBA8 exactness、deterministic result 与 candidate/version/CAS inventory零写入；cohort unavailable fail closed。MCP 覆盖默认只读、closed nested request、preflight 后 dispatch、unknown Python field 拒绝和 integrity/replay 整响应 1 MiB hard Gate。该矩阵不证明跨平台 determinism、视觉质量、PBR、人评或 Blender renderer parity。

2026-08-19 historical Mechanical Pose Geometry Preview focused matrix：contracts 175；现行合同总数为 191；Runtime 覆盖 three-Part hierarchy、`PoseWorld × inverse(RestWorld)` derived program、fixed Worker compile、strict readback、deterministic repeat、candidate/version/CAS inventory零写入、transient hash不在 CAS、重复 source ownership和 policy negative；独立数值 test 覆盖 Quaternion 与 Worker X→Y→Z Euler 等价及 near-gimbal 拒绝。MCP 覆盖默认只读工具、closed nested request、bounded summary/完整 structuredContent、1 MiB 与 unknown Python field拒绝。该矩阵不证明 original asset rig/pivot provenance、Armature/skin/animation、package/live/Viewer/visual/human/360。

2026-08-19 Subdivision artifact-lineage sidecar focused matrix：contracts 173；Store 10/10 覆盖 candidate/node 唯一、exact request/evidence/CAS kind/mime/1 MiB、相同 link 幂等与冲突/missing CAS、Link 与 reachable 同事务、linked/reachable 拒绝回滚、仅本次 created-new temporary unlinked sidecar 的失败清理，以及 8 路同 hash 并发 put 仅一个 creator；Runtime 覆盖 explicit prepare、immutable canonical sidecar、reopen/restart getter、candidate/version no-write、不同 request、cross-candidate与 corrupt CAS rejection，并在 14×14 level2 / 5,408 triangles / 17,162 elements上验证完整 Link ≤1 MiB；MCP 覆盖默认只读 getter、显式 write opt-in、closed shared input、Link round-trip、disabled-write与工具数。filesystem CAS staging 与 SQLite 不声明跨介质原子；通用 GC 仍属 MCP011。该矩阵不证明跨版本 ID 或视觉质量。receipt：`docs/evidence/mcp010f/blender-subdivision-artifact-lineage-sidecar-source-gate-20260819.json`。

2026-08-19 Subdivision artifact-lineage focused matrix：contracts 170；Runtime 覆盖 V2 geometry prepare → durable evidence/readback、full GLB byte replay、唯一 direct source primitive、128 triangles、四个 control-quad ranges、canonical/hash、read-only state equality和 reopen/restart；错 readback hash与重哈希错误 triangle range拒绝。实际 artifact-bound 大样例为 14×14 level2、5,408 triangles、17,162 lineage elements，完整 MCP envelope 在 1 MiB 下保留至少 4 KiB 余量；16×16 只属于 topology preview，不宣称 artifact-bound 上限。MCP 覆盖 closed read-only tool、bounded summary + complete structuredContent、1 MiB和 stale binding error。该矩阵不证明 persisted sidecar、glTF V/E/C identity、跨版本稳定或视觉质量。receipt：`docs/evidence/mcp010f/blender-subdivision-artifact-lineage-source-gate-20260819.json`。

2026-08-19 Subdivision root-lineage focused matrix：Worker 覆盖 3×3 level2 的 442 elements、最大 16×16 level2 的 22,802 elements、确定性与 budget/unknown fail-closed；Runtime 覆盖独立 V/E/Q/T、control root、edge-chain、quad-range、crease-chain、lineage/canonical 重验及 candidate/version/CAS no-write；MCP 覆盖 closed envelope、bounded summary、完整 structuredContent 与整个 response 1 MiB。root 篡改、重复 evaluated edge、错误 quad root、错误 operator、0/25,001 budget 均拒绝；该矩阵不证明 artifact/GLB lineage 或视觉质量。

2026-08-19 crease-aware Subdivision focused matrix：Worker 覆盖 smooth/dart/two-edge crease/three-edge corner/boundary-junction/level-1-to-2 decay、字节确定 GLB、strict readback，以及 2×2、boundary/reversed/duplicate/unsorted/non-adjacent edge、sharpness 0/3/fractional、129-edge、unknown field 和 stale catalog 负向；Runtime 覆盖 normalized request hash、request-bound result validator、read-only projection/no CAS write 与真实 `geometry_prepare` 128-triangle candidate/readback/no-version；MCP 覆盖 closed oneOf、1 MiB full wire、unknown/cross-branch/fractional fail-closed。package/live/render/visual/human/360 未由这些测试运行。

Boolean Operand Lineage source/focused Gate 覆盖 union/difference/intersection deterministic result、非 Boolean node、run budget 0/overflow、unknown field、request canonical tamper、program/catalog/node/operand/run/hash/canonical 双层绑定、哈希一致但 operation/operand/source-lineage 伪造的 Worker 输出拒绝、Manifold run 从 0 开始且三角面对齐，以及 Runtime/MCP candidate/version/CAS no-write。测试只证明 evaluated-face operand lineage 的结构语义，不证明原始 authoring face 或视觉质量。receipt：`docs/evidence/mcp010f/blender-boolean-operand-lineage-source-gate-20260819.json`。

Render Evidence Integrity 追加覆盖 artifact/reference 原始 CAS byte/hash/size readback、ArtifactReadback artifact ID 等值、JSON CAS 1 MiB 上限，以及 Quality/comparison metric、status、hard-gate、eligibility divergence 负向。该 Gate 仍只是 source structural integrity，不是视觉或人评通过。

2026-08-19 Render Evidence Integrity historical source/focused Gate 覆盖 162-schema closed contract、current candidate restart readback、ArtifactReadback/JSON object hash、camera identity、九 AOV/mask bytes hash与 RGBA8/512×512 解码、RenderProfile/threshold lineage、deterministic repeat、stale camera fail-closed 和 Runtime no-write。该 slice 当时机器面为 162 schemas、19/19 active operators、44 read + 33 opt-in write = 77 tools；历史 attempt35 断言继续原样失败/缺失。

2026-08-19 Mechanical Pose Sequence Preview historical source/focused Gate 覆盖 160-schema check、0/500/1000 tick exact samples、单 tick/序列中点 hash 等价、deterministic repeat、semantic result tamper、unsorted/duplicate/17-sample/duration/null-action/未知字段负向，以及 candidate/version/CAS no-write。该 slice 当时机器面为 160 schemas、19/19 active operators、43 read + 33 opt-in write = 76 tools；source structural PASS 不代替 package/live/Viewer/visual/human/360 Gate。

2026-08-18 historical Parametric Group v2 source/focused Gate：该 slice 当时为 158 schemas，覆盖 closed-schema、三模板确定性、same-template/different-instance hash、semantic result tamper、未知字段/脚本/URL/路径、wrong parameter branch 与 candidate/version/CAS no-write 回归。

2026-08-18 historical Mechanical pose focused Gate：该 slice 当时通过 contracts 156-schema check；Runtime 三层 root/fixed → revolute → prismatic hierarchy、500-tick midpoint、local/world TRS、同义重排 canonical hash、null action rest、candidate/version no-write PASS；cycle、limit、foreign candidate、tampered input 与未知 script 全部拒绝。MCP closed inline Schema round-trip 与 nested unknown field negative PASS。Worker/materialization/package/live/visual/human 未运行。

2026-08-18 Subdivision evaluation v2 focused Gate：contracts 151-schema check；Runtime 0/1/2 levels 精确 8/32/128 triangles、9/25/81 vertices、4/16/64 quads、8/16/32 boundary edges，重复 result 相等，现有 Worker compile triangle count 一致，控制点变化改变 cage/program hash；solid、point-count、adaptive、crease、triangle-budget、unknown nested field 与 input-hash drift 全部拒绝。MCP 第五 closed branch round-trip/unknown/cross-branch/solid negative PASS；Runtime 前后 candidate/version/CAS 不变。package/live/render/visual/human 未运行。

2026-08-18 TopologySnapshot focused Gate：覆盖 primitive closed manifold、开放 SubD cage、bounded same-Part Boolean、corner normal/UV/tangent、重复 canonical hash、read-only candidate/version count、错误 readback hash、1-face no-truncation、513-face request 与 MCP nested unknown field。完整 Runtime 无 feature run 中 10 个既有 render-dependent tests 因固定 sibling 缺失返回 `GEOMETRY_WORKER_UNAVAILABLE`，必须与 topology focused PASS 分列；正式 source Gate 继续使用仓库同 cohort worker 脚本。视觉/package/live Gate 不由本 slice 替代。

2026-08-17 P2 source/package Gate：`repair_intent_run_prepare` 的 CAS intent/observation/reference/camera/candidate 绑定、bounded action scope、staged-only 与 no-confirm boundary 已由 MCP/Runtime focused tests、完整 `script/test_mcp010f.sh` 与最终 Dev.app 真实参考 transport 覆盖；receipt：`docs/evidence/mcp010f/repair-intent-run-source-gate-20260817.json`。packaged transport PASS 但 camera evidence gate blocked；Repair apply、live restart 与视觉质量仍分别 `NOT_RUN/BLOCKED`。

版本：2026-08-09
状态：MCP001–009 focused Gates 已建立；FGC-MCP010A done；MCP010B structural source Gate、MCP010C renderer/compare source Gate、MCP010D operator/Skill source Gate、MCP010E AssetPack/PBR source Gate、MCP010F Viewer/contour/Mechanical pose source Gate 与 Hero UV durable Store→Runtime→MCP source Gate 已通过；Agentic observe/plan projection、scene/stage 嵌套只读 projection conformance 与 durable session/checkpoint/RepairIntent prepare/readback isolated Gate 已通过；Hero UV 真实 prepare/replay/drop/reopen/get 为 1/1 PASS，但仍不是 artist unwrap、visual、human、engine、commercial 或 packaged pass；Mechanical pose 仅为 read projection，不是 Armature/skin/animation asset；durable/reference/DesignSpec producer、通用单动作 orchestrator、Repair 应用、同一候选的 packaged/human/PBR/export/360 子门仍 `NOT_RUN/BLOCKED`，唯一 `in_progress` 仍为 `FGC-MCP010F`。

Stage 0 机器真值入口为 `docs/evidence/mcp010f/current-benchmark-truth.json`。源码门 PASS 不等于产品质量 PASS：attempt35 只是 provisional retained observation，它为 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，camera 绑定 `MISMATCH`。packaged Viewer 当前只能记为 `PASS_CURRENT_COHORT_BOUND_READ_MODEL`；UI E2E、正式 VoiceOver、视觉和人评仍分别 `NOT_RUN`。


<!-- forgecad-reference-source: input=ENV_AUTHORIZED_PNG original_sha256=1964704a62ed7a841b4d49c370b8d46f4626e201daad29092a9c39a40b4c4109 intake=PASS_SOURCE_SIX_REFERENCE_EVIDENCE_CAS views=6 worker=PASS_SAME_COHORT_SIX_FIXED_VIEWS target=USER_REFINED_USER_CONFIRMED_REVIEWED_STRUCTURE user_confirmed_crop=PASS_USER_CONFIRMED_SEVEN_CROPS contour=PASS_USER_CONFIRMED_SIX_IDENTITY_CONTOURS negative_space=BOUNDING_REGIONS_CONFIRMED_EXACT_SUBTRACT_UNKNOWN line_flow=EXPECTED_ROWS_DURABLE_MATCH_NOT_PROVEN camera_lock_fixture=PASS_REAL_DURABLE_REPLAY_RESTART form_art_fixture=PASS_REAL_DURABLE_NOT_PROVEN form_quality_v2_fixture=BLOCKED_ZERO_WRITE_MISSING_LEGACY_CROSS_VIEW secondary_form_approved=NOT_CREATED fixture=PASS_REAL_1_OF_1_108.07S -->

## 1. 证据层级

1. Schema/static/fuzz；
2. Core deterministic unit/property；
3. Store transaction/crash/recovery；
4. Worker sandbox/geometry/render；
5. Agentic Runtime durable session/checkpoint：schema fixture、approval/binding、SQLite/CAS persistence、immutable failed checkpoint、CAS-only RepairIntent、Runtime/MCP restart readback 和 public-contract receipt checker；
6. Runtime integration；
7. MCP conformance 和真实 Codex 宿主；
8. Viewer browser + packaged WebView/GPU；
9. 完整 reference→candidate→approval→version→restore→export；
10. 安装/升级/回滚/灾难恢复；
11. 跨类别独立真人质量。

低层通过不能替代高层；每层分别标 PASS/FAIL/BLOCKED/NOT_RUN。

## 2. CI Gate

- contracts generation/check、unknown/oversize/adversarial；
- Rust fmt/clippy/unit/integration；Runtime process lock 在退出后立即释放；Runtime 缺失/启动失败/ready 后崩溃时 MCP initialize 仍成功且 stdio 保持；最多一次有界 restart/backoff；fixture 只在 test child-local env/tempdir 中运行；
- SQLite single-writer、kill、disk-full、WAL、migration、backup/restore；
- geometry/readback/GLB header/lineage validator（MCP007 PASS）；appearance/UV/PBR/fixed-render validator（MCP008 focused PASS）；
- Skill DAG/operator/budget/hash/SBOM/license；分发签名/撤销在 MCP012/013；
- MCP tools/resources/schema/annotations/errors/idempotency/timeout；
- Codex Desktop/CLI smoke（真实发布版本，当前 P0 required）；Codex IDE/VS Code/Cursor/Windsurf 兼容 smoke 为未来非阻塞 Gate；
- Viewer typecheck/build/E2E、单 renderer、a11y、尺寸、GPU fallback；
- packaging/notarization/install/upgrade；
- security/secret/path/content-scope；
- visual benchmark/human gate。

## 3. 强制失败路径

stale base、重复 idempotency、hash mismatch、approval reject/expire、quality hard fail、attachment symlink/越权/炸弹、unknown Skill Operator、DAG cycle、Worker timeout/crash/late result、MCP/Viewer/Runtime kill、disk full、CAS corrupt、renderer unavailable、license/signature revocation。

任何失败不得创建永久版本或泄露内部数据。

## 4. MVP 硬表面 Benchmark

首个 benchmark 使用用户授权的白色硬表面机器人参考，原图只进 CAS，不复制到仓库。Evidence 至少包含 source/CAS hash、尺寸/MIME/授权、typed programs、真实 GLB/readback、wireframe/part-ID、PBR/固定 render、reference metrics、QualityReport、版本 DAG 和用户评分；MCP007 目前只提供 structural geometry/readback，不代表 PBR、渲染或参考相似度。

MVP 不预设一个没有校准的相似度数字作为营销门。MCP009 真实 Codex receipt 已记录 hard checks、fixed render 和明确 limited aspect-ratio evidence；silhouette/landmark/region threshold、真实 Codex typed visual review 和人工接受仍未运行，不能把 host golden path 写成视觉质量通过。

## 5. 通用 3D Benchmark（post-MVP）

数据集按类别、视图数、材质、几何表示和难度分层；机械只能是一类。每条保存授权参考、target claims、RenderSet/AOV、readback、QualityReport、timing/memory、Codex review 和盲评。报告展示每类失败和最差分位，不只展示平均分。

结构指标与视觉指标分开：manifold/UV/PBR/GLB 绿色不等于参考相似；视觉好看也不等于可编辑、版本和导出正确。

## 5.1 MCP010 原子测试矩阵

| Task | 必测 |
|---|---|
| 010A | docs/integrity、安全/许可证、同 revision binaries、raw stdio/CLI、用户重启后的真实 Codex capability/project/build hash |
| 010B | V2 Schema、损坏 index/source/hash/winding/UV、primitive topology/normal、五次 deterministic GLB/readback |
| 010C | synthetic camera recovery、z-buffer/occlusion、九 AOV hash、mask/IoU/F1/landmark/region、四个 MCP 工具和错误合同 |
| 010D | 每个已实现 Operator 正/负 fixture、预算/超时/崩溃、mirror/Part lineage；当前 19-entry/19-active source catalog 与 fixed sibling Worker Gate PASS；Manifold bounded C ABI/FFI、恶意输入/确定性/source-ID/残余切线 focused Gate 已通过，任意 mesh Boolean 仍 unavailable |
| 010E | AssetPack/hash/license/SBOM、颜色空间、UV/tangent、无 external URI、纹理预算、Runtime readback + glTF Validator |
| 010F | Viewer 单 context、compare/selection/isolate/explosion/a11y、真实 Codex change/confirm/restore/export/restart 同 hash、人工评分 |

当前三分之四参考的阈值和评分见 `MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`。单图通过只能写 `PARTIAL_VISIBLE_VIEW_PASS`；五张补充参考未到齐时 360 Gate 必须是 `BLOCKED_REFERENCE_COVERAGE`。

### WPN-ARCH-EVALUATION-001 架构测试矩阵

| 范围 | 必测 | 当前证据 |
|---|---|---|
| Contract/domain map | 41 个 Evaluation operation 的 `observe`/`quality_review`/`job` façade 归属、command/query split、capability façade 与 domain 双重校验 | `8/8 PASS`，含 `PASS_NO_RUNTIME_DOMAIN_MISMATCH` |
| Runtime service/router | `RuntimeOperationRouter → evaluation_service::invoke`、无 active legacy match arm、service 不回入 `dispatch_ipc`；Presentation/Delivery 已有 direct typed service，物理抽取仍部分完成 | Router `8/8 PASS`，Evaluation service `5/5 PASS`；最终 combined fast gate 另见上表 |
| Store boundary | `EvaluationRepository<'store>/JobRepository<'store>` 只借用 Store；不复制 connection、migration sequence、CAS root；Job SQL 不回到 Store root；剩余 `ReadModel`/`QualityEvidence` 必须保持声明 | Store boundary `10/10 PASS`；borrowed Job aggregate，partial gaps retained |
| MCP default/feature | 41 operation 的 default/feature route、未知 operation/domain mismatch fail closed、compatibility replay 不进入默认 Action Space | default `23/23 PASS`；feature `23/23 PASS` |
| Fresh same-cohort aggregate | Runtime/Geometry/High/Render identity、source drift、预算与 ignored 计数 | Evaluation 前序 cohort=`81eca9cbfd5cb5d2428fa46f8491c423076f881aba258e6be8b4d4d652c711ff`；`65/0/0`、166s；最终 combined cohort=`641a87b74c6ac1f28c5db25efadb52125f04624ee36ce1600f08ffdb43ccfbad`，`82/0/0`、190s |

该矩阵只证明 Evaluation 的 source/architecture boundary。它不推进 High→Low→UV→Bake、材质、FPS、引擎、视觉、人审或商业质量；不触碰用户数据，也不改写 Surface 或任何历史 receipt。Schema closure 仍为 12/125（Evaluation 1/41）的横切后续工作。

## 6. 真实 Codex Gate

自写 MCP client、fixture 或手工复制附件不能替代 Codex。MVP 已由真实 Codex CLI（带授权 image attachment）证明 reference bytes、geometry/appearance/readback、quality、write approval、version 和 CAS-only GLB export；MCP007 14 parts/516 triangles 与 MCP009 15 parts/580 triangles receipt 均 PASS。Viewer/restart/change/restore 同 hash、Desktop attachment/write surface、像素指标和人工门仍分别补证或明确 unavailable。正式发布才要求 signed package 上的 Desktop + CLI 全量路径。IDE/其他 Client 是未来 Gate。

## 7. Evidence manifest

每个任务 evidence 包含环境、commit/worktree、命令/exit code、合同/二进制/资产 hash、原始 artifacts、日志脱敏证明、未运行项和 blocker。Agentic durable slice 的机器入口是 `scripts/probe_agentic_runtime.py`，合同校验是 `scripts/check_agentic_runtime_receipt.py`，receipt 为 `docs/evidence/mcp010f/agentic-runtime-session-checkpoint-20260813.json`。Markdown 总结不替代机器收据。

<!-- forgecad-stage0: schemas=662 schema_set_sha256=202e080ec378ddb294eb9c880079dcec5c910b27a1c679034ca34c5a880dcec6 read_tools=131 write_tools=95 total_tools=226 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260815-b37-complete-auto-v3.json latest_completed=real-codex-cli-current-20260815-b37-complete-auto-v3.json -->
