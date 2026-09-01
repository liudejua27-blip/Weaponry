import type {
  KnifeCurve,
  KnifeSceneProgram,
  KnifeSection,
  KnifeVec3,
} from './knife-scene-program.ts'

/**
 * Closed, renderer-independent morphology evidence for a KnifeSceneProgram.
 *
 * This module intentionally consumes the typed program instead of a compiled
 * Three.js scene.  It is therefore an evaluator-owned structural prior: it
 * can rank mathematical design candidates, but it cannot make a visual,
 * human, engine, commercial, or quality claim.
 */
export const KNIFE_INTRINSIC_MORPHOLOGY_SCHEMA = 'KnifeIntrinsicMorphology@1' as const
export const KNIFE_INTRINSIC_MORPHOLOGY_STATUS = 'NON_VISUAL_STRUCTURAL_PRIOR' as const
export const KNIFE_INTRINSIC_MORPHOLOGY_NORMALIZATION = 'unitless-normalized-blade-local@1' as const
export const KNIFE_INTRINSIC_MORPHOLOGY_SAMPLE_COUNT = 65 as const
export const KNIFE_INTRINSIC_MORPHOLOGY_EXTREMA_BUDGET = 2 as const

export type KnifeIntrinsicMorphologyStatus = typeof KNIFE_INTRINSIC_MORPHOLOGY_STATUS
export type KnifeIntrinsicMorphologyQualityStatus = 'NOT_RUN'
export type KnifeIntrinsicMetricDirection = 'maximize' | 'minimize'
export type KnifeIntrinsicMetricComputability = 'COMPUTED'
export type KnifeIntrinsicMetricName =
  | 'belly_dominance'
  | 'tip_convergence_rate'
  | 'spine_extrema_budget'
  | 'edge_extrema_budget'
  | 'section_order_continuity'
  | 'width_continuity'
  | 'thickness_continuity'
  | 'twist_continuity'
  | 'tip_taper'
  | 'curve_g1_proxy'

export const KNIFE_INTRINSIC_MORPHOLOGY_METRIC_NAMES = Object.freeze([
  'belly_dominance',
  'tip_convergence_rate',
  'spine_extrema_budget',
  'edge_extrema_budget',
  'section_order_continuity',
  'width_continuity',
  'thickness_continuity',
  'twist_continuity',
  'tip_taper',
  'curve_g1_proxy',
] as const satisfies readonly KnifeIntrinsicMetricName[])

export interface KnifeIntrinsicMorphologyInput {
  readonly program: KnifeSceneProgram
}

export interface KnifeIntrinsicMetricRecord {
  readonly metric: KnifeIntrinsicMetricName
  /** The evaluator's named value. Ratios/rates remain dimensionless. */
  readonly value: number
  /** A bounded [0,1] form suitable for direction-aware prior ranking. */
  readonly normalized_value: number
  readonly direction: KnifeIntrinsicMetricDirection
  readonly computability: KnifeIntrinsicMetricComputability
  readonly classification: KnifeIntrinsicMorphologyStatus
  readonly basis: string
}

export interface KnifeIntrinsicMetricVector {
  /** Bounded form of width_belly / max(width_root, epsilon). */
  readonly belly_dominance: number
  /** Bounded convergence amount, independent of the station spacing. */
  readonly tip_convergence_rate: number
  /** Remaining extrema-budget headroom, where 1 is no turn-sign change. */
  readonly spine_extrema_budget: number
  readonly edge_extrema_budget: number
  readonly section_order_continuity: number
  readonly width_continuity: number
  readonly thickness_continuity: number
  readonly twist_continuity: number
  /** Mean width/thickness taper score in [0,1]. */
  readonly tip_taper: number
  /** Mean sampled tangent smoothness for the spine and cutting edge. */
  readonly curve_g1_proxy: number
}

export interface KnifeIntrinsicRawMetricVector {
  /** width_belly / max(width_root, epsilon), in normalized visual units. */
  readonly belly_dominance_ratio: number
  /** (width_belly - width_tip) / max(u_tip - u_belly, epsilon). */
  readonly tip_convergence_rate: number
}

export interface KnifeIntrinsicAxisNormalization {
  readonly longitudinal_axis: 'x' | 'y' | 'z'
  readonly lateral_axis: 'x' | 'y' | 'z'
  readonly depth_axis: 'x' | 'y' | 'z'
  /** Combined spine/edge control-point span, before local normalization. */
  readonly longitudinal_span: number
  readonly lateral_span: number
  readonly depth_span: number
}

export interface KnifeIntrinsicCurveMorphology {
  readonly curve_id: string
  readonly basis: KnifeCurve['basis']
  readonly sample_count: number
  /** Curve length divided by the combined spine/edge length scale. */
  readonly normalized_length: number
  readonly extrema_count: number
  readonly extrema_budget: typeof KNIFE_INTRINSIC_MORPHOLOGY_EXTREMA_BUDGET
  readonly extrema_budget_utilization: number
  readonly extrema_budget_headroom: number
  readonly within_extrema_budget: boolean
  readonly g1_proxy: number
  readonly mean_tangent_jump_radians: number
  readonly max_tangent_jump_radians: number
  readonly non_degenerate_samples: boolean
}

export interface KnifeIntrinsicSectionMorphology {
  readonly section_ids: readonly string[]
  readonly roles: readonly KnifeSection['role'][]
  readonly u: readonly number[]
  readonly normalized_u_gaps: readonly number[]
  readonly normalized_half_widths: readonly number[]
  readonly normalized_thicknesses: readonly number[]
  readonly normalized_twists: readonly number[]
  readonly semantic_role_order: readonly KnifeSection['role'][]
  readonly semantic_role_order_valid: boolean
  readonly minimum_u_gap: number
  readonly width_continuity: number
  readonly thickness_continuity: number
  readonly twist_continuity: number
  readonly profile_continuity: number
  readonly belly_width_over_root_width: number
  readonly tip_width_over_belly_width: number
  readonly tip_thickness_over_belly_thickness: number
  readonly tip_thickness_over_root_thickness: number
  readonly width_taper_score: number
  readonly thickness_taper_score: number
  readonly taper_monotonicity: number
}

