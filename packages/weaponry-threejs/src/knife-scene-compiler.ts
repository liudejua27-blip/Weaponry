import * as THREE from 'three'

import {
  compileKnifeAssemblyGeometry,
  type CompiledAssemblyGeometry,
  type KnifeAssemblyDescriptor,
} from './knife-assembly-compiler.ts'
import {
  compileImg2ThreeJsSourceEnvelope,
  type Img2ThreeJsCompatibilityPart,
} from './img2threejs-compatibility-compiler.ts'
import {
  KNIFE_MATERIAL_ATTRIBUTE_NAMES,
  KNIFE_MATERIAL_VERTEX_COLOR_ATTRIBUTE,
  bindKnifeLayeredMaterialGeometry,
  createKnifeLayeredMaterial,
  resolveKnifeLayeredMaterialSpec,
  validateKnifeLayeredMaterialSpecSet,
  type KnifeLayeredMaterialSpec,
} from './knife-material.ts'
import {
  LAYERED_SURFACE_FIELD_ATTRIBUTE_NAMES,
  bindLayeredSurfaceField,
} from './knife-surface-field.ts'
import type {
  KnifeAssembly,
  KnifeAssemblyAxis,
  KnifeAssemblyPrimitiveSpec,
  KnifeCurve,
  KnifeMaterialZone,
  KnifePart,
  KnifePartRole,
  KnifeSceneProgram,
  KnifeSection,
  KnifeVec3,
} from './knife-scene-program.ts'

const EPSILON = 1e-6
const MIN_THICKNESS = 1e-4
const MIN_CURVE_CONTROL_POINTS = 4
// Keep the compiler aligned with the closed KnifeSceneProgram@1 schema. The
// successor smoke intentionally exercises the smaller 6-12 window, while the
// contract still permits its bounded 4-64 curve range.
const MAX_CURVE_CONTROL_POINTS = 64
const MIN_BLADE_SECTIONS = 4
const MAX_BLADE_SECTIONS = 32
const STABLE_ID_PATTERN = /^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/

type DragonGuardSpec = Extract<KnifeAssemblyPrimitiveSpec, { primitive: 'guard'; style: 'dragon-guard' }>
type SegmentedGripSpec = Extract<KnifeAssemblyPrimitiveSpec, { primitive: 'grip'; style: 'segmented-grip' }>
type HookedPommelSpec = Extract<KnifeAssemblyPrimitiveSpec, { primitive: 'pommel'; style: 'hooked-pommel' }>

export type KnifeCompileErrorCode =
  | 'INVALID_PROGRAM'
  | 'INVALID_CURVE'
  | 'INVALID_SECTION_ORDER'
  | 'INVALID_PART_BINDING'
  | 'BUDGET_EXCEEDED'

export type KnifeRenderablePartRole = Exclude<KnifePartRole, 'helper'>

export class KnifeSceneCompileError extends Error {
  readonly code: KnifeCompileErrorCode

  constructor(code: KnifeCompileErrorCode, message: string) {
    super(`${code}: ${message}`)
    this.name = 'KnifeSceneCompileError'
    this.code = code
  }
}

export interface KnifeSceneCompileOptions {
  /** Bounded samples between root and tip. Four stations calibrate the field;
   * they are not the final render tessellation. */
  readonly longitudinal_segments?: number
  /** Optional closed first-party material vocabulary overrides by zone. */
  readonly material_specs?: readonly KnifeLayeredMaterialSpec[]
}

export interface CompiledSectionRecord {
  readonly section_id: string
  readonly role: KnifeSection['role']
  readonly u: number
  readonly center: KnifeVec3
  readonly edge_radius: number
  readonly spine_radius: number
  readonly top_thickness: number
  readonly bottom_thickness: number
  readonly edge_band: number
  readonly twist: number
}

export interface CompiledKnifePart {
  readonly part_id: string
  readonly material_zone_id: string
  readonly material_spec: KnifeLayeredMaterialSpec
  readonly surface_role: KnifeRenderablePartRole
  readonly assembly_primitive?: KnifeAssemblyPrimitiveSpec['primitive']
  readonly assembly_descriptor?: KnifeAssemblyDescriptor
  readonly center?: KnifeVec3
  readonly mesh: THREE.Mesh<THREE.BufferGeometry, THREE.MeshPhysicalMaterial>
  readonly geometry: THREE.BufferGeometry
  readonly material: THREE.MeshPhysicalMaterial
}

export interface CompiledKnifeScene {
  readonly group: THREE.Group
  readonly parts: readonly CompiledKnifePart[]
  readonly assembly_parts: readonly CompiledKnifePart[]
  readonly assembly_status: 'NOT_PRESENT' | 'COMPILED'
  readonly sections: readonly CompiledSectionRecord[]
  readonly triangle_count: number
  readonly longitudinal_segments: number
  /** A browser-safe deterministic fingerprint; it is not a Runtime/CAS SHA-256. */
  readonly deterministic_fingerprint: string
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
}

interface Point3 {
  readonly x: number
  readonly y: number
  readonly z: number
}

interface SectionFrame {
  readonly section: KnifeSection
  readonly center: THREE.Vector3
  readonly width_axis: THREE.Vector3
  readonly depth_axis: THREE.Vector3
  readonly edge_anchor: THREE.Vector3
  readonly inner_edge_anchor: THREE.Vector3
  readonly spine_anchor: THREE.Vector3
  readonly top_thickness: number
  readonly bottom_thickness: number
  readonly edge_band: number
}

interface MeshPayload {
  readonly positions: number[]
  readonly uvs: number[]
  readonly indices: number[]
  readonly section_indices: number[]
  readonly section_us: number[]
}

