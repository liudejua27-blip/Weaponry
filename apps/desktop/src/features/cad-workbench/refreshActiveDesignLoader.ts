import type { ActiveDesignApiResponse, ActiveDesignErrorState, ForgeApi } from '../../shared/api/forgeApi'
import type { ActiveDesignSnapshot, SegmentAgentBlockoutResponse } from '../../shared/types'
import { loadAgentAssetDisplayViews } from './agentBlockoutDisplayLoader.js'
import type { ActiveDesignOperation } from './activeDesignMachine.js'
import { type AgentBlockoutGlbKind, type AgentBlockoutGlbPayload } from './agentBlockoutDisplayState'

type ActiveDesignRefreshApi = Pick<
  ForgeApi,
  'getActiveDesign' |
  'getAgentAssetVersion' |
  'getAgentQualityReport' |
  'getActiveDesignNavigation' |
  'loadAgentAssetPreviewGlb' |
  'loadAgentAssetProductionGlb'
>

type ActiveDesignGetResponse = Awaited<ReturnType<ActiveDesignRefreshApi['getActiveDesign']>>
type ActiveDesign = ActiveDesignGetResponse['data']
type AgentAssetVersion = Awaited<ReturnType<ActiveDesignRefreshApi['getAgentAssetVersion']>>
type ActiveDesignNavigation = Awaited<ReturnType<ActiveDesignRefreshApi['getActiveDesignNavigation']>>
type QualityReport = Awaited<ReturnType<ActiveDesignRefreshApi['getAgentQualityReport']>>
type CameraView = 'iso' | 'front' | 'top' | 'right'
type LightPreset = 'cad_neutral' | 'soft_studio' | 'concept_contrast'

type RefreshBlockoutDisplay = {
  projectId: string
  requestId: number
  isCurrentActiveDesignRequest: () => boolean
  setAssistantNote: (note: string) => void
  setBlockoutGlb: (projectId: string | null, requestId: number, glbBase64: AgentBlockoutGlbPayload | null, glbKind: AgentBlockoutGlbKind | null) => boolean
}

export type RefreshActiveDesignCallbacks = {
  startActiveDesignRequest: (operation: Exclude<ActiveDesignOperation, 'idle'>) => number
  isCurrentActiveDesignRequest: (requestId: number) => boolean
  receiveActiveDesignSnapshot: (projectId: string, requestId: number, response: ActiveDesignApiResponse<ActiveDesignSnapshot>) => boolean
  failActiveDesignRequest: (requestId: number, caught: unknown) => ActiveDesignErrorState | null
  setCameraView: (cameraView: CameraView) => void
  setLightPreset: (lightPreset: LightPreset) => void
  clearAgentAssetWorkspace: () => void
  clearAgentEditAssistPresentation: () => void
  setAgentCandidateSelectedPartId: (partId: string | null) => void
  startAgentAssetWorkspaceHydration: (projectId: string, assetVersionId: string, selectedPartId: string | null) => number
  receiveAgentAssetWorkspaceAsset: (projectId: string, requestId: number, asset: AgentAssetVersion) => boolean
  receiveAgentAssetWorkspaceNavigation: (projectId: string, requestId: number, navigation: ActiveDesignNavigation | null) => void
  receiveAgentAssetWorkspaceQuality: (projectId: string, requestId: number, report: QualityReport | null) => boolean
  clearAgentAssetWorkspaceQuality: (projectId: string | null) => void
  hydrateBlockoutDisplay: (projectId: string, data: {
    glbKind: null
    shapeProgram: AgentAssetVersion['shape_program'] | null
    segmentation: SegmentAgentBlockoutResponse | null
  }) => number | null
  setBlockoutGlb: RefreshBlockoutDisplay['setBlockoutGlb']
  setAssistantNote: (message: string) => void
  activeDesignSelectedPartId: (snapshot: ActiveDesign | null) => string | null
}

