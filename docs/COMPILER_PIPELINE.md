# ForgeCAD 3D 编译器与质量管线

> 2026-08-26 `04AF`：管线已在真实 D1 上执行一次 `rear-stock` source materialize→GLB readback→六视图 RenderSet→CrossView quality decision，并在三视图回退时保留 baseline。`AuthoringMesh@2` 另有持久化 split-edge/restart 基线，但尚未成为该真实武器的 compiler source。下一个 compiler 目标是 Authoring revision→evaluated High 的 stable correspondence，而不是继续对参数灰模做无限搜索。

> 2026-08-26 现行 source：**525 schemas / 112 read + 84 write = 196 tools**。AuthoringMesh V2→Native High evaluator→strict correspondence/bake validation→MaterialLayerGraph plan 的编译 seam 已存在；尚未接成 Runtime-owned durable production chain。未经 real D1/user orientation Gate，不推进 Form/High。

> 商业编译链固定为 approved AuthoringMesh → High → editable Low/correspondence → Hero UV → Cage/Bake → Material → FPS/LOD → canonical GLB/engine derivatives。Manifold/QuadriFlow/xatlas/Embree/meshoptimizer 只提供受限内部算法，不改变 authoring truth。详见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

2026-08-26 Formal High 编译边界：public prepare 的输入只描述 source Stage head proof、distinct High candidate identity、idempotency 与请求 hash；candidate state、High binding、strict readback、receipt 和 CAS roots 必须由 Runtime/固定 Worker 派生。当前 adapter/IPC compile PASS，但没有合法 positive Stage fixture 和 restart receipt，因此编译面不得标记 Formal High 或 High→Low production complete。

2026-08-26 最新管线增量：Form Stage policy 已在 Store 深读 parent/head、CameraLock、FormQuality@2、FormArt、CAS/lineage；Formal High 已有 Runtime pure factory → Store atomic candidate+High record → restart-readable internal materializer seam。该链尚无完整 positive restart fixture 和独立 MCP public surface，只是 source/compile/focused 结果；真实 D1 没有进入 High/Low/Bake。证据：`docs/evidence/mcp010f/commercial-weapon-form-stage-policy-formal-high-source-gate-20260826.json`。

2026-08-26 Cage/Bake 编译边界：固定 Cage、8-map geometric Bake、8-texel dilation 与 2K Worker launcher 已 source PASS；它们只接受 Runtime 解析后的 typed 输入。High resolver、Formal High factory/Store/internal materializer 已按真实 `Stage source candidate + distinct derived High candidate/High GLB` 字段完成 compile/focused PASS；但完整 source-lineage/CAS positive materialize→drop/reopen 尚未运行，独立 public surface 也未暴露，所以真实 D1 仍以 `FORMAL_HIGH_STAGE_SOURCE_LINEAGE_UNAVAILABLE` fail closed。没有 formal High positive receipt、Low/Hero UV 精确 lineage、双 replay/cohort 和严格输出 readback时，不得启动正式 2K Bake、提交七记录或提升 Stage/质量。


> 2026-08-26 current source synchronization: **515 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 MCP tools**. The candidate-bound current Low exact provenance now feeds the public Hero UV durable compiler path; `hero_uv_durable_get/prepare` is complete through Store→Runtime→MCP, and the real prepare→replay→Runtime drop/reopen→get fixture is **1/1 PASS** with four Hero CAS roots linked/GC. This is structural/source evidence only, not artist-authored unwrap, visual, human, engine, commercial or packaged acceptance; Stage=`camera-calibrated`, visual=`QUALITY_TARGET_NOT_MET`, and no Stage/confirm/version/export transition is allowed. Evidence: `docs/evidence/mcp010f/commercial-weapon-hero-uv-durable-restart-source-gate-20260826.json`.

