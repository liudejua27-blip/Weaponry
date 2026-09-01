import {
  KNIFE_OBJECTIVE_NOT_COMPUTABLE,
  type KnifeObjectiveValueV2,
} from './knife-objective-function-v2.ts'
import {
  isIntrinsicKnifeObjectiveMetric,
  knifeObjectiveMetricCatalogEntry,
} from './knife-objective-metric-catalog.ts'
import type { KnifeObjectiveMetric } from './knife-objective-ledger.ts'
import {
  measureKnifeObjectiveMetricValues,
  type KnifeObjectiveMetricAdapterInput,
  type KnifeObjectiveMetricAdapterReceipt,
  type KnifeObjectiveMetricEvidence,
} from './knife-objective-metric-adapter.ts'
import {
  measureKnifeIntrinsicMorphology,
  type KnifeIntrinsicMorphologyReceipt,
} from './knife-intrinsic-morphology.ts'
import {
  measureKnifeAssemblyIntrinsicMetrics,
  type KnifeAssemblyIntrinsicMetrics,
  type KnifeAssemblyIntrinsicMetricValue,
} from './knife-assembly-intrinsic-metrics.ts'
import { sha256Hex } from './knife-browser-capture.ts'

/**
 * Append-only successor to Adapter@1. Raster metrics preserve the exact v1
 * derivation while the new metric IDs consume renderer-free blade and
 * assembly receipts. None of these structural priors is a visual-quality,
 * engine, human-review, or commercial acceptance result.
 */
export const KNIFE_OBJECTIVE_METRIC_ADAPTER_V2_SCHEMA = 'WeaponryThreeJsKnifeObjectiveMetricAdapter@2' as const

export interface KnifeObjectiveMetricAdapterReceiptV2 {
  readonly schema_version: typeof KNIFE_OBJECTIVE_METRIC_ADAPTER_V2_SCHEMA
  readonly status: KnifeObjectiveMetricAdapterReceipt['status']
  readonly objective_metrics: readonly KnifeObjectiveMetric[]
  readonly regression_metrics: readonly KnifeObjectiveMetric[]
  readonly metrics: Readonly<Record<KnifeObjectiveMetric, KnifeObjectiveValueV2>>
  readonly metric_evidence: readonly KnifeObjectiveMetricEvidence[]
  readonly candidate_program_sha256: string
  readonly candidate_program_hash_policy: KnifeObjectiveMetricAdapterReceipt['candidate_program_hash_policy']
  readonly source_program_sha256: string
  readonly ledger_sha256: string
  readonly source_fingerprint: string
  readonly rig_fingerprint: string
  readonly metric_receipt_fingerprints: Readonly<Record<KnifeObjectiveMetric, readonly string[]>>
  readonly raster_receipt: KnifeObjectiveMetricAdapterReceipt
  readonly intrinsic_morphology: KnifeIntrinsicMorphologyReceipt
  readonly assembly_intrinsic: KnifeAssemblyIntrinsicMetrics
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
  readonly visual_quality_status: 'NOT_COMPUTABLE'
  readonly deterministic_fingerprint: string
}

export type KnifeObjectiveMetricAdapterAnyReceipt =
  | KnifeObjectiveMetricAdapterReceipt
  | KnifeObjectiveMetricAdapterReceiptV2

