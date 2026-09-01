import {
  KNIFE_OBJECTIVE_METRICS,
  validateKnifeObjectiveLedger,
  type KnifeObjectiveLedger,
  type KnifeObjectiveMetric,
} from './knife-objective-ledger.ts'
import {
  KNIFE_OBJECTIVE_NOT_COMPUTABLE,
  type KnifeObjectiveValueV2,
} from './knife-objective-function-v2.ts'
import {
  evaluateKnifeRig,
  KNIFE_VIEW_IDS,
  type KnifeEightViewEvaluation,
} from './knife-view-evaluation.ts'
import type { CompiledKnifeScene } from './knife-scene-compiler.ts'
import type { KnifeSceneProgram } from './knife-scene-program.ts'
import {
  measureKnifePartVisibilityMetrics,
  type KnifePartVisibilityMetrics,
} from './knife-part-visibility-metrics.ts'
import {
  measureKnifeGuardFpsMetrics,
  type KnifeGuardFpsMetrics,
} from './knife-guard-fps-metrics.ts'
import { sha256Hex } from './knife-browser-capture.ts'

/**
 * Converts already-bound, renderer-independent fixed-view receipts into the
 * closed metric vector consumed by KnifeObjectiveFunction@2.
 *
 * This adapter deliberately has no reference-image input and never invents a
 * score for a missing quality measurement. Its numeric values are structural
 * observability/presentation signals only.
 */
export const KNIFE_OBJECTIVE_METRIC_ADAPTER_SCHEMA = 'WeaponryThreeJsKnifeObjectiveMetricAdapter@1' as const
export const KNIFE_OBJECTIVE_METRIC_ADAPTER_STATUS = 'MEASURED_NOT_REVIEWED' as const

export type KnifeObjectiveMetricAdapterStatus = typeof KNIFE_OBJECTIVE_METRIC_ADAPTER_STATUS
export type KnifeObjectiveMetricAdapterComputability = 'COMPUTED' | typeof KNIFE_OBJECTIVE_NOT_COMPUTABLE

export interface KnifeObjectiveMetricAdapterInput {
  /** Candidate program being measured; it is a successor, not the ledger parent. */
  readonly program: KnifeSceneProgram
  /** Exact parent/source program identity carried by the candidate plan. */
  readonly source_program_sha256: string
  readonly ledger: KnifeObjectiveLedger
  readonly compiled: CompiledKnifeScene
  /** Must be produced from this compiled scene and the same rig as both metric receipts. */
  readonly evaluation: KnifeEightViewEvaluation
  readonly visibility: KnifePartVisibilityMetrics
  readonly guard_fps: KnifeGuardFpsMetrics
}

export interface KnifeObjectiveMetricEvidence {
  readonly metric: KnifeObjectiveMetric
  readonly value: KnifeObjectiveValueV2
  readonly computability: KnifeObjectiveMetricAdapterComputability
  readonly evidence_class: 'structural-proxy' | 'visual-evidence'
  readonly basis: string
  readonly receipt_fingerprints: readonly string[]
}

export interface KnifeObjectiveMetricAdapterReceipt {
  readonly schema_version: typeof KNIFE_OBJECTIVE_METRIC_ADAPTER_SCHEMA
  readonly status: KnifeObjectiveMetricAdapterStatus
  readonly objective_metrics: readonly KnifeObjectiveMetric[]
  readonly regression_metrics: readonly KnifeObjectiveMetric[]
  readonly metrics: Readonly<Record<KnifeObjectiveMetric, KnifeObjectiveValueV2>>
  readonly metric_evidence: readonly KnifeObjectiveMetricEvidence[]
  readonly candidate_program_sha256: string
  readonly candidate_program_hash_policy: 'runtime-asserted-canonical-sha256@1' | 'browser-normalized-canonical-json@1'
  readonly source_program_sha256: string
  readonly ledger_sha256: string
  readonly source_fingerprint: string
  readonly rig_fingerprint: string
  readonly metric_receipt_fingerprints: Readonly<Record<KnifeObjectiveMetric, readonly string[]>>
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
  readonly deterministic_fingerprint: string
}

export class KnifeObjectiveMetricAdapterError extends Error {
  readonly code: KnifeObjectiveMetricAdapterErrorCode

