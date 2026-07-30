import { useCallback } from 'react'

import type { ActiveDesignApiResponse, ActiveDesignErrorState, ForgeApi } from '../../shared/api/forgeApi'
import type { ActiveDesignSnapshot, AgentAssetVersion } from '../../shared/types'
import type { ActiveDesignOperation } from './activeDesignMachine.js'
import { partDisplayActionNote } from './cadWorkbenchPanelPartDisplay'
import {
  buildPartDisplayRequest,
  buildSelectPartRequest,
  buildSelectZoneRequest,
  PART_DISPLAY_BUSY_NOTICE,
  PART_SELECTION_NOT_ACTIVE_NOTICE,
  resolvePartFirstMaterialZoneId,
} from './cadWorkbenchPanelPartSelection'

type UseCadWorkbenchPanelActiveDesignPartActionsInput = {
  api: ForgeApi
  activeDesignSnapshot: ActiveDesignSnapshot | null
  activeDesignSnapshotEtag: string | null
  agentAssetVersion: AgentAssetVersion | null
  setAssistantMode: (assistantMode: 'brief' | 'change') => void
  setAssistantNote: (message: string) => void
  setAgentCandidateSelectedPartId: (partId: string | null) => void
  setAppearanceMaterialZoneId: (zoneId: string) => void
  hasAgentAssetChangeSet: boolean
  startActiveDesignRequest: (operation: Exclude<ActiveDesignOperation, 'idle'>) => number
  failActiveDesignRequest: (requestId: number, caught: unknown) => ActiveDesignErrorState | null
  receiveActiveDesignSnapshot: (
    projectId: string,
    requestId: number,
    response: ActiveDesignApiResponse<ActiveDesignSnapshot>,
  ) => boolean
  refreshActiveDesign: (projectId: string) => Promise<void>
  activeDesignCanSelectParts: (snapshot: ActiveDesignSnapshot | null) => boolean
  activeDesignSelectedPartId: (snapshot: ActiveDesignSnapshot | null) => string | null
  activeDesignSelectedMaterialZoneId: (snapshot: ActiveDesignSnapshot | null) => string | null
  projectAgentAssetWorkspaceSelection: (projectId: string, assetVersionId: string, selectedPartId: string | null) => void
  legacyDesignReadOnly: boolean
}

type UseCadWorkbenchPanelActiveDesignPartActionsResult = {
  selectAgentPart: (partId: string) => Promise<void>
  setAgentPartDisplay: (
    action: 'lock' | 'unlock' | 'hide' | 'show' | 'isolate' | 'clear_isolation' | 'show_all',
    partId?: string,
  ) => Promise<void>
  selectMaterialZone: (zoneId: string) => Promise<void>
  requestLegacyAgentRebuild: () => Promise<void>
}

