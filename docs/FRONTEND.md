# ForgeCAD CAD 工作台前端

版本：2026-08-01
状态：当前实现、已知问题与目标约束

## 0.1 2026-08-01 W4 集成验证

W3 工作台 shell 已从 `d602eb9` 合并，并由 `48e72cf → 9f94911 → 8af0abd` 完成类别开放 smoke/CSS 修复。主环境后续 F025、T002 14/14、desktop typecheck/build、F026、F006 均 PASS；这些结果证明工作台结构和回归场景，不证明真实 Provider 或通用视觉质量。此前隔离 worktree 缺少 `tsc`/`playwright-core` 的失败仅作为 harness 历史记录保留。

## 1. 唯一产品入口

桌面应用只保留 CAD 工作台。旧武神首页、任务中心、独立资产库、Mode、Patch、Forge 和独立设置页不得重新成为产品导航。

ADR-0022 后，工作台不增加“角色 Mode”“生物 Mode”或机械/有机双产品入口。U002 已让用户只提交文字、图片或当前资产；Agent 以普通语言展示对象理解、需要补充的视图、所选表示能力和限制。四领域澄清卡已从产品主路径移除，只保留 legacy fixture；通用澄清仅用于真实身份/目标冲突，typed limitation 用于能力不足。

当前实现：

```text
apps/desktop/src/features/cad-workbench/
├── CadWorkbenchPanel.tsx
├── AgentConversation.tsx
├── AgentStepItem.tsx
├── AgentSelectionCard.tsx
├── agentBlockoutPreviewPresentation.ts
├── agentPlanSourcePresentation.ts
├── ComponentDrawer.tsx
├── MaterialDrawer.tsx
├── QualityDrawer.tsx
├── ExportDrawer.tsx
├── WorkbenchDrawerStack.tsx
├── WorkbenchInspectorRail.tsx
├── ModuleGraphViewport.tsx
├── agentAssetWorkspaceState.ts
├── useAgentAssetWorkspace.ts
├── legacyCompatibilityDisplay.ts
├── LegacyCompatibilityNotice.tsx
├── viewportDisplayPreferencesState.ts
├── useViewportDisplayPreferences.ts
├── legacyModuleGraphWorkspaceState.ts
├── useLegacyModuleGraphWorkspace.ts
├── useConceptWorkbench.ts
└── cad-workbench.css
```

### 1.1 K001–K003 当前原生边界

`ForgeApiClient` 的命令、SSE 和资源 URL 已统一经过 `appServerTransport`。packaged Tauri WebView 使用 Rust invoke/event bridge 和 `forgecad.app-server/1`；浏览器开发壳使用同一 JSON-RPC frame 合同的 loopback compatibility adapter，不再直接把产品 API 当作第二条前端 transport。连接在 `initialize` 之后才接受请求，并处理稳定 request ID、取消、通知确认、有界恢复和 SSE cursor replay。

普通 JSON/二进制命令走 `compat/http`，且只能命中代码所有的 HTTP method + segment-shape 白名单。图片、GLB、媒体和下载 URL 在 packaged WebView 中走 GET-only、无状态的 `forgecad-resource`；它复用相同路径/header 策略，不允许写请求，也不保存业务状态。browser development 因没有原生 custom protocol，仍由受限 loopback adapter 返回同一资源字节。

`appServerTransport` 明确不持有 Snapshot、Version、ChangeSet、质量、导出或 asset head。`ForgeApiClient` 在 packaged Tauri 中以 native DTO 读取 Rust-owned 生命周期，并通过同一 bridge 访问 Rust core 拥有的 Project/Snapshot/ChangeSet/Quality/Export；旧 Python lifecycle/product POST 默认返回 410。Python 只执行受限几何，不再写产品状态。F026 已移除三方向 UI，并以同一 canvas 的 docked/focus、固定 composer 和单结果槽通过 Gate；K003/F026 完成仍不把临时第一方向适配结果误写成 M108B 或 V003。

## 2. 当前可见流程

1. 输入明确领域和完整对象 Brief；
2. 查看一个临时兼容 3D 结果；它仅使用 legacy Planner 第一条文本方向；
3. 继续生成系统匹配的受限视觉 blockout；临时流程没有方向选择或“换一版外观”；
4. 查看分件候选；
5. 保存为 Agent 可编辑资产；
6. 使用固定动作调整部件、材质或组件；
7. 预览后确认子版本；
8. 运行 Agent 资产轻量检查；
9. 导出 Agent GLB。

