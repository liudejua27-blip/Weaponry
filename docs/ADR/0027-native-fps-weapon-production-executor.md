# ADR-0027：ForgeCAD 原生 FPS 武器美术生产执行器

> **Status: partially superseded by ADR-0029 (2026-08-29).** 本 ADR 的 Runtime 唯一写者、typed Worker、证据和批准边界继续有效；产品范围、Action Space 和一个月交付优先级以 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md` 与 ADR-0029 为准。

> 2026-08-26 `FPS-AUTHORING-MESH-V2-03` 决议：真实 D1 已用 ForgeCAD-owned stable-ID `MoveVertices` 打通 authoring revision → one-node GeometryProgram → strict GLB → six-view replay；当前面为 **527 schemas / 115 read + 87 write = 202 tools**。同时冻结两条边界：旧 CrossView strict-Pareto 不改语义；新 Pareto review 必须将负 delta 明确记为 regression，只能形成可审阅权衡，不是 non-regression。任何变更后的 candidate 必须重新生成 owner/void/Part-ID FormArt；没有该派生证据时 fail closed，不得推进 secondary Stage。

> 2026-08-26 `FPS-AUTHORING-MESH-V2-02` 决议落地：真实 D1 `rear-stock` 已进入 ForgeCAD-owned `AuthoringMeshRevision@2`，source binding 随 revision CAS 持久化并通过 Runtime restart；稳定 mesh/lineage identity 与易变 candidate/artifact hashes 解耦。`production_weapon_authoring_mesh_v2_source_prepare` 是第 86 个 opt-in write tool，当前总面 **114 read + 86 write = 200**。Native High V2 bridge 和 `FaceExtrude` 为 source-only；下一架构切片必须实现 `MoveVertices` 与 revision→GeometryProgram 单节点替换，不能再次退回参数灰模。Stage/visual/commercial 真值不变。

> 2026-08-26 `04AF` 实施决议：继续 ForgeCAD-only 原生执行器路线，但从“对参数灰模做微调”切换到“用 `AuthoringMesh@2` 进行可编辑形状节奏与拓扑创作”。依据是真实 D1 rear-stock 型面 proposal 虽局部执行成功，却在三视图回退并被正确拒绝。当前 source 为 **527 schemas / 114 read + 85 write = 199 tools**；本 ADR 的商业终态仍需 High→Low→UV→Bake→MaterialLayerGraph→FPS→Engine→Human 同 lineage 闭环。

> 2026-08-26 04AE 现行 source：**525 schemas / 28 operators / 112 read + 84 write = 196 tools**；CameraLock child 与 AuthoringMesh V2/Native High/strict bake/material plan seam 已编译，real D1 因 authored rear3q approval 未提供而不得落成功 row。下文较小数量均为历史 cohort。

> 2026-08-26 研究补充：ADR 的生产执行器必须以自有 Rust AuthoringMesh 和 explicit High↔Low correspondence 为中枢；第三方库只能作为固定 typed Worker 内核。商业退出门扩展为 FPS presentation、clean Unreal/Unity round-trip、target-hardware budget 和 independent Hero Art Review。完整论证见 `../FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

2026-08-26 public-source 补充：Formal High 的 closed wrapper、MCP `get/prepare`、Runtime adapter/IPC 和 Store scoped idempotency 已 source/focused PASS；完整 positive/restart/cleanup 尚未通过。当前 515 schemas 与 194 tools 必须分开解释。该增量不改变“先批准 Form，再生产独立 High/Low/Cage/Bake”的架构，也不允许把既有 Native High GLB identity 描述成新生成的一份商业 High GLB。

2026-08-26 实施补充：Form Stage policy、Formal High factory、Store scoped idempotency、Runtime materializer 与 MCP public seam 已落地。完整正向 restart/cleanup fixture仍未取得；ADR 应读为“source surface 已存在，capability acceptance 尚未通过”。真实 D1 零写，Stage/visual/commercial 真值不变。

2026-08-26 Cage/Bake public seam 增量：Store 已提供七子表原子 commit、精确重放、CAS reachability/GC 与重启 get；固定 Worker 已收口 exact-topology Cage、8-map geometric Bake 和 8-texel dilation。Runtime High resolver 已按 `Stage source candidate + distinct derived High candidate` 完成 compile/focused PASS；Formal High 的 Runtime-owned factory、原子 Store seam 与 internal materializer 已存在。当前未通过的是完整 source-lineage/CAS 正向 restart fixture 与独立 public prepare/get。真实 D1 因 Form 前置未通过且没有 formal positive receipt，全新 `production_weapon_high_low_bake_prepare` 仍以 `FORMAL_HIGH_STAGE_SOURCE_LINEAGE_UNAVAILABLE` fail closed，整体返回 `PRODUCTION_WEAPON_HIGH_LOW_BAKE_PRODUCER_UNAVAILABLE` 且零写。formal High capability/Cage-Bake positive receipt 均 `NOT_RUN/NOT_PROVEN`，不提升 Stage、视觉、人评、引擎、分发或商业结论。证据：`docs/evidence/mcp010f/commercial-weapon-form-stage-policy-formal-high-source-gate-20260826.json` 与 `docs/evidence/mcp010f/commercial-weapon-cage-bake-public-seam-source-gate-20260826.json`。


> 2026-08-26 source synchronization: current public surface is **515 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 MCP tools**. Low quad durable now reaches Runtime/Store/MCP with candidate-bound current provenance and remains `DRAFT_UNREVIEWED`, structural-only and unpromotable; prepare replay → Runtime drop/reopen → get is **1/1 PASS**, with six Low durable CAS roots linked/GC. Hero UV now reaches Store/Runtime/MCP public `hero_uv_durable_get/prepare`, with four Hero UV CAS roots linked/GC; artist UV review and packaged same-cohort remain `NOT_RUN`, while visual/human/engine/commercial remain `NOT_RUN/NOT_PROVEN`. This does not advance `Stage=camera-calibrated`, `secondary-form-approved=NOT_CREATED`, `FPS-HIGH-05=NOT_PASSED`, `QUALITY_TARGET_NOT_MET`, `HQ360=BLOCKED_REFERENCE_COVERAGE`, proposal `registered=false`, confirm/version/export, or the single `in_progress=FGC-MCP010F` gate. Evidence: `docs/evidence/mcp010f/commercial-weapon-hero-uv-durable-restart-source-gate-20260826.json`.

