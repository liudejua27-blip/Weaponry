#!/usr/bin/env node
/**
 * V003 formal browser bridge smoke.
 *
 * The only SingleResultDecision@1 in this test is emitted by the real Rust
 * Product Tool integration test.  This Node adapter freezes it, then exposes
 * its already-sealed binary preview and confirmation response through the
 * same browser compatibility transport ForgeApiClient uses.  It must never
 * construct, rank, alter, or recover a decision of its own.
 */
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { buildSync } from 'esbuild'
import { isValidElement } from 'react'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const DESKTOP = join(ROOT, 'apps', 'desktop', 'src')
const temporary = await mkdtemp(join(tmpdir(), 'forgecad-v003-rust-workbench-'))
const fixturePath = join(temporary, 'rust-v003-fixture.json')
let server

try {
  const rust = spawnSync('script/with_rust_toolchain.sh', [
    'cargo', 'test', '--manifest-path', 'apps/desktop/src-tauri/Cargo.toml',
    '-p', 'wushen-forge-desktop',
    'app_server_bridge::tests::formal_single_result_preview_get_and_confirm_create_one_atomic_asset',
    '--offline', '--', '--exact',
  ], {
    cwd: ROOT,
    encoding: 'utf8',
    env: { ...process.env, FORGECAD_V003_RUST_E2E_FIXTURE_PATH: fixturePath },
  })
  if (rust.status !== 0) {
    process.stderr.write(rust.stdout)
    process.stderr.write(rust.stderr)
    throw new Error(`Rust V003 fixture producer failed (exit ${rust.status ?? 'unknown'})`)
  }

  const fixture = deepFreeze(JSON.parse(await readFile(fixturePath, 'utf8')))
  validateRustFixture(fixture)
  const decision = fixture.decision
  const identity = {
    projectId: decision.project_id,
    turnId: decision.turn_id,
    previewId: decision.preview.preview_id,
    artifactSha256: decision.preview.artifact_sha256,
    artifactProfileId: decision.preview.artifact_profile_id,
  }
  const previewPath = `/api/v1/agent/projects/${identity.projectId}/turns/${identity.turnId}/single-results/${identity.previewId}:preview.glb`
  const confirmPath = `/api/v1/agent/projects/${identity.projectId}/turns/${identity.turnId}/single-results/${identity.previewId}:confirm`
  const observations = { opens: 0, initializes: 0, initialized: 0, previewGets: 0, confirms: 0, unexpected: [] }

  server = createServer(async (request, response) => {
    try {
      const body = await requestJson(request)
      if (request.method === 'POST' && request.url === '/api/v1/app-server/connections') {
        observations.opens += 1
        return sendJson(response, 200, { connection_id: 'conn_rust_v003_fixture' })
      }
      if (request.method !== 'POST' || request.url !== '/api/v1/app-server/connections/conn_rust_v003_fixture/frames') {
        observations.unexpected.push(`${request.method} ${request.url}`)
        return sendJson(response, 404, { error: 'unexpected route' })
      }
      const frame = body?.frame
      if (!frame || frame.jsonrpc !== '2.0' || typeof frame.method !== 'string') {
        return sendJson(response, 200, { frame: protocolFailure(null, 'INVALID_REQUEST', 'Malformed protocol frame') })
      }
      if (frame.method === 'initialize') {
        observations.initializes += 1
        return sendJson(response, 200, { frame: protocolSuccess(frame.id, {
          schema_version: 'ForgeCADInitializeResult@1',
          protocol_version: 'forgecad.app-server/1',
          connection_id: 'conn_rust_v003_fixture',
          server_info: { name: 'rust-v003-fixture-adapter', version: '1' },
          capabilities: {
            notifications: true, cursor_replay: true, cancellation: true,
            notification_ack: true, binary_body_base64: true,
          },
          limits: { max_in_flight_requests: 32, max_event_queue: 64, max_frame_bytes: 64 * 1024 * 1024 },
          migration_state: { state_owner: 'rust_app_server' },
        }) })
      }
      if (frame.method === 'initialized') {
        observations.initialized += 1
        return sendJson(response, 200, { frames: [] })
      }
      if (frame.method !== 'compat/http') {
        observations.unexpected.push(`protocol ${frame.method}`)
        return sendJson(response, 200, { frame: protocolFailure(frame.id ?? null, 'METHOD_NOT_ALLOWED', 'Only the frozen V003 fixture routes are available') })
      }
      const input = frame.params
      if (input?.schema_version !== 'ForgeCADHttpCompatibilityRequest@1') {
        return sendJson(response, 200, { frame: protocolFailure(frame.id, 'HTTP_SCHEMA_INVALID', 'Compatibility HTTP schema is invalid') })
      }
      const headers = new Map((input.headers ?? []).map(([name, value]) => [String(name).toLowerCase(), String(value)]))
      if (input.method === 'GET' && input.path === previewPath) {
        observations.previewGets += 1
        assert(headers.get('if-match') === `"sha256:${identity.artifactSha256}"`, 'preview GET must bind the frozen GLB hash')
        return sendJson(response, 200, { frame: protocolSuccess(frame.id, httpResult(200, fixture.preview_headers, { encoding: 'base64', data: fixture.preview_glb_base64 })) })
      }
      if (input.method === 'POST' && input.path === confirmPath) {
        observations.confirms += 1
        assert(headers.get('if-match') === `"sha256:${identity.artifactSha256}"`, 'confirm must bind the frozen GLB hash')
        assert(headers.get('idempotency-key') === fixture.confirm_request.client_request_id, 'confirm must preserve the Rust fixture idempotency identity')
        assert(headers.get('content-type') === 'application/json', 'confirm must remain JSON')
        assert(input.body?.encoding === 'utf8', 'confirm body must be UTF-8 JSON')
        assert(JSON.stringify(JSON.parse(input.body.data)) === JSON.stringify(fixture.confirm_request), 'confirm body must remain the Rust fixture request')
        return sendJson(response, 200, { frame: protocolSuccess(frame.id, httpResult(fixture.confirm_status, [['content-type', 'application/json']], { encoding: 'utf8', data: JSON.stringify(fixture.confirm_response) })) })
      }
      observations.unexpected.push(`${input.method} ${input.path}`)
      return sendJson(response, 200, { frame: protocolFailure(frame.id, 'ROUTE_NOT_FROZEN', 'Fixture route is not part of the sealed V003 path') })
    } catch (error) {
      sendJson(response, 500, { error: error instanceof Error ? error.message : String(error) })
    }
  })
  await listen(server)
  const port = server.address().port
  const baseUrl = `http://127.0.0.1:${port}`

  const bundle = join(temporary, 'bundle')
  buildSync({
    entryPoints: {
      forgeApi: join(DESKTOP, 'shared', 'api', 'forgeApi.ts'),
      transport: join(DESKTOP, 'shared', 'api', 'appServerTransport.ts'),
      presentation: join(DESKTOP, 'features', 'cad-workbench', 'singleResultDecisionPresentationState.ts'),
      resultCard: join(DESKTOP, 'features', 'cad-workbench', 'GenerationResultCard.tsx'),
    },
    bundle: true, platform: 'node', format: 'esm', outdir: bundle, target: 'es2022', jsx: 'automatic',
  })
  const [{ ForgeApiClient }, { appServerTransport }, presentation, { GenerationResultCard }] = await Promise.all([
    import(pathToFileURL(join(bundle, 'forgeApi.js')).href),
    import(pathToFileURL(join(bundle, 'transport.js')).href),
    import(pathToFileURL(join(bundle, 'presentation.js')).href),
    import(pathToFileURL(join(bundle, 'resultCard.js')).href),
  ])
  const api = new ForgeApiClient(baseUrl)
  const item = {
    item_id: 'item_rust_v003_fixture', thread_id: 'thread_rust_v003_fixture', turn_id: identity.turnId,
    item_type: 'tool_result', sequence: 1, created_at: '2026-07-18T00:00:00Z',
    payload: { tool_name: 'prepare_candidate_preview', tool_result: { validated_output: { value: decision } } },
  }
  const read = presentation.readSingleResultDecisionFromAgentItems([item], { projectId: identity.projectId, turnId: identity.turnId })
  assert(read?.decision_id === decision.decision_id, 'presentation must read the unchanged Rust decision from the tool item')
  let state = presentation.singleResultDecisionPresentationReducer(presentation.initialSingleResultDecisionPresentationState, { type: 'open_project', projectId: identity.projectId })
  state = presentation.singleResultDecisionPresentationReducer(state, { type: 'request_started', projectId: identity.projectId, requestId: 1 })
  state = presentation.singleResultDecisionPresentationReducer(state, { type: 'decision_received', projectId: identity.projectId, requestId: 1, decision: read })
  assert(state.presentation.state === 'ready', 'only the Rust ready decision may reach the ready result-card state')

  const preview = await api.loadSingleResultPreviewGlb(identity)
  assert(hash(Buffer.from(preview.glb)) === identity.artifactSha256, 'ForgeApi preview bytes must match the sealed Rust decision hash')
  assert(preview.artifactProfileId === identity.artifactProfileId, 'ForgeApi preview profile must match the sealed Rust decision')
  let confirmed = null
  const card = GenerationResultCard({
    state: 'ready', summary: decision.summary, onContinueEditing: () => undefined,
    onSave: async () => { confirmed = await api.confirmSingleResultPreview({ ...identity, clientRequestId: fixture.confirm_request.client_request_id, summary: fixture.confirm_request.summary }) },
  })
  const save = findButtons(card).find((button) => text(button).includes('确认保存'))
  assert(save, 'ready result card must expose one confirm-save action')
  await save.props.onClick()
  assert(confirmed?.asset_version_id, 'result card save must receive the frozen Rust confirm response')
  assert(observations.opens === 1 && observations.initializes === 1 && observations.initialized === 1, 'browser transport must complete exactly one Rust-owned protocol handshake')
  assert(observations.previewGets === 1 && observations.confirms === 1, 'workbench must request one sealed binary preview then one confirmation')
  assert(observations.unexpected.length === 0, `fixture adapter saw unexpected traffic: ${observations.unexpected.join(', ')}`)
  await appServerTransport.close()
  console.log(JSON.stringify({ ok: true, task: 'FGC-V003', source: fixture.source, assertions: ['rust_decision_fixture', 'binary_preview_hash_profile_identity', 'result_card_confirm', 'no_node_or_python_decision'] }, null, 2))
} finally {
  if (server) await new Promise((resolveClose) => server.close(resolveClose))
  await rm(temporary, { recursive: true, force: true })
}

