# M3 LLM Adapter and Contract Generation Notes

M3 foundation adds two production-facing pieces without requiring a real paid provider in the default path:

- OpenAI-compatible LLM adapter boundary for `WeaponDesignSpec` planning.
- Generated contract artifacts for JSON schemas and FastAPI OpenAPI.

The default runtime remains `WUSHEN_LLM_PROVIDER=mock`. A real provider is only used when explicitly enabled.

## LLM Provider Configuration

```text
WUSHEN_LLM_PROVIDER=mock
```

OpenAI-compatible mode:

```text
WUSHEN_LLM_PROVIDER=openai_compatible
WUSHEN_LLM_BASE_URL=https://api.openai.com/v1
WUSHEN_LLM_MODEL=<model name>
WUSHEN_LLM_API_KEY=<secret>
```

Alternative secret file:

```text
WUSHEN_LLM_API_KEY_FILE=/path/to/key.txt
```

Provider settings only expose safe fields:

- provider id
- kind
- type
- display name
- enabled
- status
- base URL
- whether a secret exists
- updated time

They never expose API keys, raw headers, raw provider response bodies, or environment dumps.

## Failure Semantics

Real provider failures are surfaced through the API error envelope:

- `PROVIDER_UNCONFIGURED`
- `PROVIDER_AUTH_FAILED`
- `PROVIDER_TIMEOUT`
- `RATE_LIMITED`
- `INVALID_LLM_JSON`
- `PROVIDER_BAD_OUTPUT`

There is no silent fallback from a real provider to mock. Mock is the default development provider, not an error recovery path.

## Generated Artifacts

JSON Schema contract generation:

```text
packages/weapon-spec/generated/types.ts
apps/agent/wushen_agent/generated/schema_registry.py
```

GPT Pro 目标态约束：`interpretation`、`creative_weapon_graph`、`skill_graph` 应采用结构化输出约束（Structured Outputs）或等价 schema 验证策略，避免输出字段遗漏和无法映射的 `combat_affordances`；
未通过 schema 的内容必须走 `INVALID_LLM_JSON`，不进入资产库。

FastAPI OpenAPI generation:

```text
packages/weapon-spec/generated/openapi.json
apps/desktop/src/shared/generated/api-types.ts
```

The desktop `shared/types.ts` is now a thin alias layer over generated OpenAPI component types.

## Gate

```text
npm run contracts:types:generate
npm run contracts:types:check
npm run agent:m3-llm-smoke
npm run agent:m3-comfyui-smoke
npm run m3:gate
```

`m3:gate` runs:

```text
contracts:types:check
agent:m3-llm-smoke
agent:m3-comfyui-smoke
m2:gate
```

The M3 LLM smoke forces a no-real-key environment, verifies the mock adapter, verifies OpenAI-compatible missing-key failure, checks provider settings for secret leakage, runs a SQLite-backed create flow through the mock LLM provider, and confirms that an invalid `WeaponDesignSpec` is rejected before AssetStore commit.

The M3 ComfyUI smoke starts a fake ComfyUI-compatible server and verifies `POST /prompt -> GET /history/{prompt_id} -> GET /view`, then checks that AssetStore wrote prompt, negative prompt, workflow, concept image, and quality report provenance.

## WeaponDesignSpec Commit Gate

`apps/agent/wushen_agent/spec_validation.py` loads `packages/weapon-spec/schemas/weapon-design-spec.schema.json` and sibling schema refs, then validates provider output with Draft 2020-12 before immutable asset files or SQLite rows are written.

If validation fails, `SQLiteAssetStore.create_weapon` raises `PROVIDER_BAD_OUTPUT`. This keeps bad real-provider output out of the asset library instead of silently normalizing it into a committed weapon version.

## Current Limitations

- Real OpenAI-compatible calls are implemented but not part of automated gate because they require user-provided credentials.
- ComfyUI HTTP adapter is implemented with a fake-server automated smoke; real local ComfyUI smoke still requires a user-installed ComfyUI runtime and workflow template.
- Tauri now has a local-development Agent supervisor; bundled production sidecar remains unwired.
