export type AgentMaterialPreselectionContext = {
  projectId: string | null
  assetVersionId: string | null
  selectedPartId: string | null
  source: 'agent_asset' | 'blockout' | 'external_glb' | 'legacy' | 'none'
}

export type AgentMaterialPreselectionPresentationState = AgentMaterialPreselectionContext & {
  materialId: string
}

export const initialAgentMaterialPreselectionPresentationState: AgentMaterialPreselectionPresentationState = {
  projectId: null,
  assetVersionId: null,
  selectedPartId: null,
  source: 'none',
  materialId: 'mat_graphite',
}

export type AgentMaterialPreselectionPresentationAction =
  | { type: 'open_context'; context: AgentMaterialPreselectionContext }
  | { type: 'select_material'; materialId: string }

/** A visual preselection only; Material Zone and committed material remain server-owned. */
export function agentMaterialPreselectionPresentationReducer(
  state: AgentMaterialPreselectionPresentationState,
  action: AgentMaterialPreselectionPresentationAction,
): AgentMaterialPreselectionPresentationState {
  switch (action.type) {
    case 'open_context':
      return matchesContext(state, action.context)
        ? state
        : { ...initialAgentMaterialPreselectionPresentationState, ...action.context }
    case 'select_material':
      if (!canPreselect(state.source) || state.materialId === action.materialId) return state
      return { ...state, materialId: action.materialId }
  }
}

function canPreselect(source: AgentMaterialPreselectionContext['source']): boolean {
  return source === 'agent_asset' || source === 'blockout'
}

function matchesContext(
  state: AgentMaterialPreselectionPresentationState,
  context: AgentMaterialPreselectionContext,
): boolean {
  return state.projectId === context.projectId
    && state.assetVersionId === context.assetVersionId
    && state.selectedPartId === context.selectedPartId
    && state.source === context.source
}
