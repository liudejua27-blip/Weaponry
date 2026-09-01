import * as THREE from 'three'

import {
  validateImg2ThreeJsSourceEnvelope,
  type Img2ThreeJsSourceComponent,
  type Img2ThreeJsSourceEnvelope,
  type Img2ThreeJsSourceTransform,
  type Img2ThreeJsSourceVec2,
  type Img2ThreeJsSourceVec3,
} from './img2threejs-source-envelope.ts'
import type { KnifePartRole, KnifeVec3 } from './knife-scene-program.ts'

export type Img2ThreeJsCompatibilitySurfaceRole = Exclude<KnifePartRole, 'helper'>

export interface Img2ThreeJsCompatibilityPart {
  readonly part_id: string
  readonly source_component_id: string
  readonly source_primitive: Img2ThreeJsSourceComponent['primitive']
  readonly surface_role: Img2ThreeJsCompatibilitySurfaceRole
  readonly assembly_primitive?: Exclude<Img2ThreeJsCompatibilitySurfaceRole, 'blade-body' | 'cutting-edge'>
  readonly center: KnifeVec3
  readonly geometry: THREE.BufferGeometry
  readonly descriptor: Readonly<Record<string, string | number | boolean>>
  readonly triangle_count: number
}

export interface CompiledImg2ThreeJsCompatibilityEnvelope {
  readonly envelope: Img2ThreeJsSourceEnvelope
  readonly parts: readonly Img2ThreeJsCompatibilityPart[]
  readonly triangle_count: number
  readonly deterministic_fingerprint: string
}

export class Img2ThreeJsCompatibilityCompileError extends Error {
  constructor(message: string) {
    super(`IMG2THREEJS_COMPATIBILITY_COMPILE_INVALID: ${message}`)
    this.name = 'Img2ThreeJsCompatibilityCompileError'
  }
}

const MAX_TRIANGLES = 1_000_000

/**
 * Compile the closed source envelope with the same primitive semantics used
 * by the pinned generator.  No semantic Box/Cylinder/Octahedron substitution
 * is performed: an unsupported source primitive is a hard error.
 */
export function compileImg2ThreeJsSourceEnvelope(value: Img2ThreeJsSourceEnvelope): CompiledImg2ThreeJsCompatibilityEnvelope {
  validateImg2ThreeJsSourceEnvelope(value)
  const components = [...value.components].sort((left, right) => left.source_order - right.source_order || left.component_id.localeCompare(right.component_id))
  const bladeComponents = components.filter((component) => component.primitive === 'ground-blade')
  if (bladeComponents.length !== 1) throw new Img2ThreeJsCompatibilityCompileError('exactly one ground-blade component is required')

  const parts: Img2ThreeJsCompatibilityPart[] = []
  for (const component of components) {
    if (component.primitive === 'ground-blade') {
      const blade = compileGroundBlade(component, value.tessellation)
      parts.push(
        makePart(component, 'blade-body', undefined, blade.body, blade.bodyTriangles),
        makePart(component, 'cutting-edge', undefined, blade.edge, blade.edgeTriangles),
      )
      continue
    }
    const surfaceRole = roleFor(component)
    const geometry = geometryFor(component, value.tessellation)
    const transformed = applyLocalTransform(geometry, component.transform)
    const assemblyPrimitive = surfaceRole as Exclude<Img2ThreeJsCompatibilitySurfaceRole, 'blade-body' | 'cutting-edge'>
    parts.push(makePart(component, surfaceRole, assemblyPrimitive, transformed, triangleCount(transformed)))
  }

  const totalTriangles = parts.reduce((total, part) => total + part.triangle_count, 0)
  if (!Number.isInteger(totalTriangles) || totalTriangles <= 0 || totalTriangles > MAX_TRIANGLES || totalTriangles > value.max_triangles) {
    throw new Img2ThreeJsCompatibilityCompileError(`triangle budget exceeded: ${totalTriangles} > ${value.max_triangles}`)
  }
  const deterministicFingerprint = fingerprint(value, parts)
  return {
    envelope: value,
    parts: Object.freeze(parts),
    triangle_count: totalTriangles,
    deterministic_fingerprint: deterministicFingerprint,
  }
}

