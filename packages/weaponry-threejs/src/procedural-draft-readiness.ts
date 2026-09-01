import {
  compileKnifeSceneProgram,
  type CompiledKnifeScene,
} from './knife-scene-compiler.ts'
import {
  KNIFE_VIEW_IDS,
  createKnifeFixedEightViewRig,
  evaluateKnifeRig,
  type KnifeEightViewEvaluation,
  type KnifeViewRig,
} from './knife-view-evaluation.ts'
import {
  measureKnifePartVisibilityMetrics,
} from './knife-part-visibility-metrics.ts'
import { sha256Hex } from './knife-browser-capture.ts'
import type { KnifeSceneProgram } from './knife-scene-program.ts'

/**
 * Readiness for the deliberately lower Three.js "Procedural Draft" tier.
 *
 * This is an evidence projection, not a Runtime, Store, CAS or approval
 * surface. The evaluator owns all derived evidence: the candidate is decoded
 * from its bytes, fixed views and structural delta are replayed here, and the
 * GLB is read back from the supplied payload before it can pass.
 */
export const PROCEDURAL_DRAFT_READINESS_SCHEMA = 'WeaponryThreeJsProceduralDraftReadiness@1' as const
export const PROCEDURAL_DRAFT_READINESS_ROUTE = 'weaponry-threejs-procedural-draft@1' as const

export type ProceduralDraftReadinessStatus = 'THREEJS_DESIGN_READY' | 'BLOCKED'
export type ProceduralDraftGateStatus = 'PASS' | 'BLOCKED'

