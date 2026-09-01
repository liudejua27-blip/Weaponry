import * as THREE from 'three'

/**
 * A bounded curve-graph payload for relief and engraving geometry.
 *
 * Points are expressed in the supplied blade-local frame.  A 2D path uses
 * `[tangent, lateral]` coordinates and a 3D path uses
 * `[tangent, lateral, normal]` coordinates.  The compiler returns derived
 * BufferGeometry only; it does not write a KnifeSceneProgram, Runtime state,
 * files, URLs, or caller-provided code.
 */

export const RELIEF_CURVE_GRAPH_SCHEMA_VERSION = 'ReliefCurveGraph@1' as const
export const RELIEF_CURVE_GRAPH_ALGORITHM = 'blade-local-volume-sweep@1' as const

export const RELIEF_CURVE_GRAPH_LIMITS = Object.freeze({
  max_paths: 16,
  min_control_points: 2,
  max_control_points: 64,
  min_samples_per_path: 8,
  max_samples_per_path: 128,
  min_round_radial_segments: 8,
  max_round_radial_segments: 24,
  max_triangles: 65536,
  max_coordinate: 4,
  max_width: 1,
  max_depth: 1,
})

const MIN_DIMENSION = 1e-4
const FRAME_EPSILON = 1e-5
const PATH_EPSILON = 1e-9
const ID_PATTERN = /^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/

export type ReliefVec2 = readonly [number, number]
export type ReliefVec3 = readonly [number, number, number]
export type ReliefCurveDimension = '2d' | '3d'
export type ReliefCurveBasis = 'bezier' | 'nurbs-like'
export type ReliefCurveProfile = 'round' | 'bevel' | 'flat'

export interface ReliefLocalFrame {
  readonly origin: ReliefVec3
  readonly tangent: ReliefVec3
  readonly lateral: ReliefVec3
  readonly normal: ReliefVec3
}

interface ReliefCurvePathBase {
  readonly path_id: string
  readonly basis: ReliefCurveBasis
  /** Closed paths join their final sampled ring back to the first ring. */
  readonly closed?: boolean
}

export interface ReliefCurvePath2D extends ReliefCurvePathBase {
  readonly dimension: '2d'
  readonly points: readonly ReliefVec2[]
}

export interface ReliefCurvePath3D extends ReliefCurvePathBase {
  readonly dimension: '3d'
  readonly points: readonly ReliefVec3[]
}

export type ReliefCurvePath = ReliefCurvePath2D | ReliefCurvePath3D

/** Closed input contract; unknown fields are rejected by the runtime guard. */
export interface ReliefCurveGraph {
  readonly schema_version: typeof RELIEF_CURVE_GRAPH_SCHEMA_VERSION
  readonly graph_id: string
  readonly part_id: string
  readonly material_zone_id: string
  readonly local_frame: ReliefLocalFrame
  readonly paths: readonly ReliefCurvePath[]
  /** Full lateral size at taper scale 1. */
  readonly width: number
  /** Full normal size at taper scale 1. */
  readonly depth: number
  /** End-to-end scale delta.  Start is `1-taper`, end is `1+taper`. */
  readonly taper: number
  readonly profile: ReliefCurveProfile
}

export type ReliefCurveGraphInput = ReliefCurveGraph

/** Optional bounded tessellation controls. */
export interface ReliefCurveGraphCompileOptions {
  readonly samples_per_path?: number
  /** Applies to round profiles; bevel and flat profiles use fixed rings. */
  readonly round_radial_segments?: number
  readonly max_triangles?: number
}

interface ResolvedReliefCurveGraphOptions {
  readonly samples_per_path: number
  readonly radial_segments: number
  readonly max_triangles: number
}

export interface ReliefCurveGraphBounds {
  readonly min: ReliefVec3
  readonly max: ReliefVec3
}

