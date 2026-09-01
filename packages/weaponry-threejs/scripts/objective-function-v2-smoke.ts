import ledger from '../../../skills/weaponry-threejs-knife-studio/references/dragonfang-objective-ledger-r5.json' with { type: 'json' }
import {
  KNIFE_OBJECTIVE_NOT_COMPUTABLE,
  createKnifeObjectiveCandidateV2,
  createKnifeObjectiveFunctionV2FromLedger,
  evaluateKnifeObjectiveFunctionV2,
  type KnifeObjectiveFunctionV2LedgerDraft,
  type KnifeObjectiveMetricTargetV2,
} from '../src/knife-objective-function-v2.ts'
import type { KnifeObjectiveLedger } from '../src/knife-objective-ledger.ts'

const sourceLedger = ledger as unknown as KnifeObjectiveLedger
const metricTargets: readonly KnifeObjectiveMetricTargetV2[] = Object.freeze([
  {
    metric: 'negative-space-error',
    role: 'objective',
    direction: 'minimize',
    target_interval: Object.freeze({ min: 0, max: 1 }),
    minimum_improvement: sourceLedger.minimum_improvement,
    regression_limit: 0.05,
    evidence_class: 'structural-proxy',
    required: true,
  },
  {
    metric: 'part-id-coverage',
    role: 'objective',
    direction: 'maximize',
    target_interval: Object.freeze({ min: 0, max: 1 }),
    minimum_improvement: sourceLedger.minimum_improvement,
    regression_limit: 0.05,
    evidence_class: 'structural-proxy',
    required: true,
  },
  {
    metric: 'material-id-coverage',
    role: 'objective',
    direction: 'maximize',
    target_interval: Object.freeze({ min: 0, max: 1 }),
    minimum_improvement: sourceLedger.minimum_improvement,
    regression_limit: 0.05,
    evidence_class: 'structural-proxy',
    required: true,
  },
  {
    metric: 'fps-occupancy',
    role: 'objective',
    direction: 'maximize',
    target_interval: Object.freeze({ min: 0, max: 1 }),
    minimum_improvement: sourceLedger.minimum_improvement,
    regression_limit: 0.05,
    evidence_class: 'structural-proxy',
    required: false,
  },
  {
    metric: 'silhouette-iou',
    role: 'regression',
    direction: 'maximize',
    target_interval: Object.freeze({ min: 0.7, max: 1 }),
    minimum_improvement: 0,
    regression_limit: 0.05,
    evidence_class: 'visual-evidence',
    required: true,
  },
  {
    metric: 'boundary-f1',
    role: 'regression',
    direction: 'maximize',
    target_interval: Object.freeze({ min: 0.6, max: 1 }),
    minimum_improvement: 0,
    regression_limit: 0.05,
    evidence_class: 'visual-evidence',
    required: true,
  },
  {
    metric: 'tip-landmark-error',
    role: 'regression',
    direction: 'minimize',
    target_interval: Object.freeze({ min: 0, max: 0.3 }),
    minimum_improvement: 0,
    regression_limit: 0.05,
    evidence_class: 'visual-evidence',
    required: true,
  },
  {
    metric: 'belly-depth-error',
    role: 'regression',
    direction: 'minimize',
    target_interval: Object.freeze({ min: 0, max: 0.3 }),
    minimum_improvement: 0,
    regression_limit: 0.05,
    evidence_class: 'visual-evidence',
    required: true,
  },
  {
    metric: 'thickness-continuity',
    role: 'regression',
    direction: 'maximize',
    target_interval: Object.freeze({ min: 0.7, max: 1 }),
    minimum_improvement: 0,
    regression_limit: 0.05,
    evidence_class: 'visual-evidence',
    required: true,
  },
  {
    metric: 'normal-continuity',
    role: 'regression',
    direction: 'maximize',
    target_interval: Object.freeze({ min: 0.7, max: 1 }),
    minimum_improvement: 0,
    regression_limit: 0.05,
    evidence_class: 'visual-evidence',
    required: true,
  },
])

