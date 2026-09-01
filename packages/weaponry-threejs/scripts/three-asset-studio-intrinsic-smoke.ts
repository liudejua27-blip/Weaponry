import dragonfangProgram from '../../../skills/weaponry-threejs-knife-studio/references/dragonfang-first-slice.json' with { type: 'json' }
import dragonfangLedger from '../../../skills/weaponry-threejs-knife-studio/references/dragonfang-objective-ledger-r6-intrinsic.json' with { type: 'json' }
import { ThreeAssetStudioController } from '../src/three-asset-studio.ts'
import type { KnifeObjectiveLedger } from '../src/knife-objective-ledger.ts'
import { createKnifeObjectiveFunctionV2FromLedger, type KnifeObjectiveMetricTargetV2 } from '../src/knife-objective-function-v2.ts'
import type { KnifeSceneProgram } from '../src/knife-scene-program.ts'

const program = dragonfangProgram as unknown as KnifeSceneProgram
const ledger = dragonfangLedger as unknown as KnifeObjectiveLedger

const controller = new ThreeAssetStudioController()
const design = await controller.dispatch({
  action: 'knife_design_create',
  request_id: 'intrinsic-design',
  program,
})
if (design.action !== 'knife_design_create') throw new Error('intrinsic design was not created')
const candidates = await controller.dispatch({
  action: 'candidates_generate',
  request_id: 'intrinsic-candidates',
  design_id: design.design_id,
  objective_ledger: ledger,
  candidate_count: 3,
  seed: 0x44524736,
})
if (candidates.action !== 'candidates_generate'
  || candidates.status !== 'CANDIDATES_GENERATED'
  || candidates.baseline_objective_metrics?.schema_version !== 'WeaponryThreeJsKnifeObjectiveMetricAdapter@2'
  || candidates.candidates.some((candidate) => candidate.objective_metrics.schema_version !== 'WeaponryThreeJsKnifeObjectiveMetricAdapter@2')) {
  throw new Error('Studio did not select Adapter@2 for the intrinsic successor ledger')
}
const baseline = candidates.baseline_objective_metrics.metrics
const targets: readonly KnifeObjectiveMetricTargetV2[] = [
  ...ledger.objective_metrics.map((metric) => target(metric, 'objective')),
  ...ledger.regression_limits.map((metric) => target(metric, 'regression')),
]
const objective = createKnifeObjectiveFunctionV2FromLedger({
  ledger,
  objective_id: 'studio-intrinsic-objective',
  metric_targets: targets,
  baseline_values: baseline,
})
const optimized = await controller.dispatch({
  action: 'optimize',
  request_id: 'intrinsic-optimize',
  design_id: design.design_id,
  objective_function: objective,
})
if (optimized.action !== 'optimize'
  || optimized.objective_evaluation_status !== 'KnifeObjectiveFunction@2'
  || optimized.selection_receipt.quality_status !== 'NOT_RUN'
  || !['REVIEW_ONLY_SELECTION', 'PARENT_RETAINED'].includes(optimized.status)) {
  throw new Error('Studio intrinsic objective did not produce a truthful selection receipt')
}

console.log(JSON.stringify({
  smoke: 'three-asset-studio-intrinsic-objective@1',
  ledger_sha256: ledger.canonical_sha256,
  adapter_schema: candidates.baseline_objective_metrics.schema_version,
  baseline_metrics: baseline,
  mutation_scopes: candidates.candidates.map((candidate) => candidate.mutation_scope),
  selection_status: optimized.status,
  selected_candidate_id: optimized.selected_candidate_id,
  visual_status: optimized.visual_status,
  quality_status: optimized.quality_status,
}))

function target(
  metric: KnifeObjectiveMetricTargetV2['metric'],
  role: KnifeObjectiveMetricTargetV2['role'],
): KnifeObjectiveMetricTargetV2 {
  return {
    metric,
    role,
    direction: 'maximize',
    target_interval: { min: 0, max: 1 },
    minimum_improvement: role === 'objective' ? ledger.minimum_improvement : 0,
    regression_limit: 0.05,
    evidence_class: 'structural-proxy',
    required: true,
  }
}
