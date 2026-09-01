import {
  RELIEF_CURVE_GRAPH_SCHEMA_VERSION,
  compileReliefCurveGraph,
  fingerprintReliefCurveGraph,
  ReliefCurveGraphError,
  type ReliefCurveGraph,
} from '../src/index.ts'

const graph: ReliefCurveGraph = {
  schema_version: RELIEF_CURVE_GRAPH_SCHEMA_VERSION,
  graph_id: 'smoke-dragon-relief',
  part_id: 'relief-dragon-curve',
  material_zone_id: 'ornament-gold',
  local_frame: {
    origin: [0, 0, 0],
    tangent: [1, 0, 0],
    lateral: [0, 1, 0],
    normal: [0, 0, 1],
  },
  paths: [
    {
      path_id: 'crest-2d',
      dimension: '2d',
      basis: 'bezier',
      points: [
        [-0.42, 0.02],
        [-0.2, 0.18],
        [0.1, 0.13],
        [0.42, 0.02],
      ],
    },
    {
      path_id: 'eye-3d-loop',
      dimension: '3d',
      basis: 'nurbs-like',
      closed: true,
      points: [
        [0.08, 0.01, 0.015],
        [0.2, 0.11, 0.02],
        [0.34, 0.02, 0.01],
        [0.2, -0.08, 0.018],
      ],
    },
  ],
  width: 0.08,
  depth: 0.028,
  taper: 0.2,
  profile: 'bevel',
}

const options = {
  samples_per_path: 24,
  round_radial_segments: 12,
  max_triangles: 4096,
} as const

const first = compileReliefCurveGraph(graph, options)
const second = compileReliefCurveGraph(graph, options)
const position = first.geometry.getAttribute('position')
const normal = first.geometry.getAttribute('normal')
const index = first.geometry.getIndex()

if (first.deterministic_fingerprint !== second.deterministic_fingerprint
  || first.geometry.uuid !== second.geometry.uuid
  || first.deterministic_fingerprint !== fingerprintReliefCurveGraph(graph, options)) {
  throw new Error('relief curve compilation is not deterministic')
}
if (first.path_ids.join(',') !== 'crest-2d,eye-3d-loop' || first.radial_segments !== 8) {
  throw new Error('relief curve path/profile receipt drifted')
}
if (first.triangle_count <= 0 || first.vertex_count <= 0 || !index || index.count !== first.triangle_count * 3) {
  throw new Error('relief curve did not produce indexed volume geometry')
}
if (position.count !== first.vertex_count || normal.count !== first.vertex_count
  || first.geometry.getAttribute('uv').count !== first.vertex_count
  || first.geometry.getAttribute('partIdHash').count !== first.vertex_count
  || first.geometry.getAttribute('materialZoneHash').count !== first.vertex_count
  || first.geometry.getAttribute('reliefPathIndex').count !== first.vertex_count) {
  throw new Error('relief curve semantic attributes are incomplete')
}
if (!first.bounds.min || !first.bounds.max || first.bounds.max[2] - first.bounds.min[2] <= graph.depth * 0.5) {
  throw new Error('relief curve did not produce positive normal volume')
}
if (first.geometry.groups.length !== graph.paths.length) throw new Error('relief curve path groups are not stable')
if (first.geometry.userData.part_id !== graph.part_id
  || first.geometry.userData.material_zone_id !== graph.material_zone_id
  || first.geometry.userData.renderer_invoked !== false
  || first.geometry.userData.quality_status !== 'NOT_RUN') {
  throw new Error('relief curve lineage/status metadata drifted')
}
for (const value of position.array) {
  if (!Number.isFinite(value)) throw new Error('relief curve emitted a non-finite position')
}
for (const profile of ['round', 'bevel', 'flat'] as const) {
  const profiled = compileReliefCurveGraph({ ...graph, graph_id: `smoke-${profile}`, profile }, options)
  if (profiled.triangle_count <= 0 || profiled.bounds.max[2] <= profiled.bounds.min[2]) {
    throw new Error(`${profile} relief profile did not produce volume`)
  }
}

const invalidExtraField = { ...graph, arbitrary_code: 'not-accepted' } as ReliefCurveGraph & { arbitrary_code: string }
try {
  compileReliefCurveGraph(invalidExtraField)
  throw new Error('unknown ReliefCurveGraph field was accepted')
} catch (error) {
  if (!(error instanceof ReliefCurveGraphError) || error.code !== 'INVALID_INPUT') throw error
}

try {
  compileReliefCurveGraph(graph, { ...options, max_triangles: 1 })
  throw new Error('relief curve triangle budget was not enforced')
} catch (error) {
  if (!(error instanceof ReliefCurveGraphError) || error.code !== 'BUDGET_EXCEEDED') throw error
}

console.log(JSON.stringify({
  smoke_status: 'PASS',
  schema_version: first.schema_version,
  algorithm: first.geometry.userData.algorithm,
  deterministic_fingerprint: first.deterministic_fingerprint,
  path_ids: first.path_ids,
  profile: first.profile,
  samples_per_path: first.samples_per_path,
  radial_segments: first.radial_segments,
  vertex_count: first.vertex_count,
  triangle_count: first.triangle_count,
  group_count: first.geometry.groups.length,
  renderer_invoked: first.renderer_invoked,
  quality_status: first.quality_status,
}))
