# ForgeCAD 兼容迁移计划

版本：2026-07-13
目标：从 Weapon/Concept 双运行时迁移到通用机械 Agent 单一真值

## 1. 迁移原则

- 不原地改写历史 Weapon 数据；
- 不把旧 ID、hash 或版本号伪装成新合同；
- 先建立新读写路径和自动证据，再删除旧路径；
- 任何阶段可回到旧数据的只读模式；
- UI 不允许同时编辑两套状态。

## 2. 当前兼容面

仍在启动链中的 legacy：

- `wushen_agent.main` 进程名和入口；
- `/api/weapons`、旧 Job/Asset/Provider 路由；
- `ConceptProject/ConceptVersion/ModuleGraph`；
- Concept Quality/Export；
- ComfyUI、神经 3D、Patch 和 Unity 回归；
- Weapon reference pack。

新 Agent 已有：Thread/Turn/Item、Domain Pack、ShapeProgram、AssemblyGraph、AgentAssetVersion、Agent ChangeSet、MaterialPreset、AgentComponent 和 GLB 导出。

## 3. 分阶段迁移

### M0：文档和能力冻结

- 主文档只描述当前 Agent；
- legacy 资料移入 `docs/legacy/`；
- 能力—Gate 矩阵区分新旧证据；
- 禁止新增旧 API 产品功能。

退出条件：文档门禁和断链检查通过。

### M1：ActiveDesignSnapshot

- S001–S003 已完成：冻结 [AUTHORITATIVE_STATE.md](AUTHORITATIVE_STATE.md) 合同，新增 Snapshot 表、repository、revision CAS、Agent head/Snapshot 同事务更新，以及 GET/select/legacy-rebuild hand-off API；
- 为旧 Concept 建立只读 adapter；
- 质量、选择和导出显式绑定同一资产版本。

S007 已完成 legacy 兼容 UI 只读、显式转换授权和确认 Agent 资产时的原子 Snapshot 提升；它不生成旧 ModuleGraph 的伪编辑副本，也不改写旧数据。下一步只剩完整并发/重启 E2E。退出条件：一个项目只有一个活动版本、选择和预览。

### M2：前端读路径迁移

- 工作台 reducer 只读取 Snapshot；
- 旧 ModuleGraph 进入只读兼容视图；
- 删除格式驱动的隐式版本切换。

退出条件：四领域 E2E 和版本一致性通过。

### M3：写路径迁移

- 新建、修改、替换、材质、质量和导出全部写 AgentAsset；
- legacy Concept 不再接受产品 UI 写入；
- legacy 项目通过显式 hand-off 创建转换授权，再由 Agent 新方向生成候选；确认后原子提升为 Agent asset，不改写 legacy source。

退出条件：父数据不变、转换可重放、失败可回滚。

### M4：Provider 和 Job 迁移

- Agent Kernel 成为唯一会话入口；
- 迁移取消、恢复、SSE 和错误码；
- 旧 Weapon Job 只服务历史重放。

退出条件：重启恢复、超时、取消和幂等门通过。

### M5：发布门迁移

- G1–G7、Agent E2E、sidecar 和安装测试成为必需检查；
- Unity、ComfyUI、神经 3D 不再属于默认 release gate；
- legacy gate 移到手动兼容 workflow。

退出条件：新 release gate 不依赖旧产品环境。

### M6：删除 legacy 运行时代码

- 删除旧桌面状态和未使用路由；
- 删除旧 Provider 和文档；
- 保留数据读取工具、迁移器和归档 Schema；
- 进行数据库和对象库恢复演练。

退出条件：全新安装、升级安装和旧库只读转换通过。

## 4. 数据映射

| Legacy | 新合同 | 迁移方式 |
| --- | --- | --- |
| Weapon/Concept Project | Project | 保留原 ID 作为 provenance，创建新 Project ID |
| ConceptVersion | AgentAssetVersion candidate | 显式转换，不共用版本号 |
| ModuleGraph node | AssemblyGraph PartNode | 通过 adapter 复制稳定引用和 transform |
| Concept ChangeSet | AgentAsset ChangeSet | 不迁移活动预览，只迁移已确认结果 |
| Concept Quality | Legacy evidence | 不附着到新版本，转换后重新检查 |
| Concept Export | Legacy artifact | 保留 hash，只读下载，不作为新导出 |

## 5. 回滚

每个阶段发布前：

- 创建并验证 Library 备份；
- 记录 schema version、对象数量和 head；
- 保留向前迁移前的应用版本；
- 新迁移只追加表/列或生成新对象；
- 回滚应用时旧数据保持只读可打开；
- 不自动删除新版本和对象。

## 6. 禁止事项

- 在 UI 中用同一个 `vN` 表示两套版本；
- 用 localStorage 覆盖服务端版本头；
- 因旧测试失败而重新把 legacy 设为新产品真值；
- 删除旧数据来简化迁移；
- 在新 API 文档中继续添加 Weapon/Unity 专属入口。
