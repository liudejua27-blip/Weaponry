import type { AssemblyDeltaProgram, MechanicalConceptPlan } from '../../shared/types'
import { useCallback } from 'react'
import type { AgentTurnRecordResult, MultimodalAgentTurnContext } from './agentTurnSubmissionLoader'
import {
  submitBriefInstructionWithText,
  submitChangeInstruction,
} from './agentTurnSubmissionLoader'
import {
  ASSISTANT_EMPTY_INSTRUCTION_NOTICE,
  resolveAssistantActionRunner,
  trimAssistantInstruction,
} from './cadWorkbenchPanelAssistantActions'
import {
  DEFAULT_CONCEPT_BRIEF,
} from './cadWorkbenchPanelPrompts'

type RecordAgentTurn = (
  message: string,
  clarificationDomainPackId?: string,
  multimodalContext?: MultimodalAgentTurnContext,
) => Promise<AgentTurnRecordResult>

type PresentationProfile = 'quick_sketch' | 'showcase'

type UseCadWorkbenchPanelAssistantActionsInput = {
  assistantMode: 'brief' | 'change'
  chatInput: string
  legacyDesignReadOnly: boolean
  presentationProfile: PresentationProfile
  setAssistantMode: (assistantMode: 'brief' | 'change') => void
  setAssistantNote: (message: string) => void
  setChatInput: (message: string) => void
  agentPlan: MechanicalConceptPlan | null
  previewAgentDirection: (
    directionId: string,
    variationIndex?: number,
    requestedProfile?: PresentationProfile,
    planOverride?: MechanicalConceptPlan,
  ) => void | Promise<unknown>
  recordAgentTurn: RecordAgentTurn
  previewAgentAssemblyDelta: (delta: AssemblyDeltaProgram) => Promise<void>
}

type UseCadWorkbenchPanelAssistantActionsResult = {
  submitAssistantInstructionWithText: (
    requestedText: string,
    clarificationDomainPackId?: string,
    multimodalContext?: MultimodalAgentTurnContext,
  ) => Promise<void>
  runAssistantAction: () => Promise<void>
  retryCandidatePreview: () => void
  focusComposerInput: () => void
}

export function useCadWorkbenchPanelAssistantActions({
  assistantMode,
  chatInput,
  legacyDesignReadOnly,
  presentationProfile,
  setAssistantMode,
  setAssistantNote,
  setChatInput,
  agentPlan,
  previewAgentDirection,
  recordAgentTurn,
  previewAgentAssemblyDelta,
}: UseCadWorkbenchPanelAssistantActionsInput): UseCadWorkbenchPanelAssistantActionsResult {
  const submitAssistantInstructionWithText = useCallback(async (
    requestedText: string,
    clarificationDomainPackId?: string,
    multimodalContext?: MultimodalAgentTurnContext,
  ) => {
    const instruction = trimAssistantInstruction(requestedText)
    if (!instruction) {
      setAssistantNote(ASSISTANT_EMPTY_INSTRUCTION_NOTICE)
      return
    }
    await submitBriefInstructionWithText({
      requestText: instruction,
      clarificationDomainPackId,
      multimodalContext,
      defaultBrief: DEFAULT_CONCEPT_BRIEF,
      legacyDesignReadOnly,
      setAssistantNote,
      setChatInput,
      recordAgentTurn,
    })
  }, [legacyDesignReadOnly, recordAgentTurn, setAssistantNote, setChatInput])

  const submitAssistantInstruction = useCallback(async () => {
    const instruction = trimAssistantInstruction(chatInput)
    if (!instruction) {
      setAssistantNote(ASSISTANT_EMPTY_INSTRUCTION_NOTICE)
      return
    }
    await submitAssistantInstructionWithText(instruction)
  }, [chatInput, setAssistantNote, submitAssistantInstructionWithText])

  const previewChangeInstruction = useCallback(async () => {
    await submitChangeInstruction({
      requestText: chatInput,
      legacyDesignReadOnly,
      setAssistantNote,
      setChatInput,
      recordAgentTurn,
      previewAgentAssemblyDelta,
    })
  }, [
    chatInput,
    legacyDesignReadOnly,
    previewAgentAssemblyDelta,
    recordAgentTurn,
    setAssistantNote,
    setChatInput,
  ])

  const runAssistantAction = useCallback(async () => {
    await resolveAssistantActionRunner(
      assistantMode,
      submitAssistantInstruction,
      previewChangeInstruction,
    )()
  }, [assistantMode, submitAssistantInstruction, previewChangeInstruction])

  const retryCandidatePreview = useCallback(() => {
    const currentDirection = agentPlan?.directions[0]
    if (agentPlan && currentDirection) {
      void previewAgentDirection(currentDirection.direction_id, 0, presentationProfile, agentPlan)
      return
    }
    void runAssistantAction()
  }, [agentPlan, previewAgentDirection, presentationProfile, runAssistantAction])

  const focusComposerInput = useCallback(() => {
    setAssistantMode('change')
    window.requestAnimationFrame(() => document.querySelector<HTMLTextAreaElement>('[aria-label="设计需求"]')?.focus())
  }, [setAssistantMode])

  return {
    submitAssistantInstructionWithText,
    runAssistantAction,
    retryCandidatePreview,
    focusComposerInput,
  }
}
