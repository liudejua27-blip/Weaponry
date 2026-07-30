import type { ForgeApi } from '../../shared/api/forgeApi'
import type { AgentAssetChangeSet } from '../../shared/types'
import type { AgentBlockoutGlbKind, AgentBlockoutGlbPayload } from './agentBlockoutDisplayState'
import { previewSurfaceAdornment, restoreAgentAssetBlockoutPreview } from './agentBlockoutDisplayLoader.js'
import type {
  SurfaceAdornmentAdapter,
} from './SurfaceAdornmentDrawer'

type SurfaceAdornmentApi = Pick<
  ForgeApi,
  'enableSurfaceAdornmentSkill' |
  'confirmAgentAssetChangeSet' |
  'rejectAgentAssetChangeSet' |
  'proposeSurfaceAdornmentPreview' |
  'previewAgentAssetChangeSet' |
  'exportAgentAssetChangeSetPreviewGlb' |
  'loadAgentAssetPreviewGlb' |
  'loadAgentAssetProductionGlb'
>

type BlockoutShapeSetter = (projectId: string | null, shapeProgram: Record<string, unknown> | null) => number | null
type BlockoutGlbSetter = (
  projectId: string | null,
  requestId: number,
  glbBase64: AgentBlockoutGlbPayload | null,
  glbKind: AgentBlockoutGlbKind | null,
) => boolean

type SurfaceAdornmentAdapterCallbacks = {
  setAgentAssetChangeSet: (changeSet: AgentAssetChangeSet | null) => void
  setBlockoutShapeProgram: BlockoutShapeSetter
  setBlockoutGlb: BlockoutGlbSetter
  clearAgentAssetWorkspaceQuality: (projectId: string) => void
  refreshActiveDesign: (projectId: string) => Promise<unknown>
  isCurrentAsset: (assetVersionId: string) => boolean
  getCurrentProjectId: () => string | null
  getCurrentAssetVersion: () => {
    projectId: string
    assetVersionId: string
    shapeProgram: Record<string, unknown> | null
  } | null
}

const SURFACE_ADORNMENT_ENABLE_FAILED_MESSAGE = '启用外观细节能力失败。'
const SURFACE_ADORNMENT_CONFIRM_UNKNOWN_ERROR_MESSAGE = '保留外观细节失败；当前版本没有变化。'

export function createSurfaceAdornmentAdapter(
  api: SurfaceAdornmentApi,
  callbacks: SurfaceAdornmentAdapterCallbacks,
): SurfaceAdornmentAdapter {
  const {
    clearAgentAssetWorkspaceQuality,
    getCurrentAssetVersion,
    getCurrentProjectId,
    isCurrentAsset,
    refreshActiveDesign,
    setAgentAssetChangeSet,
    setBlockoutGlb,
    setBlockoutShapeProgram,
  } = callbacks

  return {
    enable: async () => {
      try {
        await api.enableSurfaceAdornmentSkill(`surface-adornment-enable-${Date.now()}`)
        return { status: 'enabled' as const }
      } catch (caught) {
        return {
          status: 'failed' as const,
          message: caught instanceof Error ? caught.message : SURFACE_ADORNMENT_ENABLE_FAILED_MESSAGE,
        }
      }
    },
    preview: async (target, draft) => {
      return previewSurfaceAdornment(api,
        {
          isCurrentAsset,
          setBlockoutShapeProgram,
          setBlockoutGlb,
          setAgentAssetChangeSet,
        },
        target,
        draft,
      )
    },
    retain: async (changeSetId) => {
      try {
        const confirmed = await api.confirmAgentAssetChangeSet(
          changeSetId,
          `surface-adornment-confirm-${Date.now()}`,
        )
        const projectId = getCurrentProjectId()
        if (projectId) {
          clearAgentAssetWorkspaceQuality(projectId)
          await refreshActiveDesign(projectId)
        }
        setAgentAssetChangeSet(null)
        return {
          status: 'retained' as const,
          summary: `已保留外观细节并创建可编辑资产 v${confirmed.asset_version.version_no}。`,
        }
      } catch (caught) {
        return {
          status: 'failed' as const,
          message: caught instanceof Error ? caught.message : SURFACE_ADORNMENT_CONFIRM_UNKNOWN_ERROR_MESSAGE,
          errorCode: typeof caught === 'object' && caught !== null && 'code' in caught && typeof caught.code === 'string'
            ? caught.code
            : 'SURFACE_ADORNMENT_CONFIRM_REJECTED',
        }
      }
    },
    cancel: async (changeSetId) => {
      await api.rejectAgentAssetChangeSet(changeSetId, `surface-adornment-reject-${Date.now()}`)
      const currentAsset = getCurrentAssetVersion()
      if (currentAsset) {
        await restoreAgentAssetBlockoutPreview(
          api,
          setBlockoutShapeProgram,
          setBlockoutGlb,
          currentAsset.projectId,
          currentAsset.assetVersionId,
          currentAsset.shapeProgram,
        )
      }
      const projectId = getCurrentProjectId()
      if (projectId) {
        await refreshActiveDesign(projectId)
      }
    },
  }
}
