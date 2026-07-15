import { useCallback, useReducer, useRef } from 'react'
import type { AgentMaterialPreset } from '../../shared/types.js'
import {
  agentMaterialFilterPresentationReducer,
  initialAgentMaterialFilterPresentationState,
  type AgentMaterialFilterContext,
} from './agentMaterialFilterPresentationState.js'

/** Owns only the material-drawer filter controls, scoped to the current design context. */
export function useAgentMaterialFilterPresentation() {
  const [agentMaterialFilterPresentation, dispatch] = useReducer(
    agentMaterialFilterPresentationReducer,
    initialAgentMaterialFilterPresentationState,
  )
  const contextRef = useRef<AgentMaterialFilterContext>({ projectId: null, domainPackId: null, source: 'none' })
  const openAgentMaterialFilterPresentation = useCallback((context: AgentMaterialFilterContext) => {
    if (matchesContext(contextRef.current, context)) return
    contextRef.current = context
    dispatch({ type: 'open_context', context })
  }, [])
  const setMaterialFilterQuery = useCallback((query: string) => dispatch({ type: 'set_query', query }), [])
  const setMaterialFilterCategory = useCallback((category: AgentMaterialPreset['category'] | 'all') => {
    dispatch({ type: 'set_category', category })
  }, [])
  const setMaterialFilterCompatibilityOnly = useCallback((compatibilityOnly: boolean) => {
    dispatch({ type: 'set_compatibility_only', compatibilityOnly })
  }, [])

  return {
    agentMaterialFilterPresentation,
    openAgentMaterialFilterPresentation,
    setMaterialFilterQuery,
    setMaterialFilterCategory,
    setMaterialFilterCompatibilityOnly,
  }
}

function matchesContext(left: AgentMaterialFilterContext, right: AgentMaterialFilterContext): boolean {
  return left.projectId === right.projectId
    && left.domainPackId === right.domainPackId
    && left.source === right.source
}
