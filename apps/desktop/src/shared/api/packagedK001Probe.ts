import type { AgentEvent, AgentTurn } from '../types.js'
import { appServerTransport } from './appServerTransport.js'
import { AppServerProtocolError } from './appServerProtocol.js'
import { ForgeApiClient, ForgeApiError } from './forgeApi.js'
import { isTauriRuntime } from '../tauri/agentSupervisor.js'

const PROBE_SCHEMA = 'ForgeCADK001PackagedProbe@1' as const
const PROBE_TIMEOUT_MS = 180_000

type ProbeExpected = {
  project_id: string
  thread_id: string
  asset_version_id: string
  last_event_id: string
  cursor: string
  glb_sha256: string
}

type ProbeConfig = {
  schema_version: typeof PROBE_SCHEMA
  phase: 'initial' | 'restart'
  expected?: ProbeExpected
}

type ProbeReport = {
  schema_version: typeof PROBE_SCHEMA
  phase: 'initial' | 'restart'
  ok: boolean
  project_id?: string
  thread_id?: string
  asset_version_id?: string
  first_event_id?: string
  last_event_id?: string
  cursor?: string
  resume_from_event_id?: string
  resume_from_cursor?: string
  glb_sha256?: string
  protocol_glb_sha256?: string
  resource_glb_sha256?: string
  notification_count?: number
  native_lifecycle_transport?: boolean
  native_item_replay_verified?: boolean
  product_state_owner?: 'rust_app_server'
  python_product_api_used?: boolean
  turn_status?: string
  turn_error_code?: string
  provider_calls?: number
  error_code?: string
  diagnostic?: ProbeDiagnostic
}

type ProbeDiagnostic = {
  method: string
  route: string
  status: number
  error_code: string
  phase: 'initial' | 'restart'
  correlation_id: string
}

type EventObservation = {
  firstEventId: string | null
  lastEventId: string | null
  cursor: string | null
  notificationCount: number
  eventIds: string[]
  cursorBySequence: Map<number, string>
}

let probePromise: Promise<void> | null = null
let probeDiagnostic: ProbeDiagnostic | null = null

/**
 * Opt-in release-bundle diagnostic. It is inert unless the native smoke starts
 * the app with FORGECAD_K001_PACKAGED_PROBE=1. All product requests still use
 * the normal WebView -> Rust app-server transport; the report contains IDs and
 * hashes only, never GLB bytes, Provider credentials, prompts, or file paths.
 */
export function runPackagedK001ProbeOnce(): Promise<void> {
  if (!isTauriRuntime()) return Promise.resolve()
  if (!probePromise) probePromise = runPackagedK001Probe()
  return probePromise
}

async function runPackagedK001Probe(): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core')
  let config: ProbeConfig | null
  try {
    config = await invoke<ProbeConfig | null>('forgecad_k001_packaged_probe_config')
  } catch {
    return
  }
  if (config === null) return
  if (config.schema_version !== PROBE_SCHEMA || !['initial', 'restart'].includes(config.phase)) {
    return reportFailure(config?.phase === 'restart' ? 'restart' : 'initial', 'PROBE_CONFIG_INVALID', undefined)
  }

  const correlationId = `k001_${config.phase}_${crypto.randomUUID().replaceAll('-', '').slice(0, 16)}`
  probeDiagnostic = {
    method: 'GET',
    route: '/api/health',
    status: 0,
    error_code: 'PROBE_STARTUP_PENDING',
    phase: config.phase,
    correlation_id: correlationId,
  }
  try {
    await waitForHealth()
    markProbeSuccess(200)
    await requireRustProductStateOwner()
    const probe = config.phase === 'initial'
      ? runInitialProbe()
      : runRestartProbe(requireExpected(config))
    const report = await withTimeout(probe, PROBE_TIMEOUT_MS, 'PROBE_TOTAL_TIMEOUT')
    await invoke('forgecad_k001_packaged_probe_report', { report })
  } catch (error) {
    const errorCode = error instanceof Error && /^PROBE_[A-Z0-9_]{1,120}$/.test(error.message)
      ? error.message
      : 'PROBE_EXECUTION_FAILED'
    await reportFailure(config.phase, errorCode, diagnosticForError(error))
  }
}