  constructor(code: KnifeObjectiveMetricAdapterErrorCode, message: string) {
    super(`${code}: ${message}`)
    this.name = 'KnifeObjectiveMetricAdapterError'
    this.code = code
  }
}

export type KnifeObjectiveMetricAdapterErrorCode =
  | 'INVALID_INPUT'
  | 'PROGRAM_BINDING_MISMATCH'
  | 'LINEAGE_MISMATCH'
  | 'METRIC_RECEIPT_MISMATCH'

const FINGERPRINT = /^[a-f0-9]{16,128}$/i

/**
 * Measure the complete union of Ledger objective and regression metrics.
 * Only the three metrics with direct fixed-mask evidence receive numbers.
 */
export function measureKnifeObjectiveMetricValues(
  input: KnifeObjectiveMetricAdapterInput,
): KnifeObjectiveMetricAdapterReceipt {
  validateInputShape(input)
  const programSha = canonicalKnifeProgramSha256(input.program)
  if (input.ledger.program_sha256 !== input.source_program_sha256) {
    throw new KnifeObjectiveMetricAdapterError(
      'PROGRAM_BINDING_MISMATCH',
      'ledger.program_sha256 does not bind the candidate plan source program SHA-256',
    )
  }

  validateLineage(input)
  const metrics = unionLedgerMetrics(input.ledger)
  const derived = deriveComputableMetrics(input, metrics)
  const metricEvidence = metrics.map((metric) => derived.evidence[metric])
  const values = Object.fromEntries(metrics.map((metric) => [metric, derived.values[metric]])) as Readonly<Record<KnifeObjectiveMetric, KnifeObjectiveValueV2>>
  const metricReceiptFingerprints = Object.fromEntries(metrics.map((metric) => [metric, derived.evidence[metric].receipt_fingerprints])) as Readonly<Record<KnifeObjectiveMetric, readonly string[]>>
  const draft = {
    schema_version: KNIFE_OBJECTIVE_METRIC_ADAPTER_SCHEMA,
    status: KNIFE_OBJECTIVE_METRIC_ADAPTER_STATUS,
    objective_metrics: Object.freeze([...input.ledger.objective_metrics]),
    regression_metrics: Object.freeze([...input.ledger.regression_limits]),
    metrics: values,
    metric_evidence: Object.freeze(metricEvidence),
    candidate_program_sha256: programSha,
    candidate_program_hash_policy: input.program.canonical_sha256 === ''
      ? 'browser-normalized-canonical-json@1' as const
      : 'runtime-asserted-canonical-sha256@1' as const,
    source_program_sha256: input.source_program_sha256,
    ledger_sha256: input.ledger.canonical_sha256,
    source_fingerprint: input.compiled.deterministic_fingerprint,
    rig_fingerprint: input.evaluation.rig.deterministic_fingerprint,
    metric_receipt_fingerprints: metricReceiptFingerprints,
    renderer_invoked: false as const,
    quality_status: 'NOT_RUN' as const,
    deterministic_fingerprint: '',
  }
  return deepFreeze({
    ...draft,
    deterministic_fingerprint: sha256Hex(canonicalJson(draft)),
  })
}

/** Short alias for metric consumers that use an evaluate verb. */
export const evaluateKnifeObjectiveMetricValues = measureKnifeObjectiveMetricValues
export const measureKnifeObjectiveMetricAdapter = measureKnifeObjectiveMetricValues

/** Compute the canonical program SHA used by the Ledger@1 program binding. */
export function canonicalKnifeProgramSha256(program: KnifeSceneProgram): string {
  if (!isRecord(program)) invalid('program must be an object')
  // Non-empty canonical identities are Runtime-owned and may preserve source
  // numeric spelling (for example 1.0 versus 1) that JSON.parse normalizes.
  // Browser-generated candidate drafts deliberately leave the field blank;
  // only those drafts use the local normalized canonical JSON policy.
  if (program.canonical_sha256 !== '') {
    if (!/^[a-f0-9]{64}$/.test(program.canonical_sha256)) {
      throw new KnifeObjectiveMetricAdapterError('PROGRAM_BINDING_MISMATCH', 'program.canonical_sha256 is malformed')
    }
    return program.canonical_sha256
  }
  return sha256Hex(canonicalJson({ ...program, canonical_sha256: '' }))
}

