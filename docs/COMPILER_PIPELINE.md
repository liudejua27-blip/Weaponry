# ForgeCAD 3D 编译器与质量管线

版本：2026-08-07
状态：目标设计；现有确定性算法需迁移后重新验收

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

### 2.1 Design Compiler

输入：参考证据、目标用途、尺寸/风格/预算、已安装 Skill 能力。
输出：`SubjectProfile`、`RepresentationPlan`、语义 `AssemblyGraph`、Part/MaterialZone 稳定 ID、未知项和 limitation。

类别开放不等于任何表示都可用。每个 Part 显式选择 procedural、deformable、surface、imported-readonly 或 hybrid；不存在可靠路线时停止并要求更多视图/人工建模，不能回退机械模板。

### 2.2 Geometry Compiler

输入：`GeometryProgram`、Part scope、预算。
能力：曲线、profile、loft、sweep、surface network、solidify、受限 field/CSG、局部 lattice/deform、read-only mesh admission。
输出：规范化 geometry IR、primitive、stable source map、LOD/collision candidates 和 readback。

每个 Operator 都有固定 Schema、单位、坐标系、数值范围、复杂度、三角/内存/时间上限和 deterministic hash。禁止动态 import、反射调用、任意参数路径和未签名脚本。

### 2.3 Appearance Compiler

输入：`MaterialGraph`、`TextureSet`、`UvLayout`、`BakeRecipe`、MaterialZone binding。
输出：经过验证的 UV、切线、PBR 通道、预览与交付纹理、材质表和 provenance。

P0 材质遵循 Principled 金属/粗糙度工作流：BaseColor、Metallic、Roughness、Normal、AO，按需 Emissive/Opacity。BaseColor/Emissive 为颜色数据，Metallic/Roughness/Normal/AO 为 non-color 数据。Authoring 可使用 UDIM，GLB 交付必须显式 bake/downconvert 到受支持的 0–1 UV 和纹理集，不能隐式丢 tile。

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

Blender 可作为受签名的 headless worker，用于产品内固定的导入、烘焙或渲染 Recipe；Codex 和 Skill 均不能提交任意 Blender Python。是否随产品分发 Blender、GPL 边界和 worker 隔离须在 `FGC-MCP012` 单独通过许可证审查。

参考：[Blender Geometry Nodes modifier](https://docs.blender.org/manual/en/dev/modeling/modifiers/generate/geometry_nodes.html)、[Blender Color Management/OCIO](https://docs.blender.org/manual/en/latest/render/color_management.html)、[Blender GPL 说明](https://developer.blender.org/docs/license)。这些链接用于学习能力模型，不授权复制文档、源码或资产。

## 4. 局部修改与回退

局部修改不直接编辑三角形数组。Codex 提交 `SemanticChangeSet`，引用稳定 Part/MaterialZone/source-map 和限定 operation。Runtime 在 base version 上准备候选；未受影响的 hash 尽量复用，受影响的 downstream DAG 显式失效。

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
