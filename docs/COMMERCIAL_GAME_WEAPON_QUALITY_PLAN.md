# ForgeCAD 商业级游戏武器质量计划

> **Weaponry P0 override (2026-08-29):** 本文只有在与 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md` 和 ADR-0029 一致时才具有当前执行权。ForgeCAD 在本文中解释为 Weaponry 的 Rust Runtime lineage；当前唯一产品主线是由 Codex 生成、修改、验证并交付高质量穿越火线非功能性游戏武器。通用 3D、机器人和原创科幻示例仅作 fixture/历史能力，不得抢占本月主线。 本文中所有 2026-08-28 及更早的“当前”“下一原子”“唯一 `in_progress`”和工具/Schema 数量语句均按历史 cohort 解释，不得覆盖 `WPN-*` successor queue。

本计划的商业目标现在具体化为穿越火线武器，而不是泛化 Hero Weapon。质量重点包括：
第一人称屏幕占比与轮廓识别、inspect/ADS 下的近距离 bevel/normal/材质可读性、换弹/机械件
socket 与动作空间、目标引擎预算、LOD/collision、同系列武器风格一致性，以及合作方武器美术
人审。仍不得复制无授权资产，且玩法数值、现实结构和制造信息不属于本软件输出。

> 2026-08-27 `04AV` 商业门补充：三视图 `rear-stock` 部件身份/相机/深度校准与 owner∩expected-void=0 必须分别记录。calibration=`ELIGIBLE` 只能开放一个 bounded art-shape draft，不是质量 PASS；strict owner-void、negative-space、line-flow、六视图 non-regression、FormArt/Codex review 和用户 secondary approval 必须随后通过。当前只有 source/compile 证据，real-D1 projection 因 04AU 一次性 Store/CAS 不可用而 `NOT_RUN`；不得据此进入 High/Low/UV/Bake 或宣称《无畏契约》对标。

> 2026-08-27 `04AK` 商业门补充：商业 FormArt baseline 必须由 Runtime 在 approved lineage 与同一当前 cohort 上新鲜生成固定六视图，历史 FormArt 只能回读、不能晋级复用。当前只有 fail-closed preflight，materializer=`UNAVAILABLE`，因此任何 High/Low/UV/Bake、材质或 FPS 展示均不得提前开始；proposal 原子性改善也不构成视觉质量证据。

> 2026-08-27 `04AJ` 商业门补充：620/680 的尺寸变化本身不构成质量改善。fresh six-view 必须全量 observed；left/right/rear3q owner 区要求 expected void≥256px、boundary≥64px、owner≥128px、adjacency≥32px 且≥250 milli、owner-overlap=0；negative-space 要求 IoU≥850、Boundary F1≥800、area ratio 850–1150、centroid error≤3000；line-flow 要求 coverage/continuity≥900、chamfer≤3000、max deviation≤5000、direction≥950、crossing=0。Pareto 还要求 semantic 零回退、每项 core regression 不低于 -1000ppm、至少一项 core 改善≥1000ppm、legacy aggregate≥+1ppm；满足后也仅为等待用户 secondary confirmation。

> 2026-08-26 `04AI` 商业门补充：审稿相机必须由 Runtime 从用户确认的语义屏幕顺序派生，Codex/Viewer 不得提交 orbit 或自行计算 hash。真实 D1 zero-write projection 已给出 `180° / 9d8e590e…5b3abb / world +Y upright PASS`；在 orientation-specific user receipt 创建前，它仍只是审稿准备，不计入 Form、High/Low/UV/Bake 或商业质量。

> 2026-08-26 rear3q 质量门补充：参考像素旋转与相机侧选择必须分离。商业 FormArt 比较必须绑定用户确认的 stock/muzzle 屏幕顺序、最终 registered camera hash、Runtime source-anchor 投影和 world `+Y` upright proof；任何通过 180° 图像倒置换取端点匹配的方案一律不具晋级资格。

> 2026-08-26 执行增量：路线不变。D1 首次拥有能表达真实开放枪托负空间的 product-owned 闭合拓扑编辑，且在真实候选上跑通 durable → GLB → six-view FormArt。这不是商业质量 PASS：`700/700` 方案全局退化 `-3081 ppm` 并被拒绝，所以 High/Low/UV/Bake/Material/FPS/Engine/Human 依然不可晋级。

> 2026-08-26 最新执行真值：**528 schemas / 28 operator entries / 115 read + 87 opt-in write = 202 tools**。真实 D1 `rear-stock` 已完成第一条“美术网格纵切”及其 proposal-side 六视图 durable FormArt 回执、same-key replay 与 Runtime restart readback。当前网格仍只是 8V/6Q 枪托纵切，距商业 Hero Weapon 很远；最佳 Z taper 只能作为 `REVIEWABLE_TRADEOFF`。Part-ID 已全视图 observed，但 owner/open-void、negative-space、line-flow 未全通过，故 `BLOCKED_PROPOSAL_FORM_ART_EVIDENCE`，不得创建 `secondary-form-approved`。

> 04AG proposal-side durable evidence 已建立：CAS receipt 已有 SQLite Store/replay/restart/reachability 绑定，六视图 camera/reference/candidate/artifact/AOV 均绑定同一派生 candidate。Part-ID 全 observed，但三处 owner/open-void、negative-space 和 line-flow 尚未全部通过，因此 sidecar transport=`PASS`、内容 Gate=`BLOCKED_PROPOSAL_FORM_ART_EVIDENCE`；prepare projection、`REVIEWABLE_TRADEOFF` 或 durable PASS 都不是商业质量/secondary/Stage PASS。

## 0. 当前不可绕过的单资产纵向执行顺序

ForgeCAD 不再以“增加 Schema/工具数”作为主进度。从当前真实 D1 开始，顺序锁定为：

1. `FPS-FORM-04AG`：对派生 candidate 重新生成 hash-bound Part-ID、owner-void、negative-space、line-flow 与六视图 FormArt；缺失时 fail closed。
2. `FPS-AUTHORING-MESH-V2-04`：围绕同一 rear-stock 补齐 inset/bevel/loop-cut/slide/bridge/crease/weighted-normal/seam/group 的局部 mutation journal，让边、面、高光带与负空间节奏可被真正创作；不回退到 primitive 参数扫描。
3. `FPS-HIGH-05`：同 lineage 建立独立 High candidate 与 non-destructive DetailGraph，引入 bounded Manifold Boolean、support loops、bevel/crease/weighted normal 与 CPU OpenSubdiv evaluator；当前 Catmull-Clark seam 仅是可编译基线，不冒充 OpenSubdiv/hero High。
4. `FPS-LOW-06`：QuadriFlow 只产出 `DRAFT_UNREVIEWED`；必须落到 ForgeCAD `LowAuthoringMesh@1` 进行 quad flow、hard-edge/UV seam、aperture/socket/FPS deformation lock 编辑，并为每个 Low corner/face 保存 High correspondence。
5. `FPS-UV-07`：xatlas 只做 draft；Hero UV 必须有 artist seam/island orientation、可见性加权 density、stretch/overlap/OOB、mip/block padding、UV0/UV1 和 Mikk tangent replay。
6. `FPS-BAKE-08`：独立 High/Low/Cage，用 Embree 受限 ray kernel 产生 normal/AO/curvature/thickness/position/ID 等 `BakeSet@1`，对 miss/fallback/cross-Part/skew/penetration/bleed 生成可审计 heatmap，不允许 nearest fallback 掩盖失败。
7. `FPS-MATERIAL-09`：将当前 `VALIDATED_PLAN_NOT_EVALUATED` 的 MaterialLayerGraph 变成真正的 mask/generator/anchor/decal/wear/microdetail 求值器，输出同 UV/Bake/hash 绑定的 MaterialPack/KTX2。
8. `FPS-PRESENTATION-10`：第一人称工作台必须同时审查 hip/ADS/inspect/equip/reload/recoil/fire、third-person/ground pickup、root/pivot/socket、clip/event、VFX/audio cue 与 safe-zone/reticle/muzzle 占用。
9. `FPS-ENGINE-11`：meshoptimizer/Basis/KTX2/glTF Transform 仅作固定 allowlist 打包内核；Unreal-first clean-project import/save/reimport/restart/packaged run 为硬门，Unity 为第二 profile，必须回读 mesh/material/tangent/LOD/collision/socket/animation/event 与 p50/p95/p99。
10. `FPS-HUMAN-12`：独立 Hero Art Review 、DPT 与修订清单闭合后，才允许用户 confirm、创建 version、export 并做 restart hash 验收。

任何阶段只有 compile/source/durable replay 时，都只记 structural status；triangle collapse 不是 artist quad Low，UV replay 不是 Hero unwrap，8-map PNG 不是 approved bake，MaterialLayerGraph plan 不是纹理求值，engine-neutral GLB readiness 不是 Unreal/Unity round-trip，Codex review 不是独立艺术家审核。

> 2026-08-26 实施落点：规划首次纵向触达真实 D1 authoring authority。Runtime 新增 `production_weapon_authoring_mesh_v2_source_prepare`，只接收 exact candidate/program/artifact/readback/Part/source hashes，不接收 caller mesh；它从 `rear-stock` 原始 box 派生 8 顶点/6 四边面 genesis，把完整 `AuthoringMeshV2SourceBinding@1` 内嵌进不可变 revision，并完成重启回读。High Worker 已有 V2 original-topology projection，kernel 已有 `FaceExtrude`。当前仍缺真正影响封闭武器轮廓的 `MoveVertices/Inset/Bevel/Crease/weighted-normal`、proposal 回写和六视图通过，故本条只把 Stage 2 的“真实输入接线”标为 `PASS_STRUCTURAL_SOURCE_BOUND`，商业链其余项不晋级。

> 2026-08-26 `FPS-FORM-04AE` 研究/工程收口：三条 Luna Max 研究线与 GitHub/Hugging Face/官方引擎资料共同确认，商业目标必须是 `Art Direction → Form → AuthoringMesh → High → editable Low → Hero UV → per-Part Cage/Bake → MaterialLayerGraph → FPS presentation → Unreal/Unity → independent Hero Art Review` 的同 hash 纵向闭环。新总研究蓝图见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。Runtime 已新增四组 3D semantic anchors 物化，并完成禁止 caller 注入 program/ordering/orientation/RigV2 的 CameraLock additive durable child source 纵切；focused Contracts/Store/Runtime/MCP compile PASS。它不生成 target 2D landmarks、不替代 rear3q user approval，也不推进 Stage。证据：`docs/evidence/mcp010f/production-weapon-semantic-ordering-materializer-source-gate-20260826.json` 与 `docs/evidence/mcp010f/production-camera-lock-registration-lineage-source-compile-gate-20260826.json`。

> 2026-08-26 `FPS-FORM-04AD` 权威增量：当前合同面为 **518 schemas / 111 read + 83 opt-in write = 194 tools**。新增 `ProductionWeaponSemanticLandmarkOrdering@1` 只表达 Runtime-derived 的 3D source/subject-axis 顺序，明确 `target_landmark_arrays_present=false / metrics=NOT_PRESENT`，不得冒充 2D landmark；`ProductionWeaponAuthoredViewOrientation@1` 将诊断变换与用户方向回执分开；`RegisteredCameraRigCalibration@2` 只有绑定 promotable authored rear3q receipt 才能物化。定向 Contracts/Runtime/MCP compile 与 518-schema checker PASS。真实 D1 尚无 orientation-specific user receipt，因此保持 `BLOCKED_AUTHORED_REAR_THREE_QUARTER_ORIENTATION`、Stage=`camera-calibrated`、secondary=`NOT_CREATED`、quality=`QUALITY_TARGET_NOT_MET`，不 confirm/version/export。旧 `@1` 保持历史真值；durable 落点采用 CameraLock 的 additive child lineage，不复制/自动升级整张旧记录。

> 2026-08-26 `FPS-FORM-04AC` 工程落地：枪托负空间修复器已从“扩大 inner-lip”改为“锁定外包络的三站位型面重建”。端部 stations 只编辑 cap/receiver 内侧 junction，中心 station 只增加微小 depth contour；用 exact source-node identity 与 PartOutput 不变 Gate 防止影响下枪托/尾帽。商业 acceptance 仍严格要求：六视图 silhouette/landmark 全部 non-regressing；left/right/rear3q owner overlap 最终为 0 且 adjacency 达门；至少一个 target view strict improvement；同 candidate 的 fresh FormArt/Codex review；rear3q authored semantic orientation durable hash-bound。当前只有 source compile，不是资产质量 PASS。

> 2026-08-26 `FPS-FORM-04AB` 根因闭环：Runtime-derived subject→registered camera 已贯穿 ReferenceCanvas、CameraLock validation、FormEvidence、FormArt、raster attribution 与 CrossView，fresh durable D1/restart 1/1 PASS。`left.open-stock-void` 的唯一最高影响源为 `rear-stock`（548 px），次高 `receiver-main`（114 px），所有 muzzle source 为 0，semantic repair gate 返回 `UNIQUE_HIGHEST_IMPACT_SOURCE_OBSERVED`。立即执行顺序为：一个 rear-stock source-node bounded RepairIntent → 三张开放枪托视图与六视图复核 → strict owner-void + non-regression + Codex review → 用户批准 secondary。商业级正式架构采用 CameraLock additive child lineage，分别持久绑定 source ordering、authored rear-three-quarter orientation 和 registered rig；旧 @1 仅历史兼容。证据：`docs/evidence/mcp010f/production-weapon-real-d1-semantic-camera-relock-source-attribution-pass-20260826.json`。

现有凹形 Profile 屏幕暴露了更深的美术问题：单纯扩大枪托开口会降低 exact owner intrusion，却同时损失整体轮廓质量。04AB 同 cohort 既有 RenderSet 的零写分析显示，left/right overlap 可由 `548/921px` 降至 `281/382px`，但三张 registered 视图的 silhouette IoU 未形成 non-regression。这意味着商业修复器不能只优化一个 mask 标量；它必须编辑同一 `rear-stock` source node 的二维轮廓节奏、连接肩、尾帽衔接和深度站位，并以负空间、外轮廓、landmark、Part/source identity、六视图及第一人称可读性共同约束。该分析不改变 `QUALITY_TARGET_NOT_MET`。

本轮 FormArt 哈希绑定归因回执：`docs/evidence/mcp010f/production-weapon-formart-hash-bound-raster-attribution-source-gate-20260826.json`；Formal High transport 回执继续独立保留。

2026-08-26 诊断能力更新：ForgeCAD 已在现有只读 FormArt get 与固定 Render Worker 间建立 exact durable evidence→pixel→triangle→semantic Part/source node 的闭合通路；candidate、artifact/readback、reference target、CameraLock/rig、FormEvidence view、RenderSet、mask 与 source-table hashes 都由 Runtime 派生并校验，caller 不能提供 camera/mask。04AA 曾因语义错配 fail closed；04AB 已在 fresh D1 中完成 registered-camera replay 并唯一归因到 `rear-stock`。该 slice 只提供可解释归因，不代表 Form、High、视觉或商业质量通过。

2026-08-26 研究落地同步：ForgeCAD 当前仍是“可验证的高级灰模/技术资产管线”，不是商业级 Hero Weapon 生产软件。根因不是渲染参数，而是统一 Art Direction、可编辑 AuthoringMesh/quad flow、非破坏 High、Hero UV、逐 Part Cage/Bake、typed Material Layer Graph、FPS presentation、真实 Unreal/Unity 回读和独立 Hero Art Review尚未在同一 candidate/export hash 闭合。当前 518 schemas 与 194 tools 只表示协议表面；Formal High 的新增合同/Runtime adapter 也不改变 `QUALITY_TARGET_NOT_MET`。

2026-08-26 最新实现状态：Form 三条 Stage 晋级边已具深层 Store evidence policy；Formal High pure factory、Store atomic seam、Runtime internal materializer 与 public `get/prepare` 已存在。它们只达到 `PASS_SOURCE_COMPILE_FOCUSED`；完整 positive replay/cleanup/restart/raw transport、artist review、visual/human/engine/commercial acceptance 均未通过。真实 D1 仍为 `camera-calibrated / secondary=NOT_CREATED / QUALITY_TARGET_NOT_MET`，因此本商业路线顺序不变。证据：`docs/evidence/mcp010f/commercial-weapon-form-stage-policy-formal-high-source-gate-20260826.json`。

2026-08-26 Cage/Bake public seam 增量：Store 已提供七子表原子 commit、精确重放、CAS reachability/GC 与重启 get；固定 Worker 已收口 exact-topology Cage、8-map geometric Bake 和 8-texel dilation。MCP 已有 `production_weapon_high_low_bake_preflight_get/get/prepare`，Runtime 已有固定 2K Worker 启动门、Hero UV candidate+Low-artifact 唯一反查及机器 blocker 分类。AuthoringMesh durable 已保存 source artifact/GLB lineage，Native High durable 保存 High GLB/readback；Formal High 也已有 Runtime-owned distinct derived candidate factory、原子 Store seam、无环 internal materializer 与 public `get/prepare`。尚未通过的是完整 source-lineage/CAS 正向 materialize→replay→drop/reopen→cleanup fixture 与 raw transport，而不是“完全没有 GLB”、缺少 public surface 或缺少另一套重复合同。真实 D1 的 Form 前置未通过且无 formal positive receipt，所以新 prepare 仍报告 `FORMAL_HIGH_STAGE_SOURCE_LINEAGE_UNAVAILABLE`，整体为 `PRODUCTION_WEAPON_HIGH_LOW_BAKE_PRODUCER_UNAVAILABLE` 且零写。当前 D1 没有正式正向 Cage/Bake receipt，不提升 Stage、视觉、人评、引擎、分发或商业结论。证据：`docs/evidence/mcp010f/commercial-weapon-form-stage-policy-formal-high-source-gate-20260826.json` 与 `docs/evidence/mcp010f/commercial-weapon-cage-bake-public-seam-source-gate-20260826.json`。


> 2026-08-26 最新权威 source 口径（取代下方 2026-08-25 的“最新/当前”计数）：**518 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 MCP tools**。Low quad draft 已接入 Contracts、Store、Runtime 与公共 `low_quad_draft_durable_get/prepare`，并保留 candidate-bound current Low provenance；仍为 `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`。同键 prepare replay → Runtime drop/reopen → get 当前 cohort **1/1 PASS**，相关六个 Low CAS roots已纳入 linked/GC。Hero UV public durable链与四个 CAS roots已通过 structural/source Gate。04AB 已解除错误 source attribution blocker并锁定 `rear-stock`；当前 blocker 已转为联合形体修复、rear-three-quarter authored orientation、六视图质量和 secondary approval。以上均不推进 Stage、confirm、version 或 export；`FPS-HIGH-05=NOT_PASSED`、Stage=`camera-calibrated`、`secondary-form-approved=NOT_CREATED`、visual=`QUALITY_TARGET_NOT_MET`、`HQ360=BLOCKED_REFERENCE_COVERAGE`、proposal=`registered=false`。证据：`docs/evidence/mcp010f/commercial-weapon-hero-uv-durable-restart-source-gate-20260826.json` 与本文件顶部 FormArt attribution receipt；`FGC-MCP010F` 是唯一 `in_progress`。

> 2026-08-25 历史快照（已由上方 2026-08-26 权威口径取代）—商业作者链 source 事实：**499 schemas / 28 operator entries / 107 read + 79 opt-in write = 186 tools**。Native High 已有有界面内 support-loop chamfer arc，并以当前 cohort 完成 Worker 6/6 + 7/7 与 Runtime restart 1/1；Editable Low 已有显式 quad draft/edge-flow producer-readback；Hero UV 已有 2K/4K 双通道、可见性权重、mip/seam/stretch/overlap/Mikk 诊断；Viewer 已有只读 Art Director chain matrix。后两项仍是 Worker-only，High proposal仍 `registered=false`；这些只推进 L0 source structure，不构成 packaged/candidate-bound High/Low/UV、visual、human、engine 或 distribution PASS，`FPS-HIGH-05=NOT_PASSED`。

2026-08-25 `CQ-02-TYPED-TOPOLOGY-IDENTITY-LINEAGE`：`authoring_mesh_edit_preview → authoring_mesh_edit_prepare` 的 `split_edge / collapse_edge / dissolve_edge` proof 仍保持 source-element-only；下游 Runtime 现在只从 Store 的 exact candidate→idempotency response 恢复该 proof，并把 parent source identity 物化为 durable `AuthoringMeshIdentityLineage@1` child IDs、单调 tombstone 及 one-to-many/many-to-one relation，不接受 caller identity/proof arrays。真实 split/collapse/dissolve 已分别完成各自独立的完整持久化与 Runtime drop/reopen/get 重启链路，合计 **3/3 PASS**；Store `authoring_mesh_` **12/12**、MCP IdentityLineage **3/3**、490-schema checker与 Contracts/Store/Runtime/MCP 联合 compile PASS，工具数仍 **106 read + 78 write = 184**。general correspondence、evaluated retarget、完整 selection/undo history 与产品级 cross-version editor仍 `NOT_PROVEN`。Stage 保持 `camera-calibrated`，视觉=`QUALITY_TARGET_NOT_MET`，human/engine/distribution=`NOT_RUN`，HQ360=`BLOCKED_REFERENCE_COVERAGE`。新回执：`docs/evidence/mcp010f/authoring-mesh-typed-topology-identity-lineage-materialization-source-gate-20260825.json`；原 source-proof 回执继续作为上游证据。

2026-08-25 Native High 当前 source 口径：High Worker stable-ID 输入、`HighMeshArtifact@1`、有界 support-loop chamfer arc、embedded GLB 2.0 lowering 与严格回读通过 library **6/6**、transport **7/7**；Contracts **499/499**。公共 MCP source/focused 与当前 cohort Runtime positive prepare/replay/drop-reopen/get fixture **1/1 PASS**。但 packaged same-cohort、candidate-bound High quality report、完整 support-loop/weighted normal/Subdivision、正式视觉/真人/引擎/分发仍未运行；未注册 proposal 不能因 source slice 被激活。因此 `FPS-HIGH-05=NOT_PASSED`；Stage 仍 `camera-calibrated`，visual 仍 `QUALITY_TARGET_NOT_MET`，human/engine/distribution 仍 `NOT_RUN`，HQ360 仍 `BLOCKED_REFERENCE_COVERAGE`。

最新 supplemental receipt：`docs/evidence/mcp010f/native-high-glb-durable-source-implementation-gate-20260825.json`。该回执覆盖 source-focused Runtime restart 与 MCP，不替代未运行的 package、candidate visual、人审、引擎和分发 Gate。

版本：2026-08-26
状态：accepted target plan；当前实现仍为 `QUALITY_TARGET_NOT_MET`
适用范围：虚构、非功能性游戏武器美术资产；不包含现实制造尺寸、加工图、材料配方、性能或操作建议

## 2026-08-26 04AF 实施结论与路线修正

本轮不是又一次 source-only 扩展：真实 D1 的 `rear-stock-profile-reconstruction-v1` 已完成 source materialization、严格 GLB readback、六视图 RenderSet、CrossView bundle 与 quality decision。结果是 front/back/top 持平，left/right/rear-three-quarter 回退，因此 Runtime 将方案标记为 `rejected-regression`，没有 confirm、version 或 export。这个失败表明，只靠少量 profile 参数不足以创作商业武器的外轮廓节奏、连接肩、尾帽衔接和深度转折。

ForgeCAD 因此把下一个主路径从“继续搜索几个参数”调整为“真正美术创作网格”：`AuthoringMesh@2` 已用真实 Runtime 完成 genesis→稳定半边 `split_edge`→CAS/SQLite→Runtime restart exact readback。它当前只是隔离的 structural proof；下一阶段必须绑定真实 D1 武器，并补齐 extrude/inset/bevel/loop/slide/crease/normal/seam/group、selection/constraint set、evaluated retarget 与 branch/merge review。

修正后的纵向计划为：

1. 用户诚意朝向回执与 CameraLock child lineage 仍单独 fail closed；诊断性 180° 不得自动晋级。
2. 将 `AuthoringMesh@2` 接入真实 D1 `rear-stock`，以网格编辑创作轮廓节奏，每次 proposal 继续受六视图 non-regression 约束。
3. 在 secondary Form 通过后，把 Authoring revision 连到 Native High DetailGraph，完成 boolean/bevel/support/subdivision/weighted-normal 的高光节奏门。
4. 以 editable quad Low 而不是 triangle collapse 为产品真值；显式保存 High↔Low correspondence、逐 Part Cage 和 zero-fallback Embree Bake diagnostics。
5. 接通 Hero UV 人工 seam/density/padding 审核与 `MaterialLayerGraph` 真实纹理求值，不以 validated plan 冒充 PBR 资产。
6. 完成第一人称 hip/ADS/inspect/reload/recoil、screen safe region、VFX/audio event 时间线和固定捕获 profile。
7. Unreal-first、Unity-second 做 clean-project import/reimport/restart/packaged-build 同 export hash 回读，最后进入独立资深 Hero Art Review 与 Design Playtest。

当前 source 口径为 **527 schemas / 28 operators / 114 read + 85 opt-in write = 199 tools**；该数量不改变 `Stage=camera-calibrated / QUALITY_TARGET_NOT_MET / commercial=NOT_PROVEN`。

## 0. Goal 北极星：Codex 必须通过 ForgeCAD 完成商业 Hero Weapon

本计划的最终目的不是“让 ForgeCAD 拥有更多工具”，也不是“让一个 GLB 能打开”。唯一产品目标是：

> 用户只安装 ForgeCAD；Codex 经 authenticated MCP 把需求与用户授权参考转成原创、非功能性的商业 FPS Hero Weapon。ForgeCAD 内置模块完成 Art Direction 记录、真正的美术创作网格、High、Low、Hero UV、Cage/Bake、材质层、LOD、第一人称展示、验证和导出；Runtime 保持唯一状态写者。

目标质量是对标《无畏契约》等商业 FPS 的**品质门、可读性和生产纪律**，不是复制其现有武器、皮肤、商标、角色或声音。Riot 的官方公开方法表明，武器质量从 gameplay role、统一设计语言和第一/第三人称识别开始，并由模型、动画、VFX、音频及反复试玩共同完成；因此 ForgeCAD 不能把“单次图生 3D”或“高模渲染漂亮”定义为终点。

### 0.1 三个必须同时交付的产品对象

| 对象 | 必须包含 | 不可替代物 |
| --- | --- | --- |
| `HeroSourceAsset@1` | Brief/ReferenceViewSet、AuthoringMesh、High、editable Low、Hero UV、Cage、8-map Bake、MaterialLayerGraph、LOD/collision/socket 和完整 lineage | evaluated triangle GLB、自动 edge-collapse Low、固定公式贴图 |
| `FpsPresentationPackage@1` | hip/ADS/inspect/equip/reload/recoil、第一/第三人称相机、screen occupancy、reticle/muzzle safe region、animation/VFX/audio cue timeline | 转台 beauty、任意相机、单张截图 |
| `EngineDeliveryPackage@1` | canonical GLB、纹理/KTX2、LOD、collision/socket、命名/轴向/单位、Khronos 验证、目标引擎 round-trip 与资源预算 | Three.js 能加载、文件扩展名正确、source compile PASS |

只有三个对象绑定同一 `project → design_session → candidate → artifact → export` lineage，并由独立 `HeroArtReviewReceipt@1` 通过，才允许 `HERO_ASSET_APPROVED`。

### 0.2 第一性原理：商业质量是闭环，不是算法堆叠

1. **设计决策必须先于几何。** Codex 必须产生可审的 `WeaponArtBrief@1`、设计支柱、独特识别标记、形状语法、材质层级和颜色脚本；算法只能执行这些决定。
2. **Authoring truth 与 evaluated artifact 必须分离。** GLB 是交付/检查工件，不是可编辑源网格；任何 High、Low、UV 和修复都必须回到 Runtime-owned authoring truth。
3. **自动算法只产生 draft。** Quad remesh、UV unwrap、LOD simplify、Bake cage suggestion 和 material generators 都必须进入可编辑、可拒绝的 draft，不得自行 promotion。
4. **第一人称是主审美视图。** 武器在屏幕中的占比、视觉焦点、轮廓、操作节拍和效果安全区必须与 world/ground view 同时验证。
5. **最终判断必须跨系统。** 几何、材质、动画、VFX、音频、引擎和真人评审缺一项，都不能用另一项补偿。

### 0.3 ForgeCAD-only 目标架构

```text
Codex Desktop / CLI
  └─ authenticated MCP typed intents
       └─ ForgeCAD Runtime (唯一写者 / Stage / SQLite / CAS / approval)
            ├─ ArtDirection + ReferenceViewSet compiler
            ├─ Authoring Kernel
            │    ├─ half-edge topology + stable V/E/H/C/F IDs
            │    ├─ selection/constraint sets + mutation journal
            │    └─ original/evaluated separation + correspondence retarget
            ├─ Native High Worker
            │    └─ Boolean / bevel / normal / subdivision / support / floater DetailGraph
            ├─ Retopology Worker
            │    └─ quad draft + ForgeCAD hard-surface constraint/editor pass
            ├─ Hero UV Worker
            │    └─ chart draft + seam/density/padding/tangent policy
            ├─ Cage / Bake Worker
            │    └─ per-Part cage field + ray diagnostics + 8-map bake
            ├─ Surface Worker
            │    └─ MaterialLayerGraph + masks/generators/decals/wear + texture compiler
            ├─ LOD / Packaging Worker
            │    └─ LOD/collision/socket + GLB/KTX2/meshopt + independent validation
            ├─ FPS Presentation Worker
            │    └─ rig/animation/VFX/audio timeline + fixed capture profiles
            └─ Engine Validation Adapter
                 └─ exact export-hash Unreal/Unity studio runner receipt

