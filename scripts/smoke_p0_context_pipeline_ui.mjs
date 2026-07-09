#!/usr/bin/env node
import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, stat, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const OUTPUT_DIR = join(ROOT, 'output', 'playwright')
const PATCH_BRUSH_SCREENSHOT = join(OUTPUT_DIR, 'p0-context-patch-brush.png')
const PATCH_COMPARISON_SCREENSHOT = join(OUTPUT_DIR, 'p0-context-patch-comparison.png')
const CONTEXT_3D_SCREENSHOT = join(OUTPUT_DIR, 'p0-context-3d-handoff.png')
const LIBRARY_SYNC_SCREENSHOT = join(OUTPUT_DIR, 'p0-context-library-sync.png')
const FAILURE_SCREENSHOT = join(OUTPUT_DIR, 'p0-context-continuity-failure.png')

async function main() {
  const tempRoot = await mkdtemp(join(tmpdir(), 'wushen_p0_context_ui_'))
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
          WUSHEN_GENERATE3D_ASYNC: '1',
          WUSHEN_GENERATE3D_WORKER: '1',
          WUSHEN_EXPORT_UNITY_ASYNC: '1',
          WUSHEN_EXPORT_UNITY_WORKER: '1',
          WUSHEN_LOCAL_WORKER_INTERVAL_SECONDS: '0.05',
          WUSHEN_LOCAL_WORKER_ID: 'ui_context_pipeline_smoke',
        },
        stdio: ['ignore', 'pipe', 'pipe'],
      }
    )
    processes.push(agent)
    await waitForHttp(`${agentBaseUrl}/api/health`, agent, 'agent health')

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

    const result = await runContextContinuityFlow(viteBaseUrl, agentBaseUrl)
    console.log(JSON.stringify({ ok: true, ...result }, null, 2))
  } finally {
    await Promise.all(processes.reverse().map(stopProcess))
    await rm(tempRoot, { recursive: true, force: true })
  }
}

