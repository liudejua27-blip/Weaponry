/**
 * F026 transient presentation state for the one persistent viewport frame.
 *
 * This deliberately owns neither the canvas nor any design truth.  The Panel
 * uses `dockState` only to change the layout position of its already-mounted
 * viewport; mounting a second viewport would violate the single-renderer
 * boundary.
 */
export type ViewportDockState = 'docked' | 'focus'

export type ViewportDockPresentationState = {
  projectId: string | null
  dockState: ViewportDockState
}

export const initialViewportDockPresentationState: ViewportDockPresentationState = {
  projectId: null,
  dockState: 'docked',
}

export type ViewportDockPresentationAction =
  | { type: 'open_project'; projectId: string | null }
  | { type: 'open' }
  | { type: 'toggle' }
  | { type: 'close' }
  | { type: 'escape' }

/**
 * Reducer for docked/focus UI placement. It intentionally keeps focus local
 * to the current project and always returns to docked when that project
 * changes, so a focus overlay cannot survive onto another design.
 */
export function viewportDockPresentationReducer(
  state: ViewportDockPresentationState,
  action: ViewportDockPresentationAction,
): ViewportDockPresentationState {
  switch (action.type) {
    case 'open_project':
      return state.projectId === action.projectId
        ? state
        : { projectId: action.projectId, dockState: 'docked' }
    case 'open':
      return state.dockState === 'focus' ? state : { ...state, dockState: 'focus' }
    case 'toggle':
      return { ...state, dockState: state.dockState === 'docked' ? 'focus' : 'docked' }
    case 'close':
    case 'escape':
      return state.dockState === 'docked' ? state : { ...state, dockState: 'docked' }
  }
}

export function isViewportFocused(state: ViewportDockPresentationState): boolean {
  return state.dockState === 'focus'
}
