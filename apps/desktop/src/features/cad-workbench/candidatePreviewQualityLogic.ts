import type { AgentItem } from '../../shared/types.js'

export type CandidatePreviewStageStatus = 'pending' | 'ready' | 'warning'

export type CandidatePreviewStage = {
  id: 'silhouette' | 'structure' | 'form' | 'material' | 'surface' | 'lighting' | 'inspection'
  label: string
  status: CandidatePreviewStageStatus
  detail: string
}

export type CandidatePreviewQuality = {
  stages: CandidatePreviewStage[]
  warnings: string[]
  hardGate: 'unknown' | 'passed' | 'warning'
  referenceComparisonRecorded: boolean
}

type RecordValue = Record<string, unknown>

export const STAGE_LABELS: Record<CandidatePreviewStage['id'], string> = {
  silhouette: '轮廓',
  structure: '结构',
  form: '形体',
  material: '材质',
  surface: '表面',
  lighting: '灯光',
  inspection: '检查',
}

const SIMILARITY_MINIMUMS = {
  macro: 6_500,
  meso: 5_500,
  micro: 4_500,
} as const

type SimilarityLabel = 0 | 1 | 2

const BASIS_POINTS_TEXT_CACHE = new Map<number, string>()

const SIMILARITY_LABEL_THRESHOLD_TEXTS: Record<SimilarityLabel, string> = {
  0: formatBasisPoints(SIMILARITY_MINIMUMS.macro),
  1: formatBasisPoints(SIMILARITY_MINIMUMS.meso),
  2: formatBasisPoints(SIMILARITY_MINIMUMS.micro),
}

const SIMILARITY_LABELS = {
  macro: 0 as const,
  meso: 1 as const,
  micro: 2 as const,
} as const

const SIMILARITY_LABEL_TEXTS = {
  [SIMILARITY_LABELS.macro]: '轮廓',
  [SIMILARITY_LABELS.meso]: '形体',
  [SIMILARITY_LABELS.micro]: '表面细节',
} as const

const SIMILARITY_MINIMUM_TEXT_BY_LABEL: Record<SimilarityLabel, number> = {
  [SIMILARITY_LABELS.macro]: SIMILARITY_MINIMUMS.macro,
  [SIMILARITY_LABELS.meso]: SIMILARITY_MINIMUMS.meso,
  [SIMILARITY_LABELS.micro]: SIMILARITY_MINIMUMS.micro,
}

const FAILURE_CODE_PATTERNS = {
  macro: 'REFERENCE_MACRO_MISMATCH',
  meso: 'REFERENCE_MESO_MISMATCH',
  micro: 'REFERENCE_MICRO_MISMATCH',
  detailIncomplete: 'DETAIL_LEVEL_COVERAGE_INCOMPLETE',
  criticalUnresolved: 'CRITICAL_DETAIL_UNRESOLVED',
} as const

const FAILURE_CODE_MASKS: Record<string, number> = {
  [FAILURE_CODE_PATTERNS.macro]: 1,
  [FAILURE_CODE_PATTERNS.meso]: 2,
  [FAILURE_CODE_PATTERNS.micro]: 4,
  [FAILURE_CODE_PATTERNS.detailIncomplete]: 8,
  [FAILURE_CODE_PATTERNS.criticalUnresolved]: 16,
} as const

const FAILURE_CODE_MASK_ALL = 31
const FAILURE_CODE_NONE = 0
type SimilarityWarningTextCacheByFailureCode = [string | undefined, string | undefined]
const SIMILARITY_WARNING_TEXT_CACHE = new Map<
  SimilarityLabel,
  Map<number, SimilarityWarningTextCacheByFailureCode>
>()
const REFERENCE_SUMMARY_TEXT_CACHE = new Map<number, string>()
const FIXED_VIEW_SUMMARY_TEXT_CACHE = new Map<number, string>()
const CANDIDATE_PREVIEW_QUALITY_CACHE = new WeakMap<RecordValue, CandidatePreviewQuality | null>()
const EMPTY_QUALITY_WARNINGS: string[] = []
const MACRO_SUMMARY_PENDING_TEXT = '尚未返回轮廓比对摘要'
const FORM_SUMMARY_PENDING_TEXT = '尚未返回形体比对摘要'
const SURFACE_SUMMARY_PENDING_TEXT = '尚未返回表面摘要'
const LIGHTING_SUMMARY_PENDING_TEXT = '尚未返回固定视图摘要'
const INSPECTION_STATUS_PENDING_TEXT = '尚未返回质量门摘要'
const INSPECTION_STATUS_READY_TEXT = '质量检查已通过'
const INSPECTION_STATUS_WARNING_TEXT = '质量门有提示，继续修复'
const MATERIAL_CHANNEL_WARNING_TEXT = '材质通道未完整回读；这只是质量提示，不阻止可加载候选预览。'
const SURFACE_PROVENANCE_WARNING_TEXT = '表面来源尚未完整回读；这只是质量提示，不阻止可加载候选预览。'
const HARD_GATE_WARNING_TEXT = '质量检查尚未通过；可以继续修改，暂不能保存。'
const DETAIL_INCOMPLETE_WARNING_TEXT = '细节覆盖不足：仍有宏观/中频/微观细节未绑定或未解决；候选保留并继续修复。'
const WARNING_MASK_MACRO_SIMILARITY = 1
const WARNING_MASK_MESO_SIMILARITY = 2
const WARNING_MASK_SURFACE_SIMILARITY = 4
const WARNING_MASK_INCOMPLETE_DETAIL = 8
const WARNING_MASK_PBR_CHANNEL = 16
const WARNING_MASK_SURFACE_PROVENANCE = 32
const WARNING_MASK_HARD_GATE = 64

