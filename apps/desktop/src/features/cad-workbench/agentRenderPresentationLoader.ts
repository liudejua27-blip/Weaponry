import type { ForgeApi } from '../../shared/api/forgeApi'
import type { AgentAssetRenderSet, AgentAssetRenderView } from '../../shared/types'
import {
  captureWorkbenchPbrConceptViews,
  sha256Hex,
  WORKBENCH_PBR_RENDERER_ID,
  type WorkbenchPbrConceptCapture,
} from './workbenchPbrCapture.js'

const RENDER_VIEW_SIZE = {
  width: 512,
  height: 512,
} as const

const RENDER_SET_CHANGED_TEXT = '概念图对应的设计版本已变化，请重新生成后再下载。'
const RENDER_VIEW_READY_TEXT = '已生成四视图和爆炸概念图（浏览器兼容诊断路径）。它们是当前 Agent 资产的只读预览，不会改变模型版本。'
const RENDER_VIEW_READY_PARTIAL_TEXT = '已生成四张概念视图（浏览器兼容诊断路径）。当前模型不能安全分离出爆炸概念图；模型版本没有变化。'
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

export type AgentAssetGpuPbrRenderContext = {
  viewport: HTMLElement
  sourceGlbSha256: string
}

/** Returns the exact GLB currently mounted in the native Agent PBR viewport. */
export function readAgentAssetGpuPbrSourceGlbSha256(viewport: HTMLElement | null): string | null {
  const sourceGlbSha256 = viewport?.dataset.blockoutGlbSha256 ?? ''
  if (
    !viewport
    || viewport.dataset.blockoutLoadState !== 'ready'
    || viewport.dataset.blockoutRenderSource !== 'glb_pbr'
    || viewport.dataset.pbrRendererId !== WORKBENCH_PBR_RENDERER_ID
    || !/^[a-f0-9]{64}$/i.test(sourceGlbSha256)
  ) return null
  return sourceGlbSha256.toLowerCase()
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
  gpuPbr?: AgentAssetGpuPbrRenderContext,
): Promise<void> {
  const { startAgentRenderRequest, receiveAgentRenderSet, failAgentRenderRequest, setAssistantNote } = callbacks
  const requestId = startAgentRenderRequest(projectId, assetVersionId)
  if (requestId === null) return

  try {
    if (gpuPbr) {
      const result = await buildGpuPbrRenderSet(assetVersionId, gpuPbr)
      if (!receiveAgentRenderSet(projectId, assetVersionId, requestId, result)) return
      setAssistantNote('已从当前工作台的 GPU/PBR 渲染器生成四视图；这些图片与用户看到的模型同源，不经过软件光栅质量代理。')
      return
    }
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

async function buildGpuPbrRenderSet(
  assetVersionId: string,
  context: AgentAssetGpuPbrRenderContext,
): Promise<AgentAssetRenderSet> {
  const captures = await captureWorkbenchPbrConceptViews({
    viewport: context.viewport,
    sourceGlbSha256: context.sourceGlbSha256,
    lightPreset: 'soft_studio',
  })
  const views = captures.map((capture) => gpuPbrCaptureToRenderView(assetVersionId, capture))
  const fingerprint = {
    schema_version: 'AgentAssetRenderSet@1',
    asset_version_id: assetVersionId,
    renderer_id: 'forgecad-workbench-pbr@1',
    source_glb_sha256: context.sourceGlbSha256.toLowerCase(),
    width: captures[0]?.width ?? 0,
    height: captures[0]?.height ?? 0,
    views: views.map((view) => ({
      view_id: view.view_id,
      camera_view: view.camera_view,
      background_mode: view.background_mode,
      sha256: view.sha256,
      byte_size: view.byte_size,
    })),
  }
  const renderSetSha256 = await sha256Hex(new TextEncoder().encode(JSON.stringify(fingerprint)))
  return {
    schema_version: 'AgentAssetRenderSet@1',
    asset_version_id: assetVersionId,
    renderer_id: 'forgecad-workbench-pbr@1',
    width: captures[0]?.width ?? 0,
    height: captures[0]?.height ?? 0,
    views,
    exploded_view_available: false,
    exploded_unavailable_reason: 'GPU/PBR 概念视图不生成未经同源分件验证的爆炸图。',
    render_set_sha256: renderSetSha256,
    render_set_byte_size: views.reduce((total, view) => total + view.byte_size, 0),
    rendered_at: new Date().toISOString(),
  }
}

function gpuPbrCaptureToRenderView(
  assetVersionId: string,
  capture: WorkbenchPbrConceptCapture,
): AgentAssetRenderView {
  return {
    schema_version: 'AgentAssetRenderView@1',
    asset_version_id: assetVersionId,
    view_id: capture.view_id,
    camera_view: capture.view_id,
    presentation_mode: 'standard',
    background_mode: 'studio',
    part_ids: [],
    mime_type: 'image/png',
    width: capture.width,
    height: capture.height,
    png_base64: bytesToBase64(capture.png_bytes),
    sha256: capture.png_sha256,
    byte_size: capture.png_bytes.byteLength,
    readback_status: 'passed',
  }
}

function bytesToBase64(bytes: Uint8Array): string {
  const chunkBytes = 0x8000
  let binary = ''
  for (let offset = 0; offset < bytes.length; offset += chunkBytes) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkBytes))
  }
  return window.btoa(binary)
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
