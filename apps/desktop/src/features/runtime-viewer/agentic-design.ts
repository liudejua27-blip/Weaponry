export type AgenticGateStatus = 'passed' | 'failed' | 'unknown' | 'locked'
export type AgenticActionStatus = 'allowed' | 'locked' | 'unavailable'

export type AgenticMetric = {
  name: string
  observed: string | null
  threshold: string | null
  evidenceHash: string | null
}

export type AgenticGate = {
  id: string
  label: string
  status: AgenticGateStatus
  failedMetrics: AgenticMetric[]
  reason: string | null
}

export type AgenticStage = {
  id: string | null
  label: string | null
  status: AgenticGateStatus
  reason: string | null
}

export type AgenticAction = {
  actionId: string
  label: string
  status: AgenticActionStatus
  reason: string | null
}

export type AgenticEvidenceHashes = {
  artifactSha256: string | null
  referenceSha256: string | null
  renderSetHash: string | null
  comparisonReportHash: string | null
  qualityReportHash: string | null
}

export type AgenticDesignProjection = {
  status: 'ready' | 'unavailable'
  source: 'runtime-authenticated-read-only'
  code: string | null
  reason: string | null
  projectId: string | null
  candidateId: string | null
  stage: AgenticStage
  gates: AgenticGate[]
  failedMetrics: AgenticMetric[]
  selectedPartId: string | null
  nextAllowedActions: AgenticAction[]
  lockedActions: AgenticAction[]
  evidenceHashes: AgenticEvidenceHashes
}

export type AgenticProjectionBinding = {
  projectId?: string
  candidateId?: string
  artifactSha256?: string
  referenceSha256?: string
  renderSetHash?: string
  comparisonReportHash?: string
  qualityReportHash?: string
  visualEvidenceBound: boolean
}

type JsonObject = Record<string, unknown>

export const AGENTIC_STAGE_LABELS: Record<string, string> = {
  'reference-canvas': '参考画布',
  'primary-form': '主形体',
  'secondary-structure': '次级结构',
  'tertiary-detail': '三级细节',
  'uv-pbr': 'UV / PBR',
  'final-review': '最终复核',
}

export const AGENTIC_GATE_DEFINITIONS: Array<{ id: string; label: string }> = [
  { id: 'reference-canvas', label: '参考画布' },
  { id: 'primary-form', label: '主形体' },
  { id: 'secondary-structure', label: '次级结构' },
  { id: 'tertiary-detail', label: '三级细节' },
  { id: 'uv-pbr', label: 'UV / PBR' },
  { id: 'final-review', label: '最终复核' },
]

export const AGENTIC_STATUS_LABELS: Record<AgenticGateStatus, string> = {
  passed: '通过',
  failed: '未通过',
  unknown: '未知',
  locked: '锁定',
}

export function agenticGateStatusClass(status: AgenticGateStatus): string {
  return status === 'unknown' ? 'not-run' : status
}

function asObject(value: unknown): JsonObject | null {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? value as JsonObject
    : null
}

function nonEmptyString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0 ? value.trim() : null
}

function scalarLabel(value: unknown): string | null {
  const text = nonEmptyString(value)
  if (text) return text
  if (typeof value === 'number' && Number.isFinite(value)) return String(value)
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  return null
}

function projectionStatus(value: unknown): 'ready' | 'unavailable' {
  return typeof value === 'string' && value.toLowerCase() === 'ready' ? 'ready' : 'unavailable'
}

function gateStatus(value: unknown): AgenticGateStatus {
  if (typeof value !== 'string') return 'unknown'
  switch (value.toLowerCase()) {
    case 'passed':
    case 'pass':
      return 'passed'
    case 'failed':
    case 'fail':
    case 'blocked':
    case 'action-required':
      return 'failed'
    case 'locked':
      return 'locked'
    case 'unknown':
    case 'not-run':
    case 'not_run':
    case 'unavailable':
      return 'unknown'
    default:
      return 'unknown'
  }
}

