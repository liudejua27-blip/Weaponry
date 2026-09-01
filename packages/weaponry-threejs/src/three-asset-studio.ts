import * as THREE from 'three'

import {
  KNIFE_VIEW_IDS,
  calibrateKnifeViewRig,
  createKnifeViewCamera,
  createKnifeViewRigFromCalibration,
  evaluateKnifeRig,
  getKnifeView,
  type KnifeEightViewEvaluation,
  type KnifeViewCalibration,
  type KnifeViewId,
  type KnifeViewRig,
} from './knife-view-evaluation.ts'
import {
  captureKnifeAovs,
  sha256Hex,
  type KnifeBrowserCaptureResult,
} from './knife-browser-capture.ts'
import {
  fingerprintKnifePreviewScene,
  hashKnifeBrowserPreviewReceipt,
  makeKnifeBrowserCameraReceipt,
  overrideKnifePreviewUuid,
  stableKnifePreviewUuid,
  type KnifeBrowserCameraReceipt,
  type KnifeBrowserViewReceipt,
  type KnifePreviewViewport,
} from './knife-browser-preview.ts'
import {
  compileKnifeSceneProgram,
  type CompiledKnifePart,
  type CompiledKnifeScene,
} from './knife-scene-compiler.ts'
import type { KnifeSceneProgram } from './knife-scene-program.ts'
import {
  generateKnifeKnowledgeCandidatePlan,
  normalizeKnifeKnowledgeGoalWeights,
  type KnifeKnowledgeCandidate,
  type KnifeKnowledgeCandidatePlan,
  type KnifeKnowledgeGoalWeights,
  type KnifeKnowledgeMutationScope,
} from './knife-knowledge-candidate-generator.ts'
import {
  generateKnifeObjectiveLedgerCandidates,
  validateKnifeObjectiveLedger,
  type KnifeObjectiveLedger,
} from './knife-objective-ledger.ts'
import {
  measureKnifeObjectiveMetricValues,
  type KnifeObjectiveMetricAdapterInput,
  type KnifeObjectiveMetricAdapterReceipt,
} from './knife-objective-metric-adapter.ts'
import {
  measureKnifeObjectiveMetricValuesV2,
  type KnifeObjectiveMetricAdapterAnyReceipt,
} from './knife-objective-metric-adapter-v2.ts'
import { isIntrinsicKnifeObjectiveMetric } from './knife-objective-metric-catalog.ts'
import {
  KNIFE_OBJECTIVE_FUNCTION_V2_SCHEMA,
  createKnifeObjectiveCandidateV2,
  createKnifeObjectiveFunctionV2FromLedger,
  evaluateKnifeObjectiveFunctionV2,
  validateKnifeObjectiveFunctionV2,
  type KnifeObjectiveFunctionV2,
  type KnifeObjectiveSelectionReceiptV2,
} from './knife-objective-function-v2.ts'
import {
  KNIFE_PART_VISIBILITY_METRICS_STATUS,
  measureKnifePartVisibilityMetrics,
  type KnifePartVisibilityMetrics,
} from './knife-part-visibility-metrics.ts'
import {
  KNIFE_PART_BOUNDARY_METRICS_SCHEMA,
  KNIFE_PART_BOUNDARY_METRICS_STATUS,
  measureKnifePartBoundaryMetrics,
  type KnifePartBoundaryMetrics,
} from './knife-part-boundary-metrics.ts'
import {
  KNIFE_GUARD_FPS_METRICS_SCHEMA,
  KNIFE_GUARD_FPS_METRICS_STATUS,
  measureKnifeGuardFpsMetrics,
  type KnifeGuardFpsComputability,
  type KnifeGuardFpsMetricValue,
  type KnifeGuardFpsMetrics,
} from './knife-guard-fps-metrics.ts'
import {
  KNIFE_NATIVE_SUCCESSOR_REQUEST_SCHEMA,
  prepareKnifeNativeSuccessor,
  type KnifeNativeSuccessorPlan,
} from './knife-native-successor.ts'
import {
  KNIFE_PREVIEW_MANIFEST_SCHEMA,
  parseKnifePreviewManifest,
  type KnifePreviewManifest,
} from './knife-preview-manifest.ts'

/**
 * Small in-process browser-facing controller for the Three.js route.
 *
 * This is intentionally not a Runtime, Store, MCP adapter, or persistence
 * layer. Requests contain only closed JSON-like values. A renderer is an
 * explicitly supplied host capability and is never accepted inside a
 * request, so URL/path/script/network input cannot reach the compiler.
 */

export const THREE_ASSET_STUDIO_SCHEMA = 'WeaponryThreeAssetStudio@1' as const
export const THREE_ASSET_STUDIO_ACTIONS = Object.freeze([
  'knife_design_create',
  'candidates_generate',
  'optimize',
  'three_asset_build',
  'preview',
  'export',
] as const)

export type ThreeAssetStudioActionName = (typeof THREE_ASSET_STUDIO_ACTIONS)[number]

export type ThreeAssetStudioErrorCode =
  | 'INVALID_REQUEST'
  | 'INVALID_EXTERNAL_INPUT'
  | 'DESIGN_NOT_FOUND'
  | 'CANDIDATE_NOT_FOUND'
  | 'ASSET_NOT_BUILT'
  | 'STATE_CONFLICT'
  | 'COMPILATION_FAILED'
  | 'PREVIEW_RENDERER_REQUIRED'
  | 'PREVIEW_CAPTURE_RENDERER_REQUIRED'
  | 'GLB_EXPORT_FAILED'
  | 'BUDGET_EXCEEDED'

export class ThreeAssetStudioError extends Error {
  readonly code: ThreeAssetStudioErrorCode

  constructor(code: ThreeAssetStudioErrorCode, message: string) {
    super(`${code}: ${message}`)
    this.name = 'ThreeAssetStudioError'
    this.code = code
  }
}

export interface KnifeDesignCreateRequest {
  readonly action: 'knife_design_create'
  readonly request_id: string
  readonly program: KnifeSceneProgram
}

export interface CandidatesGenerateRequest {
  readonly action: 'candidates_generate'
  readonly request_id: string
  readonly design_id: string
  /** Immutable authority for program, scope, frozen Parts, budget, and evidence. */
  readonly objective_ledger: KnifeObjectiveLedger
  readonly candidate_count?: number
  /** Closed, immutable weights for this candidate batch. */
  readonly goal_weights?: KnifeKnowledgeGoalWeights
  readonly seed?: number
  readonly native_successor?: StudioNativeSuccessorOptions
}

export interface StudioNativeSuccessorOptions {
  readonly successor_asset_id: string
  readonly successor_design_basis: 'authorized-reference-inspired' | 'original-design'
  readonly mutable_part_ids: readonly string[]
}

export interface OptimizeRequest {
  readonly action: 'optimize'
  readonly request_id: string
  readonly design_id: string
  readonly candidate_id?: string
  /** Optional closed objective semantics. Omit for the legacy structural proxy ranker. */
  readonly objective_function?: KnifeObjectiveFunctionV2
}

export interface ThreeAssetBuildRequest {
  readonly action: 'three_asset_build'
  readonly request_id: string
  readonly candidate_id: string
}

export interface PreviewRequest {
  readonly action: 'preview'
  readonly request_id: string
  readonly candidate_id: string
  readonly view_ids?: readonly KnifeViewId[]
  readonly capture_aovs?: boolean
}

export interface ExportRequest {
  readonly action: 'export'
  readonly request_id: string
  readonly candidate_id: string
}

export type ThreeAssetStudioRequest =
  | KnifeDesignCreateRequest
  | CandidatesGenerateRequest
  | OptimizeRequest
  | ThreeAssetBuildRequest
  | PreviewRequest
  | ExportRequest

/** Host-owned capabilities; neither field is part of the closed request. */
export interface ThreeAssetStudioExecutionContext {
  readonly renderer?: THREE.WebGLRenderer
  readonly capture_renderer?: THREE.WebGLRenderer
}

export interface ThreeAssetStudioPartSummary {
  readonly part_id: string
  readonly material_zone_id: string
  readonly surface_role: string
  readonly assembly_primitive?: string
  readonly triangles: number
}

export interface ThreeAssetStudioCompiledSummary {
  readonly deterministic_fingerprint: string
  readonly triangle_count: number
  readonly part_count: number
  readonly assembly_part_count: number
  readonly assembly_status: 'NOT_PRESENT' | 'COMPILED'
  readonly longitudinal_segments: number
  readonly parts: readonly ThreeAssetStudioPartSummary[]
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
}

export interface KnifeDesignCreateResult {
  readonly schema_version: typeof THREE_ASSET_STUDIO_SCHEMA
  readonly action: 'knife_design_create'
  readonly request_id: string
  readonly status: 'DESIGN_CREATED'
  readonly design_id: string
  readonly asset_id: string
  readonly source_program_fingerprint: string
  readonly baseline: ThreeAssetStudioCompiledSummary
  readonly quality_status: 'NOT_RUN'
}

export interface StudioCandidateSummary {
  readonly candidate_id: string
  readonly design_id: string
  readonly longitudinal_segments: number
  readonly triangle_count: number
  readonly part_count: number
  readonly deterministic_fingerprint: string
  readonly mutation_scope: KnifeKnowledgeMutationScope
  readonly changed_parameter_paths: readonly string[]
  readonly candidate_plan_fingerprint: string
  readonly objective_ledger_sha256: string
  readonly visibility: StudioVisibilitySummary
  readonly part_boundary: StudioPartBoundarySummary
  readonly guard_fps: StudioGuardFpsSummary
  readonly structural_delta: StudioStructuralDeltaSummary
  readonly objective_metrics: KnifeObjectiveMetricAdapterAnyReceipt
  readonly proposal_status: 'REVIEW_ONLY'
  readonly state: 'CANDIDATE_GENERATED'
  readonly quality_status: 'NOT_RUN'
}

