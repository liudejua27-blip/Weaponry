import * as THREE from 'three'

import type { CompiledKnifeScene } from './knife-scene-compiler.ts'
import type { KnifeSceneProgram, KnifeVec3 } from './knife-scene-program.ts'

export const KNIFE_ACTION_RUNTIME_SCHEMA = 'WeaponryThreeJsKnifeActionRuntime@1' as const
export const KNIFE_DELIVERY_CONTROLLER_SCHEMA = 'WeaponryThreeJsKnifeDeliveryController@1' as const

export type KnifeSocketId = 'socket-blade-tip' | 'socket-guard' | 'socket-grip'
export type KnifeDestructionGroupId = 'blade-assembly' | 'handle-assembly'

export interface KnifeDeliverySocket {
  readonly socket_id: KnifeSocketId
  readonly node_name: string
  readonly position: KnifeVec3
}

export interface KnifeColliderIntent {
  readonly collider_id: 'collider-blade' | 'collider-handle'
  readonly shape: 'box-intent@1'
  readonly part_ids: readonly string[]
  readonly center: KnifeVec3
  readonly half_extents: KnifeVec3
}

export interface KnifeDestructionGroup {
  readonly group_id: KnifeDestructionGroupId
  readonly part_ids: readonly string[]
}

export interface KnifeActionRuntimeMetadata {
  readonly schema_version: typeof KNIFE_ACTION_RUNTIME_SCHEMA
  readonly controller_schema_version: typeof KNIFE_DELIVERY_CONTROLLER_SCHEMA
  readonly part_ids: readonly string[]
  readonly pivot_ids: readonly string[]
  readonly socket_ids: readonly KnifeSocketId[]
  readonly pick_policy: 'nearest-ancestor-part-id@1'
  readonly explode_policy: 'stable-pivot-vector@1'
  readonly collider_intents: readonly KnifeColliderIntent[]
  readonly destruction_groups: readonly KnifeDestructionGroup[]
}

export interface KnifeDeliveryController {
  readonly root: THREE.Object3D
  readonly partMeshes: ReadonlyMap<string, THREE.Mesh>
  readonly partPivots: ReadonlyMap<string, THREE.Object3D>
  readonly sockets: ReadonlyMap<KnifeSocketId, THREE.Object3D>
  readonly metadata: KnifeActionRuntimeMetadata
  setExploded(amount: number): void
  setPartVisible(partId: string, visible: boolean): void
  resolvePart(object: THREE.Object3D | null): string | null
}

const BLADE_PART_IDS = new Set([
  'blade-body',
  'cutting-edge',
  'relief-dragon-belly',
  'relief-dragon-spine',
])

/**
 * Adds the action-ready layer required by the img2threejs delivery boundary.
 *
 * The operation is deliberately geometry preserving: vertex/index buffers,
 * materials, mesh UUIDs and part IDs are untouched. Identity pivot groups,
 * named socket nodes and serializable interaction metadata are added around
 * the compiled scene before GLB export.
 */
