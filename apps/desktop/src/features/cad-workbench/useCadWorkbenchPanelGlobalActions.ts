import { useMemo } from 'react'

import type { GlobalActionState } from './cadWorkbenchPanelGlobalActions'

type UseCadWorkbenchPanelGlobalActionsInput = {
  canUndo: boolean
  canRedo: boolean
  canImport: boolean
  importingGlb: boolean
}

export function useCadWorkbenchPanelGlobalActions({
  canUndo,
  canRedo,
  canImport,
  importingGlb,
}: UseCadWorkbenchPanelGlobalActionsInput): GlobalActionState {
  return useMemo(() => ({
    canUndo,
    canRedo,
    canImport,
    importingGlb,
    importLabel: importingGlb ? '导入中…' : '导入参考',
  }), [canUndo, canRedo, canImport, importingGlb])
}
