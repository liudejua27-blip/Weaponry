# M2 SQLite AssetStore Notes

M2 replaces the M1 in-memory mock with a SQLite-backed AssetStore and immutable local object files.

The generation providers are still mock providers. The important M2 guarantee is persistence and traceability:

- `POST /api/weapons` writes `weapons`, `generation_jobs`, `job_steps`, `weapon_versions`, `weapon_specs`, `asset_files`, `agent_events`, and `models_3d`.
- Generated mock files are written under `WushenForgeLibrary/objects/sha256/<aa>/<bb>/`.
- Every asset row stores `sha256`, `byte_size`, MIME type, role, logical path, and object path.
- Reusing the same `Idempotency-Key` with the same canonical request returns the same `job_id`.
- Reusing the same `Idempotency-Key` with a different request returns `409 IDEMPOTENCY_CONFLICT`.
- SSE events replay from SQLite and support `?after=evt_id`, legacy `?last_event_id=evt_id`, and `Last-Event-ID`.

## Run

Install the backend in the local virtualenv:

```text
python3 -m venv .venv
.venv/bin/python -m pip install -e apps/agent
```

Start the Agent:

```text
WUSHEN_LIBRARY_ROOT="$PWD/WushenForgeLibrary" \
.venv/bin/uvicorn wushen_agent.main:create_app --factory --host 127.0.0.1 --port 8000 --reload
```

Start the desktop shell:

```text
npm --workspace apps/desktop run dev -- --host 127.0.0.1 --port 5173
```

## Gate

```text
npm run contracts:check
npm run agent:check
npm run agent:m2-smoke
npm run desktop:typecheck
npm run desktop:build
npm run m2:gate
```

`agent:m2-smoke` starts a temporary FastAPI process and temporary library, then verifies:

- health endpoint
- create weapon request
- idempotency replay
- idempotency conflict
- job detail
- SSE event frames
- SQLite row counts
- foreign keys
- object file existence
- sha256 and byte size
- event artifact references

## Current Limitations

- LLM, ComfyUI, and 3D provider calls are still mock providers.
- Patch workflow returns `501` in M2.
- Generate-3D workflow returned `501` in M2 because the create flow already recorded a mock rough GLB contract slice. This has been superseded by the M5 provider-backed `POST /api/weapons/{weapon_id}/generate-3d` implementation.
- Tauri sidecar startup is still not wired.
- GLB payload is a traceable placeholder, not a valid production model.
