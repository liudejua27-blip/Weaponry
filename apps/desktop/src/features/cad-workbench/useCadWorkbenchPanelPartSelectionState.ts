import { useMemo } from 'react'

import type { AgentAssetVersion } from '../../shared/types'
import { buildAgentPartByIdMap } from './cadWorkbenchPanelWorkspaceMaps'
import { resolveWorkbenchSidebarParts } from './cadWorkbenchPanelWorkspaceHelpers'

type CadWorkbenchPanelSidebarPart = {
  part_id: string
  role: string
  material_zone_ids: readonly string[]
}

type UseCadWorkbenchPanelPartSelectionStateInput = {
  hasActiveAgentAsset: boolean
  agentAssetWorkspaceSelectedPartId: string | null
  agentCandidateSelectedPartId: string | null
  agentAssetVersion: AgentAssetVersion | null
  blockoutParts: readonly CadWorkbenchPanelSidebarPart[] | null | undefined
}

type UseCadWorkbenchPanelPartSelectionStateResult = {
  displayedAgentSelectedPartId: string | null
  sidebarParts: readonly CadWorkbenchPanelSidebarPart[]
  selectedAgentPart: AgentAssetVersion['parts'][number] | null
}

export function useCadWorkbenchPanelPartSelectionState({
  hasActiveAgentAsset,
  agentAssetWorkspaceSelectedPartId,
  agentCandidateSelectedPartId,
  agentAssetVersion,
  blockoutParts,
}: UseCadWorkbenchPanelPartSelectionStateInput): UseCadWorkbenchPanelPartSelectionStateResult {
  const displayedAgentSelectedPartId = hasActiveAgentAsset
    ? agentAssetWorkspaceSelectedPartId
    : agentCandidateSelectedPartId

  const partById = useMemo(
    () => buildAgentPartByIdMap(agentAssetVersion?.parts),
    [agentAssetVersion?.asset_version_id, agentAssetVersion?.parts],
  )

  const sidebarParts = useMemo(
    () => resolveWorkbenchSidebarParts(agentAssetVersion?.parts, blockoutParts),
    [agentAssetVersion?.parts, blockoutParts],
  )

  const selectedAgentPart = displayedAgentSelectedPartId === null
    ? null
    : partById.get(displayedAgentSelectedPartId) ?? null

  return {
    displayedAgentSelectedPartId,
    sidebarParts,
    selectedAgentPart,
  }
}
