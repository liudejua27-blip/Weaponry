import {
  WEAPONRY_ACTIONS,
  type WeaponryAction,
  type WeaponryAdvanceOptions,
  type WeaponrySimulationOptions,
  type WeaponrySimulationState,
} from './types.ts'

export const WEAPONRY_SIMULATION_SCHEMA = 'WeaponryThreeJsSimulationState@1' as const

const ACTION_DURATIONS_MS: Readonly<Record<WeaponryAction, number>> = Object.freeze({
  idle: 0,
  light: 240,
  heavy: 520,
  inspect: 0,
  sheath: 320,
})

export function createWeaponrySimulationState(
  partIds: readonly string[],
  options: WeaponrySimulationOptions = {},
): WeaponrySimulationState {
  const uniquePartIds = uniqueStablePartIds(partIds)
  const camera = options.initial_camera ?? 'fps'
  if (camera !== 'fps' && camera !== 'inspect') throw new Error('WEAPONRY_ENGINE_INVALID_CAMERA_STATE: unsupported initial camera')
  const exploded = boundedUnit(options.initial_exploded ?? 0, 'initial_exploded')
  const visibleParts: Record<string, boolean> = {}
  for (const partId of uniquePartIds) visibleParts[partId] = options.visible_parts?.[partId] !== false
  return freezeState({
    schema_version: WEAPONRY_SIMULATION_SCHEMA,
    time_ms: 0,
    interaction: 'idle',
    action_started_ms: 0,
    action_progress: 0,
    camera,
    exploded,
    visible_parts: visibleParts,
    selected_part_id: null,
  })
}

/** Applies a presentation action to an external snapshot. */
export function dispatchWeaponryAction(
  state: WeaponrySimulationState,
  action: WeaponryAction,
  nowMs = state.time_ms,
): WeaponrySimulationState {
  validateState(state)
  if (!(WEAPONRY_ACTIONS as readonly string[]).includes(action)) {
    throw new Error(`WEAPONRY_ENGINE_UNKNOWN_ACTION: ${String(action)}`)
  }
  const now = finiteNonNegative(nowMs, 'now_ms')
  if (now < state.time_ms) throw new Error('WEAPONRY_ENGINE_NON_MONOTONIC_TIME: now_ms precedes state time_ms')
  const nextInteraction: WeaponryAction = action === 'inspect' && state.interaction === 'inspect'
    ? 'idle'
    : action
  const nextCamera = nextInteraction === 'inspect' ? 'inspect' : 'fps'
  return freezeState({
    ...state,
    time_ms: now,
    interaction: nextInteraction,
    action_started_ms: now,
    action_progress: 0,
    camera: nextCamera,
  })
}

/**
 * Advances a snapshot with a monotonic clock. One-shot presentation clips
 * settle back to idle; inspect remains active until toggled or replaced.
 */
export function advanceWeaponrySimulation(
  state: WeaponrySimulationState,
  deltaMs: number,
  options: WeaponryAdvanceOptions = {},
): WeaponrySimulationState {
  validateState(state)
  const delta = finiteNonNegative(deltaMs, 'delta_ms')
  const requestedNow = options.now_ms
  const now = requestedNow === undefined ? state.time_ms + delta : finiteNonNegative(requestedNow, 'now_ms')
  if (now < state.time_ms) throw new Error('WEAPONRY_ENGINE_NON_MONOTONIC_TIME: now_ms precedes state time_ms')
  const duration = ACTION_DURATIONS_MS[state.interaction]
  if (duration === 0) {
    return freezeState({ ...state, time_ms: now, action_progress: state.interaction === 'inspect' ? 1 : 0 })
  }
  const progress = clamp01((now - state.action_started_ms) / duration)
  if (progress >= 1) {
    return freezeState({
      ...state,
      time_ms: now,
      interaction: 'idle',
      action_started_ms: now,
      action_progress: 0,
      camera: 'fps',
    })
  }
  return freezeState({ ...state, time_ms: now, action_progress: progress })
}

export function setWeaponryPartVisible(
  state: WeaponrySimulationState,
  partId: string,
  visible: boolean,
): WeaponrySimulationState {
  validateState(state)
  stableId(partId, 'part_id')
  if (!(partId in state.visible_parts)) throw new Error(`WEAPONRY_ENGINE_UNKNOWN_PART: ${partId}`)
  return freezeState({
    ...state,
    visible_parts: { ...state.visible_parts, [partId]: Boolean(visible) },
  })
}

