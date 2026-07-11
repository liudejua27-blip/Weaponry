#!/usr/bin/env node
import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { copyFile, mkdir, mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, isAbsolute, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const OUTPUT_DIR = join(ROOT, 'output', 'playwright')
const SCREENSHOT = join(OUTPUT_DIR, 'r3-concept-workbench.png')
const MIRROR_SCREENSHOT = join(OUTPUT_DIR, 'r3-concept-mirror.png')
const PREVIEW_RENDER = join(OUTPUT_DIR, 'r5-concept-preview.png')
const EXPLODED_RENDER = join(OUTPUT_DIR, 'r5-concept-exploded.png')
const FRONT_RENDER = join(OUTPUT_DIR, 'r5-concept-front.png')
const TOP_RENDER = join(OUTPUT_DIR, 'r5-concept-top.png')
const TURNTABLE_RENDER = join(OUTPUT_DIR, 'r5-concept-turntable-000.png')
const QUALITY_HIGHLIGHT_SCREENSHOT = join(OUTPUT_DIR, 'r5-quality-triangle-highlight.png')
const PLANNER_SCREENSHOT = join(OUTPUT_DIR, 'r4-concept-planner-variants.png')
const CHANGE_PLANNER_SCREENSHOT = join(OUTPUT_DIR, 'r4-change-planner-ghost-preview.png')
const AUDIT_EXPORT_SCREENSHOT = join(OUTPUT_DIR, 'r3-change-set-audit-export.png')
const DCC_COMBINED_OUTPUT = process.env.FORGECAD_DCC_COMBINED_OUTPUT ?? null
const REQUIRE_BROWSER_DOWNLOADS = process.env.FORGECAD_REQUIRE_BROWSER_DOWNLOADS !== '0'
const SMOKE_TIMEOUT_MS = Number(process.env.FORGECAD_WORKBENCH_SMOKE_TIMEOUT_MS ?? 180_000)
let smokeStage = 'initializing'

async function main() {
  const tempRoot = await mkdtemp(join(tmpdir(), 'forgecad_r3_workbench_'))
  const libraryRoot = join(tempRoot, 'ForgeCADLibrary')
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
          WUSHEN_LOCAL_WORKER_ENABLED: '0',
          FORGECAD_CONCEPT_PLANNER_PROVIDER: 'deterministic_rules',
        },
        stdio: ['ignore', 'pipe', 'pipe'],
      },
    )
    processes.push(agent)
    await waitForHttp(`${agentBaseUrl}/api/health`, agent, 'agent health')
    smokeStage = 'seeding Concept Project and Module Pack'
    const seeded = await seedConceptGraph(agentBaseUrl)

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
    await waitForHttp(viteBaseUrl, vite, 'vite frontend')
    smokeStage = 'running browser workbench flow'
    const result = await runWorkbenchUi(viteBaseUrl, agentBaseUrl, seeded)
    smokeStage = 'stopping initial agent'
    await stopProcess(agent)
    const restartPort = await freePort()
    const restartBaseUrl = `http://127.0.0.1:${restartPort}`
    const restartedAgent = spawn(
      join(ROOT, '.venv', 'bin', 'python'),
      ['-m', 'uvicorn', 'wushen_agent.main:create_app', '--factory', '--host', '127.0.0.1', '--port', String(restartPort)],
      {
        cwd: ROOT,
        env: {
          ...process.env,
          WUSHEN_LIBRARY_ROOT: libraryRoot,
          WUSHEN_MIGRATIONS_DIR: join(ROOT, 'migrations'),
          WUSHEN_LOCAL_WORKER_ENABLED: '0',
          FORGECAD_CONCEPT_PLANNER_PROVIDER: 'deterministic_rules',
        },
        stdio: ['ignore', 'pipe', 'pipe'],
      },
    )
    processes.push(restartedAgent)
    await waitForHttp(`${restartBaseUrl}/api/health`, restartedAgent, 'restarted agent health')
    smokeStage = 'verifying persistence after agent restart'
    const restartVerified = await verifyReplacement(
      restartBaseUrl,
      seeded.project_id,
      result.change_set_audit_export,
    )
    smokeStage = 'writing result'
    console.log(JSON.stringify({ ok: true, ...seeded, ...result, restart_verified: restartVerified }, null, 2))
  } finally {
    smokeStage = 'cleaning child processes and temporary library'
    await Promise.all(processes.reverse().map(stopProcess))
    await rm(tempRoot, { recursive: true, force: true })
  }
}

async function seedConceptGraph(baseUrl) {
  const project = await jsonRequest(baseUrl, '/api/v1/projects', {
    method: 'POST',
    idempotencyKey: 'r3-ui-project',
    body: {
      client_request_id: 'r3-ui-project',
      name: '寒地巡逻 S1',
      intended_uses: ['game_asset', 'film_prop', 'non_functional_display'],
      style: {
        keywords: ['寒地', '工业', '紧凑', '硬表面'],
        palette: ['graphite', 'gunmetal', 'signal_red'],
        detail_density: 0.68,
      },
      proportions: { overall_length_mm: 230, body_height_mm: 54, grip_angle_deg: 15 },
      constraints: { symmetry: 'mostly_symmetric', max_triangle_count: 180000 },
      assumptions: ['非功能性概念模型，不用于真实制造或使用'],
    },
  })

  const packRoot = join(ROOT, 'assets', 'module-packs', 'weapon-concept-v1-reference')
  const pack = JSON.parse(await readFile(join(packRoot, 'pack.json'), 'utf8'))
  for (const entry of pack.modules) {
    const manifest = JSON.parse(await readFile(join(packRoot, entry.manifest_path), 'utf8'))
    const glb = await readFile(join(packRoot, entry.glb_path))
    const thumbnail = await readFile(join(packRoot, entry.thumbnail_path))
    await jsonRequest(baseUrl, '/api/v1/module-assets', {
      method: 'POST',
      idempotencyKey: `r3-ui-${manifest.module_id}`,
      body: {
        client_request_id: `r3-ui-${manifest.module_id}`,
        logical_path: `packs/weapon-concept/${manifest.module_id}.glb`,
        glb_data_base64: glb.toString('base64'),
        thumbnail_png_base64: thumbnail.toString('base64'),
        manifest,
      },
    })
  }

  const graph = {
    schema_version: 'ModuleGraph@1',
    graph_id: 'mg_r3_ui_arctic_patrol',
    project_id: project.project_id,
    root_node_id: 'node_core',
    nodes: [
      graphNode('node_core', 'module_core_shell_01', [0, 0, 0], true),
      graphNode('node_front', 'module_front_shell_01', [-50, 0, 0]),
      graphNode('node_rear', 'module_rear_shell_01', [50, 0, 0]),
      graphNode('node_grip', 'module_grip_shell_01', [14, -24, 0]),
      graphNode('node_top', 'module_top_accessory_01', [0, 24, 0]),
      graphNode('node_side', 'module_side_accessory_01', [0, 0, 20]),
      graphNode('node_lower', 'module_lower_structure_01', [-12, -24, 0]),
      graphNode('node_storage', 'module_storage_visual_01', [30, -24, 0]),
      graphNode('node_armor', 'module_armor_panel_01', [0, 0, -20]),
    ],
    edges: [
      {
        edge_id: 'edge_core_front',
        from_node_id: 'node_core',
        from_connector_id: 'connector_core_front',
        to_node_id: 'node_front',
        to_connector_id: 'connector_front_01_core',
        status: 'connected',
      },
      {
        edge_id: 'edge_core_grip',
        from_node_id: 'node_core',
        from_connector_id: 'connector_core_grip',
        to_node_id: 'node_grip',
        to_connector_id: 'connector_grip_core',
        status: 'connected',
      },
      graphEdge('rear', 'connector_core_rear', 'connector_rear_core'),
      graphEdge('top', 'connector_core_top', 'connector_top_core'),
      graphEdge('side', 'connector_core_side', 'connector_side_core'),
      graphEdge('lower', 'connector_core_lower', 'connector_lower_core'),
      graphEdge('storage', 'connector_core_storage', 'connector_storage_core'),
      graphEdge('armor', 'connector_core_armor', 'connector_armor_core'),
    ],
  }
  await jsonRequest(baseUrl, `/api/v1/module-graphs/${graph.graph_id}/validate`, {
    method: 'POST',
    idempotencyKey: 'r3-ui-graph',
    body: { client_request_id: 'r3-ui-graph', graph, persist: true },
  })
  const bound = await jsonRequest(baseUrl, `/api/v1/projects/${project.project_id}/versions`, {
    method: 'POST',
    idempotencyKey: 'r3-ui-bind-version',
    body: {
      client_request_id: 'r3-ui-bind-version',
      parent_version_id: project.current_version_id,
      summary: '绑定首个可交互 ModuleGraph。',
      spec: project.current_spec,
      module_graph_id: graph.graph_id,
    },
  })
  return {
    project_id: project.project_id,
    version_id: bound.current_version_id,
    graph_id: graph.graph_id,
    module_count: pack.modules.length,
  }
}

