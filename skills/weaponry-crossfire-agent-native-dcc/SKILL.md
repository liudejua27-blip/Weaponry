---
name: weaponry-crossfire-agent-native-dcc
description: Orchestrate an authorized nonfunctional game or film knife through Weaponry's Rust-owned 11-façade workflow. Use for reference intake, typed curve and AuthoringMesh transactions, High-to-Low surface production, UV and bake, material, FPS presentation, engine validation, quality review, approval, and recovery; do not use for real-world weapon guidance or arbitrary DCC/script execution.
---

# Weaponry knife production orchestration

Use this Skill as Codex-side orchestration for one reviewable knife candidate. The
Weaponry Runtime is the sole writer of AuthoringMesh, candidates, Jobs, SQLite/CAS,
versions, quality evidence, and approval state. Codex plans and dispatches closed
typed operations; it is not a mesh editor, database, quality authority, or arbitrary
code executor.

This workflow produces nonfunctional game/film visual assets only. Do not derive or
provide manufacturing dimensions, drawings, machining, material recipes, performance,
or operating guidance for real weapons.

## When to use

Use for an authorized knife reference or an original control knife. A named reference
profile is a benchmark input, not a kernel operator or a mandatory art direction. If
the input is missing authorization, use only an original control asset and mark the
commercial reference gate blocked. For the sanitized optional Dragonfang profile,
read [dragonfang-benchmark-profile.md](references/dragonfang-benchmark-profile.md).

Do not use this Skill to run Blender/Python/JavaScript/shell, install plugins, fetch
network assets, open a `.blend` as truth, or bypass a user decision. A fixed Blender
prototype is allowed only when the live Runtime advertises a closed typed offline
provider job; its output remains an observation until Rust readback, hash binding,
and the normal gates succeed.

## Session entry

For every fresh Weaponry MCP session:

1. Call `skill_get` for `ponytail-preflight@0.1.0` before design calls.
2. Read the live `weaponry-knife-p0-default@1` profile, capabilities, operator,
   Skill, and AssetPack hashes. The default surface is exactly the 11 façades below;
   legacy raw tools require the explicit compatibility profile.
   The checked-in profile and native curve operations are currently
   `development-only`. Unless the live Runtime advertises a promoted profile and
   all candidate-bound gates pass, keep `commercial=NOT_PROVEN` and do not present
   this Skill, its contracts, or source tests as a commercial asset result.
3. Normalize the brief before geometry: record every competing value (for example
   triangle, texture, engine, view, and action requirements), preserve its source
   and hash, and freeze a user-selected value or mark it `UNRESOLVED`. After
   `reference_import`, call `weaponry_knife_production_brief_prepare` through
   `reference_intake`, then verify it with `weaponry_knife_production_brief_get`.
   The initial Brief uses `initial-intake-no-parent@1`; resolving a conflict creates
   a new hash-bound parent-linked Brief with
   `immutable-successor-preserve-source-claims@1`. Never overwrite the parent,
   remove a losing claim, or choose a value because it appears in a benchmark image
   or prose attachment.
4. Bind project, candidate, authorization, reference, camera, target-engine, and
   evidence hashes. Do not treat a repository prose claim or an attachment path as
   a content hash. A contact card, signature image, or brand mark is not a human
   receipt and must not be copied into evidence or the repository.
5. Observe the current original AuthoringMesh, stable IDs, ModifierGraph, evaluated
   lineage, stage, and last confirmed head before proposing a change.
6. State the target artifact, budget, promotion policy, and required human/engine
   gates. If an input is unknown, preserve it as unknown rather than inventing it.

Use `weapon_preflight` for prerequisites, `reference_intake` for the durable Brief,
and `authoring_transaction` only after Runtime reports the Brief authoring-eligible.
Persisting a blocked Brief is successful intake, not permission to create geometry.

Read [action-space-and-gates.md](references/action-space-and-gates.md) when selecting
operations, preparing a transaction, evaluating High/Low/UV/Bake, or handing off
review evidence.

