# Constraints

- Discover `capabilities_get`, `runtime_status`, `doctor`, `operator_catalog_get`
  and `skill_list` before authoring.
- Match the live operator catalog digest and exact project ID; call the Runtime
  `geometry_program_hash` tool instead of calculating a client hash.
- Use metres, radians and right-handed Y-up coordinates. Keep primitive nodes
  closed and connect every source exactly once through `part_outputs`.
- Require strict ArtifactReadback@2 zero counters and full lineage coverage.
- Do not infer hidden surfaces as observed. A single image is only a visible-view
  structural candidate until the later reference/render/human gates exist.
- Treat `limited` quality as a hard stop for visual-quality claims. A user may
  explicitly request a structural MVP blockout export after the Runtime hard
  gates pass, but the receipt must say `STRUCTURAL_BLOCKOUT` and must not claim
  visual similarity, PBR fidelity or `HQ_360_PASS`.