export function compileKnifeScene(
  program: KnifeSceneProgram,
  options: KnifeSceneCompileOptions = {},
): CompiledKnifeScene {
  validateProgram(program)
  validateKnifeLayeredMaterialSpecSet(options.material_specs, program.material_zones.map((zone) => zone.material_zone_id))

  const longitudinalSegments = boundedLongitudinalSegments(options.longitudinal_segments)
  if (program.source_envelope) return compileSourceEnvelopeScene(program, options, longitudinalSegments)
  const calibrationFrames = program.blade_surface.sections.map((section) => buildSectionFrame(program, section))
  const sectionFrames = Array.from({ length: longitudinalSegments + 1 }, (_, index) => {
    const u = index / longitudinalSegments
    return buildSectionFrame(program, interpolateSection(program.blade_surface.sections, u, index))
  })
  const sections = calibrationFrames.map(toSectionRecord)

  const bodyPart = requiredPart(program.parts, 'blade-body')
  const edgePart = requiredPart(program.parts, 'cutting-edge')
  const bodyZone = requiredMaterialZone(program.material_zones, bodyPart)
  const edgeZone = requiredMaterialZone(program.material_zones, edgePart)

  const calibrationSectionIds = program.blade_surface.sections.map((section) => section.section_id)
  const bodyGeometry = buildLoftGeometry(sectionFrames, 'body', calibrationSectionIds)
  const edgeGeometry = buildLoftGeometry(sectionFrames, 'edge', calibrationSectionIds)

  const bodyMaterialSpec = resolveKnifeLayeredMaterialSpec(bodyZone, bodyPart.part_id, options.material_specs)
  const edgeMaterialSpec = resolveKnifeLayeredMaterialSpec(edgeZone, edgePart.part_id, options.material_specs)
  const bodyMaterial = buildMaterial(bodyZone, program.asset_id, bodyPart.part_id, bodyMaterialSpec)
  const edgeMaterial = buildMaterial(edgeZone, program.asset_id, edgePart.part_id, edgeMaterialSpec)
  const bodyMesh = buildMesh(bodyGeometry, bodyMaterial, bodyMaterialSpec, program, bodyPart, 'blade-body')
  const edgeMesh = buildMesh(edgeGeometry, edgeMaterial, edgeMaterialSpec, program, edgePart, 'cutting-edge')

  const bodyCompiled: CompiledKnifePart = {
    part_id: bodyPart.part_id,
    material_zone_id: bodyZone.material_zone_id,
    material_spec: bodyMaterialSpec,
    surface_role: 'blade-body',
    mesh: bodyMesh,
    geometry: bodyGeometry,
    material: bodyMaterial,
  }
  const edgeCompiled: CompiledKnifePart = {
    part_id: edgePart.part_id,
    material_zone_id: edgeZone.material_zone_id,
    material_spec: edgeMaterialSpec,
    surface_role: 'cutting-edge',
    mesh: edgeMesh,
    geometry: edgeGeometry,
    material: edgeMaterial,
  }
  const materialZoneByPart = Object.fromEntries(program.parts.map((part) => [part.part_id, part.material_zone_id]))
  const assemblyParts = compileAssemblyParts(
    program,
    compileKnifeAssemblyGeometry(program.assembly, materialZoneByPart),
    options.material_specs,
  )
  const parts: readonly CompiledKnifePart[] = [bodyCompiled, edgeCompiled, ...assemblyParts]

  const group = new THREE.Group()
  group.name = `knife-scene:${program.asset_id}`
  overrideUuid(group, stableUuid(`group:${program.asset_id}`))
  group.userData = {
    schema_version: 'KnifeSceneProgram@1',
    compiler: 'weaponry-threejs-knife-compiler@1',
    asset_id: program.asset_id,
    source_curve_ids: [program.blade_surface.spine_curve.curve_id, program.blade_surface.cutting_edge_curve.curve_id],
    section_ids: sections.map((section) => section.section_id),
    part_ids: [...program.parts].map((part) => part.part_id).sort(),
    material_zone_ids: [...program.material_zones].map((zone) => zone.material_zone_id).sort(),
    compiled_part_ids: parts.map((part) => part.part_id),
    assembly_part_ids: assemblyParts.map((part) => part.part_id),
    assembly_status: assemblyParts.length > 0 ? 'COMPILED' : 'NOT_PRESENT',
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  }
  group.add(...parts.map((part) => part.mesh))

  const triangleCount = parts.reduce((count, part) => count + triangleCountFor(part.geometry), 0)
  if (triangleCount > program.budgets.max_triangles) {
    throw new KnifeSceneCompileError(
      'BUDGET_EXCEEDED',
      `scene compile emits ${triangleCount} triangles, above max_triangles ${program.budgets.max_triangles}`,
    )
  }
  if (parts.length > program.budgets.max_draw_calls) {
    throw new KnifeSceneCompileError(
      'BUDGET_EXCEEDED',
      `scene compile emits ${parts.length} draw calls, above max_draw_calls ${program.budgets.max_draw_calls}`,
    )
  }

  return {
    group,
    parts,
    assembly_parts: assemblyParts,
    assembly_status: assemblyParts.length > 0 ? 'COMPILED' : 'NOT_PRESENT',
    sections,
    triangle_count: triangleCount,
    longitudinal_segments: longitudinalSegments,
    deterministic_fingerprint: fingerprint(program, parts, sections),
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  }
}

/** Explicit name for callers that treat the input as a canonical scene program. */
export const compileKnifeSceneProgram = compileKnifeScene

/** Compatibility alias retained for callers that use the historical blade name. */
export const compileKnifeBlade = compileKnifeScene

/**
 * Compile the pinned source envelope without routing its primitives through the
 * semantic guard/grip/pommel approximations.  The canonical blade curves and
 * semantic assembly remain validated above, while this path supplies the
 * source-faithful mesh geometry and stable source transform binding.
 */
function compileSourceEnvelopeScene(
  program: KnifeSceneProgram,
  options: KnifeSceneCompileOptions,
  longitudinalSegments: number,
): CompiledKnifeScene {
  const compatibility = compileImg2ThreeJsSourceEnvelope(program.source_envelope!)
  const calibrationFrames = program.blade_surface.sections.map((section) => buildSectionFrame(program, section))
  const sections = calibrationFrames.map(toSectionRecord)
  const compiledParts = compatibility.parts.map((sourcePart) => compileSourcePart(program, sourcePart, options.material_specs))
  const assemblyParts = compiledParts.filter((part) => part.assembly_primitive !== undefined)
  const parts: readonly CompiledKnifePart[] = compiledParts

  const group = new THREE.Group()
  group.name = `knife-scene:${program.asset_id}`
  overrideUuid(group, stableUuid(`group:${program.asset_id}`))
  group.userData = {
    schema_version: 'KnifeSceneProgram@1',
    compiler: 'weaponry-threejs-knife-compiler@1',
    compatibility_compiler: 'img2threejs-source-envelope@1',
    asset_id: program.asset_id,
    source_curve_ids: [program.blade_surface.spine_curve.curve_id, program.blade_surface.cutting_edge_curve.curve_id],
    section_ids: sections.map((section) => section.section_id),
    part_ids: [...program.parts].map((part) => part.part_id).sort(),
    material_zone_ids: [...program.material_zones].map((zone) => zone.material_zone_id).sort(),
    compiled_part_ids: parts.map((part) => part.part_id),
    assembly_part_ids: assemblyParts.map((part) => part.part_id),
    assembly_status: assemblyParts.length > 0 ? 'COMPILED' : 'NOT_PRESENT',
    source_envelope_fingerprint: compatibility.deterministic_fingerprint,
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  }
  group.add(...parts.map((part) => part.mesh))

  const triangleCount = parts.reduce((count, part) => count + triangleCountFor(part.geometry), 0)
  if (triangleCount > program.budgets.max_triangles) {
    throw new KnifeSceneCompileError(
      'BUDGET_EXCEEDED',
      `scene compile emits ${triangleCount} triangles, above max_triangles ${program.budgets.max_triangles}`,
    )
  }
  if (parts.length > program.budgets.max_draw_calls) {
    throw new KnifeSceneCompileError(
      'BUDGET_EXCEEDED',
      `scene compile emits ${parts.length} draw calls, above max_draw_calls ${program.budgets.max_draw_calls}`,
    )
  }

  return {
    group,
    parts,
    assembly_parts: assemblyParts,
    assembly_status: assemblyParts.length > 0 ? 'COMPILED' : 'NOT_PRESENT',
    sections,
    triangle_count: triangleCount,
    longitudinal_segments: longitudinalSegments,
    deterministic_fingerprint: fingerprint(program, parts, sections),
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  }
}

function compileSourcePart(
  program: KnifeSceneProgram,
  sourcePart: Img2ThreeJsCompatibilityPart,
  materialSpecs?: readonly KnifeLayeredMaterialSpec[],
): CompiledKnifePart {
  const part = partById(program.parts, sourcePart.part_id)
  const surfaceRole = sourcePart.surface_role
  const zone = requiredMaterialZone(program.material_zones, part)
  const materialSpec = resolveKnifeLayeredMaterialSpec(zone, part.part_id, materialSpecs)
  const material = buildMaterial(zone, program.asset_id, part.part_id, materialSpec)
  const mesh = buildMesh(
    sourcePart.geometry,
    material,
    materialSpec,
    program,
    part,
    surfaceRole,
    sourcePart.assembly_primitive,
    sourcePart.center,
    sourcePart.descriptor,
  )
  mesh.position.set(sourcePart.center[0], sourcePart.center[1], sourcePart.center[2])
  return {
    part_id: part.part_id,
    material_zone_id: zone.material_zone_id,
    material_spec: materialSpec,
    surface_role: surfaceRole,
    assembly_primitive: sourcePart.assembly_primitive,
    assembly_descriptor: sourcePart.descriptor,
    center: sourcePart.center,
    mesh,
    geometry: sourcePart.geometry,
    material,
  }
}