export interface ProceduralDraftStructuralDelta {
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

/** Legacy artifact shape retained for read-only consumers; it is not evaluator input. */
export interface ProceduralDraftGlbReceipt {
  readonly schema_version: 'WeaponryThreeJsCompileReceipt@1' | 'WeaponryThreeJsArtifactEvidence@1'
  readonly asset_id: string
  readonly route?: string
  readonly program_sha256?: string
  readonly program_bytes_sha256?: string
  readonly program_path?: string
  readonly compiler?: string
  readonly deterministic_fingerprint?: string
  readonly glb_path?: string
  readonly glb_sha256: string
  readonly glb_bytes: number
  readonly triangles?: number
  readonly triangle_count?: number
  readonly part_count?: number
  readonly part_ids?: readonly string[]
  readonly material_zone_count?: number
  readonly fixed_view_count?: number
  readonly aov_count_per_view?: number
  readonly captured_payload_count?: number
  readonly integrated_kernels?: readonly string[]
  readonly browser_receipt_path?: string
  readonly browser_receipt_sha256?: string
  readonly front_preview_path?: string
  readonly front_preview_sha256?: string
  readonly render_status?: 'NOT_RUN' | 'RENDERED'
  readonly quality_status?: 'NOT_RUN' | 'RENDERED_NOT_APPROVED' | 'MEASURED_NOT_APPROVED'
  readonly visual_status?: 'NOT_REQUESTED' | 'NOT_RUN' | 'RENDERED_NOT_APPROVED' | 'MEASURED_NOT_APPROVED'
  readonly human_status?: 'NOT_REQUESTED' | 'NOT_RUN'
  readonly commercial_status?: 'NOT_REQUESTED' | 'NOT_RUN'
  readonly conclusion?: string
}

export interface ProceduralDraftReadinessInput {
  /** Exact JSON bytes for the candidate KnifeSceneProgram@1. */
  readonly program_bytes: Uint8Array
  /** Optional baseline program. If baseline bytes are present, those bytes are authoritative. */
  readonly baseline_program?: unknown
  /** Exact JSON bytes for the baseline, when available. */
  readonly baseline_program_bytes?: Uint8Array
  /** Exact binary GLB payload to hash, parse and read back. */
  readonly glb_payload: Uint8Array
}

export interface ProceduralDraftGate {
  readonly status: ProceduralDraftGateStatus
  readonly reason: string | null
}

export interface ProceduralDraftClosedProgramGate extends ProceduralDraftGate {
  readonly schema_version: string | null
  readonly asset_id: string | null
  readonly canonical_sha256: string | null
}

export interface ProceduralDraftCompilationGate extends ProceduralDraftGate {
  readonly deterministic_fingerprint: string | null
  readonly triangle_count: number | null
  readonly draw_call_count: number | null
}

export interface ProceduralDraftPartsMaterialsGate extends ProceduralDraftGate {
  readonly part_count: number
  readonly compiled_part_count: number
  readonly material_zone_count: number
  readonly missing_part_ids: readonly string[]
  readonly missing_material_zone_ids: readonly string[]
  readonly duplicate_part_ids: readonly string[]
  readonly orphan_compiled_part_ids: readonly string[]
}

export interface ProceduralDraftFixedViewGate extends ProceduralDraftGate {
  readonly rig_fingerprint: string | null
  readonly view_ids: readonly string[]
  readonly visible_part_count: number
  readonly total_part_count: number
  readonly total_visible_view_count: number
  readonly missing_part_ids: readonly string[]
  readonly underexposed_part_ids: readonly string[]
}

export interface ProceduralDraftStructuralDeltaGate extends ProceduralDraftGate {
  readonly changed_view_count: number
  readonly changed_pixel_count: number
  readonly baseline_source_fingerprint: string | null
  readonly candidate_source_fingerprint: string | null
}

export interface ProceduralDraftBudgetGate extends ProceduralDraftGate {
  readonly triangle_count: number | null
  readonly max_triangles: number | null
  readonly draw_call_count: number | null
  readonly max_draw_calls: number | null
  readonly texture_bytes: 0
  readonly max_texture_bytes: number | null
}

export interface ProceduralDraftGlbGate extends ProceduralDraftGate {
  readonly present: boolean
  readonly program_bytes_sha256: string | null
  readonly glb_sha256: string | null
  readonly glb_bytes: number | null
  readonly glb_header_version: number | null
  readonly mesh_count: number | null
  readonly mesh_node_count: number | null
  readonly material_count: number | null
  readonly triangle_count: number | null
  readonly deterministic_fingerprint: string | null
}

export interface ProceduralDraftReadinessReceipt {
  readonly schema_version: typeof PROCEDURAL_DRAFT_READINESS_SCHEMA
  readonly route: typeof PROCEDURAL_DRAFT_READINESS_ROUTE
  readonly target: 'PROCEDURAL_DRAFT'
  readonly status: ProceduralDraftReadinessStatus
  readonly decision: ProceduralDraftReadinessStatus
  readonly asset_id: string | null
  readonly program_fingerprint: string | null
  readonly program_bytes_sha256: string | null
  readonly compiled_fingerprint: string | null
  readonly gates: {
    readonly closed_program: ProceduralDraftClosedProgramGate
    readonly compilation: ProceduralDraftCompilationGate
    readonly parts_materials: ProceduralDraftPartsMaterialsGate
    readonly fixed_view_observability: ProceduralDraftFixedViewGate
    readonly structural_delta: ProceduralDraftStructuralDeltaGate
    readonly budgets: ProceduralDraftBudgetGate
    readonly glb_receipt: ProceduralDraftGlbGate
  }
  readonly reasons: readonly string[]
  readonly likeness_status: 'NOT_REQUESTED'
  readonly visual_status: 'NOT_REQUESTED'
  readonly human_status: 'NOT_RUN'
  readonly commercial_status: 'NOT_RUN'
  readonly quality_status: 'NOT_RUN'
  /** Browser-safe digest of this evidence projection; not a Runtime/CAS hash. */
  readonly deterministic_fingerprint: string
}

export class ProceduralDraftReadinessError extends Error {
  constructor(message: string) {
    super(`PROCEDURAL_DRAFT_READINESS_INVALID: ${message}`)
    this.name = 'ProceduralDraftReadinessError'
  }
}

/**
 * Evaluate a draft from exact source and artifact bytes. Caller supplied
 * metric, delta, compiled-scene and GLB receipt fields are intentionally not
 * part of the accepted input envelope.
 */
export function evaluateProceduralDraftReadiness(
  input: ProceduralDraftReadinessInput,
): ProceduralDraftReadinessReceipt {
  const reasons: string[] = []
  const inputRecord = isRecord(input) ? input as unknown as Record<string, unknown> : undefined
  if (inputRecord && !hasOnlyKeys(inputRecord, ['program_bytes', 'baseline_program', 'baseline_program_bytes', 'glb_payload'])) {
    reasons.push('closed_program: input contains an unsupported field')
  }
  const candidateBytes = inputRecord ? asBytes(inputRecord.program_bytes) : undefined
  const glbBytes = inputRecord ? asBytes(inputRecord.glb_payload) : undefined
  const programBytesSha256 = candidateBytes ? sha256Hex(candidateBytes) : null
  const candidateProgram = decodeProgram(candidateBytes, 'program', reasons)
  const assetId = typeof candidateProgram?.asset_id === 'string' ? candidateProgram.asset_id : null

  const closedProgram = evaluateClosedProgram(candidateProgram, candidateBytes, reasons)
  const program = closedProgram.status === 'PASS' ? candidateProgram as unknown as KnifeSceneProgram : undefined
  const programFingerprint = closedProgram.status === 'PASS' ? closedProgram.canonical_sha256 : null

  let compiled: CompiledKnifeScene | undefined
  const compilation = evaluateCompilation(program, reasons, (value) => { compiled = value })
  const actualCompiled = compiled
  const partsMaterials = evaluatePartsMaterials(program, actualCompiled, reasons)
  const budgets = evaluateBudgets(program, actualCompiled, reasons)
  const rig = createFixedRig()
  const fixedViewObservability = evaluateFixedView(actualCompiled, rig, reasons)

  const baselineProgram = resolveBaselineProgram(inputRecord, reasons)
  let baselineCompiled: CompiledKnifeScene | undefined
  if (baselineProgram) {
    try {
      baselineCompiled = compileKnifeSceneProgram(baselineProgram)
    } catch (error) {
      reasons.push(`structural_delta: baseline program failed compilation: ${error instanceof Error ? error.message : String(error)}`)
    }
  }
  const structuralDelta = evaluateStructuralDelta(baselineCompiled, actualCompiled, rig, reasons)
  const glbReceipt = evaluateGlbPayload(glbBytes, program, actualCompiled, programBytesSha256, reasons)

  const gates = Object.freeze({
    closed_program: closedProgram,
    compilation,
    parts_materials: partsMaterials,
    fixed_view_observability: fixedViewObservability,
    structural_delta: structuralDelta,
    budgets,
    glb_receipt: glbReceipt,
  })
  const allPassed = Object.values(gates).every((gate) => gate.status === 'PASS')
  const status: ProceduralDraftReadinessStatus = allPassed ? 'THREEJS_DESIGN_READY' : 'BLOCKED'
  const fingerprint = fnv1a64(canonicalJson({
    schema_version: PROCEDURAL_DRAFT_READINESS_SCHEMA,
    status,
    asset_id: assetId,
    program_fingerprint: programFingerprint,
    program_bytes_sha256: programBytesSha256,
    compiled_fingerprint: actualCompiled?.deterministic_fingerprint ?? null,
    gates,
    reasons,
    likeness_status: 'NOT_REQUESTED',
    visual_status: 'NOT_REQUESTED',
    human_status: 'NOT_RUN',
    commercial_status: 'NOT_RUN',
    quality_status: 'NOT_RUN',
  }))

  return Object.freeze({
    schema_version: PROCEDURAL_DRAFT_READINESS_SCHEMA,
    route: PROCEDURAL_DRAFT_READINESS_ROUTE,
    target: 'PROCEDURAL_DRAFT',
    status,
    decision: status,
    asset_id: assetId,
    program_fingerprint: programFingerprint,
    program_bytes_sha256: programBytesSha256,
    compiled_fingerprint: actualCompiled?.deterministic_fingerprint ?? null,
    gates,
    reasons: Object.freeze([...new Set(reasons)]),
    likeness_status: 'NOT_REQUESTED',
    visual_status: 'NOT_REQUESTED',
    human_status: 'NOT_RUN',
    commercial_status: 'NOT_RUN',
    quality_status: 'NOT_RUN',
    deterministic_fingerprint: fingerprint,
  })
}

export const assessProceduralDraftReadiness = evaluateProceduralDraftReadiness

/** Measure a structural change from two evaluations of the same fixed rig. */
export function measureProceduralDraftStructuralDelta(
  baseline: KnifeEightViewEvaluation,
  candidate: KnifeEightViewEvaluation,
): ProceduralDraftStructuralDelta {
  if (!baseline || !candidate
    || baseline.rig.deterministic_fingerprint !== candidate.rig.deterministic_fingerprint
    || baseline.views.length !== candidate.views.length
    || baseline.views.length !== KNIFE_VIEW_IDS.length) {
    throw new ProceduralDraftReadinessError('baseline and candidate must share the complete fixed eight-view rig')
  }
  let changedViewCount = 0
  let silhouetteChangedPixelCount = 0
  let partIdChangedPixelCount = 0
  for (let viewIndex = 0; viewIndex < baseline.views.length; viewIndex += 1) {
    const baseView = baseline.views[viewIndex]
    const candidateView = candidate.views[viewIndex]
    if (baseView.view_id !== KNIFE_VIEW_IDS[viewIndex]
      || candidateView.view_id !== KNIFE_VIEW_IDS[viewIndex]
      || baseView.mask.pixels.length !== candidateView.mask.pixels.length
      || baseView.mask.part_indices.length !== candidateView.mask.part_indices.length) {
      throw new ProceduralDraftReadinessError('structural delta view or mask binding drifted')
    }
    let changed = false
    for (let pixelIndex = 0; pixelIndex < baseView.mask.pixels.length; pixelIndex += 1) {
      if (baseView.mask.pixels[pixelIndex] !== candidateView.mask.pixels[pixelIndex]) {
        silhouetteChangedPixelCount += 1
        changed = true
      }
      if (baseView.mask.part_indices[pixelIndex] !== candidateView.mask.part_indices[pixelIndex]) {
        partIdChangedPixelCount += 1
        changed = true
      }
    }
    if (changed) changedViewCount += 1
  }
  return Object.freeze({
    schema_version: 'KnifeCandidateStructuralDelta@1',
    baseline_source_fingerprint: baseline.receipt.source_fingerprint,
    candidate_source_fingerprint: candidate.receipt.source_fingerprint,
    changed_view_count: changedViewCount,
    silhouette_changed_pixel_count: silhouetteChangedPixelCount,
    part_id_changed_pixel_count: partIdChangedPixelCount,
    minimum_changed_pixel_count: 1,
    status: silhouetteChangedPixelCount + partIdChangedPixelCount > 0
      ? 'MEASURED_NONZERO'
      : 'REJECTED_NO_VISIBLE_DELTA',
    quality_status: 'NOT_RUN',
  })
}

function evaluateClosedProgram(
  value: unknown,
  bytes: Uint8Array | undefined,
  reasons: string[],
): ProceduralDraftClosedProgramGate {
  try {
    assertClosedProgram(value)
    if (!bytes) throw new ProceduralDraftReadinessError('program_bytes must be a non-empty Uint8Array')
    const canonicalSha = canonicalProgramSha256(bytes)
    if (value.canonical_sha256 !== '' && value.canonical_sha256 !== canonicalSha) {
      throw new ProceduralDraftReadinessError('canonical_sha256 does not match the canonical program bytes')
    }
    return Object.freeze({
      status: 'PASS',
      reason: null,
      schema_version: value.schema_version,
      asset_id: value.asset_id,
      canonical_sha256: canonicalSha,
    })
  } catch (error) {
    return blockedClosedProgram(error instanceof Error ? error.message : String(error), reasons)
  }
}

function blockedClosedProgram(reason: string, reasons: string[]): ProceduralDraftClosedProgramGate {
  reasons.push(`closed_program: ${reason}`)
  return Object.freeze({ status: 'BLOCKED', reason, schema_version: null, asset_id: null, canonical_sha256: null })
}

function evaluateCompilation(
  program: KnifeSceneProgram | undefined,
  reasons: string[],
  assign: (compiled: CompiledKnifeScene) => void,
): ProceduralDraftCompilationGate {
  if (!program) {
    const reason = 'closed program gate did not pass'
    reasons.push(`compilation: ${reason}`)
    return Object.freeze({ status: 'BLOCKED', reason, deterministic_fingerprint: null, triangle_count: null, draw_call_count: null })
  }
  try {
    const compiled = compileKnifeSceneProgram(program)
    assign(compiled)
    return Object.freeze({
      status: 'PASS',
      reason: null,
      deterministic_fingerprint: compiled.deterministic_fingerprint,
      triangle_count: compiled.triangle_count,
      draw_call_count: compiled.parts.length,
    })
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error)
    reasons.push(`compilation: ${reason}`)
    return Object.freeze({ status: 'BLOCKED', reason, deterministic_fingerprint: null, triangle_count: null, draw_call_count: null })
  }
}

