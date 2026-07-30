import type { ReactElement } from 'react'
import type { CandidatePreviewQuality } from './candidatePreviewQualityLogic.js'

const EMPTY_NODES: ReadonlyArray<ReactElement> = []

type CachedCandidatePreviewQualityNodes = {
  stageNodes: readonly ReactElement[]
  warningNodes: readonly ReactElement[]
  hasWarnings: boolean
  hasStages: boolean
}

type CachedCandidatePreviewQualityPresentation = {
  builtNodes?: CachedCandidatePreviewQualityNodes
  present?: CandidatePreviewQualityPresentation
}

const STAGE_STATUS_CLASS: Record<CandidatePreviewQuality['stages'][number]['status'], string> = {
  pending: 'status-pending',
  ready: 'status-ready',
  warning: 'status-warning',
}

const CANDIDATE_PREVIEW_TEXT = {
  present: {
    header: '当前唯一候选 · 同一 3D 视口',
    summary: '候选工件已返回；只有结构失败会阻止预览。',
    pending: '可核验摘要尚未返回；不把候选工件显示为质量通过。',
  },
  absent: {
    header: '候选预览核验摘要',
    summary: '正在等待候选工件进入视口。',
    pending: '可核验摘要尚未返回；不把候选工件显示为质量通过。',
  },
} as const

const CANDIDATE_PREVIEW_READY_STATUS = '核验中'
const CANDIDATE_PREVIEW_WARNING_STATUS = '有质量提示'
const CANDIDATE_PREVIEW_PENDING_STATUS = '核验中'

const CANDIDATE_PREVIEW_PRESENT_TEXT = CANDIDATE_PREVIEW_TEXT.present
const CANDIDATE_PREVIEW_ABSENT_TEXT = CANDIDATE_PREVIEW_TEXT.absent

const CANDIDATE_PREVIEW_PRESENT_STRINGS = {
  headerText: CANDIDATE_PREVIEW_PRESENT_TEXT.header,
  summaryText: CANDIDATE_PREVIEW_PRESENT_TEXT.summary,
  pendingText: CANDIDATE_PREVIEW_PRESENT_TEXT.pending,
} as const

const CANDIDATE_PREVIEW_ABSENT_STRINGS = {
  statusText: CANDIDATE_PREVIEW_PENDING_STATUS,
  headerText: CANDIDATE_PREVIEW_ABSENT_TEXT.header,
  summaryText: CANDIDATE_PREVIEW_ABSENT_TEXT.summary,
  pendingText: CANDIDATE_PREVIEW_ABSENT_TEXT.pending,
} as const

const SINGLE_WARNING_NODE_CACHE = new Map<string, readonly ReactElement[]>()

const CANDIDATE_PREVIEW_QUALITY_PRESENTATION_CACHE = new WeakMap<
  CandidatePreviewQuality,
  CachedCandidatePreviewQualityPresentation
>()

export type CandidatePreviewQualityPresentation = {
  hasWarnings: boolean
  hasStages: boolean
  statusText: string
  headerText: string
  summaryText: string
  pendingText: string
  stageNodes: readonly ReactElement[]
  warningNodes: readonly ReactElement[]
}

export const EMPTY_PRESENTATION_WITHOUT_CANDIDATE: CandidatePreviewQualityPresentation = {
  hasWarnings: false,
  hasStages: false,
  ...CANDIDATE_PREVIEW_ABSENT_STRINGS,
  stageNodes: EMPTY_NODES,
  warningNodes: EMPTY_NODES,
}

export const EMPTY_PRESENTATION_WITH_CANDIDATE: CandidatePreviewQualityPresentation = {
  hasWarnings: false,
  hasStages: false,
  ...CANDIDATE_PREVIEW_PRESENT_STRINGS,
  statusText: CANDIDATE_PREVIEW_PENDING_STATUS,
  stageNodes: EMPTY_NODES,
  warningNodes: EMPTY_NODES,
}

