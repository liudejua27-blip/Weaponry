import dragonfangProgram from '../../../skills/weaponry-threejs-knife-studio/references/dragonfang-first-slice.json' with { type: 'json' }
import dragonfangLedger from '../../../skills/weaponry-threejs-knife-studio/references/dragonfang-objective-ledger-r5.json' with { type: 'json' }
import upstreamFixture from '../benchmark/dragonfang-like-objects-sculpt-spec.json' with { type: 'json' }
import {
  ThreeAssetStudioController,
  ThreeAssetStudioError,
} from '../src/three-asset-studio.ts'
import { importImg2ThreeJsKnifeSpec } from '../src/img2threejs-object-sculpt-adapter.ts'
import type { KnifeSceneProgram } from '../src/knife-scene-program.ts'
import { createKnifeObjectiveLedger } from '../src/knife-objective-ledger.ts'
import {
  createKnifeObjectiveFunctionV2FromLedger,
  type KnifeObjectiveMetricTargetV2,
} from '../src/knife-objective-function-v2.ts'
import {
  KNIFE_NATIVE_SUCCESSOR_REQUEST_SCHEMA,
  prepareKnifeNativeSuccessor,
} from '../src/knife-native-successor.ts'
import { sha256Hex } from '../src/knife-browser-capture.ts'

const dragonfangKnifeProgram = dragonfangProgram as unknown as KnifeSceneProgram

class NodeFileReader {
  result: ArrayBuffer | string | null = null
  error: unknown = null
  onloadend: (() => void) | null = null
  onerror: ((error: unknown) => void) | null = null

  readAsArrayBuffer(blob: Blob): void {
    blob.arrayBuffer().then(
      (value) => {
        this.result = value
        this.onloadend?.()
      },
      (error) => {
        this.error = error
        this.onerror?.(error)
      },
    )
  }

  readAsDataURL(blob: Blob): void {
    blob.arrayBuffer().then(
      (value) => {
        this.result = `data:${blob.type || 'application/octet-stream'};base64,${base64(value)}`
        this.onloadend?.()
      },
      (error) => {
        this.error = error
        this.onerror?.(error)
      },
    )
  }
}

// GLTFExporter only needs this bounded FileReader surface for Node smoke;
// browsers use their native implementation.
if (typeof globalThis.FileReader === 'undefined') {
  Object.defineProperty(globalThis, 'FileReader', { configurable: true, value: NodeFileReader })
}

const controller = new ThreeAssetStudioController()
const design = await controller.dispatch({
  action: 'knife_design_create',
  request_id: 'smoke-design-create',
  program: dragonfangKnifeProgram,
})
if (design.action !== 'knife_design_create' || design.status !== 'DESIGN_CREATED') throw new Error('knife_design_create did not close')

