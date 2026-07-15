import { useCallback, useReducer, useRef } from 'react'
import type { ModuleAssetRecord } from '../../shared/types.js'
import { componentCatalogPresentationReducer, initialComponentCatalogPresentationState, type ComponentCatalogContext } from './componentCatalogPresentationState.js'

export function useComponentCatalogPresentation() {
  const [componentCatalogPresentation, dispatch] = useReducer(componentCatalogPresentationReducer, initialComponentCatalogPresentationState)
  const contextRef = useRef<ComponentCatalogContext>({ projectId: null, packId: null, source: 'none' })
  const requestRef = useRef(0)
  const next = useCallback(() => ++requestRef.current, [])
  const openComponentCatalog = useCallback((context: ComponentCatalogContext) => { if (same(contextRef.current, context)) return; contextRef.current = context; dispatch({ type: 'open', context, requestId: next() }) }, [next])
  const startComponentCatalogRead = useCallback((context: ComponentCatalogContext) => { if (!same(contextRef.current, context)) return null; const requestId = next(); dispatch({ type: 'start', context, requestId }); return requestId }, [next])
  const receiveComponentCatalog = useCallback((context: ComponentCatalogContext, requestId: number, modules: ModuleAssetRecord[]) => { if (!same(contextRef.current, context) || requestRef.current !== requestId) return false; dispatch({ type: 'receive', context, requestId, modules }); return true }, [])
  const failComponentCatalog = useCallback((context: ComponentCatalogContext, requestId: number) => { if (!same(contextRef.current, context) || requestRef.current !== requestId) return false; dispatch({ type: 'fail', context, requestId }); return true }, [])
  return { componentCatalogPresentation, openComponentCatalog, startComponentCatalogRead, receiveComponentCatalog, failComponentCatalog }
}
function same(left: ComponentCatalogContext, right: ComponentCatalogContext) { return left.projectId === right.projectId && left.packId === right.packId && left.source === right.source }
