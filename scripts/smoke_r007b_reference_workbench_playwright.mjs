#!/usr/bin/env node
// FGC-R007B rendered workbench smoke.  Python creates only the disposable
// compatibility project shell. Every product-state route below is carried by
// the Rust test driver; the proxy never constructs evidence, plan, Snapshot,
// ChangeSet, version, quality, or GLB responses itself.
import { spawn, spawnSync } from 'node:child_process'
import { createHash, randomUUID } from 'node:crypto'
import { createServer } from 'node:http'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'
import { legacyLifecycleTestOracleEnvironment } from './workbench_agent_blockout_test_helper.mjs'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const TIMEOUT = 35_000
const REFERENCE_FIXTURE_NAME = 'c106-arm-visible-surface-reference.png'
const DRIVER_TEST = 'app_server_bridge::tests::r007b_workbench_rust_driver'
// The aggregate gate invokes this independently at least three times. Keep a
// stable diagnostic token per child process, but never reuse the temp root,
// ports, Python shell, Rust SQLite/CAS root, or browser state across runs.
const RUN_ID = process.env.FORGECAD_R007B_RUN_ID ?? randomUUID()
const MANUAL_HOLD_MS = Math.max(0, Math.min(600_000, Number(process.env.FORGECAD_R007B_MANUAL_HOLD_MS ?? 0) || 0))
// An explicitly configured artifact directory is the only persistent output
// from this otherwise fully-ephemeral smoke.  The bundle stores hashes and
// bounded operation facts only; it never persists source statements, license
// text, URLs, local paths, request bodies, or the reference asset itself.
const ARTIFACT_DIR = process.env.FORGECAD_R007B_ARTIFACT_DIR ? resolve(process.env.FORGECAD_R007B_ARTIFACT_DIR) : null
const ARTIFACT_BUNDLE_FILE = `r007b-reference-evidence-${safeRunId(RUN_ID)}.json`
const ARTIFACT_REFERENCE_SCREENSHOT_FILE = `r007b-reference-workbench-reference-${safeRunId(RUN_ID)}.png`
const ARTIFACT_RESULT_SCREENSHOT_FILE = `r007b-reference-workbench-result-${safeRunId(RUN_ID)}.png`
const SCRIPT_PATH = fileURLToPath(import.meta.url)
const temporary = await mkdtemp(join(tmpdir(), `forgecad-r007b-workbench-${safeRunId(RUN_ID)}-`))
let python = null
let proxy = null
let vite = null
let browser = null

async function main() {
try {
  if (ARTIFACT_DIR) await mkdir(ARTIFACT_DIR, { recursive: true })
  const smokeSourceSha256 = await sha256File(SCRIPT_PATH)
  const pythonPort = await freePort()
  const proxyPort = await freePort()
  const vitePort = await freePort()
  const pythonBase = `http://127.0.0.1:${pythonPort}`
  const proxyBase = `http://127.0.0.1:${proxyPort}`
  const viteBase = `http://127.0.0.1:${vitePort}`
  const compatLibrary = join(temporary, 'python-compat-library')
  const rustLibrary = join(temporary, 'rust-product-library')
  assert(new Set([pythonPort, proxyPort, vitePort]).size === 3, 'R007B isolated run requires three distinct loopback ports')
  assert(compatLibrary !== rustLibrary, 'Python compatibility shell and Rust product state must not share a library root')

  python = spawn(join(ROOT, '.venv', 'bin', 'python'), ['-m', 'uvicorn', 'wushen_agent.test_oracle:create_app', '--factory', '--host', '127.0.0.1', '--port', String(pythonPort)], {
    cwd: ROOT,
    env: legacyLifecycleTestOracleEnvironment(process.env, {
      WUSHEN_LIBRARY_ROOT: compatLibrary, WUSHEN_MIGRATIONS_DIR: join(ROOT, 'migrations'),
      WUSHEN_CORS_ORIGINS: viteBase, WUSHEN_LOCAL_WORKER_ENABLED: '0', FORGECAD_CONCEPT_WORKER_ENABLED: '1',
      FORGECAD_CONCEPT_PLANNER_PROVIDER: 'deterministic_rules',
    }), stdio: ['ignore', 'pipe', 'pipe'],
  })
  drain(python)
  await waitForHttp(`${pythonBase}/api/health`, python, 'Python compatibility shell')
  const projectId = await createCompatibilityProject(pythonBase)
  const driver = new RustR007BDriver({ libraryRoot: rustLibrary, projectId, temporary })
  await driver.bootstrap()
  // R007B is allowed to lower reference facts into an A005 program only after
  // the user-owned capability has been explicitly enabled.  The production
  // golden path performs this through the A005 drawer before reference import;
  // this isolated rendered contract performs the same Rust-owned confirmation
  // directly so a missing or stale activation remains a hard failure instead
  // of being hidden by test setup.
  const enableKey = `r007b-enable-a005-v2-${safeRunId(RUN_ID)}`
  const enabledSkill = await driver.postJson(
    '/api/v1/agent/skills/surface-adornment:enable',
    enableKey,
    {
      schema_version: 'EnableSurfaceAdornmentSkillRequest@1',
      client_request_id: enableKey,
      confirm_enable: true,
    },
  )
  assert(
    enabledSkill?.status === 'enabled'
      && enabledSkill?.activation?.skill_version === 2
      && /^[a-f0-9]{64}$/.test(enabledSkill?.activation?.skill_sha256 ?? ''),
    `R007B rendered flow requires the explicitly enabled immutable A005 v2 activation: ${JSON.stringify(enabledSkill)}`,
  )

  const proxyState = { rust_requests: [], python_product_route_attempts: 0 }
  proxy = createDriverProxy({ driver, pythonBase, state: proxyState })
  await listen(proxy, proxyPort)
  vite = spawn(process.execPath, [join(ROOT, 'node_modules', 'vite', 'bin', 'vite.js'), '--host', '127.0.0.1', '--port', String(vitePort)], {
    cwd: join(ROOT, 'apps', 'desktop'), env: { ...process.env, VITE_FORGE_API_BASE_URL: proxyBase }, stdio: ['ignore', 'pipe', 'pipe'],
  })
  drain(vite)
  await waitForHttp(viteBase, vite, 'R007B Vite workbench')
  browser = await launchBrowser()
  const referenceFixture = await createReferenceImageFixture(browser)
  const fixtureSourceSha256 = sha256Bytes(referenceFixture.bytes)
  const page = await browser.newPage({ viewport: { width: 1440, height: 960 } })
  const errors = []
  page.on('pageerror', error => errors.push(error.message))
  await page.goto(`${viteBase}/#/cad`, { waitUntil: 'networkidle' })
  await page.waitForSelector('[data-testid="cad-workbench"]', { timeout: TIMEOUT })
  await page.getByLabel('设计需求', { exact: true }).waitFor({ timeout: TIMEOUT })
  // The Rust bootstrap owns the base asset; reloading is intentionally after
  // it, so the browser never observes a Python-created design version.
  await page.reload({ waitUntil: 'networkidle' })
  await page.getByLabel('设计需求', { exact: true }).waitFor({ timeout: TIMEOUT })
  await page.waitForFunction(() => document.querySelectorAll('.weapon-viewport canvas').length === 1, null, { timeout: TIMEOUT })

  const initial = await driver.getJson('GET', `/api/v1/projects/${projectId}/active-design`)
  const baseId = initial.active_design?.asset_version_id
  assert(typeof baseId === 'string', 'Rust bootstrap must provide an editable base asset')
  const baseAsset = await driver.getJson('GET', `/api/v1/agent/asset-versions/${baseId}`)
  assertC106RecipeAsset(baseAsset)
  const baseProduction = await readProductionGlb(driver, baseId)
  await waitForProductionViewport(page)
  const rejected = await runReferenceFlow({ page, driver, proxyState, projectId, baseId, suffix: 'reject', confirm: false, referenceFixture, capturePair: false })
  // Rejection clears Rust preview state. Reload the same workbench before the
  // evidence-producing confirmation run so stale preview presentation cannot
  // be mistaken for the C106 base/result comparison renderer.
  await page.reload({ waitUntil: 'networkidle' })
  await page.getByLabel('设计需求', { exact: true }).waitFor({ timeout: TIMEOUT })
  await waitForProductionViewport(page)
  const rendererBaseline = await readRendererIdentity(page)
  const confirmed = await runReferenceFlow({ page, driver, proxyState, projectId, baseId, suffix: 'confirm', confirm: true, referenceFixture, capturePair: Boolean(ARTIFACT_DIR) })
  const finalSnapshot = await driver.getJson('GET', `/api/v1/projects/${projectId}/active-design`)
  assert(finalSnapshot.preview == null, 'R007B terminal paths must clear preview')
  assert(finalSnapshot.active_design?.asset_version_id === confirmed.result_asset_version_id, 'confirmation must activate exactly its one result asset')
  assert(finalSnapshot.active_design?.asset_version_id !== baseId, 'confirmation must create an immutable child version')
  assert(rejected.result_asset_version_id === null && rejected.result_glb_sha256 === null, 'rejection must not produce a result asset')
  assert(await page.locator('.weapon-viewport canvas').count() === 1, 'reference A/B viewing must retain one WebGL canvas')
  assert(proxyState.python_product_route_attempts === 0, 'Python compatibility shell must never receive a product-state route')
  assert(proxyState.rust_requests.length > 0, 'rendered workbench must exercise Rust-owned product routes')
  assert(!errors.length, `workbench emitted page errors: ${errors.join(' | ')}`)
  const resultProduction = await readProductionGlb(driver, confirmed.result_asset_version_id)
  assert(resultProduction.sha256 === confirmed.result_glb_sha256, 'confirmed result production GLB bytes must match frozen R007B result hash')
  assertC106RecipeAsset(confirmed._evidence_bundle.resultAsset)
  const rendererFinal = await readRendererIdentity(page)
  assertRendererStable(rendererBaseline, rendererFinal)
  const evidenceBundle = await writeEvidenceBundle({
    smokeSourceSha256,
    fixtureSourceSha256,
    projectId,
    baseId,
    baseAsset,
    baseProduction,
    resultProduction,
    rendererBaseline,
    rendererFinal,
    rejected,
    confirmed,
    proxyState,
  })
  console.log(JSON.stringify({
    schema_version: 'R007BReferenceWorkbenchPlaywright@1', status: 'pass', task: 'FGC-R007B',
    assertions: ['single_image_file_input_evidence', 'explicit_a005_v2_enable', 'rust_driver_only_product_state', 'isolated_ephemeral_run', 'real_c106_production_glb_base_and_result', 'strict_c106_base_complete_lineage', 'preview_zero_version', 'same_workbench_reference_result_pair', 'paired_reference_result_screenshots', 'exact_evidence_plan_changeset_result_lineage', 'reference_effect_changes_sealed_design_surface', 'reject_zero_result', 'confirm_one_result', 'stable_renderer_generation', 'single_webgl_context', 'no_similarity_or_visual_score'],
    reference: { rejected: publicIdentity(rejected), confirmed: publicIdentity(confirmed) },
    evidence_bundle: evidenceBundle,
    formal_eligible: false, visual_fidelity_validated: false,
    isolation: {
      run_id: RUN_ID,
      project_id: projectId,
      ephemeral_roots_unique: true,
      distinct_loopback_ports: true,
      python_compatibility_shell_only: true,
      rust_product_state_only: true,
      rust_product_request_count: proxyState.rust_requests.length,
      python_product_route_attempts: proxyState.python_product_route_attempts,
    },
  }))
  if (MANUAL_HOLD_MS > 0) {
    console.log(JSON.stringify({
      schema_version: 'R007BReferenceWorkbenchManualSession@1',
      status: 'ready',
      url: `${viteBase}/#/cad`,
      project_id: projectId,
      hold_ms: MANUAL_HOLD_MS,
    }))
    await sleep(MANUAL_HOLD_MS)
  }
} finally {
  if (browser) await browser.close().catch(() => undefined)
  if (vite) await stop(vite)
  if (proxy) await new Promise(resolveClose => proxy.close(resolveClose))
  if (python) await stop(python)
  await rm(temporary, { recursive: true, force: true })
}
}