export interface CompiledReliefCurveGraph {
  readonly schema_version: typeof RELIEF_CURVE_GRAPH_SCHEMA_VERSION
  readonly graph_id: string
  readonly part_id: string
  readonly material_zone_id: string
  readonly geometry: THREE.BufferGeometry
  readonly path_ids: readonly string[]
  readonly profile: ReliefCurveProfile
  readonly samples_per_path: number
  readonly radial_segments: number
  readonly triangle_count: number
  readonly vertex_count: number
  readonly bounds: ReliefCurveGraphBounds
  /** Browser-safe deterministic fingerprint; not a Runtime/CAS SHA-256. */
  readonly deterministic_fingerprint: string
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
}

export type ReliefCurveGraphErrorCode = 'INVALID_INPUT' | 'BUDGET_EXCEEDED'

export class ReliefCurveGraphError extends Error {
  readonly code: ReliefCurveGraphErrorCode

  constructor(code: ReliefCurveGraphErrorCode, message: string) {
    super(`${code}: ${message}`)
    this.name = 'ReliefCurveGraphError'
    this.code = code
  }
}

/** Validate a graph without allocating geometry. */
export function validateReliefCurveGraph(input: ReliefCurveGraph): void {
  if (!isRecord(input)) reject('INVALID_INPUT', 'graph must be an object')
  exactKeys(input, new Set([
    'schema_version',
    'graph_id',
    'part_id',
    'material_zone_id',
    'local_frame',
    'paths',
    'width',
    'depth',
    'taper',
    'profile',
  ]), 'graph')
  if (input.schema_version !== RELIEF_CURVE_GRAPH_SCHEMA_VERSION) reject('INVALID_INPUT', 'schema_version drifted')
  stableId(input.graph_id, 'graph.graph_id')
  stableId(input.part_id, 'graph.part_id')
  stableId(input.material_zone_id, 'graph.material_zone_id')
  validateFrame(input.local_frame)

  if (!Array.isArray(input.paths) || input.paths.length < 1 || input.paths.length > RELIEF_CURVE_GRAPH_LIMITS.max_paths) {
    reject('INVALID_INPUT', `graph.paths must contain 1-${RELIEF_CURVE_GRAPH_LIMITS.max_paths} paths`)
  }
  const pathIds: string[] = []
  for (const [index, path] of input.paths.entries()) {
    validatePath(path, `graph.paths[${index}]`)
    if (pathIds.includes(path.path_id)) reject('INVALID_INPUT', `graph.paths[${index}].path_id must be unique`)
    pathIds.push(path.path_id)
  }
  boundedNumber(input.width, 'graph.width', MIN_DIMENSION, RELIEF_CURVE_GRAPH_LIMITS.max_width, true)
  boundedNumber(input.depth, 'graph.depth', MIN_DIMENSION, RELIEF_CURVE_GRAPH_LIMITS.max_depth, true)
  boundedNumber(input.taper, 'graph.taper', -0.9, 0.9)
  if (!['round', 'bevel', 'flat'].includes(input.profile)) reject('INVALID_INPUT', 'graph.profile is unsupported')
}

/** Return the deterministic graph fingerprint for the resolved tessellation settings. */
export function fingerprintReliefCurveGraph(
  input: ReliefCurveGraph,
  options?: ReliefCurveGraphCompileOptions,
): string {
  validateReliefCurveGraph(input)
  const resolved = resolveOptions(input, options)
  return hash64([
    RELIEF_CURVE_GRAPH_SCHEMA_VERSION,
    RELIEF_CURVE_GRAPH_ALGORITHM,
    stableValue(normalizedGraph(input)),
    stableValue(resolved),
  ].join('|'))
}

