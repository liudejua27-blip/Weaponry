# Action space and gate map

This ten-day profile is knife-only. General weapon examples and subject-specific legacy
tools are compatibility fixtures unless the live knife profile explicitly advertises them.

## Typed command layers

| Layer | Initial vocabulary | Truth/evidence requirement |
| --- | --- | --- |
| Selection | stable IDs, blade/edge/spine/tip/guard/grip/pommel/MaterialZone, adjacency, boundary, normal/angle, visible region | query hash and resolved ID set |
| Mesh edit | move, split, extrude, inset, bevel, bridge, dissolve, merge, loop-cut/slide | atomic journal, topology validation, before/after hash |
| Modifier | transform, mirror, array, boolean, bevel, solidify, subdivision, weighted normal | ordered graph, provider version, evaluated hash |
| Surface | seams, islands, unwrap, pack, cage, normal/AO/curvature/thickness/ID bake, PBR zones | high/low/UV/cage/bake lineage and channel readback |
| Review | fixed cameras and nine AOVs | candidate-bound images, metrics and typed critic |
| Delivery | GLB/FBX interchange, first-person knife clips, sockets and engine profile | exported hash, validator, restart/readback receipt |

Availability must come from the live Runtime catalog.  This map is a target and must
not be used to fabricate an unsupported tool call.

## Transaction rules

- Batch operations only when later operations depend on earlier topology or all
  operations implement one reviewable local intent.
- Resolve selection once per declared command unless the command explicitly uses a
  deterministic post-command query.
- Stable IDs cannot be inferred from vector position after topology changes.
- On any invalid ID, non-finite coordinate, non-manifold policy violation or budget
  overflow, reject the full transaction.
- Persist the journal only through Runtime prepare/confirm semantics.

## Authorization manifest minimum

An authorized commercial asset record requires project/partner assertion, asset
identifier, source content hash, ownership or permitted-use statement, allowed
transformations, allowed delivery target, expiry/revocation state and provenance.
Missing fields block consumption of the asset; they do not block original-control
assets.

## Acceptance order

`authorization → topology/readback → silhouette/proportion → secondary form → high/low/UV/bake → PBR → camera/AOV → engine interchange → human review → approval`

Later evidence cannot compensate for an earlier failed gate.
