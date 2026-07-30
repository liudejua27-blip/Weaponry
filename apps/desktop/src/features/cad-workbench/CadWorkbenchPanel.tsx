import {
  useCallback,
  useEffect,
  useReducer,
  useRef,
  useState,
  type ChangeEvent,
  useMemo,
} from 'react'
import {
  ArrowsOutCardinal,
  ArrowLeft,
  Cube,
  Sparkle,
  X,
} from '@phosphor-icons/react'
import type {
  AgentAssetChangeSet,
} from '../../shared/types'
import { useRuntime } from '../../app/providers/RuntimeProvider'
import { ModuleGraphViewport } from './ModuleGraphViewport'
import { AgentConversation } from './AgentConversation'
import { CadWorkbenchPanelResultCards } from './CadWorkbenchPanelResultCards'
import { CadWorkbenchPanelSelectionTools } from './CadWorkbenchPanelSelectionTools'
import {
  initialSingleResultDecisionPresentationState,
  singleResultDecisionPresentationReducer,
} from './singleResultDecisionPresentationState'
import { WorkbenchComposer } from './WorkbenchComposer'
import { WorkbenchSidebar } from './WorkbenchSidebar'
import { CadWorkbenchPanelBeginnerGuide } from './CadWorkbenchPanelBeginnerGuide'
import { selectAgentBlockoutPreviewPresentation } from './agentBlockoutPreviewPresentation'
import { selectAgentPlanSourcePresentation } from './agentPlanSourcePresentation'
import { useCadWorkbenchPanelCandidatePreviewQuality } from './useCadWorkbenchPanelCandidatePreviewQuality'
import { useCadWorkbenchPanelCandidatePreviewQualityPresentation } from './useCadWorkbenchPanelCandidatePreviewQualityPresentation'
import { useCadWorkbenchPanelAgentAssetLifecycleActions } from './useCadWorkbenchPanelAgentAssetLifecycleActions'
import { WorkbenchDrawerStack } from './WorkbenchDrawerStack'
import { WorkbenchInspectorRail } from './WorkbenchInspectorRail'
import {
  activeDesignCanSelectParts,
  activeDesignSelectedMaterialZoneId,
  activeDesignSelectedPartId,
} from './activeDesignMachine'
import { useWorkbenchLifecycle } from './useWorkbenchLifecycle'
import { CadWorkbenchPanelGlobalActions } from './cadWorkbenchPanelGlobalActions'
import {
  claimAgentTurnSubmission,
  parseCandidatePbrCapturePending,
  parseAgentTurnPresentation,
  releaseAgentTurnSubmission,
} from './agentConversationState'
import {
  authorizeCandidatePbrVisualComparison,
  captureAndSubmitCandidatePbr,
  issueCandidatePbrCapture,
  resumeCandidatePbrCapture,
} from './candidatePbrCaptureBridge'
import { useAgentConversationPresentation } from './useAgentConversationPresentation'
import { useAgentBlockoutDisplay } from './useAgentBlockoutDisplay'
import { useCadWorkbenchPanelEditAssistLoader } from './useCadWorkbenchPanelEditAssistLoader'
import { useAgentAssetWorkspace } from './useAgentAssetWorkspace'
import { getLegacyCompatibilityDisplay } from './legacyCompatibilityDisplay'
import { useViewportDisplayPreferences } from './useViewportDisplayPreferences'
import { useLegacyModuleGraphWorkspace } from './useLegacyModuleGraphWorkspace'
import { useLegacyModuleGraphOverlay } from './useLegacyModuleGraphOverlay'
import { useAgentRenderPresentation } from './useAgentRenderPresentation'
import { useAgentEditAssistPresentation } from './useAgentEditAssistPresentation'
import { useAgentMaterialCatalogPresentation } from './useAgentMaterialCatalogPresentation'
import { useAgentMaterialFilterPresentation } from './useAgentMaterialFilterPresentation'
import { useAgentMaterialPreselectionPresentation } from './useAgentMaterialPreselectionPresentation'
import { SurfaceAdornmentDrawer } from './SurfaceAdornmentDrawer'
import { errorText } from './cadWorkbenchPanelLogic.js'
import { ReferenceEvidenceDrawer } from './ReferenceEvidenceDrawer'
import { useComponentCatalogPresentation } from './useComponentCatalogPresentation'
import { useConceptWorkbench } from './useConceptWorkbench'
import { bindDrawerFocusTrap } from './drawerFocusManagement'
import { useCadWorkbenchPanelViewportActions } from './useCadWorkbenchPanelViewportActions'
import { useCadWorkbenchPanelActiveDesignSync } from './useCadWorkbenchPanelActiveDesignSync'
import { useCadWorkbenchPanelActiveDesignPartActions } from './useCadWorkbenchPanelActiveDesignPartActions'
import { useCadWorkbenchPanelConversationThreadActions } from './useCadWorkbenchPanelConversationThreadActions'
import { useCadWorkbenchPanelAssistantActions } from './useCadWorkbenchPanelAssistantActions'
import { useCadWorkbenchPanelRecordAgentTurn } from './useCadWorkbenchPanelRecordAgentTurn'
import { useCadWorkbenchPanelNavigateAgentAsset } from './useCadWorkbenchPanelNavigateAgentAsset'
import {
  initialViewportDockPresentationState,
  viewportDockPresentationReducer,
} from './viewportDockState'
import {
  DEFAULT_AGENT_CLARIFICATION_OPTIONS,
  CONCEPT_FAMILY_SUGGESTIONS,
} from './cadWorkbenchPanelPrompts'
import { COMPOSER_DEFAULT_BEGINNER_PROMPT } from './workbenchComposerPrompts.js'
import {
  type CameraView,
  type LightPreset,
  VIEWPORT_TOOLBAR_ITEMS,
} from './cadWorkbenchPanelTools'
import { useCadWorkbenchPanelConversationState } from './cadWorkbenchPanelConversationState'
import { useCadWorkbenchPanelAdapters } from './useCadWorkbenchPanelAdapters'
import { useCadWorkbenchPanelMaterialState } from './useCadWorkbenchPanelMaterialState'
import { useCadWorkbenchPanelMaterialPresetActions } from './useCadWorkbenchPanelMaterialPresetActions'
import { useCadWorkbenchPanelGlobalActions } from './useCadWorkbenchPanelGlobalActions'
import { useCadWorkbenchPanelActiveDesignSyncCallbacks } from './useCadWorkbenchPanelActiveDesignSyncCallbacks'
import { useCadWorkbenchPanelLegacyGraphWorkspaceSync } from './useCadWorkbenchPanelLegacyGraphWorkspaceSync'
import { useCadWorkbenchPanelGuideMode } from './useCadWorkbenchPanelGuideMode'
import { useCadWorkbenchPanelMaterialCatalogAndFilterSync } from './useCadWorkbenchPanelMaterialCatalogAndFilterSync'
import { useCadWorkbenchPanelProjectLifecycleSync } from './useCadWorkbenchPanelProjectLifecycleSync'
import { useCadWorkbenchPanelProjectResetState } from './useCadWorkbenchPanelProjectResetState'
import { useCadWorkbenchPanelCompatibilitySummary } from './useCadWorkbenchPanelCompatibilitySummary'
import { useCadWorkbenchPanelReferenceEvidenceVisionContext } from './useCadWorkbenchPanelReferenceEvidenceVisionContext'
import { useCadWorkbenchPanelLegacyGraphSelection } from './useCadWorkbenchPanelLegacyGraphSelection'
import { useCadWorkbenchPanelAgentThreads } from './useCadWorkbenchPanelAgentThreads'
import { useCadWorkbenchPanelPartSelectionState } from './useCadWorkbenchPanelPartSelectionState'
import { useCadWorkbenchPanelViewportState } from './useCadWorkbenchPanelViewportState'
import { CadWorkbenchPanelViewportOverlays } from './cadWorkbenchPanelViewportOverlays'
import { CadWorkbenchPanelStatusBar } from './CadWorkbenchPanelStatusBar'
import { useCadWorkbenchPanelViewportMeasurements } from './cadWorkbenchPanelViewportMeasurements'
import { useCadWorkbenchPanelProviderConfig } from './useCadWorkbenchPanelProviderConfig'
import { useCadWorkbenchPanelSelectionToolsPresentation } from './useCadWorkbenchPanelSelectionToolsPresentation'
import './cad-workbench.css'
type ReferenceViewportState = {
  projectId: string
  evidenceId: string
  sourceObjectSha256: string
  referenceClass: 'single_image' | 'multi_view_contact_sheet' | 'strict_glb_readback'
  kind: 'glb'
  glb: ArrayBuffer
} | {
  projectId: string
  evidenceId: string
  sourceObjectSha256: string
  referenceClass: 'single_image' | 'multi_view_contact_sheet'
  kind: 'image'
  imageUrl: string
}

