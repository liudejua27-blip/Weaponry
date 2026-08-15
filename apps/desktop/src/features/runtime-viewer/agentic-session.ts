import {
  normalizeAgenticDesignProjection,
  type AgenticAction,
  type AgenticDesignProjection,
  type AgenticEvidenceHashes,
  type AgenticGate,
  type AgenticProjectionBinding,
  type AgenticStage,
} from './agentic-design'

type JsonObject = Record<string, unknown>

export type AgenticBindingStatus = 'bound' | 'unknown' | 'mismatch'
export type AgenticKnowledgeState = 'observed' | 'inferred' | 'unknown'
export type AgenticSessionStatus = 'ready' | 'unavailable'
export type AgenticCheckpointStatus = 'persisted' | 'prepare' | 'awaiting-approval' | 'approved' | 'unknown' | 'locked'
export type AgenticRestoreStatus = 'prepare' | 'awaiting-approval' | 'approved' | 'unknown' | 'locked'

export type AgenticSessionRevisionBinding = {
  snapshotId?: string
  snapshotRevision?: number
  snapshotManifestHash?: string
  candidateCanonicalSha256?: string
}

export type AgenticSessionBinding = AgenticProjectionBinding & {
  revision?: AgenticSessionRevisionBinding
}

export type AgenticCheckpointState = {
  status: AgenticCheckpointStatus
  rawStatus: string | null
  durable: boolean | null
  checkpointId: string | null
  revision: string | number | null
  reason: string | null
}

export type AgenticRestoreState = {
  status: AgenticRestoreStatus
  prepareStatus: 'prepared' | 'not-prepared' | 'unknown' | 'locked'
  approvalStatus: 'not-requested' | 'awaiting-approval' | 'approved' | 'unknown' | 'locked'
  rawStatus: string | null
  reason: string | null
  viewerAction: 'locked-read-only'
}

export type AgenticUncertainty = {
  observed: string[]
  inferred: string[]
  unknown: string[]
}

export type AgenticEvidenceBinding = {
  id: keyof AgenticEvidenceHashes
  label: string
  expected: string | null
  actual: string | null
  status: AgenticBindingStatus
}

export type AgenticSessionProjection = {
  status: AgenticSessionStatus
  source: 'runtime-authenticated-read-only'
  code: string | null
  reason: string | null
  projectId: string | null
  candidateId: string | null
  sessionId: string | null
  stage: AgenticStage
  durable: boolean | null
  checkpoint: AgenticCheckpointState
  restore: AgenticRestoreState
  uncertainty: AgenticUncertainty
  failedGates: AgenticGate[]
  allowedActions: AgenticAction[]
  lockedActions: AgenticAction[]
  evidenceHashes: AgenticEvidenceHashes
  evidenceBindings: AgenticEvidenceBinding[]
  bindingStatus: AgenticBindingStatus
  readOnly: true
}

const CHECKPOINT_STATUS_LABELS: Record<AgenticCheckpointStatus, string> = {
  persisted: '已持久化',
  prepare: '准备中',
  'awaiting-approval': '等待批准',
  approved: '已批准',
  unknown: '未知',
  locked: '锁定',
}

export const AGENTIC_CHECKPOINT_STATUS_LABELS = CHECKPOINT_STATUS_LABELS

export const AGENTIC_RESTORE_STATUS_LABELS: Record<AgenticRestoreStatus, string> = {
  prepare: '准备中',
  'awaiting-approval': '等待批准',
  approved: '已批准（仍需用户确认）',
  unknown: '未知',
  locked: '锁定',
}

export const AGENTIC_RESTORE_PREPARE_STATUS_LABELS: Record<AgenticRestoreState['prepareStatus'], string> = {
  prepared: '已准备',
  'not-prepared': '未准备',
  unknown: '未知',
  locked: '锁定',
}

export const AGENTIC_RESTORE_APPROVAL_STATUS_LABELS: Record<AgenticRestoreState['approvalStatus'], string> = {
  'not-requested': '未请求批准',
  'awaiting-approval': '等待批准',
  approved: '已批准',
  unknown: '未知',
  locked: '锁定',
}

