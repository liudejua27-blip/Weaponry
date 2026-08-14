# ForgeCAD first-party MVP Skill registry

`registry.json` is the development-only aggregate manifest for the twelve
active first-party Skills. Each registry entry is materialized under
`bundles/<skill-id>/<active-semver>/` by `scripts/materialize_mcp006_bundles.py`;
the independent directory contains its own contracts, Recipe, operator/validator
allowlists, synthetic fixtures/receipt, license, SBOM, provenance and trust
manifest. Runtime embeds only the registry-declared Bundle versions and never
executes Bundle content. The actual Operator implementations remain in the Rust
Runtime/Workers.

Superseded Bundle versions belong in `archive/superseded/`, not in `bundles/`.
The archive manifest preserves their content hash and replacement while keeping
them out of `skill_list`, the Runtime build archive and the active capability
count.

The registry records the useful ideas reviewed from
[`img2threejs`](https://github.com/img2threejs/img2threejs) (staged passes,
detail inventory and comparison) and [`img2css`](https://github.com/javierbyte/img2css)
(bounded color/region preview). Their code, JavaScript, CSS and assets are not
vendored or executed.

`ponytail-preflight@0.1.0` is a first-party rewrite of a reviewed MIT workflow
reference. MCP requires Codex to read it with `skill_get` before a design tool
or another Skill; it returns static checked-in knowledge and does not install
or execute the upstream Node package, hooks or MCP server.

This is an MVP development trust profile. The checked-in `signature.bundle`
files are explicit non-cryptographic placeholders; distribution signatures,
revocation and third-party installation remain MCP012–013 work. Synthetic
bundle receipts prove declarative safety only, not geometry, render or visual
quality.