const candidates = await controller.dispatch({
  action: 'candidates_generate',
  request_id: 'smoke-candidates-generate',
  design_id: design.design_id,
  objective_ledger: dragonfangLedger,
  candidate_count: 3,
})
if (
  candidates.action !== 'candidates_generate'
  || candidates.status !== 'CANDIDATES_GENERATED'
  || candidates.generation_policy !== 'objective-ledger-bounded-one-scope-candidates@2'
  || candidates.objective_ledger_sha256 !== dragonfangLedger.canonical_sha256
  || candidates.baseline_objective_metrics?.status !== 'MEASURED_NOT_REVIEWED'
  || candidates.baseline_objective_metrics.metrics['negative-space-error'] !== 'NOT_COMPUTABLE'
  || typeof candidates.baseline_objective_metrics.metrics['part-id-coverage'] !== 'number'
  || candidates.candidate_plan_status !== 'PROPOSALS_READY'
  || candidates.candidates.length !== 3
  || new Set(candidates.candidates.map((candidate) => candidate.mutation_scope)).size !== 3
  || candidates.candidates.some((candidate) => candidate.changed_parameter_paths.length === 0
    || candidate.proposal_status !== 'REVIEW_ONLY'
    || candidate.visibility.status !== 'MEASURED_NOT_REVIEWED'
    || candidate.part_boundary.schema_version !== 'KnifePartBoundaryMetrics@1'
    || candidate.part_boundary.status !== 'MEASURED_NOT_REVIEWED'
    || candidate.part_boundary.quality_status !== 'NOT_RUN'
    || candidate.guard_fps.schema_version !== 'KnifeGuardFpsMetrics@1'
    || candidate.guard_fps.status !== 'MEASURED_NOT_REVIEWED'
    || candidate.guard_fps.quality_status !== 'NOT_RUN'
    || candidate.objective_metrics.status !== 'MEASURED_NOT_REVIEWED'
    || candidate.objective_metrics.ledger_sha256 !== dragonfangLedger.canonical_sha256
    || candidate.objective_metrics.metrics['negative-space-error'] !== 'NOT_COMPUTABLE'
    || typeof candidate.objective_metrics.metrics['material-id-coverage'] !== 'number'
    || candidate.structural_delta.status !== 'MEASURED_NONZERO'
    || candidate.structural_delta.changed_view_count < 1
    || candidate.structural_delta.silhouette_changed_pixel_count + candidate.structural_delta.part_id_changed_pixel_count < 1)
) throw new Error('candidates_generate did not produce three distinct one-scope review proposals')

const optimized = await controller.dispatch({
  action: 'optimize',
  request_id: 'smoke-optimize',
  design_id: design.design_id,
})
if (
  optimized.action !== 'optimize'
  || optimized.status !== 'REVIEW_ONLY_SELECTION'
  || optimized.objective_ledger_sha256 !== dragonfangLedger.canonical_sha256
  || optimized.objective_evaluation_status !== 'PROXY_ONLY_NOT_LEDGER_ACCEPTANCE'
  || optimized.objective !== 'best-fixed-view-structural-observability-within-budget@1'
  || optimized.decision_basis !== 'NON_VISUAL_STRUCTURAL_RANKING'
  || optimized.quality_status !== 'NOT_RUN'
  || optimized.visual_status !== 'NOT_REVIEWED'
) throw new Error('optimize crossed its structural review-only boundary')
const selectedCandidate = candidates.candidates.find((candidate) => candidate.candidate_id === optimized.selected_candidate_id)
if (
  !selectedCandidate
  || selectedCandidate.triangle_count > dragonfangKnifeProgram.budgets.max_triangles
  || optimized.selected_triangle_count !== selectedCandidate.triangle_count
  || optimized.selected_visibility.deterministic_fingerprint !== selectedCandidate.visibility.deterministic_fingerprint
) throw new Error('optimize did not select a bounded, visibility-measured review candidate')

const baselineValues = candidates.baseline_objective_metrics?.metrics
if (!baselineValues) throw new Error('objective v2 requires evaluator-owned baseline values')
const objectiveTargets: readonly KnifeObjectiveMetricTargetV2[] = [
  metricTarget('negative-space-error', 'objective', 'minimize', false),
  metricTarget('part-id-coverage', 'objective', 'maximize', true),
  metricTarget('material-id-coverage', 'objective', 'maximize', true),
  metricTarget('fps-occupancy', 'objective', 'maximize', true),
  metricTarget('silhouette-iou', 'regression', 'maximize', false),
  metricTarget('boundary-f1', 'regression', 'maximize', false),
  metricTarget('tip-landmark-error', 'regression', 'minimize', false),
  metricTarget('belly-depth-error', 'regression', 'minimize', false),
  metricTarget('thickness-continuity', 'regression', 'maximize', false),
  metricTarget('normal-continuity', 'regression', 'maximize', false),
]
const objectiveFunction = createKnifeObjectiveFunctionV2FromLedger({
  ledger: dragonfangLedger as never,
  objective_id: 'dragonfang-studio-structural-v2',
  metric_targets: objectiveTargets,
  baseline_values: baselineValues,
})
const objectiveEvaluation = await controller.dispatch({
  action: 'optimize',
  request_id: 'smoke-optimize-objective-v2',
  design_id: design.design_id,
  objective_function: objectiveFunction,
})
if (
  objectiveEvaluation.action !== 'optimize'
  || objectiveEvaluation.objective_evaluation_status !== 'KnifeObjectiveFunction@2'
  || objectiveEvaluation.objective_function_sha256 !== objectiveFunction.canonical_sha256
  || objectiveEvaluation.selection_receipt.ledger_sha256 !== dragonfangLedger.canonical_sha256
  || objectiveEvaluation.decision_basis !== 'NON_VISUAL_STRUCTURAL_RANKING'
  || objectiveEvaluation.quality_status !== 'NOT_RUN'
) throw new Error('optimize did not evaluate ObjectiveFunction@2 from evaluator-owned metric receipts')

