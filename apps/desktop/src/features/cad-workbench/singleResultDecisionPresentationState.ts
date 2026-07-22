import type { AgentItem } from '../../shared/types.js'

export type SingleResultPreview = {
  preview_id: string
  artifact_sha256: string
  artifact_profile_id: 'interactive_preview' | 'production_concept'
  expires_at?: string
}

type SingleResultDecisionBase = {
  schema_version: 'SingleResultDecision@1'
  decision_id: string
  project_id: string
  turn_id: string
  summary: string
  attempt_id: string
  gate_report_id: string
}

export type SingleResultReadyDecision = SingleResultDecisionBase & {
  state: 'ready_for_preview'
  outcome: 'passed'
  preview: SingleResultPreview
}

export type SingleResultFailedDecision = SingleResultDecisionBase & {
  state: 'failed'
  outcome: 'failed'
  failure: { code: string; message: string; repair_attempts_used: number }
}

export type SingleResultCancelledDecision = SingleResultDecisionBase & {
  state: 'cancelled'
  outcome: 'cancelled'
  cancel: { code: string; message: string }
}

export type SingleResultDecision = SingleResultReadyDecision | SingleResultFailedDecision | SingleResultCancelledDecision

export type SingleResultDecisionPresentation =
  | { state: 'idle' }
  | { state: 'processing'; detail?: string }
  | { state: 'ready'; decision: SingleResultReadyDecision }
  | { state: 'failed'; error: string }

export type SingleResultDecisionPresentationState = {
  projectId: string | null
  latestRequestId: number
  presentation: SingleResultDecisionPresentation
}

export const initialSingleResultDecisionPresentationState: SingleResultDecisionPresentationState = {
  projectId: null,
  latestRequestId: 0,
  presentation: { state: 'idle' },
}

export type SingleResultDecisionPresentationAction =
  | { type: 'open_project'; projectId: string | null }
  | { type: 'request_started'; projectId: string | null; requestId: number; detail?: string }
  | { type: 'decision_received'; projectId: string | null; requestId: number; decision: SingleResultDecision }
  | { type: 'request_failed'; projectId: string | null; requestId: number; error: string }
  | { type: 'request_cancelled'; projectId: string | null; requestId: number }

/**
 * This state is deliberately presentation-only. In particular, an accepted
 * result remains a server-owned unconfirmed preview and is never written to
 * localStorage or treated as an asset-version/Snapshot head.
 */
export function singleResultDecisionPresentationReducer(
  state: SingleResultDecisionPresentationState,
  action: SingleResultDecisionPresentationAction,
): SingleResultDecisionPresentationState {
  switch (action.type) {
    case 'open_project':
      if (state.projectId === action.projectId) return state
      return {
        projectId: action.projectId,
        latestRequestId: state.latestRequestId,
        presentation: { state: 'idle' },
      }
    case 'request_started':
      if (action.projectId !== state.projectId || action.requestId <= state.latestRequestId) return state
      return {
        ...state,
        latestRequestId: action.requestId,
        presentation: { state: 'processing', ...(action.detail ? { detail: action.detail } : {}) },
      }
    case 'decision_received':
      if (
        action.projectId !== state.projectId
        || action.requestId !== state.latestRequestId
        || action.decision.project_id !== state.projectId
      ) return state
      if (action.decision.state === 'ready_for_preview') {
        return { ...state, presentation: { state: 'ready', decision: action.decision } }
      }
      if (action.decision.state === 'failed') {
        return { ...state, presentation: { state: 'failed', error: action.decision.failure.message } }
      }
      return { ...state, presentation: { state: 'idle' } }
    case 'request_failed':
      if (action.projectId !== state.projectId || action.requestId !== state.latestRequestId) return state
      return { ...state, presentation: { state: 'failed', error: action.error } }
    case 'request_cancelled':
      if (action.projectId !== state.projectId || action.requestId !== state.latestRequestId) return state
      return { ...state, presentation: { state: 'idle' } }
  }
}

