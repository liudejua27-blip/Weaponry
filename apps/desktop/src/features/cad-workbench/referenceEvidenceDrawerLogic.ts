import type {
  ReferenceEvidenceProjectRead,
  ReferenceGuidedRebuildPlanRead,
} from '../../shared/api/forgeApi.js'

export type ReferenceEvidenceKind = 'image' | 'glb'

export type ReferenceEvidenceRecord = {
  evidenceId: string
  contentSha256: string
  kind: ReferenceEvidenceKind
  fileName: string
  sourceStatement: string
  licenseStatement: string
  missingViews: string[]
  uncertainties: string[]
  /** Explicit coverage declaration when the persisted contract provides one. */
  referenceClass?: 'single_image' | 'multi_view_contact_sheet' | 'glb_readback'
}

export type ReferenceEvidenceTarget = {
  projectId: string
  domainPackId: string | null
  baseAssetVersionId: string | null
}

export type ReferenceEvidenceHistoryEntry = {
  evidence: ReferenceEvidenceRecord
  comparison: ReferenceRebuildComparisonPlan | null
  rebuildPlanId: string | null
  resultAssetVersionId: string | null
  /** Omitted for legacy R007A history rather than guessed from partial data. */
  lineage?: ReferenceRebuildExactLineage | null
}

export type ReferenceEvidenceAdapter = {
  /** Invalidates any in-flight adapter work when the drawer/project closes. */
  invalidate?: () => void
  createEvidence: (input: ReferenceEvidenceCreateInput) => Promise<
    { status: 'created'; evidence: ReferenceEvidenceRecord } | { status: 'unavailable' | 'failed'; message: string }
  >
  previewRebuild: (target: ReferenceEvidenceTarget, evidence: ReferenceEvidenceRecord) => Promise<ReferenceRebuildPreviewResponse>
  retain: (changeSetId: string) => Promise<ReferenceRebuildRetainResponse>
  cancel: (changeSetId: string) => Promise<void>
  /** Reads persisted project evidence/plan identities when the drawer is reopened. */
  loadHistory?: (target: ReferenceEvidenceTarget) => Promise<ReferenceEvidenceHistoryEntry[]>
  /** Source bytes are always fetched by Project + evidence ID, never by CAS path/hash. */
  loadContent?: (target: ReferenceEvidenceTarget, evidence: ReferenceEvidenceRecord) => Promise<Blob>
  /** GLB reference A/B viewing must reuse the workbench's existing viewport. */
  viewReferenceGlb?: (target: ReferenceEvidenceTarget, evidence: ReferenceEvidenceRecord) => Promise<{ status: 'ready' | 'unavailable' | 'failed'; message: string }>
  /** Image reference A/B viewing uses a transient texture in that same renderer. */
  viewReferenceImage?: (target: ReferenceEvidenceTarget, evidence: ReferenceEvidenceRecord) => Promise<{ status: 'ready' | 'unavailable' | 'failed'; message: string }>
  viewResult?: (target: ReferenceEvidenceTarget, entry: ReferenceEvidenceHistoryEntry) => void
}

export type ReferenceEvidenceCreateInput = {
  target: ReferenceEvidenceTarget
  file: File
  sourceStatement: string
  licenseStatement: string
  missingViews: string[]
  referenceClass: 'single_image' | 'multi_view_contact_sheet' | null
  notes: string
}

export type ReferenceRebuildPreviewResponse =
  | {
    status: 'preview_ready'
    changeSetId: string
    summary: string
    /** Presentation-only projection of existing ReferenceGuidedRebuildPlan@1 evidence lists. */
    comparison?: ReferenceRebuildComparisonPlan
    /**
     * A validated projection of the frozen Rust-owned reference plan. It is
     * deliberately an identity record, not an appearance score or a visual
     * analysis result.
     */
    lineage?: ReferenceRebuildExactLineage
  }
  | { status: 'unavailable' | 'failed'; message: string }

export type ReferenceRebuildRetainResponse =
  | { status: 'retained'; summary: string; lineage?: ReferenceRebuildExactLineage }
  | { status: 'unavailable' | 'failed'; message: string }

/**
 * The only R007B comparison fields that may reach the workbench. They map
 * one-to-one to the existing ReferenceGuidedRebuildPlan@1 evidence lists.
 */
export type ReferenceRebuildComparisonPlan = {
  retainedEvidence: string[]
  intendedDifferences: string[]
  unresolvedUncertainties: string[]
}

/**
 * The bounded, exact R007B identity that the workbench may display. Values
 * are copied only from a frozen plan/readback response after validation.
 */
