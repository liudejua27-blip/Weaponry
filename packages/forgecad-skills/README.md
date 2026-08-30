# ForgeCAD first-party MVP Skill registry

> **Weaponry P0 override (2026-08-29):** 本文只有在与 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md` 和 ADR-0029 一致时才具有当前执行权。ForgeCAD 在本文中解释为 Weaponry 的 Rust Runtime lineage；当前唯一产品主线是由 Codex 生成、修改、验证并交付高质量穿越火线非功能性游戏武器。通用 3D、机器人和原创科幻示例仅作 fixture/历史能力，不得抢占本月主线。 本文中所有 2026-08-28 及更早的“当前”“下一原子”“唯一 `in_progress`”和工具/Schema 数量语句均按历史 cohort 解释，不得覆盖 `WPN-*` successor queue。

`registry.json` is the development-only aggregate manifest for the twelve
active first-party Skills. Each registry entry is materialized under
`bundles/<skill-id>/<active-semver>/` by `scripts/materialize_mcp006_bundles.py`;
the independent directory contains its own contracts, Recipe, operator/validator
allowlists, synthetic fixtures/receipt, license, SBOM, provenance and trust
manifest. Runtime embeds only the registry-declared Bundle versions and never
executes Bundle content. The actual Operator implementations remain in the Rust
Runtime/Workers.

All twelve current Bundle versions are now `MIGRATION_REQUIRED` for the Weaponry
CrossFire P0 direction. They remain active only to preserve current runtime replay;
their `knowledge/**` files are hash-bound and must not be edited in place. Each
successor must version its authorization, weapon-authoring, High/Low/UV/Bake,
surface, FPS/engine, evidence and recovery behavior and must pass both an authorized
CrossFire cohort and an original control cohort before registry activation.

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