async function runReferenceFlow({ page, driver, proxyState, projectId, baseId, suffix, confirm, referenceFixture, capturePair }) {
  await page.getByLabel('添加风格、材质或参考').click()
  const menu = page.getByRole('menu', { name: '设计附加操作' })
  const referenceAction = menu.locator('button[role="menuitem"]').filter({ hasText: '参考图 / GLB' })
  assert(await referenceAction.count() === 1, 'composer must expose exactly one reference evidence action')
  await referenceAction.click()
  const drawer = page.getByLabel('添加参考证据')
  await drawer.waitFor({ timeout: TIMEOUT })
  await drawer.locator('input[type="file"]').setInputFiles({
    name: REFERENCE_FIXTURE_NAME,
    mimeType: 'image/png',
    buffer: referenceFixture.bytes,
  })
  await drawer.getByLabel('后视图', { exact: true }).check()
  await drawer.getByLabel('顶视图', { exact: true }).check()
  const sourceStatement = `R007B ${suffix} authorized single-image fixture`
  await drawer.getByLabel('来源说明').fill(sourceStatement)
  await drawer.getByLabel('使用授权 / 权利声明').fill('Authorized only for ForgeCAD constrained concept rebuild verification.')
  const evidenceIndexBefore = await driver.getJson('GET', `/api/v1/agent/projects/${projectId}/reference-evidence`)
  const evidenceIdsBefore = new Set(evidenceIndexBefore.reference_evidence.map(item => item.evidence_id))
  const before = await driver.getJson('GET', `/api/v1/projects/${projectId}/active-design`)
  await drawer.getByRole('button', { name: '保存只读参考证据', exact: true }).click()
  const previewButton = drawer.getByRole('button', { name: '生成受限重建预览', exact: true })
  try { await previewButton.waitFor({ timeout: TIMEOUT }) } catch { throw new Error(`reference evidence did not reach evidence_ready: ${await drawer.innerText()}`) }
  const indexAfterEvidence = await driver.getJson('GET', `/api/v1/agent/projects/${projectId}/reference-evidence`)
  const newEvidence = indexAfterEvidence.reference_evidence.filter(item => !evidenceIdsBefore.has(item.evidence_id) && item.source_statement === sourceStatement)
  assert(newEvidence.length === 1, `saving evidence must add exactly one identifiable record: ${JSON.stringify(indexAfterEvidence.reference_evidence)}`)
  const [evidence] = newEvidence
  assert(evidence?.source_object_sha256?.match(/^[a-f0-9]{64}$/), 'evidence must expose an immutable source hash')
  assert(evidence.kind === 'image' && evidence.reference_class === 'single_image', 'R007B rendered flow must retain a single-image evidence class')
  assert(evidence.source_object_sha256 === sha256Bytes(referenceFixture.bytes), 'saved image evidence must bind the exact uploaded PNG bytes')
  assert(Array.isArray(evidence.missing_views) && evidence.missing_views.length > 0, 'single-image evidence must retain unresolved missing views')
  const referenceFigure = drawer.getByLabel('只读参考图片')
  await referenceFigure.waitFor({ timeout: TIMEOUT })
  await referenceFigure.locator('img').evaluate(image => {
    if (!(image instanceof HTMLImageElement) || !image.complete || image.naturalWidth < 64 || image.naturalHeight < 64) {
      throw new Error('reference image did not decode in the workbench')
    }
  })
  const afterEvidence = await driver.getJson('GET', `/api/v1/projects/${projectId}/active-design`)
  assert(stableSnapshot(before) === stableSnapshot(afterEvidence), 'saving read-only evidence must not advance the design version')
  const referenceCapture = capturePair
    ? await captureWorkbenchPairSurface(page, ARTIFACT_REFERENCE_SCREENSHOT_FILE, 'reference')
    : null

  await previewButton.click()
  // The reference-view action exists as soon as evidence is saved, so it is
  // not a preview completion signal. A retain action plus frozen lineage are.
  try {
    await drawer.getByRole('button', { name: '保留新版本', exact: true }).waitFor({ timeout: TIMEOUT })
  } catch {
    throw new Error(`reference preview did not reach preview_ready: ${await drawer.innerText()}`)
  }
  await drawer.getByLabel('参考重建证据谱系').waitFor({ timeout: TIMEOUT })
  const planIndex = await driver.getJson('GET', `/api/v1/agent/projects/${projectId}/reference-evidence`)
  const planSummary = planIndex.reference_guided_rebuild_plans.find(item => item.evidence_id === evidence.evidence_id)
  assert(planSummary?.rebuild_plan_id, `preview must create a project-scoped rebuild plan: ${JSON.stringify(planIndex.reference_guided_rebuild_plans)}`)
  const planRead = await driver.getJson('GET', `/api/v1/agent/projects/${projectId}/reference-guided-rebuild-plans/${planSummary.rebuild_plan_id}`)
  const plan = planRead.reference_guided_rebuild_plan
  const analysis = planRead.reference_surface_analysis
  const previewPair = planRead.reference_result_pair
  assert(
    plan.evidence_id === evidence.evidence_id && plan.project_id === projectId && plan.status === 'previewed',
    `reference and rebuild plan identities must agree exactly: ${JSON.stringify({ evidence, plan, plans: planIndex.reference_guided_rebuild_plans })}`,
  )
  assert(analysis?.evidence_id === evidence.evidence_id && typeof analysis?.analysis_id === 'string', 'preview must retain exact analysis identity')
  assertCompleteC106Lineage({ plan, analysis, evidence, projectId, baseId })
  assert(previewPair?.source_object_sha256 === evidence.source_object_sha256 && previewPair?.result_asset_version_id === null && previewPair?.result_glb_sha256 === null, 'preview must not claim frozen result GLB')
  const preview = await driver.getJson('GET', `/api/v1/projects/${projectId}/active-design`)
  const changeSetId = preview.preview?.change_set_id
  assert(typeof changeSetId === 'string' && preview.preview.base_asset_version_id === baseId && preview.active_design?.asset_version_id === baseId, 'preview must bind one ChangeSet and create zero versions')
  assert(plan.preview_change_set_id === changeSetId, 'plan and Snapshot must name same ChangeSet')
  const sealedChangeSet = await driver.getJson('GET', `/api/v1/agent/change-sets/${changeSetId}`)
  const sealedOperations = Array.isArray(sealedChangeSet?.operations) ? sealedChangeSet.operations : []
  assert(sealedOperations.length >= 1, 'R007B preview must seal at least one bounded ChangeSet operation')
  // R007B can lower one reference to several existing bounded operations.
  // Do not assume ordering: it must at minimum carry a real A005 operation,
  // whose exact program is then proved in the confirmed AssemblyGraph below.
  const adornmentOperations = sealedOperations.filter(operation => operation?.op === 'apply_surface_adornment')
  assert(adornmentOperations.length >= 1, `R007B must seal a real apply_surface_adornment operation: ${JSON.stringify(redactedOperations(sealedOperations))}`)
  for (const operation of adornmentOperations) assertSurfaceAdornmentOperation(operation)
  await assertDrawerLineage(drawer, [evidence.evidence_id, evidence.source_object_sha256, plan.rebuild_plan_id, analysis.analysis_id, changeSetId])
  assert((await drawer.innerText()).includes('不显示相似度分数'), 'drawer must explicitly reject similarity scoring')
  assert(!/相似度\s*[0-9]|视觉评分|score/i.test(await drawer.innerText()), 'workbench must not render a score')

  const viewportToken = `r007b-${suffix}-${Date.now()}`
  await page.locator('.weapon-viewport').evaluate((element, token) => element.setAttribute('data-r007b-viewport-token', token), viewportToken)
  assert(await webglViewportIsSingleAndStable(page, viewportToken), 'single-image comparison must retain one stable C106 production viewport beside the visible reference image')

  if (!confirm) {
    await drawer.getByRole('button', { name: '取消', exact: true }).click()
    try {
      await drawer.waitFor({ state: 'detached', timeout: TIMEOUT })
    } catch {
      throw new Error(`reference cancel did not reach its Rust-confirmed terminal state: ${await drawer.innerText()}`)
    }
    const rejectedRead = await driver.getJson('GET', `/api/v1/agent/projects/${projectId}/reference-guided-rebuild-plans/${plan.rebuild_plan_id}`)
    assert(rejectedRead.reference_guided_rebuild_plan.status === 'rejected', 'cancel must reject exactly previewed plan')
    assert(rejectedRead.reference_result_pair?.source_object_sha256 === evidence.source_object_sha256 && rejectedRead.reference_result_pair?.result_asset_version_id === null && rejectedRead.reference_result_pair?.result_glb_sha256 === null, 'rejection must preserve source identity without result GLB')
    const afterReject = await driver.getJson('GET', `/api/v1/projects/${projectId}/active-design`)
    assert(
      afterReject.active_design?.asset_version_id === baseId && afterReject.preview == null,
      `rejection must retain base and clear preview: ${JSON.stringify(afterReject)}`,
    )
    return identity({ evidence, plan: rejectedRead.reference_guided_rebuild_plan, analysis, changeSetId, sealedOperations, resultAsset: null, resultGlbSha256: null, referenceCapture, resultCapture: null })
  }
  await drawer.getByRole('button', { name: '保留新版本', exact: true }).click()
  try {
    await drawer.getByText(/已保留参考引导重建并创建可编辑资产/).waitFor({ timeout: TIMEOUT })
  } catch {
    throw new Error(`reference confirm did not reach its Rust-confirmed terminal state: ${await drawer.innerText()}`)
  }
  const confirmedRead = await driver.getJson('GET', `/api/v1/agent/projects/${projectId}/reference-guided-rebuild-plans/${plan.rebuild_plan_id}`)
  const confirmedPlan = confirmedRead.reference_guided_rebuild_plan
  const confirmedPair = confirmedRead.reference_result_pair
  const resultId = confirmedPlan.confirmed_asset_version_id
  assert(confirmedPlan.status === 'confirmed' && typeof resultId === 'string', 'confirm must freeze exactly one result asset')
  assert(confirmedPair?.source_object_sha256 === evidence.source_object_sha256 && confirmedPair?.result_asset_version_id === resultId, 'confirmed pair must retain exact source and result identity')
  assert(typeof confirmedPair?.result_glb_sha256 === 'string' && confirmedPair.result_glb_sha256.match(/^[a-f0-9]{64}$/) && confirmedPair.result_glb_sha256 !== evidence.source_object_sha256, 'confirmed result must have distinct immutable GLB hash')
  const [baseAsset, resultAsset] = await Promise.all([
    driver.getJson('GET', `/api/v1/agent/asset-versions/${baseId}`),
    driver.getJson('GET', `/api/v1/agent/asset-versions/${resultId}`),
  ])
  assertReferenceSurfaceReadback({ baseAsset, resultAsset, sealedOperations })
  await assertDrawerLineage(drawer, [evidence.evidence_id, evidence.source_object_sha256, plan.rebuild_plan_id, analysis.analysis_id, changeSetId, resultId, confirmedPair.result_glb_sha256])
  await waitForProductionViewport(page)
  assert(await webglViewportIsSingleAndStable(page, viewportToken), 'confirmed single-image result must retain the same ModuleGraph renderer generation')
  const resultCapture = capturePair
    ? await captureWorkbenchPairSurface(page, ARTIFACT_RESULT_SCREENSHOT_FILE, 'result')
    : null
  if (capturePair) assertPairedCaptures(referenceCapture, resultCapture)
  return identity({ evidence, plan: confirmedPlan, analysis, changeSetId, sealedOperations, resultAsset, resultGlbSha256: confirmedPair.result_glb_sha256, referenceCapture, resultCapture })
}

