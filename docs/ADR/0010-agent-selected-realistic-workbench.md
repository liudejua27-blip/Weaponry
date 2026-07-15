# ADR-0010：Agent 自动选择单一最佳方案、Codex 式工作台与视觉真实度管线

- 状态：Accepted（目标设计；实现任务均未完成）
- 日期：2026-07-14
- 决策者：项目维护者
- 取代：面向零基础用户展示三个方向并要求用户先选一个的目标流程；`FGC-V002` 的三方向解释/单维重混交互

## 背景

当前 Alpha 会把三个完整外观方向和三组预审外观轮换暴露给用户。这个流程把原本应由 Agent 完成的比较工作交给了零基础用户，同时造成工作台信息密度过高。当前模型仍主要由低多边形 ShapeProgram、单一参数材质和有限展示细节构成，比例、曲面转折、接缝、纹理、UV、环境光和材质分区不足，因此即使 Planner 文案合理，3D 结果仍与真实产品外观有明显差距。

2026-07-14 的本机运行诊断还确认：当前桌面实例虽然 Agent 健康，但没有 ForgeCAD Provider metadata，也没有 `ForgeCAD Agent Provider/default` Keychain 项，因此运行时选择了确定性离线 Planner，未发起 DeepSeek HTTP 请求。当前 UI 会把 Provider 详细错误压缩成“暂时无法连接/测试未完成”，用户无法判断是未配置、鉴权、余额、参数、限流、服务故障还是结构化空响应。

## 决策

1. Agent 可以在内部生成多个受限候选，但默认只向用户展示一个经过编译、GLB readback、概念渲染和规则评审后选出的最佳结果；不再显示三张方向选择卡。
2. “最佳”必须有 `BestCandidateDecision@1` 证据：Brief 覆盖、完整外观、语义比例、运行时/GLB 质量、可编辑性、材质/纹理覆盖和视觉一致性。失败候选不允许被选中，评分不能替代真实编译/readback。
3. 用户仍可用自然语言要求“换一个思路”或继续修改，但这是新 Turn，不是三方向选择器，也不能静默覆盖已确认版本。
4. 工作台采用 Codex 式单任务界面：中心是连续 Agent 会话、步骤和结果；3D 视口缩为左上角 mini viewport，点击后把**同一个 canvas/renderer**移动到中央焦点层，关闭后返回左上角。不得同时创建 mini 与 full 两个 renderer。
5. DeepSeek 接入升级为可观察 Provider Gateway：配置来源、连接状态、请求开始、流式进度、取消、用量和固定错误类别均形成可读 Item；未配置时明确离线，已经选择真实 Provider 后失败时不得静默回退并冒充成功。
6. ForgeCAD 参考 Codex 的 Thread/Turn/Item 生命周期和 Skill 渐进加载，参考 Claude Code 的专用 Agent、Skill、hook 和受限工具思想，但只实现 ForgeCAD 产品工具，不复制 shell/code Agent。DeepSeek thinking/tool-call 多轮必须遵守官方 `reasoning_content` 续传合同；用户只看到可审计的思考摘要和动作，不显示原始隐藏推理。
7. 允许创建 ForgeCAD 专属 Skill，但 Skill 必须是版本化声明资产：`SKILL.md + tool-policy + input/output schema + examples + evals`。Skill 只能引用 G819 运行时清单和产品工具注册表，不能执行任意 Python、JavaScript、shell、URL 或文件路径。
8. 视觉真实度由确定性资产管线提升，不把 DeepSeek 当作网格或纹理生成器。目标管线包含：语义比例配方、可编辑组件配方、受限细节几何、稳定 Material Zone、UV/切线、PBR 纹理集合、clearcoat 等受支持扩展、HDRI/色彩管理、GLB validate/inspect 和视觉基准评测。
9. 产品 Core 继续是通用机械概念系统。四个现有领域包保留为首批验证包，后续通过版本化领域包/Skill 扩展家用电器、工具设备、工程机械、农业机械和其他生活机械外观；不把“通用”解释成无限制工程设计或安全结论。

## 后果

- `FGC-V002` 标记为 `superseded`，以 `FGC-V003` 的内部候选评审和单一最佳结果取代；当前 Alpha 在相关任务完成前仍会显示三个方向，用户指南必须继续如实描述当前行为。
- `FGC-F025` 只负责 legacy 隔离和继续拆薄父层；新的 Codex 式布局由独立 `FGC-F026` 实施，避免在同一任务中重写状态和视觉。
- 新增 DeepSeek Provider Gateway、受限 Action Loop、产品 Skill、视觉真实度、多材质、组件配方、参考引导重建和通用机械扩展任务；所有任务仍受 ActiveDesignSnapshot、preview→confirm、单 WebGL 和安全范围约束。
- 高视觉真实度是目标设计，不是当前能力。它不等于工程精度、制造可行性、真实材料性能、车辆安全、适航或机器人控制结论。

## 被否决方案

- 继续让用户在三张方向卡中承担评审：增加认知负担，也掩盖 Agent 缺少自评闭环。
- 只更换大模型或扩大 prompt：无法补足几何、UV、纹理、灯光、readback 和资产质量问题。
- mini viewport 与中央 viewport 各建一个 Three.js renderer：违反单 WebGL 约束并增加显存、资源释放和状态同步问题。
- 直接安装完整 Codex/Claude Code/CAD/DCC 运行时：会引入任意工具权限、重复状态真值、安装体积和发布风险。
- 允许用户 Skill 携带任意脚本或 URL：破坏 ShapeProgram 和本地权限边界。