function validateProgram(program: KnifeSceneProgram): void {
  if (!program || typeof program !== 'object' || !program.blade_surface || !Array.isArray(program.blade_surface.sections)) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'program must be an object')
  }
  if (program.schema_version !== 'KnifeSceneProgram@1') {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'schema_version must be KnifeSceneProgram@1')
  }
  if (program.coordinate_convention !== 'weapon-front-z-up-right-handed@1') {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'unsupported coordinate convention')
  }
  if (!Array.isArray(program.parts) || !Array.isArray(program.material_zones)) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'parts and material_zones must be arrays')
  }
  if (program.blade_surface.sections.length < MIN_BLADE_SECTIONS || program.blade_surface.sections.length > MAX_BLADE_SECTIONS) {
    throw new KnifeSceneCompileError(
      'INVALID_PROGRAM',
      `blade_surface.sections must contain ${MIN_BLADE_SECTIONS} to ${MAX_BLADE_SECTIONS} sections`,
    )
  }
  if (!Array.isArray(program.blade_surface.surface_roles)
    || program.blade_surface.surface_roles.includes('blade-body') === false
    || program.blade_surface.surface_roles.includes('cutting-edge') === false) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'blade surface roles must include blade-body and cutting-edge')
  }
  validateCurve(program.blade_surface.spine_curve, 'spine_curve')
  validateCurve(program.blade_surface.cutting_edge_curve, 'cutting_edge_curve')
  if (program.blade_surface.spine_curve.curve_id === program.blade_surface.cutting_edge_curve.curve_id) {
    throw new KnifeSceneCompileError('INVALID_CURVE', 'spine and cutting-edge curves need distinct stable IDs')
  }

  let previousU = -1
  const sectionIds = new Set<string>()
  for (const section of program.blade_surface.sections) {
    if (!section || typeof section !== 'object' || !STABLE_ID_PATTERN.test(section.section_id)) {
      throw new KnifeSceneCompileError('INVALID_SECTION_ORDER', 'section IDs must be bounded stable IDs')
    }
    if (sectionIds.has(section.section_id)) {
      throw new KnifeSceneCompileError('INVALID_SECTION_ORDER', `duplicate section ID ${section.section_id}`)
    }
    sectionIds.add(section.section_id)
    if (!['root', 'shoulder', 'belly', 'tip', 'intermediate'].includes(section.role)) {
      throw new KnifeSceneCompileError('INVALID_SECTION_ORDER', `section ${section.section_id} has an unsupported role`)
    }
    if (!Number.isFinite(section.u) || section.u <= previousU || section.u < 0 || section.u > 1) {
      throw new KnifeSceneCompileError('INVALID_SECTION_ORDER', 'section u values must be finite, strict, and monotonic in [0, 1]')
    }
    previousU = section.u
    for (const [name, value] of Object.entries(section)) {
      if (name !== 'section_id' && name !== 'role' && !Number.isFinite(value)) {
        throw new KnifeSceneCompileError('INVALID_PROGRAM', `section ${section.section_id} has non-finite ${name}`)
      }
    }
    if (section.half_width <= 0 || section.thickness <= 0) {
      throw new KnifeSceneCompileError('INVALID_PROGRAM', `section ${section.section_id} must have positive width and thickness`)
    }
  }
  const firstSection = program.blade_surface.sections[0]
  const lastSection = program.blade_surface.sections[program.blade_surface.sections.length - 1]
  if (firstSection.u !== 0 || firstSection.role !== 'root' || lastSection.u !== 1 || lastSection.role !== 'tip') {
    throw new KnifeSceneCompileError(
      'INVALID_SECTION_ORDER',
      'calibration sections must start with root at u=0 and end with tip at u=1',
    )
  }

  const parts = uniqueById(program.parts, (part) => part.part_id, 'INVALID_PART_BINDING')
  const zones = uniqueById(program.material_zones, (zone) => zone.material_zone_id, 'INVALID_PART_BINDING')
  const zoneIds = new Set(zones.map((zone) => zone.material_zone_id))
  for (const part of parts) {
    if (!zoneIds.has(part.material_zone_id)) {
      throw new KnifeSceneCompileError(
        'INVALID_PART_BINDING',
        `part ${part.part_id} references missing material zone ${part.material_zone_id}`,
      )
    }
  }
  validateAssembly(program.assembly, parts, zoneIds)
  requiredPart(parts, 'blade-body')
  requiredPart(parts, 'cutting-edge')
  for (const zone of zones) {
    if (!/^#[0-9a-f]{6}$/i.test(zone.base_color)) {
      throw new KnifeSceneCompileError('INVALID_PROGRAM', `material zone ${zone.material_zone_id} has invalid base_color`)
    }
    if (!Number.isFinite(zone.metalness) || !Number.isFinite(zone.roughness) || zone.metalness < 0 || zone.metalness > 1 || zone.roughness < 0 || zone.roughness > 1) {
      throw new KnifeSceneCompileError('INVALID_PROGRAM', `material zone ${zone.material_zone_id} has invalid PBR scalar`)
    }
  }
}

function validateCurve(curve: KnifeCurve, name: string): void {
  if (!curve || typeof curve !== 'object' || !Array.isArray(curve.control_points)
    || curve.control_points.length < MIN_CURVE_CONTROL_POINTS
    || curve.control_points.length > MAX_CURVE_CONTROL_POINTS) {
    throw new KnifeSceneCompileError(
      'INVALID_CURVE',
      `${name} requires ${MIN_CURVE_CONTROL_POINTS} to ${MAX_CURVE_CONTROL_POINTS} control points`,
    )
  }
  if (typeof curve.curve_id !== 'string' || !STABLE_ID_PATTERN.test(curve.curve_id)) {
    throw new KnifeSceneCompileError('INVALID_CURVE', `${name} curve ID must be a bounded stable ID`)
  }
  if (curve.basis !== 'bezier' && curve.basis !== 'nurbs-like') {
    throw new KnifeSceneCompileError('INVALID_CURVE', `${name} has an unsupported curve basis`)
  }
  for (const point of curve.control_points) {
    if (!Array.isArray(point) || point.length !== 3 || point.some((coordinate: number) => !Number.isFinite(coordinate))) {
      throw new KnifeSceneCompileError('INVALID_CURVE', `${name} has a non-finite control point`)
    }
  }
}

function uniqueById<T>(items: readonly T[], getId: (item: T) => string, code: KnifeCompileErrorCode): T[] {
  const seen = new Set<unknown>()
  for (const item of items) {
    const id = getId(item)
    if (typeof id !== 'string' || !STABLE_ID_PATTERN.test(id) || seen.has(id)) {
      throw new KnifeSceneCompileError(code, 'duplicate or invalid stable ID')
    }
    seen.add(id)
  }
  return [...items]
}

function requiredPart(parts: readonly KnifePart[], role: KnifePart['role']): KnifePart {
  const part = parts.find((candidate) => candidate.role === role)
  if (!part) {
    throw new KnifeSceneCompileError('INVALID_PART_BINDING', `program must bind a ${role} part`)
  }
  return part
}

function requiredMaterialZone(zones: readonly KnifeMaterialZone[], part: KnifePart): KnifeMaterialZone {
  const zone = zones.find((candidate) => candidate.material_zone_id === part.material_zone_id)
  if (!zone) {
    throw new KnifeSceneCompileError('INVALID_PART_BINDING', `missing material zone ${part.material_zone_id}`)
  }
  return zone
}

