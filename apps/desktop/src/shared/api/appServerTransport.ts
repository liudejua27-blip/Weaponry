import {
  AppServerProtocolError,
  FORGECAD_PROTOCOL_VERSION,
  createProtocolNotification,
  createProtocolRequest,
  isJsonRpcNotification,
  isJsonRpcResponse,
  nextProtocolRequestId,
  unwrapJsonRpcResponse,
  type ForgeCadInitializeParams,
  type ForgeCadInitializeResult,
  type JsonRpcFrame,
  type JsonRpcError,
  type JsonRpcNotification,
  type JsonRpcRequest,
  type JsonRpcResponse,
  type ProtocolHttpBody,
  type ProtocolHttpRequest,
  type ProtocolHttpResponse,
  type ProtocolSseNotification,
} from './appServerProtocol.js'

type TransportMode = 'tauri' | 'browser-loopback-compatibility'

export type ProtocolSseHandlers = {
  onOpen?: () => void
  onEvent: (event: string, data: string, id: string | null) => void
  onCursor?: (cursor: string) => void
  onError?: (error: unknown) => void
}

type NotificationListener = (notification: JsonRpcNotification) => void

export type NativeAgentMethod =
  | 'thread/create'
  | 'thread/list'
  | 'thread/read'
  | 'thread/archive'
  | 'turn/start'
  | 'turn/read'
  | 'turn/cancel'
  | 'item/list'
  | 'item/read'
  | 'approval/create'
  | 'approval/read'
  | 'approval/resolve'
  | 'provider/preflight'
  | 'provider/check'
  | 'provider/cancel'
  | 'product-tools/list'
  | 'migration/ownership/read'

export type NativeRequestOptions = {
  signal?: AbortSignal
  retrySafe?: boolean
}

type SseSubscription = {
  path: string
  handlers: ProtocolSseHandlers
  stopped: boolean
  browserAbort: AbortController | null
  tauriActive: boolean
  tauriRecoveryRevision: number
}

type TauriConnection = {
  connectionId: string
  unlisten: () => void
}

type BrowserConnection = {
  connectionId: string
}

type BrowserFrameEnvelope = {
  frame?: JsonRpcFrame
  frames?: JsonRpcFrame[]
}

type TauriMessagePayload = {
  connection_id: string
  frame: JsonRpcFrame
}

const viteEnvironment = (import.meta as ImportMeta & {
  env?: { VITE_FORGE_API_BASE_URL?: string }
}).env
const DEFAULT_BROWSER_BASE_URL = normalizeBrowserLoopbackBaseUrl(
  viteEnvironment?.VITE_FORGE_API_BASE_URL || 'http://127.0.0.1:8000',
)
const RECONNECT_DELAY_MS = 150
const MAX_TAURI_STREAM_RECOVERY_ATTEMPTS = 8
const MAX_TAURI_STREAM_RECOVERY_DELAY_MS = 1_200
const MAX_NEGOTIATED_IN_FLIGHT_REQUESTS = 32
const MAX_NEGOTIATED_EVENT_QUEUE = 4_096
const MAX_NEGOTIATED_FRAME_BYTES = 64 * 1024 * 1024
const STABLE_PROTOCOL_ID = /^[A-Za-z0-9_.-]{1,160}$/
const REQUIRED_CAPABILITIES = [
  'notifications',
  'cursor_replay',
  'cancellation',
  'notification_ack',
  'binary_body_base64',
] as const

/**
 * Owns the one desktop-to-Agent transport connection. It deliberately keeps no
 * Thread, Snapshot, Version, ChangeSet, quality, or export state.
 */
export class AppServerTransport {
  private browserBaseUrl = DEFAULT_BROWSER_BASE_URL
  private mode: TransportMode | null = null
  private initializePromise: Promise<ForgeCadInitializeResult> | null = null
  private tauriConnection: TauriConnection | null = null
  private browserConnection: BrowserConnection | null = null
  private readonly notificationListeners = new Set<NotificationListener>()
  private readonly sseListeners = new Map<string, SseSubscription>()
  private lifecycleEpoch = 0
  private reconnectPromise: Promise<ForgeCadInitializeResult> | null = null
  private tauriRecoveryPromise: Promise<void> | null = null

  configureBrowserBaseUrl(baseUrl: string): void {
    const normalized = normalizeBrowserLoopbackBaseUrl(baseUrl)
    if (normalized === this.browserBaseUrl) return
    const previousBaseUrl = this.browserBaseUrl
    const previousConnection = this.browserConnection
    this.browserConnection = null
    this.browserBaseUrl = normalized
    if (this.mode === 'browser-loopback-compatibility') {
      this.initializePromise = null
      if (previousConnection) {
        void this.closeBrowserConnection(previousConnection, previousBaseUrl).catch(() => undefined)
      }
    }
  }

  getCompatibilityBaseUrl(): string {
    return this.browserBaseUrl
  }

