import { knifeSceneProgramFixture } from '../fixtures/knife-scene-program.fixture.ts'
import { compileKnifeScene } from '../src/knife-scene-compiler.ts'
import { createKnifeViewRig } from '../src/knife-view-evaluation.ts'
import {
  KNIFE_GUARD_NEGATIVE_SPACE_BASIS,
  KNIFE_GUARD_NEGATIVE_SPACE_INTERPRETATION,
  KNIFE_GUARD_FPS_METRICS_SCHEMA,
  measureKnifeGuardFpsMetrics,
} from '../src/knife-guard-fps-metrics.ts'

const compiled = compileKnifeScene(knifeSceneProgramFixture, { longitudinal_segments: 16 })
const rig = createKnifeViewRig({ frame_width: 64, frame_height: 48 })
const metrics = measureKnifeGuardFpsMetrics(compiled, rig)
const repeated = measureKnifeGuardFpsMetrics({ compiled, rig })

if (metrics.schema_version !== KNIFE_GUARD_FPS_METRICS_SCHEMA
  || metrics.source_fingerprint !== compiled.deterministic_fingerprint
  || metrics.rig_fingerprint !== rig.deterministic_fingerprint
  || metrics.status !== 'MEASURED_NOT_REVIEWED'
  || metrics.renderer_invoked
  || metrics.quality_status !== 'NOT_RUN') {
  throw new Error('guard/FPS metrics crossed the renderer, quality, or lineage boundary')
}
if (metrics.guard_part_id !== 'guard'
  || metrics.guard_negative_space.basis !== KNIFE_GUARD_NEGATIVE_SPACE_BASIS
  || metrics.guard_negative_space.interpretation !== KNIFE_GUARD_NEGATIVE_SPACE_INTERPRETATION
  || metrics.guard_negative_space.is_visible_opening_proxy !== true
  || metrics.guard_negative_space.views.length !== 8) {
  throw new Error('guard negative-space proxy is not closed or not bound to the fixed rig')
}
if (metrics.fps_hold.view_id !== 'FPS_HOLD'
  || metrics.fps_hold.asset_bbox === 'NOT_COMPUTABLE'
  || typeof metrics.fps_hold.asset_bbox_width_fraction !== 'number'
  || typeof metrics.fps_hold.asset_bbox_height_fraction !== 'number'
  || typeof metrics.fps_hold.tip_safe_margin_px !== 'number'
  || typeof metrics.fps_hold.guard_projected_pixel_count !== 'number'
  || typeof metrics.fps_hold.guard_visible_pixel_count !== 'number'
  || typeof metrics.fps_hold.guard_occluded_by_other_part_pixel_count !== 'number'
  || metrics.fps_hold.guard_projected_pixel_count
    !== metrics.fps_hold.guard_visible_pixel_count + metrics.fps_hold.guard_occluded_by_other_part_pixel_count) {
  throw new Error('FPS_HOLD structural metrics are not computable or do not close')
}
if (metrics.deterministic_fingerprint !== repeated.deterministic_fingerprint
  || !Object.isFrozen(metrics)
  || !Object.isFrozen(metrics.guard_negative_space)
  || !Object.isFrozen(metrics.guard_negative_space.views)
  || !Object.isFrozen(metrics.fps_hold)) {
  throw new Error('guard/FPS metrics fingerprint or immutability is not deterministic')
}

function expectRejected(label: string, action: () => unknown): void {
  try {
    action()
  } catch {
    return
  }
  throw new Error(`${label} was accepted`)
}

expectRejected('unknown input field', () => measureKnifeGuardFpsMetrics({ compiled, rig, extra: true } as never))
expectRejected('invalid source fingerprint', () => measureKnifeGuardFpsMetrics({ ...compiled, deterministic_fingerprint: 'not-a-fingerprint' } as never, rig))
expectRejected('invalid rig shape', () => measureKnifeGuardFpsMetrics(compiled, { ...rig, extra: true } as never))
expectRejected('array input envelope', () => measureKnifeGuardFpsMetrics(Object.assign([], { compiled, rig }) as never))

console.log(JSON.stringify({
  schema_version: metrics.schema_version,
  source_fingerprint: metrics.source_fingerprint,
  rig_fingerprint: metrics.rig_fingerprint,
  guard_part_id: metrics.guard_part_id,
  negative_space_views: metrics.guard_negative_space.views.map((view) => ({
    view_id: view.view_id,
    guard_negative_space_ratio: view.guard_negative_space_ratio,
    computability: view.computability,
  })),
  fps_hold: {
    asset_bbox_width_fraction: metrics.fps_hold.asset_bbox_width_fraction,
    asset_bbox_height_fraction: metrics.fps_hold.asset_bbox_height_fraction,
    tip_safe_margin_px: metrics.fps_hold.tip_safe_margin_px,
    guard_visible_pixel_count: metrics.fps_hold.guard_visible_pixel_count,
    guard_occluded_by_other_part_pixel_count: metrics.fps_hold.guard_occluded_by_other_part_pixel_count,
    computability: metrics.fps_hold.computability,
  },
  deterministic_fingerprint: metrics.deterministic_fingerprint,
  status: metrics.status,
}))
