import { knifeSceneProgramFixture } from '../fixtures/knife-scene-program.fixture.ts'
import { compileKnifeScene } from '../src/knife-scene-compiler.ts'
import { createKnifeObjectiveLedger } from '../src/knife-objective-ledger.ts'
import { createKnifeViewRig, evaluateKnifeRig } from '../src/knife-view-evaluation.ts'
import { measureKnifePartVisibilityMetrics } from '../src/knife-part-visibility-metrics.ts'
import { measureKnifeGuardFpsMetrics } from '../src/knife-guard-fps-metrics.ts'
import { canonicalKnifeProgramSha256 } from '../src/knife-objective-metric-adapter.ts'
import { measureKnifeObjectiveMetricValuesV2 } from '../src/knife-objective-metric-adapter-v2.ts'
import type { KnifeSceneProgram } from '../src/knife-scene-program.ts'

const program = knifeSceneProgramFixture as unknown as KnifeSceneProgram
const programSha = canonicalKnifeProgramSha256(program)
const ledger = createKnifeObjectiveLedger({
  schema_version: 'KnifeObjectiveLedger@1',
  ledger_id: 'intrinsic-adapter-smoke',
  revision: 1,
  parent_ledger_sha256: 'b'.repeat(64),
  program_sha256: programSha,
  baseline_candidate_sha256: 'c'.repeat(64),
  stage: 'form',
  allowed_scope: ['blade-body', 'cutting-edge'],
  frozen_parts: ['guard', 'grip', 'pommel'],
  hypothesis: 'Independent blade curves and ordered sections should rank without a reference image.',
  objective_metrics: [
    'blade-section-profile-continuity',
    'blade-curve-g1',
    'blade-tip-taper',
    'blade-extrema-headroom',
  ],
  regression_limits: ['part-id-coverage', 'fps-occupancy'],
  candidate_budget: 4,
  minimum_improvement: 0.005,
  plateau_limit: 2,
  evidence_sha256: [programSha],
  status: 'active',
  canonical_sha256: '',
})
const compiled = compileKnifeScene(program, { longitudinal_segments: 16 })
const rig = createKnifeViewRig({ frame_width: 64, frame_height: 48 })
const evaluation = evaluateKnifeRig(compiled, rig)
const visibility = measureKnifePartVisibilityMetrics(compiled, rig)
const guardFps = measureKnifeGuardFpsMetrics(compiled, rig)
const input = {
  program,
  source_program_sha256: ledger.program_sha256,
  ledger,
  compiled,
  evaluation,
  visibility,
  guard_fps: guardFps,
}
const first = measureKnifeObjectiveMetricValuesV2(input)
const second = measureKnifeObjectiveMetricValuesV2(input)

for (const metric of ledger.objective_metrics) {
  const value = first.metrics[metric]
  if (typeof value !== 'number' || value < 0 || value > 1) {
    throw new Error(`${metric} is not a bounded computed structural prior`)
  }
  if (first.metric_receipt_fingerprints[metric][0] !== first.intrinsic_morphology.deterministic_fingerprint) {
    throw new Error(`${metric} does not bind the intrinsic morphology receipt`)
  }
}
if (first.schema_version !== 'WeaponryThreeJsKnifeObjectiveMetricAdapter@2'
  || first.visual_quality_status !== 'NOT_COMPUTABLE'
  || first.quality_status !== 'NOT_RUN'
  || first.renderer_invoked
  || first.raster_receipt.schema_version !== 'WeaponryThreeJsKnifeObjectiveMetricAdapter@1'
  || first.deterministic_fingerprint !== second.deterministic_fingerprint) {
  throw new Error('Adapter@2 crossed its structural or deterministic boundary')
}

console.log(JSON.stringify({
  smoke: 'knife-objective-metric-adapter@2',
  metrics: first.metrics,
  intrinsic_morphology_fingerprint: first.intrinsic_morphology.deterministic_fingerprint,
  assembly_intrinsic_fingerprint: first.assembly_intrinsic.deterministic_fingerprint,
  deterministic_fingerprint: first.deterministic_fingerprint,
}))
