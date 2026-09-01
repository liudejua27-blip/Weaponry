import * as THREE from 'three'

import type {
  KnifeAssembly,
  KnifeAssemblyPrimitiveSpec,
  KnifePartRole,
  KnifeSceneProgram,
  KnifeVec3,
} from './knife-scene-program.ts'
import type {
  CompiledKnifePart,
  CompiledKnifeScene,
  KnifeRenderablePartRole,
} from './knife-scene-compiler.ts'

/**
 * Deterministic, no-render assembly metrics for the lightweight Three.js
 * route.  These values are design priors and structural proxies.  They are
 * deliberately not a visual-quality, human-review, engine, or commercial
 * acceptance result.
 */
export const KNIFE_ASSEMBLY_INTRINSIC_METRICS_SCHEMA = 'KnifeAssemblyIntrinsicMetrics@1' as const
export const KNIFE_ASSEMBLY_INTRINSIC_METRICS_STATUS = 'MEASURED_NOT_REVIEWED' as const
export const KNIFE_ASSEMBLY_INTRINSIC_CLASSIFICATION = 'design-prior' as const
export const KNIFE_ASSEMBLY_INTRINSIC_INTERPRETATION = 'structural-proxy-not-visual-quality' as const
export const KNIFE_ASSEMBLY_INTRINSIC_BASIS = 'program-and-compiled-scene-aabb-and-material-proxies@1' as const

const STABLE_ID = /^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/
const FINGERPRINT = /^[a-f0-9]{16,128}$/
const EPSILON = 1e-6
const MAX_RATIO = 1_000_000
const MAX_METRIC_DECIMALS = 9
const ADJACENCY_TOLERANCE_FRACTION = 0.05

export type KnifeAssemblyIntrinsicMetricValue = number | 'NOT_COMPUTABLE'
export type KnifeAssemblyIntrinsicMetricDirection = 'maximize' | 'minimize'
export type KnifeAssemblyIntrinsicMetricComputability = 'COMPUTED' | 'NOT_COMPUTABLE'
export type KnifeAssemblyIntrinsicAxis = 'x' | 'y' | 'z'

export type KnifeAssemblyIntrinsicAttachmentRelation =
  | 'blade-root-guard'
  | 'guard-grip'
  | 'grip-pommel'

export interface KnifeAssemblyIntrinsicBounds {
  readonly min: KnifeVec3
  readonly max: KnifeVec3
  readonly size: KnifeVec3
  readonly center: KnifeVec3
  readonly diagonal: number
  readonly volume: number
}

export interface KnifeAssemblyIntrinsicRatio {
  readonly ratio_id: string
  readonly numerator: string
  readonly denominator: string
  readonly value: KnifeAssemblyIntrinsicMetricValue
  readonly suggested_range: readonly [number, number]
  /** A soft 0..1 prior score; it is not a pass/fail threshold. */
  readonly prior_score: KnifeAssemblyIntrinsicMetricValue
  readonly computability: KnifeAssemblyIntrinsicMetricComputability
  readonly basis: string
}

export interface KnifeAssemblyIntrinsicAxisInterval {
  readonly axis: KnifeAssemblyIntrinsicAxis
  readonly gap: number
  readonly overlap: number
}

export interface KnifeAssemblyIntrinsicAttachment {
  readonly attachment_id: string
  readonly relation: KnifeAssemblyIntrinsicAttachmentRelation
  /** Blade-root uses both semantic blade parts; other rows contain one part. */
  readonly source_part_ids: readonly string[]
  readonly target_part_id: string
  readonly source_bounds: KnifeAssemblyIntrinsicBounds
  readonly target_bounds: KnifeAssemblyIntrinsicBounds
  readonly axis: KnifeAssemblyIntrinsicAxis
  readonly intervals: readonly KnifeAssemblyIntrinsicAxisInterval[]
  /** Euclidean AABB separation in normalized design units. */
  readonly bbox_gap: number
  readonly normalized_gap: number
  /** AABB intersection volume, zero for a face/edge touch. */
  readonly bbox_overlap: number
  readonly bbox_overlap_fraction: number
  /** 1 means the two AABBs have no separation; this is not mesh contact proof. */
  readonly bbox_continuity: number
  readonly spatially_adjacent: boolean
  readonly computability: KnifeAssemblyIntrinsicMetricComputability
  readonly basis: string
}

export interface KnifeAssemblyIntrinsicMaterialChannels {
  readonly base_tone: number
  readonly metalness: number
  readonly roughness: number
  readonly normal_strength: number
  readonly emissive: number
  readonly wear_mask: number
}

export interface KnifeAssemblyIntrinsicMaterialChannelDelta {
  readonly base_tone: number
  readonly metalness: number
  readonly roughness: number
  readonly normal_strength: number
  readonly emissive: number
  readonly wear_mask: number
}

export interface KnifeAssemblyIntrinsicMaterialAdjacency {
  readonly adjacency_id: string
  readonly left_material_zone_id: string
  readonly right_material_zone_id: string
  readonly left_part_ids: readonly string[]
  readonly right_part_ids: readonly string[]
  readonly spatial_part_pair_count: number
  readonly spatially_adjacent: boolean
  readonly bbox_gap: number
  readonly bbox_overlap_fraction: number
  readonly left_channels: KnifeAssemblyIntrinsicMaterialChannels
  readonly right_channels: KnifeAssemblyIntrinsicMaterialChannels
  readonly channel_delta: KnifeAssemblyIntrinsicMaterialChannelDelta
  /** Weighted L1 channel contrast in [0, 1]; an auxiliary readability cue. */
  readonly readability: number
  readonly computability: 'COMPUTED'
  readonly basis: string
}

export interface KnifeAssemblyIntrinsicMaterialAdjacencySummary {
  readonly zone_count: number
  readonly adjacent_zone_pair_count: number
  readonly candidate_zone_pair_count: number
  /** Ratio of candidate material-zone pairs that have a semantic/spatial row. */
  readonly adjacency_coverage: KnifeAssemblyIntrinsicMetricValue
  readonly mean_readability: KnifeAssemblyIntrinsicMetricValue
  readonly entries: readonly KnifeAssemblyIntrinsicMaterialAdjacency[]
  readonly basis: string
}

export interface KnifeAssemblyIntrinsicPartComplexity {
  readonly part_id: string
  readonly surface_role: KnifeRenderablePartRole
  readonly material_zone_id: string
  readonly draw_calls: 1
  readonly vertex_count: number
  readonly triangle_count: number
  readonly triangle_budget_fraction: number
  readonly vertex_to_triangle_ratio: number
}

export interface KnifeAssemblyIntrinsicComplexity {
  readonly draw_calls: number
  readonly vertices: number
  readonly triangles: number
  readonly max_draw_calls: number
  readonly max_triangles: number
  readonly draw_call_budget_fraction: number
  readonly triangle_budget_fraction: number
  readonly draw_call_headroom: number
  readonly triangle_headroom: number
  /** Budget headroom proxy, not a rendering-performance benchmark. */
  readonly draw_call_efficiency: number
  readonly triangle_efficiency: number
  readonly complexity_efficiency: number
  readonly triangles_per_draw_call: number
  readonly parts: readonly KnifeAssemblyIntrinsicPartComplexity[]
  readonly basis: string
}

export interface KnifeAssemblyIntrinsicMetricDetail {
  readonly direction: KnifeAssemblyIntrinsicMetricDirection
  readonly computable: boolean
  readonly classification: typeof KNIFE_ASSEMBLY_INTRINSIC_CLASSIFICATION
  readonly basis: string
}

