# ADR-0025：Codex-only MCP 3D Runtime

日期：2026-08-07
状态：accepted
决策者：产品负责人
替代：ADR-0023、ADR-0024，以及 ADR-0017/0019/0020/0021/0022 中“产品内 Agent、Provider 或模型路由”部分

## 1. 决策

ForgeCAD 不再是带内置大模型、聊天和 Provider 配置的 3D Agent 应用。它重构为一个由 Codex 驱动、可验证、可回退的本地 3D Runtime：

- Codex 是外部大脑，负责理解用户对话、图片和意图，规划调用顺序，并阅读 ForgeCAD 返回的结构化证据；
- ForgeCAD 是身体，负责类型化几何、UV、PBR、纹理、材质、渲染、质量检查、版本、局部修改、爆炸图、资产和 Skill 执行；
- 普通用户在 Codex Desktop、Codex CLI 或 Codex IDE 中对话并上传参考，ForgeCAD Desktop 只查看项目、候选、质量、版本和任务状态；
- P0 只发布和验收 Codex 客户端配置，不内置 OpenAI、DeepSeek、千问或任何其他模型 SDK、API Key、Provider Registry、聊天界面和模型调用；
- Codex 通过本地 MCP `stdio` Server 调用 ForgeCAD。MCP 层是薄适配器，产品状态只能由 Rust Runtime 单写者修改；
- 永久修改必须遵循 `prepare → compile/readback → render/evaluate → user approval in Codex → confirm`，确认前只产生候选和证据；
- 每次确认创建不可变资产版本。局部修改、恢复、爆炸图和导出均绑定项目、基线版本、候选 hash、幂等键和审批回执；
- Skill 是受签名和预算约束的产品能力包，不是提示词文件，也不是任意代码插件。

## 2. 第一性原理边界

### 2.1 “Codex-only”不是模型身份认证

MCP Server 能识别连接、进程、项目授权和客户端声明，但不能密码学证明请求背后一定是某个 Codex 模型。因此 P0 的准确含义是：

1. 只随产品提供 Codex Desktop/CLI/IDE 的安装配置和测试矩阵；
2. 只对 Codex 的实际 MCP 行为承担兼容性；
3. 通过本机进程、会话令牌、项目授权、工具审批和签名工件控制权限；
4. 不使用可伪造的 `client_name == codex` 作为安全边界。

其他 MCP Client 既不宣传、也不验收；未来是否开放必须另立 ADR。

### 2.2 上传图片不是自动成立的能力

Codex 能看到用户附件，不等于本地 MCP Server 必然收到图片字节。`reference_import` 必须通过真实 Codex 客户端证明至少一种受限传输：小文件内联内容，或 Codex 暴露且经用户授权的本地附件路径。未经端到端验证，不得把“Codex 上传图片 → ForgeCAD 导入”标为已实现。

### 2.3 删除旧实现不等于删除用户数据

旧工作台、Provider、App Server、协议和 Agent 代码直接删除；现有 Library、数据库、CAS 和用户资产保留为只读备份。新 Runtime 使用全新 V1 数据库，不自动打开或写入旧库。一次性离线导出器通过验收后，才能删除兼容代码；删除用户数据需要单独、明确的用户授权。

### 2.4 高质量不是“装了很多工具”

高质量必须由同一候选的结构、回读、固定视图、参考比较、PBR/UV、视觉评审、版本和导出证据共同证明。Skill、材质包、GLB 可打开或测试绿色，都不能单独证明参考相似度或商业质量。

## 3. 目标架构

```text
User
  └─ Codex Desktop / CLI / IDE
       ├─ conversation + image understanding + planning
       └─ MCP stdio
            └─ forgecad-mcp             thin adapter; no database
                 └─ forgecad-runtime     only product-state writer
                      ├─ forgecad-store  SQLite + CAS + immutable versions
                      ├─ forgecad-core   typed design/geometry/appearance/quality
                      ├─ skill-registry  signed declarative Skill Bundles
                      ├─ geometry-worker restricted typed compiler
                      ├─ render-worker   deterministic headless evidence
                      └─ optional signed Blender worker; never arbitrary scripts

ForgeCAD Desktop Viewer
  └─ runtime read model + ephemeral camera/selection/isolation/explosion preview
```