function buildCandidatePreviewQualityPresentVariant(
  nodes: CachedCandidatePreviewQualityNodes,
): CandidatePreviewQualityPresentation {
  if (!nodes.hasStages) {
    const statusText = CANDIDATE_PREVIEW_PENDING_STATUS
    return {
      hasWarnings: nodes.hasWarnings,
      hasStages: false,
      statusText,
      ...CANDIDATE_PREVIEW_PRESENT_STRINGS,
      stageNodes: nodes.stageNodes,
      warningNodes: nodes.warningNodes,
    }
  }

  const statusText = nodes.hasWarnings ? CANDIDATE_PREVIEW_WARNING_STATUS : CANDIDATE_PREVIEW_READY_STATUS
  return {
    hasWarnings: nodes.hasWarnings,
    hasStages: nodes.hasStages,
    statusText,
    ...CANDIDATE_PREVIEW_PRESENT_STRINGS,
    stageNodes: nodes.stageNodes,
    warningNodes: nodes.warningNodes,
  }
}

export function buildCandidatePreviewQualityPresentation(
  candidatePresent: boolean,
  quality: CandidatePreviewQuality | null,
): CandidatePreviewQualityPresentation {
  if (!candidatePresent) {
    return EMPTY_PRESENTATION_WITHOUT_CANDIDATE
  }
  if (!quality) {
    return EMPTY_PRESENTATION_WITH_CANDIDATE
  }

  const cached = CANDIDATE_PREVIEW_QUALITY_PRESENTATION_CACHE.get(quality)

  if (cached?.present) return cached.present

  const builtNodes = cached?.builtNodes ?? buildQualityPresentationFromQuality(quality)
  const nextVariant = buildCandidatePreviewQualityPresentVariant(builtNodes)
  if (cached) {
    cached.builtNodes = builtNodes
    cached.present = nextVariant
    return nextVariant
  }

  const createdVariant: CachedCandidatePreviewQualityPresentation = {
    builtNodes,
    present: nextVariant,
  }
  CANDIDATE_PREVIEW_QUALITY_PRESENTATION_CACHE.set(quality, createdVariant)
  return nextVariant
}

function buildQualityPresentationFromQuality(
  quality: CandidatePreviewQuality,
): {
  stageNodes: readonly ReactElement[]
  warningNodes: readonly ReactElement[]
  hasWarnings: boolean
  hasStages: boolean
} {
  const stageNodes = buildQualityPresentationStageNodes(quality.stages)
  const warningNodes = buildQualityPresentationWarningNodes(quality.warnings)
  const hasWarnings = warningNodes.length > 0

  return {
    stageNodes,
    warningNodes,
    hasWarnings,
    hasStages: stageNodes.length > 0,
  }
}

function buildQualityPresentationStageNodes(
  stages: CandidatePreviewQuality['stages'],
): readonly ReactElement[] {
  const stageCount = stages.length
  if (stageCount === 0) return EMPTY_NODES

  const writableStageNodes = new Array<ReactElement>(stageCount)
  for (let index = 0; index < stageCount; index += 1) {
    const item = stages[index]
    writableStageNodes[index] = (
      <li key={item.id} className={STAGE_STATUS_CLASS[item.status]}>
        <span>{item.label}</span>
        <small>{item.detail}</small>
      </li>
    )
  }

  return writableStageNodes
}

function buildQualityPresentationWarningNodes(
  warnings: readonly string[],
): readonly ReactElement[] {
  const warningCount = warnings.length
  if (warningCount === 0) return EMPTY_NODES
  if (warningCount === 1) {
    const warning = warnings[0]
    if (!warning) return EMPTY_NODES
    const cached = SINGLE_WARNING_NODE_CACHE.get(warning)
    if (cached) return cached
    const createdNodes = [<li key={0}>{warning}</li>] as const
    SINGLE_WARNING_NODE_CACHE.set(warning, createdNodes)
    return createdNodes
  }

  const writableWarningNodes = new Array<ReactElement>(warningCount)
  for (let index = 0; index < warningCount; index += 1) {
    const warning = warnings[index]
    writableWarningNodes[index] = <li key={index}>{warning}</li>
  }

  return writableWarningNodes
}
