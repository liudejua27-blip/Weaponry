# R6 Packaging Readiness Evidence

日期：2026-07-12

范围：证明 Tauri 发布门不会将空的“存在即通过” sidecar 当作可发布 Agent，并记录 macOS 本机 Tauri 联调证据。本页仍不证明应用签名、安装/卸载或无源码干净机 C10 E2E。

## 已实现

- `release:packaging-readiness` 校验 Tauri bundle、图标、CSP、capability、`Cargo.lock`、sidecar 候选文件和文档入口；
- 除文件存在外，还要求 sidecar 非空，非 Windows 目标具有 owner execute bit，并按文件名验证 Mach-O、ELF 或 PE/MZ 头；
- `release:packaging-readiness-smoke` 在临时目录中验证空 sidecar 被拒绝，而起始头与权限正确的 macOS/Linux/Windows 小型 fixture 被接受；
- README 和 [打包合同](../PACKAGING.md) 明确指向该门，且不把 Vite 开发壳当作发布应用。
- 2026-07-12 已在 Apple Silicon macOS 使用完整 `Cargo.lock` 构建 `.app`；本机 `local-dev-python` Agent 下验证应用启动、`tauri://localhost` 到 Agent 的 CORS、工作台加载、Unity ZIP 导出与重启恢复。

## 当前结果

```bash
npm run release:packaging-readiness
```

门禁按预期失败：`SIDECAR_BINARY_INVALID`。仓库中的四个 target-suffixed `wushen-agent` 文件均为 0 bytes；macOS/Linux 候选还缺少 execute bit。因此它们不能被混同为已冻结的 Agent 二进制。

```bash
.venv/bin/python scripts/smoke_release_packaging_readiness.py
```

返回 `empty_sidecar_rejected: true` 与 `target_headers_validated: true`。

## 下一个发布环境步骤

1. 使用包含所需 Python runtime、migrations 和 schema 的冻结 Agent 生成每个发布 target 的二进制。
2. 以与 target 匹配的可执行 Mach-O/ELF/PE 替换占位文件，不提交 API key 或本机 Library。
3. 在对应平台运行 `npm run desktop:tauri-check` 和 Tauri bundle，然后完成签名、安装/卸载、无源码干净机的 Brief 到导出 E2E。
