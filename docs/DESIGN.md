# Forge Studio 通用参考条件 3D Agent 系统设计

版本：v8（2026-07-29）
状态：目标架构；当前实现与迁移边界见第 3 节

## 1. 产品定义

目标面向用户产品名为 **Forge Studio**：非专业创作者和小型内容团队用自然语言与授权图片生成精致、可编辑、可回读的通用 PBR 3D 资产，并继续用自然语言修改和导出。上传/描述什么对象，系统就保持该对象身份进行生成，不以 Domain Pack 或机械硬表面白名单限制类别。仓库和内部兼容标识当前仍使用 ForgeCAD。ADR-0022 定义类别开放产品与能力沙箱；ADR-0023 固定 DeepSeek/千问唯一 AI Provider 边界；ADR-0021 提供高自由度程序化表示和一次 author + 最多一次 typed patch；ADR-0020 的小团队成本边界继续有效。

## 通用视觉理解与混合表示架构（目标）

```text
Prompt + sealed image(s) / current asset
  -> Qwen visual understanding / comparison
  -> SubjectProfile@1
       identity / parts / silhouette / pose / materials
       macro / meso / micro detail / occlusion / uncertainty
  -> VisualFeatureContract@1
       evidence binding / salience / required views / acceptance
  -> RepresentationPlan@1
       per-part procedural | deformable | local_hybrid
  -> UniversalAssetSource@1
  -> Rust capability / schema / budget / lineage validation
  -> DeepSeek typed ForgeVisualProgram authoring
  -> Restricted local procedural / deformable / material compilers
  -> one GLB/PBR + strict readback + fixed multiview
  -> visual comparison + at most one typed patch
  -> confirm -> AgentAssetVersion / ActiveDesignSnapshot
```

`SubjectProfile` 替代 domain allowlist：对象类别只帮助理解，不决定准入。`RepresentationPlan` 逐部件选择已声明 capability；若缺少可执行表示，返回 `representation_unavailable` 或 `quality_limited`，不得静默使用 C111、机械臂或其他整机模板。`UniversalAssetSource` 是统一 envelope；`ForgeVisualProgram@2`、本地形变和本地混合表示是其中的表示分支，不创建彼此独立的版本头。

U002/U003/U004 当前实现边界：`UniversalAuthorRequest@1` 由 Rust 从真实 Turn/Project/Snapshot、sealed evidence、选择/锁定和代码所有 capability manifest 构造；Provider 一次返回 profile/feature/plan/outcome，新项目首轮只可调用 `author_universal_asset`。可执行 capability 仅限经过审查的 `procedural.robotic_arm_visual_v1`、`procedural.generic_hard_surface_v1`、`deformable.local_lattice_shell_v1`，以及后两者的逐部件本地硬表面 Hybrid；lattice 只允许既有硬表面程序化源的固定 `2×2×2` cage 外壳形变，Hybrid 要求 Rust 将每个 deformable Part 与 sealed plan、terminal lattice operation、offset、Part binding 精确重算，未声明部件继续保持程序化终端。mesh seed、任意 mesh、角色和生物表示仍明确 unavailable。limitation 是正常、零几何副作用结果。可执行结果的 `UniversalAssetSource@2` 只能由 Rust 从已验证合同和程序 revision 派生，编译后精确绑定 ShapeProgram、GLB、语义/编译 readback、renderer 和固定视图；无拟合证据的相机保持 unresolved。

U004A 已移除旧远程 Mesh Seed 实验路径：主程序不再注册其 Tauri 命令，不再保存或读取对应密钥，不再拥有网络 adapter、恢复任务、前端候选入口或 live Gate。兼容数据合同和数据库迁移仅用于读取历史库，不允许恢复网络请求。`mesh_seed.generic_v1` 在 capability registry 中继续 unavailable。

U004 当前只建设自主可控的本地表示：千问从 sealed 图片提取 `observed | inferred | unknown` 视觉证据，Rust 封存合同，DeepSeek 编写受限 `ForgeVisualProgram@2`，本地编译器负责 procedural/deformable/local-hybrid 几何与 Appearance。当前 P0 在 GLB/readback 后将 Turn 置为 `waiting_for_capture`，同一工作台 renderer 的八视图 evidence 被 Rust 一次性采用后才恢复 evaluate；失败时只允许一次 DeepSeek typed patch，patch 后必须以新 GLB 重新 capture，第二次失败终止。该内存 continuation 不跨重启恢复。通过此链的非游戏 `UniversalAssetSource@2` preview 使用独立 `NativeUniversalPreviewProvenance@1`，Rust 在确认前重验 request/source hash、Project/Turn、representation direction 与精确 GLB，随后沿现有单 GLB 原子版本事务持久化该 UAS provenance；它不创建旧 V003 decision，也不使用 Domain Pack/C111 填充身份。每个新 capability 仍必须具有 Part/Zone、严格 readback、固定输出多视图比较、confirm/version/Snapshot/export 和跨类别净质量证据后才能改为 available。

游戏资产交付不是新的网格来源：当 Rust-sealed `GameAssetProfile@1` 已在 UAS@2 编译前确定，受限 worker 先产出可视觉验收的 source LOD0 GLB，Rust 再以同一 Part terminal binding 派生 `MSFT_lod`、off-scene collision、socket 和实测 texel-density 的 delivery GLB。source GLB 始终是视觉比较及 readback 真值；UAS 只保存 hash-bound delivery receipt。游戏确认通过专用双工件原子事务同时保存 source LOD0、delivery GLB 和 interactive preview，`production_glb` 继续指向 source 以保证质量与编辑真值不变，`game_delivery_glb` 才是用户交付物。`:model.glb`、`:export` 与 `ForgeAssetPackage@2` 必须重新校验 source/profile/binding/receipt 后仅消耗对应 delivery bytes，并同时披露 source hash；缺少 profile、readback、receipt 或任一 object role 时 fail-closed。用户 profile 选择、真实工作台 valid GLB 的 preview→confirm→export/package E2E、篡改/幂等和 packaged 验收仍是后续 Gate。

## ForgeVisualProgram@2 高自由度编译架构（目标）

`ForgeVisualProgram@2` 是可编辑生成设计源，不是脚本容器。它用 typed parameter、node、Part、Material Zone、Surface binding 和静态预算表达对象；Rust 将其验证并确定性展开为 `ExpandedVisualDAG@1`，再 lowering 到现有 ShapeProgram/AssemblyGraph/Material/Surface 真值。展开 DAG 是按 source hash/compiler version 可重建的缓存，不创建第二版本链。

```text
Prompt + sealed ReferenceEvidence
  -> ForgeVisualAuthoringIntent
  -> ForgeVisualProgram@2
  -> validate + canonicalize + static budget
  -> ExpandedVisualDAG@1
  -> ShapeProgram / AssemblyGraph / MaterialGraph / SurfaceGraph
  -> RestrictedGeometryExecutor
  -> GLB/PBR + strict readback + source map
  -> SingleResultDecision -> confirm -> AgentAssetVersion/Snapshot
```

第一原子切片 VP201 只实现 v2 envelope、参数/节点最小子集、canonical hash、source map 和最小 lowering；纯宏/有界 repeat 属于 VP202，高层 profile/extrude/revolve/loft/sweep/boolean/array/mirror 属于 VP203，低往返和增量编译属于 VP204。当前用户能力仍以 USER_GUIDE 为准，不得从本目标章节推导“高自由度已实现”。

语言运行时禁止任意 Python/JavaScript/shell、反射、动态 import、URL/path/网络、环境变量、递归、while 和无界展开。Rust 必须在 worker/Provider 前拒绝重复/悬空引用、环、非有限值、单位/范围错误、预算和 capability 越权。最终 Part/Shape/Zone readback 必须通过 source map 回指 v2 node、macro call 和参数。

执行模式共享同一真值：Procedural 处理规则结构；Parametric/Deformable 处理角色、生物和软体比例/姿态；Local Hybrid 将形变曲面、程序化结构、材质和可编辑细节组合为同一源。兼容 `mesh_seed` 计划不可执行。DeepSeek、千问或 WebView 均不能直接推进版本头、Snapshot 或导出真值。

当前 ForgeCAD Alpha 用户通过唯一工作台完成：

```text
创意交流 → 单次完整合成 → 硬门与有界原位修复 → 单一结果 → 3D blockout → 自动分件
→ 部件级细化 → 材质 → 检查 → 渲染与导出
```

首批四个机械 Domain Pack 继续作为 Alpha 兼容知识与回归基线，不再是输入类别清单。目标入口同时接收机械/产品、角色/人形、动物/生物、植物/自然物、家具/生活物、建筑/环境、载具和混合对象；质量按表示能力和跨类别未见基准报告，不通过登记 Domain Pack 决定能否生成。

### 安全与交付边界

武器题材结果属于虚构游戏美术资产。项目不生成现实可制造武器，不提供制造尺寸、材料配方、加工流程、功能机构或性能结论。

汽车、飞机和机械臂同样是概念数字资产；P0 不输出道路安全、碰撞、空气动力学、飞行安全、载荷、扭矩、控制安全或认证结论。

## 2. 产品原则

### 2.1 完整外观优先