/** Compile a bounded 2D/3D curve graph into a volumetric relief mesh payload. */
export function compileReliefCurveGraph(
  input: ReliefCurveGraph,
  options?: ReliefCurveGraphCompileOptions,
): CompiledReliefCurveGraph {
  validateReliefCurveGraph(input)
  const resolved = resolveOptions(input, options)
  const fingerprint = hash64([
    RELIEF_CURVE_GRAPH_SCHEMA_VERSION,
    RELIEF_CURVE_GRAPH_ALGORITHM,
    stableValue(normalizedGraph(input)),
    stableValue(resolved),
  ].join('|'))

  const geometry = new THREE.BufferGeometry()
  const positions: number[] = []
  const uvs: number[] = []
  const indices: number[] = []
  const partHashes: number[] = []
  const materialHashes: number[] = []
  const pathIndices: number[] = []
  const frame = makeFrame(input.local_frame)
  const partHash = fnv1a32(input.part_id)
  const materialHash = fnv1a32(input.material_zone_id)

  for (const [pathIndex, path] of input.paths.entries()) {
    const samples = samplePath(path, frame, resolved.samples_per_path)
    const radialSegments = resolved.radial_segments
    const rings: number[][] = []
    const indexStart = indices.length
    let previousSide: THREE.Vector3 | undefined

    for (const sample of samples) {
      const oriented = orientPathFrame(sample.tangent, frame, previousSide)
      previousSide = oriented.side
      const scale = 1 + input.taper * (2 * sample.t - 1)
      const halfWidth = input.width * scale * 0.5
      const halfDepth = input.depth * scale * 0.5
      const offsets = profileRing(input.profile, radialSegments, halfWidth, halfDepth)
      const ring: number[] = []
      for (const [sideOffset, normalOffset] of offsets) {
        const position = sample.center.clone()
          .addScaledVector(oriented.side, sideOffset)
          .addScaledVector(oriented.normal, normalOffset)
        ring.push(positions.length / 3)
        positions.push(position.x, position.y, position.z)
        uvs.push(sample.t, clamp(0.5 + sideOffset / Math.max(input.width * scale, MIN_DIMENSION), 0, 1))
        partHashes.push(partHash)
        materialHashes.push(materialHash)
        pathIndices.push(pathIndex)
      }
      rings.push(ring)
    }

    const sideSegmentCount = path.closed ? rings.length : rings.length - 1
    for (let sampleIndex = 0; sampleIndex < sideSegmentCount; sampleIndex += 1) {
      const nextSampleIndex = (sampleIndex + 1) % rings.length
      const current = rings[sampleIndex]
      const next = rings[nextSampleIndex]
      for (let radialIndex = 0; radialIndex < radialSegments; radialIndex += 1) {
        const nextRadialIndex = (radialIndex + 1) % radialSegments
        const a = current[radialIndex]
        const b = current[nextRadialIndex]
        const c = next[nextRadialIndex]
        const d = next[radialIndex]
        indices.push(a, b, c, a, c, d)
      }
    }

    if (!path.closed) {
      appendCap(samples[0], rings[0], true, positions, uvs, indices, partHashes, materialHashes, pathIndices, pathIndex, partHash, materialHash)
      appendCap(samples[samples.length - 1], rings[rings.length - 1], false, positions, uvs, indices, partHashes, materialHashes, pathIndices, pathIndex, partHash, materialHash)
    }
    geometry.addGroup(indexStart, indices.length - indexStart, 0)
  }

  if (indices.length / 3 > resolved.max_triangles) {
    reject('BUDGET_EXCEEDED', `generated triangle count exceeds ${resolved.max_triangles}`)
  }
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3))
  geometry.setAttribute('uv', new THREE.Float32BufferAttribute(uvs, 2))
  geometry.setAttribute('partIdHash', new THREE.Uint32BufferAttribute(partHashes, 1))
  geometry.setAttribute('materialZoneHash', new THREE.Uint32BufferAttribute(materialHashes, 1))
  geometry.setAttribute('reliefPathIndex', new THREE.Uint16BufferAttribute(pathIndices, 1))
  geometry.setIndex(new THREE.Uint32BufferAttribute(indices, 1))
  geometry.computeVertexNormals()
  geometry.computeBoundingBox()
  geometry.computeBoundingSphere()
  if (!geometry.boundingBox || !geometry.boundingSphere) reject('INVALID_INPUT', 'generated geometry bounds are unavailable')

  const pathIds = Object.freeze(input.paths.map((path) => path.path_id))
  const bounds: ReliefCurveGraphBounds = Object.freeze({
    min: freezeVec3([geometry.boundingBox.min.x, geometry.boundingBox.min.y, geometry.boundingBox.min.z]),
    max: freezeVec3([geometry.boundingBox.max.x, geometry.boundingBox.max.y, geometry.boundingBox.max.z]),
  })
  geometry.name = `${input.graph_id}:relief-curve-graph`
  overrideUuid(geometry, stableUuid(`relief-curve:${fingerprint}`))
  geometry.userData = Object.freeze({
    schema_version: RELIEF_CURVE_GRAPH_SCHEMA_VERSION,
    algorithm: RELIEF_CURVE_GRAPH_ALGORITHM,
    graph_id: input.graph_id,
    part_id: input.part_id,
    material_zone_id: input.material_zone_id,
    path_ids: pathIds,
    profile: input.profile,
    samples_per_path: resolved.samples_per_path,
    radial_segments: resolved.radial_segments,
    triangle_count: indices.length / 3,
    deterministic_fingerprint: fingerprint,
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  })

  return Object.freeze({
    schema_version: RELIEF_CURVE_GRAPH_SCHEMA_VERSION,
    graph_id: input.graph_id,
    part_id: input.part_id,
    material_zone_id: input.material_zone_id,
    geometry,
    path_ids: pathIds,
    profile: input.profile,
    samples_per_path: resolved.samples_per_path,
    radial_segments: resolved.radial_segments,
    triangle_count: indices.length / 3,
    vertex_count: positions.length / 3,
    bounds,
    deterministic_fingerprint: fingerprint,
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  })
}

