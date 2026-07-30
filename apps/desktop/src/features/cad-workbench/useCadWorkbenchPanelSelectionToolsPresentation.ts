import { useCadWorkbenchPanelAgentSelectionCardPresentation } from './useCadWorkbenchPanelAgentSelectionCardPresentation'
import { useCadWorkbenchPanelMaterialOptionsPresentation } from './useCadWorkbenchPanelMaterialOptionsPresentation'

type CadWorkbenchPanelSelectionToolsPresentationInput = (
  Parameters<typeof useCadWorkbenchPanelAgentSelectionCardPresentation>[0]
  & Parameters<typeof useCadWorkbenchPanelMaterialOptionsPresentation>[0]
)

type CadWorkbenchPanelSelectionToolsPresentation = {
  agentSelectionCardProps: ReturnType<typeof useCadWorkbenchPanelAgentSelectionCardPresentation>
  materialOptionsProps: ReturnType<typeof useCadWorkbenchPanelMaterialOptionsPresentation>
}

export function useCadWorkbenchPanelSelectionToolsPresentation(
  input: CadWorkbenchPanelSelectionToolsPresentationInput,
): CadWorkbenchPanelSelectionToolsPresentation {
  const agentSelectionCardProps = useCadWorkbenchPanelAgentSelectionCardPresentation({
    agentBlockoutSegmentation: input.agentBlockoutSegmentation,
    agentAssetVersion: input.agentAssetVersion,
    activeAgentAssetVersion: input.activeAgentAssetVersion,
    selectedPart: input.selectedPart,
    selectedPartId: input.selectedPartId,
    partDisplay: input.partDisplay,
    isSelectedPartLocked: input.isSelectedPartLocked,
    isExternalGlbReference: input.isExternalGlbReference,
    isSnapshotActionPending: input.isSnapshotActionPending,
    agentAssetChangeSet: input.agentAssetChangeSet,
    agentComponentCandidates: input.agentComponentCandidates,
    agentStructureSuggestions: input.agentStructureSuggestions,
    structureSuggestionUnavailableMessage: input.structureSuggestionUnavailableMessage,
    semanticProportions: input.semanticProportions,
    editAssistLoading: input.editAssistLoading,
    blockoutPreviewPresentation: input.blockoutPreviewPresentation,
    onSelectPart: input.onSelectPart,
    onPreviewEdit: input.onPreviewEdit,
    onSaveSelectedComponent: input.onSaveSelectedComponent,
    onReplaceComponent: input.onReplaceComponent,
    onPreviewStructureSuggestion: input.onPreviewStructureSuggestion,
    onSetPartDisplay: input.onSetPartDisplay,
    onInspectAsset: input.onInspectAsset,
    onRejectChange: input.onRejectChange,
    onConfirmChange: input.onConfirmChange,
    onOpenSurfaceAdornment: input.onOpenSurfaceAdornment,
    surfaceAdornmentDisabled: input.surfaceAdornmentDisabled,
    surfaceAdornmentDetail: input.surfaceAdornmentDetail,
  })

  const materialOptionsProps = useCadWorkbenchPanelMaterialOptionsPresentation({
    showComposerAdvancedActions: input.showComposerAdvancedActions,
    materialOptionsOpen: input.materialOptionsOpen,
    agentBlockoutShapeProgram: input.agentBlockoutShapeProgram,
    isExternalGlbReference: input.isExternalGlbReference,
    materialPresets: input.materialPresets,
    quickMaterialPresets: input.quickMaterialPresets,
    appearanceMaterialId: input.appearanceMaterialId,
    selectedPartRoleLabel: input.selectedPartRoleLabel,
    selectedMaterialZoneId: input.selectedMaterialZoneId,
    hasSelectedAgentPart: input.hasSelectedAgentPart,
    agentAssetChangeSet: input.agentAssetChangeSet,
    selectedMaterialZoneIds: input.selectedMaterialZoneIds,
    hasAgentAssetVersion: input.hasAgentAssetVersion,
    activeMaterialDomain: input.activeMaterialDomain,
    materialCompatibilityOnly: input.materialCompatibilityOnly,
    materialQuery: input.materialQuery,
    materialCategory: input.materialCategory,
    catalogLoading: input.catalogLoading,
    catalogMessage: input.catalogMessage,
    quickMaterialPresetSelect: input.quickMaterialPresetSelect,
    selectMaterialPreselection: input.selectMaterialPreselection,
    selectMaterialZone: input.selectMaterialZone,
    setMaterialFilterCompatibilityOnly: input.setMaterialFilterCompatibilityOnly,
    setMaterialFilterQuery: input.setMaterialFilterQuery,
    setMaterialFilterCategory: input.setMaterialFilterCategory,
    catalogMaterialPreview: input.catalogMaterialPreview,
    catalogMaterialPreviewNote: input.catalogMaterialPreviewNote,
  })

  return {
    agentSelectionCardProps,
    materialOptionsProps,
  }
}
