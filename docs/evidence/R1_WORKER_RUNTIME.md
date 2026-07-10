# R1 Legacy Worker Runtime Evidence

日期：2026-07-10

范围：证明 legacy worker 的 job claim、lease、dispatch 与 Generate-3D Provider 执行已从 `SQLiteAssetStore` 迁入 application service，同时保持 Generate-3D 和 Unity Export worker 行为。Unity 打包 handler 随后已迁入 `LegacyUnityExportService`，见 `R1_UNITY_EXPORT_SERVICE.md`；R1 仍未完成。

## 边界变化

- `LegacyWorkerService.run_worker_once` 查询并原子 claim queued/retrying/waiting_provider Job；
- service 管理 runner lease、首次开始事件、Generate-3D/Unity dispatch 和结构化失败落库；
- Generate-3D handler 管理 draft Version、模型输入、Provider submit/poll/fetch/cancel、ProviderTask、Checkpoint、质量资产与唯一 commit；
- `waiting_provider` 会释放 runner/lease，后续 poll 从持久化 task/checkpoint 恢复；
- 取消请求在 fetch 和 commit 前重复检查，避免取消后写入模型或提交 Version；
- Unity worker handler 通过显式 callable port 注入，现由 `LegacyUnityExportService` 提供；
- `SQLiteAssetStore.run_worker_once` 只剩单行 service 代理；
- `asset_store.py` 从 3052 行降至 2413 行。

## 自动门

```bash
npm run r1:worker-gate
```

覆盖：

1. AST 断言 facade 不包含 generation_jobs 查询、waiting_provider 或 Generate-3D handler；
2. queued/retrying claim、lease 和无任务返回；
3. Provider submit → polling → fetch → succeeded；
4. cancel_requested、Provider cancel 与取消后禁止 commit；
5. 中断恢复、retry-from-step 和唯一 child Version/Model；
6. 常驻 worker loop；
7. Unity Export 手动 worker 与 loop dispatch；
8. Runtime recovery、JobAction、ProviderTask、Checkpoint 和事件连续性。

## 未证明

- `App.tsx` 前端业务控制器拆分；
- 多进程高并发 worker 和正式压力阈值；
- 新 Concept jobs 的异步 worker 化。
