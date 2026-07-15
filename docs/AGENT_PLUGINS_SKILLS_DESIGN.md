# ForgeCAD 插件与 Skill 操作设计

版本：2026-07-15
目标：零基础用户不安装插件；开发者按任务选择 Codex 插件/Skill；产品内 Skill 由 Agent 自动调用

## 1. 三层概念不能混用

| 层 | 使用者 | 是否进入安装包 | 作用 |
| --- | --- | --- | --- |
| Codex 开发插件/Skill | 开发者和后续 Codex | 否 | 检索、设计审查、代码、测试、发布 |
| ForgeCAD 内置 Skill | 产品内 Agent | 是，作为版本化策略 | 把自然语言意图路由到受限 CAD 工具 |
| 外部扩展 | 专业团队 | P0 否 | 未来 DCC、团队审批或云存储适配 |

用户首次启动只配置大模型 Provider，不选择 Skill、Mode、Agent、MCP、几何内核或 pipeline。

## 2. 后续 Codex 的插件/Skill 选用表

| 任务 | 首选插件/Skill | 何时使用 | 禁止误用 |
| --- | --- | --- | --- |
| GitHub 仓库、Issue、PR、CI | `@github` / `github:github`、`github:gh-fix-ci`、`github:gh-address-comments` | 核验上游、查看仓库、修 CI 或处理 PR 评论 | 未经要求不自动 push；不以 star 代替技术评估 |
| 产品路径和零基础体验 | `@product-design` / `product-design:audit`、`product-design:research` | 有当前界面时审查流程；研究真实摩擦时引用来源 | 没有视觉目标时不直接重做 UI；研究结论不得冒充实现 |
| 视觉方案探索 | `product-design:ideate` | 用户要求新视觉方案，且已明确目标/结果 | 必须先提供三种可见方案并等用户选择，不能直接改生产 UI |
| 按截图实现与核对 | `product-design:image-to-code`、内部 `design-qa` | 用户已选择截图/Figma/方案后实现和比对 | 不从文字凭空发明视觉系统 |
| 文档重构 | `documents:documents` | README、操作、设计、发布和交接文档 | Markdown 不伪装成已渲染 DOCX；文档只写真实能力 |
| React 性能与结构 | `build-web-apps:react-best-practices` | 拆分工作台、状态订阅、bundle 和渲染优化 | 不借机改成 Next.js 或重做技术栈 |
| 前端测试调试 | `build-web-apps:frontend-testing-debugging`、`playwright` | 修复工作台 E2E、浏览器回归 | 浏览器 smoke 不能替代 Tauri 原生测试 |
| Web 3D 资产 | `game-studio:web-3d-asset-pipeline` | GLB 规范化、纹理、网格预算和运行时检查 | 不切换到 React Three Fiber；不增加第二 renderer |
| macOS 构建与发布 | `build-macos-apps:build-run-debug`、`packaging-notarization`、`signing-entitlements` | 原生启动、打包、公证和签名阶段 | 本机未签名测试不等于可外部分发 |
| Agent 生命周期 | `openai-developers:agents-sdk` | 仅当实现 OpenAI 专属可选适配或核验 Agent 模式 | 当前 Provider 兼容层不强制依赖 OpenAI Agents SDK |

使用规则：先读对应 `SKILL.md`，遵守其前置条件；Skill 只影响开发过程，不自动成为产品依赖。涉及 GitHub 参考项目时以 [GitHub 参考架构](AGENT_GITHUB_REFERENCE_ARCHITECTURE.md) 为采用边界。

## 3. Codex 标准操作流程

```text
读 AGENTS + CODEX_HANDOFF
→ 在 CODEX_TASK_INDEX 领取一个 ready 任务
→ 选择最小插件/Skill
→ 读取当前代码和权威文档
→ 记录基线失败
→ 实现一个原子变化
→ 跑任务 Gate 和文档门
→ 更新任务状态、能力矩阵与 handoff
```

默认不使用多个 Agent、不安装第三方 Skill、不复制整个 GitHub 项目。只有用户明确要求或任务确实需要时才扩展工具面。

### 外部参考的最小操作

