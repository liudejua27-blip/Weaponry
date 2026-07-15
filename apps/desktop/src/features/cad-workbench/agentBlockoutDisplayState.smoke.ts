import type { SegmentAgentBlockoutResponse } from '../../shared/types.js'
import { agentBlockoutDisplayReducer, initialAgentBlockoutDisplayState } from './agentBlockoutDisplayState.js'

const segmentation: SegmentAgentBlockoutResponse = {
  artifact_id: 'artifact_f009',
  plan_id: 'plan_f009',
  direction_id: 'direction_f009',
  domain_pack_id: 'pack_vehicle_concept',
  segmentation_status: 'candidate',
  parts: [],
  assembly_graph: { schema_version: 'AssemblyGraph@1', roots: [], parts: [], joints: [] },
}

export function runAgentBlockoutDisplayStateSmoke(): void {
  let state = agentBlockoutDisplayReducer(initialAgentBlockoutDisplayState, { type: 'open_project', projectId: 'project-a' })
  state = agentBlockoutDisplayReducer(state, { type: 'preview_started', projectId: 'project-a', requestId: 2, directionId: 'direction_f009', variationIndex: 0 })
  state = agentBlockoutDisplayReducer(state, {
    type: 'build_received', projectId: 'project-a', requestId: 2,
    glbBase64: 'glb-a', shapeProgram: { schema_version: 'ShapeProgram@1' },
  })
  assert(state.directionPreviewLoading && state.glbBase64 === 'glb-a', 'current build must update only the candidate display')

  state = agentBlockoutDisplayReducer(state, { type: 'preview_started', projectId: 'project-a', requestId: 3, directionId: 'direction_f009', variationIndex: 1 })
  assert(state.glbBase64 === null && state.shapeProgram === null && state.segmentation === null, 'direction reselect must clear the previous candidate')
  assert(state.directionId === 'direction_f009' && state.variationIndex === 1 && state.previewError === null, 'appearance rotation must keep only plain preview context')
  const afterLateSegmentation = agentBlockoutDisplayReducer(state, {
    type: 'segmentation_received', projectId: 'project-a', requestId: 2, segmentation,
  })
  assert(afterLateSegmentation === state, 'late segmentation from an earlier direction must be ignored')
  state = agentBlockoutDisplayReducer(state, {
    type: 'build_received', projectId: 'project-a', requestId: 3,
    glbBase64: 'glb-b', shapeProgram: { schema_version: 'ShapeProgram@1', outputs: [] },
  })
  state = agentBlockoutDisplayReducer(state, { type: 'segmentation_failed', projectId: 'project-a', requestId: 3 })
  assert(!state.directionPreviewLoading && state.glbBase64 === 'glb-b' && state.segmentation === null && state.previewError === 'segmentation_failed', 'segmentation failure must preserve the uncommitted visual preview without a part candidate')

  state = agentBlockoutDisplayReducer(state, { type: 'open_project', projectId: 'project-b' })
  assert(state.glbBase64 === null && state.shapeProgram === null && state.segmentation === null, 'project switch must clear candidate display state')
  const afterOldProjectBuild = agentBlockoutDisplayReducer(state, {
    type: 'build_received', projectId: 'project-a', requestId: 3,
    glbBase64: 'stale', shapeProgram: { schema_version: 'ShapeProgram@1' },
  })
  assert(afterOldProjectBuild === state, 'old project build must never pollute the next project')
  state = agentBlockoutDisplayReducer(state, { type: 'clear', projectId: 'project-b' })
  assert(state.directionId === null && state.variationIndex === 0 && state.previewError === null, 'clearing a candidate must also discard stale preview rotation context')
  assert(!('asset_version_id' in state) && !('snapshot' in state) && !('change_set_id' in state), 'blockout display state must not own persistent design truth')
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