function compileAssemblyParts(
  program: KnifeSceneProgram,
  geometries: readonly CompiledAssemblyGeometry[],
  materialSpecs?: readonly KnifeLayeredMaterialSpec[],
): readonly CompiledKnifePart[] {
  return geometries.map((compiled) => {
    const part = partById(program.parts, compiled.part_id)
    const surfaceRole = renderablePartRole(part)
    if (surfaceRole !== compiled.primitive) {
      throw new KnifeSceneCompileError(
        'INVALID_PART_BINDING',
        `assembly primitive ${compiled.primitive} must bind a ${compiled.part_id} part with the same role`,
      )
    }
    const zone = requiredMaterialZone(program.material_zones, part)
    const materialSpec = resolveKnifeLayeredMaterialSpec(zone, part.part_id, materialSpecs)
    const material = buildMaterial(zone, program.asset_id, part.part_id, materialSpec)
    const mesh = buildMesh(
      compiled.geometry,
      material,
      materialSpec,
      program,
      part,
      surfaceRole,
      compiled.primitive,
      compiled.center,
      compiled.descriptor,
    )
    mesh.position.set(compiled.center[0], compiled.center[1], compiled.center[2])
    return {
      part_id: part.part_id,
      material_zone_id: zone.material_zone_id,
      material_spec: materialSpec,
      surface_role: surfaceRole,
      assembly_primitive: compiled.primitive,
      assembly_descriptor: compiled.descriptor,
      center: compiled.center,
      mesh,
      geometry: compiled.geometry,
      material,
    }
  })
}

function validateAssembly(
  assembly: KnifeAssembly | undefined,
  parts: readonly KnifePart[],
  zoneIds: ReadonlySet<string>,
): void {
  if (assembly === undefined) return
  if (!assembly || typeof assembly !== 'object' || Array.isArray(assembly)) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'assembly must be an object')
  }

  const specs: KnifeAssemblyPrimitiveSpec[] = []
  appendAssemblySingle(specs, assembly.guard, 'guard')
  appendAssemblySingle(specs, assembly.grip, 'grip')
  appendAssemblySingle(specs, assembly.pommel, 'pommel')
  appendAssemblyList(specs, assembly.fasteners, 'fasteners', 'fastener')
  appendAssemblyList(specs, assembly.gems, 'gems', 'gem')
  appendAssemblyList(specs, assembly.reliefs, 'reliefs', 'relief')

  const primitivePartIds = new Set<string>()
  for (const spec of specs) {
    validateAssemblySpec(spec)
    if (primitivePartIds.has(spec.part_id)) {
      throw new KnifeSceneCompileError('INVALID_PART_BINDING', `assembly part ${spec.part_id} is declared more than once`)
    }
    primitivePartIds.add(spec.part_id)
    const part = partById(parts, spec.part_id)
    if (part.role !== spec.primitive) {
      throw new KnifeSceneCompileError(
        'INVALID_PART_BINDING',
        `assembly primitive ${spec.primitive} requires part ${spec.part_id} to have role ${spec.primitive}`,
      )
    }
    if (!zoneIds.has(part.material_zone_id)) {
      throw new KnifeSceneCompileError(
        'INVALID_PART_BINDING',
        `assembly part ${spec.part_id} references missing material zone ${part.material_zone_id}`,
      )
    }
  }
}

function appendAssemblySingle(
  specs: KnifeAssemblyPrimitiveSpec[],
  value: unknown,
  expectedPrimitive: KnifeAssemblyPrimitiveSpec['primitive'],
): void {
  if (value === undefined) return
  const spec = value as KnifeAssemblyPrimitiveSpec
  if (!spec || typeof spec !== 'object' || Array.isArray(spec) || spec.primitive !== expectedPrimitive) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', `assembly.${expectedPrimitive} must be one ${expectedPrimitive} primitive`)
  }
  specs.push(spec)
}

function appendAssemblyList(
  specs: KnifeAssemblyPrimitiveSpec[],
  value: unknown,
  label: string,
  expectedPrimitive: KnifeAssemblyPrimitiveSpec['primitive'],
): void {
  if (value === undefined) return
  if (!Array.isArray(value) || value.length > 32) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', `assembly.${label} must be an array with at most 32 entries`)
  }
  for (const item of value) {
    const spec = item as KnifeAssemblyPrimitiveSpec
    if (!spec || typeof spec !== 'object' || Array.isArray(spec) || spec.primitive !== expectedPrimitive) {
      throw new KnifeSceneCompileError('INVALID_PROGRAM', `assembly.${label} contains a non-${expectedPrimitive} primitive`)
    }
    specs.push(spec)
  }
}

function validateAssemblySpec(spec: KnifeAssemblyPrimitiveSpec): void {
  if (typeof spec.part_id !== 'string' || !/^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/.test(spec.part_id)) {
    throw new KnifeSceneCompileError('INVALID_PART_BINDING', 'assembly part_id must be a bounded stable ID')
  }
  if (!Array.isArray(spec.center) || spec.center.length !== 3 || spec.center.some((value) => !Number.isFinite(value) || Math.abs(value) > 4)) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', `assembly ${spec.part_id} center must be finite and bounded`)
  }
  switch (spec.primitive) {
    case 'guard':
      positiveDimension(spec.span, 'guard.span', 2)
      positiveDimension(spec.thickness, 'guard.thickness', 1)
      positiveDimension(spec.depth, 'guard.depth', 1)
      if (spec.style !== undefined && spec.style !== 'classic' && spec.style !== 'dragon-guard') {
        throw new KnifeSceneCompileError('INVALID_PROGRAM', 'guard.style must be classic or dragon-guard')
      }
      if (spec.style === 'dragon-guard') validateDragonGuardSpec(spec)
      return
    case 'grip':
      positiveDimension(spec.length, 'grip.length', 2)
      positiveDimension(spec.radius, 'grip.radius', 1)
      boundedNumber(spec.taper, 'grip.taper', -0.9, 0.9)
      if (!Number.isInteger(spec.facets) || spec.facets < 6 || spec.facets > 32) {
        throw new KnifeSceneCompileError('INVALID_PROGRAM', 'grip.facets must be an integer in [6, 32]')
      }
      if (spec.style !== undefined && spec.style !== 'classic' && spec.style !== 'segmented-grip') {
        throw new KnifeSceneCompileError('INVALID_PROGRAM', 'grip.style must be classic or segmented-grip')
      }
      if (spec.style === 'segmented-grip') validateSegmentedGripSpec(spec)
      return
    case 'pommel':
      positiveDimension(spec.length, 'pommel.length', 1)
      positiveDimension(spec.radius, 'pommel.radius', 1)
      positiveDimension(spec.depth, 'pommel.depth', 1)
      if (spec.style !== undefined && spec.style !== 'classic' && spec.style !== 'hooked-pommel') {
        throw new KnifeSceneCompileError('INVALID_PROGRAM', 'pommel.style must be classic or hooked-pommel')
      }
      if (spec.style === 'hooked-pommel') validateHookedPommelSpec(spec)
      return
    case 'fastener':
      positiveDimension(spec.radius, 'fastener.radius', 0.5)
      positiveDimension(spec.depth, 'fastener.depth', 1)
      validateAxis(spec.axis, 'fastener.axis')
      return
    case 'gem':
      positiveDimension(spec.radius, 'gem.radius', 0.5)
      positiveDimension(spec.depth, 'gem.depth', 1)
      validateAxis(spec.axis, 'gem.axis')
      return
    case 'relief':
      positiveDimension(spec.width, 'relief.width', 1)
      positiveDimension(spec.height, 'relief.height', 1)
      positiveDimension(spec.depth, 'relief.depth', 0.5)
      if (spec.shape !== 'panel' && spec.shape !== 'diamond') {
        throw new KnifeSceneCompileError('INVALID_PROGRAM', 'relief.shape must be panel or diamond')
      }
      validateAxis(spec.axis, 'relief.axis')
      return
  }
}

