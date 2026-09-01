import * as THREE from 'three'
import { GLTFLoader, type GLTF } from 'three/addons/loaders/GLTFLoader.js'

import {
  createKnifeDeliveryController,
  type KnifeDeliveryController,
} from '../knife-delivery-runtime.ts'
import { applyWeaponryCameraState, type WeaponryCameraFrame } from './camera.ts'
import { createWeaponryColliderProxyDescriptors, type WeaponryColliderProxyDescriptor } from './colliders.ts'
import {
  applyWeaponryPresentationPose,
  captureWeaponryPresentationBaseline,
  WEAPONRY_PRESENTATION_MODE,
  type WeaponryPresentationBaseline,
} from './presentation-pose.ts'
import type { WeaponrySimulationState } from './types.ts'
import type { WeaponryEngineApplyResult, WeaponryPartPick } from './types.ts'

export interface WeaponryR8GlbParser {
  parseAsync(data: ArrayBuffer | string, path: string): Promise<GLTF>
}

export interface LoadWeaponryR8Options {
  /** Testable host capability; defaults to the vanilla Three.js GLTFLoader. */
  readonly loader?: WeaponryR8GlbParser
}

export interface WeaponryR8Delivery {
  readonly gltf: GLTF
  readonly root: THREE.Object3D
  readonly controller: KnifeDeliveryController
  readonly bridge: WeaponryR8ControllerBridge
}

export interface WeaponryR8ControllerBridge {
  readonly schema_version: 'WeaponryThreeJsR8EngineBridge@1'
  readonly root: THREE.Object3D
  readonly controller: KnifeDeliveryController
  readonly part_ids: readonly string[]
  readonly collider_proxies: readonly WeaponryColliderProxyDescriptor[]
  applySimulation(state: WeaponrySimulationState, camera?: THREE.PerspectiveCamera): WeaponryEngineApplyResult & { readonly camera_frame?: WeaponryCameraFrame }
  setPartVisible(partId: string, visible: boolean): void
  setExploded(amount: number): void
  pick(raycaster: THREE.Raycaster): WeaponryPartPick | null
}

/**
 * Loads a GLB from caller-owned bytes only. No path, URL, fetch, plugin, or
 * script is accepted by this bridge; the GLTFLoader receives an empty base
 * path because the r8 package is a self-contained GLB.
 */
export async function loadWeaponryR8Delivery(
  bytes: ArrayBuffer | ArrayBufferView,
  options: LoadWeaponryR8Options = {},
): Promise<WeaponryR8Delivery> {
  const loader = options.loader ?? new GLTFLoader()
  const gltf = await loader.parseAsync(copyBytes(bytes), '')
  const root = findActionReadyRoot(gltf.scene)
  const controller = createKnifeDeliveryController(root)
  const bridge = createWeaponryR8ControllerBridge(root, controller)
  return Object.freeze({ gltf, root, controller, bridge })
}

/** Alias emphasizing the byte-only, r8-specific boundary. */
export const loadR8KnifeDeliveryFromBytes = loadWeaponryR8Delivery

export function createWeaponryR8ControllerBridge(
  root: THREE.Object3D,
  controller: KnifeDeliveryController = createKnifeDeliveryController(root),
): WeaponryR8ControllerBridge {
  const partIds = Object.freeze([...controller.metadata.part_ids])
  const colliderProxies = createWeaponryColliderProxyDescriptors(controller)
  const baseline = captureWeaponryPresentationBaseline(root)
  return {
    schema_version: 'WeaponryThreeJsR8EngineBridge@1',
    root,
    controller,
    part_ids: partIds,
    collider_proxies: colliderProxies,
    applySimulation(state, camera) {
      // Start each frame at the captured rest transform before projecting the
      // caller-owned state, making idle an exact restoration point.
      applyWeaponryPresentationPose(root, baseline, 'idle', 0)
      for (const partId of partIds) controller.setPartVisible(partId, state.visible_parts[partId] !== false)
      controller.setExploded(state.exploded)
      applyWeaponryPresentationPose(root, baseline, state.interaction, state.action_progress)
      let cameraFrame: WeaponryCameraFrame | undefined
      if (camera) cameraFrame = applyWeaponryCameraState(camera, root, controller, state.camera)
      let visibleCount = 0
      for (const partId of partIds) if (state.visible_parts[partId] !== false) visibleCount += 1
      return {
        schema_version: 'WeaponryThreeJsEngineApplyResult@1',
        interaction: state.interaction,
        camera: state.camera,
        exploded: state.exploded,
        visible_part_count: visibleCount,
        selected_part_id: state.selected_part_id,
        presentation_mode: WEAPONRY_PRESENTATION_MODE,
        ...(cameraFrame ? { camera_frame: cameraFrame } : {}),
      }
    },
    setPartVisible(partId, visible) {
      controller.setPartVisible(partId, visible)
    },
    setExploded(amount) {
      controller.setExploded(amount)
    },
    pick(raycaster) {
      const hits = raycaster.intersectObject(root, true)
      for (const hit of hits) {
        const partId = controller.resolvePart(hit.object)
        if (partId) return { part_id: partId, distance: hit.distance, object: hit.object }
      }
      return null
    },
  }
}

export const createR8KnifeControllerBridge = createWeaponryR8ControllerBridge

function findActionReadyRoot(scene: THREE.Object3D): THREE.Object3D {
  let root: THREE.Object3D | null = null
  scene.traverse((object) => {
    if (!root && object.userData?.sculptRuntime?.schema_version === 'WeaponryThreeJsKnifeActionRuntime@1') root = object
  })
  if (!root) throw new Error('WEAPONRY_ENGINE_RUNTIME_MISSING: action-ready r8 root not found')
  return root
}

function copyBytes(bytes: ArrayBuffer | ArrayBufferView): ArrayBuffer {
  if (bytes instanceof ArrayBuffer) {
    if (bytes.byteLength === 0) throw new Error('WEAPONRY_ENGINE_INVALID_GLB_BYTES: empty input')
    return bytes.slice(0)
  }
  if (!ArrayBuffer.isView(bytes)) throw new Error('WEAPONRY_ENGINE_INVALID_GLB_BYTES: expected ArrayBuffer or view')
  if (bytes.byteLength === 0) throw new Error('WEAPONRY_ENGINE_INVALID_GLB_BYTES: empty input')
  const source = new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  return Uint8Array.from(source).buffer
}
