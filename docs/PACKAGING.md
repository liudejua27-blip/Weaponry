# ForgeCAD Runtime 打包合同

版本：2026-08-09
状态：MCP013 正式发布合同；不阻塞 MCP005–009 开发 MVP，当前不可外部分发

## 1. MVP 与发布分界

MCP005–009 使用本地开发构建验证真实 3D，不要求 Developer ID/notarization。任何对外安装、自动配置 Codex、普通用户可用或正式版本声明仍必须满足本文全部要求。

## 2. 发布组件

同一 release manifest 包含：

- ForgeCAD Runtime Viewer；
- `forgecad-runtime`；
- `forgecad-mcp`（拥有 MCP stdio，并按需启动同包 `forgecad-runtime`）；
- geometry/render workers；
- 可选 Blender worker；
- first-party Skill/asset packs；
- Runtime V1 migration；
- contracts/tool manifest/license/NOTICE/SBOM/provenance/signatures；
- Codex Desktop/CLI P0 配置助手；IDE/VS Code/Cursor/Windsurf 配置基线只作为未来兼容资产保留。

组件合同版本和签名必须一致。旧 sidecar、App Server/Protocol、Python FastAPI、端口 8000、模型 Key 配置和 legacy packs 不进入安装包。

## 3. macOS 边界

安装包完成 code signing、hardened runtime、entitlements 最小化、notarization/stapling。Workers/MCP 也是签名可执行文件；Runtime 只开放 authenticated local IPC。Viewer CSP/Tauri capabilities 不允许 broad filesystem/network。

Codex MCP 配置只写本机签名二进制路径、timeout 和 write approval policy，不包含 secret 或项目绝对路径。卸载默认保留用户 Library，数据删除需独立选择。

当前 `forgecad-runtime serve` 用于独立诊断，正常入口是 `forgecad-mcp`：MCP 先完成 stdio initialize，再异步启动同包 Runtime，并通过受保护的 `ready.json`/status handoff 连接 authenticated local IPC。生命周期回归已通过；2026-08-15 同 cohort Dev.app 的四资源 Resource allowlist、ad-hoc deep-strict package verify 和 packaged Runtime → sibling Render Worker 九 AOV raw transport 已通过，证据见 `docs/evidence/mcp010f/dev-app-install-render-worker-20260815.json`、`dev-app-package-verify-render-worker-20260815.json`、`packaged-render-worker-raw-20260815.json`。该 raw probe 使用 synthetic reference，只证明 packaged resource/process/protocol；distribution signing Gate 仍保持 BLOCKED，正式 notarization、packaged UI E2E、真实 likeness/PBR/人评和 360 不由此升级。本机可见 1 个有效 codesigning certificate，但以名称和 SHA-1 选择身份的只读签名探针均返回 `errSecInternalComponent`，keychain settings 读取还返回 passphrase error，且没有修改 keychain；详见 `docs/evidence/mcp004/macos-signing-diagnostic.json`。`docs/evidence/mcp004/codex-cli-write-e2e.json` 只证明真实 Codex CLI 对开发诊断入口的事务交接。

本地打包命令要求调用方显式提供签名身份；没有身份时命令直接失败，不自动退回 unsigned：

```bash
APPLE_SIGNING_IDENTITY="<approved signing identity>" \
  npm run desktop:tauri-package:macos
```

该命令会先构建 release `forgecad-runtime` 与 `forgecad-mcp`，再运行 Tauri app bundle。仓库内 `npm run desktop:tauri-build` 通过 `script/with_rust_toolchain.sh` 固定 Cargo 查找；签名失败、notarization 未运行或 packaged Desktop 3D E2E 未验收时，状态必须分别记录为 BLOCKED/NOT_RUN。MCP010A 的开发 App 激活证据不替代 MCP013 正式发布门。

## 4. 可选 Blender worker

若打包，必须固定版本、隔离进程、无网络、固定 Recipe、完成 GPL/源码提供/NOTICE 法律审查；不得运行 Codex/Skill 提供的 Python/addon。若不打包，capabilities 明确 unavailable，核心版本/几何真值不受影响。

## 5. 安装/升级

安装前验证磁盘和兼容性；升级前备份 Runtime V1 DB/CAS manifest；在副本跑 migration；原子替换整套组件；失败回滚二进制和数据库。禁止不同版本 MCP/Runtime/Viewer/Worker 混跑写路径。

## 6. 发布 Gate

clean-room 构建可复现、签名/notarization、SBOM/license、安全扫描、无绝对路径/secret、无 legacy/model/8000、Codex Desktop/CLI P0 packaged E2E、Viewer 关闭运行、升级/回滚、离线启动、灾难恢复、跨类别质量和真人门。IDE 兼容只有在未来升级支持范围时才加入发布 Gate。