const DOMAIN_TYPE_BY_PACK: Record<string, string> = {
  pack_future_weapon_prop: 'future_weapon_prop',
  pack_vehicle_concept: 'vehicle_concept',
  pack_aircraft_concept: 'aircraft_concept',
  pack_robotic_arm_concept: 'robotic_arm_concept',
}
const EMPTY_AGENT_KERNEL_ITEMS: readonly never[] = []

async function waitForCandidatePbrViewport(viewport: HTMLElement, expectedGlbSha256: string): Promise<void> {
  const deadline = Date.now() + 15_000
  while (Date.now() < deadline) {
    if (
      viewport.dataset.blockoutLoadState === 'ready'
      && viewport.dataset.blockoutRenderSource === 'glb_pbr'
      && viewport.dataset.blockoutGlbSha256 === expectedGlbSha256
    ) return
    await new Promise<void>((resolve) => window.setTimeout(resolve, 25))
  }
  throw new Error('CANDIDATE_PBR_CAPTURE_VIEWPORT_NOT_READY')
}

/**
 * R007B intentionally has one reviewed production-arm prerequisite.  Keep the
 * Rust conflict actionable for a zero-basis user, while preserving every other
 * backend error verbatim for diagnosis.
 */

export function CadWorkbenchPanel() {
  const concept = useConceptWorkbench()
  const { api, checkService } = useRuntime()
  const [singleResultDecisionPresentation, dispatchSingleResultDecision] = useReducer(
    singleResultDecisionPresentationReducer,
    initialSingleResultDecisionPresentationState,
  )
  const {
    activeDesignState,
    openProject,
    startActiveDesignRequest,
    isCurrentActiveDesignRequest,
    receiveActiveDesignSnapshot,
    failActiveDesignRequest,
    drawerFocusRef,
    exportOpen,
    qualityOpen,
    hasOpenDrawer,
    openDrawer,
    closeDrawers,
  } = useWorkbenchLifecycle()
  const {
    agentConversationState,
    openConversationProject,
    startAgentConversationRequest,
    isCurrentAgentConversationRequest,
    receiveAgentTurn,
    receiveAgentClarification,
    markAgentKernelUnavailable,
    setChatInput,
    setAssistantMode,
    setAssistantNote,
  } = useAgentConversationPresentation()
  const {
    chatInput,
    assistantMode,
    assistantNote,
    agentThreadId,
    agentKernelItems,
    agentKernelUnavailable,
    agentClarification,
    agentPlan,
    latestRequestId: latestAgentRequestId,
  } = agentConversationState
  const hasAgentPlan = agentPlan !== null
  const agentTurnSubmissionRef = useRef(false)
  const pbrCaptureViewportRef = useRef<HTMLDivElement | null>(null)
  const activePbrCaptureRef = useRef<string | null>(null)
  const resumePbrCaptureAfterAuthorizationRef = useRef<string | null>(null)
  const [candidatePbrAuthorization, setCandidatePbrAuthorization] = useState<{
    executionId: string
    projectId: string
    turnId: string
  } | null>(null)
  const [candidatePbrAuthorizationAttempt, setCandidatePbrAuthorizationAttempt] = useState(0)
  const [candidatePbrCaptureStatus, setCandidatePbrCaptureStatus] = useState<'idle' | 'capturing' | 'authorizing' | 'authorization_required' | 'repair_required' | 'preview_ready' | 'failed'>('idle')
  const {
    agentBlockoutDisplay,
    openBlockoutProject,
    startDirectionPreview,
    isCurrentDirectionPreview,
    receiveBlockoutBuild,
    receiveSegmentation,
    failSegmentation,
    failDirectionPreview,
    hydrateBlockoutDisplay,
    setBlockoutGlb,
    setBlockoutShapeProgram,
    clearBlockoutDisplay,
  } = useAgentBlockoutDisplay()
  const onPbrCaptureViewportChange = useCallback((viewport: HTMLDivElement | null) => {
    pbrCaptureViewportRef.current = viewport
  }, [])
  const {
    agentAssetWorkspace,
    openAgentAssetWorkspaceProject,
    startAgentAssetWorkspaceHydration,
    receiveAgentAssetWorkspaceAsset,
    projectAgentAssetWorkspaceSelection,
    receiveAgentAssetWorkspaceQuality,
    receiveAgentAssetWorkspaceNavigation,
    clearAgentAssetWorkspaceQuality,
    clearAgentAssetWorkspace,
  } = useAgentAssetWorkspace()
  const {
    viewportDisplayPreferences,
    openViewportDisplayPreferences,
    setViewportTool,
    setViewportExplodeFactor,
  } = useViewportDisplayPreferences()
  const {
    legacyModuleGraphWorkspace,
    legacyModuleGraphWorkspacePreferenceKey,
    openLegacyModuleGraphWorkspace,
    selectLegacyModuleGraphNode,
    reconcileLegacyModuleGraphSelection,
  } = useLegacyModuleGraphWorkspace()
  const {
    legacyModuleGraphOverlay,
    legacyModuleGraphOverlayContextKey,
    openLegacyModuleGraphOverlay,
    reconcileLegacyModuleGraphOverlayNodes,
  } = useLegacyModuleGraphOverlay()
  const {
    agentRenderPresentation,
    openAgentRenderPresentation,
    startAgentRenderRequest,
    receiveAgentRenderSet,
    failAgentRenderRequest,
    startAgentRenderPackageRequest,
    finishAgentRenderPackageRequest,
    closeAgentRenderPresentation,
  } = useAgentRenderPresentation()
  const {
    agentEditAssistPresentation,
    openAgentEditAssistPresentation,
    startAgentEditAssistRead,
    receiveAgentEditAssistRead,
    failAgentEditAssistRead,
    clearAgentEditAssistPresentation,
  } = useAgentEditAssistPresentation()
  const {
    agentMaterialCatalogPresentation,
    openAgentMaterialCatalogPresentation,
    startAgentMaterialCatalogRead,
    receiveAgentMaterialCatalog,
    failAgentMaterialCatalog,
  } = useAgentMaterialCatalogPresentation()
  const {
    agentMaterialFilterPresentation,
    openAgentMaterialFilterPresentation,
    setMaterialFilterQuery,
    setMaterialFilterCategory,
    setMaterialFilterCompatibilityOnly,
  } = useAgentMaterialFilterPresentation()
  const {
    agentMaterialPreselectionPresentation,
    openAgentMaterialPreselectionPresentation,
    selectMaterialPreselection,
  } = useAgentMaterialPreselectionPresentation()
  const { componentCatalogPresentation, openComponentCatalog, startComponentCatalogRead, receiveComponentCatalog, failComponentCatalog } = useComponentCatalogPresentation()
  const {
    glbBase64: agentBlockoutGlbBase64,
    glbKind: agentBlockoutGlbKind,
    shapeProgram: agentBlockoutShapeProgram,
    segmentation: agentBlockoutSegmentation,
  } = agentBlockoutDisplay
  const [showComposerAdvancedActions, setShowComposerAdvancedActions] = useState(false)
  const blockoutPreviewPresentation = selectAgentBlockoutPreviewPresentation(agentBlockoutDisplay)
  const candidatePreviewPresent = showComposerAdvancedActions
    ? Boolean(agentBlockoutGlbBase64 || agentBlockoutShapeProgram)
    : false
  const candidateKernelItemsForQuality = showComposerAdvancedActions
    ? agentKernelItems
    : EMPTY_AGENT_KERNEL_ITEMS
  const candidatePreviewQuality = useCadWorkbenchPanelCandidatePreviewQuality({
    candidatePreviewPresent,
    agentKernelItems: candidateKernelItemsForQuality,
  })
  const candidatePreviewQualityPresentation = useCadWorkbenchPanelCandidatePreviewQualityPresentation({
    candidatePreviewPresent,
    quality: candidatePreviewQuality,
  })
  useEffect(() => {
    const pending = parseCandidatePbrCapturePending(agentKernelItems)
    const projectId = concept.project?.project_id ?? null
    if (!pending || pending.projectId !== projectId) return
    const viewport = pbrCaptureViewportRef.current
    if (!viewport) return
    const captureKey = `${pending.executionId}:${pending.projectId}:${pending.turnId}`
    if (activePbrCaptureRef.current === captureKey) return
    activePbrCaptureRef.current = captureKey
    let cancelled = false

    const run = async () => {
      try {
        setCandidatePbrCaptureStatus('capturing')
        setAssistantNote('候选模型正在装入当前工作台，并由同一个 PBR 渲染器生成八视图验收证据。')
        let resumed: Awaited<ReturnType<typeof resumeCandidatePbrCapture>> | null = null
        let resumeExistingCapture = resumePbrCaptureAfterAuthorizationRef.current === captureKey
        if (resumeExistingCapture) resumePbrCaptureAfterAuthorizationRef.current = null
        // A successful typed patch has a new GLB hash. It must obtain a fresh
        // renderer receipt; two capture rounds are the hard upper bound for
        // one author plus one patch.
        for (let captureRound = 0; captureRound < 2; captureRound += 1) {
          if (!resumeExistingCapture) {
            const issue = await issueCandidatePbrCapture({
              executionId: pending.executionId,
              projectId: pending.projectId,
              turnId: pending.turnId,
            })
            if (cancelled) return
            hydrateBlockoutDisplay(projectId, {
              glbBase64: issue.glbBase64,
              glbKind: issue.artifactProfileId === 'production_concept'
                ? 'compiled_agent_production_pbr'
                : 'compiled_agent_preview_pbr',
              shapeProgram: null,
              segmentation: null,
            })
            await waitForCandidatePbrViewport(viewport, issue.candidateGlbSha256)
            if (cancelled) return
            await captureAndSubmitCandidatePbr({ viewport, issue })
            if (cancelled) return
          }
          resumed = await resumeCandidatePbrCapture({
            executionId: pending.executionId,
            projectId: pending.projectId,
            turnId: pending.turnId,
          })
          if (cancelled) return
          if (resumed.status === 'authorization_required') {
            setCandidatePbrCaptureStatus('authorization_required')
            setCandidatePbrAuthorization({
              executionId: pending.executionId,
              projectId: pending.projectId,
              turnId: pending.turnId,
            })
            setAssistantNote('候选已完成同源 PBR 八视图采集。请明确授权一次千问参考图比较；未授权不会联网、不会生成预览或版本。')
            return
          }
          if (resumed.status !== 'capture_required') break
          resumeExistingCapture = false
          setAssistantNote('已完成唯一局部修复；正在对新候选进行第二次同源 PBR 八视图验收。')
        }
        if (!resumed || resumed.status === 'capture_required') {
          throw new Error('CANDIDATE_PBR_CAPTURE_ROUND_LIMIT_REACHED')
        }
        if (resumed.status === 'repair_required') {
          setCandidatePbrCaptureStatus('repair_required')
          dispatchSingleResultDecision({
            type: 'request_failed',
            projectId,
            requestId: latestAgentRequestId,
            error: '候选模型未通过同源 PBR 视觉验收；没有创建预览、版本或导出。',
          })
          const targetCount = Array.isArray(resumed.visualRepairTargetProjection?.targets)
            ? resumed.visualRepairTargetProjection.targets.length
            : 0
          setAssistantNote(targetCount > 0
            ? `候选模型未通过同源 PBR 视觉验收；Rust 已封存 ${targetCount} 个局部修复目标，尚未执行 patch，当前已确认模型保持不变。`
            : '候选模型未通过同源 PBR 视觉验收；不存在安全的局部修复目标，当前已确认模型保持不变。')
          return
        }
        const decision = resumed.singleResultDecision
        if (!decision || decision.state !== 'ready_for_preview' || decision.project_id !== projectId || decision.turn_id !== pending.turnId) {
          throw new Error('CANDIDATE_PBR_CAPTURE_FORMAL_DECISION_INVALID')
        }
        const preview = await api.loadSingleResultPreviewGlb({
          projectId: decision.project_id,
          turnId: decision.turn_id,
          previewId: decision.preview.preview_id,
          artifactSha256: decision.preview.artifact_sha256,
          artifactProfileId: decision.preview.artifact_profile_id,
        })
        if (cancelled) return
        hydrateBlockoutDisplay(projectId, {
          glbBase64: preview.glb,
          glbKind: preview.artifactProfileId === 'production_concept'
            ? 'compiled_agent_production_pbr'
            : 'compiled_agent_preview_pbr',
          shapeProgram: null,
          segmentation: null,
        })
        dispatchSingleResultDecision({
          type: 'decision_received',
          projectId,
          requestId: latestAgentRequestId,
          decision,
        })
        setCandidatePbrCaptureStatus('preview_ready')
        setAssistantNote('候选模型已通过同源 PBR 八视图验收。确认前不会创建可编辑版本。')
      } catch (caught) {
        if (cancelled) return
        setCandidatePbrCaptureStatus('failed')
        const message = `候选模型的同源 PBR 验收未完成：${errorText(caught)}；当前已确认模型保持不变。`
        dispatchSingleResultDecision({ type: 'request_failed', projectId, requestId: latestAgentRequestId, error: message })
        setAssistantNote(message)
      }
    }
    void run()
    return () => { cancelled = true }
  }, [
    activePbrCaptureRef,
    agentKernelItems,
    api,
    concept.project?.project_id,
    dispatchSingleResultDecision,
    hydrateBlockoutDisplay,
    latestAgentRequestId,
    setAssistantNote,
    candidatePbrAuthorizationAttempt,
  ])
  const authorizeCapturedCandidateVisualComparison = useCallback(async () => {
    const pendingAuthorization = candidatePbrAuthorization
    if (!pendingAuthorization) return
    try {
      setCandidatePbrCaptureStatus('authorizing')
      setAssistantNote('正在封存本次候选的千问视觉比较授权；此步骤不发送图片、不生成预览。')
      await authorizeCandidatePbrVisualComparison({
        clientRequestId: `universal_pbr_auth_${pendingAuthorization.turnId}`,
        executionId: pendingAuthorization.executionId,
        projectId: pendingAuthorization.projectId,
        turnId: pendingAuthorization.turnId,
      })
      const captureKey = `${pendingAuthorization.executionId}:${pendingAuthorization.projectId}:${pendingAuthorization.turnId}`
      resumePbrCaptureAfterAuthorizationRef.current = captureKey
      activePbrCaptureRef.current = null
      setCandidatePbrAuthorization(null)
      setAssistantNote('千问比较已获授权；正在对已封存的同源八视图执行一次质量验收。')
      setCandidatePbrAuthorizationAttempt((current) => current + 1)
    } catch (caught) {
      setCandidatePbrCaptureStatus('authorization_required')
      setAssistantNote(`千问视觉比较未获授权：${errorText(caught)}；不会联网、不会创建预览或版本。`)
    }
  }, [candidatePbrAuthorization, setAssistantNote])
  const agentPlanSourcePresentation = selectAgentPlanSourcePresentation(agentPlan)
  const [cameraView, setCameraView] = useState<CameraView>('iso')
  const [lightPreset, setLightPreset] = useState<LightPreset>('cad_neutral')
  const [presentationProfile, setPresentationProfile] = useState<'quick_sketch' | 'showcase'>('showcase')
  const [styleOptionsOpen, setStyleOptionsOpen] = useState(false)
  const [materialOptionsOpen, setMaterialOptionsOpen] = useState(false)
  const { agentThreads, threadHistoryLoading } = useCadWorkbenchPanelAgentThreads({
    api,
    projectId: concept.project?.project_id ?? null,
    activeThreadId: agentThreadId,
  })
  const [viewportDock, dispatchViewportDock] = useReducer(
    viewportDockPresentationReducer,
    initialViewportDockPresentationState,
  )
  const viewportFocusTriggerRef = useRef<HTMLButtonElement | null>(null)
  const [agentAssetChangeSet, setAgentAssetChangeSet] = useState<AgentAssetChangeSet | null>(null)
  const [agentCandidateSelectedPartId, setAgentCandidateSelectedPartId] = useState<string | null>(null)
  const agentAssetVersion = agentAssetWorkspace.assetVersion
  const agentQualityReport = agentAssetWorkspace.qualityReport
  const agentNavigation = agentAssetWorkspace.navigation
  const activeDesignSnapshot = activeDesignState.snapshot
  const {
    activeTool,
    showGrid,
    wireframe,
    xRay,
    explodeFactor,
    sectionOffset,
  } = viewportDisplayPreferences
  const {
    selectedNodeId: selectedComponent,
  } = legacyModuleGraphWorkspace
  const activeDesignAssetVersionId = activeDesignSnapshot?.active_design.source === 'agent_asset'
    ? activeDesignSnapshot.active_design.asset_version_id
    : null
  const activeAgentAssetVersion = activeDesignAssetVersionId === agentAssetVersion?.asset_version_id
    ? agentAssetVersion
    : null
  const projectHasActiveAgentSnapshot = activeDesignSnapshot?.project_id === concept.project?.project_id
    && activeDesignSnapshot?.active_design.source === 'agent_asset'
  const legacyCompatibility = useMemo(
    () => getLegacyCompatibilityDisplay(activeDesignSnapshot, activeDesignState.operation),
    [activeDesignSnapshot, activeDesignState.operation],
  )
  const legacyDesignReadOnly = legacyCompatibility.isLegacyReadOnly
  const agentComponentCandidates = agentEditAssistPresentation.componentCandidates
  const agentStructureSuggestions = agentEditAssistPresentation.structureSuggestions
  const structureSuggestionUnavailableMessage = agentEditAssistPresentation.structureSuggestionUnavailableMessage
  // Once an Agent asset is active, selection must be projected from the
  // server-owned Snapshot. The local value remains only for an uncommitted
  // blockout candidate before a Snapshot asset exists.
  const {
    displayedAgentSelectedPartId,
    sidebarParts,
    selectedAgentPart,
  } = useCadWorkbenchPanelPartSelectionState({
    hasActiveAgentAsset: Boolean(activeAgentAssetVersion),
    agentAssetWorkspaceSelectedPartId: agentAssetWorkspace.selectedPartId,
    agentCandidateSelectedPartId,
    agentAssetVersion,
    blockoutParts: agentBlockoutSegmentation?.parts,
  })
  useCadWorkbenchPanelLegacyGraphWorkspaceSync({
    isLegacyReadOnly: legacyCompatibility.isLegacyReadOnly,
    legacyDetailsEnabled: concept.legacyDetailsEnabled,
    conceptProjectId: concept.project?.project_id ?? null,
    conceptGraphRecord: concept.graphRecord,
    legacyModuleGraphWorkspacePreferenceKey,
    legacyModuleGraphOverlayContextKey,
    openLegacyModuleGraphWorkspace,
    openLegacyModuleGraphOverlay,
    reconcileLegacyModuleGraphSelection,
    reconcileLegacyModuleGraphOverlayNodes,
  })
  const isExternalGlbReference = agentAssetVersion?.shape_program?.schema_version === 'ExternalGLBReference@1'
  const [appearanceMaterialZoneId, setAppearanceMaterialZoneId] = useState('')
  const [surfaceAdornmentOpen, setSurfaceAdornmentOpen] = useState(false)
  const {
    measurementMode,
    measurementReadoutText,
    measurementPrompt,
    measurementAnnotations,
    handleMeasurePoint,
    clearMeasurements,
    pinMeasurement,
    setMeasurementMode,
  } = useCadWorkbenchPanelViewportMeasurements()
  const [referenceEvidenceOpen, setReferenceEvidenceOpen] = useState(false)
  const [referenceViewport, setReferenceViewport] = useState<ReferenceViewportState | null>(null)
  const replaceReferenceViewport = useCallback((next: ReferenceViewportState | null) => {
    setReferenceViewport(next)
  }, [])
  const referenceImageObjectUrl = referenceViewport?.kind === 'image' ? referenceViewport.imageUrl : null
  useEffect(() => () => {
    if (referenceImageObjectUrl) URL.revokeObjectURL(referenceImageObjectUrl)
  }, [referenceImageObjectUrl])

  useEffect(() => {
    if (activeDesignSnapshot?.selected_material_zone_id) {
      setAppearanceMaterialZoneId(activeDesignSnapshot.selected_material_zone_id)
    } else if (selectedAgentPart) {
      setAppearanceMaterialZoneId(selectedAgentPart.material_zone_ids[0] ?? '')
    } else {
      setAppearanceMaterialZoneId('')
    }
  }, [activeDesignSnapshot?.selected_material_zone_id, displayedAgentSelectedPartId, agentAssetVersion?.asset_version_id])

  const activeMaterialDomain = DOMAIN_TYPE_BY_PACK[
    activeAgentAssetVersion?.domain_pack_id ?? agentPlan?.domain_pack_id ?? ''
  ] ?? null
  const materialPresets = agentMaterialCatalogPresentation.materialPresets
  const catalogModules = componentCatalogPresentation.modules
  const materialQuery = agentMaterialFilterPresentation.query
  const materialCategory = agentMaterialFilterPresentation.category
  const materialCompatibilityOnly = agentMaterialFilterPresentation.compatibilityOnly
  const selectedMaterialZoneId = activeDesignSelectedMaterialZoneId(activeDesignSnapshot) ?? appearanceMaterialZoneId
  const {
    activePartDisplay,
    selectedPartRoleLabel,
    selectedAgentPartLocked,
    surfaceAdornmentTarget,
    surfaceAdornmentDisabledReason,
    materialPreselectionContext,
    appearanceMaterialId,
    quickMaterialPresets,
  } = useCadWorkbenchPanelMaterialState({
    conceptProjectId: concept.project?.project_id ?? null,
    activeAgentAssetVersionId: activeAgentAssetVersion?.asset_version_id ?? null,
    activeDesignSnapshot,
    activeDesignIsIdle: activeDesignState.operation === 'idle',
    isExternalGlbReference,
    selectedAgentPart,
    selectedMaterialZoneId,
    legacyDesignReadOnly,
    hasAgentPlan,
    hasActiveAgentAssetVersion: activeAgentAssetVersion !== null,
    materialBindings: activeAgentAssetVersion?.material_bindings,
    hasAgentAssetChangeSet: Boolean(agentAssetChangeSet),
    surfaceAdornmentOpen,
    activeMaterialDomain,
    materialPresets,
    agentMaterialPreselectionPresentation,
  })
  useEffect(() => {
    // Measurements are view-local inspection aids. They never cross a project
    // or exact Agent asset boundary and are deliberately not Snapshot facts.
    clearMeasurements()
  }, [agentAssetVersion?.asset_version_id, clearMeasurements, concept.project?.project_id])
  const {
    referenceEvidenceTarget,
    viewportGlb,
    viewportGlbKind,
    viewportShapeProgram,
    viewportReferenceImage,
    viewportReadoutText,
  } = useCadWorkbenchPanelViewportState({
    conceptProjectId: concept.project?.project_id ?? null,
    activeAgentAssetVersionDomainPackId: activeAgentAssetVersion?.domain_pack_id ?? null,
    agentPlanDomainPackId: agentPlan?.domain_pack_id ?? null,
    activeAgentAssetVersionId: activeAgentAssetVersion?.asset_version_id ?? null,
    isExternalGlbReference,
    legacyDetailsEnabled: concept.legacyDetailsEnabled,
    isPreviewActive: Boolean(agentAssetChangeSet),
    hasActiveAgentAsset: Boolean(activeAgentAssetVersion),
    referenceViewport,
    blockoutGlbBase64: agentBlockoutGlbBase64,
    blockoutGlbKind: agentBlockoutGlbKind,
    blockoutShapeProgram: agentBlockoutShapeProgram,
  })
  const {
    providerConfig,
    providerSetupOpen,
    setProviderSetupOpen,
    providerBaseUrl,
    setProviderBaseUrl,
    providerModel,
    setProviderModel,
    providerApiKey,
    setProviderApiKey,
    providerSaving,
    activeProviderTurnId,
    setActiveProviderTurnId,
    activeProviderCheckId,
    saveProvider,
    testProvider,
    cancelActiveProviderTurn,
  } = useCadWorkbenchPanelProviderConfig({
    api,
    checkService,
    setAssistantNote,
    errorText,
  })
  const [importingGlb, setImportingGlb] = useState(false)
  const chatInputTrimmedEmpty = !chatInput.trim()
  const guideState = useCadWorkbenchPanelGuideMode({
    hasProject: Boolean(concept.project),
    hasActiveAgentAssetSnapshot: projectHasActiveAgentSnapshot,
    hasActiveDesignSnapshot: Boolean(activeDesignSnapshot),
    hasBlockoutSegmentation: Boolean(agentBlockoutSegmentation),
    isDesignOperationIdle: activeDesignState.operation === 'idle',
    hasActiveAgentAssetVersion: Boolean(activeAgentAssetVersion),
    hasActiveDesignProjectMatch: activeDesignState.projectId === concept.project?.project_id,
    singleResultDecisionIdle: singleResultDecisionPresentation.presentation.state === 'idle',
    conceptLoading: concept.loading,
    directionPreviewLoading: agentBlockoutDisplay.directionPreviewLoading,
    chatInputTrimmedEmpty,
    showComposerAdvancedActions,
  })
  const {
    projectIsEmpty,
    showBeginnerGuide,
    showCompactSidebar,
  } = guideState
  const importGlbInputRef = useRef<HTMLInputElement | null>(null)
  const referenceEvidenceRequestEpochRef = useRef(0)
  const referenceRebuildPlanByChangeSetRef = useRef(new Map<string, {
    projectId: string
    baseAssetVersionId: string
    evidenceId: string
    sourceObjectSha256: string
    rebuildPlanId: string
  }>())

  const {
    resetProjectScopedState,
    resetProjectDrawerState,
  } = useCadWorkbenchPanelProjectResetState({
    setAgentAssetChangeSet,
    setAgentCandidateSelectedPartId,
    setSurfaceAdornmentOpen,
    setReferenceEvidenceOpen,
    setStyleOptionsOpen,
    setMaterialOptionsOpen,
    replaceReferenceViewport,
    referenceEvidenceRequestEpochRef,
    referenceRebuildPlanByChangeSetRef,
  })

  useEffect(() => {
    openAgentRenderPresentation(
      activeAgentAssetVersion ? concept.project?.project_id ?? null : null,
      activeAgentAssetVersion?.asset_version_id ?? null,
    )
  }, [activeAgentAssetVersion?.asset_version_id, concept.project?.project_id, openAgentRenderPresentation])

  const activeDesignSyncCallbacks = useCadWorkbenchPanelActiveDesignSyncCallbacks({
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
  })

  const {
    refreshActiveDesign,
    updateRenderPreset,
  } = useCadWorkbenchPanelActiveDesignSync({
    api,
    cameraView,
    lightPreset,
    activeDesignSnapshot,
    snapshotEtag: activeDesignState.snapshotEtag,
    setCameraView,
    setLightPreset,
    callbacks: activeDesignSyncCallbacks,
    setAssistantNote,
  })

  useCadWorkbenchPanelProjectLifecycleSync({
    conceptProjectId: concept.project?.project_id ?? null,
    conceptLegacyDetailsEnabled: concept.legacyDetailsEnabled,
    activeDesignSource: activeDesignSnapshot?.active_design?.source ?? null,
    closeLegacyDetails: concept.closeLegacyDetails,
    openProject,
    openConversationProject,
    dispatchSingleResultDecision,
    openBlockoutProject,
    openAgentAssetWorkspaceProject,
    openViewportDisplayPreferences,
    refreshActiveDesign,
    dispatchViewportDock,
    resetProjectScopedState,
    resetProjectDrawerState,
  })


  const refreshAgentEditAssist = useCadWorkbenchPanelEditAssistLoader({
    api,
    conceptProjectId: concept.project?.project_id ?? null,
    activeAssetVersionId: activeAgentAssetVersion?.asset_version_id ?? null,
    selectedPartId: selectedAgentPart?.part_id ?? null,
    isExternalGlbReference,
    openAgentEditAssistPresentation,
    startAgentEditAssistRead,
    receiveAgentEditAssistRead,
    failAgentEditAssistRead,
  })

  useCadWorkbenchPanelMaterialCatalogAndFilterSync({
    api,
    conceptProjectId: concept.project?.project_id ?? null,
    conceptProjectPackId: concept.project?.profile.pack_id ?? null,
    legacyDetailsEnabled: concept.legacyDetailsEnabled,
    activeAgentAssetVersionId: activeAgentAssetVersion?.asset_version_id ?? null,
    activeAgentAssetVersionDomainPackId: activeAgentAssetVersion?.domain_pack_id ?? null,
    agentPlanDomainPackId: agentPlan?.domain_pack_id ?? null,
    isExternalGlbReference,
    hasActiveAsset: Boolean(activeAgentAssetVersion),
    hasAgentPlan: Boolean(agentPlan),
    legacyDesignReadOnly,
    materialPreselectionContext,
    openComponentCatalog,
    startComponentCatalogRead,
    receiveComponentCatalog,
    failComponentCatalog,
    openAgentMaterialCatalogPresentation,
    startAgentMaterialCatalogRead,
    receiveAgentMaterialCatalog,
    failAgentMaterialCatalog,
    openAgentMaterialFilterPresentation,
    openAgentMaterialPreselectionPresentation,
  })

  const { selectedNode, selectedModuleLabel, workbenchStatusBar } = useCadWorkbenchPanelConversationState({
    conceptGraphRecord: concept.graphRecord,
    catalogModules,
    selectedComponent,
    conceptLoading: concept.loading,
    conceptLegacyDetailsEnabled: concept.legacyDetailsEnabled,
    activeAgentAssetVersionVersionNo: activeAgentAssetVersion?.version_no ?? null,
    activeDesignSnapshot,
    conceptVersions: concept.project?.versions ?? null,
    conceptVersionId: concept.version?.version_id ?? null,
    conceptQualityStatus: concept.qualityRun?.report.status,
    agentQualityStatus: agentQualityReport?.status,
  })

  const { getModuleFileUrl, selectGraphNode } = useCadWorkbenchPanelLegacyGraphSelection({
    graphNodes: concept.graphRecord?.graph.nodes,
    selectLegacyModuleGraphNode,
  })

  const {
    closeAllDrawers,
    openExportDrawer,
    openQualityDrawer,
    handleDownloadAgentGlb,
    handleRenderAgentViews,
    handleDownloadAgentRenderView,
    handleDownloadAgentRenderPackage,
  } = useCadWorkbenchPanelViewportActions({
    api,
    conceptProjectId: concept.project?.project_id ?? null,
    activeAgentAssetVersion: activeAgentAssetVersion
      ? {
          project_id: activeAgentAssetVersion.project_id,
          asset_version_id: activeAgentAssetVersion.asset_version_id,
          version_no: activeAgentAssetVersion.version_no,
        }
      : null,
    renderSet: agentRenderPresentation.renderSet,
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
  })

  useEffect(() => {
    if (!hasOpenDrawer) return
    return bindDrawerFocusTrap(drawerFocusRef, closeAllDrawers)
  }, [closeAllDrawers, hasOpenDrawer])

  const closeViewportFocus = useCallback((restoreFocus = true) => {
    dispatchViewportDock({ type: 'close' })
    if (restoreFocus) {
      window.requestAnimationFrame(() => viewportFocusTriggerRef.current?.focus())
    }
  }, [])

  useEffect(() => {
    if (viewportDock.dockState !== 'focus' || hasOpenDrawer) return
    const onFocusKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      dispatchViewportDock({ type: 'escape' })
      window.requestAnimationFrame(() => viewportFocusTriggerRef.current?.focus())
    }
    window.addEventListener('keydown', onFocusKeyDown, true)
    return () => window.removeEventListener('keydown', onFocusKeyDown, true)
  }, [hasOpenDrawer, viewportDock.dockState])

  const { selectConversationThread } = useCadWorkbenchPanelConversationThreadActions({
    projectId: concept.project?.project_id ?? null,
    getConversationThread: (threadId) => api.getAgentThread(threadId),
    startAgentConversationRequest,
    isCurrentAgentConversationRequest,
    receiveAgentTurn,
    setAssistantNote,
    errorText,
  })

  const { recordAgentTurn } = useCadWorkbenchPanelRecordAgentTurn({
    api,
    conceptProjectId: concept.project?.project_id ?? null,
    conceptProjectName: concept.project?.name ?? null,
    agentThreadId,
    agentKernelItems,
    clarificationOptions: DEFAULT_AGENT_CLARIFICATION_OPTIONS,
    startAgentConversationRequest,
    isCurrentAgentConversationRequest,
    claimAgentTurnSubmission: () => claimAgentTurnSubmission(agentTurnSubmissionRef),
    releaseAgentTurnSubmission: () => releaseAgentTurnSubmission(agentTurnSubmissionRef),
    parseAgentTurnPresentation,
    receiveAgentTurn,
    receiveAgentClarification,
    markAgentKernelUnavailable,
    dispatchSingleResultDecision,
    setActiveProviderTurnId,
    clearBlockoutDisplay,
    clearAgentAssetWorkspace,
    setAgentAssetChangeSet,
    setAgentCandidateSelectedPartId,
    hydrateBlockoutDisplay,
    setAssistantNote,
    errorText,
  })

  const {
    selectAgentPart,
    setAgentPartDisplay,
    selectMaterialZone,
    requestLegacyAgentRebuild,
  } = useCadWorkbenchPanelActiveDesignPartActions({
    api,
    activeDesignSnapshot,
    activeDesignSnapshotEtag: activeDesignState.snapshotEtag,
    agentAssetVersion,
    setAssistantMode,
    setAssistantNote,
    setAgentCandidateSelectedPartId,
    setAppearanceMaterialZoneId,
    hasAgentAssetChangeSet: Boolean(agentAssetChangeSet),
    startActiveDesignRequest,
    failActiveDesignRequest,
    receiveActiveDesignSnapshot,
    refreshActiveDesign,
    activeDesignCanSelectParts,
    activeDesignSelectedPartId,
    activeDesignSelectedMaterialZoneId,
    projectAgentAssetWorkspaceSelection,
    legacyDesignReadOnly,
  })

  const {
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
  } = useCadWorkbenchPanelAgentAssetLifecycleActions({
    api,
    conceptProjectId: concept.project?.project_id ?? null,
    conceptAgentPlan: agentPlan,
    presentationProfile,
    activeDesignSnapshotEtag: activeDesignState.snapshotEtag,
    activeAgentAssetVersion,
    agentBlockoutSegmentation,
    agentAssetChangeSet,
    selectedAgentPart,
    selectedPartRoleLabel,
    clearAgentAssetWorkspaceQuality,
    clearAgentEditAssistPresentation,
    hydrateBlockoutDisplay,
    clearBlockoutDisplay: () => {
      clearBlockoutDisplay(null)
    },
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
    latestSingleResultRequestId: singleResultDecisionPresentation.latestRequestId,
    errorText,
    refreshAgentEditAssist,
  })

  const importGlbReferenceWithBusy = useCallback(async (event: ChangeEvent<HTMLInputElement>) => {
    setImportingGlb(true)
    try {
      await importGlbReference(event)
    } finally {
      setImportingGlb(false)
    }
  }, [importGlbReference])

  const { surfaceAdornmentAdapter, referenceEvidenceAdapter } = useCadWorkbenchPanelAdapters({
    api,
    setAgentAssetChangeSet,
    setBlockoutGlb,
    setBlockoutShapeProgram,
    clearAgentAssetWorkspaceQuality,
    refreshActiveDesign,
    conceptProjectId: concept.project?.project_id ?? null,
    activeAgentAssetVersionId: activeAgentAssetVersion?.asset_version_id ?? null,
    activeAgentAssetVersionShapeProgram: activeAgentAssetVersion?.shape_program ?? null,
    referenceEvidenceRequestEpochRef,
    referenceRebuildPlanByChangeSetRef,
    setReferenceViewport: replaceReferenceViewport,
  })

  const { navigateAgentAsset } = useCadWorkbenchPanelNavigateAgentAsset({
    api,
    activeDesignSnapshot,
    activeDesignSnapshotEtag: activeDesignState.snapshotEtag,
    activeAgentAssetVersion,
    agentAssetChangeSet: Boolean(agentAssetChangeSet),
    startActiveDesignRequest,
    failActiveDesignRequest,
    receiveActiveDesignSnapshot,
    refreshActiveDesign,
    setAssistantNote,
    setAgentAssetChangeSet,
  })

  const {
    submitAssistantInstructionWithText,
    runAssistantAction,
    retryCandidatePreview,
    focusComposerInput,
  } = useCadWorkbenchPanelAssistantActions({
    assistantMode,
    chatInput,
    legacyDesignReadOnly,
    presentationProfile,
    setAssistantMode,
    setAssistantNote,
    setChatInput,
    agentPlan,
    previewAgentDirection,
    recordAgentTurn,
    previewAgentAssemblyDelta,
  })
  const {
    quickMaterialPresetSelect,
    catalogMaterialPreview,
    catalogMaterialPreviewNote,
  } = useCadWorkbenchPanelMaterialPresetActions({
    hasAgentAssetVersion: Boolean(agentAssetVersion),
    selectedAgentPartId: selectedAgentPart?.part_id ?? null,
    selectedMaterialZoneId,
    selectMaterialPreselection,
    previewAgentAssetEdit,
    setAssistantNote,
  })
  const openSurfaceAdornment = useCallback(() => {
    setSurfaceAdornmentOpen(true)
  }, [setSurfaceAdornmentOpen])
  const {
    agentSelectionCardProps,
    materialOptionsProps,
  } = useCadWorkbenchPanelSelectionToolsPresentation({
    agentBlockoutSegmentation,
    agentAssetVersion,
    activeAgentAssetVersion,
    selectedPart: selectedAgentPart ?? undefined,
    selectedPartId: displayedAgentSelectedPartId,
    partDisplay: activePartDisplay,
    isSelectedPartLocked: selectedAgentPartLocked,
    isExternalGlbReference,
    isSnapshotActionPending: activeDesignState.operation !== 'idle',
    agentAssetChangeSet,
    agentComponentCandidates,
    agentStructureSuggestions,
    structureSuggestionUnavailableMessage,
    semanticProportions: agentEditAssistPresentation.semanticProportions,
    editAssistLoading: agentEditAssistPresentation.loading,
    blockoutPreviewPresentation,
    onSelectPart: selectAgentPart,
    onPreviewEdit: previewAgentAssetEdit,
    onSaveSelectedComponent: saveSelectedAgentComponent,
    onReplaceComponent: replaceWithAgentComponent,
    onPreviewStructureSuggestion: previewStructureSuggestion,
    onSetPartDisplay: setAgentPartDisplay,
    onInspectAsset: inspectAgentAsset,
    onRejectChange: rejectAgentAssetEdit,
    onConfirmChange: confirmAgentAssetEdit,
    onOpenSurfaceAdornment: openSurfaceAdornment,
    surfaceAdornmentDisabled: Boolean(surfaceAdornmentDisabledReason),
    surfaceAdornmentDetail: surfaceAdornmentDisabledReason ?? undefined,
    showComposerAdvancedActions,
    materialOptionsOpen,
    agentBlockoutShapeProgram,
    materialPresets,
    quickMaterialPresets,
    appearanceMaterialId,
    selectedPartRoleLabel,
    selectedMaterialZoneId,
    hasSelectedAgentPart: Boolean(selectedAgentPart),
    selectedMaterialZoneIds: selectedAgentPart?.material_zone_ids ?? [],
    hasAgentAssetVersion: Boolean(agentAssetVersion),
    activeMaterialDomain,
    materialCompatibilityOnly,
    materialQuery,
    materialCategory,
    catalogLoading: agentMaterialCatalogPresentation.loading,
    catalogMessage: agentMaterialCatalogPresentation.catalogMessage,
    quickMaterialPresetSelect,
    selectMaterialPreselection,
    selectMaterialZone: (zoneId: string) => { void selectMaterialZone(zoneId) },
    setMaterialFilterCompatibilityOnly,
    setMaterialFilterQuery,
    setMaterialFilterCategory,
    catalogMaterialPreview,
    catalogMaterialPreviewNote,
  })
  const visibleSingleResult = singleResultDecisionPresentation.presentation.state === 'ready'
    ? singleResultDecisionPresentation.presentation.decision
    : null
  const {
    compatibilityResultSummary,
    compatibilityVersionLabel,
  } = useCadWorkbenchPanelCompatibilitySummary({
    activeAssetSummary: activeAgentAssetVersion?.summary,
    directionSummary: agentPlan?.directions[0]?.summary,
    fallbackPartCount: agentBlockoutSegmentation?.parts.length ?? activeAgentAssetVersion?.parts.length ?? 0,
    activeAssetVersionNo: activeAgentAssetVersion?.version_no ?? null,
  })
  const referenceEvidenceVisionContext = useCadWorkbenchPanelReferenceEvidenceVisionContext({
    submitAssistantInstructionWithText,
    instruction: chatInput,
    activeAssetVersionId: activeAgentAssetVersion?.asset_version_id ?? null,
    selectedPartId: selectedAgentPart?.part_id ?? null,
    selectedMaterialZoneId: selectedMaterialZoneId || null,
  })
  const handleConversationProfileChange = useCallback((profile: 'quick_sketch' | 'showcase') => {
    setPresentationProfile(profile)
    if (agentBlockoutSegmentation) {
      void previewAgentDirection(
        agentBlockoutSegmentation.direction_id,
        agentBlockoutSegmentation.variation_index ?? 0,
        profile,
      )
    }
  }, [agentBlockoutSegmentation?.direction_id, agentBlockoutSegmentation?.variation_index, previewAgentDirection])
  const handleBeginnerGuideStart = useCallback(() => {
    if (chatInputTrimmedEmpty) {
      setChatInput(COMPOSER_DEFAULT_BEGINNER_PROMPT)
    }
    focusComposerInput()
  }, [chatInputTrimmedEmpty, focusComposerInput, setChatInput])
  const toggleComposerAdvancedActions = useCallback(
    () => setShowComposerAdvancedActions((current) => !current),
    [],
  )
  const globalPanelActions = useCadWorkbenchPanelGlobalActions({
    canUndo: Boolean(activeAgentAssetVersion && agentNavigation?.can_undo && !agentAssetChangeSet),
    canRedo: Boolean(activeAgentAssetVersion && agentNavigation?.can_redo && !agentAssetChangeSet),
    canImport: Boolean(concept.project?.project_id && !agentAssetChangeSet),
    importingGlb,
  })
  const importFromActionBar = useCallback(() => {
    importGlbInputRef.current?.click()
  }, [])
  const isComposerReady = Boolean(concept.project) && !concept.loading
  const composerSending = concept.loading
    || agentBlockoutDisplay.directionPreviewLoading
    || candidatePbrCaptureStatus === 'capturing'
    || singleResultDecisionPresentation.presentation.state === 'processing'

  const handleStyleAction = useCallback(() => {
    setStyleOptionsOpen((current) => !current)
    setMaterialOptionsOpen(false)
  }, [])
  const handleMaterialAction = useCallback(() => {
    setMaterialOptionsOpen((current) => !current)
    setStyleOptionsOpen(false)
  }, [])
  const handleReferenceAction = useCallback(() => {
    if (agentAssetChangeSet) {
      setAssistantNote('请先保留或取消当前预览，再添加参考证据。')
      return
    }
    setReferenceEvidenceOpen(true)
  }, [agentAssetChangeSet, setAssistantNote])
  const handleSurfaceAdornmentAction = useCallback(() => {
    if (surfaceAdornmentDisabledReason) {
      setAssistantNote(surfaceAdornmentDisabledReason)
      return
    }
    openSurfaceAdornment()
  }, [openSurfaceAdornment, setAssistantNote, surfaceAdornmentDisabledReason])

  return (
    <div
      className="cad-workbench"
      data-testid="cad-workbench"
      // These are stable, non-secret DOM facts for the opt-in packaged WebView
      // acceptance harness.  The harness drives the visible controls; it never
      // calls product APIs or reads React state.  Keeping the lineage visible
      // here lets the native report fail closed when a stale renderer, preview
      // or Snapshot is displayed.
      data-qa-project-id={concept.project?.project_id ?? ''}
      data-qa-agent-thread-id={agentThreadId ?? ''}
      data-qa-agent-turn-id={agentKernelItems[agentKernelItems.length - 1]?.turn_id ?? ''}
      data-qa-active-asset-version-id={activeAgentAssetVersion?.asset_version_id ?? ''}
      data-qa-active-snapshot-revision={activeDesignSnapshot?.revision ?? ''}
      data-qa-single-result-turn-id={visibleSingleResult?.turn_id ?? ''}
      data-qa-single-result-preview-id={visibleSingleResult?.preview.preview_id ?? ''}
      data-qa-single-result-artifact-sha256={visibleSingleResult?.preview.artifact_sha256 ?? ''}
      data-qa-single-result-profile={visibleSingleResult?.preview.artifact_profile_id ?? ''}
    >
      <header className="cad-command-bar">
        <div className="cad-brand" aria-label="CAD 工作台">
          <span className="cad-brand-mark"><Cube size={18} weight="fill" /></span>
          <span>ForgeCAD</span>
        </div>
        <div className="cad-workspace-title" aria-label="当前项目">
          <strong>{concept.project?.name ?? '新概念设计'}</strong>
          <span>{concept.project ? '已自动保存' : concept.loading ? '正在处理…' : '未保存'}</span>
        </div>
        <div className="cad-global-actions" aria-label="工作区操作">
          <CadWorkbenchPanelGlobalActions
            actions={globalPanelActions}
            onUndo={() => void navigateAgentAsset('undo')}
            onRedo={() => void navigateAgentAsset('redo')}
            onImport={importFromActionBar}
            onCheck={openQualityDrawer}
            onExport={openExportDrawer}
            onOpenAdvanced={() => setShowComposerAdvancedActions(true)}
            canCheck={Boolean(activeAgentAssetVersion)}
            showAdvancedActions={showComposerAdvancedActions}
          />
        </div>
        <input
          ref={importGlbInputRef}
          className="visually-hidden"
          type="file"
          accept=".glb,model/gltf-binary"
          onChange={importGlbReferenceWithBusy}
          aria-label="导入 GLB 参考模型"
        />
      </header>

      <div
        className={`cad-layout f026-layout ${viewportDock.dockState === 'focus' ? 'is-viewport-focus' : ''} ${
          showComposerAdvancedActions ? '' : 'f026-layout-beginner'
        }`}
        data-viewport-dock-state={viewportDock.dockState}
      >
          <WorkbenchSidebar
            projects={concept.projects}
            activeProjectId={concept.project?.project_id ?? null}
            threads={agentThreads}
            activeThreadId={agentThreadId}
            parts={sidebarParts}
            selectedPartId={displayedAgentSelectedPartId}
            loading={concept.loading || threadHistoryLoading}
            compactMode={!showComposerAdvancedActions || showCompactSidebar}
            onCreateProject={() => void concept.createStarterProject()}
            onSelectProject={(projectId) => void concept.selectProject(projectId)}
            onSelectThread={(threadId) => void selectConversationThread(threadId)}
            onSelectPart={(partId) => void selectAgentPart(partId)}
          />

        <main className="f026-conversation-stage" aria-label="Agent 对话工作区">
          <div className="f026-conversation-scroll">
          <section className="f026-agent-timeline">
            <div className="cad-panel-title">
              <span><Sparkle size={16} weight="fill" /> 设计助手</span>
              <span className="assistant-state" role="status" aria-live="polite">
                {concept.loading ? '正在工作' : '准备就绪'}
              </span>
            </div>
            <AgentConversation
              showAdvancedControls={showComposerAdvancedActions}
              loading={concept.loading}
              projectExists={Boolean(concept.project)}
              projectIsEmpty={projectIsEmpty}
              legacyCompatibility={legacyCompatibility}
              onRequestLegacyAgentRebuild={() => void requestLegacyAgentRebuild()}
              onOpenLegacyDetails={() => void concept.openLegacyDetails()}
              providerConfig={providerConfig}
              providerSetupOpen={providerSetupOpen}
              providerBaseUrl={providerBaseUrl}
              providerModel={providerModel}
              providerApiKey={providerApiKey}
              providerSaving={providerSaving}
              onToggleProviderSetup={() => setProviderSetupOpen((current) => !current)}
              onProviderBaseUrlChange={setProviderBaseUrl}
              onProviderModelChange={setProviderModel}
              onProviderApiKeyChange={setProviderApiKey}
              onCancelProviderSetup={() => setProviderSetupOpen(false)}
              onTestProvider={() => void testProvider()}
              onSaveProvider={() => void saveProvider()}
              activeProviderTurnId={activeProviderTurnId ?? activeProviderCheckId}
              onCancelProviderTurn={() => void cancelActiveProviderTurn()}
              assistantMode={assistantMode}
              selectedNode={displayedAgentSelectedPartId}
              selectedModuleLabel={selectedModuleLabel}
              assistantNote={assistantNote}
              errorMessage={concept.error}
              blockoutPreviewPresentation={blockoutPreviewPresentation}
              agentPlanSourcePresentation={agentPlanSourcePresentation}
              conceptFamilySuggestions={CONCEPT_FAMILY_SUGGESTIONS}
              presentationProfile={presentationProfile}
              styleOptionsOpen={styleOptionsOpen}
              onAssistantModeChange={setAssistantMode}
              onSuggestionSelect={setChatInput}
              onPresentationProfileChange={handleConversationProfileChange}
              onClarificationSelect={(option) => void submitAssistantInstructionWithText(
                `${agentClarification?.originalMessage ? `${agentClarification.originalMessage}\n` : ''}${option.prompt}`,
                option.domain_pack_id,
              )}
              agentClarification={agentClarification}
              agentKernelItems={agentKernelItems}
              agentKernelUnavailable={agentKernelUnavailable}
              agentPlan={agentPlan}
              candidatePreviewQualityPresentation={candidatePreviewQualityPresentation}
            />
            {candidatePbrCaptureStatus === 'authorization_required' && candidatePbrAuthorization ? (
              <section className="cad-panel" aria-label="千问视觉验收授权" data-testid="universal-visual-authorization-card">
                <div className="cad-panel-title"><span><Sparkle size={16} weight="fill" /> 参考图视觉验收</span></div>
                <p>候选已由当前工作台的同一 PBR 渲染器完成八视图采集。授权后，千问仅比较本次封存候选与已授权参考图；最多 3 次、预算上限 US$0.10，未授权不会联网。</p>
                <button
                  type="button"
                  className="reference-evidence-primary"
                  data-testid="authorize-universal-visual-comparison"
                  onClick={() => void authorizeCapturedCandidateVisualComparison()}
                >授权千问进行本次视觉验收</button>
              </section>
            ) : null}
            <CadWorkbenchPanelResultCards
              singleResultDecisionPresentation={singleResultDecisionPresentation.presentation}
              directionPreviewLoading={agentBlockoutDisplay.directionPreviewLoading}
              conceptLoading={concept.loading}
              previewError={Boolean(agentBlockoutDisplay.previewError)}
              assistantNote={assistantNote}
              showAdvancedControls={showComposerAdvancedActions}
              onRetrySingleResult={runAssistantAction}
              onContinueEditing={focusComposerInput}
              onConfirmSingleResult={confirmSingleResultPreview}
              onRetryCandidatePreview={retryCandidatePreview}
              showCompatibilityResultCard={Boolean(agentBlockoutSegmentation || activeAgentAssetVersion)}
              compatibilitySummary={compatibilityResultSummary}
              compatibilityVersionLabel={compatibilityVersionLabel}
              onSaveCompatibility={activeAgentAssetVersion ? null : commitAgentBlockout}
          />
            <CadWorkbenchPanelSelectionTools
              agentSelectionCardProps={agentSelectionCardProps}
              materialOptionsProps={materialOptionsProps}
              showSelectionTools={showComposerAdvancedActions}
              showMaterialOptions={showComposerAdvancedActions}
              expandResultDetails={Boolean(activeAgentAssetVersion && showComposerAdvancedActions)}
              onOpenSurfaceAdornment={openSurfaceAdornment}
            />
            <SurfaceAdornmentDrawer
              open={surfaceAdornmentOpen}
              target={surfaceAdornmentTarget}
              disabledReason={surfaceAdornmentDisabledReason}
              adapter={surfaceAdornmentAdapter}
              onClose={() => setSurfaceAdornmentOpen(false)}
              onMessage={setAssistantNote}
            />
            <ReferenceEvidenceDrawer
              open={referenceEvidenceOpen}
              target={referenceEvidenceTarget}
              adapter={referenceEvidenceAdapter}
              onClose={() => {
                replaceReferenceViewport(null)
                setReferenceEvidenceOpen(false)
              }}
              onMessage={setAssistantNote}
              visionContext={referenceEvidenceVisionContext}
            />
            <small className="planner-boundary">所有生成和调整都只影响虚构、非功能展示组件；预览确认前不会写入版本。</small>
        </section>
          </div>
          <CadWorkbenchPanelBeginnerGuide
            showComposerAdvancedActions={showComposerAdvancedActions}
            isVisible={showBeginnerGuide}
            onFocusComposerInput={handleBeginnerGuideStart}
            onToggleAdvancedActions={toggleComposerAdvancedActions}
          />
          {!showComposerAdvancedActions ? (
            <section className="f026-mini-toolbelt" aria-label="创作工具快速入口">
              <div className="f026-mini-toolbelt-menu">
                <button
                  type="button"
                  className="f026-mini-toolbelt-action"
                  disabled={!isComposerReady}
                  onClick={handleStyleAction}
                >
                  换外观
                </button>
                <button
                  type="button"
                  className="f026-mini-toolbelt-action"
                  disabled={!isComposerReady}
                  onClick={handleMaterialAction}
                >
                  换材质
                </button>
                <button
                  type="button"
                  className="f026-mini-toolbelt-action"
                  disabled={!isComposerReady}
                  onClick={handleReferenceAction}
                >
                  添加参考
                </button>
                <button
                  type="button"
                  className="f026-mini-toolbelt-action"
                  disabled={Boolean(surfaceAdornmentDisabledReason) || !isComposerReady}
                  title={surfaceAdornmentDisabledReason ?? '参考当前模型，添加贴花与局部装饰'}
                  onClick={handleSurfaceAdornmentAction}
                >
                  局部装饰
                </button>
              </div>
            </section>
          ) : null}
          <WorkbenchComposer
            value={chatInput}
            disabled={!isComposerReady}
            // A formal V003 turn owns one unconfirmed result at a time. Keep
            // the composer in its existing sending state until that sealed
            // decision arrives so a double click cannot start a second Turn
            // while the same single-renderer preview is still compiling.
            sending={composerSending}
            referenceImportCapability="reference_guided_rebuild"
            showAdvancedActions={showComposerAdvancedActions}
            showStarterPrompts={!showComposerAdvancedActions}
            onChange={setChatInput}
            onSend={runAssistantAction}
            onOpenStyle={() => {
              handleStyleAction()
            }}
            onOpenMaterial={() => {
              handleMaterialAction()
            }}
            onOpenReference={() => {
              handleReferenceAction()
            }}
            onOpenSurfaceAdornment={handleSurfaceAdornmentAction}
            surfaceAdornmentDisabled={Boolean(surfaceAdornmentDisabledReason)}
            surfaceAdornmentDetail={surfaceAdornmentDisabledReason ?? undefined}
          />
        </main>

        <section className="cad-center-stage f026-viewport-stage" aria-label="3D 工作区">
          <div className="viewport-shell">
            <button
              ref={viewportFocusTriggerRef}
              type="button"
              className="f026-viewport-focus-toggle"
              aria-label={viewportDock.dockState === 'focus' ? '返回对话' : '放大 3D 视图'}
              aria-pressed={viewportDock.dockState === 'focus'}
              onClick={() => {
                if (viewportDock.dockState === 'focus') closeViewportFocus(false)
                else dispatchViewportDock({ type: 'open' })
              }}
            >
              {viewportDock.dockState === 'focus' ? <><ArrowLeft size={16} /> 返回对话</> : <><ArrowsOutCardinal size={16} /> 专注视图</>}
            </button>
            {viewportDock.dockState === 'focus' && (
              <button
                type="button"
                className="f026-viewport-focus-close"
                aria-label="关闭 3D 专注视图"
                onClick={() => closeViewportFocus()}
              ><X size={18} /></button>
            )}
            <div className="viewport-toolbar" aria-label="CAD 视口工具">
              {VIEWPORT_TOOLBAR_ITEMS.map((tool) => (
                <button
                  key={tool.id}
                  type="button"
                  className={activeTool === tool.id ? 'active' : ''}
                  disabled={!tool.implemented}
                  title={tool.unavailableReason}
                  aria-label={tool.label}
                  onClick={() => setViewportTool(tool.id)}
                >
                  <tool.icon size={17} />
                </button>
              ))}
            </div>
            <ModuleGraphViewport
              graphRecord={concept.legacyDetailsEnabled ? concept.graphRecord : null}
              modules={concept.legacyDetailsEnabled ? catalogModules : []}
              cameraView={cameraView}
              lightPreset={lightPreset}
              showGrid={showGrid}
              wireframe={wireframe}
              xRay={xRay}
              sectionEnabled={false}
              sectionOffset={sectionOffset}
              selectedNodeId={concept.legacyDetailsEnabled ? selectedComponent : ''}
              hiddenNodeIds={concept.legacyDetailsEnabled ? legacyModuleGraphOverlay.hiddenNodeIds : []}
              focusNodeId={concept.legacyDetailsEnabled ? legacyModuleGraphOverlay.focusNodeId : null}
              qualityHighlightNodeIds={[]}
              qualityGeometryRefs={[]}
              blockoutGlbBase64={viewportGlb}
              blockoutGlbKind={viewportGlbKind}
              blockoutShapeProgram={viewportShapeProgram}
              blockoutMaterialOverride={viewportShapeProgram ? appearanceMaterialId : null}
              referenceImage={viewportReferenceImage}
              onReferenceImageDisplayFailure={() => {
                replaceReferenceViewport(null)
                setAssistantNote('参考图片无法在 3D 视口显示；已安全返回当前结果。')
              }}
              selectedAgentPartId={displayedAgentSelectedPartId}
              hiddenAgentPartIds={activePartDisplay?.hidden_part_ids ?? []}
              isolatedAgentPartId={activePartDisplay?.isolated_part_id ?? null}
              lockedAgentPartIds={activePartDisplay?.locked_part_ids ?? []}
              showConnectors={false}
              explodeFactor={explodeFactor}
              ghostPreview={Boolean(agentAssetChangeSet)}
              transformTool="none"
              transformSpace="world"
              snapEnabled={false}
              measureEnabled={activeTool === 'measure'}
              getModuleFileUrl={getModuleFileUrl}
              onSelectNode={(nodeId) => { if (concept.legacyDetailsEnabled) selectGraphNode(nodeId) }}
              onDropModule={() => undefined}
              onTransformCommit={() => undefined}
              onMeasurePoint={handleMeasurePoint}
              onPbrCaptureViewportChange={onPbrCaptureViewportChange}
            />
            <CadWorkbenchPanelViewportOverlays
              activeTool={activeTool}
              ghostPreview={Boolean(agentAssetChangeSet)}
              measurementMode={measurementMode}
              measurementPrompt={measurementPrompt}
              measurementReadoutText={measurementReadoutText}
              viewportReadoutText={viewportReadoutText}
              measurementAnnotations={measurementAnnotations}
              onMeasurementModeChange={setMeasurementMode}
              onPinMeasurement={pinMeasurement}
              onClearMeasurements={clearMeasurements}
              cameraView={cameraView}
              lightPreset={lightPreset}
              explodeFactor={explodeFactor}
              onCameraViewChange={(next) => void updateRenderPreset({ cameraView: next })}
              onLightPresetChange={(next) => void updateRenderPreset({ lightPreset: next })}
              onToggleExplode={() => {
                setViewportExplodeFactor(explodeFactor > 0 ? 0 : 0.42)
              }}
            />
          </div>
          {exportOpen || qualityOpen ? (
            <WorkbenchDrawerStack
              exportOpen={exportOpen}
              qualityOpen={qualityOpen}
              exportDrawer={{
                activeAgentAssetVersion: activeAgentAssetVersion,
                activeDesignIdle: activeDesignState.operation === 'idle',
                drawerRef: drawerFocusRef,
                onClose: closeAllDrawers,
                onDownloadAgentGlb: handleDownloadAgentGlb,
                renderSet: agentRenderPresentation.renderSet,
                renderLoading: agentRenderPresentation.renderLoading,
                renderPackageLoading: agentRenderPresentation.renderPackageLoading,
                onRenderViews: handleRenderAgentViews,
                onDownloadRenderView: handleDownloadAgentRenderView,
                onDownloadRenderPackage: handleDownloadAgentRenderPackage,
              }}
              quality={{
                activeAgentAssetVersion: activeAgentAssetVersion,
                agentQualityReport: agentQualityReport,
                agentAssetChangeSet: agentAssetChangeSet,
                drawerRef: drawerFocusRef,
                onClose: closeAllDrawers,
                onInspectAgentAsset: () => void inspectAgentAsset(),
              }}
            />
          ) : null}
        </section>

            {!showCompactSidebar && showComposerAdvancedActions ? (
              <WorkbenchInspectorRail
                mode={activeDesignSnapshot?.active_design.source === 'agent_asset'
                  ? 'agent'
              : legacyDesignReadOnly
                ? 'legacy'
                : 'empty'}
            agentAssetVersion={activeAgentAssetVersion}
            agentQualityReport={agentQualityReport}
            selectedAgentPartId={displayedAgentSelectedPartId}
            selectedAgentPart={selectedAgentPart}
            materialEditor={null}
            legacyDetailsOpen={concept.legacyDetailsEnabled}
            legacyVersion={concept.version}
            legacyGraph={concept.graphRecord}
            legacyQualityRun={concept.qualityRun}
            selectedLegacyNode={selectedNode}
            onCloseLegacyDetails={concept.closeLegacyDetails}
            onSelectLegacyNode={selectGraphNode}
          />
        ) : null}
      </div>

      <CadWorkbenchPanelStatusBar
        workbenchStatusBar={workbenchStatusBar}
        showCompactSidebar={showCompactSidebar}
      />
    </div>
  )
}
