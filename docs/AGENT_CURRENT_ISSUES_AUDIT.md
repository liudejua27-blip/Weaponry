# ForgeCAD 轻量 3D Agent：当前问题审计

版本：2026-07-14
状态：产品与架构决策文档；主文档已完成分层，但状态措辞仍需随 Gate 同步，不代表以下目标均已实现

## 1. 结论

ForgeCAD 不应继续走“本机安装 TripoSR / Stable Fast 3D / Hunyuan3D，再由 Agent 调用”的路线。目标用户是零基础用户，默认安装包只应包含桌面壳、本地业务服务、轻量几何运行时和必要资产；用户只配置一个兼容的大模型 API。

新的核心闭环是：

```text
自然语言
→ Agent 理解意图并提出计划
→ 大模型生成受限 ShapeProgram
→ 本机轻量几何内核构建 GLB
→ 主视图预览与检查
→ 用户确认
→ 创建不可变子版本
→ 导出
```

大模型负责交流、规划、候选评审和“编写模型程序”，不直接返回可信网格；本机负责执行白名单几何操作、验证结果和保存版本。目标外观要接近真实产品的比例、曲面转折、接缝、纹理、材质和光照，但仍是非功能性概念资产，不是工程或制造模型。完整外观、分件清楚、部件可编辑、可撤销和低资源仍是硬门，不能为了表面照片感牺牲状态与可编辑性。

## 2. 已确认的当前基础

- 桌面端已收敛为 `CadWorkbenchPanel` 和单一 Three.js 视口；旧任务中心、Mode、Forge、设置等前端页面已不在当前 feature 目录。
- Tauri、React、FastAPI、SQLite、内容寻址对象、Project/Version、ModuleGraph、ChangeSet、质量检查和导出已有可复用实现。
- 当前 Brief、Variant、Change Planner 与 G4 通用 Mechanical Concept Planner 已有 OpenAI-compatible Provider 边界；通用 Planner 能返回三个完整外观方向，但尚未完成真实 API truth set。
- 当前 `wushen_agent.main` 仍挂载大量 legacy API，旧图像/神经 3D Provider、Unity 和发布检查仍被脚本引用，不能在不迁移启动链的情况下直接删除。
- 当前几何仍主要来自预制 GLB 模块；G3 已有可审计的 `ShapeProgram@1` 合同、validator 和轻量 Geometry Worker，可输出确定性 GLB/AssemblyGraph/topology hash；G801–G807 已覆盖受控 wedge/capsule、profile/extrude、revolve、阵列、有限布尔、bevel_approx、surface_panel 和四领域 48 个结构变体，但复杂曲面、自由 fillet、碰撞/运动学仍未覆盖。
- 当前领域资产仍以 Weapon Concept reference pack 为历史兼容中心；G2 已落地通用 `DomainPackManifest`、`MechanicalConceptSpec`、`AssemblyGraph`、`MaterialPreset` JSON Schema 与 registry，G4 已接入三方向 Planner，G3/G807 worker 已提供四领域各 12 个、共 48 个后端 blockout 变体，G5 已提供分件候选，G6 已提供视觉材质目录、独立 AgentAssetVersion、AgentComponent 注册/替换、声明式 Connector 对齐和 ShapeProgram→GLB→readback 闭环；G6.5 已加入安全外部 GLB 参考导入、内容寻址、同视口显示与原样 readback。前端变体目录、真实碰撞/运动学、外部 GLB 自动重建/深度分件和 packaged sidecar 仍未完成。

## 3. 当前主要问题

### P0：外观真实度不是完整管线

当前结果主要由受限低多边形 primitive、有限展示细节和参数 PBR 构成。多数部件只有一个 Material Zone，纹理对象只覆盖少量通道；稳定 UV/tangent、多材质区、clearcoat、HDRI/色彩管理、视觉基准和 GLB 优化/验证尚未形成闭环。结果因此更像 blockout，而不是比例、材质和细节可信的真实产品外观。

处理：先完成 G819/Q003，随后按 D005 语义比例、M108 多材质 PBR、C105 可编辑组件配方和 R007 参考引导重建推进。DeepSeek 只负责任务规划、评审和工具调用；几何、纹理、环境、readback 和质量必须由受限资产管线证明。

### P0：三方向把 Agent 评审责任交给用户

当前 Planner 合同固定返回三个方向，工作台再要求用户选择并在每个方向的三项外观中轮换。零基础用户既无法从低分辨率预览判断哪个更完整，也不应承担运行时质量、Brief 覆盖和可编辑性的比较。

处理：ADR-0010 取代 `FGC-V002`。目标 `FGC-V003` 在内部生成多个候选，逐个编译/readback/渲染/评审，默认只显示一个最佳结果。用户可继续修改或开启“换一个思路”的新 Turn，不展示三张选择卡。

### P0：DeepSeek 配置和失败不可观察

