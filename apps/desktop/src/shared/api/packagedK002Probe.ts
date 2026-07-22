import type { AgentEvent, AgentItem, AgentThreadDetail, AgentTurn } from '../types.js'
import { isTauriRuntime } from '../tauri/agentSupervisor.js'
import { appServerTransport } from './appServerTransport.js'
import { ForgeApiClient } from './forgeApi.js'

const PROBE_SCHEMA = 'ForgeCADK002PackagedProbe@1' as const
const PROBE_TIMEOUT_MS = 90_000
const REPLAY_TIMEOUT_MS = 20_000
const SHA256_PATTERN = /^[a-f0-9]{64}$/

type ProbeExpected = {
  thread_id: string
  turn_id: string
  items_sha256: string
  item_count: number
  last_sequence: number
  turn_error_code: string
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
  thread_id?: string
  turn_id?: string
  turn_status?: string
  turn_error_code?: string
  provider_status?: string
  provider_configured?: boolean
  provider_network_call_made?: boolean
  supervisor_running?: boolean
  supervisor_state?: string
  supervisor_managed_by_desktop?: boolean
  reasoning_content_present?: boolean
  legacy_lifecycle_post_status?: number
  provider_calls?: number
  item_count?: number
  last_sequence?: number
  item_sequences?: number[]
  item_ids?: string[]
  item_types?: string[]
  items_sha256?: string
  replay_items_sha256?: string
  error_code?: string
}

type ProviderPreflight = {
  status: 'unconfigured'
  configured: false
  networkCallMade: false
}

type SupervisorProof = {
  running: true
  state: 'running'
  managedByDesktop: true
}

let probePromise: Promise<void> | null = null

/**
 * Opt-in packaged K002 ownership proof. It is inert in normal launches and in
 * the K001 oracle. Every business call travels through the production
 * WebView -> Rust app-server transport. The report contains only stable IDs,
 * order metadata and hashes; it never includes prompts, Item payloads,
 * cancellation capabilities, Provider credentials, reasoning, or paths.
 */
export function runPackagedK002ProbeOnce(): Promise<void> {
  if (!isTauriRuntime()) return Promise.resolve()
  if (!probePromise) probePromise = runPackagedK002Probe()
  return probePromise
}

async function runPackagedK002Probe(): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core')
  let config: ProbeConfig | null
  try {
    config = await invoke<ProbeConfig | null>('forgecad_k002_packaged_probe_config')
  } catch {
    return
  }
  if (config === null) return
  if (config.schema_version !== PROBE_SCHEMA || !['initial', 'restart'].includes(config.phase)) {
    await reportFailure(config?.phase === 'restart' ? 'restart' : 'initial', 'PROBE_CONFIG_INVALID')
    return
  }

  try {
    await waitForHealth()
    const supervisor = await readOwnedSupervisorStatus()
    const operation = config.phase === 'initial'
      ? runInitialProbe(supervisor)
      : runRestartProbe(requireExpected(config), supervisor)
    const report = await withTimeout(operation, PROBE_TIMEOUT_MS, 'PROBE_TOTAL_TIMEOUT')
    await invoke('forgecad_k002_packaged_probe_report', { report })
  } catch (error) {
    const errorCode = error instanceof Error && /^PROBE_[A-Z0-9_]{1,120}$/.test(error.message)
      ? error.message
      : 'PROBE_EXECUTION_FAILED'
    await reportFailure(config.phase, errorCode)
  }
}

async function reportFailure(phase: 'initial' | 'restart', errorCode: string): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core')
  const report: ProbeReport = {
    schema_version: PROBE_SCHEMA,
    phase,
    ok: false,
    error_code: errorCode,
  }
  await invoke('forgecad_k002_packaged_probe_report', { report }).catch(() => undefined)
}

