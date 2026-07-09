# release 证据目录规范

所有门禁执行都落到：

`output/release/<GATE-ID>/`

每次执行至少写入：

- `report.json`
- `trace.txt`
- `artifacts.txt`

建议每个门禁一个文件夹，例如：

- `output/release/GATE-01/report.json`
- `output/release/GATE-01/trace.txt`
- `output/release/GATE-01/artifacts.txt`

命名约定：

- `GATE-xx` 使用 `IMPLEMENTATION_PLAN.md` 的记录 ID。
- `report.json` 的主字段建议包含：`gate_id`、`status`、`blocker`、`warning`、`next_action`、`owner`、`next_owner`、`next_step_id`。
- `trace.txt` 为执行命令、关键输入、时间戳。
- `artifacts.txt` 记录截图路径、日志路径、产物路径（模型/ZIP/manifest）。

可复用模板文件见：

- `output/release/_TEMPLATE/report.json`
- `output/release/_TEMPLATE/trace.txt`
- `output/release/_TEMPLATE/artifacts.txt`
