import { useCallback, useRef, type ChangeEvent } from 'react'

import type { SingleResultReadyDecision } from './singleResultDecisionPresentationState'
import type { AgentBlockoutGlbKind, AgentBlockoutGlbPayload } from './agentBlockoutDisplayState.js'
import type { ForgeApi } from '../../shared/api/forgeApi'
import type {
  AgentAssetChangeSet,
  AgentAssetVersion,
  AgentComponentCandidate,
  AgentPartEditOperation,
  AgentStructureSuggestion,
  AssemblyDeltaProgram,
  MechanicalConceptPlan,
  SegmentAgentBlockoutResponse,
} from '../../shared/types'
import {
  EDIT_NO_ASSET_NOTICE,
  EDIT_VERSION_MISMATCH_NOTICE,
  buildReplaceComponentOperation,
  buildSavedComponentDescription,
  buildSavedComponentDisplayName,
  buildSavedComponentSaveNotice,
  buildStructureSuggestionOperation,
} from './cadWorkbenchPanelEditOperations'
import { buildAgentPartEditOperations } from './cadWorkbenchPanelLogic.js'
import { buildInspectAgentAssetNote } from './cadWorkbenchPanelAgentAssetHelpers'
import { commitAgentBlockout as commitAgentBlockoutRequest } from './agentBlockoutCommitLoader'
import { confirmAgentAssetEdit as confirmAgentAssetEditRequest, rejectAgentAssetEdit as rejectAgentAssetEditRequest } from './agentAssetEditDecisionLoader'
import { confirmSingleResultPreview as confirmSingleResultPreviewRequest } from './agentSingleResultDecisionLoader'
import { previewAgentAssetEdit as previewAgentAssetEditRequest } from './agentAssetEditPreviewLoader'
import { previewAgentDirection as previewAgentDirectionRequest } from './agentDirectionPreviewLoader'
import { importAgentGlbReference as importAgentGlbReferenceRequest } from './agentGlbReferenceImportLoader'

type AgentAssetLifecycleApi = Pick<
  ForgeApi,
  | 'buildAgentBlockout'
  | 'segmentAgentBlockout'
  | 'commitAgentBlockout'
  | 'confirmSingleResultPreview'
  | 'proposeAgentAssetChangeSet'
  | 'previewAgentAssetChangeSet'
  | 'exportAgentAssetChangeSetPreviewGlb'
  | 'confirmAgentAssetChangeSet'
  | 'rejectAgentAssetChangeSet'
  | 'importAgentGlb'
  | 'loadAgentAssetPreviewGlb'
  | 'loadAgentAssetProductionGlb'
  | 'qualityAgentAssetVersion'
  | 'saveAgentComponent'
>

type DirectionProfile = 'quick_sketch' | 'showcase'

type UseCadWorkbenchPanelAgentAssetLifecycleActionsInput = {
  api: AgentAssetLifecycleApi
  conceptProjectId: string | null
  conceptAgentPlan: MechanicalConceptPlan | null
  presentationProfile: DirectionProfile
  activeDesignSnapshotEtag: string | null
  activeAgentAssetVersion: AgentAssetVersion | null
  agentBlockoutSegmentation: SegmentAgentBlockoutResponse | null
  agentAssetChangeSet: AgentAssetChangeSet | null
  selectedAgentPart: AgentAssetVersion['parts'][number] | null
  selectedPartRoleLabel: string
  clearAgentAssetWorkspaceQuality: (projectId: string | null) => void
  clearAgentEditAssistPresentation: () => void
  hydrateBlockoutDisplay: (projectId: string | null, data: {
    glbBase64: AgentBlockoutGlbPayload | null
    glbKind: AgentBlockoutGlbKind | null
    shapeProgram: null
    segmentation: SegmentAgentBlockoutResponse | null
  }) => number | null
  clearBlockoutDisplay: () => void
  refreshActiveDesign: (projectId: string) => Promise<unknown>
  startDirectionPreview: (projectId: string | null, directionId: string, variationIndex: number) => number
  receiveBlockoutBuild: (projectId: string | null, requestId: number, glbBase64: string, shapeProgram: Record<string, unknown>) => boolean
  receiveSegmentation: (projectId: string | null, requestId: number, segmentation: SegmentAgentBlockoutResponse) => boolean
  failSegmentation: (projectId: string | null, requestId: number) => boolean
  isCurrentDirectionPreview: (projectId: string | null, requestId: number) => boolean
  failDirectionPreview: (projectId: string | null, requestId: number) => boolean
  setBlockoutShapeProgram: (projectId: string | null, shapeProgram: Record<string, unknown> | null) => number | null
  setBlockoutGlb: (
    projectId: string | null,
    requestId: number,
    glbBase64: AgentBlockoutGlbPayload | null,
    glbKind: AgentBlockoutGlbKind | null,
  ) => boolean
  setAgentAssetChangeSet: (changeSet: AgentAssetChangeSet | null) => void
  setAgentCandidateSelectedPartId: (partId: string | null) => void
  setAssistantNote: (note: string) => void
  dispatchSingleResultDecision: (action: {
    type: 'request_cancelled'
    projectId: string | null
    requestId: number
  }) => void
  latestSingleResultRequestId: number
  errorText: (caught: unknown) => string
  refreshAgentEditAssist: () => void
}