async function runContextContinuityFlow(viteBaseUrl, agentBaseUrl) {
  const browser = await launchSystemBrowser()
  let page = null
  try {
    page = await browser.newPage({ viewport: { width: 1440, height: 1200 }, deviceScaleFactor: 1 })
    await page.goto(viteBaseUrl, { waitUntil: 'networkidle' })
    await page.waitForSelector('text=武神 Forge', { timeout: 20_000 })
    await page.waitForSelector('.top-bar .status-pill.connected', { timeout: 20_000 })

    const prompt = '青玉雷纹偃月刀，3渲2国风神兵，高拟真外观，仅作为虚构 Unity 游戏资产'
    await page.locator('#weapon-prompt').fill(prompt)
    const createResponsePromise = waitForApiResponse(page, /\/api\/weapons$/)
    await page.getByRole('button', { name: '自动生成 mock 武器' }).click()
    const createResponse = await createResponsePromise
    const created = await createResponse.json()
    const createJob = await waitForTerminalJob(agentBaseUrl, created.job_id)
    assertEqual(createJob.status, 'succeeded', 'create job status')
    const sourceVersionId = createJob.outputs?.current_version_id
    const sourceImageAssetId = assetIdByRole(createJob, 'concept_image')
    assert(sourceVersionId, 'create job did not expose current_version_id')
    assert(sourceImageAssetId, 'create job did not expose concept_image')
    await waitForInspectorContext(page, created.weapon_id, sourceVersionId)
    await assertTopBarVersion(page, sourceVersionId)

    await page.getByRole('button', { name: 'Patch Mode' }).click()
    await page.waitForSelector('text=源图已就绪', { timeout: 20_000 })
    assertEqual(await page.locator('#patch-weapon').inputValue(), created.weapon_id, 'Patch weapon select')
    assertEqual(await page.locator('#patch-version').inputValue(), sourceVersionId, 'Patch source version select')
    await drawPatchMask(page)
    await mkdir(OUTPUT_DIR, { recursive: true })
    await page.locator('.patch-panel').screenshot({ path: PATCH_BRUSH_SCREENSHOT })
    await assertScreenshot(PATCH_BRUSH_SCREENSHOT, 10_000)

    const patchResponsePromise = waitForApiResponse(page, new RegExp(`/api/weapons/${created.weapon_id}/patch$`))
    await page.getByRole('button', { name: '上传 mask 并生成 patch' }).click()
    const patchResponse = await patchResponsePromise
    await assertOkResponse(patchResponse, 'patch request')
    const patchRequestBody = parseRequestJson(patchResponse.request())
    assertEqual(patchRequestBody.source_version_id, sourceVersionId, 'patch request source_version_id')
    assertEqual(patchRequestBody.source_image_asset_id, sourceImageAssetId, 'patch request source_image_asset_id')
    const patch = await patchResponse.json()
    const patchJob = await waitForTerminalJob(agentBaseUrl, patch.job_id)
    assertEqual(patchJob.status, 'succeeded', 'patch job status')
    assertEqual(patchJob.type, 'patch_image', 'patch job type')
    const patchVersionId = patchJob.outputs?.current_version_id
    const patchImageAssetId = assetIdByRole(patchJob, 'concept_patch')
    assert(patchVersionId && patchVersionId !== sourceVersionId, 'patch did not create a new version')
    assert(patchImageAssetId, 'patch job did not expose concept_patch')
    await page.waitForSelector('text=Patch 前后对比', { timeout: 20_000 })
    await waitForSelectValue(page.locator('#patch-version'), patchVersionId)
    await waitForInspectorContext(page, created.weapon_id, patchVersionId)
    await assertTopBarVersion(page, patchVersionId)
    await assertVersionChain(agentBaseUrl, created.weapon_id, [
      { version_id: patchVersionId, parent_version_id: sourceVersionId, version_type: 'patch' },
    ])
    await page.locator('.comparison-panel').screenshot({ path: PATCH_COMPARISON_SCREENSHOT })
    await assertScreenshot(PATCH_COMPARISON_SCREENSHOT, 10_000)

    const preview = page.locator('.preview3d-card').first()
    await assertIncludes(await preview.innerText(), 'concept_patch', '3D preview source after patch')
    const generateButton = preview.getByRole('button', { name: '从当前图生成 3D' })
    await waitForEnabled(generateButton)
    const generateResponsePromise = waitForApiResponse(page, new RegExp(`/api/weapons/${created.weapon_id}/generate-3d$`))
    await generateButton.click()
    const generateResponse = await generateResponsePromise
    await assertOkResponse(generateResponse, 'generate-3d request')
    const generateRequestBody = parseRequestJson(generateResponse.request())
    assertEqual(generateRequestBody.source_version_id, patchVersionId, 'generate request source_version_id')
    assertEqual(generateRequestBody.source_image_asset_id, patchImageAssetId, 'generate request source_image_asset_id')
    const generate = await generateResponse.json()
    const generateJob = await waitForTerminalJob(agentBaseUrl, generate.job_id)
    assertEqual(generateJob.status, 'succeeded', 'generate-3d job status')
    assertEqual(generateJob.type, 'generate_3d', 'generate-3d job type')
    const roughVersionId = generateJob.outputs?.current_version_id
    const modelId = generateJob.outputs?.current_model_id
    assert(roughVersionId && roughVersionId !== patchVersionId, 'generate-3d did not create a rough_3d version')
    assert(modelId, 'generate-3d did not expose current_model_id')
    await assertVersionChain(agentBaseUrl, created.weapon_id, [
      { version_id: roughVersionId, parent_version_id: patchVersionId, version_type: 'rough_3d' },
    ])
    const runtime = await jsonRequest(agentBaseUrl, `/api/jobs/${generate.job_id}/runtime`)
    assert(runtime.provider_tasks?.some((task) => task.step === 'rough3d_submit' && task.provider_id === 'mock_3d' && task.status === 'succeeded'), 'generate runtime missing succeeded mock_3d task')
    await waitForInspectorContext(page, created.weapon_id, roughVersionId)
    await page.waitForSelector('text=mock_3d · 已完成', { timeout: 20_000 })
    const canvas = await assertCanvasRenderedAndInteractive(page)

    const exportButton = preview.getByRole('button', { name: '导出 Unity 包' })
    await waitForEnabled(exportButton)
    const exportResponsePromise = waitForApiResponse(page, new RegExp(`/api/weapons/${created.weapon_id}/export-unity$`))
    await exportButton.click()
    const exportResponse = await exportResponsePromise
    await assertOkResponse(exportResponse, 'export-unity request')
    const exportRequestBody = parseRequestJson(exportResponse.request())
    assertEqual(exportRequestBody.model_id, modelId, 'export request model_id')
    const exported = await exportResponse.json()
    const exportJob = await waitForTerminalJob(agentBaseUrl, exported.job_id)
    assertEqual(exportJob.status, 'succeeded', 'export-unity job status')
    assertEqual(exportJob.type, 'export_unity', 'export-unity job type')
    const exportVersionId = exportJob.outputs?.current_version_id
    const exportAssetId = assetIdByRole(exportJob, 'unity_export_package')
    assert(exportVersionId && exportAssetId, 'export-unity did not expose export version/package')
    await assertVersionChain(agentBaseUrl, created.weapon_id, [
      { version_id: exportVersionId, parent_version_id: roughVersionId, version_type: 'export' },
    ])
    await waitForInspectorContext(page, created.weapon_id, exportVersionId)
    await page.waitForSelector('text=ZIP ready', { timeout: 20_000 })
    const handoff = page.locator('.unity-handoff-card').first()
    await assertAssetLinks(handoff, 'Unity handoff card', 6)
    await page.screenshot({ path: CONTEXT_3D_SCREENSHOT, fullPage: true })
    await assertScreenshot(CONTEXT_3D_SCREENSHOT, 10_000)

    await page.getByRole('button', { name: '资产库' }).click()
    await page.waitForSelector('.library-version-card.selected', { timeout: 20_000 })
    await waitForInspectorContext(page, created.weapon_id, exportVersionId)
    await assertTopBarVersion(page, exportVersionId)
    const selectedLibraryCard = page.locator('.library-version-card.selected').first()
    assertIncludes(await selectedLibraryCard.innerText(), exportVersionId, 'selected library version')
    const libraryDetailText = await page.locator('.library-detail').first().innerText()
    for (const expected of ['版本 DAG', 'root', 'parent v', 'concept', 'patch', 'rough_3d', 'export']) {
      assertIncludes(libraryDetailText, expected, 'Library version DAG')
    }
    for (const expected of ['版本溯源', 'job_', 'roles', 'concept_patch', 'rough_optimized_glb', 'unity_export_package']) {
      assertIncludes(libraryDetailText, expected, 'Library provenance summary')
    }
    for (const expected of ['快速预览', '下载本版本文件', 'files', '下载 Unity ZIP', '打开 ZIP 位置', '查看生成轨迹', '预览', 'Unity handoff files']) {
      assertIncludes(libraryDetailText, expected, 'Library preview and batch download actions')
    }
    const previewImageCount = await page.locator('.library-asset-preview img').count()
    if (previewImageCount < 2) {
      throw new Error(`Library expected concept and patch preview thumbnails, got ${previewImageCount}`)
    }
    await assertLibraryHandoffCoverage(page.locator('.library-handoff-checklist'))
    await page.locator('.library-asset-row', { hasText: 'rough_optimized_glb' }).first().getByRole('button', { name: '预览' }).click()
    await assertVisibleText(page, '.library-preview-drawer', '资产预览')
    await assertVisibleText(page, '.library-preview-drawer', 'GLB header')
    await assertVisibleText(page, '.library-preview-drawer', 'meshes')
    await assertVisibleText(page, '.library-preview-drawer', 'materials')
    await page.locator('.library-asset-row', { hasText: 'unity_material_json' }).first().getByRole('button', { name: '预览' }).click()
    await assertVisibleText(page, '.library-preview-drawer', 'JSON metadata preview')
    await assertVisibleText(page, '.library-preview-drawer', 'schema')
    await page.locator('.library-asset-row', { hasText: 'unity_export_package' }).first().getByRole('button', { name: '预览' }).click()
    await assertVisibleText(page, '.library-preview-drawer', 'ZIP manifest / Unity package entries')
    await assertVisibleText(page, '.library-preview-drawer', 'manifest.json')
    await assertVisibleText(page, '.library-preview-drawer', 'all relative safe paths')
    await assertVisibleText(page, '.library-preview-drawer', 'rough_optimized.glb')
    await page.locator('.library-detail').first().screenshot({ path: LIBRARY_SYNC_SCREENSHOT })
    await assertScreenshot(LIBRARY_SYNC_SCREENSHOT, 10_000)
    await selectedLibraryCard.getByRole('button', { name: '查看生成轨迹' }).click()
    await page.waitForSelector('.job-center', { timeout: 20_000 })
    const jobCenterText = await page.locator('.workspace').innerText()
    assertIncludes(jobCenterText, exported.job_id, 'Library job trace restore')
    assertIncludes(jobCenterText, 'Unity 导出', 'Library job trace restore')
    assertIncludes(jobCenterText, 'Agent 执行轨迹', 'Library job trace restore')

    return {
      weapon_id: created.weapon_id,
      create_job_id: created.job_id,
      patch_job_id: patch.job_id,
      generate_job_id: generate.job_id,
      export_job_id: exported.job_id,
      source_version_id: sourceVersionId,
      patch_version_id: patchVersionId,
      rough3d_version_id: roughVersionId,
      export_version_id: exportVersionId,
      model_id: modelId,
      export_asset_id: exportAssetId,
      screenshots: {
        patch_brush: PATCH_BRUSH_SCREENSHOT,
        patch_comparison: PATCH_COMPARISON_SCREENSHOT,
        context_3d: CONTEXT_3D_SCREENSHOT,
        library_sync: LIBRARY_SYNC_SCREENSHOT,
      },
      canvas,
    }
  } catch (error) {
    if (page) {
      await mkdir(OUTPUT_DIR, { recursive: true })
      await page.screenshot({ path: FAILURE_SCREENSHOT, fullPage: true }).catch(() => undefined)
      const bodyText = await page.locator('body').innerText({ timeout: 1000 }).catch(() => '')
      if (bodyText) console.error(bodyText.slice(0, 3000))
    }
    throw error
  } finally {
    await browser.close()
  }
}