export interface StudioStructuralDeltaSummary {
  readonly schema_version: 'KnifeCandidateStructuralDelta@1'
  readonly baseline_source_fingerprint: string
  readonly candidate_source_fingerprint: string
  readonly changed_view_count: number
  readonly silhouette_changed_pixel_count: number
  readonly part_id_changed_pixel_count: number
  readonly minimum_changed_pixel_count: 1
  readonly status: 'MEASURED_NONZERO' | 'REJECTED_NO_VISIBLE_DELTA'
  readonly quality_status: 'NOT_RUN'
}

export interface StudioVisibilitySummary {
  readonly schema_version: 'KnifePartVisibilityMetrics@1'
  readonly deterministic_fingerprint: string
  readonly visible_part_count: number
  readonly total_part_count: number
  readonly missing_part_ids: readonly string[]
  readonly underexposed_part_ids: readonly string[]
  readonly total_visible_view_count: number
  readonly status: typeof KNIFE_PART_VISIBILITY_METRICS_STATUS
  readonly quality_status: 'NOT_RUN'
}

/**
 * Compact candidate-facing projection of the fixed-mask boundary metrics.
 * These are structural observations only; they never enter a quality result.
 */
export interface StudioPartBoundarySummary {
  readonly schema_version: typeof KNIFE_PART_BOUNDARY_METRICS_SCHEMA
  readonly deterministic_fingerprint: string
  readonly part_count: number
  readonly semantic_adjacency_count: number
  readonly total_boundary_pixel_count: number
  /** Mean of each part's eight-view boundary-length average. */
  readonly mean_boundary_length_normalized: number
  readonly total_connected_island_count: number
  readonly status: typeof KNIFE_PART_BOUNDARY_METRICS_STATUS
  readonly quality_status: 'NOT_RUN'
}

/**
 * Compact candidate-facing projection of guard/FPS mask metrics. The
 * negative-space values are visible-opening proxies and FPS values are
 * occupancy/occlusion observations, not visual-quality or acceptance gates.
 */
export interface StudioGuardFpsSummary {
  readonly schema_version: typeof KNIFE_GUARD_FPS_METRICS_SCHEMA
  readonly deterministic_fingerprint: string
  readonly guard_part_id: string | 'NOT_COMPUTABLE'
  readonly negative_space_computed_view_count: number
  readonly negative_space_partial_view_count: number
  readonly negative_space_not_computable_view_count: number
  readonly fps_hold_computability: KnifeGuardFpsComputability
  readonly fps_hold_asset_bbox_width_fraction: KnifeGuardFpsMetricValue
  readonly fps_hold_asset_bbox_height_fraction: KnifeGuardFpsMetricValue
  readonly fps_hold_tip_safe_margin_fraction: KnifeGuardFpsMetricValue
  readonly fps_hold_guard_occlusion_ratio: KnifeGuardFpsMetricValue
  readonly status: typeof KNIFE_GUARD_FPS_METRICS_STATUS
  readonly quality_status: 'NOT_RUN'
}

export interface CandidatesGenerateResult {
  readonly schema_version: typeof THREE_ASSET_STUDIO_SCHEMA
  readonly action: 'candidates_generate'
  readonly request_id: string
  readonly status: 'CANDIDATES_GENERATED' | 'CANDIDATES_REUSED' | 'SOURCE_REVIEW_ONLY'
  readonly design_id: string
  readonly candidates: readonly StudioCandidateSummary[]
  readonly generation_policy: 'objective-ledger-bounded-one-scope-candidates@2'
  readonly objective_ledger_sha256: string
  /** Source-program metrics measured under the same fixed rig as candidates. */
  readonly baseline_objective_metrics: KnifeObjectiveMetricAdapterAnyReceipt | null
  readonly candidate_plan_status: 'PROPOSALS_READY' | 'REVIEW_ONLY'
  readonly candidate_plan_fingerprint: string
  readonly goal_weights: KnifeKnowledgeGoalWeights
  readonly seed: number
  readonly rejection_reason: string | null
  readonly native_successor_status: 'NOT_REQUIRED' | 'REQUIRED' | 'PREPARED_REVIEW_ONLY'
  readonly native_successor_plan_fingerprint: string | null
  readonly quality_status: 'NOT_RUN'
}

export interface OptimizeProxyResult {
  readonly schema_version: typeof THREE_ASSET_STUDIO_SCHEMA
  readonly action: 'optimize'
  readonly request_id: string
  readonly status: 'REVIEW_ONLY_SELECTION'
  readonly design_id: string
  readonly selected_candidate_id: string
  readonly objective_ledger_sha256: string
  readonly objective_metrics: readonly string[]
  readonly objective_evaluation_status: 'PROXY_ONLY_NOT_LEDGER_ACCEPTANCE'
  readonly objective: 'best-fixed-view-structural-observability-within-budget@1'
  readonly candidate_ids: readonly string[]
  readonly selected_triangle_count: number
  readonly selected_visibility: StudioVisibilitySummary
  readonly decision_basis: 'NON_VISUAL_STRUCTURAL_RANKING'
  readonly quality_status: 'NOT_RUN'
  readonly visual_status: 'NOT_REVIEWED'
}

export interface OptimizeObjectiveV2Result {
  readonly schema_version: typeof THREE_ASSET_STUDIO_SCHEMA
  readonly action: 'optimize'
  readonly request_id: string
  readonly status: KnifeObjectiveSelectionReceiptV2['selection_status']
  readonly design_id: string
  readonly selected_candidate_id: string | null
  readonly objective_ledger_sha256: string
  readonly objective_function_sha256: string
  readonly objective_evaluation_status: typeof KNIFE_OBJECTIVE_FUNCTION_V2_SCHEMA
  readonly candidate_ids: readonly string[]
  readonly selection_receipt: KnifeObjectiveSelectionReceiptV2
  readonly decision_basis: 'NON_VISUAL_STRUCTURAL_RANKING'
  readonly quality_status: 'NOT_RUN'
  readonly visual_status: KnifeObjectiveSelectionReceiptV2['visual_status']
}

export type OptimizeResult = OptimizeProxyResult | OptimizeObjectiveV2Result

export interface ThreeAssetBuildResult {
  readonly schema_version: typeof THREE_ASSET_STUDIO_SCHEMA
  readonly action: 'three_asset_build'
  readonly request_id: string
  readonly status: 'ASSET_BUILT' | 'ASSET_REUSED'
  readonly candidate_id: string
  readonly asset_id: string
  readonly scene_fingerprint: string
  readonly compiled: ThreeAssetStudioCompiledSummary
  readonly quality_status: 'NOT_RUN'
}

export interface StudioPreviewViewReceipt {
  readonly view_id: KnifeViewId
  readonly camera: KnifeBrowserCameraReceipt
  readonly viewport: KnifePreviewViewport
  readonly renderer_invoked: boolean
  readonly render_status: 'RENDERED' | 'NOT_RUN'
  readonly quality_status: 'NOT_RUN'
}

export interface StudioPreviewReceipt {
  readonly schema_version: 'WeaponryThreeAssetStudioPreviewReceipt@1'
  readonly route: 'weaponry-three-asset-studio@1'
  readonly asset_id: string
  readonly candidate_id: string
  readonly scene_fingerprint: string
  readonly rig_schema_version: 'KnifeFixedEightViewRig@1'
  readonly rig_id: 'knife-fixed-eight-view@1'
  readonly rig_fingerprint: string
  readonly manifest: KnifePreviewManifest
  readonly selected_view_ids: readonly KnifeViewId[]
  readonly views: readonly StudioPreviewViewReceipt[]
  readonly renderer_invoked: boolean
  readonly render_status: 'RENDERED' | 'NOT_RUN'
  readonly capture_status: 'NOT_REQUESTED' | 'CAPTURED'
  readonly capture_manifest_sha256?: string
  readonly visual_status: 'NOT_REVIEWED'
  readonly quality_status: 'NOT_RUN'
  readonly deterministic_fingerprint: string
}

export interface PreviewResult {
  readonly schema_version: typeof THREE_ASSET_STUDIO_SCHEMA
  readonly action: 'preview'
  readonly request_id: string
  readonly status: 'PREVIEW_RENDERED' | 'PREVIEW_READY'
  readonly candidate_id: string
  readonly receipt: StudioPreviewReceipt
  readonly capture?: KnifeBrowserCaptureResult
  readonly quality_status: 'NOT_RUN'
}

export interface ExportResult {
  readonly schema_version: typeof THREE_ASSET_STUDIO_SCHEMA
  readonly action: 'export'
  readonly request_id: string
  readonly status: 'EXPORTED_GLB'
  readonly candidate_id: string
  readonly asset_id: string
  readonly encoding: 'base64'
  readonly glb_base64: string
  readonly glb_sha256: string
  readonly glb_bytes: number
  readonly triangles: number
  readonly part_ids: readonly string[]
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
}

export type ThreeAssetStudioActionResult =
  | KnifeDesignCreateResult
  | CandidatesGenerateResult
  | OptimizeResult
  | ThreeAssetBuildResult
  | PreviewResult
  | ExportResult

