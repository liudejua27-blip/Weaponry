import type { KnifeKnowledgeCandidatePlan, KnifeKnowledgeGoalWeights, KnifeKnowledgeMutationScope } from './knife-knowledge-candidate-generator.ts'
import {
  KNIFE_KNOWLEDGE_MUTATION_SCOPES,
  generateKnifeKnowledgeCandidatePlan,
  normalizeKnifeKnowledgeGoalWeights,
  validateKnifeKnowledgeNativeProgram,
} from './knife-knowledge-candidate-generator.ts'
import type { KnifeSceneProgram } from './knife-scene-program.ts'
import { sha256Hex } from './knife-browser-capture.ts'
import {
  KNIFE_OBJECTIVE_METRIC_IDS,
  type KnifeObjectiveMetricId,
} from './knife-objective-metric-catalog.ts'

/** Runtime-side closed objective contract for the Three.js knife route. */
export const KNIFE_OBJECTIVE_LEDGER_SCHEMA = 'KnifeObjectiveLedger@1' as const

export const KNIFE_OBJECTIVE_LEDGER_STAGES = Object.freeze([
  'blockout',
  'structural',
  'form',
  'material',
  'surface',
  'lighting',
  'interaction',
  'optimization',
] as const)
export type KnifeObjectiveLedgerStage = (typeof KNIFE_OBJECTIVE_LEDGER_STAGES)[number]

export const KNIFE_OBJECTIVE_METRICS = KNIFE_OBJECTIVE_METRIC_IDS
export type KnifeObjectiveMetric = KnifeObjectiveMetricId

export const KNIFE_OBJECTIVE_LEDGER_STATUSES = Object.freeze([
  'active',
  'accepted',
  'rejected',
  'plateau',
  'blocked',
  'budget-exhausted',
] as const)
export type KnifeObjectiveLedgerStatus = (typeof KNIFE_OBJECTIVE_LEDGER_STATUSES)[number]

export interface KnifeObjectiveLedger {
  readonly schema_version: typeof KNIFE_OBJECTIVE_LEDGER_SCHEMA
  readonly ledger_id: string
  readonly revision: number
  readonly parent_ledger_sha256: string | null
  readonly program_sha256: string
  readonly baseline_candidate_sha256: string
  readonly stage: KnifeObjectiveLedgerStage
  /** Semantic program Part IDs whose numeric paths may be searched. */
  readonly allowed_scope: readonly string[]
  /** Semantic program Part IDs whose full records must remain byte-equivalent. */
  readonly frozen_parts: readonly string[]
  readonly hypothesis: string
  readonly objective_metrics: readonly KnifeObjectiveMetric[]
  readonly regression_limits: readonly KnifeObjectiveMetric[]
  readonly candidate_budget: number
  readonly minimum_improvement: number
  readonly plateau_limit: 2
  readonly evidence_sha256: readonly string[]
  readonly status: KnifeObjectiveLedgerStatus
  /** Empty is permitted only for an in-memory draft; Runtime binding requires a digest. */
  readonly canonical_sha256: string
}

export type KnifeObjectiveLedgerDraft = Omit<KnifeObjectiveLedger, 'canonical_sha256'> & {
  readonly canonical_sha256?: string
}

export type KnifeObjectiveLedgerErrorCode =
  | 'INVALID_LEDGER'
  | 'CANONICAL_HASH_MISMATCH'
  | 'CANONICAL_HASH_REQUIRED'
  | 'PROGRAM_BINDING_MISMATCH'
  | 'UNKNOWN_PART'
  | 'SCOPE_CONFLICT'
  | 'NO_MUTABLE_SCOPE'
  | 'CANDIDATE_BUDGET_EXCEEDED'
  | 'SCOPE_VIOLATION'
  | 'FROZEN_PART_MUTATED'

export class KnifeObjectiveLedgerError extends Error {
  readonly code: KnifeObjectiveLedgerErrorCode

