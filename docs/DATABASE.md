# ForgeCAD Runtime V1 数据库

版本：2026-08-09
状态：MCP004 事务基座、MCP005 reference evidence/CAS、MCP007–009 3D artifact/render/quality/change/export lineage functional core 已实现

## 1. 断代策略

新 Runtime 使用独立 `migrations-runtime-v1/0001_runtime.sql`，不在旧 migration 上继续叠加。MCP002 Store 创建 schema marker、Project/Snapshot/Candidate/DesignAssetVersion/Job/Event/Checkpoint/Object/Artifact/Approval/Audit 表，并在事务中校验版本。已有数据库若没有 `schema_meta` 会直接拒绝，避免误打开旧 Library。旧 `WushenForgeLibrary`、SQLite 和 CAS 只读备份；新 Runtime 不自动打开、迁移或写入它们。Runtime 在打开 Store 前取得 OS 独占 writer 文件锁。

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
write_idempotency
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

只有 `forgecad-runtime` 取得 SQLite/CAS 写权限。MCP、Viewer、Workers 不持有 DB handle。SQLite 使用 WAL、foreign keys、busy timeout、明确 transaction boundary 和 schema version。Runtime 进程持有 OS 独占 writer 文件锁；第二实例立即返回 `RUNTIME_BUSY`，进程退出或崩溃后由操作系统释放锁。MVP 不使用数据库 TTL lease 或 heartbeat。

永久 confirm 在一笔事务中：验证 base/candidate/quality/approval/idempotency/CAS → 写 immutable version → 更新 snapshot → 写 audit。restore 以历史 confirmed version 为内容来源、以当前 head 为父，永不移动历史指针；diagnostic export 只写 hash-bound manifest/CAS 和 export receipt，不写本机路径。事务失败无部分版本；相同 idempotency key 重放原结果，不同 request hash 拒绝。MCP004 核心通过 authenticated IPC 提供，并由显式 opt-in wire adapter 转发；默认 MCP stdio 仍保持 MCP003 只读。

## 4. CAS

对象按 SHA-256 寻址，DB 保存 `sha256`、size、MIME、kind、created_at、reachability class。`object_path` 只在 Store 内部派生，不保存用户路径。写入采用临时文件、fsync、hash recheck、原子 rename；同 hash 不同字节、容量超限、缺失或篡改均 fail closed。MCP002 已通过 CAS roundtrip、capacity、hash mismatch、corruption 和 backup/restore tests；GC/reachability policy 留给 MCP011。

事件和日志不存大内容。GC 先计算 reachability，再 quarantine，经过 grace period 和 manifest 复验后删除；confirmed version、audit、approval、export 和旧库备份不可 GC。

MCP005 的图片原始字节直接进入 CAS；数据库只保存 opaque reference ID、CAS sha256、MIME、byte size、width/height、授权声明和派生 object refs，不保存 attachment absolute path。MCP007–009 的 geometry/GLB/render/quality 也只保存 CAS refs 与 lineage。新增 migration 必须在 Runtime 取得 OS 文件锁之后执行。

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