/** Convenience API for callers that only need the mesh geometry. */
export function compileReliefCurveGraphGeometry(
  input: ReliefCurveGraph,
  options?: ReliefCurveGraphCompileOptions,
): THREE.BufferGeometry {
  return compileReliefCurveGraph(input, options).geometry
}

function validateFrame(value: unknown): void {
  exactKeys(value, new Set(['origin', 'tangent', 'lateral', 'normal']), 'graph.local_frame')
  const frame = value as unknown as ReliefLocalFrame
  validateVec3(frame.origin, 'graph.local_frame.origin', RELIEF_CURVE_GRAPH_LIMITS.max_coordinate)
  const tangent = validateUnitVec3(frame.tangent, 'graph.local_frame.tangent')
  const lateral = validateUnitVec3(frame.lateral, 'graph.local_frame.lateral')
  const normal = validateUnitVec3(frame.normal, 'graph.local_frame.normal')
  if (Math.abs(tangent.dot(lateral)) > FRAME_EPSILON || Math.abs(tangent.dot(normal)) > FRAME_EPSILON || Math.abs(lateral.dot(normal)) > FRAME_EPSILON) {
    reject('INVALID_INPUT', 'graph.local_frame axes must be orthogonal')
  }
  if (tangent.clone().cross(lateral).dot(normal) <= 1 - FRAME_EPSILON) {
    reject('INVALID_INPUT', 'graph.local_frame axes must be right-handed')
  }
}

