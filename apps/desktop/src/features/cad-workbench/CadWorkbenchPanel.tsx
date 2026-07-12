import { useCallback, useEffect, useMemo, useState, type PointerEvent as ReactPointerEvent } from 'react'
import {
  ArrowsClockwise,
  ArrowsLeftRight,
  ArrowsOutCardinal,
  CaretDown,
  CaretUp,
  ChartLineUp,
  ChatCircleDots,
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
  PaperPlaneRight,
  Plus,
  Ruler,
  SelectionAll,
  Star,
  ShareNetwork,
  SlidersHorizontal,
  Sparkle,
  WarningCircle,
} from '@phosphor-icons/react'
import { forgeApi } from '../../shared/api/forgeApi'
import type { DesignChangeSet, ModuleAssetRecord, QualityFinding, Transform } from '../../shared/types'
import { ModuleGraphViewport, type ViewportMeasurementPoint } from './ModuleGraphViewport'
import { useConceptWorkbench } from './useConceptWorkbench'
import './cad-workbench.css'

type InspectorTab = 'parameters' | 'appearance' | 'connections' | 'inspection'
type Tool = 'select' | 'move' | 'rotate' | 'scale' | 'orbit' | 'measure' | 'section'
type CameraView = 'iso' | 'front' | 'top' | 'right'
type AssistantMode = 'brief' | 'change'
type ComponentDrawerMode = 'recommended' | 'all'
type ExportPurpose = 'presentation' | 'production' | 'handoff' | 'archive'

type WeaponParameters = {
  overallLength: number
  bodyHeight: number
  frontShellLength: number
  gripAngle: number
  shellThickness: number
  detailDensity: number
}

type MeasurementAnnotation = {
  annotationId: string
  kind: 'distance' | 'normal_angle'
  points: [ViewportMeasurementPoint, ViewportMeasurementPoint]
  distanceMm: number
  angleDeg: number
}

type ModuleCategory = ModuleAssetRecord['manifest']['category']
type ComponentCategory = 'all' | ModuleCategory
type ComponentFilter = ComponentCategory | 'installed' | 'compatible' | 'favorites' | 'recent'
type ReviewStatus = 'draft' | 'pending_review' | 'approved' | 'restricted'
type QualityStatus = 'passed' | 'warning' | 'failed' | 'unavailable'

type WorkbenchSession = {
  inspectorTab: InspectorTab
  activeTool: Tool
  transformSpace: 'world' | 'local'
  snapEnabled: boolean
  cameraView: CameraView
  showGrid: boolean
  wireframe: boolean
  xRay: boolean
  sectionOffset: number
  selectedComponent: string
  selectedLibraryModuleId: string
  showConnectors: boolean
  explodeFactor: number
  measurementMode: 'distance' | 'normal_angle'
  componentCategory: ComponentFilter
  reviewStatusFilter: ReviewStatus | ''
  drawerExpanded: boolean
  drawerHeight: number
}

// CAD-only session state. The v3 key deliberately ignores the retired
// multi-workbench navigation, task views, and asset-library page state.
// v5 resets retired CAD-tool state so first use opens in simple selection mode.
const WORKBENCH_SESSION_KEY = 'forgecad.cad.session.v5'

const DEFAULT_WORKBENCH_SESSION: WorkbenchSession = {
  inspectorTab: 'parameters',
  activeTool: 'select',
  transformSpace: 'world',
  snapEnabled: true,
  cameraView: 'iso',
  showGrid: true,
  wireframe: false,
  xRay: false,
  sectionOffset: 0,
  selectedComponent: '',
  selectedLibraryModuleId: '',
  showConnectors: false,
  explodeFactor: 0,
  measurementMode: 'distance',
  componentCategory: 'all',
  reviewStatusFilter: '',
  drawerExpanded: false,
  drawerHeight: 368,
}

const DEFAULT_CONCEPT_BRIEF = '紧凑、精密硬表面、石墨灰与信号红点缀的非功能未来概念展示资产'
const DEFAULT_HIDDEN_NODE_IDS = ['node_storage']
const CONCEPT_FAMILY_SUGGESTIONS = [
  ['侦察短构', '侦察轻型、紧凑、未来工业、蓝色点缀的非功能展示道具'],
  ['堡垒装甲', '堡垒重装、层级装甲、石墨灰与信号红的非功能展示道具'],
  ['典藏长轴', '典藏仪式、长轴展示、精密硬表面、低饱和金属的非功能影视道具'],
  ['棱镜脉冲', '棱镜能量、非对称、脉冲视觉、深色金属与冷色点缀的非功能游戏道具'],
] as const

const EXPORT_PURPOSES: Array<{
  id: ExportPurpose
  title: string
  description: string
  format: 'SOURCE ZIP' | 'GLB' | 'OBJ' | 'PNG' | 'MP4'
}> = [
  { id: 'presentation', title: '展示设计', description: '用于方案评审或展示画面', format: 'PNG' },
  { id: 'production', title: '游戏 / 影视项目', description: '保留展示模型与材质', format: 'GLB' },
  { id: 'handoff', title: '交给三维设计师', description: '继续在三维软件中处理', format: 'OBJ' },
  { id: 'archive', title: '保存完整设计资料', description: '包含当前版本与概念资料', format: 'SOURCE ZIP' },
]

function readWorkbenchSession(): WorkbenchSession {
  try {
    const value = JSON.parse(window.localStorage.getItem(WORKBENCH_SESSION_KEY) ?? '{}') as Partial<WorkbenchSession>
    return {
      ...DEFAULT_WORKBENCH_SESSION,
      inspectorTab: isOneOf(value.inspectorTab, ['parameters', 'appearance', 'connections', 'inspection']) ? value.inspectorTab : 'parameters',
      activeTool: isOneOf(value.activeTool, ['select', 'move', 'rotate', 'scale', 'orbit', 'measure', 'section']) ? value.activeTool : 'select',
      transformSpace: isOneOf(value.transformSpace, ['world', 'local']) ? value.transformSpace : 'world',
      snapEnabled: typeof value.snapEnabled === 'boolean' ? value.snapEnabled : true,
      cameraView: isOneOf(value.cameraView, ['iso', 'front', 'top', 'right']) ? value.cameraView : 'iso',
      showGrid: typeof value.showGrid === 'boolean' ? value.showGrid : true,
      wireframe: typeof value.wireframe === 'boolean' ? value.wireframe : false,
      xRay: typeof value.xRay === 'boolean' ? value.xRay : false,
      sectionOffset: boundedNumber(value.sectionOffset, -100, 100, 0),
      selectedComponent: typeof value.selectedComponent === 'string' ? value.selectedComponent : '',
      selectedLibraryModuleId: typeof value.selectedLibraryModuleId === 'string' ? value.selectedLibraryModuleId : '',
      showConnectors: typeof value.showConnectors === 'boolean' ? value.showConnectors : false,
      explodeFactor: boundedNumber(value.explodeFactor, 0, 1, 0),
      measurementMode: isOneOf(value.measurementMode, ['distance', 'normal_angle']) ? value.measurementMode : 'distance',
      componentCategory: isComponentFilter(value.componentCategory) ? value.componentCategory : 'all',
      reviewStatusFilter: isOneOf(value.reviewStatusFilter, ['', 'draft', 'pending_review', 'approved', 'restricted']) ? value.reviewStatusFilter : '',
      drawerExpanded: typeof value.drawerExpanded === 'boolean' ? value.drawerExpanded : false,
      drawerHeight: boundedNumber(value.drawerHeight, 280, 520, 368),
    }
  } catch {
    return DEFAULT_WORKBENCH_SESSION
  }
}

