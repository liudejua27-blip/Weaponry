import type { AgentBlockoutConceptPreview } from '../../shared/types.js'

export type AgentDirectionConceptPreview = {
  status: 'loading' | 'ready' | 'failed'
  imageDataUrl?: string
}

export type AgentDirectionConceptPreviewState = {
  projectId: string | null
  planId: string | null
  latestRequestId: number
  previews: Readonly<Record<string, AgentDirectionConceptPreview>>
}

export const initialAgentDirectionConceptPreviewState: AgentDirectionConceptPreviewState = {
  projectId: null,
  planId: null,
  latestRequestId: 0,
  previews: {},
}

export type AgentDirectionConceptPreviewAction =
  | { type: 'open_project'; projectId: string | null }
  | { type: 'previews_started'; projectId: string | null; planId: string; requestId: number; directionIds: readonly string[] }
  | { type: 'preview_received'; projectId: string | null; planId: string; requestId: number; preview: AgentBlockoutConceptPreview }
  | { type: 'preview_failed'; projectId: string | null; planId: string; requestId: number; directionId: string }
  | { type: 'clear'; projectId: string | null }

/**
 * Disposable cards for the three current direction images.  This state is
 * intentionally unable to own a GLB, candidate id, asset version, Snapshot,
 * quality result, or export identity.
 */
export function agentDirectionConceptPreviewReducer(
  state: AgentDirectionConceptPreviewState,
  action: AgentDirectionConceptPreviewAction,
): AgentDirectionConceptPreviewState {
  switch (action.type) {
    case 'open_project':
      if (state.projectId === action.projectId) return state
      return { ...initialAgentDirectionConceptPreviewState, projectId: action.projectId, latestRequestId: state.latestRequestId }
    case 'previews_started':
      if (state.projectId !== action.projectId || action.requestId <= state.latestRequestId) return state
      return {
        projectId: action.projectId,
        planId: action.planId,
        latestRequestId: action.requestId,
        previews: Object.fromEntries(action.directionIds.map((directionId) => [directionId, { status: 'loading' as const }])),
      }
    case 'preview_received':
      if (!isCurrent(state, action) || !state.previews[action.preview.direction_id]) return state
      return {
        ...state,
        previews: {
          ...state.previews,
          [action.preview.direction_id]: {
            status: 'ready',
            imageDataUrl: `data:image/png;base64,${action.preview.png_base64}`,
          },
        },
      }
    case 'preview_failed':
      if (!isCurrent(state, action) || !state.previews[action.directionId]) return state
      return { ...state, previews: { ...state.previews, [action.directionId]: { status: 'failed' } } }
    case 'clear':
      return state.projectId === action.projectId
        ? { ...initialAgentDirectionConceptPreviewState, projectId: action.projectId, latestRequestId: state.latestRequestId }
        : state
  }
}

function isCurrent(
  state: AgentDirectionConceptPreviewState,
  action: { projectId: string | null; planId: string; requestId: number },
): boolean {
  return state.projectId === action.projectId
    && state.planId === action.planId
    && state.latestRequestId === action.requestId
}
