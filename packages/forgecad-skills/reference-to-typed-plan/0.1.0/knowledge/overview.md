# Reference to Typed Plan

This first-party Skill is the planning half of the ForgeCAD image-to-3D loop.
Codex performs visual interpretation; the Skill records that interpretation in
typed, bounded fields. Runtime remains the only writer and later compilers may
consume the plan only after their own contracts and quality gates pass.

The intended loop is deliberately staged:

1. intake: verify the `ReferenceEvidence` hash, MIME, dimensions and authorization;
2. inventory: enumerate identity-defining silhouette, panel, joint, bevel, fastener and material-zone observations;
3. plan: choose stable Part/MaterialZone IDs, coordinate assumptions, symmetry and a budget;
4. review: attach confidence and explicit unknowns before any geometry operator is unlocked.

The result is a plan, not a mesh. It can be diffed, hashed and rejected without
running a browser, Python, JavaScript, CSS, external model or network service.