ForgeCAD Art Director Viewer (只读)
  └─ authoring/topology/UV/cage/bake/material/FPS/engine/human evidence
```

Worker 可以链接经过 adoption 的开源库，但只接受 closed typed request、固定版本和资源预算；不监听网络、不读取 SQLite、不接受 Python/JavaScript/shell/URL/任意路径，也不能把库内部状态写成第二真值。

### 0.4 创作真值模型

商业生产不能只靠 `GeometryProgram → GLB`。目标合同分六层：

1. **Design truth**：`WeaponArtBrief@1`、`WeaponDesignLanguage@1`、`ReferenceViewSet@1`、`ProductionCameraLock@1`、`DesignDecisionLog@1`；
2. **Authoring truth**：`AuthoringMesh@2`、`TopologyMutationJournal@1`、`StableElementIdentity@2`、`SelectionConstraintSet@1`、`ModifierDetailGraph@1`；
3. **Production derivatives**：`HighMeshArtifact@2`、`LowMeshArtifact@2`、`HighLowCorrespondence@2`、`HeroUvLayout@2`、`CageField@1`、`BakeSet@1`；
4. **Surface truth**：`MaterialLayerGraph@1`、`MaterialMaskSet@1`、`TextureBuild@2`、`SurfaceResponseReport@1`；
5. **Presentation/delivery truth**：`WeaponPresentationRig@1`、`FpsPresentationPackage@1`、`HeroLodSet@1`、`EngineDeliveryPackage@1`、`EngineValidationReceipt@1`；
6. **Approval truth**：`HeroArtReviewReceipt@1`、用户 approval、immutable version、export/restart receipt。

所有层都必须保存 source/evaluated hash、producer cohort、输入 binding、可回退关系和明确 `draft/reviewed/approved/rejected` 状态。下游只能读取上游 approved artifact；用户要求“跳过当前阶段”时，只允许产生 `preview_only=true` 的隔离展示，不得推进 Stage 或 promotion。

### 0.5 开源采用的最优职责划分

| 能力 | 首选实现 | ForgeCAD 必须自己完成的部分 | 采用结论 |
| --- | --- | --- | --- |
| Manifold Boolean | 固定 revision Manifold C API | same-Part policy、stable source IDs、post-Boolean cleanup、资源/恶意输入 Gate | 已有 bounded accepted slice；不能扩写为通用 artist Boolean |
| Subdivision | OpenSubdiv 作为可替换 evaluator 候选；保留 product-owned CPU fallback | crease/support policy、DetailGraph、stable IDs、high-light quality | `Tomorrow Open Source Technology License 1.0`，先法务/体积/确定性评估；未采用 |
| Quad draft | QuadriFlow 仅生成初稿 | Part/孔洞/radial loop/hard-edge/seam/support-flow locks、交互修订、promotion | 仓库 README 与 `LICENSE.txt` 标签不一致；按实际许可证文本和 transitive SBOM 重新审计，当前 snapshot-blocked |
| UV draft | xatlas | artist seam、first-person texel weighting、orientation、mip padding、UV0/UV1、review/promotion | MIT；优先进入隔离 benchmark，不直接成为 Hero UV truth |
| Cage/Bake rays | Embree | correspondence cage、offset/skew policy、Part isolation、miss/cross-hit heatmap、8-map semantics | Apache-2.0；通过 CPU/package/resource Gate 后采用 |
| Material graph | ForgeCAD `MaterialLayerGraph`；MaterialX 仅作 typed interchange/lowering subset | painter-like layer/mask/generator/anchor/decal/wear、provenance、deterministic texture build | MaterialX Apache-2.0；不执行任意 shader/plugin |
| Image/color | OpenImageIO + fixed OpenColorIO config | codec allowlist、memory budget、channel/color-space policy、CAS lineage | OIIO Apache-2.0、OCIO BSD-3-Clause；隔离 Worker 候选 |
| LOD/geometry compression | meshoptimizer | authored LOD policy、Part/UV/seam/socket locks、silhouette/highlight error | MIT；只能处理已批准 Low/LOD，不可冒充 retopo |
| Texture compression | Basis Universal/KTX2 | slot/color-space policy、normal quality profile、platform fallback | 固定 encoder/profile/decoded-hash Gate 后采用 |
| GLB optimization | fixed glTF Transform operation allowlist | canonical source GLB 保留、操作前后 semantic/hash diff、无用户脚本 | MIT；作为受限 Packaging Worker/开发 oracle，不成为 authoring truth |
| Format validation | Khronos glTF Validator + ForgeCAD strict readback | candidate/export binding、semantic Part/MaterialZone、engine profile | Apache-2.0；Validator 只证明 glTF conformance，不证明商业质量 |

### 0.6 可执行关键路径

以下阶段沿用 `CQ-01…CQ-11` 与 ADR-0027 的 `FPS-*` 任务，不建立第二套并行任务。每阶段先做到 Contracts/Worker/Runtime/MCP **可编译**，随后必须立即以一个固定 Hero Weapon candidate 完成真实资产回执；禁止长期堆积只通过 source 的横向能力。

| 顺序 | 当前/下一里程碑 | 代码交付 | 真实退出门 |
| ---: | --- | --- | --- |
| 0 | Goal/Art Direction reset | Brief/DesignLanguage/DecisionLog contracts 与 Viewer read model | 一把原创武器的 approved brief、五核心视图、FPS/world识别标记和预算 |
| 1 | CQ-01 / `FPS-FORM-*` | durable FormArt、hash-bound raster attribution、单 source-node RepairIntent | 当前真实 D1 取得 `secondary-form-approved`；否则后面全部 locked |
| 2 | CQ-02 AuthoringMesh | half-edge mutation journal、selection/constraint sets、undo/checkpoint、evaluated retarget | 同一 D1 完成可见局部 loop/bridge/inset/extrude/bevel 支持流编辑与 restart readback |
| 3 | CQ-03 High | non-destructive DetailGraph、Manifold seam、bevel/normal/subdivision/support/floater | fixed FPS/light rig 下高光连续、无 pinching/波纹/意外轮廓变化，`FPS-HIGH-05` 通过 |
| 4 | CQ-04/05 Low + UV | Quad draft adapter、hard-surface constraint/editor、xatlas adapter、Hero UV policy | artist/Codex 可修 Low、High↔Low correspondence、2K/4K UV 审核通过 |
| 5 | CQ-06 Cage/Bake | Embree adapter、CageField、8-map BakeSet、diagnostic heatmaps | coverage 达标，ray miss/fallback/cross-Part/skew/padding 全部门槛通过且可局部返修 |
| 6 | CQ-07 Material | MaterialLayerGraph、mask/generator/decal/wear、OIIO/OCIO texture compiler | 多灯光、FPS/world视图的材质层级与 roughness readability 通过 |
| 7 | CQ-08 FPS | PresentationRig、动画/VFX/audio事件、安全区和固定 capture profiles | hip/ADS/inspect/equip/reload/recoil 无穿插、无关键遮挡，节拍可读 |
| 8 | CQ-09 Delivery | LOD/collision/socket、meshopt、KTX2、fixed glTF Transform、Validator | canonical GLB 与所有 derived package semantic diff 受控；预算通过 |
| 9 | CQ-10/11 Engine/Human | Unreal-first studio runner、Unity profile、Art Review workflow | exact export hash 的 engine round-trip、独立资深艺术家修订闭合、用户 confirm/restart |

### 0.7 进度记账规则

- 进度按**同一真实资产穿过多少个已批准 Gate**计算，不按 Schema、工具、测试或文档数量计算。
- `PASS_COMPILE` 只说明代码能构建；`PASS_SOURCE` 只说明受限实现存在；`PASS_ASSET` 才说明真实 candidate 通过该阶段；最终仍需 `PASS_ENGINE` 和 `PASS_HUMAN_ART_REVIEW`。
- 每阶段只保留一个 active Hero candidate 和一个已批准 baseline；并行 Agent 只能研究、实现独立模块或只读审计，不能并行写同一 candidate。
- 测试只覆盖高风险合同、崩溃/资源/确定性、hash/lineage 和真实关键链；不为提高数量编写重复 fixture，也不以反复全量测试替代一次真实资产验收。
- 文档、Viewer、source Gate 或漂亮渲染不得推进 `ProductionStage@3`；只有 Runtime-owned receipt 和明确真人/用户批准可以推进。

### 0.8 商业资产的硬合同：可编辑、可烘焙、可表现、可进引擎

Authoring/High/Low 不是同一个三角网格的三个名字。ForgeCAD 必须维持四条可回溯真值：

```text
AuthoringMeshRevision
  ├─ HighRecipe → HighEvaluatedArtifact
  ├─ AutoRetopoDraft → LowAuthoringMesh（人工/Codex 局部修订后批准）
  ├─ HighLowCorrespondence → CageArtifact → HighLowBakeReceipt
  └─ MaterialLayerGraph → FpsPresentationPackage → EngineDeliveryPackage