function evaluatePartsMaterials(
  program: KnifeSceneProgram | undefined,
  compiled: CompiledKnifeScene | undefined,
  reasons: string[],
): ProceduralDraftPartsMaterialsGate {
  const programParts = program?.parts ?? []
  const programZones = program?.material_zones ?? []
  const compiledParts = compiled?.parts ?? []
  const programPartIds = programParts.map((part) => part.part_id)
  const compiledPartIds = compiledParts.map((part) => part.part_id)
  const duplicatePartIds = [...new Set([...duplicates(programPartIds), ...duplicates(compiledPartIds)])]
  const missingPartIds = programPartIds.filter((partId) => !compiledPartIds.includes(partId))
  const orphanCompiledPartIds = compiledPartIds.filter((partId) => !programPartIds.includes(partId))
  const zoneIds = new Set(programZones.map((zone) => zone.material_zone_id))
  const missingMaterialZoneIds = compiledParts
    .filter((part) => !zoneIds.has(part.material_zone_id))
    .map((part) => part.material_zone_id)
  const status: ProceduralDraftGateStatus = Boolean(program && compiled
    && programParts.length > 0
    && programZones.length > 0
    && missingPartIds.length === 0
    && orphanCompiledPartIds.length === 0
    && missingMaterialZoneIds.length === 0
    && duplicatePartIds.length === 0)
    ? 'PASS'
    : 'BLOCKED'
  const reason = status === 'PASS' ? null : 'every renderable Part and compiled material binding must have one semantic owner'
  if (reason) reasons.push(`parts_materials: ${reason}`)
  return Object.freeze({
    status,
    reason,
    part_count: programParts.length,
    compiled_part_count: compiledParts.length,
    material_zone_count: programZones.length,
    missing_part_ids: Object.freeze([...new Set(missingPartIds)]),
    missing_material_zone_ids: Object.freeze([...new Set(missingMaterialZoneIds)]),
    duplicate_part_ids: Object.freeze(duplicatePartIds),
    orphan_compiled_part_ids: Object.freeze([...new Set(orphanCompiledPartIds)]),
  })
}