interface DesignState {
  readonly design_id: string
  readonly program: KnifeSceneProgram
  readonly baseline: ThreeAssetStudioCompiledSummary
  readonly baseline_calibration: KnifeViewCalibration
  candidate_plan?: KnifeKnowledgeCandidatePlan
  objective_ledger?: KnifeObjectiveLedger
  baseline_objective_metrics?: KnifeObjectiveMetricAdapterAnyReceipt
  native_successor_plan?: KnifeNativeSuccessorPlan
  native_successor_request_fingerprint?: string
  candidates?: Map<string, CandidateState>
  selected_candidate_id?: string
}

interface CandidateState {
  readonly summary: StudioCandidateSummary
  readonly program: KnifeSceneProgram
  readonly knowledge_candidate: KnifeKnowledgeCandidate
  readonly visibility_metrics: KnifePartVisibilityMetrics
  readonly part_boundary_metrics: KnifePartBoundaryMetrics
  readonly guard_fps_metrics: KnifeGuardFpsMetrics
  readonly objective_metric_receipt: KnifeObjectiveMetricAdapterAnyReceipt
  readonly compiled: CompiledKnifeScene
}

interface AssetState {
  readonly candidate: CandidateState
  readonly compiled: CompiledKnifeScene
  readonly scene: THREE.Scene
  readonly rig: KnifeViewRig
  readonly scene_fingerprint: string
}

