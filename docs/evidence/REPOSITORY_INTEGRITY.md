# 仓库完整性与 CI 基线

日期：2026-07-11

## 复核结论

远端 `main` 在 `2683188` 已包含 `cad-workbench`、`forgecad_agent`、Concept schema、`0009`–`0017` migrations、Operations/Evidence 文档与 R1–R4 门禁。此前“README 已重构但运行时代码缺失”的静态审计对应的是早于 PR #1/#2 的远端快照，不能再作为当前 `main` 的结论。

## 自动防回归

- `npm run repository:integrity` 校验关键路径、声明脚本、App/API 接线标记及所有本地 Markdown 链接。
- `Repository Integrity` 工作流在 PR 和 `main` push 上运行完整性与生成合同检查。
- `ForgeCAD Core` 工作流运行后端 lint/contract/registry/asset-catalog/first-run smoke、桌面 typecheck/build 与 Workbench E2E，并上传 Playwright 证据。CI 的 Headless Chrome 使用 API bytes 回读补充下载事件校验；本机 smoke 默认仍要求真实浏览器 download 事件。
- `Tauri Preflight` 在 Rust 相关修改或手动触发时对 Linux/macOS/Windows 执行 `cargo check`。
- `Security Baseline` 对每个 PR 执行密钥、Tauri CSP 和资产路径边界检查；许可证/SBOM 是手动 release audit，因为当前外部模型、DCC 与 Unity 运行时许可证审阅仍是明确 blocker，不能伪装为通过的 PR 状态。

## 本机验证

```bash
npm run repository:integrity
npm run contracts:types:check
.venv/bin/ruff check apps/agent
npm run agent:r2-concept-contracts-smoke
npm run agent:r2-module-registry-smoke
npm run agent:r3-asset-catalog-smoke
npm run agent:r3-first-run-workbench-smoke
npm run desktop:typecheck
npm run desktop:build
```

以上命令在当前基线通过。K003 完成后，`r2-module-registry`、`r3-asset-catalog` 与 `r3-first-run-workbench` 命令已经迁移为 Rust Core 测试：前两者验证 legacy 组件目录只读、分页、GLB 回读与零写入，后者验证 Rust 项目 bootstrap 不创建 legacy 版本。它们不再通过 Python Agent 写入产品状态。WorkBench E2E 由独立工作流运行，以避免在每次基础完整性检查中重复启动浏览器。

`npm run release:license-sbom` 当前会如实失败并列出待审外部依赖；它是发布阻断条件，不是 CI 配置失败。
