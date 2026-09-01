import * as THREE from 'three'

import type { KnifeViewId, KnifeViewDescriptor } from './knife-view-evaluation.ts'
import type { KnifePreviewManifest } from './knife-preview-manifest.ts'

export interface KnifePreviewViewport {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

export interface KnifeBrowserCameraReceipt {
  readonly view_id: KnifeViewId
  readonly uuid: string
  readonly name: string
  readonly type: string
  readonly position: readonly [number, number, number]
  readonly matrix: readonly number[]
  readonly matrix_world: readonly number[]
  readonly matrix_world_inverse: readonly number[]
  readonly projection_matrix: readonly number[]
  readonly viewport: KnifePreviewViewport
  readonly deterministic_fingerprint: string
}

export interface KnifeBrowserViewReceipt {
  readonly view_id: KnifeViewId
  readonly viewport: KnifePreviewViewport
  readonly camera: KnifeBrowserCameraReceipt
  readonly render_status: 'RENDERED'
  readonly renderer_invoked: true
  readonly quality_status: 'NOT_RUN'
}

export interface KnifeBrowserPreviewReceipt {
  readonly schema_version: 'WeaponryThreeJsBrowserPreviewReceipt@2'
  readonly route: 'weaponry-threejs-knife-preview@1'
  readonly asset_id: string
  readonly source_fingerprint: string
  readonly scene_fingerprint: string
  readonly rig_schema_version: 'KnifeFixedEightViewRig@1'
  readonly rig_id: 'knife-fixed-eight-view@1'
  readonly rig_fingerprint: string
  readonly manifest: KnifePreviewManifest
  readonly selected_view_ids: readonly KnifeViewId[]
  readonly views: readonly KnifeBrowserViewReceipt[]
  readonly render_target: {
    readonly width: number
    readonly height: number
    readonly pixel_ratio: number
  }
  readonly renderer: {
    readonly type: 'THREE.WebGLRenderer'
    readonly three_revision: string
    readonly antialias: true
  }
  readonly renderer_invoked: true
  readonly network_policy: 'bundled-static-only@1'
  readonly external_network_used: false
  readonly render_status: 'RENDERED'
  readonly capture_status: 'CAPTURE_READY' | 'SETTLED'
  readonly capture_ready: true
  readonly settled: boolean
  readonly visual_status: 'NOT_REVIEWED'
  readonly quality_status: 'NOT_RUN'
  readonly deterministic_fingerprint: string
}

export function makeKnifeBrowserCameraReceipt(
  camera: THREE.Camera,
  view: KnifeViewDescriptor,
  viewport: KnifePreviewViewport,
): KnifeBrowserCameraReceipt {
  camera.updateMatrixWorld(true)
  const matrix = matrixValues(camera.matrix)
  const matrixWorld = matrixValues(camera.matrixWorld)
  const matrixWorldInverse = matrixValues(camera.matrixWorldInverse)
  const projectionMatrix = matrixValues(camera.projectionMatrix)
  const position = Object.freeze([camera.position.x, camera.position.y, camera.position.z]) as readonly [number, number, number]
  const fingerprint = fnv1a64([
    view.view_id,
    camera.uuid,
    camera.name,
    camera.type,
    position.map(canonicalNumber).join(','),
    matrix.map(canonicalNumber).join(','),
    matrixWorld.map(canonicalNumber).join(','),
    matrixWorldInverse.map(canonicalNumber).join(','),
    projectionMatrix.map(canonicalNumber).join(','),
    `${viewport.x},${viewport.y},${viewport.width},${viewport.height}`,
  ].join('|'))
  return Object.freeze({
    view_id: view.view_id,
    uuid: camera.uuid,
    name: camera.name,
    type: camera.type,
    position,
    matrix,
    matrix_world: matrixWorld,
    matrix_world_inverse: matrixWorldInverse,
    projection_matrix: projectionMatrix,
    viewport: Object.freeze({ ...viewport }),
    deterministic_fingerprint: fingerprint,
  })
}

/**
 * Fingerprint the actual Three.js scene graph after transforms are resolved.
 * Geometry content is bound by source_fingerprint; object IDs, matrices,
 * semantic metadata, and attribute layout come from the live scene objects.
 */
export function fingerprintKnifePreviewScene(scene: THREE.Scene, sourceFingerprint: string): string {
  scene.updateMatrixWorld(true)
  const values: string[] = [sourceFingerprint, scene.type, scene.name, scene.uuid, ...matrixValues(scene.matrixWorld).map(canonicalNumber)]
  scene.traverse((object) => {
    values.push(
      'object',
      object.type,
      object.name,
      object.uuid,
      ...matrixValues(object.matrixWorld).map(canonicalNumber),
      stableValue(object.userData),
    )
    const mesh = object as THREE.Mesh
    if (!mesh.isMesh || !(mesh.geometry instanceof THREE.BufferGeometry)) return
    const attributeNames = Object.keys(mesh.geometry.attributes).sort()
    values.push('geometry', mesh.geometry.uuid, ...attributeNames.map((name) => {
      const attribute = mesh.geometry.getAttribute(name)
      return `${name}:${attribute.itemSize}:${attribute.count}:${attribute.normalized ? 1 : 0}`
    }))
    const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material]
    values.push('materials', ...materials.map((material) => `${material.type}:${material.name}:${stableValue(material.userData)}`))
  })
  return fnv1a64(values.join('|'))
}