  constructor(code: KnifeObjectiveLedgerErrorCode, message: string) {
    super(`${code}: ${message}`)
    this.name = 'KnifeObjectiveLedgerError'
    this.code = code
  }
}

const SHA256 = /^[a-f0-9]{64}$/
const STABLE_ID = /^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/
const MAX_CANDIDATES = 32
const MAX_CANDIDATE_GENERATOR_COUNT = 4
const MIN_CANDIDATE_GENERATOR_COUNT = 2
const MAX_SEED = 0xffffffff

/**
 * The only semantic bridge from ledger Part IDs to the bounded TS mutation
 * grammar.  It is intentionally role-based and never accepts caller paths.
 */
export const KNIFE_OBJECTIVE_PART_MUTATION_SCOPES: Readonly<Record<string, readonly KnifeKnowledgeMutationScope[]>> = Object.freeze({
  'blade-body': Object.freeze(['blade-belly', 'blade-curvature', 'blade-tip-taper', 'blade-thickness'] as KnifeKnowledgeMutationScope[]),
  'cutting-edge': Object.freeze(['blade-curvature'] as KnifeKnowledgeMutationScope[]),
  guard: Object.freeze(['guard-jaw-gap', 'guard-horn-sweep'] as KnifeKnowledgeMutationScope[]),
  grip: Object.freeze(['grip-taper', 'grip-segment-rhythm'] as KnifeKnowledgeMutationScope[]),
  pommel: Object.freeze(['pommel-hook'] as KnifeKnowledgeMutationScope[]),
  relief: Object.freeze(['relief-depth'] as KnifeKnowledgeMutationScope[]),
})

export interface KnifeObjectiveLedgerCandidateGenerationOptions {
  /** Closed ten-key weights; omitted means equal positive weight for mapped scopes. */
  readonly goal_weights?: KnifeKnowledgeGoalWeights
  /** Must be 2–4 for the current candidate generator and <= ledger.candidate_budget. */
  readonly candidate_count?: number
  readonly seed?: number
}

/**
 * Explicit projection consumed by the bounded candidate generator.  Keeping
 * this object in the receipt makes the ledger boundary auditable.
 */
export interface KnifeObjectiveLedgerCandidateBinding {
  readonly schema_version: 'KnifeObjectiveLedgerCandidateBinding@1'
  readonly ledger_sha256: string
  readonly program_sha256: string
  readonly allowed_scope: readonly string[]
  readonly frozen_parts: readonly string[]
  readonly candidate_budget: number
  readonly mutation_scopes: readonly KnifeKnowledgeMutationScope[]
  readonly goal_weights: KnifeKnowledgeGoalWeights
  readonly candidate_count: number
  readonly seed: number
}