export interface KnifeAssemblyIntrinsicReadabilityProxy {
  readonly ratio_prior_score: KnifeAssemblyIntrinsicMetricValue
  readonly attachment_continuity: KnifeAssemblyIntrinsicMetricValue
  readonly material_zone_readability: KnifeAssemblyIntrinsicMetricValue
  readonly complexity_efficiency: number
  readonly combined: KnifeAssemblyIntrinsicMetricValue
  readonly basis: string
}

export interface KnifeAssemblyIntrinsicMetricsInput {
  readonly program: KnifeSceneProgram
  readonly compiled: CompiledKnifeScene
}

export interface KnifeAssemblyIntrinsicMetrics {
  readonly schema_version: typeof KNIFE_ASSEMBLY_INTRINSIC_METRICS_SCHEMA
  readonly source_fingerprint: string
  readonly program_fingerprint: string
  readonly asset_id: string
  readonly classification: typeof KNIFE_ASSEMBLY_INTRINSIC_CLASSIFICATION
  readonly interpretation: typeof KNIFE_ASSEMBLY_INTRINSIC_INTERPRETATION
  readonly basis: typeof KNIFE_ASSEMBLY_INTRINSIC_BASIS
  readonly axes: {
    readonly longitudinal: KnifeAssemblyIntrinsicAxis
    readonly lateral: KnifeAssemblyIntrinsicAxis
    readonly thickness: KnifeAssemblyIntrinsicAxis
  }
  readonly asset_bounds: KnifeAssemblyIntrinsicBounds
  readonly blade_bounds: KnifeAssemblyIntrinsicBounds
  readonly blade_root_bounds: KnifeAssemblyIntrinsicBounds | 'NOT_COMPUTABLE'
  readonly part_bounds: readonly {
    readonly part_id: string
    readonly surface_role: KnifeRenderablePartRole
    readonly material_zone_id: string
    readonly bounds: KnifeAssemblyIntrinsicBounds
  }[]
  readonly ratios: {
    readonly guard_root: KnifeAssemblyIntrinsicRatio
    readonly grip_blade: KnifeAssemblyIntrinsicRatio
    readonly pommel_grip: KnifeAssemblyIntrinsicRatio
    readonly fastener_grip: KnifeAssemblyIntrinsicRatio
    readonly gem_guard: KnifeAssemblyIntrinsicRatio
  }
  readonly attachments: readonly KnifeAssemblyIntrinsicAttachment[]
  readonly material_zone_adjacency: KnifeAssemblyIntrinsicMaterialAdjacencySummary
  readonly complexity: KnifeAssemblyIntrinsicComplexity
  readonly readability_proxy: KnifeAssemblyIntrinsicReadabilityProxy
  readonly metric_values: Readonly<Record<string, KnifeAssemblyIntrinsicMetricValue>>
  readonly metric_details: Readonly<Record<string, KnifeAssemblyIntrinsicMetricDetail>>
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
  readonly visual_quality_status: 'NOT_COMPUTABLE'
  readonly status: typeof KNIFE_ASSEMBLY_INTRINSIC_METRICS_STATUS
  readonly deterministic_fingerprint: string
}

export class KnifeAssemblyIntrinsicMetricsError extends Error {
  constructor(message: string) {
    super(`KNIFE_ASSEMBLY_INTRINSIC_METRICS_INVALID: ${message}`)
    this.name = 'KnifeAssemblyIntrinsicMetricsError'
  }
}

/**
 * Recompute the metrics from the closed program and the actual compiled
 * geometry.  No caller-supplied metric, delta, render mask, or quality result
 * is accepted.  The object overload is useful at adapter boundaries where a
 * single closed input envelope is preferable.
 */
export function measureKnifeAssemblyIntrinsicMetrics(
  program: KnifeSceneProgram,
  compiled: CompiledKnifeScene,
): KnifeAssemblyIntrinsicMetrics
export function measureKnifeAssemblyIntrinsicMetrics(
  input: KnifeAssemblyIntrinsicMetricsInput,
): KnifeAssemblyIntrinsicMetrics
export function measureKnifeAssemblyIntrinsicMetrics(
  first: KnifeSceneProgram | KnifeAssemblyIntrinsicMetricsInput,
  second?: CompiledKnifeScene,
): KnifeAssemblyIntrinsicMetrics {
  if (arguments.length > 2) throw new KnifeAssemblyIntrinsicMetricsError('only program and compiled scene are accepted')
  const { program, compiled } = resolveInput(first, second)
  validateProgramShell(program)
  validateCompiledShell(compiled)
  const specs = assemblySpecs(program.assembly)
  validateProgramCompiledBinding(program, compiled, specs)

  const clouds = compiled.parts.map((part) => ({
    part,
    cloud: buildWorldCloud(part),
  }))
  const cloudByPartId = new Map(clouds.map((entry) => [entry.part.part_id, entry.cloud]))
  const partBounds = clouds.map(({ part, cloud }) => ({
    part_id: part.part_id,
    surface_role: part.surface_role,
    material_zone_id: part.material_zone_id,
    bounds: boundsOutput(cloud.bounds),
  }))
  const assetCloud = unionClouds(clouds.map((entry) => entry.cloud))
  const bladeCloud = unionClouds(clouds
    .filter(({ part }) => part.surface_role === 'blade-body' || part.surface_role === 'cutting-edge')
    .map((entry) => entry.cloud))
  const axes = dominantAxes(bladeCloud.bounds)
  const rootCloud = bladeRootCloud(bladeCloud, compiled, axes.longitudinal)
  const rootBounds = rootCloud ? boundsOutput(rootCloud.bounds) : 'NOT_COMPUTABLE'
  const partBoundById = new Map(partBounds.map((entry) => [entry.part_id, entry.bounds]))
  const boundsByRole = roleBounds(compiled.parts, partBoundById, rootBounds)
  const ratios = buildRatios(boundsByRole, axes)
  const attachments = buildAttachments(boundsByRole, axes, assetCloud.bounds)
  const materialAdjacency = buildMaterialAdjacency(compiled.parts, clouds, assetCloud.bounds)
  const complexity = buildComplexity(compiled, program.budgets.max_draw_calls, program.budgets.max_triangles)
  const readabilityProxy = buildReadabilityProxy(ratios, attachments, materialAdjacency, complexity)
  const { metric_values, metric_details } = buildMetricMaps(ratios, attachments, materialAdjacency, complexity)

  const draft: Omit<KnifeAssemblyIntrinsicMetrics, 'deterministic_fingerprint'> = {
    schema_version: KNIFE_ASSEMBLY_INTRINSIC_METRICS_SCHEMA,
    source_fingerprint: compiled.deterministic_fingerprint,
    program_fingerprint: fingerprintProgram(program),
    asset_id: program.asset_id,
    classification: KNIFE_ASSEMBLY_INTRINSIC_CLASSIFICATION,
    interpretation: KNIFE_ASSEMBLY_INTRINSIC_INTERPRETATION,
    basis: KNIFE_ASSEMBLY_INTRINSIC_BASIS,
    axes,
    asset_bounds: boundsOutput(assetCloud.bounds),
    blade_bounds: boundsOutput(bladeCloud.bounds),
    blade_root_bounds: rootBounds,
    part_bounds: Object.freeze(partBounds),
    ratios,
    attachments: Object.freeze(attachments),
    material_zone_adjacency: materialAdjacency,
    complexity,
    readability_proxy: readabilityProxy,
    metric_values,
    metric_details,
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
    visual_quality_status: 'NOT_COMPUTABLE',
    status: KNIFE_ASSEMBLY_INTRINSIC_METRICS_STATUS,
  }
  const deterministicFingerprint = fnv1a64(canonicalJson(draft))
  return deepFreeze({ ...draft, deterministic_fingerprint: deterministicFingerprint })
}

/** Alias used by callers that name the output as an evaluator result. */
export const evaluateKnifeAssemblyIntrinsicMetrics = measureKnifeAssemblyIntrinsicMetrics
/** Short alias retained for the design-knowledge layer. */
export const measureKnifeAssemblyReadabilityMetrics = measureKnifeAssemblyIntrinsicMetrics

