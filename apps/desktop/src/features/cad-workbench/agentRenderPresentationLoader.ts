import type { ForgeApi } from '../../shared/api/forgeApi'
import type { AgentAssetRenderSet, AgentAssetRenderView } from '../../shared/types'

const RENDER_VIEW_SIZE = {
  width: 512,
  height: 512,
} as const

const RENDER_SET_CHANGED_TEXT = '概念图对应的设计版本已变化，请重新生成后再下载。'
const RENDER_VIEW_READY_TEXT = '已生成四视图和爆炸概念图。它们均为当前 Agent 资产的只读透明预览，不会改变模型版本。'
const RENDER_VIEW_READY_PARTIAL_TEXT = '已生成四张概念视图。当前模型不能安全分离出爆炸概念图；模型版本没有变化。'
const RENDER_REQUEST_FAILED_TEXT = '概念图生成失败：'
const RENDER_PACKAGE_DOWNLOAD_FAILED_TEXT = '概念图包下载失败：'
const RENDER_PACKAGE_MISMATCH_TEXT = '概念图包与当前预览不一致，未开始下载。请重新生成概念图。'
const RENDER_PACKAGE_DOWNLOADED_TEXT = '已下载概念图包：只包含当前概念 PNG 与来源清单，不包含模型源文件或工程信息。'

type AgentRenderApi = Pick<
  ForgeApi,
  'renderAgentAssetViews' |
  'downloadAgentAssetRenderPackage'
>

type RenderAgentViewsCallbacks = {
  startAgentRenderRequest: (projectId: string, assetVersionId: string) => number | null
  receiveAgentRenderSet: (
    projectId: string,
    assetVersionId: string,
    requestId: number,
    renderSet: AgentAssetRenderSet,
  ) => boolean
  failAgentRenderRequest: (projectId: string, assetVersionId: string, requestId: number) => boolean
  setAssistantNote: (note: string) => void
}

type DownloadAgentRenderPackageCallbacks = {
  startAgentRenderPackageRequest: (projectId: string, assetVersionId: string, renderSetSha256: string) => number | null
  finishAgentRenderPackageRequest: (
    projectId: string,
    assetVersionId: string,
    requestId: number,
    renderSetSha256: string,
  ) => boolean
  setAssistantNote: (note: string) => void
  downloadBlobFile: (blob: Blob, filename: string) => void
  errorText: (caught: unknown) => string
}

export async function renderAgentViews(
  api: AgentRenderApi,
  callbacks: RenderAgentViewsCallbacks,
  projectId: string,
  assetVersionId: string,
): Promise<void> {
  const { startAgentRenderRequest, receiveAgentRenderSet, failAgentRenderRequest, setAssistantNote } = callbacks
  const requestId = startAgentRenderRequest(projectId, assetVersionId)
  if (requestId === null) return

  try {
    const result = await api.renderAgentAssetViews(assetVersionId, {
      width: RENDER_VIEW_SIZE.width,
      height: RENDER_VIEW_SIZE.height,
    })
    if (!receiveAgentRenderSet(projectId, assetVersionId, requestId, result)) return
    setAssistantNote(result.exploded_view_available
      ? RENDER_VIEW_READY_TEXT
      : RENDER_VIEW_READY_PARTIAL_TEXT)
  } catch (caught) {
    if (!failAgentRenderRequest(projectId, assetVersionId, requestId)) return
    setAssistantNote(`${RENDER_REQUEST_FAILED_TEXT}${caught instanceof Error ? caught.message : ''}`)
  }
}

export async function downloadAgentRenderPackage(
  api: AgentRenderApi,
  callbacks: DownloadAgentRenderPackageCallbacks,
  input: {
    projectId: string
    assetVersionId: string
    renderSet: AgentAssetRenderSet | null
  },
): Promise<void> {
  const {
    startAgentRenderPackageRequest,
    finishAgentRenderPackageRequest,
    setAssistantNote,
    downloadBlobFile,
    errorText,
  } = callbacks

  const { projectId, assetVersionId, renderSet } = input

  if (!renderSet || renderSet.asset_version_id !== assetVersionId) {
    setAssistantNote(RENDER_SET_CHANGED_TEXT)
    return
  }

  const requestId = startAgentRenderPackageRequest(
    projectId,
    assetVersionId,
    renderSet.render_set_sha256,
  )
  if (requestId === null) return

  try {
    const result = await api.downloadAgentAssetRenderPackage(assetVersionId, {
      width: renderSet.width,
      height: renderSet.height,
      render_set_sha256: renderSet.render_set_sha256,
    })
    if (!finishAgentRenderPackageRequest(projectId, assetVersionId, requestId, renderSet.render_set_sha256)) return
    if (result.renderSetSha256 && result.renderSetSha256 !== renderSet.render_set_sha256) {
      setAssistantNote(RENDER_PACKAGE_MISMATCH_TEXT)
      return
    }
    downloadBlobFile(result.blob, result.filename)
    setAssistantNote(RENDER_PACKAGE_DOWNLOADED_TEXT)
  } catch (caught) {
    if (!finishAgentRenderPackageRequest(projectId, assetVersionId, requestId, renderSet.render_set_sha256)) return
    setAssistantNote(`${RENDER_PACKAGE_DOWNLOAD_FAILED_TEXT}${errorText(caught)}`)
  }
}

export function readRenderViewFilename(assetVersionId: string, view: AgentAssetRenderView): string {
  return `${assetVersionId}-${view.view_id}.png`
}
