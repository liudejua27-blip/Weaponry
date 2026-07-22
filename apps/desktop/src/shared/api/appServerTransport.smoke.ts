import {
  AppServerProtocolError,
  createProtocolRequest,
  nextProtocolRequestId,
  unwrapJsonRpcResponse,
} from './appServerProtocol.js'
import { AppServerTransport } from './appServerTransport.js'

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message)
}

export async function runAppServerTransportSmoke(baseUrl: string): Promise<void> {
  const firstId = nextProtocolRequestId()
  const secondId = nextProtocolRequestId()
  assert(firstId.startsWith('req_'), 'protocol request IDs must use the stable req_ namespace')
  assert(firstId !== secondId, 'protocol request IDs must be unique')

  const fixed = createProtocolRequest('smoke/fixed-id', { ok: true }, 'req_smoke_fixed')
  assert(fixed.id === 'req_smoke_fixed', 'callers must be able to keep a request ID stable across retry')
  assert(
    unwrapJsonRpcResponse({ jsonrpc: '2.0', id: fixed.id, result: 'ok' }, fixed.id) === 'ok',
    'matching JSON-RPC responses must unwrap',
  )
  let mismatchRejected = false
  try {
    unwrapJsonRpcResponse({ jsonrpc: '2.0', id: 'req_wrong', result: null }, fixed.id)
  } catch (error) {
    mismatchRejected = error instanceof AppServerProtocolError
      && error.error.data?.application_code === 'RESPONSE_ID_MISMATCH'
  }
  assert(mismatchRejected, 'mismatched JSON-RPC response IDs must be rejected')

  for (const forbidden of [
    'https://127.0.0.1:8000',
    'http://example.com:8000',
    'http://127.0.0.1',
    'http://user@127.0.0.1:8000',
    'http://127.0.0.1:8000/api',
  ]) {
    let rejected = false
    try {
      new AppServerTransport().configureBrowserBaseUrl(forbidden)
    } catch (error) {
      rejected = error instanceof TypeError
    }
    assert(rejected, `browser compatibility endpoint must reject ${forbidden}`)
  }

  const transport = new AppServerTransport()
  transport.configureBrowserBaseUrl(baseUrl)
  try {
    let missingCapabilityRejected = false
    try {
      await transport.initialize()
    } catch (error) {
      missingCapabilityRejected = error instanceof AppServerProtocolError
        && error.error.code === -32013
        && error.error.data?.application_code === 'CAPABILITY_UNSUPPORTED'
    }
    assert(missingCapabilityRejected, 'initialize must reject a missing required server capability')

    // A failed handshake must have closed the first connection. initialize()
    // must therefore open and negotiate a fresh connection here.
    const initialized = await transport.initialize()
    const initializedAgain = await transport.initialize()
    assert(initialized === initializedAgain, 'transport initialization result must be memoized')
    assert(initialized.schema_version === 'ForgeCADInitializeResult@1', 'initialize schema must be validated')
    assert(initialized.protocol_version === 'forgecad.app-server/1', 'initialize protocol version must be validated')
    assert(/^conn_mock_[A-Za-z0-9_.-]+$/.test(initialized.connection_id), 'connection_id must be stable and non-empty')
    assert(initialized.capabilities.notification_ack === true, 'all required capabilities must be true')
    assert(initialized.limits?.max_event_queue === 128, 'negotiated limits must survive strict validation')
    assert(
      initialized.migration_state?.state_owner === 'python_compatibility_adapter',
      'browser development must report the real Python compatibility adapter handshake',
    )

    const jsonResponse = await transport.request(`${baseUrl}/api/v1/k001/json?source=transport`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Client-Request-Id': 'K001' },
      body: JSON.stringify({ detail: 'round-trip' }),
    })
    assert(jsonResponse.status === 201, 'HTTP compatibility status must survive the protocol round-trip')
    const json = await jsonResponse.json() as {
      body: { detail: string }
      header: string | null
      query: string | null
    }
    assert(json.body.detail === 'round-trip', 'UTF-8 request and response bodies must round-trip')
    assert(json.header === 'K001', 'HTTP compatibility headers must round-trip')
    assert(json.query === 'transport', 'absolute compatibility URLs must normalize to path plus query')

    const binaryResponse = await transport.request('/api/v1/k001/binary')
    assert(binaryResponse.headers.get('content-type') === 'model/gltf-binary', 'binary content type must survive')
    const binary = new Uint8Array(await binaryResponse.arrayBuffer())
    assert(
      binary.length === 4 && binary[0] === 0 && binary[1] === 255 && binary[2] === 1 && binary[3] === 128,
      'binary bodies must survive base64 protocol framing without UTF-8 corruption',
    )

    const abortController = new AbortController()
    const cancelled = transport.request('/api/v1/k001/slow', { signal: abortController.signal })
    await new Promise((resolve) => globalThis.setTimeout(resolve, 10))
    abortController.abort()
    let cancellationRejected = false
    try {
      await cancelled
    } catch (error) {
      cancellationRejected = error instanceof DOMException && error.name === 'AbortError'
    }
    assert(cancellationRejected, 'AbortSignal must cancel an in-flight protocol request')

    let openCount = 0
    const seenSequences: number[] = []
    await new Promise<void>((resolve, reject) => {
      let unsubscribe: () => void = () => undefined
      const timeout = globalThis.setTimeout(() => {
        unsubscribe()
        reject(new Error('timed out waiting for retried compatibility SSE notifications'))
      }, 4_000)
      unsubscribe = transport.subscribeSse('/api/v1/k001/events?after=9', {
        onOpen: () => { openCount += 1 },
        onEvent: (event, data, id) => {
          try {
            const sequence = Number(id)
            assert(event === 'agent.item', 'SSE event name must survive compatibility framing')
            assert(sequence === 10 || sequence === 11, 'SSE cursor must advance across one-shot retries')
            assert(JSON.parse(data).sequence === sequence, 'SSE JSON data must survive compatibility framing')
            seenSequences.push(sequence)
            if (seenSequences.length === 2) {
              globalThis.clearTimeout(timeout)
              unsubscribe()
              resolve()
            }
          } catch (error) {
            globalThis.clearTimeout(timeout)
            unsubscribe()
            reject(error)
          }
        },
        onError: (error) => {
          globalThis.clearTimeout(timeout)
          unsubscribe()
          reject(error)
        },
      })
    })
    assert(seenSequences.join(',') === '10,11', 'SSE retries must resume from the latest after cursor')
    assert(openCount === 2, 'each one-shot compatibility subscription must report an open attempt')

    await new Promise<void>((resolve, reject) => {
      let unsubscribe: () => void = () => undefined
      const timeout = globalThis.setTimeout(() => {
        unsubscribe()
        reject(new Error('timed out waiting for stream/resyncRequired'))
      }, 4_000)
      unsubscribe = transport.subscribeSse('/api/v1/k001/resync?after=20', {
        onEvent: () => {
          globalThis.clearTimeout(timeout)
          unsubscribe()
          reject(new Error('resyncRequired must not be delivered as a normal SSE event'))
        },
        onError: (error) => {
          try {
            assert(error instanceof AppServerProtocolError, 'resyncRequired must surface a protocol error')
            assert(error.error.code === -32008, 'resyncRequired must keep the protocol error code')
            assert(error.error.data?.application_code === 'CURSOR_RESYNC_REQUIRED', 'resync application code must be stable')
            assert(error.error.data?.recoverable === true, 'resyncRequired must be recoverable')
            globalThis.clearTimeout(timeout)
            unsubscribe()
            resolve()
          } catch (assertionError) {
            globalThis.clearTimeout(timeout)
            unsubscribe()
            reject(assertionError)
          }
        },
      })
    })

    // Abort notifications and notification acknowledgements are transport
    // frames of their own; allow the mock server to observe them before close.
    await new Promise((resolve) => globalThis.setTimeout(resolve, 25))
  } finally {
    await transport.close()
  }

  await runTauriStreamRecoverySmoke()
}

