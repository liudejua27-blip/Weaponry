import type { AgentAssetRenderSet } from '../../shared/types.js'

export type AgentRenderPresentationState = {
  projectId: string | null
  assetVersionId: string | null
  renderSet: AgentAssetRenderSet | null
  renderLoading: boolean
  renderPackageLoading: boolean
  latestRequestId: number
}

export const initialAgentRenderPresentationState: AgentRenderPresentationState = {
  projectId: null,
  assetVersionId: null,
  renderSet: null,
  renderLoading: false,
  renderPackageLoading: false,
  latestRequestId: 0,
}

export type AgentRenderPresentationAction =
  | { type: 'open_context'; projectId: string | null; assetVersionId: string | null; requestId: number }
  | { type: 'render_started'; projectId: string; assetVersionId: string; requestId: number }
  | { type: 'render_received'; projectId: string; assetVersionId: string; requestId: number; renderSet: AgentAssetRenderSet }
  | { type: 'render_failed'; projectId: string; assetVersionId: string; requestId: number }
  | { type: 'package_started'; projectId: string; assetVersionId: string; requestId: number; renderSetSha256: string }
  | { type: 'package_finished'; projectId: string; assetVersionId: string; requestId: number; renderSetSha256: string }
  | { type: 'drawer_closed'; requestId: number }

/**
 * Current Agent render presentation only. It is an in-memory view cache, not
 * an export record, Snapshot field, version head, or image object store.
 */
export function agentRenderPresentationReducer(
  state: AgentRenderPresentationState,
  action: AgentRenderPresentationAction,
): AgentRenderPresentationState {
  switch (action.type) {
    case 'open_context':
      if (state.projectId === action.projectId && state.assetVersionId === action.assetVersionId) return state
      return {
        ...initialAgentRenderPresentationState,
        projectId: action.projectId,
        assetVersionId: action.assetVersionId,
        latestRequestId: action.requestId,
      }
    case 'render_started':
      if (!matchesContext(state, action) || action.requestId <= state.latestRequestId) return state
      return { ...state, renderLoading: true, renderPackageLoading: false, latestRequestId: action.requestId }
    case 'render_received':
      if (!isCurrentRequest(state, action) || action.renderSet.asset_version_id !== state.assetVersionId) return state
      return {
        ...state,
        renderSet: action.renderSet,
        renderLoading: false,
        renderPackageLoading: false,
      }
    case 'render_failed':
      return isCurrentRequest(state, action) ? { ...state, renderLoading: false } : state
    case 'package_started':
      if (
        !matchesContext(state, action)
        || action.requestId <= state.latestRequestId
        || state.renderSet?.render_set_sha256 !== action.renderSetSha256
      ) return state
      return { ...state, renderPackageLoading: true, latestRequestId: action.requestId }
    case 'package_finished':
      if (
        !isCurrentRequest(state, action)
        || state.renderSet?.render_set_sha256 !== action.renderSetSha256
      ) return state
      return { ...state, renderPackageLoading: false }
    case 'drawer_closed':
      if (action.requestId <= state.latestRequestId) return state
      return { ...state, renderLoading: false, renderPackageLoading: false, latestRequestId: action.requestId }
  }
}

function matchesContext(
  state: AgentRenderPresentationState,
  action: { projectId: string; assetVersionId: string },
): boolean {
  return state.projectId === action.projectId && state.assetVersionId === action.assetVersionId
}

function isCurrentRequest(
  state: AgentRenderPresentationState,
  action: { projectId: string; assetVersionId: string; requestId: number },
): boolean {
  return matchesContext(state, action) && state.latestRequestId === action.requestId
}