新建设计先生成整体轮廓、比例和姿态，再分件。首个结果不能只是车轮、机翼、关节或武器附件等局部部件。

### 2.2 Agent 优先，CAD 渐进展开

首屏只有对话、步骤、单一主视图、选中对象动作和确认条。技术合同、坐标、Connector、Joint、网格和文件格式默认隐藏。

### 2.3 代码即模型，但不是任意代码

大模型编写受控 `ShapeProgram@1`，不执行任意 Python、JavaScript 或 shell。GUI 和 Agent 修改同一权威表示。

### 2.4 一个核心，通用对象合同承载语义

Core 只认识 Project、Assembly、Part、Shape、Material、Joint、Version 和 Tool。对象身份、部件、轮廓、姿态、材质和不确定性进入 `SubjectProfile@1`；执行选择进入 `RepresentationPlan@1`。Domain Pack 只提供可选领域知识和限制，不能成为准入 allowlist。

### 2.5 预览先于永久修改

Agent 只创建候选。用户确认后才创建不可变子版本；父版本、原始资产和锁定部件不可覆盖。

### 2.6 本地真值、轻量运行

SQLite 与内容寻址对象是本地真值。默认安装不含神经 3D 模型、权重和 GPU 推理环境。P0 只维护一个 Agent Orchestrator 和一个 WebGL renderer。

### 2.7 Agent 负责一次合成与硬门，用户负责目标和确认

Planner 对每个 Turn 只提出一份完整受限模型。该模型经过编译、GLB readback、概念渲染和规则硬门后才显示；若只存在可局部修复的失败，最多一次 typed patch 必须保留同一 Brief、SubjectProfile、核心轮廓意图和 parent attempt provenance。用户可以继续描述修改或明确要求换一个思路，不需要先在三张方向卡中筛选；“换一个思路”是新 Turn。完整模型的第二方向、评分比较和淘汰记录不属于此路线；只有用户确认后才进入不可变版本链。

### 2.8 视觉真实度是完整资产管线

千问负责图片理解和候选视觉比较，DeepSeek 负责设计程序、计划与工具编排；两者都不直接拥有可信网格或真实纹理。接近真实产品的外观必须来自语义比例、可编辑组件、曲面/边缘细节、稳定材质分区、UV/切线、PBR 纹理、环境光、色彩管理、真实 readback 和视觉基准的共同作用。任何单独更换模型、扩大 prompt 或增加方向数量都不能代替这条管线。

### 2.9 采用 3D 机械设计系统，而不是 HTML 六面或单一 box 雕刻

ForgeCAD 借鉴 UI 组件库的 Token、Recipe、Props、版本和预览思想，但不把 DOM 直接变成模型：

```text
MechanicalStyleToken + EditableComponentRecipe + bounded parameters
→ ProfileSketch / ProfileSectionSet
→ ShapeProgram feature nodes
→ GeometryCompileReadback
→ GLB
```

HTML/React 负责工作台，SVG 负责受限轮廓控制点，GSAP 负责界面与相机过渡；它们都不是几何、版本、质量或导出真值。主壳体根据结构选择 Extrude、Loft、Revolve 或 Sweep，CSG 只用于局部开孔、裁剪和组合。组件 Recipe 组合经过验证的轮廓、特征、连接、材质区和可编辑参数，使汽车、飞机、家电、机械臂与其他机械产品不再从同一组 primitive 临时拼装。

### 2.10 小团队单位经济优先

默认能力必须在没有本地大权重、CUDA、ComfyUI、自建 GPU 集群和第三方远程 Mesh API 时成立。本地范围、Schema、预算、缓存和编译硬门优先；联网 AI 只允许 DeepSeek 与千问。每个联网 Turn 有授权、请求/用量/时间/成本上限和停止条件，同一意图最多一次 typed patch。Luna 只可作为仓库开发执行者，不能成为运行时 Provider 或状态真值。

## 3. 当前状态与迁移边界

### 3.1 当前可复用

- Tauri + React + TypeScript 桌面壳；
- Rust `forgecad-app-server`/`forgecad-core` 单写 SQLite/WAL、CAS、对象库和产品状态；Python FastAPI 只承载 capability-gated `RestrictedGeometryExecutor`；
- SQLite、WAL、迁移和内容寻址对象；
- Project、Version、`WeaponConceptSpec@1`、`ModuleGraph@1`、ChangeSet；
- 单一 Three.js 视口、选择、变换、替换、镜像和爆炸；
- 武器 reference pack 的质量、渲染和导出证据；
- OpenAI-compatible 结构化 Planner 边界；
- Job、Step、Event、SSE、取消、恢复和幂等基础。

### 3.2 已实现（有当前证据）

- `FGC-K001` 已实现 Rust `forgecad-app-server-protocol`、最小 `forgecad-app-server`、Tauri bridge 与 TypeScript transport。桌面命令/事件统一经过 `forgecad.app-server/1` 的 initialize、JSON-RPC 2.0、稳定 ID、取消、有界队列、通知确认和 cursor replay 合同；浏览器开发壳只通过同合同的 loopback compatibility adapter。`compat/http` 不是通用代理：每个请求必须命中代码所有的 HTTP 方法 + segment-shape 白名单，任意 URL、递归 `/api/v1/app-server`、敏感 header、shell、文件系统和动态 Tool 均被拒绝；
- `ForgeCADReadOnlyResourceCompatibility@1` 的 `forgecad-resource` 只为 packaged WebView 传递图片、GLB、媒体和用户触发下载，固定 GET-only、无状态、无持久写权限，并复用相同 path/header 白名单。它不是第二个 API 或状态源；
- Thread/Turn/Item/Approval Agent Kernel 的 G1 slice、G4 通用 Mechanical Concept Planner port 已实现；真实 API truth set、持久化运行恢复仍未完成；R002/R003 四视图与条件式透明爆炸概念 PNG、以及 R004 当前 fingerprint 约束的 PNG/manifest 图包已实现为只读派生结果；
- `DomainPackManifest@1`；
- `DomainInferenceResult@1`、四领域关键词/同义词 fixture、三态推断服务和 D003 单问题澄清 UI 已完成；`ConceptScopeDecision@1` 随后在 Planner/Provider 前执行有限、可解释的产品范围预检：含糊类别仅写 D003 clarification，明确的现实制造/工程安全/控制请求只写 scope-stop Turn/Item，均不创建 Plan、blockout、资产或 Snapshot；这不是完整内容安全系统；
- `MechanicalConceptSpec@1`；G815 已将其中的 `VisualIntentMapping@1` 用于有限的轮廓、细节、色彩和展示姿态分类，再选择既有视觉族。R006 随后把同一未保存方向的受限 GLB 交给既有软件栅格器，生成一次性的 `AgentBlockoutConceptPreview@1` iso PNG；它不创建候选、版本、Snapshot、质量或导出，也不增加 WebGL context。两者都只影响预审概念外观，不生成尺寸、自由 ShapeProgram、工程材料或性能结论；映射缺失/损坏时回退到既有方向轮廓选择；
- `AssemblyGraph@1` 与概念级 Joint；
- `ShapeProgram@1` 及只执行受限 `box`/`cylinder`/`capsule`/`wedge`/`profile`/`extrude`/`revolve`/`mirror`/`array`/`radial_array`/`union`/`subtract`/`bevel_approx`/`surface_panel` 的轻量几何 worker；G5 已能输出按领域角色组织的分件候选，G6 已提供视觉材质目录、AgentAssetVersion、AgentComponent 注册/替换、声明式 Connector 对齐和受限 GLB 导出；G7 已提供安全外部 GLB 参考导入与同视口显示；G807 已提供四领域各 12 个确定性结构变体，G816 让既有形体完整进入同一展示视口，G818 让展示档同源追加受限视觉细节和有限 PBR 索引。复杂曲面、自由 fillet、碰撞/运动学、前端变体目录与外部 GLB 自动重建/深度分件仍未完成；
- macOS Tauri 工作台已提供轻量 Provider 配置入口；Rust desktop 用代际绑定的 `0700/0600` 私密文件拥有本机 Alpha 凭据，普通启动不读 secret，Provider Key 不进入项目、Agent 数据、日志或 Git；真实 Provider 评测仍保持显式、可计费的单独门禁；
- `MaterialPreset@1` 与 Material Zone；
- 汽车、飞机、机械臂领域包；
- Agent 的完整外观 blockout → 分件候选 → editable asset → 受限编辑 → GLB 导出最小闭环。

### 3.3 部分实现 / 目标能力

路线覆盖：本节保留的 C111B/E004/PV006/M108B 表述只描述机械程序化回归和历史成熟度，不再拥有当前产品优先级。当前实施顺序与通用质量门以 U002–U005 为准。

