import type { WeaponryColliderProxyDescriptor } from './colliders.ts'
import type { WeaponryVec3 } from './types.ts'

/** Stable identity for the dependency-injected Rapier preview adapter. */
export const WEAPONRY_RAPIER_PREVIEW_SCHEMA = 'WeaponryThreeJsRapierPreviewBridge@1' as const
export const WEAPONRY_RAPIER_PREVIEW_SNAPSHOT_SCHEMA = 'WeaponryThreeJsRapierPreviewSnapshot@1' as const

/** The preview world is deliberately isolated from gameplay/weapon rules. */
export const WEAPONRY_RAPIER_PREVIEW_GRAVITY = Object.freeze({ x: 0, y: 0, z: 0 })
export const WEAPONRY_RAPIER_COLLISION_GROUPS = Object.freeze({
  preview_membership: 1 << 0,
  preview_filter: (1 << 0) | (1 << 1),
  encoded: (((1 << 0) << 16) | (1 << 0) | (1 << 1)) >>> 0,
})

export interface WeaponryRapierVector3 {
  readonly x: number
  readonly y: number
  readonly z: number
}

export interface WeaponryRapierRotation {
  readonly x: number
  readonly y: number
  readonly z: number
  readonly w: number
}

/**
 * The minimum Rapier surface consumed by this adapter. Keeping it structural
 * makes the package independent of a particular WASM/JS Rapier distribution;
 * the host injects its already-initialized module at the application boundary.
 */
export interface WeaponryRapierPreviewModule {
  readonly World: new (gravity: WeaponryRapierVector3) => WeaponryRapierWorld
  readonly RigidBodyDesc: {
    readonly kinematicPositionBased: () => WeaponryRapierRigidBodyDesc
  }
  readonly ColliderDesc: {
    readonly cuboid: (halfX: number, halfY: number, halfZ: number) => WeaponryRapierColliderDesc
  }
}

export interface WeaponryRapierRigidBodyDesc {
  setTranslation(x: number, y: number, z: number): WeaponryRapierRigidBodyDesc
  setRotation(rotation: WeaponryRapierRotation): WeaponryRapierRigidBodyDesc
}

export interface WeaponryRapierColliderDesc {
  setTranslation(x: number, y: number, z: number): WeaponryRapierColliderDesc
  setCollisionGroups(groups: number): WeaponryRapierColliderDesc
}

export interface WeaponryRapierRigidBody {
  setNextKinematicTranslation(translation: WeaponryRapierVector3): void
  setNextKinematicRotation(rotation: WeaponryRapierRotation): void
  translation(): WeaponryRapierVector3
  rotation(): WeaponryRapierRotation
}

export interface WeaponryRapierCollider {
  readonly handle?: number
}

export interface WeaponryRapierWorld {
  createRigidBody(description: WeaponryRapierRigidBodyDesc): WeaponryRapierRigidBody
  createCollider(description: WeaponryRapierColliderDesc, body: WeaponryRapierRigidBody): WeaponryRapierCollider
  removeCollider(collider: WeaponryRapierCollider, wakeUp: boolean): void
  removeRigidBody(body: WeaponryRapierRigidBody): void
  step(): void
  free?: () => void
}

export interface WeaponryRapierRootTransform {
  readonly translation: WeaponryRapierVector3
  readonly rotation?: WeaponryRapierRotation
}

export interface WeaponryRapierPreviewOptions {
  /** Exact expected part set; every part must occur in exactly one proxy. */
  readonly part_ids: readonly string[]
  readonly initial_transform?: WeaponryRapierRootTransform
  /** Uniform presentation scale applied to the normalized GLB root. */
  readonly uniform_scale?: number
  /** Defaults to the explicit preview groups above. */
  readonly collision_groups?: number
}

export interface WeaponryRapierPreviewColliderSnapshot {
  readonly collider_id: WeaponryColliderProxyDescriptor['collider_id']
  readonly part_ids: readonly string[]
  readonly center: WeaponryVec3
  readonly half_extents: WeaponryVec3
  readonly collision_groups: number
}

