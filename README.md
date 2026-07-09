# 武神 Forge

武神 Forge 是一个开源的 3渲2国风神兵设计桌面 Agent 软件。它面向游戏美术资产生产，不先做战斗、数值或玩法逻辑。

项目目标很直接：

> 用户输入文字、草图或非武器物体，Agent 自动生成可迭代的幻想战斗物体方案、概念图、局部修改记录、资产库条目，并生成可进入 Unity 流程的 3D 粗模。

## 产品定位

武神 Forge 类似 Codex / Claude Code 的 Agent 工作方式，但目标不是写代码，而是生成武器美术资产。
核心是：输入先表达“结构与意图”，先做结构解释与重诠释，再落在可进入游戏流程的神兵资产上。
用户给出的是“对象”与“意图”，不是“武器类别”。

入口约束（不再回退到分类器）：

- 首次输入可为任意可解释对象：服饰、工具、家具、器物、自然形体、抽象几何都可进入流程。
- 用户第一次提交后不做“是否武器”的分类拦截；如果解释能力不足，走重试而不是拒绝。
- `interpretation` 在结构解释层先给 2~3 条候选，再由用户确认一个候选进入后续生成。

合同层定义中 `WeaponDesignSpec@1` 保持兼容写盘，`CreativeWeaponGraph@1` 与 `SkillGraph@1` 为下一阶段主抽象，但阶段目标不依赖 `weapon_family` 分类前置。

GPT-Pro 对齐的执行闭环（每轮验收前置）：

- 先做 `interpretation`：2~3 个候选；
- 用户单选一个候选并 `recast/confirm`；
- 仅确认后的 `creative_graph` + `skill_graph` 允许继续 `concept -> patch -> generate-3d -> export-unity`；
- 资产链路必须可追溯 `creative_graph_id -> skill_graph_id -> weapon_version -> export_version`；
- 非武器对象不允许因分类失败被阻断（椅子、钥匙、树枝、花盆也必须走完整解释闭环）。

它是“草图/描述驱动的幻想战斗物体工作台”：

- 先解析输入结构：形体轮廓、骨架关系、握持点、攻击源、受控节点
- 进行 Creative Recast：把任意物件解释为可执行的神兵语义（不要求它“像武器”）
- 先给出 2~3 个结构候选，让用户确认（如防弹裤/木棍/椅子等）
- 依据确认后的结构再生成 concept、局部修改与技能映射
- 自动调度大模型 API、ComfyUI 与 3D 工作流
- 保存每次生成、修改和导出资产到不可变资产库
- 最终输出可在 Unity 流程继续加工的 3D 粗模

## 第一阶段范围

第一阶段只做自由输入与资产生成闭环：

```text
用户描述/草图/非武器输入
-> 结构解释（形体/骨架/握持/攻击源/结构约束）
-> Creative Recast（对象重诠释）
-> 结构候选确认（2~3 套，用户选择）
-> CreativeWeaponGraph（目标态）
-> WeaponDesignSpec（兼容输出）
-> 概念图
-> 局部修改
-> SkillGraph（目标态，6+ 技能卡）
-> 3D 展台（展览台 + 简单角色 + 360）
-> 3D 粗模
-> Unity ZIP
-> 可直接进 Unity 的 3D 粗模数据
```

第一阶段验收附加项：

- 3D 展台必须支持 360° 旋转，且可切换“角色持握点 / 穿戴点”提示。
- `auto_run` 路径中，`POST /api/weapons` 首次返回可包含 `needs_confirm=true`，但不得省略候选确认。
- 同一对象重复跑一次，必须保留至少一个候选排序不完全回退（有 `stable_seed` 可复现输入）。

第一阶段不做：

- Steam 游戏本体
- 战斗系统
- 数值系统
- 联机乱斗
- Unity 运行时集成
- 成品级 3D 模型保证

## 风格方向

核心风格是 **3渲2国风神兵**：

- 强剪影
- 强轮廓线
- 国风纹样
- 神兵质感
- 外观逼真可信，有重量感、视觉材质感和装饰细节层次
- 玉、金属、骨、晶石、灵纹、符箓、火焰、雷电、水墨、灵气等幻想元素
- 适合 Unity 中继续加工成游戏资产

自由度原则（目标态）：

- 先看“结构”，再看“能力”，永远不先按器类分类（剑、刀、枪只是结果标签，不是入口）。
- 任意物件都先进行结构解释（骨架、握持、攻击源、受控节点）再转为战斗形态。
- 先生成 2~3 个结构候选（如“防弹裤”可对应防御炮台/控制域/位移装置），用户先确认候选再继续生成。
- `WeaponDesignSpec@1` 与 `weapon_family` 仅保留兼容回放，不作为主分类主键，主入口必须是 `creative_graph_id + skill_graph_id` 的语义链路。
- 允许“防弹裤”“木棍”“椅子”“镜子”等非典型输入，目标是高自由度而不是类别覆盖。

统一的解释框架（GPT Pro）：每个对象先走 4 层结构解释，不再用兵器类别做第一决策。

- **结构层**：尺寸、骨架、受力点、可动关节、连接关系（握持、悬浮、穿戴、展开、折叠）
- **交互层**：握持路径、放置路径、吸附关系、旋转与位移意图
- **功能层**：`combat_affordances`（攻击、防护、控制、召唤、移动、反射、变形、回收等）
- **资产层**：材质区、发光区、法阵纹样、Unity handoff 参数（Socket/比例/插槽）

执行约束：

- 每次 `interpretation` 默认返回 `2~3` 个候选（`rank` 稳定、`confidence` 可解释）
- 候选必须至少覆盖两个 `combat_affordances` 分支（例如 `shield+area_control` 与 `mobility+projectile`）
- 同一对象重复复测时允许轻微波动，但至少保留一个候选 `rank` 不回退到完全不同能力组
- 候选未确认时，`concept / patch / generate-3d / export-unity` 统一阻断为 `INTERPRETATION_NOT_CONFIRMED`
- 候选生成不足 2 条时先进入一次重采样；重采样后仍不足 2 条或字段缺失时返回 `PROVIDER_BAD_OUTPUT`，不得降级为单一类型模板

实现约束（目标态）：