const objective = createKnifeObjectiveFunctionV2FromLedger({
  ledger: sourceLedger,
  objective_id: 'objective-v2-smoke',
  metric_targets: metricTargets,
  baseline_values: {
    'negative-space-error': 0.5,
    'part-id-coverage': 0.8,
    'material-id-coverage': 0.8,
    'fps-occupancy': 0.5,
    'silhouette-iou': 0.8,
    'boundary-f1': 0.7,
    'tip-landmark-error': 0.2,
    'belly-depth-error': 0.2,
    'thickness-continuity': 0.8,
    'normal-continuity': 0.8,
  },
} satisfies KnifeObjectiveFunctionV2LedgerDraft)

const expectedObjectiveMetrics = new Set(sourceLedger.objective_metrics)
const expectedRegressionMetrics = new Set(sourceLedger.regression_limits)
if (objective.metric_targets.length !== expectedObjectiveMetrics.size + expectedRegressionMetrics.size) {
  throw new Error('FromLedger did not require the complete objective/regression metric union')
}
for (const target of objective.metric_targets) {
  const expectedRole = expectedObjectiveMetrics.has(target.metric) ? 'objective' : expectedRegressionMetrics.has(target.metric) ? 'regression' : undefined
  if (target.role !== expectedRole) throw new Error(`Ledger role drifted for ${target.metric}`)
}
let incompleteLedgerRejected = false
try {
  createKnifeObjectiveFunctionV2FromLedger({
    ledger: sourceLedger,
    objective_id: 'objective-v2-incomplete',
    metric_targets: metricTargets.slice(0, -1),
    baseline_values: {
      'negative-space-error': 0.5,
      'part-id-coverage': 0.8,
      'material-id-coverage': 0.8,
      'fps-occupancy': 0.5,
      'silhouette-iou': 0.8,
      'boundary-f1': 0.7,
      'tip-landmark-error': 0.2,
      'belly-depth-error': 0.2,
      'thickness-continuity': 0.8,
    },
  })
} catch (error) {
  incompleteLedgerRejected = error instanceof Error && error.message.includes('LEDGER_BINDING_MISMATCH')
}
if (!incompleteLedgerRejected) throw new Error('incomplete Ledger@1 metric union was accepted')
let wrongRoleRejected = false
try {
  createKnifeObjectiveFunctionV2FromLedger({
    ledger: sourceLedger,
    objective_id: 'objective-v2-wrong-role',
    metric_targets: metricTargets.map((target, index) => index === 0 ? { ...target, role: 'regression' as const } : target),
    baseline_values: {
      'negative-space-error': 0.5,
      'part-id-coverage': 0.8,
      'material-id-coverage': 0.8,
      'fps-occupancy': 0.5,
      'silhouette-iou': 0.8,
      'boundary-f1': 0.7,
      'tip-landmark-error': 0.2,
      'belly-depth-error': 0.2,
      'thickness-continuity': 0.8,
      'normal-continuity': 0.8,
    },
  })
} catch (error) {
  wrongRoleRejected = error instanceof Error && error.message.includes('LEDGER_BINDING_MISMATCH')
}
if (!wrongRoleRejected) throw new Error('Ledger@1 metric role mismatch was accepted')

const candidate = (candidate_id: string, seed: string, values: Record<string, number | typeof KNIFE_OBJECTIVE_NOT_COMPUTABLE>) => createKnifeObjectiveCandidateV2({
  schema_version: 'KnifeObjectiveCandidate@2',
  candidate_id,
  candidate_sha256: seed.repeat(64),
  values,
})

