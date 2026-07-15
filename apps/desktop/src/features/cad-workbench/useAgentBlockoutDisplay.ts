import { useCallback, useReducer, useRef } from 'react'
import type { SegmentAgentBlockoutResponse } from '../../shared/types.js'
import {
  agentBlockoutDisplayReducer,
  initialAgentBlockoutDisplayState,
  type AgentBlockoutGlbKind,
} from './agentBlockoutDisplayState.js'

/** Owns only the current display projection; callers retain all API and write ownership. */
export function useAgentBlockoutDisplay() {
  const [agentBlockoutDisplay, dispatch] = useReducer(agentBlockoutDisplayReducer, initialAgentBlockoutDisplayState)
  const projectIdRef = useRef<string | null>(null)
  const requestIdRef = useRef(0)

  const openBlockoutProject = useCallback((projectId: string | null) => {
    if (projectIdRef.current === projectId) return
    projectIdRef.current = projectId
    requestIdRef.current += 1
    dispatch({ type: 'open_project', projectId })
  }, [])

  const startDirectionPreview = useCallback((projectId: string | null, directionId: string, variationIndex: number) => {
    const requestId = requestIdRef.current + 1
    requestIdRef.current = requestId
    dispatch({ type: 'preview_started', projectId, requestId, directionId, variationIndex })
    return requestId
  }, [])

  const isCurrentDirectionPreview = useCallback((projectId: string | null, requestId: number) => (
    projectIdRef.current === projectId && requestIdRef.current === requestId
  ), [])

  const receiveBlockoutBuild = useCallback((projectId: string | null, requestId: number, glbBase64: string, shapeProgram: Record<string, unknown>) => {
    if (!isCurrentDirectionPreview(projectId, requestId)) return false
    dispatch({ type: 'build_received', projectId, requestId, glbBase64, shapeProgram })
    return true
  }, [isCurrentDirectionPreview])

  const receiveSegmentation = useCallback((projectId: string | null, requestId: number, segmentation: SegmentAgentBlockoutResponse) => {
    if (!isCurrentDirectionPreview(projectId, requestId)) return false
    dispatch({ type: 'segmentation_received', projectId, requestId, segmentation })
    return true
  }, [isCurrentDirectionPreview])

  const failSegmentation = useCallback((projectId: string | null, requestId: number) => {
    if (!isCurrentDirectionPreview(projectId, requestId)) return false
    dispatch({ type: 'segmentation_failed', projectId, requestId })
    return true
  }, [isCurrentDirectionPreview])

  const failDirectionPreview = useCallback((projectId: string | null, requestId: number) => {
    if (!isCurrentDirectionPreview(projectId, requestId)) return false
    dispatch({ type: 'preview_failed', projectId, requestId })
    return true
  }, [isCurrentDirectionPreview])

  const hydrateBlockoutDisplay = useCallback((projectId: string | null, data: {
    glbBase64?: string | null
    glbKind: AgentBlockoutGlbKind | null
    shapeProgram?: Record<string, unknown> | null
    segmentation?: SegmentAgentBlockoutResponse | null
  }) => {
    if (projectIdRef.current !== projectId) return null
    const requestId = requestIdRef.current + 1
    requestIdRef.current = requestId
    dispatch({
      type: 'hydrate', projectId, requestId,
      glbBase64: data.glbBase64 ?? null,
      glbKind: data.glbKind,
      shapeProgram: data.shapeProgram ?? null,
      segmentation: data.segmentation ?? null,
    })
    return requestId
  }, [])

  const setBlockoutGlb = useCallback((projectId: string | null, requestId: number, glbBase64: string | null, glbKind: AgentBlockoutGlbKind | null) => {
    if (projectIdRef.current !== projectId || requestIdRef.current !== requestId) return false
    dispatch({ type: 'set_glb', projectId, requestId, glbBase64, glbKind })
    return true
  }, [])
  const setBlockoutShapeProgram = useCallback((projectId: string | null, shapeProgram: Record<string, unknown> | null) => {
    requestIdRef.current += 1
    dispatch({ type: 'set_shape_program', projectId, shapeProgram })
  }, [])
  const clearBlockoutDisplay = useCallback((projectId: string | null) => {
    requestIdRef.current += 1
    dispatch({ type: 'clear', projectId })
  }, [])

  return {
    agentBlockoutDisplay,
    openBlockoutProject,
    startDirectionPreview,
    isCurrentDirectionPreview,
    receiveBlockoutBuild,
    receiveSegmentation,
    failSegmentation,
    failDirectionPreview,
    hydrateBlockoutDisplay,
    setBlockoutGlb,
    setBlockoutShapeProgram,
    clearBlockoutDisplay,
  }
}