function actionStatus(value: unknown): AgenticActionStatus {
  if (typeof value !== 'string') return 'unavailable'
  switch (value.toLowerCase()) {
    case 'allowed':
    case 'ready':
      return 'allowed'
    case 'locked':
      return 'locked'
    default:
      return 'unavailable'
  }
}

function metric(value: unknown): AgenticMetric | null {
  const record = asObject(value)
  const name = typeof value === 'string'
    ? nonEmptyString(value)
    : nonEmptyString(record?.metric_name) ?? nonEmptyString(record?.name)
  if (!name) return null
  return {
    name,
    observed: scalarLabel(record?.observed ?? record?.value),
    threshold: scalarLabel(record?.threshold),
    evidenceHash: nonEmptyString(record?.evidence_hash ?? record?.evidenceHash),
  }
}

function metrics(value: unknown): AgenticMetric[] {
  if (!Array.isArray(value)) return []
  return value.map(metric).filter((item): item is AgenticMetric => item !== null).slice(0, 32)
}

function action(value: unknown, fallbackStatus: AgenticActionStatus = 'unavailable'): AgenticAction | null {
  const record = asObject(value)
  const actionId = typeof value === 'string'
    ? nonEmptyString(value)
    : nonEmptyString(record?.action_id) ?? nonEmptyString(record?.id)
  if (!actionId) return null
  return {
    actionId,
    label: nonEmptyString(record?.label) ?? nonEmptyString(record?.title) ?? actionId,
    status: actionStatus(record?.status ?? fallbackStatus),
    reason: nonEmptyString(record?.reason),
  }
}

function actions(value: unknown, fallbackStatus: AgenticActionStatus = 'unavailable'): AgenticAction[] {
  if (!Array.isArray(value)) return []
  return value.map((entry) => action(entry, fallbackStatus)).filter((item): item is AgenticAction => item !== null).slice(0, 16)
}

function evidenceHashes(value: unknown): AgenticEvidenceHashes {
  const record = asObject(value)
  const evidence = asObject(record?.evidence_hashes ?? record?.evidence)
  const lineage = asObject(record?.lineage)
  const visualBundle = asObject(record?.visual_evidence_bundle ?? record?.visual_evidence)
  const visualHashes = asObject(visualBundle?.hashes)
  const quality = asObject(record?.quality)
  const containers = [evidence, record, lineage, visualHashes, quality]
  const read = (...keys: string[]): string | null => {
    for (const container of containers) {
      for (const key of keys) {
        const value = nonEmptyString(container?.[key])
        if (value) return value
      }
    }
    return null
  }
  return {
    artifactSha256: read('artifact_sha256', 'artifactSha256'),
    referenceSha256: read('reference_sha256', 'referenceSha256'),
    renderSetHash: read('render_set_hash', 'renderSetHash'),
    comparisonReportHash: read('comparison_report_hash', 'comparisonReportHash'),
    qualityReportHash: read('quality_report_hash', 'qualityReportHash'),
  }
}

function evidenceMatchesBinding(hashes: AgenticEvidenceHashes, binding: AgenticProjectionBinding): boolean {
  const pairs: Array<[string | null, string | undefined]> = [
    [hashes.artifactSha256, binding.artifactSha256],
    [hashes.referenceSha256, binding.referenceSha256],
    [hashes.renderSetHash, binding.renderSetHash],
    [hashes.comparisonReportHash, binding.comparisonReportHash],
    [hashes.qualityReportHash, binding.qualityReportHash],
  ]
  return pairs.every(([actual, expected]) => actual === null || Boolean(expected && actual === expected))
}

function gateEntries(value: unknown): Array<{ id: string; value: unknown }> {
  if (Array.isArray(value)) {
    return value.map((entry) => {
      const record = asObject(entry)
      return {
        id: nonEmptyString(record?.gate_id) ?? nonEmptyString(record?.id) ?? '',
        value: entry,
      }
    }).filter((entry) => entry.id.length > 0).slice(0, 16)
  }
  const record = asObject(value)
  if (!record) return []
  return Object.entries(record).map(([id, entry]) => ({ id, value: entry })).slice(0, 16)
}