const candidates = Object.freeze([
  candidate('alpha', '1', {
    'negative-space-error': 0.4,
    'part-id-coverage': 0.83,
    'material-id-coverage': 0.81,
    'fps-occupancy': 0.52,
    'silhouette-iou': 0.79,
    'boundary-f1': 0.69,
    'tip-landmark-error': 0.21,
    'belly-depth-error': 0.21,
    'thickness-continuity': 0.79,
    'normal-continuity': 0.79,
  }),
  candidate('zeta', '2', {
    'negative-space-error': 0.35,
    'part-id-coverage': 0.81,
    'material-id-coverage': 0.8,
    'fps-occupancy': 0.5,
    'silhouette-iou': 0.79,
    'boundary-f1': 0.69,
    'tip-landmark-error': 0.21,
    'belly-depth-error': 0.21,
    'thickness-continuity': 0.79,
    'normal-continuity': 0.79,
  }),
  candidate('dominated', '3', {
    'negative-space-error': 0.45,
    'part-id-coverage': 0.81,
    'material-id-coverage': 0.8,
    'fps-occupancy': 0.5,
    'silhouette-iou': 0.79,
    'boundary-f1': 0.69,
    'tip-landmark-error': 0.21,
    'belly-depth-error': 0.21,
    'thickness-continuity': 0.79,
    'normal-continuity': 0.79,
  }),
  candidate('partial', '4', {
    'negative-space-error': 0.39,
    'part-id-coverage': 0.81,
    'material-id-coverage': 0.8,
    'fps-occupancy': KNIFE_OBJECTIVE_NOT_COMPUTABLE,
    'silhouette-iou': 0.79,
    'boundary-f1': 0.69,
    'tip-landmark-error': 0.21,
    'belly-depth-error': 0.21,
    'thickness-continuity': 0.79,
    'normal-continuity': 0.79,
  }),
  candidate('blocked', '5', {
    'negative-space-error': KNIFE_OBJECTIVE_NOT_COMPUTABLE,
    'part-id-coverage': 0.84,
    'material-id-coverage': 0.82,
    'fps-occupancy': 0.53,
    'silhouette-iou': 0.79,
    'boundary-f1': 0.69,
    'tip-landmark-error': 0.21,
    'belly-depth-error': 0.21,
    'thickness-continuity': 0.79,
    'normal-continuity': 0.79,
  }),
  candidate('regression-tradeoff', '6', {
    'negative-space-error': 0.4,
    'part-id-coverage': 0.83,
    'material-id-coverage': 0.81,
    'fps-occupancy': 0.52,
    'silhouette-iou': 0.76,
    'boundary-f1': 0.66,
    'tip-landmark-error': 0.24,
    'belly-depth-error': 0.24,
    'thickness-continuity': 0.76,
    'normal-continuity': 0.76,
  }),
  candidate('regression-blocked', '7', {
    'negative-space-error': 0.4,
    'part-id-coverage': 0.83,
    'material-id-coverage': 0.81,
    'fps-occupancy': 0.52,
    'silhouette-iou': 0.7,
    'boundary-f1': 0.69,
    'tip-landmark-error': 0.21,
    'belly-depth-error': 0.21,
    'thickness-continuity': 0.79,
    'normal-continuity': 0.79,
  }),
])

