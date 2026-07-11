# R2 Concept Contracts Evidence

日期：2026-07-10  
范围：R2 当前切片，证明合同、Concept 数据迁移、Project/Version、ModuleGraph、ChangeSet、QualityRun、Brief/Variant、JobEvent@2 与概念源包 API；不证明桌面 E2E、combined GLB 或制造级 CAD。

## 产物

- 独立 `packages/concept-spec`，没有机械改名旧 `weapon-spec`；
- 八份 JSON Schema：Domain Profile、Concept Spec、Module Asset、Module Graph、ChangeSet、Quality Report、JobEvent@2、Concept Export Manifest；
- `forgecad_agent.domain.concepts` Pydantic 合同；
- 生成的 TypeScript 与 Python schema registry；
- `r2:contracts-gate` 和正/负向合同 smoke。
- migration `0009_r2_concept_domain.sql`；
- Project/Profile/Version Repository/UoW、application service 和 `/api/v1/projects`；
- 创建、列表、详情、追加版本、幂等冲突和重启恢复 HTTP smoke。
- 不可变 GLB 模块注册、内容哈希校验、列表/筛选和重启恢复；
- Connector 所属模块、类型和缩放范围校验；
- 仅持久化通过引用完整性检查的 ModuleGraph，失败图保留结构化 issue 而不入库。
- DesignChangeSet proposed → ghost preview → confirmed 状态机；
- preview 使用新 Graph ID，不覆盖父 Graph；确认后才创建子 Version 和正式 Graph；
- 保护节点修改在合同层拒绝，stale base 在提交时转为 `stale` 并返回冲突。
- version-scoped QualityRun/Findings 持久化、幂等 replay 和报告 round-trip。
- Brief interpreted/confirmed 状态、确定性 A/B/C Graph variant、唯一选择和重启恢复。
- 独立 Concept Job/Event 表；Brief、Variant、Graph validate、QualityRun、Export 使用 JobEvent@2，支持 JSON cursor replay、SSE 和重启恢复。
- `ConceptExportManifest@1`、源模块/Spec/Graph/Quality ZIP、逐文件 hash、artifact link、下载和重启恢复。

## 已验证不变量

- Pydantic `extra=forbid`，未知字段不能静默进入领域；
- ID、Connector slot、SHA-256、Transform 和 scale range 受约束；
- ModuleGraph 的 root 必须存在，所有节点必须可达，Connector endpoint 不能重复占用；
- ChangeSet 不能删除、替换或变换受保护节点；
- Finding 失败时 QualityReport 不能声称通过；
- 导出路径不得绝对化或包含 traversal，模块与文件条目不得重复；
- Python、TypeScript 与 schema registry 生成物可重复生成且无漂移。

## 命令与结果

```bash
npm run r2:contracts-gate
```

结果：通过。合同 smoke 验证 8 个正向合同和 5 个负向不变量。

```bash
npm run r2:gate
```

结果：通过。fresh database 应用 12 个 migration；HTTP smoke 创建 `weapon_concept` Project、追加不可覆盖父版本的 V2、验证幂等 replay/conflict，并在 Agent 重启后恢复项目与版本历史。新表不存在指向 `weapons`、`weapon_versions`、`creative_weapon_graphs` 或 `skill_graphs` 的外键。

同一门还注册 2 个 R2 GLB envelope fixture 和 3 个 Connector，持久化 1 个有效 ModuleGraph，拒绝并不保存引用缺失模块的无效 Graph；重启后 registry 和 Graph 均可回读。R2 fixture 只验证存储与引用协议，不代表 R3 的高质量美术资产。

ChangeSet smoke 将已验证 Graph 绑定到 V2，提出并预览局部比例/风格修改，确认后创建 V3 与新的正式 Graph；V2 和父 Graph 保持原值。另验证锁定核心修改被拒绝、当前版本推进后旧 preview 转为 `stale`。版本总数和 Graph 总数证明 preview 本身不写正式版本。

同一 smoke 为确认后的 V3 写入 1 个 `ModelQualityReport`、1 个 QualityRun 和 1 个 Finding，并通过 GET 完整回读。该切片只证明报告合同与数据链，不代表 R5 的实际网格检查算法已经完成。

Brief/Variant smoke 基于当前已验证 Graph 生成 X 比例分别为 `0.9 / 1.0 / 1.1` 的 A/B/C 三个候选，保证 Graph ID 唯一、Connector 校验通过，并将 B 标为唯一 `selected`；重启后选择状态保持。generator 明确记录为 `deterministic_template`，不作为 R4 AI 指标证据。

Brief 与 Variant 操作写入 2 个 completed Concept Job 和 5 个 `JobEvent@2`；连同前置 Graph validation，该 smoke 共 3 个 Job / 8 个事件。它验证完整查询、cursor 续读、SSE `concept.job.event` 以及 Agent 重启恢复，不使用旧 `generation_jobs`/`agent_events` 表。

ModuleGraph smoke 为有效和无效 Graph 各写入 1 个 `validate_graph` Job，共 6 个事件；无效 Graph 的 Job 表示验证过程成功完成，业务输出仍保持 `valid=false` 且不持久化 Graph。Quality smoke 写入 1 个 `quality_run` Job 和 3 个事件，幂等 replay 保持同一 Job ID。两类任务重启后均可回读。

Export smoke 为绑定 ModuleGraph 的版本生成 1 个 validated ZIP，包含 2 个不可变 GLB、Spec、Graph、最新 QualityReport、README 和 `ConceptExportManifest@1`。逐文件与整包 SHA-256、幂等 replay/conflict、artifact link、末事件 `artifact_asset_id`、下载和重启后回读均通过。该包尚不含 combined GLB、OBJ、PNG 或爆炸图。

Legacy 重启恢复提取另由以下命令验证：

```bash
.venv/bin/python scripts/smoke_p0_runtime_recovery.py
```

结果：通过；恢复、取消和 runtime job 样本均成功。

## 未完成

- 高质量 8–12 GLB Module Pack（当前工作台只绑定 4 个程序化 R3 fixture）；
- combined GLB、OBJ、PNG、爆炸图与实际 Mesh 检查器；
- C01–C10 完整发布门。
