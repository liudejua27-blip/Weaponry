import {
  createViewportDisplayPreferenceKey,
  DEFAULT_VIEWPORT_DISPLAY_PREFERENCES,
  initialViewportDisplayPreferencesState,
  readViewportDisplayPreferences,
  viewportDisplayPreferencesReducer,
  writeViewportDisplayPreferences,
} from './viewportDisplayPreferencesState.js'

class MemoryStorage {
  private readonly values = new Map<string, string>()
  getItem(key: string) { return this.values.get(key) ?? null }
  setItem(key: string, value: string) { this.values.set(key, value) }
}

export function runViewportDisplayPreferencesStateSmoke(): void {
  const projectAKey = createViewportDisplayPreferenceKey('project-a')
  const projectBKey = createViewportDisplayPreferenceKey('project-b')
  assert(projectAKey !== projectBKey && projectAKey?.includes('project-a'), 'viewport preferences must be project-isolated')

  const storage = new MemoryStorage()
  storage.setItem(projectAKey!, '{invalid')
  assert(readViewportDisplayPreferences(storage, projectAKey).activeTool === 'select', 'corrupt local viewport preferences must safely fall back')

  let state = viewportDisplayPreferencesReducer(initialViewportDisplayPreferencesState, {
    type: 'open_context', preferenceKey: projectAKey, preferences: DEFAULT_VIEWPORT_DISPLAY_PREFERENCES,
  })
  state = viewportDisplayPreferencesReducer(state, { type: 'set_active_tool', tool: 'section' })
  state = viewportDisplayPreferencesReducer(state, { type: 'set_show_grid', value: false })
  state = viewportDisplayPreferencesReducer(state, { type: 'set_wireframe', value: true })
  state = viewportDisplayPreferencesReducer(state, { type: 'set_xray', value: true })
  state = viewportDisplayPreferencesReducer(state, { type: 'set_show_connectors', value: true })
  state = viewportDisplayPreferencesReducer(state, { type: 'set_explode_factor', value: 9 })
  state = viewportDisplayPreferencesReducer(state, { type: 'set_section_offset', value: -999 })
  assert(
    state.preferences.activeTool === 'section'
      && !state.preferences.showGrid
      && state.preferences.wireframe
      && state.preferences.xRay
      && state.preferences.showConnectors
      && state.preferences.explodeFactor === 1
      && state.preferences.sectionOffset === -120,
    'viewport toggles, tool whitelist and numeric boundaries must be retained locally',
  )
  writeViewportDisplayPreferences(storage, projectAKey, state.preferences)
  const restored = readViewportDisplayPreferences(storage, projectAKey)
  assert(restored.activeTool === 'section' && restored.explodeFactor === 1, 'saved viewport preferences must round-trip')

  const projectB = readViewportDisplayPreferences(storage, projectBKey)
  assert(projectB.activeTool === 'select' && projectB.showGrid, 'project switching must not leak viewport preferences')
  const fields = Object.keys(state.preferences).sort().join(',')
  assert(
    fields === 'activeTool,explodeFactor,sectionOffset,showConnectors,showGrid,wireframe,xRay',
    'local viewport preferences must not hold asset, version, selection, quality, export, camera or lighting truth',
  )
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