async function runWorkbenchUi(baseUrl, agentApiBaseUrl, seeded) {
  const browser = await launchSystemBrowser()
  const context = await browser.newContext({
    acceptDownloads: true,
    viewport: { width: 1536, height: 1024 },
    deviceScaleFactor: 1,
  })
  const page = await context.newPage()
  const cdpSession = await context.newCDPSession(page)
  await cdpSession.send('Browser.setDownloadBehavior', {
    behavior: 'allow',
    downloadPath: OUTPUT_DIR,
    eventsEnabled: true,
  })
  const browserErrors = []
  let conceptExportPosts = 0
  page.on('pageerror', (error) => browserErrors.push(error.message))
  page.on('request', (request) => {
    if (request.method() === 'POST' && /\/api\/v1\/versions\/[^/]+\/exports$/.test(request.url())) {
      conceptExportPosts += 1
    }
  })
  try {
    await mkdir(OUTPUT_DIR, { recursive: true })
    await page.goto(`${baseUrl}/#/cad`, { waitUntil: 'networkidle' })
    await page.waitForSelector('[data-testid="cad-workbench"]', { timeout: 20_000 })
    await page.waitForFunction(
      () => document.querySelector('.cad-left-rail')?.textContent?.includes('寒地巡逻 S1'),
      { timeout: 20_000 },
    )
    await assertText(page.locator('.cad-left-rail'), ['寒地巡逻 S1', '绑定首个可交互 ModuleGraph。'])
    await page.waitForSelector('.viewport-data-state', { state: 'detached', timeout: 20_000 })
    await assertText(page.locator('.component-library'), [
      'Core Shell 01',
      'Front Shell 01',
      'Grip Shell 01',
      'Front Shell 02',
      '待审',
    ])
    await page.waitForFunction(() => Array.from(
      document.querySelectorAll('.component-card img'),
    ).some((image) => image.complete && image.naturalWidth > 0), { timeout: 20_000 })
    await assertText(page.locator('.cad-status-bar'), ['9 nodes', '单位：mm'])

    await page.getByRole('button', { name: /Front Shell 01/ }).click()
    await assertText(page.locator('.cad-status-bar'), ['node_front'])
    await assertText(page.locator('.component-grid'), ['当前节点'])
    await page.waitForSelector('[data-testid="component-inspector"]', { timeout: 20_000 })
    await assertText(page.locator('[data-testid="component-inspector"]'), ['本人原创声明', '等待独立审阅'])
    await page.locator('.component-categories').getByRole('button', { name: /可替换/ }).click()
    await assertText(page.locator('.component-grid'), ['Front Shell 01', 'Front Shell 02'])
    if ((await page.locator('.component-card').count()) !== 2) {
      throw new Error('compatible component filter did not narrow to the selected category')
    }
    await page.locator('.component-categories').getByRole('button', { name: /全部组件/ }).click()
    const inspectorValues = await page.locator('.properties-panel .wide-field input').evaluateAll(
      (inputs) => inputs.map((input) => input.value),
    )
    if (inspectorValues[0] !== 'node_front' || inspectorValues[1] !== 'module_front_shell_01') {
      throw new Error(`inspector selection mismatch: ${JSON.stringify(inspectorValues)}`)
    }
    await page.getByRole('button', { name: '连接' }).click()
    await assertText(page.locator('.properties-panel'), ['front.core', '已连接'])

    await page.getByRole('button', { name: /Front Shell 02/ }).dragTo(
      page.locator('.weapon-viewport canvas'),
      { targetPosition: { x: 28, y: 28 } },
    )
    await assertText(page.locator('.module-replace-bar'), ['节点：node_front', '候选：Front Shell 02'])
    const replaceButton = page.getByRole('button', { name: '预览替换' }).first()
    if (await replaceButton.isDisabled()) throw new Error('compatible replacement action was disabled')
    await replaceButton.click()
    await page.waitForSelector('[data-testid="component-replacement-preview"]', { timeout: 20_000 })
    await assertText(page.locator('[data-testid="component-replacement-preview"]'), ['幽灵预览已就绪', '确认并创建新版本'])
    const confirmResponsePromise = page.waitForResponse(
      (response) => response.url().includes('/api/v1/change-sets/') && response.url().endsWith(':confirm'),
    )
    await page.locator('[data-testid="component-replacement-preview"]').getByRole('button', { name: '确认并创建新版本' }).click()
    const confirmResponse = await confirmResponsePromise
    if (!confirmResponse.ok()) throw new Error(`ChangeSet confirm failed: ${confirmResponse.status()}`)
    await page.waitForFunction(
      () => document.querySelector('.concept-runtime-state')?.textContent?.includes('替换已确认并创建新版本'),
      { timeout: 20_000 },
    )
    await assertText(page.locator('.cad-left-rail'), ['V3', 'ChangeSet: change_desktop_replace_'])
    await assertText(page.locator('.cad-status-bar'), ['node_front'])
    await page.getByRole('button', { name: '参数', exact: true }).click()
    const inspectorValuesAfterReplace = await page.locator('.properties-panel .wide-field input').evaluateAll(
      (inputs) => inputs.map((input) => input.value),
    )
    if (inspectorValuesAfterReplace[1] !== 'module_front_shell_02') {
      throw new Error(`replacement was not reflected in inspector: ${JSON.stringify(inspectorValuesAfterReplace)}`)
    }
    const snappedPosition = await page.locator('.properties-panel .axis-group').first().locator('input').evaluateAll(
      (inputs) => inputs.map((input) => input.value),
    )
    if (JSON.stringify(snappedPosition.map(Number)) !== JSON.stringify([-50, 0, 0])) {
      throw new Error(`Connector snap was not reflected in inspector: ${JSON.stringify(snappedPosition)}`)
    }

    await page.getByRole('button', { name: '撤销', exact: true }).click()
    await page.waitForFunction(
      () => document.querySelector('.concept-runtime-state')?.textContent?.includes('已切换到 V2'),
      { timeout: 20_000 },
    )
    await page.getByRole('button', { name: '重做', exact: true }).click()
    await page.waitForFunction(
      () => document.querySelector('.concept-runtime-state')?.textContent?.includes('已切换到 V3'),
      { timeout: 20_000 },
    )
    await page.waitForFunction(
      () => {
        const canvas = document.querySelector('.weapon-viewport canvas')
        if (!canvas) return false
        const bounds = canvas.getBoundingClientRect()
        return bounds.width >= 400 && bounds.height >= 300
      },
      { timeout: 20_000 },
    )
    const canvas = page.locator('.weapon-viewport canvas')
    const canvasBox = await canvas.boundingBox()
    if (!canvasBox || canvasBox.width < 400 || canvasBox.height < 300) {
      throw new Error(`ModuleGraph canvas is not usable: ${JSON.stringify(canvasBox)}`)
    }
    await page.getByRole('button', { name: '测量', exact: true }).click()
    await page.getByTestId('measurement-overlay').waitFor()
    await page.mouse.click(canvasBox.x + canvasBox.width * 0.5, canvasBox.y + canvasBox.height * 0.46)
    await assertText(page.getByTestId('measurement-overlay'), ['点击模型设置终点'])
    await page.mouse.click(canvasBox.x + canvasBox.width * 0.57, canvasBox.y + canvasBox.height * 0.46)
    await assertText(page.getByTestId('measurement-overlay'), ['点到点：', 'mm', '固定标注', '清除'])
    await assertText(page.locator('.viewport-readout'), ['测量：', 'mm'])
    await page.getByTestId('measurement-overlay').getByRole('button', { name: '固定标注' }).click()
    await assertText(page.getByTestId('measurement-annotations'), ['标注 1', 'mm'])
    await assertText(page.getByTestId('measurement-overlay'), ['点击模型设置起点'])
    await page.getByTestId('measurement-overlay').getByRole('button', { name: '法线夹角' }).click()
    await page.mouse.click(canvasBox.x + canvasBox.width * 0.5, canvasBox.y + canvasBox.height * 0.46)
    await page.mouse.click(canvasBox.x + canvasBox.width * 0.57, canvasBox.y + canvasBox.height * 0.46)
    await assertText(page.getByTestId('measurement-overlay'), ['表面法线夹角：', '°'])
    await page.getByRole('button', { name: '选择', exact: true }).click()
    await page.evaluate(() => window.scrollTo(0, 0))
    await page.screenshot({ path: SCREENSHOT, fullPage: true })
    if ((await stat(SCREENSHOT)).size < 20_000) throw new Error('R3 workbench screenshot is unexpectedly small')

    await page.getByRole('button', { name: 'Connector' }).click()
    await page.getByRole('button', { name: '隐藏' }).click()
    await page.getByRole('button', { name: '显示' }).click()
    await page.getByRole('button', { name: '聚焦' }).click()
    await page.getByRole('button', { name: '爆炸视图' }).click()
    await page.waitForSelector('.viewport-viewbar button.active[aria-label="爆炸视图"]')
    await page.getByRole('button', { name: '爆炸视图' }).click()
    await page.getByRole('button', { name: /Grip Shell 01/ }).click()
    const mirrorConfirmPromise = page.waitForResponse(
      (response) => response.url().includes('/api/v1/change-sets/') && response.url().endsWith(':confirm'),
    )
    await page.getByRole('button', { name: 'X 镜像' }).click()
    const mirrorConfirmResponse = await mirrorConfirmPromise
    if (!mirrorConfirmResponse.ok()) throw new Error(`mirror ChangeSet confirm failed: ${mirrorConfirmResponse.status()}`)
    await page.waitForFunction(
      () => document.querySelector('.concept-runtime-state')?.textContent?.includes('镜像已确认并创建新版本'),
      { timeout: 20_000 },
    )
    await assertText(page.locator('.cad-left-rail'), ['V4', 'ChangeSet: change_desktop_mirror_'])
    const mirrorInspectorValues = await page.locator('.properties-panel .wide-field input').evaluateAll(
      (inputs) => inputs.map((input) => input.value),
    )
    if (!mirrorInspectorValues.includes('node_grip') || !mirrorInspectorValues.includes('x')) {
      throw new Error(`mirror state was not reflected in inspector: ${JSON.stringify(mirrorInspectorValues)}`)
    }
    await page.getByRole('button', { name: '取消镜像' }).waitFor()
    await page.screenshot({ path: MIRROR_SCREENSHOT, fullPage: true })
    if ((await stat(MIRROR_SCREENSHOT)).size < 20_000) throw new Error('mirror screenshot is unexpectedly small')

    await page.getByRole('button', { name: '移动', exact: true }).click()
    await assertText(page.getByTestId('transform-command-controls'), ['世界坐标', '吸附：1 mm / 15°'])
    const transformAxis = page.locator('.properties-panel .axis-group').first().locator('input').first()
    await transformAxis.fill('17')
    const transformProposePromise = page.waitForResponse(
      (response) => /\/api\/v1\/versions\/[^/]+\/change-sets$/.test(response.url())
        && response.request().method() === 'POST',
    )
    const transformPreviewPromise = page.waitForResponse(
      (response) => response.url().includes('/api/v1/change-sets/')
        && response.url().endsWith(':preview')
        && response.request().method() === 'POST',
    )
    await page.getByRole('button', { name: '预览变换', exact: true }).click()
    const [transformProposeResponse, transformPreviewResponse] = await Promise.all([
      transformProposePromise,
      transformPreviewPromise,
    ])
    if (!transformProposeResponse.ok() || !transformPreviewResponse.ok()) {
      throw new Error(`transform ChangeSet preview failed: propose=${transformProposeResponse.status()} preview=${transformPreviewResponse.status()}`)
    }
    const transformChange = await transformProposeResponse.json()
    if (transformChange.operations?.[0]?.op !== 'set_transform' || transformChange.operations?.[0]?.transform?.position?.[0] !== 17) {
      throw new Error(`transform ChangeSet payload mismatch: ${JSON.stringify(transformChange)}`)
    }
    await page.getByTestId('manual-transform-preview').waitFor()
    await assertText(page.getByTestId('manual-transform-preview'), ['变换幽灵预览', '变换 node_grip', '确认并创建新版本'])
    const transformConfirmPromise = page.waitForResponse(
      (response) => response.url().includes('/api/v1/change-sets/') && response.url().endsWith(':confirm'),
    )
    await page.getByTestId('manual-transform-preview').getByRole('button', { name: '确认并创建新版本' }).click()
    const transformConfirmResponse = await transformConfirmPromise
    if (!transformConfirmResponse.ok()) throw new Error(`transform ChangeSet confirm failed: ${transformConfirmResponse.status()}`)
    await page.waitForFunction(
      () => document.querySelector('.concept-runtime-state')?.textContent?.includes('变换已确认并创建新版本'),
      { timeout: 20_000 },
    )
    await assertText(page.locator('.cad-left-rail'), ['V5', 'ChangeSet: change_desktop_transform_'])
    const committedTransformPosition = await page.locator('.properties-panel .axis-group').first().locator('input').first().inputValue()
    if (committedTransformPosition !== '17') {
      throw new Error(`transform was not reflected in inspector after confirm: ${committedTransformPosition}`)
    }

    await page.getByRole('button', { name: '连接', exact: true }).click()
    const connectorSnapProposePromise = page.waitForResponse(
      (response) => response.url().includes('change-sets:connector-snap')
        && response.request().method() === 'POST',
    )
    const connectorSnapPreviewPromise = page.waitForResponse(
      (response) => response.url().includes('/api/v1/change-sets/')
        && response.url().endsWith(':preview')
        && response.request().method() === 'POST',
    )
    await page.getByRole('button', { name: '修复并预览吸附', exact: true }).click()
    const [connectorSnapProposeResponse, connectorSnapPreviewResponse] = await Promise.all([
      connectorSnapProposePromise,
      connectorSnapPreviewPromise,
    ])
    if (!connectorSnapProposeResponse.ok() || !connectorSnapPreviewResponse.ok()) {
      throw new Error(`connector snap preview failed: propose=${connectorSnapProposeResponse.status()} preview=${connectorSnapPreviewResponse.status()}`)
    }
    const connectorSnapChange = await connectorSnapProposeResponse.json()
    if (connectorSnapChange.operations?.[0]?.op !== 'set_transform' || connectorSnapChange.operations?.[0]?.node_id !== 'node_grip') {
      throw new Error(`connector snap ChangeSet payload mismatch: ${JSON.stringify(connectorSnapChange)}`)
    }
    await assertText(page.getByTestId('manual-transform-preview'), ['Connector 吸附预览', 'Snap node_grip', '确认并创建新版本'])
    await page.getByTestId('manual-transform-preview').getByRole('button', { name: '放弃预览' }).click()
    await page.waitForFunction(
      () => document.querySelector('.concept-runtime-state')?.textContent?.includes('已放弃变换预览'),
      { timeout: 20_000 },
    )

    const lifecycle = await stressViewportLifecycle(page)
    const timelineFixture = await seedTimelineAuditFixture(agentApiBaseUrl, seeded.project_id)
    await page.getByRole('button', { name: '时间线' }).click()
    await page.getByRole('button', { name: '重置' }).click()
    await page.waitForFunction(
      () => document.querySelectorAll('.timeline-item').length === 20,
      { timeout: 20_000 },
    )
    await page.getByRole('button', { name: '加载更多' }).click()
    await page.waitForFunction(
      (expected) => document.querySelectorAll('.timeline-item').length === expected,
      timelineFixture.total_count,
      { timeout: 20_000 },
    )
    await assertText(page.locator('.timeline-drawer'), [
      'ChangeSet 操作时间线',
      'replace_module(node_front)',
      'set_mirror(node_grip)',
      'confirmed',
    ])
    await page.getByPlaceholder('搜索 ChangeSet…').fill('ui_rejected_locked')
    await page.getByLabel('ChangeSet 状态筛选').selectOption('rejected')
    await page.getByRole('button', { name: '查询' }).click()
    await page.waitForFunction(
      () => document.querySelectorAll('.timeline-item').length === 1,
      { timeout: 20_000 },
    )
    await assertText(page.locator('[data-testid="change-set-diagnostic"]'), [
      'CHANGE_SET_INVALID',
      'preview',
      'Locked ModuleGraph node cannot be changed: node_core',
      'nodes: node_core',
    ])
    await page.getByPlaceholder('搜索 ChangeSet…').fill('')
    await page.getByLabel('ChangeSet 状态筛选').selectOption('')
    await page.getByLabel('ChangeSet 操作筛选').selectOption('set_mirror')
    const mirrorFilterResponsePromise = page.waitForResponse(
      (response) => response.url().includes('/change-sets')
        && response.url().includes('operation=set_mirror'),
    )
    await page.getByRole('button', { name: '查询' }).click()
    const mirrorFilterResponse = await mirrorFilterResponsePromise
    if (!mirrorFilterResponse.ok()) {
      throw new Error(`timeline operation filter failed: ${mirrorFilterResponse.status()}`)
    }
    await assertText(page.locator('.timeline-item'), ['set_mirror(node_grip)'])

    const briefResponsePromise = page.waitForResponse(
      (response) => response.url().endsWith('/brief:interpret')
        && response.request().method() === 'POST',
    )
    const variantsResponsePromise = page.waitForResponse(
      (response) => /\/api\/v1\/projects\/[^/]+\/variants$/.test(response.url())
        && response.request().method() === 'POST',
    )
    await page.getByPlaceholder('输入设计需求…').fill(
      '寒地工业、紧凑、精密细节、信号红点缀的非功能概念资产',
    )
    await page.getByRole('button', { name: '发送设计需求' }).click()
    const briefResponse = await briefResponsePromise
    const variantsResponse = await variantsResponsePromise
    if (!briefResponse.ok() || !variantsResponse.ok()) {
      throw new Error(`planner API failed: brief=${briefResponse.status()} variants=${variantsResponse.status()}`)
    }
    const briefRecord = await briefResponse.json()
    const variantRecords = await variantsResponse.json()
    if (
      briefRecord.interpreted_spec.proportions.overall_length_mm !== 207
      || briefRecord.interpreted_spec.style.detail_density !== 0.82
      || briefRecord.planner_provenance.generator !== 'deterministic_rules'
      || briefRecord.planner_provenance.fallback_used
    ) {
      throw new Error(`planner brief interpretation mismatch: ${JSON.stringify(briefRecord)}`)
    }
    const plannerParameterValues = await page.locator('.quick-parameters input').evaluateAll(
      (inputs) => inputs.map((input) => input.value),
    )
    if (JSON.stringify(plannerParameterValues) !== JSON.stringify(['207', '120', '15', '82'])) {
      throw new Error(`planner spec was not reflected in parameter UI: ${JSON.stringify(plannerParameterValues)}`)
    }
    if (
      variantRecords.items?.length !== 3
      || !variantRecords.items.every((variant) => (
        variant.recommended_module_ids?.length
        && variant.rationale?.length
        && variant.planner_provenance?.input_sha256?.length === 64
      ))
    ) {
      throw new Error(`planner variants missing provenance: ${JSON.stringify(variantRecords)}`)
    }
    await page.locator('[data-variant-rank]').first().waitFor()
    if (await page.locator('[data-variant-rank]').count() !== 3) {
      throw new Error('desktop planner did not render three variants')
    }
    const selectVariantResponsePromise = page.waitForResponse(
      (response) => response.url().endsWith(':select')
        && response.url().includes('/variants/')
        && response.request().method() === 'POST',
    )
    await page.locator('[data-variant-rank="2"]').click()
    const selectVariantResponse = await selectVariantResponsePromise
    if (!selectVariantResponse.ok()) {
      throw new Error(`variant selection failed: ${selectVariantResponse.status()}`)
    }
    await page.locator('[data-variant-rank="2"].selected').waitFor()
    await page.waitForFunction(
      () => document.querySelector('.concept-runtime-state')?.textContent?.includes('Planner 预览'),
      { timeout: 20_000 },
    )
    await page.locator('.weapon-viewport[data-load-state="ready"]').waitFor()
    await page.screenshot({ path: PLANNER_SCREENSHOT, fullPage: true })
    if ((await stat(PLANNER_SCREENSHOT)).size < 20_000) {
      throw new Error('planner variants screenshot is unexpectedly small')
    }

    await page.getByRole('button', { name: '修改预览', exact: true }).click()
    await page.getByPlaceholder('输入设计需求…').fill(
      '整体长度调整为 218 mm，细节密度调整为 84%',
    )
    const changePlanResponsePromise = page.waitForResponse(
      (response) => (response.url().includes('change-sets%3Aplan')
        || response.url().includes('change-sets:plan'))
        && response.request().method() === 'POST',
    )
    const changePreviewResponsePromise = page.waitForResponse(
      (response) => response.url().includes('/api/v1/change-sets/')
        && response.url().endsWith(':preview')
        && response.request().method() === 'POST',
    )
    await page.getByRole('button', { name: '生成修改预览', exact: true }).click()
    const changePlanResponse = await changePlanResponsePromise
    const changePreviewResponse = await changePreviewResponsePromise
    if (!changePlanResponse.ok() || !changePreviewResponse.ok()) {
      throw new Error(
        `Change Planner API failed: plan=${changePlanResponse.status()} preview=${changePreviewResponse.status()}`,
      )
    }
    const plannedChange = await changePlanResponse.json()
    const previewedChange = await changePreviewResponse.json()
    if (
      plannedChange.change_set.operations?.length !== 2
      || plannedChange.planner_provenance.generator !== 'deterministic_rules'
      || plannedChange.planner_provenance.fallback_used
      || previewedChange.preview_spec.proportions.overall_length_mm !== 218
      || previewedChange.preview_spec.style.detail_density !== 0.84
    ) {
      throw new Error(`Change Planner response mismatch: ${JSON.stringify(plannedChange)}`)
    }
    await page.locator('[data-testid="change-preview-card"]').waitFor()
    await page.locator('[data-testid="ghost-preview-badge"]').waitFor()
    await page.locator('.weapon-viewport[data-preview-mode="ghost"]').waitFor()
    await assertText(page.locator('[data-testid="change-preview-card"]'), [
      '幽灵预览 · 待确认',
      'proportions.overall_length_mm → 218',
      'style.detail_density → 0.84',
      '确认并创建新版本',
    ])
    const changePreviewParameterValues = await page.locator('.quick-parameters input').evaluateAll(
      (inputs) => inputs.map((input) => input.value),
    )
    if (JSON.stringify(changePreviewParameterValues) !== JSON.stringify(['218', '120', '15', '84'])) {
      throw new Error(`Change Planner preview was not reflected in parameter UI: ${JSON.stringify(changePreviewParameterValues)}`)
    }
    await page.screenshot({ path: CHANGE_PLANNER_SCREENSHOT, fullPage: true })
    if ((await stat(CHANGE_PLANNER_SCREENSHOT)).size < 20_000) {
      throw new Error('Change Planner ghost preview screenshot is unexpectedly small')
    }
    const changeConfirmResponsePromise = page.waitForResponse(
      (response) => response.url().includes('/api/v1/change-sets/')
        && response.url().endsWith(':confirm')
        && response.request().method() === 'POST',
    )
    await page.getByRole('button', { name: '确认并创建新版本', exact: true }).click()
    const changeConfirmResponse = await changeConfirmResponsePromise
    if (!changeConfirmResponse.ok()) {
      throw new Error(`Change Planner confirm failed: ${changeConfirmResponse.status()}`)
    }
    await page.waitForFunction(
      () => document.querySelector('.concept-runtime-state')?.textContent?.includes('AI 修改已确认并创建新版本'),
      { timeout: 20_000 },
    )
    await page.locator('.weapon-viewport[data-preview-mode="committed"]').waitFor()
    await assertText(page.locator('.cad-left-rail'), ['V6', 'ChangeSet: change_plan_'])
    await page.getByRole('button', { name: '时间线', exact: true }).click()
    await page.locator('[data-testid="change-set-planner-meta"]').first().waitFor()
    await assertText(page.locator('[data-testid="change-set-planner-meta"]').first(), [
      'planner · deterministic_concept_rules',
      '整体长度调整为 218 mm',
    ])

    await page.getByPlaceholder('搜索 ChangeSet…').fill('')
    await page.getByLabel('ChangeSet 状态筛选').selectOption('')
    await page.getByLabel('ChangeSet 操作筛选').selectOption('')
    const auditExportResponsePromise = page.waitForResponse(
      (response) => /\/api\/v1\/projects\/[^/]+\/change-set-audit-exports$/.test(response.url())
        && response.request().method() === 'POST',
    )
    const auditDownloadPromise = waitForDownload(page)
    await page.getByTestId('change-set-audit-export').click()
    const auditExportResponse = await auditExportResponsePromise
    if (!auditExportResponse.ok()) {
      throw new Error(`ChangeSet audit export failed: ${auditExportResponse.status()}`)
    }
    const auditExportRecord = await auditExportResponse.json()
    const expectedAuditCount = timelineFixture.total_count + 1
    if (
      auditExportRecord.record_count !== expectedAuditCount
      || auditExportRecord.retention_class !== 'project_lifetime'
      || auditExportRecord.manifest?.ordering !== 'updated_at_desc_change_set_id_desc'
    ) {
      throw new Error(`ChangeSet audit export record mismatch: ${JSON.stringify(auditExportRecord)}`)
    }
    const auditDownload = await resolveDownload(
      await auditDownloadPromise,
      `${agentApiBaseUrl}/api/v1/change-set-audit-exports/${auditExportRecord.audit_export_id}/file`,
      `${auditExportRecord.audit_export_id}.zip`,
    )
    if (!auditDownload.filename.startsWith('csaudit_')
      || !auditDownload.filename.endsWith('.zip')) {
      throw new Error(`unexpected ChangeSet audit filename: ${auditDownload.filename}`)
    }
    const auditDownloadPath = auditDownload.path
    if (!auditDownloadPath || (await stat(auditDownloadPath)).size < 500) {
      throw new Error('ChangeSet audit ZIP is unexpectedly small')
    }
    const auditBytes = await readFile(auditDownloadPath)
    const auditSha256 = createHash('sha256').update(auditBytes).digest('hex')
    if (
      auditBytes.subarray(0, 2).toString('ascii') !== 'PK'
      || auditSha256 !== auditExportRecord.package_sha256
    ) {
      throw new Error('ChangeSet audit ZIP header or hash mismatch')
    }
    await assertText(page.getByTestId('change-set-audit-summary'), [
      `${expectedAuditCount} 条`,
      auditExportRecord.audit_export_id,
      'project_lifetime',
    ])
    await page.screenshot({ path: AUDIT_EXPORT_SCREENSHOT, fullPage: true })
    if ((await stat(AUDIT_EXPORT_SCREENSHOT)).size < 20_000) {
      throw new Error('ChangeSet audit export screenshot is unexpectedly small')
    }

    await page.getByRole('button', { name: '连接' }).click()
    await assertText(page.locator('.properties-panel'), ['grip.core', '已连接'])
    await page.locator('.properties-panel').getByRole('button', { name: '检查', exact: true }).click()
    const qualityResponsePromise = page.waitForResponse(
      (response) => response.url().includes('quality-runs') && response.request().method() === 'POST',
    )
    await page.getByRole('button', { name: '运行实际几何检查' }).click()
    const qualityResponse = await qualityResponsePromise
    if (!qualityResponse.ok()) throw new Error(`geometry quality inspection failed: ${qualityResponse.status()}`)
    const qualityRecord = await qualityResponse.json()
    const qualityFindings = qualityRecord.report.findings ?? []
    const highlightedFindingIndex = qualityFindings.findIndex(
      (finding) => (finding.geometry_refs ?? []).some(
        (reference) => (reference.world_triangles_mm ?? []).length > 0,
      ),
    )
    if (qualityRecord.report.status !== 'failed' || highlightedFindingIndex < 0) {
      throw new Error(`unexpected geometry quality report: ${JSON.stringify(qualityRecord.report)}`)
    }
    await assertText(page.locator('.properties-panel'), [
      'Mesh/Assembly',
      '失败',
      '几何检查不代表结构强度、制造可行性或使用安全验证',
    ])
    const highlightedFinding = qualityFindings[highlightedFindingIndex]
    const highlightedNodeIds = highlightedFinding.node_ids ?? []
    const qualityTriangleCount = (highlightedFinding.geometry_refs ?? []).reduce(
      (count, reference) => count + (reference.world_triangles_mm ?? []).length,
      0,
    )
    const qualityFinding = page.locator('.quality-finding').nth(highlightedFindingIndex)
    await qualityFinding.click()
    await page.locator('.cad-status-bar').getByText(`选择：${highlightedNodeIds[0]}`, { exact: true }).waitFor()
    await page.locator(
      `.weapon-viewport[data-quality-node-ids="${highlightedNodeIds.join(',')}"]`
      + `[data-quality-triangle-count="${qualityTriangleCount}"]`,
    ).waitFor()
    await page.screenshot({ path: QUALITY_HIGHLIGHT_SCREENSHOT, fullPage: true })
    if ((await stat(QUALITY_HIGHLIGHT_SCREENSHOT)).size < 20_000) {
      throw new Error('quality triangle highlight screenshot is unexpectedly small')
    }

    // The confirmed manual transform intentionally leaves a connected module
    // misaligned, so the quality gate blocks delivery. Switch to the previous
    // valid immutable version to prove exports never bypass that gate.
    await switchVersionAndWait(page, 'V4')
    await page.waitForFunction(
      () => document.querySelector('.concept-runtime-state')?.textContent?.includes('已切换到 V4'),
      { timeout: 20_000 },
    )

    const exportResponsePromise = page.waitForResponse(
      (response) => /\/api\/v1\/versions\/[^/]+\/exports$/.test(response.url())
        && response.request().method() === 'POST',
    )
    const downloadPromise = waitForDownload(page)
    await page.getByRole('button', { name: '创建并下载概念源包' }).click()
    const exportResponse = await exportResponsePromise
    if (!exportResponse.ok()) throw new Error(`delivery export failed: ${exportResponse.status()}`)
    const deliveryRecord = await exportResponse.json()
    const agentBaseUrl = new URL(exportResponse.url()).origin
    const download = await resolveDownload(
      await downloadPromise,
      `${agentBaseUrl}/api/v1/exports/${deliveryRecord.export_id}/file`,
      `${deliveryRecord.export_id}.zip`,
    )
    if (!download.filename.endsWith('.zip')) {
      throw new Error(`unexpected export filename: ${download.filename}`)
    }
    const downloadPath = download.path
    if (!downloadPath || (await stat(downloadPath)).size < 500) throw new Error('concept export download is empty')
    await assertText(page.locator('.export-panel'), ['export_', 'SOURCE ZIP'])
    await page.getByRole('button', { name: 'GLB', exact: true }).click()
    const glbDownloadPromise = waitForDownload(page)
    await page.getByRole('button', { name: '创建并下载 combined GLB' }).click()
    const glbDownload = await resolveDownload(
      await glbDownloadPromise,
      `${agentBaseUrl}/api/v1/exports/${deliveryRecord.export_id}/combined.glb`,
      `${deliveryRecord.export_id}.glb`,
    )
    if (!glbDownload.filename.endsWith('.glb')) {
      throw new Error(`unexpected combined GLB filename: ${glbDownload.filename}`)
    }
    const glbDownloadPath = glbDownload.path
    if (!glbDownloadPath || (await stat(glbDownloadPath)).size < 5_000) {
      throw new Error('combined GLB download is unexpectedly small')
    }
    const glbHeader = await readFile(glbDownloadPath)
    if (glbHeader.subarray(0, 4).toString('ascii') !== 'glTF') {
      throw new Error('combined GLB download has an invalid header')
    }
    const persistedCombinedGlb = await persistDccCombinedGlb(glbDownloadPath)
    await page.getByRole('button', { name: 'OBJ', exact: true }).click()
    const objDownloadPromise = waitForDownload(page)
    await page.getByRole('button', { name: '创建并下载 combined OBJ' }).click()
    const objDownload = await resolveDownload(
      await objDownloadPromise,
      `${agentBaseUrl}/api/v1/exports/${deliveryRecord.export_id}/combined.obj`,
      `${deliveryRecord.export_id}.obj`,
    )
    if (!objDownload.filename.endsWith('.obj')) {
      throw new Error(`unexpected combined OBJ filename: ${objDownload.filename}`)
    }
    const objDownloadPath = objDownload.path
    if (!objDownloadPath || (await stat(objDownloadPath)).size < 5_000) {
      throw new Error('combined OBJ download is unexpectedly small')
    }
    const objText = await readFile(objDownloadPath, 'utf8')
    for (const phrase of [
      '# ForgeCAD combined OBJ',
      '# units: meter',
      'o NODE_node_core__module_core_shell_01__GEO_module_core_shell_01_LOD0',
      '\nv ',
      '\nvt ',
      '\nvn ',
      '\nf ',
    ]) {
      if (!objText.includes(phrase)) throw new Error(`combined OBJ is missing ${phrase}`)
    }
    const mtlDownloadPromise = waitForDownload(page)
    await page.getByRole('button', { name: '下载配套 combined.mtl' }).click()
    const mtlDownload = await resolveDownload(
      await mtlDownloadPromise,
      `${agentBaseUrl}/api/v1/exports/${deliveryRecord.export_id}/combined.mtl`,
      'combined.mtl',
    )
    if (mtlDownload.filename !== 'combined.mtl') {
      throw new Error(`unexpected combined MTL filename: ${mtlDownload.filename}`)
    }
    const mtlDownloadPath = mtlDownload.path
    const mtlText = mtlDownloadPath ? await readFile(mtlDownloadPath, 'utf8') : ''
    if (!mtlText.includes('newmtl ') || !mtlText.includes('\nKd ')) {
      throw new Error('combined MTL download is invalid')
    }
    await page.getByRole('button', { name: 'PNG', exact: true }).click()
    const previewDownloadPromise = waitForDownload(page)
    await page.getByRole('button', { name: '创建并下载透明 preview.png' }).click()
    const previewDownload = await resolveDownload(
      await previewDownloadPromise,
      `${agentBaseUrl}/api/v1/exports/${deliveryRecord.export_id}/preview.png`,
      `${deliveryRecord.export_id}-preview.png`,
    )
    if (!previewDownload.filename.endsWith('-preview.png')) {
      throw new Error(`unexpected preview PNG filename: ${previewDownload.filename}`)
    }
    const previewDownloadPath = previewDownload.path
    if (!previewDownloadPath || (await stat(previewDownloadPath)).size < 5_000) {
      throw new Error('preview PNG download is unexpectedly small')
    }
    const previewBytes = await readFile(previewDownloadPath)
    assertPng(previewBytes, 'preview')
    await copyFile(previewDownloadPath, PREVIEW_RENDER)
    const explodedDownloadPromise = waitForDownload(page)
    await page.getByRole('button', { name: '下载 exploded.png' }).click()
    const explodedDownload = await resolveDownload(
      await explodedDownloadPromise,
      `${agentBaseUrl}/api/v1/exports/${deliveryRecord.export_id}/exploded.png`,
      `${deliveryRecord.export_id}-exploded.png`,
    )
    if (!explodedDownload.filename.endsWith('-exploded.png')) {
      throw new Error(`unexpected exploded PNG filename: ${explodedDownload.filename}`)
    }
    const explodedDownloadPath = explodedDownload.path
    if (!explodedDownloadPath || (await stat(explodedDownloadPath)).size < 5_000) {
      throw new Error('exploded PNG download is unexpectedly small')
    }
    const explodedBytes = await readFile(explodedDownloadPath)
    assertPng(explodedBytes, 'exploded')
    if (previewBytes.equals(explodedBytes)) throw new Error('preview and exploded PNG are identical')
    await copyFile(explodedDownloadPath, EXPLODED_RENDER)
    const renderSetDownloadPromise = waitForDownload(page)
    await page.getByRole('button', { name: '下载正交视图与转台 ZIP' }).click()
    const renderSetDownload = await resolveDownload(
      await renderSetDownloadPromise,
      `${agentBaseUrl}/api/v1/exports/${deliveryRecord.export_id}/renders.zip`,
      `${deliveryRecord.export_id}-renders.zip`,
    )
    if (!renderSetDownload.filename.endsWith('-renders.zip')) {
      throw new Error(`unexpected render set filename: ${renderSetDownload.filename}`)
    }
    const renderSetPath = renderSetDownload.path
    if (!renderSetPath || (await stat(renderSetPath)).size < 20_000) {
      throw new Error('render set ZIP is unexpectedly small')
    }
    const renderSetHeader = await readFile(renderSetPath)
    if (renderSetHeader.subarray(0, 2).toString('ascii') !== 'PK') {
      throw new Error('render set ZIP header is invalid')
    }
    if (!deliveryRecord.turntable_video_sha256 || deliveryRecord.turntable_video_mime_type !== 'video/mp4') {
      throw new Error('delivery export did not report a turntable MP4')
    }
    const videoDownloadPromise = waitForDownload(page)
    await page.getByRole('button', { name: '下载转台 MP4' }).click()
    const videoDownload = await resolveDownload(
      await videoDownloadPromise,
      `${agentBaseUrl}/api/v1/exports/${deliveryRecord.export_id}/turntable.mp4`,
      `${deliveryRecord.export_id}-turntable.mp4`,
    )
    if (!videoDownload.filename.endsWith('-turntable.mp4')) {
      throw new Error(`unexpected turntable MP4 filename: ${videoDownload.filename}`)
    }
    const videoDownloadPath = videoDownload.path
    if (!videoDownloadPath || (await stat(videoDownloadPath)).size < 1_000) {
      throw new Error('turntable MP4 is unexpectedly small')
    }
    const videoHeader = await readFile(videoDownloadPath)
    if (videoHeader.subarray(4, 8).toString('ascii') !== 'ftyp') {
      throw new Error('turntable MP4 header is invalid')
    }
    const visualArtifacts = [
      ['front', `${agentBaseUrl}/api/v1/exports/${deliveryRecord.export_id}/views/front.png`, FRONT_RENDER],
      ['top', `${agentBaseUrl}/api/v1/exports/${deliveryRecord.export_id}/views/top.png`, TOP_RENDER],
      ['turntable', `${agentBaseUrl}/api/v1/exports/${deliveryRecord.export_id}/turntable/0.png`, TURNTABLE_RENDER],
    ]
    for (const [label, url, destination] of visualArtifacts) {
      const artifact = await downloadDirect(page, url)
      const artifactPath = artifact.path
      if (!artifactPath) throw new Error(`${label} render download has no path`)
      const bytes = await readFile(artifactPath)
      assertPng(bytes, label)
      await copyFile(artifactPath, destination)
    }
    if (conceptExportPosts !== 1) {
      throw new Error(`format downloads created ${conceptExportPosts} exports instead of reusing one`)
    }

    if (browserErrors.length) throw new Error(`browser page errors: ${browserErrors.join(' | ')}`)
    return {
      screenshot: SCREENSHOT,
      viewport: { width: 1536, height: 1024 },
      selected_node_id: 'node_front',
      replacement_module_id: 'module_front_shell_02',
      undo_redo_verified: true,
      exploded_view_verified: true,
      drag_candidate_verified: true,
      connector_snap_verified: true,
      mirror_version_verified: true,
      mirror_screenshot: MIRROR_SCREENSHOT,
      viewport_lifecycle: lifecycle,
      operation_timeline_verified: true,
      timeline_pagination_verified: true,
      timeline_search_filter_verified: true,
      timeline_rejected_diagnostic_verified: true,
      planner_brief_interpretation_verified: true,
      planner_variant_count: 3,
      planner_provenance_verified: true,
      planner_variant_selection_verified: true,
      planner_screenshot: PLANNER_SCREENSHOT,
      change_planner_ghost_preview_verified: true,
      change_planner_confirmation_verified: true,
      change_planner_timeline_provenance_verified: true,
      change_set_audit_export: {
        audit_export_id: auditExportRecord.audit_export_id,
        record_count: auditExportRecord.record_count,
        package_sha256: auditExportRecord.package_sha256,
      },
      dcc_combined_glb: persistedCombinedGlb,
      change_set_audit_download_verified: true,
      change_set_audit_screenshot: AUDIT_EXPORT_SCREENSHOT,
      change_planner_screenshot: CHANGE_PLANNER_SCREENSHOT,
      geometry_quality_inspection_verified: true,
      quality_finding_focus_verified: true,
      quality_dual_node_highlight_verified: true,
      quality_triangle_overlay_count: qualityTriangleCount,
      quality_highlight_screenshot: QUALITY_HIGHLIGHT_SCREENSHOT,
      quality_run_id: qualityRecord.quality_run_id,
      export_downloaded: true,
      combined_glb_downloaded: true,
      combined_obj_downloaded: true,
      combined_mtl_downloaded: true,
      preview_png_downloaded: true,
      exploded_png_downloaded: true,
      preview_render: PREVIEW_RENDER,
      exploded_render: EXPLODED_RENDER,
      render_set_downloaded: true,
      turntable_video_downloaded: true,
      export_reuse_verified: true,
      orthographic_renders: { front: FRONT_RENDER, top: TOP_RENDER },
      turntable_render: TURNTABLE_RENDER,
    }
  } finally {
    // Headless Chrome can occasionally wait indefinitely while flushing a
    // cancelled download target in CI. All assertions and screenshots above
    // have completed at this point; let the explicit smoke-process shutdown
    // own browser teardown instead of holding the release gate forever.
    void context.close().catch(() => undefined)
    void browser.close().catch(() => undefined)
  }
}

