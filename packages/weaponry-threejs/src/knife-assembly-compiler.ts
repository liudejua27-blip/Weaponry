import * as THREE from 'three'

import {
  compileKnifeAttachmentLoft,
  type KnifeAttachmentLoftRole,
  type KnifeAttachmentLoftVec3,
} from './knife-attachment-loft.ts'
import {
  compileReliefCurveGraph,
  type ReliefCurvePath,
  type ReliefLocalFrame,
} from './knife-relief-curve.ts'

import type {
  KnifeAssembly,
  KnifeAssemblyAxis,
  KnifeAssemblyPrimitiveSpec,
  KnifeDragonGuardSpec,
  KnifeDragonEyeSocketSpec,
  KnifeDragonHornSpec,
  KnifeFastenerSpec,
  KnifeGemSpec,
  KnifeGripSpec,
  KnifeHookedPommelSpec,
  KnifePommelGemSeatSpec,
  KnifeGuardSpec,
  KnifePommelSpec,
  KnifeReliefSpec,
  KnifeVec3,
} from './knife-scene-program.ts'

const MIN_DIMENSION = 1e-5

export type KnifeAssemblyDescriptorValue = number | string | boolean
export type KnifeAssemblyDescriptor = Readonly<Record<string, KnifeAssemblyDescriptorValue>>

export interface CompiledAssemblyGeometry {
  readonly primitive: KnifeAssemblyPrimitiveSpec['primitive']
  readonly part_id: string
  readonly center: KnifeVec3
  readonly geometry: THREE.BufferGeometry
  readonly descriptor: KnifeAssemblyDescriptor
}

/**
 * Compile only the closed assembly vocabulary.  The returned geometry is
 * local to each semantic part; the scene compiler applies the declared center
 * and binds the material/part lineage. No Runtime, Store, CAS, browser, or
 * caller-provided code is reachable from this function.
 */
export function compileKnifeAssemblyGeometry(
  assembly?: KnifeAssembly,
  materialZoneByPart: Readonly<Record<string, string>> = {},
): readonly CompiledAssemblyGeometry[] {
  if (!assembly) return []

  const specs: KnifeAssemblyPrimitiveSpec[] = []
  if (assembly.guard) specs.push(assembly.guard)
  if (assembly.grip) specs.push(assembly.grip)
  if (assembly.pommel) specs.push(assembly.pommel)
  specs.push(...(assembly.fasteners ?? []), ...(assembly.gems ?? []), ...(assembly.reliefs ?? []))
  specs.sort((left, right) => {
    const primitiveOrder: Record<KnifeAssemblyPrimitiveSpec['primitive'], number> = {
      guard: 0,
      grip: 1,
      pommel: 2,
      fastener: 3,
      gem: 4,
      relief: 5,
    }
    return primitiveOrder[left.primitive] - primitiveOrder[right.primitive] || stableCompare(left.part_id, right.part_id)
  })

  return specs.map((spec) => {
    const geometry = geometryFor(spec, materialZoneByPart[spec.part_id])
    geometry.userData = {
      schema_version: 'KnifeAssemblyPrimitive@1',
      primitive: spec.primitive,
      part_id: spec.part_id,
      center: [...spec.center],
      descriptor: descriptorFor(spec),
      renderer_invoked: false,
      quality_status: 'NOT_RUN',
    }
    return {
      primitive: spec.primitive,
      part_id: spec.part_id,
      center: spec.center,
      geometry,
      descriptor: descriptorFor(spec),
    }
  })
}

function geometryFor(spec: KnifeAssemblyPrimitiveSpec, materialZoneId?: string): THREE.BufferGeometry {
  switch (spec.primitive) {
    case 'guard':
      return spec.style === 'dragon-guard' ? dragonGuardGeometry(spec) : guardGeometry(spec)
    case 'grip':
      return spec.style === 'segmented-grip' ? segmentedGripGeometry(spec) : gripGeometry(spec)
    case 'pommel':
      return spec.style === 'hooked-pommel' ? hookedPommelGeometry(spec) : pommelGeometry(spec)
    case 'fastener':
      return fastenerGeometry(spec)
    case 'gem':
      return gemGeometry(spec)
    case 'relief':
      return reliefGeometry(spec, materialZoneId)
  }
}

function guardGeometry(spec: KnifeGuardSpec): THREE.BufferGeometry {
  // x is longitudinal, y is the crossguard span, and z is front/back depth.
  return new THREE.BoxGeometry(spec.thickness, spec.span, spec.depth)
}

