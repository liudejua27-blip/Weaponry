# R4 Planner 固定评测证据

日期：2026-07-10

## 数据集与门槛

权威输入：`evaluations/r4/planner_truth_set.json`

```text
20 Brief cases
20 A/B/C variant cases
20 valid Change cases
20 locked-root probes
```

阈值：Brief ≥90%、三方案结构差异 100%、Change ≥85%、lock preservation ≥95%。报告保存 truth-set SHA-256、逐例结果、p50/p95/max latency、input/output/total tokens 和证据资格。

## 本轮实际结果

```bash
npm run agent:r4-evaluation-baseline
```

结果：

```text
brief_success_rate       1.0
variant_distinct_rate    1.0
change_success_rate      1.0
lock_preservation_rate   1.0
explainability_rate      1.0
provider_call_records    80
calls_with_token_usage   0
live_provider_run        false
real_provider_evidence_eligible false
```

首次 baseline 暴露明确数值 Brief 只能通过 40%；规则解释器随后补齐合法长度、主体高度、握持角和细节百分比解析，同一固定数据集复跑达到 100%。这证明评测器能够发现回归，也证明 fallback 的确定性合同；它不证明模型质量。

严格 live 命令：

```bash
npm run agent:r4-evaluation-live
```

当前环境实际返回非零状态和 `EVAL_PROVIDER_NOT_CONFIGURED`。没有使用 deterministic fallback，也没有生成虚假 token/latency 证据。

## Provider 遥测证据

OpenAI-compatible Adapter 读取常见 `prompt_tokens/completion_tokens` 或 `input_tokens/output_tokens`，将实际 HTTP elapsed time 和 usage 写入 `ConceptPlannerProvenance`。fake HTTP smoke 已分别验证 Brief/Variant 和 Change 请求的 latency、input/output/total token；Provider 不返回 usage 时字段为 `null`。

`agent:r4-evaluation-smoke` 使用纯内存 synthetic telemetry 跑满 80 个阶段样本，验证完整 token coverage 时的 eligible 判定分支和 `--confirm-live-provider` 防误调用门。该 smoke 标记 `synthetic_only=true`，不写真实评测报告、不调用网络，也不计入模型质量。

## 证据资格规则

只有同时满足以下条件，评测报告才可设置 `real_provider_evidence_eligible=true`：

- `configured_provider` 且 `live_provider_run=true`；
- 使用完整 20/20/20/20 数据集；
- 预期 80 次 Provider 调用全部留下记录；
- 每次调用均返回 token usage；
- 四项发布阈值全部通过。

当前没有满足上述条件，因此 R4 的真实 AI 指标仍未完成。下一步需要用户在本机 secret file/环境变量中配置 Provider，并明确授权这 80 次可能付费的调用。