export interface KnifeIntrinsicMorphologyGates {
  readonly finite_values: true
  readonly independent_curve_ids: boolean
  readonly required_section_roles_present: boolean
  readonly section_u_strictly_monotonic: true
  readonly semantic_section_order: boolean
  readonly positive_section_width: true
  readonly positive_section_thickness: true
  readonly sampled_curves_non_degenerate: boolean
  readonly structural_gate_pass: boolean
}

export interface KnifeIntrinsicMorphologyReceipt {
  readonly schema_version: typeof KNIFE_INTRINSIC_MORPHOLOGY_SCHEMA
  readonly status: KnifeIntrinsicMorphologyStatus
  readonly normalization: typeof KNIFE_INTRINSIC_MORPHOLOGY_NORMALIZATION
  readonly source_fingerprint: string
  readonly sample_count: typeof KNIFE_INTRINSIC_MORPHOLOGY_SAMPLE_COUNT
  readonly axes: KnifeIntrinsicAxisNormalization
  readonly sections: KnifeIntrinsicSectionMorphology
  readonly curves: Readonly<{
    readonly spine: KnifeIntrinsicCurveMorphology
    readonly cutting_edge: KnifeIntrinsicCurveMorphology
  }>
  readonly metrics: KnifeIntrinsicMetricVector
  readonly raw_metrics: KnifeIntrinsicRawMetricVector
  readonly metric_records: Readonly<Record<KnifeIntrinsicMetricName, KnifeIntrinsicMetricRecord>>
  readonly gates: KnifeIntrinsicMorphologyGates
  readonly renderer_invoked: false
  readonly quality_status: KnifeIntrinsicMorphologyQualityStatus
  readonly deterministic_fingerprint: string
}

export type KnifeIntrinsicMorphologyErrorCode =
  | 'INVALID_INPUT'
  | 'INVALID_PROGRAM'
  | 'INVALID_CURVE'
  | 'INVALID_SECTION'

export class KnifeIntrinsicMorphologyError extends Error {
  readonly code: KnifeIntrinsicMorphologyErrorCode

  constructor(code: KnifeIntrinsicMorphologyErrorCode, message: string) {
    super(`${code}: ${message}`)
    this.name = 'KnifeIntrinsicMorphologyError'
    this.code = code
  }
}

interface Vec3 {
  readonly x: number
  readonly y: number
  readonly z: number
}

type AxisIndex = 0 | 1 | 2

interface SampledCurve {
  readonly curve: KnifeCurve
  readonly points: readonly Vec3[]
  readonly non_degenerate: boolean
  readonly length: number
}

const ID_PATTERN = /^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/
const EPSILON = 1e-9
const SIGN_EPSILON = 1e-8
const MAX_INPUT_ABS = 1e6
const REQUIRED_ROLES = ['root', 'shoulder', 'belly', 'tip'] as const
const ALLOWED_SECTION_ROLES = new Set<KnifeSection['role']>([
  'root',
  'shoulder',
  'belly',
  'tip',
  'intermediate',
])
const ALLOWED_FAMILIES = new Set(['kukri', 'tanto', 'karambit', 'bayonet', 'machete', 'original-knife'])
const ALLOWED_BASES = new Set(['authorized-reference-inspired', 'original-design', 'img2threejs-compatible-import'])
const ALLOWED_PART_ROLES = new Set(['blade-body', 'cutting-edge', 'guard', 'grip', 'pommel', 'fastener', 'gem', 'relief', 'helper'])
const ALLOWED_SOURCE_CLASSES = new Set(['observed', 'inferred', 'design-prior', 'original-choice'])
const ALLOWED_SURFACE_ROLES = new Set(['blade-body', 'cutting-edge', 'spine', 'root-transition', 'ricasso', 'fuller'])

/**
 * Evaluate the fixed, no-render morphology prior for one typed knife program.
 * All reported values are computed here; callers cannot provide metric values.
 */