export function measureKnifeObjectiveMetricValuesV2(
  input: KnifeObjectiveMetricAdapterInput,
): KnifeObjectiveMetricAdapterReceiptV2 {
  const raster = measureKnifeObjectiveMetricValues(input)
  const morphology = measureKnifeIntrinsicMorphology(input.program)
  const assembly = measureKnifeAssemblyIntrinsicMetrics(input.program, input.compiled)
  const metrics = { ...raster.metrics } as Record<KnifeObjectiveMetric, KnifeObjectiveValueV2>
  const evidenceByMetric = new Map(raster.metric_evidence.map((row) => [row.metric, row]))
  const fingerprints = { ...raster.metric_receipt_fingerprints } as Record<KnifeObjectiveMetric, readonly string[]>

  for (const metric of [...input.ledger.objective_metrics, ...input.ledger.regression_limits]) {
    if (!isIntrinsicKnifeObjectiveMetric(metric)) continue
    const value = intrinsicValue(metric, morphology, assembly)
    const sourceReceipt = knifeObjectiveMetricCatalogEntry(metric).owner === 'blade'
      ? morphology.deterministic_fingerprint
      : assembly.deterministic_fingerprint
    const receiptFingerprints = Object.freeze([sourceReceipt])
    metrics[metric] = value
    fingerprints[metric] = receiptFingerprints
    evidenceByMetric.set(metric, Object.freeze({
      metric,
      value,
      computability: value === KNIFE_OBJECTIVE_NOT_COMPUTABLE ? KNIFE_OBJECTIVE_NOT_COMPUTABLE : 'COMPUTED',
      evidence_class: 'structural-proxy',
      basis: value === KNIFE_OBJECTIVE_NOT_COMPUTABLE
        ? `not-computable:${knifeObjectiveMetricCatalogEntry(metric).not_computable_when}`
        : knifeObjectiveMetricCatalogEntry(metric).basis_schema,
      receipt_fingerprints: receiptFingerprints,
    }))
  }

  const orderedMetrics = Object.freeze(Object.fromEntries(
    Object.keys(raster.metrics).map((metric) => [metric, metrics[metric as KnifeObjectiveMetric]]),
  )) as Readonly<Record<KnifeObjectiveMetric, KnifeObjectiveValueV2>>
  const metricEvidence = Object.freeze(Object.keys(raster.metrics).map((metric) =>
    evidenceByMetric.get(metric as KnifeObjectiveMetric)!,
  ))
  const metricReceiptFingerprints = Object.freeze(Object.fromEntries(
    Object.keys(raster.metrics).map((metric) => [metric, fingerprints[metric as KnifeObjectiveMetric]]),
  )) as Readonly<Record<KnifeObjectiveMetric, readonly string[]>>

  const draft = {
    schema_version: KNIFE_OBJECTIVE_METRIC_ADAPTER_V2_SCHEMA,
    status: raster.status,
    objective_metrics: raster.objective_metrics,
    regression_metrics: raster.regression_metrics,
    metrics: orderedMetrics,
    metric_evidence: metricEvidence,
    candidate_program_sha256: raster.candidate_program_sha256,
    candidate_program_hash_policy: raster.candidate_program_hash_policy,
    source_program_sha256: raster.source_program_sha256,
    ledger_sha256: raster.ledger_sha256,
    source_fingerprint: raster.source_fingerprint,
    rig_fingerprint: raster.rig_fingerprint,
    metric_receipt_fingerprints: metricReceiptFingerprints,
    raster_receipt: raster,
    intrinsic_morphology: morphology,
    assembly_intrinsic: assembly,
    renderer_invoked: false as const,
    quality_status: 'NOT_RUN' as const,
    visual_quality_status: 'NOT_COMPUTABLE' as const,
    deterministic_fingerprint: '',
  }
  return deepFreeze({
    ...draft,
    deterministic_fingerprint: sha256Hex(canonicalJson(draft)),
  })
}

export const evaluateKnifeObjectiveMetricValuesV2 = measureKnifeObjectiveMetricValuesV2
export const measureKnifeObjectiveMetricAdapterV2 = measureKnifeObjectiveMetricValuesV2

function intrinsicValue(
  metric: KnifeObjectiveMetric,
  morphology: KnifeIntrinsicMorphologyReceipt,
  assembly: KnifeAssemblyIntrinsicMetrics,
): KnifeObjectiveValueV2 {
  switch (metric) {
    case 'blade-section-profile-continuity': return morphology.sections.profile_continuity
    case 'blade-curve-g1': return morphology.metrics.curve_g1_proxy
    case 'blade-tip-taper': return morphology.metrics.tip_taper
    case 'blade-extrema-headroom': return boundedMean(
      morphology.metrics.spine_extrema_budget,
      morphology.metrics.edge_extrema_budget,
    )
    case 'assembly-ratio-prior-score': return assemblyValue(assembly.readability_proxy.ratio_prior_score)
    case 'assembly-attachment-continuity': return assemblyValue(assembly.readability_proxy.attachment_continuity)
    case 'assembly-material-readability': return assemblyValue(assembly.readability_proxy.material_zone_readability)
    case 'assembly-complexity-efficiency': return assembly.readability_proxy.complexity_efficiency
    default: return KNIFE_OBJECTIVE_NOT_COMPUTABLE
  }
}

function assemblyValue(value: KnifeAssemblyIntrinsicMetricValue): KnifeObjectiveValueV2 {
  return value === 'NOT_COMPUTABLE' ? KNIFE_OBJECTIVE_NOT_COMPUTABLE : value
}

function boundedMean(left: number, right: number): number {
  const value = (left + right) * 0.5
  if (!Number.isFinite(value) || value < 0 || value > 1) {
    throw new Error('KNIFE_OBJECTIVE_METRIC_ADAPTER_V2_INVALID: intrinsic mean is outside [0,1]')
  }
  return value
}

function canonicalJson(value: unknown): string {
  if (value === null) return 'null'
  if (typeof value === 'string') return JSON.stringify(value)
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new Error('KNIFE_OBJECTIVE_METRIC_ADAPTER_V2_INVALID: non-finite value')
    return Object.is(value, -0) ? '0' : JSON.stringify(value)
  }
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  if (value && typeof value === 'object') {
    const record = value as Record<string, unknown>
    return `{${Object.keys(record).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`).join(',')}}`
  }
  throw new Error('KNIFE_OBJECTIVE_METRIC_ADAPTER_V2_INVALID: unsupported canonical JSON value')
}

function deepFreeze<T>(value: T): T {
  if (!value || typeof value !== 'object' || Object.isFrozen(value)) return value
  for (const child of Object.values(value as Record<string, unknown>)) deepFreeze(child)
  return Object.freeze(value)
}