function normalizeGates(value: unknown): AgenticGate[] {
  const entries = gateEntries(value)
  if (entries.length === 0) {
    return AGENTIC_GATE_DEFINITIONS.map((definition) => ({
      ...definition,
      status: 'unknown' as const,
      failedMetrics: [],
      reason: 'Runtime gate data unavailable',
    }))
  }
  return entries.map(({ id, value }) => {
    const record = asObject(value)
    return {
      id,
      label: nonEmptyString(record?.label) ?? AGENTIC_STAGE_LABELS[id] ?? id,
      status: gateStatus(record?.status ?? value),
      failedMetrics: metrics(record?.failed_metrics ?? record?.failedMetrics),
      reason: nonEmptyString(record?.reason),
    }
  })
}

function criticFailedMetrics(value: unknown): AgenticMetric[] {
  const record = asObject(value)
  const issues = Array.isArray(record?.issues) ? record.issues : []
  return issues
    .filter((issue) => {
      const status = asObject(issue)?.status
      return typeof status === 'string' && (status === 'fail' || status === 'failed')
    })
    .map(metric)
    .filter((item): item is AgenticMetric => item !== null)
    .slice(0, 32)
}

function normalizeProjectionGates(
  record: JsonObject,
  stagePlan: JsonObject | null,
  critic: JsonObject | null,
): AgenticGate[] {
  if (gateEntries(record.gates).length > 0) return normalizeGates(record.gates)
  const strictVisualGate = asObject(stagePlan?.strict_visual_gate)
  const qualityGate = asObject(stagePlan?.quality_gate)
  const failedMetrics = criticFailedMetrics(critic)
  const gates: AgenticGate[] = []
  if (strictVisualGate) {
    gates.push({
      id: 'strict-visual-gate',
      label: '严格视觉门',
      status: gateStatus(strictVisualGate.status),
      failedMetrics,
      reason: nonEmptyString(strictVisualGate.reason),
    })
  }
  if (qualityGate) {
    gates.push({
      id: 'structural-gate',
      label: '结构门（非视觉）',
      status: gateStatus(qualityGate.structural_status),
      failedMetrics: [],
      reason: '结构门不等于视觉 PASS',
    })
  }
  return gates.length > 0 ? gates : normalizeGates(undefined)
}

function normalizeStage(value: unknown): AgenticStage {
  const record = asObject(value)
  const id = typeof value === 'string'
    ? nonEmptyString(value)
    : nonEmptyString(record?.stage_id) ?? nonEmptyString(record?.id)
  return {
    id,
    label: nonEmptyString(record?.label) ?? (id ? AGENTIC_STAGE_LABELS[id] ?? id : null),
    status: gateStatus(record?.status),
    reason: nonEmptyString(record?.reason),
  }
}

function unavailableReason(payload: JsonObject | null, fallback: string): string {
  return nonEmptyString(payload?.code) ?? nonEmptyString(payload?.reason) ?? fallback
}

const EMPTY_EVIDENCE_HASHES: AgenticEvidenceHashes = {
  artifactSha256: null,
  referenceSha256: null,
  renderSetHash: null,
  comparisonReportHash: null,
  qualityReportHash: null,
}

const LOCKED_WHEN_UNAVAILABLE: AgenticAction[] = [
  { actionId: 'repair', label: '局部修正', status: 'locked', reason: 'Agentic projection unavailable' },
  { actionId: 'confirm', label: '确认版本', status: 'locked', reason: 'Agentic projection unavailable' },
  { actionId: 'export', label: '导出', status: 'locked', reason: 'Agentic projection unavailable' },
]