export function measureKnifeIntrinsicMorphology(program: KnifeSceneProgram): KnifeIntrinsicMorphologyReceipt
export function measureKnifeIntrinsicMorphology(input: KnifeIntrinsicMorphologyInput): KnifeIntrinsicMorphologyReceipt
export function measureKnifeIntrinsicMorphology(
  first: KnifeSceneProgram | KnifeIntrinsicMorphologyInput,
): KnifeIntrinsicMorphologyReceipt {
  const program = resolveProgram(first)
  validateProgramShape(program)

  const blade = program.blade_surface
  const points = [...blade.spine_curve.control_points, ...blade.cutting_edge_curve.control_points]
    .map(toVec3)
  const axes = deriveAxes(points)
  const spine = sampleCurve(blade.spine_curve)
  const edge = sampleCurve(blade.cutting_edge_curve)
  const sections = buildSectionMorphology(blade.sections)
  const spineMetrics = buildCurveMorphology(spine, axes, points)
  const edgeMetrics = buildCurveMorphology(edge, axes, points)

  const root = sectionForRole(blade.sections, 'root')
  const belly = sectionForRole(blade.sections, 'belly')
  const tip = sectionForRole(blade.sections, 'tip')
  const bellyDominance = belly.half_width / Math.max(root.half_width, EPSILON)
  const tipConvergenceRate = (belly.half_width - tip.half_width)
    / Math.max(tip.u - belly.u, EPSILON)
  const tipConvergenceNormalized = clamp(
    (belly.half_width - tip.half_width) / Math.max(belly.half_width, EPSILON),
    0,
    1,
  )
  const tipTaper = clamp(
    0.5 * (sections.width_taper_score + sections.thickness_taper_score),
    0,
    1,
  )
  const curveG1Proxy = roundMetric(0.5 * (spineMetrics.g1_proxy + edgeMetrics.g1_proxy))
  const bellyDominanceNormalized = clamp(bellyDominance / (1 + bellyDominance), 0, 1)

  const metrics: KnifeIntrinsicMetricVector = Object.freeze({
    belly_dominance: roundMetric(bellyDominanceNormalized),
    tip_convergence_rate: roundMetric(tipConvergenceNormalized),
    spine_extrema_budget: roundMetric(spineMetrics.extrema_budget_headroom),
    edge_extrema_budget: roundMetric(edgeMetrics.extrema_budget_headroom),
    section_order_continuity: roundMetric(sections.semantic_role_order_valid ? 1 : 0),
    width_continuity: sections.width_continuity,
    thickness_continuity: sections.thickness_continuity,
    twist_continuity: sections.twist_continuity,
    tip_taper: roundMetric(tipTaper),
    curve_g1_proxy: curveG1Proxy,
  })
  const rawMetrics: KnifeIntrinsicRawMetricVector = Object.freeze({
    belly_dominance_ratio: roundMetric(bellyDominance),
    tip_convergence_rate: roundMetric(tipConvergenceRate),
  })

  const metricRecords = buildMetricRecords(metrics)
  const gates: KnifeIntrinsicMorphologyGates = Object.freeze({
    finite_values: true,
    independent_curve_ids: blade.spine_curve.curve_id !== blade.cutting_edge_curve.curve_id,
    required_section_roles_present: REQUIRED_ROLES.every((role) => blade.sections.some((section) => section.role === role)),
    section_u_strictly_monotonic: true,
    semantic_section_order: sections.semantic_role_order_valid,
    positive_section_width: true,
    positive_section_thickness: true,
    sampled_curves_non_degenerate: spine.non_degenerate && edge.non_degenerate,
    structural_gate_pass: spine.non_degenerate
      && edge.non_degenerate
      && sections.semantic_role_order_valid,
  })

  const sourceFingerprint = stableFingerprint(canonicalJson({ ...program, canonical_sha256: '' }))
  const draft = {
    schema_version: KNIFE_INTRINSIC_MORPHOLOGY_SCHEMA,
    status: KNIFE_INTRINSIC_MORPHOLOGY_STATUS,
    normalization: KNIFE_INTRINSIC_MORPHOLOGY_NORMALIZATION,
    source_fingerprint: sourceFingerprint,
    sample_count: KNIFE_INTRINSIC_MORPHOLOGY_SAMPLE_COUNT,
    axes,
    sections,
    curves: {
      spine: spineMetrics,
      cutting_edge: edgeMetrics,
    },
    metrics,
    raw_metrics: rawMetrics,
    metric_records: metricRecords,
    gates,
    renderer_invoked: false as const,
    quality_status: 'NOT_RUN' as const,
    deterministic_fingerprint: '',
  }
  const receipt = {
    ...draft,
    deterministic_fingerprint: stableFingerprint(canonicalJson(draft)),
  }
  assertFiniteReceipt(receipt)
  return deepFreeze(receipt)
}

/** Explicit evaluator alias for callers that use an evaluate verb. */
export const evaluateKnifeIntrinsicMorphology = measureKnifeIntrinsicMorphology
/** Short metric-oriented alias retained for focused callers. */
export const measureKnifeIntrinsicMetrics = measureKnifeIntrinsicMorphology

/** Validate the closed input boundary without invoking sampling or rendering. */
export function validateKnifeIntrinsicMorphologyInput(
  input: KnifeSceneProgram | KnifeIntrinsicMorphologyInput,
): void {
  validateProgramShape(resolveProgram(input))
}

function resolveProgram(first: KnifeSceneProgram | KnifeIntrinsicMorphologyInput): KnifeSceneProgram {
  if (!isRecord(first)) invalid('input must be a KnifeSceneProgram or { program }')
  const record = first as unknown as Record<string, unknown>
  if (Object.prototype.hasOwnProperty.call(record, 'program')) {
    assertExactKeys(record, ['program'], 'input')
    if (!isRecord(record.program)) invalid('input.program must be an object')
    return record.program as KnifeSceneProgram
  }
  return first as KnifeSceneProgram
}

