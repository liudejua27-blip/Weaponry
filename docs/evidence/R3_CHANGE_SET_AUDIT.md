# R3 ChangeSet Audit Timeline Evidence

日期：2026-07-10

## 范围

证明 Project ChangeSet 时间线是可分页、搜索、过滤和恢复的权威审计记录，并持久化 preview 拒绝与 confirm stale 原因。它不证明未来 AI Change Planner 的生成质量。

## 合同与存储

- migration `0012_r3_change_set_audit.sql` 增加 `diagnostic_json` 与 Project/updated/id 索引；
- 排序：`updated_at DESC, change_set_id DESC`；
- opaque cursor 绑定 query/status/operation filter hash；
- `q` 搜索 ID、ChangeSet JSON、result Version 和 diagnostic；
- `operation` 使用 JSON1 检查真实 operations；
- rejected/stale diagnostic 包含 code、message、stage、recoverable、operation_ids、node_ids 和 recorded_at。

## 自动证据

```bash
npm run r3:change-set-audit-gate
```

后端 fixture：5 条主项目 ChangeSet 以 `limit=2` 返回 3 页，顺序一致且无重复；搜索、rejected、set_mirror、非法 cursor、cursor/filter mismatch 均验证；Connector conflict 与 locked descendant 共保存 2 条 rejected diagnostic，重启后仍可读取。另一路 smoke 验证 current Version 漂移保存 `CHANGE_SET_STALE` confirm-stage diagnostic。

桌面 fixture：在真实工作台写入 2 条 confirmed、1 条 rejected 和 21 条 proposed；首屏 20，加载更多后 24；rejected 搜索显示 locked node code/message/context；set_mirror 操作筛选只返回镜像记录；Agent 重启后 rejected diagnostic 仍可按 API 搜索。

## 未覆盖

- actor/user/provider provenance；
- 审计报告批量导出；
- 长期归档与数据保留策略；
- AI Change Planner 的准确率和解释质量。