export async function refreshActiveDesign(
  api: ActiveDesignRefreshApi,
  callbacks: RefreshActiveDesignCallbacks,
  projectId: string,
): Promise<ActiveDesign | null> {
  const {
    startActiveDesignRequest,
    isCurrentActiveDesignRequest,
    receiveActiveDesignSnapshot,
    failActiveDesignRequest,
    setCameraView,
    setLightPreset,
    clearAgentAssetWorkspace,
    clearAgentEditAssistPresentation,
    setAgentCandidateSelectedPartId,
    startAgentAssetWorkspaceHydration,
    receiveAgentAssetWorkspaceAsset,
    receiveAgentAssetWorkspaceNavigation,
    receiveAgentAssetWorkspaceQuality,
    clearAgentAssetWorkspaceQuality,
    hydrateBlockoutDisplay,
    setBlockoutGlb,
    setAssistantNote,
    activeDesignSelectedPartId,
  } = callbacks

  const requestId = startActiveDesignRequest('loading')

  try {
    const response = await api.getActiveDesign(projectId)
    if (!receiveActiveDesignSnapshot(projectId, requestId, response)) return null

    if (response.data.render_preset) {
      setCameraView(response.data.render_preset.camera_view ?? 'iso')
      setLightPreset(response.data.render_preset.light_preset ?? 'cad_neutral')
    } else if (response.data.active_design.source !== 'agent_asset') {
      setCameraView('iso')
      setLightPreset('cad_neutral')
    }

    if (response.data.active_design.source !== 'agent_asset') {
      clearAgentAssetWorkspace()
      clearAgentEditAssistPresentation()
      setAgentCandidateSelectedPartId(null)
      return response.data
    }

    const workspaceRequestId = startAgentAssetWorkspaceHydration(
      projectId,
      response.data.active_design.asset_version_id,
      activeDesignSelectedPartId(response.data),
    )
    const version = await api.getAgentAssetVersion(response.data.active_design.asset_version_id)
    if (!isCurrentActiveDesignRequest(requestId) || !receiveAgentAssetWorkspaceAsset(projectId, workspaceRequestId, version)) return null

    void api.getActiveDesignNavigation(response.data.project_id)
      .then((navigation) => { receiveAgentAssetWorkspaceNavigation(projectId, workspaceRequestId, navigation) })
      .catch(() => { receiveAgentAssetWorkspaceNavigation(projectId, workspaceRequestId, null) })

    if (response.data.quality?.asset_version_id === version.asset_version_id) {
      try {
        const report = await api.getAgentQualityReport(response.data.quality.quality_report_id)
        receiveAgentAssetWorkspaceQuality(projectId, workspaceRequestId, report)
      } catch {
        receiveAgentAssetWorkspaceQuality(projectId, workspaceRequestId, null)
      }
    } else {
      clearAgentAssetWorkspaceQuality(projectId)
    }

    const isImportedReference = version.shape_program?.schema_version === 'ExternalGLBReference@1'
    const blockoutDisplayRequestId = hydrateBlockoutDisplay(projectId, {
      glbKind: null,
      shapeProgram: isImportedReference ? null : version.shape_program,
      segmentation: {
        artifact_id: version.artifact_id,
        plan_id: version.plan_id,
        direction_id: version.direction_id,
        domain_pack_id: version.domain_pack_id,
        segmentation_status: 'candidate',
        parts: version.parts,
        assembly_graph: version.assembly_graph,
      },
    })
    if (blockoutDisplayRequestId !== null) {
      void loadAgentAssetDisplayViews(
        api,
        {
          projectId,
          requestId,
          isCurrentActiveDesignRequest: () => isCurrentActiveDesignRequest(requestId),
          isCurrentDisplayRequest: () => isCurrentActiveDesignRequest(requestId),
          setAssistantNote,
          setBlockoutGlb,
        },
        version.asset_version_id,
        blockoutDisplayRequestId,
        isImportedReference,
      )
    }

    return response.data
  } catch (caught) {
    const error = failActiveDesignRequest(requestId, caught)
    if (!error) return null
    if (error.kind !== 'not_found') {
      setAssistantNote(error.message)
    }
    return null
  }
}
