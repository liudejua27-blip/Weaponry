import type { AgentMaterialPreset } from '../../shared/types.js'

export type AgentMaterialFilterContext = {
  projectId: string | null
  domainPackId: string | null
  source: 'agent_asset' | 'blockout' | 'external_glb' | 'legacy' | 'none'
}

export type AgentMaterialFilterPresentationState = AgentMaterialFilterContext & {
  query: string
  category: AgentMaterialPreset['category'] | 'all'
  compatibilityOnly: boolean
}

export const initialAgentMaterialFilterPresentationState: AgentMaterialFilterPresentationState = {
  projectId: null,
  domainPackId: null,
  source: 'none',
  query: '',
  category: 'all',
  compatibilityOnly: true,
}

export type AgentMaterialFilterPresentationAction =
  | { type: 'open_context'; context: AgentMaterialFilterContext }
  | { type: 'set_query'; query: string }
  | { type: 'set_category'; category: AgentMaterialPreset['category'] | 'all' }
  | { type: 'set_compatibility_only'; compatibilityOnly: boolean }

/**
 * Transient drawer filtering only. It never owns the selected material or
 * Material Zone, which remain Snapshot/ChangeSet concerns.
 */
export function agentMaterialFilterPresentationReducer(
  state: AgentMaterialFilterPresentationState,
  action: AgentMaterialFilterPresentationAction,
): AgentMaterialFilterPresentationState {
  switch (action.type) {
    case 'open_context':
      return matchesContext(state, action.context)
        ? state
        : { ...initialAgentMaterialFilterPresentationState, ...action.context }
    case 'set_query':
      return state.query === action.query ? state : { ...state, query: action.query }
    case 'set_category':
      return state.category === action.category ? state : { ...state, category: action.category }
    case 'set_compatibility_only':
      return state.compatibilityOnly === action.compatibilityOnly
        ? state
        : { ...state, compatibilityOnly: action.compatibilityOnly }
  }
}

function matchesContext(state: AgentMaterialFilterPresentationState, context: AgentMaterialFilterContext): boolean {
  return state.projectId === context.projectId
    && state.domainPackId === context.domainPackId
    && state.source === context.source
}
