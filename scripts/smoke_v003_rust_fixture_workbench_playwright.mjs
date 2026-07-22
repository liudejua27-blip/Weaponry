#!/usr/bin/env node
// FGC-V003 rendered browser fixture.  The Rust test below is the sole source
// of SingleResultDecision@1, GLB bytes, preview headers and confirmation body.
// This proxy only transports those frozen values into a real Vite/Playwright
// workbench; unrelated, compatibility-only bootstrap reads are forwarded to
// the isolated Python oracle and cannot create V003 state.
import { spawn, spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync } from 'node:fs'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'
import { legacyLifecycleTestOracleEnvironment } from './workbench_agent_blockout_test_helper.mjs'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const TIMEOUT = 25_000
// Optional, deliberately explicit evidence output. The default gate remains
// hermetic; a reviewer can opt into a screenshot plus redacted browser/proxy
// telemetry without turning the test's temporary worktree into a source of
// truth for the model or its version state.
const evidenceDir = process.env.FORGECAD_E2E_EVIDENCE_DIR
const temporary = await mkdtemp(join(tmpdir(), 'forgecad-v003-playwright-'))
let proxy = null
let browser = null
let vite = null
let python = null

try {
  const pythonPort = await freePort()
  const proxyPort = await freePort()
  const vitePort = await freePort()
  const pythonBase = `http://127.0.0.1:${pythonPort}`
  const proxyBase = `http://127.0.0.1:${proxyPort}`
  const viteBase = `http://127.0.0.1:${vitePort}`
  const library = join(temporary, 'python-compat-library')
  const state = {
    fixture: null,
    threadCreated: false,
    previewGets: 0,
    confirms: 0,
    forwarded: [],
    unexpected: [],
    network: [],
  }

  python = spawn(join(ROOT, '.venv', 'bin', 'python'), [
    '-m', 'uvicorn', 'wushen_agent.test_oracle:create_app', '--factory', '--host', '127.0.0.1', '--port', String(pythonPort),
  ], {
    cwd: ROOT,
    env: legacyLifecycleTestOracleEnvironment(process.env, {
      WUSHEN_LIBRARY_ROOT: library,
      WUSHEN_MIGRATIONS_DIR: join(ROOT, 'migrations'),
      WUSHEN_CORS_ORIGINS: viteBase,
      WUSHEN_LOCAL_WORKER_ENABLED: '0',
      FORGECAD_CONCEPT_WORKER_ENABLED: '1',
      FORGECAD_CONCEPT_PLANNER_PROVIDER: 'deterministic_rules',
    }),
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  drain(python)
  await waitForHttp(`${pythonBase}/api/health`, python, 'Python compatibility bootstrap')
  proxy = createFixtureProxy({ pythonBase, state })
  await listen(proxy, proxyPort)
  vite = spawn(process.execPath, [join(ROOT, 'node_modules', 'vite', 'bin', 'vite.js'), '--host', '127.0.0.1', '--port', String(vitePort)], {
    cwd: join(ROOT, 'apps', 'desktop'), env: { ...process.env, VITE_FORGE_API_BASE_URL: proxyBase }, stdio: ['ignore', 'pipe', 'pipe'],
  })
  drain(vite)
  await waitForHttp(viteBase, vite, 'Vite workbench')

  browser = await launchSystemBrowser()
  const page = await browser.newPage({ viewport: { width: 1440, height: 960 } })
  const errors = []
  const consoleEvents = []
  page.on('pageerror', (error) => errors.push(error.message))
  page.on('console', (message) => {
    // Preserve diagnostics, never request bodies or browser storage.
    consoleEvents.push({ type: message.type(), text: message.text() })
  })
  await page.goto(`${viteBase}/#/cad`, { waitUntil: 'networkidle' })
  await page.waitForSelector('[data-testid="cad-workbench"]', { timeout: TIMEOUT })
  await page.getByLabel('设计需求', { exact: true }).waitFor({ timeout: TIMEOUT })
  const projectId = await waitForProjectId(pythonBase)

  // Project ID selection does not alter a Rust decision.  It makes the real
  // Rust fixture satisfy the frontend's project-switch/late-result barrier.
  state.fixture = await produceRustFixture(join(temporary, 'fixture.json'), projectId)
  validateFixture(state.fixture, projectId)
  const beforeCanvas = await page.locator('.weapon-viewport canvas').count()
  assert(beforeCanvas === 1, 'workbench must begin with exactly one WebGL canvas')
  const input = page.getByLabel('设计需求', { exact: true })
  await input.fill('设计一台三关节维护机械臂，固定基座、两段连杆、旋转腕部和夹持末端。')
  await page.getByRole('button', { name: '发送设计需求', exact: true }).click()
  const card = page.locator('[data-generation-state="ready"]')
  await card.waitFor({ timeout: TIMEOUT })
  await card.getByText('当前生成结果', { exact: true }).waitFor({ timeout: TIMEOUT })
  assert(await page.locator('[data-generation-state="ready"]').count() === 1, 'fixture page must render one ready result card')
  assert(await page.locator('[data-variant-rank]').count() === 0, 'fixture page must not restore direction/ranking cards')
  assert(await page.locator('.weapon-viewport canvas').count() === 1, 'loading the formal preview must not mount a second renderer canvas')
  await page.waitForFunction(() => document.querySelector('.weapon-viewport canvas')?.getBoundingClientRect().width > 0, null, { timeout: TIMEOUT })
  await page.waitForFunction(
    () => document.querySelector('.weapon-viewport')?.getAttribute('data-blockout-load-state') === 'ready',
    null,
    { timeout: TIMEOUT },
  )
  assert(
    !consoleEvents.some((event) => event.type === 'error' && event.text.includes('THREE.GLTFLoader')),
    'formal preview must decode its embedded PBR textures in the rendered workbench',
  )
  const screenshot = await page.screenshot()
  assert(screenshot.byteLength > 1_000, 'rendered workbench screenshot must contain page pixels')
  const preview = state.fixture.decision.preview
  assert(state.previewGets === 1, 'rendered workbench must fetch the sealed Rust GLB exactly once')

  const save = page.getByRole('button', { name: '保存为可编辑模型', exact: true })
  await save.waitFor({ timeout: TIMEOUT })
  const confirmed = waitForState(() => state.confirms === 1, TIMEOUT, 'result-card confirmation request')
  await save.click()
  await confirmed
  assert(state.confirms === 1, 'result-card save must send exactly one Rust confirmation')
  assert(state.unexpected.length === 0, `fixture proxy observed unexpected V003 routes: ${state.unexpected.join(' | ')}`)
  assert(errors.length === 0, `rendered workbench emitted page errors: ${errors.join(' | ')}`)
  if (evidenceDir) {
    await mkdir(evidenceDir, { recursive: true })
    await Promise.all([
      writeFile(join(evidenceDir, 'workbench.png'), screenshot),
      writeFile(join(evidenceDir, 'network.json'), `${JSON.stringify(state.network, null, 2)}\n`),
      writeFile(join(evidenceDir, 'console.json'), `${JSON.stringify({ page_errors: errors, console: consoleEvents }, null, 2)}\n`),
      writeFile(join(evidenceDir, 'report.json'), `${JSON.stringify({
        schema_version: 'ForgeCADWorkbenchPlaywrightEvidence@1',
        task: 'FGC-V003',
        source: state.fixture.source,
        prompt_domain: 'robotic_arm_concept',
        fixture_domain: 'fixture-defined; not asserted as C106',
        assertions: ['one_ready_result_card', 'single_canvas', 'decoded_pbr_preview', 'frozen_rust_binary_preview', 'result_card_confirm'],
        preview_gets: state.previewGets,
        confirms: state.confirms,
      }, null, 2)}\n`),
    ])
  }
  await page.close()
  console.log(JSON.stringify({
    ok: true, task: 'FGC-V003', validation: 'playwright_rendered_workbench',
    source: state.fixture.source,
    assertions: ['one_ready_result_card', 'single_canvas', 'decoded_pbr_preview', 'frozen_rust_binary_preview', 'result_card_confirm'],
    artifact_profile_id: preview.artifact_profile_id,
    evidence_dir: evidenceDir ?? null,
  }, null, 2))
} finally {
  if (browser) await browser.close().catch(() => undefined)
  if (vite) await stop(vite)
  if (proxy) await new Promise((resolveClose) => proxy.close(resolveClose))
  if (python) await stop(python)
  await rm(temporary, { recursive: true, force: true })
}

function createFixtureProxy({ pythonBase, state }) {
  return createServer(async (request, response) => {
    try {
      if (request.method === 'OPTIONS') {
        response.writeHead(204, corsHeaders())
        return response.end()
      }
      const payload = await requestJson(request)
      if (request.method === 'POST' && request.url === '/api/v1/app-server/connections') return json(response, 200, { connection_id: 'conn_v003_playwright' })
      if (request.method !== 'POST' || request.url !== '/api/v1/app-server/connections/conn_v003_playwright/frames') return json(response, 404, { error: 'not found' })
      const frame = payload?.frame
      if (!frame || frame.jsonrpc !== '2.0' || typeof frame.method !== 'string') return json(response, 200, { frame: failure(null, 'INVALID_REQUEST', 'Malformed protocol frame') })
      if (frame.method === 'initialize') return json(response, 200, { frame: success(frame.id, initialize()) })
      if (frame.method === 'initialized' || frame.method === 'notification/ack') return json(response, 200, { frames: [] })
      if (frame.method === 'compat/subscribe') return json(response, 200, { frame: success(frame.id, { schema_version: 'ForgeCADSseSubscriptionResult@1', stream_id: frame.params.stream_id, subscribed: true }), frames: [] })
      if (frame.method === 'compat/unsubscribe') return json(response, 200, { frame: success(frame.id, { schema_version: 'ForgeCADSseUnsubscribeResult@1', stream_id: frame.params.stream_id, unsubscribed: true }) })
      if (frame.method !== 'compat/http') return json(response, 200, { frame: failure(frame.id ?? null, 'METHOD_NOT_ALLOWED', 'Unsupported fixture method') })
      const result = await fixtureHttp(frame.params, state, pythonBase)
      return json(response, 200, { frame: success(frame.id, result) })
    } catch (error) { return json(response, 500, { error: error instanceof Error ? error.message : String(error) }) }
  })
}

async function fixtureHttp(input, state, pythonBase) {
  assert(input?.schema_version === 'ForgeCADHttpCompatibilityRequest@1', 'compatibility request schema is required')
  state.network.push({ method: input.method, path: input.path, status: 'pending' })
  const fixture = state.fixture
  const headers = new Map((input.headers ?? []).map(([name, value]) => [String(name).toLowerCase(), String(value)]))
  if (fixture) {
    const d = fixture.decision
    const root = `/api/v1/agent/projects/${d.project_id}/turns/${d.turn_id}/single-results/${d.preview.preview_id}`
    if (input.method === 'GET' && input.path === `${root}:preview.glb`) {
      state.previewGets += 1
      assert(headers.get('if-match') === `"sha256:${d.preview.artifact_sha256}"`, 'preview must retain the Rust artifact ETag')
      return http(200, fixture.preview_headers, { encoding: 'base64', data: fixture.preview_glb_base64 })
    }
    if (input.method === 'POST' && input.path === `${root}:confirm`) {
      state.confirms += 1
      assert(headers.get('if-match') === `"sha256:${d.preview.artifact_sha256}"`, 'confirm must retain the Rust artifact ETag')
      assert(headers.get('idempotency-key') === `single-result-confirm-${d.preview.preview_id}`, 'rendered save must retain frontend confirmation identity')
      assert(input.body?.encoding === 'utf8', 'confirm body must be JSON')
      const body = JSON.parse(input.body.data)
      assert(body.expected_artifact_sha256 === d.preview.artifact_sha256 && body.summary === d.summary, 'rendered confirm must transport sealed decision values')
      return http(fixture.confirm_status, [['content-type', 'application/json']], { encoding: 'utf8', data: JSON.stringify(fixture.confirm_response) })
    }
    if (input.method === 'POST' && input.path === '/api/v1/agent/threads') {
      state.threadCreated = true
      return http(201, [['content-type', 'application/json']], utf8(thread(fixture)))
    }
    if (input.method === 'POST' && input.path === `/api/v1/agent/threads/${thread(fixture).thread_id}/turns`) {
      assert(state.threadCreated, 'V003 turn must follow thread creation')
      return http(201, [['content-type', 'application/json']], utf8(turn(fixture, JSON.parse(input.body.data).message)))
    }
    if (input.method === 'GET' && input.path === '/api/v1/agent/threads') return http(200, [['content-type', 'application/json']], utf8({ items: state.threadCreated ? [thread(fixture)] : [], next_cursor: null }))
  }
  state.forwarded.push(`${input.method} ${input.path}`)
  return forwardToPython(input, pythonBase)
}

function thread(fixture) { return { thread_id: 'thread_rust_v003_fixture', project_id: fixture.decision.project_id, title: 'Rust V003 fixture', status: 'idle', summary: 'Rust-owned formal fixture', provider_id: 'rust_app_server', created_at: '2026-07-18T00:00:00Z', updated_at: '2026-07-18T00:00:00Z', last_turn_id: fixture.decision.turn_id, turns: [] } }
function turn(fixture, requestText) { const decision = fixture.decision; return { turn_id: decision.turn_id, thread_id: thread(fixture).thread_id, request_text: requestText, status: 'completed', usage: {}, created_at: '2026-07-18T00:00:00Z', updated_at: '2026-07-18T00:00:00Z', approvals: [], items: [ { item_id: 'item_v003_user', thread_id: thread(fixture).thread_id, turn_id: decision.turn_id, sequence: 1, item_type: 'user_message', status: 'completed', payload: { text: requestText }, created_at: '2026-07-18T00:00:00Z' }, { item_id: 'item_v003_rust_decision', thread_id: thread(fixture).thread_id, turn_id: decision.turn_id, sequence: 2, item_type: 'tool_result', status: 'completed', payload: { tool_name: 'prepare_candidate_preview', tool_result: { validated_output: { value: decision } } }, created_at: '2026-07-18T00:00:01Z' } ] } }

async function forwardToPython(input, pythonBase) {
  const headers = Object.fromEntries((input.headers ?? []).filter(([name]) => !['host', 'content-length'].includes(String(name).toLowerCase())))
  const body = input.body?.encoding === 'base64' ? Buffer.from(input.body.data, 'base64') : input.body?.encoding === 'utf8' ? input.body.data : undefined
  const response = await fetch(`${pythonBase}${input.path}`, { method: input.method, headers, body })
  const bytes = Buffer.from(await response.arrayBuffer())
  const contentType = response.headers.get('content-type') ?? ''
  return http(response.status, [...response.headers.entries()], contentType.includes('json') || contentType.startsWith('text/') ? { encoding: 'utf8', data: bytes.toString('utf8') } : { encoding: 'base64', data: bytes.toString('base64') })
}

async function produceRustFixture(path, projectId) {
  const result = spawnSync('script/with_rust_toolchain.sh', ['cargo', 'test', '--manifest-path', 'apps/desktop/src-tauri/Cargo.toml', '-p', 'wushen-forge-desktop', 'app_server_bridge::tests::formal_single_result_preview_get_and_confirm_create_one_atomic_asset', '--offline', '--', '--exact'], { cwd: ROOT, encoding: 'utf8', env: { ...process.env, FORGECAD_V003_RUST_E2E_FIXTURE_PATH: path, FORGECAD_V003_RUST_E2E_PROJECT_ID: projectId } })
  if (result.status !== 0) throw new Error(`Rust fixture producer failed:\n${result.stdout}\n${result.stderr}`)
  return deepFreeze(JSON.parse(await readFile(path, 'utf8')))
}
function validateFixture(fixture, projectId) { const d = fixture?.decision; assert(fixture?.schema_version === 'ForgeCADV003RustWorkbenchFixture@1' && fixture.source === 'rust_app_server_native_product_tools', 'fixture must be produced by Rust Product Tools'); assert(d?.schema_version === 'SingleResultDecision@1' && d.state === 'ready_for_preview' && d.outcome === 'passed' && d.project_id === projectId, 'fixture decision must be ready and match the rendered project'); assert(hash(Buffer.from(fixture.preview_glb_base64, 'base64')) === d.preview.artifact_sha256, 'Rust fixture bytes must match decision hash'); const headers = new Map(fixture.preview_headers.map(([name, value]) => [String(name).toLowerCase(), String(value)])); assert(headers.get('x-forgecad-glb-sha256') === d.preview.artifact_sha256 && headers.get('x-forgecad-artifact-profile') === d.preview.artifact_profile_id, 'Rust fixture headers must bind hash/profile') }
function initialize() { return { schema_version: 'ForgeCADInitializeResult@1', protocol_version: 'forgecad.app-server/1', connection_id: 'conn_v003_playwright', server_info: { name: 'rust-v003-fixture-proxy', version: '1' }, capabilities: { notifications: true, cursor_replay: true, cancellation: true, notification_ack: true, binary_body_base64: true }, limits: { max_in_flight_requests: 32, max_event_queue: 64, max_frame_bytes: 64 * 1024 * 1024 }, migration_state: { state_owner: 'rust_app_server' } } }
function http(status, headers, body) { return { schema_version: 'ForgeCADHttpCompatibilityResponse@1', status, headers, body } }
function utf8(value) { return { encoding: 'utf8', data: JSON.stringify(value) } }
function success(id, result) { return { jsonrpc: '2.0', id, result } }
function failure(id, applicationCode, message) { return { jsonrpc: '2.0', id, error: { code: -32000, message, data: { schema_version: 'ForgeCADProtocolError@1', application_code: applicationCode } } } }
function hash(bytes) { return createHash('sha256').update(bytes).digest('hex') }
function assert(value, message) { if (!value) throw new Error(message) }
function corsHeaders() { return { 'access-control-allow-origin': '*', 'access-control-allow-headers': 'content-type,idempotency-key,if-match,accept,cache-control', 'access-control-allow-methods': 'GET,POST,DELETE,OPTIONS' } }
function json(response, status, value) { response.writeHead(status, { ...corsHeaders(), 'content-type': 'application/json' }); response.end(JSON.stringify(value)) }
async function requestJson(request) { const chunks = []; for await (const chunk of request) chunks.push(chunk); const raw = Buffer.concat(chunks).toString('utf8'); return raw ? JSON.parse(raw) : {} }
function listen(server, port) { return new Promise((resolveListen) => server.listen(port, '127.0.0.1', resolveListen)) }
function drain(child) { child.stdout?.on('data', () => undefined); child.stderr?.on('data', () => undefined) }
async function waitForHttp(url, child, label) { const started = Date.now(); while (Date.now() - started < TIMEOUT) { if (child.exitCode !== null) throw new Error(`${label} exited before readiness`); try { if ((await fetch(url)).ok) return } catch {} await new Promise((resolveWait) => setTimeout(resolveWait, 100)) } throw new Error(`${label} did not become ready`) }
async function waitForProjectId(base) { const started = Date.now(); while (Date.now() - started < TIMEOUT) { const response = await fetch(`${base}/api/v1/projects`); if (response.ok) { const payload = await response.json(); const id = payload.items?.[0]?.project_id; if (typeof id === 'string') return id } await new Promise((resolveWait) => setTimeout(resolveWait, 100)) } throw new Error('workbench did not create an isolated project') }
async function freePort() { return new Promise((resolvePort, rejectPort) => { const probe = createServer(); probe.unref(); probe.on('error', rejectPort); probe.listen(0, '127.0.0.1', () => { const address = probe.address(); probe.close(() => resolvePort(address.port)) }) }) }
async function stop(child) { if (child.exitCode !== null) return; child.kill('SIGTERM'); await new Promise((resolveStop) => { const timeout = setTimeout(() => { child.kill('SIGKILL'); resolveStop() }, 5_000); child.once('exit', () => { clearTimeout(timeout); resolveStop() }) }) }
async function waitForState(check, timeout, label) { const started = Date.now(); while (Date.now() - started < timeout) { if (check()) return; await new Promise((resolveWait) => setTimeout(resolveWait, 50)) } throw new Error(`${label} timed out`) }
function deepFreeze(value) { if (value && typeof value === 'object' && !Object.isFrozen(value)) { Object.freeze(value); for (const child of Object.values(value)) deepFreeze(child) } return value }
async function launchSystemBrowser() { const executablePath = process.env.WUSHEN_BROWSER_EXECUTABLE; if (executablePath) return chromium.launch({ executablePath, headless: true }); const macChrome = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'; if (process.platform === 'darwin' && existsSync(macChrome)) return chromium.launch({ executablePath: macChrome, headless: true }); return chromium.launch({ channel: process.env.WUSHEN_BROWSER_CHANNEL || 'chrome', headless: true }) }