function resolveInput(
  first: KnifeSceneProgram | KnifeAssemblyIntrinsicMetricsInput,
  second: CompiledKnifeScene | undefined,
): KnifeAssemblyIntrinsicMetricsInput {
  const record = first as unknown as Record<string, unknown>
  if (isRecord(first) && Object.prototype.hasOwnProperty.call(record, 'program')) {
    if (second !== undefined) throw new KnifeAssemblyIntrinsicMetricsError('object input cannot be combined with a positional compiled scene')
    exactKeys(record, ['program', 'compiled'], 'input')
    if (!isRecord(record.program) || !isRecord(record.compiled)) {
      throw new KnifeAssemblyIntrinsicMetricsError('input.program and input.compiled must be objects')
    }
    return { program: record.program as unknown as KnifeSceneProgram, compiled: record.compiled as unknown as CompiledKnifeScene }
  }
  if (!second) throw new KnifeAssemblyIntrinsicMetricsError('compiled scene is required')
  return { program: first as KnifeSceneProgram, compiled: second }
}

function validateProgramShell(program: KnifeSceneProgram): void {
  if (!isRecord(program)) throw new KnifeAssemblyIntrinsicMetricsError('program must be an object')
  exactKeys(
    program as unknown as Record<string, unknown>,
    ['schema_version', 'asset_id', 'family', 'design_basis', 'coordinate_convention', 'blade_surface', 'parts', 'material_zones', 'presentation', 'budgets', 'unknowns', 'canonical_sha256'],
    'program',
    ['assembly', 'source_envelope'],
  )
  if (program.schema_version !== 'KnifeSceneProgram@1'
    || program.coordinate_convention !== 'weapon-front-z-up-right-handed@1'
    || typeof program.asset_id !== 'string'
    || !STABLE_ID.test(program.asset_id)) {
    throw new KnifeAssemblyIntrinsicMetricsError('program schema, asset_id, or coordinate convention is invalid')
  }
  if (!isRecord(program.blade_surface)
    || !Array.isArray(program.blade_surface.sections)
    || !Array.isArray(program.parts)
    || !Array.isArray(program.material_zones)
    || !isRecord(program.budgets)) {
    throw new KnifeAssemblyIntrinsicMetricsError('program blade_surface, parts, material_zones, or budgets is invalid')
  }
  const partIds = new Set<string>()
  for (const part of program.parts) {
    if (!isRecord(part) || typeof part.part_id !== 'string' || !STABLE_ID.test(part.part_id) || partIds.has(part.part_id)) {
      throw new KnifeAssemblyIntrinsicMetricsError('program parts must have unique bounded IDs')
    }
    partIds.add(part.part_id)
    if (typeof part.material_zone_id !== 'string' || !STABLE_ID.test(part.material_zone_id)) {
      throw new KnifeAssemblyIntrinsicMetricsError(`program part ${part.part_id} has an invalid material zone ID`)
    }
  }
  const zoneIds = new Set<string>()
  for (const zone of program.material_zones) {
    if (!isRecord(zone) || typeof zone.material_zone_id !== 'string' || !STABLE_ID.test(zone.material_zone_id) || zoneIds.has(zone.material_zone_id)) {
      throw new KnifeAssemblyIntrinsicMetricsError('program material zones must have unique bounded IDs')
    }
    if (!/^#[0-9a-f]{6}$/i.test(String(zone.base_color))
      || !Number.isFinite(zone.metalness)
      || !Number.isFinite(zone.roughness)) {
      throw new KnifeAssemblyIntrinsicMetricsError(`material zone ${zone.material_zone_id} has a non-finite or invalid color/PBR value`)
    }
    zoneIds.add(zone.material_zone_id)
  }
  for (const part of program.parts) {
    if (!zoneIds.has(part.material_zone_id)) throw new KnifeAssemblyIntrinsicMetricsError(`part ${part.part_id} references an unknown material zone`)
  }
  if (!Number.isInteger(program.budgets.max_triangles)
    || !Number.isInteger(program.budgets.max_draw_calls)
    || program.budgets.max_triangles <= 0
    || program.budgets.max_draw_calls <= 0) {
    throw new KnifeAssemblyIntrinsicMetricsError('program budgets must be finite and positive')
  }
  if (program.assembly !== undefined) assemblySpecs(program.assembly)
}

function validateCompiledShell(compiled: CompiledKnifeScene): void {
  if (!isRecord(compiled)) throw new KnifeAssemblyIntrinsicMetricsError('compiled scene must be an object')
  exactKeys(
    compiled as unknown as Record<string, unknown>,
    ['group', 'parts', 'assembly_parts', 'assembly_status', 'sections', 'triangle_count', 'longitudinal_segments', 'deterministic_fingerprint', 'renderer_invoked', 'quality_status'],
    'compiled scene',
  )
  if (!isRecord(compiled.group)
    || !Array.isArray(compiled.parts)
    || !Array.isArray(compiled.assembly_parts)
    || !Array.isArray(compiled.sections)
    || !FINGERPRINT.test(compiled.deterministic_fingerprint)
    || compiled.renderer_invoked !== false
    || compiled.quality_status !== 'NOT_RUN'
    || !Number.isInteger(compiled.triangle_count)
    || compiled.triangle_count <= 0) {
    throw new KnifeAssemblyIntrinsicMetricsError('compiled scene is not a closed, pre-rendered scene')
  }
  const seen = new Set<string>()
  for (const part of compiled.parts) {
    validateCompiledPart(part)
    if (seen.has(part.part_id)) throw new KnifeAssemblyIntrinsicMetricsError(`compiled scene has duplicate part ${part.part_id}`)
    seen.add(part.part_id)
  }
  const triangleCount = compiled.parts.reduce((total, part) => total + triangleCountFor(part.geometry), 0)
  if (triangleCount !== compiled.triangle_count) throw new KnifeAssemblyIntrinsicMetricsError('compiled triangle_count does not match geometry')
}

function validateCompiledPart(part: CompiledKnifePart): void {
  if (!isRecord(part)
    || typeof part.part_id !== 'string'
    || !STABLE_ID.test(part.part_id)
    || typeof part.material_zone_id !== 'string'
    || !STABLE_ID.test(part.material_zone_id)
    || !isRecord(part.geometry)
    || !isRecord(part.mesh)
    || !isRecord(part.material)
    || !isRecord(part.material_spec)) {
    throw new KnifeAssemblyIntrinsicMetricsError('compiled part has an invalid closed shape')
  }
  if (part.mesh.geometry !== part.geometry || part.mesh.material !== part.material) {
    throw new KnifeAssemblyIntrinsicMetricsError(`compiled part ${part.part_id} lost geometry/material identity binding`)
  }
  const position = part.geometry.getAttribute('position')
  if (!position || position.itemSize !== 3 || position.count === 0) {
    throw new KnifeAssemblyIntrinsicMetricsError(`compiled part ${part.part_id} has no position attribute`)
  }
  for (let index = 0; index < position.count * position.itemSize; index += 1) {
    if (!Number.isFinite(Number(position.array[index]))) throw new KnifeAssemblyIntrinsicMetricsError(`compiled part ${part.part_id} has a non-finite vertex`)
  }
  for (const value of [part.mesh.position.x, part.mesh.position.y, part.mesh.position.z, part.mesh.scale.x, part.mesh.scale.y, part.mesh.scale.z]) {
    if (!Number.isFinite(value)) throw new KnifeAssemblyIntrinsicMetricsError(`compiled part ${part.part_id} has a non-finite transform`)
  }
}

