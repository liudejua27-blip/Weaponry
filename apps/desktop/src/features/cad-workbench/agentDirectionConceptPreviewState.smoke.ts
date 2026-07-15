import { agentDirectionConceptPreviewReducer, initialAgentDirectionConceptPreviewState } from './agentDirectionConceptPreviewState.js'

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

const preview = {
  schema_version: 'AgentBlockoutConceptPreview@1' as const,
  plan_id: 'plan_direction_preview',
  direction_id: 'direction_one',
  variant_id: 'vehicle_variant_1',
  variation_index: 0,
  domain_pack_id: 'pack_vehicle_concept' as const,
  topology_hash: 'a'.repeat(64),
  render_context_sha256: 'b'.repeat(64),
  renderer_id: 'forgecad-agent-software-raster@1' as const,
  width: 320 as const,
  height: 240 as const,
  png_base64: 'cG5n',
  sha256: 'c'.repeat(64),
  byte_size: 3,
}

export function runAgentDirectionConceptPreviewStateSmoke(): void {
  let state = agentDirectionConceptPreviewReducer(initialAgentDirectionConceptPreviewState, { type: 'open_project', projectId: 'project-a' })
  state = agentDirectionConceptPreviewReducer(state, {
    type: 'previews_started', projectId: 'project-a', planId: 'plan_direction_preview', requestId: 2, directionIds: ['direction_one', 'direction_two', 'direction_three'],
  })
  assert(Object.values(state.previews).every((entry) => entry.status === 'loading'), 'all three current directions must start as temporary loading cards')
  state = agentDirectionConceptPreviewReducer(state, { type: 'preview_received', projectId: 'project-a', planId: 'plan_direction_preview', requestId: 2, preview })
  assert(state.previews.direction_one?.imageDataUrl === 'data:image/png;base64,cG5n', 'a current response must become a local image card')
  const stale = agentDirectionConceptPreviewReducer(state, { type: 'preview_received', projectId: 'project-a', planId: 'plan_direction_preview', requestId: 1, preview: { ...preview, png_base64: 'c3RhbGU=' } })
  assert(stale.previews.direction_one?.imageDataUrl === state.previews.direction_one?.imageDataUrl, 'late response must not overwrite the current image')
  state = agentDirectionConceptPreviewReducer(state, { type: 'clear', projectId: 'project-a' })
  assert(Object.keys(state.previews).length === 0 && state.planId === null, 'direction or variation changes must discard all temporary images')
  state = agentDirectionConceptPreviewReducer(state, { type: 'open_project', projectId: 'project-b' })
  assert(state.projectId === 'project-b' && Object.keys(state.previews).length === 0, 'project changes must not retain temporary cards')
  assert(!('assetVersionId' in state) && !('snapshot' in state) && !('quality' in state) && !('exportId' in state), 'temporary concept image state must not own durable truth')
}