Runtime、MCP 和 Viewer 使用同一组 `packages/forgecad-contracts` Schema。MCP 不直接访问 SQLite/CAS；Viewer 不创建第二版本头；Worker 不拥有项目状态。

## 4. 保留的历史原则

以下原则继续有效：

- `ActiveDesignSnapshot` 是当前项目状态的单一投影；
- Rust 结构化状态是事实，GLB、渲染和自然语言是有 lineage 的工件；
- 创作类别开放，能力不足时返回 typed limitation，不回退固定机械模板；
- 默认不训练基础 3D 模型、不运行常驻 GPU 大模型；
- 任意 Python、JavaScript、shell、URL 或文件路径不能成为几何真值；
- 大对象进入内容寻址存储，事件只保存引用；
- 质量门、安全范围和许可证门不得因重构被删除。

## 5. 明确删除

实施任务 `FGC-MCP001` 按 [重置迁移计划](../RESET_MIGRATION_PLAN.md) 成组删除：

- 旧 `cad-workbench` 前端；
- 内置聊天、图片上传、Provider 设置和模型状态；
- DeepSeek、千问、API-first Provider Registry、coding/search/vision/3D Provider；
- 旧 App Server、JSON-RPC/HTTP/SSE 兼容层和端口 8000 Python Agent；
- 旧 Concept/Weapon/ModuleGraph/机械模板契约、fixture、脚本和 CI；
- 旧 U004/Provider 当前文档。

删除任务必须在可恢复快照后执行，并在同一原子任务中放入可编译的 Viewer Shell、Runtime skeleton 和新合同；不得长期留下故意不可编译的仓库。

## 6. Skill 决策

P0 Skill Bundle 必须同时包含：

`知识 + typed 输入/输出 Schema + Recipe DAG + 受限 Operator + Validator + 材质/资产 + Benchmark + 许可证/NOTICE/SBOM + provenance + 签名`

P0 只允许声明式 Operator 和产品内置实现。签名只证明来源和完整性，不证明安全；安装时仍需 Schema、能力、预算、许可证、恶意输入和 Benchmark Gate。第三方 WASI Operator、网络 Operator 或 Blender/Python 脚本必须另立 ADR。

## 7. 质量编译器

候选只有同时通过下列分层检查，才可进入确认：

1. 几何硬门：可解析、预算、拓扑、法线、退化面、自交、Part/primitive/source-map；
2. 轮廓与比例：固定相机、多视图 silhouette、关键尺寸和语义部件关系；
3. UV/PBR：UV 岛、重叠、padding、stretch、texel density、切线、通道语义和色彩空间；
4. 纹理与材质：分区、分辨率、接缝、重复、烘焙、材质参数和资产 provenance；
5. 局部细节：稳定 Part/face/source-map 覆盖和局部修改可追溯性；
6. 参考比较：beauty、alpha、depth、normal、AO、part-ID、material-ID、wireframe、UV stretch、silhouette；
7. Codex 视觉评审和真人 Benchmark：只作为有证据的软评分或发布门，不能绕过结构硬门。

## 8. 失败与撤销

- MCP 请求重复必须幂等；
- 基线漂移、hash 不一致、审批过期、Worker 崩溃、超时和重启恢复均 fail closed；
- 长任务必须快速返回 `RuntimeJob`，支持读取事件、取消和恢复，不依赖客户端长期保持一次工具调用；
- 未确认候选可被 GC；已确认版本不可变；
- `undo` 是尚未确认候选的局部撤销；`restore` 是从历史版本创建新子版本，不移动或改写历史提交。

## 9. 后果

优点：产品边界清晰，没有重复大脑；3D 能力可独立测试；Codex 可以自由组合工具和 Skill；所有永久结果可验证、回退和审计。

代价：ForgeCAD 单独启动不再生成模型；用户必须使用 Codex；附件传输、MCP 长任务、headless 渲染、Skill 供应链和真实视觉质量都需要新的端到端证据。重构期间旧产品能力将被有意下线，不能包装成平滑升级。

## 10. 接受条件

本 ADR 只有在 `FGC-MCP001`–`FGC-MCP013` 全部通过后才代表产品迁移完成。在此之前，当前状态是“目标设计/重置中”，不是可用的新产品。
