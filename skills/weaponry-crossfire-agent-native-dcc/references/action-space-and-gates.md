# Knife action space and gate map

This is the compact operating reference for the closed `weaponry-knife-p0-default@1`
profile. It does not replace the live Runtime catalog, request schemas, or quality
contracts. General weapon examples and subject-specific legacy tools remain
compatibility fixtures unless the live knife profile explicitly advertises them.

## 11-façade ownership

| Order | Façade | Use it for | Must come back before continuing |
| ---: | --- | --- | --- |
| 1 | `weapon_preflight` | Ponytail preflight, capability/profile and authorization state | profile/capability hashes and scope decision |
| 2 | `reference_intake` | Project and authorized reference admission | reference/source hash, coverage and unknown regions |
| 3 | `observe` | Scene, candidate, stable IDs, stage and lineage inspection | current head and exact parent hashes |
| 4 | `authoring_transaction` | Curves, selection, multi-operation AuthoringMesh journal and ModifierGraph | before/after topology, changed IDs, graph/evaluated hashes |
| 5 | `surface_pipeline` | High, editable Low, correspondence, Hero UV, cage, bake and MaterialZones | High/Low/UV/cage/map/material readback and lineage |
| 6 | `fps_presentation` | First-person cameras, sockets and nonfunctional clips | camera/socket/clip hashes and visibility diagnostics |
| 7 | `quality_review` | Pre-delivery fixed AOVs, reference comparison and typed critic; final human review only after engine validation | candidate-bound evidence and separate pre-engine/final decisions |
| 8 | `delivery` | Target-engine preparation and interchange validation | exported artifact hash and engine receipt |
| 9 | `approval` | Return through `quality_review` for export-bound independent human review, then request confirm/export | engine, human and explicit user decisions bound to one candidate/export |
| 10 | `recovery` | Reject, checkpoint, restore and deterministic replay | retained parent head and recovery receipt |
| 11 | `job` | Bounded job status/cancel and cohort evidence | terminal status, limits and receipt |

`job` and `recovery` are controls around the route, not permission to reorder it.
Use `observe` after every write and before every downstream handoff.

## Brief normalization and conflict freeze

Before creating geometry, normalize the brief into a typed table with:

`field → candidate values → source reference/hash → authority → status`

At minimum capture triangle/LOD budgets, texture resolution and channels, target
engine/version, coordinate/unit convention, required reference views, actions/clips,
socket/hand requirements, material zones, and human/engine acceptance gates. When
two supplied artifacts disagree, preserve both values and mark the field
`CONFLICT_PENDING`; ask the user to freeze one value or explicitly accept a bounded
range. An image, screenshot, filename, or prose claim never silently wins.

The initial Brief is durably stored through the `reference_intake` façade and may
remain blocked. A resolution is a new Brief with the exact parent ID/hash and the
`immutable-successor-preserve-source-claims@1` policy. Runtime must reject a missing
parent, self-parent, parent hash drift, dropped source claim, or changed claim value.
Only the successor may carry the selected resolution; the parent stays replayable.

An approver portrait, contact detail, handwritten signature, logo, or other identity
cue is not a human review receipt. Keep it out of the repository and require a typed,
candidate-bound human decision through the Runtime review path.

## Typed command layers

| Layer | Initial vocabulary | Truth/evidence requirement |
| --- | --- | --- |
| Selection | stable IDs; blade/edge/spine/tip/guard/grip/pommel/MaterialZone; adjacency, boundary, normal/angle, visible region | query hash and resolved ID set |
| Mesh edit | move, split, extrude, inset, bevel, bridge, dissolve, merge, loop-cut/slide | atomic journal, topology validation, before/after hash |
| Curve/form | blade spine/edge/profile curve, bounded sweep/loft/tessellation | closed schema, finite parameters, deterministic evaluated hash |
| Modifier | transform, mirror, array, boolean, bevel, solidify, subdivision, weighted normal | ordered graph, provider/version, input/output hashes |
| Surface | seams, islands, unwrap, pack, cage, normal/AO/curvature/thickness/ID bake, PBR zones | High/Low/UV/cage/bake lineage and channel readback |
| Review | fixed cameras and nine AOVs | candidate-bound images, metrics and typed critic |
| Delivery | GLB/FBX interchange, first-person knife clips, sockets and engine profile | exported hash, validator and restart/readback receipt |

Availability comes from the live Runtime catalog. This map must not be used to
fabricate an unsupported tool call.

## Transaction rules

- Batch operations only when later operations depend on earlier topology or all
  operations implement one reviewable local intent. Separate speculative alternatives
  into candidate branches.
- Resolve selection once per declared command unless the command explicitly uses a
  deterministic post-command query.
- Stable IDs cannot be inferred from vector position after topology changes.
- On invalid IDs, non-finite coordinates, a manifold-policy violation, or budget
  overflow, reject the full transaction with no partial write.
- Persist a journal only through Runtime prepare/confirm semantics. A preview is not
  a candidate, version, approval, or export.

## Stage gates

`authorization → reference coverage → topology/readback → silhouette/proportion → secondary form → High/Low correspondence → UV/cage/bake → PBR/material → FPS/AOV → engine interchange → human review → user approval`

Later evidence cannot compensate for an earlier failed gate. Keep `PASS`, `FAIL`,
`BLOCKED`, and `NOT_RUN` distinct. A single three-quarter reference supports at most
`PARTIAL_VISIBLE_VIEW_PASS`; `HQ_360_PASS` stays blocked until the required identity
views are hash-bound.

The checked-in knife profile is `development-only`. Source, contract, Skill, Curve,
or façade tests therefore keep `commercial=NOT_PROVEN`; only a promoted live profile
plus candidate-bound surface, engine, independent-human, and user-approval receipts
can change that label.