class RustR007BDriver {
  constructor({ libraryRoot, projectId, temporary }) { this.libraryRoot = libraryRoot; this.projectId = projectId; this.commandFile = join(temporary, 'r007b-driver-command.json'); this.outputFile = join(temporary, 'r007b-driver-output.json'); this.serial = Promise.resolve() }
  bootstrap() { return this.invoke({ operation: 'bootstrap' }) }
  async getJson(method, path) { const response = await this.http({ method, path, headers: [], body: { encoding: 'empty' } }); assert(response.status >= 200 && response.status < 300, `Rust driver ${method} ${path} returned ${response.status}`); return responseJson(response) }
  async getBinary(path) {
    const response = await this.http({ method: 'GET', path, headers: [], body: { encoding: 'empty' } })
    assert(response.status >= 200 && response.status < 300, `Rust driver GET ${path} returned ${response.status}`)
    assert(response.body?.encoding === 'base64', `Rust driver GET ${path} must return base64 binary bytes`)
    return { bytes: Buffer.from(response.body.data, 'base64'), headers: responseHeaders(response) }
  }
  async postJson(path, clientRequestId, payload) {
    const response = await this.http({
      method: 'POST',
      path,
      headers: [['Content-Type', 'application/json'], ['Idempotency-Key', clientRequestId]],
      body: { encoding: 'utf8', data: JSON.stringify(payload) },
    })
    assert(response.status >= 200 && response.status < 300, `Rust driver POST ${path} returned ${response.status}: ${JSON.stringify(responseJson(response))}`)
    return responseJson(response)
  }
  http(request) {
    const normalized = { method: request.method, path: request.path, headers: request.headers ?? [] }
    if (request.body !== null && request.body !== undefined) normalized.body = request.body
    return this.invoke({ operation: 'request', request: normalized }).then(readDriverHttp)
  }
  invoke(command) {
    const next = this.serial.then(async () => {
      await writeFile(this.commandFile, JSON.stringify({ schema_version: 'ForgeCADR007BWorkbenchRustDriverCommand@1', ...command, project_id: this.projectId }), 'utf8')
      const result = spawnSync('script/with_rust_toolchain.sh', ['cargo', 'test', '--manifest-path', 'apps/desktop/src-tauri/Cargo.toml', '-p', 'wushen-forge-desktop', DRIVER_TEST, '--offline', '--', '--ignored', '--exact'], {
        cwd: ROOT, encoding: 'utf8', env: {
          ...process.env,
          FORGECAD_R007B_WORKBENCH_DRIVER_COMMAND: this.commandFile,
          FORGECAD_R007B_WORKBENCH_DRIVER_OUTPUT: this.outputFile,
          FORGECAD_R007B_WORKBENCH_DRIVER_LIBRARY_ROOT: this.libraryRoot,
        },
      })
      if (result.status !== 0) throw new Error(`Rust R007B driver failed:\n${result.stdout}\n${result.stderr}`)
      return JSON.parse(await readFile(this.outputFile, 'utf8'))
    })
    this.serial = next.catch(() => undefined)
    return next
  }
}

