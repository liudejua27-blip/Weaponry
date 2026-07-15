import {
  MODULE_CATEGORY_LABELS,
  type ComponentDrawerMode,
  type ComponentFilter,
  type ReviewStatus,
} from './ComponentDrawer.js'
import type { ModuleAssetRecord } from '../../shared/types.js'

const MAX_FAVORITES = 120
const MAX_RECENT = 12
const MIN_DRAWER_HEIGHT = 280
const MAX_DRAWER_HEIGHT = 520
const DEFAULT_DRAWER_HEIGHT = 368

export type ComponentLibraryPreferences = {
  componentCategory: ComponentFilter
  componentQuery: string
  reviewStatusFilter: ReviewStatus | ''
  favoriteModuleIds: string[]
  recentModuleIds: string[]
  drawerExpanded: boolean
  drawerHeight: number
  componentDrawerMode: ComponentDrawerMode
}

export type ComponentLibraryPreferencesState = {
  preferenceKey: string | null
  preferences: ComponentLibraryPreferences
}

export type LocalStoragePort = Pick<Storage, 'getItem' | 'setItem'>

export type ComponentLibraryFilterContext = {
  modules: ModuleAssetRecord[]
  installedModuleIds: ReadonlySet<string>
  selectedModuleCategory: ModuleAssetRecord['manifest']['category'] | null
  selectedNodeUnlocked: boolean
  preferences: ComponentLibraryPreferences
}

export const DEFAULT_COMPONENT_LIBRARY_PREFERENCES: ComponentLibraryPreferences = {
  componentCategory: 'all',
  componentQuery: '',
  reviewStatusFilter: '',
  favoriteModuleIds: [],
  recentModuleIds: [],
  drawerExpanded: false,
  drawerHeight: DEFAULT_DRAWER_HEIGHT,
  componentDrawerMode: 'recommended',
}

export const initialComponentLibraryPreferencesState: ComponentLibraryPreferencesState = {
  preferenceKey: null,
  preferences: DEFAULT_COMPONENT_LIBRARY_PREFERENCES,
}

export type ComponentLibraryPreferencesAction =
  | { type: 'open_context'; preferenceKey: string | null; preferences: ComponentLibraryPreferences }
  | { type: 'set_category'; category: ComponentFilter }
  | { type: 'set_query'; query: string }
  | { type: 'set_review_status'; status: ReviewStatus | '' }
  | { type: 'toggle_favorite'; moduleId: string }
  | { type: 'record_recent'; moduleId: string }
  | { type: 'set_drawer_expanded'; expanded: boolean }
  | { type: 'set_drawer_height'; height: number }
  | { type: 'set_drawer_mode'; mode: ComponentDrawerMode }
  | { type: 'toggle_drawer_mode' }

export function createComponentLibraryPreferenceKey(projectId: string | null, packId: string | null): string | null {
  return projectId && packId
    ? `forgecad.component-library.preferences.v2.${projectId}.${packId}`
    : null
}

export function readComponentLibraryPreferences(
  storage: Pick<LocalStoragePort, 'getItem'>,
  preferenceKey: string | null,
): ComponentLibraryPreferences {
  if (!preferenceKey) return DEFAULT_COMPONENT_LIBRARY_PREFERENCES
  try {
    const raw = storage.getItem(preferenceKey)
    return normalizeComponentLibraryPreferences(raw ? JSON.parse(raw) : null)
  } catch {
    return DEFAULT_COMPONENT_LIBRARY_PREFERENCES
  }
}

export function writeComponentLibraryPreferences(
  storage: Pick<LocalStoragePort, 'setItem'>,
  preferenceKey: string | null,
  preferences: ComponentLibraryPreferences,
): void {
  if (!preferenceKey) return
  try {
    storage.setItem(preferenceKey, JSON.stringify(preferences))
  } catch {
    // Local UI preferences must never prevent the workbench from opening.
  }
}