/** Validate the exact schema and, when present, the canonical SHA-256 field. */
export function validateKnifeObjectiveLedger(
  value: unknown,
  options: { readonly require_canonical_sha256?: boolean } = {},
): asserts value is KnifeObjectiveLedger {
  if (!isRecord(value)) invalid('ledger must be an object')
  exactKeys(value, [
    'schema_version', 'ledger_id', 'revision', 'parent_ledger_sha256', 'program_sha256',
    'baseline_candidate_sha256', 'stage', 'allowed_scope', 'frozen_parts', 'hypothesis',
    'objective_metrics', 'regression_limits', 'candidate_budget', 'minimum_improvement',
    'plateau_limit', 'evidence_sha256', 'status', 'canonical_sha256',
  ])
  if (value.schema_version !== KNIFE_OBJECTIVE_LEDGER_SCHEMA) invalid('schema_version drifted')
  if (!isStableId(value.ledger_id)) invalid('ledger_id is invalid')
  if (!isIntegerAtLeast(value.revision, 0)) invalid('revision is invalid')
  if (value.parent_ledger_sha256 !== null && !isSha(value.parent_ledger_sha256)) invalid('parent_ledger_sha256 is invalid')
  if (!isSha(value.program_sha256)) invalid('program_sha256 is invalid')
  if (!isSha(value.baseline_candidate_sha256)) invalid('baseline_candidate_sha256 is invalid')
  if (!includes(KNIFE_OBJECTIVE_LEDGER_STAGES, value.stage)) invalid('stage is unsupported')
  validateIdList(value.allowed_scope, 'allowed_scope', 1, 4)
  validateIdList(value.frozen_parts, 'frozen_parts', 0, Number.POSITIVE_INFINITY)
  if (intersection(value.allowed_scope, value.frozen_parts).length > 0) {
    throw new KnifeObjectiveLedgerError('SCOPE_CONFLICT', 'allowed_scope and frozen_parts must be disjoint')
  }
  if (typeof value.hypothesis !== 'string' || value.hypothesis.length < 8 || value.hypothesis.length > 300) invalid('hypothesis is invalid')
  validateMetricList(value.objective_metrics, 'objective_metrics', 1)
  validateMetricList(value.regression_limits, 'regression_limits', 0)
  if (!isIntegerInRange(value.candidate_budget, 1, MAX_CANDIDATES)) invalid('candidate_budget is outside [1,32]')
  if (typeof value.minimum_improvement !== 'number' || !Number.isFinite(value.minimum_improvement) || value.minimum_improvement <= 0 || value.minimum_improvement > 1) invalid('minimum_improvement is outside (0,1]')
  if (value.plateau_limit !== 2) invalid('plateau_limit must remain 2')
  validateShaList(value.evidence_sha256, 'evidence_sha256', 1, 32)
  if (!includes(KNIFE_OBJECTIVE_LEDGER_STATUSES, value.status)) invalid('status is unsupported')
  if (typeof value.canonical_sha256 !== 'string' || (value.canonical_sha256 !== '' && !isSha(value.canonical_sha256))) invalid('canonical_sha256 must be empty or lowercase SHA-256')
  const digest = canonicalKnifeObjectiveLedgerSha256(value)
  if (value.canonical_sha256 !== '' && value.canonical_sha256 !== digest) {
    throw new KnifeObjectiveLedgerError('CANONICAL_HASH_MISMATCH', 'canonical_sha256 does not match canonical JSON')
  }
  if (options.require_canonical_sha256 && value.canonical_sha256 === '') {
    throw new KnifeObjectiveLedgerError('CANONICAL_HASH_REQUIRED', 'Runtime binding requires a canonical_sha256')
  }
}

/** Construct and deeply freeze a canonical ledger. */
export function createKnifeObjectiveLedger(input: KnifeObjectiveLedgerDraft): KnifeObjectiveLedger {
  if (!isRecord(input)) invalid('ledger draft must be an object')
  const suppliedCanonical = input.canonical_sha256
  if (suppliedCanonical !== undefined && suppliedCanonical !== '' && !isSha(suppliedCanonical)) invalid('draft canonical_sha256 is invalid')
  const draft = { ...input, canonical_sha256: '' } as KnifeObjectiveLedger
  validateKnifeObjectiveLedger(draft)
  const canonical_sha256 = canonicalKnifeObjectiveLedgerSha256(draft)
  if (suppliedCanonical !== undefined && suppliedCanonical !== '' && suppliedCanonical !== canonical_sha256) {
    throw new KnifeObjectiveLedgerError('CANONICAL_HASH_MISMATCH', 'draft canonical_sha256 does not match canonical JSON')
  }
  const result = deepFreeze({ ...draft, canonical_sha256 })
  validateKnifeObjectiveLedger(result, { require_canonical_sha256: true })
  return result
}

/** SHA-256 of canonical JSON with the ledger's own digest blanked. */
export function canonicalKnifeObjectiveLedgerSha256(value: KnifeObjectiveLedger | Record<string, unknown>): string {
  if (!isRecord(value)) invalid('ledger must be an object')
  return sha256Hex(canonicalJson({ ...value, canonical_sha256: '' }))
}