async function seedTimelineAuditFixture(baseUrl, projectId) {
  const project = await jsonRequest(baseUrl, `/api/v1/projects/${projectId}`)
  const versionId = project.current_version_id
  const rejectedId = 'change_ui_rejected_locked'
  await jsonRequest(baseUrl, `/api/v1/versions/${versionId}/change-sets`, {
    method: 'POST',
    idempotencyKey: 'r3-ui-rejected-propose',
    body: {
      client_request_id: 'r3-ui-rejected-propose',
      change_set: {
        schema_version: 'DesignChangeSet@1',
        change_set_id: rejectedId,
        project_id: projectId,
        base_version_id: versionId,
        summary: 'UI rejected diagnostic fixture for a locked core node.',
        operations: [{
          operation_id: 'op_ui_rejected_locked',
          op: 'set_transform',
          node_id: 'node_core',
          transform: { position: [1, 0, 0], rotation: [0, 0, 0], scale: [1, 1, 1] },
        }],
        protected_node_ids: [],
        status: 'proposed',
      },
    },
  })
  const rejected = await jsonRequestAllowError(
    baseUrl,
    `/api/v1/change-sets/${rejectedId}:preview`,
    {
      method: 'POST',
      idempotencyKey: 'r3-ui-rejected-preview',
    },
  )
  if (rejected.status !== 400 || rejected.body.error?.code !== 'CHANGE_SET_INVALID') {
    throw new Error(`UI rejected fixture was not rejected: ${JSON.stringify(rejected)}`)
  }
  for (let index = 1; index <= 21; index += 1) {
    const suffix = String(index).padStart(2, '0')
    await jsonRequest(baseUrl, `/api/v1/versions/${versionId}/change-sets`, {
      method: 'POST',
      idempotencyKey: `r3-ui-timeline-proposed-${suffix}`,
      body: {
        client_request_id: `r3-ui-timeline-proposed-${suffix}`,
        change_set: {
          schema_version: 'DesignChangeSet@1',
          change_set_id: `change_ui_timeline_proposed_${suffix}`,
          project_id: projectId,
          base_version_id: versionId,
          summary: `UI pagination fixture ${suffix}.`,
          operations: [{
            operation_id: `op_ui_timeline_proposed_${suffix}`,
            op: 'set_style',
            path: 'style.detail_density',
            value: 0.5,
          }],
          protected_node_ids: [],
          status: 'proposed',
        },
      },
    })
  }
  // replacement + mirror + confirmed transform + rejected connector-snap preview
  // + rejected fixture + 21 proposed fixtures
  return { rejected_id: rejectedId, total_count: 26 }
}