function validateDragonGuardSpec(spec: DragonGuardSpec): void {
  boundedNumber(spec.jaw_gap, 'dragon-guard.jaw_gap', 0.01, 0.6)
  validateDragonJaw(spec.upper_jaw, 'dragon-guard.upper_jaw', spec.span)
  validateDragonJaw(spec.lower_jaw, 'dragon-guard.lower_jaw', spec.span)
  const upperJawRadius = Math.min(spec.upper_jaw.thickness, spec.upper_jaw.depth) * 0.5
  const lowerJawRadius = Math.min(spec.lower_jaw.thickness, spec.lower_jaw.depth) * 0.5
  const negativeCurvature = Math.max(0, -spec.upper_jaw.curvature, -spec.lower_jaw.curvature)
  if (spec.jaw_gap - negativeCurvature * 2 <= upperJawRadius + lowerJawRadius) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'dragon-guard.jaw_gap must leave positive space between jaw rails')
  }

  if (!Array.isArray(spec.horns) || spec.horns.length < 2 || spec.horns.length > 4) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'dragon-guard.horns must contain 2 to 4 entries')
  }
  if (!Array.isArray(spec.eye_sockets) || spec.eye_sockets.length < 1 || spec.eye_sockets.length > 2) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'dragon-guard.eye_sockets must contain 1 to 2 entries')
  }

  const featureIds = new Set<string>()
  let positiveHornCount = 0
  let negativeHornCount = 0
  for (const horn of spec.horns) {
    validateFeatureId(horn.feature_id, 'dragon-guard.horn', featureIds)
    validateSide(horn.side, `dragon-guard.horn ${horn.feature_id}.side`)
    if (horn.side === 1) positiveHornCount += 1
    else negativeHornCount += 1
    positiveDimension(horn.length, `dragon-guard.horn ${horn.feature_id}.length`, 0.8)
    positiveDimension(horn.radius, `dragon-guard.horn ${horn.feature_id}.radius`, 0.2)
    boundedNumber(horn.sweep, `dragon-guard.horn ${horn.feature_id}.sweep`, -0.75, 0.75)
    boundedNumber(horn.offset_z, `dragon-guard.horn ${horn.feature_id}.offset_z`, -0.8, 0.8)
  }
  if (positiveHornCount === 0 || negativeHornCount === 0) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'dragon-guard horns must include both sides')
  }
  for (const eye of spec.eye_sockets) {
    validateFeatureId(eye.feature_id, 'dragon-guard.eye_socket', featureIds)
    validateSide(eye.side, `dragon-guard.eye_socket ${eye.feature_id}.side`)
    positiveDimension(eye.radius, `dragon-guard.eye_socket ${eye.feature_id}.radius`, 0.25)
    positiveDimension(eye.depth, `dragon-guard.eye_socket ${eye.feature_id}.depth`, 0.2)
    boundedNumber(eye.offset_y, `dragon-guard.eye_socket ${eye.feature_id}.offset_y`, -spec.span * 0.5, spec.span * 0.5)
    boundedNumber(eye.offset_z, `dragon-guard.eye_socket ${eye.feature_id}.offset_z`, -0.8, 0.8)
  }
}

function validateDragonJaw(
  jaw: DragonGuardSpec['upper_jaw'],
  name: string,
  guardSpan: number,
): void {
  if (!jaw || typeof jaw !== 'object' || Array.isArray(jaw)) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', `${name} must be an object`)
  }
  positiveDimension(jaw.span, `${name}.span`, guardSpan)
  positiveDimension(jaw.thickness, `${name}.thickness`, 0.4)
  positiveDimension(jaw.depth, `${name}.depth`, 0.4)
  boundedNumber(jaw.offset_y, `${name}.offset_y`, -guardSpan * 0.5, guardSpan * 0.5)
  boundedNumber(jaw.offset_z, `${name}.offset_z`, -0.8, 0.8)
  boundedNumber(jaw.curvature, `${name}.curvature`, -0.25, 0.25)
}

function validateSegmentedGripSpec(spec: SegmentedGripSpec): void {
  if (!Array.isArray(spec.centerline) || spec.centerline.length < 3 || spec.centerline.length > 8) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'segmented-grip.centerline must contain 3 to 8 points')
  }
  let previousX = Number.NEGATIVE_INFINITY
  for (const point of spec.centerline) {
    validateAssemblyPoint(point, 'segmented-grip.centerline')
    if (point[0] <= previousX) {
      throw new KnifeSceneCompileError('INVALID_PROGRAM', 'segmented-grip.centerline x values must be strict and increasing')
    }
    previousX = point[0]
  }

  if (!Array.isArray(spec.segments) || spec.segments.length < 2 || spec.segments.length > 8) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'segmented-grip.segments must contain 2 to 8 entries')
  }
  const featureIds = new Set<string>()
  let previousEnd = 0
  for (const [index, segment] of spec.segments.entries()) {
    validateFeatureId(segment.feature_id, `segmented-grip.segment[${index}]`, featureIds)
    boundedNumber(segment.start_u, `segmented-grip.segment ${segment.feature_id}.start_u`, 0, 1)
    boundedNumber(segment.end_u, `segmented-grip.segment ${segment.feature_id}.end_u`, 0, 1)
    if (Math.abs(segment.start_u - previousEnd) > EPSILON || segment.end_u <= segment.start_u) {
      throw new KnifeSceneCompileError('INVALID_PROGRAM', 'segmented-grip.segments must be contiguous and strictly increasing')
    }
    boundedNumber(segment.radius_scale, `segmented-grip.segment ${segment.feature_id}.radius_scale`, 0.5, 1.5)
    previousEnd = segment.end_u
  }
  if (Math.abs(previousEnd - 1) > EPSILON) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'segmented-grip.segments must cover [0, 1]')
  }

  if (!Array.isArray(spec.metal_frames) || spec.metal_frames.length < 1 || spec.metal_frames.length > 8) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'segmented-grip.metal_frames must contain 1 to 8 entries')
  }
  for (const [index, frame] of spec.metal_frames.entries()) {
    validateFeatureId(frame.feature_id, `segmented-grip.metal_frame[${index}]`, featureIds)
    boundedNumber(frame.at, `segmented-grip.metal_frame ${frame.feature_id}.at`, 0, 1)
    positiveDimension(frame.width, `segmented-grip.metal_frame ${frame.feature_id}.width`, 0.5)
    positiveDimension(frame.thickness, `segmented-grip.metal_frame ${frame.feature_id}.thickness`, 0.15)
    if (frame.width < frame.thickness * 2) {
      throw new KnifeSceneCompileError('INVALID_PROGRAM', `segmented-grip.metal_frame ${frame.feature_id}.width must be at least twice its thickness`)
    }
  }

  if (!Array.isArray(spec.fasteners) || spec.fasteners.length < 3 || spec.fasteners.length > 5) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'segmented-grip.fasteners must contain 3 to 5 entries')
  }
  for (const [index, fastener] of spec.fasteners.entries()) {
    validateFeatureId(fastener.feature_id, `segmented-grip.fastener[${index}]`, featureIds)
    boundedNumber(fastener.at, `segmented-grip.fastener ${fastener.feature_id}.at`, 0, 1)
    validateSide(fastener.side, `segmented-grip.fastener ${fastener.feature_id}.side`)
    positiveDimension(fastener.radius, `segmented-grip.fastener ${fastener.feature_id}.radius`, 0.1)
    positiveDimension(fastener.depth, `segmented-grip.fastener ${fastener.feature_id}.depth`, 0.25)
  }
}

function validateHookedPommelSpec(spec: HookedPommelSpec): void {
  if (!spec.hook || typeof spec.hook !== 'object' || Array.isArray(spec.hook)) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'hooked-pommel.hook must be an object')
  }
  positiveDimension(spec.hook.length, 'hooked-pommel.hook.length', 0.8)
  positiveDimension(spec.hook.radius, 'hooked-pommel.hook.radius', 0.2)
  boundedNumber(spec.hook.bend, 'hooked-pommel.hook.bend', 0.2, 1)
  validateSide(spec.hook.direction, 'hooked-pommel.hook.direction')

  const seat = spec.gem_seat
  if (!seat || typeof seat !== 'object' || Array.isArray(seat)) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'hooked-pommel.gem_seat must be an object')
  }
  const featureIds = new Set<string>()
  validateFeatureId(seat.feature_id, 'hooked-pommel.gem_seat', featureIds)
  positiveDimension(seat.radius, 'hooked-pommel.gem_seat.radius', 0.25)
  positiveDimension(seat.depth, 'hooked-pommel.gem_seat.depth', 0.2)
  boundedNumber(seat.offset_x, 'hooked-pommel.gem_seat.offset_x', -0.8, 0.8)
  boundedNumber(seat.offset_y, 'hooked-pommel.gem_seat.offset_y', -0.8, 0.8)
  boundedNumber(seat.offset_z, 'hooked-pommel.gem_seat.offset_z', -0.8, 0.8)
  validateAxis(seat.axis, 'hooked-pommel.gem_seat.axis')
}

