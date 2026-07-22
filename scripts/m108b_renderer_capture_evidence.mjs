import { createHash } from 'node:crypto'

export const M108B_RENDERER_CAPTURE_SCHEMA = 'M108BRendererCaptureEvidence@1'
export const M108B_RENDERER_DEVELOPMENT_PLAN_SCHEMA = 'M108BRendererDevelopmentCapturePlan@1'
export const M108B_RENDERER_LIMITS = Object.freeze({
  geometry_count: 72,
  texture_count: 48,
  draw_calls: 96,
  triangle_count: 24_000,
  embedded_pbr_texture_count: 35,
  texture_memory_bytes: 64 * 1024 * 1024,
})
export const M108B_RENDERER_PASS_TRIANGLE_MULTIPLIER = 3
export const M108B_RENDERER_PASS_TRIANGLE_OVERHEAD = 128

export const M108B_DEVELOPMENT_FIXTURE_ORIGINS = Object.freeze([
  'controlled_development_fixture',
  'recipe_backed_development_preflight',
])

export function sha256(value) {
  return createHash('sha256').update(value).digest('hex')
}

export function assertSourceGlbHash(fixture, bytes) {
  const actual = sha256(bytes)
  if (actual !== fixture.glb_sha256) {
    throw new Error(`M108B renderer source GLB hash drift: ${fixture.fixture_id}`)
  }
  return actual
}

export function safeCaptureStem(value) {
  if (typeof value !== 'string' || !/^[a-z0-9][a-z0-9_:-]{2,119}$/i.test(value)) {
    throw new Error('M108B renderer fixture_id must be a stable safe identifier')
  }
  return value.replace(/:/g, '__')
}

export function validateDevelopmentPlan(plan) {
  if (
    !plan || typeof plan !== 'object'
    || plan.schema_version !== M108B_RENDERER_DEVELOPMENT_PLAN_SCHEMA
    || plan.evidence_origin !== 'workbench_runtime_capture'
    || plan.formal_eligible !== false
    || plan.human_benchmark_evidence !== false
    || plan.provider_calls !== 0
    || plan.score_status !== 'not_scored'
    || !M108B_DEVELOPMENT_FIXTURE_ORIGINS.includes(plan.fixture_origin)
    || !Array.isArray(plan.fixtures)
    || plan.fixtures.length !== 12
  ) {
    throw new Error('M108B renderer development capture requires exactly 12 non-formal development fixtures')
  }
  const expectedOrder = new Set()
  const fixtureIds = new Set()
  for (const fixture of plan.fixtures) {
    if (!fixture || typeof fixture !== 'object') throw new Error('M108B renderer fixture must be an object')
    safeCaptureStem(fixture.fixture_id)
    if (fixtureIds.has(fixture.fixture_id)) throw new Error('M108B renderer fixture IDs must be unique')
    fixtureIds.add(fixture.fixture_id)
    const order = Number(fixture.preflight_order)
    if (!Number.isInteger(order) || order < 1 || order > 12 || expectedOrder.has(order)) {
      throw new Error('M108B renderer fixtures must carry a unique 1..12 preflight_order')
    }
    expectedOrder.add(order)
    if (typeof fixture.file !== 'string' || fixture.file.includes('..') || fixture.file.startsWith('/') || !fixture.file.endsWith('.glb')) {
      throw new Error('M108B renderer fixture file must be a relative GLB path')
    }
    if (!/^[a-f0-9]{64}$/.test(fixture.glb_sha256 ?? '')) throw new Error('M108B renderer fixture requires source GLB SHA-256')
    if (
      !Number.isSafeInteger(fixture.source_triangle_count)
      || fixture.source_triangle_count < 1
      || fixture.source_triangle_count > M108B_RENDERER_LIMITS.triangle_count
    ) {
      throw new Error('M108B renderer fixture requires a bounded source GLB triangle count')
    }
    if (
      !Array.isArray(fixture.bounds_mm)
      || fixture.bounds_mm.length !== 3
      || fixture.bounds_mm.some((value) => typeof value !== 'number' || !Number.isFinite(value) || value <= 0)
    ) {
      throw new Error('M108B renderer fixture requires three positive finite readback bounds')
    }
    if (fixture.visual_environment?.environment_id !== 'env_forgecad_room_studio_v1' || !/^[a-f0-9]{64}$/.test(fixture.visual_environment?.environment_sha256 ?? '')) {
      throw new Error('M108B renderer fixture requires the fixed workbench environment')
    }
  }
  if (expectedOrder.size !== 12) throw new Error('M108B renderer preflight order is incomplete')
  return [...plan.fixtures].sort((left, right) => left.preflight_order - right.preflight_order)
}