/**
 * The app-server item payload is intentionally the sole frontend boundary for
 * V003. Legacy Planner directions never satisfy this guard, so callers must
 * keep them on F026's explicitly-labelled compatibility path.
 */
export function readSingleResultDecision(value: unknown): SingleResultDecision | null {
  if (!isRecord(value) || value.schema_version !== 'SingleResultDecision@1') return null
  if (
    !isStableId(value.decision_id)
    || !isStableId(value.project_id)
    || !isStableId(value.turn_id)
    || !isStableId(value.attempt_id)
    || !isStableId(value.gate_report_id)
    || typeof value.summary !== 'string'
    || value.summary.trim().length === 0
    || value.summary.length > 1_024
  ) return null
  const base: SingleResultDecisionBase = {
    schema_version: 'SingleResultDecision@1', decision_id: value.decision_id,
    project_id: value.project_id, turn_id: value.turn_id, summary: value.summary,
    attempt_id: value.attempt_id, gate_report_id: value.gate_report_id,
  }
  if (value.state === 'ready_for_preview' && value.outcome === 'passed') {
    const preview = readSingleResultPreview(value.preview)
    return preview && value.failure === undefined && value.cancel === undefined
      ? { ...base, state: 'ready_for_preview', outcome: 'passed', preview }
      : null
  }
  if (value.state === 'failed' && value.outcome === 'failed') {
    const failure = readFailure(value.failure)
    return failure && value.preview === undefined && value.cancel === undefined
      ? { ...base, state: 'failed', outcome: 'failed', failure }
      : null
  }
  if (value.state === 'cancelled' && value.outcome === 'cancelled') {
    const cancel = readCancel(value.cancel)
    return cancel && value.preview === undefined && value.failure === undefined
      ? { ...base, state: 'cancelled', outcome: 'cancelled', cancel }
      : null
  }
  return null
}

export function readSingleResultDecisionFromAgentItems(
  items: AgentItem[],
  expected: { projectId: string | null; turnId: string },
): SingleResultDecision | null {
  for (const item of [...items].reverse()) {
    if (item.item_type !== 'tool_result' || item.payload.tool_name !== 'prepare_candidate_preview') continue
    const toolResult = item.payload.tool_result
    if (!isRecord(toolResult)) continue
    const validatedOutput = toolResult.validated_output
    if (!isRecord(validatedOutput)) continue
    const decision = readSingleResultDecision(validatedOutput.value)
    if (decision && decision.project_id === expected.projectId && decision.turn_id === expected.turnId) return decision
  }
  return null
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isStableId(value: unknown): value is string {
  return typeof value === 'string' && /^[A-Za-z0-9_.-]{1,160}$/.test(value)
}

function readSingleResultPreview(value: unknown): SingleResultPreview | null {
  if (
    !isRecord(value)
    || !isStableId(value.preview_id)
    || typeof value.artifact_sha256 !== 'string'
    || !/^[a-f0-9]{64}$/i.test(value.artifact_sha256)
  ) return null
  if (value.artifact_profile_id !== 'interactive_preview' && value.artifact_profile_id !== 'production_concept') return null
  if (value.expires_at !== undefined && (typeof value.expires_at !== 'string' || value.expires_at.length === 0)) return null
  return {
    preview_id: value.preview_id,
    artifact_sha256: value.artifact_sha256,
    artifact_profile_id: value.artifact_profile_id,
    ...(value.expires_at === undefined ? {} : { expires_at: value.expires_at }),
  }
}

function readFailure(value: unknown): SingleResultFailedDecision['failure'] | null {
  if (
    !isRecord(value)
    || !isStableId(value.code)
    || typeof value.message !== 'string'
    || !Number.isInteger(value.repair_attempts_used)
    || (value.repair_attempts_used as number) < 0
    || (value.repair_attempts_used as number) > 2
  ) return null
  return { code: value.code, message: value.message, repair_attempts_used: value.repair_attempts_used as number }
}

function readCancel(value: unknown): SingleResultCancelledDecision['cancel'] | null {
  if (!isRecord(value) || !isStableId(value.code) || typeof value.message !== 'string') return null
  return { code: value.code, message: value.message }
}
