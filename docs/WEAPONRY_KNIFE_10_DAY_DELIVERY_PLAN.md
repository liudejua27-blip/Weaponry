# Weaponry 刀类 10 天商业交付计划

## 交付对象与状态边界

目标是一个授权穿越火线刀类资产和一个原创 control knife，通过同一套 Weaponry 工作流完成：

`授权/参考 → AuthoringMesh → Modifier/Evaluation → High → editable Low → Hero UV → Cage/Bake → PBR → FPS/Animation → Engine → Human review → approval/export`

当前源码通过的是 AuthoringMesh 多操作事务的 structural/durable/MCP slice，以及纯 Rust
Modifier/Dependency/EvaluatedMesh core 单元切片。它们不等于刀类商业资产已经完成。当前商业状态
保持 `NOT_PROVEN`，视觉、人审、引擎和 packaged 刀类验收均为 `NOT_RUN`。

## 不可省略的输入

- 合作方授权记录和参考图集，至少覆盖刀身主视图、侧/背面、握柄/护手细节和材质近景；
- 目标引擎、单位、坐标系、骨骼/插槽、贴图通道、纹理预算、LOD 和三角面预算；
- 至少一位独立武器美术审阅者；
- 固定 Blender/插件是否仅内部使用或会随产品分发的许可证决策。

这些输入缺失时仍可完成原创 control knife 和 pipeline source Gate，但不能写成合作方商业验收。

## Day 1 — scope freeze and knife profile

- 冻结授权 knife brief、control knife、目标引擎 profile 和验收矩阵；
- 生成当前 Tool/Schema/Skill/Runtime 模块 reachability 清单；数量只作为该次审计
  快照，不作为刀类能力或质量 KPI；
- 默认 Codex 面只保留刀类 workflow profile，legacy 路由进入显式 compatibility profile；
- 冻结 Blender 与插件候选 revision/license，不执行任意下载脚本。

Exit: Codex 默认不再面对 subject/version-specific 工具海；历史回放仍可恢复。

## Day 2 — AuthoringMesh and knife curves

- 完成 selection/query、稳定 ID、事务 journal 与 rollback 的刀类 fixtures；
- 增加 blade spine/edge/profile 的 typed curve、sweep/loft/tessellation 子集；
- 以一组事务形成刀身、刀尖、护手和握柄 primary form。

Exit: 两把刀均可确定性重放 primary form；invalid late command 零部分写入。

## Day 3 — modifier/evaluation graph

- 接通 Contract → Runtime → Store/CAS → MCP 的 ModifierGraph；
- 覆盖 mirror、array、boolean、bevel、solidify、subdivision、weighted normal；
- 实现 dirty closure、局部重算和 original/evaluated 分离。

Exit: 修改刀身一个节点不会重写原始网格或无关稳定 ID。

## Day 4 — High and sculpt-like knife detail

- 刀刃锋线、凹槽、锯齿、护手、缠绕/防滑纹与可控倒角；
- 若使用 fixed Blender prototype，仅允许走 ADR-0030 的隔离 provider 生命周期；它只能
  产生闭合 typed job 的 draft，不能写 Runtime/CAS/Stage；每个输出都必须经 Rust
  readback、预算、许可证/provenance 和差分 fixture 独立绑定。

Exit: High mesh 通过拓扑/法线/自交/固定视图检查；不以细分数量代替形体质量。

## Day 5 — editable Low and retopology

- 锁定 silhouette、刀刃、护手接缝和变形关键边；
- 生成可编辑 quad-flow Low 与 High↔Low correspondence；
- 独立美术检查 edge flow，不把 triangle decimation 冒充 retopo。

Exit: Low 可编辑、语义 Part 保留、固定视图轮廓不回退。

## Day 6 — Hero UV and cage

- seam、unwrap、pack、镜像/堆叠声明、texel density、stretch、padding；
- 生成逐 Part cage，检测穿透、miss、过近/过远和 opposing surface 风险；
- 由 Rust 持有 Low/UV/Cage lineage。