export function extractCandidatePreviewEvaluationFromItem(item: AgentItem): RecordValue | null {
  if (item.item_type !== 'tool_result') return null
  const toolName = stringValue(item.payload?.tool_name)
  if (toolName !== 'evaluate_candidate') return null
  const direct = firstRecord(item.payload?.result)
  if (direct) return direct
  const toolResult = firstRecord(item.payload?.tool_result)
  const validated = firstRecord(toolResult?.validated_output)
  return firstRecord(validated?.value)
}

export function findLatestCandidatePreviewEvaluationFromTail(
  items: readonly AgentItem[],
  startIndex = items.length - 1,
): RecordValue | null {
  for (let index = startIndex; index >= 0; index -= 1) {
    const item = items[index]
    const extracted = item ? extractCandidatePreviewEvaluationFromItem(item) : null
    if (extracted) return extracted
  }
  return null
}

export function findLatestCandidatePreviewEvaluation(items: readonly AgentItem[]): RecordValue | null {
  return findLatestCandidatePreviewEvaluationFromTail(items)
}

export function deriveCandidatePreviewQualityFromEvaluation(
  evaluation: RecordValue | null,
): CandidatePreviewQuality | null {
  if (!evaluation) return null

  const cached = CANDIDATE_PREVIEW_QUALITY_CACHE.get(evaluation)
  if (cached) return cached

  const comparison = firstRecord(evaluation.visual_reference_comparison_report)
  const convergence = firstRecord(evaluation.visual_convergence_report)
  const readback = firstRecord(
    convergence?.readback,
    convergence?.visual_glb_readback,
    evaluation.readback,
    evaluation.visual_glb_readback,
  )
  const detailCoverage = firstRecord(convergence?.detail_coverage)
  const failureCodeFlags = failureCodeFlagsFromUnknown(comparison?.failure_codes)
  const macro = firstNumber(comparison?.macro_similarity_bps, convergence?.macro_similarity_bps)
  const meso = firstNumber(comparison?.meso_similarity_bps, convergence?.meso_similarity_bps)
  const micro = firstNumber(comparison?.micro_similarity_bps, convergence?.micro_similarity_bps)
  const fixedViewCount = firstNumber(convergence?.fixed_view_count, evaluation.fixed_view_count)
  const hardGate = evaluation.hard_gate_passed === true
    ? 'passed'
    : evaluation.hard_gate_passed === false ? 'warning' : 'unknown'
  const macroSummary = formatReferenceSummary(macro, MACRO_SUMMARY_PENDING_TEXT)
  const formSummary = formatReferenceSummary(meso, FORM_SUMMARY_PENDING_TEXT)
  const surfaceSummary = formatReferenceSummary(micro, SURFACE_SUMMARY_PENDING_TEXT)
  const lightingStatus = fixedViewCount === null
    ? 'pending'
    : fixedViewCount >= 8
      ? 'ready'
      : 'warning'
  const lightingSummary = formatFixedViewSummary(fixedViewCount)
  const inspectionStatus = hardGate === 'passed'
    ? 'ready'
    : hardGate === 'warning'
      ? 'warning'
      : 'pending'
  const inspectionSummary = hardGate === 'passed'
    ? INSPECTION_STATUS_READY_TEXT
    : hardGate === 'warning'
      ? INSPECTION_STATUS_WARNING_TEXT
      : INSPECTION_STATUS_PENDING_TEXT
const macroSimilarityWarning = buildSimilarityWarning(
  SIMILARITY_LABELS.macro,
  macro,
  (failureCodeFlags & FAILURE_CODE_MASKS[FAILURE_CODE_PATTERNS.macro]) !== 0,
)
const mesoSimilarityWarning = buildSimilarityWarning(
  SIMILARITY_LABELS.meso,
  meso,
  (failureCodeFlags & FAILURE_CODE_MASKS[FAILURE_CODE_PATTERNS.meso]) !== 0,
)
const microSimilarityWarning = buildSimilarityWarning(
  SIMILARITY_LABELS.micro,
  micro,
  (failureCodeFlags & FAILURE_CODE_MASKS[FAILURE_CODE_PATTERNS.micro]) !== 0,
)
  const incompleteDetail = (detailCoverage !== null && (
    isZero(detailCoverage.macro_bound)
    || isZero(detailCoverage.meso_bound)
    || isZero(detailCoverage.micro_bound)
    || positive(detailCoverage.critical_unresolved)
  )) || (failureCodeFlags & FAILURE_CODE_MASKS[FAILURE_CODE_PATTERNS.detailIncomplete]) !== 0
    || (failureCodeFlags & FAILURE_CODE_MASKS[FAILURE_CODE_PATTERNS.criticalUnresolved]) !== 0
  const isMaterialChannelMissing = readback?.pbr_channels_complete === false
  const isSurfaceProvenanceMissing = readback?.surface_provenance_present === false

  let warningMask = 0
  let warningCount = 0

  if (macroSimilarityWarning) {
    warningMask |= WARNING_MASK_MACRO_SIMILARITY
    warningCount += 1
  }
  if (mesoSimilarityWarning) {
    warningMask |= WARNING_MASK_MESO_SIMILARITY
    warningCount += 1
  }
  if (microSimilarityWarning) {
    warningMask |= WARNING_MASK_SURFACE_SIMILARITY
    warningCount += 1
  }
  if (incompleteDetail) {
    warningMask |= WARNING_MASK_INCOMPLETE_DETAIL
    warningCount += 1
  }
  if (isMaterialChannelMissing) {
    warningMask |= WARNING_MASK_PBR_CHANNEL
    warningCount += 1
  }
  if (isSurfaceProvenanceMissing) {
    warningMask |= WARNING_MASK_SURFACE_PROVENANCE
    warningCount += 1
  }
  if (hardGate === 'warning' && warningCount === 0) {
    warningMask |= WARNING_MASK_HARD_GATE
    warningCount += 1
  }

  const warnings: string[] = warningCount === 0 ? EMPTY_QUALITY_WARNINGS : new Array<string>(warningCount)
  let warningIndex = 0

  if (warningMask & WARNING_MASK_MACRO_SIMILARITY) {
    warnings[warningIndex] = macroSimilarityWarning!
    warningIndex += 1
  }
  if (warningMask & WARNING_MASK_MESO_SIMILARITY) {
    warnings[warningIndex] = mesoSimilarityWarning!
    warningIndex += 1
  }
  if (warningMask & WARNING_MASK_SURFACE_SIMILARITY) {
    warnings[warningIndex] = microSimilarityWarning!
    warningIndex += 1
  }
  if (warningMask & WARNING_MASK_INCOMPLETE_DETAIL) {
    warnings[warningIndex] = DETAIL_INCOMPLETE_WARNING_TEXT
    warningIndex += 1
  }
  if (warningMask & WARNING_MASK_PBR_CHANNEL) {
    warnings[warningIndex] = MATERIAL_CHANNEL_WARNING_TEXT
    warningIndex += 1
  }
  if (warningMask & WARNING_MASK_SURFACE_PROVENANCE) {
    warnings[warningIndex] = SURFACE_PROVENANCE_WARNING_TEXT
    warningIndex += 1
  }
  if (warningMask & WARNING_MASK_HARD_GATE) {
    warnings[warningIndex] = HARD_GATE_WARNING_TEXT
  }

  const result: CandidatePreviewQuality = {
    stages: [
      stage('silhouette', scoreStatus(macro, SIMILARITY_MINIMUMS.macro), macroSummary),
      booleanStage('structure', readback?.closed_manifold, '结构回读通过', '结构回读有提示', '尚未返回结构回读摘要'),
      stage('form', scoreStatus(meso, SIMILARITY_MINIMUMS.meso), formSummary),
      booleanStage('material', readback?.pbr_channels_complete, 'PBR 通道已回读', 'PBR 通道不完整', '尚未返回材质回读摘要'),
      micro === null
        ? booleanStage('surface', readback?.surface_provenance_present, '表面来源已回读', '表面来源有提示', '尚未返回表面摘要')
        : stage('surface', scoreStatus(micro, SIMILARITY_MINIMUMS.micro), surfaceSummary),
      stage('lighting', lightingStatus, lightingSummary),
      stage('inspection', inspectionStatus, inspectionSummary),
    ],
    warnings,
    hardGate,
    referenceComparisonRecorded: comparison !== null,
  }

  CANDIDATE_PREVIEW_QUALITY_CACHE.set(evaluation, result)
  return result
}

