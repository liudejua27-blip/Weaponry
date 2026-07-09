#!/usr/bin/env node
import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, stat, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const OUTPUT_DIR = join(ROOT, 'output', 'playwright')
const WORKBENCH_SCREENSHOT = join(OUTPUT_DIR, 'p0-runtime-handoff-workbench.png')
const RUNTIME_SCREENSHOT = join(OUTPUT_DIR, 'p0-runtime-handoff-runtime.png')
const HANDOFF_SCREENSHOT = join(OUTPUT_DIR, 'p0-runtime-handoff-card.png')
const LIBRARY_SCREENSHOT = join(OUTPUT_DIR, 'p0-runtime-handoff-library.png')

async function main() {
  const tempRoot = await mkdtemp(join(tmpdir(), 'wushen_p0_runtime_ui_'))
  const libraryRoot = join(tempRoot, 'WushenForgeLibrary')
  const agentPort = await freePort()
  const vitePort = await freePort()
  const agentBaseUrl = `http://127.0.0.1:${agentPort}`
  const viteBaseUrl = `http://127.0.0.1:${vitePort}`
  const processes = []

  try {
    const agent = spawn(
      join(ROOT, '.venv', 'bin', 'python'),
      [
        '-m',
        'uvicorn',
        'wushen_agent.main:create_app',
        '--factory',
        '--host',
        '127.0.0.1',
        '--port',
        String(agentPort),
      ],
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
          WUSHEN_GENERATE3D_ASYNC: '1',
          WUSHEN_GENERATE3D_WORKER: '1',
          WUSHEN_EXPORT_UNITY_ASYNC: '1',
          WUSHEN_EXPORT_UNITY_WORKER: '1',
          WUSHEN_LOCAL_WORKER_INTERVAL_SECONDS: '0.05',
          WUSHEN_LOCAL_WORKER_ID: 'ui_runtime_handoff_smoke',
        },
        stdio: ['ignore', 'pipe', 'pipe'],
      }
    )
    processes.push(agent)
    await waitForHttp(`${agentBaseUrl}/api/health`, agent, 'agent health')

    const seeded = await seedRuntimeHandoffData(agentBaseUrl)

    const vite = spawn(
      'npm',
      ['--workspace', 'apps/desktop', 'run', 'dev', '--', '--host', '127.0.0.1', '--port', String(vitePort)],
      {
        cwd: ROOT,
        env: {
          ...process.env,
          VITE_FORGE_API_BASE_URL: agentBaseUrl,
        },
        stdio: ['ignore', 'pipe', 'pipe'],
      }
    )
    processes.push(vite)
    await waitForHttp(viteBaseUrl, vite, 'vite frontend')

    const result = await captureRuntimeHandoffUi(viteBaseUrl, seeded.generate_job_id)

    console.log(JSON.stringify({
      ok: true,
      ...seeded,
      screenshots: {
        workbench: WORKBENCH_SCREENSHOT,
        runtime: RUNTIME_SCREENSHOT,
        handoff: HANDOFF_SCREENSHOT,
        library: LIBRARY_SCREENSHOT,
      },
      runtime: result.runtimeText,
      handoff: result.handoffText,
      library: result.libraryText,
      canvas: result.canvas,
    }, null, 2))
  } finally {
    await Promise.all(processes.reverse().map(stopProcess))
    await rm(tempRoot, { recursive: true, force: true })
  }
}

