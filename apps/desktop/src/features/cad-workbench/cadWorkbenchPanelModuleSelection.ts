import type { ModuleAssetRecord, ModuleGraphRecord } from '../../shared/types'

import { buildSelectedModuleLabel } from './cadWorkbenchPanelModuleDisplay'

type CadWorkbenchPanelModuleSelection = {
  selectedNode: ModuleGraphRecord['graph']['nodes'][number] | null
  selectedModuleLabel: string
}

export function resolveCadWorkbenchPanelModuleSelection(
  graphRecord: ModuleGraphRecord | null,
  catalogModules: ModuleAssetRecord[],
  selectedComponent: string,
): CadWorkbenchPanelModuleSelection {
  const selectedNode = graphRecord?.graph.nodes.find((node) => node.node_id === selectedComponent) ?? null
  const selectedModule = selectedNode
    ? catalogModules.find((item) => item.manifest.module_id === selectedNode.module_id) ?? null
    : null

  return {
    selectedNode,
    selectedModuleLabel: buildSelectedModuleLabel(selectedModule, '当前部件'),
  }
}
