# ADR-0022：通用参考条件 3D Agent 与能力沙箱

日期：2026-07-29
状态：Accepted

## 1. 决策

Forge Studio 的目标产品改为**类别开放的通用参考条件 3D Agent**：用户上传什么对象的授权图片并描述什么对象，系统就以该对象为目标进行外观重建或创作，不再用“机械硬表面”“四个 Domain Pack”或固定关键词白名单决定用户能不能开始生成。

目标类别包括但不限于：机械与产品、角色与人形、动物与生物、植物与自然物、家具与生活物品、建筑与环境道具、车辆与航空器，以及这些类别的混合设计。机械硬表面仍是当前实现最成熟、首个重点优化和回归分布，但不再是产品永久边界，也不得让未知输入静默回退为机械臂、未来武器或任何固定模板。

本 ADR 取代 ADR-0020、ADR-0021 中将机械硬表面作为产品类别上限、要求 E005 先于通用路由退出的部分。两份 ADR 的轻量化、外观优先、单一 Rust-owned 真值、`1 + 1` 调用预算、版本确认、成本纪律和未见任务验收继续有效。

## 2. “输入什么，生成什么”的准确含义

这句话是产品目标，不是对当前 Alpha 或单张图片不可观测内容的虚假保证：

1. 系统必须保持用户对象身份、主要轮廓、部件关系、比例、颜色、材质、表面特征和风格，不得将其重解释成现有模板；
2. 单图看不到的背面、内部、遮挡区和真实尺度属于不确定信息，系统应给出视觉一致的可编辑假设，并记录 `inferred`，或请求补充视图；
3. 若当前执行器或 Provider 无法达到最低质量，返回 `needs_more_views | representation_unavailable | quality_limited | provider_unavailable`，不得伪造成功；
4. “相同”以可观察外观和跨视图一致性为核心，不代表身份复刻、工程尺寸、内部结构或制造正确性；
5. 当前 `USER_GUIDE.md` 只描述已经通过 Gate 的能力，通用类别在未通过混合分布盲测前必须标为目标设计。

2026-07-29 的实施优先级补充：U004 在新表示尚未通过真人门之前按质量优先推进。允许为主体补全、最高受检质量档和严格 readback 增加等待，但不得用省一次调用、降低网格/PBR 质量或提前展示不完整主体换取速度；时间与费用仍记录并受显式授权限制，待质量基线成立后再优化。

## 3. 从类别路由改为表示能力路由

删除运行时的产品类别 allowlist。Agent 先形成两个通用合同：

- `SubjectProfile@1`：对象身份、可见部件、宏观轮廓、中观结构、微观细节、姿态、材质、风格、遮挡与不确定性；
- `RepresentationPlan@1`：为每个部件选择最合适的表示，不把整个对象强制塞进同一种几何语言。

三种当前目标表示可以在同一资产内组合。ADR-0023 已取代本 ADR 中第三方神经候选的供应商路线；兼容 `mesh_seed` 枚举保持 unavailable：

| 表示 | 适合内容 | 产品角色 |
| --- | --- | --- |
| typed procedural | 机械、产品、建筑件、规则轮廓和重复结构 | 可编辑性高、成本低的默认路径 |
| parametric/deformable | 角色、动物、软体姿态和可变比例 | 骨架、曲面模板、形变与语义分件 |
| local hybrid | 机械+生物、角色装甲、复杂形变底模+程序化细节 | 统一外观质量和后续局部编辑 |

目标编译链：

```text
Prompt + sealed image(s) / current asset
  -> SubjectProfile@1
  -> VisualFeatureContract@1
  -> RepresentationPlan@1
  -> UniversalAssetSource@1
       procedural_program?
       parametric/deformable source?
       local deformable/hybrid source?
       material/projection source?
  -> Rust validation / budget / lineage
  -> restricted compilers
  -> one GLB/PBR + strict readback + fixed multiview
  -> visual comparison + at most one typed patch
  -> user confirm
  -> AgentAssetVersion + ActiveDesignSnapshot
```

