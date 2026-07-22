#!/usr/bin/env node
import { spawn } from 'node:child_process'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'
import { legacyLifecycleTestOracleEnvironment } from './workbench_agent_blockout_test_helper.mjs'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const RUNTIME_TIMEOUT_MS = 45_000
const PAGE_TIMEOUT_MS = 60_000

async function main() {
  const tempRoot = await mkdtemp(join(tmpdir(), 'forgecad-d003-ui-'))
  const agentPort = await freePort()
  const vitePort = await freePort()
  const agentBaseUrl = `http://127.0.0.1:${agentPort}`
  const viteBaseUrl = `http://127.0.0.1:${vitePort}`
  const processes = []
  const runtimeLogs = { agent: [], vite: [] }
  let browser = null
  let context = null
  try {
    const agent = spawn(
      join(ROOT, '.venv', 'bin', 'python'),
      ['-m', 'uvicorn', 'wushen_agent.test_oracle:create_app', '--factory', '--host', '127.0.0.1', '--port', String(agentPort)],
      {
        cwd: ROOT,
        env: legacyLifecycleTestOracleEnvironment(process.env, {
          WUSHEN_LIBRARY_ROOT: join(tempRoot, 'library'),
          WUSHEN_MIGRATIONS_DIR: join(ROOT, 'migrations'),
          WUSHEN_CORS_ORIGINS: viteBaseUrl,
          WUSHEN_LOCAL_WORKER_ENABLED: '0',
          FORGECAD_CONCEPT_WORKER_ENABLED: '1',
          FORGECAD_CONCEPT_PLANNER_PROVIDER: 'deterministic_rules',
        }),
        stdio: ['ignore', 'pipe', 'pipe'],
      },
    )
    captureProcessOutput(agent, runtimeLogs.agent)
    processes.push(agent)
    await waitForHttp(`${agentBaseUrl}/api/health`, agent, 'Agent')
    const vite = spawn(
      process.execPath,
      [join(ROOT, 'node_modules', 'vite', 'bin', 'vite.js'), '--host', '127.0.0.1', '--port', String(vitePort)],
      { cwd: join(ROOT, 'apps', 'desktop'), env: { ...process.env, VITE_FORGE_API_BASE_URL: agentBaseUrl }, stdio: ['ignore', 'pipe', 'pipe'] },
    )
    captureProcessOutput(vite, runtimeLogs.vite)
    processes.push(vite)
    await waitForHttp(viteBaseUrl, vite, 'Vite')

    browser = await launchBrowser()
    context = await browser.newContext({ viewport: { width: 1536, height: 1024 } })
    const page = await context.newPage()
    const pageSignals = []
    page.on('console', (message) => {
      if (message.type() === 'error' || message.type() === 'warning') appendDiagnostic(pageSignals, `console.${message.type()}: ${message.text()}`)
    })
    page.on('pageerror', (error) => appendDiagnostic(pageSignals, `pageerror: ${error.message}`))
    page.on('requestfailed', (request) => appendDiagnostic(pageSignals, `requestfailed: ${request.method()} ${request.url()} ${request.failure()?.errorText ?? ''}`))
    let legacyBriefInterpretPosts = 0
    page.on('request', (request) => {
      if (request.method() === 'POST' && request.url().includes('/brief:interpret')) legacyBriefInterpretPosts += 1
    })
    await page.goto(`${viteBaseUrl}/#/cad`, { waitUntil: 'domcontentloaded', timeout: PAGE_TIMEOUT_MS })
    try {
      await page.waitForSelector('[data-testid="cad-workbench"]', { timeout: PAGE_TIMEOUT_MS })
    } catch (error) {
      throw new Error(await workbenchBootDiagnostic(error, page, pageSignals, runtimeLogs))
    }
    const input = page.getByPlaceholder('描述你想设计的 3D 概念模型…')
    await input.waitFor({ timeout: PAGE_TIMEOUT_MS })
    await input.fill('设计一台能飞的无人机载具')
    await page.getByRole('button', { name: '发送设计需求', exact: true }).click()
    const clarification = page.getByLabel('需要确认设计类别')
    await clarification.waitFor({ timeout: PAGE_TIMEOUT_MS })
    const text = await clarification.innerText()
    for (const expected of ['先确认设计对象', '你想先设计汽车、飞机、机械臂，还是未来概念道具？', '汽车与地面载具', '飞机与航空器']) {
      if (!text.includes(expected)) throw new Error(`clarification UI missing: ${expected}\n${text}`)
    }
    if (legacyBriefInterpretPosts !== 0) throw new Error('ambiguous input fell back to legacy Brief API')
    const projectId = await page.getByTestId('cad-workbench').getAttribute('data-qa-project-id')
    if (!projectId) throw new Error('clarification workbench did not expose its starter project')
    const beforeSnapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
    await clarification.getByRole('button', { name: '飞机与航空器', exact: true }).click()
    const failure = page.locator('[data-generation-state="failed"][aria-label="生成失败"]')
    await failure.waitFor({ timeout: PAGE_TIMEOUT_MS })
    const failureText = await failure.innerText()
    if (!failureText.includes('Agent 没有返回正式的单一结果决策')) {
      throw new Error(`clarification continuation did not enforce the V003 decision contract: ${failureText}`)
    }
    if (await page.getByLabel('当前临时结果').count()) {
      throw new Error('clarification completion rendered a legacy Planner payload as a formal V003 result')
    }
    if (await page.getByLabel('Agent 完整外观方向').count()) {
      throw new Error('clarification completion restored the retired direction-selection surface')
    }
    if (await page.getByLabel('需要确认设计类别').count() !== 0) throw new Error('clarification remained after choosing a domain')
    if (await page.locator('.weapon-viewport canvas').count() !== 1) throw new Error('clarification continuation created an extra WebGL canvas')
    const afterSnapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
    if (stableSnapshot(beforeSnapshot) !== stableSnapshot(afterSnapshot)) {
      throw new Error(`V003 rejection changed ActiveDesignSnapshot: before=${stableSnapshot(beforeSnapshot)} after=${stableSnapshot(afterSnapshot)}`)
    }
    if (legacyBriefInterpretPosts !== 0) throw new Error('clarification continuation fell back to legacy Brief API')
    console.log(JSON.stringify({
      ok: true,
      clarification: 'ambiguous',
      continuation: 'v003_decision_contract_rejected_without_snapshot_write',
      legacy_brief_posts: legacyBriefInterpretPosts,
    }))
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
      try {
        await cleanup()
      } catch (error) {
        cleanupFailures.push(error)
      }
    }
    const processCleanup = await Promise.allSettled(processes.reverse().map(stopProcess))
    cleanupFailures.push(...processCleanup.filter((entry) => entry.status === 'rejected').map((entry) => entry.reason))
    await rm(tempRoot, { recursive: true, force: true })
    if (cleanupFailures.length > 0) {
      throw new Error(`D3 runtime cleanup failed: ${cleanupFailures.map((failure) => String(failure)).join('; ').slice(0, 2_000)}`)
    }
  }
}