function validateRustFixture(fixture) {
  assert(fixture?.schema_version === 'ForgeCADV003RustWorkbenchFixture@1', 'fixture schema must be explicit')
  assert(fixture.source === 'rust_app_server_native_product_tools', 'fixture must originate from Rust Product Tools')
  const decision = fixture.decision
  assert(decision?.schema_version === 'SingleResultDecision@1' && decision.state === 'ready_for_preview' && decision.outcome === 'passed', 'Rust fixture must contain one ready V003 decision')
  assert(decision.project_id === fixture.project_id && decision.turn_id === fixture.turn_id, 'fixture decision identity must be Rust-bound')
  assert(decision.preview?.artifact_sha256 === fixture.preview_sha256, 'fixture preview hash must equal decision hash')
  const bytes = Buffer.from(fixture.preview_glb_base64, 'base64')
  assert(bytes.length > 0 && hash(bytes) === fixture.preview_sha256, 'fixture GLB bytes must match Rust preview hash')
  const headers = new Map(fixture.preview_headers.map(([name, value]) => [String(name).toLowerCase(), String(value)]))
  assert(headers.get('x-forgecad-glb-sha256') === fixture.preview_sha256, 'Rust preview header hash must bind bytes')
  assert(headers.get('x-forgecad-artifact-profile') === decision.preview.artifact_profile_id, 'Rust preview header profile must bind decision')
  assert(headers.get('x-forgecad-project-id') === decision.project_id && headers.get('x-forgecad-turn-id') === decision.turn_id && headers.get('x-forgecad-preview-id') === decision.preview.preview_id, 'Rust preview headers must bind route identity')
  assert(fixture.confirm_status === 201 && fixture.replay_status === 201, 'Rust confirmation and its idempotent replay must both be accepted')
}