function validateProgramCompiledBinding(
  program: KnifeSceneProgram,
  compiled: CompiledKnifeScene,
  specs: readonly KnifeAssemblyPrimitiveSpec[],
): void {
  if (compiled.parts.length > program.budgets.max_draw_calls || compiled.triangle_count > program.budgets.max_triangles) {
    throw new KnifeAssemblyIntrinsicMetricsError('compiled scene exceeds the declared program geometry budget')
  }
  const programParts = new Map(program.parts.map((part) => [part.part_id, part]))
  const specByPartId = new Map(specs.map((spec) => [spec.part_id, spec]))
  const assemblyPartIds = new Set(compiled.assembly_parts.map((part) => part.part_id))
  if (assemblyPartIds.size !== compiled.assembly_parts.length) throw new KnifeAssemblyIntrinsicMetricsError('compiled assembly parts contain duplicate IDs')
  if (compiled.assembly_status !== (specs.length > 0 ? 'COMPILED' : 'NOT_PRESENT')) {
    throw new KnifeAssemblyIntrinsicMetricsError('compiled assembly status is not bound to the program assembly')
  }
  for (const part of compiled.parts) {
    const source = programParts.get(part.part_id)
    if (!source || source.role === 'helper' || part.surface_role !== source.role) {
      throw new KnifeAssemblyIntrinsicMetricsError(`compiled part ${part.part_id} is not bound to a renderable program part`)
    }
    if (part.material_zone_id !== source.material_zone_id) {
      throw new KnifeAssemblyIntrinsicMetricsError(`compiled part ${part.part_id} has a material zone binding mismatch`)
    }
    const spec = specByPartId.get(part.part_id)
    if (spec) {
      if (part.assembly_primitive !== spec.primitive || !part.center || !closeVec3(part.center, spec.center)) {
        throw new KnifeAssemblyIntrinsicMetricsError(`compiled assembly part ${part.part_id} is not bound to its program primitive`)
      }
      if (!closeVec3([part.mesh.position.x, part.mesh.position.y, part.mesh.position.z], spec.center)) {
        throw new KnifeAssemblyIntrinsicMetricsError(`compiled assembly part ${part.part_id} lost its declared center`)
      }
      if (!assemblyPartIds.has(part.part_id)) throw new KnifeAssemblyIntrinsicMetricsError(`compiled assembly index omitted ${part.part_id}`)
    } else if (part.assembly_primitive !== undefined || assemblyPartIds.has(part.part_id)) {
      throw new KnifeAssemblyIntrinsicMetricsError(`compiled part ${part.part_id} has an unbound assembly primitive`)
    }
  }
  if (compiled.assembly_parts.some((part) => !specByPartId.has(part.part_id))) {
    throw new KnifeAssemblyIntrinsicMetricsError('compiled assembly contains a part absent from program assembly')
  }
}

function assemblySpecs(assembly: KnifeAssembly | undefined): readonly KnifeAssemblyPrimitiveSpec[] {
  if (assembly === undefined) return []
  if (!isRecord(assembly)) throw new KnifeAssemblyIntrinsicMetricsError('program assembly must be an object')
  exactKeys(assembly as unknown as Record<string, unknown>, [], 'assembly', ['guard', 'grip', 'pommel', 'fasteners', 'gems', 'reliefs'])
  const typedAssembly = assembly as unknown as KnifeAssembly
  const specs: KnifeAssemblyPrimitiveSpec[] = []
  if (typedAssembly.guard) specs.push(typedAssembly.guard)
  if (typedAssembly.grip) specs.push(typedAssembly.grip)
  if (typedAssembly.pommel) specs.push(typedAssembly.pommel)
  specs.push(...(typedAssembly.fasteners ?? []), ...(typedAssembly.gems ?? []), ...(typedAssembly.reliefs ?? []))
  const seen = new Set<string>()
  for (const spec of specs) {
    if (!isRecord(spec) || typeof spec.part_id !== 'string' || !STABLE_ID.test(spec.part_id) || seen.has(spec.part_id)) {
      throw new KnifeAssemblyIntrinsicMetricsError('assembly primitives must have unique bounded IDs')
    }
    if (!['guard', 'grip', 'pommel', 'fastener', 'gem', 'relief'].includes(spec.primitive)) {
      throw new KnifeAssemblyIntrinsicMetricsError(`assembly primitive ${spec.part_id} is outside the closed vocabulary`)
    }
    seen.add(spec.part_id)
  }
  return Object.freeze(specs)
}

interface WorldCloud {
  readonly points: readonly THREE.Vector3[]
  readonly bounds: Bounds
}

interface Bounds {
  readonly min: THREE.Vector3
  readonly max: THREE.Vector3
}

function buildWorldCloud(part: CompiledKnifePart): WorldCloud {
  const position = part.geometry.getAttribute('position')
  const matrix = worldMatrix(part.mesh)
  const points: THREE.Vector3[] = []
  for (let index = 0; index < position.count; index += 1) {
    const point = new THREE.Vector3(position.getX(index), position.getY(index), position.getZ(index)).applyMatrix4(matrix)
    if (![point.x, point.y, point.z].every(Number.isFinite)) throw new KnifeAssemblyIntrinsicMetricsError(`part ${part.part_id} produced a non-finite world vertex`)
    points.push(point)
  }
  const bounds = boundsForPoints(points)
  if (!bounds) throw new KnifeAssemblyIntrinsicMetricsError(`part ${part.part_id} produced an empty world bound`)
  return { points: Object.freeze(points), bounds }
}

function worldMatrix(object: THREE.Object3D): THREE.Matrix4 {
  let matrix = localMatrix(object)
  const parents: THREE.Object3D[] = []
  for (let parent = object.parent; parent; parent = parent.parent) parents.push(parent)
  for (let index = parents.length - 1; index >= 0; index -= 1) matrix = localMatrix(parents[index]).multiply(matrix)
  return matrix
}

function localMatrix(object: THREE.Object3D): THREE.Matrix4 {
  return new THREE.Matrix4().compose(object.position.clone(), object.quaternion.clone(), object.scale.clone())
}

function boundsForPoints(points: readonly THREE.Vector3[]): Bounds | undefined {
  if (points.length === 0) return undefined
  const min = new THREE.Vector3(Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY)
  const max = new THREE.Vector3(Number.NEGATIVE_INFINITY, Number.NEGATIVE_INFINITY, Number.NEGATIVE_INFINITY)
  for (const point of points) {
    min.min(point)
    max.max(point)
  }
  return { min, max }
}

function unionClouds(clouds: readonly WorldCloud[]): WorldCloud {
  const points = clouds.flatMap((cloud) => cloud.points)
  const bounds = boundsForPoints(points)
  if (!bounds) throw new KnifeAssemblyIntrinsicMetricsError('compiled scene has no geometry points')
  return { points: Object.freeze(points), bounds }
}

function boundsOutput(bounds: Bounds): KnifeAssemblyIntrinsicBounds {
  const size = new THREE.Vector3().subVectors(bounds.max, bounds.min)
  const center = new THREE.Vector3().addVectors(bounds.max, bounds.min).multiplyScalar(0.5)
  const diagonal = size.length()
  const volume = Math.max(0, size.x * size.y * size.z)
  return Object.freeze({
    min: vec3Output(bounds.min),
    max: vec3Output(bounds.max),
    size: vec3Output(size),
    center: vec3Output(center),
    diagonal: metric(diagonal),
    volume: metric(volume),
  })
}

function vec3Output(value: THREE.Vector3): KnifeVec3 {
  return Object.freeze([metric(value.x), metric(value.y), metric(value.z)]) as unknown as KnifeVec3
}

