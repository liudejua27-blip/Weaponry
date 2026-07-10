# R3 ChangeSet Audit Timeline Evidence

日期：2026-07-10

## 范围

证明 Project ChangeSet 时间线是可分页、搜索、过滤和恢复的权威审计记录，并持久化 preview 拒绝与 confirm stale 原因；当前筛选可批量归档为可验证 ZIP。它不证明 AI Change Planner 的真实模型生成质量，也不证明法规级审计存储。

## 合同与存储

- migration `0012_r3_change_set_audit.sql` 增加 `diagnostic_json` 与 Project/updated/id 索引；
- 排序：`updated_at DESC, change_set_id DESC`；
- opaque cursor 绑定 query/status/operation filter hash；
- `q` 搜索 ID、ChangeSet JSON、result Version 和 diagnostic；
- `operation` 使用 JSON1 检查真实 operations；
- rejected/stale diagnostic 包含 code、message、stage、recoverable、operation_ids、node_ids 和 recorded_at。
- migration `0015` 保存 user/planner actor、instruction、rationale、Provider provenance 与 planner Job；
- migration `0016_change_set_audit_exports.sql` 保存 Project 级归档、筛选、记录数、Manifest、`project_lifetime` 和 package asset；
- canonical `Records/change-sets.jsonl` 保存完整 ChangeSet、base/result Version、actor/provider/instruction/diagnostic/hash；CSV 是审阅投影；
- `ChangeSetAuditExportManifest@1` 固定排序并保存 README/JSONL/CSV 的 SHA-256、字节数和 MIME；ZIP 进入内容寻址对象存储；
- 应用不提供单包删除 API；超过 `max_records` 返回 `AUDIT_EXPORT_LIMIT_EXCEEDED`，不截断。

## 自动证据

```bash
npm run r3:change-set-audit-gate
```

后端 fixture：5 条主项目 ChangeSet 以 `limit=2` 返回 3 页，顺序一致且无重复；搜索、rejected、set_mirror、非法 cursor、cursor/filter mismatch 均验证；Connector conflict 与 locked descendant 共保存 2 条 rejected diagnostic，重启后仍可读取。另一路 smoke 验证 current Version 漂移保存 `CHANGE_SET_STALE` confirm-stage diagnostic。归档专项 smoke 同时保存 user/planner 两条记录，验证 JSONL/CSV、每个 Manifest hash、ZIP/header hash、幂等、1 条上限冲突、Job/artifact link 和 Agent 重启下载字节一致。

桌面 fixture：在真实工作台写入 2 条 confirmed、1 条 rejected、21 条 proposed，再确认 1 条 planner ChangeSet；首屏 20，加载更多后 24；rejected 搜索显示 locked node code/message/context；set_mirror 操作筛选只返回镜像记录。桌面导出 25 条归档，验证下载 ZIP/SHA、最近归档状态和 planner provenance；Agent 重启后列表记录数、package hash、下载字节和响应 hash 保持一致。

桌面截图：

```text
output/playwright/r3-change-set-audit-export.png
```

## 未覆盖

- `project_lifetime` 是应用级不可变快照，不是 WORM、legal hold、防篡改外部账本或独立灾备；
- 整库离线 backup/restore 演练、容量/保留成本与 Project 删除后的对象垃圾回收策略；
- HTML/PDF 人工签署报告；
- AI Change Planner 的真实模型准确率和解释质量。
