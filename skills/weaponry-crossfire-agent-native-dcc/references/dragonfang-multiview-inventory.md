# Dragonfang Kukri 多视图 Detail Inventory

本文件是 Codex 的编排参考，不是 Runtime Contract、GeometryProgram、材质真值或质量
通过凭证。它记录 2026-08-31 用户提供的 Dragonfang 多视图参考摘要，不保存图片字节、原图
路径、prompt、联系人或品牌资产。永久状态仍必须由 Runtime 以授权 ReferenceEvidence 和
CAS hash 重新绑定。

## Reference record

```yaml
inventory_id: dragonfang-kukri-multiview-inventory@1
reference_role: user-authorized-design-reference
reference_sha256: a8f1a169a3957cbeaaff2a8ceebcb9dd03802fcd7e165f043d329e8a5172dbd2
dimensions: 1536x1024
source_content_embedded: false
reference_kind: labeled-multiview-contact-sheet
visual_truth_boundary: supplied-view-observation-only
commercial_status: NOT_PROVEN
```

The sheet supplies nine labeled view groups: `front`, `back`, `left`, `right`, `top`, `bottom`,
`guard-bottom`, `pommel`, and `fps-hold`. The FPS group contains six observed hold frames. The
labels and visible shapes are observations of a supplied design sheet; they are not proof that all
views were rendered from one watertight mesh. Where a view disagrees with another, keep the
conflict explicit and do not average it into geometry.

## Evidence vocabulary

- `observed`: directly visible in one or more supplied panels.
- `design-inferred`: a constrained 3D interpretation needed to build a coherent asset, but not
  directly measured or guaranteed by the sheet.
- `unknown`: the sheet does not provide enough evidence to decide. Unknown remains blocked until a
  new authorized view or an explicit design decision arrives.

`confidence` describes observation confidence only. It is not a likeness score, a geometry gate, or
permission to promote High. Every retained detail must resolve to a semantic Part and, when it is a
surface cue, a live MaterialZone; unresolved IDs stay planned.

## Semantic target map

These are the intended targets for the current materialized multi-part candidate. They are not a
caller-supplied override: Runtime must resolve the IDs against the actual candidate and reject a
missing, duplicate, or drifted binding.

| semantic target | role | material-zone candidates | evidence boundary |
|---|---|---|---|
| `blade-body` | continuous red blade volume and primary silhouette | `dark-red-blade` | front/back/left/right/top/bottom |
| `cutting-edge` | edge strip, bevel and sharpening boundary | `silver-edge` or blade-edge zone | front/back/left/right/top/bottom |
| `dragon-relief` | raised/recessed blade ornament system | `antique-gold-ornament` | front/back; relief depth inferred |
| `guard-dragon-head` | guard, horns, jaw and blade junction | `antique-gold-ornament` | front/back/left/right/guard-bottom |
| `dragon-eye-left`, `dragon-eye-right` | focal eye/gem seats | `ruby-accent` | front/back/guard-bottom; side symmetry inferred |
| `gem` | handle/pommel focal gem or gem mount | `ruby-accent` | front/back/pommel; exact seat topology inferred |
| `grip` | black handle body, panels and curvature | `black-grip` | front/back/left/right/top/bottom/fps-hold |
| `grip-fastener` | repeated handle fasteners | `antique-gold-ornament` or metal-fastener zone | front/back/pommel; count must be read back |
| `pommel` | end cap, hook and terminal ornament | `antique-gold-ornament`/`ruby-accent` | front/back/left/right/pommel |

The material-zone names above are benchmark candidates, not an authorization to create unregistered
materials. If the active AssetPack uses different IDs, record the resolved IDs and their hashes in
the Runtime material evidence; do not silently rename them here.

## Detail inventory

### Macro: identity and assembly