function validateFeatureId(value: unknown, name: string, seen: Set<string>): void {
  if (typeof value !== 'string' || !STABLE_ID_PATTERN.test(value) || seen.has(value)) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', `${name}.feature_id must be unique bounded stable ID`)
  }
  seen.add(value)
}

function validateSide(value: unknown, name: string): asserts value is -1 | 1 {
  if (value !== -1 && value !== 1) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', `${name} must be -1 or 1`)
  }
}

function validateAssemblyPoint(value: unknown, name: string): asserts value is KnifeVec3 {
  if (!Array.isArray(value) || value.length !== 3 || value.some((coordinate) => typeof coordinate !== 'number' || !Number.isFinite(coordinate) || Math.abs(coordinate) > 4)) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', `${name} points must be finite and bounded`)
  }
}

function positiveDimension(value: number, name: string, maximum: number): void {
  if (!Number.isFinite(value) || value <= MIN_THICKNESS || value > maximum) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', `${name} must be finite and in (${MIN_THICKNESS}, ${maximum}]`)
  }
}

function boundedNumber(value: number, name: string, minimum: number, maximum: number): void {
  if (!Number.isFinite(value) || value < minimum || value > maximum) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', `${name} must be finite and in [${minimum}, ${maximum}]`)
  }
}

function validateAxis(value: KnifeAssemblyAxis, name: string): void {
  if (value !== 'x' && value !== 'y' && value !== 'z') {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', `${name} must be x, y, or z`)
  }
}

function partById(parts: readonly KnifePart[], partId: string): KnifePart {
  const part = parts.find((candidate) => candidate.part_id === partId)
  if (!part) throw new KnifeSceneCompileError('INVALID_PART_BINDING', `assembly references missing part ${partId}`)
  return part
}

function renderablePartRole(part: KnifePart): KnifeRenderablePartRole {
  if (part.role === 'helper') {
    throw new KnifeSceneCompileError('INVALID_PART_BINDING', `helper part ${part.part_id} cannot be a renderable assembly primitive`)
  }
  return part.role
}

function buildSectionFrame(program: KnifeSceneProgram, section: KnifeSection): SectionFrame {
  const spine = evaluateCurve(program.blade_surface.spine_curve, section.u)
  const edge = evaluateCurve(program.blade_surface.cutting_edge_curve, section.u)
  const spinePoint = new THREE.Vector3(spine.x, spine.y, spine.z)
  const edgePoint = new THREE.Vector3(edge.x, edge.y, edge.z)
  const center = spinePoint.clone().add(edgePoint).multiplyScalar(0.5)

  const spineTangent = evaluateCurveTangent(program.blade_surface.spine_curve, section.u)
  const edgeTangent = evaluateCurveTangent(program.blade_surface.cutting_edge_curve, section.u)
  const tangent = new THREE.Vector3(spineTangent.x + edgeTangent.x, spineTangent.y + edgeTangent.y, 0)
  if (tangent.lengthSq() <= EPSILON) tangent.set(1, 0, 0)
  tangent.normalize()

  const rawWidth = spinePoint.clone().sub(edgePoint)
  rawWidth.z = 0
  if (rawWidth.lengthSq() <= EPSILON) rawWidth.set(0, 1, 0)
  rawWidth.normalize()
  const widthAxis = rawWidth.clone().addScaledVector(tangent, -rawWidth.dot(tangent))
  if (widthAxis.lengthSq() <= EPSILON) widthAxis.set(-tangent.y, tangent.x, 0)
  widthAxis.normalize()
  const baseDepth = tangent.clone().cross(widthAxis).normalize()
  const widthAxisTwisted = rotateAroundAxis(widthAxis, tangent, section.twist)
  const depthAxis = rotateAroundAxis(baseDepth, tangent, section.twist)

  const edgeRadius = Math.max(center.distanceTo(edgePoint) - section.edge_offset, MIN_THICKNESS)
  const spineRadius = Math.max(center.distanceTo(spinePoint) + section.spine_offset, MIN_THICKNESS)
  const edgeBand = Math.max(Math.min(section.half_width * 0.18, 0.05), Math.min(section.half_width * 0.5, 0.0015))
  // The two curves are the actual silhouette rails. Section offsets refine
  // them locally; half_width remains a calibration prior and edge-band bound,
  // rather than replacing both rails with one global width scalar.
  const edgeAnchor = edgePoint.clone().addScaledVector(widthAxisTwisted, section.edge_offset)
  const innerEdgeAnchor = edgeAnchor.clone().addScaledVector(widthAxisTwisted, Math.min(edgeBand, edgeRadius * 0.5))
  const spineAnchor = spinePoint.clone().addScaledVector(widthAxisTwisted, section.spine_offset)
  const asymmetry = THREE.MathUtils.clamp(section.asymmetry, -0.95, 0.95)
  const topThickness = Math.max(section.thickness * (0.5 + asymmetry * 0.5), MIN_THICKNESS)
  const bottomThickness = Math.max(section.thickness * (0.5 - asymmetry * 0.5), MIN_THICKNESS)

  return {
    section,
    center,
    width_axis: widthAxisTwisted,
    depth_axis: depthAxis,
    edge_anchor: edgeAnchor,
    inner_edge_anchor: innerEdgeAnchor,
    spine_anchor: spineAnchor,
    top_thickness: topThickness,
    bottom_thickness: bottomThickness,
    edge_band: Math.min(edgeBand, edgeRadius * 0.5),
  }
}

function toSectionRecord(frame: SectionFrame): CompiledSectionRecord {
  return {
    section_id: frame.section.section_id,
    role: frame.section.role,
    u: frame.section.u,
    center: [frame.center.x, frame.center.y, frame.center.z],
    edge_radius: frame.center.distanceTo(frame.edge_anchor),
    spine_radius: frame.center.distanceTo(frame.spine_anchor),
    top_thickness: frame.top_thickness,
    bottom_thickness: frame.bottom_thickness,
    edge_band: frame.edge_band,
    twist: frame.section.twist,
  }
}