const EVIDENCE_BINDINGS: Array<[keyof AgenticEvidenceHashes, string]> = [
  ['artifactSha256', 'artifact'],
  ['referenceSha256', 'reference'],
  ['renderSetHash', 'RenderSet'],
  ['comparisonReportHash', 'comparison'],
  ['qualityReportHash', 'QualityReport'],
]

const LOCKED_SESSION_ACTIONS: AgenticAction[] = [
  { actionId: 'checkpoint', label: '创建检查点', status: 'locked', reason: 'Viewer is read-only; checkpoint writes require Runtime approval flow' },
  { actionId: 'restore', label: '恢复版本', status: 'locked', reason: 'Viewer only displays prepare/approval state' },
  { actionId: 'candidate_confirm', label: '确认版本', status: 'locked', reason: 'Viewer cannot confirm or bypass user approval' },
  { actionId: 'export_confirm', label: '导出', status: 'locked', reason: 'Viewer cannot export or bypass user approval' },
]

const ACTION_LABELS: Record<string, string> = {
  read_reference_evidence: '读取参考证据',
  prepare_candidate: '准备候选版本',
  inspect_failed_gate: '检查失败门',
  repair_bounded_part_or_camera: '准备有界局部修正',
  rerun_readback_render_compare: '重新回读 / 渲染 / 比较',
  inspect_artifact: '检查 artifact',
  render_reference_comparison: '生成参考比较',
  evaluate_quality: '评估质量',
  advance_one_stage: '推进一个阶段',
  inspect_part_lineage: '检查部件 lineage',
  prepare_bounded_action: 'prepare 有界动作',
  pbr_prepare: '准备 PBR',
  candidate_confirm: '确认版本',
  export_confirm: '导出',
  restore: '恢复版本',
}

function asObject(value: unknown): JsonObject | null {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? value as JsonObject
    : null
}

function nonEmptyString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0 ? value.trim() : null
}

function scalar(value: unknown): string | number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value
  return nonEmptyString(value)
}

function uniqueStrings(values: string[]): string[] {
  return [...new Set(values.filter((value) => value.trim().length > 0))].slice(0, 64)
}

function readString(record: JsonObject | null, ...keys: string[]): string | null {
  for (const key of keys) {
    const value = nonEmptyString(record?.[key])
    if (value) return value
  }
  return null
}

function readNumber(record: JsonObject | null, ...keys: string[]): number | null {
  for (const key of keys) {
    const value = record?.[key]
    if (typeof value === 'number' && Number.isFinite(value)) return value
  }
  return null
}

function statusText(value: unknown): string | null {
  return nonEmptyString(value)?.toLowerCase().replaceAll('_', '-') ?? null
}

function checkpointStatus(value: unknown): AgenticCheckpointStatus {
  switch (statusText(value)) {
    case 'persisted':
    case 'available':
      return 'persisted'
    case 'prepare':
    case 'prepared':
      return 'prepare'
    case 'awaiting-approval':
    case 'approval-required':
      return 'awaiting-approval'
    case 'approved':
      return 'approved'
    case 'locked':
      return 'locked'
    default:
      return 'unknown'
  }
}

function checkpoint(value: unknown, fallbackReason: string): AgenticCheckpointState {
  const record = asObject(value)
  const rawStatus = statusText(record?.status ?? value)
  const status = checkpointStatus(rawStatus)
  const durable = typeof record?.durable === 'boolean'
    ? record.durable
    : status === 'persisted' || status === 'approved'
      ? true
      : status === 'unknown'
        ? null
        : false
  return {
    status,
    rawStatus,
    durable,
    checkpointId: readString(record, 'checkpoint_id', 'checkpointId', 'id'),
    revision: scalar(record?.revision ?? record?.snapshot_revision),
    reason: readString(record, 'reason') ?? (record ? null : fallbackReason),
  }
}