async function runInitialProbe(supervisor: SupervisorProof): Promise<ProbeReport> {
  const api = new ForgeApiClient()
  const preflight = await readUnconfiguredPreflight()
  const thread = await api.createAgentThread({
    client_request_id: 'k002-packaged-native-thread',
    title: 'K002 packaged native lifecycle probe',
    provider_id: 'deepseek',
  })
  const turn = await api.startAgentTurn(thread.thread_id, {
    client_request_id: 'k002-packaged-native-turn',
    message: '为游戏场景设计一个非功能、可编辑的工业机械臂概念外观。',
  })
  assertFailedUnconfiguredTurn(turn)

  const authoritativeThread = await api.getAgentThread(thread.thread_id)
  const authoritativeTurn = requireTurn(authoritativeThread, turn.turn_id)
  assertFailedUnconfiguredTurn(authoritativeTurn)
  const replayed = await replayItems(api, thread.thread_id)
  const facts = await verifyLifecycleFacts(authoritativeThread, authoritativeTurn, replayed)
  const legacyStatus = await legacyLifecyclePostStatus()
  if (legacyStatus !== 410) throw new Error('PROBE_LEGACY_LIFECYCLE_NOT_GONE')

  return successReport('initial', authoritativeTurn, preflight, supervisor, facts, legacyStatus)
}

async function runRestartProbe(expected: ProbeExpected, supervisor: SupervisorProof): Promise<ProbeReport> {
  validateExpected(expected)
  const api = new ForgeApiClient()
  const preflight = await readUnconfiguredPreflight()
  const authoritativeThread = await api.getAgentThread(expected.thread_id)
  const authoritativeTurn = requireTurn(authoritativeThread, expected.turn_id)
  assertFailedUnconfiguredTurn(authoritativeTurn)
  const replayed = await replayItems(api, expected.thread_id)
  const facts = await verifyLifecycleFacts(authoritativeThread, authoritativeTurn, replayed)
  if (
    authoritativeTurn.error_code !== expected.turn_error_code
    || facts.itemsSha256 !== expected.items_sha256
    || facts.itemCount !== expected.item_count
    || facts.lastSequence !== expected.last_sequence
  ) {
    throw new Error('PROBE_RESTART_TRUTH_DIVERGED')
  }
  const legacyStatus = await legacyLifecyclePostStatus()
  if (legacyStatus !== 410) throw new Error('PROBE_LEGACY_LIFECYCLE_NOT_GONE')

  return successReport('restart', authoritativeTurn, preflight, supervisor, facts, legacyStatus)
}

function successReport(
  phase: 'initial' | 'restart',
  turn: AgentTurn,
  preflight: ProviderPreflight,
  supervisor: SupervisorProof,
  facts: Awaited<ReturnType<typeof verifyLifecycleFacts>>,
  legacyStatus: number,
): ProbeReport {
  return {
    schema_version: PROBE_SCHEMA,
    phase,
    ok: true,
    thread_id: turn.thread_id,
    turn_id: turn.turn_id,
    turn_status: turn.status,
    turn_error_code: turn.error_code ?? undefined,
    provider_status: preflight.status,
    provider_configured: preflight.configured,
    provider_network_call_made: preflight.networkCallMade,
    supervisor_running: supervisor.running,
    supervisor_state: supervisor.state,
    supervisor_managed_by_desktop: supervisor.managedByDesktop,
    reasoning_content_present: false,
    legacy_lifecycle_post_status: legacyStatus,
    provider_calls: 0,
    item_count: facts.itemCount,
    last_sequence: facts.lastSequence,
    item_sequences: facts.itemSequences,
    item_ids: facts.itemIds,
    item_types: facts.itemTypes,
    items_sha256: facts.itemsSha256,
    replay_items_sha256: facts.replayItemsSha256,
  }
}