> 2026-08-25 实现附记：当前 source truth 为 **499 schemas / 28 operator entries / 107 read + 79 opt-in write = 186 tools**。Native High 的公共 MCP/source durable chain 与当前 cohort Runtime restart **1/1 PASS**；Low quad draft、Hero UV 和 Viewer Art Director matrix 仅为 source slices。packaged/candidate-bound quality、visual、human、engine、distribution 仍未通过，因此本 ADR 接受的目标架构不能被解读为 Commercial High/Low/UV 已完成。

> 2026-08-23 实施补充：`FPS-ART-DECISION-04B` 已把 CameraLock/FormEvidence/FormArt 与 GeometryProgram lineage 投影为 Runtime-owned、只读、可重启验证的 5-group/10-gate assembly 决策。typed descriptor resolver 现在让 receiver-envelope、muzzle-axis 在可解析时进入 `READY_FOR_SEARCH`，但 stock-open-frame、trigger-void、rail-spine 仍 `BLOCKED_PARAMETER_SINK`，所以整体 parameter-sink gate 仍阻断。negative-space source policy 已要求 left/right/rear-three-quarter 的 exact trigger-void + open-stock-void observed rows；真实 04A 仍 bbox / `mask_operation=none`，故 `BLOCKED_NEGATIVE_SPACE`，line-flow 和 first-person profile 也仍阻断。它不会自动改几何，也不会用单 Part 指标冒充艺术决策；Stage 仍停留 `camera-calibrated`，D1 sink fixture/restart 未运行。

日期：2026-08-23

状态：Accepted as target architecture；当前实现仍为 `QUALITY_TARGET_NOT_MET`

替代范围：补充 ADR-0025/0026；收紧所有可能暗示 Blender 运行时、Blender Worker 或商业质量已完成的表述

## 1. 决策

ForgeCAD 的第一垂直目标是合法、虚构、不可制造的 FPS 游戏武器 Hero Asset。Codex 是外部美术总监与编排大脑，ForgeCAD Runtime 是唯一状态写者，ForgeCAD Worker 是唯一高信息量美术生产执行器。

产品不安装、不启动、不调用、不捆绑 Blender，也不存在 Blender Worker、Blender fallback 或 `.blend` 真值。禁止 `bpy`、Blender Python、BlenderMCP、任意脚本和通过临时导出绕过 Runtime。Blender 仅作为公开概念和官方源码的 reference-only 研究对象；任何可采用思想必须转写为 ForgeCAD 自有 closed Schema、Rust 实现、typed Worker 协议、canonical hash、资源预算和独立证据。复制源码、链接 Blender/Cycles 或采用 GPL 文件必须另行完成逐文件许可证决策；本 ADR 不授权复制。

“对标无畏契约、生死狙击等游戏”只定义表现级别、第一人称可读性和生产完整度，不授权复制它们的枪械造型、皮肤、标识、贴图或独特视觉资产。输出必须是原创或基于用户有权使用的参考。

## 2. 第一性原理判断

### 2.1 Goal 纠偏：产品目标是 Hero Asset 闭环，不是协议表面

本 ADR 的北极星是“Codex 经 ForgeCAD 交付原创商业 FPS Hero Weapon”。工具数量、合同数量、source compile、GLB 可解析、单次 Three.js/Godot 查看都不是替代目标。每个实施阶段必须尽快作用于同一真实候选；长期只增加横向 source slices 属于能力建设，不属于资产进度。

ForgeCAD 必须分别交付并绑定：

- `HeroSourceAsset@1`：Art Direction、AuthoringMesh、High、editable Low、Hero UV、Cage/Bake、MaterialLayerGraph、LOD/collision/socket；
- `FpsPresentationPackage@1`：hip/ADS/inspect/equip/reload/recoil、第一/第三人称、GameplayBeat、VFXCue、AudioCue 与屏幕可读性；
- `EngineDeliveryPackage@1`：canonical GLB、独立 LOD/collision/socket/animation sidecars、KTX2/meshopt delivery、Khronos validation 与真实目标引擎回执。

静态 `HeroSourceAsset` 通过可以作为商业静态资产评审对象，但没有动画/VFX/audio/实机节拍时不得称为完整 premium skin experience。Riot 官方公开流程明确把概念/模型、动画、VFX、音频和 gameplay readability 当作同一体验的独立职责；ForgeCAD 必须保留这些轴，不能用总分互相抵消。

### 2.2 GLB 是交付合同，不是 AuthoringMesh 或完整引擎合同

glTF/GLB 继续作为 ForgeCAD 的 canonical visual delivery format，但它不保存 ForgeCAD 的完整创作历史，也不能单独承载不同引擎的 LOD、collision、socket、gameplay event 和性能语义。最终包固定为：

```text
EngineDeliveryPackage@1
├─ visual.glb
├─ texture-set/               # PNG/KTX2，slot 与 color-space 明确
├─ HeroLodSet@1
├─ CollisionSet@1
├─ SocketSet@1
├─ AnimationClipSet@1
├─ GameplayBeatSet@1
├─ VfxCueSet@1
├─ AudioCueSet@1
└─ engine-profile-manifest
```

Runtime 保存所有对象及 lineage；GLB/sidecar/engine-imported artifacts 都是派生 CAS 工件。Khronos Validator 只验证 glTF 格式，不能替代 ForgeCAD strict semantic readback、Unreal/Unity round-trip 或人类艺术评审。

### 2.3 商业引擎验证分两层

1. **本机 export preflight**：ForgeCAD 内置、离线、固定版本的 strict readback、Khronos Validator、mesh/texture budget、轴向/单位/命名/切线/材质/animation contract 检查；用户仍只安装 ForgeCAD。
2. **studio engine certification**：由固定版本、签名、无任意脚本的 Unreal-first/Unity validation runner 在 clean project 和 packaged build 中读取 exact export hash，回传 `EngineValidationReceipt@1`。目标引擎不可用时必须是 `ENGINE_VALIDATOR_UNAVAILABLE/NOT_RUN`，不能由 Three.js 或 preflight 补位。

引擎 runner 不写 ForgeCAD SQLite/CAS，不成为第二真值；Runtime 验证 runner cohort、engine version、import settings、export hash、readback、fixed-shot renders、performance trace 和 restart/reimport 后，才持久化 receipt。

游戏武器的质量不是“是否生成一个 GLB”，而是以下信息是否沿同一 candidate/hash/lineage 被生产、验证和批准：

