import type {
  KnifeDragonGuardSpec,
  KnifeGripSpec,
  KnifeHookedPommelSpec,
  KnifeSceneProgram,
  KnifeSection,
} from './knife-scene-program.ts'
import { validateImg2ThreeJsSourceEnvelope } from './img2threejs-source-envelope.ts'

/**
 * A deterministic, data-only candidate planner for the Three.js knife route.
 *
 * The planner is deliberately below the compiler boundary.  It copies a
 * native KnifeSceneProgram, applies one bounded semantic scope, and returns a
 * reviewable proposal.  It does not render, write Runtime/CAS state, or
 * interpret an img2threejs source envelope.
 */

export const KNIFE_KNOWLEDGE_CANDIDATE_PLAN_SCHEMA = 'KnifeKnowledgeCandidatePlan@1' as const
export const KNIFE_KNOWLEDGE_CANDIDATE_SCHEMA = 'KnifeKnowledgeCandidate@1' as const
export const KNIFE_KNOWLEDGE_PARAMETER_CHANGE_SCHEMA = 'KnifeKnowledgeParameterChange@1' as const
export const KNIFE_KNOWLEDGE_BUDGET_ESTIMATE_SCHEMA = 'KnifeKnowledgeBudgetEstimate@1' as const
export const KNIFE_KNOWLEDGE_HARD_BOUNDS_SCHEMA = 'KnifeKnowledgeHardBounds@1' as const

export const KNIFE_KNOWLEDGE_ID = 'crossfire-knife-visual-priors-v1' as const
export const KNIFE_KNOWLEDGE_ROUTE = 'weaponry-threejs-knife-studio' as const
export const KNIFE_KNOWLEDGE_SOURCE = 'skills/weaponry-threejs-knife-studio/references/crossfire-knife-knowledge.json' as const

export const KNIFE_KNOWLEDGE_MUTATION_SCOPES = Object.freeze([
  'blade-belly',
  'blade-curvature',
  'blade-tip-taper',
  'blade-thickness',
  'guard-jaw-gap',
  'guard-horn-sweep',
  'grip-taper',
  'grip-segment-rhythm',
  'pommel-hook',
  'relief-depth',
] as const)

export type KnifeKnowledgeMutationScope = (typeof KNIFE_KNOWLEDGE_MUTATION_SCOPES)[number]

export const KNIFE_KNOWLEDGE_GOAL_WEIGHT_KEYS = KNIFE_KNOWLEDGE_MUTATION_SCOPES
export type KnifeKnowledgeGoalWeightKey = KnifeKnowledgeMutationScope

/**
 * Closed normalized goal weights.  Every key is required, values are
 * non-negative finite numbers, and the planner normalizes them to sum to one.
 */
export type KnifeKnowledgeGoalWeights = Readonly<Record<KnifeKnowledgeGoalWeightKey, number>>

export const KNIFE_KNOWLEDGE_CANDIDATE_LIMITS = Object.freeze({
  min_candidate_count: 2,
  max_candidate_count: 4,
  max_seed: 0xffffffff,
  minimum_absolute_delta: 0.0001,
  max_parameter_paths_per_candidate: 32,
  max_rationale_length: 480,
  max_unknowns: 64,
} as const)

export interface KnifeKnowledgeHardBounds {
  readonly schema_version: typeof KNIFE_KNOWLEDGE_HARD_BOUNDS_SCHEMA
  readonly scope: KnifeKnowledgeMutationScope
  readonly value_min: number
  readonly value_max: number
  readonly max_abs_delta: number
  readonly formula: string
}

/**
 * Hard bounds are part of the planner's closed contract.  They are visual
 * normalized-unit limits, never real-world dimensions.
 */
export const KNIFE_KNOWLEDGE_CANDIDATE_HARD_BOUNDS: Readonly<Record<KnifeKnowledgeMutationScope, KnifeKnowledgeHardBounds>> = Object.freeze({
  'blade-belly': Object.freeze({
    schema_version: KNIFE_KNOWLEDGE_HARD_BOUNDS_SCHEMA,
    scope: 'blade-belly',
    value_min: 0.0005,
    value_max: 2,
    max_abs_delta: 0.08,
    formula: 'half_width_belly := clamp(half_width_belly * (1 + delta), 0.0005, 2)',
  }),
  'blade-curvature': Object.freeze({
    schema_version: KNIFE_KNOWLEDGE_HARD_BOUNDS_SCHEMA,
    scope: 'blade-curvature',
    value_min: -4,
    value_max: 4,
    max_abs_delta: 0.06,
    formula: 'paired_lateral_curve_y := clamp(curve_y +/- delta, -4, 4) and spine_y > edge_y',
  }),
  'blade-tip-taper': Object.freeze({
    schema_version: KNIFE_KNOWLEDGE_HARD_BOUNDS_SCHEMA,
    scope: 'blade-tip-taper',
    value_min: 0.0005,
    value_max: 2,
    max_abs_delta: 0.08,
    formula: 'half_width_tip := clamp(half_width_tip * factor, 0.0005, min(2, 0.9 * half_width_belly))',
  }),
  'blade-thickness': Object.freeze({
    schema_version: KNIFE_KNOWLEDGE_HARD_BOUNDS_SCHEMA,
    scope: 'blade-thickness',
    value_min: 0.0005,
    value_max: 1,
    max_abs_delta: 0.025,
    formula: 'thickness_belly := clamp(thickness_belly * (1 + delta), 0.0005, 1)',
  }),
  'guard-jaw-gap': Object.freeze({
    schema_version: KNIFE_KNOWLEDGE_HARD_BOUNDS_SCHEMA,
    scope: 'guard-jaw-gap',
    value_min: 0.01,
    value_max: 0.6,
    max_abs_delta: 0.025,
    formula: 'jaw_gap := clamp(jaw_gap * (1 + delta), 0.01, min(0.6, guard_span))',
  }),
  'guard-horn-sweep': Object.freeze({
    schema_version: KNIFE_KNOWLEDGE_HARD_BOUNDS_SCHEMA,
    scope: 'guard-horn-sweep',
    value_min: -0.75,
    value_max: 0.75,
    max_abs_delta: 0.08,
    formula: 'horn_sweep := clamp(horn_sweep + delta, -0.75, 0.75)',
  }),
  'grip-taper': Object.freeze({
    schema_version: KNIFE_KNOWLEDGE_HARD_BOUNDS_SCHEMA,
    scope: 'grip-taper',
    value_min: -0.9,
    value_max: 0.9,
    max_abs_delta: 0.12,
    formula: 'grip_taper := clamp(grip_taper + delta, -0.9, 0.9)',
  }),
  'grip-segment-rhythm': Object.freeze({
    schema_version: KNIFE_KNOWLEDGE_HARD_BOUNDS_SCHEMA,
    scope: 'grip-segment-rhythm',
    value_min: 0.5,
    value_max: 1.5,
    max_abs_delta: 0.08,
    formula: 'segment_radius_scale_i := clamp(segment_radius_scale_i + alternating_delta, 0.5, 1.5)',
  }),
  'pommel-hook': Object.freeze({
    schema_version: KNIFE_KNOWLEDGE_HARD_BOUNDS_SCHEMA,
    scope: 'pommel-hook',
    value_min: 0.2,
    value_max: 1,
    max_abs_delta: 0.08,
    formula: 'hook_bend := clamp(hook_bend + delta, 0.2, 1)',
  }),
  'relief-depth': Object.freeze({
    schema_version: KNIFE_KNOWLEDGE_HARD_BOUNDS_SCHEMA,
    scope: 'relief-depth',
    value_min: 0.0005,
    value_max: 1,
    max_abs_delta: 0.025,
    formula: 'relief_depth := clamp(relief_depth * (1 + delta), 0.0005, 1)',
  }),
})

export interface KnifeKnowledgeParameterBounds {
  readonly min: number
  readonly max: number
}

export interface KnifeKnowledgeParameterChange {
  readonly schema_version: typeof KNIFE_KNOWLEDGE_PARAMETER_CHANGE_SCHEMA
  readonly path: string
  readonly old_value: number
  readonly new_value: number
  readonly delta: number
  readonly hard_bounds: KnifeKnowledgeParameterBounds
  readonly source_rationale: string
  readonly source_refs: readonly string[]
}

export interface KnifeKnowledgeBudgetEstimate {
  readonly schema_version: typeof KNIFE_KNOWLEDGE_BUDGET_ESTIMATE_SCHEMA
  readonly estimator: 'closed-program-heuristic@1'
  readonly estimated_triangles: number
  readonly estimated_draw_calls: number
  readonly estimated_texture_bytes: number
  readonly max_triangles: number
  readonly max_draw_calls: number
  readonly max_texture_bytes: number
  readonly within_budget: boolean
  readonly rationale: string
}

export interface KnifeKnowledgeCandidate {
  readonly schema_version: typeof KNIFE_KNOWLEDGE_CANDIDATE_SCHEMA
  readonly candidate_id: string
  readonly ordinal: number
  readonly mutation_scope: KnifeKnowledgeMutationScope
  readonly changed_parameter_paths: readonly string[]
  readonly changes: readonly KnifeKnowledgeParameterChange[]
  readonly source_rationale: string
  readonly source_refs: readonly string[]
  readonly budget_estimate: KnifeKnowledgeBudgetEstimate
  readonly candidate_program_fingerprint: string
  readonly program: KnifeSceneProgram
  readonly proposal_status: 'REVIEW_ONLY'
}

export type KnifeKnowledgeCandidatePlanStatus = 'PROPOSALS_READY' | 'REVIEW_ONLY' | 'REJECTED'

export interface KnifeKnowledgeCandidatePlan {
  readonly schema_version: typeof KNIFE_KNOWLEDGE_CANDIDATE_PLAN_SCHEMA
  readonly route: typeof KNIFE_KNOWLEDGE_ROUTE
  readonly knowledge_id: typeof KNIFE_KNOWLEDGE_ID
  readonly knowledge_source: typeof KNIFE_KNOWLEDGE_SOURCE
  readonly source_program_fingerprint: string
  readonly source_program_canonical_sha256: string
  readonly goal_weights: KnifeKnowledgeGoalWeights
  readonly seed: number
  readonly requested_candidate_count: number
  readonly generated_candidate_count: number
  readonly candidate_budget: number
  readonly mutation_scopes: readonly KnifeKnowledgeMutationScope[]
  readonly hard_bounds: Readonly<Record<KnifeKnowledgeMutationScope, KnifeKnowledgeHardBounds>>
  readonly candidates: readonly KnifeKnowledgeCandidate[]
  readonly status: KnifeKnowledgeCandidatePlanStatus
  readonly direct_mutation_performed: false
  readonly source_envelope_policy: 'REVIEW_ONLY_NO_DIRECT_MUTATION@1'
  readonly rejection_reason: string | null
  readonly deterministic_fingerprint: string
}

export interface KnifeKnowledgeCandidateGenerationOptions {
  readonly goal_weights: KnifeKnowledgeGoalWeights
  readonly candidate_count?: number
  readonly seed?: number
}

export class KnifeKnowledgeCandidatePlanError extends Error {
  readonly code: KnifeKnowledgeCandidatePlanErrorCode

  constructor(code: KnifeKnowledgeCandidatePlanErrorCode, message: string) {
    super(`${code}: ${message}`)
    this.name = 'KnifeKnowledgeCandidatePlanError'
    this.code = code
  }
}

