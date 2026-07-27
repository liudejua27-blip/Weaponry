import { memo, useMemo } from 'react'
import type { ReactElement } from 'react'
import type { CandidatePreviewQuality } from './candidatePreviewQualityLogic.js'

const EMPTY_NODES: ReactElement[] = []
const STATUS_TEXT = {
  hasWarnings: '有质量提示',
  pending: '核验中',
} as const
const SUMMARY_TEXT = {
  present: '候选工件已返回；只有结构失败会阻止预览。',
  absent: '正在等待候选工件进入视口。',
} as const
const PENDING_TEXT = '可核验摘要尚未返回；不把候选工件显示为质量通过。'
const WARNING_TEXT = '质量提示不会伪装成通过，也不会阻止可加载候选继续显示。'

type CandidatePreviewRenderData = {
  hasWarnings: boolean
  statusText: string
  summaryText: string
  stageNodes: ReactElement[]
  warningNodes: ReactElement[]
}

const EMPTY_CANDIDATE_RENDER_DATA: CandidatePreviewRenderData = {
  hasWarnings: false,
  statusText: STATUS_TEXT.pending,
  summaryText: SUMMARY_TEXT.present,
  stageNodes: EMPTY_NODES,
  warningNodes: EMPTY_NODES,
}

type CandidatePreviewQualityPanelProps = {
  candidatePresent: boolean
  quality: CandidatePreviewQuality | null
}

export const CandidatePreviewQualityPanel = memo(function CandidatePreviewQualityPanel({
  candidatePresent,
  quality,
}: CandidatePreviewQualityPanelProps) {
  const renderData = useMemo(() => {
    if (!candidatePresent && !quality) return null
    if (!quality) {
      return EMPTY_CANDIDATE_RENDER_DATA
    }

    const warnings = quality.warnings
    const stages = quality.stages
    const warningCount = warnings.length
    const stageCount = stages.length
    const summaryText = candidatePresent ? SUMMARY_TEXT.present : SUMMARY_TEXT.absent
    const stageNodes = stageCount === 0 ? EMPTY_NODES : new Array<ReactElement>(stageCount)
    for (let index = 0; stageCount > 0 && index < stageCount; index += 1) {
      const item = stages[index]!
      stageNodes[index] = (
        <li key={item.id} className={`status-${item.status}`}>
          <span>{item.label}</span>
          <small>{item.detail}</small>
        </li>
      )
    }

    if (warningCount === 0) {
      return {
        hasWarnings: false,
        statusText: STATUS_TEXT.pending,
        summaryText,
        stageNodes,
        warningNodes: EMPTY_NODES,
      }
    }

    const warningNodes = warningCount === 0 ? EMPTY_NODES : new Array<ReactElement>(warningCount)
    for (let index = 0; warningCount > 0 && index < warningCount; index += 1) {
      const warning = warnings[index]!
      warningNodes[index] = <li key={warning}>{warning}</li>
    }

    return {
      hasWarnings: true,
      statusText: STATUS_TEXT.hasWarnings,
      summaryText,
      stageNodes,
      warningNodes,
    }
  }, [candidatePresent, quality])

  if (!renderData) return null

  const {
    hasWarnings,
    statusText,
    summaryText,
    stageNodes,
    warningNodes,
  } = renderData
  return (
    <section className={`candidate-preview-quality ${hasWarnings ? 'has-warnings' : ''}`} aria-label="当前候选预览核验摘要" data-testid="candidate-preview-quality">
      <header>
        <div>
          <strong>{candidatePresent ? '当前唯一候选 · 同一 3D 视口' : '候选预览核验摘要'}</strong>
          <small>{summaryText}</small>
        </div>
        <span>{statusText}</span>
      </header>
      {stageNodes.length > 0 ? (
        <ol className="candidate-preview-quality-stages">{stageNodes}</ol>
      ) : (
        <small>{PENDING_TEXT}</small>
      )}
      {hasWarnings && (
        <div className="candidate-preview-quality-warnings" role="status" aria-live="polite">
          <strong>继续修复</strong>
          <ul>{warningNodes}</ul>
          <small>{WARNING_TEXT}</small>
        </div>
      )}
    </section>
  )
})
