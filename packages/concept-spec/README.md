# ForgeCAD Agent Asset Contracts

This package is the machine-readable source of truth for current ForgeCAD Agent assets and the future ADR-0022 universal asset contracts. Existing mechanical/domain contracts remain compatibility inputs; they are not a permanent category allowlist.

Source schemas live in `schemas/`. Generated TypeScript in `generated/types.ts` must not be edited by hand.

Contracts:

U002 universal author contracts and U003 `UniversalAssetSource@1`/camera/detail/appearance contracts are implemented in Schema, generated types, Rust validators/builders, and focused Gates. U004 adds `UniversalAssetSource@2` as the forward-only discriminated source contract: its procedural branch independently re-lowers a bounded VP203/author source; deformable, local mesh patch and hybrid are explicit but unavailable until their own compiler and visual gates exist. The current product executable source branch remains the validated procedural robotic-arm capability.

- `UniversalAuthorRequest@1`
- `SubjectProfile@1`
- `VisualFeatureContract@1`
- `RepresentationPlan@1`
- `RepresentationLimitation@1`
- `UniversalAuthorOutcome@1`
- `VisualEvidenceGraph@2`

- `DesignDomainProfile@1`
- `WeaponConceptSpec@1`
- `ModuleAssetManifest@1`
- `ModulePackManifest@1`
- `FormalModuleReview@1`
- `ModuleGraph@1`
- `DesignChangeSet@1`
- `ModelQualityReport@1`
- `JobEvent@2`
- `ConceptExportManifest@1`
- `DomainPackManifest@1`
- `DomainInferenceResult@1`
- `MechanicalConceptSpec@1`
- `AssemblyGraph@1`
- `MaterialPreset@1`
- `ShapeProgram@1`
- `AgentComponent@1`
- `AgentAssetExport@1`
- `AgentAssetRenderPackage@1`

Regenerate after schema changes:

```bash
npm run contracts:types:generate
```

Validate the first R2 slice:

```bash
npm run r2:contracts-gate
```

The legacy `packages/weapon-spec` package remains frozen for backward-compatible M2–M6 regression. New Concept code must not add fields to legacy Weapon/Skill graphs.