async function reportFailure(
  phase: 'initial' | 'restart',
  errorCode: string,
  diagnostic: ProbeDiagnostic | undefined,
): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core')
  const report: ProbeReport = {
    schema_version: PROBE_SCHEMA,
    phase,
    ok: false,
    error_code: errorCode,
    ...(diagnostic === undefined ? {} : { diagnostic }),
  }
  await invoke('forgecad_k001_packaged_probe_report', { report }).catch(() => undefined)
}

async function runInitialProbe(): Promise<ProbeReport> {
  const api = new ForgeApiClient()
  const project = await jsonRequest('/api/v1/projects', 'POST', {
    client_request_id: 'k001-packaged-project',
    name: 'K001 packaged Rust app-server probe',
    profile_id: 'profile_weapon_concept_v1',
  }, 'k001-packaged-project')
  const projectId = stringField(project, 'project_id')
  markProbeRequest('NATIVE', '/api/v1/agent/threads', 'PROBE_NATIVE_THREAD_CREATE_PENDING')
  const thread = await api.createAgentThread({
    client_request_id: 'k001-packaged-thread',
    project_id: projectId,
    title: 'K001 packaged app-server probe',
    provider_id: 'deepseek',
  })
  markProbeSuccess(200)
  const threadId = thread.thread_id
  const events = observeNativeThread(api, threadId, 0)

  try {
    markProbeRequest('NATIVE', '/api/v1/agent/threads/:thread_id/events', 'PROBE_NATIVE_SSE_PENDING')
    await events.opened
    markProbeSuccess(200)
    markProbeRequest('NATIVE', '/api/v1/agent/threads/:thread_id/turns', 'PROBE_NATIVE_TURN_START_PENDING')
    const turn = await api.startAgentTurn(threadId, {
      client_request_id: 'k001-packaged-turn',
      message: '为游戏场景设计一件非功能、不可制造的未来机械概念道具外观。',
    })
    markProbeSuccess(200)
    assertOfflineNativeTurn(turn)
    await waitUntil(() => {
      const last = events.observation.eventIds.at(-1)
      return events.observation.eventIds.length >= turn.items.length
        && last !== undefined
        && events.observation.cursorBySequence.has(Number(last))
    }, 20_000)

    const plan = buildProbePlan(projectId)
    const directions = arrayField(plan, 'directions')
    const directionId = stringField(recordAt(directions, 0), 'direction_id')
    const built = await jsonRequest('/api/v1/agent/blockouts', 'POST', {
      client_request_id: 'k001-packaged-build',
      plan,
      direction_id: directionId,
      variation_index: 0,
      presentation_profile: 'quick_sketch',
    }, 'k001-packaged-build')
    const segmented = await jsonRequest('/api/v1/agent/blockouts:segment', 'POST', {
      client_request_id: 'k001-packaged-segment',
      plan,
      direction_id: directionId,
      artifact_id: stringField(built, 'artifact_id'),
      variant_id: stringField(built, 'variant_id'),
      variation_index: 0,
      presentation_profile: 'quick_sketch',
    }, 'k001-packaged-segment')
    const committed = await jsonRequest('/api/v1/agent/blockouts:commit', 'POST', {
      client_request_id: 'k001-packaged-commit',
      project_id: projectId,
      artifact_id: stringField(segmented, 'artifact_id'),
      summary: 'K001 packaged editable concept asset',
    }, 'k001-packaged-commit')
    const initialAssetVersionId = stringField(committed, 'asset_version_id')
    const initialSha = await exportGlbSha(initialAssetVersionId)

    const asset = await jsonRequest(`/api/v1/agent/asset-versions/${encodeURIComponent(initialAssetVersionId)}`)
    const editablePart = arrayField(asset, 'parts')
      .map(asRecord)
      .find((part) => Array.isArray(part.editable_parameter_bindings) && part.editable_parameter_bindings.length > 0)
    if (!editablePart) throw new Error('PROBE_EDITABLE_PART_MISSING')
    const binding = recordAt(arrayField(editablePart, 'editable_parameter_bindings'), 0)
    const minimum = numberField(binding, 'min')
    const maximum = numberField(binding, 'max')
    const step = numberField(binding, 'step')
    const current = numberField(binding, 'default')
    let value = minimum + step
    if (Math.abs(value - current) <= 1e-9) value += step
    if (value > maximum) throw new Error('PROBE_PARAMETER_RANGE_INVALID')

    const proposed = await jsonRequest(
      `/api/v1/agent/asset-versions/${encodeURIComponent(initialAssetVersionId)}/change-sets`,
      'POST',
      {
        client_request_id: 'k001-packaged-propose',
        summary: 'K001 packaged Rust transport edit',
        operations: [{
          operation_id: 'op_k001_packaged_parameter',
          op: 'set_part_parameter',
          part_id: stringField(editablePart, 'part_id'),
          path: stringField(binding, 'path'),
          value,
        }],
      },
      'k001-packaged-propose',
    )
    const changeSetId = stringField(proposed, 'change_set_id')
    const previewed = await jsonRequest(
      `/api/v1/agent/change-sets/${encodeURIComponent(changeSetId)}:preview`,
      'POST',
      undefined,
      'k001-packaged-preview',
    )
    if (stringField(previewed, 'status') !== 'previewed') throw new Error('PROBE_PREVIEW_FAILED')
    const confirmed = await jsonRequest(
      `/api/v1/agent/change-sets/${encodeURIComponent(changeSetId)}:confirm`,
      'POST',
      undefined,
      'k001-packaged-confirm',
    )
    const editedAssetVersionId = stringField(asRecord(confirmed.asset_version), 'asset_version_id')
    const editedSha = await exportGlbSha(editedAssetVersionId)
    if (editedSha === initialSha) throw new Error('PROBE_EDIT_DID_NOT_CHANGE_GLB')

    const active = await jsonRequest(`/api/v1/projects/${encodeURIComponent(projectId)}/active-design`)
    const undone = await jsonRequest(
      `/api/v1/projects/${encodeURIComponent(projectId)}/active-design:undo`,
      'POST',
      { client_request_id: 'k001-packaged-undo', snapshot_revision: integerField(active, 'revision') },
      'k001-packaged-undo',
    )
    const undoneAssetVersionId = stringField(asRecord(undone.active_design), 'asset_version_id')
    if (await exportGlbSha(undoneAssetVersionId) !== initialSha) throw new Error('PROBE_UNDO_GLB_MISMATCH')
    const redone = await jsonRequest(
      `/api/v1/projects/${encodeURIComponent(projectId)}/active-design:redo`,
      'POST',
      { client_request_id: 'k001-packaged-redo', snapshot_revision: integerField(undone, 'revision') },
      'k001-packaged-redo',
    )
    const finalAssetVersionId = stringField(asRecord(redone.active_design), 'asset_version_id')
    const glbSha = await exportGlbSha(finalAssetVersionId)
    if (glbSha !== editedSha) throw new Error('PROBE_REDO_GLB_MISMATCH')
    const protocolGlbSha = await protocolModelGlbSha(finalAssetVersionId)
    const resourceGlbSha = await resourceModelGlbSha(finalAssetVersionId)
    if (glbSha !== protocolGlbSha || glbSha !== resourceGlbSha) {
      throw new Error('PROBE_BINARY_TRANSPORT_MISMATCH')
    }

    const lastEventId = events.observation.eventIds.at(-1)
    const cursor = lastEventId === undefined
      ? undefined
      : events.observation.cursorBySequence.get(Number(lastEventId))
    if (!lastEventId || !cursor) throw new Error('PROBE_NATIVE_ITEM_CURSOR_MISSING')
    return {
      schema_version: PROBE_SCHEMA,
      phase: 'initial',
      ok: true,
      project_id: projectId,
      thread_id: threadId,
      asset_version_id: finalAssetVersionId,
      first_event_id: events.observation.eventIds[0],
      last_event_id: lastEventId,
      cursor,
      glb_sha256: glbSha,
      protocol_glb_sha256: protocolGlbSha,
      resource_glb_sha256: resourceGlbSha,
      notification_count: events.observation.notificationCount,
      native_lifecycle_transport: true,
      native_item_replay_verified: true,
      product_state_owner: 'rust_app_server',
      python_product_api_used: false,
      turn_status: turn.status,
      turn_error_code: turn.error_code ?? undefined,
      provider_calls: 0,
    }
  } finally {
    events.stop()
  }
}