export type ReferenceRebuildExactLineage = {
  evidenceId: string
  sourceObjectSha256: string
  rebuildPlanId: string
  analysisId: string
  fidelityCeiling: ReferenceSurfaceFidelityCeiling
  status: 'draft' | 'previewed' | 'confirmed' | 'rejected'
  previewChangeSetId: string | null
  confirmedAssetVersionId: string | null
  resultGlbSha256: string | null
}

export type ReferenceSurfaceFidelityCeiling =
  | 'single_image_visible_surface_only'
  | 'multi_view_image_visible_surface_only'
  | 'strict_glb_readback_visible_bounds_only'

export type ReferenceRebuildLineageExpectation = {
  evidenceId?: string
  sourceObjectSha256?: string
  previewChangeSetId?: string
}

type UnknownRecord = Record<string, unknown>

const SHA256_PATTERN = /^[a-f0-9]{64}$/
const STABLE_ID_PATTERN = /^[A-Za-z0-9_.-]{1,160}$/
const FIDELITY_CEILINGS = new Set<ReferenceSurfaceFidelityCeiling>([
  'single_image_visible_surface_only',
  'multi_view_image_visible_surface_only',
  'strict_glb_readback_visible_bounds_only',
])
const REBUILD_STATUSES = new Set<ReferenceRebuildExactLineage['status']>([
  'draft',
  'previewed',
  'confirmed',
  'rejected',
])
const FORBIDDEN_LINEAGE_FIELD = /(?:similarity|score|vision|pixel|provider|visual_fidelity)/i

export type ReferenceDrawerCancelGuard = { current: boolean }
export type ReferenceDrawerCancelResult =
  | { status: 'cancelled' }
  | { status: 'pending' }
  | { status: 'failed'; message: string }

/** Project/domain is the evidence scope; an asset-version advance is state, not a new drawer. */
export function referenceEvidenceScopeKey(target: ReferenceEvidenceTarget | null): string {
  return target ? `${target.projectId}:${target.domainPackId ?? 'unknown'}` : 'unavailable'
}

export function isReferencePreviewBaseStale(
  previewBaseAssetVersionId: string | null,
  currentBaseAssetVersionId: string | null,
): boolean {
  return previewBaseAssetVersionId !== null
    && currentBaseAssetVersionId !== null
    && previewBaseAssetVersionId !== currentBaseAssetVersionId
}

/**
 * One terminal cancellation coordinator shared by the close button and Escape.
 * It never reports success before the adapter has rejected the ChangeSet and
 * refreshed the authoritative Snapshot. A failed attempt releases the guard so
 * the same visible preview remains retryable.
 */
export async function cancelReferencePreviewOnce(
  adapter: Pick<ReferenceEvidenceAdapter, 'cancel'>,
  changeSetId: string,
  guard: ReferenceDrawerCancelGuard,
): Promise<ReferenceDrawerCancelResult> {
  if (guard.current) return { status: 'pending' }
  guard.current = true
  try {
    await adapter.cancel(changeSetId)
    return { status: 'cancelled' }
  } catch (caught) {
    const suffix = caught instanceof Error && caught.message.trim() ? ` ${caught.message}` : ''
    return {
      status: 'failed',
      message: `取消参考重建预览失败；预览仍保留，请重试。${suffix}`,
    }
  } finally {
    guard.current = false
  }
}

export function isReferenceDrawerCloseShortcut(key: string): boolean {
  return key === 'Escape'
}

function isUnknownRecord(value: unknown): value is UnknownRecord {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value)
}

function isStableId(value: unknown): value is string {
  return typeof value === 'string' && STABLE_ID_PATTERN.test(value)
}

function isSha256(value: unknown): value is string {
  return typeof value === 'string' && SHA256_PATTERN.test(value)
}

/**
 * The R007B surface contract must never gain a hidden visual-analysis channel
 * through a future response field. Reject the whole projection instead of
 * silently ignoring a similarity, vision, pixel or provider assertion.
 */
function containsForbiddenLineageField(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(containsForbiddenLineageField)
  if (!isUnknownRecord(value)) return false
  return Object.entries(value).some(([key, child]) => (
    FORBIDDEN_LINEAGE_FIELD.test(key) || containsForbiddenLineageField(child)
  ))
}

function nullableStableId(value: unknown): string | null | undefined {
  if (value === null || value === undefined) return null
  return isStableId(value) ? value : undefined
}

function nullableSha256(value: unknown): string | null | undefined {
  if (value === null || value === undefined) return null
  return isSha256(value) ? value : undefined
}

function safeStringList(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  const list: string[] = []
  for (let index = 0; index < value.length && list.length < 16; index += 1) {
    const item = value[index]
    if (typeof item !== 'string') continue
    const valueText = item.trim()
    if (valueText) list.push(valueText)
  }
  return list
}

