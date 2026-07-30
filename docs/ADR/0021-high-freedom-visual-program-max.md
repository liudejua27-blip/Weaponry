# ADR-0021：高自由度 ForgeVisualProgram MAX 路线

日期：2026-07-29
状态：Accepted

2026-07-29 取代说明：ADR-0022 已取代本文将机械硬表面作为产品类别上限、要求 E005 先于通用能力路由退出的部分；`ForgeVisualProgram@2`、1+1、Rust 校验、静态预算、readback 和安全边界继续作为程序化表示有效。

## 1. 决策

ForgeCAD 的下一阶段不再用“继续手工加深一台黄金机械臂”作为生成能力的前置门。项目改为先建立 `ForgeVisualProgram@2`：一种由模型一次规划、由 Rust 校验与编译、可以表达高差异机械硬表面外观的受限 typed design source。

`FGC-C111B` 保留为未通过视觉相似度和真人评分的回归资产，状态改为 `superseded`；其合同、fixture、预算门、readback、八视图和 packaged evidence 不删除、不降阈值，也不得写成已通过。新路线先证明语言能用更少的模型往返产生更广的结构，再在冻结未见集上证明首轮质量。

ADR-0020 的小团队、轻量、外观优先、单一 Rust-owned 真值和成本约束继续有效。本 ADR 只取代“必须先完成 C111B，才能开始设计语言”的实施顺序，不取代安全、版本、确认、导出或人工质量门。

## 2. 产品定义

目标不是一个会不断写任意 Three.js/Blender 脚本的聊天机器人，而是一个“视觉 3D 编译器 + Agent”：

1. 用户输入提示词、授权图片或多视图参考；
2. Agent 一次生成紧凑、参数化的 `ForgeVisualProgram@2`；
3. Rust 做 schema、类型、单位、预算、资源、谱系和能力校验；
4. Rust 将其展开为 `ExpandedVisualDAG@1`，再 lowering 到现有 `ShapeProgram`、`AssemblyGraph`、Material/Surface Graph；
5. 受限 worker 编译 GLB/PBR，唯一 renderer 产生固定多视图；
6. code-owned hard gate 先验收；可选视觉 Provider 只对可见误差给出 typed patch；
7. 最多一次同意图 patch 后给出唯一未保存结果；用户确认才创建版本和 Snapshot。

“高自由度”由语言组合能力和未见任务结构差异证明，不由整机模板数量证明。“高质量”由冻结参考、多视图、PBR/readback、独立真人评分和失败分布证明，不由 triangle 数、模型自评或单张最好截图证明。“轻量”同时约束安装体积、默认算力、Provider 调用数、生成时间和长期维护成本。

大量 token 与长串行时间不是同一个问题。ForgeCAD 可以允许 author 在一次调用中进行较深的图像理解和设计推理，但不能把一个对象拆成 5–8 次模型串行创作循环。首轮输出必须是高语义密度的完整设计程序；展开、细节实例化、编译、PBR、固定视图和确定性诊断由本地流水线并行或增量完成。目标是“允许模型想得深，但只让用户等待一个 author 和最多一个视觉 patch”。

## 3. 单一真值与编译链

```text
Prompt + sealed ReferenceEvidence
  -> ForgeVisualAuthoringIntent
  -> ForgeVisualProgram@2                 # 可编辑设计源
  -> ExpandedVisualDAG@1                  # 确定性派生缓存，不是第二真值
  -> ShapeProgram + AssemblyGraph
     + MaterialGraph + SurfaceGraph
  -> RestrictedGeometryExecutor
  -> GLB/PBR + strict readback
  -> SingleResultDecision
  -> user confirm
  -> AgentAssetVersion + ActiveDesignSnapshot
```

`ForgeVisualProgram@2` 是资产的生成设计源；展开 DAG、编译工件、截图和视觉报告都必须记录 source hash、compiler version 与输入 lineage。Provider、WebView、Python worker、神经网格和外部 GLB 均不能直接推进 asset head 或 Snapshot。

## 4. 语言最小核心

第一版只实现足够形成安全组合的语言核，不一次性发明完整 CAD：