`UniversalAssetSource@1` 是统一资产源 envelope，不是第二几何真值。派生网格、纹理、截图、视觉报告和缓存必须绑定 source hash、Provider/编译器版本、参考证据与预算。

## 4. 深度吸收 img2threejs

锁定研究基线为 `img2threejs/img2threejs@8b53125081c3798cf95bd517b64be024515a1c8d`。吸收以下方法：

- 宿主视觉 Agent 先读图，再写结构化对象描述，而不是先做类别关键词匹配；
- `ObjectSculptSpec` 式对象分解、component binding、topology class 和 silhouette-first 构建；
- macro / meso / micro `Detail Inventory`，先覆盖高显著性结构，再补表面细节；
- 固定相机、投影/构图求解、去光照、纹理投影、PBR 分离和多视图一致性检查；
- build→render→compare→correct 的闭环，以及预算、平台期和振荡停止条件；
- 对物体、角色和混合对象使用开放式程序组合，而不是为每种整机建立模板。

源码审计后的真实边界必须写入采用判断：上游当前真正闭环的是通用 `ObjectSculptSpec` 组件表示和单一 Three.js generator；默认 spec 仍只是一个 box，具体对象理解与分件主要由宿主 Agent 完成。Character 路由目前是固定风格化半身模板，anatomy 尚未真正驱动几何；Creature 没有专用 CLI/schema/validator/generator；Hybrid 目前只是 Character 路由别名，不是机械+生物的混合表示。人物骨骼、蒙皮、blendshape、通用投影烘焙和环境/多对象仍主要属于目标或 roadmap。Forge Studio 不能复制这些超前声明，必须为每条表示建立真实 compiler/readback/Gate。

同样，img2threejs 的 camera solve 当前主要产生宽高比、默认 FOV 和待 Agent 填写的姿态；通用 projection 脚本主要生成计划，不执行完整 UV rasterization；照片 PBR 是确定性启发式，不是 inverse rendering。高质量 showcase 的关键优势来自单对象手写平面 UV、材质区、粗糙度/金属度/AO/normal 和磨损规则。Forge Studio 应把这种手工外观工程抽象成受限、类型化、可复现的通用 Appearance Compiler，不能把脚手架命名当作功能已实现。

不直接复制以下机制：

- 不让运行时 Agent 执行任意 TypeScript、JavaScript、Python 或 shell；
- 不把 5–8 次串行写码/截图审查作为默认用户等待路径；
- 不把手写 showcase、单一英雄角度或 `THREE.Group` 能渲染当成未见任务和资产质量证据；
- 不让浏览器场景、Provider 输出或外部网格绕过版本、readback、确认和恢复。

Forge Studio 的升级点不是“比上游多一些 primitive”，而是把上游的开放视觉理解与程序组合，压缩成一次高信息密度 author + 最多一次视觉 patch，并补齐 typed IR、混合表示、单一资产真值、确定性预算、可编辑分件、GLB/PBR/readback、恢复和正式盲测。

## 5. 创作边界与执行安全边界

本决策**解除对象类别限制，不解除执行安全**。两者不能混为一谈。

允许：任何合法的非功能性视觉对象、虚构角色/生物/机械、艺术化外观、混合类别和用户拥有权利的参考。

运行时仍禁止：任意代码执行、shell、动态 import、直接文件路径、任意 URL/网络、环境变量/密钥访问、未声明插件、无界循环/递归、未授权付费调用和超预算资源。所有外部结果只能作为受检候选进入统一 source 和版本链。

Forge Studio 是视觉资产工具，不输出现实武器制造细节、危险功能机构、制造工艺，也不对车辆、飞机、建筑、机械或人体提供安全、结构、医疗、适航、控制或认证结论。这些是输出用途和执行能力边界，不是 3D 外观类别边界。