1. 多视图主形、比例、负空间和第一人称持枪构图；
2. 可读的中频结构、硬表面平面变化、倒角高光和语义 Part；
3. 独立 High、Low、Cage 及其可追溯的 High-to-Low 射线映射；
4. 面向 Hero Asset 的 UV、切线、材质区、贴图与 mip/seam 安全；
5. PBR 表面在固定九 AOV、第一人称相机、轮廓图和材质分解图中可解释；
6. socket、刚体机构动画、VFX、LOD、碰撞和目标引擎回读；
7. 艺术家独立批准，而不是 Codex 自评或结构测试代替人审。

因此，更多 primitive、更多 Schema 或一个可打开的 GLB 都不是商业 Hero Asset 的充分条件。Production Stage 必须代表可审计的资产成熟度，而不是功能模块是否曾运行。

## 3. 当前事实基线

当前 source truth 为 **515 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 MCP tools**。AuthoringMesh split/collapse/dissolve 3/3、High durable source、candidate-bound Low provenance 的 prepare replay/drop-reopen/get **1/1 PASS** 与 Hero UV public `get/prepare` 已通过窄门；Low 仍 `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`，Hero UV/Low 仅 structural/source，artist UV review、packaged same-cohort、visual/human/engine/commercial 仍 `NOT_RUN/NOT_PROVEN`。

当前 AB 灰模审计快照为 23 semantic Parts / 728 triangles，strict GLB readback 与结构 hard gate 通过；几何仍以 primitive 为主，MaterialZone 主要是通用白壳、黑机械、金色点缀和琥珀发光。没有同一候选上的商业 PBR、Hero UV、High/Low/Cage bake、LOD、动画、VFX 或引擎交付证据。

当前可见视图指标同样不支持“接近商业武器”：左侧 IoU/F1 为 0.646598686967/0.342466558400，右侧 0.614892877823/0.253260468575，前视 0.473051976024/0.166625505392；顶视相机重定基线由 0.307644222183/0.046501935753 改善到 0.516328331862/0.124038954382，但这是取景改善，不是几何通过。targets 仍自动且未审阅，landmarks/regions 不可靠，human/engine 为 `NOT_RUN`，结论保持 `QUALITY_TARGET_NOT_MET`、`visual NOT_PROVEN`、不 confirm/export、HQ360 `BLOCKED_REFERENCE_COVERAGE`。

现有 `surface_bake.rs` 明示：

- normal policy 是 `evaluated-candidate-surface-tangent-field-not-high-low-cage@1`；
- AO policy 是固定 8-ray candidate self-occlusion；
- 它能证明确定性 surface-layer lowering，但没有独立 high/low/cage 输入和 cross-mesh ray hit lineage。

故 `CandidateSurfaceBake@1` 必须在产品语言中称为 Surface Layer Bake，不得作为 High-to-Low Bake、法线细节转移或商业烘焙完成证据。

## 4. 原生 Worker 模块边界

ForgeCAD Worker 逐步拆为固定、无网络、无任意脚本的 typed executors；它们可以是同一可执行文件内的模块，也可以按资源风险隔离为 sibling worker，但都不能成为第二状态写者。

| 执行模块 | 输入真值 | 输出与责任 | 不负责 |
| --- | --- | --- | --- |
| Shape Worker | ReferenceCanvas、DesignSpec、GeometryProgram | profile/multi-loop loft、panel、vent、recess、joint、bevel、stable Part/source lineage | 视觉批准、数据库写入 |
| High Worker | approved primary/secondary form、detail graph | 非破坏高模、support/crease、浮动细节、high artifact | 自动把高模当低模 |
| Low Worker | approved high、game budgets、semantic Part policy | authoring low mesh、retopo lineage、LOD source | 只靠三角形抽稀冒充 retopo |
| UV Worker | approved low、MaterialZone、visibility weights | seams、islands、texel density、packing、Mikk input | 自动宣称艺术布局通过 |
| Cage Worker | exact high/low pair | cage mesh、offset field、intersection/skew diagnostics | 隐式 extrusion 无证据 |
| Bake Worker | exact high/low/cage/UV hashes | tangent normal、AO、curvature、thickness、position、object/material/part ID、miss/skew map、padding receipt | 当前 self-surface bake 的改名升级 |
| Surface Worker | bake outputs、material graph、AssetPack | PBR texture set、decal/wear/microdetail、channel packing | 用 emissive 掩盖几何错误 |
| Animation/VFX Worker | approved rigid hierarchy、anchors、clips | inspect/equip/reload/recoil、emissive/particle/trail/bloom evidence | 功能武器仿真 |
| Render Worker | exact artifact/camera/profile | fixed beauty + 8 data AOV、first-person/readability turntable | 修改资产真值 |

Runtime 负责 prepare、审批、candidate、CAS、Job、版本、回退与 durable stage head；Worker 只计算并返回结果与 receipt；Viewer 只显示 read model 和临时选择/相机状态。

### 4.1 商业中间产物的职责与解锁门

以下名称是 ForgeCAD 产品内的 typed contract/Worker 边界，不是对外部 DCC 的运行时依赖。每个门都必须绑定同一 candidate、artifact、reference/camera、cohort 和 hash；source/transport/restart 通过只更新该模块的结构状态。

