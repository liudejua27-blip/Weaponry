# ForgeCAD 商业 FPS Hero Weapon 生产研究与实施蓝图

版本：2026-08-26
状态：accepted engineering direction；资产质量仍为 `QUALITY_TARGET_NOT_MET`

## 0B. AuthoringMesh V2-03 真实纵切与门设计纠偏

当前 source 为 **528 schemas / 28 operators / 115 read + 87 opt-in write = 202 tools**。真实 D1 `rear-stock` 已完成 stable-ID `MoveVertices` child、单 source-node lowering、Worker strict GLB readback、六视图，以及 proposal FormArt durable replay/restart readback。这是第一个真实资产上的“网格编辑→候选→六视图持久艺术证据”纵切，但仅为 8V/6Q 枪托，不是商业网格。

评估中暴露的本质问题是：旧 gate 对八个指标要求绝对 Pareto non-regression，会拒绝任何存在艺术权衡的候选；但把 `1/512` 直接叫做 IoU/F1 的“一像素容差”同样不严谨。最终方案不改旧 CrossView，只新增 `form-art-secondary-pareto-review@1` 诊断：所有负 delta 仍显式记为 regression；以 direction-normalized PPM 量化、限定 core tradeoff budget，要求 semantic metrics 不回退、aggregate 改善且至少一个 core metric 实质改善。它只产生 `REVIEWABLE_TRADEOFF`，不是质量 PASS。

当前 Z taper 的 aggregate 改善 `2038 ppm`，主要 winner 为 top silhouette/centroid；top Boundary F1 与 rear-3q silhouette/F1 仍保留 regression 标记。派生 candidate 已生成独立 proposal FormArt receipt，未复用 source pixels；Part-ID 全 observed，但 owner/open-void、negative-space、line-flow 未全通过，系统正确返回 `BLOCKED_PROPOSAL_FORM_ART_EVIDENCE`。下一步仍在 04AG 内修复这些真实艺术证据失败项，再谈人审和 secondary Stage。

### 0C. 04AG proposal-side durable evidence 边界

`ProductionWeaponFormArtProposalEvidence@1` 是 04AG 的目标 durable sidecar，不是对源 `ProductionWeaponFormArtEvidence@1` 的改名，也不是一次 prepare response 的内联 PASS。Runtime 必须把 proposal candidate 的六视图证据以 hash-only CAS receipt 写入 durable Store，并在 get/restart 时重验 source/proposal candidate、artifact/readback、ReferenceCanvas、DesignSpec、CameraLock/CameraRig、view spec、RenderSet 与 AOV lineage。每一视图还必须保留 Part-ID、negative-space、line-flow 状态；left/right/rear-three-quarter 必须单独满足 `rear-stock` owner/void 约束。

本条现已完成合同注册与真实 D1 CAS/Store/same-key replay/Runtime restart readback；receipt/canonical/identity hash 稳定。实际六视图结果为 Part-ID 全 observed、negative-space/line-flow 未全部 resolved、left/right/rear-three-quarter 的 `rear-stock` owner/open-void 严格门 false，因此为 `PASS_DURABLE_TRANSPORT / BLOCKED_PROPOSAL_FORM_ART_EVIDENCE`。`REVIEWABLE_TRADEOFF` 仍只是允许人审的诊断标签，不得升级为 PASS、secondary-form-approved、Stage、confirm、version 或 export。

## 0A. 04AF 真实执行反馈

研究结论已用真实 D1 验证了一次：单 `rear-stock` 型面重建能够在 ForgeCAD 内部编译、回读、六视图渲染和评分，但它在 left/right/rear-three-quarter 回退，因此被正确拒绝。这证明商业差距的核心不是“缺更多参数”，而是缺少可像美术家一样直接组织边、面、高光和形状节奏的编辑真值。

`AuthoringMesh@2` 因此从研究 seam 进入真实 Runtime 持久化基线：稳定 V/E/H/F/Corner IDs、immutable parent revision、journal/tombstone、一次共享边 split 和 restart exact readback 已通过。它仍未绑定真实武器，也没有证明 merge DAG、deform/shading/UV/bake；后续研究和实施必须优先让这个 kernel 成为武器美术创作的主真值，再向 High/Low/UV/Bake/Material/FPS/Engine 纵向贯通。