function isOneOf<T extends string>(value: unknown, options: readonly T[]): value is T {
  return typeof value === 'string' && options.includes(value as T)
}

function boundedNumber(value: unknown, min: number, max: number, fallback: number) {
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.max(min, Math.min(max, value))
    : fallback
}

function clampNumber(value: number, min: number, max: number) {
  return Math.max(min, Math.min(max, value))
}

function isComponentFilter(value: unknown): value is ComponentFilter {
  return isOneOf(value, ['all', 'installed', 'compatible', 'favorites', 'recent', ...Object.keys(MODULE_CATEGORY_LABELS)])
}

const MODULE_CATEGORY_LABELS: Record<ModuleCategory, string> = {
  core_shell: '核心外壳',
  front_shell: '前部外壳',
  rear_shell: '后部外壳',
  grip_shell: '握持外壳',
  top_accessory: '顶部附件',
  side_accessory: '侧部附件',
  lower_structure: '下部结构',
  storage_visual: '储存视觉',
  armor_panel: '装甲面板',
}

const COMPONENT_CATEGORIES: Array<{ id: ComponentFilter; label: string }> = [
  { id: 'all', label: '全部组件' },
  { id: 'installed', label: '当前装配' },
  { id: 'compatible', label: '可替换' },
  { id: 'favorites', label: '收藏' },
  { id: 'recent', label: '最近使用' },
  ...Object.entries(MODULE_CATEGORY_LABELS).map(([id, label]) => ({
    id: id as ModuleCategory,
    label,
  })),
]

const REVIEW_STATUS_LABELS: Record<ReviewStatus, string> = {
  draft: '草稿',
  pending_review: '待审',
  approved: '已批准',
  restricted: '受限',
}

const ORIGIN_CLAIM_LABELS = {
  self_declared_original: '本人原创声明',
  third_party: '第三方来源',
  unknown: '来源待补充',
} as const

