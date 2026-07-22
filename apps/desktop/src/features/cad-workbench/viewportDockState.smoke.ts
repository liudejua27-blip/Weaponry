import {
  initialViewportDockPresentationState,
  isViewportFocused,
  viewportDockPresentationReducer,
} from './viewportDockState.js'

export function runViewportDockStateSmoke(): void {
  let state = viewportDockPresentationReducer(initialViewportDockPresentationState, {
    type: 'open_project', projectId: 'project-a',
  })
  assert(state.projectId === 'project-a' && state.dockState === 'docked', 'opening a project must start in docked mode')

  state = viewportDockPresentationReducer(state, { type: 'open' })
  assert(isViewportFocused(state), 'open must focus the existing viewport frame')
  const unchangedOnSameProject = viewportDockPresentationReducer(state, { type: 'open_project', projectId: 'project-a' })
  assert(unchangedOnSameProject === state, 'refreshing the same project must preserve the user focus choice')

  state = viewportDockPresentationReducer(state, { type: 'toggle' })
  assert(state.dockState === 'docked', 'toggle must return focus to the dock')
  state = viewportDockPresentationReducer(state, { type: 'toggle' })
  assert(state.dockState === 'focus', 'toggle must focus from the dock')

  state = viewportDockPresentationReducer(state, { type: 'escape' })
  assert(state.dockState === 'docked', 'Escape must close focus')
  const repeatClose = viewportDockPresentationReducer(state, { type: 'close' })
  assert(repeatClose === state, 'repeated close must be idempotent')
  const repeatEscape = viewportDockPresentationReducer(state, { type: 'escape' })
  assert(repeatEscape === state, 'repeated Escape must be idempotent')

  state = viewportDockPresentationReducer(state, { type: 'open' })
  state = viewportDockPresentationReducer(state, { type: 'open_project', projectId: 'project-b' })
  assert(
    state.projectId === 'project-b' && state.dockState === 'docked',
    'a project change must forcibly return the viewport to docked mode',
  )

  const fields = Object.keys(state).sort().join(',')
  assert(
    fields === 'dockState,projectId',
    'dock presentation state must not own renderer, canvas, Snapshot, version, selection, material, quality, or export truth',
  )
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
