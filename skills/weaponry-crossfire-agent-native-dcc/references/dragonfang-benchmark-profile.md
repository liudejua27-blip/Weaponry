# Dragonfang kukri benchmark profile

This is a replaceable benchmark profile for the user-supplied Dragonfang kukri
brief. It is deliberately metadata-only: no reference image, thumbnail, embedded
bytes, contact detail, identity cue, handwritten signature, logo, or other source
content belongs in this Skill or repository. At execution time the Runtime must
admit an authorized reference package and bind its content hash.

## Profile contract

```yaml
profile_id: dragonfang-kukri-benchmark@1
kind: optional-benchmark
source_artifact: runtime-admitted-reference-package
embedded_source: false
authorization: required-at-runtime
commercial_claim: prohibited-until-human-and-engine-gates
replacement_policy: any-authorized-knife-or-original-control
```

The profile is not a generic requirement and does not select values for a conflicting
brief. It is a named fixture for comparing the same workflow against another knife.
The profile may be replaced by an original control knife without changing the
orchestration or the 11-façade surface.

For the current authorized 1536x1024 Dragonfang contact sheet, read
[dragonfang-multiview-inventory.md](dragonfang-multiview-inventory.md) before the first
multi-part High review. That reference is observation-only: it supplies fixed view roles,
detail-to-Part intent, and an explicit observed/inferred/unknown boundary; Runtime must still
bind the authorized ReferenceEvidence and derive its own hashes.

## Normalized brief fields

Record these fields in the Runtime brief before any transaction. The checked-in
Dragonfang fixture is an initial, blocked intake: its parent fields are null and its
freeze policy is `initial-intake-no-parent@1`. It must be bound to an existing
Runtime `ReferenceEvidence` triple (`reference_id`, reference object hash, evidence
hash) before persistence.

| Field | Benchmark cue | Required state |
| --- | --- | --- |
| identity | curved kukri-like blade with a pronounced spine/edge silhouette | user-authorized reference hash |
| form | blade, guard, grip, pommel and decorative secondary forms | semantic Part/MaterialZone map |
| surface | metal blade, guard/accent metal, grip/coating and controlled wear | PBR channel/provenance readback |
| views | front, back, left, right, rear-three-quarter, detail and FPS-hold views | hash-bound coverage matrix |
| presentation | inspect/hold and bounded nonfunctional action clips | camera/socket/clip receipt |
| delivery | target engine, coordinate system, texture/LOD/triangle budgets | explicit user freeze; `UNRESOLVED` until then |
| acceptance | silhouette, edge flow, bake, material, FPS, engine and independent review | separate gate receipts |

## Art-direction inventory

Keep these cues as replaceable benchmark semantics rather than copied source art:

- retain a recognizably forward-heavy kukri silhouette before ornamental detail;
- separate the broad blade belly, cutting edge, blade body, shallow relief, guard,
  grip, fasteners/gems, and pommel into stable semantic Parts;
- use an ancient-gold dragon-inspired guard and shallow blade relief as the identity
  zone, while the dark-red blade body and black grip remain visually dominant;
- reserve silver for the cutting edge and restrained ruby/emissive treatment for a
  small focal gem; emissive must not read as an LED or neon strip;
- use controlled edge wear, sharpening marks, cavity darkening, and grip-contact wear;
  do not use corrosion, blood, excessive dirt, cartoon anatomy, or silhouette-breaking
  fantasy spikes to disguise weak geometry;
- FPS framing should keep the grip low-right, the blade toward upper-left, the center
  reticle area readable, and the guard/relief visible during inspect.

The benchmark's approximate visual balance is dark red > antique gold > black >
silver edge > ruby accent. This is an art-direction ratio, not a pixel or material
coverage pass. Geometry and material-ID AOV evidence decide whether the actual asset
respects it.

The visual cues are modeling prompts only. They do not authorize copying any mark,
ornament, texture, or identifiable source detail. If the brief, reference sheet, or
user message supplies competing triangle, texture, engine, action, or view values,
write every value with its source/hash and set the field to `CONFLICT_PENDING`.
Continue only after the user freezes a value or accepts a typed range.

## Execution use

Use the profile to seed inspection and benchmark labels, never to bypass gates:

1. Admit the authorized reference package and record the profile ID plus source hash.
2. Persist the initial Brief through `weaponry_knife_production_brief_prepare`, read
   it back through `weaponry_knife_production_brief_get`, and retain its blocked
   conflict ledger. User resolutions create an immutable successor carrying the
   parent Brief ID/hash and every original claim; they never mutate the initial row.
3. Build an original semantic Part map and a control candidate; keep uncertain regions
   unknown instead of filling them from memory.
4. Run the same curve/AuthoringTransaction, High, editable Low, UV/cage/bake,
   Material, FPS, engine, and review route for the benchmark and control candidates.
5. Compare only same-candidate, same-build, same-camera evidence. A profile match is
   not a visual pass; a structural or GLB readback pass is not human acceptance.
6. Retain rejected parents and all failed receipts. Confirm/export only after the
   user approves the exact candidate and the required independent human and engine
   gates are present.
