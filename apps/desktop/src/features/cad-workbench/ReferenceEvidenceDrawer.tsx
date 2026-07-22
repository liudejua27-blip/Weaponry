import { useEffect, useRef, useState, type ChangeEvent, type KeyboardEvent } from 'react'

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

export type ReferenceEvidenceHistoryEntry = {
  evidence: ReferenceEvidenceRecord
  comparison: ReferenceRebuildComparisonPlan | null
  rebuildPlanId: string | null
  resultAssetVersionId: string | null
  /** Omitted for legacy R007A history rather than guessed from partial data. */
  lineage?: ReferenceRebuildExactLineage | null
}

/**
 * R007 only accepts user-authorized evidence. The adapter owns all persistence
 * and ChangeSet calls; this drawer never treats a reference file as editable
 * geometry or creates a renderer of its own.
 */
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

export type ReferenceDrawerCancelGuard = { current: boolean }
export type ReferenceDrawerCancelResult =
  | { status: 'cancelled' }
  | { status: 'pending' }
  | { status: 'failed'; message: string }

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

/**
 * The only R007B comparison fields that may reach the workbench. They map
 * one-to-one to the existing ReferenceGuidedRebuildPlan@1 evidence lists.
 * This deliberately excludes source paths, licence text, structural details,
 * model reasoning and similarity scores.
 */
export type ReferenceRebuildComparisonPlan = {
  retainedEvidence: string[]
  intendedDifferences: string[]
  unresolvedUncertainties: string[]
}