| 能力 | ForgeCAD-only 职责 | 必须通过的门 | 当前权威边界 |
| --- | --- | --- | --- |
| AuthoringMesh | original/evaluated 拓扑、语义 Part/source map、稳定 identity lineage、可编辑 split/collapse/dissolve | correspondence、edge-flow、stable retarget、strict readback 与可重放 edit history | 三种操作 durable/restart **3/3 PASS** 仅为结构切片；general correspondence/evaluated retarget/editor 仍 `NOT_PROVEN` |
| Native High | detail graph、support/crease/floating detail、非破坏 High artifact 与 Part lineage | `HighMeshArtifact` 独立回读、预算、确定性、approved form binding | source/durable chain 有窄门通过，但 `FPS-HIGH-05=NOT_PASSED`、proposal=`registered=false`，不能视为商业 High |
| Native Low / Retopo | artist-editable quad topology、loop/ring/crease、High↔Low correspondence | 独立 Low artifact、quad/edge-flow、feature protection、可编辑回读 | current Low 为 candidate-bound `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`；不能用 triangle collapse/LOD lowering 代替 |
| Hero UV | visibility-weighted seam/island、density/stretch/overlap/OOB/padding、Mikk tangent 输入 | exact Low/Part/MaterialZone binding、2K/4K policy、目标引擎 tangent round-trip | 7 registered contracts 与 public durable get/prepare 的 restart **1/1 PASS**；只为 structural/source，artist unwrap/visual/engine 未通过 |
| Cage / Bake | 独立 cage/offset field、High→Low per-ray map、miss/skew/cross-hit/padding diagnostics | distinct High/Low/Cage hashes、逐 Part isolation、双回放和 target tangent | fixed Worker、七记录原子 Store seam、public get/prepare 与 exact replay 已 source PASS；全新 prepare 因七类正式 producer 未闭合返回 `PRODUCTION_WEAPON_HIGH_LOW_BAKE_PRODUCER_UNAVAILABLE` 并零写，当前 D1 仍 `NOT_HIGH_LOW_BAKE` |
| Material Layer Graph | layer/mask/generator/wear/microdetail、channel/color-space/provenance 与 Hero surface | UV+Bake exact binding、roughness/readability、多视图/第一人称 AOV | 当前 fixed-formula/embedded preview 只证明消费链，商业 material gate 未通过 |
| LOD | authored LOD0/1/2、silhouette/UV/tangent/material continuity、collision/socket | identity、误差预算、engine import 与 FPS readability | 只有 transient/structural slices；LOD/collision/socket 尚未形成商业 receipt |
| Art Director Viewer | 只读阶段矩阵、AOV/compare、Part/MaterialZone、first-person review evidence | exact candidate/artifact/reference binding；人审前不得批准或写状态 | source/read-model surface 可用；正式 Art Director/人审仍 `NOT_RUN`，Viewer 无写权限 |
| EngineValidation | 目标引擎 importer、material/tangent、LOD/socket/animation/collision round-trip | same export hash、真实目标引擎 receipt、失败可回读 | `NOT_RUN`；解析器 smoke、Three.js 或 source Gate 不能替代 |
| HeroArtReview | 独立艺术家盲审、原创性/IP、跨视图与第一人称质量签核 | same export hash、reviewer identity、问题闭环与独立批准 | `NOT_RUN`；Codex typed review 不能替代人审 |

这些门的上游真值缺失时，ForgeCAD 只能称可验证高级灰模/技术管线，不能称商业级资产生产软件。Runtime 仍是唯一写者；所有 Worker 无网络、无路径、无 SQLite/CAS 写权限、无任意脚本和无 DCC 运行依赖。

## 5. 精细 Production Stage 状态机

现有 `draft → gray-model → topology → material-surface → animation-vfx → game-delivery` 过粗。它允许“topology passed”同时掩盖主形、高模、低模、UV、Cage 和 Bake 未完成。目标状态机按如下顺序推进；每次推进都必须绑定 immutable input/output candidate、artifact、quality receipt、approval 和 cohort：

1. `reference-intake`
2. `reference-coverage-reviewed`
3. `camera-calibrated`
4. `blockout-reviewed`
5. `primary-form-approved`
6. `secondary-form-approved`
7. `high-poly-approved`
8. `low-poly-approved`
9. `uv-approved`
10. `cage-approved`
11. `bake-approved`
12. `material-approved`
13. `rig-socket-approved`
14. `animation-approved`
15. `vfx-approved`
16. `lod-collision-approved`
17. `hero-art-review-approved`
18. `engine-validated`
19. `export-confirmed`

任何阶段都允许生成 review candidate，但只有对应质量门和独立 approval 可推进 head。失败结果必须留痕并保持上一 head；禁止跨阶段补票，禁止 Material/VFX 覆盖轮廓或负空间失败。

现有 V1/V2 粗粒度 stage 保留为兼容投影，不能作为新 Hero Asset 的完成定义。落地时新增 `ProductionStage@3`，由 Runtime 显式映射回旧阶段；不就地扩大 V1/V2 enum，避免历史 receipt 语义漂移。

## 6. 各阶段 Visual Gate

不同阶段必须使用不同视觉门，不能用一个总分吞掉失败原因。

| Gate | 必须观察 | 阻断条件 |
| --- | --- | --- |
| Reference Coverage | HQ360 核心 front/back/left/right/rear-3/4、扩展审阅 top、遮挡与推断标签 | 未审 target、关键视图缺失、授权不明；top 不得替代任一 HQ360 核心视图 |
| Camera | 同一参考/候选/相机/裁切 | 裁切、透视不一致、相机改变被算作几何改善 |
| Primary Form | silhouette IoU、Boundary F1、bbox、centroid、landmark/region | 任一关键视图退化；不允许 scalar average 抵消 |
| Secondary Form | Part-ID、normal、wireframe、负空间、平面节奏 | 穿插、封死孔洞、无语义分件、过密噪声 |
| High/Low | 法线连续、倒角高光、拓扑、预算、对应关系 | 高低模身份不独立、source lineage 缺失 |
| UV | overlap、out-of-bounds、density、stretch、padding、seam/hard-edge congruence | 零面积、重叠、mip padding 不足、首视角密度不足 |
| Cage/Bake | hit/miss、skew、front/back hit、distance、seam、dilation | miss/穿透/反投射、切线约定不一致、污染跨 Part |
| Material | beauty/normal/AO/material-ID/UV-stretch、roughness hierarchy | 材质只靠颜色区分、发光溢出、微细节无尺度层级 |
| First-person | hip/ADS/inspect/equip/reload/recoil 固定相机 | 手部遮挡区、枪口/机匣读形、动画穿插失败 |
| Hero Art | 盲审表、艺术家签名、原创性/IP 检查 | 仅 Codex 自评、仅结构 Gate、明显竞品复制 |
| Engine | glTF importer、材质/切线、LOD、socket、animation、collision | source-only、解析器 smoke 或单引擎结构证据冒充商业通过 |

所有 Gate 同时保留 `structural_status`、`visual_status`、`human_status`、`engine_status` 和 `distribution_status`，不得压缩成单一 `passed`。

### 6.1 多视图与相机 profile 不得混用

视图数量不是质量语义。Runtime 和合同必须显式区分：

