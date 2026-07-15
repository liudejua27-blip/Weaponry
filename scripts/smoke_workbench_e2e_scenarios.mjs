#!/usr/bin/env node

// FGC-T002: split the broad workbench regression into readable, named browser
// scenarios. The scenarios intentionally use the deterministic local Planner;
// they prove UI/API state alignment, not live Provider quality.
import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const OUTPUT_DIR = join(ROOT, 'output', 'playwright', 'fgt002-scenarios')
const TIMEOUT_MS = 20_000

async function main() {
  const tempRoot = await mkdtemp(join(tmpdir(), 'forgecad-t002-workbench-'))
  const libraryRoot = join(tempRoot, 'library')
  const agentPort = await freePort()
  const vitePort = await freePort()
  const agentBaseUrl = `http://127.0.0.1:${agentPort}`
  const viteBaseUrl = `http://127.0.0.1:${vitePort}`
  const processes = []
  const results = []
  let browser = null
  let projectId = null

  try {
    await mkdir(OUTPUT_DIR, { recursive: true })
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
      {
        cwd: join(ROOT, 'apps', 'desktop'),
        env: { ...process.env, VITE_FORGE_API_BASE_URL: agentBaseUrl },
        stdio: ['ignore', 'pipe', 'pipe'],
      },
    )
    processes.push(vite)
    await waitForHttp(viteBaseUrl, vite, 'Vite')

    browser = await launchBrowser()
    const context = await browser.newContext({ viewport: { width: 1440, height: 960 } })

    await runScenario(results, 'T002-01-bootstrap-single-canvas', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = await waitForProjectId(agentBaseUrl)
        const snapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(await page.locator('.weapon-viewport canvas').count() === 1, 'workbench must have one WebGL canvas')
        assert(await page.getByPlaceholder('描述你想设计的道具…').count() === 1, 'Agent input must be present')
        return evidence(projectId, snapshot, ['initial_workbench', 'single_canvas', 'agent_input'])
      } finally { await page.close() }
    })

    await runScenario(results, 'T002-02-legacy-explicit-handoff', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        const notice = page.getByLabel('旧版设计转换')
        if (await notice.count() === 1 && await notice.isVisible()) {
          await notice.getByRole('button', { name: '让 Agent 重建可编辑资产', exact: true }).click()
          await page.getByText(/已准备 legacy 只读设计的 Agent 重建输入/).waitFor({ timeout: TIMEOUT_MS })
        }
        const snapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(snapshot.body?.active_design?.source !== 'agent_asset', 'legacy handoff must not activate an Agent asset')
        return evidence(projectId, snapshot, ['explicit_legacy_handoff', 'legacy_write_barrier'])
      } finally { await page.close() }
    })

    await runScenario(results, 'T002-03-ambiguous-clarification-write-barrier', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        await sendBrief(page, '设计一台能飞的无人机载具')
        const clarification = page.getByLabel('需要确认设计类别')
        await clarification.waitFor({ timeout: TIMEOUT_MS })
        await assertText(clarification, ['先确认设计对象', '汽车与地面载具', '飞机与航空器'])
        const snapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(snapshot.body?.active_design?.source !== 'agent_asset', 'ambiguous input must not create an Agent asset')
        assert((await page.locator('.agent-first-panel').innerText()).includes('同时接近多个方向'), 'clarification must explain ambiguity')
        await clarification.getByRole('button', { name: '汽车与地面载具', exact: true }).click()
        await page.getByLabel('Agent 完整外观方向').waitFor({ timeout: TIMEOUT_MS })
        return evidence(projectId, snapshot, ['one_question', 'zero_asset_write', 'choice_continues_flow'])
      } finally { await page.close() }
    })

    await runScenario(results, 'T002-03b-scope-stop-before-direction', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        await sendBrief(page, '请设计一把现实枪械，并给出加工图纸和制造尺寸。')
        const stop = page.getByLabel('当前请求超出概念范围')
        await stop.waitFor({ timeout: TIMEOUT_MS })
        await assertText(stop, ['请换一种外观创意描述', '未发送给模型', '没有创建 3D 模型、版本或导出'])
        assert(await page.getByLabel('Agent 完整外观方向').count() === 0, 'scope stop must not show directions')
        const snapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(snapshot.body?.active_design?.source !== 'agent_asset', 'scope stop must not create an Agent asset')
        return evidence(projectId, snapshot, ['local_scope_stop', 'no_direction_card', 'zero_asset_write'])
      } finally { await page.close() }
    })

    await runScenario(results, 'T002-04b-unsaved-direction-concept-images-no-write', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        const before = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        await sendBrief(page, '设计一辆双座冰原探索汽车，完整封闭车身，深色耐候外观，作为非功能展示模型。')
        const directions = page.getByLabel('Agent 完整外观方向')
        await directions.waitFor({ timeout: TIMEOUT_MS })
        await page.waitForFunction(
          () => document.querySelectorAll('img.agent-direction-concept-image').length === 3,
          undefined,
          { timeout: TIMEOUT_MS },
        )
        assert(await page.locator('img.agent-direction-concept-image').count() === 3, 'three current directions must receive disposable software concept images')
        assert(await page.locator('.weapon-viewport canvas').count() === 1, 'concept image cards must not create a second WebGL canvas')
        const beforeSelect = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(beforeSelect.active_design?.asset_version_id === before.active_design?.asset_version_id, 'concept images must not advance the active asset version')
        const directionButtons = directions.getByRole('button')
        assert(await directionButtons.count() === 3, 'each current direction must remain selectable')
        await directionButtons.nth(0).click()
        await page.getByLabel('分件候选').waitFor({ timeout: TIMEOUT_MS })
        assert(await page.locator('img.agent-direction-concept-image').count() === 0, 'selecting a direction must discard the now-stale concept images')
        const after = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(after.active_design?.asset_version_id === before.active_design?.asset_version_id, 'temporary image and selected blockout must not write an asset version')
        return evidence(projectId, after, ['three_software_concept_images', 'same_source_direction_context', 'discard_on_selection', 'preview_no_version_write', 'single_canvas'])
      } finally { await page.close() }
    })

    for (const [scenarioId, brief, labels] of [
      ['T002-04-car-brief', '设计一辆双座冰原探索汽车，完整封闭车身，大轮胎，短前后悬，深色耐候外观。', ['car_direction']],
      ['T002-05-aircraft-brief', '设计一架紧凑的垂直起降概念飞机，宽机身、双短翼、深色舱罩，用于科幻救援展示。', ['aircraft_direction']],
      ['T002-06-robotic-arm-brief', '设计一台三关节维护机械臂，固定基座、两段连杆、旋转腕部和夹持末端。', ['robotic_arm_direction']],
      ['T002-07-future-prop-brief', '设计一个厚重、紧凑、非功能性的未来武器概念道具，用于游戏展示。', ['future_prop_direction', 'non_functional_scope']],
    ]) {
      await runScenario(results, scenarioId, async () => {
        const page = await openWorkbench(context, viteBaseUrl)
        try {
          projectId = projectId ?? await waitForProjectId(agentBaseUrl)
          await sendBrief(page, brief)
          const directions = page.getByLabel('Agent 完整外观方向')
          await directions.waitFor({ timeout: TIMEOUT_MS })
          await assertText(directions, ['完整外观方向', '生成轻量 blockout'])
          await directions.getByRole('button').first().click()
          await page.getByLabel('分件候选').waitFor({ timeout: TIMEOUT_MS })
          const snapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
          assert(snapshot.body?.active_design?.source !== 'agent_asset', 'direction selection must remain preview-only')
          return evidence(projectId, snapshot, labels.concat(['three_directions', 'preview_only']))
        } finally { await page.close() }
      })
    }

    await runScenario(results, 'T002-08-preview-does-not-write-version', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        const before = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        await sendBrief(page, '设计一辆双座冰原探索汽车，完整封闭车身，作为非功能展示模型。')
        await page.getByLabel('Agent 完整外观方向').waitFor({ timeout: TIMEOUT_MS })
        await page.getByLabel('Agent 完整外观方向').getByRole('button').first().click()
        await page.getByLabel('分件候选').waitFor({ timeout: TIMEOUT_MS })
        const after = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(after.active_design?.asset_version_id === before.active_design?.asset_version_id, 'preview must not advance asset version')
        assert(after.preview === before.preview || after.preview === null, 'preview response must remain Snapshot-owned')
        return evidence(projectId, after, ['preview_no_version_write', 'snapshot_version_unchanged'])
      } finally { await page.close() }
    })

    await runScenario(results, 'T002-09-commit-editable-agent-asset', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        await ensureLegacyConversion(page, agentBaseUrl, projectId)
        const before = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        if (before.active_design?.source !== 'agent_asset') {
          await sendBrief(page, '设计一辆双座冰原探索汽车，完整封闭车身，作为非功能展示模型。')
          await page.getByLabel('Agent 完整外观方向').waitFor({ timeout: TIMEOUT_MS })
          await page.getByLabel('Agent 完整外观方向').getByRole('button').first().click()
          await page.getByLabel('分件候选').waitFor({ timeout: TIMEOUT_MS })
        }
        await ensureLegacyConversion(page, agentBaseUrl, projectId)
        const saveButton = page.getByRole('button', { name: '保存为可编辑模型', exact: true })
        await saveButton.waitFor({ timeout: TIMEOUT_MS })
        const saveResponse = page.waitForResponse((response) => response.request().method() === 'POST' && response.url().includes('/agent/blockouts:commit'))
        await saveButton.click()
        const saveResult = await saveResponse.catch(() => null)
        try {
          await page.getByLabel('分件候选').getByText(/可编辑资产 v\d+/, { exact: false }).waitFor({ timeout: TIMEOUT_MS })
        } catch (error) {
          const bodyText = await page.locator('body').innerText().catch(() => '')
          const responseText = saveResult ? `${saveResult.status()} ${saveResult.url()}` : 'no save response'
          throw new Error(`editable asset did not appear (${responseText}): ${error instanceof Error ? error.message : String(error)}\n${bodyText.slice(0, 3000)}`)
        }
        const snapshot = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(snapshot.active_design?.source === 'agent_asset', 'commit must activate Agent asset')
        assert(snapshot.export?.source_version_id === snapshot.active_design?.asset_version_id, 'export source must follow active asset')
        return evidence(projectId, snapshot, ['editable_asset_commit', 'export_snapshot_alignment'])
      } finally { await page.close() }
    })

    await runScenario(results, 'T002-10-part-selection-and-material-zone', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        await page.getByLabel('分件候选').waitFor({ timeout: TIMEOUT_MS })
        // A selected display-only showcase detail deliberately has no editable
        // parameters.  This scenario exercises the bounded ChangeSet path, so
        // choose an actual server-declared adjustable part rather than relying
        // on list order across the four domain packs.
        const firstPart = page.getByLabel('分件候选').locator('.agent-segmentation-list button').filter({ hasText: '可调整' }).first()
        await firstPart.waitFor({ timeout: TIMEOUT_MS })
        const selectionResponse = page.waitForResponse((response) => response.url().includes('/active-design:select') && response.request().method() === 'POST')
        await firstPart.click()
        assert((await (await selectionResponse).json()).selected_part_id, 'selection response must include selected_part_id')
        const materials = page.getByLabel('视觉材质目录')
        await materials.waitFor({ timeout: TIMEOUT_MS })
        await materials.getByRole('button', { name: '拉丝铝', exact: true }).click()
        const snapshot = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(typeof snapshot.selected_part_id === 'string', 'Snapshot must own selected part')
        return evidence(projectId, snapshot, ['part_selection_snapshot', 'material_zone_action'])
      } finally { await page.close() }
    })

    await runScenario(results, 'T002-11-changeset-preview-cancel', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        await page.getByLabel('分件候选').waitFor({ timeout: TIMEOUT_MS })
        await page.getByLabel('部件可调参数').getByRole('button', { name: '减小 长度比例', exact: true }).click()
        const preview = page.getByLabel('可编辑资产修改预览')
        await preview.waitFor({ timeout: TIMEOUT_MS })
        const withPreview = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(withPreview.preview?.change_set_id, 'preview must be persisted in Snapshot')
        const rejectResponse = page.waitForResponse((response) => response.url().includes('/agent/change-sets/') && response.url().endsWith(':reject') && response.request().method() === 'POST')
        await preview.getByRole('button', { name: '取消修改', exact: true }).click()
        assert((await rejectResponse).ok(), 'changeset rejection request must succeed')
        await preview.waitFor({ state: 'detached', timeout: TIMEOUT_MS })
        const afterCancel = await waitForSnapshot(agentBaseUrl, projectId, (snapshot) => snapshot.preview == null)
        assert(afterCancel.preview === null, 'cancel must clear preview without creating a version')
        return evidence(projectId, afterCancel, ['changeset_preview', 'cancel_clears_preview'])
      } finally { await page.close() }
    })

    await runScenario(results, 'T002-12-confirm-quality-export-reload', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        await page.getByLabel('分件候选').waitFor({ timeout: TIMEOUT_MS })
        const before = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        await page.getByLabel('部件可调参数').getByRole('button', { name: '增大 长度比例', exact: true }).click()
        const preview = page.getByLabel('可编辑资产修改预览')
        await preview.waitFor({ timeout: TIMEOUT_MS })
        await preview.getByRole('button', { name: '保留并创建新版本', exact: true }).click()
        await page.getByLabel('分件候选').getByText(/可编辑资产 v\d+/, { exact: false }).waitFor({ timeout: TIMEOUT_MS })
        const committed = await waitForSnapshot(agentBaseUrl, projectId, (snapshot) => snapshot.active_design?.asset_version_id !== before.active_design?.asset_version_id)
        assert(committed.active_design?.asset_version_id !== before.active_design?.asset_version_id, 'confirm must create a new immutable version')
        await page.getByRole('button', { name: '检查', exact: true }).click()
        const quality = page.locator('.quality-drawer')
        await quality.waitFor({ timeout: TIMEOUT_MS })
        await assertText(quality, ['模型检查', '当前版本'])
        const qualityResponse = page.waitForResponse((response) => /\/api\/v1\/agent\/asset-versions\/[^/]+:quality$/.test(response.url()) && response.request().method() === 'POST')
        await quality.getByRole('button', { name: '检查当前 Agent 资产', exact: true }).click()
        const qualityPayload = await (await qualityResponse).json()
        assert(['passed', 'warning'].includes(qualityPayload.status), `quality check returned unexpected status: ${JSON.stringify(qualityPayload)}`)
        const afterQuality = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(afterQuality.quality?.asset_version_id === afterQuality.active_design?.asset_version_id, 'quality must reference active version')
        await quality.getByRole('button', { name: '关闭模型检查', exact: true }).click()
        await quality.waitFor({ state: 'detached', timeout: TIMEOUT_MS })
        await page.getByRole('button', { name: '导出', exact: true }).click()
        const exportDrawer = page.locator('.export-drawer')
        await exportDrawer.waitFor({ timeout: TIMEOUT_MS })
        await assertText(exportDrawer, ['选择你现在需要的内容', '下载 3D 模型 (GLB)', '概念视图'])
        assert(await exportDrawer.getByText('交给三维设计师', { exact: true }).count() === 0, 'Agent export drawer must not show legacy export choices')
        const renderResponse = page.waitForResponse((response) => /\/api\/v1\/agent\/asset-versions\/[^/]+:render\?/.test(response.url()) && response.request().method() === 'GET')
        await exportDrawer.getByRole('button', { name: '生成概念图', exact: true }).click()
        assert((await renderResponse).ok(), 'agent concept render request must succeed')
        await exportDrawer.locator('.agent-concept-view-card').nth(4).waitFor({ timeout: TIMEOUT_MS })
        await assertText(exportDrawer, ['爆炸概念图', '透明背景'])
        const packageResponse = page.waitForResponse((response) => /\/api\/v1\/agent\/asset-versions\/[^/]+:render-package\?/.test(response.url()) && response.request().method() === 'GET')
        const packageDownload = page.waitForEvent('download')
        await exportDrawer.getByRole('button', { name: '下载概念图包', exact: true }).click()
        assert((await packageResponse).ok(), 'agent concept render package request must succeed')
        const packageArtifact = await packageDownload
        assert(packageArtifact.suggestedFilename().endsWith('-concept-views.zip'), 'concept package must download a bounded ZIP')
        const glbDownload = page.waitForEvent('download')
        await exportDrawer.getByRole('button', { name: '下载 3D 模型 (GLB)', exact: true }).click()
        assert((await glbDownload).suggestedFilename().endsWith('.glb'), 'Agent drawer must download a GLB directly')
        const beforeReloadCanvas = await page.locator('.weapon-viewport canvas').count()
        await page.reload({ waitUntil: 'networkidle' })
        await page.getByLabel('分件候选').waitFor({ timeout: TIMEOUT_MS })
        const afterReload = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(await page.locator('.weapon-viewport canvas').count() === 1 && beforeReloadCanvas === 1, 'reload must preserve one WebGL canvas')
        assert(afterReload.active_design?.asset_version_id === afterQuality.active_design?.asset_version_id, 'reload must restore active asset')
        return evidence(projectId, afterReload, ['immutable_confirm', 'quality_alignment', 'direct_glb_download', 'render_package_download', 'reload_restore', 'single_canvas'])
      } finally { await page.close() }
    })

    const failed = results.filter((item) => item.status === 'failed')
    const report = { ok: failed.length === 0, suite: 'FGC-T002', scenario_count: results.length, project_id: projectId, results }
    await writeFile(join(OUTPUT_DIR, 'report.json'), `${JSON.stringify(report, null, 2)}\n`)
    if (failed.length) {
      console.error(JSON.stringify(report, null, 2))
      process.exitCode = 1
      return
    }
    console.log(JSON.stringify(report, null, 2))
  } finally {
    if (browser) await browser.close()
    await Promise.all(processes.reverse().map(stopProcess))
    await rm(tempRoot, { recursive: true, force: true })
  }
}

