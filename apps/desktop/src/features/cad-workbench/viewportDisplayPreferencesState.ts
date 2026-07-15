export type ViewportTool = 'select' | 'move' | 'rotate' | 'scale' | 'orbit' | 'measure' | 'section'

export type ViewportDisplayPreferences = {
  activeTool: ViewportTool
  showGrid: boolean
  wireframe: boolean
  xRay: boolean
  showConnectors: boolean
  explodeFactor: number
  sectionOffset: number
}

export type ViewportDisplayPreferencesState = {
  preferenceKey: string | null
  preferences: ViewportDisplayPreferences
}

export type LocalStoragePort = Pick<Storage, 'getItem' | 'setItem'>

export const DEFAULT_VIEWPORT_DISPLAY_PREFERENCES: ViewportDisplayPreferences = {
  activeTool: 'select',
  showGrid: true,
  wireframe: false,
  xRay: false,
  showConnectors: false,
  explodeFactor: 0,
  sectionOffset: 0,
}

export const initialViewportDisplayPreferencesState: ViewportDisplayPreferencesState = {
  preferenceKey: null,
  preferences: DEFAULT_VIEWPORT_DISPLAY_PREFERENCES,
}

export type ViewportDisplayPreferencesAction =
  | { type: 'open_context'; preferenceKey: string | null; preferences: ViewportDisplayPreferences }
  | { type: 'set_active_tool'; tool: ViewportTool }
  | { type: 'set_show_grid'; value: boolean }
  | { type: 'set_wireframe'; value: boolean }
  | { type: 'set_xray'; value: boolean }
  | { type: 'set_show_connectors'; value: boolean }
  | { type: 'set_explode_factor'; value: number }
  | { type: 'set_section_offset'; value: number }

/** Per-project display state only. It deliberately excludes Snapshot and design facts. */
export function createViewportDisplayPreferenceKey(projectId: string | null): string | null {
  return projectId ? `forgecad.viewport-display.preferences.v1.${projectId}` : null
}

export function readViewportDisplayPreferences(
  storage: Pick<LocalStoragePort, 'getItem'>,
  preferenceKey: string | null,
): ViewportDisplayPreferences {
  if (!preferenceKey) return DEFAULT_VIEWPORT_DISPLAY_PREFERENCES
  try {
    const raw = storage.getItem(preferenceKey)
    return normalizeViewportDisplayPreferences(raw ? JSON.parse(raw) : null)
  } catch {
    return DEFAULT_VIEWPORT_DISPLAY_PREFERENCES
  }
}

export function writeViewportDisplayPreferences(
  storage: Pick<LocalStoragePort, 'setItem'>,
  preferenceKey: string | null,
  preferences: ViewportDisplayPreferences,
): void {
  if (!preferenceKey) return
  try {
    storage.setItem(preferenceKey, JSON.stringify(preferences))
  } catch {
    // Local display choices must never prevent a project from opening.
  }
}

export function viewportDisplayPreferencesReducer(
  state: ViewportDisplayPreferencesState,
  action: ViewportDisplayPreferencesAction,
): ViewportDisplayPreferencesState {
  switch (action.type) {
    case 'open_context':
      if (state.preferenceKey === action.preferenceKey && state.preferences === action.preferences) return state
      return { preferenceKey: action.preferenceKey, preferences: action.preferences }
    case 'set_active_tool':
      return setPreferences(state, { activeTool: action.tool })
    case 'set_show_grid':
      return setPreferences(state, { showGrid: action.value })
    case 'set_wireframe':
      return setPreferences(state, { wireframe: action.value })
    case 'set_xray':
      return setPreferences(state, { xRay: action.value })
    case 'set_show_connectors':
      return setPreferences(state, { showConnectors: action.value })
    case 'set_explode_factor':
      return setPreferences(state, { explodeFactor: clampExplodeFactor(action.value) })
    case 'set_section_offset':
      return setPreferences(state, { sectionOffset: clampSectionOffset(action.value) })
  }
}

function setPreferences(
  state: ViewportDisplayPreferencesState,
  change: Partial<ViewportDisplayPreferences>,
): ViewportDisplayPreferencesState {
  return { ...state, preferences: { ...state.preferences, ...change } }
}

function normalizeViewportDisplayPreferences(value: unknown): ViewportDisplayPreferences {
  const candidate = value && typeof value === 'object' ? value as Record<string, unknown> : {}
  return {
    activeTool: isViewportTool(candidate.activeTool) ? candidate.activeTool : 'select',
    showGrid: typeof candidate.showGrid === 'boolean' ? candidate.showGrid : true,
    wireframe: typeof candidate.wireframe === 'boolean' ? candidate.wireframe : false,
    xRay: typeof candidate.xRay === 'boolean' ? candidate.xRay : false,
    showConnectors: typeof candidate.showConnectors === 'boolean' ? candidate.showConnectors : false,
    explodeFactor: clampExplodeFactor(candidate.explodeFactor),
    sectionOffset: clampSectionOffset(candidate.sectionOffset),
  }
}

function isViewportTool(value: unknown): value is ViewportTool {
  return value === 'select' || value === 'move' || value === 'rotate' || value === 'scale'
    || value === 'orbit' || value === 'measure' || value === 'section'
}

function clampExplodeFactor(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.max(0, Math.min(1, value))
    : 0
}

function clampSectionOffset(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.max(-120, Math.min(120, value))
    : 0
}
