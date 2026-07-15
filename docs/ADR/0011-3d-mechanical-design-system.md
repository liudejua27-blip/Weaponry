# ADR-0011：以轮廓、特征与组件配方构建 3D 机械设计系统

- 状态：Accepted（目标设计；新增几何任务均未完成）
- 日期：2026-07-15
- 决策者：项目维护者
- 补充：ADR-0010 的视觉真实度与单一最佳结果决策

## 背景

当前 Alpha 主要依赖低多边形 primitive、少量参数材质和有限视觉细节。它可以验证 Agent 资产、Snapshot、受限编辑和 GLB 路径，但汽车、飞机、家电、机械臂与其他生活机械的主壳体仍过于方正，曲率、边缘、材质区、UV 和纹理不足，外观与真实产品差距明显。

讨论过两条替代思路：用 HTML/CSS/SVG 分别制作多个面再拼接，或持续从立方体做裁剪。前者不能可靠形成封闭实体、连续法线、UV 和可导出的几何真值；后者适合局部硬表面开孔，但单独使用会限制曲面质量并放大布尔拓扑问题。

## 决策

1. ForgeCAD 采用“3D 机械设计系统”作为目标建模架构：借鉴 UI 组件库的 Token、Recipe、Props、版本和预览思想，但模型真值仍是受限 ShapeProgram、AssemblyGraph 与 GLB readback。
2. HTML/React 只负责工作台；SVG 只编辑规范化 `ProfileSketch@1`；GSAP 只表现状态过渡。DOM、SVG path 和动画都不能成为网格、版本、质量或导出真值。
3. 主形体优先使用轮廓、放样、扫掠和旋转：平直外壳使用 Profile + Extrude，连续外壳使用多截面 Loft，轴对称部件使用 Revolve，管状/框架部件使用 Sweep。
4. CSG 保留为局部工具，用于窗洞、轮拱、进气口、凹槽和有限组合；不把所有对象都从 box 开始。布尔操作进入生产运行时前，必须先比较现有 Worker、Manifold Python 和 Manifold WASM 的许可证、体积、冷启动、内存、确定性、失败诊断、材质区保留与目标平台打包，只选择一种默认实现。
5. ShapeProgram 保存有序、不可变的特征节点和输入 hash；派生顶点不是编辑真值。每个节点必须在 `ShapeProgramRuntimeManifest@1` 中存在并具有真实执行器、预算和 readback；未实现操作明确拒绝，不能删除节点后继续成功。
6. 新增目标合同：`MechanicalStyleToken@1`、`ProfileSketch@1`、`ProfileSectionSet@1` 和 `EditableComponentRecipe@1`。Token 只表达相对视觉比例；Recipe 组合轮廓、特征、受限参数、Connector/pivot、Material Zone、child slot、领域和质量 Profile，不包含任意代码或工程参数。
7. 表面完成顺序固定为受控边缘处理、法线、UV0、tangent、稳定 Material Zone，再进入 PBR 纹理、glTF 扩展、HDRI 和色彩管理。字段存在不能替代真实 GLB readback。
8. SDF/体素可以作为早期、可丢弃的形体探索候选，但不能成为可编辑资产、Material Zone、UV 或最终 GLB 的权威源。
9. Agent 根据对象结构、Domain Pack、Style Token、Recipe 和运行时白名单自动选择建模语法；零基础用户不选择几何内核、操作符或三个方向。最终只展示通过真实编译/readback/渲染/评审硬门的一个最佳结果。

## 后果

- 新增 `FGC-G820`–`FGC-G826` 几何子链，并把组件配方与最佳候选安排在几何、表面和多材质事实完成之后。
- `FGC-G819` 与 `FGC-Q003` 仍是任何新几何之前的强制基础，当前唯一 ready 不变。
- 当前 USER_GUIDE 不增加轮廓编辑、Loft、Sweep、稳健布尔或 Recipe 操作；目标操作见 `MECHANICAL_DESIGN_OPERATIONS.md`。
- 生产运行时仍只有一个几何真值、一个 Agent Orchestrator、一个 ActiveDesignSnapshot 和一个 WebGL renderer。
- 该路线提高的是非功能性概念资产的外观与可编辑性，不提供 B-Rep、STEP、制造尺寸、工程材料、结构、气动、安全、控制或认证结论。

## 被否决方案

- HTML/CSS 六面拼接作为最终模型：缺少实体、连续曲率、可靠法线、UV 和导出真值。
- 只用立方体和布尔雕刻所有产品：曲面语言单一，拓扑和 UV 风险随复杂度快速增加。
- 同时保留多个生产几何内核：会产生重复执行语义、打包成本和不一致 readback。
- 直接把神经 3D/SDF 输出当可编辑资产：部件、参数、材质区、UV 和版本来源不稳定。
- 用 GSAP/CSS 动画生成或修改几何：把展示状态误当模型与版本事实。