function dragonGuardGeometry(spec: KnifeDragonGuardSpec): THREE.BufferGeometry {
  const components: THREE.BufferGeometry[] = [
    dragonJawGeometry(spec, spec.upper_jaw, 1),
    dragonJawGeometry(spec, spec.lower_jaw, -1),
    ...spec.horns.map((horn) => dragonHornGeometry(spec, horn)),
    ...spec.eye_sockets.map((eye) => dragonEyeSocketGeometry(eye)),
  ]
  return mergeGeometries(components)
}

function dragonJawGeometry(
  spec: KnifeDragonGuardSpec,
  jaw: KnifeDragonGuardSpec['upper_jaw'],
  side: 1 | -1,
): THREE.BufferGeometry {
  const points = Array.from({ length: 7 }, (_, index) => {
    const t = index / 6
    return new THREE.Vector3(
      (t - 0.5) * jaw.span,
      jaw.offset_y + side * (spec.jaw_gap * 0.5 + jaw.curvature * 4 * t * (1 - t)),
      jaw.offset_z,
    )
  })
  return attachmentLoftAlongPath(
    `${spec.part_id}-${side > 0 ? 'upper' : 'lower'}-jaw`,
    side > 0 ? 'guard-upper-jaw' : 'guard-lower-jaw',
    points,
    points.map((_, index) => (index === 0 || index === points.length - 1 ? 0.62 : index === 3 ? 1.08 : 0.9)),
    jaw.thickness * 0.5,
    jaw.depth * 0.5,
  )
}

function dragonHornGeometry(spec: KnifeDragonGuardSpec, horn: KnifeDragonHornSpec): THREE.BufferGeometry {
  const direction = new THREE.Vector3(
    -Math.sin(Math.abs(horn.sweep)),
    horn.side * Math.cos(horn.sweep),
    Math.sin(horn.sweep) * 0.15,
  ).normalize()
  const base = new THREE.Vector3(0, horn.side * spec.span * 0.5, horn.offset_z)
  const centers = [0, 0.36, 0.72, 1].map((t) => base.clone().addScaledVector(direction, horn.length * t))
  return attachmentLoftAlongPath(
    horn.feature_id,
    'guard-horn',
    centers,
    [1, 0.78, 0.48, 0.14],
    horn.radius,
    horn.radius * 0.82,
  )
}

function dragonEyeSocketGeometry(eye: KnifeDragonEyeSocketSpec): THREE.BufferGeometry {
  const y = eye.side * Math.abs(eye.offset_y)
  const centers = [
    new THREE.Vector3(-eye.depth * 0.5, y, eye.offset_z),
    new THREE.Vector3(0, y, eye.offset_z),
    new THREE.Vector3(eye.depth * 0.5, y, eye.offset_z),
  ]
  return attachmentLoftAlongPath(
    eye.feature_id,
    'guard-eye-shell',
    centers,
    [0.72, 1, 0.72],
    eye.radius,
    eye.radius * 0.82,
  )
}

function gripGeometry(spec: KnifeGripSpec): THREE.BufferGeometry {
  if (spec.style === 'segmented-grip') return segmentedGripGeometry(spec)
  const startRadius = spec.radius * (1 - spec.taper * 0.5)
  const endRadius = spec.radius * (1 + spec.taper * 0.5)
  const geometry = new THREE.CylinderGeometry(startRadius, endRadius, spec.length, spec.facets, 1, false)
  // CylinderGeometry is y-aligned by default; handles are x-aligned.
  geometry.rotateZ(Math.PI / 2)
  return geometry
}

function segmentedGripGeometry(spec: Extract<KnifeGripSpec, { style: 'segmented-grip' }>): THREE.BufferGeometry {
  const centerline = spec.centerline.map((point) => new THREE.Vector3(point[0], point[1], point[2]))
  const path = catmullRom(centerline, 48)
  const components: THREE.BufferGeometry[] = []
  const radialSegments = Math.min(spec.facets, 16)
  for (const segment of spec.segments) {
    const segmentPoints = Array.from({ length: 5 }, (_, index) => path.getPointAt(THREE.MathUtils.lerp(segment.start_u, segment.end_u, index / 4)))
    const segmentPath = catmullRom(segmentPoints, 16)
    const midpoint = (segment.start_u + segment.end_u) * 0.5
    const taperRadius = spec.radius * (1 + spec.taper * (midpoint - 0.5))
    components.push(new THREE.TubeGeometry(segmentPath, 10, taperRadius * segment.radius_scale, Math.min(radialSegments, 8), false))
  }
  for (const frame of spec.metal_frames) {
    const point = path.getPointAt(frame.at)
    const taperRadius = spec.radius * (1 + spec.taper * (frame.at - 0.5))
    const geometry = new THREE.TorusGeometry(taperRadius * 1.04, frame.thickness, 6, 10)
    geometry.rotateY(Math.PI / 2)
    geometry.scale(frame.width / (frame.thickness * 2), 1, 1)
    geometry.translate(point.x, point.y, point.z)
    components.push(geometry)
  }
  for (const fastener of spec.fasteners) {
    const point = path.getPointAt(fastener.at)
    const taperRadius = spec.radius * (1 + spec.taper * (fastener.at - 0.5))
    const geometry = new THREE.CylinderGeometry(fastener.radius, fastener.radius, fastener.depth, 10, 1, false)
    geometry.rotateX(Math.PI / 2)
    geometry.translate(point.x, point.y, point.z + fastener.side * (taperRadius + fastener.depth * 0.5))
    components.push(geometry)
  }
  return mergeGeometries(components)
}

