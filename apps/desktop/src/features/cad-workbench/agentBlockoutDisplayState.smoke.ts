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
  assert(state.glbKind === 'compiled_agent_pbr', 'generated build must carry its strict compiled PBR display contract')

  state = agentBlockoutDisplayReducer(state, { type: 'preview_started', projectId: 'project-a', requestId: 3, directionId: 'direction_f009', variationIndex: 1 })
  assert(state.glbBase64 === null && state.glbKind === null && state.shapeProgram === null && state.segmentation === null, 'direction reselect must clear the previous candidate')
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
  state = agentBlockoutDisplayReducer(state, {
    type: 'hydrate', projectId: 'project-b', requestId: 4, glbBase64: 'reference-glb', glbKind: 'external_reference',
    shapeProgram: null, segmentation,
  })
  assert(state.glbKind === 'external_reference', 'external reference display provenance must travel with its GLB')
  state = agentBlockoutDisplayReducer(state, { type: 'preview_started', projectId: 'project-b', requestId: 5, directionId: 'direction_f009', variationIndex: 0 })
  state = agentBlockoutDisplayReducer(state, {
    type: 'build_received', projectId: 'project-b', requestId: 5,
    glbBase64: 'new-generated-glb', shapeProgram: { schema_version: 'ShapeProgram@1' },
  })
  const afterLateExternalGlb = agentBlockoutDisplayReducer(state, {
    type: 'set_glb', projectId: 'project-b', requestId: 4, glbBase64: 'late-reference-glb', glbKind: 'external_reference',
  })
  assert(afterLateExternalGlb === state, 'a late external export must not overwrite a newer generated candidate display')
  assert(state.glbBase64 === 'new-generated-glb' && state.glbKind === 'compiled_agent_pbr', 'generated GLB and provenance must remain atomic after a stale external export')
  state = agentBlockoutDisplayReducer(state, {
    type: 'set_shape_program',
    projectId: 'project-b',
    requestId: 6,
    shapeProgram: { schema_version: 'ShapeProgram@1', operations: [{ operation_id: 'op_material_preview' }] },
  })
  assert(
    state.glbBase64 === null && state.glbKind === null && state.shapeProgram !== null && state.latestRequestId === 6,
    'a ChangeSet ShapeProgram preview must clear the previous compiled GLB instead of displaying stale material truth',
  )
  state = agentBlockoutDisplayReducer(state, {
    type: 'set_glb',
    projectId: 'project-b',
    requestId: 6,
    glbBase64: 'preview-pbr-glb',
    glbKind: 'compiled_agent_pbr',
  })
  assert(
    state.glbBase64 === 'preview-pbr-glb' && state.glbKind === 'compiled_agent_pbr',
    'the same ChangeSet preview request must accept its compiled PBR GLB readback',
  )
  state = agentBlockoutDisplayReducer(state, {
    type: 'set_shape_program',
    projectId: 'project-b',
    requestId: 7,
    shapeProgram: { schema_version: 'ShapeProgram@1', operations: [{ operation_id: 'op_newer_preview' }] },
  })
  const afterLatePreviewGlb = agentBlockoutDisplayReducer(state, {
    type: 'set_glb',
    projectId: 'project-b',
    requestId: 6,
    glbBase64: 'late-preview-pbr-glb',
    glbKind: 'compiled_agent_pbr',
  })
  assert(afterLatePreviewGlb === state, 'a late compiled GLB from an older ChangeSet preview must be ignored')
  assert(
    state.latestRequestId === 7
    && state.glbBase64 === null
    && (state.shapeProgram?.operations as Array<{ operation_id?: string }> | undefined)?.[0]?.operation_id === 'op_newer_preview',
    'the newer preview ShapeProgram must remain visible while its own compiled GLB is pending',
  )
  state = agentBlockoutDisplayReducer(state, { type: 'clear', projectId: 'project-b' })
  assert(state.directionId === null && state.variationIndex === 0 && state.previewError === null && state.glbKind === null, 'clearing a candidate must also discard stale preview rotation context')
  assert(!('asset_version_id' in state) && !('snapshot' in state) && !('change_set_id' in state), 'blockout display state must not own persistent design truth')
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
