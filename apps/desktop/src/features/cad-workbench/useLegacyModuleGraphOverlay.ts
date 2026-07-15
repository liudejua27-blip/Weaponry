import { useCallback, useMemo, useReducer } from 'react'
import type { QualityFinding } from '../../shared/types'
import {
  createLegacyModuleGraphOverlayContextKey,
  initialLegacyModuleGraphOverlayState,
  legacyModuleGraphOverlayReducer,
} from './legacyModuleGraphOverlayState.js'

/** Owns transient legacy viewport overlays only; Agent part display remains Snapshot-owned. */
export function useLegacyModuleGraphOverlay() {
  const [legacyModuleGraphOverlayState, dispatch] = useReducer(
    legacyModuleGraphOverlayReducer,
    initialLegacyModuleGraphOverlayState,
  )

  const openLegacyModuleGraphOverlay = useCallback(
    (projectId: string | null, graphId: string | null, defaultHiddenNodeIds: string[]) => {
      dispatch({
        type: 'open_context',
        contextKey: createLegacyModuleGraphOverlayContextKey(projectId, graphId),
        defaultHiddenNodeIds,
      })
    },
    [],
  )
  const reconcileLegacyModuleGraphOverlayNodes = useCallback(
    (nodeIds: string[]) => dispatch({ type: 'reconcile_nodes', nodeIds }),
    [],
  )
  const toggleLegacyHiddenNode = useCallback(
    (nodeId: string) => dispatch({ type: 'toggle_hidden_node', nodeId }),
    [],
  )
  const setLegacyFocusNode = useCallback(
    (nodeId: string | null) => dispatch({ type: 'set_focus_node', nodeId }),
    [],
  )
  const setLegacyQualityOverlay = useCallback(
    (nodeIds: string[], geometryRefs: NonNullable<QualityFinding['geometry_refs']>) => (
      dispatch({ type: 'set_quality_overlay', nodeIds, geometryRefs })
    ),
    [],
  )
  const clearLegacyQualityOverlay = useCallback(
    () => dispatch({ type: 'clear_quality_overlay' }),
    [],
  )
  const recordLegacyThumbnailFailure = useCallback(
    (moduleId: string) => dispatch({ type: 'record_thumbnail_failure', moduleId }),
    [],
  )
  const thumbnailFailures = useMemo(
    () => new Set(legacyModuleGraphOverlayState.thumbnailFailureModuleIds),
    [legacyModuleGraphOverlayState.thumbnailFailureModuleIds],
  )

  return {
    legacyModuleGraphOverlay: legacyModuleGraphOverlayState,
    legacyModuleGraphOverlayContextKey: legacyModuleGraphOverlayState.contextKey,
    thumbnailFailures,
    openLegacyModuleGraphOverlay,
    reconcileLegacyModuleGraphOverlayNodes,
    toggleLegacyHiddenNode,
    setLegacyFocusNode,
    setLegacyQualityOverlay,
    clearLegacyQualityOverlay,
    recordLegacyThumbnailFailure,
  }
}
