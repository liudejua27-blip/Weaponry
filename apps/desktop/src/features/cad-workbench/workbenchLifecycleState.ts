export type WorkbenchDrawerId = 'component' | 'export' | 'quality'

export type WorkbenchDrawerState = {
  openDrawer: WorkbenchDrawerId | null
}

export type WorkbenchDrawerAction =
  | { type: 'open'; drawer: WorkbenchDrawerId }
  | { type: 'close' }

export const initialWorkbenchDrawerState: WorkbenchDrawerState = {
  openDrawer: null,
}

/**
 * Only one workbench drawer may own focus at a time. This state is purely
 * presentational: it never stores a Version, Snapshot or ChangeSet.
 */
export function workbenchDrawerReducer(
  state: WorkbenchDrawerState,
  action: WorkbenchDrawerAction,
): WorkbenchDrawerState {
  switch (action.type) {
    case 'open':
      return state.openDrawer === action.drawer ? state : { openDrawer: action.drawer }
    case 'close':
      return state.openDrawer === null ? state : initialWorkbenchDrawerState
  }
}

export function isCurrentWorkbenchRequest(latestRequestId: number, requestId: number): boolean {
  return requestId === latestRequestId
}