- `interpretation` 输出不应返回单一“类型”；每次给到 2~3 个候选且每个候选携带至少 1 个 `combat_affordances`。
- 选中的 `selected_candidate_id` 与 `selected_candidate_rank` 进入 `POST /api/weapons/{weapon_id}/recast/confirm`，再固定为后续 `creative_graph_id + skill_graph_id` 来源。
- 单条 `POST /api/weapons/{weapon_id}/generate-3d` 或 `POST /api/weapons/{weapon_id}/export-unity` 在未确认前必须阻断。

解释闭环补充（v1）：

- 每次 `/interpretation` 必须返回 2~3 条候选，且至少两条在 `combat_affordances` 上有结构差异。
- 每条候选必须有 `anchor_points`、`protected_regions`、`risk_tags` 与 `confidence`。
- 只允许单选，未确认前不可进入 concept/patch/3D/export。

子智能体执行约束（固定）：

- 并发子智能体上限：`8`
- 运行映射允许 12 个职责角色，但时序层仅允许最多 8 个实例并发。
- 并发溢出时采用同一工具节点内职责复用与串行化，而不新增无限角色。

GPT Pro 式进阶思路（本阶段建议默认启用）：

- 把候选的 2~3 路视为“结构解释预算”，优先比较它们的 `combat_affordances` 与 `anchor_points` 差异，而非仅看文本相似度。
- 每条候选至少要绑定一个 `protected_regions` 和一个 `skill_anchor_points`，让 Patch 与 3D 都有稳定约束来源。
- 每轮手工验收固定一组“非武器对象”，并要求结果至少产出 2 种不同能力方向，避免恢复到固定类型映射。
- 对象池建议按周补充：椅子、钥匙、树枝、雨伞、花盆、风车、花环、门把手、书卷、鞋底、花草等，优先覆盖材质/结构/交互差异。
- 若某类非武器对象第一次回退到单一模板，进入“重采样模式”：保留一个稳定 `rank`/`confidence` 作为锚点，重新生成其余候选；若仍无法形成 2 条以上有效候选，则本轮失败，不继续出概念图。

反模式（禁止）：

- 不能在输入阶段要求用户先选武器类别或预设分类。
- 不能用 `weapon_family`、`type`、固定类型模板作为第一决策字段。
- 不能拒绝“非武器输入”；只要有结构语义即可推进候选重诠释。
- 不能在解释候选未确认前直接启动概念图/3D。

### 非具化输入示例（高自由度）

| 输入 | 解释示例 |
| --- | --- |
| 防弹裤 | 腰部环形炮台 + 护体风雷阵 |
| 木棍 | 符文炮杖 / 锁链炮杖 |
| 椅子 | 王座炮台 / 折叠领域盾阵 |
| 镜子 | 反射法器 / 召唤门 |
| 伞 | 天幕防御阵 / 针雨伞 |
| 门 | 传送门阵 / 空间折叠符箓 |
| 戒指 | 玄纹护符场 / 启灵召唤环 |
| 树枝 | 龙骨锁链刃 / 祭坛杵 |
| 花盆 | 守域护甲 + 召唤触须 |
| 钥匙 | 传送触发枢纽 |
| 风车 | 反射旋场 |
| 花环 | 持续增益场 |

### 对象池建议（第一阶段）

- 已覆盖：防弹裤、木棍、椅子、镜子、伞、门、戒指、树枝
- 每周新增至少 2~3 个对象：贝壳、花盆、钥匙、风车、花环、竹简、车把

### 生成对象种类收口（建议）

为避免回退到“武器模板思维”，第一阶段的对象池建议覆盖 4 类：  
1) 服饰/穿戴类：裤子、护符、鞋底、手套、戒指  
2) 家具/结构类：椅子、花盆、门把、车把、栏杆  
3) 工具/器械类：锤子、雨伞、钥匙、卷轴、书签、风铃  
4) 天然/抽象形态：树枝、贝壳、风车、花环、花草藤蔓、网面、环形框架  

验收规则（每周）：

- 同一轮至少 3 条非武器对象必须产生 2~3 候选，且至少两组候选在 `combat_affordances` 主轴不同。  
- 每条候选必须包含 `anchor_points`、`protected_regions`、`risk_tags`；缺失任一项时本轮阻断 interpretation。  
- `generate-3d` / `export-unity` 仍只允许在确认 `creative_graph_id` 后进入。  
- 每次回归测试需保留一个候选 `stable rank`，用于校验解释排序与复现实用性。

### 更高细节目标（闭环验收）

- 非武器对象必须通过 `interpretation` 并产生 2~3 个候选，拒绝任何“按类别先验拒绝”。
- `recast/confirm` 成功前禁止 `generate-3d` / `export-unity`。
- 生成结果需持续可追踪：`creative_graph_id` -> `skill_graph_id` -> `版本 -> 资产` 链路完整。

### 自由度滑块（用于控制可编辑性与可用性平衡）

- 形态自由度：保守 / 奇异 / 异形 / 超现实
- 神化程度：现实材质感 / 国风神兵 / 仙术机关 / 神话概念
- 玩法复杂度：轻量技能 / 多段技能 / 变形联动 / 多形态
- 资产可用性：概念优先 / 低模优先 / Unity 直用优先

可选增强策略（用于提升自由度而不失可用）：

- 先做结构锚点：每个候选必须明确 `抓持 / 发射 / 控制 / 连接 / 变形` 的主路径。
- 先做能力映射：每个候选至少 1 个主 `combat_affordance`，1 个增强 `combat_affordance`，减少“只有外观形似”的假阳性。
- 先做风险标签：每个候选至少 1 个 `risk_tags`（如 `pivot_sensitive`、`symmetry_break`），用于 Patch 与 3D 阶段稳定性提示。
- 先做复现性：对同一对象的候选排序记录 `seed` 说明；再次验证时至少保持 1 个候选 rank 稳定。

更进一步的“更好的想法”：把物体语义固定为 4 层解释，而不是 1 层命名：

1. **结构层**：尺寸感、骨架、受力点、可动关节；
2. **交互层**：握持/放置/穿戴、吸附、扭矩路径；
3. **功能层**：`combat_affordance`（攻击/防护/控制/召唤/移动等）；
4. **资产层**：材质区、发光区、法阵纹样、Unity handoff 参数。

这样可以稳定支持“防弹裤=移动炮台”“木棍=符文炮杖”“椅子=折叠领域盾阵”等非具像化闭环，而不会回退到武器类别模板。

## 重要边界