  resourceUrl(path: string): string {
    const normalized = normalizeProtocolPath(path)
    // Synchronous URL getters remain only on the isolated legacy surface. A
    // packaged WebView receives a Rust-owned custom protocol URL, never the
    // Python loopback address. Browser development has no native IPC bridge.
    return isNativeDesktopRuntime()
      ? `forgecad-resource://localhost${normalized}`
      : `${this.browserBaseUrl}${normalized}`
  }

  async initialize(): Promise<ForgeCadInitializeResult> {
    const lifecycleEpoch = this.lifecycleEpoch
    return this.initializePromise ?? this.startInitialization(lifecycleEpoch)
  }

  async reconnect(): Promise<ForgeCadInitializeResult> {
    return this.reconnectAtLifecycle(this.lifecycleEpoch)
  }

  async close(): Promise<void> {
    this.lifecycleEpoch += 1
    for (const subscription of this.sseListeners.values()) {
      subscription.stopped = true
      subscription.browserAbort?.abort()
    }
    this.sseListeners.clear()
    this.reconnectPromise = null
    this.tauriRecoveryPromise = null
    await this.resetProtocolConnection()
    this.initializePromise = null
    this.mode = null
  }

  async request(path: string, init: RequestInit = {}): Promise<Response> {
    const method = (init.method ?? 'GET').toUpperCase()
    const requestId = nextProtocolRequestId()
    const frame = createProtocolRequest<ProtocolHttpRequest>('compat/http', {
      schema_version: 'ForgeCADHttpCompatibilityRequest@1',
      path: normalizeProtocolPath(path),
      method,
      headers: Array.from(new Headers(init.headers).entries()),
      body: await requestBody(init.body),
    }, requestId)

    if (init.signal?.aborted) throw abortError()
    await this.initialize()

    const cancel = () => { void this.cancelRequest(requestId).catch(() => undefined) }
    init.signal?.addEventListener('abort', cancel, { once: true })
    try {
      const result = await this.dispatch<ProtocolHttpResponse>(frame, init.signal ?? undefined)
      return responseFromProtocol(result)
    } catch (error) {
      if (init.signal?.aborted) throw abortError()
      if (isSafeRead(method) && this.mode === 'tauri' && isReconnectable(error)) {
        await this.reconnect()
        const result = await this.dispatch<ProtocolHttpResponse>(frame, init.signal ?? undefined)
        return responseFromProtocol(result)
      }
      throw error
    } finally {
      init.signal?.removeEventListener('abort', cancel)
    }
  }

  /**
   * K002 native Agent lifecycle entry. Method names are a closed TypeScript
   * union mirrored by the Rust protocol crate; callers cannot turn this into
   * a generic JSON-RPC or dynamic-tool escape hatch.
   */
  async nativeRequest<TResult>(
    method: NativeAgentMethod,
    params: Record<string, unknown>,
    options: NativeRequestOptions = {},
  ): Promise<TResult> {
    if (!isNativeDesktopRuntime()) {
      throw new Error('ForgeCAD native Agent lifecycle methods require the Tauri desktop runtime')
    }
    const requestId = nextProtocolRequestId()
    const frame = createProtocolRequest(method, params, requestId)
    if (options.signal?.aborted) throw abortError()
    await this.initialize()

    const cancel = () => { void this.cancelRequest(requestId).catch(() => undefined) }
    options.signal?.addEventListener('abort', cancel, { once: true })
    try {
      return await this.dispatch<TResult>(frame, options.signal)
    } catch (error) {
      if (options.signal?.aborted) throw abortError()
      if (options.retrySafe === true && this.mode === 'tauri' && isReconnectable(error)) {
        await this.reconnect()
        return this.dispatch<TResult>(frame, options.signal)
      }
      throw error
    } finally {
      options.signal?.removeEventListener('abort', cancel)
    }
  }