function validatePath(value: unknown, label: string): void {
  if (!isRecord(value)) reject('INVALID_INPUT', `${label} must be an object`)
  const expected = new Set(['path_id', 'dimension', 'basis', 'points'])
  if ('closed' in value) expected.add('closed')
  exactKeys(value, expected, label)
  stableId(value.path_id, `${label}.path_id`)
  if (value.dimension !== '2d' && value.dimension !== '3d') reject('INVALID_INPUT', `${label}.dimension is unsupported`)
  if (value.basis !== 'bezier' && value.basis !== 'nurbs-like') reject('INVALID_INPUT', `${label}.basis is unsupported`)
  if ('closed' in value && typeof value.closed !== 'boolean') reject('INVALID_INPUT', `${label}.closed must be boolean`)
  if (!Array.isArray(value.points) || value.points.length < RELIEF_CURVE_GRAPH_LIMITS.min_control_points || value.points.length > RELIEF_CURVE_GRAPH_LIMITS.max_control_points) {
    reject('INVALID_INPUT', `${label}.points count is outside [${RELIEF_CURVE_GRAPH_LIMITS.min_control_points},${RELIEF_CURVE_GRAPH_LIMITS.max_control_points}]`)
  }
  for (const [index, point] of value.points.entries()) {
    if (value.dimension === '2d') validateVec2(point, `${label}.points[${index}]`, RELIEF_CURVE_GRAPH_LIMITS.max_coordinate)
    else validateVec3(point, `${label}.points[${index}]`, RELIEF_CURVE_GRAPH_LIMITS.max_coordinate)
  }
}

function resolveOptions(input: ReliefCurveGraph, options?: ReliefCurveGraphCompileOptions): ResolvedReliefCurveGraphOptions {
  if (options !== undefined) {
    exactKeys(options, new Set(['samples_per_path', 'round_radial_segments', 'max_triangles']), 'compile options')
  }
  const samples = options?.samples_per_path ?? 32
  const roundRadial = options?.round_radial_segments ?? 12
  const maxTriangles = options?.max_triangles ?? RELIEF_CURVE_GRAPH_LIMITS.max_triangles
  integerInRange(samples, 'compile options.samples_per_path', RELIEF_CURVE_GRAPH_LIMITS.min_samples_per_path, RELIEF_CURVE_GRAPH_LIMITS.max_samples_per_path)
  integerInRange(roundRadial, 'compile options.round_radial_segments', RELIEF_CURVE_GRAPH_LIMITS.min_round_radial_segments, RELIEF_CURVE_GRAPH_LIMITS.max_round_radial_segments)
  integerInRange(maxTriangles, 'compile options.max_triangles', 1, RELIEF_CURVE_GRAPH_LIMITS.max_triangles)
  const radialSegments = input.profile === 'round' ? roundRadial : input.profile === 'bevel' ? 8 : 4
  const estimatedTriangles = input.paths.reduce((total, path) => {
    const side = (path.closed ? samples : samples - 1) * radialSegments * 2
    return total + side + (path.closed ? 0 : radialSegments * 2)
  }, 0)
  if (estimatedTriangles > maxTriangles) reject('BUDGET_EXCEEDED', `estimated triangle count ${estimatedTriangles} exceeds ${maxTriangles}`)
  return Object.freeze({ samples_per_path: samples, radial_segments: radialSegments, max_triangles: maxTriangles })
}

interface FrameVectors {
  readonly origin: THREE.Vector3
  readonly tangent: THREE.Vector3
  readonly lateral: THREE.Vector3
  readonly normal: THREE.Vector3
}

function makeFrame(frame: ReliefLocalFrame): FrameVectors {
  return {
    origin: new THREE.Vector3(...frame.origin),
    tangent: new THREE.Vector3(...frame.tangent),
    lateral: new THREE.Vector3(...frame.lateral),
    normal: new THREE.Vector3(...frame.normal),
  }
}

interface PathSample {
  readonly t: number
  readonly center: THREE.Vector3
  readonly tangent: THREE.Vector3
}

