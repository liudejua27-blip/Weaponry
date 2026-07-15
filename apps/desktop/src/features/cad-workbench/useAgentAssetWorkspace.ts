import { useCallback, useReducer, useRef } from 'react'
import type { ActiveDesignNavigation, AgentAssetQualityReport, AgentAssetVersion } from '../../shared/types.js'
import {
  agentAssetWorkspaceReducer,
  initialAgentAssetWorkspaceState,
} from './agentAssetWorkspaceState.js'

/** Coordinates only the read projection of the Snapshot-selected Agent asset. */
export function useAgentAssetWorkspace() {
  const [agentAssetWorkspace, dispatch] = useReducer(agentAssetWorkspaceReducer, initialAgentAssetWorkspaceState)
  const projectIdRef = useRef<string | null>(null)
  const requestIdRef = useRef(0)

  const openAgentAssetWorkspaceProject = useCallback((projectId: string | null) => {
    if (projectIdRef.current === projectId) return
    projectIdRef.current = projectId
    requestIdRef.current += 1
    dispatch({ type: 'open_project', projectId })
  }, [])
  const startAgentAssetWorkspaceHydration = useCallback((projectId: string, assetVersionId: string, selectedPartId: string | null) => {
    const requestId = requestIdRef.current + 1
    requestIdRef.current = requestId
    dispatch({ type: 'hydrate_started', projectId, requestId, assetVersionId, selectedPartId })
    return requestId
  }, [])
  const isCurrent = useCallback((projectId: string, requestId: number) => (
    projectIdRef.current === projectId && requestIdRef.current === requestId
  ), [])
  const receiveAsset = useCallback((projectId: string, requestId: number, assetVersion: AgentAssetVersion) => {
    if (!isCurrent(projectId, requestId)) return false
    dispatch({ type: 'asset_received', projectId, requestId, assetVersion })
    return true
  }, [isCurrent])
  const projectSelection = useCallback((projectId: string, assetVersionId: string, selectedPartId: string | null) => {
    if (projectIdRef.current !== projectId) return false
    dispatch({ type: 'selection_updated', projectId, assetVersionId, selectedPartId })
    return true
  }, [])
  const receiveQuality = useCallback((projectId: string, requestId: number, qualityReport: AgentAssetQualityReport | null) => {
    if (!isCurrent(projectId, requestId)) return false
    dispatch({ type: 'quality_received', projectId, requestId, qualityReport })
    return true
  }, [isCurrent])
  const receiveNavigation = useCallback((projectId: string, requestId: number, navigation: ActiveDesignNavigation | null) => {
    if (!isCurrent(projectId, requestId)) return false
    dispatch({ type: 'navigation_received', projectId, requestId, navigation })
    return true
  }, [isCurrent])
  const clearQuality = useCallback((projectId: string | null) => dispatch({ type: 'clear_quality', projectId }), [])
  const clearWorkspace = useCallback(() => dispatch({ type: 'clear' }), [])

  return {
    agentAssetWorkspace,
    openAgentAssetWorkspaceProject,
    startAgentAssetWorkspaceHydration,
    isCurrentAgentAssetWorkspaceRequest: isCurrent,
    receiveAgentAssetWorkspaceAsset: receiveAsset,
    projectAgentAssetWorkspaceSelection: projectSelection,
    receiveAgentAssetWorkspaceQuality: receiveQuality,
    receiveAgentAssetWorkspaceNavigation: receiveNavigation,
    clearAgentAssetWorkspaceQuality: clearQuality,
    clearAgentAssetWorkspace: clearWorkspace,
  }
}