async function runScenario(results, id, fn) {
  const startedAt = new Date().toISOString()
  try {
    const evidence = await fn()
    const result = { id, status: 'passed', started_at: startedAt, ...evidence }
    results.push(result)
    await writeFile(join(OUTPUT_DIR, `${id}.json`), `${JSON.stringify(result, null, 2)}\n`)
  } catch (error) {
    const result = { id, status: 'failed', started_at: startedAt, error: error instanceof Error ? error.message : String(error) }
    results.push(result)
    await writeFile(join(OUTPUT_DIR, `${id}.json`), `${JSON.stringify(result, null, 2)}\n`)
  }
}

async function openWorkbench(context, baseUrl) {
  const page = await context.newPage()
  await page.goto(`${baseUrl}/#/cad`, { waitUntil: 'networkidle' })
  await page.waitForSelector('[data-testid="cad-workbench"]', { timeout: TIMEOUT_MS })
  await page.getByPlaceholder('描述你想设计的道具…').waitFor({ timeout: TIMEOUT_MS })
  await page.waitForFunction(() => {
    const button = document.querySelector('button[aria-label="发送设计需求"]')
    return button instanceof HTMLButtonElement && !button.disabled
  }, undefined, { timeout: TIMEOUT_MS })
  return page
}

async function sendBrief(page, brief) {
  const input = page.getByPlaceholder('描述你想设计的道具…')
  await input.fill(brief)
  await page.getByRole('button', { name: '发送设计需求', exact: true }).click()
}