export function useCadWorkbenchPanelActiveDesignPartActions({
  api,
  activeDesignSnapshot,
  activeDesignSnapshotEtag,
  agentAssetVersion,
  setAssistantMode,
  setAssistantNote,
  setAgentCandidateSelectedPartId,
  setAppearanceMaterialZoneId,
  hasAgentAssetChangeSet,
  startActiveDesignRequest,
  failActiveDesignRequest,
  receiveActiveDesignSnapshot,
  refreshActiveDesign,
  activeDesignCanSelectParts,
  activeDesignSelectedPartId,
  activeDesignSelectedMaterialZoneId,
  projectAgentAssetWorkspaceSelection,
  legacyDesignReadOnly,
}: UseCadWorkbenchPanelActiveDesignPartActionsInput): UseCadWorkbenchPanelActiveDesignPartActionsResult {
  const selectAgentPart = useCallback(async (partId: string) => {
    if (
      !agentAssetVersion
      || !activeDesignSnapshot
      || !activeDesignCanSelectParts(activeDesignSnapshot)
      || !('asset_version_id' in activeDesignSnapshot.active_design)
    ) {
      setAgentCandidateSelectedPartId(partId)
      return
    }
    if (activeDesignSnapshot.active_design.asset_version_id !== agentAssetVersion.asset_version_id) {
      setAssistantNote(PART_SELECTION_NOT_ACTIVE_NOTICE)
      if (activeDesignSnapshot.project_id) await refreshActiveDesign(activeDesignSnapshot.project_id)
      return
    }
    const requestId = startActiveDesignRequest('selecting')
    try {
      const response = await api.selectActiveDesignPart(
        activeDesignSnapshot.project_id,
        buildSelectPartRequest(
          activeDesignSnapshot.revision,
          partId,
          resolvePartFirstMaterialZoneId(agentAssetVersion.parts, partId),
        ),
        { ifMatch: activeDesignSnapshotEtag ?? undefined },
      )
      if (!receiveActiveDesignSnapshot(activeDesignSnapshot.project_id, requestId, response)) return
      if ('asset_version_id' in response.data.active_design) {
        projectAgentAssetWorkspaceSelection(
          response.data.project_id,
          response.data.active_design.asset_version_id,
          activeDesignSelectedPartId(response.data),
        )
      }
    } catch (caught) {
      const error = failActiveDesignRequest(requestId, caught)
      if (!error) return
      setAssistantNote(error.message)
      if (error.shouldReloadSnapshot && activeDesignSnapshot.project_id) await refreshActiveDesign(activeDesignSnapshot.project_id)
    }
  }, [
    activeDesignSnapshot,
    activeDesignSnapshotEtag,
    activeDesignCanSelectParts,
    api,
    agentAssetVersion,
    failActiveDesignRequest,
    receiveActiveDesignSnapshot,
    refreshActiveDesign,
    setAgentCandidateSelectedPartId,
    startActiveDesignRequest,
    activeDesignSelectedPartId,
    projectAgentAssetWorkspaceSelection,
    resolvePartFirstMaterialZoneId,
    setAssistantNote,
  ])

  const setAgentPartDisplay = useCallback(async (
    action: 'lock' | 'unlock' | 'hide' | 'show' | 'isolate' | 'clear_isolation' | 'show_all',
    partId?: string,
  ) => {
    const snapshot = activeDesignSnapshot
    if (!snapshot || snapshot.active_design.source !== 'agent_asset' || hasAgentAssetChangeSet) {
      setAssistantNote(PART_DISPLAY_BUSY_NOTICE)
      return
    }
    const requestId = startActiveDesignRequest('setting_part_display')
    try {
      const response = await api.setActiveDesignPartDisplay(
        snapshot.project_id,
        buildPartDisplayRequest(snapshot.revision, action, partId),
        { ifMatch: activeDesignSnapshotEtag ?? undefined },
      )
      if (!receiveActiveDesignSnapshot(snapshot.project_id, requestId, response)) return
      if ('asset_version_id' in response.data.active_design) {
        projectAgentAssetWorkspaceSelection(
          response.data.project_id,
          response.data.active_design.asset_version_id,
          activeDesignSelectedPartId(response.data),
        )
      }
      const message = partDisplayActionNote(action)
      setAssistantNote(message)
    } catch (caught) {
      const error = failActiveDesignRequest(requestId, caught)
      if (!error) return
      setAssistantNote(error.message)
      if (error.shouldReloadSnapshot) await refreshActiveDesign(snapshot.project_id)
    }
  }, [
    activeDesignSnapshot,
    api,
    activeDesignSnapshotEtag,
    failActiveDesignRequest,
    receiveActiveDesignSnapshot,
    refreshActiveDesign,
    setAssistantNote,
    startActiveDesignRequest,
    projectAgentAssetWorkspaceSelection,
    activeDesignSelectedPartId,
    hasAgentAssetChangeSet,
  ])

  const selectMaterialZone = useCallback(async (zoneId: string) => {
    setAppearanceMaterialZoneId(zoneId)
    const selectedPartId = activeDesignSelectedPartId(activeDesignSnapshot)
    if (
      !activeDesignSnapshot
      || !selectedPartId
      || !activeDesignCanSelectParts(activeDesignSnapshot)
      || !('asset_version_id' in activeDesignSnapshot.active_design)
      || legacyDesignReadOnly
    ) return
    const requestId = startActiveDesignRequest('selecting')
    try {
      const response = await api.selectActiveDesignPart(
        activeDesignSnapshot.project_id,
        buildSelectZoneRequest(
          activeDesignSnapshot.revision,
          selectedPartId,
          zoneId,
        ),
        { ifMatch: activeDesignSnapshotEtag ?? undefined },
      )
      if (!receiveActiveDesignSnapshot(activeDesignSnapshot.project_id, requestId, response)) return
      if ('asset_version_id' in response.data.active_design) {
        projectAgentAssetWorkspaceSelection(
          response.data.project_id,
          response.data.active_design.asset_version_id,
          activeDesignSelectedPartId(response.data),
        )
      }
      setAppearanceMaterialZoneId(activeDesignSelectedMaterialZoneId(response.data) ?? zoneId)
    } catch (caught) {
      const error = failActiveDesignRequest(requestId, caught)
      if (!error) return
      setAssistantNote(error.message)
      if (error.shouldReloadSnapshot) {
        if (activeDesignSnapshot?.project_id) await refreshActiveDesign(activeDesignSnapshot.project_id)
      }
    }
  }, [
    activeDesignSnapshot,
    activeDesignSnapshotEtag,
    activeDesignCanSelectParts,
    api,
    failActiveDesignRequest,
    legacyDesignReadOnly,
    receiveActiveDesignSnapshot,
    refreshActiveDesign,
    setAppearanceMaterialZoneId,
    setAssistantNote,
    startActiveDesignRequest,
    activeDesignSelectedPartId,
    activeDesignSelectedMaterialZoneId,
    projectAgentAssetWorkspaceSelection,
  ])

  const requestLegacyAgentRebuild = useCallback(async () => {
    if (!activeDesignSnapshot || !legacyDesignReadOnly || !('legacy_version_id' in activeDesignSnapshot.active_design)) return
    const requestId = startActiveDesignRequest('converting_legacy')
    try {
      const result = await api.convertLegacyActiveDesign(
        activeDesignSnapshot.project_id,
        {
          client_request_id: `legacy-agent-rebuild-${Date.now()}`,
          snapshot_revision: activeDesignSnapshot.revision,
        },
        { ifMatch: activeDesignSnapshotEtag ?? undefined },
      )
      setAssistantMode('brief')
      setAssistantNote(
        `${result.data.message} 请描述希望保留或重新设计的外观，Agent 会生成新的可编辑候选。`,
      )
      await refreshActiveDesign(activeDesignSnapshot.project_id)
    } catch (caught) {
      const error = failActiveDesignRequest(requestId, caught)
      if (!error) return
      setAssistantNote(error.message)
      if (error.shouldReloadSnapshot && activeDesignSnapshot.project_id) await refreshActiveDesign(activeDesignSnapshot.project_id)
    }
  }, [
    activeDesignSnapshot,
    activeDesignSnapshotEtag,
    api,
    failActiveDesignRequest,
    legacyDesignReadOnly,
    refreshActiveDesign,
    setAssistantMode,
    setAssistantNote,
    startActiveDesignRequest,
  ])

  return {
    selectAgentPart,
    setAgentPartDisplay,
    selectMaterialZone,
    requestLegacyAgentRebuild,
  }
}
