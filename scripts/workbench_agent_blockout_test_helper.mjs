const UI_SETTLE_TIMEOUT_MS = 20_000
const DEFAULT_AGENT_GEOMETRY_TIMEOUT_MS = 90_000
const DEFAULT_PLAYWRIGHT_RESPONSE_TIMEOUT_MS = 30_000
const APP_SERVER_FRAMES_PATH = /^\/api\/v1\/app-server\/connections\/[^/]+\/frames$/
const BASE64_PATTERN = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/
const TEST_ONLY_LEGACY_AGENT_LIFECYCLE_ENV = 'FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE'
const TEST_ONLY_LEGACY_PRODUCT_CORE_ENV = 'FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE'
const K001_PACKAGED_PROBE_ENV = 'FORGECAD_K001_PACKAGED_PROBE'

/**
 * Build the environment for browser/dev-shell compatibility-oracle processes.
 *
 * K002 makes the public Python Thread/Turn mutation routes return 410 by
 * default. These deterministic browser regressions still exercise the old UI
 * through the K001 compat/http observer, so they must opt in to lifecycle,
 * product-core and packaged-probe test switches together. Inject the complete
 * set here instead of inheriting any switch from the developer shell. Production/default ownership remains covered by
 * test_k002_internal_routes.py::test_default_public_python_lifecycle_mutations_are_rust_owned.
 */
export function legacyLifecycleTestOracleEnvironment(baseEnvironment, overrides = {}) {
  const environment = { ...baseEnvironment, ...overrides }
  delete environment[TEST_ONLY_LEGACY_AGENT_LIFECYCLE_ENV]
  delete environment[TEST_ONLY_LEGACY_PRODUCT_CORE_ENV]
  delete environment[K001_PACKAGED_PROBE_ENV]
  environment[TEST_ONLY_LEGACY_AGENT_LIFECYCLE_ENV] = '1'
  environment[TEST_ONLY_LEGACY_PRODUCT_CORE_ENV] = '1'
  environment[K001_PACKAGED_PROBE_ENV] = '1'
  assertLegacyLifecycleTestOracleEnvironment(environment)
  return environment
}

export function assertLegacyLifecycleTestOracleEnvironment(environment) {
  if (
    environment?.[TEST_ONLY_LEGACY_AGENT_LIFECYCLE_ENV] !== '1'
    || environment?.[TEST_ONLY_LEGACY_PRODUCT_CORE_ENV] !== '1'
    || environment?.[K001_PACKAGED_PROBE_ENV] !== '1'
  ) {
    throw new Error(
      'browser compatibility oracle requires the explicit legacy lifecycle and product-core test switches',
    )
  }
}

export async function selectAgentDirectionAndWaitForCandidate(
  page,
  selectDirection,
  {
    candidateLocator = page.getByLabel('分件候选'),
    label = 'Agent direction preview',
  } = {},
) {
  // Both listeners begin before the click to avoid missing the fast segment
  // response. This is intentionally one bounded budget for the complete
  // build -> segment chain, not a production Worker timeout or per-stage SLA.
  const apiChainTimeoutMs = agentGeometryTimeoutMs()
  const startedAt = Date.now()
  const buildOutcomePromise = responseOutcome(
    page,
    '/api/v1/agent/blockouts',
    apiChainTimeoutMs,
  )
  const segmentationOutcomePromise = responseOutcome(
    page,
    '/api/v1/agent/blockouts:segment',
    apiChainTimeoutMs,
  )

  try {
    await selectDirection()
  } catch (error) {
    throw await diagnosticError(page, label, 'direction click failed', error, startedAt, apiChainTimeoutMs)
  }

  const buildOutcome = await buildOutcomePromise
  if (buildOutcome.error) {
    throw await diagnosticError(page, label, 'blockout build response timed out', buildOutcome.error, startedAt, apiChainTimeoutMs)
  }
  const build = await requireCreatedJson(buildOutcome.response, `${label} blockout build`)
  if (
    typeof build.artifact_id !== 'string'
    || typeof build.direction_id !== 'string'
    || typeof build.glb_base64 !== 'string'
    || build.glb_base64.length === 0
    || build.shape_program?.schema_version !== 'ShapeProgram@1'
    || !Number.isInteger(build.triangle_count)
    || build.triangle_count <= 0
  ) {
    throw new Error(`${label} blockout build returned incomplete GLB/ShapeProgram evidence: ${summary(build)}`)
  }

  const segmentationOutcome = await segmentationOutcomePromise
  if (segmentationOutcome.error) {
    throw await diagnosticError(page, label, 'blockout segmentation response timed out', segmentationOutcome.error, startedAt, apiChainTimeoutMs)
  }
  const segmentation = await requireCreatedJson(segmentationOutcome.response, `${label} blockout segmentation`)
  if (
    segmentation.artifact_id !== build.artifact_id
    || segmentation.direction_id !== build.direction_id
    || segmentation.segmentation_status !== 'candidate'
    || !Array.isArray(segmentation.parts)
    || segmentation.parts.length === 0
  ) {
    throw new Error(`${label} segmentation did not match the built candidate: build=${summary(build)} segment=${summary(segmentation)}`)
  }

  try {
    await candidateLocator.waitFor({ timeout: UI_SETTLE_TIMEOUT_MS })
  } catch (error) {
    throw await diagnosticError(page, label, 'successful API responses were not reflected in the candidate UI', error, startedAt, apiChainTimeoutMs)
  }

  return {
    build,
    segmentation,
    elapsed_ms: Date.now() - startedAt,
  }
}

