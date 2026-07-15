import { useCallback, useEffect, useReducer, useRef } from 'react'
import {
  createViewportDisplayPreferenceKey,
  initialViewportDisplayPreferencesState,
  readViewportDisplayPreferences,
  viewportDisplayPreferencesReducer,
  writeViewportDisplayPreferences,
  type ViewportTool,
} from './viewportDisplayPreferencesState.js'

/** Owns local viewport display choices only; camera and lighting remain Snapshot-owned. */
export function useViewportDisplayPreferences() {
  const [viewportDisplayPreferencesState, dispatch] = useReducer(
    viewportDisplayPreferencesReducer,
    initialViewportDisplayPreferencesState,
  )
  const preferenceKeyRef = useRef<string | null>(null)

  const openViewportDisplayPreferences = useCallback((projectId: string | null) => {
    const preferenceKey = createViewportDisplayPreferenceKey(projectId)
    if (preferenceKeyRef.current === preferenceKey) return
    preferenceKeyRef.current = preferenceKey
    dispatch({
      type: 'open_context',
      preferenceKey,
      preferences: readViewportDisplayPreferences(window.localStorage, preferenceKey),
    })
  }, [])

  useEffect(() => {
    writeViewportDisplayPreferences(
      window.localStorage,
      viewportDisplayPreferencesState.preferenceKey,
      viewportDisplayPreferencesState.preferences,
    )
  }, [viewportDisplayPreferencesState])

  return {
    viewportDisplayPreferences: viewportDisplayPreferencesState.preferences,
    openViewportDisplayPreferences,
    setViewportTool: (tool: ViewportTool) => dispatch({ type: 'set_active_tool', tool }),
    setViewportShowGrid: (value: boolean) => dispatch({ type: 'set_show_grid', value }),
    setViewportWireframe: (value: boolean) => dispatch({ type: 'set_wireframe', value }),
    setViewportXRay: (value: boolean) => dispatch({ type: 'set_xray', value }),
    setViewportShowConnectors: (value: boolean) => dispatch({ type: 'set_show_connectors', value }),
    setViewportExplodeFactor: (value: number) => dispatch({ type: 'set_explode_factor', value }),
    setViewportSectionOffset: (value: number) => dispatch({ type: 'set_section_offset', value }),
  }
}