CameraLock 也新增了只读预检：父系和诊断性 180° 存在，但 `user_approved_orientation=false`，因此 durable lineage 不创建。这条边界保证系统不会为了跑通管线伪造人的美术意图。

## 0. 2026-08-26 实施检查点

本蓝图已有第一批 compile-first 内核落地，但尚未形成商业资产：

- AuthoringMesh V2：稳定 vertex/edge/half-edge/corner/face/loop/ring ID、不可变 parent revision DAG、journal/tombstone、original/evaluated 分离与局部 `split_edge`；尚缺其余编辑操作、持久化/MCP 和 evaluated retarget；
- Native High：独立 High Worker、CPU regular-quad Catmull-Clark subset、确定性 replay；Manifold 固定 revision 可通过 opt-in feature 编译链接，OpenSubdiv 仍未 vendored/链接；
- High↔Low↔Cage/Bake：严格绑定 Part/source node/material/solid/primitive/topology/correspondence，ray miss、nearest fallback、cross-Part、backface、penetration、skew、UV overlap 任一非零均不能 PASS；正式 Embree bake producer 尚未接通；
- MaterialLayerGraph：14 种闭合节点与确定性 DAG plan 已进入 525-schema manifest；当前只返回 `VALIDATED_PLAN_NOT_EVALUATED`，没有纹理求值或 PBR 资产；
- 联合编译、Manifold feature 链接、525-schema checker 已通过。证据为 `docs/evidence/mcp010f/commercial-weapon-native-authoring-high-bake-material-source-compile-gate-20260826.json`。

这些改动解决的是“ForgeCAD 是否拥有正确的商业生产内核边界”，不是“当前武器是否已达到《无畏契约》质量”。真实 D1 仍停在 `camera-calibrated`，必须先完成用户授权朝向、一个真实 source-node Form repair 和 fresh 六视图 FormArt，再把以上 seam 逐段接成 Runtime-owned durable chain。

## 1. 核心结论

ForgeCAD 要对标《无畏契约》同类商业 FPS 武器，必须从“能生成几何和 GLB 的 Runtime”升级为“内建 DCC 等价生产能力的 Hero Asset System”。目标不是复制 Riot 资产，而是达到相同等级的生产纪律、第一人称可读性、跨专业协作和可交付证据。

```text
Art Direction
→ Primary / Secondary Form
→ Runtime-owned AuthoringMesh
→ Non-destructive High
→ Editable Low + High↔Low correspondence
→ Hero UV
→ Per-Part Cage + zero-fallback Bake
→ closed MaterialLayerGraph
→ FPS rig / animation / VFX / audio / safe zones
→ LOD / collision / sockets / compression
→ Unreal-first / Unity round-trip
→ independent Hero Art Review
→ confirm / immutable version / export / restart readback
```

Riot 的公开文章把武器身份、第一/第三人称辨识、动画节拍、VFX 安全区、音频线索和 Design Playtest 视为一个共同产品闭环。商业质量不是“更高三角形数”或“一张漂亮转台图”，而是上述环节绑定同一 candidate/export hash 的乘法结果。

## 2. 当前 ForgeCAD 为什么仍差得很大

当前 source/contracts 数量已经很大，但真实 D1 仍停在：

- Stage=`camera-calibrated`；
- `secondary-form-approved=NOT_CREATED`；
- `QUALITY_TARGET_NOT_MET`；
- Low=`DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`；
- `FPS-HIGH-05=NOT_PASSED`；
- Unreal/Unity、FPS animation/VFX/audio、独立 Hero Art Review=`NOT_RUN`；
- HQ360=`BLOCKED_REFERENCE_COVERAGE`。

历史 2K 诊断中的 provisional Low/Cage 只有 1,000 triangles，且 Low 来自 triangle edge collapse；Bake coverage 约 36.25%，存在 45,386 miss、107,063 nearest fallback、3,982 cross-Part hit、padding=0。它只能证明技术链可运行，不能证明商业资产生产闭环。

根因不是缺少更多 Schema，而是缺少同一个真实资产的纵向闭环：