Exit: UV 与 cage 有可读诊断且由美术复核；无隐藏 fallback。

## Day 7 — Bake and material

- 生成 tangent-space normal、AO、curvature、thickness、ID；
- 固定 MikkTSpace/色彩空间和逐通道 readback；
- 完成刀刃金属、涂层、握柄、护手和受控磨损材质层。

Exit: 无未声明 overlap、硬边/seam/tangent 冲突、cage miss 或通道绑定漂移。

## Day 8 — FPS, animation and presentation

- 第一人称 idle、inspect、slash、stab 的非功能性视觉 clip；
- 手部占位、镜头穿插、近裁剪、遮挡、socket/hierarchy 和关键帧 readback；
- 固定 beauty/depth/normal/AO/part-ID/material-ID/wireframe/UV-stretch/silhouette。

Exit: clip/相机/资产 hash 同 lineage；不生成现实操作或性能结论。

## Day 9 — engine validation and repair

- 目标引擎真实导入，校验单位、坐标、材质、贴图、LOD、collision、socket、动画和 shading；
- 只修 evidence 定位的问题；重复导出与 Runtime restart 后 hash/readback 一致；
- Blender prototype 失败必须能回退到前一确认候选，不得成为单点真值。

Exit: `ENGINE_VALIDATED` 只能由目标引擎真实 receipt 产生。

## Day 10 — same-cohort acceptance

- 授权 knife 与 control knife 全链同 build 重跑；
- 独立武器美术按 silhouette、edge flow、bake、material、FPS 近景评分；
- 用户批准精确候选后 confirm/export；打包只包含通过 cohort。

Exit labels 独立：`AUTHORING_RUNTIME_PASS`、`GAME_READY_SURFACE_PASS`、
`VISUAL_REFERENCE_PASS`、`HUMAN_ARTIST_ACCEPTED`、`ENGINE_VALIDATED`、`PACKAGED`。

## 进度降级规则

若时间不足，先减少刀型数量、装饰纹样、通用 UI、节点编辑器和 Rust 替换范围；保留受控 Blender
内部 prototype。不得删减事务回滚、Low 可编辑性、UV/Cage/Bake 诊断、同 hash 回读、真实引擎门、
独立人审和明确批准。

## 每日 Gate 合同

下表是十天计划的精确退出条件。`source/compile` 只证明边界存在，
`candidate-bound` 才证明同一刀类候选已实际穿过该门；任何一项为
`NOT_RUN`、`NOT_PROVEN`、`BLOCKED` 或 `FAIL`，都不得把后续阶段写成
`PASS`。每天只能产生一个 reviewable candidate 或一个有原因的失败结果，
失败时保留上一确认 head，不覆盖历史版本。

