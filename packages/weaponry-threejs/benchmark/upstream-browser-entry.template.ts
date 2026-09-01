import * as THREE from 'three'

import normalizationContract from './upstream-render-normalization.contract.json' with { type: 'json' }
import { createDragonfangLikeBaselineModel } from './generated/DragonfangLikeBaseline.ts'

/**
 * Temporary browser baseline entry. The generated factory is copied into the
 * sibling `generated/` directory by the benchmark runner and is never stored
 * in the product tree. This module performs scene construction only; a live
 * WebGL renderer and PNG/AOV capture remain an explicit caller boundary.
 */

export const UPSTREAM_RENDER_NORMALIZATION_CONTRACT = normalizationContract
export const UPSTREAM_FIXED_VIEW_IDS = Object.freeze(
  normalizationContract.fixed_view_rig.views.map((view) => view.view_id),
)
export const UPSTREAM_REQUIRED_AOV_IDS = Object.freeze([...normalizationContract.aov_contract.required])
export const UPSTREAM_OPTIONAL_AOV_IDS = Object.freeze([...normalizationContract.aov_contract.optional])

const EPSILON = normalizationContract.scene_normalization.epsilon

export interface UpstreamBaselineScene {
  readonly scene: THREE.Scene
  readonly root: THREE.Group
  readonly source_root: THREE.Group
  readonly bounds_before: THREE.Box3
  readonly bounds_after: THREE.Box3
  readonly source_center: THREE.Vector3
  readonly source_size: THREE.Vector3
  readonly uniform_scale: number
}

export interface UpstreamFixedViewRig {
  readonly schema_version: 'KnifeFixedEightViewRig@1'
  readonly rig_id: 'knife-fixed-eight-view@1'
  readonly coordinate_convention: 'weapon-front-z-up-right-handed@1'
  readonly frame_width: number
  readonly frame_height: number
  readonly margin: number
  readonly views: readonly UpstreamFixedViewDescriptor[]
  readonly deterministic_fingerprint: string
}

export interface UpstreamFixedViewDescriptor {
  readonly view_id: string
  readonly projection: 'orthographic' | 'perspective'
  readonly position: readonly [number, number, number]
  readonly target: readonly [number, number, number]
  readonly up: readonly [number, number, number]
  readonly near: number
  readonly far: number
  readonly ortho_height?: number
  readonly fov_degrees?: number
}

export function createUpstreamBaselineScene(): UpstreamBaselineScene {
  const sourceRoot = createDragonfangLikeBaselineModel({
    castShadow: false,
    receiveShadow: false,
  })
  sourceRoot.updateMatrixWorld(true)

  const boundsBefore = new THREE.Box3().setFromObject(sourceRoot)
  const sourceCenter = boundsBefore.getCenter(new THREE.Vector3())
  const sourceSize = boundsBefore.getSize(new THREE.Vector3())
  const sourceExtent = Math.max(sourceSize.x, sourceSize.y, sourceSize.z)
  if (!Number.isFinite(sourceExtent) || sourceExtent <= EPSILON) {
    throw new Error('img2threejs baseline factory produced an empty or non-finite bounds')
  }

  const targetExtent = normalizationContract.scene_normalization.target_max_extent
  const uniformScale = targetExtent / sourceExtent
  const root = new THREE.Group()
  root.name = 'img2threejs-baseline-normalized'
  root.scale.setScalar(uniformScale)
  root.position.copy(sourceCenter).multiplyScalar(-uniformScale)
  root.add(sourceRoot)
  assignStableObjectIds(root, 'upstream-baseline')

  const scene = new THREE.Scene()
  scene.name = 'img2threejs-baseline-scene'
  scene.add(root)
  assignStableObjectIds(scene, 'upstream-scene')
  scene.updateMatrixWorld(true)
  const boundsAfter = new THREE.Box3().setFromObject(root)
  scene.userData = {
    schema_version: 'WeaponryThreeJsUpstreamBrowserBaseline@1',
    contract_id: normalizationContract.contract_id,
    source_revision: normalizationContract.source.revision,
    normalization: 'center_then_uniform_max_extent',
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  }

  return {
    scene,
    root,
    source_root: sourceRoot,
    bounds_before: boundsBefore,
    bounds_after: boundsAfter,
    source_center: sourceCenter,
    source_size: sourceSize,
    uniform_scale: uniformScale,
  }
}

export function createUpstreamFixedViewRig(): UpstreamFixedViewRig {
  const source = normalizationContract.fixed_view_rig
  const views = source.views.map((view) => ({
    view_id: view.view_id,
    projection: view.projection,
    position: Object.freeze([...view.position]) as [number, number, number],
    target: Object.freeze([...view.target]) as [number, number, number],
    up: Object.freeze([...view.up]) as [number, number, number],
    near: view.near,
    far: view.far,
    ...(view.ortho_height === undefined ? {} : { ortho_height: view.ortho_height }),
    ...(view.fov_degrees === undefined ? {} : { fov_degrees: view.fov_degrees }),
  })) as readonly UpstreamFixedViewDescriptor[]
  const rigWithoutFingerprint = {
    schema_version: source.schema_version,
    rig_id: source.rig_id,
    coordinate_convention: source.coordinate_convention,
    frame_width: source.frame_width,
    frame_height: source.frame_height,
    margin: source.margin,
    views,
  }
  return Object.freeze({
    ...rigWithoutFingerprint,
    deterministic_fingerprint: hashRig(rigWithoutFingerprint),
  }) as UpstreamFixedViewRig
}

