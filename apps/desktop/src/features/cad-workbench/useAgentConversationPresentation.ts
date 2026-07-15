import { useCallback, useReducer, useRef } from 'react'
import type { AgentItem } from '../../shared/types.js'
import {
  agentConversationReducer,
  initialAgentConversationState,
  isCurrentAgentConversationRequest,
  type AgentClarification,
  type AgentTurnPresentation,
} from './agentConversationState.js'

/**
 * Coordinates ephemeral conversation presentation only. API/SSE calls remain
 * in CadWorkbenchPanel, and all durable design truth remains in the server
 * ActiveDesignSnapshot path.
 */
export function useAgentConversationPresentation() {
  const [agentConversationState, dispatch] = useReducer(
    agentConversationReducer,
    initialAgentConversationState,
  )
  const currentProjectIdRef = useRef<string | null>(null)
  const latestRequestIdRef = useRef(0)

  const openConversationProject = useCallback((projectId: string | null) => {
    if (currentProjectIdRef.current === projectId) return
    currentProjectIdRef.current = projectId
    latestRequestIdRef.current += 1
    dispatch({ type: 'open_project', projectId })
  }, [])

  const startAgentConversationRequest = useCallback((projectId: string | null) => {
    const requestId = latestRequestIdRef.current + 1
    latestRequestIdRef.current = requestId
    dispatch({ type: 'request_started', projectId, requestId })
    return { projectId, requestId }
  }, [])

  const isCurrentRequest = useCallback((projectId: string | null, requestId: number) => (
    currentProjectIdRef.current === projectId
    && isCurrentAgentConversationRequest(latestRequestIdRef.current, requestId)
  ), [])

  const receiveAgentTurn = useCallback((
    projectId: string | null,
    requestId: number,
    threadId: string,
    items: AgentItem[],
    presentation: AgentTurnPresentation,
  ) => {
    if (!isCurrentRequest(projectId, requestId)) return false
    dispatch({ type: 'turn_received', projectId, requestId, threadId, items, presentation })
    return true
  }, [isCurrentRequest])

  const receiveAgentClarification = useCallback((
    projectId: string | null,
    requestId: number,
    clarification: AgentClarification,
  ) => {
    if (!isCurrentRequest(projectId, requestId)) return false
    dispatch({ type: 'clarification_received', projectId, requestId, clarification })
    return true
  }, [isCurrentRequest])

  const markAgentKernelUnavailable = useCallback((projectId: string | null, requestId: number) => {
    if (!isCurrentRequest(projectId, requestId)) return false
    dispatch({ type: 'kernel_unavailable', projectId, requestId })
    return true
  }, [isCurrentRequest])

  const setChatInput = useCallback((value: string) => dispatch({ type: 'set_chat_input', value }), [])
  const setAssistantMode = useCallback((value: 'brief' | 'change') => dispatch({ type: 'set_assistant_mode', value }), [])
  const setAssistantNote = useCallback((value: string) => dispatch({ type: 'set_assistant_note', value }), [])

  return {
    agentConversationState,
    openConversationProject,
    startAgentConversationRequest,
    isCurrentAgentConversationRequest: isCurrentRequest,
    receiveAgentTurn,
    receiveAgentClarification,
    markAgentKernelUnavailable,
    setChatInput,
    setAssistantMode,
    setAssistantNote,
  }
}