| Gate | 必须锁定/产出的对象 | 精确退出条件 | 必须留下的证据 | 禁止用来替代的结果 |
| --- | --- | --- | --- | --- |
| `K0_AUTH_REFERENCE` | `KnifeBrief`、授权记录、ReferenceViewSet、Camera/Unit/Engine/预算 profile | 授权源 hash、允许修改/导出范围、项目接收方、五个核心参考视图和来源状态齐全；control knife 单独标为 original；阈值在首次运行前锁定 | canonical brief/reference/provenance hash、coverage matrix、profile hash、negative-input receipt | 合作方口头声明、单张图片、旧 receipt、Codex 推断或 Blender 文件 |
| `K1_AUTHORING` | original `AuthoringMesh`、stable V/E/H/C/F/loop/ring/Part IDs、`SelectionQuery/Resolution`、多操作 journal | move/split/extrude/inset/bevel/loop/merge/dissolve 的 profile-允许子集在同一事务中确定性重放；非法末尾命令全回滚；不相关稳定 ID、Part 和 MaterialZone 不变；无 degenerate/non-manifold/orientation violation | request/response canonical hash、before/after topology readback、replay byte/hash comparison、rollback negative receipt | primitive 参数搜索、单次 preview、GLB 可打开、triangle collapse 或人工替换网格 |
| `K2_FORM` | knife semantic Part/MaterialZone/source map、Primary/Secondary Form receipt | blade/edge, guard, handle, tang/ricasso 等语义边界可读；锁定的 front/back/left/right/rear-three-quarter 参考视图及 review-extended 视图均无 silhouette/negative-space/landmark 回退；相机和参考未被候选偷偷改变 | same candidate/artifact/reference/camera hashes、固定 AOV、Part-ID、silhouette/landmark metrics、Codex review and user decision | 只看 beauty、只看平均分、相机翻转、材质/VFX 遮挡、历史 FormArt |
| `K3_GRAPH_HIGH` | `ModifierGraph`、`DependencyGraph`、`EvaluatedMesh` link、独立 `HighMeshArtifact`/DetailGraph | node/edge ID 唯一、missing input/duplicate/cycle fail closed；topological order 和 dirty closure 稳定；disabled node 可追踪；AuthoringMesh 不被求值覆盖；High 独立 hash/readback，预算、法线、封闭性和固定视图高光通过 | graph/selection/input/output hashes、replay plan、High topology/normal/self-intersection/readback、Worker cohort/resource receipt | Modifier preview、未注册 Blender modifier、细分次数、source compile、Codex 自评 |
| `K4_LOW` | 独立 `LowMeshArtifact`、High↔Low correspondence、hard-edge/seam/Part locks | Low 是可编辑 quad-flow，刀刃锋线、护手接缝、柄部变形边和 socket 区域受保护；每个 bake surface 有可回读 correspondence；面向目标预算；固定视图无回退 | Low topology/edge-flow readback、correspondence coverage、feature-lock report、same-candidate hash and restart receipt | 自动 decimation、LOD triangle mesh、unreviewed retopo draft、仅渲染 Low |
| `K5_UV` | `HeroUvLayout`、UV0/UV1 policy、Mikk tangent input、island/seam records | overlap、out-of-bounds、zero-area、inverted island 均为 0；seam/hard-edge congruence 通过；visibility-weighted density、orientation、目标 mip padding 在 D1 锁定 profile 内；Mikk 输入可复现 | UV island/metric map、density/stretch/OOB/overlap report、padding calculation、tangent input hash/replay | 512 atlas、自动 pack 截图、UV replay 没有 Low binding、通过扩大纹理掩盖缺陷 |
| `K6_CAGE_BAKE` | 独立 High/Low/Cage、`HighLowBakePlan`、`BakeSet` 八类 map 和诊断 | Cage 与 Low topology 对应；per-Part ray isolation、normal/AO/curvature/thickness/position/object/material/Part-ID 输出可回读；miss、cross-Part hit、penetration、skew、bleed、未声明 fallback 为 0 或 D1 锁定阈值内；normal convention/Mikk/dilation 一致 | distinct high/low/cage/map hashes、ray histogram、heatmap、tangent/dilation readback、deterministic replay/restart receipt | self-surface bake、nearest/vertex fallback、8 张 PNG、source seam、只看 normal beauty |
| `K7_MATERIAL` | `MaterialLayerGraph`、mask/generator/decal/wear/microdetail、MaterialPack、channel/provenance | blade/guard/handle 层次和 roughness hierarchy 在 FPS/world 固定光照下可读；normal/AO/ID 与 UV/Bake exact binding；纹理通道、色彩空间、压缩解码 hash 和授权 provenance 均一致 | layer graph hash、per-channel decoded hash/color-space report、AOV/roughness/normal readback、asset license/SBOM/provenance | fixed-formula preview、只改变颜色、灯光/VFX、未绑定 Bake 的贴图 |
| `K8_FPS_LOD` | `WeaponPresentationRig`、hip/ADS/inspect/equip profiles、LOD0/1/2、collision/socket/animation events | 固定镜头下刀身、护手、握柄无关键遮挡、裁切、穿插、漂移；inspect/equip 节拍和 socket/hierarchy 可读；LOD、collision、socket 与 Material/UV/tangent lineage 一致且在目标预算内 | camera/clip/socket/LOD hashes、fixed AOV capture、occlusion/clipping report、animation tick/replay receipt | orbit 截图、单一 beauty、Viewer 临时状态、未绑定动画或碰撞元数据 |
| `K9_ENGINE` | `EngineDeliveryPackage`、目标引擎 importer/validation receipt、canonical/optimized semantic diff | 目标引擎 clean project import → save → reimport → restart → packaged run 全部成功；单位/轴向、mesh/material/tangent/LOD/collision/socket/animation 回读一致；target hardware p50/p95/p99、显存/纹理/streaming 在 D1 预算内 | exact export hash、import/reimport/restart receipt、decoded texture/tangent/semantic diff、performance report | glTF Validator、Three.js、文件扩展名正确、source compile、单次编辑器导入 |
| `K10_HUMAN_EXPORT` | `HeroArtReview`、revision closure、approval、immutable version/export/restart | 独立武器美术以盲审清单确认形体、edge flow、Bake、材质、FPS 和原创/IP；阻塞项关闭；用户批准精确 candidate；confirm/version/export 后 restart 仍读回同一 hash | reviewer identity/decision、修订前后 diff、approval receipt、version/export/restart hash | Codex typed review、内部自评、旧人审、口头批准或只导出 GLB |