function evaluateBudgets(
  program: KnifeSceneProgram | undefined,
  compiled: CompiledKnifeScene | undefined,
  reasons: string[],
): ProceduralDraftBudgetGate {
  const triangles = compiled?.triangle_count ?? null
  const draws = compiled?.parts.length ?? null
  const maxTriangles = program?.budgets.max_triangles ?? null
  const maxDraws = program?.budgets.max_draw_calls ?? null
  const maxTextureBytes = program?.budgets.max_texture_bytes ?? null
  const validLimits = [maxTriangles, maxDraws, maxTextureBytes].every((value) => value !== null && Number.isInteger(value) && value >= 0)
  const pass = Boolean(compiled && validLimits && triangles !== null && draws !== null
    && maxTriangles !== null && maxDraws !== null && maxTextureBytes !== null
    && triangles <= maxTriangles && draws <= maxDraws)
  const reason = pass ? null : 'compiled geometry or declared budgets are missing, invalid, or exceeded'
  if (reason) reasons.push(`budgets: ${reason}`)
  return Object.freeze({
    status: pass ? 'PASS' : 'BLOCKED',
    reason,
    triangle_count: triangles,
    max_triangles: maxTriangles,
    draw_call_count: draws,
    max_draw_calls: maxDraws,
    texture_bytes: 0,
    max_texture_bytes: maxTextureBytes,
  })
}

function createFixedRig(): KnifeViewRig {
  return createKnifeFixedEightViewRig()
}

function evaluateFixedView(
  compiled: CompiledKnifeScene | undefined,
  rig: KnifeViewRig,
  reasons: string[],
): ProceduralDraftFixedViewGate {
  if (!compiled) return blockedFixedView('compiled scene is unavailable', reasons)
  try {
    const metrics = measureKnifePartVisibilityMetrics(compiled, rig)
    const visiblePartCount = metrics.parts.filter((part) => part.visible_view_count > 0).length
    const totalVisibleViewCount = metrics.parts.reduce((sum, part) => sum + part.visible_view_count, 0)
    const observedViewIds = new Set(metrics.parts.flatMap((part) => part.views
      .filter((view) => view.visible_pixel_count > 0)
      .map((view) => view.view_id)))
    const allViewsObserved = KNIFE_VIEW_IDS.every((viewId) => observedViewIds.has(viewId))
    const status: ProceduralDraftGateStatus = metrics.missing_part_ids.length === 0
      && visiblePartCount === compiled.parts.length
      && metrics.parts.length === compiled.parts.length
      && metrics.view_ids.join('|') === KNIFE_VIEW_IDS.join('|')
      && metrics.rig_fingerprint === rig.deterministic_fingerprint
      && totalVisibleViewCount > 0
      && allViewsObserved
      ? 'PASS'
      : 'BLOCKED'
    const reason = status === 'PASS' ? null : 'one or more semantic Parts are not observable in the canonical fixed eight-view rig'
    if (reason) reasons.push(`fixed_view_observability: ${reason}`)
    return Object.freeze({
      status,
      reason,
      rig_fingerprint: metrics.rig_fingerprint,
      view_ids: Object.freeze([...metrics.view_ids]),
      visible_part_count: visiblePartCount,
      total_part_count: metrics.parts.length,
      total_visible_view_count: totalVisibleViewCount,
      missing_part_ids: Object.freeze([...metrics.missing_part_ids]),
      underexposed_part_ids: Object.freeze([...metrics.underexposed_part_ids]),
    })
  } catch (error) {
    return blockedFixedView(error instanceof Error ? error.message : String(error), reasons)
  }
}

function blockedFixedView(reason: string, reasons: string[]): ProceduralDraftFixedViewGate {
  reasons.push(`fixed_view_observability: ${reason}`)
  return Object.freeze({
    status: 'BLOCKED',
    reason,
    rig_fingerprint: null,
    view_ids: Object.freeze([]),
    visible_part_count: 0,
    total_part_count: 0,
    total_visible_view_count: 0,
    missing_part_ids: Object.freeze([]),
    underexposed_part_ids: Object.freeze([]),
  })
}

function evaluateStructuralDelta(
  baseline: CompiledKnifeScene | undefined,
  candidate: CompiledKnifeScene | undefined,
  rig: KnifeViewRig,
  reasons: string[],
): ProceduralDraftStructuralDeltaGate {
  if (!baseline || !candidate) {
    return blockedStructuralDelta('baseline_program is required for a recomputable structural delta', reasons)
  }
  try {
    const delta = measureProceduralDraftStructuralDelta(
      evaluateKnifeRig(baseline, rig),
      evaluateKnifeRig(candidate, rig),
    )
    if (delta.status !== 'MEASURED_NONZERO') {
      throw new ProceduralDraftReadinessError('structural delta has no non-zero fixed-view change')
    }
    return Object.freeze({
      status: 'PASS',
      reason: null,
      changed_view_count: delta.changed_view_count,
      changed_pixel_count: delta.silhouette_changed_pixel_count + delta.part_id_changed_pixel_count,
      baseline_source_fingerprint: delta.baseline_source_fingerprint,
      candidate_source_fingerprint: delta.candidate_source_fingerprint,
    })
  } catch (error) {
    return blockedStructuralDelta(error instanceof Error ? error.message : String(error), reasons)
  }
}

function blockedStructuralDelta(reason: string, reasons: string[]): ProceduralDraftStructuralDeltaGate {
  reasons.push(`structural_delta: ${reason}`)
  return Object.freeze({
    status: 'BLOCKED',
    reason,
    changed_view_count: 0,
    changed_pixel_count: 0,
    baseline_source_fingerprint: null,
    candidate_source_fingerprint: null,
  })
}

interface GlbReadback {
  readonly mesh_count: number
  readonly mesh_node_count: number
  readonly material_count: number
  readonly triangle_count: number
}

function evaluateGlbPayload(
  payload: Uint8Array | undefined,
  program: KnifeSceneProgram | undefined,
  compiled: CompiledKnifeScene | undefined,
  programBytesSha256: string | null,
  reasons: string[],
): ProceduralDraftGlbGate {
  if (!payload) return blockedGlbReceipt('glb_payload must be a non-empty Uint8Array', reasons)
  try {
    if (!program || !compiled || !programBytesSha256) throw new ProceduralDraftReadinessError('program and compiled scene are required for GLB readback')
    const readback = readGlbPayload(payload, compiled, program)
    const glbSha256 = sha256Hex(payload)
    return Object.freeze({
      status: 'PASS',
      reason: null,
      present: true,
      program_bytes_sha256: programBytesSha256,
      glb_sha256: glbSha256,
      glb_bytes: payload.byteLength,
      glb_header_version: 2,
      mesh_count: readback.mesh_count,
      mesh_node_count: readback.mesh_node_count,
      material_count: readback.material_count,
      triangle_count: readback.triangle_count,
      deterministic_fingerprint: compiled.deterministic_fingerprint,
    })
  } catch (error) {
    return blockedGlbReceipt(error instanceof Error ? error.message : String(error), reasons)
  }
}

