import { useEffect } from 'react'

type SingleResultProjectAction = {
  type: 'open_project'
  projectId: string | null
}

type ViewportDockProjectAction = {
  type: 'open_project'
  projectId: string | null
}

type UseCadWorkbenchPanelProjectLifecycleSyncInput = {
  conceptProjectId: string | null
  conceptLegacyDetailsEnabled: boolean
  activeDesignSource: string | null
  closeLegacyDetails: () => void
  openProject: (projectId: string) => void
  openConversationProject: (projectId: string | null) => void
  dispatchSingleResultDecision: (action: SingleResultProjectAction) => void
  openBlockoutProject: (projectId: string | null) => void
  openAgentAssetWorkspaceProject: (projectId: string | null) => void
  openViewportDisplayPreferences: (projectId: string | null) => void
  refreshActiveDesign: (projectId: string) => void | Promise<unknown>
  dispatchViewportDock: (action: ViewportDockProjectAction) => void
  resetProjectScopedState: () => void
  resetProjectDrawerState: () => void
}

export function useCadWorkbenchPanelProjectLifecycleSync({
  conceptProjectId,
  conceptLegacyDetailsEnabled,
  activeDesignSource,
  closeLegacyDetails,
  openProject,
  openConversationProject,
  dispatchSingleResultDecision,
  openBlockoutProject,
  openAgentAssetWorkspaceProject,
  openViewportDisplayPreferences,
  refreshActiveDesign,
  dispatchViewportDock,
  resetProjectScopedState,
  resetProjectDrawerState,
}: UseCadWorkbenchPanelProjectLifecycleSyncInput): void {
  useEffect(() => {
    openConversationProject(conceptProjectId)
    dispatchSingleResultDecision({ type: 'open_project', projectId: conceptProjectId })
    openBlockoutProject(conceptProjectId)
    openAgentAssetWorkspaceProject(conceptProjectId)
    openViewportDisplayPreferences(conceptProjectId)
    dispatchViewportDock({ type: 'open_project', projectId: conceptProjectId })
    resetProjectScopedState()
    resetProjectDrawerState()

    if (!conceptProjectId) return

    openProject(conceptProjectId)
    void refreshActiveDesign(conceptProjectId)
  }, [
    conceptProjectId,
    dispatchSingleResultDecision,
    dispatchViewportDock,
    openAgentAssetWorkspaceProject,
    openBlockoutProject,
    openConversationProject,
    openProject,
    openViewportDisplayPreferences,
    refreshActiveDesign,
    resetProjectDrawerState,
    resetProjectScopedState,
  ])

  useEffect(() => {
    if (activeDesignSource === 'agent_asset' && conceptLegacyDetailsEnabled) {
      closeLegacyDetails()
    }
  }, [activeDesignSource, closeLegacyDetails, conceptLegacyDetailsEnabled])
}
