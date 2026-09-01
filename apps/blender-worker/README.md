# Weaponry fixed Blender knife worker

This directory contains the first isolated Blender prototype for the knife
vertical slice. It is a one-shot, first-party entrypoint with the closed Rust
wire `WeaponryBlenderKnifeWorkerRequest@1` and
`WeaponryBlenderKnifeWorkerResponse@1`. The Runtime launcher creates an offline
scratch directory, stages the authorized source bytes at `input/source.glb`,
and invokes the fixed entrypoint with `--scratch-root <that-directory>`.

The worker has one allowlisted recipe:

`knife_high_low_uv_bake@1`

It imports the staged GLB with Blender's built-in glTF importer, creates a
non-destructive source/high/low separation, applies High Bevel plus Weighted
Normal, applies Low Decimate plus Weighted Normal, creates Low UVs with Smart
Project, and performs bounded Cycles CPU tangent-normal and AO bakes. It
exports temporary High and Low GLBs and per-part PNG maps. Output bytes remain
temporary observations until Rust independently validates and adopts them.

The request cannot select a Python file, add-on, URL, executable, input path,
output path, operation list, or network policy. No database, CAS, Runtime
state, stage, approval, candidate, version, or export is written here. The
`.blend` session is never saved and is not product truth.

The checked-in `manifest.json` records the fixed worker script, Rust wire,
recipe identity, and the measured Blender 5.2.1 LTS arm64 macOS sidecar:
version/build fields, Developer ID identity, executable SHA-256, complete
bundle inventory, bundled-Python tree hash, and the four in-bundle license
resources. The checked-in entrypoint hash is intentionally `null`: it is
derived from the current source bytes for every staging run, and the staged
`source-manifest.json` records that derived value. Re-run:

```bash
python3 scripts/stage_weaponry_blender_worker.py
python3 scripts/stage_weaponry_blender_worker.py --verify
```

after changing `weaponry_knife_worker.py`; `--verify` rejects stale staged
bytes. Staging is development-only and writes only below the ignored Tauri
`target` directory. The launcher must clear the environment and use
`--background --factory-startup --disable-autoexec --threads 1
--debug-depsgraph-no-threads --python-exit-code 1` with the sealed entrypoint
and runtime scratch directory. The single-thread/depsgraph flags, together
with fixed 1e-6 position, split-normal and surface-signal quantization plus a
fixed 1/65536 UV grid in
the entrypoint, are part of deterministic artifact replay. Callers cannot provide
Python, add-ons, paths, URLs, executables, environment values, or network
access.

Every staged Tauri resource now also carries these offline compliance files
under `compliance/`:

* `NOTICE` — Blender and worker attribution plus the exact in-bundle license
  resource locations and measured sidecar identity.
* `sbom.spdx.json` — SPDX-2.3 package inventory binding the Blender executable
  and fixed worker entrypoint hashes. Blender's own third-party license index
  remains authoritative for embedded libraries.
* `GPL-SOURCE-OFFER.md` — official source locations and reproducible identity
  checks for the covered Blender build. It is deliberately marked as an
  unreviewed acquisition description, not a legal opinion or completed written
  offer.
* `release-eligibility.json` — a canonical, supplemental gate record generated
  during staging. It is not Runtime truth and cannot advance a candidate or
  stage.

Both `apps/desktop/src-tauri/tauri.conf.json` and
`apps/desktop/src-tauri/tauri.dev.conf.json` map
`Resources/weaponry-blender-worker` to the generated
`target/weaponry-blender-worker` tree. The staging verifier checks this mapping
before accepting a package. The top-level Tauri build/package scripts run the
Blender stage and verification step before invoking Tauri; staging never
downloads Blender or silently falls back to an unverified host installation.

The source `manifest.json` intentionally retains its closed
`NOT_INCLUDED_DEVELOPMENT_STAGING` distribution fields. They are part of the
existing Rust Worker compatibility contract; changing them would make the
Runtime reject the source/package identity. The generated supplemental record
is the packaging-layer compliance projection and does not widen that contract.

Including `runtime/Blender.app` means a matching packaged macOS arm64 user does
not need to install Blender separately. The package status remains
`DEVELOPMENT_STAGED_NOT_RELEASE_ELIGIBLE`: no corresponding Blender source
archive or approved source-offer wording is present, the first-party worker
license is `NOASSERTION`, product/legal review is `NOT_RUN`, and a Weaponry
distribution signature or DMG is not present. The upstream Blender Developer ID
signature is verified only for the copied sidecar; it is not a product release
signature. The sample job is bound to the current Dragonfang r8 GLB and is an
example transport envelope, not a release receipt. Blender output remains
temporary observation until Rust independently validates and adopts it.