function restoreStatus(value: unknown): AgenticRestoreStatus {
  switch (statusText(value)) {
    case 'prepare':
    case 'prepared':
      return 'prepare'
    case 'awaiting-approval':
    case 'approval-required':
      return 'awaiting-approval'
    case 'approved':
      return 'approved'
    case 'locked':
      return 'locked'
    default:
      return 'unknown'
  }
}

function restore(value: unknown, fallbackReason: string): AgenticRestoreState {
  const record = asObject(value)
  const rawStatus = statusText(record?.status ?? value)
  const status = restoreStatus(rawStatus)
  return {
    status,
    prepareStatus: status === 'prepare' || status === 'awaiting-approval' || status === 'approved'
      ? 'prepared'
      : status === 'locked'
        ? 'locked'
        : rawStatus === 'not-persisted'
          ? 'not-prepared'
          : 'unknown',
    approvalStatus: status === 'awaiting-approval'
      ? 'awaiting-approval'
      : status === 'approved'
        ? 'approved'
        : status === 'locked'
          ? 'locked'
          : 'not-requested',
    rawStatus,
    reason: readString(record, 'reason') ?? (record ? null : fallbackReason),
    viewerAction: 'locked-read-only',
  }
}

function actions(value: unknown, fallbackStatus: 'allowed' | 'locked'): AgenticAction[] {
  if (!Array.isArray(value)) return []
  return value.map((entry): AgenticAction | null => {
    const record = asObject(entry)
    const actionId = typeof entry === 'string'
      ? nonEmptyString(entry)
      : readString(record, 'action_id', 'actionId', 'id')
    if (!actionId) return null
    const runtimeStatus = statusText(record?.status)
    const status = runtimeStatus === 'locked' || fallbackStatus === 'locked' ? 'locked' : 'allowed'
    return {
      actionId,
      label: readString(record, 'label', 'title') ?? ACTION_LABELS[actionId] ?? actionId,
      status,
      reason: readString(record, 'reason'),
    }
  }).filter((entry): entry is AgenticAction => entry !== null).slice(0, 32)
}

function mergeLockedActions(actionsToMerge: AgenticAction[], reason: string): AgenticAction[] {
  const merged = new Map<string, AgenticAction>()
  for (const action of [...actionsToMerge, ...LOCKED_SESSION_ACTIONS]) {
    merged.set(action.actionId, {
      ...action,
      status: 'locked',
      reason: action.reason ?? reason,
    })
  }
  return [...merged.values()]
}

function uncertainty(...values: unknown[]): AgenticUncertainty {
  const buckets: Record<AgenticKnowledgeState, string[]> = {
    observed: [],
    inferred: [],
    unknown: [],
  }
  for (const value of values) {
    const record = asObject(value)
    if (!record) continue
    for (const state of Object.keys(buckets) as AgenticKnowledgeState[]) {
      const entries = record[state]
      if (Array.isArray(entries)) {
        buckets[state].push(...entries.filter((entry): entry is string => typeof entry === 'string'))
      }
    }
  }
  if (buckets.observed.length === 0 && buckets.inferred.length === 0 && buckets.unknown.length === 0) {
    buckets.unknown.push('session_uncertainty_ledger')
  }
  return {
    observed: uniqueStrings(buckets.observed),
    inferred: uniqueStrings(buckets.inferred),
    unknown: uniqueStrings(buckets.unknown),
  }
}

function lineage(value: unknown): JsonObject | null {
  return asObject(value)
}

function lineageValue(lineages: JsonObject[], ...keys: string[]): string | number | null {
  for (const record of lineages) {
    for (const key of keys) {
      const value = scalar(record[key])
      if (value !== null) return value
    }
  }
  return null
}

function lineageString(lineages: JsonObject[], ...keys: string[]): string | null {
  const value = lineageValue(lineages, ...keys)
  return typeof value === 'string' ? value : null
}

function compareBinding(expected: string | number | undefined, actual: string | number | null): AgenticBindingStatus {
  if (expected === undefined) return 'unknown'
  if (actual === null) return 'unknown'
  return expected === actual ? 'bound' : 'mismatch'
}