function buildLoftGeometry(
  frames: readonly SectionFrame[],
  kind: 'body' | 'edge',
  calibrationSectionIds: readonly string[],
): THREE.BufferGeometry {
  const payload: MeshPayload = {
    positions: [],
    uvs: [],
    indices: [],
    section_indices: [],
    section_us: [],
  }

  for (let sectionIndex = 0; sectionIndex < frames.length; sectionIndex += 1) {
    const frame = frames[sectionIndex]
    const ridge = frame.inner_edge_anchor.clone().lerp(frame.spine_anchor, 0.62)
    const ring = kind === 'body'
      ? [
          offsetAlongDepth(frame.spine_anchor, frame.depth_axis, frame.top_thickness * 0.72),
          offsetAlongDepth(ridge, frame.depth_axis, frame.top_thickness),
          offsetAlongDepth(frame.inner_edge_anchor, frame.depth_axis, frame.top_thickness * 0.28),
          offsetAlongDepth(frame.inner_edge_anchor, frame.depth_axis, -frame.bottom_thickness * 0.28),
          offsetAlongDepth(ridge, frame.depth_axis, -frame.bottom_thickness),
          offsetAlongDepth(frame.spine_anchor, frame.depth_axis, -frame.bottom_thickness * 0.72),
        ]
      : [
          frame.edge_anchor.clone(),
          offsetAlongDepth(frame.inner_edge_anchor, frame.depth_axis, frame.top_thickness * 0.28),
          offsetAlongDepth(frame.inner_edge_anchor, frame.depth_axis, -frame.bottom_thickness * 0.28),
        ]
    for (let ringIndex = 0; ringIndex < ring.length; ringIndex += 1) {
      const point = ring[ringIndex]
      payload.positions.push(point.x, point.y, point.z)
      payload.uvs.push(frame.section.u, ringIndex / (ring.length - 1))
      payload.section_indices.push(sectionIndex)
      payload.section_us.push(frame.section.u)
    }
  }

  const ringSize = kind === 'body' ? 6 : 3
  for (let sectionIndex = 0; sectionIndex < frames.length - 1; sectionIndex += 1) {
    const current = sectionIndex * ringSize
    const next = (sectionIndex + 1) * ringSize
    for (let ringIndex = 0; ringIndex < ringSize; ringIndex += 1) {
      const nextRingIndex = (ringIndex + 1) % ringSize
      const a = current + ringIndex
      const b = next + ringIndex
      const c = next + nextRingIndex
      const d = current + nextRingIndex
      payload.indices.push(a, b, c, a, c, d)
    }
  }

  // Closed root and tip caps keep the derived blade parts renderable even when
  // a tip section approaches a zero visual width. Thickness remains bounded.
  for (let index = 1; index < ringSize - 1; index += 1) {
    payload.indices.push(0, index + 1, index)
  }
  const tip = (frames.length - 1) * ringSize
  for (let index = 1; index < ringSize - 1; index += 1) {
    payload.indices.push(tip, tip + index, tip + index + 1)
  }

  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(payload.positions, 3))
  geometry.setAttribute('uv', new THREE.Float32BufferAttribute(payload.uvs, 2))
  geometry.setAttribute('sectionIndex', new THREE.Float32BufferAttribute(payload.section_indices, 1))
  geometry.setAttribute('sectionU', new THREE.Float32BufferAttribute(payload.section_us, 1))
  geometry.setIndex(new THREE.Uint32BufferAttribute(payload.indices, 1))
  geometry.computeVertexNormals()
  geometry.userData = {
    topology: kind === 'body' ? 'six-point-faceted-blade-loft@1' : 'three-point-cutting-edge-loft@1',
    sample_count: frames.length,
    calibration_section_ids: [...calibrationSectionIds],
    surface_role: kind === 'body' ? 'blade-body' : 'cutting-edge',
    source_curves: ['spine_curve', 'cutting_edge_curve'],
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  }
  return geometry
}

function boundedLongitudinalSegments(value: number | undefined): number {
  const segments = value ?? 64
  if (!Number.isInteger(segments) || segments < 16 || segments > 256) {
    throw new KnifeSceneCompileError('INVALID_PROGRAM', 'longitudinal_segments must be an integer in [16, 256]')
  }
  return segments
}

function interpolateSection(sections: readonly KnifeSection[], u: number, sampleIndex: number): KnifeSection {
  const exact = sections.find((section) => Math.abs(section.u - u) <= Number.EPSILON)
  if (exact) return exact
  let upperIndex = sections.findIndex((section) => section.u > u)
  if (upperIndex <= 0) upperIndex = 1
  const lower = sections[upperIndex - 1]
  const upper = sections[upperIndex]
  const span = upper.u - lower.u
  const t = span <= EPSILON ? 0 : (u - lower.u) / span
  const lerp = (a: number, b: number): number => THREE.MathUtils.lerp(a, b, t)
  return {
    section_id: `__sample-${sampleIndex.toString().padStart(3, '0')}`,
    role: 'intermediate',
    u,
    half_width: lerp(lower.half_width, upper.half_width),
    thickness: lerp(lower.thickness, upper.thickness),
    edge_offset: lerp(lower.edge_offset, upper.edge_offset),
    spine_offset: lerp(lower.spine_offset, upper.spine_offset),
    asymmetry: lerp(lower.asymmetry, upper.asymmetry),
    twist: lerp(lower.twist, upper.twist),
  }
}

function offsetAlongDepth(anchor: THREE.Vector3, depth: THREE.Vector3, amount: number): THREE.Vector3 {
  return anchor.clone().addScaledVector(depth, amount)
}

function buildMaterial(
  zone: KnifeMaterialZone,
  assetId: string,
  partId: string,
  materialSpec: KnifeLayeredMaterialSpec,
): THREE.MeshPhysicalMaterial {
  const material = createKnifeLayeredMaterial(materialSpec, zone, partId)
  material.name = zone.material_zone_id
  overrideUuid(material, stableUuid(`material:${assetId}:${zone.material_zone_id}`))
  material.userData = {
    ...material.userData,
    material_zone_id: zone.material_zone_id,
    part_id: partId,
    model: zone.model,
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  }
  return material
}

function buildMesh(
  geometry: THREE.BufferGeometry,
  material: THREE.MeshPhysicalMaterial,
  materialSpec: KnifeLayeredMaterialSpec,
  program: KnifeSceneProgram,
  part: KnifePart,
  surfaceRole: KnifeRenderablePartRole,
  assemblyPrimitive?: KnifeAssemblyPrimitiveSpec['primitive'],
  center?: KnifeVec3,
  assemblyDescriptor?: KnifeAssemblyDescriptor,
): THREE.Mesh<THREE.BufferGeometry, THREE.MeshPhysicalMaterial> {
  bindKnifeLayeredMaterialGeometry(geometry, materialSpec)
  const surfaceFieldReceipt = bindLayeredSurfaceField(geometry, material, materialSpec)
  const mesh = new THREE.Mesh(geometry, material)
  mesh.name = `knife-part:${part.part_id}`
  overrideUuid(mesh, stableUuid(`part:${program.asset_id}:${part.part_id}`))
  overrideUuid(geometry, stableUuid(`geometry:${program.asset_id}:${part.part_id}`))
  mesh.userData = {
    schema_version: 'KnifeSceneProgram@1',
    asset_id: program.asset_id,
    part_id: part.part_id,
    material_zone_id: part.material_zone_id,
    surface_role: surfaceRole,
    source_class: part.source_class,
    frozen: part.frozen,
    stable_id: part.part_id,
    ...(assemblyPrimitive ? { assembly_primitive: assemblyPrimitive, assembly_center: [...center ?? []] } : {}),
    ...(assemblyDescriptor ? { assembly_descriptor: { ...assemblyDescriptor } } : {}),
    surface_field_schema_version: surfaceFieldReceipt.schema_version,
    surface_field_fingerprint: surfaceFieldReceipt.deterministic_fingerprint,
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  }
  geometry.userData.part_id = part.part_id
  geometry.userData.material_zone_id = part.material_zone_id
  if (assemblyPrimitive) geometry.userData.assembly_primitive = assemblyPrimitive
  const vertexCount = geometry.getAttribute('position').count
  geometry.setAttribute('partIdHash', new THREE.Uint32BufferAttribute(new Array(vertexCount).fill(fnv1a32(part.part_id)), 1))
  geometry.setAttribute('materialZoneHash', new THREE.Uint32BufferAttribute(new Array(vertexCount).fill(fnv1a32(part.material_zone_id)), 1))
  return mesh
}

function triangleCountFor(geometry: THREE.BufferGeometry): number {
  const index = geometry.getIndex()
  return index ? index.count / 3 : geometry.getAttribute('position').count / 3
}

function evaluateCurve(curve: KnifeCurve, u: number): Point3 {
  if (curve.basis === 'bezier') return evaluateBezier(curve.control_points, u)
  return evaluateNurbsLike(curve.control_points, u)
}

