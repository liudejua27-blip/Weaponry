import { ForgeApiError, type ForgeApi } from '../../shared/api/forgeApi'
import type { AgentAssetChangeSet } from '../../shared/types'
import type { SingleResultReadyDecision } from './singleResultDecisionPresentationState'

type ConfirmSingleResultApi = Pick<
  ForgeApi,
  'confirmSingleResultPreview'
>

type ConfirmSingleResultCallbacks = {
  clearAgentEditAssistPresentation: () => void
  refreshActiveDesign: (projectId: string) => Promise<unknown>
  setAgentAssetChangeSet: (changeSet: AgentAssetChangeSet | null) => void
  setAgentCandidateSelectedPartId: (partId: string | null) => void
  setAssistantNote: (message: string) => void
  dispatchSingleResultDecision: (action: {
    type: 'request_cancelled'
    projectId: string | null
    requestId: number
  }) => void
  latestRequestId: number
  errorText: (caught: unknown) => string
}

export async function confirmSingleResultPreview(
  api: ConfirmSingleResultApi,
  callbacks: ConfirmSingleResultCallbacks,
  decision: SingleResultReadyDecision,
): Promise<void> {
  const {
    clearAgentEditAssistPresentation,
    dispatchSingleResultDecision,
    errorText,
    latestRequestId,
    refreshActiveDesign,
    setAgentAssetChangeSet,
    setAgentCandidateSelectedPartId,
    setAssistantNote,
  } = callbacks

  try {
    const version = await api.confirmSingleResultPreview({
      projectId: decision.project_id,
      turnId: decision.turn_id,
      previewId: decision.preview.preview_id,
      artifactSha256: decision.preview.artifact_sha256,
      artifactProfileId: decision.preview.artifact_profile_id,
      clientRequestId: `single-result-confirm-${decision.preview.preview_id}`,
      summary: decision.summary,
    })
    clearAgentEditAssistPresentation()
    setAgentAssetChangeSet(null)
    setAgentCandidateSelectedPartId(null)
    await refreshActiveDesign(decision.project_id)
    dispatchSingleResultDecision({
      type: 'request_cancelled',
      projectId: decision.project_id,
      requestId: latestRequestId,
    })
    setAssistantNote(`已保存为可编辑资产 v${version.version_no}；预览、质量、导出和当前版本将继续由同一 Snapshot 约束。`)
  } catch (caught) {
    const message = caught instanceof ForgeApiError ? `${caught.message}（${caught.code}）` : errorText(caught)
    setAssistantNote(`正式结果保存失败：${message}。当前预览仍未写入版本。`)
  }
}
