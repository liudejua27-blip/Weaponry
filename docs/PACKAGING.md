# Desktop Packaging

This document defines the production packaging contract for Forge Studio / ForgeCAD.

The target packaged product is a category-open desktop Agent for non-functional visual 3D assets. The current Alpha remains mechanically focused; the four mechanical domain packs are regression/knowledge packages, not a permanent input allowlist. Packaging must preserve the capability sandbox: no arbitrary generated code, shell, filesystem path, URL/network, secret access, unbounded resources, or unauthorized paid calls. It must not enable real-world weapon blueprints, manufacturing dimensions, material recipes, fabrication processes, structural certification, medical claims, or assembly instructions.

## Current Status

当前默认开发 supervisor 仍可启动 repository-local Python Agent，并报告：

```text
mode=local-dev-python
```

P002 本轮已重新构建 `aarch64-apple-darwin` frozen sidecar，并通过独立 sidecar alpha、P008 readiness、Rust ownership 静态 Gate、真实 LaunchServices `.app` runtime、K002 和 K003 原生双启动；真实 sidecar 的健康检查、临时 Library、受限几何归属、Rust-owned 产品状态、GLB/readback/render package、重启恢复和 `provider_calls=0` 已有本轮证据。K003 smoke 的健康校验已严格兼容 Rust supervisor 合法返回的动态 session/process-group 归属字段。当前仍不是可分发安装包：签名、公证、全新机安装/升级、跨平台 sidecar 和正式发布证据未完成。

生产构建必须启动 bundled Agent sidecar 并报告：

```text
mode=packaged-sidecar
```

## P008：本机 packaged Alpha 输入预检

`apps/desktop/src-tauri/binaries/sidecar-inputs.json` 是版本化的 `ForgeCADPackagedSidecarInput@1`。当前本机目标是 `aarch64-apple-darwin`；清单只保存目标三元组、相对二进制路径、启动参数、健康检查约定、首次初始化/恢复检查项和“运行时 Keychain 或权限受限 secret file”的边界，绝不保存 Provider Key、Base URL、模型名或用户项目数据。

```bash
npm run release:packaged-sidecar-preflight
npm run release:packaged-sidecar-preflight-smoke
```

第一个命令不联网、不读取 Keychain/secret file、不执行 sidecar，也不写项目、资产或 Snapshot。它会检查路径、容器格式、CPU 架构、可执行权限、启动参数和健康检查命令；当前 macOS arm64 frozen input 报告 `ready_for_local_alpha`。这只表示结构输入可进入 P002，不表示安装包可发布、已经签名或已经公证。

P002 本机命令：

```bash
npm run desktop:packaged-sidecar-build
npm run desktop:packaged-sidecar-alpha-smoke
npm run desktop:packaged-tauri-alpha-smoke
npm run desktop:k002-packaged-native-smoke
npm run desktop:k003-packaged-native-smoke
npm run k003:layered-gate
```

第一条只在构建机冻结当前 macOS arm64 runtime；后续命令分别执行真实 sidecar、通过 LaunchServices 启动的实际 Tauri bundle、K002/K003 原生双启动和五层最终聚合。它们均使用临时 Rust-owned Library、清空/最小化 Python sidecar 环境、不读取 Keychain、不会自动调用模型 Provider。原生命令当前只适用于 macOS；其程序化证明不等于用户点击下载、安装器、签名、公证或外部发布。若已有同路径桌面进程占用 8000，先由操作者正常关闭该应用，再重跑 `desktop:packaged-tauri-alpha-smoke`；不得通过杀死未知进程绕过前置条件。

## Required Production Shape

ForgeCAD uses Tauri 2. Tauri's official sidecar model bundles target-suffixed external binaries through `bundle.externalBin` in `tauri.conf.json`.

Required Tauri config shape:

```json
{
  "bundle": {
    "active": true,
    "targets": "all",
    "externalBin": ["binaries/wushen-agent"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

Required sidecar files:

```text
apps/desktop/src-tauri/binaries/wushen-agent-aarch64-apple-darwin
apps/desktop/src-tauri/binaries/wushen-agent-x86_64-apple-darwin
apps/desktop/src-tauri/binaries/wushen-agent-x86_64-pc-windows-msvc.exe
apps/desktop/src-tauri/binaries/wushen-agent-x86_64-unknown-linux-gnu
```

The exact target list can be narrowed per release platform, but each published target must have a matching sidecar binary.

## Agent Sidecar Build Contract

The sidecar must be a frozen Agent runtime, not a dependency on the user's local Python environment.

Minimum contract:

- Contains only the FastAPI restricted-geometry entrypoint and audited geometry/PBR dependencies.
- Contains the read-only geometry schema/material resources needed for compilation, but no selectable product-state migration or database handler.
- Receives no Provider credential, user session, Snapshot write token, database path or object-store path.
- Resolves only its code-derived read-only resource root; the user data Library is owned by Rust core and is not mounted into Python.
- Exposes `GET /api/health` with `python_role=restricted_geometry_executor` and a capability-gated compile surface.
- Keeps API keys out of logs, SQLite rows, job events, asset files, Unity ZIP packages, and crash reports.
- Preserves the fictional mechanical concept / non-manufacturing safety boundary.

The development fallback may remain available behind explicit dev overrides such as `WUSHEN_REPO_ROOT` and `WUSHEN_AGENT_PYTHON`, but release builds must prefer the packaged sidecar.

## Rust Supervisor Contract

Production supervisor behavior:

1. Open the Rust-owned app-data Library, run Rust migrations, acquire the single-writer epoch and initialize CAS before accepting product requests.
2. Resolve and spawn the bundled restricted-geometry sidecar without Library, migration, object-store or Provider paths.
3. Keep Project/Version/Snapshot/ChangeSet/Quality/Export transactions in Rust; pass Python only validated geometry IR through the capability-gated port.
4. Probe `GET /api/health` and verify the restricted geometry identity before marking the geometry capability online.
5. Reject wrong services on the fixed local port.
6. Stop the managed child on window close and process shutdown.
7. Persist sidecar lifecycle logs without leaking secrets.
8. Report `mode=packaged-sidecar` to the frontend.

Development supervisor behavior may keep `mode=local-dev-python` and repo-local paths.

开发 supervisor 的本机启动、CSP、capability 与日志边界见 [DEVELOPMENT.md](DEVELOPMENT.md)。这些开发覆盖不能进入发布构建，也不能成为绕过 sidecar gate 的替代路径。

## Release Gate

Run:

```bash
npm run release:packaging-readiness
```

The gate blocks production release until:

- `Cargo.lock` is committed.
- Tauri bundle settings are present.
- Production icons are configured and the icon files exist.
- `bundle.externalBin` includes `binaries/wushen-agent`.
- At least one target-suffixed `wushen-agent-*` sidecar binary exists.
- Each published sidecar is non-empty, executable where the target requires it, and has the expected Mach-O/ELF/PE header for its target.
- Rust supervisor code implements a `packaged-sidecar` mode.
- `ForgeCADPackagedSidecarInput@1` 通过无密钥结构预检；`ready_for_local_alpha` 仍必须由 P002 的真实启动、健康检查和恢复验证继续确认。
- Packaging docs and release command references are current.

`npm run release:gate` includes this packaging gate. Until it passes, the app is a development desktop Agent, not a distributable production desktop package.

Reference:

- Tauri 2 sidecar documentation: https://v2.tauri.app/develop/sidecar/