- 真实 Provider truth set、持久化运行恢复和正式审阅纹理资产仍未完成；M108A 已建立 preview/production 双档五通道 PBR、同源 GLB/视口/readback、质量/导出和派生缓存自动门，但 M108B 的 Recipe 驱动独立人工视觉基准尚未完成；R002–R004 的软件概念渲染与 PNG/manifest 图包已完成，但不等于工程渲染、装配或制造说明；
- 复杂 ShapeProgram 操作、精确碰撞/运动学、外部 GLB 自动重建/深度分件仍未完成；
- 自由拆分/合并、任意版本历史浏览和多格式 Agent 导出仍未完成；C103 已实现由现有 AssemblyGraph、role、受限 ShapeProgram primitive output 和连接事实驱动的拆分/合并候选，但不推断切割线、工程连接或功能；C104 已将部件锁定、隐藏和单独查看写入 Agent Snapshot：锁定阻止相关 ChangeSet，显示状态只控制同一个视口而不创建几何版本，不是工程装配约束；
- 完整外观到高多样性 editable asset 的闭环仍是目标，不得写入用户指南为已支持。
- 运行时操作白名单、实际 GLB 编译/回读质量、ProfileSketch、增强 Extrude/Revolve、受限 Loft/Sweep、单一 Manifold Python CSG，以及几何侧 edge finish/UV0/tangent/稳定 face→part/zone 事实已由 G819/Q003/G820–G826 实现。A003/A004 已实现 DeepSeek Provider 可观察性和受限单 Turn Product Tool Action Loop；F025/D005 已完成 Agent-first/legacy 隔离及四领域语义比例配方。M108A 在 G826 表面事实上接入 `interactive_preview` 与 `production_concept` 两个代码所有的派生 profile、多区五通道 PBR、固定展示环境、`GeometryCompileReadback@2`、二进制传输、production 质量/导出和 CAS。K001–K003 已建立 Rust-owned 协议、Agent 生命周期、DeepSeek、Product Tool 和 core 持久化所有权；C105/A005 也已完成受限 Recipe 与表面细节闭环。ADR-0016 把这些能力统一为 ForgeCAD Design Surface Compiler：设计层用 Style Token、Profile/Section、Surface Zone、Surface Adornment、`SurfaceLayerProgram@1` 和 Recipe 表达，Rust 降低并密封为 ShapeProgram/AssemblyGraph/SurfaceLayerLowering，Python 只编译已验证几何与五通道 PBR。R007A/R007B、V003、C106 与 C107 已完成机械臂工程黄金路径；当前第一视觉主门是 C111B→E004/PV006，M108B 后移为 PV007 后的跨领域正式基准，M109 继续等待它。任何 production profile 或 C107 Gate 通过都不等于模型视觉已达到目标图或独立真人生产级门。
- ADR-0014 的 Rust-first 核心所有权已经由 K001–K003 完成：Rust app-server/core 单一拥有 Thread/Turn/Item/Approval、Context、DeepSeek、Product Tool，以及 Project/Version/ActiveDesignSnapshot/ChangeSet/Quality/Export、SQLite/WAL、CAS 和对象库。Rust/Tauri 仍启动 Python FastAPI sidecar，但该进程只作为无数据库、无对象库、无密钥、无 Snapshot 写权限的 `RestrictedGeometryExecutor`；这不把几何编译重写成 Rust，也不削弱单写和受限几何边界。

`ActiveDesignSnapshot` 的合同、存储、CAS、API、desktop reducer、Agent 恢复/选择/视口/质量/GLB 导出，以及 legacy 只读重建授权和不可变回退/前进已完成。D003 已提供未知/含糊领域的一问式澄清 UI，F001 已通过本机 Chrome 行为基线。当前仍不能把整个工作台称为生产级运行时：legacy 兼容 UI、原生安装恢复、广泛多客户端压力验证、真实 Provider 评测和打包发布仍待后续。

### 3.4 Legacy

当前 `wushen_agent.main`、旧图像/神经 3D、Patch、Unity 和 Weapon API 仍被启动脚本或回归门引用。它们不是目标产品权威源，但必须按“新入口 → 测试迁移 → 发布门迁移 → 删除 legacy”的顺序退出。

历史 `WeaponConceptSpec@1` 和 `ModuleGraph@1` 不直接改名。新合同完成后通过 `WeaponConceptCompatibilityAdapter` 显式转换，并保存原始 schema、ID 和 hash。

## 4. 系统结构

```text
Tauri Desktop
├── LeftRail
│   ├── Project / Conversation History
│   └── Component Library
├── AgentPanel
│   ├── Conversation
│   ├── Step Items
│   ├── Result Summary
│   ├── Approval Cards
│   └── Bottom Composer + Style/Material/Reference Menu
├── ThreeViewport（唯一 WebGL canvas；右侧 docked / 中央 focus 二态）
├── SelectionCard
├── ConfirmationBar
├── ComponentDrawer
├── MaterialDrawer
└── Inspect / Export Drawer
          │ K001 Tauri bridge / bounded JSON-RPC（已实现）
          ▼
Rust ForgeCAD app-server
├── initialize / capability negotiation / cancel / cursor replay（K001 已实现）
├── strict method + segment allowlist compatibility transport（K001 已实现）
├── GET-only stateless forgecad-resource transport（K001 已实现）
├── Thread / Turn / Item / Approval policy（K002 已实现）
├── Context Builder
├── Provider Gateway
│   ├── DeepSeek Adapter / preflight / stream / cancel
│   ├── Qwen Vision Evidence Adapter / compare / cancel
│   ├── DeepSeek/Qwen official endpoint + model-family allowlist
│   └── error taxonomy / usage / redacted trace
├── Agent Orchestrator
│   ├── Visual Brief / typed ForgeVisualProgram authoring（PV003 已实现最小合同）
│   ├── ForgeVisualProgram validator + lowering（PV001/PV008 已实现当前机械臂链）
│   ├── Programmatic fixture/compiler/readback（PV002 已实现当前机械臂链）
│   ├── Build ledger / bounded repair / eight views（PV004 已实现当前机械臂链）
│   └── Single-result decision
├── Product Skill Registry
├── Tool Registry + Runtime Policy
├── Domain Pack Registry
├── Approval / Cancel / Resume
├── Project / Version / ActiveDesignSnapshot / ChangeSet（K003 已实现）
├── SQLite + Content-addressed Objects（K003 已实现）
│   ├── Project / Version / AssemblyGraph
│   ├── Thread / Turn / Item
│   ├── ShapeProgram / GLB / thumbnail
│   ├── MaterialPreset / texture
│   └── ChangeSet / QualityReport / Export
├── Programmatic Visual Coordinator（部分实现；PV002–PV005/PV008 已有机械臂证据，PV006C blocked）
│   ├── ForgeVisualProgram / Detail Inventory / semantic hash
│   ├── Shape + Assembly + Material + Surface lowering
│   ├── GLB/PBR readback + fixed eight views
│   └── one programmatic preview → confirm
└── RestrictedGeometryExecutor Port
          │ versioned request; no DB path, Provider Key or Snapshot write
          ▼
Python Restricted Geometry Executor（当前受限几何边界）
├── 已由 Rust 展开的 ShapeProgram/Profile geometry IR validator
├── primitive / Extrude / Revolve / Loft / Sweep
├── Manifold CSG + bounded edge finish
├── surface provenance / UV0 / tangent / Material Zone
├── PBR GLB compile / readback
└── deterministic hash / structured failure
```

上图同时标出当前所有权和后续产品能力目标。K003 后 Rust app-server/core 已拥有协议连接、Thread/Turn/Item/Approval policy、Context、DeepSeek、Product Tool、预算/取消/usage/trace，以及 Project、版本、Snapshot、ChangeSet、质量、导出、SQLite/WAL、CAS 和对象库；Python 已缩减为纯受限几何执行器。

## 5. Agent 运行模型

### 5.1 Thread、Turn 与 Item

- Thread：一个项目内的设计会话；
- Turn：用户的一次请求；
- Item：消息、计划、工具、结果、预览、批准和工件。

Turn 状态：

```text
queued → running → waiting_for_approval → completed
                  ↘ failed / cancelled
```

Item 追加写入；K002 已让 Rust app-server 单一决定 Item 的创建、排序、终态、取消和 replay，K003 再让 Rust core 单写 SQLite 投影。Python 不再运行 Agent 生命周期、Provider/Tool 编排或产品状态持久化；前端也不从自然语言猜测 Tool 状态。

当前面向用户的 Item 只显示“正在理解、正在检索参考、正在生成模型、正在检查、正在修复、需要确认、已完成或失败”。F026 移除三方向 UI；在 V003 完成前，临时兼容适配器只能读取 legacy Planner 的第一条文本方向并编译/展示一个结果，且不得显示为 `SingleResultDecision@1` 或硬门通过。模型原始隐藏推理不进入数据库、日志或 UI；系统只保存短的 `ReasoningSummary@1`、调用过的产品工具、输入/输出摘要、耗时、用量和固定错误类别。

### 5.2 ActiveDesignSnapshot

Project、活动 AgentAssetVersion、Selection、Preview ChangeSet、Quality 和 Export 必须由同一个服务端 Snapshot 绑定。前端不得分别从旧 Concept hook、Agent asset state 和 localStorage 推断“当前版本”。

Snapshot 的字段、所有权、并发和 legacy 只读规则见 [唯一权威状态设计](AUTHORITATIVE_STATE.md)。合同、Agent-first 接入、任务级 CAS、不可变回退/前进和核心重启路径已实现；生产级广泛并发、legacy UI 完全退出、原生安装恢复和发布仍是后续阻断。