function pommelGeometry(spec: KnifePommelSpec): THREE.BufferGeometry {
  const geometry = new THREE.SphereGeometry(1, 12, 8)
  geometry.scale(spec.length * 0.5, spec.radius, spec.depth * 0.5)
  return geometry
}

function hookedPommelGeometry(spec: KnifeHookedPommelSpec): THREE.BufferGeometry {
  const base = pommelGeometry(spec)
  const length = spec.hook.length
  const bend = spec.hook.bend
  const direction = spec.hook.direction
  const hookPoints = [
    new THREE.Vector3(length * 0.12, 0, 0),
    new THREE.Vector3(length * 0.42, 0, 0),
    new THREE.Vector3(length * 0.7, direction * length * 0.2 * bend, 0),
    new THREE.Vector3(length * 0.92, direction * length * 0.58 * bend, 0),
    new THREE.Vector3(length * 0.7, direction * length * 0.88 * bend, 0),
    new THREE.Vector3(length * 0.46, direction * length * 0.96 * bend, 0),
  ]
  const hook = attachmentLoftAlongPath(
    `${spec.part_id}-hook`,
    'pommel-hook',
    hookPoints,
    hookPoints.map((_, index) => 1 - 0.55 * (index / Math.max(hookPoints.length - 1, 1))),
    spec.hook.radius,
    spec.hook.radius * 0.86,
  )
  const seat = gemSeatGeometry(spec.gem_seat, hookPoints[hookPoints.length - 1])
  return mergeGeometries([base, hook, seat])
}

function attachmentLoftAlongPath(
  attachmentId: string,
  role: KnifeAttachmentLoftRole,
  centers: readonly THREE.Vector3[],
  scales: readonly number[],
  lateralRadius: number,
  normalRadius: number,
): THREE.BufferGeometry {
  if (centers.length !== scales.length || centers.length < 2) {
    throw new Error('attachment loft path requires matching centers and scales')
  }
  const sections = centers.map((center, index) => {
    const previous = centers[Math.max(0, index - 1)]
    const next = centers[Math.min(centers.length - 1, index + 1)]
    const tangent = next.clone().sub(previous).normalize()
    const normal = new THREE.Vector3(0, 0, 1)
    let lateral = normal.clone().cross(tangent)
    if (lateral.lengthSq() <= MIN_DIMENSION) lateral = new THREE.Vector3(0, 1, 0)
    lateral.normalize()
    const depth = tangent.clone().cross(lateral).normalize()
    const scale = scales[index]
    const points = [
      center.clone().addScaledVector(lateral, lateralRadius * scale),
      center.clone().addScaledVector(depth, normalRadius * scale),
      center.clone().addScaledVector(lateral, -lateralRadius * scale),
      center.clone().addScaledVector(depth, -normalRadius * scale),
    ]
    return {
      section_id: `section-${index}`,
      ring: points.map((point, pointIndex) => ({
        point_id: `ring-${pointIndex}`,
        position: [point.x, point.y, point.z] as KnifeAttachmentLoftVec3,
      })),
    }
  })
  const result = compileKnifeAttachmentLoft({
    schema_version: 'KnifeAttachmentLoft@1',
    attachment_id: attachmentId,
    role,
    sections,
    cap_ends: true,
  })
  result.geometry.userData = {
    ...result.geometry.userData,
    attachment_loft_fingerprint: result.deterministic_fingerprint,
    primitive_fallback_used: false,
  }
  return result.geometry
}