- `hq360-core@1`：`front/back/left/right/rear-three-quarter` 五个用户授权、已审参考视图；它是 HQ360 reference coverage 的核心集合；
- `review-extended@1`：上述五个核心视图再加 `top`，用于 FPS 武器主形和相机锁；它不能用 top 替代缺失的核心视图；
- `camera-rig-orthographic@1`：`front/back/left/right/top/bottom` 六个正交相机；它是坐标、裁切和取景校准，不是六个真实参考，也不等于 HQ360；
- FPS camera lock 可以把 `review-extended@1` 与正交 rig、`rear-three-quarter` perspective camera 绑定为七相机集合，但必须保留每个参考 view 的 target/mask/ViewSpec/camera hash 和 observed/inferred/unknown 来源。

`ReferenceViewSet@2`、`ReferenceCanvas@1`、`CameraCalibration@2` 和 `CameraRigCalibration@1` 只能在 exact project/candidate/artifact/reference/renderer cohort 下组合。顶层一个 `camera_hash`、fixture camera、自动 mask 或数组长度都不能代替逐视图绑定和显式 approval。

## 7. Hero Asset UV 合同

`HeroUvLayout@1` 至少需要：

- exact low candidate/artifact/topology/Part/MaterialZone binding；
- first-person、world-view、hidden 三档 visibility weight；
- target texel density 与 per-island deviation；
- hard-edge/seam congruence、mirroring/stacking policy、orientation locks；
- overlap、out-of-bounds、zero-area、inverted、stretch、island count；
- 按目标 mip 级计算的 padding，而不是固定像素常数冒充所有分辨率安全；
- UV0 游戏材质用途与未来 UV1/lightmap 用途分离；
- MikkTSpace 输入和目标引擎 tangent round-trip。

P0 默认以单套 2K/4K Hero UV profile 为目标，是否使用 UDIM 必须由目标引擎/平台合同决定；不能因为 Blender 支持 UDIM 就把 UDIM 作为 FPS 游戏资产默认答案。当前 512 atlas 或由 512 输入确定性放大的 2K pack 只保留为开发结构证据。

## 8. 真正的 High-to-Low Cage Bake 合同

新增 `HighLowBakePlan@1`、`CageArtifact@1`、`HighLowBakeReceipt@1`，至少绑定：

- distinct `high_candidate_sha256`、`low_candidate_sha256`、`cage_artifact_sha256`；
- exact low UV/tangent/MaterialZone/Part ID；
- cage 与 low 的 topology correspondence；
- per-ray origin/direction/max distance/front-back policy；
- per Part/material bake isolation 与 anti-cross-hit policy；
- normal OpenGL +Y、AO、curvature、thickness、position、object/material/part ID 输出；
- miss、backface、skew、overlap、cage intersection、distance histogram 与 heatmap；
- padding/dilation、seam continuity、MikkTSpace/目标引擎重建误差；
- deterministic replay、resource budget、Worker cohort、output byte hashes。

只要 high/low/cage 不是三个可独立回读的绑定对象，或没有 ray diagnostic，就必须返回 `NOT_HIGH_LOW_BAKE`。现有 `CandidateSurfaceBake@1` 不迁移、不重命名 schema，只在 UI/文档中显示为 Surface Layer Bake，避免破坏历史 receipt。

## 9. 最短实施顺序

### Round 1：多视图主形和相机

冻结 AB/open-stock/camera/reference；先把 HQ360 核心 front/back/left/right/rear-3/4 与扩展 top target 变为 reviewed，再将它们与固定七相机 rig 写入独立、可重启验证的 camera-lock prerequisite。camera lock 只证明同一参考/候选/产物的目标、裁切和相机被锁定；在独立 stage transition 消费它之前不得直接推进 `camera-calibrated`。之后一次只改一个 assembly variable，优先 receiver-main/upper/lower clearance、muzzle/core、trigger void。每次固定执行 `geometry_program_hash → geometry_prepare → artifact_readback_get → one compare/render → boundary_error_get/silhouette_part_error_get`。无裁切且多视图 Boundary F1 达标前锁住 PBR。

### Round 2：中频硬表面

轮廓通过后，使用 live `operator_catalog_get` 返回的 active typed operators，按语义 Part 添加 profile/multi-loop loft、panel、vent、recessed channel、energy core、joint stack、bevel。每次回读 Part/source/material map、穿插与负空间；Boolean 只有 live active 且有当前 cohort 证据时才能使用。

### Round 3：High/Low/UV/Cage/Bake 基础

先交付独立 High/Low/Cage 合同和诊断 Bake Worker，再做 LOD。`game_asset_lod_derive` 的 transient tessellation lowering 不能充当 retopo；必须显式 materialize 并证明 Part、MaterialZone、AABB、silhouette 和 tangent 稳定。

### Round 4：实质 PBR

只有主形、拓扑、UV 和 High-to-Low Bake 通过后，才进入 white clearcoat、dark metal、black anodized、brushed gold、cyan emissive、grip rubber、worn edge。固定九 AOV + first-person views 评审；首版仍标 `QUALITY_TARGET_NOT_MET`，直到人审通过。

### Round 5：交付收口

完成 3 LOD、collision、socket anchors、inspect/equip/reload/recoil、emissive/particle/trail/bloom、export/restart/rollback/GC 和目标引擎导入。source Gate、local smoke、Godot 单次结构回读均不能替代商业引擎与真人 Hero review。

## 10. 实施优先级

P0 顺序固定为：

1. `ProductionStage@3` 与各阶段质量 receipt；
2. reviewed multi-view target 和 camera lock；
3. High/Low/Cage 三资产身份与 lineage；
4. Hero UV 合同与诊断；
5. High-to-Low Bake Worker；
6. material/readability gate；
7. rigid weapon animation、VFX、LOD/collision；
8. human art review 与 engine validation。

### 10.1 可执行升级队列

新方案命名映射（不新增重复任务）：`FPS-REF-02` 对应 reviewed reference/visual-structure/coverage prerequisite，由现有 `FPS-CAM-02` prerequisite 与 `FPS-CAM-03` transition 消费；`FPS-FORM-03` 对应现有 `FPS-FORM-EVIDENCE-04A` typed evidence 与 `FPS-FORM-04` 的 FormArt@1/FormQuality@2 阶段链。后续沿用这些既有任务 ID 和 receipt 边界。