/**
 * F026 presents one automatically built result through the current planner
 * adapter. It intentionally waits for rendered evidence only: no caller may
 * enumerate or click legacy direction cards.
 */
export async function waitForAgentSingleResultAndCandidate(
  page,
  {
    candidateLocator = page.getByLabel('分件候选'),
    resultLocator = page.getByLabel('当前临时结果'),
    label = 'Agent single result preview',
  } = {},
) {
  const startedAt = Date.now()
  const timeout = agentGeometryTimeoutMs()
  const deadline = startedAt + timeout
  const failedResultLocator = page.locator('[data-generation-state="failed"][aria-label="生成失败"]')
  try {
    // A V003 result is terminal in either direction.  In particular, a
    // legacy Planner response reaches the visible failure card immediately;
    // waiting the full geometry budget for a success-only locator turns a
    // useful contract error into an apparent hung browser run.  Poll both
    // terminal states so a genuine slow compile retains the same bounded
    // budget, while a declared failure is reported at once.
    while (Date.now() < deadline) {
      if (await failedResultLocator.count() > 0 && await failedResultLocator.isVisible()) {
        const failureText = await failedResultLocator.innerText().catch(() => 'unknown V003 failure')
        throw new Error(`Agent published a terminal generation failure: ${failureText.slice(0, 2000)}`)
      }
      const resultVisible = await resultLocator.isVisible().catch(() => false)
      const candidateAttached = await candidateLocator.count() > 0
      if (resultVisible && candidateAttached) break
      await page.waitForTimeout(100)
    }
    if (!await resultLocator.isVisible().catch(() => false)) {
      throw new Error('current result did not become visible before the generation deadline')
    }
    if (await candidateLocator.count() === 0) {
      throw new Error('current result became visible without a candidate surface before the generation deadline')
    }
    if (!await candidateLocator.isVisible()) {
      const details = candidateLocator.locator('xpath=ancestor::details[1]')
      if (await details.count()) await details.locator('summary').click()
    }
    await candidateLocator.waitFor({ timeout })
  } catch (error) {
    throw await diagnosticError(page, label, 'automatic single result was not reflected in the candidate UI', error, startedAt, timeout)
  }
  if (await page.getByLabel('Agent 完整外观方向').count()) {
    throw new Error(`${label} restored a direction-selection surface`)
  }
  return {
    generation_state: await resultLocator.getAttribute('data-generation-state'),
    candidate_visible: true,
  }
}

function responseOutcome(page, pathname, timeout) {
  return waitForCompatHttpResponse(page, {
    method: 'POST',
    path: pathname,
    timeout,
  }).then(
    (response) => ({ response, error: null }),
    (error) => ({ response: null, error }),
  )
}

async function requireCreatedJson(response, label) {
  const bodyText = response.body.text
  if (response.status !== 201) {
    throw new Error(`${label} failed (${response.status}): ${bodyText.slice(0, 2000)}`)
  }
  try {
    return JSON.parse(bodyText)
  } catch (error) {
    throw new Error(`${label} returned invalid JSON: ${error instanceof Error ? error.message : String(error)}; ${bodyText.slice(0, 1000)}`)
  }
}

async function diagnosticError(page, label, stage, error, startedAt, apiTimeoutMs) {
  const bodyText = await page.locator('body').innerText().catch(() => '')
  const viewport = await page.locator('.weapon-viewport').evaluate((element) => ({
    load_state: element.getAttribute('data-blockout-load-state'),
    glb_kind: element.getAttribute('data-blockout-glb-kind'),
    render_source: element.getAttribute('data-blockout-render-source'),
  })).catch(() => null)
  return new Error(
    `${label} ${stage} after ${Date.now() - startedAt}ms (API limit ${apiTimeoutMs}ms): `
    + `${error instanceof Error ? error.message : String(error)}; viewport=${JSON.stringify(viewport)}\n`
    + bodyText.slice(0, 3000),
  )
}