async function runRestartProbe(expected: ProbeExpected): Promise<ProbeReport> {
  const api = new ForgeApiClient()
  markProbeRequest('NATIVE', '/api/v1/agent/threads/:thread_id/events', 'PROBE_NATIVE_SSE_PENDING')
  const events = observeNativeThread(api, expected.thread_id, 0)
  const checkpointSequence = parseEventSequence(expected.last_event_id)
  try {
    await events.opened
    markProbeSuccess(200)
    await events.replayComplete
    markProbeRequest('NATIVE', '/api/v1/agent/threads/:thread_id', 'PROBE_NATIVE_THREAD_READ_PENDING')
    const authoritativeThread = await api.getAgentThread(expected.thread_id)
    markProbeSuccess(200)
    const turn = authoritativeThread.turns.at(-1)
    if (!turn) throw new Error('PROBE_RESTART_TURN_MISSING')
    assertOfflineNativeTurn(turn)
    assertOrderedNativeReplay(events.events, expected.thread_id, turn.turn_id)
    const firstEventId = events.observation.eventIds[0]
    const lastEventId = events.observation.eventIds.at(-1)
    if (!firstEventId || lastEventId !== expected.last_event_id) {
      throw new Error('PROBE_RESTART_ITEM_REPLAY_DIVERGED')
    }
    const resumed = observeNativeThread(api, expected.thread_id, checkpointSequence)
    try {
      await resumed.opened
      await resumed.replayComplete
      if (resumed.events.length !== 0) {
        throw new Error('PROBE_RESTART_CHECKPOINT_REPLAY_DIVERGED')
      }
    } finally {
      resumed.stop()
    }
    const asset = await jsonRequest(`/api/v1/agent/asset-versions/${encodeURIComponent(expected.asset_version_id)}`)
    if (stringField(asset, 'asset_version_id') !== expected.asset_version_id) {
      throw new Error('PROBE_RESTART_ASSET_MISMATCH')
    }
    const glbSha = await exportGlbSha(expected.asset_version_id)
    const protocolGlbSha = await protocolModelGlbSha(expected.asset_version_id)
    const resourceGlbSha = await resourceModelGlbSha(expected.asset_version_id)
    if (glbSha !== expected.glb_sha256 || protocolGlbSha !== glbSha || resourceGlbSha !== glbSha) {
      throw new Error('PROBE_RESTART_GLB_MISMATCH')
    }
    return {
      schema_version: PROBE_SCHEMA,
      phase: 'restart',
      ok: true,
      project_id: expected.project_id,
      thread_id: expected.thread_id,
      asset_version_id: expected.asset_version_id,
      first_event_id: firstEventId,
      last_event_id: lastEventId,
      cursor: expected.cursor,
      resume_from_event_id: expected.last_event_id,
      resume_from_cursor: expected.cursor,
      glb_sha256: glbSha,
      protocol_glb_sha256: protocolGlbSha,
      resource_glb_sha256: resourceGlbSha,
      notification_count: events.observation.notificationCount,
      native_lifecycle_transport: true,
      native_item_replay_verified: true,
      product_state_owner: 'rust_app_server',
      python_product_api_used: false,
      turn_status: turn.status,
      turn_error_code: turn.error_code ?? undefined,
      provider_calls: 0,
    }
  } finally {
    events.stop()
  }
}