| 原子切片 | 产物 | 退出条件 | 当前状态 |
| --- | --- | --- | --- |
| `FPS-REF-01` | `ProductionStage@3` 第一条 reference coverage transition | durable Canvas/DesignSpec、五核心 coverage、approval、restart get | source implemented；仅 `PASS_SOURCE_STRUCTURAL` |
| `FPS-CAM-02` | 独立 `ProductionCameraLock@1` prerequisite | 扩展六参考视图 + 七相机 rig exact binding、两个 owned CAS、幂等/回滚/重启；不推进 stage | 真实 durable prepare/replay/get/restart `PASS` |
| `FPS-CAM-03` | `reference-coverage-reviewed → camera-calibrated` | 只消费通过的 current camera lock；parent/head/stale 检查；视觉仍可未达标 | 真实 camera-calibrated transition/head `PASS`；visual 未通过 |
| `FPS-FORM-04` | blockout/primary/secondary 多视图质量 receipt | 每个关键视图独立 no-regression；Part-ID、负空间、line flow、固定相机 | 真实 reviewed FormArt 与 structural-only legacy FormQuality durable `PASS`；FormArt/overall `NOT_PROVEN`，FormQuality@2 仅剩 CrossView hard gate 与 FormArt target observation zero-write blocked，无 form Stage head |
| `FPS-FORM-EVIDENCE-04A` | Part-ID、negative-space、line-flow 三类 typed evidence producer | 六视图 exact binding、真实 same-cohort prepare/replay/restart、可使 FORM Gate 从强制 blocked 转为按证据判定 | 真实六视图 reviewed-structure receipt `PASS`；bbox negative-space unknown、line match 未提升，不推进 stage |
| `FPS-HIGH-LOW-CAGE-05` | 独立 High/Low/Cage contracts + readback/diagnostic gates | High、Low、Cage 三个独立 hash-bound artifact、对应关系、ray diagnostics；不得复用 self-surface bake | bounded Low/Cage source bundle durable `PASS_SOURCE_STRUCTURAL`；formal High/Low/Cage producer、Stage binding、rollback fixture 与 Bake pending |
| `FPS-HIGH-05` | 独立 `HighMeshArtifact@1` | product-owned detail graph、artifact/readback/Part lineage、确定性 | Native High source/durable GLB/readback、Formal High pure factory、Store atomic seam、internal materializer 与 Runtime adapter/IPC 已形成窄幅 source slice；完整 source-lineage/CAS positive restart、public MCP、candidate quality/package/human Gate 未通过，`FPS-HIGH-05=NOT_PASSED` |
| `FPS-LOW-06` | 独立 `LowMeshArtifact@1` | explicit retopo lineage；不得复用 transient LOD lowering 冒充 Low | partial source；feature protection 8/8；UV-only seam、quad flow 和 artist retopo 未证明 |
| `FPS-UV-07` | `HeroUvLayout@1` | density/stretch/seam/hard-edge/overlap/OOB/mip padding/Mikk/engine tangent | pending |
| `FPS-CAGE-08` | `CageArtifact@1` | low topology/face order 对应、offset field、相交/反面/Part crossing 为有界诊断 | Worker exact topology/order source PASS；Runtime-owned formal producer、current-candidate positive artifact 与 Stage binding pending |
| `FPS-BAKE-09` | `HighLowBakePlan@1` + diagnostic + receipt | distinct High/Low/Cage、ray hit/miss/skew/cross-hit、8 类 map、双回放 | 8-map Worker + 8-texel dilation、七记录 Store/MCP public seam source PASS；new prepare fail-closed、current D1 positive receipt pending |
| `FPS-MAT-10` | Hero material/readability gate | Hero UV+Bake 精确绑定、真实 map provenance、九 AOV、first-person views | pending |
| `FPS-LOD-11` | authored `HeroLodSet@1` + collision | LOD0/1/2 独立 identity、silhouette/UV/tangent/material continuity | pending |
| `FPS-HUMAN-12` | `HeroArtReviewReceipt@1` | 独立艺术家盲审、原创性/IP、同一 export hash | pending / human required |
| `FPS-ENGINE-13` | `EngineValidationReceipt@1` | 真实目标引擎 importer/material/tangent/LOD/socket/animation/collision round-trip | pending |
| `FPS-EXPORT-14` | final export/restart receipt | 全阶段同一 hash、packaged same-cohort、rollback/GC/reopen | pending |

任一切片的 source test 通过只更新该切片的 structural 状态。它不得自动推进后续切片，也不得把 `QUALITY_TARGET_NOT_MET` 改写成视觉、真人、引擎或商业完成。

在 P0 的 1–5 完成前，不扩展 Shader Graph、通用角色系统、任意脚本插件、Cycles/EEVEE parity、UDIM 全功能或通用 CAD 类别。

### 10.2 当前成熟度与实际缺口

| 生产域 | 当前可复用能力 | 仍缺失的商业中间产物 | 未满足时的硬边界 |
| --- | --- | --- | --- |
| Reference / Camera | durable ReferenceCanvas、DesignSpec、候选绑定 compare、独立 Camera Lock | 真实同 cohort 的六审阅参考/七相机 durable positive receipt；逐视图可靠 landmarks/regions | 不得把 camera-fit 改善写成几何或视觉通过 |
| Shape / Form | 28 个 active typed operators、profile/multi-loop loft、稳定 Part/source lineage、固定 AOV | blockout/primary/secondary 各自多视图 quality receipt；第一人称读形与负空间审批 | 继续保持 `QUALITY_TARGET_NOT_MET`，锁住 PBR/VFX |
| High | panel/vent/bevel 等可作为 detail graph 原语 | 独立 `HighMeshArtifact@1`、非破坏 detail graph、high readback、support/crease/floating-detail lineage | evaluated GLB 或更多 triangles 不能冒充 High |
| Low / Retopo | triangulated ArtifactReadback、有限 authoring mesh preview、结构 topology gate | 独立 `LowMeshArtifact@1`、可编辑 authoring topology、High↔Low correspondence、edge-flow receipt | transient simplification/LOD lowering 不能冒充 Low 或专业布线 |
| Hero UV | bounded 512 atlas、UV/tangent integrity、MikkTSpace 结构证据 | `HeroUvLayout@1`、seam/hard-edge policy、visibility-weighted density、stretch/overlap/padding diagnostics、2K/4K 原生布局 | 512 输入插值到 2K 不能算商业 2K UV/纹理 |
| Cage / Bake | CandidateSurfaceBake self-surface tangent/AO；另有 formal fixed Worker/public persistence seam | 独立 cage mesh/offset field、High→Low ray map、miss/skew/cross-hit/heatmap、padding/seam diagnostics | public seam 不等于 producer/正向 receipt；当前 D1 必须保持 `NOT_HIGH_LOW_BAKE`，不得重命名旧 Surface Layer receipt 升级语义 |
| Material | AssetPack、embedded texture、PBR lowering、九 AOV、MaterialZone | Hero UV+Bake 精确绑定、真实 2K/4K map provenance、roughness/readability/first-person review | development material preview 不得推进 `material-approved` |
| LOD / Collision | GLB delivery、transient LOD 派生、Part lineage | authored LOD0/1/2 identity、误差预算、UV/tangent/material continuity、collision receipt | triangle budget 通过不等于 LOD 交付 |
| Animation / VFX | rigid clip、socket、projection、particle/trail/bloom 的 structural chain | approved hierarchy 后的 inspect/equip/reload/recoil 与第一人称穿插/readability gate | VFX 不得掩盖 form/material 失败 |
| Human / Engine / Package | approval/session、restart readback、结构性 Godot evidence | 独立 Hero Art Review、原创性/IP、真实目标引擎 round-trip、packaged same-cohort full chain | Codex 自评、source test、解析器 smoke 均不能称商业完成 |

