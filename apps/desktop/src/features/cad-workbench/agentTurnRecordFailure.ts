import { ForgeApiError } from '../../shared/api/forgeApi'
import type { AgentClarification, AgentClarificationOption } from './agentConversationState'
import type { CandidatePbrCapturePendingPresentation } from './agentConversationState'
import type { SingleResultDecisionPresentationAction } from './singleResultDecisionPresentationState'
import { isProviderExecutionError } from './providerConnectionPresentation'

type AgentTurnRecordFailureResult = {
  recorded: false
  clarification: boolean
  cancelled: boolean
  failed: boolean
  plan: null
  decision: null
  candidatePbrCapturePending: CandidatePbrCapturePendingPresentation | null
}

type ResolveAgentTurnFailureOptions = {
  caught: unknown
  projectId: string | null
  requestId: number
  message: string
  clarificationOptions: readonly AgentClarificationOption[]
  isCurrentRequest: (projectId: string | null, requestId: number) => boolean
  receiveAgentClarification: (projectId: string | null, requestId: number, clarification: AgentClarification) => boolean
  setAssistantNote: (note: string) => void
  dispatchSingleResultDecision: (action: Extract<SingleResultDecisionPresentationAction, { type: 'request_cancelled' | 'request_failed' }>) => void
  markKernelUnavailable: (projectId: string | null, requestId: number) => boolean
}

const FAILED_REQUEST_REJECTED: AgentTurnRecordFailureResult = {
  recorded: false,
  clarification: false,
  cancelled: true,
  failed: false,
  plan: null,
  decision: null,
  candidatePbrCapturePending: null,
}

const CANCELED_BY_USER: AgentTurnRecordFailureResult = {
  recorded: false,
  clarification: false,
  cancelled: false,
  failed: false,
  plan: null,
  decision: null,
  candidatePbrCapturePending: null,
}

export function resolveAgentTurnRecordFailure(options: ResolveAgentTurnFailureOptions): AgentTurnRecordFailureResult {
  const {
    caught,
    projectId,
    requestId,
    message,
    clarificationOptions,
    isCurrentRequest,
    receiveAgentClarification,
    setAssistantNote,
    dispatchSingleResultDecision,
    markKernelUnavailable,
  } = options

  if (!isCurrentRequest(projectId, requestId)) return FAILED_REQUEST_REJECTED

  if (caught instanceof ForgeApiError && (caught.code === 'DOMAIN_AMBIGUOUS' || caught.code === 'DOMAIN_UNSUPPORTED')) {
    const clarification: AgentClarification = {
      status: caught.code === 'DOMAIN_AMBIGUOUS' ? 'ambiguous' : 'unsupported',
      kind: 'domain',
      question: caught.message,
      options: [...clarificationOptions],
      originalMessage: message,
    }
    if (!receiveAgentClarification(projectId, requestId, clarification)) {
      return FAILED_REQUEST_REJECTED
    }
    dispatchSingleResultDecision({ type: 'request_cancelled', projectId, requestId })
    setAssistantNote(caught.message)
    return {
      recorded: false,
      clarification: true,
      cancelled: false,
      failed: false,
      plan: null,
      decision: null,
      candidatePbrCapturePending: null,
    }
  }

  if (caught instanceof ForgeApiError && isProviderExecutionError(caught.code)) {
    const networkCall = caught.details.network_call_made === true ? 'true' : 'false'
    setAssistantNote(`模型请求失败：${caught.message}（${caught.code}，network_call_made=${networkCall}）。不会切换到离线 Planner；已保存资产没有变化。`)
    dispatchSingleResultDecision({ type: 'request_failed', projectId, requestId, error: caught.message })
    return {
      recorded: false,
      clarification: false,
      cancelled: false,
      failed: true,
      plan: null,
      decision: null,
      candidatePbrCapturePending: null,
    }
  }

  if (caught instanceof ForgeApiError && (
    caught.code.startsWith('MULTIMODAL_')
    || caught.code === 'AGENT_CLIENT_REQUEST_REUSE_CONFLICT'
  )) {
    setAssistantNote(`视觉证据绑定失败：${caught.message}当前设计没有变化。`)
    dispatchSingleResultDecision({ type: 'request_failed', projectId, requestId, error: caught.message })
    return {
      recorded: false,
      clarification: false,
      cancelled: false,
      failed: true,
      plan: null,
      decision: null,
      candidatePbrCapturePending: null,
    }
  }

  // The compatibility planner remains usable when the new kernel is not
  // available yet (for example while an older local Agent is running).
  if (!markKernelUnavailable(projectId, requestId)) {
    return FAILED_REQUEST_REJECTED
  }

  dispatchSingleResultDecision({ type: 'request_cancelled', projectId, requestId })
  return CANCELED_BY_USER
}