function dominantAxes(bounds: Bounds): KnifeAssemblyIntrinsicMetrics['axes'] {
  const size = [bounds.max.x - bounds.min.x, bounds.max.y - bounds.min.y, bounds.max.z - bounds.min.z]
  const longitudinal = axisName(indexOfLargest(size, [0, 1, 2]))
  const lateralCandidates = [0, 1, 2].filter((axis) => axisName(axis) !== longitudinal)
  const lateral = axisName(indexOfLargest(size, lateralCandidates))
  const thickness = axisName([0, 1, 2].find((axis) => axisName(axis) !== longitudinal && axisName(axis) !== lateral) ?? 2)
  return Object.freeze({ longitudinal, lateral, thickness })
}

function indexOfLargest(values: readonly number[], candidates: readonly number[]): number {
  return candidates.reduce((best, candidate) => values[candidate] > values[best] ? candidate : best, candidates[0])
}

function axisName(axis: number): KnifeAssemblyIntrinsicAxis {
  return axis === 0 ? 'x' : axis === 1 ? 'y' : 'z'
}

function axisIndex(axis: KnifeAssemblyIntrinsicAxis): 0 | 1 | 2 {
  return axis === 'x' ? 0 : axis === 'y' ? 1 : 2
}

function bladeRootCloud(
  blade: WorldCloud,
  compiled: CompiledKnifeScene,
  longitudinal: KnifeAssemblyIntrinsicAxis,
): WorldCloud | undefined {
  if (blade.points.length === 0) return undefined
  const index = axisIndex(longitudinal)
  const rootCenter = compiled.sections[0]?.center[index]
  const min = component(blade.bounds.min, index)
  const max = component(blade.bounds.max, index)
  if (!Number.isFinite(rootCenter)) return undefined
  const rootAtMin = Math.abs(rootCenter - min) <= Math.abs(rootCenter - max)
  const span = Math.max(max - min, EPSILON)
  const window = Math.max(span * 0.12, EPSILON * 16)
  const limit = rootAtMin ? min + window : max - window
  const points = blade.points.filter((point) => rootAtMin ? component(point, index) <= limit : component(point, index) >= limit)
  const selected = points.length >= 3 ? points : blade.points.slice().sort((left, right) => {
    const leftDistance = Math.abs(component(left, index) - rootCenter)
    const rightDistance = Math.abs(component(right, index) - rootCenter)
    return leftDistance - rightDistance
  }).slice(0, Math.min(32, blade.points.length))
  const bounds = boundsForPoints(selected)
  return bounds ? { points: Object.freeze(selected), bounds } : undefined
}

function component(vector: THREE.Vector3, index: number): number {
  return index === 0 ? vector.x : index === 1 ? vector.y : vector.z
}

interface RoleBounds {
  readonly blade: KnifeAssemblyIntrinsicBounds
  readonly bladeRoot: KnifeAssemblyIntrinsicBounds | 'NOT_COMPUTABLE'
  readonly guard: KnifeAssemblyIntrinsicBounds | undefined
  readonly grip: KnifeAssemblyIntrinsicBounds | undefined
  readonly pommel: KnifeAssemblyIntrinsicBounds | undefined
  readonly fasteners: readonly KnifeAssemblyIntrinsicBounds[]
  readonly gems: readonly KnifeAssemblyIntrinsicBounds[]
}

function roleBounds(
  parts: readonly CompiledKnifePart[],
  boundsByPartId: ReadonlyMap<string, KnifeAssemblyIntrinsicBounds>,
  bladeRoot: KnifeAssemblyIntrinsicBounds | 'NOT_COMPUTABLE',
): RoleBounds {
  const find = (role: KnifePartRole): KnifeAssemblyIntrinsicBounds | undefined => {
    const part = parts.find((candidate) => candidate.surface_role === role)
    return part ? boundsByPartId.get(part.part_id) : undefined
  }
  return {
    blade: boundsForRole(parts, boundsByPartId, ['blade-body', 'cutting-edge']),
    bladeRoot,
    guard: find('guard'),
    grip: find('grip'),
    pommel: find('pommel'),
    fasteners: parts.filter((part) => part.surface_role === 'fastener').map((part) => boundsByPartId.get(part.part_id)!).filter(Boolean),
    gems: parts.filter((part) => part.surface_role === 'gem').map((part) => boundsByPartId.get(part.part_id)!).filter(Boolean),
  }
}

function boundsForRole(
  parts: readonly CompiledKnifePart[],
  boundsByPartId: ReadonlyMap<string, KnifeAssemblyIntrinsicBounds>,
  roles: readonly KnifeRenderablePartRole[],
): KnifeAssemblyIntrinsicBounds {
  const matching = parts.filter((part) => roles.includes(part.surface_role)).map((part) => boundsByPartId.get(part.part_id)).filter((value): value is KnifeAssemblyIntrinsicBounds => value !== undefined)
  if (matching.length === 0) throw new KnifeAssemblyIntrinsicMetricsError(`compiled scene has no ${roles.join('/')} bounds`)
  const min = new THREE.Vector3(Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY)
  const max = new THREE.Vector3(Number.NEGATIVE_INFINITY, Number.NEGATIVE_INFINITY, Number.NEGATIVE_INFINITY)
  for (const bound of matching) {
    min.min(new THREE.Vector3(...bound.min))
    max.max(new THREE.Vector3(...bound.max))
  }
  return boundsOutput({ min, max })
}

function buildRatios(
  role: RoleBounds,
  axes: KnifeAssemblyIntrinsicMetrics['axes'],
): KnifeAssemblyIntrinsicMetrics['ratios'] {
  const lateral = axisIndex(axes.lateral)
  const longitudinal = axisIndex(axes.longitudinal)
  const width = (bounds: KnifeAssemblyIntrinsicBounds | undefined): number | undefined => bounds ? bounds.size[lateral] : undefined
  const length = (bounds: KnifeAssemblyIntrinsicBounds | undefined): number | undefined => bounds ? bounds.size[longitudinal] : undefined
  const ratio = (
    ratioId: string,
    numerator: string,
    denominator: string,
    numeratorValue: number | undefined,
    denominatorValue: number | undefined,
    range: readonly [number, number],
    basis: string,
  ): KnifeAssemblyIntrinsicRatio => {
    if (numeratorValue === undefined || denominatorValue === undefined || denominatorValue <= EPSILON) {
      return Object.freeze({ ratio_id: ratioId, numerator, denominator, value: 'NOT_COMPUTABLE', suggested_range: range, prior_score: 'NOT_COMPUTABLE', computability: 'NOT_COMPUTABLE', basis })
    }
    const value = metric(Math.min(MAX_RATIO, Math.max(0, numeratorValue / denominatorValue)))
    return Object.freeze({ ratio_id: ratioId, numerator, denominator, value, suggested_range: range, prior_score: metric(rangePriorScore(value, range)), computability: 'COMPUTED', basis })
  }
  return Object.freeze({
    guard_root: ratio('guard-width-to-root-width', 'guard compiled lateral bbox width', 'blade root compiled lateral bbox width', width(role.guard), role.bladeRoot === 'NOT_COMPUTABLE' ? undefined : role.bladeRoot.size[lateral], [1, 3.2], 'compiled-guard-to-blade-root-bbox-ratio@1'),
    grip_blade: ratio('grip-length-to-blade-length', 'grip compiled longitudinal bbox length', 'blade compiled longitudinal bbox length', length(role.grip), length(role.blade), [0.22, 0.58], 'compiled-grip-to-blade-bbox-ratio@1'),
    pommel_grip: ratio('pommel-width-to-grip-width', 'pommel compiled lateral bbox width', 'grip compiled lateral bbox width', width(role.pommel), width(role.grip), [0.8, 2.4], 'compiled-pommel-to-grip-bbox-ratio@1'),
    fastener_grip: ratio('fastener-diameter-to-grip-width', 'largest fastener compiled lateral bbox width', 'grip compiled lateral bbox width', role.fasteners.length > 0 ? Math.max(...role.fasteners.map((bound) => width(bound) ?? 0)) : undefined, width(role.grip), [0.04, 0.28], 'compiled-fastener-to-grip-bbox-ratio@1'),
    gem_guard: ratio('gem-diameter-to-guard-width', 'largest gem compiled lateral bbox width', 'guard compiled lateral bbox width', role.gems.length > 0 ? Math.max(...role.gems.map((bound) => width(bound) ?? 0)) : undefined, width(role.guard), [0.05, 0.34], 'compiled-gem-to-guard-bbox-ratio@1'),
  })
}