function samplePath(path: ReliefCurvePath, frame: FrameVectors, sampleCount: number): PathSample[] {
  const controls = path.points.map((point) => path.dimension === '2d'
    ? new THREE.Vector3(point[0], point[1], 0)
    : new THREE.Vector3(point[0], point[1], point[2]))
  const sampled: THREE.Vector3[] = []
  for (let index = 0; index < sampleCount; index += 1) {
    const t = path.closed ? index / sampleCount : index / (sampleCount - 1)
    const local = path.basis === 'bezier'
      ? evaluateBezier(controls, t)
      : evaluateNurbsLike(controls, t, Boolean(path.closed))
    sampled.push(new THREE.Vector3()
      .copy(frame.origin)
      .addScaledVector(frame.tangent, local.x)
      .addScaledVector(frame.lateral, local.y)
      .addScaledVector(frame.normal, local.z))
  }
  const result: PathSample[] = []
  for (let index = 0; index < sampled.length; index += 1) {
    const previousIndex = path.closed ? (index - 1 + sampled.length) % sampled.length : Math.max(0, index - 1)
    const nextIndex = path.closed ? (index + 1) % sampled.length : Math.min(sampled.length - 1, index + 1)
    const tangent = sampled[nextIndex].clone().sub(sampled[previousIndex])
    if (tangent.lengthSq() <= PATH_EPSILON * PATH_EPSILON) reject('INVALID_INPUT', `${path.path_id} has a degenerate sampled tangent`)
    result.push({ t: path.closed ? index / sampleCount : index / (sampleCount - 1), center: sampled[index], tangent: tangent.normalize() })
  }
  return result
}

interface OrientedPathFrame {
  readonly side: THREE.Vector3
  readonly normal: THREE.Vector3
}

function orientPathFrame(tangent: THREE.Vector3, frame: FrameVectors, previousSide?: THREE.Vector3): OrientedPathFrame {
  let normal = frame.normal.clone().addScaledVector(tangent, -frame.normal.dot(tangent))
  if (normal.lengthSq() <= PATH_EPSILON * PATH_EPSILON) {
    normal = frame.lateral.clone().addScaledVector(tangent, -frame.lateral.dot(tangent))
  }
  if (normal.lengthSq() <= PATH_EPSILON * PATH_EPSILON) reject('INVALID_INPUT', 'path tangent is parallel to the local frame')
  normal.normalize()
  if (normal.dot(frame.normal) < 0) normal.negate()
  let side = normal.clone().cross(tangent)
  if (side.lengthSq() <= PATH_EPSILON * PATH_EPSILON) reject('INVALID_INPUT', 'path frame side is degenerate')
  side.normalize()
  if (previousSide && side.dot(previousSide) < 0) {
    side.negate()
    normal.negate()
  }
  return { side, normal }
}

function profileRing(
  profile: ReliefCurveProfile,
  radialSegments: number,
  halfWidth: number,
  halfDepth: number,
): readonly (readonly [number, number])[] {
  if (profile === 'flat') {
    return [
      [halfWidth, halfDepth],
      [-halfWidth, halfDepth],
      [-halfWidth, -halfDepth],
      [halfWidth, -halfDepth],
    ]
  }
  if (profile === 'bevel') {
    const bevel = Math.min(halfWidth, halfDepth) * 0.25
    return [
      [halfWidth - bevel, halfDepth],
      [-halfWidth + bevel, halfDepth],
      [-halfWidth, halfDepth - bevel],
      [-halfWidth, -halfDepth + bevel],
      [-halfWidth + bevel, -halfDepth],
      [halfWidth - bevel, -halfDepth],
      [halfWidth, -halfDepth + bevel],
      [halfWidth, halfDepth - bevel],
    ]
  }
  return Array.from({ length: radialSegments }, (_, index) => {
    const angle = (Math.PI * 2 * index) / radialSegments
    return [Math.cos(angle) * halfWidth, Math.sin(angle) * halfDepth] as const
  })
}

