# R2 Concept Contracts Evidence

日期：2026-07-10  
范围：R2 第一切片，仅证明合同、生成类型和核心内存不变量；不证明数据库、API、GLB 模块或桌面 E2E。

## 产物

- 独立 `packages/concept-spec`，没有机械改名旧 `weapon-spec`；
- 七份 JSON Schema：Domain Profile、Concept Spec、Module Asset、Module Graph、ChangeSet、Quality Report、JobEvent@2；
- `forgecad_agent.domain.concepts` Pydantic 合同；
- 生成的 TypeScript 与 Python schema registry；
- `r2:contracts-gate` 和正/负向合同 smoke。

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

Legacy 重启恢复提取另由以下命令验证：

```bash
.venv/bin/python scripts/smoke_p0_runtime_recovery.py
```

结果：通过；恢复、取消和 runtime job 样本均成功。

## 未完成

- R2 migration、Repository/UoW 和 `/api/v1/projects`；
- Module/Connector registry 持久化；
- Version DAG、ChangeSet preview/commit；
- GLB fixture、工作台真实 ModuleGraph 绑定；
- C01–C10 完整发布门。
