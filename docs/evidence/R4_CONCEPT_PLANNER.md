# R4 Concept Brief / Module Planner Evidence

日期：2026-07-10

范围：证明 Brief Interpreter 与 Module Planner 已有可配置 Provider 边界、结构化输出、注册表约束、显式降级、持久化 provenance 和真实桌面操作链。它不证明任何真实模型的 Brief 成功率、三方案审美质量、自然语言 Change Planner 或自主制造能力。

## 实现边界

- `deterministic_rules` 会实际解析紧凑/延展、精密/简洁、颜色和对称等有限视觉词汇，输出有界 WeaponConceptSpec；它被明确标记为 deterministic，不冒充 AI；
- `openai_compatible` 只接收清洗后的 Brief、Spec、Graph 和注册模块摘要，通过 strict JSON Schema 返回 Brief patch 或三条 Variant Plan；system prompt 排除真实机构、制造、加工、装配、性能和弹药指令；
- `auto` 外部失败后允许规则降级；provenance 同时保存实际 provider 与 attempted provider/model、warning、fallback 标记、输入/输出 SHA-256 和当时 registry ids；
- `configured_provider` 外部失败直接返回 502 类 Planner 错误，不静默回退；
- Variant Plan 只能修改未锁定非 root 节点，scale 限制为 `0.85–1.15`，推荐 module id 必须存在于 Profile registry，三条 target/scale signature 必须不同；完整 ModuleGraph 仍经过 Connector/lock/Graph 校验；
- migration `0014` 为 Brief/Variant 增加 provenance、module recommendations 与 rationale，并给旧模板数据写入明确 legacy provenance；
- 桌面输入 Brief 后真实调用 interpret/generate API，展示三条方案和 generator；选择方案切换视口预览并更新 selected/rejected，但明确不创建子版本。

## 自动门

```bash
npm run r4:planner-gate
```

证据包括：

1. `寒地、紧凑、工业、石墨灰、精密细节` 把整体长度从 `230` 解释为 `207 mm`，detail density 解释为 `0.82`；
2. deterministic A/B/C 结构 scale 分别为 `[0.9,0.96,0.96]`、`[1.0,0.94,1.0]`、`[1.1,1.04,1.04]`；
3. 三条方案均包含非空注册 module recommendations、rationale 和 provenance hash；选择、JobEvent、数据库规范化与 Agent 重启恢复通过；
4. fake OpenAI-compatible HTTP server 收到两次 strict JSON Schema 请求，Brief patch 与三 Variant Plan 均通过 Pydantic；安全 system prompt 被断言；
5. synthetic timeout 在 `auto` 下记录 attempted provider 并降级，在 `configured_provider` 下保留错误；外部方案引用未注册 module id 会被 `PLANNER_BAD_OUTPUT` 拒绝；
6. 浏览器 E2E 真实执行 Brief → 参数面板同步 `207/82` → 三方案 → 选择 B → 视口预览，随后原有质量检查、20 轮视口生命周期、导出和重启恢复继续通过；
7. Planner 页面截图：`output/playwright/r4-concept-planner-variants.png`。

## 未完成

- 固定 20/20/20/20 truth set、评测器和 latency/token 采集已见 `R4_PLANNER_EVALUATION.md`；仍需在真实配置 Provider 上运行并达到 ≥90% Brief 等发布阈值；
- Change Planner 把自然语言修改编译成 DesignChangeSet，并复用现有 ghost preview、lock、stale 与 confirm 子版本链；
- 当前 module recommendation 是已注册候选列表，不会自动替换模块；不满足需求时的局部生成 Job 仍未接入；
- Provider 调用尚未 worker 化，也没有取消、重试、partial success 与 readiness；
- 正式 Blender 资产和 Beta 用户评测仍未开始。
