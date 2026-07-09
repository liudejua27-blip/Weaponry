#!/usr/bin/env node
import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, rm, stat } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const OUTPUT_DIR = join(ROOT, 'output', 'playwright')
const WEAPON_LINK_SCREENSHOT = join(OUTPUT_DIR, 'p1-deeplink-weapon-version.png')
const JOB_LINK_SCREENSHOT = join(OUTPUT_DIR, 'p1-deeplink-job-trace.png')

async function main() {
  const tempRoot = await mkdtemp(join(tmpdir(), 'wushen_p1_deeplink_ui_'))
  const libraryRoot = join(tempRoot, 'WushenForgeLibrary')
  const agentPort = await freePort()
  const vitePort = await freePort()
  const agentBaseUrl = `http://127.0.0.1:${agentPort}`
  const viteBaseUrl = `http://127.0.0.1:${vitePort}`
  const processes = []

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
          WUSHEN_LLM_PROVIDER: 'mock',
          WUSHEN_IMAGE_PROVIDER: 'mock',
          WUSHEN_3D_PROVIDER: 'mock',
        },
        stdio: ['ignore', 'pipe', 'pipe'],
      }
    )
    processes.push(agent)
    await waitForHttp(`${agentBaseUrl}/api/health`, agent, 'agent health')

    const seeded = await seedDeeplinkData(agentBaseUrl)

    const vite = spawn(
      'npm',
      ['--workspace', 'apps/desktop', 'run', 'dev', '--', '--host', '127.0.0.1', '--port', String(vitePort)],
      {
        cwd: ROOT,
        env: { ...process.env, VITE_FORGE_API_BASE_URL: agentBaseUrl },
        stdio: ['ignore', 'pipe', 'pipe'],
      }
    )
    processes.push(vite)
    await waitForHttp(viteBaseUrl, vite, 'vite frontend')

    const result = await runDeeplinkUi(viteBaseUrl, seeded)
    console.log(JSON.stringify({ ok: true, ...seeded, ...result }, null, 2))
  } finally {
    await Promise.all(processes.reverse().map(stopProcess))
    await rm(tempRoot, { recursive: true, force: true })
  }
}

async function seedDeeplinkData(baseUrl) {
  const created = await jsonRequest(baseUrl, '/api/weapons', {
    method: 'POST',
    idempotencyKey: 'ui-deeplink-source-key',
    body: {
      client_request_id: 'ui-deeplink-source',
      text: '玄铁青鳞长枪，3渲2国风神兵，高拟真外观，仅作为虚构 Unity 游戏资产',
      sketch_asset_id: null,
      reference_asset_ids: [],
      auto_run: true,
      target: { phase: 'concept_to_rough_3d', engine: 'unity', output_format: 'glb' },
    },
  })
  const sourceJob = await waitForTerminalJob(baseUrl, created.job_id)
  const sourceVersionId = sourceJob.outputs?.current_version_id
  const sourceImageAssetId = assetIdByRole(sourceJob, 'concept_image')
  if (!sourceVersionId || !sourceImageAssetId) throw new Error('create weapon did not produce a source image version')

  const generated = await jsonRequest(baseUrl, `/api/weapons/${created.weapon_id}/generate-3d`, {
    method: 'POST',
    idempotencyKey: 'ui-deeplink-generate-key',
    body: {
      client_request_id: 'ui-deeplink-generate',
      source_version_id: sourceVersionId,
      source_image_asset_id: sourceImageAssetId,
      provider_id: 'mock_3d',
      target_format: 'glb',
      style: 'stylized_toon_weapon',
      orientation_policy: { forward_axis: '+Z', long_axis: '+Y', pivot: 'grip_center' },
      scale_policy: 'normalized_game_asset_scale',
      build_unity_export: true,
    },
  })
  const generateJob = await waitForTerminalJob(baseUrl, generated.job_id)
  if (generateJob.status !== 'succeeded') throw new Error(`generate-3d did not succeed: ${generateJob.status}`)
  const rough3dVersionId = generateJob.outputs?.current_version_id
  const modelId = generateJob.outputs?.current_model_id
  if (!rough3dVersionId || !modelId) throw new Error('generate-3d did not produce a model/version')

  const exported = await jsonRequest(baseUrl, `/api/weapons/${created.weapon_id}/export-unity`, {
    method: 'POST',
    idempotencyKey: 'ui-deeplink-export-key',
    body: {
      client_request_id: 'ui-deeplink-export',
      model_id: modelId,
      export_type: 'unity_glb',
      include_source_spec: true,
      include_quality_reports: true,
    },
  })
  const exportJob = await waitForTerminalJob(baseUrl, exported.job_id)
  if (exportJob.status !== 'succeeded') throw new Error(`export-unity did not succeed: ${exportJob.status}`)
  const exportVersionId = exportJob.outputs?.current_version_id
  if (!exportVersionId) throw new Error('export-unity did not produce an export version')

  return {
    weapon_id: created.weapon_id,
    source_job_id: created.job_id,
    generate_job_id: generated.job_id,
    export_job_id: exported.job_id,
    source_version_id: sourceVersionId,
    rough3d_version_id: rough3dVersionId,
    export_version_id: exportVersionId,
    model_id: modelId,
  }
}

