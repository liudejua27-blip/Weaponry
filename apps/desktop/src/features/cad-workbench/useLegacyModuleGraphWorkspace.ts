import { useCallback, useEffect, useReducer, useRef } from 'react'
import {
  createLegacyModuleGraphWorkspacePreferenceKey,
  initialLegacyModuleGraphWorkspaceState,
  legacyModuleGraphWorkspaceReducer,
  readLegacyModuleGraphWorkspacePreferences,
  writeLegacyModuleGraphWorkspacePreferences,
  type LegacyInspectorTab,
  type LegacyMeasurementMode,
  type LegacyModuleGraphNode,
  type LegacyTransformSpace,
} from './legacyModuleGraphWorkspaceState.js'

/** Owns local legacy ModuleGraph presentation; Agent selections remain Snapshot-owned. */
export function useLegacyModuleGraphWorkspace() {
  const [legacyModuleGraphWorkspaceState, dispatch] = useReducer(
    legacyModuleGraphWorkspaceReducer,
    initialLegacyModuleGraphWorkspaceState,
  )
  const preferenceKeyRef = useRef<string | null>(null)

  const openLegacyModuleGraphWorkspace = useCallback((projectId: string | null) => {
    const preferenceKey = createLegacyModuleGraphWorkspacePreferenceKey(projectId)
    if (preferenceKeyRef.current === preferenceKey) return
    preferenceKeyRef.current = preferenceKey
    dispatch({
      type: 'open_context',
      preferenceKey,
      preferences: readLegacyModuleGraphWorkspacePreferences(window.localStorage, preferenceKey),
    })
  }, [])

  useEffect(() => {
    writeLegacyModuleGraphWorkspacePreferences(
      window.localStorage,
      legacyModuleGraphWorkspaceState.preferenceKey,
      legacyModuleGraphWorkspaceState.preferences,
    )
  }, [legacyModuleGraphWorkspaceState])

  const setLegacyInspectorTab = useCallback(
    (value: LegacyInspectorTab) => dispatch({ type: 'set_inspector_tab', value }),
    [],
  )
  const setLegacyTransformSpace = useCallback(
    (value: LegacyTransformSpace) => dispatch({ type: 'set_transform_space', value }),
    [],
  )
  const setLegacySnapEnabled = useCallback(
    (value: boolean) => dispatch({ type: 'set_snap_enabled', value }),
    [],
  )
  const selectLegacyModuleGraphNode = useCallback(
    (nodeId: string, moduleId: string) => dispatch({ type: 'select_node', nodeId, moduleId }),
    [],
  )
  const setLegacySelectedModule = useCallback(
    (moduleId: string) => dispatch({ type: 'set_selected_module', moduleId }),
    [],
  )
  const clearLegacyModuleGraphSelection = useCallback(
    () => dispatch({ type: 'clear_selection' }),
    [],
  )
  const setLegacyMeasurementMode = useCallback(
    (value: LegacyMeasurementMode) => dispatch({ type: 'set_measurement_mode', value }),
    [],
  )
  const reconcileLegacyModuleGraphSelection = useCallback(
    (nodes: LegacyModuleGraphNode[], rootNodeId: string | null) => dispatch({ type: 'reconcile_graph', nodes, rootNodeId }),
    [],
  )

  return {
    legacyModuleGraphWorkspace: legacyModuleGraphWorkspaceState.preferences,
    legacyModuleGraphWorkspacePreferenceKey: legacyModuleGraphWorkspaceState.preferenceKey,
    openLegacyModuleGraphWorkspace,
    setLegacyInspectorTab,
    setLegacyTransformSpace,
    setLegacySnapEnabled,
    selectLegacyModuleGraphNode,
    setLegacySelectedModule,
    clearLegacyModuleGraphSelection,
    setLegacyMeasurementMode,
    reconcileLegacyModuleGraphSelection,
  }
}
