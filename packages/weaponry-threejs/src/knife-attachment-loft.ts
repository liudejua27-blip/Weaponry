import * as THREE from 'three'

const STABLE_ID_PATTERN = /^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/
const MIN_EDGE_LENGTH = 1e-5
const MIN_AREA = 1e-10
const MIN_FACE_AREA = 1e-12
const MIN_FOLDOVER_DOT = 1e-6
const PLANAR_TOLERANCE = 1e-4

export const KNIFE_ATTACHMENT_LOFT_SCHEMA_VERSION = 'KnifeAttachmentLoft@1' as const

export const KNIFE_ATTACHMENT_LOFT_ROLES = [
  'guard-upper-jaw',
  'guard-lower-jaw',
  'guard-horn',
  'guard-eye-shell',
  'pommel-hook',
  'custom-attachment',
] as const

export type KnifeAttachmentLoftRole = typeof KNIFE_ATTACHMENT_LOFT_ROLES[number]
export type KnifeAttachmentLoftVec3 = readonly [number, number, number]

export interface KnifeAttachmentLoftRingPoint {
  readonly point_id: string
  readonly position: KnifeAttachmentLoftVec3
}

export interface KnifeAttachmentLoftSection {
  readonly section_id: string
  /** Ordered, planar ring points. The first and last point are implicitly joined. */
  readonly ring: readonly KnifeAttachmentLoftRingPoint[]
}

/**
 * Closed input for one non-functional hard-surface attachment. Sections are
 * explicit rings in the same normalized scene frame as the knife program;
 * there is no primitive, script, URL, or shader escape hatch.
 */
export interface KnifeAttachmentLoftSpec {
  readonly schema_version: typeof KNIFE_ATTACHMENT_LOFT_SCHEMA_VERSION
  readonly attachment_id: string
  readonly role: KnifeAttachmentLoftRole
  readonly sections: readonly KnifeAttachmentLoftSection[]
  readonly cap_ends: true
}

export const KNIFE_ATTACHMENT_LOFT_LIMITS = Object.freeze({
  max_coordinate_abs: 4,
  min_sections: 2,
  max_sections: 64,
  min_ring_points: 3,
  max_ring_points: 32,
  max_vertices: 2048,
  max_triangles: 4096,
})

export type KnifeAttachmentLoftErrorCode =
  | 'INVALID_SPEC'
  | 'BUDGET_EXCEEDED'
  | 'DEGENERATE_PROFILE'
  | 'SELF_INTERSECTION'

export class KnifeAttachmentLoftError extends Error {
  readonly code: KnifeAttachmentLoftErrorCode

  constructor(code: KnifeAttachmentLoftErrorCode, message: string) {
    super(`${code}: ${message}`)
    this.name = 'KnifeAttachmentLoftError'
    this.code = code
  }
}

export interface KnifeAttachmentLoftResult {
  readonly schema_version: typeof KNIFE_ATTACHMENT_LOFT_SCHEMA_VERSION
  readonly attachment_id: string
  readonly role: KnifeAttachmentLoftRole
  readonly geometry: THREE.BufferGeometry
  readonly section_count: number
  readonly ring_point_count: number
  readonly vertex_count: number
  readonly triangle_count: number
  readonly welded_indexed: true
  readonly deterministic_fingerprint: string
}

interface Point2 {
  readonly x: number
  readonly y: number
}

interface RingAnalysis {
  readonly points: readonly THREE.Vector3[]
  readonly projected: readonly Point2[]
  readonly normal: THREE.Vector3
  readonly signed_area: number
}

