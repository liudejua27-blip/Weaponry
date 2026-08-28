# Native High Bundle proposal

Status: `target-design` / `development-only` / `not registered` / `execution unavailable`; source-only Runtime/MCP durability gate passed.

This proposal records the ForgeCAD-owned Native High typed boundary associated with
`hard-surface-detail@0.2.0`. It is intentionally outside `packages/forgecad-skills/bundles/**`
and is therefore not exposed by the active registry or Runtime Skill capability count.

The proposal binds `HighMeshWorkerRequest@1`, `HighMeshArtifact@1` and `DetailGraph@1`
to a fixed one-shot worker lock and its deterministic embedded-only
`forgecad.production.high-mesh-glb-materialize@1` sibling. The embedded schema set also
contains the seven durable/GLB contracts (`NativeHighDurableGetRequest/Result@1`,
`NativeHighDurableLink@1`, `NativeHighDurablePrepareRequest/Result@1`, and
`NativeHighGlbMaterializeRequest/Result@1`). The standalone worker source slice has a
source-only double replay/byte-exact determinism receipt, strict GLB readback and
preserves the AuthoringMesh base. The Runtime-owned source path now persists the High,
GLB, readback and link objects in CAS/Store, supports idempotent replay and survives a
real Runtime drop/reopen/get; public MCP get/prepare source also compiles and passes its
focused test. None of these operations advances a ProductionStage.

Current evidence note (2026-08-25): the embedded schemas, source lock and benchmark
receipts are synchronized to the stable-ID AuthoringMesh adapter and current GLB sibling.
The exact offline test target passes six library behavioral tests plus seven dedicated
transport tests, including High/GLB two-process replay, strict embedded GLB readback and
closed failure behavior. A same-cohort real-process Runtime restart fixture passes 1/1,
and the public MCP source surface is 107 read + 79 opt-in write = 186 tools. This is
source-only structural/durable evidence, not a packaged or High quality gate.

The following gates remain explicit:

- `runtime_integration_gate=PASS_SOURCE_DURABLE_RESTART_MCP_ONLY`; source Runtime/MCP/CAS
  integration has a focused receipt. Packaged same-cohort integration, candidate-bound
  commercial quality and the approval evidence chain remain pending. This source receipt
  does not promote or activate the proposal.
- `high_gate=NOT_PASSED`; `structural_status=PASS_SOURCE_STRUCTURAL` is limited to the
  recorded source-only structural pipeline and is not a current High production or
  visual-quality PASS.
- UV, tangent, visual, human, engine and distribution gates remain `NOT_RUN`.

Do not add this proposal to `registry.json` or copy it into active `bundles/**` until
the Runtime owner supplies the fixed packaged worker cohort, candidate-bound quality
evidence, approval-bound workflow and the independent visual/human/engine/distribution gates.
