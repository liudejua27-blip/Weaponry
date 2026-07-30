import { useCallback } from 'react'

import type { ForgeApi } from '../../shared/api/forgeApi'
import type { AgentAssetChangeSet, AgentItem } from '../../shared/types'
import type { AgentBlockoutGlbPayload } from './agentBlockoutDisplayState.js'
import type {
  AgentClarification,
  AgentClarificationOption,
  AgentTurnPresentation,
} from './agentConversationState'
import type { MultimodalAgentTurnContext, AgentTurnRecordResult } from './agentTurnSubmissionLoader'
import { recordAgentTurn as recordAgentTurnRequest } from './agentTurnSubmissionLoader'
import type { SingleResultDecisionPresentationAction } from './singleResultDecisionPresentationState'

type RecordAgentTurnApi = Pick<
  ForgeApi,
  | 'createAgentThread'
  | 'subscribeAgentThreadEvents'
  | 'startAgentTurn'
  | 'rejectSingleResultPreview'
  | 'loadSingleResultPreviewGlb'
>

type StartAgentConversationRequest = (projectId: string | null) => { requestId: number; projectId: string | null }
type IsCurrentAgentConversationRequest = (projectId: string | null, requestId: number) => boolean
type ParseAgentTurnPresentation = (items: readonly AgentItem[], requestText: string) => AgentTurnPresentation
type ReceiveAgentTurn = (
  projectId: string | null,
  requestId: number,
  threadId: string,
  items: readonly AgentItem[],
  presentation: AgentTurnPresentation,
) => boolean
type ReceiveAgentClarification = (
  projectId: string | null,
  requestId: number,
  clarification: AgentClarification,
) => boolean
type MarkAgentKernelUnavailable = (projectId: string | null, requestId: number) => boolean
type HydrateBlockoutDisplay = (
  projectId: string | null,
  data: {
    glbBase64: AgentBlockoutGlbPayload
    glbKind: 'compiled_agent_production_pbr' | 'compiled_agent_preview_pbr'
    shapeProgram: null
    segmentation: null
  },
) => number | null

type UseCadWorkbenchPanelRecordAgentTurnInput = {
  api: RecordAgentTurnApi
  conceptProjectId: string | null
  conceptProjectName: string | null
  agentThreadId: string | null
  agentKernelItems: readonly AgentItem[]
  clarificationOptions: readonly AgentClarificationOption[]
  startAgentConversationRequest: StartAgentConversationRequest
  isCurrentAgentConversationRequest: IsCurrentAgentConversationRequest
  claimAgentTurnSubmission: () => boolean
  releaseAgentTurnSubmission: () => void
  parseAgentTurnPresentation: ParseAgentTurnPresentation
  receiveAgentTurn: ReceiveAgentTurn
  receiveAgentClarification: ReceiveAgentClarification
  markAgentKernelUnavailable: MarkAgentKernelUnavailable
  dispatchSingleResultDecision: (action: SingleResultDecisionPresentationAction) => void
  setActiveProviderTurnId: (value: string | null) => void
  clearBlockoutDisplay: (projectId: string | null) => void
  clearAgentAssetWorkspace: () => void
  setAgentAssetChangeSet: (changeSet: AgentAssetChangeSet | null) => void
  setAgentCandidateSelectedPartId: (partId: string | null) => void
  hydrateBlockoutDisplay: HydrateBlockoutDisplay
  setAssistantNote: (message: string) => void
  errorText: (caught: unknown) => string
}

type UseCadWorkbenchPanelRecordAgentTurnOutput = {
  recordAgentTurn: (
    message: string,
    clarificationDomainPackId?: string,
    multimodalContext?: MultimodalAgentTurnContext,
  ) => Promise<AgentTurnRecordResult>
}

export function useCadWorkbenchPanelRecordAgentTurn({
  api,
  conceptProjectId,
  conceptProjectName,
  agentThreadId,
  agentKernelItems,
  clarificationOptions,
  startAgentConversationRequest,
  isCurrentAgentConversationRequest,
  claimAgentTurnSubmission,
  releaseAgentTurnSubmission,
  parseAgentTurnPresentation,
  receiveAgentTurn,
  receiveAgentClarification,
  markAgentKernelUnavailable,
  dispatchSingleResultDecision,
  setActiveProviderTurnId,
  clearBlockoutDisplay,
  clearAgentAssetWorkspace,
  setAgentAssetChangeSet,
  setAgentCandidateSelectedPartId,
  hydrateBlockoutDisplay,
  setAssistantNote,
  errorText,
}: UseCadWorkbenchPanelRecordAgentTurnInput): UseCadWorkbenchPanelRecordAgentTurnOutput {
  const recordAgentTurn = useCallback((
    message: string,
    clarificationDomainPackId?: string,
    multimodalContext?: MultimodalAgentTurnContext,
  ) => recordAgentTurnRequest(
    api,
    {
      startAgentConversationRequest,
      isCurrentAgentConversationRequest,
      claimAgentTurnSubmission,
      releaseAgentTurnSubmission,
      parseAgentTurnPresentation,
      receiveAgentTurn,
      receiveAgentClarification,
      markAgentKernelUnavailable,
      dispatchSingleResultDecision,
      setActiveProviderTurnId,
      clearBlockoutDisplay,
      clearAgentAssetWorkspace,
      setAgentAssetChangeSet,
      setAgentCandidateSelectedPartId,
      hydrateBlockoutDisplay,
      setAssistantNote,
      errorText,
    },
    {
      projectId: conceptProjectId,
      projectName: conceptProjectName,
      agentThreadId,
      agentKernelItems,
      message,
      clarificationDomainPackId,
      multimodalContext,
      clarificationOptions,
    },
  ), [
    api,
    agentKernelItems,
    agentThreadId,
    clarificationOptions,
    claimAgentTurnSubmission,
    conceptProjectId,
    conceptProjectName,
    clearAgentAssetWorkspace,
    clearBlockoutDisplay,
    dispatchSingleResultDecision,
    errorText,
    hydrateBlockoutDisplay,
    isCurrentAgentConversationRequest,
    markAgentKernelUnavailable,
    parseAgentTurnPresentation,
    receiveAgentClarification,
    receiveAgentTurn,
    releaseAgentTurnSubmission,
    setActiveProviderTurnId,
    setAgentAssetChangeSet,
    setAgentCandidateSelectedPartId,
    setAssistantNote,
    startAgentConversationRequest,
  ])

  return {
    recordAgentTurn,
  }
}