function rangePriorScore(value: number, range: readonly [number, number]): number {
  const [minimum, maximum] = range
  if (value >= minimum && value <= maximum) return 1
  const distance = value < minimum ? minimum - value : value - maximum
  return clamp01(1 - distance / Math.max(maximum - minimum, EPSILON))
}

function buildAttachments(
  role: RoleBounds,
  axes: KnifeAssemblyIntrinsicMetrics['axes'],
  assetBounds: Bounds,
): readonly KnifeAssemblyIntrinsicAttachment[] {
  const relations: readonly {
    readonly relation: KnifeAssemblyIntrinsicAttachmentRelation
    readonly sourcePartIds: readonly string[]
    readonly targetPartId: string
    readonly source: KnifeAssemblyIntrinsicBounds | undefined
    readonly target: KnifeAssemblyIntrinsicBounds | undefined
  }[] = [
    {
      relation: 'blade-root-guard',
      sourcePartIds: ['blade-body', 'cutting-edge'],
      targetPartId: 'guard',
      source: role.bladeRoot === 'NOT_COMPUTABLE' ? role.blade : role.bladeRoot,
      target: role.guard,
    },
    {
      relation: 'guard-grip',
      sourcePartIds: ['guard'],
      targetPartId: 'grip',
      source: role.guard,
      target: role.grip,
    },
    {
      relation: 'grip-pommel',
      sourcePartIds: ['grip'],
      targetPartId: 'pommel',
      source: role.grip,
      target: role.pommel,
    },
  ]
  const assetDiagonal = Math.max(boundsOutput(assetBounds).diagonal, EPSILON)
  const result: KnifeAssemblyIntrinsicAttachment[] = []
  for (const item of relations) {
    if (!item.source || !item.target) continue
    const pair = boundsPair(item.source, item.target, axes.longitudinal, assetDiagonal)
    result.push(Object.freeze({
      attachment_id: `${item.relation}@1`,
      relation: item.relation,
      source_part_ids: Object.freeze([...item.sourcePartIds]),
      target_part_id: item.targetPartId,
      source_bounds: item.source,
      target_bounds: item.target,
      axis: axes.longitudinal,
      intervals: Object.freeze(pair.intervals),
      bbox_gap: metric(pair.gapDistance),
      normalized_gap: metric(pair.gapDistance / assetDiagonal),
      bbox_overlap: metric(pair.overlapVolume),
      bbox_overlap_fraction: metric(pair.overlapFraction),
      bbox_continuity: metric(clamp01(1 - pair.gapDistance / assetDiagonal)),
      spatially_adjacent: pair.gapDistance <= assetDiagonal * ADJACENCY_TOLERANCE_FRACTION + EPSILON,
      computability: 'COMPUTED',
      basis: 'compiled-world-aabb-gap-overlap-continuity-proxy@1',
    }))
  }
  return result
}

interface BoundsPair {
  readonly intervals: readonly KnifeAssemblyIntrinsicAxisInterval[]
  readonly gapDistance: number
  readonly overlapVolume: number
  readonly overlapFraction: number
}

function boundsPair(
  left: KnifeAssemblyIntrinsicBounds,
  right: KnifeAssemblyIntrinsicBounds,
  longitudinal: KnifeAssemblyIntrinsicAxis,
  _assetDiagonal: number,
): BoundsPair {
  const axes: readonly KnifeAssemblyIntrinsicAxis[] = ['x', 'y', 'z']
  const intervals = axes.map((axis) => {
    const index = axisIndex(axis)
    const leftMin = left.min[index]
    const leftMax = left.max[index]
    const rightMin = right.min[index]
    const rightMax = right.max[index]
    return {
      axis,
      gap: metric(Math.max(0, Math.max(leftMin - rightMax, rightMin - leftMax))),
      overlap: metric(Math.max(0, Math.min(leftMax, rightMax) - Math.max(leftMin, rightMin))),
    }
  })
  const gapDistance = Math.hypot(...intervals.map((interval) => interval.gap))
  const overlapSize = intervals.map((interval) => interval.overlap)
  const overlapVolume = overlapSize[0] * overlapSize[1] * overlapSize[2]
  const denominator = Math.min(Math.max(left.volume, EPSILON), Math.max(right.volume, EPSILON))
  const overlapFraction = clamp01(overlapVolume / denominator)
  // Keep longitudinal in the contract even when the dominant scene axis is
  // not x; this explicit read prevents accidental axis-order assumptions.
  if (!axes.includes(longitudinal)) throw new KnifeAssemblyIntrinsicMetricsError('unsupported longitudinal axis')
  return { intervals: Object.freeze(intervals), gapDistance, overlapVolume, overlapFraction }
}

function buildMaterialAdjacency(
  parts: readonly CompiledKnifePart[],
  clouds: readonly { readonly part: CompiledKnifePart; readonly cloud: WorldCloud }[],
  assetBounds: Bounds,
): KnifeAssemblyIntrinsicMaterialAdjacencySummary {
  const zones = new Set(parts.map((part) => part.material_zone_id))
  const candidateZonePairCount = zones.size * Math.max(zones.size - 1, 0) / 2
  const assetDiagonal = Math.max(boundsOutput(assetBounds).diagonal, EPSILON)
  const cloudByPart = new Map(clouds.map((entry) => [entry.part.part_id, entry.cloud.bounds]))
  const roleByPart = new Map(parts.map((part) => [part.part_id, part.surface_role]))
  const zoneById = new Map(parts.map((part) => [part.material_zone_id, materialChannels(part)]))
  const groups = new Map<string, {
    leftZone: string
    rightZone: string
    leftParts: Set<string>
    rightParts: Set<string>
    pairs: BoundsPair[]
  }>()
  for (let leftIndex = 0; leftIndex < parts.length; leftIndex += 1) {
    for (let rightIndex = leftIndex + 1; rightIndex < parts.length; rightIndex += 1) {
      const leftPart = parts[leftIndex]
      const rightPart = parts[rightIndex]
      if (leftPart.material_zone_id === rightPart.material_zone_id) continue
      const leftZone = leftPart.material_zone_id < rightPart.material_zone_id ? leftPart.material_zone_id : rightPart.material_zone_id
      const rightZone = leftPart.material_zone_id < rightPart.material_zone_id ? rightPart.material_zone_id : leftPart.material_zone_id
      const key = `${leftZone}|${rightZone}`
      const pair = boundsPair(
        boundsOutput(cloudByPart.get(leftPart.part_id)!),
        boundsOutput(cloudByPart.get(rightPart.part_id)!),
        'x',
        assetDiagonal,
      )
      const semantic = semanticAdjacentRole(roleByPart.get(leftPart.part_id)!, roleByPart.get(rightPart.part_id)!)
      if (!semantic && pair.gapDistance > assetDiagonal * ADJACENCY_TOLERANCE_FRACTION + EPSILON) continue
      const group = groups.get(key) ?? {
        leftZone,
        rightZone,
        leftParts: new Set<string>(),
        rightParts: new Set<string>(),
        pairs: [],
      }
      if (leftPart.material_zone_id === leftZone) group.leftParts.add(leftPart.part_id)
      else group.rightParts.add(leftPart.part_id)
      if (rightPart.material_zone_id === leftZone) group.leftParts.add(rightPart.part_id)
      else group.rightParts.add(rightPart.part_id)
      group.pairs.push(pair)
      groups.set(key, group)
    }
  }
  const entries = [...groups.values()].sort((left, right) => `${left.leftZone}|${left.rightZone}`.localeCompare(`${right.leftZone}|${right.rightZone}`)).map((group) => {
    const leftChannels = zoneById.get(group.leftZone)
    const rightChannels = zoneById.get(group.rightZone)
    if (!leftChannels || !rightChannels) throw new KnifeAssemblyIntrinsicMetricsError('material adjacency lost a zone channel binding')
    const delta = channelDelta(leftChannels, rightChannels)
    const readability = weightedReadability(delta)
    return Object.freeze({
      adjacency_id: `${group.leftZone}|${group.rightZone}@1`,
      left_material_zone_id: group.leftZone,
      right_material_zone_id: group.rightZone,
      left_part_ids: Object.freeze([...group.leftParts].sort()),
      right_part_ids: Object.freeze([...group.rightParts].sort()),
      spatial_part_pair_count: group.pairs.length,
      spatially_adjacent: group.pairs.some((pair) => pair.gapDistance <= assetDiagonal * ADJACENCY_TOLERANCE_FRACTION + EPSILON),
      bbox_gap: metric(Math.min(...group.pairs.map((pair) => pair.gapDistance))),
      bbox_overlap_fraction: metric(Math.max(...group.pairs.map((pair) => pair.overlapFraction))),
      left_channels: leftChannels,
      right_channels: rightChannels,
      channel_delta: delta,
      readability: metric(readability),
      computability: 'COMPUTED' as const,
      basis: 'material-zone-adjacency-and-weighted-channel-contrast-proxy@1',
    })
  })
  const meanReadability = entries.length > 0 ? entries.reduce((sum, entry) => sum + entry.readability, 0) / entries.length : 'NOT_COMPUTABLE'
  const adjacencyCoverage = candidateZonePairCount > 0 ? metric(entries.length / candidateZonePairCount) : 'NOT_COMPUTABLE'
  return Object.freeze({
    zone_count: zones.size,
    adjacent_zone_pair_count: entries.length,
    candidate_zone_pair_count: candidateZonePairCount,
    adjacency_coverage: adjacencyCoverage,
    mean_readability: meanReadability,
    entries: Object.freeze(entries),
    basis: 'compiled-world-aabb-neighborhood-or-semantic-chain@1',
  })
}

