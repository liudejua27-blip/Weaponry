# ADR-0025：Codex-only MCP 3D Runtime

日期：2026-08-07；2026-08-09 增补 MCP005–009 functional-core、工具/Skill 目录与 MCP010A–F 质量轨道
状态：accepted
决策者：产品负责人
替代：ADR-0023、ADR-0024，以及 ADR-0017/0019/0020/0021/0022 中“产品内 Agent、Provider 或模型路由”部分

## 1. 决策

ForgeCAD 不再是带内置大模型、聊天和 Provider 配置的 3D Agent 应用。它重构为一个由 Codex 驱动、可验证、可回退的本地 3D Runtime：

- Codex 是外部大脑，负责理解用户对话、图片和意图，规划调用顺序，并阅读 ForgeCAD 返回的结构化证据；
- ForgeCAD 是身体，负责类型化几何、UV、PBR、纹理、材质、渲染、质量检查、版本、局部修改、爆炸图、资产和 Skill 执行；
- P0 普通用户在 Codex Desktop 或 Codex CLI 中对话并上传参考，ForgeCAD Desktop 只查看项目、候选、质量、版本和任务状态；Codex IDE/VS Code/Cursor/Windsurf 保留为未来兼容宿主，不是当前 P0 入口；
- P0 只发布和验收 Codex 客户端配置，不内置 OpenAI、DeepSeek、千问或任何其他模型 SDK、API Key、Provider Registry、聊天界面和模型调用；
- Codex 通过本地 MCP `stdio` Server 调用 ForgeCAD。MCP 层是薄适配器，产品状态只能由 Rust Runtime 单写者修改；
- 永久修改必须遵循 `prepare → compile/readback → render/evaluate → user approval in Codex → confirm`，确认前只产生候选和证据；
- 每次确认创建不可变资产版本。局部修改、恢复、爆炸图和导出均绑定项目、基线版本、候选 hash、幂等键和审批回执；
- Skill 是受签名和预算约束的产品能力包，不是提示词文件，也不是任意代码插件。

## 2. 第一性原理边界

### 2.1 “Codex-only”不是模型身份认证

MCP Server 能识别连接、进程、项目授权和客户端声明，但不能密码学证明请求背后一定是某个 Codex 模型。因此 P0 的准确含义是：

1. P0 只随产品提供 Codex Desktop/CLI 的安装配置和测试矩阵；Codex IDE/VS Code/Cursor/Windsurf 只保留未来兼容配置，不作为当前发布 Gate；
2. 只对 P0 Codex Desktop/CLI 的实际 MCP 行为承担当前兼容性；
3. 通过本机进程、会话令牌、项目授权、工具审批和签名工件控制权限；
4. 不使用可伪造的 `client_name == codex` 作为安全边界。

其他 MCP Client 既不宣传、也不验收；未来是否开放必须另立 ADR。IDE 若因 Skill SDK、插件开发生态或第三方开发者模式而升级为正式支持，也必须新增/修订独立 Gate。

### 2.2 上传图片不是自动成立的能力

Codex 能看到用户附件，不等于本地 MCP Server 必然收到图片字节。`reference_import` 必须通过真实 Codex 客户端证明至少一种受限传输：小文件内联内容，或 Codex 暴露且经用户授权的本地附件路径。未经端到端验证，不得把“Codex 上传图片 → ForgeCAD 导入”标为已实现。

### 2.3 删除旧实现不等于删除用户数据

旧工作台、Provider、App Server、协议和 Agent 代码直接删除；现有 Library、数据库、CAS 和用户资产保留为只读备份。新 Runtime 使用全新 V1 数据库，不自动打开或写入旧库。一次性离线导出器通过验收后，才能删除兼容代码；删除用户数据需要单独、明确的用户授权。

### 2.4 高质量不是“装了很多工具”

高质量必须由同一候选的结构、回读、固定视图、参考比较、PBR/UV、视觉评审、版本和导出证据共同证明。Skill、材质包、GLB 可打开或测试绿色，都不能单独证明参考相似度或商业质量。

## 3. 目标架构

