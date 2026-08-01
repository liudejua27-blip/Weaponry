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
  '描述任意对象，或上传单图/多视图；Agent 会先理解对象与关键外观，再生成可执行结果或明确说明当前限制。'

export type UniversalAuthorPresentation = {
  outcome: 'executable' | 'limitation' | 'clarification_required'
  identityLabel: string | null
  category: string | null
  keyFeatures: string[]
  limitationCode: string | null
  message: string | null
  suggestedViews: string[]
}

export type CandidatePbrCapturePendingPresentation = {
  schemaVersion: 'CandidatePbrCapturePending@1'
  executionId: string
  projectId: string
  turnId: string
  route:
    | 'forge_visual_program'
    | 'universal_hard_surface'
    | 'universal_visual_exterior'
    | 'universal_local_lattice'
    | 'universal_local_hybrid'
}

const CANDIDATE_PBR_CAPTURE_ROUTES = new Set<CandidatePbrCapturePendingPresentation['route']>([
  'forge_visual_program',
  'universal_hard_surface',
  'universal_visual_exterior',
  'universal_local_lattice',
  'universal_local_hybrid',
])

function record(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null ? value as Record<string, unknown> : null
}

export function hasAgentToolInvocation(items: readonly AgentItem[], toolName: string): boolean {
  return items.some((item) => (
    (item.item_type === 'tool_call' || item.item_type === 'tool_result')
      && item.payload.tool_name === toolName
  ))
}

export function parseUniversalAuthorPresentation(items: readonly AgentItem[]): UniversalAuthorPresentation | null {
  for (const item of [...items].reverse()) {
    if (item.item_type !== 'tool_result' || item.payload.tool_name !== 'author_universal_asset') continue
    const toolResult = record(item.payload.tool_result)
    const validated = record(toolResult?.validated_output)
    const value = record(validated?.value)
    if (!value) continue
    const outcome = value?.outcome
    if (outcome !== 'executable' && outcome !== 'limitation' && outcome !== 'clarification_required') continue
    const profile = record(value.subject_profile)
    const features = Array.isArray(profile?.features) ? profile.features : []
    const limitation = record(value.limitation)
    return {
      outcome,
      identityLabel: typeof profile?.identity_label === 'string' ? profile.identity_label : null,
      category: typeof profile?.category === 'string' ? profile.category : null,
      keyFeatures: features
        .map((feature) => record(feature)?.description)
        .filter((description): description is string => typeof description === 'string')
        .slice(0, 3),
      limitationCode: typeof limitation?.code === 'string' ? limitation.code : null,
      message: typeof limitation?.message === 'string'
        ? limitation.message
        : typeof value.reason === 'string' ? value.reason : null,
      suggestedViews: Array.isArray(limitation?.suggested_views)
        ? limitation.suggested_views.filter((view): view is string => typeof view === 'string')
        : [],
    }
  }
  return null
}

/**
 * The pending descriptor is emitted by the Rust Action Loop only after a
 * candidate has completed compile/readback. It is not a preview, asset
 * version, or user-confirmable result.
 */
export function parseCandidatePbrCapturePending(
  items: readonly AgentItem[],
): CandidatePbrCapturePendingPresentation | null {
  for (const item of [...items].reverse()) {
    if (item.item_type !== 'assistant_message') continue
    const pending = record(item.payload.candidate_pbr_capture_pending)
    if (
      pending?.schema_version === 'CandidatePbrCapturePending@1'
      && typeof pending.execution_id === 'string'
      && typeof pending.project_id === 'string'
      && typeof pending.turn_id === 'string'
      && typeof pending.route === 'string'
      && CANDIDATE_PBR_CAPTURE_ROUTES.has(
        pending.route as CandidatePbrCapturePendingPresentation['route'],
      )
    ) {
      return {
        schemaVersion: 'CandidatePbrCapturePending@1',
        executionId: pending.execution_id,
        projectId: pending.project_id,
        turnId: pending.turn_id,
        route: pending.route as CandidatePbrCapturePendingPresentation['route'],
      }
    }
  }
  return null
}

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
        // Preserve enough alternating ToolCall/ToolResult rows to derive a
        // compact, verifiable process timeline without retaining Provider
        // reasoning or unbounded tool payloads.
        agentKernelItems: action.items.slice(-16),
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

/**
 * A workbench has one visible Agent turn at a time. Keep this tiny guard pure
 * so all entry points (composer, clarification, and visual evidence) can
 * share the same submission boundary before a request is started.
 */
export function claimAgentTurnSubmission(guard: { current: boolean }): boolean {
  if (guard.current) return false
  guard.current = true
  return true
}

export function releaseAgentTurnSubmission(guard: { current: boolean }): void {
  guard.current = false
}

export function parseAgentTurnPresentation(items: readonly AgentItem[], requestText: string): AgentTurnPresentation {
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
    const status = payload.status === 'unsupported' || payload.status === 'ambiguous'
      ? payload.status
      : null
    const isScopeStop = payload.kind === 'scope' && status === 'unsupported' && options.length === 0
    const isDomainClarification = payload.kind === 'domain' && status === 'ambiguous' && options.length > 0
    if (typeof payload.question === 'string' && (isScopeStop || isDomainClarification)) {
      return {
        clarification: {
          status,
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
