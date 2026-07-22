export type AgentMaterialPreselectionContext = {
  projectId: string | null
  assetVersionId: string | null
  selectedPartId: string | null
  materialZoneId: string | null
  source: 'agent_asset' | 'blockout' | 'external_glb' | 'legacy' | 'none'
}

export type AgentMaterialPreselectionPresentationState = AgentMaterialPreselectionContext & {
  materialId: string
  hasSelection: boolean
}

export const initialAgentMaterialPreselectionPresentationState: AgentMaterialPreselectionPresentationState = {
  projectId: null,
  assetVersionId: null,
  selectedPartId: null,
  materialZoneId: null,
  source: 'none',
  materialId: 'mat_graphite',
  hasSelection: false,
}

export type AgentMaterialPreselectionPresentationAction =
  | { type: 'open_context'; context: AgentMaterialPreselectionContext }
  | { type: 'select_material'; materialId: string }

/**
 * A visual preselection only. The zone id is a reset key projected from the
 * server-owned Snapshot; committed material bindings remain on AgentAssetVersion.
 */
export function agentMaterialPreselectionPresentationReducer(
  state: AgentMaterialPreselectionPresentationState,
  action: AgentMaterialPreselectionPresentationAction,
): AgentMaterialPreselectionPresentationState {
  switch (action.type) {
    case 'open_context':
      return agentMaterialPreselectionMatchesContext(state, action.context)
        ? state
        : { ...initialAgentMaterialPreselectionPresentationState, ...action.context }
    case 'select_material':
      if (!canPreselect(state.source) || (state.hasSelection && state.materialId === action.materialId)) return state
      return { ...state, materialId: action.materialId, hasSelection: true }
  }
}

function canPreselect(source: AgentMaterialPreselectionContext['source']): boolean {
  return source === 'agent_asset' || source === 'blockout'
}

export function agentMaterialPreselectionMatchesContext(
  state: AgentMaterialPreselectionPresentationState,
  context: AgentMaterialPreselectionContext,
): boolean {
  return state.projectId === context.projectId
    && state.assetVersionId === context.assetVersionId
    && state.selectedPartId === context.selectedPartId
    && state.materialZoneId === context.materialZoneId
    && state.source === context.source
}

export function resolveAgentMaterialDisplayId(
  state: AgentMaterialPreselectionPresentationState,
  context: AgentMaterialPreselectionContext,
  committedMaterialId: string | null,
): string {
  return state.hasSelection && agentMaterialPreselectionMatchesContext(state, context)
    ? state.materialId
    : committedMaterialId ?? initialAgentMaterialPreselectionPresentationState.materialId
}
