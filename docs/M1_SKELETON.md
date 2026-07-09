# M1 Skeleton Notes

M1 establishes the first runnable shape of Wushen Forge:

- `apps/agent`: local FastAPI Agent service with mock endpoints and SSE job events.
- `apps/desktop`: Tauri + React + Vite desktop workbench shell.
- `packages/weapon-spec/schemas`: JSON schema contracts.
- `migrations/0001_init.sql`: initial SQLite schema.
- `scripts/check_contracts.py`: contract smoke check.
- `scripts/check_asset_library.py`: asset library consistency check.

## Agent Service

Install dependencies in a Python environment:

```text
cd apps/agent
python3 -m pip install -e .
uvicorn wushen_agent.main:create_app --factory --host 127.0.0.1 --port 8000 --reload
```

Health check:

```text
curl http://127.0.0.1:8000/api/health
```

Create a mock job:

```text
curl -X POST http://127.0.0.1:8000/api/weapons \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: local-test-001' \
  -d '{"client_request_id":"local-test-001","text":"防弹裤神炮，穿上后可将腿部动作转化为雷焰连射","reference_asset_ids":[],"auto_run":true}'
```

## Desktop

Install dependencies from the repository root:

```text
npm install
npm --workspace apps/desktop run dev -- --host 127.0.0.1 --port 5173
```

The desktop shell expects the Agent service at `http://127.0.0.1:8000` for M1.

## M1 Limitations

- M1 originally used an in-memory mock store; M2 replaced create-weapon persistence with SQLite AssetStore.
- No real LLM, ComfyUI, or 3D provider is called.
- The 3D preview panel is a contract placeholder.
- Tauri sidecar packaging is declared as an architecture target, not wired yet.
