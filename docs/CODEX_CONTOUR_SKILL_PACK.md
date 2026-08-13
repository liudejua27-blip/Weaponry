# ForgeCAD contour-first Skill pack

This pack is the Codex/Luna operating contract for reference-driven modeling.
It is declarative: Runtime remains the only executor and these profiles do not
install Python, Blender, JavaScript, remote segmentation, or model weights.

| Profile | Purpose | Runtime surface | Status |
| --- | --- | --- | --- |
| `reference-silhouette-intake@1` | authorized image, 512px target, visible/unknown regions | `reference_mask_prepare`, `reference_mask_refine_prepare`, `silhouette_target_get` | source available |
| `camera-solve@1` | yaw, pitch, roll, FOV, distance, target offset and framing scale | `camera_fit_prepare`, `silhouette_fit_prepare` | source available |
| `silhouette-rig-fit@1` | bounded `SilhouetteRig@1` camera + candidate-bound geometry variant search | `silhouette_fit_prepare` | source available |
| `part-boundary-repair@1` | one semantic Part adjustment from SDF/boundary evidence | `boundary_error_get`, `part_contour_fit_prepare` | source available |
| `hard-surface-form@1` | accepted typed geometry/detail program | `geometry_program_hash`, `geometry_prepare`, `artifact_readback_get` | existing first-party path |
| `surface-material@1` | offline AssetPack materials after form gates | `appearance_prepare`, `material_pack_get` | existing first-party path |
| `visual-review@1` | fixed AOV comparison and typed reviews | `reference_compare_prepare`, `render_pass_get`, `silhouette_candidate_compare`, review tools | source available |

These are execution profiles, not independent plugins. They share project,
candidate, target, camera, RenderSet and hash lineage; no profile confirms a
version.

## Required loop

```text
reference_import → reference_mask_prepare → silhouette_target_get
→ operator_catalog_get / geometry_program_hash → geometry_prepare
→ artifact_readback_get → camera_fit_prepare or silhouette_fit_prepare
→ reference_compare_prepare → boundary_error_get
→ part_contour_fit_prepare (one Part) → new geometry_prepare
→ silhouette_candidate_compare → visual_review_submit
→ human_visual_review_submit → quality_get
```

The optimizer is deliberately bounded at 64 total evaluations and eight
iterations. When V2 evidence is available, Runtime compiles and renders a
small number of candidate-bound Rig variants and returns their metrics and
parameter deltas; it never silently rewrites a mesh. SDF/Chamfer is a
diagnostic loss, not multi-view reconstruction.

Hard stops: `silhouette_iou >= 0.90`, `boundary_f1_4px >= 0.90`,
`bbox_edge_error <= 0.02`, `centroid_error <= 0.02`, landmark coverage `>= 0.80`,
landmark NME `<= 0.03`, region median IoU `>= 0.85`, and critical-region IoU
`>= 0.85` are required for `VISIBLE_SILHOUETTE_PASS`. Until
front/back/left/right/rear-three-quarter references exist, `HQ_360_PASS` remains
`BLOCKED_REFERENCE_COVERAGE`.