function validateInputShape(input: KnifeObjectiveMetricAdapterInput): void {
  if (!isRecord(input)
    || !isRecord(input.program)
    || typeof input.source_program_sha256 !== 'string'
    || !isRecord(input.ledger)
    || !isRecord(input.compiled)
    || !isRecord(input.evaluation)
    || !isRecord(input.visibility)
    || !isRecord(input.guard_fps)) {
    invalid('adapter input must contain program, source_program_sha256, ledger, compiled, evaluation, visibility and guard_fps')
  }
  if (!/^[a-f0-9]{64}$/.test(input.source_program_sha256)) invalid('source_program_sha256 is invalid')
  try {
    validateKnifeObjectiveLedger(input.ledger, { require_canonical_sha256: true })
  } catch (error) {
    invalid(error instanceof Error ? error.message : 'ledger is invalid')
  }
}

function validateLineage(input: KnifeObjectiveMetricAdapterInput): void {
  const { compiled, evaluation, visibility, guard_fps: guardFps } = input
  try {
    if (evaluation.rig.schema_version !== 'KnifeFixedEightViewRig@1'
      || evaluation.rig.rig_id !== 'knife-fixed-eight-view@1'
      || evaluation.rig.deterministic_fingerprint !== evaluation.receipt.rig_fingerprint
      || evaluation.receipt.source_fingerprint !== compiled.deterministic_fingerprint
      || evaluation.receipt.view_ids.join('|') !== KNIFE_VIEW_IDS.join('|')
      || evaluation.receipt.renderer_invoked !== false
      || evaluation.receipt.quality_status !== 'NOT_RUN'
      || evaluation.views.length !== KNIFE_VIEW_IDS.length) {
      throw new Error('eight-view evaluation is not bound to the complete fixed rig and compiled source')
    }
    for (let index = 0; index < evaluation.views.length; index += 1) {
      const view = evaluation.views[index]
      if (view.view_id !== KNIFE_VIEW_IDS[index]
        || view.receipt.view_id !== view.view_id
        || view.receipt.rig_fingerprint !== evaluation.rig.deterministic_fingerprint
        || view.receipt.source_fingerprint !== compiled.deterministic_fingerprint
        || view.receipt.renderer_invoked !== false
        || view.receipt.quality_status !== 'NOT_RUN'
        || view.mask.width !== evaluation.rig.frame_width
        || view.mask.height !== evaluation.rig.frame_height
        || view.mask.receipt.view_id !== view.view_id
        || view.mask.receipt.projection_fingerprint !== view.projection.receipt.deterministic_fingerprint
        || view.mask.receipt.frame_width !== evaluation.rig.frame_width
        || view.mask.receipt.frame_height !== evaluation.rig.frame_height
        || view.mask.receipt.renderer_invoked !== false
        || view.mask.receipt.quality_status !== 'NOT_RUN'
        || view.receipt.mask_fingerprint !== view.mask.receipt.deterministic_fingerprint
        || view.receipt.projection_fingerprint !== view.projection.receipt.deterministic_fingerprint) {
        throw new Error(`evaluation view ${view.view_id} is not bound to the same source/rig`)
      }
    }
    if (visibility.source_fingerprint !== compiled.deterministic_fingerprint
      || visibility.rig_fingerprint !== evaluation.rig.deterministic_fingerprint
      || visibility.view_ids.join('|') !== KNIFE_VIEW_IDS.join('|')
      || visibility.frame_width !== evaluation.rig.frame_width
      || visibility.frame_height !== evaluation.rig.frame_height
      || visibility.renderer_invoked !== false
      || visibility.quality_status !== 'NOT_RUN'
      || visibility.status !== 'MEASURED_NOT_REVIEWED') {
      throw new Error('Part visibility receipt is not bound to the same source/rig')
    }
    if (guardFps.source_fingerprint !== compiled.deterministic_fingerprint
      || guardFps.rig_fingerprint !== evaluation.rig.deterministic_fingerprint
      || guardFps.view_ids.join('|') !== KNIFE_VIEW_IDS.join('|')
      || guardFps.frame_width !== evaluation.rig.frame_width
      || guardFps.frame_height !== evaluation.rig.frame_height
      || guardFps.renderer_invoked !== false
      || guardFps.quality_status !== 'NOT_RUN'
      || guardFps.status !== 'MEASURED_NOT_REVIEWED') {
      throw new Error('guard/FPS receipt is not bound to the same source/rig')
    }
    if (!isFingerprint(compiled.deterministic_fingerprint)
      || !isFingerprint(evaluation.receipt.deterministic_fingerprint)
      || !isFingerprint(visibility.deterministic_fingerprint)
      || !isFingerprint(guardFps.deterministic_fingerprint)) {
      throw new Error('source or metric receipt fingerprint is invalid')
    }

    // Recompute from the compiled source and supplied rig. A caller-supplied
    // numeric receipt is not accepted merely because its fields look valid.
    const expectedEvaluation = evaluateKnifeRig(compiled, evaluation.rig)
    const expectedVisibility = measureKnifePartVisibilityMetrics(compiled, evaluation.rig)
    const expectedGuardFps = measureKnifeGuardFpsMetrics(compiled, evaluation.rig)
    if (expectedEvaluation.receipt.deterministic_fingerprint !== evaluation.receipt.deterministic_fingerprint) {
      throw new Error('eight-view evaluation receipt does not match a fresh deterministic evaluation')
    }
    for (let index = 0; index < expectedEvaluation.views.length; index += 1) {
      const expectedView = expectedEvaluation.views[index]
      const suppliedView = evaluation.views[index]
      if (expectedView.receipt.projection_fingerprint !== suppliedView.receipt.projection_fingerprint
        || expectedView.receipt.mask_fingerprint !== suppliedView.receipt.mask_fingerprint
        || expectedView.mask.receipt.deterministic_fingerprint !== suppliedView.mask.receipt.deterministic_fingerprint) {
        throw new Error(`evaluation view ${expectedView.view_id} does not match a fresh deterministic evaluation`)
      }
      if (!sameTypedArray(expectedView.mask.pixels, suppliedView.mask.pixels)
        || !sameTypedArray(expectedView.mask.part_indices, suppliedView.mask.part_indices)
        || !sameTypedArray(expectedView.mask.material_indices, suppliedView.mask.material_indices)
        || !sameTypedArray(expectedView.mask.depth, suppliedView.mask.depth)) {
        throw new Error(`evaluation mask ${expectedView.view_id} does not match a fresh deterministic evaluation`)
      }
    }
    if (expectedVisibility.deterministic_fingerprint !== visibility.deterministic_fingerprint) {
      throw new Error('Part visibility receipt does not match a fresh deterministic measurement')
    }
    if (expectedGuardFps.deterministic_fingerprint !== guardFps.deterministic_fingerprint) {
      throw new Error('guard/FPS receipt does not match a fresh deterministic measurement')
    }
  } catch (error) {
    throw new KnifeObjectiveMetricAdapterError(
      error instanceof KnifeObjectiveMetricAdapterError ? error.code : 'LINEAGE_MISMATCH',
      error instanceof Error ? error.message : String(error),
    )
  }
}

