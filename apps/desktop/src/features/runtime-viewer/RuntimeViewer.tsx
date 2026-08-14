import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import {
  ArrowClockwise,
  ArrowUUpLeft,
  ArrowUUpRight,
  ArrowsOut,
  Camera,
  CaretDown,
  CaretRight,
  Check,
  CheckCircle,
  CircleNotch,
  Copy,
  CornersOut,
  Cursor,
  Cube,
  DotsThree,
  Eye,
  EyeSlash,
  Export,
  FloppyDisk,
  FolderSimple,
  GearSix,
  GitBranch,
  Hand,
  Info,
  Lightbulb,
  LinkSimple,
  Lock,
  MagnifyingGlass,
  MapPin,
  Plus,
  Pulse,
  Ruler,
  SlidersHorizontal,
  Sparkle,
  Stack,
  Trash,
  WarningCircle,
  X,
} from '@phosphor-icons/react'
import * as THREE from 'three'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'

type ViewerProject = {
  project?: { project_id?: string; name?: string }
  record?: { head_snapshot_id?: string | null }
  versions?: unknown[]
  candidates?: Array<{
    candidate?: { candidate_id?: string; state?: string; quality_hard_gate_passed?: boolean }
      artifact?: {
      artifact_id?: string
      program_sha256?: string
      mime?: string
      part_ids?: string[]
      source_node_ids?: string[]
      material_zone_ids?: string[]
      part_bindings?: Array<{ part_id?: string; source_node_id?: string; material_zone_id?: string }>
      triangle_count?: number
      validator_status?: string
      uv_status?: string
      tangent_status?: string
    } | null
    quality?: QualityReport | null
    reference?: { reference?: ReferenceEvidence } | null
  }>
  head_snapshot?: unknown
}

type ArtifactBytes = {
  artifact_id?: string
  candidate_id?: string
  mime?: string
  bytes_base64?: string
  sha256?: string
}

type ReferenceEvidence = {
  reference_id?: string
  project_id?: string
  object_sha256?: string
  mime?: string
  width?: number
  height?: number
}

type QualityReport = {
  program_sha256?: string
  reference_id?: string | null
  reference_sha256?: string | null
  render_set_hash?: string
  comparison_report_hash?: string
  visual_status?: string
  hard_gate_passed?: boolean
}

type ViewerVisualEvidence = {
  reference_id?: string
  render_set_hash?: string
  comparison_report_hash?: string | null
  quality_report_hash?: string
  quality_report?: QualityReport
  comparison_report?: {
    status?: string
    metrics?: Record<string, number>
  } | null
}

type RenderPass = {
  mime?: string
  png_base64?: string
  sha256?: string
  pass?: string
}

const AOV_PASSES = ['beauty', 'silhouette', 'depth', 'normal', 'ao', 'part-id', 'material-id', 'wireframe', 'uv-stretch'] as const
type AovPass = typeof AOV_PASSES[number]
type CompareMode = 'split' | 'overlay' | 'flicker'

type VisualMetricKey =
  | 'silhouette_iou'
  | 'boundary_f1_4px'
  | 'bbox_edge_error'
  | 'centroid_error'
  | 'landmark_coverage'
  | 'landmark_nme'
  | 'region_median_iou'
  | 'critical_region_min_iou'

type WorkflowGateStatus = 'passed' | 'failed' | 'locked' | 'not-run'

type WorkflowGate = {
  status: WorkflowGateStatus
  failedMetrics: VisualMetricKey[]
  missingMetrics: VisualMetricKey[]
}

type VisualWorkflow = {
  currentStage: 'reference-canvas' | 'silhouette-blockout' | 'landmark-structure' | 'semantic-part-fill' | 'surface-detail' | 'uv-pbr' | 'final'
  gates: {
    silhouette: WorkflowGate
    structure: WorkflowGate
    form: WorkflowGate
    surfaceMaterialUnlocked: boolean
  }
}

type CorrectionIntent = {
  actionId: 'fit-silhouette' | 'fit-landmarks' | 'fit-regions' | 'review-surface'
  stage: VisualWorkflow['currentStage']
  failedMetrics: VisualMetricKey[]
  instruction: string
  constraints: string[]
}

const VISUAL_GATE_THRESHOLDS: Record<VisualMetricKey, { operator: '>=' | '<='; threshold: number }> = {
  silhouette_iou: { operator: '>=', threshold: 0.90 },
  boundary_f1_4px: { operator: '>=', threshold: 0.90 },
  bbox_edge_error: { operator: '<=', threshold: 0.02 },
  centroid_error: { operator: '<=', threshold: 0.02 },
  landmark_coverage: { operator: '>=', threshold: 0.8 },
  landmark_nme: { operator: '<=', threshold: 0.03 },
  region_median_iou: { operator: '>=', threshold: 0.85 },
  critical_region_min_iou: { operator: '>=', threshold: 0.85 },
}

const SILHOUETTE_GATE_METRICS: VisualMetricKey[] = ['silhouette_iou', 'boundary_f1_4px', 'bbox_edge_error', 'centroid_error']
const STRUCTURE_GATE_METRICS: VisualMetricKey[] = ['landmark_coverage', 'landmark_nme']
const FORM_GATE_METRICS: VisualMetricKey[] = ['region_median_iou', 'critical_region_min_iou']

function evaluateWorkflowGate(metrics: Record<string, unknown>, keys: VisualMetricKey[]): WorkflowGate {
  const missingMetrics = keys.filter((key) => typeof metrics[key] !== 'number' || !Number.isFinite(metrics[key] as number))
  const failedMetrics = keys.filter((key) => {
    const value = metrics[key]
    if (typeof value !== 'number' || !Number.isFinite(value)) return false
    const target = VISUAL_GATE_THRESHOLDS[key]
    return target.operator === '>=' ? value < target.threshold : value > target.threshold
  })
  return {
    status: missingMetrics.length > 0 ? 'not-run' : failedMetrics.length > 0 ? 'failed' : 'passed',
    failedMetrics,
    missingMetrics,
  }
}

function lockWorkflowGate(gate: WorkflowGate): WorkflowGate {
  return { ...gate, status: 'locked' }
}

function deriveVisualWorkflow(metrics: Record<string, unknown>, hardGatePassed: boolean): VisualWorkflow {
  const silhouette = evaluateWorkflowGate(metrics, SILHOUETTE_GATE_METRICS)
  const structureDirect = evaluateWorkflowGate(metrics, STRUCTURE_GATE_METRICS)
  const formDirect = evaluateWorkflowGate(metrics, FORM_GATE_METRICS)
  const structure = silhouette.status === 'passed' ? structureDirect : lockWorkflowGate(structureDirect)
  const form = structure.status === 'passed' ? formDirect : lockWorkflowGate(formDirect)
  let currentStage: VisualWorkflow['currentStage'] = 'reference-canvas'
  const hasMetric = [...SILHOUETTE_GATE_METRICS, ...STRUCTURE_GATE_METRICS, ...FORM_GATE_METRICS].some((key) => typeof metrics[key] === 'number' && Number.isFinite(metrics[key] as number))
  if (!hasMetric) currentStage = 'reference-canvas'
  else if (silhouette.status !== 'passed') currentStage = 'silhouette-blockout'
  else if (structure.status !== 'passed') currentStage = 'landmark-structure'
  else if (form.status !== 'passed') currentStage = 'semantic-part-fill'
  else if (!hardGatePassed) currentStage = 'uv-pbr'
  else currentStage = 'final'
  return {
    currentStage,
    gates: {
      silhouette,
      structure,
      form,
      surfaceMaterialUnlocked: form.status === 'passed',
    },
  }
}