### 5.3 工具权限

自动允许：读取当前项目/选择、检索兼容组件和材质、创建临时候选、运行快速检查。

必须确认：提交 ShapeProgram、确认 ChangeSet、自动分件覆盖候选、批量替换、导出外部目录和删除项目。

默认禁止：任意 shell、任意代码、工作区外读写、隐式联网和覆盖父版本。

### 5.4 上下文

Provider 只收到任务所需摘要：领域包、当前阶段、完整外观约束、选择路径、可用参数、允许 ID、材料预设和工具 Schema。不得发送整个 Library、绝对路径、密钥或无关历史。

Agent 多轮对话使用 `ForgeCADProviderConversation@1`。DeepSeek 是无状态接口，服务端必须保留先前已发送的用户/助手消息并在新请求末尾追加当前 Snapshot 摘要与新请求；固定安全合同、领域包和 JSON Schema 位于消息前缀，动态 Brief、项目 ID、选择、Snapshot revision、时间戳和 UUID 不得破坏该前缀。当前只保留最近四组消息；超过 12 组可由确定性 `ThreadMemorySummary@1` 压缩旧消息。摘要是可删除的会话辅助记录，不是 `ActiveDesignSnapshot`、资产版本或质量真值。

DeepSeek Provider 只接受受 Schema 验证的概念计划或受限产品 Tool Call。A004 已实现 `AgentActionLoop@1`：不可动态扩展的 13 项 ForgeCAD Product Tool Registry 只允许领域/参考/Style/Profile/ShapeProgram/候选 build/readback/render/evaluate/preview，限制 12 次调用、总 token、时间、费用和单次并发；永久修改继续留在既有审批/ChangeSet 路径。DeepSeek 思考模式返回 Tool Call 时，后续同一轮子请求按官方合同回传对应 `reasoning_content`；该字段只在短生命周期 Provider 执行上下文中存在，不进入用户可见思考、资产、Snapshot、Item 或长期日志。复杂概念规划使用思考模式；本地范围停止、领域澄清和确定性操作不消耗 Provider token。调用前后记录脱敏用量、缓存命中和预算结算，Provider HTTP 永远在 SQLite 事务之外执行。

### 5.5 DeepSeek / 千问固定 Provider Gateway

当前产品代码只保留 DeepSeek 文本 adapter 与千问视觉 adapter。模型 ID、价格和能力仍会漂移，每次真实调用前必须依据当时官方资料和本机 capability preflight 验证；但供应商边界不是自由配置：DeepSeek 只接受官方 `api.deepseek.com + deepseek-*`，千问只接受官方 `aliyuncs.com + qwen*`。A003 已实现 Gateway 的配置/运行诊断：本机无 metadata/私密凭据时稳定报告 `unconfigured + network_call_made=false`，不再把离线结果包装成真实 Provider 结果。第三个 Provider 必须由新 ADR 和代码 Gate 显式批准，不能通过更改 Base URL 绕过。

`ProviderConnectionState@1`：

```text
unconfigured → checking → ready
                    ↘ failed(auth | balance | invalid_request | rate_limited
                              | server_unavailable | timeout | invalid_output)
```

A003 当前实现：

- 保存后先验证官方 endpoint、模型家族、metadata 和私密凭据代际可读，再确认 Rust runtime capability；
- `provider:check` 和普通 Turn 产生可取消的脱敏 lifecycle；工作台轮询正在运行 Turn 以展示 Item 和取消 ID，原始 token delta 不进入 UI；
- 固定映射 DeepSeek 400、401、402、422、429、500、503 和网络/超时；不向 UI 泄露 Key、原始响应或内部 Base URL；
- JSON 模式 prompt 明确包含 JSON 要求和输出示例；空 `content`、缺少 choice、无效 JSON、Schema 不符分别记录为结构化错误，不能回退为离线成功；
- 普通 Turn 使用 streaming 反馈和明确取消；Provider 失败后保留已有设计不变，由用户显式重试；
- 稳定系统合同、输出 Schema、版本化 JSON 示例和 Domain Pack 前缀保持 canonical，以利用 DeepSeek 上下文缓存；动态 Snapshot、Brief 和请求 ID 放在后部。Tool/Skill 前缀仍属于 A004/A005。

### 5.6 单次合成、硬门与唯一结果

目标 Orchestrator 在一个 Turn 内执行：

```text
Planner 解析 Brief、Domain Pack / Style Token / Recipe（不生成多个完整候选）
→ 选择建模语法
→ 单次完整合成通过 G819 manifest 编译
→ Q003 GLB readback + R006 概念渲染
→ GenerationGateReport@1
→ 最多一次同意图 typed patch（仅在可局部修复时）
→ SingleResultDecision@1
→ 只向用户展示通过硬门的唯一结果
```

硬门先于展示：范围、Schema、运行时、预算、GLB readback、完整外观和安全任一失败即拒绝。gate profile 只记录该一次合成的通过/失败及可修复字段，不对多个完整模型评分或排序，也不是工程质量或审美真理。若该尝试与其允许的原位修复都未通过，Turn 明确失败并给出可操作原因，不能展示“最不坏”的无效模型。

### 5.7 产品 Skill

ForgeCAD Skill 是声明式产品能力包，不是开发插件。每个 Skill 至少包含：

```text
skills/<skill_id>/<version>/
├── SKILL.md
├── skill-manifest.json
├── tool-policy.json
├── input.schema.json
├── output.schema.json
├── references/
├── examples/
└── evals/
```

`tool-policy.json` 是严格交集：Skill 声明工具、全局 Tool Registry、G819 runtime manifest 和当前用户授权四者同时允许才可调用。不能照搬某些开发 Agent 中“allowed tools 只是免确认但不限制其他工具”的语义。用户可通过引导式编辑器创建“家用电器外观”“复古工业语言”“紧凑桌面设备”等专属 Skill，但发布前必须通过 Schema、无任意代码/URL/路径、示例、失败样例和零副作用 dry-run；失败 Skill 保持禁用。

### 5.8 Codex 式 DesignWorkspace 与视觉收敛

目标 Agent 不只是选择一个完整 Recipe，而是在 Rust-owned `DesignWorkspace@1` 中读取和修改 typed design sources：

```text
brief/reference
→ MechanicalMorphologyProgram
→ ProfileSketch/ProfileSectionSet
→ Shape/Assembly sources
→ Material Zone/SurfaceAdornmentProgram
→ compile/readback/render
→ VisualConvergenceReport
→ 修改同一 workspace
```

Workspace revision 是未保存草稿，不是第二条 AssetVersion 链。每次 `DesignWorkspacePatch@1` 必须绑定预期 revision、typed AST 操作、预算和幂等键；编译、截图和比较图按 source hash 派生。只有 V003 硬门通过且用户确认，Rust 才把密封 source、ShapeProgram、AssemblyGraph、Surface Program 和 readback 原子提升为不可变资产版本并更新 `ActiveDesignSnapshot`。

视觉收敛采用 [ADR-0017](ADR/0017-codex-design-workspace-visual-convergence.md) 的确定性优先方法：`VisualDetailInventory@1` 中每个关键细节都必须映射到真实 morphology/operation/Material Zone/A005 输出；固定构建阶段逐步通过合同、编译、多视图、材质和表面 Gate；视觉模型只在 hard gates 通过后判断难以确定化的外观差异。该自动判断用于开发和有界修复，不能替代 C111B/PV006 的机械硬表面独立评分或后续 M108B 的跨领域三位真人评分。

## 6. 核心领域合同

### 6.1 DomainPackManifest@1

定义领域的意图样例、部件角色、Connector、Joint、Shape 模板、材质集合、质量 Profile 和导出 Profile。领域包不包含可执行代码。

详见 [首批领域包设计](DOMAIN_PACKS.md)。

### 6.1.1 DomainInferenceResult@1 与 ConceptScopeDecision@1

在任何 Planner 或 Geometry Worker 写入前，领域推断必须返回判别结果：`recognized` 绑定唯一 Pack，`ambiguous` 给出两个到四个候选，`unsupported` 不给候选。D001 冻结 Schema/Pydantic/fixture，D002 停止旧的默认武器回退，D003 将普通含糊类别记录为单个 clarification Turn/Item。

G814 在其后增加 `ConceptScopeDecision@1`：`allowed` 才能进入 Planner，`clarification_required` 复用 D003 的单问题，而 `unsupported` 只保留一个已完成的 scope-stop Turn/Item。该停止发生在 Provider/Planner 前，只允许 Thread/Turn/Item/幂等记录写入，不产生 Plan、Blockout、Version、Asset、Snapshot、Quality 或 Export。规则集刻意有限且版本化，不能被称作关键词即完整安全系统；工具权限、ShapeProgram 限制和确认边界仍独立存在。

### 6.2 MechanicalConceptSpec@1

目标字段：

```text
schema_version
project_id / domain_pack_id
brief / intended_use
design_language
overall_envelope
proportion_targets
symmetry
detail_level
material_intent
pose_or_stance
generation_stage
safety_scope
```

`generation_stage`：

```text
blockout | segmented_concept | editable_asset
```