```

- `AuthoringMeshRevision@1` 使用 half-edge/corner 数据、Runtime 生成且不复用的 element ID、tombstone 与不可变 operation DAG；evaluated element 采用独立命名空间，只能以显式 lineage 回指 source element。
- `AutoRetopoDraft@1` 永远是 `DRAFT_UNREVIEWED`。商业 Low 必须是可编辑 `LowAuthoringMesh@1`，至少支持 loop/cut/slide、bridge、inset、extrude、dissolve、relax 和 surface project；自动 triangle collapse 或 quad remesh 不能直接 promotion。
- `HighLowCorrespondence@1` 必须保存 Low corner/face 到 High surface 的 barycentric/parametric binding、Part/Feature/MaterialZone、confidence 和局部失效范围；最近顶点或 nearest-surface fallback 只能用于诊断。
- `CageArtifact@1` 与 Low 保持相同 topology/face order，但拥有独立 artifact/hash、per-vertex 或 per-corner offset、front/rear distance、per-region override、caster/receiver mask。任何 miss、nearest fallback、未授权 cross-Part hit、cage penetration 或 owner bleed 都使正式 Bake fail closed。
- Hero UV 的 padding 必须由过滤半径、目标 streamed mip 和 block compression margin 推导，不能把固定 8 texel 当成跨分辨率答案；dilation 必须按 island/Part/MaterialZone ownership 记录 primary/dilated coverage 和 bleed count。
- `MaterialLayerGraph@1` 采用 closed typed DAG：`Source/Constant/Anchor/Generator/Mask/Filter/Transform/Blend/NormalCombine/RoughnessRemap/ChannelPack/Output`。每个 generator 必须有 seed、domain、budget 和 provenance；磨损必须由接触、摩擦、热或明确艺术 anchor 驱动，不能靠无锚随机噪声冒充商业层次。

静态 Hero Asset 与 Premium Skin Experience 分开验收：前者要求模型、UV、Bake、材质和 LOD；后者还必须闭合 inspect/equip/reload/recoil、VFX、音频与 gameplay beat。两者都需要第一/第三人称和 ground-pickup 固定镜头，不能由转台 beauty 替代。

`EngineValidationReceipt@1` 至少绑定 `engine/version/runner build/clean-project hash/export hash/import settings`，回读 mesh/material/tangent/LOD/collision/socket/animation/event，并记录 packaged build 的 draw call、texture/GPU memory、streaming 与 frame-time `p50/p95/p99`。Unreal-first 是首个产品化 profile，Unity 为第二 profile；目标引擎未安装或 runner 不可用时只能记录 `NOT_RUN/ENGINE_VALIDATOR_UNAVAILABLE`。

## 商业级 11 组质量/交付路线

商业 Hero Weapon 必须沿同一 `candidate_hash → export_hash` 闭合全部 11 组质量门；前一组未通过时，后一组只能做隔离诊断，不能推进 confirm、version 或 export。11 组是商业验收与工程工作分解，不是第二套 Runtime 状态机；唯一 Stage 晋级顺序仍是 ADR-0027/`ProductionStage@3` 的 19 个值，其中 `hero-art-review-approved` 先于 `engine-validated`。下表为研究与交付分组顺序；任何实际 Stage write 都必须服从 19 状态的 1→19 顺序。`MCP010F` 的 source/transport/Viewer/Three.js 证据不改变这条边界。

| # | 阶段 | 退出门 | 当前状态 |
|---:|---|---|---|
| 1 | Art Direction / ReferenceViewSet | `WeaponArtBrief@1`、style pillars、授权/IP与平台预算；front/back/left/right/rear-three-quarter 的 reviewed ReferenceViewSet、CameraLock、silhouette/negative-space/landmark 与 observed/inferred/unknown 分层 | `Stage=camera-calibrated`；CrossView=`QUALITY_TARGET_NOT_MET`、`secondary-form-approved=NOT_CREATED`、depth=`UNKNOWN`；`HQ360=BLOCKED_REFERENCE_COVERAGE` |
| 2 | AuthoringMesh | Runtime-owned original/evaluated 分离；稳定 vertex/edge/half-edge/corner/face/loop/ring/boundary ID；可编辑局部操作、source map 与 High↔Low correspondence | split/collapse/dissolve 独立 durable full-chain **3/3 PASS**；general correspondence、evaluated retarget、完整编辑历史与商业 editor `NOT_PROVEN` |
| 3 | High | 非破坏 `HighMeshArtifact@1`/`DetailGraph`、布尔/倒角/support loop/weighted normal/Subdivision/floater、高光连续与 strict GLB readback | source-only structural/durable slice；`FPS-HIGH-05=NOT_PASSED`、proposal=`registered=false`，不解锁后续阶段 |
| 4 | Low / Retopo | artist-authored editable quad flow；hard edge/UV seam/Part边界约束；High↔Low correspondence 与 bake-ready topology | candidate-bound `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`；prepare replay→drop/reopen→get **1/1 PASS**，artist review与 packaged same-cohort `NOT_RUN` |
| 5 | Hero UV | 2K/4K texel density、seam/hard-edge congruence、stretch/overlap/OOB、mip padding、UV0/UV1 与 tangent/Mikk replay | 7 contracts 已接 Store/Runtime/MCP public `hero_uv_durable_get/prepare`；prepare/replay/drop/reopen/get **1/1 PASS**、4 CAS roots linked/GC；artist UV review、package、engine `NOT_RUN/NOT_PROVEN` |
| 6 | Cage / Bake | topology-correspondent Cage、per-vertex/per-region offset、front/back ray、miss/fallback/cross-part/skew/penetration/dilation 及 8 类 maps 全部通过 | fixed Worker 与七记录 Store/MCP public seam 已 source PASS；但新建 prepare 因正式 High/Low/Hero UV/Cage/Correspondence/Plan/Diagnostic producer 未闭合而返回 `PRODUCTION_WEAPON_HIGH_LOW_BAKE_PRODUCER_UNAVAILABLE` 且零写。当前 D1 无正向 receipt，正式 Cage/Bake=`NOT_PASSED`；旧 36.25% coverage/miss/fallback/cross-part/padding=0 仅是失败诊断 |
| 7 | Material | `MaterialLayerGraph@1` 的 Layer/Mask/Generator/Decal/Wear/Microdetail、roughness hierarchy、通道色彩空间与 provenance，多视图材质一致 | 当前仅 **4 MaterialZones / 6 formula textures**；embedded PBR/2K 是 structural preview，commercial PBR=`NOT_PROVEN` |
| 8 | LOD / Collision / Socket | authored LOD0/1/2、轮廓/属性误差、collision/socket、同 hash 资源与平台预算 | 仅有局部 structural delivery；商业 LOD/collision/socket 与性能预算 `NOT_RUN` |
| 9 | Viewer / animation / VFX / audio validation | Viewer 同 candidate/read model、first/third-person fixed cameras、animation、VFX、audio timing/readability 与 accessibility evidence | Viewer source/read-model smoke 可记录为结构证据；Three.js 仅结构消费，animation/VFX/audio、正式 VoiceOver与真人观看 `NOT_RUN` |
| 10 | Engine | Unreal 或 Unity 实际 importer/material/tangent/LOD/collision/socket/animation round-trip，draw-call/texture/memory/frame-time/streaming budget | **Unreal/Unity 均未运行**；`commercial engine=NOT_RUN`，Three.js 不能替代引擎验收 |
| 11 | Independent Hero Art Review | 独立资深 Hero Art Review/盲审、修订清单闭合、同 `export_hash` 的 confirm/version/export/restart readback | `human=NOT_RUN`，不存在 `PASS_HUMAN_ART_REVIEW`；只有 1–11 全通过才允许 `HERO_ASSET_APPROVED` |

终态动作不是第 12 阶段：只有上述 11 门在同一 candidate/export hash 通过，才允许用户批准后 `confirm → immutable version → export → restart readback`。任何 source/transport/Three.js/Codex self-review PASS 都不能替代正式 Cage/Bake、Unreal/Unity 或独立 Hero Art Review。

## 1. 决策摘要

ForgeCAD 当前是可验证、可回退、typed 的 3D Runtime 骨架，还不是商业级游戏武器资产生产软件。当前 **515 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 tools** 只能证明协议表面。AuthoringMesh split/collapse/dissolve 独立 full-chain **3/3 PASS**；High 的 durable source链已通过，Low 为 candidate-bound `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false` 且 prepare replay/drop-reopen/get **1/1 PASS**，Hero UV public `get/prepare` 已接 Store/Runtime/MCP。它们仍仅 structural/source；artist UV review、packaged same-cohort、visual/human/engine/commercial 仍 `NOT_RUN/NOT_PROVEN`，商业质量继续 `QUALITY_TARGET_NOT_MET`。

商业 Hero Weapon 必须由同一 candidate/export hash 的完整生产链共同证明：

```text
Brief / Art Direction
  → Reviewed ReferenceViewSet + CameraLock
  → Blockout / Primary / Secondary Form
  → AuthoringMesh + non-destructive High
  → editable Low / Retopology
  → Hero UV
  → topology-correspondent Cage
  → High-to-Low Bake
  → typed Material Layer Graph
  → FPS cameras / animation / VFX / audio
  → LOD / collision / sockets
  → commercial engine round-trip
  → independent Hero Art Review
  → confirm / immutable version / export