export type KnifeKnowledgeCandidatePlanErrorCode =
  | 'INVALID_PROGRAM'
  | 'INVALID_GOAL_WEIGHTS'
  | 'INVALID_OPTIONS'
  | 'SOURCE_ENVELOPE_REVIEW_ONLY'
  | 'NO_MUTABLE_SCOPE'
  | 'INSUFFICIENT_MUTABLE_SCOPE'
  | 'BUDGET_EXCEEDED'
  | 'DUPLICATE_CANDIDATE'
  | 'INVALID_PLAN'

interface MutationResult {
  readonly program: KnifeSceneProgram
  readonly changes: readonly KnifeKnowledgeParameterChange[]
  readonly source_rationale: string
  readonly source_refs: readonly string[]
}

interface ScopeAvailability {
  readonly scope: KnifeKnowledgeMutationScope
  readonly weight: number
  readonly available: boolean
  readonly reason: string
}

const SOURCE_ENVELOPE_POLICY = 'REVIEW_ONLY_NO_DIRECT_MUTATION@1' as const
const MIN_PROGRAM_SHA = /^[a-f0-9]{64}$/
const STABLE_ID = /^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/
const MAX_PROGRAM_COORDINATE = 4
const MAX_CURVE_POINTS = 64
const MIN_VALUE = 0.0005

const RATIONALES: Readonly<Record<KnifeKnowledgeMutationScope, { readonly text: string; readonly refs: readonly string[] }>> = Object.freeze({
  'blade-belly': Object.freeze({
    text: 'Forward belly dominance is a bounded design prior for a readable kukri-inspired silhouette; it ranks a section-width change and does not assert hidden reference geometry.',
    refs: Object.freeze(['claim:kukri-forward-belly', 'formula:belly-dominance']),
  }),
  'blade-curvature': Object.freeze({
    text: 'A sparse spine/edge arc with bounded lateral curvature supports a strong sweep without introducing high-frequency waviness; the paired curves remain independently addressable.',
    refs: Object.freeze(['curve-family:kukri-spine-arc', 'formula:kukri-spine-regularity']),
  }),
  'blade-tip-taper': Object.freeze({
    text: 'Tip convergence is ranked through both independent boundary curves and a positive terminal section; the bounded taper never turns a hidden side into a fact.',
    refs: Object.freeze(['claim:tip-convergence-prior', 'formula:tip-narrower']),
  }),
  'blade-thickness': Object.freeze({
    text: 'Positive section thickness is a hard loft sanity rule; this small normalized change explores readable thickness continuity without claiming a physical specification.',
    refs: Object.freeze(['formula:positive-thickness', 'formula:section-thickness-positive']),
  }),
  'guard-jaw-gap': Object.freeze({
    text: 'A dragon guard reads through separate jaw and negative-space cues; jaw gap is changed inside a bounded normalized interval and remains review-only.',
    refs: Object.freeze(['claim:dragon-guard-negative-space', 'metric:negative-space-visibility']),
  }),
  'guard-horn-sweep': Object.freeze({
    text: 'Horn sweep is a bounded guard detail supporting distinct semantic components; it cannot compensate for a failed guard silhouette or establish hidden anatomy.',
    refs: Object.freeze(['claim:dragon-guard-negative-space', 'assembly:guard-negative-space']),
  }),
  'grip-taper': Object.freeze({
    text: 'A moderate grip taper keeps the attachment subordinate while preserving a stable centerline and visual continuity; normalized ratios are soft priors only.',
    refs: Object.freeze(['claim:segmented-grip-readability', 'ratio:grip-length-to-blade-length']),
  }),
  'grip-segment-rhythm': Object.freeze({
    text: 'Alternating wrap-band radius scales provide a finite grip rhythm while retaining stable segment IDs; backside segmentation remains inferred.',
    refs: Object.freeze(['claim:segmented-grip-readability', 'formula:section-order']),
  }),
  'pommel-hook': Object.freeze({
    text: 'A hooked pommel benefits from a distinct return cue; bend is varied within the closed hook interval while the gem seat and hidden cross-section remain unchanged.',
    refs: Object.freeze(['claim:hooked-pommel-cue', 'ratio:pommel-width-to-grip-width']),
  }),
  'relief-depth': Object.freeze({
    text: 'Relief depth remains an inset/readability cue bounded inside the local blade envelope; it cannot repair silhouette, topology, or material ownership.',
    refs: Object.freeze(['curve-family:relief-fuller-run', 'formula:relief-inset']),
  }),
})

/**
 * Normalize and validate the closed goal-weight object.  Unknown keys,
 * negative values, non-finite values, and a zero total are rejected.
 */
export function normalizeKnifeKnowledgeGoalWeights(value: unknown): KnifeKnowledgeGoalWeights {
  if (!isRecord(value)) throw new KnifeKnowledgeCandidatePlanError('INVALID_GOAL_WEIGHTS', 'goal_weights must be a closed object')
  const keys = Object.keys(value).sort()
  const expected = [...KNIFE_KNOWLEDGE_GOAL_WEIGHT_KEYS].sort()
  if (keys.length !== expected.length || keys.some((key, index) => key !== expected[index])) {
    throw new KnifeKnowledgeCandidatePlanError('INVALID_GOAL_WEIGHTS', 'goal_weights keys must exactly match the closed mutation scopes')
  }
  const total = KNIFE_KNOWLEDGE_GOAL_WEIGHT_KEYS.reduce((sum, key) => {
    const weight = value[key]
    if (typeof weight !== 'number' || !Number.isFinite(weight) || weight < 0 || weight > 1) {
      throw new KnifeKnowledgeCandidatePlanError('INVALID_GOAL_WEIGHTS', `${key} weight must be finite in [0,1]`)
    }
    return sum + weight
  }, 0)
  if (!(total > 0) || !Number.isFinite(total)) {
    throw new KnifeKnowledgeCandidatePlanError('INVALID_GOAL_WEIGHTS', 'goal_weights must have a positive finite total')
  }
  const normalized: Record<string, number> = {}
  let running = 0
  for (const key of KNIFE_KNOWLEDGE_GOAL_WEIGHT_KEYS) {
    const ratio = round12((value[key] as number) / total)
    normalized[key] = ratio
    running += ratio
  }
  // Correct only the final representable rounding residue, keeping sum == 1.
  // The recipient must itself be positive in the caller's closed scope. Using
  // the vocabulary's final key injected a tiny signed residue into a disabled
  // scope and could make a valid subset fail normalization.
  const finalKey = [...KNIFE_KNOWLEDGE_GOAL_WEIGHT_KEYS]
    .reverse()
    .find((key) => (value[key] as number) > 0)!
  normalized[finalKey] = round12(normalized[finalKey] + (1 - running))
  if (normalized[finalKey] < 0 || normalized[finalKey] > 1) {
    throw new KnifeKnowledgeCandidatePlanError('INVALID_GOAL_WEIGHTS', 'normalized goal_weights left the closed [0,1] interval')
  }
  return deepFreeze(normalized as KnifeKnowledgeGoalWeights)
}

/** Validate a native program without allocating a candidate. */
export function validateKnifeKnowledgeNativeProgram(program: unknown): asserts program is KnifeSceneProgram {
  validateProgramShape(program)
  const source = program as KnifeSceneProgram
  if (source.source_envelope !== undefined) {
    throw new KnifeKnowledgeCandidatePlanError('SOURCE_ENVELOPE_REVIEW_ONLY', 'source_envelope compatibility programs are REVIEW_ONLY and cannot be directly mutated')
  }
  if (source.design_basis === 'img2threejs-compatible-import') {
    throw new KnifeKnowledgeCandidatePlanError('SOURCE_ENVELOPE_REVIEW_ONLY', 'img2threejs-compatible-import programs are REVIEW_ONLY and cannot be directly mutated')
  }
}

/**
 * Generate 2–4 deterministic, one-scope, review-only native candidates.
 *
 * A compatibility/source-envelope program returns a closed REVIEW_ONLY plan
 * with no candidate instead of silently mutating upstream geometry.
 */
export function generateKnifeKnowledgeCandidatePlan(
  program: KnifeSceneProgram,
  options: KnifeKnowledgeCandidateGenerationOptions,
): KnifeKnowledgeCandidatePlan
export function generateKnifeKnowledgeCandidatePlan(
  program: KnifeSceneProgram,
  goal_weights: KnifeKnowledgeGoalWeights,
  options?: Omit<KnifeKnowledgeCandidateGenerationOptions, 'goal_weights'>,
): KnifeKnowledgeCandidatePlan
export function generateKnifeKnowledgeCandidatePlan(
  program: KnifeSceneProgram,
  optionsOrWeights: KnifeKnowledgeCandidateGenerationOptions | KnifeKnowledgeGoalWeights,
  legacyOptions: Omit<KnifeKnowledgeCandidateGenerationOptions, 'goal_weights'> = {},
): KnifeKnowledgeCandidatePlan {
  validateProgramShape(program)
  const options = isGenerationOptions(optionsOrWeights)
    ? optionsOrWeights
    : { ...legacyOptions, goal_weights: optionsOrWeights }
  validateGenerationOptions(options)
  const candidateCount = validateCandidateCount(options.candidate_count ?? 3)
  const seed = validateSeed(options.seed ?? 0)
  const goalWeights = normalizeKnifeKnowledgeGoalWeights(options.goal_weights)
  const sourceFingerprint = fingerprintProgram(program)
  const sourceCanonical = canonicalProgramSha256(program)

  if (program.source_envelope !== undefined || program.design_basis === 'img2threejs-compatible-import') {
    return makePlan({
      program,
      sourceFingerprint,
      sourceCanonical,
      goalWeights,
      candidateCount,
      seed,
      candidates: [],
      status: 'REVIEW_ONLY',
      rejectionReason: 'source_envelope compatibility input is preserved as upstream truth; direct mutation is rejected',
    })
  }

  validateKnifeKnowledgeNativeProgram(program)
  const availability = KNIFE_KNOWLEDGE_MUTATION_SCOPES.map((scope) => scopeAvailability(program, scope, goalWeights[scope]))
  const mutable = availability.filter((item) => item.available)
  if (mutable.length === 0) {
    throw new KnifeKnowledgeCandidatePlanError('NO_MUTABLE_SCOPE', 'native program has no mutable knowledge scope')
  }
  const positive = mutable.filter((item) => item.weight > 0)
  // A candidate must always be backed by a positive-weight mutable scope.
  // Falling back to all mutable scopes here would silently introduce an
  // unrelated zero-weight mutation when the requested candidate budget cannot
  // be satisfied by the selected objective.
  if (positive.length < candidateCount) {
    throw new KnifeKnowledgeCandidatePlanError(
      'INSUFFICIENT_MUTABLE_SCOPE',
      `only ${positive.length} positive-weight mutable scopes are available for ${candidateCount} candidates`,
    )
  }
  const ranked = rankScopes(positive, seed)
  if (ranked.length === 0) throw new KnifeKnowledgeCandidatePlanError('NO_MUTABLE_SCOPE', 'goal weights selected no mutable knowledge scope')

  const candidates: KnifeKnowledgeCandidate[] = []
  const fingerprints = new Set<string>()
  for (let ordinal = 0; ordinal < candidateCount; ordinal += 1) {
    const scope = ranked[ordinal % ranked.length].scope
    const repetition = Math.floor(ordinal / ranked.length)
    const mutation = mutateScope(program, scope, repetition, seed)
    const candidateProgram = mutation.program
    const candidateFingerprint = fingerprintCandidateProgram(candidateProgram, scope, mutation.changes)
    if (fingerprints.has(candidateFingerprint)) {
      throw new KnifeKnowledgeCandidatePlanError('DUPLICATE_CANDIDATE', `candidate ${ordinal + 1} repeated an earlier program fingerprint`)
    }
    fingerprints.add(candidateFingerprint)
    const budgetEstimate = estimateKnifeKnowledgeCandidateBudget(candidateProgram, scope)
    if (!budgetEstimate.within_budget) {
      throw new KnifeKnowledgeCandidatePlanError('BUDGET_EXCEEDED', `candidate ${ordinal + 1} exceeds the native program budget`)
    }
    const candidateId = `knowledge-candidate-${String(ordinal + 1).padStart(2, '0')}-${candidateFingerprint.slice(0, 12)}`
    const candidate: KnifeKnowledgeCandidate = {
      schema_version: KNIFE_KNOWLEDGE_CANDIDATE_SCHEMA,
      candidate_id: candidateId,
      ordinal: ordinal + 1,
      mutation_scope: scope,
      changed_parameter_paths: Object.freeze(mutation.changes.map((change) => change.path)),
      changes: Object.freeze([...mutation.changes]),
      source_rationale: mutation.source_rationale,
      source_refs: mutation.source_refs,
      budget_estimate: budgetEstimate,
      candidate_program_fingerprint: candidateFingerprint,
      program: candidateProgram,
      proposal_status: 'REVIEW_ONLY',
    }
    candidates.push(deepFreeze(candidate))
  }

  return makePlan({
    program,
    sourceFingerprint,
    sourceCanonical,
    goalWeights,
    candidateCount,
    seed,
    candidates,
    status: 'PROPOSALS_READY',
    rejectionReason: null,
  })
}

