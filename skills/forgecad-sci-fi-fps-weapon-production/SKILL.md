---
name: forgecad-sci-fi-fps-weapon-production
description: Orchestrate original, nonfunctional sci-fi FPS game or film visual assets through ForgeCAD's typed, stage-gated design loop. Use when Codex must turn an authorized brief or reference into a reviewable fictional energy-rifle/weapon-style asset, coordinate bounded geometry and appearance actions, preserve reference uncertainty, or prepare evidence for human approval without using arbitrary scripts, network services, proprietary game assets, or manufacturing claims.
---

# ForgeCAD Sci-Fi FPS Weapon Production

## Purpose

Use this skill as Codex-side orchestration only. ForgeCAD Runtime remains the sole
writer of project, candidate, CAS, version, Job, quality, and evidence state. The
skill may plan and call typed MCP actions, but it must not become a second database,
local geometry compiler, model provider, or approval authority.

The only allowed product scope is an original `fictional-game-asset` with
`nonfunctional_asset=true`: a game, film, or presentation prop with no manufacturing
dimensions, materials recipe, performance, operation, safety, or engineering claim.
Do not use or reproduce proprietary models, textures, logos, or patterns from
《生死狙击》 or any other named commercial game. Ask for a new authorized reference
or use a synthetic brief when provenance is unclear.

## Session entry

For every fresh ForgeCAD design session, perform this order before any design tool
or other Skill:

1. Read `ponytail-preflight@0.1.0` with `skill_get`. The bootstrap diagnostics
   `capabilities_get`, `runtime_status`, and `doctor` are the only read-only
   exceptions before preflight.
2. Read the live capability and Operator Catalog hashes. Treat the Runtime response
   as authoritative; never recreate a catalog or canonical hash locally.
3. Read the active Skill/AssetPack manifests and verify that the requested route is
   actually available. A draft Bundle in this repository is not registered merely
   because it exists on disk.
4. Bind the project, authorized reference CAS object(s), candidate, camera, and
   observation/evidence hashes. Do not pass local paths, URLs, image bytes, secrets,
   environment variables, or prompts into Runtime truth.

Load [contract-and-operator-map.md](references/contract-and-operator-map.md) when
constructing a profile, plan, stage action, or evidence handoff.

## Production loop

Keep the loop linear and observable:

`Intake → Profile → ReferenceCanvas → Primary Form → Secondary Structure → Tertiary Detail → UV/PBR → Review`

At every stage use:

`Observe → Plan one bounded action → Prepare → Strict readback → Render/compare → Evaluate → Ask/Checkpoint`

Do not batch independent speculative edits, continue after a failed gate, or treat a
natural-language impression as a quality result.

### 1. Intake and profile

- Record the creative brief as original style language: silhouette, surface language,
  material IDs, accent placement, and detail density.
- Set `scope=fictional-game-asset`, `nonfunctional_asset=true`, and keep the intended
  use to visual presentation.
- Classify every reference observation as `observed`, `inferred`, or `unknown`.
  Hidden/back-side/function claims remain unknown until separately authorized and
  evidenced.
- Build a `FictionalEnergyRifleProfile@1` only from bounded macro intents. Preserve
  `project_id`, `representation_plan_sha256`, `subject_coordinate_frame_sha256`,
  `operator_catalog_sha256`, and the Runtime-owned canonical hash.
- Profile/plan authoring is structural only: it does not create a candidate, write
  SQLite/CAS, confirm a version, or prove likeness.

### 2. Reference and silhouette gate

- Import only user-authorized PNG/JPEG bytes through the real reference intake path.
  Retain hashes, dimensions, authorization, coverage, and uncertainty; never retain
  an absolute path or raw image in the skill output.
- Treat the global contour as the highest visual authority. Use neutral visual
  regions, continuity groups, layers, overlap/shared boundaries, depth policy, and
  line flow; do not force an imagined functional taxonomy.
- Keep `HQ_360_PASS` blocked until the five identity views
  `front/back/left/right/rear-three-quarter` are supplied and bound. Perspective,
  top, material, and detail views do not substitute for identity coverage.
- Do not unlock tertiary detail or PBR while the primary silhouette/proportion gate
  is failed, unbound, or only inferred.

### 3. Stage actions

Use the smallest active typed route and one semantic Part or MaterialZone at a time.
The current Runtime owns macro expansion and bounded search; Codex does not run a
continuous parameter loop. A proposed action must include the current candidate,
reference/camera/observation lineage, stage, exact Part or MaterialZone scope, and
the declared budget.

| Stage | Allowed intent | Required evidence before advancing |
| --- | --- | --- |
| `primary-form` | housing/frame/panel silhouette and proportion | candidate-bound contour, strict readback, fixed camera compare |
| `secondary-structure` | vents, joints, sensor and module hierarchy | semantic/source map, Part evidence, non-regressing compare |
| `tertiary-detail` | panel breaks, grooves, fasteners, line flow | detail evidence tied to visible region and bounded operator |
| `uv-pbr` | MaterialZone, UV/tangent, glTF PBR subset | visible-view gate, channel readback, asset provenance |
| `final-review` | nine AOV, compare, typed review, human review | strict metrics, same hashes, explicit user approval |

For geometry use the active `GeometryProgram@2`/PDK path and strict
`ArtifactReadback@2`; for appearance use the active bounded `AppearanceProgram@2`
and PBR readback. Use the fixed RenderSet and nine AOVs for review. A staged candidate
is not a confirmed version.

### 4. Approval and recovery

- Keep `QUALITY_TARGET_NOT_MET`, `BLOCKED_REFERENCE_COVERAGE`, and typed binding
  failures visible. Never soften a threshold or relabel a structural receipt as a
  visual PASS.
- On failure, stop at the failed stage, record the evidence hash and one bounded
  repair suggestion, then ask the user when the action is consequential.
- `prepare` may produce a staged candidate; only an explicit user approval may allow
  `confirm`, immutable version creation, restore-as-new-version, or export.
- Never send messages, invitations, calendar commitments, payments, or offline
  actions on the user's behalf. Never silently approve or export.

## Material and asset discipline

Use only product-owned, offline, hash-bound material declarations. The draft Bundle
declares a small glTF PBR subset (`white-dielectric-clearcoat`, `dark-painted-metal`,
`black-anodized-metal`, `engineering-plastic`, and `warm-orange-emissive`) but carries
no texture payload and does not download assets. If an asset or texture is needed,
require a first-party/offline AssetPack manifest with license, hash, provenance,
channel semantics, and modification status. Do not install BlenderMCP, FreeCAD MCP,
CadQuery/build123d MCP, arbitrary Python/JavaScript, remote image-to-3D services, or
unreviewed GitHub repositories.

## Evidence handoff

End each turn with a compact, hash-oriented handoff:

- scope and authorization state;
- current stage and exact Part/MaterialZone;
- Runtime/worker/renderer cohort and canonical input/output hashes;
- readback, AOV, compare, typed review, approval, and export status;
- `PASS`, `FAIL`, `BLOCKED`, and `NOT_RUN` separated;
- unknowns, limitations, and the one safe next action.

Use the repository Bundle only as a reviewable draft until the maintainer adds a
registry entry, generated aggregate manifest, focused Runtime consumer evidence, and
the required release gates. This Codex Skill is intentionally not installed under
`~/.codex/skills` and must not be copied there automatically.