> 2026-08-25 目标编译链补充：商业武器不能由单个 evaluated triangle mesh 贯穿全部阶段；Compiler 必须保留 original AuthoringMesh、evaluated High、editable Low、topology-correspondent Cage、Hero UV/tangent、Bake maps、Material Layer output、LOD/collision/socket 和 export artifact 的独立 identity/hash/lineage。当前 edge-collapse Low 与 normal-offset Cage 只保留为 provisional diagnostics。详见 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`。

> 2026-08-26 当前增量：High Worker 已有有界面内 chamfer arc；Low 与 Hero UV 已分别接入 Runtime/CAS/Store durable public seam，但仍是 structural/unreviewed。Cage/Bake fixed Worker、exact Low topology/order Cage、8-map output、8-texel dilation 与七记录 atomic Store/MCP seam 已 source PASS；Runtime-owned formal producer 尚未闭合，new prepare 零写失败。任何 compile/source PASS 都不能写成当前 D1 production artifact 或商业 Gate PASS。

版本：2026-08-09
状态：MVP bounded compiler 已完成；FGC-MCP010A done；FGC-MCP010B V2 structural compiler 与固定同级 Worker 子门已通过（Darwin OS memory hard cap deferred）；FGC-MCP010C fixed renderer/reference compare source Gate PASS_WITH_UNRUN_VISUAL_GATES；FGC-MCP010D 的 profile/loft/revolve/sweep/transform/mirror/array/panel/vent/joint/part-output 已通过 source Gate；FGC-MCP010E 的离线 AssetPack、512px UV atlas、固定 mikktspace、embedded PBR textures 已通过 source Gate（xatlas/Validator/packaged/视觉子门 deferred）

## 1. 核心原则

Codex 负责“想什么”，ForgeCAD Compiler 负责“允许什么、如何确定性执行、结果是否合格”。自然语言、图片、Codex 视觉判断和第三方资产都不是产品真值；它们只能生成或约束 typed IR，并由 Runtime 记录 lineage。

```text
ReferenceEvidence
  → SubjectProfile
  → RepresentationPlan
  → AssemblyGraph + GeometryProgram + AppearanceProgram
  → deterministic lowering
  → geometry/UV/material/texture artifacts
  → strict readback
  → RenderSet + AOV
  → QualityReport
  → Candidate
  → approval
  → immutable DesignAssetVersion