export function deriveCandidatePreviewQuality(items: readonly AgentItem[]): CandidatePreviewQuality | null {
  return deriveCandidatePreviewQualityFromEvaluation(findLatestCandidatePreviewEvaluation(items))
}

function formatBasisPoints(value: number | null): string {
  if (value === null) return '未提供'
  const tenthsValue = Math.round(value)
  const cached = BASIS_POINTS_TEXT_CACHE.get(tenthsValue)
  if (cached) return cached
  const rendered = `${(tenthsValue / 100).toFixed(1)}%`
  BASIS_POINTS_TEXT_CACHE.set(tenthsValue, rendered)
  return rendered
}

function formatReferenceSummary(value: number | null, pendingText: string): string {
  if (value === null) return pendingText
  const normalized = Math.round(value)
  const cached = REFERENCE_SUMMARY_TEXT_CACHE.get(normalized)
  if (cached) return cached
  const rendered = `参考摘要 ${formatBasisPoints(normalized)}`
  REFERENCE_SUMMARY_TEXT_CACHE.set(normalized, rendered)
  return rendered
}

function formatFixedViewSummary(value: number | null): string {
  if (value === null) return LIGHTING_SUMMARY_PENDING_TEXT
  const normalized = Math.round(value)
  const cached = FIXED_VIEW_SUMMARY_TEXT_CACHE.get(normalized)
  if (cached) return cached
  const rendered = `已记录 ${normalized} 个固定视图`
  FIXED_VIEW_SUMMARY_TEXT_CACHE.set(normalized, rendered)
  return rendered
}

