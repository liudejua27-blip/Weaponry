---
name: weaponry-threejs-knife-studio
description: Design, generate, evaluate, and refine authorized nonfunctional knife assets as canonical Weaponry scene programs compiled to Three.js. Use for the lightweight browser-asset route, img2threejs compatibility intake, deterministic fixed-view comparison, and bounded knife-specific correction; do not use as proof of UE5-ready High-to-Low commercial delivery or for real-world weapon construction.
---

# Weaponry Three.js Knife Studio

Build one editable, reproducible Three.js knife asset from a brief, an original design, or an optional authorized
reference used for inspiration or comparison. A reference-inspired design is not a promise of 1:1 reconstruction.
Use img2threejs as an accepted compatibility baseline, then normalize its ObjectSculptSpec into the stronger
Weaponry `KnifeSceneProgram@1`. Generated TypeScript, `THREE.Group`, GLB, and screenshots are derived
artifacts; the Runtime-owned program, goal ledger, hashes, and provenance are authoritative.

This Skill is only for nonfunctional game, film, and display assets. Never provide manufacturing dimensions,
machining plans, structural performance, material recipes, or real-world operating guidance.

Declare the requested completion tier before authoring: `procedural-draft`, `reference-similar`, or `commercial`.
For the lightweight first phase, default to `procedural-draft`: require a closed program, legal geometry, complete
Part/Material ownership, fixed-view observability, a nonzero delta from an explicit replayable baseline, and strict
readback of the actual derived GLB bytes. A reference image and likeness score
are optional at this tier. If they are not requested, report likeness as `NOT_REQUESTED`, not `PASS`. The higher
tiers retain their reference, engine, bake, human-review, and commercial evidence requirements.

## Closed method: local knowledge, discrete grammar, objective, search

Every draft follows the four-layer contract defined by the local
`KnifeKnowledge@1` object (`references/crossfire-knife-knowledge.json`), the
objective ledger, and the bounded search script—not an informal visual
impression:

1. **Local weapon knowledge space.** Read and validate the checked-in knowledge
   object and its schema. Its claims are bounded priors with explicit
   `observed`, `inferred`, `design-prior`, `original-choice`, or `unknown`
   classification. Web research is allowed as an acquisition input: preserve
   its URL, retrieval date, license/provenance and extracted claim separately,
   then admit only an abstract normalized ratio, grammar rule, material cue or
   composition prior as `design-prior`/`inferred`. A web result, model guess, or
   family stereotype is never a source for an observed feature and copied code,
   textures or meshes require their own license/adoption review. If an authorized view does not show a back
   face, thickness, socket, attachment, or other structure, preserve it as
   `unknown` or explicitly mark the continuation `inferred`.
2. **Discrete grammar.** Emit only the finite curve bases, section roles,
   semantic assembly styles, and bounded parameter paths declared by the
   grammar. The grammar is a construction language, not a license for an
   arbitrary mesh, script, URL, or hidden-geometry completion.
3. **Mathematical objective.** Compare a candidate `c` with the frozen baseline
   `b` using named normalized metric errors and hard gates. Direction-aware
   improvement is `Δ_i(c,b) = m_i(c)-m_i(b)` for a maximize metric and
   `m_i(b)-m_i(c)` for a minimize metric. Missing evidence is
   `NOT_COMPUTABLE`, never zero; a failed hard gate has an infinite penalty.
   The append-only metric catalog separates legacy fixed-view/reference IDs
   from renderer-free blade and assembly IDs. `KnifeIntrinsicMorphology@1`
   measures curve smoothness, section continuity, taper and extrema headroom;
   `KnifeAssemblyIntrinsicMetrics@1` measures ratio priors, attachment continuity,
   MaterialZone readability proxies and bounded complexity. Adapter@2 may rank
   these values, but always reports visual quality as `NOT_COMPUTABLE` and quality
   as `NOT_RUN`.
4. **Candidate search.** Use `scripts/search_candidates.py` with an explicit
   seed and ledger budget (at most 32 candidates). It mutates only
   `allowed_scope`, preserves frozen hashes, ranks a direction-aware Pareto
   set, and emits `REVIEW_ONLY` successor proposals. It does not browse,
   render, or self-approve.

