import { useCallback, useEffect } from 'react'
import { loadAgentEditAssist } from './agentEditAssistLoader'

type AgentEditAssistApi = Parameters<typeof loadAgentEditAssist>[0]
type AgentEditAssistCallbacks = Parameters<typeof loadAgentEditAssist>[1]

type UseCadWorkbenchPanelEditAssistLifecycleParams = {
  api: AgentEditAssistApi
  conceptProjectId: string | null
  activeAssetVersionId: string | null
  selectedPartId: string | null
  isExternalGlbReference: boolean
  openAgentEditAssistPresentation: (
    projectId: string | null,
    assetVersionId: string | null,
    selectedPartId: string | null,
  ) => void
  startAgentEditAssistRead: AgentEditAssistCallbacks['startAgentEditAssistRead']
  receiveAgentEditAssistRead: AgentEditAssistCallbacks['receiveAgentEditAssistRead']
  failAgentEditAssistRead: AgentEditAssistCallbacks['failAgentEditAssistRead']
}

export function useCadWorkbenchPanelEditAssistLoader({
  api,
  conceptProjectId,
  activeAssetVersionId,
  selectedPartId,
  isExternalGlbReference,
  openAgentEditAssistPresentation,
  startAgentEditAssistRead,
  receiveAgentEditAssistRead,
  failAgentEditAssistRead,
}: UseCadWorkbenchPanelEditAssistLifecycleParams): () => void {
  const loadAgentEditAssistForSelection = useCallback((
    projectId: string,
    assetVersionId: string,
    partId: string,
  ) => loadAgentEditAssist(
    api,
    {
      startAgentEditAssistRead,
      receiveAgentEditAssistRead,
      failAgentEditAssistRead,
    },
    {
      projectId,
      assetVersionId,
      partId,
    },
  ), [
    api,
    failAgentEditAssistRead,
    receiveAgentEditAssistRead,
    startAgentEditAssistRead,
  ])

  const refreshCurrentAgentEditAssist = useCallback(() => {
    if (!conceptProjectId || isExternalGlbReference || !activeAssetVersionId || !selectedPartId) {
      return
    }

    void loadAgentEditAssistForSelection(conceptProjectId, activeAssetVersionId, selectedPartId)
  }, [
    activeAssetVersionId,
    conceptProjectId,
    isExternalGlbReference,
    loadAgentEditAssistForSelection,
    selectedPartId,
  ])

  useEffect(() => {
    if (!conceptProjectId || isExternalGlbReference || !activeAssetVersionId || !selectedPartId) {
      openAgentEditAssistPresentation(null, null, null)
      return
    }

    openAgentEditAssistPresentation(conceptProjectId, activeAssetVersionId, selectedPartId)
    void loadAgentEditAssistForSelection(conceptProjectId, activeAssetVersionId, selectedPartId)
  }, [
    activeAssetVersionId,
    conceptProjectId,
    isExternalGlbReference,
    loadAgentEditAssistForSelection,
    openAgentEditAssistPresentation,
    selectedPartId,
  ])

  return refreshCurrentAgentEditAssist
}
