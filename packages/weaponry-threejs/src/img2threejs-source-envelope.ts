/**
 * Closed, first-party projection of the pinned img2threejs ObjectSculptSpec
 * data surface.  This is deliberately data-only: it carries no source code,
 * URL, path, shader, or executable callback.
 */

export const IMG2THREEJS_SOURCE_ENVELOPE_SCHEMA = 'Img2ThreeJsSourceEnvelope@1' as const
export type Img2ThreeJsSourceEnvelopeSchema = typeof IMG2THREEJS_SOURCE_ENVELOPE_SCHEMA

export const IMG2THREEJS_SOURCE_COORDINATE_FRAME = 'source-right-x-up-y-forward-z@1' as const
export type Img2ThreeJsSourceCoordinateFrame = typeof IMG2THREEJS_SOURCE_COORDINATE_FRAME

/**
 * Identity of the pinned upstream source used by this compatibility envelope.
 * These are source-control/content hashes, never a mutable checkout path.
 */
export interface Img2ThreeJsSourceIdentity {
  readonly revision: string
  readonly tree: string
  readonly generator_sha256: string
  readonly validator_sha256: string
}

export const IMG2THREEJS_SOURCE_IDENTITY: Img2ThreeJsSourceIdentity = Object.freeze({
  revision: '9fbd0ca5bbcc3b13bebe712745d6784d33db0b85',
  tree: '0ee3c2a6d781407808df98b33174539842f85fcc',
  generator_sha256: 'b090d5258a8009570ef24d6de2b84651c8bcc5e7c8b065905e47755f63acdbbb',
  validator_sha256: '734b9c502df3f0e711e13cc0ae0f4da7a7dada611d52dcf4a162b7148419efde',
})

export type Img2ThreeJsSourceVec2 = readonly [number, number]
export type Img2ThreeJsSourceVec3 = readonly [number, number, number]

export type Img2ThreeJsSourcePrimitive =
  | 'ground-blade'
  | 'extrude'
  | 'curve-sweep'
  | 'sphere'
  | 'cylinder'

export type Img2ThreeJsSourceRole = 'blade' | 'guard' | 'grip' | 'pommel' | 'fastener' | 'gem' | 'relief'

export interface Img2ThreeJsSourceTransform {
  readonly position: Img2ThreeJsSourceVec3
  readonly rotation_xyz: Img2ThreeJsSourceVec3
  readonly scale: Img2ThreeJsSourceVec3
  readonly pivot: Img2ThreeJsSourceVec3
  readonly rotation_order: 'XYZ'
}

export interface Img2ThreeJsSourceBladeGeometry {
  readonly primitive: 'ground-blade'
  readonly stations: readonly Img2ThreeJsSourceVec3[]
  readonly thickness: number
  readonly grind_frac: number
  readonly swedge_from_tip_frac: number
}

export interface Img2ThreeJsSourceProfileGeometry {
  readonly primitive: 'extrude'
  readonly profile_2d: readonly Img2ThreeJsSourceVec2[]
  readonly depth: number
}

export interface Img2ThreeJsSourceCurveSweepGeometry {
  readonly primitive: 'curve-sweep'
  readonly spine: readonly Img2ThreeJsSourceVec3[]
  readonly cross_section: readonly Img2ThreeJsSourceVec2[]
  readonly closed: boolean
}

export interface Img2ThreeJsSourceUnitGeometry {
  readonly primitive: 'sphere' | 'cylinder'
}

export type Img2ThreeJsSourceGeometry =
  | Img2ThreeJsSourceBladeGeometry
  | Img2ThreeJsSourceProfileGeometry
  | Img2ThreeJsSourceCurveSweepGeometry
  | Img2ThreeJsSourceUnitGeometry

export interface Img2ThreeJsSourceMaterial {
  readonly material_id: string
  readonly source_order: number
  readonly base_color: string
  readonly metalness: number
  readonly roughness: number
}

export interface Img2ThreeJsSourceComponent {
  readonly component_id: string
  readonly source_order: number
  readonly role: Img2ThreeJsSourceRole
  readonly primitive: Img2ThreeJsSourcePrimitive
  readonly material_id: string
  readonly parent_id: string | null
  readonly transform: Img2ThreeJsSourceTransform
  readonly geometry: Img2ThreeJsSourceGeometry
}

