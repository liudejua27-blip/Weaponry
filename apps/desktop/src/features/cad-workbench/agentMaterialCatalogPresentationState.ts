import type { AgentMaterialPreset } from '../../shared/types.js'

export type AgentMaterialCatalogContext = {
  projectId: string | null
  assetVersionId: string | null
  domainPackId: string | null
  source: 'agent_asset' | 'blockout' | 'external_glb' | 'none'
}

export type AgentMaterialCatalogPresentationState = AgentMaterialCatalogContext & {
  materialPresets: AgentMaterialPreset[]
  loading: boolean
  catalogMessage: string | null
  latestRequestId: number
}

export const initialAgentMaterialCatalogPresentationState: AgentMaterialCatalogPresentationState = {
  projectId: null,
  assetVersionId: null,
  domainPackId: null,
  source: 'none',
  materialPresets: [],
  loading: false,
  catalogMessage: null,
  latestRequestId: 0,
}

export type AgentMaterialCatalogPresentationAction =
  | { type: 'open_context'; context: AgentMaterialCatalogContext; requestId: number }
  | { type: 'read_started'; context: AgentMaterialCatalogContext; requestId: number }
  | { type: 'read_received'; context: AgentMaterialCatalogContext; requestId: number; materialPresets: AgentMaterialPreset[] }
  | { type: 'read_failed'; context: AgentMaterialCatalogContext; requestId: number; fallbackPresets: AgentMaterialPreset[] }

/**
 * Read-only visual-material catalog presentation. Material-zone selection and
 * preview/confirm writes remain owned by the parent and ActiveDesignSnapshot.
 */
export function agentMaterialCatalogPresentationReducer(
  state: AgentMaterialCatalogPresentationState,
  action: AgentMaterialCatalogPresentationAction,
): AgentMaterialCatalogPresentationState {
  switch (action.type) {
    case 'open_context':
      if (matchesContext(state, action.context)) return state
      return { ...initialAgentMaterialCatalogPresentationState, ...action.context, latestRequestId: action.requestId }
    case 'read_started':
      if (!matchesContext(state, action.context) || action.requestId <= state.latestRequestId) return state
      return { ...state, loading: true, catalogMessage: null, latestRequestId: action.requestId }
    case 'read_received':
      if (!isCurrentRequest(state, action.context, action.requestId)) return state
      if (action.materialPresets.length === 0) {
        return { ...state, materialPresets: [], loading: false, catalogMessage: '当前目录没有可用的视觉材质预设。' }
      }
      return { ...state, materialPresets: action.materialPresets, loading: false, catalogMessage: null }
    case 'read_failed':
      if (!isCurrentRequest(state, action.context, action.requestId)) return state
      return action.fallbackPresets.length > 0
        ? {
          ...state,
          materialPresets: action.fallbackPresets,
          loading: false,
          catalogMessage: '服务目录暂时无法读取，正在使用本机内置视觉预设。',
        }
        : { ...state, materialPresets: [], loading: false, catalogMessage: '视觉材质目录暂时不可用。' }
  }
}

function matchesContext(state: AgentMaterialCatalogPresentationState, context: AgentMaterialCatalogContext): boolean {
  return state.projectId === context.projectId
    && state.assetVersionId === context.assetVersionId
    && state.domainPackId === context.domainPackId
    && state.source === context.source
}

function isCurrentRequest(
  state: AgentMaterialCatalogPresentationState,
  context: AgentMaterialCatalogContext,
  requestId: number,
): boolean {
  return matchesContext(state, context) && state.latestRequestId === requestId
}
