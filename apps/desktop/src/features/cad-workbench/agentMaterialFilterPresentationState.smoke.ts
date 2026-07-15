import {
  agentMaterialFilterPresentationReducer,
  initialAgentMaterialFilterPresentationState,
  type AgentMaterialFilterContext,
} from './agentMaterialFilterPresentationState.js'

const robotContext: AgentMaterialFilterContext = {
  projectId: 'project-a', domainPackId: 'pack_robotic_arm_concept', source: 'agent_asset',
}
const externalContext: AgentMaterialFilterContext = {
  projectId: 'project-a', domainPackId: null, source: 'external_glb',
}

export function runAgentMaterialFilterPresentationStateSmoke(): void {
  let state = agentMaterialFilterPresentationReducer(initialAgentMaterialFilterPresentationState, {
    type: 'open_context', context: robotContext,
  })
  state = agentMaterialFilterPresentationReducer(state, { type: 'set_query', query: '金属' })
  state = agentMaterialFilterPresentationReducer(state, { type: 'set_category', category: 'metal' })
  state = agentMaterialFilterPresentationReducer(state, { type: 'set_compatibility_only', compatibilityOnly: false })
  assert(
    state.query === '金属' && state.category === 'metal' && !state.compatibilityOnly,
    'keyword, category, and compatibility controls must compose in the current context',
  )
  state = agentMaterialFilterPresentationReducer(state, { type: 'open_context', context: externalContext })
  assert(
    state.query === '' && state.category === 'all' && state.compatibilityOnly && state.source === 'external_glb',
    'source or domain context changes must clear transient filters without selecting or rewriting a material zone',
  )
  const unchanged = agentMaterialFilterPresentationReducer(state, { type: 'open_context', context: externalContext })
  assert(unchanged === state, 'opening the same context must preserve the current filter display state')
  assert(
    Object.keys(state).sort().join(',') === 'category,compatibilityOnly,domainPackId,projectId,query,source',
    'filter state must not contain selected material, Material Zone, Snapshot, version, quality, ChangeSet, export, or renderer truth',
  )
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
