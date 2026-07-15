import { useCallback, useReducer, useRef } from 'react'
import type { AgentComponentCandidate, AgentStructureSuggestionList } from '../../shared/types.js'
import {
  agentEditAssistPresentationReducer,
  initialAgentEditAssistPresentationState,
} from './agentEditAssistPresentationState.js'

type EditAssistContext = { projectId: string | null; assetVersionId: string | null; selectedPartId: string | null }

/** Coordinates transient read results; preview, confirmation, and component writes remain in the parent. */
export function useAgentEditAssistPresentation() {
  const [agentEditAssistPresentation, dispatch] = useReducer(
    agentEditAssistPresentationReducer,
    initialAgentEditAssistPresentationState,
  )
  const contextRef = useRef<EditAssistContext>({ projectId: null, assetVersionId: null, selectedPartId: null })
  const requestIdRef = useRef(0)

  const nextRequestId = useCallback(() => {
    requestIdRef.current += 1
    return requestIdRef.current
  }, [])
  const openAgentEditAssistPresentation = useCallback((
    projectId: string | null,
    assetVersionId: string | null,
    selectedPartId: string | null,
  ) => {
    if (
      contextRef.current.projectId === projectId
      && contextRef.current.assetVersionId === assetVersionId
      && contextRef.current.selectedPartId === selectedPartId
    ) return
    contextRef.current = { projectId, assetVersionId, selectedPartId }
    dispatch({ type: 'open_context', projectId, assetVersionId, selectedPartId, requestId: nextRequestId() })
  }, [nextRequestId])
  const startAgentEditAssistRead = useCallback((projectId: string, assetVersionId: string, selectedPartId: string) => {
    if (!matchesContextRef(contextRef.current, projectId, assetVersionId, selectedPartId)) return null
    const requestId = nextRequestId()
    dispatch({ type: 'read_started', projectId, assetVersionId, selectedPartId, requestId })
    return requestId
  }, [nextRequestId])
  const receiveAgentEditAssistRead = useCallback((
    projectId: string,
    assetVersionId: string,
    selectedPartId: string,
    requestId: number,
    componentCandidates: AgentComponentCandidate[],
    structure: AgentStructureSuggestionList,
  ) => {
    if (!isCurrentContextRequest(contextRef.current, projectId, assetVersionId, selectedPartId, requestId, requestIdRef.current)) return false
    dispatch({ type: 'read_received', projectId, assetVersionId, selectedPartId, requestId, componentCandidates, structure })
    return true
  }, [])
  const failAgentEditAssistRead = useCallback((projectId: string, assetVersionId: string, selectedPartId: string, requestId: number) => {
    if (!isCurrentContextRequest(contextRef.current, projectId, assetVersionId, selectedPartId, requestId, requestIdRef.current)) return false
    dispatch({ type: 'read_failed', projectId, assetVersionId, selectedPartId, requestId })
    return true
  }, [])
  const clearAgentEditAssistPresentation = useCallback(() => {
    if (contextRef.current.projectId === null && contextRef.current.assetVersionId === null && contextRef.current.selectedPartId === null) return
    contextRef.current = { projectId: null, assetVersionId: null, selectedPartId: null }
    dispatch({ type: 'open_context', projectId: null, assetVersionId: null, selectedPartId: null, requestId: nextRequestId() })
  }, [nextRequestId])

  return {
    agentEditAssistPresentation,
    openAgentEditAssistPresentation,
    startAgentEditAssistRead,
    receiveAgentEditAssistRead,
    failAgentEditAssistRead,
    clearAgentEditAssistPresentation,
  }
}

function matchesContextRef(context: EditAssistContext, projectId: string, assetVersionId: string, selectedPartId: string): boolean {
  return context.projectId === projectId
    && context.assetVersionId === assetVersionId
    && context.selectedPartId === selectedPartId
}

function isCurrentContextRequest(
  context: EditAssistContext,
  projectId: string,
  assetVersionId: string,
  selectedPartId: string,
  requestId: number,
  latestRequestId: number,
): boolean {
  return matchesContextRef(context, projectId, assetVersionId, selectedPartId) && requestId === latestRequestId
}