export type Img2ThreeJsTessellationTier = 'low' | 'standard' | 'hero'

export interface Img2ThreeJsSourceEnvelope {
  readonly schema_version: Img2ThreeJsSourceEnvelopeSchema
  readonly source_schema_version: string
  readonly source_identity: Img2ThreeJsSourceIdentity
  readonly target_name: string
  readonly coordinate_frame: Img2ThreeJsSourceCoordinateFrame
  readonly components: readonly Img2ThreeJsSourceComponent[]
  readonly materials: readonly Img2ThreeJsSourceMaterial[]
  readonly tessellation: Img2ThreeJsTessellationTier
  readonly max_triangles: number
}

export class Img2ThreeJsSourceEnvelopeError extends Error {
  constructor(message: string) {
    super(`INVALID_IMG2THREEJS_SOURCE_ENVELOPE: ${message}`)
    this.name = 'Img2ThreeJsSourceEnvelopeError'
  }
}

const STABLE_ID_PATTERN = /^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/
const MAX_POSITION = 4
const MAX_ROTATION = Math.PI * 2
const MIN_SCALE = 1e-4
const MAX_SCALE = 4
const MAX_COMPONENTS = 64
const MAX_MATERIALS = 64
const MAX_POINTS = 64

function exactKeys(value: Record<string, unknown>, expected: readonly string[], label: string): void {
  const actual = Object.keys(value).sort()
  const wanted = [...expected].sort()
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) {
    throw new Img2ThreeJsSourceEnvelopeError(`${label} contains unknown or missing keys`)
  }
}

/** Validate the closed envelope before a compatibility compiler consumes it. */
export function validateImg2ThreeJsSourceEnvelope(value: unknown): asserts value is Img2ThreeJsSourceEnvelope {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Img2ThreeJsSourceEnvelopeError('envelope must be an object')
  const envelope = value as Record<string, unknown>
  exactKeys(envelope, ['schema_version', 'source_schema_version', 'source_identity', 'target_name', 'coordinate_frame', 'components', 'materials', 'tessellation', 'max_triangles'], 'envelope')
  if (envelope.schema_version !== IMG2THREEJS_SOURCE_ENVELOPE_SCHEMA) throw new Img2ThreeJsSourceEnvelopeError('schema_version drifted')
  if (typeof envelope.source_schema_version !== 'string' || envelope.source_schema_version.length === 0 || envelope.source_schema_version.length > 64) {
    throw new Img2ThreeJsSourceEnvelopeError('source_schema_version must be bounded text')
  }
  validateSourceIdentity(envelope.source_identity)
  if (typeof envelope.target_name !== 'string' || envelope.target_name.length === 0 || envelope.target_name.length > 160) {
    throw new Img2ThreeJsSourceEnvelopeError('target_name must be bounded text')
  }
  if (envelope.coordinate_frame !== IMG2THREEJS_SOURCE_COORDINATE_FRAME) throw new Img2ThreeJsSourceEnvelopeError('coordinate_frame is unsupported')
  if (!Array.isArray(envelope.components) || envelope.components.length === 0 || envelope.components.length > MAX_COMPONENTS) {
    throw new Img2ThreeJsSourceEnvelopeError(`components must contain 1 to ${MAX_COMPONENTS} entries`)
  }
  if (!Array.isArray(envelope.materials) || envelope.materials.length === 0 || envelope.materials.length > MAX_MATERIALS) {
    throw new Img2ThreeJsSourceEnvelopeError(`materials must contain 1 to ${MAX_MATERIALS} entries`)
  }
  if (envelope.tessellation !== 'low' && envelope.tessellation !== 'standard' && envelope.tessellation !== 'hero') {
    throw new Img2ThreeJsSourceEnvelopeError('tessellation is outside the closed tier set')
  }
  finiteInRange(envelope.max_triangles, 1, 1_000_000, 'max_triangles')

  const materialIds = new Set<string>()
  const materialOrders = new Set<number>()
  for (const [index, item] of envelope.materials.entries()) {
    validateMaterial(item, `materials[${index}]`, materialIds, materialOrders, envelope.materials.length)
  }
  const componentIds = new Set<string>()
  const componentOrders = new Set<number>()
  for (const [index, item] of envelope.components.entries()) {
    validateComponent(item, `components[${index}]`, componentIds, componentOrders, materialIds, envelope.components.length)
  }
}

