# Knife design language and prior space

This file is the progressive-disclosure knowledge entry for the lightweight
`weaponry-threejs-knife-studio` route. The machine-readable shape is
[`knife-knowledge.schema.json`](knife-knowledge.schema.json). It turns common
knife-art practice into bounded, inspectable priors for a Codex authoring loop;
it is not a visual oracle.

## Scope and units

This route creates nonfunctional game, film, or display assets. All values below
are normalized visual values or ratios. They must not be converted into real
dimensions, manufacturing tolerances, material recipes, handling advice, or
performance claims.

Use a local blade frame:

- `u ∈ [0, 1]` runs from blade root to tip (`u=0` root, `u=1` tip).
- `v` is signed lateral position in the chosen silhouette plane; `d` is signed
  depth/thickness position. They are visual coordinates, not world units.
- `L_B = 1` is the projected blade root-to-tip span used only as a ratio
  denominator. Every attachment length is expressed as `L_part / L_B`.
- `s(u)` and `e(u)` are the spine and cutting-edge lateral curves. The envelope
  width and center are `w(u) = s(u) - e(u)` and
  `c(u) = (s(u) + e(u)) / 2`. A renderable section requires `w(u) > 0` and
  `t(u) > 0`, where `t` is the normalized section thickness.

The scene-program coordinate convention remains the one in
`KnifeSceneProgram@1`; this local frame only makes the design reasoning
explicit. A caller must record any frame mapping as an authored choice.

## Claim classes and uncertainty

Every statement consumed by a program or ledger carries exactly one class:

| Class | Evidence rule | Permitted use | Forbidden shortcut |
| --- | --- | --- | --- |
| `observed` | Directly visible in an authorized view or present in an authorized structured source; include the view/source reference. | Lock a visible contour, region, landmark, or named component. | Do not extend it to a hidden side, thickness, or unseen attachment. |
| `inferred` | A hypothesis required to make multiple observations consistent; list its supporting observations and keep it revisable. | Choose a provisional centerline, depth sign, or occluded continuation. | Do not report it as reference fact. |
| `design-prior` | Family knowledge or a normalized plausibility rule, independent of this asset's evidence. | Rank or initialize candidates; use soft checks unless the brief explicitly freezes it. | Do not use it to overwrite an observation or prove likeness. |
| `original-choice` | A deliberate choice for an original/control knife, with no claim of reference attribution. | Fill missing detail, choose a material separation, or define a stylistic variant. | Do not label it `observed` merely because it renders well. |
| `unknown` | Evidence is absent, cropped, occluded, or contradictory. | Preserve the gap and stop a dependent claim. | Do not silently replace it with a family default. |

`confidence` is ordinal (`high`, `medium`, `low`), not a probability. A
`design-prior` can have high confidence as a reusable rule while still having
zero authority over an authorized reference. Contradictory source claims remain
contradictory until an authorized successor brief resolves them.

## Local knowledge authority

The checked-in `crossfire-knife-knowledge.json` is the only local weapon
knowledge space used to seed this route. It is a bounded prior registry, not a
reference archive: its `design-prior` records may rank or initialize a
candidate, but they cannot turn an unseen feature into an observation. Web
search, generated text, and a family label are external context only. They must
not be copied into `source_refs`, a canonical program, or a goal ledger as
evidence. A single crop cannot establish the hidden side, thickness, attachment
or topology; preserve that gap as `unknown`, or record a revisable `inferred`
hypothesis with its supporting observations.

## Discrete grammar

The authoring surface is a finite grammar, not free-form shape completion. The
closed productions are:

```text
Program       ::= IndependentCurvePair Sections Assembly
CurvePair     ::= SpineCurve CuttingEdgeCurve
Sections      ::= RootSection ShoulderSection BellySection TipSection IntermediateSection*
CurveBasis    ::= Bezier | NurbsLike
Assembly      ::= Guard? Grip? Pommel? Fastener* Gem* Relief*
Guard         ::= ClassicGuard | DragonGuard
Grip          ::= ClassicGrip | SegmentedGrip
Pommel        ::= ClassicPommel | HookedPommel
Mutation      ::= one bounded path in ledger.allowed_scope
```

`CurveBasis` is `bezier` or `nurbs-like`; section roles are the ordered
`root`, `shoulder`, `belly`, `tip` sequence plus bounded `intermediate`
stations; each assembly union and its fields are closed by
`knife-scene-program.schema.json`. Candidate search may choose only finite
parameter paths declared by the ledger, with a fixed seed and maximum budget.
There is no grammar production for arbitrary code, URL, network asset,
unknown-side geometry, or an unbounded mesh operator.

## Silhouette grammar

The silhouette is the primary signal. Start with the two independent curves
`s(u)` and `e(u)`, then loft sections between them. Do not collapse the blade to
a box, flat extrusion, constant ellipse sweep, or one global thickness value
when the evidence or the selected family requires variation.

