# R1 Generate-3D Application Service Evidence

日期：2026-07-10

范围：证明旧 `POST /api/weapons/{weapon_id}/generate-3d` 的 runtime 选择、同步编排和排队事务已从 `SQLiteAssetStore` 迁入 application service，并保持追加版本、ProviderTask、Checkpoint、JobEvent 和错误合同。异步 worker 随后已迁入 `LegacyWorkerService`，前端边界见 `R1_FRONTEND_COMPOSITION.md`。

## 边界变化

- `LegacyGenerate3DService.generate_3d` 根据当前环境选择 sync 或 worker queue；
- 同步路径验证源 Version/Image，生成 `ModelGenerationInput@1`、rough GLB variants、质量报告、ProviderTask、Checkpoint 和 child Version；
- queue 路径只创建 queued Job/Steps/Event，不提前创建模型、资产或 Version；
- service 使用注入的 connection factory、asset read/write、rough model、task/checkpoint/event 端口；
- facade 的 `generate_3d` 与 `enqueue_generate_3d` 只代理并映射幂等/业务错误；
- `asset_store.py` 从 3272 行降至 3052 行。

## 自动门

```bash
npm run r1:generate3d-gate
```

覆盖：

1. AST 断言 facade 不包含 runtime 环境判断、rough model 写入或 JobEvent 编排；
2. 同步 HTTP 创建 child `rough_3d` Version，并保持幂等 replay/409 conflict；
3. 异步 queue 不提前创建 Version、Model 或 rough assets；
4. worker 成功、恢复重试、唯一提交与连续事件序号；
5. ProviderTask submit/poll/fetch/cancel 与 Checkpoint 状态；
6. 常驻 worker loop 自动完成 queued Job；
7. Local HTTP Provider 和本地 3D runtime wrapper 的成功、等待与取消路径；
8. 内容寻址资产库无 blocker。

## 未证明

- 本专项门本身不证明前端组合边界；该边界由后续 `r1:frontend-composition-gate` 证明；
- 正式 GPU 模型生成质量或制造级 CAD/DFM。