export function makeKnifeSceneActionReady(
  compiled: CompiledKnifeScene,
  program: KnifeSceneProgram,
): KnifeDeliveryController {
  const root = compiled.group
  const expectedPartIds = [...program.parts].map((part) => part.part_id).sort()
  const compiledPartIds = [...compiled.parts].map((part) => part.part_id).sort()
  if (expectedPartIds.join('\u0000') !== compiledPartIds.join('\u0000')) {
    throw new Error('KNIFE_DELIVERY_PART_COVERAGE_MISMATCH: compiled parts do not cover the program exactly')
  }

  const sceneBounds = new THREE.Box3().setFromObject(root)
  if (sceneBounds.isEmpty()) throw new Error('KNIFE_DELIVERY_EMPTY_SCENE: compiled scene has no bounds')
  const sceneCenter = sceneBounds.getCenter(new THREE.Vector3())
  const sceneSize = sceneBounds.getSize(new THREE.Vector3())
  const explodeScale = Math.max(sceneSize.length() * 0.18, 0.25)

  for (const part of compiled.parts) {
    const mesh = part.mesh
    if (mesh.parent !== root) {
      throw new Error(`KNIFE_DELIVERY_INVALID_PARENT: ${part.part_id} is not a direct scene child`)
    }
    mesh.updateMatrix()
    const pivot = new THREE.Group()
    pivot.name = `knife-pivot:${part.part_id}`
    pivot.position.copy(mesh.position)
    pivot.quaternion.copy(mesh.quaternion)
    pivot.scale.copy(mesh.scale)
    pivot.userData = {
      schema_version: 'WeaponryThreeJsKnifePartPivot@1',
      pivot_id: `pivot-${part.part_id}`,
      part_id: part.part_id,
    }

    root.remove(mesh)
    mesh.position.set(0, 0, 0)
    mesh.quaternion.identity()
    mesh.scale.set(1, 1, 1)
    mesh.updateMatrix()
    pivot.add(mesh)
    root.add(pivot)

    const center = new THREE.Box3().setFromObject(pivot).getCenter(new THREE.Vector3())
    const direction = center.sub(sceneCenter)
    if (direction.lengthSq() < 1e-10) {
      direction.set(BLADE_PART_IDS.has(part.part_id) ? 0 : -1, part.part_id.length % 2 === 0 ? 1 : -1, 0)
    }
    direction.normalize().multiplyScalar(explodeScale)
    pivot.userData.explode_vector = vectorTuple(direction)
    pivot.userData.rest_position = vectorTuple(pivot.position)
  }

  const sockets = buildSockets(program)
  for (const socket of sockets) {
    const node = new THREE.Object3D()
    node.name = `knife-socket:${socket.socket_id}`
    node.position.fromArray(socket.position)
    node.userData = {
      schema_version: 'WeaponryThreeJsKnifeSocket@1',
      socket_id: socket.socket_id,
    }
    root.add(node)
  }

  const bladePartIds = expectedPartIds.filter((partId) => BLADE_PART_IDS.has(partId))
  const handlePartIds = expectedPartIds.filter((partId) => !BLADE_PART_IDS.has(partId))
  const colliderIntents = [
    colliderIntent(root, 'collider-blade', bladePartIds),
    colliderIntent(root, 'collider-handle', handlePartIds),
  ] as const
  const destructionGroups: readonly KnifeDestructionGroup[] = [
    { group_id: 'blade-assembly', part_ids: bladePartIds },
    { group_id: 'handle-assembly', part_ids: handlePartIds },
  ]
  const metadata: KnifeActionRuntimeMetadata = {
    schema_version: KNIFE_ACTION_RUNTIME_SCHEMA,
    controller_schema_version: KNIFE_DELIVERY_CONTROLLER_SCHEMA,
    part_ids: expectedPartIds,
    pivot_ids: expectedPartIds.map((partId) => `pivot-${partId}`),
    socket_ids: sockets.map((socket) => socket.socket_id),
    pick_policy: 'nearest-ancestor-part-id@1',
    explode_policy: 'stable-pivot-vector@1',
    collider_intents: colliderIntents,
    destruction_groups: destructionGroups,
  }
  root.userData.sculptRuntime = metadata
  root.userData.delivery_status = 'ACTION_READY'
  root.userData.visual_status = 'NOT_APPROVED'
  root.userData.human_status = 'NOT_RUN'
  root.userData.engine_status = 'NOT_RUN'
  root.userData.commercial_status = 'NOT_RUN'

  return createKnifeDeliveryController(root)
}