1. 用 `@github` 读取目标仓库，而不是先 clone 或把上游代码粘进本仓库；
2. 只摘取一个可验证的模式，例如“步骤/确认卡”或“受限几何 benchmark”；
3. 在任务文档写明不采用的上游范围、许可证与删除条件；
4. 只有任务明确批准后才新增依赖，并同步 lock、SBOM、许可证台账、体积/内存 benchmark 与回归 Gate。

零基础用户不会看到、安装或配置这些开发插件、Codex Skill、MCP 或 GitHub 凭据。

## 4. P0 产品内置 Skill

| Skill | 用户说法示例 | 内部行为 | 永久修改 |
| --- | --- | --- | --- |
| `start_design` | “做一辆适合冰原探索的未来车” | 澄清领域、内部生成多个完整候选，逐个编译/检查/渲染并只展示最佳 blockout | 确认后建版本 |
| `refine_profile` | “让外壳更圆润，前端收得更紧” | 只修改 Recipe 已声明的 ProfileSketch/section 参数并生成预览 | 确认后建版本 |
| `segment_model` | “把它拆成可以分别改的部件” | 生成分件建议和 AssemblyGraph 预览 | 确认后建版本 |
| `modify_selection` | “把我选中的部分短一点、厚一点” | 读取选择、参数范围和锁定状态，形成 ChangeSet | 确认后建版本 |
| `replace_part` | “换一个更轻盈的机翼” | 只找兼容角色/Connector/质量状态的组件 | 确认后建版本 |
| `apply_material` | “车身改成哑光汽车漆，玻璃保持不变” | 预览视觉材质绑定 | 确认后建版本 |
| `pose_joint` | “把机械臂抬高一些” | 只在声明 Joint 的范围内预览概念姿态 | 确认后建版本 |
| `check_model` | “帮我检查一下” | 读取最新质量报告并解释问题 | 否 |
| `prepare_export` | “导出一个可继续使用的模型” | 说明用途、检查活动版本、生成 GLB | 生成导出工件 |
| `explain_design` | “这个模型由什么组成” | 总结当前版本、部件、材质和已知限制 | 否 |

首版只有一个 Orchestrator。不要为四领域创建四个聊天 Agent；领域差异属于 Domain Pack，而不是四套状态和 UI。

内部候选不是用户可选的三个方向，也不是三个资产版本。`start_design` 必须调用候选评审和最佳结果选择；只有通过 runtime/readback 硬门的第一名可以展示，没有候选通过时明确失败。用户说“换一个思路”会开启新 Turn，而不是展开被淘汰候选。

## 5. 产品内 Skill 合同

每个 Skill 使用版本化、可测试的策略包：

```text
skills/start_design/
├── SKILL.md
├── skill-manifest.json
├── tool-policy.json
├── input.schema.json
├── output.schema.json
├── references/
├── examples/
└── evals/
```

最小元数据：

```yaml
id: start_design
version: 1
allowed_tools:
  - inspect_active_design
  - infer_domain_pack
  - plan_complete_concept
  - select_modeling_recipe
  - author_profile_sketch
  - validate_shape_program
  - build_preview
  - inspect_compile_readback
  - evaluate_candidate
  - select_best_candidate
approval_before:
  - confirm_change_set
limits:
  max_tool_calls: 12
  max_wall_time_seconds: 120
```

`SKILL.md` 只定义目标、输入、允许工具、停止条件和用户语言。几何、权限、Schema、预算和路径安全必须由代码校验，不能只依靠提示词。

ForgeCAD 的 `allowed_tools` 是严格限制，不是“免确认列表”。有效工具集固定为：

```text
Skill allowed_tools
∩ 全局 Product Tool Registry
∩ ShapeProgramRuntimeManifest@1
∩ 当前 Turn 权限/批准
```

任何一层不允许都必须返回稳定拒绝，不能让模型改用同义工具、文本命令或隐藏 fallback。

## 6. 工具权限

### 自动只读

`inspect_active_design`、`inspect_selection`、`list_domain_packs`、`list_compatible_modules`、`list_material_presets`、`read_quality_report`、`read_export_profiles`。

### 自动生成候选

`plan_complete_concept`、`select_modeling_recipe`、`author_profile_sketch`、`author_shape_program`、`validate_shape_program`、`build_preview`、`propose_segmentation`、`propose_change_set`、`preview_replacement`、`preview_material`。