export function metricsFromViewport(facts, sourceTriangleCount) {
  return {
    geometry_count: facts.renderer_geometries,
    texture_count: facts.renderer_textures,
    draw_calls: facts.renderer_draw_calls,
    // Formal M108B `triangle_count` is an immutable GLB/readback fact.  The
    // Three.js render counter includes the main pass, shadow pass and a second
    // transparent-double-sided pass, so it is recorded separately below.
    triangle_count: sourceTriangleCount,
    embedded_pbr_texture_count: facts.embedded_pbr_texture_count,
    texture_memory_bytes: facts.estimated_gpu_texture_bytes,
  }
}

export function assertRendererBudget(metrics, fixtureId) {
  for (const [field, limit] of Object.entries(M108B_RENDERER_LIMITS)) {
    const value = metrics[field]
    if (!Number.isSafeInteger(value) || value <= 0 || value > limit) {
      throw new Error(`M108B renderer budget exceeded: ${fixtureId}; ${field}=${value}, limit=${limit}`)
    }
  }
}

export function createDevelopmentEvidence({ fixture, executionId, captureId, png, viewport, previousDisposedAssetCount }) {
  const metrics = metricsFromViewport(viewport, fixture.source_triangle_count)
  assertRendererBudget(metrics, fixture.fixture_id)
  const submittedTriangleLimit = (
    fixture.source_triangle_count * M108B_RENDERER_PASS_TRIANGLE_MULTIPLIER
    + M108B_RENDERER_PASS_TRIANGLE_OVERHEAD
  )
  if (
    viewport.load_state !== 'ready' || viewport.render_source !== 'glb_pbr'
    || viewport.camera_view !== 'iso' || viewport.active_webgl_contexts !== 1
    || viewport.pbr_color_spaces !== 'valid' || viewport.pbr_sampling_valid !== 'true'
    || viewport.embedded_pbr_material_count < 1
  ) {
    throw new Error(`M108B renderer runtime contract failed: ${fixture.fixture_id}`)
  }
  if (
    !Number.isSafeInteger(viewport.renderer_triangles)
    || viewport.renderer_triangles < fixture.source_triangle_count
    || viewport.renderer_triangles > submittedTriangleLimit
  ) {
    throw new Error(`M108B renderer submitted-triangle budget failed: ${fixture.fixture_id}`)
  }
  if (!Number.isSafeInteger(viewport.disposed_blockout_asset_count) || viewport.disposed_blockout_asset_count < previousDisposedAssetCount) {
    throw new Error(`M108B renderer asset cleanup counter regressed: ${fixture.fixture_id}`)
  }
  if (fixture.preflight_order > 1 && viewport.disposed_blockout_asset_count <= previousDisposedAssetCount) {
    throw new Error(`M108B renderer did not dispose the previous asset: ${fixture.fixture_id}`)
  }
  return {
    schema_version: M108B_RENDERER_CAPTURE_SCHEMA,
    evidence_origin: 'workbench_runtime_capture',
    formal_eligible: false,
    human_benchmark_evidence: false,
    provider_calls: 0,
    score_status: 'not_scored',
    capture_id: captureId,
    execution_id: executionId,
    fixture_id: fixture.fixture_id,
    preflight_order: fixture.preflight_order,
    source_glb_sha256: fixture.glb_sha256,
    png,
    metrics,
    runtime_metrics: {
      submitted_triangle_count: viewport.renderer_triangles,
      submitted_triangle_limit: submittedTriangleLimit,
      pass_multiplier_limit: M108B_RENDERER_PASS_TRIANGLE_MULTIPLIER,
      fixed_scene_overhead: M108B_RENDERER_PASS_TRIANGLE_OVERHEAD,
    },
    renderer_contract: {
      renderer_id: 'ForgeCADWorkbenchRenderer@1',
      environment_id: viewport.visual_environment_id,
      environment_sha256: viewport.visual_environment_sha256,
      camera_preset: viewport.camera_view,
      load_state: viewport.load_state,
      render_source: viewport.render_source,
      single_webgl_context: viewport.active_webgl_contexts === 1,
      embedded_pbr_material_count: viewport.embedded_pbr_material_count,
    },
    cleanup: {
      renderer_generation: viewport.renderer_generation,
      replacement_generation: viewport.blockout_replacement_generation,
      disposed_blockout_asset_count: viewport.disposed_blockout_asset_count,
      previous_disposed_blockout_asset_count: previousDisposedAssetCount,
    },
  }
}