export interface WeaponryRapierPreviewSnapshot {
  readonly schema_version: typeof WEAPONRY_RAPIER_PREVIEW_SNAPSHOT_SCHEMA
  readonly bridge_schema_version: typeof WEAPONRY_RAPIER_PREVIEW_SCHEMA
  readonly body_type: 'kinematic-position-based'
  readonly uniform_scale: number
  readonly gravity: WeaponryRapierVector3
  readonly translation: WeaponryRapierVector3
  readonly rotation: WeaponryRapierRotation
  readonly collider_status: 'PREVIEW_PHYSICS_BODY_CREATED'
  readonly collider_count: 2
  readonly colliders: readonly WeaponryRapierPreviewColliderSnapshot[]
}

export interface WeaponryRapierPreviewBridge {
  readonly schema_version: typeof WEAPONRY_RAPIER_PREVIEW_SCHEMA
  readonly body_type: 'kinematic-position-based'
  readonly gravity: WeaponryRapierVector3
  readonly collider_status: 'PREVIEW_PHYSICS_BODY_CREATED'
  readonly collider_proxies: readonly WeaponryColliderProxyDescriptor[]
  syncRootTransform(transform: WeaponryRapierRootTransform): void
  step(): WeaponryRapierPreviewSnapshot
  snapshot(): WeaponryRapierPreviewSnapshot
  dispose(): void
}

/**
 * Creates the smallest real-physics adapter for the r8 preview.
 *
 * This function does not import, initialize, or download Rapier. The host
 * supplies an initialized module, so the production package remains usable in
 * environments that do not ship WASM. Two typed box intents become two
 * colliders on one zero-gravity kinematic body. No damage, hit, or gameplay
 * semantics are represented here.
 */
export function createWeaponryRapierPreviewBridge(
  rapier: WeaponryRapierPreviewModule,
  colliderProxies: readonly WeaponryColliderProxyDescriptor[],
  options: WeaponryRapierPreviewOptions,
): WeaponryRapierPreviewBridge {
  validateRapierModule(rapier)
  const normalized = validatePreviewInputs(colliderProxies, options)
  const collisionGroups = options.collision_groups ?? WEAPONRY_RAPIER_COLLISION_GROUPS.encoded
  validateCollisionGroups(collisionGroups)
  const uniformScale = finitePositive(options.uniform_scale ?? 1, 'uniform_scale')

  const initial = normalizeTransform(options.initial_transform)
  const bodyDescription = rapier.RigidBodyDesc.kinematicPositionBased()
    .setTranslation(initial.translation.x, initial.translation.y, initial.translation.z)
    .setRotation(initial.rotation)
  const world = new rapier.World(WEAPONRY_RAPIER_PREVIEW_GRAVITY)
  const body = world.createRigidBody(bodyDescription)
  const colliders = normalized.map((proxy) => {
    const description = rapier.ColliderDesc.cuboid(
      proxy.half_extents[0] * uniformScale,
      proxy.half_extents[1] * uniformScale,
      proxy.half_extents[2] * uniformScale,
    )
      .setTranslation(
        proxy.center[0] * uniformScale,
        proxy.center[1] * uniformScale,
        proxy.center[2] * uniformScale,
      )
      .setCollisionGroups(collisionGroups)
    return world.createCollider(description, body)
  })

  let disposed = false
  const assertLive = () => {
    if (disposed) throw new Error('WEAPONRY_RAPIER_PREVIEW_DISPOSED: bridge is no longer usable')
  }
  const bridge: WeaponryRapierPreviewBridge = {
    schema_version: WEAPONRY_RAPIER_PREVIEW_SCHEMA,
    body_type: 'kinematic-position-based',
    gravity: WEAPONRY_RAPIER_PREVIEW_GRAVITY,
    collider_status: 'PREVIEW_PHYSICS_BODY_CREATED',
    collider_proxies: Object.freeze([...normalized]),
    syncRootTransform(transform) {
      assertLive()
      const next = normalizeTransform(transform)
      body.setNextKinematicTranslation(next.translation)
      body.setNextKinematicRotation(next.rotation)
    },
    step() {
      assertLive()
      world.step()
      return snapshot()
    },
    snapshot() {
      assertLive()
      return snapshot()
    },
    dispose() {
      if (disposed) return
      // Remove child colliders before their parent body. Rapier's remove calls
      // are intentionally explicit so the adapter never relies on GC timing.
      for (const collider of colliders) world.removeCollider(collider, true)
      world.removeRigidBody(body)
      world.free?.()
      disposed = true
    },
  }
  return bridge

  function snapshot(): WeaponryRapierPreviewSnapshot {
    const translation = normalizeVector(body.translation(), 'body_translation')
    const rotation = normalizeRotation(body.rotation(), 'body_rotation')
    return Object.freeze({
      schema_version: WEAPONRY_RAPIER_PREVIEW_SNAPSHOT_SCHEMA,
      bridge_schema_version: WEAPONRY_RAPIER_PREVIEW_SCHEMA,
      body_type: 'kinematic-position-based',
      uniform_scale: uniformScale,
      gravity: WEAPONRY_RAPIER_PREVIEW_GRAVITY,
      translation,
      rotation,
      collider_status: 'PREVIEW_PHYSICS_BODY_CREATED',
      collider_count: 2,
      colliders: Object.freeze(normalized.map((proxy) => Object.freeze({
        collider_id: proxy.collider_id,
        part_ids: Object.freeze([...proxy.part_ids]),
        center: tuple(proxy.center.map((entry) => entry * uniformScale)),
        half_extents: tuple(proxy.half_extents.map((entry) => entry * uniformScale)),
        collision_groups: collisionGroups,
      }))),
    })
  }
}

