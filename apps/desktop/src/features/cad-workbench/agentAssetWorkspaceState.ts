import type { ActiveDesignNavigation, AgentAssetQualityReport, AgentAssetVersion } from '../../shared/types.js'

/**
 * Read-only workspace projection for the Agent asset selected by the current
 * ActiveDesignSnapshot. It is deliberately a cache: it owns no asset head,
 * Snapshot revision, ETag, ChangeSet, quality write, or export identity.
 */
export type AgentAssetWorkspaceState = {
  projectId: string | null
  expectedAssetVersionId: string | null
  assetVersion: AgentAssetVersion | null
  selectedPartId: string | null
  qualityReport: AgentAssetQualityReport | null
  navigation: ActiveDesignNavigation | null
  latestRequestId: number
}

export const initialAgentAssetWorkspaceState: AgentAssetWorkspaceState = {
  projectId: null,
  expectedAssetVersionId: null,
  assetVersion: null,
  selectedPartId: null,
  qualityReport: null,
  navigation: null,
  latestRequestId: 0,
}

export type AgentAssetWorkspaceAction =
  | { type: 'open_project'; projectId: string | null }
  | { type: 'hydrate_started'; projectId: string; requestId: number; assetVersionId: string; selectedPartId: string | null }
  | { type: 'asset_received'; projectId: string; requestId: number; assetVersion: AgentAssetVersion }
  | { type: 'selection_updated'; projectId: string; assetVersionId: string; selectedPartId: string | null }
  | { type: 'quality_received'; projectId: string; requestId: number; qualityReport: AgentAssetQualityReport | null }
  | { type: 'navigation_received'; projectId: string; requestId: number; navigation: ActiveDesignNavigation | null }
  | { type: 'clear_quality'; projectId: string | null }
  | { type: 'clear' }

export function agentAssetWorkspaceReducer(
  state: AgentAssetWorkspaceState,
  action: AgentAssetWorkspaceAction,
): AgentAssetWorkspaceState {
  switch (action.type) {
    case 'open_project':
      if (state.projectId === action.projectId) return state
      return { ...initialAgentAssetWorkspaceState, projectId: action.projectId, latestRequestId: state.latestRequestId }
    case 'hydrate_started':
      if (state.projectId !== action.projectId || action.requestId <= state.latestRequestId) return state
      return {
        ...state,
        expectedAssetVersionId: action.assetVersionId,
        assetVersion: null,
        selectedPartId: action.selectedPartId,
        qualityReport: null,
        navigation: null,
        latestRequestId: action.requestId,
      }
    case 'asset_received':
      if (!isCurrentWorkspaceRequest(state, action) || action.assetVersion.asset_version_id !== state.expectedAssetVersionId) return state
      return { ...state, assetVersion: action.assetVersion }
    case 'selection_updated':
      if (state.projectId !== action.projectId || state.expectedAssetVersionId !== action.assetVersionId) return state
      return state.selectedPartId === action.selectedPartId ? state : { ...state, selectedPartId: action.selectedPartId }
    case 'quality_received':
      if (!isCurrentWorkspaceRequest(state, action)) return state
      if (action.qualityReport && action.qualityReport.asset_version_id !== state.expectedAssetVersionId) return state
      return { ...state, qualityReport: action.qualityReport }
    case 'navigation_received':
      return isCurrentWorkspaceRequest(state, action) ? { ...state, navigation: action.navigation } : state
    case 'clear_quality':
      return state.projectId === action.projectId && state.qualityReport ? { ...state, qualityReport: null } : state
    case 'clear':
      return { ...initialAgentAssetWorkspaceState, projectId: state.projectId, latestRequestId: state.latestRequestId }
  }
}

function isCurrentWorkspaceRequest(
  state: AgentAssetWorkspaceState,
  action: { projectId: string; requestId: number },
): boolean {
  return state.projectId === action.projectId && state.latestRequestId === action.requestId
}
