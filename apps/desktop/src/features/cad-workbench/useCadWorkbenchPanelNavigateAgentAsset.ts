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
    if (!activeAgentAssetVersion || agentAssetChangeSet) return
    const projectId = activeDesignSnapshot?.project_id ?? activeAgentAssetVersion.project_id
    if (!projectId) return
    // Confirmation can update the rendered asset before the async Snapshot
    // hydration finishes. The action button remains valid, so obtain the
    // Rust-owned snapshot/ETag here instead of silently dropping undo/redo.
    const hydratedSnapshot = activeDesignSnapshot
      ? { data: activeDesignSnapshot, etag: activeDesignSnapshotEtag }
      : await api.getActiveDesign(projectId)
    const snapshot = hydratedSnapshot.data
    const snapshotEtag = hydratedSnapshot.etag
    const requestId = startActiveDesignRequest(action === 'undo' ? 'undoing' : 'redoing')
    setAssistantNote(action === 'undo'
      ? '正在返回上一个 Agent 资产版本…'
      : '正在重做上一次 Agent 修改…')
    try {
      const input = {
        client_request_id: `active-design-${action}-${Date.now()}`,
        snapshot_revision: snapshot.revision,
      }
      const response = action === 'undo'
        ? await api.undoActiveDesign(projectId, input, { ifMatch: snapshotEtag ?? undefined })
        : await api.redoActiveDesign(projectId, input, { ifMatch: snapshotEtag ?? undefined })
      if (!receiveActiveDesignSnapshot(projectId, requestId, response)) return
      setAgentAssetChangeSet(null)
      await refreshActiveDesign(projectId)
      setAssistantNote(action === 'undo'
        ? '已返回上一版内容，并创建新的可恢复资产版本。'
        : '已重做上一次内容，并创建新的可恢复资产版本。')
    } catch (caught) {
      const error = failActiveDesignRequest(requestId, caught)
      if (!error) return
      setAssistantNote(error.message)
      if (error.shouldReloadSnapshot) await refreshActiveDesign(projectId)
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