function deriveCorrectionQueue(workflow: VisualWorkflow, visualHardGatePassed: boolean): CorrectionIntent[] {
  // A missing candidate-bound comparison is an intake state, not a failed
  // geometry gate.  Do not manufacture a repair intent before Runtime has
  // produced the metrics that justify one.
  if (workflow.currentStage === 'reference-canvas' || workflow.gates.silhouette.status === 'not-run') return []
  if (visualHardGatePassed) return []
  const failed = (gate: WorkflowGate) => gate.failedMetrics
  if (workflow.gates.silhouette.status !== 'passed') {
    return [{
      actionId: 'fit-silhouette',
      stage: 'silhouette-blockout',
      failedMetrics: failed(workflow.gates.silhouette),
      instruction: '先匹配投影外轮廓；下一轮只改变一个高置信度可见部件的宽度、高度或位置。',
      constraints: ['保持参考图和 camera lock 不变', '只提交一个 semantic Part intent', '重新运行 geometry/readback → reference_compare_prepare → quality_get'],
    }]
  }
  if (workflow.gates.structure.status !== 'passed') {
    return [{
      actionId: 'fit-landmarks',
      stage: 'landmark-structure',
      failedMetrics: failed(workflow.gates.structure),
      instruction: '用可见 landmark 校准比例和姿态；不要在同一轮移动 camera。',
      constraints: ['只使用 observed landmark', '只改变一个控制最多 landmark 的 semantic Part', '重新运行 geometry/readback → reference_compare_prepare → quality_get'],
    }]
  }
  if (workflow.gates.form.status !== 'passed') {
    return [{
      actionId: 'fit-regions',
      stage: 'semantic-part-fill',
      failedMetrics: failed(workflow.gates.form),
      instruction: '按可见语义区域逐个补齐形体和部件，不把隐藏区域当成已知。',
      constraints: ['一次只改一个 semantic Part 或一个受控 detail operator', '保留旧 candidate 作为比较基线', '重新运行 geometry/readback → reference_compare_prepare → quality_get'],
    }]
  }
  if (!workflow.gates.surfaceMaterialUnlocked) return []
  return [{
    actionId: 'review-surface',
    stage: 'uv-pbr',
    failedMetrics: [],
    instruction: '几何门已通过；先提交 typed visual review，再只改一个 MaterialZone 或 surface recipe。',
    constraints: ['保持 geometry/program/artifact hash 不变', '重复同一组九个 AOV', '等待 Runtime QualityReport 和 human gate'],
  }]
}

const WORKFLOW_STAGE_LABELS: Record<VisualWorkflow['currentStage'], string> = {
  'reference-canvas': '参考画布',
  'silhouette-blockout': '轮廓修正',
  'landmark-structure': '结构修正',
  'semantic-part-fill': '语义部件填充',
  'surface-detail': '表面细节',
  'uv-pbr': 'UV / PBR',
  final: '固定渲染复核',
}

const WORKBENCH_STAGE_STEPS: Array<[VisualWorkflow['currentStage'], string]> = [
  ['reference-canvas', '参考图'],
  ['silhouette-blockout', '大体外形'],
  ['landmark-structure', '结构'],
  ['semantic-part-fill', '细节'],
  ['uv-pbr', '材质'],
  ['final', '完成'],
]

const WORKFLOW_GATE_LABELS: Array<[keyof VisualWorkflow['gates'], string]> = [
  ['silhouette', '轮廓'],
  ['structure', '结构'],
  ['form', '形体'],
]

const WORKFLOW_GATE_STATUS_LABELS: Record<WorkflowGateStatus, string> = {
  passed: '通过',
  failed: '未通过',
  locked: '锁定',
  'not-run': '未运行',
}

type ViewerModel = {
  status: 'Ready' | 'Unavailable'
  retryable: boolean
  projects: ViewerProject[]
  code?: string
}

type ViewerObjectState = {
  basePosition: THREE.Vector3
  direction: THREE.Vector3
  partId: string
  materialZoneId: string
}

type ViewerSceneState = {
  root: THREE.Object3D
  renderer: THREE.WebGLRenderer
  scene: THREE.Scene
  camera: THREE.PerspectiveCamera
  objects: Map<THREE.Object3D, ViewerObjectState>
}

function displayPartLabel(partId: string) {
  const normalized = partId.replace(/[_-]+/g, ' ').replace(/([a-z])([A-Z])/g, '$1 $2').trim()
  const aliases: Record<string, string> = {
    'chest shell': '胸甲',
    'chest panel': '胸甲面板',
    'shoulder left': '左肩甲',
    'shoulder l': '左肩甲',
    'shoulder right': '右肩甲',
    'shoulder r': '右肩甲',
    'forearm left': '左前臂',
    'forearm l': '左前臂',
    'forearm right': '右前臂',
    'forearm r': '右前臂',
    head: '头部',
    pelvis: '骨盆',
    'back thruster': '后背推进器',
  }
  const alias = aliases[normalized.toLowerCase()]
  if (alias) return alias
  return normalized ? normalized.replace(/\b\w/g, (character) => character.toUpperCase()) : '未命名部件'
}

function shortHash(value?: string | null) {
  if (!value) return '—'
  return value.length > 14 ? `${value.slice(0, 8)}…${value.slice(-4)}` : value
}

const HEATMAP_SIZE = 512

function drawContainedImage(context: CanvasRenderingContext2D, image: HTMLImageElement) {
  const width = image.naturalWidth || HEATMAP_SIZE
  const height = image.naturalHeight || HEATMAP_SIZE
  const scale = Math.min(HEATMAP_SIZE / width, HEATMAP_SIZE / height)
  const drawWidth = width * scale
  const drawHeight = height * scale
  const offsetX = (HEATMAP_SIZE - drawWidth) / 2
  const offsetY = (HEATMAP_SIZE - drawHeight) / 2
  context.drawImage(image, offsetX, offsetY, drawWidth, drawHeight)
}

function createDifferenceHeatmap(reference: HTMLImageElement, render: HTMLImageElement): string | null {
  const referenceCanvas = document.createElement('canvas')
  const renderCanvas = document.createElement('canvas')
  const heatmapCanvas = document.createElement('canvas')
  referenceCanvas.width = HEATMAP_SIZE
  referenceCanvas.height = HEATMAP_SIZE
  renderCanvas.width = HEATMAP_SIZE
  renderCanvas.height = HEATMAP_SIZE
  heatmapCanvas.width = HEATMAP_SIZE
  heatmapCanvas.height = HEATMAP_SIZE
  const referenceContext = referenceCanvas.getContext('2d', { willReadFrequently: true })
  const renderContext = renderCanvas.getContext('2d', { willReadFrequently: true })
  const heatmapContext = heatmapCanvas.getContext('2d')
  if (!referenceContext || !renderContext || !heatmapContext) return null
  referenceContext.fillStyle = '#000'
  renderContext.fillStyle = '#000'
  referenceContext.fillRect(0, 0, HEATMAP_SIZE, HEATMAP_SIZE)
  renderContext.fillRect(0, 0, HEATMAP_SIZE, HEATMAP_SIZE)
  drawContainedImage(referenceContext, reference)
  drawContainedImage(renderContext, render)
  const referencePixels = referenceContext.getImageData(0, 0, HEATMAP_SIZE, HEATMAP_SIZE).data
  const renderPixels = renderContext.getImageData(0, 0, HEATMAP_SIZE, HEATMAP_SIZE).data
  const heatmap = heatmapContext.createImageData(HEATMAP_SIZE, HEATMAP_SIZE)
  for (let index = 0; index < heatmap.data.length; index += 4) {
    const redDelta = Math.abs(referencePixels[index] - renderPixels[index])
    const greenDelta = Math.abs(referencePixels[index + 1] - renderPixels[index + 1])
    const blueDelta = Math.abs(referencePixels[index + 2] - renderPixels[index + 2])
    const delta = Math.min(1, (redDelta + greenDelta + blueDelta) / (255 * 3))
    const hue = (1 - delta) * 220
    const chroma = 1 - Math.abs((hue / 60) % 2 - 1)
    const sector = Math.floor(hue / 60)
    const base = delta * 255
    const secondary = chroma * base
    const channels = sector === 0 ? [base, secondary, 0] : sector === 1 ? [secondary, base, 0] : sector === 2 ? [0, base, secondary] : sector === 3 ? [0, secondary, base] : sector === 4 ? [secondary, 0, base] : [base, 0, secondary]
    heatmap.data[index] = channels[0]
    heatmap.data[index + 1] = channels[1]
    heatmap.data[index + 2] = channels[2]
    heatmap.data[index + 3] = delta === 0 ? 0 : Math.round(80 + delta * 150)
  }
  heatmapContext.putImageData(heatmap, 0, 0)
  return heatmapCanvas.toDataURL('image/png')
}