function appendCap(
  sample: PathSample,
  ring: readonly number[],
  start: boolean,
  positions: number[],
  uvs: number[],
  indices: number[],
  partHashes: number[],
  materialHashes: number[],
  pathIndices: number[],
  pathIndex: number,
  partHash: number,
  materialHash: number,
): void {
  const centerIndex = positions.length / 3
  positions.push(sample.center.x, sample.center.y, sample.center.z)
  uvs.push(sample.t, 0.5)
  partHashes.push(partHash)
  materialHashes.push(materialHash)
  pathIndices.push(pathIndex)
  for (let index = 0; index < ring.length; index += 1) {
    const next = (index + 1) % ring.length
    if (start) indices.push(centerIndex, ring[next], ring[index])
    else indices.push(centerIndex, ring[index], ring[next])
  }
}

function evaluateBezier(points: readonly THREE.Vector3[], t: number): THREE.Vector3 {
  let work = points.map((point) => point.clone())
  while (work.length > 1) {
    work = work.slice(0, -1).map((point, index) => point.clone().lerp(work[index + 1], t))
  }
  return work[0]
}

function evaluateNurbsLike(points: readonly THREE.Vector3[], t: number, closed: boolean): THREE.Vector3 {
  const count = points.length
  if (closed) {
    const scaled = clamp(t, 0, 1) * count
    const base = Math.floor(scaled)
    const segment = base % count
    const local = scaled - base
    return catmullPoint(
      points[(segment - 1 + count) % count],
      points[segment],
      points[(segment + 1) % count],
      points[(segment + 2) % count],
      local,
    )
  }
  const scaled = clamp(t, 0, 1) * (count - 1)
  const segment = Math.min(Math.floor(scaled), count - 2)
  const local = scaled - segment
  return catmullPoint(
    points[Math.max(0, segment - 1)],
    points[segment],
    points[segment + 1],
    points[Math.min(count - 1, segment + 2)],
    local,
  )
}

function catmullPoint(p0: THREE.Vector3, p1: THREE.Vector3, p2: THREE.Vector3, p3: THREE.Vector3, t: number): THREE.Vector3 {
  const t2 = t * t
  const t3 = t2 * t
  return new THREE.Vector3(
    0.5 * ((2 * p1.x) + (-p0.x + p2.x) * t + (2 * p0.x - 5 * p1.x + 4 * p2.x - p3.x) * t2 + (-p0.x + 3 * p1.x - 3 * p2.x + p3.x) * t3),
    0.5 * ((2 * p1.y) + (-p0.y + p2.y) * t + (2 * p0.y - 5 * p1.y + 4 * p2.y - p3.y) * t2 + (-p0.y + 3 * p1.y - 3 * p2.y + p3.y) * t3),
    0.5 * ((2 * p1.z) + (-p0.z + p2.z) * t + (2 * p0.z - 5 * p1.z + 4 * p2.z - p3.z) * t2 + (-p0.z + 3 * p1.z - 3 * p2.z + p3.z) * t3),
  )
}

function normalizedGraph(input: ReliefCurveGraph): Record<string, unknown> {
  return {
    schema_version: input.schema_version,
    graph_id: input.graph_id,
    part_id: input.part_id,
    material_zone_id: input.material_zone_id,
    local_frame: input.local_frame,
    paths: input.paths.map((path) => ({ ...path, closed: path.closed ?? false })),
    width: input.width,
    depth: input.depth,
    taper: input.taper,
    profile: input.profile,
  }
}