This method supports both original and reference-inspired designs. A reference is
optional for authoring, and reference-inspired does not mean 1:1 reconstruction.
Rendered images and AOVs are optional quantitative comparison inputs (mask,
boundary, landmark, continuity, ownership, and occupancy errors). An original or
reference-inspired structural draft may be valid without them, but image-dependent
terms are then `NOT_RUN` when not requested or `NOT_COMPUTABLE` when required and
missing—not zero. Intrinsic geometry metrics may still be computed, but they cannot
stand in for visual evidence. Completely disabling visual validation is therefore
valid only when the declared objective excludes visual likeness/quality; it cannot
support a visual convergence/superiority claim or `HUMAN_ACCEPTED`/
`COMMERCIAL_ACCEPTED`. A screenshot that “looks better” cannot override a metric,
a missing-evidence state, a camera mismatch, or a failed hard gate. Human art
direction remains a separate later gate.

## Read only what the current stage needs

- Before authoring or importing a program, read [architecture-and-truth.md](references/architecture-and-truth.md).
- Before creating curves, sections, parts, materials, or presentation, read
  [knife-design-language.md](references/knife-design-language.md). When creating or revising design priors,
  validate the bounded knowledge object against
  [knife-knowledge.schema.json](references/knife-knowledge.schema.json).
- When creating or revising a material, use the closed
  [`KnifeLayeredMaterialSpec@1`](references/knife-layered-material.schema.json) vocabulary only:
  `red-lacquer-metal`, `antique-gold`, `black-wrapped-grip`, or `ruby-emissive`. Its only procedural
  controls are bounded `curvature`, `edge_wear`, `engraving_mask`, and `scale_repeat` (U/V). Material
  input is declarative: URL textures, arbitrary shader/code input, and network material dependencies are
  invalid. Readability layers cannot repair silhouette, section, topology, normal, UV, attachment, or
  Part-ID geometry; return to the geometry scope when one of those gates fails.
- Before drafting a `KnifeDesignIntent`, search the checked-in
  [crossfire-knife-knowledge.json](references/crossfire-knife-knowledge.json) with
  `scripts/search_knife_knowledge.py`. Treat returned records only as ranked design priors; they cannot become
  observed reference evidence, real-world dimensions, or quality approval. Network research may produce a proposed
  knowledge successor only through the provenance/classification process above; the URL itself never becomes geometry
  truth. A normalized, licensed design prior may influence an `inferred` or `original-choice` program field after its
  uncertainty is retained. If neither a bounded prior nor an explicit original choice is available, preserve the gap.
- Before using online research to extend the knowledge space, follow
  [research-to-design-prior.md](references/research-to-design-prior.md). Research expands candidate priors; it does not
  bypass the closed grammar, evidence computation, licensing boundary, or readiness gate.
- Before evaluating or changing a goal, read [objective-and-quality.md](references/objective-and-quality.md).
- Use [dragonfang-first-slice.json](references/dragonfang-first-slice.json) only as a normalized-unit template,
  not as a finished asset or observed truth.
- Keep [dragonfang-objective-ledger-r5.json](references/dragonfang-objective-ledger-r5.json) as the immutable
  legacy attachment/relief objective and use
  [dragonfang-objective-ledger-r6-intrinsic.json](references/dragonfang-objective-ledger-r6-intrinsic.json)
  as its explicit assembly-intrinsic successor. Revisions r2/r3 remain historical blade-form evidence and r4
  remains the primitive-assembly baseline; create another hashed successor rather than editing any prior
  revision's meaning in place.

## Session contract

1. Identify the route as `weaponry-threejs-knife-studio@0.1.0`. Do not silently fall back to the Rust
   commercial DCC route or claim its High/Low/UV/Bake gates.
2. Classify every input statement as `observed`, `inferred`, `design-prior`, or `original-choice`.
   Image-visible silhouette may be observed; hidden thickness and back-side structure are not.
3. Load or create an immutable `KnifeObjectiveLedger@1`. A changed objective creates a successor with
   `parent_ledger_sha256`; chat text never overwrites the active ledger.
4. Load or create a canonical `KnifeSceneProgram@1`. Prefer normalized design units and visual ratios;
   do not produce manufacturing measurements.
5. If an img2threejs ObjectSculptSpec exists, preserve it as source evidence and run the closed compatibility
   import before upgrading the blade to independent spine/edge curves and changing sections. Every source
   component and material ID must be either mapped to a bounded Weaponry node or retained by the closed
   compatibility component vocabulary. A non-empty `ignored_component_ids` list is
   `PARTIAL_COMPATIBILITY_IMPORT`: it may support a blade experiment, but it cannot become a full-assembly
   result or a superiority claim. A 7/7 component and 4/4 material import with exact primitive geometry is
   `STRUCTURAL_PARITY`, not visual superiority: Weaponry material layering, candidate enrichment, and quality
   ranking remain separate stages. Never treat upstream primitive geometry as unchangeable truth, and never
   silently drop unsupported components.