function gemSeatGeometry(seat: KnifePommelGemSeatSpec, hookEnd: THREE.Vector3): THREE.BufferGeometry {
  const geometry = new THREE.TorusGeometry(seat.radius, Math.min(seat.depth * 0.25, seat.radius * 0.35), 6, 10)
  orientNormal(geometry, seat.axis)
  geometry.translate(hookEnd.x + seat.offset_x, hookEnd.y + seat.offset_y, hookEnd.z + seat.offset_z)
  return geometry
}

function fastenerGeometry(spec: KnifeFastenerSpec): THREE.BufferGeometry {
  const geometry = new THREE.CylinderGeometry(spec.radius, spec.radius, spec.depth, 12, 1, false)
  geometry.rotateX(Math.PI / 2)
  orientNormal(geometry, spec.axis)
  return geometry
}

function gemGeometry(spec: KnifeGemSpec): THREE.BufferGeometry {
  const geometry = new THREE.OctahedronGeometry(spec.radius, 0)
  geometry.scale(1, 1, Math.max(spec.depth / (2 * spec.radius), MIN_DIMENSION))
  orientNormal(geometry, spec.axis)
  return geometry
}

function reliefGeometry(spec: KnifeReliefSpec, materialZoneId?: string): THREE.BufferGeometry {
  const frame = reliefFrame(spec.axis)
  const paths: readonly ReliefCurvePath[] = spec.shape === 'panel'
    ? [
        {
          path_id: `${spec.part_id}-spine`,
          basis: 'nurbs-like' as const,
          dimension: '2d' as const,
          points: [
            [-spec.width * 0.5, -spec.height * 0.08],
            [-spec.width * 0.28, spec.height * 0.34],
            [-spec.width * 0.02, -spec.height * 0.18],
            [spec.width * 0.25, spec.height * 0.28],
            [spec.width * 0.5, 0],
          ],
        },
        {
          path_id: `${spec.part_id}-belly`,
          basis: 'nurbs-like' as const,
          dimension: '2d' as const,
          points: [
            [-spec.width * 0.4, -spec.height * 0.3],
            [-spec.width * 0.12, -spec.height * 0.42],
            [spec.width * 0.14, -spec.height * 0.3],
            [spec.width * 0.38, -spec.height * 0.08],
          ],
        },
      ]
    : [
        {
          path_id: `${spec.part_id}-diamond`,
          basis: 'nurbs-like' as const,
          dimension: '2d' as const,
          closed: true,
          points: [
            [-spec.width * 0.5, 0],
            [0, spec.height * 0.5],
            [spec.width * 0.5, 0],
            [0, -spec.height * 0.5],
          ],
        },
      ]
  const compiled = compileReliefCurveGraph({
    schema_version: 'ReliefCurveGraph@1',
    graph_id: `${spec.part_id}-curve-graph`,
    part_id: spec.part_id,
    material_zone_id: materialZoneId ?? 'unbound-relief',
    local_frame: frame,
    paths,
    width: Math.max(Math.min(spec.height * 0.16, spec.width * 0.08), MIN_DIMENSION * 4),
    depth: spec.depth,
    taper: spec.shape === 'panel' ? -0.18 : 0,
    profile: 'bevel',
  }, {
    samples_per_path: 32,
    round_radial_segments: 8,
    max_triangles: 65_536,
  })
  compiled.geometry.userData = {
    ...compiled.geometry.userData,
    relief_curve_graph_fingerprint: compiled.deterministic_fingerprint,
    primitive_fallback_used: false,
  }
  return compiled.geometry
}

function reliefFrame(axis: KnifeAssemblyAxis): ReliefLocalFrame {
  if (axis === 'x') {
    return { origin: [0, 0, 0], tangent: [0, 1, 0], lateral: [0, 0, 1], normal: [1, 0, 0] }
  }
  if (axis === 'y') {
    return { origin: [0, 0, 0], tangent: [1, 0, 0], lateral: [0, 0, 1], normal: [0, 1, 0] }
  }
  return { origin: [0, 0, 0], tangent: [1, 0, 0], lateral: [0, 1, 0], normal: [0, 0, 1] }
}

function orientNormal(geometry: THREE.BufferGeometry, axis: KnifeAssemblyAxis): void {
  if (axis === 'x') geometry.rotateY(Math.PI / 2)
  if (axis === 'y') geometry.rotateX(-Math.PI / 2)
}

