# Desktop Packaging

This document defines the production packaging contract for Wushen Forge.

The product boundary remains unchanged in packaged builds: Wushen Forge ships a desktop Agent for fictional Unity game-art assets and non-manufacturing descriptions. A packaged build must not enable real-world weapon blueprints, manufacturing dimensions, material recipes, fabrication processes, or assembly instructions.

## Current Status

The current desktop supervisor is a development runtime. It starts a repository-local Python Agent and reports:

```text
mode=local-dev-python
```

That is acceptable for development and smoke tests, but it is not acceptable for production distribution. A production build must start a bundled Agent sidecar and report:

```text
mode=packaged-sidecar
```

## Required Production Shape

Wushen Forge uses Tauri 2. Tauri's official sidecar model bundles target-suffixed external binaries through `bundle.externalBin` in `tauri.conf.json`.

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
- Preserves the fictional Unity game-art / non-manufacturing safety boundary.

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
- Packaging docs and release command references are current.

`npm run release:gate` includes this packaging gate. Until it passes, the app is a development desktop Agent, not a distributable production desktop package.

Reference:

- Tauri 2 sidecar documentation: https://v2.tauri.app/develop/sidecar/