Spec 描述意图和约束，不直接保存三角形或任意代码。

### 6.2.1 SingleGenerationAttempt@1、GenerationGateReport@1 与 SingleResultDecision@1

`SingleGenerationAttempt@1` 绑定 `turn_id + attempt_id + ShapeProgram hash + Recipe provenance`。`GenerationGateReport@1` 绑定该尝试的 `GLB hash + compile_readback_id + render_fingerprint + gate_profile_version`，记录硬门事实、可修复字段和可读摘要；它不是审美评分或候选排序。`RepairAttempt@1` 只可由失败报告指定的字段触发，最多一次，并绑定同一 Brief、SubjectProfile、核心轮廓意图、parent attempt 和 runtime manifest。`SingleResultDecision@1` 只引用通过硬门的最终尝试，保存通过理由和已知缺口；它不保存隐藏推理，也不成为 AgentAssetVersion。

Brief、Domain Pack、Skill、runtime manifest、核心 Recipe/轮廓意图或安全范围任一变化都禁止原位修复，必须开启新 Turn。用户确认唯一结果后才创建 Agent 资产；用户要求“换一个思路”会创建新 Turn，绝不在同 Turn 生成第二完整模型。

### 6.2.2 ProviderExecutionTrace@1 与 AgentSkillManifest@1

`ProviderExecutionTrace@1` 只保存脱敏运行事实：连接状态、attempt、Item 生命周期、延迟、usage、缓存命中、固定错误类别和取消原因；不保存 Key、完整 prompt、完整 response、原始 reasoning 或内部 URL。

`AgentSkillManifest@1` 保存稳定 ID、版本、用途、触发条件、允许领域、输入/输出 Schema、严格工具列表、引用资源 hash、示例/eval 版本、作者/来源/许可证、启用状态和失败原因。Skill 不能修改全局 Tool Registry、运行时白名单或 Snapshot 规则。

### 6.2.3 MechanicalStyleToken@1

Style Token 表达相对的机械视觉语言，而不是制造尺寸：

```text
style_id / version / display_name
proportion_profile
edge_radius_ratio
surface_tension
panel_gap_ratio
trim_scale
detail_density
symmetry
material_palette_id
lighting_profile_id
allowed_domains[]
provenance
```

D005 当前实现 `compact_rounded`、`aerodynamic_sleek`、`industrial_substantial` 与 `clean_balanced` 四个 builtin Token，并为四领域各提供 4 个普通语言比例 Recipe。Recipe 先解析稳定语义部件槽，再同时核对当前 G808 ratio binding 与 G826 GLB surface provenance；只输出一个声明步长的 preview 输入。所有数值是有界比率或离散档位，不能产生 mm、壁厚、载荷、推力、气动或制造建议；Style Token 不创建第二套参数真值。更多 Token、连续参数和 Agent 自动选择仍是后续范围。

### 6.2.4 ProfileSketch@1 与 ProfileSectionSet@1

`ProfileSketch@1` 是受限二维轮廓合同：

```text
sketch_id / version / plane
closed / winding
normalized_bounds
segments[]（line / quadratic / cubic 的受限集合）
holes[]
symmetry / continuity_hint
resample_count
source / provenance
```

`ProfileSectionSet@1` 按主轴引用一组排序截面：

```text
section_set_id / version / main_axis
sections[]（position + profile_sketch_id + bounded scale/twist）
cap_start / cap_end
resample_policy
symmetry
```

前端 SVG path 只是编辑表示；提交时必须转换为规范 JSON，由后端重新验证闭合、自交、绕序、孔洞、连续性、点数和预算。六个互不相关的面不能分别进入几何 Worker。Loft/Sweep 只能消费 G819 已启用的合同和有序引用。

### 6.3 AssemblyGraph@1

AssemblyGraph 是目标装配真值：

```text
Assembly
└── PartNode[]
    ├── role / parent_id / child_ids
    ├── geometry_source
    │   ├── registered_asset
    │   └── shape_program
    ├── transform / pivot / mirror / lock
    ├── connectors[] / joint
    ├── material_zones[]
    ├── editable_parameters[]
    └── provenance / immutable_glb_hash
```

Graph 必须有一个或多个 root，但不能有环、悬空引用或重复 ID。局部修改只影响目标节点和由 Joint/Connector 明确依赖的子树。

### 6.4 ShapeProgram@1

ShapeProgram 是部件或候选组合的程序化几何真值。

运行时接受集合不在本文重复维护。已实现的 `ShapeProgramRuntimeManifest@1` 位于 `packages/concept-spec/fixtures/shape-program-runtime-manifest.json`，是 Schema、Pydantic、Worker、编译/readback、质量入口和导出共同消费的唯一清单；Schema enum 由 `contracts:types:generate` 生成，运行时会拒绝 schema/manifest 漂移。文档、prompt、Skill 或前端不能单独扩大它。当前仅声明并执行 box、cylinder、wedge、capsule、profile/extrude/revolve、loft、sweep、mirror/array/radial_array、受限 union/subtract、bevel_approx 和 surface_panel；未知、缺执行器或运行时非法参数返回 `UNSUPPORTED_RUNTIME_OPERATION`，不得跳过节点后继续成功。质量入口通过 `GeometryCompileReadback@1` 消费同一次编译后的 GLB triangle、bounds、operation/output role、material、normal/UV0/tangent、稳定 face→part/zone 和 hash 事实；导出使用同一编译/readback 结果。readback 损坏时质量显式为 unavailable 且导出拒绝，旧估算报告不能被当作当前证据。这仍只是概念 Mesh/GLB 事实，不是工程、结构、材料或安全结论。

硬性限制：unknown field 拒绝；数值有限且有范围；引用有序无环；禁止 URL、路径和可执行文本；operation、array、布尔深度、bounds 和 triangle budget 有上限；canonical JSON、validator 与 runtime version 进入 hash。

ShapeProgram 的目标演进是在同一合同内保存有序、不可变的 feature nodes：每个节点包含稳定 ID、operation、输入节点、规范参数、role/zone provenance、预算和输入 hash。派生顶点/索引是编译结果，不是可编辑历史；修改节点生成新 ShapeProgram/ChangeSet，不破坏性改写旧网格。不得同时建立另一套 FeatureGraph 作为竞争真值。

### 6.4.1 EditableComponentRecipe@1

Recipe 是可编辑 3D 组件库的复用单元：

```text
recipe_id / version / component_role
profiles[] / section_sets[]
geometry_features[]
parameter_bindings[]
connector / pivot
material_zones[]
child_slots[]
allowed_domains[]
quality_profile
source / provenance / review_state
```

Recipe 只引用已存在的 ShapeProgram operation、G808/D005 参数绑定、稳定 Material Zone 和已审阅资源。当前 C105 的 `EditableComponentRecipeRegistry@1` 是代码所有的 first-party、`ForgeCAD-Internal-Visual-Only`、不可再分发目录；source/review/license 是强制可追溯边界，不是对第三方素材或真实工程材料的声明。实例化只创建零写候选；替换、调参和换材质仍走 preview → ChangeSet → confirm。父子 Recipe 必须无环，锁定、跨领域、连接不兼容、质量失败、来源不明、registry/ref stale 或超预算均拒绝。Recipe 不表示工程装配、公差、紧固、载荷或功能机构。

每个确认后的实例保存 `recipe_id + version + recipe_sha256`、registry SHA-256、instance path、parent/slot、connector `up`、pivot、领域、审阅和许可证事实到 `AssemblyGraph.component_recipe_instances[]`；旧资产永远按其已保存 ref/hash 可读，绝不被同名 Recipe 的新版本隐式改写。不可解析或不匹配的 ref 必须被拒绝为 stale；升级只能显式生成新的 preview→confirm 子版本。

child slot 是一个固定的、已审阅 child Recipe 引用，不是任意组件选择器。显式 slot binding 只能启用该固定 child；项目内替换必须继续经过 C102 兼容性事实和 ChangeSet。局部装配按 `parent world × parent connector × slot local × inverse(child connector) × child root` 计算；静态 GLB/当前 worker 无法表示的残余旋转或缩放、非正交 frame 和循环全部 fail closed，不能在几何端忽略。Rust core 展开 Recipe、校验 hash/slot/connector 并创建候选 AssemblyGraph/provenance；Python 只编译经 Rust 展开的受限 ShapeProgram，不能读取 Recipe/项目状态或写 Snapshot。

`ComponentRecipeCandidate@1` 的 `expanded` 状态只说明 Rust 配方展开完成。它必须随后拥有同一 ShapeProgram 与 `interactive_preview`/`production_concept` profile hash 的实际 GLB/readback，才可能进入预览或质量路径。C105 的小型四领域 fixture 是合同/线路证据，不是 M108B 的最终视觉资产，也未证明照片级或独立人工 `4/5`。

### 6.5 Connector 与 Joint

Connector：`surface_mount`、`axial_mount`、`socket_mount`、`panel_mount`、`wheel_mount`、`wing_mount`、`tool_mount` 等。

Joint：`fixed`、`revolute`、`hinge`、`slider`、`ball_preview`。Joint 保存轴、原点、概念限位和默认姿态；只用于装配、姿态与简单动画，不用于工程动力学。