function observeNativeThread(api: ForgeApiClient, threadId: string, after: number): {
  opened: Promise<void>
  replayComplete: Promise<void>
  observation: EventObservation
  events: AgentEvent[]
  stop: () => void
} {
  const observation: EventObservation = {
    firstEventId: null,
    lastEventId: null,
    cursor: null,
    notificationCount: 0,
    eventIds: [],
    cursorBySequence: new Map(),
  }
  let resolveOpened: (() => void) | null = null
  const opened = new Promise<void>((resolve) => { resolveOpened = resolve })
  let resolveReplay: (() => void) | null = null
  let rejectReplay: ((error: Error) => void) | null = null
  const replayComplete = new Promise<void>((resolve, reject) => {
    resolveReplay = resolve
    rejectReplay = reject
  })
  const events: AgentEvent[] = []
  const capture = (event: AgentEvent): void => {
    if (event.thread_id !== threadId || event.sequence <= 0) return
    if (events.some((candidate) => candidate.turn_id === event.turn_id && candidate.sequence === event.sequence)) return
    events.push(event)
    events.sort((left, right) => left.sequence - right.sequence)
    observation.eventIds = events.map((candidate) => String(candidate.sequence))
    observation.firstEventId = observation.eventIds[0] ?? null
    observation.lastEventId = observation.eventIds.at(-1) ?? null
    observation.notificationCount = events.length
  }
  const stopNotifications = appServerTransport.subscribeNotifications((notification) => {
    if (notification.method !== 'item/updated' || !isRecord(notification.params)) return
    const params = notification.params
    if (params.schema_version !== 'NativeAgentNotification@1' || params.thread_id !== threadId) return
    const sequence = params.sequence
    const cursor = notification.cursor
    if (
      typeof sequence !== 'number'
      || !Number.isSafeInteger(sequence)
      || sequence <= 0
      || typeof cursor !== 'string'
      || params.cursor !== cursor
    ) {
      rejectReplay?.(new Error('PROBE_NATIVE_NOTIFICATION_INVALID'))
      return
    }
    observation.cursorBySequence.set(sequence, cursor)
    observation.cursor = cursor
  })
  const stopReplay = api.subscribeAgentThreadEvents(threadId, {
    onOpen: () => resolveOpened?.(),
    onEvent: capture,
    onReplayComplete: () => resolveReplay?.(),
    onError: (error) => rejectReplay?.(nativeReplayProbeError(error)),
  }, after)
  return {
    opened: withTimeout(opened, 20_000, 'PROBE_SSE_OPEN_TIMEOUT'),
    replayComplete: withTimeout(replayComplete, 20_000, 'PROBE_NATIVE_ITEM_REPLAY_TIMEOUT'),
    observation,
    events,
    stop: () => {
      stopReplay()
      stopNotifications()
    },
  }
}

