import { useMemo } from 'react'

import type { RefreshActiveDesignCallbacks } from './refreshActiveDesignLoader.js'

type UseCadWorkbenchPanelActiveDesignSyncCallbacksInput = {
  startActiveDesignRequest: RefreshActiveDesignCallbacks['startActiveDesignRequest']
  isCurrentActiveDesignRequest: RefreshActiveDesignCallbacks['isCurrentActiveDesignRequest']
  receiveActiveDesignSnapshot: RefreshActiveDesignCallbacks['receiveActiveDesignSnapshot']
  failActiveDesignRequest: RefreshActiveDesignCallbacks['failActiveDesignRequest']
  setCameraView: RefreshActiveDesignCallbacks['setCameraView']
  setLightPreset: RefreshActiveDesignCallbacks['setLightPreset']
  clearAgentAssetWorkspace: RefreshActiveDesignCallbacks['clearAgentAssetWorkspace']
  clearAgentEditAssistPresentation: RefreshActiveDesignCallbacks['clearAgentEditAssistPresentation']
  setAgentCandidateSelectedPartId: RefreshActiveDesignCallbacks['setAgentCandidateSelectedPartId']
  startAgentAssetWorkspaceHydration: RefreshActiveDesignCallbacks['startAgentAssetWorkspaceHydration']
  receiveAgentAssetWorkspaceAsset: RefreshActiveDesignCallbacks['receiveAgentAssetWorkspaceAsset']
  receiveAgentAssetWorkspaceNavigation: RefreshActiveDesignCallbacks['receiveAgentAssetWorkspaceNavigation']
  receiveAgentAssetWorkspaceQuality: RefreshActiveDesignCallbacks['receiveAgentAssetWorkspaceQuality']
  clearAgentAssetWorkspaceQuality: RefreshActiveDesignCallbacks['clearAgentAssetWorkspaceQuality']
  hydrateBlockoutDisplay: RefreshActiveDesignCallbacks['hydrateBlockoutDisplay']
  setBlockoutGlb: RefreshActiveDesignCallbacks['setBlockoutGlb']
  setAssistantNote: RefreshActiveDesignCallbacks['setAssistantNote']
  activeDesignSelectedPartId: RefreshActiveDesignCallbacks['activeDesignSelectedPartId']
}

export function useCadWorkbenchPanelActiveDesignSyncCallbacks(
  {
    startActiveDesignRequest,
    isCurrentActiveDesignRequest,
    receiveActiveDesignSnapshot,
    failActiveDesignRequest,
    setCameraView,
    setLightPreset,
    clearAgentAssetWorkspace,
    clearAgentEditAssistPresentation,
    setAgentCandidateSelectedPartId,
    startAgentAssetWorkspaceHydration,
    receiveAgentAssetWorkspaceAsset,
    receiveAgentAssetWorkspaceNavigation,
    receiveAgentAssetWorkspaceQuality,
    clearAgentAssetWorkspaceQuality,
    hydrateBlockoutDisplay,
    setBlockoutGlb,
    setAssistantNote,
    activeDesignSelectedPartId,
  }: UseCadWorkbenchPanelActiveDesignSyncCallbacksInput,
): RefreshActiveDesignCallbacks {
  return useMemo(() => ({
    startActiveDesignRequest,
    isCurrentActiveDesignRequest,
    receiveActiveDesignSnapshot,
    failActiveDesignRequest,
    setCameraView,
    setLightPreset,
    clearAgentAssetWorkspace,
    clearAgentEditAssistPresentation,
    setAgentCandidateSelectedPartId,
    startAgentAssetWorkspaceHydration,
    receiveAgentAssetWorkspaceAsset,
    receiveAgentAssetWorkspaceNavigation,
    receiveAgentAssetWorkspaceQuality,
    clearAgentAssetWorkspaceQuality,
    hydrateBlockoutDisplay,
    setBlockoutGlb,
    setAssistantNote,
    activeDesignSelectedPartId,
  }), [
    clearAgentAssetWorkspace,
    clearAgentEditAssistPresentation,
    setAssistantNote,
    setAgentCandidateSelectedPartId,
    setBlockoutGlb,
    setCameraView,
    setLightPreset,
    startActiveDesignRequest,
    startAgentAssetWorkspaceHydration,
    isCurrentActiveDesignRequest,
    receiveActiveDesignSnapshot,
    receiveAgentAssetWorkspaceAsset,
    receiveAgentAssetWorkspaceNavigation,
    receiveAgentAssetWorkspaceQuality,
    clearAgentAssetWorkspaceQuality,
    hydrateBlockoutDisplay,
    failActiveDesignRequest,
    activeDesignSelectedPartId,
  ])
}