1. 形体还没有通过真人可接受的 primary/secondary Form Gate；
2. AuthoringMesh 的商业编辑能力、corner attributes、general correspondence 与完整 revision history 未闭合；
3. High、Low、UV、Cage、Bake 仍有自动 draft 或 source slice 冒充正式美术产物的风险；
4. Material 仍是固定公式 preview，不是可编辑 layer/mask/generator/decal/wear graph；
5. Viewer 还没有把第一人称、动画、VFX、音频和玩法可读性变成同一个 Art Director 工作台；
6. GLB/Three.js 可打开没有经过真实 Unreal/Unity clean-project import、reimport、restart 和 packaged build 回读；
7. 自动指标不能替代独立资深武器艺术家与 Design Playtest。

## 3. ForgeCAD-only 目标架构

```text
Codex Desktop / CLI
  └─ authenticated MCP typed intents
       └─ ForgeCAD Runtime                 唯一状态写者
            ├─ ArtDirection Compiler       Brief / variants / decisions
            ├─ Rust Authoring Kernel       half-edge / stable IDs / revisions
            ├─ Native High Worker          DetailGraph / Manifold / Subdiv
            ├─ Native Low Worker           quad draft + editable retopo
            ├─ Correspondence Worker       Low corner/face → High binding
            ├─ Hero UV Worker              chart draft + artist policy
            ├─ Cage / Bake Worker          owner policy + Embree + Mikk
            ├─ Surface Worker              MaterialLayerGraph + texture build
            ├─ FPS Presentation Worker     rig / clips / events / safe zones
            ├─ Delivery Worker             LOD / collision / socket / GLB/KTX2
            └─ Engine Harness Adapter       Unreal-first / Unity receipts

ForgeCAD Art Director Viewer               只读
  └─ form / topology / UV / bake / material / FPS / engine / human evidence
```

Worker 可以链接固定版本开源库，但只接受 closed typed request，不联网、不读 SQLite、不接受脚本/URL/任意路径/环境变量，也不能成为第二真值。Runtime 保存 canonical input/output、CAS roots、producer cohort、资源预算、逐项 diagnostics、approval 和 rollback lineage。

## 4. 自有 AuthoringMesh 是第一核心

`AuthoringMesh@2` 必须由 ForgeCAD 自有 Rust kernel 实现。第三方 half-edge handle 或 GLB index 只能是某个 revision 内的 ephemeral index，不能成为 durable identity。

必须具备：

- vertex / edge / half-edge / corner / face 稳定 ID；
- quad / ngon / triangle、loop / ring / boundary；
- hard edge / crease / seam / MaterialZone / Part / Feature / source map；
- split / collapse / dissolve / bridge / inset / extrude；
- selection/constraint set、mutation journal、tombstone、revision DAG；
- original/evaluated namespace 分离；
- evaluated retarget 和 High↔Low correspondence。

硬不变量包括 twin 对称、next/prev 互逆、face cycle 闭合、边界/内部边 incident 数量正确、无非流形/零面积/非有限/悬挂引用/ID 重用。corner 必须独立承载 UV、normal、tangent 与 seam 数据。

## 5. High → Low → UV → Cage/Bake

### 5.1 High

High 是 `AuthoringMesh revision + closed DetailGraph` 的非破坏 evaluated artifact。Manifold 只负责有界 same-Part Boolean；OpenSubdiv 只负责固定 CPU limit-surface evaluation。Bevel、support loop、crease、weighted normal、floater、panel/recess/vent 语义和 stable lineage 仍由 ForgeCAD 自己负责。

### 5.2 Low 与 correspondence

QuadriFlow 或任何自动 remesher 只能产生 `DRAFT_UNREVIEWED`。正式 Low 必须是可编辑 `LowAuthoringMesh`，锁定 Part boundary、hard edge、UV seam、aperture、open-stock、radial loop、support flow、socket 和 first-person visible faces，并经过 edge-flow/pole/maintainability 人审。

每个 Low corner/face 必须绑定 High face、barycentric/parametric coordinate、normal offset、Part/Feature/MaterialZone、confidence/tolerance，以及 miss/ambiguous/cross-Part/owner-bleed 状态。nearest-surface fallback 只能用于诊断，不能 PASS。

### 5.3 Hero UV

xatlas 只能提供 chart/pack draft。ForgeCAD 必须提供 per-corner UV、artist seam、first-person/world/hidden texel weighting、island direction、UV0/UV1、stretch/overlap/OOB、2K/4K atlas、8–32 px mip padding、xref 和 MikkTSpace replay。自动重顶点和随机 packing 必须被固定输入排序/seed/algorithm receipt 约束。

