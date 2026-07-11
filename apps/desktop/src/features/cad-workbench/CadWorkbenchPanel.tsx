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
  Gear,
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
  UserCircle,
  WarningCircle,
} from '@phosphor-icons/react'
import { forgeApi } from '../../shared/api/forgeApi'
import type { DesignChangeSet, ModuleAssetRecord, QualityFinding, Transform } from '../../shared/types'
import { ModuleGraphViewport, type ViewportMeasurementPoint } from './ModuleGraphViewport'
import {
  useConceptWorkbench,
  type ChangeSetTimelineFilters,
} from './useConceptWorkbench'
import './cad-workbench.css'

type WorkspaceTab = 'concept' | 'assembly' | 'refine' | 'inspect' | 'showcase'
type InspectorTab = 'parameters' | 'appearance' | 'connections' | 'inspection'
type DrawerTab = 'components' | 'variants' | 'versions' | 'timeline'
type Tool = 'select' | 'move' | 'rotate' | 'scale' | 'orbit' | 'measure' | 'section'
type CameraView = 'iso' | 'front' | 'top' | 'right'
type AssistantMode = 'brief' | 'change'

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
  { id: 'section', label: '截面', icon: SelectionAll, implemented: false, unavailableReason: '裁切平面尚未实现，避免显示为可用能力。' },
]