function stage(id: CandidatePreviewStage['id'], status: CandidatePreviewStageStatus, detail: string): CandidatePreviewStage {
  return { id, label: STAGE_LABELS[id], status, detail }
}

function scoreStatus(score: number | null, minimum: number): CandidatePreviewStageStatus {
  if (score === null) return 'pending'
  return score >= minimum ? 'ready' : 'warning'
}

function buildSimilarityWarning(
  label: SimilarityLabel,
  score: number | null,
  hasFailureCode: boolean,
): string | null {
  const minimum = SIMILARITY_MINIMUM_TEXT_BY_LABEL[label]
  if (!hasFailureCode && (score === null || score >= minimum)) return null
  const normalized = score === null ? -1 : Math.round(score)
  const minimumText = SIMILARITY_LABEL_THRESHOLD_TEXTS[label]
  let labelBucket = SIMILARITY_WARNING_TEXT_CACHE.get(label)
  if (!labelBucket) {
    labelBucket = new Map()
    SIMILARITY_WARNING_TEXT_CACHE.set(label, labelBucket)
  }
  const failureBucket = labelBucket.get(normalized)
  const failureKey = hasFailureCode ? 1 : 0
  const cached = failureBucket?.[failureKey]
  if (cached !== undefined) return cached

  let nextFailureBucket = failureBucket
  if (!nextFailureBucket) {
    nextFailureBucket = [undefined, undefined]
    labelBucket.set(normalized, nextFailureBucket)
  }
  const labelText = SIMILARITY_LABEL_TEXTS[label]
  const rendered = `参考相似度不足：${labelText} ${normalized === -1 ? '未提供' : formatBasisPoints(normalized)}（目标至少 ${minimumText}）`
  nextFailureBucket[failureKey] = rendered
  return rendered
}

function booleanStage(
  id: CandidatePreviewStage['id'],
  value: unknown,
  ready: string,
  warning: string,
  pending: string,
): CandidatePreviewStage {
  return stage(id, value === true ? 'ready' : value === false ? 'warning' : 'pending', value === true ? ready : value === false ? warning : pending)
}

function firstRecord(...values: unknown[]): RecordValue | null {
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index]
    if (value !== null && typeof value === 'object' && !Array.isArray(value)) return value as RecordValue
  }
  return null
}

function firstNumber(...values: unknown[]): number | null {
  for (let index = 0; index < values.length; index += 1) {
    const candidate = values[index]
    if (typeof candidate === 'number' && Number.isFinite(candidate)) return candidate
  }
  return null
}

function failureCodeFlagsFromUnknown(value: unknown): number {
  if (!Array.isArray(value)) return FAILURE_CODE_NONE
  let mask = FAILURE_CODE_NONE
  for (let index = 0; index < value.length; index += 1) {
    const item = value[index]
    if (typeof item !== 'string' || item.length === 0) continue
    const flag = FAILURE_CODE_MASKS[item]
    if (flag === undefined) continue
    mask |= flag
    if (mask === FAILURE_CODE_MASK_ALL) return mask
  }
  return mask
}

function stringValue(value: unknown): string | null {
  return typeof value === 'string' ? value : null
}

function isZero(value: unknown): boolean {
  return typeof value === 'number' && Number.isFinite(value) && value === 0
}

function positive(value: unknown): boolean {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
}
