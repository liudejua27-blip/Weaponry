import type { ActiveDesignErrorState, ActiveDesignApiResponse } from '../../shared/api/forgeApi.js'
import type { ActiveDesignPartDisplay, ActiveDesignSnapshot } from '../../shared/types.js'

export type ActiveDesignOperation = 'idle' | 'loading' | 'selecting' | 'setting_render_preset' | 'setting_part_display' | 'converting_legacy' | 'undoing' | 'redoing'

/**
 * The workbench's future single source for server-owned design state.
 * It deliberately stores no duplicate current version, selected part, quality
 * or export IDs: each is derived from the latest ActiveDesignSnapshot.
 */
export type ActiveDesignMachineState = {
  projectId: string | null
  snapshot: ActiveDesignSnapshot | null
  snapshotEtag: string | null
  operation: ActiveDesignOperation
  latestRequestId: number
  error: ActiveDesignErrorState | null
}

export type ActiveDesignMachineAction =
  | { type: 'open_project'; projectId: string }
  | { type: 'request_started'; requestId: number; operation: Exclude<ActiveDesignOperation, 'idle'> }
  | { type: 'request_cancelled'; requestId: number }
  | {
      type: 'snapshot_received'
      projectId: string
      requestId: number
      response: ActiveDesignApiResponse<ActiveDesignSnapshot>
    }
  | { type: 'request_failed'; requestId: number; error: ActiveDesignErrorState }
  | { type: 'clear_error' }

export const initialActiveDesignMachineState: ActiveDesignMachineState = {
  projectId: null,
  snapshot: null,
  snapshotEtag: null,
  operation: 'idle',
  latestRequestId: 0,
  error: null,
}

export function activeDesignMachineReducer(
  state: ActiveDesignMachineState,
  action: ActiveDesignMachineAction,
): ActiveDesignMachineState {
  switch (action.type) {
    case 'open_project':
      if (state.projectId === action.projectId) return state
      return {
        projectId: action.projectId,
        snapshot: null,
        snapshotEtag: null,
        operation: 'idle',
        latestRequestId: state.latestRequestId,
        error: null,
      }
    case 'request_started':
      if (!state.projectId || action.requestId <= state.latestRequestId) return state
      return {
        ...state,
        operation: action.operation,
        latestRequestId: action.requestId,
        error: null,
      }
    case 'request_cancelled':
      if (action.requestId !== state.latestRequestId) return state
      return {
        ...state,
        operation: 'idle',
      }
    case 'snapshot_received':
      if (
        !state.projectId ||
        action.projectId !== state.projectId ||
        action.response.data.project_id !== state.projectId ||
        action.requestId !== state.latestRequestId
      ) {
        return state
      }
      return {
        ...state,
        snapshot: action.response.data,
        snapshotEtag: action.response.etag,
        operation: 'idle',
        error: null,
      }
    case 'request_failed':
      if (action.requestId !== state.latestRequestId) return state
      return {
        ...state,
        operation: 'idle',
        error: action.error,
      }
    case 'clear_error':
      return state.error ? { ...state, error: null } : state
  }
}

export function activeDesignVersionId(snapshot: ActiveDesignSnapshot | null): string | null {
  if (!snapshot) return null
  return 'asset_version_id' in snapshot.active_design
    ? snapshot.active_design.asset_version_id
    : snapshot.active_design.legacy_version_id
}

export function activeDesignSelectedPartId(snapshot: ActiveDesignSnapshot | null): string | null {
  return snapshot?.selected_part_id ?? null
}

export function activeDesignSelectedMaterialZoneId(snapshot: ActiveDesignSnapshot | null): string | null {
  return snapshot?.selected_material_zone_id ?? null
}

export function activeDesignPartDisplay(snapshot: ActiveDesignSnapshot | null): ActiveDesignPartDisplay | null {
  return snapshot?.part_display ?? null
}

export function activeDesignPartIsVisible(snapshot: ActiveDesignSnapshot | null, partId: string): boolean {
  const display = activeDesignPartDisplay(snapshot)
  if (!display) return true
  return !(display.hidden_part_ids ?? []).includes(partId)
    && (display.isolated_part_id === null || display.isolated_part_id === partId)
}

export function activeDesignPartIsLocked(snapshot: ActiveDesignSnapshot | null, partId: string): boolean {
  return (activeDesignPartDisplay(snapshot)?.locked_part_ids ?? []).includes(partId)
}

export function activeDesignCanSelectParts(snapshot: ActiveDesignSnapshot | null): boolean {
  return Boolean(snapshot && 'asset_version_id' in snapshot.active_design)
}

export function activeDesignIsLegacyReadOnly(snapshot: ActiveDesignSnapshot | null): boolean {
  return Boolean(snapshot && 'legacy_version_id' in snapshot.active_design)
}
