import type { AgentItem, MechanicalConceptPlan } from '../../shared/types.js'

export type AgentClarificationOption = {
  domain_pack_id: string
  label: string
  prompt: string
}

export type AgentClarification = {
  status: 'ambiguous' | 'unsupported'
  kind: 'domain' | 'scope'
  question: string
  options: AgentClarificationOption[]
  originalMessage?: string
}

export type AgentTurnPresentation = {
  clarification: AgentClarification | null
  plan: MechanicalConceptPlan | null
}

export type AgentConversationState = {
  projectId: string | null
  chatInput: string
  assistantMode: 'brief' | 'change'
  assistantNote: string
  agentThreadId: string | null
  agentKernelItems: AgentItem[]
  agentKernelUnavailable: boolean
  agentClarification: AgentClarification | null
  agentPlan: MechanicalConceptPlan | null
  latestRequestId: number
}

export const DEFAULT_AGENT_ASSISTANT_NOTE =
  '输入汽车、飞机、机械臂或未来道具创意；Agent 会先记录理解，再生成可预览方向。'

export const initialAgentConversationState: AgentConversationState = {
  projectId: null,
  chatInput: '',
  assistantMode: 'brief',
  assistantNote: DEFAULT_AGENT_ASSISTANT_NOTE,
  agentThreadId: null,
  agentKernelItems: [],
  agentKernelUnavailable: false,
  agentClarification: null,
  agentPlan: null,
  latestRequestId: 0,
}

export type AgentConversationAction =
  | { type: 'open_project'; projectId: string | null }
  | { type: 'request_started'; projectId: string | null; requestId: number }
  | { type: 'set_chat_input'; value: string }
  | { type: 'set_assistant_mode'; value: 'brief' | 'change' }
  | { type: 'set_assistant_note'; value: string }
  | {
      type: 'turn_received'
      projectId: string | null
      requestId: number
      threadId: string
      items: AgentItem[]
      presentation: AgentTurnPresentation
    }
  | { type: 'clarification_received'; projectId: string | null; requestId: number; clarification: AgentClarification }
  | { type: 'kernel_unavailable'; projectId: string | null; requestId: number }

/**
 * Pure presentation state for the Agent conversation. It intentionally has no
 * Snapshot, asset version, ChangeSet, quality, export, or API ownership.
 */
export function agentConversationReducer(
  state: AgentConversationState,
  action: AgentConversationAction,
): AgentConversationState {
  switch (action.type) {
    case 'open_project':
      if (state.projectId === action.projectId) return state
      return {
        ...initialAgentConversationState,
        projectId: action.projectId,
        latestRequestId: state.latestRequestId,
      }
    case 'request_started':
      if (action.projectId !== state.projectId || action.requestId <= state.latestRequestId) return state
      // A selected clarification begins a new turn. Do not let the previous
      // question mask the current turn's directions while its response arrives.
      return {
        ...state,
        latestRequestId: action.requestId,
        agentClarification: null,
        agentPlan: null,
        agentKernelUnavailable: false,
      }
    case 'set_chat_input':
      return state.chatInput === action.value ? state : { ...state, chatInput: action.value }
    case 'set_assistant_mode':
      return state.assistantMode === action.value ? state : { ...state, assistantMode: action.value }
    case 'set_assistant_note':
      return state.assistantNote === action.value ? state : { ...state, assistantNote: action.value }
    case 'turn_received':
      if (action.projectId !== state.projectId || action.requestId !== state.latestRequestId) return state
      return {
        ...state,
        agentThreadId: action.threadId,
        agentKernelItems: action.items.slice(-6),
        agentKernelUnavailable: false,
        agentClarification: action.presentation.clarification,
        agentPlan: action.presentation.clarification
          ? null
          : action.presentation.plan ?? state.agentPlan,
      }
    case 'clarification_received':
      if (action.projectId !== state.projectId || action.requestId !== state.latestRequestId) return state
      return {
        ...state,
        agentKernelUnavailable: false,
        agentClarification: action.clarification,
        agentPlan: null,
      }
    case 'kernel_unavailable':
      if (action.projectId !== state.projectId || action.requestId !== state.latestRequestId) return state
      return {
        ...state,
        agentKernelUnavailable: true,
        agentClarification: null,
      }
  }
}

export function isCurrentAgentConversationRequest(latestRequestId: number, requestId: number): boolean {
  return latestRequestId === requestId
}

export function parseAgentTurnPresentation(items: AgentItem[], requestText: string): AgentTurnPresentation {
  const clarificationItem = items.find((item) => item.item_type === 'clarification')
  if (clarificationItem) {
    const payload = clarificationItem.payload
    const options = Array.isArray(payload.options)
      ? payload.options.filter((option): option is AgentClarificationOption => (
        typeof option === 'object'
        && option !== null
        && typeof (option as { domain_pack_id?: unknown }).domain_pack_id === 'string'
        && typeof (option as { label?: unknown }).label === 'string'
        && typeof (option as { prompt?: unknown }).prompt === 'string'
      ))
      : []
    if (
      typeof payload.question === 'string'
      && (payload.status === 'ambiguous' || payload.status === 'unsupported')
      && (
        ((payload.kind === 'domain' || typeof payload.kind !== 'string') && options.length > 0)
        || (payload.kind === 'scope' && payload.status === 'unsupported' && options.length === 0)
      )
    ) {
      return {
        clarification: {
          status: payload.status,
          kind: payload.kind === 'scope' ? 'scope' : 'domain',
          question: payload.question,
          options,
          originalMessage: requestText,
        },
        plan: null,
      }
    }
  }

  const plan = items
    .filter((item) => item.item_type === 'tool_result')
    .map((item) => {
      const result = item.payload.result
      if (typeof result === 'object' && result !== null && 'plan' in result) {
        return (result as { plan?: unknown }).plan
      }
      return result
    })
    .find((resultPayload): resultPayload is MechanicalConceptPlan => (
      typeof resultPayload === 'object'
      && resultPayload !== null
      && 'plan_id' in resultPayload
      && 'directions' in resultPayload
    )) ?? null
  return { clarification: null, plan }
}
