import * as THREE from 'three'

import type { WeaponryInteractionState } from './types.ts'

/** Explicitly separates this adapter from skeleton or gameplay animation. */
export const WEAPONRY_PRESENTATION_MODE = 'PRESENTATION_ONLY_NO_SKELETON' as const

export interface WeaponryPresentationBaseline {
  readonly position: THREE.Vector3
  readonly quaternion: THREE.Quaternion
  readonly scale: THREE.Vector3
}

export function captureWeaponryPresentationBaseline(root: THREE.Object3D): WeaponryPresentationBaseline {
  return Object.freeze({
    position: root.position.clone(),
    quaternion: root.quaternion.clone(),
    scale: root.scale.clone(),
  })
}

/**
 * Replaces the root transform from the captured baseline and applies a small,
 * deterministic visual pose for the current interaction state. The baseline
 * reset is intentional: applying a frame repeatedly cannot accumulate drift.
 */
export function applyWeaponryPresentationPose(
  root: THREE.Object3D,
  baseline: WeaponryPresentationBaseline,
  interaction: WeaponryInteractionState,
  progress: number,
): void {
  if (!Number.isFinite(progress) || progress < 0 || progress > 1) {
    throw new Error('WEAPONRY_ENGINE_INVALID_PRESENTATION_PROGRESS: expected 0..1')
  }
  root.position.copy(baseline.position)
  root.quaternion.copy(baseline.quaternion)
  root.scale.copy(baseline.scale)

  const pulse = Math.sin(Math.PI * progress)
  const offset = new THREE.Vector3()
  const rotation = new THREE.Quaternion()
  switch (interaction) {
    case 'light':
      offset.set(0, 0, -0.018 * pulse)
      rotation.setFromEuler(new THREE.Euler(0, 0.10 * pulse, -0.035 * pulse))
      break
    case 'heavy':
      offset.set(0, -0.032 * pulse, -0.028 * pulse)
      rotation.setFromEuler(new THREE.Euler(-0.06 * pulse, 0.19 * pulse, 0))
      break
    case 'inspect':
      offset.set(0, 0.045, 0)
      rotation.setFromEuler(new THREE.Euler(-0.10, 0.20, 0.025))
      break
    case 'sheath':
      offset.set(0, -0.024 * pulse, 0.035 * pulse)
      rotation.setFromEuler(new THREE.Euler(0.13 * pulse, -0.08 * pulse, 0))
      break
    case 'idle':
      break
  }
  root.position.add(offset)
  root.quaternion.multiply(rotation)
  root.updateMatrixWorld(true)
}
