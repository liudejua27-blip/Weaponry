import {
  createKnifeViewRig,
  evaluateKnifeRig,
  KNIFE_VIEW_IDS,
  type KnifeEightViewEvaluation,
  type KnifeViewId,
  type KnifeViewRig,
} from './knife-view-evaluation.ts'
import type { CompiledKnifeScene } from './knife-scene-compiler.ts'

/** Closed schema for renderer-independent per-part fixed-view measurements. */
export const KNIFE_PART_VISIBILITY_METRICS_SCHEMA = 'KnifePartVisibilityMetrics@1' as const
export const KNIFE_PART_VISIBILITY_METRICS_STATUS = 'MEASURED_NOT_REVIEWED' as const

/**
 * These are raster observability floors, not visual-quality thresholds.
 *
 * A part is missing when the depth-resolved mask assigns it no pixels in any
 * fixed view. A non-missing part is underexposed when it is observed in fewer
 * than two of the eight views, or never reaches a 2x2 pixel footprint in one
 * view. Four pixels is deliberately a resolution/aliasing floor; it does not
 * express an artistic or commercial acceptance criterion.
 */
export const KNIFE_PART_VISIBILITY_THRESHOLDS = Object.freeze({
  missing_visible_view_count: 0,
  underexposed_min_visible_view_count: 2,
  underexposed_min_visible_pixel_count_in_any_view: 4,
}) as {
  readonly missing_visible_view_count: 0
  readonly underexposed_min_visible_view_count: 2
  readonly underexposed_min_visible_pixel_count_in_any_view: 4
}

export type KnifePartVisibilityMetricsStatus = typeof KNIFE_PART_VISIBILITY_METRICS_STATUS

export interface KnifePartVisibilityViewMetric {
  readonly view_id: KnifeViewId
  /** Count of depth-resolved pixels assigned to this part by part_indices. */
  readonly visible_pixel_count: number
  /** visible_pixel_count / (frame_width * frame_height). */
  readonly coverage_ratio: number
  /** visible_pixel_count / all covered pixels in this fixed view. */
  readonly occlusion_share: number
}

export interface KnifePartVisibilityPartMetric {
  readonly part_id: string
  readonly triangle_count: number
  readonly material_zone_id: string
  readonly views: readonly KnifePartVisibilityViewMetric[]
  readonly visible_view_count: number
  /** Minimum over all eight fixed views, including zero for an absent view. */
  readonly min_coverage_ratio: number
  /** Maximum over all eight fixed views. */
  readonly max_coverage_ratio: number
  /** Arithmetic mean over all eight fixed views, including zero for an absent view. */
  readonly mean_coverage_ratio: number
  /** Presence in FRONT only. */
  readonly front_presence: boolean
  /** Presence in TOP only. */
  readonly top_presence: boolean
  /** Presence in either canonical LEFT or RIGHT view. */
  readonly side_presence: boolean
  /** Presence in FPS_HOLD only. */
  readonly fps_presence: boolean
  readonly status: KnifePartVisibilityMetricsStatus
}

export interface KnifePartVisibilityMetrics {
  readonly schema_version: typeof KNIFE_PART_VISIBILITY_METRICS_SCHEMA
  readonly source_fingerprint: string
  readonly rig_fingerprint: string
  readonly view_ids: readonly KnifeViewId[]
  readonly frame_width: number
  readonly frame_height: number
  readonly parts: readonly KnifePartVisibilityPartMetric[]
  readonly missing_part_ids: readonly string[]
  readonly underexposed_part_ids: readonly string[]
  readonly thresholds: typeof KNIFE_PART_VISIBILITY_THRESHOLDS
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
  readonly status: KnifePartVisibilityMetricsStatus
  /** Browser-safe deterministic fingerprint; it is not a Runtime/CAS hash. */
  readonly deterministic_fingerprint: string
}

