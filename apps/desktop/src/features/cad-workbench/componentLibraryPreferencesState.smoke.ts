import type { ModuleAssetRecord } from '../../shared/types.js'
import {
  componentLibraryPreferencesReducer,
  createComponentLibraryPreferenceKey,
  DEFAULT_COMPONENT_LIBRARY_PREFERENCES,
  filterComponentLibraryRecords,
  initialComponentLibraryPreferencesState,
  readComponentLibraryPreferences,
  writeComponentLibraryPreferences,
} from './componentLibraryPreferencesState.js'

const module = (id: string, category: 'core_shell' | 'front_shell', status: 'approved' | 'pending_review', tags: string[]): ModuleAssetRecord => ({
  manifest: { module_id: id, category },
  catalog_metadata: { display_name: id, description: `${id} description`, tags, review_status: status },
} as ModuleAssetRecord)

class MemoryStorage {
  private readonly values = new Map<string, string>()
  getItem(key: string) { return this.values.get(key) ?? null }
  setItem(key: string, value: string) { this.values.set(key, value) }
}

export function runComponentLibraryPreferencesStateSmoke(): void {
  const projectAKey = createComponentLibraryPreferenceKey('project-a', 'pack_vehicle_concept')
  const projectBKey = createComponentLibraryPreferenceKey('project-b', 'pack_vehicle_concept')
  assert(projectAKey !== projectBKey && projectAKey?.includes('project-a'), 'preferences must be isolated by both project and domain pack')

  const storage = new MemoryStorage()
  storage.setItem(projectAKey!, '{invalid')
  assert(readComponentLibraryPreferences(storage, projectAKey).favoriteModuleIds.length === 0, 'corrupt local preferences must safely fall back')

  let state = componentLibraryPreferencesReducer(initialComponentLibraryPreferencesState, {
    type: 'open_context', preferenceKey: projectAKey, preferences: DEFAULT_COMPONENT_LIBRARY_PREFERENCES,
  })
  state = componentLibraryPreferencesReducer(state, { type: 'toggle_favorite', moduleId: 'module-a' })
  state = componentLibraryPreferencesReducer(state, { type: 'toggle_favorite', moduleId: 'module-a' })
  assert(state.preferences.favoriteModuleIds.length === 0, 'favorite toggling must remain deduplicated')
  state = componentLibraryPreferencesReducer(state, { type: 'record_recent', moduleId: 'module-a' })
  state = componentLibraryPreferencesReducer(state, { type: 'record_recent', moduleId: 'module-a' })
  state = componentLibraryPreferencesReducer(state, { type: 'set_drawer_height', height: 999 })
  assert(state.preferences.recentModuleIds.join() === 'module-a' && state.preferences.drawerHeight === 520, 'recent list and drawer height must be bounded')
  writeComponentLibraryPreferences(storage, projectAKey, state.preferences)
  assert(readComponentLibraryPreferences(storage, projectAKey).recentModuleIds.join() === 'module-a', 'saved local preferences must round-trip')

  const filtered = filterComponentLibraryRecords({
    modules: [module('module-a', 'core_shell', 'approved', ['armor']), module('module-b', 'front_shell', 'pending_review', ['sensor'])],
    installedModuleIds: new Set(['module-a']),
    selectedModuleCategory: 'core_shell',
    selectedNodeUnlocked: true,
    preferences: { ...state.preferences, componentCategory: 'installed', reviewStatusFilter: 'approved', componentQuery: 'armor' },
  })
  assert(filtered.map((item) => item.manifest.module_id).join() === 'module-a', 'catalog, status and keyword filters must compose over real metadata')
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
