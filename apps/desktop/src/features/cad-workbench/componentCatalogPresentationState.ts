import type { ModuleAssetRecord } from '../../shared/types.js'

export type ComponentCatalogContext = { projectId: string | null; packId: string | null; source: 'legacy' | 'agent_asset' | 'none' }
export type ComponentCatalogPresentationState = ComponentCatalogContext & { modules: ModuleAssetRecord[]; loading: boolean; message: string | null; requestId: number }
export const initialComponentCatalogPresentationState: ComponentCatalogPresentationState = { projectId: null, packId: null, source: 'none', modules: [], loading: false, message: null, requestId: 0 }
export type ComponentCatalogPresentationAction =
  | { type: 'open'; context: ComponentCatalogContext; requestId: number }
  | { type: 'start'; context: ComponentCatalogContext; requestId: number }
  | { type: 'receive'; context: ComponentCatalogContext; requestId: number; modules: ModuleAssetRecord[] }
  | { type: 'fail'; context: ComponentCatalogContext; requestId: number }

export function componentCatalogPresentationReducer(state: ComponentCatalogPresentationState, action: ComponentCatalogPresentationAction): ComponentCatalogPresentationState {
  if (action.type === 'open') return matches(state, action.context) ? state : { ...initialComponentCatalogPresentationState, ...action.context, requestId: action.requestId }
  if (!matches(state, action.context) || action.requestId !== state.requestId && action.type !== 'start') return state
  if (action.type === 'start') return action.requestId > state.requestId ? { ...state, loading: true, message: null, requestId: action.requestId } : state
  if (action.type === 'receive') return { ...state, modules: action.modules, loading: false, message: action.modules.length ? null : '当前目录没有可用组件。' }
  return { ...state, modules: [], loading: false, message: '组件目录暂时无法读取。' }
}
function matches(state: ComponentCatalogPresentationState, context: ComponentCatalogContext) { return state.projectId === context.projectId && state.packId === context.packId && state.source === context.source }
