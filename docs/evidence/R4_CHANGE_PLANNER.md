# R4 Change Planner 纵向切片证据

日期：2026-07-10

## 已证明

- `ConceptPlannerProvider.plan_change_set` 同时有 `deterministic_rules` 与 OpenAI-compatible strict JSON Schema 实现。
- Planner 只输出 `replace_module`、`set_mirror`、`set_style`、`set_parameter`；服务端再次校验 node、root/lock、同类别 registry module、路径白名单、类型与 no-op。
- `POST /api/v1/versions/{version_id}/change-sets:plan` 只创建 proposed ChangeSet；它不创建 Version。
- `preview` 继续复用既有 Connector remap/snap、Graph validation 与锁定保护，桌面以半透明青色 ghost 显示。
- 用户只能在 ghost 后显式 `confirm` 创建 child Version，或 `reject` 保持当前 Version 并保存 `CHANGE_SET_DISCARDED`。
- migration `0015` 保存 `user|planner` actor、instruction、rationale、`ConceptPlannerProvenance` 和 `concept_change_plan` Job ID；API、桌面时间线和 Agent 重启均能回读。
- `auto` 外部 Provider 失败会显式记录 attempted provider/warning/fallback；`configured_provider` 不降级。

## 自动证据

```bash
npm run agent:r1-foundation-smoke
npm run agent:r4-change-planner-smoke
npm run agent:r4-change-planner-api-smoke
npm run desktop:r3-concept-workbench-smoke
npm run r4:planner-gate
```

重点结果：

- 15 个 migration fresh/repeat apply；旧 Job/Event 数据在 `concept_change_plan` CHECK 扩展时保留；
- fake OpenAI-compatible 请求使用 strict JSON Schema，顶层和 operation 字段均 required；
- safety prompt、未注册 Module、锁定/root、同类替换和 strict failure 语义通过；
- 确定性 API 生成 `replace_module + set_parameter + set_style`，ghost 不改变 current Version；
- confirm 创建唯一 child Version，reject 不创建 Version；
- planner provenance、JobEvent 与 diagnostic 在重启后恢复；
- 桌面真实调用 `:plan → :preview → :confirm`，参数从 `230/68` 预览为 `218/84`，确认后成为 V5，并在重启后恢复。

桌面证据：

```text
output/playwright/r4-change-planner-ghost-preview.png
```

## 未证明

- fake Provider 和确定性规则不证明真实模型理解质量。
- 尚未用固定真实 Provider truth set 证明 Brief ≥90%、AI 修改成功率 ≥85%、锁定模块保持率 ≥95% 或三方案差异度。
- Change Planner 不允许 AI 自由生成 add/remove/connect/disconnect/set_transform；这些操作仍只能走显式结构化客户端或后续经过单独约束设计。
- 方案 Variant 选择仍是 Planner 预览，不自动提交为 Version。
- 本证据不证明功能性武器工程、制造可行性、结构强度、弹道、安全认证、STEP/3MF 或完整 DFM。
