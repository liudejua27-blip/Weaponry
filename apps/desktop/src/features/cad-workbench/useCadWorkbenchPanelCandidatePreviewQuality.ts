import { useMemo } from 'react'
import type { AgentItem } from '../../shared/types.js'
import {
  deriveCandidatePreviewQualityFromEvaluation,
  extractCandidatePreviewEvaluationFromItem,
  findLatestCandidatePreviewEvaluationFromTail,
} from './candidatePreviewQualityLogic.js'
import type { CandidatePreviewQuality } from './candidatePreviewQualityLogic.js'

type UseCadWorkbenchPanelCandidatePreviewQualityInput = {
  candidatePreviewPresent: boolean
  agentKernelItems: readonly AgentItem[]
}

export function useCadWorkbenchPanelCandidatePreviewQuality({
  candidatePreviewPresent,
  agentKernelItems,
}: UseCadWorkbenchPanelCandidatePreviewQualityInput): CandidatePreviewQuality | null {
  return useMemo(() => {
    if (!candidatePreviewPresent || agentKernelItems.length === 0) return null
    const tailIndex = agentKernelItems.length - 1
    const candidateKernelTailItem = agentKernelItems[tailIndex]
    if (!candidateKernelTailItem) return null
    const directTailEvaluation = extractCandidatePreviewEvaluationFromItem(candidateKernelTailItem)
    if (directTailEvaluation) return deriveCandidatePreviewQualityFromEvaluation(directTailEvaluation)
    if (tailIndex <= 0) return null
    const fallbackEvaluation = findLatestCandidatePreviewEvaluationFromTail(agentKernelItems, tailIndex - 1)
    return deriveCandidatePreviewQualityFromEvaluation(fallbackEvaluation)
  }, [candidatePreviewPresent, agentKernelItems])
}