export function validateKnifeAttachmentLoftSpec(value: unknown): asserts value is KnifeAttachmentLoftSpec {
  exactKeys(value, ['schema_version', 'attachment_id', 'role', 'sections', 'cap_ends'], 'KnifeAttachmentLoftSpec')
  if (value.schema_version !== KNIFE_ATTACHMENT_LOFT_SCHEMA_VERSION) {
    fail('INVALID_SPEC', 'schema_version must be KnifeAttachmentLoft@1')
  }
  boundedId(value.attachment_id, 'attachment_id')
  if (typeof value.role !== 'string'
    || !(KNIFE_ATTACHMENT_LOFT_ROLES as readonly string[]).includes(value.role)) {
    fail('INVALID_SPEC', 'role is outside the closed attachment vocabulary')
  }
  if (value.cap_ends !== true) fail('INVALID_SPEC', 'cap_ends must be true for a closed loft')
  if (!Array.isArray(value.sections)
    || value.sections.length < KNIFE_ATTACHMENT_LOFT_LIMITS.min_sections
    || value.sections.length > KNIFE_ATTACHMENT_LOFT_LIMITS.max_sections) {
    fail(
      'BUDGET_EXCEEDED',
      `sections must contain ${KNIFE_ATTACHMENT_LOFT_LIMITS.min_sections} to ${KNIFE_ATTACHMENT_LOFT_LIMITS.max_sections} entries`,
    )
  }

  let ringPointIds: readonly string[] | undefined
  let ringPointCount = 0
  const sectionIds = new Set<string>()
  for (const [sectionIndex, sectionValue] of value.sections.entries()) {
    exactKeys(sectionValue, ['section_id', 'ring'], `sections[${sectionIndex}]`)
    boundedId(sectionValue.section_id, `sections[${sectionIndex}].section_id`)
    if (sectionIds.has(sectionValue.section_id)) {
      fail('INVALID_SPEC', `duplicate section_id ${sectionValue.section_id}`)
    }
    sectionIds.add(sectionValue.section_id)
    if (!Array.isArray(sectionValue.ring)
      || sectionValue.ring.length < KNIFE_ATTACHMENT_LOFT_LIMITS.min_ring_points
      || sectionValue.ring.length > KNIFE_ATTACHMENT_LOFT_LIMITS.max_ring_points) {
      fail(
        'BUDGET_EXCEEDED',
        `sections[${sectionIndex}].ring must contain ${KNIFE_ATTACHMENT_LOFT_LIMITS.min_ring_points} to ${KNIFE_ATTACHMENT_LOFT_LIMITS.max_ring_points} points`,
      )
    }
    if (sectionIndex === 0) {
      ringPointCount = sectionValue.ring.length
    } else if (sectionValue.ring.length !== ringPointCount) {
      fail('INVALID_SPEC', 'all sections must have the same ring point count')
    }

    const currentPointIds: string[] = []
    const currentPointIdSet = new Set<string>()
    for (const [pointIndex, pointValue] of sectionValue.ring.entries()) {
      exactKeys(pointValue, ['point_id', 'position'], `sections[${sectionIndex}].ring[${pointIndex}]`)
      boundedId(pointValue.point_id, `sections[${sectionIndex}].ring[${pointIndex}].point_id`)
      if (currentPointIdSet.has(pointValue.point_id)) {
        fail('INVALID_SPEC', `duplicate point_id ${pointValue.point_id} in section ${sectionValue.section_id}`)
      }
      currentPointIdSet.add(pointValue.point_id)
      currentPointIds.push(pointValue.point_id)
      finitePosition(pointValue.position, `sections[${sectionIndex}].ring[${pointIndex}].position`)
    }
    if (!ringPointIds) ringPointIds = currentPointIds
    if (ringPointIds.join('|') !== currentPointIds.join('|')) {
      fail('INVALID_SPEC', 'all sections must preserve the same ordered point IDs')
    }
    analyzeRing(sectionValue.ring, sectionIndex)
  }

  const vertexCount = value.sections.length * ringPointCount
  const triangleCount = sideTriangleCount(value.sections.length, ringPointCount) + 2 * (ringPointCount - 2)
  if (vertexCount > KNIFE_ATTACHMENT_LOFT_LIMITS.max_vertices || triangleCount > KNIFE_ATTACHMENT_LOFT_LIMITS.max_triangles) {
    fail(
      'BUDGET_EXCEEDED',
      `loft emits ${vertexCount} vertices and ${triangleCount} triangles above the fixed budget`,
    )
  }
}

