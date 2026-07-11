# Third-Party License and SBOM Gate

This document is the release-facing license ledger for Wushen Forge. It does not replace full legal review, but it gives the release gate a concrete inventory to check.

## Release Commands

```text
npm run release:license-sbom
npm run release:gate
```

`release:license-sbom` checks machine-readable dependency inventories and this ledger. A production release remains blocked while any release dependency is unlocked, missing a license, uses a disallowed license, or has pending external model/runtime review.

## Automated Coverage

| Surface | Source | Gate status |
| --- | --- | --- |
| Desktop npm dependencies | `package-lock.json` | Automated license expression check. |
| Desktop Rust/Tauri dependencies | `apps/desktop/src-tauri/Cargo.lock` | Blocked until `Cargo.lock` is committed and audited. |
| Agent Python dependencies | `apps/agent/requirements-release.lock` | Automated pin and license expression check. |
| Unity import package | Generated export ZIP manifest | Covered by `release:safety-scope` and `unity:import:gate`; Unity package license review remains separate. |

Allowed license atoms for current integrated code dependencies:

- `MIT`
- `Apache-2.0`
- `Apache-2.0 OR MIT`
- `BSD-2-Clause`
- `BSD-3-Clause`
- `ISC`
- `MPL-2.0`
- `CC-BY-4.0` for metadata/data packages only

Blocked license families unless explicitly approved for this product:

- `AGPL`
- `GPL`
- `LGPL`
- `SSPL`
- `BUSL`
- `Commons-Clause`
- custom commercial or unknown licenses

## External Runtime and Model Ledger

These projects are referenced by the product design or supported as external adapters. They are not automatically safe to bundle. Before production release, each selected runtime/model must have code license, model-weight license, commercial-use terms, attribution, redistribution, and output-ownership reviewed.

| Item | Role | Status | Release note |
| --- | --- | --- | --- |
| ComfyUI | External image workflow server | Pending | Not bundled. Workflows and node dependencies need review before shipping templates as production defaults. |
| Stable Fast 3D | Candidate local 3D runtime backend | Pending | Manual SF3D smoke exists, but code/model terms, commercial threshold, model access, and redistribution rules must be recorded before choosing it as default. |
| TripoSR | Candidate local 3D runtime fallback | Pending | Manual TripoSR smoke exists, but exact repo/model licenses and output terms must be recorded before production use. |
| Hunyuan3D | Candidate higher-quality 3D runtime | Pending | Not integrated. Requires separate code, model-weight, VRAM, texture pipeline, and commercial-use review. |
| TRELLIS | Candidate 3D generation runtime | Pending | Not integrated. Requires separate code, model-weight, and commercial-use review. |
| Unity glTFast | Unity import verifier dependency | Pending | Used by the Unity import smoke through Package Manager; release needs package license and redistribution note. |
| Tauri | Desktop shell | Blocked | Direct dependency exists, but Rust transitive dependency SBOM is blocked until `Cargo.lock` is committed. |
| FastAPI | Agent API framework | Automated | Covered by `apps/agent/requirements-release.lock`; transitive Python runtime dependencies are pinned with license metadata. |
| Phosphor Icons for React | CAD 工作台图标 | Automated | `@phosphor-icons/react@2.1.10`，MIT；由 `package-lock.json` 固定并进入 npm license gate。 |

## Current Production Blockers

- Commit and audit `apps/desktop/src-tauri/Cargo.lock`.
- Decide the first production 3D backend and record its code/model license terms.
- Record Unity glTFast package license and distribution requirements.
- Add an attribution bundle for licenses that require notices.
