#!/usr/bin/env node

// FGC-F001: small, readable characterization checks for the current Agent-first
// workbench. This intentionally observes the existing UI/API behavior; it does
// not replace the broader r3 workflow or introduce a second state implementation.
import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'
import { legacyLifecycleTestOracleEnvironment } from './workbench_agent_blockout_test_helper.mjs'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))

async function main() {
  const tempRoot = await mkdtemp(join(tmpdir(), 'forgecad-f001-workbench-'))
  const libraryRoot = join(tempRoot, 'library')
  const agentPort = await freePort()
  const vitePort = await freePort()
  const agentBaseUrl = `http://127.0.0.1:${agentPort}`
  const viteBaseUrl = `http://127.0.0.1:${vitePort}`
  const processes = []
  let browser = null
  let context = null
  try {
    const agent = spawn(
      join(ROOT, '.venv', 'bin', 'python'),
      ['-m', 'uvicorn', 'wushen_agent.test_oracle:create_app', '--factory', '--host', '127.0.0.1', '--port', String(agentPort)],
      {
        cwd: ROOT,
        env: legacyLifecycleTestOracleEnvironment(process.env, {
          WUSHEN_LIBRARY_ROOT: libraryRoot,
          WUSHEN_MIGRATIONS_DIR: join(ROOT, 'migrations'),
          WUSHEN_CORS_ORIGINS: viteBaseUrl,
          WUSHEN_LOCAL_WORKER_ENABLED: '0',
          FORGECAD_CONCEPT_WORKER_ENABLED: '1',
          FORGECAD_CONCEPT_PLANNER_PROVIDER: 'deterministic_rules',
        }),
        stdio: ['ignore', 'pipe', 'pipe'],
      },
    )
    processes.push(agent)
    await waitForHttp(`${agentBaseUrl}/api/health`, agent, 'Agent')

    const vite = spawn(
      process.execPath,
      [join(ROOT, 'node_modules', 'vite', 'bin', 'vite.js'), '--host', '127.0.0.1', '--port', String(vitePort)],
      { cwd: join(ROOT, 'apps', 'desktop'), env: { ...process.env, VITE_FORGE_API_BASE_URL: agentBaseUrl }, stdio: ['ignore', 'pipe', 'pipe'] },
    )
    processes.push(vite)
    await waitForHttp(viteBaseUrl, vite, 'Vite')

    browser = await launchBrowser()
    context = await browser.newContext({ viewport: { width: 1440, height: 960 } })
    const page = await context.newPage()
    const browserErrors = []
    let legacyBriefPosts = 0
    let legacyWorkbenchInitializations = 0
    const legacyDetailReads = []
    const legacyMutationRequests = []
    let legacyDetailReadCountAfterExplicitClose = 0
    page.on('pageerror', (error) => browserErrors.push(error.message))
    page.on('request', (request) => {
      const url = request.url()
      if (request.method() === 'POST' && url.includes('/brief:interpret')) legacyBriefPosts += 1
      if (request.method() === 'POST' && url.includes(':initialize-workbench')) legacyWorkbenchInitializations += 1
      if (request.method() === 'GET' && (
        url.includes('/api/v1/module-graphs/')
        || /\/api\/v1\/versions\/[^/]+$/.test(url)
        || /\/api\/v1\/projects\/[^/]+\/(variants|change-sets|change-set-audit-exports)/.test(url)
      )) legacyDetailReads.push(url)
      if (request.method() !== 'GET' && (
        url.includes('/brief:interpret')
        || url.includes('/change-sets')
        || url.includes('/quality-runs:inspect')
        || /\/api\/v1\/versions\/[^/]+\/exports$/.test(url)
      )) legacyMutationRequests.push(`${request.method()} ${url}`)
    })
    if (process.env.FGC_F001_DEBUG === '1') {
      page.on('response', async (response) => {
        if (response.url().includes('/api/v1/agent/threads/') && response.url().endsWith('/turns')) {
          const payload = await response.text().catch(() => '')
          console.error(`[F001 turn response ${response.status()}] ${payload.slice(0, 4000)}`)
        }
      })
    }

    await page.goto(`${viteBaseUrl}/#/cad`, { waitUntil: 'networkidle' })
    await page.waitForSelector('[data-testid="cad-workbench"]', { timeout: 20_000 })
    await page.getByLabel('设计需求', { exact: true }).waitFor({ timeout: 20_000 })
    await waitForUiProject(page)
    if (await page.locator('.weapon-viewport canvas').count() !== 1) {
      throw new Error('characterization requires exactly one WebGL canvas')
    }
    await assertText(page.locator('.f026-agent-timeline'), ['设计助手', '汽车', '飞机', '机械臂'])

    const projectId = await waitForProjectId(agentBaseUrl)
    const initialProject = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}`)
    const initialActiveDesign = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
    if (!isLegacyOrMissingSnapshot(initialActiveDesign)) {
      throw new Error(`new characterization project has an unexpected Snapshot state: ${initialActiveDesign.status} ${JSON.stringify(initialActiveDesign.body)}`)
    }
    if (initialActiveDesign.status === 404) {
      const emptyProject = page.getByTestId('agent-empty-project')
      try {
        await emptyProject.waitFor({ timeout: 20_000 })
      } catch (error) {
        const timeline = await page.locator('.f026-agent-timeline').innerText().catch(() => '')
        throw new Error(
          `empty project state did not render (current_version_id=${initialProject.current_version_id ?? 'null'}): `
          + `${error instanceof Error ? error.message : String(error)}\n${timeline.slice(0, 2000)}`,
        )
      }
      await assertText(emptyProject, ['空项目已就绪', '生成第一个 3D 资产'])
      if (await page.getByRole('button', { name: '准备展示组件', exact: true }).count() !== 0) {
        throw new Error('empty Project still exposed the legacy workbench initializer')
      }
    }
    if (legacyWorkbenchInitializations !== 0) {
      throw new Error(`empty Project called the legacy workbench initializer ${legacyWorkbenchInitializations} time(s)`)
    }

    // The current starter project can legitimately open as a legacy read-only
    // design. Request the explicit rebuild hand-off before exercising the Agent
    // path; this must not mutate the legacy version or silently bypass the
    // ACTIVE_DESIGN_INVALID write barrier.
    let legacyConversionRequested = false
    if (initialActiveDesign.body?.active_design?.source === 'legacy_concept_read_only') {
      const legacyNotice = page.getByLabel('旧版设计转换')
      await legacyNotice.waitFor({ timeout: 20_000 })
      if (legacyDetailReads.length !== 0) {
        throw new Error(`legacy details were read before explicit entry: ${legacyDetailReads.join(' | ')}`)
      }
      if (await page.getByLabel('旧版只读 Graph Inspector').count() !== 0) {
        throw new Error('legacy Graph Inspector was visible before explicit entry')
      }
      let delayedLegacyVersionRead = false
      const delayLegacyVersion = async (route) => {
        if (!delayedLegacyVersionRead && route.request().method() === 'GET') {
          delayedLegacyVersionRead = true
          await sleep(500)
        }
        await route.continue()
      }
      await page.route('**/api/v1/versions/*', delayLegacyVersion)
      await legacyNotice.getByRole('button', { name: '查看旧版只读信息', exact: true }).click()
      const interruptedInspector = page.getByLabel('旧版只读 Graph Inspector')
      await interruptedInspector.waitFor({ timeout: 20_000 })
      await interruptedInspector.getByRole('button', { name: '关闭', exact: true }).click()
      await sleep(700)
      await page.unroute('**/api/v1/versions/*', delayLegacyVersion)
      if (!delayedLegacyVersionRead || await page.getByLabel('旧版只读 Graph Inspector').count() !== 0) {
        throw new Error('late legacy detail response reopened the closed compatibility surface')
      }
      const legacyGraphResponse = page.waitForResponse((response) => response.url().includes('/api/v1/module-graphs/') && response.request().method() === 'GET')
      await legacyNotice.getByRole('button', { name: '查看旧版只读信息', exact: true }).click()
      const readonlyInspector = page.getByLabel('旧版只读 Graph Inspector')
      await readonlyInspector.waitFor({ timeout: 20_000 })
      await legacyGraphResponse
      if (legacyDetailReads.length === 0 || !legacyDetailReads.some((url) => url.includes('/api/v1/module-graphs/'))) {
        throw new Error(`explicit legacy entry did not load the compatibility graph: ${legacyDetailReads.join(' | ')}`)
      }
      if ((await readonlyInspector.locator('input:not([readonly])').count()) !== 0) {
        throw new Error('legacy read-only inspector exposed an editable input')
      }
      await assertText(readonlyInspector, ['Graph Inspector · 只读', '旧参数 · 只读', 'SOURCE ZIP · OBJ · PNG · MP4'])
      await readonlyInspector.getByRole('button', { name: '关闭', exact: true }).click()
      if (await page.getByLabel('旧版只读 Graph Inspector').count() !== 0) {
        throw new Error('legacy Graph Inspector did not close explicitly')
      }
      legacyDetailReadCountAfterExplicitClose = legacyDetailReads.length
      const rebuildButton = legacyNotice.getByRole('button', { name: '让 Agent 重建可编辑资产', exact: true })
      if (await rebuildButton.count() !== 1) throw new Error('legacy project did not expose explicit Agent rebuild action')
      await rebuildButton.click()
      await page.getByText(/已准备 legacy 只读设计的 Agent 重建输入/).waitFor({ timeout: 20_000 })
      const afterConversion = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
      if (afterConversion.body?.active_design?.source !== 'legacy_concept_read_only') {
        throw new Error('legacy conversion hand-off changed the active design before Agent commit')
      }
      legacyConversionRequested = true
    }

    // Ambiguous input is a write barrier: one question, no legacy fallback,
    // and no Plan/Asset/Snapshot before the user chooses a domain.
    const input = page.getByLabel('设计需求', { exact: true })
    await input.fill('设计一台能飞的无人机载具')
    await page.getByRole('button', { name: '发送设计需求', exact: true }).click()
    const clarification = page.getByLabel('需要确认设计类别')
    try {
      await clarification.waitFor({ timeout: 20_000 })
    } catch (error) {
      const bodyText = await page.locator('body').innerText().catch(() => '')
      throw new Error(`clarification did not appear: ${error instanceof Error ? error.message : String(error)}\n${bodyText.slice(0, 2000)}`)
    }
    const clarificationBodyText = await page.locator('body').innerText()
    for (const phrase of ['先确认设计对象', '汽车与地面载具', '飞机与航空器']) {
      if (!clarificationBodyText.includes(phrase)) throw new Error(`clarification UI missing ${phrase}: ${clarificationBodyText.slice(0, 2000)}`)
    }
    if (legacyBriefPosts !== 0) throw new Error('ambiguous input fell back to legacy Brief interpretation')
    const afterClarificationSnapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
    if (afterClarificationSnapshot.body?.active_design?.source === 'agent_asset') {
      throw new Error('ambiguous input changed the legacy Snapshot before clarification')
    }

    const aircraftChoice = page.getByRole('button', { name: '飞机与航空器', exact: true })
    const aircraftChoiceCount = await aircraftChoice.count()
    if (aircraftChoiceCount !== 1) {
      const bodyText = await page.locator('body').innerText()
      throw new Error(`clarification did not expose exactly one aircraft choice (count=${aircraftChoiceCount}): ${bodyText.slice(0, 1500)}`)
    }
    await clickWithRetry(aircraftChoice, 'aircraft clarification choice')
    const failure = page.locator('[data-generation-state="failed"][aria-label="生成失败"]')
    await failure.waitFor({ timeout: 60_000 })
    const failureText = await failure.innerText()
    if (!failureText.includes('Agent 没有返回正式的单一结果决策')) {
      throw new Error(`compatibility Planner was not rejected by the V003 decision contract: ${failureText}`)
    }
    if (await page.getByLabel('当前临时结果').count() !== 0) {
      throw new Error('F001 rendered a legacy Planner payload as a formal V003 result')
    }
    if (await page.getByLabel('Agent 完整外观方向').count() !== 0) {
      throw new Error('F001 restored the retired direction-selection surface')
    }
    if (await page.locator('.weapon-viewport canvas').count() !== 1) {
      throw new Error('workbench created a second WebGL canvas after V003 rejection')
    }
    const afterRejectionSnapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
    if (stableSnapshot(afterClarificationSnapshot) !== stableSnapshot(afterRejectionSnapshot)) {
      throw new Error(
        `V003 compatibility rejection changed ActiveDesignSnapshot: before=${stableSnapshot(afterClarificationSnapshot)} after=${stableSnapshot(afterRejectionSnapshot)}`,
      )
    }
    if (legacyDetailReads.length !== legacyDetailReadCountAfterExplicitClose) {
      throw new Error(`Agent request issued implicit legacy detail reads: ${legacyDetailReads.slice(legacyDetailReadCountAfterExplicitClose).join(' | ')}`)
    }
    if (legacyBriefPosts !== 0 || legacyWorkbenchInitializations !== 0 || legacyMutationRequests.length > 0 || browserErrors.length > 0) {
      throw new Error(`browser characterization errors: legacyBriefPosts=${legacyBriefPosts}, legacyWorkbenchInitializations=${legacyWorkbenchInitializations}, legacyMutationRequests=${legacyMutationRequests.join(' | ')}, errors=${browserErrors.join(' | ')}`)
    }
    console.log(JSON.stringify({
      ok: true,
      project_id: projectId,
      assertions: [
        'single_canvas',
        'ambiguous_clarification_write_barrier',
        'v003_rejects_legacy_planner_without_snapshot_write',
        'legacy_details_require_explicit_entry',
        'legacy_surface_is_read_only',
        'agent_flow_makes_no_legacy_mutation_calls',
        'empty_project_requires_no_legacy_initializer',
        ...(legacyConversionRequested ? ['legacy_rebuild_requires_explicit_handoff'] : []),
    ],
    }, null, 2))
  } finally {
    const cleanupFailures = []
    for (const cleanup of [
      context && (() => withinTimeout(context.close(), 5_000, 'browser context close')),
      browser && (async () => {
        try {
          await withinTimeout(browser.close(), 5_000, 'browser close')
        } catch (error) {
          if (browser.isConnected()) throw error
        }
      }),
    ].filter(Boolean)) {
      try { await cleanup() } catch (error) { cleanupFailures.push(error) }
    }
    const processCleanup = await Promise.allSettled(processes.reverse().map(stopProcess))
    cleanupFailures.push(...processCleanup.filter((entry) => entry.status === 'rejected').map((entry) => entry.reason))
    await rm(tempRoot, { recursive: true, force: true })
    if (cleanupFailures.length > 0) {
      throw new Error(`F001 runtime cleanup failed: ${cleanupFailures.map((failure) => String(failure)).join('; ').slice(0, 2_000)}`)
    }
  }
}

function isLegacyOrMissingSnapshot(response) {
  return response.status === 404 || response.body?.active_design?.source === 'legacy_concept_read_only'
}

function stableSnapshot(response) {
  return JSON.stringify({
    status: response.status,
    revision: response.body?.revision ?? null,
    asset_version_id: response.body?.active_design?.asset_version_id ?? null,
    preview: response.body?.preview ?? null,
    error_code: response.body?.error?.code ?? null,
  })
}

async function waitForProjectId(baseUrl) {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    const response = await requestStatus(baseUrl, '/api/v1/projects')
    const projectId = response.body?.items?.[0]?.project_id
    if (response.status === 200 && projectId) {
      const project = await requestStatus(baseUrl, `/api/v1/projects/${projectId}`)
      if (project.status === 200 && project.body?.project_id) return projectId
    }
    await sleep(200)
  }
  throw new Error('workbench did not create a starter project')
}

async function waitForUiProject(page) {
  const title = page.locator('[aria-label="当前项目"] strong')
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    const text = await title.textContent().catch(() => null)
    if (text && text.trim() && text.trim() !== '新概念设计') return text.trim()
    await sleep(200)
  }
  throw new Error('workbench did not finish loading the current project in the UI')
}

async function requestStatus(baseUrl, path) {
  const response = await fetch(`${baseUrl}${path}`, { signal: AbortSignal.timeout(5_000) })
  let body = null
  try { body = await response.json() } catch { /* empty error body */ }
  return { status: response.status, body }
}

async function jsonRequest(baseUrl, path) {
  const response = await fetch(`${baseUrl}${path}`)
  const body = await response.json()
  if (!response.ok) throw new Error(`${response.status} ${path}: ${JSON.stringify(body)}`)
  return body
}

async function assertText(locator, expected) {
  const text = await locator.innerText()
  for (const phrase of expected) if (!text.includes(phrase)) throw new Error(`missing text ${phrase}: ${text}`)
}

async function launchBrowser() {
  const executablePath = process.env.WUSHEN_BROWSER_EXECUTABLE
  if (executablePath) return chromium.launch({ executablePath, headless: true })
  if (process.platform === 'darwin') {
    const macChrome = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
    if (existsSync(macChrome)) return chromium.launch({ executablePath: macChrome, headless: true })
  }
  return chromium.launch({ channel: process.env.WUSHEN_BROWSER_CHANNEL || 'chrome', headless: true })
}

async function waitForHttp(url, child, label) {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    if (child.exitCode !== null) throw new Error(`${label} exited with ${child.exitCode}`)
    try { if ((await fetch(url)).ok) return } catch { /* still starting */ }
    await sleep(200)
  }
  throw new Error(`${label} did not become ready: ${url}`)
}

async function freePort() {
  const net = await import('node:net')
  return new Promise((resolvePort, reject) => {
    const server = net.createServer()
    server.listen(0, '127.0.0.1', () => {
      const address = server.address()
      server.close(() => resolvePort(address.port))
    })
    server.on('error', reject)
  })
}

function sleep(milliseconds) { return new Promise((resolveSleep) => setTimeout(resolveSleep, milliseconds)) }

function withinTimeout(promise, timeoutMs, label) {
  return Promise.race([
    promise,
    new Promise((_, reject) => setTimeout(() => reject(new Error(`${label} timed out after ${timeoutMs}ms`)), timeoutMs)),
  ])
}

async function clickWithRetry(locator, label) {
  let lastError = null
  for (let attempt = 0; attempt < 4; attempt += 1) {
    try {
      await locator.click({ timeout: 10_000 })
      return
    } catch (error) {
      lastError = error
      await sleep(150)
    }
  }
  throw new Error(`${label} could not be clicked: ${lastError instanceof Error ? lastError.message : String(lastError)}`)
}

function stopProcess(child) {
  return new Promise((resolveStop) => {
    if (!child || child.exitCode !== null) return resolveStop()
    const timer = setTimeout(() => { child.kill('SIGKILL'); resolveStop() }, 5_000)
    child.once('exit', () => { clearTimeout(timer); resolveStop() })
    child.kill('SIGTERM')
  })
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack || error.message : error)
  process.exitCode = 1
})