function validateProgramShape(program: KnifeSceneProgram): void {
  if (!isRecord(program)) invalid('program must be an object')
  const programRecord = program as unknown as Record<string, unknown>
  assertExactKeys(programRecord, [
    'schema_version',
    'asset_id',
    'family',
    'design_basis',
    'coordinate_convention',
    'blade_surface',
    'parts',
    'material_zones',
    'presentation',
    'budgets',
    'unknowns',
    'canonical_sha256',
  ], 'program', ['source_envelope', 'assembly'])
  if (program.schema_version !== 'KnifeSceneProgram@1') invalid('program schema_version drifted')
  if (!isStableId(program.asset_id)) invalid('program.asset_id is not a bounded stable ID')
  if (!ALLOWED_FAMILIES.has(program.family)) invalid('program.family is unsupported')
  if (!ALLOWED_BASES.has(program.design_basis)) invalid('program.design_basis is unsupported')
  if (program.coordinate_convention !== 'weapon-front-z-up-right-handed@1') invalid('program coordinate convention drifted')
  if (typeof program.canonical_sha256 !== 'string' || (program.canonical_sha256 !== '' && !/^[a-f0-9]{64}$/.test(program.canonical_sha256))) {
    invalid('program.canonical_sha256 is invalid')
  }

  const blade = program.blade_surface
  if (!isRecord(blade)) invalid('program.blade_surface must be an object')
  assertExactKeys(blade as unknown as Record<string, unknown>, ['spine_curve', 'cutting_edge_curve', 'sections', 'surface_roles'], 'program.blade_surface')
  validateCurve(blade.spine_curve, 'program.blade_surface.spine_curve')
  validateCurve(blade.cutting_edge_curve, 'program.blade_surface.cutting_edge_curve')
  if (blade.spine_curve.curve_id === blade.cutting_edge_curve.curve_id) {
    throw new KnifeIntrinsicMorphologyError('INVALID_CURVE', 'spine and cutting-edge curve IDs must be independent')
  }
  if (!Array.isArray(blade.surface_roles) || blade.surface_roles.length < 2 || blade.surface_roles.some((role) => typeof role !== 'string' || !ALLOWED_SURFACE_ROLES.has(role))) {
    invalid('blade surface roles are invalid')
  }
  if (!Array.isArray(blade.sections) || blade.sections.length < 4 || blade.sections.length > 32) {
    throw new KnifeIntrinsicMorphologyError('INVALID_SECTION', 'blade sections must contain 4 to 32 entries')
  }

  const ids = new Set<string>()
  let previousU = -1
  for (let index = 0; index < blade.sections.length; index += 1) {
    const section = blade.sections[index]
    validateSection(section, `program.blade_surface.sections[${index}]`)
    if (ids.has(section.section_id)) throw new KnifeIntrinsicMorphologyError('INVALID_SECTION', `duplicate section ID ${section.section_id}`)
    ids.add(section.section_id)
    if (section.u <= previousU) throw new KnifeIntrinsicMorphologyError('INVALID_SECTION', 'section u values must be strictly increasing')
    previousU = section.u
  }
  const first = blade.sections[0]
  const last = blade.sections[blade.sections.length - 1]
  if (first.u !== 0 || first.role !== 'root' || last.u !== 1 || last.role !== 'tip') {
    throw new KnifeIntrinsicMorphologyError('INVALID_SECTION', 'sections must start at root u=0 and end at tip u=1')
  }
  for (const role of REQUIRED_ROLES) {
    if (blade.sections.filter((section) => section.role === role).length !== 1) {
      throw new KnifeIntrinsicMorphologyError('INVALID_SECTION', `section role ${role} must occur exactly once`)
    }
  }
  validateParts(program.parts)
  validateMaterialZones(program.material_zones)
  validatePresentation(program.presentation)
  validateBudgets(program.budgets)
  validateUnknowns(program.unknowns)
  if (program.source_envelope !== undefined && !isRecord(program.source_envelope)) invalid('program.source_envelope must be an object when present')
  if (program.assembly !== undefined && !isRecord(program.assembly)) invalid('program.assembly must be an object when present')
  // The intrinsic evaluator does not inspect assembly/material/render state,
  // but it still rejects executable/non-JSON values at fingerprint time.
}

function validateParts(value: unknown): void {
  if (!Array.isArray(value) || value.length < 2 || value.length > 64) invalid('program.parts count is outside [2,64]')
  const ids = new Set<string>()
  for (let index = 0; index < value.length; index += 1) {
    const part = value[index]
    const label = `program.parts[${index}]`
    if (!isRecord(part)) invalid(`${label} must be an object`)
    assertExactKeys(part, ['part_id', 'role', 'source_class', 'material_zone_id', 'frozen'], label)
    if (!isStableId(part.part_id) || ids.has(part.part_id)) invalid(`${label}.part_id is invalid or duplicated`)
    if (typeof part.role !== 'string' || !ALLOWED_PART_ROLES.has(part.role)) invalid(`${label}.role is unsupported`)
    if (typeof part.source_class !== 'string' || !ALLOWED_SOURCE_CLASSES.has(part.source_class)) invalid(`${label}.source_class is unsupported`)
    if (!isStableId(part.material_zone_id) || typeof part.frozen !== 'boolean') invalid(`${label} has invalid material binding or frozen flag`)
    ids.add(part.part_id)
  }
}

