# M4 Patch AssetStore Notes

This slice implements the first backend patch loop and the first ComfyUI HTTP inpaint adapter boundary.

It turns `POST /api/weapons/{weapon_id}/patch` from a placeholder into a provider-backed patch version commit with production-style validation.

The product boundary remains unchanged: patch output is a fictional Unity game-art weapon appearance asset. It may look visually realistic, but it must not become a real-world weapon blueprint, manufacturing dimension sheet, material formula, fabrication process, or assembly instruction.

## What Exists Now

`SQLiteAssetStore.patch_weapon(...)` validates:

- source weapon exists
- source version belongs to the weapon
- source image belongs to the source version
- mask asset exists and has role `patch_mask`
- mask dimensions equal source concept image dimensions
- mask PNG is not empty
- patch manifest exists, has role `patch_manifest`, and validates against `PatchManifest@1`
- manifest `weapon_id`, `source_asset_id`, and `mask_asset_id` match the request

`SQLiteAssetStore.upload_asset(...)` and `POST /api/weapons/{weapon_id}/versions/{version_id}/assets` now provide the pre-commit path for desktop canvas exports:

- `patch_mask`: PNG only, real pixel dimensions parsed on upload
- `patch_manifest`: JSON only, schema-valid `PatchManifest@1`
- idempotent replay through `idempotency_records`
- immutable object-store write through the same `_write_asset(...)` path as generated assets

`GET /api/weapons/{weapon_id}` now returns version and asset metadata so desktop Patch Mode can find a patchable source image automatically.

`GET /api/assets/{asset_id}` and `/file` return metadata and controlled object-store bytes by asset id. File reads verify object-store containment and sha256 before streaming.

`PatchModePanel` in the desktop app currently provides the first API-connected canvas slice. It loads the source concept image through the asset file API, draws brush or lasso masks at source pixel size, supports brush size, mask opacity, undo/redo, and submits that mask to the patch workflow. When a patch version is selected, it compares the parent source image and current patch result with a slider, can activate the patch version, can return to the parent version, and can switch the editor to the parent version for another patch attempt.

Successful patch jobs write:

- new `generation_jobs` row with `job_type=patch_image`
- new `weapon_versions` row with `version_type=patch`
- `parent_version_id` pointing to the source version
- `patch_prompt`
- `concept_patch`
- `comfyui_workflow`
- `quality_report`
- append-only `agent_events`

The source version is not overwritten.

## Provider Boundary

Default local development still uses `mock_comfyui`, which returns a deterministic SVG patch so the app can run without external services.

When `WUSHEN_IMAGE_PROVIDER=comfyui`, the patch path now:

- uploads the source image to ComfyUI `/upload/image`
- uploads the PNG mask to ComfyUI `/upload/image`
- binds both uploaded filenames into `workflows/comfyui/patch_inpaint_api_template.json`
- submits the inpaint workflow through `/prompt`
- polls `/history/{prompt_id}`
- downloads the output image through `/view`
- writes the downloaded output as `concept_patch`
- writes the submitted workflow as `comfyui_workflow`

The default inpaint workflow is intentionally minimal and replaceable. Production art direction should export a ComfyUI API workflow and set:

```text
WUSHEN_COMFYUI_PATCH_WORKFLOW_TEMPLATE=/absolute/path/to/exported_patch_api_workflow_template.json
```

The important M4 guarantee in this slice is:

```text
source concept + mask + PatchManifest -> new patch version
```

## Gate

```text
npm run agent:m4-patch-smoke
npm run agent:m4-patch-http-smoke
npm run agent:m4-comfyui-patch-smoke
npm run m4:gate
```

The smoke covers:

- successful patch version commit
- idempotency replay
- idempotency conflict
- older SQLite library migration through `0002`, `0003`, and `0004`
- empty mask rejection
- size-mismatched mask rejection
- source version remains committed and unmodified
- patch event order
- patch asset roles
- patch quality report schema
- public upload -> patch HTTP flow
- upload idempotency replay and conflict
- version activation, parent rollback, and missing-version errors
- fake-server ComfyUI patch adapter flow: source upload, mask upload, prompt binding, history polling, output download, and asset metadata

## Remaining Work

- Run the default patch workflow against a real local ComfyUI instance and replace it with a production art workflow.
- Add task-level retry policies for real provider failures.