  subscribeSse(path: string, handlers: ProtocolSseHandlers): () => void {
    const streamId = `stream_${nextProtocolRequestId().slice(4)}`
    const normalizedPath = normalizeProtocolPath(path)
    const subscription: SseSubscription = {
      path: normalizedPath,
      handlers,
      stopped: false,
      browserAbort: null,
      tauriActive: false,
      tauriRecoveryRevision: 0,
    }
    this.sseListeners.set(streamId, subscription)

    void this.initialize().then(async () => {
      if (subscription.stopped) return
      if (this.mode === 'tauri') {
        const recoveryRevision = subscription.tauriRecoveryRevision
        await this.subscribeTauriStream(streamId, normalizedPath)
          .then(() => {
            if (subscription.tauriRecoveryRevision !== recoveryRevision) {
              this.scheduleTauriStreamRecovery()
              return
            }
            subscription.tauriActive = true
            if (!subscription.stopped) handlers.onOpen?.()
          })
          .catch((error) => {
            subscription.tauriActive = false
            this.scheduleTauriStreamRecovery()
            if (!subscription.stopped) handlers.onError?.(error)
          })
        return
      }

      while (!subscription.stopped) {
        subscription.browserAbort = new AbortController()
        try {
          await this.dispatch(createProtocolRequest('compat/subscribe', {
            schema_version: 'ForgeCADSseSubscription@1',
            stream_id: streamId,
            path: subscription.path,
          }), subscription.browserAbort.signal)
        } catch (error) {
          if (!subscription.stopped && !subscription.browserAbort.signal.aborted) {
            handlers.onError?.(error)
          }
        } finally {
          subscription.browserAbort = null
          if (this.mode === 'browser-loopback-compatibility' && this.browserConnection) {
            await this.dispatch(createProtocolRequest('compat/unsubscribe', {
              schema_version: 'ForgeCADSseUnsubscribe@1',
              stream_id: streamId,
            })).catch(() => undefined)
          }
        }
        if (!subscription.stopped) await delay(RECONNECT_DELAY_MS)
      }
    }).catch((error) => handlers.onError?.(error))

    return () => {
      subscription.stopped = true
      subscription.tauriActive = false
      subscription.browserAbort?.abort()
      if (this.sseListeners.delete(streamId) && this.mode !== null) {
        void this.dispatch(createProtocolRequest('compat/unsubscribe', {
          schema_version: 'ForgeCADSseUnsubscribe@1',
          stream_id: streamId,
        })).catch(() => undefined)
      }
    }
  }

  subscribeNotifications(listener: NotificationListener): () => void {
    this.notificationListeners.add(listener)
    return () => this.notificationListeners.delete(listener)
  }

  private async initializeConnection(lifecycleEpoch: number): Promise<ForgeCadInitializeResult> {
    this.assertActiveLifecycle(lifecycleEpoch)
    this.mode = isNativeDesktopRuntime() ? 'tauri' : 'browser-loopback-compatibility'
    try {
      if (this.mode === 'tauri') await this.openTauriConnection()
      else await this.openBrowserConnection()
      this.assertActiveLifecycle(lifecycleEpoch)

      const params: ForgeCadInitializeParams = {
        schema_version: 'ForgeCADInitializeParams@1',
        supported_protocol_versions: [FORGECAD_PROTOCOL_VERSION],
        client_info: {
          name: 'forgecad-desktop',
          version: '0.1.0',
          transport: this.mode,
        },
        capabilities: {
          notifications: true,
          cursor_replay: true,
          cancellation: true,
          notification_ack: true,
          binary_body_base64: true,
        },
      }
      const rawResult = await this.dispatch<unknown>(createProtocolRequest('initialize', params), undefined, true)
      const expectedConnectionId = this.mode === 'tauri'
        ? this.tauriConnection?.connectionId
        : this.browserConnection?.connectionId
      const result = validateInitializeResult(rawResult, expectedConnectionId)
      await this.sendNotification(createProtocolNotification('initialized', {
        protocol_version: result.protocol_version,
      }))
      this.assertActiveLifecycle(lifecycleEpoch)
      if (this.mode === 'tauri' && this.sseListeners.size > 0) {
        await Promise.all([...this.sseListeners.entries()]
          .filter(([, subscription]) => subscription.tauriActive)
          .map(async ([streamId, subscription]) => {
            try {
              const recoveryRevision = subscription.tauriRecoveryRevision
              await this.subscribeTauriStream(streamId, subscription.path, true)
              this.assertActiveLifecycle(lifecycleEpoch)
              if (subscription.tauriRecoveryRevision !== recoveryRevision) return
              if (!subscription.stopped) subscription.handlers.onOpen?.()
            } catch (error) {
              subscription.handlers.onError?.(error)
              throw error
            }
          }))
      }
      return result
    } catch (error) {
      // A failed initialize request or initialized notification invalidates
      // the whole handshake. Close that exact connection before allowing a
      // retry so no half-initialized server state can be reused.
      await this.resetProtocolConnection().catch(() => undefined)
      this.mode = null
      throw error
    }
  }

  private startInitialization(lifecycleEpoch: number): Promise<ForgeCadInitializeResult> {
    const initialization = this.initializeConnection(lifecycleEpoch).catch((error) => {
      if (this.initializePromise === initialization) this.initializePromise = null
      throw error
    })
    this.initializePromise = initialization
    return initialization
  }

  private async dispatch<TResult>(
    frame: JsonRpcRequest,
    signal?: AbortSignal,
    allowBeforeInitialize = false,
  ): Promise<TResult> {
    if (!allowBeforeInitialize) await this.initialize()
    if (signal?.aborted) throw abortError()
    if (this.mode === 'browser-loopback-compatibility') {
      const response = await this.dispatchBrowser(frame, signal)
      return unwrapJsonRpcResponse<TResult>(response as JsonRpcResponse<TResult>, frame.id)
    }
    const response = await this.sendTauriFrame(frame)
    return unwrapJsonRpcResponse<TResult>(response as JsonRpcResponse<TResult>, frame.id)
  }