```

任一阶段缺失或失败，最终状态继续保持 `QUALITY_TARGET_NOT_MET`、`NOT_PROVEN`、`NOT_RUN` 或对应 `BLOCKED_*`；不得用后续材质、灯光、VFX、GLB 可打开、Schema 数量或 Codex 自评补偿前序失败。

## 2. 当前证据基线

当前 2026-08-26 总账仍只能支持“结构性高级灰模”结论；下列数值来自 2026-08-25 standalone 2K 历史失败诊断，不是 2026-08-26 formal Cage/Bake positive receipt：

- 当前 provisional High 为 23 Parts / 2,280 triangles；Low/Cage 为 1,000 triangles；
- Low 是确定性 triangle edge-collapse，不是 artist-authored quad topology；`artist_authored_quad_topology=false`、edge flow=`NOT_PROVEN`；
- 2K geometric bake coverage=`0.3625035285949707`，ray miss=`45,386`、nearest-surface fallback=`107,063`、cross-part hit=`3,982`、Bake padding=`0`；
- Hero material 只有 4 MaterialZones / 6 embedded 2K textures，主要由固定公式和 curvature 推导，不是可编辑 Layer/Mask/Generator/Decal/Wear/Microdetail 系统；
- Three.js r185 实际回读通过，但它只是 web 3D consumer；commercial engine round-trip=`NOT_RUN`；
- human Hero Art Review、confirm/version/export 和 HQ360 均未通过或未运行；
- 当前真实 D1 仍停在 `camera-calibrated`，FormQuality/secondary-form 未通过，depth 仍 `UNKNOWN`。

证据入口：

- `docs/evidence/mcp010f/production-weapon-real-2k-geometric-bake-hero-render-threejs-source-20260825.json`
- `docs/evidence/mcp010f/production-weapon-provisional-surface-topology-material-pass-20260825.json`
- `apps/geometry-worker/src/production_low_retopology.rs`
- `apps/geometry-worker/src/production_geometric_bake.rs`
- `apps/geometry-worker/src/production_hero_material.rs`

## 3. 商业质量乘法模型

商业质量按乘法而不是加法理解：

```text
CommercialWeaponQuality =
  DesignIntent
  × FormReadability
  × EditableTopology
  × UVTangentCorrectness
  × BakeReliability
  × MaterialBelievability
  × FPSPresentation
  × EngineValidation
  × HumanArtApproval
