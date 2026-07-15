import {
  createLegacyModuleGraphWorkspacePreferenceKey,
  DEFAULT_LEGACY_MODULE_GRAPH_WORKSPACE_PREFERENCES,
  initialLegacyModuleGraphWorkspaceState,
  legacyModuleGraphWorkspaceReducer,
  readLegacyModuleGraphWorkspacePreferences,
  writeLegacyModuleGraphWorkspacePreferences,
} from './legacyModuleGraphWorkspaceState.js'

class MemoryStorage {
  private readonly values = new Map<string, string>()
  getItem(key: string) { return this.values.get(key) ?? null }
  setItem(key: string, value: string) { this.values.set(key, value) }
}

export function runLegacyModuleGraphWorkspaceStateSmoke(): void {
  const projectAKey = createLegacyModuleGraphWorkspacePreferenceKey('project-a')
  const projectBKey = createLegacyModuleGraphWorkspacePreferenceKey('project-b')
  assert(projectAKey !== projectBKey && projectAKey?.includes('project-a'), 'legacy workspace preferences must be project-isolated')

  const storage = new MemoryStorage()
  storage.setItem(projectAKey!, '{invalid')
  assert(readLegacyModuleGraphWorkspacePreferences(storage, projectAKey).selectedNodeId === '', 'corrupt legacy preferences must safely fall back')

  let state = legacyModuleGraphWorkspaceReducer(initialLegacyModuleGraphWorkspaceState, {
    type: 'open_context', preferenceKey: projectAKey, preferences: DEFAULT_LEGACY_MODULE_GRAPH_WORKSPACE_PREFERENCES,
  })
  state = legacyModuleGraphWorkspaceReducer(state, { type: 'set_inspector_tab', value: 'connections' })
  state = legacyModuleGraphWorkspaceReducer(state, { type: 'set_transform_space', value: 'local' })
  state = legacyModuleGraphWorkspaceReducer(state, { type: 'set_snap_enabled', value: false })
  state = legacyModuleGraphWorkspaceReducer(state, { type: 'set_measurement_mode', value: 'normal_angle' })
  state = legacyModuleGraphWorkspaceReducer(state, {
    type: 'select_node', nodeId: 'gone-node', moduleId: 'gone-module',
  })
  state = legacyModuleGraphWorkspaceReducer(state, {
    type: 'reconcile_graph',
    rootNodeId: 'root',
    nodes: [
      { nodeId: 'root', moduleId: 'root-module', locked: false },
      { nodeId: 'editable', moduleId: 'editable-module', locked: false },
    ],
  })
  assert(
    state.preferences.inspectorTab === 'connections'
      && state.preferences.transformSpace === 'local'
      && !state.preferences.snapEnabled
      && state.preferences.measurementMode === 'normal_angle'
      && state.preferences.selectedNodeId === 'editable'
      && state.preferences.selectedModuleId === 'editable-module',
    'invalid graph selection must safely reconcile without changing local interaction preferences',
  )
  writeLegacyModuleGraphWorkspacePreferences(storage, projectAKey, state.preferences)
  assert(readLegacyModuleGraphWorkspacePreferences(storage, projectAKey).selectedNodeId === 'editable', 'legacy session must round-trip')
  assert(readLegacyModuleGraphWorkspacePreferences(storage, projectBKey).selectedNodeId === '', 'project switch must not leak a legacy node selection')
  writeLegacyModuleGraphWorkspacePreferences(storage, null, state.preferences)
  assert(
    readLegacyModuleGraphWorkspacePreferences(storage, null).selectedNodeId === '',
    'Agent source must not read or write a legacy ModuleGraph session',
  )

  const fields = Object.keys(state.preferences).sort().join(',')
  assert(
    fields === 'inspectorTab,measurementMode,selectedModuleId,selectedNodeId,snapEnabled,transformSpace',
    'legacy session must not hold Agent asset, Snapshot, quality, export, camera or measurement annotation truth',
  )
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