function createDriverProxy({ driver, pythonBase, state }) {
  return createServer(async (request, response) => {
    try {
      if (request.method === 'OPTIONS') return json(response, 204, {})
      const payload = await requestJson(request)
      if (request.method === 'POST' && request.url === '/api/v1/app-server/connections') return json(response, 200, { connection_id: 'conn_r007b_workbench' })
      if (request.method !== 'POST' || request.url !== '/api/v1/app-server/connections/conn_r007b_workbench/frames') return json(response, 404, { error: 'not found' })
      const frame = payload?.frame
      if (!frame || frame.jsonrpc !== '2.0' || typeof frame.method !== 'string') return json(response, 200, { frame: failure(null, 'INVALID_REQUEST', 'Malformed protocol frame') })
      if (frame.method === 'initialize') return json(response, 200, { frame: success(frame.id, initialize()) })
      if (frame.method === 'initialized' || frame.method === 'notification/ack') return json(response, 200, { frames: [] })
      if (frame.method === 'compat/subscribe') return json(response, 200, { frame: success(frame.id, { schema_version: 'ForgeCADSseSubscriptionResult@1', stream_id: frame.params.stream_id, subscribed: true }), frames: [] })
      if (frame.method === 'compat/unsubscribe') return json(response, 200, { frame: success(frame.id, { schema_version: 'ForgeCADSseUnsubscribeResult@1', stream_id: frame.params.stream_id, unsubscribed: true }) })
      if (frame.method !== 'compat/http') return json(response, 200, { frame: failure(frame.id ?? null, 'METHOD_NOT_ALLOWED', 'Unsupported fixture method') })
      const product = isRustProductRoute(frame.params?.path)
      const result = product ? await driver.http(frame.params) : await forwardToPython(frame.params, pythonBase)
      if (product) state.rust_requests.push({ request: { method: frame.params.method, path: frame.params.path }, response: result })
      else if (isPotentialProductRoute(frame.params?.path)) state.python_product_route_attempts += 1
      return json(response, 200, { frame: success(frame.id, result) })
    } catch (error) { return json(response, 500, { error: error instanceof Error ? error.message : String(error) }) }
  })
}

