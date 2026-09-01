# Objective ledger and deterministic quality

## Deterministic front-contour tools

- `extract_front_contour.py` performs fixed-threshold, fixed-crop foreground extraction on an authorized
  image and emits normalized contour/landmark data. The crop must be explicitly chosen; extraction is not
  a claim that the image camera is calibrated.
- `evaluate_metrics.py` computes silhouette IoU, Boundary F1, symmetric Chamfer, P95 contour distance and
  four semantic landmark errors. A computed result is `MEASURED_NOT_APPROVED`.
- `refine_against_reference.py` evaluates at most 32 deterministic blade-only candidates, preserves frozen
  assembly, and can emit only `SUCCESSOR_PROPOSED_NOT_APPROVED` or parent-retained.

An exact synthetic self-comparison is only a software check. It is never reference-quality evidence.

## Optional reference and visual-evidence boundary

The route supports original designs and reference-inspired designs. An authorized
visual reference is optional for authoring and is not a 1:1 reconstruction target.
Without one, a program may still satisfy structural gates and produce a Three.js
artifact from bounded knowledge, discrete grammar, and intrinsic mathematical
metrics. Reference- or image-dependent terms are `NOT_RUN` when not in scope and
`NOT_COMPUTABLE` when required evidence is absent; neither state is a zero score.

“Disable all visual validation” is therefore a valid scope choice only when the
declared objective is explicitly structural/non-visual. It cannot be used to emit
a visual likeness/quality, visual convergence, or visual superiority claim.
Intrinsic metrics may still support a structural convergence/superiority result,
but that result must not be presented as visual evidence. Human and commercial
acceptance remain separate gates.

## Quantitative objective contract

The route does not select a candidate by taste or by an unbound “looks better”
instruction. Let `b` be the frozen baseline and `c` a candidate. For every
named metric `m_i`, define the direction-aware improvement:

```text
Δ_i(c,b) = m_i(c) - m_i(b)       when m_i is maximized
           m_i(b) - m_i(c)       when m_i is minimized

J(c|b) = [ hard_gate(c), regression_ok(c,b), Δ_1, ..., Δ_n ]
```

`J` is maximized subject to all hard gates, frozen-part/camera bindings, and
the ledger's regression limits. Candidate comparison is direction-aware
Pareto comparison: a candidate must be no worse in every computable objective
term and strictly better in at least one, with `minimum_improvement` applied
before a successor can be proposed. An optional weighted scalar may be added
only when the ledger explicitly declares its weights; the Skill never invents
weights from a screenshot.

If a required reference, view, calibration, or metric is absent, the term is
`NOT_COMPUTABLE`, not zero and not an inferred score. A failed hard gate is an
infinite penalty and cannot be averaged away. This keeps a single-image crop
from silently becoming a full-object objective.

The executable form is `KnifeObjectiveFunction@2`. It must cover the complete
union of `KnifeObjectiveLedger@1.objective_metrics` and `regression_limits`:

- `objective` metrics participate in minimum-improvement and Pareto comparison;
- `regression` metrics only prevent a prohibited regression and never become a
  hidden second objective;
- baseline values must exactly equal the Studio's evaluator-owned baseline
  receipt;
- candidate values come from evaluator-owned fixed-rig receipts, never caller
  numbers;
- no eligible candidate yields `PARENT_RETAINED`; missing required evidence
  yields `NOT_COMPUTABLE`.

Metric identity is append-only. The first twelve IDs keep their historical
fixed-view/reference meanings. Renderer-free measurements use new IDs and do
not reinterpret an old missing-evidence result:

- `KnifeIntrinsicMorphology@1` derives section profile continuity, curve G1
  proxy, tip taper and curve-extrema headroom directly from the typed blade;
- `KnifeAssemblyIntrinsicMetrics@1` derives ratio priors, AABB attachment
  continuity, MaterialZone readability proxies and bounded complexity from the
  program plus compiled scene;
- `WeaponryThreeJsKnifeObjectiveMetricAdapter@2` combines those receipts with
  the unchanged Adapter@1 raster receipt only when the active successor ledger
  names an intrinsic ID.

These values support deterministic structural ranking. They keep
`visual_quality_status=NOT_COMPUTABLE`, `quality_status=NOT_RUN`, and never
constitute artistic or commercial approval.

## Candidate search boundary

`scripts/search_candidates.py` is the bounded search implementation for this
contract. It uses a fixed explicit seed, a ledger budget of at most 32
candidates, and only the ledger's `allowed_scope`; frozen Part hashes and the
baseline program/ledger hashes remain bound. It evaluates intrinsic geometry
signals without rendering, forms a direction-aware Pareto set, and emits only
`REVIEW_ONLY` successor proposals. `evaluate_metrics.py` and the browser
capture route may supply quantitative image/AOV errors later, but neither
script changes the objective, refits the camera, or grants acceptance.

## Visual comparison policy

When supplied, images and AOVs are measurement inputs for masks, boundaries,
landmarks, continuity, ownership, and occupancy errors. A visual comparison can
explain which measured term failed; it cannot choose an artistic winner, replace
a missing view, override `NOT_COMPUTABLE`, or turn a metric result into human or
commercial approval. Human art direction is an explicitly separate gate.

## Successor rule

An objective change creates a new ledger with a parent hash. It must state why the prior objective is obsolete,
which evidence caused the change, and which fields remain frozen. A ledger without baseline/program hashes or
stop conditions is not executable.

## Required evaluation order

1. Geometry hard gates.
2. Camera and framing identity.
3. Per-view silhouette, boundary and landmarks.
4. Section/thickness and surface continuity.
5. Attachment and negative-space.
6. Material regions and FPS presentation.

Do not average a failed critical feature into a passing total. Use named metrics and regression limits. Useful
metrics include silhouette IoU, Boundary F1, symmetric Chamfer distance, P95 contour distance, tip/belly/root
landmark error, thickness continuity, normal/curvature discontinuity, Part/Material-ID coverage, and viewport
occupancy.

## Stop policy

Stop and retain the parent when:

- a hard gate fails;
- a frozen Part or camera hash changes;
- candidate budget is exhausted;
- improvement is below the ledger threshold for two successive revisions;
- the requested evidence does not exist;
- the compiler or renderer cohort drifts.

Only a same-input baseline comparison can claim metric superiority. Human and commercial acceptance remain
separate optional gates.

## Procedural Draft readiness

`THREEJS_DESIGN_READY` is a lower product tier, not a quality score. The readiness
evaluator accepts exact candidate program bytes, exact baseline program bytes and
exact GLB bytes. It internally decodes and compiles both programs, replays the fixed
eight-view masks, measures structural delta, checks budgets, hashes the GLB and reads
back its v2 JSON scene, Part nodes, materials and triangle count. A caller-supplied
compile receipt, metric receipt, delta or GLB hash is not authority. If the baseline
cannot be replayed, return `BLOCKED` even when all other gates and the GLB readback pass.
