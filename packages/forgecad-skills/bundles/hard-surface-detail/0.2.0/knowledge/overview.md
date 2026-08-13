# hard-surface-detail

First-party declarative Skill `hard-surface-detail@0.2.0`. It declares typed inputs, a bounded Recipe and product-owned validators; it does not contain executable code.

The default recipe is an ordered hard-surface pass rather than a single
primitive: profile body and shell, revolved joint, panel, vent array, joint
stack, tube-sweep cable, mirrored limb, repeated ribs, transform fit and a
semantic Part sink. Codex must still supply the actual closed
`GeometryProgram@2` parameters from the live operator catalog; recipe stages
are planning guidance, not an alternate executor.

For appearance, pair this Skill with the separately verified offline
`forgecad-hard-surface-robot@1.0.0` AssetPack and `uv-pbr@0.2.0`. Material
names and texture hashes come from that pack; this bundle does not silently
install or fetch assets.

This bundle is planning metadata for the single-user MVP. A successful registry or declarative benchmark check does not claim that geometry, render or visual similarity has passed.
