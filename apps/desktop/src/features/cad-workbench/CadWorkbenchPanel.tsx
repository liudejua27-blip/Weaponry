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
  CheckCircle,
  Cube,
  DotsThreeCircle,
  FolderOpen,
  List,
  SpinnerGap,
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
import { useCadWorkbenchPanelKeyboardShortcuts } from './useCadWorkbenchPanelKeyboardShortcuts'
import { useCadWorkbenchVoiceInput } from './useCadWorkbenchVoiceInput'
import {
  activeDesignCanSelectParts,
  activeDesignSelectedMaterialZoneId,
  activeDesignSelectedPartId,
} from './activeDesignMachine'
import { useWorkbenchLifecycle } from './useWorkbenchLifecycle'
import { CadWorkbenchPanelGlobalActions, type WorkbenchPanelWorkflowMode } from './cadWorkbenchPanelGlobalActions'
import type { WorkflowState as CadWorkbenchWorkflowStep } from './cadWorkbenchPanelGlobalActions'
import {
  claimAgentTurnSubmission,
  hasAgentToolInvocation,
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
import {
  gameAssetDeliveryRequestForProfile,
  type GameAssetDeliveryProfile,
} from './agentTurnSubmissionLoader'
import { useCadWorkbenchPanelNavigateAgentAsset } from './useCadWorkbenchPanelNavigateAgentAsset'
import { QUICK_MODIFY_PRESETS } from './cadWorkbenchQuickModifyPresets.js'
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
  pack_unclassified: 'generic_visual_exterior',
}
const EMPTY_AGENT_KERNEL_ITEMS: readonly never[] = []


const CREATE_PROJECT_TEMPLATES: readonly string[] = [
  '写实动物外观',
  '角色与生物',
  '家具与产品',
  '建筑与环境',
  '游戏道具外观',
  '混合对象',
]