const built = await controller.dispatch({
  action: 'three_asset_build',
  request_id: 'smoke-three-asset-build',
  candidate_id: optimized.selected_candidate_id,
})
if (built.action !== 'three_asset_build' || built.compiled.part_count !== 13 || built.quality_status !== 'NOT_RUN') {
  throw new Error('three_asset_build did not reuse the bounded compiler')
}

const preview = await controller.dispatch({
  action: 'preview',
  request_id: 'smoke-preview',
  candidate_id: optimized.selected_candidate_id,
  view_ids: ['FRONT', 'REAR_THREE_QUARTER'],
})
if (preview.action !== 'preview' || preview.status !== 'PREVIEW_READY' || preview.receipt.renderer_invoked) {
  throw new Error('preview without a host renderer must remain a non-rendered plan')
}

const lowerCandidate = candidates.candidates[0]
if (!lowerCandidate) throw new Error('candidate list unexpectedly became empty')
const lowerBuilt = await controller.dispatch({
  action: 'three_asset_build',
  request_id: 'smoke-three-asset-build-lower',
  candidate_id: lowerCandidate.candidate_id,
})
const lowerPreview = await controller.dispatch({
  action: 'preview',
  request_id: 'smoke-preview-lower',
  candidate_id: lowerCandidate.candidate_id,
  view_ids: ['FRONT'],
})
if (
  lowerBuilt.action !== 'three_asset_build'
  || lowerPreview.action !== 'preview'
  || lowerPreview.receipt.rig_fingerprint !== preview.receipt.rig_fingerprint
) throw new Error('build did not reuse one full-asset fixed-view calibration across candidates')

const exported = await controller.dispatch({
  action: 'export',
  request_id: 'smoke-export',
  candidate_id: optimized.selected_candidate_id,
})
if (exported.action !== 'export' || exported.status !== 'EXPORTED_GLB' || exported.glb_bytes <= 0 || exported.glb_base64.length <= 0) {
  throw new Error('export did not produce an in-memory GLB')
}

let unsafeRejected = false
try {
  await controller.dispatch({
    action: 'knife_design_create',
    request_id: 'smoke-unsafe-input',
    program: { ...dragonfangKnifeProgram, unknowns: ['https://not-accepted.example'] },
  })
} catch (error) {
  unsafeRejected = error instanceof ThreeAssetStudioError && error.code === 'INVALID_EXTERNAL_INPUT'
}
if (!unsafeRejected) throw new Error('URL input was not rejected by the closed Studio request boundary')