function makePart(
  component: Img2ThreeJsSourceComponent,
  surfaceRole: Img2ThreeJsCompatibilitySurfaceRole,
  assemblyPrimitive: Exclude<Img2ThreeJsCompatibilitySurfaceRole, 'blade-body' | 'cutting-edge'> | undefined,
  geometry: THREE.BufferGeometry,
  triangles: number,
): Img2ThreeJsCompatibilityPart {
  geometry.userData = {
    schema_version: 'Img2ThreeJsSourceEnvelope@1',
    source_component_id: component.component_id,
    source_order: component.source_order,
    source_primitive: component.primitive,
    source_role: component.role,
    source_geometry_fidelity: 'EXACT',
    source_material_id: component.material_id,
    source_parent_id: component.parent_id,
    source_transform: {
      position: [...component.transform.position],
      rotation_xyz: [...component.transform.rotation_xyz],
      scale: [...component.transform.scale],
      pivot: [...component.transform.pivot],
      rotation_order: component.transform.rotation_order,
    },
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  }
  return {
    part_id: surfaceRole === 'blade-body' || surfaceRole === 'cutting-edge' ? surfaceRole : component.component_id,
    source_component_id: component.component_id,
    source_primitive: component.primitive,
    surface_role: surfaceRole,
    assembly_primitive: assemblyPrimitive,
    center: component.transform.position,
    geometry,
    descriptor: {
      source_component_id: component.component_id,
      source_order: component.source_order,
      source_primitive: component.primitive,
      source_role: component.role,
      source_geometry_fidelity: 'EXACT',
      source_material_id: component.material_id,
      parent_bound: component.parent_id !== null,
      rotation_order_xyz: true,
    },
    triangle_count: triangles,
  }
}

function roleFor(component: Img2ThreeJsSourceComponent): Exclude<Img2ThreeJsCompatibilitySurfaceRole, 'blade-body' | 'cutting-edge'> {
  switch (component.role) {
    case 'guard':
      return 'guard'
    case 'grip':
      return 'grip'
    case 'pommel':
      return 'pommel'
    case 'fastener':
      return 'fastener'
    case 'gem':
      return 'gem'
    case 'relief':
      return 'relief'
    case 'blade':
      throw new Img2ThreeJsCompatibilityCompileError(`blade component ${component.component_id} has no assembly role`)
  }
}

function geometryFor(component: Img2ThreeJsSourceComponent, tier: Img2ThreeJsSourceEnvelope['tessellation']): THREE.BufferGeometry {
  switch (component.geometry.primitive) {
    case 'extrude': {
      const shape = shapeFromPoints(component.geometry.profile_2d)
      return new THREE.ExtrudeGeometry(shape, {
        depth: component.geometry.depth,
        bevelEnabled: false,
        steps: 1,
      })
    }
    case 'curve-sweep': {
      const shape = shapeFromPoints(component.geometry.cross_section)
      const path = new THREE.CatmullRomCurve3(
        component.geometry.spine.map((point) => new THREE.Vector3(point[0], point[1], point[2])),
        component.geometry.closed,
      )
      return new THREE.ExtrudeGeometry(shape, {
        extrudePath: path,
        steps: Math.max(24, component.geometry.spine.length * 8),
        bevelEnabled: false,
      })
    }
    case 'sphere': {
      const segments = tessellation(tier)
      return new THREE.SphereGeometry(0.5, segments.sphere_width, segments.sphere_height)
    }
    case 'cylinder': {
      const segments = tessellation(tier)
      return new THREE.CylinderGeometry(0.5, 0.5, 1, segments.cylinder_radial, segments.cylinder_height)
    }
    case 'ground-blade':
      throw new Img2ThreeJsCompatibilityCompileError(`ground-blade must use the dedicated dual-track compiler: ${component.component_id}`)
  }
}