## 6. 新的实施顺序

1. `FGC-U001`：冻结本 ADR、产品范围、文档和当前/目标边界；
2. `FGC-U002`：`SubjectProfile@1`、`VisualFeatureContract@1`、`RepresentationPlan@1` 与类别开放的 multimodal author request；
3. `FGC-U003`：`UniversalAssetSource@1`、通用 component/detail/material/projection 合同，以及未知对象不得模板回退；
4. `FGC-U004A`：DeepSeek/千问唯一 AI Provider 与旧远程 Mesh 运行时退役；
5. `FGC-U004`：程序化、形变模板与 local-hybrid 的能力路由和统一 readback；
6. `FGC-U005`：跨类别冻结未见集、真实单图/多视图、首轮/一次 patch、时间/成本与真人盲评；
7. 通过 U005 后推进打包、商业试用和按质量数据扩充本地算子，而不是恢复类别白名单或第三方 Mesh API。

E005 保留为机械硬表面回归与程序化表示子基准，不再是通用产品入口的前置条件。N201 被 U004 吸收。任何旧 `unsupported domain` 分支都必须迁移为表示能力诊断；在 U002 完成前，当前 Alpha 可以明确拒绝尚不支持的输入，但不能替换成机械臂结果。

U002/U003 的关键视觉合同至少包括：

- `ReferenceCameraHypothesis@1`：投影类型、参数、来源、重投影误差、landmark/轮廓证据、置信度和未解析字段；没有拟合证据时不得命名为 solved camera；
- `VisualDetailClaim@2`：macro/meso/micro、证据区域、observed/inferred/hidden/conflicting、Part/Material Zone/Surface/Geometry binding、geometry/normal/roughness/albedo/emissive 表示、轮廓影响和最低可见视图；
- `AppearanceEvidenceBundle@1`：原图、mask、区域、相机假设、近似 de-light、启发式 PBR 派生物、算法版本和全部 hash；派生物必须标记 evidence，不自动成为材质真值；
- `MaterialZoneAppearance@1`：基础材料、finish、coating/clearcoat/transmission、macro pattern、meso relief、micro response、wear、projection layer、来源 claim 与不确定性；
- 受限 projection compiler：相机投影、可见性/朝向过滤、UV rasterization 和未观测 texel mask，禁止任意 shader/TypeScript；
- 同源 clay/map-stripped、neutral PBR、grazing-light、reference-light、Part ID 和 Material Zone ID 验收视图，防止英雄角度投影用二维相似度掩盖错误三维结构。

## 7. 完成门

通用能力只能由混合分布证明。U005 至少覆盖机械/产品、角色/人形、动物/生物、植物/自然物、家具/生活物、建筑/环境、载具和混合对象；每类包含纯文本、真实单图、真实多视图或已有资产编辑中的适用模式。

分别报告：身份保持、轮廓、部件结构、材质、跨视图、可编辑性、首轮成功率、一次 patch 后成功率、P50/P90 时间、Provider 调用/成本、失败类型和独立真人评分。类别平均分不得掩盖某一类完全失败。没有正式证据时只能说“类别开放目标”或“best effort”，不能说“已经支持任意图片生成任意高质量 3D”。

## 8. 后果

正面：产品自由度由输入对象和表示能力决定，不再由模板目录决定；同一个 Agent 可以逐步吸收新的本地几何/形变能力而不改变用户心智；机械硬表面已有投资成为通用系统的一条高质量表示路径，而不是产品牢笼。

代价：需要真正的通用视觉合同、本地混合表示和角色/生物形变；单图歧义必须诚实处理；正式基准规模扩大。小公司不自训基础 3D 模型，也不购买第三方 Mesh API 兜底，因此必须把核心投入放在千问视觉理解、DeepSeek 受限 author、通用资产源、本地算子、快速验收、可编辑性和成本控制上。