All rows in this table are `design-prior`, not observations:

| Grammar family | Curve behavior to initialize | Section rhythm | Typical readable cue |
| --- | --- | --- | --- |
| `straight-tanto` | Nearly straight spine; edge has one controlled shoulder break. | Root → shoulder transition is deliberate; belly is shallow; tip is a short wedge. | A clear shoulder and a planar-looking terminal direction. |
| `drop-point` | Spine descends gently toward the tip; edge carries the main belly. | Belly width peaks before tip; distal thickness tapers continuously. | A calm spine against a fuller edge. |
| `clip-point` | Spine is mostly stable, then uses one clipped terminal segment. | Tip section preserves a distinct clip transition; no high-frequency zig-zag. | A legible clipped line, not a noisy saw edge. |
| `kukri` | Spine and edge are both curved; edge belly is intentionally forward-weighted. | Shoulder is compact, belly is broad, and tip converges after the belly. | A strong single sweep with one dominant belly. |
| `machete` | Long low-frequency sweep; edge curvature is broad rather than ornamental. | Width changes slowly; tip can be broad but remains renderable. | Readability at distance through one large contour rhythm. |
| `karambit` | Edge may recurved; handle/pommel may form a visual ring. | Tip and ring are separate semantic parts; avoid a degenerate junction. | A return curve and circular handle cue. |
| `bayonet` | Predominantly axial spine and edge with a restrained point. | Guard/root transition carries more contrast than the tip. | A straight, compact silhouette with a distinct root assembly. |
| `original-knife` | Select only the invariants needed by the brief. | Declare the chosen rhythm as `original-choice`. | A coherent authored grammar, not an accidental hybrid. |

The reusable silhouette invariants are:

1. Longitudinal order is explicit: `root < shoulder < belly < tip` in `u`.
2. `w(u)` remains positive through all renderable stations. A near-zero tip is
   allowed only when the final bevel/section still produces faces and a visible
   highlight.
3. Curvature changes are sparse and intentional. A family may have one dominant
   inflection or return, but arbitrary alternating micro-waves are not a useful
   default prior.
4. The spine and edge have separate IDs, control points, and review roles.
   Their apparent intersection is a candidate failure unless the tip grammar
   explicitly describes a convergent terminal.
5. A silhouette change is judged in fixed views first. Color, lighting, or
   relief cannot compensate for an unresolved boundary.

## Spine and edge curve families

Use a bounded curve descriptor with a named role, continuity, and extrema. A
curve family describes how to start a design; it does not reconstruct hidden
geometry. The following formula vocabulary is sufficient for a first pass:

- `monotone-u`: sampled longitudinal positions are strictly increasing; no
  control point reverses the root-to-tip order.
- `C0`: neighboring segments meet. Use at least `G1` (matching tangent
  direction) at root/shoulder/belly joins when a highlight should flow through.
- `convex-belly`: one signed curvature lobe dominates the belly interval; the
  lobe's maximum is a declared station, not a guessed hidden feature.
- `recurve-edge`: the edge has one bounded sign change in curvature before the
  tip; the sign change must be represented by an intermediate section.
- `return-tip`: the terminal tangent turns toward the spine within a bounded
  tip interval while preserving positive width and thickness.
- `fuller-run`: a secondary longitudinal curve follows the body and terminates
  before the tip or at a declared terminal; it is a surface role, not a second
  blade boundary.

Concrete starter families keep spine and edge edits independently addressable:

| Curve family ID | Role | Behavior | Bounded use |
| --- | --- | --- | --- |
| `spine-monotone-sweep` | `spine` | `monotone-u` + `G1` | One broad sweep with no reversal in longitudinal order. |
| `spine-drop` | `spine` | `convex-belly` + `G1` | A single gentle descent toward the tip. |
| `edge-straight` | `cutting-edge` | `straight` + `G1` | A restrained edge for tanto/bayonet-like drafts. |
| `edge-convex-belly` | `cutting-edge` | `convex-belly` + `G1` | One dominant belly lobe; station is explicit. |
| `edge-recurve` | `cutting-edge` | `recurve-edge` + `C1` where visible | One bounded return before the tip; add an intermediate section. |
| `edge-return-tip` | `cutting-edge` | `return-tip` + `G1` | A terminal turn that still leaves renderable width/thickness. |
| `fuller-follow` | `fuller` | `fuller-run` + `C0` | A surface accent that never silently becomes a silhouette edge. |

These IDs name edit targets, not hidden features. A view that does not support a
fuller, recurved edge, or return tip should keep that role `unknown` instead of
inventing it from a family label.

Useful normalized checks are:

