import { componentCatalogPresentationReducer, initialComponentCatalogPresentationState, type ComponentCatalogContext } from './componentCatalogPresentationState.js'
const legacy: ComponentCatalogContext = { projectId: 'p1', packId: 'pack_future_weapon_prop', source: 'legacy' }
const agent: ComponentCatalogContext = { projectId: 'p1', packId: 'pack_robotic_arm_concept', source: 'agent_asset' }
export function runComponentCatalogPresentationStateSmoke(): void {
  let state = componentCatalogPresentationReducer(initialComponentCatalogPresentationState, { type: 'open', context: legacy, requestId: 1 })
  state = componentCatalogPresentationReducer(state, { type: 'start', context: legacy, requestId: 2 })
  state = componentCatalogPresentationReducer(state, { type: 'receive', context: legacy, requestId: 2, modules: [{ manifest: { module_id: 'm1' } } as never] })
  state = componentCatalogPresentationReducer(state, { type: 'open', context: agent, requestId: 3 })
  if (state.modules.length || state.message || state.source !== 'agent_asset') throw new Error('context switch must clear old catalog')
  const late = componentCatalogPresentationReducer(state, { type: 'receive', context: legacy, requestId: 2, modules: [{ manifest: { module_id: 'late' } } as never] })
  if (late !== state) throw new Error('late catalog response overwrote current context')
  if (Object.keys(state).sort().join(',') !== 'loading,message,modules,packId,projectId,requestId,source') throw new Error('catalog state owns forbidden truth')
}