function blockedGlbReceipt(reason: string, reasons: string[]): ProceduralDraftGlbGate {
  reasons.push(`glb_receipt: ${reason}`)
  return Object.freeze({
    status: 'BLOCKED',
    reason,
    present: false,
    program_bytes_sha256: null,
    glb_sha256: null,
    glb_bytes: null,
    glb_header_version: null,
    mesh_count: null,
    mesh_node_count: null,
    material_count: null,
    triangle_count: null,
    deterministic_fingerprint: null,
  })
}

function readGlbPayload(
  bytes: Uint8Array,
  compiled: CompiledKnifeScene,
  program: KnifeSceneProgram,
): GlbReadback {
  if (bytes.byteLength < 20) throw new ProceduralDraftReadinessError('GLB payload is too small')
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  if (view.getUint32(0, true) !== 0x46546c67) throw new ProceduralDraftReadinessError('GLB magic is invalid')
  if (view.getUint32(4, true) !== 2) throw new ProceduralDraftReadinessError('GLB header version must be 2')
  if (view.getUint32(8, true) !== bytes.byteLength) throw new ProceduralDraftReadinessError('GLB header length does not match payload')

  let offset = 12
  let json: Record<string, unknown> | undefined
  while (offset < bytes.byteLength) {
    if (offset + 8 > bytes.byteLength) throw new ProceduralDraftReadinessError('GLB chunk header is truncated')
    const chunkLength = view.getUint32(offset, true)
    const chunkType = view.getUint32(offset + 4, true)
    offset += 8
    if (chunkLength > bytes.byteLength - offset) throw new ProceduralDraftReadinessError('GLB chunk exceeds payload')
    if (chunkType === 0x4e4f534a) {
      if (json) throw new ProceduralDraftReadinessError('GLB contains multiple JSON chunks')
      const text = new TextDecoder().decode(bytes.subarray(offset, offset + chunkLength))
        .replace(/[\u0000 ]+$/g, '')
        .trim()
      const parsed = JSON.parse(text) as unknown
      if (!isRecord(parsed)) throw new ProceduralDraftReadinessError('GLB JSON chunk must be an object')
      json = parsed
    }
    offset += chunkLength
  }
  if (!json) throw new ProceduralDraftReadinessError('GLB JSON chunk is missing')
  const asset = asRecord(json.asset, 'GLB asset')
  if (asset.version !== '2.0') throw new ProceduralDraftReadinessError('GLB JSON asset.version must be 2.0')
  const meshes = arrayField(json.meshes, 'GLB meshes')
  const nodes = arrayField(json.nodes, 'GLB nodes')
  const materials = arrayField(json.materials, 'GLB materials')
  const accessors = arrayField(json.accessors, 'GLB accessors')
  if (meshes.length === 0 || nodes.length === 0 || materials.length === 0) {
    throw new ProceduralDraftReadinessError('GLB must read back at least one mesh, node and material')
  }
  if (!nodes.some((value) => isRecord(value) && value.name === `knife-scene:${program.asset_id}` && value.mesh === undefined)) {
    throw new ProceduralDraftReadinessError('GLB root node does not bind the program asset_id')
  }
  if (meshes.length !== compiled.parts.length) {
    throw new ProceduralDraftReadinessError('GLB mesh count does not bind the compiled Part count')
  }
  const expectedPartIds = compiled.parts.map((part) => part.part_id)
  const meshNodes = nodes.filter((value) => isRecord(value) && value.mesh !== undefined)
  const meshIndices = new Set<number>()
  const partNodeIds: string[] = []
  for (const value of meshNodes) {
    const node = asRecord(value, 'GLB mesh node')
    if (!Number.isInteger(node.mesh) || (node.mesh as number) < 0 || (node.mesh as number) >= meshes.length) {
      throw new ProceduralDraftReadinessError('GLB node.mesh is outside the mesh table')
    }
    meshIndices.add(node.mesh as number)
    if (typeof node.name === 'string' && node.name.startsWith('knife-part:')) {
      partNodeIds.push(node.name.slice('knife-part:'.length))
    }
  }
  if (meshNodes.length !== meshes.length || meshIndices.size !== meshes.length) {
    throw new ProceduralDraftReadinessError('GLB mesh-node bindings are incomplete')
  }
  if (partNodeIds.length !== expectedPartIds.length
    || new Set(partNodeIds).size !== partNodeIds.length
    || expectedPartIds.some((partId) => !partNodeIds.includes(partId))) {
    throw new ProceduralDraftReadinessError('GLB mesh nodes do not read back the compiled Part IDs')
  }

  const expectedMaterialCount = new Set(compiled.parts.map((part) => part.material_zone_id)).size
  if (materials.length !== expectedMaterialCount) {
    throw new ProceduralDraftReadinessError('GLB material count does not bind the compiled MaterialZone set')
  }
  const usedMaterialIndices = new Set<number>()
  let triangleCount = 0
  for (const meshValue of meshes) {
    const mesh = asRecord(meshValue, 'GLB mesh')
    const primitives = arrayField(mesh.primitives, 'GLB mesh primitives')
    if (primitives.length === 0) throw new ProceduralDraftReadinessError('GLB mesh has no primitive')
    for (const primitiveValue of primitives) {
      const primitive = asRecord(primitiveValue, 'GLB primitive')
      if (primitive.mode !== undefined && primitive.mode !== 4) {
        throw new ProceduralDraftReadinessError('GLB primitive mode must be TRIANGLES')
      }
      const attributes = asRecord(primitive.attributes, 'GLB primitive attributes')
      const positionAccessor = requireIndex(attributes.POSITION, 'GLB POSITION accessor')
      const positionCount = accessorCount(accessors, positionAccessor, 'GLB POSITION accessor')
      const indexCount = primitive.indices === undefined
        ? positionCount
        : accessorCount(accessors, requireIndex(primitive.indices, 'GLB index accessor'), 'GLB index accessor')
      if (indexCount < 3 || indexCount % 3 !== 0) throw new ProceduralDraftReadinessError('GLB primitive triangle index count is invalid')
      if (primitive.material === undefined) throw new ProceduralDraftReadinessError('GLB primitive material reference is missing')
      const materialIndex = requireIndex(primitive.material, 'GLB material reference')
      if (materialIndex >= materials.length) throw new ProceduralDraftReadinessError('GLB primitive material is outside the material table')
      usedMaterialIndices.add(materialIndex)
      triangleCount += indexCount / 3
    }
  }
  if (triangleCount !== compiled.triangle_count) {
    throw new ProceduralDraftReadinessError('GLB triangle count does not bind the compiled scene')
  }
  if (usedMaterialIndices.size !== materials.length) {
    throw new ProceduralDraftReadinessError('GLB material table contains unreferenced materials')
  }
  // Keep this explicit: the readback must be tied to the same program, not a
  // merely shape-compatible GLB from another asset.
  if (program.asset_id.length === 0) throw new ProceduralDraftReadinessError('program asset_id is empty')
  return {
    mesh_count: meshes.length,
    mesh_node_count: meshNodes.length,
    material_count: materials.length,
    triangle_count: triangleCount,
  }
}

