import { useCallback, useEffect, useMemo, useRef, useState, type ChangeEvent, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent } from 'react'
import {
  ArrowsClockwise,
  ArrowsLeftRight,
  ArrowsOutCardinal,
  CaretDown,
  CaretUp,
  ChartLineUp,
  Check,
  ClockCounterClockwise,
  Crosshair,
  Cube,
  CursorClick,
  Export,
  Eye,
  FileArrowDown,
  FloppyDisk,
  FolderOpen,
  Funnel,
  GridFour,
  House,
  MagnifyingGlass,
  Plus,
  Ruler,
  SelectionAll,
  Star,
  ShareNetwork,
  SlidersHorizontal,
  Sparkle,
  WarningCircle,
} from '@phosphor-icons/react'
import { ForgeApiError, forgeApi, mapActiveDesignError } from '../../shared/api/forgeApi'
import type { ActiveDesignNavigation, AgentAssetChangeSet, AgentAssetQualityReport, AgentAssetRenderView, AgentAssetVersion, AgentComponentCandidate, AgentMaterialPreset, AgentPartEditOperation, AgentStructureSuggestion, DesignChangeSet, ModuleAssetRecord, QualityFinding, Transform } from '../../shared/types'
import { useRuntime } from '../../app/providers/RuntimeProvider'
import {
  getProviderConfig as getTauriProviderConfig,
  restartAgentSupervisor,
  saveProviderConfig as saveTauriProviderConfig,
  type ProviderConfigMetadata,
} from '../../shared/tauri/agentSupervisor'
import { ModuleGraphViewport, type ViewportMeasurementPoint } from './ModuleGraphViewport'
import { AgentConversation } from './AgentConversation'
import { AgentSelectionCard } from './AgentSelectionCard'
import { selectAgentBlockoutPreviewPresentation } from './agentBlockoutPreviewPresentation'
import { selectAgentPlanSourcePresentation } from './agentPlanSourcePresentation'
import {
  ComponentDrawer,
  COMPONENT_CATEGORIES,
  MODULE_CATEGORY_LABELS,
  ORIGIN_CLAIM_LABELS,
  QUALITY_STATUS_LABELS,
  REVIEW_STATUS_LABELS,
  type ModuleCategory,
  type QualityStatus,
} from './ComponentDrawer'
import { ExportDrawer, type ExportFormat, type ExportPurpose, type ExportPurposeOption } from './ExportDrawer'
import { MaterialDrawer } from './MaterialDrawer'
import { QualityDrawer } from './QualityDrawer'
import { WorkbenchDrawerStack } from './WorkbenchDrawerStack'
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
import { useAgentDirectionConceptPreviews } from './useAgentDirectionConceptPreviews'
import { useAgentAssetWorkspace } from './useAgentAssetWorkspace'
import { getLegacyCompatibilityDisplay } from './legacyCompatibilityDisplay'
import { useComponentLibraryPreferences } from './useComponentLibraryPreferences'
import { filterComponentLibraryRecords } from './componentLibraryPreferencesState'
import { useViewportDisplayPreferences } from './useViewportDisplayPreferences'
import { useLegacyModuleGraphWorkspace } from './useLegacyModuleGraphWorkspace'
import { useLegacyModuleGraphOverlay } from './useLegacyModuleGraphOverlay'
import { useAgentRenderPresentation } from './useAgentRenderPresentation'
import { useAgentEditAssistPresentation } from './useAgentEditAssistPresentation'
import { useAgentMaterialCatalogPresentation } from './useAgentMaterialCatalogPresentation'
import { useAgentMaterialFilterPresentation } from './useAgentMaterialFilterPresentation'
import { useAgentMaterialPreselectionPresentation } from './useAgentMaterialPreselectionPresentation'
import { useComponentCatalogPresentation } from './useComponentCatalogPresentation'
import { useConceptWorkbench } from './useConceptWorkbench'
import './cad-workbench.css'

type InspectorTab = 'parameters' | 'appearance' | 'connections' | 'inspection'
type Tool = 'select' | 'move' | 'rotate' | 'scale' | 'orbit' | 'measure' | 'section'
type CameraView = 'iso' | 'front' | 'top' | 'right'
type LightPreset = 'cad_neutral' | 'soft_studio' | 'concept_contrast'
type AgentTurnRecordResult = { recorded: boolean; clarification: boolean; cancelled: boolean }

