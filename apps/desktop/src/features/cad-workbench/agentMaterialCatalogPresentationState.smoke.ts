import type { AgentMaterialPreset } from '../../shared/types.js'
import {
  agentMaterialCatalogPresentationReducer,
  initialAgentMaterialCatalogPresentationState,
  type AgentMaterialCatalogContext,
} from './agentMaterialCatalogPresentationState.js'

const robotContext: AgentMaterialCatalogContext = {
  projectId: 'project-a', assetVersionId: 'asset-a', domainPackId: 'pack_robotic_arm_concept', source: 'agent_asset',
}
const vehicleContext: AgentMaterialCatalogContext = {
  projectId: 'project-a', assetVersionId: 'asset-b', domainPackId: 'pack_vehicle_concept', source: 'agent_asset',
}
const preset = (id: string): AgentMaterialPreset => ({
  material_id: id, display_name: id, category: 'metal', pbr: { base_color: '#000000', metallic: 0.5, roughness: 0.5, opacity: 1 },
  visual_only: true, allowed_domains: ['robotic_arm_concept'], provenance: 'forgecad_builtin',
})

export function runAgentMaterialCatalogPresentationStateSmoke(): void {
  let state = agentMaterialCatalogPresentationReducer(initialAgentMaterialCatalogPresentationState, {
    type: 'open_context', context: robotContext, requestId: 1,
  })
  state = agentMaterialCatalogPresentationReducer(state, { type: 'read_started', context: robotContext, requestId: 2 })
  state = agentMaterialCatalogPresentationReducer(state, {
    type: 'read_received', context: robotContext, requestId: 2, materialPresets: [preset('robot-metal')],
  })
  assert(state.materialPresets.length === 1 && state.catalogMessage === null && !state.loading, 'current context must accept its catalog result')

  state = agentMaterialCatalogPresentationReducer(state, { type: 'open_context', context: vehicleContext, requestId: 3 })
  assert(state.materialPresets.length === 0 && state.catalogMessage === null, 'project, asset, domain, or source changes must clear old catalog results')
  const late = agentMaterialCatalogPresentationReducer(state, {
    type: 'read_received', context: robotContext, requestId: 2, materialPresets: [preset('late')],
  })
  assert(late === state, 'late prior-context success must not overwrite the current catalog')

  state = agentMaterialCatalogPresentationReducer(state, { type: 'read_started', context: vehicleContext, requestId: 4 })
  state = agentMaterialCatalogPresentationReducer(state, {
    type: 'read_failed', context: vehicleContext, requestId: 4, fallbackPresets: [preset('builtin-metal')],
  })
  assert(
    state.materialPresets[0]?.material_id === 'builtin-metal'
      && state.catalogMessage === '服务目录暂时无法读取，正在使用本机内置视觉预设。',
    'failed reads may expose only the supplied factual builtin fallback',
  )
  const unavailable = agentMaterialCatalogPresentationReducer(state, {
    type: 'read_failed', context: robotContext, requestId: 4, fallbackPresets: [],
  })
  assert(unavailable === state, 'late failures must not overwrite the current catalog context')
  const fields = Object.keys(state).sort().join(',')
  assert(
    fields === 'assetVersionId,catalogMessage,domainPackId,latestRequestId,loading,materialPresets,projectId,source',
    'material catalog state must not contain Snapshot, selection, quality, ChangeSet, export, asset head, or renderer truth',
  )
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