function validateRapierModule(rapier: WeaponryRapierPreviewModule): void {
  if (!rapier || typeof rapier !== 'object') throw new Error('WEAPONRY_RAPIER_PREVIEW_INVALID_MODULE: object required')
  if (typeof rapier.World !== 'function') throw new Error('WEAPONRY_RAPIER_PREVIEW_INVALID_MODULE: World missing')
  if (typeof rapier.RigidBodyDesc?.kinematicPositionBased !== 'function') {
    throw new Error('WEAPONRY_RAPIER_PREVIEW_INVALID_MODULE: kinematicPositionBased missing')
  }
  if (typeof rapier.ColliderDesc?.cuboid !== 'function') {
    throw new Error('WEAPONRY_RAPIER_PREVIEW_INVALID_MODULE: cuboid missing')
  }
}

function validatePreviewInputs(
  proxies: readonly WeaponryColliderProxyDescriptor[],
  options: WeaponryRapierPreviewOptions,
): readonly WeaponryColliderProxyDescriptor[] {
  if (!Array.isArray(proxies) || proxies.length !== 2) {
    throw new Error('WEAPONRY_RAPIER_PREVIEW_INVALID_COLLIDERS: exactly two collider proxies are required')
  }
  const expected = validatePartIds(options?.part_ids)
  const seenColliderIds = new Set<string>()
  const seenPartIds = new Set<string>()
  const normalized = [...proxies].sort((left, right) => left.collider_id.localeCompare(right.collider_id))
  for (const proxy of normalized) {
    if (proxy.shape !== 'box-proxy' || proxy.source_shape !== 'box-intent@1') {
      throw new Error(`WEAPONRY_RAPIER_PREVIEW_UNSUPPORTED_SHAPE: ${proxy.collider_id}`)
    }
    if (proxy.status !== 'INTENT_ONLY' || proxy.physics_body_created !== false) {
      throw new Error(`WEAPONRY_RAPIER_PREVIEW_INVALID_PROXY_STATUS: ${proxy.collider_id}`)
    }
    if (seenColliderIds.has(proxy.collider_id)) throw new Error(`WEAPONRY_RAPIER_PREVIEW_DUPLICATE_COLLIDER: ${proxy.collider_id}`)
    seenColliderIds.add(proxy.collider_id)
    validateTuple(proxy.center, `collider_${proxy.collider_id}_center`)
    validateTuple(proxy.half_extents, `collider_${proxy.collider_id}_half_extents`)
    if (proxy.half_extents.some((entry: number) => entry <= 0)) {
      throw new Error(`WEAPONRY_RAPIER_PREVIEW_INVALID_EXTENTS: ${proxy.collider_id}`)
    }
    if (proxy.part_ids.length === 0) throw new Error(`WEAPONRY_RAPIER_PREVIEW_EMPTY_PART_COVERAGE: ${proxy.collider_id}`)
    for (const partId of proxy.part_ids) {
      validateStableId(partId, 'part_id')
      if (seenPartIds.has(partId)) throw new Error(`WEAPONRY_RAPIER_PREVIEW_DUPLICATE_PART_COVERAGE: ${partId}`)
      seenPartIds.add(partId)
    }
  }
  if (seenPartIds.size !== expected.size || [...expected].some((partId) => !seenPartIds.has(partId))) {
    throw new Error('WEAPONRY_RAPIER_PREVIEW_PART_COVERAGE_MISMATCH: proxy union must equal expected part IDs')
  }
  return Object.freeze(normalized.map((proxy) => Object.freeze({
    ...proxy,
    part_ids: Object.freeze([...proxy.part_ids]),
    center: tuple(proxy.center),
    half_extents: tuple(proxy.half_extents),
  })))
}

