import { useCallback, useEffect, useReducer, useRef } from 'react'
import type { ComponentFilter, ReviewStatus } from './ComponentDrawer.js'
import {
  componentLibraryPreferencesReducer,
  createComponentLibraryPreferenceKey,
  initialComponentLibraryPreferencesState,
  readComponentLibraryPreferences,
  writeComponentLibraryPreferences,
} from './componentLibraryPreferencesState.js'

/** Owns only per-project, per-pack local browser preferences for the component drawer. */
export function useComponentLibraryPreferences() {
  const [componentLibraryPreferencesState, dispatch] = useReducer(
    componentLibraryPreferencesReducer,
    initialComponentLibraryPreferencesState,
  )
  const preferenceKeyRef = useRef<string | null>(null)

  const openComponentLibraryPreferences = useCallback((projectId: string | null, packId: string | null) => {
    const preferenceKey = createComponentLibraryPreferenceKey(projectId, packId)
    if (preferenceKeyRef.current === preferenceKey) return
    preferenceKeyRef.current = preferenceKey
    dispatch({
      type: 'open_context',
      preferenceKey,
      preferences: readComponentLibraryPreferences(window.localStorage, preferenceKey),
    })
  }, [])

  useEffect(() => {
    writeComponentLibraryPreferences(
      window.localStorage,
      componentLibraryPreferencesState.preferenceKey,
      componentLibraryPreferencesState.preferences,
    )
  }, [componentLibraryPreferencesState])

  return {
    componentLibraryPreferences: componentLibraryPreferencesState.preferences,
    openComponentLibraryPreferences,
    setComponentCategory: (category: ComponentFilter) => dispatch({ type: 'set_category', category }),
    setComponentQuery: (query: string) => dispatch({ type: 'set_query', query }),
    setReviewStatusFilter: (status: ReviewStatus | '') => dispatch({ type: 'set_review_status', status }),
    toggleLibraryFavorite: (moduleId: string) => dispatch({ type: 'toggle_favorite', moduleId }),
    recordRecentLibraryModule: (moduleId: string) => dispatch({ type: 'record_recent', moduleId }),
    setDrawerExpanded: (expanded: boolean) => dispatch({ type: 'set_drawer_expanded', expanded }),
    setDrawerHeight: (height: number) => dispatch({ type: 'set_drawer_height', height }),
    setComponentDrawerMode: (mode: 'recommended' | 'all') => dispatch({ type: 'set_drawer_mode', mode }),
    toggleComponentDrawerMode: () => dispatch({ type: 'toggle_drawer_mode' }),
  }
}