### 6.6 MaterialPreset@1

MaterialPreset 保存 metallic-roughness PBR 参数、纹理、透明、发光、涂层、标签、来源、许可证和版本。Material Zone 使用稳定 ID；换材质不改变部件和几何 ID。

详见 [视觉材质系统](MATERIAL_SYSTEM.md)。

### 6.7 DesignChangeSet@2

允许操作：

```text
replace_part / add_part / remove_part
set_parameter / replace_shape_program
set_transform / set_pivot / set_mirror
set_connector / set_joint_pose
set_material_binding
split_part / merge_parts
```

`split_part` 和 `merge_parts` 只修改候选 AssemblyGraph；确认前必须展示新旧分件和受影响范围。

状态：`proposed → previewed → confirmed`，或 `rejected / stale / failed`。

### 6.8 ModelQualityReport@1

通用 P0 检查：

- 空网格、非有限值、退化面、非法索引；
- bounds、三角预算、法线、开放/非流形边；
- 父子关系、悬空节点、Connector 和 Joint 引用；
- 严重穿插、重复隐藏几何和异常尺度；
- pivot、材质槽、GLB 回读和 provenance；
- 领域包附加的概念一致性规则。

检查不是 DFM、结构、空气动力学或安全认证。

### 6.9 DesignWorkspace、Morphology 与视觉收敛合同

`DesignWorkspace@1` 是当前 Turn 的可恢复设计草稿；`DesignWorkspacePatch@1` 是带 revision/CAS 的 typed source 修改；`MechanicalMorphologyProgram@1` 表达 scaffold、连接、topology family、负空间、壳层、材料区和表面意图，并由 Rust 降低为现有 `ShapeProgram@1`、`AssemblyGraph@1` 和 A005 合同。它们不接受任意网格、代码、URL 或路径。

`VisualDetailInventory@1` 将 macro/meso/micro 细节绑定到实际输出并记录 `planned | lowered | readback_verified | unresolved`；critical unresolved 项阻止唯一结果展示。`DesignBuildLedger@1` 记录同一 workspace 的 silhouette、structure、form、material、surface、lighting 和 optimization 阶段，不创建中间资产版本。`VisualConvergenceReport@2` 依次消费来源/合同、真实 GLB readback、多视图结构、PBR/Material Zone 和受控渲染证据；任一 hard gate 失败时不得由 VLM 覆盖。`@1` 仅可读取旧 V003/C111 回归记录。

这些是 ADR-0017 的目标合同。C111A 已被接续，当前 C111B 只允许以冻结 fixture 先验证黄金机械臂的细节清单、固定视图和质量合同；在对应 C112–Q004 原子任务完成前，USER_GUIDE 不得宣称 Agent 已经可以自由编写通用三维设计程序。

### 6.10 VisualAcceptanceContract 与成本证据

`VisualAcceptanceContract@1` 是目标合同，用于把“看起来高质量”转成一次生成可执行、可审计的验收输入。最小字段包括：

```text
brief_id / domain_distribution / intended_use
authorized_reference_ids
must_show / must_not_show
macro_claims / meso_claims / micro_claims / material_claims
fixed_view_ids
triangle_budget / texture_budget / latency_budget
provider_call_budget / max_repair_attempts
human_review_protocol / failure_and_stop_conditions
```

Provider 预算必须按职责隔离，不能用一个全局 `maximum_calls` 同时约束离线生成和外部视觉验收。C111B 的受控 `C111BVisualAcceptanceContract@2` 已验证该边界：生成链保持 0 网络/0 可变成本；视觉比较只有在显式用户授权、调用前预算预留和当前 Project sealed evidence 都存在时才可调用，每候选最多初次 1 次加两次同意图修复，总可变成本硬上限 `100000 microusd`。这只是 benchmark 上限，不是默认消费许可；专用 `visual_reference_comparison_report` 必须进入 evidence lineage。

C111B 当前已把上述边界落成 Rust 产品状态：短期 `VisualReferenceComparisonAuthorization@1` 精确绑定 Project/request/graph/policy，第一次预留绑定实际 Turn；SQLite 0044 迁移和 CoreRepository 的 IMMEDIATE transaction 在每次网络调用前分配保守 ceiling，三次累计严格不超过 `100000 microusd`。Provider future 被取消、超时或丢弃时按已开始网络保守结算；可证明的凭据/校验等预网络失败才释放预留。`VisualReferenceComparisonReport@2` 可携带 `VisualReferenceComparisonBudgetEvidence@1`，而生产网络结果缺少该证据会被 Rust 拒绝。授权和账本不进入几何、版本或 Snapshot 真值。

参考 claim 必须标为 `observed | inferred | unknown`，并绑定可验证的 reference/view/region；`inferred` 和 `unknown` 不能进入 must-show 硬事实。每条输出 claim 最终必须映射真实 source、Part/output、Material Zone/texture、GLB readback 和固定视图，不得只存在于 prompt 或 inventory 文案。

质量分为 macro 轮廓/比例/负空间、meso 部件层级/连接/壳体过渡、micro 倒角/紧固/线束/Decal/磨损、PBR、presentation 和 asset usability 六层。确定性 hard gate 先检查结构、lineage 和跨视图一致性；VLM 只处理难以确定化的参考差异；独立真人判断审美和目标用途。三者证据分开存放，任何一层不能覆盖前一层失败。

每个真实生成还派生 `GenerationCostEvidence@1`（目标合同）：Provider/model capability fingerprint、调用/用量、缓存命中、修复次数、阶段耗时、估算可变成本、授权和停止原因。它只用于预算、模型路由和商业验证，不进入几何真值。没有联网授权时 `network_call_count` 必须为 0。通用 GenerationCostEvidence 仍由后续 Q004/评测任务落地；C111B 已先用专用 Authorization/Reservation/BudgetEvidence 运行时合同完成视觉比较硬停，不把它泛化成所有 Provider 的成本真值。

## 7. 当前机械知识包（兼容与回归）

以下四包是当前 Alpha 已实现的机械知识、fixture 和回归集合，不是 ADR-0022 的对象准入列表。U002 后所有合法对象先进入 `SubjectProfile` 和 `RepresentationPlan`；包存在与否只能增强术语、Recipe 和评测，不能决定是否接受输入。

### 7.1 Future Weapon Prop

完整外观角色包括主体、前后外壳、握持外壳、视觉附件、能源/存储造型和面板。只做非功能性概念道具。

### 7.2 Vehicle Concept

完整外观角色包括车身、座舱、底盘视觉、车轮/履带、灯组、空气动力外观件和内饰剪影。关键参数包括轴距、轮距、离地高度、车身尺度、轮径和座舱位置。

### 7.3 Aircraft Concept

完整外观角色包括机身、座舱罩、主翼、尾翼、发动机舱、起落装置视觉和面板。关键参数包括翼展、机身尺度、后掠视觉角、翼厚和布局。

### 7.4 Robotic Arm Concept

完整运动链包括基座、肩关节、连杆、肘关节、前臂、腕关节、末端工具和护罩。关键参数包括自由度、各段长度、关节外壳、工具和姿态。

每个包的详细角色、模板和验收见 [DOMAIN_PACKS.md](DOMAIN_PACKS.md)。

## 8. 完整外观与自动分件

本节描述目标闭环。`当前前端已展示 Planner 的三个方向` 是 F026 前的历史事实，不能据此恢复方向 UI；F026 已移除它；在 V003 前只允许临时适配器使用 legacy Planner 第一条文本方向编译/展示一个结果，不能把它称为硬门通过的唯一结果。后端已完成四领域 48 个 blockout 变体、确定性分件候选、受限 AgentAssetVersion 编辑和 R002–R004 四视图、条件式爆炸概念 PNG 与当前 PNG/manifest 图包。前端变体目录、自由合并/拆分和深度几何分件尚未实现。

### 8.1 Blockout

Agent 先生成完整体量：主轮廓、比例、对称、姿态和主要空隙。当前 `quick_sketch` 保留轻量体量；`showcase` 只通过版本化的本机规则追加有上限的外观面板、分缝视觉线、护板、孔洞/紧固件、灯带和线缆槽视觉线，并使用固定的石墨、复合外观、金属外观与灯带发光 PBR 映射。相同 profile、ShapeProgram、GLB、AssemblyGraph、分件候选与确认链始终同源；视觉部件没有参数绑定或机械臂 Joint。它不从 Brief 推导功能、尺寸、材料或工程结构，视觉线/孔洞/灯带也不等于真实槽、开孔、电气或散热设计。R002/R003 已从同一 Agent 资产生成三分之四、正面、侧面、顶部和条件式透明爆炸概念 PNG；后者仅在真实 Part/几何组一一对应时出现。R004 仅在当前预览 fingerprint 匹配时，将这些 PNG 与 machine-readable manifest 打包下载。它们通过导出抽屉展示，是绑定资产版本的只读派生结果，不是工程渲染、装配或制造图。

### 8.2 Segmented Concept