function nativeReplayProbeError(error: Event): Error {
  const cause = (error as Event & { forgecad_error_code?: unknown }).forgecad_error_code
  if (typeof cause === 'string' && /^[A-Z0-9_]{1,80}$/.test(cause)) {
    return new Error(`PROBE_NATIVE_ITEM_REPLAY_${cause}`)
  }
  return new Error('PROBE_NATIVE_ITEM_REPLAY_FAILED')
}

async function waitForHealth(): Promise<void> {
  await waitUntil(async () => {
    try {
      const response = await appServerTransport.request('/api/health', { cache: 'no-store' })
      return response.ok
    } catch {
      return false
    }
  }, 35_000)
}

async function jsonRequest(
  path: string,
  method = 'GET',
  body?: Record<string, unknown>,
  idempotencyKey?: string,
): Promise<Record<string, unknown>> {
  markProbeRequest(method, path, 'PROBE_HTTP_PENDING')
  const headers = new Headers()
  if (body !== undefined) headers.set('Content-Type', 'application/json')
  if (idempotencyKey) headers.set('Idempotency-Key', idempotencyKey)
  const response = await appServerTransport.request(path, {
    method,
    headers,
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    cache: 'no-store',
  })
  if (!response.ok) {
    let stableErrorCode = 'UNKNOWN'
    try {
      const payload = asRecord(await response.json())
      const error = payload.error
      if (error && typeof error === 'object' && !Array.isArray(error)) {
        const errorRecord = error as Record<string, unknown>
        const details = errorRecord.details
        const detailRecord = details && typeof details === 'object' && !Array.isArray(details)
          ? details as Record<string, unknown>
          : undefined
        const missing = Array.isArray(detailRecord?.missing)
          ? detailRecord.missing.filter((value): value is string => typeof value === 'string').join('_')
          : ''
        const cause = typeof detailRecord?.cause_code === 'string' ? detailRecord.cause_code : ''
        const code = typeof errorRecord.code === 'string' ? errorRecord.code : ''
        const detailCode = [cause, missing, code].filter(Boolean).join('_')
        if (/^[A-Za-z0-9_.:-]{1,120}$/.test(detailCode)) {
          stableErrorCode = detailCode
        }
      }
    } catch {
      // The bounded route diagnostic intentionally omits any response body.
    }
    const routeDiagnostic = path
      .split('?')[0]
      .replace(/[^A-Za-z0-9]+/g, '_')
      .replace(/^_+|_+$/g, '')
      .toUpperCase()
      .slice(0, 48)
    const errorCode = `PROBE_HTTP_${response.status}_${routeDiagnostic || 'ROOT'}_${stableErrorCode.slice(0, 56)}`
    markProbeFailure(response.status, stableErrorCode)
    throw new Error(errorCode)
  }
  markProbeSuccess(response.status)
  return asRecord(await response.json())
}

