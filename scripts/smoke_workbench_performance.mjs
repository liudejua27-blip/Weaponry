#!/usr/bin/env node

// FGC-T003: resource lifecycle, single-WebGL, memory and bundle budget gate.
// This is deliberately a deterministic local smoke: it measures the workbench
// shell and renderer lifecycle, not live Provider quality or CAD correctness.
import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdtemp, mkdir, readdir, rm, stat } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'
import { legacyLifecycleTestOracleEnvironment } from './workbench_agent_blockout_test_helper.mjs'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const OUTPUT = join(ROOT, 'output', 'playwright', 'fgt003-performance.json')
const TIMEOUT_MS = 20_000
const LIMITS = {
  heap_growth_bytes: 64 * 1024 * 1024,
  geometries: 64,
  textures: 32,
  max_javascript_bytes: 1_200_000,
  max_total_javascript_bytes: 1_400_000,
  max_css_bytes: 150_000,
}

async function main() {
  const tempRoot = await mkdtemp(join(tmpdir(), 'forgecad-t003-performance-'))
  const libraryRoot = join(tempRoot, 'library')
  const agentPort = await freePort()
  const vitePort = await freePort()
  const agentBaseUrl = `http://127.0.0.1:${agentPort}`
  const viteBaseUrl = `http://127.0.0.1:${vitePort}`
  const processes = []
  let browser = null
  let page = null
  try {
    await mkdir(join(ROOT, 'output', 'playwright'), { recursive: true })
    const agent = spawn(join(ROOT, '.venv', 'bin', 'python'), ['-m', 'uvicorn', 'wushen_agent.main:create_app', '--factory', '--host', '127.0.0.1', '--port', String(agentPort)], {
      cwd: ROOT,
      env: legacyLifecycleTestOracleEnvironment(process.env, { WUSHEN_LIBRARY_ROOT: libraryRoot, WUSHEN_MIGRATIONS_DIR: join(ROOT, 'migrations'), WUSHEN_CORS_ORIGINS: viteBaseUrl, WUSHEN_LOCAL_WORKER_ENABLED: '0', FORGECAD_CONCEPT_WORKER_ENABLED: '1', FORGECAD_CONCEPT_PLANNER_PROVIDER: 'deterministic_rules' }),
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    processes.push(agent)
    await waitForHttp(`${agentBaseUrl}/api/health`, agent, 'Agent')
    const vite = spawn(process.execPath, [join(ROOT, 'node_modules', 'vite', 'bin', 'vite.js'), '--host', '127.0.0.1', '--port', String(vitePort)], {
      cwd: join(ROOT, 'apps', 'desktop'), env: { ...process.env, VITE_FORGE_API_BASE_URL: agentBaseUrl }, stdio: ['ignore', 'pipe', 'pipe'],
    })
    processes.push(vite)
    await waitForHttp(viteBaseUrl, vite, 'Vite')
    const bundle = await inspectBundle()
    browser = await launchBrowser()
    const context = await browser.newContext({ viewport: { width: 1440, height: 960 } })
    page = await context.newPage()
    const client = await context.newCDPSession(page)
    await client.send('Performance.enable')
    await client.send('HeapProfiler.enable')
    await page.goto(`${viteBaseUrl}/#/cad`, { waitUntil: 'domcontentloaded' })
    await page.locator('.cad-workbench').waitFor({ timeout: TIMEOUT_MS })
    await page.locator('.weapon-viewport canvas').waitFor({ timeout: TIMEOUT_MS })
    const baseline = await collectMetrics(page, client)

    const drawerActions = [
      ['检查', '关闭模型检查'],
      ['导出', '关闭导出'],
      ['替换', '关闭组件选择'],
    ]
    for (let cycle = 0; cycle < 10; cycle += 1) {
      for (const [openLabel, closeLabel] of drawerActions) {
        const opener = page.getByRole('button', { name: openLabel, exact: true })
        if (await opener.count() && await opener.isVisible()) {
          await opener.click()
          const closer = page.getByRole('button', { name: closeLabel, exact: true })
          await closer.waitFor({ timeout: TIMEOUT_MS })
          await closer.click()
          await closer.waitFor({ state: 'detached', timeout: TIMEOUT_MS }).catch(() => {})
        }
      }
      await assertRendererShape(page, `drawer cycle ${cycle + 1}`)
    }
    const afterDrawers = await collectMetrics(page, client)
    await client.send('HeapProfiler.collectGarbage')
    const afterGc = await collectMetrics(page, client)

    for (let cycle = 0; cycle < 3; cycle += 1) {
      await page.reload({ waitUntil: 'domcontentloaded' })
      await page.locator('.cad-workbench').waitFor({ timeout: TIMEOUT_MS })
      await page.locator('.weapon-viewport canvas').waitFor({ timeout: TIMEOUT_MS })
      await assertRendererShape(page, `reload cycle ${cycle + 1}`)
    }
    const afterReloads = await collectMetrics(page, client)
    const heapGrowth = Math.max(0, afterGc.heap_used_bytes - baseline.heap_used_bytes)
    const checks = {
      single_canvas: afterReloads.canvas_count === 1,
      single_context: afterReloads.active_contexts === 1,
      renderer_generation_stable: afterDrawers.renderer_generation - baseline.renderer_generation <= 1,
      heap_growth_within_budget: heapGrowth <= LIMITS.heap_growth_bytes,
      geometries_within_budget: afterReloads.geometries <= LIMITS.geometries,
      textures_within_budget: afterReloads.textures <= LIMITS.textures,
      bundle_within_budget: bundle.ok,
    }
    for (const [name, ok] of Object.entries(checks)) assert(ok, `${name} failed: ${JSON.stringify({ baseline, afterDrawers, afterGc, afterReloads, bundle, heapGrowth })}`)
    const report = { ok: true, suite: 'FGC-T003', limits: LIMITS, checks, baseline, after_drawers: afterDrawers, after_gc: afterGc, after_reloads: afterReloads, peak_geometry_count_during_drawers: afterDrawers.geometries, heap_growth_after_gc_bytes: heapGrowth, drawer_cycles: 10, reload_cycles: 3, bundle }
    await (await import('node:fs/promises')).writeFile(OUTPUT, `${JSON.stringify(report, null, 2)}\n`)
    console.log(JSON.stringify(report, null, 2))
  } finally {
    if (page) await page.close().catch(() => {})
    if (browser) await browser.close().catch(() => {})
    await Promise.all(processes.reverse().map(stopProcess))
    await rm(tempRoot, { recursive: true, force: true })
  }
}

async function inspectBundle() {
  const assets = join(ROOT, 'apps', 'desktop', 'dist', 'assets')
  if (!existsSync(assets)) return { ok: false, reason: 'apps/desktop/dist/assets is missing; run desktop:build first' }
  const names = await readdir(assets)
  const entries = []
  for (const name of names) {
    if (!/\.(js|css)$/.test(name)) continue
    const bytes = (await stat(join(assets, name))).size
    entries.push({ name, bytes, kind: name.endsWith('.js') ? 'javascript' : 'css' })
  }
  const js = entries.filter((entry) => entry.kind === 'javascript')
  const css = entries.filter((entry) => entry.kind === 'css')
  const maxJs = Math.max(0, ...js.map((entry) => entry.bytes))
  const totalJs = js.reduce((sum, entry) => sum + entry.bytes, 0)
  const totalCss = css.reduce((sum, entry) => sum + entry.bytes, 0)
  return { ok: maxJs <= LIMITS.max_javascript_bytes && totalJs <= LIMITS.max_total_javascript_bytes && totalCss <= LIMITS.max_css_bytes, max_javascript_bytes: maxJs, total_javascript_bytes: totalJs, total_css_bytes: totalCss, entries }
}

async function collectMetrics(page, client) {
  const metrics = await client.send('Performance.getMetrics')
  const value = Object.fromEntries(metrics.metrics.map((metric) => [metric.name, metric.value]))
  return page.evaluate((cdpHeap) => {
    const host = document.querySelector('.weapon-viewport')
    return { canvas_count: host?.querySelectorAll('canvas').length ?? 0, active_contexts: Number(host?.dataset.activeWebglContexts ?? 0), renderer_generation: Number(host?.dataset.rendererGeneration ?? 0), geometries: Number(host?.dataset.rendererGeometries ?? 0), textures: Number(host?.dataset.rendererTextures ?? 0), heap_used_bytes: cdpHeap }
  }, Math.round((value.JSHeapUsedSize ?? 0) * 1))
}

async function assertRendererShape(page, label) {
  const shape = await page.locator('.weapon-viewport').evaluate((host) => ({ canvas: host.querySelectorAll('canvas').length, contexts: Number(host.dataset.activeWebglContexts ?? 0) }))
  assert(shape.canvas === 1, `${label}: expected one canvas, got ${shape.canvas}`)
  assert(shape.contexts === 1, `${label}: expected one active context, got ${shape.contexts}`)
}

function assert(condition, message) { if (!condition) throw new Error(message) }
async function launchBrowser() { const executablePath = process.env.WUSHEN_BROWSER_EXECUTABLE; if (executablePath) return chromium.launch({ executablePath, headless: true }); if (process.platform === 'darwin' && existsSync('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome')) return chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); return chromium.launch({ channel: process.env.WUSHEN_BROWSER_CHANNEL || 'chrome', headless: true }) }
async function waitForHttp(url, child, label) { const deadline = Date.now() + TIMEOUT_MS; while (Date.now() < deadline) { if (child.exitCode !== null) throw new Error(`${label} exited with ${child.exitCode}`); try { if ((await fetch(url)).ok) return } catch {} await sleep(200) } throw new Error(`${label} did not become ready: ${url}`) }
async function freePort() { const net = await import('node:net'); return new Promise((resolvePort, reject) => { const server = net.createServer(); server.once('error', reject); server.listen(0, '127.0.0.1', () => { const address = server.address(); server.close(() => resolvePort(address.port)) }) }) }
function stopProcess(child) { if (!child || child.exitCode !== null) return Promise.resolve(); return new Promise((resolveStop) => { const timer = setTimeout(() => { child.kill('SIGKILL'); resolveStop() }, 3_000); child.once('exit', () => { clearTimeout(timer); resolveStop() }); child.kill('SIGTERM') }) }
function sleep(milliseconds) { return new Promise((resolveSleep) => setTimeout(resolveSleep, milliseconds)) }
main().catch((error) => { console.error(error instanceof Error ? error.stack : error); process.exitCode = 1 })