function validatePartIds(partIds: readonly string[] | undefined): ReadonlySet<string> {
  if (!Array.isArray(partIds) || partIds.length === 0) {
    throw new Error('WEAPONRY_RAPIER_PREVIEW_INVALID_PART_IDS: expected a non-empty exact part set')
  }
  const unique = new Set<string>()
  for (const partId of partIds) {
    validateStableId(partId, 'part_id')
    if (unique.has(partId)) throw new Error(`WEAPONRY_RAPIER_PREVIEW_DUPLICATE_EXPECTED_PART: ${partId}`)
    unique.add(partId)
  }
  return unique
}

function normalizeTransform(transform: WeaponryRapierRootTransform | undefined): {
  readonly translation: WeaponryRapierVector3
  readonly rotation: WeaponryRapierRotation
} {
  const translation = normalizeVector(transform?.translation ?? { x: 0, y: 0, z: 0 }, 'root_translation')
  const rotation = normalizeRotation(
    transform?.rotation ?? { x: 0, y: 0, z: 0, w: 1 },
    'root_rotation',
  )
  const length = Math.hypot(rotation.x, rotation.y, rotation.z, rotation.w)
  if (length < 1e-12) throw new Error('WEAPONRY_RAPIER_PREVIEW_INVALID_ROOT_ROTATION: non-zero quaternion required')
  return {
    translation,
    rotation: {
      x: rotation.x / length,
      y: rotation.y / length,
      z: rotation.z / length,
      w: rotation.w / length,
    },
  }
}

function normalizeVector(value: WeaponryRapierVector3, field: string): WeaponryRapierVector3 {
  if (!value || !Number.isFinite(value.x) || !Number.isFinite(value.y) || !Number.isFinite(value.z)) {
    throw new Error(`WEAPONRY_RAPIER_PREVIEW_INVALID_${field.toUpperCase()}: finite vector required`)
  }
  return Object.freeze({ x: value.x, y: value.y, z: value.z })
}

function normalizeRotation(value: WeaponryRapierRotation, field: string): WeaponryRapierRotation {
  if (
    !value ||
    !Number.isFinite(value.x) ||
    !Number.isFinite(value.y) ||
    !Number.isFinite(value.z) ||
    !Number.isFinite(value.w)
  ) {
    throw new Error(`WEAPONRY_RAPIER_PREVIEW_INVALID_${field.toUpperCase()}: finite quaternion required`)
  }
  return Object.freeze({ x: value.x, y: value.y, z: value.z, w: value.w })
}

function validateTuple(value: readonly number[], field: string): void {
  if (value.length !== 3 || value.some((entry) => !Number.isFinite(entry))) {
    throw new Error(`WEAPONRY_RAPIER_PREVIEW_INVALID_${field.toUpperCase()}: finite vec3 required`)
  }
}

function validateCollisionGroups(groups: number): void {
  if (!Number.isInteger(groups) || groups < 0 || groups > 0xffffffff) {
    throw new Error('WEAPONRY_RAPIER_PREVIEW_INVALID_COLLISION_GROUPS: uint32 required')
  }
}

function finitePositive(value: number, field: string): number {
  if (!Number.isFinite(value) || value <= 0) {
    throw new Error(`WEAPONRY_RAPIER_PREVIEW_INVALID_${field.toUpperCase()}: finite positive number required`)
  }
  return value
}

function validateStableId(value: string, field: string): void {
  if (typeof value !== 'string' || !/^[A-Za-z][A-Za-z0-9_.-]{0,63}$/.test(value)) {
    throw new Error(`WEAPONRY_RAPIER_PREVIEW_INVALID_${field.toUpperCase()}: stable ID required`)
  }
}

function tuple(value: readonly number[]): WeaponryVec3 {
  return Object.freeze([value[0], value[1], value[2]]) as WeaponryVec3
}