/** Alias kept intentionally data-only for callers that name the operation by its output. */
export const createKnifeKnowledgeCandidatePlan = generateKnifeKnowledgeCandidatePlan

/** Generate candidates and return the closed plan; no approval is implied. */
export const generateKnifeKnowledgeCandidates = generateKnifeKnowledgeCandidatePlan

/** Return a stable fingerprint for a program while ignoring its own canonical field. */
export function fingerprintKnifeKnowledgeProgram(program: KnifeSceneProgram): string {
  validateProgramShape(program)
  return fingerprintProgram(program)
}

/** Return the Runtime-owned canonical field without recomputing or rewriting it. */
export function canonicalKnifeKnowledgeProgramSha256(program: KnifeSceneProgram): string {
  validateProgramShape(program)
  return canonicalProgramSha256(program)
}

/** Estimate a candidate's bounded structural budget without invoking a compiler. */
export function estimateKnifeKnowledgeCandidateBudget(
  program: KnifeSceneProgram,
  scope: KnifeKnowledgeMutationScope = 'blade-belly',
): KnifeKnowledgeBudgetEstimate {
  validateProgramShape(program)
  if (!isMutationScope(scope)) throw new KnifeKnowledgeCandidatePlanError('INVALID_OPTIONS', 'budget scope is outside the closed mutation scopes')
  const partCount = program.parts.length
  const sectionCount = program.blade_surface.sections.length
  const curvePointCount = program.blade_surface.spine_curve.control_points.length
    + program.blade_surface.cutting_edge_curve.control_points.length
  const assemblyCount = assemblyPrimitiveCount(program)
  // This is an intentionally conservative planning estimate, not a compiler receipt.
  const baseTriangles = (sectionCount - 1) * 64 * 8 + curvePointCount * 12 + assemblyCount * 96
  const scopeOverhead = scope === 'blade-curvature' ? 32 : scope === 'relief-depth' ? 24 : 8
  const estimatedTriangles = Math.max(64, Math.ceil(baseTriangles + scopeOverhead))
  const estimatedDrawCalls = Math.max(1, partCount + Math.ceil(assemblyCount / 4))
  const estimatedTextureBytes = 0
  const withinBudget = estimatedTriangles <= program.budgets.max_triangles
    && estimatedDrawCalls <= program.budgets.max_draw_calls
    && estimatedTextureBytes <= program.budgets.max_texture_bytes
  return deepFreeze({
    schema_version: KNIFE_KNOWLEDGE_BUDGET_ESTIMATE_SCHEMA,
    estimator: 'closed-program-heuristic@1',
    estimated_triangles: estimatedTriangles,
    estimated_draw_calls: estimatedDrawCalls,
    estimated_texture_bytes: estimatedTextureBytes,
    max_triangles: program.budgets.max_triangles,
    max_draw_calls: program.budgets.max_draw_calls,
    max_texture_bytes: program.budgets.max_texture_bytes,
    within_budget: withinBudget,
    rationale: 'Estimate uses only bounded program cardinalities; compiler, renderer, quality, and engine receipts remain separate.',
  })
}

/** Validate the closed plan shape and all candidate lineage bindings. */
export function validateKnifeKnowledgeCandidatePlan(value: unknown): asserts value is KnifeKnowledgeCandidatePlan {
  if (!isRecord(value)) throw new KnifeKnowledgeCandidatePlanError('INVALID_PLAN', 'plan must be an object')
  exactKeys(value, [
    'schema_version', 'route', 'knowledge_id', 'knowledge_source', 'source_program_fingerprint',
    'source_program_canonical_sha256', 'goal_weights', 'seed', 'requested_candidate_count',
    'generated_candidate_count', 'candidate_budget', 'mutation_scopes', 'hard_bounds', 'candidates',
    'status', 'direct_mutation_performed', 'source_envelope_policy', 'rejection_reason', 'deterministic_fingerprint',
  ], 'plan')
  if (value.schema_version !== KNIFE_KNOWLEDGE_CANDIDATE_PLAN_SCHEMA) failPlan('plan schema_version drifted')
  if (value.route !== KNIFE_KNOWLEDGE_ROUTE || value.knowledge_id !== KNIFE_KNOWLEDGE_ID || value.knowledge_source !== KNIFE_KNOWLEDGE_SOURCE) failPlan('plan knowledge binding drifted')
  fingerprintText(value.source_program_fingerprint, 'source_program_fingerprint')
  shaText(value.source_program_canonical_sha256, 'source_program_canonical_sha256', true)
  const weights = normalizeKnifeKnowledgeGoalWeights(value.goal_weights)
  if (canonicalJson(weights) !== canonicalJson(value.goal_weights)) failPlan('goal_weights are not normalized')
  validateSeed(value.seed)
  validateCandidateCount(value.requested_candidate_count)
  if (value.candidate_budget !== value.requested_candidate_count) failPlan('candidate_budget must equal requested_candidate_count')
  if (!Number.isInteger(value.generated_candidate_count) || value.generated_candidate_count < 0 || value.generated_candidate_count > value.requested_candidate_count) failPlan('generated_candidate_count is outside [0,requested_candidate_count]')
  if (!Array.isArray(value.mutation_scopes) || value.mutation_scopes.some((scope) => !isMutationScope(scope))) failPlan('mutation_scopes are not closed')
  if (!isRecord(value.hard_bounds)) failPlan('hard_bounds must be a closed object')
  exactKeys(value.hard_bounds, KNIFE_KNOWLEDGE_MUTATION_SCOPES, 'hard_bounds')
  for (const scope of KNIFE_KNOWLEDGE_MUTATION_SCOPES) validateHardBounds(value.hard_bounds[scope], scope)
  if (!Array.isArray(value.candidates) || value.candidates.length !== value.generated_candidate_count) failPlan('candidate count does not match generated_candidate_count')
  if (value.status !== 'PROPOSALS_READY' && value.status !== 'REVIEW_ONLY' && value.status !== 'REJECTED') failPlan('status is unsupported')
  if (value.direct_mutation_performed !== false || value.source_envelope_policy !== SOURCE_ENVELOPE_POLICY) failPlan('source-envelope policy drifted')
  if (value.rejection_reason !== null && (typeof value.rejection_reason !== 'string' || value.rejection_reason.length === 0)) failPlan('rejection_reason must be null or bounded text')
  if (value.mutation_scopes.length !== value.generated_candidate_count || value.mutation_scopes.some((scope, index) => scope !== value.candidates[index]?.mutation_scope)) failPlan('mutation_scopes do not match candidates')
  if (value.status === 'PROPOSALS_READY' && (value.generated_candidate_count < 2 || value.rejection_reason !== null)) failPlan('PROPOSALS_READY requires 2-4 candidates and no rejection reason')
  if ((value.status === 'REVIEW_ONLY' || value.status === 'REJECTED') && (value.generated_candidate_count !== 0 || value.rejection_reason === null)) failPlan(`${value.status} requires zero candidates and a rejection reason`)
  const fingerprints = new Set<string>()
  for (const [index, candidate] of value.candidates.entries()) {
    if (!isRecord(candidate) || candidate.ordinal !== index + 1) failPlan('candidate ordinals must be contiguous and ordered')
    validateCandidate(candidate, value.source_program_fingerprint, fingerprints)
  }
  fingerprintText(value.deterministic_fingerprint, 'deterministic_fingerprint')
  if (value.deterministic_fingerprint !== fingerprintPlan(value)) failPlan('deterministic_fingerprint does not match plan contents')
}

function isGenerationOptions(value: KnifeKnowledgeCandidateGenerationOptions | KnifeKnowledgeGoalWeights): value is KnifeKnowledgeCandidateGenerationOptions {
  return isRecord(value) && 'goal_weights' in value
}

function validateGenerationOptions(value: unknown): asserts value is KnifeKnowledgeCandidateGenerationOptions {
  if (!isRecord(value)) throw new KnifeKnowledgeCandidatePlanError('INVALID_OPTIONS', 'generation options must be a closed object')
  const allowed = new Set(['goal_weights', 'candidate_count', 'seed'])
  if (Object.keys(value).some((key) => !allowed.has(key)) || Object.prototype.hasOwnProperty.call(value, 'goal_weights') === false) {
    throw new KnifeKnowledgeCandidatePlanError('INVALID_OPTIONS', 'generation options contain an unknown or missing key')
  }
  for (const key of ['candidate_count', 'seed'] as const) {
    if (Object.prototype.hasOwnProperty.call(value, key) && value[key] === undefined) {
      throw new KnifeKnowledgeCandidatePlanError('INVALID_OPTIONS', `${key} must be omitted or finite`)
    }
  }
}

function makePlan(input: {
  readonly program: KnifeSceneProgram
  readonly sourceFingerprint: string
  readonly sourceCanonical: string
  readonly goalWeights: KnifeKnowledgeGoalWeights
  readonly candidateCount: number
  readonly seed: number
  readonly candidates: readonly KnifeKnowledgeCandidate[]
  readonly status: KnifeKnowledgeCandidatePlanStatus
  readonly rejectionReason: string | null
}): KnifeKnowledgeCandidatePlan {
  const planDraft: Omit<KnifeKnowledgeCandidatePlan, 'deterministic_fingerprint'> & { deterministic_fingerprint: string } = {
    schema_version: KNIFE_KNOWLEDGE_CANDIDATE_PLAN_SCHEMA,
    route: KNIFE_KNOWLEDGE_ROUTE,
    knowledge_id: KNIFE_KNOWLEDGE_ID,
    knowledge_source: KNIFE_KNOWLEDGE_SOURCE,
    source_program_fingerprint: input.sourceFingerprint,
    source_program_canonical_sha256: input.sourceCanonical,
    goal_weights: input.goalWeights,
    seed: input.seed,
    requested_candidate_count: input.candidateCount,
    generated_candidate_count: input.candidates.length,
    candidate_budget: input.candidateCount,
    mutation_scopes: Object.freeze(input.candidates.map((candidate) => candidate.mutation_scope)),
    hard_bounds: KNIFE_KNOWLEDGE_CANDIDATE_HARD_BOUNDS,
    candidates: Object.freeze([...input.candidates]),
    status: input.status,
    direct_mutation_performed: false,
    source_envelope_policy: SOURCE_ENVELOPE_POLICY,
    rejection_reason: input.rejectionReason,
    deterministic_fingerprint: '',
  }
  planDraft.deterministic_fingerprint = fingerprintPlan(planDraft)
  const plan = deepFreeze(planDraft as KnifeKnowledgeCandidatePlan)
  validateKnifeKnowledgeCandidatePlan(plan)
  return plan
}