```

## 2. 编译层

### 2.0 MVP profile

MVP 不实现通用图生 3D 模型。Codex 已能看参考图，由 Codex 提交 typed `SubjectProfile/AssemblyGraph/GeometryProgram/AppearanceProgram`；ForgeCAD 只做确定性验证和编译。当前实际 Runtime allowlist 包含 product-owned primitive@2 与 MCP010D 的 profile-extrude/profile-loft/revolve/tube-sweep/transform@2/mirror/array/boolean（同一 Part bounded union/difference/intersection）/panel/vent-array/joint-stack/part-output；Manifold 只作为隔离 Worker 实现，不开放通用 mesh Boolean。

MCP007 先产生真实多 Part mesh/GLB；MCP008 加 bounded UV/PBR 和 beauty/silhouette/normal/part-ID；MCP009 加 limited reference aspect evidence、稳定 Part `change_prepare`、immutable version/restore 和 CAS-backed `mvp-glb` receipt。像素级 silhouette/landmark/region compare、surface network、deformable、有机角色、UDIM、完整 AOV、Blender worker 和跨类别 benchmark 均是 post-MVP。

### 2.0.1 MCP010 V2 顺序

MCP010 必须依次完成：B 的封闭 `GeometryProgram@2`/真实 DAG/GLB readback → C 的 perspective/z-buffer renderer、九 AOV 和 reference metrics → D 的高细节 Operator → E 的离线 AssetPack、UV/tangent/PBR/texture。当前 B structural Gate 已通过但 Darwin OS 总内存硬门 deferred；C source Gate 已通过 `script/test_mcp010c.sh`，覆盖固定 camera/z-buffer、九 AOV、local mask/metrics、MCP image block、Codex/human review 和 deterministic raw stdio。首次真实机器人 PNG 也完成了 C 的 compare/review transport，但 primitive blockout 的 likeness threshold 为 `FAIL_QUALITY_TARGET_NOT_MET`；C 仍未完成 Viewer/package/live、人评和材质视觉门。D 的当前 16 个 Operator、Skill 0.2、bounded Manifold Boolean、strict readback/lineage、raw stdio Gate 和同 cohort packaged D raw structural probe 已通过；任意 mesh Boolean 与视觉门仍 NOT_RUN。E source Gate 已通过 `script/test_mcp010e.sh`，覆盖 AssetPack provenance、512px bounded UV atlas、固定 `mikktspace@0.3.0`、embedded PBR textures、strict readback 和九 AOV；同 cohort packaged E structural probe已通过；xatlas、Khronos Validator、Viewer/package/live C/F 与视觉 PBR 仍 NOT_RUN。其后 Hero UV durable source slice 已补齐 Store→Runtime→MCP public `hero_uv_durable_get/prepare`、current Low exact provenance、四 root linked/GC 与真实 replay/drop/reopen/get **1/1 PASS**，但不能先把它写成 artist unwrap、visual、human、engine、commercial 或 packaged producer；不推进 Stage/confirm/version/export。

目标九 pass 固定为 beauty、silhouette、depth、normal、AO、part-ID、material-ID、wireframe、UV-stretch，全部绑定同一 candidate/camera/material/renderer hash。快速 Viewer renderer 可以不同，但不能生成第二套模型、材质或质量真值。

### 2.1 Design Compiler

输入：参考证据、目标用途、尺寸/风格/预算、已安装 Skill 能力。
输出：`SubjectProfile`、`RepresentationPlan`、语义 `AssemblyGraph`、Part/MaterialZone 稳定 ID、未知项和 limitation。

类别开放不等于任何表示都可用。每个 Part 显式选择 procedural、deformable、surface、imported-readonly 或 hybrid；不存在可靠路线时停止并要求更多视图/人工建模，不能回退机械模板。

### 2.2 Geometry Compiler

输入：`GeometryProgram@1`、Part scope、预算。
能力（当前 MVP）：bounded box/cylinder/sphere primitives、稳定 Part/source-map、finite/index/triangle/byte budget、确定性 GLB lowering、strict readback。曲线、profile、loft、sweep、surface network、solidify、field/CSG、局部 deform 和 read-only mesh admission 必须等对应 Operator 被实现、审计并加入 capability manifest 后才可使用。
输出：规范化 geometry IR、primitive、stable source map、LOD/collision candidates 和 readback。

每个 Operator 都有固定 Schema、单位、坐标系、数值范围、复杂度、三角/内存/时间上限和 deterministic hash。禁止动态 import、反射调用、任意参数路径和未签名脚本。

### 2.3 Appearance Compiler

输入：`AppearanceProgram@1`、MaterialZone binding。
输出：经过验证的 UV、切线、PBR 通道、预览与交付纹理、材质表和 provenance。

P0 材质遵循 Principled 金属/粗糙度工作流：BaseColor、Metallic、Roughness、Normal、AO，按需 Emissive。MVP 使用 bounded procedural values 和 0–1 UV；纹理烘焙、UDIM、Opacity 和完整色彩管理尚未实现，不得把声明式 `TextureSet/BakeRecipe` metadata 当作已生成纹理。

当前 Hero UV 编译结果只可通过 candidate-bound current Low exact provenance 进入 `hero_uv_durable_prepare`；`hero_uv_durable_get` 仅回读 Store/CAS durable lineage。四个 Hero CAS roots 的 linked/GC 与真实 replay/drop/reopen/get **1/1 PASS** 是 structural/source evidence，不是 artist-authored unwrap、visual、human、engine、commercial 或 packaged acceptance，也不推进 Stage/confirm/version/export。

### 2.4 Render Evidence Compiler

编译器在 Viewer 关闭时仍能生成可重复的 headless evidence。所有候选用同一 scene/material truth，输出：

- beauty、alpha、depth、world/view normal；
- AO、part-ID、material-ID；
- wireframe、UV-stretch、silhouette；
- 与参考绑定的 front/side/back/top/three-quarter 和可选自定义相机；
- 相机、灯光、HDRI、色彩管理、分辨率、renderer version 和 hash。

快速交互 renderer 和高质量证据 renderer 可以是不同执行器，但必须由同一 RenderRecipe 和材质真值编译，不能形成两个版本或两套材质状态。

### 2.5 Quality Compiler

| 层 | 硬门/软门 | 主要检查 |
|---|---|---|
| Contract | 硬 | Schema、hash、lineage、单位、预算 |
| Geometry | 硬 | 退化/非有限值、法线、边界、自交、manifold 要求、triangle budget |
| Semantics | 硬 | Part/primitive/source-map、MaterialZone、稳定 ID |
| Silhouette | 软阈值/发布硬门 | mask IoU、轮廓距离、占框、关键比例、多视图一致性 |
| UV/Tangent | 硬 | overlap、padding、stretch、density、切线和 normal map 方向 |
| PBR/Texture | 硬+软 | 通道完整/范围/色彩空间、接缝、重复、清晰度、材质分区 |
| Detail | 软阈值 | 中观结构、局部细节覆盖、reference feature claims |
| Visual | 软+真人门 | 固定视图差异、Codex typed review、独立真人盲评 |
| Delivery | 硬 | GLB validator、LOD/collision、引擎 roundtrip、export hash |

Codex `VisualReviewReport` 必须引用具体 render/pass/region/claim，使用 bounded rubric 和置信度。它不能修改硬门结果，也不能独自证明质量。

## 2.6 商业生产编译边界（目标/排队）

商业编译器改为六段，不再把 `GeometryProgram → GLB` 当完整生产链：

```text
DesignCompiler
  → AuthoringCompiler
  → HighLowUvBakeCompiler
  → SurfaceCompiler
  → PresentationCompiler
  → DeliveryCompiler / EngineValidation