function combineBindingStatus(statuses: AgenticBindingStatus[]): AgenticBindingStatus {
  if (statuses.some((status) => status === 'mismatch')) return 'mismatch'
  if (statuses.some((status) => status === 'unknown')) return 'unknown'
  return 'bound'
}

function evidenceBindings(
  actual: AgenticEvidenceHashes,
  binding: AgenticSessionBinding,
): AgenticEvidenceBinding[] {
  return EVIDENCE_BINDINGS.map(([id, label]) => {
    const expected = binding[id] ?? null
    const actualValue = actual[id]
    return {
      id,
      label,
      expected,
      actual: actualValue,
      status: compareBinding(expected ?? undefined, actualValue),
    }
  })
}

function unknownStage(reason: string): AgenticStage {
  return { id: null, label: null, status: 'unknown', reason }
}

function unknownGate(reason: string): AgenticGate {
  return { id: 'design-session-readback', label: 'DesignSession readback', status: 'unknown', failedMetrics: [], reason }
}

function emptyEvidenceBindings(binding: AgenticSessionBinding): AgenticEvidenceBinding[] {
  return EVIDENCE_BINDINGS.map(([id, label]) => ({
    id,
    label,
    expected: binding[id] ?? null,
    actual: null,
    status: 'unknown',
  }))
}

export function unavailableAgenticSessionProjection(
  binding: AgenticSessionBinding = { visualEvidenceBound: false },
  reason = 'AGENTIC_SESSION_UNAVAILABLE',
): AgenticSessionProjection {
  return {
    status: 'unavailable',
    source: 'runtime-authenticated-read-only',
    code: reason,
    reason,
    projectId: binding.projectId ?? null,
    candidateId: binding.candidateId ?? null,
    sessionId: null,
    stage: unknownStage(reason),
    durable: null,
    checkpoint: checkpoint(null, reason),
    restore: {
      status: 'locked',
      prepareStatus: 'locked',
      approvalStatus: 'locked',
      rawStatus: null,
      reason,
      viewerAction: 'locked-read-only',
    },
    uncertainty: { observed: [], inferred: [], unknown: ['project', 'candidate', 'revision', 'session', 'checkpoint', 'restore'] },
    failedGates: [unknownGate(reason)],
    allowedActions: [],
    lockedActions: mergeLockedActions([], reason),
    evidenceHashes: {
      artifactSha256: null,
      referenceSha256: null,
      renderSetHash: null,
      comparisonReportHash: null,
      qualityReportHash: null,
    },
    evidenceBindings: emptyEvidenceBindings(binding),
    bindingStatus: reason.includes('MISMATCH') ? 'mismatch' : 'unknown',
    readOnly: true,
  }
}

function unwrapProjection(payload: unknown): { envelope: JsonObject | null; record: JsonObject | null } {
  const envelope = asObject(payload)
  const nested = asObject(envelope?.projection)
  return { envelope, record: nested ?? envelope }
}

function durableCheckpoint(session: JsonObject | null): JsonObject | null {
  if (!session) return null
  const checkpointId = readString(session, 'current_checkpoint_id', 'currentCheckpointId')
  return {
    status: checkpointId ? 'persisted' : 'locked',
    durable: true,
    checkpoint_id: checkpointId,
    revision: session.revision ?? null,
    reason: checkpointId ? null : 'no immutable checkpoint has been persisted',
  }
}

function identityMismatch(record: JsonObject, session: JsonObject, stagePlan: JsonObject, binding: AgenticSessionBinding): string | null {
  const expectedProject = nonEmptyString(binding.projectId)
  const expectedCandidate = nonEmptyString(binding.candidateId)
  if (!expectedProject || !expectedCandidate) return 'AGENTIC_SESSION_BINDING_MISSING'
  const records = [record, session, stagePlan]
  for (const current of records) {
    const projectId = readString(current, 'project_id', 'projectId')
    const candidateId = readString(current, 'candidate_id', 'candidateId')
    if (projectId !== expectedProject || candidateId !== expectedCandidate) return 'AGENTIC_SESSION_BINDING_MISMATCH'
  }
  return null
}