async function seedRuntimeHandoffData(baseUrl) {
  const createBody = {
    client_request_id: 'ui-runtime-handoff-source',
    text: '赤金龙纹偃月长剑，3渲2国风神兵，高拟真外观，仅作为虚构 Unity 游戏资产',
    sketch_asset_id: null,
    reference_asset_ids: [],
    auto_run: true,
    target: { phase: 'concept_to_rough_3d', engine: 'unity', output_format: 'glb' },
  }
  const created = await jsonRequest(baseUrl, '/api/weapons', { method: 'POST', body: createBody, idempotencyKey: 'ui-runtime-handoff-source-key' })
  const sourceJob = await jsonRequest(baseUrl, `/api/jobs/${created.job_id}`)
  const sourceVersionId = sourceJob.outputs.current_version_id
  const sourceImageAssetId = Object.entries(sourceJob.outputs.asset_roles).find(([, role]) => role === 'concept_image')?.[0]
  if (!sourceVersionId || !sourceImageAssetId) throw new Error('Source weapon did not produce concept image outputs')

  const generate = await jsonRequest(baseUrl, `/api/weapons/${created.weapon_id}/generate-3d`, {
    method: 'POST',
    idempotencyKey: 'ui-runtime-handoff-generate-key',
    body: {
      client_request_id: 'ui-runtime-handoff-generate',
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
  const generateJob = await waitForTerminalJob(baseUrl, generate.job_id)
  if (generateJob.status !== 'succeeded') throw new Error(`generate-3d did not succeed: ${generateJob.status}`)
  const runtime = await jsonRequest(baseUrl, `/api/jobs/${generate.job_id}/runtime`)
  if (!runtime.provider_tasks?.some((task) => task.step === 'rough3d_submit' && task.status === 'succeeded')) {
    throw new Error('Runtime state did not include succeeded rough3d provider task')
  }

  const detail = await jsonRequest(baseUrl, `/api/weapons/${created.weapon_id}`)
  if (!detail.current_model_id) throw new Error('Weapon detail did not expose current_model_id')
  const exported = await jsonRequest(baseUrl, `/api/weapons/${created.weapon_id}/export-unity`, {
    method: 'POST',
    idempotencyKey: 'ui-runtime-handoff-export-key',
    body: {
      client_request_id: 'ui-runtime-handoff-export',
      model_id: detail.current_model_id,
      export_type: 'unity_glb',
      include_source_spec: true,
      include_quality_reports: true,
    },
  })
  const exportJob = await waitForTerminalJob(baseUrl, exported.job_id)
  if (exportJob.status !== 'succeeded') throw new Error(`export-unity did not succeed: ${exportJob.status}`)

  return {
    weapon_id: created.weapon_id,
    source_job_id: created.job_id,
    generate_job_id: generate.job_id,
    export_job_id: exported.job_id,
  }
}

async function captureRuntimeHandoffUi(baseUrl, generateJobId) {
  const browser = await launchSystemBrowser()
  let page = null
  try {
    page = await browser.newPage({ viewport: { width: 1440, height: 1200 }, deviceScaleFactor: 1 })
    await page.addInitScript((jobId) => {
      localStorage.setItem('wushen.recentJobId', jobId)
    }, generateJobId)
    await page.goto(baseUrl, { waitUntil: 'networkidle' })
    await page.waitForSelector('text=武神 Forge', { timeout: 20_000 })
    await page.waitForSelector('text=Unity 交接状态', { timeout: 20_000 })
    await page.waitForSelector('text=Provider Task', { timeout: 20_000 })
    await page.waitForFunction(() => localStorage.getItem('wushen.recentJobId')?.startsWith('job_') === true)
    await page.waitForTimeout(1000)

    const runtime = page.locator('.runtime-mini:not(.empty)').first()
    const handoff = page.locator('.unity-handoff-card').first()
    const drawer = page.locator('.job-drawer').first()
    await runtime.waitFor({ timeout: 20_000 })
    await handoff.waitFor({ timeout: 20_000 })
    await assertVisibleText(page, '.job-trace-header', 'Agent 执行轨迹')
    await assertVisibleText(page, '.runtime-mini', 'Runtime')
    await assertVisibleText(page, '.runtime-mini', 'Provider Task')
    await assertVisibleText(page, '.runtime-mini', 'Checkpoint')
    await assertVisibleText(page, '.runtime-mini', 'Recovery')
    await assertVisibleText(page, '.unity-handoff-card', 'Unity 交接状态')

    const runtimeText = await runtime.innerText()
    const handoffText = await handoff.innerText()
    for (const expected of ['Provider Task', 'Checkpoint', 'Recovery']) {
      assertIncludes(runtimeText, expected, 'Runtime panel', { caseSensitive: false })
    }
    if (!runtimeText.includes('mock_3d · succeeded') && !runtimeText.includes('mock_3d · 已完成')) {
      throw new Error(`Runtime panel missing completed mock_3d provider task: ${runtimeText}`)
    }
    for (const expected of [
      'Unity 交接状态',
      'Raw GLB',
      'Normalized GLB',
      'Optimized GLB',
      'Unity Material',
      'Quality Report',
      'Export ZIP',
      '模型质量证据',
      'Triangles',
      'Materials',
      'Bounds ready',
      'Unity 轴向 / 尺度',
      'Forward',
      '+Z',
      'Long Axis',
      '+Y',
      'Pivot',
      'grip_center',
      'Scale',
      'normalized_game_asset_scale',
      'model ',
    ]) {
      assertIncludes(handoffText, expected, 'Unity handoff card')
    }
    if (!/(ZIP ready|可导出|缺资产)/.test(handoffText)) {
      throw new Error(`Unity handoff card missing export readiness state: ${handoffText}`)
    }
    await assertAssetLinks(handoff, 'Unity handoff card', 6)

    const canvas = await assertCanvasRenderedAndInteractive(page)

    await mkdir(OUTPUT_DIR, { recursive: true })
    await page.screenshot({ path: WORKBENCH_SCREENSHOT, fullPage: true })
    await runtime.screenshot({ path: RUNTIME_SCREENSHOT })
    await handoff.screenshot({ path: HANDOFF_SCREENSHOT })
    await assertScreenshot(WORKBENCH_SCREENSHOT, 10_000)
    await assertScreenshot(RUNTIME_SCREENSHOT, 4_000)
    await assertScreenshot(HANDOFF_SCREENSHOT, 4_000)

    await page.getByRole('button', { name: '资产库' }).click()
    const libraryChecklists = page.locator('.library-handoff-checklist')
    await libraryChecklists.first().waitFor({ timeout: 20_000 })
    const libraryText = (await libraryChecklists.allInnerTexts()).join('\n---\n')
    for (const expected of ['Unity handoff', 'QC passed', 'blockers 0', 'triangles 36', 'materials 1', 'bounds ready', 'raw', 'normalized', 'optimized', 'material', 'report', 'zip']) {
      assertIncludes(libraryText, expected, 'Library handoff checklist')
    }
    const libraryDetailText = await page.locator('.library-detail').first().innerText()
    for (const expected of ['版本 DAG', 'root', 'parent v', 'concept', 'rough_3d', 'export']) {
      assertIncludes(libraryDetailText, expected, 'Library version DAG')
    }
    for (const expected of ['版本溯源', 'job_', 'roles', 'concept_image', 'rough_optimized_glb', 'unity_export_package']) {
      assertIncludes(libraryDetailText, expected, 'Library provenance summary')
    }
    for (const expected of ['快速预览', '下载本版本文件', 'files', '下载 Unity ZIP', '打开 ZIP 位置', '查看生成轨迹', '预览', 'Unity handoff files']) {
      assertIncludes(libraryDetailText, expected, 'Library preview and batch download actions')
    }
    const previewImageCount = await page.locator('.library-asset-preview img').count()
    if (previewImageCount < 1) {
      throw new Error('Library expected at least one image preview thumbnail')
    }
    await assertLibraryHandoffCoverage(libraryChecklists)
    await page.locator('.library-detail').first().screenshot({ path: LIBRARY_SCREENSHOT })
    await assertScreenshot(LIBRARY_SCREENSHOT, 4_000)

    return { runtimeText, handoffText, libraryText, canvas }
  } catch (error) {
    if (page) {
      await mkdir(OUTPUT_DIR, { recursive: true })
      await page.screenshot({ path: join(OUTPUT_DIR, 'p0-runtime-handoff-failure.png'), fullPage: true }).catch(() => undefined)
      const bodyText = await page.locator('body').innerText({ timeout: 1000 }).catch(() => '')
      if (bodyText) console.error(bodyText.slice(0, 3000))
    }
    throw error
  } finally {
    await browser.close()
  }
}

async function assertVisibleText(page, selector, text) {
  const locator = page.locator(selector, { hasText: text }).first()
  await locator.waitFor({ timeout: 20_000 })
}

async function assertAssetLinks(locator, label, minimumCount) {
  const links = await locator.locator('a').evaluateAll((anchors) => anchors.map((anchor) => anchor.getAttribute('href') || ''))
  const assetLinks = links.filter((href) => /\/api\/assets\/[^/]+\/file/.test(href))
  if (assetLinks.length < minimumCount) {
    throw new Error(`${label} expected at least ${minimumCount} controlled asset links, got ${assetLinks.length}: ${links.join(', ')}`)
  }
}

async function assertLibraryHandoffCoverage(checklists) {
  const rows = await checklists.evaluateAll((nodes) => nodes.map((node) => {
    const ready = Array.from(node.querySelectorAll('span.ready')).map((item) => item.textContent || '')
    const missing = Array.from(node.querySelectorAll('span.missing')).map((item) => item.textContent || '')
    const hrefs = Array.from(node.querySelectorAll('a')).map((item) => item.getAttribute('href') || '')
    return { ready, missing, hrefs, text: node.textContent || '' }
  }))
  const readyText = rows.flatMap((row) => row.ready).join('\n')
  for (const expected of ['raw', 'normalized', 'optimized', 'material', 'report', 'zip']) {
    assertIncludes(readyText, expected, 'Library handoff ready coverage')
  }
  const qualityText = rows.map((row) => row.text).join('\n')
  for (const expected of ['QC passed', 'blockers 0', 'triangles 36', 'materials 1', 'bounds ready']) {
    assertIncludes(qualityText, expected, 'Library quality badge coverage')
  }
  const assetLinks = rows.flatMap((row) => row.hrefs).filter((href) => /\/api\/assets\/[^/]+\/file/.test(href))
  if (assetLinks.length < 6) {
    throw new Error(`Library handoff expected at least 6 controlled asset links, got ${assetLinks.length}: ${assetLinks.join(', ')}`)
  }
}

async function assertCanvasRenderedAndInteractive(page) {
  const frame = page.locator('.preview3d-frame').first()
  const canvas = frame.locator('canvas').first()
  await canvas.waitFor({ timeout: 20_000 })
  const before = await waitForCanvasChecksum(page)
  const box = await frame.boundingBox()
  if (!box) throw new Error('3D preview frame has no bounding box')
  await page.mouse.move(box.x + box.width * 0.4, box.y + box.height * 0.5)
  await page.mouse.down()
  await page.mouse.move(box.x + box.width * 0.75, box.y + box.height * 0.5, { steps: 8 })
  await page.mouse.up()
  await page.waitForTimeout(300)
  const after = await waitForCanvasChecksum(page)
  if (before.checksum === after.checksum) {
    throw new Error(`3D preview canvas did not change after drag: checksum=${before.checksum}`)
  }
  return { before, after }
}

async function waitForCanvasChecksum(page) {
  const deadline = Date.now() + 20_000
  let latest = null
  while (Date.now() < deadline) {
    latest = await page.locator('.preview3d-frame canvas').first().evaluate((canvas) => {
      const context = canvas.getContext('webgl2') || canvas.getContext('webgl')
      if (!context) return { checksum: 0, nonEmptySamples: 0, width: canvas.width, height: canvas.height }
      const width = context.drawingBufferWidth
      const height = context.drawingBufferHeight
      const samples = [
        [Math.floor(width * 0.2), Math.floor(height * 0.35)],
        [Math.floor(width * 0.5), Math.floor(height * 0.5)],
        [Math.floor(width * 0.8), Math.floor(height * 0.65)],
        [Math.floor(width * 0.5), Math.floor(height * 0.85)],
      ]
      const pixel = new Uint8Array(4)
      let checksum = 0
      let nonEmptySamples = 0
      for (const [x, y] of samples) {
        context.readPixels(x, y, 1, 1, context.RGBA, context.UNSIGNED_BYTE, pixel)
        checksum = (checksum * 131 + pixel[0] * 3 + pixel[1] * 5 + pixel[2] * 7 + pixel[3] * 11) >>> 0
        if (pixel[0] || pixel[1] || pixel[2] || pixel[3]) nonEmptySamples += 1
      }
      return { checksum, nonEmptySamples, width, height }
    })
    if (latest.nonEmptySamples >= 2 && latest.checksum > 0) return latest
    await page.waitForTimeout(250)
  }
  throw new Error(`3D preview canvas did not render non-empty pixels: ${JSON.stringify(latest)}`)
}

async function assertScreenshot(path, minimumBytes) {
  const screenshot = await stat(path)
  if (screenshot.size < minimumBytes) throw new Error(`${path} looks too small: ${screenshot.size} bytes`)
}

function assertIncludes(text, expected, label, options = {}) {
  const haystack = options.caseSensitive === false ? text.toLowerCase() : text
  const needle = options.caseSensitive === false ? expected.toLowerCase() : expected
  if (!haystack.includes(needle)) throw new Error(`${label} missing ${expected}: ${text}`)
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

async function waitForTerminalJob(baseUrl, jobId) {
  const deadline = Date.now() + 20_000
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
    if (child.exitCode !== null) {
      throw new Error(`${label} process exited early with code ${child.exitCode}`)
    }
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

function sleep(ms) {
  return new Promise((resolveSleep) => setTimeout(resolveSleep, ms))
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