function validateCandidate(value: unknown, sourceProgramFingerprint: string, fingerprints: Set<string>): asserts value is KnifeKnowledgeCandidate {
  if (!isRecord(value)) failPlan('candidate must be an object')
  exactKeys(value, [
    'schema_version', 'candidate_id', 'ordinal', 'mutation_scope', 'changed_parameter_paths', 'changes',
    'source_rationale', 'source_refs', 'budget_estimate', 'candidate_program_fingerprint', 'program', 'proposal_status',
  ], 'candidate')
  if (value.schema_version !== KNIFE_KNOWLEDGE_CANDIDATE_SCHEMA || typeof value.candidate_id !== 'string' || !STABLE_ID.test(value.candidate_id)) failPlan('candidate identity is invalid')
  if (!Number.isInteger(value.ordinal) || value.ordinal < 1 || value.ordinal > 4) failPlan('candidate ordinal is outside [1,4]')
  if (!isMutationScope(value.mutation_scope)) failPlan('candidate mutation_scope is unsupported')
  if (!Array.isArray(value.changed_parameter_paths) || value.changed_parameter_paths.length === 0 || value.changed_parameter_paths.length > KNIFE_KNOWLEDGE_CANDIDATE_LIMITS.max_parameter_paths_per_candidate) failPlan('candidate changed_parameter_paths is invalid')
  if (!value.changed_parameter_paths.every((path) => typeof path === 'string' && path.length > 0 && path.length <= 180)) failPlan('candidate parameter path is invalid')
  if (!Array.isArray(value.changes) || value.changes.length !== value.changed_parameter_paths.length) failPlan('candidate change count is inconsistent')
  if (value.changed_parameter_paths.some((path, index) => path !== value.changes[index]?.path)) failPlan('candidate changed_parameter_paths are not ordered with changes')
  if (typeof value.source_rationale !== 'string' || value.source_rationale.length === 0 || value.source_rationale.length > KNIFE_KNOWLEDGE_CANDIDATE_LIMITS.max_rationale_length) failPlan('candidate source_rationale is invalid')
  validateRefs(value.source_refs)
  validateBudgetEstimate(value.budget_estimate)
  fingerprintText(value.candidate_program_fingerprint, 'candidate_program_fingerprint')
  validateProgramShape(value.program)
  if (value.program.source_envelope !== undefined || value.program.design_basis === 'img2threejs-compatible-import') failPlan('candidate program must be native and source-envelope free')
  if (fingerprints.has(value.candidate_program_fingerprint)) failPlan('candidate fingerprints must be unique')
  fingerprints.add(value.candidate_program_fingerprint)
  if (value.proposal_status !== 'REVIEW_ONLY') failPlan('candidate proposal_status must remain REVIEW_ONLY')
  const changePaths = new Set<string>()
  for (const change of value.changes) {
    validateChange(change, value.mutation_scope)
    if (changePaths.has(change.path)) failPlan('candidate change paths must be unique')
    changePaths.add(change.path)
    if (change.source_rationale !== value.source_rationale || canonicalJson(change.source_refs) !== canonicalJson(value.source_refs)) failPlan('candidate change rationale bindings drifted')
  }
  const actualProgramFingerprint = fingerprintCandidateProgram(value.program, value.mutation_scope, value.changes)
  if (actualProgramFingerprint !== value.candidate_program_fingerprint) failPlan('candidate program fingerprint does not match changes')
  if (fingerprintProgram(value.program) === sourceProgramFingerprint) failPlan('candidate program must differ from source program')
}

function validateChange(value: unknown, scope: KnifeKnowledgeMutationScope): asserts value is KnifeKnowledgeParameterChange {
  if (!isRecord(value)) failPlan('change must be an object')
  exactKeys(value, ['schema_version', 'path', 'old_value', 'new_value', 'delta', 'hard_bounds', 'source_rationale', 'source_refs'], 'change')
  if (value.schema_version !== KNIFE_KNOWLEDGE_PARAMETER_CHANGE_SCHEMA || typeof value.path !== 'string' || value.path.length === 0) failPlan('change identity is invalid')
  finite(value.old_value, 'change.old_value')
  finite(value.new_value, 'change.new_value')
  finite(value.delta, 'change.delta')
  if (round12(value.new_value - value.old_value) !== round12(value.delta) || Math.abs(value.delta) < KNIFE_KNOWLEDGE_CANDIDATE_LIMITS.minimum_absolute_delta) failPlan('change delta is inconsistent or too small')
  if (!isRecord(value.hard_bounds)) failPlan('change hard_bounds must be an object')
  exactKeys(value.hard_bounds, ['min', 'max'], 'change.hard_bounds')
  finite(value.hard_bounds.min, 'change.hard_bounds.min')
  finite(value.hard_bounds.max, 'change.hard_bounds.max')
  if (value.hard_bounds.min > value.hard_bounds.max || value.old_value < value.hard_bounds.min || value.old_value > value.hard_bounds.max || value.new_value < value.hard_bounds.min || value.new_value > value.hard_bounds.max) failPlan('change value exceeds hard_bounds')
  if (typeof value.source_rationale !== 'string' || value.source_rationale.length === 0) failPlan('change source_rationale is invalid')
  validateRefs(value.source_refs)
  if (!pathBelongsToScope(value.path, scope)) failPlan(`change path ${value.path} does not belong to ${scope}`)
  const scopeBounds = KNIFE_KNOWLEDGE_CANDIDATE_HARD_BOUNDS[scope]
  if (value.hard_bounds.min !== scopeBounds.value_min || value.hard_bounds.max !== scopeBounds.value_max || Math.abs(value.delta) > scopeBounds.max_abs_delta) failPlan(`change exceeds the ${scope} hard step bound`)
}

function validateBudgetEstimate(value: unknown): asserts value is KnifeKnowledgeBudgetEstimate {
  if (!isRecord(value)) failPlan('budget_estimate must be an object')
  exactKeys(value, ['schema_version', 'estimator', 'estimated_triangles', 'estimated_draw_calls', 'estimated_texture_bytes', 'max_triangles', 'max_draw_calls', 'max_texture_bytes', 'within_budget', 'rationale'], 'budget_estimate')
  if (value.schema_version !== KNIFE_KNOWLEDGE_BUDGET_ESTIMATE_SCHEMA || value.estimator !== 'closed-program-heuristic@1') failPlan('budget_estimate identity drifted')
  for (const key of ['estimated_triangles', 'estimated_draw_calls', 'estimated_texture_bytes', 'max_triangles', 'max_draw_calls', 'max_texture_bytes'] as const) {
    if (!Number.isInteger(value[key]) || value[key] < 0) failPlan(`budget_estimate.${key} is invalid`)
  }
  if (typeof value.within_budget !== 'boolean' || typeof value.rationale !== 'string' || value.rationale.length === 0) failPlan('budget_estimate status is invalid')
  if (value.within_budget !== (value.estimated_triangles <= value.max_triangles && value.estimated_draw_calls <= value.max_draw_calls && value.estimated_texture_bytes <= value.max_texture_bytes)) failPlan('budget_estimate within_budget is inconsistent')
}

function validateHardBounds(value: unknown, scope: KnifeKnowledgeMutationScope): asserts value is KnifeKnowledgeHardBounds {
  if (!isRecord(value)) failPlan(`hard_bounds.${scope} must be an object`)
  exactKeys(value, ['schema_version', 'scope', 'value_min', 'value_max', 'max_abs_delta', 'formula'], `hard_bounds.${scope}`)
  if (value.schema_version !== KNIFE_KNOWLEDGE_HARD_BOUNDS_SCHEMA || value.scope !== scope || typeof value.formula !== 'string') failPlan(`hard_bounds.${scope} identity is invalid`)
  finite(value.value_min, `hard_bounds.${scope}.value_min`)
  finite(value.value_max, `hard_bounds.${scope}.value_max`)
  finite(value.max_abs_delta, `hard_bounds.${scope}.max_abs_delta`)
  if (value.value_min > value.value_max || value.max_abs_delta <= 0) failPlan(`hard_bounds.${scope} interval is invalid`)
  const expected = KNIFE_KNOWLEDGE_CANDIDATE_HARD_BOUNDS[scope]
  if (value.value_min !== expected.value_min || value.value_max !== expected.value_max || value.max_abs_delta !== expected.max_abs_delta || value.formula !== expected.formula) failPlan(`hard_bounds.${scope} drifted from the closed contract`)
}

function validateProgramShape(value: unknown): asserts value is KnifeSceneProgram {
  if (!isRecord(value)) throw new KnifeKnowledgeCandidatePlanError('INVALID_PROGRAM', 'program must be an object')
  exactKeys(value, ['schema_version', 'asset_id', 'family', 'design_basis', 'coordinate_convention', 'blade_surface', 'parts', 'material_zones', 'presentation', 'budgets', 'unknowns', 'canonical_sha256'], 'program', ['source_envelope', 'assembly'])
  if (value.schema_version !== 'KnifeSceneProgram@1' || typeof value.asset_id !== 'string' || !STABLE_ID.test(value.asset_id)) invalidProgram('program identity is invalid')
  if (!['kukri', 'tanto', 'karambit', 'bayonet', 'machete', 'original-knife'].includes(value.family as string)) invalidProgram('program family is unsupported')
  if (!['authorized-reference-inspired', 'original-design', 'img2threejs-compatible-import'].includes(value.design_basis as string)) invalidProgram('program design_basis is unsupported')
  if (value.coordinate_convention !== 'weapon-front-z-up-right-handed@1') invalidProgram('program coordinate convention is unsupported')
  if (value.source_envelope !== undefined) {
    try {
      validateImg2ThreeJsSourceEnvelope(value.source_envelope)
    } catch {
      invalidProgram('source_envelope is not a valid pinned compatibility envelope')
    }
  }
  validateBladeSurface(value.blade_surface)
  if (value.assembly !== undefined) validateAssembly(value.assembly)
  validateParts(value.parts, value.material_zones, value.assembly)
  validatePresentation(value.presentation)
  validateBudgets(value.budgets)
  if (!Array.isArray(value.unknowns) || value.unknowns.length > KNIFE_KNOWLEDGE_CANDIDATE_LIMITS.max_unknowns || value.unknowns.some((item) => typeof item !== 'string' || item.length === 0 || item.length > 120)) invalidProgram('unknowns are outside the closed bounded shape')
  shaText(value.canonical_sha256, 'program.canonical_sha256', true)
}

