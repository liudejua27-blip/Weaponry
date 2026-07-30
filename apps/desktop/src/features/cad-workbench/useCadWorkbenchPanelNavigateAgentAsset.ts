import { useCallback } from 'react'

import type { ActiveDesignApiResponse, ActiveDesignErrorState, ForgeApi } from '../../shared/api/forgeApi'
import type { ActiveDesignSnapshot, AgentAssetChangeSet, AgentAssetVersion } from '../../shared/types'
import type { ActiveDesignOperation } from './activeDesignMachine.js'

type UseCadWorkbenchPanelNavigateAgentAssetInput = {
  api: ForgeApi
  activeDesignSnapshot: ActiveDesignSnapshot | null
  activeDesignSnapshotEtag: string | null
  activeAgentAssetVersion: AgentAssetVersion | null
  agentAssetChangeSet: boolean
  startActiveDesignRequest: (operation: Exclude<ActiveDesignOperation, 'idle'>) => number
  failActiveDesignRequest: (requestId: number, caught: unknown) => ActiveDesignErrorState | null
  receiveActiveDesignSnapshot: (
    projectId: string,
    requestId: number,
    response: ActiveDesignApiResponse<ActiveDesignSnapshot>,
  ) => boolean
  refreshActiveDesign: (projectId: string) => Promise<void>
  setAssistantNote: (message: string) => void
  setAgentAssetChangeSet: (changeSet: AgentAssetChangeSet | null) => void
}

type UseCadWorkbenchPanelNavigateAgentAssetOutput = {
  navigateAgentAsset: (action: 'undo' | 'redo') => Promise<void>
}

export function useCadWorkbenchPanelNavigateAgentAsset({
  api,
  activeDesignSnapshot,
  activeDesignSnapshotEtag,
  activeAgentAssetVersion,
  agentAssetChangeSet,
  startActiveDesignRequest,
  failActiveDesignRequest,
  receiveActiveDesignSnapshot,
  refreshActiveDesign,
  setAssistantNote,
  setAgentAssetChangeSet,
}: UseCadWorkbenchPanelNavigateAgentAssetInput): UseCadWorkbenchPanelNavigateAgentAssetOutput {
  const navigateAgentAsset = useCallback(async (action: 'undo' | 'redo') => {
    if (!activeDesignSnapshot || !activeAgentAssetVersion || agentAssetChangeSet) return
    const requestId = startActiveDesignRequest(action === 'undo' ? 'undoing' : 'redoing')
    setAssistantNote(action === 'undo'
      ? '正在返回上一个 Agent 资产版本…'
      : '正在重做上一次 Agent 修改…')
    try {
      const input = {
        client_request_id: `active-design-${action}-${Date.now()}`,
        snapshot_revision: activeDesignSnapshot.revision,
      }
      const response = action === 'undo'
        ? await api.undoActiveDesign(activeDesignSnapshot.project_id, input, { ifMatch: activeDesignSnapshotEtag ?? undefined })
        : await api.redoActiveDesign(activeDesignSnapshot.project_id, input, { ifMatch: activeDesignSnapshotEtag ?? undefined })
      if (!receiveActiveDesignSnapshot(activeDesignSnapshot.project_id, requestId, response)) return
      setAgentAssetChangeSet(null)
      await refreshActiveDesign(activeDesignSnapshot.project_id)
      setAssistantNote(action === 'undo'
        ? '已返回上一版内容，并创建新的可恢复资产版本。'
        : '已重做上一次内容，并创建新的可恢复资产版本。')
    } catch (caught) {
      const error = failActiveDesignRequest(requestId, caught)
      if (!error) return
      setAssistantNote(error.message)
      if (error.shouldReloadSnapshot) await refreshActiveDesign(activeDesignSnapshot.project_id)
    }
  }, [
    activeAgentAssetVersion,
    activeDesignSnapshot,
    activeDesignSnapshotEtag,
    agentAssetChangeSet,
    api,
    failActiveDesignRequest,
    receiveActiveDesignSnapshot,
    refreshActiveDesign,
    setAgentAssetChangeSet,
    setAssistantNote,
    startActiveDesignRequest,
  ])

  return {
    navigateAgentAsset,
  }
}
