import { useCallback, useReducer, useRef } from 'react'
import type { AgentBlockoutConceptPreview, MechanicalConceptPlan } from '../../shared/types.js'
import {
  agentDirectionConceptPreviewReducer,
  initialAgentDirectionConceptPreviewState,
} from './agentDirectionConceptPreviewState.js'

/** Owns only disposable image-card context and ignores every late response. */
export function useAgentDirectionConceptPreviews() {
  const [agentDirectionConceptPreviewState, dispatch] = useReducer(
    agentDirectionConceptPreviewReducer,
    initialAgentDirectionConceptPreviewState,
  )
  const projectIdRef = useRef<string | null>(null)
  const requestIdRef = useRef(0)

  const openDirectionConceptPreviewProject = useCallback((projectId: string | null) => {
    if (projectIdRef.current === projectId) return
    projectIdRef.current = projectId
    requestIdRef.current += 1
    dispatch({ type: 'open_project', projectId })
  }, [])

  const startDirectionConceptPreviews = useCallback((projectId: string | null, plan: MechanicalConceptPlan) => {
    const requestId = requestIdRef.current + 1
    requestIdRef.current = requestId
    dispatch({
      type: 'previews_started',
      projectId,
      planId: plan.plan_id,
      requestId,
      directionIds: plan.directions.map((direction) => direction.direction_id),
    })
    return requestId
  }, [])

  const isCurrentDirectionConceptPreviewRequest = useCallback((projectId: string | null, planId: string, requestId: number) => (
    projectIdRef.current === projectId && requestIdRef.current === requestId
  ), [])

  const receiveDirectionConceptPreview = useCallback((projectId: string | null, planId: string, requestId: number, preview: AgentBlockoutConceptPreview) => {
    if (!isCurrentDirectionConceptPreviewRequest(projectId, planId, requestId)) return false
    dispatch({ type: 'preview_received', projectId, planId, requestId, preview })
    return true
  }, [isCurrentDirectionConceptPreviewRequest])

  const failDirectionConceptPreview = useCallback((projectId: string | null, planId: string, requestId: number, directionId: string) => {
    if (!isCurrentDirectionConceptPreviewRequest(projectId, planId, requestId)) return false
    dispatch({ type: 'preview_failed', projectId, planId, requestId, directionId })
    return true
  }, [isCurrentDirectionConceptPreviewRequest])

  const clearDirectionConceptPreviews = useCallback((projectId: string | null) => {
    requestIdRef.current += 1
    dispatch({ type: 'clear', projectId })
  }, [])

  return {
    agentDirectionConceptPreviewState,
    openDirectionConceptPreviewProject,
    startDirectionConceptPreviews,
    receiveDirectionConceptPreview,
    failDirectionConceptPreview,
    clearDirectionConceptPreviews,
  }
}
