#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFile, mkdtemp, rm, symlink, writeFile } from 'node:fs/promises'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { pathToFileURL, fileURLToPath } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const DESKTOP_SOURCE = join(ROOT, 'apps', 'desktop', 'src')
const forgeApiPath = join(DESKTOP_SOURCE, 'shared', 'api', 'forgeApi.ts')
const workbenchPath = join(DESKTOP_SOURCE, 'features', 'cad-workbench', 'CadWorkbenchPanel.tsx')
const transportPath = join(DESKTOP_SOURCE, 'shared', 'api', 'appServerTransport.ts')
const output = await mkdtemp(join(tmpdir(), 'forgecad-k001-transport-'))
const compatibilityFixture = JSON.parse(await readFile(
  join(ROOT, 'packages', 'concept-spec', 'fixtures', 'k001-a004-turn-compatibility.json'),
  'utf8',
))

function canonicalJson(value) {
  if (value === null || typeof value !== 'object') return JSON.stringify(value)
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`
}

function sha256(value) {
  return createHash('sha256').update(value, 'utf8').digest('hex')
}

const golden = compatibilityFixture.canonical_golden
const itemHashes = golden.items.map((item) => sha256(canonicalJson(item)))
if (JSON.stringify(itemHashes) !== JSON.stringify(golden.item_sha256)) {
  throw new Error('desktop canonical AgentItem hashes drifted from the shared A004 fixture')
}
const turnHash = sha256(canonicalJson(golden.items.map((item, index) => ({
  sequence: item.sequence,
  item_sha256: itemHashes[index],
}))))
if (turnHash !== golden.turn_items_sha256) {
  throw new Error('desktop canonical Turn item hash drifted from the shared A004 fixture')
}

const [forgeApiSource, workbenchSource, transportSource] = await Promise.all([
  readFile(forgeApiPath, 'utf8'),
  readFile(workbenchPath, 'utf8'),
  readFile(transportPath, 'utf8'),
])

const forbiddenForgeApiPatterns = [
  [/\bfetch\s*\(/, 'direct fetch'],
  [/\bEventSource\b/, 'EventSource'],
  [/127\.0\.0\.1/, 'hard-coded Python loopback'],
]
for (const [pattern, label] of forbiddenForgeApiPatterns) {
  if (pattern.test(forgeApiSource)) throw new Error(`ForgeApi still contains ${label}`)
}
if (/window\.setInterval\s*\(/.test(workbenchSource)) {
  throw new Error('CadWorkbenchPanel still polls Thread state with setInterval')
}
if (!forgeApiSource.includes('appServerTransport.resourceUrl(')) {
  throw new Error('ForgeApi resource URL getters do not route through the protocol transport')
}
if (/sendNotification[\s\S]{0,500}sendTauriFrame\s*\(/.test(transportSource)) {
  throw new Error('JSON-RPC notifications incorrectly require a JSON-RPC response')
}
const transportWithoutResourceUrl = transportSource.replace(
  /  resourceUrl\(path: string\): string \{[\s\S]*?\n  \}\n\n  async initialize/,
  '  async initialize',
)
const rawFetchTargets = [...transportWithoutResourceUrl.matchAll(/\bfetch\s*\(([^,\n]+)/g)]
if (
  rawFetchTargets.length !== 1
  || !rawFetchTargets[0][1].includes('this.appServerCompatibilityUrl(')
) {
  throw new Error('browser transport raw fetch must have exactly one app-server compatibility target')
}
for (const [pattern, label] of [
  [/\bEventSource\b/, 'EventSource'],
  [/readSseStream\s*\(/, 'direct SSE reader'],
  [/fetch\s*\(`\$\{this\.browserBaseUrl\}/, 'direct loopback product fetch'],
  [/frame\.method === ['"]initialize['"][\s\S]{0,200}ForgeCADInitializeResult@1/, 'locally forged initialize result'],
]) {
  if (pattern.test(transportWithoutResourceUrl)) throw new Error(`browser transport still contains ${label}`)
}
if (!transportSource.includes("'/api/v1/app-server/connections'")) {
  throw new Error('browser transport does not open the Python app-server compatibility connection')
}
const receiveProtocolFrameSource = transportSource.match(
  /private receiveProtocolFrame\(frame: JsonRpcFrame\): void \{[\s\S]*?\n  \}\n\n  private async cancelRequest/,
)?.[0]
if (!receiveProtocolFrameSource) {
  throw new Error('transport smoke could not locate the shared protocol notification receiver')
}
if (
  !receiveProtocolFrameSource.includes('subscription.path = withAfterCursor(subscription.path, params.id)')
  || /this\.mode\s*===\s*['"]browser-loopback-compatibility['"]/.test(receiveProtocolFrameSource)
) {
  throw new Error('compat/sse cursor advancement must be transport-neutral for Tauri and browser reconnects')
}
if (
  !transportSource.includes('this.receiveProtocolFrame(event.payload.frame)')
  || !transportSource.includes('this.receiveProtocolFrame(frame)')
) {
  throw new Error('Tauri and browser notifications must converge on receiveProtocolFrame')
}

const observations = {
  opened: 0,
  closed: 0,
  initializedRequests: 0,
  initializedNotifications: 0,
  httpPaths: [],
  subscriptionPaths: [],
  unsubscribeCount: 0,
  ackCount: 0,
  cancelCount: 0,
  unexpectedRoutes: [],
}
const connections = new Map()
let nextConnection = 1

function protocolSuccess(id, result) {
  return { jsonrpc: '2.0', id, result }
}

function protocolFailure(id, code, applicationCode, message) {
  return {
    jsonrpc: '2.0',
    id,
    error: {
      code,
      message,
      data: {
        schema_version: 'ForgeCADProtocolError@1',
        application_code: applicationCode,
        recoverable: code <= -32000 && code >= -32099,
      },
    },
  }
}

function httpCompatibilityResult(status, headers, body) {
  return {
    schema_version: 'ForgeCADHttpCompatibilityResponse@1',
    status,
    headers,
    body,
  }
}

function sendJson(response, status, value) {
  response.writeHead(status, { 'Content-Type': 'application/json' })
  response.end(JSON.stringify(value))
}

async function readJson(request) {
  const chunks = []
  for await (const chunk of request) chunks.push(chunk)
  const text = Buffer.concat(chunks).toString('utf8')
  return text ? JSON.parse(text) : {}
}

function protocolRequestBody(body) {
  if (!body || body.encoding === 'empty') return Buffer.alloc(0)
  if (body.encoding === 'utf8') return Buffer.from(body.data, 'utf8')
  if (body.encoding === 'base64') return Buffer.from(body.data, 'base64')
  throw new Error(`unexpected protocol body encoding: ${body?.encoding}`)
}

async function handleFrame(connection, frame, response) {
  if (!frame || frame.jsonrpc !== '2.0' || typeof frame.method !== 'string') {
    sendJson(response, 200, { frame: protocolFailure(null, -32600, 'INVALID_REQUEST', 'Invalid frame') })
    return
  }
  const isRequest = typeof frame.id === 'string'
  if (frame.method === 'initialize') {
    observations.initializedRequests += 1
    connection.state = 'awaiting_initialized'
    const capabilities = {
      notifications: true,
      cursor_replay: true,
      cancellation: true,
      notification_ack: true,
      binary_body_base64: true,
    }
    // The first connection intentionally omits one mandatory capability. The
    // client must reject and close it, then a second initialize() must create
    // a new connection rather than reuse this half-handshake.
    if (connection.id === 'conn_mock_1') delete capabilities.notification_ack
    sendJson(response, 200, {
      frame: protocolSuccess(frame.id, {
        schema_version: 'ForgeCADInitializeResult@1',
        protocol_version: 'forgecad.app-server/1',
        connection_id: connection.id,
        server_info: { name: 'strict-python-compatibility-mock', version: '1' },
        capabilities,
        limits: { max_in_flight_requests: 32, max_event_queue: 128, max_frame_bytes: 64 * 1024 * 1024 },
        migration_state: { state_owner: 'python_compatibility_adapter' },
      }),
    })
    return
  }
  if (frame.method === 'initialized') {
    observations.initializedNotifications += 1
    connection.state = 'ready'
    sendJson(response, 200, { frames: [] })
    return
  }
  if (connection.state !== 'ready') {
    sendJson(response, 200, {
      frame: protocolFailure(frame.id ?? null, -32002, 'NOT_INITIALIZED', 'Connection is not initialized'),
    })
    return
  }
  if (frame.method === 'notification/ack') {
    observations.ackCount += 1
    sendJson(response, 200, { frames: [] })
    return
  }
  if (frame.method === 'request/cancel') {
    observations.cancelCount += 1
    sendJson(response, 200, { frames: [] })
    return
  }
  if (!isRequest) {
    sendJson(response, 200, {
      frame: protocolFailure(null, -32601, 'METHOD_NOT_FOUND', 'Unknown notification'),
    })
    return
  }
  if (frame.method === 'compat/http') {
    const input = frame.params
    observations.httpPaths.push(input.path)
    const productUrl = new URL(input.path, 'http://forgecad.product')
    if (productUrl.pathname === '/api/v1/k001/json') {
      const headers = new Map(input.headers.map(([name, value]) => [name.toLowerCase(), value]))
      const payload = JSON.parse(protocolRequestBody(input.body).toString('utf8'))
      sendJson(response, 200, {
        frame: protocolSuccess(frame.id, httpCompatibilityResult(
          201,
          [['content-type', 'application/json']],
          {
            encoding: 'utf8',
            data: JSON.stringify({
              body: payload,
              header: headers.get('x-client-request-id') ?? null,
              query: productUrl.searchParams.get('source'),
            }),
          },
        )),
      })
      return
    }
    if (productUrl.pathname === '/api/v1/k001/binary') {
      sendJson(response, 200, {
        frame: protocolSuccess(frame.id, httpCompatibilityResult(
          200,
          [['content-type', 'model/gltf-binary']],
          { encoding: 'base64', data: Buffer.from([0, 255, 1, 128]).toString('base64') },
        )),
      })
      return
    }
    if (productUrl.pathname === '/api/v1/k001/slow') {
      await new Promise((resolveDelay) => setTimeout(resolveDelay, 1_000))
      if (!response.destroyed) {
        sendJson(response, 200, {
          frame: protocolSuccess(frame.id, httpCompatibilityResult(200, [], { encoding: 'empty' })),
        })
      }
      return
    }
    sendJson(response, 200, {
      frame: protocolFailure(frame.id, -32602, 'INVALID_PARAMS', 'Unexpected mock product path'),
    })
    return
  }
  if (frame.method === 'compat/subscribe') {
    const { stream_id: streamId, path } = frame.params
    observations.subscriptionPaths.push(path)
    connection.streams.add(streamId)
    const productUrl = new URL(path, 'http://forgecad.product')
    if (productUrl.pathname === '/api/v1/k001/resync') {
      sendJson(response, 200, {
        frame: protocolSuccess(frame.id, {
          schema_version: 'ForgeCADSseSubscriptionResult@1',
          stream_id: streamId,
          subscribed: true,
        }),
        frames: [{
          jsonrpc: '2.0',
          method: 'stream/resyncRequired',
          params: {
            schema_version: 'ForgeCADResyncRequired@1',
            reason: 'slow_consumer',
          },
          notification_id: 'notification_resync_21',
        }],
      })
      return
    }
    const after = Number(productUrl.searchParams.get('after') ?? '0')
    const sequence = after + 1
    const notification = {
      jsonrpc: '2.0',
      method: 'compat/sse',
      params: {
        schema_version: 'ForgeCADSseNotification@1',
        stream_id: streamId,
        event: 'agent.item',
        data: JSON.stringify({ sequence }),
        id: String(sequence),
      },
      notification_id: `notification_${sequence}`,
      cursor: `fc1_mock_${sequence}`,
    }
    sendJson(response, 200, {
      frame: protocolSuccess(frame.id, {
        schema_version: 'ForgeCADSseSubscriptionResult@1',
        stream_id: streamId,
        subscribed: true,
      }),
      frames: [notification],
    })
    return
  }
  if (frame.method === 'compat/unsubscribe') {
    observations.unsubscribeCount += 1
    const streamId = frame.params.stream_id
    const unsubscribed = connection.streams.delete(streamId)
    sendJson(response, 200, {
      frame: protocolSuccess(frame.id, {
        schema_version: 'ForgeCADSseUnsubscribeResult@1',
        stream_id: streamId,
        unsubscribed,
      }),
    })
    return
  }
  sendJson(response, 200, {
    frame: protocolFailure(frame.id, -32601, 'METHOD_NOT_FOUND', `Unknown method ${frame.method}`),
  })
}

const server = createServer(async (request, response) => {
  const url = new URL(request.url ?? '/', 'http://127.0.0.1')
  if (request.method === 'POST' && url.pathname === '/api/v1/app-server/connections') {
    const body = await readJson(request)
    if (Object.keys(body).length !== 0) {
      sendJson(response, 400, { error: { code: -32602, message: 'Unexpected open body' } })
      return
    }
    const connectionId = `conn_mock_${nextConnection++}`
    connections.set(connectionId, { id: connectionId, state: 'opened', streams: new Set() })
    observations.opened += 1
    sendJson(response, 200, { connection_id: connectionId })
    return
  }
  const frameRoute = url.pathname.match(/^\/api\/v1\/app-server\/connections\/([^/]+)\/frames$/)
  if (request.method === 'POST' && frameRoute) {
    const connection = connections.get(decodeURIComponent(frameRoute[1]))
    if (!connection) {
      sendJson(response, 404, { error: { code: -32006, message: 'Connection not found' } })
      return
    }
    const body = await readJson(request)
    if (!body || Object.keys(body).join(',') !== 'frame') {
      sendJson(response, 400, { error: { code: -32602, message: 'Expected exact frame envelope' } })
      return
    }
    await handleFrame(connection, body.frame, response)
    return
  }
  const closeRoute = url.pathname.match(/^\/api\/v1\/app-server\/connections\/([^/]+):close$/)
  if (request.method === 'POST' && closeRoute) {
    const connectionId = decodeURIComponent(closeRoute[1])
    const closed = connections.delete(connectionId)
    if (closed) observations.closed += 1
    sendJson(response, closed ? 200 : 404, closed ? { closed: true } : { error: { code: -32006, message: 'Connection not found' } })
    return
  }
  observations.unexpectedRoutes.push(`${request.method} ${url.pathname}`)
  response.writeHead(404)
  response.end('unexpected route')
})

await new Promise((resolveListen, rejectListen) => {
  server.once('error', rejectListen)
  server.listen(0, '127.0.0.1', resolveListen)
})
const address = server.address()
if (!address || typeof address === 'string') throw new Error('smoke server did not expose a TCP port')

try {
  const result = spawnSync(
    join(ROOT, 'node_modules', '.bin', 'tsc'),
    [
      '--target', 'ES2022',
      '--module', 'ESNext',
      '--moduleResolution', 'Bundler',
      '--strict',
      '--skipLibCheck',
      '--types', 'vite/client',
      '--outDir', output,
      '--rootDir', DESKTOP_SOURCE,
      join(DESKTOP_SOURCE, 'shared', 'api', 'appServerProtocol.ts'),
      join(DESKTOP_SOURCE, 'shared', 'api', 'appServerTransport.ts'),
      join(DESKTOP_SOURCE, 'shared', 'api', 'appServerTransport.smoke.ts'),
    ],
    { cwd: ROOT, encoding: 'utf8' },
  )
  if (result.status !== 0) {
    process.stderr.write(result.stdout)
    process.stderr.write(result.stderr)
    process.exit(result.status ?? 1)
  }
  await symlink(join(ROOT, 'node_modules'), join(output, 'node_modules'), 'junction')
  await writeFile(join(output, 'package.json'), '{"type":"module"}\n', 'utf8')
  const module = await import(pathToFileURL(join(output, 'shared', 'api', 'appServerTransport.smoke.js')).href)
  await module.runAppServerTransportSmoke(`http://127.0.0.1:${address.port}`)
  if (observations.opened !== 2 || observations.closed !== 2) {
    throw new Error(`browser compatibility connection lifecycle mismatch: ${JSON.stringify(observations)}`)
  }
  if (observations.initializedRequests !== 2 || observations.initializedNotifications !== 1) {
    throw new Error(`failed initialize was reused or initialized notification count is wrong: ${JSON.stringify(observations)}`)
  }
  if (observations.httpPaths.join(',') !== [
    '/api/v1/k001/json?source=transport',
    '/api/v1/k001/binary',
    '/api/v1/k001/slow',
  ].join(',')) {
    throw new Error(`compat/http product path sequence mismatch: ${JSON.stringify(observations.httpPaths)}`)
  }
  if (observations.subscriptionPaths.join(',') !== [
    '/api/v1/k001/events?after=9',
    '/api/v1/k001/events?after=10',
    '/api/v1/k001/resync?after=20',
  ].join(',')) {
    throw new Error(`SSE retry did not advance after cursor: ${JSON.stringify(observations.subscriptionPaths)}`)
  }
  if (observations.unsubscribeCount < 3 || observations.ackCount < 3 || observations.cancelCount !== 1) {
    throw new Error(`notification/unsubscribe frame coverage mismatch: ${JSON.stringify(observations)}`)
  }
  if (observations.unexpectedRoutes.length !== 0) {
    throw new Error(`transport bypassed app-server compatibility routes: ${observations.unexpectedRoutes.join(', ')}`)
  }
  console.log('K001 App Server transport smoke passed')
} finally {
  await new Promise((resolveClose) => server.close(resolveClose))
  await rm(output, { recursive: true, force: true })
}
