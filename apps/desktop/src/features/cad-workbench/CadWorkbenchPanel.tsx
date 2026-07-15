import { useCallback, useEffect, useMemo, useRef, useState, type ChangeEvent } from 'react'
import {
  ArrowsClockwise,
  ArrowsOutCardinal,
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
} from '@phosphor-icons/react'
import { ForgeApiError, forgeApi, mapActiveDesignError } from '../../shared/api/forgeApi'
import type { ActiveDesignNavigation, AgentAssetChangeSet, AgentAssetQualityReport, AgentAssetRenderView, AgentAssetVersion, AgentComponentCandidate, AgentMaterialPreset, AgentPartEditOperation, AgentStructureSuggestion, AgentTurn } from '../../shared/types'
import { useRuntime } from '../../app/providers/RuntimeProvider'
import {
  getProviderConfig as getTauriProviderConfig,
  saveProviderConfig as saveTauriProviderConfig,
  type ProviderConfigMetadata,
} from '../../shared/tauri/agentSupervisor'
import { ModuleGraphViewport } from './ModuleGraphViewport'
import { AgentConversation } from './AgentConversation'
import { AgentSelectionCard } from './AgentSelectionCard'
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
import { useAgentDirectionConceptPreviews } from './useAgentDirectionConceptPreviews'
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
import { useComponentCatalogPresentation } from './useComponentCatalogPresentation'
import { useConceptWorkbench } from './useConceptWorkbench'
import { isProviderExecutionError, providerCheckPresentation } from './providerConnectionPresentation'
import './cad-workbench.css'

