import {
  agentMaterialPreselectionPresentationReducer,
  initialAgentMaterialPreselectionPresentationState,
  resolveAgentMaterialDisplayId,
  type AgentMaterialPreselectionContext,
} from './agentMaterialPreselectionPresentationState.js'

const agentContext: AgentMaterialPreselectionContext = {
  projectId: 'project-a',
  assetVersionId: 'asset-a',
  selectedPartId: 'part-a',
  materialZoneId: 'zone_primary',
  source: 'agent_asset',
}

export function runAgentMaterialPreselectionPresentationStateSmoke(): void {
  let state = agentMaterialPreselectionPresentationReducer(initialAgentMaterialPreselectionPresentationState, {
    type: 'open_context', context: agentContext,
  })
  state = agentMaterialPreselectionPresentationReducer(state, { type: 'select_material', materialId: 'mat_aluminum' })
  assert(state.materialId === 'mat_aluminum' && state.hasSelection, 'current Agent context may retain only an explicit visual material preselection')
  assert(
    resolveAgentMaterialDisplayId(state, agentContext, 'mat_graphite') === 'mat_aluminum',
    'matching context must display the explicit local preselection over the committed binding',
  )
  assert(
    resolveAgentMaterialDisplayId(state, { ...agentContext, materialZoneId: 'zone_trim' }, 'mat_signal_red') === 'mat_signal_red',
    'a new zone must display its committed binding even before the reducer effect clears the old local preselection',
  )
  state = agentMaterialPreselectionPresentationReducer(state, {
    type: 'open_context', context: { ...agentContext, materialZoneId: 'zone_trim' },
  })
  assert(state.materialId === 'mat_graphite' && !state.hasSelection, 'zone, part, asset, project, or source changes must clear the visual preselection')
  state = agentMaterialPreselectionPresentationReducer(state, {
    type: 'open_context', context: { ...agentContext, selectedPartId: 'part-b' },
  })
  assert(!state.hasSelection, 'part changes must preserve no local override so committed bindings can be projected')
  const external = agentMaterialPreselectionPresentationReducer(state, {
    type: 'open_context', context: { ...agentContext, source: 'external_glb' },
  })
  const blocked = agentMaterialPreselectionPresentationReducer(external, { type: 'select_material', materialId: 'mat_aluminum' })
  assert(blocked === external, 'external GLB and legacy sources must reject local material preselection')
  assert(
    Object.keys(state).sort().join(',') === 'assetVersionId,hasSelection,materialId,materialZoneId,projectId,selectedPartId,source',
    'preselection state may retain only a zone reset key, never Snapshot, committed binding, version, quality, ChangeSet, export, or renderer truth',
  )
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