/**
 * Map ledger boundaries to the closed candidate grammar.  The projection
 * refuses zero-weight fallbacks, program-frozen Parts, unknown IDs, and any
 * request that would exceed the ledger budget.
 */
export function mapKnifeObjectiveLedgerToCandidateGeneration(
  program: KnifeSceneProgram,
  ledger: KnifeObjectiveLedger,
  options: KnifeObjectiveLedgerCandidateGenerationOptions = {},
): KnifeObjectiveLedgerCandidateBinding {
  validateKnifeObjectiveLedger(ledger, { require_canonical_sha256: true })
  validateKnifeKnowledgeNativeProgram(program)
  // A non-empty program digest is Runtime-owned and may have been produced
  // before JSON parsing preserved numeric values but discarded numeric source
  // spelling (for example 1.0 versus 1). The in-process Studio therefore binds
  // that asserted Runtime digest; blank browser drafts use normalized JSON.
  if (program.canonical_sha256 !== '' && !isSha(program.canonical_sha256)) {
    throw new KnifeObjectiveLedgerError('PROGRAM_BINDING_MISMATCH', 'supplied program canonical_sha256 is malformed')
  }
  const programSha = program.canonical_sha256 || canonicalProgramSha256(program)
  if (ledger.program_sha256 !== programSha) {
    throw new KnifeObjectiveLedgerError('PROGRAM_BINDING_MISMATCH', 'ledger.program_sha256 does not bind the supplied program')
  }
  const partById = new Map(program.parts.map((part) => [part.part_id, part]))
  for (const partId of [...ledger.allowed_scope, ...ledger.frozen_parts]) {
    if (!partById.has(partId)) throw new KnifeObjectiveLedgerError('UNKNOWN_PART', `ledger references unknown program Part ${partId}`)
  }

  const allowed = new Set(ledger.allowed_scope)
  const frozen = new Set(ledger.frozen_parts)
  const mutable = (partId: string | undefined): boolean => {
    const part = partId === undefined ? undefined : partById.get(partId)
    return Boolean(part && allowed.has(part.part_id) && !frozen.has(part.part_id) && !part.frozen)
  }
  const partIdForRole = (role: KnifeSceneProgram['parts'][number]['role']): string | undefined =>
    program.parts.find((part) => part.role === role)?.part_id
  const bladeBodyId = partIdForRole('blade-body')
  const cuttingEdgeId = partIdForRole('cutting-edge')
  const guardId = program.assembly?.guard?.part_id
  const gripId = program.assembly?.grip?.part_id
  const pommelId = program.assembly?.pommel?.part_id
  const reliefParts = program.parts.filter((part) => part.role === 'relief')
  const allMutableReliefsAreAllowed = reliefParts.every((part) => part.frozen || frozen.has(part.part_id) || allowed.has(part.part_id))
  const scopes = KNIFE_KNOWLEDGE_MUTATION_SCOPES.filter((scope) => {
    switch (scope) {
      case 'blade-belly':
      case 'blade-tip-taper':
      case 'blade-thickness': return mutable(bladeBodyId)
      case 'blade-curvature': return mutable(bladeBodyId) && mutable(cuttingEdgeId)
      case 'guard-jaw-gap':
      case 'guard-horn-sweep': return mutable(guardId)
      case 'grip-taper':
      case 'grip-segment-rhythm': return mutable(gripId)
      case 'pommel-hook': return mutable(pommelId)
      case 'relief-depth': return reliefParts.some((part) => mutable(part.part_id)) && allMutableReliefsAreAllowed
    }
  })
  if (scopes.length === 0) throw new KnifeObjectiveLedgerError('NO_MUTABLE_SCOPE', 'ledger has no mutable, represented candidate scope')

  let requestedWeights: KnifeKnowledgeGoalWeights
  try {
    requestedWeights = options.goal_weights === undefined
      ? defaultGoalWeights(scopes)
      : normalizeKnifeKnowledgeGoalWeights(options.goal_weights)
  } catch (error) {
    throw new KnifeObjectiveLedgerError('INVALID_LEDGER', error instanceof Error ? error.message : String(error))
  }
  let scopedWeights: KnifeKnowledgeGoalWeights
  try {
    scopedWeights = scopeWeights(requestedWeights, scopes)
  } catch (error) {
    throw new KnifeObjectiveLedgerError('NO_MUTABLE_SCOPE', 'all ledger-mapped scopes have zero weight')
  }
  const positiveScopes = scopes.filter((scope) => scopedWeights[scope] > 0)
  // The ledger requires enough positively weighted mutable scopes before the
  // lower-level generator is invoked. No unrelated scope may fill the batch.
  if (positiveScopes.length < MIN_CANDIDATE_GENERATOR_COUNT) {
    throw new KnifeObjectiveLedgerError('NO_MUTABLE_SCOPE', 'at least two positive-weight mutable scopes are required')
  }
  const candidateCount = options.candidate_count ?? Math.min(MAX_CANDIDATE_GENERATOR_COUNT, ledger.candidate_budget, positiveScopes.length)
  if (!isIntegerInRange(candidateCount, MIN_CANDIDATE_GENERATOR_COUNT, MAX_CANDIDATE_GENERATOR_COUNT) || candidateCount > ledger.candidate_budget) {
    throw new KnifeObjectiveLedgerError('CANDIDATE_BUDGET_EXCEEDED', `candidate_count must be 2–4 and <= candidate_budget ${ledger.candidate_budget}`)
  }
  if (candidateCount > positiveScopes.length) {
    throw new KnifeObjectiveLedgerError('NO_MUTABLE_SCOPE', 'candidate_count would force a zero-weight scope')
  }
  const seed = options.seed ?? 0
  if (!isIntegerInRange(seed, 0, MAX_SEED)) throw new KnifeObjectiveLedgerError('INVALID_LEDGER', 'seed must be an integer in [0,2^32-1]')
  return deepFreeze({
    schema_version: 'KnifeObjectiveLedgerCandidateBinding@1' as const,
    ledger_sha256: ledger.canonical_sha256,
    program_sha256: programSha,
    allowed_scope: [...ledger.allowed_scope],
    frozen_parts: [...ledger.frozen_parts],
    candidate_budget: ledger.candidate_budget,
    mutation_scopes: [...positiveScopes],
    goal_weights: scopedWeights,
    candidate_count: candidateCount,
    seed,
  })
}

