import { memo } from 'react'
import type { CandidatePreviewQualityPresentation } from './candidatePreviewQualityPresentation.js'

type CandidatePreviewQualityPanelProps = {
  presentation: CandidatePreviewQualityPresentation
}

const CANDIDATE_PREVIEW_WARNING_HINT = '请处理以上建议后重试，可继续观察下一轮候选。'
const CANDIDATE_PREVIEW_QUALITY_PANEL_CLASS = {
  clean: 'candidate-preview-quality',
  warning: 'candidate-preview-quality has-warnings',
} as const

export const CandidatePreviewQualityPanel = memo(function CandidatePreviewQualityPanel({
  presentation,
}: CandidatePreviewQualityPanelProps) {
  const {
    hasWarnings,
    hasStages,
    statusText,
    headerText,
    summaryText,
    pendingText,
    stageNodes,
    warningNodes,
  } = presentation

  const sectionClassName = hasWarnings
    ? CANDIDATE_PREVIEW_QUALITY_PANEL_CLASS.warning
    : CANDIDATE_PREVIEW_QUALITY_PANEL_CLASS.clean
  return (
    <section className={sectionClassName} aria-label="当前候选预览核验摘要" data-testid="candidate-preview-quality">
      <header>
        <div>
          <strong>{headerText}</strong>
          <small>{summaryText}</small>
        </div>
        <span>{statusText}</span>
      </header>
      {hasStages ? (
        <ol className="candidate-preview-quality-stages">{stageNodes}</ol>
      ) : (
        <small>{pendingText}</small>
      )}
      {hasWarnings && (
        <div className="candidate-preview-quality-warnings" role="status" aria-live="polite">
          <strong>继续修复</strong>
          <ul>{warningNodes}</ul>
          <small>{CANDIDATE_PREVIEW_WARNING_HINT}</small>
        </div>
      )}
    </section>
  )
}, (prev, next) => {
  const prevPresentation = prev.presentation
  const nextPresentation = next.presentation
  return prevPresentation === nextPresentation
})
