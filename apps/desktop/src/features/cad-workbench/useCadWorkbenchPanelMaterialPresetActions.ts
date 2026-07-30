import { useCallback } from 'react'

import { createQuickMaterialPreviewOperationForPreset } from './cadWorkbenchPanelMaterialPreview'
import {
  MATERIAL_PRESET_NO_ZONE_NOTICE,
  buildBlockoutMaterialPreviewNote,
  buildMaterialPresetSummary,
  buildMaterialPreviewNote,
} from './cadWorkbenchPanelEditOperations'
import type { AgentPartEditOperation } from '../../shared/types'

type MaterialAssetEditPreview = (
  operation: AgentPartEditOperation | readonly AgentPartEditOperation[],
  summary: string,
) => void | Promise<void>

type UseCadWorkbenchPanelMaterialPresetActionsInput = {
  hasAgentAssetVersion: boolean
  selectedAgentPartId: string | null
  selectedMaterialZoneId: string | null
  selectMaterialPreselection: (materialId: string) => void
  previewAgentAssetEdit: MaterialAssetEditPreview
  setAssistantNote: (note: string) => void
}
type UseCadWorkbenchPanelMaterialPresetActionsOutput = {
  quickMaterialPresetSelect: (materialId: string, materialName: string) => void
  catalogMaterialPreview: (materialId: string, materialName: string) => void
  catalogMaterialPreviewNote: (materialName: string) => void
}

export function useCadWorkbenchPanelMaterialPresetActions({
  hasAgentAssetVersion,
  selectedAgentPartId,
  selectedMaterialZoneId,
  selectMaterialPreselection,
  previewAgentAssetEdit,
  setAssistantNote,
}: UseCadWorkbenchPanelMaterialPresetActionsInput): UseCadWorkbenchPanelMaterialPresetActionsOutput {
  const quickMaterialPresetSelect = useCallback((materialId: string, materialName: string): void => {
    selectMaterialPreselection(materialId)
    if (hasAgentAssetVersion && selectedAgentPartId) {
      const operation = createQuickMaterialPreviewOperationForPreset({
        partId: selectedAgentPartId,
        materialId,
        materialZoneId: selectedMaterialZoneId,
      })
      if (operation) {
        void previewAgentAssetEdit(
          operation,
          buildMaterialPresetSummary(selectedMaterialZoneId, materialName),
        )
        return
      }

      setAssistantNote(MATERIAL_PRESET_NO_ZONE_NOTICE)
      return
    }

    setAssistantNote(buildBlockoutMaterialPreviewNote(materialName))
  }, [
    hasAgentAssetVersion,
    previewAgentAssetEdit,
    selectMaterialPreselection,
    selectedAgentPartId,
    selectedMaterialZoneId,
    setAssistantNote,
  ])

  const catalogMaterialPreview = useCallback((materialId: string, materialName: string): void => {
    if (!hasAgentAssetVersion || !selectedAgentPartId) return
    const operation = createQuickMaterialPreviewOperationForPreset({
      partId: selectedAgentPartId,
      materialId,
      materialZoneId: selectedMaterialZoneId,
    })
    if (!operation) return
    void previewAgentAssetEdit(operation, buildMaterialPresetSummary(selectedMaterialZoneId, materialName))
  }, [
    hasAgentAssetVersion,
    previewAgentAssetEdit,
    selectedAgentPartId,
    selectedMaterialZoneId,
  ])

  const catalogMaterialPreviewNote = useCallback((materialName: string) => {
    setAssistantNote(buildMaterialPreviewNote(materialName))
  }, [setAssistantNote])

  return {
    quickMaterialPresetSelect,
    catalogMaterialPreview,
    catalogMaterialPreviewNote,
  }
}
