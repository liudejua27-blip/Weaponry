# ForgeCAD Agent：GitHub 参考、采用边界与目标架构

版本：2026-07-15
状态：参考决策；不是依赖清单

2026-07-14 已再次用 GitHub connector 核验 `openai/codex` 的 app-server/skill loader、`KittyCAD/modeling-app` 的场景/状态结构、`KhronosGroup/glTF` 的材质扩展以及 `donmccurdy/glTF-Transform` 的 inspect/validate/优化入口；同时阅读 DeepSeek 与 Claude Code 官方文档。该检查不等同于许可证、release、安全公告或二进制来源审计。结论仍是“参考小而稳定的模式，保留现有技术栈”，不 fork、不整套复制、不因为参考项目存在就宣称 ForgeCAD 已具备对应能力。

## 1. 已核验的参考项目

| 项目 | 已观察事实 | ForgeCAD 借鉴 | 明确不套用 | 决策 |
| --- | --- | --- | --- | --- |
| [OpenAI Codex](https://github.com/openai/codex) | app-server 明确建模 Thread/Turn/Item，Turn 与 Item 通过事件流增量更新；core-skills 从 `SKILL.md` 加载 | 任务生命周期、状态事件、可取消 Turn、技能渐进加载、工具结果与批准 | 通用 shell、代码库权限、原始推理展示和完整 Rust 运行时 | 核心 Agent 交互参考 |
| [Claude Code 官方文档](https://code.claude.com/docs/en/overview) | 专用 subagent 有独立上下文/工具/权限；Skill 使用 `SKILL.md` 与支持文件；hook 可在 tool 前后允许、拒绝或升级 | 专属 Skill、工具最小化、研究上下文隔离、生命周期检查 | 终端 coding agent、任意 Bash/MCP、把开发者权限语义原样给零基础用户 | 产品 Skill/Action Loop 参考，不引入运行时 |
| [OpenCode](https://github.com/anomalyco/opencode) | 开源 Agent，提供 plan/build 权限区分和桌面 Beta，MIT | 只读规划与可写执行分离、Provider 配置分层 | 多用途 coding agent、终端 UI、无限扩展生态 | 辅助权限模型参考 |
| [goose](https://github.com/aaif-goose/goose) | 桌面/CLI/API 通用 Agent，支持大量 Provider 与 MCP 扩展，Apache-2.0 | Provider port、扩展隔离和诊断思想 | 15+ Provider、70+ 扩展、通用自动化平台 | 只参考边界，不引入运行时 |
| [Zoo Design Studio](https://github.com/KittyCAD/modeling-app) | CAD 模型以人类可读代码表示，GUI 操作修改同一表示 | code-as-model、状态机和 GUI/程序表示统一 | 云几何引擎、视频流视口、KCL 全语言 | ShapeProgram 语义参考 |
| [Aider](https://github.com/Aider-AI/aider) | LLM 变更结合 diff、Git、lint 和 test | 变更可检查、失败可见、验证后交付 | 自动 Git 提交和面向开发者的终端体验 | ChangeSet 体验参考 |
| [Tauri](https://github.com/tauri-apps/tauri) | Rust 后端加系统 WebView，支持跨平台打包，MIT/Apache-2.0 | 继续作为桌面壳、密钥与 sidecar 边界 | Electron 或第二套桌面框架 | 已采用，继续维护 |
| [three.js](https://github.com/mrdoob/three.js) | 轻量通用 WebGL/WebGPU 3D 库 | 单一 renderer、GLB 主视图、选择和预览 | 为每张资产卡创建 renderer | 已采用，保持单视图 |
| [Three.js CSS3DRenderer](https://threejs.org/docs/pages/CSS3DRenderer.html) | CSS3D 只对普通 DOM 元素应用 3D 变换，不能使用 WebGL 材质/几何系统 | 说明标签、说明卡或概念演示的边界 | 把 HTML 六面当实体网格、PBR、UV 或 GLB 真值 | 仅 UI 参考，不作为模型路线 |
| [Three.js ExtrudeGeometry](https://threejs.org/docs/pages/ExtrudeGeometry.html) | 将 2D Shape 沿深度或路径挤出，并支持受限 bevel/UV 行为 | `ProfileSketch` 预览语义、挤出参数和失败 fixture 参考 | 把前端 Three.js geometry 变成服务端资产真值 | G821 语义参考，运行时仍由 Worker 负责 |
| [Three.js LoftGeometry](https://threejs.org/docs/pages/LoftGeometry.html) | 以一组有序截面生成连续曲面 | 截面排序、统一重采样、尺寸/方向变化的交互预览参考 | 未经独立 runtime/readback Gate 就宣传 Loft 已实现 | G822 语义参考，需独立验证 |
| [JSCAD](https://github.com/jscad/OpenJSCAD.org) | 模块化浏览器/CLI 参数化 2D/3D JavaScript 工具，MIT | primitive、transform、boolean、array 的确定性语义 | 执行模型生成的任意 JavaScript、替换 Python 服务 | 语言设计参考 |
| [Manifold](https://github.com/elalish/manifold) | 面向 manifold triangle mesh 的可靠布尔库，有 Python 与 JS/WASM 包，Apache-2.0 | 稳健布尔、实体输出和材料属性追踪 | P0 未评测就直接进安装包 | G8 候选，先 benchmark |
| [Trimesh](https://github.com/mikedh/trimesh) | Python 网格读写、分析和 GLB/GLTF 支持，核心仅依赖 NumPy，API 需固定版本 | bounds、watertight、退化、场景读写和导出检查 | 当作 B-Rep CAD 内核 | G8/G9 候选，固定版本后采用 |
| [glTF Transform](https://github.com/donmccurdy/glTF-Transform) | Node/Web glTF 2.0 读写、优化和可复现变换，MIT | 导出后的 prune/dedup/压缩评估 | 在几何生成前引入第二权威模型 | 导出管线候选 |
| [Khronos glTF](https://github.com/KhronosGroup/glTF) | glTF 2.0 定义 metallic-roughness、normal/occlusion/emissive；Khronos 扩展提供 clearcoat 与 KTX2/BasisU | 视觉材质互操作、汽车漆涂层、GPU 纹理压缩和 readback 合同 | 把格式/扩展支持冒充纹理已生效或真实材料 | 高真实度材质合同参考 |
| [Khronos glTF-Validator](https://github.com/KhronosGroup/glTF-Validator) | 对 glTF 2.0/GLB、引用、buffer、image 和 extension 输出 JSON 报告 | Agent GLB 与导入 GLB 的标准合规门 | 只验证格式就声称模型质量通过 | 验证门候选 |

观察事实来自各项目当前默认分支；“借鉴/不套用/决策”是 ForgeCAD 的产品推断，不是上游项目承诺。

## 2. 不应作为主线的项目

| 类别 | 例子 | 原因 |
| --- | --- | --- |
| 完整通用 Agent Runtime | OpenHands、LangGraph、多 Agent 平台 | 增加部署、状态和权限复杂度；ForgeCAD P0 只有一个领域 Orchestrator |
| 神经 3D 本地模型 | TripoSR、Stable Fast 3D、Hunyuan3D、TRELLIS | 权重、GPU、显存、许可证和首次安装成本不符合轻量零基础目标 |
| 用户必装 DCC | Blender 插件、Unity Package | 专业往返可以后置，但不能成为第一次生成和编辑的前置条件 |
| 完整工程 CAD 内核 | OCCT/CadQuery 全栈 | P0 是可编辑机械概念 Mesh，不是 B-Rep、STEP、DFM 或认证软件 |

## 3. 为什么不直接套用桌面 Agent

通用 coding agent 的权威对象是文件、命令、代码仓库和外部工具；ForgeCAD 的权威对象是 Project、AgentAssetVersion、Selection、AssemblyGraph、ShapeProgram、MaterialPreset、QualityReport、Export 和 ChangeSet。

因此复用粒度固定为：

- 参考 thread/turn/item、计划/执行、批准、取消与恢复；
- 保留 Tauri + React + FastAPI + SQLite + Three.js；
- 自己定义受限 CAD 工具和不可变资产版本；
- 不给模型通用 shell、Python、JavaScript、URL 或任意文件系统权限。

## 4. 目标架构

```text
Tauri + React CAD Workbench
├── 单一 Agent 会话与步骤
├── 左上 mini / 中央 focus 共用单一 Three.js canvas
├── 当前选择的简单编辑卡
└── 预览/确认/检查/导出抽屉
          │ HTTP + SSE
          ▼
ForgeCAD Agent API
├── Thread / Turn / Item / Approval
├── DeepSeek/OpenAI-compatible Provider Gateway
├── constrained Agent Action Loop
├── Product Skill Registry
├── Tool Registry + Runtime Policy + Budget
├── internal Candidate Evaluation / Best Selector
├── Domain Pack Registry
├── ActiveDesignSnapshot
└── ChangeSet preview / confirm
          │
          ▼
Restricted Geometry Worker
├── ShapeProgramRuntimeManifest + validator
├── primitive / ProfileSketch / extrude / revolve / loft / sweep
├── transform / array / mirror / bounded edge finish
├── optional Manifold boolean benchmark
├── surface provenance / UV0 / tangent / Material Zone
├── Trimesh inspect/export candidate
└── GLB writer + glTF validation candidate
```

Codex 的重要模式是生命周期和可检查动作，不是界面外观复刻；Claude Code 的重要模式是专用上下文、Skill 支持文件和工具前后策略，不是把 shell/MCP 交给产品用户；Zoo 的重要模式是 GUI 与代码模型共享表示，不是使用其云引擎。ForgeCAD 所有采用均落到自身的 Snapshot、ShapeProgram、ChangeSet 和单 renderer 边界。

## 4.1 DeepSeek 官方合同结论

- Base URL 使用 `https://api.deepseek.com`，当前模型可使用 `deepseek-v4-pro`/`deepseek-v4-flash`；不能继续用过时模型名猜测故障；
- API 本身无状态，多轮请求必须由 ForgeCAD 携带完整必要 history；稳定前缀可利用上下文缓存；
- JSON Output 要求 `response_format=json_object`，prompt 明确要求 JSON 并给出输出示例；官方说明仍可能出现空 `content`，必须作为独立错误处理；
- thinking 模式可配合 Tool Calls，但后续工具子请求必须续传对应 `reasoning_content`，否则会 400；
- 400/401/402/422/429/500/503 必须分别映射，不能全部显示“暂时无法连接”。

官方入口：[DeepSeek API 文档](https://api-docs.deepseek.com/zh-cn/)、[JSON Output](https://api-docs.deepseek.com/zh-cn/guides/json_mode)、[工具调用](https://api-docs.deepseek.com/zh-cn/guides/tool_calls)、[思考模式](https://api-docs.deepseek.com/zh-cn/guides/thinking_mode)、[错误码](https://api-docs.deepseek.com/zh-cn/quick_start/error_codes)。

## 5. 分阶段采用

### 现在保留

- Tauri、React、FastAPI、SQLite、Three.js；
- 当前 OpenAI-compatible Provider port；
- ShapeProgram 白名单和 ChangeSet 不可变版本；
- 单一 Orchestrator，不引入多 Agent 框架。

### G8 几何扩展前必须完成

1. 用四领域 fixture 对 JSCAD 语义、Manifold Python/WASM 和现有 Python worker 做小型 benchmark；Profile/Loft/Sweep 还需覆盖截面闭合、自交、frame 翻转和表面 provenance；
2. 记录安装体积、冷启动、内存、构建复杂度、确定性和失败诊断；
3. 只选一条生产几何执行路线；
4. 固定版本和许可证，更新 lock、SBOM 与 THIRD_PARTY_LICENSES；
5. 保留 ShapeProgram 合同，使底层实现可替换。

`ProfileSketch@1`、`ProfileSectionSet@1`、Extrude/Revolve、Loft、Sweep、布尔和表面完成必须分别进入原子任务。一个参考 API 存在不等于 ForgeCAD 当前支持该操作。Manifold benchmark 必须在集成前完成，Python 与 WASM 不能同时作为默认生产真值。

### 导出质量阶段

先用 Khronos glTF-Validator 产生标准报告，再评估 glTF Transform 的 prune/dedup/压缩。任何优化必须保留 node/part/material ID 映射和输出 hash，不得破坏继续编辑。

## 6. 采用否决门

候选项目只要触发任一项，就不能进入默认安装包：

- 需要用户自行安装 Python 环境、GPU 驱动、模型权重或 DCC；
- 引入第二套 Project/Version/Selection 真值；
- 允许模型执行任意代码或访问任意路径；
- 四领域 fixture 无法确定性复现；
- 安装体积、冷启动或内存超过已批准预算；
- 许可证、NOTICE、二进制来源或商用条款不清楚；
- 无法在 macOS 与 Windows 发布机稳定打包。

## 7. 研究更新规则

GitHub 项目会变化。每次真正采用前重新核验默认分支、release、许可证、维护状态和安全公告；本文的链接是设计证据，不是永久许可证结论。参考项目不得进入 SBOM，只有实际安装、打包或派生的依赖才进入 [THIRD_PARTY_LICENSES](THIRD_PARTY_LICENSES.md)。

采用记录必须写入任务交付：仓库 URL、精确 tag/commit、许可证与 NOTICE、安装体积、冷启动/峰值内存、macOS/Windows 打包结论、四领域 fixture 结果和移除方案。任何一项缺失时，只能保留为“参考”，不能进入默认安装包。