Agent 按 Domain Pack 角色提出分件。目标系统验证层级、最小部件尺寸、连接关系和可选择性，并允许用户预览边界、合并或拆分候选；当前已支持由现有 AssemblyGraph、稳定 role、受限 primitive output 和连接事实共同证明的候选 split/merge 预览，但只支持受限 ChangeSet，不能任意指定切割线或自由合并网格。深度自动分件和整组之外的自由编辑仍是目标能力。

### 8.3 Editable Asset

确认后补齐稳定名称、pivot、Connector、Joint、Material Zone、缩略图、质量报告和 GLB。可编辑资产不等于最终美术，也不等于工程 CAD。

## 9. 几何运行时

- 当前运行时：仓库自有的受限 ShapeProgram mesh worker，已执行 `box`/`cylinder`/`capsule`/`wedge` 及文档列明的有限组合操作；主视口以同一 Three.js renderer 显示四类基础外形和 display-only 柔化边缘/工作室环境，不成为几何真值；
- Manifold：G825 已按 ADR-0013 将 Python 3.5.2 接为唯一生产 CSG handler；只处理已验证封闭输入和有界 union/subtract，在隔离进程内支持取消/超时，并把内核版本、node/input/result hash 与 surface/material provenance 写入同次 GLB readback。它不是自由 mesh 修复、B-Rep 或工程实体内核；
- Trimesh：候选网格分析、Scene 和 GLB 实现，未集成，必须固定版本并验证依赖；
- Khronos glTF-Validator：锁定的开发/CI 标准合规门；glTF Transform core/extensions：锁定的开发评估依赖。四领域评估已证明标准读取阶段仍保留所需 Part/zone/material 映射，但 writer 会删除 ForgeCAD 真实 readback 所需的显式默认 PBR 参数，因此写出按预期被拒绝并禁止作为生产 export transform；functions 的 dedup/prune 与 KTX2/BasisU 仍未通过当前平台/编码/解码 Gate；
- Three.js：交互预览，不是几何真值；
- Blender：可选专业 DCC 往返，不是用户前置依赖。

两阶段构建：preview 使用较低细节，final 在确认/导出时重建、检查和固化。几何运行在可取消 worker/process；失败返回 operation ID、错误码和建议。

候选采用边界见 [GitHub 参考架构](AGENT_GITHUB_REFERENCE_ARCHITECTURE.md)。ShapeProgram 是稳定合同，底层候选只能有一个进入默认安装包，不能同时制造多套几何真值。

### 9.1 视觉真实度构建链

目标不是简单增加三角形，而是按顺序建立可追溯的外观层：

```text
Brief / reference evidence
→ DomainSemanticProportionRecipe + MechanicalStyleToken
→ EditableComponentRecipe
→ ProfileSketch / ordered section sets
→ Extrude / Revolve / Loft / Sweep 主形体
→ bounded CSG / array / edge finish
→ GeometryCompileReadback
→ UV0 + tangent + stable Material Zones
→ PBR texture set / supported glTF material extensions
→ HDRI studio lighting + tone mapping + color management
→ multi-view concept render
→ visual benchmark + GLB validate/inspect
→ candidate evaluation
```

几何最低要求：主次体块清楚、边缘有受控圆角/倒角近似、连接处不悬浮、重复细节有节奏、左右/径向重复稳定、面板/接缝服从曲面与部件边界、零尺寸和明显穿插在候选阶段拒绝。细节仍是外观表达，不解释为通风、散热、电气、紧固或真实功能。

材质最低要求：每个正式视觉材质可引用 base color、metallic-roughness、normal、occlusion 和 emissive 纹理；汽车漆、涂层塑料等可在 Three.js 与导出端共同支持时使用 clearcoat，透明件可在明确兼容边界内使用 transmission/IOR。每张纹理有内容 hash、用途、色彩空间、分辨率、来源、许可证和回退。KTX2/BasisU、meshopt/Draco 等优化只能在 GLB readback 和目标平台兼容 Gate 通过后启用。

展示最低要求：固定可复现的 HDRI/工作室环境、物理正确灯光、线性色彩工作流、统一 tone mapping、接触阴影和受控环境遮蔽；不得用强轮廓线、过曝高光或随机背景掩盖几何问题。mini 与中央 focus 使用同一个场景、renderer、相机状态和材质资源。

资产层级分为 `preview`、`editable`、`export` 三档，但都从同一 ShapeProgram/AssemblyGraph 重建，不保存三套互相漂移的网格。preview 优先反馈速度；editable 保留选择与材质区；export 才执行完整 validate/inspect、纹理压缩和未引用资源清理。

DeepSeek 不是网格生成或纹理服务。默认 Provider 边界只允许它把模糊意图写成紧凑的 `ForgeVisualAuthoringIntent@1`：选择视觉架构、比例、材质与表面语言，不直接填写 Shape operation、内部 Part/Zone/Detail ID 或可执行代码。Rust Core 将该意图确定性降级为严格 typed `ForgeVisualProgram@1`，并在校验失败时只返回结构化、可有界修复的错误；Rust 继续负责引用完整性、操作白名单、预算、hash、GLB/PBR readback、八视角、质量、版本和导出。`ForgeVisualAuthoringIntent@1` 只是一次编译输入，`ForgeVisualProgram@1` 是设计源信封，二者都不创建第二条资产版本链：lowering 后仍由 `ShapeProgram@1`、`AssemblyGraph@1`、Material Zone、Surface program、`AgentAssetVersion@1` 和 `ActiveDesignSnapshot@1` 分别承担既有真值。

程序化视觉主链为：

```text
Natural-language brief / authorized reference evidence
→ Qwen observed/inferred/unknown evidence
→ DeepSeek typed ForgeVisualAuthoringIntent
→ Rust validate + lower to ForgeVisualProgram
→ Restricted Shape/Assembly/Material/Surface compilation
→ real GLB/PBR readback + fixed eight-view review
→ at most one typed in-place repair of the same intent
→ one transient result → user confirm
→ procedural_shape_program AgentAssetVersion
→ ForgeAssetPackage
```

该链当前已完成 PV001–PV005、PV006A/B 与 PV008：Rust 合同/lowering、机械臂真实 fixture/双档编译、typed author/patch、七阶段构建与八视角收敛、同一资产三次修改/确认/恢复/GLB/六成员资产包工程闭环，以及多模态请求、只读证据图和独立千问 Vision 端口。千问只读取本 Turn 明确授权的 CAS 图片并返回 `observed | inferred | unknown` 证据，不得生成网格、执行代码、写 Snapshot 或直接修改资产；其配置与 DeepSeek 分离。2026-07-27 的真实 DeepSeek 验收已以 `provider_authoring_ir` 来源完成 author→build→readback→render→evaluate→preview→confirm→Snapshot→export；它证明单独文本 Provider 的工程链真实可用，不证明真实千问+DeepSeek 组合、目标图级视觉质量或任意类别自由生成。

PV006C 的目标是把 Rust-normalized request+graph 接入原生 `turn/start`，在任何永久写入前重读 sealed evidence、校验当前 Snapshot/选择/锁定，并为每条视觉 claim 提交 `bound | unresolved | evaluation_only` disposition；Rust 再构造并随 preview/AssetVersion 持久化 `MultimodalProgramEvidenceBinding@1`。候选 GLB 的确切八视图与 sealed 参考像素只能通过千问 comparison port 形成逐 claim assessment；Rust 用 hash-only comparison input/report 决定三层相似度、修复目标和硬门。离线 exact-lineage 机制已有局部证据，但真实千问+DeepSeek 组合 E2E 仍缺当前可复现成功证据，因此 PV006C 保持 `blocked`。PV006 的 20 条多模态盲测与真人门、PV007 的逐领域扩展也未完成，不得宣称自由生成或收藏级 MVP 闭环。历史 N001–N004 远程网格合同仅为兼容读取，不注册命令、凭据、网络或 UI；千问视觉理解不替换 ShapeProgram/Surface Compiler。

### 9.2 建模语法路由

Agent 不得对所有对象应用同一 primitive 模板。建模路由只从 Domain Pack、part role、Style Token、Recipe 和当前 runtime manifest 计算：

| 结构 | 主语法 | CSG/细节作用 |
| --- | --- | --- |
| 机柜、打印机、工业设备外壳 | Profile + Extrude | 门、开口、控制区和局部倒角 |
| 汽车、飞机、咖啡机、吸尘器外壳 | 多截面 Loft | 窗洞、轮拱、进气口和局部罩体 |
| 轮胎、旋钮、轴套、关节罩 | Revolve | 孔、槽和阵列细节 |
| 扶手、管路、框架、线缆外观 | Sweep | 接头、端帽和局部裁剪 |
| 机械臂、设备、工程机械 | Component Recipe + Connector | 可替换装配和受限姿态 |
| 接缝、标识和浅表面细节 | decal / normal / roughness | 原则上不增加网格 |

路由结果必须可解释为“为什么选择这种主形体语法”，但不向零基础用户暴露 operation ID。请求的语法未进入 G819 manifest 时，候选失败并选择另一条已实现 Recipe；不能把不支持的节点静默删除后返回低质量模型。

### 9.3 轮廓、截面、Loft 与 Sweep

不分别制作前后左右上下六个独立表面。目标流程以正视、侧视、顶视轮廓和沿主轴的共享横截面形成封闭体：

