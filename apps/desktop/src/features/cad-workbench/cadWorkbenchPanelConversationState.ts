import type { ActiveDesignSnapshot, ModuleAssetRecord, ModuleGraphRecord } from '../../shared/types'

import type { WorkbenchStatusBarPresentation } from './workbenchStatusBarPresentation'
import { resolveCadWorkbenchPanelModuleSelection } from './cadWorkbenchPanelModuleSelection'
import { useCadWorkbenchPanelStatusBar } from './useCadWorkbenchPanelStatusBar'

type ProjectVersion = {
  version_id: string
  version_no: number | string
}

export type CadWorkbenchPanelConversationState = {
  selectedNode: ModuleGraphRecord['graph']['nodes'][number] | null
  selectedModuleLabel: string
  workbenchStatusBar: WorkbenchStatusBarPresentation
}

type CadWorkbenchPanelConversationStateInput = {
  conceptGraphRecord: ModuleGraphRecord | null
  catalogModules: ModuleAssetRecord[]
  selectedComponent: string
  conceptLoading: boolean
  conceptLegacyDetailsEnabled: boolean
  activeAgentAssetVersionVersionNo: number | null
  activeDesignSnapshot: ActiveDesignSnapshot | null
  conceptVersions: readonly ProjectVersion[] | null | undefined
  conceptVersionId: string | null
  conceptQualityStatus?: 'passed' | 'warning' | 'failed' | 'not_run' | 'unavailable'
  agentQualityStatus?: 'passed' | 'warning' | 'failed' | 'unavailable' | null
}

export function useCadWorkbenchPanelConversationState({
  conceptGraphRecord,
  catalogModules,
  selectedComponent,
  conceptLoading,
  conceptLegacyDetailsEnabled,
  activeAgentAssetVersionVersionNo,
  activeDesignSnapshot,
  conceptVersions,
  conceptVersionId,
  conceptQualityStatus,
  agentQualityStatus,
}: CadWorkbenchPanelConversationStateInput): CadWorkbenchPanelConversationState {
  const { selectedNode, selectedModuleLabel } = resolveCadWorkbenchPanelModuleSelection(
    conceptGraphRecord,
    catalogModules,
    selectedComponent,
  )

  const workbenchStatusBar = useCadWorkbenchPanelStatusBar({
    conceptLoading,
    conceptLegacyDetailsEnabled,
    activeAgentAssetVersionVersionNo,
    activeDesignSnapshot,
    conceptVersions,
    conceptVersionId,
    conceptQualityStatus,
    agentQualityStatus,
  })

  return {
    selectedNode,
    selectedModuleLabel,
    workbenchStatusBar,
  }
}