const imported = importImg2ThreeJsKnifeSpec(upstreamFixture)
if (imported.receipt.full_assembly_status !== 'COMPILED' || !imported.program.source_envelope) {
  throw new Error('full compatibility fixture did not produce an immutable source envelope')
}
const compatibilityController = new ThreeAssetStudioController()
const compatibilityDesign = await compatibilityController.dispatch({
  action: 'knife_design_create',
  request_id: 'smoke-compatibility-design',
  program: imported.program,
})
if (compatibilityDesign.action !== 'knife_design_create') throw new Error('compatibility design was not accepted')
const compatibilityLedger = makeLedger(imported.program, {
  ledger_id: 'compatibility-source-review',
  allowed_scope: ['blade-body', 'cutting-edge'],
  frozen_parts: imported.program.parts.filter((part) => part.part_id !== 'blade-body' && part.part_id !== 'cutting-edge').map((part) => part.part_id),
})
const compatibilityCandidates = await compatibilityController.dispatch({
  action: 'candidates_generate',
  request_id: 'smoke-compatibility-candidates',
  design_id: compatibilityDesign.design_id,
  objective_ledger: compatibilityLedger,
  candidate_count: 2,
})
if (
  compatibilityCandidates.action !== 'candidates_generate'
  || compatibilityCandidates.status !== 'SOURCE_REVIEW_ONLY'
  || compatibilityCandidates.candidate_plan_status !== 'REVIEW_ONLY'
  || compatibilityCandidates.candidates.length !== 0
) throw new Error('Studio directly mutated the immutable compatibility baseline')
const nativeSuccessorOptions = {
  successor_asset_id: 'compatibility-native-successor',
  successor_design_basis: 'original-design' as const,
  mutable_part_ids: ['blade-body', 'cutting-edge', 'guard-dragon-head'],
}
const successorPlan = prepareKnifeNativeSuccessor({
  schema_version: KNIFE_NATIVE_SUCCESSOR_REQUEST_SCHEMA,
  source_program: imported.program,
  ...nativeSuccessorOptions,
})
const successorLedger = makeLedger(successorPlan.successor_program, {
  ledger_id: 'compatibility-native-blade',
  allowed_scope: ['blade-body', 'cutting-edge'],
  frozen_parts: successorPlan.successor_program.parts.filter((part) => part.part_id !== 'blade-body' && part.part_id !== 'cutting-edge').map((part) => part.part_id),
})
const promotedCandidates = await compatibilityController.dispatch({
  action: 'candidates_generate',
  request_id: 'smoke-compatibility-native-successor',
  design_id: compatibilityDesign.design_id,
  objective_ledger: successorLedger,
  candidate_count: 2,
  goal_weights: {
    'blade-belly': 0,
    'blade-curvature': 1,
    'blade-tip-taper': 0,
    'blade-thickness': 1,
    'guard-jaw-gap': 0,
    'guard-horn-sweep': 0,
    'grip-taper': 0,
    'grip-segment-rhythm': 0,
    'pommel-hook': 0,
    'relief-depth': 0,
  },
  native_successor: nativeSuccessorOptions,
})
if (
  promotedCandidates.action !== 'candidates_generate'
  || promotedCandidates.status !== 'CANDIDATES_GENERATED'
  || promotedCandidates.native_successor_status !== 'PREPARED_REVIEW_ONLY'
  || !promotedCandidates.native_successor_plan_fingerprint
  || promotedCandidates.candidates.length !== 2
  || promotedCandidates.candidates.some((candidate) => candidate.proposal_status !== 'REVIEW_ONLY')
) throw new Error('compatibility baseline did not fork into a bounded native review successor')