const STABLE_ID_PATTERN = /^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/
const MAX_DESIGNS = 4
const MAX_CANDIDATES_PER_DESIGN = 4
const MAX_ASSETS = 4
const MAX_GLB_BYTES = 32 * 1024 * 1024
const DEFAULT_KNIFE_GOAL_WEIGHTS = Object.freeze({
  'blade-belly': 1,
  'blade-curvature': 1,
  'blade-tip-taper': 1,
  'blade-thickness': 0.35,
  'guard-jaw-gap': 0.85,
  'guard-horn-sweep': 0.4,
  'grip-taper': 0.3,
  'grip-segment-rhythm': 0.3,
  'pommel-hook': 0.25,
  'relief-depth': 0.2,
}) satisfies KnifeKnowledgeGoalWeights
const DEFAULT_CANDIDATE_SEED = 0x4b4e4946
const UNSAFE_KEY_PATTERN = /(?:^|[_-])(url|path|script|network|command|shell|secret|token|env)(?:$|[_-])/i
const UNSAFE_VALUE_PATTERN = /(?:https?:\/\/|file:\/\/|data:|javascript:|<script|\beval\s*\(|\bfetch\s*\(|\brequire\s*\(|\bshell\b)/i

function measureStudioObjectiveMetrics(
  input: KnifeObjectiveMetricAdapterInput,
): KnifeObjectiveMetricAdapterAnyReceipt {
  const usesIntrinsicMetrics = [...input.ledger.objective_metrics, ...input.ledger.regression_limits]
    .some(isIntrinsicKnifeObjectiveMetric)
  return usesIntrinsicMetrics
    ? measureKnifeObjectiveMetricValuesV2(input)
    : measureKnifeObjectiveMetricValues(input)
}

export function parseThreeAssetStudioRequest(value: unknown): ThreeAssetStudioRequest {
  const record = requestRecord(value, 'request')
  const action = record.action
  if (!isActionName(action)) throw new ThreeAssetStudioError('INVALID_REQUEST', 'action is not one of the six bounded Studio actions')
  const requestId = stableId(record.request_id, 'request_id')

  switch (action) {
    case 'knife_design_create':
      exactKeys(record, ['action', 'request_id', 'program'], 'knife_design_create request')
      return Object.freeze({
        action,
        request_id: requestId,
        program: parseProgram(record.program),
      })
    case 'candidates_generate':
      exactKeys(record, ['action', 'request_id', 'design_id', 'objective_ledger'], 'candidates_generate request', ['candidate_count', 'goal_weights', 'seed', 'native_successor'])
      return Object.freeze({
        action,
        request_id: requestId,
        design_id: stableId(record.design_id, 'design_id'),
        objective_ledger: parseObjectiveLedger(record.objective_ledger),
        ...(record.candidate_count === undefined ? {} : { candidate_count: boundedCandidateCount(record.candidate_count) }),
        ...(record.goal_weights === undefined ? {} : { goal_weights: parseGoalWeights(record.goal_weights) }),
        ...(record.seed === undefined ? {} : { seed: boundedSeed(record.seed) }),
        ...(record.native_successor === undefined ? {} : { native_successor: parseNativeSuccessorOptions(record.native_successor) }),
      })
    case 'optimize':
      exactKeys(record, ['action', 'request_id', 'design_id'], 'optimize request', ['candidate_id', 'objective_function'])
      if (record.candidate_id !== undefined && record.objective_function !== undefined) {
        throw new ThreeAssetStudioError('INVALID_REQUEST', 'candidate_id cannot bypass an objective_function decision')
      }
      return Object.freeze({
        action,
        request_id: requestId,
        design_id: stableId(record.design_id, 'design_id'),
        ...(record.candidate_id === undefined ? {} : { candidate_id: stableId(record.candidate_id, 'candidate_id') }),
        ...(record.objective_function === undefined ? {} : { objective_function: parseObjectiveFunction(record.objective_function) }),
      })
    case 'three_asset_build':
      exactKeys(record, ['action', 'request_id', 'candidate_id'], 'three_asset_build request')
      return Object.freeze({
        action,
        request_id: requestId,
        candidate_id: stableId(record.candidate_id, 'candidate_id'),
      })
    case 'preview':
      exactKeys(record, ['action', 'request_id', 'candidate_id'], 'preview request', ['view_ids', 'capture_aovs'])
      return Object.freeze({
        action,
        request_id: requestId,
        candidate_id: stableId(record.candidate_id, 'candidate_id'),
        ...(record.view_ids === undefined ? {} : { view_ids: normalizeViewIds(record.view_ids) }),
        ...(record.capture_aovs === undefined ? {} : { capture_aovs: boundedBoolean(record.capture_aovs, 'capture_aovs') }),
      })
    case 'export':
      exactKeys(record, ['action', 'request_id', 'candidate_id'], 'export request')
      return Object.freeze({
        action,
        request_id: requestId,
        candidate_id: stableId(record.candidate_id, 'candidate_id'),
      })
  }
}

export class ThreeAssetStudioController {
  private readonly designs = new Map<string, DesignState>()
  private readonly assets = new Map<string, AssetState>()

  async dispatch(
    input: unknown,
    context: ThreeAssetStudioExecutionContext = {},
  ): Promise<ThreeAssetStudioActionResult> {
    const request = parseThreeAssetStudioRequest(input)
    switch (request.action) {
      case 'knife_design_create':
        return this.knifeDesignCreate(request)
      case 'candidates_generate':
        return this.candidatesGenerate(request)
      case 'optimize':
        return this.optimize(request)
      case 'three_asset_build':
        return this.threeAssetBuild(request)
      case 'preview':
        return this.preview(request, context)
      case 'export':
        return this.export(request)
    }
  }

  private knifeDesignCreate(request: KnifeDesignCreateRequest): KnifeDesignCreateResult {
    if (this.designs.size >= MAX_DESIGNS && !this.designs.has(designIdFor(request.program))) {
      throw new ThreeAssetStudioError('STATE_CONFLICT', `in-process design limit is ${MAX_DESIGNS}`)
    }
    const designId = designIdFor(request.program)
    const existing = this.designs.get(designId)
    if (existing) {
      return Object.freeze({
        schema_version: THREE_ASSET_STUDIO_SCHEMA,
        action: request.action,
        request_id: request.request_id,
        status: 'DESIGN_CREATED',
        design_id: existing.design_id,
        asset_id: existing.program.asset_id,
        source_program_fingerprint: existing.baseline.deterministic_fingerprint,
        baseline: existing.baseline,
        quality_status: 'NOT_RUN',
      })
    }

    const program = cloneAndFreezeProgram(request.program)
    let compiled: CompiledKnifeScene
    try {
      compiled = compileKnifeSceneProgram(program)
    } catch (error) {
      throw compilationError(error)
    }
    const baseline = summarizeCompiledScene(compiled)
    const baselineCalibration = calibrateKnifeViewRig(compiled, {
      focus_part_ids: compiled.parts.map((part) => part.part_id),
    }).calibration
    disposeCompiledScene(compiled)
    this.designs.set(designId, {
      design_id: designId,
      program,
      baseline,
      baseline_calibration: baselineCalibration,
    })
    return Object.freeze({
      schema_version: THREE_ASSET_STUDIO_SCHEMA,
      action: request.action,
      request_id: request.request_id,
      status: 'DESIGN_CREATED',
      design_id: designId,
      asset_id: program.asset_id,
      source_program_fingerprint: baseline.deterministic_fingerprint,
      baseline,
      quality_status: 'NOT_RUN',
    })
  }

  private candidatesGenerate(request: CandidatesGenerateRequest): CandidatesGenerateResult {
    const design = this.requireDesign(request.design_id)
    const candidateCount = request.candidate_count ?? Math.min(3, request.objective_ledger.candidate_budget)
    const goalWeights = request.goal_weights ?? normalizeKnifeKnowledgeGoalWeights(DEFAULT_KNIFE_GOAL_WEIGHTS)
    const seed = request.seed ?? DEFAULT_CANDIDATE_SEED
    const successorRequestFingerprint = request.native_successor === undefined
      ? undefined
      : sha256Hex(canonicalJson(request.native_successor))
    const promotesReviewOnlySource = Boolean(
      design.candidates
      && design.candidates.size === 0
      && design.candidate_plan?.status === 'REVIEW_ONLY'
      && request.native_successor,
    )
    if (promotesReviewOnlySource) {
      design.candidates = undefined
      design.candidate_plan = undefined
      design.objective_ledger = undefined
    } else if (design.candidates && design.candidate_plan) {
      if (design.candidate_plan.requested_candidate_count !== candidateCount
        || design.candidate_plan.seed !== seed
        || canonicalJson(design.candidate_plan.goal_weights) !== canonicalJson(goalWeights)
        || design.objective_ledger?.canonical_sha256 !== request.objective_ledger.canonical_sha256
        || design.native_successor_request_fingerprint !== successorRequestFingerprint) {
        throw new ThreeAssetStudioError('STATE_CONFLICT', 'this design already has an immutable candidate batch with a different objective ledger, seed, or count')
      }
      return Object.freeze({
        schema_version: THREE_ASSET_STUDIO_SCHEMA,
        action: request.action,
        request_id: request.request_id,
        status: design.candidate_plan.status === 'REVIEW_ONLY' ? 'SOURCE_REVIEW_ONLY' : 'CANDIDATES_REUSED',
        design_id: design.design_id,
        candidates: Object.freeze([...design.candidates.values()].map((candidate) => candidate.summary)),
        generation_policy: 'objective-ledger-bounded-one-scope-candidates@2',
        objective_ledger_sha256: request.objective_ledger.canonical_sha256,
        baseline_objective_metrics: design.baseline_objective_metrics ?? null,
        candidate_plan_status: design.candidate_plan!.status as 'PROPOSALS_READY' | 'REVIEW_ONLY',
        candidate_plan_fingerprint: design.candidate_plan!.deterministic_fingerprint,
        goal_weights: design.candidate_plan!.goal_weights,
        seed: design.candidate_plan!.seed,
        rejection_reason: design.candidate_plan!.rejection_reason,
        native_successor_status: design.native_successor_plan
          ? 'PREPARED_REVIEW_ONLY'
          : design.program.source_envelope ? 'REQUIRED' : 'NOT_REQUIRED',
        native_successor_plan_fingerprint: design.native_successor_plan?.deterministic_fingerprint ?? null,
        quality_status: 'NOT_RUN',
      })
    }
    if (this.totalCandidateCount() + candidateCount > MAX_CANDIDATES_PER_DESIGN * MAX_DESIGNS) {
      throw new ThreeAssetStudioError('STATE_CONFLICT', `in-process candidate limit is ${MAX_CANDIDATES_PER_DESIGN * MAX_DESIGNS}`)
    }

    let candidateSourceProgram = design.program
    if (design.program.source_envelope && request.native_successor) {
      try {
        design.native_successor_plan = prepareKnifeNativeSuccessor({
          schema_version: KNIFE_NATIVE_SUCCESSOR_REQUEST_SCHEMA,
          source_program: design.program,
          ...request.native_successor,
        })
      } catch (error) {
        throw new ThreeAssetStudioError('COMPILATION_FAILED', error instanceof Error ? error.message : String(error))
      }
      design.native_successor_request_fingerprint = successorRequestFingerprint
      candidateSourceProgram = design.native_successor_plan.successor_program
    } else if (!design.program.source_envelope && request.native_successor) {
      throw new ThreeAssetStudioError('INVALID_REQUEST', 'native_successor is accepted only for an immutable compatibility source program')
    }

    let plan: KnifeKnowledgeCandidatePlan
    try {
      assertObjectiveLedgerProgramBinding(candidateSourceProgram, request.objective_ledger)
      plan = candidateSourceProgram.source_envelope
        ? generateKnifeKnowledgeCandidatePlan(candidateSourceProgram, {
            goal_weights: goalWeights,
            candidate_count: candidateCount,
            seed,
          })
        : generateKnifeObjectiveLedgerCandidates(candidateSourceProgram, request.objective_ledger, {
            goal_weights: goalWeights,
            candidate_count: candidateCount,
            seed,
          })
    } catch (error) {
      throw new ThreeAssetStudioError('COMPILATION_FAILED', error instanceof Error ? error.message : String(error))
    }
    design.candidate_plan = plan
    design.objective_ledger = request.objective_ledger
    if (plan.status === 'REVIEW_ONLY') {
      design.candidates = new Map()
      return Object.freeze({
        schema_version: THREE_ASSET_STUDIO_SCHEMA,
        action: request.action,
        request_id: request.request_id,
        status: 'SOURCE_REVIEW_ONLY',
        design_id: design.design_id,
        candidates: Object.freeze([]),
        generation_policy: 'objective-ledger-bounded-one-scope-candidates@2',
        objective_ledger_sha256: request.objective_ledger.canonical_sha256,
        baseline_objective_metrics: null,
        candidate_plan_status: 'REVIEW_ONLY',
        candidate_plan_fingerprint: plan.deterministic_fingerprint,
        goal_weights: plan.goal_weights,
        seed: plan.seed,
        rejection_reason: plan.rejection_reason,
        native_successor_status: 'REQUIRED',
        native_successor_plan_fingerprint: null,
        quality_status: 'NOT_RUN',
      })
    }

    const candidates = new Map<string, CandidateState>()
    const rig = createKnifeViewRigFromCalibration(design.baseline_calibration)
    let sourceCompiled: CompiledKnifeScene
    let sourceEvaluation: KnifeEightViewEvaluation
    let baselineObjectiveMetrics: KnifeObjectiveMetricAdapterAnyReceipt
    try {
      sourceCompiled = compileKnifeSceneProgram(candidateSourceProgram)
      sourceEvaluation = evaluateKnifeRig(sourceCompiled, rig)
      baselineObjectiveMetrics = measureStudioObjectiveMetrics({
        program: candidateSourceProgram,
        source_program_sha256: request.objective_ledger.program_sha256,
        ledger: request.objective_ledger,
        compiled: sourceCompiled,
        evaluation: sourceEvaluation,
        visibility: measureKnifePartVisibilityMetrics(sourceCompiled, rig),
        guard_fps: measureKnifeGuardFpsMetrics(sourceCompiled, rig),
      })
    } catch (error) {
      throw compilationError(error)
    }
    design.baseline_objective_metrics = baselineObjectiveMetrics
    for (const proposal of plan.candidates) {
      let compiled: CompiledKnifeScene
      try {
        compiled = compileKnifeSceneProgram(proposal.program)
      } catch (error) {
        throw compilationError(error)
      }
      let visibilityMetrics: KnifePartVisibilityMetrics
      let partBoundaryMetrics: KnifePartBoundaryMetrics
      let guardFpsMetrics: KnifeGuardFpsMetrics
      let structuralDelta: StudioStructuralDeltaSummary
      let objectiveMetricReceipt: KnifeObjectiveMetricAdapterAnyReceipt
      try {
        visibilityMetrics = measureKnifePartVisibilityMetrics(compiled, rig)
        partBoundaryMetrics = measureKnifePartBoundaryMetrics(compiled, rig)
        guardFpsMetrics = measureKnifeGuardFpsMetrics(compiled, rig)
        const candidateEvaluation = evaluateKnifeRig(compiled, rig)
        structuralDelta = measureStructuralDelta(sourceEvaluation, candidateEvaluation)
        if (structuralDelta.status !== 'MEASURED_NONZERO') {
          throw new Error(`candidate ${proposal.candidate_id} produced no fixed-view structural delta`)
        }
        objectiveMetricReceipt = measureStudioObjectiveMetrics({
          program: proposal.program,
          source_program_sha256: request.objective_ledger.program_sha256,
          ledger: request.objective_ledger,
          compiled,
          evaluation: candidateEvaluation,
          visibility: visibilityMetrics,
          guard_fps: guardFpsMetrics,
        })
      } catch (error) {
        disposeCompiledScene(compiled)
        throw new ThreeAssetStudioError('COMPILATION_FAILED', error instanceof Error ? error.message : String(error))
      }
      const candidateId = `candidate-${sha256Hex(canonicalJson({
        ledger_sha256: request.objective_ledger.canonical_sha256,
        program_sha256: sha256Hex(canonicalJson({ ...proposal.program, canonical_sha256: '' })),
        ordinal: proposal.ordinal,
      })).slice(0, 32)}`
      const summary: StudioCandidateSummary = {
        candidate_id: candidateId,
        design_id: design.design_id,
        longitudinal_segments: compiled.longitudinal_segments,
        triangle_count: compiled.triangle_count,
        part_count: compiled.parts.length,
        deterministic_fingerprint: compiled.deterministic_fingerprint,
        mutation_scope: proposal.mutation_scope,
        changed_parameter_paths: proposal.changed_parameter_paths,
        candidate_plan_fingerprint: plan.deterministic_fingerprint,
        objective_ledger_sha256: request.objective_ledger.canonical_sha256,
        visibility: summarizeVisibility(visibilityMetrics),
        part_boundary: summarizePartBoundary(partBoundaryMetrics),
        guard_fps: summarizeGuardFps(guardFpsMetrics),
        structural_delta: structuralDelta,
        objective_metrics: objectiveMetricReceipt,
        proposal_status: 'REVIEW_ONLY',
        state: 'CANDIDATE_GENERATED',
        quality_status: 'NOT_RUN',
      }
      candidates.set(candidateId, {
        summary: deepFreeze(summary),
        program: proposal.program,
        knowledge_candidate: proposal,
        visibility_metrics: visibilityMetrics,
        part_boundary_metrics: partBoundaryMetrics,
        guard_fps_metrics: guardFpsMetrics,
        objective_metric_receipt: objectiveMetricReceipt,
        compiled,
      })
    }
    disposeCompiledScene(sourceCompiled)
    design.candidates = candidates
    return Object.freeze({
      schema_version: THREE_ASSET_STUDIO_SCHEMA,
      action: request.action,
      request_id: request.request_id,
      status: 'CANDIDATES_GENERATED',
      design_id: design.design_id,
      candidates: Object.freeze([...candidates.values()].map((candidate) => candidate.summary)),
      generation_policy: 'objective-ledger-bounded-one-scope-candidates@2',
      objective_ledger_sha256: request.objective_ledger.canonical_sha256,
      baseline_objective_metrics: baselineObjectiveMetrics,
      candidate_plan_status: 'PROPOSALS_READY',
      candidate_plan_fingerprint: plan.deterministic_fingerprint,
      goal_weights: plan.goal_weights,
      seed: plan.seed,
      rejection_reason: null,
      native_successor_status: design.native_successor_plan ? 'PREPARED_REVIEW_ONLY' : 'NOT_REQUIRED',
      native_successor_plan_fingerprint: design.native_successor_plan?.deterministic_fingerprint ?? null,
      quality_status: 'NOT_RUN',
    })
  }

  private optimize(request: OptimizeRequest): OptimizeResult {
    const design = this.requireDesign(request.design_id)
    const candidates = design.candidates
    if (!candidates || candidates.size === 0) {
      throw new ThreeAssetStudioError('STATE_CONFLICT', 'candidates_generate must run before optimize')
    }
    const objectiveLedger = design.objective_ledger
    if (!objectiveLedger) {
      throw new ThreeAssetStudioError('STATE_CONFLICT', 'an immutable objective ledger must be bound before optimize')
    }
    const maxTriangles = design.program.budgets.max_triangles
    const eligibleCandidates = [...candidates.values()].filter(
      (candidate) => candidate.summary.triangle_count <= maxTriangles,
    )
    if (eligibleCandidates.length === 0) {
      throw new ThreeAssetStudioError('BUDGET_EXCEEDED', `no generated candidate fits max_triangles ${maxTriangles}`)
    }

    if (request.objective_function) {
      return this.optimizeWithObjectiveV2(request, design, objectiveLedger, eligibleCandidates)
    }

    const selected = request.candidate_id === undefined
      ? eligibleCandidates.sort(compareCandidateObservability)[0]
      : this.requireCandidate(request.candidate_id, design.design_id)
    if (selected.summary.triangle_count > maxTriangles) {
      throw new ThreeAssetStudioError(
        'BUDGET_EXCEEDED',
        `candidate ${selected.summary.candidate_id} emits ${selected.summary.triangle_count} triangles, above max_triangles ${maxTriangles}`,
      )
    }
    design.selected_candidate_id = selected.summary.candidate_id
    return Object.freeze({
      schema_version: THREE_ASSET_STUDIO_SCHEMA,
      action: request.action,
      request_id: request.request_id,
      status: 'REVIEW_ONLY_SELECTION',
      design_id: design.design_id,
      selected_candidate_id: selected.summary.candidate_id,
      objective_ledger_sha256: objectiveLedger.canonical_sha256,
      objective_metrics: Object.freeze([...objectiveLedger.objective_metrics]),
      objective_evaluation_status: 'PROXY_ONLY_NOT_LEDGER_ACCEPTANCE',
      objective: 'best-fixed-view-structural-observability-within-budget@1',
      candidate_ids: Object.freeze([...candidates.keys()]),
      selected_triangle_count: selected.summary.triangle_count,
      selected_visibility: selected.summary.visibility,
      decision_basis: 'NON_VISUAL_STRUCTURAL_RANKING',
      quality_status: 'NOT_RUN',
      visual_status: 'NOT_REVIEWED',
    })
  }

  private optimizeWithObjectiveV2(
    request: OptimizeRequest,
    design: DesignState,
    objectiveLedger: KnifeObjectiveLedger,
    eligibleCandidates: readonly CandidateState[],
  ): OptimizeObjectiveV2Result {
    const supplied = request.objective_function!
    const baseline = design.baseline_objective_metrics
    if (!baseline) {
      throw new ThreeAssetStudioError('STATE_CONFLICT', 'baseline objective metrics are unavailable')
    }
    let objective: KnifeObjectiveFunctionV2
    try {
      objective = createKnifeObjectiveFunctionV2FromLedger({
        ledger: objectiveLedger,
        objective_id: supplied.objective_id,
        metric_targets: supplied.metric_targets,
        baseline_values: supplied.baseline_values,
      })
    } catch (error) {
      throw new ThreeAssetStudioError('INVALID_REQUEST', error instanceof Error ? error.message : String(error))
    }
    if (objective.canonical_sha256 !== supplied.canonical_sha256) {
      throw new ThreeAssetStudioError('STATE_CONFLICT', 'objective_function does not exactly bind the active ledger')
    }
    if (canonicalJson(objective.baseline_values) !== canonicalJson(baseline.metrics)) {
      throw new ThreeAssetStudioError('STATE_CONFLICT', 'objective_function baseline_values do not match evaluator-owned baseline metrics')
    }
    const candidates = eligibleCandidates.map((candidate) => createKnifeObjectiveCandidateV2({
      candidate_id: candidate.summary.candidate_id,
      candidate_sha256: candidate.objective_metric_receipt.candidate_program_sha256,
      values: candidate.objective_metric_receipt.metrics,
    }))
    const selection = evaluateKnifeObjectiveFunctionV2(objective, candidates)
    if (selection.selected_candidate_id !== null) {
      design.selected_candidate_id = selection.selected_candidate_id
    }
    return Object.freeze({
      schema_version: THREE_ASSET_STUDIO_SCHEMA,
      action: request.action,
      request_id: request.request_id,
      status: selection.selection_status,
      design_id: design.design_id,
      selected_candidate_id: selection.selected_candidate_id,
      objective_ledger_sha256: objectiveLedger.canonical_sha256,
      objective_function_sha256: objective.canonical_sha256,
      objective_evaluation_status: KNIFE_OBJECTIVE_FUNCTION_V2_SCHEMA,
      candidate_ids: Object.freeze(eligibleCandidates.map((candidate) => candidate.summary.candidate_id)),
      selection_receipt: selection,
      decision_basis: 'NON_VISUAL_STRUCTURAL_RANKING',
      quality_status: 'NOT_RUN',
      visual_status: selection.visual_status,
    })
  }

  private threeAssetBuild(request: ThreeAssetBuildRequest): ThreeAssetBuildResult {
    const candidate = this.requireCandidate(request.candidate_id)
    const existing = this.assets.get(candidate.summary.candidate_id)
    if (existing) {
      return Object.freeze({
        schema_version: THREE_ASSET_STUDIO_SCHEMA,
        action: request.action,
        request_id: request.request_id,
        status: 'ASSET_REUSED',
        candidate_id: candidate.summary.candidate_id,
        asset_id: assetIdFor(candidate),
        scene_fingerprint: existing.scene_fingerprint,
        compiled: summarizeCompiledScene(existing.compiled),
        quality_status: 'NOT_RUN',
      })
    }
    if (this.assets.size >= MAX_ASSETS) throw new ThreeAssetStudioError('STATE_CONFLICT', `in-process asset limit is ${MAX_ASSETS}`)
    const design = this.requireDesign(candidate.summary.design_id)
    const scene = createStudioScene(candidate)
    const rig = createKnifeViewRigFromCalibration(design.baseline_calibration)
    scene.updateMatrixWorld(true)
    const sceneFingerprint = fingerprintKnifePreviewScene(scene, candidate.compiled.deterministic_fingerprint)
    const asset: AssetState = {
      candidate,
      compiled: candidate.compiled,
      scene,
      rig,
      scene_fingerprint: sceneFingerprint,
    }
    this.assets.set(candidate.summary.candidate_id, asset)
    return Object.freeze({
      schema_version: THREE_ASSET_STUDIO_SCHEMA,
      action: request.action,
      request_id: request.request_id,
      status: 'ASSET_BUILT',
      candidate_id: candidate.summary.candidate_id,
      asset_id: assetIdFor(candidate),
      scene_fingerprint: sceneFingerprint,
      compiled: summarizeCompiledScene(candidate.compiled),
      quality_status: 'NOT_RUN',
    })
  }

  private async preview(
    request: PreviewRequest,
    context: ThreeAssetStudioExecutionContext,
  ): Promise<PreviewResult> {
    const asset = this.requireAsset(request.candidate_id)
    const captureRequested = request.capture_aovs === true
    const selectedViewIds = captureRequested
      ? [...KNIFE_VIEW_IDS]
      : [...request.view_ids ?? ['FRONT']]
    const renderer = context.renderer
    const captureRenderer = context.capture_renderer ?? (captureRequested ? renderer : undefined)
    let renderedViewIds: readonly KnifeViewId[] = []
    if (renderer) {
      renderStudioViews(renderer, asset.scene, asset.rig, selectedViewIds)
      renderedViewIds = selectedViewIds
    } else if (!captureRequested && context.capture_renderer) {
      throw new ThreeAssetStudioError('PREVIEW_RENDERER_REQUIRED', 'preview requires renderer in execution context')
    }

    let capture: KnifeBrowserCaptureResult | undefined
    if (captureRequested) {
      if (!captureRenderer) {
        throw new ThreeAssetStudioError('PREVIEW_CAPTURE_RENDERER_REQUIRED', 'preview capture requires a preserveDrawingBuffer renderer')
      }
      try {
        capture = captureKnifeAovs({
          renderer: captureRenderer,
          scene: asset.scene,
          compiled: asset.compiled,
          rig: asset.rig,
          manifest_id: `studio-${asset.candidate.summary.candidate_id}`,
        })
      } catch (error) {
        throw new ThreeAssetStudioError('PREVIEW_CAPTURE_RENDERER_REQUIRED', error instanceof Error ? error.message : String(error))
      }
      renderedViewIds = [...KNIFE_VIEW_IDS]
    }

    asset.scene.userData.renderer_invoked = renderedViewIds.length > 0
    asset.scene.updateMatrixWorld(true)
    const sceneFingerprint = fingerprintKnifePreviewScene(asset.scene, asset.compiled.deterministic_fingerprint)
    const manifest = parseKnifePreviewManifest({
      schema_version: KNIFE_PREVIEW_MANIFEST_SCHEMA,
      view_ids: selectedViewIds,
      capture: captureRequested ? 'capture-ready' : (renderedViewIds.length > 0 ? 'settled' : 'capture-ready'),
      ...(captureRequested ? { aovs: 'required' } : {}),
    })
    const views = Object.freeze(selectedViewIds.map((viewId) => {
      const camera = createKnifeViewCamera(asset.rig, viewId)
      const cameraReceipt = makeKnifeBrowserCameraReceipt(camera, getKnifeView(asset.rig, viewId), {
        x: 0,
        y: 0,
        width: asset.rig.frame_width,
        height: asset.rig.frame_height,
      })
      return Object.freeze({
        view_id: viewId,
        camera: cameraReceipt,
        viewport: cameraReceipt.viewport,
        renderer_invoked: renderedViewIds.includes(viewId),
        render_status: renderedViewIds.includes(viewId) ? 'RENDERED' as const : 'NOT_RUN' as const,
        quality_status: 'NOT_RUN' as const,
      })
    }))
    const deterministicFingerprint = renderedViewIds.length === selectedViewIds.length
      ? hashKnifeBrowserPreviewReceipt(
          asset.compiled.deterministic_fingerprint,
          sceneFingerprint,
          asset.rig.deterministic_fingerprint,
          views as unknown as readonly KnifeBrowserViewReceipt[],
        )
      : sha256Hex(canonicalJson({
          candidate_id: asset.candidate.summary.candidate_id,
          scene_fingerprint: sceneFingerprint,
          rig_fingerprint: asset.rig.deterministic_fingerprint,
          view_ids: selectedViewIds,
          renderer_invoked: false,
        }))
    const receipt: StudioPreviewReceipt = Object.freeze({
      schema_version: 'WeaponryThreeAssetStudioPreviewReceipt@1',
      route: 'weaponry-three-asset-studio@1',
      asset_id: assetIdFor(asset.candidate),
      candidate_id: asset.candidate.summary.candidate_id,
      scene_fingerprint: sceneFingerprint,
      rig_schema_version: asset.rig.schema_version,
      rig_id: asset.rig.rig_id,
      rig_fingerprint: asset.rig.deterministic_fingerprint,
      manifest,
      selected_view_ids: Object.freeze([...selectedViewIds]),
      views,
      renderer_invoked: renderedViewIds.length > 0,
      render_status: renderedViewIds.length > 0 ? 'RENDERED' : 'NOT_RUN',
      capture_status: capture ? 'CAPTURED' : 'NOT_REQUESTED',
      ...(capture ? { capture_manifest_sha256: capture.manifest.canonical_sha256 } : {}),
      visual_status: 'NOT_REVIEWED',
      quality_status: 'NOT_RUN',
      deterministic_fingerprint: deterministicFingerprint,
    })
    return Object.freeze({
      schema_version: THREE_ASSET_STUDIO_SCHEMA,
      action: request.action,
      request_id: request.request_id,
      status: renderedViewIds.length > 0 ? 'PREVIEW_RENDERED' : 'PREVIEW_READY',
      candidate_id: asset.candidate.summary.candidate_id,
      receipt,
      ...(capture ? { capture } : {}),
      quality_status: 'NOT_RUN',
    })
  }

  private async export(request: ExportRequest): Promise<ExportResult> {
    const asset = this.requireAsset(request.candidate_id)
    asset.scene.updateMatrixWorld(true)
    let bytes: Uint8Array
    try {
      // Export is optional and heavy. Load the writer only when this bounded
      // action is invoked so the interactive design/preview route stays small.
      const { GLTFExporter } = await import('three/examples/jsm/exporters/GLTFExporter.js')
      const exporter = new GLTFExporter()
      const result = await exporter.parseAsync(asset.compiled.group, {
        binary: true,
        onlyVisible: true,
        trs: false,
      })
      if (!(result instanceof ArrayBuffer)) throw new Error('GLTFExporter did not return binary GLB bytes')
      bytes = new Uint8Array(result)
    } catch (error) {
      throw new ThreeAssetStudioError('GLB_EXPORT_FAILED', error instanceof Error ? error.message : String(error))
    }
    if (bytes.byteLength <= 0 || bytes.byteLength > MAX_GLB_BYTES) {
      throw new ThreeAssetStudioError('BUDGET_EXCEEDED', `GLB bytes must be in (0, ${MAX_GLB_BYTES}]`)
    }
    return Object.freeze({
      schema_version: THREE_ASSET_STUDIO_SCHEMA,
      action: request.action,
      request_id: request.request_id,
      status: 'EXPORTED_GLB',
      candidate_id: asset.candidate.summary.candidate_id,
      asset_id: assetIdFor(asset.candidate),
      encoding: 'base64',
      glb_base64: bytesToBase64(bytes),
      glb_sha256: sha256Hex(bytes),
      glb_bytes: bytes.byteLength,
      triangles: asset.compiled.triangle_count,
      part_ids: Object.freeze(asset.compiled.parts.map((part) => part.part_id)),
      renderer_invoked: false,
      quality_status: 'NOT_RUN',
    })
  }

  private requireDesign(designId: string): DesignState {
    const design = this.designs.get(designId)
    if (!design) throw new ThreeAssetStudioError('DESIGN_NOT_FOUND', `unknown design ${designId}`)
    return design
  }

  private requireCandidate(candidateId: string, designId?: string): CandidateState {
    if (designId !== undefined) {
      const design = this.requireDesign(designId)
      const candidate = design.candidates?.get(candidateId)
      if (!candidate) throw new ThreeAssetStudioError('CANDIDATE_NOT_FOUND', `unknown candidate ${candidateId} for ${designId}`)
      return candidate
    }
    for (const design of this.designs.values()) {
      const candidate = design.candidates?.get(candidateId)
      if (candidate) return candidate
    }
    throw new ThreeAssetStudioError('CANDIDATE_NOT_FOUND', `unknown candidate ${candidateId}`)
  }

  private requireAsset(candidateId: string): AssetState {
    const asset = this.assets.get(candidateId)
    if (!asset) throw new ThreeAssetStudioError('ASSET_NOT_BUILT', `three_asset_build must run before ${candidateId}`)
    return asset
  }

  private totalCandidateCount(): number {
    let count = 0
    for (const design of this.designs.values()) count += design.candidates?.size ?? 0
    return count
  }
}

function parseProgram(value: unknown): KnifeSceneProgram {
  const program = requestRecord(value, 'program')
  exactKeys(program, [
    'schema_version',
    'asset_id',
    'family',
    'design_basis',
    'coordinate_convention',
    'blade_surface',
    'assembly',
    'parts',
    'material_zones',
    'presentation',
    'budgets',
    'unknowns',
    'canonical_sha256',
  ], 'program', ['assembly', 'source_envelope'])
  assertNoExternalInputs(program)
  return cloneAndFreezeProgram(program as unknown as KnifeSceneProgram)
}

function cloneAndFreezeProgram(program: KnifeSceneProgram): KnifeSceneProgram {
  const clone = cloneJson(program)
  deepFreeze(clone)
  return clone as KnifeSceneProgram
}

function parseObjectiveLedger(value: unknown): KnifeObjectiveLedger {
  try {
    validateKnifeObjectiveLedger(value, { require_canonical_sha256: true })
  } catch (error) {
    throw new ThreeAssetStudioError('INVALID_REQUEST', error instanceof Error ? error.message : String(error))
  }
  const clone = cloneJson(value) as KnifeObjectiveLedger
  deepFreeze(clone)
  return clone
}

function parseObjectiveFunction(value: unknown): KnifeObjectiveFunctionV2 {
  try {
    validateKnifeObjectiveFunctionV2(value, { require_canonical_sha256: true })
  } catch (error) {
    throw new ThreeAssetStudioError('INVALID_REQUEST', error instanceof Error ? error.message : String(error))
  }
  const clone = cloneJson(value) as KnifeObjectiveFunctionV2
  deepFreeze(clone)
  return clone
}

function assertObjectiveLedgerProgramBinding(program: KnifeSceneProgram, ledger: KnifeObjectiveLedger): void {
  const programSha = program.canonical_sha256 || sha256Hex(canonicalJson({ ...program, canonical_sha256: '' }))
  if (ledger.program_sha256 !== programSha) {
    throw new ThreeAssetStudioError(
      'STATE_CONFLICT',
      `objective ledger ${ledger.ledger_id}@${ledger.revision} does not bind the candidate source program`,
    )
  }
}

function designIdFor(program: KnifeSceneProgram): string {
  return `design-${sha256Hex(canonicalJson(program)).slice(0, 16)}`
}

function assetIdFor(candidate: CandidateState): string {
  return `three-asset-${candidate.summary.candidate_id.slice('candidate-'.length)}`
}

function summarizeCompiledScene(compiled: CompiledKnifeScene): ThreeAssetStudioCompiledSummary {
  return Object.freeze({
    deterministic_fingerprint: compiled.deterministic_fingerprint,
    triangle_count: compiled.triangle_count,
    part_count: compiled.parts.length,
    assembly_part_count: compiled.assembly_parts.length,
    assembly_status: compiled.assembly_status,
    longitudinal_segments: compiled.longitudinal_segments,
    parts: Object.freeze(compiled.parts.map((part) => Object.freeze({
      part_id: part.part_id,
      material_zone_id: part.material_zone_id,
      surface_role: part.surface_role,
      ...(part.assembly_primitive === undefined ? {} : { assembly_primitive: part.assembly_primitive }),
      triangles: triangleCount(part),
    }))),
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  })
}

function summarizeVisibility(metrics: KnifePartVisibilityMetrics): StudioVisibilitySummary {
  return deepFreeze({
    schema_version: metrics.schema_version,
    deterministic_fingerprint: metrics.deterministic_fingerprint,
    visible_part_count: metrics.parts.length - metrics.missing_part_ids.length,
    total_part_count: metrics.parts.length,
    missing_part_ids: [...metrics.missing_part_ids],
    underexposed_part_ids: [...metrics.underexposed_part_ids],
    total_visible_view_count: metrics.parts.reduce((sum, part) => sum + part.visible_view_count, 0),
    status: metrics.status,
    quality_status: 'NOT_RUN',
  })
}

function measureStructuralDelta(
  baseline: KnifeEightViewEvaluation,
  candidate: KnifeEightViewEvaluation,
): StudioStructuralDeltaSummary {
  if (baseline.rig.deterministic_fingerprint !== candidate.rig.deterministic_fingerprint
    || baseline.views.length !== candidate.views.length) {
    throw new ThreeAssetStudioError('COMPILATION_FAILED', 'candidate structural delta must use the frozen baseline rig')
  }
  let changedViewCount = 0
  let silhouetteChangedPixelCount = 0
  let partIdChangedPixelCount = 0
  for (let viewIndex = 0; viewIndex < baseline.views.length; viewIndex += 1) {
    const baselineView = baseline.views[viewIndex]
    const candidateView = candidate.views[viewIndex]
    if (baselineView.view_id !== candidateView.view_id
      || baselineView.mask.pixels.length !== candidateView.mask.pixels.length
      || baselineView.mask.part_indices.length !== candidateView.mask.part_indices.length) {
      throw new ThreeAssetStudioError('COMPILATION_FAILED', 'candidate structural delta view binding drifted')
    }
    let viewChanged = false
    for (let pixelIndex = 0; pixelIndex < baselineView.mask.pixels.length; pixelIndex += 1) {
      if (baselineView.mask.pixels[pixelIndex] !== candidateView.mask.pixels[pixelIndex]) {
        silhouetteChangedPixelCount += 1
        viewChanged = true
      }
      if (baselineView.mask.part_indices[pixelIndex] !== candidateView.mask.part_indices[pixelIndex]) {
        partIdChangedPixelCount += 1
        viewChanged = true
      }
    }
    if (viewChanged) changedViewCount += 1
  }
  const status = silhouetteChangedPixelCount + partIdChangedPixelCount >= 1
    ? 'MEASURED_NONZERO'
    : 'REJECTED_NO_VISIBLE_DELTA'
  return deepFreeze({
    schema_version: 'KnifeCandidateStructuralDelta@1' as const,
    baseline_source_fingerprint: baseline.receipt.source_fingerprint,
    candidate_source_fingerprint: candidate.receipt.source_fingerprint,
    changed_view_count: changedViewCount,
    silhouette_changed_pixel_count: silhouetteChangedPixelCount,
    part_id_changed_pixel_count: partIdChangedPixelCount,
    minimum_changed_pixel_count: 1 as const,
    status,
    quality_status: 'NOT_RUN' as const,
  })
}

function summarizePartBoundary(metrics: KnifePartBoundaryMetrics): StudioPartBoundarySummary {
  const partCount = metrics.parts.length
  return deepFreeze({
    schema_version: metrics.schema_version,
    deterministic_fingerprint: metrics.deterministic_fingerprint,
    part_count: partCount,
    semantic_adjacency_count: metrics.semantic_adjacencies.length,
    total_boundary_pixel_count: metrics.parts.reduce((sum, part) => sum + part.boundary_pixel_count, 0),
    mean_boundary_length_normalized: partCount === 0
      ? 0
      : metrics.parts.reduce((sum, part) => sum + part.boundary_length_normalized, 0) / partCount,
    total_connected_island_count: metrics.parts.reduce((sum, part) => sum + part.connected_island_count, 0),
    status: metrics.status,
    quality_status: 'NOT_RUN',
  })
}

function summarizeGuardFps(metrics: KnifeGuardFpsMetrics): StudioGuardFpsSummary {
  const negativeSpaceComputability = metrics.guard_negative_space.views.reduce(
    (counts, view) => {
      if (view.computability === 'COMPUTED') counts.computed += 1
      else if (view.computability === 'PARTIAL') counts.partial += 1
      else counts.notComputable += 1
      return counts
    },
    { computed: 0, partial: 0, notComputable: 0 },
  )
  return deepFreeze({
    schema_version: metrics.schema_version,
    deterministic_fingerprint: metrics.deterministic_fingerprint,
    guard_part_id: metrics.guard_part_id,
    negative_space_computed_view_count: negativeSpaceComputability.computed,
    negative_space_partial_view_count: negativeSpaceComputability.partial,
    negative_space_not_computable_view_count: negativeSpaceComputability.notComputable,
    fps_hold_computability: metrics.fps_hold.computability,
    fps_hold_asset_bbox_width_fraction: metrics.fps_hold.asset_bbox_width_fraction,
    fps_hold_asset_bbox_height_fraction: metrics.fps_hold.asset_bbox_height_fraction,
    fps_hold_tip_safe_margin_fraction: metrics.fps_hold.tip_safe_margin_fraction,
    fps_hold_guard_occlusion_ratio: metrics.fps_hold.guard_occlusion_ratio,
    status: metrics.status,
    quality_status: 'NOT_RUN',
  })
}

/**
 * Rank only measurable structural observability. This deliberately does not
 * infer attractiveness, likeness, material quality, or commercial readiness.
 */
function compareCandidateObservability(left: CandidateState, right: CandidateState): number {
  return left.summary.visibility.missing_part_ids.length - right.summary.visibility.missing_part_ids.length
    || left.summary.visibility.underexposed_part_ids.length - right.summary.visibility.underexposed_part_ids.length
    || right.summary.visibility.total_visible_view_count - left.summary.visibility.total_visible_view_count
    || left.summary.triangle_count - right.summary.triangle_count
    || left.summary.candidate_id.localeCompare(right.summary.candidate_id)
}

function createStudioScene(candidate: CandidateState): THREE.Scene {
  const scene = new THREE.Scene()
  const fingerprint = candidate.summary.deterministic_fingerprint
  scene.name = `three-asset-studio:${candidate.summary.candidate_id}`
  overrideKnifePreviewUuid(scene, stableKnifePreviewUuid(`studio-scene:${fingerprint}`))
  scene.background = new THREE.Color('#080b10')
  scene.userData = {
    schema_version: 'WeaponryThreeAssetStudioScene@1',
    candidate_id: candidate.summary.candidate_id,
    source_fingerprint: fingerprint,
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  }
  scene.add(candidate.compiled.group)

  const hemisphere = new THREE.HemisphereLight(0xe8edf5, 0x101820, 1.45)
  hemisphere.name = 'studio-light:hemisphere'
  overrideKnifePreviewUuid(hemisphere, stableKnifePreviewUuid(`studio-light:${fingerprint}:hemisphere`))
  scene.add(hemisphere)
  const key = new THREE.DirectionalLight(0xfff0d8, 4.2)
  key.name = 'studio-light:key'
  key.position.set(-2.4, 3.0, 4.0)
  overrideKnifePreviewUuid(key, stableKnifePreviewUuid(`studio-light:${fingerprint}:key`))
  scene.add(key)
  const rim = new THREE.DirectionalLight(0x8abfff, 2.1)
  rim.name = 'studio-light:rim'
  rim.position.set(2.8, -1.2, 2.4)
  overrideKnifePreviewUuid(rim, stableKnifePreviewUuid(`studio-light:${fingerprint}:rim`))
  scene.add(rim)
  return scene
}

function renderStudioViews(
  renderer: THREE.WebGLRenderer,
  scene: THREE.Scene,
  rig: KnifeViewRig,
  viewIds: readonly KnifeViewId[],
): void {
  if (!renderer || !renderer.domElement || typeof renderer.render !== 'function') {
    throw new ThreeAssetStudioError('PREVIEW_RENDERER_REQUIRED', 'execution context renderer is not a live Three.js renderer')
  }
  renderer.setPixelRatio(1)
  renderer.setSize(rig.frame_width, rig.frame_height, false)
  renderer.setScissorTest(false)
  renderer.setViewport(0, 0, rig.frame_width, rig.frame_height)
  renderer.setClearColor('#080b10', 1)
  for (const viewId of viewIds) renderer.render(scene, createKnifeViewCamera(rig, viewId))
}

function triangleCount(part: CompiledKnifePart): number {
  const index = part.geometry.getIndex()
  const position = part.geometry.getAttribute('position')
  const count = index ? index.count : position?.count ?? 0
  return Math.floor(count / 3)
}

function disposeCompiledScene(compiled: CompiledKnifeScene): void {
  const geometries = new Set<THREE.BufferGeometry>()
  const materials = new Set<THREE.Material>()
  for (const part of compiled.parts) {
    geometries.add(part.geometry)
    materials.add(part.material)
  }
  for (const geometry of geometries) geometry.dispose()
  for (const material of materials) material.dispose()
  compiled.group.clear()
}

function compilationError(error: unknown): ThreeAssetStudioError {
  const message = error instanceof Error ? error.message : String(error)
  if (/BUDGET_EXCEEDED/.test(message)) return new ThreeAssetStudioError('BUDGET_EXCEEDED', message)
  return new ThreeAssetStudioError('COMPILATION_FAILED', message)
}

function requestRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new ThreeAssetStudioError('INVALID_REQUEST', `${label} must be a plain object`)
  }
  const prototype = Object.getPrototypeOf(value)
  if (prototype !== Object.prototype && prototype !== null) {
    throw new ThreeAssetStudioError('INVALID_REQUEST', `${label} must be a plain object`)
  }
  return value as Record<string, unknown>
}

function exactKeys(
  record: Record<string, unknown>,
  required: readonly string[],
  label: string,
  optional: readonly string[] = [],
): void {
  const allowed = new Set([...required, ...optional])
  for (const key of Object.keys(record)) {
    if (!allowed.has(key)) throw new ThreeAssetStudioError('INVALID_REQUEST', `${label} has unsupported field ${key}`)
  }
  for (const key of required) {
    if (!(key in record) || record[key] === undefined) throw new ThreeAssetStudioError('INVALID_REQUEST', `${label} requires ${key}`)
  }
}

function stableId(value: unknown, label: string): string {
  if (typeof value !== 'string' || !STABLE_ID_PATTERN.test(value)) {
    throw new ThreeAssetStudioError('INVALID_REQUEST', `${label} must be a bounded stable ID`)
  }
  return value
}

function boundedCandidateCount(value: unknown): number {
  if (!Number.isInteger(value) || (value as number) < 2 || (value as number) > MAX_CANDIDATES_PER_DESIGN) {
    throw new ThreeAssetStudioError('INVALID_REQUEST', `candidate_count must be an integer in [2, ${MAX_CANDIDATES_PER_DESIGN}]`)
  }
  return value as number
}

function boundedSeed(value: unknown): number {
  if (!Number.isInteger(value) || (value as number) < 0 || (value as number) > 0xffffffff) {
    throw new ThreeAssetStudioError('INVALID_REQUEST', 'seed must be an integer in [0, 2^32-1]')
  }
  return value as number
}

function parseGoalWeights(value: unknown): KnifeKnowledgeGoalWeights {
  try {
    return normalizeKnifeKnowledgeGoalWeights(value)
  } catch (error) {
    throw new ThreeAssetStudioError('INVALID_REQUEST', error instanceof Error ? error.message : String(error))
  }
}

function parseNativeSuccessorOptions(value: unknown): StudioNativeSuccessorOptions {
  const record = requestRecord(value, 'native_successor')
  exactKeys(record, ['successor_asset_id', 'successor_design_basis', 'mutable_part_ids'], 'native_successor')
  const designBasis = record.successor_design_basis
  if (designBasis !== 'authorized-reference-inspired' && designBasis !== 'original-design') {
    throw new ThreeAssetStudioError('INVALID_REQUEST', 'native_successor.successor_design_basis is unsupported')
  }
  if (!Array.isArray(record.mutable_part_ids) || record.mutable_part_ids.length < 1 || record.mutable_part_ids.length > 32) {
    throw new ThreeAssetStudioError('INVALID_REQUEST', 'native_successor.mutable_part_ids must contain 1 to 32 stable Part IDs')
  }
  const mutablePartIds = record.mutable_part_ids.map((partId) => stableId(partId, 'native_successor.mutable_part_ids[]'))
  if (new Set(mutablePartIds).size !== mutablePartIds.length) {
    throw new ThreeAssetStudioError('INVALID_REQUEST', 'native_successor.mutable_part_ids must be unique')
  }
  return deepFreeze({
    successor_asset_id: stableId(record.successor_asset_id, 'native_successor.successor_asset_id'),
    successor_design_basis: designBasis,
    mutable_part_ids: mutablePartIds,
  })
}

function boundedBoolean(value: unknown, label: string): boolean {
  if (typeof value !== 'boolean') throw new ThreeAssetStudioError('INVALID_REQUEST', `${label} must be boolean`)
  return value
}

function normalizeViewIds(value: unknown): readonly KnifeViewId[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > KNIFE_VIEW_IDS.length) {
    throw new ThreeAssetStudioError('INVALID_REQUEST', 'view_ids must contain one to eight fixed views')
  }
  const selected = new Set<KnifeViewId>()
  for (const raw of value) {
    if (typeof raw !== 'string') throw new ThreeAssetStudioError('INVALID_REQUEST', 'view_ids must contain text IDs')
    const normalized = raw.trim().toUpperCase().replaceAll('-', '_') as KnifeViewId
    if (!KNIFE_VIEW_IDS.includes(normalized)) throw new ThreeAssetStudioError('INVALID_REQUEST', `unsupported fixed view ${raw}`)
    if (selected.has(normalized)) throw new ThreeAssetStudioError('INVALID_REQUEST', `duplicate fixed view ${normalized}`)
    selected.add(normalized)
  }
  return Object.freeze(KNIFE_VIEW_IDS.filter((viewId) => selected.has(viewId)))
}

