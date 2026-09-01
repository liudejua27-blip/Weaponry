import { knifeSceneProgramFixture } from '../fixtures/knife-scene-program.fixture.ts'
import { compileKnifeScene } from '../src/knife-scene-compiler.ts'
import {
  createKnifeObjectiveLedger,
  type KnifeObjectiveLedger,
} from '../src/knife-objective-ledger.ts'
import {
  evaluateKnifeRig,
  createKnifeViewRig,
} from '../src/knife-view-evaluation.ts'
import { measureKnifePartVisibilityMetrics } from '../src/knife-part-visibility-metrics.ts'
import { measureKnifeGuardFpsMetrics } from '../src/knife-guard-fps-metrics.ts'
import type { KnifeSceneProgram } from '../src/knife-scene-program.ts'
import {
  canonicalKnifeProgramSha256,
  measureKnifeObjectiveMetricValues,
  KnifeObjectiveMetricAdapterError,
} from '../src/knife-objective-metric-adapter.ts'

const program = knifeSceneProgramFixture as unknown as KnifeSceneProgram
const programSha = canonicalKnifeProgramSha256(program)
const ledger = createKnifeObjectiveLedger({
  schema_version: 'KnifeObjectiveLedger@1',
  ledger_id: 'objective-metric-adapter-smoke',
  revision: 0,
  parent_ledger_sha256: null,
  program_sha256: programSha,
  baseline_candidate_sha256: 'a'.repeat(64),
  stage: 'structural',
  allowed_scope: ['guard'],
  frozen_parts: ['blade-body', 'cutting-edge', 'grip', 'pommel'],
  hypothesis: 'A fixed-view structural metric adapter remains bound to the source program.',
  objective_metrics: ['part-id-coverage', 'material-id-coverage', 'negative-space-error', 'fps-occupancy'],
  regression_limits: ['silhouette-iou', 'boundary-f1', 'symmetric-chamfer', 'p95-contour-distance', 'thickness-continuity', 'normal-continuity', 'tip-landmark-error', 'belly-depth-error'],
  candidate_budget: 4,
  minimum_improvement: 0.005,
  plateau_limit: 2,
  evidence_sha256: [programSha],
  status: 'active',
  canonical_sha256: '',
}) satisfies KnifeObjectiveLedger

const compiled = compileKnifeScene(program, { longitudinal_segments: 16 })
const rig = createKnifeViewRig({ frame_width: 64, frame_height: 48 })
const evaluation = evaluateKnifeRig(compiled, rig)
const visibility = measureKnifePartVisibilityMetrics(compiled, rig)
const guardFps = measureKnifeGuardFpsMetrics(compiled, rig)

const first = measureKnifeObjectiveMetricValues({ program, source_program_sha256: ledger.program_sha256, ledger, compiled, evaluation, visibility, guard_fps: guardFps })
const second = measureKnifeObjectiveMetricValues({ program, source_program_sha256: ledger.program_sha256, ledger, compiled, evaluation, visibility, guard_fps: guardFps })
const expectedMetrics = [...new Set([...ledger.objective_metrics, ...ledger.regression_limits])].sort()
const actualMetrics = Object.keys(first.metrics).sort()
if (actualMetrics.join('|') !== expectedMetrics.join('|')) {
  throw new Error('adapter did not emit the complete Ledger objective and regression metric union')
}
if (first.candidate_program_sha256 !== programSha
  || first.source_fingerprint !== compiled.deterministic_fingerprint
  || first.source_program_sha256 !== ledger.program_sha256
  || first.rig_fingerprint !== rig.deterministic_fingerprint
  || first.status !== 'MEASURED_NOT_REVIEWED'
  || first.renderer_invoked
  || first.quality_status !== 'NOT_RUN') {
  throw new Error('adapter output crossed the program, source, rig or quality boundary')
}
for (const metric of ['part-id-coverage', 'material-id-coverage', 'fps-occupancy'] as const) {
  const value = first.metrics[metric]
  if (typeof value !== 'number' || value <= 0 || value > 1) throw new Error(`${metric} was not computed from the fixed-view masks`)
}
for (const metric of [
  'negative-space-error',
  'silhouette-iou',
  'boundary-f1',
  'symmetric-chamfer',
  'p95-contour-distance',
  'tip-landmark-error',
  'belly-depth-error',
  'thickness-continuity',
  'normal-continuity',
] as const) {
  if (first.metrics[metric] !== 'NOT_COMPUTABLE') throw new Error(`${metric} was incorrectly inferred`)
}
if (first.metric_evidence.find((metric) => metric.metric === 'negative-space-error')?.basis
  !== 'guard-convex-hull-visible-opening-proxy-without-bound-reference-target@1') {
  throw new Error('negative-space error did not retain its visible-opening proxy boundary')
}
if (first.metric_evidence.find((metric) => metric.metric === 'material-id-coverage')?.basis
  !== 'eight-view-depth-resolved-material-id-union@1') {
  throw new Error('material-id coverage did not bind to per-pixel material indices')
}
if (first.deterministic_fingerprint !== second.deterministic_fingerprint) throw new Error('adapter receipt is not deterministic')
if (first.metric_receipt_fingerprints['fps-occupancy'].length !== 3) throw new Error('FPS metric receipt lineage is incomplete')
if (first.metric_receipt_fingerprints['silhouette-iou'].length !== 0) throw new Error('reference metric has fabricated evidence')

const coveredIndex = evaluation.views[0].mask.pixels.findIndex((pixel) => pixel !== 0)
if (coveredIndex < 0) throw new Error('fixture did not produce a covered fixed-view pixel')
const tamperedMaterialIndices = new Uint16Array(evaluation.views[0].mask.material_indices)
tamperedMaterialIndices[coveredIndex] = 0xffff
const tamperedEvaluation = {
  ...evaluation,
  views: evaluation.views.map((view, index) => index === 0
    ? { ...view, mask: { ...view.mask, material_indices: tamperedMaterialIndices } }
    : view),
}
let tamperedRejected = false
try {
  measureKnifeObjectiveMetricValues({
    program,
    source_program_sha256: ledger.program_sha256,
    ledger,
    compiled,
    evaluation: tamperedEvaluation,
    visibility,
    guard_fps: guardFps,
  })
} catch (error) {
  tamperedRejected = error instanceof KnifeObjectiveMetricAdapterError
}
if (!tamperedRejected) throw new Error('tampered fixed-view material evidence was accepted')

console.log(JSON.stringify({
  smoke: 'knife-objective-metric-adapter@1',
  candidate_program_sha256: first.candidate_program_sha256,
  source_fingerprint: first.source_fingerprint,
  rig_fingerprint: first.rig_fingerprint,
  metrics: first.metrics,
  metric_evidence: first.metric_evidence,
  deterministic_fingerprint: first.deterministic_fingerprint,
}))
