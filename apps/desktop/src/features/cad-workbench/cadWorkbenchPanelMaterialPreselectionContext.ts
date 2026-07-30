export type CadWorkbenchPanelMaterialPreselectionSource =
  | 'external_glb'
  | 'legacy'
  | 'agent_asset'
  | 'blockout'
  | 'none'

export type CadWorkbenchPanelMaterialPreselectionContext = {
  projectId: string | null
  assetVersionId: string | null
  selectedPartId: string | null
  materialZoneId: string | null
  source: CadWorkbenchPanelMaterialPreselectionSource
}

type ResolveCadWorkbenchPanelMaterialPreselectionContextInput = {
  projectId: string | null
  isExternalGlbReference: boolean
  legacyDesignReadOnly: boolean
  assetVersionId: string | null
  selectedPartId: string | null
  selectedMaterialZoneId: string | null
  hasAgentAssetVersion: boolean
  hasAgentPlan: boolean
}

export function resolveCadWorkbenchPanelMaterialPreselectionContext(
  input: ResolveCadWorkbenchPanelMaterialPreselectionContextInput,
): CadWorkbenchPanelMaterialPreselectionContext {
  const source: CadWorkbenchPanelMaterialPreselectionSource = input.isExternalGlbReference
    ? 'external_glb'
    : input.legacyDesignReadOnly
      ? 'legacy'
      : input.hasAgentAssetVersion
        ? 'agent_asset'
        : input.hasAgentPlan
          ? 'blockout'
          : 'none'

  return {
    projectId: input.projectId,
    assetVersionId: input.assetVersionId,
    selectedPartId: input.selectedPartId,
    materialZoneId: input.selectedMaterialZoneId,
    source,
  }
}