type WeaponParameters = {
  overallLength: number
  bodyHeight: number
  frontShellLength: number
  gripAngle: number
  shellThickness: number
  detailDensity: number
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

type MeasurementAnnotation = {
  annotationId: string
  kind: 'distance' | 'normal_angle'
  points: [ViewportMeasurementPoint, ViewportMeasurementPoint]
  distanceMm: number
  angleDeg: number
}

const DEFAULT_CONCEPT_BRIEF = '一台结构清晰、比例协调、适合继续编辑的未来机械概念展示模型'
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

const EXPORT_PURPOSES: ExportPurposeOption[] = [
  { id: 'presentation', title: '展示设计', description: '用于方案评审或展示画面', format: 'PNG' },
  { id: 'production', title: '游戏 / 影视项目', description: '保留展示模型与材质', format: 'GLB' },
  { id: 'handoff', title: '交给三维设计师', description: '继续在三维软件中处理', format: 'OBJ' },
  { id: 'archive', title: '保存完整设计资料', description: '包含当前版本与概念资料', format: 'SOURCE ZIP' },
]

function clampNumber(value: number, min: number, max: number) {
  return Math.max(min, Math.min(max, value))
}

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
  const {
    activeDesignState,
    openProject,
    startActiveDesignRequest,
    isCurrentActiveDesignRequest,
    receiveActiveDesignSnapshot,
    failActiveDesignRequest,
    drawerFocusRef,
    componentDrawerOpen,
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
    agentDirectionConceptPreviewState,
    openDirectionConceptPreviewProject,
    startDirectionConceptPreviews,
    receiveDirectionConceptPreview,
    failDirectionConceptPreview,
    clearDirectionConceptPreviews,
  } = useAgentDirectionConceptPreviews()
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
    componentLibraryPreferences,
    openComponentLibraryPreferences,
    setComponentCategory,
    setComponentQuery,
    setReviewStatusFilter,
    toggleLibraryFavorite,
    recordRecentLibraryModule,
    setDrawerExpanded,
    setDrawerHeight,
    setComponentDrawerMode,
    toggleComponentDrawerMode,
  } = useComponentLibraryPreferences()
  const {
    viewportDisplayPreferences,
    openViewportDisplayPreferences,
    setViewportTool,
    setViewportShowConnectors,
    setViewportExplodeFactor,
    setViewportSectionOffset,
  } = useViewportDisplayPreferences()
  const {
    legacyModuleGraphWorkspace,
    legacyModuleGraphWorkspacePreferenceKey,
    openLegacyModuleGraphWorkspace,
    setLegacyInspectorTab,
    setLegacyTransformSpace,
    setLegacySnapEnabled,
    selectLegacyModuleGraphNode,
    setLegacySelectedModule,
    clearLegacyModuleGraphSelection,
    setLegacyMeasurementMode,
    reconcileLegacyModuleGraphSelection,
  } = useLegacyModuleGraphWorkspace()
  const {
    legacyModuleGraphOverlay,
    legacyModuleGraphOverlayContextKey,
    thumbnailFailures,
    openLegacyModuleGraphOverlay,
    reconcileLegacyModuleGraphOverlayNodes,
    toggleLegacyHiddenNode,
    setLegacyFocusNode,
    setLegacyQualityOverlay,
    clearLegacyQualityOverlay,
    recordLegacyThumbnailFailure,
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
    shapeProgram: agentBlockoutShapeProgram,
    segmentation: agentBlockoutSegmentation,
  } = agentBlockoutDisplay
  const blockoutPreviewPresentation = selectAgentBlockoutPreviewPresentation(agentBlockoutDisplay)
  const agentPlanSourcePresentation = selectAgentPlanSourcePresentation(agentPlan)
  const [cameraView, setCameraView] = useState<CameraView>('iso')
  const [lightPreset, setLightPreset] = useState<LightPreset>('cad_neutral')
  const [measurementPoints, setMeasurementPoints] = useState<ViewportMeasurementPoint[]>([])
  const [measurementAnnotations, setMeasurementAnnotations] = useState<MeasurementAnnotation[]>([])
  const [showPrecisionAdjustments, setShowPrecisionAdjustments] = useState(false)
  const [presentationProfile, setPresentationProfile] = useState<'quick_sketch' | 'showcase'>('showcase')
  const [exportPurpose, setExportPurpose] = useState<ExportPurpose>('presentation')
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
    showConnectors,
    explodeFactor,
    sectionOffset,
  } = viewportDisplayPreferences
  const {
    inspectorTab,
    transformSpace,
    snapEnabled,
    selectedNodeId: selectedComponent,
    selectedModuleId: selectedLibraryModuleId,
    measurementMode,
  } = legacyModuleGraphWorkspace
  const activeDesignAssetVersionId = activeDesignSnapshot?.active_design.source === 'agent_asset'
    ? activeDesignSnapshot.active_design.asset_version_id
    : null
  const activeAgentAssetVersion = activeDesignAssetVersionId === agentAssetVersion?.asset_version_id
    ? agentAssetVersion
    : null
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
  const activePartDisplay = activeDesignPartDisplay(activeDesignSnapshot)
  const selectedAgentPartLocked = selectedAgentPart
    ? activeDesignPartIsLocked(activeDesignSnapshot, selectedAgentPart.part_id)
    : false
  const [appearanceMaterialZoneId, setAppearanceMaterialZoneId] = useState('')
  const [exportFormat, setExportFormat] = useState('SOURCE ZIP')
  const [parameters, setParameters] = useState<WeaponParameters>({
    overallLength: 230,
    bodyHeight: 54,
    frontShellLength: 120,
    gripAngle: 15,
    shellThickness: 2.5,
    detailDensity: 68,
  })

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
  const appearanceMaterialId = agentMaterialPreselectionPresentation.materialId
  const [transformDraft, setTransformDraft] = useState<Transform>(() => identityTransform())
  const [providerConfig, setProviderConfig] = useState<ProviderConfigMetadata | null>(null)
  const [providerSetupOpen, setProviderSetupOpen] = useState(false)
  const [providerBaseUrl, setProviderBaseUrl] = useState('https://api.deepseek.com')
  const [providerModel, setProviderModel] = useState('deepseek-v4-pro')
  const [providerApiKey, setProviderApiKey] = useState('')
  const [providerSaving, setProviderSaving] = useState(false)
  const [importingGlb, setImportingGlb] = useState(false)
  const importGlbInputRef = useRef<HTMLInputElement | null>(null)
  const {
    componentCategory,
    componentQuery,
    reviewStatusFilter,
    favoriteModuleIds,
    recentModuleIds,
    drawerExpanded,
    drawerHeight,
    componentDrawerMode,
  } = componentLibraryPreferences
  const isExternalGlbReference = agentAssetVersion?.shape_program?.schema_version === 'ExternalGLBReference@1'

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
        void api.getAgentQualityReport(response.data.quality.quality_report_id)
          .then((report) => {
            receiveAgentAssetWorkspaceQuality(projectId, workspaceRequestId, report)
          })
          .catch(() => {
            receiveAgentAssetWorkspaceQuality(projectId, workspaceRequestId, null)
          })
      } else {
        clearAgentAssetWorkspaceQuality(projectId)
      }
      clearAgentEditAssistPresentation()
      const isImportedReference = version.shape_program?.schema_version === 'ExternalGLBReference@1'
      hydrateBlockoutDisplay(projectId, {
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
      if (isImportedReference) {
        void api.exportAgentAssetGlb(version.asset_version_id).then((exported) => {
          if (isCurrentActiveDesignRequest(requestId)) setBlockoutGlb(projectId, exported.glb_base64)
        }).catch(() => {
          if (isCurrentActiveDesignRequest(requestId)) setAssistantNote('导入参考模型的原始 GLB 不可读取；不会影响其他项目版本。')
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
      const [candidates, structure] = await Promise.all([
        api.listAgentComponentCandidates(assetVersionId, partId),
        api.listAgentStructureSuggestions(assetVersionId),
      ])
      receiveAgentEditAssistRead(projectId, assetVersionId, partId, requestId, candidates, structure)
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
      projectId: concept.project?.project_id ?? null,
      packId: concept.project?.profile.pack_id ?? null,
      source: activeDesignSnapshot?.active_design.source === 'agent_asset' ? 'agent_asset' as const : concept.project ? 'legacy' as const : 'none' as const,
    }
    openComponentCatalog(context)
    if (!context.packId) return
    const requestId = startComponentCatalogRead(context)
    if (requestId === null) return
    void api.listModuleAssets(context.packId).then((response) => {
      receiveComponentCatalog(context, requestId, response.items ?? [])
    }).catch(() => { failComponentCatalog(context, requestId) })
  }, [activeDesignSnapshot?.active_design.source, api, concept.project, failComponentCatalog, openComponentCatalog, receiveComponentCatalog, startComponentCatalogRead])

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
    openAgentMaterialPreselectionPresentation({
      projectId: concept.project?.project_id ?? null,
      assetVersionId: activeAgentAssetVersion?.asset_version_id ?? null,
      selectedPartId: selectedAgentPart?.part_id ?? null,
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
    activeAgentAssetVersion?.asset_version_id,
    agentPlan?.domain_pack_id,
    concept.project?.project_id,
    isExternalGlbReference,
    legacyDesignReadOnly,
    openAgentMaterialPreselectionPresentation,
    selectedAgentPart?.part_id,
  ])

  useEffect(() => {
    void getTauriProviderConfig().then((config) => {
      if (!config) return
      setProviderConfig(config)
      setProviderBaseUrl(config.base_url)
      setProviderModel(config.model)
    })
  }, [])

  useEffect(() => {
    const spec = concept.version?.spec
    if (!spec) return
    setParameters((current) => ({
      ...current,
      overallLength: spec.proportions.overall_length_mm,
      bodyHeight: spec.proportions.body_height_mm,
      gripAngle: spec.proportions.grip_angle_deg,
      detailDensity: Math.round(spec.style.detail_density * 100),
    }))
  }, [concept.version])

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
  const selectedLibraryModule = catalogModules.find(
    (module) => module.manifest.module_id === selectedLibraryModuleId,
  ) ?? null
  const selectedModuleLabel = selectedModule
    ? MODULE_CATEGORY_LABELS[selectedModule.manifest.category]
    : '当前部件'
  const selectedNodeConnections = (concept.graphRecord?.graph.edges ?? []).filter((edge) => (
    edge.from_node_id === selectedNode?.node_id || edge.to_node_id === selectedNode?.node_id
  ))
  const measurementDistance = measurementPoints.length === 2
    ? distanceBetween(measurementPoints[0].position, measurementPoints[1].position)
    : null
  const measurementAngle = measurementPoints.length === 2
    ? angleBetweenNormals(measurementPoints[0].normal, measurementPoints[1].normal)
    : null
  const measurementStorageKey = concept.project && concept.version
    ? `forgecad.measurements.v1.${concept.project.project_id}.${concept.version.version_id}`
    : null

  useEffect(() => {
    if (!measurementStorageKey) {
      setMeasurementAnnotations([])
      return
    }
    try {
      const stored = window.localStorage.getItem(measurementStorageKey)
      const parsed = stored ? JSON.parse(stored) : []
      setMeasurementAnnotations(Array.isArray(parsed) ? parsed : [])
    } catch {
      setMeasurementAnnotations([])
    }
  }, [measurementStorageKey])
  const canSnapSelectedNode = Boolean(
    selectedNode
      && selectedNode.node_id !== concept.graphRecord?.graph.root_node_id
      && !selectedNode.locked
      && selectedNodeConnections.length > 0,
  )

  useEffect(() => {
    if (!selectedNode) {
      setTransformDraft(identityTransform())
      return
    }
    setTransformDraft(copyTransform(selectedNode.transform))
  }, [selectedNode?.node_id, selectedNode?.transform])

  const qualityByModuleId = useMemo(() => {
    const result = new Map<string, QualityStatus>()
    const nodes = concept.graphRecord?.graph.nodes ?? []
    const report = concept.qualityRun?.report
    if (!report || report.status === 'not_run') return result
    for (const node of nodes) result.set(node.module_id, 'passed')
    for (const finding of report.findings ?? []) {
      const status: QualityStatus = finding.severity === 'error' ? 'failed' : 'warning'
      for (const nodeId of finding.node_ids ?? []) {
        const node = nodes.find((candidate) => candidate.node_id === nodeId)
        if (!node) continue
        const current = result.get(node.module_id)
        if (status === 'failed' || current !== 'failed') result.set(node.module_id, status)
      }
    }
    return result
  }, [concept.graphRecord, concept.qualityRun])

  const qualityStatusFor = useCallback(
    (moduleId: string): QualityStatus => qualityByModuleId.get(moduleId) ?? 'unavailable',
    [qualityByModuleId],
  )

  const visibleComponents = useMemo(() => {
    const installedModuleIds = new Set(
      (concept.graphRecord?.graph.nodes ?? []).map((node) => node.module_id),
    )
    return filterComponentLibraryRecords({
      modules: catalogModules,
      installedModuleIds,
      selectedModuleCategory: selectedModule?.manifest.category ?? null,
      selectedNodeUnlocked: Boolean(selectedNode && !selectedNode.locked),
      preferences: componentLibraryPreferences,
    })
  }, [
    componentLibraryPreferences,
    concept.graphRecord,
    catalogModules,
    selectedModule,
    selectedNode,
  ])

  const componentFilterCounts = useMemo(() => {
    const installedModuleIds = new Set(
      (concept.graphRecord?.graph.nodes ?? []).map((node) => node.module_id),
    )
    const compatibleCount = selectedNode && !selectedNode.locked && selectedModule
      ? catalogModules.filter((component) => component.manifest.category === selectedModule.manifest.category).length
      : 0
    return {
      all: catalogModules.length,
      installed: installedModuleIds.size,
      compatible: compatibleCount,
      favorites: catalogModules.filter((component) => favoriteModuleIds.includes(component.manifest.module_id)).length,
      recent: catalogModules.filter((component) => recentModuleIds.includes(component.manifest.module_id)).length,
    }
  }, [catalogModules, concept.graphRecord, favoriteModuleIds, recentModuleIds, selectedModule, selectedNode])

  const displayedComponents = useMemo(
    () => componentDrawerMode === 'recommended' ? visibleComponents.slice(0, 3) : visibleComponents,
    [componentDrawerMode, visibleComponents],
  )

  const getModuleFileUrl = useCallback(
    (moduleId: string) => forgeApi.getModuleAssetFileUrl(moduleId),
    [],
  )
  const canReplaceSelected = Boolean(
    selectedNode
    && selectedLibraryModule
    && !selectedNode.locked
    && selectedNode.module_id !== selectedLibraryModule.manifest.module_id
    && selectedModule?.manifest.category === selectedLibraryModule.manifest.category
    && selectedLibraryModule.catalog_metadata.review_status !== 'restricted'
    && qualityStatusFor(selectedLibraryModule.manifest.module_id) !== 'failed'
  )
  const activeVersionSummary = (concept.project?.versions ?? []).find(
    (item) => item.version_id === concept.version?.version_id,
  )
  const undoVersionId = activeVersionSummary?.parent_version_id ?? null
  const redoVersionId = (concept.project?.versions ?? [])
    .filter((item) => item.parent_version_id === concept.version?.version_id)
    .sort((left, right) => right.version_no - left.version_no)[0]?.version_id ?? null

  const selectGraphNode = useCallback((nodeId: string) => {
    const node = concept.graphRecord?.graph.nodes.find((item) => item.node_id === nodeId)
    selectLegacyModuleGraphNode(nodeId, node?.module_id ?? '')
  }, [concept.graphRecord, selectLegacyModuleGraphNode])

  const selectLibraryModule = useCallback((module: ModuleAssetRecord) => {
    const moduleId = module.manifest.module_id
    setLegacySelectedModule(moduleId)
    recordRecentLibraryModule(moduleId)
    const graphNode = concept.graphRecord?.graph.nodes.find((node) => node.module_id === moduleId)
    if (graphNode && !componentDrawerOpen) selectLegacyModuleGraphNode(graphNode.node_id, graphNode.module_id)
  }, [componentDrawerOpen, concept.graphRecord, recordRecentLibraryModule, selectLegacyModuleGraphNode, setLegacySelectedModule])

  const beginDrawerResize = useCallback((event: ReactPointerEvent<HTMLDivElement>) => {
    if (!drawerExpanded) return
    const startY = event.clientY
    const startHeight = drawerHeight
    const onMove = (moveEvent: PointerEvent) => {
      setDrawerHeight(Math.max(280, Math.min(520, startHeight + startY - moveEvent.clientY)))
    }
    const onUp = () => {
      window.removeEventListener('pointermove', onMove)
      window.removeEventListener('pointerup', onUp)
    }
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
  }, [drawerExpanded, drawerHeight])

  const resizeDrawerByKeyboard = useCallback((event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (!drawerExpanded && event.key !== 'Enter' && event.key !== ' ') return
    if (event.key === 'ArrowUp' || event.key === 'ArrowDown' || event.key === 'Home' || event.key === 'End' || event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
    }
    if (event.key === 'Enter' || event.key === ' ') {
      setDrawerExpanded(true)
      return
    }
    if (event.key === 'ArrowUp') setDrawerHeight(drawerHeight + 24)
    if (event.key === 'ArrowDown') setDrawerHeight(drawerHeight - 24)
    if (event.key === 'Home') setDrawerHeight(280)
    if (event.key === 'End') setDrawerHeight(520)
  }, [drawerExpanded, drawerHeight, setDrawerExpanded, setDrawerHeight])

  const closeAllDrawers = useCallback(() => {
    closeAgentRenderPresentation()
    closeDrawers()
  }, [closeAgentRenderPresentation, closeDrawers])
  const openExportDrawer = useCallback(() => openDrawer('export'), [openDrawer])
  const openQualityDrawer = useCallback(() => openDrawer('quality'), [openDrawer])

  const focusQualityFinding = useCallback((finding: QualityFinding) => {
    const validNodeIds = (finding.node_ids ?? []).filter((candidate) => (
      concept.graphRecord?.graph.nodes.some((node) => node.node_id === candidate)
    ))
    const nodeId = validNodeIds[0]
    if (!nodeId) return
    selectGraphNode(nodeId)
    setLegacyFocusNode(nodeId)
    setLegacyQualityOverlay(
      validNodeIds,
      (finding.geometry_refs ?? []).filter((reference) => validNodeIds.includes(reference.node_id)),
    )
  }, [concept.graphRecord, selectGraphNode, setLegacyFocusNode, setLegacyQualityOverlay])

  useEffect(() => {
    clearLegacyQualityOverlay()
  }, [clearLegacyQualityOverlay, concept.qualityRun?.quality_run_id, concept.version?.version_id])

  const handleCreateExport = useCallback(async () => {
    if (activeDesignSnapshot?.active_design.source === 'agent_asset') {
      setAssistantNote('当前 Agent 设计请使用“下载 3D 模型 (GLB)”，不会回退到旧 Concept 版本。')
      return
    }
    const currentExport = concept.lastExport?.version_id === concept.version?.version_id
      ? concept.lastExport
      : null
    const reusable = currentExport && (
      exportFormat === 'SOURCE ZIP'
      || exportFormat === 'GLB'
      || (exportFormat === 'OBJ' && currentExport.combined_obj_sha256)
      || (exportFormat === 'PNG' && currentExport.preview_png_sha256)
      || (exportFormat === 'MP4' && currentExport.turntable_video_sha256)
    )
    const result = reusable ? currentExport : await concept.createExport()
    if (!result) return
    const url = exportFormat === 'GLB'
      ? forgeApi.getConceptCombinedGlbUrl(result.export_id)
      : exportFormat === 'OBJ'
      ? forgeApi.getConceptCombinedObjUrl(result.export_id)
      : exportFormat === 'PNG'
      ? forgeApi.getConceptPreviewPngUrl(result.export_id)
      : exportFormat === 'MP4'
      ? forgeApi.getConceptTurntableVideoUrl(result.export_id)
      : forgeApi.getConceptExportFileUrl(result.export_id)
    try {
      await downloadBrowserFile(url, exportDownloadFilename(result.export_id, exportFormat))
    } catch (caught) {
      setAssistantNote(`导出已生成，但浏览器下载失败：${errorText(caught)}`)
    }
  }, [activeDesignSnapshot, concept, exportFormat])

  const handleDownloadAgentGlb = useCallback(async () => {
    if (!activeAgentAssetVersion) {
      setAssistantNote('正在同步当前设计版本，请稍后再下载。')
      return
    }
    try {
      const result = await api.exportAgentAssetGlb(activeAgentAssetVersion.asset_version_id)
      downloadBase64File(result.glb_base64, `${activeAgentAssetVersion.asset_version_id}.glb`, 'model/gltf-binary')
      setAssistantNote(`已下载当前 Agent 设计 v${activeAgentAssetVersion.version_no} 的 GLB；下载前已完成 ${result.readback_triangle_count.toLocaleString()} 三角形回读。`)
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

  const downloadExistingExport = useCallback((url: string, filename: string) => {
    downloadBrowserFile(url, filename).catch((caught) => {
      setAssistantNote(`浏览器下载失败：${errorText(caught)}`)
    })
  }, [])

  const handleReplaceSelected = useCallback(() => {
    if (activeDesignSnapshot?.active_design.source === 'agent_asset') {
      setAssistantNote('当前是 Agent 资产；请在“分件候选”中选择部件，旧版组件替换入口不会修改 Agent 版本。')
      return
    }
    if (legacyDesignReadOnly) {
      setAssistantNote('旧版设计为只读状态。请先让 Agent 重建为可编辑资产。')
      return
    }
    if (!selectedNode || !selectedLibraryModule) return
    concept.previewModuleReplacement(selectedNode.node_id, selectedLibraryModule.manifest.module_id)
      .catch(() => undefined)
  }, [activeDesignSnapshot, concept, legacyDesignReadOnly, selectedLibraryModule, selectedNode])

  const openComponentReplacement = useCallback(() => {
    if (!selectedNode || selectedNode.locked) return
    setComponentCategory('compatible')
    setComponentQuery('')
    setReviewStatusFilter('')
    setComponentDrawerMode('recommended')
    setDrawerExpanded(false)
    openDrawer('component')
  }, [openDrawer, selectedNode, setComponentCategory, setComponentDrawerMode, setComponentQuery, setDrawerExpanded, setReviewStatusFilter])

  const handleToggleMirrorX = useCallback(() => {
    if (legacyDesignReadOnly) {
      setAssistantNote('旧版设计为只读状态。请先让 Agent 重建为可编辑资产。')
      return
    }
    if (!selectedNode) return
    const nextAxis = selectedNode.mirror_axis === 'x' ? 'none' : 'x'
    concept.setMirror(selectedNode.node_id, nextAxis).catch(() => undefined)
  }, [concept, legacyDesignReadOnly, selectedNode])

  const updateTransformDraft = useCallback((
    field: keyof Transform,
    axis: 0 | 1 | 2,
    value: number,
  ) => {
    if (!Number.isFinite(value)) return
    setTransformDraft((current) => {
      const next = copyTransform(current)
      next[field][axis] = value
      return next
    })
  }, [])

  const previewTransformDraft = useCallback(() => {
    if (legacyDesignReadOnly) {
      setAssistantNote('旧版设计为只读状态。请先让 Agent 重建为可编辑资产。')
      return
    }
    if (!selectedNode) return
    concept.previewNodeTransform(selectedNode.node_id, transformDraft).catch(() => undefined)
  }, [concept, legacyDesignReadOnly, selectedNode, transformDraft])

  const previewQuickTransform = useCallback((action: 'smaller' | 'larger' | 'forward' | 'backward' | 'rotateLeft' | 'rotateRight') => {
    if (legacyDesignReadOnly) {
      setAssistantNote('旧版设计为只读状态。请先让 Agent 重建为可编辑资产。')
      return
    }
    if (!selectedNode || selectedNode.locked || concept.loading || concept.pendingPreview) return
    const next = copyTransform(selectedNode.transform)
    if (action === 'smaller' || action === 'larger') {
      const delta = action === 'smaller' ? -0.04 : 0.04
      next.scale = next.scale.map((value) => clampNumber(value + delta, 0.9, 1.1)) as Transform['scale']
    }
    if (action === 'forward' || action === 'backward') {
      next.position[0] += action === 'forward' ? -4 : 4
    }
    if (action === 'rotateLeft' || action === 'rotateRight') {
      next.rotation[2] += action === 'rotateLeft' ? 0.1 : -0.1
    }
    setTransformDraft(next)
    concept.previewNodeTransform(selectedNode.node_id, next).catch(() => undefined)
  }, [concept, legacyDesignReadOnly, selectedNode])

  const handleTransformCommit = useCallback((nodeId: string, transform: Transform) => {
    if (legacyDesignReadOnly) {
      setAssistantNote('旧版设计为只读状态。请先让 Agent 重建为可编辑资产。')
      return
    }
    setTransformDraft(copyTransform(transform))
    concept.previewNodeTransform(nodeId, transform).catch(() => undefined)
  }, [concept.previewNodeTransform, legacyDesignReadOnly])

  const handleMeasurePoint = useCallback((point: ViewportMeasurementPoint) => {
    selectGraphNode(point.nodeId)
    setMeasurementPoints((current) => current.length >= 2 ? [point] : [...current, point])
  }, [selectGraphNode])

  const saveMeasurementAnnotations = useCallback((next: MeasurementAnnotation[]) => {
    setMeasurementAnnotations(next)
    if (measurementStorageKey) {
      window.localStorage.setItem(measurementStorageKey, JSON.stringify(next))
    }
  }, [measurementStorageKey])

  const pinMeasurement = useCallback(() => {
    if (measurementPoints.length !== 2 || measurementDistance == null || measurementAngle == null) return
    const annotation: MeasurementAnnotation = {
      annotationId: `measure_${Date.now().toString(36)}`,
      kind: measurementMode,
      points: [measurementPoints[0], measurementPoints[1]],
      distanceMm: measurementDistance,
      angleDeg: measurementAngle,
    }
    saveMeasurementAnnotations([...measurementAnnotations, annotation])
    setMeasurementPoints([])
  }, [measurementAngle, measurementAnnotations, measurementDistance, measurementMode, measurementPoints, saveMeasurementAnnotations])

  const removeMeasurementAnnotation = useCallback((annotationId: string) => {
    saveMeasurementAnnotations(measurementAnnotations.filter((item) => item.annotationId !== annotationId))
  }, [measurementAnnotations, saveMeasurementAnnotations])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const element = event.target as HTMLElement | null
      if (element?.matches('input, textarea, select, [contenteditable="true"]')) return
      if (event.key === 'g' || event.key === 'G') {
        event.preventDefault()
        setViewportTool('move')
      } else if (event.key === 'r' || event.key === 'R') {
        event.preventDefault()
        setViewportTool('rotate')
      } else if (event.key === 's' || event.key === 'S') {
        event.preventDefault()
        setViewportTool('scale')
      } else if (event.key === 'Escape' && concept.pendingManualChange) {
        event.preventDefault()
        concept.discardManualChange().catch(() => undefined)
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [concept.discardManualChange, concept.pendingManualChange])

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

  const toggleSelectedNodeVisibility = useCallback(() => {
    if (!selectedNode) return
    toggleLegacyHiddenNode(selectedNode.node_id)
  }, [selectedNode, toggleLegacyHiddenNode])

  const handleModuleDrop = useCallback((nodeId: string, moduleId: string) => {
    selectGraphNode(nodeId)
    setLegacySelectedModule(moduleId)
    setAssistantNote(`已将 ${moduleId} 设为 ${nodeId} 的替换候选；点击“替换并创建新版本”后才会提交 ChangeSet。`)
  }, [selectGraphNode, setLegacySelectedModule])

  const updateParameter = (key: keyof WeaponParameters, value: number) => {
    setParameters((current) => ({ ...current, [key]: value }))
  }

  const previewParameterDraft = async () => {
    const instruction = [
      `整体长度调整为 ${parameters.overallLength} mm`,
      `握持角度调整为 ${parameters.gripAngle} 度`,
      `细节密度调整为 ${parameters.detailDensity}%`,
    ].join('，')
    setAssistantMode('change')
    setAssistantNote(`正在将参数草稿转换为 ChangeSet：${instruction}`)
    const result = await concept.planChange(instruction, {
      selectedNodeId: selectedNode?.node_id,
      selectedModuleId: selectedLibraryModule?.manifest.module_id,
    })
    if (!result) return
    const previewSpec = result.preview.preview_spec
    setParameters((current) => ({
      ...current,
      overallLength: previewSpec.proportions.overall_length_mm,
      bodyHeight: previewSpec.proportions.body_height_mm,
      gripAngle: previewSpec.proportions.grip_angle_deg,
      detailDensity: Math.round(previewSpec.style.detail_density * 100),
    }))
    setAssistantNote('参数已生成 ghost preview；确认后会创建不可变新版本。')
  }

  useEffect(() => {
    openConversationProject(concept.project?.project_id ?? null)
    openBlockoutProject(concept.project?.project_id ?? null)
    openDirectionConceptPreviewProject(concept.project?.project_id ?? null)
    openAgentAssetWorkspaceProject(concept.project?.project_id ?? null)
    openComponentLibraryPreferences(concept.project?.project_id ?? null, concept.project?.profile.pack_id ?? null)
    openViewportDisplayPreferences(concept.project?.project_id ?? null)
    setAgentAssetChangeSet(null)
    setAgentCandidateSelectedPartId(null)
  }, [concept.project?.profile.pack_id, concept.project?.project_id, openAgentAssetWorkspaceProject, openBlockoutProject, openComponentLibraryPreferences, openConversationProject, openDirectionConceptPreviewProject, openViewportDisplayPreferences])

  useEffect(() => {
    openLegacyModuleGraphWorkspace(
      legacyCompatibility.isLegacyReadOnly ? concept.project?.project_id ?? null : null,
    )
  }, [concept.project?.project_id, legacyCompatibility.isLegacyReadOnly, openLegacyModuleGraphWorkspace])

  useEffect(() => {
    openLegacyModuleGraphOverlay(
      legacyCompatibility.isLegacyReadOnly ? concept.project?.project_id ?? null : null,
      legacyCompatibility.isLegacyReadOnly ? concept.graphRecord?.graph.graph_id ?? null : null,
      legacyCompatibility.isLegacyReadOnly ? DEFAULT_HIDDEN_NODE_IDS : [],
    )
  }, [
    concept.graphRecord?.graph.graph_id,
    concept.project?.project_id,
    legacyCompatibility.isLegacyReadOnly,
    openLegacyModuleGraphOverlay,
  ])

  const requestAgentDirectionConceptPreviews = useCallback((plan: NonNullable<typeof agentPlan>, projectId: string | null) => {
    const requestId = startDirectionConceptPreviews(projectId, plan)
    void Promise.all(plan.directions.map(async (direction) => {
      try {
        const preview = await api.renderAgentBlockoutConceptPreview({
          client_request_id: `agent-concept-preview-${Date.now()}-${direction.direction_id}`,
          plan,
          direction_id: direction.direction_id,
        })
        receiveDirectionConceptPreview(projectId, plan.plan_id, requestId, preview)
      } catch {
        failDirectionConceptPreview(projectId, plan.plan_id, requestId, direction.direction_id)
      }
    }))
  }, [api, failDirectionConceptPreview, receiveDirectionConceptPreview, startDirectionConceptPreviews])

  const recordAgentTurn = useCallback(async (message: string, clarificationDomainPackId?: string): Promise<AgentTurnRecordResult> => {
    const projectId = concept.project?.project_id ?? null
    const { requestId } = startAgentConversationRequest(projectId)
    clearDirectionConceptPreviews(projectId)
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
          return { recorded: false, clarification: false, cancelled: true }
        }
      }
      const turn = await api.startAgentTurn(threadId, {
        client_request_id: `agent-turn-${Date.now()}`,
        message,
        ...(clarificationDomainPackId ? { clarification_domain_pack_id: clarificationDomainPackId } : {}),
      })
      const presentation = parseAgentTurnPresentation(turn.items, turn.request_text)
      if (!receiveAgentTurn(projectId, requestId, threadId, turn.items, presentation)) {
        return { recorded: false, clarification: false, cancelled: true }
      }
      if (presentation.clarification) {
        clearBlockoutDisplay(projectId)
        clearAgentAssetWorkspace()
        setAgentAssetChangeSet(null)
        setAgentCandidateSelectedPartId(null)
        setAssistantNote(presentation.clarification.question)
        return { recorded: true, clarification: true, cancelled: false }
      }
      if (presentation.plan) requestAgentDirectionConceptPreviews(presentation.plan, projectId)
      return { recorded: true, clarification: false, cancelled: false }
    } catch (caught) {
      if (!isCurrentAgentConversationRequest(projectId, requestId)) {
        return { recorded: false, clarification: false, cancelled: true }
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
          return { recorded: false, clarification: false, cancelled: true }
        }
        setAssistantNote(caught.message)
        return { recorded: false, clarification: true, cancelled: false }
      }
      // The compatibility planner remains usable when the new kernel is not
      // available yet (for example while an older local Agent is running).
      if (!markAgentKernelUnavailable(projectId, requestId)) {
        return { recorded: false, clarification: false, cancelled: true }
      }
      return { recorded: false, clarification: false, cancelled: false }
    }
  }, [agentThreadId, api, clearAgentAssetWorkspace, clearBlockoutDisplay, clearDirectionConceptPreviews, concept.project?.name, concept.project?.project_id, isCurrentAgentConversationRequest, markAgentKernelUnavailable, parseAgentTurnPresentation, receiveAgentClarification, receiveAgentTurn, requestAgentDirectionConceptPreviews, startAgentConversationRequest])

  const previewAgentDirection = useCallback(async (directionId: string, variationIndex = 0, requestedProfile = presentationProfile) => {
    if (!agentPlan) return
    const projectId = concept.project?.project_id ?? null
    clearDirectionConceptPreviews(projectId)
    const requestId = startDirectionPreview(projectId, directionId, variationIndex)
    setAssistantNote('正在生成轻量 3D blockout 预览…')
    try {
      const result = await api.buildAgentBlockout({
        client_request_id: `agent-blockout-${Date.now()}`,
        plan: agentPlan,
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
          plan: agentPlan,
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
  }, [agentPlan, api, clearAgentAssetWorkspace, clearDirectionConceptPreviews, concept.project?.project_id, failDirectionPreview, failSegmentation, isCurrentDirectionPreview, presentationProfile, receiveBlockoutBuild, receiveSegmentation, startDirectionPreview])

  const regenerateAgentBlockoutAppearance = useCallback(() => {
    if (!agentBlockoutSegmentation || !agentPlan || agentBlockoutSegmentation.plan_id !== agentPlan.plan_id) {
      setAssistantNote('请先选择一个完整外观方向，再换一版外观。')
      return
    }
    const nextVariationIndex = ((agentBlockoutSegmentation.variation_index ?? 0) + 1) % 3
    void previewAgentDirection(agentBlockoutSegmentation.direction_id, nextVariationIndex)
  }, [agentBlockoutSegmentation, agentPlan, previewAgentDirection])

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

  const previewAgentAssetEdit = useCallback(async (operation: AgentPartEditOperation, summary: string) => {
    if (!agentAssetVersion) return
    setAssistantNote('正在预览部件修改…')
    try {
      const proposed = await api.proposeAgentAssetChangeSet(agentAssetVersion.asset_version_id, {
        client_request_id: `agent-asset-change-${Date.now()}`,
        summary,
        operations: [operation],
      })
      const preview = await api.previewAgentAssetChangeSet(proposed.change_set_id, `agent-asset-preview-${Date.now()}`)
      setAgentAssetChangeSet(preview)
      if (preview.preview) {
        setBlockoutShapeProgram(concept.project?.project_id ?? null, preview.preview.shape_program)
      }
      setAssistantNote(`已生成“${summary}”预览；确认后才会创建新版本。`)
    } catch {
      setAssistantNote('部件修改预览失败；当前资产版本没有变化。')
    }
  }, [agentAssetVersion, api, concept.project?.project_id, setBlockoutShapeProgram])

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
      setAgentAssetChangeSet(null)
      setBlockoutShapeProgram(concept.project?.project_id ?? null, confirmed.asset_version.shape_program)
      clearAgentAssetWorkspaceQuality(concept.project?.project_id ?? null)
      if (concept.project?.project_id) await refreshActiveDesign(concept.project.project_id)
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
  }, [activeAgentAssetVersion, activeDesignState.snapshotEtag, api])

  const submitAssistantInstruction = async () => {
    return submitAssistantInstructionWithText(chatInput.trim() || DEFAULT_CONCEPT_BRIEF)
  }

  const submitAssistantInstructionWithText = async (requestedText: string, clarificationDomainPackId?: string) => {
    const instruction = requestedText.trim() || DEFAULT_CONCEPT_BRIEF
    setAssistantNote(`正在解释 Brief：“${instruction}”`)
    const kernelResult = await recordAgentTurn(instruction, clarificationDomainPackId)
    if (kernelResult.cancelled) return
    if (kernelResult.clarification) {
      setChatInput('')
      return
    }
    if (legacyDesignReadOnly) {
      if (!kernelResult.recorded) {
        setAssistantNote('请先点击“让 Agent 重建可编辑资产”，并确认本地 Agent 已启动。旧版设计不会被修改。')
      } else {
        setAssistantNote('Agent 已生成新的设计方向。选择一个方向后保存为可编辑资产，即会安全替换活动设计；旧版数据仍保留。')
      }
      setChatInput('')
      return
    }
    const result = await concept.planBrief(instruction)
    if (!result) return
    const provenance = result.brief.planner_provenance
    const interpreted = result.brief.interpreted_spec
    setParameters((current) => ({
      ...current,
      overallLength: interpreted.proportions.overall_length_mm,
      bodyHeight: interpreted.proportions.body_height_mm,
      gripAngle: interpreted.proportions.grip_angle_deg,
      detailDensity: Math.round(interpreted.style.detail_density * 100),
    }))
    setAssistantNote(
      `已解析 ${result.variants.length} 个受限设计方向 · ${provenance.generator}`
      + `${provenance.fallback_used ? ' · Provider 失败后已显式降级' : ''}`
      + `${kernelResult.recorded ? ' · Agent Kernel 已记录步骤' : ''}。`,
    )
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
    if (kernelResult.clarification) {
      setChatInput('')
      return
    }
    const result = await concept.planChange(instruction, {
      selectedNodeId: selectedNode?.node_id,
      selectedModuleId: selectedLibraryModule?.manifest.module_id,
    })
    if (!result) return
    const previewSpec = result.preview.preview_spec
    setParameters((current) => ({
      ...current,
      overallLength: previewSpec.proportions.overall_length_mm,
      bodyHeight: previewSpec.proportions.body_height_mm,
      gripAngle: previewSpec.proportions.grip_angle_deg,
      detailDensity: Math.round(previewSpec.style.detail_density * 100),
    }))
    setAssistantNote(
      `已生成 ${result.planned.change_set.operations.length} 个受限操作；当前仅为幽灵预览。`
      + `${kernelResult.recorded ? ' Agent Kernel 已记录步骤。' : ''}`,
    )
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
      setProviderSetupOpen(false)
      const restarted = await restartAgentSupervisor()
      checkService()
      setAssistantNote(restarted.running ? '模型服务已保存并重新连接；现在可以让 Agent 生成真实设计方向。' : '配置已保存，但本地 Agent 尚未连接，请检查服务状态。')
    } catch (caught) {
      setAssistantNote(`模型服务配置失败：${errorText(caught)}`)
    } finally {
      setProviderSaving(false)
    }
  }, [checkService, providerApiKey, providerBaseUrl, providerModel])

  const testProvider = useCallback(async () => {
    setProviderSaving(true)
    try {
      const result = await api.checkAgentProvider()
      setAssistantNote(result.status === 'ready'
        ? '模型服务连接成功，已返回结构化设计计划。'
        : result.status === 'not_configured'
        ? '当前仍是本机离线规划，没有发起大模型请求。'
        : '模型服务暂时无法连接。已保存设计没有变化；请检查配置后再试。')
    } catch {
      setAssistantNote('模型服务测试未完成。已保存设计没有变化；请稍后重试。')
    } finally {
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

    ?? agentBlockoutSegmentation?.parts.find((part) => part.part_id === displayedAgentSelectedPartId)

  return (
    <div className="cad-workbench" data-testid="cad-workbench">
      <header className="cad-command-bar">
        <div className="cad-brand" aria-label="CAD 工作台">
          <span className="cad-brand-mark"><Cube size={18} weight="fill" /></span>
          <span>ForgeCAD</span>
        </div>
        <div className="cad-workspace-title" aria-label="当前项目">
          <strong>{concept.project?.name ?? '新概念设计'}</strong>
          <span>{concept.loading ? '正在处理…' : '已自动保存'}</span>
        </div>
        <div className="cad-global-actions" aria-label="工作区操作">
          <button
            type="button"
            className="text-action"
            onClick={() => activeAgentAssetVersion ? void navigateAgentAsset('undo') : undoVersionId && concept.selectVersion(undoVersionId)}
            disabled={activeAgentAssetVersion
              ? !agentNavigation?.can_undo || Boolean(agentAssetChangeSet)
              : !undoVersionId || concept.loading || legacyDesignReadOnly}
            title={activeAgentAssetVersion ? '返回上一版 Agent 内容，并保留完整版本历史' : '返回上一个 legacy 已确认版本'}
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
            disabled={!concept.project || importingGlb}
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

      <div className="cad-layout">
        <aside className="cad-left-rail">
          <section className="cad-panel assistant-panel agent-first-panel">
            <div className="cad-panel-title">
              <span><Sparkle size={16} weight="fill" /> 设计助手</span>
              <span className="assistant-state" role="status" aria-live="polite">
                {concept.loading ? '正在工作' : '准备就绪'}
              </span>
            </div>
            <AgentConversation
              loading={concept.loading}
              projectExists={Boolean(concept.project)}
              projectNeedsInitialization={Boolean(concept.project && !concept.version?.module_graph_id)}
              legacyCompatibility={legacyCompatibility}
              onCreateStarterProject={() => void concept.createStarterProject()}
              onInitializeCurrentProject={() => void concept.initializeCurrentProject()}
              onRequestLegacyAgentRebuild={() => void requestLegacyAgentRebuild()}
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
              assistantMode={assistantMode}
              selectedNode={selectedNode?.node_id ?? null}
              selectedModuleLabel={selectedModuleLabel}
              chatInput={chatInput}
              assistantNote={assistantNote}
              errorMessage={concept.error}
              blockoutPreviewPresentation={blockoutPreviewPresentation}
              agentPlanSourcePresentation={agentPlanSourcePresentation}
              directionConceptPreviews={agentDirectionConceptPreviewState.previews}
              conceptFamilySuggestions={CONCEPT_FAMILY_SUGGESTIONS}
              presentationProfile={presentationProfile}
              onAssistantModeChange={setAssistantMode}
              onChatInputChange={setChatInput}
              onRunAssistantAction={runAssistantAction}
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
              onPreviewDirection={(directionId) => void previewAgentDirection(directionId)}
            />
            {agentBlockoutSegmentation && (
              <AgentSelectionCard
                segmentation={agentBlockoutSegmentation}
                agentAssetVersion={agentAssetVersion}
                activeAgentAssetVersion={activeAgentAssetVersion}
                selectedPart={selectedAgentPart}
                selectedPartId={displayedAgentSelectedPartId}
                partDisplay={activePartDisplay}
                isSelectedPartLocked={selectedAgentPartLocked}
                isExternalGlbReference={isExternalGlbReference}
                isSnapshotActionPending={activeDesignState.operation === 'setting_part_display'}
                agentAssetChangeSet={agentAssetChangeSet}
                agentComponentCandidates={agentComponentCandidates}
                agentStructureSuggestions={agentStructureSuggestions}
                structureSuggestionUnavailableMessage={structureSuggestionUnavailableMessage}
                editAssistLoading={agentEditAssistPresentation.loading}
                blockoutPreviewPresentation={blockoutPreviewPresentation}
                onSelectPart={selectAgentPart}
                onCommitBlockout={commitAgentBlockout}
                onRegenerateBlockout={regenerateAgentBlockoutAppearance}
                onPreviewEdit={previewAgentAssetEdit}
                onSaveSelectedComponent={saveSelectedAgentComponent}
                onReplaceComponent={replaceWithAgentComponent}
                onPreviewStructureSuggestion={previewStructureSuggestion}
                onSetPartDisplay={setAgentPartDisplay}
                onInspectAsset={inspectAgentAsset}
                onRejectChange={rejectAgentAssetEdit}
                onConfirmChange={confirmAgentAssetEdit}
              />
            )}
            {agentBlockoutShapeProgram && materialPresets.length > 0 && !isExternalGlbReference && (
              <div className="agent-material-preview" aria-label="视觉材质目录">
                <div className="assistant-directions-heading">
                  <span>换一个视觉材质</span>
                  <small>{agentAssetVersion ? '先预览，再确认版本' : '只影响当前预览'}</small>
                </div>
                <div className="agent-material-preview-list">
                  {materialPresets.slice(0, 5).map((preset) => (
                    <button
                      key={preset.material_id}
                      type="button"
                      className={appearanceMaterialId === preset.material_id ? 'active' : ''}
                      onClick={() => {
                        selectMaterialPreselection(preset.material_id)
                        if (agentAssetVersion && selectedAgentPart) {
                          void previewAgentAssetEdit({
                            operation_id: `op_material_${Date.now().toString(36)}`,
                            op: 'apply_material_preset',
                            part_id: selectedAgentPart.part_id,
                            material_id: preset.material_id,
                          }, `换成${preset.display_name}`)
                        } else {
                          setAssistantNote(`已将 blockout 预览材质切换为「${preset.display_name}」；保存为可编辑模型后才能确认材质版本。`)
                        }
                      }}
                      disabled={Boolean(agentAssetChangeSet)}
                    >{preset.display_name}</button>
                  ))}
                </div>
              </div>
            )}
            {concept.variants.length > 0 && (
              <div className="assistant-directions" aria-label="AI 设计方向">
                <div className="assistant-directions-heading">
                  <span>选择一个方向</span>
                  <small>不会覆盖当前设计</small>
                </div>
                {concept.variants.slice(0, 3).map((variant) => (
                  <button
                    key={variant.variant_id}
                    type="button"
                    className={variant.status === 'selected' ? 'selected' : ''}
                    disabled={concept.loading}
                    onClick={() => {
                      concept.selectVariant(variant.variant_id).then((selected) => {
                        if (selected) {
                          setAssistantNote(`已在主视图预览「${selected.name}」；确认修改后才会写入新版本。`)
                        }
                      })
                    }}
                    title={variant.summary}
                  >
                    <strong>{variant.rank}. {variant.name}</strong>
                    <span>{variant.summary}</span>
                    <small>{variant.status === 'selected' ? '当前预览' : '点击预览'}</small>
                  </button>
                ))}
              </div>
            )}
            <small className="planner-boundary">所有生成和调整都只影响虚构、非功能展示组件；预览确认前不会写入版本。</small>
          </section>
        </aside>

        <main
          className="cad-center-stage"
          style={{ gridTemplateRows: componentDrawerOpen ? `minmax(0, 1fr) ${componentDrawerMode === 'all' ? 360 : 250}px` : 'minmax(0, 1fr)' }}
        >
          <div className="viewport-shell">
            <div className="viewport-toolbar" aria-label="CAD 视口工具">
              {TOOL_ITEMS.filter((tool) => tool.id === 'select' || tool.id === 'orbit').map((tool) => (
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
              graphRecord={concept.graphRecord}
              modules={catalogModules}
              cameraView={cameraView}
              lightPreset={lightPreset}
              showGrid={showGrid}
              wireframe={wireframe}
              xRay={xRay}
              sectionEnabled={activeTool === 'section'}
              sectionOffset={sectionOffset}
              selectedNodeId={selectedComponent}
              hiddenNodeIds={legacyModuleGraphOverlay.hiddenNodeIds}
              focusNodeId={legacyModuleGraphOverlay.focusNodeId}
              qualityHighlightNodeIds={legacyModuleGraphOverlay.qualityHighlightNodeIds}
              qualityGeometryRefs={legacyModuleGraphOverlay.qualityGeometryRefs}
              blockoutGlbBase64={agentBlockoutGlbBase64}
              blockoutShapeProgram={agentBlockoutShapeProgram}
              blockoutMaterialOverride={agentBlockoutShapeProgram ? appearanceMaterialId : null}
              selectedAgentPartId={displayedAgentSelectedPartId}
              hiddenAgentPartIds={activePartDisplay?.hidden_part_ids ?? []}
              isolatedAgentPartId={activePartDisplay?.isolated_part_id ?? null}
              lockedAgentPartIds={activePartDisplay?.locked_part_ids ?? []}
              showConnectors={showConnectors}
              explodeFactor={explodeFactor}
              ghostPreview={Boolean(concept.pendingPreview)}
              transformTool={activeTool === 'move' ? 'translate' : activeTool === 'rotate' ? 'rotate' : activeTool === 'scale' ? 'scale' : 'none'}
              transformSpace={transformSpace}
              snapEnabled={snapEnabled}
              measureEnabled={activeTool === 'measure'}
              getModuleFileUrl={getModuleFileUrl}
              onSelectNode={selectGraphNode}
              onDropModule={handleModuleDrop}
              onTransformCommit={handleTransformCommit}
              onMeasurePoint={handleMeasurePoint}
            />
            {concept.pendingPreview && (
              <div className="ghost-preview-badge" data-testid="ghost-preview-badge">
                幽灵预览 · 尚未写入版本
              </div>
            )}
            {activeTool === 'measure' && (
              <div className="measurement-overlay" data-testid="measurement-overlay">
                <div className="measurement-mode-toggle">
                  <button className={measurementMode === 'distance' ? 'active' : ''} onClick={() => { setLegacyMeasurementMode('distance'); setMeasurementPoints([]) }}>距离</button>
                  <button className={measurementMode === 'normal_angle' ? 'active' : ''} onClick={() => { setLegacyMeasurementMode('normal_angle'); setMeasurementPoints([]) }}>法线夹角</button>
                </div>
                {measurementDistance == null ? (
                  <span>{measurementPoints.length === 0 ? '点击模型设置起点' : '点击模型设置终点'}</span>
                ) : (
                  <><strong>{measurementMode === 'distance' ? `点到点：${measurementDistance.toFixed(2)} mm` : `表面法线夹角：${measurementAngle?.toFixed(2)}°`}</strong><button onClick={pinMeasurement}>固定标注</button></>
                )}
                {measurementPoints.length > 0 && (
                  <button onClick={() => setMeasurementPoints([])}>清除</button>
                )}
                {measurementAnnotations.length > 0 && (
                  <div className="measurement-annotations" data-testid="measurement-annotations">
                    {measurementAnnotations.map((annotation, index) => (
                      <span key={annotation.annotationId}>标注 {index + 1} · {annotation.kind === 'distance' ? `${annotation.distanceMm.toFixed(2)} mm` : `${annotation.angleDeg.toFixed(2)}° 法线夹角`} <button onClick={() => removeMeasurementAnnotation(annotation.annotationId)}>×</button></span>
                    ))}
                  </div>
                )}
              </div>
            )}
            {activeTool === 'section' && (
              <label className="section-overlay" data-testid="section-overlay">
                <span>X 向裁切平面 · {sectionOffset.toFixed(0)} mm</span>
                <input aria-label="截面偏移" type="range" min="-120" max="120" step="1" value={sectionOffset} onChange={(event) => setViewportSectionOffset(Number(event.target.value))} />
              </label>
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
              <span>{concept.pendingPreview ? '正在预览修改，尚未保存' : selectedNode ? '已选中部件，可直接调整' : '点击模型部件即可开始调整'}</span>
              <span>单位：mm</span>
            </div>
            {selectedNode && (
              <section className="contextual-edit-card" data-testid="contextual-edit-card">
                <div className="contextual-edit-heading">
                  <div>
                    <span>已选中部件</span>
                    <strong>{selectedModuleLabel}</strong>
                  </div>
                  <button type="button" onClick={clearLegacyModuleGraphSelection} aria-label="关闭部件编辑">×</button>
                </div>
                <div className="contextual-edit-actions">
                  <button type="button" onClick={openComponentReplacement} disabled={selectedNode.locked || concept.loading}>替换</button>
                  <button type="button" onClick={() => { setAssistantMode('change'); setAssistantNote(`告诉我如何调整这个${selectedModuleLabel}。`) }} disabled={selectedNode.locked}>让 Agent 调整</button>
                  <button type="button" onClick={toggleSelectedNodeVisibility}>{legacyModuleGraphOverlay.hiddenNodeIds.includes(selectedNode.node_id) ? '显示' : '隐藏'}</button>
                </div>
                <div className="contextual-adjustments">
                  <span>快速调整</span>
                  <div><small>大小</small><button onClick={() => previewQuickTransform('smaller')} disabled={selectedNode.locked || concept.loading || Boolean(concept.pendingPreview)}>缩小</button><button onClick={() => previewQuickTransform('larger')} disabled={selectedNode.locked || concept.loading || Boolean(concept.pendingPreview)}>放大</button></div>
                  <div><small>位置</small><button onClick={() => previewQuickTransform('forward')} disabled={selectedNode.locked || concept.loading || Boolean(concept.pendingPreview)}>向前</button><button onClick={() => previewQuickTransform('backward')} disabled={selectedNode.locked || concept.loading || Boolean(concept.pendingPreview)}>向后</button></div>
                  <div><small>方向</small><button onClick={() => previewQuickTransform('rotateLeft')} disabled={selectedNode.locked || concept.loading || Boolean(concept.pendingPreview)}>左转</button><button onClick={() => previewQuickTransform('rotateRight')} disabled={selectedNode.locked || concept.loading || Boolean(concept.pendingPreview)}>右转</button></div>
                </div>
                <button className="precision-toggle" type="button" onClick={() => setShowPrecisionAdjustments((current) => !current)}>
                  {showPrecisionAdjustments ? '收起精确调整' : '精确调整'} <CaretDown size={13} />
                </button>
                {showPrecisionAdjustments && (
                  <div className="precision-adjustments">
                    <div className="axis-group"><span>位置</span><div>
                      <AxisField axis="X" value={transformDraft.position[0]} onChange={(value) => updateTransformDraft('position', 0, value)} />
                      <AxisField axis="Y" value={transformDraft.position[1]} onChange={(value) => updateTransformDraft('position', 1, value)} />
                      <AxisField axis="Z" value={transformDraft.position[2]} onChange={(value) => updateTransformDraft('position', 2, value)} />
                    </div></div>
                    <div className="axis-group"><span>旋转</span><div>
                      <AxisField axis="X" value={transformDraft.rotation[0]} onChange={(value) => updateTransformDraft('rotation', 0, value)} />
                      <AxisField axis="Y" value={transformDraft.rotation[1]} onChange={(value) => updateTransformDraft('rotation', 1, value)} />
                      <AxisField axis="Z" value={transformDraft.rotation[2]} onChange={(value) => updateTransformDraft('rotation', 2, value)} />
                    </div></div>
                    <div className="axis-group"><span>比例</span><div>
                      <AxisField axis="X" value={transformDraft.scale[0]} onChange={(value) => updateTransformDraft('scale', 0, value)} />
                      <AxisField axis="Y" value={transformDraft.scale[1]} onChange={(value) => updateTransformDraft('scale', 1, value)} />
                      <AxisField axis="Z" value={transformDraft.scale[2]} onChange={(value) => updateTransformDraft('scale', 2, value)} />
                    </div></div>
                    <button className="precision-preview" onClick={previewTransformDraft} disabled={selectedNode.locked || concept.loading || Boolean(concept.pendingPreview)}>预览精确调整</button>
                  </div>
                )}
              </section>
            )}
          </div>
{componentDrawerOpen || exportOpen || qualityOpen ? (
            <WorkbenchDrawerStack
              componentDrawerOpen={componentDrawerOpen}
              exportOpen={exportOpen}
              qualityOpen={qualityOpen}
              component={{
                mode: componentDrawerMode,
                selectedModuleLabel: selectedModuleLabel,
                componentCategory: componentCategory,
                reviewStatusFilter: reviewStatusFilter,
                categories: COMPONENT_CATEGORIES,
                filterCounts: componentFilterCounts,
                query: componentQuery,
                displayedComponents: displayedComponents,
                totalModuleCount: catalogModules.length,
                selectedLibraryModule: selectedLibraryModule,
                selectedLibraryModuleId: selectedLibraryModuleId,
                selectedNode: selectedNode ? { node_id: selectedNode.node_id, module_id: selectedNode.module_id, locked: selectedNode.locked } : null,
                selectedModuleCategory: selectedModule?.manifest.category ?? null,
                graphNodes: concept.graphRecord?.graph.nodes ?? [],
                favoriteModuleIds: favoriteModuleIds,
                thumbnailFailures: thumbnailFailures,
                drawerRef: drawerFocusRef,
                canReplaceSelected: canReplaceSelected,
                expanded: drawerExpanded,
                loading: concept.loading || componentCatalogPresentation.loading,
                legacyDesignReadOnly: legacyDesignReadOnly,
                agentAssetActive: activeDesignSnapshot?.active_design.source === 'agent_asset',
                qualityStatusFor: qualityStatusFor,
                onResizeStart: beginDrawerResize,
                onResizeKeyDown: resizeDrawerByKeyboard,
                onCategoryChange: setComponentCategory,
                onReviewStatusChange: setReviewStatusFilter,
                onQueryChange: setComponentQuery,
                onModeToggle: toggleComponentDrawerMode,
                onClose: closeAllDrawers,
                onSelectModule: selectLibraryModule,
                onToggleFavorite: toggleLibraryFavorite,
                onThumbnailError: recordLegacyThumbnailFailure,
                onLocateModule: (module) => {
                  const node = concept.graphRecord?.graph.nodes.find((item) => item.module_id === module.manifest.module_id)
                  if (node) selectGraphNode(node.node_id)
                  else setAssistantNote('候选资产已选中；主视图只显示已确认版本，替换会先创建 ChangeSet 预览。')
                },
                onPreviewReplace: handleReplaceSelected,
                onDiscardReplacement: () => concept.discardModuleReplacement(),
                onConfirmReplacement: () => concept.confirmModuleReplacement(),
                thumbnailUrl: (moduleId) => forgeApi.getModuleAssetThumbnailUrl(moduleId),
              }}
              exportDrawer={{
                exportPurpose: exportPurpose,
                exportPurposeOptions: EXPORT_PURPOSES,
                agentAssetActive: activeDesignSnapshot?.active_design.source === 'agent_asset',
                activeAgentAssetVersion: activeAgentAssetVersion,
                activeDesignIdle: activeDesignState.operation === 'idle',
                activeVersionLabel: activeVersionSummary ? `v${activeVersionSummary.version_no}` : '—',
                originLabel: ORIGIN_CLAIM_LABELS[selectedModule?.catalog_metadata.origin_claim ?? 'unknown'],
                hasLegacyVersion: Boolean(concept.version?.module_graph_id),
                loading: concept.loading,
                drawerRef: drawerFocusRef,
                onClose: closeAllDrawers,
                onPurposeChange: (purpose, format: ExportFormat) => { setExportPurpose(purpose); setExportFormat(format) },
                onExport: handleCreateExport,
                onDownloadAgentGlb: handleDownloadAgentGlb,
                renderSet: agentRenderPresentation.renderSet,
                renderLoading: agentRenderPresentation.renderLoading,
                renderPackageLoading: agentRenderPresentation.renderPackageLoading,
                onRenderViews: handleRenderAgentViews,
                onDownloadRenderView: handleDownloadAgentRenderView,
                onDownloadRenderPackage: handleDownloadAgentRenderPackage,
              }}
              quality={{
                agentAssetActive: activeDesignSnapshot?.active_design.source === 'agent_asset',
                activeAgentAssetVersion: activeAgentAssetVersion,
                agentQualityReport: agentQualityReport,
                agentAssetChangeSet: agentAssetChangeSet,
                graphReady: Boolean(concept.graphRecord),
                legacyVersionReady: Boolean(concept.version?.module_graph_id),
                legacyQualityStatus: concept.qualityRun?.report.status,
                legacyFindings: concept.qualityRun?.report.findings ?? [],
                loading: concept.loading,
                drawerRef: drawerFocusRef,
                onClose: closeAllDrawers,
                onFocusLegacyFinding: (finding) => { focusQualityFinding(finding); closeAllDrawers() },
                onInspectAgentAsset: () => void inspectAgentAsset(),
                onRunLegacyInspection: () => concept.runQualityInspection(),
              }}
            />
          ) : null}
        </main>

        <aside className="cad-right-rail">
          <section className="cad-panel properties-panel">
            <div className="cad-panel-title"><span><SlidersHorizontal size={16} /> 属性面板</span></div>
            <nav className="inspector-tabs" aria-label="属性分类">
              {([
                ['parameters', '参数'],
                ['appearance', '外观'],
                ['connections', '连接'],
                ['inspection', '检查'],
              ] as Array<[InspectorTab, string]>).map(([id, label]) => (
                <button key={id} className={inspectorTab === id ? 'active' : ''} onClick={() => setLegacyInspectorTab(id)}>
                  {label}
                </button>
              ))}
            </nav>
            {inspectorTab === 'parameters' && <>
              <label className="wide-field"><span>Graph 节点</span><input value={selectedNode?.node_id ?? '未选择'} readOnly /></label>
              <label className="wide-field"><span>模块资产</span><input value={selectedModule?.manifest.module_id ?? '—'} readOnly /></label>
              <div className="node-actions">
                <button onClick={toggleSelectedNodeVisibility} disabled={!selectedNode}>
                  <Eye size={13} /> {selectedNode && legacyModuleGraphOverlay.hiddenNodeIds.includes(selectedNode.node_id) ? '显示' : '隐藏'}
                </button>
                <button onClick={() => selectedNode && setLegacyFocusNode(selectedNode.node_id)} disabled={!selectedNode}>
                  <Crosshair size={13} /> 聚焦
                </button>
                <button className={showConnectors ? 'active' : ''} onClick={() => setViewportShowConnectors(!showConnectors)}>
                  <ShareNetwork size={13} /> Connector
                </button>
                <button
                  className={selectedNode?.mirror_axis === 'x' ? 'active' : ''}
                  onClick={handleToggleMirrorX}
                  disabled={!selectedNode || selectedNode.locked || concept.loading || legacyDesignReadOnly}
                  title={selectedNode?.locked ? '锁定节点不能镜像' : '通过 ChangeSet 创建 X 轴镜像子版本'}
                >
                  <ArrowsLeftRight size={13} /> {selectedNode?.mirror_axis === 'x' ? '取消镜像' : 'X 镜像'}
                </button>
              </div>
              <div className="axis-group"><span>位置</span><div>
                <AxisField axis="X" value={transformDraft.position[0]} onChange={(value) => updateTransformDraft('position', 0, value)} />
                <AxisField axis="Y" value={transformDraft.position[1]} onChange={(value) => updateTransformDraft('position', 1, value)} />
                <AxisField axis="Z" value={transformDraft.position[2]} onChange={(value) => updateTransformDraft('position', 2, value)} />
              </div></div>
              <div className="axis-group"><span>旋转（rad）</span><div>
                <AxisField axis="X" value={transformDraft.rotation[0]} onChange={(value) => updateTransformDraft('rotation', 0, value)} />
                <AxisField axis="Y" value={transformDraft.rotation[1]} onChange={(value) => updateTransformDraft('rotation', 1, value)} />
                <AxisField axis="Z" value={transformDraft.rotation[2]} onChange={(value) => updateTransformDraft('rotation', 2, value)} />
              </div></div>
              <div className="axis-group"><span>缩放</span><div>
                <AxisField axis="X" value={transformDraft.scale[0]} onChange={(value) => updateTransformDraft('scale', 0, value)} />
                <AxisField axis="Y" value={transformDraft.scale[1]} onChange={(value) => updateTransformDraft('scale', 1, value)} />
                <AxisField axis="Z" value={transformDraft.scale[2]} onChange={(value) => updateTransformDraft('scale', 2, value)} />
              </div></div>
              <div className="transform-command-controls" data-testid="transform-command-controls">
                <button
                  className={transformSpace === 'world' ? 'active' : ''}
                  onClick={() => setLegacyTransformSpace(transformSpace === 'world' ? 'local' : 'world')}
                  disabled={!selectedNode || selectedNode.locked || concept.loading || legacyDesignReadOnly}
                >{transformSpace === 'world' ? '世界坐标' : '本地坐标'}</button>
                <button
                  className={snapEnabled ? 'active' : ''}
                  onClick={() => setLegacySnapEnabled(!snapEnabled)}
                  disabled={!selectedNode || selectedNode.locked || concept.loading}
                >{snapEnabled ? '吸附：1 mm / 15°' : '吸附：关'}</button>
              </div>
              <button
                className="transform-preview-action"
                onClick={previewTransformDraft}
                disabled={!selectedNode || selectedNode.locked || selectedNode.node_id === concept.graphRecord?.graph.root_node_id || concept.loading || Boolean(concept.pendingPreview) || legacyDesignReadOnly}
                title="移动、旋转和缩放先写入 ChangeSet 幽灵预览；确认后才创建子版本"
              >预览变换</button>
              <label className="wide-field"><span>镜像轴</span><input value={selectedNode?.mirror_axis ?? 'none'} readOnly /></label>
              <div className="property-divider" />
              <div className="property-heading">概念比例 <CaretDown size={13} /></div>
              <PropertyNumber label="整体长度" value={parameters.overallLength} unit="mm" onChange={(value) => updateParameter('overallLength', value)} />
              <PropertyNumber label="主体高度" value={parameters.bodyHeight} unit="mm" onChange={(value) => updateParameter('bodyHeight', value)} />
              <PropertyNumber label="前部长度" value={parameters.frontShellLength} unit="mm" onChange={(value) => updateParameter('frontShellLength', value)} />
              <PropertyNumber label="握持角度" value={parameters.gripAngle} unit="°" onChange={(value) => updateParameter('gripAngle', value)} />
              <PropertyNumber label="外壳厚度" value={parameters.shellThickness} unit="mm" onChange={(value) => updateParameter('shellThickness', value)} />
            </>}
            {inspectorTab === 'appearance' && (
              <MaterialDrawer
                materialPresets={materialPresets}
                selectedMaterialId={appearanceMaterialId}
                detailDensity={parameters.detailDensity}
                selectedPartLabel={selectedAgentPart ? `已选部件 · ${displayPartRole(selectedAgentPart.role)}` : '当前预览部件'}
                selectedZoneLabel={appearanceMaterialZoneId ? `材质区 ${appearanceMaterialZoneId}` : '主材质区'}
                materialZoneIds={selectedAgentPart?.material_zone_ids ?? []}
                selectedZoneId={activeDesignSelectedMaterialZoneId(activeDesignSnapshot) ?? appearanceMaterialZoneId}
                activeDomain={activeMaterialDomain}
                compatibilityOnly={materialCompatibilityOnly}
                query={materialQuery}
                category={materialCategory}
                catalogLoading={agentMaterialCatalogPresentation.loading}
                catalogMessage={agentMaterialCatalogPresentation.catalogMessage}
                disabled={legacyDesignReadOnly || isExternalGlbReference || Boolean(agentAssetChangeSet)}
                onMaterialChange={selectMaterialPreselection}
                onDetailDensityChange={(value) => updateParameter('detailDensity', value)}
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
                  } else {
                    setAssistantNote(`已预览「${preset.display_name}」；保存为可编辑模型并选中部件后，才能确认材质区修改。`)
                  }
                }}
                onPreviewNote={(preset) => setAssistantNote(`已将 blockout 预览材质切换为「${preset.display_name}」；确认型材质 ChangeSet 仍未写入版本。`)}
              />
            )}
            {inspectorTab === 'connections' && <div className="connection-list">
              <div className="connection-summary">
                <span>{selectedNodeConnections.length} 条真实连接</span>
                <button
                  onClick={() => selectedNode && concept.previewConnectorSnap(selectedNode.node_id)}
                  disabled={!canSnapSelectedNode || concept.loading || Boolean(concept.pendingPreview) || legacyDesignReadOnly}
                  title={!canSnapSelectedNode ? '选择一个未锁定、已连接的非根节点以修复吸附。' : '以父 Connector 为基准生成可确认的吸附 ChangeSet。'}
                >修复并预览吸附</button>
              </div>
              {(selectedModule?.manifest.connectors ?? []).map((connector) => {
                const connected = (concept.graphRecord?.graph.edges ?? []).some((edge) => (
                  edge.from_connector_id === connector.connector_id
                  || edge.to_connector_id === connector.connector_id
                ))
                return (
                  <div key={connector.connector_id}>
                    <DfmRow
                      label={connector.slot}
                      value={connected ? '已连接' : '可用'}
                      ok={connected}
                    />
                    <small className="connector-contract">
                      {connector.connector_type} · {connector.exclusive ? '独占' : '可共享'} · 缩放 {connector.scale_range[0]}–{connector.scale_range[1]}
                    </small>
                  </div>
                )
              })}
              {!selectedModule && <span className="muted-inspector">选择一个 Graph 节点查看真实 Connector。</span>}
            </div>}
            {inspectorTab === 'inspection' && <>
              <DfmRow label="Graph 状态" value={concept.graphRecord?.validation_status ?? '未运行'} ok={concept.graphRecord?.validation_status === 'valid'} />
              <DfmRow label="模块节点" value={String(concept.graphRecord?.graph.nodes.length ?? 0)} ok={Boolean(concept.graphRecord)} />
              <DfmRow label="连接边" value={String(concept.graphRecord?.graph.edges?.length ?? 0)} ok={Boolean(concept.graphRecord)} />
              <DfmRow
                label="Mesh/Assembly"
                value={qualityStatusLabel(concept.qualityRun?.report.status)}
                ok={concept.qualityRun?.report.status === 'passed'}
              />
              {(concept.qualityRun?.report.findings ?? []).slice(0, 4).map((finding) => (
                <button
                  type="button"
                  className={`quality-finding ${finding.severity}`}
                  key={finding.finding_id}
                  disabled={!finding.node_ids?.length}
                  onClick={() => focusQualityFinding(finding)}
                  title={finding.node_ids?.length ? `选择并聚焦 ${finding.node_ids.join(', ')}` : undefined}
                >
                  <strong>{finding.check_id}</strong>
                  <span>{finding.message}</span>
                  {finding.measured_value != null && <small>测量值：{String(finding.measured_value)}</small>}
                  {Boolean(finding.geometry_refs?.length) && (
                    <small>
                      局部三角形：{finding.geometry_refs!.reduce(
                        (count, reference) => count + (reference.triangle_indices?.length ?? 0),
                        0,
                      )} · 点击高亮双方
                    </small>
                  )}
                </button>
              ))}
              <div className="dfm-suggestion"><WarningCircle size={15} /> 几何检查不代表结构强度、制造可行性或使用安全验证。</div>
              <button
                className="secondary-action"
                disabled={concept.loading || !concept.version?.module_graph_id || legacyDesignReadOnly}
                onClick={() => concept.runQualityInspection()}
                title="检查索引、退化面、法线、UV0、拓扑、Connector 对齐与未连接组件精确穿插"
              >
                {concept.loading ? '检查中…' : '运行实际几何检查'}
              </button>
            </>}
            {concept.pendingManualChange && concept.pendingPreview && (
              <div className="change-preview-card manual-transform-preview" data-testid="manual-transform-preview">
                <div><strong>{concept.pendingManualChange.summary.startsWith('Snap ') ? 'Connector 吸附预览 · 待确认' : '变换幽灵预览 · 待确认'}</strong><span>{concept.pendingManualChange.summary}</span></div>
                <ul>{concept.pendingManualChange.operations.map((operation) => <li key={operation.operation_id}>{formatChangeOperation(operation)}</li>)}</ul>
                <div className="change-preview-actions">
                  <button onClick={() => concept.discardManualChange()} disabled={concept.loading || legacyDesignReadOnly}>放弃预览</button>
                  <button className="confirm" onClick={() => concept.confirmManualChange()} disabled={concept.loading || legacyDesignReadOnly}>确认并创建新版本</button>
                </div>
              </div>
            )}
          </section>

          <section className="cad-panel export-panel">
            <div className="cad-panel-title"><span><Export size={16} /> 展示与导出</span></div>
            <div className="export-formats">
              {[
                { id: 'SOURCE ZIP', enabled: true },
                { id: 'GLB', enabled: true },
                { id: 'OBJ', enabled: true },
                { id: 'PNG', enabled: true },
                { id: 'MP4', enabled: true },
              ].map((format) => (
                <button
                  key={format.id}
                  className={exportFormat === format.id ? 'active' : ''}
                  onClick={() => format.enabled && setExportFormat(format.id)}
                  disabled={!format.enabled}
                  title={format.enabled ? '当前可用' : 'R5 实现'}
                >
                  {format.id}
                </button>
              ))}
            </div>
            <div className="export-summary">
              <span><FileArrowDown size={15} /> 当前格式</span>
              <strong>{exportFormat}</strong>
            </div>
            {concept.lastExport && (
              <div className="last-export">{concept.lastExport.export_id} · {concept.lastExport.package_sha256.slice(0, 10)}…</div>
            )}
            <button
              className="primary-action"
              onClick={handleCreateExport}
              disabled={!concept.version?.module_graph_id || concept.loading}
            >
              <FileArrowDown size={16} /> {
                exportFormat === 'GLB'
                  ? '创建并下载 combined GLB'
                  : exportFormat === 'OBJ'
                  ? '创建并下载 combined OBJ'
                  : exportFormat === 'PNG'
                  ? '创建并下载透明 preview.png'
                  : exportFormat === 'MP4'
                  ? '创建并下载转台 MP4'
                  : '创建并下载概念源包'
              }
            </button>
            {exportFormat === 'OBJ' && concept.lastExport && (
              <button
                className="secondary-action"
                onClick={() => downloadExistingExport(
                  forgeApi.getConceptCombinedMtlUrl(concept.lastExport!.export_id),
                  'combined.mtl',
                )}
              >
                下载配套 combined.mtl
              </button>
            )}
            {exportFormat === 'PNG' && concept.lastExport && (
              <>
                <button
                  className="secondary-action"
                onClick={() => downloadExistingExport(
                  forgeApi.getConceptExplodedPngUrl(concept.lastExport!.export_id),
                  `${concept.lastExport!.export_id}-exploded.png`,
                )}
                >
                  下载 exploded.png
                </button>
                <button
                  className="secondary-action"
                onClick={() => downloadExistingExport(
                  forgeApi.getConceptRenderSetUrl(concept.lastExport!.export_id),
                  `${concept.lastExport!.export_id}-renders.zip`,
                )}
                >
                  下载正交视图与转台 ZIP
                </button>
                {concept.lastExport.turntable_video_sha256 && (
                  <button
                    className="secondary-action"
                    onClick={() => downloadExistingExport(
                      forgeApi.getConceptTurntableVideoUrl(concept.lastExport!.export_id),
                      `${concept.lastExport!.export_id}-turntable.mp4`,
                    )}
                  >
                    下载转台 MP4
                  </button>
                )}
              </>
            )}
          </section>
        </aside>
      </div>

      {concept.pendingPreview && (
        <section className="agent-change-review" data-testid="agent-change-review">
          <div className="agent-change-review-copy">
            <span>本次修改</span>
            <strong>{concept.pendingChange?.change_set.summary ?? concept.pendingManualChange?.summary ?? '组件替换预览已准备好'}</strong>
            {concept.pendingChange && (
              <small>{concept.pendingChange.change_set.operations.slice(0, 2).map(formatChangeOperation).join('；')}</small>
            )}
            {concept.pendingManualChange && (
              <small>{concept.pendingManualChange.operations.slice(0, 2).map(formatChangeOperation).join('；')}</small>
            )}
            {concept.pendingReplacement && <small>当前设计尚未被覆盖；确认后会保存为新版本。</small>}
          </div>
          <div className="agent-change-review-actions">
            {concept.pendingChange && <button onClick={() => concept.discardPlannedChange()} disabled={concept.loading || legacyDesignReadOnly}>撤销本次修改</button>}
            {concept.pendingManualChange && <button onClick={() => concept.discardManualChange()} disabled={concept.loading || legacyDesignReadOnly}>撤销本次修改</button>}
            {concept.pendingReplacement && <button onClick={() => concept.discardModuleReplacement()} disabled={concept.loading || legacyDesignReadOnly}>撤销本次修改</button>}
            {concept.pendingChange && <button className="confirm" onClick={() => concept.confirmPlannedChange()} disabled={concept.loading || legacyDesignReadOnly}>保留此修改</button>}
            {concept.pendingManualChange && <button className="confirm" onClick={() => concept.confirmManualChange()} disabled={concept.loading || legacyDesignReadOnly}>保留此修改</button>}
            {concept.pendingReplacement && <button className="confirm" onClick={() => concept.confirmModuleReplacement()} disabled={concept.loading || legacyDesignReadOnly}>保留此修改</button>}
          </div>
        </section>
      )}


      <footer className="cad-status-bar" role="status" aria-live="polite" aria-label="工作台状态">
        <span>{concept.loading ? 'Agent 正在处理' : '设计就绪'}</span>
        <span>{selectedNode ? `正在调整：${selectedModuleLabel}` : '点击模型的任意部件即可调整'}</span>
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

function IconAction({
  icon: Icon,
  label,
  onClick,
  disabled = false,
  title,
}: {
  icon: typeof Plus
  label: string
  onClick?: () => void
  disabled?: boolean
  title?: string
}) {
  return <button onClick={onClick} disabled={disabled} title={title}><Icon size={15} /><span>{label}</span></button>
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

function ParameterInput({
  label,
  value,
  unit,
  onChange,
  disabled = false,
  title,
}: {
  label: string
  value: number
  unit: string
  onChange: (value: number) => void
  disabled?: boolean
  title?: string
}) {
  return (
    <label className="parameter-row" title={title}>
      <span>{label}</span>
      <input
        type="number"
        value={value}
        disabled={disabled}
        onChange={(event) => onChange(Number(event.target.value))}
      />
      <small>{unit}</small>
    </label>
  )
}

function PropertyNumber({
  label,
  value,
  unit,
  onChange,
}: {
  label: string
  value: number
  unit: string
  onChange: (value: number) => void
}) {
  return (
    <label className="property-number">
      <span>{label}</span>
      <input type="number" value={value} onChange={(event) => onChange(Number(event.target.value))} />
      <small>{unit}</small>
    </label>
  )
}

function AxisField({
  axis,
  value,
  onChange,
}: {
  axis: string
  value: number
  onChange: (value: number) => void
}) {
  return (
    <label className={`axis-field axis-${axis.toLowerCase()}`}>
      <span>{axis}</span>
      <input
        type="number"
        step="0.01"
        value={Number.isFinite(value) ? value : ''}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </label>
  )
}

function DfmRow({ label, value, ok }: { label: string; value: string; ok: boolean }) {
  return (
    <div className="dfm-row">
      <span>{label}</span>
      <strong>{value}</strong>
      {ok ? <Check size={15} weight="bold" /> : <WarningCircle size={15} />}
    </div>
  )
}

function qualityStatusLabel(status?: 'passed' | 'warning' | 'failed' | 'not_run') {
  if (!status || status === 'not_run') return '未运行'
  return ({ passed: '通过', warning: '需复核', failed: '失败' } as const)[status]
}

function formatChangeOperation(operation: DesignChangeSet['operations'][number]): string {
  if (operation.op === 'replace_module') {
    return `替换 ${operation.node_id} → ${operation.module_id}`
  }
  if (operation.op === 'set_mirror') {
    return `镜像 ${operation.node_id} → ${operation.mirror_axis}`
  }
  if (operation.op === 'set_transform') {
    const position = operation.transform?.position?.map((value) => Number(value).toFixed(2)).join(', ')
    return `变换 ${operation.node_id}${position ? ` · 位置 ${position} mm` : ''}`
  }
  if (operation.op === 'set_parameter' || operation.op === 'set_style') {
    const value = Array.isArray(operation.value)
      ? operation.value.join(', ')
      : String(operation.value)
    return `${operation.path} → ${value}`
  }
  return operation.op
}

function formatVersionTime(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date)
}

function formatAxis(value: number | undefined): string {
  return typeof value === 'number' && Number.isFinite(value) ? value.toFixed(2) : '—'
}

function identityTransform(): Transform {
  return {
    position: [0, 0, 0],
    rotation: [0, 0, 0],
    scale: [1, 1, 1],
  }
}

function copyTransform(transform: Transform): Transform {
  return {
    position: [...transform.position],
    rotation: [...transform.rotation],
    scale: [...transform.scale],
  }
}

function distanceBetween(
  first: [number, number, number],
  second: [number, number, number],
) {
  return Math.hypot(
    first[0] - second[0],
    first[1] - second[1],
    first[2] - second[2],
  )
}

function angleBetweenNormals(
  first: [number, number, number],
  second: [number, number, number],
) {
  const firstLength = Math.hypot(...first)
  const secondLength = Math.hypot(...second)
  if (firstLength === 0 || secondLength === 0) return 0
  const dot = (first[0] * second[0] + first[1] * second[1] + first[2] * second[2]) / (firstLength * secondLength)
  return Math.acos(Math.max(-1, Math.min(1, dot))) * 180 / Math.PI
}

async function downloadBrowserFile(url: string, fallbackName?: string): Promise<void> {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`下载请求失败：HTTP ${response.status}`)
  const blob = await response.blob()
  const urlFallbackName = url.split('/').pop() || 'forgecad-export'
  const disposition = response.headers.get('content-disposition') ?? ''
  const filenameMatch = disposition.match(/filename="?([^";]+)"?/i)
  const anchor = document.createElement('a')
  const objectUrl = URL.createObjectURL(blob)
  anchor.href = objectUrl
  anchor.download = filenameMatch?.[1] || fallbackName || urlFallbackName
  anchor.style.display = 'none'
  document.body.appendChild(anchor)
  anchor.click()
  anchor.remove()
  window.setTimeout(() => URL.revokeObjectURL(objectUrl), 0)
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

function exportDownloadFilename(exportId: string, format: string): string {
  if (format === 'GLB') return `${exportId}.glb`
  if (format === 'OBJ') return `${exportId}.obj`
  if (format === 'PNG') return `${exportId}-preview.png`
  if (format === 'MP4') return `${exportId}-turntable.mp4`
  return `${exportId}.zip`
}

function errorText(caught: unknown): string {
  return caught instanceof Error ? caught.message : String(caught)
}
