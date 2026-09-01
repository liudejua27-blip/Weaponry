import type { KnifeObjectiveLedger, KnifeObjectiveMetric } from './knife-objective-ledger.ts'
import { KNIFE_OBJECTIVE_METRICS, validateKnifeObjectiveLedger } from './knife-objective-ledger.ts'
import { sha256Hex } from './knife-browser-capture.ts'

/**
 * Closed direction-aware objective contract for the lightweight Three.js
 * route.  This is deliberately a successor to KnifeObjectiveLedger@1 rather
 * than a mutation of that ledger: the ledger owns scope/budget and this
 * module owns metric semantics and candidate comparison.
 */
export const KNIFE_OBJECTIVE_FUNCTION_V2_SCHEMA = 'KnifeObjectiveFunction@2' as const
export const KNIFE_OBJECTIVE_CANDIDATE_V2_SCHEMA = 'KnifeObjectiveCandidate@2' as const
export const KNIFE_OBJECTIVE_METRIC_EVALUATION_V2_SCHEMA = 'KnifeObjectiveMetricEvaluation@2' as const
export const KNIFE_OBJECTIVE_SELECTION_RECEIPT_V2_SCHEMA = 'KnifeObjectiveSelectionReceipt@2' as const
export const KNIFE_OBJECTIVE_NOT_COMPUTABLE = 'NOT_COMPUTABLE' as const

export const KNIFE_OBJECTIVE_DIRECTION_V2 = Object.freeze(['maximize', 'minimize'] as const)
export type KnifeObjectiveDirectionV2 = (typeof KNIFE_OBJECTIVE_DIRECTION_V2)[number]

/** Objective metrics drive improvement; regression metrics only guard regressions. */
export const KNIFE_OBJECTIVE_METRIC_ROLES_V2 = Object.freeze(['objective', 'regression'] as const)
export type KnifeObjectiveMetricRoleV2 = (typeof KNIFE_OBJECTIVE_METRIC_ROLES_V2)[number]

/** A structural proxy is measurable, but never a visual-quality decision. */
export const KNIFE_OBJECTIVE_EVIDENCE_CLASSES_V2 = Object.freeze([
  'structural-proxy',
  'visual-evidence',
] as const)
export type KnifeObjectiveEvidenceClassV2 = (typeof KNIFE_OBJECTIVE_EVIDENCE_CLASSES_V2)[number]

export type KnifeObjectiveValueV2 = number | typeof KNIFE_OBJECTIVE_NOT_COMPUTABLE
export type KnifeObjectiveMetricValuesV2 = Readonly<Partial<Record<KnifeObjectiveMetric, KnifeObjectiveValueV2>>>

export interface KnifeObjectiveTargetIntervalV2 {
  readonly min: number
  readonly max: number
}

/** One named metric's direction, target interval and allowed regression. */
export interface KnifeObjectiveMetricTargetV2 {
  readonly metric: KnifeObjectiveMetric
  readonly role: KnifeObjectiveMetricRoleV2
  readonly direction: KnifeObjectiveDirectionV2
  readonly target_interval: KnifeObjectiveTargetIntervalV2
  /** Direction-normalized improvement required for a candidate successor. */
  readonly minimum_improvement: number
  /** Direction-normalized regression allowed before this metric rejects a candidate. */
  readonly regression_limit: number
  readonly evidence_class: KnifeObjectiveEvidenceClassV2
  /** Required metrics missing evidence make the candidate NOT_COMPUTABLE. */
  readonly required: boolean
}

export interface KnifeObjectiveFunctionV2 {
  readonly schema_version: typeof KNIFE_OBJECTIVE_FUNCTION_V2_SCHEMA
  readonly objective_id: string
  readonly ledger_sha256: string
  readonly baseline_candidate_sha256: string
  readonly candidate_budget: number
  /** Kept equal to KnifeObjectiveLedger@1.minimum_improvement. */
  readonly minimum_improvement: number
  readonly metric_targets: readonly KnifeObjectiveMetricTargetV2[]
  readonly baseline_values: KnifeObjectiveMetricValuesV2
  readonly canonical_sha256: string
}

export type KnifeObjectiveFunctionV2Draft = Omit<KnifeObjectiveFunctionV2, 'canonical_sha256'> & {
  readonly canonical_sha256?: string
}

/** Convenience input that binds all budget and stop semantics from Ledger@1. */
export interface KnifeObjectiveFunctionV2LedgerDraft {
  readonly ledger: KnifeObjectiveLedger
  readonly objective_id: string
  readonly metric_targets: readonly KnifeObjectiveMetricTargetV2[]
  readonly baseline_values: KnifeObjectiveMetricValuesV2
}

export interface KnifeObjectiveCandidateV2 {
  readonly schema_version: typeof KNIFE_OBJECTIVE_CANDIDATE_V2_SCHEMA
  readonly candidate_id: string
  readonly candidate_sha256: string
  readonly values: KnifeObjectiveMetricValuesV2
}

export type KnifeObjectiveCandidateV2Draft = Omit<KnifeObjectiveCandidateV2, 'schema_version'> & {
  readonly schema_version?: typeof KNIFE_OBJECTIVE_CANDIDATE_V2_SCHEMA
}

