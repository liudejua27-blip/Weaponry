# ForgeCAD Skill Archive

This directory is not part of the active Skill registry.

Archived Skills are retained only for historical provenance, migration review, or evidence comparison. They must not be loaded by `skill_list`, referenced by `packages/forgecad-skills/registry.json`, or used as current product capability.

Rules:

- archived bundles keep their original receipt/status files;
- archived bundles must remain outside `packages/forgecad-skills/bundles/**`;
- archived bundles must not be counted as active first-party Skills;
- any reuse requires a new active bundle, new manifest hash, current Schema, current validator, benchmark, LICENSE/NOTICE/SBOM/provenance, and Runtime consumer evidence.

Current archive:

- `superseded/reference-to-typed-plan/0.1.0`: superseded provenance from the early image-to-typed-plan Skill; replaced by the materialized MCP006 first-party bundles.
- `superseded/hard-surface-detail/0.1.0`: superseded by the active bounded Operator-backed `hard-surface-detail@0.2.0` Bundle.
- `superseded/uv-pbr/0.1.0`: superseded by the active AssetPack/PBR-backed `uv-pbr@0.2.0` Bundle.

`superseded/manifest.json` records the immutable pre-archive tree hash and
replacement for every archived Bundle. The MCP006 Skill gate verifies it.