### 10.3 外部项目选择性采用队列

外部项目只进入 evaluation/adoption cache；在固定版本、许可证、SBOM、provenance、恶意输入、资源上限、确定性 replay 和退出方案全部通过前，不成为 Runtime 依赖或 active Skill。

| 项目/思想 | 可解决的问题 | 不能解决的问题 | 采用判定 |
| --- | --- | --- | --- |
| Blender [Dependency Graph](https://developer.blender.org/docs/features/core/depsgraph/) / [Baking](https://docs.blender.org/manual/en/latest/render/cycles/baking.html) 思想 | original/evaluated 分离；selected-to-active、cage、ray distance、margin 的问题定义 | 不授权 Blender runtime、`bpy`、Cycles、`.blend` 真值或 GPL 源码直接并入 | clean-room specification reference only |
| [`Manifold`](https://github.com/elalish/manifold) | robust manifold Boolean 与 operand lineage | 不提供任意 mesh authoring、艺术设计或第二状态写者 | Apache-2.0；固定 revision 的 bounded same-Part `boolean@1` 已 accepted，仅限隔离 Worker |
| [`mikktspace@0.3.0`](https://github.com/gltf-rs/mikktspace) | tangent basis 与 handedness 重放 | 不生成 Hero UV、Bake、PBR 或引擎通过 | MIT/Apache-2.0；受限 tangent Worker 已 accepted |
| [`OpenSubdiv`](https://github.com/PixarAnimationStudios/OpenSubdiv) | subdivision/crease/face-varying 边界候选 | 不解决 High 设计、support flow 或商业高光审美 | research-authorized；TSL-1.0 需法务，未采用 |
| [`QuadriFlow`](https://github.com/hjwdzh/QuadriFlow) | 自动 quad draft 候选 | 不提供硬表面 Part/孔洞/support-flow/artist promotion | snapshot-blocked；README MIT 标签与实际 BSD-style+enhancement grant 文本不一致，且 solver/Eigen 路径须审计；仅允许 `BUILD_FREE_LICENSE=ON` 的 draft evaluator |
| [`xatlas`](https://github.com/jpcy/xatlas) | UV chart parameterization/packing 的候选基线；可用于与自有 Hero UV 诊断对照 | 不提供艺术 seam、首视角密度、mip 安全或真人 UV 审批 | MIT；固定 commit 后隔离 benchmark，再决定 vendoring/sidecar |
| [`Embree`](https://github.com/RenderKit/embree) | High→Low per-Part ray kernel、front/back hit 与诊断候选 | 不生成 Cage policy、correspondence 或 formal receipt | Apache-2.0；approved-for-evaluation/snapshot-blocked，未采用 |
| [`MaterialX`](https://github.com/AcademySoftwareFoundation/MaterialX) | typed material graph/interchange 语义 | 不允许 shader/plugin/runtime 成为第二真值 | Apache-2.0；research-authorized typed subset，未采用 |
| [`OpenImageIO`](https://github.com/AcademySoftwareFoundation/OpenImageIO) | image/map I/O、mipmap、channel processing 候选 | 不提供材质艺术决策，codec 不能无界开放 | Apache-2.0；approved-for-evaluation/snapshot-blocked，未采用 |
| [`OpenColorIO`](https://github.com/AcademySoftwareFoundation/OpenColorIO) | 固定色彩配置与 display transform 候选 | 不允许动态 config/plugin/path 输入 | BSD-3-Clause；research-authorized，未采用 |
| [`meshoptimizer`](https://github.com/zeux/meshoptimizer) | attribute-aware LOD、边界锁定、几何误差与缓存优化的候选基线 | 不生成 authoring Low、High↔Low 对应、Hero UV 或 Art Gate | MIT；仅评估 stable API，实验 flags 不进入 P0 真值 |
| [`glTF Transform`](https://github.com/donmccurdy/glTF-Transform) | prune/dedup/meshopt/KTX2 的可重放导出优化 | 不提供 AuthoringMesh、艺术建模、引擎或人审 | MIT；仅固定 operation allowlist，必须保留 canonical source GLB 和前后 semantic diff |
| Basis Universal / KTX2 | color/data/normal 纹理的 GPU 交付与 mip chain | 不替代逻辑材质图、master raster 或颜色空间真值 | benchmark-first；固定 encoder/profile/thread，保存 source/decoded/compressed 三类 hash并逐组件审许可证 |
| Khronos [`glTF-Validator`](https://github.com/KhronosGroup/glTF-Validator) | glTF 2.0 语法、引用、buffer/accessor、image/extension 的独立验证报告 | 不证明外观、拓扑审美、引擎一致性或商业质量 | Apache-2.0；优先作为隔离验证器评估，不替代自有 strict readback |

选择性采用的退出条件是“增强一个已定义的生产 Gate”，而不是“增加一个库”。任何候选库若不能输出 hash-bound、candidate-bound、可重启验证的 typed receipt，就只保留为研究证据。精确 revision、许可证、SBOM 与当前 adoption 状态只认 `EXTERNAL_PROJECT_ADOPTION.md` 和 `THIRD_PARTY_LICENSES.md`；本表不得单独升级依赖。

### 10.4 执行节奏与资源纪律

1. 同一时刻只允许一个 Runtime writer；Contracts、Store、Runtime/MCP 和 evidence 可并行，但不能并行生成同一 candidate。
2. 默认先跑 closed parser、canonical hash、FK、幂等、冲突和 restart get；真实 Worker/多视图渲染只在 source Gate 收口后运行一次有界 fixture。
3. 每个可见质量轮只改一个 assembly variable，固定 reference/target/camera/cohort；失败候选留痕但不推进 head。
4. 每条 Stage 边独立记录 `PASS_SOURCE_STRUCTURAL`、`PASS_STAGE_VISUAL`、human、engine、distribution；禁止由后一轴补偿前一轴。
5. 每个真实长跑必须记录 PID、CPU/RSS、超时、输出目录和清理结果；进程或日志消失时标为 evidence ambiguous，不推断 PASS。

### 10.5 2026-08-25 商业质量研究收口

完整研究结论、商业生产链、产品缺口和落地顺序由
[`COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`](../COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md)
统一维护。本 ADR 保留架构决定，该计划负责执行级质量基线；两者冲突时不得选择更宽松的验收口径。

当前同 cohort 事实仍是：High 为 23 Parts / 2,280 triangles；所谓 Low/Cage 为 1,000 triangles，Low 来自确定性 triangle edge collapse，并非艺术家可编辑 quad retopo；bake coverage 仅 `0.3625035285949707`，ray miss `45,386`、nearest fallback `107,063`、cross-part `3,982`、padding `0`。四个材质区和六张嵌入式 2K 固定公式贴图只证明开发材质层，Three.js 回读只证明消费链可打开；商业引擎、独立人审、完整 FPS 动画/VFX/LOD/collision/socket 均未运行。

因此下一阶段不得继续以材质、灯光或 VFX 修饰替代形体与资产生产链。固定顺序为：`FormQuality → secondary-form-approved → AuthoringMesh → High → Low/Retopo → Hero UV → Cage/Bake → MaterialLayerGraph → FPS/LOD/collision/socket → commercial engine → human review`。任何阶段未通过，都保持 `QUALITY_TARGET_NOT_MET`。

## 11. 验收用语

- `PASS_SOURCE_STRUCTURAL`：合同和固定 Worker 结构证据通过；
- `PASS_STAGE_VISUAL`：当前阶段、当前视图和阈值通过；
- `PASS_HUMAN_ART_REVIEW`：独立人类艺术评审通过；
- `PASS_ENGINE_VALIDATION`：目标引擎回读通过；
- `HERO_ASSET_APPROVED`：同一 export hash 同时满足全部必需 stage、visual、human、engine 和 restart evidence。

除最后一种外，均不得对外描述为“达到无畏契约/商业 Hero Asset 水平”。

## 12. 依据与边界

Blender 官方 Baking 文档说明了游戏贴图 bake 的 UV 前提、Selected-to-Active 的 low→high 射线、Cage/Ray Distance 和 UV island margin；ForgeCAD 采用这些问题定义，但不采用 Blender 运行时。Blender Depsgraph 的 original/evaluated copy-on-write 分离启发 ForgeCAD 保持 authoring truth、evaluated artifact 和 stage head 分离；具体实现仍由 ForgeCAD 自有协议完成。Blender 整体 GPL 许可证意味着“研究思想”和“复制代码”不是同一授权路径。

本 ADR 不改变安全范围：只生产虚构、非功能性游戏美术资产，不输出现实武器制造尺寸、加工图、材料配方、性能或操作建议。

## 13. 2026-08-23 实施状态

`ProductionStage@3` 已以 additive 方式落地 19 阶段枚举、五轴状态、Transition/Head/Prepare/Get 合同及 Runtime/Store/MCP 入口；V1/V2 保持不变。当前开放前两条公开转换 `reference-intake → reference-coverage-reviewed → camera-calibrated`，并且都只允许 same-candidate/same-artifact 证据提升。第一条边要求 Runtime 读回 durable `ReferenceCanvas@1`/`DesignSpec@1`，确认 front/back/left/right/rear-three-quarter 已提供且 coverage=complete。独立 `ProductionCameraLock@1` 把扩展 top 参考与 bottom calibration-only helper 区分开，严格绑定六参考视图、七相机 Rig、同一 source head/candidate/artifact、approval session/expiry 和两个 owned CAS；它自身不推进 Stage。第二条边只能消费 current、passed、exact-bound lock，并把 8 项 lock/rig/source-parent binding 写入 immutable transition/head；stale head、retarget 或字段冲突全部 fail closed。

`ProductionWeaponFormQuality@1` 保留为早期 additive、non-promoting 的六视图结构质量 record；当前严格主链新增 `ProductionWeaponFormArtEvidence@1` 与 `ProductionWeaponFormQuality@2`。V2 精确绑定 current Camera Stage head、Camera Lock、ReferenceCanvas、DesignSpec、CrossViewEvidenceBundle、逐视图 RenderSet/Comparison/Quality 三联 CAS，以及 user-confirmed Part-ID、negative-space、line-flow art evidence；Store/Runtime 独立重验对象 kind、固定九 AOV、度量阈值、ReferenceCanvas 映射、canonical、幂等冲突和 reachability。三条 approval-gated Stage@3 form 边只能消费 exact-bound 的 V2 quality 与 art receipt，不能由 V1 record、直接 Store caller 或后期材质/VFX 伪造晋级。

该切片只能记为 `PASS_SOURCE_STRUCTURAL`：visual 仍固定 `QUALITY_TARGET_NOT_MET`，human/engine/distribution 仍为 `NOT_RUN`，不 confirm、不创建 version、不 export。后续 17 条转换必须分别实现对应的质量 receipt，禁止因为枚举已声明就绕过 Gate。

当前 source truth 为 **515 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 tools**。AuthoringMesh split/collapse/dissolve 独立 full-chain 3/3 PASS；High已有当前 cohort durable restart 1/1；Low 为 candidate-bound current provenance，`DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`，prepare replay/drop-reopen/get **1/1 PASS**；Hero UV public get/prepare 与四个 CAS roots linked/GC 已通过 source Gate。它们仍仅 structural/source；artist UV review、packaged same-cohort、visual/human/engine/commercial 仍 `NOT_RUN/NOT_PROVEN`，Stage=`camera-calibrated`、`secondary-form-approved=NOT_CREATED`、`FPS-HIGH-05=NOT_PASSED`、`QUALITY_TARGET_NOT_MET`、`HQ360=BLOCKED_REFERENCE_COVERAGE`、proposal=`registered=false`，不得 confirm/version/export。