async function downloadDirect(page, url) {
  const promise = waitForDownload(page)
  await page.evaluate((target) => {
    const anchor = document.createElement('a')
    anchor.href = target
    anchor.download = ''
    document.body.appendChild(anchor)
    anchor.click()
    anchor.remove()
  }, url)
  return resolveDownload(await promise, url, url.split('/').pop() || 'artifact')
}

function waitForDownload(page) {
  return REQUIRE_BROWSER_DOWNLOADS
    ? page.waitForEvent('download')
    : Promise.resolve(null)
}

async function resolveDownload(download, url, fallbackFilename) {
  if (download) {
    const path = await download.path()
    if (!path) throw new Error(`browser download has no path: ${download.suggestedFilename()}`)
    return { filename: download.suggestedFilename(), path }
  }
  const response = await fetch(url)
  if (!response.ok) throw new Error(`fallback download failed: ${response.status} ${url}`)
  const payload = Buffer.from(await response.arrayBuffer())
  const path = join(OUTPUT_DIR, `ci-${fallbackFilename}`)
  await writeFile(path, payload)
  return { filename: fallbackFilename, path }
}

function assertPng(bytes, label) {
  if (bytes.subarray(0, 8).toString('hex') !== '89504e470d0a1a0a') {
    throw new Error(`${label} PNG signature is invalid`)
  }
  const width = bytes.readUInt32BE(16)
  const height = bytes.readUInt32BE(20)
  if (width !== 640 || height !== 640) {
    throw new Error(`${label} PNG dimensions are ${width}x${height}`)
  }
}

