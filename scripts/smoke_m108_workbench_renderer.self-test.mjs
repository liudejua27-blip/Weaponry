#!/usr/bin/env node

import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const R3_PATH = join(ROOT, 'scripts', 'smoke_r3_concept_workbench_ui.mjs')
const RUNNER_PATH = join(ROOT, 'scripts', 'smoke_m108_workbench_renderer.mjs')
const REPORT_SCHEMA = 'M108WorkbenchRendererSelfTest@1'
const RUN_COUNT = 3
const configuredOutputRoot = process.env.FORGECAD_M108_SELF_TEST_OUTPUT_ROOT

function fail(stableErrorCode, message) {
  const error = new Error(message)
  error.stableErrorCode = stableErrorCode
  throw error
}

function assertTeardownContract(source) {
  if (source.includes('patchR3RouteHandling') || source.includes('materializePatchedR3Runner')) {
    fail('M108_SELF_TEST_RUNTIME_REWRITE_PRESENT', 'M108 wrapper must not rewrite or materialize the R3 runner')
  }
  const handlerStart = source.indexOf('const compatibilityRouteHandler = async (route) => {')
  const trackedHandlerStart = source.indexOf('const trackedCompatibilityRouteHandler = (route) => {')
  if (handlerStart < 0 || trackedHandlerStart < 0 || trackedHandlerStart <= handlerStart) {
    fail('M108_SELF_TEST_HANDLER_TRACKING_MISSING', 'M108 route handler tracking contract is missing')
  }
  if (source.slice(handlerStart, trackedHandlerStart).includes('route.continue(')) {
    fail('M108_SELF_TEST_DIRECT_ROUTE_CONTINUE', 'route handler must call the once-only continuation helper')
  }
  const teardownTokens = [
    'acceptingCompatibilityHandlers = false',
    'releaseActiveDesignRequest()',
    'await page.unrouteAll({ behavior: \'wait\' })',
    'await rm(corruptRuntimeFixturePath, { force: true })',
  ]
  let previous = -1
  for (const token of teardownTokens) {
    const position = source.indexOf(token)
    if (position <= previous) {
      fail('M108_SELF_TEST_TEARDOWN_ORDER', `teardown contract is out of order: ${token}`)
    }
    previous = position
  }
  const settledToken = 'await Promise.allSettled([...inFlightCompatibilityHandlers])'
  const firstSettled = source.indexOf(settledToken)
  const secondSettled = source.indexOf(settledToken, firstSettled + settledToken.length)
  const unrouteWait = source.indexOf('await page.unrouteAll({ behavior: \'wait\' })')
  if (firstSettled < 0 || secondSettled < 0 || firstSettled >= unrouteWait || secondSettled <= unrouteWait) {
    fail('M108_SELF_TEST_IN_FLIGHT_DRAIN_ORDER', 'M108 must drain handlers before and after unrouteAll(wait)')
  }
}

async function readManifestEvidence(outputDir) {
  const manifestPath = join(outputDir, 'workbench-captures', 'capture-manifest.json')
  const bytes = await readFile(manifestPath)
  const manifest = JSON.parse(bytes.toString('utf8'))
  if (manifest.schema_version !== 'M108WorkbenchCapture@1') {
    fail('M108_SELF_TEST_MANIFEST_SCHEMA', 'M108 renderer manifest schema is not M108WorkbenchCapture@1')
  }
  const captures = Array.isArray(manifest.captures) ? manifest.captures : []
  if (captures.length !== 4 || new Set(captures.map((capture) => capture.screenshot_sha256)).size !== 4) {
    fail('M108_SELF_TEST_MANIFEST_CAPTURE_COUNT', 'M108 renderer manifest must contain four distinct captures')
  }
  return {
    manifest_schema: manifest.schema_version,
    fixture_count: captures.length,
    capture_count: captures.length,
    manifest_sha256: createHash('sha256').update(bytes).digest('hex'),
    screenshot_sha256: captures.map((capture) => capture.screenshot_sha256).sort(),
    ...(configuredOutputRoot
      ? { artifact_path: relative(resolve(configuredOutputRoot), manifestPath) }
      : {}),
  }
}

async function runOnce(outputDir) {
  const result = spawnSync(
    process.execPath,
    [RUNNER_PATH],
    {
      cwd: ROOT,
      env: {
        ...process.env,
        FORGECAD_M108_RENDERER_OUTPUT_DIR: outputDir,
        FORGECAD_REQUIRE_BROWSER_DOWNLOADS: process.env.FORGECAD_REQUIRE_BROWSER_DOWNLOADS ?? '0',
      },
      encoding: 'utf8',
      timeout: 900_000,
      maxBuffer: 4 * 1024 * 1024,
    },
  )
  const combinedOutput = `${result.stdout ?? ''}\n${result.stderr ?? ''}`
  if (combinedOutput.includes('Route is already handled') || combinedOutput.includes('route.continue: Route is already handled')) {
    fail('M108_SELF_TEST_ROUTE_ALREADY_HANDLED', 'M108 renderer repeat reported an already-handled route')
  }
  if (result.error || result.status !== 0) {
    fail(
      result.error?.code === 'ETIMEDOUT' ? 'M108_SELF_TEST_TIMEOUT' : 'M108_SELF_TEST_CHILD_FAILED',
      `M108 renderer repeat ${outputDir} exited with ${result.status ?? result.error?.code ?? 'unknown'}`,
    )
  }
  return {
    exit_code: result.status,
    ...(await readManifestEvidence(outputDir)),
  }
}

const report = {
  schema_version: REPORT_SCHEMA,
  phase: 'self_test',
  subsystem: 'workbench_renderer_route_lifecycle',
  stable_error_code: null,
  repeat_count: RUN_COUNT,
  runs: [],
  ok: false,
}
let tempRoot = null
try {
  const source = await readFile(R3_PATH, 'utf8')
  assertTeardownContract(source)
  tempRoot = configuredOutputRoot
    ? resolve(configuredOutputRoot)
    : await mkdtemp(join(tmpdir(), 'forgecad_m108_renderer_self_test_'))
  await mkdir(tempRoot, { recursive: true })
  for (let index = 1; index <= RUN_COUNT; index += 1) {
    const outputDir = join(tempRoot, `run-${index}`)
    const result = await runOnce(outputDir)
    report.runs.push({ index, ...result })
  }
  report.ok = true
} catch (error) {
  report.stable_error_code = error?.stableErrorCode ?? 'M108_SELF_TEST_FAILED'
} finally {
  if (tempRoot && !configuredOutputRoot) await rm(tempRoot, { recursive: true, force: true })
}

console.log(JSON.stringify(report))
if (!report.ok) process.exitCode = 1