function deriveComputableMetrics(
  input: KnifeObjectiveMetricAdapterInput,
  metrics: readonly KnifeObjectiveMetric[],
): {
  readonly values: Readonly<Record<KnifeObjectiveMetric, KnifeObjectiveValueV2>>
  readonly evidence: Readonly<Record<KnifeObjectiveMetric, KnifeObjectiveMetricEvidence>>
} {
  const values = {} as Record<KnifeObjectiveMetric, KnifeObjectiveValueV2>
  const evidence = {} as Record<KnifeObjectiveMetric, KnifeObjectiveMetricEvidence>
  const partCoverage = computePartIdCoverage(input)
  const materialCoverage = computeMaterialIdCoverage(input)
  const fpsOccupancy = computeFpsOccupancy(input)
  for (const metric of metrics) {
    const computed = metric === 'part-id-coverage'
      ? partCoverage
      : metric === 'material-id-coverage'
        ? materialCoverage
        : metric === 'fps-occupancy'
          ? fpsOccupancy
          : unavailableMetric(metric, input)
    values[metric] = computed.value
    evidence[metric] = {
      metric,
      value: computed.value,
      computability: computed.value === KNIFE_OBJECTIVE_NOT_COMPUTABLE ? KNIFE_OBJECTIVE_NOT_COMPUTABLE : 'COMPUTED',
      evidence_class: computed.evidence_class,
      basis: computed.basis,
      receipt_fingerprints: Object.freeze([...computed.receipt_fingerprints]),
    }
  }
  return { values, evidence }
}

