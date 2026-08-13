# ForgeCAD 产品定义

版本：2026-08-13
状态：单用户 MVP host golden path 已完成；MCP005–009 geometry/appearance/render/limited-quality/change/version/export 可用，真实 Codex CLI 主链已通过；MCP010F 仍为 `QUALITY_TARGET_NOT_MET`。ADR-0026 已把后续方向定义为 Agentic Design Runtime，但 DesignSession/SemanticSceneGraph/ReferenceCanvas/Critic loop 仍是目标设计，尚未成为当前实现能力。

## 1. 一句话

ForgeCAD 是 Codex 可自由调用的本地 3D Runtime：把用户的合法参考和要求编译成可检查、可局部修改、可回退和可导出的 3D 资产。MVP 只提供 bounded hard-surface functional core；“高质量/相似度”必须由真实参考指标和真人门证明，不能由工具存在或单张截图推出。

Codex 是大脑，ForgeCAD 是身体。ForgeCAD 不再内置大模型或 Agent 对话。ADR-0026 进一步规定：未来高质量路线不是把 ForgeCAD 变成聊天 Agent，而是在 Runtime 上增加可观察的 Agentic design loop，让 Codex 每步都能读取语义场景、视觉证据、阶段门和下一步允许动作。

## 2. 目标用户

- 不会传统 DCC/CAD，但能用自然语言和参考图表达需求的普通创作者；
- 需要可编辑、可追踪 3D 资产的独立游戏、影视预演、电商视觉和概念设计团队；
- 使用 Codex 编排复杂 3D 工作流的技术美术与开发者。

用户必须能使用受支持的 Codex 宿主。ForgeCAD 单独启动只用于查看和诊断，不承诺自主生成。

## 3. 核心任务

1. 在 Codex 中描述对象、上传一张或多张授权图片；
2. 让 Codex 调用 ForgeCAD 建立几何、轮廓、比例和语义部件；
3. 生成 bounded UV/tangent、PBR MaterialZone 和局部细节；纹理烘焙/UDIM 属后续能力；
4. 通过固定视图、AOV、参考比较和视觉评审暴露差异；
5. 在 Viewer 选择局部，在 Codex 中反复提出 typed 修改；
6. 用户批准后创建不可变版本；
7. 查看爆炸图、恢复历史或导出 GLB 等交付资产。

目标高质量流程升级为：

```text
ReferenceCanvas
→ DesignSpec
→ SemanticSceneGraph / ModelUnderstandingBundle
→ primary-form gate
→ secondary-structure gate
→ tertiary-detail gate
→ uv-pbr gate
→ final review / human / export
```

Primary form 未过时不能做细节；visible-view 未过时不能解锁 PBR；`QUALITY_TARGET_NOT_MET` 不能 confirm/export。

## 4. 产品承诺

### 可验证

每个结果都能回答：来自哪些参考、用了哪些 Skill/Recipe/Operator/资产、输入与输出 hash、几何和 PBR 回读、哪些质量门通过/失败、谁在何时批准、导出是否与查看版本一致。

### 可编辑

资产具有 Assembly/Part/MaterialZone 和稳定 source map。修改针对 typed 语义范围，而不是让 Codex 直接改未知三角数组。

### 可回退

确认创建不可变版本；历史恢复创建新子版本，任何版本可读取和导出。候选内 undo/redo 与已确认版本 restore 明确分离。

### 类别开放但诚实

参考对象类别不由机械、武器或固定关键词白名单限制。Runtime 根据 `SubjectProfile → RepresentationPlan` 选择能力；没有可靠表示时返回 limitation 或要求更多视图，不生成错类别模板。

### 高质量优先

质量包括完整主体、身份轮廓、比例、结构层级、中观/局部细节、UV、PBR、纹理、材质、固定视图相似度和交付 readback。时间和资源有上限，但不靠跳过必要证据换取“看起来很快”。

### 看得见和可解释

未来 Codex 不能只收到 object/position/mesh 数字。ForgeCAD 必须能提供 Scene Graph、语义 Part、尺寸、对称关系、source operator、MaterialZone、几何统计、相机、selection、多视图 AOV、失败门和 evidence hash。每个设计建议必须能回指具体 `part_id`、`material_zone_id`、`render_id`、`quality_report_hash` 或 `feature_id`。

