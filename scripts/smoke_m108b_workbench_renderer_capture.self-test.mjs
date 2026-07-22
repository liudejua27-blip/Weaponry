#!/usr/bin/env node
import {
  M108B_RENDERER_DEVELOPMENT_PLAN_SCHEMA,
  assertRendererBudget,
  assertSourceGlbHash,
  createDevelopmentEvidence,
  sha256,
  validateDevelopmentPlan,
} from './m108b_renderer_capture_evidence.mjs'

const digest = 'a'.repeat(64)
const fixture = (index) => ({
  fixture_id: `controlled_fixture_${index}`,
  domain_pack_id: 'pack_future_weapon_prop', preflight_order: index,
  file: `fixtures/controlled_${index}.glb`, glb_sha256: digest,
  source_triangle_count: 8000,
  bounds_mm: [1200, 420, 360],
  visual_environment: { environment_id: 'env_forgecad_room_studio_v1', environment_sha256: digest },
})
const plan = {
  schema_version: M108B_RENDERER_DEVELOPMENT_PLAN_SCHEMA,
  evidence_origin: 'workbench_runtime_capture', formal_eligible: false,
  human_benchmark_evidence: false, provider_calls: 0, score_status: 'not_scored',
  fixture_origin: 'controlled_development_fixture', fixtures: Array.from({ length: 12 }, (_, index) => fixture(index + 1)),
}
validateDevelopmentPlan(plan)
validateDevelopmentPlan({ ...structuredClone(plan), fixture_origin: 'recipe_backed_development_preflight' })
const viewport = {
  renderer_geometries: 12, renderer_textures: 10, renderer_draw_calls: 24, renderer_triangles: 8000,
  embedded_pbr_texture_count: 5, estimated_gpu_texture_bytes: 1024,
  load_state: 'ready', render_source: 'glb_pbr', camera_view: 'iso', active_webgl_contexts: 1,
  pbr_color_spaces: 'valid', pbr_sampling_valid: 'true', embedded_pbr_material_count: 1,
  visual_environment_id: 'env_forgecad_room_studio_v1', visual_environment_sha256: digest,
  renderer_generation: 7, blockout_replacement_generation: 12, disposed_blockout_asset_count: 11,
}
const evidence = createDevelopmentEvidence({
  fixture: plan.fixtures[0], executionId: 'renderer_development_test', captureId: 'capture_test',
  png: { file: 'workbench-captures/controlled_fixture_1.png', sha256: digest, byte_size: 10000 }, viewport, previousDisposedAssetCount: 0,
})
if (evidence.formal_eligible !== false || evidence.human_benchmark_evidence !== false || evidence.provider_calls !== 0 || evidence.metrics.triangle_count !== 8000) throw new Error('development evidence positive contract failed')
for (const mutation of [
  () => { const invalid = structuredClone(plan); invalid.formal_eligible = true; validateDevelopmentPlan(invalid) },
  () => { const invalid = structuredClone(plan); invalid.human_benchmark_evidence = true; validateDevelopmentPlan(invalid) },
  () => { const invalid = structuredClone(plan); invalid.provider_calls = 1; validateDevelopmentPlan(invalid) },
  () => { const invalid = structuredClone(plan); invalid.score_status = 'scored'; validateDevelopmentPlan(invalid) },
  () => { const invalid = structuredClone(plan); invalid.fixture_origin = 'formal_visual_fixture'; validateDevelopmentPlan(invalid) },
  () => { const invalid = structuredClone(plan); invalid.fixtures[1].preflight_order = 1; validateDevelopmentPlan(invalid) },
  () => { const invalid = structuredClone(plan); invalid.fixtures[0].source_triangle_count = 24_001; validateDevelopmentPlan(invalid) },
  () => { const invalid = structuredClone(plan); invalid.fixtures[0].bounds_mm = [1200, 0, 360]; validateDevelopmentPlan(invalid) },
  () => assertSourceGlbHash({ ...fixture(1), glb_sha256: 'b'.repeat(64) }, Buffer.from('controlled glb')),
  () => assertRendererBudget({ ...evidence.metrics, triangle_count: 24_001 }, 'controlled_fixture_1'),
  () => createDevelopmentEvidence({ fixture: plan.fixtures[0], executionId: 'x', captureId: 'x', png: evidence.png, viewport: { ...viewport, active_webgl_contexts: 2 }, previousDisposedAssetCount: 0 }),
  () => createDevelopmentEvidence({ fixture: plan.fixtures[0], executionId: 'x', captureId: 'x', png: evidence.png, viewport: { ...viewport, renderer_triangles: 24_129 }, previousDisposedAssetCount: 0 }),
]) {
  try { mutation() } catch { continue }
  throw new Error('M108B renderer negative case was accepted')
}
console.log(JSON.stringify({ schema_version: 'M108BRendererCaptureSelfTest@1', ok: true, evidence_sha256: sha256(Buffer.from(JSON.stringify(evidence))) }))