function validateMaterialZones(value: unknown): void {
  if (!Array.isArray(value) || value.length < 1 || value.length > 32) invalid('program.material_zones count is outside [1,32]')
  const ids = new Set<string>()
  for (let index = 0; index < value.length; index += 1) {
    const zone = value[index]
    const label = `program.material_zones[${index}]`
    if (!isRecord(zone)) invalid(`${label} must be an object`)
    assertExactKeys(zone, ['material_zone_id', 'model', 'base_color', 'metalness', 'roughness'], label)
    if (!isStableId(zone.material_zone_id) || ids.has(zone.material_zone_id)) invalid(`${label}.material_zone_id is invalid or duplicated`)
    if (zone.model !== 'mesh-standard-layered@1' || typeof zone.base_color !== 'string' || !/^#[0-9a-f]{6}$/i.test(zone.base_color)) invalid(`${label} material model or base color is invalid`)
    if (!isBoundedFiniteNumber(zone.metalness) || zone.metalness < 0 || zone.metalness > 1 || !isBoundedFiniteNumber(zone.roughness) || zone.roughness < 0 || zone.roughness > 1) invalid(`${label} PBR scalars are invalid`)
    ids.add(zone.material_zone_id)
  }
}

function validatePresentation(value: unknown): void {
  if (!isRecord(value)) invalid('program.presentation must be an object')
  assertExactKeys(value, ['camera_set', 'renderer', 'aovs'], 'program.presentation')
  if (value.camera_set !== 'knife-fixed-eight-view@1' || value.renderer !== 'threejs-browser-authority@1') invalid('program.presentation identity drifted')
  if (!Array.isArray(value.aovs) || value.aovs.length < 6 || value.aovs.some((aov) => typeof aov !== 'string')) invalid('program.presentation.aovs is invalid')
}

function validateBudgets(value: unknown): void {
  if (!isRecord(value)) invalid('program.budgets must be an object')
  assertExactKeys(value, ['max_triangles', 'max_draw_calls', 'max_texture_bytes'], 'program.budgets')
  if (!Number.isInteger(value.max_triangles) || value.max_triangles < 64 || value.max_triangles > 200000
    || !Number.isInteger(value.max_draw_calls) || value.max_draw_calls < 1 || value.max_draw_calls > 128
    || !Number.isInteger(value.max_texture_bytes) || value.max_texture_bytes < 0 || value.max_texture_bytes > 268435456) {
    invalid('program.budgets values are invalid')
  }
}

function validateUnknowns(value: unknown): void {
  if (!Array.isArray(value) || value.length > 32 || value.some((unknown) => typeof unknown !== 'string' || unknown.length > 120)) invalid('program.unknowns is invalid')
}

function validateCurve(value: unknown, label: string): asserts value is KnifeCurve {
  if (!isRecord(value)) throw new KnifeIntrinsicMorphologyError('INVALID_CURVE', `${label} must be an object`)
  assertExactKeys(value, ['curve_id', 'basis', 'control_points'], label)
  if (!isStableId(value.curve_id)) throw new KnifeIntrinsicMorphologyError('INVALID_CURVE', `${label}.curve_id is invalid`)
  if (value.basis !== 'bezier' && value.basis !== 'nurbs-like') throw new KnifeIntrinsicMorphologyError('INVALID_CURVE', `${label}.basis is unsupported`)
  if (!Array.isArray(value.control_points) || value.control_points.length < 4 || value.control_points.length > 64) {
    throw new KnifeIntrinsicMorphologyError('INVALID_CURVE', `${label}.control_points must contain 4 to 64 points`)
  }
  for (let index = 0; index < value.control_points.length; index += 1) {
    const point = value.control_points[index]
    if (!Array.isArray(point) || point.length !== 3 || point.some((coordinate) => !isBoundedFiniteNumber(coordinate))) {
      throw new KnifeIntrinsicMorphologyError('INVALID_CURVE', `${label}.control_points[${index}] is not a finite normalized point`)
    }
  }
}

function validateSection(value: unknown, label: string): asserts value is KnifeSection {
  if (!isRecord(value)) throw new KnifeIntrinsicMorphologyError('INVALID_SECTION', `${label} must be an object`)
  assertExactKeys(value, [
    'section_id',
    'role',
    'u',
    'half_width',
    'thickness',
    'edge_offset',
    'spine_offset',
    'asymmetry',
    'twist',
  ], label)
  if (!isStableId(value.section_id)) throw new KnifeIntrinsicMorphologyError('INVALID_SECTION', `${label}.section_id is invalid`)
  if (typeof value.role !== 'string' || !ALLOWED_SECTION_ROLES.has(value.role as KnifeSection['role'])) {
    throw new KnifeIntrinsicMorphologyError('INVALID_SECTION', `${label}.role is unsupported`)
  }
  if (!isBoundedFiniteNumber(value.u) || value.u < 0 || value.u > 1) throw new KnifeIntrinsicMorphologyError('INVALID_SECTION', `${label}.u is outside [0,1]`)
  if (!isBoundedFiniteNumber(value.half_width) || value.half_width <= 0) throw new KnifeIntrinsicMorphologyError('INVALID_SECTION', `${label}.half_width must be positive and finite`)
  if (!isBoundedFiniteNumber(value.thickness) || value.thickness <= 0) throw new KnifeIntrinsicMorphologyError('INVALID_SECTION', `${label}.thickness must be positive and finite`)
  for (const field of ['edge_offset', 'spine_offset', 'asymmetry', 'twist'] as const) {
    if (!isBoundedFiniteNumber(value[field])) throw new KnifeIntrinsicMorphologyError('INVALID_SECTION', `${label}.${field} must be finite`)
  }
}

function deriveAxes(points: readonly Vec3[]): KnifeIntrinsicAxisNormalization {
  const spans = [0, 1, 2].map((axis) => {
    const values = points.map((point) => pointAt(point, axis as AxisIndex))
    return Math.max(...values) - Math.min(...values)
  }) as [number, number, number]
  const longitudinal = maxSpanAxis(spans)
  const remaining = ([0, 1, 2] as AxisIndex[]).filter((axis) => axis !== longitudinal)
  const firstRemaining = remaining[0]!
  const secondRemaining = remaining[1]!
  const lateral = spans[firstRemaining] >= spans[secondRemaining] ? firstRemaining : secondRemaining
  const depth = remaining.find((axis) => axis !== lateral)!
  return Object.freeze({
    longitudinal_axis: axisName(longitudinal),
    lateral_axis: axisName(lateral),
    depth_axis: axisName(depth),
    longitudinal_span: roundMetric(spans[longitudinal]),
    lateral_span: roundMetric(spans[lateral]),
    depth_span: roundMetric(spans[depth]),
  })
}

function buildSectionMorphology(sections: readonly KnifeSection[]): KnifeIntrinsicSectionMorphology {
  const roles = sections.map((section) => section.role)
  const u = sections.map((section) => roundMetric(section.u))
  const gaps = sections.slice(1).map((section, index) => roundMetric(section.u - sections[index].u))
  const maxWidth = Math.max(...sections.map((section) => section.half_width), EPSILON)
  const maxThickness = Math.max(...sections.map((section) => section.thickness), EPSILON)
  const maxTwist = Math.max(...sections.map((section) => Math.abs(section.twist)), EPSILON)
  const widths = sections.map((section) => roundMetric(section.half_width / maxWidth))
  const thicknesses = sections.map((section) => roundMetric(section.thickness / maxThickness))
  const twists = sections.map((section) => roundMetric(section.twist / maxTwist))
  const roleIndices = REQUIRED_ROLES.map((role) => sections.findIndex((section) => section.role === role))
  const semanticRoleOrder = Object.freeze([...sections]
    .sort((left, right) => left.u - right.u)
    .filter((section) => REQUIRED_ROLES.includes(section.role as (typeof REQUIRED_ROLES)[number]))
    .map((section) => section.role))
  const semanticRoleOrderValid = roleIndices.every((index, position) => position === 0 || index > roleIndices[position - 1])
  const widthContinuity = continuityScore(widths)
  const thicknessContinuity = continuityScore(thicknesses)
  const twistContinuity = continuityScore(twists)
  const profileContinuity = roundMetric((widthContinuity + thicknessContinuity + twistContinuity) / 3)
  const root = sectionForRole(sections, 'root')
  const belly = sectionForRole(sections, 'belly')
  const tip = sectionForRole(sections, 'tip')
  const tipWidthRatio = tip.half_width / Math.max(belly.half_width, EPSILON)
  const tipThicknessRatio = tip.thickness / Math.max(belly.thickness, EPSILON)
  const rootThicknessRatio = tip.thickness / Math.max(root.thickness, EPSILON)
  const widthTaperScore = clamp(1 - tipWidthRatio, 0, 1)
  const thicknessTaperScore = clamp(1 - tipThicknessRatio, 0, 1)
  const taperMonotonicity = pairwiseNonIncreasingScore(sections.map((section) => section.half_width))
  return Object.freeze({
    section_ids: Object.freeze(sections.map((section) => section.section_id)),
    roles: Object.freeze([...roles]),
    u: Object.freeze(u),
    normalized_u_gaps: Object.freeze(gaps),
    normalized_half_widths: Object.freeze(widths),
    normalized_thicknesses: Object.freeze(thicknesses),
    normalized_twists: Object.freeze(twists),
    semantic_role_order: Object.freeze(semanticRoleOrder),
    semantic_role_order_valid: semanticRoleOrderValid,
    minimum_u_gap: roundMetric(Math.min(...gaps)),
    width_continuity: widthContinuity,
    thickness_continuity: thicknessContinuity,
    twist_continuity: twistContinuity,
    profile_continuity: profileContinuity,
    belly_width_over_root_width: roundMetric(belly.half_width / Math.max(root.half_width, EPSILON)),
    tip_width_over_belly_width: roundMetric(tipWidthRatio),
    tip_thickness_over_belly_thickness: roundMetric(tipThicknessRatio),
    tip_thickness_over_root_thickness: roundMetric(rootThicknessRatio),
    width_taper_score: roundMetric(widthTaperScore),
    thickness_taper_score: roundMetric(thicknessTaperScore),
    taper_monotonicity: taperMonotonicity,
  })
}

function buildCurveMorphology(
  sampled: SampledCurve,
  axes: KnifeIntrinsicAxisNormalization,
  allControlPoints: readonly Vec3[],
): KnifeIntrinsicCurveMorphology {
  const longitudinal = axisIndex(axes.longitudinal_axis)
  const lateral = axisIndex(axes.lateral_axis)
  const lateralValues = sampled.points.map((point) => pointAt(point, lateral))
  const lateralSpan = Math.max(Math.max(...lateralValues) - Math.min(...lateralValues), EPSILON)
  const extremaCount = countTurningSignChanges(lateralValues, lateralSpan)
  const budget = KNIFE_INTRINSIC_MORPHOLOGY_EXTREMA_BUDGET
  const tangentJumps = tangentJumpsFor(sampled.points)
  const meanTangentJump = tangentJumps.length > 0
    ? tangentJumps.reduce((sum, value) => sum + value, 0) / tangentJumps.length
    : Math.PI
  const maxTangentJump = tangentJumps.length > 0 ? Math.max(...tangentJumps) : Math.PI
  const g1Proxy = tangentJumps.length > 0 ? clamp(1 - meanTangentJump / Math.PI, 0, 1) : 0
  const combinedLongitudinalSpan = Math.max(
    Math.max(...allControlPoints.map((point) => pointAt(point, longitudinal)))
      - Math.min(...allControlPoints.map((point) => pointAt(point, longitudinal))),
    EPSILON,
  )
  return Object.freeze({
    curve_id: sampled.curve.curve_id,
    basis: sampled.curve.basis,
    sample_count: KNIFE_INTRINSIC_MORPHOLOGY_SAMPLE_COUNT,
    normalized_length: roundMetric(sampled.length / combinedLongitudinalSpan),
    extrema_count: extremaCount,
    extrema_budget: budget,
    extrema_budget_utilization: roundMetric(extremaCount / budget),
    extrema_budget_headroom: roundMetric(clamp((budget - extremaCount) / budget, 0, 1)),
    within_extrema_budget: extremaCount <= budget,
    g1_proxy: roundMetric(g1Proxy),
    mean_tangent_jump_radians: roundMetric(meanTangentJump),
    max_tangent_jump_radians: roundMetric(maxTangentJump),
    non_degenerate_samples: sampled.non_degenerate,
  })
}

function buildMetricRecords(metrics: KnifeIntrinsicMetricVector): Readonly<Record<KnifeIntrinsicMetricName, KnifeIntrinsicMetricRecord>> {
  const record = (
    metric: KnifeIntrinsicMetricName,
    value: number,
    normalizedValue: number,
    direction: KnifeIntrinsicMetricDirection,
    basis: string,
  ): KnifeIntrinsicMetricRecord => Object.freeze({
    metric,
    value,
    normalized_value: roundMetric(clamp(normalizedValue, 0, 1)),
    direction,
    computability: 'COMPUTED',
    classification: KNIFE_INTRINSIC_MORPHOLOGY_STATUS,
    basis,
  })
  return Object.freeze({
    belly_dominance: record('belly_dominance', metrics.belly_dominance, metrics.belly_dominance, 'maximize', 'section-width-ratio-root-to-belly@1'),
    tip_convergence_rate: record('tip_convergence_rate', metrics.tip_convergence_rate, metrics.tip_convergence_rate, 'maximize', 'section-width-delta-over-u-delta@1'),
    spine_extrema_budget: record('spine_extrema_budget', metrics.spine_extrema_budget, metrics.spine_extrema_budget, 'maximize', 'sampled-lateral-turn-sign-budget-headroom@1'),
    edge_extrema_budget: record('edge_extrema_budget', metrics.edge_extrema_budget, metrics.edge_extrema_budget, 'maximize', 'sampled-lateral-turn-sign-budget-headroom@1'),
    section_order_continuity: record('section_order_continuity', metrics.section_order_continuity, metrics.section_order_continuity, 'maximize', 'semantic-root-shoulder-belly-tip-order@1'),
    width_continuity: record('width_continuity', metrics.width_continuity, metrics.width_continuity, 'maximize', 'normalized-section-width-second-difference@1'),
    thickness_continuity: record('thickness_continuity', metrics.thickness_continuity, metrics.thickness_continuity, 'maximize', 'normalized-section-thickness-second-difference@1'),
    twist_continuity: record('twist_continuity', metrics.twist_continuity, metrics.twist_continuity, 'maximize', 'normalized-section-twist-second-difference@1'),
    tip_taper: record('tip_taper', metrics.tip_taper, metrics.tip_taper, 'maximize', 'tip-width-and-thickness-convergence@1'),
    curve_g1_proxy: record('curve_g1_proxy', metrics.curve_g1_proxy, metrics.curve_g1_proxy, 'maximize', 'sampled-adjacent-tangent-angle@1'),
  })
}

function sampleCurve(curve: KnifeCurve): SampledCurve {
  const points: Vec3[] = []
  for (let index = 0; index < KNIFE_INTRINSIC_MORPHOLOGY_SAMPLE_COUNT; index += 1) {
    const u = index / (KNIFE_INTRINSIC_MORPHOLOGY_SAMPLE_COUNT - 1)
    points.push(curve.basis === 'bezier'
      ? evaluateBezier(curve.control_points, u)
      : evaluateNurbsLike(curve.control_points, u))
  }
  let length = 0
  let nonDegenerate = true
  for (let index = 1; index < points.length; index += 1) {
    const segmentLength = distance(points[index - 1], points[index])
    if (!Number.isFinite(segmentLength)) throw new KnifeIntrinsicMorphologyError('INVALID_CURVE', `${curve.curve_id} sample became non-finite`)
    length += segmentLength
    if (segmentLength <= EPSILON) nonDegenerate = false
  }
  return { curve, points: Object.freeze(points), non_degenerate: nonDegenerate, length: roundMetric(length) }
}

function evaluateBezier(points: readonly KnifeVec3[], u: number): Vec3 {
  let work = points.map(toVec3)
  while (work.length > 1) {
    work = work.slice(0, -1).map((point, index) => ({
      x: lerp(point.x, work[index + 1].x, u),
      y: lerp(point.y, work[index + 1].y, u),
      z: lerp(point.z, work[index + 1].z, u),
    }))
  }
  return work[0]
}

/** Matches the compiler's closed Catmull-Rom-like basis without importing Three.js. */
function evaluateNurbsLike(points: readonly KnifeVec3[], u: number): Vec3 {
  const control = points.map(toVec3)
  const scaled = clamp(u, 0, 1) * (control.length - 1)
  const segment = Math.min(Math.floor(scaled), control.length - 2)
  const local = scaled - segment
  const p0 = control[Math.max(0, segment - 1)]
  const p1 = control[segment]
  const p2 = control[segment + 1]
  const p3 = control[Math.min(control.length - 1, segment + 2)]
  const local2 = local * local
  const local3 = local2 * local
  const interpolate = (a: number, b: number, c: number, d: number): number => 0.5 * (
    2 * b
      + (-a + c) * local
      + (2 * a - 5 * b + 4 * c - d) * local2
      + (-a + 3 * b - 3 * c + d) * local3
  )
  return { x: interpolate(p0.x, p1.x, p2.x, p3.x), y: interpolate(p0.y, p1.y, p2.y, p3.y), z: interpolate(p0.z, p1.z, p2.z, p3.z) }
}

function tangentJumpsFor(points: readonly Vec3[]): number[] {
  const jumps: number[] = []
  for (let index = 1; index < points.length - 1; index += 1) {
    const before = subtract(points[index], points[index - 1])
    const after = subtract(points[index + 1], points[index])
    const beforeLength = length(before)
    const afterLength = length(after)
    if (beforeLength <= EPSILON || afterLength <= EPSILON) continue
    const dot = clamp(dotProduct(before, after) / (beforeLength * afterLength), -1, 1)
    jumps.push(Math.acos(dot))
  }
  return jumps
}

function countTurningSignChanges(values: readonly number[], scale: number): number {
  let previousSign = 0
  let changes = 0
  for (let index = 1; index < values.length; index += 1) {
    const delta = values[index] - values[index - 1]
    const sign = Math.abs(delta) <= SIGN_EPSILON * scale ? 0 : delta > 0 ? 1 : -1
    if (sign === 0) continue
    if (previousSign !== 0 && sign !== previousSign) changes += 1
    previousSign = sign
  }
  return changes
}

function continuityScore(values: readonly number[]): number {
  if (values.length < 3) return 1
  const range = Math.max(...values) - Math.min(...values)
  const maxAbs = Math.max(...values.map((value) => Math.abs(value)))
  const scale = Math.max(range, maxAbs * 0.25, EPSILON)
  let secondDifference = 0
  for (let index = 1; index < values.length - 1; index += 1) {
    secondDifference += Math.abs(values[index + 1] - 2 * values[index] + values[index - 1])
  }
  return roundMetric(clamp(1 - secondDifference / (2 * scale * (values.length - 2)), 0, 1))
}

function pairwiseNonIncreasingScore(values: readonly number[]): number {
  if (values.length < 2) return 1
  let nonIncreasing = 0
  for (let index = 1; index < values.length; index += 1) {
    if (values[index] <= values[index - 1] + EPSILON) nonIncreasing += 1
  }
  return roundMetric(nonIncreasing / (values.length - 1))
}

function sectionForRole(sections: readonly KnifeSection[], role: KnifeSection['role']): KnifeSection {
  const section = sections.find((candidate) => candidate.role === role)
  if (!section) throw new KnifeIntrinsicMorphologyError('INVALID_SECTION', `missing section role ${role}`)
  return section
}

function assertFiniteReceipt(value: unknown): void {
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new KnifeIntrinsicMorphologyError('INVALID_PROGRAM', 'evaluator produced a non-finite metric')
    return
  }
  if (!value || typeof value !== 'object') return
  if (Array.isArray(value)) {
    for (const item of value) assertFiniteReceipt(item)
    return
  }
  for (const child of Object.values(value as Record<string, unknown>)) assertFiniteReceipt(child)
}