/** Generate review-only proposals under a validated ledger boundary. */
export function generateKnifeObjectiveLedgerCandidates(
  program: KnifeSceneProgram,
  ledger: KnifeObjectiveLedger,
  options: KnifeObjectiveLedgerCandidateGenerationOptions = {},
): KnifeKnowledgeCandidatePlan {
  const binding = mapKnifeObjectiveLedgerToCandidateGeneration(program, ledger, options)
  let plan: KnifeKnowledgeCandidatePlan
  try {
    plan = generateKnifeKnowledgeCandidatePlan(program, {
      goal_weights: binding.goal_weights,
      candidate_count: binding.candidate_count,
      seed: binding.seed,
    })
  } catch (error) {
    if (error instanceof KnifeObjectiveLedgerError) throw error
    throw new KnifeObjectiveLedgerError('NO_MUTABLE_SCOPE', error instanceof Error ? error.message : String(error))
  }
  if (plan.generated_candidate_count > ledger.candidate_budget) {
    throw new KnifeObjectiveLedgerError('CANDIDATE_BUDGET_EXCEEDED', 'generated candidates exceed ledger candidate_budget')
  }
  for (const candidate of plan.candidates) assertCandidateBoundary(program, candidate.program, ledger)
  return plan
}

function assertCandidateBoundary(program: KnifeSceneProgram, candidate: KnifeSceneProgram, ledger: KnifeObjectiveLedger): void {
  const changedPaths = diffPaths(program, candidate)
  const allowed = new Set(ledger.allowed_scope)
  for (const path of changedPaths) {
    const owner = ownerForPath(path, program)
    if (owner === null || !allowed.has(owner)) {
      throw new KnifeObjectiveLedgerError('SCOPE_VIOLATION', `candidate changed path outside allowed_scope: ${path}`)
    }
  }
  const candidateParts = new Map(candidate.parts.map((part) => [part.part_id, part]))
  const sourceParts = new Map(program.parts.map((part) => [part.part_id, part]))
  for (const partId of ledger.frozen_parts) {
    if (canonicalJson(sourceParts.get(partId)) !== canonicalJson(candidateParts.get(partId))) {
      throw new KnifeObjectiveLedgerError('FROZEN_PART_MUTATED', `candidate changed frozen Part ${partId}`)
    }
    const sourceAssembly = assemblyPartById(program, partId)
    const candidateAssembly = assemblyPartById(candidate, partId)
    if (sourceAssembly !== undefined || candidateAssembly !== undefined) {
      if (canonicalJson(sourceAssembly) !== canonicalJson(candidateAssembly)) {
        throw new KnifeObjectiveLedgerError('FROZEN_PART_MUTATED', `candidate changed frozen assembly Part ${partId}`)
      }
    }
  }
}

