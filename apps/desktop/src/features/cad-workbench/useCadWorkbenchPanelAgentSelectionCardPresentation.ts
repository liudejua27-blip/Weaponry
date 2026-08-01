import { useMemo } from 'react'
import type { AgentSelectionCardProps } from './AgentSelectionCard'

type UseCadWorkbenchPanelAgentSelectionCardPresentationInput = {
  agentBlockoutSegmentation: AgentSelectionCardProps['segmentation'] | null
} & Omit<AgentSelectionCardProps, 'segmentation'>

export function useCadWorkbenchPanelAgentSelectionCardPresentation(
  input: UseCadWorkbenchPanelAgentSelectionCardPresentationInput,
): AgentSelectionCardProps | null {
  const {
    agentBlockoutSegmentation,
    agentAssetVersion,
    activeAgentAssetVersion,
    selectedPart,
    selectedPartId,
    partDisplay,
    isSelectedPartLocked,
    isExternalGlbReference,
    isSnapshotActionPending,
    agentAssetChangeSet,
    agentComponentCandidates,
    agentStructureSuggestions,
    structureSuggestionUnavailableMessage,
    semanticProportions,
    editAssistLoading,
    blockoutPreviewPresentation,
    onSelectPart,
    onPreviewEdit,
    onSaveSelectedComponent,
    onReplaceComponent,
    onPreviewStructureSuggestion,
    onSetPartDisplay,
    onInspectAsset,
    onRejectChange,
    onConfirmChange,
    onOpenSurfaceAdornment,
    surfaceAdornmentDisabled,
    surfaceAdornmentDetail,
  } = input
  const visibleAgentAssetVersion = agentAssetVersion ?? activeAgentAssetVersion
  // A restart restores the Rust-owned active asset/Snapshot before it restores
  // the transient blockout response. Project the persisted parts back into the
  // selection surface so the UI does not hide durable editing affordances.
  const visibleSegmentation = agentBlockoutSegmentation ?? (visibleAgentAssetVersion ? {
    artifact_id: visibleAgentAssetVersion.artifact_id,
    plan_id: visibleAgentAssetVersion.plan_id,
    direction_id: visibleAgentAssetVersion.direction_id,
    domain_pack_id: visibleAgentAssetVersion.domain_pack_id,
    parts: visibleAgentAssetVersion.parts,
    assembly_graph: visibleAgentAssetVersion.assembly_graph,
  } : null)

  return useMemo(
    () => visibleSegmentation
      ? {
        segmentation: visibleSegmentation,
        agentAssetVersion: visibleAgentAssetVersion,
        activeAgentAssetVersion,
        selectedPart,
        selectedPartId,
        partDisplay,
        isSelectedPartLocked,
        isExternalGlbReference,
        isSnapshotActionPending,
        agentAssetChangeSet,
        agentComponentCandidates,
        agentStructureSuggestions,
        structureSuggestionUnavailableMessage,
        semanticProportions,
        editAssistLoading,
        blockoutPreviewPresentation,
        onSelectPart,
        onPreviewEdit,
        onSaveSelectedComponent,
        onReplaceComponent,
        onPreviewStructureSuggestion,
        onSetPartDisplay,
        onInspectAsset,
        onRejectChange,
        onConfirmChange,
        onOpenSurfaceAdornment,
        surfaceAdornmentDisabled,
        surfaceAdornmentDetail,
      }
      : null,
    [
      visibleSegmentation,
      visibleAgentAssetVersion,
      activeAgentAssetVersion,
      selectedPart,
      selectedPartId,
      partDisplay,
      isSelectedPartLocked,
      isExternalGlbReference,
      isSnapshotActionPending,
      agentAssetChangeSet,
      agentComponentCandidates,
      agentStructureSuggestions,
      structureSuggestionUnavailableMessage,
      semanticProportions,
      editAssistLoading,
      blockoutPreviewPresentation,
      onSelectPart,
      onPreviewEdit,
      onSaveSelectedComponent,
      onReplaceComponent,
      onPreviewStructureSuggestion,
      onSetPartDisplay,
      onInspectAsset,
      onRejectChange,
      onConfirmChange,
      onOpenSurfaceAdornment,
      surfaceAdornmentDisabled,
      surfaceAdornmentDetail,
    ],
  )
}