export type KnifeObjectiveMetricEvaluationStatusV2 =
  | 'WITHIN_TARGET'
  | 'OUTSIDE_TARGET'
  | typeof KNIFE_OBJECTIVE_NOT_COMPUTABLE

export type KnifeObjectiveRegressionStatusV2 =
  | 'WITHIN_LIMIT'
  | 'REGRESSION_OVER_LIMIT'
  | typeof KNIFE_OBJECTIVE_NOT_COMPUTABLE

export interface KnifeObjectiveMetricEvaluationV2 {
  readonly schema_version: typeof KNIFE_OBJECTIVE_METRIC_EVALUATION_V2_SCHEMA
  readonly metric: KnifeObjectiveMetric
  readonly role: KnifeObjectiveMetricRoleV2
  readonly direction: KnifeObjectiveDirectionV2
  readonly evidence_class: KnifeObjectiveEvidenceClassV2
  readonly required: boolean
  readonly target_interval: KnifeObjectiveTargetIntervalV2
  readonly regression_limit: number
  readonly baseline_value: KnifeObjectiveValueV2
  readonly candidate_value: KnifeObjectiveValueV2
  /** Candidate minus baseline for maximize, baseline minus candidate for minimize. */
  readonly improvement: KnifeObjectiveValueV2
  /** Negative direction-aware improvement remains visible even when within the allowed limit. */
  readonly is_regression: boolean
  readonly target_status: KnifeObjectiveMetricEvaluationStatusV2
  readonly regression_status: KnifeObjectiveRegressionStatusV2
}

export type KnifeObjectiveCandidateGateV2 = 'ELIGIBLE' | 'REJECTED' | typeof KNIFE_OBJECTIVE_NOT_COMPUTABLE
export type KnifeObjectiveComputabilityV2 = 'COMPUTABLE' | 'PARTIAL' | typeof KNIFE_OBJECTIVE_NOT_COMPUTABLE

export interface KnifeObjectiveCandidateEvaluationV2 {
  readonly candidate_id: string
  readonly candidate_sha256: string
  readonly metrics: readonly KnifeObjectiveMetricEvaluationV2[]
  readonly computability: KnifeObjectiveComputabilityV2
  readonly objective_gate: KnifeObjectiveCandidateGateV2
  readonly required_metrics_computable: boolean
  readonly meets_target_intervals: boolean
  readonly meets_regression_limits: boolean
  readonly meets_minimum_improvement: boolean
  readonly improved_metrics: readonly KnifeObjectiveMetric[]
  readonly regression_metrics: readonly KnifeObjectiveMetric[]
  readonly non_computable_metrics: readonly KnifeObjectiveMetric[]
  /** This is a structural/objective eligibility result, not a visual result. */
  readonly selection_eligible: boolean
}

export type KnifeObjectiveSelectionStatusV2 =
  | 'REVIEW_ONLY_SELECTION'
  | 'PARENT_RETAINED'
  | typeof KNIFE_OBJECTIVE_NOT_COMPUTABLE

export interface KnifeObjectiveSelectionReceiptV2 {
  readonly schema_version: typeof KNIFE_OBJECTIVE_SELECTION_RECEIPT_V2_SCHEMA
  readonly objective_sha256: string
  readonly ledger_sha256: string
  readonly baseline_candidate_sha256: string
  readonly candidate_evaluations: readonly KnifeObjectiveCandidateEvaluationV2[]
  readonly pareto_candidate_ids: readonly string[]
  readonly selected_candidate_id: string | null
  readonly selection_status: KnifeObjectiveSelectionStatusV2
  readonly selection_basis: 'direction-aware-pareto@1/computability-first/lexical-tie-break'
  readonly decision_label: 'NON_VISUAL_STRUCTURAL_RANKING'
  /** Structural proxy comparison never grants visual review or quality. */
  readonly visual_status: 'NOT_REVIEWED' | typeof KNIFE_OBJECTIVE_NOT_COMPUTABLE
  readonly quality_status: 'NOT_RUN'
  readonly human_status: 'NOT_RUN'
  readonly deterministic_fingerprint: string
}

export class KnifeObjectiveFunctionV2Error extends Error {
  readonly code: KnifeObjectiveFunctionV2ErrorCode

  constructor(code: KnifeObjectiveFunctionV2ErrorCode, message: string) {
    super(`${code}: ${message}`)
    this.name = 'KnifeObjectiveFunctionV2Error'
    this.code = code
  }
}

export type KnifeObjectiveFunctionV2ErrorCode =
  | 'INVALID_OBJECTIVE'
  | 'INVALID_CANDIDATE'
  | 'CANONICAL_HASH_MISMATCH'
  | 'CANONICAL_HASH_REQUIRED'
  | 'LEDGER_BINDING_MISMATCH'
  | 'CANDIDATE_BUDGET_EXCEEDED'

const SHA256 = /^[a-f0-9]{64}$/
const STABLE_ID = /^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/
const MAX_CANDIDATES = 32