function lineageMismatch(lineages: JsonObject[], binding: AgenticSessionBinding): { status: AgenticBindingStatus; reason: string | null } {
  const fieldPairs: Array<[string, string[], string | number | undefined]> = [
    ['project', ['project_id'], binding.projectId],
    ['candidate', ['candidate_id'], binding.candidateId],
    ['snapshot', ['snapshot_id'], binding.revision?.snapshotId],
    ['snapshot_revision', ['snapshot_revision'], binding.revision?.snapshotRevision],
    ['snapshot_manifest', ['snapshot_manifest_hash'], binding.revision?.snapshotManifestHash],
    ['candidate_canonical', ['candidate_canonical_sha256'], binding.revision?.candidateCanonicalSha256],
  ]
  const statuses: AgenticBindingStatus[] = []
  for (const [label, keys, expected] of fieldPairs) {
    const actual = lineageValue(lineages, ...keys)
    const status = compareBinding(expected, actual)
    statuses.push(status)
    if (status === 'mismatch') return { status, reason: `AGENTIC_SESSION_${label.toUpperCase()}_MISMATCH` }
  }
  const consistencyKeys = [
    'project_id',
    'candidate_id',
    'snapshot_id',
    'snapshot_revision',
    'snapshot_manifest_hash',
    'candidate_canonical_sha256',
    'artifact_sha256',
    'reference_sha256',
    'render_set_hash',
    'comparison_report_hash',
    'quality_report_hash',
  ]
  for (const key of consistencyKeys) {
    const values = lineages.map((item) => scalar(item[key])).filter((item): item is string | number => item !== null)
    if (new Set(values).size > 1) return { status: 'mismatch', reason: `AGENTIC_SESSION_LINEAGE_${key.toUpperCase()}_MISMATCH` }
  }
  return { status: combineBindingStatus(statuses), reason: null }
}

