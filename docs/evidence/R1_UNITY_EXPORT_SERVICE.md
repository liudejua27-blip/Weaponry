# R1 Unity Export Application Service Evidence

日期：2026-07-10

范围：证明 legacy Unity Export 的同步、排队、worker、Manifest 和 ZIP builder 已从 `SQLiteAssetStore` 迁入 application service，并保持旧游戏美术交接合同。它不证明 Unity Editor 已实际导入，也不代表 R1 完成。

## 边界变化

- `LegacyUnityExportService` 拥有 runtime 选择、幂等、输入验证和 model/spec/quality 资产解析；
- sync 与 worker 路径都创建不可变 export child Version、`UnityExportManifest@1` 和内容寻址 ZIP；
- worker 路径在 Manifest 与 package commit 前检查取消，避免取消后提交；
- Manifest 只记录相对 package path、SHA-256、byte size、MIME 和非制造边界；
- ZIP builder 包含 GLB、Unity material、可选 Spec/Quality、Manifest 与 README；
- `LegacyWorkerService` 直接注入 `complete_worker_job`，不再回调 facade export handler；
- facade 的 `export_unity`/`enqueue_export_unity` 只代理并映射旧错误；
- `asset_store.py` 从 2413 行降至 1819 行。

## 自动门

```bash
npm run r1:unity-export-gate
```

覆盖：

1. AST 断言 facade 不包含 export_packages SQL、ZIP builder 或 worker handler；
2. 同步 HTTP、幂等 replay 和 409 conflict；
3. 手动 work-once 与常驻 worker loop；
4. child export Version、package Asset、Manifest hash 和重启回读；
5. ZIP 包含 6 个预期条目且全部位于相对 package root；
6. Finder reveal dry-run 与内容寻址文件验证；
7. Unity preflight 可解析 GLB 与 Manifest。

## 环境边界

当前未配置 `WUSHEN_UNITY_EXECUTABLE` / `UNITY_EXECUTABLE`，因此 `unity:preflight` 报告 `blocked_unity_not_configured`。这不是代码失败，但不能作为真实 Unity batchmode import 成功证据；发布门仍须在安装 Unity 的环境运行 `npm run unity:import:gate`。

## 未证明

- Unity Editor batchmode 实际导入；
- `App.tsx` 前端业务控制器拆分；
- 新 Concept Export 的 DCC round-trip；
- 制造级 CAD/DFM。
