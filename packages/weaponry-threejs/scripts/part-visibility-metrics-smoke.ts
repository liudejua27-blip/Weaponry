import { knifeSceneProgramFixture } from '../fixtures/knife-scene-program.fixture.ts'
import {
  compileKnifeScene,
  createKnifeViewRig,
  evaluateKnifeRig,
  measureKnifePartVisibilityMetrics,
} from '../src/index.ts'

const compiled = compileKnifeScene(knifeSceneProgramFixture)
const rig = createKnifeViewRig({ frame_width: 64, frame_height: 48 })
const evaluation = evaluateKnifeRig(compiled, rig)
const metrics = measureKnifePartVisibilityMetrics(compiled, rig)
const repeated = measureKnifePartVisibilityMetrics({ compiled, rig })

if (metrics.schema_version !== 'KnifePartVisibilityMetrics@1'
  || metrics.status !== 'MEASURED_NOT_REVIEWED'
  || metrics.renderer_invoked
  || metrics.quality_status !== 'NOT_RUN') {
  throw new Error('visibility metrics crossed the renderer or quality boundary')
}
if (metrics.parts.length !== compiled.parts.length
  || metrics.parts.map((part) => part.part_id).join(',') !== compiled.parts.map((part) => part.part_id).join(',')) {
  throw new Error('visibility metrics did not preserve all parts in canonical order')
}
if (metrics.deterministic_fingerprint !== repeated.deterministic_fingerprint) {
  throw new Error('visibility metrics fingerprint is not deterministic')
}
if (!Object.isFrozen(metrics)
  || !Object.isFrozen(metrics.parts)
  || !Object.isFrozen(metrics.parts[0])
  || !Object.isFrozen(metrics.parts[0].views)) {
  throw new Error('visibility metrics result is not frozen')
}

const compiledPartIndex = new Map(compiled.parts.map((part, index) => [part.part_id, index]))
for (const part of metrics.parts) {
  const partIndex = compiledPartIndex.get(part.part_id)
  if (partIndex === undefined || part.triangle_count <= 0 || part.views.length !== 8) {
    throw new Error(`invalid per-part visibility metric for ${part.part_id}`)
  }
  const coverages = part.views.map((view) => view.coverage_ratio)
  const expectedMin = Math.min(...coverages)
  const expectedMax = Math.max(...coverages)
  const expectedMean = coverages.reduce((sum, value) => sum + value, 0) / coverages.length
  if (Math.abs(part.min_coverage_ratio - expectedMin) > 1e-15
    || Math.abs(part.max_coverage_ratio - expectedMax) > 1e-15
    || Math.abs(part.mean_coverage_ratio - expectedMean) > 1e-15) {
    throw new Error(`coverage summary mismatch for ${part.part_id}`)
  }

  for (const viewMetric of part.views) {
    const view = evaluation.views.find((candidate) => candidate.view_id === viewMetric.view_id)
    if (!view) throw new Error(`missing evaluation view ${viewMetric.view_id}`)
    let expectedVisiblePixels = 0
    for (let pixelIndex = 0; pixelIndex < view.mask.part_indices.length; pixelIndex += 1) {
      if (view.mask.pixels[pixelIndex] !== 0 && view.mask.part_indices[pixelIndex] === partIndex) expectedVisiblePixels += 1
    }
    const framePixelCount = view.mask.width * view.mask.height
    const expectedCoverage = expectedVisiblePixels / framePixelCount
    const expectedOcclusionShare = view.mask.receipt.covered_pixel_count === 0
      ? 0
      : expectedVisiblePixels / view.mask.receipt.covered_pixel_count
    if (viewMetric.visible_pixel_count !== expectedVisiblePixels
      || Math.abs(viewMetric.coverage_ratio - expectedCoverage) > 1e-15
      || Math.abs(viewMetric.occlusion_share - expectedOcclusionShare) > 1e-15) {
      throw new Error(`part_indices attribution mismatch for ${part.part_id}/${viewMetric.view_id}`)
    }
  }
}

function expectRejected(label: string, action: () => unknown): void {
  try {
    action()
  } catch {
    return
  }
  throw new Error(`${label} was accepted`)
}

expectRejected('unknown input field', () => measureKnifePartVisibilityMetrics({ compiled, rig, extra: true } as never))
expectRejected('invalid scene', () => measureKnifePartVisibilityMetrics({ ...compiled, deterministic_fingerprint: 'not-a-fingerprint' } as never, rig))

console.log(JSON.stringify({
  schema_version: metrics.schema_version,
  frame: `${metrics.frame_width}x${metrics.frame_height}`,
  parts: metrics.parts.map((part) => ({
    part_id: part.part_id,
    triangle_count: part.triangle_count,
    visible_view_count: part.visible_view_count,
    front_presence: part.front_presence,
    top_presence: part.top_presence,
    side_presence: part.side_presence,
    fps_presence: part.fps_presence,
  })),
  missing_part_ids: metrics.missing_part_ids,
  underexposed_part_ids: metrics.underexposed_part_ids,
  deterministic_fingerprint: metrics.deterministic_fingerprint,
  status: metrics.status,
}))
