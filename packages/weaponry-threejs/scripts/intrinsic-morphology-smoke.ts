import { knifeSceneProgramFixture } from '../fixtures/knife-scene-program.fixture.ts'
import {
  KNIFE_INTRINSIC_MORPHOLOGY_NORMALIZATION,
  KNIFE_INTRINSIC_MORPHOLOGY_SAMPLE_COUNT,
  KNIFE_INTRINSIC_MORPHOLOGY_SCHEMA,
  KNIFE_INTRINSIC_MORPHOLOGY_STATUS,
  KnifeIntrinsicMorphologyError,
  measureKnifeIntrinsicMorphology,
} from '../src/knife-intrinsic-morphology.ts'

const first = measureKnifeIntrinsicMorphology(knifeSceneProgramFixture)
const second = measureKnifeIntrinsicMorphology({ program: knifeSceneProgramFixture })

if (first.schema_version !== KNIFE_INTRINSIC_MORPHOLOGY_SCHEMA
  || first.status !== KNIFE_INTRINSIC_MORPHOLOGY_STATUS
  || first.normalization !== KNIFE_INTRINSIC_MORPHOLOGY_NORMALIZATION
  || first.sample_count !== KNIFE_INTRINSIC_MORPHOLOGY_SAMPLE_COUNT
  || first.renderer_invoked
  || first.quality_status !== 'NOT_RUN'
  || !first.gates.structural_gate_pass) {
  throw new Error('intrinsic morphology crossed its structural/no-render boundary')
}

if (first.deterministic_fingerprint !== second.deterministic_fingerprint
  || first.source_fingerprint !== second.source_fingerprint
  || first.metrics.belly_dominance <= 0
  || first.metrics.tip_convergence_rate <= 0
  || first.metrics.tip_taper <= 0
  || first.metrics.curve_g1_proxy <= 0
  || first.curves.spine.extrema_count > first.curves.spine.extrema_budget
  || first.curves.cutting_edge.extrema_count > first.curves.cutting_edge.extrema_budget) {
  throw new Error('intrinsic morphology values are not deterministic or do not describe the fixture')
}

if (first.raw_metrics.belly_dominance_ratio <= 1
  || first.raw_metrics.tip_convergence_rate <= 0) {
  throw new Error('raw formula values were not retained beside normalized metrics')
}

for (const value of Object.values(first.metrics)) {
  if (!Number.isFinite(value) || value < 0 || value > 1) throw new Error('intrinsic normalized metric is outside [0,1]')
}
for (const record of Object.values(first.metric_records)) {
  if (record.classification !== KNIFE_INTRINSIC_MORPHOLOGY_STATUS
    || record.computability !== 'COMPUTED'
    || record.normalized_value < 0
    || record.normalized_value > 1
    || !Number.isFinite(record.value)) {
    throw new Error(`invalid evaluator-owned metric record for ${record.metric}`)
  }
}

if (!Object.isFrozen(first)
  || !Object.isFrozen(first.metrics)
  || !Object.isFrozen(first.sections)
  || !Object.isFrozen(first.curves)
  || !Object.isFrozen(first.curves.spine)
  || !Object.isFrozen(first.metric_records)) {
  throw new Error('intrinsic morphology receipt is not deeply frozen')
}

const bellyWider = {
  ...knifeSceneProgramFixture,
  blade_surface: {
    ...knifeSceneProgramFixture.blade_surface,
    sections: knifeSceneProgramFixture.blade_surface.sections.map((section) => section.role === 'belly'
      ? { ...section, half_width: section.half_width * 1.1 }
      : section),
  },
}
const changed = measureKnifeIntrinsicMorphology(bellyWider)
if (changed.metrics.belly_dominance === first.metrics.belly_dominance
  || changed.source_fingerprint === first.source_fingerprint) {
  throw new Error('intrinsic evaluator did not bind its values to the changed program')
}

function expectRejected(label: string, action: () => unknown): void {
  try {
    action()
  } catch (error) {
    if (!(error instanceof KnifeIntrinsicMorphologyError)) throw error
    return
  }
  throw new Error(`${label} was accepted`)
}

expectRejected('unknown input field', () => measureKnifeIntrinsicMorphology({ program: knifeSceneProgramFixture, extra: true } as never))
expectRejected('non-finite curve point', () => measureKnifeIntrinsicMorphology({
  ...knifeSceneProgramFixture,
  blade_surface: {
    ...knifeSceneProgramFixture.blade_surface,
    spine_curve: {
      ...knifeSceneProgramFixture.blade_surface.spine_curve,
      control_points: [[Number.NaN, 0, 0], ...knifeSceneProgramFixture.blade_surface.spine_curve.control_points.slice(1)],
    },
  },
} as never))

console.log(JSON.stringify({
  smoke_status: 'PASS',
  schema_version: first.schema_version,
  status: first.status,
  quality_status: first.quality_status,
  source_fingerprint: first.source_fingerprint,
  metrics: first.metrics,
  extrema: {
    spine: first.curves.spine.extrema_count,
    cutting_edge: first.curves.cutting_edge.extrema_count,
  },
  deterministic_fingerprint: first.deterministic_fingerprint,
}))