type UseCadWorkbenchPanelAgentAssetLifecycleActionsOutput = {
  previewAgentDirection: (
    directionId: string,
    variationIndex?: number,
    requestedProfile?: DirectionProfile,
    planOverride?: MechanicalConceptPlan,
  ) => Promise<void>
  commitAgentBlockout: () => Promise<void> | undefined
  confirmSingleResultPreview: (decision: SingleResultReadyDecision) => Promise<void>
  previewAgentAssetEdit: (
    operation: AgentPartEditOperation | readonly AgentPartEditOperation[],
    summary: string,
  ) => Promise<void>
  previewAgentAssemblyDelta: (delta: AssemblyDeltaProgram) => Promise<void>
  saveSelectedAgentComponent: () => Promise<void>
  replaceWithAgentComponent: (candidate: AgentComponentCandidate) => Promise<void>
  previewStructureSuggestion: (suggestion: AgentStructureSuggestion) => Promise<void>
  confirmAgentAssetEdit: () => Promise<void>
  rejectAgentAssetEdit: () => Promise<void>
  inspectAgentAsset: () => Promise<void>
  importGlbReference: (event: ChangeEvent<HTMLInputElement>) => Promise<void>
}

export function useCadWorkbenchPanelAgentAssetLifecycleActions({
  api,
  conceptProjectId,
  conceptAgentPlan,
  presentationProfile,
  activeDesignSnapshotEtag,
  activeAgentAssetVersion,
  agentBlockoutSegmentation,
  agentAssetChangeSet,
  selectedAgentPart,
  selectedPartRoleLabel,
  clearAgentAssetWorkspaceQuality,
  clearAgentEditAssistPresentation,
  hydrateBlockoutDisplay,
  clearBlockoutDisplay,
  refreshActiveDesign,
  startDirectionPreview,
  receiveBlockoutBuild,
  receiveSegmentation,
  failSegmentation,
  isCurrentDirectionPreview,
  failDirectionPreview,
  setBlockoutShapeProgram,
  setBlockoutGlb,
  setAgentAssetChangeSet,
  setAgentCandidateSelectedPartId,
  setAssistantNote,
  dispatchSingleResultDecision,
  latestSingleResultRequestId,
  errorText,
  refreshAgentEditAssist,
}: UseCadWorkbenchPanelAgentAssetLifecycleActionsInput): UseCadWorkbenchPanelAgentAssetLifecycleActionsOutput {
  const agentAssetPreviewInFlightRef = useRef(false)

  const previewAgentDirection = useCallback((
    directionId: string,
    variationIndex = 0,
    requestedProfile = presentationProfile,
    planOverride?: MechanicalConceptPlan,
  ) => previewAgentDirectionRequest(
    api,
    {
      startDirectionPreview,
      receiveBlockoutBuild,
      clearAgentAssetWorkspace: clearBlockoutDisplay,
      resetDirectionDraftSelections: () => {
        setAgentAssetChangeSet(null)
        setAgentCandidateSelectedPartId(null)
      },
      receiveSegmentation,
      failSegmentation,
      isCurrentDirectionPreview,
      failDirectionPreview,
      setAssistantNote,
    },
    conceptProjectId,
    directionId,
    variationIndex,
    requestedProfile,
    planOverride ?? conceptAgentPlan ?? undefined,
  ), [
    api,
    clearBlockoutDisplay,
    conceptProjectId,
    conceptAgentPlan,
    failDirectionPreview,
    failSegmentation,
    isCurrentDirectionPreview,
    receiveBlockoutBuild,
    receiveSegmentation,
    setAgentAssetChangeSet,
    setAgentCandidateSelectedPartId,
    setAssistantNote,
    startDirectionPreview,
    presentationProfile,
  ])

  const commitAgentBlockout = useCallback(() => {
    if (!agentBlockoutSegmentation) return
    return commitAgentBlockoutRequest(api, {
      clearAgentEditAssistPresentation,
      refreshActiveDesign,
      setAgentAssetChangeSet,
      setAgentCandidateSelectedPartId,
      setAssistantNote,
    }, {
      projectId: conceptProjectId,
      segmentation: agentBlockoutSegmentation,
    })
  }, [
    agentBlockoutSegmentation,
    api,
    clearAgentEditAssistPresentation,
    conceptProjectId,
    refreshActiveDesign,
    setAgentAssetChangeSet,
    setAgentCandidateSelectedPartId,
    setAssistantNote,
  ])

  const confirmSingleResultPreview = useCallback(async (decision: SingleResultReadyDecision) => {
    if (conceptProjectId !== decision.project_id) {
      setAssistantNote('当前项目已切换；不会确认先前项目的临时结果。')
      return
    }
    setAssistantNote('正在把这次设计保存为可编辑资产…')
    await confirmSingleResultPreviewRequest(
      api,
      {
        clearAgentEditAssistPresentation,
        dispatchSingleResultDecision,
        errorText,
        latestRequestId: latestSingleResultRequestId,
        refreshActiveDesign,
        setAgentAssetChangeSet,
        setAgentCandidateSelectedPartId,
        setAssistantNote,
      },
      decision,
    )
  }, [
    api,
    clearAgentEditAssistPresentation,
    conceptProjectId,
    dispatchSingleResultDecision,
    errorText,
    refreshActiveDesign,
    latestSingleResultRequestId,
    setAgentAssetChangeSet,
    setAgentCandidateSelectedPartId,
    setAssistantNote,
  ])

  const previewAgentAssetEdit = useCallback(async (operation: AgentPartEditOperation | readonly AgentPartEditOperation[], summary: string) => {
    if (!activeAgentAssetVersion || agentAssetPreviewInFlightRef.current) return
    const projectId = conceptProjectId
    agentAssetPreviewInFlightRef.current = true
    setAssistantNote('正在预览部件修改…')
    try {
      await previewAgentAssetEditRequest(api, {
        setBlockoutGlb,
        setBlockoutShapeProgram,
        setAgentAssetChangeSet,
        setAssistantNote,
      }, {
        projectId,
        assetVersionId: activeAgentAssetVersion.asset_version_id,
        shapeProgram: activeAgentAssetVersion.shape_program,
        summary,
        operation,
      })
    } finally {
      agentAssetPreviewInFlightRef.current = false
    }
  }, [
    activeAgentAssetVersion,
    api,
    conceptProjectId,
    setAssistantNote,
    setAgentAssetChangeSet,
    setBlockoutGlb,
    setBlockoutShapeProgram,
  ])

  const previewAgentAssemblyDelta = useCallback(async (delta: AssemblyDeltaProgram) => {
    if (!activeAgentAssetVersion) {
      setAssistantNote(EDIT_NO_ASSET_NOTICE)
      return
    }
    if (activeAgentAssetVersion.asset_version_id !== delta.base_asset_version_id) {
      setAssistantNote(EDIT_VERSION_MISMATCH_NOTICE)
      return
    }
    const operations = buildAgentPartEditOperations(delta)
    await previewAgentAssetEdit(operations, delta.summary)
  }, [
    activeAgentAssetVersion,
    previewAgentAssetEdit,
    setAssistantNote,
  ])

  const saveSelectedAgentComponent = useCallback(async () => {
    if (!activeAgentAssetVersion || !selectedAgentPart) return
    try {
      const component = await api.saveAgentComponent(activeAgentAssetVersion.asset_version_id, {
        client_request_id: `agent-component-${Date.now()}`,
        part_id: selectedAgentPart.part_id,
        display_name: buildSavedComponentDisplayName(selectedPartRoleLabel),
        description: buildSavedComponentDescription(activeAgentAssetVersion.version_no),
      })
      await refreshAgentEditAssist()
      setAssistantNote(buildSavedComponentSaveNotice(component.display_name))
    } catch {
      setAssistantNote('保存可复用部件失败；当前资产版本没有变化。')
    }
  }, [
    activeAgentAssetVersion,
    api,
    refreshAgentEditAssist,
    selectedAgentPart,
    selectedPartRoleLabel,
    setAssistantNote,
  ])

  const replaceWithAgentComponent = useCallback(async (candidate: AgentComponentCandidate) => {
    if (!selectedAgentPart) return
    await previewAgentAssetEdit(
      buildReplaceComponentOperation(selectedAgentPart.part_id, candidate),
      `替换为「${candidate.component.display_name}」`,
    )
  }, [selectedAgentPart, previewAgentAssetEdit])

  const previewStructureSuggestion = useCallback(async (suggestion: AgentStructureSuggestion) => {
    await previewAgentAssetEdit(
      buildStructureSuggestionOperation(suggestion),
      suggestion.summary,
    )
  }, [previewAgentAssetEdit])

  const confirmAgentAssetEdit = useCallback(async () => {
    await confirmAgentAssetEditRequest(
      api,
      {
        clearAgentAssetWorkspaceQuality,
        refreshActiveDesign,
        setBlockoutShapeProgram,
        setAgentAssetChangeSet,
        setAssistantNote,
      },
      { changeSet: agentAssetChangeSet },
    )
  }, [
    api,
    agentAssetChangeSet,
    clearAgentAssetWorkspaceQuality,
    refreshActiveDesign,
    setAgentAssetChangeSet,
    setAssistantNote,
    setBlockoutShapeProgram,
  ])

  const rejectAgentAssetEdit = useCallback(async () => {
    await rejectAgentAssetEditRequest(
      api,
      {
        refreshActiveDesign,
        setAgentAssetChangeSet,
        setAssistantNote,
      },
      { changeSet: agentAssetChangeSet },
    )
  }, [
    api,
    agentAssetChangeSet,
    refreshActiveDesign,
    setAgentAssetChangeSet,
    setAssistantNote,
  ])

  const inspectAgentAsset = useCallback(async () => {
    if (!activeAgentAssetVersion) {
      setAssistantNote('请先同步一个活动 Agent 资产，再运行检查。')
      return
    }
    if (!activeDesignSnapshotEtag) {
      setAssistantNote('当前工作台版本尚未同步完成；请稍后再检查模型。')
      return
    }
    setAssistantNote('正在检查当前 Agent 资产…')
    try {
      const report = await api.qualityAgentAssetVersion(activeAgentAssetVersion.asset_version_id, {
        idempotencyKey: `agent-asset-quality-${Date.now()}`,
        ifMatch: activeDesignSnapshotEtag,
      })
      clearAgentAssetWorkspaceQuality(activeAgentAssetVersion.project_id)
      if (activeAgentAssetVersion.project_id) await refreshActiveDesign(activeAgentAssetVersion.project_id)
      setAssistantNote(buildInspectAgentAssetNote(report))
    } catch {
      setAssistantNote('模型检查失败；当前资产版本没有变化。')
    }
  }, [
    activeAgentAssetVersion,
    activeDesignSnapshotEtag,
    api,
    clearAgentAssetWorkspaceQuality,
    refreshActiveDesign,
    setAssistantNote,
  ])

  const importGlbReference = useCallback(async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    event.target.value = ''
    if (!file) return
    if (!conceptProjectId) {
      setAssistantNote('请先创建或打开一个设计项目，再导入 GLB。')
      return
    }
    setAssistantNote(`正在检查并导入「${file.name}」…`)
    try {
      const result = await importAgentGlbReferenceRequest(
        api,
        {
          setAgentAssetChangeSet,
          setAgentCandidateSelectedPartId,
          clearAgentAssetWorkspaceQuality,
          hydrateBlockoutDisplay,
          refreshActiveDesign,
        },
        {
          projectId: conceptProjectId,
          file,
          domainPackId: conceptAgentPlan?.domain_pack_id ?? null,
        },
      )
      setAssistantNote(`已导入参考模型：${result.triangleCount.toLocaleString()} 三角形、${result.materialCount} 个材质。它不会被伪装成可编辑模型；可让 Agent 依据它重建。`)
    } catch (caught) {
      const message = caught instanceof Error && caught.message
        ? caught.message
        : errorText(caught)
      setAssistantNote(`GLB 导入失败：${message}`)
    }
  }, [
    api,
    clearAgentAssetWorkspaceQuality,
    conceptAgentPlan?.domain_pack_id,
    conceptProjectId,
    errorText,
    hydrateBlockoutDisplay,
    refreshActiveDesign,
    setAgentAssetChangeSet,
    setAgentCandidateSelectedPartId,
    setAssistantNote,
  ])

  return {
    previewAgentDirection,
    commitAgentBlockout,
    confirmSingleResultPreview,
    previewAgentAssetEdit,
    previewAgentAssemblyDelta,
    saveSelectedAgentComponent,
    replaceWithAgentComponent,
    previewStructureSuggestion,
    confirmAgentAssetEdit,
    rejectAgentAssetEdit,
    inspectAgentAsset,
    importGlbReference,
  }
}
