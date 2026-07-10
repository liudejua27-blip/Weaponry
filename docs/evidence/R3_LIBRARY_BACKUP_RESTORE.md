# R3 Library Backup / Restore Evidence

日期：2026-07-10

## 范围

证明 ForgeCAD Library 的 SQLite、legacy/Concept 内容寻址资产和 ChangeSet `project_lifetime` 审计归档可以形成可校验备份，并恢复到全新目录后由真实 Agent 回读。该证据是隔离 reference fixture，不代表加密异地备份、WORM、legal hold 或正式资产规模性能。

## 已证明

- `scripts/library_backup.py backup` 使用 SQLite Backup API，而不是直接复制活动中的 WAL 数据库；
- 快照固定转换为独立 `journal_mode=DELETE` 的 `library.db`，输出不依赖 `library.db-wal/-shm`；
- `asset_files` 与 `concept_assets` 引用合并并按 object path 去重，soft-deleted row 仍属于数据库引用；
- `ForgeCADLibraryBackupManifest@1` 保存 16 个 migration、关键表行数、数据库/对象 SHA-256 与 byte size，以及逻辑/物理/去重/未引用候选容量；
- 备份排除 Provider secret/config、WAL/SHM、trash/cache 和未引用对象候选；未引用对象只报告，不擅自删除；
- `verify` 重新运行 SQLite integrity/FK、数据库引用集合、Manifest 元数据和文件 hash/size 校验；
- 模拟未来新增未知 `object_path` 表时，旧 CLI 以 `UNSUPPORTED_OBJECT_REFERENCE_TABLE` 失败，不静默漏备；
- 任一对象被追加 `tamper` 后验证以 `BACKUP_SIZE_MISMATCH` 失败，额外塞入未登记对象则以 `BACKUP_OBJECT_FILE_SET_MISMATCH` 失败；
- `backup` 拒绝写回源 Library 内部；`restore` 拒绝写入备份内部或覆盖已有目录，在临时目录验证完成后原子落位，并保存来源 Manifest；
- 恢复 Agent 能回读 Project/Version、两个 Module、Planner JobEvent 和同 SHA-256 的 ChangeSet 审计 ZIP。

## 自动证据

```bash
npm run agent:r3-library-backup-restore-smoke
npm run agent:r3-library-recovery-drill-smoke
npm run r3:library-backup-gate
npm run r3:change-set-audit-gate
```

当前隔离容量样本：

```text
database_bytes                 about 659–663 KB
reference_rows                5
unique_object_count           4
unique_object_bytes           about 3.0 KB
logical_object_bytes          about 3.2 KB
deduplicated_bytes            160
backup_payload_bytes          about 663–666 KB
source_object_store_files     5
unreferenced_candidates       1 / 29 bytes
```

对象 ZIP 包含动态 ID/时间，因此少量字节波动属于预期；断言固定的是引用/对象/候选数量、去重关系、hash 一致性和恢复行为。

## 未证明

- 正式 10–12 或 24–30 模块资产库的备份、验证、恢复耗时和峰值磁盘占用；参考 Pack 的演练基线见 [R3_LIBRARY_RECOVERY_DRILL.md](R3_LIBRARY_RECOVERY_DRILL.md)；
- 定时调度、增量备份、远端复制、加密、密钥托管、WORM、legal hold；
- Project 删除后的 reference-aware garbage collection；
- 跨操作系统或未来较新 schema 向旧二进制恢复。
