import { useMemo } from 'react'
import type { AgentAssetChangeSet, AgentMaterialPreset } from '../../shared/types'

type CadWorkbenchPanelMaterialOptionsPresentation = {
  open: boolean
  hasShapeProgram: boolean
  isExternalGlbReference: boolean
  materialPresets: readonly AgentMaterialPreset[]
  quickMaterialPresets: readonly AgentMaterialPreset[]
  appearanceMaterialId: string
  selectedPartLabel: string
  selectedMaterialZoneId: string
  hasAgentAssetVersion: boolean
  quickMaterialDisabled: boolean
  activeMaterialDomain: string | null
  compatibilityOnly: boolean
  materialQuery: string
  materialCategory: AgentMaterialPreset['category'] | 'all'
  catalogLoading: boolean
  catalogMessage: string | null
  selectedMaterialZoneIds: readonly string[]
  materialEditorDisabled: boolean
  onQuickMaterialPreset: (materialId: string, materialName: string) => void
  onMaterialChange: (materialId: string) => void
  onMaterialZoneChange: (zoneId: string) => void
  onMaterialCompatibilityChange: (value: boolean) => void
  onMaterialQueryChange: (query: string) => void
  onMaterialCategoryChange: (category: AgentMaterialPreset['category'] | 'all') => void
  onCatalogMaterialPreview: (materialId: string, materialName: string) => void
  onCatalogMaterialPreviewNote: (materialName: string) => void
}

type UseCadWorkbenchPanelMaterialOptionsPresentationInput = {
  showComposerAdvancedActions: boolean
  materialOptionsOpen: boolean
  agentBlockoutShapeProgram: Record<string, unknown> | null
  isExternalGlbReference: boolean
  materialPresets: readonly AgentMaterialPreset[]
  quickMaterialPresets: readonly AgentMaterialPreset[]
  appearanceMaterialId: string
  selectedPartRoleLabel: string
  selectedMaterialZoneId: string
  hasSelectedAgentPart: boolean
  agentAssetChangeSet: AgentAssetChangeSet | null
  selectedMaterialZoneIds: readonly string[]
  hasAgentAssetVersion: boolean
  activeMaterialDomain: string | null
  materialCompatibilityOnly: boolean
  materialQuery: string
  materialCategory: AgentMaterialPreset['category'] | 'all'
  catalogLoading: boolean
  catalogMessage: string | null
  quickMaterialPresetSelect: (materialId: string, materialName: string) => void
  selectMaterialPreselection: (materialId: string) => void
  selectMaterialZone: (zoneId: string) => void
  setMaterialFilterCompatibilityOnly: (value: boolean) => void
  setMaterialFilterQuery: (query: string) => void
  setMaterialFilterCategory: (category: AgentMaterialPreset['category'] | 'all') => void
  catalogMaterialPreview: (materialId: string, materialName: string) => void
  catalogMaterialPreviewNote: (materialName: string) => void
}

export function useCadWorkbenchPanelMaterialOptionsPresentation(
  input: UseCadWorkbenchPanelMaterialOptionsPresentationInput,
): CadWorkbenchPanelMaterialOptionsPresentation | null {
  return useMemo(
    () => (
      input.showComposerAdvancedActions
        ? {
          open: input.materialOptionsOpen,
          hasShapeProgram: Boolean(input.agentBlockoutShapeProgram),
          isExternalGlbReference: input.isExternalGlbReference,
          materialPresets: input.materialPresets,
          quickMaterialPresets: input.quickMaterialPresets,
          appearanceMaterialId: input.appearanceMaterialId,
          selectedPartLabel: input.selectedPartRoleLabel,
          selectedMaterialZoneId: input.selectedMaterialZoneId,
          hasAgentAssetVersion: input.hasAgentAssetVersion,
          quickMaterialDisabled: Boolean(input.agentAssetChangeSet)
            || Boolean(input.hasAgentAssetVersion && input.hasSelectedAgentPart && !input.selectedMaterialZoneId),
          activeMaterialDomain: input.activeMaterialDomain,
          compatibilityOnly: input.materialCompatibilityOnly,
          materialQuery: input.materialQuery,
          materialCategory: input.materialCategory,
          catalogLoading: input.catalogLoading,
          catalogMessage: input.catalogMessage,
          selectedMaterialZoneIds: input.selectedMaterialZoneIds,
          materialEditorDisabled: input.isExternalGlbReference || Boolean(input.agentAssetChangeSet),
          onQuickMaterialPreset: input.quickMaterialPresetSelect,
          onMaterialChange: input.selectMaterialPreselection,
          onMaterialZoneChange: input.selectMaterialZone,
          onMaterialCompatibilityChange: input.setMaterialFilterCompatibilityOnly,
          onMaterialQueryChange: input.setMaterialFilterQuery,
          onMaterialCategoryChange: input.setMaterialFilterCategory,
          onCatalogMaterialPreview: input.catalogMaterialPreview,
          onCatalogMaterialPreviewNote: input.catalogMaterialPreviewNote,
        }
        : null
    ),
    [
      input.showComposerAdvancedActions,
      input.materialOptionsOpen,
      input.agentBlockoutShapeProgram,
      input.isExternalGlbReference,
      input.materialPresets,
      input.quickMaterialPresets,
      input.appearanceMaterialId,
      input.selectedPartRoleLabel,
      input.selectedMaterialZoneId,
      input.hasSelectedAgentPart,
      input.agentAssetChangeSet,
      input.selectedMaterialZoneIds,
      input.hasAgentAssetVersion,
      input.activeMaterialDomain,
      input.materialCompatibilityOnly,
      input.materialQuery,
      input.materialCategory,
      input.catalogLoading,
      input.catalogMessage,
      input.quickMaterialPresetSelect,
      input.selectMaterialPreselection,
      input.selectMaterialZone,
      input.setMaterialFilterCompatibilityOnly,
      input.setMaterialFilterQuery,
      input.setMaterialFilterCategory,
      input.catalogMaterialPreview,
      input.catalogMaterialPreviewNote,
    ],
  )
}