### 5.4 Cage/Bake

Cage 与 Low 的 topology、face order、corner order必须完全一致，并支持 per-vertex/per-region front/back offset、owner mask、penetration/self-hit/skew 诊断。Embree 只提供 ray kernel；Part isolation、owner policy、dilation 和 map semantics 由 ForgeCAD 负责。

正式 Bake 至少包含 tangent normal、world normal、AO、curvature、thickness、position、object/material/Part ID，并保存每 texel hit/miss/self/cross-Part/skew/fallback/owner-bleed 分类。任何 fallback、未授权 cross-Part 污染或未解释 miss 都不能 promotion。

## 6. MaterialLayerGraph 与表面叙事

商业材质不能由固定噪声公式或一套 smart material 直接生成。`MaterialLayerGraph@1` 必须是 closed DAG，仅允许：

`Source / Constant / Anchor / Generator / Mask / Filter / Transform / Blend / NormalCombine / RoughnessRemap / Decal / Trim / ChannelPack / Output`

每个 generator 记录 seed、domain、budget、Part/MaterialZone ownership 和 provenance。层次顺序应为：基础材质 → edge/roughness response → decal/marking → macro variation → microdetail → 有因果的接触/摩擦/热/维护磨损。随机 grunge 只能是 draft；真人必须审核形状语言、value/roughness hierarchy、色彩脚本和磨损叙事。

MaterialX 只作为 closed semantic interchange/lowering subset；OpenImageIO 只处理白名单 codec、mip、channel pack 和 dilation；OpenColorIO 只消费固定版本 color config。normal/MR/AO/ID 属于 data，不得误走颜色变换。

## 7. 第一人称武器工作台

`FpsPresentationPackage@1` 必须包含：

- hip、ADS、inspect、equip、reload、recoil/fire、third-person、ground pickup 固定相机；
- rigid skeletal hierarchy、root/pivot、named sockets；
- animation clips、rest-pose hash、event markers、socket follow samples；
- muzzle/eject/mag/optic/grip/hand/FX sockets；
- screen occupancy、reticle/muzzle safe region、hands/weapon clipping；
- VFX cue、Audio cue 与 gameplay beat timeline；
- first-person/world model parity 和动画节拍。

Riot 的公开 Design Playtest 原则说明：即使动画、VFX 或音频单独看很酷，只要影响武器辨识、准星、关键信息或操作感，就必须退回修改。ForgeCAD 因此需要 DPT receipt，而不是只显示 turntable beauty。

## 8. 发布与引擎验证

发布对象是 `canonical GLB + explicit sidecars + engine profiles`，不是只有一个 GLB：

```text
visual.glb
texture-set/*.ktx2 + fallback
HeroLodSet@1
CollisionSet@1
SocketSet@1
AnimationClipSet@1
GameplayBeat / VfxCue / AudioCue sidecars
engine-profiles/unreal-*.json
engine-profiles/unity-*.json
```

glTF Transform 只能执行固定 allowlist 的 prune/dedup/meshopt/tangent/KTX2 操作，并保留 canonical GLB 与前后 semantic diff。meshoptimizer 只能处理已批准 Low/LOD，不能冒充 retopo。KTX2/Basis 必须区分 color 与 data profile、保存 source/encoded/decoded hash 和平台 transcode receipt。

Unreal-first harness 必须在 clean project、固定引擎/插件/设置下自动 import、save、reimport、restart、packaged run，回读 skeleton/root/pivot/socket、material/texture color space、tangent、LOD/Nanite、collision、clip/event，并采集真实目标硬件性能。Unity-second 使用固定 Generic rig、ModelImporter/glTFast/KtxUnity/meshopt profile，分别保留 Editor 与 Player receipt。

没有通用的商业 FPS 三角形或毫秒预算。`FirstPersonPerformanceBudget@1` 必须绑定目标硬件、RHI、分辨率、帧率、hip/ADS/inspect/reload/fire 场景，并记录 draw calls、sections、triangles/vertices、resident/streaming texture bytes、skin/animation cost 和 frame/GPU p50/p95/p99。

## 9. 开源采用结论

