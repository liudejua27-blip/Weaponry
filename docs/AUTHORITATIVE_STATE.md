# ForgeCAD 权威状态与版本真值

版本：2026-08-07
状态：目标合同；旧 `ConceptVersion/AgentAssetVersion` 不再是新 Runtime 真值

## 1. 真值层级

1. **Runtime V1 SQLite + CAS**：项目、候选、版本、Job、Skill、审批和工件唯一持久真值；
2. **公开 JSON Schema + canonical serialization**：对象合法性和 hash 规则；
3. **`ActiveDesignSnapshot`**：当前项目状态的单一只读投影；
4. **Worker receipts/readback**：对特定输入和工件 hash 的事实；
5. **Render/Quality evidence**：对特定 candidate/version 的检查；
6. **MCP/Viewer projection**：可丢弃、可重建的展示；
7. **Codex 对话/自然语言**：意图与解释，不是产品状态。

GLB、图片、`.blend`、Three.js scene、prompt、Skill 文档和 Codex 评价都不能单独成为版本头。

## 2. 核心对象

### Project

包含 project ID、名称、创建时间、policy/profile、active snapshot revision。无绝对本机路径和模型/Provider 信息。

### ReferenceEvidence

保存 CAS hash、MIME、尺寸、用户授权声明、导入方式、视图/相机 claims 和派生证据 lineage。原始绝对附件路径入 CAS 后丢弃。

### Candidate

未确认、可 GC 的完整构建单元，引用：base version、SubjectProfile、RepresentationPlan、AssemblyGraph、Geometry/Appearance programs、Skill receipts、artifacts/readback、RenderSet、QualityReport、SemanticChangeSet 和状态。

Candidate 状态：`prepared → compiling → evaluating → reviewable → confirmed | rejected | failed | expired`。只有 `reviewable` 且 hard gates 通过者可 confirm。

### DesignAssetVersion

不可变提交，至少包含：version ID、project、parent version、confirmed candidate hash、assembly/program/material/texture/artifact manifests、quality、approval、created_at 和 canonical digest。任何修改都创建新子版本。

### ActiveDesignSnapshot

```text
project_id
snapshot_revision
confirmed_version_id | null
review_candidate_id | null
runtime_capabilities_digest
selection_projection_revision | null
updated_at
```

Snapshot 不复制完整模型，不合并两套 `vN`，不按导出格式切换版本链。Viewer/localStorage 不能写它。

### RuntimeJob

持久 job ID、kind、project/candidate scope、request hash、state、event cursor、checkpoint/result/error refs 和取消状态。事件只追加、可重放；大内容只引用 CAS。

### SkillExecutionReceipt

绑定 Bundle/Recipe/Operator/Validator/asset/SBOM/signature hash、canonical input/output、预算和结果。不记录模型 prompt 或任意执行环境。

### ApprovalReceipt

由 Runtime 接收 Codex write approval 后创建，绑定 user-visible summary、tool、project、base version、prepared object ID/hash、quality report、expiry、decision 和 session。它不证明模型身份，只证明本地审批事务。

## 3. 写入不变量

- 只有 Runtime 进程持有 DB writer lease；
- `prepare` 不移动 confirmed head；
- `confirm` 在单一 SQLite 事务中校验 base/hash/quality/approval/idempotency、写版本、更新 snapshot、追加 audit；
- 同一 idempotency key + request hash 返回同一结果；同 key 不同 hash 拒绝；
- stale base 不自动 rebase 或 last-write-wins；
- rejected/failed/expired candidate 永不确认；
- 质量报告只能附着其 input artifact/candidate hash；
- export 只能引用 confirmed version，或明确标记为 unconfirmed diagnostic；
- CAS 对象以内容 hash 寻址，DB 事务提交前验证存在/尺寸/hash；
- GC 只能删除无 reachability 的临时候选工件，不能删除已确认版本、审批、audit 或其依赖。

## 4. 局部修改

`SemanticChangeSet` 必须引用 base version、Part/MaterialZone/source-map 稳定 ID 和 allowlisted operation。Runtime 校验 scope 后生成新 candidate，重新编译受影响 DAG 并复用未影响 hash。不能接受任意 JSON pointer、vertex buffer patch、脚本或路径。

Viewer selection 只是提示；prepare 时必须重新绑定当前 snapshot/part。Part 已不存在或版本漂移时返回 typed conflict。

## 5. Undo、Reject、Restore

- `undo/redo`：只作用于同一未确认 candidate 的 typed change stack；
- `reject`：终止 candidate，不改 confirmed version；
- `restore_prepare(version_id)`：从历史内容产生基于当前头的新 candidate；
- `restore_confirm`：批准后创建当前头的子版本，历史版本保持不变；
- 禁止移动数据库指针伪装新版本，禁止覆盖旧 GLB/CAS 对象。

## 6. 爆炸图

默认爆炸图是由 confirmed AssemblyGraph 派生的 `ExplodedViewPlan`。临时距离只存在 Viewer；保存计划必须产生 candidate/change/approval/version。Plan 引用稳定 Part ID，不能以渲染 primitive 顺序作为唯一身份。

## 7. 导出

`export_prepare` 生成 manifest 与候选文件，绑定 confirmed version、format/profile、artifact hashes、validator/readback、license/provenance 和 toolchain。用户批准后 `export_confirm` 原子写入授权目标，并返回最终文件 hash；导出目录不得成为版本真值。

如果 Viewer、candidate、quality、export 的 version/hash 不一致，导出 fail closed。导出包不包含绝对本机路径、secret、prompt、原始 Codex attachment path 或未授权资产。

## 8. 旧数据

旧 `ConceptVersion`、`ModuleGraph`、`AgentAssetVersion`、Thread/Turn/Item、Provider 和 migrations 仅属于只读归档。新 Runtime V1 不自动打开旧 DB，也不把旧 `vN` 投影为当前 snapshot。

一次性离线工具可以读取备份、校验旧工件、生成中立 export manifest，再由用户显式导入新项目。失败不修改旧库或新库。用户数据删除需要独立明确授权。

## 9. 重启与灾难恢复

重启时 Runtime：验证 DB migration/version → CAS reachability → writer lease → 非终态 Job checkpoint → snapshot/version hashes → Skill revocation。无法安全恢复的 Job 转为 typed failure；不得以猜测状态续跑。已确认版本必须在 MCP/Viewer 不可用时仍可离线备份和校验。