当前不应在 UI 或文档中承诺转台视频、自由 split/merge、完整版本历史浏览或 Agent 多格式资产导出；C103 仅在当前装配/几何事实足够时显示“预览拆分/预览合并”，仍需 ChangeSet 确认；C104 已提供 Snapshot 持久化的部件锁定、隐藏和单独查看：锁定会禁用并由后端拒绝相关 ChangeSet，隐藏/单独查看只作用于现有主视口且不创建版本；R002–R004 已提供四视图、条件式透明爆炸概念 PNG 的只读预览/单图下载，以及仅在当前预览 fingerprint 一致时可下载的 PNG/manifest 图包。R005 将 Agent 下载抽屉收敛为“下载 3D 模型 (GLB)”、生成/下载单张概念图和下载概念图包三类直接动作；旧 Concept 的用途、OBJ 和源包选项不会在 Agent 路径显示。

当前只提供少量固定比例、材质、组件和部分关节动作；自然语言自由参数仍属于目标能力。

F026 的当前契约是移除三方向和“第 N / 3 版”UI：临时兼容适配器只读取 legacy Planner 第一条文本方向并编译/展示一个 3D 结果。它不是模型质量等级、`SingleGenerationAttempt@1`、硬门通过或新的目标体验；ADR-0010 的正式单次结果仍须等待 V003。

M108A 当前工作区已把显示工件拆成两档：交互和 ChangeSet 先加载 `interactive_preview`（128×128 v3），正式质量、展示、下载和导出读取 `production_concept`（512×512 v4）；桌面在同一 renderer/context 中后台加载 production 并原子替换，项目切换或迟到响应不得覆盖当前资产，production 失败则保留 preview 并显示原因。该前端工件闭环不等于 M108B 视觉 `4/5`；现有固定 showcase 仍不是 Recipe-backed 生产级概念资产基线。

## 3. 当前结构问题

2026-08-01 对本地 Vite 工作台做了 1280×720 首屏实机检查，并与用户提供的目标图对照。详细测量、目标 token 和四 Luna 实施边界见 [U004 第一阶段高质量工作台总图](U004_STAGE1_HIGH_QUALITY_WORKBENCH_PLAN.md)。当前最重要的事实不是“换一套皮肤”，而是页面状态与布局责任已经漂移：

- 首次进入、没有发送任何 Turn 时，右侧助手已经显示“需要重试”，流程区显示“生成失败，请重试”，属于错误初始状态；初始 presentation 必须固定为 `idle`；
- `CadWorkbenchPanel.tsx` 当前 2,632 行，已经超过 F025 `<2200` 行的责任预算；`ModuleGraphViewport.tsx` 2,744 行，渲染生命周期与大量 overlay/展示职责仍耦合；
- `cad-workbench.css` 当前 7,106 行；`.f026-layout`、`.f026-right-rail`、`.f026-conversation-stage` 等核心选择器被多个历史段和 media query 后置覆盖，fixed、bottom offset、overflow 和 z-index 互相叠加，是遮挡和裁切的主要根因；
- 1280×720 下右侧助手内部滚动，底部历史被视窗裁切，中央视口同时受左右栏和固定底栏挤压；高质量 3D 没有成为最大视觉区域；
- “快速修改”同时出现在视口覆盖层和右侧助手；需求、AI 分析、候选、结果、输入又在同一栏常驻，操作来源和当前阶段不清；
- 空项目仍突出导出、四段历史和失败恢复，和“第一阶段先稳定展示高质量模型”的任务冲突；
- 品牌图标与 `ForgeCAD` 文本在当前首屏发生视觉碰撞，说明顶栏仍依赖位置补丁而不是稳定的 flex slot；
- 已确认无运行时 import 的三方向概念预览前端 state/hook 和组件库偏好 state/hook 已在 2026-08-01 清理；后端 `AgentBlockoutConceptPreview@1`、legacy fixture 和现有 Export/Snapshot 真值链继续保留。

仍有效的当前结构事实：