export function compileKnifeAttachmentLoft(spec: KnifeAttachmentLoftSpec): KnifeAttachmentLoftResult {
  validateKnifeAttachmentLoftSpec(spec)
  const sectionCount = spec.sections.length
  const ringPointCount = spec.sections[0].ring.length
  const analyses = spec.sections.map((section, index) => analyzeRing(section.ring, index))

  for (let sectionIndex = 1; sectionIndex < analyses.length; sectionIndex += 1) {
    const previous = analyses[sectionIndex - 1]
    const current = analyses[sectionIndex]
    if (previous.normal.dot(current.normal) <= MIN_FOLDOVER_DOT) {
      fail('SELF_INTERSECTION', `section ${sectionIndex - 1} to ${sectionIndex} reverses the ring normal`)
    }
    const previousCenter = centroid(previous.points)
    const currentCenter = centroid(current.points)
    if (previousCenter.distanceTo(currentCenter) <= MIN_EDGE_LENGTH) {
      fail('DEGENERATE_PROFILE', `section ${sectionIndex - 1} and ${sectionIndex} share a degenerate center transition`)
    }
  }

  const positions = new Float32Array(sectionCount * ringPointCount * 3)
  const uvs = new Float32Array(sectionCount * ringPointCount * 2)
  const sectionIndices = new Float32Array(sectionCount * ringPointCount)
  const ringIndices = new Float32Array(sectionCount * ringPointCount)
  for (let sectionIndex = 0; sectionIndex < sectionCount; sectionIndex += 1) {
    const points = analyses[sectionIndex].points
    for (let ringIndex = 0; ringIndex < ringPointCount; ringIndex += 1) {
      const vertexIndex = sectionIndex * ringPointCount + ringIndex
      const point = points[ringIndex]
      positions[vertexIndex * 3] = point.x
      positions[vertexIndex * 3 + 1] = point.y
      positions[vertexIndex * 3 + 2] = point.z
      uvs[vertexIndex * 2] = sectionIndex / Math.max(sectionCount - 1, 1)
      uvs[vertexIndex * 2 + 1] = ringIndex / ringPointCount
      sectionIndices[vertexIndex] = sectionIndex
      ringIndices[vertexIndex] = ringIndex
    }
  }

  const indices: number[] = []
  for (let sectionIndex = 0; sectionIndex < sectionCount - 1; sectionIndex += 1) {
    for (let ringIndex = 0; ringIndex < ringPointCount; ringIndex += 1) {
      const nextRingIndex = (ringIndex + 1) % ringPointCount
      const a = sectionIndex * ringPointCount + ringIndex
      const b = (sectionIndex + 1) * ringPointCount + ringIndex
      const c = (sectionIndex + 1) * ringPointCount + nextRingIndex
      const d = sectionIndex * ringPointCount + nextRingIndex
      validateQuad(positions, a, b, c, d, sectionIndex, ringIndex)
      indices.push(a, b, c, a, c, d)
    }
  }

  const firstCap = triangulateCap(analyses[0].projected, 0)
  const lastCap = triangulateCap(analyses[analyses.length - 1].projected, sectionCount - 1)
  for (const [a, b, c] of firstCap) indices.push(c, b, a)
  for (const [a, b, c] of lastCap) {
    const offset = (sectionCount - 1) * ringPointCount
    indices.push(offset + a, offset + b, offset + c)
  }

  const triangleCount = indices.length / 3
  if (triangleCount !== sideTriangleCount(sectionCount, ringPointCount) + 2 * (ringPointCount - 2)) {
    fail('INVALID_SPEC', 'deterministic cap triangulation emitted an unexpected triangle count')
  }

  const geometry = new THREE.BufferGeometry()
  geometry.name = `attachment-loft:${spec.attachment_id}`
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3))
  geometry.setAttribute('uv', new THREE.Float32BufferAttribute(uvs, 2))
  geometry.setAttribute('sectionIndex', new THREE.Float32BufferAttribute(sectionIndices, 1))
  geometry.setAttribute('ringIndex', new THREE.Float32BufferAttribute(ringIndices, 1))
  geometry.setIndex(new THREE.Uint32BufferAttribute(indices, 1))
  geometry.computeVertexNormals()
  geometry.computeBoundingBox()
  geometry.computeBoundingSphere()
  assertFiniteAttribute(geometry.getAttribute('normal'), 'derived normal')

  const deterministicFingerprint = fingerprint(spec, positions, indices)
  overrideUuid(geometry, stableUuid(`attachment:${spec.attachment_id}:${deterministicFingerprint}`))
  geometry.userData = {
    schema_version: KNIFE_ATTACHMENT_LOFT_SCHEMA_VERSION,
    attachment_id: spec.attachment_id,
    role: spec.role,
    section_ids: spec.sections.map((section) => section.section_id),
    ring_point_ids: [...spec.sections[0].ring].map((point) => point.point_id),
    cap_ends: true,
    welded_indexed: true,
    vertex_count: positions.length / 3,
    triangle_count: triangleCount,
    deterministic_fingerprint: deterministicFingerprint,
    self_intersection_check: 'planar-ring-and-adjacent-quad-foldover@1',
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  }

  return Object.freeze({
    schema_version: KNIFE_ATTACHMENT_LOFT_SCHEMA_VERSION,
    attachment_id: spec.attachment_id,
    role: spec.role,
    geometry,
    section_count: sectionCount,
    ring_point_count: ringPointCount,
    vertex_count: positions.length / 3,
    triangle_count: triangleCount,
    welded_indexed: true,
    deterministic_fingerprint: deterministicFingerprint,
  })
}