function isRustProductRoute(path) {
  return /^\/api\/v1\/projects\/[^/]+\/active-design(?:$|[:/?])/.test(path)
    || /^\/api\/v1\/agent\/(?:reference-evidence:create|projects\/[^/]+\/(?:reference-evidence|reference-guided-rebuild-plans|reference-guided-rebuild:preview)|asset-versions|quality-reports|change-sets)/.test(path)
    || /^\/api\/v1\/change-sets\//.test(path)
}
function isPotentialProductRoute(path) {
  return typeof path === 'string' && (
    /^\/api\/v1\/projects\/[^/]+\/active-design(?:$|[:/?])/.test(path)
    || /^\/api\/v1\/agent\/(?:reference-evidence:create|projects\/[^/]+\/(?:reference-evidence|reference-guided-rebuild-plans|reference-guided-rebuild:preview)|asset-versions|quality-reports|change-sets)/.test(path)
    || /^\/api\/v1\/change-sets\//.test(path)
  )
}
function readDriverHttp(output) {
  const response = output?.response ?? output?.http_response ?? output?.result?.response ?? output?.result
  assert(response?.schema_version === 'ForgeCADHttpCompatibilityResponse@1', 'Rust driver must return its authoritative compat/http response')
  return response
}
function responseJson(response) { assert(response.body?.encoding === 'utf8', 'expected a Rust JSON response'); return JSON.parse(response.body.data) }
function identity({ evidence, plan, analysis, changeSetId, sealedOperations, resultAsset, resultGlbSha256, referenceCapture, resultCapture }) {
  return {
    evidence_id: evidence.evidence_id,
    source_object_sha256: evidence.source_object_sha256,
    rebuild_plan_id: plan.rebuild_plan_id,
    analysis_id: analysis.analysis_id,
    preview_change_set_id: changeSetId,
    result_asset_version_id: plan.confirmed_asset_version_id ?? null,
    result_glb_sha256: resultGlbSha256,
    // Stored only in the configured redacted evidence bundle.  Keeping the
    // raw API objects here avoids accidentally treating a console report as a
    // provenance archive.
    _evidence_bundle: { plan, analysis, sealedChangeSetId: changeSetId, sealedOperations, resultAsset, referenceCapture, resultCapture },
  }
}
function publicIdentity(identityValue) {
  return {
    source_object_sha256: identityValue.source_object_sha256,
    result_glb_sha256: identityValue.result_glb_sha256,
    result_created: Boolean(identityValue.result_asset_version_id),
  }
}
function assertSurfaceAdornmentOperation(operation) {
  const program = operation?.surface_adornment_program
  assert(
    operation?.op === 'apply_surface_adornment'
      && typeof operation?.operation_id === 'string'
      && typeof operation?.part_id === 'string'
      && typeof operation?.material_zone_id === 'string'
      && program?.schema_version === 'SurfaceAdornmentProgram@1'
      && typeof program?.program_id === 'string'
      && program.target_part_id === operation.part_id
      && program.target_zone_id === operation.material_zone_id,
    'R007B apply_surface_adornment operation must retain one exact bounded A005 program and target.',
  )
}
function assertReferenceSurfaceReadback({ baseAsset, resultAsset, sealedOperations }) {
  const adornmentOperations = sealedOperations.filter(operation => operation?.op === 'apply_surface_adornment')
  assert(adornmentOperations.length >= 1, 'confirmed R007B result requires a sealed A005 operation')
  const resultPrograms = resultAsset?.assembly_graph?.surface_adornments
  assert(Array.isArray(resultPrograms), 'confirmed R007B AssemblyGraph must expose surface_adornments readback')
  for (const operation of adornmentOperations) {
    assertSurfaceAdornmentOperation(operation)
    const sealedProgram = operation.surface_adornment_program
    const readbackProgram = resultPrograms.find(program => program?.program_id === sealedProgram.program_id)
    assert(readbackProgram && canonicalJson(readbackProgram) === canonicalJson(sealedProgram), 'confirmed R007B AssemblyGraph must retain the exact sealed SurfaceAdornmentProgram')
  }
  // Preserve a second, generic readback check for parameter operations without
  // assuming they are first or that every reference class uses one.
  for (const operation of sealedOperations.filter(item => item?.op === 'set_part_parameter')) {
    const targetOperationId = baseAsset?.assembly_graph?.parts?.find(part => part?.part_id === operation.part_id)?.operation_id
    const baseOperation = baseAsset?.shape_program?.operations?.find(item => item?.operation_id === targetOperationId)
    const resultOperation = resultAsset?.shape_program?.operations?.find(item => item?.operation_id === targetOperationId)
    assert(typeof targetOperationId === 'string' && baseOperation && resultOperation, 'R007B parameter target must retain its stable C106 part-to-operation mapping')
    assert(canonicalJson(baseOperation) !== canonicalJson(resultOperation), 'confirmed R007B asset must contain each sealed visible parameter change')
  }
}
function redactedOperations(operations) {
  return operations.map(operation => ({
    op: operation?.op ?? null,
    operation_sha256: sha256Canonical(operation),
    program_id: operation?.surface_adornment_program?.program_id ?? null,
  }))
}
function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  if (value && typeof value === 'object') return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`
  return JSON.stringify(value)
}
function sha256Canonical(value) { return createHash('sha256').update(canonicalJson(value)).digest('hex') }
async function sha256File(path) { return createHash('sha256').update(await readFile(path)).digest('hex') }
function sha256Bytes(value) { return createHash('sha256').update(value).digest('hex') }
function sha256Identity(label, value) { return sha256Canonical({ label, value }) }
async function createReferenceImageFixture(activeBrowser) {
  const fixturePage = await activeBrowser.newPage({ viewport: { width: 960, height: 720 }, deviceScaleFactor: 1 })
  try {
    await fixturePage.setContent(`<!doctype html>
      <html><head><style>
        html,body{margin:0;width:960px;height:720px;overflow:hidden;background:#111925}
        #reference{width:960px;height:720px;display:block}
      </style></head><body>
      <svg id="reference" viewBox="0 0 960 720" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="deterministic robotic arm visible-surface reference fixture">
        <defs>
          <linearGradient id="bg" x2="0" y2="1"><stop stop-color="#172334"/><stop offset="1" stop-color="#0a111b"/></linearGradient>
          <linearGradient id="blue" x2="0.8" y2="1"><stop stop-color="#388cff"/><stop offset="0.55" stop-color="#1552aa"/><stop offset="1" stop-color="#092851"/></linearGradient>
          <linearGradient id="steel" x2="0.7" y2="1"><stop stop-color="#526277"/><stop offset="0.5" stop-color="#161f2c"/><stop offset="1" stop-color="#05080d"/></linearGradient>
          <filter id="shadow"><feDropShadow dx="0" dy="18" stdDeviation="18" flood-color="#000" flood-opacity=".6"/></filter>
          <pattern id="grid" width="48" height="48" patternUnits="userSpaceOnUse"><path d="M48 0H0V48" fill="none" stroke="#7ba6d9" stroke-opacity=".08"/></pattern>
        </defs>
        <rect width="960" height="720" fill="url(#bg)"/><rect width="960" height="720" fill="url(#grid)"/>
        <g filter="url(#shadow)" stroke-linejoin="round">
          <path d="M305 588h350l58 48-31 45H273l-30-45z" fill="url(#steel)" stroke="#7890aa" stroke-width="4"/>
          <path d="M326 575h310l35 28-29 26H303l-26-26z" fill="url(#blue)" stroke="#4ea1ff" stroke-width="3"/>
          <path d="M367 575c0-66 50-119 113-119s114 53 114 119" fill="url(#steel)" stroke="#71839a" stroke-width="5"/>
          <circle cx="481" cy="479" r="79" fill="#101824" stroke="#24364d" stroke-width="18"/>
          <circle cx="481" cy="479" r="51" fill="url(#blue)" stroke="#7cc0ff" stroke-width="5"/>
          <circle cx="481" cy="479" r="24" fill="#09111c" stroke="#9ab0c9" stroke-width="5"/>
          <path d="M464 425L326 248l64-55 151 205-15 50z" fill="url(#blue)" stroke="#63adff" stroke-width="5"/>
          <path d="M494 403L373 235l30-25 137 180z" fill="url(#steel)" stroke="#778ba3" stroke-width="4"/>
          <circle cx="355" cy="228" r="70" fill="#0d1622" stroke="#24364d" stroke-width="16"/>
          <circle cx="355" cy="228" r="45" fill="url(#blue)" stroke="#7cc0ff" stroke-width="5"/>
          <circle cx="355" cy="228" r="20" fill="#060b12" stroke="#a2b5c9" stroke-width="5"/>
          <path d="M306 205L191 297l-42-56 119-103z" fill="url(#blue)" stroke="#63adff" stroke-width="5"/>
          <path d="M196 282l-62 53-31-36 61-56z" fill="url(#steel)" stroke="#778ba3" stroke-width="4"/>
          <g fill="#111b27" stroke="#8ba1b9" stroke-width="3">
            <path d="M104 319l35 28-23 68-22-11 8-42-31 22-15-18z"/>
            <path d="M135 347l32 20-7 59-22-2-1-36-25 27-17-14z"/>
          </g>
          <path d="M534 431c45-12 67-43 58-91-7-37-6-77 35-94" fill="none" stroke="#1b9cff" stroke-width="8"/>
          <path d="M540 444c61-4 88-40 79-97-5-31 2-63 36-82" fill="none" stroke="#0a1725" stroke-width="15"/>
          <path d="M540 444c61-4 88-40 79-97-5-31 2-63 36-82" fill="none" stroke="#3a6ea5" stroke-width="4"/>
          <g fill="#8bd4ff"><circle cx="323" cy="292" r="5"/><circle cx="343" cy="318" r="5"/><circle cx="363" cy="344" r="5"/></g>
          <path d="M330 583h298" stroke="#7ad8ff" stroke-width="8" stroke-dasharray="62 16"/>
        </g>
      </svg></body></html>`, { waitUntil: 'load' })
    const locator = fixturePage.locator('#reference')
    const bytes = await locator.screenshot({ type: 'png', animations: 'disabled' })
    assert(bytes.length > 4_096, 'single-image reference fixture must contain a real rendered PNG')
    return { bytes, width: 960, height: 720 }
  } finally {
    await fixturePage.close()
  }
}
function responseHeaders(response) {
  return new Map((response.headers ?? []).map(([name, value]) => [String(name).toLowerCase(), String(value)]))
}
async function readProductionGlb(driver, assetId) {
  const { bytes, headers } = await driver.getBinary(`/api/v1/agent/asset-versions/${assetId}:model.glb`)
  const sha256 = sha256Bytes(bytes)
  const headerSha256 = headers.get('x-forgecad-glb-sha256')
  const byteSize = Number(headers.get('x-forgecad-glb-byte-size'))
  const triangleCount = Number(headers.get('x-forgecad-triangle-count'))
  assert(headers.get('x-forgecad-artifact-profile') === 'production_concept', 'R007B C106 workbench must consume production_concept GLB bytes')
  assert(headerSha256 === sha256, 'production GLB response hash must match its exact bytes')
  assert(byteSize === bytes.length && byteSize > 0, 'production GLB response byte size must match its exact bytes')
  assert(Number.isInteger(triangleCount) && triangleCount > 0, 'production GLB must expose a positive triangle readback')
  const readback = inspectGlb(bytes)
  assert(readback.triangle_count === triangleCount, 'production GLB JSON readback must match its Rust-owned triangle header')
  assert(readback.mesh_count >= 1 && readback.primitive_count >= 1 && readback.pbr_material_count >= 1, 'production GLB must contain meshes, primitives and PBR materials')
  return {
    sha256,
    byte_size: byteSize,
    artifact_profile_id: 'production_concept',
    artifact_profile_sha256: headers.get('x-forgecad-artifact-profile-sha256') ?? null,
    shape_program_sha256: headers.get('x-forgecad-shape-program-sha256') ?? null,
    readback,
  }
}
function inspectGlb(bytes) {
  assert(bytes.length >= 20 && bytes.readUInt32LE(0) === 0x46546c67, 'production artifact must be a GLB container')
  assert(bytes.readUInt32LE(4) === 2 && bytes.readUInt32LE(8) === bytes.length, 'production GLB header must be version 2 and length-exact')
  let offset = 12
  let document = null
  while (offset + 8 <= bytes.length) {
    const chunkLength = bytes.readUInt32LE(offset)
    const chunkType = bytes.readUInt32LE(offset + 4)
    const end = offset + 8 + chunkLength
    assert(end <= bytes.length, 'production GLB chunk must remain inside the container')
    if (chunkType === 0x4e4f534a) document = JSON.parse(bytes.subarray(offset + 8, end).toString('utf8').replace(/[\u0000\s]+$/u, ''))
    offset = end
  }
  assert(document && Array.isArray(document.meshes) && Array.isArray(document.accessors), 'production GLB must contain readable mesh/accessor JSON')
  let triangleCount = 0
  let primitiveCount = 0
  for (const mesh of document.meshes) {
    for (const primitive of mesh.primitives ?? []) {
      primitiveCount += 1
      assert((primitive.mode ?? 4) === 4, 'R007B production readback only accepts triangle primitives')
      const accessorIndex = Number.isInteger(primitive.indices) ? primitive.indices : primitive.attributes?.POSITION
      const count = document.accessors?.[accessorIndex]?.count
      assert(Number.isInteger(count) && count >= 3 && count % 3 === 0, 'production primitive must expose triangle-aligned accessor counts')
      triangleCount += count / 3
    }
  }
  const materials = Array.isArray(document.materials) ? document.materials : []
  return {
    triangle_count: triangleCount,
    mesh_count: document.meshes.length,
    primitive_count: primitiveCount,
    material_count: materials.length,
    pbr_material_count: materials.filter(material => material?.pbrMetallicRoughness && typeof material.pbrMetallicRoughness === 'object').length,
  }
}
async function waitForProductionViewport(page) {
  try {
    await page.waitForFunction(() => {
      const viewport = document.querySelector('.weapon-viewport')
      return viewport instanceof HTMLElement
        && viewport.dataset.blockoutLoadState === 'ready'
        && viewport.dataset.blockoutGlbKind === 'compiled_agent_production_pbr'
        && viewport.dataset.blockoutRenderSource === 'glb_pbr'
        && Number(viewport.dataset.blockoutEmbeddedPbrMaterialCount ?? 0) >= 1
        && Number(viewport.dataset.activeWebglContexts ?? 0) === 1
        && document.querySelectorAll('.weapon-viewport canvas').length === 1
    }, null, { timeout: TIMEOUT })
  } catch (error) {
    const state = await page.locator('.weapon-viewport').evaluate(element => ({ ...element.dataset, canvasCount: element.querySelectorAll('canvas').length })).catch(() => null)
    throw new Error(`production viewport did not become ready: ${JSON.stringify(state)}; ${error instanceof Error ? error.message : String(error)}`)
  }
}
async function readRendererIdentity(page) {
  const identityValue = await page.locator('.weapon-viewport').evaluate(element => ({
    renderer_generation: Number(element.getAttribute('data-renderer-generation')),
    active_webgl_contexts: Number(element.getAttribute('data-active-webgl-contexts')),
    canvas_count: element.querySelectorAll('canvas').length,
    artifact_kind: element.getAttribute('data-blockout-glb-kind'),
    render_source: element.getAttribute('data-blockout-render-source'),
    load_state: element.getAttribute('data-blockout-load-state'),
    embedded_pbr_material_count: Number(element.getAttribute('data-blockout-embedded-pbr-material-count')),
  }))
  assert(Number.isInteger(identityValue.renderer_generation) && identityValue.renderer_generation >= 1, 'ModuleGraph renderer generation must be a positive stable identity')
  assert(identityValue.active_webgl_contexts === 1 && identityValue.canvas_count === 1, 'R007B workbench must own exactly one WebGL context and canvas')
  assert(identityValue.artifact_kind === 'compiled_agent_production_pbr' && identityValue.render_source === 'glb_pbr' && identityValue.load_state === 'ready', 'R007B workbench must visibly render the production GLB')
  assert(identityValue.embedded_pbr_material_count >= 1, 'R007B production viewport must read embedded PBR material data')
  return identityValue
}
function assertRendererStable(before, after) {
  assert(before.renderer_generation === after.renderer_generation, 'R007B reference and result must retain one ModuleGraph renderer generation')
  assert(before.active_webgl_contexts === 1 && after.active_webgl_contexts === 1 && before.canvas_count === 1 && after.canvas_count === 1, 'R007B renderer stability forbids a second WebGL context')
}
async function captureWorkbenchPairSurface(page, filename, phase) {
  assert(ARTIFACT_DIR, 'paired workbench capture requires an explicit artifact directory')
  await waitForProductionViewport(page)
  await page.getByLabel('只读参考图片').waitFor({ timeout: TIMEOUT })
  const renderer = await readRendererIdentity(page)
  const target = join(ARTIFACT_DIR, filename)
  await page.getByTestId('cad-workbench').screenshot({ path: target, type: 'png', animations: 'disabled' })
  const bytes = await readFile(target)
  assert(bytes.length > 64, `R007B ${phase} workbench screenshot must be non-empty`)
  return {
    phase,
    filename,
    sha256: sha256Bytes(bytes),
    byte_size: bytes.length,
    renderer_generation: renderer.renderer_generation,
    active_webgl_contexts: renderer.active_webgl_contexts,
  }
}
function assertPairedCaptures(referenceCapture, resultCapture) {
  assert(referenceCapture?.phase === 'reference' && resultCapture?.phase === 'result', 'R007B evidence requires one reference-state and one result-state capture')
  assert(referenceCapture.filename !== resultCapture.filename, 'R007B paired screenshots must have distinct safe filenames')
  assert(referenceCapture.sha256 !== resultCapture.sha256, 'R007B paired screenshots must prove two distinct workbench states')
  assert(referenceCapture.renderer_generation === resultCapture.renderer_generation, 'R007B paired screenshots must retain one renderer generation')
  assert(referenceCapture.active_webgl_contexts === 1 && resultCapture.active_webgl_contexts === 1, 'R007B paired screenshots must retain one active WebGL context')
}
function productionEvidence(production) {
  return {
    glb_sha256: production.sha256,
    byte_size: production.byte_size,
    artifact_profile_id: production.artifact_profile_id,
    artifact_profile_sha256: production.artifact_profile_sha256,
    shape_program_sha256: production.shape_program_sha256,
    readback: production.readback,
  }
}
function c106RecipeEvidence(asset) {
  const instances = asset?.assembly_graph?.component_recipe_instances ?? []
  const roots = instances.filter(instance => instance?.parent_instance_id == null)
  return {
    domain_pack_id: asset?.domain_pack_id ?? null,
    component_recipe_instance_count: instances.length,
    root_recipe_ids: roots.map(instance => instance?.recipe?.recipe_id).filter(Boolean).sort(),
    recipe_hashes: instances.map(instance => instance?.recipe?.recipe_sha256).filter(Boolean).sort(),
  }
}
function assertC106RecipeAsset(asset) {
  const evidence = c106RecipeEvidence(asset)
  assert(evidence.domain_pack_id === 'pack_robotic_arm_concept', 'R007B asset must retain the robotic-arm domain pack')
  assert(evidence.component_recipe_instance_count === 10, 'R007B asset must retain exactly ten reviewed C106 Recipe instances')
  assert(evidence.root_recipe_ids.length === 1 && evidence.root_recipe_ids[0].startsWith('recipe_c106_arm_'), 'R007B asset must retain one reviewed C106 arm root Recipe')
  assert(evidence.recipe_hashes.length === 10 && evidence.recipe_hashes.every(value => /^[a-f0-9]{64}$/.test(value)), 'R007B asset must retain immutable hashes for all C106 Recipe instances')
}
function evidenceRecord(identityValue, outcome) {
  const audit = identityValue._evidence_bundle
  const analysis = audit.analysis
  const plan = audit.plan
  const sealedOperations = audit.sealedOperations
  const adornmentOperations = sealedOperations.filter(operation => operation?.op === 'apply_surface_adornment')
  return {
    outcome,
    source_hash: identityValue.source_object_sha256,
    analysis_hash: sha256Canonical(analysis),
    plan_hash: sha256Canonical(plan),
    change_set_hash: sha256Canonical({ change_set_id: audit.sealedChangeSetId, operations: sealedOperations }),
    result_glb_hash: identityValue.result_glb_sha256,
    fidelity_ceiling: analysis.fidelity_ceiling,
    retained: [...analysis.retained_observation_kinds],
    intentionally_changed: [...analysis.intentionally_changed],
    unresolved: [...analysis.unresolved],
    sealed_operations: redactedOperations(sealedOperations),
    surface_adornment_readback: outcome === 'confirmed'
      ? adornmentOperations.map(operation => ({
        program_id: operation.surface_adornment_program.program_id,
        program_sha256: sha256Canonical(operation.surface_adornment_program),
        target_part_id: operation.part_id,
        target_zone_id: operation.material_zone_id,
      }))
      : [],
  }
}
async function writeEvidenceBundle({ smokeSourceSha256, fixtureSourceSha256, projectId, baseId, baseAsset, baseProduction, resultProduction, rendererBaseline, rendererFinal, rejected, confirmed, proxyState }) {
  if (!ARTIFACT_DIR) return { retained: false, artifact_dir_configured: false, bundle_file: null, screenshot_path: null, reference_screenshot_path: null, result_screenshot_path: null }
  assert(rejected.source_object_sha256 === fixtureSourceSha256 && confirmed.source_object_sha256 === fixtureSourceSha256, 'R007B evidence source hash must be the exact uploaded fixture bytes')
  const referenceCapture = confirmed._evidence_bundle.referenceCapture
  const resultCapture = confirmed._evidence_bundle.resultCapture
  assertPairedCaptures(referenceCapture, resultCapture)
  const bundle = {
    schema_version: 'R007BEvidenceBundle@1',
    status: 'pass',
    task: 'FGC-R007B',
    // This is a test-run nonce only, not a project/asset identifier.
    run_id: safeRunId(RUN_ID),
    smoke_source_sha256: smokeSourceSha256,
    fixture_source_sha256: fixtureSourceSha256,
    lineage_bindings: {
      project_identity_sha256: sha256Identity('project', projectId),
      base_asset_identity_sha256: sha256Identity('asset', baseId),
      evidence_identity_sha256: sha256Identity('evidence', confirmed.evidence_id),
      plan_identity_sha256: sha256Identity('plan', confirmed.rebuild_plan_id),
      analysis_identity_sha256: sha256Identity('analysis', confirmed.analysis_id),
      change_set_identity_sha256: sha256Identity('change_set', confirmed.preview_change_set_id),
      result_asset_identity_sha256: sha256Identity('asset', confirmed.result_asset_version_id),
    },
    flows: [evidenceRecord(rejected, 'rejected'), evidenceRecord(confirmed, 'confirmed')],
    c106_recipe_readback: {
      base: c106RecipeEvidence(baseAsset),
      result: c106RecipeEvidence(confirmed._evidence_bundle.resultAsset),
    },
    production_glb: {
      base: productionEvidence(baseProduction),
      result: productionEvidence(resultProduction),
    },
    single_renderer: {
      canvas_count: 1,
      stable_reference_result_swap: true,
      renderer_generation: rendererFinal.renderer_generation,
      active_webgl_contexts: rendererFinal.active_webgl_contexts,
      render_source: rendererFinal.render_source,
      artifact_kind: rendererFinal.artifact_kind,
      base_and_result_generation_equal: rendererBaseline.renderer_generation === rendererFinal.renderer_generation,
    },
    paired_screenshots: {
      reference: referenceCapture,
      result: resultCapture,
    },
    rust_only_counts: {
      rust_product_request_count: proxyState.rust_requests.length,
      python_product_route_attempts: proxyState.python_product_route_attempts,
    },
    screenshot_path: resultCapture.filename,
    reference_vision_capability: false,
    visual_fidelity_validated: false,
    formal_eligible: false,
  }
  // Do not permit a future refactor to accidentally serialize a source
  // statement, local path, request payload, or raw reference bytes.
  assertRedactedEvidenceBundle(bundle)
  await writeFile(join(ARTIFACT_DIR, ARTIFACT_BUNDLE_FILE), `${JSON.stringify(bundle, null, 2)}\n`, 'utf8')
  return {
    retained: true,
    artifact_dir_configured: true,
    bundle_file: ARTIFACT_BUNDLE_FILE,
    screenshot_path: resultCapture.filename,
    reference_screenshot_path: referenceCapture.filename,
    result_screenshot_path: resultCapture.filename,
  }
}
function assertRedactedEvidenceBundle(bundle) {
  const raw = JSON.stringify(bundle)
  const forbiddenKeys = ['source_statement', 'license_statement', 'license', 'request_body', 'absolute_path', 'project_id', 'asset_version_id', 'evidence_id']
  for (const key of forbiddenKeys) assert(!raw.includes(`"${key}"`), `R007B evidence bundle must redact ${key}`)
  assert(!raw.includes(ROOT), 'R007B evidence bundle must not expose a workspace path')
  assert(!raw.includes('\nAuthorized only for ForgeCAD'), 'R007B evidence bundle must not expose source/license text')
}
function assertCompleteC106Lineage({ plan, analysis, evidence, projectId, baseId }) {
  const roots = new Set(['recipe_c106_arm_desktop_assistant', 'recipe_c106_arm_gallery_industrial', 'recipe_c106_arm_service_display'])
  assert(plan?.base_asset_version_id === baseId, 'R007B plan must bind exactly the active C106 base asset')
  assert(plan?.project_id === projectId && plan?.domain_pack_id === 'pack_robotic_arm_concept', 'R007B plan must retain one project and the robotic-arm C106 domain')
  assert(roots.has(plan?.recipe_id), 'R007B plan must select an exact reviewed C106 root recipe')
  assert(typeof plan?.recipe_registry_sha256 === 'string' && /^[a-f0-9]{64}$/.test(plan.recipe_registry_sha256), 'R007B plan must retain the immutable C106 registry hash')
  assert(analysis?.schema_version === 'ReferenceSurfaceAnalysis@1', 'R007B preview must persist frozen surface analysis rather than a fallback plan')
  assert(analysis?.rebuild_plan_id === plan.rebuild_plan_id && analysis?.evidence_id === evidence.evidence_id, 'frozen analysis must bind exactly its plan and evidence')
  assert(analysis?.source_object_sha256 === evidence.source_object_sha256, 'frozen analysis must bind exact immutable source bytes')
  assert(analysis?.domain_pack_id === plan.domain_pack_id && analysis?.c106_registry_sha256 === plan.recipe_registry_sha256, 'frozen analysis must retain the exact C106 domain and registry')
  assert(analysis?.target_root_recipe?.recipe_id === plan.recipe_id, 'frozen analysis must name the same reviewed C106 root as its plan')
  assert(analysis?.target_root_recipe?.recipe_sha256?.match(/^[a-f0-9]{64}$/), 'frozen analysis must retain the reviewed C106 root hash')
  assert(analysis?.surface_skill_id === 'skill_first_party_surface_adornment' && analysis?.surface_skill_version === 2 && analysis?.surface_skill_sha256?.match(/^[a-f0-9]{64}$/), 'frozen analysis must retain the immutable A005 v2 surface skill lineage')
  assert(Array.isArray(analysis?.bindings) && analysis.bindings.length >= 3, 'frozen analysis must retain reviewed component, material-zone and surface-slot bindings')
  for (const binding of analysis.bindings) {
    assert(binding?.target_recipe?.recipe_id && binding?.target_recipe?.recipe_sha256?.match(/^[a-f0-9]{64}$/), 'each R007B binding must retain a reviewed C106 recipe hash')
    assert(typeof binding?.target_part_role === 'string' && typeof binding?.target_material_zone_id === 'string' && typeof binding?.target_surface_slot_id === 'string', 'each R007B binding must retain part, material-zone and A005 surface-slot lineage')
  }
}
function stableSnapshot(value) { return JSON.stringify({ revision: value?.revision ?? null, active: value?.active_design?.asset_version_id ?? null, preview: value?.preview ?? null }) }
async function assertDrawerLineage(drawer, values) { const lineage = drawer.getByLabel('参考重建证据谱系'); await lineage.waitFor({ timeout: TIMEOUT }); const rendered = await lineage.innerText(); for (const value of values) assert(rendered.includes(value), `drawer lineage must render exact API identity: ${value}`) }
async function webglViewportIsSingleAndStable(page, token) { return page.evaluate(value => { const viewport = document.querySelector('.weapon-viewport'); const canvases = [...document.querySelectorAll('.weapon-viewport canvas')]; const canvas = canvases[0]; return viewport?.getAttribute('data-r007b-viewport-token') === value && canvases.length === 1 && Boolean(canvas?.getContext('webgl2') || canvas?.getContext('webgl')) }, token) }
async function forwardToPython(input, base) { const headers = Object.fromEntries((input.headers ?? []).filter(([name]) => !['host', 'content-length'].includes(String(name).toLowerCase()))); const body = input.body?.encoding === 'base64' ? Buffer.from(input.body.data, 'base64') : input.body?.encoding === 'utf8' ? input.body.data : undefined; const response = await fetch(`${base}${input.path}`, { method: input.method, headers, body }); const bytes = Buffer.from(await response.arrayBuffer()); const contentType = response.headers.get('content-type') ?? ''; return http(response.status, [...response.headers.entries()], contentType.includes('json') || contentType.startsWith('text/') ? { encoding: 'utf8', data: bytes.toString('utf8') } : { encoding: 'base64', data: bytes.toString('base64') }) }
function http(status, headers, body) { return { schema_version: 'ForgeCADHttpCompatibilityResponse@1', status, headers, body } }
function initialize() { return { schema_version: 'ForgeCADInitializeResult@1', protocol_version: 'forgecad.app-server/1', connection_id: 'conn_r007b_workbench', server_info: { name: 'rust-r007b-driver-proxy', version: '1' }, capabilities: { notifications: true, cursor_replay: true, cancellation: true, notification_ack: true, binary_body_base64: true }, limits: { max_in_flight_requests: 32, max_event_queue: 64, max_frame_bytes: 64 * 1024 * 1024 }, migration_state: { state_owner: 'rust_app_server' } } }
function success(id, result) { return { jsonrpc: '2.0', id, result } }
function failure(id, applicationCode, message) { return { jsonrpc: '2.0', id, error: { code: -32000, message, data: { schema_version: 'ForgeCADProtocolError@1', application_code: applicationCode } } } }
function corsHeaders() { return { 'access-control-allow-origin': '*', 'access-control-allow-headers': 'content-type,idempotency-key,if-match,accept,cache-control', 'access-control-allow-methods': 'GET,POST,DELETE,OPTIONS' } }
function json(response, status, value) { response.writeHead(status, { ...corsHeaders(), 'content-type': 'application/json' }); response.end(status === 204 ? '' : JSON.stringify(value)) }
async function requestJson(request) { const chunks = []; for await (const chunk of request) chunks.push(chunk); const raw = Buffer.concat(chunks).toString('utf8'); return raw ? JSON.parse(raw) : {} }
function assert(value, message) { if (!value) throw new Error(message) }
function safeRunId(value) { return String(value).replace(/[^a-zA-Z0-9_.-]/g, '_').slice(0, 80) || 'run' }
async function createCompatibilityProject(base) {
  const projectRequestId = `r007b-browser-project-shell-${safeRunId(RUN_ID)}`
  const response = await fetch(`${base}/api/v1/projects`, {
    method: 'POST', headers: { 'Content-Type': 'application/json', 'Idempotency-Key': projectRequestId },
    body: JSON.stringify({
      client_request_id: projectRequestId, profile_id: 'profile_weapon_concept_v1', name: `R007B browser compatibility shell ${safeRunId(RUN_ID)}`,
      intended_uses: ['game_asset', 'film_prop', 'non_functional_display'],
      style: { keywords: ['industrial', 'robotic arm', 'visual only'], palette: ['graphite', 'blue'], detail_density: 0.68 },
      proportions: { overall_length_mm: 230, body_height_mm: 54, grip_angle_deg: 15 },
      required_slots: ['core', 'front', 'rear', 'grip'], optional_slots: ['top', 'left', 'right', 'bottom', 'side_panels'],
      constraints: { symmetry: 'mostly_symmetric', max_triangle_count: 180000 }, assumptions: ['非功能性概念模型，不用于真实制造或使用'],
    }),
  })
  if (!response.ok) throw new Error(`Python compatibility project shell: ${response.status} ${await response.text()}`)
  const project = await response.json()
  assert(typeof project?.project_id === 'string', 'Python compatibility shell must return a project ID')
  return project.project_id
}
async function waitForHttp(url, child, label) { for (let index = 0; index < 120; index += 1) { if (child.exitCode !== null) throw new Error(`${label} exited before ready`); try { if ((await fetch(url)).ok) return } catch {} await sleep(100) } throw new Error(`${label} did not become ready`) }
async function freePort() { const net = await import('node:net'); return new Promise((resolvePort, reject) => { const server = net.createServer(); server.once('error', reject); server.listen(0, '127.0.0.1', () => { const { port } = server.address(); server.close(() => resolvePort(port)) }) }) }
function listen(server, port) { return new Promise(resolveListen => server.listen(port, '127.0.0.1', resolveListen)) }
function drain(child) { child.stdout?.resume(); child.stderr?.resume() }
function stop(child) { return new Promise(resolveStop => { if (!child || child.exitCode !== null) return resolveStop(); const timer = setTimeout(() => { child.kill('SIGKILL'); resolveStop() }, 5_000); child.once('exit', () => { clearTimeout(timer); resolveStop() }); child.kill('SIGTERM') }) }
async function launchBrowser() { return chromium.launch({ channel: process.env.WUSHEN_BROWSER_CHANNEL ?? 'chrome', headless: true }) }
function sleep(ms) { return new Promise(resolveSleep => setTimeout(resolveSleep, ms)) }

await main()
