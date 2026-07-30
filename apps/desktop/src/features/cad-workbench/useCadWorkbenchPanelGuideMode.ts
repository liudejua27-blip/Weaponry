import { useMemo } from 'react'

import {
  buildCadWorkbenchPanelGuideState,
  type CadWorkbenchPanelGuideStateInput,
} from './cadWorkbenchPanelGuideState'

export function useCadWorkbenchPanelGuideMode(
  input: CadWorkbenchPanelGuideStateInput,
) {
  return useMemo(() => buildCadWorkbenchPanelGuideState(input), [
    input.hasProject,
    input.hasActiveAgentAssetSnapshot,
    input.hasActiveDesignSnapshot,
    input.hasBlockoutSegmentation,
    input.isDesignOperationIdle,
    input.hasActiveAgentAssetVersion,
    input.hasActiveDesignProjectMatch,
    input.singleResultDecisionIdle,
    input.conceptLoading,
    input.directionPreviewLoading,
    input.chatInputTrimmedEmpty,
    input.showComposerAdvancedActions,
  ])
}
