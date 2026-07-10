#!/usr/bin/env node
import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, rm, stat } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const OUTPUT_DIR = join(ROOT, 'output', 'playwright')
const SCREENSHOT = join(OUTPUT_DIR, 'r3-concept-workbench.png')

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
        },
        stdio: ['ignore', 'pipe', 'pipe'],
      },
    )
    processes.push(agent)
    await waitForHttp(`${agentBaseUrl}/api/health`, agent, 'agent health')
    const seeded = await seedConceptGraph(agentBaseUrl)

    const vite = spawn(
      'npm',
      ['--workspace', 'apps/desktop', 'run', 'dev', '--', '--host', '127.0.0.1', '--port', String(vitePort)],
      {
        cwd: ROOT,
        env: { ...process.env, VITE_FORGE_API_BASE_URL: agentBaseUrl },
        stdio: ['ignore', 'pipe', 'pipe'],
      },
    )
    processes.push(vite)
    await waitForHttp(viteBaseUrl, vite, 'vite frontend')
    const result = await runWorkbenchUi(viteBaseUrl, seeded)
    console.log(JSON.stringify({ ok: true, ...seeded, ...result }, null, 2))
  } finally {
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

  const modules = [
    {
      moduleId: 'module_core_shell_01',
      assetId: 'asset_core_shell_01',
      category: 'core_shell',
      color: [0.31, 0.37, 0.44, 1],
      scale: [70, 30, 24],
      connectors: [
        connector('connector_core_front', 'core.front', 'shell_mount'),
        connector('connector_core_grip', 'core.grip', 'grip_mount'),
      ],
    },
    {
      moduleId: 'module_front_shell_01',
      assetId: 'asset_front_shell_01',
      category: 'front_shell',
      color: [0.19, 0.23, 0.28, 1],
      scale: [58, 22, 20],
      connectors: [connector('connector_front_core', 'front.core', 'shell_mount')],
    },
    {
      moduleId: 'module_grip_shell_01',
      assetId: 'asset_grip_shell_01',
      category: 'grip_shell',
      color: [0.12, 0.16, 0.2, 1],
      scale: [28, 62, 25],
      connectors: [connector('connector_grip_core', 'grip.core', 'grip_mount')],
    },
  ]

  for (const item of modules) {
    const glb = boxGlb(item.moduleId, item.color, item.scale)
    await jsonRequest(baseUrl, '/api/v1/module-assets', {
      method: 'POST',
      idempotencyKey: `r3-ui-${item.moduleId}`,
      body: {
        client_request_id: `r3-ui-${item.moduleId}`,
        logical_path: `packs/weapon-concept/${item.moduleId}.glb`,
        glb_data_base64: glb.toString('base64'),
        manifest: {
          schema_version: 'ModuleAssetManifest@1',
          module_id: item.moduleId,
          pack_id: 'pack_weapon_concept_v1',
          category: item.category,
          asset_id: item.assetId,
          sha256: createHash('sha256').update(glb).digest('hex'),
          bounds_mm: item.scale,
          triangle_count: 12,
          material_slots: ['primary'],
          connectors: item.connectors,
        },
      },
    })
  }

  const graph = {
    schema_version: 'ModuleGraph@1',
    graph_id: 'mg_r3_ui_arctic_patrol',
    project_id: project.project_id,
    root_node_id: 'node_core',
    nodes: [
      graphNode('node_core', 'module_core_shell_01', [0, 15, 0], true),
      graphNode('node_front', 'module_front_shell_01', [-64, 17, 0]),
      graphNode('node_grip', 'module_grip_shell_01', [22, -34, 0]),
    ],
    edges: [
      {
        edge_id: 'edge_core_front',
        from_node_id: 'node_core',
        from_connector_id: 'connector_core_front',
        to_node_id: 'node_front',
        to_connector_id: 'connector_front_core',
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
    module_count: modules.length,
  }
}

async function runWorkbenchUi(baseUrl, seeded) {
  const browser = await launchSystemBrowser()
  const page = await browser.newPage({ viewport: { width: 1536, height: 1024 }, deviceScaleFactor: 1 })
  const browserErrors = []
  page.on('pageerror', (error) => browserErrors.push(error.message))
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
      'module_core_shell_01',
      'module_front_shell_01',
      'module_grip_shell_01',
    ])
    await assertText(page.locator('.cad-status-bar'), ['3 nodes', '单位：mm'])

    await page.getByRole('button', { name: /module_front_shell_01/ }).click()
    await assertText(page.locator('.cad-status-bar'), ['node_front'])
    const inspectorValues = await page.locator('.properties-panel .wide-field input').evaluateAll(
      (inputs) => inputs.map((input) => input.value),
    )
    if (inspectorValues[0] !== 'node_front' || inspectorValues[1] !== 'module_front_shell_01') {
      throw new Error(`inspector selection mismatch: ${JSON.stringify(inspectorValues)}`)
    }
    await page.getByRole('button', { name: '连接' }).click()
    await assertText(page.locator('.properties-panel'), ['front.core', '已连接'])

    const downloadPromise = page.waitForEvent('download')
    await page.getByRole('button', { name: '创建并下载概念源包' }).click()
    const download = await downloadPromise
    if (!download.suggestedFilename().endsWith('.zip')) {
      throw new Error(`unexpected export filename: ${download.suggestedFilename()}`)
    }
    const downloadPath = await download.path()
    if (!downloadPath || (await stat(downloadPath)).size < 500) throw new Error('concept export download is empty')
    await assertText(page.locator('.export-panel'), ['export_', 'SOURCE ZIP'])

    const canvas = page.locator('.weapon-viewport canvas')
    const canvasBox = await canvas.boundingBox()
    if (!canvasBox || canvasBox.width < 400 || canvasBox.height < 300) {
      throw new Error(`ModuleGraph canvas is not usable: ${JSON.stringify(canvasBox)}`)
    }
    await page.evaluate(() => window.scrollTo(0, 0))
    await page.screenshot({ path: SCREENSHOT, fullPage: true })
    if ((await stat(SCREENSHOT)).size < 20_000) throw new Error('R3 workbench screenshot is unexpectedly small')
    if (browserErrors.length) throw new Error(`browser page errors: ${browserErrors.join(' | ')}`)
    return {
      screenshot: SCREENSHOT,
      viewport: { width: 1536, height: 1024 },
      selected_node_id: 'node_front',
      export_downloaded: true,
    }
  } finally {
    await browser.close()
  }
}

function connector(connectorId, slot, connectorType) {
  return {
    connector_id: connectorId,
    slot,
    connector_type: connectorType,
    transform: { position: [0, 0, 0], rotation: [0, 0, 0], scale: [1, 1, 1] },
    scale_range: [0.8, 1.2],
    exclusive: true,
  }
}

function graphNode(nodeId, moduleId, position, locked = false) {
  return {
    node_id: nodeId,
    module_id: moduleId,
    transform: { position, rotation: [0, 0, 0], scale: [1, 1, 1] },
    locked,
    visible: true,
  }
}

function boxGlb(name, color, scale) {
  const [sx, sy, sz] = scale.map((value) => value / 2)
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

main().catch((error) => {
  console.error(error)
  process.exitCode = 1
})
