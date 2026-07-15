import { useCallback, useReducer, useRef } from 'react'
import type { AgentAssetRenderSet } from '../../shared/types.js'
import {
  agentRenderPresentationReducer,
  initialAgentRenderPresentationState,
} from './agentRenderPresentationState.js'

type RenderContext = { projectId: string | null; assetVersionId: string | null }

/** Coordinates transient concept-image presentation; download and render APIs remain in the parent. */
export function useAgentRenderPresentation() {
  const [agentRenderPresentation, dispatch] = useReducer(
    agentRenderPresentationReducer,
    initialAgentRenderPresentationState,
  )
  const contextRef = useRef<RenderContext>({ projectId: null, assetVersionId: null })
  const requestIdRef = useRef(0)

  const nextRequestId = useCallback(() => {
    requestIdRef.current += 1
    return requestIdRef.current
  }, [])
  const openAgentRenderPresentation = useCallback((projectId: string | null, assetVersionId: string | null) => {
    if (contextRef.current.projectId === projectId && contextRef.current.assetVersionId === assetVersionId) return
    contextRef.current = { projectId, assetVersionId }
    dispatch({ type: 'open_context', projectId, assetVersionId, requestId: nextRequestId() })
  }, [nextRequestId])
  const startAgentRenderRequest = useCallback((projectId: string, assetVersionId: string) => {
    if (contextRef.current.projectId !== projectId || contextRef.current.assetVersionId !== assetVersionId) return null
    const requestId = nextRequestId()
    dispatch({ type: 'render_started', projectId, assetVersionId, requestId })
    return requestId
  }, [nextRequestId])
  const receiveAgentRenderSet = useCallback((
    projectId: string,
    assetVersionId: string,
    requestId: number,
    renderSet: AgentAssetRenderSet,
  ) => {
    if (!isCurrentContextRequest(contextRef.current, projectId, assetVersionId, requestId, requestIdRef.current)) return false
    dispatch({ type: 'render_received', projectId, assetVersionId, requestId, renderSet })
    return true
  }, [])
  const failAgentRenderRequest = useCallback((projectId: string, assetVersionId: string, requestId: number) => {
    if (!isCurrentContextRequest(contextRef.current, projectId, assetVersionId, requestId, requestIdRef.current)) return false
    dispatch({ type: 'render_failed', projectId, assetVersionId, requestId })
    return true
  }, [])
  const startAgentRenderPackageRequest = useCallback((projectId: string, assetVersionId: string, renderSetSha256: string) => {
    if (contextRef.current.projectId !== projectId || contextRef.current.assetVersionId !== assetVersionId) return null
    const requestId = nextRequestId()
    dispatch({ type: 'package_started', projectId, assetVersionId, requestId, renderSetSha256 })
    return requestId
  }, [nextRequestId])
  const finishAgentRenderPackageRequest = useCallback((
    projectId: string,
    assetVersionId: string,
    requestId: number,
    renderSetSha256: string,
  ) => {
    if (!isCurrentContextRequest(contextRef.current, projectId, assetVersionId, requestId, requestIdRef.current)) return false
    dispatch({ type: 'package_finished', projectId, assetVersionId, requestId, renderSetSha256 })
    return true
  }, [])
  const closeAgentRenderPresentation = useCallback(() => {
    dispatch({ type: 'drawer_closed', requestId: nextRequestId() })
  }, [nextRequestId])

  return {
    agentRenderPresentation,
    openAgentRenderPresentation,
    startAgentRenderRequest,
    receiveAgentRenderSet,
    failAgentRenderRequest,
    startAgentRenderPackageRequest,
    finishAgentRenderPackageRequest,
    closeAgentRenderPresentation,
  }
}

function isCurrentContextRequest(
  context: RenderContext,
  projectId: string,
  assetVersionId: string,
  requestId: number,
  latestRequestId: number,
): boolean {
  return context.projectId === projectId
    && context.assetVersionId === assetVersionId
    && latestRequestId === requestId
}