  private async dispatchBrowser(frame: JsonRpcRequest, signal?: AbortSignal): Promise<JsonRpcResponse> {
    const output = await this.invokeBrowserFrame(frame, signal)
    const response = output.frame
    if (!isJsonRpcResponse(response)) {
      throw new Error('ForgeCAD browser compatibility adapter returned a malformed response')
    }
    if (frame.method === 'compat/subscribe' && !('error' in response)) {
      const streamId = readStreamId(frame.params)
      this.sseListeners.get(streamId)?.handlers.onOpen?.()
    }
    this.receiveBrowserFrames(output.frames)
    return response
  }

  private async openTauriConnection(): Promise<void> {
    if (this.tauriConnection) return
    const [{ invoke }, { listen }] = await Promise.all([
      import('@tauri-apps/api/core'),
      import('@tauri-apps/api/event'),
    ])
    const opened = await invoke<{ connection_id: string }>('forgecad_protocol_connect')
    const connectionId = readConnectionId(opened)
    // Record the native connection before listener registration. If listen()
    // fails, initializeConnection() can still disconnect this half-open peer.
    this.tauriConnection = { connectionId, unlisten: () => undefined }
    const unlisten = await listen<TauriMessagePayload>('forgecad://app-server/message', (event) => {
      if (event.payload.connection_id !== connectionId) return
      this.receiveProtocolFrame(event.payload.frame)
    })
    if (this.tauriConnection?.connectionId === connectionId) this.tauriConnection.unlisten = unlisten
    else unlisten()
  }