/** Construct an immutable objective from a full hash-bound draft. */
export function createKnifeObjectiveFunctionV2(input: KnifeObjectiveFunctionV2Draft): KnifeObjectiveFunctionV2
/** Construct an immutable objective bound directly to an existing Ledger@1. */
export function createKnifeObjectiveFunctionV2(input: KnifeObjectiveFunctionV2LedgerDraft): KnifeObjectiveFunctionV2
export function createKnifeObjectiveFunctionV2(
  input: KnifeObjectiveFunctionV2Draft | KnifeObjectiveFunctionV2LedgerDraft,
): KnifeObjectiveFunctionV2 {
  if (!isRecord(input)) invalidObjective('objective draft must be an object')
  const source = input as Record<string, unknown>

  if ('ledger' in source) {
    exactKeys(source, ['ledger', 'objective_id', 'metric_targets', 'baseline_values'], 'ledger objective draft')
    if (!isRecord(source.ledger)) invalidObjective('ledger must be an object')
    validateKnifeObjectiveLedger(source.ledger, { require_canonical_sha256: true })
    const ledger = source.ledger as KnifeObjectiveLedger
    const draft: KnifeObjectiveFunctionV2Draft = {
      schema_version: KNIFE_OBJECTIVE_FUNCTION_V2_SCHEMA,
      objective_id: source.objective_id as string,
      ledger_sha256: ledger.canonical_sha256,
      baseline_candidate_sha256: ledger.baseline_candidate_sha256,
      candidate_budget: ledger.candidate_budget,
      minimum_improvement: ledger.minimum_improvement,
      metric_targets: source.metric_targets as readonly KnifeObjectiveMetricTargetV2[],
      baseline_values: source.baseline_values as KnifeObjectiveMetricValuesV2,
      canonical_sha256: '',
    }
    validateObjectiveDraft(draft)
    validateObjectiveLedgerBinding(draft, ledger)
    return finalizeObjective(draft)
  }

  exactKeysAllowOptional(source, [
    'schema_version',
    'objective_id',
    'ledger_sha256',
    'baseline_candidate_sha256',
    'candidate_budget',
    'minimum_improvement',
    'metric_targets',
    'baseline_values',
  ], ['canonical_sha256'], 'objective draft')
  const draft = { ...source } as unknown as KnifeObjectiveFunctionV2Draft
  validateObjectiveDraft(draft)
  const suppliedCanonical = draft.canonical_sha256
  const blank = { ...draft, canonical_sha256: '' } as KnifeObjectiveFunctionV2
  const canonical_sha256 = canonicalKnifeObjectiveFunctionV2Sha256(blank)
  if (suppliedCanonical !== undefined && suppliedCanonical !== '' && suppliedCanonical !== canonical_sha256) {
    throw new KnifeObjectiveFunctionV2Error('CANONICAL_HASH_MISMATCH', 'canonical_sha256 does not match canonical JSON')
  }
  return deepFreeze({ ...blank, canonical_sha256 })
}

export function createKnifeObjectiveFunctionV2FromLedger(
  input: KnifeObjectiveFunctionV2LedgerDraft,
): KnifeObjectiveFunctionV2 {
  return createKnifeObjectiveFunctionV2(input)
}

/** Construct an immutable, hash-bound candidate record. */
export function createKnifeObjectiveCandidateV2(input: KnifeObjectiveCandidateV2Draft): KnifeObjectiveCandidateV2 {
  if (!isRecord(input)) invalidCandidate('candidate must be an object')
  const source = input as Record<string, unknown>
  exactKeysAllowOptionalCandidate(source, ['candidate_id', 'candidate_sha256', 'values'], ['schema_version'])
  if (source.schema_version !== undefined && source.schema_version !== KNIFE_OBJECTIVE_CANDIDATE_V2_SCHEMA) {
    invalidCandidate('schema_version drifted')
  }
  if (!isStableId(source.candidate_id)) invalidCandidate('candidate_id is invalid')
  if (!isSha(source.candidate_sha256)) invalidCandidate('candidate_sha256 is invalid')
  if (!isRecord(source.values)) invalidCandidate('values must be an object')
  validateCandidateMetricValueShape(source.values)
  return deepFreeze({
    schema_version: KNIFE_OBJECTIVE_CANDIDATE_V2_SCHEMA,
    candidate_id: source.candidate_id,
    candidate_sha256: source.candidate_sha256,
    values: cloneRecord(source.values),
  } as KnifeObjectiveCandidateV2)
}

export function validateKnifeObjectiveFunctionV2(
  value: unknown,
  options: { readonly require_canonical_sha256?: boolean } = {},
): asserts value is KnifeObjectiveFunctionV2 {
  if (!isRecord(value)) invalidObjective('objective must be an object')
  exactKeys(value, [
    'schema_version',
    'objective_id',
    'ledger_sha256',
    'baseline_candidate_sha256',
    'candidate_budget',
    'minimum_improvement',
    'metric_targets',
    'baseline_values',
    'canonical_sha256',
  ], 'objective')
  validateObjectiveDraft(value as unknown as KnifeObjectiveFunctionV2Draft)
  const digest = canonicalKnifeObjectiveFunctionV2Sha256(value)
  if (value.canonical_sha256 !== '' && value.canonical_sha256 !== digest) {
    throw new KnifeObjectiveFunctionV2Error('CANONICAL_HASH_MISMATCH', 'canonical_sha256 does not match canonical JSON')
  }
  if (options.require_canonical_sha256 && value.canonical_sha256 === '') {
    throw new KnifeObjectiveFunctionV2Error('CANONICAL_HASH_REQUIRED', 'objective binding requires a canonical_sha256')
  }
}

