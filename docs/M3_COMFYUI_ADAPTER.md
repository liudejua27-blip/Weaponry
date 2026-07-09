# M3 ComfyUI Adapter Notes

This slice adds a provider boundary for concept image generation.

The default remains `mock_comfyui`, so local gates do not require a real ComfyUI install. Real ComfyUI is only used when explicitly configured.

## Provider Configuration

Default mock mode:

```text
WUSHEN_IMAGE_PROVIDER=mock
```

HTTP ComfyUI mode:

```text
WUSHEN_IMAGE_PROVIDER=comfyui
WUSHEN_COMFYUI_BASE_URL=http://127.0.0.1:8188
WUSHEN_COMFYUI_WORKFLOW_TEMPLATE=/path/to/api-workflow-template.json
WUSHEN_COMFYUI_CHECKPOINT=<checkpoint name>
WUSHEN_COMFYUI_WIDTH=1280
WUSHEN_COMFYUI_HEIGHT=720
WUSHEN_COMFYUI_TIMEOUT_SECONDS=30
WUSHEN_COMFYUI_POLL_INTERVAL_SECONDS=1
WUSHEN_COMFYUI_MAX_WAIT_SECONDS=180
WUSHEN_COMFYUI_RETRY_ATTEMPTS=3
WUSHEN_COMFYUI_RETRY_BACKOFF_SECONDS=0.5
WUSHEN_COMFYUI_CLIENT_ID=wushen-forge-agent
```

## Adapter Contract

`apps/agent/wushen_agent/providers/image.py` defines:

- `ImageProvider.generate_concept(...)`
- `MockComfyUIProvider`
- `ComfyUIHTTPProvider`

The HTTP provider uses the ComfyUI server API shape:

- `POST /prompt` to submit a workflow
- `GET /history/{prompt_id}` to retrieve outputs
- `GET /view` to download the generated image

The adapter does not write SQLite or object-store files directly. It returns image bytes plus provider metadata. `SQLiteAssetStore` commits all artifacts atomically.

HTTP retry policy:

- `POST /prompt`, `GET /history/{prompt_id}`, and `GET /view` retry transient failures.
- Retryable HTTP statuses: `408`, `409`, `425`, `429`, and `5xx`.
- Network errors, local timeouts, and connection resets are retryable.
- `400`-class workflow/config errors are not retried and return `PROVIDER_BAD_OUTPUT`.

## Workflow Templates

Default template:

```text
workflows/comfyui/concept_api_template.json
```

The template file stores:

- `template_id`
- `template_version`
- `prompt`: pure ComfyUI API prompt graph
- `bindings`: node/input locations for prompt, negative prompt, seed, image size, checkpoint, and filename prefix

Only `prompt` is submitted to ComfyUI. Template metadata is recorded in AssetStore metadata for traceability.

## AssetStore Outputs

For each create-weapon concept slice, AssetStore writes:

- `weapon_spec`
- `prompt`
- `negative_prompt`
- `comfyui_workflow`
- `concept_image`
- `quality_report`
- `rough_raw_glb`
- `unity_material_json`

The concept image metadata links back to:

- prompt asset
- negative prompt asset
- workflow asset
- provider task id

The workflow asset metadata includes:

- provider
- provider task id
- workflow sha256
- seed
- checkpoint name
- sampler name
- scheduler
- steps
- cfg
- denoise
- detected width and height for concept images

## Quality Gate

`concept_quality_report.json` is validated against `QualityReport@1`.

The rough 3D mock step only runs after the concept image has a passing or warning quality report. Event metadata records the quality report asset that gates `rough3d_submit`.

## Gate

```text
npm run agent:m3-comfyui-smoke
npm run agent:m3-comfyui-manual
npm run agent:m3-image-dimensions-smoke
npm run m3:gate
```

`agent:m3-comfyui-smoke` starts a fake local ComfyUI-compatible server and verifies the adapter path:

```text
POST /prompt -> GET /history/{prompt_id} -> GET /view
```

This proves the protocol boundary, retry/backoff behavior, non-retryable 400 handling, and AssetStore traceability without requiring a real model runtime.

`agent:m3-image-dimensions-smoke` validates PNG, JPEG, and WebP header parsing. Downloaded ComfyUI images must persist positive `width` and `height` in `asset_files`; unknown image payloads fail with `PROVIDER_BAD_OUTPUT`.

`agent:m3-comfyui-manual` requires a running local ComfyUI server and a workflow/checkpoint that exists on that server. It is intentionally not part of default `m3:gate`.

## Remaining Work

- Add production art workflow templates beyond the minimal SD basic template.