  private async openBrowserConnection(): Promise<void> {
    if (this.browserConnection) return
    const response = await this.fetchAppServerCompatibility('/api/v1/app-server/connections', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: '{}',
      cache: 'no-store',
    })
    const output = await readCompatibilityJson(response)
    if (!response.ok) throw compatibilityTransportError(response.status, output)
    const connectionId = readConnectionId(output)
    this.browserConnection = { connectionId }
  }

  private async invokeBrowserFrame(
    frame: JsonRpcFrame,
    signal?: AbortSignal,
  ): Promise<BrowserFrameEnvelope> {
    const connection = this.browserConnection
    if (!connection) throw new Error('ForgeCAD browser compatibility connection is unavailable')
    const path = `/api/v1/app-server/connections/${encodeURIComponent(connection.connectionId)}/frames`
    const response = await this.fetchAppServerCompatibility(path, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ frame }),
      cache: 'no-store',
      signal,
    })
    const output = await readCompatibilityJson(response)
    if (!response.ok) throw compatibilityTransportError(response.status, output)
    if (!isRecord(output)) {
      throw new Error('ForgeCAD browser compatibility adapter returned a malformed frame envelope')
    }
    const frames = output.frames
    if (frames !== undefined && (!Array.isArray(frames) || frames.some((item) => !isJsonRpcNotification(item)))) {
      throw new Error('ForgeCAD browser compatibility adapter returned malformed notifications')
    }
    return {
      ...('frame' in output ? { frame: output.frame as JsonRpcFrame } : {}),
      ...(Array.isArray(frames) ? { frames } : {}),
    }
  }

  private receiveBrowserFrames(frames: JsonRpcFrame[] | undefined): void {
    for (const frame of frames ?? []) this.receiveProtocolFrame(frame)
  }

  private fetchAppServerCompatibility(
    path: string,
    init: RequestInit,
    baseUrl = this.browserBaseUrl,
  ): Promise<Response> {
    return fetch(this.appServerCompatibilityUrl(path, baseUrl), init)
  }

  private appServerCompatibilityUrl(path: string, baseUrl = this.browserBaseUrl): string {
    const normalized = normalizeProtocolPath(path)
    if (
      normalized !== '/api/v1/app-server/connections'
      && !normalized.startsWith('/api/v1/app-server/connections/')
    ) {
      throw new TypeError(`ForgeCAD browser transport rejected a non-app-server route: ${normalized}`)
    }
    return `${baseUrl}${normalized}`
  }

  private async invokeTauriFrame(frame: JsonRpcFrame): Promise<unknown> {
    const connection = this.tauriConnection
    if (!connection) throw new Error('ForgeCAD Tauri protocol connection is unavailable')
    const { invoke } = await import('@tauri-apps/api/core')
    return invoke('forgecad_protocol_send', {
      request: { connection_id: connection.connectionId, frame },
    })
  }

  private async sendTauriFrame(frame: JsonRpcRequest): Promise<JsonRpcResponse> {
    const output = await this.invokeTauriFrame(frame) as JsonRpcResponse | { frame: JsonRpcResponse }
    const response = 'frame' in output ? output.frame : output
    if (!isJsonRpcResponse(response)) throw new Error('ForgeCAD protocol bridge returned a malformed response')
    return response
  }

  private async sendNotification(frame: JsonRpcNotification): Promise<void> {
    if (this.mode === 'browser-loopback-compatibility') {
      const output = await this.invokeBrowserFrame(frame)
      rejectNotificationResponse(output.frame, 'browser compatibility adapter')
      this.receiveBrowserFrames(output.frames)
      return
    }
    // JSON-RPC notifications intentionally have no JSON-RPC response. The
    // native bridge may return a transport-level `{ accepted: true }`, but a
    // protocol failure (including a rejected initialized notification) must
    // invalidate the handshake rather than be silently accepted.
    const output = await this.invokeTauriFrame(frame)
    const response = isRecord(output) && 'frame' in output ? output.frame : output
    rejectNotificationResponse(response, 'native bridge')
  }

  private async subscribeTauriStream(
    streamId: string,
    path: string,
    allowBeforeInitialize = false,
  ): Promise<void> {
    await this.dispatch(createProtocolRequest('compat/subscribe', {
      schema_version: 'ForgeCADSseSubscription@1',
      stream_id: streamId,
      path,
    }), undefined, allowBeforeInitialize)
  }

  private receiveProtocolFrame(frame: JsonRpcFrame): void {
    if (!isJsonRpcNotification(frame)) return
    if (frame.method === 'stream/resyncRequired') {
      const params = isRecord(frame.params) ? frame.params : {}
      const reason = typeof params.reason === 'string' ? params.reason : 'resync_required'
      const error = new AppServerProtocolError({
        code: -32008,
        message: `ForgeCAD stream requires a state refresh: ${reason}`,
        data: {
          schema_version: 'ForgeCADProtocolError@1',
          application_code: 'CURSOR_RESYNC_REQUIRED',
          recoverable: true,
          details: { reason },
        },
      })
      for (const subscription of this.sseListeners.values()) {
        subscription.tauriActive = false
        subscription.tauriRecoveryRevision += 1
      }
      this.scheduleTauriStreamRecovery()
      for (const subscription of this.sseListeners.values()) subscription.handlers.onError?.(error)
    }
    if (frame.method === 'compat/sse') {
      const params = frame.params as ProtocolSseNotification
      const subscription = this.sseListeners.get(params.stream_id)
      // Both the native event channel and browser adapter converge here. The
      // next subscribe/reconnect must resume after the last delivered event.
      if (subscription && params.id) {
        subscription.path = withAfterCursor(subscription.path, params.id)
      }
      if (subscription && typeof frame.cursor === 'string') {
        subscription.handlers.onCursor?.(frame.cursor)
      }
      subscription?.handlers.onEvent(params.event, params.data, params.id ?? null)
    }
    for (const listener of this.notificationListeners) listener(frame)
    if (frame.notification_id || frame.cursor) {
      void this.sendNotification(createProtocolNotification('notification/ack', {
        notification_id: frame.notification_id ?? null,
        cursor: frame.cursor ?? null,
      })).catch(() => undefined)
    }
  }

  private async cancelRequest(requestId: string): Promise<void> {
    if (
      (this.mode === 'tauri' && !this.tauriConnection)
      || (this.mode === 'browser-loopback-compatibility' && !this.browserConnection)
      || this.mode === null
    ) return
    await this.sendNotification(createProtocolNotification('request/cancel', {
      request_id: requestId,
      cancel_token: requestId,
    }))
  }

  private scheduleTauriStreamRecovery(): void {
    if (!isNativeDesktopRuntime() || !this.hasRecoverableTauriStreams() || this.tauriRecoveryPromise) return
    const lifecycleEpoch = this.lifecycleEpoch
    const recovery = this.recoverTauriStreams(lifecycleEpoch)
    this.tauriRecoveryPromise = recovery
    void recovery
      .finally(() => {
        if (this.tauriRecoveryPromise === recovery) this.tauriRecoveryPromise = null
      })
      .catch(() => undefined)
  }

  private async recoverTauriStreams(lifecycleEpoch: number): Promise<void> {
    let lastError: unknown = new Error('ForgeCAD SSE recovery did not start')
    for (let attempt = 0; attempt < MAX_TAURI_STREAM_RECOVERY_ATTEMPTS; attempt += 1) {
      await delay(tauriRecoveryDelay(attempt))
      if (!this.isTauriRecoveryActive(lifecycleEpoch)) return

      try {
        if (!this.tauriConnection || this.mode !== 'tauri') {
          await this.reopenTauriConnectionForRecovery(lifecycleEpoch)
        }
        const pending = [...this.sseListeners.entries()]
          .filter(([, subscription]) => !subscription.stopped && !subscription.tauriActive)
        for (const [streamId, subscription] of pending) {
          const recoveryRevision = subscription.tauriRecoveryRevision
          await this.subscribeTauriStream(streamId, subscription.path)
          if (!this.isTauriRecoveryActive(lifecycleEpoch) || subscription.stopped) return
          if (subscription.tauriRecoveryRevision !== recoveryRevision) continue
          subscription.tauriActive = true
          subscription.handlers.onOpen?.()
        }
        if ([...this.sseListeners.values()].every((subscription) => subscription.stopped || subscription.tauriActive)) {
          return
        }
      } catch (error) {
        lastError = error
        if (shouldReopenTauriConnection(error)) {
          try {
            await this.reopenTauriConnectionForRecovery(lifecycleEpoch)
          } catch (reopenError) {
            lastError = reopenError
          }
        }
      }
    }

    if (!this.isTauriRecoveryActive(lifecycleEpoch)) return
    for (const subscription of this.sseListeners.values()) {
      if (!subscription.stopped && !subscription.tauriActive) subscription.handlers.onError?.(lastError)
    }
  }

  private async reopenTauriConnectionForRecovery(lifecycleEpoch: number): Promise<void> {
    await this.reconnectAtLifecycle(lifecycleEpoch)
    this.assertActiveLifecycle(lifecycleEpoch)
  }

  private reconnectAtLifecycle(lifecycleEpoch: number): Promise<ForgeCadInitializeResult> {
    this.assertActiveLifecycle(lifecycleEpoch)
    if (this.reconnectPromise) return this.reconnectPromise
    const reconnecting = this.performReconnect(lifecycleEpoch)
    this.reconnectPromise = reconnecting
    void reconnecting
      .finally(() => {
        if (this.reconnectPromise === reconnecting) this.reconnectPromise = null
      })
      .catch(() => undefined)
    return reconnecting
  }

  private async performReconnect(lifecycleEpoch: number): Promise<ForgeCadInitializeResult> {
    await this.resetProtocolConnection()
    this.assertActiveLifecycle(lifecycleEpoch)
    this.initializePromise = null
    this.mode = null
    return this.startInitialization(lifecycleEpoch)
  }

  private hasRecoverableTauriStreams(): boolean {
    return [...this.sseListeners.values()].some((subscription) => !subscription.stopped && !subscription.tauriActive)
  }

  private isTauriRecoveryActive(lifecycleEpoch: number): boolean {
    return lifecycleEpoch === this.lifecycleEpoch && this.hasRecoverableTauriStreams()
  }

  private assertActiveLifecycle(lifecycleEpoch: number): void {
    if (lifecycleEpoch !== this.lifecycleEpoch) {
      throw new DOMException('The ForgeCAD protocol lifecycle was closed', 'AbortError')
    }
  }

  private async resetTauriConnection(): Promise<void> {
    const connection = this.tauriConnection
    this.tauriConnection = null
    if (!connection) return
    connection.unlisten()
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('forgecad_protocol_disconnect', {
      request: { connection_id: connection.connectionId },
    }).catch(() => undefined)
  }

  private async closeBrowserConnection(
    connection = this.browserConnection,
    baseUrl = this.browserBaseUrl,
  ): Promise<void> {
    if (!connection) return
    if (this.browserConnection?.connectionId === connection.connectionId) this.browserConnection = null
    const path = `/api/v1/app-server/connections/${encodeURIComponent(connection.connectionId)}:close`
    const response = await this.fetchAppServerCompatibility(path, {
      method: 'POST',
      cache: 'no-store',
    }, baseUrl)
    if (!response.ok && response.status !== 404) {
      const output = await readCompatibilityJson(response)
      throw compatibilityTransportError(response.status, output)
    }
  }

  private async resetProtocolConnection(): Promise<void> {
    if (this.mode === 'browser-loopback-compatibility') await this.closeBrowserConnection()
    else await this.resetTauriConnection()
  }
}