function ownerForPath(path: string, program: KnifeSceneProgram): string | null {
  if (/^blade_surface\.spine_curve\./.test(path) || /^blade_surface\.sections\[\d+\]\./.test(path)) return program.parts.find((part) => part.role === 'blade-body')?.part_id ?? null
  if (/^blade_surface\.cutting_edge_curve\./.test(path)) return program.parts.find((part) => part.role === 'cutting-edge')?.part_id ?? null
  if (/^assembly\.guard\./.test(path)) return program.assembly?.guard?.part_id ?? null
  if (/^assembly\.grip\./.test(path)) return program.assembly?.grip?.part_id ?? null
  if (/^assembly\.pommel\./.test(path)) return program.assembly?.pommel?.part_id ?? null
  const match = /^assembly\.(fasteners|gems|reliefs)\[(\d+)\]\./.exec(path)
  if (match) {
    const values = program.assembly?.[match[1] as 'fasteners' | 'gems' | 'reliefs']
    const item = values?.[Number(match[2])]
    return item?.part_id ?? null
  }
  return null
}

function assemblyPartById(program: KnifeSceneProgram, partId: string): unknown {
  const assembly = program.assembly
  if (!assembly) return undefined
  for (const key of ['guard', 'grip', 'pommel'] as const) if (assembly[key]?.part_id === partId) return assembly[key]
  for (const key of ['fasteners', 'gems', 'reliefs'] as const) {
    const item = assembly[key]?.find((candidate) => candidate.part_id === partId)
    if (item) return item
  }
  return undefined
}

function canonicalProgramSha256(program: KnifeSceneProgram): string {
  return sha256Hex(canonicalJson({ ...program, canonical_sha256: '' }))
}

function defaultGoalWeights(scopes: readonly KnifeKnowledgeMutationScope[]): KnifeKnowledgeGoalWeights {
  const values = Object.fromEntries(KNIFE_KNOWLEDGE_MUTATION_SCOPES.map((scope) => [scope, scopes.includes(scope) ? 1 : 0])) as KnifeKnowledgeGoalWeights
  return normalizeKnifeKnowledgeGoalWeights(values)
}

function scopeWeights(weights: KnifeKnowledgeGoalWeights, scopes: readonly KnifeKnowledgeMutationScope[]): KnifeKnowledgeGoalWeights {
  const selected = new Set(scopes)
  const values = Object.fromEntries(KNIFE_KNOWLEDGE_MUTATION_SCOPES.map((scope) => [scope, selected.has(scope) ? weights[scope] : 0])) as KnifeKnowledgeGoalWeights
  return normalizeKnifeKnowledgeGoalWeights(values)
}