export function hashKnifeBrowserPreviewReceipt(
  sourceFingerprint: string,
  sceneFingerprint: string,
  rigFingerprint: string,
  views: readonly KnifeBrowserViewReceipt[],
): string {
  return fnv1a64([
    sourceFingerprint,
    sceneFingerprint,
    rigFingerprint,
    ...views.map((view) => `${view.view_id}:${view.camera.deterministic_fingerprint}:${view.render_status}`),
  ].join('|'))
}

export function stableKnifePreviewUuid(value: string): string {
  const raw = `${fnv1a64(`${value}:0`)}${fnv1a64(`${value}:1`)}${fnv1a64(`${value}:2`)}${fnv1a64(`${value}:3`)}`
  return `${raw.slice(0, 8)}-${raw.slice(8, 12)}-${raw.slice(12, 16)}-${raw.slice(16, 20)}-${raw.slice(20, 32)}`
}

export function overrideKnifePreviewUuid(object: { readonly uuid: string }, uuid: string): void {
  Object.defineProperty(object, 'uuid', { configurable: true, enumerable: true, value: uuid, writable: true })
}

function matrixValues(matrix: THREE.Matrix4): readonly number[] {
  return Object.freeze(Array.from(matrix.elements))
}

function stableValue(value: unknown): string {
  if (value === null) return 'null'
  if (typeof value === 'string') return JSON.stringify(value)
  if (typeof value === 'number') return canonicalNumber(value)
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (Array.isArray(value)) return `[${value.map(stableValue).join(',')}]`
  if (typeof value === 'object') {
    const record = value as Record<string, unknown>
    return `{${Object.keys(record).sort().map((key) => `${JSON.stringify(key)}:${stableValue(record[key])}`).join(',')}}`
  }
  return String(value)
}

function canonicalNumber(value: number): string {
  if (!Number.isFinite(value)) return value === Number.POSITIVE_INFINITY ? 'INF' : 'NAN'
  return Object.is(value, -0) ? '0' : value.toPrecision(12)
}

function fnv1a64(value: string): string {
  let hash = 0xcbf29ce484222325n
  const prime = 0x100000001b3n
  const mask = 0xffffffffffffffffn
  for (let index = 0; index < value.length; index += 1) {
    hash ^= BigInt(value.charCodeAt(index))
    hash = (hash * prime) & mask
  }
  return hash.toString(16).padStart(16, '0')
}
