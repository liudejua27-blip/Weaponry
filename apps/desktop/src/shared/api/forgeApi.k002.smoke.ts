import { ForgeApiClient, ForgeApiError } from './forgeApi.js'
import { AppServerProtocolError } from './appServerProtocol.js'
import { isNativeDesktopRuntime } from './appServerTransport.js'

type Frame = {
  jsonrpc: '2.0'
  id?: string
  method: string
  params?: Record<string, unknown>
}

type TauriEvent = { event: string; id: number; payload: unknown }

type AgentItemFixture = {
  item_id: string
  thread_id: string
  turn_id: string
  sequence: number
  item_type: string
  status: string
  payload: Record<string, unknown>
  created_at: string
}

type AgentApprovalFixture = {
  approval_id: string
  thread_id: string
  turn_id: string
  item_id: string
  action: string
  status: string
  payload: Record<string, unknown>
  created_at: string
  resolved_at?: string | null
}

type AgentTurnFixture = {
  turn_id: string
  thread_id: string
  request_text: string
  status: string
  error_code?: string | null
  error_message?: string | null
  usage: Record<string, unknown>
  created_at: string
  updated_at: string
  items: AgentItemFixture[]
  approvals: AgentApprovalFixture[]
}

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message)
}

function clone<T>(value: T): T {
  return structuredClone(value)
}

