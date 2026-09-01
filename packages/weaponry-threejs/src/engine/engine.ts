import * as THREE from 'three'

import { createWeaponryR8ControllerBridge, type WeaponryR8ControllerBridge } from './r8-bridge.ts'
import {
  DEFAULT_WEAPONRY_INPUT_MAPPING,
  resolveWeaponryAction,
} from './actions.ts'
import {
  advanceWeaponrySimulation,
  createWeaponrySimulationState,
  dispatchWeaponryAction,
  setWeaponryExploded,
  setWeaponryPartVisible,
  setWeaponrySelectedPart,
} from './simulation.ts'
import type {
  WeaponryAction,
  WeaponryEngineApplyResult,
  WeaponryInputEvent,
  WeaponryInputMapping,
  WeaponryPartPick,
  WeaponrySimulationOptions,
  WeaponrySimulationState,
} from './types.ts'
import type { WeaponryColliderProxyDescriptor } from './colliders.ts'

export interface WeaponryEngineSnapshot {
  readonly schema_version: 'WeaponryThreeJsEngineSnapshot@1'
  readonly engine_schema_version: 'WeaponryThreeJsVanillaEngine@1'
  readonly interaction: WeaponrySimulationState['interaction']
  readonly camera: WeaponrySimulationState['camera']
  readonly time_ms: number
  readonly action_progress: number
  readonly exploded: number
  readonly visible_parts: Readonly<Record<string, boolean>>
  readonly selected_part_id: string | null
  readonly presentation_mode: 'PRESENTATION_ONLY_NO_SKELETON'
  readonly part_ids: readonly string[]
  readonly collider_status: 'INTENT_ONLY'
  readonly collider_proxies: readonly WeaponryColliderProxyDescriptor[]
}

export interface WeaponryThreeJsEngine {
  readonly schema_version: 'WeaponryThreeJsVanillaEngine@1'
  readonly bridge: WeaponryR8ControllerBridge
  readonly input_mapping: WeaponryInputMapping
  readonly part_ids: readonly string[]
  readonly collider_proxies: readonly WeaponryColliderProxyDescriptor[]
  createState(options?: WeaponrySimulationOptions): WeaponrySimulationState
  dispatch(state: WeaponrySimulationState, action: WeaponryAction, nowMs?: number): WeaponrySimulationState
  input(state: WeaponrySimulationState, event: WeaponryInputEvent | string, nowMs?: number): WeaponrySimulationState
  advance(state: WeaponrySimulationState, deltaMs: number, nowMs?: number): WeaponrySimulationState
  setPartVisible(state: WeaponrySimulationState, partId: string, visible: boolean): WeaponrySimulationState
  setExploded(state: WeaponrySimulationState, amount: number): WeaponrySimulationState
  pick(state: WeaponrySimulationState, raycaster: THREE.Raycaster): { readonly state: WeaponrySimulationState; readonly hit: WeaponryPartPick | null }
  apply(state: WeaponrySimulationState, camera?: THREE.PerspectiveCamera): WeaponryEngineApplyResult & { readonly camera_frame?: unknown }
  snapshot(state: WeaponrySimulationState): WeaponryEngineSnapshot
}

/**
 * Creates a stateless simulation-to-scene adapter around an action-ready r8
 * root. The returned object owns the delivery bridge only. The simulation
 * snapshot remains caller-owned and every transition returns a new snapshot.
 */
export function createWeaponryThreeJsEngine(
  bridge: WeaponryR8ControllerBridge,
  inputMapping: WeaponryInputMapping = DEFAULT_WEAPONRY_INPUT_MAPPING,
): WeaponryThreeJsEngine {
  const partIds = Object.freeze([...bridge.part_ids])
  return {
    schema_version: 'WeaponryThreeJsVanillaEngine@1',
    bridge,
    input_mapping: inputMapping,
    part_ids: partIds,
    collider_proxies: bridge.collider_proxies,
    createState(options = {}) {
      return createWeaponrySimulationState(partIds, options)
    },
    dispatch(state, action, nowMs) {
      return dispatchWeaponryAction(state, action, nowMs)
    },
    input(state, event, nowMs) {
      const action = resolveWeaponryAction(event, inputMapping)
      return action ? dispatchWeaponryAction(state, action, nowMs) : state
    },
    advance(state, deltaMs, nowMs) {
      return advanceWeaponrySimulation(state, deltaMs, nowMs === undefined ? {} : { now_ms: nowMs })
    },
    setPartVisible(state, partId, visible) {
      return setWeaponryPartVisible(state, partId, visible)
    },
    setExploded(state, amount) {
      return setWeaponryExploded(state, amount)
    },
    pick(state, raycaster) {
      const hit = bridge.pick(raycaster)
      return {
        state: hit ? setWeaponrySelectedPart(state, hit.part_id) : state,
        hit,
      }
    },
    apply(state, camera) {
      return bridge.applySimulation(state, camera)
    },
    snapshot(state) {
      return snapshot(state, partIds, bridge.collider_proxies)
    },
  }
}

export function createWeaponryThreeJsEngineFromRoot(
  root: THREE.Object3D,
  inputMapping: WeaponryInputMapping = DEFAULT_WEAPONRY_INPUT_MAPPING,
): WeaponryThreeJsEngine {
  return createWeaponryThreeJsEngine(createWeaponryR8ControllerBridge(root), inputMapping)
}

export const createR8WeaponryEngine = createWeaponryThreeJsEngineFromRoot

export function snapshotWeaponryEngineState(
  state: WeaponrySimulationState,
  bridge: WeaponryR8ControllerBridge,
): WeaponryEngineSnapshot {
  return snapshot(state, bridge.part_ids, bridge.collider_proxies)
}

function snapshot(
  state: WeaponrySimulationState,
  partIds: readonly string[],
  colliderProxies: readonly WeaponryColliderProxyDescriptor[],
): WeaponryEngineSnapshot {
  return Object.freeze({
    schema_version: 'WeaponryThreeJsEngineSnapshot@1',
    engine_schema_version: 'WeaponryThreeJsVanillaEngine@1',
    interaction: state.interaction,
    camera: state.camera,
    time_ms: state.time_ms,
    action_progress: state.action_progress,
    exploded: state.exploded,
    visible_parts: Object.freeze({ ...state.visible_parts }),
    selected_part_id: state.selected_part_id,
    presentation_mode: 'PRESENTATION_ONLY_NO_SKELETON',
    part_ids: Object.freeze([...partIds]),
    collider_status: 'INTENT_ONLY',
    collider_proxies: colliderProxies,
  })
}
