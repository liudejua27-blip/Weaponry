# Weaponry one-month delivery plan

> 十天首交付由 `WEAPONRY_KNIFE_10_DAY_DELIVERY_PLAN.md` 与 ADR-0030 管理；本文件继续管理
> 后续一个月扩展到更广穿越火线武器的队列，不再作为刀类 Day 1–10 的直接计划。

## Delivery truth

The achievable target is **CrossFire weapon Agent-native DCC V1**, not "Rust
Blender completed in one month".  Claiming the latter would be false: Blender's
modeling, animation, simulation, rendering, compositing and extension surface is
the result of decades of work.  V1 is successful only when Codex can construct,
revise, evaluate and deliver at least one authorized production weapon through a
generic edit/modifier workflow without bespoke code for that asset.

## Acceptance artifact

The acceptance cohort must contain one authorized CrossFire weapon and one
original control weapon.  Both must use the same Runtime build and generic
operators.  At minimum, the final receipt binds:

- authorization and reference hashes;
- original AuthoringMesh and evaluated mesh hashes;
- modifier graph and deterministic replay hash;
- semantic Parts and MaterialZones;
- high/low/UV/cage/bake lineage;
- fixed first-person, inspect, ADS and orthographic review cameras;
- beauty, depth, normal, AO, part-ID, material-ID, wireframe, UV-stretch and
  silhouette evidence;
- engine interchange validation;
- independent artist review and explicit user approval.

## Week 1 — authoring kernel and deletion boundary

- Land stable-ID mesh topology, selection/query and atomic transaction APIs.
- Execute move/split/extrude as the first tested vertical slice; add the remaining
  core edit operators behind the same transaction contract.
- Define original versus evaluated documents and an ordered modifier graph.
- Produce a generated reachability report for all public tools, Schemas and Skills.
- Remove only manifest-approved dead surfaces; do not bulk-delete dirty evidence.

Exit: a multi-operation edit replays deterministically, invalid operations roll
back completely, and strict topology validation passes.

## Week 2 — hard-surface modifier and weapon workflow

- Implement mirror, array, boolean, bevel, solidify, subdivision and
  weighted-normal evaluation.
- Use an audited Manifold provider for robust boolean rather than writing a weak
  boolean solely to satisfy "pure Rust" branding.
- Express receiver/stock/shroud/rail/sight construction as macros over generic
  operations.
- Replace one-operation-per-turn orchestration with atomic edit batches followed
  by checkpoint, render and critic evaluation.

Exit: Codex can reproduce the same blockout from a command journal and repair a
selected region without replacing unrelated stable IDs.

## Week 3 — game-ready surface and delivery

- High/low association, seams, UV islands, cage, normal/AO bake and glTF PBR.
- Hard-surface material library and authorized texture/provenance records.
- First-person, inspect and ADS camera/occlusion checks.
- Unreal interchange validation modeled on BlenderTools' delivery discipline,
  without embedding Blender or Unreal as truth owners.

Exit: engine-importable candidate with same-version geometry, appearance and bake
lineage; no visual-quality claim yet.

## Week 4 — authorized acceptance cohort

- Run the authorized CrossFire weapon and original control weapon end to end.
- Fix only evidence-localized regressions; do not mask failed geometry with PBR/VFX.
- Run deterministic replay, restart/export hash, fixed-view comparison and human
  artist gate.
- Package only the cohort that passed; keep failed gates explicit.

Exit labels remain separate: `RUNTIME_PASS`, `DELIVERY_PASS`, `VISUAL_PASS`,
`HUMAN_ACCEPTED`, `PACKAGED`, and `RELEASED` must never be collapsed.

## Scope cuts if schedule slips

Cut in this order:

1. general node-editor UI;
2. NURBS and shape keys;
3. sculpt and retopology automation;
4. broad animation tooling;
5. non-weapon asset categories.

Never cut topology validation, rollback, provenance, deterministic replay, fixed
view evidence or human acceptance.  Those are the product's truth boundary.