function createReferenceContourAid(reference: HTMLImageElement): string | null {
  const canvas = document.createElement('canvas')
  canvas.width = HEATMAP_SIZE
  canvas.height = HEATMAP_SIZE
  const context = canvas.getContext('2d', { willReadFrequently: true })
  if (!context) return null
  context.fillStyle = '#000'
  context.fillRect(0, 0, HEATMAP_SIZE, HEATMAP_SIZE)
  drawContainedImage(context, reference)
  const source = context.getImageData(0, 0, HEATMAP_SIZE, HEATMAP_SIZE).data
  const pixelCount = HEATMAP_SIZE * HEATMAP_SIZE
  const background = new Uint8Array(pixelCount)
  const queue = new Int32Array(pixelCount)
  let queueHead = 0
  let queueTail = 0
  const enqueue = (index: number) => {
    if (background[index] !== 0) return
    background[index] = 1
    queue[queueTail] = index
    queueTail += 1
  }
  for (let offset = 0; offset < HEATMAP_SIZE; offset += 1) {
    enqueue(offset)
    enqueue((HEATMAP_SIZE - 1) * HEATMAP_SIZE + offset)
    enqueue(offset * HEATMAP_SIZE)
    enqueue(offset * HEATMAP_SIZE + HEATMAP_SIZE - 1)
  }
  const localBackgroundEdgeThreshold = 18
  while (queueHead < queueTail) {
    const index = queue[queueHead]
    queueHead += 1
    const x = index % HEATMAP_SIZE
    const y = Math.floor(index / HEATMAP_SIZE)
    const currentOffset = index * 4
    for (let direction = 0; direction < 4; direction += 1) {
      const neighbor = direction === 0
        ? (x > 0 ? index - 1 : -1)
        : direction === 1
          ? (x + 1 < HEATMAP_SIZE ? index + 1 : -1)
          : direction === 2
            ? (y > 0 ? index - HEATMAP_SIZE : -1)
            : (y + 1 < HEATMAP_SIZE ? index + HEATMAP_SIZE : -1)
      if (neighbor < 0 || background[neighbor] !== 0) continue
      const nextOffset = neighbor * 4
      const distance = Math.abs(source[currentOffset] - source[nextOffset])
        + Math.abs(source[currentOffset + 1] - source[nextOffset + 1])
        + Math.abs(source[currentOffset + 2] - source[nextOffset + 2])
      if (distance <= localBackgroundEdgeThreshold) enqueue(neighbor)
    }
  }
  const foreground = new Uint8Array(pixelCount)
  let foregroundCount = 0
  for (let index = 0; index < pixelCount; index += 1) {
    if (background[index] === 0) {
      foreground[index] = 1
      foregroundCount += 1
    }
  }
  if (foregroundCount === 0) {
    const luminance = (index: number) => source[index] * 0.2126 + source[index + 1] * 0.7152 + source[index + 2] * 0.0722
    for (let index = 0; index < pixelCount; index += 1) {
      foreground[index] = luminance(index * 4) > 48 ? 1 : 0
    }
  }
  const contour = context.createImageData(HEATMAP_SIZE, HEATMAP_SIZE)
  for (let y = 1; y < HEATMAP_SIZE - 1; y += 1) {
    for (let x = 1; x < HEATMAP_SIZE - 1; x += 1) {
      const index = y * HEATMAP_SIZE + x
      if (!foreground[index]) continue
      if (foreground[index - 1] && foreground[index + 1] && foreground[index - HEATMAP_SIZE] && foreground[index + HEATMAP_SIZE]) continue
      const output = index * 4
      contour.data[output] = 255
      contour.data[output + 1] = 170
      contour.data[output + 2] = 55
      contour.data[output + 3] = 230
    }
  }
  context.clearRect(0, 0, HEATMAP_SIZE, HEATMAP_SIZE)
  context.putImageData(contour, 0, 0)
  return canvas.toDataURL('image/png')
}

const EMPTY_MODEL: ViewerModel = { status: 'Unavailable', retryable: true, projects: [] }

type RuntimeViewerProps = {
  onNavigate?: (page: 'home' | 'create' | 'workbench' | 'check' | 'export') => void
}

