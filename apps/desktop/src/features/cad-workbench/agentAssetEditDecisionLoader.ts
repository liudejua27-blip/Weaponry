import type { ForgeApi } from '../../shared/api/forgeApi'
import type { AgentAssetChangeSet } from '../../shared/types'

type AgentAssetEditDecisionApi = Pick<
  ForgeApi,
  'confirmAgentAssetChangeSet' |
  'rejectAgentAssetChangeSet'
>

type ConfirmAgentAssetEditCallbacks = {
  clearAgentAssetWorkspaceQuality: (projectId: string | null) => void
  refreshActiveDesign: (projectId: string) => Promise<unknown>
  setBlockoutShapeProgram: (
    projectId: string | null,
    shapeProgram: Record<string, unknown> | null,
  ) => number | null
  setAgentAssetChangeSet: (changeSet: AgentAssetChangeSet | null) => void
  setAssistantNote: (note: string) => void
}

type RejectAgentAssetEditCallbacks = {
  refreshActiveDesign: (projectId: string) => Promise<unknown>
  setAgentAssetChangeSet: (changeSet: AgentAssetChangeSet | null) => void
  setAssistantNote: (note: string) => void
}

type ConfirmAgentAssetEditInput = {
  changeSet: AgentAssetChangeSet | null
}

type RejectAgentAssetEditInput = {
  changeSet: AgentAssetChangeSet | null
}

export async function confirmAgentAssetEdit(
  api: AgentAssetEditDecisionApi,
  callbacks: ConfirmAgentAssetEditCallbacks,
  input: ConfirmAgentAssetEditInput,
): Promise<void> {
  const {
    clearAgentAssetWorkspaceQuality,
    refreshActiveDesign,
    setBlockoutShapeProgram,
    setAgentAssetChangeSet,
    setAssistantNote,
  } = callbacks
  const { changeSet } = input
  if (!changeSet) return

  const projectId = changeSet.project_id
  try {
    const confirmed = await api.confirmAgentAssetChangeSet(
      changeSet.change_set_id,
      `agent-asset-confirm-${Date.now()}`,
    )
    setBlockoutShapeProgram(projectId, confirmed.asset_version.shape_program)
    if (projectId) {
      clearAgentAssetWorkspaceQuality(projectId)
      await refreshActiveDesign(projectId)
    }
    setAgentAssetChangeSet(null)
    setAssistantNote(`已确认修改并创建可编辑资产 v${confirmed.asset_version.version_no}。`)
  } catch {
    setAssistantNote('确认部件修改失败；请重新预览，当前版本没有变化。')
  }
}

export async function rejectAgentAssetEdit(
  api: AgentAssetEditDecisionApi,
  callbacks: RejectAgentAssetEditCallbacks,
  input: RejectAgentAssetEditInput,
): Promise<void> {
  const { refreshActiveDesign, setAgentAssetChangeSet, setAssistantNote } = callbacks
  const { changeSet } = input
  if (!changeSet) return

  const projectId = changeSet.project_id
  try {
    await api.rejectAgentAssetChangeSet(changeSet.change_set_id, `agent-asset-reject-${Date.now()}`)
    setAgentAssetChangeSet(null)
    if (projectId) await refreshActiveDesign(projectId)
    setAssistantNote('已取消本次部件修改；当前资产版本没有变化。')
  } catch {
    setAssistantNote('取消修改失败，请稍后重试。')
  }
}
