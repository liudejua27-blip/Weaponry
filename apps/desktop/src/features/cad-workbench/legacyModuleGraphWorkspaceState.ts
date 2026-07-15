export type LegacyInspectorTab = 'parameters' | 'appearance' | 'connections' | 'inspection'
export type LegacyTransformSpace = 'world' | 'local'
export type LegacyMeasurementMode = 'distance' | 'normal_angle'

export type LegacyModuleGraphWorkspacePreferences = {
  inspectorTab: LegacyInspectorTab
  transformSpace: LegacyTransformSpace
  snapEnabled: boolean
  selectedNodeId: string
  selectedModuleId: string
  measurementMode: LegacyMeasurementMode
}

export type LegacyModuleGraphWorkspaceState = {
  preferenceKey: string | null
  preferences: LegacyModuleGraphWorkspacePreferences
}

export type LegacyModuleGraphNode = {
  nodeId: string
  moduleId: string
  locked: boolean
}

export type LocalStoragePort = Pick<Storage, 'getItem' | 'setItem'>

export const DEFAULT_LEGACY_MODULE_GRAPH_WORKSPACE_PREFERENCES: LegacyModuleGraphWorkspacePreferences = {
  inspectorTab: 'parameters',
  transformSpace: 'world',
  snapEnabled: true,
  selectedNodeId: '',
  selectedModuleId: '',
  measurementMode: 'distance',
}

export const initialLegacyModuleGraphWorkspaceState: LegacyModuleGraphWorkspaceState = {
  preferenceKey: null,
  preferences: DEFAULT_LEGACY_MODULE_GRAPH_WORKSPACE_PREFERENCES,
}

export type LegacyModuleGraphWorkspaceAction =
  | { type: 'open_context'; preferenceKey: string | null; preferences: LegacyModuleGraphWorkspacePreferences }
  | { type: 'set_inspector_tab'; value: LegacyInspectorTab }
  | { type: 'set_transform_space'; value: LegacyTransformSpace }
  | { type: 'set_snap_enabled'; value: boolean }
  | { type: 'select_node'; nodeId: string; moduleId: string }
  | { type: 'set_selected_module'; moduleId: string }
  | { type: 'clear_selection' }
  | { type: 'set_measurement_mode'; value: LegacyMeasurementMode }
  | { type: 'reconcile_graph'; nodes: LegacyModuleGraphNode[]; rootNodeId: string | null }

/** Per-project legacy ModuleGraph presentation only; never an Agent selection or version source. */
export function createLegacyModuleGraphWorkspacePreferenceKey(projectId: string | null): string | null {
  return projectId ? `forgecad.legacy-module-graph-workspace.v1.${projectId}` : null
}

export function readLegacyModuleGraphWorkspacePreferences(
  storage: Pick<LocalStoragePort, 'getItem'>,
  preferenceKey: string | null,
): LegacyModuleGraphWorkspacePreferences {
  if (!preferenceKey) return DEFAULT_LEGACY_MODULE_GRAPH_WORKSPACE_PREFERENCES
  try {
    const raw = storage.getItem(preferenceKey)
    return normalizeLegacyModuleGraphWorkspacePreferences(raw ? JSON.parse(raw) : null)
  } catch {
    return DEFAULT_LEGACY_MODULE_GRAPH_WORKSPACE_PREFERENCES
  }
}

export function writeLegacyModuleGraphWorkspacePreferences(
  storage: Pick<LocalStoragePort, 'setItem'>,
  preferenceKey: string | null,
  preferences: LegacyModuleGraphWorkspacePreferences,
): void {
  if (!preferenceKey) return
  try {
    storage.setItem(preferenceKey, JSON.stringify(preferences))
  } catch {
    // A legacy UI preference must never prevent the read-only compatibility view from opening.
  }
}

export function legacyModuleGraphWorkspaceReducer(
  state: LegacyModuleGraphWorkspaceState,
  action: LegacyModuleGraphWorkspaceAction,
): LegacyModuleGraphWorkspaceState {
  switch (action.type) {
    case 'open_context':
      if (state.preferenceKey === action.preferenceKey && state.preferences === action.preferences) return state
      return { preferenceKey: action.preferenceKey, preferences: action.preferences }
    case 'set_inspector_tab':
      return setPreferences(state, { inspectorTab: action.value })
    case 'set_transform_space':
      return setPreferences(state, { transformSpace: action.value })
    case 'set_snap_enabled':
      return setPreferences(state, { snapEnabled: action.value })
    case 'select_node':
      return setPreferences(state, { selectedNodeId: action.nodeId, selectedModuleId: action.moduleId })
    case 'set_selected_module':
      return setPreferences(state, { selectedModuleId: action.moduleId })
    case 'clear_selection':
      return setPreferences(state, { selectedNodeId: '', selectedModuleId: '' })
    case 'set_measurement_mode':
      return setPreferences(state, { measurementMode: action.value })
    case 'reconcile_graph':
      return reconcileGraphSelection(state, action.nodes, action.rootNodeId)
  }
}

function setPreferences(
  state: LegacyModuleGraphWorkspaceState,
  change: Partial<LegacyModuleGraphWorkspacePreferences>,
): LegacyModuleGraphWorkspaceState {
  return { ...state, preferences: { ...state.preferences, ...change } }
}

function reconcileGraphSelection(
  state: LegacyModuleGraphWorkspaceState,
  nodes: LegacyModuleGraphNode[],
  rootNodeId: string | null,
): LegacyModuleGraphWorkspaceState {
  if (nodes.length === 0) {
    return setPreferences(state, { selectedNodeId: '', selectedModuleId: '' })
  }
  const current = nodes.find((node) => node.nodeId === state.preferences.selectedNodeId)
  const selected = current && current.nodeId !== rootNodeId && !current.locked
    ? current
    : nodes.find((node) => node.nodeId !== rootNodeId && !node.locked) ?? nodes[0]
  if (
    state.preferences.selectedNodeId === selected.nodeId
    && state.preferences.selectedModuleId === selected.moduleId
  ) return state
  return setPreferences(state, { selectedNodeId: selected.nodeId, selectedModuleId: selected.moduleId })
}

function normalizeLegacyModuleGraphWorkspacePreferences(value: unknown): LegacyModuleGraphWorkspacePreferences {
  const candidate = value && typeof value === 'object' ? value as Record<string, unknown> : {}
  return {
    inspectorTab: isInspectorTab(candidate.inspectorTab) ? candidate.inspectorTab : 'parameters',
    transformSpace: candidate.transformSpace === 'local' ? 'local' : 'world',
    snapEnabled: typeof candidate.snapEnabled === 'boolean' ? candidate.snapEnabled : true,
    selectedNodeId: typeof candidate.selectedNodeId === 'string' ? candidate.selectedNodeId : '',
    selectedModuleId: typeof candidate.selectedModuleId === 'string' ? candidate.selectedModuleId : '',
    measurementMode: candidate.measurementMode === 'normal_angle' ? 'normal_angle' : 'distance',
  }
}

function isInspectorTab(value: unknown): value is LegacyInspectorTab {
  return value === 'parameters' || value === 'appearance' || value === 'connections' || value === 'inspection'
}
