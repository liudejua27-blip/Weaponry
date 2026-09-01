import dragonfang from '../../../skills/weaponry-threejs-knife-studio/references/dragonfang-first-slice.json' with { type: 'json' }
import type { KnifeSceneProgram } from '../src/knife-scene-program.ts'
import { IMG2THREEJS_SOURCE_IDENTITY } from '../src/img2threejs-source-envelope.ts'
import {
  KNIFE_KNOWLEDGE_MUTATION_SCOPES,
  KnifeKnowledgeCandidatePlanError,
  generateKnifeKnowledgeCandidatePlan,
  normalizeKnifeKnowledgeGoalWeights,
  validateKnifeKnowledgeCandidatePlan,
  type KnifeKnowledgeGoalWeights,
} from '../src/knife-knowledge-candidate-generator.ts'

const native = dragonfang as unknown as KnifeSceneProgram
const weights = Object.fromEntries(KNIFE_KNOWLEDGE_MUTATION_SCOPES.map((scope) => [scope, 1])) as KnifeKnowledgeGoalWeights
const sourceSnapshot = JSON.stringify(native)

const firstPlan = generateKnifeKnowledgeCandidatePlan(native, {
  goal_weights: weights,
  candidate_count: 4,
  seed: 20260901,
})
const secondPlan = generateKnifeKnowledgeCandidatePlan(native, {
  goal_weights: weights,
  candidate_count: 4,
  seed: 20260901,
})

validateKnifeKnowledgeCandidatePlan(firstPlan)
if (firstPlan.status !== 'PROPOSALS_READY' || firstPlan.generated_candidate_count !== 4) {
  throw new Error('native dragonfang plan did not produce four review-only proposals')
}
if (firstPlan.deterministic_fingerprint !== secondPlan.deterministic_fingerprint) {
  throw new Error('candidate plan fingerprint is not deterministic')
}
if (new Set(firstPlan.candidates.map((candidate) => candidate.candidate_program_fingerprint)).size !== 4) {
  throw new Error('candidate program fingerprints are not unique')
}
if (firstPlan.candidates.some((candidate) => candidate.proposal_status !== 'REVIEW_ONLY' || candidate.changed_parameter_paths.length === 0)) {
  throw new Error('candidate proposal boundary or change record is incomplete')
}
for (const candidate of firstPlan.candidates) {
  const bound = firstPlan.hard_bounds[candidate.mutation_scope]
  for (const change of candidate.changes) {
    if (Math.abs(change.delta) > bound.max_abs_delta || change.old_value === change.new_value) {
      throw new Error(`change escaped hard step bounds: ${change.path}`)
    }
  }
  if (!Object.isFrozen(candidate) || !Object.isFrozen(candidate.program) || !Object.isFrozen(candidate.changes)) {
    throw new Error('candidate outputs must be deeply frozen')
  }
}
if (JSON.stringify(native) !== sourceSnapshot) throw new Error('source program was mutated')
if (!Object.isFrozen(firstPlan) || !Object.isFrozen(firstPlan.goal_weights)) throw new Error('plan and normalized weights must be frozen')

const reliefPlusZeroWeightMutableProgram = {
  ...native,
  parts: native.parts.map((part) => ({ ...part, frozen: part.role !== 'relief' && part.role !== 'guard' })),
} as KnifeSceneProgram
const reliefOnlyWeights = Object.fromEntries(
  KNIFE_KNOWLEDGE_MUTATION_SCOPES.map((scope) => [scope, scope === 'relief-depth' ? 1 : 0]),
) as KnifeKnowledgeGoalWeights
let insufficientMutableScopeRejected = false
try {
  generateKnifeKnowledgeCandidatePlan(reliefPlusZeroWeightMutableProgram, {
    goal_weights: reliefOnlyWeights,
    candidate_count: 2,
    seed: 20260901,
  })
} catch (error) {
  if (!(error instanceof KnifeKnowledgeCandidatePlanError) || error.code !== 'INSUFFICIENT_MUTABLE_SCOPE') {
    throw error
  }
  insufficientMutableScopeRejected = true
}
if (!insufficientMutableScopeRejected) {
  throw new Error('candidate generation silently filled the requested budget with an unrelated mutable scope')
}

