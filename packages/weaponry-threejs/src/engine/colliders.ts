import type { KnifeColliderIntent, KnifeDeliveryController } from '../knife-delivery-runtime.ts'
import type { WeaponryVec3 } from './types.ts'

export const WEAPONRY_COLLIDER_PROXY_SCHEMA = 'WeaponryThreeJsColliderProxyDescriptor@1' as const

/**
 * A typed presentation/collision intent projection. This is deliberately
 * not a Three.js mesh, physics body, or Rapier shape. The only source is the
 * action-ready delivery's serialized collider intent.
 */
export interface WeaponryColliderProxyDescriptor {
  readonly schema_version: typeof WEAPONRY_COLLIDER_PROXY_SCHEMA
  readonly collider_id: KnifeColliderIntent['collider_id']
  readonly source_shape: KnifeColliderIntent['shape']
  readonly shape: 'box-proxy'
  readonly coordinate_space: 'delivery-root@1'
  readonly part_ids: readonly string[]
  readonly center: WeaponryVec3
  readonly half_extents: WeaponryVec3
  readonly status: 'INTENT_ONLY'
  readonly physics_body_created: false
}

export function createWeaponryColliderProxyDescriptors(
  controller: KnifeDeliveryController,
): readonly WeaponryColliderProxyDescriptor[] {
  const intents = controller.metadata.collider_intents
  return Object.freeze(intents.map((intent) => projectIntent(intent)))
}

export function projectWeaponryColliderIntent(
  intent: KnifeColliderIntent,
): WeaponryColliderProxyDescriptor {
  return projectIntent(intent)
}

function projectIntent(intent: KnifeColliderIntent): WeaponryColliderProxyDescriptor {
  if (intent.shape !== 'box-intent@1') throw new Error(`WEAPONRY_ENGINE_UNSUPPORTED_COLLIDER_INTENT: ${intent.shape}`)
  finiteTuple(intent.center, 'center')
  finiteTuple(intent.half_extents, 'half_extents')
  if (intent.half_extents.some((entry) => entry <= 0)) throw new Error(`WEAPONRY_ENGINE_INVALID_COLLIDER_EXTENTS: ${intent.collider_id}`)
  return Object.freeze({
    schema_version: WEAPONRY_COLLIDER_PROXY_SCHEMA,
    collider_id: intent.collider_id,
    source_shape: intent.shape,
    shape: 'box-proxy',
    coordinate_space: 'delivery-root@1',
    part_ids: Object.freeze([...intent.part_ids]),
    center: tuple(intent.center),
    half_extents: tuple(intent.half_extents),
    status: 'INTENT_ONLY',
    physics_body_created: false,
  })
}

function finiteTuple(value: readonly number[], field: string): void {
  if (value.length !== 3 || value.some((entry) => !Number.isFinite(entry))) {
    throw new Error(`WEAPONRY_ENGINE_INVALID_COLLIDER_${field.toUpperCase()}`)
  }
}

function tuple(value: readonly number[]): WeaponryVec3 {
  return [value[0], value[1], value[2]]
}