function materialChannels(part: CompiledKnifePart): KnifeAssemblyIntrinsicMaterialChannels {
  const zone = part.material_zone_id
  const color = part.material.color
  const baseTone = luminance(color.r, color.g, color.b)
  const controls = part.material_spec.controls
  const emissiveColor = part.material.emissive
  const emissive = clamp01(luminance(emissiveColor.r, emissiveColor.g, emissiveColor.b) * part.material.emissiveIntensity)
  return Object.freeze({
    base_tone: metric(baseTone),
    metalness: metric(clamp01(part.material.metalness)),
    roughness: metric(clamp01(part.material.roughness)),
    normal_strength: metric(clamp01(controls.curvature)),
    emissive: metric(emissive),
    wear_mask: metric(clamp01(controls.edge_wear)),
  })
}

function luminance(red: number, green: number, blue: number): number {
  return clamp01(0.2126 * red + 0.7152 * green + 0.0722 * blue)
}

function channelDelta(left: KnifeAssemblyIntrinsicMaterialChannels, right: KnifeAssemblyIntrinsicMaterialChannels): KnifeAssemblyIntrinsicMaterialChannelDelta {
  return Object.freeze({
    base_tone: metric(Math.abs(left.base_tone - right.base_tone)),
    metalness: metric(Math.abs(left.metalness - right.metalness)),
    roughness: metric(Math.abs(left.roughness - right.roughness)),
    normal_strength: metric(Math.abs(left.normal_strength - right.normal_strength)),
    emissive: metric(Math.abs(left.emissive - right.emissive)),
    wear_mask: metric(Math.abs(left.wear_mask - right.wear_mask)),
  })
}

const MATERIAL_READABILITY_WEIGHTS = Object.freeze({
  base_tone: 0.28,
  metalness: 0.18,
  roughness: 0.18,
  normal_strength: 0.12,
  emissive: 0.12,
  wear_mask: 0.12,
})

function weightedReadability(delta: KnifeAssemblyIntrinsicMaterialChannelDelta): number {
  return clamp01(
    MATERIAL_READABILITY_WEIGHTS.base_tone * delta.base_tone
    + MATERIAL_READABILITY_WEIGHTS.metalness * delta.metalness
    + MATERIAL_READABILITY_WEIGHTS.roughness * delta.roughness
    + MATERIAL_READABILITY_WEIGHTS.normal_strength * delta.normal_strength
    + MATERIAL_READABILITY_WEIGHTS.emissive * delta.emissive
    + MATERIAL_READABILITY_WEIGHTS.wear_mask * delta.wear_mask,
  )
}

function semanticAdjacentRole(left: KnifeRenderablePartRole, right: KnifeRenderablePartRole): boolean {
  const blade = (role: KnifeRenderablePartRole): boolean => role === 'blade-body' || role === 'cutting-edge'
  const feature = (role: KnifeRenderablePartRole): boolean => role === 'fastener' || role === 'gem' || role === 'relief'
  if (blade(left) && blade(right)) return true
  if ((blade(left) && right === 'guard') || (blade(right) && left === 'guard')) return true
  if ((left === 'guard' && right === 'grip') || (right === 'guard' && left === 'grip')) return true
  if ((left === 'grip' && right === 'pommel') || (right === 'grip' && left === 'pommel')) return true
  if (feature(left) || feature(right)) return true
  return false
}

function buildComplexity(
  compiled: CompiledKnifeScene,
  maxDrawCallsInput: number,
  maxTrianglesInput: number,
): KnifeAssemblyIntrinsicComplexity {
  const drawCalls = compiled.parts.length
  const vertices = compiled.parts.reduce((sum, part) => sum + part.geometry.getAttribute('position').count, 0)
  const triangles = compiled.triangle_count
  const maxDrawCalls = Math.max(maxDrawCallsInput, 1)
  const maxTriangles = Math.max(maxTrianglesInput, 1)
  const drawBudgetFraction = metric(drawCalls / maxDrawCalls)
  const triangleBudgetFraction = metric(triangles / maxTriangles)
  const drawHeadroom = metric(clamp01(1 - drawBudgetFraction))
  const triangleHeadroom = metric(clamp01(1 - triangleBudgetFraction))
  if (!Number.isFinite(maxDrawCalls) || !Number.isFinite(maxTriangles)) {
    throw new KnifeAssemblyIntrinsicMetricsError('complexity budgets are not finite')
  }
  const parts = compiled.parts.map((part) => {
    const vertexCount = part.geometry.getAttribute('position').count
    const triangleCount = triangleCountFor(part.geometry)
    return Object.freeze({
      part_id: part.part_id,
      surface_role: part.surface_role,
      material_zone_id: part.material_zone_id,
      draw_calls: 1 as const,
      vertex_count: vertexCount,
      triangle_count: triangleCount,
      triangle_budget_fraction: metric(triangleCount / maxTriangles),
      vertex_to_triangle_ratio: metric(vertexCount / Math.max(triangleCount, 1)),
    })
  })
  return Object.freeze({
    draw_calls: drawCalls,
    vertices,
    triangles,
    max_draw_calls: maxDrawCalls,
    max_triangles: maxTriangles,
    draw_call_budget_fraction: drawBudgetFraction,
    triangle_budget_fraction: triangleBudgetFraction,
    draw_call_headroom: drawHeadroom,
    triangle_headroom: triangleHeadroom,
    draw_call_efficiency: drawHeadroom,
    triangle_efficiency: triangleHeadroom,
    complexity_efficiency: metric((drawHeadroom + triangleHeadroom) * 0.5),
    triangles_per_draw_call: metric(triangles / Math.max(drawCalls, 1)),
    parts: Object.freeze(parts),
    basis: 'compiled-geometry-draw-call-and-triangle-count@1',
  })
}