| 项目 | 用途 | 采用边界 |
| --- | --- | --- |
| Manifold | bounded Boolean | 固定 C ABI/FFI Worker；不能成为 High authoring 或跨 Part 静默合并器 |
| OpenSubdiv | CPU subdivision evaluator | Tomorrow OSL 1.0，需法务/体积/确定性；不生成 DetailGraph |
| QuadriFlow | quad draft | README/许可证标签需重新核验；`BUILD_FREE_LICENSE=ON`；永不自动 promotion |
| xatlas | UV draft | MIT；固定 commit、输入排序、xref、brute-force/seed；不能替代 artist UV |
| Embree | ray intersection | Apache-2.0；不生成 Cage、不判断 owner、不完成 dilation |
| MikkTSpace | final tangent | 固定最终 UV/triangulation/handedness；不覆盖错误的 topology/UV |
| MaterialX | material semantic subset | Apache-2.0；禁用 custom shader、URL/path 和外部 implementation |
| OpenImageIO | image/mip/channel processing | Apache-2.0；codec/plugin/path/env 白名单和恶意图像预算 |
| OpenColorIO | fixed color policy | BSD-3-Clause；固定 config/processor hash；data maps 不做颜色变换 |
| meshoptimizer | LOD/cache/compression | MIT；只处理 approved Low/LOD，保护 Part/UV/socket/silhouette |
| glTF Transform | delivery optimization | MIT；固定 operation allowlist，无任意 JS，保留 canonical GLB |
| KTX2/Basis | GPU texture delivery | 固定 encoder/profile/thread；color 与 normal/data 使用不同质量 profile |
| Khronos Validator | format conformance | 只能证明 glTF 合规，不能证明视觉、引擎或商业质量 |

所有项目必须固定 revision、LICENSE/NOTICE/SBOM/provenance、恶意输入、资源、确定性、包体和可移除策略后才能进入 bundled Worker。

## 10. 生成式 3D / Hugging Face 的边界

TRELLIS.2、Hunyuan3D、SPAR3D 等可以作为 concept/blockout proposal 研究对象，但不能成为 P0 authoring truth。它们的输出通常是高密度 remesh/decimation GLB 或生成式 PBR，不能自动提供稳定 Part identity、artist quad flow、Hero UV、High↔Low correspondence、零 fallback Bake、FPS rig、引擎 parity 或独立人审。

若未来采用，只允许隔离离线 `ConceptProposalWorker`：输出 `DRAFT_UNREVIEWED`，保留模型/权重许可证、GPU/内存预算、seed、输入参考授权和 provenance；输出必须重新进入 Form → AuthoringMesh → High/Low/UV/Bake 全链。SAM2 一类分割模型可作为参考 mask proposal，同样不能直接写 reviewed contour 或 Part ownership。

## 11. Typed contracts

除现有合同外，商业闭环需要：

- `AuthoringMesh@2`、`TopologyMutationJournal@1`、`SelectionConstraintSet@1`；
- `HighArtifact@1`、`LowAuthoringMesh@1`、`HighLowCorrespondence@1`；
- `HeroUVLayout@1`、`CageArtifact@1`、`BakeSet@1`、`MaterialLayerGraph@1`；
- `AssetFramePolicy@1`、`SkeletalHierarchy@1`、`SocketSet@1`、`AnimationClipSet@1`；
- `GeometryTangentPolicy@1`、`TexturePackingPolicy@1`、`HeroLodSet@1`；
- `EngineImportProfile@1`、`ShaderParityReceipt@1`、`EngineValidationPlan@1`、`EngineValidationReceipt@1`；
- `FirstPersonPerformanceBudget@1`、`HeroArtReviewReceipt@1`。

这些是目标合同；未进入 manifest/Runtime/Store/MCP 并取得真实 receipt 前不得宣称可用。

## 12. 实施计划

### P0-A：先让真实 D1 进入 secondary form

1. 持久化 semantic ordering + user-authored rear-3/4 orientation + `RegisteredCameraRigCalibration@2` CameraLock child lineage；其 Contracts/Store/Runtime/MCP source 已于 2026-08-26 编译通过，待用户 orientation-specific approval 后执行 real D1 durable/restart Gate；
2. 只执行一个 `rear-stock-profile-reconstruction-v1` proposal；
3. fresh six-view RenderSet/FormArt；严格 owner-void、landmark、silhouette non-regression；
4. Codex review + 用户批准后才创建 `secondary-form-approved`。

### P0-B：AuthoringMesh vertical slice