export const appServerTransport = new AppServerTransport()

function rejectNotificationResponse(value: unknown, source: string): void {
  if (value === undefined || value === null) return
  if (isJsonRpcResponse(value)) {
    if ('error' in value) throw new AppServerProtocolError(value.error)
    throw new Error(`ForgeCAD ${source} returned a JSON-RPC result for a notification`)
  }
  if (isRecord(value) && value.accepted === true) return
  throw new Error(`ForgeCAD ${source} returned a malformed notification acknowledgement`)
}

function validateInitializeResult(
  value: unknown,
  expectedConnectionId: string | undefined,
): ForgeCadInitializeResult {
  if (!isRecord(value)) {
    throw initializeResultError('MALFORMED_INITIALIZE_RESULT', 'ForgeCAD initialize result must be an object')
  }
  if (value.schema_version !== 'ForgeCADInitializeResult@1') {
    throw initializeResultError(
      'INITIALIZE_SCHEMA_MISMATCH',
      'ForgeCAD initialize result has an unsupported schema_version',
    )
  }
  if (value.protocol_version !== FORGECAD_PROTOCOL_VERSION) {
    throw initializeResultError(
      'PROTOCOL_VERSION_MISMATCH',
      `Unsupported ForgeCAD protocol version: ${String(value.protocol_version)}`,
      -32003,
    )
  }
  if (
    typeof value.connection_id !== 'string'
    || !STABLE_PROTOCOL_ID.test(value.connection_id)
    || value.connection_id !== expectedConnectionId
  ) {
    throw initializeResultError(
      'CONNECTION_ID_MISMATCH',
      'ForgeCAD initialize result did not preserve the opened stable connection_id',
    )
  }
  if (
    !isRecord(value.server_info)
    || !isBoundedText(value.server_info.name, 1, 160)
    || !isBoundedText(value.server_info.version, 1, 160)
  ) {
    throw initializeResultError(
      'MALFORMED_INITIALIZE_RESULT',
      'ForgeCAD initialize result has invalid server_info',
    )
  }
  if (!isRecord(value.capabilities)) {
    throw initializeCapabilityError('ForgeCAD initialize result is missing required capabilities')
  }
  const capabilities = value.capabilities
  const missingCapability = REQUIRED_CAPABILITIES.find((capability) => capabilities[capability] !== true)
  if (missingCapability) {
    throw initializeCapabilityError(`ForgeCAD initialize result did not enable ${missingCapability}`)
  }
  if (!isRecord(value.limits)) {
    throw initializeResultError('INVALID_NEGOTIATED_LIMITS', 'ForgeCAD initialize result is missing limits')
  }
  validateNegotiatedLimit(
    value.limits.max_in_flight_requests,
    'max_in_flight_requests',
    MAX_NEGOTIATED_IN_FLIGHT_REQUESTS,
  )
  validateNegotiatedLimit(value.limits.max_event_queue, 'max_event_queue', MAX_NEGOTIATED_EVENT_QUEUE)
  validateNegotiatedLimit(value.limits.max_frame_bytes, 'max_frame_bytes', MAX_NEGOTIATED_FRAME_BYTES)
  if (
    !isRecord(value.migration_state)
    || !['python_compatibility_adapter', 'rust_app_server'].includes(
      String(value.migration_state.state_owner),
    )
  ) {
    throw initializeResultError(
      'STATE_OWNER_MISMATCH',
      'ForgeCAD initialize result must report one recognized single state owner',
    )
  }
  return value as ForgeCadInitializeResult
}