/**
 * Read the existing plan lists without requiring a new endpoint shape. This
 * accepts either current-plan envelope naming and ignores future structural
 * fields rather than guessing at them.
 */
export function readReferenceRebuildComparisonPlan(value: unknown): ReferenceRebuildComparisonPlan | null {
  if (!isUnknownRecord(value)) return null
  const nested = value.reference_guided_rebuild_plan ?? value.referenceGuidedRebuildPlan ?? value.rebuild_plan
  const plan = isUnknownRecord(nested) ? nested : value
  const retainedEvidence = safeStringList(plan.retained_evidence)
  const intendedDifferences = safeStringList(plan.intended_differences)
  const unresolvedUncertainties = safeStringList(plan.unresolved_uncertainties)
  if (retainedEvidence.length === 0 && intendedDifferences.length === 0 && unresolvedUncertainties.length === 0) return null
  return { retainedEvidence, intendedDifferences, unresolvedUncertainties }
}

/** Guards adapter-provided presentation data before it reaches the DOM. */
export function isReferenceRebuildExactLineage(value: unknown): value is ReferenceRebuildExactLineage {
  if (!isUnknownRecord(value) || containsForbiddenLineageField(value)) return false
  const previewChangeSetId = nullableStableId(value.previewChangeSetId)
  const confirmedAssetVersionId = nullableStableId(value.confirmedAssetVersionId)
  const resultGlbSha256 = nullableSha256(value.resultGlbSha256)
  if (
    !isStableId(value.evidenceId)
    || !isSha256(value.sourceObjectSha256)
    || !isStableId(value.rebuildPlanId)
    || !isStableId(value.analysisId)
    || typeof value.status !== 'string'
    || !REBUILD_STATUSES.has(value.status as ReferenceRebuildExactLineage['status'])
    || typeof value.fidelityCeiling !== 'string'
    || !FIDELITY_CEILINGS.has(value.fidelityCeiling as ReferenceSurfaceFidelityCeiling)
    || previewChangeSetId === undefined
    || confirmedAssetVersionId === undefined
    || resultGlbSha256 === undefined
  ) return false
  if (value.status === 'draft') return previewChangeSetId === null && confirmedAssetVersionId === null && resultGlbSha256 === null
  if (value.status === 'previewed' || value.status === 'rejected') {
    return previewChangeSetId !== null && confirmedAssetVersionId === null && resultGlbSha256 === null
  }
  return (
    previewChangeSetId !== null
    && confirmedAssetVersionId !== null
    && resultGlbSha256 !== null
    && resultGlbSha256 !== value.sourceObjectSha256
  )
}

/** A preview is displayable only when it carries its exact frozen lineage. */
export function readReferenceRebuildPreviewLineage(
  response: ReferenceRebuildPreviewResponse,
): ReferenceRebuildExactLineage | null {
  if (response.status !== 'preview_ready' || !isReferenceRebuildExactLineage(response.lineage)) return null
  if (
    response.lineage.status !== 'previewed'
    || response.lineage.previewChangeSetId !== response.changeSetId
    || response.lineage.confirmedAssetVersionId !== null
    || response.lineage.resultGlbSha256 !== null
  ) return null
  return response.lineage
}

/** A retained result is displayable only after Rust binds a distinct result GLB. */
export function readReferenceRebuildRetainLineage(
  response: ReferenceRebuildRetainResponse,
): ReferenceRebuildExactLineage | null {
  if (response.status !== 'retained' || !isReferenceRebuildExactLineage(response.lineage)) return null
  if (
    response.lineage.status !== 'confirmed'
    || response.lineage.confirmedAssetVersionId === null
    || response.lineage.resultGlbSha256 === null
  ) return null
  return response.lineage
}

export const unavailableReferenceEvidenceAdapter: ReferenceEvidenceAdapter = {
  invalidate() {},
  async createEvidence() {
    return { status: 'unavailable', message: '参考证据服务尚未连接；没有上传文件，也没有创建设计版本。' }
  },
  async previewRebuild() {
    return { status: 'unavailable', message: '参考引导重建尚未连接；当前模型没有变化。' }
  },
  async retain() {
    return { status: 'unavailable', message: '没有可保留的参考重建预览。' }
  },
  async cancel() {},
}

/**
 * Projects only an internally consistent frozen R007B plan. The input may be
 * either the read endpoint envelope or the reference-preview response.
 */