### Gate 的可计算状态账本

以下账本是刀类首个 cohort 的计划基线，不是新 receipt，也不改写已有
`docs/evidence/**`。运行开始前应复制为一份 candidate-bound ledger，并把每格
替换为实际 receipt ID；未运行不得留空或写 `PASS`。

| 能力轴 | Day | 当前刀类 baseline | 允许的晋级状态 | 责任边界 |
| --- | ---: | --- | --- | --- |
| 授权/参考/预算 | 1 | `NOT_RUN`（本计划未绑定真实刀类 cohort） | `PASS_ASSET` only with source/reference/provenance hash | Runtime owns authorization binding; Viewer only reads coverage |
| AuthoringMesh/selection/transaction | 2 | `PASS_SOURCE`/`NOT_PROVEN`（现有切片不是刀类商业候选） | `PASS_ASSET` after exact replay + rollback + restart | Runtime sole writer; Store/CAS stores immutable artifacts |
| Form/semantic Part | 2 | `NOT_RUN` | `PASS_ASSET` with fixed-view non-regression | Worker renders evidence; Viewer cannot approve |
| Modifier/Dependency/Evaluated | 3 | `PASS_SOURCE`/`NOT_PROVEN`（纯 core slice 不等于 Runtime 接线） | `PASS_ASSET` after graph/High candidate readback | Core model, Runtime integration and MCP transport remain separate receipts |
| High | 4 | `NOT_RUN` for knife; existing `FPS-HIGH-05=NOT_PASSED` remains separate | `PASS_ASSET` only with independent High and visual/readback gate | Blender prototype is draft/provider lane, not source truth |
| Editable Low/correspondence | 5 | `NOT_RUN` for knife；既有 Low draft 不升级 | `PASS_ASSET` only with authored quad and correspondence | Retopo helper cannot promote itself |
| Hero UV/tangent | 6 | `NOT_RUN` for knife；既有 structural UV 不升级 | `PASS_ASSET` with zero-defect report and target profile | UV worker may propose; Runtime validates binding |
| Cage/Bake | 6–7 | `NOT_RUN`/`NOT_PROVEN`；formal producer availability must be checked | `PASS_ASSET` with distinct High/Low/Cage and diagnostics | No hidden fallback; old bake diagnostics remain historical |
| Material | 7 | `NOT_RUN` for knife；fixed-formula preview not enough | `PASS_ASSET` with evaluated layers and decoded channel hashes | Texture worker is bounded; provenance remains Runtime-owned |
| FPS/LOD/collision/socket | 8 | `NOT_RUN` | `PASS_ASSET` with fixed profiles and same lineage | Viewer is read-only; animation/engine receipts separate |
| Target engine | 9 | `NOT_RUN` | `PASS_ENGINE` only after clean-project round-trip and budget | Engine adapter returns receipt; it cannot confirm version |
| Independent human review | 10 | `NOT_RUN` | `PASS_HUMAN_ART_REVIEW` only with reviewer identity and closure | Codex review is non-substitutive |
| Confirm/version/export/restart | 10 | `NOT_RUN` | final only after all preceding labels are PASS | Runtime sole confirmer; failed candidate remains recoverable |

