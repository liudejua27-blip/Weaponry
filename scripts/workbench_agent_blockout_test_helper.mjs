const UI_SETTLE_TIMEOUT_MS = 20_000
const DEFAULT_AGENT_GEOMETRY_TIMEOUT_MS = 90_000

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

function responseOutcome(page, pathname, timeout) {
  return page.waitForResponse(
    (response) => response.request().method() === 'POST' && new URL(response.url()).pathname === pathname,
    { timeout },
  ).then(
    (response) => ({ response, error: null }),
    (error) => ({ response: null, error }),
  )
}

async function requireCreatedJson(response, label) {
  const bodyText = await response.text().catch(() => '')
  if (response.status() !== 201) {
    throw new Error(`${label} failed (${response.status()}): ${bodyText.slice(0, 2000)}`)
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

export function assertGeometryCompileReadbackQuality(report, label = 'Agent quality report') {
  const readback = report?.compile_readback
  const boundsMatch = JSON.stringify(report?.bounds_mm ?? null) === JSON.stringify(readback?.bounds_mm ?? null)
  if (
    report?.evidence_source !== 'geometry_compile_readback'
    || readback?.schema_version !== 'GeometryCompileReadback@1'
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
