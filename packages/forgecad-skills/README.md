# ForgeCAD first-party MVP Skill registry

`registry.json` is the development-only aggregate manifest for the ten MVP
Skills. Each entry is also materialized under
`bundles/<skill-id>/0.1.0/` by `scripts/materialize_mcp006_bundles.py`; the
independent directory contains its own contracts, Recipe, operator/validator
allowlists, synthetic fixtures/receipt, license, SBOM, provenance and trust
manifest. Runtime may expose metadata and validate the allowlist, but it never
executes content from this directory. The actual Operator implementations
remain in the Rust Runtime/Workers.

The registry records the useful ideas reviewed from
[`img2threejs`](https://github.com/img2threejs/img2threejs) (staged passes,
detail inventory and comparison) and [`img2css`](https://github.com/javierbyte/img2css)
(bounded color/region preview). Their code, JavaScript, CSS and assets are not
vendored or executed.

This is an MVP development trust profile. The checked-in `signature.bundle`
files are explicit non-cryptographic placeholders; distribution signatures,
revocation and third-party installation remain MCP012–013 work. Synthetic
bundle receipts prove declarative safety only, not geometry, render or visual
quality.
