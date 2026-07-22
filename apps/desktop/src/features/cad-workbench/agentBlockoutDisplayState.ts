import type { SegmentAgentBlockoutResponse } from '../../shared/types.js'

export type AgentBlockoutGlbKind =
  | 'compiled_agent_pbr'
  | 'compiled_agent_preview_pbr'
  | 'compiled_agent_production_pbr'
  | 'external_reference'
export type AgentBlockoutGlbPayload = string | ArrayBuffer

export type AgentBlockoutDisplayState = {
  projectId: string | null
  /** Legacy candidate builds are base64; hydrated/derived artifacts stay binary. */
  glbBase64: AgentBlockoutGlbPayload | null
  glbKind: AgentBlockoutGlbKind | null
  shapeProgram: Record<string, unknown> | null
  segmentation: SegmentAgentBlockoutResponse | null
  directionId: string | null
  variationIndex: number
  previewError: 'blockout_failed' | 'segmentation_failed' | null
  directionPreviewLoading: boolean
  latestRequestId: number
}

export const initialAgentBlockoutDisplayState: AgentBlockoutDisplayState = {
  projectId: null,
  glbBase64: null,
  glbKind: null,
  shapeProgram: null,
  segmentation: null,
  directionId: null,
  variationIndex: 0,
  previewError: null,
  directionPreviewLoading: false,
  latestRequestId: 0,
}

export type AgentBlockoutDisplayAction =
  | { type: 'open_project'; projectId: string | null }
  | { type: 'preview_started'; projectId: string | null; requestId: number; directionId: string; variationIndex: number }
  | { type: 'build_received'; projectId: string | null; requestId: number; glbBase64: string; shapeProgram: Record<string, unknown> }
  | { type: 'segmentation_received'; projectId: string | null; requestId: number; segmentation: SegmentAgentBlockoutResponse }
  | { type: 'segmentation_failed'; projectId: string | null; requestId: number }
  | { type: 'preview_failed'; projectId: string | null; requestId: number }
  | { type: 'hydrate'; projectId: string | null; requestId: number; glbBase64: AgentBlockoutGlbPayload | null; glbKind: AgentBlockoutGlbKind | null; shapeProgram: Record<string, unknown> | null; segmentation: SegmentAgentBlockoutResponse | null }
  | { type: 'set_glb'; projectId: string | null; requestId: number; glbBase64: AgentBlockoutGlbPayload | null; glbKind: AgentBlockoutGlbKind | null }
  | { type: 'set_shape_program'; projectId: string | null; requestId: number; shapeProgram: Record<string, unknown> | null }
  | { type: 'clear'; projectId: string | null }

/**
 * This is display-only state for a candidate or the currently hydrated asset
 * projection. It deliberately stores no AgentAssetVersion, Snapshot, ChangeSet,
 * quality, or export identity.
 */
export function agentBlockoutDisplayReducer(
  state: AgentBlockoutDisplayState,
  action: AgentBlockoutDisplayAction,
): AgentBlockoutDisplayState {
  switch (action.type) {
    case 'open_project':
      if (state.projectId === action.projectId) return state
      return { ...initialAgentBlockoutDisplayState, projectId: action.projectId, latestRequestId: state.latestRequestId }
    case 'preview_started':
      if (state.projectId !== action.projectId || action.requestId <= state.latestRequestId) return state
      return {
        ...state,
        glbBase64: null,
        glbKind: null,
        shapeProgram: null,
        segmentation: null,
        directionId: action.directionId,
        variationIndex: action.variationIndex,
        previewError: null,
        directionPreviewLoading: true,
        latestRequestId: action.requestId,
      }
    case 'build_received':
      if (!isCurrent(state, action)) return state
      return { ...state, glbBase64: action.glbBase64, glbKind: 'compiled_agent_pbr', shapeProgram: action.shapeProgram, segmentation: null }
    case 'segmentation_received':
      if (!isCurrent(state, action)) return state
      return { ...state, segmentation: action.segmentation, previewError: null, directionPreviewLoading: false }
    case 'segmentation_failed':
      if (!isCurrent(state, action)) return state
      return { ...state, segmentation: null, previewError: 'segmentation_failed', directionPreviewLoading: false }
    case 'preview_failed':
      if (!isCurrent(state, action)) return state
      return { ...state, previewError: 'blockout_failed', directionPreviewLoading: false }
    case 'hydrate':
      if (state.projectId !== action.projectId || action.requestId <= state.latestRequestId) return state
      return {
        ...state,
        glbBase64: action.glbBase64,
        glbKind: action.glbKind,
        shapeProgram: action.shapeProgram,
        segmentation: action.segmentation,
        directionPreviewLoading: false,
        latestRequestId: action.requestId,
      }
    case 'set_glb':
      return isCurrent(state, action) ? { ...state, glbBase64: action.glbBase64, glbKind: action.glbKind } : state
    case 'set_shape_program':
      return state.projectId === action.projectId && action.requestId > state.latestRequestId
        ? {
            ...state,
            glbBase64: null,
            glbKind: null,
            shapeProgram: action.shapeProgram,
            latestRequestId: action.requestId,
          }
        : state
    case 'clear':
      return state.projectId === action.projectId
        ? {
            ...state,
            glbBase64: null,
            glbKind: null,
            shapeProgram: null,
            segmentation: null,
            directionId: null,
            variationIndex: 0,
            previewError: null,
            directionPreviewLoading: false,
          }
        : state
  }
}

function isCurrent(
  state: AgentBlockoutDisplayState,
  action: { projectId: string | null; requestId: number },
): boolean {
  return state.projectId === action.projectId && state.latestRequestId === action.requestId
}