const first = evaluateKnifeObjectiveFunctionV2(objective, candidates)
const second = evaluateKnifeObjectiveFunctionV2(objective, candidates)
const reversed = evaluateKnifeObjectiveFunctionV2(objective, [...candidates].reverse())
if (first.deterministic_fingerprint !== second.deterministic_fingerprint) throw new Error('selection receipt is not deterministic')
if (first.deterministic_fingerprint !== reversed.deterministic_fingerprint) throw new Error('selection receipt depends on candidate input order')
if (first.selection_status !== 'REVIEW_ONLY_SELECTION') throw new Error('eligible structural candidates were not review-selected')
if (first.selected_candidate_id !== 'alpha') throw new Error(`lexical Pareto tie-break drifted: ${first.selected_candidate_id}`)
if (first.selection_basis !== 'direction-aware-pareto@1/computability-first/lexical-tie-break') throw new Error('selection basis drifted')
if (!first.pareto_candidate_ids.includes('alpha') || !first.pareto_candidate_ids.includes('zeta')) throw new Error('direction-aware Pareto front lost non-dominated candidates')
if (!first.pareto_candidate_ids.includes('regression-tradeoff')) throw new Error('regression-role metric incorrectly became a Pareto dimension')
if (first.pareto_candidate_ids.includes('dominated') || first.pareto_candidate_ids.includes('blocked') || first.pareto_candidate_ids.includes('regression-blocked')) throw new Error('dominated or rejected candidate entered Pareto front')
if (first.decision_label !== 'NON_VISUAL_STRUCTURAL_RANKING') throw new Error('structural selection label drifted')
if (first.visual_status !== 'NOT_REVIEWED' || first.quality_status !== 'NOT_RUN' || first.human_status !== 'NOT_RUN') {
  throw new Error('structural proxy was represented as a visual or quality result')
}
const blocked = first.candidate_evaluations.find((evaluation) => evaluation.candidate_id === 'blocked')
if (!blocked || blocked.objective_gate !== KNIFE_OBJECTIVE_NOT_COMPUTABLE || !blocked.non_computable_metrics.includes('negative-space-error')) {
  throw new Error('required NOT_COMPUTABLE metric did not fail closed')
}
const partial = first.candidate_evaluations.find((evaluation) => evaluation.candidate_id === 'partial')
if (!partial || partial.computability !== 'PARTIAL' || partial.objective_gate !== 'ELIGIBLE') {
  throw new Error('optional NOT_COMPUTABLE metric was coerced or rejected incorrectly')
}
const regressionBlocked = first.candidate_evaluations.find((evaluation) => evaluation.candidate_id === 'regression-blocked')
if (!regressionBlocked || regressionBlocked.objective_gate !== 'REJECTED' || !regressionBlocked.regression_metrics.includes('silhouette-iou')) {
  throw new Error('regression-role metric did not remain a hard regression constraint')
}

const baselineMissingObjective = createKnifeObjectiveFunctionV2FromLedger({
  ledger: sourceLedger,
  objective_id: 'objective-v2-baseline-missing',
  metric_targets: metricTargets,
  baseline_values: {
    'negative-space-error': 0.5,
    'part-id-coverage': 0.8,
    'material-id-coverage': 0.8,
    'fps-occupancy': 0.5,
    'silhouette-iou': KNIFE_OBJECTIVE_NOT_COMPUTABLE,
    'boundary-f1': 0.7,
    'tip-landmark-error': 0.2,
    'belly-depth-error': 0.2,
    'thickness-continuity': 0.8,
    'normal-continuity': 0.8,
  },
})
const baselineMissingCandidate = candidate('baseline-missing', '8', {
  'negative-space-error': 0.4,
  'part-id-coverage': 0.83,
  'material-id-coverage': 0.81,
  'fps-occupancy': 0.52,
  'silhouette-iou': 0.79,
  'boundary-f1': 0.69,
  'tip-landmark-error': 0.21,
  'belly-depth-error': 0.21,
  'thickness-continuity': 0.79,
  'normal-continuity': 0.79,
})
const baselineMissingReceipt = evaluateKnifeObjectiveFunctionV2(baselineMissingObjective, [baselineMissingCandidate])
const baselineMissingEvaluation = baselineMissingReceipt.candidate_evaluations[0]
const missingSilhouette = baselineMissingEvaluation.metrics.find((metric) => metric.metric === 'silhouette-iou')
if (!missingSilhouette || missingSilhouette.target_status !== KNIFE_OBJECTIVE_NOT_COMPUTABLE || baselineMissingEvaluation.objective_gate !== KNIFE_OBJECTIVE_NOT_COMPUTABLE) {
  throw new Error('baseline NOT_COMPUTABLE evidence was coerced into a target or objective result')
}

console.log(JSON.stringify({
  smoke: 'knife-objective-function@2',
  objective_sha256: objective.canonical_sha256,
  selection_receipt_sha256: first.deterministic_fingerprint,
  pareto_candidate_ids: first.pareto_candidate_ids,
  selected_candidate_id: first.selected_candidate_id,
  selection_status: first.selection_status,
  decision_label: first.decision_label,
  visual_status: first.visual_status,
  quality_status: first.quality_status,
  blocked_gate: blocked.objective_gate,
  partial_computability: partial.computability,
  baseline_missing_target_status: missingSilhouette.target_status,
  regression_blocked_gate: regressionBlocked.objective_gate,
}))
