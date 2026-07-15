import type { AgentComponentCandidate, AgentStructureSuggestion, AgentStructureSuggestionList, ResolvedSemanticProportionOptions } from '../../shared/types.js'

export type AgentEditAssistPresentationState = {
  projectId: string | null
  assetVersionId: string | null
  selectedPartId: string | null
  componentCandidates: AgentComponentCandidate[]
  structureSuggestions: AgentStructureSuggestion[]
  structureSuggestionUnavailableMessage: string | null
  semanticProportions: ResolvedSemanticProportionOptions | null
  loading: boolean
  latestRequestId: number
}

export const initialAgentEditAssistPresentationState: AgentEditAssistPresentationState = {
  projectId: null,
  assetVersionId: null,
  selectedPartId: null,
  componentCandidates: [],
  structureSuggestions: [],
  structureSuggestionUnavailableMessage: null,
  semanticProportions: null,
  loading: false,
  latestRequestId: 0,
}

export type AgentEditAssistPresentationAction =
  | { type: 'open_context'; projectId: string | null; assetVersionId: string | null; selectedPartId: string | null; requestId: number }
  | { type: 'read_started'; projectId: string; assetVersionId: string; selectedPartId: string; requestId: number }
  | {
    type: 'read_received'
    projectId: string
    assetVersionId: string
    selectedPartId: string
    requestId: number
    componentCandidates: AgentComponentCandidate[]
    structure: AgentStructureSuggestionList
    semanticProportions: ResolvedSemanticProportionOptions | null
  }
  | { type: 'read_failed'; projectId: string; assetVersionId: string; selectedPartId: string; requestId: number }

/**
 * Read-only component-replacement and structure-suggestion presentation.
 * Candidate eligibility and ChangeSet authorization remain server-owned.
 */
export function agentEditAssistPresentationReducer(
  state: AgentEditAssistPresentationState,
  action: AgentEditAssistPresentationAction,
): AgentEditAssistPresentationState {
  switch (action.type) {
    case 'open_context':
      if (
        state.projectId === action.projectId
        && state.assetVersionId === action.assetVersionId
        && state.selectedPartId === action.selectedPartId
      ) return state
      return {
        ...initialAgentEditAssistPresentationState,
        projectId: action.projectId,
        assetVersionId: action.assetVersionId,
        selectedPartId: action.selectedPartId,
        latestRequestId: action.requestId,
      }
    case 'read_started':
      if (!matchesContext(state, action) || action.requestId <= state.latestRequestId) return state
      return { ...state, loading: true, latestRequestId: action.requestId }
    case 'read_received':
      if (!isCurrentRequest(state, action) || action.structure.asset_version_id !== state.assetVersionId) return state
      return {
        ...state,
        componentCandidates: action.componentCandidates.filter((candidate) => (
          candidate.compatibility.target_asset_version_id === state.assetVersionId
          && candidate.compatibility.target_part_id === state.selectedPartId
        )),
        structureSuggestions: (action.structure.suggestions ?? []).filter((suggestion) => (
          suggestion.asset_version_id === state.assetVersionId
          && (suggestion.part_id === state.selectedPartId || suggestion.target_part_id === state.selectedPartId)
        )),
        structureSuggestionUnavailableMessage: action.structure.unavailable_message ?? null,
        semanticProportions: action.semanticProportions?.asset_version_id === state.assetVersionId
          && action.semanticProportions?.part_id === state.selectedPartId
          ? action.semanticProportions
          : null,
        loading: false,
      }
    case 'read_failed':
      return isCurrentRequest(state, action)
        ? {
          ...state,
          componentCandidates: [],
          structureSuggestions: [],
          structureSuggestionUnavailableMessage: '暂时无法读取结构建议；模型没有被修改。',
          semanticProportions: null,
          loading: false,
        }
        : state
  }
}

function matchesContext(
  state: AgentEditAssistPresentationState,
  action: { projectId: string; assetVersionId: string; selectedPartId: string },
): boolean {
  return state.projectId === action.projectId
    && state.assetVersionId === action.assetVersionId
    && state.selectedPartId === action.selectedPartId
}

function isCurrentRequest(
  state: AgentEditAssistPresentationState,
  action: { projectId: string; assetVersionId: string; selectedPartId: string; requestId: number },
): boolean {
  return matchesContext(state, action) && state.latestRequestId === action.requestId
}