export function normalizeAgenticSessionProjection(
  payload: unknown,
  binding: AgenticSessionBinding,
): AgenticSessionProjection {
  const { envelope, record } = unwrapProjection(payload)
  const durableEnvelope = asObject(envelope?.durable_session)
  const durableSession = asObject(durableEnvelope?.session ?? envelope?.durable_session)
  const session = durableSession ?? asObject(record?.design_session)
  const stagePlan = asObject(record?.design_stage_plan)
  if (!record || !session || !stagePlan) return unavailableAgenticSessionProjection(binding, 'AGENTIC_SESSION_NOT_IN_READBACK')
  if (envelope?.readback_kind && envelope.readback_kind !== 'design-session-checkpoint') {
    return unavailableAgenticSessionProjection(binding, 'AGENTIC_SESSION_READBACK_KIND_MISMATCH')
  }
  if (envelope?.read_only !== undefined && envelope.read_only !== true) {
    return unavailableAgenticSessionProjection(binding, 'AGENTIC_SESSION_READBACK_NOT_READ_ONLY')
  }
  const identityError = identityMismatch(record, session, stagePlan, binding)
  if (identityError) return unavailableAgenticSessionProjection(binding, identityError)

  const baseProjection = normalizeAgenticDesignProjection(record, binding)
  if (baseProjection.status === 'unavailable') {
    return unavailableAgenticSessionProjection(binding, baseProjection.code ?? 'AGENTIC_SESSION_PROJECTION_UNAVAILABLE')
  }
  const sessionLineage = lineage(session.lineage)
  const stagePlanLineage = lineage(stagePlan.lineage)
  const recordLineage = lineage(record.lineage)
  const lineages = [recordLineage, sessionLineage, stagePlanLineage].filter((item): item is JsonObject => item !== null)
  const lineageResult = lineageMismatch(lineages, binding)
  const actualHashes = baseProjection.evidenceHashes
  const hashBindings = evidenceBindings(actualHashes, binding)
  const hashStatus = combineBindingStatus(hashBindings.map((item) => item.status))
  const bindingStatus = combineBindingStatus([lineageResult.status, hashStatus])
  if (bindingStatus === 'mismatch') {
    return unavailableAgenticSessionProjection(binding, lineageResult.reason ?? 'AGENTIC_SESSION_EVIDENCE_BINDING_MISMATCH')
  }

  const sessionStatusReason = readString(session, 'reason')
  const stage = baseProjection.stage
  const checkpointValue = session.checkpoint ?? stagePlan.checkpoint ?? durableCheckpoint(durableSession)
  const restoreValue = session.rollback_intent ?? stagePlan.rollback
  const checkpointState = checkpoint(checkpointValue, 'checkpoint is not present in Runtime projection')
  const restoreState = restore(restoreValue, 'restore intent is not present in Runtime projection')
  const sessionActions = actions(stagePlan.allowed_actions, 'allowed')
  const stageLockedActions = actions(stagePlan.blocked_actions, 'locked')
  const allowedActions = bindingStatus === 'bound' && baseProjection.status === 'ready'
    ? sessionActions.filter((action) => !LOCKED_SESSION_ACTIONS.some((locked) => locked.actionId === action.actionId))
    : []
  const lockedActions = mergeLockedActions(
    bindingStatus === 'bound' ? stageLockedActions : [...sessionActions, ...stageLockedActions],
    bindingStatus === 'bound' ? 'Runtime stage plan keeps this action locked' : 'project/candidate/revision/evidence binding is not proven',
  )
  const sceneGraph = asObject(record.semantic_scene_graph)
  const modelBundle = asObject(record.model_understanding_bundle)
  const uncertaintyState = uncertainty(sceneGraph?.uncertainty, modelBundle?.uncertainty, record.uncertainty)
  if (bindingStatus !== 'bound') {
    uncertaintyState.unknown = uniqueStrings([...uncertaintyState.unknown, 'revision_binding', 'evidence_hash_binding'])
  }
  const failedGates = baseProjection.gates.filter((gate) => gate.status === 'failed')
  return {
    status: 'ready',
    source: 'runtime-authenticated-read-only',
    code: null,
    reason: bindingStatus === 'unknown'
      ? 'project/candidate identity matched; revision or evidence hash binding remains unknown'
      : sessionStatusReason ?? checkpointState.reason,
    projectId: baseProjection.projectId,
    candidateId: baseProjection.candidateId,
    sessionId: readString(session, 'session_id', 'sessionId'),
    stage,
    durable: durableSession ? true : typeof session.durable === 'boolean' ? session.durable : null,
    checkpoint: checkpointState,
    restore: restoreState,
    uncertainty: uncertaintyState,
    failedGates: failedGates.length > 0 ? failedGates : baseProjection.gates,
    allowedActions,
    lockedActions,
    evidenceHashes: actualHashes,
    evidenceBindings: hashBindings,
    bindingStatus,
    readOnly: true,
  }
}