- 稳定 envelope：`schema_version`、`program_id`、`domain`、`units`、`seed`、`parameters`、`materials`、`nodes`、`outputs`、`budgets`；
- typed parameter：标量、整数、布尔、枚举、长度、角度、比例、颜色；每项有默认值、范围和单位；
- typed node：primitive、transform、profile、extrude、revolve、loft、sweep、boolean、array、mirror、part、material zone、surface binding；
- 纯宏：显式参数与返回类型、词法作用域、无副作用；
- 有界组合：`repeat`/array 必须有静态上限，禁止 `while`、递归和动态代码生成；
- 引用只能指向本程序内 ID 或 CAS 中经过 capability 验证的只读资产；
- source map 从每个最终 Part/Shape/Zone 回到原始 node、macro call 和参数；
- canonical serialization/hash 保证相同输入得到相同展开结果。

首个原子任务 `FGC-VP201` 只交付 envelope、参数/节点最小子集、Rust validator、canonical hash、source map 和 lowering smoke。宏、完整高级几何、神经 seed、UI 编辑器分别后续领取，避免再次形成不可验收的大任务。

## 5. 安全与预算

禁止：任意 Python/JavaScript/shell、反射、动态 import、网络、URL、文件路径、环境变量、插件代码、无界循环、递归、未声明随机源和 Provider 自选预算。

Rust 在执行前静态拒绝：

- 展开节点、primitive、triangle、纹理、材质、Part、boolean、macro depth 超限；
- 引用环、悬空引用、重复 ID、单位不一致、非有限数值、范围越界；
- 需要未授权 capability、外部网络或未知资源；
- source hash、compiler profile、project/turn/revision 不一致。

默认生成采用 `1 + 1` 调用预算：一次 authoring，一次可选 typed patch。schema repair 可在同一次响应的受限解析阶段处理，但不能变成新的创意重写。禁止模型通过反复看图、重写整份程序来弥补语言缺陷。

token 预算、输出语义密度和 wall-clock 分开管理：输入可携带压缩后的参考观察、领域规则和质量合同；输出应优先表达参数、宏、motif、profile/section、装配和材质/表面绑定，而不是随最终 primitive 数线性增长。Provider 可以使用较高推理预算，但网络串行调用仍固定为 `1 + 1`，且每阶段必须记录真实耗时。

## 6. 三种本地执行模式

### Procedural

默认路径。适合机械硬表面、产品概念和明确部件结构；完全 lowering 到 ForgeCAD typed graphs，编辑性最高、成本最低。

### Parametric / Deformable

目标路径。DeepSeek 只能提交受限形变/姿态参数，Rust 验证骨架、曲面、预算、Part/Zone 与 lineage，本地执行器生成几何。它不能成为新的版本头，也不能绕过 readback/confirm。

### Hybrid

自动路由只选择执行能力，不改变产品真值。本地形变曲面提供有机轮廓或复杂基底，程序化节点负责机械结构、分件、材质区、表面层和后续编辑。DeepSeek/千问不可用时，既有项目仍能打开、编辑、导出；首次请求应给出可解释 limitation 而不是无限等待。

兼容 `mesh_seed` 不属于 `FGC-VP201` 且保持 unavailable；本地 deformable/local-hybrid 由 U004 实施。

## 7. 时间、质量与商业门

冻结 30 条未见机械硬表面任务，按难度、单图/多图/纯文本、结构族和风格分层。禁止将测试对象或其整机变体写入 fixture/registry。

目标而非当前能力：

| 指标 | 首个可用目标 |
| --- | --- |
| 无 patch 首次结果 | P50 ≤32 秒，P90 ≤70 秒 |
| 最多一次 patch | P90 ≤105 秒 |
| Provider 创意调用 | 1 次 author + 最多 1 次 typed patch |
| 首次真人外观评分 ≥4/5 | ≥70% 未见任务 |
| 一次 patch 后真人外观评分 ≥4/5 | ≥85% 未见任务 |
| 严重结构/版本/导出回归 | <10% |
| 默认安装 | 不带 CUDA、常驻 GPU 服务或大权重 |
| 可变成本 | 冻结分布内不高于实收收入 25% |

自动门分为：合同/安全、结构完整、PBR/readback、固定视图可见性、时间/成本、结构差异。视觉 Provider 评分只能作辅助；最终 4/5 必须由未参与实现的真人盲评。

## 8. 与 img2threejs 的关系

吸收其本质：让多模态模型把图片理解直接表达为可执行 3D 场景，从而获得比固定类别模板更高的开放性；采用图片准入、macro/meso/micro Detail Inventory、固定相机、参考—渲染比较和有界停止策略。