export function validateKnifeObjectiveCandidateV2(value: unknown): asserts value is KnifeObjectiveCandidateV2 {
  createKnifeObjectiveCandidateV2(value as KnifeObjectiveCandidateV2Draft)
}

/** SHA-256 over canonical JSON with this object's own digest blanked. */
export function canonicalKnifeObjectiveFunctionV2Sha256(
  value: KnifeObjectiveFunctionV2 | Record<string, unknown>,
): string {
  if (!isRecord(value)) invalidObjective('objective must be an object')
  return sha256Hex(canonicalJson({ ...value, canonical_sha256: '' }))
}

/**
 * Evaluate and select candidates using direction-aware Pareto comparison.
 * Missing required evidence is NOT_COMPUTABLE; it is never coerced to zero.
 */
export function evaluateKnifeObjectiveFunctionV2(
  objective: KnifeObjectiveFunctionV2,
  candidates: readonly KnifeObjectiveCandidateV2[],
): KnifeObjectiveSelectionReceiptV2 {
  validateKnifeObjectiveFunctionV2(objective, { require_canonical_sha256: true })
  if (!Array.isArray(candidates)) invalidCandidate('candidates must be an array')
  if (candidates.length > objective.candidate_budget || candidates.length > MAX_CANDIDATES) {
    throw new KnifeObjectiveFunctionV2Error('CANDIDATE_BUDGET_EXCEEDED', `candidate count exceeds budget ${objective.candidate_budget}`)
  }

  const seenIds = new Set<string>()
  const seenHashes = new Set<string>()
  const validatedCandidates = candidates.map((candidate) => {
    validateCandidateShape(candidate as unknown)
    validateCandidateValues(candidate.values, new Set(objective.metric_targets.map((target) => target.metric)))
    if (seenIds.has(candidate.candidate_id)) invalidCandidate(`duplicate candidate_id ${candidate.candidate_id}`)
    if (seenHashes.has(candidate.candidate_sha256)) invalidCandidate(`duplicate candidate_sha256 ${candidate.candidate_sha256}`)
    seenIds.add(candidate.candidate_id)
    seenHashes.add(candidate.candidate_sha256)
    return candidate
  })
  // Candidate order is not evidence. Sort with a locale-independent comparator
  // so the receipt is replay-stable across callers and host locales.
  validatedCandidates.sort(compareCandidates)
  const evaluations = validatedCandidates.map((candidate) => evaluateCandidate(objective, candidate))

  const frozenEvaluations = Object.freeze(evaluations)
  const pareto = paretoFront(frozenEvaluations)
  const paretoIds = Object.freeze(pareto.map((evaluation) => evaluation.candidate_id).sort())
  const selected = pareto.length > 0
    ? [...pareto].sort(compareEvaluationsForSelection)[0].candidate_id
    : null
  const anyComputable = frozenEvaluations.some((evaluation) => evaluation.computability !== KNIFE_OBJECTIVE_NOT_COMPUTABLE)
  const anyRequiredComputable = frozenEvaluations.some((evaluation) => evaluation.required_metrics_computable)
  const selection_status: KnifeObjectiveSelectionStatusV2 = selected !== null
    ? 'REVIEW_ONLY_SELECTION'
    : frozenEvaluations.length === 0
      ? 'PARENT_RETAINED'
    : anyComputable && anyRequiredComputable
      ? 'PARENT_RETAINED'
      : KNIFE_OBJECTIVE_NOT_COMPUTABLE
  const visual_status: KnifeObjectiveSelectionReceiptV2['visual_status'] = anyComputable
    ? 'NOT_REVIEWED'
    : KNIFE_OBJECTIVE_NOT_COMPUTABLE

  const draft = {
    schema_version: KNIFE_OBJECTIVE_SELECTION_RECEIPT_V2_SCHEMA,
    objective_sha256: objective.canonical_sha256,
    ledger_sha256: objective.ledger_sha256,
    baseline_candidate_sha256: objective.baseline_candidate_sha256,
    candidate_evaluations: frozenEvaluations,
    pareto_candidate_ids: paretoIds,
    selected_candidate_id: selected,
    selection_status,
    selection_basis: 'direction-aware-pareto@1/computability-first/lexical-tie-break' as const,
    decision_label: 'NON_VISUAL_STRUCTURAL_RANKING' as const,
    visual_status,
    quality_status: 'NOT_RUN' as const,
    human_status: 'NOT_RUN' as const,
    deterministic_fingerprint: '',
  }
  return deepFreeze({
    ...draft,
    deterministic_fingerprint: sha256Hex(canonicalJson(draft)),
  })
}

export const evaluateKnifeObjectiveV2 = evaluateKnifeObjectiveFunctionV2
export const selectKnifeObjectiveCandidatesV2 = evaluateKnifeObjectiveFunctionV2