export interface KnifePartVisibilityMetricsInput {
  readonly compiled: CompiledKnifeScene
  readonly rig?: KnifeViewRig
}

export class KnifePartVisibilityMetricsError extends Error {
  constructor(message: string) {
    super(`KNIFE_PART_VISIBILITY_METRICS_INVALID: ${message}`)
    this.name = 'KnifePartVisibilityMetricsError'
  }
}

/**
 * Measure all compiled parts against one canonical fixed rig. The optional
 * object-envelope overload is useful for callers that pass a typed job; both
 * forms reject unknown envelope keys and invalid scene/rig data.
 */
export function measureKnifePartVisibilityMetrics(
  compiled: CompiledKnifeScene,
  rig?: KnifeViewRig,
): KnifePartVisibilityMetrics
export function measureKnifePartVisibilityMetrics(
  input: KnifePartVisibilityMetricsInput,
): KnifePartVisibilityMetrics
export function measureKnifePartVisibilityMetrics(
  first: CompiledKnifeScene | KnifePartVisibilityMetricsInput,
  second?: KnifeViewRig,
): KnifePartVisibilityMetrics {
  if (arguments.length > 2) {
    throw new KnifePartVisibilityMetricsError('only compiled scene and optional fixed rig are accepted')
  }

  const { compiled, rig } = resolveInput(first, second)
  validateCompiledScene(compiled)
  const effectiveRig = rig ?? createKnifeViewRig()
  let evaluation: KnifeEightViewEvaluation
  try {
    evaluation = evaluateKnifeRig(compiled, effectiveRig)
  } catch (error) {
    const reason = error instanceof Error ? error.message : 'fixed-view evaluation failed'
    throw new KnifePartVisibilityMetricsError(reason)
  }
  validateEvaluationBinding(compiled, effectiveRig, evaluation)

  const partMetrics = buildPartMetrics(compiled, evaluation)
  const missingPartIds = partMetrics
    .filter((part) => part.visible_view_count === KNIFE_PART_VISIBILITY_THRESHOLDS.missing_visible_view_count)
    .map((part) => part.part_id)
  const underexposedPartIds = partMetrics
    .filter((part) => isUnderexposed(part))
    .map((part) => part.part_id)

  const frozenParts = Object.freeze(partMetrics)
  const frozenMissing = Object.freeze(missingPartIds)
  const frozenUnderexposed = Object.freeze(underexposedPartIds)
  const fingerprint = fingerprintMetrics(
    compiled.deterministic_fingerprint,
    effectiveRig.deterministic_fingerprint,
    effectiveRig.frame_width,
    effectiveRig.frame_height,
    frozenParts,
    frozenMissing,
    frozenUnderexposed,
  )

  return Object.freeze({
    schema_version: KNIFE_PART_VISIBILITY_METRICS_SCHEMA,
    source_fingerprint: compiled.deterministic_fingerprint,
    rig_fingerprint: effectiveRig.deterministic_fingerprint,
    view_ids: Object.freeze([...KNIFE_VIEW_IDS]),
    frame_width: effectiveRig.frame_width,
    frame_height: effectiveRig.frame_height,
    parts: frozenParts,
    missing_part_ids: frozenMissing,
    underexposed_part_ids: frozenUnderexposed,
    thresholds: KNIFE_PART_VISIBILITY_THRESHOLDS,
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
    status: KNIFE_PART_VISIBILITY_METRICS_STATUS,
    deterministic_fingerprint: fingerprint,
  })
}

/** Explicit evaluation-named alias for callers that use the existing rig API vocabulary. */
export const evaluateKnifePartVisibilityMetrics = measureKnifePartVisibilityMetrics

/** Short alias retained for metric consumers that use a measure verb. */
export const measureKnifePartVisibility = measureKnifePartVisibilityMetrics

