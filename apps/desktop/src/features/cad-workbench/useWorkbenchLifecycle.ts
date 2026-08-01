import { useCallback, useReducer, useRef } from 'react'
import {
  mapActiveDesignError,
  type ActiveDesignApiResponse,
  type ActiveDesignErrorState,
} from '../../shared/api/forgeApi.js'
import type { ActiveDesignSnapshot } from '../../shared/types.js'
import {
  activeDesignMachineReducer,
  initialActiveDesignMachineState,
  type ActiveDesignOperation,
} from './activeDesignMachine.js'
import {
  initialWorkbenchDrawerState,
  isCurrentWorkbenchRequest,
  workbenchDrawerReducer,
  type WorkbenchDrawerId,
} from './workbenchLifecycleState.js'

export type { WorkbenchDrawerId } from './workbenchLifecycleState.js'

export function mapWorkbenchLifecycleError(caught: unknown): ActiveDesignErrorState {
  return mapActiveDesignError(caught)
}

/**
 * Owns transient workbench lifecycle coordination only. The caller still owns
 * API calls and all hydration side effects; this hook never creates a second
 * design truth beside ActiveDesignSnapshot.
 */
export function useWorkbenchLifecycle() {
  const [activeDesignState, dispatchActiveDesign] = useReducer(
    activeDesignMachineReducer,
    initialActiveDesignMachineState,
  )
  const latestRequestIdRef = useRef(0)
  // `openProject` and the first refresh can be dispatched in the same React
  // event. Keep the project identity synchronously available so that the
  // request-start action is not rejected against the previous render's
  // null project state.
  const projectIdRef = useRef<string | null>(null)
  const [drawerState, dispatchDrawer] = useReducer(
    workbenchDrawerReducer,
    initialWorkbenchDrawerState,
  )
  const drawerFocusRef = useRef<HTMLElement | null>(null)
  const drawerTriggerRef = useRef<HTMLElement | null>(null)

  const invalidateActiveDesignRequests = useCallback(() => {
    const requestId = latestRequestIdRef.current
    if (requestId > 0) {
      dispatchActiveDesign({ type: 'request_cancelled', requestId })
    }
    latestRequestIdRef.current += 1
  }, [])

  const openProject = useCallback((projectId: string) => {
    projectIdRef.current = projectId
    invalidateActiveDesignRequests()
    dispatchActiveDesign({ type: 'open_project', projectId })
  }, [invalidateActiveDesignRequests])

  const startActiveDesignRequest = useCallback((operation: Exclude<ActiveDesignOperation, 'idle'>) => {
    if (!projectIdRef.current) return 0
    const requestId = latestRequestIdRef.current + 1
    latestRequestIdRef.current = requestId
    dispatchActiveDesign({ type: 'request_started', requestId, operation })
    return requestId
  }, [])

  const isCurrentActiveDesignRequest = useCallback(
    (requestId: number) => isCurrentWorkbenchRequest(latestRequestIdRef.current, requestId),
    [],
  )

  const receiveActiveDesignSnapshot = useCallback((
    projectId: string,
    requestId: number,
    response: ActiveDesignApiResponse<ActiveDesignSnapshot>,
  ) => {
    if (!isCurrentWorkbenchRequest(latestRequestIdRef.current, requestId)) return false
    dispatchActiveDesign({ type: 'snapshot_received', projectId, requestId, response })
    return true
  }, [])

  const failActiveDesignRequest = useCallback((requestId: number, caught: unknown): ActiveDesignErrorState | null => {
    if (!isCurrentWorkbenchRequest(latestRequestIdRef.current, requestId)) return null
    const error = mapWorkbenchLifecycleError(caught)
    dispatchActiveDesign({ type: 'request_failed', requestId, error })
    return error
  }, [])

  const rememberDrawerTrigger = useCallback(() => {
    const active = document.activeElement
    drawerTriggerRef.current = active instanceof HTMLElement ? active : null
  }, [])

  const restoreDrawerFocus = useCallback(() => {
    const trigger = drawerTriggerRef.current
    if (!trigger) return
    if (trigger.isConnected) trigger.focus()
    window.requestAnimationFrame(() => {
      if (trigger.isConnected) trigger.focus()
      if (drawerTriggerRef.current === trigger) drawerTriggerRef.current = null
    })
  }, [])

  const openDrawer = useCallback((drawer: WorkbenchDrawerId) => {
    rememberDrawerTrigger()
    dispatchDrawer({ type: 'open', drawer })
  }, [rememberDrawerTrigger])

  const closeDrawers = useCallback(() => {
    dispatchDrawer({ type: 'close' })
    restoreDrawerFocus()
  }, [restoreDrawerFocus])

  return {
    activeDesignState,
    openProject,
    startActiveDesignRequest,
    isCurrentActiveDesignRequest,
    receiveActiveDesignSnapshot,
    failActiveDesignRequest,
    drawerFocusRef,
    componentDrawerOpen: drawerState.openDrawer === 'component',
    exportOpen: drawerState.openDrawer === 'export',
    qualityOpen: drawerState.openDrawer === 'quality',
    hasOpenDrawer: drawerState.openDrawer !== null,
    openDrawer,
    closeDrawers,
  }
}
