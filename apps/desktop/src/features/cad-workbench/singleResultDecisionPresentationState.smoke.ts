import type { AgentItem } from '../../shared/types.js'
import {
  initialSingleResultDecisionPresentationState,
  readSingleResultDecision,
  readSingleResultDecisionFromAgentItems,
  singleResultDecisionPresentationReducer,
} from './singleResultDecisionPresentationState.js'

const readyPayload = {
  schema_version: 'SingleResultDecision@1',
  decision_id: 'decision-v003-1',
  project_id: 'project-v003',
  turn_id: 'turn-v003',
  state: 'ready_for_preview',
  outcome: 'passed',
  summary: '机械臂完整外观已通过本次正式生成质量门。',
  attempt_id: 'attempt-v003-1',
  gate_report_id: 'gate-v003-1',
  preview: {
    preview_id: 'preview-v003-1',
    artifact_sha256: 'a'.repeat(64),
    artifact_profile_id: 'interactive_preview',
  },
}

export function runSingleResultDecisionPresentationStateSmoke(): void {
  const decision = readSingleResultDecision(readyPayload)
  assert(decision?.outcome === 'passed', 'a formal ready decision must be accepted')
  assert(readSingleResultDecision({ ...readyPayload, schema_version: 'MechanicalConceptPlan@1' }) === null, 'legacy planner payloads must not enter ready')
  assert(readSingleResultDecision({ ...readyPayload, outcome: 'failed' }) === null, 'failed gate results must not enter ready')
  assert(readSingleResultDecision({ ...readyPayload, gate_report_id: '' }) === null, 'ready requires an identified formal gate report')
  assert(readSingleResultDecision({ ...readyPayload, preview: 'legacy preview' }) === null, 'preview metadata must be structured when supplied')

  const items: AgentItem[] = [{
    item_id: 'item-v003-result',
    thread_id: 'thread-v003',
    turn_id: 'turn-v003',
    sequence: 1,
    item_type: 'tool_result',
    status: 'completed',
    payload: { tool_name: 'prepare_candidate_preview', tool_result: { validated_output: { value: readyPayload } } },
    created_at: '2026-07-18T00:00:00Z',
  }]
  assert(readSingleResultDecisionFromAgentItems(items, { projectId: 'project-v003', turnId: 'turn-v003' })?.decision_id === 'decision-v003-1', 'only the adapter may unwrap a formal decision from Agent items')
  assert(readSingleResultDecisionFromAgentItems(items, { projectId: 'project-other', turnId: 'turn-v003' }) === null, 'an otherwise valid result must not cross the project boundary')
  assert(readSingleResultDecisionFromAgentItems(items, { projectId: 'project-v003', turnId: 'turn-other' }) === null, 'an otherwise valid result must not cross the Turn boundary')

  let state = singleResultDecisionPresentationReducer(initialSingleResultDecisionPresentationState, { type: 'open_project', projectId: 'project-v003' })
  state = singleResultDecisionPresentationReducer(state, { type: 'request_started', projectId: 'project-v003', requestId: 1 })
  assert(state.presentation.state === 'processing', 'an active request must render processing without a result')
  state = singleResultDecisionPresentationReducer(state, { type: 'decision_received', projectId: 'project-v003', requestId: 1, decision: decision! })
  assert(state.presentation.state === 'ready', 'only the matching project/request may reveal the ready result')
  state = singleResultDecisionPresentationReducer(state, { type: 'open_project', projectId: 'project-other' })
  assert(state.presentation.state === 'idle', 'project switch must discard the unconfirmed preview')
  const late = singleResultDecisionPresentationReducer(state, { type: 'decision_received', projectId: 'project-v003', requestId: 1, decision: decision! })
  assert(late === state, 'late decisions from the prior project must be ignored')
  state = singleResultDecisionPresentationReducer(state, { type: 'request_started', projectId: 'project-other', requestId: 3 })
  state = singleResultDecisionPresentationReducer(state, { type: 'request_cancelled', projectId: 'project-other', requestId: 3 })
  assert(state.presentation.state === 'idle', 'cancellation must not preserve an unconfirmed preview across restart')
  state = singleResultDecisionPresentationReducer(state, { type: 'request_started', projectId: 'project-other', requestId: 4 })
  state = singleResultDecisionPresentationReducer(state, { type: 'request_failed', projectId: 'project-other', requestId: 4, error: 'quality gate failed' })
  assert(state.presentation.state === 'failed', 'failure must remain visible without inventing a result')
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