2026-07-14 本机诊断显示 Agent 服务健康，但 `~/Library/Application Support/ForgeCAD/provider.json` 和 `ForgeCAD Agent Provider/default` Keychain 项均不存在；Rust supervisor 因此没有注入 `FORGECAD_AGENT_PROVIDER=openai_compatible`，当前 Turn 实际使用确定性离线 Planner，日志中没有 Provider check/DeepSeek 请求。`deepseek-v4-pro` 是当前官方有效模型，不是本次根因。

当前 adapter 还拒绝 Tool Calls，把 400/402/422/500/503 压成泛化 HTTP 错误，把空 JSON content 归为统一坏输出；前端进一步只显示“暂时无法连接/测试未完成”，导致用户误以为请求已经发送但没有响应。

处理：新增 A003 Provider Gateway，提供配置/Keychain preflight、连接状态、流式 Item、取消、固定错误分类、空 JSON/Schema 错误、usage 和真实网络调用标记。选择真实 Provider 后失败不得静默回退为离线成功；Codex/Claude 式 Action Loop 另由 A004 在 G819 工具边界内实现。

### P0：工作台信息密度仍过高

`CadWorkbenchPanel` 仍超过 2,000 行并装配 legacy Graph Inspector、旧参数/导出、Agent 主流程、Provider 配置和主视口。中心大视口、方向卡、抽屉和参数同时争夺空间，连续 Agent 步骤反而不是视觉主线。

处理：F025 先隔离 legacy 并继续拆薄父层，V003 移除三方向选择，F026 再实施 Codex 式布局：3D 缩为左上 mini viewport，点击后把同一个 canvas 移到中央 focus；中心默认展示会话、步骤和单一最佳结果，右侧不常驻属性面板。

### P0：产品方向与运行时冲突

旧文档把 SF3D、TripoSR、Hunyuan3D 描述为候选 3D 生成主线。这与“零安装知识、只接 API、轻量桌面软件”冲突，会带来 Python 模型环境、PyTorch、权重、显存、许可证和首次下载问题。

处理：冻结这些路径为 legacy 实验，不进入默认安装、首次启动、设置界面、测试清单或路线图。默认依赖门明确禁止 `torch`、`tensorflow`、CUDA 包和模型权重。

### P0：Agent 没有稳定的 3D 编程中间层

让大模型直接输出 Three.js、Python 或任意脚本存在四个问题：执行权限过大、结果不可预测、难以版本化、零基础用户无法理解失败原因。

处理：新增 `ShapeProgram@1`。它不是通用代码，而是严格 JSON 合同，只允许：

- 基元：box、cylinder、prism、wedge、capsule；
- 变换：translate、rotate、scale、mirror、array；
- 造型：union、subtract、intersect、bevel/chamfer 近似；
- 语义：module、material_zone、connector、editable_parameter；
- 输出：mesh、bounds、triangle budget、warnings。

所有操作都必须有稳定 ID、参数范围和输入引用；未知操作、循环引用、超预算、非有限数值和越权文件访问一律拒绝。

### P0：Agent 运行模型尚未统一

当前 Job/Event、聊天、Planner、ChangeSet 和桌面反馈分别演进，零基础用户会看到“生成方案”“任务”“版本”“检查”等多个并行概念。

处理：统一为四层：

```text
Thread：一个设计会话
Turn：用户的一次请求
Item：消息、计划、工具步骤、预览、确认、结果
Artifact：ShapeProgram、GLB、缩略图、报告、导出文件
```

界面只显示“正在理解 / 正在生成 / 需要确认 / 已完成 / 需要处理”，内部再映射 Job、Step、Event 和 ChangeSet。

### P0 历史问题：活动设计曾有两套状态真值

在 S001–S008 之前，`ConceptVersion/ModuleGraph` 与 `AgentAssetVersion/AssemblyGraph` 曾同时被工作台读取，版本、选择、质量和导出可能指向不同对象。

处理：按 [ActiveDesignSnapshot](AUTHORITATIVE_STATE.md) 建立服务端单一快照，前端只消费同一 revision；legacy Concept 进入只读/显式转换路径。`FGC-S001`–`FGC-S008` 已完成 Schema/Pydantic/TS、SQLite Snapshot、revision CAS、API、desktop client/reducer、Agent 工作台接入、legacy 授权提升、不可变回退/前进与核心并发矩阵 smoke。仍需处理 legacy UI 完全退出、原生安装恢复与更广的并发压力验证，但不能重新增加条件分支修补双状态。

### P0：变更权限需要更严格

Agent 既要“能编程”，又不能默认获得任意 shell、磁盘或网络权限。当前产品用户不是开发者，不能把代码 Agent 的权限模型原样暴露给他们。

处理：

- 默认设计 Agent 只调用 ForgeCAD 领域工具；
- 生成与修改只写入临时候选和 ChangeSet；
- 覆盖父版本、删除项目、导出外部路径、运行开发脚本均需独立权限；
- “开发者模式”以后作为隐藏的独立能力，不进入新手工作台。

### P1：前端仍暴露过多 CAD 术语

当前实现仍有测量模式、连接器、属性、质量和导出等专业控件。它们对验证有价值，但不应全部常驻首屏。

处理：默认只显示对话、主视图、选中对象的三到五个动作和底部确认条。高级属性、连接器、网格检查、格式和精确坐标按需展开。