Before the first Dragonfang geometry write and after every failed High review, read
[knife-reference-convergence-loop.md](references/knife-reference-convergence-loop.md).
An eligible Brief does not unlock arbitrary High work: the Runtime-owned
`KnifeReferenceIntentBundle@1`, `KnifePassState@1`, and
`KnifeCorrectionLedger@1` must all be present. Keep the public Action Space at
the same 11 façades.

## The closed 11-façade route

Use this order and the live profile's exact route names. `job` and `recovery` are
cross-cutting controls; they never authorize skipping a production gate.

1. `weapon_preflight` — session, capabilities, profile, authorization prerequisites.
2. `reference_intake` — create the project and admit authorized references.
3. `observe` — inspect scene, stable IDs, lineage, stage, and current candidate.
4. `authoring_transaction` — select/query stable IDs; form blade curves; compose a
   closed AuthoringMesh transaction and ordered ModifierGraph.
5. `surface_pipeline` — produce and read back High, editable Low, correspondence,
   Hero UV, cage, bake maps, and MaterialZones in that order.
6. `fps_presentation` — bind first-person cameras, sockets, and nonfunctional clips.
7. `quality_review` — run the pre-delivery fixed-view/AOV, reference-comparison and
   typed-critic gate. Do not submit final human acceptance yet.
8. `delivery` — prepare target-engine interchange and obtain an engine-bound
   validation receipt for the exported artifact.
9. `approval` — first return to `quality_review` for the final candidate/export-bound
   independent human decision, then present that exact evidence; only the user can
   approve confirmation or export.
10. `recovery` — reject, checkpoint, restore, and replay without rewriting history.
11. `job` — inspect/cancel bounded work and preserve its receipt and cohort.

The canonical production progression is:

`Reference → Curve/AuthoringTransaction → High → editable Low → UV/Cage/Bake → Material → FPS → Quality(pre-delivery) → Engine → Quality(final human) → Approval → Recovery/export`

Use `observe` between every write and downstream handoff. A failed prepare/evaluate,
readback, budget, or quality gate leaves the original document and last confirmed head
untouched; retain the failure receipt and stop at that stage.

## Authoring and surface rules

- Batch only dependent edits forming one local intent. Use stable-ID selections or
  typed semantic queries; never infer IDs from array positions after topology changes.
- Every write is `preview → validate → prepare → readback`; transactions reject
  atomically on invalid IDs, non-finite values, topology/manifold policy violations,
  or budget overflow.
- AuthoringMesh is the source of truth. Modifier order and provider/version hashes
  are evidence; EvaluatedMesh, High, Low, UV, bake maps, and render outputs are
  derived artifacts and never overwrite their parent.
- Low must remain artist-editable with High↔Low correspondence and feature locks.
  UV, cage, tangent, bake-channel, and PBR diagnostics are mandatory; a pretty
  render cannot compensate for a failed form or surface gate.
- Material layers follow semantic MaterialZones and carry channel/provenance
  readback. Avoid unregistered decals, textures, or hidden fallback materials.

## Quality, approval, and truth labels

Review one exact candidate hash in beauty, depth, normal, AO, part-ID, material-ID,
wireframe, UV-stretch, and silhouette views. A single three-quarter reference can
only yield `PARTIAL_VISIBLE_VIEW_PASS`; keep `HQ_360_PASS` blocked until required
identity views are hash-bound.

Keep these labels separate: Runtime/authoring, surface/game-ready, visual/reference,
human accepted, engine validated, packaged, and released. Source compile, Skill
activation, GLB readability, or Codex self-review never proves commercial quality.
Only explicit user approval may confirm a staged candidate or authorize export.

## Evidence handoff

End each turn with the current stage and a single evidence-backed next action. Include
authorization/reference status; project/candidate and original/evaluated/High/Low/UV/
bake/material/FPS/engine hashes; changed stable IDs; Runtime/worker/provider cohort;
topology/readback/AOV/compare results; human and engine decisions; and independent
`PASS`, `FAIL`, `BLOCKED`, and `NOT_RUN` labels. Never claim a later stage from an
earlier source or structural pass.
