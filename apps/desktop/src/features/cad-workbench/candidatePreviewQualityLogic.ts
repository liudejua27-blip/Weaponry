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

export function deriveCandidatePreviewQuality(items: AgentItem[]): CandidatePreviewQuality | null {
  const evaluation = latestEvaluation(items)
  if (!evaluation) return null

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
  const macroSummary = macro === null ? '尚未返回轮廓比对摘要' : `参考摘要 ${formatBasisPoints(macro)}`
  const formSummary = meso === null ? '尚未返回形体比对摘要' : `参考摘要 ${formatBasisPoints(meso)}`
  const surfaceSummary = micro === null ? '尚未返回表面摘要' : `参考摘要 ${formatBasisPoints(micro)}`
  const warnings: string[] = []

  if ((macro !== null && macro < SIMILARITY_MINIMUMS.macro) || (failureCodeFlags & FAILURE_CODE_MASKS[FAILURE_CODE_PATTERNS.macro]) !== 0) {
    warnings.push(`参考相似度不足：轮廓 ${formatBasisPoints(macro)}（目标至少 ${formatBasisPoints(SIMILARITY_MINIMUMS.macro)}）`)
  }
  if ((meso !== null && meso < SIMILARITY_MINIMUMS.meso) || (failureCodeFlags & FAILURE_CODE_MASKS[FAILURE_CODE_PATTERNS.meso]) !== 0) {
    warnings.push(`参考相似度不足：形体 ${formatBasisPoints(meso)}（目标至少 ${formatBasisPoints(SIMILARITY_MINIMUMS.meso)}）`)
  }
  if ((micro !== null && micro < SIMILARITY_MINIMUMS.micro) || (failureCodeFlags & FAILURE_CODE_MASKS[FAILURE_CODE_PATTERNS.micro]) !== 0) {
    warnings.push(`参考相似度不足：表面细节 ${formatBasisPoints(micro)}（目标至少 ${formatBasisPoints(SIMILARITY_MINIMUMS.micro)}）`)
  }

  const incompleteDetail = (detailCoverage !== null && (
    isZero(detailCoverage.macro_bound)
    || isZero(detailCoverage.meso_bound)
    || isZero(detailCoverage.micro_bound)
    || positive(detailCoverage.critical_unresolved)
  )) || (failureCodeFlags & FAILURE_CODE_MASKS[FAILURE_CODE_PATTERNS.detailIncomplete]) !== 0
    || (failureCodeFlags & FAILURE_CODE_MASKS[FAILURE_CODE_PATTERNS.criticalUnresolved]) !== 0
  if (incompleteDetail) {
    warnings.push('细节覆盖不足：仍有宏观/中频/微观细节未绑定或未解决；候选保留并继续修复。')
  }
  if (readback?.pbr_channels_complete === false) {
    warnings.push('材质通道未完整回读；这只是质量提示，不阻止可加载候选预览。')
  }
  if (readback?.surface_provenance_present === false) {
    warnings.push('表面来源尚未完整回读；这只是质量提示，不阻止可加载候选预览。')
  }
  if (hardGate === 'warning' && warnings.length === 0) {
    warnings.push('Rust 质量门尚未通过；候选仍可继续修复，不能确认保存。')
  }

  return {
    stages: [
      stage('silhouette', scoreStatus(macro, SIMILARITY_MINIMUMS.macro), macroSummary),
      booleanStage('structure', readback?.closed_manifold, '结构回读通过', '结构回读有提示', '尚未返回结构回读摘要'),
      stage('form', scoreStatus(meso, SIMILARITY_MINIMUMS.meso), formSummary),
      booleanStage('material', readback?.pbr_channels_complete, 'PBR 通道已回读', 'PBR 通道不完整', '尚未返回材质回读摘要'),
      micro === null
        ? booleanStage('surface', readback?.surface_provenance_present, '表面来源已回读', '表面来源有提示', '尚未返回表面摘要')
        : stage('surface', scoreStatus(micro, SIMILARITY_MINIMUMS.micro), surfaceSummary),
      stage('lighting', fixedViewCount === null
        ? 'pending'
        : fixedViewCount >= 8 ? 'ready' : 'warning', fixedViewCount === null ? '尚未返回固定视图摘要' : `已记录 ${fixedViewCount} 个固定视图`),
      stage('inspection', hardGate === 'passed' ? 'ready' : hardGate === 'warning' ? 'warning' : 'pending', hardGate === 'passed' ? 'Rust 硬门摘要已通过' : hardGate === 'warning' ? '质量门有提示，继续修复' : '尚未返回质量门摘要'),
    ],
    warnings,
    hardGate,
    referenceComparisonRecorded: comparison !== null,
  }
}

function latestEvaluation(items: AgentItem[]): RecordValue | null {
  for (let index = items.length - 1; index >= 0; index -= 1) {
    const item = items[index]
    if (item.item_type !== 'tool_result') continue
    const toolName = stringValue(item.payload.tool_name)
    if (toolName !== 'evaluate_candidate') continue
    const direct = firstRecord(item.payload.result)
    if (direct) return direct
    const toolResult = firstRecord(item.payload.tool_result)
    const validated = firstRecord(toolResult?.validated_output)
    const value = firstRecord(validated?.value)
    if (value) return value
  }
  return null
}

function stage(id: CandidatePreviewStage['id'], status: CandidatePreviewStageStatus, detail: string): CandidatePreviewStage {
  return { id, label: STAGE_LABELS[id], status, detail }
}

function scoreStatus(score: number | null, minimum: number): CandidatePreviewStageStatus {
  if (score === null) return 'pending'
  return score >= minimum ? 'ready' : 'warning'
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

function formatBasisPoints(value: number | null): string {
  return value === null ? '未提供' : `${(value / 100).toFixed(1)}%`
}