这些工具只写临时候选区，允许取消和清理。

`select_modeling_recipe` 只能选择已审阅、领域兼容且运行时可执行的 Recipe；`author_profile_sketch` 只能输出 `ProfileSketch@1` 的规范 JSON。Skill 不能直接提交 SVG path、注册 Loft/Sweep/布尔 operation 或选择几何内核；新的运行时能力必须通过 G819 与对应 G820–G826 任务进入全局 manifest。

### 必须确认

`confirm_change_set`、`confirm_segmentation`、`register_generated_component`、`export_project`、`delete_project`。

确认文案必须说清：会改哪些部件、是否创建新版本、是否写出文件、失败时资产是否保持不变。

### P0 永远禁止

任意 shell、Python、JavaScript、URL 下载、绝对路径、读取整个 Library、上传无关资产、静默覆盖版本、无预算循环调用。

## 7. Provider 配置

用户只填写：API Base URL、API Key、Model，并点击“测试连接”。Provider Key 进入 Keychain 或权限受限 secret file；数据库只保存类型、Base URL、Model 和密钥引用。

应用必须自设合理预算：超时、取消、工具调用上限、结构化输出校验、token/费用记录。网络、超时、鉴权和余额失败默认不自动重试；DeepSeek JSON 空响应或 Schema 不符最多允许一次**可见、计费、同 Turn 的结构修复子请求**，且必须记录 attempt 和费用，仍失败就停止。服务商的最大上下文不是 ForgeCAD 默认预算；P0 应用预算以可预测成本和响应时间为准。

## 8. 零基础交互

界面只显示一个输入框和当前步骤：

```text
描述你想设计什么…

✓ 已理解：双座、短轴距、冰原探索
✓ 已生成并检查多个候选
✓ 已为你选择完整度最高的一版
○ 查看 3D 结果或继续描述修改
```

需要确认时：

```text
这次会修改车头和前轮罩，并创建一个可以返回的新版本。
[先预览] [保留修改] [取消]
```

Mode、Skill、Tool、Schema、Connector、GLB 和 Provider pipeline 默认隐藏在“技术详情”中。

### 8.1 专属 Skill 设计器

用户可以为重复设计任务创建专属 Skill，但使用引导表单而不是代码编辑器。最小流程：

```text
说明 Skill 目的和适用对象
→ 选择允许领域与只读/候选工具
→ 定义输入提示和输出格式
→ 添加 3 个成功示例、3 个失败/停止示例
→ dry-run（零版本/零 Snapshot 副作用）
→ eval 通过
→ 保存为禁用草稿或显式启用版本
```

可支持的例子：家用电器完整外观、复古工业设计语言、紧凑桌面设备、汽车内外饰配色研究。不能支持现实武器制造、工程尺寸/性能、任意网络抓取、shell/脚本、直接文件路径或绕过确认。编辑 Skill 会创建新 Skill 版本；已存在的 AgentAssetVersion 继续记录原 Skill hash，不被新版本追溯改写。

## 9. P0 不安装

- TripoSR、Stable Fast 3D、Hunyuan3D、TRELLIS、ComfyUI；
- Blender/Unity 插件；
- 通用 MCP 市场、远程自动化扩展和多 Agent 框架；
- 云向量数据库和任意代码执行插件。

以后加入 Blender 或团队审批时，它们只能是专业扩展，不能成为“说一句话生成并编辑第一个模型”的前置条件。

## 10. 评测

每个核心 Skill 至少覆盖：

- 四领域各 20 条正常中文 Brief；
- 20 条含糊/未知领域输入，必须进入单问题澄清；
- 20 条非法 ShapeProgram、越权工具和路径输入；
- 取消、超时、断线和重启恢复；
- 连续 10 次局部修改后的版本、选择、质量和导出一致性；
- 零基础用户 5 分钟内查看 Agent 选出的最佳结果、完成首次修改和首次 GLB 导出；不要求先理解或选择三方向。

指标包括任务完成率、结构化输出通过率、首次可见反馈、预览时间、确认前永久副作用为零、取消成功率、token 成本、峰值内存和 renderer 数量。
