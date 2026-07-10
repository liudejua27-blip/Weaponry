# R2 Concept Contracts Evidence

日期：2026-07-10  
范围：R2 当前切片，证明合同、生成类型、Concept 数据迁移和 Project/Version 最小 API；不证明 ModuleGraph API、GLB 模块或桌面 E2E。

## 产物

- 独立 `packages/concept-spec`，没有机械改名旧 `weapon-spec`；
- 七份 JSON Schema：Domain Profile、Concept Spec、Module Asset、Module Graph、ChangeSet、Quality Report、JobEvent@2；
- `forgecad_agent.domain.concepts` Pydantic 合同；
- 生成的 TypeScript 与 Python schema registry；
- `r2:contracts-gate` 和正/负向合同 smoke。
- migration `0009_r2_concept_domain.sql`；
- Project/Profile/Version Repository/UoW、application service 和 `/api/v1/projects`；
- 创建、列表、详情、追加版本、幂等冲突和重启恢复 HTTP smoke。

## 已验证不变量

- Pydantic `extra=forbid`，未知字段不能静默进入领域；
- ID、Connector slot、SHA-256、Transform 和 scale range 受约束；
- ModuleGraph 的 root 必须存在，所有节点必须可达，Connector endpoint 不能重复占用；
- ChangeSet 不能删除、替换或变换受保护节点；
- Finding 失败时 QualityReport 不能声称通过；
- Python、TypeScript 与 schema registry 生成物可重复生成且无漂移。

## 命令与结果

```bash
npm run r2:contracts-gate
```

结果：通过。合同 smoke 验证 7 个正向合同和 4 个负向不变量。

```bash
npm run r2:gate
```

结果：通过。fresh database 应用 9 个 migration；HTTP smoke 创建 `weapon_concept` Project、追加不可覆盖父版本的 V2、验证幂等 replay/conflict，并在 Agent 重启后恢复项目与版本历史。新表不存在指向 `weapons`、`weapon_versions`、`creative_weapon_graphs` 或 `skill_graphs` 的外键。

Legacy 重启恢复提取另由以下命令验证：

```bash
.venv/bin/python scripts/smoke_p0_runtime_recovery.py
```

结果：通过；恢复、取消和 runtime job 样本均成功。

## 未完成

- Module/Connector Repository、注册与列表 API；
- Version DAG、ChangeSet preview/commit；
- GLB fixture、工作台真实 ModuleGraph 绑定；
- C01–C10 完整发布门。