const CREATE_PROJECT_EXAMPLES: readonly string[] = [
  '生成一只用于游戏美术的写实短毛家猫，保持自然体态和清晰毛发层次。',
  '设计一套手工白瓷茶具，壶、杯、托盘分件清楚并体现釉面变化。',
  '创建一座山谷中的现代玻璃住宅，表达建筑体块、露台、岩石和植被关系。',
]

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
  throw new Error(
    `CANDIDATE_PBR_CAPTURE_VIEWPORT_NOT_READY:${viewport.dataset.blockoutLoadState ?? 'missing'}:${viewport.dataset.blockoutRenderSource ?? 'missing'}:${viewport.dataset.blockoutGlbSha256 ? 'hash-present' : 'hash-missing'}`,
  )
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
  const [pbrCaptureViewportReady, setPbrCaptureViewportReady] = useState(false)
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
    setPbrCaptureViewportReady(Boolean(viewport))
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
  const [workbenchMode, setWorkbenchMode] = useState<WorkbenchPanelWorkflowMode>('generate')
  const [gameAssetDeliveryProfile, setGameAssetDeliveryProfile] = useState<GameAssetDeliveryProfile>('off')
  const gameAssetDelivery = gameAssetDeliveryRequestForProfile(gameAssetDeliveryProfile)
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
            setAssistantNote(`候选 GLB 已从 Rust 验收会话返回（${Math.round(issue.glbBase64.length / 1024)} KB），正在装入同一工作台。`)
            hydrateBlockoutDisplay(projectId, {
              glbBase64: issue.glbBase64,
              glbKind: issue.artifactProfileId === 'production_concept'
                ? 'compiled_agent_production_pbr'
                : 'compiled_agent_preview_pbr',
              shapeProgram: null,
              segmentation: null,
            })
            setAssistantNote('候选 GLB 已返回，正在等待同一 WebGL 渲染器完成 PBR 装载。')
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
            ? '候选模型还需要继续调整；当前已确认模型保持不变。'
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
    pbrCaptureViewportReady,
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
  const [historyPreview, setHistoryPreview] = useState<{
    threadId: string
    returnThreadId: string | null
    mode: 'compare' | 'restore'
    title: string
  } | null>(null)
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
  const [isSelectionDismissed, setIsSelectionDismissed] = useState(false)
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
  const presentedAgentSelectedPartId = isSelectionDismissed ? null : displayedAgentSelectedPartId
  const presentedSelectedAgentPart = isSelectionDismissed ? null : selectedAgentPart
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
  const [isCreateSetupOpen, setIsCreateSetupOpen] = useState(false)
  const [createPrompt, setCreatePrompt] = useState('')
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
    projectIsEmpty: guideProjectIsEmpty,
    showBeginnerGuide,
    showCompactSidebar,
  } = guideState
  const projectIsEmpty = guideProjectIsEmpty && !projectHasActiveAgentSnapshot
  const [isCompactViewport, setIsCompactViewport] = useState(false)
  const [isMobileSidebarOpen, setIsMobileSidebarOpen] = useState(false)
  const [isMobileAssistantOpen, setIsMobileAssistantOpen] = useState(false)
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState(false)
  const [isAssistantCollapsed, setIsAssistantCollapsed] = useState(false)
  const [focusAgentPartId, setFocusAgentPartId] = useState<string | null>(null)
  const [focusAgentPartRequest, setFocusAgentPartRequest] = useState(0)
  const isMobileLayout = isCompactViewport
  const shouldCompactSidebar = showCompactSidebar || isCompactViewport

  useEffect(() => {
    if (typeof window === 'undefined') return
    const mediaQuery = window.matchMedia('(max-width: 1024px)')
    const syncCompactLayout = (nextCompact: boolean) => {
      setIsCompactViewport(nextCompact)
      setIsMobileSidebarOpen(false)
      setIsMobileAssistantOpen(false)
    }
    syncCompactLayout(mediaQuery.matches)
    const onChange = (event: MediaQueryListEvent) => syncCompactLayout(event.matches)
    mediaQuery.addEventListener('change', onChange)
    return () => mediaQuery.removeEventListener('change', onChange)
  }, [])
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
    selectedPartId: presentedSelectedAgentPart?.part_id ?? null,
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
    pbrCaptureViewportRef,
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
    // bindDrawerFocusTrap schedules focusInitialControl and keeps Tab inside the active drawer.
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

  const handleConversationThreadSelect = useCallback((threadId: string) => {
    setHistoryPreview(null)
    void selectConversationThread(threadId)
  }, [selectConversationThread])

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

  const selectAgentPartFromUi = useCallback(async (partId: string) => {
    setIsSelectionDismissed(false)
    await selectAgentPart(partId)
  }, [selectAgentPart])

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
    submitAssistantChangeInstructionWithText,
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
    gameAssetDelivery,
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
    onSelectPart: selectAgentPartFromUi,
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
    agentBlockoutShapeProgram: agentBlockoutShapeProgram ?? activeAgentAssetVersion?.shape_program ?? null,
    materialPresets,
    quickMaterialPresets,
    appearanceMaterialId,
    selectedPartRoleLabel,
    selectedMaterialZoneId,
    hasSelectedAgentPart: Boolean(selectedAgentPart),
    selectedMaterialZoneIds: selectedAgentPart?.material_zone_ids ?? [],
    hasAgentAssetVersion: Boolean(agentAssetVersion ?? activeAgentAssetVersion),
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
    // Empty submissions use the friendly notice: 请先在输入框描述想生成的 3D 概念，再发送给 Agent。
    if (chatInputTrimmedEmpty) {
      setChatInput(COMPOSER_DEFAULT_BEGINNER_PROMPT)
    }
    focusComposerInput()
  }, [chatInputTrimmedEmpty, focusComposerInput, setChatInput])
  const hasActiveProject = Boolean(concept.project)
  const hasSnapshotForPreview = Boolean(
    activeDesignSnapshot
    && activeDesignSnapshot.project_id === concept.project?.project_id
    && activeDesignSnapshot.active_design.source === 'agent_asset'
  )
  const hasActiveChangeSet = Boolean(agentAssetChangeSet)
  const isWorkflowNetworkBusy = Boolean(
    !concept.error
    && !activeDesignState.error?.message
    && (
      (concept.loading && latestAgentRequestId > 0)
      || activeDesignState.operation !== 'idle'
      || candidatePbrCaptureStatus === 'capturing'
      || candidatePbrCaptureStatus === 'authorizing'
    )
  )
  const isWorkflowComputeBusy = singleResultDecisionPresentation.presentation.state === 'processing'
  const isWorkflowExportBusy = Boolean(
    agentRenderPresentation.renderLoading || agentRenderPresentation.renderPackageLoading,
  )
  const isWorkflowBusy = isWorkflowNetworkBusy || isWorkflowComputeBusy || isWorkflowExportBusy
  const workflowBusyStatus: CadWorkbenchWorkflowStep['status'] = isWorkflowNetworkBusy ? 'network' : 'processing'
  const hasActiveModeError = Boolean(
    concept.error
    || activeDesignState.error?.message,
  )
  const workflowErrorHintByMode: Record<WorkbenchPanelWorkflowMode, string> = {
    generate: '模型生成失败，请重试。',
    modify: '模型修改失败，请重试。',
    preview: '展示生成失败，请重新尝试。',
    export: '导出失败，请重新尝试。',
  }
  const workflowActiveModeHint = hasActiveModeError
    ? workflowErrorHintByMode[workbenchMode]
    : '当前步骤还在处理，请稍后再试。'
  const inFlightDecisionHint = singleResultDecisionPresentation.presentation.state === 'processing'
    ? singleResultDecisionPresentation.presentation.detail?.trim()
    : undefined
  const isCurrentAgentRequest = latestAgentRequestId > 0
    && isCurrentAgentConversationRequest(concept.project?.project_id ?? null, latestAgentRequestId)
  const generateRunHint = isWorkflowBusy && isCurrentAgentRequest
    ? '需求已提交，AI 正在生成当前版本。'
    : inFlightDecisionHint
      ? inFlightDecisionHint
      : '已接收输入，AI 正在生成模型。'
  const modifyRunHint = inFlightDecisionHint
    ? inFlightDecisionHint
    : 'AI 修改已提交，正在优化当前模型。'
  const hasSelectedComponent = Boolean(displayedAgentSelectedPartId)
  const isProjectUnsaved = hasActiveProject && hasActiveChangeSet
  const isProjectSyncing = !concept.error
    && !activeDesignState.error?.message
    && (concept.loading
    || isWorkflowBusy
    || activeDesignState.operation !== 'idle')
  const projectAutosaveState: 'empty' | 'busy' | 'dirty' | 'clean' = !hasActiveProject
    ? 'empty'
    : isProjectUnsaved
      ? 'dirty'
      : isProjectSyncing
        ? 'busy'
        : 'clean'
  const headerAutosaveText = hasActiveProject
    ? isProjectUnsaved
      ? '当前项目未保存'
      : isProjectSyncing
        ? '系统正在处理'
        : '已自动保存'
    : '未创建项目'
  const canGenerateMode = true
  const hasGenerateCompletion = hasSnapshotForPreview
  const canModifyMode = hasActiveProject && hasGenerateCompletion
  const canPreviewMode = hasActiveProject && hasSnapshotForPreview
  const hasExportableAgentVersion = Boolean(
    activeAgentAssetVersion
    && !agentAssetChangeSet
    && activeDesignState.operation === 'idle'
  )
  const canExportMode = hasActiveProject && hasExportableAgentVersion
  const hasModifyTurn = useMemo(() => (
    agentKernelItems.filter((item) => item.item_type === 'user_message').length >= 2
  ), [agentKernelItems])
  const workflowTechnicalMessage = useMemo(() => {
    const rawError = concept.error ?? activeDesignState.error?.message
    if (!rawError) return null
    return rawError
  }, [activeDesignState.error?.message, concept.error])
  const workflowState = useMemo((): Record<WorkbenchPanelWorkflowMode, CadWorkbenchWorkflowStep> => ({
      generate: {
        status: hasActiveModeError && workbenchMode === 'generate'
          ? 'error'
        : isWorkflowBusy && workbenchMode === 'generate'
          ? workflowBusyStatus
        : !hasActiveProject && !isWorkflowBusy
          ? 'empty'
          : workbenchMode === 'generate'
              ? 'active'
              : hasGenerateCompletion
                ? 'done'
                : 'ready',
        hint: hasActiveModeError && workbenchMode === 'generate'
        ? workflowActiveModeHint
        : !hasActiveProject && !isWorkflowBusy
          ? '先创建项目，再输入一句话开始生成。'
          : isWorkflowBusy && workbenchMode === 'generate'
            ? (workflowBusyStatus === 'network'
              ? '网络不稳定，请稍后再试。'
              : generateRunHint)
            : hasGenerateCompletion
              ? '模型已生成，建议继续补充描述优化效果。'
              : '请先输入设计需求，开始 AI 生成。',
      },
      modify: {
        status: !canModifyMode
          ? 'blocked'
        : hasActiveModeError && workbenchMode === 'modify'
          ? 'error'
        : workbenchMode === 'modify' && isWorkflowBusy
          ? workflowBusyStatus
          : hasActiveChangeSet
            ? 'saving'
            : workbenchMode === 'modify'
              ? 'active'
              : hasModifyTurn
                ? 'done'
                : 'ready',
        hint: !canModifyMode
        ? '先完成一次生成后可进入修改。'
        : hasActiveModeError && workbenchMode === 'modify'
          ? workflowActiveModeHint
        : workbenchMode === 'modify' && isWorkflowBusy
          ? (workflowBusyStatus === 'network' ? '网络不稳定，正在等待重试。' : modifyRunHint)
            : hasActiveChangeSet
            ? '有未确认修改，先保存后再继续。'
            : hasModifyTurn
              ? '可继续发起一次修改指令，按需细化局部。'
              : '生成已完成，可发起第一条修改。',
      },
      preview: {
        status: !canPreviewMode
          ? 'blocked'
        : hasActiveModeError && workbenchMode === 'preview'
          ? 'error'
        : workbenchMode === 'preview' && isWorkflowBusy
          ? workflowBusyStatus
          : hasActiveChangeSet
            ? 'saving'
            : workbenchMode === 'preview'
              ? 'active'
              : hasSnapshotForPreview
                ? 'done'
                : 'ready',
        hint: !canPreviewMode
          ? '请先确认一个可展示版本后再进入展示。'
          : hasActiveModeError && workbenchMode === 'preview'
            ? workflowActiveModeHint
        : workbenchMode === 'preview' && isWorkflowBusy
              ? (workflowBusyStatus === 'network'
                ? '展示刷新遇到网络问题，请稍后。'
                : '展示刷新中...')
            : hasActiveChangeSet
            ? '有未确认变更，先保存到版本后再展示。'
            : hasSnapshotForPreview
              ? '当前版本可展示。'
              : '请先确认并创建可展示版本。',
      },
      export: {
        status: !canExportMode
          ? hasActiveChangeSet
            ? 'saving'
            : 'blocked'
        : hasActiveModeError && workbenchMode === 'export'
          ? 'error'
        : workbenchMode === 'export' && isWorkflowBusy
          ? workflowBusyStatus
          : workbenchMode === 'export'
            ? 'active'
            : 'done',
        hint: !canExportMode
          ? hasActiveChangeSet
            ? '有未确认改动，先确认后再导出。'
            : '请先确认并保存可导出版本。'
          : hasActiveModeError && workbenchMode === 'export'
            ? workflowActiveModeHint
          : workbenchMode === 'export' && isWorkflowBusy
              ? (workflowBusyStatus === 'network' ? '网络不稳定，导出链路暂缓。' : '导出进行中...')
              : canExportMode
                ? '可导出当前版本；建议先确认交付格式与材质设置。'
                : '导出前请先确认并保存当前版本。',
      },
  }), [
    activeDesignState.error?.message,
    activeDesignState.operation,
    canExportMode,
    canModifyMode,
    canPreviewMode,
    hasModifyTurn,
    hasGenerateCompletion,
    concept.error,
    hasActiveChangeSet,
    hasActiveProject,
    hasSnapshotForPreview,
    generateRunHint,
    singleResultDecisionPresentation.presentation.state,
    isWorkflowBusy,
    isWorkflowComputeBusy,
    isWorkflowNetworkBusy,
    workflowBusyStatus,
    workbenchMode,
  ])
  const enterWorkflowMode = useCallback((nextMode: WorkbenchPanelWorkflowMode) => {
    if (nextMode === 'generate') {
      setShowComposerAdvancedActions(true)
      setAssistantMode('brief')
      if (!concept.project) {
        setAssistantNote('已切换到 AI 生成。正在准备项目。')
        void concept.createStarterProject()
          .then((created) => {
            if (created) {
              window.requestAnimationFrame(() => focusComposerInput())
            } else {
              setAssistantNote('项目创建没有完成，请检查本地服务后重试。')
            }
          })
      } else {
        setAssistantNote('已切换到 AI 生成。你可以在输入框继续描述新需求。')
        focusComposerInput()
      }
      return
    }
    if (nextMode === 'modify') {
      setShowComposerAdvancedActions(true)
      if (!concept.project) {
        setAssistantNote('正在创建项目，随后进入修改流程。')
        void concept.createStarterProject()
          .then((created) => {
            if (created) {
              window.requestAnimationFrame(() => focusComposerInput())
            } else {
              setAssistantNote('项目创建没有完成，请检查本地服务后重试。')
            }
          })
      } else {
        setAssistantNote('已切换到修改。你可以在输入框追加细节。')
        focusComposerInput()
      }
      return
    }
    if (nextMode === 'preview') {
      setShowComposerAdvancedActions(false)
      if (!hasSnapshotForPreview) {
        setAssistantNote('请先生成并确认一次版本，再进入展示模式。')
        return
      }
      setAssistantNote(hasActiveChangeSet ? '当前存在未确认修改；已切换到展示，仅查看可确认版本。' : '已切换到展示。当前资产预览已聚焦。')
      return
    }
    setShowComposerAdvancedActions(false)
    if (!canExportMode) {
      setAssistantNote('当前不可导出：请先完成一个已确认版本后再导出。')
      return
    }
    openExportDrawer()
  }, [
    canExportMode,
    concept.createStarterProject,
    concept.project,
    focusComposerInput,
    hasActiveChangeSet,
    hasSnapshotForPreview,
    openExportDrawer,
    setAssistantMode,
    setAssistantNote,
  ])

  const handleModeSelect = useCallback((nextMode: WorkbenchPanelWorkflowMode) => {
    const sameMode = workbenchMode === nextMode
    const nextModeStatus = workflowState[nextMode].status

    if (nextMode === 'export') {
      if (nextModeStatus === 'blocked' || !canExportMode) {
        setAssistantNote('当前不可导出：请先完成一个已确认版本后再导出。')
        return
      }
      if (isWorkflowBusy) {
        setAssistantNote('当前阶段仍在执行中，完成后可切换到该步骤。')
        return
      }
      setWorkbenchMode('export')
      enterWorkflowMode('export')
      openExportDrawer()
      return
    }

    if (sameMode) {
      if (isWorkflowBusy) {
        setAssistantNote('当前阶段仍在执行中，完成后可继续。')
        return
      }

      if (nextMode === 'generate' || nextMode === 'modify') {
        focusComposerInput()
      }
      return
    }

    if (nextModeStatus === 'blocked') {
      setAssistantNote(`当前步骤未就绪：${workflowState[nextMode].hint}`)
      return
    }
    if (isWorkflowBusy && nextModeStatus !== 'done') {
      setAssistantNote('当前阶段仍在执行中，完成后可切换。')
      return
    }
    setWorkbenchMode(nextMode)
    enterWorkflowMode(nextMode)
  }, [
    canExportMode,
    enterWorkflowMode,
    focusComposerInput,
    workflowState,
    isWorkflowBusy,
    workbenchMode,
    setAssistantNote,
    openExportDrawer,
  ])
  useEffect(() => {
    if (workbenchMode === 'modify' && !canModifyMode) {
      void handleModeSelect('generate')
      return
    }
    if (workbenchMode === 'preview' && !canPreviewMode) {
      if (canModifyMode) {
        void handleModeSelect('modify')
      } else if (!concept.loading) {
        void handleModeSelect('generate')
      }
      return
    }
    if (workbenchMode === 'export' && !canExportMode) {
      if (canPreviewMode) {
        void handleModeSelect('preview')
      } else if (canModifyMode) {
        void handleModeSelect('modify')
      } else if (!concept.loading) {
        void handleModeSelect('generate')
      }
    }
  }, [
    canExportMode,
    canModifyMode,
    canPreviewMode,
    concept.loading,
    handleModeSelect,
    workbenchMode,
  ])
  const toggleComposerAdvancedActions = useCallback(
    () => {
      if (showComposerAdvancedActions) {
        setShowComposerAdvancedActions(false)
        if (hasSnapshotForPreview) {
          void handleModeSelect('preview')
        } else {
          setAssistantNote('已回到新手模式；输入一句话即可开始生成。')
        }
        return
      }
      // An empty project has no modifiable asset yet. Opening the advanced
      // authoring surface is still valid; routing it through the guarded
      // "modify" workflow used to make this button appear inert until the
      // first asset already existed.
      setShowComposerAdvancedActions(true)
      setAssistantMode('brief')
      setAssistantNote('已打开进阶模式。你可以直接描述任意对象、结构与外观要求。')
      window.requestAnimationFrame(() => focusComposerInput())
    },
    [focusComposerInput, handleModeSelect, hasSnapshotForPreview, setAssistantMode, setAssistantNote, showComposerAdvancedActions],
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
  const openSidebarSettings = useCallback(() => {
    setProviderSetupOpen(true)
    setAssistantNote('已打开 Provider 设置，用于核对模型提供商和运行参数。')
  }, [setAssistantNote, setProviderSetupOpen])
  const openSidebarHelp = useCallback(() => {
    setAssistantNote('建议路线：AI 生成 → 修改 → 展示 → 导出。每个阶段都有撤销与质量校验。')
  }, [setAssistantNote])
  const isComposerReady = Boolean(concept.project) && !concept.loading
  const composerSending = !concept.error
    && !activeDesignState.error?.message
    && (
      concept.loading
      && latestAgentRequestId > 0
      || agentBlockoutDisplay.directionPreviewLoading
      || candidatePbrCaptureStatus === 'capturing'
      || singleResultDecisionPresentation.presentation.state === 'processing'
    )

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
  const handleTemplateStart = useCallback((template: string) => {
    void handleModeSelect('generate')
    const prompt = `请以“${template}”作为一个可改写的示例，生成对应对象的 3D 外观模型。保持用户实际描述的对象身份，不套用机械臂、机器人或其他固定模板；先理解对象，再选择当前可执行的表示能力。`
    setChatInput(prompt)
    focusComposerInput()
  }, [focusComposerInput, handleModeSelect, setChatInput])

  const runWithStarterProject = useCallback(async (runner: () => void) => {
    setAssistantNote(isProjectUnsaved
      ? '有未保存修改：将新建独立项目，不会替换当前未确认草稿。'
      : '正在创建新项目，请稍候。')
    try {
      const created = await concept.createStarterProject()
      if (!created) {
        setAssistantNote('项目创建没有完成，请检查本地服务后重试。')
        setIsCreateSetupOpen(true)
        return
      }
      runner()
    } catch {
      setAssistantNote('新建项目失败，请重试。')
      setIsCreateSetupOpen(true)
    }
  }, [concept.createStarterProject, isProjectUnsaved, setAssistantNote])

  const openCreateSetup = useCallback(() => {
    if (concept.loading || isWorkflowBusy) {
      setAssistantNote('当前正在处理中，完成后再创建新项目。')
      return
    }
    setIsCreateSetupOpen(true)
  }, [concept.loading, isWorkflowBusy, setAssistantNote])

  const closeCreateSetup = useCallback(() => {
    setIsCreateSetupOpen(false)
  }, [])

  const handleCreateByPrompt = useCallback(() => {
    setChatInput(createPrompt.trim() || COMPOSER_DEFAULT_BEGINNER_PROMPT)
    closeCreateSetup()
    void runWithStarterProject(() => {
      void handleModeSelect('generate')
      window.requestAnimationFrame(() => focusComposerInput())
    })
  }, [closeCreateSetup, createPrompt, focusComposerInput, handleModeSelect, runWithStarterProject, setChatInput])

  const handleCreateByReference = useCallback(() => {
    closeCreateSetup()
    void runWithStarterProject(() => {
      void handleModeSelect('generate')
      handleReferenceAction()
    })
  }, [closeCreateSetup, handleModeSelect, handleReferenceAction, runWithStarterProject])

  const handleCreateByTemplate = useCallback((template: string) => {
    closeCreateSetup()
    void runWithStarterProject(() => {
      handleTemplateStart(template)
    })
  }, [closeCreateSetup, handleTemplateStart, runWithStarterProject])

  const handleQuickModify = useCallback(async (instruction: string) => {
    if (!canModifyMode) return
    void handleModeSelect('modify')
    await submitAssistantChangeInstructionWithText(instruction)
  }, [canModifyMode, handleModeSelect, submitAssistantChangeInstructionWithText])

  const handleVersionCompare = useCallback((threadId: string) => {
    const title = agentThreads.find((thread) => thread.thread_id === threadId)?.title || '历史会话'
    setHistoryPreview({
      threadId,
      returnThreadId: agentThreadId !== threadId ? agentThreadId : null,
      mode: 'compare',
      title,
    })
    setAssistantNote('已打开该版本对应的历史会话用于对比；当前资产版本保持不变。')
    void handleModeSelect('preview')
    void selectConversationThread(threadId)
  }, [agentThreadId, agentThreads, handleModeSelect, selectConversationThread, setAssistantNote])

  const handleVersionRestore = useCallback((threadId: string) => {
    const title = agentThreads.find((thread) => thread.thread_id === threadId)?.title || '历史会话'
    setHistoryPreview({
      threadId,
      returnThreadId: agentThreadId !== threadId ? agentThreadId : null,
      mode: 'restore',
      title,
    })
    setAssistantNote('已打开该版本对应的历史会话；当前资产版本未被覆盖，请确认后再继续编辑。')
    void handleModeSelect('preview')
    void selectConversationThread(threadId)
  }, [agentThreadId, agentThreads, handleModeSelect, selectConversationThread, setAssistantNote])

  const exitHistoryPreview = useCallback(() => {
    const returnThreadId = historyPreview?.returnThreadId
    setHistoryPreview(null)
    setAssistantNote('已返回当前会话；当前资产版本未修改。')
    if (returnThreadId) void selectConversationThread(returnThreadId)
  }, [historyPreview, selectConversationThread, setAssistantNote])

  const handleWorkflowModeSelectFromUi = useCallback((mode: WorkbenchPanelWorkflowMode) => {
    if (historyPreview && mode !== 'preview') {
      setAssistantNote('请先退出历史会话预览，再继续修改、展示或导出当前设计。')
      return
    }
    void handleModeSelect(mode)
  }, [handleModeSelect, historyPreview, setAssistantNote])

  const saveFromKeyboard = useCallback(() => {
    if (historyPreview) {
      setAssistantNote('请先退出历史会话预览，再确认当前版本。')
      return
    }
    const presentation = singleResultDecisionPresentation.presentation
    if (presentation.state === 'ready') {
      void confirmSingleResultPreview(presentation.decision)
      return
    }
    if (agentAssetChangeSet) {
      void confirmAgentAssetEdit()
      return
    }
    setAssistantNote('当前没有待确认的修改。')
  }, [agentAssetChangeSet, confirmAgentAssetEdit, confirmSingleResultPreview, historyPreview, setAssistantNote, singleResultDecisionPresentation.presentation])

  const focusSelectedComponentFromKeyboard = useCallback(() => {
    if (!displayedAgentSelectedPartId) {
      setAssistantNote('先选择一个组件，再按 F 聚焦。')
      return
    }
    setFocusAgentPartId(displayedAgentSelectedPartId)
    setFocusAgentPartRequest((current) => current + 1)
    setAssistantNote(`已聚焦${selectedPartRoleLabel || '当前组件'}。`)
  }, [displayedAgentSelectedPartId, selectedPartRoleLabel, setAssistantNote])

  const { isListening: voiceInputListening, toggle: toggleVoiceInput } = useCadWorkbenchVoiceInput({
    onTranscript: setChatInput,
    onNotice: setAssistantNote,
  })

  const closeFromKeyboard = useCallback(() => {
    if (isCreateSetupOpen) {
      closeCreateSetup()
      return true
    }
    if (hasOpenDrawer) {
      closeAllDrawers()
      return true
    }
    if (viewportDock.dockState === 'focus') {
      closeViewportFocus()
      return true
    }
    if (historyPreview) {
      exitHistoryPreview()
      return true
    }
    if (presentedAgentSelectedPartId || presentedSelectedAgentPart) {
      setIsSelectionDismissed(true)
      setFocusAgentPartId(null)
      setAssistantNote('已取消当前视图选择；资产版本未修改。')
      return true
    }
    return false
  }, [closeAllDrawers, closeCreateSetup, closeViewportFocus, exitHistoryPreview, hasOpenDrawer, historyPreview, isCreateSetupOpen, presentedSelectedAgentPart, presentedAgentSelectedPartId, setAssistantNote, viewportDock.dockState])

  useCadWorkbenchPanelKeyboardShortcuts({
    onUndo: () => void navigateAgentAsset('undo'),
    onRedo: () => void navigateAgentAsset('redo'),
    onSave: saveFromKeyboard,
    onFocusSelectedComponent: focusSelectedComponentFromKeyboard,
    onEscape: closeFromKeyboard,
  })

  return (
    <div
      className="cad-workbench"
      data-testid="cad-workbench"
      data-workbench-mode={workbenchMode}
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
      {isCreateSetupOpen ? (
        <div
          className="f026-create-setup-overlay"
          role="presentation"
          onMouseDown={closeCreateSetup}
          onKeyDown={(event) => {
            if (event.key === 'Escape') {
              event.preventDefault()
              closeCreateSetup()
            }
          }}
        >
          <section
            className="f026-create-setup-panel"
            role="dialog"
            aria-modal="true"
            aria-labelledby="f026-create-setup-title"
            aria-describedby="f026-create-setup-copy"
            onMouseDown={(event) => event.stopPropagation()}
          >
            <header className="f026-create-setup-header">
              <div>
                <h2 id="f026-create-setup-title">你想设计什么？</h2>
                <p id="f026-create-setup-copy">你想设计什么？输入目标对象和风格需求，AI 即可开始第一版生成。</p>
              </div>
              <button
                type="button"
                className="f026-create-setup-close"
                onClick={closeCreateSetup}
                aria-label="关闭创建面板"
              >
                <X size={16} />
              </button>
            </header>
            <label className="f026-create-setup-prompt">
              <span>一句话描述你的设计</span>
              <textarea
                aria-label="新建设计需求"
                value={createPrompt}
                onChange={(event) => setCreatePrompt(event.target.value)}
                placeholder="例如：设计一台用于城市废墟搜索的履带式救援机器人。"
                rows={3}
              />
              <small>可以先说用途、外观或风格，专业参数之后再补充。</small>
            </label>
            <div className="f026-create-setup-actions">
              <button
                type="button"
                className="f026-create-setup-option"
                onClick={handleCreateByPrompt}
              >
                <Sparkle size={16} aria-hidden="true" />
                <div>
                  <strong>开始设计</strong>
                  <small>从一句话直接输入目标需求，立即进入生成。</small>
                </div>
              </button>
              <button
                type="button"
                className="f026-create-setup-option"
                onClick={handleCreateByReference}
              >
                <FolderOpen size={16} aria-hidden="true" />
                <div>
                  <strong>参考图启动</strong>
                  <small>先添加参考图，AI 会按参考图生成外观构思。</small>
                </div>
              </button>
            </div>
            <div className="f026-create-setup-examples">
              <div className="f026-create-setup-examples-title">推荐样例（可直接改写）</div>
              <div className="f026-create-setup-examples-list" role="list" aria-label="推荐样例">
                {CREATE_PROJECT_EXAMPLES.map((example) => (
                  <button
                    key={example}
                    type="button"
                    className="f026-create-setup-example"
                    role="listitem"
                    onClick={() => setCreatePrompt(example)}
                  >
                    {example}
                  </button>
                ))}
              </div>
            </div>
            <div className="f026-create-setup-template-block">
              <div className="f026-create-setup-template-title">从示例开始</div>
              <div className="f026-create-setup-template-grid" role="list" aria-label="示例入口">
                {CREATE_PROJECT_TEMPLATES.map((template) => (
                  <button
                    type="button"
                    key={template}
                    className="f026-create-setup-template-item"
                    role="listitem"
                    title={`从${template}示例开始设计`}
                    onClick={() => handleCreateByTemplate(template)}
                  >
                    <Cube size={14} aria-hidden="true" />
                    {template}
                  </button>
                ))}
              </div>
            </div>
          </section>
        </div>
      ) : null}
      <header className="cad-command-bar">
        <div className="cad-brand" aria-label="CAD 工作台">
          <span className="cad-brand-mark"><Cube size={18} weight="fill" /></span>
          <span>ForgeCAD</span>
        </div>
        <div className="cad-workspace-title" aria-label="当前项目">
            <strong>{concept.project?.name ?? '新概念设计'}</strong>
          <span role="status" aria-live="polite" aria-label="工程保存状态">
            <span className={`cad-autosave cad-autosave--${projectAutosaveState}`}>
              {hasActiveProject ? (
                projectAutosaveState === 'busy' ? (
                  <SpinnerGap size={12} className="cad-global-actions-spin" />
                ) : projectAutosaveState === 'dirty' ? (
                  <DotsThreeCircle size={10} />
                ) : (
                  <CheckCircle size={10} weight="fill" />
                )
              ) : (
                <DotsThreeCircle size={10} />
              )}
              {headerAutosaveText}
            </span>
          </span>
        </div>
        <div className="cad-global-actions" aria-label="工作区操作">
          {isMobileLayout ? (
            <button
              type="button"
              className={`cad-sidebar-toggle ${isMobileSidebarOpen ? 'is-open' : ''}`}
              aria-label={isMobileSidebarOpen ? '关闭我的设计侧栏' : '打开我的设计侧栏'}
              title={isMobileSidebarOpen ? '关闭我的设计侧栏' : '打开我的设计侧栏'}
              aria-expanded={isMobileSidebarOpen}
              onClick={() => {
                setIsMobileSidebarOpen((open) => !open)
                setIsMobileAssistantOpen(false)
              }}
            >
              <List size={16} />
              <span>我的设计</span>
            </button>
          ) : null}
          {isMobileLayout ? (
            <button
              type="button"
              className={`cad-assistant-toggle ${isMobileAssistantOpen ? 'is-open' : ''}`}
              aria-label={isMobileAssistantOpen ? '关闭 AI 设计助手' : '打开 AI 设计助手'}
              title={isMobileAssistantOpen ? '关闭 AI 设计助手' : '打开 AI 设计助手'}
              aria-expanded={isMobileAssistantOpen}
              onClick={() => {
                setIsMobileAssistantOpen((open) => !open)
                setIsMobileSidebarOpen(false)
              }}
            >
              <Sparkle size={16} weight="fill" />
              <span>AI助手</span>
            </button>
          ) : null}
          <CadWorkbenchPanelGlobalActions
            actions={globalPanelActions}
            activeMode={workbenchMode}
            workflowState={workflowState}
            onUndo={() => void navigateAgentAsset('undo')}
            onRedo={() => void navigateAgentAsset('redo')}
            onImport={importFromActionBar}
            onCheck={openQualityDrawer}
            onModeSelect={handleWorkflowModeSelectFromUi}
            onOpenAdvanced={() => {
              void handleModeSelect('modify')
            }}
            canGenerateMode={canGenerateMode}
            canModifyMode={canModifyMode}
            canPreviewMode={canPreviewMode}
            canExportMode={canExportMode}
            canCheck={Boolean(activeAgentAssetVersion ?? agentAssetVersion)}
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
        className={`cad-layout f026-layout ${
          viewportDock.dockState === 'focus' ? 'is-viewport-focus' : ''} ${
          showComposerAdvancedActions ? '' : 'f026-layout-beginner'
        } ${
          isMobileLayout ? 'is-mobile' : ''
        } ${
          isMobileSidebarOpen ? 'is-sidebar-open' : ''
        } ${
          isMobileAssistantOpen ? 'is-assistant-open' : ''
        } ${
          isSidebarCollapsed ? 'is-sidebar-collapsed' : ''
        } ${
          isAssistantCollapsed ? 'is-assistant-collapsed' : ''
        }`}
        data-viewport-dock-state={viewportDock.dockState}
      >
          {isMobileLayout && (isMobileSidebarOpen || isMobileAssistantOpen) ? (
            <button
              type="button"
              className="cad-sidebar-backdrop"
              aria-label="关闭侧栏"
              onClick={() => {
                setIsMobileSidebarOpen(false)
                setIsMobileAssistantOpen(false)
              }}
            />
          ) : null}
          <WorkbenchSidebar
            projects={concept.projects}
            activeProjectId={concept.project?.project_id ?? null}
            threads={agentThreads}
            activeThreadId={agentThreadId}
            parts={sidebarParts}
            selectedPartId={presentedAgentSelectedPartId}
            loading={concept.loading || threadHistoryLoading}
            compactMode={isMobileLayout || shouldCompactSidebar}
            onToggle={isMobileLayout ? () => setIsMobileSidebarOpen(false) : undefined}
            onCollapse={isMobileLayout ? undefined : () => setIsSidebarCollapsed(true)}
            onCreateProject={openCreateSetup}
            onSelectProject={(projectId) => {
              setHistoryPreview(null)
              void concept.selectProject(projectId)
            }}
            onSelectThread={handleConversationThreadSelect}
            onSelectPart={(partId) => void selectAgentPartFromUi(partId)}
            onUploadReference={handleReferenceAction}
            onTemplateSelect={handleTemplateStart}
            onOpenFromTemplatePrompt={handleTemplateStart}
            onOpenSettings={openSidebarSettings}
            onOpenHelp={openSidebarHelp}
          />

        <main className="f026-conversation-stage" aria-label="AI 设计助手工作区">
          <div className="f026-conversation-scroll">
          <section className="f026-agent-timeline">
            <div className="cad-panel-title">
              <span><Sparkle size={16} weight="fill" /> AI设计助手</span>
              <span className="assistant-state" role="status" aria-live="polite">
                {hasActiveModeError ? '需要重试' : isWorkflowBusy ? '正在工作' : '准备就绪'}
              </span>
              <button
                type="button"
                className="f026-assistant-collapse"
                onClick={() => {
                  if (isMobileLayout) setIsMobileAssistantOpen(false)
                  else setIsAssistantCollapsed(true)
                }}
                aria-label={isMobileLayout ? '关闭 AI 设计助手' : '收起 AI 设计助手'}
                title={isMobileLayout ? '关闭 AI 设计助手' : '收起 AI 设计助手'}
              >
                <ArrowLeft size={14} aria-hidden="true" />
              </button>
            </div>
            <AgentConversation
              showAdvancedControls={showComposerAdvancedActions}
              loading={composerSending}
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
              compatibilityDecisionRejected={singleResultDecisionPresentation.presentation.state === 'failed'}
              blockoutPreviewPresentation={blockoutPreviewPresentation}
              agentPlanSourcePresentation={agentPlanSourcePresentation}
              conceptFamilySuggestions={CONCEPT_FAMILY_SUGGESTIONS}
              presentationProfile={presentationProfile}
              styleOptionsOpen={styleOptionsOpen}
              onQuickModify={handleQuickModify}
              canQuickModify={canModifyMode}
              onFocusComposer={focusComposerInput}
              onAssistantModeChange={setAssistantMode}
              onSuggestionSelect={setChatInput}
              onPresentationProfileChange={handleConversationProfileChange}
              onClarificationSelect={(option) => void submitAssistantInstructionWithText(
                `${agentClarification?.originalMessage ? `${agentClarification.originalMessage}\n` : ''}${option.prompt}`,
                option.domain_pack_id,
                undefined,
                gameAssetDelivery,
              )}
              agentClarification={agentClarification}
              agentKernelItems={agentKernelItems}
              agentKernelUnavailable={agentKernelUnavailable}
              agentPlan={agentPlan}
              candidatePreviewQualityPresentation={candidatePreviewQualityPresentation}
            />
            {singleResultDecisionPresentation.presentation.state === 'failed'
              && singleResultDecisionPresentation.presentation.error?.includes('没有返回正式的单一结果决策')
              && hasAgentToolInvocation(agentKernelItems, 'plan_complete_concept') ? (
              <section className="agent-kernel-events compatibility-decision-evidence" role="log" aria-label="兼容规划证据">
                <div className="agent-kernel-events-title">
                  <span>兼容规划证据</span>
                  <small>本次旧适配器未形成正式结果，不代表 3D 资产</small>
                </div>
                <div className="agent-kernel-event status-completed" data-agent-item-type="tool_call">
                  <span className="visually-hidden">工具 tool_call</span>
                  <div className="agent-kernel-event-heading"><strong>兼容规划调用</strong><span>完成</span></div>
                  <div className="agent-kernel-event-meta"><small>工具动作：plan_complete_concept</small><small>输入证据：文字设计需求</small></div>
                </div>
                <div className="agent-kernel-event status-completed" data-agent-item-type="tool_result">
                  <span className="visually-hidden">结果 tool_result</span>
                  <div className="agent-kernel-event-heading"><strong>兼容规划结果</strong><span>完成</span></div>
                  <div className="agent-kernel-event-meta"><small>已返回外观规划，未进入正式 3D 结果链。</small></div>
                </div>
              </section>
            ) : null}
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
              conceptLoading={concept.loading && !concept.error}
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
              decisionContractFailure={singleResultDecisionPresentation.presentation.state === 'failed'}
              onSaveCompatibility={activeAgentAssetVersion ? null : commitAgentBlockout}
          />
            <CadWorkbenchPanelSelectionTools
              agentSelectionCardProps={agentSelectionCardProps}
              materialOptionsProps={materialOptionsProps}
              showSelectionTools={showComposerAdvancedActions || Boolean(selectedAgentPart) || Boolean(agentSelectionCardProps)}
              showMaterialOptions={showComposerAdvancedActions || Boolean(activeAgentAssetVersion)}
              expandResultDetails={Boolean(activeAgentAssetVersion || showComposerAdvancedActions)}
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
          <button
            type="button"
            className="f026-quality-check-entry"
            aria-label="检查"
            disabled={!isComposerReady}
            onClick={openQualityDrawer}
          >
            检查
          </button>
          <button
            type="button"
            className="f026-quality-check-entry"
            aria-label="导出"
            disabled={!isComposerReady}
            onClick={openExportDrawer}
          >
            导出
          </button>
          {!showComposerAdvancedActions ? (
            <section className="f026-mini-toolbelt" aria-label="创作工具快速入口">
              <div className="f026-mini-toolbelt-menu">
                <button
                  type="button"
                  className="f026-mini-toolbelt-action"
                  disabled={!isComposerReady}
                  title="调整整体外观风格"
                  onClick={handleStyleAction}
                >
                  换外观
                </button>
                <button
                  type="button"
                  className="f026-mini-toolbelt-action"
                  disabled={!isComposerReady}
                  title="为当前设计选择材质"
                  onClick={handleMaterialAction}
                >
                  换材质
                </button>
                <button
                  type="button"
                  className="f026-mini-toolbelt-action"
                  disabled={!isComposerReady}
                  title="添加参考图片或视觉线索"
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
            onOpenTemplate={openCreateSetup}
            onToggleVoice={toggleVoiceInput}
            voiceListening={voiceInputListening}
            onOpenSurfaceAdornment={handleSurfaceAdornmentAction}
            surfaceAdornmentDisabled={Boolean(surfaceAdornmentDisabledReason)}
            surfaceAdornmentDetail={surfaceAdornmentDisabledReason ?? undefined}
            gameAssetDeliveryProfile={gameAssetDeliveryProfile}
            onGameAssetDeliveryProfileChange={setGameAssetDeliveryProfile}
          />
        </main>

        <section className="cad-center-stage f026-viewport-stage" aria-label="3D 工作区">
          {isWorkflowBusy ? (
            <div className="cad-viewport-processing-overlay" role="status" aria-live="polite">
              <span className="cad-viewport-processing-spinner" aria-hidden="true" />
              <span>
                <strong>{workflowState[workbenchMode]?.status === 'saving' ? '正在保存当前设计' : workflowBusyStatus === 'network' ? '正在等待网络恢复' : 'AI 正在处理设计'}</strong>
                <small>当前已确认版本不会被覆盖</small>
              </span>
            </div>
          ) : null}
          {historyPreview ? (
            <div className="cad-viewport-history-banner" role="status" aria-live="polite">
              <span>
                <strong>{historyPreview.mode === 'compare' ? '历史会话对比预览' : '历史会话恢复预览'}</strong>
                <small>{historyPreview.title} · 当前设计版本未改变</small>
              </span>
              <button type="button" onClick={exitHistoryPreview}>返回当前会话</button>
            </div>
          ) : null}
          {isSidebarCollapsed ? (
            <button
              type="button"
              className="f026-sidebar-reopen"
              onClick={() => setIsSidebarCollapsed(false)}
              aria-label="打开我的设计侧栏"
              title="打开我的设计侧栏"
            >
              <List size={16} aria-hidden="true" />
              <span>我的设计</span>
            </button>
          ) : null}
          {isAssistantCollapsed ? (
            <button
              type="button"
              className="f026-assistant-reopen"
              onClick={() => setIsAssistantCollapsed(false)}
              aria-label="打开 AI 设计助手"
              title="打开 AI 设计助手"
            >
              <Sparkle size={15} weight="fill" aria-hidden="true" />
              <span>AI助手</span>
            </button>
          ) : null}
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
	            {!concept.project ? (
	              <section className="f026-viewport-empty-entry" aria-label="开始设计引导">
	                <header className="f026-viewport-empty-entry-header">
	                  <strong>从一句需求开始你的第一个 3D 设计</strong>
	                  <p>适合零基础用户：先输入“我要什么样的模型”，AI 会先生成外观预览，你可继续修改、查看展示并导出。</p>
	                </header>
	                <div className="f026-viewport-empty-entry-actions">
	                  <button
	                    type="button"
	                    className="f026-viewport-empty-action"
	                    onClick={() => {
                      void handleModeSelect('generate')
	                    }}
	                  >
	                    AI 一键生成
	                  </button>
                  <button
                    type="button"
	                    className="f026-viewport-empty-action is-secondary"
	                    onClick={() => {
	                      void handleCreateByReference()
	                    }}
	                  >
	                    上传参考图
	                  </button>
                  <button
                    type="button"
	                    className="f026-viewport-empty-action is-secondary"
	                    onClick={openCreateSetup}
	                  >
	                    浏览模板
	                  </button>
                </div>
              </section>
            ) : null}
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
                  title={tool.implemented ? tool.label : tool.unavailableReason}
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
              selectedAgentPartId={presentedAgentSelectedPartId}
              focusAgentPartId={focusAgentPartId}
              focusAgentPartRequest={focusAgentPartRequest}
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
              quickModifyPresets={QUICK_MODIFY_PRESETS}
              canQuickModify={canModifyMode}
              onQuickModify={handleQuickModify}
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

        {!shouldCompactSidebar
          && showComposerAdvancedActions
          && legacyDesignReadOnly ? (
          <WorkbenchInspectorRail
            className="f026-right-rail"
            mode={activeDesignSnapshot?.active_design.source === 'agent_asset'
              ? 'agent'
              : legacyDesignReadOnly
                ? 'legacy'
                : 'empty'}
            agentAssetVersion={activeAgentAssetVersion}
            agentQualityReport={agentQualityReport}
            selectedAgentPartId={presentedAgentSelectedPartId}
            selectedAgentPart={presentedSelectedAgentPart}
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
        <CadWorkbenchPanelStatusBar
          workbenchStatusBar={workbenchStatusBar}
          showCompactSidebar={shouldCompactSidebar}
          workflowState={workflowState}
          activeMode={workbenchMode}
          threadSummaries={agentThreads}
          activeThreadId={agentThreadId}
          historyPreview={historyPreview}
          onExitHistoryPreview={exitHistoryPreview}
          hasSelectedComponent={hasSelectedComponent && !isSelectionDismissed}
          isProjectUnsaved={isProjectUnsaved}
          technicalMessage={workflowTechnicalMessage}
          onModeSelect={handleWorkflowModeSelectFromUi}
          onThreadSelect={handleConversationThreadSelect}
          onVersionRestore={handleVersionRestore}
          onVersionCompare={handleVersionCompare}
        />
      </div>
    </div>
  )
}