async function stressViewportLifecycle(page) {
  const session = await page.context().newCDPSession(page)
  await session.send('Performance.enable')
  await session.send('HeapProfiler.collectGarbage')
  const before = await performanceMetric(session, 'JSHeapUsedSize')
  const host = page.locator('.weapon-viewport')
  const initialGeneration = Number(await host.getAttribute('data-renderer-generation') || '0')
  const cycles = 20
  for (let index = 0; index < cycles; index += 1) {
    await switchVersionAndWait(page, 'V4')
    // End each cycle on the current immutable version. ChangeSet confirmation
    // intentionally rejects previews made from an older historical branch.
    await switchVersionAndWait(page, 'V5')
    const canvasCount = await page.locator('.weapon-viewport canvas').count()
    const activeContexts = Number(await host.getAttribute('data-active-webgl-contexts') || '0')
    if (canvasCount !== 1 || activeContexts !== 1) {
      throw new Error(`viewport lifecycle leak at cycle ${index}: canvases=${canvasCount}, contexts=${activeContexts}`)
    }
  }
  await session.send('HeapProfiler.collectGarbage')
  const after = await performanceMetric(session, 'JSHeapUsedSize')
  const finalGeneration = Number(await host.getAttribute('data-renderer-generation') || '0')
  const generationDelta = finalGeneration - initialGeneration
  const heapGrowth = after - before
  if (generationDelta > 1) {
    throw new Error(`viewport recreated its renderer during version switching: ${generationDelta}`)
  }
  if (heapGrowth > 64 * 1024 * 1024) {
    throw new Error(`viewport JS heap grew by ${heapGrowth} bytes after GC`)
  }
  const geometries = Number(await host.getAttribute('data-renderer-geometries') || '0')
  const textures = Number(await host.getAttribute('data-renderer-textures') || '0')
  // The persistent GLTF cache intentionally retains the bounded 10-module Pack;
  // this ceiling catches growth across repeated version switches without treating
  // cached source geometry as a leak.
  if (geometries > 64 || textures > 32) {
    throw new Error(`viewport GPU resource counts are unbounded: geometries=${geometries}, textures=${textures}`)
  }
  await session.detach()
  return {
    cycles,
    renderer_generations: generationDelta,
    active_contexts: 1,
    canvas_count: 1,
    heap_growth_bytes_after_gc: heapGrowth,
    geometries,
    textures,
  }
}