- A003 已让 Provider 配置读取失败保持可见，并将 metadata、Keychain、supervisor restart 和 Agent capability 组合成启用门；只有四项就绪才允许“测试连接（会联网）”。连接测试与普通 Turn 均显示真实 `network_call_made`/稳定错误，提供取消入口，Provider 失败会中止当前 Agent 路径而不会继续调用 legacy Planner。真实四领域质量评测仍未执行；
- Agent 对话、Kernel 步骤、分件选择卡和抽屉已由 F002–F005 提取为独立组件；F025 后 Agent 抽屉栈只组合质量与导出，legacy Graph/参数/旧格式只在用户显式打开的只读表面加载；
- Agent 选择、质量和 GLB 导出已读 Snapshot；遗留 Concept 兼容出口不属于 Agent 产品能力；
- F005 已将抽屉渲染组合收敛到 `WorkbenchDrawerStack`；F007–F024 的仍在用展示层继续保持只读 projection；F019 将当前 project/domain/source 的材质关键词、分类与适配筛选提取到 `useAgentMaterialFilterPresentation`，F022/F023/F024 保留单结果兼容展示和 plan 来源。F025 进一步删除 Agent Turn/修改意图对 legacy Planner 的回退，Agent 导出/质量抽屉不再接收旧版本、旧质量或旧格式 props；`useConceptWorkbench` 只有在显式入口打开后才读取旧版本、ChangeSet、审计和 ModuleGraph，并以 request guard 拒绝迟到响应。上述本机层均不拥有 selected material、Material Zone、asset head、Snapshot revision、ETag、转换授权、ChangeSet、质量写入或导出身份；相机/灯光仍由 Snapshot CAS 拥有；
- F006 已消除工作台用户界面中低于 11px 的辅助文字，补齐 32/40px 控件基线、可见焦点、中文 aria 标签、状态播报以及抽屉 Escape/焦点返回；完整屏幕阅读器人工验收仍未完成。

## 4. 下一阶段状态架构

前端必须只消费 [ActiveDesignSnapshot](AUTHORITATIVE_STATE.md)：

```text
CadWorkbenchMachine
├── snapshot
├── providerConversationEnvelope
├── projectConversationMemory
├── agentTurnState（idle → authoring → compiling → evaluating → ready/failed）
├── providerConnection
├── generationResultPresentation
├── appearanceCompilationPresentation
├── changePreview
├── drawers
├── viewportDock（docked | focus）
└── transientViewportState
```

Project、AgentAssetVersion、Selection、Quality 和 Export 不再由不同 hook 分别推断“当前”。localStorage 只保存抽屉高度、筛选等无害 UI 偏好。

S004 已完成 `ForgeApiClient` 的 Snapshot GET/select/legacy-rebuild hand-off 调用、ETag 读取和可恢复错误映射；S005 已完成独立 `activeDesignMachine` reducer 与乱序响应 smoke。S006 已把 Agent 资产恢复、分件列表选择、视口高亮、质量检查和 GLB 导出接入 Snapshot；S007 已将 legacy Concept 收紧为只读并要求显式重建授权。S008 已持久化 preview/quality，引入服务端导航 frame，并在顶部提供撤销/重做；R001 已把相机视图和灯光预设通过 CAS 绑定 Snapshot。仍在用的本机 UI 偏好和概念图/编辑辅助读取缓存不以任何形式替代资产或 Snapshot 真值。下一步必须把 Provider conversation、结构化 project memory、token-budget compaction 和 stable-prefix/cache receipt 接入 Rust 状态，再让前端只消费可显示的阶段事件；前端不得自行拼接历史或把刷新失败映射为生成失败。

## 5. 目标布局

```text
Tauri Desktop
└── CAD Workbench
    ├── 顶栏 72px：品牌、项目、自动保存、AI 生成、修改、展示；导出进入更多菜单
    ├── 左侧 264px：新建设计、最近作品；模板为次要入口
    ├── 中央：持续可见且面积最大的唯一 Three.js 视口
    ├── 右侧 360px：AI 助手、当前步骤、折叠历史和固定输入框
    ├── 3D focus：隐藏两侧栏并扩大同一 canvas，关闭后恢复
    ├── 浮层：只在选中部件时出现 3–5 个上下文动作
    ├── 底部 112px：只有存在结果/版本时显示修改历史
    └── 按需抽屉：材质详情、检查、兼容导出
```