type FakeTauriFrame = {
  jsonrpc: '2.0'
  id?: string
  method: string
  params?: Record<string, unknown>
}

type FakeTauriEvent = {
  event: string
  id: number
  payload: unknown
}

async function runTauriStreamRecoverySmoke(): Promise<void> {
  const originalWindow = Object.getOwnPropertyDescriptor(globalThis, 'window')
  const callbacks = new Map<number, (payload: FakeTauriEvent) => void>()
  const listeners = new Map<number, { event: string; callbackId: number }>()
  const activeConnections = new Set<string>()
  const subscriptionPaths: string[] = []
  let nextCallbackId = 1
  let nextListenerId = 1
  let nextConnectionId = 1
  let subscribeAttempt = 0
  let disconnectCount = 0
  let scenario: 'recover' | 'close-before-retry' = 'recover'

  const emitFrame = (connectionId: string, frame: Record<string, unknown>): void => {
    for (const { event, callbackId } of listeners.values()) {
      if (event !== 'forgecad://app-server/message') continue
      callbacks.get(callbackId)?.({
        event,
        id: 1,
        payload: { connection_id: connectionId, frame },
      })
    }
  }

  const success = (id: string, result: unknown): Record<string, unknown> => ({ jsonrpc: '2.0', id, result })
  const adapterUnavailable = (id: string): Record<string, unknown> => ({
    jsonrpc: '2.0',
    id,
    error: {
      code: -32006,
      message: 'The compatibility sidecar is restarting',
      data: {
        schema_version: 'ForgeCADProtocolError@1',
        application_code: 'ADAPTER_UNAVAILABLE',
        recoverable: true,
      },
    },
  })

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
      const connectionId = `conn_tauri_mock_${nextConnectionId++}`
      activeConnections.add(connectionId)
      return { connection_id: connectionId }
    }
    if (command === 'forgecad_protocol_disconnect') {
      const request = args.request as { connection_id: string }
      if (activeConnections.delete(request.connection_id)) disconnectCount += 1
      return { disconnected: true }
    }
    if (command !== 'forgecad_protocol_send') throw new Error(`Unexpected mock Tauri command: ${command}`)

    const request = args.request as { connection_id: string; frame: FakeTauriFrame }
    if (!activeConnections.has(request.connection_id)) throw new Error('Mock native connection is closed')
    const { frame } = request
    if (frame.method === 'initialize') {
      return success(frame.id ?? '', {
        schema_version: 'ForgeCADInitializeResult@1',
        protocol_version: 'forgecad.app-server/1',
        connection_id: request.connection_id,
        server_info: { name: 'strict-tauri-recovery-mock', version: '1' },
        capabilities: {
          notifications: true,
          cursor_replay: true,
          cancellation: true,
          notification_ack: true,
          binary_body_base64: true,
        },
        limits: { max_in_flight_requests: 32, max_event_queue: 128, max_frame_bytes: 64 * 1024 * 1024 },
        migration_state: { state_owner: 'python_compatibility_adapter' },
      })
    }
    if (frame.method === 'initialized' || frame.method === 'notification/ack') return { accepted: true }
    if (frame.method === 'compat/unsubscribe') {
      return success(frame.id ?? '', {
        schema_version: 'ForgeCADSseUnsubscribeResult@1',
        stream_id: frame.params?.stream_id,
        unsubscribed: true,
      })
    }
    if (frame.method !== 'compat/subscribe') throw new Error(`Unexpected mock protocol method: ${frame.method}`)

    subscribeAttempt += 1
    const path = String(frame.params?.path)
    const streamId = String(frame.params?.stream_id)
    subscriptionPaths.push(path)
    if (scenario === 'recover' && subscribeAttempt === 2) {
      throw new Error('Mock native invoke channel dropped during sidecar restart')
    }
    if (scenario === 'recover' && subscribeAttempt === 3) {
      return adapterUnavailable(frame.id ?? '')
    }

    const response = success(frame.id ?? '', {
      schema_version: 'ForgeCADSseSubscriptionResult@1',
      stream_id: streamId,
      subscribed: true,
    })
    if (subscribeAttempt === 1) {
      const firstSequence = scenario === 'recover' ? 40 : 50
      globalThis.setTimeout(() => {
        emitFrame(request.connection_id, {
          jsonrpc: '2.0',
          method: 'compat/sse',
          params: {
            schema_version: 'ForgeCADSseNotification@1',
            stream_id: streamId,
            event: 'agent.item',
            data: JSON.stringify({ sequence: firstSequence }),
            id: String(firstSequence),
          },
          notification_id: `notification_tauri_${firstSequence}`,
          cursor: `fc1_tauri_${firstSequence}`,
        })
      }, 0)
      globalThis.setTimeout(() => {
        emitFrame(request.connection_id, {
          jsonrpc: '2.0',
          method: 'stream/resyncRequired',
          params: {
            schema_version: 'ForgeCADResyncRequired@1',
            reason: 'adapter_unavailable',
          },
        })
      }, 20)
    } else if (scenario === 'recover') {
      globalThis.setTimeout(() => {
        emitFrame(request.connection_id, {
          jsonrpc: '2.0',
          method: 'compat/sse',
          params: {
            schema_version: 'ForgeCADSseNotification@1',
            stream_id: streamId,
            event: 'agent.item',
            data: JSON.stringify({ sequence: 41 }),
            id: '41',
          },
          notification_id: 'notification_tauri_41',
          cursor: 'fc1_tauri_41',
        })
      }, 0)
    }
    return response
  }

  const runtimeWindow = {
    location: { protocol: 'tauri:' },
    __TAURI_INTERNALS__: {
      invoke,
      transformCallback: (callback: (payload: FakeTauriEvent) => void) => {
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
  }
  Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: runtimeWindow,
    writable: true,
  })

  try {
    const transport = new AppServerTransport()
    let openCount = 0
    let resyncCount = 0
    const sequences: number[] = []
    const cursors: string[] = []
    let unsubscribe: () => void = () => undefined
    await new Promise<void>((resolve, reject) => {
      const timeout = globalThis.setTimeout(() => {
        unsubscribe()
        reject(new Error('timed out waiting for Tauri SSE recovery'))
      }, 5_000)
      unsubscribe = transport.subscribeSse('/api/v1/k001/tauri-recovery?after=39', {
        onOpen: () => { openCount += 1 },
        onEvent: (_event, _data, id) => {
          sequences.push(Number(id))
          if (Number(id) === 41) {
            globalThis.clearTimeout(timeout)
            resolve()
          }
        },
        onCursor: (cursor) => { cursors.push(cursor) },
        onError: (error) => {
          if (
            error instanceof AppServerProtocolError
            && error.error.data?.application_code === 'CURSOR_RESYNC_REQUIRED'
          ) {
            resyncCount += 1
            return
          }
          globalThis.clearTimeout(timeout)
          reject(error)
        },
      })
    })
    assert(sequences.join(',') === '40,41', 'Tauri recovery must replay from the last delivered SSE cursor')
    assert(cursors.join(',') === 'fc1_tauri_40,fc1_tauri_41', 'cursor callbacks must stay bound to the target SSE stream')
    assert(openCount === 2, 'Tauri recovery must reopen the logical SSE stream exactly once after recovery')
    assert(resyncCount === 1, 'Tauri recovery must surface one recoverable resync signal')
    assert(subscriptionPaths[0] === '/api/v1/k001/tauri-recovery?after=39', 'initial Tauri cursor must be preserved')
    assert(
      subscriptionPaths.slice(1).every((path) => path === '/api/v1/k001/tauri-recovery?after=40'),
      'all Tauri recovery attempts must resume after the last delivered cursor',
    )
    assert(nextConnectionId >= 3, 'a native invoke failure must reopen and reinitialize the Tauri connection')

    unsubscribe()
    const attemptsAfterUnsubscribe = subscribeAttempt
    await new Promise((resolve) => globalThis.setTimeout(resolve, 350))
    assert(subscribeAttempt === attemptsAfterUnsubscribe, 'unsubscribe must prevent a recovered stream from reviving')
    await transport.close()

    scenario = 'close-before-retry'
    subscribeAttempt = 0
    subscriptionPaths.length = 0
    const closingTransport = new AppServerTransport()
    let closePromise: Promise<void> = Promise.resolve()
    let closeUnsubscribe: () => void = () => undefined
    await new Promise<void>((resolve, reject) => {
      const timeout = globalThis.setTimeout(() => reject(new Error('timed out waiting for close-during-resync')), 2_000)
      closeUnsubscribe = closingTransport.subscribeSse('/api/v1/k001/tauri-close?after=49', {
        onEvent: () => undefined,
        onError: (error) => {
          if (
            !(error instanceof AppServerProtocolError)
            || error.error.data?.application_code !== 'CURSOR_RESYNC_REQUIRED'
          ) {
            globalThis.clearTimeout(timeout)
            reject(error)
            return
          }
          closeUnsubscribe()
          closePromise = closingTransport.close()
          globalThis.clearTimeout(timeout)
          resolve()
        },
      })
    })
    await closePromise
    const attemptsAtClose = subscribeAttempt
    const connectionsAtClose = nextConnectionId
    await new Promise((resolve) => globalThis.setTimeout(resolve, 500))
    assert(subscribeAttempt === attemptsAtClose, 'closing during resync must cancel bounded recovery retries')
    assert(nextConnectionId === connectionsAtClose, 'closing during resync must not reopen a Tauri connection')
    assert(disconnectCount >= 3, 'all mock native Tauri connections must be disconnected during recovery/close')
  } finally {
    if (originalWindow) Object.defineProperty(globalThis, 'window', originalWindow)
    else Reflect.deleteProperty(globalThis, 'window')
  }
}