/**
 * The bounded, exact R007B identity that the workbench may display. Values
 * are copied only from a frozen plan/readback response after validation; this
 * is never a model-similarity, vision, pixel or provider result.
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

export type ReferenceRebuildLineageExpectation = {
  evidenceId?: string
  sourceObjectSha256?: string
  previewChangeSetId?: string
}

/**
 * Projects only an internally consistent frozen R007B plan. The input may be
 * either the read endpoint envelope or the reference-preview response. No
 * partial / legacy record is upgraded into a lineage display.
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

function safeStringList(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  return value.filter((item): item is string => typeof item === 'string')
    .map((item) => item.trim())
    .filter(Boolean)
    .slice(0, 16)
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

export type ReferenceEvidenceDrawerProps = {
  open: boolean
  target: ReferenceEvidenceTarget | null
  adapter: ReferenceEvidenceAdapter
  onClose: () => void
  onMessage?: (message: string) => void
}

const VIEW_OPTIONS = [
  ['front', '正视图'],
  ['side', '侧视图'],
  ['rear', '后视图'],
  ['top', '顶视图'],
] as const

const IMAGE_ACCEPT = 'image/png,image/jpeg,image/webp'
const GLB_ACCEPT = '.glb,model/gltf-binary'
const MAX_IMAGE_BYTES = 16 * 1024 * 1024
const MAX_GLB_BYTES = 32 * 1024 * 1024

function missingViewDescription(missingViews: string[]): string {
  const labels = missingViews.map((view) => VIEW_OPTIONS.find(([id]) => id === view)?.[1] ?? '未标注视角')
  return labels.length > 0 ? labels.join('、') : '未声明缺失视角'
}

function ComparisonColumn({ title, items, empty }: { title: string; items: string[]; empty: string }) {
  return (
    <section className="reference-comparison-column" aria-label={title}>
      <h3>{title}</h3>
      {items.length > 0 ? <ul>{items.map((item) => <li key={item}>{item}</li>)}</ul> : <p>{empty}</p>}
    </section>
  )
}

function ReferenceRebuildComparison({
  evidence,
  comparison,
}: {
  evidence: ReferenceEvidenceRecord
  comparison: ReferenceRebuildComparisonPlan | null
}) {
  const hasMissingViews = evidence.missingViews.length > 0
  return (
    <section className="reference-rebuild-comparison" aria-labelledby="reference-rebuild-comparison-title">
      <header>
        <strong id="reference-rebuild-comparison-title">参考与重建对比</strong>
        <small>仅记录可见设计证据与受限重建计划；不显示相似度分数，也不推断遮挡或隐藏结构。</small>
      </header>
      <div className="reference-comparison-grid">
        <ComparisonColumn title="保留" items={comparison?.retainedEvidence ?? []} empty="重建计划尚未返回可展示的保留证据。" />
        <ComparisonColumn title="主动改变" items={comparison?.intendedDifferences ?? []} empty="重建计划尚未返回可展示的主动改变。" />
        <ComparisonColumn
          title="仍未知"
          items={comparison?.unresolvedUncertainties ?? evidence.uncertainties}
          empty="没有额外未知项；这不代表已获得隐藏结构或完整视角。"
        />
      </div>
      <p className="reference-fidelity-ceiling" role="note">
        <strong>保真度上限</strong>
        <span>{hasMissingViews
          ? `已标注缺失：${missingViewDescription(evidence.missingViews)}。这些区域不在本次证据范围内。`
          : evidence.kind === 'image'
            ? '单张图片只约束画面中可见的外观；遮挡、背面与内部结构仍保持未知。'
            : '参考 GLB 仍保持只读；结果是新的受限可编辑资产，而不是原网格的复制。'}</span>
      </p>
    </section>
  )
}

const FIDELITY_CEILING_LABELS: Record<ReferenceSurfaceFidelityCeiling, string> = {
  single_image_visible_surface_only: '单张图片的可见表面范围',
  multi_view_image_visible_surface_only: '多视图图片的可见表面范围',
  strict_glb_readback_visible_bounds_only: '只读 GLB 的严格 readback 可见范围',
}

const LINEAGE_STATUS_LABELS: Record<ReferenceRebuildExactLineage['status'], string> = {
  draft: '已保存，尚未预览',
  previewed: '预览中，尚未创建版本',
  confirmed: '已确认，结果版本已冻结',
  rejected: '已拒绝，未保留结果版本',
}

/** Renders immutable identities only; no estimated likeness or visual claim. */
function ReferenceRebuildLineage({ lineage }: { lineage: ReferenceRebuildExactLineage }) {
  return (
    <section
      className="reference-rebuild-comparison"
      aria-label="参考重建证据谱系"
      data-qa-evidence-id={lineage.evidenceId}
      data-qa-source-object-sha256={lineage.sourceObjectSha256}
      data-qa-rebuild-plan-id={lineage.rebuildPlanId}
      data-qa-analysis-id={lineage.analysisId}
      data-qa-fidelity-ceiling={lineage.fidelityCeiling}
      data-qa-preview-change-set-id={lineage.previewChangeSetId ?? ''}
      data-qa-confirmed-asset-version-id={lineage.confirmedAssetVersionId ?? ''}
      data-qa-result-glb-sha256={lineage.resultGlbSha256 ?? ''}
      data-qa-lineage-status={lineage.status}
    >
      <header>
        <strong>证据谱系</strong>
        <small>这是冻结的来源、计划和结果身份记录，不是外观评分、视觉识别或完整性结论。</small>
      </header>
      <dl className="reference-comparison-grid">
        <div><dt>状态</dt><dd>{LINEAGE_STATUS_LABELS[lineage.status]}</dd></div>
        <div><dt>证据 ID</dt><dd><code>{lineage.evidenceId}</code></dd></div>
        <div><dt>来源 SHA-256</dt><dd><code>{lineage.sourceObjectSha256}</code></dd></div>
        <div><dt>重建计划 ID</dt><dd><code>{lineage.rebuildPlanId}</code></dd></div>
        <div><dt>分析记录 ID</dt><dd><code>{lineage.analysisId}</code></dd></div>
        <div><dt>证据范围</dt><dd>{FIDELITY_CEILING_LABELS[lineage.fidelityCeiling]}</dd></div>
        {lineage.previewChangeSetId && <div><dt>预览 ChangeSet</dt><dd><code>{lineage.previewChangeSetId}</code></dd></div>}
        {lineage.confirmedAssetVersionId && <div><dt>结果资产版本</dt><dd><code>{lineage.confirmedAssetVersionId}</code></dd></div>}
        {lineage.resultGlbSha256 && <div><dt>结果 GLB SHA-256</dt><dd><code>{lineage.resultGlbSha256}</code></dd></div>}
      </dl>
      <p className="reference-fidelity-ceiling" role="note">
        <strong>受限说明</strong>
        <span>证据范围之外的遮挡、隐藏结构、精确尺寸、材料物理和功能均保持未知。</span>
      </p>
    </section>
  )
}