function validateBladeSurface(value: unknown): asserts value is KnifeSceneProgram['blade_surface'] {
  if (!isRecord(value)) invalidProgram('blade_surface must be an object')
  exactKeys(value, ['spine_curve', 'cutting_edge_curve', 'sections', 'surface_roles'], 'blade_surface')
  validateCurve(value.spine_curve, 'blade_surface.spine_curve')
  validateCurve(value.cutting_edge_curve, 'blade_surface.cutting_edge_curve')
  if (!Array.isArray(value.sections) || value.sections.length < 4 || value.sections.length > 32) invalidProgram('blade_surface.sections count is outside [4,32]')
  const roles = new Set<string>()
  let previousU = -1
  for (const [index, section] of value.sections.entries()) {
    validateSection(section, `blade_surface.sections[${index}]`)
    if ((section as KnifeSection).u <= previousU) invalidProgram('blade sections must be strictly u-increasing')
    previousU = (section as KnifeSection).u
    if (roles.has((section as KnifeSection).role) && ['root', 'shoulder', 'belly', 'tip'].includes((section as KnifeSection).role)) invalidProgram(`duplicate required section role ${(section as KnifeSection).role}`)
    roles.add((section as KnifeSection).role)
  }
  for (const role of ['root', 'shoulder', 'belly', 'tip']) if (!roles.has(role)) invalidProgram(`missing required section role ${role}`)
  if (!Array.isArray(value.surface_roles) || value.surface_roles.length < 4 || new Set(value.surface_roles).size !== value.surface_roles.length || value.surface_roles.some((role) => !['blade-body', 'cutting-edge', 'spine', 'root-transition', 'ricasso', 'fuller'].includes(role as string))) invalidProgram('surface_roles are not closed')
}

function validateCurve(value: unknown, label: string): void {
  if (!isRecord(value)) invalidProgram(`${label} must be an object`)
  exactKeys(value, ['curve_id', 'basis', 'control_points'], label)
  if (typeof value.curve_id !== 'string' || !STABLE_ID.test(value.curve_id) || !['bezier', 'nurbs-like'].includes(value.basis as string)) invalidProgram(`${label} identity is invalid`)
  if (!Array.isArray(value.control_points) || value.control_points.length < 4 || value.control_points.length > MAX_CURVE_POINTS) invalidProgram(`${label}.control_points count is outside [4,64]`)
  for (const [index, point] of value.control_points.entries()) validatePoint(point, `${label}.control_points[${index}]`)
}

function validatePoint(value: unknown, label: string): void {
  if (!Array.isArray(value) || value.length !== 3 || value.some((item) => typeof item !== 'number' || !Number.isFinite(item) || Math.abs(item) > MAX_PROGRAM_COORDINATE)) invalidProgram(`${label} must be a finite bounded point`)
}

function validateSection(value: unknown, label: string): void {
  if (!isRecord(value)) invalidProgram(`${label} must be an object`)
  exactKeys(value, ['section_id', 'role', 'u', 'half_width', 'thickness', 'edge_offset', 'spine_offset', 'asymmetry', 'twist'], label)
  if (typeof value.section_id !== 'string' || !STABLE_ID.test(value.section_id) || !['root', 'shoulder', 'belly', 'tip', 'intermediate'].includes(value.role as string)) invalidProgram(`${label} identity is invalid`)
  finiteRange(value.u, 0, 1, `${label}.u`)
  finiteRange(value.half_width, 0, 2, `${label}.half_width`, true)
  finiteRange(value.thickness, 0, 1, `${label}.thickness`, true)
  finiteRange(value.edge_offset, -1, 1, `${label}.edge_offset`)
  finiteRange(value.spine_offset, -1, 1, `${label}.spine_offset`)
  finiteRange(value.asymmetry, -1, 1, `${label}.asymmetry`)
  finiteRange(value.twist, -1.5708, 1.5708, `${label}.twist`)
}

function validateAssembly(value: unknown): asserts value is NonNullable<KnifeSceneProgram['assembly']> {
  if (!isRecord(value)) invalidProgram('assembly must be an object')
  exactKeys(value, [], 'assembly', ['guard', 'grip', 'pommel', 'fasteners', 'gems', 'reliefs'])
  if (value.guard !== undefined) validateGuard(value.guard)
  if (value.grip !== undefined) validateGrip(value.grip)
  if (value.pommel !== undefined) validatePommel(value.pommel)
  for (const key of ['fasteners', 'gems', 'reliefs'] as const) {
    if (value[key] !== undefined && (!Array.isArray(value[key]) || value[key].length > 32)) invalidProgram(`assembly.${key} is outside the closed array bound`)
    if (Array.isArray(value[key])) for (const item of value[key]) validatePrimitive(item, `assembly.${key}`)
  }
}

function validateGuard(value: unknown): asserts value is NonNullable<NonNullable<KnifeSceneProgram['assembly']>['guard']> {
  if (!isRecord(value)) invalidProgram('assembly.guard must be an object')
  exactKeys(value, ['primitive', 'part_id', 'center', 'span', 'thickness', 'depth'], 'assembly.guard', ['style', 'jaw_gap', 'upper_jaw', 'lower_jaw', 'horns', 'eye_sockets'])
  if (value.primitive !== 'guard' || typeof value.part_id !== 'string' || !STABLE_ID.test(value.part_id) || !['classic', 'dragon-guard', undefined].includes(value.style as string | undefined)) invalidProgram('assembly.guard identity is invalid')
  validatePoint(value.center, 'assembly.guard.center')
  finiteRange(value.span, 0, 2, 'assembly.guard.span', true)
  finiteRange(value.thickness, 0, 1, 'assembly.guard.thickness', true)
  finiteRange(value.depth, 0, 1, 'assembly.guard.depth', true)
  if (value.style === 'dragon-guard') {
    finiteRange(value.jaw_gap, 0.01, 0.6, 'assembly.guard.jaw_gap')
    validateJaw(value.upper_jaw, 'assembly.guard.upper_jaw')
    validateJaw(value.lower_jaw, 'assembly.guard.lower_jaw')
    if (!Array.isArray(value.horns) || value.horns.length < 2 || value.horns.length > 4) invalidProgram('assembly.guard.horns count is outside [2,4]')
    for (const horn of value.horns) validateHorn(horn)
    if (!Array.isArray(value.eye_sockets) || value.eye_sockets.length < 1 || value.eye_sockets.length > 2) invalidProgram('assembly.guard.eye_sockets count is outside [1,2]')
    for (const eye of value.eye_sockets) validateEye(eye)
  }
}

function validateJaw(value: unknown, label: string): void {
  if (!isRecord(value)) invalidProgram(`${label} must be an object`)
  exactKeys(value, ['span', 'thickness', 'depth', 'offset_y', 'offset_z', 'curvature'], label)
  finiteRange(value.span, 0, 2, `${label}.span`, true)
  finiteRange(value.thickness, 0, 0.4, `${label}.thickness`, true)
  finiteRange(value.depth, 0, 0.4, `${label}.depth`, true)
  finiteRange(value.offset_y, -1, 1, `${label}.offset_y`)
  finiteRange(value.offset_z, -0.8, 0.8, `${label}.offset_z`)
  finiteRange(value.curvature, -0.25, 0.25, `${label}.curvature`)
}

function validateHorn(value: unknown): void {
  if (!isRecord(value)) invalidProgram('guard horn must be an object')
  exactKeys(value, ['feature_id', 'side', 'length', 'radius', 'sweep', 'offset_z'], 'guard horn')
  if (typeof value.feature_id !== 'string' || !STABLE_ID.test(value.feature_id) || (value.side !== -1 && value.side !== 1)) invalidProgram('guard horn identity is invalid')
  finiteRange(value.length, 0, 0.8, 'guard horn.length', true)
  finiteRange(value.radius, 0, 0.2, 'guard horn.radius', true)
  finiteRange(value.sweep, -0.75, 0.75, 'guard horn.sweep')
  finiteRange(value.offset_z, -0.8, 0.8, 'guard horn.offset_z')
}

function validateEye(value: unknown): void {
  if (!isRecord(value)) invalidProgram('guard eye must be an object')
  exactKeys(value, ['feature_id', 'side', 'radius', 'depth', 'offset_y', 'offset_z'], 'guard eye')
  if (typeof value.feature_id !== 'string' || !STABLE_ID.test(value.feature_id) || (value.side !== -1 && value.side !== 1)) invalidProgram('guard eye identity is invalid')
  finiteRange(value.radius, 0, 0.25, 'guard eye.radius', true)
  finiteRange(value.depth, 0, 0.2, 'guard eye.depth', true)
  finiteRange(value.offset_y, -1, 1, 'guard eye.offset_y')
  finiteRange(value.offset_z, -0.8, 0.8, 'guard eye.offset_z')
}

function validateGrip(value: unknown): asserts value is KnifeGripSpec {
  if (!isRecord(value)) invalidProgram('assembly.grip must be an object')
  exactKeys(value, ['primitive', 'part_id', 'center', 'length', 'radius', 'taper', 'facets'], 'assembly.grip', ['style', 'centerline', 'segments', 'metal_frames', 'fasteners'])
  if (value.primitive !== 'grip' || typeof value.part_id !== 'string' || !STABLE_ID.test(value.part_id) || !['classic', 'segmented-grip', undefined].includes(value.style as string | undefined)) invalidProgram('assembly.grip identity is invalid')
  validatePoint(value.center, 'assembly.grip.center')
  finiteRange(value.length, 0, 2, 'assembly.grip.length', true)
  finiteRange(value.radius, 0, 1, 'assembly.grip.radius', true)
  finiteRange(value.taper, -0.9, 0.9, 'assembly.grip.taper')
  if (typeof value.facets !== 'number' || !Number.isInteger(value.facets) || value.facets < 6 || value.facets > 32) invalidProgram('assembly.grip.facets is outside [6,32]')
  if (value.style === 'segmented-grip') {
    if (!Array.isArray(value.centerline) || value.centerline.length < 3 || value.centerline.length > 8) invalidProgram('assembly.grip.centerline count is outside [3,8]')
    for (const point of value.centerline) validatePoint(point, 'assembly.grip.centerline point')
    if (!Array.isArray(value.segments) || value.segments.length < 2 || value.segments.length > 8) invalidProgram('assembly.grip.segments count is outside [2,8]')
    for (const segment of value.segments) {
      if (!isRecord(segment)) invalidProgram('assembly.grip segment must be an object')
      exactKeys(segment, ['feature_id', 'start_u', 'end_u', 'radius_scale'], 'assembly.grip segment')
      if (typeof segment.feature_id !== 'string' || !STABLE_ID.test(segment.feature_id)) invalidProgram('assembly.grip segment feature_id is invalid')
      finiteRange(segment.start_u, 0, 1, 'assembly.grip segment.start_u')
      finiteRange(segment.end_u, 0, 1, 'assembly.grip segment.end_u')
      if ((segment.end_u as number) <= (segment.start_u as number)) invalidProgram('assembly.grip segment range is inverted')
      finiteRange(segment.radius_scale, 0.5, 1.5, 'assembly.grip segment.radius_scale')
    }
    if (!Array.isArray(value.metal_frames) || value.metal_frames.length < 1 || value.metal_frames.length > 8) invalidProgram('assembly.grip.metal_frames count is outside [1,8]')
    if (!Array.isArray(value.fasteners) || value.fasteners.length < 3 || value.fasteners.length > 5) invalidProgram('assembly.grip.fasteners count is outside [3,5]')
    for (const frame of value.metal_frames) validateGripFrame(frame)
    for (const fastener of value.fasteners) validateGripFastener(fastener)
  }
}

