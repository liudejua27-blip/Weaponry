import type * as THREE from 'three'

/** Public identity for the bounded, browser-side r8 interaction adapter. */
export const WEAPONRY_THREEJS_ENGINE_SCHEMA = 'WeaponryThreeJsVanillaEngine@1' as const
export const WEAPONRY_THREEJS_ENGINE_BRIDGE_SCHEMA = 'WeaponryThreeJsR8EngineBridge@1' as const

/**
 * These actions are presentation intents only. They do not model weapon
 * mechanics, damage, networking, or input side effects.
 */
export const WEAPONRY_ACTIONS = Object.freeze([
  'idle',
  'light',
  'heavy',
  'inspect',
  'sheath',
] as const)

export const WEAPONRY_ACTION = Object.freeze({
  IDLE: 'idle',
  LIGHT: 'light',
  HEAVY: 'heavy',
  INSPECT: 'inspect',
  SHEATH: 'sheath',
} as const)

export type WeaponryAction = (typeof WEAPONRY_ACTIONS)[number]
export type WeaponryInteractionState = WeaponryAction
/** Readable aliases for consumers that call the enum an action name/state. */
export type WeaponryActionName = WeaponryAction
export type WeaponryInteractionStateName = WeaponryInteractionState

export const WEAPONRY_CAMERA_STATES = Object.freeze(['fps', 'inspect'] as const)
export type WeaponryCameraStateName = (typeof WEAPONRY_CAMERA_STATES)[number]

export const WEAPONRY_INPUT_PHASES = Object.freeze(['pressed', 'released'] as const)
export type WeaponryInputPhase = (typeof WEAPONRY_INPUT_PHASES)[number]

export interface WeaponryInputEvent {
  readonly code: string
  readonly phase?: WeaponryInputPhase
  readonly repeat?: boolean
}

export interface WeaponryInputMapping {
  readonly bindings: Readonly<Record<string, WeaponryAction>>
}

export type WeaponryVec3 = readonly [number, number, number]

/**
 * Serializable simulation snapshot. The engine never owns or mutates this
 * object; callers pass a snapshot to transition/advance/apply functions.
 */
export interface WeaponrySimulationState {
  readonly schema_version: 'WeaponryThreeJsSimulationState@1'
  readonly time_ms: number
  readonly interaction: WeaponryInteractionState
  readonly action_started_ms: number
  readonly action_progress: number
  readonly camera: WeaponryCameraStateName
  readonly exploded: number
  readonly visible_parts: Readonly<Record<string, boolean>>
  readonly selected_part_id: string | null
}

export interface WeaponrySimulationOptions {
  readonly initial_camera?: WeaponryCameraStateName
  readonly initial_exploded?: number
  readonly visible_parts?: Readonly<Record<string, boolean>>
}

export interface WeaponryAdvanceOptions {
  /** Optional absolute clock value. Omit to advance by delta from state time. */
  readonly now_ms?: number
}

export interface WeaponryPartPick {
  readonly part_id: string
  readonly distance: number
  readonly object: THREE.Object3D
}

export interface WeaponryEngineApplyResult {
  readonly schema_version: 'WeaponryThreeJsEngineApplyResult@1'
  readonly interaction: WeaponryInteractionState
  readonly camera: WeaponryCameraStateName
  readonly exploded: number
  readonly visible_part_count: number
  readonly selected_part_id: string | null
  readonly presentation_mode: 'PRESENTATION_ONLY_NO_SKELETON'
}
