import { useCallback } from 'react'
import type { ForgeApi } from '../../shared/api/forgeApi'
import { isNativeDesktopRuntime } from '../../shared/api/appServerTransport'
import type { AgentAssetRenderSet, AgentAssetRenderView } from '../../shared/types'
import { downloadBase64File, downloadBlobFile, downloadUrlFile } from './cadWorkbenchPanelFileUtils'
import {
  readRenderViewFilename,
  renderAgentViews,
  downloadAgentRenderPackage,
} from './agentRenderPresentationLoader'

type AgentRenderAssetVersion = {
  project_id: string
  asset_version_id: string
  version_no: number
}

type DrawerType = 'export' | 'quality'

type UseCadWorkbenchPanelViewportActionsInput = {
  api: ForgeApi
  conceptProjectId: string | null
  activeAgentAssetVersion: AgentRenderAssetVersion | null
  renderSet: AgentAssetRenderSet | null
  openDrawer: (drawer: DrawerType) => void
  closeAgentRenderPresentation: () => void
  closeDrawers: () => void
  startAgentRenderRequest: (projectId: string, assetVersionId: string) => number | null
  receiveAgentRenderSet: (
    projectId: string,
    assetVersionId: string,
    requestId: number,
    renderSet: AgentAssetRenderSet,
  ) => boolean
  failAgentRenderRequest: (projectId: string, assetVersionId: string, requestId: number) => boolean
  startAgentRenderPackageRequest: (projectId: string, assetVersionId: string, renderSetSha256: string) => number | null
  finishAgentRenderPackageRequest: (
    projectId: string,
    assetVersionId: string,
    requestId: number,
    renderSetSha256: string,
  ) => boolean
  setAssistantNote: (note: string) => void
  errorText: (caught: unknown) => string
}

type UseCadWorkbenchPanelViewportActionsResult = {
  closeAllDrawers: () => void
  openExportDrawer: () => void
  openQualityDrawer: () => void
  handleDownloadAgentGlb: () => Promise<void>
  handleRenderAgentViews: () => Promise<void>
  handleDownloadAgentRenderView: (view: AgentAssetRenderView) => void
  handleDownloadAgentRenderPackage: () => Promise<void>
}

export function useCadWorkbenchPanelViewportActions({
  api,
  conceptProjectId,
  activeAgentAssetVersion,
  renderSet,
  openDrawer,
  closeAgentRenderPresentation,
  closeDrawers,
  startAgentRenderRequest,
  receiveAgentRenderSet,
  failAgentRenderRequest,
  startAgentRenderPackageRequest,
  finishAgentRenderPackageRequest,
  setAssistantNote,
  errorText,
}: UseCadWorkbenchPanelViewportActionsInput): UseCadWorkbenchPanelViewportActionsResult {
  const closeAllDrawers = useCallback(() => {
    closeAgentRenderPresentation()
    closeDrawers()
  }, [closeAgentRenderPresentation, closeDrawers])

  const openExportDrawer = useCallback(() => openDrawer('export'), [openDrawer])
  const openQualityDrawer = useCallback(() => openDrawer('quality'), [openDrawer])

  const handleDownloadAgentGlb = useCallback(async () => {
    if (!activeAgentAssetVersion) {
      setAssistantNote('正在同步当前设计版本，请稍后再下载。')
      return
    }
    try {
      if (!isNativeDesktopRuntime()) {
        downloadUrlFile(
          api.getAgentAssetProductionGlbUrl(activeAgentAssetVersion.asset_version_id),
          `${activeAgentAssetVersion.asset_version_id}.glb`,
        )
        setAssistantNote(`已请求下载当前 Agent 设计 v${activeAgentAssetVersion.version_no} 的生产级概念 GLB。`)
        return
      }
      const result = await api.downloadAgentAssetProductionGlb(activeAgentAssetVersion.asset_version_id)
      downloadBlobFile(result.blob, result.filename)
      setAssistantNote(`已下载当前 Agent 设计 v${activeAgentAssetVersion.version_no} 的生产级概念 GLB；下载前已完成 ${result.triangleCount.toLocaleString()} 三角形回读。`)
    } catch (caught) {
      setAssistantNote(`3D 模型下载失败：${errorText(caught)}`)
    }
  }, [
    activeAgentAssetVersion,
    api,
    setAssistantNote,
  ])

  const handleRenderAgentViews = useCallback(async () => {
    const projectId = conceptProjectId
    if (!activeAgentAssetVersion || !projectId) return
    await renderAgentViews(
      api,
      {
        startAgentRenderRequest,
        receiveAgentRenderSet,
        failAgentRenderRequest,
        setAssistantNote,
      },
      projectId,
      activeAgentAssetVersion.asset_version_id,
    )
  }, [
    activeAgentAssetVersion,
    api,
    conceptProjectId,
    failAgentRenderRequest,
    receiveAgentRenderSet,
    setAssistantNote,
    startAgentRenderRequest,
  ])

  const handleDownloadAgentRenderView = useCallback((view: AgentAssetRenderView) => {
    const assetVersionId = activeAgentAssetVersion?.asset_version_id
    if (!assetVersionId) return
    downloadBase64File(view.png_base64, readRenderViewFilename(assetVersionId, view), 'image/png')
  }, [activeAgentAssetVersion])

  const handleDownloadAgentRenderPackage = useCallback(async () => {
    const projectId = conceptProjectId
    if (!activeAgentAssetVersion || !projectId) return
    await downloadAgentRenderPackage(
      api,
      {
        startAgentRenderPackageRequest,
        finishAgentRenderPackageRequest,
        setAssistantNote,
        downloadBlobFile,
        errorText,
      },
      {
        projectId,
        assetVersionId: activeAgentAssetVersion.asset_version_id,
        renderSet,
      },
    )
  }, [
    activeAgentAssetVersion,
    conceptProjectId,
    api,
    downloadBlobFile,
    errorText,
    finishAgentRenderPackageRequest,
    renderSet,
    setAssistantNote,
    startAgentRenderPackageRequest,
  ])

  return {
    closeAllDrawers,
    openExportDrawer,
    openQualityDrawer,
    handleDownloadAgentGlb,
    handleRenderAgentViews,
    handleDownloadAgentRenderView,
    handleDownloadAgentRenderPackage,
  }
}