function validateGripFrame(value: unknown): void {
  if (!isRecord(value)) invalidProgram('assembly.grip metal frame must be an object')
  exactKeys(value, ['feature_id', 'at', 'width', 'thickness'], 'assembly.grip metal frame')
  if (typeof value.feature_id !== 'string' || !STABLE_ID.test(value.feature_id)) invalidProgram('assembly.grip metal frame feature_id is invalid')
  finiteRange(value.at, 0, 1, 'assembly.grip metal frame.at')
  finiteRange(value.width, 0, 1, 'assembly.grip metal frame.width', true)
  finiteRange(value.thickness, 0, 0.2, 'assembly.grip metal frame.thickness', true)
}

function validateGripFastener(value: unknown): void {
  if (!isRecord(value)) invalidProgram('assembly.grip fastener feature must be an object')
  exactKeys(value, ['feature_id', 'at', 'side', 'radius', 'depth'], 'assembly.grip fastener feature')
  if (typeof value.feature_id !== 'string' || !STABLE_ID.test(value.feature_id) || (value.side !== -1 && value.side !== 1)) invalidProgram('assembly.grip fastener feature identity is invalid')
  finiteRange(value.at, 0, 1, 'assembly.grip fastener feature.at')
  finiteRange(value.radius, 0, 0.2, 'assembly.grip fastener feature.radius', true)
  finiteRange(value.depth, 0, 0.2, 'assembly.grip fastener feature.depth', true)
}

function validatePommel(value: unknown): asserts value is KnifeSceneProgram['assembly'] extends { pommel?: infer P } ? P : never {
  if (!isRecord(value)) invalidProgram('assembly.pommel must be an object')
  exactKeys(value, ['primitive', 'part_id', 'center', 'length', 'radius', 'depth'], 'assembly.pommel', ['style', 'hook', 'gem_seat'])
  if (value.primitive !== 'pommel' || typeof value.part_id !== 'string' || !STABLE_ID.test(value.part_id) || !['classic', 'hooked-pommel', undefined].includes(value.style as string | undefined)) invalidProgram('assembly.pommel identity is invalid')
  validatePoint(value.center, 'assembly.pommel.center')
  finiteRange(value.length, 0, 1, 'assembly.pommel.length', true)
  finiteRange(value.radius, 0, 1, 'assembly.pommel.radius', true)
  finiteRange(value.depth, 0, 1, 'assembly.pommel.depth', true)
  if (value.style === 'hooked-pommel') {
    if (!isRecord(value.hook)) invalidProgram('assembly.pommel.hook must be an object')
    exactKeys(value.hook, ['length', 'radius', 'bend', 'direction'], 'assembly.pommel.hook')
    finiteRange(value.hook.length, 0, 0.8, 'assembly.pommel.hook.length', true)
    finiteRange(value.hook.radius, 0, 0.2, 'assembly.pommel.hook.radius', true)
    finiteRange(value.hook.bend, 0.2, 1, 'assembly.pommel.hook.bend')
    if (value.hook.direction !== -1 && value.hook.direction !== 1) invalidProgram('assembly.pommel.hook.direction is invalid')
    if (!isRecord(value.gem_seat)) invalidProgram('assembly.pommel.gem_seat must be an object')
    exactKeys(value.gem_seat, ['feature_id', 'radius', 'depth', 'offset_x', 'offset_y', 'offset_z', 'axis'], 'assembly.pommel.gem_seat')
    if (typeof value.gem_seat.feature_id !== 'string' || !STABLE_ID.test(value.gem_seat.feature_id) || !['x', 'y', 'z'].includes(value.gem_seat.axis as string)) invalidProgram('assembly.pommel.gem_seat identity is invalid')
    finiteRange(value.gem_seat.radius, 0, 0.25, 'assembly.pommel.gem_seat.radius', true)
    finiteRange(value.gem_seat.depth, 0, 0.2, 'assembly.pommel.gem_seat.depth', true)
    finiteRange(value.gem_seat.offset_x, -1, 1, 'assembly.pommel.gem_seat.offset_x')
    finiteRange(value.gem_seat.offset_y, -1, 1, 'assembly.pommel.gem_seat.offset_y')
    finiteRange(value.gem_seat.offset_z, -1, 1, 'assembly.pommel.gem_seat.offset_z')
  }
}

function validatePrimitive(value: unknown, label: string): void {
  if (!isRecord(value)) invalidProgram(`${label} item must be an object`)
  if (typeof value.part_id !== 'string' || !STABLE_ID.test(value.part_id) || !['fastener', 'gem', 'relief'].includes(value.primitive as string)) invalidProgram(`${label} item identity is invalid`)
  if (value.primitive === 'relief') {
    exactKeys(value, ['primitive', 'part_id', 'center', 'width', 'height', 'depth', 'shape', 'axis'], label)
  } else {
    exactKeys(value, ['primitive', 'part_id', 'center', 'radius', 'depth', 'axis'], label)
  }
  validatePoint(value.center, `${label}.center`)
  if (value.primitive === 'relief') {
    finiteRange(value.width, 0, 2, `${label}.width`, true)
    finiteRange(value.height, 0, 2, `${label}.height`, true)
    finiteRange(value.depth, 0, 1, `${label}.depth`, true)
    if (!['panel', 'diamond'].includes(value.shape as string) || !['x', 'y', 'z'].includes(value.axis as string)) invalidProgram(`${label}.relief shape or axis is invalid`)
  } else {
    finiteRange(value.radius, 0, 2, `${label}.radius`, true)
    finiteRange(value.depth, 0, 1, `${label}.depth`, true)
    if (value.primitive === 'fastener' || value.primitive === 'gem') if (!['x', 'y', 'z'].includes(value.axis as string)) invalidProgram(`${label}.axis is invalid`)
  }
}

