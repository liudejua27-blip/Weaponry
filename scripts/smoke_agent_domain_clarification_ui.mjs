#!/usr/bin/env node
import { spawn } from 'node:child_process'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))

async function main() {
  const tempRoot = await mkdtemp(join(tmpdir(), 'forgecad-d003-ui-'))
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
          WUSHEN_LIBRARY_ROOT: join(tempRoot, 'library'),
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
    const page = await browser.newPage({ viewport: { width: 1536, height: 1024 } })
    let legacyBriefInterpretPosts = 0
    page.on('request', (request) => {
      if (request.method() === 'POST' && request.url().includes('/brief:interpret')) legacyBriefInterpretPosts += 1
    })
    await page.goto(`${viteBaseUrl}/#/cad`, { waitUntil: 'networkidle' })
    await page.waitForSelector('[data-testid="cad-workbench"]', { timeout: 20_000 })
    const input = page.getByPlaceholder('描述你想设计的道具…')
    await input.waitFor({ timeout: 20_000 })
    await input.fill('设计一台能飞的无人机载具')
    await page.getByRole('button', { name: '发送设计需求', exact: true }).click()
    const clarification = page.getByLabel('需要确认设计类别')
    await clarification.waitFor({ timeout: 20_000 })
    const text = await clarification.innerText()
    for (const expected of ['先确认设计对象', '你想先设计汽车、飞机、机械臂，还是未来概念道具？', '汽车与地面载具', '飞机与航空器']) {
      if (!text.includes(expected)) throw new Error(`clarification UI missing: ${expected}\n${text}`)
    }
    if (legacyBriefInterpretPosts !== 0) throw new Error('ambiguous input fell back to legacy Brief API')
    await clarification.getByRole('button', { name: '飞机与航空器', exact: true }).click()
    await page.getByLabel('Agent 完整外观方向').waitFor({ timeout: 20_000 })
    if (await page.getByLabel('需要确认设计类别').count() !== 0) throw new Error('clarification remained after choosing a domain')
    console.log(JSON.stringify({ ok: true, clarification: 'ambiguous', legacy_brief_posts: legacyBriefInterpretPosts }))
  } finally {
    if (browser) await browser.close()
    await Promise.all(processes.reverse().map(stopProcess))
    await rm(tempRoot, { recursive: true, force: true })
  }
}

async function launchBrowser() {
  const executablePath = process.env.WUSHEN_BROWSER_EXECUTABLE
  if (executablePath) return chromium.launch({ executablePath, headless: true })
  return chromium.launch({ channel: process.env.WUSHEN_BROWSER_CHANNEL || 'chrome', headless: true })
}

async function waitForHttp(url, child, label) {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    if (child.exitCode !== null) throw new Error(`${label} exited with ${child.exitCode}`)
    try {
      if ((await fetch(url)).ok) return
    } catch { /* process is still starting */ }
    await new Promise((resolveSleep) => setTimeout(resolveSleep, 200))
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