function descriptorFor(spec: KnifeAssemblyPrimitiveSpec): KnifeAssemblyDescriptor {
  switch (spec.primitive) {
    case 'guard':
      return spec.style === 'dragon-guard'
        ? {
            style: spec.style,
            span: spec.span,
            thickness: spec.thickness,
            depth: spec.depth,
            jaw_gap: spec.jaw_gap,
            horn_count: spec.horns.length,
            eye_socket_count: spec.eye_sockets.length,
            horn_feature_ids: spec.horns.map((horn) => horn.feature_id).join(','),
            eye_socket_feature_ids: spec.eye_sockets.map((eye) => eye.feature_id).join(','),
            horn_sides: spec.horns.map((horn) => horn.side).join(','),
            eye_socket_sides: spec.eye_sockets.map((eye) => eye.side).join(','),
          }
        : { style: spec.style ?? 'classic', span: spec.span, thickness: spec.thickness, depth: spec.depth }
    case 'grip':
      return spec.style === 'segmented-grip'
        ? {
            style: spec.style,
            length: spec.length,
            radius: spec.radius,
            taper: spec.taper,
            facets: spec.facets,
            centerline_points: spec.centerline.length,
            segment_count: spec.segments.length,
            metal_frame_count: spec.metal_frames.length,
            fastener_count: spec.fasteners.length,
            segment_feature_ids: spec.segments.map((segment) => segment.feature_id).join(','),
            metal_frame_feature_ids: spec.metal_frames.map((frame) => frame.feature_id).join(','),
            fastener_feature_ids: spec.fasteners.map((fastener) => fastener.feature_id).join(','),
          }
        : { style: spec.style ?? 'classic', length: spec.length, radius: spec.radius, taper: spec.taper, facets: spec.facets }
    case 'pommel':
      return spec.style === 'hooked-pommel'
        ? {
            style: spec.style,
            length: spec.length,
            radius: spec.radius,
            depth: spec.depth,
            hook_length: spec.hook.length,
            hook_bend: spec.hook.bend,
            gem_seat_radius: spec.gem_seat.radius,
            gem_seat_feature_id: spec.gem_seat.feature_id,
          }
        : { style: spec.style ?? 'classic', length: spec.length, radius: spec.radius, depth: spec.depth }
    case 'fastener':
      return { radius: spec.radius, depth: spec.depth, axis: spec.axis }
    case 'gem':
      return { radius: spec.radius, depth: spec.depth, axis: spec.axis }
    case 'relief':
      return { width: spec.width, height: spec.height, depth: spec.depth, shape: spec.shape, axis: spec.axis }
  }
}

function tubeGeometry(
  points: readonly THREE.Vector3[],
  radius: number,
  tubularSegments: number,
  radialSegments: number,
): THREE.BufferGeometry {
  return new THREE.TubeGeometry(catmullRom(points, tubularSegments * 2), tubularSegments, Math.max(radius, MIN_DIMENSION), radialSegments, false)
}

function catmullRom(points: readonly THREE.Vector3[], arcLengthDivisions: number): THREE.CatmullRomCurve3 {
  const curve = new THREE.CatmullRomCurve3([...points], false, 'centripetal')
  curve.arcLengthDivisions = arcLengthDivisions
  return curve
}

function mergeGeometries(geometries: readonly THREE.BufferGeometry[]): THREE.BufferGeometry {
  const positions: number[] = []
  const normals: number[] = []
  const uvs: number[] = []
  const indices: number[] = []
  let vertexOffset = 0
  for (const geometry of geometries) {
    const source = geometry.index ? geometry.toNonIndexed() : geometry
    const position = source.getAttribute('position')
    const normal = source.getAttribute('normal')
    const uv = source.getAttribute('uv')
    for (let index = 0; index < position.count; index += 1) {
      positions.push(position.getX(index), position.getY(index), position.getZ(index))
      if (normal) normals.push(normal.getX(index), normal.getY(index), normal.getZ(index))
      if (uv) uvs.push(uv.getX(index), uv.getY(index))
      indices.push(vertexOffset + index)
    }
    vertexOffset += position.count
    if (source !== geometry) source.dispose()
  }
  const merged = new THREE.BufferGeometry()
  merged.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3))
  if (normals.length === positions.length) merged.setAttribute('normal', new THREE.Float32BufferAttribute(normals, 3))
  if (uvs.length === (positions.length / 3) * 2) merged.setAttribute('uv', new THREE.Float32BufferAttribute(uvs, 2))
  merged.setIndex(new THREE.Uint32BufferAttribute(indices, 1))
  if (!merged.getAttribute('normal')) merged.computeVertexNormals()
  merged.computeBoundingBox()
  merged.computeBoundingSphere()
  return merged
}

function stableCompare(left: string, right: string): number {
  if (left < right) return -1
  if (left > right) return 1
  return 0
}