async function drawPatchMask(page) {
  const canvas = page.locator('.mask-canvas-shell canvas').first()
  await canvas.waitFor({ timeout: 20_000 })
  await page.waitForFunction(() => {
    const node = document.querySelector('.mask-canvas-shell canvas')
    return node instanceof HTMLCanvasElement && node.width >= 1000 && node.height >= 600
  }, { timeout: 20_000 })
  const box = await canvas.boundingBox()
  if (!box) throw new Error('Patch mask canvas has no bounding box')
  await page.getByRole('button', { name: '画笔' }).click()
  await page.mouse.move(box.x + box.width * 0.42, box.y + box.height * 0.42)
  await page.mouse.down()
  await page.mouse.move(box.x + box.width * 0.6, box.y + box.height * 0.54, { steps: 10 })
  await page.mouse.up()
  const whiteSamples = await canvas.evaluate((node) => {
    const context = node.getContext('2d')
    if (!context) return 0
    const data = context.getImageData(0, 0, node.width, node.height).data
    let count = 0
    for (let index = 0; index < data.length; index += 4) {
      if (data[index] > 180 && data[index + 1] > 180 && data[index + 2] > 180) count += 1
      if (count > 50) return count
    }
    return count
  })
  if (whiteSamples <= 50) throw new Error(`Patch mask did not record enough white pixels: ${whiteSamples}`)
  await waitForEnabled(page.getByRole('button', { name: '上传 mask 并生成 patch' }))
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
  if (before.checksum === after.checksum) throw new Error(`3D preview canvas did not change after drag: checksum=${before.checksum}`)
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

async function assertVersionChain(baseUrl, weaponId, expectedVersions) {
  const detail = await jsonRequest(baseUrl, `/api/weapons/${weaponId}`)
  for (const expected of expectedVersions) {
    const version = detail.versions?.find((item) => item.version_id === expected.version_id)
    assert(version, `version not found in weapon detail: ${expected.version_id}`)
    assertEqual(version.parent_version_id ?? null, expected.parent_version_id ?? null, `parent for ${expected.version_id}`)
    assertEqual(version.version_type, expected.version_type, `version_type for ${expected.version_id}`)
  }
  return detail
}

async function assertLibraryHandoffCoverage(checklists) {
  const rows = await checklists.evaluateAll((nodes) => nodes.map((node) => {
    const ready = Array.from(node.querySelectorAll('span.ready')).map((item) => item.textContent || '')
    const hrefs = Array.from(node.querySelectorAll('a')).map((item) => item.getAttribute('href') || '')
    return { ready, hrefs }
  }))
  const readyText = rows.flatMap((row) => row.ready).join('\n')
  for (const expected of ['raw', 'normalized', 'optimized', 'material', 'report', 'zip']) {
    assertIncludes(readyText, expected, 'Library handoff ready coverage')
  }
  const assetLinks = rows.flatMap((row) => row.hrefs).filter((href) => /\/api\/assets\/[^/]+\/file/.test(href))
  if (assetLinks.length < 6) throw new Error(`Library handoff expected at least 6 controlled asset links, got ${assetLinks.length}`)
}

async function assertAssetLinks(locator, label, minimumCount) {
  const links = await locator.locator('a').evaluateAll((anchors) => anchors.map((anchor) => anchor.getAttribute('href') || ''))
  const assetLinks = links.filter((href) => /\/api\/assets\/[^/]+\/file/.test(href))
  if (assetLinks.length < minimumCount) {
    throw new Error(`${label} expected at least ${minimumCount} controlled asset links, got ${assetLinks.length}: ${links.join(', ')}`)
  }
}

async function waitForInspectorContext(page, weaponId, versionId) {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    const rows = await page.locator('.inspector-summary .status-row').evaluateAll((nodes) => nodes.map((node) => node.textContent || '')).catch(() => [])
    if (rows.some((row) => row.includes('weapon') && row.includes(weaponId)) && rows.some((row) => row.includes('version') && row.includes(versionId))) return
    await page.waitForTimeout(250)
  }
  throw new Error(`Inspector did not sync to ${weaponId} / ${versionId}`)
}