```text
u_0 < u_1 < ... < u_n
w(u_i) = s(u_i) - e(u_i) > 0
|Δ tangent(root→shoulder)|, |Δ tangent(shoulder→belly)|,
|Δ tangent(belly→tip)| are bounded by the selected family profile
```

The tangent limits are profile parameters and should be stored with the
knowledge object; there is no universal number. If a reference visibly breaks
continuity, classify that break as `observed` and preserve it as a named feature
rather than smoothing it away with a prior.

## Root, shoulder, belly, and tip sections

`KnifeSceneProgram@1` already requires at least these four roles. The knowledge
layer adds role-specific reasoning and ratio checks:

| Section | Normalized constraint | What to inspect | Common failure |
| --- | --- | --- | --- |
| `root` | `u=0`; `w_root>0`; `t_root>0`; attach to a declared ricasso/tang or blade-body transition. | Does the first section explain where the blade meets the guard? | A floating blade or a root that is only an arbitrary box face. |
| `shoulder` | `0<u_shoulder<u_belly`; width/thickness change is continuous; any shoulder break has a declared curve family. | Does the transition establish the blade's character without a hard accidental kink? | A global taper that erases the shoulder, or an unbounded step. |
| `belly` | `u_shoulder<u_belly<1`; `w_belly` is a declared local maximum or inflection; edge and spine remain independently readable. | Is there one dominant belly rhythm and a believable path to the tip? | Repeating bulges, a belly hidden by material, or a constant ellipse. |
| `tip` | `u=1`; `0<w_tip≤w_belly`; `0<t_tip≤t_root`; terminal bevel remains renderable. | Does the tip converge in the intended direction without degenerate faces? | Zero-area faces, a disconnected point, or a tip that is only a camera artifact. |

Recommended invariant set:

```text
0 = u_root < u_shoulder < u_belly < u_tip = 1
w(u) > 0 and t(u) > 0 at every sampled station
0 < w_tip / w_belly ≤ 1
0 < t_tip / t_root ≤ 1
|asymmetry| ≤ 1 and |twist| ≤ the profile's declared bound
```

These inequalities are geometry-sanity checks. The direction of taper, the
location of the belly maximum, and the amount of asymmetry remain family- or
brief-dependent `design-prior` choices. They must not be used to infer a hidden
back face from one front crop.

A section profile can be selected from `diamond`, `lenticular`, `wedge`,
`faceted`, or `custom`. For `custom`, record the profile polygon and its
classification; do not substitute a generic ellipse without a reason.

## Guard, grip, pommel, and attachment ratios

Treat attachment as a semantic continuity problem, not a real joint or
manufacturing interface. Define a local root anchor and bind each child to a
stable parent Part. Let `L_G` be grip span, `D_G` its normalized visual width,
`S_guard` guard span, and `L_P` pommel span.

The following broad starter ranges are useful priors for a first visual draft;
they are deliberately wide and are not acceptance thresholds:

| Ratio or invariant | Starter prior | Reasoning |
| --- | --- | --- |
| `L_G / L_B` | `0.25–1.10` | Keeps the handle visually subordinate or co-dominant without fixing a knife size. |
| `S_guard / D_G` | `0.8–3.0` | Makes the guard legible while allowing compact or sweeping families. |
| `L_P / L_G` | `0.05–0.40` | Prevents a pommel from swallowing the grip in the first pass. |
| `T_guard / t_root` | `0.25–2.0` | Maintains an attachment cue without assigning a physical thickness. |
| `gap(root, guard) / L_B` | `0–0.02` | A visible seam may exist, but a floating gap is usually a continuity error. |
| `child_anchor_error / L_B` | `0` in a locked draft; otherwise explicit `inferred` | Parent/child identity should not drift during a blade-only correction. |

Use the ratios as a starting vector, then test in fixed front, side, rear
three-quarter, and FPS views. The attachment check is:

```text
anchor_error = ||P_child(anchor) - P_parent(anchor)|| / L_B
continuity_error = max(anchor_error, tangent_mismatch, overlap_or_gap)
```

`continuity_error` is a review signal, not proof that the hidden assembly is
correct. Guard, grip, pommel, fasteners, gems, and reliefs each need a semantic
Part and a source class. A primitive is adequate only when the visible evidence
does not require a more expressive form.

## FPS occupancy and readability

The FPS view is a nonfunctional presentation profile. It must be fixed before a
comparison; it is not a claim about gameplay, handling, ergonomics, or real use.
For a normalized viewport `F=[0,1]²` and rendered silhouette mask `M`, compute:

```text
bbox_w = x_max(M) - x_min(M)
bbox_h = y_max(M) - y_min(M)
bbox_occupancy = bbox_w * bbox_h
mask_occupancy = area(M) / area(F)
centroid_error = ||centroid(M) - target_centroid||
tip_margin = min distance from tip(M) to the frame boundary
visible_part_fraction(p) = visible pixels owned by p / all silhouette pixels
```