async function launchBrowser() {
  const executablePath = process.env.WUSHEN_BROWSER_EXECUTABLE
  if (executablePath) return chromium.launch({ executablePath, headless: true })
  return chromium.launch({ channel: process.env.WUSHEN_BROWSER_CHANNEL || 'chrome', headless: true })
}

async function waitForHttp(url, child, label) {
  const deadline = Date.now() + RUNTIME_TIMEOUT_MS
  while (Date.now() < deadline) {
    if (child.exitCode !== null) throw new Error(`${label} exited with ${child.exitCode}`)
    try {
      if ((await fetch(url)).ok) return
    } catch { /* process is still starting */ }
    await new Promise((resolveSleep) => setTimeout(resolveSleep, 200))
  }
  throw new Error(`${label} did not become ready: ${url}`)
}

function captureProcessOutput(child, destination) {
  for (const stream of [child.stdout, child.stderr]) {
    stream?.setEncoding('utf8')
    stream?.on('data', (chunk) => {
      for (const line of String(chunk).split(/\r?\n/).filter(Boolean)) appendDiagnostic(destination, line)
    })
  }
}

function appendDiagnostic(destination, value) {
  destination.push(String(value).slice(0, 2_000))
  if (destination.length > 40) destination.shift()
}

async function workbenchBootDiagnostic(error, page, pageSignals, runtimeLogs) {
  let title = '<unavailable>'
  let body = '<unavailable>'
  try { title = await page.title() } catch { /* page may already be closed */ }
  try { body = (await page.locator('body').innerText()).slice(0, 4_000) } catch { /* page may already be closed */ }
  return [
    `D3_WORKBENCH_BOOT_FAILED: ${error instanceof Error ? error.message : String(error)}`,
    `url=${page.url()}`,
    `title=${title}`,
    `body=${body}`,
    `page_signals=${pageSignals.join('\n') || '<none>'}`,
    `vite_tail=${runtimeLogs.vite.join('\n') || '<none>'}`,
    `agent_tail=${runtimeLogs.agent.join('\n') || '<none>'}`,
  ].join('\n')
}

function withinTimeout(promise, timeoutMs, label) {
  return Promise.race([
    promise,
    new Promise((_, reject) => setTimeout(() => reject(new Error(`${label} timed out after ${timeoutMs}ms`)), timeoutMs)),
  ])
}

async function requestStatus(baseUrl, path) {
  const response = await fetch(`${baseUrl}${path}`, { signal: AbortSignal.timeout(PAGE_TIMEOUT_MS) })
  let body = null
  try { body = await response.json() } catch { /* keep non-JSON response visible through status */ }
  return { status: response.status, body }
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