6. Treat the compatibility import as an immutable baseline. `candidates_generate` never mutates a
   `source_envelope` or an `img2threejs-compatible-import` program directly; it returns
   `SOURCE_REVIEW_ONLY`. Create an explicit native `KnifeSceneProgram@1` successor before knowledge-driven
   refinement. For a native program, pass a canonical `KnifeObjectiveLedger@1`, one closed set of ten goal
   weights, a seed, and a count of 2–4. The ledger must bind the exact source program, allowed Part IDs,
   frozen Part IDs, evidence and budget. A changed goal requires a successor ledger; omitting the ledger,
   asking for a frozen scope, or providing too few positive mutable scopes is a hard refusal.
   The resulting `KnifeKnowledgeCandidatePlan@1` is immutable: each candidate changes exactly one bounded
   semantic scope, records old/new values and rationale, and remains `REVIEW_ONLY`.
7. Compile through the advertised Weaponry Three.js adapter. If the adapter is unavailable, return
   `IMPLEMENTATION_NOT_AVAILABLE`; do not invent a factory hash, render, or PASS.
8. Before ranking a native candidate, measure every compiled Part in the same fixed eight-view rig with
   depth-resolved `part_indices`. `KnifePartVisibilityMetrics@1` reports visible pixels, occupancy share,
   visible-view count, missing Parts, and raster-underexposed Parts. Also record Part boundary/semantic adjacency,
   guard visible-opening proxy, FPS occupancy/occlusion, and the candidate-to-baseline fixed-view mask delta.
   Reject a candidate whose compiled geometry produces zero visible structural delta. The four-pixel/two-view
   floor and one-pixel delta floor only prevent inert or invisible proposals; they are not quality thresholds.
   Prefer a closed `KnifeObjectiveFunction@2` that covers the ledger's complete objective/regression union, binds the
   evaluator-owned baseline values, and consumes only evaluator-owned candidate receipts. Objective metrics participate
   in direction-aware Pareto improvement; regression metrics are hard no-regression gates. Missing required evidence is
   `NOT_COMPUTABLE`, and a batch with no eligible improvement returns `PARENT_RETAINED`. The legacy
   `best-fixed-view-structural-observability-within-budget@1` ranker remains a review-only fallback, never ledger acceptance.
   Both paths must label the decision `NON_VISUAL_STRUCTURAL_RANKING`, visual status `NOT_REVIEWED`, and quality `NOT_RUN`.
   Use `WeaponryThreeJsKnifeObjectiveMetricAdapter@2` only when a successor ledger
   explicitly names an intrinsic metric. Existing ledgers and Adapter@1 receipts
   retain their historical bytes and meanings; never reinterpret an old
   `NOT_COMPUTABLE` metric as a newly inferred score.
9. Rendering is optional for an explicitly non-visual original/structural objective, but required before any
   visual likeness or visual-quality claim. Render the ledger's fixed views and AOVs when the evidence gate calls
   for them. Treat every image/AOV as a quantitative error input under the frozen camera/calibration. Codex may
   explain likely causes and propose a bounded delta; it cannot manufacture metric values, turn a missing view into
   a fact, or approve itself.
   For a caller-authorized front sheet, `scripts/extract_front_contour.py` can create a deterministic
   `KnifeContourReference@1`; measure it with `scripts/evaluate_metrics.py`, then use
   `scripts/refine_against_reference.py` for a maximum 32-candidate blade-only successor proposal. These
   scripts do not verify the Runtime camera or grant visual approval. For an actual browser capture, run
   `scripts/calibrate_browser_reference.py` once in `baseline` mode against the fixed FRONT Part-ID AOV,
   then evaluate every successor in `replay` mode with the same calibration, rig, camera, resolution, and
   allowed Part-ID set. Candidate refitting is forbidden.
   Before comparing supplier-sheet labels with fixed cameras, validate a closed
   `WeaponryThreeJsReferenceViewMapping@1` using `scripts/check_reference_view_mapping.py`. A sheet label is
   descriptive evidence, not a camera axis: Dragonfang LEFT/RIGHT are long edge-profile aliases of TOP/BOTTOM,
   while Runtime LEFT/RIGHT are longitudinal end views. Aliases may be scored but never increase independent-view
   coverage. Component close-ups and FPS-hold views require their dedicated camera routes.
   Record exactly one review action with `scripts/record_review.py`. Because Weaponry's canonical document is
   `KnifeSceneProgram@1`, synchronize that sidecar ledger with `scripts/sync_review_state.py`; do not inject
   img2threejs-only `sculptPipeline` or `reviewHistory` fields into the closed program.