function finalizeObjective(draft: KnifeObjectiveFunctionV2Draft): KnifeObjectiveFunctionV2 {
  validateObjectiveDraft(draft)
  const canonical_sha256 = canonicalKnifeObjectiveFunctionV2Sha256({ ...draft, canonical_sha256: '' })
  return deepFreeze({ ...draft, schema_version: KNIFE_OBJECTIVE_FUNCTION_V2_SCHEMA, canonical_sha256 }) as KnifeObjectiveFunctionV2
}

function validateObjectiveDraft(draft: KnifeObjectiveFunctionV2Draft): void {
  if (draft.schema_version !== KNIFE_OBJECTIVE_FUNCTION_V2_SCHEMA) invalidObjective('schema_version drifted')
  if (!isStableId(draft.objective_id)) invalidObjective('objective_id is invalid')
  if (!isSha(draft.ledger_sha256)) invalidObjective('ledger_sha256 is invalid')
  if (!isSha(draft.baseline_candidate_sha256)) invalidObjective('baseline_candidate_sha256 is invalid')
  if (!isIntegerInRange(draft.candidate_budget, 1, MAX_CANDIDATES)) invalidObjective('candidate_budget is outside [1,32]')
  if (!isFinitePositive(draft.minimum_improvement)) invalidObjective('minimum_improvement must be positive and finite')
  if (!Array.isArray(draft.metric_targets) || draft.metric_targets.length === 0 || draft.metric_targets.length > 12) {
    invalidObjective('metric_targets must contain 1 to 12 metrics')
  }
  const metrics = new Set<KnifeObjectiveMetric>()
  for (const target of draft.metric_targets) {
    validateMetricTarget(target)
    if (metrics.has(target.metric)) invalidObjective(`duplicate metric target ${target.metric}`)
    metrics.add(target.metric)
    if (target.role === 'objective' && target.minimum_improvement < draft.minimum_improvement) {
      invalidObjective(`${target.metric} minimum_improvement cannot be below objective minimum_improvement`)
    }
  }
  if (!draft.metric_targets.some((target) => target.role === 'objective')) {
    invalidObjective('metric_targets must contain at least one objective metric')
  }
  validateMetricValues(draft.baseline_values, metrics, 'baseline_values')
  if (draft.canonical_sha256 !== undefined && draft.canonical_sha256 !== '' && !isSha(draft.canonical_sha256)) {
    invalidObjective('canonical_sha256 is invalid')
  }
}

function validateObjectiveLedgerBinding(draft: KnifeObjectiveFunctionV2Draft, ledger: KnifeObjectiveLedger): void {
  if (draft.ledger_sha256 !== ledger.canonical_sha256
    || draft.baseline_candidate_sha256 !== ledger.baseline_candidate_sha256
    || draft.candidate_budget !== ledger.candidate_budget
    || draft.minimum_improvement !== ledger.minimum_improvement) {
    throw new KnifeObjectiveFunctionV2Error('LEDGER_BINDING_MISMATCH', 'objective fields do not bind Ledger@1')
  }
  const objectiveMetrics = new Set(ledger.objective_metrics)
  const regressionMetrics = new Set(ledger.regression_limits)
  const overlap = [...objectiveMetrics].filter((metric) => regressionMetrics.has(metric))
  if (overlap.length > 0) {
    throw new KnifeObjectiveFunctionV2Error('LEDGER_BINDING_MISMATCH', `Ledger@1 metric roles overlap: ${overlap.join(',')}`)
  }
  const ledgerRoles = new Map<KnifeObjectiveMetric, KnifeObjectiveMetricRoleV2>()
  for (const metric of objectiveMetrics) ledgerRoles.set(metric, 'objective')
  for (const metric of regressionMetrics) ledgerRoles.set(metric, 'regression')
  if (draft.metric_targets.length !== ledgerRoles.size) {
    throw new KnifeObjectiveFunctionV2Error('LEDGER_BINDING_MISMATCH', 'objective targets must cover the complete Ledger@1 objective/regression union')
  }
  const seen = new Set<KnifeObjectiveMetric>()
  for (const target of draft.metric_targets) {
    const expectedRole = ledgerRoles.get(target.metric)
    if (expectedRole === undefined) {
      throw new KnifeObjectiveFunctionV2Error('LEDGER_BINDING_MISMATCH', `${target.metric} is not declared by Ledger@1`)
    }
    if (target.role !== expectedRole) {
      throw new KnifeObjectiveFunctionV2Error('LEDGER_BINDING_MISMATCH', `${target.metric} must use Ledger@1 role ${expectedRole}`)
    }
    seen.add(target.metric)
  }
  if (seen.size !== ledgerRoles.size) {
    throw new KnifeObjectiveFunctionV2Error('LEDGER_BINDING_MISMATCH', 'objective targets must cover every Ledger@1 objective and regression metric exactly once')
  }
}