function buildReadabilityProxy(
  ratios: KnifeAssemblyIntrinsicMetrics['ratios'],
  attachments: readonly KnifeAssemblyIntrinsicAttachment[],
  materialAdjacency: KnifeAssemblyIntrinsicMaterialAdjacencySummary,
  complexity: KnifeAssemblyIntrinsicComplexity,
): KnifeAssemblyIntrinsicReadabilityProxy {
  const ratioScores = Object.values(ratios).map((ratio) => ratio.prior_score).filter((value): value is number => typeof value === 'number')
  const attachmentScores = attachments.map((attachment) => attachment.bbox_continuity)
  const ratioPrior = ratioScores.length > 0 ? metric(mean(ratioScores)) : 'NOT_COMPUTABLE'
  const attachmentContinuity = attachmentScores.length > 0 ? metric(mean(attachmentScores)) : 'NOT_COMPUTABLE'
  const materialReadability = materialAdjacency.mean_readability
  const computable = [ratioPrior, attachmentContinuity, materialReadability].filter((value): value is number => typeof value === 'number')
  const combined = computable.length > 0 ? metric(mean([...computable, complexity.complexity_efficiency])) : 'NOT_COMPUTABLE'
  return Object.freeze({
    ratio_prior_score: ratioPrior,
    attachment_continuity: attachmentContinuity,
    material_zone_readability: materialReadability,
    complexity_efficiency: complexity.complexity_efficiency,
    combined,
    basis: 'design-prior-ratio-attachment-material-complexity-composite@1',
  })
}

function buildMetricMaps(
  ratios: KnifeAssemblyIntrinsicMetrics['ratios'],
  attachments: readonly KnifeAssemblyIntrinsicAttachment[],
  materialAdjacency: KnifeAssemblyIntrinsicMaterialAdjacencySummary,
  complexity: KnifeAssemblyIntrinsicComplexity,
): {
  readonly metric_values: Readonly<Record<string, KnifeAssemblyIntrinsicMetricValue>>
  readonly metric_details: Readonly<Record<string, KnifeAssemblyIntrinsicMetricDetail>>
} {
  const values: Record<string, KnifeAssemblyIntrinsicMetricValue> = {}
  const details: Record<string, KnifeAssemblyIntrinsicMetricDetail> = {}
  const put = (name: string, value: KnifeAssemblyIntrinsicMetricValue, direction: KnifeAssemblyIntrinsicMetricDirection, basis: string): void => {
    values[name] = value
    details[name] = Object.freeze({ direction, computable: typeof value === 'number', classification: KNIFE_ASSEMBLY_INTRINSIC_CLASSIFICATION, basis })
  }
  put('guard-root-ratio', ratios.guard_root.value, 'maximize', ratios.guard_root.basis)
  put('guard-root-ratio-prior-score', ratios.guard_root.prior_score, 'maximize', 'knowledge-range-soft-prior@1')
  put('grip-blade-ratio', ratios.grip_blade.value, 'maximize', ratios.grip_blade.basis)
  put('grip-blade-ratio-prior-score', ratios.grip_blade.prior_score, 'maximize', 'knowledge-range-soft-prior@1')
  put('pommel-grip-ratio', ratios.pommel_grip.value, 'maximize', ratios.pommel_grip.basis)
  put('pommel-grip-ratio-prior-score', ratios.pommel_grip.prior_score, 'maximize', 'knowledge-range-soft-prior@1')
  put('fastener-grip-ratio', ratios.fastener_grip.value, 'maximize', ratios.fastener_grip.basis)
  put('gem-guard-ratio', ratios.gem_guard.value, 'maximize', ratios.gem_guard.basis)
  for (const attachment of attachments) {
    put(`attachment-gap:${attachment.relation}`, attachment.normalized_gap, 'minimize', attachment.basis)
    put(`attachment-overlap:${attachment.relation}`, attachment.bbox_overlap_fraction, 'maximize', attachment.basis)
    put(`attachment-continuity:${attachment.relation}`, attachment.bbox_continuity, 'maximize', attachment.basis)
  }
  put('material-zone-adjacency-coverage', materialAdjacency.adjacency_coverage, 'maximize', materialAdjacency.basis)
  put('material-zone-readability', materialAdjacency.mean_readability, 'maximize', 'material-zone-adjacency-and-weighted-channel-contrast-proxy@1')
  put('draw-call-budget-fraction', complexity.draw_call_budget_fraction, 'minimize', complexity.basis)
  put('draw-call-efficiency', complexity.draw_call_efficiency, 'maximize', complexity.basis)
  put('triangle-budget-fraction', complexity.triangle_budget_fraction, 'minimize', complexity.basis)
  put('triangle-efficiency', complexity.triangle_efficiency, 'maximize', complexity.basis)
  put('complexity-efficiency', complexity.complexity_efficiency, 'maximize', complexity.basis)
  return { metric_values: Object.freeze(values), metric_details: Object.freeze(details) }
}

function triangleCountFor(geometry: THREE.BufferGeometry): number {
  const index = geometry.getIndex()
  const position = geometry.getAttribute('position')
  const count = index ? index.count : position.count
  const triangles = count / 3
  if (!Number.isInteger(triangles) || triangles <= 0 || !Number.isFinite(triangles)) {
    throw new KnifeAssemblyIntrinsicMetricsError('compiled geometry has an invalid triangle count')
  }
  return triangles
}

function mean(values: readonly number[]): number {
  return values.length === 0 ? 0 : values.reduce((sum, value) => sum + value, 0) / values.length
}

function clamp01(value: number): number {
  return Math.max(0, Math.min(1, value))
}

function metric(value: number): number {
  if (!Number.isFinite(value)) throw new KnifeAssemblyIntrinsicMetricsError('derived metric is not finite')
  const rounded = Number(value.toFixed(MAX_METRIC_DECIMALS))
  return Object.is(rounded, -0) ? 0 : rounded
}

function closeVec3(left: readonly number[], right: readonly number[]): boolean {
  return left.length === 3 && right.length === 3 && left.every((value, index) => Math.abs(value - right[index]) <= EPSILON)
}

function exactKeys(
  value: Record<string, unknown>,
  required: readonly string[],
  label: string,
  optional: readonly string[] = [],
): void {
  const expected = new Set([...required, ...optional])
  const keys = Object.keys(value)
  if (keys.length !== new Set(keys).size || keys.some((key) => !expected.has(key)) || required.some((key) => !Object.prototype.hasOwnProperty.call(value, key))) {
    throw new KnifeAssemblyIntrinsicMetricsError(`${label} keys are not closed`)
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function deepFreeze<T>(value: T): T {
  if (value && typeof value === 'object' && !Object.isFrozen(value)) {
    Object.freeze(value)
    for (const child of Object.values(value as Record<string, unknown>)) deepFreeze(child)
  }
  return value
}

function fingerprintProgram(program: KnifeSceneProgram): string {
  const canonical = { ...program, canonical_sha256: '' }
  return fnv1a64(canonicalJson(canonical))
}

function canonicalJson(value: unknown): string {
  if (value === null) return 'null'
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new KnifeAssemblyIntrinsicMetricsError('canonical input contains a non-finite number')
    return metric(value).toString()
  }
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'string') return JSON.stringify(value)
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  if (isRecord(value)) {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`
  }
  throw new KnifeAssemblyIntrinsicMetricsError('canonical input contains an unsupported value')
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