export function readReferenceRebuildExactLineage(
  value: unknown,
  expectation: ReferenceRebuildLineageExpectation = {},
): ReferenceRebuildExactLineage | null {
  if (!isUnknownRecord(value) || containsForbiddenLineageField(value)) return null
  const plan = value.reference_guided_rebuild_plan
  const analysis = value.reference_surface_analysis
  const pair = value.reference_result_pair
  if (!isUnknownRecord(plan) || !isUnknownRecord(analysis)) return null

  const rebuildPlanId = plan.rebuild_plan_id
  const evidenceId = plan.evidence_id
  const status = plan.status
  const previewChangeSetId = nullableStableId(plan.preview_change_set_id)
  const confirmedAssetVersionId = nullableStableId(plan.confirmed_asset_version_id)
  const analysisId = analysis.analysis_id
  const fidelityCeiling = analysis.fidelity_ceiling
  const sourceObjectSha256 = analysis.source_object_sha256
  if (
    !isStableId(rebuildPlanId)
    || !isStableId(evidenceId)
    || !isStableId(analysisId)
    || !isSha256(sourceObjectSha256)
    || typeof status !== 'string'
    || !REBUILD_STATUSES.has(status as ReferenceRebuildExactLineage['status'])
    || typeof fidelityCeiling !== 'string'
    || !FIDELITY_CEILINGS.has(fidelityCeiling as ReferenceSurfaceFidelityCeiling)
    || previewChangeSetId === undefined
    || confirmedAssetVersionId === undefined
    || analysis.rebuild_plan_id !== rebuildPlanId
    || analysis.evidence_id !== evidenceId
    || (expectation.evidenceId !== undefined && evidenceId !== expectation.evidenceId)
    || (expectation.sourceObjectSha256 !== undefined && sourceObjectSha256 !== expectation.sourceObjectSha256)
    || (expectation.previewChangeSetId !== undefined && previewChangeSetId !== expectation.previewChangeSetId)
  ) return null

  let resultGlbSha256: string | null = null
  if (pair !== null && pair !== undefined) {
    if (!isUnknownRecord(pair) || pair.source_object_sha256 !== sourceObjectSha256) return null
    const pairResultAssetVersionId = nullableStableId(pair.result_asset_version_id)
    const pairResultGlbSha256 = nullableSha256(pair.result_glb_sha256)
    if (pairResultAssetVersionId === undefined || pairResultGlbSha256 === undefined) return null
    if (pairResultAssetVersionId !== confirmedAssetVersionId) return null
    resultGlbSha256 = pairResultGlbSha256
  }

  if (status === 'draft') {
    if (previewChangeSetId !== null || confirmedAssetVersionId !== null || resultGlbSha256 !== null) return null
  } else if (status === 'previewed' || status === 'rejected') {
    if (previewChangeSetId === null || confirmedAssetVersionId !== null || resultGlbSha256 !== null) return null
  } else {
    if (
      previewChangeSetId === null
      || confirmedAssetVersionId === null
      || resultGlbSha256 === null
      || resultGlbSha256 === sourceObjectSha256
    ) return null
  }

  return {
    evidenceId,
    sourceObjectSha256,
    rebuildPlanId,
    analysisId,
    fidelityCeiling: fidelityCeiling as ReferenceSurfaceFidelityCeiling,
    status: status as ReferenceRebuildExactLineage['status'],
    previewChangeSetId,
    confirmedAssetVersionId,
    resultGlbSha256,
  }
}

type ReferenceEvidenceHistoryLoader = {
  listProjectReferenceEvidence(projectId: string): Promise<ReferenceEvidenceProjectRead>
  getReferenceGuidedRebuildPlan(projectId: string, rebuildPlanId: string): Promise<ReferenceGuidedRebuildPlanRead>
}

export async function loadReferenceEvidenceHistory(
  loader: ReferenceEvidenceHistoryLoader,
  projectId: string,
): Promise<ReferenceEvidenceHistoryEntry[]> {
  const indexRead = await loader.listProjectReferenceEvidence(projectId)
  const reads = await Promise.all(indexRead.reference_guided_rebuild_plans.map((plan) => (
    loader.getReferenceGuidedRebuildPlan(projectId, plan.rebuild_plan_id)
  )))
  const planByEvidenceId = new Map(reads.map((read) => [read.reference_guided_rebuild_plan.evidence_id, read]))
  const entries: ReferenceEvidenceHistoryEntry[] = new Array(indexRead.reference_evidence.length)
  for (let entryIndex = 0; entryIndex < indexRead.reference_evidence.length; entryIndex += 1) {
    const record = indexRead.reference_evidence[entryIndex]
    const read = planByEvidenceId.get(record.evidence_id)
    const plan = read?.reference_guided_rebuild_plan
    const lineage = read && plan?.project_id === projectId
      ? readReferenceRebuildExactLineage(read, {
        evidenceId: record.evidence_id,
        sourceObjectSha256: record.source_object_sha256,
      })
      : null
    entries[entryIndex] = {
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
    }
  return entries
}
