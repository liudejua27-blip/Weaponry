import * as THREE from 'three'

/**
 * Narrow structural surface consumed by the existing browser capture path.
 * This is deliberately a benchmark-local shape instead of a product export:
 * the upstream scene remains an isolated source fixture and never becomes
 * Runtime/Store/CAS truth.
 */
export interface Img2ThreeJsCompiledPart {
  readonly part_id: string
  readonly material_zone_id: string
  readonly surface_role: string
  readonly assembly_primitive?: string
  readonly center: readonly [number, number, number]
  readonly mesh: THREE.Mesh
  readonly geometry: THREE.BufferGeometry
  readonly material: THREE.MeshStandardMaterial
}

export interface Img2ThreeJsCompiledScene {
  readonly group: THREE.Group
  readonly parts: readonly Img2ThreeJsCompiledPart[]
  readonly assembly_parts: readonly Img2ThreeJsCompiledPart[]
  readonly assembly_status: 'COMPILED'
  readonly sections: readonly []
  readonly triangle_count: number
  readonly longitudinal_segments: 0
  readonly deterministic_fingerprint: string
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
}

export interface Img2ThreeJsAdapterOptions {
  /** Frozen generated factory hash; this is provenance, not a Runtime hash. */
  readonly source_fingerprint: string
  readonly group_name?: string
}

export class Img2ThreeJsAdapterError extends Error {
  constructor(message: string) {
    super(`IMG2THREEJS_COMPILED_ADAPTER_INVALID: ${message}`)
    this.name = 'Img2ThreeJsAdapterError'
  }
}

const SHA256_PATTERN = /^[a-f0-9]{64}$/
const STABLE_ID_PATTERN = /^[a-z][a-z0-9_.-]{0,63}$/
const MAX_PARTS = 64
const MAX_TRIANGLES = 200_000

const SURFACE_ROLE_BY_UPSTREAM_ROLE: Readonly<Record<string, string>> = {
  blade: 'blade-body',
  guard: 'guard',
  grip: 'grip',
  pommel: 'pommel',
  fastener: 'fastener',
  gem: 'gem',
  relief: 'relief',
}

const ASSEMBLY_PRIMITIVES = new Set(['guard', 'grip', 'pommel', 'fastener', 'gem', 'relief'])

/**
 * Adapt only the closed img2threejs sculptComponent vocabulary into the
 * minimum object shape validated by captureKnifeAovs. Every renderable mesh
 * must carry exactly one stable part ID and one stable material ID.
 */
