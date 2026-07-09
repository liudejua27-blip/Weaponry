# Unity Import Smoke

This smoke validates that a generated `unity_export_package` can become a Unity project asset handoff, not only a ZIP file.

The product boundary stays unchanged: the package contains fictional game-art assets for Unity workflows. It must not contain real-world weapon blueprints, manufacturing dimensions, material recipes, fabrication processes, or assembly instructions.

## Command

```text
npm run unity:preflight
```

This command always runs a local export-package preflight:

- creates a temporary mock weapon
- exports a `unity_export_package`
- validates safe ZIP paths under `Assets/WushenForge/Weapons/{weapon_id}/`
- validates `UnityExportManifest@1`
- validates the fictional game-art / non-manufacturing boundary
- validates `rough_optimized.glb` as GLB 2.0
- validates `unity_material.json`, `weapon_spec.json`, and `model_quality_report.json`
- checks manifest file `sha256` and `byte_size` entries against the ZIP payload
- runs asset library validation

If Unity is available, the same command then creates a temporary Unity project, installs Unity glTFast, extracts the package into `Assets/`, and launches Unity in batch mode to verify that:

- `rough_optimized.glb` has a registered Unity importer
- Unity exposes imported assets for the GLB
- `manifest.json`, `unity_material.json`, `weapon_spec.json`, and `model_quality_report.json` import as text assets

For release gating, use:

```text
npm run unity:import:gate
```

That command passes `--require-unity`; if Unity is missing, it exits non-zero instead of only recording a blocker.

The broader release entry point is:

```text
npm run release:gate
```

That command first runs `npm run release:safety-scope` to verify the fictional game-art / non-manufacturing boundary across schema, prompts, generated export package manifest, export README, model quality report, and docs; then it runs `unity:import:gate`.

## Unity Configuration

Set one of these environment variables when Unity is not installed at a default path:

```text
WUSHEN_UNITY_EXECUTABLE=/Applications/Unity/Hub/Editor/<version>/Unity.app/Contents/MacOS/Unity
UNITY_EXECUTABLE=/Applications/Unity/Hub/Editor/<version>/Unity.app/Contents/MacOS/Unity
```

Optional settings:

```text
WUSHEN_UNITY_GLTF_PACKAGE_VERSION=6.1.0
WUSHEN_UNITY_TIMEOUT_SECONDS=240
```

## Status Semantics

`unity_import_status=imported` means Unity batchmode opened the temporary project and imported the package successfully.

`unity_import_status=blocked_unity_not_configured` means the export package passed preflight, but Unity was not available on this machine. `unity:preflight` records this blocker and exits zero so ordinary mock development can continue. `unity:import:gate` treats the same condition as a non-zero release gate failure.

`unity_import_status=failed` means Unity was available but could not import the generated package. This is a failing smoke.

Current local result:

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

## References

- Unity command-line arguments: https://docs.unity3d.com/6000.1/Documentation/Manual/EditorCommandLineArguments.html
- Unity glTFast installation: https://docs.unity3d.com/Packages/com.unity.cloud.gltfast@6.1/manual/installation.html
- Unity glTFast editor import: https://docs.unity3d.com/Packages/com.unity.cloud.gltfast@5.2/manual/ImportEditor.html
