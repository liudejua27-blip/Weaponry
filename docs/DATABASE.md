# ForgeCAD Runtime V1 数据库

版本：2026-08-08
状态：部分实现；MCP002 Runtime V1 Store/CAS/lease/backup Gate 与 MCP003 只读 projections 已通过，候选确认与长期 Job 在后续任务

## 1. 断代策略

新 Runtime 使用独立 `migrations-runtime-v1/0001_runtime.sql`，不在旧 migration 上继续叠加。MCP002 Store 创建 schema marker、writer lease、Project/Snapshot/Candidate/DesignAssetVersion/Job/Event/Checkpoint/Object/Artifact/Approval/Audit 表，并在事务中校验版本。已有数据库若没有 `schema_meta` 会直接拒绝，避免误打开旧 Library。旧 `WushenForgeLibrary`、SQLite 和 CAS 只读备份；新 Runtime 不自动打开、迁移或写入它们。

一次性离线导出器可在后续任务读取旧库、校验工件并生成中立 manifest；用户显式选择后再导入新项目。失败不得修改任一数据库。删除旧用户数据需要单独明确授权。

## 2. V1 表域

```text
projects
active_design_snapshots
reference_evidence
candidates
semantic_change_sets
design_asset_versions
approval_receipts
runtime_jobs
runtime_job_events
runtime_job_checkpoints
objects
artifact_manifests
material_graphs
texture_sets
uv_layouts
render_sets
quality_reports
skill_bundles
skill_execution_receipts
export_manifests
audit_events
```

不创建 threads、turns、items、conversations、Provider registry/budget/snapshot、coding/search/vision/remote-3D jobs、legacy concept/module/weapon 表。

## 3. 单写者与事务

只有 `forgecad-runtime` 取得 writer lease。MCP、Viewer、Workers 不持有 DB handle。SQLite 使用 WAL、foreign keys、busy timeout、明确 transaction boundary 和 schema version。Lease 保存 owner、token hash、acquired/heartbeat 时间；活动租约拒绝第二 writer，TTL 过期后才允许 crash recovery，Runtime Drop 会释放正常退出的租约。

永久 confirm 在一笔事务中：验证 base/candidate/quality/approval/idempotency/CAS → 写 immutable version → 更新 snapshot → 写 audit。事务失败无部分版本。

## 4. CAS

对象按 SHA-256 寻址，DB 保存 `sha256`、size、MIME、kind、created_at、reachability class。`object_path` 只在 Store 内部派生，不保存用户路径。写入采用临时文件、fsync、hash recheck、原子 rename；同 hash 不同字节、容量超限、缺失或篡改均 fail closed。MCP002 已通过 CAS roundtrip、capacity、hash mismatch、corruption 和 backup/restore tests；GC/reachability policy 留给 MCP011。

事件和日志不存大内容。GC 先计算 reachability，再 quarantine，经过 grace period 和 manifest 复验后删除；confirmed version、audit、approval、export 和旧库备份不可 GC。

## 5. 路径与隐私

数据库不保存原 Codex attachment path、用户名、HOME、workspace absolute path、secret、prompt 或图片原始字节。导出包保证 `no export package contains absolute local paths`，只包含相对逻辑路径和 CAS/manifest hash。

## 6. 迁移、备份、恢复

- migration 单向、事务化、可在副本测试；
- 升级前 consistent DB snapshot + CAS manifest；
- 恢复先在隔离 Library 验证 integrity/reachability/version，再原子切换；
- migration 失败保持旧版本可读，不能半升级；
- 灾难恢复定期证明全量和增量备份，见 `DISASTER_RECOVERY.md`。

## 7. Gate

并发双 writer、kill -9、磁盘满、CAS missing/corrupt、WAL recovery、duplicate idempotency、stale base、migration failure、backup/restore、GC race 和路径泄露均必须自动测试。