async function switchVersionAndWait(page, versionLabel) {
  await page.getByRole('button', { name: new RegExp(`^${versionLabel}\\b`) }).click()
  await page.waitForFunction(
    ({ label }) => {
      const activeVersion = document.querySelector('.version-list button.active')
      const viewport = document.querySelector('.weapon-viewport')
      return activeVersion?.textContent?.trim().startsWith(label)
        && viewport?.dataset.loadState === 'ready'
    },
    { label: versionLabel },
    { timeout: 20_000 },
  )
}

async function performanceMetric(session, name) {
  const result = await session.send('Performance.getMetrics')
  return result.metrics.find((metric) => metric.name === name)?.value ?? 0
}

async function verifyReplacement(baseUrl, projectId, expectedAuditExport) {
  const project = await jsonRequest(baseUrl, `/api/v1/projects/${projectId}`)
  if ((project.versions ?? []).length !== 6) {
    throw new Error(`replacement, mirror, transform, and Change Planner should create V6, got ${(project.versions ?? []).length} versions`)
  }
  const version = await jsonRequest(baseUrl, `/api/v1/versions/${project.current_version_id}`)
  const graph = await jsonRequest(baseUrl, `/api/v1/module-graphs/${version.module_graph_id}`)
  const frontNode = graph.graph.nodes.find((node) => node.node_id === 'node_front')
  const gripNode = graph.graph.nodes.find((node) => node.node_id === 'node_grip')
  const frontEdge = graph.graph.edges.find((edge) => edge.to_node_id === 'node_front')
  if (frontNode?.module_id !== 'module_front_shell_02') {
    throw new Error(`restart restored wrong front module: ${frontNode?.module_id}`)
  }
  if (frontEdge?.to_connector_id !== 'connector_front_02_core') {
    throw new Error(`replacement connector was not remapped: ${frontEdge?.to_connector_id}`)
  }
  if (JSON.stringify(frontNode?.transform.position) !== JSON.stringify([-50, 0, 0])) {
    throw new Error(`replacement node was not snapped after restart: ${JSON.stringify(frontNode?.transform.position)}`)
  }
  if (gripNode?.mirror_axis !== 'x') {
    throw new Error(`restart lost grip mirror state: ${gripNode?.mirror_axis}`)
  }
  if (gripNode?.transform?.position?.[0] !== 17) {
    throw new Error(`restart lost confirmed grip transform: ${JSON.stringify(gripNode?.transform)}`)
  }
  if (
    version.spec.proportions.overall_length_mm !== 218
    || version.spec.style.detail_density !== 0.84
  ) {
    throw new Error(`restart lost confirmed Change Planner parameters: ${JSON.stringify(version.spec)}`)
  }
  const rejectedTimeline = await jsonRequest(
    baseUrl,
    `/api/v1/projects/${projectId}/change-sets?status=rejected&q=ui_rejected_locked`,
  )
  if (rejectedTimeline.items?.length !== 1) {
    throw new Error(`restart lost rejected ChangeSet search: ${JSON.stringify(rejectedTimeline)}`)
  }
  const diagnostic = rejectedTimeline.items[0].diagnostic
  if (diagnostic?.code !== 'CHANGE_SET_INVALID' || !diagnostic.node_ids?.includes('node_core')) {
    throw new Error(`restart lost rejected diagnostic: ${JSON.stringify(diagnostic)}`)
  }
  const auditExports = await jsonRequest(
    baseUrl,
    `/api/v1/projects/${projectId}/change-set-audit-exports?limit=10`,
  )
  const restoredAudit = auditExports.items?.find(
    (item) => item.audit_export_id === expectedAuditExport.audit_export_id,
  )
  if (
    !restoredAudit
    || restoredAudit.record_count !== expectedAuditExport.record_count
    || restoredAudit.package_sha256 !== expectedAuditExport.package_sha256
  ) {
    throw new Error(`restart lost ChangeSet audit export: ${JSON.stringify(auditExports)}`)
  }
  const restoredAuditResponse = await fetch(
    `${baseUrl}/api/v1/change-set-audit-exports/${expectedAuditExport.audit_export_id}/file`,
  )
  const restoredAuditBytes = Buffer.from(await restoredAuditResponse.arrayBuffer())
  const restoredAuditHash = createHash('sha256').update(restoredAuditBytes).digest('hex')
  if (
    !restoredAuditResponse.ok
    || restoredAuditBytes.subarray(0, 2).toString('ascii') !== 'PK'
    || restoredAuditHash !== expectedAuditExport.package_sha256
    || restoredAuditResponse.headers.get('x-content-sha256') !== restoredAuditHash
  ) {
    throw new Error('restart ChangeSet audit download or hash mismatch')
  }
  return {
    version_id: version.version_id,
    graph_id: graph.graph.graph_id,
    module_id: frontNode.module_id,
    connector_id: frontEdge.to_connector_id,
    position_mm: frontNode.transform.position,
    mirrored_node_id: gripNode.node_id,
    mirror_axis: gripNode.mirror_axis,
    planner_overall_length_mm: version.spec.proportions.overall_length_mm,
    planner_detail_density: version.spec.style.detail_density,
    rejected_change_set_id: rejectedTimeline.items[0].change_set.change_set_id,
    rejected_diagnostic_code: diagnostic.code,
    audit_export_id: restoredAudit.audit_export_id,
    audit_record_count: restoredAudit.record_count,
    audit_package_sha256: restoredAuditHash,
  }
}

