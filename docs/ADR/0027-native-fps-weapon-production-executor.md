# ADR-0027：ForgeCAD 原生 FPS 武器美术生产执行器

日期：2026-08-23

状态：Accepted as target architecture；当前实现仍为 `QUALITY_TARGET_NOT_MET`

替代范围：补充 ADR-0025/0026；收紧所有可能暗示 Blender 运行时、Blender Worker 或商业质量已完成的表述

## 1. 决策

ForgeCAD 的第一垂直目标是合法、虚构、不可制造的 FPS 游戏武器 Hero Asset。Codex 是外部美术总监与编排大脑，ForgeCAD Runtime 是唯一状态写者，ForgeCAD Worker 是唯一高信息量美术生产执行器。

产品不安装、不启动、不调用、不捆绑 Blender，也不存在 Blender Worker、Blender fallback 或 `.blend` 真值。禁止 `bpy`、Blender Python、BlenderMCP、任意脚本和通过临时导出绕过 Runtime。Blender 仅作为公开概念和官方源码的 reference-only 研究对象；任何可采用思想必须转写为 ForgeCAD 自有 closed Schema、Rust 实现、typed Worker 协议、canonical hash、资源预算和独立证据。复制源码、链接 Blender/Cycles 或采用 GPL 文件必须另行完成逐文件许可证决策；本 ADR 不授权复制。

“对标无畏契约、生死狙击等游戏”只定义表现级别、第一人称可读性和生产完整度，不授权复制它们的枪械造型、皮肤、标识、贴图或独特视觉资产。输出必须是原创或基于用户有权使用的参考。

## 2. 第一性原理判断

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

合并后的 source baseline 为 411 contracts、28 个 operator catalog entries、91 read + 69 explicit write = 160 MCP tools。该数量只描述可调用表面，不描述视觉成熟度。

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
| Reference Coverage | front/back/left/right/top/rear-3/4、遮挡与推断标签 | 未审 target、关键视图缺失、授权不明 |
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

冻结 AB/open-stock/camera/reference；把 left/right/front/top/rear-3/4 target 变为 reviewed。一次只改一个 assembly variable，优先 receiver-main/upper/lower clearance、muzzle/core、trigger void。每次固定执行 `geometry_program_hash → geometry_prepare → artifact_readback_get → one compare/render → boundary_error_get/silhouette_part_error_get`。无裁切且多视图 Boundary F1 达标前锁住 PBR。

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

在 P0 的 1–5 完成前，不扩展 Shader Graph、通用角色系统、任意脚本插件、Cycles/EEVEE parity、UDIM 全功能或通用 CAD 类别。

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