```text
ProfileSketch
→ validate / normalize / resample
→ ordered ProfileSectionSet
→ extrude | revolve | loft | sweep
→ cap / normals / surface provenance
→ compile/readback
```

Loft 的每个截面必须有确定顺序、统一重采样数量、受限缩放/扭转、封盖和曲率边界；Sweep 必须限定 profile、路径、frame 计算、弯曲/扭转上限和自交拒绝。每个新 operation 单独交付 Schema、Pydantic、runtime、确定性 topology hash、预算、GLB readback、损坏/退化输入和重启回归，不能一次批量开放。

### 9.4 CSG 与特征历史

目标特征链保存操作节点，而不是破坏性修改顶点：

```text
base_shell = loft(sections)
wheel_arch = subtract(base_shell, arch_cutters)
windows = subtract(wheel_arch, window_cutters)
trimmed = union(windows, reviewed_trim_components)
finished = edge_finish(trimmed, bounded_edge_set)
```

G824 比较现有 Worker、Manifold Python 与 WASM，G824A–G824D 补齐双平台 provenance/readback、隔离进程、真实临时权威状态提升和 packaged 预算/许可证。G825 已按 ADR-0013 删除旧 box 布尔作为默认 fallback，只保留一个 Manifold Python handler；旧 G805 ShapeProgram 继续经该 handler 编译。每个有序节点保存规范参数、输入节点/hash、runtime/kernel version、surface/material provenance 和结果 hash；任何 boolean 失败返回稳定节点 ID/错误码且不输出部分模型。该能力仍是受限概念级 CSG，不得宣传为通用工程布尔。

### 9.5 表面完成与 GLB 事实

几何完成顺序固定为：

```text
受控 edge finish
→ split/weighted normals
→ UV0
→ tangent
→ stable face/Material Zone provenance
→ PBR binding
→ GLB validate/readback
```

法线、UV、tangent 和 zone 必须来自真实编译结果；不能由 UI、材质名称或平行估算猜测。G826 已把 `TANGENT`、`_FORGECAD_FACE_ID`、`_FORGECAD_SOURCE_FACE_ID`、part-instance 和 zone 写入同一 GLB，并验证单位/正交 tangent、非退化 UV、完整 face ID 和非空不重叠 zone。优化或重索引必须携带这些顶点属性和 extras；丢失即 readback 失败。受控 edge finish 只允许 X/Z 周边、半径比例不超过 0.25、1–3 级细分，并明确称为 approximation。M108A 把完整纹理集、clearcoat/透明兼容、工作室环境和色彩管理接到这些事实上；M108B 再验证 Recipe 组合后的最终视觉质量。

同一资产只保留一条不可变 ShapeProgram/版本链，展示性能通过派生工件 profile 解决：

```text
AgentAssetVersion + ShapeProgram
├── interactive_preview
│   ├── 24 段基础旋转体
│   ├── 128×128 v3 PBR
│   └── 编辑、ChangeSet preview、即时工作台反馈
└── production_concept
    ├── 48 段与更高 capsule 半球采样
    ├── Loft/Sweep 平滑法线
    ├── 512×512 v4 PBR
    └── Q003、展示、多视图、正式下载与导出
```

`GeometryArtifactProfile@1` 的 canonical hash 必须进入 GLB root extras、`GeometryCompileReadback@2`、派生缓存 key 和响应头。production 只按实际使用材质惰性生成纹理，并写入内容寻址对象库；前端先显示 preview，再在同一 renderer 中原子替换 production。profile 仅定义工件预算和传输层级，不自动证明完整产品外观；C111B/E005/PV006 只保留机械程序化回归，跨类别正式质量由 U005 证明。

### 9.6 UI、GSAP 与可丢弃 SDF 边界

SVG 编辑器只编辑 `ProfileSketch@1`；HTML 只显示表单和属性；GSAP 只负责 mini/focus、抽屉、步骤、相机、爆炸图和确认状态的可取消动画。实现优先使用 Timeline、transform/autoAlpha 和 `gsap.matchMedia()` 的 reduced-motion 分支。动画完成与否不得改变 ShapeProgram、Snapshot、质量或版本。

SDF/体素只可作为早期、未保存的形体探索候选，并必须在进入 editable asset 前重建为受限 ShapeProgram。它不能提供稳定 Part、Recipe、UV、Material Zone 或最终 GLB provenance，因此不能成为资产真值。

## 10. 工作台信息架构

```text
┌─────────────────────────────────────────────────────────────┐
│ 项目名   保存状态                  撤销   检查   导出       │
├───────────┬────────────────────────────┬────────────────────┤
│ 对话记录  │ Agent 会话 / 连续步骤       │ 单一 3D dock       │
│ 组件库    │ 单一最佳结果 / 确认条       │ 点击→中央 focus    │
│ 项目列表  │                            │                    │
│           │ 理解 → 构建 → 检查 → 结果  │ 材质/部件选择同源  │
├───────────┴────────────────────────────┴────────────────────┤
│ +  输入你的创意或继续修改…                       发送       │
└─────────────────────────────────────────────────────────────┘

点击右侧 3D dock 后，同一个 canvas 移入中央 focus 层：

┌──────────────┬──────────────────────────────────────────────┐
│ 会话摘要     │ 单一 3D 中央焦点视图                    关闭 │
│              │                                              │
│              │       选中部件动作按需浮出                   │
└──────────────┴──────────────────────────────────────────────┘
```

docked 与中央焦点不是两张视图；状态机只切换 `viewport_dock = docked | focus` 并移动同一个 canvas 容器。底部“+”只打开已有 Style Token、视觉材质和参考输入，不直接创造几何 operation 或第二状态真值。属性、检查和导出按需打开。用户不选择 Mode、Agent、内部候选、Skill 或 Domain Pack；专属 Skill 由 Agent 根据触发条件使用，技术详情只在明确的管理表面出现。

## 11. 主工作流

以下是目标工作流，不是当前 Alpha 的完整能力清单：

```text
输入 Brief / 授权参考 / 已有资产
→ SubjectProfile + VisualFeatureContract
→ RepresentationPlan；知识包只提供提示
→ 一次受限完整合成
→ 编译/readback/概念渲染/硬门；仅可最多一次同意图 typed patch
→ 只展示通过硬门的唯一结果 + 多视图概念图
→ 自动分件候选
→ 用户确认 AssemblyGraph
→ 部件级对话修改和材质
→ editable_asset 检查
→ 用途导出
```

Provider 离线时，已有项目仍可打开、手动编辑、检查和导出；只禁用新的 Agent Turn。

当前已验证的最小闭环以 [零基础用户指南](USER_GUIDE.md) 为准。目标验收和当前差距分别见 [测试策略](TEST_STRATEGY.md) 与 [生产发布清单](PRODUCTION_RELEASE_CHECKLIST.md)。

目标界面、轮廓、Recipe、材质区、Provider 诊断和失败恢复的完整操作顺序见 [3D 机械设计系统目标操作手册](MECHANICAL_DESIGN_OPERATIONS.md)。

## 12. 数据、安全与材料边界

- API Key 存系统密钥存储；数据库只存引用；
- 日志、Item、导出和 crash report 不含密钥；
- ShapeProgram 永不 `eval`/`exec`；
- 导入/导出路径经过允许根和 canonical path 检查；
- 临时候选与正式对象分区；
- 原创、审阅、质量和许可证来自真实记录；
- 视觉 MaterialPreset 不自动转换为工程材料 Profile。

## 13. 性能目标

以下是目标，必须在真实 Tauri 环境测量：

| 项目 | P0 目标 |
| --- | --- |
| 本地神经模型与权重 | 0 |
| WebGL context | 1 |
| 首个步骤反馈 | <1 秒，不含 Provider 网络 |
| blockout preview | 默认 ≤100k triangles |
| editable asset | 默认 ≤250k triangles，可按 Profile 调整 |
| Turn 可取消 | 100% |
| Thread/Version 重启恢复 | 100% |
| GLB 导出成功率 | ≥98% |
| 新用户首次有效导出 | 中位数 <5 分钟 |
| 单次合成运行时/readback 硬门 | 100% 初始尝试及每次原位修复先验证，失败工件不得展示 |
| 正式 Material Zone 外观覆盖 | 100% 有有效 PBR 绑定或明确参数回退 |
| U005 跨类别视觉基准 | 八类冻结未见任务分别报告；首次独立真人 `≥4/5` 至少 70%，最多一次 typed patch 后至少 85% |
| 首次有效结果时间 | 中位数 <5 分钟，P90 <10 分钟 |
| 可变推理与存储成本 | 商业验证中不高于实收收入的 25%；当前仅为 target |
| 表示能力晋级 | 每种 representation/capability 单独冻结任务并达到独立人工 `≥4/5`，不继承机械臂结论 |
| 默认可见完整模型数 | 1；不存在内部多模型比较或用户选择器 |

## 14. 后续 Engineering Pack

Engineering Pack 可独立引入 build123d/OpenCascade、B-Rep、STEP/3MF、真实材料 Profile 和领域工程检查。它使用另一套质量门，不把 Mesh ShapeProgram 或视觉材质冒充工程真值。