async function runDeeplinkUi(baseUrl, seeded) {
  const browser = await launchSystemBrowser()
  const page = await browser.newPage({ viewport: { width: 1440, height: 1200 }, deviceScaleFactor: 1 })
  try {
    await mkdir(OUTPUT_DIR, { recursive: true })

    await page.goto(`${baseUrl}/#/weapons/${seeded.weapon_id}/versions/${seeded.rough3d_version_id}`, { waitUntil: 'networkidle' })
    await page.waitForSelector('.top-bar .status-pill.connected', { timeout: 20_000 })
    await page.waitForSelector('.library-detail', { timeout: 20_000 })
    await waitForText(page.locator('.top-bar'), seeded.rough3d_version_id)
    await assertText(page.locator('.library-version-card.selected'), [seeded.rough3d_version_id, 'rough_3d', '当前上下文版本'])
    await assertText(page.locator('.library-detail'), ['版本 DAG', 'Unity handoff', 'rough_optimized_glb'])
    await page.screenshot({ path: WEAPON_LINK_SCREENSHOT, fullPage: true })
    await assertScreenshot(WEAPON_LINK_SCREENSHOT, 10_000)

    await page.goto(`${baseUrl}/#/weapons/${seeded.weapon_id}/versions/${seeded.export_version_id}`, { waitUntil: 'networkidle' })
    await page.waitForSelector('.library-version-card.selected', { timeout: 20_000 })
    await waitForText(page.locator('.top-bar'), seeded.export_version_id)
    await assertText(page.locator('.library-version-card.selected'), [seeded.export_version_id, 'export', '下载 Unity ZIP'])

    await page.goto(`${baseUrl}/#/jobs/${seeded.export_job_id}`, { waitUntil: 'networkidle' })
    await page.waitForSelector('.job-center', { timeout: 20_000 })
    await waitForText(page.locator('.job-center'), seeded.export_job_id)
    await page.waitForSelector('.job-center-detail .job-drawer', { timeout: 20_000 })
    await assertText(page.locator('.job-center'), ['Unity 导出', 'Agent 执行轨迹', 'Action 审计'])
    const recentJobId = await page.evaluate(() => localStorage.getItem('wushen.recentJobId'))
    if (recentJobId !== seeded.export_job_id) throw new Error(`deep-linked job was not persisted as recent job: ${recentJobId}`)
    await page.screenshot({ path: JOB_LINK_SCREENSHOT, fullPage: true })
    await assertScreenshot(JOB_LINK_SCREENSHOT, 10_000)

    await page.getByRole('button', { name: 'Forge 工作台' }).click()
    await page.waitForFunction(() => window.location.hash === '#/forge')
    await browser.close()
    return {
      screenshots: {
        weapon_version: WEAPON_LINK_SCREENSHOT,
        job_trace: JOB_LINK_SCREENSHOT,
      },
    }
  } catch (error) {
    await page.screenshot({ path: join(OUTPUT_DIR, 'p1-deeplink-failure.png'), fullPage: true }).catch(() => undefined)
    await browser.close()
    throw error
  }
}

async function waitForTerminalJob(baseUrl, jobId) {
  const deadline = Date.now() + 25_000
  while (Date.now() < deadline) {
    const job = await jsonRequest(baseUrl, `/api/jobs/${jobId}`)
    if (['succeeded', 'failed', 'cancelled', 'partial_succeeded'].includes(job.status)) return job
    await sleep(200)
  }
  throw new Error(`Job did not reach terminal status: ${jobId}`)
}

async function jsonRequest(baseUrl, path, options = {}) {
  const response = await fetch(baseUrl + path, {
    method: options.method || 'GET',
    headers: {
      'Content-Type': 'application/json',
      ...(options.idempotencyKey ? { 'Idempotency-Key': options.idempotencyKey } : {}),
    },
    body: options.body ? JSON.stringify(options.body) : undefined,
  })
  if (!response.ok) throw new Error(`${options.method || 'GET'} ${path} failed: ${response.status} ${await response.text()}`)
  return response.json()
}

async function waitForHttp(url, child, label) {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    if (child.exitCode !== null) throw new Error(`${label} process exited early with code ${child.exitCode}`)
    try {
      const response = await fetch(url)
      if (response.ok) return
    } catch {
      // keep polling
    }
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

async function launchSystemBrowser() {
  const executablePath = process.env.WUSHEN_BROWSER_EXECUTABLE
  if (executablePath) return chromium.launch({ executablePath, headless: true })
  const channel = process.env.WUSHEN_BROWSER_CHANNEL || 'chrome'
  try {
    return await chromium.launch({ channel, headless: true })
  } catch (error) {
    throw new Error(`Browser smoke requires system Chrome or WUSHEN_BROWSER_EXECUTABLE. Launch failed: ${error}`)
  }
}

function stopProcess(child) {
  return new Promise((resolveStop) => {
    if (!child || child.exitCode !== null) {
      resolveStop()
      return
    }
    const timer = setTimeout(() => {
      child.kill('SIGKILL')
      resolveStop()
    }, 5000)
    child.once('exit', () => {
      clearTimeout(timer)
      resolveStop()
    })
    child.kill('SIGTERM')
  })
}

async function assertText(locator, expected) {
  const text = await locator.innerText({ timeout: 20_000 })
  for (const item of expected) {
    if (!text.includes(item)) throw new Error(`Missing ${item}: ${text}`)
  }
}

async function waitForText(locator, expected) {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    const text = await locator.innerText().catch(() => '')
    if (text.includes(expected)) return
    await sleep(150)
  }
  throw new Error(`Timed out waiting for ${expected}`)
}

async function assertScreenshot(path, minimumBytes) {
  const screenshot = await stat(path)
  if (screenshot.size < minimumBytes) throw new Error(`${path} looks too small: ${screenshot.size} bytes`)
}

function assetIdByRole(job, role) {
  return Object.entries(job.outputs?.asset_roles ?? {}).find(([, value]) => value === role)?.[0] ?? null
}

function sleep(ms) {
  return new Promise((resolveSleep) => setTimeout(resolveSleep, ms))
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