export const buildKnifeAttachmentLoft = compileKnifeAttachmentLoft

function analyzeRing(
  ring: readonly KnifeAttachmentLoftRingPoint[],
  sectionIndex: number,
): RingAnalysis {
  const points = ring.map((point) => new THREE.Vector3(point.position[0], point.position[1], point.position[2]))
  const normal = newellNormal(points)
  if (normal.length() <= MIN_AREA) {
    fail('DEGENERATE_PROFILE', `section ${sectionIndex} ring has near-zero area`)
  }
  const unitNormal = normal.clone().normalize()
  const origin = points[0]
  const maxPlanarError = points.reduce((maximum, point) => Math.max(maximum, Math.abs(point.clone().sub(origin).dot(unitNormal))), 0)
  if (maxPlanarError > PLANAR_TOLERANCE) {
    fail('DEGENERATE_PROFILE', `section ${sectionIndex} ring is not a bounded planar profile`)
  }
  const axis = dominantAxis(normal)
  const projected = points.map((point) => projectPoint(point, axis))
  const signedArea = polygonArea(projected)
  if (Math.abs(signedArea) <= MIN_AREA) {
    fail('DEGENERATE_PROFILE', `section ${sectionIndex} ring projection has near-zero area`)
  }
  for (let edgeIndex = 0; edgeIndex < projected.length; edgeIndex += 1) {
    const nextEdgeIndex = (edgeIndex + 1) % projected.length
    if (distance2(projected[edgeIndex], projected[nextEdgeIndex]) <= MIN_EDGE_LENGTH ** 2) {
      fail('DEGENERATE_PROFILE', `section ${sectionIndex} ring has a zero-length edge`)
    }
    for (let otherEdgeIndex = edgeIndex + 1; otherEdgeIndex < projected.length; otherEdgeIndex += 1) {
      if (edgesAreAdjacent(edgeIndex, otherEdgeIndex, projected.length)) continue
      const otherNextEdgeIndex = (otherEdgeIndex + 1) % projected.length
      if (segmentsIntersect(projected[edgeIndex], projected[nextEdgeIndex], projected[otherEdgeIndex], projected[otherNextEdgeIndex])) {
        fail('SELF_INTERSECTION', `section ${sectionIndex} ring edges ${edgeIndex} and ${otherEdgeIndex} self-intersect`)
      }
    }
  }
  return { points, projected, normal: unitNormal, signed_area: signedArea }
}

function validateQuad(
  positions: Float32Array,
  a: number,
  b: number,
  c: number,
  d: number,
  sectionIndex: number,
  ringIndex: number,
): void {
  const pa = vectorAt(positions, a)
  const pb = vectorAt(positions, b)
  const pc = vectorAt(positions, c)
  const pd = vectorAt(positions, d)
  const firstNormal = pb.clone().sub(pa).cross(pc.clone().sub(pa))
  const secondNormal = pc.clone().sub(pa).cross(pd.clone().sub(pa))
  if (firstNormal.length() <= MIN_FACE_AREA || secondNormal.length() <= MIN_FACE_AREA) {
    fail('DEGENERATE_PROFILE', `section transition ${sectionIndex} ring edge ${ringIndex} has a zero-area face`)
  }
  const dot = firstNormal.normalize().dot(secondNormal.normalize())
  if (dot <= MIN_FOLDOVER_DOT) {
    fail('SELF_INTERSECTION', `section transition ${sectionIndex} ring edge ${ringIndex} folds over`)
  }
}