const duplicateSingletonRoleProgram = {
  ...native,
  parts: [
    ...native.parts,
    { ...native.parts.find((part) => part.role === 'guard')!, part_id: 'guard-fallback', frozen: true },
  ],
} as KnifeSceneProgram
let duplicateSingletonRoleRejected = false
try {
  generateKnifeKnowledgeCandidatePlan(duplicateSingletonRoleProgram, {
    goal_weights: weights,
    candidate_count: 2,
    seed: 20260901,
  })
} catch (error) {
  if (!(error instanceof KnifeKnowledgeCandidatePlanError) || error.code !== 'INVALID_PROGRAM') throw error
  duplicateSingletonRoleRejected = true
}
if (!duplicateSingletonRoleRejected) throw new Error('duplicate singleton part role was accepted')

const reviewInput = { ...native, design_basis: 'img2threejs-compatible-import' } as KnifeSceneProgram
const reviewPlan = generateKnifeKnowledgeCandidatePlan(reviewInput, {
  goal_weights: weights,
  candidate_count: 2,
  seed: 7,
})
if (reviewPlan.status !== 'REVIEW_ONLY' || reviewPlan.generated_candidate_count !== 0 || reviewPlan.direct_mutation_performed) {
  throw new Error('compatibility input crossed the review-only boundary')
}

const sourceEnvelope = {
  schema_version: 'Img2ThreeJsSourceEnvelope@1',
  source_schema_version: 'ObjectSculptSpec@2.1',
  source_identity: IMG2THREEJS_SOURCE_IDENTITY,
  target_name: 'smoke compatibility envelope',
  coordinate_frame: 'source-right-x-up-y-forward-z@1',
  components: [{
    component_id: 'blade',
    source_order: 0,
    role: 'blade',
    primitive: 'ground-blade',
    material_id: 'steel',
    parent_id: null,
    transform: {
      position: [0, 0, 0],
      rotation_xyz: [0, 0, 0],
      scale: [1, 1, 1],
      pivot: [0, 0, 0],
      rotation_order: 'XYZ',
    },
    geometry: {
      primitive: 'ground-blade',
      stations: [[0, 0.08, -0.08], [0.3, 0.1, -0.07], [0.7, 0.06, -0.03], [1, 0.01, -0.01]],
      thickness: 0.05,
      grind_frac: 0.5,
      swedge_from_tip_frac: 0.3,
    },
  }],
  materials: [{
    material_id: 'steel',
    source_order: 0,
    base_color: '#777777',
    metalness: 0.8,
    roughness: 0.3,
  }],
  tessellation: 'standard',
  max_triangles: 2048,
} as unknown as NonNullable<KnifeSceneProgram['source_envelope']>
const envelopeReviewInput = { ...native, source_envelope: sourceEnvelope } as KnifeSceneProgram
const envelopeReviewPlan = generateKnifeKnowledgeCandidatePlan(envelopeReviewInput, {
  goal_weights: weights,
  candidate_count: 2,
  seed: 11,
})
if (envelopeReviewPlan.status !== 'REVIEW_ONLY' || envelopeReviewPlan.generated_candidate_count !== 0 || envelopeReviewPlan.direct_mutation_performed) {
  throw new Error('source-envelope input crossed the review-only boundary')
}

let closedWeightsRejected = false
try {
  normalizeKnifeKnowledgeGoalWeights({ ...weights, unexpected: 1 })
} catch {
  closedWeightsRejected = true
}
if (!closedWeightsRejected) throw new Error('goal weights accepted an unknown key')

console.log(JSON.stringify({
  smoke: 'knife-knowledge-candidate-generator@1',
  native_asset_id: native.asset_id,
  candidate_count: firstPlan.generated_candidate_count,
  mutation_scopes: firstPlan.mutation_scopes,
  deterministic_fingerprint: firstPlan.deterministic_fingerprint,
  compatibility_status: reviewPlan.status,
  source_envelope_status: envelopeReviewPlan.status,
}))