function computePartIdCoverage(input: KnifeObjectiveMetricAdapterInput): ComputedMetric {
  const expectedPartIds = input.compiled.parts.map((part) => part.part_id)
  const observedPartIndexes = new Set<number>()
  let coveredPixelCount = 0
  for (const view of input.evaluation.views) {
    for (let index = 0; index < view.mask.pixels.length; index += 1) {
      if (view.mask.pixels[index] === 0) continue
      coveredPixelCount += 1
      const partIndex = view.mask.part_indices[index]
      if (partIndex >= expectedPartIds.length) return notComputable('part-id-coverage requires valid per-pixel Part-ID ownership', input)
      observedPartIndexes.add(partIndex)
    }
  }
  if (expectedPartIds.length === 0 || coveredPixelCount === 0) {
    return notComputable('part-id-coverage has no observed fixed-view pixels', input)
  }
  return computed(
    observedPartIndexes.size / expectedPartIds.length,
    'eight-view-depth-resolved-part-id-union@1',
    [input.evaluation.receipt.deterministic_fingerprint, input.visibility.deterministic_fingerprint],
  )
}

function computeMaterialIdCoverage(input: KnifeObjectiveMetricAdapterInput): ComputedMetric {
  const declaredZoneIds = new Set(input.program.material_zones.map((zone) => zone.material_zone_id))
  const compiledZoneIds = [...new Set(input.compiled.parts.map((part) => part.material_zone_id))].sort()
  const observedMaterialIndexes = new Set<number>()
  let coveredPixelCount = 0
  for (const view of input.evaluation.views) {
    if (view.mask.material_indices.length !== view.mask.pixels.length) {
      return notComputable('material-id-coverage requires a material index for every fixed-view pixel', input)
    }
    for (let index = 0; index < view.mask.pixels.length; index += 1) {
      if (view.mask.pixels[index] === 0) continue
      coveredPixelCount += 1
      const materialIndex = view.mask.material_indices[index]
      if (materialIndex >= compiledZoneIds.length) {
        return notComputable('material-id-coverage contains an out-of-range Material-ID', input)
      }
      observedMaterialIndexes.add(materialIndex)
    }
  }
  if (declaredZoneIds.size === 0 || coveredPixelCount === 0 || compiledZoneIds.some((id) => !declaredZoneIds.has(id))) {
    return notComputable('material-id-coverage has incomplete declared or observed MaterialZone ownership', input)
  }
  const observedZoneIds = new Set([...observedMaterialIndexes].map((index) => compiledZoneIds[index]))
  return computed(
    observedZoneIds.size / declaredZoneIds.size,
    'eight-view-depth-resolved-material-id-union@1',
    [input.evaluation.receipt.deterministic_fingerprint, input.visibility.deterministic_fingerprint],
  )
}

function computeFpsOccupancy(input: KnifeObjectiveMetricAdapterInput): ComputedMetric {
  const fpsView = input.evaluation.views.find((view) => view.view_id === 'FPS_HOLD')
  if (!fpsView || fpsView.mask.width !== input.evaluation.rig.frame_width || fpsView.mask.height !== input.evaluation.rig.frame_height) {
    return notComputable('fps-occupancy requires a bound FPS_HOLD mask', input)
  }
  const coveredPixelCount = fpsView.mask.pixels.reduce((count, pixel) => count + (pixel === 0 ? 0 : 1), 0)
  const framePixelCount = fpsView.mask.width * fpsView.mask.height
  if (framePixelCount <= 0 || coveredPixelCount === 0) {
    return notComputable('fps-occupancy has no observed FPS_HOLD silhouette pixels', input)
  }
  const visibilityFpsPixels = input.visibility.parts.reduce((sum, part) => {
    const row = part.views.find((view) => view.view_id === 'FPS_HOLD')
    return sum + (row?.visible_pixel_count ?? 0)
  }, 0)
  if (visibilityFpsPixels !== coveredPixelCount) {
    return notComputable('FPS_HOLD mask and Part visibility pixel ownership disagree', input)
  }
  return computed(
    coveredPixelCount / framePixelCount,
    'fps-hold-depth-resolved-mask-occupancy@1',
    [input.evaluation.receipt.deterministic_fingerprint, input.visibility.deterministic_fingerprint, input.guard_fps.deterministic_fingerprint],
  )
}

