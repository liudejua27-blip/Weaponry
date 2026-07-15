# ForgeCAD Concept Contracts

This package is the machine-readable source of truth for the P0 Concept domain.

Source schemas live in `schemas/`. Generated TypeScript in `generated/types.ts` must not be edited by hand.

Contracts:

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