function markProbeRequest(method: string, route: string, errorCode: string): void {
  if (!probeDiagnostic) return
  probeDiagnostic = {
    ...probeDiagnostic,
    method: method.toUpperCase(),
    route: normalizeProbeRoute(route),
    status: 0,
    error_code: errorCode,
  }
}

function markProbeSuccess(status: number): void {
  if (!probeDiagnostic) return
  probeDiagnostic = { ...probeDiagnostic, status, error_code: 'OK' }
}

function markProbeFailure(status: number, errorCode: string): void {
  if (!probeDiagnostic) return
  probeDiagnostic = { ...probeDiagnostic, status, error_code: normalizeProbeErrorCode(errorCode) }
}

function diagnosticForError(error: unknown): ProbeDiagnostic | undefined {
  if (!probeDiagnostic) return undefined
  const status = error instanceof ForgeApiError ? error.status : probeDiagnostic.status
  const errorCode = probeErrorCode(error)
  return {
    ...probeDiagnostic,
    status: Number.isInteger(status) && status >= 0 && status <= 599 ? status : 0,
    error_code: errorCode,
  }
}

function probeErrorCode(error: unknown): string {
  if (error instanceof ForgeApiError) return normalizeProbeErrorCode(error.code)
  if (error instanceof AppServerProtocolError) {
    const applicationCode = error.error.data?.application_code
    if (typeof applicationCode === 'string' && /^[A-Za-z0-9_.:-]{1,80}$/.test(applicationCode)) {
      return `RPC_${applicationCode}`
    }
    return `RPC_${error.error.code}`
  }
  if (error instanceof Error && /^PROBE_[A-Z0-9_]{1,120}$/.test(error.message)) return error.message
  return 'PROBE_EXECUTION_FAILED'
}

function normalizeProbeErrorCode(value: string): string {
  return /^[A-Za-z0-9_.:-]{1,80}$/.test(value) ? value : 'UNKNOWN'
}

function normalizeProbeRoute(route: string): string {
  const normalized = route.split('?')[0].replace(/\/+$/, '')
  if (normalized.startsWith('/')) return normalized.replace(/[^A-Za-z0-9_:/.-]/g, '_').slice(0, 120)
  return `/${normalized.replace(/[^A-Za-z0-9_:/.-]/g, '_').slice(0, 119)}`
}

async function exportGlbSha(assetVersionId: string): Promise<string> {
  const exported = await jsonRequest(
    `/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}:export`,
    'POST',
  )
  return sha256(assertGlb(decodeBase64(stringField(exported, 'glb_base64'))))
}

async function protocolModelGlbSha(assetVersionId: string): Promise<string> {
  const response = await appServerTransport.request(
    `/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}:model.glb`,
    { headers: { Accept: 'model/gltf-binary' }, cache: 'no-store' },
  )
  if (!response.ok) throw new Error(`PROBE_PROTOCOL_GLB_${response.status}`)
  return sha256(assertGlb(new Uint8Array(await response.arrayBuffer())))
}

async function resourceModelGlbSha(assetVersionId: string): Promise<string> {
  const response = await fetch(
    appServerTransport.resourceUrl(
      `/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}:model.glb`,
    ),
    { headers: { Accept: 'model/gltf-binary' }, cache: 'no-store' },
  )
  if (!response.ok) throw new Error(`PROBE_RESOURCE_GLB_${response.status}`)
  return sha256(assertGlb(new Uint8Array(await response.arrayBuffer())))
}

function buildProbePlan(projectId: string): Record<string, unknown> {
  const direction = (directionId: string, title: string): Record<string, unknown> => ({
    direction_id: directionId,
    title,
    summary: 'Complete non-functional exterior concept.',
    silhouette: 'compact',
    primary_part_roles: ['primary_form', 'secondary_form'],
    material_direction: 'dark metal and bounded visual coating',
  })
  return {
    schema_version: 'MechanicalConceptPlan@1',
    plan_id: 'plan_k001_packaged_probe',
    domain_pack_id: 'pack_future_weapon_prop',
    brief: 'non-functional future game prop production concept',
    generation_stage: 'blockout',
    spec: { project_id: projectId },
    // V003 permits one complete synthesis per Turn.  This packaged probe must
    // exercise the same code-owned Product Tool schema instead of replaying
    // the legacy three-direction planner contract.
    directions: [direction('direction_primary', 'Primary')],
    provider_id: 'rust_app_server',
    shape_program_ready: false,
  }
}