export function RuntimeViewer({ onNavigate }: RuntimeViewerProps = {}) {
  const [model, setModel] = useState<ViewerModel>(EMPTY_MODEL)
  const [selectedPass, setSelectedPass] = useState<AovPass>('beauty')
  const [compareMode, setCompareMode] = useState<CompareMode>('split')
  const [evidence, setEvidence] = useState<ViewerVisualEvidence | null>(null)
  const [referenceImage, setReferenceImage] = useState<ArtifactBytes | null>(null)
  const [renderImage, setRenderImage] = useState<RenderPass | null>(null)
  const [flickerOn, setFlickerOn] = useState(true)
  const [selectedPartId, setSelectedPartId] = useState('all')
  const [selectedMaterialZone, setSelectedMaterialZone] = useState('all')
  const [selectedObjectId, setSelectedObjectId] = useState('all')
  const [activeTool, setActiveTool] = useState<'select' | 'move' | 'rotate' | 'scale' | 'hand'>('select')
  const [inspectorTab, setInspectorTab] = useState<'inspector' | 'evidence'>('inspector')
  const [advancedOpen, setAdvancedOpen] = useState(false)
  const [notice, setNotice] = useState<string | null>(null)
  const [exploded, setExploded] = useState(false)
  const [diffHeatmap, setDiffHeatmap] = useState(false)
  const [differenceHeatmapUrl, setDifferenceHeatmapUrl] = useState<string | null>(null)
  const [referenceContourAidUrl, setReferenceContourAidUrl] = useState<string | null>(null)
  const [contourPoints, setContourPoints] = useState<Array<{ x: number; y: number }>>([])
  const [contourCopyStatus, setContourCopyStatus] = useState<'idle' | 'copied' | 'unavailable'>('idle')
  const contourDrawingRef = useRef(false)
  const contourCanvasActive = selectedPass === 'silhouette' && compareMode === 'overlay' && !diffHeatmap
  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  const viewerSceneRef = useRef<ViewerSceneState | null>(null)
  const capabilities = useMemo(() => [
    ['入口', 'Codex → MCP stdio'],
    ['写入模型', 'preview → confirm → immutable snapshot'],
    ['Viewer 权限', '只读 Runtime read model'],
    ['当前阶段', 'MCP010F · Viewer compare source'],
  ], [])

  useEffect(() => {
    let active = true
    const refresh = async () => {
      try {
        const next = await invoke<ViewerModel>('viewer_read_model')
        if (active) setModel(next)
      } catch {
        if (active) setModel(EMPTY_MODEL)
      }
    }
    void refresh()
    const timer = window.setInterval(() => void refresh(), 2000)
    return () => {
      active = false
      window.clearInterval(timer)
    }
  }, [])

  const project = model.projects[0]
  const ready = model.status === 'Ready'
  const projectName = project?.project?.name ?? '暂无项目'
  const versionCount = project?.versions?.length ?? 0
  // A single authoring round can leave a geometry candidate followed by an
  // appearance/compare candidate.  Prefer the candidate with candidate-bound
  // visual evidence so the Viewer does not silently fall back to a structural
  // artifact whose reference/render hashes are absent.
  const latestCandidate = project?.candidates?.find((entry) => Boolean(
    entry.quality?.reference_id
      && entry.quality?.comparison_report_hash
      && entry.quality?.render_set_hash,
  )) ?? project?.candidates?.[0]
  const geometryCandidate = project?.candidates?.find((entry) => Boolean(
    entry.artifact?.artifact_id
      && entry.artifact?.part_ids?.length
      && (!latestCandidate?.quality?.program_sha256 || entry.artifact.program_sha256 === latestCandidate.quality.program_sha256),
  ))
    ?? project?.candidates?.find((entry) => Boolean(entry.artifact?.artifact_id && entry.artifact?.part_ids?.length))
    ?? latestCandidate
  const artifact = geometryCandidate?.artifact
  const partCount = artifact?.part_ids?.length ?? 0
  const candidateId = latestCandidate?.candidate?.candidate_id
  const artifactCandidateId = geometryCandidate?.candidate?.candidate_id
  const projectId = project?.project?.project_id
  const reference = latestCandidate?.reference?.reference
  const referenceId = evidence?.reference_id ?? reference?.reference_id ?? undefined
  const renderSetHash = evidence?.render_set_hash
  const partIds = artifact?.part_ids ?? []
  const materialZoneIds = artifact?.material_zone_ids ?? []
  const contourBindingReady = Boolean(
    projectId
      && candidateId
      && referenceId
      && reference?.object_sha256
      && artifact?.artifact_id
      && renderSetHash
      && evidence?.comparison_report_hash,
  )

  const focusAovTab = (pass: AovPass) => {
    setSelectedPass(pass)
    window.requestAnimationFrame(() => document.getElementById(`render-aov-tab-${pass}`)?.focus())
  }

  const handleAovKeyDown = (event: KeyboardEvent<HTMLButtonElement>, pass: AovPass) => {
    const currentIndex = AOV_PASSES.indexOf(pass)
    let nextIndex: number | null = null
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') nextIndex = (currentIndex + 1) % AOV_PASSES.length
    else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') nextIndex = (currentIndex - 1 + AOV_PASSES.length) % AOV_PASSES.length
    else if (event.key === 'Home') nextIndex = 0
    else if (event.key === 'End') nextIndex = AOV_PASSES.length - 1
    if (nextIndex === null) return
    event.preventDefault()
    focusAovTab(AOV_PASSES[nextIndex] ?? pass)
  }

  useEffect(() => {
    setSelectedPartId('all')
    setSelectedMaterialZone('all')
    setSelectedObjectId('all')
    setExploded(false)
    setDiffHeatmap(false)
    setContourPoints([])
    setContourCopyStatus('idle')
  }, [candidateId])

  useEffect(() => {
    let active = true
    setEvidence(null)
    setReferenceImage(null)
    setRenderImage(null)
    if (!candidateId) return () => { active = false }
    void invoke<ViewerVisualEvidence>('viewer_visual_evidence', { candidateId }).then((next) => {
      if (active && next?.quality_report) setEvidence(next)
    }).catch(() => undefined)
    return () => { active = false }
  }, [candidateId])

  useEffect(() => {
    let active = true
    setReferenceImage(null)
    setRenderImage(null)
    if (!referenceId || !projectId || !renderSetHash) return () => { active = false }
    void Promise.all([
      invoke<ArtifactBytes>('viewer_reference_bytes', { referenceId, projectId }),
      invoke<RenderPass>('viewer_render_pass', { renderSetHash, pass: selectedPass }),
    ]).then(([referencePayload, renderPayload]) => {
      if (!active) return
      setReferenceImage(referencePayload)
      setRenderImage(renderPayload)
    }).catch(() => undefined)
    return () => { active = false }
  }, [referenceId, projectId, renderSetHash, selectedPass])

  useEffect(() => {
    if (compareMode !== 'flicker') {
      setFlickerOn(true)
      return undefined
    }
    const timer = window.setInterval(() => setFlickerOn((value) => !value), 500)
    return () => window.clearInterval(timer)
  }, [compareMode])

  useEffect(() => {
    const artifactId = artifact?.artifact_id
    const candidateId = artifactCandidateId
    const canvas = canvasRef.current
    if (!artifactId || !candidateId || !canvas) return
    let disposed = false
    const scene = new THREE.Scene()
    scene.background = new THREE.Color('#080d14')
    const camera = new THREE.PerspectiveCamera(32, 1, 0.01, 100)
    const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true })
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
    renderer.setSize(canvas.clientWidth || 640, canvas.clientHeight || 520, false)
    scene.add(new THREE.HemisphereLight('#f1f6ff', '#172536', 2.2))
    const key = new THREE.DirectionalLight('#ffd39c', 2.4)
    key.position.set(4, 5, 6)
    scene.add(key)
    const loader = new GLTFLoader()
    void invoke<ArtifactBytes>('viewer_artifact_bytes', { artifactId, candidateId }).then((payload) => {
      if (disposed || !payload.bytes_base64) return
      const binary = Uint8Array.from(atob(payload.bytes_base64), (character) => character.charCodeAt(0))
      loader.parse(binary.buffer, '', (gltf) => {
        if (disposed) return
        const root = gltf.scene
        scene.add(root)
        const box = new THREE.Box3().setFromObject(root)
        const size = box.getSize(new THREE.Vector3())
        const center = box.getCenter(new THREE.Vector3())
        const radius = Math.max(size.x, size.y, size.z, 0.1)
        camera.position.set(radius * 1.9, radius * 1.15, radius * 2.1)
        camera.lookAt(center)
        const objects = new Map<THREE.Object3D, ViewerObjectState>()
        root.traverse((object) => {
          if (!(object as THREE.Mesh).isMesh) return
          const mesh = object as THREE.Mesh
          const metadata = (mesh.userData ?? {}) as Record<string, unknown>
          const material = Array.isArray(mesh.material) ? mesh.material[0] : mesh.material
          const partId = typeof metadata.part_id === 'string'
            ? metadata.part_id
            : (mesh.name || mesh.parent?.name || 'unknown-part')
          const materialZoneId = typeof metadata.material_zone_id === 'string'
            ? metadata.material_zone_id
            : (material && 'name' in material && material.name ? material.name : 'unknown-material-zone')
          const objectCenter = new THREE.Box3().setFromObject(mesh).getCenter(new THREE.Vector3())
          const direction = objectCenter.sub(center)
          if (direction.lengthSq() < 1e-8) direction.set(0, 1, 0)
          else direction.normalize()
          objects.set(mesh, { basePosition: mesh.position.clone(), direction, partId, materialZoneId })
        })
        viewerSceneRef.current = { root, renderer, scene, camera, objects }
        renderer.render(scene, camera)
      }, () => undefined)
    }).catch(() => undefined)
    return () => {
      disposed = true
      renderer.dispose()
      if (viewerSceneRef.current?.scene === scene) viewerSceneRef.current = null
      while (scene.children.length) scene.remove(scene.children[0])
    }
  }, [artifact?.artifact_id, artifactCandidateId])

  useEffect(() => {
    const state = viewerSceneRef.current
    if (!state) return
    state.objects.forEach((objectState, object) => {
      const partMatches = selectedPartId === 'all' || objectState.partId === selectedPartId
      const materialMatches = selectedMaterialZone === 'all' || objectState.materialZoneId === selectedMaterialZone
      object.visible = partMatches && materialMatches
      object.position.copy(objectState.basePosition)
      if (exploded && object.visible) object.position.addScaledVector(objectState.direction, 0.18)
    })
    state.renderer.render(state.scene, state.camera)
  }, [selectedPartId, selectedMaterialZone, exploded, diffHeatmap])

  const referenceDataUrl = referenceImage?.bytes_base64
    ? `data:${referenceImage.mime ?? 'image/png'};base64,${referenceImage.bytes_base64}`
    : undefined
  const renderDataUrl = renderImage?.png_base64
    ? `data:${renderImage.mime ?? 'image/png'};base64,${renderImage.png_base64}`
    : undefined

  useEffect(() => {
    let active = true
    setDifferenceHeatmapUrl(null)
    if (!diffHeatmap || !referenceDataUrl || !renderDataUrl) return () => { active = false }
    const load = async () => {
      const reference = new Image()
      const render = new Image()
      reference.src = referenceDataUrl
      render.src = renderDataUrl
      await Promise.all([reference.decode(), render.decode()])
      if (active) setDifferenceHeatmapUrl(createDifferenceHeatmap(reference, render))
    }
    void load().catch(() => undefined)
    return () => { active = false }
  }, [diffHeatmap, referenceDataUrl, renderDataUrl])

  useEffect(() => {
    let active = true
    setReferenceContourAidUrl(null)
    if (!contourCanvasActive || !referenceDataUrl) return () => { active = false }
    const load = async () => {
      const reference = new Image()
      reference.src = referenceDataUrl
      await reference.decode()
      if (active) setReferenceContourAidUrl(createReferenceContourAid(reference))
    }
    void load().catch(() => undefined)
    return () => { active = false }
  }, [contourCanvasActive, referenceDataUrl])

  const comparisonMetrics = evidence?.comparison_report?.metrics ?? {}
  const visualQualityReport = evidence?.quality_report
  const visualStatus = visualQualityReport?.visual_status ?? 'not-run'
  const visualStatusLabel = visualStatus === 'PARTIAL_VISIBLE_VIEW_PASS'
    ? '局部可见视图通过'
    : visualStatus === 'QUALITY_TARGET_NOT_MET'
      ? '质量目标未达标'
      : visualStatus === 'not-run'
        ? '未运行'
        : visualStatus
  const visualHardGatePassed = visualQualityReport?.hard_gate_passed === true && visualStatus === 'PARTIAL_VISIBLE_VIEW_PASS'
  const visualGateSource = visualQualityReport ? 'candidate-bound QualityReport@2' : 'not-run: visual QualityReport unavailable'
  const visualWorkflow = useMemo(
    () => deriveVisualWorkflow(comparisonMetrics as Record<string, unknown>, visualHardGatePassed),
    [comparisonMetrics, visualHardGatePassed],
  )
  const correctionQueue = useMemo(() => deriveCorrectionQueue(visualWorkflow, visualHardGatePassed), [visualWorkflow, visualHardGatePassed])
  const workflowNote = visualWorkflow.currentStage === 'reference-canvas'
    ? '等待 candidate-bound comparison metrics；画布只用于临时对照。'
    : visualWorkflow.currentStage === 'silhouette-blockout'
      ? '先修正外轮廓；结构、形体和表面材质不会绕过轮廓门。'
      : visualWorkflow.currentStage === 'landmark-structure'
        ? '轮廓已通过，继续用可见 landmark 校准比例和姿态。'
        : visualWorkflow.currentStage === 'semantic-part-fill'
          ? '轮廓与结构已通过，继续补齐可追踪语义部件和形体区域。'
          : visualWorkflow.currentStage === 'uv-pbr'
            ? '几何门已通过；继续检查同一 candidate 的 UV、PBR 和固定 AOV。'
            : '当前候选可进入固定渲染复核；仍需以 Runtime QualityReport 和真人门为准。'
  const metricLabels: Array<[string, string]> = [
    ['silhouette_iou', 'Silhouette IoU'],
    ['boundary_f1_4px', 'Boundary F1'],
    ['bbox_edge_error', 'BBox edge error'],
    ['centroid_error', 'Centroid error'],
    ['landmark_coverage', 'Landmark coverage'],
    ['landmark_nme', 'Landmark NME'],
    ['region_median_iou', 'Region median IoU'],
    ['critical_region_min_iou', 'Critical-region IoU'],
  ]

  const selectedObjectLabel = selectedObjectId === 'all' ? '模型主体' : displayPartLabel(selectedObjectId)
  const selectionIsAvailable = selectedObjectId !== 'all' && partIds.includes(selectedObjectId)
  const viewportSnapshotLabel = ready
    ? (project?.record?.head_snapshot_id ? '已读取当前快照' : '暂无已确认快照')
    : '等待 Runtime'
  const viewportMeta = artifact
    ? `${partCount} 个语义部件 · ${artifact.triangle_count ?? 0} 个三角形 · UV ${artifact.uv_status ?? '未知'} · 切线 ${artifact.tangent_status ?? '未知'}`
    : '选择对象、观察模型；永久修改仍由 Codex 通过 MCP 提交。'
  const candidateStateLabel = latestCandidate?.candidate?.state === 'confirmed'
    ? '已确认'
    : latestCandidate?.candidate?.state === 'draft'
      ? '草稿'
      : latestCandidate?.candidate?.state === 'rejected'
        ? '已拒绝'
        : '无'
  const activityItems = [
    {
      state: selectionIsAvailable ? 'active' : 'idle',
      label: selectionIsAvailable ? `选择 ${selectedObjectLabel}` : '等待 Codex / 用户选择对象',
      detail: selectionIsAvailable ? 'Viewer 临时选择' : viewportSnapshotLabel,
    },
    {
      state: visualWorkflow.gates.silhouette.status === 'passed' ? 'done' : 'warn',
      label: visualWorkflow.gates.silhouette.status === 'passed' ? '轮廓门已通过' : '轮廓门未通过',
      detail: visualStatusLabel,
    },
    {
      state: evidence?.comparison_report_hash ? 'done' : 'idle',
      label: evidence?.comparison_report_hash ? '已读取固定视图证据' : '等待 candidate-bound 证据',
      detail: evidence?.comparison_report_hash ? shortHash(evidence.comparison_report_hash) : '参考图 / 渲染集',
    },
  ]

  const normalizeContourPoint = (event: PointerEvent<SVGSVGElement>) => {
    const bounds = event.currentTarget.getBoundingClientRect()
    if (bounds.width <= 0 || bounds.height <= 0) return null
    return {
      x: Math.min(1, Math.max(0, (event.clientX - bounds.left) / bounds.width)),
      y: Math.min(1, Math.max(0, (event.clientY - bounds.top) / bounds.height)),
    }
  }

  const appendContourPoint = (event: PointerEvent<SVGSVGElement>) => {
    if (!contourCanvasActive) return
    const point = normalizeContourPoint(event)
    if (!point) return
    setContourPoints((current) => {
      if (current.length >= 128) return current
      const previous = current[current.length - 1]
      if (previous && Math.hypot(previous.x - point.x, previous.y - point.y) < 0.004) return current
      return [...current, point]
    })
  }

  const handleContourPointerDown = (event: PointerEvent<SVGSVGElement>) => {
    if (!contourCanvasActive) return
    event.preventDefault()
    event.currentTarget.setPointerCapture(event.pointerId)
    contourDrawingRef.current = true
    appendContourPoint(event)
  }

  const handleContourPointerMove = (event: PointerEvent<SVGSVGElement>) => {
    if (!contourDrawingRef.current) return
    appendContourPoint(event)
  }

  const handleContourPointerUp = (event: PointerEvent<SVGSVGElement>) => {
    contourDrawingRef.current = false
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
  }

  const clearContourDraft = () => {
    setContourPoints([])
    setContourCopyStatus('idle')
  }

  const undoContourPoint = () => {
    setContourPoints((current) => current.slice(0, -1))
    setContourCopyStatus('idle')
  }

  const copyContourDraft = async () => {
    if (contourPoints.length < 3 || !contourBindingReady || !navigator.clipboard) {
      setContourCopyStatus('unavailable')
      return
    }
    const draft = {
      schema_version: 'ForgeCADViewerContourDraft@2',
      coordinate_space: 'normalized_reference_image',
      points: contourPoints,
      closed: true,
      transient_only: true,
      runtime_write: false,
      project_id: projectId,
      candidate_id: candidateId,
      reference_id: referenceId,
      artifact_sha256: artifact?.artifact_id,
      render_set_hash: renderSetHash,
      comparison_report_hash: evidence?.comparison_report_hash,
      source_pass: 'silhouette',
      selected_part_id: selectedPartId === 'all' ? null : selectedPartId,
      selected_material_zone_id: selectedMaterialZone === 'all' ? null : selectedMaterialZone,
      // The Viewer remains read-only, but a selected Part must be carried
      // into the Codex handoff.  Runtime can then bind the drawn chain to a
      // semantic Part instead of comparing it as an unlabeled whole-body
      // contour.  The indices are intentionally local to this draft; Codex
      // submits them with the same point array to reference_mask_refine_prepare.
      parts: selectedPartId === 'all'
        ? []
        : [{
            part_id: selectedPartId,
            start_index: 0,
            end_index: contourPoints.length - 1,
            visibility: 'observed',
          }],
    }
    try {
      await navigator.clipboard.writeText(JSON.stringify(draft))
      setContourCopyStatus('copied')
    } catch {
      setContourCopyStatus('unavailable')
    }
  }

  const currentWorkbenchStageIndex = Math.max(0, WORKBENCH_STAGE_STEPS.findIndex(([stage]) => stage === visualWorkflow.currentStage))

  return <main className="forgecad-app" data-testid="forgecad-workbench">
    <header className="app-bar">
      <div className="brand-lockup" aria-label="ForgeCAD">
        <span className="brand-mark" aria-hidden="true"><Cube size={19} weight="duotone" /></span>
        <strong>ForgeCAD</strong>
      </div>
      <span className="app-bar-divider" aria-hidden="true" />
      <button type="button" className="project-switcher" title="当前项目" onClick={() => onNavigate?.('home')}><span>{projectName}</span><CaretDown size={14} /></button>
      <div className="app-bar-spacer" />
      <div className={`connection-state ${ready ? 'connection-ready' : 'connection-muted'}`} role="status"><span className="connection-dot" />{ready ? 'Codex · Runtime 已连接' : '等待 Codex / Runtime'}</div>
      <span className="app-bar-divider" aria-hidden="true" />
      <div className="app-actions" aria-label="全局操作">
        <button type="button" className="icon-button" aria-label="撤销" title="撤销（由 Codex 管理）" onClick={() => setNotice('Viewer 不直接修改 Runtime；请在 Codex 中发起撤销或恢复。')}><ArrowUUpLeft size={17} /></button>
        <button type="button" className="icon-button" aria-label="重做" title="重做（由 Codex 管理）" onClick={() => setNotice('Viewer 不直接修改 Runtime；请在 Codex 中发起重做或恢复。')}><ArrowUUpRight size={17} /></button>
        <button type="button" className="top-action" onClick={() => setNotice('保存是 Runtime 的确认事务；Viewer 当前只读。')}><FloppyDisk size={16} />保存</button>
        <button type="button" className="top-action top-action-primary" onClick={() => onNavigate?.('export')}><Export size={16} />导出</button>
      </div>
    </header>

    <nav className="stage-progress-bar" aria-label="创作阶段">
      {WORKBENCH_STAGE_STEPS.map(([stage, label], index) => <span key={stage} className={`stage-progress-step ${index === currentWorkbenchStageIndex ? 'stage-progress-current' : ''} ${index < currentWorkbenchStageIndex ? 'stage-progress-done' : ''}`}><span>{index < currentWorkbenchStageIndex ? <Check size={11} /> : index + 1}</span>{label}</span>)}
    </nav>

    <section className="workbench-grid" aria-label="ForgeCAD 工作台">
      <aside className="scene-panel" data-testid="scene-tree" aria-label="场景树">
        <div className="panel-header">
          <div><span className="panel-title">场景</span><span className="panel-subtitle">结构树</span></div>
          <div className="panel-header-actions"><button type="button" className="icon-button icon-button-small" aria-label="搜索对象" title="搜索对象"><MagnifyingGlass size={15} /></button><button type="button" className="icon-button icon-button-small" aria-label="添加对象" title="由 Codex 创建对象" onClick={() => setNotice('新对象由 Codex 通过 MCP 创建；Viewer 不直接写入。')}><Plus size={16} /></button></div>
        </div>
        <div className="tree-scroll">
          <div className="tree-row tree-row-root"><CaretDown size={14} /><Stack size={15} weight="duotone" /><span>Robot_A</span><button type="button" className="tree-visibility" aria-label="Robot_A 可见" title="可见"><Eye size={14} /></button></div>
          <div className="tree-row tree-row-group"><CaretDown size={14} /><FolderSimple size={15} weight="duotone" /><span>模型</span></div>
          {partIds.length > 0 ? partIds.map((partId) => <button type="button" key={partId} className={`tree-row tree-row-part ${selectedObjectId === partId ? 'tree-row-selected' : ''}`} onClick={() => { setSelectedObjectId(partId); setInspectorTab('inspector') }} aria-current={selectedObjectId === partId ? 'true' : undefined}><Cube size={14} weight={selectedObjectId === partId ? 'fill' : 'regular'} /><span>{displayPartLabel(partId)}</span><span className="tree-row-id">{partId}</span><span className="tree-row-state"><Eye size={13} /></span></button>) : <div className="tree-empty"><Cube size={18} /><span>等待 Runtime 候选</span><small>Codex 提交后显示语义部件</small></div>}
          <div className="tree-row tree-row-group tree-row-muted"><CaretRight size={14} /><Lightbulb size={15} weight="duotone" /><span>环境 / 灯光</span><span className="tree-row-state"><Eye size={13} /></span></div>
          <div className="tree-row tree-row-group tree-row-muted"><CaretRight size={14} /><Camera size={15} weight="duotone" /><span>相机</span><span className="tree-row-state"><Eye size={13} /></span></div>
        </div>
        <div className="scene-footer"><span><MapPin size={13} />临时选择</span><button type="button" className="text-button" onClick={() => setSelectedObjectId('all')} disabled={selectedObjectId === 'all'}>清除</button></div>
      </aside>

      <section className="viewport-panel" data-testid="viewport-stage" aria-label="3D Viewport">
        <div className="viewport-header">
          <div className="viewport-breadcrumb"><span>3D 视口</span><span className="breadcrumb-separator">/</span><strong>{selectedObjectLabel}</strong></div>
          <div className="viewport-header-meta"><span className="snapshot-indicator"><span className="snapshot-dot" />{viewportSnapshotLabel}</span><button type="button" className="icon-button icon-button-small" title="重置视角" aria-label="重置视角" onClick={() => setNotice('视角重置只影响 Viewer 临时状态。')}><ArrowClockwise size={15} /></button><button type="button" className="icon-button icon-button-small" title="聚焦当前对象（F）" aria-label="聚焦当前对象" onClick={() => setNotice(selectionIsAvailable ? `已聚焦 ${selectedObjectLabel}。` : '请先在场景树中选择一个对象。')}><ArrowsOut size={16} /></button></div>
        </div>
        <div className="viewport-content">
          <div className="viewport-tool-rail" aria-label="Viewport 工具">
            {([['select', '选择', <Cursor size={18} />], ['move', '移动', <ArrowsOut size={18} />], ['rotate', '旋转', <ArrowClockwise size={18} />], ['scale', '缩放', <CornersOut size={18} />], ['hand', '平移', <Hand size={18} />]] as const).map(([tool, label, icon]) => <button type="button" key={tool} className={`viewport-tool ${activeTool === tool ? 'viewport-tool-active' : ''}`} title={label} aria-label={label} aria-pressed={activeTool === tool} onClick={() => setActiveTool(tool)}>{icon}</button>)}
            <span className="tool-rail-divider" />
            <button type="button" className="viewport-tool" title="测量" aria-label="测量" onClick={() => setNotice('测量工具将在 Runtime 测量证据接入后启用。')}><Ruler size={18} /></button>
          </div>
          <div className="viewport-stage" aria-label={artifact ? 'GLB artifact readback' : '等待 Runtime 的 3D Viewport'}>
            <div className="viewport-grid-lines" aria-hidden="true" />
            <div className="viewport-axis" aria-hidden="true"><span className="axis-y">Y</span><span className="axis-x">X</span><span className="axis-z">Z</span></div>
            {artifact ? <><canvas ref={canvasRef} className="glb-canvas" aria-label="Runtime GLB 3D preview" /><div className="viewport-readback-badge"><CheckCircle size={14} />模型回读已连接</div><div className="viewport-model-meta"><strong>{artifact.artifact_id ? shortHash(artifact.artifact_id) : 'artifact'}</strong><span>{viewportMeta}</span></div></> : <div className="viewport-message"><span className="viewport-empty-icon"><Cube size={26} weight="duotone" /></span><strong>等待 Codex 提交设计</strong><span>这里仅查看模型、材质、参考比较和版本状态。</span></div>}
            {selectionIsAvailable && <div className="ai-cursor-badge"><Sparkle size={14} />Codex 正在查看：{selectedObjectLabel}</div>}
            <div className="viewport-help"><span>左键选择</span><span>右键旋转</span><span>滚轮缩放</span><kbd>F</kbd> 聚焦</div>
            <div className="viewport-corner-actions"><button type="button" className="icon-button" title="网格显示" aria-label="网格显示"><SlidersHorizontal size={17} /></button><button type="button" className="icon-button" title="相机设置" aria-label="相机设置" onClick={() => setNotice('相机由 candidate-bound RenderSet 管理；Viewer 只保留临时观察状态。')}><Camera size={17} /></button><button type="button" className="icon-button" title="适应窗口" aria-label="适应窗口" onClick={() => setNotice('已请求适应当前 Viewport。')}><ArrowsOut size={17} /></button></div>
          </div>
        </div>
        <div className="viewport-statusbar"><span><span className={`status-led ${ready ? 'status-led-green' : ''}`} />{ready ? 'Runtime 已就绪 · 只读' : 'Runtime 不可用 · 查看模式'}</span><span>项目：{projectName}</span><span>候选：{candidateStateLabel}</span><span className="statusbar-spacer" /><span>{artifact?.triangle_count ?? 0} 个三角形</span><span>512×512 证据</span></div>
      </section>

      <aside className="inspector-panel" data-testid="inspector-panel" aria-label="属性与证据">
        <div className="inspector-tabs" role="tablist" aria-label="右侧面板">
          <button type="button" role="tab" aria-selected={inspectorTab === 'inspector'} className={`inspector-tab ${inspectorTab === 'inspector' ? 'inspector-tab-active' : ''}`} onClick={() => setInspectorTab('inspector')}>属性 <span>当前对象</span></button>
          <button type="button" role="tab" aria-selected={inspectorTab === 'evidence'} className={`inspector-tab ${inspectorTab === 'evidence' ? 'inspector-tab-active' : ''}`} onClick={() => setInspectorTab('evidence')}>证据 <span>AOV</span></button>
        </div>
        {inspectorTab === 'inspector' ? <div className="inspector-scroll">
          <div className="selected-object-card"><div className="selected-object-icon"><Cube size={20} weight="duotone" /></div><div><span className="muted-label">当前选择</span><strong>{selectedObjectLabel}</strong><small>{selectionIsAvailable ? selectedObjectId : 'Viewer 临时选择'}</small></div><span className={`selection-dot ${selectionIsAvailable ? 'selection-dot-active' : ''}`} /></div>
          <div className="inspector-action-row"><button type="button" title="复制对象" aria-label="复制对象" onClick={() => setNotice('复制是 Runtime candidate 事务；请通过 Codex 发起。')}><Copy size={16} /></button><button type="button" title="隐藏对象" aria-label="隐藏对象" onClick={() => setNotice('隐藏只影响临时 Viewer selection，当前对象未改变。')}><EyeSlash size={16} /></button><button type="button" title="锁定对象" aria-label="锁定对象" onClick={() => setNotice('对象锁定需要 Runtime 的显式状态合同。')}><Lock size={16} /></button><button type="button" title="删除对象" aria-label="删除对象" onClick={() => setNotice('Viewer 不提供删除；永久修改必须由 Codex 审批。')}><Trash size={16} /></button></div>
          <section className="inspector-section"><button type="button" className="inspector-section-heading" onClick={() => setAdvancedOpen((value) => !value)}><CaretDown size={14} /><span>变换</span><span className="section-count">只读</span></button><div className="property-grid"><span>位置</span><div className="property-value"><b>X</b><span>—</span><b>Y</b><span>—</span><b>Z</b><span>—</span><em>mm</em></div><span>旋转</span><div className="property-value"><b>X</b><span>—</span><b>Y</b><span>—</span><b>Z</b><span>—</span><em>°</em></div><span>缩放</span><div className="property-value"><b>X</b><span>—</span><b>Y</b><span>—</span><b>Z</b><span>—</span></div></div></section>
          <section className="inspector-section"><div className="inspector-section-heading"><CaretDown size={14} /><span>几何</span><span className="section-count">{artifact ? `${artifact.triangle_count ?? 0} 三角形` : '不可用'}</span></div><div className="property-list"><div><span>尺寸</span><strong>—</strong></div><div><span>镜像</span><strong>—</strong></div><div><span>平滑 / 细分</span><strong>—</strong></div></div></section>
          <section className="inspector-section"><div className="inspector-section-heading"><CaretDown size={14} /><span>材质</span><span className="section-count">{materialZoneIds.length} 个区域</span></div><div className="material-row"><span className="material-swatch" /><div><strong>{selectedMaterialZone === 'all' ? '未选择材质区' : selectedMaterialZone}</strong><small>金属度 / 粗糙度由资产包提供</small></div><DotsThree size={18} /></div></section>
          <section className="inspector-section inspector-quality"><div className="inspector-section-heading"><CaretDown size={14} /><span>质量</span><span className={`quality-inline ${visualHardGatePassed ? 'quality-inline-pass' : 'quality-inline-warn'}`}><span />{visualStatusLabel}</span></div><div className="quality-line"><span>视觉门</span><strong>{visualHardGatePassed ? '通过' : '未通过'}</strong></div><div className="quality-line"><span>生成版本</span><code>{shortHash(candidateId)}</code></div><div className="quality-line"><span>模型文件</span><code>{shortHash(artifact?.artifact_id)}</code></div></section>
          <button type="button" className="advanced-row" onClick={() => setAdvancedOpen((value) => !value)}><CaretRight size={14} className={advancedOpen ? 'advanced-open' : ''} />高级参数<span>技术详情</span></button>
          {advancedOpen && <div className="advanced-note"><Info size={15} /><span>Viewer 只呈现 Runtime read model；这里不编辑 GeometryProgram、AppearanceProgram 或 QualityReport。</span></div>}
          <div className="inspector-note"><Info size={14} /><span>永久修改先由 Codex 准备，再经回读、质量、审批和确认。</span></div>
        </div> : <div className="evidence-scroll" data-testid="evidence-panel">
          <div className="evidence-heading"><div><span className="muted-label">固定视图证据</span><h2>与参考图对比</h2></div><span className={`quality-inline ${visualHardGatePassed ? 'quality-inline-pass' : 'quality-inline-warn'}`}><span />{visualStatusLabel}</span></div>
          <div className="evidence-controls"><div className="evidence-control-label">AOV</div><div className="aov-tabs" role="tablist" aria-label="Render AOV passes">{AOV_PASSES.map((pass) => <button key={pass} id={`render-aov-tab-${pass}`} role="tab" aria-controls="render-aov-panel" tabIndex={selectedPass === pass ? 0 : -1} type="button" className={`aov-tab ${selectedPass === pass ? 'aov-tab-active' : ''}`} aria-selected={selectedPass === pass} onClick={() => focusAovTab(pass)} onKeyDown={(event) => handleAovKeyDown(event, pass)}>{pass}</button>)}</div><div className="mode-tabs" role="group" aria-label="Compare mode">{(['split', 'overlay', 'flicker'] as CompareMode[]).map((mode) => <button key={mode} type="button" className={`mode-tab ${compareMode === mode ? 'mode-tab-active' : ''}`} aria-pressed={compareMode === mode} onClick={() => setCompareMode(mode)}>{mode === 'split' ? '分屏' : mode === 'overlay' ? '叠加' : '闪烁'}</button>)}</div></div>
          <div className="viewer-controls" aria-label="Part and material controls"><label>Part<select value={selectedPartId} onChange={(event) => setSelectedPartId(event.target.value)} disabled={partIds.length === 0}><option value="all">全部部件</option>{partIds.map((partId) => <option key={partId} value={partId}>{displayPartLabel(partId)}</option>)}</select></label><label>MaterialZone<select value={selectedMaterialZone} onChange={(event) => setSelectedMaterialZone(event.target.value)} disabled={materialZoneIds.length === 0}><option value="all">全部材质区</option>{materialZoneIds.map((zoneId) => <option key={zoneId} value={zoneId}>{zoneId}</option>)}</select></label><button type="button" className={`viewer-toggle ${exploded ? 'viewer-toggle-active' : ''}`} aria-pressed={exploded} onClick={() => setExploded((value) => !value)}>爆炸图</button><button type="button" className={`viewer-toggle ${contourCanvasActive ? 'viewer-toggle-active' : ''}`} aria-pressed={contourCanvasActive} onClick={() => { setSelectedPass('silhouette'); setCompareMode('overlay'); setDiffHeatmap(false) }}>轮廓</button><button type="button" className={`viewer-toggle ${diffHeatmap ? 'viewer-toggle-active' : ''}`} aria-pressed={diffHeatmap} onClick={() => setDiffHeatmap((value) => !value)}>热图</button></div>
          <div id="render-aov-panel" role="tabpanel" aria-labelledby={`render-aov-tab-${selectedPass}`} className={`compare-stage compare-${compareMode} ${contourCanvasActive ? 'contour-canvas' : ''} ${diffHeatmap ? 'compare-heatmap' : ''}`} aria-label={`${selectedPass} reference comparison`}>{contourCanvasActive && <div className="contour-canvas-badge">CONTOUR · VIEWER ONLY</div>}{referenceDataUrl && (compareMode === 'split' || compareMode === 'overlay' || (compareMode === 'flicker' && !flickerOn)) && <div className="compare-pane compare-reference"><span>REFERENCE</span><img src={referenceDataUrl} alt="Authorized reference" /></div>}{renderDataUrl && (compareMode === 'split' || compareMode === 'overlay' || (compareMode === 'flicker' && flickerOn)) && <div className="compare-pane compare-render"><span>{selectedPass.toUpperCase()}</span><img src={renderDataUrl} alt={`Fixed render ${selectedPass}`} /></div>}{contourCanvasActive && referenceContourAidUrl && <div className="reference-contour-aid"><span>CONTOUR AID · VIEWER ONLY</span><img src={referenceContourAidUrl} alt="Deterministic reference contour aid" /></div>}{contourCanvasActive && referenceDataUrl && <svg className="contour-annotation-layer" viewBox="0 0 1 1" role="img" aria-label="临时参考轮廓草图；不会写入 Runtime" onPointerDown={handleContourPointerDown} onPointerMove={handleContourPointerMove} onPointerUp={handleContourPointerUp} onPointerCancel={handleContourPointerUp}>{contourPoints.length > 1 && <polyline points={contourPoints.map((point) => `${point.x},${point.y}`).join(' ')} fill="none" stroke="#ffb15a" strokeWidth="0.006" vectorEffect="non-scaling-stroke" />}{contourPoints.length > 2 && <line x1={contourPoints[contourPoints.length - 1]?.x} y1={contourPoints[contourPoints.length - 1]?.y} x2={contourPoints[0]?.x} y2={contourPoints[0]?.y} stroke="#ffb15a" strokeWidth="0.003" strokeDasharray="0.012 0.008" vectorEffect="non-scaling-stroke" />}{contourPoints.map((point, index) => <circle key={`${point.x}-${point.y}-${index}`} cx={point.x} cy={point.y} r="0.008" fill="#ffb15a" stroke="#26180c" strokeWidth="0.003" vectorEffect="non-scaling-stroke" />)}</svg>}{diffHeatmap && differenceHeatmapUrl && <div className="heatmap-layer"><span>PIXEL DIFF · 512×512</span><img className="heatmap-image" src={differenceHeatmapUrl} alt="Reference and render difference heatmap" /></div>}{diffHeatmap && <div className="heatmap-legend" role="status">{differenceHeatmapUrl ? '差异热图由当前参考图和 Render AOV 在 Viewer 内存中生成；数值质量仍以 Runtime QualityReport 为准。' : '正在生成当前参考图与 Render AOV 的差异热图…'}</div>}{!referenceDataUrl || !renderDataUrl ? <div className="compare-empty">等待 candidate-bound 参考图、RenderSet 和 {selectedPass} PNG</div> : null}</div>
          {contourCanvasActive && referenceDataUrl && <div className="contour-draft-toolbar" aria-label="临时轮廓草图工具"><span aria-live="polite">轮廓点：{contourPoints.length}/128 · {contourBindingReady ? 'candidate-bound' : '等待 evidence'}</span><button type="button" className="viewer-toggle" onClick={undoContourPoint} disabled={contourPoints.length === 0}>撤销</button><button type="button" className="viewer-toggle" onClick={clearContourDraft} disabled={contourPoints.length === 0}>清除</button><button type="button" className="viewer-toggle" onClick={() => void copyContourDraft()} disabled={contourPoints.length < 3 || !contourBindingReady}>{contourCopyStatus === 'copied' ? '已复制给 Codex' : '复制点集'}</button></div>}
          <div className="compare-footer"><span>Camera lock · 512×512</span><span>RenderSet: {shortHash(renderSetHash)}</span><span>Reference: {shortHash(referenceId)}</span></div>
          <section className="evidence-workflow"><div className="evidence-section-heading"><span>轮廓优先门</span><strong>{WORKFLOW_STAGE_LABELS[visualWorkflow.currentStage]}</strong></div><div className="workflow-gates" aria-label="轮廓优先阶段门">{WORKFLOW_GATE_LABELS.map(([key, label]) => { const gate = visualWorkflow.gates[key] as WorkflowGate; return <div className="workflow-gate-row" key={key}><span>{label}</span><strong className={`workflow-gate-status workflow-gate-status-${gate.status}`}>{WORKFLOW_GATE_STATUS_LABELS[gate.status]}</strong></div> })}<div className="workflow-gate-row"><span>表面 / 材质</span><strong className={`workflow-gate-status ${visualWorkflow.gates.surfaceMaterialUnlocked ? 'workflow-gate-status-passed' : 'workflow-gate-status-locked'}`}>{visualWorkflow.gates.surfaceMaterialUnlocked ? '可进入' : '锁定'}</strong></div></div><p className="workflow-note">{workflowNote} Viewer 不写 Runtime。</p></section>
          <section className="evidence-metrics"><div className="evidence-section-heading"><span>质量指标</span><code>{shortHash(evidence?.comparison_report_hash)}</code></div>{metricLabels.map(([key, label]) => <div className="metric-row" key={key}><span>{label}</span><strong>{typeof comparisonMetrics[key] === 'number' ? comparisonMetrics[key].toFixed(3) : '—'}</strong></div>)}</section>
          <section className="evidence-next"><div className="evidence-section-heading"><span>下一轮修正意图</span><Sparkle size={15} /></div>{correctionQueue.length > 0 ? correctionQueue.map((intent) => <article className="correction-card" key={intent.actionId}><div className="correction-card-header"><strong>{intent.actionId}</strong><span>{WORKFLOW_STAGE_LABELS[intent.stage]}</span></div><p>{intent.instruction}</p>{intent.failedMetrics.length > 0 && <div className="correction-metrics">{intent.failedMetrics.map((metric) => <code key={metric}>{metric}</code>)}</div>}</article>) : <p className="panel-copy">当前没有可安全生成的修正意图；等待 candidate-bound 视觉证据或真人评审。</p>}</section>
        </div>}
      </aside>
    </section>

    <section className="bottom-rail" aria-label="Codex 操作与版本历史">
      <section className="activity-panel" data-testid="codex-activity"><div className="rail-header"><div><span className="panel-title">Codex 操作</span><span className="panel-subtitle">实时状态</span></div><button type="button" className="text-button" onClick={() => setInspectorTab('evidence')}><Pulse size={14} />查看详情</button></div><div className="activity-list">{activityItems.map((item) => <div className="activity-row" key={item.label}><span className={`activity-state activity-state-${item.state}`}>{item.state === 'done' ? <Check size={12} /> : item.state === 'warn' ? <WarningCircle size={13} /> : item.state === 'active' ? <CircleNotch size={13} /> : <span />}</span><span className="activity-label">{item.label}</span><span className="activity-detail">{item.detail}</span></div>)}</div></section>
      <section className="versions-panel" data-testid="version-rail"><div className="rail-header"><div><span className="panel-title">版本</span><span className="panel-subtitle">不可变历史</span></div><button type="button" className="text-button" onClick={() => onNavigate?.('check')}><GitBranch size={14} />版本历史</button></div><div className="version-content"><div className="current-version-card"><span className="version-label">当前版本</span><strong>{versionCount > 0 ? `V${versionCount}` : '—'}</strong><span>{project?.record?.head_snapshot_id ? shortHash(project.record.head_snapshot_id) : '暂无已确认版本'}</span></div><div className="version-summary"><span>不可变版本</span><strong>{versionCount}</strong><small>恢复会创建新子版本，不改写历史</small></div></div></section>
    </section>
    {notice && <div className="notice-toast" role="status"><Info size={15} /><span>{notice}</span><button type="button" aria-label="关闭提示" onClick={() => setNotice(null)}><X size={15} /></button></div>}
  </main>
}
