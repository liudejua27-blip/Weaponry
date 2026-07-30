import type { AgentItem, MechanicalConceptPlan } from '../../shared/types.js'
import {
  agentConversationReducer,
  claimAgentTurnSubmission,
  initialAgentConversationState,
  parseAgentTurnPresentation,
  parseCandidatePbrCapturePending,
  parseUniversalAuthorPresentation,
  releaseAgentTurnSubmission,
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
  payload: {
    schema_version: 'AgentActionToolEvent@1',
    tool_name: 'plan_complete_concept',
    result: { plan, accepted: true },
  },
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

const universalLimitationItems: AgentItem[] = [{
  item_id: 'item_u002_limitation',
  thread_id: 'thread_u002',
  turn_id: 'turn_u002',
  sequence: 1,
  item_type: 'tool_result',
  status: 'completed',
  payload: {
    tool_name: 'author_universal_asset',
    tool_result: {
      validated_output: {
        value: {
          schema_version: 'UniversalAuthorOutcome@1',
          outcome: 'limitation',
          subject_profile: {
            identity_label: '写实家猫',
            category: 'domestic cat',
            features: [
              { description: '猫科整体轮廓' },
              { description: '面部与四肢比例' },
              { description: '短毛表面' },
            ],
          },
          limitation: {
            code: 'representation_unavailable',
            message: '当前尚无写实动物可形变表示能力。',
            suggested_views: ['front', 'side', 'back'],
          },
        },
      },
    },
  },
  created_at: '2026-07-29T00:00:00Z',
}]

const pendingCandidatePbrCaptureItems: AgentItem[] = [{
  item_id: 'item_u004_pending_capture',
  thread_id: 'thread_u004',
  turn_id: 'turn_u004',
  sequence: 1,
  item_type: 'assistant_message',
  status: 'completed',
  payload: {
    candidate_pbr_capture_pending: {
      schema_version: 'CandidatePbrCapturePending@1',
      execution_id: 'execution_u004',
      project_id: 'project_u004',
      turn_id: 'turn_u004',
      route: 'universal_hard_surface',
    },
  },
  created_at: '2026-07-30T00:00:00Z',
}]

export function runAgentConversationStateSmoke(): void {
  const submissionGuard = { current: false }
  assert(claimAgentTurnSubmission(submissionGuard), 'first Agent turn submission must claim the shared workbench boundary')
  assert(!claimAgentTurnSubmission(submissionGuard), 'a concurrent composer or shortcut submission must not start a second turn')
  releaseAgentTurnSubmission(submissionGuard)
  assert(claimAgentTurnSubmission(submissionGuard), 'a later turn must be accepted after the active turn settles')
  releaseAgentTurnSubmission(submissionGuard)

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
  const universalLimitation = parseUniversalAuthorPresentation(universalLimitationItems)
  assert(universalLimitation?.outcome === 'limitation', 'U002 limitation must be a normal readable author outcome')
  assert(universalLimitation.identityLabel === '写实家猫', 'U002 must show the understood subject rather than a fallback template')
  assert(universalLimitation.suggestedViews.length === 3, 'U002 must expose bounded additional-view guidance')
  const pendingCapture = parseCandidatePbrCapturePending(pendingCandidatePbrCaptureItems)
  assert(pendingCapture?.executionId === 'execution_u004', 'U004 pending capture must retain the native execution identity')
  assert(pendingCapture?.projectId === 'project_u004' && pendingCapture.turnId === 'turn_u004', 'U004 pending capture must remain bound to one Project and Turn')
  assert(pendingCapture?.route === 'universal_hard_surface', 'U004 pending capture must retain its Rust-owned representation route')

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
