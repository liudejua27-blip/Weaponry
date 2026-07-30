import { useMemo } from 'react'
import type { MultimodalAgentTurnContext } from './agentTurnSubmissionLoader'
import { buildReferenceEvidenceVisionContext } from './cadWorkbenchPanelAgentAssetHelpers'

type ReferenceEvidenceVisionContextInput = {
  submitAssistantInstructionWithText: (
    requestedText: string,
    clarificationDomainPackId?: string,
    multimodalContext?: MultimodalAgentTurnContext,
  ) => Promise<void>
  instruction: string
  activeAssetVersionId: string | null
  selectedPartId: string | null
  selectedMaterialZoneId: string | null
}

type ReferenceEvidenceVisionContext = ReturnType<typeof buildReferenceEvidenceVisionContext>

export function useCadWorkbenchPanelReferenceEvidenceVisionContext({
  submitAssistantInstructionWithText,
  instruction,
  activeAssetVersionId,
  selectedPartId,
  selectedMaterialZoneId,
}: ReferenceEvidenceVisionContextInput): ReferenceEvidenceVisionContext {
  return useMemo(
    () => buildReferenceEvidenceVisionContext(
      submitAssistantInstructionWithText,
      {
        instruction,
        activeAssetVersionId,
        selectedPartId,
        selectedMaterialZoneId,
      },
    ),
    [
      activeAssetVersionId,
      instruction,
      selectedMaterialZoneId,
      selectedPartId,
      submitAssistantInstructionWithText,
    ],
  )
}
