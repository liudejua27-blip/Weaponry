import { useCallback, useReducer, useRef } from 'react'
import type { AgentMaterialPreset } from '../../shared/types.js'
import {
  agentMaterialCatalogPresentationReducer,
  initialAgentMaterialCatalogPresentationState,
  type AgentMaterialCatalogContext,
} from './agentMaterialCatalogPresentationState.js'

/** Coordinates only transient material catalog reads; all material edits stay in the parent. */
export function useAgentMaterialCatalogPresentation() {
  const [agentMaterialCatalogPresentation, dispatch] = useReducer(
    agentMaterialCatalogPresentationReducer,
    initialAgentMaterialCatalogPresentationState,
  )
  const contextRef = useRef<AgentMaterialCatalogContext>(initialContext())
  const requestIdRef = useRef(0)
  const nextRequestId = useCallback(() => {
    requestIdRef.current += 1
    return requestIdRef.current
  }, [])
  const openAgentMaterialCatalogPresentation = useCallback((context: AgentMaterialCatalogContext) => {
    if (matchesContext(contextRef.current, context)) return
    contextRef.current = context
    dispatch({ type: 'open_context', context, requestId: nextRequestId() })
  }, [nextRequestId])
  const startAgentMaterialCatalogRead = useCallback((context: AgentMaterialCatalogContext) => {
    if (!matchesContext(contextRef.current, context)) return null
    const requestId = nextRequestId()
    dispatch({ type: 'read_started', context, requestId })
    return requestId
  }, [nextRequestId])
  const receiveAgentMaterialCatalog = useCallback((context: AgentMaterialCatalogContext, requestId: number, materialPresets: AgentMaterialPreset[]) => {
    if (!matchesContext(contextRef.current, context) || requestId !== requestIdRef.current) return false
    dispatch({ type: 'read_received', context, requestId, materialPresets })
    return true
  }, [])
  const failAgentMaterialCatalog = useCallback((context: AgentMaterialCatalogContext, requestId: number, fallbackPresets: AgentMaterialPreset[]) => {
    if (!matchesContext(contextRef.current, context) || requestId !== requestIdRef.current) return false
    dispatch({ type: 'read_failed', context, requestId, fallbackPresets })
    return true
  }, [])

  return {
    agentMaterialCatalogPresentation,
    openAgentMaterialCatalogPresentation,
    startAgentMaterialCatalogRead,
    receiveAgentMaterialCatalog,
    failAgentMaterialCatalog,
  }
}

function initialContext(): AgentMaterialCatalogContext {
  return { projectId: null, assetVersionId: null, domainPackId: null, source: 'none' }
}

function matchesContext(left: AgentMaterialCatalogContext, right: AgentMaterialCatalogContext): boolean {
  return left.projectId === right.projectId
    && left.assetVersionId === right.assetVersionId
    && left.domainPackId === right.domainPackId
    && left.source === right.source
}
