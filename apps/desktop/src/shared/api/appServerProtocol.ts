export const FORGECAD_PROTOCOL_VERSION = 'forgecad.app-server/1' as const
export const JSON_RPC_VERSION = '2.0' as const

export type JsonRpcId = string

export type JsonRpcRequest<TParams = unknown> = {
  jsonrpc: typeof JSON_RPC_VERSION
  id: JsonRpcId
  method: string
  params?: TParams
}

export type JsonRpcNotification<TParams = unknown> = {
  jsonrpc: typeof JSON_RPC_VERSION
  method: string
  params?: TParams
  notification_id?: string
  cursor?: string | null
}

export type JsonRpcErrorData = {
  schema_version?: 'ForgeCADProtocolError@1'
  application_code?: string
  recoverable?: boolean
  details?: Record<string, unknown>
  request_id?: string
  retry_after_ms?: number | null
}

export type JsonRpcError = {
  code: number
  message: string
  data?: JsonRpcErrorData
}

export type JsonRpcSuccess<TResult = unknown> = {
  jsonrpc: typeof JSON_RPC_VERSION
  id: JsonRpcId
  result: TResult
}

export type JsonRpcFailure = {
  jsonrpc: typeof JSON_RPC_VERSION
  id: JsonRpcId | null
  error: JsonRpcError
}

export type JsonRpcResponse<TResult = unknown> = JsonRpcSuccess<TResult> | JsonRpcFailure
export type JsonRpcFrame = JsonRpcRequest | JsonRpcNotification | JsonRpcResponse

export type ForgeCadInitializeParams = {
  schema_version: 'ForgeCADInitializeParams@1'
  supported_protocol_versions: string[]
  client_info: {
    name: 'forgecad-desktop'
    version: string
    transport: 'tauri' | 'browser-loopback-compatibility'
  }
  capabilities: {
    notifications: true
    cursor_replay: true
    cancellation: true
    notification_ack: true
    binary_body_base64: true
  }
}

export type ForgeCadInitializeResult = {
  schema_version: 'ForgeCADInitializeResult@1'
  protocol_version: string
  connection_id: string
  server_info: {
    name: string
    version: string
  }
  capabilities: Record<string, unknown>
  limits?: {
    max_in_flight_requests?: number
    max_event_queue?: number
    max_frame_bytes?: number
  }
  migration_state?: {
    state_owner: 'python_compatibility_adapter' | 'rust_app_server'
  }
}

export type ProtocolHttpBody =
  | { encoding: 'empty'; data?: never }
  | { encoding: 'utf8'; data: string }
  | { encoding: 'base64'; data: string }

export type ProtocolHttpRequest = {
  schema_version: 'ForgeCADHttpCompatibilityRequest@1'
  path: string
  method: string
  headers: Array<[string, string]>
  body: ProtocolHttpBody
}

export type ProtocolHttpResponse = {
  schema_version: 'ForgeCADHttpCompatibilityResponse@1'
  status: number
  headers: Array<[string, string]>
  body: ProtocolHttpBody
}

export type ProtocolSseNotification = {
  schema_version: 'ForgeCADSseNotification@1'
  stream_id: string
  event: string
  data: string
  id?: string | null
}

export class AppServerProtocolError extends Error {
  constructor(readonly error: JsonRpcError) {
    super(error.message)
    this.name = 'AppServerProtocolError'
  }
}

let requestSequence = 0

export function createProtocolRequest<TParams>(method: string, params?: TParams, id = nextProtocolRequestId()): JsonRpcRequest<TParams> {
  return {
    jsonrpc: JSON_RPC_VERSION,
    id,
    method,
    ...(params === undefined ? {} : { params }),
  }
}

export function createProtocolNotification<TParams>(method: string, params?: TParams): JsonRpcNotification<TParams> {
  return {
    jsonrpc: JSON_RPC_VERSION,
    method,
    ...(params === undefined ? {} : { params }),
  }
}

export function nextProtocolRequestId(): string {
  requestSequence += 1
  const random = globalThis.crypto?.randomUUID?.().replaceAll('-', '')
    ?? `${Date.now().toString(36)}${requestSequence.toString(36)}`
  return `req_${random}`
}

export function isJsonRpcResponse(value: unknown): value is JsonRpcResponse {
  if (!isRecord(value) || value.jsonrpc !== JSON_RPC_VERSION || !('id' in value)) return false
  return ('result' in value) !== ('error' in value)
}

export function isJsonRpcNotification(value: unknown): value is JsonRpcNotification {
  return isRecord(value)
    && value.jsonrpc === JSON_RPC_VERSION
    && typeof value.method === 'string'
    && !('id' in value)
}

export function unwrapJsonRpcResponse<TResult>(response: JsonRpcResponse<TResult>, expectedId: string): TResult {
  if (response.id !== expectedId) {
    throw new AppServerProtocolError({
      code: -32006,
      message: `Protocol response ID mismatch: expected ${expectedId}`,
      data: { schema_version: 'ForgeCADProtocolError@1', application_code: 'RESPONSE_ID_MISMATCH' },
    })
  }
  if ('error' in response) throw new AppServerProtocolError(response.error)
  return response.result
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}