async function ensureLegacyConversion(page, agentBaseUrl, projectId) {
  const notice = page.getByLabel('旧版设计转换')
  if (await notice.count() === 1 && await notice.isVisible()) {
    const conversionResponse = page.waitForResponse((response) => response.url().includes('/active-design:convert-legacy') && response.request().method() === 'POST')
    await notice.getByRole('button', { name: '让 Agent 重建可编辑资产', exact: true }).click()
    const response = await conversionResponse
    assert(response.ok(), `legacy conversion handoff failed: ${response.status()}`)
    await page.getByText(/已准备 legacy 只读设计的 Agent 重建输入/).waitFor({ timeout: TIMEOUT_MS })
  }
  // A fresh browser page has no in-memory conversion flag. Re-assert the
  // durable server intent before committing a candidate so this scenario does
  // not depend on the previous page's React state.
  const snapshot = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
  if (snapshot.active_design?.source === 'legacy_concept_read_only') {
    const response = await fetch(`${agentBaseUrl}/api/v1/projects/${projectId}/active-design:convert-legacy`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'Idempotency-Key': `t002-legacy-${Date.now()}` },
      body: JSON.stringify({ client_request_id: `t002-legacy-${Date.now()}`, snapshot_revision: snapshot.revision }),
      signal: AbortSignal.timeout(5_000),
    })
    if (!response.ok) throw new Error(`durable legacy conversion handoff failed: ${response.status} ${await response.text()}`)
  }
}