F005 只改变组件边界，不改变运行时真值：`WorkbenchDrawerStack` 是无状态的组合层，`CadWorkbenchPanel` 继续通过 props 提供数据和回调，Snapshot/ETag/ChangeSet/下载副作用仍只能由父层或服务端拥有。F006 已处理可访问性与主流程的可读性；F007 已让 `useWorkbenchLifecycle` 独占生命周期请求和抽屉焦点的短暂状态，F008 已让 `useAgentConversationPresentation` 独占会话展示与 project/request 过期屏障；二者都没有新增版本真值或领域能力。完整状态机拆分仍未完成。

目标布局不等于当前已经全部实现。

### 5.1 目标图式信息层级

目标图提供的是空间和层级参考，不是可直接使用的 3D 资产。ForgeCAD 保留 Codex 的一个任务、连续状态和可检查动作，但把真实 3D 视口放在中央：

```text
左列（264）             中央（弹性，最大）                右列（360）
┌──────────────────┐   ┌──────────────────────────────┐  ┌──────────────────┐
│ + 新建设计        │   │ 视角 / 爆炸 / 渲染模式        │  │ AI 助手          │
│ 最近作品          │   │                              │  │ 当前步骤展开      │
│                  │   │     唯一 3D viewport         │  │ 已完成步骤折叠    │
│ 模板（次要）      │   │     完整主体与材质细节        │  │ 快速修改（唯一）  │
│                  │   │                              │  ├──────────────────┤
│ 设置 / 帮助       │   │ 选中部件时才出现局部动作      │  │ 描述或继续修改…   │
└──────────────────┘   └──────────────────────────────┘  └──────────────────┘
                       结果存在后：版本/修改历史 112px
```

中间只常驻 3D，不常驻第二个会话或属性面板。右侧只展开当前阶段，已完成步骤折叠为摘要；同一组快速修改不能同时出现在视口和助手。内部候选、评分表、Skill 名、工具 JSON、Provider model ID、缓存 hash 和 ShapeProgram 不进入默认界面。

### 5.2 单 renderer 的 standard / focus 切换

为兼容现有合同仍保留纯 UI 状态名 `ViewportDockState = docked | focus`，但 `docked` 从现在起表示中央标准三栏位，不再表示右侧迷你视口。切换时只能扩大同一个 canvas host 并隐藏两侧栏，不能 mount 第二个 `ModuleGraphViewport`。场景、相机、选择、高亮、材质、资源缓存和 `ActiveDesignSnapshot.render_preset` 保持不变；切换不创建版本、质量、导出或 Snapshot revision。

验收至少覆盖：快速连续点击、Escape/关闭、窗口缩放、项目切换、抽屉打开、模型重载、选中/隐藏/隔离、renderer/context 计数始终为 1、geometry/material/texture 无重复分配和焦点返回中央标准视口按钮。

### 5.3 结果状态槽与单次唯一结果

F026 先实现 `GenerationResultPresentation@1`，其纯展示状态为 `idle | processing | compatibility_result | ready | failed`，不拥有 Snapshot、Version、Quality、Export 或候选真值。`compatibility_result` 仅由 legacy Planner 第一条文本方向的临时适配器产生，显示一个 3D 结果且明确不含 V003 Gate/修复结论；`ready` 只由 V003 的 `SingleResultDecision@1` 消费，显示一个 `SingleResultCard`：完整外观预览、Agent 理由、Brief 已覆盖/未覆盖摘要、当前来源（离线或模型服务）、“查看 3D”“继续修改”“换一个思路”三个动作。最后一个动作创建新 Turn，不能修改已确认版本。

类别开放产品链只进行一次完整 author、硬门和最多一次同意图 typed patch；UI 不显示多个完整候选、计数、方向标题或评分。`VisualConvergenceReport@2` 以外的 legacy V003/C111 记录不得进入产品结果卡。F026 不渲染、隐藏或复用三方向交互；它只能将 legacy Planner 第一条文本方向适配为 `compatibility_result`。该状态不得冒充 `ready`、不得产生 `SingleResultDecision@1`，失败状态必须显示失败阶段、已保存资产是否安全和下一步。