function resolveInput(
  first: CompiledKnifeScene | KnifePartVisibilityMetricsInput,
  second: KnifeViewRig | undefined,
): { readonly compiled: CompiledKnifeScene; readonly rig: KnifeViewRig | undefined } {
  if (isRecord(first) && Object.prototype.hasOwnProperty.call(first, 'compiled')) {
    if (second !== undefined) {
      throw new KnifePartVisibilityMetricsError('object input cannot be combined with a positional rig')
    }
    assertExactKeys(first as unknown as Record<string, unknown>, ['compiled', 'rig'], 'input')
    const input = first as unknown as Record<string, unknown>
    if (!isRecord(input.compiled)) throw new KnifePartVisibilityMetricsError('input.compiled must be an object')
    if (input.rig !== undefined && !isRecord(input.rig)) {
      throw new KnifePartVisibilityMetricsError('input.rig must be an object when present')
    }
    return {
      compiled: input.compiled as CompiledKnifeScene,
      rig: input.rig as KnifeViewRig | undefined,
    }
  }
  if (second !== undefined && !isRecord(second)) {
    throw new KnifePartVisibilityMetricsError('rig must be an object')
  }
  return { compiled: first as CompiledKnifeScene, rig: second }
}

function validateCompiledScene(compiled: CompiledKnifeScene): void {
  if (!isRecord(compiled)) throw new KnifePartVisibilityMetricsError('compiled scene must be an object')
  assertExactKeys(compiled as unknown as Record<string, unknown>, [
    'group',
    'parts',
    'assembly_parts',
    'assembly_status',
    'sections',
    'triangle_count',
    'longitudinal_segments',
    'deterministic_fingerprint',
    'renderer_invoked',
    'quality_status',
  ], 'compiled scene')
  if (!Array.isArray(compiled.parts) || compiled.parts.length === 0 || compiled.parts.length > 256) {
    throw new KnifePartVisibilityMetricsError('compiled.parts must contain 1 to 256 parts')
  }
  if (!isStableFingerprint(compiled.deterministic_fingerprint)) {
    throw new KnifePartVisibilityMetricsError('compiled.deterministic_fingerprint is invalid')
  }
  if (compiled.renderer_invoked !== false || compiled.quality_status !== 'NOT_RUN') {
    throw new KnifePartVisibilityMetricsError('compiled scene crossed the renderer or quality boundary')
  }
  if (!isRecord(compiled.group) || typeof compiled.group.updateMatrixWorld !== 'function') {
    throw new KnifePartVisibilityMetricsError('compiled.group is not a valid scene group')
  }
  if (!Number.isInteger(compiled.triangle_count) || compiled.triangle_count <= 0 || compiled.triangle_count > 1_000_000) {
    throw new KnifePartVisibilityMetricsError('compiled.triangle_count must be a bounded positive integer')
  }

  const partIds = new Set<string>()
  let triangleTotal = 0
  for (const part of compiled.parts) {
    if (!isRecord(part)) throw new KnifePartVisibilityMetricsError('compiled.parts contains a non-object')
    if (!isStablePartId(part.part_id) || partIds.has(part.part_id)) {
      throw new KnifePartVisibilityMetricsError('compiled.parts must use unique stable part IDs')
    }
    partIds.add(part.part_id)
    if (!isStablePartId(part.material_zone_id)) {
      throw new KnifePartVisibilityMetricsError(`part ${String(part.part_id)} has an invalid material zone ID`)
    }
    if (!isRecord(part.geometry) || typeof part.geometry.getAttribute !== 'function' || typeof part.geometry.getIndex !== 'function') {
      throw new KnifePartVisibilityMetricsError(`part ${part.part_id} has invalid geometry`)
    }
    if (!isRecord(part.mesh) || typeof part.mesh.updateWorldMatrix !== 'function') {
      throw new KnifePartVisibilityMetricsError(`part ${part.part_id} has invalid mesh binding`)
    }
    const position = part.geometry.getAttribute('position')
    if (!isRecord(position)
      || position.itemSize !== 3
      || !Number.isInteger(position.count)
      || position.count < 3
      || typeof position.getX !== 'function'
      || typeof position.getY !== 'function'
      || typeof position.getZ !== 'function') {
      throw new KnifePartVisibilityMetricsError(`part ${part.part_id} has invalid position attribute`)
    }
    for (let vertexIndex = 0; vertexIndex < position.count; vertexIndex += 1) {
      const x = position.getX(vertexIndex)
      const y = position.getY(vertexIndex)
      const z = position.getZ(vertexIndex)
      if (![x, y, z].every((value) => Number.isFinite(value) && Math.abs(value) <= 1_000_000)) {
        throw new KnifePartVisibilityMetricsError(`part ${part.part_id} contains non-finite or unbounded vertex data`)
      }
    }
    const index = part.geometry.getIndex()
    const indexCount = index ? index.count : position.count
    if (!Number.isInteger(indexCount) || indexCount < 3 || indexCount % 3 !== 0) {
      throw new KnifePartVisibilityMetricsError(`part ${part.part_id} index count must be a positive multiple of three`)
    }
    if (index && typeof index.getX !== 'function') {
      throw new KnifePartVisibilityMetricsError(`part ${part.part_id} index attribute is invalid`)
    }
    for (let offset = 0; offset < indexCount; offset += 1) {
      const vertexIndex = index ? index.getX(offset) : offset
      if (!Number.isInteger(vertexIndex) || vertexIndex < 0 || vertexIndex >= position.count) {
        throw new KnifePartVisibilityMetricsError(`part ${part.part_id} contains an out-of-range index`)
      }
    }
    triangleTotal += indexCount / 3
  }
  if (triangleTotal !== compiled.triangle_count) {
    throw new KnifePartVisibilityMetricsError('compiled.triangle_count does not match part geometry')
  }
}