export function unavailableAgenticDesignProjection(
  projectId?: string,
  candidateId?: string,
  reason = 'AGENTIC_PROJECTION_UNAVAILABLE',
): AgenticDesignProjection {
  return {
    status: 'unavailable',
    source: 'runtime-authenticated-read-only',
    code: reason,
    reason,
    projectId: projectId ?? null,
    candidateId: candidateId ?? null,
    stage: { id: null, label: null, status: 'unknown', reason },
    gates: AGENTIC_GATE_DEFINITIONS.map((definition) => ({
      ...definition,
      status: 'unknown' as const,
      failedMetrics: [],
      reason,
    })),
    failedMetrics: [],
    selectedPartId: null,
    nextAllowedActions: [],
    lockedActions: LOCKED_WHEN_UNAVAILABLE.map((item) => ({ ...item, reason })),
    evidenceHashes: { ...EMPTY_EVIDENCE_HASHES },
  }
}

export function normalizeAgenticDesignProjection(
  payload: unknown,
  binding: AgenticProjectionBinding,
): AgenticDesignProjection {
  const record = asObject(payload)
  const projectId = nonEmptyString(record?.project_id)
  const candidateId = nonEmptyString(record?.candidate_id)
  const expectedProjectId = nonEmptyString(binding.projectId)
  const expectedCandidateId = nonEmptyString(binding.candidateId)
  const runtimeProjectionReady = record?.projection_status === 'projection/read-only' && record.read_only === true
  if (!record || (projectionStatus(record.status) !== 'ready' && !runtimeProjectionReady)) {
    return unavailableAgenticDesignProjection(expectedProjectId ?? undefined, expectedCandidateId ?? undefined, unavailableReason(record, 'AGENTIC_PROJECTION_UNAVAILABLE'))
  }
  if (!expectedProjectId || !expectedCandidateId || projectId !== expectedProjectId || candidateId !== expectedCandidateId) {
    return unavailableAgenticDesignProjection(expectedProjectId ?? undefined, expectedCandidateId ?? undefined, 'AGENTIC_PROJECTION_BINDING_MISMATCH')
  }
  const visualEvidenceBundle = asObject(record.visual_evidence_bundle ?? record.visual_evidence)
  if (visualEvidenceBundle?.available === true && !binding.visualEvidenceBound) {
    return unavailableAgenticDesignProjection(expectedProjectId ?? undefined, expectedCandidateId ?? undefined, 'AGENTIC_VISUAL_EVIDENCE_UNBOUND')
  }
  const hashes = evidenceHashes(record)
  if (!evidenceMatchesBinding(hashes, binding)) {
    return unavailableAgenticDesignProjection(expectedProjectId ?? undefined, expectedCandidateId ?? undefined, 'AGENTIC_EVIDENCE_BINDING_MISMATCH')
  }
  const stagePlan = asObject(record.design_stage_plan)
  const critic = asObject(record.design_critic_report)
  const stagePlanStage = stagePlan
    ? {
        id: stagePlan.current_stage,
        label: stagePlan.current_stage,
        status: stagePlan.current_stage_status,
        reason: asObject(stagePlan.strict_visual_gate)?.reason,
      }
    : record.stage ?? record.current_stage
  const stage = normalizeStage(stagePlanStage)
  const gates = normalizeProjectionGates(record, stagePlan, critic)
  const gateMetrics = gates.flatMap((gate) => gate.failedMetrics)
  const failedMetrics = metrics(record.failed_metrics ?? record.failedMetrics)
  const criticMetrics = criticFailedMetrics(critic)
  const selectedPart = asObject(record.selected_part)
  const strictGate = asObject(stagePlan?.strict_visual_gate)
  return {
    status: 'ready',
    source: 'runtime-authenticated-read-only',
    code: null,
    reason: nonEmptyString(record.reason) ?? nonEmptyString(strictGate?.reason),
    projectId,
    candidateId,
    stage,
    gates,
    failedMetrics: failedMetrics.length > 0 ? failedMetrics : criticMetrics.length > 0 ? criticMetrics : gateMetrics,
    selectedPartId: nonEmptyString(record.selected_part_id) ?? nonEmptyString(selectedPart?.part_id),
    nextAllowedActions: actions(record.next_allowed_actions ?? record.nextAllowedActions ?? record.allowed_actions, 'allowed'),
    lockedActions: actions(record.locked_actions ?? record.lockedActions ?? record.blocked_actions, 'locked'),
    evidenceHashes: hashes,
  }
}