function evaluateCurveTangent(curve: KnifeCurve, u: number): Point3 {
  if (curve.basis === 'bezier') return evaluateBezierTangent(curve.control_points, u)
  const step = 1e-4
  const before = evaluateNurbsLike(curve.control_points, Math.max(0, u - step))
  const after = evaluateNurbsLike(curve.control_points, Math.min(1, u + step))
  return { x: after.x - before.x, y: after.y - before.y, z: after.z - before.z }
}

function evaluateBezier(points: readonly KnifeVec3[], u: number): Point3 {
  const degree = points.length - 1
  let x = 0
  let y = 0
  let z = 0
  for (let i = 0; i <= degree; i += 1) {
    const weight = binomial(degree, i) * (u ** i) * ((1 - u) ** (degree - i))
    x += points[i][0] * weight
    y += points[i][1] * weight
    z += points[i][2] * weight
  }
  return { x, y, z }
}

function evaluateBezierTangent(points: readonly KnifeVec3[], u: number): Point3 {
  const degree = points.length - 1
  if (degree <= 0) return { x: 1, y: 0, z: 0 }
  let x = 0
  let y = 0
  let z = 0
  for (let i = 0; i < degree; i += 1) {
    const weight = binomial(degree - 1, i) * (u ** i) * ((1 - u) ** (degree - 1 - i)) * degree
    x += (points[i + 1][0] - points[i][0]) * weight
    y += (points[i + 1][1] - points[i][1]) * weight
    z += (points[i + 1][2] - points[i][2]) * weight
  }
  return { x, y, z }
}

/** A bounded uniform cubic interpolation used only for the contract's nurbs-like basis. */
function evaluateNurbsLike(points: readonly KnifeVec3[], u: number): Point3 {
  const scaled = THREE.MathUtils.clamp(u, 0, 1) * (points.length - 1)
  const segment = Math.min(Math.floor(scaled), points.length - 2)
  const local = scaled - segment
  const p0 = points[Math.max(0, segment - 1)]
  const p1 = points[segment]
  const p2 = points[segment + 1]
  const p3 = points[Math.min(points.length - 1, segment + 2)]
  const local2 = local * local
  const local3 = local2 * local
  return {
    x: 0.5 * ((2 * p1[0]) + (-p0[0] + p2[0]) * local + (2 * p0[0] - 5 * p1[0] + 4 * p2[0] - p3[0]) * local2 + (-p0[0] + 3 * p1[0] - 3 * p2[0] + p3[0]) * local3),
    y: 0.5 * ((2 * p1[1]) + (-p0[1] + p2[1]) * local + (2 * p0[1] - 5 * p1[1] + 4 * p2[1] - p3[1]) * local2 + (-p0[1] + 3 * p1[1] - 3 * p2[1] + p3[1]) * local3),
    z: 0.5 * ((2 * p1[2]) + (-p0[2] + p2[2]) * local + (2 * p0[2] - 5 * p1[2] + 4 * p2[2] - p3[2]) * local2 + (-p0[2] + 3 * p1[2] - 3 * p2[2] + p3[2]) * local3),
  }
}

function binomial(n: number, k: number): number {
  if (k < 0 || k > n) return 0
  let result = 1
  for (let i = 1; i <= k; i += 1) result = (result * (n - (k - i))) / i
  return result
}

function rotateAroundAxis(vector: THREE.Vector3, axis: THREE.Vector3, angle: number): THREE.Vector3 {
  const cos = Math.cos(angle)
  const sin = Math.sin(angle)
  const cross = axis.clone().cross(vector)
  const dot = axis.dot(vector)
  return vector.clone().multiplyScalar(cos).addScaledVector(cross, sin).addScaledVector(axis, dot * (1 - cos))
}

function fingerprint(
  program: KnifeSceneProgram,
  parts: readonly CompiledKnifePart[],
  sections: readonly CompiledSectionRecord[],
): string {
  const tokens: string[] = [
    'weaponry-threejs-knife-compiler@1',
    program.schema_version,
    program.asset_id,
    program.blade_surface.spine_curve.curve_id,
    program.blade_surface.cutting_edge_curve.curve_id,
  ]
  for (const section of sections) tokens.push(section.section_id, section.u.toString(), section.center.join(','), section.top_thickness.toString(), section.bottom_thickness.toString())
  for (const part of parts) {
    tokens.push(part.part_id, part.material_zone_id, part.surface_role)
    tokens.push(
      part.material_spec.schema_version,
      part.material_spec.vocabulary,
      part.material_spec.controls.curvature.toString(),
      part.material_spec.controls.edge_wear.toString(),
      part.material_spec.controls.engraving_mask.toString(),
      ...part.material_spec.controls.scale_repeat.map((value) => value.toString()),
    )
    if (part.assembly_primitive) {
      tokens.push(part.assembly_primitive)
      for (const coordinate of part.center ?? []) tokens.push(coordinate.toString())
      for (const key of Object.keys(part.assembly_descriptor ?? {}).sort()) {
        tokens.push(key, String(part.assembly_descriptor![key]))
      }
    }
    const position = part.geometry.getAttribute('position')
    const normal = part.geometry.getAttribute('normal')
    const uv = part.geometry.getAttribute('uv')
    for (let i = 0; i < position.count * position.itemSize; i += 1) tokens.push(Number(position.array[i]).toString())
    for (let i = 0; i < normal.count * normal.itemSize; i += 1) tokens.push(Number(normal.array[i]).toString())
    for (let i = 0; i < uv.count * uv.itemSize; i += 1) tokens.push(Number(uv.array[i]).toString())
    for (const attributeName of Object.values(KNIFE_MATERIAL_ATTRIBUTE_NAMES)) {
      const attribute = part.geometry.getAttribute(attributeName)
      tokens.push(attributeName, attribute.itemSize.toString(), attribute.count.toString())
      for (let i = 0; i < attribute.count * attribute.itemSize; i += 1) tokens.push(Number(attribute.array[i]).toString())
    }
    for (const attributeName of Object.values(LAYERED_SURFACE_FIELD_ATTRIBUTE_NAMES)) {
      const attribute = part.geometry.getAttribute(attributeName)
      tokens.push(attributeName, attribute.itemSize.toString(), attribute.count.toString())
      for (let i = 0; i < attribute.count * attribute.itemSize; i += 1) tokens.push(Number(attribute.array[i]).toString())
    }
    const vertexColors = part.geometry.getAttribute(KNIFE_MATERIAL_VERTEX_COLOR_ATTRIBUTE)
    tokens.push(KNIFE_MATERIAL_VERTEX_COLOR_ATTRIBUTE, vertexColors.itemSize.toString(), vertexColors.count.toString())
    for (let i = 0; i < vertexColors.count * vertexColors.itemSize; i += 1) tokens.push(Number(vertexColors.array[i]).toString())
    const index = part.geometry.getIndex()
    if (index) for (let i = 0; i < index.count; i += 1) tokens.push(Number(index.array[i]).toString())
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

function fnv1a32(value: string): number {
  let hash = 0x811c9dc5
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 0x01000193)
  }
  return hash >>> 0
}

function overrideUuid(object: { readonly uuid: string }, uuid: string): void {
  // Three.js UUIDs are random by design. The derived scene uses a stable
  // deterministic UUID so repeated compiles can be compared without treating
  // runtime allocation order as asset data.
  Object.defineProperty(object, 'uuid', {
    configurable: true,
    enumerable: true,
    value: uuid,
    writable: true,
  })
}

function stableUuid(value: string): string {
  const raw = `${fnv1a64(`${value}:0`)}${fnv1a64(`${value}:1`)}${fnv1a64(`${value}:2`)}${fnv1a64(`${value}:3`)}`
  return `${raw.slice(0, 8)}-${raw.slice(8, 12)}-${raw.slice(12, 16)}-${raw.slice(16, 20)}-${raw.slice(20, 32)}`
}