function arrayField(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) throw new ProceduralDraftReadinessError(`${label} must be an array`)
  return value
}

function accessorCount(accessors: readonly unknown[], index: number, label: string): number {
  if (index < 0 || index >= accessors.length) throw new ProceduralDraftReadinessError(`${label} is outside the accessor table`)
  const accessor = asRecord(accessors[index], label)
  if (!Number.isInteger(accessor.count) || (accessor.count as number) < 3) throw new ProceduralDraftReadinessError(`${label}.count is invalid`)
  return accessor.count as number
}

function requireIndex(value: unknown, label: string): number {
  if (!Number.isInteger(value) || (value as number) < 0) throw new ProceduralDraftReadinessError(`${label} must be a non-negative integer`)
  return value as number
}

function resolveBaselineProgram(
  input: Record<string, unknown> | undefined,
  reasons: string[],
): KnifeSceneProgram | undefined {
  if (!input) return undefined
  const baselineBytes = input.baseline_program_bytes === undefined
    ? undefined
    : asBytes(input.baseline_program_bytes)
  const provided = input.baseline_program
  if (input.baseline_program_bytes !== undefined && !baselineBytes) {
    reasons.push('structural_delta: baseline_program_bytes must be a non-empty Uint8Array')
    return undefined
  }
  if (!baselineBytes && provided === undefined) return undefined
  try {
    const baseline = baselineBytes ? decodeJsonBytes(baselineBytes, 'baseline_program') : provided
    assertClosedProgram(baseline)
    if (baselineBytes) {
      const canonicalSha = canonicalProgramSha256(baselineBytes)
      if (baseline.canonical_sha256 !== '' && baseline.canonical_sha256 !== canonicalSha) {
        throw new ProceduralDraftReadinessError('baseline canonical_sha256 does not match baseline program bytes')
      }
      if (provided !== undefined && canonicalJson(provided) !== canonicalJson(baseline)) {
        throw new ProceduralDraftReadinessError('baseline_program does not match baseline_program_bytes')
      }
    }
    return baseline
  } catch (error) {
    reasons.push(`structural_delta: ${error instanceof Error ? error.message : String(error)}`)
    return undefined
  }
}

function decodeProgram(
  bytes: Uint8Array | undefined,
  label: string,
  reasons: string[],
): Record<string, unknown> | undefined {
  try {
    const value = decodeJsonBytes(bytes, label)
    return isRecord(value) ? value : undefined
  } catch (error) {
    reasons.push(`closed_program: ${error instanceof Error ? error.message : String(error)}`)
    return undefined
  }
}

function decodeJsonBytes(bytes: Uint8Array | undefined, label: string): unknown {
  if (!bytes || bytes.byteLength === 0) throw new ProceduralDraftReadinessError(`${label} bytes are required`)
  try {
    return JSON.parse(new TextDecoder().decode(bytes)) as unknown
  } catch (error) {
    throw new ProceduralDraftReadinessError(`${label} bytes are not valid JSON: ${error instanceof Error ? error.message : String(error)}`)
  }
}

function asBytes(value: unknown): Uint8Array | undefined {
  if (!(value instanceof Uint8Array) || value.byteLength === 0) return undefined
  return value
}