状态账本必须同时显示 `structural_status`、`candidate_status`、
`visual_status`、`human_status`、`engine_status`、`distribution_status`。
不得以 `PASS_SOURCE`、`PASS_COMPILE`、`GLB_READBACK_PASS` 或
`BLENDER_PROTOTYPE_PASS` 填充商业阶段的 `PASS_ASSET`。

## Definition of Done：`KNIFE_VERTICAL_SLICE_DONE`

只有一个授权刀类候选和一个 original control knife 在同一 build、同一套
generic workflow、同一 `candidate_hash → export_hash` 上满足以下全部条件，
才能写 `KNIFE_VERTICAL_SLICE_DONE`：

1. Day 1 的授权、参考覆盖、相机/单位/目标引擎/预算 profile 被锁定，且每个
   私有源、纹理和导出目标都有 hash 与 permitted-use provenance。
2. 原始 AuthoringMesh、SelectionQuery/Resolution、stable semantic IDs、事务
   journal、invalid rollback 和 restart replay 可读回；original 与 evaluated
   不混为一个真值。
3. ModifierGraph/DependencyGraph 通过 unique/missing/cycle/ordering/dirty
   closure Gate；EvaluatedMesh 只链接 source/graph/input/output hashes，不保存
   第二份可编辑 mesh truth。
4. Primary/Secondary Form 在五个核心视图和锁定 FPS review views 中通过
   silhouette、landmark、negative-space、Part-ID 和 no-regression Gate。
5. High 是独立可读的非破坏细节对象；Low 是 artist-editable quad 对象；两者
   有逐 bake surface correspondence、预算、hard-edge/seam/Part 保护。
6. Hero UV 的 overlap/OOB/zero-area/inverted 全为零，density/stretch/padding/
   UV0/UV1/Mikk policy 通过；不能用更大纹理或自动 pack 隐藏问题。
7. Cage/Bake 具有 distinct High/Low/Cage/map hashes、per-Part diagnostics、
   no hidden fallback、目标切线约定和 deterministic replay。
8. MaterialLayerGraph 真正求值，MaterialPack 的通道、色彩空间、粗糙度、法线、
   AO、wear/decal/microdetail 和 provenance 可解码回读；preview plan 不算完成。
9. FPS/Animation 的 hip、ADS、inspect、equip 和 world profiles 无关键遮挡/穿插；
   LOD0/1/2、collision、socket、animation event 的语义和预算可回读。
10. 目标引擎 clean-project import/save/reimport/restart/packaged run、semantic
    diff、性能预算和导出后重启 hash 全部通过。
11. 独立武器美术完成盲审及修订闭环，用户明确批准 exact candidate；Runtime
    confirm/version/export 后仍保持同一 lineage/hash。

最终状态必须拆开写：`AUTHORING_RUNTIME_PASS`、`GAME_READY_SURFACE_PASS`、
`VISUAL_REFERENCE_PASS`、`HUMAN_ARTIST_ACCEPTED`、`ENGINE_VALIDATED`、
`PACKAGED`、`RELEASED`。任何本地 GLB、Three.js、历史 evidence、Codex 自评、
Blender prototype 或文档本身都不能替代缺失的商业标签。若授权、目标引擎或
独立人审不可用，结果保持 `NOT_PROVEN`/`BLOCKED`。

## 刀类任务覆盖成熟度（替代 Blender 子系统成熟度）