export function setWeaponryExploded(
  state: WeaponrySimulationState,
  amount: number,
): WeaponrySimulationState {
  validateState(state)
  return freezeState({ ...state, exploded: boundedUnit(amount, 'exploded') })
}

export function setWeaponrySelectedPart(
  state: WeaponrySimulationState,
  partId: string | null,
): WeaponrySimulationState {
  validateState(state)
  if (partId !== null) {
    stableId(partId, 'part_id')
    if (!(partId in state.visible_parts)) throw new Error(`WEAPONRY_ENGINE_UNKNOWN_PART: ${partId}`)
  }
  return freezeState({ ...state, selected_part_id: partId })
}

export function weaponryActionDurationMs(action: WeaponryAction): number {
  if (!(action in ACTION_DURATIONS_MS)) throw new Error(`WEAPONRY_ENGINE_UNKNOWN_ACTION: ${String(action)}`)
  return ACTION_DURATIONS_MS[action]
}

export function validateWeaponrySimulationState(state: WeaponrySimulationState): void {
  validateState(state)
}

function validateState(state: WeaponrySimulationState): void {
  if (!state || state.schema_version !== WEAPONRY_SIMULATION_SCHEMA) throw new Error('WEAPONRY_ENGINE_INVALID_SIMULATION_STATE: schema_version')
  finiteNonNegative(state.time_ms, 'time_ms')
  finiteNonNegative(state.action_started_ms, 'action_started_ms')
  if (!(WEAPONRY_ACTIONS as readonly string[]).includes(state.interaction)) throw new Error('WEAPONRY_ENGINE_INVALID_SIMULATION_STATE: interaction')
  clamp01(state.action_progress)
  if (state.camera !== 'fps' && state.camera !== 'inspect') throw new Error('WEAPONRY_ENGINE_INVALID_SIMULATION_STATE: camera')
  boundedUnit(state.exploded, 'exploded')
  if (!state.visible_parts || typeof state.visible_parts !== 'object') throw new Error('WEAPONRY_ENGINE_INVALID_SIMULATION_STATE: visible_parts')
  if (state.selected_part_id !== null && !(state.selected_part_id in state.visible_parts)) {
    throw new Error('WEAPONRY_ENGINE_INVALID_SIMULATION_STATE: selected_part_id is not a known part')
  }
  if (state.action_started_ms > state.time_ms) throw new Error('WEAPONRY_ENGINE_INVALID_SIMULATION_STATE: action_started_ms is in the future')
}

function freezeState(input: Omit<WeaponrySimulationState, 'schema_version'> & { schema_version: string }): WeaponrySimulationState {
  const visibleParts = Object.freeze({ ...input.visible_parts })
  return Object.freeze({ ...input, schema_version: WEAPONRY_SIMULATION_SCHEMA, visible_parts: visibleParts })
}

function uniqueStablePartIds(partIds: readonly string[]): string[] {
  if (!Array.isArray(partIds)) throw new Error('WEAPONRY_ENGINE_INVALID_PART_IDS: expected an array')
  const unique = [...new Set(partIds)]
  if (unique.length === 0) throw new Error('WEAPONRY_ENGINE_INVALID_PART_IDS: at least one part is required')
  for (const partId of unique) stableId(partId, 'part_id')
  return unique
}

function stableId(value: string, field: string): void {
  if (typeof value !== 'string' || !/^[A-Za-z][A-Za-z0-9_.-]{0,63}$/.test(value)) {
    throw new Error(`WEAPONRY_ENGINE_INVALID_${field.toUpperCase()}: stable ID required`)
  }
}

function finiteNonNegative(value: number, field: string): number {
  if (!Number.isFinite(value) || value < 0) throw new Error(`WEAPONRY_ENGINE_INVALID_${field.toUpperCase()}: finite non-negative number required`)
  return value
}

function boundedUnit(value: number, field: string): number {
  if (!Number.isFinite(value) || value < 0 || value > 1) throw new Error(`WEAPONRY_ENGINE_INVALID_${field.toUpperCase()}: expected 0..1`)
  return value
}

function clamp01(value: number): number {
  if (!Number.isFinite(value)) throw new Error('WEAPONRY_ENGINE_INVALID_PROGRESS: finite number required')
  return Math.max(0, Math.min(1, value))
}
