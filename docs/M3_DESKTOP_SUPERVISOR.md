# M3 Desktop Agent Supervisor Notes

This slice adds a local-development Agent supervisor to the Tauri desktop shell.

It is not the final bundled sidecar. The final sidecar still needs a frozen platform binary and Tauri bundle permission configuration.

Release packaging is tracked in [Desktop Packaging](PACKAGING.md). Production builds must move from `mode=local-dev-python` to `mode=packaged-sidecar`, and Tauri must bundle the frozen Agent with `bundle.externalBin`.

## What Exists Now

Rust commands in `apps/desktop/src-tauri/src/main.rs`:

- `agent_health_endpoint`
- `agent_service_status`
- `start_agent_service`
- `stop_agent_service`

Frontend wrapper:

```text
apps/desktop/src/shared/tauri/agentSupervisor.ts
```

The wrapper uses dynamic `import('@tauri-apps/api/core')` and returns an unsupported state in browser/Vite preview. This keeps normal web development working without a Tauri runtime.

Settings UI now exposes:

- Agent API health check
- Desktop supervisor status
- start local Agent
- stop managed Agent
- restart managed Agent

The status payload reports:

- `base_url`
- `health_url`
- `running`
- `managed_by_desktop`
- `pid`
- `state`
- `last_error`

The frontend treats this payload as the runtime source of truth in Tauri mode. Browser/Vite mode still uses `VITE_FORGE_API_BASE_URL` or `http://127.0.0.1:8000`.

## Runtime Contract

The local supervisor starts:

```text
.venv/bin/python -m uvicorn wushen_agent.main:create_app --factory --host 127.0.0.1 --port 8000
```

It sets:

```text
PYTHONPATH=<repo>/apps/agent
WUSHEN_LIBRARY_ROOT=<repo>/WushenForgeLibrary
WUSHEN_MIGRATIONS_DIR=<repo>/migrations
```

Override hooks:

```text
WUSHEN_REPO_ROOT=/path/to/wushen-forge
WUSHEN_AGENT_PYTHON=/path/to/python
```

Runtime safety rules:

- The supervisor validates `GET /api/health` and only accepts `service=wushen-agent` with `status=ok`.
- A different service on port 8000 is reported as `wrong_service`; the desktop will not treat it as Wushen Agent.
- If startup times out or returns a wrong service, the supervisor kills and clears only the child process it started.
- Window close and state drop both attempt to stop the managed child process.
- Agent stdout/stderr are appended to `.wushen-agent.log` in the repository root for local diagnostics.
- Tauri now defines a first-pass production CSP in `tauri.conf.json`: scripts are self-only, object/frame embedding is disabled, `connect-src` is limited to self and local Agent HTTP, and image sources are limited to self, Tauri asset protocols, local Agent asset URLs, data, and blob.
- `src-tauri/capabilities/default.json` grants only `core:default` permissions to the main window. File access and opening asset locations remain behind the Agent API, where asset id, object-store containment, and sha256 are checked before any reveal action.

## Gate

Frontend and backend gate:

```text
npm run m3:gate
```

Rust/Tauri compile gate when Rust is installed:

```text
npm run desktop:tauri-check
```

This environment currently has no `cargo`/`rustc`, so Rust compilation cannot be verified here.

## Remaining Sidecar Work

- Package the Python Agent as a platform sidecar binary.
- Configure Tauri `bundle.externalBin` with `binaries/wushen-agent`.
- Use a packaged sidecar startup path for production bundles and report `mode=packaged-sidecar`.
- Add packaged-sidecar lifecycle logging and crash restart policy.
- Add sidecar-specific capabilities once the bundled sidecar replaces local-dev Python startup.
- Add runtime smoke that launches Tauri and verifies invoke calls.