function validateNegotiatedLimit(value: unknown, name: string, maximum: number): void {
  if (!Number.isInteger(value) || typeof value !== 'number' || value < 1 || value > maximum) {
    throw initializeResultError(
      'INVALID_NEGOTIATED_LIMITS',
      `ForgeCAD initialize result has an out-of-range ${name}`,
    )
  }
}

function initializeCapabilityError(message: string): AppServerProtocolError {
  return initializeResultError('CAPABILITY_UNSUPPORTED', message, -32013)
}

function initializeResultError(
  applicationCode: string,
  message: string,
  code = -32603,
): AppServerProtocolError {
  return new AppServerProtocolError({
    code,
    message,
    data: {
      schema_version: 'ForgeCADProtocolError@1',
      application_code: applicationCode,
      recoverable: false,
    },
  })
}

function isBoundedText(value: unknown, minimum: number, maximum: number): value is string {
  return typeof value === 'string' && value.length >= minimum && value.length <= maximum
}

async function requestBody(body: BodyInit | null | undefined): Promise<ProtocolHttpBody> {
  if (body === undefined || body === null) return { encoding: 'empty' }
  if (typeof body === 'string') return { encoding: 'utf8', data: body }
  if (body instanceof URLSearchParams) return { encoding: 'utf8', data: body.toString() }
  if (body instanceof Blob) return { encoding: 'base64', data: bytesToBase64(new Uint8Array(await body.arrayBuffer())) }
  if (body instanceof ArrayBuffer) return { encoding: 'base64', data: bytesToBase64(new Uint8Array(body)) }
  if (ArrayBuffer.isView(body)) {
    return { encoding: 'base64', data: bytesToBase64(new Uint8Array(body.buffer, body.byteOffset, body.byteLength)) }
  }
  throw new TypeError('ForgeCAD protocol transport does not accept streaming or FormData request bodies')
}

function responseFromProtocol(input: ProtocolHttpResponse): Response {
  if (input.schema_version !== 'ForgeCADHttpCompatibilityResponse@1') {
    throw new Error('ForgeCAD protocol returned an unsupported HTTP compatibility response')
  }
  return new Response(decodeProtocolBody(input.body), {
    status: input.status,
    headers: new Headers(input.headers),
  })
}

function decodeProtocolBody(body: ProtocolHttpBody): BodyInit | null {
  if (body.encoding === 'empty') return null
  if (body.encoding === 'utf8') return body.data
  const bytes = base64ToBytes(body.data)
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer
}