function shapeFromPoints(points: readonly Img2ThreeJsSourceVec2[]): THREE.Shape {
  const shape = new THREE.Shape()
  shape.moveTo(points[0][0], points[0][1])
  for (let index = 1; index < points.length; index += 1) shape.lineTo(points[index][0], points[index][1])
  shape.closePath()
  return shape
}

interface GroundBladeCompileResult {
  readonly body: THREE.BufferGeometry
  readonly edge: THREE.BufferGeometry
  readonly bodyTriangles: number
  readonly edgeTriangles: number
}

function compileGroundBlade(
  component: Img2ThreeJsSourceComponent,
  _tier: Img2ThreeJsSourceEnvelope['tessellation'],
): GroundBladeCompileResult {
  const spec = component.geometry
  if (spec.primitive !== 'ground-blade') throw new Img2ThreeJsCompatibilityCompileError(`component ${component.component_id} is not a ground-blade`)
  const stations = spec.stations
  const xGround = stations[0][0]
  const xTip = stations[stations.length - 1][0]
  const length = xTip - xGround
  if (length <= 1e-6) throw new Img2ThreeJsCompatibilityCompileError(`blade ${component.component_id} has no longitudinal span`)
  let yMin = Number.POSITIVE_INFINITY
  let yMax = Number.NEGATIVE_INFINITY
  for (const station of stations) {
    yMin = Math.min(yMin, station[2])
    yMax = Math.max(yMax, station[1])
  }
  const yHeight = Math.max(yMax - yMin, 1e-6)
  const thickness = spec.thickness * 0.5
  const rings = stations.map((station) => {
    const [x, topY, bottomY] = station
    const height = Math.max(1e-4, topY - bottomY)
    const grindY = bottomY + spec.grind_frac * height
    const swedgeY = topY - 0.42 * height
    const swedgeZ = ((xTip - x) / length < spec.swedge_from_tip_frac) ? 0 : thickness
    return [
      [x, bottomY, 0],
      [x, grindY, thickness],
      [x, swedgeY, thickness],
      [x, topY, swedgeZ],
      [x, topY, -swedgeZ],
      [x, swedgeY, -thickness],
      [x, grindY, -thickness],
    ] as readonly Img2ThreeJsSourceVec3[]
  })

  type Triangle = readonly [Img2ThreeJsSourceVec3, Img2ThreeJsSourceVec3, Img2ThreeJsSourceVec3]
  const bodyTriangles: Triangle[] = []
  const edgeTriangles: Triangle[] = []
  const append = (target: Triangle[], a: Img2ThreeJsSourceVec3, b: Img2ThreeJsSourceVec3, c: Img2ThreeJsSourceVec3): void => {
    target.push([a, b, c])
  }
  const root = rings[0]
  for (let ringIndex = 1; ringIndex < 6; ringIndex += 1) append(bodyTriangles, root[0], root[ringIndex], root[ringIndex + 1])
  let current = root
  for (let stationIndex = 1; stationIndex < rings.length; stationIndex += 1) {
    const next = rings[stationIndex]
    for (let ringIndex = 0; ringIndex < 7; ringIndex += 1) {
      const nextRingIndex = (ringIndex + 1) % 7
      const target = ringIndex === 0 ? edgeTriangles : bodyTriangles
      append(target, current[ringIndex], current[nextRingIndex], next[nextRingIndex])
      append(target, current[ringIndex], next[nextRingIndex], next[ringIndex])
    }
    current = next
  }

  return {
    body: geometryFromTriangles(bodyTriangles, xGround, length, yMin, yHeight, component),
    edge: geometryFromTriangles(edgeTriangles, xGround, length, yMin, yHeight, component),
    bodyTriangles: bodyTriangles.length,
    edgeTriangles: edgeTriangles.length,
  }
}

