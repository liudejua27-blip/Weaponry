# ADR-0029: Weaponry Agent-native DCC

> **Weaponry P0 override (2026-08-29):** 本文只有在与 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md` 和 ADR-0029 一致时才具有当前执行权。ForgeCAD 在本文中解释为 Weaponry 的 Rust Runtime lineage；当前唯一产品主线是由 Codex 生成、修改、验证并交付高质量穿越火线非功能性游戏武器。通用 3D、机器人和原创科幻示例仅作 fixture/历史能力，不得抢占本月主线。 本文中所有 2026-08-28 及更早的“当前”“下一原子”“唯一 `in_progress`”和工具/Schema 数量语句均按历史 cohort 解释，不得覆盖 `WPN-*` successor queue。

- Status: accepted for implementation
- Date: 2026-08-29
- Supersedes product-direction portions of: ADR-0028
- Preserves: ADR-0025 Runtime ownership and ADR-0026 approval/evidence boundaries

> 2026-08-29 knife-first successor: ADR-0030 将十天首交付收缩为刀类，并允许固定、隔离、
> typed 的 Blender+插件内部原型。本文的 Rust-owned truth、事务、证据和任意代码禁令继续有效；
> “Blender 只作学习来源”的绝对表述由 ADR-0030 局部替代。

## Decision

ForgeCAD will be productized as **Weaponry**, a Rust-owned, Codex-operated,
weapon-specialized Agent-native DCC.  The one-month delivery target is not a
Rust clone of Blender.  It is a production vertical slice for authorized
CrossFire visual weapon assets with a substantially more composable modeling
action space than the current single-purpose tool surface.

Rust Runtime remains the sole writer and owns authoring state, evaluation,
transactions, lineage, evidence and approval.  External libraries may provide
audited algorithms behind typed Worker boundaries; they do not own product
truth.  Blender, BlenderMCP, BlenderTools and img2threejs are learning or
workflow sources, not embedded runtimes.

## First-principles correction

The previous implementation optimized the wrong proxy: contract, Schema and
tool coverage.  A large catalog does not imply a large modeling action space.
The current product exposes many versioned, subject-specific operations while
the generic editable topology and modifier vocabulary remains small.  This
causes Codex to search within a narrow parameter space instead of constructing
forms through composable operations.

Weaponry will measure modeling capability by the following expression:

`selection/query x mesh operations x modifier graph x transaction composition x evaluation evidence`

Tool count is diagnostic only and is not a quality KPI.

## Product state model

```text
Authorized brief/references
          |
          v
Original AuthoringMesh + stable semantic IDs
          |
          v
Ordered Modifier/Evaluation Graph --------+
          |                                |
          v                                |
Disposable EvaluatedMesh                   |
          |                                |
          +--> High/Low/UV/Cage/Bake ------+
          |
          +--> fixed views/AOV/compare/critic
          |
          +--> staged candidate -> explicit approval -> immutable version
```

The evaluated mesh is never an alternative source of truth.  Modifier order is
significant.  A failed evaluation leaves the original authoring document and
last confirmed version untouched.

## Open action space without arbitrary execution

"Do not severely constrain the Action Space" is accepted.  "Allow arbitrary
Python or add-ons inside Runtime" is rejected.  These are not equivalent.
Arbitrary code would make results non-deterministic, unbounded and difficult to
audit, and would invalidate the sole-writer and replay guarantees.

The replacement is a composable typed command algebra:

- stable-ID selection and semantic queries;
- atomic multi-command edit transactions;
- generic topology operators;
- ordered non-destructive modifiers;
- typed geometry-node subgraphs with bounded inputs and declared budgets;
- signed/audited native or WASI extension providers in a later lifecycle;
- deterministic replay, validation, hashes and per-node evidence.

The command algebra is generic.  Weapon specialization belongs in reusable
macros, semantic presets, material libraries, camera rigs, benchmarks and
quality contracts.  Receiver, stock, barrel shroud and sight must not become
hard-coded kernel operators.

## One-month V1 capability boundary

Required:

1. stable-ID mesh selection and inspection;
2. atomic multi-operation transactions with rollback;
3. move, split, extrude, inset, bevel, bridge, dissolve, merge and loop-cut
   authoring primitives;
4. transform, mirror, array, boolean, solidify, subdivision and weighted-normal
   modifier stack;
5. original/evaluated graph separation and dirty-subgraph recompute;
6. high/low association, UV seams/islands, cage and normal/AO bake path;
7. PBR MaterialZones, first-person/inspect/ADS cameras and fixed AOV review;
8. Unreal-oriented GLB/FBX interchange receipt and hash/provenance handoff;
9. authorized CrossFire asset manifest and independent human acceptance gate.

Deferred:

- sculpting parity, full NURBS, shape keys and production animation editor;
- general Geometry Nodes parity;
- arbitrary Python/add-ons;
- full Blender UI parity;
- universal asset categories outside weapon production.

## CrossFire authorization boundary

The reported commercial cooperation is treated as a user-provided authorization
assertion, not as a blanket provenance substitute.  Every private reference,
source mesh, texture, logo and export target must still be bound to an
authorization record containing source hash, permitted use and project scope.
Weaponry only produces nonfunctional game/film visual assets and does not expose
manufacturing drawings, real-world weapon dimensions, machining instructions or
performance guidance.

## Deletion and migration policy

Deletion follows reachability and product value, not naming preference:

- remove public tools that only report permanently unavailable forbidden paths;
- replace the unregistered original-only sci-fi orchestration Skill;
- hide superseded V1/V2/V3 public surfaces behind a legacy profile before code
  removal;
- consolidate subject-specific weapon operators into macros over generic kernel
  actions;
- retain animation, VFX and delivery capabilities that are required by the
  authorized game-asset pipeline;
- never delete generated evidence, user references or unknown dirty-worktree
  files without a manifest and recoverable boundary.

## Consequences

This decision intentionally lowers the number of public concepts while raising
their composition power.  Existing quality receipts remain historical evidence;
they do not prove this architecture is implemented.  Each capability is promoted
only after typed execution, strict readback, deterministic replay and visual/human
gates pass for the same candidate hash.
