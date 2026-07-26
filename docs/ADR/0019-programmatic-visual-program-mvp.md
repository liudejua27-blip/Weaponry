# ADR-0019：AI 编写三维视觉程序的默认 MVP

- 状态：Accepted（2026-07-26；PV001 路线切换与 Rust 最小合同已实现，完整黄金路径未完成）
- 日期：2026-07-26
- 决策者：项目维护者
- 取代范围：取代 ADR-0018 将远程概念图和神经 Image-to-3D 设为 MVP 默认主链的决定；恢复并深化 ADR-0016/0017 的程序化视觉设计主线
- 保留：ADR-0014 Rust-first 所有权、ADR-0015 工件与视觉验收分离、ActiveDesignSnapshot、V003 单一结果、A005、R007、GLB/CAS/readback、已实现的可选 Provider 端口和历史迁移

## 1. 决策

Forge Studio 的默认产品模型改为：

> DeepSeek 像 Codex 编写软件程序一样编写 typed 三维视觉设计程序；Rust 校验、降低、版本化和验收；受限几何/表面编译器生成精致 GLB；用户只通过语言和授权参考持续创造、修改、检查和交付资产。

默认闭环：

```text
用户描述 / 本机只读参考
→ DeepSeek VisualDesignBrief
→ ForgeVisualProgram@1 + Detail Inventory
→ Rust validator / semantic hash / lowering
→ ShapeProgram + AssemblyGraph + Material Zone + Surface Program
→ RestrictedGeometryExecutor
→ GLB / PBR readback
→ fixed eight-view deterministic-first review
→ 最多两次同意图局部程序修复
→ 唯一未保存结果
→ preview → confirm → immutable version / Snapshot
→ 连续语言 ForgeVisualPatch
→ GLB / ForgeAssetPackage
```

远程图像和神经 Image-to-3D 不再是首次生成、MVP Gate、工作台启动或导出的前置条件。默认工作台不得要求 FAL Key、提示购买 FAL 或在用户发送普通 Brief 时调用 Fal。

## 2. 为什么取代 ADR-0018

ADR-0018 的远程路线可以较快返回一个纹理化 GLB，但会把核心体验变成按次付费的黑盒再生成：局部修改难以稳定保留未修改区域，输出不可解释，产品质量和成本依赖上游服务，且当前没有真实付费结果证明它达到目标视觉质量。

Forge Studio 已有 Rust-owned ShapeProgram、AssemblyGraph、A005/Surface Compiler、GLB readback、CAS、Snapshot、V003、版本和恢复。缺口是 Agent 可编写的高表现力设计源与视觉收敛，不是再增加一个必需的远程网格供应商。

## 3. img2threejs 采用边界

锁定研究基线为 [img2threejs/img2threejs](https://github.com/img2threejs/img2threejs) commit `ffe0ace9cfcb8686fd8473371ccbf0ffc2e906e0`，Apache-2.0。采用其：

- detail inventory；
- silhouette→structure→form→material→surface→lighting→optimization 阶段；
- 程序化 Three.js 形体与 PBR 方法；
- 确定性优先、多视角退化检查；
- 有界视觉修复和停止策略。

不直接采用：

- Agent 生成并执行任意 TypeScript/JavaScript；
- `THREE.Group` 作为资产、版本或几何真值；
- 尚在上游 roadmap 的 GLB/LOD/完整 UV/烘焙能力；
- 单图隐藏面推断为事实；
- 上游 Skill/runtime 直接进入零基础用户安装包。

任何上游代码进入产品前仍需单独的许可证/NOTICE、依赖体积、内存、打包、SBOM 和移除方案审计。本 ADR 当前只采用方法，不新增运行时依赖。

## 4. ForgeVisualProgram 真值

`ForgeVisualProgram@1` 是设计源封套，不是第二几何内核：

```text
ForgeVisualProgram@1
├── design_tokens
├── parts / hierarchy
├── geometry_graph: ShapeProgram@1
├── assembly_graph: AssemblyGraph@1
├── material_graph: part + Material Zone + MaterialPreset
├── surface_graph: SurfaceAdornment/SurfaceLayer program identity
├── detail_inventory: macro / meso / micro
└── export_profile
```

Rust 必须拒绝：未知字段、任意代码、空操作/输出、悬空 Part/parent/output/zone、重复材质目标、表面程序指向未绑定材质区，以及细节清单声称 `bound` 却没有真实 geometry/material/surface 输出。`sealed` 程序不能含 critical unresolved 项。

降低只产生现有 ShapeProgram、AssemblyGraph、Material Zone 和 Surface Program 输入；底层执行器、GLB 和 ActiveDesignSnapshot 继续是既有唯一真值链。

## 5. 连续对话修改

后续 `ForgeVisualPatch@1` 以 workspace revision 和 expected source hash 修改同一个 typed source：

- 轮廓/比例修改 geometry source；
- 局部结构修改 Part/operation；
- 材质修改 material binding；
- 图案、流线、磨损修改 surface source；
- “保持外形”锁定 geometry graph；
- “保持材质”锁定 material/surface graph。

所有永久修改仍执行 preview→confirm→一个不可变子版本。用户不需要查看或编辑内部参数、UV、拓扑或程序代码。

## 6. 可选 Provider 的处理

现有 Fal/神经代码不立即破坏性删除：

- Provider port、取消/超时/恢复、凭据隔离和有界下载作为通用基础设施保留；
- Fal-specific adapter 移到默认关闭的实验/扩展边界；
- 默认 UI、默认 Turn 和 MVP 聚合 Gate 不引用它；
- SQLite 0042 历史迁移保留，停止将其作为新主链前置；
- 未来有机体、毛发、布料或扫描类需求可在用户明确启用、授权和自带凭据时独立评估。

禁用 Provider 不得删除或降低 GLB readback、CAS、版本、质量、恢复和导出 Gate。

## 7. 实施顺序

```text
PV001 路线切换、默认 UI 退役 FAL、ForgeVisualProgram 最小合同
→ PV002 将 C111 机械臂黄金资产封装为可编译 ForgeVisualProgram fixture
→ PV003 DeepSeek typed author/inspect/patch Product Tool
→ PV004 固定 build passes、八视角与 Detail Inventory Gate
→ PV005 三轮连续语言修改、preview/confirm/restart/export
→ PV006 20 条未见机械硬表面 Brief 与真人视觉验收
→ PV007 逐领域扩展
```

第一阶段先证明未来工业机械臂收藏品；在该黄金路径达到视觉门以前，不扩展商城、游戏、工程 CAD、角色或生物。

## 8. MVP 退出条件

1. 默认启动和生成不需要 FAL Key，不产生默认付费请求；
2. 一句未预制完整模型的机械臂 Brief 形成真实 `ForgeVisualProgram@1`；
3. 每项 critical visible detail 绑定实际 geometry/material/surface 输出；
4. 编译为真实 PBR GLB，并通过 Rust readback 与固定八视角；
5. 工作台只显示一个结果且只有一个 renderer；
6. 同一资产至少三轮语言修改，几何锁定和材质锁定分别有证据；
7. preview/confirm/undo/restart/export 与同一 Snapshot/GLB hash 一致；
8. 20 条未见硬表面 Brief 全部能明确成功或明确失败，至少 15/20 独立真人视觉评分达到 4/5；
9. 不执行任意 JavaScript/Python/shell/URL/path，不把自动 Gate 冒充真人视觉结论。

PV001 只完成路线切换、默认 UI 和最小 Rust 合同，不证明以上完整 MVP 已完成。