```text
User
  └─ Codex Desktop / CLI
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

Codex IDE/VS Code/Cursor/Windsurf 的 MCP 兼容代码可以保留，但不属于当前 P0 产品链路、安装要求或 MCP003/MCP004 发布阻断。

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

## 6. 单用户 MVP profile

为避免把未来生产级治理压到个人 MVP，当前实施采用：`forgecad-mcp` + `forgecad-runtime` 两个后端 executable、OS 文件锁单写者、一次有界 Runtime restart、可选只读 Viewer。MVP 不使用 TTL lease/heartbeat、独立 Host、broker、远程 transport、密码学人类 attestation 或第三方插件市场；Codex/MCP 退出后未完成 Job 可以明确失败。

MCP005–009 的完成边界是单用户 development functional core 和真实 Codex CLI host golden path，不是像素相似度或高质量验收。MCP010A–F 在不改写既有 receipt 的前提下关闭首个硬表面可见视图质量轨道；Developer ID/notarization、packaged Desktop、复杂 Job 恢复、通用第三方分发和跨类别真人门仍由 MCP011–013 承担。这个拆分不放宽 Runtime 单写者、typed Operator、hash/approval/idempotency、无任意脚本和真实参考/GLB evidence。

### 6.1 MCP010 质量轨道边界

MCP010 固定拆分为：A 权威重排/开发激活、B 合同与几何回读真值、C 固定渲染/参考比较、D 高细节 Operator、E first-party 离线 AssetPack/UV/PBR、F Viewer 与真实机器人闭环。每次只允许一个子任务 `in_progress`。当前单张三分之四参考最多产生 `PARTIAL_VISIBLE_VIEW_PASS`；补齐 front/back/left/right/rear-three-quarter 全身参考前，`HQ_360_PASS` 必须保持 `BLOCKED_REFERENCE_COVERAGE`。

010E 允许对计划点名的 CC0 文件做一次性实施期下载和逐资产 adoption，再将派生内容编入 first-party 离线 AssetPack；Runtime 不联网、不调用素材 API，也不提供通用安装器。Job checkpoint/GC/全局并发属于 MCP011，第三方 publisher/安装/升级/撤销属于 MCP012，Developer ID/notarization/clean install/packaged E2E/filesystem export 和跨类别真人门属于 MCP013。

详细实施合同见 `MVP_DELIVERY_PLAN.md`。

## 7. Skill 决策

正式 Skill Bundle 必须同时包含：

`知识 + typed 输入/输出 Schema + Recipe DAG + 受限 Operator + Validator + 材质/资产 + Benchmark + 许可证/NOTICE/SBOM + provenance + 签名`

MVP 只允许 first-party 声明式 Operator 和产品内置实现，使用 canonical hash + 开发 trust root，并从第一天保留 Schema、Recipe、Validator、Benchmark、LICENSE/NOTICE、SBOM 和 provenance。分发签名/撤销在 MCP012/013 完成。签名只证明来源和完整性，不证明安全。第三方 WASI Operator、网络 Operator 或 Blender/Python 脚本必须另立 ADR。

## 8. 质量编译器

候选只有同时通过下列分层检查，才可进入确认：

1. 几何硬门：可解析、预算、拓扑、法线、退化面、自交、Part/primitive/source-map；
2. 轮廓与比例：固定相机、多视图 silhouette、关键尺寸和语义部件关系；
3. UV/PBR：UV 岛、重叠、padding、stretch、texel density、切线、通道语义和色彩空间；
4. 纹理与材质：分区、分辨率、接缝、重复、烘焙、材质参数和资产 provenance；
5. 局部细节：稳定 Part/face/source-map 覆盖和局部修改可追溯性；
6. 参考比较：beauty、alpha、depth、normal、AO、part-ID、material-ID、wireframe、UV stretch、silhouette；
7. Codex 视觉评审和真人 Benchmark：只作为有证据的软评分或发布门，不能绕过结构硬门。

## 9. 失败与撤销

- MCP 请求重复必须幂等；
- 基线漂移、hash 不一致、审批过期、Worker 崩溃、超时和重启恢复均 fail closed；
- 长任务必须快速返回 `RuntimeJob`，支持读取事件、取消和恢复，不依赖客户端长期保持一次工具调用；
- 未确认候选可被 GC；已确认版本不可变；
- `undo` 是尚未确认候选的局部撤销；`restore` 是从历史版本创建新子版本，不移动或改写历史提交。

## 10. 后果

优点：产品边界清晰，没有重复大脑；3D 能力可独立测试；Codex 可以自由组合工具和 Skill；所有永久结果可验证、回退和审计。

代价：ForgeCAD 单独启动不再生成模型；用户必须使用 Codex；附件传输、MCP 长任务、headless 渲染、Skill 供应链和真实视觉质量都需要新的端到端证据。重构期间旧产品能力将被有意下线，不能包装成平滑升级。

## 11. 接受条件

MCP005–009 全部通过后，本 ADR 只代表单用户 MVP functional core 已落地。MCP010F 的可见视图指标、typed review、用户评分、版本/restore/export 同 hash 全部通过后，才可声明“首个硬表面可见视图质量闭环”；多视图缺失时不得声明 360。只有 MCP001–MCP013 全部通过后，才代表可分发、跨类别的产品迁移完成。
