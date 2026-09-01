import program from '../../../skills/weaponry-threejs-knife-studio/references/dragonfang-first-slice.json' with { type: 'json' }
import ledger from '../../../skills/weaponry-threejs-knife-studio/references/dragonfang-objective-ledger-r5.json' with { type: 'json' }
import type { KnifeSceneProgram } from '../src/knife-scene-program.ts'
import {
  canonicalKnifeObjectiveLedgerSha256,
  createKnifeObjectiveLedger,
  generateKnifeObjectiveLedgerCandidates,
  mapKnifeObjectiveLedgerToCandidateGeneration,
  validateKnifeObjectiveLedger,
  KnifeObjectiveLedgerError,
  type KnifeObjectiveLedger,
} from '../src/knife-objective-ledger.ts'

const sourceProgram = program as unknown as KnifeSceneProgram
const sourceLedger = ledger as unknown as KnifeObjectiveLedger
validateKnifeObjectiveLedger(sourceLedger, { require_canonical_sha256: true })
if (canonicalKnifeObjectiveLedgerSha256(sourceLedger) !== sourceLedger.canonical_sha256) throw new Error('r5 ledger canonical SHA drifted')

const firstBinding = mapKnifeObjectiveLedgerToCandidateGeneration(sourceProgram, sourceLedger, { seed: 20260901 })
const secondBinding = mapKnifeObjectiveLedgerToCandidateGeneration(sourceProgram, sourceLedger, { seed: 20260901 })
if (JSON.stringify(firstBinding) !== JSON.stringify(secondBinding)) throw new Error('ledger binding is not deterministic')
if (firstBinding.candidate_count > sourceLedger.candidate_budget || firstBinding.mutation_scopes.some((scope) => firstBinding.goal_weights[scope] <= 0)) {
  throw new Error('binding crossed the budget or zero-weight boundary')
}
const firstPlan = generateKnifeObjectiveLedgerCandidates(sourceProgram, sourceLedger, { seed: 20260901 })
const secondPlan = generateKnifeObjectiveLedgerCandidates(sourceProgram, sourceLedger, { seed: 20260901 })
if (firstPlan.deterministic_fingerprint !== secondPlan.deterministic_fingerprint) throw new Error('ledger candidate generation is not deterministic')
if (firstPlan.generated_candidate_count !== firstBinding.candidate_count) throw new Error('candidate count did not honor binding')

const zeroWeight = Object.fromEntries(Object.entries(firstBinding.goal_weights).map(([key, value]) => [key, value]))
zeroWeight['guard-jaw-gap'] = 0
let zeroWeightRejected = false
try {
  mapKnifeObjectiveLedgerToCandidateGeneration(sourceProgram, sourceLedger, {
    goal_weights: zeroWeight as never,
    candidate_count: firstBinding.candidate_count,
  })
} catch (error) {
  zeroWeightRejected = error instanceof KnifeObjectiveLedgerError
}
if (!zeroWeightRejected) throw new Error('zero-weight scope was not rejected')

const frozenBladeLedger = createKnifeObjectiveLedger({
  ...sourceLedger,
  ledger_id: 'frozen-blade-smoke',
  revision: 0,
  parent_ledger_sha256: null,
  allowed_scope: ['blade-body', 'cutting-edge'],
  frozen_parts: ['guard', 'grip', 'pommel'],
  evidence_sha256: [sourceLedger.program_sha256],
  canonical_sha256: '',
})
let frozenRejected = false
try {
  mapKnifeObjectiveLedgerToCandidateGeneration(sourceProgram, frozenBladeLedger)
} catch (error) {
  frozenRejected = error instanceof KnifeObjectiveLedgerError
}
if (!frozenRejected) throw new Error('program-frozen parts crossed the mutable boundary')

let closedRejected = false
try {
  validateKnifeObjectiveLedger({ ...sourceLedger, unexpected: true })
} catch (error) {
  closedRejected = error instanceof KnifeObjectiveLedgerError
}
if (!closedRejected) throw new Error('unknown ledger key was accepted')

console.log(JSON.stringify({
  smoke: 'knife-objective-ledger@1',
  ledger_sha256: sourceLedger.canonical_sha256,
  program_sha256: firstBinding.program_sha256,
  allowed_scope: firstBinding.allowed_scope,
  frozen_parts: firstBinding.frozen_parts,
  mutation_scopes: firstBinding.mutation_scopes,
  candidate_budget: firstBinding.candidate_budget,
  candidate_count: firstBinding.candidate_count,
  plan_fingerprint: firstPlan.deterministic_fingerprint,
  zero_weight_rejected: zeroWeightRejected,
  frozen_rejected: frozenRejected,
  closed_rejected: closedRejected,
}))