/**
 * Freeze the envelope and all nested arrays/records.  The compatibility
 * compiler treats the result as immutable input, so a caller cannot mutate a
 * transform or descriptor after its receipt was generated.
 */
export function freezeImg2ThreeJsSourceEnvelope<T extends Img2ThreeJsSourceEnvelope>(value: T): T {
  return deepFreeze(value)
}

function validateSourceIdentity(value: unknown): asserts value is Img2ThreeJsSourceIdentity {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Img2ThreeJsSourceEnvelopeError('source_identity must be an object')
  const identity = value as Record<string, unknown>
  exactKeys(identity, ['revision', 'tree', 'generator_sha256', 'validator_sha256'], 'source_identity')
  for (const [key, length] of [['revision', 40], ['tree', 40], ['generator_sha256', 64], ['validator_sha256', 64]] as const) {
    if (typeof identity[key] !== 'string' || !new RegExp(`^[0-9a-f]{${length}}$`, 'i').test(identity[key] as string)) {
      throw new Img2ThreeJsSourceEnvelopeError(`source_identity.${key} must be a ${length}-character hex digest`)
    }
  }
  if (identity.revision !== IMG2THREEJS_SOURCE_IDENTITY.revision || identity.tree !== IMG2THREEJS_SOURCE_IDENTITY.tree || identity.generator_sha256 !== IMG2THREEJS_SOURCE_IDENTITY.generator_sha256 || identity.validator_sha256 !== IMG2THREEJS_SOURCE_IDENTITY.validator_sha256) {
    throw new Img2ThreeJsSourceEnvelopeError('source_identity is not the pinned upstream identity')
  }
}

function validateMaterial(value: unknown, label: string, seen: Set<string>, orders: Set<number>, maximumOrder: number): void {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Img2ThreeJsSourceEnvelopeError(`${label} must be an object`)
  const material = value as Record<string, unknown>
  exactKeys(material, ['material_id', 'source_order', 'base_color', 'metalness', 'roughness'], label)
  const id = stableId(material.material_id, `${label}.material_id`)
  if (seen.has(id)) throw new Img2ThreeJsSourceEnvelopeError(`duplicate material_id ${id}`)
  seen.add(id)
  sourceOrder(material.source_order, `${label}.source_order`, orders, maximumOrder)
  if (typeof material.base_color !== 'string' || !/^#[0-9a-f]{6}$/i.test(material.base_color)) {
    throw new Img2ThreeJsSourceEnvelopeError(`${label}.base_color must be #RRGGBB`)
  }
  finiteInRange(material.metalness, 0, 1, `${label}.metalness`)
  finiteInRange(material.roughness, 0, 1, `${label}.roughness`)
}

function validateComponent(value: unknown, label: string, seen: Set<string>, orders: Set<number>, materialIds: ReadonlySet<string>, maximumOrder: number): void {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Img2ThreeJsSourceEnvelopeError(`${label} must be an object`)
  const component = value as Record<string, unknown>
  exactKeys(component, ['component_id', 'source_order', 'role', 'primitive', 'material_id', 'parent_id', 'transform', 'geometry'], label)
  const id = stableId(component.component_id, `${label}.component_id`)
  if (seen.has(id)) throw new Img2ThreeJsSourceEnvelopeError(`duplicate component_id ${id}`)
  seen.add(id)
  sourceOrder(component.source_order, `${label}.source_order`, orders, maximumOrder)
  if (!isRole(component.role)) throw new Img2ThreeJsSourceEnvelopeError(`${label}.role is unsupported`)
  if (!isPrimitive(component.primitive)) throw new Img2ThreeJsSourceEnvelopeError(`${label}.primitive is unsupported`)
  const materialId = stableId(component.material_id, `${label}.material_id`)
  if (!materialIds.has(materialId)) throw new Img2ThreeJsSourceEnvelopeError(`${label} references missing material ${materialId}`)
  if (component.parent_id !== null) throw new Img2ThreeJsSourceEnvelopeError(`${label}.parent_id must be null in the root-only compatibility profile`)
  validateTransform(component.transform, `${label}.transform`)
  if (component.role === 'blade' && component.primitive !== 'ground-blade') throw new Img2ThreeJsSourceEnvelopeError(`${label} blade must use ground-blade`)
  if ((component.role === 'guard' || component.role === 'relief') && component.primitive !== 'extrude') throw new Img2ThreeJsSourceEnvelopeError(`${label} ${component.role} must use extrude`)
  if (component.role === 'grip' && component.primitive !== 'curve-sweep') throw new Img2ThreeJsSourceEnvelopeError(`${label} grip must use curve-sweep`)
  if ((component.role === 'pommel' || component.role === 'gem') && component.primitive !== 'sphere') throw new Img2ThreeJsSourceEnvelopeError(`${label} ${component.role} must use sphere`)
  if (component.role === 'fastener' && component.primitive !== 'cylinder') throw new Img2ThreeJsSourceEnvelopeError(`${label} fastener must use cylinder`)
  validateGeometry(component.geometry, `${label}.geometry`)
  if (!component.geometry || typeof component.geometry !== 'object' || Array.isArray(component.geometry) || (component.geometry as Record<string, unknown>).primitive !== component.primitive) {
    throw new Img2ThreeJsSourceEnvelopeError(`${label}.primitive must equal geometry.primitive`)
  }
}