function assertClosedProgram(value: unknown): asserts value is KnifeSceneProgram {
  const program = asRecord(value, 'program')
  assertExactKeys(program, [
    'schema_version',
    'asset_id',
    'family',
    'design_basis',
    'coordinate_convention',
    'blade_surface',
    'assembly',
    'source_envelope',
    'parts',
    'material_zones',
    'presentation',
    'budgets',
    'unknowns',
    'canonical_sha256',
  ], 'program')
  if (program.schema_version !== 'KnifeSceneProgram@1') throw new ProceduralDraftReadinessError('schema_version must be KnifeSceneProgram@1')
  if (typeof program.asset_id !== 'string' || program.asset_id.length < 1) throw new ProceduralDraftReadinessError('asset_id must be text')
  if (!['kukri', 'tanto', 'karambit', 'bayonet', 'machete', 'original-knife'].includes(program.family as string)) {
    throw new ProceduralDraftReadinessError('family is unsupported')
  }
  if (!['authorized-reference-inspired', 'original-design', 'img2threejs-compatible-import'].includes(program.design_basis as string)) {
    throw new ProceduralDraftReadinessError('design_basis is unsupported')
  }
  if (program.coordinate_convention !== 'weapon-front-z-up-right-handed@1') throw new ProceduralDraftReadinessError('coordinate convention is unsupported')
  if (!Array.isArray(program.parts) || !Array.isArray(program.material_zones) || !Array.isArray(program.unknowns)) {
    throw new ProceduralDraftReadinessError('parts, material_zones and unknowns must be arrays')
  }
  const blade = asRecord(program.blade_surface, 'blade_surface')
  assertExactKeys(blade, ['spine_curve', 'cutting_edge_curve', 'sections', 'surface_roles'], 'blade_surface')
  validateCurveShape(blade.spine_curve, 'spine_curve')
  validateCurveShape(blade.cutting_edge_curve, 'cutting_edge_curve')
  if (!Array.isArray(blade.sections)) throw new ProceduralDraftReadinessError('blade_surface.sections must be an array')
  for (const section of blade.sections) {
    const valueSection = asRecord(section, 'section')
    assertExactKeys(valueSection, ['section_id', 'role', 'u', 'half_width', 'thickness', 'edge_offset', 'spine_offset', 'asymmetry', 'twist'], 'section')
  }
  for (const part of program.parts) {
    assertExactKeys(asRecord(part, 'part'), ['part_id', 'role', 'source_class', 'material_zone_id', 'frozen'], 'part')
    const partRecord = part as Record<string, unknown>
    if (typeof partRecord.part_id !== 'string'
      || !['blade-body', 'cutting-edge', 'guard', 'grip', 'pommel', 'fastener', 'gem', 'relief', 'helper'].includes(partRecord.role as string)
      || !['observed', 'inferred', 'design-prior', 'original-choice'].includes(partRecord.source_class as string)
      || typeof partRecord.material_zone_id !== 'string'
      || typeof partRecord.frozen !== 'boolean') {
      throw new ProceduralDraftReadinessError('part contains an invalid semantic ownership field')
    }
  }
  for (const zone of program.material_zones) {
    assertExactKeys(asRecord(zone, 'material zone'), ['material_zone_id', 'model', 'base_color', 'metalness', 'roughness'], 'material zone')
    const zoneRecord = zone as Record<string, unknown>
    if (typeof zoneRecord.material_zone_id !== 'string' || zoneRecord.model !== 'mesh-standard-layered@1') {
      throw new ProceduralDraftReadinessError('material zone contains an invalid ownership field')
    }
  }
  const presentation = asRecord(program.presentation, 'presentation')
  assertExactKeys(presentation, ['camera_set', 'renderer', 'aovs'], 'presentation')
  if (presentation.camera_set !== 'knife-fixed-eight-view@1'
    || presentation.renderer !== 'threejs-browser-authority@1'
    || !Array.isArray(presentation.aovs)
    || presentation.aovs.length < 1) {
    throw new ProceduralDraftReadinessError('presentation must bind the fixed eight-view browser authority')
  }
  const budgets = asRecord(program.budgets, 'budgets')
  assertExactKeys(budgets, ['max_triangles', 'max_draw_calls', 'max_texture_bytes'], 'budgets')
  if (program.assembly !== undefined) validateAssemblyShape(program.assembly)
  if (program.source_envelope !== undefined && !isRecord(program.source_envelope)) throw new ProceduralDraftReadinessError('source_envelope must be an object')
  for (const unknown of program.unknowns) if (typeof unknown !== 'string') throw new ProceduralDraftReadinessError('unknowns must contain text')
  if (typeof program.canonical_sha256 !== 'string' || (program.canonical_sha256 !== '' && !isSha256(program.canonical_sha256))) {
    throw new ProceduralDraftReadinessError('canonical_sha256 must be empty or SHA-256')
  }
  assertJsonSafeAndClosed(program)
}

function validateCurveShape(value: unknown, label: string): void {
  const curve = asRecord(value, label)
  assertExactKeys(curve, ['curve_id', 'basis', 'control_points'], label)
  if (!Array.isArray(curve.control_points)) throw new ProceduralDraftReadinessError(`${label}.control_points must be an array`)
  for (const point of curve.control_points) {
    if (!Array.isArray(point) || point.length !== 3) throw new ProceduralDraftReadinessError(`${label} contains an invalid point`)
  }
}

function validateAssemblyShape(value: unknown): void {
  const assembly = asRecord(value, 'assembly')
  assertExactKeys(assembly, ['guard', 'grip', 'pommel', 'fasteners', 'gems', 'reliefs'], 'assembly')
  for (const key of ['guard', 'grip', 'pommel'] as const) if (assembly[key] !== undefined) asRecord(assembly[key], `assembly.${key}`)
  for (const key of ['fasteners', 'gems', 'reliefs'] as const) {
    if (assembly[key] !== undefined && !Array.isArray(assembly[key])) throw new ProceduralDraftReadinessError(`assembly.${key} must be an array`)
  }
}

function assertJsonSafeAndClosed(value: unknown, path = 'program'): void {
  if (value === null || typeof value === 'boolean' || typeof value === 'number') {
    if (typeof value === 'number' && !Number.isFinite(value)) throw new ProceduralDraftReadinessError(`${path} contains a non-finite number`)
    return
  }
  if (typeof value === 'string') {
    if (value.includes('/') || value.includes('\\') || /(?:https?:|javascript:|data:|file:)/i.test(value)) {
      throw new ProceduralDraftReadinessError(`${path} cannot contain a URL, path or script value`)
    }
    return
  }
  if (Array.isArray(value)) {
    value.forEach((child, index) => assertJsonSafeAndClosed(child, `${path}[${index}]`))
    return
  }
  if (!isRecord(value)) throw new ProceduralDraftReadinessError(`${path} is not JSON-like`)
  for (const [key, child] of Object.entries(value)) {
    if (/^(?:path|url|script|command|shell|secret|token|api[_-]?key|env)$/i.test(key)) {
      throw new ProceduralDraftReadinessError(`${path}.${key} is not an allowed typed field`)
    }
    assertJsonSafeAndClosed(child, `${path}.${key}`)
  }
}

function canonicalProgramSha256(bytes: Uint8Array): string {
  return sha256Hex(new CanonicalJsonParser(new TextDecoder().decode(bytes)).parse())
}

/** Parse JSON while retaining source number spellings (notably Python's 0.0). */
class CanonicalJsonParser {
  private readonly text: string
  private offset = 0

  constructor(text: string) {
    this.text = text
  }

  parse(): string {
    const result = this.parseValue()
    this.skipWhitespace()
    if (this.offset !== this.text.length) throw new ProceduralDraftReadinessError('program bytes contain trailing JSON data')
    return result
  }

  private parseValue(): string {
    this.skipWhitespace()
    const char = this.text[this.offset]
    if (char === '{') return this.parseObject()
    if (char === '[') return this.parseArray()
    if (char === '"') return JSON.stringify(this.parseString())
    if (char === 't' && this.take('true')) return 'true'
    if (char === 'f' && this.take('false')) return 'false'
    if (char === 'n' && this.take('null')) return 'null'
    return this.parseNumber()
  }