function normalizeProtocolPath(path: string): string {
  if (path.startsWith('/')) return path
  try {
    const parsed = new URL(path)
    return `${parsed.pathname}${parsed.search}`
  } catch {
    throw new TypeError(`ForgeCAD protocol paths must be relative API paths: ${path}`)
  }
}

function normalizeBrowserLoopbackBaseUrl(value: string): string {
  let parsed: URL
  try {
    parsed = new URL(value)
  } catch {
    throw new TypeError('ForgeCAD browser compatibility endpoint must be a loopback HTTP origin')
  }
  const loopbackHost = parsed.hostname === '127.0.0.1'
    || parsed.hostname === 'localhost'
    || parsed.hostname === '[::1]'
  if (
    parsed.protocol !== 'http:'
    || !loopbackHost
    || !parsed.port
    || parsed.username
    || parsed.password
    || parsed.pathname !== '/'
    || parsed.search
    || parsed.hash
  ) {
    throw new TypeError('ForgeCAD browser compatibility endpoint must be loopback HTTP with an explicit port')
  }
  return parsed.origin
}

function withAfterCursor(path: string, cursor: string): string {
  const parsed = new URL(path, 'http://forgecad.compatibility')
  parsed.searchParams.set('after', cursor)
  return `${parsed.pathname}${parsed.search}`
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = ''
  const chunkSize = 0x8000
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize))
  }
  return btoa(binary)
}

function base64ToBytes(value: string): Uint8Array {
  const binary = atob(value)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index)
  return bytes
}

function readStreamId(params: unknown): string {
  if (!isRecord(params) || typeof params.stream_id !== 'string') {
    throw new Error('ForgeCAD SSE subscription response is missing stream_id')
  }
  return params.stream_id
}

async function readCompatibilityJson(response: Response): Promise<unknown> {
  try {
    return await response.json() as unknown
  } catch {
    throw new Error(`ForgeCAD app-server compatibility route returned non-JSON HTTP ${response.status}`)
  }
}

function readConnectionId(value: unknown): string {
  if (!isRecord(value) || typeof value.connection_id !== 'string' || value.connection_id.length === 0) {
    throw new Error('ForgeCAD app-server compatibility route returned an invalid connection ID')
  }
  return value.connection_id
}

function compatibilityTransportError(status: number, value: unknown): Error {
  if (
    isRecord(value)
    && isRecord(value.error)
    && typeof value.error.code === 'number'
    && typeof value.error.message === 'string'
  ) {
    return new AppServerProtocolError(value.error as JsonRpcError)
  }
  return new Error(`ForgeCAD app-server compatibility route failed with HTTP ${status}`)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

/**
 * Read-only runtime boundary used by ForgeApi to select Rust-native lifecycle
 * methods in the packaged WebView. Browser development intentionally remains
 * on the K001 compatibility HTTP/SSE adapter.
 */
export function isNativeDesktopRuntime(): boolean {
  return typeof window !== 'undefined'
    && ('__TAURI_INTERNALS__' in window || window.location.protocol === 'tauri:')
}

function isSafeRead(method: string): boolean {
  return method === 'GET' || method === 'HEAD'
}

function isReconnectable(error: unknown): boolean {
  // A typed ForgeApi error was produced after a native response passed the
  // transport boundary and failed its closed client contract. Reconnecting
  // cannot make that persisted response valid, so retrying it would hide a
  // protocol regression behind a second request. Only a missing compatibility
  // adapter is a known, read-only transport transient. Other untyped invoke
  // failures retain the existing reconnect path because there is no accepted
  // response whose semantics could have been validated.
  if (isForgeApiContractError(error)) return false
  return !(error instanceof AppServerProtocolError)
    || error.error.data?.application_code === 'ADAPTER_UNAVAILABLE'
}

function isForgeApiContractError(error: unknown): error is {
  name: 'ForgeApiError'
  code: string
  recoverable: boolean
} {
  return typeof error === 'object'
    && error !== null
    && (error as { name?: unknown }).name === 'ForgeApiError'
    && typeof (error as { code?: unknown }).code === 'string'
    && typeof (error as { recoverable?: unknown }).recoverable === 'boolean'
}

function shouldReopenTauriConnection(error: unknown): boolean {
  // ADAPTER_UNAVAILABLE means the Rust connection is still valid while its
  // compatibility sidecar is restarting. Retrying the same connection keeps
  // the protocol/event queue stable. Native invoke/listener failures require a
  // full reconnect and initialize handshake.
  return !(error instanceof AppServerProtocolError)
}

function tauriRecoveryDelay(attempt: number): number {
  return Math.min(RECONNECT_DELAY_MS * (2 ** attempt), MAX_TAURI_STREAM_RECOVERY_DELAY_MS)
}

function abortError(): DOMException {
  return new DOMException('The ForgeCAD protocol request was cancelled', 'AbortError')
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => globalThis.setTimeout(resolve, milliseconds))
}
