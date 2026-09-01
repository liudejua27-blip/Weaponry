import * as THREE from 'three'

import type {
  KnifeDeliveryController,
  KnifeSocketId,
} from '../knife-delivery-runtime.ts'

/** Closed identity for the presentation-only FPS grip binding. */
export const WEAPONRY_FPS_SOCKET_BINDING_SCHEMA = 'WeaponryThreeJsFpsSocketBinding@1' as const
export const WEAPONRY_FPS_HAND_ID = 'right-hand' as const
export const WEAPONRY_GRIP_SOCKET_ID = 'socket-grip' as const
export const WEAPONRY_FPS_BINDING_MODE = 'PRESENTATION_ONLY_NO_SKELETON' as const

export interface WeaponryFpsSocketBindingSnapshot {
  readonly schema_version: typeof WEAPONRY_FPS_SOCKET_BINDING_SCHEMA
  readonly binding_mode: typeof WEAPONRY_FPS_BINDING_MODE
  readonly socket_id: typeof WEAPONRY_GRIP_SOCKET_ID
  readonly hand_id: typeof WEAPONRY_FPS_HAND_ID
  readonly socket_node_name: string
  readonly hand_node_name: string | null
  readonly status: 'UNBOUND' | 'BOUND'
}

export interface WeaponryFpsSocketBinding {
  readonly schema_version: typeof WEAPONRY_FPS_SOCKET_BINDING_SCHEMA
  readonly binding_mode: typeof WEAPONRY_FPS_BINDING_MODE
  readonly root: THREE.Object3D
  readonly socket: THREE.Object3D
  bind(hand: THREE.Object3D): WeaponryFpsSocketBindingSnapshot
  unbind(): WeaponryFpsSocketBindingSnapshot
  validate(): void
  snapshot(): WeaponryFpsSocketBindingSnapshot
}

/** Creates an explicitly typed right-hand presentation target for FPS tests. */
export function createWeaponryRightHandTarget(name = 'fps-right-hand'): THREE.Group {
  if (!/^[A-Za-z][A-Za-z0-9_.-]{0,63}$/.test(name)) throw new Error('WEAPONRY_FPS_INVALID_HAND_NAME')
  const hand = new THREE.Group()
  hand.name = name
  hand.userData = { ...hand.userData, weaponry_hand_id: WEAPONRY_FPS_HAND_ID }
  return hand
}

/**
 * Resolves and validates the action-ready `socket-grip` node. The binding is
 * intentionally not a skeleton/IK system and never changes the source GLB.
 */
export function createWeaponryRightHandGripBinding(
  controller: KnifeDeliveryController,
  initialHand?: THREE.Object3D,
): WeaponryFpsSocketBinding {
  const root = controller.root
  const socket = resolveGripSocket(controller)
  let hand: THREE.Object3D | null = null
  let bound = false

  const validate = (): void => {
    if (!controller.metadata.socket_ids.includes(WEAPONRY_GRIP_SOCKET_ID as KnifeSocketId)) {
      throw new Error('WEAPONRY_FPS_SOCKET_INVALID: socket-grip is not declared')
    }
    if (socket.userData.schema_version !== 'WeaponryThreeJsKnifeSocket@1' || socket.userData.socket_id !== WEAPONRY_GRIP_SOCKET_ID) {
      throw new Error('WEAPONRY_FPS_SOCKET_INVALID: socket metadata drifted')
    }
    if (!/^knife-socket:?socket-grip$/.test(socket.name) || root.getObjectById(socket.id) !== socket) {
      throw new Error(`WEAPONRY_FPS_SOCKET_INVALID: socket node placement drifted (${socket.name})`)
    }
    if (hand && hand.userData.weaponry_hand_id !== WEAPONRY_FPS_HAND_ID) {
      throw new Error('WEAPONRY_FPS_HAND_INVALID: right-hand tag required')
    }
    if (bound && (!hand || hand.parent !== socket)) {
      throw new Error('WEAPONRY_FPS_BINDING_DRIFTED: bound hand must remain under socket-grip')
    }
  }

  const snapshot = (): WeaponryFpsSocketBindingSnapshot => Object.freeze({
    schema_version: WEAPONRY_FPS_SOCKET_BINDING_SCHEMA,
    binding_mode: WEAPONRY_FPS_BINDING_MODE,
    socket_id: WEAPONRY_GRIP_SOCKET_ID,
    hand_id: WEAPONRY_FPS_HAND_ID,
    socket_node_name: socket.name,
    hand_node_name: hand?.name ?? null,
    status: bound ? 'BOUND' : 'UNBOUND',
  })

  const binding: WeaponryFpsSocketBinding = {
    schema_version: WEAPONRY_FPS_SOCKET_BINDING_SCHEMA,
    binding_mode: WEAPONRY_FPS_BINDING_MODE,
    root,
    socket,
    bind(target) {
      if (!(target instanceof THREE.Object3D)) throw new Error('WEAPONRY_FPS_HAND_INVALID: Object3D required')
      if (target === root || target === socket || target.getObjectById(root.id)) {
        throw new Error('WEAPONRY_FPS_HAND_INVALID: target creates a hierarchy cycle')
      }
      if (target.userData.weaponry_hand_id !== WEAPONRY_FPS_HAND_ID) {
        throw new Error('WEAPONRY_FPS_HAND_INVALID: right-hand tag required')
      }
      if (bound) {
        if (hand === target) return snapshot()
        throw new Error('WEAPONRY_FPS_DUPLICATE_HAND_BINDING: unbind before replacing the right hand')
      }
      validate()
      socket.add(target)
      target.position.set(0, 0, 0)
      target.quaternion.identity()
      target.scale.set(1, 1, 1)
      target.updateMatrix()
      hand = target
      bound = true
      socket.updateMatrixWorld(true)
      validate()
      return snapshot()
    },
    unbind() {
      if (hand) root.attach(hand)
      hand = null
      bound = false
      validate()
      return snapshot()
    },
    validate,
    snapshot,
  }
  validate()
  if (initialHand) binding.bind(initialHand)
  return Object.freeze(binding)
}

function resolveGripSocket(controller: KnifeDeliveryController): THREE.Object3D {
  const socket = controller.sockets.get(WEAPONRY_GRIP_SOCKET_ID)
  if (!socket) throw new Error('WEAPONRY_FPS_SOCKET_MISSING: socket-grip')
  return socket
}