function toVec3(point: KnifeVec3): Vec3 {
  return { x: point[0], y: point[1], z: point[2] }
}

function pointAt(point: Vec3, axis: AxisIndex): number {
  return axis === 0 ? point.x : axis === 1 ? point.y : point.z
}

function axisName(axis: AxisIndex): 'x' | 'y' | 'z' {
  return axis === 0 ? 'x' : axis === 1 ? 'y' : 'z'
}

function axisIndex(axis: 'x' | 'y' | 'z'): AxisIndex {
  return axis === 'x' ? 0 : axis === 'y' ? 1 : 2
}

function maxSpanAxis(spans: readonly [number, number, number]): AxisIndex {
  if (spans[1] > spans[0] && spans[1] >= spans[2]) return 1
  if (spans[2] > spans[0] && spans[2] > spans[1]) return 2
  return 0
}

function subtract(left: Vec3, right: Vec3): Vec3 {
  return { x: left.x - right.x, y: left.y - right.y, z: left.z - right.z }
}

function dotProduct(left: Vec3, right: Vec3): number {
  return left.x * right.x + left.y * right.y + left.z * right.z
}

function length(value: Vec3): number {
  return Math.sqrt(dotProduct(value, value))
}

function distance(left: Vec3, right: Vec3): number {
  return length(subtract(left, right))
}

