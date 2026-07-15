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
  try {
    const agent = spawn(
      join(ROOT, '.venv', 'bin', 'python'),
      ['-m', 'uvicorn', 'wushen_agent.main:create_app', '--factory', '--host', '127.0.0.1', '--port', String(agentPort)],
      {
        cwd: ROOT,
        env: {
          ...process.env,
          WUSHEN_LIBRARY_ROOT: libraryRoot,
          WUSHEN_MIGRATIONS_DIR: join(ROOT, 'migrations'),
          WUSHEN_CORS_ORIGINS: viteBaseUrl,
          WUSHEN_LOCAL_WORKER_ENABLED: '0',
          FORGECAD_CONCEPT_WORKER_ENABLED: '1',
          FORGECAD_CONCEPT_PLANNER_PROVIDER: 'deterministic_rules',
        },
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
    const page = await browser.newPage({ viewport: { width: 1440, height: 960 } })
    const browserErrors = []
    let legacyBriefPosts = 0
    page.on('pageerror', (error) => browserErrors.push(error.message))
    page.on('request', (request) => {
      if (request.method() === 'POST' && request.url().includes('/brief:interpret')) legacyBriefPosts += 1
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
    await page.getByPlaceholder('描述你想设计的道具…').waitFor({ timeout: 20_000 })
    await waitForUiProject(page)
    if (await page.locator('.weapon-viewport canvas').count() !== 1) {
      throw new Error('characterization requires exactly one WebGL canvas')
    }
    await assertText(page.locator('.agent-first-panel'), ['设计助手', '汽车', '飞机', '机械臂'])

    const projectId = await waitForProjectId(agentBaseUrl)
    const initialProject = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}`)
    const initialActiveDesign = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
    if (!isLegacyOrMissingSnapshot(initialActiveDesign)) {
      throw new Error(`new characterization project has an unexpected Snapshot state: ${initialActiveDesign.status} ${JSON.stringify(initialActiveDesign.body)}`)
    }

    // The current starter project can legitimately open as a legacy read-only
    // design. Request the explicit rebuild hand-off before exercising the Agent
    // path; this must not mutate the legacy version or silently bypass the
    // ACTIVE_DESIGN_INVALID write barrier.
    let legacyConversionRequested = false
    if (initialActiveDesign.body?.active_design?.source === 'legacy_concept_read_only') {
      const legacyNotice = page.getByLabel('旧版设计转换')
      await legacyNotice.waitFor({ timeout: 20_000 })
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
    const input = page.getByPlaceholder('描述你想设计的道具…')
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
    if (afterClarificationSnapshot.active_design?.source === 'agent_asset') {
      throw new Error('ambiguous input changed the legacy Snapshot before clarification')
    }

    const aircraftChoice = page.getByRole('button', { name: '飞机与航空器', exact: true })
    const aircraftChoiceCount = await aircraftChoice.count()
    if (aircraftChoiceCount !== 1) {
      const bodyText = await page.locator('body').innerText()
      throw new Error(`clarification did not expose exactly one aircraft choice (count=${aircraftChoiceCount}): ${bodyText.slice(0, 1500)}`)
    }
    await clickWithRetry(aircraftChoice, 'aircraft clarification choice')
    const directions = page.getByLabel('Agent 完整外观方向')
    await directions.waitFor({ timeout: 20_000 })
    await directions.getByRole('button').first().click()
    const candidates = page.getByLabel('分件候选')
    await candidates.waitFor({ timeout: 20_000 })
    await assertText(candidates, ['分件候选', '预览状态 · 未写入版本'])
    const afterPreviewSnapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
    if (!isLegacyOrMissingSnapshot(afterPreviewSnapshot)) {
      throw new Error('direction preview changed the legacy Snapshot before commit')
    }

    await page.getByRole('button', { name: '保存为可编辑模型', exact: true }).click()
    await candidates.getByText('可编辑资产 v1', { exact: true }).waitFor({ timeout: 20_000 })
    const committedSnapshot = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
    if (committedSnapshot.active_design?.source !== 'agent_asset') {
      throw new Error(`commit did not activate an Agent asset: ${JSON.stringify(committedSnapshot)}`)
    }
    if (committedSnapshot.export?.source_version_id !== committedSnapshot.active_design.asset_version_id) {
      throw new Error('export source did not follow the committed Agent asset')
    }
    if (await page.locator('.weapon-viewport canvas').count() !== 1) {
      throw new Error('workbench created a second WebGL canvas after commit')
    }

    await page.reload({ waitUntil: 'networkidle' })
    await page.getByLabel('分件候选').getByText('可编辑资产 v1', { exact: true }).waitFor({ timeout: 20_000 })
    if (await page.locator('.weapon-viewport canvas').count() !== 1) {
      throw new Error('workbench created a second WebGL canvas after reload')
    }
    if (legacyBriefPosts !== 0 || browserErrors.length > 0) {
      throw new Error(`browser characterization errors: legacyBriefPosts=${legacyBriefPosts}, errors=${browserErrors.join(' | ')}`)
    }
    console.log(JSON.stringify({
      ok: true,
      project_id: projectId,
      assertions: [
        'single_canvas',
        'ambiguous_clarification_write_barrier',
      'preview_does_not_write_version',
      'agent_commit_snapshot_export_alignment',
      'reload_restores_agent_head',
        ...(legacyConversionRequested ? ['legacy_rebuild_requires_explicit_handoff'] : []),
    ],
    }, null, 2))
  } finally {
    if (browser) await browser.close()
    await Promise.all(processes.reverse().map(stopProcess))
    await rm(tempRoot, { recursive: true, force: true })
  }
}

function isLegacyOrMissingSnapshot(response) {
  return response.status === 404 || response.body?.active_design?.source === 'legacy_concept_read_only'
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