U004 的类别开放 executable 在 GLB/readback 后会显示“正在同工作台检查外观”，而不是复用旧结果或提前显示 `ready`。桌面把精确候选 GLB 装入唯一 `ModuleGraphViewport` 并采集固定八视图；Rust 将 Turn 置为 `waiting_for_capture`。capture 后仅恢复评估：通过才显示唯一确认卡；失败才执行一次 typed patch，补丁后的新候选必须再次采集，第二次失败显示终止且保留既有已确认资产。UI 不展示 nonce、原始图片、工具 JSON、Provider 内部重试或第二次修复入口。

### 5.4 Provider 状态与错误

顶栏显示四态：`未配置（离线）`、`正在连接`、`已连接`、`需要处理`。点击状态打开诊断抽屉，显示配置是否存在、Keychain 是否可读、最后检查时间、是否真的发起网络请求、错误类别和普通语言修复；不显示 Key、完整 Base URL、原始响应或隐藏推理。

错误文案至少区分：请求格式、API Key、余额不足、参数/模型、请求过多、服务故障、超时/网络、JSON 空响应、Schema 不符。保存 Provider 后必须等新 Agent 进程报告 `ready` 才显示已连接，不能只因 metadata 写入成功就声称“现在可以生成真实设计方向”。

### 5.5 ProfileSketch 轮廓编辑器

轮廓编辑是目标高级辅助，不是当前本机 Alpha 软件已开放的用户能力。它只在选中 Recipe 声明可编辑 `ProfileSketch@1` 时按需打开，不成为常驻 CAD 面板。

```text
SVG 控制点 / 普通语言比例档位
→ 规范化 ProfileSketch JSON
→ 后端闭合、自交、绕序、孔洞、点数与预算验证
→ ShapeProgram preview
→ ChangeSet confirm
```

前端不得把 SVG path 字符串、DOM 节点或六个独立面直接送入 Geometry Worker。正/侧/顶轮廓和横截面必须引用同一组规范数据；切换截面只改变编辑目标，预览仍使用唯一 Three.js renderer。轮廓错误保留已确认资产并显示可修复原因，不输出部分成功网格。

### 5.6 Surface Layer 二维外观编辑

C107 的表面抽屉以真实 SVG 显示受限二维预览和当前 Material Zone，但 SVG/HTML/CSS 永远不是模型或版本真值。用户仍通过现有 preview→confirm 流程提交；Rust 将受限 token 校验并密封为 `RestrictedSurfaceLayerInput@1`，Python 只把五通道 PBR 写入已验证 zone，GLB readback 必须带 lowering、retained 和每张 map 的 hash。工作台部件拾取、两点距离/法向角测量和剖切均复用唯一 `ModuleGraphViewport`，测量标注是临时 UI 状态，不写 Snapshot 或资产版本。

### 5.7 GSAP 动画边界

GSAP 只用于状态已确定后的展示过渡：中央标准视口扩展为 focus、Agent Item 进入、抽屉/确认条、相机和爆炸图。Timeline 必须可暂停、反向和取消，优先动画 `x/y/scale/rotation/autoAlpha`；使用 `gsap.matchMedia()` 提供 `prefers-reduced-motion` 分支。

动画不得生成网格、执行布尔、产生 UV、创建版本、写 Snapshot 或决定质量。状态机是动画输入；动画中断后 UI 必须落到明确的 `docked | focus`、open/closed 或 preview/confirmed 状态，不能留下第三种视觉真值。

## 6. 零基础规则

