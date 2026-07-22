import { useCallback, useEffect, useMemo, useReducer, useRef, useState, type ChangeEvent } from 'react'
import {
  ArrowsClockwise,
  ArrowsOutCardinal,
  ArrowLeft,
  Check,
  ClockCounterClockwise,
  Crosshair,
  Cube,
  CursorClick,
  Export,
  FloppyDisk,
  FolderOpen,
  GridFour,
  House,
  MagnifyingGlass,
  Plus,
  Ruler,
  SelectionAll,
  Star,
  Sparkle,
  X,
} from '@phosphor-icons/react'
import { ForgeApiError, forgeApi, mapActiveDesignError } from '../../shared/api/forgeApi'
import type { ActiveDesignNavigation, AgentAssetChangeSet, AgentAssetQualityReport, AgentAssetRenderView, AgentAssetVersion, AgentComponentCandidate, AgentMaterialPreset, AgentPartEditOperation, AgentStructureSuggestion, AgentThreadSummary, AgentTurn, AssemblyDeltaProgram, MechanicalConceptPlan } from '../../shared/types'
import { useRuntime } from '../../app/providers/RuntimeProvider'
import {
  getProviderConfig as getTauriProviderConfig,
  saveProviderConfig as saveTauriProviderConfig,
  type ProviderConfigMetadata,
} from '../../shared/tauri/agentSupervisor'
import { ModuleGraphViewport, type ViewportMeasurementPoint } from './ModuleGraphViewport'
import { AgentConversation } from './AgentConversation'
import { AgentSelectionCard } from './AgentSelectionCard'
import { GenerationResultCard } from './GenerationResultCard'
import {
  initialSingleResultDecisionPresentationState,
  readSingleResultDecisionFromAgentItems,
  singleResultDecisionPresentationReducer,
  type SingleResultDecision,
  type SingleResultReadyDecision,
} from './singleResultDecisionPresentationState'
import { WorkbenchComposer } from './WorkbenchComposer'
import { WorkbenchSidebar } from './WorkbenchSidebar'
import { selectAgentBlockoutPreviewPresentation } from './agentBlockoutPreviewPresentation'
import { selectAgentPlanSourcePresentation } from './agentPlanSourcePresentation'
import { MODULE_CATEGORY_LABELS } from './ComponentDrawer'
import { MaterialDrawer } from './MaterialDrawer'
import { WorkbenchDrawerStack } from './WorkbenchDrawerStack'
import { WorkbenchInspectorRail } from './WorkbenchInspectorRail'
import { displayPartRole } from './partRoleLabels.js'
import {
  activeDesignCanSelectParts,
  activeDesignPartDisplay,
  activeDesignPartIsLocked,
  activeDesignSelectedMaterialZoneId,
  activeDesignSelectedPartId,
} from './activeDesignMachine'
import { useWorkbenchLifecycle } from './useWorkbenchLifecycle'
import { parseAgentTurnPresentation, type AgentClarification, type AgentClarificationOption } from './agentConversationState'
import { useAgentConversationPresentation } from './useAgentConversationPresentation'
import { useAgentBlockoutDisplay } from './useAgentBlockoutDisplay'
import { useAgentAssetWorkspace } from './useAgentAssetWorkspace'
import { getLegacyCompatibilityDisplay } from './legacyCompatibilityDisplay'
import { useViewportDisplayPreferences } from './useViewportDisplayPreferences'
import {
  formatViewportMeasurement,
  readViewportMeasurement,
  type ViewportMeasurementMode,
  type ViewportMeasurementReadout,
} from './viewportMeasurementPresentation'
import { useLegacyModuleGraphWorkspace } from './useLegacyModuleGraphWorkspace'
import { useLegacyModuleGraphOverlay } from './useLegacyModuleGraphOverlay'
import { useAgentRenderPresentation } from './useAgentRenderPresentation'
import { useAgentEditAssistPresentation } from './useAgentEditAssistPresentation'
import { useAgentMaterialCatalogPresentation } from './useAgentMaterialCatalogPresentation'
import { useAgentMaterialFilterPresentation } from './useAgentMaterialFilterPresentation'
import { useAgentMaterialPreselectionPresentation } from './useAgentMaterialPreselectionPresentation'
import { resolveAgentMaterialDisplayId } from './agentMaterialPreselectionPresentationState'
import {
  SurfaceAdornmentDrawer,
  type SurfaceAdornmentAdapter,
  type SurfaceAdornmentDraft,
  type SurfaceAdornmentTarget,
} from './SurfaceAdornmentDrawer'
import {
  ReferenceEvidenceDrawer,
  readReferenceRebuildComparisonPlan,
  readReferenceRebuildExactLineage,
  type ReferenceEvidenceAdapter,
  type ReferenceEvidenceHistoryEntry,
  type ReferenceEvidenceRecord,
  type ReferenceEvidenceTarget,
} from './ReferenceEvidenceDrawer'
import {
  compatibleQuickMaterialPresets,
  createQuickMaterialPreviewOperation,
} from './agentMaterialQuickActions'
import { useComponentCatalogPresentation } from './useComponentCatalogPresentation'
import { useConceptWorkbench } from './useConceptWorkbench'
import { isProviderExecutionError, providerCheckPresentation } from './providerConnectionPresentation'
import {
  initialViewportDockPresentationState,
  viewportDockPresentationReducer,
} from './viewportDockState'
import './cad-workbench.css'

type Tool = 'select' | 'move' | 'rotate' | 'scale' | 'orbit' | 'measure' | 'section'
type CameraView = 'iso' | 'front' | 'top' | 'right'
type LightPreset = 'cad_neutral' | 'soft_studio' | 'concept_contrast'
type MeasurementAnnotation = {
  id: string
  readout: ViewportMeasurementReadout
}
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
type AgentTurnRecordResult = {
  recorded: boolean
  clarification: boolean
  cancelled: boolean
  failed: boolean
  plan: MechanicalConceptPlan | null
  decision: SingleResultDecision | null
}


const DEFAULT_AGENT_MATERIAL_PRESETS: AgentMaterialPreset[] = [
  { schema_version: 'MaterialPreset@1', material_id: 'mat_graphite', display_name: '石墨深灰', category: 'metal', pbr: { base_color: '#26313b', metallic: 0.78, roughness: 0.34, opacity: 1 }, visual_only: true, allowed_domains: ['future_weapon_prop', 'vehicle_concept', 'aircraft_concept', 'robotic_arm_concept'], provenance: 'forgecad_builtin' },
  { schema_version: 'MaterialPreset@1', material_id: 'mat_aluminum', display_name: '拉丝铝', category: 'metal', pbr: { base_color: '#8a9aa8', metallic: 0.88, roughness: 0.28, opacity: 1 }, visual_only: true, allowed_domains: ['future_weapon_prop', 'vehicle_concept', 'aircraft_concept', 'robotic_arm_concept'], provenance: 'forgecad_builtin' },
  { schema_version: 'MaterialPreset@1', material_id: 'mat_automotive_paint', display_name: '亮面汽车漆', category: 'coating', pbr: { base_color: '#3d78b8', metallic: 0.38, roughness: 0.2, opacity: 1 }, visual_only: true, allowed_domains: ['vehicle_concept', 'future_weapon_prop'], provenance: 'forgecad_builtin' },
]

const DOMAIN_TYPE_BY_PACK: Record<string, string> = {
  pack_future_weapon_prop: 'future_weapon_prop',
  pack_vehicle_concept: 'vehicle_concept',
  pack_aircraft_concept: 'aircraft_concept',
  pack_robotic_arm_concept: 'robotic_arm_concept',
}

const DEFAULT_CONCEPT_BRIEF = '一台结构清晰、比例协调、适合继续编辑的未来机械概念展示模型'

/**
 * R007B intentionally has one reviewed production-arm prerequisite.  Keep the
 * Rust conflict actionable for a zero-basis user, while preserving every other
 * backend error verbatim for diagnosis.
 */
function referenceRebuildFailureMessage(error: unknown): string {
  if (error instanceof ForgeApiError && error.code === 'REFERENCE_REBUILD_C106_BASE_REQUIRED') {
    return '请先生成并确认机械臂生产基准，再使用参考重建；当前设计没有变化。'
  }
  return error instanceof Error ? error.message : '参考引导重建预览失败；当前设计没有变化。'
}

function compileSurfaceAdornmentDraft(draft: SurfaceAdornmentDraft) {
  const intensity: 'subtle' | 'balanced' | 'pronounced' = draft.intensity === 'bold'
    ? 'pronounced'
    : draft.intensity
  const coverage = {
    center: 'center_band', edge: 'edge_band', full: 'full_zone', symmetric: 'symmetric_pair',
  }[draft.coverage] as 'center_band' | 'edge_band' | 'full_zone' | 'symmetric_pair'
  if (draft.kind === 'streamline') {
    return { kind: 'flowline' as const, motif: 'double_flowline' as const, intensity, coverage }
  }
  if (draft.kind === 'texture') {
    return {
      kind: 'micro_surface' as const,
      motif: draft.motif === 'parallel' ? 'parallel_groove' as const : 'hex_microgrid' as const,
      intensity,
      coverage,
    }
  }
  return {
    kind: 'normal_relief' as const,
    motif: draft.motif === 'radial' || draft.motif === 'technical_mark'
      ? 'chevron_relief' as const
      : 'parallel_groove' as const,
    intensity,
    coverage,
  }
}
const DEFAULT_HIDDEN_NODE_IDS = ['node_storage']
const CONCEPT_FAMILY_SUGGESTIONS = [
  ['冰原探索车', '一台适合冰原探索的紧凑未来概念车，四轮独立悬挂、分层外壳、耐候材料'],
  ['垂直起降器', '一架用于城市救援的轻型垂直起降飞行器，明确机身、旋翼舱和维护面板'],
  ['三关节机械臂', '一台桌面级三关节机械臂，底座、关节、末端工具分件清晰，适合继续调整比例'],
  ['未来概念道具', '一件非功能性的未来概念展示道具，外观完整、模块清楚、使用深色金属与冷色点缀'],
] as const

const DEFAULT_AGENT_CLARIFICATION_OPTIONS: AgentClarificationOption[] = [
  { domain_pack_id: 'pack_vehicle_concept', label: '汽车与地面载具', prompt: CONCEPT_FAMILY_SUGGESTIONS[0][1] },
  { domain_pack_id: 'pack_aircraft_concept', label: '飞机与航空器', prompt: CONCEPT_FAMILY_SUGGESTIONS[1][1] },
  { domain_pack_id: 'pack_robotic_arm_concept', label: '机械臂与机器人机构', prompt: CONCEPT_FAMILY_SUGGESTIONS[2][1] },
  { domain_pack_id: 'pack_future_weapon_prop', label: '未来武器概念道具', prompt: CONCEPT_FAMILY_SUGGESTIONS[3][1] },
]

