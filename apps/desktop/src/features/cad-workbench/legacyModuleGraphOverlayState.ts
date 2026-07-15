import type { QualityFinding } from '../../shared/types'

export type LegacyModuleGraphOverlayState = {
  contextKey: string | null
  hiddenNodeIds: string[]
  focusNodeId: string | null
  qualityHighlightNodeIds: string[]
  qualityGeometryRefs: NonNullable<QualityFinding['geometry_refs']>
  thumbnailFailureModuleIds: string[]
}

export type LegacyModuleGraphOverlayAction =
  | { type: 'open_context'; contextKey: string | null; defaultHiddenNodeIds: string[] }
  | { type: 'reconcile_nodes'; nodeIds: string[] }
  | { type: 'toggle_hidden_node'; nodeId: string }
  | { type: 'set_focus_node'; nodeId: string | null }
  | {
    type: 'set_quality_overlay'
    nodeIds: string[]
    geometryRefs: NonNullable<QualityFinding['geometry_refs']>
  }
  | { type: 'clear_quality_overlay' }
  | { type: 'record_thumbnail_failure'; moduleId: string }

const MAX_THUMBNAIL_FAILURES = 64

/**
 * Transient old-graph presentation only. It deliberately has no persistence,
 * Snapshot, version, quality-result, export, or Agent part-display fields.
 */
export const initialLegacyModuleGraphOverlayState: LegacyModuleGraphOverlayState = {
  contextKey: null,
  hiddenNodeIds: [],
  focusNodeId: null,
  qualityHighlightNodeIds: [],
  qualityGeometryRefs: [],
  thumbnailFailureModuleIds: [],
}

export function createLegacyModuleGraphOverlayContextKey(
  projectId: string | null,
  graphId: string | null,
): string | null {
  return projectId && graphId ? `forgecad.legacy-module-graph-overlay.v1.${projectId}.${graphId}` : null
}

export function legacyModuleGraphOverlayReducer(
  state: LegacyModuleGraphOverlayState,
  action: LegacyModuleGraphOverlayAction,
): LegacyModuleGraphOverlayState {
  switch (action.type) {
    case 'open_context':
      if (state.contextKey === action.contextKey) return state
      return {
        contextKey: action.contextKey,
        hiddenNodeIds: action.contextKey ? uniqueIds(action.defaultHiddenNodeIds) : [],
        focusNodeId: null,
        qualityHighlightNodeIds: [],
        qualityGeometryRefs: [],
        thumbnailFailureModuleIds: [],
      }
    case 'reconcile_nodes':
      return state.contextKey ? reconcileNodes(state, action.nodeIds) : state
    case 'toggle_hidden_node':
      if (!state.contextKey || !action.nodeId) return state
      return {
        ...state,
        hiddenNodeIds: state.hiddenNodeIds.includes(action.nodeId)
          ? state.hiddenNodeIds.filter((nodeId) => nodeId !== action.nodeId)
          : [...state.hiddenNodeIds, action.nodeId],
      }
    case 'set_focus_node':
      return state.contextKey ? { ...state, focusNodeId: action.nodeId } : state
    case 'set_quality_overlay':
      return state.contextKey
        ? {
          ...state,
          qualityHighlightNodeIds: uniqueIds(action.nodeIds),
          qualityGeometryRefs: [...action.geometryRefs],
        }
        : state
    case 'clear_quality_overlay':
      return state.contextKey
        ? { ...state, qualityHighlightNodeIds: [], qualityGeometryRefs: [] }
        : state
    case 'record_thumbnail_failure':
      if (!state.contextKey || !action.moduleId || state.thumbnailFailureModuleIds.includes(action.moduleId)) return state
      return {
        ...state,
        thumbnailFailureModuleIds: [...state.thumbnailFailureModuleIds, action.moduleId].slice(-MAX_THUMBNAIL_FAILURES),
      }
  }
}

function reconcileNodes(
  state: LegacyModuleGraphOverlayState,
  nodeIds: string[],
): LegacyModuleGraphOverlayState {
  const knownNodeIds = new Set(uniqueIds(nodeIds))
  const hiddenNodeIds = state.hiddenNodeIds.filter((nodeId) => knownNodeIds.has(nodeId))
  const focusNodeId = state.focusNodeId && knownNodeIds.has(state.focusNodeId) ? state.focusNodeId : null
  const qualityHighlightNodeIds = state.qualityHighlightNodeIds.filter((nodeId) => knownNodeIds.has(nodeId))
  const qualityGeometryRefs = state.qualityGeometryRefs.filter((reference) => knownNodeIds.has(reference.node_id))
  if (
    arraysEqual(hiddenNodeIds, state.hiddenNodeIds)
    && focusNodeId === state.focusNodeId
    && arraysEqual(qualityHighlightNodeIds, state.qualityHighlightNodeIds)
    && arraysEqual(qualityGeometryRefs, state.qualityGeometryRefs)
  ) return state
  return { ...state, hiddenNodeIds, focusNodeId, qualityHighlightNodeIds, qualityGeometryRefs }
}

function uniqueIds(ids: string[]): string[] {
  return [...new Set(ids.filter((id) => Boolean(id)))]
}

function arraysEqual<T>(left: T[], right: T[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index])
}