export function createUpstreamFixedCameras(
  rig: UpstreamFixedViewRig = createUpstreamFixedViewRig(),
): readonly THREE.Camera[] {
  const aspect = rig.frame_width / rig.frame_height
  return Object.freeze(rig.views.map((view) => {
    const effectiveOrthoHeight = view.ortho_height === undefined
      ? undefined
      : view.ortho_height * (1 + rig.margin * 2)
    const camera = view.projection === 'orthographic'
      ? new THREE.OrthographicCamera(
          -(effectiveOrthoHeight as number) * aspect * 0.5,
          (effectiveOrthoHeight as number) * aspect * 0.5,
          (effectiveOrthoHeight as number) * 0.5,
          -(effectiveOrthoHeight as number) * 0.5,
          view.near,
          view.far,
        )
      : new THREE.PerspectiveCamera(view.fov_degrees as number, aspect, view.near, view.far)
    camera.name = `knife-camera:${view.view_id}`
    overrideUuid(camera, stableUuid(`camera:${rig.deterministic_fingerprint}:${view.view_id}`))
    camera.position.fromArray(view.position)
    camera.up.fromArray(view.up)
    camera.lookAt(new THREE.Vector3(...view.target))
    camera.updateProjectionMatrix()
    camera.updateMatrixWorld(true)
    camera.userData = {
      schema_version: 'KnifeFixedEightViewRig@1',
      rig_id: rig.rig_id,
      rig_fingerprint: rig.deterministic_fingerprint,
      view_id: view.view_id,
      projection: view.projection,
      renderer_invoked: false,
      quality_status: 'NOT_RUN',
    }
    return camera
  }))
}

export function boundsSummary(bounds: THREE.Box3): {
  readonly min: readonly [number, number, number]
  readonly max: readonly [number, number, number]
  readonly center: readonly [number, number, number]
  readonly size: readonly [number, number, number]
  readonly max_extent: number
} {
  const center = bounds.getCenter(new THREE.Vector3())
  const size = bounds.getSize(new THREE.Vector3())
  return {
    min: [bounds.min.x, bounds.min.y, bounds.min.z],
    max: [bounds.max.x, bounds.max.y, bounds.max.z],
    center: [center.x, center.y, center.z],
    size: [size.x, size.y, size.z],
    max_extent: Math.max(size.x, size.y, size.z),
  }
}

export function stableObjectPathIds(root: THREE.Object3D): readonly string[] {
  const ids: string[] = []
  root.traverse((object) => ids.push(object.uuid))
  return Object.freeze(ids)
}

export function stableSceneIdentity(root: THREE.Object3D): readonly string[] {
  const ids: string[] = []
  root.traverse((object) => {
    ids.push(`object:${object.uuid}`)
    const mesh = object as THREE.Mesh
    if (!mesh.isMesh || !(mesh.geometry instanceof THREE.BufferGeometry)) return
    ids.push(`geometry:${mesh.geometry.uuid}`)
    const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material]
    for (const material of materials) ids.push(`material:${material.uuid}`)
  })
  return Object.freeze(ids)
}

function assignStableObjectIds(root: THREE.Object3D, namespace: string): void {
  let ordinal = 0
  root.traverse((object) => {
    const component = object.userData?.sculptComponent
    const semantic = typeof component?.id === 'string'
      ? component.id
      : object.name || object.type
    overrideUuid(object, stableUuid(`${namespace}:${semantic}:${ordinal}`))
    const mesh = object as THREE.Mesh
    if (mesh.isMesh && mesh.geometry instanceof THREE.BufferGeometry) {
      overrideUuid(mesh.geometry, stableUuid(`${namespace}:geometry:${semantic}:${ordinal}`))
      const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material]
      materials.forEach((material, materialIndex) => {
        overrideUuid(material, stableUuid(`${namespace}:material:${semantic}:${ordinal}:${materialIndex}`))
      })
    }
    ordinal += 1
  })
}

function hashRig(rig: {
  readonly schema_version: string
  readonly rig_id: string
  readonly coordinate_convention: string
  readonly frame_width: number
  readonly frame_height: number
  readonly margin: number
  readonly views: readonly UpstreamFixedViewDescriptor[]
}): string {
  const values = [
    rig.schema_version,
    rig.rig_id,
    rig.coordinate_convention,
    `${rig.frame_width}x${rig.frame_height}`,
    canonicalNumber(rig.margin),
  ]
  for (const view of rig.views) {
    values.push(
      view.view_id,
      view.projection,
      view.position.join(','),
      view.target.join(','),
      view.up.join(','),
      canonicalNumber(view.near),
      canonicalNumber(view.far),
      canonicalNumber(view.ortho_height ?? 0),
      canonicalNumber(view.fov_degrees ?? 0),
    )
  }
  return fnv1a64(values.join('|'))
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

function stableUuid(value: string): string {
  const raw = `${fnv1a64(`${value}:0`)}${fnv1a64(`${value}:1`)}${fnv1a64(`${value}:2`)}${fnv1a64(`${value}:3`)}`
  return `${raw.slice(0, 8)}-${raw.slice(8, 12)}-${raw.slice(12, 16)}-${raw.slice(16, 20)}-${raw.slice(20, 32)}`
}

function overrideUuid(object: { readonly uuid: string }, uuid: string): void {
  Object.defineProperty(object, 'uuid', { configurable: true, enumerable: true, value: uuid, writable: true })
}