function exactKeys(value: unknown, expected: ReadonlySet<string>, label: string): asserts value is Record<string, unknown> {
  if (!isRecord(value)) reject('INVALID_INPUT', `${label} must be an object`)
  const keys = Object.keys(value)
  if (keys.length !== expected.size || keys.some((key) => !expected.has(key))) reject('INVALID_INPUT', `${label} keys are not closed`)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function stableId(value: unknown, label: string): asserts value is string {
  if (typeof value !== 'string' || ID_PATTERN.test(value) === false) reject('INVALID_INPUT', `${label} is not a bounded stable ID`)
}

function validateVec2(value: unknown, label: string, maximum: number): asserts value is ReliefVec2 {
  if (!Array.isArray(value) || value.length !== 2) reject('INVALID_INPUT', `${label} must be a 2D point`)
  for (const [index, coordinate] of value.entries()) boundedNumber(coordinate, `${label}[${index}]`, -maximum, maximum)
}

function validateVec3(value: unknown, label: string, maximum: number): THREE.Vector3 {
  if (!Array.isArray(value) || value.length !== 3) reject('INVALID_INPUT', `${label} must be a 3D point`)
  for (const [index, coordinate] of value.entries()) boundedNumber(coordinate, `${label}[${index}]`, -maximum, maximum)
  return new THREE.Vector3(value[0], value[1], value[2])
}

function validateUnitVec3(value: unknown, label: string): THREE.Vector3 {
  const vector = validateVec3(value, label, 1)
  if (Math.abs(vector.length() - 1) > FRAME_EPSILON) reject('INVALID_INPUT', `${label} must be unit length`)
  return vector.normalize()
}

function boundedNumber(value: unknown, label: string, minimum: number, maximum: number, exclusiveMinimum = false): asserts value is number {
  if (typeof value !== 'number' || !Number.isFinite(value)) reject('INVALID_INPUT', `${label} must be finite`)
  if (exclusiveMinimum ? value <= minimum : value < minimum) reject('INVALID_INPUT', `${label} is outside [${minimum},${maximum}]`)
  if (value > maximum) reject('INVALID_INPUT', `${label} is outside [${minimum},${maximum}]`)
}

function integerInRange(value: unknown, label: string, minimum: number, maximum: number): asserts value is number {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < minimum || value > maximum) reject('INVALID_INPUT', `${label} must be an integer in [${minimum},${maximum}]`)
}

function reject(code: ReliefCurveGraphErrorCode, message: string): never {
  throw new ReliefCurveGraphError(code, message)
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value))
}

function freezeVec3(value: ReliefVec3): ReliefVec3 {
  return Object.freeze(value)
}

function canonicalNumber(value: number): string {
  if (!Number.isFinite(value)) return value === Number.POSITIVE_INFINITY ? 'INF' : 'NAN'
  return Object.is(value, -0) ? '0' : value.toPrecision(12)
}

function stableValue(value: unknown): string {
  if (value === null) return 'null'
  if (typeof value === 'number') return canonicalNumber(value)
  if (typeof value === 'string') return JSON.stringify(value)
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (Array.isArray(value)) return `[${value.map(stableValue).join(',')}]`
  if (isRecord(value)) return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${stableValue(value[key])}`).join(',')}}`
  return String(value)
}

function hash64(value: string): string {
  let hash = 0xcbf29ce484222325n
  const prime = 0x100000001b3n
  const mask = 0xffffffffffffffffn
  for (let index = 0; index < value.length; index += 1) {
    hash ^= BigInt(value.charCodeAt(index))
    hash = (hash * prime) & mask
  }
  return hash.toString(16).padStart(16, '0')
}

function fnv1a32(value: string): number {
  let hash = 0x811c9dc5
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 0x01000193)
  }
  return hash >>> 0
}

function stableUuid(value: string): string {
  const raw = `${hash64(`${value}:0`)}${hash64(`${value}:1`)}${hash64(`${value}:2`)}${hash64(`${value}:3`)}`
  return `${raw.slice(0, 8)}-${raw.slice(8, 12)}-${raw.slice(12, 16)}-${raw.slice(16, 20)}-${raw.slice(20, 32)}`
}

function overrideUuid(object: { readonly uuid: string }, uuid: string): void {
  Object.defineProperty(object, 'uuid', { configurable: true, enumerable: true, value: uuid, writable: true })
}
