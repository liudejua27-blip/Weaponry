import * as THREE from 'three'

import type { KnifeDeliveryController } from '../knife-delivery-runtime.ts'
import type { WeaponryCameraStateName, WeaponryVec3 } from './types.ts'

export const WEAPONRY_CAMERA_SCHEMA = 'WeaponryThreeJsCameraState@1' as const
export const WEAPONRY_FPS_CAMERA_STATE = 'fps' as const
export const WEAPONRY_INSPECT_CAMERA_STATE = 'inspect' as const

export interface WeaponryCameraFrame {
  readonly schema_version: 'WeaponryThreeJsCameraFrame@1'
  readonly state: WeaponryCameraStateName
  readonly position: WeaponryVec3
  readonly target: WeaponryVec3
  readonly near: number
  readonly far: number
  readonly fov: number
}

/**
 * Applies one of the two explicit presentation camera states. The camera is a
 * caller-owned render object; no mesh, material, or simulation state is
 * created here. Values are derived from the action-ready delivery bounds and
 * grip socket, with deterministic offsets for browser inspection.
 */
export function applyWeaponryCameraState(
  camera: THREE.PerspectiveCamera,
  root: THREE.Object3D,
  controller: KnifeDeliveryController,
  state: WeaponryCameraStateName,
): WeaponryCameraFrame {
  if (state !== WEAPONRY_FPS_CAMERA_STATE && state !== WEAPONRY_INSPECT_CAMERA_STATE) {
    throw new Error(`WEAPONRY_ENGINE_INVALID_CAMERA_STATE: ${String(state)}`)
  }
  root.updateMatrixWorld(true)
  const bounds = new THREE.Box3().setFromObject(root)
  if (bounds.isEmpty()) throw new Error('WEAPONRY_ENGINE_EMPTY_DELIVERY: camera bounds are empty')
  const center = bounds.getCenter(new THREE.Vector3())
  const size = bounds.getSize(new THREE.Vector3())
  const extent = Math.max(size.length(), 0.001)
  const target = center.clone()
  const position = new THREE.Vector3()

  if (state === WEAPONRY_FPS_CAMERA_STATE) {
    const grip = controller.sockets.get('socket-grip')
    if (grip) grip.getWorldPosition(position)
    else position.copy(center)
    // A small deterministic shoulder offset keeps the asset visible while
    // remaining an inspection/presentation pose rather than a game camera.
    position.add(new THREE.Vector3(-extent * 0.22, extent * 0.12, extent * 0.78))
    target.lerp(center, 0.72)
  } else {
    position.copy(center).add(new THREE.Vector3(extent * 0.82, extent * 0.32, extent * 0.82))
  }

  camera.position.copy(position)
  camera.near = Math.max(extent * 0.0025, 0.001)
  camera.far = Math.max(extent * 8, camera.near + 1)
  camera.fov = state === WEAPONRY_FPS_CAMERA_STATE ? 54 : 42
  camera.lookAt(target)
  camera.updateProjectionMatrix()
  camera.updateMatrixWorld(true)

  return Object.freeze({
    schema_version: 'WeaponryThreeJsCameraFrame@1',
    state,
    position: tuple(position),
    target: tuple(target),
    near: camera.near,
    far: camera.far,
    fov: camera.fov,
  })
}

export function createWeaponryCamera(
  state: WeaponryCameraStateName = WEAPONRY_FPS_CAMERA_STATE,
): THREE.PerspectiveCamera {
  if (state !== WEAPONRY_FPS_CAMERA_STATE && state !== WEAPONRY_INSPECT_CAMERA_STATE) {
    throw new Error(`WEAPONRY_ENGINE_INVALID_CAMERA_STATE: ${String(state)}`)
  }
  return new THREE.PerspectiveCamera(state === 'fps' ? 54 : 42, 1, 0.001, 100)
}

function tuple(value: THREE.Vector3): WeaponryVec3 {
  return [value.x, value.y, value.z]
}