  private parseObject(): string {
    this.offset += 1
    this.skipWhitespace()
    const entries: Array<{ readonly key: string; readonly value: string }> = []
    const keys = new Set<string>()
    if (this.text[this.offset] !== '}') {
      while (true) {
        this.skipWhitespace()
        if (this.text[this.offset] !== '"') throw new ProceduralDraftReadinessError('program object key must be a string')
        const key = this.parseString()
        if (keys.has(key)) throw new ProceduralDraftReadinessError(`program contains duplicate key ${key}`)
        keys.add(key)
        this.skipWhitespace()
        if (this.text[this.offset] !== ':') throw new ProceduralDraftReadinessError('program object key is missing a colon')
        this.offset += 1
        const value = this.parseValue()
        entries.push({ key, value: key === 'canonical_sha256' ? '""' : value })
        this.skipWhitespace()
        if (this.text[this.offset] === '}') break
        if (this.text[this.offset] !== ',') throw new ProceduralDraftReadinessError('program object is missing a comma')
        this.offset += 1
      }
    }
    if (this.text[this.offset] !== '}') throw new ProceduralDraftReadinessError('program object is unterminated')
    this.offset += 1
    entries.sort((left, right) => left.key < right.key ? -1 : left.key > right.key ? 1 : 0)
    return `{${entries.map((entry) => `${JSON.stringify(entry.key)}:${entry.value}`).join(',')}}`
  }

  private parseArray(): string {
    this.offset += 1
    this.skipWhitespace()
    const values: string[] = []
    if (this.text[this.offset] !== ']') {
      while (true) {
        values.push(this.parseValue())
        this.skipWhitespace()
        if (this.text[this.offset] === ']') break
        if (this.text[this.offset] !== ',') throw new ProceduralDraftReadinessError('program array is missing a comma')
        this.offset += 1
      }
    }
    if (this.text[this.offset] !== ']') throw new ProceduralDraftReadinessError('program array is unterminated')
    this.offset += 1
    return `[${values.join(',')}]`
  }

  private parseString(): string {
    const start = this.offset
    this.offset += 1
    while (this.offset < this.text.length) {
      const char = this.text[this.offset]
      if (char === '\\') {
        this.offset += 2
        continue
      }
      this.offset += 1
      if (char === '"') {
        try {
          return JSON.parse(this.text.slice(start, this.offset)) as string
        } catch {
          throw new ProceduralDraftReadinessError('program contains an invalid JSON string')
        }
      }
    }
    throw new ProceduralDraftReadinessError('program string is unterminated')
  }

  private parseNumber(): string {
    const match = this.text.slice(this.offset).match(/^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/)
    if (!match) throw new ProceduralDraftReadinessError(`program contains an invalid JSON value at byte ${this.offset}`)
    this.offset += match[0].length
    return pythonFloatNumber(match[0])
  }

  private take(value: string): boolean {
    if (this.text.slice(this.offset, this.offset + value.length) !== value) return false
    this.offset += value.length
    return true
  }

  private skipWhitespace(): void {
    while (/\s/.test(this.text[this.offset] ?? '')) this.offset += 1
  }
}

/** Match Python json.dumps' compact float spelling used by the checked-in programs. */
function pythonFloatNumber(raw: string): string {
  if (!/[.eE]/.test(raw)) return raw === '-0' ? '0' : raw
  const value = Number(raw)
  if (!Number.isFinite(value)) throw new ProceduralDraftReadinessError('program contains a non-finite number')
  if (Object.is(value, -0)) return '-0.0'
  let spelling = value.toString()
  const absolute = Math.abs(value)
  if (spelling.includes('e')) spelling = normalizeExponent(spelling)
  if (!spelling.includes('e') && (absolute !== 0 && (absolute < 1e-4 || absolute >= 1e16))) {
    spelling = decimalToExponent(spelling)
  }
  if (!spelling.includes('.') && !spelling.includes('e')) spelling += '.0'
  return spelling
}

function normalizeExponent(value: string): string {
  const [mantissa, exponentText] = value.split('e')
  const exponent = Number(exponentText)
  return `${mantissa}e${exponent >= 0 ? '+' : '-'}${Math.abs(exponent).toString().padStart(2, '0')}`
}

function decimalToExponent(value: string): string {
  const negative = value.startsWith('-')
  const unsigned = negative ? value.slice(1) : value
  const [whole, fraction = ''] = unsigned.split('.')
  const digits = `${whole}${fraction}`
  const first = digits.search(/[1-9]/)
  if (first < 0) return `${negative ? '-' : ''}0.0`
  const exponent = whole !== '0' ? whole.length - first - 1 : -(fraction.search(/[1-9]/) + 1)
  const significant = `${digits.slice(first, first + 1)}.${digits.slice(first + 1)}`.replace(/\.0+$/, '')
  return `${negative ? '-' : ''}${significant}e${exponent >= 0 ? '+' : '-'}${Math.abs(exponent).toString().padStart(2, '0')}`
}

function asRecord(value: unknown, label: string): Record<string, any> {
  if (!isRecord(value)) throw new ProceduralDraftReadinessError(`${label} must be an object`)
  return value
}

function assertExactKeys(value: Record<string, unknown>, allowed: readonly string[], label: string): void {
  const allowedSet = new Set(allowed)
  for (const key of Object.keys(value)) if (!allowedSet.has(key)) throw new ProceduralDraftReadinessError(`${label} contains unsupported field ${key}`)
}

function hasOnlyKeys(value: object, allowed: readonly string[]): boolean {
  const allowedSet = new Set(allowed)
  return Object.keys(value).every((key) => allowedSet.has(key))
}

function isRecord(value: unknown): value is Record<string, any> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function duplicates(values: readonly string[]): string[] {
  const seen = new Set<string>()
  const duplicate = new Set<string>()
  for (const value of values) {
    if (seen.has(value)) duplicate.add(value)
    seen.add(value)
  }
  return [...duplicate]
}

function isSha256(value: unknown): value is string {
  return typeof value === 'string' && /^[a-f0-9]{64}$/i.test(value)
}

function canonicalJson(value: unknown): string {
  if (value === null) return 'null'
  if (typeof value === 'string') return JSON.stringify(value)
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new ProceduralDraftReadinessError('canonical value contains a non-finite number')
    return Object.is(value, -0) ? '0' : JSON.stringify(value)
  }
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  if (isRecord(value)) return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`
  throw new ProceduralDraftReadinessError('canonical value contains an unsupported type')
}

function fnv1a64(value: string): string {
  let hash = 0xcbf29ce484222325n
  const prime = 0x100000001b3n
  const mask = 0xffffffffffffffffn
  for (let index = 0; index < value.length; index += 1) {
    hash ^= BigInt(value.charCodeAt(index))
    hash = (hash * prime) & mask
  }
  return hash.toString(16).padStart(16, '0')
}
