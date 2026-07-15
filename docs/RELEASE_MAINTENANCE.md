# ForgeCAD 发布维护手册

版本：2026-07-13
适用对象：版本维护者、CI 维护者和发布负责人

## 1. 当前发布状态

当前仓库可以构建本机 Tauri 测试 `.app`，但不能发布生产安装包：

- supervisor 仍使用 `local-dev-python`；
- 四个平台 `wushen-agent-*` sidecar 是空占位文件；
- `FGC-P008` 已提供无密钥 `ForgeCADPackagedSidecarInput@1` 与结构预检；当前 macOS arm64 正确报告 `blocked_missing_sidecar`，不会把空文件伪装成可运行工件；
- 工作台核心 Snapshot E2E 当前 Agent-first 路径已通过（含不可变 Agent 回退/前进）；完整并发和原生安装 E2E 仍未完成；
- 新旧版本真值尚未统一；
- 真实 Provider truth set 未完成；
- macOS/Windows 签名和安装验证未完成。

完整阻断清单见 [PRODUCTION_RELEASE_CHECKLIST.md](PRODUCTION_RELEASE_CHECKLIST.md)。

## 2. 发布候选的 Git 条件

发布候选必须：

- 工作区干净；
- 所有交付文件已提交并推送；
- PR 指向当前候选 commit，而不是旧绿色快照；
- 必需 CI 检查针对同一 commit；
- 生成文件和 Schema 无漂移；
- 不包含 API Key、私有绝对路径或本机输出目录。

## 3. 必需验证

```bash
npm ci
.venv/bin/pip install -e "apps/agent[dev]"

npm run agent:check
npm run contracts:types:check
npm run desktop:typecheck
npm run desktop:build
npm run desktop:tauri-check

npm run agent:g1-kernel-smoke
npm run agent:g2-contracts-smoke
npm run agent:d1-domain-inference-contract-smoke
npm run agent:d2-domain-inference-service-smoke
npm run agent:g3-shape-program-smoke
npm run agent:g4-mechanical-planner-smoke
npm run agent:g5-geometry-worker-smoke
npm run agent:g801-shape-primitive-smoke
npm run agent:g802-profile-extrude-smoke
npm run agent:g803-revolve-smoke
npm run agent:g804-transform-arrays-smoke
npm run agent:g805-boolean-smoke
npm run agent:g806-bevel-surface-panel-smoke
npm run agent:g807-blockout-diversity-smoke
npm run agent:g6-segmentation-smoke
npm run agent:g6-material-catalog-smoke
npm run agent:g6-asset-editing-smoke
npm run agent:g6-component-registry-smoke
npm run agent:g7-external-glb-import-smoke
npm run agent:unit
npm run desktop:r3-concept-workbench-smoke

npm run release:safety-scope
npm run release:secrets-files
npm run release:docs-walkthrough
npm run release:license-sbom
npm run release:packaging-readiness
```

依赖审计由 CI `dependency-audits` job 执行，原始 npm/Python/Rust JSON 位于 `dependency-audit-reports` artifact；审计高危项会阻断该 job。当前锁定的 Python/Rust 依赖审计均无漏洞，但依赖升级后必须重新执行，不能用离线 smoke 替代。

任何失败都是阻断，不得只在发布说明中标记为“已知问题”后继续外部分发。

## 4. Packaged sidecar

生产构建必须把本地 Agent 冻结为目标平台二进制，并让 Rust supervisor 报告：

```text
mode=packaged-sidecar
```

每个发布平台必须有非空、正确格式和正确权限的目标文件。详细合同见 [PACKAGING.md](PACKAGING.md)。普通用户机器不得依赖仓库、本机 Python、`.venv` 或开发路径。

先运行 `npm run release:packaged-sidecar-preflight` 阅读 P008 报告。它不读取 Provider Key、不联网或执行外来二进制；当前 macOS arm64 P002 已由 `desktop:packaged-sidecar-alpha-smoke` 与 `desktop:packaged-tauri-alpha-smoke` 验证真实启动、`GET /api/health`、首次初始化、GLB 导出和重启恢复。预检绿色或本机 Alpha E2E 都不替代新机器安装、签名、公证或外部发布。

## 5. 许可证与资产审阅

```bash
npm run release:license-sbom
npm run assets:formal-review-validate -- \
  --pack-root <pack-root> \
  --source-root <source-root> \
  --review <review.json>
```

本人原创声明不能替代独立 reviewer 批准。受限、质量失败或未完成审阅的资产不能作为正式默认替换候选。

## 6. 真实 Provider 评测

真实评测会产生费用，必须显式授权并记录：

- Provider、Base URL 和模型；
- truth set 版本；
- 每领域成功率；
- 结构化输出率；
- token 和费用；
- 失败类型和人工审阅结论。

密钥只存 Keychain 或权限为 0600 的 secret file，不进入报告。离线 deterministic smoke 不能标记为真实 AI 质量证据。

## 7. 签名与安装验证

本机内部测试可以使用未签名构建。外部发布前必须完成：

- macOS Developer ID 签名、公证、Gatekeeper 安装和首次启动；
- Windows 代码签名、SmartScreen/Defender 安装和首次启动；
- 全新用户账户首次初始化；
- 离线启动、Provider 配置、生成、编辑、导出和重启恢复；
- 升级安装和卸载；
- 日志、崩溃报告和备份不泄露密钥。

## 8. 发布后维护

每个版本保留：

- commit SHA、构建环境和依赖锁；
- SBOM、许可证报告和签名信息；
- sidecar hash；
- 数据库迁移范围；
- 已知兼容版本；
- 回滚步骤；
- 完整备份验证记录。

事故响应和恢复见 [DISASTER_RECOVERY.md](DISASTER_RECOVERY.md)。