function validateTransform(value: unknown, label: string): void {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Img2ThreeJsSourceEnvelopeError(`${label} must be an object`)
  const transform = value as Record<string, unknown>
  exactKeys(transform, ['position', 'rotation_xyz', 'scale', 'pivot', 'rotation_order'], label)
  validateVec3(transform.position, `${label}.position`, MAX_POSITION)
  validateVec3(transform.rotation_xyz, `${label}.rotation_xyz`, MAX_ROTATION)
  validateVec3(transform.scale, `${label}.scale`, MAX_SCALE)
  validateVec3(transform.pivot, `${label}.pivot`, MAX_POSITION)
  if (!Array.isArray(transform.pivot) || transform.pivot.some((item) => item !== 0)) {
    throw new Img2ThreeJsSourceEnvelopeError(`${label}.pivot must be [0,0,0] for pinned img2threejs semantics`)
  }
  if (transform.scale && Array.isArray(transform.scale) && transform.scale.some((item) => typeof item !== 'number' || item <= MIN_SCALE)) {
    throw new Img2ThreeJsSourceEnvelopeError(`${label}.scale must be positive and above ${MIN_SCALE}`)
  }
  if (transform.rotation_order !== 'XYZ') throw new Img2ThreeJsSourceEnvelopeError(`${label}.rotation_order must be XYZ`)
  if (Array.isArray(transform.rotation_xyz)) {
    for (const [index, angle] of transform.rotation_xyz.entries()) {
      const turns = Math.round((angle as number) / (Math.PI * 0.5))
      if (Math.abs((angle as number) - turns * Math.PI * 0.5) > 1e-4) throw new Img2ThreeJsSourceEnvelopeError(`${label}.rotation_xyz[${index}] must be a quarter turn`)
    }
  }
}