function validateEvaluationBinding(
  compiled: CompiledKnifeScene,
  rig: KnifeViewRig,
  evaluation: KnifeEightViewEvaluation,
): void {
  if (evaluation.rig !== rig
    || evaluation.receipt.rig_fingerprint !== rig.deterministic_fingerprint
    || evaluation.receipt.source_fingerprint !== compiled.deterministic_fingerprint
    || evaluation.receipt.renderer_invoked !== false
    || evaluation.receipt.quality_status !== 'NOT_RUN'
    || evaluation.views.length !== KNIFE_VIEW_IDS.length
    || evaluation.receipt.view_ids.join('|') !== KNIFE_VIEW_IDS.join('|')) {
    throw new KnifePartVisibilityMetricsError('fixed-view evaluation is not bound to the supplied scene and rig')
  }

  assertExactKeys(rig as unknown as Record<string, unknown>, [
    'schema_version',
    'rig_id',
    'coordinate_convention',
    'frame_width',
    'frame_height',
    'margin',
    'views',
    'calibration',
    'calibration_receipt',
    'deterministic_fingerprint',
  ], 'fixed rig')

  const expectedPixelCount = rig.frame_width * rig.frame_height
  for (let viewIndex = 0; viewIndex < evaluation.views.length; viewIndex += 1) {
    const view = evaluation.views[viewIndex]
    if (view.view_id !== KNIFE_VIEW_IDS[viewIndex]
      || view.mask.width !== rig.frame_width
      || view.mask.height !== rig.frame_height
      || view.mask.pixels.length !== expectedPixelCount
      || view.mask.part_indices.length !== expectedPixelCount
      || view.mask.material_indices.length !== expectedPixelCount
      || view.mask.depth.length !== expectedPixelCount
      || view.receipt.renderer_invoked !== false
      || view.receipt.quality_status !== 'NOT_RUN') {
      throw new KnifePartVisibilityMetricsError(`view ${KNIFE_VIEW_IDS[viewIndex]} mask binding is invalid`)
    }

    let coveredPixelCount = 0
    for (let pixelIndex = 0; pixelIndex < expectedPixelCount; pixelIndex += 1) {
      const pixel = view.mask.pixels[pixelIndex]
      if (pixel !== 0 && pixel !== 255) {
        throw new KnifePartVisibilityMetricsError(`view ${view.view_id} contains a non-binary mask pixel`)
      }
      const covered = pixel !== 0
      const partIndex = view.mask.part_indices[pixelIndex]
      if (covered) {
        coveredPixelCount += 1
        if (partIndex >= compiled.parts.length) {
          throw new KnifePartVisibilityMetricsError(`view ${view.view_id} contains an invalid part index`)
        }
        if (!Number.isFinite(view.mask.depth[pixelIndex])) {
          throw new KnifePartVisibilityMetricsError(`view ${view.view_id} contains a non-finite covered depth`)
        }
      } else if (partIndex !== 0xffff) {
        throw new KnifePartVisibilityMetricsError(`view ${view.view_id} has an uncovered pixel with a part index`)
      }
    }
    if (!Number.isInteger(view.mask.receipt.covered_pixel_count)
      || view.mask.receipt.covered_pixel_count < 0
      || view.mask.receipt.covered_pixel_count > expectedPixelCount
      || coveredPixelCount !== view.mask.receipt.covered_pixel_count
      || Math.abs(view.mask.receipt.coverage_ratio - coveredPixelCount / expectedPixelCount) > 1e-12
      || view.mask.receipt.rasterizer !== 'software-triangle-mask@2'
      || view.mask.receipt.anti_aliasing !== 'none') {
      throw new KnifePartVisibilityMetricsError(`view ${view.view_id} mask receipt is inconsistent`)
    }
  }
}