export function runAgenticSessionNormalizationFixtures(): { status: 'PASS'; cases: number } {
  const binding: AgenticSessionBinding = {
    projectId: 'project-a',
    candidateId: 'candidate-a',
    artifactSha256: 'artifact-a',
    referenceSha256: 'reference-a',
    renderSetHash: 'render-a',
    comparisonReportHash: 'comparison-a',
    qualityReportHash: 'quality-a',
    visualEvidenceBound: true,
    revision: {
      snapshotId: 'snapshot-a',
      snapshotRevision: 7,
      snapshotManifestHash: 'manifest-a',
      candidateCanonicalSha256: 'candidate-canonical-a',
    },
  }
  const positive = {
    projection_status: 'projection/read-only',
    read_only: true,
    project_id: 'project-a',
    candidate_id: 'candidate-a',
    lineage: {
      project_id: 'project-a', candidate_id: 'candidate-a', snapshot_id: 'snapshot-a', snapshot_revision: 7,
      snapshot_manifest_hash: 'manifest-a', candidate_canonical_sha256: 'candidate-canonical-a', artifact_sha256: 'artifact-a',
      reference_sha256: 'reference-a', render_set_hash: 'render-a', comparison_report_hash: 'comparison-a', quality_report_hash: 'quality-a',
    },
    design_session: {
      project_id: 'project-a', candidate_id: 'candidate-a', session_id: 'session-a', durable: false, current_stage: 'primary-form',
      checkpoint: { status: 'not-persisted', durable: false }, rollback_intent: { status: 'not-persisted', durable: false },
      lineage: { project_id: 'project-a', candidate_id: 'candidate-a', snapshot_id: 'snapshot-a', snapshot_revision: 7, snapshot_manifest_hash: 'manifest-a', candidate_canonical_sha256: 'candidate-canonical-a', artifact_sha256: 'artifact-a', reference_sha256: 'reference-a', render_set_hash: 'render-a', comparison_report_hash: 'comparison-a', quality_report_hash: 'quality-a' },
    },
    design_stage_plan: {
      project_id: 'project-a', candidate_id: 'candidate-a', current_stage: 'primary-form', current_stage_status: 'awaiting-evidence',
      allowed_actions: ['inspect_artifact'], blocked_actions: ['candidate_confirm', 'export_confirm'],
      checkpoint: { status: 'not-persisted', durable: false }, rollback: { status: 'not-available-in-projection' },
      lineage: { project_id: 'project-a', candidate_id: 'candidate-a', snapshot_id: 'snapshot-a', snapshot_revision: 7, snapshot_manifest_hash: 'manifest-a', candidate_canonical_sha256: 'candidate-canonical-a', artifact_sha256: 'artifact-a', reference_sha256: 'reference-a', render_set_hash: 'render-a', comparison_report_hash: 'comparison-a', quality_report_hash: 'quality-a' },
      strict_visual_gate: { status: 'unknown' }, quality_gate: { structural_status: 'unknown' },
    },
    design_critic_report: { issues: [] },
    visual_evidence_bundle: { available: false },
    semantic_scene_graph: { uncertainty: { observed: ['project_id'], inferred: ['display_name'], unknown: ['bbox'] } },
  }
  const normalized = normalizeAgenticSessionProjection(positive, binding)
  if (normalized.status !== 'ready' || normalized.bindingStatus !== 'bound' || normalized.allowedActions[0]?.actionId !== 'inspect_artifact') {
    throw new Error('Agentic session positive fixture failed')
  }
  if (normalized.lockedActions.some((action) => action.actionId === 'candidate_confirm' && action.status !== 'locked')) {
    throw new Error('Agentic session approval boundary fixture failed')
  }
  const mismatch = normalizeAgenticSessionProjection({ ...positive, lineage: { ...positive.lineage, snapshot_revision: 8 } }, binding)
  if (mismatch.status !== 'unavailable' || mismatch.bindingStatus !== 'mismatch') throw new Error('Agentic session revision mismatch fixture failed')
  const unknown = normalizeAgenticSessionProjection({ ...positive, lineage: { ...positive.lineage, quality_report_hash: null } }, { ...binding, qualityReportHash: undefined })
  if (unknown.status !== 'ready' || unknown.bindingStatus !== 'unknown' || unknown.allowedActions.length !== 0) throw new Error('Agentic session unknown binding fixture failed')
  return { status: 'PASS', cases: 3 }
}

export function assertAgenticSessionViewerSourceContract(source: string): void {
  const required = ['normalizeAgenticSessionProjection', 'viewer_agentic_session', 'locked-read-only', 'evidenceBindings', 'uncertainty']
  const missing = required.filter((token) => !source.includes(token))
  if (missing.length > 0) throw new Error(`Agentic session source is missing: ${missing.join(', ')}`)
  const forbiddenInvocations = [
    ['candidate', '_confirm('],
    ['export', '_confirm('],
    ['restore', '_confirm('],
    ['invoke', 'Model('],
    ['fet', 'ch('],
  ].map(([prefix, suffix]) => `${prefix}${suffix}`)
  const leaked = forbiddenInvocations.filter((token) => source.includes(token))
  if (leaked.length > 0) throw new Error(`Agentic session source contains forbidden action invocation: ${leaked.join(', ')}`)
}