function isActionName(value: unknown): value is ThreeAssetStudioActionName {
  return typeof value === 'string' && (THREE_ASSET_STUDIO_ACTIONS as readonly string[]).includes(value)
}

function assertNoExternalInputs(value: unknown, keyPath = 'request'): void {
  if (typeof value === 'string') {
    if (value.includes('/') || value.includes('\\') || UNSAFE_VALUE_PATTERN.test(value)) {
      throw new ThreeAssetStudioError('INVALID_EXTERNAL_INPUT', `${keyPath} cannot contain URL, path, script, or network input`)
    }
    return
  }
  if (value === null || typeof value === 'boolean' || typeof value === 'number') return
  if (typeof value !== 'object') throw new ThreeAssetStudioError('INVALID_EXTERNAL_INPUT', `${keyPath} must be JSON-like`)
  if (Array.isArray(value)) {
    value.forEach((child, index) => assertNoExternalInputs(child, `${keyPath}[${index}]`))
    return
  }
  const record = requestRecord(value, keyPath)
  for (const [key, child] of Object.entries(record)) {
    if (UNSAFE_KEY_PATTERN.test(key)) throw new ThreeAssetStudioError('INVALID_EXTERNAL_INPUT', `${keyPath}.${key} is not an allowed input`)
    assertNoExternalInputs(child, `${keyPath}.${key}`)
  }
}