```

任何因子为零，结果仍不是商业资产。这个模型用于停止“继续堆 operator、三角形、贴图分辨率或发光强度”的错误优化方向。

## 4. 设计方法

### 4.1 Brief 与 Art Direction

建模前必须先形成 `WeaponArtBrief@1` 目标合同：

- gameplay role、目标玩家感受、风格关键词和世界观；
- 第一人称、第三人称和地面拾取时的唯一识别标记；
- silhouette、negative-space、plane rhythm、material hierarchy 和 color script；
- 目标平台、triangle/texture/draw-call/memory 预算；
- 授权、原创性与 IP 边界；
- observed / inferred / unknown 分离。

没有明确 Brief、概念探索和艺术决策时，不得直接从单张图片进入细节建模。

### 4.2 多视图与相机

核心 reference coverage 至少为 `front/back/left/right/rear-three-quarter`；top 可作为扩展视图，bottom 只能作为 calibration helper。每个视图必须绑定 source hash、crop、orientation/registration、closed outer contour、negative-space contour、camera/FOV 和审核状态。

固定审查相机至少包括：

- first-person hip；
- ADS；
- inspect；
- equip；
- reload；
- recoil；
- third-person；
- ground pickup；
- orthographic/reference views。

### 4.3 形体与细节层级

形体顺序固定为：

1. silhouette / overall proportion；
2. primary mass；
3. negative space / structural openings；
4. secondary form / plane rhythm；
5. tertiary detail / panel / vent / seam；
6. bevel/highlight continuity；
7. surface microdetail。

材质、VFX 和灯光不能解锁尚未通过的 silhouette、negative-space 或 primary/secondary-form Gate。

### 4.4 曲面过渡与高光语言

商业级硬表面不是“平面 + 随机倒角”。ForgeCAD 必须显式表达并验证：

- primary plane 之间的切线关系、曲率过渡和高光宽度；
- hard break、soft break、rolled edge、tensioned surface 和 stamped panel 的设计意图；
- 相邻 Part 连接处的切线连续、间隙、压层和遮挡关系；
- 固定 key/fill/rim 与 matcap 下的高光连续，以及 normal/curvature/wireframe AOV 的交叉审查；
- 不允许用 weighted normal 或贴图隐藏轮廓、自交、波纹和大平面凹陷。

必需的新能力是 `SurfaceContinuityIntent@1`、`HighlightFlowReport@1` 和固定灯光下的曲率/高光过渡热图。当前这些都没有形成 candidate-bound durable Gate。

### 4.5 倒角密度与视觉尺度

倒角必须由目标距离和像素覆盖控制，而不是全局统一半径。每条可见边需记录 `edge role + viewing distance + target pixel width + material class + LOD policy`，并区分：

- 轮廓/主高光边：确保 first-person 距离下可读；
- 结构接缝和面板边：必须服务装配层次，不争抢主轮廓；
- 微小硬边：优先烘焙到 Normal，不得无限增加几何密度；
- 软材料/橡胶包胶：使用更宽过渡和不同 roughness response；
- LOD 切换：优先保留轮廓和主高光，再删除内部支撑环与小浮雕。

软件缺口是可视距离感知的 `BevelDensityPolicy@1`、按 Part/edge role 的局部预览和跨 LOD 高光保真报告。当前 `bevel@2` 只是有界的读取/降级表面，不等于这个商业倒角系统。

### 4.6 拓扑流与 High/Low 对应

拓扑质量必须根据资产用途审查，不能只看 triangle count：

- High 需支撑回路、均匀曲率采样、合理 pole 位置、Boolean 后清洁面流与无夹杂面；
- Low 需以 silhouette、deformation、first-person visibility、hard edge、UV seam、material/Part 边界和 bake skew 为约束的 artist-editable quad flow；
- High↔Low↔Cage 需持久 correspondence，能从 bake miss/cross-hit/skew 回溯到局部面、边和修复意图；
- normal split、tangent、UV seam 和 hard edge 必须成套验证；
- 自动 retopo 只能产生 draft，不得直接写成 artist-authored topology PASS。

当前 split/collapse/dissolve 证明了有界稳定 ID 与完整重启链，但 bridge/inset/extrude、loop/ring selection、evaluated retarget、跨版本 selection/undo 和商业 Low edge flow 仍未证明。

### 4.7 材质层次与表面叙事

材质质量需先在灰度和 roughness 上建立层次，再添加颜色、贴花和磨损：

1. 一级：主金属、次金属、聚合物/橡胶、透光/能量材质的可读分区；
2. 二级：同类材质内的 roughness、coating、anisotropy 和 edge response 变化；
3. 三级：制造方式、表面处理、污垢、油脂、局部磨损、decal 与 micro-normal；
4. 故事层：只在接触、操作、遮挡、热区和维修逻辑支持时添加磨损，禁止随机噪点；
5. 展示层：first-person 主视区、ADS 视线和地面拾取距离下均保持材质可读。

必需的产品功能是 `MaterialLayerGraph@1`、mask/generator/anchor/filter/blend、通道打包、颜色空间策略、mip 预览和按材质类别的 roughness range Gate。当前 4 zones / 6 textures 的固定公式只是 preview，不是商业材质系统。

## 5. 必须新增或升级的软件能力

| 能力 | ForgeCAD 产品合同 | 商业退出门 | 当前状态 |
|---|---|---|---|
| Art Direction | `WeaponArtBrief@1`、`ConceptVariantSet@1`、`ArtDecisionReceipt@1` | 设计语言、轮廓、第一/第三人称识别与原创性人审通过 | target / missing |
| Authoring Mesh | `AuthoringMesh@1` + durable canonical/artifact/link contracts + `AuthoringMeshIdentityLineage@1` | half-edge；稳定 vertex/edge/face/corner ID；quad/ngon/tri；loop/ring/boundary；局部编辑；original/evaluated 分离 | typed split/collapse/dissolve 独立 full-chain 3/3 PASS；general correspondence、evaluated retarget、完整编辑历史与产品 editor `NOT_PROVEN` |
| Native High | `HighMeshArtifact@1`、`DetailGraph@1` | Boolean、bevel、weighted normal、Subdivision、crease、support loop、floater；高光连续 | deterministic artifact/embedded GLB/strict readback + 有界 chamfer arc + Store/Runtime restart + public MCP source PASS；完整 support-loop topology、weighted normal/Subdivision与 packaged/candidate quality `NOT_PASSED` |
| Editable Low | `LowMeshArtifact@1`、`RetopologyConstraintSet@1` | 可编辑 quad flow；Part/hard-edge/hole/bevel/first-person 约束；High↔Low correspondence | candidate-bound current provenance；`DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`；prepare replay/drop-reopen/get **1/1 PASS**，artist edge-flow review与packaged same-cohort `NOT_RUN`，仍不可晋级 |
| Hero UV | `HeroUvLayout@1` | 2K/4K、visibility-weighted texel density、seam/hard-edge congruence、stretch、overlap/OOB、mip padding、UV0/UV1、Mikk replay | public `hero_uv_durable_get/prepare` 已接 Store/Runtime/MCP，四个 CAS roots linked/GC；仅 structural/source，artist UV review、packaged same-cohort、engine tangent round-trip `NOT_RUN/NOT_PROVEN` |
| Cage | `CageArtifact@1` | Low-topology correspondent；per-vertex/per-region offset；front/back ray；intersection/skew/penetration 可修正 | exact Low topology/order Cage Worker 已 source PASS；正式 Runtime-owned producer 与当前 D1 artifact receipt 不存在，`NOT_PASSED` |
| Bake | `HighLowBakeReceipt@1` | Tangent Normal/AO/Curvature/Thickness/Position/Object/Material/Part ID；miss/fallback/cross-hit/skew heatmap；8-texel dilation；CAS/hash | 8-map fixed Worker、七子表原子 Store seam、public get/prepare 与 exact replay 已 source PASS；Formal High internal materializer 已存在，但完整 positive restart/public surface 与其余正式 lineage/current-D1 receipt 未闭合，故新 materialization 零写失败，商业质量仍 `NOT_PASSED` |
| Surface | `MaterialLayerGraph@1`、`HeroMaterialPack@1` | Layer/Mask/Generator/Anchor/Filter/Blend、roughness hierarchy、decal、wear、microdetail、channel packing、provenance | fixed-formula preview only |
| FPS content | `FirstPersonPresentationSet@1` | hip/ADS/inspect/equip/reload/recoil camera、animation、VFX、audio timing/readability | `NOT_RUN` / structural fragments |
| Delivery | `HeroLodSet@1`、`CollisionSet@1`、`SocketSet@1` | LOD0/1/2 identity、silhouette error、UV/tangent/material continuity、collision/socket readback | partial / `NOT_RUN` |
| Engine | `EngineValidationPlan@1`、`EngineValidationReceipt@1` | Unreal 或 Unity 实际 importer、material、tangent、LOD、socket、animation、collision、memory/draw-call | `NOT_RUN` |
| Human | `HeroArtReviewReceipt@1` | 独立资深武器艺术家盲审、原创/IP、玩家实机反馈、同 export hash | `NOT_RUN` |

### 5.1 商业成熟度分级

| 等级 | 可以对外宣称 | 必要证据 | ForgeCAD 当前位置 |
|---|---|---|---|
| L0 结构原型 | typed 几何/材质/交付子链可重放 | 合同、hash、CAS/Store、strict readback | 多个子链已达成，Native High durable/MCP 为 source-only PASS；不代表 L1–L4 |
| L1 高级灰模 | 固定视图下形体可评审 | approved Form、primary/secondary planes、negative space、高光连续 | **未达成**；当前 FormQuality 失败 |
| L2 Portfolio Hero | 静态展示级英雄资产 | High/Low/UV/Bake/PBR、多视图渲染、独立艺术审核 | **未达成** |
| L3 Commercial FPS | 可进商业 FPS 内容管线 | first-person/world model、动画/VFX/音频、LOD/collision/socket、Unity/Unreal 实机、性能预算 | **未达成** |
| L4 Shipping/Live | 可交付、可更新、可回滚的上线资产 | 同 hash 审批/版本/导出/重启、打包、许可证、跨类别人评 | **未达成** |

与《无畏契约》同类商业 FPS 武器对标时，目标是 L3/L4；当前不应使用“接近商业级”的描述。下一个真实里程碑是先达到 L1，而不是通过增加材质、VFX 或渲染数量跨级到 L3。

### 5.2 AuthoringMesh canonical 决策

商业终态的 `AuthoringMesh@1` 必须是 Runtime-owned、单 Part、可编辑的 original half-edge 真值；evaluated 只保存固定 Worker 产生的 artifact/readback 引用，不能把三角 GLB 或 `TopologySnapshot@1` 反向当成 authoring source。稳定 ID 需要覆盖 vertex/edge/half-edge/corner/face/loop/ring/boundary，保留元素跨 revision 保持 ID、删除 ID 永不复用、新 ID 由 mesh lineage + operation ID + parent IDs 确定性派生。

Runtime validator 必须 fail closed 验证 `next/prev` 互逆与完整 face cycle、twin 对称且方向相反、boundary edge 只有一个 half-edge、内部 edge 恰有两个 half-edge、无非流形/悬空引用/零面积/重复元素、original/evaluated ID namespace 分离以及同 cohort 双 replay exact。Translate、single-face extrude、split、collapse、dissolve、bridge 按风险顺序逐步开放；每次编辑都产生新 candidate，prepare 不得直接 confirm/version/export。

当前公开 V1 `AuthoringMesh@1` 仍明确 `cross_version_stable=false`；Runtime 不接受 caller identity/proof arrays。split/collapse/dissolve 独立 full-chain 已通过；general correspondence、evaluated retarget、完整编辑历史及产品级 cross-version stable identity仍 `NOT_PROVEN`。

## 6. 原生 Worker 目标架构

```text
Codex Desktop
  ↓ authenticated MCP stdio