function evidence(projectId, snapshotResponse, assertions) {
  const body = snapshotResponse.body ?? snapshotResponse
  return {
    project_id: projectId,
    active_asset_version_id: body.active_design?.asset_version_id ?? null,
    snapshot_revision: body.revision ?? body.snapshot?.revision ?? null,
    assertions,
  }
}

function assert(condition, message) {
  if (!condition) throw new Error(message)
}

async function assertText(locator, expected) {
  const text = await locator.innerText()
  for (const phrase of expected) assert(text.includes(phrase), `missing text ${phrase}: ${text}`)
}

async function waitForProjectId(baseUrl) {
  const deadline = Date.now() + TIMEOUT_MS
  while (Date.now() < deadline) {
    const response = await requestStatus(baseUrl, '/api/v1/projects')
    const id = response.body?.items?.[0]?.project_id
    if (response.status === 200 && id) return id
    await sleep(200)
  }
  throw new Error('workbench did not create a starter project')
}

async function requestStatus(baseUrl, path) {
  const response = await fetch(`${baseUrl}${path}`, { signal: AbortSignal.timeout(5_000) })
  let body = null
  try { body = await response.json() } catch {}
  return { status: response.status, body }
}

async function jsonRequest(baseUrl, path) {
  const response = await fetch(`${baseUrl}${path}`, { signal: AbortSignal.timeout(5_000) })
  const body = await response.json()
  if (!response.ok) throw new Error(`${response.status} ${path}: ${JSON.stringify(body)}`)
  return body
}

