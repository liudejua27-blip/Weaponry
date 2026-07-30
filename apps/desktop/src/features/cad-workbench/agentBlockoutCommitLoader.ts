import { ForgeApiError, type ForgeApi } from '../../shared/api/forgeApi'
import type { SegmentAgentBlockoutResponse, AgentAssetChangeSet } from '../../shared/types'

type AgentBlockoutCommitApi = Pick<ForgeApi, 'commitAgentBlockout'>

type AgentBlockoutCommitCallbacks = {
  clearAgentEditAssistPresentation: () => void
  refreshActiveDesign: (projectId: string) => Promise<unknown>
  setAgentAssetChangeSet: (changeSet: AgentAssetChangeSet | null) => void
  setAgentCandidateSelectedPartId: (partId: string | null) => void
  setAssistantNote: (message: string) => void
}

type AgentBlockoutCommitInput = {
  projectId: string | null
  segmentation: SegmentAgentBlockoutResponse
}

export async function commitAgentBlockout(
  api: AgentBlockoutCommitApi,
  callbacks: AgentBlockoutCommitCallbacks,
  input: AgentBlockoutCommitInput,
): Promise<void> {
  const {
    clearAgentEditAssistPresentation,
    refreshActiveDesign,
    setAgentAssetChangeSet,
    setAgentCandidateSelectedPartId,
    setAssistantNote,
  } = callbacks

  const { projectId, segmentation } = input
  if (projectId === null) return

  setAssistantNote('正在把分件候选保存为可编辑资产…')
  try {
    const version = await api.commitAgentBlockout({
      client_request_id: `agent-asset-commit-${Date.now()}`,
      artifact_id: segmentation.artifact_id,
      project_id: projectId,
      summary: '确认分件候选并保存为可编辑资产',
    })
    clearAgentEditAssistPresentation()
    setAgentAssetChangeSet(null)
    setAgentCandidateSelectedPartId(null)
    await refreshActiveDesign(projectId)
    setAssistantNote(`已保存为可编辑资产 v${version.version_no}；之后的部件修改都会先预览再创建新版本。`)
  } catch (caught) {
    const message = caught instanceof ForgeApiError
      ? `${caught.message}（${caught.code}）`
      : '保存可编辑资产失败。'
    setAssistantNote(`${message} 当前仍保留候选预览，未覆盖已有版本。`)
  }
}