function triangulateCap(points: readonly Point2[], sectionIndex: number): readonly (readonly [number, number, number])[] {
  const winding = polygonArea(points) >= 0 ? 1 : -1
  const remaining = Array.from({ length: points.length }, (_, index) => index)
  const triangles: [number, number, number][] = []
  while (remaining.length > 3) {
    let earFound = false
    for (let cursor = 0; cursor < remaining.length; cursor += 1) {
      const previous = remaining[(cursor + remaining.length - 1) % remaining.length]
      const current = remaining[cursor]
      const next = remaining[(cursor + 1) % remaining.length]
      if (triangleArea2(points[previous], points[current], points[next]) * winding <= MIN_AREA) continue
      if (remaining.some((candidate) => candidate !== previous && candidate !== current && candidate !== next
        && pointInTriangle(points[candidate], points[previous], points[current], points[next]))) continue
      triangles.push([previous, current, next])
      remaining.splice(cursor, 1)
      earFound = true
      break
    }
    if (!earFound) fail('SELF_INTERSECTION', `section ${sectionIndex} cap cannot be triangulated without a foldover`)
  }
  const [a, b, c] = remaining
  if (triangleArea2(points[a], points[b], points[c]) * winding <= MIN_AREA) {
    fail('DEGENERATE_PROFILE', `section ${sectionIndex} cap has a zero-area terminal triangle`)
  }
  triangles.push([a, b, c])
  return triangles
}

function exactKeys(value: unknown, expected: readonly string[], label: string): asserts value is Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) fail('INVALID_SPEC', `${label} must be an object`)
  const actual = Object.keys(value).sort()
  const wanted = [...expected].sort()
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) {
    fail('INVALID_SPEC', `${label} keys are not closed`)
  }
}

function boundedId(value: unknown, label: string): asserts value is string {
  if (typeof value !== 'string' || !STABLE_ID_PATTERN.test(value)) fail('INVALID_SPEC', `${label} must be a bounded stable ID`)
}

function finitePosition(value: unknown, label: string): asserts value is KnifeAttachmentLoftVec3 {
  if (!Array.isArray(value) || value.length !== 3
    || value.some((coordinate) => typeof coordinate !== 'number'
      || !Number.isFinite(coordinate)
      || Math.abs(coordinate) > KNIFE_ATTACHMENT_LOFT_LIMITS.max_coordinate_abs)) {
    fail('INVALID_SPEC', `${label} must be finite and within the normalized coordinate bound`)
  }
}

function fail(code: KnifeAttachmentLoftErrorCode, message: string): never {
  throw new KnifeAttachmentLoftError(code, message)
}

function sideTriangleCount(sectionCount: number, ringPointCount: number): number {
  return (sectionCount - 1) * ringPointCount * 2
}

function centroid(points: readonly THREE.Vector3[]): THREE.Vector3 {
  const center = new THREE.Vector3()
  for (const point of points) center.add(point)
  return center.multiplyScalar(1 / points.length)
}

function newellNormal(points: readonly THREE.Vector3[]): THREE.Vector3 {
  const normal = new THREE.Vector3()
  for (let index = 0; index < points.length; index += 1) {
    const current = points[index]
    const next = points[(index + 1) % points.length]
    normal.x += (current.y - next.y) * (current.z + next.z)
    normal.y += (current.z - next.z) * (current.x + next.x)
    normal.z += (current.x - next.x) * (current.y + next.y)
  }
  return normal
}

function dominantAxis(normal: THREE.Vector3): 0 | 1 | 2 {
  const absolute = [Math.abs(normal.x), Math.abs(normal.y), Math.abs(normal.z)]
  if (absolute[0] >= absolute[1] && absolute[0] >= absolute[2]) return 0
  if (absolute[1] >= absolute[2]) return 1
  return 2
}

function projectPoint(point: THREE.Vector3, axis: 0 | 1 | 2): Point2 {
  if (axis === 0) return { x: point.y, y: point.z }
  if (axis === 1) return { x: point.x, y: point.z }
  return { x: point.x, y: point.y }
}