function validateGeometry(value: unknown, label: string): void {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Img2ThreeJsSourceEnvelopeError(`${label} must be an object`)
  const geometry = value as Record<string, unknown>
  if (!isPrimitive(geometry.primitive)) throw new Img2ThreeJsSourceEnvelopeError(`${label}.primitive is unsupported`)
  switch (geometry.primitive) {
    case 'ground-blade':
      exactKeys(geometry, ['primitive', 'stations', 'thickness', 'grind_frac', 'swedge_from_tip_frac'], label)
      if (!Array.isArray(geometry.stations) || geometry.stations.length < 4 || geometry.stations.length > MAX_POINTS) throw new Img2ThreeJsSourceEnvelopeError(`${label}.stations must contain 4 to ${MAX_POINTS} points`)
      for (const [index, point] of geometry.stations.entries()) validateVec3(point, `${label}.stations[${index}]`, MAX_POSITION)
      for (let index = 1; index < geometry.stations.length; index += 1) {
        const previous = geometry.stations[index - 1] as number[]
        const current = geometry.stations[index] as number[]
        if (current[0] <= previous[0]) throw new Img2ThreeJsSourceEnvelopeError(`${label}.stations must be strictly x-increasing`)
        if (current[1] <= current[2]) throw new Img2ThreeJsSourceEnvelopeError(`${label}.stations contain crossing rails`)
      }
      finiteInRange(geometry.thickness, 1e-5, 1, `${label}.thickness`)
      finiteInRange(geometry.grind_frac, 0, 1, `${label}.grind_frac`)
      finiteInRange(geometry.swedge_from_tip_frac, 0, 1, `${label}.swedge_from_tip_frac`)
      return
    case 'extrude':
      exactKeys(geometry, ['primitive', 'profile_2d', 'depth'], label)
      validateProfile(geometry.profile_2d, `${label}.profile_2d`)
      finiteInRange(geometry.depth, 1e-5, 1, `${label}.depth`)
      return
    case 'curve-sweep':
      exactKeys(geometry, ['primitive', 'spine', 'cross_section', 'closed'], label)
      if (!Array.isArray(geometry.spine) || geometry.spine.length < 3 || geometry.spine.length > MAX_POINTS) throw new Img2ThreeJsSourceEnvelopeError(`${label}.spine must contain 3 to ${MAX_POINTS} points`)
      for (const [index, point] of geometry.spine.entries()) validateVec3(point, `${label}.spine[${index}]`, MAX_POSITION)
      validateProfile(geometry.cross_section, `${label}.cross_section`)
      if (typeof geometry.closed !== 'boolean') throw new Img2ThreeJsSourceEnvelopeError(`${label}.closed must be boolean`)
      return
    case 'sphere':
    case 'cylinder':
      exactKeys(geometry, ['primitive'], label)
      return
  }
}

function sourceOrder(value: unknown, label: string, seen: Set<number>, maximum: number): asserts value is number {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 0 || value >= maximum) {
    throw new Img2ThreeJsSourceEnvelopeError(`${label} must be a unique integer in [0, ${maximum - 1}]`)
  }
  if (seen.has(value)) throw new Img2ThreeJsSourceEnvelopeError(`duplicate ${label}`)
  seen.add(value)
}

function validateProfile(value: unknown, label: string): void {
  if (!Array.isArray(value) || value.length < 3 || value.length > MAX_POINTS) throw new Img2ThreeJsSourceEnvelopeError(`${label} must contain 3 to ${MAX_POINTS} points`)
  for (const [index, point] of value.entries()) validateVec2(point, `${label}[${index}]`, MAX_POSITION)
}

function validateVec2(value: unknown, label: string, maximum: number): asserts value is Img2ThreeJsSourceVec2 {
  if (!Array.isArray(value) || value.length !== 2 || value.some((item) => typeof item !== 'number' || !Number.isFinite(item) || Math.abs(item) > maximum)) {
    throw new Img2ThreeJsSourceEnvelopeError(`${label} must be a finite bounded vec2`)
  }
}

function validateVec3(value: unknown, label: string, maximum: number): asserts value is Img2ThreeJsSourceVec3 {
  if (!Array.isArray(value) || value.length !== 3 || value.some((item) => typeof item !== 'number' || !Number.isFinite(item) || Math.abs(item) > maximum)) {
    throw new Img2ThreeJsSourceEnvelopeError(`${label} must be a finite bounded vec3`)
  }
}

function finiteInRange(value: unknown, minimum: number, maximum: number, label: string): asserts value is number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < minimum || value > maximum) {
    throw new Img2ThreeJsSourceEnvelopeError(`${label} must be finite and in [${minimum}, ${maximum}]`)
  }
}

function stableId(value: unknown, label: string): string {
  if (typeof value !== 'string' || !STABLE_ID_PATTERN.test(value)) throw new Img2ThreeJsSourceEnvelopeError(`${label} must be a bounded stable ID`)
  return value
}

function isRole(value: unknown): value is Img2ThreeJsSourceRole {
  return value === 'blade' || value === 'guard' || value === 'grip' || value === 'pommel' || value === 'fastener' || value === 'gem' || value === 'relief'
}

function isPrimitive(value: unknown): value is Img2ThreeJsSourcePrimitive {
  return value === 'ground-blade' || value === 'extrude' || value === 'curve-sweep' || value === 'sphere' || value === 'cylinder'
}

function deepFreeze<T>(value: T): T {
  if (!value || typeof value !== 'object' || Object.isFrozen(value)) return value
  Object.freeze(value)
  for (const child of Object.values(value as Record<string, unknown>)) deepFreeze(child)
  return value
}