function httpResult(status, headers, body) { return { schema_version: 'ForgeCADHttpCompatibilityResponse@1', status, headers, body } }
function protocolSuccess(id, result) { return { jsonrpc: '2.0', id, result } }
function protocolFailure(id, code, message) { return { jsonrpc: '2.0', id, error: { code: -32000, message, data: { schema_version: 'ForgeCADProtocolError@1', application_code: code } } } }
function hash(bytes) { return createHash('sha256').update(bytes).digest('hex') }
function assert(value, message) { if (!value) throw new Error(message) }
function sendJson(response, status, value) { response.writeHead(status, { 'content-type': 'application/json' }); response.end(JSON.stringify(value)) }
async function requestJson(request) { const chunks = []; for await (const chunk of request) chunks.push(chunk); const raw = Buffer.concat(chunks).toString('utf8'); return raw ? JSON.parse(raw) : {} }
function listen(httpServer) { return new Promise((resolveListen) => httpServer.listen(0, '127.0.0.1', resolveListen)) }
function deepFreeze(value) { if (value && typeof value === 'object' && !Object.isFrozen(value)) { Object.freeze(value); for (const child of Object.values(value)) deepFreeze(child) }; return value }
function text(node) { if (node == null || typeof node === 'boolean') return ''; if (typeof node === 'string' || typeof node === 'number') return String(node); if (Array.isArray(node)) return node.map(text).join(' '); if (!isValidElement(node)) return ''; if (typeof node.type === 'function') return text(node.type(node.props)); return text(node.props.children) }
function findButtons(node) { if (node == null || typeof node === 'boolean') return []; if (Array.isArray(node)) return node.flatMap(findButtons); if (!isValidElement(node)) return []; if (typeof node.type === 'function') return findButtons(node.type(node.props)); return (node.type === 'button' ? [node] : []).concat(findButtons(node.props.children)) }