export function agentGeometryTimeoutMs() {
  const configured = Number(process.env.FORGECAD_AGENT_GEOMETRY_TIMEOUT_MS ?? DEFAULT_AGENT_GEOMETRY_TIMEOUT_MS)
  return Number.isFinite(configured) && configured > 0 ? configured : DEFAULT_AGENT_GEOMETRY_TIMEOUT_MS
}

/**
 * Read the product request carried by the browser-loopback app-server frame.
 * Unrelated network requests and non-HTTP protocol frames return null.
 */
export function inspectCompatHttpRequest(request) {
  if (request.method() !== 'POST') return null
  let outerPath
  try { outerPath = new URL(request.url()).pathname } catch { return null }
  if (!APP_SERVER_FRAMES_PATH.test(outerPath)) return null

  let envelope
  try { envelope = JSON.parse(request.postData() ?? '') } catch { return null }
  const frame = envelope?.frame
  const params = frame?.params
  if (
    frame?.jsonrpc !== '2.0'
    || typeof frame.id !== 'string'
    || frame.method !== 'compat/http'
    || params?.schema_version !== 'ForgeCADHttpCompatibilityRequest@1'
    || typeof params.path !== 'string'
    || typeof params.method !== 'string'
    || !Array.isArray(params.headers)
    || typeof params.body !== 'object'
    || params.body === null
  ) return null

  return {
    frame,
    id: frame.id,
    path: params.path,
    method: params.method,
    headers: headerObject(params.headers, 'compat/http request'),
    body: protocolBody(params.body, 'compat/http request'),
  }
}

/**
 * Observe one compat/http exchange and expose the inner product response.
 * The outer `/frames` HTTP 200 is transport-only and is never treated as the
 * business status.
 */
export async function waitForCompatHttpResponse(
  page,
  { method, path, timeout = DEFAULT_PLAYWRIGHT_RESPONSE_TIMEOUT_MS },
) {
  const outerResponse = await page.waitForResponse((response) => {
    const observed = inspectCompatHttpRequest(response.request())
    return observed !== null
      && observed.method === method
      && matchesProductPath(observed.path, path)
  }, { timeout })

  return readCompatHttpResponse(outerResponse)
}

/** Normalize an already observed outer `/frames` response. */
export async function readCompatHttpResponse(outerResponse) {

  const observed = inspectCompatHttpRequest(outerResponse.request())
  if (!observed) throw new Error('compat/http response has no matching product request envelope')
  const outerText = await outerResponse.text().catch(() => '')
  if (outerResponse.status() !== 200) {
    throw new Error(`compat/http outer transport failed (${outerResponse.status()}): ${outerText.slice(0, 2000)}`)
  }

  let envelope
  try { envelope = JSON.parse(outerText) } catch (error) {
    throw new Error(`compat/http outer transport returned invalid JSON: ${error instanceof Error ? error.message : String(error)}; ${outerText.slice(0, 1000)}`)
  }
  const frame = envelope?.frame
  if (frame?.jsonrpc !== '2.0' || frame.id !== observed.id) {
    throw new Error(`compat/http response frame did not match request ${observed.id}: ${outerText.slice(0, 2000)}`)
  }
  if (frame.error) {
    throw new Error(`compat/http ${observed.method} ${observed.path} failed at protocol layer: ${JSON.stringify(frame.error).slice(0, 2000)}`)
  }
  const result = frame.result
  if (
    result?.schema_version !== 'ForgeCADHttpCompatibilityResponse@1'
    || !Number.isInteger(result.status)
    || result.status < 100
    || result.status > 599
    || !Array.isArray(result.headers)
    || typeof result.body !== 'object'
    || result.body === null
  ) {
    throw new Error(`compat/http ${observed.method} ${observed.path} returned an invalid product response: ${JSON.stringify(result).slice(0, 2000)}`)
  }

  return {
    path: observed.path,
    url: new URL(observed.path, outerResponse.url()).toString(),
    method: observed.method,
    request: observed,
    status: result.status,
    ok: result.status >= 200 && result.status < 300,
    headers: headerObject(result.headers, 'compat/http response'),
    body: protocolBody(result.body, 'compat/http response'),
  }
}

/**
 * Response-like compatibility view for older browser smokes. All methods read
 * the normalized inner product response, never the outer `/frames` HTTP 200.
 */
