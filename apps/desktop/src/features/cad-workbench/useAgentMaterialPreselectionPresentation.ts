import { useCallback, useReducer, useRef } from 'react'
import {
  agentMaterialPreselectionPresentationReducer,
  initialAgentMaterialPreselectionPresentationState,
  type AgentMaterialPreselectionContext,
} from './agentMaterialPreselectionPresentationState.js'

/** Coordinates transient material preselection; the parent retains all preview/confirm writes. */
export function useAgentMaterialPreselectionPresentation() {
  const [agentMaterialPreselectionPresentation, dispatch] = useReducer(
    agentMaterialPreselectionPresentationReducer,
    initialAgentMaterialPreselectionPresentationState,
  )
  const contextRef = useRef<AgentMaterialPreselectionContext>({
    projectId: null, assetVersionId: null, selectedPartId: null, source: 'none',
  })
  const openAgentMaterialPreselectionPresentation = useCallback((context: AgentMaterialPreselectionContext) => {
    if (matchesContext(contextRef.current, context)) return
    contextRef.current = context
    dispatch({ type: 'open_context', context })
  }, [])
  const selectMaterialPreselection = useCallback((materialId: string) => {
    dispatch({ type: 'select_material', materialId })
  }, [])
  return { agentMaterialPreselectionPresentation, openAgentMaterialPreselectionPresentation, selectMaterialPreselection }
}

function matchesContext(left: AgentMaterialPreselectionContext, right: AgentMaterialPreselectionContext): boolean {
  return left.projectId === right.projectId
    && left.assetVersionId === right.assetVersionId
    && left.selectedPartId === right.selectedPartId
    && left.source === right.source
}
