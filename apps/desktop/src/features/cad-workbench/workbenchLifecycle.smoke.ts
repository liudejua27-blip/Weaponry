import {
  initialWorkbenchDrawerState,
  isCurrentWorkbenchRequest,
  workbenchDrawerReducer,
} from './workbenchLifecycleState.js'

export function runWorkbenchLifecycleSmoke(): void {
  let drawer = workbenchDrawerReducer(initialWorkbenchDrawerState, { type: 'open', drawer: 'export' })
  assert(drawer.openDrawer === 'export', 'export must become the only open drawer')
  drawer = workbenchDrawerReducer(drawer, { type: 'open', drawer: 'quality' })
  assert(drawer.openDrawer === 'quality', 'opening quality must close export instead of stacking drawers')
  drawer = workbenchDrawerReducer(drawer, { type: 'close' })
  assert(drawer.openDrawer === null, 'close must leave no drawer focus owner')

  assert(isCurrentWorkbenchRequest(7, 7), 'latest request must be current')
  assert(!isCurrentWorkbenchRequest(8, 7), 'older request must become cancelled before hydration')

}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