function validateMetricTarget(value: unknown): asserts value is KnifeObjectiveMetricTargetV2 {
  if (!isRecord(value)) invalidObjective('metric target must be an object')
  exactKeys(value, [
    'metric',
    'role',
    'direction',
    'target_interval',
    'minimum_improvement',
    'regression_limit',
    'evidence_class',
    'required',
  ], 'metric target')
  if (!isKnownMetric(value.metric)) invalidObjective('metric target metric is unsupported')
  if (!includes(KNIFE_OBJECTIVE_METRIC_ROLES_V2, value.role)) invalidObjective(`${value.metric} role is invalid`)
  if (!includes(KNIFE_OBJECTIVE_DIRECTION_V2, value.direction)) invalidObjective(`${value.metric} direction is invalid`)
  if (!isRecord(value.target_interval)
    || Object.keys(value.target_interval).sort().join('|') !== 'max|min'
    || typeof value.target_interval.min !== 'number'
    || typeof value.target_interval.max !== 'number'
    || !Number.isFinite(value.target_interval.min)
    || !Number.isFinite(value.target_interval.max)
    || value.target_interval.min > value.target_interval.max) {
    invalidObjective(`${value.metric} target_interval is invalid`)
  }
  if (typeof value.minimum_improvement !== 'number' || !Number.isFinite(value.minimum_improvement)
    || value.minimum_improvement < 0
    || (value.role === 'objective' && value.minimum_improvement === 0)) {
    invalidObjective(`${value.metric} minimum_improvement is invalid`)
  }
  if (typeof value.regression_limit !== 'number' || !Number.isFinite(value.regression_limit) || value.regression_limit < 0) {
    invalidObjective(`${value.metric} regression_limit is invalid`)
  }
  if (!includes(KNIFE_OBJECTIVE_EVIDENCE_CLASSES_V2, value.evidence_class)) invalidObjective(`${value.metric} evidence_class is invalid`)
  if (typeof value.required !== 'boolean') invalidObjective(`${value.metric} required must be boolean`)
}

function validateMetricValues(value: unknown, metrics: ReadonlySet<KnifeObjectiveMetric>, label: string): void {
  if (!isRecord(value)) invalidObjective(`${label} must be an object`)
  const keys = Object.keys(value).sort()
  const expected = [...metrics].sort()
  if (keys.length !== expected.length || keys.some((key, index) => key !== expected[index])) {
    invalidObjective(`${label} keys must exactly match metric_targets`)
  }
  for (const metric of expected) {
    const metricValue = value[metric]
    if (metricValue !== KNIFE_OBJECTIVE_NOT_COMPUTABLE
      && (typeof metricValue !== 'number' || !Number.isFinite(metricValue))) {
      invalidObjective(`${label}.${metric} must be finite or NOT_COMPUTABLE`)
    }
  }
}

function validateCandidateShape(candidate: unknown): asserts candidate is KnifeObjectiveCandidateV2 {
  if (!isRecord(candidate)) invalidCandidate('candidate must be an object')
  exactKeys(candidate as Record<string, unknown>, ['schema_version', 'candidate_id', 'candidate_sha256', 'values'], 'candidate', invalidCandidate)
  if (candidate.schema_version !== KNIFE_OBJECTIVE_CANDIDATE_V2_SCHEMA) invalidCandidate('candidate schema_version drifted')
  if (!isStableId(candidate.candidate_id)) invalidCandidate('candidate_id is invalid')
  if (!isSha(candidate.candidate_sha256)) invalidCandidate('candidate_sha256 is invalid')
}

function validateCandidateValues(value: unknown, metrics: ReadonlySet<KnifeObjectiveMetric>): void {
  if (!isRecord(value)) invalidCandidate('values must be an object')
  const keys = Object.keys(value).sort()
  const expected = [...metrics].sort()
  if (keys.length !== expected.length || keys.some((key, index) => key !== expected[index])) {
    invalidCandidate('candidate values keys must exactly match metric_targets')
  }
  for (const metric of expected) {
    const metricValue = value[metric]
    if (metricValue !== KNIFE_OBJECTIVE_NOT_COMPUTABLE
      && (typeof metricValue !== 'number' || !Number.isFinite(metricValue))) {
      invalidCandidate(`candidate values.${metric} must be finite or NOT_COMPUTABLE`)
    }
  }
}

function validateCandidateMetricValueShape(value: Record<string, unknown>): void {
  const keys = Object.keys(value)
  if (keys.length === 0 || keys.some((metric) => !isKnownMetric(metric))) {
    invalidCandidate('candidate values must contain known, non-empty metrics')
  }
  for (const metric of keys) {
    const metricValue = value[metric]
    if (metricValue !== KNIFE_OBJECTIVE_NOT_COMPUTABLE
      && (typeof metricValue !== 'number' || !Number.isFinite(metricValue))) {
      invalidCandidate(`candidate values.${metric} must be finite or NOT_COMPUTABLE`)
    }
  }
}

