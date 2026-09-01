import { knifeSceneProgramFixture } from '../fixtures/knife-scene-program.fixture.ts'
import { compileKnifeScene } from '../src/knife-scene-compiler.ts'
import {
  KNIFE_ASSEMBLY_INTRINSIC_METRICS_SCHEMA,
  KNIFE_ASSEMBLY_INTRINSIC_METRICS_STATUS,
  measureKnifeAssemblyIntrinsicMetrics,
} from '../src/knife-assembly-intrinsic-metrics.ts'

const compiled = compileKnifeScene(knifeSceneProgramFixture)
const sourceBefore = JSON.stringify(knifeSceneProgramFixture)
const metrics = measureKnifeAssemblyIntrinsicMetrics(knifeSceneProgramFixture, compiled)
const repeated = measureKnifeAssemblyIntrinsicMetrics({ program: knifeSceneProgramFixture, compiled })

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message)
}

function finite(value: unknown, path = 'metrics'): void {
  if (typeof value === 'number') {
    assert(Number.isFinite(value), `${path} is not finite`)
    return
  }
  if (Array.isArray(value)) {
    value.forEach((child, index) => finite(child, `${path}[${index}]`))
    return
  }
  if (value && typeof value === 'object') {
    Object.entries(value).forEach(([key, child]) => finite(child, `${path}.${key}`))
  }
}

function bounded(value: unknown, label: string): asserts value is number {
  assert(typeof value === 'number' && Number.isFinite(value) && value >= 0 && value <= 1, `${label} is not bounded`)
}

function expectRejected(label: string, action: () => unknown): void {
  try {
    action()
  } catch {
    return
  }
  throw new Error(`${label} was accepted`)
}

assert(metrics.schema_version === KNIFE_ASSEMBLY_INTRINSIC_METRICS_SCHEMA, 'schema drifted')
assert(metrics.status === KNIFE_ASSEMBLY_INTRINSIC_METRICS_STATUS, 'status drifted')
assert(metrics.renderer_invoked === false && metrics.quality_status === 'NOT_RUN', 'metric crossed render/quality boundary')
assert(metrics.visual_quality_status === 'NOT_COMPUTABLE', 'metric made a visual-quality claim')
assert(metrics.classification === 'design-prior' && metrics.interpretation === 'structural-proxy-not-visual-quality', 'design-prior boundary drifted')
assert(metrics.ratios.guard_root.value !== 'NOT_COMPUTABLE', 'guard/root ratio is not computable')
assert(metrics.ratios.grip_blade.value !== 'NOT_COMPUTABLE', 'grip/blade ratio is not computable')
assert(metrics.ratios.pommel_grip.value !== 'NOT_COMPUTABLE', 'pommel/grip ratio is not computable')
for (const ratio of Object.values(metrics.ratios)) {
  if (ratio.value !== 'NOT_COMPUTABLE') assert(Number.isFinite(ratio.value), `${ratio.ratio_id} is not finite`)
  if (ratio.prior_score !== 'NOT_COMPUTABLE') bounded(ratio.prior_score, `${ratio.ratio_id}.prior_score`)
}
assert(metrics.attachments.map((attachment) => attachment.relation).join('|') === 'blade-root-guard|guard-grip|grip-pommel', 'attachment chain is incomplete or unstable')
for (const attachment of metrics.attachments) {
  assert(attachment.intervals.length === 3, `${attachment.relation} has an open axis interval set`)
  assert(Number.isFinite(attachment.bbox_gap) && Number.isFinite(attachment.bbox_overlap), `${attachment.relation} gap/overlap is not finite`)
  assert(Number.isFinite(attachment.normalized_gap) && attachment.normalized_gap >= 0, `${attachment.relation}.normalized_gap is invalid`)
  bounded(attachment.bbox_overlap_fraction, `${attachment.relation}.bbox_overlap_fraction`)
  bounded(attachment.bbox_continuity, `${attachment.relation}.bbox_continuity`)
}
assert(metrics.material_zone_adjacency.zone_count === 4, 'material-zone count drifted')
assert(metrics.material_zone_adjacency.entries.length > 0, 'material-zone adjacency proxy is empty')
assert(metrics.material_zone_adjacency.mean_readability !== 'NOT_COMPUTABLE', 'material-zone readability is not computable')
bounded(metrics.material_zone_adjacency.mean_readability, 'material_zone_adjacency.mean_readability')
bounded(metrics.material_zone_adjacency.adjacency_coverage, 'material_zone_adjacency.adjacency_coverage')
for (const entry of metrics.material_zone_adjacency.entries) bounded(entry.readability, `${entry.adjacency_id}.readability`)
assert(metrics.complexity.draw_calls === compiled.parts.length, 'draw-call count was not recomputed from compiled parts')
assert(metrics.complexity.triangles === compiled.triangle_count, 'triangle count was not recomputed from compiled geometry')
assert(metrics.complexity.max_draw_calls === knifeSceneProgramFixture.budgets.max_draw_calls, 'draw-call budget was not bound to program')
assert(metrics.complexity.max_triangles === knifeSceneProgramFixture.budgets.max_triangles, 'triangle budget was not bound to program')
bounded(metrics.complexity.draw_call_budget_fraction, 'draw_call_budget_fraction')
bounded(metrics.complexity.triangle_budget_fraction, 'triangle_budget_fraction')
bounded(metrics.complexity.draw_call_efficiency, 'draw_call_efficiency')
bounded(metrics.complexity.triangle_efficiency, 'triangle_efficiency')
bounded(metrics.complexity.complexity_efficiency, 'complexity_efficiency')
assert(metrics.deterministic_fingerprint === repeated.deterministic_fingerprint, 'repeated metric fingerprints differ')
assert(sourceBefore === JSON.stringify(knifeSceneProgramFixture), 'program input was mutated')
assert(Object.isFrozen(metrics) && Object.isFrozen(metrics.ratios) && Object.isFrozen(metrics.attachments), 'metric result is not immutable')
finite(metrics)

expectRejected('unknown input field', () => measureKnifeAssemblyIntrinsicMetrics({ program: knifeSceneProgramFixture, compiled, extra: true } as never))
expectRejected('invalid source fingerprint', () => measureKnifeAssemblyIntrinsicMetrics(knifeSceneProgramFixture, { ...compiled, deterministic_fingerprint: 'not-a-fingerprint' } as never))

console.log(JSON.stringify({
  schema_version: metrics.schema_version,
  asset_id: metrics.asset_id,
  axes: metrics.axes,
  ratios: Object.fromEntries(Object.entries(metrics.ratios).map(([key, ratio]) => [key, ratio.value])),
  attachment_continuity: metrics.attachments.map((attachment) => [attachment.relation, attachment.bbox_continuity]),
  material_zone_adjacency: {
    zone_count: metrics.material_zone_adjacency.zone_count,
    adjacent_zone_pair_count: metrics.material_zone_adjacency.adjacent_zone_pair_count,
    mean_readability: metrics.material_zone_adjacency.mean_readability,
  },
  complexity: {
    draw_calls: metrics.complexity.draw_calls,
    triangles: metrics.complexity.triangles,
    draw_call_efficiency: metrics.complexity.draw_call_efficiency,
    triangle_efficiency: metrics.complexity.triangle_efficiency,
  },
  readability_proxy: metrics.readability_proxy,
  deterministic_fingerprint: metrics.deterministic_fingerprint,
  status: metrics.status,
}))