- 首屏不出现 Domain Pack、Mode、Skill、pipeline、Connector、Joint、GLB 或 PBR；
- 未知领域只问一个澄清问题；
- 每一步只突出一个主动作；
- F026 过渡期不显示“换一版外观”、三方向或 `N / 3` 选择器；兼容适配器只使用第一条 legacy 文本方向。V003 后以正式单次合成替代该适配器，“换一个思路”创建一次新的单次合成 Turn。两种路径都不得显示变体 ID、技术编号或自由造型参数，也不得影响已保存设计；
- 技术错误翻译为“发生了什么、资产是否安全、下一步做什么”；
- 所有永久修改先预览再确认；
- 版本、质量和导出始终显示同一活动资产；
- 组件和材质只在需要时展开；
- Material Zone 抽屉先显示当前部件/区域，再用搜索和中文分类缩小视觉材质；纹理对象不存在时明确显示参数外观回退；
- Material Zone 的选择和“预览材质”动作只通过父层回调传递 `part_id`/`material_zone_id`，抽屉不直接写 Agent 资产版本；当前 blockout 若只有一个 zone，只显示真实的一个 zone，不人为制造多 zone；
- “适合当前设计”只读取当前 Domain Pack 对应的 `allowed_domains`；“全部视觉材质”是明确的用户切换，未知领域只提示尚未确认，不把任何材质自动标为适配；
- 内部 part role 必须映射为用户可读中文名称。
- 已实现的 C101 映射仅用于显示：候选卡、已选部件、材质上下文和组件保存名称共享同一只读词典；未知值显示“未命名部件”，不暴露内部 role、不猜测领域或功能。
- C102 的项目内组件替换先读取后端候选结论：只显示同领域、同 role、启用且来源质量为 `passed`/`warning` 的候选，并用“来源检查通过/有提示；保留当前连接位置”说明原因；未检查、失败、停用或不匹配组件不提供替换动作。正式 Module Asset 的审阅状态不移植到 `AgentComponent`。
- C103 的结构建议必须来自当前 `AssemblyGraph`、稳定 role、受限 ShapeProgram 输出和已有连接事实；选择卡只显示“预览拆分/预览合并”及简明影响说明。建议为空时显示“暂不能建议”，不猜测切割线、连接、强度或功能；永久修改仍由父层通过 ChangeSet/Snapshot 负责。
- C104 的显示与保护状态只来自当前 `ActiveDesignSnapshot.part_display`；选择卡不能以本地 state 伪造锁定。锁定后编辑、组件替换和结构建议均不可提交；隐藏或隔离导致部件不可见时必须清空该选择。视口继续复用唯一 renderer/context，不得为单独查看创建新预览器。

交互参考 Codex 的重点是“一个任务、连续步骤、明确状态和可检查的动作”，不是把 coding agent 的终端、Mode 或权限面板搬进 CAD。默认只显示用户目标、Agent 选出的一个结果和下一步；技术详情、组件、材质、检查和导出按需展开。

所有类别共用一个壳、同一套选择/版本/确认逻辑。Domain Pack 或 Representation capability 只能改变建议、角色、组件、材质、执行计划和评测，不得增加汽车、角色、生物、飞机、机械臂或武器专属工作台。

目标操作细节见 [3D 机械设计系统目标操作手册](MECHANICAL_DESIGN_OPERATIONS.md)。

## 7. 可访问性与视觉最低线

- 正文不低于 12px，辅助文本不低于 11px；
- 常用点击目标至少 32px，关键主动作至少 40px；
- 键盘焦点清晰；
- Agent 状态、错误和完成使用 `aria-live`；
- 不仅依靠颜色表达状态；
- 支持 1180×760 最小窗口且不出现主流程横向滚动；
- 模型在默认灯光下具有可见轮廓和选中反馈。

## 8. 性能边界

- 工作台生命周期内只有一个 WebGL canvas/context；
- 不在组件卡中创建第二个 Three.js renderer；
- 释放被替换的 geometry/material/texture；
- 组件筛选和 Agent 消息不得重建 renderer；
- 大型几何工作不在浏览器主线程执行；
- 前端 bundle 超预算时进行路由/功能懒加载。

## 9. 验证

前端变更至少运行：

```bash
npm run desktop:typecheck
npm run desktop:build
npm run desktop:r3-concept-workbench-smoke
```

K001/K002 transport 或 native lifecycle 变更还必须运行 `npm run k002:code-gate` 和 `npm run k002:packaged-gate`。packaged Gate 必须同时保留 K001 的真实 `.app` WebView 业务/重启链，并由 K002 原生探针验证双启动、未配置 Provider、`network_call_made=false`、稳定失败、Item 顺序、旧 POST 410 和无隐藏 reasoning；仅运行 Rust 单测或直接请求 Python helper 不算 packaged 前端证据。

原生交互还必须运行 `script/build_and_run.sh --verify` 并人工检查 Keychain、文件选择、下载、重启和单 renderer。详细矩阵见 [TEST_STRATEGY.md](TEST_STRATEGY.md)。