Blender 本身不是 Weaponry 的公共 DCC 或产品真值；经 ADR-0030 审核的固定版本可处于
`ISOLATED_INTERNAL_PROTOTYPE`。不能把 BMesh、Sculpt、NURBS、Depsgraph、
UV、Bake、Material 或动画子系统的“成熟”写成 Weaponry 产品指标。下面的
`K0–K4` 衡量的是同一刀类任务在 Weaponry generic Rust/typed workflow 上的
覆盖深度。内部 Blender prototype 若获 ADR-0030 provider Gate，只能作为
`K2` 之前或并行的 draft/provider 证据，不能越过 Rust readback、candidate
lineage、视觉、人审和引擎 Gate。

| Level | 刀类任务含义 | 最低证据 |
| --- | --- | --- |
| `K0_REFERENCE_ONLY` | 仅借鉴 Blender/外部 DCC 的概念；没有产品执行能力 | 研究、许可证和威胁模型记录；不得写 Runtime/Stage/质量 PASS |
| `K1_TYPED_BOUNDARY` | 任务拥有 closed typed contract、预算、hash 和负向约束 | contract review、canonical hash、unknown/path/URL/script/secret rejection |
| `K2_SOURCE_STRUCTURAL` | Rust Core/Runtime/Worker/Viewer source path 可编译且有确定性测试 | focused positive/negative/replay/readback；不得晋级真实候选 |
| `K3_CANDIDATE_BOUND` | 同一授权刀与 control knife 真实执行，持久候选/CAS lineage 与 restart readback 闭合 | candidate-bound artifact hashes、Store/CAS reachability、exact restart/readback |
| `K4_COMMERCIAL_ACCEPTED` | 同一 export hash 通过视觉、独立人审和目标引擎 | fixed views/FPS evidence、artist sign-off、engine round-trip、approval/export |

| 刀类任务面 | 需要覆盖的 generic 能力 | K4 退出门 | 当前诚实基线 |
| --- | --- | --- | --- |
| AuthoringMesh / Selection | stable topology IDs、SelectionQuery、multi-operation rollback | Day 2 replay/rollback + Day 3 semantic Part/form readback | 现有 AuthoringMesh/transaction 与纯 core slice 仅是 structural；刀类 K3 未运行 |
| Modifier / Dependency / Evaluated | ordered graph、dirty closure、disabled traceability、original/evaluated split | Day 3 graph + Day 4 High same-lineage readback | source/core ≠ Runtime candidate evaluation；刀类 K3 未证明 |
| High / hard surface | support loop、bevel/normal、bounded boolean/subdivision（若 profile active） | Day 4 High visual/readback/no hidden fallback | `FPS-HIGH-05=NOT_PASSED`；刀类候选未运行 |
| Editable Low / correspondence | authored quad flow、feature lock、High↔Low mapping | Day 5 independent Low and correspondence | 既有 draft 不等于刀类 artist-authored Low |
| Hero UV / tangent | seam/island/density/stretch/padding、UV0/UV1、Mikk | Day 6 zero-defect UV report | 既有 structural UV 不等于刀类 K3/K4 |
| Cage / Bake | per-Part cage/rays、8 maps、miss/cross-hit/skew/bleed diagnostics | Day 6–7 distinct hashes + exact replay | formal producer/刀类 Bake `NOT_PROVEN` |
| Material / surface | typed layers/masks/wear/microdetail、color-space/provenance | Day 7 decoded channels + FPS/world readability | fixed-formula preview only |
| FPS / animation | hip/ADS/inspect/equip、socket/hierarchy、safe region、occlusion | Day 8 fixed capture/readback | 刀类 FPS `NOT_RUN` |
| LOD / collision / socket | authored LOD continuity、collision/socket budgets | Day 8 same-lineage readback | commercial metadata `NOT_RUN` |
| Engine delivery | target importer/tangent/material/LOD/collision/socket/animation/performance | Day 9 clean-project round-trip | Unreal/Unity `NOT_RUN` until real receipt |
| Human/provenance/export | blind artist review、authorization、revision closure、approval | Day 10 same export hash | human/package/release `NOT_RUN` |

成熟度取所有必需任务面的最低等级，不取平均，也不按 Tool/Schema 数量计分。
一个 Blender prototype、一个漂亮截图或一个历史 receipt 只能改变对应 lane 的
研究状态，不能提升 `K3/K4`。