async function verifyLifecycleFacts(
  thread: AgentThreadDetail,
  turn: AgentTurn,
  replayed: AgentEvent[],
): Promise<{
  itemCount: number
  lastSequence: number
  itemSequences: number[]
  itemIds: string[]
  itemTypes: string[]
  itemsSha256: string
  replayItemsSha256: string
}> {
  if (containsReasoningContent(thread) || containsReasoningContent(replayed)) {
    throw new Error('PROBE_REASONING_PERSISTED')
  }
  const providerRequests = turn.usage.provider_requests
  if (providerRequests !== undefined && providerRequests !== 0) {
    throw new Error('PROBE_PROVIDER_REQUEST_RECORDED')
  }
  const items = [...turn.items].sort((left, right) => left.sequence - right.sequence)
  const replayItems = replayed
    .filter((event) => event.turn_id === turn.turn_id)
    .sort((left, right) => left.sequence - right.sequence)
    .map((event) => event.item)
  if (items.length < 2 || replayItems.length !== items.length) {
    throw new Error('PROBE_ITEM_REPLAY_INCOMPLETE')
  }
  assertOrderedItems(items, turn)
  assertOrderedItems(replayItems, turn)
  const gateway = items.find((item) => (
    item.item_type === 'tool_result'
    && item.payload.tool_name === 'provider_gateway'
  ))
  const gatewayResult = isRecord(gateway?.payload.result) ? gateway.payload.result : null
  if (
    !gateway
    || gateway.status !== 'failed'
    || gatewayResult?.network_call_made !== false
    || gatewayResult.error_code !== turn.error_code
  ) {
    throw new Error('PROBE_PROVIDER_GATEWAY_FACT_MISSING')
  }
  const itemsSha256 = await sha256(canonicalJson(items))
  const replayItemsSha256 = await sha256(canonicalJson(replayItems))
  if (itemsSha256 !== replayItemsSha256) throw new Error('PROBE_ITEM_REPLAY_DIVERGED')
  return {
    itemCount: items.length,
    lastSequence: items.at(-1)?.sequence ?? 0,
    itemSequences: items.map((item) => item.sequence),
    itemIds: items.map((item) => item.item_id),
    itemTypes: items.map((item) => item.item_type),
    itemsSha256,
    replayItemsSha256,
  }
}

function assertOrderedItems(items: AgentItem[], turn: AgentTurn): void {
  const seenIds = new Set<string>()
  for (let index = 0; index < items.length; index += 1) {
    const item = items[index]
    if (
      item.thread_id !== turn.thread_id
      || item.turn_id !== turn.turn_id
      || item.sequence <= 0
      || (index > 0 && item.sequence <= items[index - 1].sequence)
      || seenIds.has(item.item_id)
    ) {
      throw new Error('PROBE_ITEM_ORDER_INVALID')
    }
    seenIds.add(item.item_id)
  }
  if (items[0]?.item_type !== 'user_message') throw new Error('PROBE_FIRST_ITEM_NOT_USER_MESSAGE')
}

async function replayItems(api: ForgeApiClient, threadId: string): Promise<AgentEvent[]> {
  const events: AgentEvent[] = []
  let stop: () => void = () => undefined
  const complete = new Promise<void>((resolve, reject) => {
    stop = api.subscribeAgentThreadEvents(threadId, {
      onEvent: (event) => events.push(event),
      onReplayComplete: resolve,
      onError: () => reject(new Error('PROBE_ITEM_REPLAY_FAILED')),
    }, 0)
  })
  try {
    await withTimeout(complete, REPLAY_TIMEOUT_MS, 'PROBE_ITEM_REPLAY_TIMEOUT')
  } finally {
    stop()
  }
  return events
}

async function readUnconfiguredPreflight(): Promise<ProviderPreflight> {
  const executionId = `probe_preflight_${crypto.randomUUID().replaceAll('-', '')}`
  const raw = await appServerTransport.nativeRequest<unknown>('provider/preflight', {
    schema_version: 'ProviderPreflightCommand@1',
    execution_id: executionId,
    requested_provider_id: 'deepseek',
  }, { retrySafe: true })
  if (!isRecord(raw)) throw new Error('PROBE_PREFLIGHT_CONTRACT_INVALID')
  const allowed = new Set([
    'schema_version',
    'execution_id',
    'status',
    'provider_id',
    'configured',
    'network_call_made',
    'failure_category',
  ])
  if (Object.keys(raw).some((key) => !allowed.has(key))) {
    throw new Error('PROBE_PREFLIGHT_CONTRACT_INVALID')
  }
  if (
    raw.schema_version !== 'ProviderPreflightResult@1'
    || raw.execution_id !== executionId
    || raw.status !== 'unconfigured'
    || raw.provider_id !== 'deepseek'
    || raw.configured !== false
    || raw.network_call_made !== false
    || (raw.failure_category !== undefined && raw.failure_category !== null)
  ) {
    throw new Error('PROBE_PREFLIGHT_NOT_UNCONFIGURED')
  }
  return { status: 'unconfigured', configured: false, networkCallMade: false }
}