function geometryFromTriangles(
  triangles: readonly (readonly [Img2ThreeJsSourceVec3, Img2ThreeJsSourceVec3, Img2ThreeJsSourceVec3])[],
  xGround: number,
  length: number,
  yMin: number,
  yHeight: number,
  component: Img2ThreeJsSourceComponent,
): THREE.BufferGeometry {
  const positions: number[] = []
  const uvs: number[] = []
  for (const triangle of triangles) {
    for (const point of triangle) {
      positions.push(point[0], point[1], point[2])
      uvs.push((point[0] - xGround) / length, (point[1] - yMin) / yHeight)
    }
  }
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3))
  geometry.setAttribute('uv', new THREE.Float32BufferAttribute(uvs, 2))
  geometry.computeVertexNormals()
  const transformed = applyLocalTransform(geometry, component.transform)
  return transformed
}

function applyLocalTransform(geometry: THREE.BufferGeometry, transform: Img2ThreeJsSourceTransform): THREE.BufferGeometry {
  const pivot = transform.pivot
  const local = new THREE.Matrix4()
    .makeTranslation(pivot[0], pivot[1], pivot[2])
    .multiply(new THREE.Matrix4().makeRotationFromEuler(new THREE.Euler(
      transform.rotation_xyz[0],
      transform.rotation_xyz[1],
      transform.rotation_xyz[2],
      'XYZ',
    )))
    .multiply(new THREE.Matrix4().makeScale(transform.scale[0], transform.scale[1], transform.scale[2]))
    .multiply(new THREE.Matrix4().makeTranslation(-pivot[0], -pivot[1], -pivot[2]))
  geometry.applyMatrix4(local)
  return geometry
}

function tessellation(tier: Img2ThreeJsSourceEnvelope['tessellation']): {
  readonly sphere_width: number
  readonly sphere_height: number
  readonly cylinder_radial: number
  readonly cylinder_height: number
} {
  if (tier === 'low') return { sphere_width: 16, sphere_height: 10, cylinder_radial: 10, cylinder_height: 4 }
  if (tier === 'standard') return { sphere_width: 32, sphere_height: 20, cylinder_radial: 24, cylinder_height: 8 }
  return { sphere_width: 64, sphere_height: 40, cylinder_radial: 48, cylinder_height: 16 }
}

function triangleCount(geometry: THREE.BufferGeometry): number {
  const index = geometry.getIndex()
  const position = geometry.getAttribute('position')
  const count = index ? index.count : position.count
  const triangles = count / 3
  if (!Number.isInteger(triangles) || triangles <= 0) throw new Img2ThreeJsCompatibilityCompileError('geometry emitted an invalid triangle count')
  return triangles
}

function fingerprint(envelope: Img2ThreeJsSourceEnvelope, parts: readonly Img2ThreeJsCompatibilityPart[]): string {
  const tokens: string[] = [
    envelope.schema_version,
    envelope.source_schema_version,
    envelope.source_identity.revision,
    envelope.source_identity.tree,
    envelope.source_identity.generator_sha256,
    envelope.source_identity.validator_sha256,
    envelope.target_name,
    envelope.coordinate_frame,
    envelope.tessellation,
    envelope.max_triangles.toString(),
  ]
  for (const material of envelope.materials) tokens.push(material.material_id, material.source_order.toString(), material.base_color, material.metalness.toString(), material.roughness.toString())
  for (const part of parts) {
    tokens.push(part.part_id, part.source_component_id, part.source_primitive, part.surface_role, part.triangle_count.toString())
    tokens.push(...part.center.map((value) => value.toString()))
    const position = part.geometry.getAttribute('position')
    for (let index = 0; index < position.count * position.itemSize; index += 1) tokens.push(Number(position.array[index]).toString())
    const uv = part.geometry.getAttribute('uv')
    if (uv) for (let index = 0; index < uv.count * uv.itemSize; index += 1) tokens.push(Number(uv.array[index]).toString())
    const normal = part.geometry.getAttribute('normal')
    if (normal) for (let index = 0; index < normal.count * normal.itemSize; index += 1) tokens.push(Number(normal.array[index]).toString())
    const indices = part.geometry.getIndex()
    if (indices) for (let index = 0; index < indices.count; index += 1) tokens.push(Number(indices.array[index]).toString())
  }
  return fnv1a64(tokens.join('|'))
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