```

- `DesignCompiler` 输出 approved Brief/DesignLanguage/ReferenceViewSet/CameraLock；
- `AuthoringCompiler` 保存 `AuthoringMesh@2 + TopologyMutationJournal + ModifierDetailGraph`，并分别产生 original/evaluated hash；
- `HighLowUvBakeCompiler` 产生独立 High、editable Low、correspondence、Hero UV、CageField 和 8-map BakeSet；任何自动算法只输出 draft；
- `SurfaceCompiler` 把 typed MaterialLayerGraph、masks/generators/decals/wear 编译为纹理与材质包；MaterialX 只允许作为受限 interchange/lowering subset；
- `PresentationCompiler` 产生 hip/ADS/inspect/equip/reload/recoil fixed shots、animation/event/VFX/audio cues；
- `DeliveryCompiler` 保留 canonical GLB，同时生成 LOD/collision/socket/animation sidecars、KTX2/meshopt derived package，并运行固定 allowlist 的 glTF Transform/Khronos Validator；
- `EngineValidation` 必须读取 exact export hash，在 clean project/packaged build 中回读，不能由 delivery compiler 自我授予。

每段都必须先验证上游 approved identity，输出新的 CAS object/readback/quality receipt；失败时不部分写入、不推进 Stage。优化前后的 Part、MaterialZone、node hierarchy、animation、socket、bounds、UV/tangent 与 texture slot 必须做 semantic diff，压缩或去重不得改变设计真值。

商业链的每一阶段都必须消费上游 immutable identity，并产生独立 artifact/readback；不能把 evaluated triangle GLB 作为全链路 authoring source：

```text
AuthoringMesh@1
  → Native High / HighMeshArtifact@1
  → Retopology / LowMeshArtifact@1
  → HeroUvLayout@1
  → CageArtifact@1 + HighLowBakeReceipt@1
  → MaterialLayerGraph@1 / HeroMaterialPack@1
  → HeroLodSet@1 / CollisionSet@1 / SocketSet@1
  → EngineValidationReceipt@1 + HeroArtReviewReceipt@1
