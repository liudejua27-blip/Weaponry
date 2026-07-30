import { useMemo } from 'react'
import type { CandidatePreviewQuality } from './candidatePreviewQualityLogic.js'
import type { CandidatePreviewQualityPresentation } from './candidatePreviewQualityPresentation.js'
import {
  buildCandidatePreviewQualityPresentation,
  EMPTY_PRESENTATION_WITH_CANDIDATE,
  EMPTY_PRESENTATION_WITHOUT_CANDIDATE,
} from './candidatePreviewQualityPresentation.js'

type UseCadWorkbenchPanelCandidatePreviewQualityPresentationInput = {
  candidatePreviewPresent: boolean
  quality: CandidatePreviewQuality | null
}

export function useCadWorkbenchPanelCandidatePreviewQualityPresentation({
  candidatePreviewPresent,
  quality,
}: UseCadWorkbenchPanelCandidatePreviewQualityPresentationInput): CandidatePreviewQualityPresentation {
  return useMemo(
    () => quality === null
      ? (candidatePreviewPresent
        ? EMPTY_PRESENTATION_WITH_CANDIDATE
        : EMPTY_PRESENTATION_WITHOUT_CANDIDATE)
      : buildCandidatePreviewQualityPresentation(candidatePreviewPresent, quality),
    [candidatePreviewPresent, quality],
  )
}
