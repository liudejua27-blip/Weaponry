---
name: weaponry-crossfire-agent-native-dcc
description: Orchestrate authorized CrossFire and original knife visual assets through Weaponry's Rust-owned Agent-native DCC. Use when Codex must compose stable-ID mesh edits, modifier evaluation, high/low/UV/bake, material, first-person animation, review and engine-delivery actions while preserving transactions, provenance, quality evidence and explicit approval.
---

# Weaponry CrossFire knife Agent-native DCC

## Purpose

Use this Skill as Codex-side modeling orchestration.  Weaponry Runtime remains the
sole writer of authoring documents, evaluated meshes, candidates, SQLite/CAS,
versions, Jobs, quality and evidence.  Codex may compose typed actions but must not
become a competing mesh editor, database, approval authority or arbitrary-code
executor.

The ten-day P0 scope is nonfunctional game/film knife visual assets.  The user's
reported CrossFire cooperation is an authorization assertion, not a replacement for
per-asset provenance.  Bind each private reference, mesh, texture, logo and delivery
target to an authorization manifest before use.  Never output manufacturing
drawings, real weapon dimensions, material recipes, machining, performance or
operating guidance.

## Session entry

For every fresh Weaponry session:

1. Read `ponytail-preflight@0.1.0` through `skill_get`; only bootstrap diagnostics
   may precede it.
2. Read live capability, operator, modifier and active Skill/AssetPack hashes.
3. Bind project, candidate, authorization, reference, camera and evidence hashes.
4. Inspect the current original AuthoringMesh, modifier graph and evaluated-mesh
   lineage before proposing an edit.
5. Treat repository declarations as drafts until Runtime advertises the same
   canonical hash.

Read [action-space-and-gates.md](references/action-space-and-gates.md) before
constructing an edit transaction, modifier graph, bake or delivery handoff.

## Modeling loop

Use this state flow:

`Observe → Select/query → Plan transaction → Preview on clone → Validate → Prepare → Evaluate modifiers → Read back → Render/compare → Critic → Checkpoint/approval`

### Atomic edit transactions

Multiple dependent edit operations may be batched when they form one coherent local
intent.  This replaces the obsolete one-Part/one-operation restriction.

Every `AuthoringTransaction@1` must:

- bind the original document and candidate hash;
- use stable vertex/edge/face/Part/MaterialZone IDs or typed semantic queries;
- declare operation and topology budgets;
- preserve an ordered command journal;
- execute against a cloned authoring document;
- fail atomically with no partial write;
- run finite-coordinate, connectivity, manifold-policy and budget validation;
- return strict before/after hashes and changed-ID sets.

Do not batch unrelated speculative alternatives.  Use separate candidate branches
for alternatives, not one transaction with hidden fallback behavior.

### Non-destructive evaluation

The original AuthoringMesh is truth.  Transform, mirror, array, boolean, bevel,
solidify, subdivision and weighted-normal nodes form an ordered ModifierGraph.
Evaluation produces a disposable mesh and never rewrites the original implicitly.
Modifier order, parameters, provider version, budget and input/output hashes are
part of evidence.  A failed node invalidates downstream evaluated results while
leaving the authoring document recoverable.

### Knife production stages

1. `primary-form`: blade silhouette, spine/edge/tip profile, handle/guard proportion and first-person occupancy.
2. `secondary-form`: bevel language, fuller/groove, serration, guard, pommel and grip hierarchy.
3. `hard-surface`: controlled curvature, edge highlights, normals, engraved/patterned detail and repeated grip structure.
4. `game-ready`: high/low, seams, UV, cage, normal/AO/curvature bake and PBR zones.
5. `delivery`: idle/inspect/slash/stab clips, first-person cameras, AOV review and engine interchange.

Weapon names describe semantic macros and benchmarks, not kernel operators.  Prefer
generic split/extrude/inset/bevel/bridge/dissolve/merge/loop-cut actions plus
modifiers.  Add a new kernel operation only when the geometry operation is reusable
outside one named asset.

## Quality and approval

- Geometry gates precede materials and VFX; appearance must not hide failed form.
- A single three-quarter reference can support only `PARTIAL_VISIBLE_VIEW_PASS`.
  Keep `HQ_360_PASS` blocked until required identity views are hash-bound.
- Review the same evaluated candidate in beauty, depth, normal, AO, part-ID,
  material-ID, wireframe, UV-stretch and silhouette AOVs.
- Keep Runtime, delivery, visual, human-acceptance, packaging and release labels
  separate.
- Only explicit user approval may confirm a staged candidate or authorize export.

## Extension boundary

Do not run caller-supplied Python, JavaScript, shell, Blender add-ons, network services
or unreviewed native plugins. ADR-0030 permits fixed Blender and fixed add-ons only as
offline internal providers behind a closed typed job. Such a provider must have pinned
versions, declared resource bounds, deterministic knife fixtures, license/SBOM/provenance,
Rust readback and an exit strategy. `.blend`, Python state and plugin state never become
product truth or direct approval evidence.

## Evidence handoff

End each modeling turn with:

- authorization/reference state and current production stage;
- original AuthoringMesh, transaction journal, ModifierGraph and EvaluatedMesh
  hashes;
- changed stable IDs and provider/cohort versions;
- topology, readback, AOV, compare, bake and engine-delivery results;
- `PASS`, `FAIL`, `BLOCKED` and `NOT_RUN` separated;
- explicit approval/export state and one evidence-backed next action.

This repository Skill is not automatically installed or registered.
