import type { ForgeApi } from '../../shared/api/forgeApi'
import type { AgentAssetChangeSet, AgentPartEditOperation } from '../../shared/types'
import type { AgentBlockoutGlbKind, AgentBlockoutGlbPayload } from './agentBlockoutDisplayState'
import { previewAgentAssetChangeSet } from './agentBlockoutDisplayLoader'

type AgentAssetEditPreviewApi = Pick<
  ForgeApi,
  'proposeAgentAssetChangeSet' |
  'previewAgentAssetChangeSet' |
  'exportAgentAssetChangeSetPreviewGlb' |
  'rejectAgentAssetChangeSet' |
  'loadAgentAssetPreviewGlb' |
  'loadAgentAssetProductionGlb'
>

type AgentAssetEditPreviewCallbacks = {
  setBlockoutShapeProgram: (projectId: string | null, shapeProgram: Record<string, unknown> | null) => number | null
  setBlockoutGlb: (
    projectId: string | null,
    requestId: number,
    glbBase64: AgentBlockoutGlbPayload | null,
    glbKind: AgentBlockoutGlbKind | null,
  ) => boolean
  setAgentAssetChangeSet: (changeSet: AgentAssetChangeSet | null) => void
  setAssistantNote: (message: string) => void
}

type AgentAssetEditPreviewInput = {
  projectId: string | null
  assetVersionId: string
  shapeProgram: Record<string, unknown> | null
  summary: string
  operation: AgentPartEditOperation | readonly AgentPartEditOperation[]
}

export async function previewAgentAssetEdit(
  api: AgentAssetEditPreviewApi,
  callbacks: AgentAssetEditPreviewCallbacks,
  input: AgentAssetEditPreviewInput,
): Promise<void> {
  const {
    setBlockoutGlb,
    setBlockoutShapeProgram,
    setAgentAssetChangeSet,
    setAssistantNote,
  } = callbacks

  const {
    projectId,
    assetVersionId,
    shapeProgram,
    summary,
    operation,
  } = input
  const operations = Array.isArray(operation) ? [...operation] : [operation]
  const preview = await previewAgentAssetChangeSet(
    api,
    setBlockoutShapeProgram,
    setBlockoutGlb,
    projectId,
    assetVersionId,
    shapeProgram,
    summary,
    operations,
  )

  if (!preview) {
    setAgentAssetChangeSet(null)
    setAssistantNote('真实 PBR 模型预览失败；已取消本次 ChangeSet，当前资产版本没有变化。')
    return
  }

  setAgentAssetChangeSet(preview)
  setAssistantNote(`已生成“${summary}”的真实 PBR 模型预览；确认后才会创建新版本。`)
}