function kindFromFile(file: File): ReferenceEvidenceKind | null {
  if (file.name.toLowerCase().endsWith('.glb') || file.type === 'model/gltf-binary') return 'glb'
  if (['image/png', 'image/jpeg', 'image/webp'].includes(file.type)) return 'image'
  return null
}

function validateFile(file: File): string | null {
  const kind = kindFromFile(file)
  if (!kind) return '仅支持 PNG、JPEG、WebP 或自包含 GLB 参考。'
  if (file.size === 0) return '参考文件为空，未上传。'
  if (file.size > (kind === 'image' ? MAX_IMAGE_BYTES : MAX_GLB_BYTES)) {
    return kind === 'image' ? '图片超过 16 MB 轻量限制。' : 'GLB 超过 32 MB 轻量限制。'
  }
  return null
}

export function ReferenceEvidenceDrawer({
  open,
  target,
  adapter,
  onClose,
  onMessage,
}: ReferenceEvidenceDrawerProps) {
  const [file, setFile] = useState<File | null>(null)
  const [sourceStatement, setSourceStatement] = useState('')
  const [licenseStatement, setLicenseStatement] = useState('')
  const [notes, setNotes] = useState('')
  const [missingViews, setMissingViews] = useState<string[]>([])
  const [referenceClass, setReferenceClass] = useState<'single_image' | 'multi_view_contact_sheet'>('single_image')
  const [evidence, setEvidence] = useState<ReferenceEvidenceRecord | null>(null)
  const [comparison, setComparison] = useState<ReferenceRebuildComparisonPlan | null>(null)
  const [lineage, setLineage] = useState<ReferenceRebuildExactLineage | null>(null)
  const [history, setHistory] = useState<ReferenceEvidenceHistoryEntry[]>([])
  const [historyLoading, setHistoryLoading] = useState(false)
  const [imageUrl, setImageUrl] = useState<string | null>(null)
  const [status, setStatus] = useState<'editing' | 'saving_evidence' | 'evidence_ready' | 'building_preview' | 'preview_ready' | 'preview_stale' | 'cancelling' | 'cancel_failed' | 'failed'>('editing')
  const [detail, setDetail] = useState('')
  const fileInputRef = useRef<HTMLInputElement | null>(null)
  const closeButtonRef = useRef<HTMLButtonElement | null>(null)
  const dialogRef = useRef<HTMLElement | null>(null)
  const requestTokenRef = useRef(0)
  const changeSetIdRef = useRef<string | null>(null)
  const previewBaseAssetVersionIdRef = useRef<string | null>(null)
  const cancelPendingRef = useRef(false)
  // Cancellation completion must not be invalidated by history reloads or
  // harmless target object hydration; only a real drawer reset/project switch
  // advances this independent close-attempt epoch.
  const cancelAttemptRef = useRef(0)
  const retainAttemptRef = useRef(0)
  const scopeKey = referenceEvidenceScopeKey(target)
  const historyProjectId = target?.projectId ?? null
  const historyDomainPackId = target?.domainPackId ?? null

  const reset = () => {
    adapter.invalidate?.()
    requestTokenRef.current += 1
    cancelAttemptRef.current += 1
    retainAttemptRef.current += 1
    changeSetIdRef.current = null
    previewBaseAssetVersionIdRef.current = null
    cancelPendingRef.current = false
    setFile(null)
    setSourceStatement('')
    setLicenseStatement('')
    setNotes('')
    setMissingViews([])
    setReferenceClass('single_image')
    setEvidence(null)
    setComparison(null)
    setLineage(null)
    setHistory([])
    setHistoryLoading(false)
    if (imageUrl) URL.revokeObjectURL(imageUrl)
    setImageUrl(null)
    setStatus('editing')
    setDetail('')
    if (fileInputRef.current) fileInputRef.current.value = ''
  }

  useEffect(() => {
    reset()
  // Only Project/Domain changes reset evidence. A successful retain advances
  // the base version inside this same scope and must preserve its final lineage.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopeKey])

  useEffect(() => () => {
    if (imageUrl) URL.revokeObjectURL(imageUrl)
  }, [imageUrl])

  useEffect(() => {
    if (!open || !historyProjectId || !adapter.loadHistory) return
    const token = ++requestTokenRef.current
    setHistoryLoading(true)
    void adapter.loadHistory({
      projectId: historyProjectId,
      domainPackId: historyDomainPackId,
      baseAssetVersionId: null,
    })
      .then((entries) => {
        if (token !== requestTokenRef.current) return
        setHistory(entries)
      })
      .catch(() => {
        if (token !== requestTokenRef.current) return
        setHistory([])
      })
      .finally(() => {
        if (token === requestTokenRef.current) setHistoryLoading(false)
      })
  }, [adapter, historyDomainPackId, historyProjectId, open])

  useEffect(() => {
    if (
      !changeSetIdRef.current
      || status === 'building_preview'
      || status === 'cancelling'
      || status === 'preview_stale'
      || !isReferencePreviewBaseStale(previewBaseAssetVersionIdRef.current, target?.baseAssetVersionId ?? null)
    ) return
    setStatus('preview_stale')
    setDetail('当前设计已在别处推进；旧参考预览不能确认。请取消该预览后重新生成。')
  }, [status, target?.baseAssetVersionId])

  useEffect(() => {
    if (!open) return
    window.requestAnimationFrame(() => closeButtonRef.current?.focus())
  }, [open])

  if (!open) return null

  const close = () => {
    const changeSetId = changeSetIdRef.current
    if (!changeSetId) {
      // Upload/evidence-only drawers have no product preview to settle, so
      // ordinary closing remains synchronous.
      reset()
      onClose()
      return
    }
    if (cancelPendingRef.current) return
    const cancelAttempt = ++cancelAttemptRef.current
    setStatus('cancelling')
    setDetail('正在取消参考重建预览…')
    void cancelReferencePreviewOnce(adapter, changeSetId, cancelPendingRef).then((result) => {
      if (cancelAttempt !== cancelAttemptRef.current) return
      if (result.status === 'pending') return
      if (result.status === 'failed') {
        setStatus('cancel_failed')
        setDetail(result.message)
        onMessage?.(result.message)
        return
      }
      reset()
      onClose()
    })
  }

  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (isReferenceDrawerCloseShortcut(event.key)) {
      event.preventDefault()
      close()
      return
    }
    if (event.key !== 'Tab') return
    const dialog = dialogRef.current
    if (!dialog) return
    const controls = [...dialog.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), textarea:not(:disabled)')]
    if (controls.length === 0) return
    const first = controls[0]
    const last = controls[controls.length - 1]
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last?.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first?.focus()
    }
  }

  const selectFile = (event: ChangeEvent<HTMLInputElement>) => {
    const selected = event.target.files?.[0] ?? null
    if (!selected) return
    const error = validateFile(selected)
    if (error) {
      setFile(null)
      setDetail(error)
      setStatus('failed')
      event.target.value = ''
      return
    }
    setFile(selected)
    setEvidence(null)
    setComparison(null)
    setLineage(null)
    setStatus('editing')
    if (imageUrl) URL.revokeObjectURL(imageUrl)
    setImageUrl(kindFromFile(selected) === 'image' ? URL.createObjectURL(selected) : null)
    setDetail(`${kindFromFile(selected) === 'glb' ? 'GLB' : '图片'}将作为只读参考证据；它不会成为可编辑模型。`)
  }

  const toggleMissingView = (view: string) => {
    if (status === 'saving_evidence' || status === 'building_preview') return
    setMissingViews((current) => current.includes(view) ? current.filter((item) => item !== view) : [...current, view])
  }

  const saveEvidence = async () => {
    if (!target || !file || status === 'saving_evidence' || status === 'building_preview') return
    const validation = validateFile(file)
    if (validation) {
      setStatus('failed')
      setDetail(validation)
      return
    }
    if (!sourceStatement.trim() || !licenseStatement.trim()) {
      setStatus('failed')
      setDetail('请说明参考来源和使用授权/权利声明；没有声明不会上传。')
      return
    }
    const token = ++requestTokenRef.current
    setStatus('saving_evidence')
    setDetail('正在保存只读参考证据与来源声明…')
    const result = await adapter.createEvidence({
      target,
      file,
      sourceStatement: sourceStatement.trim(),
      licenseStatement: licenseStatement.trim(),
      missingViews,
      referenceClass: selectedKind === 'image' ? referenceClass : null,
      notes: notes.trim(),
    }).catch((caught) => ({
      status: 'failed' as const,
      message: caught instanceof Error ? caught.message : '保存参考证据失败；当前设计没有变化。',
    }))
    if (token !== requestTokenRef.current) return
    if (result.status !== 'created') {
      setStatus('failed')
      setDetail(result.message)
      onMessage?.(result.message)
      return
    }
    setEvidence(result.evidence)
    setLineage(null)
    setStatus('evidence_ready')
    const uncertainty = result.evidence.uncertainties.length > 0
      ? `不确定性：${result.evidence.uncertainties.join('；')}`
      : '未声明额外不确定性。'
    setDetail(`已保存只读证据（${result.evidence.contentSha256.slice(0, 12)}…）。${uncertainty}`)
    onMessage?.('已保存参考证据；它仅用于引导重建，不会被直接编辑。')
    if (result.evidence.kind === 'image' && !imageUrl && adapter.loadContent) {
      void adapter.loadContent(target, result.evidence).then((blob) => {
        if (token !== requestTokenRef.current) return
        setImageUrl(URL.createObjectURL(blob))
      }).catch(() => undefined)
    }
  }

  const buildPreview = async () => {
    if (!target || !evidence || status === 'building_preview') return
    const token = ++requestTokenRef.current
    setStatus('building_preview')
    setDetail('正在依据可见轮廓、比例和材质线索生成受限重建预览…')
    const result = await adapter.previewRebuild(target, evidence).catch((caught): ReferenceRebuildPreviewResponse => ({
      status: 'failed',
      message: caught instanceof Error ? caught.message : '参考引导重建预览失败；当前设计没有变化。',
    }))
    if (token !== requestTokenRef.current) return
    if (result.status !== 'preview_ready') {
      setStatus('failed')
      setDetail(result.message)
      onMessage?.(result.message)
      return
    }
    const previewLineage = readReferenceRebuildPreviewLineage(result)
    if (!previewLineage) {
      const message = '参考重建预览缺少可验证的冻结证据谱系；当前设计没有变化。'
      setStatus('failed')
      setDetail(message)
      onMessage?.(message)
      return
    }
    changeSetIdRef.current = result.changeSetId
    previewBaseAssetVersionIdRef.current = target.baseAssetVersionId
    setComparison(result.comparison ?? null)
    setLineage(previewLineage)
    setStatus('preview_ready')
    setDetail(result.summary)
    onMessage?.(result.summary)
  }

  const retain = async () => {
    const changeSetId = changeSetIdRef.current
    if (!changeSetId || (status !== 'preview_ready' && status !== 'cancel_failed')) return
    if (isReferencePreviewBaseStale(previewBaseAssetVersionIdRef.current, target?.baseAssetVersionId ?? null)) {
      setStatus('preview_stale')
      setDetail('当前设计已在别处推进；旧参考预览不能确认。请取消该预览后重新生成。')
      return
    }
    const retainAttempt = ++retainAttemptRef.current
    setStatus('building_preview')
    setDetail('正在确认参考引导重建…')
    const result = await adapter.retain(changeSetId).catch((caught): ReferenceRebuildRetainResponse => ({
      status: 'failed',
      message: caught instanceof Error ? caught.message : '确认参考引导重建失败；当前版本未被覆盖。',
    }))
    if (retainAttempt !== retainAttemptRef.current) return
    if (result.status !== 'retained') {
      setStatus('failed')
      setDetail(result.message)
      onMessage?.(result.message)
      return
    }
    const retainedLineage = readReferenceRebuildRetainLineage(result)
    if (!retainedLineage) {
      const message = '参考重建已返回，但缺少可验证的结果 GLB 谱系；请重新打开项目核对。'
      setStatus('failed')
      setDetail(message)
      onMessage?.(message)
      return
    }
    changeSetIdRef.current = null
    previewBaseAssetVersionIdRef.current = null
    setStatus('evidence_ready')
    // A preview identity must not be relabelled as confirmed without the
    // post-confirm frozen result pair from Rust.
    setLineage(retainedLineage)
    setDetail(result.summary)
    onMessage?.(result.summary)
  }

  const selectHistoryEntry = (entry: ReferenceEvidenceHistoryEntry) => {
    if (!target) return
    setEvidence(entry.evidence)
    setComparison(entry.comparison)
    setLineage(entry.lineage && isReferenceRebuildExactLineage(entry.lineage) ? entry.lineage : null)
    setStatus('evidence_ready')
    setDetail('已恢复此项目保存的参考证据与重建对比记录。')
    if (entry.evidence.kind !== 'image' || !adapter.loadContent) return
    const token = ++requestTokenRef.current
    void adapter.loadContent(target, entry.evidence).then((blob) => {
      if (token !== requestTokenRef.current) return
      if (imageUrl) URL.revokeObjectURL(imageUrl)
      setImageUrl(URL.createObjectURL(blob))
    }).catch(() => {
      if (token === requestTokenRef.current) setDetail('已恢复证据记录，但参考图片暂时无法读取。')
    })
  }

  const viewReferenceGlb = async () => {
    if (!target || !evidence || evidence.kind !== 'glb' || !adapter.viewReferenceGlb) return
    const result = await adapter.viewReferenceGlb(target, evidence)
    setDetail(result.message)
    if (result.status !== 'ready') onMessage?.(result.message)
  }

  const viewReferenceImage = async () => {
    if (!target || !evidence || evidence.kind !== 'image' || !adapter.viewReferenceImage) return
    const result = await adapter.viewReferenceImage(target, evidence)
    setDetail(result.message)
    if (result.status !== 'ready') onMessage?.(result.message)
  }

  const processing = status === 'saving_evidence' || status === 'building_preview' || status === 'cancelling'
  const hasActivePreview = status === 'preview_ready' || status === 'preview_stale' || status === 'cancelling' || status === 'cancel_failed'
  const selectedKind = file ? kindFromFile(file) : null
  const rebuildRequiresEditableBase = Boolean(evidence) && !target?.baseAssetVersionId
  return (
    <section ref={dialogRef} className="reference-evidence-drawer" role="dialog" aria-modal="true" aria-label="添加参考证据" onKeyDown={onKeyDown}>
      <header>
        <div>
          <strong>参考图 / GLB 引导重建</strong>
          <small>仅提取可见轮廓、比例、部件和材质线索；不会复制隐藏结构、尺寸或功能。</small>
        </div>
        <button ref={closeButtonRef} type="button" aria-label="关闭参考证据" disabled={status === 'cancelling'} onClick={close}>{status === 'cancelling' ? '正在取消…' : '关闭'}</button>
      </header>
      {!target ? (
        <p className="reference-evidence-status failed" role="status">请先创建或打开一个设计项目。</p>
      ) : (
        <>
          <p className="reference-evidence-boundary"><strong>只读证据边界</strong><span>参考文件进入受限对象库；重建结果会是新的可编辑版本。</span></p>
          {historyLoading && <p className="reference-evidence-status" role="status">正在恢复此项目的参考证据…</p>}
          {history.length > 0 && (
            <section className="reference-evidence-history" aria-label="已保存的参考证据">
              <strong>已保存的参考证据</strong>
              <div>
                {history.map((entry) => (
                  <button key={entry.evidence.evidenceId} type="button" onClick={() => selectHistoryEntry(entry)}>
                    {entry.evidence.kind === 'glb' ? 'GLB' : '图片'} · {entry.evidence.fileName}
                  </button>
                ))}
              </div>
            </section>
          )}
          <label className="reference-evidence-file">
            <span>授权参考文件</span>
            <input ref={fileInputRef} type="file" accept={`${IMAGE_ACCEPT},${GLB_ACCEPT}`} disabled={processing || hasActivePreview} onChange={selectFile} />
            <small>{file ? `${selectedKind === 'glb' ? 'GLB' : '图片'} · ${file.name} · ${(file.size / 1024 / 1024).toFixed(2)} MB` : 'PNG/JPEG/WebP（≤16 MB）或 GLB（≤32 MB）'}</small>
          </label>
          <label className="reference-evidence-field">
            <span>来源说明</span>
            <textarea value={sourceStatement} onChange={(event) => setSourceStatement(event.target.value)} disabled={processing || Boolean(evidence)} placeholder="例如：本人制作并上传；或已获授权的项目资料。" rows={2} />
          </label>
          <label className="reference-evidence-field">
            <span>使用授权 / 权利声明</span>
            <textarea value={licenseStatement} onChange={(event) => setLicenseStatement(event.target.value)} disabled={processing || Boolean(evidence)} placeholder="例如：本人拥有使用权，仅用于本项目概念重建。" rows={2} />
          </label>
          <fieldset className="reference-evidence-views" disabled={processing || Boolean(evidence)}>
            <legend>缺失视角（可选）</legend>
            {VIEW_OPTIONS.map(([view, label]) => (
              <label key={view}><input type="checkbox" checked={missingViews.includes(view)} onChange={() => toggleMissingView(view)} />{label}</label>
            ))}
          </fieldset>
          {selectedKind === 'image' && (
            <fieldset className="reference-evidence-views" disabled={processing || Boolean(evidence)}>
              <legend>图片覆盖范围</legend>
              <label><input type="radio" name="reference-class" checked={referenceClass === 'single_image'} onChange={() => setReferenceClass('single_image')} />单张/有限视角</label>
              <label><input type="radio" name="reference-class" checked={referenceClass === 'multi_view_contact_sheet'} onChange={() => setReferenceClass('multi_view_contact_sheet')} />多视图联系表</label>
              <small>这是一项用户声明；即使是多视图，也不会推断隐藏结构、精确尺寸或工程信息。</small>
            </fieldset>
          )}
          <label className="reference-evidence-field">
            <span>可见线索或不确定性（可选）</span>
            <textarea value={notes} onChange={(event) => setNotes(event.target.value)} disabled={processing || Boolean(evidence)} placeholder="只写可见外观线索；未知或遮挡部分请明确说明。" rows={2} />
          </label>
          {!evidence ? (
            <button type="button" className="reference-evidence-primary" disabled={!file || processing} onClick={() => void saveEvidence()}>{status === 'saving_evidence' ? '正在保存证据…' : '保存只读参考证据'}</button>
          ) : !hasActivePreview ? (
            <>
              <button type="button" className="reference-evidence-primary" disabled={processing || rebuildRequiresEditableBase} onClick={() => void buildPreview()}>{status === 'building_preview' ? '正在生成预览…' : '生成受限重建预览'}</button>
              {rebuildRequiresEditableBase && (
                <p className="reference-evidence-status" role="status">证据已保存。请先生成并确认机械臂生产基准，再使用参考重建。</p>
              )}
            </>
          ) : (
            <div className="reference-evidence-actions">
              <button type="button" disabled={status === 'cancelling'} onClick={close}>{status === 'cancelling' ? '正在取消…' : status === 'cancel_failed' ? '重试取消' : '取消'}</button>
              <button type="button" className="reference-evidence-primary" disabled={status === 'cancelling' || status === 'preview_stale'} onClick={() => void retain()}>保留新版本</button>
            </div>
          )}
          {evidence && <ReferenceRebuildComparison evidence={evidence} comparison={comparison} />}
          {lineage && <ReferenceRebuildLineage lineage={lineage} />}
          {evidence?.kind === 'image' && imageUrl && (
            <>
              <figure className="reference-evidence-image" aria-label="只读参考图片">
                <img src={imageUrl} alt="只读参考图片" />
                <figcaption>参考图片只用于可见外观对比；不成为几何真值。</figcaption>
              </figure>
              <div className="reference-evidence-viewport-actions">
                <button type="button" onClick={() => void viewReferenceImage()} disabled={!adapter.viewReferenceImage}>在同一 3D 视口查看参考图片</button>
                <button
                  type="button"
                  onClick={() => adapter.viewResult?.(target, history.find((entry) => entry.evidence.evidenceId === evidence.evidenceId) ?? {
                    evidence, comparison, rebuildPlanId: null, resultAssetVersionId: null,
                  })}
                  disabled={!adapter.viewResult}
                >返回重建结果</button>
              </div>
            </>
          )}
          {evidence?.kind === 'glb' && (
            <div className="reference-evidence-viewport-actions">
              <button type="button" onClick={() => void viewReferenceGlb()} disabled={!adapter.viewReferenceGlb}>在同一 3D 视口查看参考 GLB</button>
              <button
                type="button"
                onClick={() => adapter.viewResult?.(target, history.find((entry) => entry.evidence.evidenceId === evidence.evidenceId) ?? {
                  evidence, comparison, rebuildPlanId: null, resultAssetVersionId: null,
                })}
                disabled={!adapter.viewResult}
              >返回重建结果</button>
            </div>
          )}
        </>
      )}
      {detail && <p className={`reference-evidence-status ${status === 'failed' || status === 'cancel_failed' ? 'failed' : ''}`} role="status" aria-live="polite">{detail}</p>}
    </section>
  )
}
