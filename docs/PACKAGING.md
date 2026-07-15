# Desktop Packaging

This document defines the production packaging contract for ForgeCAD.

The packaged product is a desktop Agent for fictional mechanical concept assets and non-manufacturing descriptions. The first domain packs cover future weapon props, vehicles, aircraft and robotic arms. A packaged build must not enable real-world weapon blueprints, manufacturing dimensions, material recipes, fabrication processes, structural certification or assembly instructions.

## Current Status

当前默认开发 supervisor 仍可启动 repository-local Python Agent，并报告：

```text
mode=local-dev-python
```

P002 已在本机构建 `aarch64-apple-darwin` frozen sidecar。独立与原生 bundled smoke 均验证：无 Provider 环境的 health、空 SQLite Library 初始化、确定性可编辑资产的 GLB 导出，以及重启后资产读取/导出。原生 smoke 还验证了实际 `.app` 的 supervisor 报告下列 mode、sidecar 为其受管后代，以及正常窗口关闭回收 listener；当前仍不是可分发安装包。

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
```

前者只在构建机冻结当前 macOS arm64 runtime；后两者分别执行真实 sidecar 与通过 LaunchServices 启动的实际 Tauri bundle。它们均使用临时 Library、移除 Provider 环境变量、不读取 Keychain、不会自动调用模型 Provider。第三条命令当前只适用于 macOS，本机 Alpha 通过不等于安装器、签名、公证或外部发布。

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

- Contains the FastAPI Agent entrypoint.
- Contains migrations and schema registry needed at runtime.
- Reads provider configuration from environment variables and local config, not from committed secrets.
- Uses a user data directory for the library by default, not the source repository.
- Exposes `GET /api/health` with `service=wushen-agent` and `status=ok`.
- Keeps API keys out of logs, SQLite rows, job events, asset files, Unity ZIP packages, and crash reports.
- Preserves the fictional mechanical concept / non-manufacturing safety boundary.

The development fallback may remain available behind explicit dev overrides such as `WUSHEN_REPO_ROOT` and `WUSHEN_AGENT_PYTHON`, but release builds must prefer the packaged sidecar.

## Rust Supervisor Contract

Production supervisor behavior:

1. Resolve and spawn the bundled sidecar.
2. Set `WUSHEN_LIBRARY_ROOT` to an app data directory.
3. Set `WUSHEN_MIGRATIONS_DIR` and schema paths to bundled resources or sidecar-internal paths.
4. Probe `GET /api/health` before marking the Agent online.
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
