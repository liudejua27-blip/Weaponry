# M5 Rough 3D Preview Notes

This slice makes the first rough 3D provider and preview path real enough to validate in the desktop workbench.

The product boundary remains unchanged: the model is a fictional Unity game-art proxy asset. It is not a physical weapon blueprint, manufacturing dimension sheet, material formula, fabrication process, or assembly instruction.

## What Exists Now

`create_weapon` now writes minimal valid GLB assets instead of text placeholders with `.glb` extensions.

`POST /api/weapons/{weapon_id}/generate-3d` is implemented. It creates an append-only `rough_3d` child version from a selected `concept_image` or `concept_patch` and does not overwrite the source version.

The mock 3D provider returns:

- GLB 2.0 header
- JSON chunk
- non-empty BIN chunk
- triangle mesh primitive
- `POSITION` and `NORMAL` accessors
- simple material metadata
- raw, normalized, and optimized GLB variants
- Unity material metadata
- model quality metrics

Successful generate-3d jobs write:

- `ModelGenerationInput@1` as an `other` asset
- `rough_raw_glb`
- `rough_normalized_glb`
- `rough_optimized_glb`
- `unity_material_json`
- model `quality_report`
- `models_3d` row with source image, GLB variants, material metadata, orientation policy, and quality report references

The desktop `Preview3DPanel` now:

- reads weapons through `listWeapons()`
- resolves `GET /api/weapons/{weapon_id}` detail
- identifies the current source `concept_patch` or `concept_image`
- submits `POST /api/weapons/{weapon_id}/generate-3d` from the desktop workbench
- forwards the returned job id to the global timeline event stream
- finds the current or latest `rough_raw_glb`
- loads the controlled `/api/assets/{asset_id}/file` URL with Three.js `GLTFLoader`
- renders a WebGL exhibition scene with a pedestal, a simple original placeholder character, and the weapon held in the character hand socket
- auto-rotates the whole display rig and supports manual drag rotation for 360-degree inspection
- auto-centers the display rig by bounding box
- supports `toon`, `solid`, and `wireframe` preview modes
- supports reset view and screenshot export
- shows the linked `unity_material_json` asset
- submits `POST /api/weapons/{weapon_id}/export-unity` from the desktop workbench
- shows the latest `unity_export_package` ZIP asset with a controlled download link
- shows parsed model quality evidence from the current model report, including triangle, mesh, vertex, material, texture, longest-axis, center/extents, PBR, and bounds status
- shows the current Unity orientation and scale policy, including forward axis, long axis, pivot, fallback pivot, and game-relative scale policy
- refreshes when a new job is created so the latest generated weapon becomes the default preview target

`scripts/check_asset_library.py` now treats invalid GLB files and malformed generate-3d jobs as blockers.

Model quality reports now parse the optimized GLB JSON chunk and record triangle count, vertex count, mesh/primitive count, material/texture/image counts, PBR material presence, finite bounds, center, extents, and longest axis. Asset library validation treats missing mesh and bounds evidence as blockers and missing material slots as warnings.

## Gate

```text
npm run agent:m5-glb-smoke
npm run agent:m5-generate3d-http-smoke
npm run agent:m5-export-unity-http-smoke
npm run unity:preflight
npm run unity:import:gate
npm run m5:gate
```

The smoke verifies:

- `rough_raw_glb` is `model/gltf-binary`
- GLB header is valid
- JSON and BIN chunks exist
- mesh primitive uses triangles
- `POSITION` and `NORMAL` accessors exist
- `unity_material_json` is present
- explicit generate-3d creates a child `rough_3d` version
- generate-3d idempotency replay and conflict behavior
- raw, normalized, and optimized GLB variants are committed
- model quality report targets `model_3d`
- model quality report includes triangle count, mesh count, material count, and finite bounds evidence
- Unity export creates a `unity_export_package` ZIP snapshot with relative Unity paths
- export package manifest declares the fictional game-art / non-manufacturing boundary
- Unity import preflight validates ZIP paths, manifest hashes, optimized GLB, Unity material JSON, weapon spec, and model quality report
- `unity:preflight` records `UNITY_EXECUTABLE_NOT_CONFIGURED` when Unity is unavailable, while keeping ordinary mock development unblocked
- `unity:import:gate` requires Unity and exits non-zero when Unity is unavailable or import fails
- asset library validation has no blockers

Browser verification covered:

- create weapon from the desktop workbench
- submit generate-3d from the 3D preview panel using the current concept image
- submit export-unity from the 3D preview panel using the current rough model
- global job timeline receives `rough3d_plan`, `rough3d_submit`, `model_qc_optimize`, `asset_commit_model`, and `finalize_job`
- global job timeline receives `export_plan`, `export_manifest`, `export_package`, and `finalize_job`
- preview panel shows a controlled `unity_export_package` download link
- preview panel auto-refreshes to the newly generated weapon
- `rough_raw_glb` loads from the controlled asset file endpoint
- WebGL canvas renders nonblank pedestal, character, and held-weapon pixels
- manual drag rotation changes the rendered canvas checksum
- asset library lists version-by-version handoff files, including concept image, GLB variants, Unity material JSON, quality report, and Unity export ZIP, with controlled download URLs
- asset library handoff checklists show quality badges for current model reports, including QC status, blocker/warning counts, triangle/material counts, and bounds readiness
- asset library detail shows a clickable version DAG strip with root/parent version relationships across concept, patch, rough_3d, and export versions
- asset library version cards show provenance summaries with job id, root/parent source version, created time, and asset role list
- asset library version cards show concept/patch thumbnails or non-image summaries, total file count/size, controlled version-level batch downloads, and Unity ZIP direct links when present
- asset library asset rows can open JSON/GLB/ZIP preview drawers: JSON previews show schema/top-level keys/clipped body; GLB previews show header, chunks, mesh/material/texture/node counts, generator, and BIN summary; Unity ZIP previews parse central directory, manifest.json, package root, payload counts, relative path safety, and manifest coverage
- asset library export versions can request a local ZIP location reveal through `POST /api/assets/{asset_id}/reveal` without exposing absolute object-store paths to the desktop UI
- asset library version cards can restore the producing job into Task Center so users can inspect events, runtime, and action audit for that version
- desktop hash deep links restore state from cold load: `#/weapons/:weaponId/versions/:versionId` opens the Library on a specific version, and `#/jobs/:jobId` opens Task Center on a specific job trace
- Task Center preserves history filters after reload, can wake recently restored jobs from local storage, keeps local terminal-job notification records, can restore a job from a notification record, and action audit rows can locate/highlight the corresponding Agent Timeline event evidence

Latest generate-3d browser job:

```text
job_a135e3ca57ef
```

Latest export-unity browser job:

```text
job_b77d6d43d935
```

Latest export-unity package asset:

```text
file_19244adb5039
```

Latest screenshot:

```text
output/playwright/m5-final-preview3d-panel.png
output/playwright/m5-library-handoff.png
```

Latest canvas pixel check:

```json
{
  "ok": true,
  "width": 245,
  "height": 260,
  "uniqueSampledColors": 178,
  "nonBackgroundSamples": 504
}
```

Latest drag-rotation check:

```json
{
  "before_checksum": 2489209341,
  "after_checksum": 3563662150,
  "changed": true
}
```

Latest Unity import smoke result:

```json
{
  "package_preflight": {
    "ok": true,
    "zip_entries": 6
  },
  "unity_import_status": "blocked_unity_not_configured",
  "release_gate": "blocked",
  "blocking_failure": {
    "code": "UNITY_EXECUTABLE_NOT_CONFIGURED"
  }
}
```

## Remaining Work

- `local_http_3d` now provides the first real-runtime adapter boundary for Stable Fast 3D / Hunyuan3D / TripoSR / TRELLIS style services. `scripts/wushen_local_3d_runtime.py` now implements the Wushen local HTTP protocol as a separate subprocess with deterministic `mock` mode, an `sf3d-cli` backend path for a local Stable Fast 3D checkout, and a `triposr-cli` backend path for a local TripoSR checkout. `docs/LOCAL_3D_RUNTIME.md`, `npm run agent:p0-local-3d-runtime-sf3d-manual`, and `npm run agent:p0-local-3d-runtime-triposr-manual` define the manual install/verification paths. Remaining work is to run those paths against real model environments and record output/GPU/license evidence.
- Provider submit/poll/fetch/cancel is now implemented at the `ThreeDProvider` boundary for the async generate-3d worker. Remaining work is to persist provider-specific resume metadata across real runtime restarts and classify real provider timeout/quota/model errors.
- Deepen model quality reports after real provider runs: compare texture presence and material slots from SF3D/TripoSR outputs, add Unity import warning ingestion, and decide warning/blocker thresholds for production assets.
- Add production export package generation.
- Configure Unity in local CI and require `unity_import_status=imported` before claiming the Unity release gate is green.