### P0：产品语义仍绑定单一 Weapon Concept

当前 WeaponConceptSpec、九类模块和 reference pack 可以继续作为兼容 fixture，但不能承载汽车、飞机和机械臂的完整外观、分件与关节语义。

处理：Core 升级为通用机械概念模型；新增 `DomainPackManifest@1`、`MechanicalConceptSpec@1`、`AssemblyGraph@1` 和 `MaterialPreset@1`。武器、汽车、飞机、机械臂成为同级领域包，不创建四个工作台。

当前四领域最小合同与 `recognized | ambiguous | unsupported` 服务已经落地；未知或含糊输入在 Planner 前停止，只创建一个 waiting-for-clarification Turn/Item，不创建 Plan、Blockout、版本或资产。D003 已将该阻断变成普通语言 clarification Item 与四选项 UI，并保留原始 Brief 继续新 Turn。

### P1：组件库与程序化生成尚未形成统一真值

预制模块适合稳定组合，ShapeProgram 适合产生更多轮廓。若两者各自修改网格，会形成双真值。

处理：目标 `AssemblyGraph` 是装配真值；每个节点的几何来源只能是 `registered_asset` 或 `shape_program`。生成后都固化为不可变 GLB，并记录来源 hash。后续修改回到对应来源生成新工件，不直接编辑已固化 GLB。当前 ModuleGraph 通过兼容 adapter 迁移。

### 主结构已解决，状态措辞仍需持续同步

2026-07-13 已完成主文档重构：当前 Agent API、用户指南、开发、资产、发布、恢复和 legacy 已分离，并建立 Codex 执行计划、原子任务和完成定义。仍需在每次 Gate 重跑后同步 PASS/KNOWN FAIL/NOT RUN，不能仅凭历史 evidence 宣布当前桌面工作台通过。

当前维护规则：

- 当前能力以 README、USER_GUIDE 和 API 为准；
- 目标能力进入 DESIGN、IMPLEMENTATION_PLAN 和 CODEX_EXECUTION_PLAN；
- 主文档只通过 `docs/legacy/` 导航进入 legacy 资料；
- `release:docs-walkthrough` 拒绝用户指南过度承诺和 legacy 命令泄漏；
- 运行时代码和 release gate 完成迁移前，仍被脚本引用的 legacy 文件保持明确标记，不提前删除。

## 4. 轻量资源基线

以下是设计目标，不是当前测量结果：

| 项目 | P0 目标 |
| --- | --- |
| 本地神经模型 | 0 |
| 模型权重下载 | 0 |
| GPU 推理依赖 | 0 |
| WebGL canvas | 1 |
| 预览三角预算 | 默认不超过 100k，可配置硬上限 |
| 单次几何构建 | 后台 worker/process，可取消、可超时 |
| 默认输出 | GLB + AssemblyGraph + ShapeProgram + Material bindings + Manifest |
| API 上下文 | 发送摘要、选择和必要合同，不上传整个资产库 |

大模型 Provider 的 token 上限不应直接等于每次请求预算。应用需要自己的输入、输出、时间和费用上限，并对超长会话做摘要。

## 5. 本轮删除边界

本轮允许删除：

- 无任何仓库引用的旧 M1–M5 里程碑说明；
- 无任何调用点的 `wushen_agent/job_store.py`；
- 会让新路线误以为需要本地神经模型的非权威说明；
- 已被 FRONTEND/USER_GUIDE/ASSET_AUTHORING 合并的重复工作台和 Blender Starter 文档；
- 已否决的本地神经 3D、Unity 专用操作和旧 Weapon/TripoSR 设计 QA 文档。

本轮不删除：

- `wushen_agent.main` 和当前被启动脚本使用的 legacy 服务；
- 当前 Agent API、Supervisor、数据/恢复、发布和安全边界文档；
- 当前测试、迁移和回归 fixture。

本轮先迁移文档门和 safety gate，再删除 Local Runtime、Unity、Blender Starter、旧工作台和 root design QA 文档。legacy 运行时代码仍必须在后续“切换启动入口 → 迁移测试 → 修改发布门 → 删除 legacy”四步完成后再删，否则会把可运行仓库变成文档看似干净、实际无法启动的仓库。

## 6. 完成标准

轻量 Agent P0 只有同时满足以下条件才算完成：

- 新机器无需安装模型、CUDA、ComfyUI 或外部 3D 服务；
- 只配置一个大模型 API 即可交流、生成、修改和导出低多边形 3D 概念道具；
- 四个首批领域各至少 20 条固定 Brief 可生成完整、结构不同且可继续编辑的结果；
- 汽车、飞机、机械臂和未来武器道具均先生成完整外观，再完成可预览的自动分件；
- 每次修改都有预览、解释、确认、撤销和子版本；
- Provider 不可用时已有项目仍可打开、手动调整和导出；
- 非法 ShapeProgram 不会执行任意代码或读写工作区外文件；
- 一次会话只维护一个 Three.js renderer；
- 所有“通过”“批准”“原创”状态来自真实数据。
