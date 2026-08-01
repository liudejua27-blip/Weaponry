import { memo } from 'react'
import { GenerationResultCard } from './GenerationResultCard'
import type { SingleResultDecisionPresentation, SingleResultReadyDecision } from './singleResultDecisionPresentationState'

type CadWorkbenchPanelResultCardsProps = {
  singleResultDecisionPresentation: SingleResultDecisionPresentation
  directionPreviewLoading: boolean
  conceptLoading: boolean
  previewError: boolean
  assistantNote: string
  showAdvancedControls: boolean
  onContinueEditing: () => void
  onRetrySingleResult: () => void
  onConfirmSingleResult: (decision: SingleResultReadyDecision) => void
  onRetryCandidatePreview: () => void
  showCompatibilityResultCard: boolean
  compatibilitySummary: string
  compatibilityVersionLabel: string
  decisionContractFailure: boolean
  onSaveCompatibility: (() => void | Promise<void>) | null
}

export const CadWorkbenchPanelResultCards = memo(function CadWorkbenchPanelResultCards({
  singleResultDecisionPresentation,
  directionPreviewLoading,
  conceptLoading,
  previewError,
  assistantNote,
  showAdvancedControls,
  onContinueEditing,
  onRetrySingleResult,
  onConfirmSingleResult,
  onRetryCandidatePreview,
  showCompatibilityResultCard,
  compatibilitySummary,
  compatibilityVersionLabel,
  decisionContractFailure,
  onSaveCompatibility,
}: CadWorkbenchPanelResultCardsProps) {
  if (singleResultDecisionPresentation.state === 'processing') {
    return <GenerationResultCard
      state="processing"
      detail={singleResultDecisionPresentation.detail ?? assistantNote}
    />
  }

  if (singleResultDecisionPresentation.state === 'failed') {
    const error = decisionContractFailure && !singleResultDecisionPresentation.error.includes('正式的单一结果决策')
      ? `${singleResultDecisionPresentation.error}\nAgent 没有返回正式的单一结果决策；这次生成没有形成可用结果，当前设计没有变化。`
      : singleResultDecisionPresentation.error
    return <GenerationResultCard
      state="failed"
      error={error}
      onRetry={onRetrySingleResult}
    />
  }

  if (singleResultDecisionPresentation.state === 'ready') {
    return <GenerationResultCard
      state="ready"
      summary={singleResultDecisionPresentation.decision.summary}
      versionLabel="正式生成质量门已通过 · 确认前不会写入版本"
      onSave={() => onConfirmSingleResult(singleResultDecisionPresentation.decision)}
      onContinueEditing={onContinueEditing}
      compactMode={!showAdvancedControls}
    />
  }

  if (directionPreviewLoading || conceptLoading) {
    return <GenerationResultCard state="processing" detail={assistantNote} />
  }

  if (previewError) {
    return <GenerationResultCard
      state="failed"
      error="3D 构建或分件检查未通过；当前设计没有变化。"
      onRetry={onRetryCandidatePreview}
    />
  }

  if (showAdvancedControls && showCompatibilityResultCard) {
    return <GenerationResultCard
      state="compatibility_result"
      summary={compatibilitySummary}
      versionLabel={compatibilityVersionLabel}
      onSave={onSaveCompatibility === null ? undefined : onSaveCompatibility}
      onContinueEditing={onContinueEditing}
      compactMode={!showAdvancedControls}
    />
  }

  return <GenerationResultCard
    state="idle"
  />
})