function validateMetricList(value: unknown, label: string, minimum: number): asserts value is readonly KnifeObjectiveMetric[] {
  if (!Array.isArray(value) || value.length < minimum || value.length > 12 || new Set(value).size !== value.length || value.some((metric) => !includes(KNIFE_OBJECTIVE_METRICS, metric))) invalid(`${label} is invalid`)
}

function validateShaList(value: unknown, label: string, minimum: number, maximum: number): asserts value is readonly string[] {
  if (!Array.isArray(value) || value.length < minimum || value.length > maximum || new Set(value).size !== value.length || value.some((item) => !isSha(item))) invalid(`${label} is invalid`)
}

function validateIdList(value: unknown, label: string, minimum: number, maximum: number): asserts value is readonly string[] {
  if (!Array.isArray(value) || value.length < minimum || value.length > maximum || new Set(value).size !== value.length || value.some((item) => !isStableId(item))) invalid(`${label} is invalid`)
}

function exactKeys(value: Record<string, unknown>, expected: readonly string[]): void {
  const actual = Object.keys(value).sort()
  const wanted = [...expected].sort()
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) invalid('ledger contains unknown, missing, or undefined keys')
}

function intersection(left: readonly string[], right: readonly string[]): string[] {
  const rightSet = new Set(right)
  return left.filter((item) => rightSet.has(item))
}

function isRecord(value: unknown): value is Record<string, any> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value)
}

function isStableId(value: unknown): value is string {
  return typeof value === 'string' && STABLE_ID.test(value)
}

function isSha(value: unknown): value is string {
  return typeof value === 'string' && SHA256.test(value)
}

function isIntegerAtLeast(value: unknown, minimum: number): value is number {
  return Number.isInteger(value) && (value as number) >= minimum
}

function isIntegerInRange(value: unknown, minimum: number, maximum: number): value is number {
  return Number.isInteger(value) && (value as number) >= minimum && (value as number) <= maximum
}

function includes<T extends string>(values: readonly T[], value: unknown): value is T {
  return typeof value === 'string' && values.includes(value as T)
}

function invalid(message: string): never {
  throw new KnifeObjectiveLedgerError('INVALID_LEDGER', message)
}

function deepFreeze<T>(value: T): T {
  if (!value || typeof value !== 'object' || Object.isFrozen(value)) return value
  for (const child of Object.values(value as Record<string, unknown>)) deepFreeze(child)
  return Object.freeze(value)
}

function canonicalJson(value: unknown): string {
  if (value === null) return 'null'
  if (typeof value === 'string') return JSON.stringify(value)
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) invalid('canonical JSON cannot contain non-finite numbers')
    return Object.is(value, -0) ? '0' : JSON.stringify(value)
  }
  if (Array.isArray(value)) return `[${value.map((item) => canonicalJson(item)).join(',')}]`
  if (isRecord(value)) return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`
  invalid('canonical JSON cannot contain undefined or executable values')
}

function diffPaths(left: unknown, right: unknown, prefix = ''): string[] {
  if (isRecord(left) && isRecord(right)) {
    const paths: string[] = []
    for (const key of [...new Set([...Object.keys(left), ...Object.keys(right)])].sort()) {
      if (key === 'canonical_sha256') continue
      const path = prefix ? `${prefix}.${key}` : key
      if (!(key in left) || !(key in right)) paths.push(path)
      else paths.push(...diffPaths(left[key], right[key], path))
    }
    return paths
  }
  if (Array.isArray(left) && Array.isArray(right)) {
    const paths: string[] = []
    for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
      const path = `${prefix}[${index}]`
      if (index >= left.length || index >= right.length) paths.push(path)
      else paths.push(...diffPaths(left[index], right[index], path))
    }
    return paths
  }
  return left === right ? [] : [prefix]
}