function connector(connectorId, slot, connectorType, position = [0, 0, 0]) {
  return {
    connector_id: connectorId,
    slot,
    connector_type: connectorType,
    transform: { position, rotation: [0, 0, 0], scale: [1, 1, 1] },
    scale_range: [0.8, 1.2],
    exclusive: true,
  }
}

function graphNode(nodeId, moduleId, position, locked = false) {
  return {
    node_id: nodeId,
    module_id: moduleId,
    transform: { position, rotation: [0, 0, 0], scale: [1, 1, 1] },
    mirror_axis: 'none',
    locked,
    visible: true,
  }
}

function graphEdge(name, sourceConnectorId, targetConnectorId) {
  return {
    edge_id: `edge_core_${name}`,
    from_node_id: 'node_core',
    from_connector_id: sourceConnectorId,
    to_node_id: `node_${name}`,
    to_connector_id: targetConnectorId,
    status: 'connected',
  }
}

function boxGlb(name, color, scale) {
  const [sx, sy, sz] = scale.map((value) => value / 2000)
  const positions = new Float32Array([
    -sx, -sy, -sz, sx, -sy, -sz, sx, sy, -sz, -sx, sy, -sz,
    -sx, -sy, sz, sx, -sy, sz, sx, sy, sz, -sx, sy, sz,
  ])
  const indices = new Uint16Array([
    0, 1, 2, 0, 2, 3, 4, 6, 5, 4, 7, 6,
    0, 4, 5, 0, 5, 1, 1, 5, 6, 1, 6, 2,
    2, 6, 7, 2, 7, 3, 3, 7, 4, 3, 4, 0,
  ])
  const binary = Buffer.alloc(positions.byteLength + indices.byteLength)
  Buffer.from(positions.buffer).copy(binary, 0)
  Buffer.from(indices.buffer).copy(binary, positions.byteLength)
  const document = {
    asset: { version: '2.0', generator: 'ForgeCAD R3 smoke' },
    scene: 0,
    scenes: [{ nodes: [0] }],
    nodes: [{ name, mesh: 0 }],
    meshes: [{ primitives: [{ attributes: { POSITION: 0 }, indices: 1, material: 0 }] }],
    materials: [{ pbrMetallicRoughness: { baseColorFactor: color, metallicFactor: 0.72, roughnessFactor: 0.34 } }],
    buffers: [{ byteLength: binary.byteLength }],
    bufferViews: [
      { buffer: 0, byteOffset: 0, byteLength: positions.byteLength, target: 34962 },
      { buffer: 0, byteOffset: positions.byteLength, byteLength: indices.byteLength, target: 34963 },
    ],
    accessors: [
      { bufferView: 0, componentType: 5126, count: 8, type: 'VEC3', min: [-sx, -sy, -sz], max: [sx, sy, sz] },
      { bufferView: 1, componentType: 5123, count: indices.length, type: 'SCALAR' },
    ],
  }
  const json = Buffer.from(JSON.stringify(document))
  const paddedJsonLength = Math.ceil(json.length / 4) * 4
  const paddedBinaryLength = Math.ceil(binary.length / 4) * 4
  const output = Buffer.alloc(12 + 8 + paddedJsonLength + 8 + paddedBinaryLength)
  output.write('glTF', 0)
  output.writeUInt32LE(2, 4)
  output.writeUInt32LE(output.length, 8)
  output.writeUInt32LE(paddedJsonLength, 12)
  output.writeUInt32LE(0x4e4f534a, 16)
  json.copy(output, 20)
  output.fill(0x20, 20 + json.length, 20 + paddedJsonLength)
  const binaryHeader = 20 + paddedJsonLength
  output.writeUInt32LE(paddedBinaryLength, binaryHeader)
  output.writeUInt32LE(0x004e4942, binaryHeader + 4)
  binary.copy(output, binaryHeader + 8)
  return output
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

async function persistDccCombinedGlb(sourcePath) {
  if (!DCC_COMBINED_OUTPUT) return null
  if (!isAbsolute(DCC_COMBINED_OUTPUT) || !DCC_COMBINED_OUTPUT.endsWith('.glb')) {
    throw new Error('FORGECAD_DCC_COMBINED_OUTPUT must be an absolute .glb path')
  }
  const destination = resolve(DCC_COMBINED_OUTPUT)
  const committedPackRoot = resolve(ROOT, 'assets', 'module-packs')
  if (destination === committedPackRoot || destination.startsWith(`${committedPackRoot}/`)) {
    throw new Error('FORGECAD_DCC_COMBINED_OUTPUT cannot target committed module assets')
  }
  try {
    await stat(destination)
    throw new Error(`FORGECAD_DCC_COMBINED_OUTPUT already exists: ${destination}`)
  } catch (error) {
    if (error?.code !== 'ENOENT') throw error
  }
  await mkdir(dirname(destination), { recursive: true })
  await copyFile(sourcePath, destination)
  return destination
}

async function jsonRequestAllowError(baseUrl, path, options = {}) {
  const response = await fetch(baseUrl + path, {
    method: options.method || 'GET',
    headers: {
      'Content-Type': 'application/json',
      ...(options.idempotencyKey ? { 'Idempotency-Key': options.idempotencyKey } : {}),
    },
    body: options.body ? JSON.stringify(options.body) : undefined,
  })
  const body = await response.json()
  return { status: response.status, body }
}

async function waitForHttp(url, child, label) {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    if (child.exitCode !== null) throw new Error(`${label} process exited early with code ${child.exitCode}`)
    try {
      const response = await fetch(url)
      if (response.ok) return
    } catch {
      // Continue polling while the process starts.
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
    throw new Error(`R3 UI smoke requires system Chrome or WUSHEN_BROWSER_EXECUTABLE: ${error}`)
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
    if (!text.includes(item)) throw new Error(`expected text not found: ${item}\n${text}`)
  }
}

function sleep(milliseconds) {
  return new Promise((resolveSleep) => setTimeout(resolveSleep, milliseconds))
}

const smokeTimeout = setTimeout(() => {
  console.error(`Workbench smoke timed out during: ${smokeStage}`)
  process.exit(1)
}, SMOKE_TIMEOUT_MS)

main().then(
  () => {
    clearTimeout(smokeTimeout)
    process.exit(0)
  },
  (error) => {
    clearTimeout(smokeTimeout)
    console.error(error)
    process.exit(1)
  },
)