/** Rehydrates interaction helpers from serializable GLB node extras. */
export function createKnifeDeliveryController(root: THREE.Object3D): KnifeDeliveryController {
  const metadata = root.userData.sculptRuntime as KnifeActionRuntimeMetadata | undefined
  if (metadata?.schema_version !== KNIFE_ACTION_RUNTIME_SCHEMA) {
    throw new Error('KNIFE_DELIVERY_RUNTIME_MISSING: root does not carry the action-ready runtime metadata')
  }

  const partMeshes = new Map<string, THREE.Mesh>()
  const partPivots = new Map<string, THREE.Object3D>()
  const sockets = new Map<KnifeSocketId, THREE.Object3D>()
  root.traverse((object) => {
    const partId = stringValue(object.userData.part_id)
    if (object instanceof THREE.Mesh && partId) {
      if (partMeshes.has(partId)) throw new Error(`KNIFE_DELIVERY_DUPLICATE_PART: ${partId}`)
      partMeshes.set(partId, object)
    }
    const pivotId = stringValue(object.userData.pivot_id)
    if (pivotId && partId) {
      if (pivotId !== `pivot-${partId}`) throw new Error(`KNIFE_DELIVERY_PIVOT_ID_MISMATCH: ${partId}`)
      if (partPivots.has(partId)) throw new Error(`KNIFE_DELIVERY_DUPLICATE_PIVOT: ${partId}`)
      partPivots.set(partId, object)
    }
    const socketId = stringValue(object.userData.socket_id)
    if (socketId && metadata.socket_ids.includes(socketId as KnifeSocketId)) {
      if (sockets.has(socketId as KnifeSocketId)) throw new Error(`KNIFE_DELIVERY_DUPLICATE_SOCKET: ${socketId}`)
      sockets.set(socketId as KnifeSocketId, object)
    }
  })

  for (const partId of metadata.part_ids) {
    if (!partMeshes.has(partId) || !partPivots.has(partId)) {
      throw new Error(`KNIFE_DELIVERY_PART_MISSING: ${partId} lacks a mesh or pivot`)
    }
  }
  for (const socketId of metadata.socket_ids) {
    if (!sockets.has(socketId)) throw new Error(`KNIFE_DELIVERY_SOCKET_MISSING: ${socketId}`)
  }

  return {
    root,
    partMeshes,
    partPivots,
    sockets,
    metadata,
    setExploded(amount: number): void {
      if (!Number.isFinite(amount) || amount < 0 || amount > 1) {
        throw new Error('KNIFE_DELIVERY_EXPLODE_RANGE: amount must be finite and within 0..1')
      }
      for (const pivot of partPivots.values()) {
        const rest = tupleValue(pivot.userData.rest_position, 'rest_position')
        const vector = tupleValue(pivot.userData.explode_vector, 'explode_vector')
        pivot.position.set(
          rest[0] + vector[0] * amount,
          rest[1] + vector[1] * amount,
          rest[2] + vector[2] * amount,
        )
      }
      root.updateMatrixWorld(true)
    },
    setPartVisible(partId: string, visible: boolean): void {
      const pivot = partPivots.get(partId)
      if (!pivot) throw new Error(`KNIFE_DELIVERY_UNKNOWN_PART: ${partId}`)
      pivot.visible = visible
    },
    resolvePart(object: THREE.Object3D | null): string | null {
      for (let cursor = object; cursor; cursor = cursor.parent) {
        const partId = stringValue(cursor.userData.part_id)
        if (partId && partMeshes.has(partId)) return partId
        if (cursor === root) break
      }
      return null
    },
  }
}

function buildSockets(program: KnifeSceneProgram): readonly KnifeDeliverySocket[] {
  const guard = program.assembly?.guard?.center ?? ([-1, 0, 0] as const)
  const grip = program.assembly?.grip?.center ?? ([-1.45, 0, 0] as const)
  const spineTip = program.blade_surface.spine_curve.control_points.at(-1)!
  const edgeTip = program.blade_surface.cutting_edge_curve.control_points.at(-1)!
  const bladeTip: KnifeVec3 = [
    (spineTip[0] + edgeTip[0]) / 2,
    (spineTip[1] + edgeTip[1]) / 2,
    (spineTip[2] + edgeTip[2]) / 2,
  ]
  return [
    { socket_id: 'socket-blade-tip', node_name: 'knife-socket:socket-blade-tip', position: bladeTip },
    { socket_id: 'socket-guard', node_name: 'knife-socket:socket-guard', position: [...guard] as KnifeVec3 },
    { socket_id: 'socket-grip', node_name: 'knife-socket:socket-grip', position: [...grip] as KnifeVec3 },
  ]
}

function colliderIntent(
  root: THREE.Group,
  colliderId: KnifeColliderIntent['collider_id'],
  partIds: readonly string[],
): KnifeColliderIntent {
  const bounds = new THREE.Box3()
  for (const partId of partIds) {
    const pivot = root.getObjectByName(`knife-pivot:${partId}`)
    if (!pivot) throw new Error(`KNIFE_DELIVERY_COLLIDER_PART_MISSING: ${partId}`)
    bounds.union(new THREE.Box3().setFromObject(pivot))
  }
  if (bounds.isEmpty()) throw new Error(`KNIFE_DELIVERY_COLLIDER_EMPTY: ${colliderId}`)
  const center = bounds.getCenter(new THREE.Vector3())
  const halfExtents = bounds.getSize(new THREE.Vector3()).multiplyScalar(0.5)
  return {
    collider_id: colliderId,
    shape: 'box-intent@1',
    part_ids: partIds,
    center: vectorTuple(center),
    half_extents: vectorTuple(halfExtents),
  }
}

function stringValue(value: unknown): string | null {
  return typeof value === 'string' && value.length > 0 ? value : null
}

function tupleValue(value: unknown, field: string): KnifeVec3 {
  if (!Array.isArray(value) || value.length !== 3 || value.some((entry) => typeof entry !== 'number' || !Number.isFinite(entry))) {
    throw new Error(`KNIFE_DELIVERY_INVALID_VECTOR: ${field}`)
  }
  return [value[0], value[1], value[2]]
}

function vectorTuple(value: THREE.Vector3): KnifeVec3 {
  return [value.x, value.y, value.z]
}