不复制其产品真值与串行创作成本：任意生成的 JavaScript/Three.js 场景难以提供稳定 schema、版本迁移、部件/材质区编辑、静态预算、确定性回读和安全边界；上游公开成本模型也把一个对象估算为约 `80k–180k` token 和 `5–8` 次 render-review cycle，并明确不是实测保证。ForgeCAD MAX 用 typed Visual Program 取代任意代码，用 Rust compiler 取代浏览器直接执行，用一次完整 author + 最多一次视觉 patch 取代八阶段模型串行写码，用唯一版本链和 strict GLB readback 取代“能渲染即成功”。具体上游版本、许可证和采用证据继续登记在 `AGENT_GITHUB_REFERENCE_ARCHITECTURE.md`。

img2threejs 的 showcase 证明了宿主 coding Agent 直接写 Three.js 的表达上限，不等于已经证明快速、自动、未见分布和产品级编辑。ForgeCAD 的“MAX”升级必须同时保留其开放组合思路，并解决它没有解决的稳定 IR、视觉 patch 证据、资产真值、版本恢复和正式真人基准。

## 9. Fixture 与复用政策

允许复用的是小而正交的 primitive、operator、材质、表面图案、连接规则、编译器 pass 和评测 fixture。禁止用完整对象 Recipe、整机专用 lowering、隐藏模型类别分支或 prompt 命中固定 GLB 伪装高自由度。

任何新增结构族必须证明：同一语言原语能组合出至少三个拓扑/轮廓明显不同的未见结果；移除专用 fixture 后仍能通过；程序 source hash、展开 DAG hash、Shape/Assembly/GLB hash 可追溯。

## 10. 迁移顺序

1. `FGC-VP201`：`ForgeVisualProgram@2` 最小合同、Rust validator/canonical hash/source map、到现有 ShapeProgram 的最小 lowering；
2. `FGC-VP202`：纯宏、有界 repeat、静态预算和 `ExpandedVisualDAG@1`；
3. `FGC-VP203`：高层 profile/extrude/revolve/loft/sweep/boolean/array/mirror 组合与 source-map readback；
4. `FGC-VP204`：一次 author + 最多一次 patch、增量编译、缓存和时间证据；
5. `FGC-E005-R1`：把 VP201 参数/变换、VP202 宏/repeat、VP203 高层几何与 Assembly/Material/Surface/detail motif 合并到同一 formal author source；
6. `FGC-E005-R2`：让唯一第二次调用读取 sealed 参考证据与同源候选四/八视图，返回 `accept | typed_visual_patch`，而不是只修工程 Gate；
7. `FGC-E005-R3`：以同源 `production_review`、真实单图/多视图/当前资产编辑任务和分阶段 wall-clock 完成正式运行前预检；
8. `FGC-E005`：30 条未见任务 success@1、差异性、失败分布和真人基线；
9. `FGC-U004A/U004`：DeepSeek/千问 Provider 主权与本地 deformable/local-hybrid，必须证明净质量收益、退出方案和单位经济性；
10. 通过 E005 后才扩展正式 Domain Pack、跨领域 kit 和发布能力。

实现状态（2026-07-29）：步骤 1–5 已完成。步骤 6 已完成动态 Core、尚未完成 formal integration。Provider 的同一次 visual response 返回 assessments + `E005VisualPatchProposal@1`，Rust 派生 report 后才密封 `E005VisualPatch@1`，避免 Provider 伪造无法预知的 report SHA。accept 复用首个候选；typed patch 只允许一次重建，严格不发第三次 VLM，并标记 `patched_pending_visual_confirmation`。SurfacePlan 未进入 RestrictedGeometry/PBR 前，Surface tuning proposal fail closed。下一原子工作是接入 0045 Patch reservation、formal runner、VP204/session/restart receipt 与真实 sealed image task。

## 11. 后果

正面：把 token 从重复修一台模型转移到一次编写高信息密度设计源；编译器能力可跨对象复用；生成时间和调用数可硬限制；程序化与神经结果共享一条版本、编辑和导出真值。

代价：需要建设真正的语言、编译器、source map、静态预算与未见评测，而不是继续堆视觉 fixture；前几版不会覆盖角色/有机物等全部 3D；高外观质量仍需优质材质、轮廓算子、参考比较和真人证据，不能仅靠 schema 达成。

失败条件：若 VP201–VP204 仍需整机专用 lowering，或 E005 首次/一次 patch 通过率、时间和成本长期未达目标，就收窄为机械硬表面可编辑生成器，不以“MAX”名义继续扩大类别承诺。
