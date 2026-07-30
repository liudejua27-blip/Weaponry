import { useCallback, useMemo } from 'react'
import { forgeApi } from '../../shared/api/forgeApi'
import { buildLegacyGraphNodeModuleIdMap } from './cadWorkbenchPanelWorkspaceMaps'

type LegacyGraphNodeIndex = {
  readonly node_id: string
  readonly module_id?: string | null
}
type UseCadWorkbenchPanelLegacyGraphSelectionInput = {
  graphNodes: readonly LegacyGraphNodeIndex[] | null | undefined
  selectLegacyModuleGraphNode: (nodeId: string, moduleId: string) => void
}

type UseCadWorkbenchPanelLegacyGraphSelectionResult = {
  getModuleFileUrl: (moduleId: string) => string
  selectGraphNode: (nodeId: string) => void
}

export function useCadWorkbenchPanelLegacyGraphSelection({
  graphNodes,
  selectLegacyModuleGraphNode,
}: UseCadWorkbenchPanelLegacyGraphSelectionInput): UseCadWorkbenchPanelLegacyGraphSelectionResult {
  const getModuleFileUrl = useCallback((moduleId: string) => forgeApi.getModuleAssetFileUrl(moduleId), [])
  const graphNodeModuleIdByNodeId = useMemo(() => {
    return buildLegacyGraphNodeModuleIdMap(graphNodes)
  }, [graphNodes])

  const selectGraphNode = useCallback(
    (nodeId: string) => selectLegacyModuleGraphNode(nodeId, graphNodeModuleIdByNodeId.get(nodeId) ?? ''),
    [graphNodeModuleIdByNodeId, selectLegacyModuleGraphNode],
  )

  return {
    getModuleFileUrl,
    selectGraphNode,
  }
}