function buildPartMetrics(
  compiled: CompiledKnifeScene,
  evaluation: KnifeEightViewEvaluation,
): KnifePartVisibilityPartMetric[] {
  const metrics = compiled.parts.map((part, partIndex) => {
    const triangleCount = triangleCountFor(part)
    const views = evaluation.views.map((evaluationView) => {
      let visiblePixelCount = 0
      for (let pixelIndex = 0; pixelIndex < evaluationView.mask.part_indices.length; pixelIndex += 1) {
        if (evaluationView.mask.pixels[pixelIndex] !== 0 && evaluationView.mask.part_indices[pixelIndex] === partIndex) {
          visiblePixelCount += 1
        }
      }
      const framePixelCount = evaluationView.mask.width * evaluationView.mask.height
      const coveredPixelCount = evaluationView.mask.receipt.covered_pixel_count
      return Object.freeze({
        view_id: evaluationView.view_id,
        visible_pixel_count: visiblePixelCount,
        coverage_ratio: visiblePixelCount / framePixelCount,
        occlusion_share: coveredPixelCount === 0 ? 0 : visiblePixelCount / coveredPixelCount,
      })
    })
    const coverageRatios = views.map((view) => view.coverage_ratio)
    const visibleViewCount = views.filter((view) => view.visible_pixel_count > 0).length
    return Object.freeze({
      part_id: part.part_id,
      triangle_count: triangleCount,
      material_zone_id: part.material_zone_id,
      views: Object.freeze(views),
      visible_view_count: visibleViewCount,
      min_coverage_ratio: Math.min(...coverageRatios),
      max_coverage_ratio: Math.max(...coverageRatios),
      mean_coverage_ratio: coverageRatios.reduce((sum, value) => sum + value, 0) / coverageRatios.length,
      front_presence: hasVisibleView(views, 'FRONT'),
      top_presence: hasVisibleView(views, 'TOP'),
      side_presence: hasVisibleView(views, 'LEFT') || hasVisibleView(views, 'RIGHT'),
      fps_presence: hasVisibleView(views, 'FPS_HOLD'),
      status: KNIFE_PART_VISIBILITY_METRICS_STATUS,
    }) as KnifePartVisibilityPartMetric
  })

  // Keep the compiler's stable part order: mask part_indices are defined in
  // that order, and the source fingerprint binds the same ordered scene.
  return metrics
}