async function readOwnedSupervisorStatus(): Promise<SupervisorProof> {
  const { invoke } = await import('@tauri-apps/api/core')
  const raw = await invoke<unknown>('agent_service_status')
  if (!isRecord(raw)) throw new Error('PROBE_SUPERVISOR_OWNERSHIP_NOT_VERIFIED')
  if (
    raw.running !== true
    || raw.state !== 'running'
    || raw.managed_by_desktop !== true
    || raw.mode !== 'packaged-sidecar'
    || (raw.last_error !== undefined && raw.last_error !== null)
  ) {
    throw new Error('PROBE_SUPERVISOR_OWNERSHIP_NOT_VERIFIED')
  }
  return { running: true, state: 'running', managedByDesktop: true }
}

async function legacyLifecyclePostStatus(): Promise<number> {
  const response = await appServerTransport.request('/api/v1/agent/threads', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Idempotency-Key': 'k002-packaged-legacy-tombstone',
    },
    body: JSON.stringify({}),
    cache: 'no-store',
  })
  return response.status
}

function requireTurn(thread: AgentThreadDetail, turnId: string): AgentTurn {
  const turn = thread.turns.find((candidate) => candidate.turn_id === turnId)
  if (!turn) throw new Error('PROBE_TURN_NOT_PERSISTED')
  return turn
}

function assertFailedUnconfiguredTurn(turn: AgentTurn): void {
  if (
    turn.status !== 'failed'
    || turn.error_code !== 'PROVIDER_NOT_CONFIGURED'
    || turn.items.length < 2
  ) {
    throw new Error('PROBE_TURN_NOT_EXPLICITLY_FAILED')
  }
}

function requireExpected(config: ProbeConfig): ProbeExpected {
  if (config.phase !== 'restart' || !config.expected) throw new Error('PROBE_CONFIG_INVALID')
  validateExpected(config.expected)
  return config.expected
}

function validateExpected(expected: ProbeExpected): void {
  if (
    !isStableId(expected.thread_id)
    || !isStableId(expected.turn_id)
    || !isStableId(expected.turn_error_code)
    || !SHA256_PATTERN.test(expected.items_sha256)
    || !Number.isSafeInteger(expected.item_count)
    || expected.item_count < 2
    || expected.item_count > 200
    || !Number.isSafeInteger(expected.last_sequence)
    || expected.last_sequence <= 0
  ) {
    throw new Error('PROBE_CONFIG_INVALID')
  }
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

function containsReasoningContent(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(containsReasoningContent)
  if (!isRecord(value)) return false
  return Object.entries(value).some(([key, nested]) => (
    key === 'reasoning_content' || containsReasoningContent(nested)
  ))
}

function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  if (isRecord(value)) {
    return `{${Object.keys(value).sort().map((key) => (
      `${JSON.stringify(key)}:${canonicalJson(value[key])}`
    )).join(',')}}`
  }
  return JSON.stringify(value)
}

async function sha256(value: string): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(value))
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('')
}

async function waitUntil(predicate: () => boolean | Promise<boolean>, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    if (await predicate()) return
    await delay(100)
  }
  throw new Error('PROBE_HEALTH_TIMEOUT')
}

async function withTimeout<T>(operation: Promise<T>, timeoutMs: number, code: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | null = null
  try {
    return await Promise.race([
      operation,
      new Promise<never>((_resolve, reject) => {
        timer = setTimeout(() => reject(new Error(code)), timeoutMs)
      }),
    ])
  } finally {
    if (timer !== null) clearTimeout(timer)
  }
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds))
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isStableId(value: unknown): value is string {
  return typeof value === 'string' && /^[A-Za-z0-9_.-]{1,160}$/.test(value)
}