function polygonArea(points: readonly Point2[]): number {
  let area = 0
  for (let index = 0; index < points.length; index += 1) {
    const current = points[index]
    const next = points[(index + 1) % points.length]
    area += current.x * next.y - next.x * current.y
  }
  return area * 0.5
}

function triangleArea2(a: Point2, b: Point2, c: Point2): number {
  return (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

function distance2(left: Point2, right: Point2): number {
  const dx = left.x - right.x
  const dy = left.y - right.y
  return dx * dx + dy * dy
}

function edgesAreAdjacent(left: number, right: number, count: number): boolean {
  return left === right || (left + 1) % count === right || (right + 1) % count === left
}

function segmentsIntersect(a: Point2, b: Point2, c: Point2, d: Point2): boolean {
  const abC = triangleArea2(a, b, c)
  const abD = triangleArea2(a, b, d)
  const cdA = triangleArea2(c, d, a)
  const cdB = triangleArea2(c, d, b)
  const epsilon = MIN_AREA
  if (Math.abs(abC) <= epsilon && onSegment(a, b, c)) return true
  if (Math.abs(abD) <= epsilon && onSegment(a, b, d)) return true
  if (Math.abs(cdA) <= epsilon && onSegment(c, d, a)) return true
  if (Math.abs(cdB) <= epsilon && onSegment(c, d, b)) return true
  return ((abC > epsilon && abD < -epsilon) || (abC < -epsilon && abD > epsilon))
    && ((cdA > epsilon && cdB < -epsilon) || (cdA < -epsilon && cdB > epsilon))
}

function onSegment(a: Point2, b: Point2, point: Point2): boolean {
  return point.x >= Math.min(a.x, b.x) - MIN_AREA
    && point.x <= Math.max(a.x, b.x) + MIN_AREA
    && point.y >= Math.min(a.y, b.y) - MIN_AREA
    && point.y <= Math.max(a.y, b.y) + MIN_AREA
}

function pointInTriangle(point: Point2, a: Point2, b: Point2, c: Point2): boolean {
  const ab = triangleArea2(a, b, point)
  const bc = triangleArea2(b, c, point)
  const ca = triangleArea2(c, a, point)
  const hasNegative = ab < -MIN_AREA || bc < -MIN_AREA || ca < -MIN_AREA
  const hasPositive = ab > MIN_AREA || bc > MIN_AREA || ca > MIN_AREA
  return !(hasNegative && hasPositive)
}

function vectorAt(values: Float32Array, index: number): THREE.Vector3 {
  return new THREE.Vector3(values[index * 3], values[index * 3 + 1], values[index * 3 + 2])
}

function assertFiniteAttribute(
  attribute: THREE.BufferAttribute | THREE.InterleavedBufferAttribute,
  label: string,
): void {
  for (const value of attribute.array) {
    if (!Number.isFinite(value)) fail('DEGENERATE_PROFILE', `${label} contains a non-finite value`)
  }
}

function fingerprint(spec: KnifeAttachmentLoftSpec, positions: Float32Array, indices: readonly number[]): string {
  const values = [KNIFE_ATTACHMENT_LOFT_SCHEMA_VERSION, spec.attachment_id, spec.role, String(spec.cap_ends)]
  for (const section of spec.sections) {
    values.push(section.section_id)
    for (const point of section.ring) values.push(point.point_id, ...point.position.map(canonicalNumber))
  }
  values.push(...Array.from(positions, canonicalNumber), ...indices.map(String))
  return fnv1a64(values.join('|'))
}

function canonicalNumber(value: number): string {
  if (!Number.isFinite(value)) return 'NON_FINITE'
  return Object.is(value, -0) ? '0' : value.toPrecision(12)
}

function stableUuid(value: string): string {
  const raw = `${fnv1a64(`${value}:0`)}${fnv1a64(`${value}:1`)}${fnv1a64(`${value}:2`)}${fnv1a64(`${value}:3`)}`
  return `${raw.slice(0, 8)}-${raw.slice(8, 12)}-${raw.slice(12, 16)}-${raw.slice(16, 20)}-${raw.slice(20, 32)}`
}

function overrideUuid(object: { readonly uuid: string }, uuid: string): void {
  Object.defineProperty(object, 'uuid', { configurable: true, enumerable: true, value: uuid, writable: true })
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