function evaluateCandidate(
  objective: KnifeObjectiveFunctionV2,
  candidate: KnifeObjectiveCandidateV2,
): KnifeObjectiveCandidateEvaluationV2 {
  const metrics = objective.metric_targets.map((target) => {
    const baseline = objective.baseline_values[target.metric] ?? KNIFE_OBJECTIVE_NOT_COMPUTABLE
    const candidateValue = candidate.values[target.metric] ?? KNIFE_OBJECTIVE_NOT_COMPUTABLE
    const computable = typeof baseline === 'number' && typeof candidateValue === 'number'
    const improvement: KnifeObjectiveValueV2 = computable
      ? target.direction === 'maximize' ? candidateValue - baseline : baseline - candidateValue
      : KNIFE_OBJECTIVE_NOT_COMPUTABLE
    const targetStatus: KnifeObjectiveMetricEvaluationStatusV2 = typeof baseline !== 'number' || typeof candidateValue !== 'number'
      ? KNIFE_OBJECTIVE_NOT_COMPUTABLE
      : candidateValue >= target.target_interval.min && candidateValue <= target.target_interval.max
        ? 'WITHIN_TARGET'
        : 'OUTSIDE_TARGET'
    const regressionStatus: KnifeObjectiveRegressionStatusV2 = typeof improvement !== 'number'
      ? KNIFE_OBJECTIVE_NOT_COMPUTABLE
      : improvement < -target.regression_limit ? 'REGRESSION_OVER_LIMIT' : 'WITHIN_LIMIT'
    return Object.freeze({
      schema_version: KNIFE_OBJECTIVE_METRIC_EVALUATION_V2_SCHEMA,
      metric: target.metric,
      role: target.role,
      direction: target.direction,
      evidence_class: target.evidence_class,
      required: target.required,
      target_interval: Object.freeze({ ...target.target_interval }),
      regression_limit: target.regression_limit,
      baseline_value: baseline,
      candidate_value: candidateValue,
      improvement,
      is_regression: typeof improvement === 'number' && improvement < 0,
      target_status: targetStatus,
      regression_status: regressionStatus,
    })
  })

  const required = metrics.filter((metric) => metric.required)
  const nonComputable = metrics.filter((metric) => metric.improvement === KNIFE_OBJECTIVE_NOT_COMPUTABLE)
  const requiredMetricsComputable = required.every((metric) => metric.improvement !== KNIFE_OBJECTIVE_NOT_COMPUTABLE)
  // Target intervals and minimum improvement are objective-role semantics.
  // Regression-role targets are retained for auditability but never become a
  // second objective or hidden scalar score.
  const objectiveMetrics = metrics.filter((metric) => metric.role === 'objective')
  const regressionMetrics = metrics.filter((metric) => metric.role === 'regression')
  const meetsTargets = objectiveMetrics.every((metric) => metric.target_status === 'WITHIN_TARGET' || (!metric.required && metric.target_status === KNIFE_OBJECTIVE_NOT_COMPUTABLE))
  // Regression metrics contribute only this hard no-regression gate.
  const meetsRegression = regressionMetrics.every((metric) => metric.regression_status === 'WITHIN_LIMIT' || (!metric.required && metric.regression_status === KNIFE_OBJECTIVE_NOT_COMPUTABLE))
  const improved = objectiveMetrics.filter((metric) => {
    const target = objective.metric_targets.find((item) => item.metric === metric.metric)!
    return typeof metric.improvement === 'number' && metric.improvement >= target.minimum_improvement
  })
  const meetsMinimum = improved.length > 0
  const computability: KnifeObjectiveComputabilityV2 = nonComputable.length === 0
    ? 'COMPUTABLE'
    : nonComputable.length === metrics.length
      ? KNIFE_OBJECTIVE_NOT_COMPUTABLE
      : 'PARTIAL'
  const objectiveGate: KnifeObjectiveCandidateGateV2 = !requiredMetricsComputable
    ? KNIFE_OBJECTIVE_NOT_COMPUTABLE
    : meetsTargets && meetsRegression && meetsMinimum
      ? 'ELIGIBLE'
      : 'REJECTED'

  return Object.freeze({
    candidate_id: candidate.candidate_id,
    candidate_sha256: candidate.candidate_sha256,
    metrics: Object.freeze(metrics),
    computability,
    objective_gate: objectiveGate,
    required_metrics_computable: requiredMetricsComputable,
    meets_target_intervals: meetsTargets,
    meets_regression_limits: meetsRegression,
    meets_minimum_improvement: meetsMinimum,
    improved_metrics: Object.freeze(improved.map((metric) => metric.metric)),
    regression_metrics: Object.freeze(metrics.filter((metric) => metric.is_regression).map((metric) => metric.metric)),
    non_computable_metrics: Object.freeze(nonComputable.map((metric) => metric.metric)),
    selection_eligible: objectiveGate === 'ELIGIBLE',
  })
}

function paretoFront(evaluations: readonly KnifeObjectiveCandidateEvaluationV2[]): KnifeObjectiveCandidateEvaluationV2[] {
  const eligible = evaluations.filter((evaluation) => evaluation.selection_eligible)
  return eligible.filter((candidate) => !eligible.some((other) => other !== candidate && dominates(other, candidate)))
}

function compareEvaluationsForSelection(
  left: KnifeObjectiveCandidateEvaluationV2,
  right: KnifeObjectiveCandidateEvaluationV2,
): number {
  // Selection tie-breaking follows objective-role evidence. Regression-role
  // metrics have already passed their hard constraint and cannot improve or
  // penalize the chosen Pareto member.
  const objectiveMissingCount = (value: KnifeObjectiveCandidateEvaluationV2): number =>
    value.metrics.filter((metric) => metric.role === 'objective' && metric.improvement === KNIFE_OBJECTIVE_NOT_COMPUTABLE).length
  return objectiveMissingCount(left) - objectiveMissingCount(right)
    || compareText(left.candidate_id, right.candidate_id)
    || compareText(left.candidate_sha256, right.candidate_sha256)
}

