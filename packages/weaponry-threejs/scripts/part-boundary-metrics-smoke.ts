import { knifeSceneProgramFixture } from '../fixtures/knife-scene-program.fixture.ts'
import { compileKnifeScene } from '../src/knife-scene-compiler.ts'
import { createKnifeViewRig } from '../src/knife-view-evaluation.ts'
import {
  KNIFE_PART_BOUNDARY_METRICS_SCHEMA,
  KNIFE_PART_BOUNDARY_METRICS_STATUS,
  measureKnifePartBoundaryMetrics,
  KnifePartBoundaryMetricsError,
} from '../src/knife-part-boundary-metrics.ts'

const compiled = compileKnifeScene(knifeSceneProgramFixture)
const rig = createKnifeViewRig({ frame_width: 64, frame_height: 48 })
const first = measureKnifePartBoundaryMetrics(compiled, rig)
const second = measureKnifePartBoundaryMetrics({ compiled, rig })

if (first.schema_version !== KNIFE_PART_BOUNDARY_METRICS_SCHEMA
  || first.status !== KNIFE_PART_BOUNDARY_METRICS_STATUS
  || first.renderer_invoked
  || first.quality_status !== 'NOT_RUN'
  || first.connectivity !== 'four-neighbor@1'
  || first.normalization !== 'frame-diagonal-pixels@1') {
  throw new Error('part boundary metrics crossed the renderer or quality boundary')
}
if (first.deterministic_fingerprint !== second.deterministic_fingerprint
  || first.parts.length !== compiled.parts.length
  || first.view_ids.join('|') !== 'FRONT|BACK|TOP|BOTTOM|LEFT|RIGHT|REAR_THREE_QUARTER|FPS_HOLD'
  || first.adjacency_matrix.length !== compiled.parts.length
  || first.adjacency_matrix.some((row) => row.length !== compiled.parts.length)) {
  throw new Error('part boundary metrics are not fixed, deterministic, or square')
}
if (!Object.isFrozen(first)
  || !Object.isFrozen(first.parts)
  || !Object.isFrozen(first.parts[0])
  || !Object.isFrozen(first.parts[0].views)
  || !Object.isFrozen(first.adjacency_matrix)
  || !Object.isFrozen(first.adjacency_matrix[0])) {
  throw new Error('part boundary metrics result is not frozen')
}

for (const part of first.parts) {
  if (part.views.length !== 8
    || part.boundary_pixel_count < 0
    || part.connected_island_count < 0
    || !Number.isFinite(part.boundary_length_normalized)) {
    throw new Error(`invalid boundary summary for ${part.part_id}`)
  }
  for (const view of part.views) {
    if (!Number.isInteger(view.visible_pixel_count)
      || !Number.isInteger(view.boundary_pixel_count)
      || !Number.isInteger(view.boundary_edge_count)
      || !Number.isInteger(view.connected_island_count)
      || view.boundary_pixel_count < 0
      || view.boundary_edge_count < 0
      || view.connected_island_count < 0
      || !Number.isFinite(view.boundary_length_normalized)) {
      throw new Error(`invalid boundary view metric for ${part.part_id}/${view.view_id}`)
    }
  }
}

const bladeRootCell = first.adjacency_matrix[0].find((cell) => cell.relation === 'blade-root-attachment')
if (!bladeRootCell || bladeRootCell.views.length !== 8
  || first.semantic_adjacencies.every((cell) => cell.relation !== 'blade-root-attachment')) {
  throw new Error('semantic blade/root adjacency was not emitted')
}
for (let row = 0; row < first.adjacency_matrix.length; row += 1) {
  for (let column = 0; column < first.adjacency_matrix.length; column += 1) {
    const left = first.adjacency_matrix[row][column]
    const right = first.adjacency_matrix[column][row]
    if (left.contact_pixel_count !== right.contact_pixel_count
      || left.contact_edge_count !== right.contact_edge_count
      || left.gap_pixel_count !== right.gap_pixel_count
      || left.gap_edge_count !== right.gap_edge_count) {
      throw new Error(`adjacency matrix is not symmetric at ${row}/${column}`)
    }
  }
}

function expectRejected(label: string, action: () => unknown): void {
  try {
    action()
  } catch (error) {
    if (!(error instanceof KnifePartBoundaryMetricsError)) throw error
    return
  }
  throw new Error(`${label} was accepted`)
}

expectRejected('unknown input field', () => measureKnifePartBoundaryMetrics({ compiled, rig, extra: true } as never))
expectRejected('invalid source fingerprint', () => measureKnifePartBoundaryMetrics({ ...compiled, deterministic_fingerprint: 'invalid' } as never, rig))

console.log(JSON.stringify({
  smoke_status: 'PASS',
  schema_version: first.schema_version,
  frame: `${first.frame_width}x${first.frame_height}`,
  parts: first.parts.map((part) => ({
    part_id: part.part_id,
    boundary_pixel_count: part.boundary_pixel_count,
    connected_island_count: part.connected_island_count,
  })),
  semantic_adjacency_count: first.semantic_adjacencies.length,
  deterministic_fingerprint: first.deterministic_fingerprint,
  renderer_invoked: first.renderer_invoked,
  quality_status: first.quality_status,
}))