export async function runK002NativeForgeApiSmoke(): Promise<void> {
  const originalWindow = Object.getOwnPropertyDescriptor(globalThis, 'window')
  const callbacks = new Map<number, (payload: TauriEvent) => void>()
  const listeners = new Map<number, { event: string; callbackId: number }>()
  const activeConnections = new Set<string>()
  const methods: string[] = []
  const commands = new Map<string, Record<string, unknown>[]>()
  let nextCallbackId = 1
  let nextListenerId = 1
  let nextConnectionId = 1
  let invalidThreadList = false
  let providerCheckMode: 'ready' | 'pending_cancel' = 'ready'
  let finishPendingProviderCheck: ((value: Record<string, unknown>) => void) | null = null
  let pendingProviderRequestId: string | null = null
  let terminalReplayTurn: AgentTurnFixture | null = null
  let terminalReplayResponseLost = false
  let cancellationTransitionTurnId: string | null = null
  let cancellationPendingReadObserved = false
  let transientReplayAdapterUnavailableBudget = 0
  let persistentReplayContractFailureBudget = 0
  let nativeItemListRequestCount = 0

  const threadId = 'thread_k002_frontend'
  const historicalTurn: AgentTurnFixture = {
    turn_id: 'turn_k002_history',
    thread_id: threadId,
    request_text: '历史外观概念',
    status: 'completed',
    usage: {},
    created_at: '2026-07-17T00:00:00Z',
    updated_at: '2026-07-17T00:00:01Z',
    items: [{
      item_id: 'item_k002_history_1',
      thread_id: threadId,
      turn_id: 'turn_k002_history',
      sequence: 1,
      item_type: 'assistant_message',
      status: 'completed',
      payload: { text: '已恢复历史结果' },
      created_at: '2026-07-17T00:00:01Z',
    }],
    approvals: [],
  }
  const turns: AgentTurnFixture[] = [historicalTurn]

  const threadSummary = (): Record<string, unknown> => ({
    thread_id: threadId,
    project_id: 'project_k002_frontend',
    title: '未来机械概念',
    status: 'active',
    summary: '桌面原生生命周期',
    provider_id: 'deepseek',
    created_at: '2026-07-17T00:00:00Z',
    updated_at: '2026-07-17T00:00:10Z',
    last_turn_id: turns.at(-1)?.turn_id ?? null,
  })
  const threadDetail = (): Record<string, unknown> => ({ ...threadSummary(), turns: clone(turns) })

  const success = (id: string, result: unknown): Record<string, unknown> => ({ jsonrpc: '2.0', id, result })
  const failure = (
    id: string,
    applicationCode: string,
    message: string,
    recoverable = false,
  ): Record<string, unknown> => ({
    jsonrpc: '2.0',
    id,
    error: {
      code: -32010,
      message,
      data: {
        schema_version: 'ForgeCADProtocolError@1',
        application_code: applicationCode,
        recoverable,
        details: {},
      },
    },
  })
  const commandResult = (
    schemaVersion: string,
    commandId: unknown,
    result: Record<string, unknown>,
  ): Record<string, unknown> => ({ schema_version: schemaVersion, command_id: commandId, result })

  const emit = (connectionId: string, method: string, params: Record<string, unknown>): void => {
    for (const { event, callbackId } of listeners.values()) {
      if (event !== 'forgecad://app-server/message') continue
      callbacks.get(callbackId)?.({
        event,
        id: 1,
        payload: {
          connection_id: connectionId,
          frame: {
            jsonrpc: '2.0',
            method,
            params,
            notification_id: params.notification_id,
            cursor: params.cursor,
          },
        },
      })
    }
  }

  const nativeNotification = (
    event: string,
    sequence: number,
    turn: AgentTurnFixture,
    extra: Record<string, unknown>,
  ): Record<string, unknown> => ({
    schema_version: 'NativeAgentNotification@1',
    notification_id: `notification_${event}_${sequence}`,
    cursor: `fc1_${event}_${sequence}`,
    sequence,
    thread_id: turn.thread_id,
    turn_id: turn.turn_id,
    ...extra,
  })

  const recordCommand = (method: string, params: Record<string, unknown>): void => {
    const entries = commands.get(method) ?? []
    entries.push(clone(params))
    commands.set(method, entries)
  }

  const handleNativeMethod = async (
    connectionId: string,
    frame: Frame,
  ): Promise<Record<string, unknown>> => {
    const params = frame.params ?? {}
    recordCommand(frame.method, params)
    const command = (params.command ?? {}) as Record<string, unknown>
    const request = (command.request ?? {}) as Record<string, unknown>
    if (frame.method === 'thread/create') {
      assert(command.operation === 'create', 'thread/create operation drifted')
      return success(frame.id ?? '', commandResult('AgentThreadCommandResult@1', params.command_id, {
        outcome: 'thread',
        thread: threadDetail(),
      }))
    }
    if (frame.method === 'thread/list') {
      const result = commandResult('AgentThreadCommandResult@1', params.command_id, {
        outcome: 'threads',
        threads: [threadSummary()],
      })
      if (invalidThreadList) result.unexpected = true
      return success(frame.id ?? '', result)
    }
    if (frame.method === 'thread/read') {
      assert(command.thread_id === threadId, 'thread/read identity drifted')
      return success(frame.id ?? '', commandResult('AgentThreadCommandResult@1', params.command_id, {
        outcome: 'thread',
        thread: threadDetail(),
      }))
    }
    if (frame.method === 'item/list') {
      nativeItemListRequestCount += 1
      if (transientReplayAdapterUnavailableBudget > 0) {
        transientReplayAdapterUnavailableBudget -= 1
        return failure(
          frame.id ?? '',
          'ADAPTER_UNAVAILABLE',
          'The packaged restricted adapter is between its readiness and first native replay request.',
          true,
        )
      }
      const turn = turns.find((candidate) => candidate.turn_id === command.turn_id)
      const after = Number(command.after_sequence)
      const items = (turn?.items ?? []).filter((item) => item.sequence > after)
      const result = commandResult('AgentItemCommandResult@1', params.command_id, {
        outcome: 'items',
        items: clone(items),
      })
      if (persistentReplayContractFailureBudget > 0) {
        persistentReplayContractFailureBudget -= 1
        result.unexpected = true
      }
      return success(frame.id ?? '', result)
    }
    if (frame.method === 'turn/read') {
      const turn = turns.find((candidate) => candidate.turn_id === command.turn_id)
      assert(turn, 'turn/read requested an unknown Turn')
      if (turn.turn_id === cancellationTransitionTurnId && turn.status === 'running') {
        cancellationPendingReadObserved = true
      }
      return success(frame.id ?? '', commandResult('AgentTurnCommandResult@1', params.command_id, {
        outcome: 'turn',
        turn: clone(turn),
      }))
    }
    if (frame.method === 'turn/start') {
      if (request.client_request_id === 'client_turn_native_response_lost') {
        if (!terminalReplayTurn) {
          terminalReplayTurn = {
            turn_id: 'turn_k002_terminal_replay',
            thread_id: threadId,
            request_text: String(request.message),
            status: 'failed',
            error_code: 'PROVIDER_NOT_CONFIGURED',
            error_message: 'Provider is not configured.',
            usage: {},
            created_at: '2026-07-17T00:00:30Z',
            updated_at: '2026-07-17T00:00:31Z',
            items: [{
              item_id: 'item_k002_terminal_replay_user',
              thread_id: threadId,
              turn_id: 'turn_k002_terminal_replay',
              sequence: 6,
              item_type: 'user_message',
              status: 'completed',
              payload: { text: String(request.message) },
              created_at: '2026-07-17T00:00:30Z',
            }],
            approvals: [],
          }
          turns.push(terminalReplayTurn)
        }
        return success(frame.id ?? '', commandResult('AgentTurnCommandResult@1', params.command_id, {
          outcome: 'turn',
          turn: clone(terminalReplayTurn),
        }))
      }
      if (request.client_request_id === 'client_turn_native_invalid_replay') {
        return success(frame.id ?? '', commandResult('AgentTurnCommandResult@1', params.command_id, {
          outcome: 'turn',
          turn: {
            turn_id: 'turn_k002_invalid_running_replay',
            thread_id: threadId,
            request_text: String(request.message),
            status: 'running',
            usage: {},
            created_at: '2026-07-17T00:00:32Z',
            updated_at: '2026-07-17T00:00:32Z',
            items: [],
            approvals: [],
          },
        }))
      }
      const turnNumber = turns.length
      const turn: AgentTurnFixture = {
        turn_id: `turn_k002_native_${turnNumber}`,
        thread_id: threadId,
        request_text: String(request.message),
        status: 'running',
        usage: {},
        created_at: `2026-07-17T00:00:${10 + turnNumber}Z`,
        updated_at: `2026-07-17T00:00:${10 + turnNumber}Z`,
        items: [],
        approvals: [],
      }
      turns.push(turn)
      const cancellationId = `turn_cancel_${turnNumber}`
      const cancellationToken = `turn_cancel_token_${turnNumber}`
      globalThis.setTimeout(() => {
        const item: AgentItemFixture = {
          item_id: `item_native_${turnNumber}_user`,
          thread_id: threadId,
          turn_id: turn.turn_id,
          sequence: turnNumber === 1 ? 2 : 5,
          item_type: 'user_message',
          status: 'completed',
          payload: { text: turn.request_text },
          created_at: turn.updated_at,
        }
        turn.items.push(item)
        emit(connectionId, 'item/updated', nativeNotification('item', item.sequence, turn, {
          item_id: item.item_id,
          payload: { event: 'item_updated', item: clone(item) },
        }))
        emit(connectionId, 'item/updated', {
          ...nativeNotification('other', item.sequence + 100, { ...turn, thread_id: 'thread_other' }, {
            item_id: 'item_other_thread',
            payload: {
              event: 'item_updated',
              item: { ...clone(item), item_id: 'item_other_thread', thread_id: 'thread_other', sequence: item.sequence + 100 },
            },
          }),
        })
      }, 0)
      if (turnNumber === 1) {
        globalThis.setTimeout(() => {
          const item: AgentItemFixture = {
            item_id: 'item_native_1_plan',
            thread_id: threadId,
            turn_id: turn.turn_id,
            sequence: 3,
            item_type: 'plan',
            status: 'completed',
            payload: { plan_id: 'plan_native_1' },
            created_at: '2026-07-17T00:00:13Z',
          }
          turn.items.push(item)
          emit(connectionId, 'item/updated', nativeNotification('item', item.sequence, turn, {
            item_id: item.item_id,
            payload: { event: 'item_updated', item: clone(item) },
          }))
          turn.status = 'completed'
          turn.updated_at = '2026-07-17T00:00:14Z'
          emit(connectionId, 'turn/completed', nativeNotification('turn_terminal', item.sequence, turn, {
            payload: { event: 'turn_completed', turn: clone(turn) },
          }))
        }, 15)
      }
      return success(frame.id ?? '', commandResult('AgentTurnCommandResult@1', params.command_id, {
        outcome: 'started',
        turn: clone(turn),
        cancellation_id: cancellationId,
        cancellation_token: cancellationToken,
      }))
    }
    if (frame.method === 'turn/cancel') {
      const turn = turns.find((candidate) => candidate.turn_id === command.turn_id)
      assert(turn, 'turn/cancel requested an unknown Turn')
      const startCommand = commands.get('turn/start')?.at(-1)
      const startResultNumber = turns.length - 1
      assert(command.cancellation_id === `turn_cancel_${startResultNumber}`, 'turn cancel ID did not come from started result')
      assert(command.cancellation_token === `turn_cancel_token_${startResultNumber}`, 'turn cancel token did not stay in memory')
      assert(startCommand, 'turn/cancel ran without turn/start')
      cancellationTransitionTurnId = turn.turn_id
      // A cancellation command is only an accepted intent. Persistence of the
      // authoritative terminal Turn may linearize after the command response.
      // This catches clients that do one immediate read and return `running`.
      globalThis.setTimeout(() => {
        turn.status = 'cancelled'
        turn.updated_at = '2026-07-17T00:00:20Z'
        emit(connectionId, 'turn/cancelled', nativeNotification('turn_cancelled', turn.items.at(-1)?.sequence ?? 5, turn, {
          payload: { event: 'turn_cancelled', turn: clone(turn) },
        }))
      }, 25)
      return success(frame.id ?? '', commandResult('AgentTurnCommandResult@1', params.command_id, {
        outcome: 'cancellation_accepted',
        thread_id: threadId,
        turn_id: turn.turn_id,
        cancellation_id: command.cancellation_id,
        accepted: true,
      }))
    }
    if (frame.method === 'approval/create') {
      return failure(
        frame.id ?? '',
        'AGENT_APPROVAL_NOT_RUNTIME_REQUESTED',
        'Approvals can only be created by a suspended Rust ActionLoop tool continuation.',
      )
    }
    if (frame.method === 'approval/resolve') {
      const turn = turns.find((candidate) => candidate.turn_id === command.turn_id)
      const approval = turn?.approvals.find((candidate) => candidate.approval_id === command.approval_id)
      assert(turn && approval, 'approval/resolve requested an unknown Approval')
      approval.status = String(request.decision)
      approval.resolved_at = '2026-07-17T00:00:16Z'
      const item = turn.items.find((candidate) => candidate.item_id === approval.item_id)
      if (item) item.status = 'completed'
      turn.status = 'completed'
      turn.updated_at = '2026-07-17T00:00:16Z'
      return success(frame.id ?? '', commandResult('AgentApprovalCommandResult@1', params.command_id, {
        outcome: 'approval',
        approval: clone(approval),
      }))
    }
    if (frame.method === 'provider/preflight') {
      return success(frame.id ?? '', {
        schema_version: 'ProviderPreflightResult@1',
        execution_id: params.execution_id,
        status: 'ready',
        provider_id: 'deepseek',
        configured: true,
        network_call_made: false,
      })
    }
    if (frame.method === 'provider/check') {
      if (providerCheckMode === 'ready') {
        return success(frame.id ?? '', {
          schema_version: 'ProviderCheckResult@1',
          execution_id: params.execution_id,
          provider_id: 'deepseek',
          status: 'ready',
          network_call_made: true,
          usage: {
            input_tokens: 8,
            output_tokens: 1,
            prompt_cache_hit_tokens: 0,
            prompt_cache_miss_tokens: 8,
          },
        })
      }
      pendingProviderRequestId = frame.id ?? null
      return new Promise((resolve) => { finishPendingProviderCheck = resolve })
    }
    if (frame.method === 'provider/cancel') {
      const pending = commands.get('provider/check')?.at(-1)
      assert(pending?.cancellation_id === params.cancellation_id, 'provider cancel ID drifted')
      assert(pending?.cancellation_token === params.cancellation_token, 'provider cancel token did not stay in memory')
      finishPendingProviderCheck?.(success(pendingProviderRequestId ?? '', {
        schema_version: 'ProviderCheckResult@1',
        execution_id: params.execution_id,
        provider_id: 'deepseek',
        status: 'cancelled',
        network_call_made: true,
        failure_category: 'cancelled',
      }))
      finishPendingProviderCheck = null
      pendingProviderRequestId = null
      return success(frame.id ?? '', {
        schema_version: 'ProviderCancelResult@1',
        execution_id: params.execution_id,
        cancellation_id: params.cancellation_id,
        accepted: true,
        already_terminal: false,
      })
    }
    throw new Error(`Unexpected native method ${frame.method}`)
  }

  const invoke = async (command: string, rawArgs: unknown): Promise<unknown> => {
    const args = (rawArgs ?? {}) as Record<string, unknown>
    if (command === 'plugin:event|listen') {
      const callbackId = Number(args.handler)
      const listenerId = nextListenerId++
      listeners.set(listenerId, { event: String(args.event), callbackId })
      return listenerId
    }
    if (command === 'plugin:event|unlisten') return null
    if (command === 'forgecad_protocol_connect') {
      const connectionId = `conn_k002_frontend_${nextConnectionId++}`
      activeConnections.add(connectionId)
      return { connection_id: connectionId }
    }
    if (command === 'forgecad_protocol_disconnect') {
      const request = args.request as { connection_id: string }
      activeConnections.delete(request.connection_id)
      return { disconnected: true }
    }
    if (command !== 'forgecad_protocol_send') throw new Error(`Unexpected Tauri command ${command}`)
    const request = args.request as { connection_id: string; frame: Frame }
    assert(activeConnections.has(request.connection_id), 'native request used a closed connection')
    const frame = request.frame
    methods.push(frame.method)
    if (frame.method === 'initialize') {
      return success(frame.id ?? '', {
        schema_version: 'ForgeCADInitializeResult@1',
        protocol_version: 'forgecad.app-server/1',
        connection_id: request.connection_id,
        server_info: { name: 'k002-native-frontend-smoke', version: '1' },
        capabilities: {
          notifications: true,
          cursor_replay: true,
          cancellation: true,
          notification_ack: true,
          binary_body_base64: true,
        },
        limits: { max_in_flight_requests: 32, max_event_queue: 128, max_frame_bytes: 64 * 1024 * 1024 },
        // K001's legacy initialize facet remains frozen until K003; K002
        // publishes Rust lifecycle ownership through migration/ownership/read.
        migration_state: { state_owner: 'python_compatibility_adapter' },
      })
    }
    if (frame.method === 'initialized' || frame.method === 'notification/ack') return { accepted: true }
    const output = await handleNativeMethod(request.connection_id, frame)
    const nativeCommand = (frame.params?.command ?? {}) as Record<string, unknown>
    const commandRequest = (nativeCommand.request ?? {}) as Record<string, unknown>
    if (
      frame.method === 'turn/start'
      && commandRequest.client_request_id === 'client_turn_native_response_lost'
      && !terminalReplayResponseLost
    ) {
      terminalReplayResponseLost = true
      throw new Error('simulated response loss after terminal Turn persistence')
    }
    return output
  }

  Object.defineProperty(globalThis, 'window', {
    configurable: true,
    writable: true,
    value: {
      location: { protocol: 'tauri:' },
      localStorage: {
        getItem: () => { throw new Error('native cancellation state must not read localStorage') },
        setItem: () => { throw new Error('native cancellation state must not write localStorage') },
      },
      __TAURI_INTERNALS__: {
        invoke,
        transformCallback: (callback: (payload: TauriEvent) => void) => {
          const callbackId = nextCallbackId++
          callbacks.set(callbackId, callback)
          return callbackId
        },
        unregisterCallback: (callbackId: number) => { callbacks.delete(callbackId) },
      },
      __TAURI_EVENT_PLUGIN_INTERNALS__: {
        unregisterListener: (_event: string, listenerId: number) => {
          const listener = listeners.get(listenerId)
          if (listener) callbacks.delete(listener.callbackId)
          listeners.delete(listenerId)
        },
      },
    },
  })

  try {
    assert(isNativeDesktopRuntime(), 'Tauri WebView must select the native lifecycle runtime')
    const api = new ForgeApiClient()
    const created = await api.createAgentThread({
      client_request_id: 'client_thread_native_1',
      project_id: 'project_k002_frontend',
      title: '未来机械概念',
      provider_id: 'deepseek',
    })
    assert(created.thread_id === threadId, 'thread/create did not map the native detail')
    const listed = await api.listAgentThreads()
    assert(listed.items.length === 1 && listed.items[0].thread_id === threadId, 'thread/list did not map native summaries')
    const read = await api.getAgentThread(threadId)
    assert(read.turns[0].items[0].sequence === 1, 'thread/read did not validate nested Items')

    const streamed: number[] = []
    let replayCompleted = false
    const unsubscribe = api.subscribeAgentThreadEvents(threadId, {
      onEvent: (event) => { streamed.push(event.sequence) },
      onReplayComplete: () => { replayCompleted = true },
      onError: (event) => { throw new Error(`native event subscription failed: ${String((event as Event & { message?: string }).message)}`) },
    })
    await waitUntil(() => replayCompleted)
    assert(streamed.join(',') === '1', 'native item/list replay must finish before live delivery')

    const completed = await api.startAgentTurn(threadId, {
      client_request_id: 'client_turn_native_1',
      message: '生成精细的非功能未来机械概念',
    })
    assert(completed.status === 'completed', 'turn/start must wait for an authoritative readable Turn state')
    assert(completed.items.map((item) => item.sequence).join(',') === '2,3', 'turn/read lost native Item ordering')
    await waitUntil(() => streamed.includes(3))
    assert(streamed.join(',') === '1,2,3', 'native item notifications were not mapped to AgentEvent order')
    unsubscribe()

    // A restart can expose an already healthy Rust shell while the restricted
    // sidecar rejects the first read-only native replay. The client may make
    // one new connection and replay the exact same persisted interval once.
    const transientReplayItemListStart = nativeItemListRequestCount
    const transientReplayConnectionStart = nextConnectionId
    transientReplayAdapterUnavailableBudget = 2
    const recoveredSequences: number[] = []
    let recoveredReplayComplete = false
    let recoveredReplayError: Event | null = null
    const recoveredUnsubscribe = api.subscribeAgentThreadEvents(threadId, {
      onEvent: (event) => { recoveredSequences.push(event.sequence) },
      onReplayComplete: () => { recoveredReplayComplete = true },
      onError: (event) => { recoveredReplayError = event },
    })
    await waitUntil(() => recoveredReplayComplete || recoveredReplayError !== null)
    recoveredUnsubscribe()
    assert(recoveredReplayError === null, 'one ADAPTER_UNAVAILABLE replay transient must recover without surfacing an error')
    assert(recoveredSequences.join(',') === '1,2,3', 'bounded replay recovery must preserve the full ordered persisted interval')
    assert(transientReplayAdapterUnavailableBudget === 0, 'replay smoke did not consume both adapter transient responses')
    assert(
      nativeItemListRequestCount > transientReplayItemListStart
        && nativeItemListRequestCount <= transientReplayItemListStart + 4,
      `native replay recovery exceeded its request retry plus one bounded reconnect/replay; start=${transientReplayItemListStart} actual=${nativeItemListRequestCount}`,
    )
    assert(
      nextConnectionId === transientReplayConnectionStart + 2,
      `native replay recovery opened more than the request retry and one bounded reconnect/replay; expected=${transientReplayConnectionStart + 3} actual=${nextConnectionId}`,
    )

    // A closed-result failure is authoritative evidence of a client/server
    // contract regression, not a retryable transport outage. It must reach
    // the caller with a stable code after the first failed read.
    const persistentReplayItemListStart = nativeItemListRequestCount
    const persistentReplayConnectionStart = nextConnectionId
    persistentReplayContractFailureBudget = 1
    let persistentReplayComplete = false
    let persistentReplayError: Event | null = null
    const persistentUnsubscribe = api.subscribeAgentThreadEvents(threadId, {
      onEvent: () => undefined,
      onReplayComplete: () => { persistentReplayComplete = true },
      onError: (event) => { persistentReplayError = event },
    })
    await waitUntil(() => persistentReplayError !== null)
    persistentUnsubscribe()
    if (persistentReplayError === null) throw new Error('persistent replay smoke did not capture its failure')
    assert(!persistentReplayComplete, 'closed native replay contract failure must not report completion')
    assert(
      (persistentReplayError as Event & { forgecad_error_code?: unknown }).forgecad_error_code === 'NATIVE_AGENT_PROTOCOL_INVALID',
      'closed native replay contract failure must preserve its stable first error code',
    )
    assert(nativeItemListRequestCount === persistentReplayItemListStart + 1, 'closed native replay contract failure must not retry item/list')
    assert(nextConnectionId === persistentReplayConnectionStart, 'closed native replay contract failure must not reconnect')

    let forgedApprovalRejected = false
    try {
      await api.createAgentApproval(threadId, {
        client_request_id: 'client_approval_native_1',
        turn_id: completed.turn_id,
        action: 'confirm_candidate',
        payload: { permanent_side_effects: 0 },
      })
    } catch (error) {
      forgedApprovalRejected = error instanceof AppServerProtocolError
        && error.error.data?.application_code === 'AGENT_APPROVAL_NOT_RUNTIME_REQUESTED'
    }
    assert(forgedApprovalRejected, 'external clients must not fabricate an Approval after a terminal Turn')

    const cancellingStart = api.startAgentTurn(threadId, {
      client_request_id: 'client_turn_native_cancel',
      message: '取消这次概念生成',
    })
    await waitUntil(() => (commands.get('turn/start')?.length ?? 0) >= 2)
    await waitUntil(() => turns.at(-1)?.items.length === 1)
    const cancellingTurnId = turns.at(-1)?.turn_id
    assert(cancellingTurnId, 'cancel smoke did not create a Turn')
    const cancelledRead = await api.cancelAgentTurn(cancellingTurnId, 'client_turn_cancel_native')
    const cancelledStarted = await cancellingStart
    assert(cancelledRead.status === 'cancelled' && cancelledStarted.status === 'cancelled', 'turn/cancel did not converge on authoritative cancelled state')
    assert(cancellationPendingReadObserved, 'cancel smoke did not exercise accepted-before-terminal persistence ordering')

    const readyProvider = await api.checkAgentProvider('provider_check_ready')
    assert(readyProvider.status === 'ready' && readyProvider.network_call_made, 'provider/check ready result mapping drifted')
    assert(
      (commands.get('provider/preflight')?.length ?? 0) === 0,
      'explicit provider check must not open a second credential preflight',
    )
    providerCheckMode = 'pending_cancel'
    const pendingProvider = api.checkAgentProvider('provider_check_cancel')
    await waitUntil(() => (commands.get('provider/check')?.length ?? 0) >= 2)
    const providerCancel = await api.cancelAgentProviderCheck('provider_check_cancel')
    const cancelledProvider = await pendingProvider
    assert(providerCancel.cancel_requested, 'provider/cancel did not report accepted cancellation')
    assert(cancelledProvider.status === 'cancelled', 'cancelled provider/check result mapping drifted')

    const responseLostRequest = {
      client_request_id: 'client_turn_native_response_lost',
      message: '响应丢失后以相同幂等键读取终态',
    }
    let responseLossObserved = false
    try {
      await api.startAgentTurn(threadId, responseLostRequest)
    } catch (error) {
      responseLossObserved = error instanceof Error
        && error.message.includes('simulated response loss')
    }
    assert(responseLossObserved, 'smoke did not simulate a lost terminal turn/start response')
    const terminalReplay = await api.startAgentTurn(threadId, responseLostRequest)
    assert(
      terminalReplay.turn_id === 'turn_k002_terminal_replay'
      && terminalReplay.status === 'failed',
      'turn/start idempotent replay did not accept the persisted terminal Turn outcome',
    )
    let replayCancellationRejected = false
    try {
      await api.cancelAgentTurn(terminalReplay.turn_id, 'client_turn_native_replay_cancel')
    } catch (error) {
      replayCancellationRejected = error instanceof ForgeApiError
        && error.code === 'TURN_CANCELLATION_NOT_AVAILABLE'
    }
    assert(replayCancellationRejected, 'terminal turn/start replay must not cache a cancellation capability')

    let runningReplayRejected = false
    try {
      await api.startAgentTurn(threadId, {
        client_request_id: 'client_turn_native_invalid_replay',
        message: '拒绝没有取消能力的非终态 replay',
      })
    } catch (error) {
      runningReplayRejected = error instanceof ForgeApiError
        && error.code === 'NATIVE_AGENT_PROTOCOL_INVALID'
    }
    assert(runningReplayRejected, 'turn/start must reject non-terminal replay without cancellation capability')

    invalidThreadList = true
    let malformedRejected = false
    try {
      await api.listAgentThreads()
    } catch (error) {
      malformedRejected = error instanceof ForgeApiError && error.code === 'NATIVE_AGENT_PROTOCOL_INVALID'
    }
    assert(malformedRejected, 'native closed result guards must reject unknown envelope fields')
    assert(!methods.includes('compat/http') && !methods.includes('compat/subscribe'), 'Tauri lifecycle bypassed native Rust methods')
    assert(methods.includes('item/list'), 'native subscription did not perform item/list replay')
  } finally {
    if (originalWindow) Object.defineProperty(globalThis, 'window', originalWindow)
    else Reflect.deleteProperty(globalThis, 'window')
  }
}

async function waitUntil(predicate: () => boolean, timeoutMs = 2_000): Promise<void> {
  const deadline = Date.now() + timeoutMs
  while (!predicate()) {
    if (Date.now() >= deadline) throw new Error('timed out waiting for the native ForgeApi smoke condition')
    await new Promise((resolve) => globalThis.setTimeout(resolve, 5))
  }
}