function lerp(left: number, right: number, t: number): number {
  return left + (right - left) * t
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value))
}

function roundMetric(value: number): number {
  if (!Number.isFinite(value)) throw new KnifeIntrinsicMorphologyError('INVALID_PROGRAM', 'metric is non-finite')
  const rounded = Number(value.toFixed(9))
  return Object.is(rounded, -0) ? 0 : rounded
}

function isBoundedFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && Math.abs(value) <= MAX_INPUT_ABS
}

function isStableId(value: unknown): value is string {
  return typeof value === 'string' && ID_PATTERN.test(value)
}

function isRecord(value: unknown): value is Record<string, any> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value)
}

function assertExactKeys(
  value: Record<string, unknown>,
  required: readonly string[],
  label: string,
  optional: readonly string[] = [],
): void {
  const allowed = new Set([...required, ...optional])
  const keys = Object.keys(value)
  for (const key of required) {
    if (!Object.prototype.hasOwnProperty.call(value, key)) invalid(`${label}.${key} is missing`)
  }
  for (const key of keys) {
    if (!allowed.has(key)) invalid(`${label} contains unknown field ${key}`)
  }
}

function invalid(message: string): never {
  throw new KnifeIntrinsicMorphologyError('INVALID_INPUT', message)
}

function canonicalJson(value: unknown): string {
  if (value === null) return 'null'
  if (typeof value === 'string') return JSON.stringify(value)
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new KnifeIntrinsicMorphologyError('INVALID_PROGRAM', 'canonical source contains a non-finite number')
    return Object.is(value, -0) ? '0' : JSON.stringify(value)
  }
  if (Array.isArray(value)) return `[${value.map((item) => canonicalJson(item)).join(',')}]`
  if (isRecord(value)) return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`
  throw new KnifeIntrinsicMorphologyError('INVALID_PROGRAM', 'canonical source contains undefined or executable values')
}

function stableFingerprint(value: string): string {
  let hash = 0xcbf29ce484222325n
  for (let index = 0; index < value.length; index += 1) {
    hash ^= BigInt(value.charCodeAt(index))
    hash = BigInt.asUintN(64, hash * 0x100000001b3n)
  }
  return hash.toString(16).padStart(16, '0')
}

function deepFreeze<T>(value: T): T {
  if (!value || typeof value !== 'object' || Object.isFrozen(value)) return value
  for (const child of Object.values(value as Record<string, unknown>)) deepFreeze(child)
  return Object.freeze(value)
}