function assertOfflineNativeTurn(turn: AgentTurn): void {
  if (
    turn.status !== 'failed'
    || turn.error_code !== 'PROVIDER_NOT_CONFIGURED'
    || turn.usage.provider_requests !== 0
    || turn.items.length < 2
  ) {
    throw new Error('PROBE_NATIVE_TURN_NOT_OFFLINE_FAILED')
  }
}

function assertOrderedNativeReplay(events: AgentEvent[], threadId: string, turnId: string): void {
  const replayed = events
    .filter((event) => event.thread_id === threadId && event.turn_id === turnId)
    .sort((left, right) => left.sequence - right.sequence)
  if (
    replayed.length < 2
    || replayed.some((event, index) => event.sequence !== index + 1)
    || new Set(replayed.map((event) => event.item.item_id)).size !== replayed.length
  ) {
    throw new Error('PROBE_NATIVE_ITEM_REPLAY_INVALID')
  }
}

async function requireRustProductStateOwner(): Promise<void> {
  const initialized = await appServerTransport.initialize()
  if (initialized.migration_state?.state_owner !== 'rust_app_server') {
    throw new Error('PROBE_PRODUCT_STATE_NOT_RUST_OWNED')
  }
}

function requireExpected(config: ProbeConfig): ProbeExpected {
  if (!config.expected) throw new Error('PROBE_EXPECTED_MISSING')
  return config.expected
}

function parseEventSequence(value: string): number {
  const sequence = Number(value)
  if (!Number.isSafeInteger(sequence) || sequence <= 0 || String(sequence) !== value) {
    throw new Error('PROBE_EXPECTED_EVENT_SEQUENCE_INVALID')
  }
  return sequence
}

function asRecord(value: unknown): Record<string, unknown> {
  if (!isRecord(value)) throw new Error('PROBE_OBJECT_EXPECTED')
  return value
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function stringField(value: Record<string, unknown>, field: string): string {
  const result = value[field]
  if (typeof result !== 'string' || result.length === 0) throw new Error(`PROBE_${field.toUpperCase()}_INVALID`)
  return result
}

function numberField(value: Record<string, unknown>, field: string): number {
  const result = value[field]
  if (typeof result !== 'number' || !Number.isFinite(result)) throw new Error(`PROBE_${field.toUpperCase()}_INVALID`)
  return result
}

function integerField(value: Record<string, unknown>, field: string): number {
  const result = numberField(value, field)
  if (!Number.isSafeInteger(result) || result < 0) throw new Error(`PROBE_${field.toUpperCase()}_INVALID`)
  return result
}

function arrayField(value: Record<string, unknown>, field: string): unknown[] {
  const result = value[field]
  if (!Array.isArray(result)) throw new Error(`PROBE_${field.toUpperCase()}_INVALID`)
  return result
}

function recordAt(values: unknown[], index: number): Record<string, unknown> {
  return asRecord(values[index])
}

function decodeBase64(value: string): Uint8Array {
  const decoded = atob(value)
  return Uint8Array.from(decoded, (character) => character.charCodeAt(0))
}

function assertGlb(bytes: Uint8Array): Uint8Array {
  if (bytes.length < 12 || bytes[0] !== 0x67 || bytes[1] !== 0x6c || bytes[2] !== 0x54 || bytes[3] !== 0x46) {
    throw new Error('PROBE_GLB_INVALID')
  }
  return bytes
}

async function sha256(bytes: Uint8Array): Promise<string> {
  const owned = Uint8Array.from(bytes)
  const digest = await crypto.subtle.digest('SHA-256', owned.buffer)
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, '0')).join('')
}

async function waitUntil(
  predicate: () => boolean | Promise<boolean>,
  timeoutMs: number,
): Promise<void> {
  const started = Date.now()
  while (Date.now() - started < timeoutMs) {
    if (await predicate()) return
    await new Promise((resolve) => window.setTimeout(resolve, 100))
  }
  throw new Error('PROBE_WAIT_TIMEOUT')
}

async function withTimeout<T>(promise: Promise<T>, timeoutMs: number, errorCode: string): Promise<T> {
  let timeout = 0
  const rejected = new Promise<never>((_resolve, reject) => {
    timeout = window.setTimeout(() => reject(new Error(errorCode)), timeoutMs)
  })
  try {
    return await Promise.race([promise, rejected])
  } finally {
    window.clearTimeout(timeout)
  }
}