10. Change only `allowed_scope`. Verify every `frozen_part` is unchanged. Store the candidate, program,
   renderer cohort, metric receipt, and decision as one lineage.
   If canonical number serialization changes, do not rewrite the old program or its receipt. Preserve the old
   byte/semantic identity, emit a `KnifeSceneProgramCanonicalMigration@1` identity-only successor, prove the
   payload is equal with `canonical_sha256` blank, and only then parent a geometry successor from the new policy.
11. Stop on hard-gate failure, budget exhaustion, two plateau revisions, or missing evidence. A stop is a
    useful result, not permission to broaden scope.
12. Before returning `THREEJS_DESIGN_READY`, call the Procedural Draft readiness evaluator with exact candidate
    program bytes, exact baseline program bytes, and exact GLB bytes. Do not pass precomputed compile/delta/GLB receipts
    as authority. Without a replayable baseline, return `BLOCKED` even if the GLB opens.

## Modeling language

For a primary blade, require:

- independent stable spine and cutting-edge curves;
- root, shoulder, belly, and tip sections in monotonic longitudinal order;
- explicit thickness, asymmetry, edge offset, twist, and taper per section;
- separate semantic surfaces for blade body, cutting edge, spine, and root transition;
- stable Part, MaterialZone, attachment, socket, and node identities;
- camera handedness and fixed front/back/top/bottom/side/rear-three-quarter/FPS views.

Use primitives for fasteners, gems, simple grip segments, and helper geometry. Do not reduce the main blade
to a box, flat extrusion, constant ellipse sweep, or global thickness scalar when the program can express a
changing loft.

## Dynamic objective discipline

Every refinement ledger fixes:

- exact program and baseline candidate hashes;
- one `allowed_scope` and explicit `frozen_parts`;
- one falsifiable hypothesis;
- metrics and hard gates to improve;
- bounded parameters and maximum candidate count;
- `minimum_improvement`, plateau limit, and terminal statuses.

Accept a child only when all hard gates remain true, frozen hashes match, and at least one named target metric
improves without violating its regression limits. Otherwise retain the parent. Never substitute a new camera,
lighting rig, crop, or reference to make a candidate appear better.

## Truthful output labels

Keep these states separate:

- `PROGRAM_VALID`
- `THREEJS_ARTIFACT_BUILT`
- `THREEJS_DESIGN_READY`
- `METRICALLY_CONVERGED`
- `METRICALLY_SUPERIOR_TO_PINNED_BASELINE`
- `HUMAN_ACCEPTED`
- `COMMERCIAL_ACCEPTED`

A browser-loadable asset is not automatically high quality. Deterministic convergence is not independent
art direction. Without the corresponding receipt, later labels remain `NOT_RUN` or `NOT_PROVEN`.

The lightweight closure is now explicitly split. Part/Material coverage, boundary,
guard-opening, FPS occupancy, fixed-view structural delta, ObjectiveFunction@2,
and strict program/GLB-byte readiness are executable. Dragonfang R2 still returns
`BLOCKED` because its historical receipt does not contain an exact replayable
baseline program; its other Draft gates pass. The reference-dependent and
continuity metrics remain `NOT_COMPUTABLE`, so ObjectiveFunction@2 currently retains
the parent instead of fabricating an improvement. Automatic intake may recommend,
measure, reject, or retain a proposal; it cannot approve it. Runtime/Store/CAS/MCP
persistence exists for the current design/preview/comparison slice, but it still does not prove visual, human,
engine, or commercial acceptance.

The repeated same-input run now closes the compatibility substep: pinned upstream
and Weaponry consume the same source bytes and produce the same normalized bounds,
1,049 triangles, all 7 source components, and all 4 source material identities.
Weaponry splits the blade into two semantic render parts, so it emits 8 target parts,
but the source-to-target mapping remains complete and no component is ignored. The
receipt classification is `STRUCTURAL_PARITY`; metric and visual superiority remain
`NOT_PROVEN`. The Studio now accepts immutable goal weights and a seed, creates 2–4
truly different native programs with one semantic mutation scope each, compiles them,
and measures depth-resolved visibility for every Part in the frozen eight-view rig.
Its legacy selection label is `NON_VISUAL_STRUCTURAL_RANKING`; the selected candidate remains
`REVIEW_ONLY / NOT_REVIEWED / NOT_RUN`. The stricter ObjectiveFunction@2 path currently
returns `PARENT_RETAINED`. The next bounded substep is an exact R2 baseline program for
readiness replay, not more compatibility work and not a quality label.

## Handoff

Return the active objective revision, allowed/frozen scope, source classification, program/candidate/factory/
render/metric hashes, renderer cohort, metrics before and after, rejected alternatives, stop reason, and one
evidence-backed next action. Do not return invented asset hashes or claim a stage that was not executed.
