import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react'
import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import type * as THREE from 'three'
import type { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import {
  AGENTIC_STATUS_LABELS,
  agenticGateStatusClass,
  normalizeAgenticDesignProjection,
  unavailableAgenticDesignProjection,
  type AgenticDesignProjection,
  type AgenticMetric,
} from './agentic-design'
import {
  AGENTIC_CHECKPOINT_STATUS_LABELS,
  AGENTIC_RESTORE_APPROVAL_STATUS_LABELS,
  AGENTIC_RESTORE_PREPARE_STATUS_LABELS,
  AGENTIC_RESTORE_STATUS_LABELS,
  normalizeAgenticSessionProjection,
  unavailableAgenticSessionProjection,
  type AgenticSessionBinding,
  type AgenticSessionProjection,
} from './agentic-session'

const VIEWER_SELECTION_CACHE = '__forgecad_selection_state_v1'
const VIEWER_HOVER_CACHE = '__forgecad_hover_state_v1'
const DEFAULT_VIEWPORT_CONTROL_HINT = '左键点击选中 · Shift+左键：框选 · 右键：旋转 · 中键：平移 · 滚轮：缩放 · 1/2/3/4/5/6 快速视角 · Z/X/C 光照 · F 聚焦 · R 重置 · Esc 清选'
const VIEWPORT_KEYBOARD_HINTS: string[] = ['左键：选中对象', 'Shift+左键：框选（按住 Shift+左键拖拽）', '右键：旋转', '中键：平移', '滚轮：缩放', '1：前视角', '2：左视角', '3：顶视角', '4：右视角', '5：后视角', '6：三分之四视角', 'Z/X/C：光照预设', 'F：聚焦', 'R：重置视角', 'Esc：清选']
const VIEWPORT_KEYBOARD_HINTS_NO_FOCUS = ['请先点击视口后再使用键盘快捷键']
const VIEWPORT_KEYBOARD_HINTS_ACTIVE = ['当前：快捷键生效']

async function runtimeInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (typeof tauriInvoke !== 'function') throw new Error('RUNTIME_BRIDGE_UNAVAILABLE')
  try {
    return await tauriInvoke<T>(command, args)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    if (/cannot read properties of undefined.*invoke|tauri|__tauri|not a function/i.test(message)) {
      throw new Error('RUNTIME_BRIDGE_UNAVAILABLE')
    }
    throw error
  }
}

type ThreeRuntimeCore = typeof import('./three-runtime-core')
type ThreeRuntimeCoreMath = Pick<ThreeRuntimeCore, 'Box3' | 'Vector3'>

type SceneTreeMaterialSummary = {
  materialZoneId: string
  objectCount: number
}

type SceneTreePartSummary = {
  partId: string
  objectCount: number
  materials: SceneTreeMaterialSummary[]
}

type SceneTreeNodeType = 'part' | 'material'

type SceneTreeNavigationNode = {
  id: string
  type: SceneTreeNodeType
  partId: string
  materialZoneId?: string
}

const SCENE_TREE_PART_NODE_PREFIX = 'part'
const SCENE_TREE_MATERIAL_NODE_PREFIX = 'material'

const sceneTreePartNodeId = (partId: string): string => `${SCENE_TREE_PART_NODE_PREFIX}:${partId}`
const sceneTreeMaterialNodeId = (partId: string, materialZoneId: string): string => `${SCENE_TREE_MATERIAL_NODE_PREFIX}:${partId}:${materialZoneId}`