1. canonical half-edge/corner arrays；
2. stable IDs/tombstones/revision DAG；
3. loop/ring/boundary + bridge/inset/extrude；
4. selection/constraint/mutation journal；
5. original/evaluated namespace和 correspondence retarget；
6. Viewer topology/editor read model。

### P0-C：同一 Part 的 High→Low→UV→Bake 纵向切片

选择真实 D1 的 `rear-stock`：

1. DetailGraph + bounded Manifold + CPU subdivision High；
2. Low quad draft → ForgeCAD editor review；
3. exact High↔Low correspondence；
4. Hero UV draft → seam/density/padding review；
5. topology-correspondent Cage；
6. Embree/Mikk 8-map Bake，zero fallback/cross-Part；
7. 同一 Part 在 first-person fixed camera 下 visual review。

这个纵向切片通过后再扩展其余 Parts，避免再次出现“很多模块都有一点、没有一个资产真的完成”。

### P0-D：Surface / FPS / Delivery

1. closed MaterialLayerGraph 与 texture compiler；
2. WeaponPresentationRig、clips/events/sockets、安全区；
3. HeroLodSet、collision、meshopt、KTX2、fixed glTF Transform；
4. Unreal clean-project round-trip；
5. Unity second profile；
6. independent Hero Art Review/DPT；
7. confirm/version/export/restart exact hash。

## 13. 质量门

自动硬门：topology、stable identity、silhouette/negative-space/landmark、UV、tangent、Bake miss/fallback/cross-Part、PBR color/data roles、LOD/socket/collision、引擎 import/readback/performance、CAS/export/restart hash。

必须真人：原创与设计语言、主次平面/高光节奏、负空间、Low 可维护性、材质与磨损叙事、第一人称可读性、动画重量感、VFX/音频对竞技信息的影响、最终 Hero Art approval。

只有 `Form + Authoring + High + Low + UV + Cage/Bake + Material + FPS + Engine + Human` 全部绑定同一 hash 并通过，才允许 `HERO_ASSET_APPROVED`。

## 14. 主要资料

- [Riot：How the VALORANT Arsenal Was Built](https://playvalorant.com/en-us/news/dev/how-the-valorant-arsenal-was-built/)
- [Riot：The Craft and Fantasy of VALORANT Weapon Skins](https://playvalorant.com/en-us/news/dev/the-craft-and-fantasy-of-valorant-weapon-skins/)
- [Marmoset：Baking a Hard Surface Weapon](https://marmoset.co/posts/baking-a-hard-surface-weapon-in-toolbag/)
- [Adobe Substance：Bake Mesh Maps](https://helpx.adobe.com/substance-3d-painter/using/baking.html)
- [Manifold](https://github.com/elalish/manifold)
- [OpenSubdiv](https://github.com/PixarAnimationStudios/OpenSubdiv)
- [QuadriFlow](https://github.com/hjwdzh/QuadriFlow)
- [xatlas](https://github.com/jpcy/xatlas)
- [Embree](https://github.com/RenderKit/embree)
- [MaterialX](https://github.com/AcademySoftwareFoundation/MaterialX)
- [OpenImageIO](https://github.com/AcademySoftwareFoundation/OpenImageIO)
- [OpenColorIO](https://github.com/AcademySoftwareFoundation/OpenColorIO)
- [meshoptimizer](https://github.com/zeux/meshoptimizer)
- [glTF Transform](https://github.com/donmccurdy/glTF-Transform)
- [KTX2 Specification](https://registry.khronos.org/KTX/specs/2.0/ktxspec.v2.html)
- [glTF 2.0 Specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html)
- [Unreal Skeletal Mesh Pipeline](https://dev.epicgames.com/documentation/en-us/unreal-engine/fbx-skeletal-mesh-pipeline-in-unreal-engine)
- [Unreal Sockets](https://dev.epicgames.com/documentation/en-us/unreal-engine/skeletal-mesh-sockets-in-unreal-engine)
- [Unity Model Importer](https://docs.unity3d.com/6000.0/Documentation/Manual/FBXImporter-Model.html)
- [Unity glTFast](https://github.com/Unity-Technologies/com.unity.cloud.gltfast)
- [TRELLIS.2](https://huggingface.co/microsoft/TRELLIS.2-4B)
- [Hunyuan3D 2.1](https://huggingface.co/spaces/tencent/Hunyuan3D-2.1)