console.log(JSON.stringify({
  schema_version: 'WeaponryThreeAssetStudio@1',
  actions: ['knife_design_create', 'candidates_generate', 'optimize', 'three_asset_build', 'preview', 'export'],
  design_id: design.design_id,
  candidate_count: candidates.candidates.length,
  candidate_plan_fingerprint: candidates.candidate_plan_fingerprint,
  mutation_scopes: candidates.candidates.map((candidate) => candidate.mutation_scope),
  selected_candidate_id: optimized.selected_candidate_id,
  objective: optimized.objective,
  selected_mutation_scope: selectedCandidate.mutation_scope,
  selected_visibility_status: selectedCandidate.visibility.status,
  selected_visibility_fingerprint: selectedCandidate.visibility.deterministic_fingerprint,
  selected_total_visible_view_count: selectedCandidate.visibility.total_visible_view_count,
  selected_missing_part_ids: selectedCandidate.visibility.missing_part_ids,
  selected_structural_delta: selectedCandidate.structural_delta,
  selected_objective_metric_adapter_sha256: selectedCandidate.objective_metrics.deterministic_fingerprint,
  selected_objective_metric_values: selectedCandidate.objective_metrics.metrics,
  objective_v2_selection_status: objectiveEvaluation.status,
  objective_v2_selected_candidate_id: objectiveEvaluation.selected_candidate_id,
  objective_v2_receipt_sha256: objectiveEvaluation.selection_receipt.deterministic_fingerprint,
  objective_ledger_sha256: optimized.objective_ledger_sha256,
  triangles: built.compiled.triangle_count,
  parts: built.compiled.part_count,
  preview_status: preview.status,
  fixed_rig_fingerprint: preview.receipt.rig_fingerprint,
  fixed_rig_shared_across_candidates: lowerPreview.receipt.rig_fingerprint === preview.receipt.rig_fingerprint,
  glb_bytes: exported.glb_bytes,
  glb_sha256: exported.glb_sha256,
  unsafe_url_rejected: unsafeRejected,
  compatibility_status: compatibilityCandidates.status,
  native_successor_status: promotedCandidates.native_successor_status,
  quality_status: optimized.quality_status,
  visual_status: optimized.visual_status,
}))

function metricTarget(
  metric: KnifeObjectiveMetricTargetV2['metric'],
  role: KnifeObjectiveMetricTargetV2['role'],
  direction: KnifeObjectiveMetricTargetV2['direction'],
  required: boolean,
): KnifeObjectiveMetricTargetV2 {
  return {
    metric,
    role,
    direction,
    target_interval: { min: 0, max: 1 },
    minimum_improvement: role === 'objective' ? dragonfangLedger.minimum_improvement : 0,
    regression_limit: 0.05,
    evidence_class: 'structural-proxy',
    required,
  }
}

function base64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer)
  if (typeof btoa === 'function') {
    let binary = ''
    for (let offset = 0; offset < bytes.length; offset += 0x8000) {
      binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000))
    }
    return btoa(binary)
  }
  const bufferConstructor = (globalThis as unknown as {
    Buffer?: { from(value: Uint8Array): { toString(encoding: 'base64'): string } }
  }).Buffer
  if (!bufferConstructor) throw new Error('base64 encoder is unavailable')
  return bufferConstructor.from(bytes).toString('base64')
}

function makeLedger(
  program: KnifeSceneProgram,
  scope: { readonly ledger_id: string; readonly allowed_scope: readonly string[]; readonly frozen_parts: readonly string[] },
) {
  const programSha = program.canonical_sha256 || sha256Hex(canonicalJson({ ...program, canonical_sha256: '' }))
  return createKnifeObjectiveLedger({
    schema_version: 'KnifeObjectiveLedger@1',
    ledger_id: scope.ledger_id,
    revision: 0,
    parent_ledger_sha256: null,
    program_sha256: programSha,
    baseline_candidate_sha256: programSha,
    stage: 'structural',
    allowed_scope: scope.allowed_scope,
    frozen_parts: scope.frozen_parts,
    hypothesis: 'A single bounded structural scope can improve the procedural draft without changing frozen Parts.',
    objective_metrics: ['part-id-coverage'],
    regression_limits: ['fps-occupancy'],
    candidate_budget: 2,
    minimum_improvement: 0.005,
    plateau_limit: 2,
    evidence_sha256: [programSha],
    status: 'active',
  })
}

function canonicalJson(value: unknown): string {
  if (value === null) return 'null'
  if (typeof value === 'string' || typeof value === 'boolean' || typeof value === 'number') return JSON.stringify(value)
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  if (value && typeof value === 'object') {
    const record = value as Record<string, unknown>
    return `{${Object.keys(record).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`).join(',')}}`
  }
  throw new Error('non-canonical smoke value')
}