type ViewerProject = {
  project?: { project_id?: string; name?: string }
  record?: { head_snapshot_id?: string | null }
  head_snapshot?: {
    snapshot_id?: string
    revision?: number
    manifest_hash?: string
    candidate_id?: string | null
  } | null
  versions?: unknown[]
  candidates?: Array<{
    candidate?: {
      candidate_id?: string
      project_id?: string
      state?: string
      canonical_sha256?: string
      quality_hard_gate_passed?: boolean
      created_at?: string
      updated_at?: string
    }
    artifact?: {
      artifact_id?: string
      candidate_id?: string
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
}

type ArtifactBytes = {
  status?: string
  code?: string
  artifact_id?: string
  candidate_id?: string
  reference_id?: string
  project_id?: string
  mime?: string
  width?: number
  height?: number
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
  candidate_id?: string
  artifact_sha256?: string
  program_sha256?: string
  reference_id?: string | null
  reference_sha256?: string | null
  render_set_hash?: string
  comparison_report_hash?: string
  quality_report_hash?: string
  visual_status?: string
  hard_gate_passed?: boolean
}

type ViewerVisualEvidence = {
  status?: string
  code?: string
  candidate_id?: string
  project_id?: string
  reference_id?: string
  render_set_hash?: string
  comparison_report_hash?: string | null
  quality_report_hash?: string
  quality_report?: QualityReport
  render_set?: {
    candidate_id?: string
    artifact_sha256?: string
    reference_id?: string
  } | null
  comparison_report?: {
    candidate_id?: string
    artifact_sha256?: string
    reference_id?: string
    reference_sha256?: string
    render_set_hash?: string
    status?: string
    metrics?: Record<string, number>
  } | null
}

type RenderPass = {
  status?: string
  code?: string
  candidate_id?: string
  render_set_hash?: string
  mime?: string
  png_base64?: string
  sha256?: string
  pass?: string
}

type CandidateView = NonNullable<ViewerProject['candidates']>[number]

type CandidateGenerationTiming = {
  candidateId: string
  state: string
  createdAtText: string | null
  durationSource: 'updated_at' | 'live' | null
  elapsedSeconds: number | null
  elapsedDisplay: string | null
  statusLabel: string
  statusClass: 'passed' | 'failed' | 'not-run'
  anomaly: boolean
  qualityGate: '通过' | '未通过' | '待绑定'
  artifactReady: boolean
  compareReady: boolean
  qualityReady: boolean
}

type CandidateSortMode = 'time' | 'id'

type SceneTreeVisibilityFilter = 'all' | 'visible' | 'locked'

type SnapshotBindingState = {
  artifact: boolean
  reference: boolean
  renderSet: boolean
  comparison: boolean
  qualityReport: boolean
  qualityGate: 'pass' | 'fail' | 'pending'
}

type CandidateSnapshotDelta = {
  label: string
  value: string
  direction: 'up' | 'down' | 'same' | 'unknown'
}

type CandidateSnapshotRecord = {
  candidateId: string
  candidateName: string
  candidateState: string
  candidateCanonicalSha256: string | null
  createdAtText: string | null
  createdAtEpochMs: number | null
  updatedAtText: string | null
  updatedAtEpochMs: number | null
  partCount: number
  partIdSignature: string
  materialZoneCount: number
  materialZoneSignature: string
  triangleCount: number
  programSha256: string | null
  artifactId: string | null
  artifactSha256: string | null
  referenceId: string | null
  referenceSha256: string | null
  renderSetHash: string | null
  comparisonReportHash: string | null
  qualityReportHash: string | null
  visualStatus: string
  qualityPass: boolean
  uvStatus: string
  tangentStatus: string
  validatorStatus: string
  artifact: CandidateView['artifact']
  quality: CandidateView['quality']
  reference: CandidateView['reference']
  hasArtifactBinding: boolean
  hasReferenceBinding: boolean
  hasComparisonBinding: boolean
  hasVisualEvidenceBinding: boolean
}

type CandidateSnapshotDiffRow = {
  label: string
  current: string
  previous: string
  status: 'changed' | 'same' | 'missing'
}

type ErrorConsoleCategory = '读取失败' | '加载失败' | '绑定不一致' | '数据未可用' | '未就绪' | '异常'

type ErrorConsoleItem = {
  id: string
  scope: string
  code: string
  title: string
  category: ErrorConsoleCategory
  summary: string
  meaning: string
  severity: 'error' | 'warn'
  actionLabel?: string
  action?: () => void
}

function deriveErrorCategory(code: string, fallback: ErrorConsoleCategory): ErrorConsoleCategory {
  if (/binding|mismatch/i.test(code)) return '绑定不一致'
  if (/bridge|tauri|host/i.test(code)) return '数据未可用'
  if (/unavailable|missing|empty|not[-_ ]?ready/i.test(code)) return '数据未可用'
  if (/glb|artifact|render|reference|parse|decode|load|bytes/i.test(code)) return '加载失败'
  if (/request|request_failed|sync|summary/i.test(code)) return '读取失败'
  if (/compare|evidence|runtime|permission/i.test(code)) return '读取失败'
  return fallback
}

function compactHash(value: string | null | undefined, length = 12): string {
  if (!value) return '未绑定'
  if (value.length <= length) return value
  const side = Math.max(3, Math.floor((length - 1) / 2))
  return `${value.slice(0, side)}…${value.slice(-side)}`
}

function compactSignature(value: string | null | undefined, length = 12): string {
  if (!value) return '无'
  return compactHash(value, length)
}

function buildCandidateSnapshotRecord(entry: CandidateView, projectId?: string): CandidateSnapshotRecord | null {
  const candidate = entry.candidate
  if (!candidate?.candidate_id) return null
  const artifact = entry.artifact
  const quality = entry.quality
  const reference = entry.reference?.reference
  const createdAtText = buildGenerationTimestamp(candidate.created_at)
  const updatedAtText = buildGenerationTimestamp(candidate.updated_at)
  const artifactId = artifact?.artifact_id ?? null
  const artifactSha256 = artifact?.artifact_id ?? null
  const referenceId = reference?.reference_id ?? null
  const referenceSha256 = reference?.object_sha256 ?? null
  const hasArtifactBinding = hasCandidateBoundArtifact(entry, projectId)
  const candidateId = candidate.candidate_id
  const hasReferenceBinding = hasCandidateBoundReference(entry, projectId)
  const hasComparisonBinding = hasCandidateBoundComparison(entry, projectId)
  const visualEvidenceBinding = hasCandidateBoundVisualEvidence(entry, projectId)
  const qualityReport = quality
  const qualityVisualStatus = qualityReport?.visual_status ?? 'not-run'
  const hardGatePassed = qualityReport?.hard_gate_passed ?? null

  return {
    candidateId: candidate.candidate_id,
    candidateName: candidate.candidate_id,
    candidateState: candidate.state ?? '未知',
    candidateCanonicalSha256: candidate.canonical_sha256 ?? null,
    createdAtText,
    createdAtEpochMs: parseEpochMillis(candidate.created_at),
    updatedAtText,
    updatedAtEpochMs: parseEpochMillis(candidate.updated_at),
    partCount: artifact?.part_ids?.length ?? 0,
    partIdSignature: [...(artifact?.part_ids ?? [])].sort().join('|'),
    materialZoneCount: artifact?.material_zone_ids?.length ?? 0,
    materialZoneSignature: [...(artifact?.material_zone_ids ?? [])].sort().join('|'),
    triangleCount: artifact?.triangle_count ?? 0,
    programSha256: artifact?.program_sha256 ?? null,
    artifactId,
    artifactSha256,
    referenceId,
    referenceSha256,
    renderSetHash: qualityReport?.render_set_hash ?? null,
    comparisonReportHash: qualityReport?.comparison_report_hash ?? null,
    qualityReportHash: qualityReport?.quality_report_hash ?? null,
    visualStatus: qualityVisualStatus,
    qualityPass: hardGatePassed === true,
    uvStatus: artifact?.uv_status ?? '未知',
    tangentStatus: artifact?.tangent_status ?? '未知',
    validatorStatus: artifact?.validator_status ?? '未知',
    artifact,
    quality,
    reference: entry.reference,
    hasArtifactBinding,
    hasReferenceBinding,
    hasComparisonBinding,
    hasVisualEvidenceBinding: visualEvidenceBinding,
  }
}

function buildSnapshotDiffRows(current: CandidateSnapshotRecord, previous: CandidateSnapshotRecord | null): CandidateSnapshotDiffRow[] {
  if (!previous) {
    return [
      { label: '候选状态', current: current.candidateState, previous: '—', status: 'missing' },
      { label: '结构哈希', current: compactHash(current.candidateCanonicalSha256), previous: '—', status: 'missing' },
      { label: 'Part 数', current: String(current.partCount), previous: '—', status: 'missing' },
      { label: '部件清单', current: compactSignature(current.partIdSignature), previous: '—', status: 'missing' },
      { label: '材质区数', current: String(current.materialZoneCount), previous: '—', status: 'missing' },
      { label: '材质区清单', current: compactSignature(current.materialZoneSignature), previous: '—', status: 'missing' },
      { label: '三角形', current: String(current.triangleCount), previous: '—', status: 'missing' },
    ]
  }

  const build = (label: string, left: string | null | undefined, right: string | null | undefined): CandidateSnapshotDiffRow => {
    const currentValue = left ?? '未知'
    const previousValue = right ?? '未知'
    const status: CandidateSnapshotDiffRow['status'] = currentValue === previousValue ? 'same' : 'changed'
    return { label, current: currentValue, previous: previousValue, status }
  }

  const visualCurrent = formatQualityStatus(current.visualStatus)
  const visualPrevious = formatQualityStatus(previous.visualStatus)
  const bindCurrent = current.hasVisualEvidenceBinding ? '已绑定' : '未绑定'
  const bindPrevious = previous.hasVisualEvidenceBinding ? '已绑定' : '未绑定'

  return [
    build('候选状态', current.candidateState, previous.candidateState),
    build('结构哈希', compactHash(current.candidateCanonicalSha256), compactHash(previous.candidateCanonicalSha256)),
    build('几何程序哈希', compactHash(current.programSha256), compactHash(previous.programSha256)),
    build('Part 数', current.partCount === 0 ? '0' : String(current.partCount), previous.partCount === 0 ? '0' : String(previous.partCount)),
    build('部件清单', compactSignature(current.partIdSignature), compactSignature(previous.partIdSignature)),
    build('材质区数', String(current.materialZoneCount), String(previous.materialZoneCount)),
    build('材质区清单', compactSignature(current.materialZoneSignature), compactSignature(previous.materialZoneSignature)),
    build('三角形', String(current.triangleCount), String(previous.triangleCount)),
    build('UV 状态', current.uvStatus, previous.uvStatus),
    build('切线状态', current.tangentStatus, previous.tangentStatus),
    build('Validator', current.validatorStatus, previous.validatorStatus),
    {
      label: '可视化状态',
      current: visualCurrent,
      previous: visualPrevious,
      status: visualCurrent === visualPrevious ? 'same' : 'changed',
    },
    build('质量门', current.hasVisualEvidenceBinding ? (current.qualityPass ? '通过' : '未通过') : '待绑定',
      previous.hasVisualEvidenceBinding ? (previous.qualityPass ? '通过' : '未通过') : '待绑定'),
    {
      label: '证据绑定',
      current: bindCurrent,
      previous: bindPrevious,
      status: bindCurrent === bindPrevious ? 'same' : 'changed',
    },
    {
      label: '引用比对',
      current: current.referenceId ? '已加载' : '未加载',
      previous: previous.referenceId ? '已加载' : '未加载',
      status: current.referenceId === previous.referenceId ? 'same' : 'changed',
    },
  ]
}

function buildCandidateSnapshotBindingState(record: CandidateSnapshotRecord): SnapshotBindingState {
  return {
    artifact: record.hasArtifactBinding && Boolean(record.artifactId),
    reference: record.hasReferenceBinding && Boolean(record.referenceId),
    renderSet: record.hasComparisonBinding && Boolean(record.renderSetHash),
    comparison: record.hasComparisonBinding && Boolean(record.comparisonReportHash),
    qualityReport: record.hasVisualEvidenceBinding && Boolean(record.qualityReportHash),
    qualityGate: record.hasVisualEvidenceBinding ? (record.qualityPass ? 'pass' : 'fail') : 'pending',
  }
}

function snapshotBindingStatusText(state: SnapshotBindingState): string {
  return `${state.artifact ? 'G' : '×'} ${state.reference ? 'R' : '×'} ${state.renderSet ? 'A' : '×'} ${state.comparison ? 'C' : '×'} ${state.qualityReport ? 'Q' : '×'}`
}

function numericDeltaStatus(current: number, previous: number | null): CandidateSnapshotDelta['direction'] {
  if (previous === null) return 'unknown'
  if (current > previous) return 'up'
  if (current < previous) return 'down'
  return 'same'
}

function buildSnapshotMetricDelta(current: number, previous: number | null, label: string): CandidateSnapshotDelta {
  return {
    label,
    value: `${current}${previous === null ? '' : `（${previous}）`}`,
    direction: numericDeltaStatus(current, previous),
  }
}

function buildSnapshotTextMetricDelta(current: string, previous: string | null, label: string): CandidateSnapshotDelta {
  return {
    label,
    value: `${current}${previous === null ? '' : `（${previous}）`}`,
    direction: current === previous ? 'same' : 'unknown',
  }
}

function completedSnapshotDurationSeconds(record: CandidateSnapshotRecord): number | null {
  if (record.createdAtEpochMs === null || record.updatedAtEpochMs === null) return null
  const elapsedSeconds = Math.floor((record.updatedAtEpochMs - record.createdAtEpochMs) / 1000)
  return Number.isFinite(elapsedSeconds) && elapsedSeconds >= 0 ? elapsedSeconds : null
}

function buildSnapshotDurationDelta(current: CandidateSnapshotRecord, previous: CandidateSnapshotRecord): CandidateSnapshotDelta {
  const currentSeconds = completedSnapshotDurationSeconds(current)
  const previousSeconds = completedSnapshotDurationSeconds(previous)
  return {
    label: '完成耗时',
    value: `${currentSeconds === null ? '时间缺失' : formatGenerationDurationFromSeconds(currentSeconds)}（${previousSeconds === null ? '时间缺失' : formatGenerationDurationFromSeconds(previousSeconds)}）`,
    direction: currentSeconds === null || previousSeconds === null ? 'unknown' : numericDeltaStatus(currentSeconds, previousSeconds),
  }
}

function snapshotStatusClass(status: CandidateSnapshotDiffRow['status']): string {
  if (status === 'changed') return 'snapshot-diff-changed'
  if (status === 'missing') return 'workflow-gate-status-not-run'
  return 'workflow-gate-status-passed'
}

function snapshotDeltaClass(direction: CandidateSnapshotDelta['direction']): string {
  if (direction === 'up') return 'snapshot-delta-increase'
  if (direction === 'down') return 'snapshot-delta-decrease'
  if (direction === 'unknown') return 'snapshot-delta-unknown'
  return 'snapshot-delta-same'
}

function formatBindingStatusText(binding: SnapshotBindingState): string {
  const chunks = [
    binding.artifact ? 'GLB：已绑定' : 'GLB：未绑定',
    binding.reference ? '参考：已绑定' : '参考：未绑定',
    binding.renderSet ? '渲染集：已绑定' : '渲染集：未绑定',
    binding.comparison ? '比对：已绑定' : '比对：未绑定',
    binding.qualityReport ? '质量报告：已绑定' : '质量报告：未绑定',
    binding.qualityGate === 'pass' ? '质量门：通过' : binding.qualityGate === 'fail' ? '质量门：未通过' : '质量门：待确认',
  ]
  return chunks.join(' ｜ ')
}

function normalizeStatusText(status: string): string {
  const normalized = status.trim().toLowerCase()
  if (normalized === 'pass' || normalized === 'passed' || normalized === 'quality_pass') return '通过'
  if (normalized === 'fail' || normalized === 'failed' || normalized === 'quality_fail' || normalized.includes('target_not_met')) return '未通过'
  if (normalized === 'not-run' || normalized === 'not_run' || normalized === 'unavailable' || normalized === '') return '未运行'
  if (normalized.includes('partial') && normalized.includes('pass')) return '部分通过'
  if (normalized.includes('pending') || normalized.includes('running') || normalized.includes('queued') || normalized.includes('in_progress')) return '进行中'
  return '未知'
}

function formatCandidateState(state: string): string {
  const normalized = state.trim().toLowerCase().replaceAll('_', '-')
  const labels: Record<string, string> = {
    prepared: '已准备',
    compiling: '编译中',
    evaluating: '评估中',
    reviewable: '可评审',
    confirmed: '已确认',
    rejected: '已拒绝',
    failed: '失败',
    expired: '已过期',
    pending: '等待中',
    queued: '排队中',
    running: '运行中',
    processing: '处理中',
    'in-progress': '进行中',
    staged: '已暂存',
    unknown: '未知',
  }
  return labels[normalized] ?? normalizeStatusText(state)
}

function deriveCandidateErrorMeaning(code: string): string {
  if (code === 'ARTIFACT_BYTES_UNAVAILABLE') return '未能读取 GLB 资产。可能资产尚未生成完成或未与候选绑定。'
  if (code === 'ARTIFACT_BYTES_BINDING_MISMATCH') return 'GLB 绑定到候选 ID 或 SHA 发生不一致，候选已失配。'
  if (code === 'REFERENCE_BYTES_UNAVAILABLE') return '参考图读取不可用，请确认该候选有有效的可访问 reference。'
  if (code === 'RENDER_PASS_UNAVAILABLE') return 'RenderPass PNG 缺失或 AOV pass 未生成。'
  if (code === 'REFERENCE_BYTES_BINDING_MISMATCH') return '候选绑定的 reference 不一致，当前对比不能直接判定。'
  if (code === 'RENDER_PASS_BINDING_MISMATCH') return 'RenderPass 与当前 RenderSet 不一致。'
  if (code === 'GLB_PARSE_FAILED') return 'GLB 解析失败，模型文件可能损坏或不完整。'
  if (code === 'COMPARE_IMAGE_DATA_UNAVAILABLE') return '比较图像数据不可用，暂无法生成差异视图。'
  if (code === 'COMPARE_RESULT_IMAGE_UNAVAILABLE') return '比较 Worker 已返回，但 Viewer 无法生成临时显示图像。'
  if (code === 'COMPARE_WORKER_UNAVAILABLE') return '当前环境无法启动比较计算 Worker；可重试或关闭热图/轮廓辅助。'
  if (code === 'COMPARE_WORKER_FAILED' || code === 'DIFFERENCE_HEATMAP_FAILED') return '差异热图计算 Worker 失败；原始 AOV 与 Runtime QualityReport 不受影响。'
  if (code === 'GLB_EMPTY_SCENE') return 'GLB 场景为空。'
  if (code === 'REFERENCE_CONTOUR_FAILED' || code === 'REFERENCE_CONTOUR_IMAGE_DATA_UNAVAILABLE') return '参考轮廓辅助计算失败；原始参考图与 Runtime 质量门不受影响。'
  if (code === 'VISUAL_EVIDENCE_BINDING_MISMATCH') return '候选的 VisualEvidence 与当前候选/哈希不一致。'
  if (code === 'VISUAL_EVIDENCE_UNAVAILABLE') return 'Quality/对比证据暂不可读取。'
  if (code === 'RUNTIME_BRIDGE_UNAVAILABLE') return '当前页面没有连接 ForgeCAD Desktop 的本地 Runtime。请在 Tauri Desktop 中打开工作台，或确认应用桥接后重试。'
  if (code === 'RUNTIME_SUMMARY_UNAVAILABLE') return '模型摘要服务暂时不可用。'
  if (code === 'RUNTIME_REQUEST_FAILED' || code === 'RUNTIME_SUMMARY_REQUEST_FAILED') return 'Runtime 读取失败，当前状态可能为历史缓存。'
  return '请检查 Runtime 会话与当前候选绑定是否可用。'
}

function isCandidateInProgress(state?: string): boolean {
  if (!state) return false
  const normalized = state.toLowerCase()
  return /(pending|queued|running|processing|in[-_ ]?progress|draft|staged|building|submit|waiting)/.test(normalized)
}

function summaryPollDelaySeconds(opts: { changed: boolean; hasActiveCandidates: boolean; hasError: boolean; isVisible: boolean; firstRun: boolean }): number {
  if (!opts.isVisible) return 30000
  if (opts.changed || opts.hasError || opts.firstRun) return 2000
  if (opts.hasActiveCandidates) return 4500
  return 15000
}

function statusClassFromCode(code: string, isWarning?: boolean): 'error' | 'warn' {
  if (isWarning) return 'warn'
  const normalized = code.toLowerCase()
  if (normalized.includes('unavailable') || normalized.includes('not-run') || normalized === 'not_run') return 'warn'
  return 'error'
}

function hasCandidateBoundArtifact(entry: CandidateView, projectId?: string): boolean {
  const candidateId = entry.candidate?.candidate_id
  return Boolean(
    candidateId
      && projectId
      && entry.candidate?.project_id === projectId
      && entry.artifact?.artifact_id
      && entry.artifact.candidate_id === candidateId,
  )
}

function hasCandidateBoundReference(entry: CandidateView, projectId?: string): boolean {
  const candidateId = entry.candidate?.candidate_id
  const reference = entry.reference?.reference
  return Boolean(
    candidateId
      && projectId
      && entry.candidate?.project_id === projectId
      && reference?.reference_id
      && reference.project_id === projectId
      && reference.object_sha256,
  )
}

function hasCandidateBoundComparison(entry: CandidateView, projectId?: string): boolean {
  const candidateId = entry.candidate?.candidate_id
  const quality = entry.quality
  const reference = entry.reference?.reference
  const artifactId = entry.artifact?.artifact_id
  return Boolean(
    candidateId
      && projectId
      && entry.candidate?.project_id === projectId
      && hasCandidateBoundArtifact(entry, projectId)
      && hasCandidateBoundReference(entry, projectId)
      && quality?.candidate_id === candidateId
      && quality.artifact_sha256 === artifactId
      && quality.reference_id === reference?.reference_id
      && quality.reference_sha256 === reference?.object_sha256
      && quality.render_set_hash
      && quality.comparison_report_hash,
  )
}

function hasCandidateBoundVisualEvidence(entry: CandidateView, projectId?: string): boolean {
  const candidateId = entry.candidate?.candidate_id
  const quality = entry.quality
  return Boolean(
    candidateId
      && quality?.quality_report_hash
      && hasCandidateBoundComparison(entry, projectId),
  )
}

function isCandidateBoundVisualEvidence(
  payload: ViewerVisualEvidence | null | undefined,
  candidateId?: string,
  projectId?: string,
  artifactId?: string,
  referenceSha256?: string,
): payload is ViewerVisualEvidence {
  const quality = payload?.quality_report
  const renderSet = payload?.render_set
  const comparison = payload?.comparison_report
  return Boolean(
    payload
      && candidateId
      && projectId
      && artifactId
      && referenceSha256
      && payload.candidate_id === candidateId
      && payload.project_id === projectId
      && payload.reference_id
      && payload.render_set_hash
      && payload.comparison_report_hash
      && payload.quality_report_hash
      && quality?.candidate_id === candidateId
      // QualityReport@2 has no project_id field. Its project binding is the
      // authenticated ViewerVisualEvidence envelope plus the candidate and
      // reference projection validated above.
      && quality?.artifact_sha256 === artifactId
      && quality?.reference_id === payload.reference_id
      && quality?.reference_sha256 === referenceSha256
      && quality?.render_set_hash === payload.render_set_hash
      && quality?.comparison_report_hash === payload.comparison_report_hash
      && renderSet?.candidate_id === candidateId
      && renderSet?.artifact_sha256 === artifactId
      && renderSet?.reference_id === payload.reference_id
      && comparison?.candidate_id === candidateId
      && comparison?.artifact_sha256 === artifactId
      && comparison?.reference_id === payload.reference_id
      && comparison?.reference_sha256 === referenceSha256
      && comparison?.render_set_hash === payload.render_set_hash,
  )
}

function isCandidateBoundArtifactPayload(
  payload: ArtifactBytes | null | undefined,
  artifactId?: string,
  candidateId?: string,
  artifactSha256?: string | null,
): boolean {
  return Boolean(
    payload
      && artifactId
      && candidateId
      && payload.artifact_id === artifactId
      && payload.candidate_id === candidateId
      && (!artifactSha256 || payload.sha256 === artifactSha256 || payload.sha256 === artifactId),
  )
}

function isCandidateBoundReferencePayload(
  payload: ArtifactBytes | null | undefined,
  referenceId?: string,
  projectId?: string,
  referenceSha256?: string,
): boolean {
  return Boolean(
    payload
      && referenceId
      && projectId
      && payload.reference_id === referenceId
      && payload.project_id === projectId
      && payload.sha256 === referenceSha256,
  )
}

function isCandidateBoundRenderPayload(
  payload: RenderPass | null | undefined,
  candidateId?: string,
  renderSetHash?: string,
  pass?: string,
): boolean {
  return Boolean(
    payload
      && candidateId
      && renderSetHash
      && payload.candidate_id === candidateId
      && payload.render_set_hash === renderSetHash
      && payload.pass === pass,
  )
}

const AOV_PASSES = ['beauty', 'silhouette', 'depth', 'normal', 'ao', 'part-id', 'material-id', 'wireframe', 'uv-stretch'] as const
type AovPass = typeof AOV_PASSES[number]
type CompareMode = 'split' | 'overlay' | 'flicker'
type ViewportLightPreset = 'neutral' | 'high-key' | 'dramatic'
type ViewportDragMode = 'idle' | 'box-select'
type ViewportCameraPreset = 'front' | 'left' | 'top' | 'right' | 'rear' | 'three-quarter'

const VIEWPORT_LIGHT_PRESETS: ReadonlyArray<{ id: ViewportLightPreset; label: string }> = [
  { id: 'neutral', label: '中性光' },
  { id: 'high-key', label: '高亮光' },
  { id: 'dramatic', label: '轮廓光' },
]

const VIEWPORT_CAMERA_PRESETS: ReadonlyArray<{ id: ViewportCameraPreset; label: string }> = [
  { id: 'front', label: '1 前' },
  { id: 'left', label: '2 左' },
  { id: 'top', label: '3 顶' },
  { id: 'right', label: '4 右' },
  { id: 'rear', label: '5 后' },
  { id: 'three-quarter', label: '6 三分之四' },
]

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
  isSelected: boolean
}

type ViewerSceneState = {
  root: THREE.Object3D
  renderer: THREE.WebGLRenderer
  scene: THREE.Scene
  camera: THREE.PerspectiveCamera
  controls: OrbitControls
  raycaster: THREE.Raycaster
  objects: Map<THREE.Object3D, ViewerObjectState>
  lights: {
    hemi?: THREE.HemisphereLight
    key?: THREE.DirectionalLight
    fill?: THREE.DirectionalLight
    rim?: THREE.DirectionalLight
  }
}

type ViewerCandidateSummary = {
  candidate_id?: string
  project_id?: string
  state?: string
  canonical_sha256?: string
  quality_hard_gate_passed?: boolean
  created_at?: string
  updated_at?: string
}

type ViewerModelSummary = {
  status: 'Ready' | 'Unavailable'
  retryable: boolean
  projects: Array<{
    project?: { project_id?: string; name?: string }
    record?: { head_snapshot_id?: string | null }
    versions_count?: number
    candidates?: ViewerCandidateSummary[]
  }>
  code?: string
}

type LoadState = 'idle' | 'loading' | 'ready' | 'error'
type CompareActionStatus = 'idle' | 'exporting' | 'exported' | 'unavailable'
type Point = { x: number; y: number }
type ViewportMarqueeRect = { left: number; top: number; width: number; height: number }

const AUTO_LATEST_CANDIDATE = '__auto_latest__'

const HEATMAP_SIZE = 512
const COMPARE_MODE_LABELS: Record<CompareMode, string> = {
  split: '分屏',
  overlay: '叠加',
  flicker: '闪烁',
}
const AOV_PASS_LABELS: Record<AovPass, string> = {
  beauty: 'beauty',
  silhouette: '轮廓',
  depth: '深度',
  normal: '法线',
  ao: 'AO',
  'part-id': '部件ID',
  'material-id': '材质ID',
  wireframe: '线框',
  'uv-stretch': 'UV拉伸',
}

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

function createContainedImageData(image: HTMLImageElement): ImageData | null {
  const canvas = document.createElement('canvas')
  canvas.width = HEATMAP_SIZE
  canvas.height = HEATMAP_SIZE
  const context = canvas.getContext('2d', { willReadFrequently: true })
  if (!context) return null
  context.fillStyle = '#000'
  context.fillRect(0, 0, HEATMAP_SIZE, HEATMAP_SIZE)
  drawContainedImage(context, image)
  return context.getImageData(0, 0, HEATMAP_SIZE, HEATMAP_SIZE)
}

type CompareWorkerRequest = {
  kind: 'difference' | 'contour'
  width: number
  height: number
  referenceBuffer: ArrayBuffer
  renderBuffer?: ArrayBuffer
  sensitivity?: number
}

type CompareWorkerResponse = {
  id: string
  ok: boolean
  width: number
  height: number
  buffer?: ArrayBuffer
  error?: string
}

function imageDataToDataUrl(result: { buffer: ArrayBuffer; width: number; height: number }): string | null {
  const canvas = document.createElement('canvas')
  canvas.width = result.width
  canvas.height = result.height
  const context = canvas.getContext('2d')
  if (!context) return null
  context.putImageData(new ImageData(new Uint8ClampedArray(result.buffer), result.width, result.height), 0, 0)
  return canvas.toDataURL('image/png')
}

function runCompareWorker(
  request: CompareWorkerRequest,
  onResult: (result: { buffer: ArrayBuffer; width: number; height: number }) => void,
  onError: (code: string) => void,
): () => void {
  if (typeof Worker === 'undefined') {
    onError('COMPARE_WORKER_UNAVAILABLE')
    return () => undefined
  }
  const worker = new Worker(new URL('./compare-worker.ts', import.meta.url), { type: 'module' })
  const id = `${request.kind}-${Date.now()}-${Math.random().toString(16).slice(2)}`
  let active = true
  worker.onmessage = (event: MessageEvent<CompareWorkerResponse>) => {
    if (!active || event.data.id !== id) return
    if (!event.data.ok || !event.data.buffer) onError(event.data.error ?? 'COMPARE_WORKER_FAILED')
    else onResult({ buffer: event.data.buffer, width: event.data.width, height: event.data.height })
    worker.terminate()
  }
  worker.onerror = () => {
    if (!active) return
    onError('COMPARE_WORKER_FAILED')
    worker.terminate()
  }
  const transferables: Transferable[] = [request.referenceBuffer]
  if (request.renderBuffer) transferables.push(request.renderBuffer)
  worker.postMessage({ ...request, id }, transferables)
  return () => {
    active = false
    worker.terminate()
  }
}

function decodeImage(dataUrl: string): Promise<HTMLImageElement> {
  const image = new Image()
  image.src = dataUrl
  return image.decode().then(() => image)
}

function drawComparisonImage(
  context: CanvasRenderingContext2D,
  image: HTMLImageElement,
  x: number,
  y: number,
  width: number,
  height: number,
  zoom: number,
  pan: { x: number; y: number },
  opacity: number,
  brightness: number,
) {
  const naturalWidth = image.naturalWidth || width
  const naturalHeight = image.naturalHeight || height
  const scale = Math.min(width / naturalWidth, height / naturalHeight) * zoom
  const drawWidth = naturalWidth * scale
  const drawHeight = naturalHeight * scale
  const offsetX = x + (width - drawWidth) / 2 + pan.x
  const offsetY = y + (height - drawHeight) / 2 + pan.y
  context.save()
  context.globalAlpha = opacity
  context.filter = `brightness(${brightness})`
  context.drawImage(image, offsetX, offsetY, drawWidth, drawHeight)
  context.restore()
}

async function createCompareSnapshot(options: {
  referenceDataUrl: string
  renderDataUrl: string
  mode: CompareMode
  flickerOn: boolean
  zoom: number
  pan: { x: number; y: number }
  referenceOpacity: number
  renderOpacity: number
  brightness: number
  heatmapDataUrl?: string
}): Promise<string> {
  const [reference, render] = await Promise.all([decodeImage(options.referenceDataUrl), decodeImage(options.renderDataUrl)])
  const heatmap = options.heatmapDataUrl ? await decodeImage(options.heatmapDataUrl) : null
  const canvas = document.createElement('canvas')
  canvas.width = 1200
  canvas.height = 760
  const context = canvas.getContext('2d')
  if (!context) throw new Error('COMPARE_EXPORT_CANVAS_UNAVAILABLE')
  context.fillStyle = '#060a10'
  context.fillRect(0, 0, canvas.width, canvas.height)
  const imageHeight = canvas.height - 36
  if (options.mode === 'split') {
    drawComparisonImage(context, reference, 0, 36, canvas.width / 2, imageHeight, options.zoom, options.pan, 1, 1)
    drawComparisonImage(context, render, canvas.width / 2, 36, canvas.width / 2, imageHeight, options.zoom, options.pan, 1, options.brightness)
  } else if (options.mode === 'flicker') {
    drawComparisonImage(context, options.flickerOn ? render : reference, 0, 36, canvas.width, imageHeight, options.zoom, options.pan, 1, options.flickerOn ? options.brightness : 1)
  } else {
    drawComparisonImage(context, reference, 0, 36, canvas.width, imageHeight, options.zoom, options.pan, options.referenceOpacity, 1)
    drawComparisonImage(context, render, 0, 36, canvas.width, imageHeight, options.zoom, options.pan, options.renderOpacity, options.brightness)
  }
  if (heatmap) drawComparisonImage(context, heatmap, 0, 36, canvas.width, imageHeight, options.zoom, options.pan, 1, 1)
  context.fillStyle = '#cfe2f2'
  context.font = '12px system-ui, sans-serif'
  context.fillText(`ForgeCAD compare · ${options.mode}`, 14, 22)
  return canvas.toDataURL('image/png')
}

function parseEpochMillis(timestamp: string | number | null | undefined): number | null {
  if (timestamp == null || timestamp === '') return null
  if (typeof timestamp === 'number') {
    if (!Number.isFinite(timestamp) || timestamp <= 0) return null
    return timestamp < 1e11 ? timestamp * 1000 : timestamp
  }
  const text = String(timestamp).trim()
  if (!text) return null
  const parsed = Number(text)
  if (Number.isFinite(parsed) && parsed > 0) return parsed < 1e11 ? parsed * 1000 : parsed
  const parsedDate = new Date(text)
  if (Number.isNaN(parsedDate.getTime())) return null
  return parsedDate.getTime()
}

function buildGenerationTimestamp(timestamp?: string | number | null): string | null {
  const epochMs = parseEpochMillis(timestamp)
  if (!epochMs) return null
  const date = new Date(epochMs)
  return date.toLocaleString('zh-CN', { hour12: false })
}

function formatGenerationDurationFromSeconds(totalSecondsInput: number): string {
  if (!Number.isFinite(totalSecondsInput) || totalSecondsInput < 0) return 'invalid'
  const totalSeconds = Math.max(0, Math.floor(totalSecondsInput))
  const seconds = totalSeconds % 60
  const totalMinutes = Math.floor(totalSeconds / 60)
  const minutes = totalMinutes % 60
  const totalHours = Math.floor(totalMinutes / 60)
  const hours = totalHours % 24
  const days = Math.floor(totalHours / 24)
  const segments = []
  if (days > 0) segments.push(`${days}d`)
  if (hours > 0 || segments.length > 0) segments.push(`${hours}h`)
  if (minutes > 0 || segments.length > 0) segments.push(`${minutes}m`)
  segments.push(`${seconds}s`)
  return segments.join(' ')
}

function candidateGenerationOutcome(state: string): 'success' | 'failed' | 'unknown' {
  const normalized = state.trim().toLowerCase()
  if (/(fail|reject|error|cancel|invalid|abort)/.test(normalized)) return 'failed'
  if (/(pending|queued|running|processing|in[-_ ]progress|draft|unknown)/.test(normalized)) return 'unknown'
  return 'success'
}

const EMPTY_MODEL: ViewerModel = { status: 'Unavailable', retryable: true, projects: [] }

function viewerSummarySignature(summary: ViewerModelSummary): string {
  return JSON.stringify(summary.projects.map((project) => ({
    projectId: project.project?.project_id ?? null,
    name: project.project?.name ?? null,
    headSnapshotId: project.record?.head_snapshot_id ?? null,
    versionsCount: project.versions_count ?? 0,
    candidates: [...(project.candidates ?? [])]
      .sort((left, right) => (left.candidate_id ?? '').localeCompare(right.candidate_id ?? ''))
      .map((candidate) => [
        candidate.candidate_id ?? null,
        candidate.state ?? null,
        candidate.canonical_sha256 ?? null,
        candidate.quality_hard_gate_passed ?? null,
        candidate.created_at ?? null,
        candidate.updated_at ?? null,
      ]),
  })))
}

function formatAgenticMetric(metric: AgenticMetric): string {
  const observed = metric.observed ? `observed ${metric.observed}` : null
  const threshold = metric.threshold ? `threshold ${metric.threshold}` : null
  return [metric.name, observed, threshold].filter((value): value is string => Boolean(value)).join(' · ')
}

function agenticSessionStatusClass(status: string): string {
  if (status === 'mismatch' || status === 'failed') return 'failed'
  if (status === 'bound' || status === 'persisted' || status === 'approved') return 'passed'
  if (status === 'locked' || status === 'prepare' || status === 'awaiting-approval') return 'locked'
  return 'not-run'
}

function agenticBindingStatusLabel(status: 'bound' | 'unknown' | 'mismatch'): string {
  if (status === 'bound') return '已绑定'
  if (status === 'mismatch') return '不一致'
  return '未知'
}

function readErrorCode(error: unknown, fallback: string): string {
  if (typeof error === 'string' && error.trim()) return error.trim()
  if (error instanceof Error && error.message.trim()) return error.message.trim()
  if (typeof error === 'object' && error !== null && 'code' in error) {
    const code = (error as { code?: unknown }).code
    if (typeof code === 'string' && code.trim()) return code.trim()
  }
  return fallback
}

function formatQualityStatus(status: string): string {
  const normalized = status.toLowerCase()
  if (normalized === 'pass' || normalized === 'passed' || normalized === 'quality_pass') return '通过'
  if (normalized.includes('partial') && normalized.includes('pass')) return '部分通过'
  if (normalized === 'not-run' || normalized === 'not_run' || normalized === 'unavailable') return '未运行'
  if (normalized.includes('blocked')) return '已阻断'
  if (normalized.includes('fail') || normalized.includes('target_not_met')) return '未通过'
  return '未知'
}

function qualityStatusClass(status: string): 'passed' | 'failed' | 'not-run' | 'partial' {
  const normalized = status.trim().toLowerCase().replaceAll('_', '-')
  if (normalized === 'pass' || normalized === 'passed' || normalized === 'quality-pass') return 'passed'
  if (normalized.includes('partial') && normalized.includes('pass')) return 'partial'
  if (!normalized || normalized === 'not-run' || normalized === 'unavailable' || normalized === 'unknown') return 'not-run'
  return 'failed'
}

type ForgeSelectionSnapshot = {
  hasEmissive: boolean
  emissive?: [number, number, number]
  emissiveIntensity?: number
  color?: [number, number, number]
  roughness?: number
  metalness?: number
}

type ForgeHoverSnapshot = {
  hasEmissive: boolean
  emissive?: [number, number, number]
  emissiveIntensity?: number
}

function applyMaterialSelectionState(material: THREE.Material, isSelected: boolean): void {
  const cache = (material.userData as Record<string, unknown>)[VIEWER_SELECTION_CACHE] as ForgeSelectionSnapshot | undefined
  const anyMaterial = material as {
    emissive?: { toArray?: () => number[]; set?: (...args: unknown[]) => void }
    emissiveIntensity?: number
    color?: { toArray?: () => number[]; set?: (...args: unknown[]) => void }
    roughness?: number
    metalness?: number
  }

  if (!isSelected) {
    if (!cache) return
    if (cache.hasEmissive && anyMaterial.emissive?.set && cache.emissive) {
      anyMaterial.emissive.set(cache.emissive[0], cache.emissive[1], cache.emissive[2])
      if (cache.emissiveIntensity !== undefined && anyMaterial.emissiveIntensity !== undefined) {
        anyMaterial.emissiveIntensity = cache.emissiveIntensity
      }
    }
    if (cache.color && anyMaterial.color?.set) anyMaterial.color.set(cache.color[0], cache.color[1], cache.color[2])
    if (cache.roughness !== undefined && anyMaterial.roughness !== undefined) anyMaterial.roughness = cache.roughness
    if (cache.metalness !== undefined && anyMaterial.metalness !== undefined) anyMaterial.metalness = cache.metalness
    delete (material.userData as Record<string, unknown>)[VIEWER_SELECTION_CACHE]
    return
  }

  if (cache) return
  const next: ForgeSelectionSnapshot = {
    hasEmissive: Boolean(anyMaterial.emissive && anyMaterial.emissive.toArray),
    emissive: anyMaterial.emissive?.toArray?.().map((item) => Number(item.toFixed(6))) as [number, number, number] | undefined,
    emissiveIntensity: anyMaterial.emissiveIntensity,
    color: anyMaterial.color?.toArray?.().map((item) => Number(item.toFixed(6))) as [number, number, number] | undefined,
    roughness: anyMaterial.roughness,
    metalness: anyMaterial.metalness,
  }
  ;(material.userData as Record<string, unknown>)[VIEWER_SELECTION_CACHE] = next
  if (next.hasEmissive && anyMaterial.emissive?.set) {
    anyMaterial.emissive.set(1, 0.74, 0.2)
  }
  if (anyMaterial.emissiveIntensity !== undefined) anyMaterial.emissiveIntensity = Math.max(anyMaterial.emissiveIntensity, 0.9)
  if (anyMaterial.roughness !== undefined) anyMaterial.roughness = Math.max(0, Math.min(0.75, anyMaterial.roughness))
  if (anyMaterial.metalness !== undefined) anyMaterial.metalness = Math.min(0.15, anyMaterial.metalness)
}

function applyMaterialHoverState(material: THREE.Material, isHovered: boolean): void {
  const cache = (material.userData as Record<string, unknown>)[VIEWER_HOVER_CACHE] as ForgeHoverSnapshot | undefined
  const anyMaterial = material as {
    emissive?: { toArray?: () => number[]; set?: (...args: unknown[]) => void }
    emissiveIntensity?: number
  }

  if (!isHovered) {
    if (!cache) return
    if (cache.hasEmissive && anyMaterial.emissive?.set && cache.emissive) {
      anyMaterial.emissive.set(cache.emissive[0], cache.emissive[1], cache.emissive[2])
      if (cache.emissiveIntensity !== undefined && anyMaterial.emissiveIntensity !== undefined) {
        anyMaterial.emissiveIntensity = cache.emissiveIntensity
      }
    }
    delete (material.userData as Record<string, unknown>)[VIEWER_HOVER_CACHE]
    return
  }

  if (cache) return
  const next: ForgeHoverSnapshot = {
    hasEmissive: Boolean(anyMaterial.emissive && anyMaterial.emissive.toArray),
    emissive: anyMaterial.emissive?.toArray?.().map((item) => Number(item.toFixed(6))) as [number, number, number] | undefined,
    emissiveIntensity: anyMaterial.emissiveIntensity,
  }
  ;(material.userData as Record<string, unknown>)[VIEWER_HOVER_CACHE] = next
  if (next.hasEmissive && anyMaterial.emissive?.set) {
    anyMaterial.emissive.set(1, 0.85, 0.25)
  }
  if (anyMaterial.emissiveIntensity !== undefined) {
    anyMaterial.emissiveIntensity = Math.max(anyMaterial.emissiveIntensity, 0.55)
  }
}

function applyObjectSelectionState(object: THREE.Object3D, isSelected: boolean): void {
  if (!(object as THREE.Mesh).isMesh) return
  const mesh = object as THREE.Mesh
  const meshMaterials = Array.isArray(mesh.material) ? mesh.material : [mesh.material]
  for (const material of meshMaterials) {
    if (!material) continue
    applyMaterialSelectionState(material, isSelected)
  }
}

function applyObjectHoverState(object: THREE.Object3D, isHovered: boolean): void {
  if (!(object as THREE.Mesh).isMesh) return
  const mesh = object as THREE.Mesh
  const meshMaterials = Array.isArray(mesh.material) ? mesh.material : [mesh.material]
  for (const material of meshMaterials) {
    if (!material) continue
    applyMaterialHoverState(material, isHovered)
  }
}

function isolateViewerMeshMaterials(mesh: THREE.Mesh): void {
  // GLTFLoader may share a Material across meshes. Selection/hover is an
  // ephemeral Viewer effect, so each mesh needs its own material instance;
  // clone() keeps the underlying textures shared and avoids extra image data.
  if (Array.isArray(mesh.material)) {
    mesh.material = mesh.material.map((material) => material.clone())
  } else {
    mesh.material = mesh.material.clone()
  }
}

function buildSceneTreeSummary(objects: Map<THREE.Object3D, ViewerObjectState>): SceneTreePartSummary[] {
  const partMap = new Map<string, SceneTreePartSummary>()
  for (const objectState of objects.values()) {
    const partId = objectState.partId || 'unknown-part'
    const materialZoneId = objectState.materialZoneId || 'unknown-material-zone'
    const partEntry = partMap.get(partId) ?? {
      partId,
      objectCount: 0,
      materials: [],
    }
    partEntry.objectCount += 1
    const zoneIndex = partEntry.materials.findIndex((zone) => zone.materialZoneId === materialZoneId)
    if (zoneIndex < 0) partEntry.materials.push({ materialZoneId, objectCount: 1 })
    else partEntry.materials[zoneIndex]!.objectCount += 1
    partMap.set(partId, partEntry)
  }
  return [...partMap.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([_, part]) => ({
      ...part,
      materials: [...part.materials].sort((left, right) => left.materialZoneId.localeCompare(right.materialZoneId)),
    }))
}

function isViewportObjectLocked(
  objectState: ViewerObjectState | undefined,
  partLockState: Record<string, boolean>,
  materialLockState: Record<string, boolean>,
): boolean {
  if (!objectState) return false
  return Boolean(partLockState[objectState.partId]) || Boolean(materialLockState[objectState.materialZoneId])
}

function viewportLightPresetValues(preset: ViewportLightPreset) {
  if (preset === 'high-key') {
    return { key: 3.0, fill: 1.6, rim: 0.4 }
  }
  if (preset === 'dramatic') {
    return { key: 1.4, fill: 0.6, rim: 1.6 }
  }
  return { key: 2.4, fill: 1.1, rim: 1.0 }
}

function clampViewportControlHint(preset: ViewportLightPreset, lockState: {
  selectedPartLocked?: boolean
  selectedMaterialLocked?: boolean
}) {
  const lights = preset === 'neutral'
    ? '中性光'
    : preset === 'high-key'
      ? '高亮光'
      : '轮廓光'
  const lockHint = lockState.selectedPartLocked || lockState.selectedMaterialLocked
    ? ' · 当前选中对象已锁定（无法重新拾取）'
    : ''
  return `${DEFAULT_VIEWPORT_CONTROL_HINT} · 当前光照：${lights}${lockHint}`
}

function viewportCameraOffset(preset: ViewportCameraPreset): [number, number, number] {
  if (preset === 'left') return [-1.1, 0.26, 0.15]
  if (preset === 'top') return [0.02, 1.2, 0.22]
  if (preset === 'right') return [1.1, 0.26, -0.15]
  if (preset === 'rear') return [0.08, 0.25, -1.35]
  if (preset === 'three-quarter') return [1.0, 0.45, 0.9]
  return [0.08, 0.25, 1.2]
}

function isPointInsideSelectionRect(point: { x: number; y: number }, left: number, top: number, right: number, bottom: number) {
  return point.x >= left && point.x <= right && point.y >= top && point.y <= bottom
}

function pickViewportObjectsInMarqueeRect(
  state: ViewerSceneState,
  marqueeRect: ViewportMarqueeRect,
  canvasRect: DOMRect,
  partLockState: Record<string, boolean>,
  materialLockState: Record<string, boolean>,
  threeRuntime: ThreeRuntimeCoreMath | null,
): THREE.Object3D[] {
  if (!threeRuntime) return []
  const box = new threeRuntime.Box3()
  const Vector3 = threeRuntime.Vector3
  const vector = new Vector3()
  const camera = state.camera
  const left = (marqueeRect.left - canvasRect.left) / canvasRect.width
  const right = (marqueeRect.left + marqueeRect.width - canvasRect.left) / canvasRect.width
  const top = (marqueeRect.top - canvasRect.top) / canvasRect.height
  const bottom = (marqueeRect.top + marqueeRect.height - canvasRect.top) / canvasRect.height
  const normalized = {
    left: Math.min(left, right),
    right: Math.max(left, right),
    top: Math.min(top, bottom),
    bottom: Math.max(top, bottom),
  }
  const width = normalized.right - normalized.left
  const height = normalized.bottom - normalized.top
  if (width <= 0 || height <= 0) return []
  const hits: THREE.Object3D[] = []
  state.objects.forEach((objectState, object) => {
    if (isViewportObjectLocked(objectState, partLockState, materialLockState)) return
    if (!object.visible) return
    box.setFromObject(object)
    if (box.isEmpty()) return
    const min = box.min
    const max = box.max
    const candidates: THREE.Vector3[] = [
      new Vector3(min.x, min.y, min.z),
      new Vector3(min.x, min.y, max.z),
      new Vector3(min.x, max.y, min.z),
      new Vector3(min.x, max.y, max.z),
      new Vector3(max.x, min.y, min.z),
      new Vector3(max.x, min.y, max.z),
      new Vector3(max.x, max.y, min.z),
      new Vector3(max.x, max.y, max.z),
      box.getCenter(vector),
    ]
    const hit = candidates.some((candidate) => {
      const projected = candidate.project(camera)
      if (projected.z < -1 || projected.z > 1) return false
      const point = { x: (projected.x + 1) / 2, y: (-projected.y + 1) / 2 }
      return isPointInsideSelectionRect(point, normalized.left, normalized.top, normalized.right, normalized.bottom)
    })
    if (hit) hits.push(object)
  })
  return hits
}

function pickViewportObjectFromPointer(
  state: ViewerSceneState,
  event: PointerEvent<HTMLCanvasElement>,
  rect: DOMRect,
  partLockState: Record<string, boolean>,
  materialLockState: Record<string, boolean>,
): THREE.Object3D | null {
  const pointer = {
    x: (event.clientX - rect.left) / rect.width * 2 - 1,
    y: -((event.clientY - rect.top) / rect.height) * 2 + 1,
  }
  state.raycaster.setFromCamera(pointer as unknown as THREE.Vector2, state.camera)
  const intersects = state.raycaster.intersectObjects([...state.objects.keys()], true)
  for (const candidate of intersects) {
    let object: THREE.Object3D | null = candidate.object
    while (object) {
      const data = state.objects.get(object)
      if (data) {
        if (!isViewportObjectLocked(data, partLockState, materialLockState)) return object
        break
      }
      object = object.parent
    }
  }
  return null
}

function disposeObjectResources(root: THREE.Object3D): void {
  const geometries = new Set<THREE.BufferGeometry>()
  const materials = new Set<THREE.Material>()
  const textures = new Set<THREE.Texture>()
  root.traverse((object) => {
    const mesh = object as THREE.Mesh
    if (!mesh.isMesh) return
    if (mesh.geometry && !geometries.has(mesh.geometry)) {
      geometries.add(mesh.geometry)
      mesh.geometry.dispose()
    }
    const meshMaterials = Array.isArray(mesh.material) ? mesh.material : [mesh.material]
    for (const material of meshMaterials) {
      if (!material || materials.has(material)) continue
      materials.add(material)
      const values = Object.values(material as THREE.Material & Record<string, unknown>)
      for (const value of values) {
        if (value && typeof value === 'object' && 'isTexture' in value && (value as { isTexture?: boolean }).isTexture === true) {
          const texture = value as THREE.Texture
          if (!textures.has(texture)) {
            textures.add(texture)
            texture.dispose()
          }
        }
      }
      material.dispose()
    }
  })
}

function disposeViewerScene(state: ViewerSceneState): void {
  state.controls.dispose()
  disposeObjectResources(state.scene)
  state.scene.clear()
  state.renderer.renderLists.dispose()
  state.renderer.dispose()
  state.renderer.forceContextLoss()
}

type RuntimeViewerProps = {
  onNavigate?: (page: 'home' | 'create' | 'workbench' | 'check' | 'export') => void
}

export function RuntimeViewer({ onNavigate }: RuntimeViewerProps = {}) {
  const [model, setModel] = useState<ViewerModel>(EMPTY_MODEL)
  const [modelRefreshNonce, setModelRefreshNonce] = useState(0)
  const [modelSyncError, setModelSyncError] = useState<string | null>(null)
  const [modelRefreshing, setModelRefreshing] = useState(false)
  const [selectedCandidateId, setSelectedCandidateId] = useState<string | null>(null)
  const [snapshotCompareCandidateId, setSnapshotCompareCandidateId] = useState<string | null>(null)
  const [candidateSortOrder, setCandidateSortOrder] = useState<'newest' | 'oldest'>('newest')
  const [candidateSortMode, setCandidateSortMode] = useState<CandidateSortMode>('time')
  const [timingSortOrder, setTimingSortOrder] = useState<'desc' | 'asc'>('desc')
  const [selectedPass, setSelectedPass] = useState<AovPass>('beauty')
  const [compareMode, setCompareMode] = useState<CompareMode>('split')
  const [evidence, setEvidence] = useState<ViewerVisualEvidence | null>(null)
  const [evidenceError, setEvidenceError] = useState<string | null>(null)
  const [agenticProjection, setAgenticProjection] = useState<AgenticDesignProjection>(() => unavailableAgenticDesignProjection())
  const [agenticSession, setAgenticSession] = useState<AgenticSessionProjection>(() => unavailableAgenticSessionProjection())
  const [referenceImage, setReferenceImage] = useState<ArtifactBytes | null>(null)
  const [renderImage, setRenderImage] = useState<RenderPass | null>(null)
  const [compareLoadState, setCompareLoadState] = useState<LoadState>('idle')
  const [compareError, setCompareError] = useState<string | null>(null)
  const [compareRetryNonce, setCompareRetryNonce] = useState(0)
  const [compareActionStatus, setCompareActionStatus] = useState<CompareActionStatus>('idle')
  const [compareZoom, setCompareZoom] = useState(1)
  const [compareBrightness, setCompareBrightness] = useState(1)
  const [referenceOpacity, setReferenceOpacity] = useState(0.45)
  const [renderOpacity, setRenderOpacity] = useState(0.65)
  const [heatmapSensitivity, setHeatmapSensitivity] = useState(1)
  const [comparePan, setComparePan] = useState({ x: 0, y: 0 })
  const [measureMode, setMeasureMode] = useState(false)
  const [measurePoints, setMeasurePoints] = useState<Point[]>([])
  const [flickerOn, setFlickerOn] = useState(true)
  const [selectedPartId, setSelectedPartId] = useState('all')
  const [selectedMaterialZone, setSelectedMaterialZone] = useState('all')
  const [exploded, setExploded] = useState(false)
  const [diffHeatmap, setDiffHeatmap] = useState(false)
  const [differenceHeatmapUrl, setDifferenceHeatmapUrl] = useState<string | null>(null)
  const [referenceContourAidUrl, setReferenceContourAidUrl] = useState<string | null>(null)
  const [contourPoints, setContourPoints] = useState<Array<{ x: number; y: number }>>([])
  const [contourCopyStatus, setContourCopyStatus] = useState<'idle' | 'copied' | 'unavailable'>('idle')
  const [artifactLoadState, setArtifactLoadState] = useState<LoadState>('idle')
  const [artifactError, setArtifactError] = useState<string | null>(null)
  const [artifactRetryNonce, setArtifactRetryNonce] = useState(0)
  const [selectedObjectId, setSelectedObjectId] = useState<string | null>(null)
  const [hoveredObjectId, setHoveredObjectId] = useState<string | null>(null)
  const [partVisibility, setPartVisibility] = useState<Record<string, boolean>>({})
  const [materialVisibility, setMaterialVisibility] = useState<Record<string, boolean>>({})
  const [partLockState, setPartLockState] = useState<Record<string, boolean>>({})
  const [materialLockState, setMaterialLockState] = useState<Record<string, boolean>>({})
  const [sceneTreeSearch, setSceneTreeSearch] = useState('')
  const [sceneTreeFilter, setSceneTreeFilter] = useState<SceneTreeVisibilityFilter>('all')
  const [expandedPartIds, setExpandedPartIds] = useState<Record<string, boolean>>({})
  const [focusedSceneTreeNodeId, setFocusedSceneTreeNodeId] = useState<string | null>(null)
  const [viewportFocused, setViewportFocused] = useState(false)
  const [viewportLightPreset, setViewportLightPreset] = useState<ViewportLightPreset>('neutral')
  const [viewportCameraPreset, setViewportCameraPreset] = useState<ViewportCameraPreset>('front')
  const [viewportMarqueeRect, setViewportMarqueeRect] = useState<ViewportMarqueeRect | null>(null)
  const [viewportActionHint, setViewportActionHint] = useState<string>('等待输入')
  const contourDrawingRef = useRef(false)
  const comparePanRef = useRef<{ active: boolean; pointerId: number; startX: number; startY: number; originX: number; originY: number }>({ active: false, pointerId: -1, startX: 0, startY: 0, originX: 0, originY: 0 })
  const viewportDragRef = useRef<{ mode: ViewportDragMode; pointerId: number; startX: number; startY: number; endX: number; endY: number }>({ mode: 'idle', pointerId: -1, startX: 0, startY: 0, endX: 0, endY: 0 })
  const sceneTreeNodeRefs = useRef<Record<string, HTMLElement | null>>({})
  const lastSummarySignatureRef = useRef<string | null>(null)
  const activeCandidateIdRef = useRef<string | null>(null)
  const threeRuntimeRef = useRef<Pick<typeof import('./three-runtime-core'), 'Box3' | 'Vector3'> | null>(null)
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
    let timer: number | undefined
    lastSummarySignatureRef.current = null

    const refreshFullModel = async () => {
      setModelRefreshing(true)
      try {
        const next = await runtimeInvoke<ViewerModel>('viewer_read_model')
        if (!active) return
        setModel(next)
        setModelSyncError(next.status === 'Unavailable' ? next.code ?? 'RUNTIME_UNAVAILABLE' : null)
      } catch (error) {
        if (!active) return
        setModel(EMPTY_MODEL)
        setModelSyncError(readErrorCode(error, 'RUNTIME_REQUEST_FAILED'))
      } finally {
        if (active) setModelRefreshing(false)
      }
    }

    const pollSummary = async () => {
      const firstRun = lastSummarySignatureRef.current === null
      let changed = false
      let hasActiveCandidates = false
      let nextDelay: number | null = null
      let hasError = false
      try {
        const summary = await runtimeInvoke<ViewerModelSummary>('viewer_read_model_summary')
        if (!active) return
        if (summary.status === 'Unavailable') {
          hasError = true
          setModelSyncError(summary.code ?? 'RUNTIME_SUMMARY_UNAVAILABLE')
          nextDelay = summaryPollDelaySeconds({
            changed: true,
            hasActiveCandidates: false,
            hasError: true,
            isVisible: document.visibilityState === 'visible',
            firstRun,
          })
        } else {
          hasActiveCandidates = summary.projects.some((project) => (project.candidates ?? []).some((candidate) => isCandidateInProgress(candidate.state)))
          const signature = viewerSummarySignature(summary)
          changed = lastSummarySignatureRef.current !== null && lastSummarySignatureRef.current !== signature
          if (changed) void refreshFullModel()
          lastSummarySignatureRef.current = signature
          setModelSyncError(null)
        }
      } catch (error) {
        if (active) setModelSyncError(readErrorCode(error, 'RUNTIME_SUMMARY_REQUEST_FAILED'))
        if (!active) return
        hasError = true
        nextDelay = summaryPollDelaySeconds({
          changed: true,
          hasActiveCandidates: false,
          hasError: true,
          isVisible: document.visibilityState === 'visible',
          firstRun,
        })
      } finally {
        if (!active) return
        const delay = nextDelay ?? summaryPollDelaySeconds({
          changed,
          hasActiveCandidates,
          hasError,
          isVisible: document.visibilityState === 'visible',
          firstRun,
        })
        timer = window.setTimeout(() => void pollSummary(), delay)
      }
    }

    const handleVisibilityChange = () => {
      if (document.visibilityState === 'visible') {
        if (timer !== undefined) window.clearTimeout(timer)
        void pollSummary()
      }
    }
    document.addEventListener('visibilitychange', handleVisibilityChange)
    void refreshFullModel()
    void pollSummary()
    return () => {
      active = false
      if (timer !== undefined) window.clearTimeout(timer)
      document.removeEventListener('visibilitychange', handleVisibilityChange)
    }
  }, [modelRefreshNonce])

  const project = model.projects[0]
  const ready = model.status === 'Ready'
  const projectName = project?.project?.name ?? '暂无项目'
  const versionCount = project?.versions?.length ?? 0
  const projectId = project?.project?.project_id
  const candidateEntries = project?.candidates ?? []
  const sortedCandidateEntries = useMemo(() => candidateEntries
    .filter((entry): entry is Required<Pick<CandidateView, 'candidate'>> & CandidateView => Boolean(entry.candidate?.candidate_id))
    .sort((left, right) => {
      if (candidateSortMode === 'id') {
        const leftId = left.candidate?.candidate_id ?? ''
        const rightId = right.candidate?.candidate_id ?? ''
        return candidateSortOrder === 'newest'
          ? rightId.localeCompare(leftId)
          : leftId.localeCompare(rightId)
      }
      const leftCandidate = left.candidate
      const rightCandidate = right.candidate
      const leftTime = parseEpochMillis(leftCandidate?.updated_at ?? leftCandidate?.created_at) ?? 0
      const rightTime = parseEpochMillis(rightCandidate?.updated_at ?? rightCandidate?.created_at) ?? 0
      const timeDifference = candidateSortOrder === 'newest' ? rightTime - leftTime : leftTime - rightTime
      return timeDifference || (candidateSortOrder === 'newest'
        ? (right.candidate?.candidate_id ?? '').localeCompare(left.candidate?.candidate_id ?? '')
        : (left.candidate?.candidate_id ?? '').localeCompare(right.candidate?.candidate_id ?? ''))
    }), [candidateEntries, candidateSortMode, candidateSortOrder])
  const candidateSummaries = useMemo(() => sortedCandidateEntries
    .map((entry) => entry.candidate)
    .filter((candidate): candidate is NonNullable<CandidateView['candidate']> => Boolean(candidate?.candidate_id)),
  [sortedCandidateEntries])
  const automaticCandidateId = useMemo(() => {
    if (sortedCandidateEntries.length === 0) return null
    const bestBound = sortedCandidateEntries.find((entry) => hasCandidateBoundVisualEvidence(entry, projectId) && hasCandidateBoundArtifact(entry, projectId))
    if (bestBound?.candidate?.candidate_id) return bestBound.candidate.candidate_id
    const bestArtifact = sortedCandidateEntries.find((entry) => hasCandidateBoundArtifact(entry, projectId))
    if (bestArtifact?.candidate?.candidate_id) return bestArtifact.candidate.candidate_id
    return candidateSortOrder === 'newest'
      ? sortedCandidateEntries[0]?.candidate?.candidate_id ?? null
      : sortedCandidateEntries[sortedCandidateEntries.length - 1]?.candidate?.candidate_id ?? null
  }, [candidateSortOrder, sortedCandidateEntries, projectId])
  const activeCandidateId = selectedCandidateId ?? automaticCandidateId
  const selectedCandidateIsManual = selectedCandidateId !== null
  useEffect(() => {
    if (selectedCandidateId && !candidateSummaries.some((candidate) => candidate.candidate_id === selectedCandidateId)) {
      setSelectedCandidateId(null)
    }
  }, [candidateSummaries, selectedCandidateId])
  // Each candidate view is a separate lineage. Incomplete candidate-bound
  // evidence stays unavailable; the Viewer never borrows an artifact or
  // comparison surface from another candidate.
  const activeCandidate = candidateEntries.find((entry) => entry.candidate?.candidate_id === activeCandidateId)
  const artifact = activeCandidate && hasCandidateBoundArtifact(activeCandidate, projectId)
    ? activeCandidate.artifact
    : undefined
  const activeArtifactSha256 = artifact?.artifact_id ?? null
  const partCount = artifact?.part_ids?.length ?? 0
  const candidateId = activeCandidate?.candidate?.candidate_id
  const candidateRecord = activeCandidate?.candidate
  const artifactCandidateId = artifact?.candidate_id
  const candidateReference = activeCandidate?.reference?.reference
  const referenceId = evidence?.reference_id
  const reference = candidateReference && candidateReference.reference_id === referenceId
    && candidateReference.project_id === projectId
    ? candidateReference
    : undefined
  const renderSetHash = evidence?.render_set_hash
  const partIds = artifact?.part_ids ?? []
  const materialZoneIds = artifact?.material_zone_ids ?? []
  const contourBindingReady = Boolean(
    projectId
      && candidateId
      && referenceId
      && reference?.object_sha256
      && artifact?.artifact_id
      && artifactCandidateId === candidateId
      && renderSetHash
      && evidence?.comparison_report_hash
    && isCandidateBoundVisualEvidence(evidence, candidateId, projectId, artifact?.artifact_id, reference?.object_sha256),
  )
  const visualEvidenceBound = isCandidateBoundVisualEvidence(
    evidence,
    candidateId,
    projectId,
    artifact?.artifact_id,
    candidateReference?.object_sha256,
  )
  const viewerSceneTree = useMemo<SceneTreePartSummary[]>(() => {
    const state = viewerSceneRef.current
    if (!state) return []
    return buildSceneTreeSummary(state.objects)
  }, [artifactLoadState, artifactCandidateId])
  const normalizedSceneTreeSearch = sceneTreeSearch.trim().toLowerCase()
  const filteredSceneTree = normalizedSceneTreeSearch === ''
    ? viewerSceneTree
    : viewerSceneTree.map((part) => {
      const partMatched = part.partId.toLowerCase().includes(normalizedSceneTreeSearch)
      if (partMatched) return part
      const matchedMaterials = part.materials.filter((material) => material.materialZoneId.toLowerCase().includes(normalizedSceneTreeSearch))
      if (matchedMaterials.length === 0) return null
      return {
        ...part,
        materials: matchedMaterials,
      }
    }).filter((part): part is SceneTreePartSummary => Boolean(part))
  const filteredSceneTreeByState = filteredSceneTree.filter((part) => {
    if (sceneTreeFilter === 'all') return true
    const partLocked = Boolean(partLockState[part.partId])
    const hasLockedMaterial = part.materials.some((material) => materialLockState[material.materialZoneId])
    if (sceneTreeFilter === 'locked') return partLocked || hasLockedMaterial
    const partVisible = partVisibility[part.partId] ?? true
    const hasVisibleMaterial = part.materials.length === 0 || part.materials.some((material) => materialVisibility[material.materialZoneId] ?? true)
    return partVisible && hasVisibleMaterial
  })
  const selectedSceneObject = useMemo(() => {
    const state = viewerSceneRef.current
    if (!selectedObjectId || !state) return null
    for (const [object, data] of state.objects) {
      if (object.uuid === selectedObjectId) {
        return { object, data }
      }
    }
    return null
  }, [selectedObjectId, artifactLoadState, artifactCandidateId, filteredSceneTreeByState])
  const sceneTreeNavigationNodes = useMemo<SceneTreeNavigationNode[]>(() => {
    const list: SceneTreeNavigationNode[] = []
    for (const part of filteredSceneTreeByState) {
      list.push({
        id: sceneTreePartNodeId(part.partId),
        type: 'part',
        partId: part.partId,
      })
      const isPartExpanded = Boolean(expandedPartIds[part.partId])
      if (!isPartExpanded) continue
      for (const material of part.materials) {
        list.push({
          id: sceneTreeMaterialNodeId(part.partId, material.materialZoneId),
          type: 'material',
          partId: part.partId,
          materialZoneId: material.materialZoneId,
        })
      }
    }
    return list
  }, [expandedPartIds, filteredSceneTreeByState])
  const sceneTreeNodeMap = useMemo(() => {
    const map = new Map<string, SceneTreeNavigationNode>()
    for (const node of sceneTreeNavigationNodes) {
      map.set(node.id, node)
    }
    return map
  }, [sceneTreeNavigationNodes])
  useEffect(() => {
    if (sceneTreeNavigationNodes.length === 0) {
      if (focusedSceneTreeNodeId !== null) {
        setFocusedSceneTreeNodeId(null)
      }
      return
    }
    if (!focusedSceneTreeNodeId || !sceneTreeNodeMap.has(focusedSceneTreeNodeId)) {
      const fallbackNode = sceneTreeNavigationNodes[0]
      if (fallbackNode) {
        setFocusedSceneTreeNodeId(fallbackNode.id)
      }
    }
  }, [focusedSceneTreeNodeId, sceneTreeNavigationNodes, sceneTreeNodeMap])

  const candidateSnapshots = useMemo<CandidateSnapshotRecord[]>(() => candidateEntries
    .map((entry) => buildCandidateSnapshotRecord(entry, projectId))
    .filter((record): record is CandidateSnapshotRecord => Boolean(record))
    .sort((left, right) => {
      const leftTime = left.createdAtEpochMs ?? left.updatedAtEpochMs ?? 0
      const rightTime = right.createdAtEpochMs ?? right.updatedAtEpochMs ?? 0
      return (rightTime - leftTime) || right.candidateId.localeCompare(left.candidateId)
    }),
  [candidateEntries, projectId])
  const activeSnapshot = useMemo<CandidateSnapshotRecord | null>(() => candidateSnapshots.find((entry) => entry.candidateId === candidateId) ?? null, [candidateSnapshots, candidateId])
  const candidateSnapshotIndex = useMemo(() => candidateSnapshots.findIndex((entry) => entry.candidateId === candidateId), [candidateSnapshots, candidateId])
  const automaticSnapshotCompare = useMemo(() => candidateSnapshotIndex >= 0 ? candidateSnapshots[candidateSnapshotIndex + 1] ?? null : null, [candidateSnapshots, candidateSnapshotIndex])
  const explicitSnapshotCompare = useMemo(() => {
    if (!snapshotCompareCandidateId || snapshotCompareCandidateId === candidateId) return null
    return candidateSnapshots.find((entry) => entry.candidateId === snapshotCompareCandidateId) ?? null
  }, [candidateId, candidateSnapshots, snapshotCompareCandidateId])
  const comparisonSnapshot = explicitSnapshotCompare ?? automaticSnapshotCompare
  const snapshotCompareIsManual = Boolean(explicitSnapshotCompare)
  useEffect(() => {
    if (!snapshotCompareCandidateId) return
    if (snapshotCompareCandidateId === candidateId || !candidateSnapshots.some((entry) => entry.candidateId === snapshotCompareCandidateId)) {
      setSnapshotCompareCandidateId(null)
    }
  }, [candidateId, candidateSnapshots, snapshotCompareCandidateId])
  const candidateSnapshotDiff = useMemo(() => {
    if (!activeSnapshot) return [] as CandidateSnapshotDiffRow[]
    return buildSnapshotDiffRows(activeSnapshot, comparisonSnapshot)
  }, [activeSnapshot, comparisonSnapshot])
  const activeSnapshotBinding = useMemo(() => activeSnapshot ? buildCandidateSnapshotBindingState(activeSnapshot) : null, [activeSnapshot])
  const comparisonSnapshotBinding = useMemo(() => comparisonSnapshot ? buildCandidateSnapshotBindingState(comparisonSnapshot) : null, [comparisonSnapshot])
  const snapshotBindingDelta = useMemo(() => {
    if (!activeSnapshot || !comparisonSnapshot) return [] as CandidateSnapshotDelta[]
    return [
      buildSnapshotMetricDelta(activeSnapshot.partCount, comparisonSnapshot.partCount, 'Part 数'),
      buildSnapshotMetricDelta(activeSnapshot.materialZoneCount, comparisonSnapshot.materialZoneCount, '材质区数'),
      buildSnapshotMetricDelta(activeSnapshot.triangleCount, comparisonSnapshot.triangleCount, '三角形'),
      buildSnapshotMetricDelta(activeSnapshot.qualityPass ? 1 : 0, comparisonSnapshot.qualityPass ? 1 : 0, '质量门通过'),
      buildSnapshotDurationDelta(activeSnapshot, comparisonSnapshot),
    ]
  }, [activeSnapshot, comparisonSnapshot])
  const candidateQuickPreview = useMemo(() => {
    if (!activeSnapshot) return null
    return {
      partCount: activeSnapshot.partCount,
      materialZoneCount: activeSnapshot.materialZoneCount,
      triangleCount: activeSnapshot.triangleCount,
      uvStatus: activeSnapshot.uvStatus,
      tangentStatus: activeSnapshot.tangentStatus,
      quality: activeSnapshot.qualityPass ? '通过' : activeSnapshot.hasVisualEvidenceBinding ? '未通过' : '待绑定',
      visualStatus: formatQualityStatus(activeSnapshot.visualStatus),
      gateStatusClass: activeSnapshot.hasVisualEvidenceBinding ? (activeSnapshot.qualityPass ? 'passed' : 'failed') : 'not-run',
    }
  }, [activeSnapshot])
  useEffect(() => {
    if (!selectedSceneObject) return
    if (!selectedSceneObject.object.visible || isViewportObjectLocked(selectedSceneObject.data, partLockState, materialLockState)) {
      setSelectedObjectId(null)
      setSelectedPartId('all')
      setSelectedMaterialZone('all')
      setFocusedSceneTreeNodeId(null)
    }
  }, [partLockState, materialLockState, selectedSceneObject])
  const selectedSceneObjectLockState = useMemo(() => {
    if (!selectedSceneObject) return { selectedPartLocked: false, selectedMaterialLocked: false }
    return {
      selectedPartLocked: Boolean(partLockState[selectedSceneObject.data.partId]),
      selectedMaterialLocked: Boolean(materialLockState[selectedSceneObject.data.materialZoneId]),
    }
  }, [selectedSceneObject, partLockState, materialLockState])
  const viewportHintItems = useMemo(() => [
    viewportActionHint,
    viewportFocused ? '视口快捷键已就绪' : '视口未聚焦 · 点击空白区域或画布获取快捷键',
    `视图状态：${selectedObjectId ? `已选中 ${selectedSceneObject?.data.partId ?? '对象'}` : '未选中'}`,
    ...(viewportFocused
      ? [...VIEWPORT_KEYBOARD_HINTS_ACTIVE, ...VIEWPORT_KEYBOARD_HINTS]
      : VIEWPORT_KEYBOARD_HINTS_NO_FOCUS),
  ], [viewportActionHint, selectedObjectId, selectedSceneObject?.data.partId, viewportFocused])
  const compareSelectionHint = selectedMaterialZone !== 'all'
    ? `材质筛选：${selectedMaterialZone}`
    : selectedPartId !== 'all'
      ? `部件筛选：${selectedPartId}`
      : '未设置语义筛选'
  const refreshViewerData = () => setModelRefreshNonce((value) => value + 1)
  const setSceneTreePartExpanded = (partId: string, expanded: boolean) => {
    setExpandedPartIds((current) => {
      const next = { ...current }
      if (expanded) next[partId] = true
      else delete next[partId]
      return next
    })
  }
  const setAllSceneTreeExpanded = (expanded: boolean) => {
    if (!expanded) {
      setExpandedPartIds({})
      return
    }
    setExpandedPartIds((current) => {
      if (viewerSceneTree.length === 0) return current
      const next: Record<string, boolean> = {}
      for (const part of viewerSceneTree) {
        next[part.partId] = true
      }
      return next
    })
  }
  const toggleSceneTreePartExpanded = (partId: string) => {
    setExpandedPartIds((current) => {
      const next = { ...current }
      if (current[partId]) delete next[partId]
      else next[partId] = true
      return next
    })
  }
  const syncSceneTreeExpandedFromScene = (parts: SceneTreePartSummary[]) => {
    setExpandedPartIds((current) => {
      const next: Record<string, boolean> = {}
      for (const part of parts) {
        if (current[part.partId]) next[part.partId] = true
      }
      if (parts.length > 0 && Object.keys(next).length === 0) {
        next[parts[0]!.partId] = true
      }
      return next
    })
  }
  const retryArtifactLoad = () => {
    setArtifactError(null)
    setArtifactLoadState('idle')
    setArtifactRetryNonce((value) => value + 1)
  }
  const retryCompareLoad = () => {
    setCompareError(null)
    setCompareLoadState('idle')
    setCompareRetryNonce((value) => value + 1)
  }
  const retryEvidenceLoad = () => {
    setEvidenceError(null)
    setEvidence(null)
    refreshViewerData()
  }
  const clearManualCandidateSelection = () => {
    setSelectedCandidateId(null)
    setSelectedObjectId(null)
    setHoveredObjectId(null)
    setSelectedPartId('all')
    setSelectedMaterialZone('all')
    setSelectedPass('beauty')
    setExpandedPartIds({})
  }
  const errorConsoleItems = useMemo<ErrorConsoleItem[]>(() => {
    const result: ErrorConsoleItem[] = []
    if (modelSyncError) {
      result.push({
        id: 'runtime-read-model',
        scope: 'Runtime 模型读取',
        code: modelSyncError,
        title: 'Runtime 会话读取失败',
        category: deriveErrorCategory(modelSyncError, '读取失败'),
        summary: '无法确认候选与项目元数据',
        meaning: deriveCandidateErrorMeaning(modelSyncError),
        severity: statusClassFromCode(modelSyncError),
        actionLabel: '重试读取 Runtime',
        action: refreshViewerData,
      })
    }

    if (evidenceError) {
      result.push({
        id: 'runtime-evidence',
        scope: '候选证据',
        code: evidenceError,
        title: 'QualityEvidence 读取失败',
        category: deriveErrorCategory(evidenceError, '未就绪'),
        summary: '固定视图/质量指标无法同步',
        meaning: deriveCandidateErrorMeaning(evidenceError),
        severity: statusClassFromCode(evidenceError, evidenceError === 'VISUAL_EVIDENCE_UNAVAILABLE' || evidenceError === 'REFERENCE_UNAVAILABLE' || evidenceError === 'VISUAL_EVIDENCE_BINDING_MISMATCH'),
        actionLabel: '重新读取该候选证据',
        action: retryEvidenceLoad,
      })
    }

    if (artifactError && artifactLoadState === 'error' && candidateId) {
      result.push({
        id: 'runtime-artifact',
        scope: `候选 GLB（${candidateId}）`,
        code: artifactError,
        title: artifactError === 'ARTIFACT_BYTES_UNAVAILABLE'
          ? '候选 GLB 尚未就绪'
          : artifactError === 'ARTIFACT_BYTES_BINDING_MISMATCH'
            ? '候选 GLB 绑定冲突'
            : '候选 GLB 加载失败',
        category: deriveErrorCategory(artifactError, '加载失败'),
        summary: '3D 视图无法使用当前 candidate 的几何体',
        meaning: deriveCandidateErrorMeaning(artifactError),
        severity: statusClassFromCode(artifactError),
        actionLabel: '重试 GLB',
        action: retryArtifactLoad,
      })
    }

    if (compareError && candidateId) {
      const compareAssetError = compareLoadState === 'error'
      result.push({
        id: 'runtime-compare',
        scope: compareAssetError ? `参考对比（${selectedPass}）` : '对比辅助计算',
        code: compareError,
        title: compareAssetError ? '参考图/Render PNG 对比读取失败' : '差异热图/轮廓辅助计算失败',
        category: deriveErrorCategory(compareError, compareAssetError ? '读取失败' : '异常'),
        summary: compareAssetError ? 'AOV 比较窗口将不可用' : '比较辅助层暂不可用，原始 AOV 与 Runtime 质量门仍可查看',
        meaning: deriveCandidateErrorMeaning(compareError),
        severity: statusClassFromCode(compareError),
        actionLabel: '重试比较资源',
        action: retryCompareLoad,
      })
    }

    if (candidateId && !artifact && artifactLoadState === 'idle') {
      result.push({
        id: 'runtime-artifact-missing',
        scope: `当前候选（${candidateId}）`,
        code: 'ARTIFACT_MISSING_FOR_CANDIDATE',
        title: '候选已存在但缺少 GLB 载荷',
        category: '未就绪',
        summary: '可能为生成中、未完成或数据未可用',
        meaning: '当前候选没有可用的 GLB 绑定，请等待生成完成或手动切换其他候选查看。',
        severity: 'warn',
        actionLabel: '取消手动选择（切到自动候选）',
        action: clearManualCandidateSelection,
      })
    }

    if (candidateId && !modelSyncError && evidence == null && model.status === 'Ready') {
      result.push({
        id: 'runtime-evidence-wait',
        scope: `当前候选（${candidateId}）`,
        code: 'VISUAL_EVIDENCE_MISSING',
        title: '候选证据未就绪',
        category: '未就绪',
        summary: '候选已存在，但质量与对比证据尚未全部写回',
        meaning: '数据处于生成或写回窗口内，属于可重试/可等待的“数据未可用”状态。',
        severity: 'warn',
        actionLabel: '取消手动选择（切到自动候选）',
        action: clearManualCandidateSelection,
      })
    }

    return result
  }, [
    artifact,
    artifactError,
    artifactLoadState,
    candidateId,
    compareError,
    compareLoadState,
    evidence,
    evidenceError,
    model.status,
    modelSyncError,
    refreshViewerData,
    retryArtifactLoad,
    retryCompareLoad,
    selectedPass,
    clearManualCandidateSelection,
    retryEvidenceLoad,
  ])
  const errorSummaryByCategory = useMemo(() => {
    const counts = {
      读取失败: 0,
      加载失败: 0,
      绑定不一致: 0,
      数据未可用: 0,
      未就绪: 0,
      异常: 0,
    }
    let errorCount = 0
    let warnCount = 0
    for (const item of errorConsoleItems) {
      counts[item.category] += 1
      if (item.severity === 'error') errorCount += 1
      else warnCount += 1
    }
    return {
      counts,
      errorCount,
      warnCount,
      total: errorConsoleItems.length,
    }
  }, [errorConsoleItems])
  const viewportControlHint = clampViewportControlHint(viewportLightPreset, selectedSceneObjectLockState)
  useEffect(() => {
    const nextCandidateId = activeCandidateId ?? null
    if (activeCandidateIdRef.current !== nextCandidateId) {
      activeCandidateIdRef.current = nextCandidateId
      setSelectedObjectId(null)
      setHoveredObjectId(null)
      setSelectedPartId('all')
      setSelectedMaterialZone('all')
      setExpandedPartIds({})
      setPartLockState({})
      setMaterialLockState({})
      setViewportMarqueeRect(null)
    }
  }, [activeCandidateId])
  const generationTimings = useMemo<CandidateGenerationTiming[]>(() => {
    const timings: CandidateGenerationTiming[] = []
    for (const entry of candidateEntries) {
      const id = entry.candidate?.candidate_id
      if (!id) continue
      const state = entry.candidate?.state ?? 'unknown'
      const snapshot = buildCandidateSnapshotRecord(entry, projectId)
      const qualityGate: CandidateGenerationTiming['qualityGate'] = snapshot?.hasVisualEvidenceBinding
        ? (snapshot.qualityPass ? '通过' : '未通过')
        : '待绑定'
      const artifactReady = Boolean(snapshot?.hasArtifactBinding && snapshot.artifactId)
      const compareReady = Boolean(snapshot?.hasComparisonBinding && snapshot.comparisonReportHash)
      const qualityReady = Boolean(snapshot?.hasVisualEvidenceBinding && snapshot.qualityReportHash)
      const outcome = candidateGenerationOutcome(state)
      const createdAtRaw = entry.candidate?.created_at
      const createdAtText = buildGenerationTimestamp(createdAtRaw)
      const epochMs = parseEpochMillis(createdAtRaw)
      if (epochMs === null) {
        timings.push({
          candidateId: id,
          state,
          createdAtText: null,
          durationSource: null,
          elapsedSeconds: null,
          elapsedDisplay: null,
          statusLabel: '时间缺失',
          statusClass: 'failed',
          anomaly: true,
          qualityGate,
          artifactReady,
          compareReady,
          qualityReady,
        })
        continue
      }
      const updatedAtRaw = entry.candidate?.updated_at
      const updatedEpochMs = updatedAtRaw ? parseEpochMillis(updatedAtRaw) : null
      if (updatedAtRaw && updatedEpochMs === null) {
        timings.push({
          candidateId: id,
          state,
          createdAtText,
          durationSource: null,
          elapsedSeconds: null,
          elapsedDisplay: null,
          statusLabel: '时间异常（updated_at 无法解析）',
          statusClass: 'failed',
          anomaly: true,
          qualityGate,
          artifactReady,
          compareReady,
          qualityReady,
        })
        continue
      }
      const durationSource = updatedEpochMs === null ? 'live' : 'updated_at'
      const elapsedMs = (updatedEpochMs ?? Date.now()) - epochMs
      if (!Number.isFinite(elapsedMs)) {
        timings.push({
          candidateId: id,
          state,
          createdAtText,
          durationSource,
          elapsedSeconds: null,
          elapsedDisplay: null,
          statusLabel: '时间异常',
          statusClass: 'failed',
          anomaly: true,
          qualityGate,
          artifactReady,
          compareReady,
          qualityReady,
        })
        continue
      }
      if (elapsedMs < 0) {
        timings.push({
          candidateId: id,
          state,
          createdAtText,
          durationSource,
          elapsedSeconds: Math.floor(elapsedMs / 1000),
          elapsedDisplay: null,
          statusLabel: '时间异常（未来时间）',
          statusClass: 'failed',
          anomaly: true,
          qualityGate,
          artifactReady,
          compareReady,
          qualityReady,
        })
        continue
      }
      const elapsedSeconds = Math.floor(elapsedMs / 1000)
      const elapsedDays = elapsedSeconds / (24 * 3600)
      const isAbnormal = elapsedDays > 14
      const missingEndTime = !updatedAtRaw
      const statusClass = outcome === 'failed' || (outcome !== 'unknown' && (isAbnormal || missingEndTime))
        ? 'failed'
        : outcome === 'unknown'
          ? 'not-run'
          : 'passed'
      timings.push({
        candidateId: id,
        state,
        createdAtText,
        durationSource,
        elapsedSeconds,
        elapsedDisplay: formatGenerationDurationFromSeconds(elapsedSeconds),
        statusLabel: isAbnormal
          ? '异常（耗时>14天）'
          : missingEndTime
            ? outcome === 'unknown' ? '实时估算（缺少结束时间）' : '时间缺失（缺少 updated_at）'
            : outcome === 'failed'
              ? '状态失败'
              : outcome === 'unknown'
                ? '进行中/状态未知'
                : durationSource === 'live' ? '实时耗时' : '完成耗时',
        statusClass,
        anomaly: isAbnormal || missingEndTime,
        qualityGate,
        artifactReady,
        compareReady,
        qualityReady,
      })
    }
    return timings.sort((a, b) => {
      const left = a.candidateId ?? ''
      const right = b.candidateId ?? ''
      return timingSortOrder === 'desc' ? right.localeCompare(left) : left.localeCompare(right)
    })
  }, [candidateEntries, projectId, timingSortOrder])
  const generationTimingByCandidateId = useMemo(() => new Map(generationTimings.map((timing) => [timing.candidateId, timing])), [generationTimings])
  const activeSnapshotTiming = activeSnapshot ? generationTimingByCandidateId.get(activeSnapshot.candidateId) : undefined
  const comparisonSnapshotTiming = comparisonSnapshot ? generationTimingByCandidateId.get(comparisonSnapshot.candidateId) : undefined
  const successfulGenerationTimings = generationTimings.filter((timing) => timing.statusClass === 'passed')
  const averageGenerationSeconds = successfulGenerationTimings.length > 0
    ? successfulGenerationTimings.reduce((sum, timing) => sum + (timing.elapsedSeconds ?? 0), 0) / successfulGenerationTimings.length
    : null
  const averageGenerationText = averageGenerationSeconds === null
    ? '未运行'
    : formatGenerationDurationFromSeconds(averageGenerationSeconds)
  const completedGenerationTimings = generationTimings.filter((timing) => timing.statusClass === 'passed' || timing.statusClass === 'failed')
  const generationSuccessRate = completedGenerationTimings.length === 0
    ? 0
    : (successfulGenerationTimings.length / completedGenerationTimings.length) * 100
  const generationSuccessRateClass = completedGenerationTimings.length === 0
    ? 'not-run'
    : generationSuccessRate >= 100
      ? 'passed'
      : generationSuccessRate > 0
        ? 'locked'
        : 'failed'
  const generationSuccessRateText = completedGenerationTimings.length === 0
    ? '未运行'
    : `${Math.round(generationSuccessRate)}%（${successfulGenerationTimings.length}/${completedGenerationTimings.length}）${generationTimings.length > completedGenerationTimings.length ? ` · 进行中/未知 ${generationTimings.length - completedGenerationTimings.length}` : ''}`
  const generationAnomalyCount = generationTimings.filter((timing) => timing.anomaly).length
  const agenticSessionBinding = useMemo<AgenticSessionBinding>(() => ({
    projectId,
    candidateId,
    artifactSha256: artifact?.artifact_id,
    referenceSha256: candidateReference?.object_sha256,
    renderSetHash: evidence?.render_set_hash,
    comparisonReportHash: evidence?.comparison_report_hash ?? undefined,
    qualityReportHash: evidence?.quality_report_hash,
    visualEvidenceBound,
    revision: {
      snapshotId: project?.head_snapshot?.snapshot_id ?? project?.record?.head_snapshot_id ?? undefined,
      snapshotRevision: project?.head_snapshot?.revision,
      snapshotManifestHash: project?.head_snapshot?.manifest_hash,
      candidateCanonicalSha256: candidateRecord?.canonical_sha256,
    },
  }), [artifact?.artifact_id, candidateId, candidateRecord?.canonical_sha256, candidateReference?.object_sha256, evidence?.comparison_report_hash, evidence?.quality_report_hash, evidence?.render_set_hash, project?.head_snapshot?.manifest_hash, project?.head_snapshot?.revision, project?.head_snapshot?.snapshot_id, project?.record?.head_snapshot_id, projectId, visualEvidenceBound])
  const stageEvidenceHashes = agenticProjection.status === 'ready'
    ? agenticProjection.evidenceHashes
    : visualEvidenceBound
      ? {
          artifactSha256: artifact?.artifact_id ?? null,
          referenceSha256: candidateReference?.object_sha256 ?? null,
          renderSetHash: evidence?.render_set_hash ?? null,
          comparisonReportHash: evidence?.comparison_report_hash ?? null,
          qualityReportHash: evidence?.quality_report_hash ?? null,
        }
      : agenticProjection.evidenceHashes

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
    setSelectedPass('beauty')
    setHoveredObjectId(null)
    setExploded(false)
    setDiffHeatmap(false)
    setCompareZoom(1)
    setCompareBrightness(1)
    setReferenceOpacity(0.45)
    setRenderOpacity(0.65)
    setHeatmapSensitivity(1)
    setComparePan({ x: 0, y: 0 })
    setMeasureMode(false)
    setMeasurePoints([])
    setCompareActionStatus('idle')
    setContourPoints([])
    setContourCopyStatus('idle')
    setSelectedObjectId(null)
    setPartVisibility({})
    setMaterialVisibility({})
    setExpandedPartIds({})
  }, [candidateId, projectId])

  useEffect(() => {
    let active = true
    setEvidence(null)
    setEvidenceError(null)
    setReferenceImage(null)
    setRenderImage(null)
    if (!candidateId) return () => { active = false }
    void runtimeInvoke<ViewerVisualEvidence>('viewer_visual_evidence', { candidateId }).then((next) => {
      if (!active) return
      if (next.status === 'Unavailable') {
        setEvidenceError(next.code ?? 'VISUAL_EVIDENCE_UNAVAILABLE')
        return
      }
      if (isCandidateBoundVisualEvidence(next, candidateId, projectId, artifact?.artifact_id, candidateReference?.object_sha256)) setEvidence(next)
      else setEvidenceError('VISUAL_EVIDENCE_BINDING_MISMATCH')
    }).catch((error) => {
      if (active) setEvidenceError(readErrorCode(error, 'VISUAL_EVIDENCE_REQUEST_FAILED'))
    })
    return () => { active = false }
  }, [artifact?.artifact_id, candidateId, candidateReference?.object_sha256, modelRefreshNonce, projectId])

  useEffect(() => {
    let active = true
    setAgenticProjection(unavailableAgenticDesignProjection(projectId, candidateId))
    if (!projectId || !candidateId) return () => { active = false }
    void runtimeInvoke<unknown>('viewer_agentic_projection', { projectId, candidateId }).then((payload) => {
      if (!active) return
      setAgenticProjection(normalizeAgenticDesignProjection(payload, {
        projectId,
        candidateId,
        artifactSha256: artifact?.artifact_id,
        referenceSha256: candidateReference?.object_sha256,
        renderSetHash: evidence?.render_set_hash,
        comparisonReportHash: evidence?.comparison_report_hash ?? undefined,
        qualityReportHash: evidence?.quality_report_hash,
        visualEvidenceBound,
      }))
    }).catch(() => {
      if (active) setAgenticProjection(unavailableAgenticDesignProjection(projectId, candidateId))
    })
    return () => { active = false }
  }, [artifact?.artifact_id, candidateId, candidateReference?.object_sha256, evidence?.comparison_report_hash, evidence?.quality_report_hash, evidence?.render_set_hash, modelRefreshNonce, projectId, visualEvidenceBound])

  useEffect(() => {
    let active = true
    setAgenticSession(unavailableAgenticSessionProjection(agenticSessionBinding))
    if (!agenticSessionBinding.projectId || !agenticSessionBinding.candidateId) return () => { active = false }
    void runtimeInvoke<unknown>('viewer_agentic_session', {
      projectId: agenticSessionBinding.projectId,
      candidateId: agenticSessionBinding.candidateId,
    }).then((payload) => {
      if (!active) return
      setAgenticSession(normalizeAgenticSessionProjection(payload, agenticSessionBinding))
    }).catch(() => {
      if (active) setAgenticSession(unavailableAgenticSessionProjection(agenticSessionBinding))
    })
    return () => { active = false }
  }, [agenticSessionBinding, modelRefreshNonce])

  useEffect(() => {
    let active = true
    setReferenceImage(null)
    setRenderImage(null)
    setCompareError(null)
    setCompareLoadState('idle')
    if (!referenceId || !projectId || !renderSetHash || !candidateId || !reference?.object_sha256) return () => { active = false }
    setCompareLoadState('loading')
    void Promise.all([
      runtimeInvoke<ArtifactBytes>('viewer_reference_bytes', { referenceId, projectId }),
      runtimeInvoke<RenderPass>('viewer_render_pass', { renderSetHash, pass: selectedPass }),
    ]).then(([referencePayload, renderPayload]) => {
      if (!active) return
      if (referencePayload.status === 'Unavailable') throw new Error(referencePayload.code ?? 'REFERENCE_BYTES_UNAVAILABLE')
      if (renderPayload.status === 'Unavailable') throw new Error(renderPayload.code ?? 'RENDER_PASS_UNAVAILABLE')
      if (!isCandidateBoundReferencePayload(referencePayload, referenceId, projectId, reference.object_sha256)) throw new Error('REFERENCE_BYTES_BINDING_MISMATCH')
      if (!isCandidateBoundRenderPayload(renderPayload, candidateId, renderSetHash, selectedPass)) throw new Error('RENDER_PASS_BINDING_MISMATCH')
      if (!referencePayload.bytes_base64) throw new Error('REFERENCE_BYTES_EMPTY')
      if (!renderPayload.png_base64) throw new Error('RENDER_PASS_BYTES_EMPTY')
      setReferenceImage(referencePayload)
      setRenderImage(renderPayload)
      setCompareLoadState('ready')
    }).catch((error) => {
      if (!active) return
      setCompareLoadState('error')
      setCompareError(readErrorCode(error, 'COMPARE_ASSET_LOAD_FAILED'))
    })
    return () => { active = false }
  }, [candidateId, compareRetryNonce, reference?.object_sha256, referenceId, projectId, renderSetHash, selectedPass])

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
    if (!artifactId || !candidateId || !canvas) {
      setArtifactLoadState('idle')
      setArtifactError(null)
      return
    }
    let disposed = false
    let scene: THREE.Scene | null = null
    let renderer: THREE.WebGLRenderer | null = null
    let controls: OrbitControls | null = null
    let raycaster: THREE.Raycaster | null = null
    let state: ViewerSceneState | null = null
    let resizeObserver: ResizeObserver | null = null
    let resizeListener: (() => void) | null = null
    let resizeFrame: number | null = null
    setArtifactLoadState('loading')
    setArtifactError(null)

    const load = async () => {
      try {
        const [THREE, runtimeLoader, runtimeRenderer, runtimeControls] = await Promise.all([
          import('./three-runtime-core'),
          import('./three-runtime-loader'),
          import('./three-runtime-renderer'),
          import('./three-runtime-controls'),
        ])
        if (!disposed) threeRuntimeRef.current = THREE
        if (disposed) return
        scene = new THREE.Scene()
        scene.background = new THREE.Color('#080d14')
        const camera = new THREE.PerspectiveCamera(32, 1, 0.01, 100)
        renderer = new runtimeRenderer.WebGLRenderer({ canvas, antialias: true, alpha: true })
        renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
        renderer.outputColorSpace = THREE.SRGBColorSpace
        const hemisphereLight = new THREE.HemisphereLight('#f1f6ff', '#172536', 2.2)
        const keyLight = new THREE.DirectionalLight('#ffffff', 2.4)
        keyLight.position.set(4, 6, 6)
        keyLight.name = 'runtime-key'
        const fillLight = new THREE.DirectionalLight('#a2d8ff', 1.1)
        fillLight.position.set(-3, 3, -2)
        fillLight.name = 'runtime-fill'
        const rimLight = new THREE.DirectionalLight('#ffdeb5', 1.0)
        rimLight.position.set(-5, -2, -4)
        rimLight.name = 'runtime-rim'
        scene.add(hemisphereLight)
        scene.add(keyLight)
        scene.add(fillLight)
        scene.add(rimLight)
        const nextControls = new runtimeControls.OrbitControls(camera, canvas)
        controls = nextControls
        const nextRaycaster = new THREE.Raycaster()
        raycaster = nextRaycaster
        nextControls.enableDamping = false
        nextControls.enablePan = true
        nextControls.screenSpacePanning = true
        nextControls.minDistance = 0.05
        nextControls.maxDistance = 1000
        nextControls.rotateSpeed = 0.8
        nextControls.zoomSpeed = 0.9
        nextControls.mouseButtons = {
          LEFT: null,
          MIDDLE: THREE.MOUSE.PAN,
          RIGHT: THREE.MOUSE.ROTATE,
        }
        nextControls.listenToKeyEvents(window)
        nextControls.touches = {
          ONE: THREE.TOUCH.ROTATE,
          TWO: THREE.TOUCH.DOLLY_PAN,
        }
        const renderScene = () => {
          if (!disposed && renderer && scene) renderer.render(scene, camera)
        }
        nextControls.addEventListener('change', renderScene)
        const resize = () => {
          if (resizeFrame !== null) window.cancelAnimationFrame(resizeFrame)
          resizeFrame = window.requestAnimationFrame(() => {
            resizeFrame = null
            if (disposed || !renderer || !scene) return
            const bounds = canvas.parentElement?.getBoundingClientRect()
            const width = Math.max(1, Math.floor(bounds?.width ?? canvas.clientWidth ?? 640))
            const height = Math.max(1, Math.floor(bounds?.height ?? canvas.clientHeight ?? 520))
            camera.aspect = width / height
            camera.updateProjectionMatrix()
            renderer.setSize(width, height, false)
            renderScene()
          })
        }
        if (typeof ResizeObserver !== 'undefined') {
          resizeObserver = new ResizeObserver(resize)
          resizeObserver.observe(canvas.parentElement ?? canvas)
        } else {
          resizeListener = resize
          window.addEventListener('resize', resizeListener)
        }
        resize()

        const loader = new runtimeLoader.GLTFLoader()
          const payload = await runtimeInvoke<ArtifactBytes>('viewer_artifact_bytes', { artifactId, candidateId })
        if (disposed) return
        if (payload.status === 'Unavailable') throw new Error(payload.code ?? 'ARTIFACT_BYTES_UNAVAILABLE')
        if (!payload.bytes_base64) throw new Error('ARTIFACT_BYTES_EMPTY')
        if (!isCandidateBoundArtifactPayload(payload, artifactId, candidateId, activeArtifactSha256)) throw new Error('ARTIFACT_BYTES_BINDING_MISMATCH')
        const binary = Uint8Array.from(atob(payload.bytes_base64), (character) => character.charCodeAt(0))
        loader.parse(binary.buffer, '', (gltf: import('three/examples/jsm/loaders/GLTFLoader.js').GLTF) => {
          if (disposed || !scene || !renderer) return
          const root = gltf.scene
          if (!root || root.children.length === 0) {
            setArtifactLoadState('error')
            setArtifactError('GLB_EMPTY_SCENE')
            return
          }
          scene.add(root)
          const box = new THREE.Box3().setFromObject(root)
          const size = box.getSize(new THREE.Vector3())
          const center = box.getCenter(new THREE.Vector3())
          const radius = Math.max(size.x, size.y, size.z, 0.1)
          camera.position.set(radius * 1.9, radius * 1.15, radius * 2.1)
          camera.near = Math.max(radius / 1000, 0.0001)
          camera.far = Math.max(radius * 100, 100)
          camera.updateProjectionMatrix()
          nextControls.target.copy(center)
          nextControls.saveState()
          nextControls.update()
          const objects = new Map<THREE.Object3D, ViewerObjectState>()
          root.traverse((object: THREE.Object3D) => {
            if (!(object as THREE.Mesh).isMesh) return
            const mesh = object as THREE.Mesh
            isolateViewerMeshMaterials(mesh)
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
            objects.set(mesh, { basePosition: mesh.position.clone(), direction, partId, materialZoneId, isSelected: false })
          })
          state = {
            root,
            renderer,
            scene,
            camera,
            controls: nextControls,
            raycaster: nextRaycaster,
            objects,
            lights: {
              hemi: hemisphereLight,
              key: keyLight,
              fill: fillLight,
              rim: rimLight,
            },
          }
          viewerSceneRef.current = state
          applyViewportLightPreset(viewportLightPreset)
          moveViewportCameraToPreset(viewportCameraPreset)
          setArtifactLoadState('ready')
          renderScene()
        }, (_error: unknown) => {
          if (!disposed) {
            setArtifactLoadState('error')
            setArtifactError('GLB_PARSE_FAILED')
          }
        })
      } catch (error) {
        if (!disposed) {
          setArtifactLoadState('error')
          setArtifactError(readErrorCode(error, 'ARTIFACT_LOAD_FAILED'))
        }
      }
    }
    void load()
    return () => {
      disposed = true
      resizeObserver?.disconnect()
      if (resizeListener) window.removeEventListener('resize', resizeListener)
      if (resizeFrame !== null) window.cancelAnimationFrame(resizeFrame)
      if (state) disposeViewerScene(state)
      else if (scene && renderer) {
        controls?.dispose()
        disposeObjectResources(scene)
        scene.clear()
        renderer.renderLists.dispose()
        renderer.dispose()
        renderer.forceContextLoss()
      }
      if (viewerSceneRef.current === state) viewerSceneRef.current = null
    }
  }, [artifact?.artifact_id, artifactCandidateId, activeArtifactSha256, artifactRetryNonce])

  useEffect(() => {
    const state = viewerSceneRef.current
    if (!state) return
    state.objects.forEach((objectState, object) => {
      const partMatches = selectedPartId === 'all' || objectState.partId === selectedPartId
      const materialMatches = selectedMaterialZone === 'all' || objectState.materialZoneId === selectedMaterialZone
      const partVisible = partVisibility[objectState.partId] ?? true
      const materialVisible = materialVisibility[objectState.materialZoneId] ?? true
      object.visible = partMatches && materialMatches && partVisible && materialVisible
      object.position.copy(objectState.basePosition)
      if (exploded && object.visible) object.position.addScaledVector(objectState.direction, 0.18)
    })
    state.renderer.render(state.scene, state.camera)
  }, [artifactLoadState, selectedPartId, selectedMaterialZone, exploded, diffHeatmap, partVisibility, materialVisibility])

  useEffect(() => {
    const state = viewerSceneRef.current
    if (!state || artifactLoadState !== 'ready') return
    const nextPartVisibility: Record<string, boolean> = {}
    const nextMaterialVisibility: Record<string, boolean> = {}
    state.objects.forEach((objectState) => {
      nextPartVisibility[objectState.partId] = true
      nextMaterialVisibility[objectState.materialZoneId] = true
    })
    setPartVisibility(nextPartVisibility)
    setMaterialVisibility(nextMaterialVisibility)
    if (viewerSceneTree.length > 0) syncSceneTreeExpandedFromScene(viewerSceneTree)
  }, [artifactLoadState, artifactCandidateId, artifactRetryNonce, viewerSceneTree])

  useEffect(() => {
    const state = viewerSceneRef.current
    if (!state) return
    state.objects.forEach((objectState, object) => {
      const isSelected = object.uuid === selectedObjectId
      const isHovered = object.uuid === hoveredObjectId && !isSelected
      objectState.isSelected = isSelected
      applyObjectSelectionState(object, isSelected)
      applyObjectHoverState(object, isHovered)
    })
    state.renderer.render(state.scene, state.camera)
  }, [hoveredObjectId, selectedObjectId, artifactLoadState])

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
    let stopWorker: () => void = () => undefined
    const load = async () => {
      setCompareError(null)
      const reference = new Image()
      const render = new Image()
      reference.src = referenceDataUrl
      render.src = renderDataUrl
      await Promise.all([reference.decode(), render.decode()])
      if (!active) return
      const referenceData = createContainedImageData(reference)
      const renderData = createContainedImageData(render)
      if (!referenceData || !renderData) throw new Error('COMPARE_IMAGE_DATA_UNAVAILABLE')
      stopWorker = runCompareWorker({
        kind: 'difference',
        width: HEATMAP_SIZE,
        height: HEATMAP_SIZE,
        referenceBuffer: referenceData.data.buffer,
        renderBuffer: renderData.data.buffer,
        sensitivity: heatmapSensitivity,
      }, (result) => {
        if (!active) return
        const dataUrl = imageDataToDataUrl(result)
        if (!dataUrl) {
          setCompareError('COMPARE_RESULT_IMAGE_UNAVAILABLE')
          return
        }
        setCompareError(null)
        setDifferenceHeatmapUrl(dataUrl)
      }, (code) => {
        if (active) setCompareError(code)
      })
    }
    void load().catch((error) => {
      if (active) setCompareError(readErrorCode(error, 'DIFFERENCE_HEATMAP_FAILED'))
    })
    return () => {
      active = false
      stopWorker()
    }
  }, [diffHeatmap, heatmapSensitivity, referenceDataUrl, renderDataUrl])

  useEffect(() => {
    let active = true
    setReferenceContourAidUrl(null)
    if (!contourCanvasActive || !referenceDataUrl) return () => { active = false }
    let stopWorker: () => void = () => undefined
    const load = async () => {
      setCompareError(null)
      const reference = new Image()
      reference.src = referenceDataUrl
      await reference.decode()
      if (!active) return
      const referenceData = createContainedImageData(reference)
      if (!referenceData) throw new Error('REFERENCE_CONTOUR_IMAGE_DATA_UNAVAILABLE')
      stopWorker = runCompareWorker({
        kind: 'contour',
        width: HEATMAP_SIZE,
        height: HEATMAP_SIZE,
        referenceBuffer: referenceData.data.buffer,
      }, (result) => {
        if (!active) return
        const dataUrl = imageDataToDataUrl(result)
        if (!dataUrl) {
          setCompareError('COMPARE_RESULT_IMAGE_UNAVAILABLE')
          return
        }
        setCompareError(null)
        setReferenceContourAidUrl(dataUrl)
      }, (code) => {
        if (active) setCompareError(code)
      })
    }
    void load().catch((error) => {
      if (active) setCompareError(readErrorCode(error, 'REFERENCE_CONTOUR_FAILED'))
    })
    return () => {
      active = false
      stopWorker()
    }
  }, [contourCanvasActive, referenceDataUrl])

  const comparisonMetrics = evidence?.comparison_report?.metrics ?? {}
  const visualQualityReport = evidence?.quality_report
  const visualStatus = visualQualityReport?.visual_status ?? 'not-run'
  const visualStatusLabel = formatQualityStatus(visualStatus)
  const visualStatusClass = qualityStatusClass(visualStatus)
  const visualStatusIcon = visualStatusClass === 'passed' ? '✓' : visualStatusClass === 'failed' ? '!' : visualStatusClass === 'partial' ? '~' : '·'
  const activeCandidateChain = useMemo(() => {
    const artifactReady = artifactLoadState === 'ready' && Boolean(artifact)
    const compareReady = compareLoadState === 'ready' && Boolean(referenceImage) && Boolean(renderImage)
    const evidenceReady = Boolean(visualEvidenceBound && evidence)
    const qualityReady = visualQualityReport?.hard_gate_passed !== undefined
    return {
      artifactReady,
      compareReady,
      evidenceReady,
      qualityReady,
      visualStatus: formatQualityStatus(visualStatus),
    }
  }, [artifact, artifactLoadState, compareLoadState, evidence, referenceImage, renderImage, visualEvidenceBound, visualQualityReport?.hard_gate_passed, visualStatus])
  // Display the Runtime decision verbatim. Viewer does not add a second
  // status predicate or recompute the visual gate from metrics.
  const visualHardGatePassed = visualQualityReport?.hard_gate_passed === true
  const visualGateStatusClass = visualQualityReport ? (visualHardGatePassed ? 'passed' : 'failed') : 'not-run'
  const visualGateLabel = visualQualityReport ? (visualHardGatePassed ? '通过' : '未通过') : '未运行'
  const visualGateSource = agenticProjection.status === 'ready'
    ? 'Runtime Agentic 投影 → 候选绑定 ReferenceComparisonReport@1 / 质量报告@2'
    : visualQualityReport
      ? '候选绑定 质量报告@2'
      : '未运行：Runtime 质量报告不可用'
  const metricLabels: Array<[string, string]> = [
    ['silhouette_iou', '轮廓 IoU · Silhouette'],
    ['boundary_f1_4px', '边界 F1 · Boundary'],
    ['bbox_edge_error', '包围盒边缘误差 · BBox'],
    ['centroid_error', '中心点误差 · Centroid'],
    ['landmark_coverage', '关键点覆盖率 · Landmark'],
    ['landmark_nme', '关键点 NME · Landmark'],
    ['region_median_iou', '区域中位 IoU · Region'],
    ['critical_region_min_iou', '关键区域最小 IoU'],
  ]

  const resetCompareView = () => {
    setCompareZoom(1)
    setComparePan({ x: 0, y: 0 })
    setCompareBrightness(1)
    setReferenceOpacity(0.45)
    setRenderOpacity(0.65)
    setHeatmapSensitivity(1)
    setMeasureMode(false)
    setMeasurePoints([])
  }

  const applyViewportLightPreset = (preset: ViewportLightPreset) => {
    const state = viewerSceneRef.current
    if (!state) return
    const intensity = viewportLightPresetValues(preset)
    if (state.lights.key) state.lights.key.intensity = intensity.key
    if (state.lights.fill) state.lights.fill.intensity = intensity.fill
    if (state.lights.rim) state.lights.rim.intensity = intensity.rim
    state.renderer.render(state.scene, state.camera)
  }

  useEffect(() => {
    applyViewportLightPreset(viewportLightPreset)
  }, [viewportLightPreset])

  const moveViewportCameraToPreset = (preset: ViewportCameraPreset) => {
    const state = viewerSceneRef.current
    const threeRuntime = threeRuntimeRef.current
    if (!threeRuntime) return
    if (!state) return
    const root = state.root
    const bounds = new threeRuntime.Box3().setFromObject(root)
    if (!bounds || bounds.isEmpty()) return
    const center = bounds.getCenter(new threeRuntime.Vector3())
    const size = bounds.getSize(new threeRuntime.Vector3())
    const maxExtent = Math.max(size.x, size.y, size.z, 0.1)
    const offset = viewportCameraOffset(preset)
    const offsetVector = new threeRuntime.Vector3(offset[0], offset[1], offset[2]).multiplyScalar(maxExtent * 1.65)
    state.controls.target.copy(center)
    state.camera.position.copy(center).add(offsetVector)
    state.camera.near = Math.max(maxExtent / 1200, 0.0002)
    state.camera.far = Math.max(maxExtent * 220, 120)
    state.camera.lookAt(center)
    state.camera.updateProjectionMatrix()
    state.controls.update()
    state.controls.saveState()
    state.renderer.render(state.scene, state.camera)
    setViewportCameraPreset(preset)
  }

  const focusViewportTarget = (object?: THREE.Object3D | null) => {
    const state = viewerSceneRef.current
    if (!state) return
    const threeRuntime = threeRuntimeRef.current
    if (!threeRuntime) return
    let focusObject: THREE.Object3D | null | undefined = object
    if (!focusObject) focusObject = state.root
    if (!focusObject) return
    const bounds = new threeRuntime.Box3().setFromObject(focusObject)
    if (bounds.isEmpty()) return
    const center = bounds.getCenter(new threeRuntime.Vector3())
    const size = bounds.getSize(new threeRuntime.Vector3())
    const maxExtent = Math.max(size.x, size.y, size.z, 0.1)
    const offset = new threeRuntime.Vector3(0.8, 0.6, 1.1).normalize().multiplyScalar(maxExtent * 1.7)
    state.controls.target.copy(center)
    state.camera.position.copy(center).add(offset)
    state.camera.near = Math.max(maxExtent / 1200, 0.0002)
    state.camera.far = Math.max(maxExtent * 250, 120)
    state.camera.lookAt(center)
    state.camera.updateProjectionMatrix()
    state.controls.update()
    state.renderer.render(state.scene, state.camera)
  }

  const toggleViewportPartLock = (partId: string) => {
    setPartLockState((current) => ({ ...current, [partId]: !current[partId] }))
  }

  const toggleViewportMaterialLock = (materialZoneId: string) => {
    setMaterialLockState((current) => ({ ...current, [materialZoneId]: !current[materialZoneId] }))
  }

  const handleViewportPointerDown = (event: PointerEvent<HTMLCanvasElement>) => {
    setViewportFocused(true)
    setViewportActionHint('等待输入')
    event.currentTarget.focus()
    const state = viewerSceneRef.current
    if (!state || artifactLoadState !== 'ready' || event.button !== 0 || event.ctrlKey || event.altKey || event.metaKey) {
      setViewportActionHint('请先等待模型就绪后再点选')
      setHoveredObjectId(null)
      return
    }
    const rect = event.currentTarget.getBoundingClientRect()
    if (rect.width <= 0 || rect.height <= 0) return
    if (event.shiftKey) {
      event.preventDefault()
      setViewportActionHint('开始框选')
      setHoveredObjectId(null)
      setViewportMarqueeRect({
        left: event.clientX,
        top: event.clientY,
        width: 0,
        height: 0,
      })
      viewportDragRef.current = {
        mode: 'box-select',
        pointerId: event.pointerId,
        startX: event.clientX,
        startY: event.clientY,
        endX: event.clientX,
        endY: event.clientY,
      }
      state.controls.enabled = false
      event.currentTarget.setPointerCapture(event.pointerId)
      return
    }
    viewportDragRef.current = {
      ...viewportDragRef.current,
      mode: 'idle',
      pointerId: event.pointerId,
    }
    const target = pickViewportObjectFromPointer(state, event, rect, partLockState, materialLockState)
    if (!target) {
      setViewportActionHint('未命中对象')
      resetCompareSelection()
      setHoveredObjectId(null)
      return
    }
    setHoveredObjectId(target.uuid)
    setSelectedObjectId(target.uuid)
    const targetState = state.objects.get(target)
    if (targetState) {
      syncSceneTreeSelection(targetState.partId, targetState.materialZoneId, target.uuid)
      setViewportActionHint(`已选中：${targetState.partId} · 按 F 聚焦`)
    }
  }

  const handleViewportPointerMove = (event: PointerEvent<HTMLCanvasElement>) => {
    const state = viewerSceneRef.current
    if (!state || artifactLoadState !== 'ready') return
    const drag = viewportDragRef.current
    if (drag.mode === 'box-select' && drag.pointerId === event.pointerId) {
      drag.endX = event.clientX
      drag.endY = event.clientY
      setViewportMarqueeRect({
        left: Math.min(drag.startX, drag.endX),
        top: Math.min(drag.startY, drag.endY),
        width: Math.abs(drag.endX - drag.startX),
        height: Math.abs(drag.endY - drag.startY),
      })
      return
    }
    if (drag.mode !== 'idle' || event.buttons !== 0) return
    const rect = event.currentTarget.getBoundingClientRect()
    if (rect.width <= 0 || rect.height <= 0) return
    const target = pickViewportObjectFromPointer(state, event, rect, partLockState, materialLockState)
    if (!target) {
      setHoveredObjectId(null)
      return
    }
    setHoveredObjectId(target.uuid)
  }

  const handleViewportPointerLeave = () => {
    setHoveredObjectId(null)
  }

  const applyViewportViewportSelection = (event: PointerEvent<HTMLCanvasElement>) => {
    const state = viewerSceneRef.current
    const rect = event.currentTarget.getBoundingClientRect()
    const drag = viewportDragRef.current
    if (!state || !rect.width || !rect.height || drag.mode !== 'box-select') return
    if (!viewportMarqueeRect || viewportMarqueeRect.width < 5 || viewportMarqueeRect.height < 5) {
      const target = pickViewportObjectFromPointer(state, event, rect, partLockState, materialLockState)
      if (!target) {
        setViewportActionHint('框选未命中')
        resetCompareSelection()
      } else {
        const targetState = state.objects.get(target)
        setSelectedObjectId(target.uuid)
        if (targetState) {
          syncSceneTreeSelection(targetState.partId, targetState.materialZoneId, target.uuid)
          setViewportActionHint(`已命中：${targetState.partId} · 按 F 聚焦`)
        }
      }
      return
    }
    const hits = pickViewportObjectsInMarqueeRect(
      state,
      viewportMarqueeRect,
      rect,
      partLockState,
      materialLockState,
      threeRuntimeRef.current,
    )
      const first = hits[0]
    if (!first) {
      setViewportActionHint('框选完成但未命中')
      setSelectedObjectId(null)
      setSelectedPartId('all')
      setSelectedMaterialZone('all')
      setFocusedSceneTreeNodeId(null)
      setExpandedPartIds({})
      return
    }
    const targetState = state.objects.get(first)
    const targetPartId = targetState?.partId
      setSelectedObjectId(first.uuid)
    setViewportActionHint(targetPartId ? `框选命中：${targetPartId} · 按 F 聚焦` : '框选命中 · 按 F 聚焦')
    if (targetState) {
      syncSelectionViewportContext(targetState.partId, targetState.materialZoneId)
      const targetNodeId = sceneTreeMaterialNodeId(targetState.partId, targetState.materialZoneId)
      setFocusedSceneTreeNodeId(targetNodeId)
      const focusButton = sceneTreeNodeRefs.current[targetNodeId]
      if (focusButton) {
        focusButton.focus()
        focusButton.scrollIntoView({ block: 'nearest' })
      }
      setSceneTreePartExpanded(targetState.partId, true)
    }
  }

  const handleViewportPointerUp = (event: PointerEvent<HTMLCanvasElement>) => {
    const drag = viewportDragRef.current
    const state = viewerSceneRef.current
    if (drag.mode !== 'box-select' || drag.pointerId !== event.pointerId) {
      drag.mode = 'idle'
      setViewportActionHint('等待输入')
      return
    }
    drag.mode = 'idle'
    setViewportActionHint('框选结束')
    if (state) state.controls.enabled = true
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
    applyViewportViewportSelection(event)
    setViewportMarqueeRect(null)
  }

  const syncSelectionViewportContext = (partId: string, materialZoneId: string) => {
    if (materialZoneId !== 'all') {
      setSelectedPartId(partId)
      setSelectedMaterialZone(materialZoneId)
      setSelectedPass('material-id')
      return
    }
    setSelectedPartId(partId)
    setSelectedMaterialZone('all')
    setSelectedPass(partId === 'all' ? 'beauty' : 'part-id')
  }

  const focusSceneTreeNode = (nodeId: string) => {
    setFocusedSceneTreeNodeId(nodeId)
    const focus = () => {
      const nodeElement = sceneTreeNodeRefs.current[nodeId]
      if (!nodeElement) return
      nodeElement.focus()
      nodeElement.scrollIntoView({ block: 'nearest' })
    }
    if (sceneTreeNodeRefs.current[nodeId]) focus()
    else window.requestAnimationFrame(focus)
  }

  const syncSceneTreeSelection = (partId: string, materialZoneId: string, objectId?: string | null) => {
    syncSelectionViewportContext(partId, materialZoneId)
    setSceneTreePartExpanded(partId, true)
    if (objectId !== undefined) setSelectedObjectId(objectId)
    const targetNodeId = materialZoneId === 'all'
      ? sceneTreePartNodeId(partId)
      : sceneTreeMaterialNodeId(partId, materialZoneId)
    focusSceneTreeNode(targetNodeId)
  }

  const resetCompareSelection = () => {
    setSelectedPartId('all')
    setSelectedMaterialZone('all')
    setSelectedPass('beauty')
    setFocusedSceneTreeNodeId(null)
    setSelectedObjectId(null)
    setHoveredObjectId(null)
  }

  const setComparePartFilter = (partId: string) => {
    if (partId === 'all') syncSelectionViewportContext('all', selectedMaterialZone)
    else syncSelectionViewportContext(partId, 'all')
  }

  const setCompareMaterialFilter = (materialZoneId: string) => {
    syncSelectionViewportContext(selectedPartId, materialZoneId)
  }

  const handleViewportKeyDown = (event: KeyboardEvent<HTMLCanvasElement>) => {
    if (!viewportFocused) return
    if (event.key === 'f' || event.key === 'F') {
      event.preventDefault()
      focusViewportTarget(selectedSceneObject?.object ?? null)
      setViewportActionHint('F：已聚焦当前对象')
    } else if (event.key === 'r' || event.key === 'R') {
      event.preventDefault()
      resetViewport()
      setViewportActionHint('R：已重置视角')
    } else if (event.key === '1') {
      event.preventDefault()
      moveViewportCameraToPreset('front')
      setViewportActionHint('1：切到前视角')
    } else if (event.key === '2') {
      event.preventDefault()
      moveViewportCameraToPreset('left')
      setViewportActionHint('2：切到左视角')
    } else if (event.key === '3') {
      event.preventDefault()
      moveViewportCameraToPreset('top')
      setViewportActionHint('3：切到顶视角')
    } else if (event.key === '4') {
      event.preventDefault()
      moveViewportCameraToPreset('right')
      setViewportActionHint('4：切到右视角')
    } else if (event.key === '5') {
      event.preventDefault()
      moveViewportCameraToPreset('rear')
      setViewportActionHint('5：切到后视角')
    } else if (event.key === '6') {
      event.preventDefault()
      moveViewportCameraToPreset('three-quarter')
      setViewportActionHint('6：切到三分之四视角')
    } else if (event.key === 'Escape' && (selectedObjectId || selectedPartId !== 'all' || selectedMaterialZone !== 'all')) {
      event.preventDefault()
      resetCompareSelection()
      setViewportActionHint('Esc：已清除选中')
    } else if (event.key === 'z' || event.key === 'Z') {
      event.preventDefault()
      setViewportLightPreset('neutral')
      setViewportActionHint('Z：切中性光')
    } else if (event.key === 'x' || event.key === 'X') {
      event.preventDefault()
      setViewportLightPreset('high-key')
      setViewportActionHint('X：切高亮光')
    } else if (event.key === 'c' || event.key === 'C') {
      event.preventDefault()
      setViewportLightPreset('dramatic')
      setViewportActionHint('C：切轮廓光')
    }
  }

  const findSceneTreeObject = (partId: string, materialZoneId?: string): THREE.Object3D | null => {
    const state = viewerSceneRef.current
    if (!state) return null
    for (const [object, objectState] of state.objects) {
      if (objectState.partId !== partId) continue
      if (materialZoneId && objectState.materialZoneId !== materialZoneId) continue
      return object
    }
    return null
  }

  const handleSceneTreePartSelect = (partId: string) => {
    syncSceneTreeSelection(partId, 'all', null)
    const object = findSceneTreeObject(partId)
    if (object) {
      syncSceneTreeSelection(partId, 'all', object.uuid)
      focusViewportTarget(object)
      return
    }
  }

  const handleSceneTreeMaterialSelect = (partId: string, materialZoneId: string) => {
    syncSceneTreeSelection(partId, materialZoneId)
    const object = findSceneTreeObject(partId, materialZoneId)
    if (object) {
      syncSceneTreeSelection(partId, materialZoneId, object.uuid)
      focusViewportTarget(object)
      return
    }
    syncSceneTreeSelection(partId, materialZoneId, null)
  }

  const toggleTreePartVisibility = (partId: string) => {
    setPartVisibility((current) => ({ ...current, [partId]: !(current[partId] ?? true) }))
  }

  const toggleTreeMaterialVisibility = (materialZoneId: string) => {
    setMaterialVisibility((current) => ({ ...current, [materialZoneId]: !(current[materialZoneId] ?? true) }))
  }

  const toggleTreePartLock = (partId: string) => {
    toggleViewportPartLock(partId)
  }

  const toggleTreeMaterialLock = (materialZoneId: string) => {
    toggleViewportMaterialLock(materialZoneId)
  }

  const handleSceneTreeKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (sceneTreeNavigationNodes.length === 0) return
    const currentIndex = focusedSceneTreeNodeId
      ? sceneTreeNavigationNodes.findIndex((node) => node.id === focusedSceneTreeNodeId)
      : 0
    const safeCurrentIndex = currentIndex >= 0 ? currentIndex : 0
    const currentNode = sceneTreeNavigationNodes[safeCurrentIndex]
    if (!currentNode) return
    const currentPart = filteredSceneTreeByState.find((part) => part.partId === currentNode.partId)
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      const nextIndex = Math.min(sceneTreeNavigationNodes.length - 1, safeCurrentIndex + 1)
      focusSceneTreeNode(sceneTreeNavigationNodes[nextIndex]?.id ?? currentNode.id)
      return
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault()
      const prevIndex = Math.max(0, safeCurrentIndex - 1)
      focusSceneTreeNode(sceneTreeNavigationNodes[prevIndex]?.id ?? currentNode.id)
      return
    }
    if (event.key === 'Home') {
      event.preventDefault()
      if (sceneTreeNavigationNodes[0]) {
        focusSceneTreeNode(sceneTreeNavigationNodes[0].id)
      }
      return
    }
    if (event.key === 'End') {
      event.preventDefault()
      const tail = sceneTreeNavigationNodes[sceneTreeNavigationNodes.length - 1]
      if (tail) {
        focusSceneTreeNode(tail.id)
      }
      return
    }
    if (event.key === 'ArrowRight') {
      event.preventDefault()
      if (currentNode.type === 'part' && currentPart) {
        const isExpanded = Boolean(expandedPartIds[currentNode.partId])
        const hasChildren = currentPart.materials.length > 0
      if (!isExpanded && hasChildren) {
        setSceneTreePartExpanded(currentNode.partId, true)
        const firstMaterial = currentPart.materials[0]
        if (firstMaterial) {
          window.requestAnimationFrame(() => {
            const fallbackNodeId = sceneTreeMaterialNodeId(currentNode.partId, firstMaterial.materialZoneId)
            const fallbackNode = sceneTreeNodeRefs.current[fallbackNodeId]
            if (!fallbackNode) return
            focusSceneTreeNode(fallbackNodeId)
          })
        }
        return
      }
      }
      const nextIndex = Math.min(sceneTreeNavigationNodes.length - 1, safeCurrentIndex + 1)
      focusSceneTreeNode(sceneTreeNavigationNodes[nextIndex]?.id ?? currentNode.id)
      return
    }
    if (event.key === 'ArrowLeft') {
      event.preventDefault()
      if (currentNode.type === 'material') {
        const parentId = sceneTreePartNodeId(currentNode.partId)
        focusSceneTreeNode(parentId)
        return
      }
      if (currentNode.type === 'part' && expandedPartIds[currentNode.partId]) {
        setSceneTreePartExpanded(currentNode.partId, false)
        return
      }
      const prevIndex = Math.max(0, safeCurrentIndex - 1)
      focusSceneTreeNode(sceneTreeNavigationNodes[prevIndex]?.id ?? currentNode.id)
      return
    }
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      if (currentNode.type === 'part') {
        handleSceneTreePartSelect(currentNode.partId)
      } else if (currentNode.materialZoneId) {
        handleSceneTreeMaterialSelect(currentNode.partId, currentNode.materialZoneId)
      }
    }
  }

  const resetViewport = () => {
    const state = viewerSceneRef.current
    if (!state) return
    state.controls.reset()
    state.renderer.render(state.scene, state.camera)
  }

  const normalizeComparePoint = (event: PointerEvent<HTMLDivElement>): Point | null => {
    const bounds = event.currentTarget.getBoundingClientRect()
    if (bounds.width <= 0 || bounds.height <= 0) return null
    return {
      x: Math.min(1, Math.max(0, (event.clientX - bounds.left) / bounds.width)),
      y: Math.min(1, Math.max(0, (event.clientY - bounds.top) / bounds.height)),
    }
  }

  const handleComparePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (contourCanvasActive || measureMode || compareZoom <= 1) return
    if (event.target instanceof HTMLElement && event.target.closest('button, input, select')) return
    event.preventDefault()
    event.currentTarget.setPointerCapture(event.pointerId)
    comparePanRef.current = {
      active: true,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      originX: comparePan.x,
      originY: comparePan.y,
    }
  }

  const handleComparePointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const current = comparePanRef.current
    if (!current.active || current.pointerId !== event.pointerId) return
    setComparePan({
      x: current.originX + event.clientX - current.startX,
      y: current.originY + event.clientY - current.startY,
    })
  }

  const handleComparePointerUp = (event: PointerEvent<HTMLDivElement>) => {
    const current = comparePanRef.current
    if (current.pointerId !== event.pointerId) return
    comparePanRef.current.active = false
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
  }

  const appendMeasurePoint = (event: PointerEvent<SVGSVGElement>) => {
    if (!measureMode) return
    const bounds = event.currentTarget.getBoundingClientRect()
    if (bounds.width <= 0 || bounds.height <= 0) return
    const point = {
      x: Math.min(1, Math.max(0, (event.clientX - bounds.left) / bounds.width)),
      y: Math.min(1, Math.max(0, (event.clientY - bounds.top) / bounds.height)),
    }
    setMeasurePoints((current) => current.length >= 2 ? [point] : [...current, point])
  }

  const exportCompareSnapshot = async () => {
    if (!referenceDataUrl || !renderDataUrl) {
      setCompareActionStatus('unavailable')
      return
    }
    setCompareActionStatus('exporting')
    try {
      const dataUrl = await createCompareSnapshot({
        referenceDataUrl,
        renderDataUrl,
        mode: compareMode,
        flickerOn,
        zoom: compareZoom,
        pan: comparePan,
        referenceOpacity,
        renderOpacity,
        brightness: compareBrightness,
        heatmapDataUrl: diffHeatmap ? differenceHeatmapUrl ?? undefined : undefined,
      })
      const anchor = document.createElement('a')
      anchor.href = dataUrl
      anchor.download = `forgecad-${candidateId ?? 'candidate'}-${selectedPass}-compare.png`
      anchor.click()
      setCompareActionStatus('exported')
    } catch {
      setCompareActionStatus('unavailable')
    }
  }

  const measuredPixels = measurePoints.length === 2
    ? Math.round(Math.hypot((measurePoints[1]?.x ?? 0) - (measurePoints[0]?.x ?? 0), (measurePoints[1]?.y ?? 0) - (measurePoints[0]?.y ?? 0)) * HEATMAP_SIZE)
    : null
  const compareImageTransform = `translate(${comparePan.x}px, ${comparePan.y}px) scale(${compareZoom})`
  const referenceImageStyle = {
    opacity: compareMode === 'overlay' ? referenceOpacity : 1,
    transform: compareImageTransform,
  }
  const renderImageStyle = {
    opacity: compareMode === 'overlay' ? renderOpacity : 1,
    transform: compareImageTransform,
    filter: `brightness(${compareBrightness})`,
  }

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

  const sceneTreeContent = (
    <>
      <div className="runtime-scene-tree-controls" aria-label="场景树搜索、过滤和展开控制">
        <label className="scene-tree-search runtime-scene-tree-search" htmlFor="runtime-scene-tree-query">
          <span>搜索 Part / MaterialZone</span>
          <input
            id="runtime-scene-tree-query"
            type="search"
            value={sceneTreeSearch}
            onChange={(event) => setSceneTreeSearch(event.target.value)}
            placeholder="输入名称"
          />
        </label>
        <div className="runtime-scene-tree-toolbar">
          <div className="toolbar-segmented" role="group" aria-label="场景树筛选">
            <button type="button" className={`viewer-toggle ${sceneTreeFilter === 'all' ? 'viewer-toggle-active' : ''}`} onClick={() => setSceneTreeFilter('all')} disabled={viewerSceneTree.length === 0}>全部</button>
            <button type="button" className={`viewer-toggle ${sceneTreeFilter === 'visible' ? 'viewer-toggle-active' : ''}`} onClick={() => setSceneTreeFilter('visible')} disabled={viewerSceneTree.length === 0}>可见</button>
            <button type="button" className={`viewer-toggle ${sceneTreeFilter === 'locked' ? 'viewer-toggle-active' : ''}`} onClick={() => setSceneTreeFilter('locked')} disabled={viewerSceneTree.length === 0}>锁定</button>
          </div>
          <div className="toolbar-segmented" role="group" aria-label="场景树展开控制">
            <button type="button" className="viewer-toggle" onClick={() => setAllSceneTreeExpanded(true)} disabled={viewerSceneTree.length === 0} title="展开全部材质区">＋</button>
            <button type="button" className="viewer-toggle" onClick={() => setAllSceneTreeExpanded(false)} disabled={viewerSceneTree.length === 0} title="收起全部材质区">−</button>
          </div>
        </div>
      </div>
      <div
        className="scene-tree-list runtime-scene-tree-list"
        role="tree"
        tabIndex={0}
        onKeyDown={handleSceneTreeKeyDown}
        aria-multiselectable="false"
        aria-label="场景树，可用上下左右方向键导航，Enter 选择"
      >
        {filteredSceneTreeByState.length === 0 ? (
          <div className="panel-copy scene-tree-empty">{viewerSceneTree.length === 0 ? '等待 Runtime 回读模型。' : '无匹配节点，可清空搜索或切换过滤器。'}</div>
        ) : filteredSceneTreeByState.map((part, index) => {
          const partVisible = partVisibility[part.partId] ?? true
          const partLocked = Boolean(partLockState[part.partId])
          const partNodeId = sceneTreePartNodeId(part.partId)
          const isPartExpanded = Boolean(expandedPartIds[part.partId])
          const partSelected = selectedPartId === part.partId && selectedMaterialZone === 'all'
          return <div
            className="scene-tree-part"
            role="treeitem"
            aria-level={1}
            aria-posinset={index + 1}
            aria-setsize={filteredSceneTreeByState.length}
            aria-selected={partSelected}
            aria-expanded={isPartExpanded}
            key={part.partId}
          >
            <div className="runtime-scene-tree-part-row">
              <button
                type="button"
                className="runtime-scene-tree-expander"
                onClick={() => toggleSceneTreePartExpanded(part.partId)}
                aria-label={`${part.partId} ${isPartExpanded ? '收起' : '展开'}材质区`}
                aria-expanded={isPartExpanded}
                aria-controls={`runtime-scene-tree-materials-${part.partId}`}
              >{isPartExpanded ? '⌄' : '›'}</button>
              <button
                type="button"
                className={`scene-tree-row scene-tree-part-select ${partSelected ? 'scene-tree-row-selected' : ''} ${focusedSceneTreeNodeId === partNodeId ? 'scene-tree-row-focused' : ''}`}
                ref={(node) => {
                  if (node) sceneTreeNodeRefs.current[partNodeId] = node
                  else delete sceneTreeNodeRefs.current[partNodeId]
                }}
                tabIndex={focusedSceneTreeNodeId === partNodeId || (focusedSceneTreeNodeId === null && sceneTreeNavigationNodes[0]?.id === partNodeId) ? 0 : -1}
                onClick={() => handleSceneTreePartSelect(part.partId)}
                onFocus={() => setFocusedSceneTreeNodeId(partNodeId)}
                aria-pressed={partSelected}
                aria-label={`选择部件 ${part.partId}`}
              >
                <span className="scene-tree-row-title">{index === filteredSceneTreeByState.length - 1 ? '└ ' : '├ '}{part.partId}</span>
                <span className="scene-tree-row-meta">{part.objectCount}</span>
              </button>
              <button
                type="button"
                className={`runtime-scene-tree-icon-toggle ${!partVisible ? 'runtime-scene-tree-icon-toggle-off' : ''}`}
                onClick={() => toggleTreePartVisibility(part.partId)}
                aria-label={`${part.partId} ${partVisible ? '隐藏' : '显示'}`}
                aria-pressed={partVisible}
                title={`${partVisible ? '隐藏' : '显示'} ${part.partId}`}
              >{partVisible ? '◉' : '○'}</button>
              <button
                type="button"
                className={`runtime-scene-tree-icon-toggle ${partLocked ? 'runtime-scene-tree-icon-toggle-locked' : ''}`}
                onClick={() => toggleTreePartLock(part.partId)}
                aria-label={`${part.partId} ${partLocked ? '解除锁定' : '锁定'}`}
                aria-pressed={partLocked}
                title={`${partLocked ? '解除锁定' : '锁定'} ${part.partId}`}
              >{partLocked ? '🔒' : '·'}</button>
            </div>
            {isPartExpanded && <div id={`runtime-scene-tree-materials-${part.partId}`} className="scene-tree-material-list runtime-scene-tree-material-list" role="group" aria-label={`${part.partId} 的材质区`}>
              {part.materials.map((material, materialIndex) => {
                const materialVisible = materialVisibility[material.materialZoneId] ?? true
                const materialSelected = selectedPartId === part.partId && selectedMaterialZone === material.materialZoneId
                const materialLocked = Boolean(materialLockState[material.materialZoneId])
                const materialNodeId = sceneTreeMaterialNodeId(part.partId, material.materialZoneId)
                return <div
                  role="treeitem"
                  aria-level={2}
                  aria-posinset={materialIndex + 1}
                  aria-setsize={part.materials.length}
                  aria-selected={materialSelected}
                  key={`${part.partId}-${material.materialZoneId}`}
                  className="runtime-scene-tree-material-row"
                >
                  <button
                    type="button"
                    className={`scene-tree-row scene-tree-row-child scene-tree-material-select ${materialSelected ? 'scene-tree-row-selected' : ''} ${focusedSceneTreeNodeId === materialNodeId ? 'scene-tree-row-focused' : ''}`}
                    onClick={() => handleSceneTreeMaterialSelect(part.partId, material.materialZoneId)}
                    ref={(node) => {
                      if (node) sceneTreeNodeRefs.current[materialNodeId] = node
                      else delete sceneTreeNodeRefs.current[materialNodeId]
                    }}
                    tabIndex={focusedSceneTreeNodeId === materialNodeId ? 0 : -1}
                    onFocus={() => setFocusedSceneTreeNodeId(materialNodeId)}
                    aria-pressed={materialSelected}
                    aria-label={`选择材质区 ${material.materialZoneId}`}
                  >
                    <span className="scene-tree-row-title">└ {material.materialZoneId}</span>
                    <span className="scene-tree-row-meta">{material.objectCount}</span>
                  </button>
                  <button
                    type="button"
                    className={`runtime-scene-tree-icon-toggle ${!materialVisible ? 'runtime-scene-tree-icon-toggle-off' : ''}`}
                    onClick={() => toggleTreeMaterialVisibility(material.materialZoneId)}
                    aria-label={`${material.materialZoneId} ${materialVisible ? '隐藏' : '显示'}`}
                    aria-pressed={materialVisible}
                    title={`${materialVisible ? '隐藏' : '显示'} ${material.materialZoneId}`}
                  >{materialVisible ? '◉' : '○'}</button>
                  <button
                    type="button"
                    className={`runtime-scene-tree-icon-toggle ${materialLocked ? 'runtime-scene-tree-icon-toggle-locked' : ''}`}
                    onClick={() => toggleTreeMaterialLock(material.materialZoneId)}
                    aria-label={`${material.materialZoneId} ${materialLocked ? '解除锁定' : '锁定'}`}
                    aria-pressed={materialLocked}
                    title={`${materialLocked ? '解除锁定' : '锁定'} ${material.materialZoneId}`}
                  >{materialLocked ? '🔒' : '·'}</button>
                </div>
              })}
            </div>}
          </div>
        })}
      </div>
    </>
  )

  const sceneOverview = (
    <aside className="runtime-scene-panel" aria-labelledby="runtime-scene-overview-title">
      <div className="runtime-workbench-panel-header">
        <div>
          <span className="runtime-workbench-panel-eyebrow">SCENE / 场景</span>
          <h2 id="runtime-scene-overview-title">场景</h2>
        </div>
        <span className="runtime-workbench-panel-count">{viewerSceneTree.length} 部件</span>
      </div>
      <div className="runtime-scene-tree-scroll">
        <button
          type="button"
          className={`tree-row tree-row-root runtime-scene-root ${selectedPartId === 'all' && selectedMaterialZone === 'all' ? 'tree-row-selected' : ''}`}
          onClick={() => resetCompareSelection()}
          aria-pressed={selectedPartId === 'all' && selectedMaterialZone === 'all'}
        >
          <span>{projectName || 'Robot'}</span>
          <span className="tree-row-id">{viewerSceneTree.length ? 'model' : '等待'}</span>
        </button>
        {sceneTreeContent}
        {selectedSceneObject ? (
          <div className="selected-object-card runtime-selected-object">
            <span className="selected-object-icon" aria-hidden="true">◎</span>
            <div>
              <strong>当前选中</strong>
              <small>{selectedSceneObject.data.partId}</small>
              <small>{selectedSceneObject.data.materialZoneId}</small>
            </div>
            <div className="runtime-selected-object-controls">
              <button type="button" className="viewer-toggle" onClick={() => focusViewportTarget(selectedSceneObject.object)}>聚焦</button>
              <button type="button" className="viewer-toggle" onClick={() => toggleTreePartLock(selectedSceneObject.data.partId)}>{selectedSceneObjectLockState.selectedPartLocked ? '解锁' : '锁定'}</button>
            </div>
          </div>
        ) : null}
      </div>
      <div className="scene-footer">
        <span><span className="status-led" />{selectedPartId === 'all' ? '全部部件' : selectedPartId}</span>
        <span>{filteredSceneTreeByState.length}/{viewerSceneTree.length}</span>
      </div>
    </aside>
  )

  return <main className="runtime-shell">
    <header className="runtime-app-bar">
      <div className="runtime-brand-lockup">
        <span className="runtime-brand-mark" aria-hidden="true">◇</span>
        <strong>ForgeCAD</strong>
        <span className="runtime-project-name">{projectName}</span>
      </div>
      <div className={`runtime-codex-connection ${ready ? 'runtime-codex-connection-ready' : ''}`} role="status">
        <span className="runtime-connection-dot" aria-hidden="true" />
        {ready ? 'Codex 已连接' : 'Codex / Runtime 未连接'}
      </div>
      <div className="runtime-app-actions">
        <button type="button" className="runtime-top-action" disabled title="Viewer 只读；撤销由 Codex 通过 Runtime 批准链路完成">↶ 撤销</button>
        <button type="button" className="runtime-top-action runtime-top-action-primary" onClick={() => onNavigate?.('export')} title="打开受保护的导出页">导出</button>
      </div>
    </header>
    {errorConsoleItems.length > 0 && (
        <section className="panel-section error-console" aria-labelledby="error-console-title">
        <div className="section-toolbar">
          <div><p className="section-kicker">异常台</p><h2 id="error-console-title">统一异常面板</h2></div>
          <button type="button" className="viewer-toggle" onClick={() => setModelRefreshNonce((value) => value + 1)} disabled={modelRefreshing}>{modelRefreshing ? '重试中…' : '重试模型读取'}</button>
        </div>
        <div className="error-console-summary" role="status" aria-live="polite">
          <span className="error-summary-chip">
            <strong>总计</strong><code>{errorSummaryByCategory.total}</code>
          </span>
          <span className="error-summary-chip">
            <strong>错误</strong><code className="error-summary-code-error">{errorSummaryByCategory.errorCount}</code>
          </span>
          <span className="error-summary-chip">
            <strong>警告</strong><code className="error-summary-code-warn">{errorSummaryByCategory.warnCount}</code>
          </span>
          <span className="error-summary-chip">
            <strong>读取失败</strong><code>{errorSummaryByCategory.counts.读取失败}</code>
          </span>
          <span className="error-summary-chip">
            <strong>加载失败</strong><code>{errorSummaryByCategory.counts.加载失败}</code>
          </span>
          <span className="error-summary-chip">
            <strong>绑定不一致</strong><code>{errorSummaryByCategory.counts.绑定不一致}</code>
          </span>
          <span className="error-summary-chip">
            <strong>数据未可用</strong><code>{errorSummaryByCategory.counts.数据未可用}</code>
          </span>
          <span className="error-summary-chip">
            <strong>未就绪</strong><code>{errorSummaryByCategory.counts.未就绪}</code>
          </span>
          <span className="error-summary-chip">
            <strong>异常</strong><code>{errorSummaryByCategory.counts.异常}</code>
          </span>
        </div>
        <div className="error-console-list">
          {errorConsoleItems.map((item) => (
            <div key={item.id} className={`error-console-item error-console-item-${item.severity}`} role={item.severity === 'error' ? 'alert' : 'status'}>
              <div className="error-console-item-title">
                <span className={`status-icon ${item.severity === 'warn' ? 'status-icon-muted' : 'status-icon-error'}`}>{item.severity === 'warn' ? 'i' : '!'}</span>
                <strong>{item.title}</strong>
                <span className="error-console-item-category">{item.category}</span>
                <code>{item.code}</code>
              </div>
              <p>{item.summary}</p>
              <p><strong>范围：</strong>{item.scope}</p>
              <p><strong>处理建议：</strong>{item.meaning}</p>
              {item.action && item.actionLabel ? <button type="button" className="viewer-toggle" onClick={item.action}>{item.actionLabel}</button> : null}
            </div>
          ))}
        </div>
      </section>
    )}
    <section className="runtime-workbench-grid" aria-label="ForgeCAD runtime viewer">
      {sceneOverview}
      <div className="viewport-card runtime-viewport-card">
        <div className="viewport-toolbar">
          <span>当前活跃快照</span>
          <span className="toolbar-muted">{ready ? (project?.record?.head_snapshot_id ? '已读取当前快照' : '暂无已确认快照') : '等待 Runtime'}</span>
          <button type="button" className="viewer-toggle" onClick={resetViewport} disabled={!viewerSceneRef.current}>重置视角</button>
        </div>
        <div className="candidate-toolbar" aria-label="候选与历史版本选择">
          <label>当前候选 / 历史<select value={selectedCandidateId ?? AUTO_LATEST_CANDIDATE} onChange={(event) => setSelectedCandidateId(event.target.value === AUTO_LATEST_CANDIDATE ? null : event.target.value)} disabled={candidateSummaries.length === 0}>
            <option value={AUTO_LATEST_CANDIDATE}>自动 · 最新任务 {automaticCandidateId ? `(${automaticCandidateId})` : ''}</option>
            {candidateSummaries.map((candidate) => <option key={candidate.candidate_id} value={candidate.candidate_id}>{candidate.candidate_id} · {formatCandidateState(candidate.state ?? '未知')} · {buildGenerationTimestamp(candidate.updated_at ?? candidate.created_at) ?? '时间缺失'}</option>)}
          </select></label>
          <div className="toolbar-segmented" role="group" aria-label="候选排序方式">
            <button type="button" className={`viewer-toggle ${candidateSortMode === 'time' ? 'viewer-toggle-active' : ''}`} onClick={() => setCandidateSortMode('time')} disabled={candidateSummaries.length < 2}>
              按时间
            </button>
            <button type="button" className={`viewer-toggle ${candidateSortMode === 'id' ? 'viewer-toggle-active' : ''}`} onClick={() => setCandidateSortMode('id')} disabled={candidateSummaries.length < 2}>
              按任务ID
            </button>
          </div>
          <button type="button" className="viewer-toggle" onClick={() => setCandidateSortOrder((value) => value === 'newest' ? 'oldest' : 'newest')} disabled={candidateSummaries.length < 2}>{candidateSortOrder === 'newest' ? '最新 → 最旧' : '最旧 → 最新'}</button>
          <span className="candidate-selection-badge"><span className="status-icon status-icon-info">{selectedCandidateIsManual ? 'M' : 'A'}</span>{selectedCandidateIsManual ? '手动候选' : '自动最新候选'} · 任务ID {candidateId ?? '无'}</span>
        </div>
        <div className="viewport-toolbar">
          <div className="toolbar-segmented" role="group" aria-label="3D 视角预设">
            {VIEWPORT_CAMERA_PRESETS.map((preset) => <button
              type="button"
              key={preset.id}
              className={`viewer-toggle ${viewportCameraPreset === preset.id ? 'viewer-toggle-active' : ''}`}
              onClick={() => moveViewportCameraToPreset(preset.id)}
            >
              {preset.label}
            </button>)}
          </div>
          <span className="toolbar-muted">当前：{VIEWPORT_CAMERA_PRESETS.find((preset) => preset.id === viewportCameraPreset)?.label ?? '前视角'}</span>
        </div>
        <div className="viewport-stage" aria-label={artifact ? 'GLB 资产回读视口' : '3D 视口占位区域'}><div className="viewport-crosshair" aria-hidden="true" />{artifact ? <><canvas
              ref={canvasRef}
              className="glb-canvas"
              tabIndex={0}
              aria-label="Runtime GLB 三维预览；左键选中，Shift 加左键框选，右键旋转，中键平移，滚轮缩放，F 聚焦，R 重置视角"
              onFocus={() => {
                setViewportFocused(true)
                setViewportActionHint('快捷键已激活（F/R/Z/X/C/1-6）')
              }}
              onBlur={() => {
                setViewportFocused(false)
                setViewportActionHint('视口未聚焦，请点击画布')
              }}
              onPointerDown={handleViewportPointerDown}
              onPointerMove={handleViewportPointerMove}
              onPointerUp={handleViewportPointerUp}
              onPointerCancel={handleViewportPointerUp}
              onPointerLeave={handleViewportPointerLeave}
              onKeyDown={handleViewportKeyDown}
              onContextMenu={(event) => event.preventDefault()}
            />{viewportMarqueeRect && <div className="viewport-marquee" style={{
              left: `${viewportMarqueeRect.left - (canvasRef.current?.getBoundingClientRect().left ?? 0)}px`,
              top: `${viewportMarqueeRect.top - (canvasRef.current?.getBoundingClientRect().top ?? 0)}px`,
              width: `${viewportMarqueeRect.width}px`,
              height: `${viewportMarqueeRect.height}px`,
            }} />}
            <ul className="viewport-hints" aria-live="polite">
              {viewportHintItems.map((hint, index) => <li key={`${hint}-${index}`} className="viewport-hint-item">{hint}</li>)}
              <li className="viewport-hint-item">{viewportControlHint}</li>
            </ul>
            <div className="viewport-message" role={artifactLoadState === 'error' ? 'alert' : 'status'} aria-live="polite"><span className="viewport-icon">◇</span><strong>{artifactLoadState === 'loading' ? '正在加载 GLB…' : artifactLoadState === 'error' ? 'GLB 加载失败' : 'GLB 读取通道已连接'}</strong><span>{artifactLoadState === 'error' ? `故障码：${artifactError ?? 'ARTIFACT_LOAD_FAILED'}` : `${partCount} 个语义部件 · ${artifact.triangle_count ?? 0} 三角形 · UV ${artifact.uv_status ?? '未知'} · 切线 ${artifact.tangent_status ?? '未知'}`}</span><code>{artifact.artifact_id}</code>{artifactLoadState === 'error' && <button type="button" className="viewer-toggle" onClick={() => setArtifactRetryNonce((value) => value + 1)}>重试 GLB</button>}</div></> : <div className="viewport-message" role="status"><span className="viewport-icon">◇</span><strong>等待 Codex 提交设计</strong><span>这里仅查看模型、材质、参考比较和版本状态。</span></div>}</div>
          <div className="viewport-footer">
            <span>项目：{projectName}</span>
            <span>版本：{versionCount}</span>
            <span>候选：{activeCandidate?.candidate?.state ? formatCandidateState(activeCandidate.candidate.state) : '无'}</span>
            <span>任务ID：{candidateId ?? '无'}</span>
          </div>
        <section className="compare-panel" aria-label="参考图与固定渲染对比">
          <div className="compare-header">
            <div><p className="section-kicker">参考比较</p><h2>固定视图证据</h2></div>
            <div className="compare-status"><span className={`status-icon ${visualStatusClass === 'passed' ? 'status-icon-pass' : visualStatusClass === 'failed' ? 'status-icon-error' : visualStatusClass === 'partial' ? 'status-icon-info' : 'status-icon-muted'}`}>{visualStatusIcon}</span><span>{visualStatusLabel}</span><code>{visualStatus}</code></div>
          </div>
          <div className="compare-toolbar">
            <div className="toolbar-muted">语义联动：{compareSelectionHint}</div>
            <div className="aov-tabs" role="tablist" aria-label="渲染 AOV 通道">
            {AOV_PASSES.map((pass) => <button key={pass} id={`render-aov-tab-${pass}`} role="tab" aria-controls="render-aov-panel" tabIndex={selectedPass === pass ? 0 : -1} type="button" className={`aov-tab ${selectedPass === pass ? 'aov-tab-active' : ''}`} aria-selected={selectedPass === pass} onClick={() => focusAovTab(pass)} onKeyDown={(event) => handleAovKeyDown(event, pass)}>{AOV_PASS_LABELS[pass]}</button>)}
          </div>
          <div className="mode-tabs" role="group" aria-label="对比模式">
            {(['split', 'overlay', 'flicker'] as CompareMode[]).map((mode) => <button key={mode} type="button" className={`mode-tab ${compareMode === mode ? 'mode-tab-active' : ''}`} aria-pressed={compareMode === mode} onClick={() => setCompareMode(mode)}>{COMPARE_MODE_LABELS[mode]}</button>)}
          </div>
          </div>
          <div className="viewer-controls" aria-label="部件与材质控制">
            <label>部件<select value={selectedPartId} onChange={(event) => setComparePartFilter(event.target.value)} disabled={partIds.length === 0}><option value="all">全部部件</option>{partIds.map((partId) => <option key={partId} value={partId}>{partId}</option>)}</select></label>
            <label>材质区<select value={selectedMaterialZone} onChange={(event) => setCompareMaterialFilter(event.target.value)} disabled={materialZoneIds.length === 0}><option value="all">全部材质区</option>{materialZoneIds.map((zoneId) => <option key={zoneId} value={zoneId}>{zoneId}</option>)}</select></label>
            <button type="button" className={`viewer-toggle ${exploded ? 'viewer-toggle-active' : ''}`} aria-pressed={exploded} onClick={() => setExploded((value) => !value)}>爆炸图</button>
            <button type="button" className={`viewer-toggle ${contourCanvasActive ? 'viewer-toggle-active' : ''}`} aria-pressed={contourCanvasActive} onClick={() => { setSelectedPass('silhouette'); setCompareMode('overlay'); setDiffHeatmap(false) }}>轮廓画布</button>
            <button type="button" className={`viewer-toggle ${diffHeatmap ? 'viewer-toggle-active' : ''}`} aria-pressed={diffHeatmap} onClick={() => setDiffHeatmap((value) => !value)}>差异热图</button>
            <button type="button" className={`viewer-toggle ${measureMode ? 'viewer-toggle-active' : ''}`} aria-pressed={measureMode} onClick={() => { setMeasureMode((value) => !value); setMeasurePoints([]) }}>标尺测量</button>
            <button type="button" className="viewer-toggle" onClick={resetCompareSelection}>清空语义筛选</button>
            <button type="button" className="viewer-toggle" onClick={resetCompareView}>重置比较视图</button>
            <button type="button" className="viewer-toggle" onClick={() => void exportCompareSnapshot()} disabled={!referenceDataUrl || !renderDataUrl || compareActionStatus === 'exporting'}>{compareActionStatus === 'exporting' ? '导出中…' : compareActionStatus === 'exported' ? '已导出截图' : '导出当前视图'}</button>
          </div>
          <div className="compare-parameters" aria-label="比较参数">
            <label>缩放 {Math.round(compareZoom * 100)}%<input type="range" min="1" max="4" step="0.1" value={compareZoom} onChange={(event) => setCompareZoom(Number(event.target.value))} /></label>
            <label>亮度 {Math.round(compareBrightness * 100)}%<input type="range" min="0.5" max="1.8" step="0.05" value={compareBrightness} onChange={(event) => setCompareBrightness(Number(event.target.value))} /></label>
            {compareMode === 'overlay' && <><label>参考透明度 {Math.round(referenceOpacity * 100)}%<input type="range" min="0.1" max="1" step="0.05" value={referenceOpacity} onChange={(event) => setReferenceOpacity(Number(event.target.value))} /></label><label>渲染透明度 {Math.round(renderOpacity * 100)}%<input type="range" min="0.1" max="1" step="0.05" value={renderOpacity} onChange={(event) => setRenderOpacity(Number(event.target.value))} /></label></>}
            {diffHeatmap && <label>热图敏感度 {heatmapSensitivity.toFixed(1)}×<input type="range" min="0.5" max="2.5" step="0.1" value={heatmapSensitivity} onChange={(event) => setHeatmapSensitivity(Number(event.target.value))} /></label>}
            <span className="compare-parameter-hint">{compareZoom > 1 ? '拖拽画面平移' : '放大后可拖拽平移'}{measureMode ? ' · 点击两点完成测量' : ''}</span>
          </div>
          <div id="render-aov-panel" role="tabpanel" aria-labelledby={`render-aov-tab-${selectedPass}`} className={`compare-stage compare-${compareMode} ${contourCanvasActive ? 'contour-canvas' : ''} ${diffHeatmap ? 'compare-heatmap' : ''} ${compareZoom > 1 && !measureMode && !contourCanvasActive ? 'compare-pan-ready' : ''}`} aria-label={`${selectedPass} 参考对比`} onPointerDown={handleComparePointerDown} onPointerMove={handleComparePointerMove} onPointerUp={handleComparePointerUp} onPointerCancel={handleComparePointerUp}>
            {contourCanvasActive && <div className="contour-canvas-badge">轮廓草图画布 · SILHOUETTE AOV</div>}
            {referenceDataUrl && (compareMode === 'split' || compareMode === 'overlay' || (compareMode === 'flicker' && !flickerOn)) && <div className="compare-pane compare-reference"><span>参考图</span><img src={referenceDataUrl} alt="授权参考图" style={referenceImageStyle} onError={() => { setCompareLoadState('error'); setCompareError('REFERENCE_IMAGE_DECODE_FAILED') }} /></div>}
            {renderDataUrl && (compareMode === 'split' || compareMode === 'overlay' || (compareMode === 'flicker' && flickerOn)) && <div className="compare-pane compare-render"><span>{selectedPass}</span><img src={renderDataUrl} alt={`固定渲染 ${selectedPass}`} style={renderImageStyle} onError={() => { setCompareLoadState('error'); setCompareError('RENDER_PASS_IMAGE_DECODE_FAILED') }} /></div>}
            {contourCanvasActive && referenceContourAidUrl && <div className="reference-contour-aid"><span>参考轮廓引导 · 仅 Viewer</span><img src={referenceContourAidUrl} alt="确定性参考轮廓引导" /></div>}
            {contourCanvasActive && referenceDataUrl && <svg
              className="contour-annotation-layer"
              viewBox="0 0 1 1"
              role="img"
              aria-label="临时参考轮廓草图；不会写入 Runtime"
              onPointerDown={handleContourPointerDown}
              onPointerMove={handleContourPointerMove}
              onPointerUp={handleContourPointerUp}
              onPointerCancel={handleContourPointerUp}
            >
              {contourPoints.length > 1 && <polyline points={contourPoints.map((point) => `${point.x},${point.y}`).join(' ')} fill="none" stroke="#ffb15a" strokeWidth="0.006" vectorEffect="non-scaling-stroke" />}
              {contourPoints.length > 2 && <line x1={contourPoints[contourPoints.length - 1]?.x} y1={contourPoints[contourPoints.length - 1]?.y} x2={contourPoints[0]?.x} y2={contourPoints[0]?.y} stroke="#ffb15a" strokeWidth="0.003" strokeDasharray="0.012 0.008" vectorEffect="non-scaling-stroke" />}
              {contourPoints.map((point, index) => <circle key={`${point.x}-${point.y}-${index}`} cx={point.x} cy={point.y} r="0.008" fill="#ffb15a" stroke="#26180c" strokeWidth="0.003" vectorEffect="non-scaling-stroke" />)}
            </svg>}
            {measureMode && referenceDataUrl && <svg className="measure-layer" viewBox="0 0 1 1" role="img" aria-label="临时两点测量；不会写入 Runtime" onPointerDown={appendMeasurePoint}>
              {measurePoints.length === 2 && <line x1={measurePoints[0]?.x} y1={measurePoints[0]?.y} x2={measurePoints[1]?.x} y2={measurePoints[1]?.y} stroke="#8bd6ff" strokeWidth="0.004" vectorEffect="non-scaling-stroke" />}
              {measurePoints.map((point, index) => <circle key={`measure-${point.x}-${point.y}-${index}`} cx={point.x} cy={point.y} r="0.009" fill="#8bd6ff" stroke="#092031" strokeWidth="0.003" vectorEffect="non-scaling-stroke" />)}
              {measuredPixels !== null && <text x="0.02" y="0.96" fill="#d9f2ff" fontSize="0.035" stroke="#06121c" strokeWidth="0.006" paintOrder="stroke">约 {measuredPixels}px / 512</text>}
            </svg>}
            {diffHeatmap && differenceHeatmapUrl && <div className="heatmap-layer"><span>像素差异 · 512×512</span><img className="heatmap-image" src={differenceHeatmapUrl} alt="参考图与渲染图差异热图" /></div>}
            {diffHeatmap && <div className="heatmap-legend" role="status">{differenceHeatmapUrl ? '差异热图已由后台 Worker 生成；数值质量仍以 Runtime QualityReport 为准。' : compareError ? `热图失败：${compareError}` : '正在生成当前参考图与 Render AOV 的差异热图…'}</div>}
            {compareLoadState === 'loading' && <div className="compare-empty">正在读取候选绑定的参考图与 {selectedPass} AOV…</div>}
            {compareLoadState === 'error' && <div className="compare-error" role="alert"><span className="status-icon status-icon-error">!</span><span>比较资源读取失败：{compareError ?? 'COMPARE_ASSET_LOAD_FAILED'}</span><button type="button" className="viewer-toggle" onClick={() => setCompareRetryNonce((value) => value + 1)}>重试比较</button></div>}
            {compareLoadState === 'idle' && (!referenceDataUrl || !renderDataUrl) ? <div className="compare-empty">等待候选绑定的参考图、RenderSet 和 {selectedPass} PNG</div> : null}
          </div>
          {contourCanvasActive && referenceDataUrl && <div className="contour-draft-toolbar" aria-label="临时轮廓草图工具">
            <span aria-live="polite">临时轮廓点：{contourPoints.length}/128 · {contourBindingReady ? '候选已绑定' : '等待候选绑定证据'} · 仅 Viewer 显示</span>
            <button type="button" className="viewer-toggle" onClick={undoContourPoint} disabled={contourPoints.length === 0}>撤销上一点</button>
            <button type="button" className="viewer-toggle" onClick={clearContourDraft} disabled={contourPoints.length === 0}>清除草图</button>
            <button type="button" className="viewer-toggle" onClick={() => void copyContourDraft()} disabled={contourPoints.length < 3 || !contourBindingReady}>{contourCopyStatus === 'copied' ? '已复制给 Codex' : '复制哈希绑定轮廓点集'}</button>
            {contourCopyStatus === 'unavailable' && <span role="status">需要至少 3 个点、同一候选的 reference/render/comparison hash 以及可用剪贴板；点集仅保存在 Viewer 内存。</span>}
          </div>}
          <div className="compare-footer"><span>相机锁定：512×512 固定透视</span><span>RenderSet：{renderSetHash ?? '未运行'}</span><span>参考ID：{referenceId ?? '未运行'}</span>{measuredPixels !== null && <span>临时测量：{measuredPixels}px</span>}{compareActionStatus === 'unavailable' && <span role="status">导出失败或资源不可用</span>}</div>
        </section>
      </div>
      <aside className="runtime-panel runtime-inspector-panel">
        <div className="runtime-inspector-scroll">
        <section className="panel-section runtime-inspector-summary" aria-labelledby="runtime-inspector-title">
          <div className="runtime-workbench-panel-header">
            <div>
              <span className="runtime-workbench-panel-eyebrow">INSPECTOR / 属性</span>
              <h2 id="runtime-inspector-title">属性检查器</h2>
            </div>
            <span className={`selection-dot ${selectedSceneObject ? 'selection-dot-active' : ''}`} aria-label={selectedSceneObject ? '已选中对象' : '未选中对象'} />
          </div>
          <div className="runtime-inspector-selection">
            <strong>{selectedSceneObject?.data.partId ?? '未选中对象'}</strong>
            <span>{selectedSceneObject ? `材质区：${selectedSceneObject.data.materialZoneId}` : '从场景树或 3D 视口选择一个部件'}</span>
          </div>
          <div className="property-list runtime-inspector-properties">
            <div><span>变换 Transform</span><strong>{selectedSceneObject ? 'Runtime 回读' : '未选中'}</strong></div>
            <div><span>材质 Material</span><strong>{selectedSceneObject?.data.materialZoneId ?? '未选中'}</strong></div>
            <div><span>几何 Geometry</span><strong>{artifact ? `${partCount} Part · ${artifact.triangle_count ?? 0} 三角形` : '未绑定 GLB'}</strong></div>
          </div>
          <p className="inspector-note">属性检查器只显示 Runtime 已回读字段；位置、材质和几何修改仍需由 Codex 提交并经过批准。</p>
        </section>
        <section className="panel-section runtime-panel-route"><p className="section-kicker">控制路径</p><h2>Codex 是唯一外部 Agent</h2><p className="panel-copy">普通用户在 Codex 中对话并上传授权参考图。Codex 通过 MCP 工具提交类型化请求，ForgeCAD 不内置模型、聊天页或 API Key。</p></section>
        <section className="panel-section"><p className="section-kicker">实时约束</p><div className="capability-list">{capabilities.map(([label, value]) => <div className="capability-row" key={label}><span>{label}</span><strong>{value}</strong></div>)}</div></section>
        <section className="panel-section" aria-labelledby="candidate-snapshot-title">
          <p className="section-kicker">候选快照</p>
          <h2 id="candidate-snapshot-title">当前候选与历史快照比对</h2>
          <div className="snapshot-compare-toolbar" aria-label="候选快照对比选择">
            <label>
              对比候选
              <select
                value={snapshotCompareCandidateId ?? AUTO_LATEST_CANDIDATE}
                onChange={(event) => setSnapshotCompareCandidateId(event.target.value === AUTO_LATEST_CANDIDATE ? null : event.target.value)}
                disabled={!activeSnapshot || candidateSnapshots.length < 2}
              >
                <option value={AUTO_LATEST_CANDIDATE}>自动 · 上一个候选{automaticSnapshotCompare ? `（${automaticSnapshotCompare.candidateId}）` : ''}</option>
                {candidateSnapshots.filter((snapshot) => snapshot.candidateId !== candidateId).map((snapshot) => (
                  <option key={snapshot.candidateId} value={snapshot.candidateId}>
                    {snapshot.candidateId} · {formatCandidateState(snapshot.candidateState)} · {snapshot.createdAtText ?? '时间缺失'}
                  </option>
                ))}
              </select>
            </label>
            <span className="snapshot-compare-mode">{snapshotCompareIsManual ? '手动历史候选' : '自动上一候选'} · 比对只读取快照，不改变 Runtime</span>
          </div>
          <div className="snapshot-chain-summary">
            <span>链路联动：</span>
            <span className={`snapshot-chain-chip ${activeCandidateChain.artifactReady ? 'snapshot-chain-chip-ok' : 'snapshot-chain-chip-missing'}`}>GLB</span>
            <span className={`snapshot-chain-chip ${activeCandidateChain.compareReady ? 'snapshot-chain-chip-ok' : 'snapshot-chain-chip-missing'}`}>对比</span>
            <span className={`snapshot-chain-chip ${activeCandidateChain.evidenceReady ? 'snapshot-chain-chip-ok' : 'snapshot-chain-chip-missing'}`}>证据</span>
            <span className={`snapshot-chain-chip ${activeCandidateChain.qualityReady ? 'snapshot-chain-chip-ok' : 'snapshot-chain-chip-missing'}`}>质量</span>
            <span>可视状态：{activeCandidateChain.visualStatus}</span>
          </div>
          {!activeSnapshot ? <p className="panel-copy">当前候选无快照数据，等待候选数据可用。</p> : (
            <div className="snapshot-compare">
              <div className="snapshot-card snapshot-card-current">
                <div className="snapshot-card-title">
                  <span>当前候选：{activeSnapshot.candidateName}</span>
                  <code>{formatCandidateState(activeSnapshot.candidateState)}</code>
                </div>
                <div className="snapshot-metrics">
                  <span>部件：{activeSnapshot.partCount}</span>
                  <span>材质区：{activeSnapshot.materialZoneCount}</span>
                  <span>三角形：{activeSnapshot.triangleCount}</span>
                  <span>耗时：{activeSnapshotTiming?.elapsedDisplay ?? activeSnapshotTiming?.statusLabel ?? '未运行'}</span>
                  <span>UV: {activeSnapshot.uvStatus}</span>
                  <span>切线: {activeSnapshot.tangentStatus}</span>
                  <span>校验: {activeSnapshot.validatorStatus}</span>
                  <span>可视化: {candidateQuickPreview?.visualStatus}</span>
                  <span>
                    质量门：
                    <strong className={`workflow-gate-status ${candidateQuickPreview ? `workflow-gate-status-${candidateQuickPreview.gateStatusClass}` : ''}`}>
                      {candidateQuickPreview?.quality ?? '未绑定'}
                    </strong>
                  </span>
                </div>
                <div className="snapshot-meta">
                  <span>结构：{compactHash(activeSnapshot.candidateCanonicalSha256)}</span>
                  <span>几何程序：{compactHash(activeSnapshot.programSha256)}</span>
                  <span>部件清单：{compactSignature(activeSnapshot.partIdSignature)}</span>
                  <span>材质区清单：{compactSignature(activeSnapshot.materialZoneSignature)}</span>
                  <span>GLB：{activeSnapshot.artifactId ?? '未就绪'}</span>
                  <span>参考：{activeSnapshot.referenceId ? '已绑定' : '未绑定'}</span>
                  <span>渲染集：{activeSnapshot.renderSetHash ? '已绑定' : '未绑定'}</span>
                  <span>比对：{activeSnapshot.comparisonReportHash ? '已绑定' : '未绑定'}</span>
                  <span>质量报告：{activeSnapshot.qualityReportHash || activeSnapshot.hasVisualEvidenceBinding ? '已绑定' : '未绑定'}</span>
                  <span>绑定摘要：{activeSnapshotBinding ? formatBindingStatusText(activeSnapshotBinding) : '—'}</span>
                </div>
              </div>
              <div className="snapshot-card snapshot-card-comparison">
                <div className="snapshot-card-title">
                  <span>{snapshotCompareIsManual ? '手动对比候选' : '上一候选'}：{comparisonSnapshot?.candidateName ?? '无历史候选'}</span>
                  <code>{comparisonSnapshot ? formatCandidateState(comparisonSnapshot.candidateState) : '—'}</code>
                </div>
                {comparisonSnapshot ? (
                  <div className="snapshot-metrics">
                    <span>部件：{comparisonSnapshot.partCount}</span>
                    <span>材质区：{comparisonSnapshot.materialZoneCount}</span>
                    <span>三角形：{comparisonSnapshot.triangleCount}</span>
                    <span>耗时：{comparisonSnapshotTiming?.elapsedDisplay ?? comparisonSnapshotTiming?.statusLabel ?? '未运行'}</span>
                    <span>UV: {comparisonSnapshot.uvStatus}</span>
                    <span>切线: {comparisonSnapshot.tangentStatus}</span>
                    <span>校验: {comparisonSnapshot.validatorStatus}</span>
                    <span>可视化: {formatQualityStatus(comparisonSnapshot.visualStatus)}</span>
                    <span>
                      质量门：
                      <strong className={`workflow-gate-status ${comparisonSnapshot.hasVisualEvidenceBinding ? `workflow-gate-status-${comparisonSnapshot.qualityPass ? 'passed' : 'failed'}` : 'workflow-gate-status-not-run'}`}>
                    {comparisonSnapshot.hasVisualEvidenceBinding
                          ? (comparisonSnapshot.qualityPass ? '通过' : '未通过')
                          : '待绑定'}
                      </strong>
                    </span>
                    {comparisonSnapshotBinding ? <span>绑定摘要：{formatBindingStatusText(comparisonSnapshotBinding)}</span> : null}
                    <span>结构：{compactHash(comparisonSnapshot.candidateCanonicalSha256)}</span>
                    <span>几何程序：{compactHash(comparisonSnapshot.programSha256)}</span>
                  </div>
                ) : <p className="panel-copy">还没有历史候选用于对比。</p>}
              </div>
            </div>
          )}
          {candidateSnapshotDiff.length > 0 ? (
            <>
              <div className="workflow-gates snapshot-diff-list" aria-label="候选快照差异">
                {candidateSnapshotDiff.map((row) => (
                  <div key={row.label} className="workflow-gate-row">
                    <span>{row.label}</span>
                    <div>
                      <strong className={`workflow-gate-status ${snapshotStatusClass(row.status)}`}>
                        {row.current}
                      </strong>
                      <span className="snapshot-diff-prev">↔</span>
                      <code className={`workflow-gate-status ${snapshotStatusClass(row.status)}`}>{row.previous}</code>
                    </div>
                  </div>
                ))}
              </div>
              {snapshotBindingDelta.length > 0 ? (
                <div className="snapshot-binding-delta">
                  {snapshotBindingDelta.map((delta) => <span key={`${delta.label}-${delta.value}`} className={`snapshot-binding-delta-item ${snapshotDeltaClass(delta.direction)}`}>{`${delta.label}: ${delta.value}`}</span>)}
                </div>
              ) : null}
              <div className="snapshot-diff-controls section-toolbar">
                <button type="button" className="viewer-toggle" onClick={() => comparisonSnapshot && setSelectedCandidateId(comparisonSnapshot.candidateId)} disabled={!comparisonSnapshot}>
                  快速回看对比候选
                    </button>
                <span>当前候选的耗时、GLB、对比图和 Runtime 质量报告已绑定；差异字段包含结构、部件、材质区、UV/切线和证据。</span>
              </div>
            </>
          ) : null}
        </section>
        <section className="panel-section" aria-labelledby="generation-timing-title">
          <div className="section-toolbar">
            <p className="section-kicker">生成耗时面板</p>
            <button
              type="button"
              className="viewer-toggle"
              onClick={() => setTimingSortOrder((value) => value === 'desc' ? 'asc' : 'desc')}
              disabled={generationTimings.length < 2}
            >
              按任务ID排序：{timingSortOrder === 'desc' ? '降序 (从新到旧)' : '升序 (从旧到新)'}
            </button>
          </div>
          <h2 id="generation-timing-title">任务生成耗时统计</h2>
          <div className="workflow-summary">
            <div className="workflow-current">
              <span>任务总数</span>
              <strong>{generationTimings.length}</strong>
            </div>
            <div className="workflow-gates" aria-label="生成耗时聚合">
              <div className="workflow-gate-row"><span>成功率（候选状态）</span><strong className={`workflow-gate-status workflow-gate-status-${generationSuccessRateClass}`}>{generationSuccessRateText}</strong></div>
              <div className="workflow-gate-row"><span>平均耗时</span><strong className={`workflow-gate-status ${averageGenerationSeconds === null ? 'workflow-gate-status-not-run' : 'workflow-gate-status-passed'}`}>{averageGenerationText}</strong></div>
              <div className="workflow-gate-row"><span>异常计数</span><strong className={`workflow-gate-status ${generationAnomalyCount === 0 ? 'workflow-gate-status-passed' : 'workflow-gate-status-failed'}`}>{generationAnomalyCount}</strong></div>
            </div>
          </div>
          {generationTimings.length === 0 ? <p className="panel-copy">当前无候选可展示任务耗时。</p> : (
                <div className="workflow-gates" aria-label="按任务ID排序的生成耗时">
                {generationTimings.map((timing) => (
                  <button type="button" className={`workflow-gate-row workflow-gate-row-button ${timing.candidateId === candidateId ? 'workflow-gate-row-current' : ''} ${timing.anomaly ? 'workflow-gate-row-anomaly' : timing.statusClass === 'failed' ? 'workflow-gate-row-failed' : ''}`} aria-pressed={timing.candidateId === candidateId} key={timing.candidateId} onClick={() => setSelectedCandidateId(timing.candidateId)}>
                    <span><span className={`status-icon ${timing.anomaly || timing.statusClass === 'failed' ? 'status-icon-error' : timing.statusClass === 'passed' ? 'status-icon-pass' : 'status-icon-muted'}`}>{timing.anomaly || timing.statusClass === 'failed' ? '!' : timing.statusClass === 'passed' ? '✓' : '·'}</span>任务 {timing.candidateId} · {formatCandidateState(timing.state)}</span>
                    <div>
                    <code className={`workflow-gate-status ${timing.statusClass === 'failed' ? 'workflow-gate-status-failed' : timing.statusClass === 'passed' ? 'workflow-gate-status-passed' : 'workflow-gate-status-not-run'}`}>
                        创建 {timing.createdAtText ?? '时间缺失'}
                      </code>
                      <strong className={`workflow-gate-status workflow-gate-status-${timing.statusClass}`}>
                        {timing.statusClass === 'passed'
                          ? timing.elapsedDisplay
                          : timing.statusLabel}
                      </strong>
                      <span>
                        {`GLB:${timing.artifactReady ? '已就绪' : '未就绪'} · 比对:${timing.compareReady ? '已就绪' : '未就绪'} · 质量:${timing.qualityReady ? '已就绪' : '未就绪'} · 质量门:${timing.qualityGate}`}
                      </span>
                    </div>
                  </button>
                ))}
              </div>
            )}
          {generationAnomalyCount > 0 ? <p className="workflow-note"><span className="status-icon status-icon-error">!</span> 异常提示：存在时间缺失或异常记录，已用图标、文本和边框共同标识。请确认 Runtime 候选时间戳和系统时钟一致性。</p> : <p className="workflow-note"><span className="status-icon status-icon-pass">✓</span> 时间字段正常，当前无异常任务。</p>}
          <p className="workflow-note">点击任一任务可把 GLB、参考比较、质量门和本面板绑定到同一候选；当前耗时为创建时间到最后更新时间/当前时刻的 Viewer 估算。</p>
        </section>
        <section className="panel-section" aria-labelledby="agentic-stage-console-title">
          <p className="section-kicker">阶段现场控制台</p>
          <h2 id="agentic-stage-console-title">阶段现场</h2>
          <div className="workflow-summary" data-stage={agenticProjection.stage.id ?? 'unavailable'}>
            <div className="workflow-current">
              <span>当前阶段 · Runtime</span>
              <strong>{agenticProjection.stage.label ?? '未可用'} · {AGENTIC_STATUS_LABELS[agenticProjection.stage.status]}</strong>
            </div>
            <div className="workflow-gate-row">
              <span>Agentic 投影</span>
              <strong className="workflow-gate-status workflow-gate-status-not-run">{agenticProjection.status === 'ready' ? '已就绪' : '未可用'}</strong>
            </div>
            <div className="workflow-gate-row">
              <span>证据来源</span>
              <strong>{agenticProjection.status === 'ready' ? 'Agentic 投影' : visualEvidenceBound ? '候选绑定对比证据' : '未可用'}</strong>
            </div>
            <div className="workflow-gate-row">
              <span>Runtime 选中部件</span>
              <strong>{agenticProjection.selectedPartId ?? '未可用'}</strong>
            </div>
            <div className="workflow-gate-row">
              <span>当前视图选中</span>
              <strong>{selectedPartId === 'all' ? '全部部件' : selectedPartId}</strong>
            </div>
            <div className="workflow-gates" aria-label="Runtime Agentic stage gates">
              {agenticProjection.gates.map((gate) => <div className="workflow-gate-row" key={gate.id}>
                <span>{gate.label}</span>
                <strong className={`workflow-gate-status workflow-gate-status-${agenticGateStatusClass(gate.status)}`}>{AGENTIC_STATUS_LABELS[gate.status]}</strong>
              </div>)}
            </div>
            <div className="correction-metrics" aria-label="Runtime Agentic failed metrics">
              <span>失败指标：</span>
              {agenticProjection.failedMetrics.length > 0
                ? agenticProjection.failedMetrics.map((metric) => <code key={`${metric.name}-${metric.observed ?? 'unknown'}`}>{formatAgenticMetric(metric)}</code>)
                : <code>{agenticProjection.status === 'ready' ? '无记录' : '未可用'}</code>}
            </div>
            <div className="workflow-gates" aria-label="Runtime next allowed actions">
              {agenticProjection.nextAllowedActions.length > 0
                ? agenticProjection.nextAllowedActions.map((action) => {
                    const statusClass = action.status === 'allowed' ? 'passed' : action.status === 'locked' ? 'locked' : 'not-run'
                    const statusLabel = action.status === 'allowed' ? '允许' : action.status === 'locked' ? '锁定' : '未可用'
                    return <div className="workflow-gate-row" key={action.actionId}><span>{action.label}</span><strong className={`workflow-gate-status workflow-gate-status-${statusClass}`}>{statusLabel}</strong></div>
                  })
                : <div className="workflow-gate-row"><span>下一步允许动作</span><strong className="workflow-gate-status workflow-gate-status-not-run">未可用</strong></div>}
            </div>
            <div className="correction-metrics" aria-label="Runtime locked actions">
              <span>锁定动作：</span>
              {agenticProjection.lockedActions.length > 0
                ? agenticProjection.lockedActions.map((action) => <code key={action.actionId}>{action.label}</code>)
                : <code>未可用</code>}
            </div>
            <div className="quality-summary" aria-label="Runtime Agentic evidence hashes">
              {([
                ['artifact', stageEvidenceHashes.artifactSha256],
                ['reference', stageEvidenceHashes.referenceSha256],
                ['渲染集', stageEvidenceHashes.renderSetHash],
                ['比对报告', stageEvidenceHashes.comparisonReportHash],
                ['质量报告', stageEvidenceHashes.qualityReportHash],
              ] as const).map(([label, hash]) => <div key={label}><span>{label} 哈希</span><strong><code style={{ overflowWrap: 'anywhere' }}>{hash ?? '未可用'}</code></strong></div>)}
            </div>
            <p className="workflow-note">来源：Runtime authenticated read-only projection · {agenticProjection.reason ?? '无额外原因'}。Viewer 不调用写工具；未知/未可用不等于视觉通过。</p>
          </div>
        </section>
        <section className="panel-section" aria-labelledby="agentic-session-console-title">
          <p className="section-kicker">设计会话 / 里程碑</p>
          <h2 id="agentic-session-console-title">设计阶段回读</h2>
          <div className="workflow-summary" data-session-status={agenticSession.status} data-binding-status={agenticSession.bindingStatus}>
            <div className="workflow-current">
              <span>DesignSession · Runtime 回读</span>
              <strong>{agenticSession.status === 'ready' ? (agenticSession.sessionId ?? '会话 ID 未知') : '未可用'}</strong>
            </div>
            <div className="workflow-gate-row">
              <span>当前阶段</span>
              <strong>{agenticSession.stage.label ?? '未知'} · {AGENTIC_STATUS_LABELS[agenticSession.stage.status]}</strong>
            </div>
            <div className="workflow-gate-row">
              <span>会话绑定</span>
              <strong className={`workflow-gate-status workflow-gate-status-${agenticSessionStatusClass(agenticSession.bindingStatus)}`}>{agenticBindingStatusLabel(agenticSession.bindingStatus)}</strong>
            </div>
            <div className="workflow-gate-row">
              <span>持久化会话</span>
              <strong className={`workflow-gate-status workflow-gate-status-${agenticSessionStatusClass(agenticSession.durable === true ? 'persisted' : agenticSession.durable === false ? 'locked' : 'unknown')}`}>{agenticSession.durable === true ? '已持久化' : agenticSession.durable === false ? '只读投影 · 未持久化' : '未知'}</strong>
            </div>
            <div className="workflow-gates" aria-label="DesignSession 检查点状态">
              <div className="workflow-gate-row">
                <span>检查点</span>
                <strong className={`workflow-gate-status workflow-gate-status-${agenticSessionStatusClass(agenticSession.checkpoint.status)}`}>{AGENTIC_CHECKPOINT_STATUS_LABELS[agenticSession.checkpoint.status]}</strong>
              </div>
              <div className="workflow-gate-row">
                <span>检查点持久化</span>
                <strong>{agenticSession.checkpoint.durable === true ? '已持久化' : agenticSession.checkpoint.durable === false ? '未持久化' : '未知'}</strong>
              </div>
              <div className="workflow-gate-row">
                <span>恢复版本</span>
                <strong className={`workflow-gate-status workflow-gate-status-${agenticSessionStatusClass(agenticSession.restore.status)}`}>{AGENTIC_RESTORE_STATUS_LABELS[agenticSession.restore.status]}</strong>
              </div>
              <div className="workflow-gate-row">
                <span>恢复准备 / 批准</span>
                <strong>{AGENTIC_RESTORE_PREPARE_STATUS_LABELS[agenticSession.restore.prepareStatus]} / {AGENTIC_RESTORE_APPROVAL_STATUS_LABELS[agenticSession.restore.approvalStatus]}</strong>
              </div>
            </div>
            <p className="workflow-note">恢复版本仅显示准备/批准状态；Viewer 不提供确认、导出、恢复或绕过用户批准的动作。</p>
            <div className="correction-metrics" aria-label="DesignSession observed facts">
              <span>已观察：</span>
              {agenticSession.uncertainty.observed.length > 0 ? agenticSession.uncertainty.observed.map((item) => <code key={`observed-${item}`}>{item}</code>) : <code>无</code>}
            </div>
            <div className="correction-metrics" aria-label="DesignSession inferred facts">
              <span>推断：</span>
              {agenticSession.uncertainty.inferred.length > 0 ? agenticSession.uncertainty.inferred.map((item) => <code key={`inferred-${item}`}>{item}</code>) : <code>无</code>}
            </div>
            <div className="correction-metrics" aria-label="DesignSession unknown facts">
              <span>未知：</span>
              {agenticSession.uncertainty.unknown.length > 0 ? agenticSession.uncertainty.unknown.map((item) => <code key={`unknown-${item}`}>{item}</code>) : <code>无</code>}
            </div>
            <div className="workflow-gates" aria-label="DesignSession failed gates">
              {agenticSession.failedGates.map((gate) => <div className="workflow-gate-row" key={gate.id}>
                <span>{gate.label}</span>
                <strong className={`workflow-gate-status workflow-gate-status-${agenticGateStatusClass(gate.status)}`}>{AGENTIC_STATUS_LABELS[gate.status]}</strong>
              </div>)}
            </div>
            <div className="workflow-gates" aria-label="DesignSession 允许动作">
              <div className="workflow-gate-row"><span>允许动作 · 仅展示</span><strong>{agenticSession.allowedActions.length > 0 ? 'Runtime 允许' : '无 / 未知'}</strong></div>
              {agenticSession.allowedActions.map((action) => <div className="workflow-gate-row" key={`allowed-${action.actionId}`}>
                <span>{action.label}</span>
                <strong className="workflow-gate-status workflow-gate-status-passed">允许显示</strong>
              </div>)}
            </div>
            <div className="correction-metrics" aria-label="DesignSession locked actions">
              <span>锁定动作：</span>
              {agenticSession.lockedActions.map((action) => <code key={`locked-${action.actionId}`}>{action.label}</code>)}
            </div>
            <div className="quality-summary" aria-label="DesignSession evidence hash binding">
              {agenticSession.evidenceBindings.map((binding) => <div key={binding.id}>
                <span>{binding.label} hash · {agenticBindingStatusLabel(binding.status)}</span>
                <strong><code style={{ overflowWrap: 'anywhere' }}>{binding.actual ?? '未知'}</code></strong>
              </div>)}
            </div>
            <p className="workflow-note">来源：Runtime authenticated read-only `design-session-checkpoint` readback · {agenticSession.reason ?? '无额外原因'}。只显示已观察/推断/未知，不从 Viewer 推断质量或设计事实。</p>
          </div>
        </section>
        <section className="panel-section"><p className="section-kicker">质量证据</p><div className="status-legend" aria-label="状态图例"><span><span className="status-icon status-icon-pass">✓</span>通过</span><span><span className="status-icon status-icon-info">~</span>部分通过</span><span><span className="status-icon status-icon-error">!</span>未通过/异常</span><span><span className="status-icon status-icon-muted">·</span>未运行</span><span><span className="status-icon status-icon-muted">○</span>未绑定/未知</span></div><div className="quality-summary"><div><span>可见性状态</span><strong>{visualStatusLabel} <code>{visualStatus}</code></strong></div><div><span>可见性门</span><strong><span className={`status-icon ${visualGateStatusClass === 'passed' ? 'status-icon-pass' : visualGateStatusClass === 'failed' ? 'status-icon-error' : 'status-icon-muted'}`}>{visualGateStatusClass === 'passed' ? '✓' : visualGateStatusClass === 'failed' ? '!' : '·'}</span>{visualGateLabel}</strong></div><div><span>门来源</span><strong>{visualGateSource}</strong></div>{metricLabels.map(([key, label]) => <div key={key}><span>{label}</span><strong>{typeof comparisonMetrics[key] === 'number' ? comparisonMetrics[key].toFixed(3) : '—'}</strong></div>)}</div></section>
        <section className="panel-section" aria-labelledby="runtime-quality-workflow-title"><p className="section-kicker">Runtime 质量流程</p><h2 id="runtime-quality-workflow-title">Runtime 权威质量门</h2><div className="workflow-summary" data-stage={agenticProjection.stage.id ?? 'unavailable'}><div className="workflow-current"><span>当前阶段 · Runtime</span><strong>{agenticProjection.stage.label ?? '未可用'} · {AGENTIC_STATUS_LABELS[agenticProjection.stage.status]}</strong></div><div className="workflow-gates" aria-label="Runtime quality gates">{agenticProjection.gates.map((gate) => <div className="workflow-gate-row" key={gate.id}><span>{gate.label}</span><strong className={`workflow-gate-status workflow-gate-status-${agenticGateStatusClass(gate.status)}`}>{AGENTIC_STATUS_LABELS[gate.status]}</strong></div>)}</div><p className="workflow-note">门状态、阈值和失败指标只从 Runtime authenticated read-only projection / QualityReport 读取；Viewer 不再从 comparison metrics 重新计算质量门。</p></div></section>
        <section className="panel-section" aria-labelledby="runtime-next-action-title"><p className="section-kicker">Runtime 下一步</p><h2 id="runtime-next-action-title">Runtime 返回的下一步</h2><div className="correction-queue" aria-label="Runtime next actions">{agenticProjection.nextAllowedActions.length > 0 ? agenticProjection.nextAllowedActions.map((action) => <article className="correction-card" key={action.actionId}><div className="correction-card-header"><strong>{action.label}</strong><span>{agenticProjection.stage.label ?? '未知阶段'}</span></div><p>{action.reason ?? '由 Runtime projection 返回的 bounded action；Viewer 仅展示，不执行。'}</p><div className="correction-metrics"><code className={`workflow-gate-status workflow-gate-status-${action.status === 'allowed' ? 'passed' : action.status === 'locked' ? 'locked' : 'not-run'}`}>{action.status === 'allowed' ? '允许' : action.status === 'locked' ? '锁定' : '未可用'}</code></div></article>) : <p className="panel-copy">当前没有 Runtime 返回的可安全动作；等待候选绑定证据或真人评审。</p>}</div><p className="workflow-note">这是 Runtime 的只读 action projection，不直接调用写工具，也不替代用户批准。</p></section>
        <section className="panel-section panel-note"><p className="section-kicker">MVP 状态</p><p className="panel-copy">Viewer 通过受保护的本地 IPC 读取 Runtime 的候选、GLB 数据、版本和当前快照；Three.js 只创建临时 canvas scene，不写数据库、不改变 Runtime artifact。固定渲染证据和 PBR metadata 与候选哈希绑定。</p></section>
        </div>
      </aside>
    </section>
    <section className="runtime-bottom-rail" aria-label="History Versions 与 Codex Activity">
      <section className="runtime-history-panel" aria-labelledby="runtime-history-title">
        <div className="runtime-rail-header">
          <div>
            <span className="runtime-workbench-panel-eyebrow">HISTORY / VERSIONS</span>
            <h2 id="runtime-history-title">History / Versions</h2>
          </div>
          <span className="runtime-rail-meta">{versionCount} 个版本 · {candidateSnapshots.length} 个候选</span>
        </div>
        <div className="runtime-version-strip">
          {candidateSnapshots.length === 0 ? <p className="runtime-rail-empty">暂无候选版本；等待 Codex 通过 MCP 提交。</p> : candidateSnapshots.slice(0, 8).map((snapshot) => {
            const snapshotIsCurrent = snapshot.candidateId === candidateId
            const snapshotIsComparison = snapshot.candidateId === comparisonSnapshot?.candidateId
            const snapshotTiming = generationTimingByCandidateId.get(snapshot.candidateId)
            const snapshotBinding = buildCandidateSnapshotBindingState(snapshot)
            const snapshotTimingLabel = snapshotTiming?.elapsedDisplay ?? snapshotTiming?.statusLabel ?? '耗时未运行'
            const snapshotChainLabel = `GLB ${snapshotBinding.artifact ? '✓' : '—'} · 对比 ${snapshotBinding.comparison ? '✓' : '—'} · 质量 ${snapshotBinding.qualityReport ? '✓' : '—'}`
            const snapshotRole = snapshotIsCurrent ? '当前候选' : snapshotIsComparison ? '对比候选' : '历史候选'
            const snapshotTimingAnomaly = snapshotTiming?.anomaly === true
            return (
              <button
                type="button"
                key={snapshot.candidateId}
                className={`runtime-version-card ${snapshotIsCurrent ? 'runtime-version-card-current' : ''} ${snapshotIsComparison ? 'runtime-version-card-comparison' : ''} ${snapshotTimingAnomaly ? 'runtime-version-card-anomaly' : ''}`}
                onClick={() => setSelectedCandidateId(snapshot.candidateId)}
                aria-pressed={snapshotIsCurrent}
                aria-current={snapshotIsCurrent ? 'true' : undefined}
                aria-label={`${snapshot.candidateId}，${snapshotRole}，${formatCandidateState(snapshot.candidateState)}，${snapshotTimingLabel}${snapshotTimingAnomaly ? '，时间异常' : ''}`}
              >
                <span className="runtime-version-card-id">{snapshot.candidateId}<b className={`runtime-version-card-badge ${snapshotIsCurrent ? 'runtime-version-card-badge-current' : snapshotIsComparison ? 'runtime-version-card-badge-comparison' : ''}`}>{snapshotRole}</b></span>
                <strong>{formatCandidateState(snapshot.candidateState)}</strong>
                <small className={snapshotTimingAnomaly ? 'runtime-version-card-timing-anomaly' : ''}>{snapshotTimingAnomaly ? '⚠ ' : ''}{snapshotTimingLabel} · {snapshot.triangleCount} tris</small>
                <small className="runtime-version-card-chain">{snapshotChainLabel}</small>
              </button>
            )
          })}
        </div>
      </section>
      <section className="runtime-activity-panel" aria-labelledby="runtime-activity-title">
        <div className="runtime-rail-header">
          <div>
            <span className="runtime-workbench-panel-eyebrow">CODEX ACTIVITY</span>
            <h2 id="runtime-activity-title">Codex Activity</h2>
          </div>
          <span className="runtime-rail-meta">只读回读</span>
        </div>
        <div className="runtime-activity-list">
          <div className="runtime-activity-row"><span className={`activity-state ${ready ? 'activity-state-active' : 'activity-state-warn'}`}>●</span><span className="activity-label">Codex / Runtime</span><strong>{ready ? '已连接' : '未连接'}</strong></div>
          <div className="runtime-activity-row"><span className={`activity-state ${candidateId ? 'activity-state-active' : 'activity-state-idle'}`}>◇</span><span className="activity-label">当前候选</span><strong>{candidateId ?? '等待候选'}</strong></div>
          <div className="runtime-activity-row"><span className={`activity-state ${artifactLoadState === 'error' ? 'activity-state-warn' : artifactLoadState === 'ready' ? 'activity-state-active' : 'activity-state-idle'}`}>◆</span><span className="activity-label">GLB / 参考对比</span><strong>{artifactLoadState === 'ready' ? 'GLB 已就绪' : artifactLoadState === 'error' ? 'GLB 异常' : compareLoadState === 'ready' ? '对比已就绪' : '等待回读'}</strong></div>
          <div className="runtime-activity-row"><span className={`activity-state ${visualGateStatusClass === 'failed' ? 'activity-state-warn' : visualGateStatusClass === 'passed' ? 'activity-state-active' : 'activity-state-idle'}`}>✓</span><span className="activity-label">Runtime 质量门</span><strong>{visualGateLabel}</strong></div>
        </div>
      </section>
    </section>
  </main>
}