export function CadWorkbenchPanel({ onOpenLegacy }: { onOpenLegacy: () => void }) {
  const concept = useConceptWorkbench()
  const [activeTab, setActiveTab] = useState<WorkspaceTab>('concept')
  const [inspectorTab, setInspectorTab] = useState<InspectorTab>('parameters')
  const [drawerTab, setDrawerTab] = useState<DrawerTab>('components')
  const [activeTool, setActiveTool] = useState<Tool>('select')
  const [transformSpace, setTransformSpace] = useState<'world' | 'local'>('world')
  const [snapEnabled, setSnapEnabled] = useState(true)
  const [cameraView, setCameraView] = useState<CameraView>('iso')
  const [showGrid, setShowGrid] = useState(true)
  const [wireframe, setWireframe] = useState(false)
  const [xRay, setXRay] = useState(false)
  const [selectedComponent, setSelectedComponent] = useState('')
  const [selectedLibraryModuleId, setSelectedLibraryModuleId] = useState('')
  const [hiddenNodeIds, setHiddenNodeIds] = useState<string[]>([])
  const [focusedNodeId, setFocusedNodeId] = useState<string | null>(null)
  const [qualityHighlightNodeIds, setQualityHighlightNodeIds] = useState<string[]>([])
  const [qualityGeometryRefs, setQualityGeometryRefs] = useState<
    NonNullable<QualityFinding['geometry_refs']>
  >([])
  const [showConnectors, setShowConnectors] = useState(false)
  const [explodeFactor, setExplodeFactor] = useState(0)
  const [measurementPoints, setMeasurementPoints] = useState<ViewportMeasurementPoint[]>([])
  const [measurementAnnotations, setMeasurementAnnotations] = useState<MeasurementAnnotation[]>([])
  const [measurementMode, setMeasurementMode] = useState<'distance' | 'normal_angle'>('distance')
  const [componentCategory, setComponentCategory] = useState<ComponentFilter>('all')
  const [componentQuery, setComponentQuery] = useState('')
  const [reviewStatusFilter, setReviewStatusFilter] = useState<ReviewStatus | ''>('')
  const [drawerExpanded, setDrawerExpanded] = useState(false)
  const [drawerHeight, setDrawerHeight] = useState(368)
  const [favoriteModuleIds, setFavoriteModuleIds] = useState<string[]>([])
  const [recentModuleIds, setRecentModuleIds] = useState<string[]>([])
  const [thumbnailFailures, setThumbnailFailures] = useState<Set<string>>(() => new Set())
  const [timelineQuery, setTimelineQuery] = useState('')
  const [timelineStatus, setTimelineStatus] = useState<ChangeSetTimelineFilters['status']>('')
  const [timelineOperation, setTimelineOperation] = useState<
    ChangeSetTimelineFilters['operation']
  >('')
  const [chatInput, setChatInput] = useState('')
  const [assistantMode, setAssistantMode] = useState<AssistantMode>('brief')
  const [assistantNote, setAssistantNote] = useState(
    '输入修改要求后，系统将生成结构化 DesignChangeSet 预览；确认前不会覆盖当前版本。',
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

  const applyTimelineFilters = () => concept.searchTimeline({
    query: timelineQuery,
    status: timelineStatus,
    operation: timelineOperation,
  })

  const clearTimelineFilters = () => {
    setTimelineQuery('')
    setTimelineStatus('')
    setTimelineOperation('')
    concept.searchTimeline({ query: '', status: '', operation: '' })
  }

  const handleCreateAuditExport = async () => {
    const result = await concept.createChangeSetAuditExport({
      query: timelineQuery,
      status: timelineStatus,
      operation: timelineOperation,
    })
    if (result) {
      window.location.assign(
        forgeApi.getChangeSetAuditExportFileUrl(result.audit_export_id),
      )
    }
  }

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
    const nodes = concept.graphRecord?.graph.nodes ?? []
    if (nodes.length === 0) {
      setSelectedComponent('')
      setSelectedLibraryModuleId('')
      return
    }
    setSelectedComponent((current) => {
      const nextNode = nodes.find((node) => node.node_id === current) ?? nodes[0]
      setSelectedLibraryModuleId(nextNode.module_id)
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
    setDrawerExpanded(true)
    const graphNode = concept.graphRecord?.graph.nodes.find((node) => node.module_id === moduleId)
    if (graphNode) setSelectedComponent(graphNode.node_id)
  }, [concept.graphRecord])

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
    const instruction = chatInput.trim()
    if (!instruction) return
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
      `已生成 ${result.variants.length} 个注册表约束方案 · ${provenance.generator}`
      + `${provenance.fallback_used ? ' · Provider 失败后已显式降级' : ''}。`,
    )
    setDrawerTab('variants')
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
        <button className="cad-brand" onClick={onOpenLegacy} aria-label="返回迁移前工作台">
          <span className="cad-brand-mark"><Cube size={18} weight="fill" /></span>
          <span>ForgeCAD</span>
        </button>
        <div className="cad-file-actions" aria-label="文件操作">
          <IconAction icon={Plus} label="新建" onClick={() => concept.createStarterProject()} />
          <IconAction icon={FolderOpen} label="同步" onClick={() => concept.refresh()} />
          <IconAction icon={FloppyDisk} label="保存" onClick={() => setAssistantNote('参数仍是本地草稿；请通过 ChangeSet 确认后创建不可变新版本。')} />
          <IconAction
            icon={ClockCounterClockwise}
            label="撤销"
            onClick={() => undoVersionId && concept.selectVersion(undoVersionId)}
            disabled={!undoVersionId || concept.loading}
            title="切换到当前版本的 parent"
          />
          <IconAction
            icon={ArrowsClockwise}
            label="重做"
            onClick={() => redoVersionId && concept.selectVersion(redoVersionId)}
            disabled={!redoVersionId || concept.loading}
            title="切换到最近的 child version"
          />
        </div>
        <nav className="cad-mode-tabs" aria-label="工作模式">
          {([
            ['concept', '概念'],
            ['assembly', '组装'],
            ['refine', '精修'],
            ['inspect', '检查'],
            ['showcase', '展示'],
          ] as Array<[WorkspaceTab, string]>).map(([id, label]) => (
            <button
              key={id}
              className={activeTab === id ? 'active' : ''}
              onClick={() => setActiveTab(id)}
            >
              {label}
            </button>
          ))}
        </nav>
        <div className="cad-global-actions">
          <IconButton icon={ShareNetwork} label="共享" />
          <IconButton icon={Gear} label="设置" />
          <UserCircle size={25} weight="duotone" />
          <CaretDown size={14} />
        </div>
      </header>

      <div className="cad-layout">
        <aside className="cad-left-rail">
          <section className="cad-panel project-panel">
            <div className="cad-panel-heading">
              <div>
                <span className="eyebrow">项目</span>
                <strong>{concept.project?.name ?? '尚未创建 Concept Project'}</strong>
              </div>
              <CaretDown size={14} />
            </div>
            {concept.projects.length > 1 && (
              <label className="project-select">
                <span>切换项目</span>
                <select
                  value={concept.project?.project_id ?? ''}
                  onChange={(event) => concept.selectProject(event.target.value)}
                >
                  {concept.projects.map((project) => (
                    <option key={project.project_id} value={project.project_id}>{project.name}</option>
                  ))}
                </select>
              </label>
            )}
            <div className="version-heading">版本历史</div>
            <div className="version-list">
              {(concept.project?.versions ?? []).slice().reverse().map((version) => (
                <button
                  key={version.version_id}
                  className={concept.version?.version_id === version.version_id ? 'active' : ''}
                  onClick={() => concept.selectVersion(version.version_id)}
                >
                  <span>V{version.version_no}</span>
                  <strong>{version.summary}</strong>
                  <small>{formatVersionTime(version.created_at)}</small>
                </button>
              ))}
              {!concept.project && !concept.loading && (
                <button className="empty-action" onClick={() => concept.createStarterProject()}>
                  <Plus size={14} /> 创建“寒地巡逻 S1”
                </button>
              )}
            </div>
            {concept.project && !concept.version?.module_graph_id && (
              <button
                className="empty-action"
                onClick={() => concept.initializeCurrentProject()}
                disabled={concept.loading}
              >
                <Cube size={14} /> 安装内置组件并初始化工作台
              </button>
            )}
          </section>

          <section className="cad-panel assistant-panel">
            <div className="cad-panel-title">
              <span><Sparkle size={16} weight="fill" /> AI 设计助手</span>
              <span className="assistant-state">
                {concept.pendingChange?.planner_provenance.generator
                  ?? concept.brief?.planner_provenance.generator
                  ?? '待输入'}
              </span>
            </div>
            <div className="assistant-mode-tabs" aria-label="AI 设计助手模式">
              <button
                className={assistantMode === 'brief' ? 'active' : ''}
                onClick={() => setAssistantMode('brief')}
              >概念方案</button>
              <button
                className={assistantMode === 'change' ? 'active' : ''}
                onClick={() => setAssistantMode('change')}
              >修改预览</button>
            </div>
            <div className="assistant-message">{concept.error ?? assistantNote}</div>
            <div className={`concept-runtime-state ${concept.error ? 'error' : ''}`}>
              {concept.loading ? '同步中 · ' : ''}{concept.statusMessage}
            </div>
            <div className="assistant-suggestions">
              {assistantMode === 'brief' ? <>
                <button onClick={() => setChatInput('寒地工业、紧凑、精密细节、信号红点缀的非功能概念资产')}>紧凑精密</button>
                <button onClick={() => setChatInput('修长展示轮廓、未来工业、蓝色点缀的非功能影视道具')}>延展展示</button>
              </> : <>
                <button onClick={() => setChatInput('将选中节点替换为候选模块，整体长度调整为 218 mm，细节密度调整为 84%')}>替换并调比例</button>
                <button onClick={() => setChatInput('整体更紧凑，增加精密细节，并使用信号蓝点缀配色')}>紧凑与配色</button>
              </>}
            </div>
            <button
              className="secondary-action"
              disabled={!chatInput.trim() || concept.loading}
              onClick={() => runAssistantAction()}
            >
              {assistantMode === 'brief' ? '生成 A/B/C 方案' : '生成修改预览'}
            </button>
            {concept.pendingChange && concept.pendingPreview && (
              <div className="change-preview-card" data-testid="change-preview-card">
                <div>
                  <strong>幽灵预览 · 待确认</strong>
                  <span>{concept.pendingChange.change_set.summary}</span>
                </div>
                <ul>
                  {concept.pendingChange.change_set.operations.map((operation) => (
                    <li key={operation.operation_id}>
                      {formatChangeOperation(operation)}
                    </li>
                  ))}
                </ul>
                <small>
                  {concept.pendingChange.planner_provenance.provider_id}
                  {concept.pendingChange.planner_provenance.fallback_used ? ' · 已显式降级' : ''}
                </small>
                <div className="change-preview-actions">
                  <button
                    onClick={() => concept.discardPlannedChange()}
                    disabled={concept.loading}
                  >放弃预览</button>
                  <button
                    className="confirm"
                    onClick={() => concept.confirmPlannedChange()}
                    disabled={concept.loading}
                  >确认并创建新版本</button>
                </div>
              </div>
            )}
            <small className="planner-boundary">只引用注册 Module；修改先生成 ghost preview，确认后才创建子版本。</small>
          </section>

          <section className="cad-panel quick-parameters">
            <div className="cad-panel-title"><span><SlidersHorizontal size={16} /> 参数化输入</span></div>
            <ParameterInput
              label="整体长度"
              value={parameters.overallLength}
              unit="mm"
              onChange={(value) => updateParameter('overallLength', value)}
            />
            <ParameterInput
              label="前部长度"
              value={parameters.frontShellLength}
              unit="mm"
              onChange={(value) => updateParameter('frontShellLength', value)}
              disabled
              title="当前 ModuleGraph 不提供前部程序化拉伸；请用模块替换或 ChangeSet 调整整体规格。"
            />
            <ParameterInput
              label="握把角度"
              value={parameters.gripAngle}
              unit="°"
              onChange={(value) => updateParameter('gripAngle', value)}
            />
            <ParameterInput
              label="细节密度"
              value={parameters.detailDensity}
              unit="%"
              onChange={(value) => updateParameter('detailDensity', value)}
            />
            <button className="primary-action" onClick={previewParameterDraft} disabled={concept.loading}>
              生成参数 ChangeSet 预览
            </button>
          </section>

          <div className="assistant-composer">
            <ChatCircleDots size={17} />
            <input
              value={chatInput}
              onChange={(event) => setChatInput(event.target.value)}
              onKeyDown={(event) => event.key === 'Enter' && runAssistantAction()}
              placeholder="输入设计需求…"
            />
            <button onClick={runAssistantAction} aria-label="发送设计需求">
              <PaperPlaneRight size={16} weight="fill" />
            </button>
          </div>
        </aside>

        <main
          className="cad-center-stage"
          style={{ gridTemplateRows: `minmax(0, 1fr) ${drawerExpanded ? drawerHeight : 162}px` }}
        >
          <div className="viewport-shell">
            <div className="viewport-toolbar" aria-label="CAD 视口工具">
              {TOOL_ITEMS.map((tool) => (
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
              <span className="toolbar-divider" />
              <IconButton
                icon={GridFour}
                label="网格"
                active={showGrid}
                onClick={() => setShowGrid((current) => !current)}
              />
              <IconButton
                icon={Eye}
                label="线框"
                active={wireframe}
                onClick={() => setWireframe((current) => !current)}
              />
              <IconButton
                icon={Eye}
                label="X-Ray"
                active={xRay}
                onClick={() => setXRay((current) => !current)}
              />
              {(activeTool === 'move' || activeTool === 'rotate' || activeTool === 'scale') && <>
                <span className="toolbar-divider" />
                <button
                  className={transformSpace === 'world' ? 'viewport-tool-text active' : 'viewport-tool-text'}
                  onClick={() => setTransformSpace((current) => current === 'world' ? 'local' : 'world')}
                  aria-label="切换世界或本地坐标"
                  title="切换世界或本地坐标"
                >{transformSpace === 'world' ? '世界' : '本地'}</button>
                <button
                  className={snapEnabled ? 'viewport-tool-text active' : 'viewport-tool-text'}
                  onClick={() => setSnapEnabled((current) => !current)}
                  aria-label="切换变换吸附"
                  title="切换变换吸附"
                >吸附</button>
              </>}
            </div>
            <ModuleGraphViewport
              graphRecord={concept.graphRecord}
              modules={concept.modules}
              cameraView={cameraView}
              showGrid={showGrid}
              wireframe={wireframe}
              xRay={xRay}
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
              <span>{activeTool === 'measure' ? measurementDistance == null ? `测量：${measurementPoints.length}/2 点` : measurementMode === 'distance' ? `测量：${measurementDistance.toFixed(2)} mm` : `法线夹角：${measurementAngle?.toFixed(2)}°` : activeTool === 'move' || activeTool === 'rotate' || activeTool === 'scale' ? `${activeTool === 'move' ? '移动' : activeTool === 'rotate' ? '旋转' : '缩放'} · ${transformSpace === 'world' ? '世界坐标' : '本地坐标'}${snapEnabled ? ' · 吸附' : ''}` : `${activeTool} 工具已启用`}</span>
              <span>{xRay ? 'X-Ray' : '单位：mm'}</span>
            </div>
          </div>

          <section className={`component-library ${drawerExpanded ? 'expanded' : ''}`}>
            <div
              className="component-library-resize-handle"
              onPointerDown={beginDrawerResize}
              aria-hidden="true"
            />
            <div className="component-library-header">
              <nav className="drawer-tabs" aria-label="底部工作区">
                {([
                  ['components', '组件'],
                  ['variants', '方案'],
                  ['versions', '版本'],
                  ['timeline', '时间线'],
                ] as Array<[DrawerTab, string]>).map(([id, label]) => (
                  <button key={id} className={drawerTab === id ? 'active' : ''} onClick={() => setDrawerTab(id)}>
                    {label}
                  </button>
                ))}
              </nav>
              <div className="component-library-header-actions">
                {drawerTab === 'components' && (
                  <select
                    className="component-status-filter"
                    aria-label="组件审阅状态"
                    value={reviewStatusFilter}
                    onChange={(event) => setReviewStatusFilter(event.target.value as ReviewStatus | '')}
                  >
                    <option value="">全部状态</option>
                    {Object.entries(REVIEW_STATUS_LABELS).map(([status, label]) => (
                      <option key={status} value={status}>{label}</option>
                    ))}
                  </select>
                )}
                <div className="component-search">
                  <MagnifyingGlass size={15} />
                  <input
                    value={drawerTab === 'timeline' ? timelineQuery : componentQuery}
                    onChange={(event) => {
                      if (drawerTab === 'timeline') setTimelineQuery(event.target.value)
                      else setComponentQuery(event.target.value)
                    }}
                    onKeyDown={(event) => {
                      if (drawerTab === 'timeline' && event.key === 'Enter') {
                        applyTimelineFilters()
                      }
                    }}
                    placeholder={drawerTab === 'timeline' ? '搜索 ChangeSet…' : '搜索名称、描述或标签…'}
                  />
                  <Funnel size={14} />
                </div>
                <button
                  type="button"
                  className="component-drawer-toggle"
                  onClick={() => setDrawerExpanded((current) => !current)}
                  title={drawerExpanded ? '收起组件检视器' : '展开组件检视器'}
                  aria-label={drawerExpanded ? '收起组件检视器' : '展开组件检视器'}
                >
                  {drawerExpanded ? <CaretDown size={15} /> : <CaretUp size={15} />}
                </button>
              </div>
            </div>
            <div className="component-library-body">
              {drawerTab === 'components' ? (
                <>
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
                  <div className="component-library-content">
                    <div className="module-replace-bar">
                      <span>节点：{selectedNode?.node_id ?? '未选择'}</span>
                      <span>候选：{selectedLibraryModule?.catalog_metadata.display_name ?? '未选择'}</span>
                      <span className="component-result-count">显示 {visibleComponents.length} / {concept.modules.length}</span>
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
                      {visibleComponents.map((component) => {
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
                      {visibleComponents.length === 0 && (
                        <div className="component-empty">
                          <strong>Module Pack 为空</strong>
                          <span>先通过 ModuleAssetManifest 注册 GLB，工作台不会用假组件替代。</span>
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
                        <div><dt>审阅</dt><dd>{selectedLibraryModule.catalog_metadata.reviewer_name ? `${selectedLibraryModule.catalog_metadata.reviewer_name} · ${selectedLibraryModule.catalog_metadata.reviewed_at ?? '时间待补充'}` : '等待独立审阅'}</dd></div>
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
              ) : (
                <div className={`drawer-placeholder${drawerTab === 'timeline' ? ' timeline-drawer' : ''}`}>
                  {drawerTab === 'variants' && <>
                    <strong>候选方案</strong>
                    {concept.variants.map((variant) => (
                      <button
                        key={variant.variant_id}
                        data-variant-rank={variant.rank}
                        className={variant.status === 'selected' ? 'selected' : ''}
                        onClick={() => concept.selectVariant(variant.variant_id)}
                        title={variant.summary}
                      >
                        <strong>{variant.rank}. {variant.name}</strong>
                        <span>{variant.status} · {variant.planner_provenance.generator}</span>
                        <small>建议 Module：{variant.recommended_module_ids?.length ?? 0}</small>
                      </button>
                    ))}
                    {concept.variants.length === 0 && <span>当前项目尚无已持久化方案。</span>}
                  </>}
                  {drawerTab === 'versions' && <>
                    <strong>版本分支</strong>
                    {(concept.project?.versions ?? []).slice().reverse().map((version) => (
                      <button key={version.version_id} onClick={() => concept.selectVersion(version.version_id)}>
                        V{version.version_no} · {version.summary}
                      </button>
                    ))}
                  </>}
                  {drawerTab === 'timeline' && <>
                    <div className="timeline-heading">
                      <strong>ChangeSet 操作时间线</strong>
                      <select
                        aria-label="ChangeSet 状态筛选"
                        value={timelineStatus}
                        onChange={(event) => setTimelineStatus(
                          event.target.value as ChangeSetTimelineFilters['status'],
                        )}
                      >
                        <option value="">全部状态</option>
                        <option value="confirmed">confirmed</option>
                        <option value="rejected">rejected</option>
                        <option value="stale">stale</option>
                        <option value="previewed">previewed</option>
                        <option value="proposed">proposed</option>
                      </select>
                      <select
                        aria-label="ChangeSet 操作筛选"
                        value={timelineOperation}
                        onChange={(event) => setTimelineOperation(
                          event.target.value as ChangeSetTimelineFilters['operation'],
                        )}
                      >
                        <option value="">全部操作</option>
                        <option value="replace_module">replace_module</option>
                        <option value="set_mirror">set_mirror</option>
                        <option value="set_transform">set_transform</option>
                        <option value="add_module">add_module</option>
                        <option value="remove_module">remove_module</option>
                        <option value="connect">connect</option>
                        <option value="disconnect">disconnect</option>
                        <option value="set_style">set_style</option>
                        <option value="set_parameter">set_parameter</option>
                      </select>
                      <button onClick={applyTimelineFilters} disabled={concept.timelineLoading}>
                        查询
                      </button>
                      <button onClick={clearTimelineFilters} disabled={concept.timelineLoading}>
                        重置
                      </button>
                      <button
                        data-testid="change-set-audit-export"
                        onClick={() => handleCreateAuditExport().catch(() => undefined)}
                        disabled={concept.timelineLoading}
                        title="按当前筛选导出 JSONL/CSV、哈希清单与归档说明"
                      >
                        <FileArrowDown size={13} /> 导出审计 ZIP
                      </button>
                    </div>
                    {concept.lastAuditExport ? (
                      <div className="timeline-audit-summary" data-testid="change-set-audit-summary">
                        最近归档 · {concept.lastAuditExport.record_count} 条 ·{' '}
                        {concept.lastAuditExport.audit_export_id} · project_lifetime
                      </div>
                    ) : null}
                    <div className="timeline-items" aria-live="polite">
                      {concept.timeline.map((item) => (
                        <article
                          className={`timeline-item status-${item.status}`}
                          key={item.change_set.change_set_id}
                        >
                          <div>
                            {formatVersionTime(item.confirmed_at ?? item.updated_at)} ·{' '}
                            <b>{item.status}</b> · {item.change_set.change_set_id}
                          </div>
                          <div>
                            {item.change_set.operations.map((operation) => (
                              `${operation.op}${operation.node_id ? `(${operation.node_id})` : ''}`
                            )).join(' + ')}
                            {item.result_version_id ? ` → ${item.result_version_id}` : ''}
                          </div>
                          {item.actor_type === 'planner' ? (
                            <div className="timeline-planner-meta" data-testid="change-set-planner-meta">
                              planner · {item.planner_provenance?.provider_id ?? 'unknown provider'}
                              {item.planner_provenance?.fallback_used ? ' · fallback' : ''}
                              {item.planner_instruction ? ` · ${item.planner_instruction}` : ''}
                            </div>
                          ) : null}
                          {item.diagnostic ? (
                            <div className="timeline-diagnostic" data-testid="change-set-diagnostic">
                              {item.diagnostic.code} · {item.diagnostic.stage} ·{' '}
                              {item.diagnostic.message}
                              {(item.diagnostic.node_ids ?? []).length > 0
                                ? ` · nodes: ${(item.diagnostic.node_ids ?? []).join(', ')}`
                                : ''}
                            </div>
                          ) : null}
                        </article>
                      ))}
                      {concept.timeline.length === 0 ? (
                        <div className="timeline-empty">没有符合条件的持久化 ChangeSet。</div>
                      ) : null}
                    </div>
                    {concept.timelineNextCursor ? (
                      <button
                        className="timeline-more"
                        onClick={concept.loadMoreTimeline}
                        disabled={concept.timelineLoading}
                      >
                        {concept.timelineLoading ? '正在加载…' : '加载更多'}
                      </button>
                    ) : null}
                  </>}
                </div>
              )}
            </div>
          </section>
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

      <footer className="cad-status-bar">
        <span>{({ concept: '概念', assembly: '组装', refine: '精修', inspect: '检查', showcase: '展示' } as Record<WorkspaceTab, string>)[activeTab]}阶段</span>
        <span>选择：{selectedComponent || '无'}</span>
        <span>模型：{concept.graphRecord ? `${concept.graphRecord.graph.nodes.length} nodes` : '未绑定 ModuleGraph'}</span>
        <span>单位：mm</span>
        <span>网格：10 mm</span>
        <span className="status-spacer" />
        <span>右键：上下文菜单</span>
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