async function waitForSnapshot(baseUrl, projectId, predicate) {
  const deadline = Date.now() + TIMEOUT_MS
  let latest = null
  while (Date.now() < deadline) {
    latest = await jsonRequest(baseUrl, `/api/v1/projects/${projectId}/active-design`)
    if (predicate(latest)) return latest
    await sleep(150)
  }
  throw new Error(`Snapshot did not reach expected state: ${JSON.stringify(latest)}`)
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
  const deadline = Date.now() + TIMEOUT_MS
  while (Date.now() < deadline) {
    if (child.exitCode !== null) throw new Error(`${label} exited with ${child.exitCode}`)
    try { if ((await fetch(url)).ok) return } catch {}
    await sleep(200)
  }
  throw new Error(`${label} did not become ready: ${url}`)
}

async function freePort() {
  const net = await import('node:net')
  return new Promise((resolvePort, reject) => {
    const server = net.createServer()
    server.once('error', reject)
    server.listen(0, '127.0.0.1', () => {
      const address = server.address()
      server.close(() => resolvePort(address.port))
    })
  })
}

function stopProcess(child) {
  if (!child || child.exitCode !== null) return Promise.resolve()
  return new Promise((resolveStop) => {
    const timer = setTimeout(() => { child.kill('SIGKILL'); resolveStop() }, 3_000)
    child.once('exit', () => { clearTimeout(timer); resolveStop() })
    child.kill('SIGTERM')
  })
}

function sleep(milliseconds) { return new Promise((resolveSleep) => setTimeout(resolveSleep, milliseconds)) }

main().catch((error) => {
  console.error(error instanceof Error ? error.stack : error)
  process.exitCode = 1
})