const QUALITY_STATUS_LABELS: Record<QualityStatus, string> = {
  passed: '通过',
  warning: '警告',
  failed: '失败',
  unavailable: '未检查',
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
  const [restoredSession] = useState(readWorkbenchSession)
  const [inspectorTab, setInspectorTab] = useState<InspectorTab>(() => restoredSession.inspectorTab)
  const [activeTool, setActiveTool] = useState<Tool>(() => restoredSession.activeTool)
  const [transformSpace, setTransformSpace] = useState<'world' | 'local'>(() => restoredSession.transformSpace)
  const [snapEnabled, setSnapEnabled] = useState(() => restoredSession.snapEnabled)
  const [cameraView, setCameraView] = useState<CameraView>(() => restoredSession.cameraView)
  const [showGrid, setShowGrid] = useState(() => restoredSession.showGrid)
  const [wireframe, setWireframe] = useState(() => restoredSession.wireframe)
  const [xRay, setXRay] = useState(() => restoredSession.xRay)
  const [sectionOffset, setSectionOffset] = useState(() => restoredSession.sectionOffset)
  const [selectedComponent, setSelectedComponent] = useState(() => restoredSession.selectedComponent)
  const [selectedLibraryModuleId, setSelectedLibraryModuleId] = useState(() => restoredSession.selectedLibraryModuleId)
  // The optional visual-storage module remains part of the 9-node graph, but
  // opening with it hidden gives the compact prop a single-grip silhouette.
  // Selecting it exposes the ordinary “显示” action in the property panel.
  const [hiddenNodeIds, setHiddenNodeIds] = useState<string[]>(DEFAULT_HIDDEN_NODE_IDS)
  const [focusedNodeId, setFocusedNodeId] = useState<string | null>(null)
  const [qualityHighlightNodeIds, setQualityHighlightNodeIds] = useState<string[]>([])
  const [qualityGeometryRefs, setQualityGeometryRefs] = useState<
    NonNullable<QualityFinding['geometry_refs']>
  >([])
  const [showConnectors, setShowConnectors] = useState(() => restoredSession.showConnectors)
  const [explodeFactor, setExplodeFactor] = useState(() => restoredSession.explodeFactor)
  const [measurementPoints, setMeasurementPoints] = useState<ViewportMeasurementPoint[]>([])
  const [measurementAnnotations, setMeasurementAnnotations] = useState<MeasurementAnnotation[]>([])
  const [measurementMode, setMeasurementMode] = useState<'distance' | 'normal_angle'>(() => restoredSession.measurementMode)
  const [componentCategory, setComponentCategory] = useState<ComponentFilter>(() => restoredSession.componentCategory)
  const [componentQuery, setComponentQuery] = useState('')
  const [reviewStatusFilter, setReviewStatusFilter] = useState<ReviewStatus | ''>(() => restoredSession.reviewStatusFilter)
  const [drawerExpanded, setDrawerExpanded] = useState(() => restoredSession.drawerExpanded)
  const [drawerHeight, setDrawerHeight] = useState(() => restoredSession.drawerHeight)
  const [componentDrawerOpen, setComponentDrawerOpen] = useState(false)
  const [componentDrawerMode, setComponentDrawerMode] = useState<ComponentDrawerMode>('recommended')
  const [showPrecisionAdjustments, setShowPrecisionAdjustments] = useState(false)
  const [exportOpen, setExportOpen] = useState(false)
  const [exportPurpose, setExportPurpose] = useState<ExportPurpose>('presentation')
  const [qualityOpen, setQualityOpen] = useState(false)
  const [favoriteModuleIds, setFavoriteModuleIds] = useState<string[]>([])
  const [recentModuleIds, setRecentModuleIds] = useState<string[]>([])
  const [thumbnailFailures, setThumbnailFailures] = useState<Set<string>>(() => new Set())
  const [chatInput, setChatInput] = useState('')
  const [assistantMode, setAssistantMode] = useState<AssistantMode>('brief')
  const [assistantNote, setAssistantNote] = useState(
    '输入概念需求后生成受限设计方向；AI 只使用已注册的展示模块。',
  )
  const [exportFormat, setExportFormat] = useState('SOURCE ZIP')
  const [parameters, setParameters] = useState<WeaponParameters>({
    overallLength: 230,
    bodyHeight: 54,
    frontShellLength: 120,
    gripAngle: 15,
    shellThickness: 2.5,
    detailDensity: 68,
  })
  const [transformDraft, setTransformDraft] = useState<Transform>(() => identityTransform())

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
    try {
      // Draft parameters and ChangeSet previews are intentionally omitted:
      // only confirmed Versions are durable design truth.
      window.localStorage.setItem(WORKBENCH_SESSION_KEY, JSON.stringify({
        inspectorTab,
        activeTool,
        transformSpace,
        snapEnabled,
        cameraView,
        showGrid,
        wireframe,
        xRay,
        sectionOffset,
        selectedComponent,
        selectedLibraryModuleId,
        showConnectors,
        explodeFactor,
        measurementMode,
        componentCategory,
        reviewStatusFilter,
        drawerExpanded,
        drawerHeight,
      } satisfies WorkbenchSession))
    } catch {
      // A storage failure must not prevent a local workbench session from opening.
    }
  }, [
    activeTool,
    cameraView,
    componentCategory,
    drawerExpanded,
    drawerHeight,
    explodeFactor,
    inspectorTab,
    measurementMode,
    reviewStatusFilter,
    sectionOffset,
    selectedComponent,
    selectedLibraryModuleId,
    showConnectors,
    showGrid,
    snapEnabled,
    transformSpace,
    wireframe,
    xRay,
  ])

  useEffect(() => {
    // A project reload briefly clears graphRecord while API requests are in
    // flight. Preserve the stored selection during that gap so a restart does
    // not replace it with the root node before the same graph returns.
    if (!concept.graphRecord) return
    const nodes = concept.graphRecord?.graph.nodes ?? []
    if (nodes.length === 0) {
      setSelectedComponent('')
      setSelectedLibraryModuleId('')
      return
    }
    setSelectedComponent((current) => {
      const existingNode = nodes.find((node) => node.node_id === current)
      const nextNode = (existingNode && existingNode.node_id !== concept.graphRecord?.graph.root_node_id && !existingNode.locked
        ? existingNode
        : undefined)
        ?? nodes.find((node) => node.node_id !== concept.graphRecord?.graph.root_node_id && !node.locked)
        ?? nodes[0]
      setSelectedLibraryModuleId((currentModuleId) => currentModuleId || nextNode.module_id)
      return nextNode.node_id
    })
    setHiddenNodeIds((current) => current.filter((nodeId) => nodes.some((node) => node.node_id === nodeId)))
  }, [concept.graphRecord])

  const catalogPreferenceKey = concept.project
    ? `forgecad.component-library.preferences.v1.${concept.project.profile.pack_id}`
    : null

  useEffect(() => {
    if (!catalogPreferenceKey) return
    try {
      const stored = window.localStorage.getItem(catalogPreferenceKey)
      const parsed = stored ? JSON.parse(stored) : null
      setFavoriteModuleIds(Array.isArray(parsed?.favorites) ? parsed.favorites : [])
      setRecentModuleIds(Array.isArray(parsed?.recent) ? parsed.recent : [])
    } catch {
      setFavoriteModuleIds([])
      setRecentModuleIds([])
    }
  }, [catalogPreferenceKey])

  useEffect(() => {
    if (!catalogPreferenceKey) return
    window.localStorage.setItem(catalogPreferenceKey, JSON.stringify({
      favorites: favoriteModuleIds,
      recent: recentModuleIds,
    }))
  }, [catalogPreferenceKey, favoriteModuleIds, recentModuleIds])

  const selectedNode = concept.graphRecord?.graph.nodes.find(
    (node) => node.node_id === selectedComponent,
  ) ?? null
  const selectedModule = concept.modules.find(
    (module) => module.manifest.module_id === selectedNode?.module_id,
  ) ?? null
  const selectedLibraryModule = concept.modules.find(
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
    const query = componentQuery.trim().toLowerCase()
    const installedModuleIds = new Set(
      (concept.graphRecord?.graph.nodes ?? []).map((node) => node.module_id),
    )
    const selectedCategory = selectedModule?.manifest.category
    const categoryItems = componentCategory === 'all'
      ? concept.modules
      : componentCategory === 'installed'
      ? concept.modules.filter((component) => installedModuleIds.has(component.manifest.module_id))
      : componentCategory === 'compatible'
      ? concept.modules.filter((component) => (
        selectedNode !== null
        && !selectedNode.locked
        && component.manifest.category === selectedCategory
      ))
      : componentCategory === 'favorites'
      ? concept.modules.filter((component) => favoriteModuleIds.includes(component.manifest.module_id))
      : componentCategory === 'recent'
      ? concept.modules.filter((component) => recentModuleIds.includes(component.manifest.module_id))
      : concept.modules.filter((component) => component.manifest.category === componentCategory)
    return categoryItems.filter((component) => {
      const metadata = component.catalog_metadata
      const haystack = [
        component.manifest.module_id,
        MODULE_CATEGORY_LABELS[component.manifest.category],
        metadata.display_name,
        metadata.description,
        ...(metadata.tags ?? []),
      ].join(' ').toLowerCase()
      return (!query || haystack.includes(query))
        && (!reviewStatusFilter || metadata.review_status === reviewStatusFilter)
    })
  }, [
    componentCategory,
    componentQuery,
    concept.graphRecord,
    concept.modules,
    favoriteModuleIds,
    recentModuleIds,
    reviewStatusFilter,
    selectedModule,
    selectedNode,
  ])

  const componentFilterCounts = useMemo(() => {
    const installedModuleIds = new Set(
      (concept.graphRecord?.graph.nodes ?? []).map((node) => node.module_id),
    )
    const compatibleCount = selectedNode && !selectedNode.locked && selectedModule
      ? concept.modules.filter((component) => component.manifest.category === selectedModule.manifest.category).length
      : 0
    return {
      all: concept.modules.length,
      installed: installedModuleIds.size,
      compatible: compatibleCount,
      favorites: concept.modules.filter((component) => favoriteModuleIds.includes(component.manifest.module_id)).length,
      recent: concept.modules.filter((component) => recentModuleIds.includes(component.manifest.module_id)).length,
    }
  }, [concept.graphRecord, concept.modules, favoriteModuleIds, recentModuleIds, selectedModule, selectedNode])

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
    setSelectedComponent(nodeId)
    const node = concept.graphRecord?.graph.nodes.find((item) => item.node_id === nodeId)
    if (node) setSelectedLibraryModuleId(node.module_id)
  }, [concept.graphRecord])

  const selectLibraryModule = useCallback((module: ModuleAssetRecord) => {
    const moduleId = module.manifest.module_id
    setSelectedLibraryModuleId(moduleId)
    setRecentModuleIds((current) => [moduleId, ...current.filter((item) => item !== moduleId)].slice(0, 12))
    const graphNode = concept.graphRecord?.graph.nodes.find((node) => node.module_id === moduleId)
    if (graphNode && !componentDrawerOpen) setSelectedComponent(graphNode.node_id)
  }, [componentDrawerOpen, concept.graphRecord])

  const toggleLibraryFavorite = useCallback((moduleId: string) => {
    setFavoriteModuleIds((current) => (
      current.includes(moduleId)
        ? current.filter((item) => item !== moduleId)
        : [...current, moduleId]
    ))
  }, [])

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

  const focusQualityFinding = useCallback((finding: QualityFinding) => {
    const validNodeIds = (finding.node_ids ?? []).filter((candidate) => (
      concept.graphRecord?.graph.nodes.some((node) => node.node_id === candidate)
    ))
    const nodeId = validNodeIds[0]
    if (!nodeId) return
    selectGraphNode(nodeId)
    setFocusedNodeId(nodeId)
    setQualityHighlightNodeIds(validNodeIds)
    setQualityGeometryRefs(
      (finding.geometry_refs ?? []).filter((reference) => validNodeIds.includes(reference.node_id)),
    )
  }, [concept.graphRecord, selectGraphNode])

  useEffect(() => {
    setQualityHighlightNodeIds([])
    setQualityGeometryRefs([])
  }, [concept.qualityRun?.quality_run_id, concept.version?.version_id])

  const handleCreateExport = useCallback(async () => {
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
  }, [concept, exportFormat])

  const downloadExistingExport = useCallback((url: string, filename: string) => {
    downloadBrowserFile(url, filename).catch((caught) => {
      setAssistantNote(`浏览器下载失败：${errorText(caught)}`)
    })
  }, [])

  const handleReplaceSelected = useCallback(() => {
    if (!selectedNode || !selectedLibraryModule) return
    concept.previewModuleReplacement(selectedNode.node_id, selectedLibraryModule.manifest.module_id)
      .catch(() => undefined)
  }, [concept, selectedLibraryModule, selectedNode])

  const openComponentReplacement = useCallback(() => {
    if (!selectedNode || selectedNode.locked) return
    setComponentCategory('compatible')
    setComponentQuery('')
    setReviewStatusFilter('')
    setComponentDrawerMode('recommended')
    setDrawerExpanded(false)
    setComponentDrawerOpen(true)
  }, [selectedNode])

  const handleToggleMirrorX = useCallback(() => {
    if (!selectedNode) return
    const nextAxis = selectedNode.mirror_axis === 'x' ? 'none' : 'x'
    concept.setMirror(selectedNode.node_id, nextAxis).catch(() => undefined)
  }, [concept, selectedNode])

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
    if (!selectedNode) return
    concept.previewNodeTransform(selectedNode.node_id, transformDraft).catch(() => undefined)
  }, [concept, selectedNode, transformDraft])

  const previewQuickTransform = useCallback((action: 'smaller' | 'larger' | 'forward' | 'backward' | 'rotateLeft' | 'rotateRight') => {
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
  }, [concept, selectedNode])

  const handleTransformCommit = useCallback((nodeId: string, transform: Transform) => {
    setTransformDraft(copyTransform(transform))
    concept.previewNodeTransform(nodeId, transform).catch(() => undefined)
  }, [concept.previewNodeTransform])

  const handleMeasurePoint = useCallback((point: ViewportMeasurementPoint) => {
    setSelectedComponent(point.nodeId)
    setMeasurementPoints((current) => current.length >= 2 ? [point] : [...current, point])
  }, [])

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
        setActiveTool('move')
      } else if (event.key === 'r' || event.key === 'R') {
        event.preventDefault()
        setActiveTool('rotate')
      } else if (event.key === 's' || event.key === 'S') {
        event.preventDefault()
        setActiveTool('scale')
      } else if (event.key === 'Escape' && concept.pendingManualChange) {
        event.preventDefault()
        concept.discardManualChange().catch(() => undefined)
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [concept.discardManualChange, concept.pendingManualChange])

  const toggleSelectedNodeVisibility = useCallback(() => {
    if (!selectedNode) return
    setHiddenNodeIds((current) => (
      current.includes(selectedNode.node_id)
        ? current.filter((nodeId) => nodeId !== selectedNode.node_id)
        : [...current, selectedNode.node_id]
    ))
  }, [selectedNode])

  const handleModuleDrop = useCallback((nodeId: string, moduleId: string) => {
    selectGraphNode(nodeId)
    setSelectedLibraryModuleId(moduleId)
    setAssistantNote(`已将 ${moduleId} 设为 ${nodeId} 的替换候选；点击“替换并创建新版本”后才会提交 ChangeSet。`)
  }, [selectGraphNode])

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

  const submitAssistantInstruction = async () => {
    const instruction = chatInput.trim() || DEFAULT_CONCEPT_BRIEF
    setAssistantNote(`正在解释 Brief：“${instruction}”`)
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
      + `${provenance.fallback_used ? ' · Provider 失败后已显式降级' : ''}。`,
    )
    setChatInput('')
  }

  const previewChangeInstruction = async () => {
    const instruction = chatInput.trim()
    if (!instruction) return
    setAssistantNote(`正在规划修改：“${instruction}”`)
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
      `已生成 ${result.planned.change_set.operations.length} 个受限操作；当前仅为幽灵预览。`,
    )
    setChatInput('')
  }

  const runAssistantAction = () => (
    assistantMode === 'brief'
      ? submitAssistantInstruction()
      : previewChangeInstruction()
  )

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
            className="text-action"
            onClick={() => undoVersionId && concept.selectVersion(undoVersionId)}
            disabled={!undoVersionId || concept.loading}
            title="返回上一个已确认版本"
          ><ClockCounterClockwise size={16} /> 撤销</button>
          <button className="text-action" onClick={() => setQualityOpen(true)}><Check size={16} /> 检查</button>
          <button className="export-action" onClick={() => setExportOpen(true)}><Export size={16} /> 导出</button>
        </div>
      </header>

      <div className="cad-layout">
        <aside className="cad-left-rail">
          <section className="cad-panel assistant-panel agent-first-panel">
            <div className="cad-panel-title">
              <span><Sparkle size={16} weight="fill" /> 设计助手</span>
              <span className="assistant-state">
                {concept.loading ? '正在工作' : '准备就绪'}
              </span>
            </div>
            {!concept.project && !concept.loading && (
              <button className="empty-action" onClick={() => concept.createStarterProject()}>
                <Plus size={14} /> 创建第一个设计
              </button>
            )}
            {concept.project && !concept.version?.module_graph_id && (
              <button className="empty-action" onClick={() => concept.initializeCurrentProject()} disabled={concept.loading}>
                <Cube size={14} /> 准备展示组件
              </button>
            )}
            <p className="agent-welcome">用一句话描述你想要的虚构展示道具；我会生成可预览、可继续修改的方向。</p>
            <div className="assistant-composer agent-composer">
              <ChatCircleDots size={17} />
              <input
                value={chatInput}
                onChange={(event) => setChatInput(event.target.value)}
                onKeyDown={(event) => event.key === 'Enter' && runAssistantAction()}
                placeholder={assistantMode === 'change' && selectedNode ? `告诉我怎么调整这个${selectedModuleLabel}…` : '描述你想设计的道具…'}
              />
              <button onClick={runAssistantAction} aria-label="发送设计需求" disabled={concept.loading}>
                <PaperPlaneRight size={16} weight="fill" />
              </button>
            </div>
            <div className="concept-family-suggestions" aria-label="概念家族">
              <span>从一个方向开始</span>
              <div>
                {CONCEPT_FAMILY_SUGGESTIONS.map(([label, prompt]) => (
                  <button key={label} type="button" onClick={() => { setAssistantMode('brief'); setChatInput(prompt) }}>{label}</button>
                ))}
              </div>
            </div>
            {selectedNode && (
              <button
                type="button"
                className={`agent-selection-context ${assistantMode === 'change' ? 'active' : ''}`}
                onClick={() => setAssistantMode('change')}
              >正在调整：{selectedModuleLabel}</button>
            )}
            <div className={`assistant-message ${concept.error ? 'error' : ''}`}>{concept.error ?? assistantNote}</div>
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
                  onClick={() => setActiveTool(tool.id)}
                />
              ))}
            </div>
            <ModuleGraphViewport
              graphRecord={concept.graphRecord}
              modules={concept.modules}
              cameraView={cameraView}
              showGrid={showGrid}
              wireframe={wireframe}
              xRay={xRay}
              sectionEnabled={activeTool === 'section'}
              sectionOffset={sectionOffset}
              selectedNodeId={selectedComponent}
              hiddenNodeIds={hiddenNodeIds}
              focusNodeId={focusedNodeId}
              qualityHighlightNodeIds={qualityHighlightNodeIds}
              qualityGeometryRefs={qualityGeometryRefs}
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
                  <button className={measurementMode === 'distance' ? 'active' : ''} onClick={() => { setMeasurementMode('distance'); setMeasurementPoints([]) }}>距离</button>
                  <button className={measurementMode === 'normal_angle' ? 'active' : ''} onClick={() => { setMeasurementMode('normal_angle'); setMeasurementPoints([]) }}>法线夹角</button>
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
                <input aria-label="截面偏移" type="range" min="-120" max="120" step="1" value={sectionOffset} onChange={(event) => setSectionOffset(Number(event.target.value))} />
              </label>
            )}
            <div className="view-cube"><Cube size={28} weight="duotone" /></div>
            <div className="viewport-viewbar">
              <IconButton icon={House} label="等轴" active={cameraView === 'iso'} onClick={() => setCameraView('iso')} />
              <IconButton icon={Crosshair} label="正视" active={cameraView === 'front'} onClick={() => setCameraView('front')} />
              <IconButton icon={GridFour} label="顶视" active={cameraView === 'top'} onClick={() => setCameraView('top')} />
              <IconButton icon={Cube} label="右视" active={cameraView === 'right'} onClick={() => setCameraView('right')} />
              <IconButton
                icon={ArrowsOutCardinal}
                label="爆炸视图"
                active={explodeFactor > 0}
                onClick={() => setExplodeFactor((current) => current > 0 ? 0 : 0.42)}
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
                  <button type="button" onClick={() => setSelectedComponent('')} aria-label="关闭部件编辑">×</button>
                </div>
                <div className="contextual-edit-actions">
                  <button type="button" onClick={openComponentReplacement} disabled={selectedNode.locked || concept.loading}>替换</button>
                  <button type="button" onClick={() => { setAssistantMode('change'); setAssistantNote(`告诉我如何调整这个${selectedModuleLabel}。`) }} disabled={selectedNode.locked}>让 Agent 调整</button>
                  <button type="button" onClick={toggleSelectedNodeVisibility}>{hiddenNodeIds.includes(selectedNode.node_id) ? '显示' : '隐藏'}</button>
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

          {componentDrawerOpen && <section className={`component-library contextual-library ${componentDrawerMode === 'all' ? 'expanded' : ''}`}>
            <div
              className="component-library-resize-handle"
              onPointerDown={beginDrawerResize}
              aria-hidden="true"
            />
            <div className="component-library-header">
              <div className="component-library-title">
                <Cube size={15} weight="duotone" />
                {componentDrawerMode === 'recommended'
                  ? `替换「${selectedModuleLabel}」`
                  : '选择展示组件'}
              </div>
              <div className="component-library-header-actions">
                {componentDrawerMode === 'all' && <>
                  <select
                    className="component-status-filter"
                    aria-label="部件审阅状态"
                    value={reviewStatusFilter}
                    onChange={(event) => setReviewStatusFilter(event.target.value as ReviewStatus | '')}
                  >
                    <option value="">全部状态</option>
                    {Object.entries(REVIEW_STATUS_LABELS).map(([status, label]) => (
                      <option key={status} value={status}>{label}</option>
                    ))}
                  </select>
                  <div className="component-search">
                    <MagnifyingGlass size={15} />
                    <input
                      value={componentQuery}
                      onChange={(event) => setComponentQuery(event.target.value)}
                      placeholder="搜索名称、描述或标签…"
                    />
                    <Funnel size={14} />
                  </div>
                </>}
                <button
                  type="button"
                  className="component-drawer-toggle text"
                  onClick={() => setComponentDrawerMode((current) => current === 'recommended' ? 'all' : 'recommended')}
                >
                  {componentDrawerMode === 'recommended' ? '查看更多' : '返回推荐'}
                </button>
                <button type="button" className="component-drawer-toggle" onClick={() => setComponentDrawerOpen(false)} aria-label="关闭组件选择">×</button>
              </div>
            </div>
            <div className="component-library-body">
              <>
                {componentDrawerMode === 'all' &&
                  <nav className="component-categories">
                    {COMPONENT_CATEGORIES.map((category) => (
                      <button
                        key={category.id}
                        className={componentCategory === category.id ? 'active' : ''}
                        onClick={() => setComponentCategory(category.id)}
                      >
                        <span>{category.label}</span>
                        <small>{
                          category.id === 'all'
                            ? componentFilterCounts.all
                            : category.id === 'installed'
                            ? componentFilterCounts.installed
                            : category.id === 'compatible'
                            ? componentFilterCounts.compatible
                            : category.id === 'favorites'
                            ? componentFilterCounts.favorites
                            : category.id === 'recent'
                            ? componentFilterCounts.recent
                            : concept.modules.filter((module) => module.manifest.category === category.id).length
                        }</small>
                      </button>
                    ))}
                  </nav>
                }
                  <div className="component-library-content">
                    <div className="module-replace-bar">
                      <span>{componentDrawerMode === 'recommended' ? '只显示当前部件可用的替换建议' : '选择一个组件后，可先预览再决定保留'}</span>
                      <span className="component-result-count">显示 {displayedComponents.length} / {concept.modules.length}</span>
                      <button
                        onClick={handleReplaceSelected}
                        disabled={!canReplaceSelected || concept.loading}
                        title={selectedNode?.locked
                          ? '锁定节点不能替换'
                          : selectedLibraryModule?.catalog_metadata.review_status === 'restricted'
                          ? '受限资产不能用于替换'
                          : qualityStatusFor(selectedLibraryModule?.manifest.module_id ?? '') === 'failed'
                          ? '质量检查失败的资产不能用于替换'
                          : '通过 ChangeSet preview/confirm 创建子版本'}
                      >
                        预览替换
                      </button>
                    </div>
            <div className="component-grid">
                      {displayedComponents.map((component) => {
                        const ComponentIcon = componentIconFor(component.manifest.category)
                        const graphNode = concept.graphRecord?.graph.nodes.find(
                          (node) => node.module_id === component.manifest.module_id,
                        )
                        const isActiveNode = Boolean(graphNode && selectedComponent === graphNode.node_id)
                        const isCandidate = selectedLibraryModuleId === component.manifest.module_id
                        const isInstalled = Boolean(graphNode)
                        const compatible = Boolean(
                          selectedNode
                          && !selectedNode.locked
                          && selectedModule?.manifest.category === component.manifest.category,
                        )
                        const metadata = component.catalog_metadata
                        const reviewStatus = metadata.review_status as ReviewStatus
                        const qualityStatus = qualityStatusFor(component.manifest.module_id)
                        const thumbnailFailed = thumbnailFailures.has(component.manifest.module_id)
                        return (
                          <button
                            key={component.manifest.module_id}
                            className={`component-card ${isActiveNode ? 'active' : ''} ${isCandidate ? 'candidate' : ''}`.trim()}
                            draggable
                            onDragStart={(event) => {
                              event.dataTransfer.effectAllowed = 'copy'
                              event.dataTransfer.setData('application/x-forgecad-module-id', component.manifest.module_id)
                              event.dataTransfer.setData('text/plain', component.manifest.module_id)
                            }}
                            onClick={() => {
                              selectLibraryModule(component)
                            }}
                          >
                            <span className="component-visual">
                              {!thumbnailFailed && <img
                                src={forgeApi.getModuleAssetThumbnailUrl(component.manifest.module_id)}
                                alt={`${component.manifest.module_id} 模块缩略图`}
                                onError={() => setThumbnailFailures((current) => new Set(current).add(component.manifest.module_id))}
                              />}
                              <span className="component-icon-fallback" hidden={!thumbnailFailed}>
                                <ComponentIcon size={34} weight="duotone" />
                              </span>
                              <span className={`component-state ${reviewStatus}`}>
                                {REVIEW_STATUS_LABELS[reviewStatus]}
                              </span>
                            </span>
                            <strong>{metadata.display_name}</strong>
                            <small>
                              {MODULE_CATEGORY_LABELS[component.manifest.category]} · {component.manifest.triangle_count.toLocaleString()} tris · {(component.manifest.connectors ?? []).length} 接口
                            </small>
                            <span className="component-card-activity">
                              {isActiveNode ? '当前节点' : isCandidate ? '替换候选' : isInstalled ? '已装配' : compatible ? '可替换' : QUALITY_STATUS_LABELS[qualityStatus]}
                            </span>
                          </button>
                        )
                      })}
                      {displayedComponents.length === 0 && (
                        <div className="component-empty">
                          <strong>暂时没有可直接替换的组件</strong>
                          <span>当前组件库没有同类、兼容且可用的展示组件；你可以继续用 Agent 调整，或稍后添加原创组件。</span>
                        </div>
                      )}
                    </div>
                  </div>
                  {drawerExpanded && selectedLibraryModule && (
                    <aside className="component-inspector" data-testid="component-inspector">
                      <div className="component-inspector-visual">
                        {!thumbnailFailures.has(selectedLibraryModule.manifest.module_id) && (
                          <img
                            src={forgeApi.getModuleAssetThumbnailUrl(selectedLibraryModule.manifest.module_id)}
                            alt={`${selectedLibraryModule.catalog_metadata.display_name} 预览`}
                            onError={() => setThumbnailFailures((current) => new Set(current).add(selectedLibraryModule.manifest.module_id))}
                          />
                        )}
                        <span>{selectedLibraryModule.catalog_metadata.catalog_path}</span>
                      </div>
                      <div className="component-inspector-heading">
                        <div>
                          <strong>{selectedLibraryModule.catalog_metadata.display_name}</strong>
                          <span>{selectedLibraryModule.manifest.module_id}</span>
                        </div>
                        <button
                          type="button"
                          className={favoriteModuleIds.includes(selectedLibraryModule.manifest.module_id) ? 'active' : ''}
                          onClick={() => toggleLibraryFavorite(selectedLibraryModule.manifest.module_id)}
                          aria-label="切换组件收藏"
                          title="切换组件收藏"
                        >
                          <Star size={16} weight={favoriteModuleIds.includes(selectedLibraryModule.manifest.module_id) ? 'fill' : 'regular'} />
                        </button>
                      </div>
                      <p>{selectedLibraryModule.catalog_metadata.description}</p>
                      <div className="component-inspector-statuses">
                        <span className={`review-status ${selectedLibraryModule.catalog_metadata.review_status}`}>
                          {REVIEW_STATUS_LABELS[selectedLibraryModule.catalog_metadata.review_status as ReviewStatus]}
                        </span>
                        <span className={`quality-status ${qualityStatusFor(selectedLibraryModule.manifest.module_id)}`}>
                          质量：{QUALITY_STATUS_LABELS[qualityStatusFor(selectedLibraryModule.manifest.module_id)]}
                        </span>
                      </div>
                      <dl className="component-spec-list">
                        <div><dt>尺寸</dt><dd>{selectedLibraryModule.manifest.bounds_mm.map((value) => `${value} mm`).join(' × ')}</dd></div>
                        <div><dt>几何</dt><dd>{selectedLibraryModule.manifest.triangle_count.toLocaleString()} tris · {selectedLibraryModule.manifest.material_slots.length} 材质槽</dd></div>
                        <div><dt>连接器</dt><dd>{(selectedLibraryModule.manifest.connectors ?? []).length} 个 · {(selectedLibraryModule.manifest.connectors ?? []).map((item) => item.slot).join('、') || '无'}</dd></div>
                        <div><dt>适配</dt><dd>{canReplaceSelected ? '可替换当前节点' : selectedNode?.locked ? '当前节点已锁定' : '选择同分类节点后验证'}</dd></div>
                        <div><dt>来源</dt><dd>{ORIGIN_CLAIM_LABELS[selectedLibraryModule.catalog_metadata.origin_claim ?? 'unknown']}</dd></div>
                        <div><dt>审阅</dt><dd>{selectedLibraryModule.catalog_metadata.reviewer_name ? `${selectedLibraryModule.catalog_metadata.reviewer_name} · ${selectedLibraryModule.catalog_metadata.reviewed_at ? `已记录 ${selectedLibraryModule.catalog_metadata.reviewed_at}` : '已指派，待完成'}` : '等待独立审阅'}</dd></div>
                      </dl>
                      {(selectedLibraryModule.catalog_metadata.tags ?? []).length > 0 && (
                        <div className="component-tags">
                          {(selectedLibraryModule.catalog_metadata.tags ?? []).map((tag) => <span key={tag}>#{tag}</span>)}
                        </div>
                      )}
                      <div className="component-inspector-actions">
                        <button
                          type="button"
                          onClick={() => {
                            const node = concept.graphRecord?.graph.nodes.find((item) => item.module_id === selectedLibraryModule.manifest.module_id)
                            if (node) selectGraphNode(node.node_id)
                            else setAssistantNote('候选资产已选中；主视图只显示已确认版本，替换会先创建 ChangeSet 预览。')
                          }}
                        >{concept.graphRecord?.graph.nodes.some((item) => item.module_id === selectedLibraryModule.manifest.module_id) ? '定位主视图' : '设置替换候选'}</button>
                        <button
                          type="button"
                          className="primary"
                          disabled={!canReplaceSelected || concept.loading}
                          onClick={handleReplaceSelected}
                        >预览替换</button>
                      </div>
                      {concept.pendingReplacement && (
                        <div className="component-replacement-preview" data-testid="component-replacement-preview">
                          <span>幽灵预览已就绪，当前版本尚未改动。</span>
                          <button type="button" onClick={() => concept.discardModuleReplacement()} disabled={concept.loading}>放弃</button>
                          <button type="button" className="confirm" onClick={() => concept.confirmModuleReplacement()} disabled={concept.loading}>确认并创建新版本</button>
                        </div>
                      )}
                    </aside>
                  )}
              </>
            </div>
          </section>
          }
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
                <button key={id} className={inspectorTab === id ? 'active' : ''} onClick={() => setInspectorTab(id)}>
                  {label}
                </button>
              ))}
            </nav>
            {inspectorTab === 'parameters' && <>
              <label className="wide-field"><span>Graph 节点</span><input value={selectedNode?.node_id ?? '未选择'} readOnly /></label>
              <label className="wide-field"><span>模块资产</span><input value={selectedModule?.manifest.module_id ?? '—'} readOnly /></label>
              <div className="node-actions">
                <button onClick={toggleSelectedNodeVisibility} disabled={!selectedNode}>
                  <Eye size={13} /> {selectedNode && hiddenNodeIds.includes(selectedNode.node_id) ? '显示' : '隐藏'}
                </button>
                <button onClick={() => selectedNode && setFocusedNodeId(selectedNode.node_id)} disabled={!selectedNode}>
                  <Crosshair size={13} /> 聚焦
                </button>
                <button className={showConnectors ? 'active' : ''} onClick={() => setShowConnectors((current) => !current)}>
                  <ShareNetwork size={13} /> Connector
                </button>
                <button
                  className={selectedNode?.mirror_axis === 'x' ? 'active' : ''}
                  onClick={handleToggleMirrorX}
                  disabled={!selectedNode || selectedNode.locked || concept.loading}
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
                  onClick={() => setTransformSpace((current) => current === 'world' ? 'local' : 'world')}
                  disabled={!selectedNode || selectedNode.locked || concept.loading}
                >{transformSpace === 'world' ? '世界坐标' : '本地坐标'}</button>
                <button
                  className={snapEnabled ? 'active' : ''}
                  onClick={() => setSnapEnabled((current) => !current)}
                  disabled={!selectedNode || selectedNode.locked || concept.loading}
                >{snapEnabled ? '吸附：1 mm / 15°' : '吸附：关'}</button>
              </div>
              <button
                className="transform-preview-action"
                onClick={previewTransformDraft}
                disabled={!selectedNode || selectedNode.locked || selectedNode.node_id === concept.graphRecord?.graph.root_node_id || concept.loading || Boolean(concept.pendingPreview)}
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
            {inspectorTab === 'appearance' && <>
              <label className="wide-field"><span>表面主题</span><select defaultValue="arctic"><option value="arctic">寒地石墨</option><option value="industrial">工业枪灰</option><option value="prototype">原型树脂</option></select></label>
              <PropertyNumber label="细节密度" value={parameters.detailDensity} unit="%" onChange={(value) => updateParameter('detailDensity', value)} />
              <div className="appearance-swatches"><button aria-label="石墨黑" /><button aria-label="枪灰" /><button aria-label="信号红" /></div>
            </>}
            {inspectorTab === 'connections' && <div className="connection-list">
              <div className="connection-summary">
                <span>{selectedNodeConnections.length} 条真实连接</span>
                <button
                  onClick={() => selectedNode && concept.previewConnectorSnap(selectedNode.node_id)}
                  disabled={!canSnapSelectedNode || concept.loading || Boolean(concept.pendingPreview)}
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
                disabled={concept.loading || !concept.version?.module_graph_id}
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
                  <button onClick={() => concept.discardManualChange()} disabled={concept.loading}>放弃预览</button>
                  <button className="confirm" onClick={() => concept.confirmManualChange()} disabled={concept.loading}>确认并创建新版本</button>
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
            {concept.pendingChange && <button onClick={() => concept.discardPlannedChange()} disabled={concept.loading}>撤销本次修改</button>}
            {concept.pendingManualChange && <button onClick={() => concept.discardManualChange()} disabled={concept.loading}>撤销本次修改</button>}
            {concept.pendingReplacement && <button onClick={() => concept.discardModuleReplacement()} disabled={concept.loading}>撤销本次修改</button>}
            {concept.pendingChange && <button className="confirm" onClick={() => concept.confirmPlannedChange()} disabled={concept.loading}>保留此修改</button>}
            {concept.pendingManualChange && <button className="confirm" onClick={() => concept.confirmManualChange()} disabled={concept.loading}>保留此修改</button>}
            {concept.pendingReplacement && <button className="confirm" onClick={() => concept.confirmModuleReplacement()} disabled={concept.loading}>保留此修改</button>}
          </div>
        </section>
      )}

      {exportOpen && (
        <div className="workbench-overlay" role="presentation" onMouseDown={() => setExportOpen(false)}>
          <section className="workbench-drawer export-drawer" role="dialog" aria-label="导出设计" onMouseDown={(event) => event.stopPropagation()}>
            <div className="drawer-heading"><div><span>导出当前设计</span><strong>你准备如何使用它？</strong></div><button onClick={() => setExportOpen(false)} aria-label="关闭导出">×</button></div>
            <div className="export-purpose-list">
              {EXPORT_PURPOSES.map((purpose) => (
                <button
                  key={purpose.id}
                  className={exportPurpose === purpose.id ? 'active' : ''}
                  onClick={() => { setExportPurpose(purpose.id); setExportFormat(purpose.format) }}
                >
                  <strong>{purpose.title}</strong><span>{purpose.description}</span>
                </button>
              ))}
            </div>
            <div className="export-ready-summary">
              <span>将导出：<strong>{exportPurpose === 'presentation' ? '展示图像' : exportPurpose === 'production' ? 'GLB 展示模型' : exportPurpose === 'handoff' ? 'OBJ 模型' : '概念源包'}</strong></span>
              <small>{concept.version ? `当前版本 v${activeVersionSummary?.version_no ?? '—'} · ${ORIGIN_CLAIM_LABELS[selectedModule?.catalog_metadata.origin_claim ?? 'unknown']}` : '请先创建或打开一个设计'}</small>
            </div>
            <button className="drawer-primary-action" onClick={handleCreateExport} disabled={!concept.version?.module_graph_id || concept.loading}>
              <FileArrowDown size={16} /> 导出当前版本
            </button>
            <button className="drawer-secondary-action" onClick={() => setExportOpen(false)}>取消</button>
          </section>
        </div>
      )}

      {qualityOpen && (
        <div className="workbench-overlay" role="presentation" onMouseDown={() => setQualityOpen(false)}>
          <section className="workbench-drawer quality-drawer" role="dialog" aria-label="模型检查" onMouseDown={(event) => event.stopPropagation()}>
            <div className="drawer-heading"><div><span>模型检查</span><strong>{qualityStatusLabel(concept.qualityRun?.report.status)}</strong></div><button onClick={() => setQualityOpen(false)} aria-label="关闭模型检查">×</button></div>
            <p>这里仅在需要时显示模型连接和几何质量信息，不会干扰设计过程。</p>
            <div className="quality-overview">
              <DfmRow label="展示组件已加载" value={concept.graphRecord ? '已就绪' : '未准备'} ok={Boolean(concept.graphRecord)} />
              <DfmRow label="当前版本" value={concept.version ? '可保存' : '未创建'} ok={Boolean(concept.version)} />
              <DfmRow label="几何检查" value={qualityStatusLabel(concept.qualityRun?.report.status)} ok={concept.qualityRun?.report.status === 'passed'} />
            </div>
            {(concept.qualityRun?.report.findings ?? []).slice(0, 3).map((finding) => (
              <button type="button" className={`quality-finding ${finding.severity}`} key={finding.finding_id} onClick={() => { focusQualityFinding(finding); setQualityOpen(false) }}>
                <strong>{finding.check_id}</strong><span>{finding.message}</span>
              </button>
            ))}
            <button className="drawer-primary-action" disabled={concept.loading || !concept.version?.module_graph_id} onClick={() => concept.runQualityInspection()}>
              {concept.loading ? '检查中…' : '运行模型检查'}
            </button>
          </section>
        </div>
      )}

      <footer className="cad-status-bar">
        <span>{concept.loading ? 'Agent 正在处理' : '设计就绪'}</span>
        <span>{selectedNode ? `正在调整：${selectedModuleLabel}` : '点击模型的任意部件即可调整'}</span>
        <span>版本：{activeVersionSummary ? `v${activeVersionSummary.version_no}` : '草稿'}</span>
        <span>单位：mm</span>
        <span className="status-spacer" />
        <span>{qualityStatusLabel(concept.qualityRun?.report.status)} · 模型检查</span>
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

function componentIconFor(category: ModuleCategory) {
  const icons: Record<ModuleCategory, typeof Cube> = {
    core_shell: SelectionAll,
    front_shell: Ruler,
    rear_shell: ArrowsClockwise,
    grip_shell: GridFour,
    top_accessory: ChartLineUp,
    side_accessory: Crosshair,
    lower_structure: ArrowsOutCardinal,
    storage_visual: Cube,
    armor_panel: SelectionAll,
  }
  return icons[category]
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
