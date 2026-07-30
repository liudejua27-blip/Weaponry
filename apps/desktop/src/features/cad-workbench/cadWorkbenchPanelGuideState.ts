export type CadWorkbenchPanelGuideState = {
  projectIsEmpty: boolean
  showBeginnerGuide: boolean
  showCompactSidebar: boolean
}

export type CadWorkbenchPanelGuideStateInput = {
  hasProject: boolean
  hasActiveAgentAssetSnapshot: boolean
  hasActiveDesignSnapshot: boolean
  hasBlockoutSegmentation: boolean
  isDesignOperationIdle: boolean
  hasActiveAgentAssetVersion: boolean
  hasActiveDesignProjectMatch: boolean
  singleResultDecisionIdle: boolean
  conceptLoading: boolean
  directionPreviewLoading: boolean
  chatInputTrimmedEmpty: boolean
  showComposerAdvancedActions: boolean
}

export function buildCadWorkbenchPanelGuideState({
  hasProject,
  hasActiveAgentAssetSnapshot,
  hasActiveDesignSnapshot,
  hasBlockoutSegmentation,
  isDesignOperationIdle,
  hasActiveAgentAssetVersion,
  hasActiveDesignProjectMatch,
  singleResultDecisionIdle,
  conceptLoading,
  directionPreviewLoading,
  chatInputTrimmedEmpty,
  showComposerAdvancedActions,
}: CadWorkbenchPanelGuideStateInput): CadWorkbenchPanelGuideState {
  const projectIsEmpty = Boolean(
    hasProject
    && !hasActiveAgentAssetSnapshot
    && !hasActiveDesignSnapshot
    && !hasBlockoutSegmentation
    && hasActiveDesignProjectMatch
    && isDesignOperationIdle,
  )

  const showBeginnerGuide = Boolean(
    hasProject
    && !hasBlockoutSegmentation
    && !hasActiveAgentAssetVersion
    && singleResultDecisionIdle
    && !conceptLoading
    && !directionPreviewLoading
    && chatInputTrimmedEmpty,
  )

  return {
    projectIsEmpty,
    showBeginnerGuide,
    showCompactSidebar: showBeginnerGuide && !showComposerAdvancedActions,
  }
}