export async function waitForCompatHttpPlaywrightResponse(page, options) {
  const response = await waitForCompatHttpResponse(page, options)
  return {
    ok: () => response.ok,
    status: () => response.status,
    url: () => response.url,
    text: async () => response.body.text,
    json: async () => response.body.json(),
    headers: () => response.headers,
    allHeaders: async () => response.headers,
    request: () => ({
      method: () => response.method,
      url: () => response.url,
      headers: () => response.request.headers,
      postData: () => response.request.body.text,
      postDataJSON: () => response.request.body.json(),
    }),
    compat: response,
  }
}

function matchesProductPath(actual, expected) {
  if (typeof expected === 'string') {
    if (expected.includes('?')) return actual === expected
    try { return new URL(actual, 'http://forgecad.product').pathname === expected } catch { return false }
  }
  if (expected instanceof RegExp) {
    expected.lastIndex = 0
    return expected.test(actual)
  }
  if (typeof expected === 'function') return Boolean(expected(actual))
  throw new TypeError('compat/http path matcher must be a string, RegExp, or function')
}

function headerObject(pairs, label) {
  const headers = {}
  for (const pair of pairs) {
    if (!Array.isArray(pair) || pair.length !== 2 || typeof pair[0] !== 'string' || typeof pair[1] !== 'string') {
      throw new Error(`${label} contains malformed headers`)
    }
    headers[pair[0].toLowerCase()] = pair[1]
  }
  return headers
}

function protocolBody(body, label) {
  if (body.encoding === 'empty' && Object.keys(body).length === 1) {
    return bodyView(Buffer.alloc(0), label)
  }
  if (body.encoding === 'utf8' && typeof body.data === 'string' && Object.keys(body).length === 2) {
    return bodyView(Buffer.from(body.data, 'utf8'), label)
  }
  if (
    body.encoding === 'base64'
    && typeof body.data === 'string'
    && Object.keys(body).length === 2
    && body.data.length % 4 === 0
    && BASE64_PATTERN.test(body.data)
  ) {
    return bodyView(Buffer.from(body.data, 'base64'), label)
  }
  throw new Error(`${label} contains an invalid protocol body: ${JSON.stringify(body).slice(0, 1000)}`)
}

function bodyView(bytes, label) {
  const text = bytes.toString('utf8')
  return {
    bytes,
    text,
    json() {
      try { return JSON.parse(text) } catch (error) {
        throw new Error(`${label} body is not JSON: ${error instanceof Error ? error.message : String(error)}; ${text.slice(0, 1000)}`)
      }
    },
  }
}

export function assertGeometryCompileReadbackQuality(report, label = 'Agent quality report') {
  const readback = report?.compile_readback
  const boundsMatch = JSON.stringify(report?.bounds_mm ?? null) === JSON.stringify(readback?.bounds_mm ?? null)
  const currentProductionReadback = readback?.schema_version === 'GeometryCompileReadback@2'
    && readback?.artifact_profile?.artifact_profile_id === 'production_concept'
  const compatibleLegacyReadback = readback?.schema_version === 'GeometryCompileReadback@1'
  if (
    report?.evidence_source !== 'geometry_compile_readback'
    || (!currentProductionReadback && !compatibleLegacyReadback)
    || readback?.readback_status !== 'passed'
    || report?.triangle_count !== readback?.triangle_count
    || !boundsMatch
    || !/^[a-f0-9]{64}$/.test(readback?.glb_sha256 ?? '')
  ) {
    throw new Error(`${label} is not bound to the compiled GLB readback: ${JSON.stringify({
      evidence_source: report?.evidence_source ?? null,
      report_triangle_count: report?.triangle_count ?? null,
      report_bounds_mm: report?.bounds_mm ?? null,
      readback_schema_version: readback?.schema_version ?? null,
      artifact_profile_id: readback?.artifact_profile?.artifact_profile_id ?? null,
      readback_status: readback?.readback_status ?? null,
      readback_triangle_count: readback?.triangle_count ?? null,
      readback_bounds_mm: readback?.bounds_mm ?? null,
      glb_sha256: readback?.glb_sha256 ?? null,
    })}`)
  }
}

function summary(payload) {
  return JSON.stringify({
    artifact_id: payload?.artifact_id ?? null,
    direction_id: payload?.direction_id ?? null,
    segmentation_status: payload?.segmentation_status ?? null,
    triangle_count: payload?.triangle_count ?? null,
    part_count: Array.isArray(payload?.parts) ? payload.parts.length : null,
    shape_program_schema: payload?.shape_program?.schema_version ?? null,
    has_glb: typeof payload?.glb_base64 === 'string' && payload.glb_base64.length > 0,
  })
}