function validateParts(parts: unknown, zones: unknown, assembly: unknown): void {
  if (!Array.isArray(parts) || parts.length < 2 || parts.length > 64) invalidProgram('parts count is outside [2,64]')
  if (!Array.isArray(zones) || zones.length < 1 || zones.length > 32) invalidProgram('material_zones count is outside [1,32]')
  const partIds = new Set<string>()
  const singletonRoles = new Set(['blade-body', 'cutting-edge', 'guard', 'grip', 'pommel'])
  const singletonRoleIds = new Set<string>()
  const zoneIds = new Set<string>()
  for (const zone of zones) {
    if (!isRecord(zone)) invalidProgram('material zone must be an object')
    exactKeys(zone, ['material_zone_id', 'model', 'base_color', 'metalness', 'roughness'], 'material zone')
    if (typeof zone.material_zone_id !== 'string' || !STABLE_ID.test(zone.material_zone_id) || zone.model !== 'mesh-standard-layered@1' || typeof zone.base_color !== 'string' || !/^#[0-9a-f]{6}$/i.test(zone.base_color) || zoneIds.has(zone.material_zone_id)) invalidProgram('material zone identity is invalid')
    zoneIds.add(zone.material_zone_id)
    finiteRange(zone.metalness, 0, 1, 'material zone.metalness')
    finiteRange(zone.roughness, 0, 1, 'material zone.roughness')
  }
  for (const part of parts) {
    if (!isRecord(part)) invalidProgram('part must be an object')
    exactKeys(part, ['part_id', 'role', 'source_class', 'material_zone_id', 'frozen'], 'part')
    if (typeof part.part_id !== 'string' || !STABLE_ID.test(part.part_id) || partIds.has(part.part_id) || !['blade-body', 'cutting-edge', 'guard', 'grip', 'pommel', 'fastener', 'gem', 'relief', 'helper'].includes(part.role as string) || !['observed', 'inferred', 'design-prior', 'original-choice'].includes(part.source_class as string) || typeof part.material_zone_id !== 'string' || !zoneIds.has(part.material_zone_id) || typeof part.frozen !== 'boolean') invalidProgram('part identity is invalid')
    if (singletonRoles.has(part.role as string) && singletonRoleIds.has(part.role as string)) invalidProgram(`duplicate singleton part role ${part.role}`)
    partIds.add(part.part_id)
    if (singletonRoles.has(part.role as string)) singletonRoleIds.add(part.role as string)
  }
  const assemblyPartIds = assemblyPartIdSet(assembly)
  for (const partId of assemblyPartIds) if (!partIds.has(partId)) invalidProgram(`assembly part ${partId} is missing from parts`)
}

function validatePresentation(value: unknown): void {
  if (!isRecord(value)) invalidProgram('presentation must be an object')
  exactKeys(value, ['camera_set', 'renderer', 'aovs'], 'presentation')
  if (value.camera_set !== 'knife-fixed-eight-view@1' || value.renderer !== 'threejs-browser-authority@1' || !Array.isArray(value.aovs) || value.aovs.length < 6 || new Set(value.aovs).size !== value.aovs.length || value.aovs.some((item) => !['beauty', 'silhouette', 'depth', 'normal', 'part-id', 'material-id', 'wireframe', 'curvature', 'uv-stretch'].includes(item as string))) invalidProgram('presentation is outside the closed vocabulary')
}

function validateBudgets(value: unknown): void {
  if (!isRecord(value)) invalidProgram('budgets must be an object')
  exactKeys(value, ['max_triangles', 'max_draw_calls', 'max_texture_bytes'], 'budgets')
  if (!Number.isInteger(value.max_triangles) || value.max_triangles < 64 || value.max_triangles > 200000 || !Number.isInteger(value.max_draw_calls) || value.max_draw_calls < 1 || value.max_draw_calls > 128 || !Number.isInteger(value.max_texture_bytes) || value.max_texture_bytes < 0 || value.max_texture_bytes > 268435456) invalidProgram('budgets are outside the closed bounds')
}

function scopeAvailability(program: KnifeSceneProgram, scope: KnifeKnowledgeMutationScope, weight: number): ScopeAvailability {
  const partsById = new Map(program.parts.map((part) => [part.part_id, part]))
  const assembly = program.assembly
  const guard = assembly?.guard
  const grip = assembly?.grip
  const pommel = assembly?.pommel
  const mutableRole = (role: KnifeSceneProgram['parts'][number]['role']): boolean => {
    const part = program.parts.find((candidate) => candidate.role === role)
    return Boolean(part && !part.frozen)
  }
  const mutableAssemblyPart = (
    partId: string | undefined,
    role: KnifeSceneProgram['parts'][number]['role'],
  ): boolean => Boolean(partId && partsById.get(partId)?.role === role && partsById.get(partId)?.frozen === false)
  let available = false
  let reason = 'scope is not represented by the native program'
  switch (scope) {
    case 'blade-belly':
    case 'blade-tip-taper':
    case 'blade-thickness':
      available = mutableRole('blade-body') && Boolean(findSection(program, 'belly'))
      reason = available ? 'mutable blade-body belly section is present' : 'blade-body is frozen or the belly section is absent'
      break
    case 'blade-curvature':
      available = mutableRole('blade-body') && mutableRole('cutting-edge')
        && program.blade_surface.spine_curve.control_points.length >= 4
        && program.blade_surface.cutting_edge_curve.control_points.length >= 4
      reason = available ? 'both boundary curves are mutable' : 'one boundary curve is frozen or incomplete'
      break
    case 'guard-jaw-gap':
      available = isDragonGuard(guard) && mutableAssemblyPart(guard.part_id, 'guard')
      reason = available ? 'mutable dragon guard jaw gap is present' : 'a mutable dragon guard is absent'
      break
    case 'guard-horn-sweep':
      available = isDragonGuard(guard) && mutableAssemblyPart(guard.part_id, 'guard') && guard.horns.length > 0
      reason = available ? 'mutable dragon guard horns are present' : 'mutable dragon guard horns are absent'
      break
    case 'grip-taper':
      available = grip !== undefined && mutableAssemblyPart(grip.part_id, 'grip')
      reason = available ? 'mutable grip taper is present' : 'a mutable grip is absent'
      break
    case 'grip-segment-rhythm':
      available = isSegmentedGrip(grip) && mutableAssemblyPart(grip.part_id, 'grip')
      reason = available ? 'mutable segmented grip rhythm is present' : 'a mutable segmented grip is absent'
      break
    case 'pommel-hook':
      available = isHookedPommel(pommel) && mutableAssemblyPart(pommel.part_id, 'pommel')
      reason = available ? 'mutable hooked pommel is present' : 'a mutable hooked pommel is absent'
      break
    case 'relief-depth':
      available = Boolean(assembly?.reliefs?.some((relief) => {
        const part = partsById.get(relief.part_id)
        return part?.role === 'relief' && part.frozen === false
      }))
      reason = available ? 'mutable relief depth is present' : 'a mutable relief is absent'
      break
  }
  return { scope, weight, available, reason }
}

function rankScopes(scopes: readonly ScopeAvailability[], seed: number): readonly ScopeAvailability[] {
  return [...scopes].sort((left, right) => right.weight - left.weight
    || tieBreak(seed, left.scope) - tieBreak(seed, right.scope)
    || left.scope.localeCompare(right.scope))
}

function tieBreak(seed: number, scope: string): number {
  let hash = (seed ^ 0x811c9dc5) >>> 0
  for (const character of scope) {
    hash ^= character.codePointAt(0)!
    hash = Math.imul(hash, 0x01000193) >>> 0
  }
  return hash
}

function mutateScope(program: KnifeSceneProgram, scope: KnifeKnowledgeMutationScope, repetition: number, seed: number): MutationResult {
  const draft = clone(program) as MutableProgram
  const rationale = RATIONALES[scope]
  const polarity = ((repetition + (tieBreak(seed, scope) & 1)) % 2 === 0) ? 1 : -1
  const changes: KnifeKnowledgeParameterChange[] = []
  switch (scope) {
    case 'blade-belly': mutateBladeBelly(draft, polarity, changes); break
    case 'blade-curvature': mutateBladeCurvature(draft, polarity, changes); break
    case 'blade-tip-taper': mutateBladeTip(draft, polarity, changes); break
    case 'blade-thickness': mutateBladeThickness(draft, polarity, changes); break
    case 'guard-jaw-gap': mutateGuardGap(draft, polarity, changes); break
    case 'guard-horn-sweep': mutateHornSweep(draft, polarity, changes); break
    case 'grip-taper': mutateGripTaper(draft, polarity, changes); break
    case 'grip-segment-rhythm': mutateGripRhythm(draft, polarity, changes); break
    case 'pommel-hook': mutatePommelHook(draft, polarity, changes); break
    case 'relief-depth': mutateReliefDepth(draft, polarity, changes); break
  }
  if (changes.length === 0) throw new KnifeKnowledgeCandidatePlanError('NO_MUTABLE_SCOPE', `${scope} could not produce a non-zero bounded change`)
  // Runtime owns canonicalization.  The planner never invents or rewrites the
  // upstream program hash while creating a review-only draft.
  draft.canonical_sha256 = ''
  validateKnifeKnowledgeNativeProgram(draft)
  return {
    program: deepFreeze(draft as KnifeSceneProgram),
    changes: Object.freeze(changes),
    source_rationale: rationale.text,
    source_refs: rationale.refs,
  }
}

function mutateBladeBelly(program: MutableProgram, polarity: number, changes: KnifeKnowledgeParameterChange[]): void {
  const index = sectionIndex(program, 'belly')
  const section = program.blade_surface.sections[index]
  const next = boundedMutationValue(section.half_width, section.half_width * (1 + polarity * 0.12), 'blade-belly')
  recordNumberChange(changes, `blade_surface.sections[${index}].half_width`, section.half_width, next, boundsFor('blade-belly'), 'blade-belly')
  section.half_width = next
}

function mutateBladeCurvature(program: MutableProgram, polarity: number, changes: KnifeKnowledgeParameterChange[]): void {
  const spine = program.blade_surface.spine_curve.control_points
  const edge = program.blade_surface.cutting_edge_curve.control_points
  const belly = findSection(program, 'belly')!
  const index = nearestControlPointIndex(spine, belly.u)
  const delta = 0.035 * polarity
  const spineOld = spine[index][1]
  const edgeOld = edge[Math.min(index, edge.length - 1)][1]
  let spineNext = boundedMutationValue(spineOld, spineOld + delta, 'blade-curvature')
  let edgeNext = boundedMutationValue(edgeOld, edgeOld - delta, 'blade-curvature')
  if (spineNext <= edgeNext) {
    spineNext = boundedMutationValue(spineOld, spineOld + delta * 0.35, 'blade-curvature')
    edgeNext = boundedMutationValue(edgeOld, edgeOld - delta * 0.35, 'blade-curvature')
  }
  if (spineNext > edgeNext) {
    recordNumberChange(changes, `blade_surface.spine_curve.control_points[${index}][1]`, spineOld, spineNext, boundsFor('blade-curvature'), 'blade-curvature')
    recordNumberChange(changes, `blade_surface.cutting_edge_curve.control_points[${Math.min(index, edge.length - 1)}][1]`, edgeOld, edgeNext, boundsFor('blade-curvature'), 'blade-curvature')
    spine[index][1] = spineNext
    edge[Math.min(index, edge.length - 1)][1] = edgeNext
  }
}

function mutateBladeTip(program: MutableProgram, polarity: number, changes: KnifeKnowledgeParameterChange[]): void {
  const index = sectionIndex(program, 'tip')
  const belly = findSection(program, 'belly')!
  const section = program.blade_surface.sections[index]
  const factor = polarity > 0 ? 0.78 : 1.12
  const maximum = Math.min(2, belly.half_width * 0.9)
  const next = boundedMutationValue(section.half_width, section.half_width * factor, 'blade-tip-taper', MIN_VALUE, maximum)
  recordNumberChange(changes, `blade_surface.sections[${index}].half_width`, section.half_width, next, boundsFor('blade-tip-taper'), 'blade-tip-taper')
  section.half_width = next
}

function mutateBladeThickness(program: MutableProgram, polarity: number, changes: KnifeKnowledgeParameterChange[]): void {
  const index = sectionIndex(program, 'belly')
  const section = program.blade_surface.sections[index]
  const next = boundedMutationValue(section.thickness, section.thickness * (1 + polarity * 0.12), 'blade-thickness')
  recordNumberChange(changes, `blade_surface.sections[${index}].thickness`, section.thickness, next, boundsFor('blade-thickness'), 'blade-thickness')
  section.thickness = next
}

function mutateGuardGap(program: MutableProgram, polarity: number, changes: KnifeKnowledgeParameterChange[]): void {
  const guard = program.assembly!.guard as MutableDragonGuard
  const maximum = Math.min(0.6, guard.span)
  const next = boundedMutationValue(guard.jaw_gap, guard.jaw_gap * (1 + polarity * 0.18), 'guard-jaw-gap', 0.01, maximum)
  recordNumberChange(changes, 'assembly.guard.jaw_gap', guard.jaw_gap, next, boundsFor('guard-jaw-gap'), 'guard-jaw-gap')
  guard.jaw_gap = next
}

function mutateHornSweep(program: MutableProgram, polarity: number, changes: KnifeKnowledgeParameterChange[]): void {
  const guard = program.assembly!.guard as MutableDragonGuard
  for (const [index, horn] of guard.horns.entries()) {
    const next = boundedMutationValue(horn.sweep, horn.sweep + polarity * 0.06, 'guard-horn-sweep')
    recordNumberChange(changes, `assembly.guard.horns[${index}].sweep`, horn.sweep, next, boundsFor('guard-horn-sweep'), 'guard-horn-sweep')
    horn.sweep = next
  }
}

function mutateGripTaper(program: MutableProgram, polarity: number, changes: KnifeKnowledgeParameterChange[]): void {
  const grip = program.assembly!.grip as MutableGrip
  const next = boundedMutationValue(grip.taper, grip.taper + polarity * 0.09, 'grip-taper')
  recordNumberChange(changes, 'assembly.grip.taper', grip.taper, next, boundsFor('grip-taper'), 'grip-taper')
  grip.taper = next
}

function mutateGripRhythm(program: MutableProgram, polarity: number, changes: KnifeKnowledgeParameterChange[]): void {
  const grip = program.assembly!.grip as MutableSegmentedGrip
  for (const [index, segment] of grip.segments.entries()) {
    const signed = polarity * (index % 2 === 0 ? 1 : -1)
    const next = boundedMutationValue(segment.radius_scale, segment.radius_scale + signed * 0.05, 'grip-segment-rhythm')
    recordNumberChange(changes, `assembly.grip.segments[${index}].radius_scale`, segment.radius_scale, next, boundsFor('grip-segment-rhythm'), 'grip-segment-rhythm')
    segment.radius_scale = next
  }
}

function mutatePommelHook(program: MutableProgram, polarity: number, changes: KnifeKnowledgeParameterChange[]): void {
  const pommel = program.assembly!.pommel as MutableHookedPommel
  const next = boundedMutationValue(pommel.hook.bend, pommel.hook.bend + polarity * 0.06, 'pommel-hook')
  recordNumberChange(changes, 'assembly.pommel.hook.bend', pommel.hook.bend, next, boundsFor('pommel-hook'), 'pommel-hook')
  pommel.hook.bend = next
}

function mutateReliefDepth(program: MutableProgram, polarity: number, changes: KnifeKnowledgeParameterChange[]): void {
  const roles = new Map<string, KnifeSceneProgram['parts'][number]>(program.parts.map((part: KnifeSceneProgram['parts'][number]) => [part.part_id, part]))
  for (const [index, relief] of (program.assembly?.reliefs ?? []).entries()) {
    const part = roles.get(relief.part_id)
    if (!part || part.frozen) continue
    const next = boundedMutationValue(relief.depth, relief.depth * (1 + polarity * 0.18), 'relief-depth')
    recordNumberChange(changes, `assembly.reliefs[${index}].depth`, relief.depth, next, boundsFor('relief-depth'), 'relief-depth')
    relief.depth = next
  }
}

function recordNumberChange(
  changes: KnifeKnowledgeParameterChange[],
  path: string,
  oldValue: number,
  newValue: number,
  hardBounds: KnifeKnowledgeParameterBounds,
  scope: KnifeKnowledgeMutationScope,
): void {
  const oldRounded = round6(oldValue)
  const newRounded = round6(newValue)
  if (oldRounded === newRounded) return
  const rationale = RATIONALES[scope]
  changes.push({
    schema_version: KNIFE_KNOWLEDGE_PARAMETER_CHANGE_SCHEMA,
    path,
    old_value: oldRounded,
    new_value: newRounded,
    delta: round6(newRounded - oldRounded),
    hard_bounds: Object.freeze({ min: hardBounds.min, max: hardBounds.max }),
    source_rationale: rationale.text,
    source_refs: rationale.refs,
  })
}

function boundsFor(scope: KnifeKnowledgeMutationScope): KnifeKnowledgeParameterBounds {
  const bound = KNIFE_KNOWLEDGE_CANDIDATE_HARD_BOUNDS[scope]
  return { min: bound.value_min, max: bound.value_max }
}

/** Apply a proposed scalar mutation while enforcing both interval and step bounds. */
function boundedMutationValue(
  oldValue: number,
  proposedValue: number,
  scope: KnifeKnowledgeMutationScope,
  minimum = KNIFE_KNOWLEDGE_CANDIDATE_HARD_BOUNDS[scope].value_min,
  maximum = KNIFE_KNOWLEDGE_CANDIDATE_HARD_BOUNDS[scope].value_max,
): number {
  const bound = KNIFE_KNOWLEDGE_CANDIDATE_HARD_BOUNDS[scope]
  if (minimum > maximum || oldValue < minimum || oldValue > maximum) {
    throw new KnifeKnowledgeCandidatePlanError('NO_MUTABLE_SCOPE', `${scope} baseline is outside its bounded mutation interval`)
  }
  const cappedDelta = clamp(proposedValue - oldValue, -bound.max_abs_delta, bound.max_abs_delta)
  const next = clamp(round6(oldValue + cappedDelta), minimum, maximum)
  if (Math.abs(round6(next - oldValue)) > bound.max_abs_delta) {
    throw new KnifeKnowledgeCandidatePlanError('NO_MUTABLE_SCOPE', `${scope} mutation exceeds its bounded step interval`)
  }
  return next
}

function pathBelongsToScope(path: string, scope: KnifeKnowledgeMutationScope): boolean {
  switch (scope) {
    case 'blade-belly': return /^blade_surface\.sections\[\d+\]\.half_width$/.test(path)
    case 'blade-curvature': return /^blade_surface\.(?:spine_curve|cutting_edge_curve)\.control_points\[\d+\]\[1\]$/.test(path)
    case 'blade-tip-taper': return /^blade_surface\.sections\[\d+\]\.half_width$/.test(path)
    case 'blade-thickness': return /^blade_surface\.sections\[\d+\]\.thickness$/.test(path)
    case 'guard-jaw-gap': return path === 'assembly.guard.jaw_gap'
    case 'guard-horn-sweep': return /^assembly\.guard\.horns\[\d+\]\.sweep$/.test(path)
    case 'grip-taper': return path === 'assembly.grip.taper'
    case 'grip-segment-rhythm': return /^assembly\.grip\.segments\[\d+\]\.radius_scale$/.test(path)
    case 'pommel-hook': return path === 'assembly.pommel.hook.bend'
    case 'relief-depth': return /^assembly\.reliefs\[\d+\]\.depth$/.test(path)
  }
}

function fingerprintCandidateProgram(program: KnifeSceneProgram, scope: KnifeKnowledgeMutationScope, changes: readonly KnifeKnowledgeParameterChange[]): string {
  return stableFingerprint(canonicalJson({
    schema_version: KNIFE_KNOWLEDGE_CANDIDATE_SCHEMA,
    mutation_scope: scope,
    changed_parameter_paths: changes.map((change) => change.path),
    program: programForHash(program),
  }))
}

function fingerprintProgram(program: KnifeSceneProgram): string {
  return stableFingerprint(canonicalJson(programForHash(program)))
}

function canonicalProgramSha256(program: KnifeSceneProgram): string {
  // A canonical SHA is a Runtime-owned truth field.  Returning the source
  // value preserves lineage and keeps this pure planner from impersonating
  // Runtime/CAS canonicalization.
  return program.canonical_sha256
}

function fingerprintPlan(plan: KnifeKnowledgeCandidatePlan | Record<string, unknown>): string {
  return stableFingerprint(canonicalJson({ ...plan, deterministic_fingerprint: '' }))
}

function programForHash(program: KnifeSceneProgram): KnifeSceneProgram {
  return { ...program, canonical_sha256: '' }
}

function assemblyPrimitiveCount(program: KnifeSceneProgram): number {
  const assembly = program.assembly
  if (!assembly) return 0
  return (assembly.guard ? 1 : 0) + (assembly.grip ? 1 : 0) + (assembly.pommel ? 1 : 0)
    + (assembly.fasteners?.length ?? 0) + (assembly.gems?.length ?? 0) + (assembly.reliefs?.length ?? 0)
}

function assemblyPartIdSet(assembly: unknown): Set<string> {
  const result = new Set<string>()
  if (!isRecord(assembly)) return result
  for (const key of ['guard', 'grip', 'pommel'] as const) {
    const item = assembly[key]
    if (isRecord(item) && typeof item.part_id === 'string') result.add(item.part_id)
  }
  for (const key of ['fasteners', 'gems', 'reliefs'] as const) {
    const items = assembly[key]
    if (Array.isArray(items)) for (const item of items) if (isRecord(item) && typeof item.part_id === 'string') result.add(item.part_id)
  }
  return result
}

function findSection(program: KnifeSceneProgram, role: KnifeSection['role']): KnifeSection | undefined {
  return program.blade_surface.sections.find((section) => section.role === role)
}

function sectionIndex(program: KnifeSceneProgram, role: KnifeSection['role']): number {
  const index = program.blade_surface.sections.findIndex((section) => section.role === role)
  if (index < 0) throw new KnifeKnowledgeCandidatePlanError('NO_MUTABLE_SCOPE', `missing section role ${role}`)
  return index
}

function nearestControlPointIndex(points: readonly (readonly [number, number, number])[], u: number): number {
  let best = 1
  let distance = Number.POSITIVE_INFINITY
  for (let index = 1; index < points.length - 1; index += 1) {
    const candidateU = (points[index][0] - points[0][0]) / Math.max(points.at(-1)![0] - points[0][0], Number.EPSILON)
    const candidateDistance = Math.abs(candidateU - u)
    if (candidateDistance < distance) {
      best = index
      distance = candidateDistance
    }
  }
  return best
}

function isDragonGuard(value: unknown): value is KnifeDragonGuardSpec {
  return isRecord(value) && value.style === 'dragon-guard' && Array.isArray(value.horns)
}

function isSegmentedGrip(value: unknown): value is Extract<KnifeGripSpec, { style: 'segmented-grip' }> {
  return isRecord(value) && value.style === 'segmented-grip' && Array.isArray(value.segments)
}

function isHookedPommel(value: unknown): value is KnifeHookedPommelSpec {
  return isRecord(value) && value.style === 'hooked-pommel' && isRecord(value.hook)
}

function validateRefs(value: unknown): asserts value is readonly string[] {
  if (!Array.isArray(value) || value.length === 0 || value.some((item) => typeof item !== 'string' || item.length === 0 || item.length > 120)) failPlan('source_refs must be a bounded non-empty string array')
}

function isMutationScope(value: unknown): value is KnifeKnowledgeMutationScope {
  return typeof value === 'string' && (KNIFE_KNOWLEDGE_MUTATION_SCOPES as readonly string[]).includes(value)
}

function exactKeys(value: Record<string, unknown>, required: readonly string[], label: string, optional: readonly string[] = []): void {
  const allowed = new Set([...required, ...optional])
  const keys = Object.keys(value)
  if (keys.some((key) => !allowed.has(key)) || required.some((key) => !Object.prototype.hasOwnProperty.call(value, key)) || optional.some((key) => Object.prototype.hasOwnProperty.call(value, key) && value[key] === undefined)) {
    throw new KnifeKnowledgeCandidatePlanError('INVALID_PROGRAM', `${label} contains unknown, missing, or undefined keys`)
  }
}

function finite(value: unknown, label: string): asserts value is number {
  if (typeof value !== 'number' || !Number.isFinite(value)) invalidProgram(`${label} must be finite`)
}

function finiteRange(value: unknown, minimum: number, maximum: number, label: string, exclusiveMinimum = false): asserts value is number {
  finite(value, label)
  if (exclusiveMinimum ? value <= minimum || value > maximum : value < minimum || value > maximum) invalidProgram(`${label} is outside the closed range`)
}

function validateSeed(value: unknown): number {
  if (!Number.isInteger(value) || (value as number) < 0 || (value as number) > KNIFE_KNOWLEDGE_CANDIDATE_LIMITS.max_seed) throw new KnifeKnowledgeCandidatePlanError('INVALID_OPTIONS', 'seed must be an integer in [0, 2^32-1]')
  return value as number
}

function validateCandidateCount(value: unknown): number {
  if (!Number.isInteger(value) || (value as number) < KNIFE_KNOWLEDGE_CANDIDATE_LIMITS.min_candidate_count || (value as number) > KNIFE_KNOWLEDGE_CANDIDATE_LIMITS.max_candidate_count) throw new KnifeKnowledgeCandidatePlanError('INVALID_OPTIONS', 'candidate_count must be an integer in [2,4]')
  return value as number
}

function fingerprintText(value: unknown, label: string): asserts value is string {
  if (typeof value !== 'string' || !/^[a-f0-9]{16,128}$/.test(value)) failPlan(`${label} must be a lowercase stable fingerprint`)
}

function shaText(value: unknown, label: string, allowEmpty: boolean): asserts value is string {
  if (typeof value === 'string' && ((allowEmpty && value === '') || MIN_PROGRAM_SHA.test(value))) return
  failPlan(`${label} must be a lowercase SHA-256${allowEmpty ? ' or empty' : ''}`)
}

function failPlan(message: string): never {
  throw new KnifeKnowledgeCandidatePlanError('INVALID_PLAN', message)
}

function invalidProgram(message: string): never {
  throw new KnifeKnowledgeCandidatePlanError('INVALID_PROGRAM', message)
}

function isRecord(value: unknown): value is Record<string, any> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value)
}