type Tool = 'select' | 'move' | 'rotate' | 'scale' | 'orbit' | 'measure' | 'section'
type CameraView = 'iso' | 'front' | 'top' | 'right'
type LightPreset = 'cad_neutral' | 'soft_studio' | 'concept_contrast'
type AgentTurnRecordResult = { recorded: boolean; clarification: boolean; cancelled: boolean; failed: boolean }


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
    agentDirectionConceptPreviewState,
    openDirectionConceptPreviewProject,
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
    shapeProgram: agentBlockoutShapeProgram,
    segmentation: agentBlockoutSegmentation,
  } = agentBlockoutDisplay
  const blockoutPreviewPresentation = selectAgentBlockoutPreviewPresentation(agentBlockoutDisplay)
  const agentPlanSourcePresentation = selectAgentPlanSourcePresentation(agentPlan)
  const [cameraView, setCameraView] = useState<CameraView>('iso')
  const [lightPreset, setLightPreset] = useState<LightPreset>('cad_neutral')
  const [presentationProfile, setPresentationProfile] = useState<'quick_sketch' | 'showcase'>('showcase')
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

  useEffect(() => {
    openConversationProject(concept.project?.project_id ?? null)
    openBlockoutProject(concept.project?.project_id ?? null)
    openDirectionConceptPreviewProject(concept.project?.project_id ?? null)
    openAgentAssetWorkspaceProject(concept.project?.project_id ?? null)
    openViewportDisplayPreferences(concept.project?.project_id ?? null)
    setAgentAssetChangeSet(null)
    setAgentCandidateSelectedPartId(null)
  }, [concept.project?.project_id, openAgentAssetWorkspaceProject, openBlockoutProject, openConversationProject, openDirectionConceptPreviewProject, openViewportDisplayPreferences])

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
          return { recorded: false, clarification: false, cancelled: true, failed: false }
        }
      }
      const turnPromise = api.startAgentTurn(threadId, {
        client_request_id: `agent-turn-${Date.now()}`,
        message,
        ...(clarificationDomainPackId ? { clarification_domain_pack_id: clarificationDomainPackId } : {}),
      })
      const discovery = window.setInterval(() => {
        void api.getAgentThread(threadId).then((detail) => {
          if (!isCurrentAgentConversationRequest(projectId, requestId)) return
          const running = [...detail.turns].reverse().find((candidate) => candidate.status === 'running')
          if (!running) return
          setActiveProviderTurnId(running.turn_id)
          receiveAgentTurn(
            projectId,
            requestId,
            threadId,
            running.items,
            parseAgentTurnPresentation(running.items, running.request_text),
          )
        }).catch(() => undefined)
      }, 150)
      let turn: AgentTurn
      try {
        turn = await turnPromise
      } finally {
        window.clearInterval(discovery)
        setActiveProviderTurnId(null)
      }
      const presentation = parseAgentTurnPresentation(turn.items, turn.request_text)
      if (!receiveAgentTurn(projectId, requestId, threadId, turn.items, presentation)) {
        return { recorded: false, clarification: false, cancelled: true, failed: false }
      }
      if (turn.status === 'cancelled') {
        setAssistantNote('本次模型请求已取消；没有创建计划、资产版本或导出。')
        return { recorded: true, clarification: false, cancelled: true, failed: false }
      }
      if (presentation.clarification) {
        clearBlockoutDisplay(projectId)
        clearAgentAssetWorkspace()
        setAgentAssetChangeSet(null)
        setAgentCandidateSelectedPartId(null)
        setAssistantNote(presentation.clarification.question)
        return { recorded: true, clarification: true, cancelled: false, failed: false }
      }
      return { recorded: true, clarification: false, cancelled: false, failed: false }
    } catch (caught) {
      if (!isCurrentAgentConversationRequest(projectId, requestId)) {
        return { recorded: false, clarification: false, cancelled: true, failed: false }
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
          return { recorded: false, clarification: false, cancelled: true, failed: false }
        }
        setAssistantNote(caught.message)
        return { recorded: false, clarification: true, cancelled: false, failed: false }
      }
      if (caught instanceof ForgeApiError && isProviderExecutionError(caught.code)) {
        const networkCall = caught.details.network_call_made === true ? 'true' : 'false'
        setAssistantNote(`模型请求失败：${caught.message}（${caught.code}，network_call_made=${networkCall}）。不会切换到离线 Planner；已保存资产没有变化。`)
        return { recorded: false, clarification: false, cancelled: false, failed: true }
      }
      // The compatibility planner remains usable when the new kernel is not
      // available yet (for example while an older local Agent is running).
      if (!markAgentKernelUnavailable(projectId, requestId)) {
        return { recorded: false, clarification: false, cancelled: true, failed: false }
      }
      return { recorded: false, clarification: false, cancelled: false, failed: false }
    }
  }, [agentThreadId, api, clearAgentAssetWorkspace, clearBlockoutDisplay, clearDirectionConceptPreviews, concept.project?.name, concept.project?.project_id, isCurrentAgentConversationRequest, markAgentKernelUnavailable, parseAgentTurnPresentation, receiveAgentClarification, receiveAgentTurn, startAgentConversationRequest])

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
    if (kernelResult.failed) {
      setChatInput('')
      return
    }
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
    setAssistantNote(kernelResult.recorded
      ? 'Agent 已生成受限设计计划；请选择计划中的预览方向继续构建。'
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
    setAssistantNote(kernelResult.recorded
      ? '已记录修改意图。自然语言修改必须等待受限 Agent Action Loop，不会回退调用旧版参数或 ChangeSet Planner；当前可继续使用分件卡中的受限操作。'
      : '修改意图未记录成功；当前资产没有变化。')
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
  return (
    <div className="cad-workbench" data-testid="cad-workbench">
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
              projectNeedsInitialization={Boolean(concept.project && !concept.project.current_version_id)}
              legacyCompatibility={legacyCompatibility}
              onCreateStarterProject={() => void concept.createStarterProject()}
              onInitializeCurrentProject={() => void concept.initializeCurrentProject()}
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
                semanticProportions={agentEditAssistPresentation.semanticProportions}
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
            <small className="planner-boundary">所有生成和调整都只影响虚构、非功能展示组件；预览确认前不会写入版本。</small>
          </section>
        </aside>

        <main className="cad-center-stage">
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
              blockoutGlbBase64={agentBlockoutGlbBase64}
              blockoutShapeProgram={agentBlockoutShapeProgram}
              blockoutMaterialOverride={agentBlockoutShapeProgram ? appearanceMaterialId : null}
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
              measureEnabled={false}
              getModuleFileUrl={getModuleFileUrl}
              onSelectNode={(nodeId) => { if (concept.legacyDetailsEnabled) selectGraphNode(nodeId) }}
              onDropModule={() => undefined}
              onTransformCommit={() => undefined}
              onMeasurePoint={() => undefined}
            />
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
              <span>单位：mm</span>
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
        </main>

        <WorkbenchInspectorRail
          mode={activeDesignSnapshot?.active_design.source === 'agent_asset'
            ? 'agent'
            : legacyDesignReadOnly
              ? 'legacy'
              : 'empty'}
          agentAssetVersion={activeAgentAssetVersion}
          agentQualityReport={agentQualityReport}
          selectedAgentPartId={displayedAgentSelectedPartId}
          materialEditor={(
            <MaterialDrawer
              materialPresets={materialPresets}
              selectedMaterialId={appearanceMaterialId}
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
          )}
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