const TOOL_ITEMS: Array<{
  id: Tool
  label: string
  icon: typeof CursorClick
  implemented: boolean
  unavailableReason?: string
}> = [
  { id: 'select', label: '选择', icon: CursorClick, implemented: true },
  { id: 'move', label: '移动', icon: ArrowsOutCardinal, implemented: true },
  { id: 'rotate', label: '旋转', icon: ArrowsClockwise, implemented: true },
  { id: 'scale', label: '缩放', icon: ArrowsOutCardinal, implemented: true },
  { id: 'orbit', label: '旋转视图', icon: ArrowsClockwise, implemented: true },
  { id: 'measure', label: '测量', icon: Ruler, implemented: true },
  { id: 'section', label: '截面', icon: SelectionAll, implemented: true },
]

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
  } = agentConversationState
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
  const blockoutPreviewPresentation = selectAgentBlockoutPreviewPresentation(agentBlockoutDisplay)
  const agentPlanSourcePresentation = selectAgentPlanSourcePresentation(agentPlan)
  const [cameraView, setCameraView] = useState<CameraView>('iso')
  const [lightPreset, setLightPreset] = useState<LightPreset>('cad_neutral')
  const [presentationProfile, setPresentationProfile] = useState<'quick_sketch' | 'showcase'>('showcase')
  const [styleOptionsOpen, setStyleOptionsOpen] = useState(false)
  const [materialOptionsOpen, setMaterialOptionsOpen] = useState(false)
  const [agentThreads, setAgentThreads] = useState<AgentThreadSummary[]>([])
  const [threadHistoryLoading, setThreadHistoryLoading] = useState(false)
  const [viewportDock, dispatchViewportDock] = useReducer(
    viewportDockPresentationReducer,
    initialViewportDockPresentationState,
  )
  const viewportFocusTriggerRef = useRef<HTMLButtonElement | null>(null)
  const [agentAssetChangeSet, setAgentAssetChangeSet] = useState<AgentAssetChangeSet | null>(null)
  const agentAssetPreviewInFlightRef = useRef(false)
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
  const projectIsEmpty = Boolean(
    concept.project
    && !projectHasActiveAgentSnapshot
    && !activeDesignSnapshot
    && !agentBlockoutSegmentation
    && activeDesignState.projectId === concept.project.project_id
    && activeDesignState.operation === 'idle',
  )
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
  const displayedAgentSelectedPartId = activeAgentAssetVersion
    ? agentAssetWorkspace.selectedPartId
    : agentCandidateSelectedPartId
  const selectedAgentPart = agentAssetVersion?.parts.find((part) => part.part_id === displayedAgentSelectedPartId)
  const isExternalGlbReference = agentAssetVersion?.shape_program?.schema_version === 'ExternalGLBReference@1'
  const activePartDisplay = activeDesignPartDisplay(activeDesignSnapshot)
  const selectedAgentPartLocked = selectedAgentPart
    ? activeDesignPartIsLocked(activeDesignSnapshot, selectedAgentPart.part_id)
    : false
  const [appearanceMaterialZoneId, setAppearanceMaterialZoneId] = useState('')
  const [surfaceAdornmentOpen, setSurfaceAdornmentOpen] = useState(false)
  const [measurementMode, setMeasurementMode] = useState<ViewportMeasurementMode>('distance')
  const [measurementStart, setMeasurementStart] = useState<ViewportMeasurementPoint | null>(null)
  const [measurementEnd, setMeasurementEnd] = useState<ViewportMeasurementPoint | null>(null)
  const [measurementAnnotations, setMeasurementAnnotations] = useState<MeasurementAnnotation[]>([])
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
  const surfaceAdornmentTarget = useMemo<SurfaceAdornmentTarget | null>(() => {
    if (!concept.project?.project_id || !activeAgentAssetVersion || !selectedAgentPart || !selectedMaterialZoneId) return null
    return {
      projectId: concept.project.project_id,
      assetVersionId: activeAgentAssetVersion.asset_version_id,
      partId: selectedAgentPart.part_id,
      partLabel: displayPartRole(selectedAgentPart.role),
      materialZoneId: selectedMaterialZoneId,
      materialZoneLabel: `材质区 ${selectedMaterialZoneId}`,
    }
  }, [activeAgentAssetVersion, concept.project?.project_id, selectedAgentPart, selectedMaterialZoneId])
  const surfaceAdornmentDisabledReason = !activeAgentAssetVersion
    ? '请先确认保存当前设计，再添加外观细节。'
    : isExternalGlbReference
      ? '导入参考模型不能直接编辑；请先让 Agent 重建为可编辑设计。'
      : !selectedAgentPart
        ? '请先从左侧选择一个部件。'
        : selectedAgentPartLocked
          ? '当前部件已锁定，请先解除锁定。'
          : !selectedMaterialZoneId
            ? '当前部件没有可编辑的材质区。'
            // The open A005 drawer owns its own preview/retain lifecycle. Once
            // that preview is sealed, `agentAssetChangeSet` intentionally
            // becomes non-null; disabling the same drawer here would hide its
            // retain/cancel controls and strand the preview. Other entry
            // points remain blocked while the drawer is closed.
            : agentAssetChangeSet && !surfaceAdornmentOpen
              ? '请先保留或取消当前预览，再添加外观细节。'
              : activeDesignState.operation !== 'idle'
                ? '正在同步当前设计，请稍后再试。'
                : null
  const measurementReadout = useMemo(
    () => readViewportMeasurement(measurementMode, measurementStart, measurementEnd),
    [measurementEnd, measurementMode, measurementStart],
  )
  const handleMeasurePoint = useCallback((point: ViewportMeasurementPoint) => {
    if (!measurementStart || measurementEnd) {
      setMeasurementStart(point)
      setMeasurementEnd(null)
      return
    }
    setMeasurementEnd(point)
  }, [measurementEnd, measurementStart])
  const clearMeasurements = useCallback(() => {
    setMeasurementStart(null)
    setMeasurementEnd(null)
    setMeasurementAnnotations([])
  }, [])
  const pinMeasurement = useCallback(() => {
    if (!measurementReadout) return
    setMeasurementAnnotations((current) => [
      ...current.slice(-4),
      { id: `measurement-${Date.now().toString(36)}`, readout: measurementReadout },
    ])
    setMeasurementStart(null)
    setMeasurementEnd(null)
  }, [measurementReadout])

  useEffect(() => {
    // Measurements are view-local inspection aids. They never cross a project
    // or exact Agent asset boundary and are deliberately not Snapshot facts.
    clearMeasurements()
  }, [agentAssetVersion?.asset_version_id, clearMeasurements, concept.project?.project_id])
  const referenceEvidenceTarget = useMemo<ReferenceEvidenceTarget | null>(() => {
    if (!concept.project?.project_id) return null
    return {
      projectId: concept.project.project_id,
      // R007 starts with the robotic-arm Recipe when no current domain exists.
      domainPackId: activeAgentAssetVersion?.domain_pack_id ?? agentPlan?.domain_pack_id ?? 'pack_robotic_arm_concept',
      baseAssetVersionId: isExternalGlbReference ? null : activeAgentAssetVersion?.asset_version_id ?? null,
    }
  }, [activeAgentAssetVersion?.asset_version_id, activeAgentAssetVersion?.domain_pack_id, agentPlan?.domain_pack_id, concept.project?.project_id, isExternalGlbReference])
  const referenceViewportActive = referenceViewport?.projectId === concept.project?.project_id
  const viewportGlb = referenceViewportActive
    ? referenceViewport?.kind === 'glb' ? referenceViewport.glb : null
    : agentBlockoutGlbBase64
  const viewportGlbKind = referenceViewportActive
    ? referenceViewport?.kind === 'glb' ? 'external_reference' as const : null
    : agentBlockoutGlbKind
  const viewportShapeProgram = referenceViewportActive ? null : agentBlockoutShapeProgram
  const viewportReferenceImage = referenceViewportActive && referenceViewport?.kind === 'image'
    ? {
      url: referenceViewport.imageUrl,
      evidenceId: referenceViewport.evidenceId,
      sourceObjectSha256: referenceViewport.sourceObjectSha256,
      referenceClass: referenceViewport.referenceClass,
    }
    : null
  const materialPreselectionSource = isExternalGlbReference
    ? 'external_glb' as const
    : legacyDesignReadOnly
      ? 'legacy' as const
      : activeAgentAssetVersion
        ? 'agent_asset' as const
        : agentPlan
          ? 'blockout' as const
          : 'none' as const
  const materialPreselectionContext = useMemo(() => ({
    projectId: concept.project?.project_id ?? null,
    assetVersionId: activeAgentAssetVersion?.asset_version_id ?? null,
    selectedPartId: selectedAgentPart?.part_id ?? null,
    materialZoneId: selectedMaterialZoneId || null,
    source: materialPreselectionSource,
  }), [
    activeAgentAssetVersion?.asset_version_id,
    concept.project?.project_id,
    materialPreselectionSource,
    selectedAgentPart?.part_id,
    selectedMaterialZoneId,
  ])
  const committedMaterialBinding = selectedAgentPart && selectedMaterialZoneId
    ? activeAgentAssetVersion?.material_bindings?.[`${selectedAgentPart.part_id}:${selectedMaterialZoneId}`]
    : null
  const committedMaterialId = typeof committedMaterialBinding === 'string' ? committedMaterialBinding : null
  const appearanceMaterialId = resolveAgentMaterialDisplayId(
    agentMaterialPreselectionPresentation,
    materialPreselectionContext,
    committedMaterialId,
  )
  const quickMaterialPresets = useMemo(
    () => compatibleQuickMaterialPresets(materialPresets, activeMaterialDomain),
    [activeMaterialDomain, materialPresets],
  )
  const [providerConfig, setProviderConfig] = useState<ProviderConfigMetadata | null>(null)
  const [providerSetupOpen, setProviderSetupOpen] = useState(false)
  const [providerBaseUrl, setProviderBaseUrl] = useState('https://api.deepseek.com')
  const [providerModel, setProviderModel] = useState('deepseek-v4-pro')
  const [providerApiKey, setProviderApiKey] = useState('')
  const [providerSaving, setProviderSaving] = useState(false)
  const [activeProviderTurnId, setActiveProviderTurnId] = useState<string | null>(null)
  const [activeProviderCheckId, setActiveProviderCheckId] = useState<string | null>(null)
  const [importingGlb, setImportingGlb] = useState(false)
  const importGlbInputRef = useRef<HTMLInputElement | null>(null)
  const referenceEvidenceRequestEpochRef = useRef(0)
  const referenceRebuildPlanByChangeSetRef = useRef(new Map<string, {
    projectId: string
    baseAssetVersionId: string
    evidenceId: string
    sourceObjectSha256: string
    rebuildPlanId: string
  }>())

  useEffect(() => {
    openAgentRenderPresentation(
      activeAgentAssetVersion ? concept.project?.project_id ?? null : null,
      activeAgentAssetVersion?.asset_version_id ?? null,
    )
  }, [activeAgentAssetVersion?.asset_version_id, concept.project?.project_id, openAgentRenderPresentation])

  const refreshActiveDesign = useCallback(async (projectId: string) => {
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
        void api.loadAgentAssetPreviewGlb(version.asset_version_id).then((preview) => {
          const previewKind = preview.artifactProfileId === 'external_reference'
            ? 'external_reference'
            : 'compiled_agent_preview_pbr'
          if (!setBlockoutGlb(projectId, blockoutDisplayRequestId, preview.glb, previewKind)) return
          if (preview.artifactProfileId === 'external_reference') return
          setAssistantNote('已加载轻量编辑预览；生产级概念工件正在按需生成，完成后会在同一视口中替换。')
          void api.loadAgentAssetProductionGlb(version.asset_version_id).then((production) => {
            if (production.artifactProfileId !== 'production_concept') {
              throw new Error('Production GLB response did not use the production concept profile')
            }
            if (!setBlockoutGlb(
              projectId,
              blockoutDisplayRequestId,
              production.glb,
              'compiled_agent_production_pbr',
            )) return
            setAssistantNote(`生产级概念工件已加载：${production.triangleCount.toLocaleString()} 三角形、512×512 PBR 纹理；当前仍是可编辑概念资产，不是制造 CAD。`)
          }).catch(() => {
            if (!isCurrentActiveDesignRequest(requestId)) return
            setAssistantNote('生产级概念工件暂未加载；同源轻量预览仍可编辑，正式质量检查和下载不会使用该预览冒充最终结果。')
          })
        }).catch(async () => {
          if (isImportedReference || !isCurrentActiveDesignRequest(requestId)) {
            if (!setBlockoutGlb(projectId, blockoutDisplayRequestId, null, null)) return
            setAssistantNote('导入参考模型的原始 GLB 不可读取；不会影响其他项目版本。')
            return
          }
          // A confirmed asset may already own a valid production object even
          // when its lightweight preview object is stale or unavailable. Do
          // not strand the one real viewport in an empty state: independently
          // request the production artifact and keep all existing profile,
          // readback and request-id checks. This is a display recovery path,
          // not an export fallback or a second geometry truth.
          try {
            const production = await api.loadAgentAssetProductionGlb(version.asset_version_id)
            if (production.artifactProfileId !== 'production_concept') {
              throw new Error('Production GLB response did not use the production concept profile')
            }
            if (!setBlockoutGlb(
              projectId,
              blockoutDisplayRequestId,
              production.glb,
              'compiled_agent_production_pbr',
            )) return
            setAssistantNote(`轻量预览不可用，已直接加载生产级概念工件：${production.triangleCount.toLocaleString()} 三角形、512×512 PBR 纹理。`)
          } catch {
            if (!setBlockoutGlb(projectId, blockoutDisplayRequestId, null, null)) return
            setAssistantNote('当前 Agent 资产的预览与生产 PBR GLB 均不可读取；视口已明确回退为参数外观，没有继续显示旧材质。')
          }
        })
      }
      return response.data
    } catch (caught) {
      const error = failActiveDesignRequest(requestId, caught)
      if (!error) return null
      // A freshly created project has no Snapshot until its first Agent asset
      // is committed. This is an empty state, not a user-facing failure.
      if (error.kind !== 'not_found') {
        setAssistantNote(error.message)
      }
      return null
    }
  }, [api, clearAgentAssetWorkspace, clearAgentAssetWorkspaceQuality, clearAgentEditAssistPresentation, failActiveDesignRequest, hydrateBlockoutDisplay, isCurrentActiveDesignRequest, receiveActiveDesignSnapshot, receiveAgentAssetWorkspaceAsset, receiveAgentAssetWorkspaceNavigation, receiveAgentAssetWorkspaceQuality, setBlockoutGlb, startActiveDesignRequest, startAgentAssetWorkspaceHydration])

  const updateRenderPreset = useCallback(async (next: { cameraView?: CameraView; lightPreset?: LightPreset }) => {
    const nextCameraView = next.cameraView ?? cameraView
    const nextLightPreset = next.lightPreset ?? lightPreset
    setCameraView(nextCameraView)
    setLightPreset(nextLightPreset)
    const snapshot = activeDesignSnapshot
    const etag = activeDesignState.snapshotEtag
    if (!snapshot || snapshot.active_design.source !== 'agent_asset' || !etag) return
    const requestId = startActiveDesignRequest('setting_render_preset')
    try {
      const response = await api.setActiveDesignRenderPreset(
        snapshot.project_id,
        {
          client_request_id: `render-preset-${requestId}`,
          snapshot_revision: snapshot.revision,
          camera_view: nextCameraView,
          light_preset: nextLightPreset,
        },
        { ifMatch: etag },
      )
      receiveActiveDesignSnapshot(snapshot.project_id, requestId, response)
    } catch (caught) {
      const error = failActiveDesignRequest(requestId, caught)
      if (!error) return
      if (error.shouldReloadSnapshot) void refreshActiveDesign(snapshot.project_id)
      setAssistantNote(error.message)
    }
  }, [activeDesignSnapshot, activeDesignState.snapshotEtag, api, cameraView, failActiveDesignRequest, lightPreset, receiveActiveDesignSnapshot, refreshActiveDesign, startActiveDesignRequest])

  useEffect(() => {
    const projectId = concept.project?.project_id
    if (!projectId) return
    openProject(projectId)
    openAgentAssetWorkspaceProject(projectId)
    void refreshActiveDesign(projectId)
  }, [concept.project?.project_id, openAgentAssetWorkspaceProject, openProject, refreshActiveDesign])


  const loadAgentEditAssist = useCallback(async (projectId: string, assetVersionId: string, partId: string) => {
    const requestId = startAgentEditAssistRead(projectId, assetVersionId, partId)
    if (requestId === null) return
    try {
      const [candidates, structure, semanticProportions] = await Promise.all([
        api.listAgentComponentCandidates(assetVersionId, partId),
        api.listAgentStructureSuggestions(assetVersionId),
        api.listAgentSemanticProportions(assetVersionId, partId).catch(() => null),
      ])
      receiveAgentEditAssistRead(projectId, assetVersionId, partId, requestId, candidates, structure, semanticProportions)
    } catch {
      failAgentEditAssistRead(projectId, assetVersionId, partId, requestId)
    }
  }, [api, failAgentEditAssistRead, receiveAgentEditAssistRead, startAgentEditAssistRead])

  useEffect(() => {
    const projectId = activeAgentAssetVersion && !isExternalGlbReference
      ? concept.project?.project_id ?? null
      : null
    const assetVersionId = projectId ? activeAgentAssetVersion?.asset_version_id ?? null : null
    const selectedPartId = assetVersionId ? selectedAgentPart?.part_id ?? null : null
    openAgentEditAssistPresentation(projectId, assetVersionId, selectedPartId)
    if (!projectId || !assetVersionId || !selectedPartId) return
    void loadAgentEditAssist(projectId, assetVersionId, selectedPartId)
  }, [
    activeAgentAssetVersion?.asset_version_id,
    concept.project?.project_id,
    isExternalGlbReference,
    loadAgentEditAssist,
    openAgentEditAssistPresentation,
    selectedAgentPart?.part_id,
  ])

  useEffect(() => {
    const context = {
      projectId: concept.legacyDetailsEnabled ? concept.project?.project_id ?? null : null,
      packId: concept.legacyDetailsEnabled ? concept.project?.profile.pack_id ?? null : null,
      source: concept.legacyDetailsEnabled ? 'legacy' as const : 'none' as const,
    }
    openComponentCatalog(context)
    if (!context.packId || context.source !== 'legacy') return
    const requestId = startComponentCatalogRead(context)
    if (requestId === null) return
    void api.listModuleAssets(context.packId).then((response) => {
      receiveComponentCatalog(context, requestId, response.items ?? [])
    }).catch(() => { failComponentCatalog(context, requestId) })
  }, [api, concept.legacyDetailsEnabled, concept.project, failComponentCatalog, openComponentCatalog, receiveComponentCatalog, startComponentCatalogRead])

  useEffect(() => {
    const context = {
      projectId: concept.project?.project_id ?? null,
      assetVersionId: activeAgentAssetVersion?.asset_version_id ?? null,
      domainPackId: activeAgentAssetVersion?.domain_pack_id ?? agentPlan?.domain_pack_id ?? null,
      source: isExternalGlbReference ? 'external_glb' as const : activeAgentAssetVersion ? 'agent_asset' as const : agentPlan ? 'blockout' as const : 'none' as const,
    }
    openAgentMaterialCatalogPresentation(context)
    const requestId = startAgentMaterialCatalogRead(context)
    if (requestId === null) return
    void api.listAgentMaterials().then((items) => {
      receiveAgentMaterialCatalog(context, requestId, items)
    }).catch(() => {
      failAgentMaterialCatalog(context, requestId, DEFAULT_AGENT_MATERIAL_PRESETS)
    })
  }, [
    activeAgentAssetVersion?.asset_version_id,
    activeAgentAssetVersion?.domain_pack_id,
    agentPlan?.domain_pack_id,
    api,
    concept.project?.project_id,
    failAgentMaterialCatalog,
    isExternalGlbReference,
    openAgentMaterialCatalogPresentation,
    receiveAgentMaterialCatalog,
    startAgentMaterialCatalogRead,
  ])

  useEffect(() => {
    openAgentMaterialFilterPresentation({
      projectId: concept.project?.project_id ?? null,
      domainPackId: activeAgentAssetVersion?.domain_pack_id ?? agentPlan?.domain_pack_id ?? null,
      source: isExternalGlbReference
        ? 'external_glb'
        : legacyDesignReadOnly
          ? 'legacy'
          : activeAgentAssetVersion
            ? 'agent_asset'
            : agentPlan
              ? 'blockout'
              : 'none',
    })
  }, [
    activeAgentAssetVersion?.domain_pack_id,
    agentPlan?.domain_pack_id,
    concept.project?.project_id,
    isExternalGlbReference,
    legacyDesignReadOnly,
    openAgentMaterialFilterPresentation,
  ])

  useEffect(() => {
    openAgentMaterialPreselectionPresentation(materialPreselectionContext)
  }, [
    materialPreselectionContext,
    openAgentMaterialPreselectionPresentation,
  ])

  useEffect(() => {
    void getTauriProviderConfig()
      .then((config) => {
        if (!config) return
        setProviderConfig(config)
        setProviderBaseUrl(config.base_url)
        setProviderModel(config.model)
      })
      .catch((caught) => {
        setAssistantNote(`无法读取模型服务配置：${errorText(caught)}。当前不会假定 DeepSeek 已配置。`)
      })
  }, [])

  useEffect(() => {
    // The compatibility graph may briefly clear during reload. Its local
    // selection is reconciled only against the returned legacy graph; it never
    // becomes the Agent Snapshot selection.
    if (!concept.graphRecord) return
    const nodes = concept.graphRecord?.graph.nodes ?? []
    reconcileLegacyModuleGraphSelection(
      nodes.map((node) => ({ nodeId: node.node_id, moduleId: node.module_id, locked: Boolean(node.locked) })),
      concept.graphRecord.graph.root_node_id,
    )
    reconcileLegacyModuleGraphOverlayNodes(nodes.map((node) => node.node_id))
  }, [
    concept.graphRecord,
    legacyModuleGraphOverlayContextKey,
    legacyModuleGraphWorkspacePreferenceKey,
    reconcileLegacyModuleGraphOverlayNodes,
    reconcileLegacyModuleGraphSelection,
  ])

  const selectedNode = concept.graphRecord?.graph.nodes.find(
    (node) => node.node_id === selectedComponent,
  ) ?? null
  const selectedModule = catalogModules.find(
    (module) => module.manifest.module_id === selectedNode?.module_id,
  ) ?? null
  const selectedModuleLabel = selectedModule
    ? MODULE_CATEGORY_LABELS[selectedModule.manifest.category]
    : '当前部件'
  const getModuleFileUrl = useCallback(
    (moduleId: string) => forgeApi.getModuleAssetFileUrl(moduleId),
    [],
  )
  const activeVersionSummary = (concept.project?.versions ?? []).find(
    (item) => item.version_id === concept.version?.version_id,
  )

  const selectGraphNode = useCallback((nodeId: string) => {
    const node = concept.graphRecord?.graph.nodes.find((item) => item.node_id === nodeId)
    selectLegacyModuleGraphNode(nodeId, node?.module_id ?? '')
  }, [concept.graphRecord, selectLegacyModuleGraphNode])

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
      const result = await api.downloadAgentAssetProductionGlb(activeAgentAssetVersion.asset_version_id)
      downloadBlobFile(result.blob, result.filename)
      setAssistantNote(`已下载当前 Agent 设计 v${activeAgentAssetVersion.version_no} 的生产级概念 GLB；下载前已完成 ${result.triangleCount.toLocaleString()} 三角形回读。`)
    } catch (caught) {
      setAssistantNote(`3D 模型下载失败：${errorText(caught)}`)
    }
  }, [activeAgentAssetVersion, api])

  const handleRenderAgentViews = useCallback(async () => {
    const projectId = concept.project?.project_id
    if (!activeAgentAssetVersion || !projectId) return
    const requestId = startAgentRenderRequest(projectId, activeAgentAssetVersion.asset_version_id)
    if (requestId === null) return
    try {
      const result = await api.renderAgentAssetViews(activeAgentAssetVersion.asset_version_id, { width: 512, height: 512 })
      if (!receiveAgentRenderSet(projectId, activeAgentAssetVersion.asset_version_id, requestId, result)) return
      setAssistantNote(result.exploded_view_available
        ? '已生成四视图和爆炸概念图。它们均为当前 Agent 资产的只读透明预览，不会改变模型版本。'
        : '已生成四张概念视图。当前模型不能安全分离出爆炸概念图；模型版本没有变化。')
    } catch (caught) {
      if (!failAgentRenderRequest(projectId, activeAgentAssetVersion.asset_version_id, requestId)) return
      setAssistantNote(`概念图生成失败：${errorText(caught)}`)
    }
  }, [activeAgentAssetVersion, api, concept.project?.project_id, failAgentRenderRequest, receiveAgentRenderSet, startAgentRenderRequest])

  const handleDownloadAgentRenderView = useCallback((view: AgentAssetRenderView) => {
    downloadBase64File(view.png_base64, `${activeAgentAssetVersion?.asset_version_id ?? 'agent-asset'}-${view.view_id}.png`, 'image/png')
  }, [activeAgentAssetVersion])

  const handleDownloadAgentRenderPackage = useCallback(async () => {
    const projectId = concept.project?.project_id
    const agentRenderSet = agentRenderPresentation.renderSet
    if (!activeAgentAssetVersion || !projectId || !agentRenderSet) return
    if (agentRenderSet.asset_version_id !== activeAgentAssetVersion.asset_version_id) {
      setAssistantNote('概念图对应的设计版本已变化，请重新生成后再下载。')
      return
    }
    const requestId = startAgentRenderPackageRequest(
      projectId,
      activeAgentAssetVersion.asset_version_id,
      agentRenderSet.render_set_sha256,
    )
    if (requestId === null) return
    try {
      const result = await api.downloadAgentAssetRenderPackage(activeAgentAssetVersion.asset_version_id, agentRenderSet)
      if (!finishAgentRenderPackageRequest(projectId, activeAgentAssetVersion.asset_version_id, requestId, agentRenderSet.render_set_sha256)) return
      if (result.renderSetSha256 && result.renderSetSha256 !== agentRenderSet.render_set_sha256) {
        setAssistantNote('概念图包与当前预览不一致，未开始下载。请重新生成概念图。')
        return
      }
      downloadBlobFile(result.blob, result.filename)
      setAssistantNote('已下载概念图包：只包含当前概念 PNG 与来源清单，不包含模型源文件或工程信息。')
    } catch (caught) {
      if (!finishAgentRenderPackageRequest(projectId, activeAgentAssetVersion.asset_version_id, requestId, agentRenderSet.render_set_sha256)) return
      setAssistantNote(`概念图包下载失败：${errorText(caught)}`)
    }
  }, [activeAgentAssetVersion, agentRenderPresentation.renderSet, api, concept.project?.project_id, finishAgentRenderPackageRequest, startAgentRenderPackageRequest])

  useEffect(() => {
    if (!hasOpenDrawer) return

    const focusableSelector = 'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex="-1"])'
    const focusInitialControl = () => {
      const drawer = drawerFocusRef.current
      if (!drawer) return
      const initial = drawer.querySelector<HTMLElement>('[data-dialog-initial-focus="true"]')
        ?? drawer.querySelector<HTMLElement>(focusableSelector)
      initial?.focus()
    }
    const frame = window.requestAnimationFrame(focusInitialControl)
    const onDrawerKeyDown = (event: KeyboardEvent) => {
      const drawer = drawerFocusRef.current
      if (!drawer) return
      if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        closeAllDrawers()
        return
      }
      if (event.key !== 'Tab') return
      const focusable = Array.from(drawer.querySelectorAll<HTMLElement>(focusableSelector))
        .filter((element) => !element.hasAttribute('disabled') && element.offsetParent !== null)
      if (focusable.length === 0) {
        event.preventDefault()
        drawer.focus()
        return
      }
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      if (!drawer.contains(document.activeElement)) {
        event.preventDefault()
        first.focus()
      } else if (event.shiftKey && document.activeElement === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault()
        first.focus()
      }
    }
    window.addEventListener('keydown', onDrawerKeyDown, true)
    return () => {
      window.cancelAnimationFrame(frame)
      window.removeEventListener('keydown', onDrawerKeyDown, true)
    }
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

  useEffect(() => {
    openConversationProject(concept.project?.project_id ?? null)
    dispatchSingleResultDecision({ type: 'open_project', projectId: concept.project?.project_id ?? null })
    openBlockoutProject(concept.project?.project_id ?? null)
    openAgentAssetWorkspaceProject(concept.project?.project_id ?? null)
    openViewportDisplayPreferences(concept.project?.project_id ?? null)
    setAgentAssetChangeSet(null)
    setAgentCandidateSelectedPartId(null)
    setSurfaceAdornmentOpen(false)
    setReferenceEvidenceOpen(false)
    replaceReferenceViewport(null)
    referenceEvidenceRequestEpochRef.current += 1
    referenceRebuildPlanByChangeSetRef.current.clear()
  }, [concept.project?.project_id, openAgentAssetWorkspaceProject, openBlockoutProject, openConversationProject, openViewportDisplayPreferences, replaceReferenceViewport])

  useEffect(() => {
    dispatchViewportDock({ type: 'open_project', projectId: concept.project?.project_id ?? null })
    setStyleOptionsOpen(false)
    setMaterialOptionsOpen(false)
  }, [concept.project?.project_id])

  useEffect(() => {
    const projectId = concept.project?.project_id ?? null
    let cancelled = false
    setThreadHistoryLoading(true)
    void api.listAgentThreads()
      .then((response) => {
        if (cancelled) return
        setAgentThreads(response.items.filter((thread) => (thread.project_id ?? null) === projectId))
      })
      .catch(() => {
        if (!cancelled) setAgentThreads([])
      })
      .finally(() => {
        if (!cancelled) setThreadHistoryLoading(false)
      })
    return () => { cancelled = true }
  }, [agentThreadId, api, concept.project?.project_id])

  const selectConversationThread = useCallback(async (threadId: string) => {
    const projectId = concept.project?.project_id ?? null
    const { requestId } = startAgentConversationRequest(projectId)
    try {
      const thread = await api.getAgentThread(threadId)
      if ((thread.project_id ?? null) !== projectId) {
        setAssistantNote('这个对话不属于当前项目，未切换工作台。')
        return
      }
      const lastTurn = thread.turns.at(-1)
      const items = thread.turns.flatMap((turn) => turn.items).sort((left, right) => left.sequence - right.sequence)
      const presentation = lastTurn
        ? parseAgentTurnPresentation(lastTurn.items, lastTurn.request_text)
        : { clarification: null, plan: null }
      if (!receiveAgentTurn(projectId, requestId, thread.thread_id, items, presentation)) return
      setAssistantNote(lastTurn
        ? `已打开“${thread.title}”；当前 3D 与 Snapshot 保持不变。`
        : `已打开“${thread.title}”；这个对话还没有消息。`)
    } catch (caught) {
      if (!isCurrentAgentConversationRequest(projectId, requestId)) return
      setAssistantNote(`对话记录加载失败：${errorText(caught)}`)
    }
  }, [api, concept.project?.project_id, isCurrentAgentConversationRequest, receiveAgentTurn, setAssistantNote, startAgentConversationRequest])

  useEffect(() => {
    openLegacyModuleGraphWorkspace(
      legacyCompatibility.isLegacyReadOnly && concept.legacyDetailsEnabled
        ? concept.project?.project_id ?? null
        : null,
    )
  }, [concept.legacyDetailsEnabled, concept.project?.project_id, legacyCompatibility.isLegacyReadOnly, openLegacyModuleGraphWorkspace])

  useEffect(() => {
    openLegacyModuleGraphOverlay(
      legacyCompatibility.isLegacyReadOnly && concept.legacyDetailsEnabled ? concept.project?.project_id ?? null : null,
      legacyCompatibility.isLegacyReadOnly && concept.legacyDetailsEnabled ? concept.graphRecord?.graph.graph_id ?? null : null,
      legacyCompatibility.isLegacyReadOnly && concept.legacyDetailsEnabled ? DEFAULT_HIDDEN_NODE_IDS : [],
    )
  }, [
    concept.graphRecord?.graph.graph_id,
    concept.legacyDetailsEnabled,
    concept.project?.project_id,
    legacyCompatibility.isLegacyReadOnly,
    openLegacyModuleGraphOverlay,
  ])

  useEffect(() => {
    if (activeDesignSnapshot?.active_design.source === 'agent_asset' && concept.legacyDetailsEnabled) {
      concept.closeLegacyDetails()
    }
  }, [activeDesignSnapshot?.active_design.source, concept.closeLegacyDetails, concept.legacyDetailsEnabled])

  const recordAgentTurn = useCallback(async (message: string, clarificationDomainPackId?: string): Promise<AgentTurnRecordResult> => {
    const projectId = concept.project?.project_id ?? null
    const { requestId } = startAgentConversationRequest(projectId)
    dispatchSingleResultDecision({ type: 'request_started', projectId, requestId, detail: 'Agent 正在构建并检查 3D 结果。' })
    try {
      let threadId = agentThreadId
      if (!threadId) {
        const created = await api.createAgentThread({
          client_request_id: `agent-thread-${Date.now()}`,
          project_id: concept.project?.project_id,
          title: concept.project?.name ? `${concept.project.name} · Agent` : '新建设计会话',
        })
        threadId = created.thread_id
        if (!isCurrentAgentConversationRequest(projectId, requestId)) {
          return { recorded: false, clarification: false, cancelled: true, failed: false, plan: null, decision: null }
        }
      }
      const afterSequence = agentKernelItems.reduce(
        (latest, item) => Math.max(latest, item.sequence),
        0,
      )
      const streamedItems = new Map<number, (typeof agentKernelItems)[number]>()
      const unsubscribeThreadEvents = api.subscribeAgentThreadEvents(threadId, {
        onEvent: (event) => {
          if (!isCurrentAgentConversationRequest(projectId, requestId)) return
          streamedItems.set(event.item.sequence, event.item)
          const items = [...streamedItems.values()].sort((left, right) => left.sequence - right.sequence)
          setActiveProviderTurnId(event.turn_id)
          receiveAgentTurn(
            projectId,
            requestId,
            threadId,
            items,
            parseAgentTurnPresentation(items, message),
          )
        },
      }, afterSequence)
      const turnPromise = api.startAgentTurn(threadId, {
        client_request_id: `agent-turn-${Date.now()}`,
        message,
        ...(clarificationDomainPackId ? { clarification_domain_pack_id: clarificationDomainPackId } : {}),
      })
      let turn: AgentTurn
      try {
        turn = await turnPromise
      } finally {
        unsubscribeThreadEvents()
        setActiveProviderTurnId(null)
      }
      const presentation = parseAgentTurnPresentation(turn.items, turn.request_text)
      if (!receiveAgentTurn(projectId, requestId, threadId, turn.items, presentation)) {
        return { recorded: false, clarification: false, cancelled: true, failed: false, plan: null, decision: null }
      }
      if (turn.status === 'cancelled') {
        dispatchSingleResultDecision({ type: 'request_cancelled', projectId, requestId })
        setAssistantNote('本次模型请求已取消；没有创建计划、资产版本或导出。')
        return { recorded: true, clarification: false, cancelled: true, failed: false, plan: null, decision: null }
      }
      if (presentation.clarification) {
        clearBlockoutDisplay(projectId)
        clearAgentAssetWorkspace()
        setAgentAssetChangeSet(null)
        setAgentCandidateSelectedPartId(null)
        setAssistantNote(presentation.clarification.question)
        return { recorded: true, clarification: true, cancelled: false, failed: false, plan: null, decision: null }
      }
      const decision = readSingleResultDecisionFromAgentItems(turn.items, { projectId, turnId: turn.turn_id })
      // A continuation turn is an edit intent, not a second independent
      // asset.  The current Action Loop may still have produced its bounded
      // single-result audit while the delta contract was being introduced;
      // reject that transient candidate and hand the real delta to the
      // existing ChangeSet preview flow instead of replacing the asset.
      if (presentation.plan?.assembly_delta) {
        if (decision?.state === 'ready_for_preview') {
          void api.rejectSingleResultPreview({
            projectId: decision.project_id,
            turnId: decision.turn_id,
            previewId: decision.preview.preview_id,
            artifactSha256: decision.preview.artifact_sha256,
            artifactProfileId: decision.preview.artifact_profile_id,
            clientRequestId: `single-result-delta-reject-${decision.preview.preview_id}`,
          }).catch(() => undefined)
        }
        dispatchSingleResultDecision({ type: 'request_cancelled', projectId, requestId })
        return { recorded: true, clarification: false, cancelled: false, failed: false, plan: presentation.plan, decision: null }
      }
      if (decision) {
        dispatchSingleResultDecision({ type: 'decision_received', projectId, requestId, decision })
        if (decision.state === 'ready_for_preview') {
          try {
            const preview = await api.loadSingleResultPreviewGlb({
              projectId: decision.project_id,
              turnId: decision.turn_id,
              previewId: decision.preview.preview_id,
              artifactSha256: decision.preview.artifact_sha256,
              artifactProfileId: decision.preview.artifact_profile_id,
            })
            if (!isCurrentAgentConversationRequest(projectId, requestId)) {
              return { recorded: false, clarification: false, cancelled: true, failed: false, plan: null, decision: null }
            }
            clearAgentAssetWorkspace()
            setAgentAssetChangeSet(null)
            setAgentCandidateSelectedPartId(null)
            hydrateBlockoutDisplay(projectId, {
              glbBase64: preview.glb,
              glbKind: preview.artifactProfileId === 'production_concept'
                ? 'compiled_agent_production_pbr'
                : 'compiled_agent_preview_pbr',
              shapeProgram: null,
              segmentation: null,
            })
          } catch (caught) {
            const error = `正式结果已通过质量门，但 3D 预览读取失败：${errorText(caught)}`
            dispatchSingleResultDecision({ type: 'request_failed', projectId, requestId, error })
            setAssistantNote(error)
            return { recorded: true, clarification: false, cancelled: false, failed: true, plan: null, decision: null }
          }
        }
        return { recorded: true, clarification: false, cancelled: false, failed: false, plan: presentation.plan, decision }
      }
      const missingDecisionError = 'Agent 没有返回正式的单一结果决策；当前设计没有变化。'
      dispatchSingleResultDecision({ type: 'request_failed', projectId, requestId, error: missingDecisionError })
      setAssistantNote(missingDecisionError)
      return { recorded: true, clarification: false, cancelled: false, failed: true, plan: null, decision: null }
    } catch (caught) {
      if (!isCurrentAgentConversationRequest(projectId, requestId)) {
        return { recorded: false, clarification: false, cancelled: true, failed: false, plan: null, decision: null }
      }
      if (caught instanceof ForgeApiError && (caught.code === 'DOMAIN_AMBIGUOUS' || caught.code === 'DOMAIN_UNSUPPORTED')) {
        const clarification: AgentClarification = {
          status: caught.code === 'DOMAIN_AMBIGUOUS' ? 'ambiguous' : 'unsupported',
          kind: 'domain',
          question: caught.message,
          options: DEFAULT_AGENT_CLARIFICATION_OPTIONS,
          originalMessage: message,
        }
        if (!receiveAgentClarification(projectId, requestId, clarification)) {
          return { recorded: false, clarification: false, cancelled: true, failed: false, plan: null, decision: null }
        }
        dispatchSingleResultDecision({ type: 'request_cancelled', projectId, requestId })
        setAssistantNote(caught.message)
        return { recorded: false, clarification: true, cancelled: false, failed: false, plan: null, decision: null }
      }
      if (caught instanceof ForgeApiError && isProviderExecutionError(caught.code)) {
        const networkCall = caught.details.network_call_made === true ? 'true' : 'false'
        setAssistantNote(`模型请求失败：${caught.message}（${caught.code}，network_call_made=${networkCall}）。不会切换到离线 Planner；已保存资产没有变化。`)
        dispatchSingleResultDecision({ type: 'request_failed', projectId, requestId, error: caught.message })
        return { recorded: false, clarification: false, cancelled: false, failed: true, plan: null, decision: null }
      }
      // The compatibility planner remains usable when the new kernel is not
      // available yet (for example while an older local Agent is running).
      if (!markAgentKernelUnavailable(projectId, requestId)) {
        return { recorded: false, clarification: false, cancelled: true, failed: false, plan: null, decision: null }
      }
      dispatchSingleResultDecision({ type: 'request_cancelled', projectId, requestId })
      return { recorded: false, clarification: false, cancelled: false, failed: false, plan: null, decision: null }
    }
  }, [agentKernelItems, agentThreadId, api, clearAgentAssetWorkspace, clearBlockoutDisplay, concept.project?.name, concept.project?.project_id, hydrateBlockoutDisplay, isCurrentAgentConversationRequest, markAgentKernelUnavailable, parseAgentTurnPresentation, receiveAgentClarification, receiveAgentTurn, startAgentConversationRequest])

  const cancelActiveProviderTurn = useCallback(async () => {
    if (!activeProviderTurnId && !activeProviderCheckId) return
    try {
      if (activeProviderCheckId) {
        await api.cancelAgentProviderCheck(activeProviderCheckId)
      } else if (activeProviderTurnId) {
        await api.cancelAgentTurn(activeProviderTurnId, `agent-turn-cancel-${Date.now()}`)
      }
      setAssistantNote('正在取消本次模型请求；已保存资产不会变化。')
    } catch (caught) {
      setAssistantNote(`取消请求失败：${errorText(caught)}。请等待当前请求结束后再试。`)
    }
  }, [activeProviderCheckId, activeProviderTurnId, api])

  const previewAgentDirection = useCallback(async (
    directionId: string,
    variationIndex = 0,
    requestedProfile = presentationProfile,
    planOverride?: MechanicalConceptPlan,
  ) => {
    const plan = planOverride ?? agentPlan
    if (!plan) return
    const projectId = concept.project?.project_id ?? null
    const requestId = startDirectionPreview(projectId, directionId, variationIndex)
    setAssistantNote('正在构建当前唯一展示结果…')
    try {
      const result = await api.buildAgentBlockout({
        client_request_id: `agent-blockout-${Date.now()}`,
        plan,
        direction_id: directionId,
        variation_index: variationIndex,
        presentation_profile: requestedProfile,
      })
      if (!receiveBlockoutBuild(projectId, requestId, result.glb_base64, result.shape_program)) return
      clearAgentAssetWorkspace()
      setAgentAssetChangeSet(null)
      setAgentCandidateSelectedPartId(null)
      try {
        const segmentation = await api.segmentAgentBlockout({
          client_request_id: `agent-segment-${Date.now()}`,
          plan,
          direction_id: directionId,
          variant_id: result.variant_id,
          variation_index: result.variation_index,
          presentation_profile: result.presentation_profile,
          artifact_id: result.artifact_id,
        })
        if (!receiveSegmentation(projectId, requestId, segmentation)) return
      } catch {
        if (!failSegmentation(projectId, requestId)) return
      }
      if (!isCurrentDirectionPreview(projectId, requestId)) return
      setAssistantNote(`${requestedProfile === 'showcase' ? '展示模型' : '快速草图'}已生成 ${result.triangle_count.toLocaleString()} 个展示面；确认前不会写入正式版本。`)
    } catch {
      if (!failDirectionPreview(projectId, requestId)) return
      setAssistantNote('blockout 预览生成失败；当前设计仍未写入版本。')
    }
  }, [agentPlan, api, clearAgentAssetWorkspace, concept.project?.project_id, failDirectionPreview, failSegmentation, isCurrentDirectionPreview, presentationProfile, receiveBlockoutBuild, receiveSegmentation, startDirectionPreview])

  const commitAgentBlockout = useCallback(async () => {
    if (!agentBlockoutSegmentation) return
    setAssistantNote('正在把分件候选保存为可编辑资产…')
    try {
      const version = await api.commitAgentBlockout({
        client_request_id: `agent-asset-commit-${Date.now()}`,
        artifact_id: agentBlockoutSegmentation.artifact_id,
        project_id: concept.project?.project_id,
        summary: '确认分件候选并保存为可编辑资产',
      })
      clearAgentEditAssistPresentation()
      setAgentAssetChangeSet(null)
      setAgentCandidateSelectedPartId(null)
      if (concept.project?.project_id) await refreshActiveDesign(concept.project.project_id)
      setAssistantNote(`已保存为可编辑资产 v${version.version_no}；之后的部件修改都会先预览再创建新版本。`)
    } catch (caught) {
      const message = caught instanceof ForgeApiError ? `${caught.message}（${caught.code}）` : '保存可编辑资产失败。'
      setAssistantNote(`${message} 当前仍保留候选预览，未覆盖已有版本。`)
    }
  }, [agentBlockoutSegmentation, api, clearAgentEditAssistPresentation, concept.project?.project_id, refreshActiveDesign])

  const confirmSingleResultPreview = useCallback(async (decision: SingleResultReadyDecision) => {
    if (concept.project?.project_id !== decision.project_id) {
      setAssistantNote('当前项目已切换；不会确认先前项目的临时结果。')
      return
    }
    setAssistantNote('正在把正式单一结果保存为可编辑资产…')
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
        requestId: singleResultDecisionPresentation.latestRequestId,
      })
      setAssistantNote(`已保存为可编辑资产 v${version.version_no}；预览、质量、导出和当前版本将继续由同一 Snapshot 约束。`)
    } catch (caught) {
      const message = caught instanceof ForgeApiError ? `${caught.message}（${caught.code}）` : errorText(caught)
      setAssistantNote(`正式结果保存失败：${message}。当前预览仍未写入版本。`)
    }
  }, [api, clearAgentEditAssistPresentation, concept.project?.project_id, refreshActiveDesign, singleResultDecisionPresentation.latestRequestId])

  const selectAgentPart = useCallback(async (partId: string) => {
    if (
      !agentAssetVersion
      || !activeDesignSnapshot
      || !activeDesignCanSelectParts(activeDesignSnapshot)
      || !('asset_version_id' in activeDesignSnapshot.active_design)
    ) {
      setAgentCandidateSelectedPartId(partId)
      return
    }
    if (activeDesignSnapshot.active_design.asset_version_id !== agentAssetVersion.asset_version_id) {
      setAssistantNote('当前显示的模型不是活动设计版本，正在同步后重试。')
      if (concept.project?.project_id) await refreshActiveDesign(concept.project.project_id)
      return
    }
    const requestId = startActiveDesignRequest('selecting')
    try {
      const response = await api.selectActiveDesignPart(
        activeDesignSnapshot.project_id,
        {
          client_request_id: `active-design-select-${Date.now()}`,
          snapshot_revision: activeDesignSnapshot.revision,
          selected_part_id: partId,
          selected_material_zone_id: agentAssetVersion.parts.find((part) => part.part_id === partId)?.material_zone_ids[0] ?? null,
        },
        { ifMatch: activeDesignState.snapshotEtag ?? undefined },
      )
      if (!receiveActiveDesignSnapshot(activeDesignSnapshot.project_id, requestId, response)) return
      if ('asset_version_id' in response.data.active_design) {
        projectAgentAssetWorkspaceSelection(
          response.data.project_id,
          response.data.active_design.asset_version_id,
          activeDesignSelectedPartId(response.data),
        )
      }
    } catch (caught) {
      const error = failActiveDesignRequest(requestId, caught)
      if (!error) return
      setAssistantNote(error.message)
      if (error.shouldReloadSnapshot && concept.project?.project_id) await refreshActiveDesign(concept.project.project_id)
    }
  }, [activeDesignSnapshot, activeDesignState.snapshotEtag, agentAssetVersion, api, concept.project?.project_id, failActiveDesignRequest, receiveActiveDesignSnapshot, refreshActiveDesign, startActiveDesignRequest])

  const setAgentPartDisplay = useCallback(async (
    action: 'lock' | 'unlock' | 'hide' | 'show' | 'isolate' | 'clear_isolation' | 'show_all',
    partId?: string,
  ) => {
    const snapshot = activeDesignSnapshot
    if (!snapshot || snapshot.active_design.source !== 'agent_asset' || agentAssetChangeSet) {
      setAssistantNote('正在同步当前设计版本；同步完成后才能修改部件显示状态。')
      return
    }
    const requestId = startActiveDesignRequest('setting_part_display')
    try {
      const response = await api.setActiveDesignPartDisplay(
        snapshot.project_id,
        {
          client_request_id: `active-design-part-display-${action}-${Date.now()}`,
          snapshot_revision: snapshot.revision,
          action,
          ...(partId ? { part_id: partId } : {}),
        },
        { ifMatch: activeDesignState.snapshotEtag ?? undefined },
      )
      if (!receiveActiveDesignSnapshot(snapshot.project_id, requestId, response)) return
      if ('asset_version_id' in response.data.active_design) {
        projectAgentAssetWorkspaceSelection(
          response.data.project_id,
          response.data.active_design.asset_version_id,
          activeDesignSelectedPartId(response.data),
        )
      }
      const message = {
        lock: '已锁定这个部件；后续修改会被安全阻止。',
        unlock: '已解除部件锁定。',
        hide: '已隐藏这个部件；模型内容没有被删除。',
        show: '已显示这个部件。',
        isolate: '现在只显示这个部件。',
        clear_isolation: '已结束单独查看。',
        show_all: '已显示所有部件。',
      }[action]
      setAssistantNote(message)
    } catch (caught) {
      const error = failActiveDesignRequest(requestId, caught)
      if (!error) return
      setAssistantNote(error.message)
      if (error.shouldReloadSnapshot) await refreshActiveDesign(snapshot.project_id)
    }
  }, [activeDesignSnapshot, activeDesignState.snapshotEtag, agentAssetChangeSet, api, failActiveDesignRequest, receiveActiveDesignSnapshot, refreshActiveDesign, startActiveDesignRequest])

  const selectMaterialZone = useCallback(async (zoneId: string) => {
    setAppearanceMaterialZoneId(zoneId)
    const selectedPartId = activeDesignSelectedPartId(activeDesignSnapshot)
    if (
      !activeDesignSnapshot
      || !selectedPartId
      || !activeDesignCanSelectParts(activeDesignSnapshot)
      || !('asset_version_id' in activeDesignSnapshot.active_design)
      || legacyDesignReadOnly
    ) return
    const requestId = startActiveDesignRequest('selecting')
    try {
      const response = await api.selectActiveDesignPart(
        activeDesignSnapshot.project_id,
        {
          client_request_id: `active-design-zone-${Date.now()}`,
          snapshot_revision: activeDesignSnapshot.revision,
          selected_part_id: selectedPartId,
          selected_material_zone_id: zoneId,
        },
        { ifMatch: activeDesignState.snapshotEtag ?? undefined },
      )
      if (!receiveActiveDesignSnapshot(activeDesignSnapshot.project_id, requestId, response)) return
      if ('asset_version_id' in response.data.active_design) {
        projectAgentAssetWorkspaceSelection(
          response.data.project_id,
          response.data.active_design.asset_version_id,
          activeDesignSelectedPartId(response.data),
        )
      }
      setAppearanceMaterialZoneId(activeDesignSelectedMaterialZoneId(response.data) ?? zoneId)
    } catch (caught) {
      const error = failActiveDesignRequest(requestId, caught)
      if (!error) return
      setAssistantNote(error.message)
      if (error.shouldReloadSnapshot && concept.project?.project_id) await refreshActiveDesign(concept.project.project_id)
    }
  }, [activeDesignSnapshot, activeDesignState.snapshotEtag, api, concept.project?.project_id, failActiveDesignRequest, legacyDesignReadOnly, receiveActiveDesignSnapshot, refreshActiveDesign, startActiveDesignRequest])

  const requestLegacyAgentRebuild = useCallback(async () => {
    if (
      !activeDesignSnapshot
      || !legacyDesignReadOnly
      || !('legacy_version_id' in activeDesignSnapshot.active_design)
    ) return
    const requestId = startActiveDesignRequest('converting_legacy')
    try {
      const result = await api.convertLegacyActiveDesign(
        activeDesignSnapshot.project_id,
        {
          client_request_id: `legacy-agent-rebuild-${Date.now()}`,
          snapshot_revision: activeDesignSnapshot.revision,
        },
        { ifMatch: activeDesignState.snapshotEtag ?? undefined },
      )
      setAssistantMode('brief')
      setAssistantNote(`${result.data.message} 请描述希望保留或重新设计的外观，Agent 会生成新的可编辑候选。`)
      await refreshActiveDesign(activeDesignSnapshot.project_id)
    } catch (caught) {
      const error = failActiveDesignRequest(requestId, caught)
      if (!error) return
      setAssistantNote(error.message)
      if (error.shouldReloadSnapshot && concept.project?.project_id) await refreshActiveDesign(concept.project.project_id)
    }
  }, [activeDesignSnapshot, activeDesignState.snapshotEtag, api, concept.project?.project_id, failActiveDesignRequest, legacyDesignReadOnly, refreshActiveDesign, startActiveDesignRequest])

  const previewAgentAssetEdit = useCallback(async (operation: AgentPartEditOperation | AgentPartEditOperation[], summary: string) => {
    if (!agentAssetVersion || agentAssetPreviewInFlightRef.current) return
    const operations = Array.isArray(operation) ? operation : [operation]
    const projectId = concept.project?.project_id ?? null
    let previewChangeSetId: string | null = null
    let displayRequestId: number | null = null
    agentAssetPreviewInFlightRef.current = true
    setAssistantNote('正在预览部件修改…')
    try {
      const proposed = await api.proposeAgentAssetChangeSet(agentAssetVersion.asset_version_id, {
        client_request_id: `agent-asset-change-${Date.now()}`,
        summary,
        operations,
      })
      previewChangeSetId = proposed.change_set_id
      const preview = await api.previewAgentAssetChangeSet(proposed.change_set_id, `agent-asset-preview-${Date.now()}`)
      if (!preview.preview) {
        throw new Error('ChangeSet preview did not return an Agent asset candidate')
      }
      displayRequestId = setBlockoutShapeProgram(projectId, preview.preview.shape_program)
      if (displayRequestId === null) {
        throw new Error('ChangeSet preview no longer belongs to the open project')
      }
      const compiled = await api.exportAgentAssetChangeSetPreviewGlb(preview.change_set_id)
      if (
        compiled.baseAssetVersionId !== agentAssetVersion.asset_version_id
        || !compiled.sha256?.match(/^[a-f0-9]{64}$/)
        || !Number.isInteger(compiled.triangleCount)
        || (compiled.triangleCount ?? 0) <= 0
      ) {
        throw new Error('ChangeSet preview GLB metadata does not match the active asset version')
      }
      if (!setBlockoutGlb(projectId, displayRequestId, compiled.glb, 'compiled_agent_preview_pbr')) {
        throw new Error('ChangeSet preview display was superseded by a newer request')
      }
      setAgentAssetChangeSet(preview)
      setAssistantNote(`已生成“${summary}”的真实 PBR 模型预览；确认后才会创建新版本。`)
    } catch {
      if (previewChangeSetId) {
        await api.rejectAgentAssetChangeSet(previewChangeSetId, `agent-asset-preview-cleanup-${Date.now()}`).catch(() => undefined)
      }
      setAgentAssetChangeSet(null)
      const stillOwnsPreviewDisplay = displayRequestId !== null
        && setBlockoutGlb(projectId, displayRequestId, null, null)
      if (stillOwnsPreviewDisplay) {
        const restoreRequestId = setBlockoutShapeProgram(projectId, agentAssetVersion.shape_program)
        if (restoreRequestId !== null) {
          await api.loadAgentAssetPreviewGlb(agentAssetVersion.asset_version_id)
            .then((preview) => {
              setBlockoutGlb(
                projectId,
                restoreRequestId,
                preview.glb,
                preview.artifactProfileId === 'external_reference'
                  ? 'external_reference'
                  : 'compiled_agent_preview_pbr',
              )
            })
            .catch(() => undefined)
        }
      }
      setAssistantNote('真实 PBR 模型预览失败；已取消本次 ChangeSet，当前资产版本没有变化。')
    } finally {
      agentAssetPreviewInFlightRef.current = false
    }
  }, [agentAssetVersion, api, concept.project?.project_id, setBlockoutGlb, setBlockoutShapeProgram])

  const previewAgentAssemblyDelta = useCallback(async (delta: AssemblyDeltaProgram) => {
    if (!agentAssetVersion) {
      setAssistantNote('当前没有可编辑机械臂资产；请先生成并确认一个机械臂。')
      return
    }
    if (agentAssetVersion.asset_version_id !== delta.base_asset_version_id) {
      setAssistantNote('当前机械臂版本已经变化；这条修改已安全丢弃，请重新描述一次。')
      return
    }
    const operations = delta.operations.map((operation, index) => {
      const operationId = `op_${operation.operation_id.replace(/[^A-Za-z0-9_-]/g, '_').slice(0, 112)}_${index}`
      if (operation.op === 'add_reviewed_recipe') {
        return {
          operation_id: operationId,
          op: operation.op,
          part_id: operation.parent_part_id,
          new_part_id: operation.new_part_id,
          parent_connector_id: operation.parent_connector_id,
          child_connector_id: operation.child_connector_id,
          recipe_id: operation.recipe_id,
          slot_id: operation.slot_id,
          transform: operation.transform,
        } as unknown as AgentPartEditOperation
      }
      if (operation.op === 'set_joint_pose') {
        return {
          operation_id: operationId,
          op: operation.op,
          part_id: operation.part_id,
          joint_id: operation.joint_id,
          pose: operation.pose,
        } as unknown as AgentPartEditOperation
      }
      return {
        ...operation,
        operation_id: operationId,
      } as unknown as AgentPartEditOperation
    })
    await previewAgentAssetEdit(operations, delta.summary)
  }, [agentAssetVersion, previewAgentAssetEdit, setAssistantNote])

  const saveSelectedAgentComponent = useCallback(async () => {
    if (!agentAssetVersion || !selectedAgentPart) return
    try {
      const component = await api.saveAgentComponent(agentAssetVersion.asset_version_id, {
        client_request_id: `agent-component-${Date.now()}`,
        part_id: selectedAgentPart.part_id,
        display_name: `${displayPartRole(selectedAgentPart.role)} · 可复用部件`,
        description: `来自 Agent 资产 v${agentAssetVersion.version_no} 的概念部件`,
      })
      const projectId = concept.project?.project_id
      if (projectId) void loadAgentEditAssist(projectId, agentAssetVersion.asset_version_id, selectedAgentPart.part_id)
      setAssistantNote(`已保存「${component.display_name}」到当前项目的 Agent 部件库。`)
    } catch {
      setAssistantNote('保存可复用部件失败；当前资产版本没有变化。')
    }
  }, [agentAssetVersion, api, concept.project?.project_id, loadAgentEditAssist, selectedAgentPart])

  const replaceWithAgentComponent = useCallback(async (candidate: AgentComponentCandidate) => {
    if (!selectedAgentPart) return
    await previewAgentAssetEdit({
      operation_id: `op_replace_${Date.now().toString(36)}`,
      op: 'replace_part',
      part_id: selectedAgentPart.part_id,
      replacement_component_id: candidate.component.component_id,
    }, `替换为「${candidate.component.display_name}」`)
  }, [previewAgentAssetEdit, selectedAgentPart])

  const previewStructureSuggestion = useCallback(async (suggestion: AgentStructureSuggestion) => {
    const operation: AgentPartEditOperation = suggestion.kind === 'split_part'
      ? {
          operation_id: `op_split_${Date.now().toString(36)}`,
          op: 'split_part',
          part_id: suggestion.part_id,
          structure_suggestion_id: suggestion.suggestion_id,
        }
      : {
          operation_id: `op_merge_${Date.now().toString(36)}`,
          op: 'merge_parts',
          part_id: suggestion.part_id,
          target_part_id: suggestion.target_part_id ?? undefined,
          structure_suggestion_id: suggestion.suggestion_id,
        }
    await previewAgentAssetEdit(operation, suggestion.summary)
  }, [previewAgentAssetEdit])

  const confirmAgentAssetEdit = useCallback(async () => {
    if (!agentAssetChangeSet) return
    try {
      const confirmed = await api.confirmAgentAssetChangeSet(agentAssetChangeSet.change_set_id, `agent-asset-confirm-${Date.now()}`)
      setBlockoutShapeProgram(concept.project?.project_id ?? null, confirmed.asset_version.shape_program)
      clearAgentAssetWorkspaceQuality(concept.project?.project_id ?? null)
      if (concept.project?.project_id) await refreshActiveDesign(concept.project.project_id)
      // Keep the ChangeSet present while the new Snapshot and asset workspace
      // hydrate. Quality/export actions use it as their write-transition
      // barrier, so clearing it earlier can submit the superseded asset id and
      // ETag after the server has already advanced the active design.
      setAgentAssetChangeSet(null)
      setAssistantNote(`已确认修改并创建可编辑资产 v${confirmed.asset_version.version_no}。`)
    } catch {
      setAssistantNote('确认部件修改失败；请重新预览，当前版本未被覆盖。')
    }
  }, [agentAssetChangeSet, api, clearAgentAssetWorkspaceQuality, concept.project?.project_id, refreshActiveDesign, setBlockoutShapeProgram])

  const rejectAgentAssetEdit = useCallback(async () => {
    if (!agentAssetChangeSet) return
    try {
      await api.rejectAgentAssetChangeSet(agentAssetChangeSet.change_set_id, `agent-asset-reject-${Date.now()}`)
      setAgentAssetChangeSet(null)
      if (agentAssetChangeSet.project_id) await refreshActiveDesign(agentAssetChangeSet.project_id)
      setAssistantNote('已取消本次部件修改；当前资产版本没有变化。')
    } catch {
      setAssistantNote('取消修改失败，请稍后重试。')
    }
  }, [agentAssetChangeSet, api, refreshActiveDesign])

  const surfaceAdornmentAdapter = useMemo<SurfaceAdornmentAdapter>(() => ({
    enable: async () => {
      try {
        await api.enableSurfaceAdornmentSkill(`surface-adornment-enable-${Date.now()}`)
        return { status: 'enabled' as const }
      } catch (caught) {
        return {
          status: 'failed' as const,
          message: caught instanceof Error ? caught.message : '启用外观细节能力失败。',
        }
      }
    },
    preview: async (target, draft) => {
      if (!agentAssetVersion || target.assetVersionId !== agentAssetVersion.asset_version_id) {
        return { status: 'unavailable' as const, message: '当前模型已切换，请重新选择部件。' }
      }
      const projectId = concept.project?.project_id ?? null
      let changeSetId: string | null = null
      let displayRequestId: number | null = null
      let failureStage = 'SURFACE_ADORNMENT_PROPOSE_FAILED'
      try {
        const clientRequestId = `surface-adornment-${Date.now()}`
        const proposed = await api.proposeSurfaceAdornmentPreview(target.assetVersionId, {
          client_request_id: clientRequestId,
          part_id: target.partId,
          material_zone_id: target.materialZoneId,
          ...compileSurfaceAdornmentDraft(draft),
        })
        changeSetId = proposed.change_set_id
        failureStage = 'SURFACE_ADORNMENT_CHANGE_SET_PREVIEW_FAILED'
        const preview = await api.previewAgentAssetChangeSet(
          proposed.change_set_id,
          `surface-adornment-preview-${Date.now()}`,
        )
        if (!preview.preview) throw new Error('外观细节预览没有返回可验证模型。')
        failureStage = 'SURFACE_ADORNMENT_VIEWPORT_STAGE_FAILED'
        displayRequestId = setBlockoutShapeProgram(projectId, preview.preview.shape_program)
        if (displayRequestId === null) throw new Error('当前项目已切换。')
        failureStage = 'SURFACE_ADORNMENT_PREVIEW_GLB_FAILED'
        const compiled = await api.exportAgentAssetChangeSetPreviewGlb(preview.change_set_id)
        failureStage = 'SURFACE_ADORNMENT_PREVIEW_GLB_IDENTITY_FAILED'
        if (
          compiled.baseAssetVersionId !== target.assetVersionId
          || !compiled.sha256?.match(/^[a-f0-9]{64}$/)
          || !Number.isInteger(compiled.triangleCount)
          || (compiled.triangleCount ?? 0) <= 0
        ) {
          throw new Error('外观细节 GLB 与当前模型版本不一致。')
        }
        failureStage = 'SURFACE_ADORNMENT_VIEWPORT_COMMIT_FAILED'
        if (!setBlockoutGlb(projectId, displayRequestId, compiled.glb, 'compiled_agent_preview_pbr')) {
          throw new Error('外观细节预览已被更新的请求取代。')
        }
        setAgentAssetChangeSet(preview)
        return {
          status: 'preview_ready' as const,
          changeSetId: preview.change_set_id,
          summary: '已在同一个 3D 视口中加载真实 PBR 外观细节预览；保留后才创建新版本。',
        }
      } catch (caught) {
        if (changeSetId) {
          await api.rejectAgentAssetChangeSet(
            changeSetId,
            `surface-adornment-cleanup-${Date.now()}`,
          ).catch(() => undefined)
        }
        if (caught instanceof ForgeApiError && caught.code === 'SURFACE_ADORNMENT_SKILL_DISABLED') {
          return { status: 'activation_required' as const, message: caught.message }
        }
        if (displayRequestId !== null) setBlockoutGlb(projectId, displayRequestId, null, null)
        return {
          status: 'failed' as const,
          message: caught instanceof Error ? caught.message : '外观细节预览失败；当前版本没有变化。',
          errorCode: caught instanceof ForgeApiError
            ? caught.code
            : typeof caught === 'object' && caught !== null && 'code' in caught && typeof caught.code === 'string'
              ? caught.code
              : failureStage,
        }
      }
    },
    retain: async (changeSetId) => {
      try {
        const confirmed = await api.confirmAgentAssetChangeSet(
          changeSetId,
          `surface-adornment-confirm-${Date.now()}`,
        )
        setBlockoutShapeProgram(concept.project?.project_id ?? null, confirmed.asset_version.shape_program)
        clearAgentAssetWorkspaceQuality(concept.project?.project_id ?? null)
        if (concept.project?.project_id) await refreshActiveDesign(concept.project.project_id)
        setAgentAssetChangeSet(null)
        return {
          status: 'retained' as const,
          summary: `已保留外观细节并创建可编辑资产 v${confirmed.asset_version.version_no}。`,
        }
      } catch (caught) {
        return {
          status: 'failed' as const,
          message: caught instanceof Error ? caught.message : '保留外观细节失败；当前版本没有变化。',
        }
      }
    },
    cancel: async (changeSetId) => {
      await api.rejectAgentAssetChangeSet(
        changeSetId,
        `surface-adornment-reject-${Date.now()}`,
      ).catch(() => undefined)
      setAgentAssetChangeSet(null)
      const projectId = concept.project?.project_id ?? null
      if (agentAssetVersion) {
        const requestId = setBlockoutShapeProgram(projectId, agentAssetVersion.shape_program)
        if (requestId !== null) {
          await api.loadAgentAssetPreviewGlb(agentAssetVersion.asset_version_id)
            .then((preview) => {
              setBlockoutGlb(projectId, requestId, preview.glb, 'compiled_agent_preview_pbr')
            })
            .catch(() => undefined)
        }
      }
      if (projectId) await refreshActiveDesign(projectId).catch(() => undefined)
    },
  }), [
    agentAssetVersion,
    api,
    clearAgentAssetWorkspaceQuality,
    concept.project?.project_id,
    refreshActiveDesign,
    setBlockoutGlb,
    setBlockoutShapeProgram,
  ])

  const referenceEvidenceAdapter = useMemo<ReferenceEvidenceAdapter>(() => ({
    invalidate: () => {
      referenceEvidenceRequestEpochRef.current += 1
      referenceRebuildPlanByChangeSetRef.current.clear()
    },
    createEvidence: async ({ target, file, sourceStatement, licenseStatement, missingViews, referenceClass, notes }) => {
      const epoch = referenceEvidenceRequestEpochRef.current
      try {
        const kind = file.name.toLowerCase().endsWith('.glb') || file.type === 'model/gltf-binary' ? 'glb' as const : 'image' as const
        // R007 deliberately bypasses imports:glb. That legacy-compatible
        // endpoint creates an external AgentAssetVersion and advances the
        // Snapshot. Evidence bytes instead enter the Rust-owned read-only CAS
        // path, which still performs the same strict GLB inspection but has
        // zero project/version side effects before a rebuild is confirmed.
        const contentBase64 = arrayBufferToBase64(await file.arrayBuffer())
        if (epoch !== referenceEvidenceRequestEpochRef.current) {
          return { status: 'unavailable' as const, message: '参考输入已关闭或项目已切换；未继续创建重建预览。' }
        }
        const created = await api.createReferenceEvidence({
          client_request_id: `reference-evidence-${Date.now()}`,
          project_id: target.projectId,
          domain_pack_id: target.domainPackId ?? 'pack_robotic_arm_concept',
          kind,
          file_name: file.name,
          media_type: kind === 'glb' ? 'model/gltf-binary' : file.type,
          source_statement: sourceStatement,
          license_statement: licenseStatement,
          missing_views: missingViews,
          ...(kind === 'image' && referenceClass ? { reference_class: referenceClass } : {}),
          ...(notes ? { user_notes: notes } : {}),
          content_base64: contentBase64,
        })
        if (epoch !== referenceEvidenceRequestEpochRef.current) {
          return { status: 'unavailable' as const, message: '参考输入已关闭或项目已切换；证据已保持只读，未继续生成预览。' }
        }
        const record = created.reference_evidence
        return {
          status: 'created' as const,
          evidence: {
            evidenceId: record.evidence_id,
            contentSha256: record.source_object_sha256,
            kind: record.kind,
            fileName: record.source_file_name,
            sourceStatement: record.source_statement,
            licenseStatement: record.license_statement,
            missingViews: record.missing_views,
            uncertainties: record.observations?.uncertainties ?? [],
            referenceClass: record.reference_class,
          },
        }
      } catch (caught) {
        return {
          status: 'failed' as const,
          message: caught instanceof Error ? caught.message : '保存参考证据失败；当前设计没有变化。',
        }
      }
    },
    previewRebuild: async (target, evidence: ReferenceEvidenceRecord) => {
      const epoch = referenceEvidenceRequestEpochRef.current
      let changeSetId: string | null = null
      try {
        if (!target.baseAssetVersionId) {
          return {
            status: 'unavailable' as const,
            message: '请先生成并确认机械臂生产基准，再使用参考重建；当前设计没有变化。',
          }
        }
        const proposed = await api.proposeReferenceGuidedRebuildPreview(target.projectId, {
          client_request_id: `reference-rebuild-${Date.now()}`,
          evidence_id: evidence.evidenceId,
          domain_pack_id: target.domainPackId ?? 'pack_robotic_arm_concept',
          base_asset_version_id: target.baseAssetVersionId,
        })
        changeSetId = proposed.changeSet.change_set_id
        const draftLineage = readReferenceRebuildExactLineage(proposed.planRead, {
          evidenceId: evidence.evidenceId,
          sourceObjectSha256: evidence.contentSha256,
        })
        if (
          proposed.planRead.reference_guided_rebuild_plan.project_id !== target.projectId
          || !draftLineage
          || draftLineage.status !== 'draft'
        ) {
          throw new Error('参考重建计划没有返回可验证的冻结证据谱系。')
        }
        const preview = await api.previewAgentAssetChangeSet(changeSetId, `reference-rebuild-preview-${Date.now()}`)
        if (!preview.preview) throw new Error('参考引导重建没有返回可验证的 ShapeProgram 预览。')
        const planRead = await api.getReferenceGuidedRebuildPlan(target.projectId, draftLineage.rebuildPlanId)
        const lineage = readReferenceRebuildExactLineage(planRead, {
          evidenceId: evidence.evidenceId,
          sourceObjectSha256: evidence.contentSha256,
          previewChangeSetId: changeSetId,
        })
        if (
          planRead.reference_guided_rebuild_plan.project_id !== target.projectId
          || !lineage
          || lineage.status !== 'previewed'
          || lineage.rebuildPlanId !== draftLineage.rebuildPlanId
        ) {
          throw new Error('参考重建预览与冻结证据谱系不一致，已拒绝此次预览。')
        }
        const compiled = await api.exportAgentAssetChangeSetPreviewGlb(changeSetId)
        if (!compiled.sha256?.match(/^[a-f0-9]{64}$/) || !Number.isInteger(compiled.triangleCount) || (compiled.triangleCount ?? 0) <= 0) {
          throw new Error('参考引导重建预览没有返回可验证 GLB。')
        }
        if (epoch !== referenceEvidenceRequestEpochRef.current) {
          await api.rejectAgentAssetChangeSet(changeSetId, `reference-rebuild-late-reject-${Date.now()}`).catch(() => undefined)
          return { status: 'unavailable' as const, message: '参考预览已过期并被取消；当前设计没有变化。' }
        }
        // The read-only reference image/GLB is only a transient A/B display.
        // The preview itself must replace it in the same renderer so the
        // reference pixels can never be mistaken for generated geometry.
        replaceReferenceViewport(null)
        const displayRequestId = setBlockoutShapeProgram(target.projectId, preview.preview.shape_program)
        if (displayRequestId === null || !setBlockoutGlb(target.projectId, displayRequestId, compiled.glb, 'compiled_agent_preview_pbr')) {
          await api.rejectAgentAssetChangeSet(changeSetId, `reference-rebuild-display-reject-${Date.now()}`).catch(() => undefined)
          return { status: 'unavailable' as const, message: '当前项目已切换；参考预览已取消。' }
        }
        setAgentAssetChangeSet(preview)
        referenceRebuildPlanByChangeSetRef.current.set(changeSetId, {
          projectId: target.projectId,
          baseAssetVersionId: target.baseAssetVersionId,
          evidenceId: evidence.evidenceId,
          sourceObjectSha256: evidence.contentSha256,
          rebuildPlanId: lineage.rebuildPlanId,
        })
        return {
          status: 'preview_ready' as const,
          changeSetId,
          summary: '已在同一个 3D 视口加载新的可编辑机械臂重建预览；参考源仍保持只读，保留后才创建版本。',
          // R007B only renders the three existing plan lists if a compatible
          // response exposes them. A ChangeSet-only response must not invent
          // evidence, intended changes, or unknown geometry in the UI.
          comparison: readReferenceRebuildComparisonPlan(planRead) ?? undefined,
          lineage,
        }
      } catch (caught) {
        if (changeSetId) {
          await api.rejectAgentAssetChangeSet(changeSetId, `reference-rebuild-cleanup-${Date.now()}`).catch(() => undefined)
          referenceRebuildPlanByChangeSetRef.current.delete(changeSetId)
        }
        return {
          status: 'failed' as const,
          message: referenceRebuildFailureMessage(caught),
        }
      }
    },
    retain: async (changeSetId) => {
      const binding = referenceRebuildPlanByChangeSetRef.current.get(changeSetId)
      if (!binding || binding.projectId !== concept.project?.project_id) {
        return {
          status: 'failed' as const,
          message: '参考重建预览缺少当前项目的冻结谱系，未执行确认。',
        }
      }
      const epoch = referenceEvidenceRequestEpochRef.current
      try {
        const previewRead = await api.getReferenceGuidedRebuildPlan(binding.projectId, binding.rebuildPlanId)
        const previewLineage = readReferenceRebuildExactLineage(previewRead, {
          evidenceId: binding.evidenceId,
          sourceObjectSha256: binding.sourceObjectSha256,
          previewChangeSetId: changeSetId,
        })
        if (
          previewRead.reference_guided_rebuild_plan.project_id !== binding.projectId
          || !previewLineage
          || previewLineage.status !== 'previewed'
        ) {
          throw new Error('确认前参考谱系已发生变化，未创建新版本。')
        }
        if (
          epoch !== referenceEvidenceRequestEpochRef.current
          || concept.project?.project_id !== binding.projectId
        ) {
          return {
            status: 'unavailable' as const,
            message: '项目已切换；没有把旧项目的参考预览确认到当前项目。',
          }
        }
        const confirmed = await api.confirmAgentAssetChangeSet(changeSetId, `reference-rebuild-confirm-${Date.now()}`)
        const confirmedRead = await api.getReferenceGuidedRebuildPlan(binding.projectId, binding.rebuildPlanId)
        const lineage = readReferenceRebuildExactLineage(confirmedRead, {
          evidenceId: binding.evidenceId,
          sourceObjectSha256: binding.sourceObjectSha256,
          previewChangeSetId: changeSetId,
        })
        if (
          confirmedRead.reference_guided_rebuild_plan.project_id !== binding.projectId
          || !lineage
          || lineage.status !== 'confirmed'
          || lineage.confirmedAssetVersionId !== confirmed.asset_version.asset_version_id
        ) {
          throw new Error('新版本已提交，但返回的生产 GLB 谱系无法验证；请重新打开项目核对。')
        }
        if (
          epoch !== referenceEvidenceRequestEpochRef.current
          || concept.project?.project_id !== binding.projectId
        ) {
          referenceRebuildPlanByChangeSetRef.current.delete(changeSetId)
          return {
            status: 'unavailable' as const,
            message: '参考重建已在原项目确认；当前项目保持不变，请返回原项目查看结果。',
          }
        }
        const projectId = concept.project?.project_id ?? null
        let retainedDisplaySummary = ''
        if (projectId) {
          clearAgentAssetWorkspaceQuality(projectId)
          await refreshActiveDesign(projectId)
          // The Snapshot/workspace refresh intentionally starts its
          // preview→production replacement in the background. Rebind the
          // visible retain action after that refresh so a late V2 load cannot
          // win the display request and leave the confirmed V3 viewport
          // empty. This still consumes the exact Rust-owned version objects;
          // it creates no second renderer or geometry truth.
          const displayRequestId = setBlockoutShapeProgram(
            projectId,
            confirmed.asset_version.shape_program,
          )
          if (displayRequestId !== null) {
            try {
              const production = await api.loadAgentAssetProductionGlb(
                confirmed.asset_version.asset_version_id,
              )
              if (production.artifactProfileId !== 'production_concept') {
                throw new Error('Production GLB response did not use the production concept profile')
              }
              if (!setBlockoutGlb(
                projectId,
                displayRequestId,
                production.glb,
                'compiled_agent_production_pbr',
              )) {
                retainedDisplaySummary = ' 当前项目已切换，结果保留在原项目。'
              }
            } catch {
              try {
                const preview = await api.loadAgentAssetPreviewGlb(
                  confirmed.asset_version.asset_version_id,
                )
                if (!setBlockoutGlb(
                  projectId,
                  displayRequestId,
                  preview.glb,
                  'compiled_agent_preview_pbr',
                )) {
                  retainedDisplaySummary = ' 当前项目已切换，结果保留在原项目。'
                } else {
                  retainedDisplaySummary = ' 生产工件暂不可用，当前明确显示同源轻量预览。'
                }
              } catch {
                setBlockoutGlb(projectId, displayRequestId, null, null)
                retainedDisplaySummary = ' 新版本已保存，但其 PBR 视图暂不可读取；没有继续显示旧版本。'
              }
            }
          }
        }
        setAgentAssetChangeSet(null)
        referenceRebuildPlanByChangeSetRef.current.delete(changeSetId)
        return {
          status: 'retained' as const,
          summary: `已保留参考引导重建并创建可编辑资产 v${confirmed.asset_version.version_no}。${retainedDisplaySummary}`,
          lineage,
        }
      } catch (caught) {
        return {
          status: 'failed' as const,
          message: caught instanceof Error ? caught.message : '确认参考引导重建失败；当前版本未被覆盖。',
        }
      }
    },
    cancel: async (changeSetId) => {
      const binding = referenceRebuildPlanByChangeSetRef.current.get(changeSetId)
      if (!binding) throw new Error('参考重建预览缺少原项目绑定；未执行取消。')
      const epoch = referenceEvidenceRequestEpochRef.current
      // Drawer close is terminal only after a read-only, same-project Snapshot
      // read proves Rust cleared the preview and retained the exact base asset.
      // Do not call refreshActiveDesign here: its workbench hydration changes
      // drawer/load effects before the close promise can settle.
      await api.rejectAgentAssetChangeSet(changeSetId, `reference-rebuild-reject-${Date.now()}`)
      const readback = await api.getActiveDesign(binding.projectId)
      const snapshot = readback.data
      if (
        snapshot.project_id !== binding.projectId
        || (snapshot.preview !== null && snapshot.preview !== undefined)
        || snapshot.active_design.project_id !== binding.projectId
        || !('asset_version_id' in snapshot.active_design)
        || snapshot.active_design.asset_version_id !== binding.baseAssetVersionId
      ) {
        throw new Error('取消后的当前设计读回不一致；抽屉保持打开，请重试。')
      }
      referenceRebuildPlanByChangeSetRef.current.delete(changeSetId)
      if (
        epoch === referenceEvidenceRequestEpochRef.current
        && concept.project?.project_id === binding.projectId
      ) {
        setAgentAssetChangeSet(null)
        replaceReferenceViewport(null)
      }
    },
    loadHistory: async (target) => {
      const index = await api.listProjectReferenceEvidence(target.projectId)
      const plans = await Promise.all(index.reference_guided_rebuild_plans.map(async (plan) => {
        const read = await api.getReferenceGuidedRebuildPlan(target.projectId, plan.rebuild_plan_id)
        return read
      }))
      const planByEvidenceId = new Map(plans.map((read) => [
        read.reference_guided_rebuild_plan.evidence_id,
        read,
      ]))
      return index.reference_evidence.map((record): ReferenceEvidenceHistoryEntry => {
        const read = planByEvidenceId.get(record.evidence_id)
        const plan = read?.reference_guided_rebuild_plan
        const lineage = read && plan?.project_id === target.projectId
          ? readReferenceRebuildExactLineage(read, {
            evidenceId: record.evidence_id,
            sourceObjectSha256: record.source_object_sha256,
          })
          : null
        return {
          evidence: {
            evidenceId: record.evidence_id,
            contentSha256: record.source_object_sha256,
            kind: record.kind,
            fileName: record.source_file_name,
            sourceStatement: record.source_statement,
            licenseStatement: record.license_statement,
            missingViews: record.missing_views,
            uncertainties: record.observations?.uncertainties ?? [],
            referenceClass: record.reference_class,
          },
          comparison: plan ? {
            retainedEvidence: plan.retained_evidence,
            intendedDifferences: plan.intended_differences,
            unresolvedUncertainties: plan.unresolved_uncertainties,
          } : null,
          rebuildPlanId: plan?.rebuild_plan_id ?? null,
          resultAssetVersionId: read?.reference_result_pair?.result_asset_version_id ?? null,
          lineage,
        }
      })
    },
    loadContent: async (target, evidence) => {
      const content = await api.loadReferenceEvidenceContent(target.projectId, evidence.evidenceId)
      return content.blob
    },
    viewReferenceImage: async (target, evidence) => {
      const epoch = referenceEvidenceRequestEpochRef.current
      try {
        const content = await api.loadReferenceEvidenceContent(target.projectId, evidence.evidenceId)
        if (!content.mediaType.startsWith('image/')) {
          replaceReferenceViewport(null)
          return { status: 'failed' as const, message: '参考证据不是可在同一视口显示的图片。' }
        }
        if (epoch !== referenceEvidenceRequestEpochRef.current || concept.project?.project_id !== target.projectId) {
          return { status: 'unavailable' as const, message: '项目已切换；没有加载过期的参考图片。' }
        }
        const referenceClass = evidence.referenceClass === 'multi_view_contact_sheet'
          ? 'multi_view_contact_sheet' as const
          : 'single_image' as const
        const imageUrl = URL.createObjectURL(content.blob)
        if (epoch !== referenceEvidenceRequestEpochRef.current || concept.project?.project_id !== target.projectId) {
          URL.revokeObjectURL(imageUrl)
          return { status: 'unavailable' as const, message: '项目已切换；没有显示过期的参考图片。' }
        }
        replaceReferenceViewport({
          projectId: target.projectId,
          evidenceId: evidence.evidenceId,
          sourceObjectSha256: evidence.contentSha256,
          referenceClass,
          kind: 'image',
          imageUrl,
        })
        return { status: 'ready' as const, message: '已在同一个 3D 视口显示只读参考图片；它只是纹理化对照，不成为几何或版本真值。' }
      } catch (caught) {
        replaceReferenceViewport(null)
        return { status: 'failed' as const, message: caught instanceof Error ? caught.message : '参考图片无法读取；已回到当前结果。' }
      }
    },
    viewReferenceGlb: async (target, evidence) => {
      const epoch = referenceEvidenceRequestEpochRef.current
      try {
        const content = await api.loadReferenceEvidenceContent(target.projectId, evidence.evidenceId)
        if (content.mediaType !== 'model/gltf-binary') {
          return { status: 'failed' as const, message: '参考证据不是可在 3D 视口读取的 GLB。' }
        }
        if (epoch !== referenceEvidenceRequestEpochRef.current || concept.project?.project_id !== target.projectId) {
          return { status: 'unavailable' as const, message: '项目已切换；没有加载过期的参考 GLB。' }
        }
        const glb = await content.blob.arrayBuffer()
        // Blob decoding is asynchronous too. Re-check after it completes so a
        // project switch during arrayBuffer() cannot paint an old reference
        // into the current project's one shared viewport.
        if (epoch !== referenceEvidenceRequestEpochRef.current || concept.project?.project_id !== target.projectId) {
          return { status: 'unavailable' as const, message: '项目已切换；没有加载过期的参考 GLB。' }
        }
        replaceReferenceViewport({
          projectId: target.projectId,
          evidenceId: evidence.evidenceId,
          sourceObjectSha256: evidence.contentSha256,
          referenceClass: 'strict_glb_readback',
          kind: 'glb',
          glb,
        })
        return { status: 'ready' as const, message: '已在同一个 3D 视口查看只读参考 GLB；它不会成为可编辑资产。' }
      } catch (caught) {
        return { status: 'failed' as const, message: caught instanceof Error ? caught.message : '参考 GLB 无法读取；当前结果保持不变。' }
      }
    },
    viewResult: (target) => {
      if (concept.project?.project_id === target.projectId) replaceReferenceViewport(null)
    },
  }), [
    api,
    clearAgentAssetWorkspaceQuality,
    concept.project?.project_id,
    refreshActiveDesign,
    setBlockoutGlb,
    setBlockoutShapeProgram,
    replaceReferenceViewport,
  ])

  const navigateAgentAsset = useCallback(async (action: 'undo' | 'redo') => {
    if (!activeDesignSnapshot || !activeAgentAssetVersion || agentAssetChangeSet) return
    const requestId = startActiveDesignRequest(action === 'undo' ? 'undoing' : 'redoing')
    setAssistantNote(action === 'undo' ? '正在返回上一个 Agent 资产版本…' : '正在重做上一次 Agent 修改…')
    try {
      const input = {
        client_request_id: `active-design-${action}-${Date.now()}`,
        snapshot_revision: activeDesignSnapshot.revision,
      }
      const response = action === 'undo'
        ? await api.undoActiveDesign(activeDesignSnapshot.project_id, input, { ifMatch: activeDesignState.snapshotEtag ?? undefined })
        : await api.redoActiveDesign(activeDesignSnapshot.project_id, input, { ifMatch: activeDesignState.snapshotEtag ?? undefined })
      if (!receiveActiveDesignSnapshot(activeDesignSnapshot.project_id, requestId, response)) return
      setAgentAssetChangeSet(null)
      await refreshActiveDesign(activeDesignSnapshot.project_id)
      setAssistantNote(action === 'undo'
        ? '已返回上一版内容，并创建新的可恢复资产版本。'
        : '已重做上一次内容，并创建新的可恢复资产版本。')
    } catch (caught) {
      const error = failActiveDesignRequest(requestId, caught)
      if (!error) return
      setAssistantNote(error.message)
      if (error.shouldReloadSnapshot && concept.project?.project_id) await refreshActiveDesign(concept.project.project_id)
    }
  }, [activeAgentAssetVersion, activeDesignSnapshot, activeDesignState.snapshotEtag, agentAssetChangeSet, api, concept.project?.project_id, failActiveDesignRequest, receiveActiveDesignSnapshot, refreshActiveDesign, startActiveDesignRequest])

  const inspectAgentAsset = useCallback(async () => {
    if (!activeAgentAssetVersion) {
      setAssistantNote('请先同步一个活动 Agent 资产，再运行检查。')
      return
    }
    if (!activeDesignState.snapshotEtag) {
      setAssistantNote('当前工作台版本尚未同步完成；请稍后再检查模型。')
      return
    }
    setAssistantNote('正在检查当前 Agent 资产…')
    try {
      const report = await api.qualityAgentAssetVersion(activeAgentAssetVersion.asset_version_id, {
        idempotencyKey: `agent-asset-quality-${Date.now()}`,
        ifMatch: activeDesignState.snapshotEtag,
      })
      clearAgentAssetWorkspaceQuality(activeAgentAssetVersion.project_id)
      if (activeAgentAssetVersion.project_id) await refreshActiveDesign(activeAgentAssetVersion.project_id)
      setAssistantNote(report.status === 'passed'
        ? `模型检查通过：${report.triangle_count.toLocaleString()} 三角形，部件层级和关节引用正常。`
        : `模型检查${report.status === 'warning' ? '有提示' : '未通过'}：${report.findings?.[0]?.message ?? '请查看检查结果。'}`)
    } catch {
      setAssistantNote('模型检查失败；当前资产版本没有变化。')
    }
  }, [activeAgentAssetVersion, activeDesignState.snapshotEtag, api, clearAgentAssetWorkspaceQuality, refreshActiveDesign])

  const submitAssistantInstruction = async () => {
    return submitAssistantInstructionWithText(chatInput.trim() || DEFAULT_CONCEPT_BRIEF)
  }

  const submitAssistantInstructionWithText = async (requestedText: string, clarificationDomainPackId?: string) => {
    const instruction = requestedText.trim() || DEFAULT_CONCEPT_BRIEF
    setAssistantNote(`正在解释 Brief：“${instruction}”`)
    const kernelResult = await recordAgentTurn(instruction, clarificationDomainPackId)
    if (kernelResult.cancelled) return
    if (kernelResult.failed) {
      setChatInput('')
      return
    }
    if (kernelResult.clarification) {
      setChatInput('')
      return
    }
    if (kernelResult.decision) {
      setAssistantNote(kernelResult.decision.state === 'ready_for_preview'
        ? '本次唯一结果已通过正式生成质量门；确认前不会创建可编辑版本。'
        : '本次正式生成未产生可展示结果；当前设计没有变化。')
      setChatInput('')
      return
    }
    if (legacyDesignReadOnly) {
      if (!kernelResult.recorded) {
        setAssistantNote('请先点击“让 Agent 重建可编辑资产”，并确认本地 Agent 已启动。旧版设计不会被修改。')
      } else {
        setAssistantNote('Agent 没有返回可构建的单一结果；旧版数据仍保持只读且没有变化。')
      }
      setChatInput('')
      return
    }
    setAssistantNote(kernelResult.recorded
      ? 'Agent 计划没有返回可构建结果；当前设计没有变化。'
      : '当前 Agent 计划未记录成功；不会调用旧版 Planner 作为替代。')
    setChatInput('')
  }

  const previewChangeInstruction = async () => {
    if (legacyDesignReadOnly) {
      setAssistantNote('旧版设计为只读状态。请先让 Agent 重建为可编辑资产。')
      return
    }
    const instruction = chatInput.trim()
    if (!instruction) return
    setAssistantNote(`正在规划修改：“${instruction}”`)
    const kernelResult = await recordAgentTurn(instruction)
    if (kernelResult.cancelled) return
    if (kernelResult.failed) {
      setChatInput('')
      return
    }
    if (kernelResult.clarification) {
      setChatInput('')
      return
    }
    if (kernelResult.plan?.assembly_delta) {
      await previewAgentAssemblyDelta(kernelResult.plan.assembly_delta)
    } else {
      setAssistantNote(kernelResult.recorded
        ? 'Agent 没有生成针对当前版本的受限 AssemblyDelta；当前资产没有变化，请明确描述“在当前机械臂上增加/替换/调整什么”。'
        : '修改意图未记录成功；当前资产没有变化。')
    }
    setChatInput('')
  }

  const runAssistantAction = () => (
    assistantMode === 'brief'
      ? submitAssistantInstruction()
      : previewChangeInstruction()
  )

  const saveProvider = useCallback(async () => {
    if (!providerApiKey.trim()) {
      setAssistantNote('请填写 API Key；密钥只会保存到 macOS Keychain，不会写入项目。')
      return
    }
    setProviderSaving(true)
    try {
      const saved = await saveTauriProviderConfig({
        base_url: providerBaseUrl,
        model: providerModel,
        api_key: providerApiKey,
      })
      setProviderConfig(saved)
      setProviderApiKey('')
      void checkService()
      if (saved.metadata_status !== 'valid' || saved.secret_status !== 'available') {
        setAssistantNote(`配置尚未启用：${saved.failure_code ?? 'Provider metadata 或 Keychain 未通过验证'}。没有发起 DeepSeek 请求。`)
      } else if (saved.supervisor_status !== 'running' || saved.capability_status !== 'ready') {
        setAssistantNote(`密钥已安全保存，但 Agent 尚未载入新配置：${saved.failure_code ?? '本地 capability 不匹配'}。没有发起 DeepSeek 请求，请先修复服务状态。`)
      } else {
        setProviderSetupOpen(false)
        setAssistantNote('模型服务配置、Keychain、Agent 重启和本地 capability 均已验证；尚未发起收费请求，可点击“测试连接”。')
      }
    } catch (caught) {
      setAssistantNote(`模型服务配置失败：${errorText(caught)}`)
    } finally {
      setProviderSaving(false)
    }
  }, [checkService, providerApiKey, providerBaseUrl, providerModel])

  const testProvider = useCallback(async () => {
    setProviderSaving(true)
    const checkId = `provider-check-${Date.now()}`
    setActiveProviderCheckId(checkId)
    try {
      const result = await api.checkAgentProvider(checkId)
      setAssistantNote(providerCheckPresentation(result))
    } catch (caught) {
      const detail = caught instanceof ForgeApiError
        ? `${caught.message}（${caught.code}，network_call_made=${caught.details?.network_call_made === true ? 'true' : 'unknown'}）`
        : errorText(caught)
      setAssistantNote(`模型服务测试未完成：${detail}。不会静默切换为离线成功，已保存设计没有变化。`)
    } finally {
      setActiveProviderCheckId(null)
      setProviderSaving(false)
    }
  }, [api])

  const importGlbReference = useCallback(async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    event.target.value = ''
    if (!file) return
    if (!concept.project?.project_id) {
      setAssistantNote('请先创建或打开一个设计项目，再导入 GLB。')
      return
    }
    if (!file.name.toLowerCase().endsWith('.glb')) {
      setAssistantNote('当前只支持自包含的 .glb 文件。')
      return
    }
    if (file.size > 32 * 1024 * 1024) {
      setAssistantNote('GLB 超过 32 MB 轻量导入限制；请先在 DCC 软件中简化。')
      return
    }
    setImportingGlb(true)
    setAssistantNote(`正在检查并导入「${file.name}」…`)
    try {
      const payload = arrayBufferToBase64(await file.arrayBuffer())
      const response = await api.importAgentGlb({
        client_request_id: `agent-glb-import-${Date.now()}`,
        project_id: concept.project.project_id,
        domain_pack_id: agentPlan?.domain_pack_id ?? inferImportDomainPack(file.name),
        file_name: file.name,
        glb_base64: payload,
        summary: `导入参考模型：${file.name}`,
      })
      const version = response.asset_version
      setAgentAssetChangeSet(null)
      hydrateBlockoutDisplay(concept.project.project_id, {
        glbBase64: payload,
        glbKind: 'external_reference',
        shapeProgram: null,
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
      setAgentCandidateSelectedPartId(null)
      clearAgentAssetWorkspaceQuality(concept.project.project_id)
      await refreshActiveDesign(concept.project.project_id)
      setAssistantNote(`已导入参考模型：${response.inspection.triangle_count.toLocaleString()} 三角形、${response.inspection.material_count} 个材质。它不会被伪装成可编辑模型；可让 Agent 依据它重建。`)
    } catch (caught) {
      setAssistantNote(`GLB 导入失败：${errorText(caught)}`)
    } finally {
      setImportingGlb(false)
    }
  }, [agentPlan?.domain_pack_id, api, clearAgentAssetWorkspaceQuality, concept.project?.project_id, hydrateBlockoutDisplay, refreshActiveDesign])
  const materialEditor = (
    <MaterialDrawer
      materialPresets={materialPresets}
      selectedMaterialId={appearanceMaterialId}
      selectedPartLabel={selectedAgentPart ? `已选部件 · ${displayPartRole(selectedAgentPart.role)}` : '当前预览部件'}
      selectedZoneLabel={selectedMaterialZoneId ? `材质区 ${selectedMaterialZoneId}` : '主材质区'}
      materialZoneIds={selectedAgentPart?.material_zone_ids ?? []}
      selectedZoneId={selectedMaterialZoneId}
      activeDomain={activeMaterialDomain}
      compatibilityOnly={materialCompatibilityOnly}
      query={materialQuery}
      category={materialCategory}
      catalogLoading={agentMaterialCatalogPresentation.loading}
      catalogMessage={agentMaterialCatalogPresentation.catalogMessage}
      disabled={isExternalGlbReference || Boolean(agentAssetChangeSet)}
      onMaterialChange={selectMaterialPreselection}
      onZoneChange={(zoneId) => { void selectMaterialZone(zoneId) }}
      onCompatibilityChange={setMaterialFilterCompatibilityOnly}
      onQueryChange={setMaterialFilterQuery}
      onCategoryChange={setMaterialFilterCategory}
      onPreviewMaterial={(preset, zoneId) => {
        if (agentAssetVersion && selectedAgentPart) {
          void previewAgentAssetEdit({
            operation_id: `op_material_${Date.now().toString(36)}`,
            op: 'apply_material_preset',
            part_id: selectedAgentPart.part_id,
            material_id: preset.material_id,
            material_zone_id: zoneId,
          }, `将${zoneId}换成${preset.display_name}`)
        }
      }}
      onPreviewNote={(preset) => setAssistantNote(`已将预览材质切换为「${preset.display_name}」；确认前不会写入版本。`)}
    />
  )
  const visibleSingleResult = singleResultDecisionPresentation.presentation.state === 'ready'
    ? singleResultDecisionPresentation.presentation.decision
    : null

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
          <button
            type="button"
            className="text-action"
            onClick={() => void navigateAgentAsset('undo')}
            disabled={!activeAgentAssetVersion || !agentNavigation?.can_undo || Boolean(agentAssetChangeSet)}
            title="返回上一版 Agent 内容，并保留完整版本历史"
          ><ClockCounterClockwise size={16} /> 撤销</button>
          <button
            type="button"
            className="text-action"
            onClick={() => void navigateAgentAsset('redo')}
            disabled={!activeAgentAssetVersion || !agentNavigation?.can_redo || Boolean(agentAssetChangeSet)}
            title="重做上一次 Agent 修改，并保留完整版本历史"
          ><ArrowsClockwise size={16} /> 重做</button>
          <button
            type="button"
            className="text-action"
            onClick={() => importGlbInputRef.current?.click()}
            disabled={!concept.project || importingGlb || Boolean(agentAssetChangeSet)}
            title="导入自包含 GLB 作为参考模型"
          ><FolderOpen size={16} /> {importingGlb ? '导入中…' : '导入参考'}</button>
          <button type="button" className="text-action" onClick={openQualityDrawer} aria-label="检查"><Check size={16} /> 检查</button>
          <button type="button" className="export-action" onClick={openExportDrawer} aria-label="导出"><Export size={16} /> 导出</button>
        </div>
        <input
          ref={importGlbInputRef}
          className="visually-hidden"
          type="file"
          accept=".glb,model/gltf-binary"
          onChange={importGlbReference}
          aria-label="导入 GLB 参考模型"
        />
      </header>

      <div
        className={`cad-layout f026-layout ${viewportDock.dockState === 'focus' ? 'is-viewport-focus' : ''}`}
        data-viewport-dock-state={viewportDock.dockState}
      >
        <WorkbenchSidebar
          projects={concept.projects}
          activeProjectId={concept.project?.project_id ?? null}
          threads={agentThreads}
          activeThreadId={agentThreadId}
          parts={agentAssetVersion?.parts ?? agentBlockoutSegmentation?.parts ?? []}
          selectedPartId={displayedAgentSelectedPartId}
          loading={concept.loading || threadHistoryLoading}
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
              onPresentationProfileChange={(profile) => {
                setPresentationProfile(profile)
                if (agentBlockoutSegmentation) {
                  void previewAgentDirection(
                    agentBlockoutSegmentation.direction_id,
                    agentBlockoutSegmentation.variation_index ?? 0,
                    profile,
                  )
                }
              }}
              onClarificationSelect={(option) => void submitAssistantInstructionWithText(
                `${agentClarification?.originalMessage ? `${agentClarification.originalMessage}\n` : ''}${option.prompt}`,
                option.domain_pack_id,
              )}
              agentClarification={agentClarification}
              agentKernelItems={agentKernelItems}
              agentKernelUnavailable={agentKernelUnavailable}
              agentPlan={agentPlan}
            />
            {singleResultDecisionPresentation.presentation.state === 'processing' ? (
              <GenerationResultCard
                state="processing"
                detail={singleResultDecisionPresentation.presentation.detail ?? assistantNote}
              />
            ) : singleResultDecisionPresentation.presentation.state === 'failed' ? (
              <GenerationResultCard
                state="failed"
                error={singleResultDecisionPresentation.presentation.error}
                onRetry={() => void runAssistantAction()}
              />
            ) : singleResultDecisionPresentation.presentation.state === 'ready' ? (
              <GenerationResultCard
                state={'ready'}
                summary={singleResultDecisionPresentation.presentation.decision.summary}
                versionLabel="正式生成质量门已通过 · 确认前不会写入版本"
                onSave={() => {
                  const presentation = singleResultDecisionPresentation.presentation
                  if (presentation.state === 'ready') void confirmSingleResultPreview(presentation.decision)
                }}
                onContinueEditing={() => {
                  setAssistantMode('change')
                  window.requestAnimationFrame(() => document.querySelector<HTMLTextAreaElement>('[aria-label="设计需求"]')?.focus())
                }}
              />
            ) : agentBlockoutDisplay.directionPreviewLoading || concept.loading ? (
              <GenerationResultCard state="processing" detail={assistantNote} />
            ) : agentBlockoutDisplay.previewError ? (
              <GenerationResultCard
                state="failed"
                error="3D 构建或分件检查未通过；当前设计没有变化。"
                onRetry={() => {
                  const currentDirection = agentPlan?.directions[0]
                  if (agentPlan && currentDirection) {
                    void previewAgentDirection(currentDirection.direction_id, 0, presentationProfile, agentPlan)
                    return
                  }
                  void runAssistantAction()
                }}
              />
            ) : agentBlockoutSegmentation || activeAgentAssetVersion ? (
              <GenerationResultCard
                state="compatibility_result"
                summary={activeAgentAssetVersion?.summary
                  ?? agentPlan?.directions[0]?.summary
                  ?? `已生成 ${agentBlockoutSegmentation?.parts.length ?? activeAgentAssetVersion?.parts.length ?? 0} 个可编辑组件。`}
                versionLabel={activeAgentAssetVersion
                  ? `可编辑资产 v${activeAgentAssetVersion.version_no}`
                  : '预览状态 · 确认前不会写入版本'}
                onSave={activeAgentAssetVersion ? undefined : () => void commitAgentBlockout()}
                onContinueEditing={() => {
                  setAssistantMode('change')
                  window.requestAnimationFrame(() => document.querySelector<HTMLTextAreaElement>('[aria-label="设计需求"]')?.focus())
                }}
              />
            ) : (
              <GenerationResultCard state="idle" />
            )}
            {agentBlockoutSegmentation && (
              <details className="f026-result-details" open={Boolean(activeAgentAssetVersion)}>
                <summary>组件与继续编辑</summary>
              <AgentSelectionCard
                segmentation={agentBlockoutSegmentation}
                agentAssetVersion={agentAssetVersion}
                activeAgentAssetVersion={activeAgentAssetVersion}
                selectedPart={selectedAgentPart}
                selectedPartId={displayedAgentSelectedPartId}
                partDisplay={activePartDisplay}
                isSelectedPartLocked={selectedAgentPartLocked}
                isExternalGlbReference={isExternalGlbReference}
                isSnapshotActionPending={activeDesignState.operation !== 'idle'}
                agentAssetChangeSet={agentAssetChangeSet}
                agentComponentCandidates={agentComponentCandidates}
                agentStructureSuggestions={agentStructureSuggestions}
                structureSuggestionUnavailableMessage={structureSuggestionUnavailableMessage}
                semanticProportions={agentEditAssistPresentation.semanticProportions}
                editAssistLoading={agentEditAssistPresentation.loading}
                blockoutPreviewPresentation={blockoutPreviewPresentation}
                onSelectPart={selectAgentPart}
                onPreviewEdit={previewAgentAssetEdit}
                onSaveSelectedComponent={saveSelectedAgentComponent}
                onReplaceComponent={replaceWithAgentComponent}
                onPreviewStructureSuggestion={previewStructureSuggestion}
                onSetPartDisplay={setAgentPartDisplay}
                onInspectAsset={inspectAgentAsset}
                onRejectChange={rejectAgentAssetEdit}
                onConfirmChange={confirmAgentAssetEdit}
                onOpenSurfaceAdornment={() => setSurfaceAdornmentOpen(true)}
                surfaceAdornmentDisabled={Boolean(surfaceAdornmentDisabledReason)}
                surfaceAdornmentDetail={surfaceAdornmentDisabledReason ?? undefined}
              />
              </details>
            )}
            {materialOptionsOpen && agentBlockoutShapeProgram && materialPresets.length > 0 && !isExternalGlbReference && (
              <div className="agent-material-preview" aria-label="视觉材质目录">
                <div className="assistant-directions-heading">
                  <span>换一个视觉材质</span>
                  <small>{agentAssetVersion ? '先预览，再确认版本' : '只影响当前预览'}</small>
                </div>
                <div className="agent-material-preview-list">
                  {quickMaterialPresets.map((preset) => (
                    <button
                      key={preset.material_id}
                      type="button"
                      className={appearanceMaterialId === preset.material_id ? 'active' : ''}
                      onClick={() => {
                        selectMaterialPreselection(preset.material_id)
                        if (agentAssetVersion && selectedAgentPart) {
                          const operation = createQuickMaterialPreviewOperation({
                            operationId: `op_material_${Date.now().toString(36)}`,
                            partId: selectedAgentPart.part_id,
                            materialId: preset.material_id,
                            materialZoneId: selectedMaterialZoneId,
                          })
                          if (operation) {
                            void previewAgentAssetEdit(operation, `将${selectedMaterialZoneId}换成${preset.display_name}`)
                          } else {
                            setAssistantNote('当前部件没有可写入的稳定材质区；未创建 ChangeSet。')
                          }
                        } else {
                          setAssistantNote(`已将 blockout 预览材质切换为「${preset.display_name}」；保存为可编辑模型后才能确认材质版本。`)
                        }
                      }}
                      disabled={Boolean(agentAssetChangeSet) || Boolean(agentAssetVersion && selectedAgentPart && !selectedMaterialZoneId)}
                    >{preset.display_name}</button>
                  ))}
                </div>
                <details className="agent-material-catalog-details" data-testid="agent-material-catalog">
                  <summary>全部 {materialPresets.length} 项材质、分类与材质区</summary>
                  {materialEditor}
                </details>
              </div>
            )}
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
            />
            <small className="planner-boundary">所有生成和调整都只影响虚构、非功能展示组件；预览确认前不会写入版本。</small>
          </section>
          </div>
          <WorkbenchComposer
            value={chatInput}
            disabled={!concept.project || concept.loading}
            // A formal V003 turn owns one unconfirmed result at a time. Keep
            // the composer in its existing sending state until that sealed
            // decision arrives so a double click cannot start a second Turn
            // while the same single-renderer preview is still compiling.
            sending={concept.loading
              || agentBlockoutDisplay.directionPreviewLoading
              || singleResultDecisionPresentation.presentation.state === 'processing'}
            referenceImportCapability="reference_guided_rebuild"
            onChange={setChatInput}
            onSend={runAssistantAction}
            onOpenStyle={() => {
              setStyleOptionsOpen((current) => !current)
              setMaterialOptionsOpen(false)
            }}
            onOpenMaterial={() => {
              setMaterialOptionsOpen((current) => !current)
              setStyleOptionsOpen(false)
            }}
            onOpenReference={() => {
              if (agentAssetChangeSet) {
                setAssistantNote('请先保留或取消当前预览，再添加参考证据。')
                return
              }
              setReferenceEvidenceOpen(true)
            }}
            onOpenSurfaceAdornment={() => setSurfaceAdornmentOpen(true)}
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
              {TOOL_ITEMS.filter((tool) => tool.id === 'select' || tool.id === 'orbit' || tool.id === 'measure').map((tool) => (
                <IconButton
                  key={tool.id}
                  icon={tool.icon}
                  label={tool.label}
                  active={activeTool === tool.id}
                  disabled={!tool.implemented}
                  title={tool.unavailableReason}
                  onClick={() => setViewportTool(tool.id)}
                />
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
            />
            {activeTool === 'measure' && (
              <div className="measurement-overlay" data-testid="measurement-overlay" role="status" aria-live="polite">
                <strong>测量</strong>
                <div className="measurement-mode-toggle" aria-label="测量模式">
                  <button
                    type="button"
                    className={measurementMode === 'distance' ? 'active' : ''}
                    aria-pressed={measurementMode === 'distance'}
                    onClick={() => setMeasurementMode('distance')}
                  >点到点</button>
                  <button
                    type="button"
                    className={measurementMode === 'normal_angle' ? 'active' : ''}
                    aria-pressed={measurementMode === 'normal_angle'}
                    onClick={() => setMeasurementMode('normal_angle')}
                  >法线夹角</button>
                </div>
                <span>
                  {!measurementStart
                    ? '点击模型设置起点'
                    : !measurementEnd
                      ? '点击模型设置终点'
                      : formatViewportMeasurement(measurementReadout)}
                </span>
                {measurementReadout && <button type="button" onClick={pinMeasurement}>固定标注</button>}
                <button type="button" onClick={clearMeasurements}>清除</button>
                {measurementAnnotations.length > 0 && (
                  <div className="measurement-annotations" data-testid="measurement-annotations">
                    {measurementAnnotations.map((annotation, index) => (
                      <span key={annotation.id}>标注 {index + 1}<em>{formatViewportMeasurement(annotation.readout)}</em></span>
                    ))}
                  </div>
                )}
              </div>
            )}
            {agentAssetChangeSet && (
              <div className="ghost-preview-badge" data-testid="ghost-preview-badge">
                幽灵预览 · 尚未写入版本
              </div>
            )}
            <div className="view-cube"><Cube size={28} weight="duotone" /></div>
            <div className="viewport-viewbar">
              <IconButton icon={House} label="等轴" active={cameraView === 'iso'} onClick={() => void updateRenderPreset({ cameraView: 'iso' })} />
              <IconButton icon={Crosshair} label="正视" active={cameraView === 'front'} onClick={() => void updateRenderPreset({ cameraView: 'front' })} />
              <IconButton icon={GridFour} label="顶视" active={cameraView === 'top'} onClick={() => void updateRenderPreset({ cameraView: 'top' })} />
              <IconButton icon={Cube} label="右视" active={cameraView === 'right'} onClick={() => void updateRenderPreset({ cameraView: 'right' })} />
              <label className="viewport-light-preset">
                <span>灯光</span>
                <select aria-label="灯光预设" value={lightPreset} onChange={(event) => void updateRenderPreset({ lightPreset: event.target.value as LightPreset })}>
                  <option value="cad_neutral">CAD 中性</option>
                  <option value="soft_studio">柔和棚拍</option>
                  <option value="concept_contrast">概念对比</option>
                </select>
              </label>
              <IconButton
                icon={ArrowsOutCardinal}
                label="爆炸视图"
                active={explodeFactor > 0}
                onClick={() => setViewportExplodeFactor(explodeFactor > 0 ? 0 : 0.42)}
              />
            </div>
            <div className="viewport-readout">
              <span>{agentAssetChangeSet ? '正在预览 Agent 修改，尚未保存' : activeAgentAssetVersion ? '当前视口绑定 Agent Snapshot' : concept.legacyDetailsEnabled ? '旧版 Graph 只读查看' : '等待 Agent 预览'}</span>
              <span>{measurementReadout ? `测量：${formatViewportMeasurement(measurementReadout)}` : '单位：mm'}</span>
            </div>
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

        <WorkbenchInspectorRail
          mode={activeDesignSnapshot?.active_design.source === 'agent_asset'
            ? 'agent'
            : legacyDesignReadOnly
              ? 'legacy'
              : 'empty'}
          agentAssetVersion={activeAgentAssetVersion}
          agentQualityReport={agentQualityReport}
          selectedAgentPartId={displayedAgentSelectedPartId}
          materialEditor={null}
          legacyDetailsOpen={concept.legacyDetailsEnabled}
          legacyVersion={concept.version}
          legacyGraph={concept.graphRecord}
          legacyQualityRun={concept.qualityRun}
          selectedLegacyNode={selectedNode}
          onCloseLegacyDetails={concept.closeLegacyDetails}
          onSelectLegacyNode={selectGraphNode}
        />
      </div>

      <footer className="cad-status-bar" role="status" aria-live="polite" aria-label="工作台状态">
        <span>{concept.loading ? 'Agent 正在处理' : '设计就绪'}</span>
        <span>{activeAgentAssetVersion ? 'Agent 资产可编辑' : concept.legacyDetailsEnabled ? '旧版信息只读' : '等待 Agent 资产'}</span>
        <span>版本：{activeDesignSnapshot?.active_design.source === 'agent_asset'
          ? `Agent v${activeAgentAssetVersion?.version_no ?? '同步中'}`
          : activeDesignSnapshot
          ? '旧版只读设计'
          : activeVersionSummary
          ? `v${activeVersionSummary.version_no}`
          : '草稿'}</span>
        <span>单位：mm</span>
        <span className="status-spacer" />
        <span>{activeDesignSnapshot?.active_design.source === 'agent_asset'
          ? (agentQualityReport?.status === 'passed' ? '通过' : agentQualityReport?.status === 'warning' ? '需复核' : agentQualityReport?.status === 'failed' ? '未通过' : '未检查')
          : qualityStatusLabel(concept.qualityRun?.report.status)} · 模型检查</span>
      </footer>
    </div>
  )
}

function IconButton({
  icon: Icon,
  label,
  active = false,
  onClick,
  disabled = false,
  title,
}: {
  icon: typeof CursorClick
  label: string
  active?: boolean
  onClick?: () => void
  disabled?: boolean
  title?: string
}) {
  return (
    <button
      className={active ? 'active' : ''}
      onClick={onClick}
      disabled={disabled}
      title={title ?? label}
      aria-label={label}
    >
      <Icon size={17} />
    </button>
  )
}

function qualityStatusLabel(status?: 'passed' | 'warning' | 'failed' | 'not_run') {
  if (!status || status === 'not_run') return '未运行'
  return ({ passed: '通过', warning: '需复核', failed: '失败' } as const)[status]
}

function downloadBase64File(encoded: string, filename: string, mime: string): void {
  const bytes = Uint8Array.from(window.atob(encoded), (character) => character.charCodeAt(0))
  downloadBlobFile(new Blob([bytes], { type: mime }), filename)
}

function downloadBlobFile(blob: Blob, filename: string): void {
  const objectUrl = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = objectUrl
  anchor.download = filename
  anchor.style.display = 'none'
  document.body.appendChild(anchor)
  anchor.click()
  anchor.remove()
  window.setTimeout(() => URL.revokeObjectURL(objectUrl), 0)
}

function arrayBufferToBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer)
  const chunkSize = 0x8000
  let binary = ''
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, Math.min(offset + chunkSize, bytes.length)))
  }
  return window.btoa(binary)
}

function inferImportDomainPack(fileName: string): 'pack_future_weapon_prop' | 'pack_vehicle_concept' | 'pack_aircraft_concept' | 'pack_robotic_arm_concept' {
  const value = fileName.toLowerCase()
  if (/(car|vehicle|truck|auto|汽车|车辆|载具)/.test(value)) return 'pack_vehicle_concept'
  if (/(plane|aircraft|drone|jet|飞机|飞行|无人机)/.test(value)) return 'pack_aircraft_concept'
  if (/(arm|robot|joint|机械臂|机器人|关节)/.test(value)) return 'pack_robotic_arm_concept'
  return 'pack_future_weapon_prop'
}

function errorText(caught: unknown): string {
  return caught instanceof Error ? caught.message : String(caught)
}