```

每一箭头都要绑定 `project_id`、`candidate_id`、`parent_artifact_sha256`、`input_sha256`、`output_sha256`、`worker_build_cohort_sha256`、recipe/operator/schema set hash 和 CAS lineage。Runtime 才能把结果接入 candidate/Stage；MCP、Viewer、Worker 均不能直接写 SQLite/CAS。当前 AuthoringMesh 仅 partial structural，Native High 为 source-only，Low 为 `DRAFT_UNREVIEWED`，Hero UV durable 为 structural/source 1/1；Formal High internal materializer 与 Cage/Bake Worker/public persistence seam 只有 source/compile/focused，完整 positive restart/public surface/current-D1 receipt 缺失且质量 failed；Surface/LOD/Engine/Hero Art Review 均 `NOT_RUN/NOT_PROVEN`。

每个 Worker 必须通过 `ForgeCadModule@1` manifest 才能进入编译图。该 manifest 至少带 `schema_refs`、`operator_refs`、有限 `budget`、正/负 `fixture_refs`、LICENSE/NOTICE/SBOM hashes、source/build `provenance`、`signature`、`module_sha256`、`contract_set_sha256` 与 input/output hashes，并明确无 network、dynamic plugin、script、direct DB/CAS write。缺少任一 receipt 时，Compiler 只生成诊断或 `queued` 状态，不生成 production artifact，不推进 Stage、confirm、version 或 export。

## 3. 从 Blender 借鉴的能力模型

| Blender 优点 | ForgeCAD 采用方式 | 不采用的部分 |
|---|---|---|
| Data-block 分离 | Geometry/Material/Texture/Image/Assembly 稳定 ID 和 hash | `.blend` 作为真值 |
| Modifier Stack / Geometry Nodes | typed Recipe DAG、非破坏候选和稳定 source map | 任意节点/Python |
| Principled BSDF | `MaterialGraph` 的 PBR 核心和 glTF lowering | 直接复制 Blender 内部状态 |
| UV/UDIM/Bake | `UvLayout`、`BakeRecipe`、交付 downconvert | 未声明的 tile 丢失 |
| Eevee/Cycles 工作方式 | 快速预览 + 固定高质量证据，两者共享真值 | Viewer renderer 变成产品真值 |
| Render passes/AOV | 固定 evidence passes | 只交 beauty 截图 |
| OCIO/AgX | scene-linear、显示变换和通道色彩语义 | 依赖显示截图的隐式色彩 |
| Asset Browser | CAS 资产目录、预览、license/provenance | 无许可证的素材拖入 |
| Outliner/Collections | Assembly/Part 层级、隔离、爆炸图 | legacy ModuleGraph |

Blender 永久保持 `reference-only / unavailable-for-product`：不作为 headless worker、fallback、导入器、烘焙器或 renderer 分发，也不进入 Runtime/Worker/Skill/lockfile/CAS 真值。ForgeCAD 只 clean-room 学习其 data-block、BMesh、Depsgraph、Modifier、UV/Bake、AOV 和色彩管理方法，并落实为自有 Rust/typed contract；Codex 和 Skill 不能提交 Blender Python。

参考：[Blender Geometry Nodes modifier](https://docs.blender.org/manual/en/dev/modeling/modifiers/generate/geometry_nodes.html)、[Blender Color Management/OCIO](https://docs.blender.org/manual/en/latest/render/color_management.html)、[Blender GPL 说明](https://developer.blender.org/docs/license)。这些链接用于学习能力模型，不授权复制文档、源码或资产。

## 4. 局部修改与回退

局部修改不直接编辑三角形数组。Codex 提交 `SemanticChangeSet`/`change_set`，引用稳定 Part/MaterialZone/source-map 和限定 operation；MVP `change_prepare` 要求完整新 Geometry/Appearance programs，并由 Runtime 在 base version 上重新编译候选，不宣称未受影响 DAG 已做增量复用。

候选内 undo/redo 只操作未确认 change stack。历史回退使用 `restore_prepare` 生成以旧版本为内容来源、以当前版本为父的新候选；批准后创建新版本，不改写历史头。任意 mesh 三方合并不在 P0。

## 5. 爆炸图

`ExplodedViewPlan` 包含 Part ID、层级、方向、距离、顺序、引导线、碰撞和相机 framing。默认计划来自 AssemblyGraph 包围盒和邻接关系；Viewer 可临时调节距离，永久保存需经 Codex prepare/confirm。质量门要求部件不遮挡关键邻接、标签可读、primitive lineage 未丢失。

## 6. Benchmark

每个 Compiler/Skill 至少有：

- deterministic unit fixtures；
- adversarial Schema/预算/路径测试；
- golden readback 和 render evidence；
- 跨类别参考集，不只机械硬表面；
- 性能、内存、取消和重启；
- 独立真人评分，结果与版本/hash 绑定；
- 明确的未通过项，不能用平均分掩盖类别失败。