function triangleCountFor(part: CompiledKnifeScene['parts'][number]): number {
  const position = part.geometry.getAttribute('position')
  const index = part.geometry.getIndex()
  return (index ? index.count : position.count) / 3
}

function hasVisibleView(views: readonly KnifePartVisibilityViewMetric[], viewId: KnifeViewId): boolean {
  return views.some((view) => view.view_id === viewId && view.visible_pixel_count > 0)
}

function isUnderexposed(part: KnifePartVisibilityPartMetric): boolean {
  const maxVisiblePixelCount = Math.max(...part.views.map((view) => view.visible_pixel_count))
  return part.visible_view_count > KNIFE_PART_VISIBILITY_THRESHOLDS.missing_visible_view_count
    && (part.visible_view_count < KNIFE_PART_VISIBILITY_THRESHOLDS.underexposed_min_visible_view_count
      || maxVisiblePixelCount < KNIFE_PART_VISIBILITY_THRESHOLDS.underexposed_min_visible_pixel_count_in_any_view)
}

function fingerprintMetrics(
  sourceFingerprint: string,
  rigFingerprint: string,
  frameWidth: number,
  frameHeight: number,
  parts: readonly KnifePartVisibilityPartMetric[],
  missingPartIds: readonly string[],
  underexposedPartIds: readonly string[],
): string {
  const values = [
    KNIFE_PART_VISIBILITY_METRICS_SCHEMA,
    sourceFingerprint,
    rigFingerprint,
    `${frameWidth}x${frameHeight}`,
    `${KNIFE_PART_VISIBILITY_THRESHOLDS.missing_visible_view_count}`,
    `${KNIFE_PART_VISIBILITY_THRESHOLDS.underexposed_min_visible_view_count}`,
    `${KNIFE_PART_VISIBILITY_THRESHOLDS.underexposed_min_visible_pixel_count_in_any_view}`,
  ]
  for (const part of parts) {
    values.push(
      part.part_id,
      `${part.triangle_count}`,
      part.material_zone_id,
      `${part.visible_view_count}`,
      canonicalNumber(part.min_coverage_ratio),
      canonicalNumber(part.max_coverage_ratio),
      canonicalNumber(part.mean_coverage_ratio),
      part.front_presence ? '1' : '0',
      part.top_presence ? '1' : '0',
      part.side_presence ? '1' : '0',
      part.fps_presence ? '1' : '0',
      part.status,
    )
    for (const view of part.views) {
      values.push(view.view_id, `${view.visible_pixel_count}`, canonicalNumber(view.coverage_ratio), canonicalNumber(view.occlusion_share))
    }
  }
  values.push('missing', ...missingPartIds, 'underexposed', ...underexposedPartIds)
  return fnv1a64(values.join('|'))
}

function assertExactKeys(value: Record<string, unknown>, allowed: readonly string[], context: string): void {
  const allowedSet = new Set(allowed)
  for (const key of Object.keys(value)) {
    if (!allowedSet.has(key)) throw new KnifePartVisibilityMetricsError(`${context} contains unsupported field ${key}`)
  }
}

function isRecord(value: unknown): value is Record<string, any> {
  return typeof value === 'object' && value !== null
}

function isStablePartId(value: unknown): value is string {
  return typeof value === 'string' && /^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/.test(value)
}

function isStableFingerprint(value: unknown): value is string {
  return typeof value === 'string' && /^[a-f0-9]{16,128}$/i.test(value)
}

function canonicalNumber(value: number): string {
  if (!Number.isFinite(value)) return 'INVALID'
  return Object.is(value, -0) ? '0' : value.toPrecision(12)
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