export function componentLibraryPreferencesReducer(
  state: ComponentLibraryPreferencesState,
  action: ComponentLibraryPreferencesAction,
): ComponentLibraryPreferencesState {
  switch (action.type) {
    case 'open_context':
      if (state.preferenceKey === action.preferenceKey && state.preferences === action.preferences) return state
      return { preferenceKey: action.preferenceKey, preferences: action.preferences }
    case 'set_category':
      return setPreferences(state, { componentCategory: action.category })
    case 'set_query':
      return setPreferences(state, { componentQuery: action.query.slice(0, 160) })
    case 'set_review_status':
      return setPreferences(state, { reviewStatusFilter: action.status })
    case 'toggle_favorite': {
      const favorites = state.preferences.favoriteModuleIds
      const next = favorites.includes(action.moduleId)
        ? favorites.filter((moduleId) => moduleId !== action.moduleId)
        : boundedUnique([action.moduleId, ...favorites], MAX_FAVORITES)
      return setPreferences(state, { favoriteModuleIds: next })
    }
    case 'record_recent':
      return setPreferences(state, {
        recentModuleIds: boundedUnique([action.moduleId, ...state.preferences.recentModuleIds], MAX_RECENT),
      })
    case 'set_drawer_expanded':
      return setPreferences(state, { drawerExpanded: action.expanded })
    case 'set_drawer_height':
      return setPreferences(state, { drawerHeight: clampDrawerHeight(action.height) })
    case 'set_drawer_mode':
      return setPreferences(state, { componentDrawerMode: action.mode })
    case 'toggle_drawer_mode':
      return setPreferences(state, {
        componentDrawerMode: state.preferences.componentDrawerMode === 'recommended' ? 'all' : 'recommended',
      })
  }
}

/** Filters real catalog metadata only; it never manufactures review or quality state. */
export function filterComponentLibraryRecords({
  modules,
  installedModuleIds,
  selectedModuleCategory,
  selectedNodeUnlocked,
  preferences,
}: ComponentLibraryFilterContext): ModuleAssetRecord[] {
  const query = preferences.componentQuery.trim().toLowerCase()
  const categoryItems = preferences.componentCategory === 'all'
    ? modules
    : preferences.componentCategory === 'installed'
    ? modules.filter((component) => installedModuleIds.has(component.manifest.module_id))
    : preferences.componentCategory === 'compatible'
    ? modules.filter((component) => selectedNodeUnlocked && component.manifest.category === selectedModuleCategory)
    : preferences.componentCategory === 'favorites'
    ? modules.filter((component) => preferences.favoriteModuleIds.includes(component.manifest.module_id))
    : preferences.componentCategory === 'recent'
    ? modules.filter((component) => preferences.recentModuleIds.includes(component.manifest.module_id))
    : modules.filter((component) => component.manifest.category === preferences.componentCategory)
  return categoryItems.filter((component) => {
    const metadata = component.catalog_metadata
    const haystack = [
      component.manifest.module_id,
      MODULE_CATEGORY_LABELS[component.manifest.category],
      metadata.display_name,
      metadata.description,
      ...(metadata.tags ?? []),
    ].join(' ').toLowerCase()
    return (!query || haystack.includes(query))
      && (!preferences.reviewStatusFilter || metadata.review_status === preferences.reviewStatusFilter)
  })
}

function setPreferences(
  state: ComponentLibraryPreferencesState,
  change: Partial<ComponentLibraryPreferences>,
): ComponentLibraryPreferencesState {
  return { ...state, preferences: { ...state.preferences, ...change } }
}

function normalizeComponentLibraryPreferences(value: unknown): ComponentLibraryPreferences {
  const candidate = value && typeof value === 'object' ? value as Record<string, unknown> : {}
  return {
    componentCategory: isComponentFilter(candidate.componentCategory) ? candidate.componentCategory : 'all',
    componentQuery: typeof candidate.componentQuery === 'string' ? candidate.componentQuery.slice(0, 160) : '',
    reviewStatusFilter: isReviewStatus(candidate.reviewStatusFilter) ? candidate.reviewStatusFilter : '',
    favoriteModuleIds: boundedUnique(candidate.favoriteModuleIds, MAX_FAVORITES),
    recentModuleIds: boundedUnique(candidate.recentModuleIds, MAX_RECENT),
    drawerExpanded: typeof candidate.drawerExpanded === 'boolean' ? candidate.drawerExpanded : false,
    drawerHeight: clampDrawerHeight(candidate.drawerHeight),
    componentDrawerMode: candidate.componentDrawerMode === 'all' ? 'all' : 'recommended',
  }
}

function boundedUnique(value: unknown, limit: number): string[] {
  if (!Array.isArray(value)) return []
  return [...new Set(value.filter((item): item is string => typeof item === 'string' && item.length > 0))].slice(0, limit)
}

function clampDrawerHeight(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.max(MIN_DRAWER_HEIGHT, Math.min(MAX_DRAWER_HEIGHT, value))
    : DEFAULT_DRAWER_HEIGHT
}

function isReviewStatus(value: unknown): value is ReviewStatus | '' {
  return value === '' || value === 'draft' || value === 'pending_review' || value === 'approved' || value === 'restricted'
}

function isComponentFilter(value: unknown): value is ComponentFilter {
  return value === 'all' || value === 'installed' || value === 'compatible' || value === 'favorites' || value === 'recent'
    || (typeof value === 'string' && value in MODULE_CATEGORY_LABELS)
}