async function assertTopBarVersion(page, versionId) {
  const topBar = page.locator('.top-bar').first()
  await topBar.waitFor({ timeout: 20_000 })
  assertIncludes(await topBar.innerText(), versionId, 'top bar')
}

async function waitForSelectValue(locator, value) {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    if (await locator.inputValue().catch(() => '') === value) return
    await locator.page().waitForTimeout(250)
  }
  throw new Error(`Select did not reach value: ${value}`)
}

async function waitForEnabled(locator) {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    if (await locator.isEnabled().catch(() => false)) return
    await locator.page().waitForTimeout(250)
  }
  throw new Error('Control did not become enabled')
}

function waitForApiResponse(page, pathPattern) {
  return page.waitForResponse((response) => {
    const request = response.request()
    if (request.method() !== 'POST') return false
    const pathname = new URL(response.url()).pathname
    return pathPattern.test(pathname)
  }, { timeout: 30_000 })
}

async function assertOkResponse(response, label) {
  if (response.ok()) return
  throw new Error(`${label} failed: ${response.status()} ${await response.text()}`)
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

async function assertScreenshot(path, minimumBytes) {
  const screenshot = await stat(path)
  if (screenshot.size < minimumBytes) throw new Error(`${path} looks too small: ${screenshot.size} bytes`)
}

async function assertVisibleText(page, selector, text) {
  await page.waitForSelector(selector, { timeout: 20_000 })
  const locator = page.locator(selector).first()
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    const content = await locator.innerText().catch(() => '')
    if (content.includes(text)) return
    await sleep(150)
  }
  const content = await locator.innerText().catch(() => '')
  assertIncludes(content, text, selector)
}

function assetIdByRole(job, role) {
  return Object.entries(job.outputs?.asset_roles ?? {}).find(([, value]) => value === role)?.[0] ?? null
}

function parseRequestJson(request) {
  const text = request.postData()
  return text ? JSON.parse(text) : {}
}

function assertIncludes(text, expected, label) {
  if (!text.includes(expected)) throw new Error(`${label} missing ${expected}: ${text}`)
}

function assertEqual(actual, expected, label) {
  if (actual !== expected) throw new Error(`${label} expected ${expected}, got ${actual}`)
}

function assert(condition, message) {
  if (!condition) throw new Error(message)
}

function sleep(ms) {
  return new Promise((resolveSleep) => setTimeout(resolveSleep, ms))
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