武神 Forge 面向虚构游戏资产和高拟真武器外观设计。这里的“逼真”指视觉可信：比例、视觉材质、磨损、雕刻、结构层次、发光区域和 3D 形体看起来像能存在于游戏世界里。

项目不输出可用于现实制造武器的精确图纸、制造尺寸、材料配方、加工流程或结构工艺。这个边界只限制现实制造可操作性，不限制外观细节、国风神兵气质或游戏资产的视觉真实感。

换句话说，产品可以生成逼真的武器外观、概念图、局部改稿、3D 粗模、Unity 材质意图和资产库元数据；但所有内容都必须保持为虚构游戏美术资产和非制造说明，不能变成现实武器的工程设计资料。

## 技术栈

首选技术栈：

| 模块 | 技术 |
| --- | --- |
| 桌面端 | Tauri |
| 前端 | TypeScript + React |
| Agent 后端 | Python |
| Agent 编排 | LangGraph |
| 本地 API | FastAPI |
| 草图/标注 | tldraw / Excalidraw / Fabric.js |
| 图像生成调度 | ComfyUI 外部服务 |
| 大模型调用 | OpenAI-compatible API adapter |
| 3D 粗模 | Hunyuan3D / Stable Fast 3D / TripoSR / TRELLIS adapter |
| 3D 预览 | Three.js / React Three Fiber |
| 资产处理 | Blender Python + glTF-Transform |
| 数据库 | SQLite |
| 资产文件 | 本地项目文件夹 |

LangGraph 目标执行链路（第一阶段）：

1) 输入解析（文本/草图）
2) Creative Recast（结构重诠释）
3) CreativeWeaponGraph（结构图）
4) WeaponDesignSpec + SkillGraph（玩法与约束）
5) 概念图 -> Patch -> 3D -> Unity 交付

## GitHub 参考项目