## 5. 单用户 MVP 范围

- 本地 macOS Alpha；
- Codex Desktop、Codex CLI；
- MCP stdio；
- 一张 PNG/JPEG evidence 导入；
- Codex 生成的 typed hard-surface procedural mesh；
- 首个机器人基准的 GLB/PBR、固定视图和 limited quality report；像素级参考比较和真人评分仍是验收证据，不是当前代码承诺；
- first-party canonical-hash declarative Skills；
- Runtime Viewer；
- 候选、用户批准、不可变版本、一次局部修改、回退和 GLB 导出；
- 默认无模型 API 网络调用、无常驻大模型或 3D 神经权重。

ADR-0026 的 Agentic Design Runtime、DesignSession、SemanticSceneGraph、Parametric Design Kit 和 Critic/Repair loop 属于下一阶段目标设计，不列入当前 MVP 已实现范围。

Codex IDE / VS Code / Cursor / Windsurf 保留为未来兼容目标，不安装、不作为当前 P0 产品入口或发布 Gate。若未来正式建设 Skill SDK、插件开发生态或第三方开发者模式，再单独升级其支持级别。

MVP 不包含爆炸图完整 UX、后台 Job checkpoint、第三方插件市场、Developer ID/notarization、跨类别通用质量。这些是 MCP010–013 的产品化范围。

## 6. 明确不做

- ForgeCAD 内置聊天、Agent、OpenAI/DeepSeek/千问/其他模型 SDK 和 Provider Registry；
- 通用 MCP Client、云多租户或远程协作；
- 任意 Python、JavaScript、shell、插件、Blender addon 或网络 Operator；
- 用 `.blend`、Three.js scene、截图、prompt 或外部服务响应作为项目真值；
- 自动训练/下载基础 3D 模型、CUDA 大权重或常驻 GPU 服务；
- 工程 B-Rep、制造、公差、结构、适航、医疗或认证；
- 未经用户批准创建永久版本；
- 未经 adoption receipt、许可证/SBOM/Benchmark 审核的 GitHub 项目或资产；
- BlenderMCP、FreeCAD MCP、任意 Python CAD MCP 或远程 image-to-3D Provider。
- 直接套用 Pi Agent、Omniverse Kit、OpenUSD、FreeCAD、build123d/CadQuery、BlenderMCP、TRELLIS/Hunyuan3D 的代码、Skill 或权重作为产品真值。它们只能按 `EXTERNAL_PROJECT_ADOPTION.md` 作为 reference-only、approved-for-evaluation 或 accepted 依赖进入。

## 7. 安全与内容范围

任何类别都可作为合法的非功能性视觉资产目标。虚构游戏资产、影视道具和展示模型可以包含武器外观，但项目不生成现实武器制造图纸、现实可制造武器、制造尺寸、材料配方、功能机构、加工流程或性能建议。

汽车、飞机、建筑、角色、医疗器械和机械设备只作为视觉资产；结果不提供安全、结构、适航、医疗、动力学或认证结论。用户必须拥有参考图和外部资产的使用权，ForgeCAD 保存 provenance 并在导出时生成清单。

## 8. MVP 成功标准

首个硬表面 MVP 至少需要：

- 产品代码中无内置模型/Provider/聊天和端口 8000；
- Codex Desktop/CLI 能真实传入参考字节并调用 MCP；IDE 兼容不属于当前 P0 成功标准；
- 真实 Codex CLI 将用户授权参考字节送入 CAS；
- Codex typed program 编译为真实多 Part GLB，并在 Viewer/headless render 使用同一 hash；
- Viewer 关闭时 Runtime 仍能完成 compile/render/evaluate；
- 用户拒绝不写版本，批准只写一个幂等版本；
- 一次 stable-Part `change_prepare`、回退和 CAS-backed GLB receipt 在 focused Runtime tests 中一致；真实 host/restart 仍需证据；
- 几何/GLB/PBR 硬门已有 evidence；参考指标、Codex typed review 和用户评分尚未运行时，不得写“参考基准已验收”。

这些条件通过后只能声明“首个硬表面参考基准 MVP”。通用高质量和外部分发仍要求跨类别真人门以及 Runtime/MCP/workers/Viewer/Skills 的签名安装包一致性。