`bbox_occupancy`, `mask_occupancy`, centroid, tip margin, guard visibility, and
grip visibility should be recorded separately. Do not average a cropped tip or
hidden grip into a passing total. A broad initial ranking prior may use
`bbox_h ∈ [0.45,0.95]`, `bbox_w ∈ [0.15,0.90]`, and `tip_margin ≥ 0.03`;
these are soft defaults only. The brief, camera identity, crop, and ledger
override them.

In FPS inspect views, prioritize this order:

1. blade silhouette and tip margin;
2. guard/blade and grip/guard negative space;
3. stable Part-ID ownership of blade, guard, grip, and pommel;
4. material-zone separation under the fixed light rig.

If a view is cropped or an ownership mask is unresolved, record
`unknown`/`NOT_RUN`; do not enlarge the crop or move the camera to hide the
uncertainty.

## Material zones and visual readability

Material zones are semantic visual regions, not color labels or material-making
instructions. A zone may carry normalized renderer inputs such as base tone,
metalness, roughness, normal strength, clearcoat, emissive, and wear mask. Each
input needs provenance and a source class.

The lightweight Three.js route exposes the closed
[`KnifeLayeredMaterialSpec@1`](knife-layered-material.schema.json) vocabulary:
`red-lacquer-metal`, `antique-gold`, `black-wrapped-grip`, and `ruby-emissive`.
Its only procedural controls are bounded curvature, edge-wear, engraving-mask,
and U/V scale-repeat values. The compiler maps them to built-in Three.js PBR
properties and named geometry attributes; it accepts no network texture, custom
shader, or executable material source.

These layers are a bounded readability vocabulary, not a geometry correction
channel. Curvature, edge wear, engraving, and scale-repeat attributes may expose
authored surface cues, but they cannot repair a wrong silhouette, section
profile, topology, normal, UV, attachment, or Part-ID boundary. If a geometry
gate fails, keep the material spec unchanged and revise the permitted geometry
scope; do not tune a material, crop, light, or texture to conceal the failure.

For neighboring zones `i` and `j`, let `q_i,k ∈ [0,1]` be normalized channel
projections and `α_k≥0` with `Σα_k=1`. A simple separation prior is:

```text
R(i,j) = Σ_k α_k * |q_i,k - q_j,k|
```

Record `R` as an auxiliary readability signal. It is not a universal threshold,
and it cannot override a boundary, normal, UV, or ownership failure. Prefer at
least two independent cues for a critical boundary (for example, geometry plus
roughness or Part-ID plus base tone). Never use a color or roughness change to
hide a silhouette error.

Minimum semantic zone vocabulary for a knife draft:

- `blade-body`, `cutting-edge`, `spine`, and `root-transition`;
- `guard`, `grip`, and `pommel`;
- optional `fuller`, `relief`, `fastener`, `gem`, or `accent` only when observed,
  inferred, or explicitly chosen.

Each zone declares whether it is `observed`, `inferred`, `design-prior`, or
`original-choice`. An absent texture, relief, or hidden layer remains unknown;
the compiler must not synthesize a provenance claim.

## Codex execution pattern

Use this knowledge space as a bounded prior in the following order:

1. Create a `KnifeKnowledge@1` object and classify every claim.
2. Freeze the authorized reference, camera identity, and normalized frame.
3. Author independent spine/edge curves, then the four ordered sections.
4. Check section positivity/continuity before adding guard, grip, or pommel.
5. Bind attachments through stable semantic anchors and ratio checks.
6. Render fixed views and compute silhouette, boundary, section, ownership,
   readability, and FPS occupancy signals.
7. Change only one `allowed_scope`; preserve `frozen_parts`, program hash, and
   evidence lineage. Retain the parent when a hard gate or regression limit
   fails.

The formulas are useful because they make a plausible design reproducible and
auditable. They do not solve the single-image-to-3D underdetermination: an
inferred hidden side is still unknown, and a lower metric is not automatically a
better artistic result.

## Explicit non-claims

This knowledge space does not establish:

- hidden geometry, backside topology, manufacturing dimensions, machining,
  structural performance, or real-world operation;
- reference likeness from a formula, a browser-loadable GLB, or a single render;
- PBR correctness, editable Low/High/Cage/Bake delivery, engine readiness, or
  commercial acceptance;
- human art direction, partner approval, or release permission.

Those claims require their own evidence and gates. A deterministic compiler or
metric evaluator may return `PROGRAM_VALID`, `THREEJS_ARTIFACT_BUILT`, or
`MEASURED_NOT_APPROVED`; it must not promote these priors to
`HUMAN_ACCEPTED` or `COMMERCIAL_ACCEPTED`.