type MutableProgram = any
type MutableDragonGuard = any
type MutableGrip = any
type MutableSegmentedGrip = any
type MutableHookedPommel = any

function clone<T>(value: T): T {
  if (Array.isArray(value)) return value.map((item) => clone(item)) as T
  if (value && typeof value === 'object') {
    const output: Record<string, unknown> = {}
    for (const [key, child] of Object.entries(value as Record<string, unknown>)) output[key] = clone(child)
    return output as T
  }
  return value
}

function deepFreeze<T>(value: T): T {
  if (!value || typeof value !== 'object' || Object.isFrozen(value)) return value
  for (const child of Object.values(value as Record<string, unknown>)) deepFreeze(child)
  return Object.freeze(value)
}

function round6(value: number): number {
  return Object.is(value, -0) ? 0 : Number(value.toFixed(6))
}

function round12(value: number): number {
  return Object.is(value, -0) ? 0 : Number(value.toPrecision(12))
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value))
}

function canonicalJson(value: unknown): string {
  if (value === null) return 'null'
  if (typeof value === 'string') return JSON.stringify(value)
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new KnifeKnowledgeCandidatePlanError('INVALID_PLAN', 'canonical JSON cannot contain non-finite numbers')
    return Object.is(value, -0) ? '0' : JSON.stringify(value)
  }
  if (Array.isArray(value)) return `[${value.map((item) => canonicalJson(item)).join(',')}]`
  if (isRecord(value)) return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`
  throw new KnifeKnowledgeCandidatePlanError('INVALID_PLAN', 'canonical JSON cannot contain undefined or executable values')
}

function stableFingerprint(input: string): string {
  let hash = 0xcbf29ce484222325n
  for (const character of input) {
    hash ^= BigInt(character.codePointAt(0)!)
    hash = BigInt.asUintN(64, hash * 0x100000001b3n)
  }
  return hash.toString(16).padStart(16, '0')
}