function unavailableMetric(metric: KnifeObjectiveMetric, input: KnifeObjectiveMetricAdapterInput): ComputedMetric {
  const visualMetrics = new Set<KnifeObjectiveMetric>([
    'silhouette-iou',
    'boundary-f1',
    'symmetric-chamfer',
    'p95-contour-distance',
    'tip-landmark-error',
    'belly-depth-error',
  ])
  const structuralMetrics = new Set<KnifeObjectiveMetric>(['thickness-continuity', 'normal-continuity', 'negative-space-error'])
  return {
    value: KNIFE_OBJECTIVE_NOT_COMPUTABLE,
    evidence_class: visualMetrics.has(metric) ? 'visual-evidence' : structuralMetrics.has(metric) ? 'structural-proxy' : 'visual-evidence',
    basis: metric === 'negative-space-error'
      ? 'guard-convex-hull-visible-opening-proxy-without-bound-reference-target@1'
      : visualMetrics.has(metric)
        ? 'authorized-reference-mask-or-landmark-evidence-not-supplied@1'
        : 'dedicated-thickness-or-normal-evidence-not-supplied@1',
    receipt_fingerprints: metric === 'negative-space-error'
      ? [input.guard_fps.deterministic_fingerprint]
      : Object.freeze([]),
  }
}

interface ComputedMetric {
  readonly value: KnifeObjectiveValueV2
  readonly evidence_class: 'structural-proxy' | 'visual-evidence'
  readonly basis: string
  readonly receipt_fingerprints: readonly string[]
}

function computed(value: number, basis: string, receiptFingerprints: readonly string[]): ComputedMetric {
  if (!Number.isFinite(value) || value < 0 || value > 1) {
    throw new KnifeObjectiveMetricAdapterError('METRIC_RECEIPT_MISMATCH', `computed metric value is outside [0,1]: ${value}`)
  }
  return { value, evidence_class: 'structural-proxy', basis, receipt_fingerprints: receiptFingerprints }
}

function notComputable(reason: string, input: KnifeObjectiveMetricAdapterInput): ComputedMetric {
  // The reason is represented in the basis and never converted into a zero.
  return {
    value: KNIFE_OBJECTIVE_NOT_COMPUTABLE,
    evidence_class: 'structural-proxy',
    basis: `not-computable:${reason}`,
    receipt_fingerprints: [input.evaluation.receipt.deterministic_fingerprint],
  }
}

function unionLedgerMetrics(ledger: KnifeObjectiveLedger): readonly KnifeObjectiveMetric[] {
  const selected = new Set([...ledger.objective_metrics, ...ledger.regression_limits])
  return Object.freeze(KNIFE_OBJECTIVE_METRICS.filter((metric) => selected.has(metric)))
}

function invalid(message: string): never {
  throw new KnifeObjectiveMetricAdapterError('INVALID_INPUT', message)
}

function isFingerprint(value: unknown): value is string {
  return typeof value === 'string' && FINGERPRINT.test(value)
}

function isRecord(value: unknown): value is Record<string, any> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value)
}

function sameTypedArray(left: ArrayLike<number>, right: ArrayLike<number>): boolean {
  if (left.length !== right.length) return false
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return false
  }
  return true
}

function canonicalJson(value: unknown): string {
  if (value === null) return 'null'
  if (typeof value === 'string') return JSON.stringify(value)
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new KnifeObjectiveMetricAdapterError('INVALID_INPUT', 'canonical JSON cannot contain non-finite numbers')
    return Object.is(value, -0) ? '0' : JSON.stringify(value)
  }
  if (Array.isArray(value)) return `[${value.map((item) => canonicalJson(item)).join(',')}]`
  if (isRecord(value)) {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`
  }
  throw new KnifeObjectiveMetricAdapterError('INVALID_INPUT', 'canonical JSON cannot contain undefined or executable values')
}

function deepFreeze<T>(value: T): T {
  if (!value || typeof value !== 'object' || Object.isFrozen(value)) return value
  for (const child of Object.values(value as Record<string, unknown>)) deepFreeze(child)
  return Object.freeze(value)
}