function compareCandidates(left: KnifeObjectiveCandidateV2, right: KnifeObjectiveCandidateV2): number {
  return compareText(left.candidate_id, right.candidate_id) || compareText(left.candidate_sha256, right.candidate_sha256)
}

function compareText(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0
}

function dominates(left: KnifeObjectiveCandidateEvaluationV2, right: KnifeObjectiveCandidateEvaluationV2): boolean {
  const leftByMetric = new Map(left.metrics.map((metric) => [metric.metric, metric]))
  const rightObjectiveMetrics = right.metrics.filter((metric) => metric.role === 'objective')
  const leftObjectiveMetrics = left.metrics.filter((metric) => metric.role === 'objective')
  const comparable = rightObjectiveMetrics.filter((metric) => {
    const other = leftByMetric.get(metric.metric)
    return other !== undefined && typeof other.improvement === 'number' && typeof metric.improvement === 'number'
  })
  // Missing optional evidence cannot be used as an advantage. Only compare
  // complete objective vectors; regression metrics are hard constraints, not
  // Pareto dimensions.
  if (comparable.length !== rightObjectiveMetrics.length || comparable.length !== leftObjectiveMetrics.length) return false
  let strict = false
  for (const rightMetric of comparable) {
    const leftMetric = leftByMetric.get(rightMetric.metric)!
    if ((leftMetric.improvement as number) < (rightMetric.improvement as number)) return false
    if ((leftMetric.improvement as number) > (rightMetric.improvement as number)) strict = true
  }
  return strict
}

function cloneRecord(value: Record<string, unknown>): KnifeObjectiveMetricValuesV2 {
  const clone: Record<string, unknown> = {}
  for (const [key, child] of Object.entries(value)) clone[key] = child
  return deepFreeze(clone) as KnifeObjectiveMetricValuesV2
}

function exactKeys(
  value: Record<string, unknown>,
  expected: readonly string[],
  label: string,
  fail: (message: string) => never = invalidObjective,
): void {
  const actual = Object.keys(value).sort()
  const wanted = [...expected].sort()
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) {
    fail(`${label} contains unknown, missing, or undefined keys`)
  }
}

function exactKeysAllowOptional(
  value: Record<string, unknown>,
  required: readonly string[],
  optional: readonly string[],
  label: string,
): void {
  const allowed = new Set([...required, ...optional])
  const actual = Object.keys(value)
  if (actual.some((key) => !allowed.has(key)) || required.some((key) => !Object.prototype.hasOwnProperty.call(value, key))) {
    invalidObjective(`${label} contains unknown, missing, or undefined keys`)
  }
}

function exactKeysAllowOptionalCandidate(
  value: Record<string, unknown>,
  required: readonly string[],
  optional: readonly string[],
): void {
  const allowed = new Set([...required, ...optional])
  const actual = Object.keys(value)
  if (actual.some((key) => !allowed.has(key)) || required.some((key) => !Object.prototype.hasOwnProperty.call(value, key))) {
    invalidCandidate('candidate contains unknown, missing, or undefined keys')
  }
}

function canonicalJson(value: unknown): string {
  if (value === null) return 'null'
  if (typeof value === 'string') return JSON.stringify(value)
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) invalidObjective('canonical JSON cannot contain non-finite numbers')
    return Object.is(value, -0) ? '0' : JSON.stringify(value)
  }
  if (Array.isArray(value)) return `[${value.map((item) => canonicalJson(item)).join(',')}]`
  if (isRecord(value)) {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`
  }
  invalidObjective('canonical JSON cannot contain undefined or executable values')
}

function deepFreeze<T>(value: T): T {
  if (!value || typeof value !== 'object' || Object.isFrozen(value)) return value
  for (const child of Object.values(value as Record<string, unknown>)) deepFreeze(child)
  return Object.freeze(value)
}

function isRecord(value: unknown): value is Record<string, any> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value)
}

function isKnownMetric(value: unknown): value is KnifeObjectiveMetric {
  return typeof value === 'string' && KNIFE_OBJECTIVE_METRICS.includes(value as KnifeObjectiveMetric)
}

function includes<T extends string>(values: readonly T[], value: unknown): value is T {
  return typeof value === 'string' && values.includes(value as T)
}

function isSha(value: unknown): value is string {
  return typeof value === 'string' && SHA256.test(value)
}

function isStableId(value: unknown): value is string {
  return typeof value === 'string' && STABLE_ID.test(value)
}

function isIntegerInRange(value: unknown, min: number, max: number): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= min && value <= max
}

function isFinitePositive(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
}

function invalidObjective(message: string): never {
  throw new KnifeObjectiveFunctionV2Error('INVALID_OBJECTIVE', message)
}

function invalidCandidate(message: string): never {
  throw new KnifeObjectiveFunctionV2Error('INVALID_CANDIDATE', message)
}
