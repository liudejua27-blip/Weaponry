# ForgeCAD 产品定义

版本：2026-08-07
状态：目标产品；代码迁移中

## 1. 一句话

ForgeCAD 是 Codex 可自由调用的本地 3D Runtime：把用户的合法参考和要求编译成高质量、可检查、可局部修改、可回退、可爆炸查看和可导出的 3D 资产。

Codex 是大脑，ForgeCAD 是身体。ForgeCAD 不再内置大模型或 Agent 对话。

## 2. 目标用户

- 不会传统 DCC/CAD，但能用自然语言和参考图表达需求的普通创作者；
- 需要可编辑、可追踪 3D 资产的独立游戏、影视预演、电商视觉和概念设计团队；
- 使用 Codex 编排复杂 3D 工作流的技术美术与开发者。

用户必须能使用受支持的 Codex 宿主。ForgeCAD 单独启动只用于查看和诊断，不承诺自主生成。

## 3. 核心任务

1. 在 Codex 中描述对象、上传一张或多张授权图片；
2. 让 Codex 调用 ForgeCAD 建立几何、轮廓、比例和语义部件；
3. 生成 UV、PBR、纹理、材质和局部细节；
4. 通过固定视图、AOV、参考比较和视觉评审暴露差异；
5. 在 Viewer 选择局部，在 Codex 中反复提出 typed 修改；
6. 用户批准后创建不可变版本；
7. 查看爆炸图、恢复历史或导出 GLB 等交付资产。

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

## 5. P0 范围

- 本地 macOS Alpha；
- Codex Desktop、CLI、IDE；
- MCP stdio；
- 图片 evidence 导入；
- typed procedural/surface/deform/read-only mesh hybrid；
- GLB/PBR、固定视图、参考比较和质量报告；
- first-party signed declarative Skills；
- Runtime Viewer；
- 候选、用户批准、不可变版本、局部修改、回退、爆炸图和导出；
- 默认无模型 API 网络调用、无常驻大模型或 3D 神经权重。

## 6. 明确不做

- ForgeCAD 内置聊天、Agent、OpenAI/DeepSeek/千问/其他模型 SDK 和 Provider Registry；
- 通用 MCP Client、云多租户或远程协作；
- 任意 Python、JavaScript、shell、插件、Blender addon 或网络 Operator；
- 用 `.blend`、Three.js scene、截图、prompt 或外部服务响应作为项目真值；
- 自动训练/下载基础 3D 模型、CUDA 大权重或常驻 GPU 服务；
- 工程 B-Rep、制造、公差、结构、适航、医疗或认证；
- 未经用户批准创建永久版本；
- 未经许可证/SBOM/签名/Benchmark 审核的 GitHub 项目或资产。

## 7. 安全与内容范围

任何类别都可作为合法的非功能性视觉资产目标。虚构游戏资产、影视道具和展示模型可以包含武器外观，但项目不生成现实武器制造图纸、现实可制造武器、制造尺寸、材料配方、功能机构、加工流程或性能建议。

汽车、飞机、建筑、角色、医疗器械和机械设备只作为视觉资产；结果不提供安全、结构、适航、医疗、动力学或认证结论。用户必须拥有参考图和外部资产的使用权，ForgeCAD 保存 provenance 并在导出时生成清单。

## 8. 成功标准

迁移完成至少需要：

- 产品代码中无内置模型/Provider/聊天和端口 8000；
- Codex 三宿主能真实传入参考字节并调用 MCP；
- Viewer 关闭时 Runtime 仍能完成 compile/render/evaluate；
- 用户拒绝不写版本，批准只写一个幂等版本；
- 局部选择/修改、回退、爆炸图和导出在重启后仍一致；
- 跨类别 Benchmark 与独立真人门达标；
- 安装包内 Runtime/MCP/workers/Viewer/Skills 合同和签名一致。

在这些条件完成前，任何演示都只能按其真实证据标为 prototype、部分实现或目标设计。
