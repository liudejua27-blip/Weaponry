import {
  agentMaterialPreselectionPresentationReducer,
  initialAgentMaterialPreselectionPresentationState,
  type AgentMaterialPreselectionContext,
} from './agentMaterialPreselectionPresentationState.js'

const agentContext: AgentMaterialPreselectionContext = {
  projectId: 'project-a', assetVersionId: 'asset-a', selectedPartId: 'part-a', source: 'agent_asset',
}

export function runAgentMaterialPreselectionPresentationStateSmoke(): void {
  let state = agentMaterialPreselectionPresentationReducer(initialAgentMaterialPreselectionPresentationState, {
    type: 'open_context', context: agentContext,
  })
  state = agentMaterialPreselectionPresentationReducer(state, { type: 'select_material', materialId: 'mat_aluminum' })
  assert(state.materialId === 'mat_aluminum', 'current Agent context may retain only a visual material preselection')
  state = agentMaterialPreselectionPresentationReducer(state, {
    type: 'open_context', context: { ...agentContext, selectedPartId: 'part-b' },
  })
  assert(state.materialId === 'mat_graphite', 'part, asset, project, or source changes must clear the visual preselection')
  const external = agentMaterialPreselectionPresentationReducer(state, {
    type: 'open_context', context: { ...agentContext, source: 'external_glb' },
  })
  const blocked = agentMaterialPreselectionPresentationReducer(external, { type: 'select_material', materialId: 'mat_aluminum' })
  assert(blocked === external, 'external GLB and legacy sources must reject local material preselection')
  assert(
    Object.keys(state).sort().join(',') === 'assetVersionId,materialId,projectId,selectedPartId,source',
    'preselection state must not contain Material Zone, Snapshot, version, quality, ChangeSet, export, or renderer truth',
  )
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