| 项目 | 用途 |
| --- | --- |
| [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | 桌面应用外壳、文件权限、跨平台打包 |
| [dieharders/example-tauri-v2-python-server-sidecar](https://github.com/dieharders/example-tauri-v2-python-server-sidecar) | Python FastAPI sidecar 示例，参考 Tauri sidecar 生命周期与打包/运行脚本 |
| [langchain-ai/langgraph](https://github.com/langchain-ai/langgraph) | 多 Agent 流程编排、状态机、可追踪执行 |
| [cline/cline](https://github.com/cline/cline) | 参考 Agent 工具调用、人类确认、执行日志体验 |
| [All-Hands-AI/OpenHands](https://github.com/All-Hands-AI/OpenHands) | 参考长任务执行、工作区、任务轨迹 |
| [Aider-AI/aider](https://github.com/Aider-AI/aider) | 参考版本化迭代与变更记录思路 |
| [tldraw/tldraw](https://github.com/tldraw/tldraw) | 草图画布、标注、圈选 |
| [excalidraw/excalidraw](https://github.com/excalidraw/excalidraw) | MVP 手绘草图输入 |
| [fabricjs/fabric.js](https://github.com/fabricjs/fabric.js) | 正式版自定义画布、图层、对象编辑 |
| [Comfy-Org/ComfyUI](https://github.com/Comfy-Org/ComfyUI) | 概念图、材质图、图标和生成工作流调度 |
| [Tencent-Hunyuan/Hunyuan3D-2](https://github.com/Tencent-Hunyuan/Hunyuan3D-2) | 高质量 3D 资产生成实验管线 |
| [Stability-AI/stable-fast-3d](https://github.com/Stability-AI/stable-fast-3d) | 单图快速 3D 粗模 |
| [VAST-AI-Research/TripoSR](https://github.com/VAST-AI-Research/TripoSR) | 快速图生 3D 备选管线 |
| [microsoft/TRELLIS](https://github.com/microsoft/TRELLIS) | 3D 生成备选管线 |
| [mrdoob/three.js](https://github.com/mrdoob/three.js) | 3D 预览 |
| [pmndrs/react-three-fiber](https://github.com/pmndrs/react-three-fiber) | React 内嵌 Three.js 场景 |
| [Unity-Technologies/com.unity.toonshader](https://github.com/Unity-Technologies/com.unity.toonshader) | 3渲2描边、边缘光、Emission、MatCap 规则参考 |
| [donmccurdy/glTF-Transform](https://github.com/donmccurdy/glTF-Transform) | GLB/GLTF 优化、压缩、资产处理 |

## 计划中的项目结构

```text
wushen-forge/
  apps/
    desktop/          # Tauri + React
    agent/            # Python FastAPI + LangGraph
  packages/
    weapon-spec/      # WeaponDesignSpec / JobEvent / UnityMaterial JSON schemas
    unity-export/     # Unity 导出约定和材质映射
  migrations/         # SQLite schema migrations
  workflows/
    comfyui/          # ComfyUI workflow JSON
    blender/          # Blender Python 脚本
  docs/
    QUICKSTART.md
    DESIGN.md
    API.md
    SCHEMAS.md
    DATABASE.md
    FRONTEND.md
    IMPLEMENTATION_PLAN.md
    M1_SKELETON.md
    M2_ASSETSTORE.md
    M3_LLM_AND_CONTRACTS.md
    M3_COMFYUI_ADAPTER.md
    M3_DESKTOP_SUPERVISOR.md
    M4_PATCH_ASSETSTORE.md
    M5_ROUGH3D_PREVIEW.md
    LOCAL_3D_RUNTIME.md
    PACKAGING.md
    PROMPT_QUALITY_SET.md
    UNITY_IMPORT_SMOKE.md
  assets/
    examples/
```

## Agent 工作方式

第一阶段采用自动执行模式：

1. 用户输入文字、草图或参考图。
2. `interpretation` 先提取结构与动作锚点，输出 **2~3 条**重诠释候选（按可执行性和稳定性排序）。
3. 用户确认候选（系统内只允许选一个），固定写入 `CreativeWeaponGraph`。
4. Agent 基于确认结果生成概念图，并保持候选与版本上下文（`creative_graph_id` / `skill_graph_id`）追溯。
5. 局部修改（局部掩模/补丁）后再生成 3D 粗模。
6. 输出 `SkillGraph`（至少 6 个技能卡）与资产清单。
7. 生成可导入 Unity 的粗模资产与预览参数。
8. 资产入库并可追溯回到版本链。

## 生产级设计方向

第一阶段虽然功能闭环很小，但必须按生产级软件设计：

- 桌面端启动后直接进入工作台，不做营销首页。
- Tauri 使用受控文件权限，Python Agent 服务可作为 sidecar 或本地服务运行。
- 前端不直接调用 LLM、ComfyUI 或 3D provider，所有生成任务统一走 FastAPI + LangGraph。
- 长任务提交后立即返回 `job_id`，后续通过事件流展示 Agent 执行过程。
- 资产库采用 SQLite + 本地不可变对象库，所有概念图、mask、prompt、workflow、GLB、导出包都可追溯。
- 3D 粗模管线必须保留原始模型、规范化模型、优化模型、预览截图和 Unity 导出元数据。
- Release 必须通过功能、质量、安全、Unity 导入、license、文档和桌面打包 gate；当前安全边界 gate 是 `npm run release:safety-scope`，secret/file gate 是 `npm run release:secrets-files`，固定 prompt 质量 gate 是 `npm run release:prompt-quality`，文档 walkthrough gate 是 `npm run release:docs-walkthrough`，桌面打包 readiness gate 是 `npm run release:packaging-readiness`，license/SBOM gate 是 `npm run release:license-sbom`，聚合发布入口是 `npm run release:gate`。

## 8 人位子 Agent 分工（上限）

后续每次大设计迭代默认按 8 人位子 Agent 分工讨论，再由主 Agent 收敛到文档和实现计划；复杂 release 审计可以临时增加专家，但总数不超过 8 个子 Agent。

| 分组 | 子 Agent | 职责 |
| --- | --- | --- |
| 前端 | Frontend Agent A | Forge 工作台任务流、3D 展台可用性、资产库交接体验 |
| 前端 | Frontend Agent B | 桌面信息架构、视觉层级、Codex/Claude Code 式 Agent 操作模式 |
| 后端/架构 | Backend Architecture Agent | API contract、任务状态、资产库、provider adapter、Unity export/import 管线 |
| 后端/编排 | Runtime Agent | 3D/图像异步 worker、任务恢复、provider checkpoint、重试策略 |
| 后端/部署 | Packaging/Distribution Agent | Tauri 打包、sidecar、签名/安装器、桌面发布脚本 |
| 数据 | Quality & Safety Agent | 安全边界审计、非制造约束、prompt 风险和质量报告 |
| 数据 | Verification Agent | 自动化测试、Playwright、Unity gate、安全/发布 blocker 与文档证据 |
| 外部依赖 | Provider Specialist | ComfyUI、3D provider、Unity 导入行为和适配协议评估 |

总上限：同时活跃子智能体不超过 8 个。超出时复用 Runtime/Provider/Verification 的同类任务，保持主任务队列单轨推进，避免状态不可控。

### 当前轮角色交付（8 人位）

| 子 Agent | 状态 | 负责范围 | 下轮交付 |
| --- | --- | --- | --- |
| Frontend Agent A | In Progress | Forge、Patch、3D 预览主链路 | 任务流图、交互边界、可复用 Playwright 建议 |
| Frontend Agent B | In Progress | 信息架构、状态文案、界面可读性 | 关键交互文案、视觉优先级、错误状态归一化 |
| Backend Architecture Agent | In Progress | API、资产、provider adapter | contract 差异项、接口兼容性、schema 风险 |
| Runtime Agent | In Progress | Worker、恢复、provider checkpoint | 任务状态机边界、重试/取消失败模式 |
| Packaging/Distribution Agent | In Progress | Sidecar、打包、安装链路 | sidecar 交付清单、release readiness 阻塞项 |
| Quality & Safety Agent | In Progress | 非制造边界、prompt 质量、安全约束 | 安全红线检查、negative prompt 清单 |
| Verification Agent | In Progress | 测试、网关、文档证据 | gate 覆盖缺口、证据归档 |
| Provider Specialist | In Progress | ComfyUI / 3D provider / Unity 导入 | provider 真实运行对比、失败归因与可采纳结论 |

## goal 模式执行节奏（设计闭环）

- 第 1 步：边界确认。确认阶段范围、非制造安全边界、输出边界与子 Agent 并发上限。
- 第 2 步：参考映射。将功能能力映射到 GitHub 参考项目与正式文档。
- 第 3 步：文档更新。同步 README、DESIGN、IMPLEMENTATION_PLAN。
- 第 4 步：证据化。新增/更新 gate、smoke、复盘记录，再把阻塞项放进下一阶段优先级。
- 第 5 步：实现执行。仅在证据项可复用时推进代码切片。

每轮更新都要在 `git` 变更中保留“下一步计划”字段，避免设计演化失联。

## 开源策略

本项目计划开源。由于会调用外部大模型 API、ComfyUI 工作流和 3D 生成模型，所有第三方项目在正式集成前都需要做 license 审核。

建议策略：

- 桌面端、Agent 编排、资产库代码开源
- 大模型 API provider 通过配置接入
- ComfyUI 作为外部服务调用，不直接合入客户端源码
- 3D 模型管线通过 adapter 接入，避免锁死某一个模型

## 状态

当前处于产品设计与架构设计阶段。

当前工程契约：

- [API Contract](docs/API.md)
- [Quickstart](docs/QUICKSTART.md)
- [Schema Contract](docs/SCHEMAS.md)
- [Database Contract](docs/DATABASE.md)
- [Frontend Contract](docs/FRONTEND.md)
- [Implementation Plan](docs/IMPLEMENTATION_PLAN.md)
- [M1 Skeleton Notes](docs/M1_SKELETON.md)
- [M2 SQLite AssetStore Notes](docs/M2_ASSETSTORE.md)
- [M3 LLM and Contract Generation Notes](docs/M3_LLM_AND_CONTRACTS.md)
- [M3 ComfyUI Adapter Notes](docs/M3_COMFYUI_ADAPTER.md)
- [M3 Desktop Agent Supervisor Notes](docs/M3_DESKTOP_SUPERVISOR.md)
- [M4 Patch AssetStore Notes](docs/M4_PATCH_ASSETSTORE.md)
- [M5 Rough 3D Preview Notes](docs/M5_ROUGH3D_PREVIEW.md)
- [Local 3D Runtime](docs/LOCAL_3D_RUNTIME.md)
- [Desktop Packaging](docs/PACKAGING.md)
- [Prompt Quality Set](docs/PROMPT_QUALITY_SET.md)
- [Unity Import Smoke](docs/UNITY_IMPORT_SMOKE.md)
- [SQLite migration](migrations/0001_init.sql)
- [JSON schemas](packages/weapon-spec/schemas/)

当前已完成的工程切片：

- M1: Tauri + React 桌面工作台骨架，FastAPI Agent API 骨架，SSE mock 事件。
- M2: SQLite-backed AssetStore mock，幂等创建，结构化错误 envelope，SSE replay，资产文件 sha256 校验，M2 smoke gate。
- M3 foundation: schema/OpenAPI 类型生成，OpenAI-compatible LLM adapter 边界，WeaponDesignSpec 入库前 JSON Schema gate，ComfyUI mock/HTTP adapter 边界，concept quality report gate，M3 gate。
- M3 desktop supervisor: Tauri dev/local Rust commands 可启动、停止、查询本地 Python Agent，并校验 `/api/health` 身份、清理 managed 子进程、向前端返回 runtime base URL；正式 bundled sidecar 仍未完成。
- M4 patch foundation: 后端 patch job 已接入 AssetStore，并提供武器详情查询、受控资产文件读取、`patch_mask` / `patch_manifest` 本地上传 API；桌面端已有 API-connected Patch Mode 基础，可选择武器/版本、加载源概念图、用画笔或套索绘制 mask，支持画笔尺寸、mask 透明度、撤销/重做，上传 PatchManifest 并提交 patch；成功时创建追加式 patch version、`concept_patch`、`patch_prompt`、`comfyui_workflow` 和 schema-valid `quality_report`，不覆盖旧版本；默认仍可用 mock provider 离线开发，显式设置 `WUSHEN_IMAGE_PROVIDER=comfyui` 时 patch 会上传源图和 mask，并通过 ComfyUI HTTP inpaint workflow 生成 patch 结果；当前 patch 版本可用滑杆对比父版本源图和 patch 结果图，并可设为当前版本、回到父版本或从父版本重试；SQLite 迁移已覆盖旧 M4 库的幂等表、`concept_patch` 角色约束和重复内容资产复用。
- M5 rough 3D foundation: mock 3D 产物已从文本占位升级为最小合法 GLB，`mock_3d` provider 会返回 raw / normalized / optimized GLB、Unity material metadata 和模型质量指标；`POST /api/weapons/{weapon_id}/generate-3d` 已接入，可从 `concept_image` 或 `concept_patch` 追加生成 `rough_3d` 子版本，不覆盖源版本；模型质量报告现在会解析 optimized GLB，记录 triangle/vertex/mesh/material/texture/image counts、PBR material、bounds、center、extents 和 longest axis，资产库检查会把缺失 mesh/bounds 证据标为 blocker、缺失 material slots 标为 warning；桌面端 `Preview3DPanel` 会读取资产库、定位当前或最近的 `rough_raw_glb`，通过受控 asset file URL 用 Three.js 加载，并展示为“展览台 + 简单角色 + 手持武器”的 360 度预览场景，支持自动旋转、拖拽旋转、toon/solid/wireframe、重置视角、截图，并可从当前概念图或 patch 图一键发起 generate-3d job，接回全局任务时间线；`POST /api/weapons/{weapon_id}/export-unity` 已可生成 `unity_export_package` ZIP 快照，包含优化 GLB、Unity material、weapon spec、质量报告和非制造说明，桌面端可从 3D 展台面板触发导出并显示下载链接；资产库页面已可按武器/版本展示概念图、patch、GLB、Unity material、质量报告和 Unity ZIP，并通过受控 asset URL 下载交接文件；`scripts/smoke_m5_unity_import.py` 会生成临时 export 包并做 Unity ZIP/manifest/GLB preflight，若配置了 `WUSHEN_UNITY_EXECUTABLE` 或 `UNITY_EXECUTABLE` 则进一步创建临时 Unity 项目、安装 glTFast 并用 batchmode 验证导入；当前本机未配置 Unity 时会记录 `UNITY_EXECUTABLE_NOT_CONFIGURED` release blocker；资产库检查会把非法 GLB、不完整 generate-3d job 和 malformed export 包标为 blocker；`npm run m5:gate` 已覆盖 M4 gate、GLB contract smoke、generate-3d HTTP smoke、Unity export HTTP smoke、desktop runtime/handoff browser smoke 和 Unity import preflight/batchmode smoke。
- P0 desktop context foundation: `App` 现在统一持有当前 weapon/version/job 上下文，Forge、Patch、资产库、Inspector 和 3D 展台通过同一 active weapon/version 工作；job restore 和 terminal event 会优先切到该 job 的 `outputs.current_version_id`，保证 create / patch / generate-3d / export-unity 自动流程完成后 Inspector、top bar、3D 和资产库都跟随新输出版本；Patch Mode 的 mask canvas 现在会在 source asset/ref 就绪后按源图真实尺寸幂等初始化，避免上传尺寸漂移。非 Patch 主舞台会显示当前源图、版本、3D 粗模状态、Unity 导出状态和安全边界，Inspector 顶部显示 weapon id、version id、model id 和可用资产角色。浏览器证据见 `output/playwright/p0-unified-context-workbench.png`、`output/playwright/p0-library-selection-sync.png`、`output/playwright/p0-context-patch-brush.png`、`output/playwright/p0-context-patch-comparison.png`、`output/playwright/p0-context-3d-handoff.png`、`output/playwright/p0-context-library-sync.png`。
- P0 Agent trace and job action foundation: 底部 `JobTimeline` 已从平铺事件列表升级为可恢复 Agent 轨迹抽屉，支持 `GET /api/jobs/{job_id}` 水合历史事件、最近 job 自动恢复、SSE `after` 续订、按公开 `seq` 排序、按 step 分组的中文阶段卡、进度条、metadata/asset 展示，以及按状态启用的请求重试、从失败步骤重试、请求取消、打开设置和跳过 3D 操作。任务中心也复用同一 trace 视图。后端已持久化 job action request：`job_actions` 审计、job 状态更新、step attempt/cancel 状态、追加 action event，并由 `agent:p0-job-actions-smoke` 覆盖。浏览器证据见 `output/playwright/p0-jobtimeline-success-trace.png` 和 `output/playwright/p0-jobtimeline-task-center.png`。
- P0 job action-state browser coverage: `npm run desktop:p0-job-action-state-smoke` 已覆盖 failed rough3d job 的任务重试、从失败步骤重试、waiting_provider job 的取消请求、manual recovery 后 waiting_user job 的重试动作；脚本断言按钮启用/禁用、中文状态、runtime recovery/cancel 文案、provider task cancel 状态、action response 和截图产物。`GET /api/jobs/{job_id}/runtime` 现在不再把 `retrying` 标为 resumable，避免 UI 暴露会被后端 409 拒绝的二次重试入口。截图见 `output/playwright/p0-job-trace-failed-retry.png`、`output/playwright/p0-job-trace-retry-from.png`、`output/playwright/p0-job-trace-waiting-provider-cancel.png`、`output/playwright/p0-job-trace-recovered-waiting-user.png`。
- P0 Task Center history and audit: 后端新增 `GET /api/jobs` 历史任务读模型和 `GET /api/jobs/{job_id}/actions` action 审计读模型，支持 query/status/job_type/error_code/cursor/limit，并通过 `0007_p0_job_history_indexes.sql` 增加只读索引；`GET /api/jobs/{job_id}` 现在会填充结构化 `error`。桌面端 `/jobs` 已升级为主工作区 `JobCenterPanel`，支持历史 job 搜索、状态过滤、失败原因过滤、手动 job id 恢复、最近任务唤醒、终态任务本机通知记录、选中 job 详情、Runtime、失败原因和 action 审计列表；筛选条件和最近任务会保存到本机，下次打开任务中心自动恢复；action 审计中的 event id 可一键定位并高亮对应 Agent Timeline step。查看历史不会自动切换工作台上下文，只有“恢复到工作台”、手动恢复、最近任务唤醒或通知记录打开任务才会订阅为 active job。该切片由 `agent:p0-job-history-search-smoke` 和 `desktop:p0-job-center-history-smoke` 覆盖，并已纳入 `npm run m5:gate`。浏览器证据见 `output/playwright/p0-job-center-history.png`。
- P0 runtime recovery metadata foundation: SQLite 迁移 `0006` 已加入 `provider_tasks`、`job_checkpoints`，并给 `generation_jobs` / `job_steps` 增加 runner lease、checkpoint、cancel intent、provider task、cancel state 字段；后端提供 `GET /api/jobs/{job_id}/runtime` 和 `POST /api/runtime/recover`，应用启动会把中断的 active job 暂停为 `waiting_user` 并追加 recovery event；取消动作会对当前 rough3d provider task 发起 cancel，并把 provider task 记录为 `cancel_requested` 或 `cancelled`；SSE unknown `after` cursor 会显式返回 `INVALID_EVENT_CURSOR` 事件；前端恢复 job 时会用该 job 的事件替换轨迹，避免混入旧 job。该切片由 `agent:p0-runtime-recovery-smoke` 覆盖，并已纳入 `npm run m5:gate`。
- P0 generate-3d provider runtime boundary: opt-in generate-3d worker 已从阻塞式 mock 调用升级为 `submit_rough_model` / `poll_rough_model` / `fetch_rough_model` / `cancel_rough_model` Provider 边界；默认 mock 仍可立即成功以保持 M5 兼容，设置 `WUSHEN_MOCK_3D_POLL_SEQUENCE=polling,succeeded` 可模拟真实异步 Provider。Worker 会在 `polling/submitted/unknown` 时保持 `waiting_provider`，不提交 rough_3d 版本、不写 GLB 资产；后续 worker tick 继续 poll，成功后 fetch 并提交 raw/normalized/optimized GLB、Unity material 和质量报告；取消会调用 provider cancel，late output 不会写入资产库。该边界由 `agent:p0-provider-runtime-boundary-smoke` 覆盖，并已纳入 `npm run m5:gate`。
- P0 local HTTP 3D provider adapter: 后端新增 `local_http_3d` adapter，可通过 `WUSHEN_3D_PROVIDER=local_http` 和 `WUSHEN_3D_HTTP_BASE_URL=http://127.0.0.1:PORT` 接入本地 Stable Fast 3D、TripoSR、Hunyuan3D 或自研 3D runtime 包装服务。协议固定为 `POST /v1/rough-models`、`GET /v1/rough-models/{task}`、`GET /v1/rough-models/{task}/result`、`POST /v1/rough-models/{task}/cancel`，结果用 base64 GLB 和 Unity material JSON 交接；adapter 会校验 GLB header，不接受非 GLB 伪结果。该切片由 `agent:p0-local-http-3d-provider-smoke` 覆盖，并已纳入 `npm run m5:gate`。
- P0 local 3D runtime wrapper: 新增 `scripts/wushen_local_3d_runtime.py`，作为独立 HTTP 子进程实现 Wushen local 3D 协议，避免把模型权重、GPU 依赖和模型进程崩溃放进桌面 Agent。当前支持 deterministic `mock` backend、调用本地 Stable Fast 3D `run.py` 的 `sf3d-cli` backend 路径，以及调用本地 TripoSR `run.py --model-save-format glb` 的 `triposr-cli` fallback 路径；`agent:p0-local-3d-runtime-wrapper-smoke` 会启动真实 wrapper 子进程，驱动 Agent worker 完成 submit -> waiting_provider -> fetch -> GLB 入库，并验证 cancel 不产生 late assets。`agent:p0-local-3d-runtime-sf3d-manual` 和 `agent:p0-local-3d-runtime-triposr-manual` 是不进默认 gate 的真实模型手动验收入口，安装和验收步骤见 [Local 3D Runtime](docs/LOCAL_3D_RUNTIME.md)。
- P0 async generate-3d worker loop: 默认 M5 路径仍保持同步，保证现有开发 gate 和桌面体验稳定；显式设置 `WUSHEN_GENERATE3D_WORKER=1` 时，`POST /api/weapons/{weapon_id}/generate-3d` 只创建 `queued` job，不立即创建 `rough_3d` 版本或 GLB 资产，FastAPI 启动时会拉起本地后台 Worker 自动领取 queued/retrying/waiting-provider generate-3d job，写 runner lease、provider task、checkpoint、worker events，并在 provider 成功后提交一个 rough_3d 子版本、GLB variants、Unity material 和质量报告；`POST /api/runtime/work-once` 保留为本地/测试单步领取钩子。该 opt-in 路径由 `agent:p0-async-generate3d-worker-smoke` 和 `agent:p0-generate3d-worker-loop-smoke` 覆盖，并已纳入 `npm run m5:gate`。当前仍未完成真实 SF3D/TripoSR/Hunyuan3D 模型环境验收，也未完成任意 step 的通用 checkpoint resume。
- P0 async export-unity worker loop: 默认 Unity export HTTP 路径仍同步完成以保持兼容；显式设置 `WUSHEN_EXPORT_UNITY_ASYNC=1` 时，`POST /api/weapons/{weapon_id}/export-unity` 只创建 queued job，不提前创建 export version、`export_packages` 行或 ZIP asset；`POST /api/runtime/work-once` 可领取并完成 export_unity job。设置 `WUSHEN_EXPORT_UNITY_WORKER=1` 或 `WUSHEN_RUNTIME_WORKER=1` 时，FastAPI 启动本地后台 Worker 自动领取 export_unity job。该路径由 `agent:p0-export-unity-worker-smoke` 覆盖，并已纳入 `npm run m5:gate`。
- P0 frontend runtime and Unity handoff visibility: `App` 现在会随 active job 拉取 `GET /api/jobs/{job_id}/runtime`，`JobTimeline` 在事件轨迹上方显示 runtime mini panel，包括 provider task、checkpoint、resumable/cancellable 和 last seen；3D 展台新增 Unity 交接状态卡，保守展示 raw/normalized/optimized GLB、Unity material、quality report、export ZIP、model id、质量状态、fallback 和旧 ZIP 风险，并在同一卡片内展示 parsed GLB 模型质量证据：triangles、meshes、vertices、materials、textures、longest axis、center/extents、PBR 和 bounds 状态；同一卡片也展示 Unity 轴向/尺度策略，包括 forward axis、long axis、pivot、fallback pivot 和 `normalized_game_asset_scale`，明确这是游戏资产相对比例而非现实尺寸。资产库每个版本卡新增 Unity handoff checklist、质量徽标、版本溯源摘要、快速预览、版本级下载动作和“查看生成轨迹”入口：当前模型 report 会显示 `QC passed/warning/blocker`、blocker/warning 数量、triangles、materials 和 bounds 状态；旧版本或非模型版本会保守显示 report present/missing/not applicable，避免把“存在 ZIP”误说成“当前版本可交接”；版本溯源会显示 job id、root/parent v 来源、创建时间和该版本输出的资产角色；快速预览优先展示 concept/patch 图片缩略图，非图片版本显示 GLB/ZIP 摘要；资产行的 JSON/GLB/ZIP 预览抽屉会通过受控 asset file URL 读取文件，JSON 展示 schema、顶层 keys、字节数和截断正文，GLB 展示 header、chunk、mesh/material/texture/node 计数、generator 和 BIN 摘要，Unity ZIP 展示 central directory、deflated manifest.json、package root、payload counts、relative path safety 和 manifest coverage；版本级下载按钮通过受控 asset file URL 批量下载当前版本文件，导出版本额外提供 Unity ZIP 直链和“打开 ZIP 位置”动作；生成轨迹入口会恢复该版本的 job、切到任务中心并展示事件、runtime 和 action 审计。`POST /api/assets/{asset_id}/reveal` 会在本地 Agent 内部按 asset id 校验对象库 containment 与 sha256 后调用系统文件管理器，不把本地绝对路径返回给前端；`dry_run=true` 用于自动化验收。资产库详情顶部新增版本 DAG 条带，按 v 号展示 concept / patch / rough_3d / export 节点、root/parent v 关系，并可点击切换上下文版本。桌面端新增轻量 hash route：`#/jobs/:jobId` 会直接打开任务中心并恢复该 job 的事件、runtime 和 action 审计；`#/weapons/:weaponId/versions/:versionId` 会直接打开资产库并恢复指定版本上下文；普通导航会写回 `#/forge`、`#/library` 等状态。`npm run desktop:p0-runtime-handoff-smoke` 会用临时 Agent/Vite、mock providers 和 system Chrome 复现 generate-3d worker -> export-unity worker -> recent job restore，断言 runtime/handoff DOM、模型质量证据、轴向/尺度策略、资产库质量徽标、版本 DAG、版本溯源、快速预览、批量下载动作、受控 asset file 链接、资产库跨版本 handoff 覆盖，以及 3D canvas 非空和拖拽 checksum 变化；`npm run desktop:p0-context-continuity-smoke` 会通过真实 UI 操作覆盖 Forge create -> Patch mask -> patch job -> generate-3d from concept_patch -> export-unity -> Library sync，并校验请求体中的 source version/image、model id、版本父子链、版本 DAG、版本溯源、快速预览、JSON/GLB/ZIP 预览抽屉、批量下载动作、asset links 和从资产库恢复 export job 轨迹；`npm run desktop:p1-deeplink-smoke` 会冷启动 `#/weapons/:weaponId/versions/:versionId` 和 `#/jobs/:jobId`，验证 Library/Task Center 上下文恢复和普通导航 hash 写回；`npm run agent:p1-asset-reveal-smoke` 会验证 reveal dry-run 不打开系统窗口、不泄露本地路径，并已纳入 `npm run m5:gate`。

下一步建议：

1. 由 Backend Architecture Agent 先冻结 `CreativeWeaponGraph@1`、`SkillGraph@1`、`structure_interpretation` 与 `recast/confirm` 的文档合同，保持 `WeaponDesignSpec@1` / `weapon_family` 仅作兼容说明。
2. 由 Frontend Agent A/B 补齐结构解释选择 UI、4 个自由度滑块、候选重试和未确认阻断规则，确保用户入口不回退到武器类别。
3. 由 Quality & Safety Agent 扩充非武器对象 prompt 集，验证 `2~3` 候选、`combat_affordances` 分叉、稳定排序和非制造边界。
4. 由 Provider Specialist 在结构合同冻结后继续跑通真实 Stable Fast 3D 与 TripoSR，对比 GLB 质量、显存、失败模式和 license 风险。
5. 由 Runtime Agent 输出 generate-3d 与 export-unity 的恢复模型：明确 `provider_task_id` 持久化重试、超时重试、cancel 探测规则，并补齐对应 smoke 覆盖。
6. 由 Packaging Agent 与 Verification Agent 接上 sidecar 打包和 Unity 真实验收，让 `release:packaging-readiness` 与 `unity:import:gate` 进入可发布证据链。

## 生产级交付门禁矩阵

只有当下列门禁全部清零，且有对应证据可复现时，才允许进入下一阶段（包括新的实现切片或主要里程碑）。

| 执行记录ID | 门禁 | 目标状态 | 触发阻塞 | 当前状态 | 负责人 | 证据路径 | 失败归档 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| GATE-01 | 安全边界 | `npm run release:safety-scope` 通过 | 非制造内容、泄露风险、目录越权 | 部分通过（文档边界已写入） | Quality & Safety / Verification | `scripts/check_release_safety_scope.py`、`docs/API.md`、`docs/DESIGN.md` | `scope_violation`、`non_manufacturing_drift` |
| GATE-02 | 秘钥与文件安全 | `npm run release:secrets-files` 通过 | 明文密钥、非法外泄文件路径、绝对路径入库 | 待排查 | Verification / Packaging | `scripts/check_release_secrets_files.py`、`apps/desktop/src-tauri/tauri.conf.json`、`apps/agent/wushen_agent/asset_store.py` | `secret_literal`、`path_leak` |
| GATE-03 | Prompt 质量门 | `npm run release:prompt-quality` 通过 | 质量报告缺失、negative prompt 未覆盖风险词 | 待补齐 | Quality & Safety | `scripts/check_release_prompt_quality.py`、`docs/PROMPT_QUALITY_SET.md` | `prompt_coverage_gap`、`negative_prompt_missing` |
| GATE-04 | 文档可复现性 | `npm run release:docs-walkthrough` 通过 | 关键流程无可复现脚本或证据缺失 | 待补齐 | Backend Architecture / Verification | `scripts/check_release_docs_walkthrough.py`、`docs/QUICKSTART.md`、`docs/API.md` | `walkthrough_gap`、`endpoint_mismatch` |
| GATE-05 | 打包就绪 | `npm run release:packaging-readiness` 通过 | Tauri 打包 pipeline、sidecar 二进制、签名/资源项缺失 | 待处理 | Packaging/Distribution | `scripts/check_release_packaging_readiness.py`、`docs/PACKAGING.md`、`apps/desktop/src-tauri` | `sidecar_binary_missing`、`externalbin_mismatch` |
| GATE-06 | License/SBOM | `npm run release:license-sbom` 通过 | 未知许可项或 SBOM 缺失 | 待确认 | Packaging / Verification | `scripts/check_release_license_sbom.py`、`docs/THIRD_PARTY_LICENSES.md` | `license_forbidden`、`lockfile_missing` |
| GATE-07 | 3D provider 真实对比 | `agent:p0-local-3d-runtime-sf3d-manual` 与 `agent:p0-local-3d-runtime-triposr-manual` 有结果 | 无法稳定输出可用 raw/normalized/optimized 模型 | 待真实验收 | Provider Specialist | `scripts/smoke_p0_local_3d_runtime_sf3d_manual.py`、`scripts/smoke_p0_local_3d_runtime_triposr_manual.py`、`docs/LOCAL_3D_RUNTIME.md` | `backend_install`、`no_glb_output`、`invalid_glb` |
| GATE-08 | 任务恢复能力 | `agent:p0-runtime-recovery-smoke` / `agent:p0-generate3d-worker-loop-smoke` 通过 | provider task 重试、cancel、checkpoint 恢复异常 | 待补齐 | Runtime | `scripts/smoke_p0_runtime_recovery.py`、`scripts/smoke_p0_generate3d_worker_loop.py`、`scripts/smoke_p0_provider_runtime_boundary.py` | `cursor_invalid`、`cancel_conflict` |
| GATE-09 | 运行时恢复边界 | `GET /api/jobs/{job_id}/runtime` 与 runtime action 映射一致 | unknown cursor、cancel 409、超时状态不一致 | 基础有，但未闭环 | Runtime / Backend Architecture | `scripts/smoke_p0_runtime_recovery.py`、`apps/agent/wushen_agent/main.py`、`apps/agent/wushen_agent/asset_store.py` | `runtime_action_mismatch`、`cancel_not_propagated` |
| GATE-10 | Unity 导入验证 | `npm run unity:import:gate` 由 `blocked_unity_not_configured` 转 `imported` | 环境缺失或导入失败 | 阻塞（待本机/CI 配置） | Verification / Provider Specialist | `scripts/smoke_m5_unity_import.py`、`docs/UNITY_IMPORT_SMOKE.md` | `unity_not_configured`、`unity_import_failed` |

说明：门禁证据默认归档到 `output/release/<执行记录ID>/`，包含 `report.json`、`trace.txt`、`artifacts.txt`，并用本表中的失败归档代码进行分类归档，方便 `Implementation Plan` 与 `Design` 联动追踪。

证据模板：

- `output/release/README.md`
- `output/release/_TEMPLATE/report.json`
- `output/release/_TEMPLATE/trace.txt`
- `output/release/_TEMPLATE/artifacts.txt`

门禁执行规则：

- 本轮任一门禁为 blocker 时，不推进到下一阶段；仅将对应项升级为下轮执行目标。
- 本轮建议只覆盖“可复现、可验证、可回滚”的项，不做不可验证的范围扩展。
- 若 1~2 项阻塞可以并行拉起，超过 3 项阻塞则先完成 `structure-first docs + prompt-quality + safety-scope`，再并行 provider/runtime/packaging/Unity。

## 第一阶段产品决策冻结（与 DESIGN.md §16 对齐）

### API 与模型服务

- LLM：默认使用 OpenAI-compatible API；provider profile 支持 `openai` / `deepseek` / `claude` 映射到统一适配层。
- ComfyUI：默认外部本地服务，用户提供 endpoint；不在主应用内捆绑可执行文件。
- 3D 粗模：优先采用 `local_http` provider 抽象链路，先稳定一个轻量 provider 输出（raw/normalized/optimized）闭环。

### 开源与交付边界

- 核心代码与文档以 MIT / Apache-2.0 为主线；GPL/AGPL 及高风险许可仅允许外部服务/工具链边界隔离。
- Windows 为第一阶段发布优先目标平台（面向 Steam 场景），macOS/Linux 随发布阶段并行补齐。

这些决策已在 `docs/DESIGN.md` 与 `docs/IMPLEMENTATION_PLAN.md` 的门禁行动中形成验证要求。