| detail_id | target | observation | confidence | supplied views | review signal | first-pass state |
|---|---|---|---:|---|---|---|
| `macro-kukri-forward-belly` | `blade-body` | Forward-heavy kukri silhouette with a broad belly and a low sweeping cutting edge. | 0.98 | front, back | silhouette, bbox | required for blockout |
| `macro-spine-arc-and-tip` | `blade-body` | Spine rises from the guard and arcs toward a tapered tip; tip is visibly narrow in front/back and side views. | 0.95 | front, back, left, right | silhouette, edge | required for blockout |
| `macro-blade-to-guard-junction` | `blade-body` + `guard-dragon-head` | Blade terminates into a dense gold guard/dragon-head assembly rather than a plain tang. | 0.94 | front, back, left, right, guard-bottom | attachment, part-id | required for structural form |
| `macro-guard-jaw-negative-space` | `guard-dragon-head` | A crescent/jaw-like negative space is visible below the guard eye and around the finger opening. | 0.92 | front, back, guard-bottom | silhouette, depth, part-id | required for structural form |
| `macro-handle-offset` | `grip` + `pommel` | Grip axis departs from the blade axis and curves down toward the terminal pommel. | 0.91 | front, back, left, right, fps-hold | silhouette, attachment | required for structural form |
| `macro-pommel-terminal-hook` | `pommel` | Terminal cap ends in a hooked/crescent gold form with a focal red element. | 0.90 | front, back, pommel | silhouette, part-id | required for secondary form |
| `macro-thickness-taper` | `blade-body` + `cutting-edge` | Top/bottom and left/right panels show a thin tip and thicker guard-adjacent section. Exact section profile is not measured. | 0.86 | left, right, top, bottom | depth, normal, wireframe | required for cross-section |
| `macro-fps-composition` | all assembly | In FPS holds, blade occupies the upper-left/center field and grip remains lower-right with guard readable. | 0.88 | fps-hold | camera, silhouette, part-id | presentation only |

### Meso: form, topology and repeated systems

| detail_id | target | observation | confidence | supplied views | review signal | first-pass state |
|---|---|---|---:|---|---|---|
| `meso-spine-dragon-armor` | `dragon-relief` | Raised gold dragon/scale system follows the upper blade spine and changes density near the guard. | 0.92 | front, back | normal, material-id, part-id | high geometry target |
| `meso-blade-belly-inlay` | `dragon-relief` + `blade-body` | Long gold ornamental flow occupies the inner red blade field and follows the curve rather than a straight decal line. | 0.90 | front, back | normal, material-id | high geometry target |
| `meso-cutting-edge-bevel` | `cutting-edge` | A contrasting edge strip and bevel highlight run along the cutting contour; its exact width varies with perspective. | 0.89 | front, back, left, right | normal, depth, wireframe | high geometry target |
| `meso-tip-transition` | `blade-body` + `cutting-edge` | Belly and spine converge into a tapered tip with a distinct edge transition. | 0.90 | front, back, left, right, top, bottom | silhouette, normal | high geometry target |
| `meso-grind-plunge-region` | `blade-body` + `cutting-edge` | A long inner blade plane/finish boundary is suggested between the ornament and cutting edge. Exact plunge/ricasso topology is inferred. | 0.72 | front, back, left, right | normal, roughness-material-id | inspect before sculpt |
| `meso-guard-horns-and-brow` | `guard-dragon-head` | Horns, brow and cheek planes form an outward-facing dragon-head relief at the blade root. | 0.93 | front, back, guard-bottom | silhouette, normal, part-id | secondary form |
| `meso-guard-eye-seat` | `dragon-eye-left` + `dragon-eye-right` | Red focal eye/gem sits in a gold circular/organic seat at the guard. | 0.90 | front, back, guard-bottom | part-id, material-id, normal | secondary form |
| `meso-guard-finger-opening` | `guard-dragon-head` | Guard-bottom panel exposes the opening and underside depth; it must be a real void, not dark paint. | 0.88 | front, back, guard-bottom | silhouette, depth, interior | secondary form |
| `meso-grip-palm-swell` | `grip` | Handle has a curved palm swell and narrowing near the guard/pommel. | 0.90 | front, back, left, right, fps-hold | silhouette, depth | structural form |
| `meso-grip-panel-seams` | `grip` | Black handle is segmented by longitudinal panels/borders and short transverse breaks. | 0.86 | front, back, left, right, top, bottom | part-id, normal, roughness | secondary form |
| `meso-grip-fastener-row` | `grip-fastener` | Repeated round gold fasteners follow the handle panels; visible count and spacing must be preserved per side. | 0.86 | front, back, pommel | part-id, material-id | repeated system |
| `meso-pommel-cap-ring` | `pommel` | Gold terminal ring/cap encloses the handle and transitions into the hook. | 0.88 | front, back, left, right, pommel | attachment, silhouette | secondary form |
| `meso-side-thickness-planes` | `blade-body` + `guard-dragon-head` + `grip` | Left/right/top/bottom establish object thickness and side-plane continuity, but do not prove hidden topology. | 0.82 | left, right, top, bottom | depth, normal, wireframe | cross-section gate |
| `meso-fps-hold-alignment` | `grip` + assembly pivot | Six holds show grip/hand-relative placement and blade visibility at inspect angles; hand geometry is not weapon truth. | 0.84 | fps-hold | camera, socket proxy | FPS deferred |