function cloneJson(value: unknown): unknown {
  if (value === null || typeof value === 'string' || typeof value === 'boolean' || typeof value === 'number') return value
  if (Array.isArray(value)) return value.map(cloneJson)
  if (typeof value !== 'object') throw new ThreeAssetStudioError('INVALID_REQUEST', 'request contains a non-JSON value')
  const source = requestRecord(value, 'request value')
  return Object.fromEntries(Object.entries(source).map(([key, child]) => [key, cloneJson(child)]))
}

function deepFreeze<T>(value: T): T {
  if (!value || typeof value !== 'object' || Object.isFrozen(value)) return value
  for (const child of Object.values(value as Record<string, unknown>)) deepFreeze(child)
  return Object.freeze(value)
}

function canonicalJson(value: unknown): string {
  if (value === null) return 'null'
  if (typeof value === 'string') return JSON.stringify(value)
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new ThreeAssetStudioError('INVALID_REQUEST', 'canonical input cannot contain non-finite numbers')
    return Object.is(value, -0) ? '0' : JSON.stringify(value)
  }
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  if (typeof value === 'object') {
    const record = requestRecord(value, 'canonical value')
    return `{${Object.keys(record).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`).join(',')}}`
  }
  throw new ThreeAssetStudioError('INVALID_REQUEST', 'canonical input cannot contain undefined or functions')
}

function bytesToBase64(bytes: Uint8Array): string {
  if (typeof btoa === 'function') {
    let binary = ''
    const chunkSize = 0x8000
    for (let offset = 0; offset < bytes.length; offset += chunkSize) {
      binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize))
    }
    return btoa(binary)
  }
  const bufferConstructor = (globalThis as unknown as {
    Buffer?: { from(value: Uint8Array): { toString(encoding: 'base64'): string } }
  }).Buffer
  if (bufferConstructor) return bufferConstructor.from(bytes).toString('base64')
  throw new ThreeAssetStudioError('GLB_EXPORT_FAILED', 'base64 encoding is unavailable in this host')
}
