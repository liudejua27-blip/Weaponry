import type { AgentItem, MechanicalConceptPlan } from '../../shared/types.js'
import {
  agentConversationReducer,
  initialAgentConversationState,
  parseAgentTurnPresentation,
} from './agentConversationState.js'

const plan: MechanicalConceptPlan = {
  schema_version: 'MechanicalConceptPlan@1',
  plan_id: 'plan_f008',
  domain_pack_id: 'pack_vehicle_concept',
  brief: '双座冰原探索汽车',
  generation_stage: 'blockout',
  spec: {},
  directions: [{
    direction_id: 'direction_f008',
    title: '紧凑探索车',
    summary: '完整封闭车身与大轮胎',
    silhouette: 'balanced',
    primary_part_roles: ['vehicle_body'],
    material_direction: '深色耐候涂层',
  }],
  provider_id: 'deterministic_rules',
}

const planItems: AgentItem[] = [{
  item_id: 'item_a003_provider_trace',
  thread_id: 'thread_f008',
  turn_id: 'turn_f008',
  sequence: 1,
  item_type: 'tool_result',
  status: 'completed',
  payload: {
    tool: 'provider_gateway',
    provider_execution_trace: {
      schema_version: 'ProviderExecutionTrace@1',
      trace_id: 'trace_f008',
      provider_id: 'deterministic_mechanical_planner',
      phase: 'completed',
      message: '本机离线规划已完成；未调用外部 Provider。',
      attempt: 1,
      network_call_made: false,
    },
  },
  created_at: '2026-07-14T00:00:00Z',
}, {
  item_id: 'item_f008_plan',
  thread_id: 'thread_f008',
  turn_id: 'turn_f008',
  sequence: 2,
  item_type: 'tool_result',
  status: 'completed',
  payload: { result: plan },
  created_at: '2026-07-14T00:00:00Z',
}]

const scopeStopItems: AgentItem[] = [{
  item_id: 'item_g814_scope',
  thread_id: 'thread_g814',
  turn_id: 'turn_g814',
  sequence: 1,
  item_type: 'clarification',
  status: 'completed',
  payload: {
    kind: 'scope',
    status: 'unsupported',
    question: '这个请求涉及现实制造、安全、控制或性能内容。',
    options: [],
  },
  created_at: '2026-07-14T00:00:00Z',
}]

export function runAgentConversationStateSmoke(): void {
  let state = agentConversationReducer(initialAgentConversationState, { type: 'open_project', projectId: 'project-a' })
  state = agentConversationReducer(state, { type: 'set_chat_input', value: '设计一辆探索汽车' })
  state = agentConversationReducer(state, { type: 'request_started', projectId: 'project-a', requestId: 2 })
  const presentation = parseAgentTurnPresentation(planItems, '设计一辆探索汽车')
  state = agentConversationReducer(state, {
    type: 'turn_received',
    projectId: 'project-a',
    requestId: 2,
    threadId: 'thread_f008',
    items: planItems,
    presentation,
  })
  assert(state.agentPlan?.plan_id === 'plan_f008', 'current turn must expose its design directions')
  assert(state.agentThreadId === 'thread_f008', 'current turn must retain only its project thread')
  const scopePresentation = parseAgentTurnPresentation(scopeStopItems, '给我现实枪械的加工尺寸')
  assert(scopePresentation.clarification?.kind === 'scope' && scopePresentation.clarification.options.length === 0, 'scope stop must not offer a domain selection or expose directions')

  state = agentConversationReducer(state, { type: 'open_project', projectId: 'project-b' })
  assert(state.chatInput === '', 'project switch must atomically clear the input draft')
  assert(state.agentPlan === null && state.agentClarification === null, 'project switch must clear old direction and clarification presentation')
  assert(state.agentThreadId === null && state.agentKernelItems.length === 0, 'project switch must not retain the old thread or turn items')

  const afterStaleTurn = agentConversationReducer(state, {
    type: 'turn_received',
    projectId: 'project-a',
    requestId: 2,
    threadId: 'thread_f008',
    items: planItems,
    presentation,
  })
  assert(afterStaleTurn === state, 'late response from a previous project must be ignored')

  state = agentConversationReducer(state, { type: 'request_started', projectId: 'project-b', requestId: 4 })
  state = agentConversationReducer(state, {
    type: 'clarification_received',
    projectId: 'project-b',
    requestId: 4,
    clarification: {
      status: 'ambiguous',
      kind: 'domain',
      question: '你想从哪类对象开始？',
      options: [{ domain_pack_id: 'pack_aircraft_concept', label: '飞机与航空器', prompt: '设计一架飞机' }],
      originalMessage: '设计一个能飞的载具',
    },
  })
  assert(state.agentClarification?.status === 'ambiguous', 'current project must show its clarification')
  assert(state.agentPlan === null, 'clarification must not preserve a selectable direction')
  state = agentConversationReducer(state, { type: 'request_started', projectId: 'project-b', requestId: 5 })
  assert(state.agentClarification === null && state.agentPlan === null, 'a follow-up turn must clear the previous clarification before its plan arrives')
  const afterCancelledClarification = agentConversationReducer(state, {
    type: 'clarification_received',
    projectId: 'project-b',
    requestId: 4,
    clarification: state.agentClarification!,
  })
  assert(afterCancelledClarification === state, 'late response from a cancelled turn must be ignored')
  state = agentConversationReducer(state, {
    type: 'turn_received',
    projectId: 'project-b',
    requestId: 5,
    threadId: 'thread_f008',
    items: planItems,
    presentation,
  })
  assert(state.agentPlan?.plan_id === 'plan_f008' && state.agentClarification === null, 'the selected clarification must reveal its current design directions')
  assert(!('asset_version_id' in state), 'conversation presentation must not own asset-version truth')
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
