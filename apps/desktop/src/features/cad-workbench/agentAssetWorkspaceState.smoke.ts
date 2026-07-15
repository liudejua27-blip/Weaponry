import type { AgentAssetVersion } from '../../shared/types.js'
import { agentAssetWorkspaceReducer, initialAgentAssetWorkspaceState } from './agentAssetWorkspaceState.js'

const asset = (id: string): AgentAssetVersion => ({
  schema_version: 'AgentAssetVersion@1', asset_version_id: id, project_id: 'project-a', version_no: 1,
  parent_asset_version_id: null, status: 'committed', summary: id, stage: 'segmented_concept', plan_id: 'plan', direction_id: 'direction',
  domain_pack_id: 'pack_vehicle_concept', artifact_id: 'artifact', parts: [], shape_program: {}, assembly_graph: { schema_version: 'AssemblyGraph@1', roots: [], parts: [], joints: [] }, created_at: '2026-07-14T00:00:00Z',
})

export function runAgentAssetWorkspaceStateSmoke(): void {
  let state = agentAssetWorkspaceReducer(initialAgentAssetWorkspaceState, { type: 'open_project', projectId: 'project-a' })
  state = agentAssetWorkspaceReducer(state, { type: 'hydrate_started', projectId: 'project-a', requestId: 2, assetVersionId: 'asset-a', selectedPartId: 'part-a' })
  state = agentAssetWorkspaceReducer(state, { type: 'asset_received', projectId: 'project-a', requestId: 2, assetVersion: asset('asset-a') })
  assert(state.assetVersion?.asset_version_id === 'asset-a' && state.selectedPartId === 'part-a', 'current Snapshot hydration must project its asset and selection')
  state = agentAssetWorkspaceReducer(state, { type: 'selection_updated', projectId: 'project-a', assetVersionId: 'asset-a', selectedPartId: 'part-b' })
  assert(state.selectedPartId === 'part-b', 'a current Snapshot selection response must update only the read projection')
  state = agentAssetWorkspaceReducer(state, { type: 'hydrate_started', projectId: 'project-a', requestId: 3, assetVersionId: 'asset-b', selectedPartId: 'part-b' })
  const stale = agentAssetWorkspaceReducer(state, { type: 'asset_received', projectId: 'project-a', requestId: 2, assetVersion: asset('asset-a') })
  assert(stale === state, 'late asset reads must not replace a newer Snapshot projection')
  state = agentAssetWorkspaceReducer(state, { type: 'asset_received', projectId: 'project-a', requestId: 3, assetVersion: asset('asset-b') })
  const staleSelection = agentAssetWorkspaceReducer(state, { type: 'selection_updated', projectId: 'project-a', assetVersionId: 'asset-a', selectedPartId: 'part-a' })
  assert(staleSelection === state, 'a prior asset selection must not overwrite the current Snapshot asset projection')
  state = agentAssetWorkspaceReducer(state, { type: 'quality_received', projectId: 'project-a', requestId: 2, qualityReport: null })
  assert(state.qualityReport === null, 'late quality reads must not alter the current workspace projection')
  state = agentAssetWorkspaceReducer(state, { type: 'navigation_received', projectId: 'project-a', requestId: 2, navigation: null })
  assert(state.navigation === null, 'late navigation reads must not alter the current workspace projection')
  state = agentAssetWorkspaceReducer(state, { type: 'quality_received', projectId: 'project-a', requestId: 3, qualityReport: null })
  state = agentAssetWorkspaceReducer(state, { type: 'open_project', projectId: 'project-b' })
  assert(state.assetVersion === null && state.selectedPartId === null && state.qualityReport === null && state.navigation === null, 'project switch must clear the read projection')
  assert(!('snapshot_revision' in state) && !('asset_version_id' in state), 'workspace cache must not own Snapshot or version-head truth')
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