export function adaptImg2ThreeJsGroupToCompiledScene(
  sourceRoot: THREE.Group,
  options: Img2ThreeJsAdapterOptions,
): Img2ThreeJsCompiledScene {
  if (!sourceRoot || sourceRoot.isObject3D !== true) throw new Img2ThreeJsAdapterError('a THREE.Object3D source root is required')
  if (!options || !SHA256_PATTERN.test(options.source_fingerprint)) throw new Img2ThreeJsAdapterError('source_fingerprint must be a SHA-256')

  sourceRoot.updateMatrixWorld(true)
  const sourceMeshes: THREE.Mesh[] = []
  sourceRoot.traverse((object) => {
    if (object.isMesh) sourceMeshes.push(object as THREE.Mesh)
  })
  if (sourceMeshes.length < 1 || sourceMeshes.length > MAX_PARTS) {
    throw new Img2ThreeJsAdapterError(`source mesh count must be in [1, ${MAX_PARTS}]`)
  }

  const group = new THREE.Group()
  group.name = options.group_name ?? 'img2threejs-compiled-scene'
  overrideUuid(group, stableUuid(`group:${options.source_fingerprint}`))
  const parts: Img2ThreeJsCompiledPart[] = []
  const partIds = new Set<string>()
  const materialIds = new Set<string>()
  const materialObjects = new Map<THREE.Material, string>()
  let triangleCount = 0

  for (const [index, mesh] of sourceMeshes.entries()) {
    const component = readComponent(mesh)
    if (partIds.has(component.id)) throw new Img2ThreeJsAdapterError(`duplicate sculptComponent id ${component.id}`)
    partIds.add(component.id)
    materialIds.add(component.material)

    const geometry = mesh.geometry
    if (!(geometry instanceof THREE.BufferGeometry)) throw new Img2ThreeJsAdapterError(`part ${component.id} has no BufferGeometry`)
    const position = geometry.getAttribute('position')
    if (!position || position.itemSize !== 3 || position.count < 3) throw new Img2ThreeJsAdapterError(`part ${component.id} has no bounded position attribute`)
    const indexAttribute = geometry.getIndex()
    const indexCount = indexAttribute ? indexAttribute.count : position.count
    if (indexCount <= 0 || indexCount % 3 !== 0) throw new Img2ThreeJsAdapterError(`part ${component.id} has a non-triangular index/position count`)
    const triangles = indexCount / 3
    triangleCount += triangles
    if (triangleCount > MAX_TRIANGLES) throw new Img2ThreeJsAdapterError(`triangle budget exceeds ${MAX_TRIANGLES}`)

    const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material]
    if (materials.length !== 1 || !(materials[0] instanceof THREE.MeshStandardMaterial)) {
      throw new Img2ThreeJsAdapterError(`part ${component.id} must bind exactly one MeshStandardMaterial-compatible material`)
    }
    const material = materials[0]
    const sourceMaterial = material.userData?.sculptMaterial
    if (!sourceMaterial || sourceMaterial.id !== component.material) {
      throw new Img2ThreeJsAdapterError(`part ${component.id} material metadata does not match ${component.material}`)
    }
    const existingMaterialId = materialObjects.get(material)
    if (existingMaterialId !== undefined && existingMaterialId !== component.material) {
      throw new Img2ThreeJsAdapterError(`material object is bound to multiple IDs: ${existingMaterialId}, ${component.material}`)
    }

    const worldMatrix = mesh.matrixWorld.clone()
    group.add(mesh)
    mesh.matrixAutoUpdate = false
    mesh.matrix.copy(worldMatrix)
    overrideUuid(mesh, stableUuid(`mesh:${options.source_fingerprint}:${component.id}`))
    overrideUuid(geometry, stableUuid(`geometry:${options.source_fingerprint}:${component.id}`))
    const materialUuid = materialObjects.get(material) ?? stableUuid(`material:${options.source_fingerprint}:${component.material}`)
    materialObjects.set(material, component.material)
    overrideUuid(material, materialUuid)
    mesh.updateMatrixWorld(true)

    const bounds = new THREE.Box3().setFromObject(mesh)
    const center = bounds.getCenter(new THREE.Vector3())
    const surfaceRole = SURFACE_ROLE_BY_UPSTREAM_ROLE[component.role]
    if (!surfaceRole) throw new Img2ThreeJsAdapterError(`unsupported sculptComponent role ${component.role}`)
    const assemblyPrimitive = ASSEMBLY_PRIMITIVES.has(component.role) ? component.role : undefined
    parts.push({
      part_id: component.id,
      material_zone_id: component.material,
      surface_role: surfaceRole,
      ...(assemblyPrimitive === undefined ? {} : { assembly_primitive: assemblyPrimitive }),
      center: [center.x, center.y, center.z],
      mesh,
      geometry,
      material,
    })
  }

  group.userData = {
    schema_version: 'WeaponryThreeJsUpstreamCompiledScene@1',
    adapter: 'img2threejs-raw-group-to-compiled-scene@1',
    source_fingerprint: options.source_fingerprint,
    part_ids: parts.map((part) => part.part_id),
    material_zone_ids: [...materialIds].sort(),
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  }
  group.updateMatrixWorld(true)
  const assemblyParts = parts.filter((part) => part.assembly_primitive !== undefined)
  return {
    group,
    parts: Object.freeze(parts),
    assembly_parts: Object.freeze(assemblyParts),
    assembly_status: 'COMPILED',
    sections: Object.freeze([]) as readonly [],
    triangle_count: triangleCount,
    longitudinal_segments: 0,
    deterministic_fingerprint: fingerprint(options.source_fingerprint, parts),
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  }
}

interface SculptComponent {
  readonly id: string
  readonly role: string
  readonly primitive: string
  readonly material: string
}

function readComponent(mesh: THREE.Mesh): SculptComponent {
  const component = mesh.userData?.sculptComponent
  if (!component || typeof component !== 'object') throw new Img2ThreeJsAdapterError(`mesh ${mesh.name || '(unnamed)'} has no sculptComponent metadata`)
  const id = component.id
  const role = component.role
  const primitive = component.primitive
  const material = component.material
  if (!isStableId(id) || typeof role !== 'string' || typeof primitive !== 'string' || !isStableId(material)) {
    throw new Img2ThreeJsAdapterError(`mesh ${mesh.name || '(unnamed)'} has incomplete bounded sculptComponent metadata`)
  }
  return { id, role, primitive, material }
}

function isStableId(value: unknown): value is string {
  return typeof value === 'string' && STABLE_ID_PATTERN.test(value)
}

function fingerprint(sourceFingerprint: string, parts: readonly Img2ThreeJsCompiledPart[]): string {
  const values = ['img2threejs-raw-group-to-compiled-scene@1', sourceFingerprint]
  for (const part of parts) {
    values.push(part.part_id, part.material_zone_id, part.surface_role, part.assembly_primitive ?? '')
    values.push(...part.mesh.matrix.toArray().map(canonicalNumber))
    const position = part.geometry.getAttribute('position')
    const normal = part.geometry.getAttribute('normal')
    const uv = part.geometry.getAttribute('uv')
    values.push(`position:${position.count}:${position.itemSize}`, `normal:${normal?.count ?? 0}:${normal?.itemSize ?? 0}`, `uv:${uv?.count ?? 0}:${uv?.itemSize ?? 0}`)
    const index = part.geometry.getIndex()
    values.push(`index:${index?.count ?? 0}`)
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

function overrideUuid(object: { readonly uuid: string }, uuid: string): void {
  Object.defineProperty(object, 'uuid', { configurable: true, enumerable: true, value: uuid, writable: true })
}

function stableUuid(value: string): string {
  const raw = `${fnv1a64(`${value}:0`)}${fnv1a64(`${value}:1`)}${fnv1a64(`${value}:2`)}${fnv1a64(`${value}:3`)}`
  return `${raw.slice(0, 8)}-${raw.slice(8, 12)}-${raw.slice(12, 16)}-${raw.slice(16, 20)}-${raw.slice(20, 32)}`
}