### Micro: surface identity and finish cues

| detail_id | target | observation | confidence | material-zone candidates | supplied views | treatment |
|---|---|---|---:|---|---|---|
| `micro-dragon-scale-repetition` | `dragon-relief` | Repeated scale-like modules are visible along the raised dragon body. | 0.90 | `antique-gold-ornament` | front, back | geometry/normal after form gate |
| `micro-engraved-line-flow` | `dragon-relief` | Fine engraved lines branch around the central relief and follow the blade curvature. | 0.86 | `antique-gold-ornament` | front, back | normal/decal only after identity geometry |
| `micro-gold-border-bevel` | blade/guard/grip borders | Thin gold borders separate red blade, black grip and gold ornament zones. | 0.89 | `antique-gold-ornament` | front, back, left, right | edge/bevel + material ID |
| `micro-red-blade-coating` | `blade-body` | Dominant dark-red painted/coated appearance remains visible between gold relief and edge. | 0.94 | `dark-red-blade` | front, back, left, right | material layer; not silhouette compensation |
| `micro-edge-highlight` | `cutting-edge` | Bright narrow edge response is visible, but highlight alone does not prove a bevel. | 0.87 | `silver-edge` | front, back, left, right | geometry first, material second |
| `micro-ruby-focal-response` | `dragon-eye-left`, `dragon-eye-right`, `gem`, `pommel` | Small red gems provide focal accents with controlled reflection/emission-like response. | 0.88 | `ruby-accent` | front, back, guard-bottom, pommel | PBR readback; emissive not assumed |
| `micro-grip-fastener-heads` | `grip-fastener` | Circular fastener heads have gold rims and darker centers. | 0.87 | metal-fastener zone | front, back, pommel | repeated part + material |
| `micro-grip-contact-wear` | `grip` | Handle surface shows darker panel variation consistent with contact wear or intentional panel design. | 0.66 | `black-grip` | front, back, fps-hold | design-inferred until material evidence |
| `micro-controlled-edge-wear` | `cutting-edge` + `blade-body` | Edge and blade boundary show restrained tonal variation; no physical wear distribution is directly measurable. | 0.61 | `silver-edge`, `dark-red-blade` | front, back | planned material override only |
| `micro-cavity-darkening` | guard/relief/grip recesses | Dark cavities increase separation around relief and panel seams. | 0.78 | zone-specific | front, back, guard-bottom, pommel | AO/roughness cue, not geometry |
| `micro-hidden-relief-backside` | `dragon-relief` | Back panel has a related ornamental flow, but exact continuity and depth between front/back are not observable. | 0.56 | `antique-gold-ornament` | back | design-inferred; do not overfit |
| `micro-guard-under-cut` | `guard-dragon-head` | Guard-bottom exposes undercuts and inner jaw planes not visible in the primary front view. | 0.79 | `antique-gold-ornament` | guard-bottom | high geometry/negative-space review |
| `micro-pommel-gem-seat` | `pommel` + `gem` | Pommel view shows a red focal element nested inside concentric gold/black forms. | 0.88 | `ruby-accent`, `antique-gold-ornament` | pommel | part/material ID |
| `micro-top-bottom-edge-continuity` | `cutting-edge` | Top/bottom panels imply edge continuity around the tip, but exact winding/UV seams remain unknown. | 0.72 | `silver-edge` | top, bottom | topology/UV later |
| `micro-surface-finish-variation` | all material zones | Gold, red coating, black grip and red gem have distinct visible response; numeric roughness/metalness is not observed. | 0.83 | all zones | all non-FPS views | infer PBR parameters only with evidence |

## Observed versus inferred boundary

The following is safe to use as direct visual observation:

- overall curved kukri silhouette, forward belly, tapered tip and handle offset;
- visible front/back ornament placement and red/gold/black visual zones;
- left/right/top/bottom evidence that the blade is a thin solid with a thicker root;
- guard underside opening/undercut cues and pommel terminal view;
- six FPS hold framings as presentation observations.

The following must remain `design-inferred` or `unknown` until separately validated:

- exact continuous 3D depth of the dragon relief and underside of every ornament;
- exact bevel width, grind/plunge topology, edge radius and normal continuity;
- hidden internal attachment, tang, socket transforms and collision geometry;
- whether repeated fasteners/gems are perfectly symmetric across the two faces;
- physical roughness, metalness, clearcoat, emissive intensity, wear chronology and texture pixels;
- a single-source-model guarantee for the generated contact sheet;
- hand mesh, skeleton scale and animation timing inferred from FPS holds.

Do not convert an inference into a Runtime source hash merely because it is visually plausible.

## Fixed-view set and priority

The supplied panels are the reference views. Rendered orbits are diagnostic 3D evidence and must
not be relabeled as reference likeness. All comparison views use one candidate, one camera set, one
reference hash and one Worker cohort.

| priority | view_id | role | primary questions | promotion use |
|---:|---|---|---|---|
| 0 | `front` | canonical reference | blade silhouette, belly/spine balance, relief flow, guard and grip layout | primary visible-view gate |
| 1 | `back` | opposite reference | reverse silhouette, ornament continuity, handle/pommel placement | reverse-face gate |
| 2 | `left`, `right` | thickness/side references | edge profile, root thickness, handle section, chirality and attachment | cross-section gate |
| 3 | `top`, `bottom` | orthographic thickness references | tip taper, blade thickness, guard/grip depth and edge continuity | cross-section/UV seam planning |
| 4 | `guard-bottom` | local underside reference | jaw void, eye seat, undercut and blade/guard junction | negative-space/attachment gate |
| 5 | `pommel` | local terminal reference | cap ring, hook, gem seat and grip termination | pommel gate |
| 6 | `fps-hold-01..06` | presentation references | screen occupancy, grip position, blade readability, inspect framing | FPS presentation only |

The first post-High render batch should include all supplied non-FPS views plus at least two
non-degenerate product orbits, for example `orbit-45` and `orbit-135`. Orbit passes answer volume,
occlusion and self-consistency questions; they do not establish unseen reference likeness. The FPS
frames should be reviewed separately after pivot/socket decisions and must not be mixed into the
blade-shape score.

## First single-scope correction after multi-part High

The first correction is deliberately narrow:

```yaml
pass: high-geometry-review-01
decision_order:
  - camera_normalization
  - macro-silhouette
  - primary_proportion
  - guard_negative_space
  - cross_section_and_attachment
  - identity_relief
  - material
  - lighting
first_scope:
  name: macro-silhouette
  targets: [blade-body, cutting-edge]
  views: [front, back, left, right, top, bottom]
  allowed_changes:
    - blade spine arc
    - belly depth
    - tip position and taper
    - cutting-edge path
  forbidden_changes:
    - dragon-relief
    - guard-dragon-head
    - grip
    - pommel
    - materials
    - FPS camera or socket
  decision: continue | refine-spec | refine-code | request-input | stop
```

If the first comparison shows a camera/scale mismatch, perform a camera-only correction and retain
the exact geometry candidate. If the camera is valid but the silhouette is wrong, create one child
AuthoringMesh successor touching only `blade-body`/`cutting-edge`; preserve the baseline and run the
same fixed views again. Do not add dragon scales, brighten the edge, or tune roughness to compensate
for a wrong silhouette. A later correction may address exactly one of `guard-dragon-head` negative
space, blade cross-section/attachment, `dragon-relief`, or material zones, in that order.

Stop and request input when the same defect repeats twice, when a missing view is needed to decide
between competing 3D interpretations, or when binding/topology/readback/cohort integrity fails.
The corrected child is not High-pass, commercial, human-approved or engine-validated until its
independent gates actually produce those statuses.

## Handoff to Runtime orchestration

This inventory supplies Codex with stable intent and comparison scope only. The next product action
is to bind `reference_sha256` to the immutable ReferenceEvidence successor and let Runtime derive its
own source/detail/quality hashes. It must not copy this Markdown into CAS, infer missing geometry
from the inventory, or treat the inventory hash as a GeometryProgram hash. After multi-part High,
the first bounded review is:

`surface_pipeline → high_artifact_reference_compare_prepare → observe → quality_review`

with `candidate_visual_evidence_projection=NOT_UPDATED` until a separate candidate-bound review is
explicitly authorized. This preserves the distinction between High Artifact structural evidence,
reference comparison, visual review, human acceptance, engine validation and commercial delivery.