ForgeCAD Runtime                      only product-state writer
  ├─ Authoring Mesh Kernel
  ├─ Shape / High Worker
  ├─ Retopology / Low Worker
  ├─ UV Worker
  ├─ Cage / Bake Worker
  ├─ Surface / Texture Worker
  ├─ LOD / Collision Worker
  ├─ Render / AOV / FPS Review Worker
  └─ GLB / Commercial Engine Validator
```

Worker 可以是 ForgeCAD App 包内隔离子进程，但不得联网、访问 SQLite/CAS、接收任意 Python/JavaScript/shell/URL/路径或成为第二状态写者。所有永久结果仍由 Runtime 通过 typed request/result、CAS、candidate、Stage、approval 和 immutable version 管理。

## 7. 外部算法采用路线

以下项目只表示 `approved-for-evaluation` 或 `benchmark-first`，除已另有 accepted receipt 的 Manifold/MikkTSpace 外，不表示已进入产品：

| 项目 | 目标用途 | 首个 Gate |
|---|---|---|
| [Manifold](https://github.com/elalish/manifold) | robust Boolean | 已有 bounded accepted slice；继续补 generic production constraints、lineage 和 resource benchmark |
| [OpenSubdiv](https://github.com/PixarAnimationStudios/OpenSubdiv) | Subdivision/crease/UV boundary | 固定 revision、许可证/体积/CPU-GPU/确定性/打包 benchmark |
| [QuadriFlow](https://github.com/hjwdzh/QuadriFlow) | automatic quad draft | `BUILD_FREE_LICENSE=ON`、依赖/SPDX、determinism、hard-surface constraint pass |
| [xatlas](https://github.com/jpcy/xatlas) | UV unwrap/pack | seam/density/stretch/overlap/padding/cross-platform benchmark |
| [Embree](https://github.com/RenderKit/embree) | Cage/Bake ray kernel | CPU feature、package size、malicious mesh、front/back hit、determinism/resource Gate |
| [MaterialX](https://github.com/AcademySoftwareFoundation/MaterialX) | typed material graph interchange | glTF metallic-roughness subset、no shader execution、deterministic lowering |
| [OpenImageIO](https://github.com/AcademySoftwareFoundation/OpenImageIO) | image IO/mipmap/channel processing | codec allowlist、malicious image、memory、color/channel semantics |
| [OpenColorIO](https://github.com/AcademySoftwareFoundation/OpenColorIO) | color management | fixed config、sRGB/linear/ACES policy、no dynamic plugin/config path |
| [meshoptimizer](https://github.com/zeux/meshoptimizer) | LOD/cache/mesh optimization | Part/MaterialZone/seam/socket locks、silhouette/attribute error、determinism |
| [glTF Validator](https://github.com/KhronosGroup/glTF-Validator) | delivery validation | fixed version、offline bundled invocation、normalized report、malicious GLB limits |

所有 adoption 必须固定 revision、LICENSE/NOTICE/SBOM/provenance、可移除策略、negative/security、determinism、resource、package 和 benchmark receipt；不得把 GitHub 仓库直接安装为 Skill 或 Runtime 插件。

## 8. 实施顺序

在不新增第二个 `in_progress` 原子任务的前提下，后续工程顺序固定为：

1. 完成当前 FormQuality：修复 reference registration、Part-ID owner binding、主形和负空间；取得 `secondary-form-approved`；
2. 在已完成 typed split/collapse/dissolve 三操作独立 full-chain 之上，补 bridge/inset/extrude 等闭合关系、evaluated retarget 与完整编辑历史；
3. 完成 Native High：Manifold + bevel/weighted normal/OpenSubdiv evaluation；
4. 完成 Native Low：QuadriFlow draft + ForgeCAD hard-surface constraint/editor；
5. 完成 Hero UV：xatlas draft + ForgeCAD policy/validation；
6. 完成 Cage/Bake：先闭合 Runtime-owned 七类正式 producer 与当前候选正向 receipt；Embree 仅在 adoption Gate 通过后作为 per-Part ray kernel 内部实现，不能成为完成前提或独立真值；
7. 完成 Surface：MaterialX/OIIO/OCIO typed subset + offline AssetPack；
8. 完成 LOD/Collision：meshoptimizer candidate + authored review；
9. 升级 Viewer 为 Art Director mode；
10. 完成 first-person animation/VFX/audio、commercial engine 和 independent human review；
11. 仅在同一 export hash 全门通过后 confirm/version/export。

当前 FormQuality 的停止条件已经进一步明确：`FPS-FORM-04Y` 只证明左/右/后 3/4 当前 registration identity 唯一，不能替代 owner-void Gate；`FPS-FORM-04Z` 的两个 upper-profile station 方向都因 rear-three-quarter adjacency `248 < 250` 且非零 intrusion 被停止。source-map/triangle attribution 的 source path 已绑定 exact durable FormArt/candidate/reference/camera/render evidence，但真实 D1 execution 尚未运行；获得合法 D1 FormArt readback 后必须据归因结果设计具明确形体语义的 bounded correction，不得继续盲扫 profile 标量。两项均未创建 `secondary-form-approved`，也没有解锁 AuthoringMesh、High 或后续阶段。

上述 11 项是工程 workstream，不替换 ADR-0027 已接受的 19 个 `ProductionStage@3`。两者映射如下：Form/Art Direction 覆盖 stage 1–6；AuthoringMesh 是 stage 6→7 的生产前置而不是额外 Stage；Native High/Low/Hero UV 分别对应 stage 7/8/9；Cage/Bake 对应 10/11；Material 对应 12；FPS presentation 覆盖 13–15；LOD/Collision 对应 16；独立 Hero Art Review 对应 17；commercial engine 对应 18；同 hash confirm/restart/export 对应 19。工程模块可以提前开发，Stage head 仍只能按 1→19 顺序晋级；因此下表的 Engine/Human 研究分组排列不得被用于越过 `hero-art-review-approved → engine-validated` 的 Runtime transition。

最终 P0 用户只安装 ForgeCAD。任何通过评估的第三方算法都必须被固定 revision、许可证/SBOM/provenance、资源预算、确定性与恶意输入 Gate 包装进 ForgeCAD 自带的 typed Worker；用户和 Runtime 都不得依赖外部 Blender、Substance、Maya、任意脚本插件或联网素材服务。

## 9. 统一状态与禁用捷径

所有相关文档、receipt 和 UI 必须分别记录：

- `structural_status`；
- `visual_status`；
- `human_status`；
- `engine_status`；
- `distribution_status`。

禁止：

- 用工具/Schema/triangle/texture 数量代表质量；
- 用自动 decimation 冒充 artist-authored Low；
- 用 self-surface bake 或 nearest fallback 冒充正式 High-to-Low Bake；
- 用固定公式颜色/粗糙度冒充 Hero material authoring；
- 用 Three.js、GLB Validator 或单张渲染冒充商业引擎验收；
- 用 Codex 自评代替独立艺术家审核；
- 在 Form/High/Low/UV/Cage 失败时用材质、发光、VFX 或后处理掩盖问题；
- 把 Blender、Substance、Maya、任意脚本插件或远程 image-to-3D 变成用户运行依赖。

## 10. 行业依据

- Riot Games, [How the VALORANT Arsenal Was Built](https://playvalorant.com/en-us/news/dev/how-the-valorant-arsenal-was-built/)：gameplay-first、grounded/identifiable、tactical、cohesive，以及第一/第三人称唯一识别标记。
- Riot Games, [The Craft and Fantasy of VALORANT Weapon Skins](https://playvalorant.com/en-us/news/dev/the-craft-and-fantasy-of-valorant-weapon-skins/)：概念批准后建模；第一/第三人称和地面识别；动画/VFX/音频服从 gameplay readability；反复 Design Playtest。
- Marmoset, [Baking A Hard Surface Weapon in Toolbag](https://marmoset.co/posts/baking-a-hard-surface-weapon-in-toolbag/)：High/Low、hard edge/UV、Bake Group、Cage offset、Paint Offset/Skew、AO/ID/Thickness/Curvature 和分层材质。
- Adobe, [Bake Mesh Maps](https://helpx.adobe.com/substance-3d-painter/using/baking.html)：High-to-Low transfer 与 Normal/AO/Curvature/Position/Thickness/ID mesh maps。
- Epic Games, [FBX Static Mesh Pipeline](https://dev.epicgames.com/documentation/en-us/unreal-engine/fbx-static-mesh-pipeline-in-unreal-engine?lang=en-US)：LOD、material、texture 和真实引擎导入验证。
- Khronos, [glTF 2.0](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html)：交付格式、PBR 通道与色彩空间基础；格式合法性不等于视觉质量。

## 11. 完成定义

只有同一 export hash 同时满足：

`reviewed reference + approved form + High + Low + Hero UV + Cage/Bake + PBR + FPS presentation + LOD/collision/socket + commercial engine + independent human review + restart readback`

才能写 `HERO_ASSET_APPROVED`。当前状态明确保持：

```text
structural = PARTIAL_SOURCE_PASS
visual = QUALITY_TARGET_NOT_MET
human = NOT_RUN
engine = NOT_RUN
distribution = NOT_RUN
HQ_360 = BLOCKED_REFERENCE_COVERAGE
```
