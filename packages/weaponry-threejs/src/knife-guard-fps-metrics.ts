import * as THREE from 'three'

import {
  createKnifeViewCamera,
  createKnifeViewRig,
  evaluateKnifeRig,
  KNIFE_VIEW_IDS,
  type KnifeEightViewEvaluation,
  type KnifeMaskResult,
  type KnifeViewId,
  type KnifeViewRig,
} from './knife-view-evaluation.ts'
import {
  measureKnifePartVisibilityMetrics,
} from './knife-part-visibility-metrics.ts'
import type { CompiledKnifeScene } from './knife-scene-compiler.ts'

/** Closed schema for structural guard negative-space and FPS occupancy measurements. */
export const KNIFE_GUARD_FPS_METRICS_SCHEMA = 'KnifeGuardFpsMetrics@1' as const
export const KNIFE_GUARD_FPS_METRICS_STATUS = 'MEASURED_NOT_REVIEWED' as const
export const KNIFE_GUARD_NEGATIVE_SPACE_BASIS = 'guard-convex-hull-background-proxy@2' as const
export const KNIFE_GUARD_NEGATIVE_SPACE_INTERPRETATION = 'visible-opening-proxy-only' as const
export const KNIFE_FPS_HOLD_BBOX_BASIS = 'depth-resolved-mask-full-asset-bbox@1' as const
export const KNIFE_FPS_HOLD_TIP_BASIS = 'compiled-tip-section-center-projection@1' as const
export const KNIFE_FPS_HOLD_GUARD_OCCLUSION_BASIS = 'projected-guard-depth-vs-depth-resolved-part-id@1' as const

export type KnifeGuardFpsMetricValue = number | 'NOT_COMPUTABLE'
export type KnifeGuardFpsComputability = 'COMPUTED' | 'PARTIAL' | 'NOT_COMPUTABLE'
export type KnifeGuardFpsMetricsStatus = typeof KNIFE_GUARD_FPS_METRICS_STATUS

export interface KnifeGuardFpsPixelBounds {
  readonly min_x: number
  readonly min_y: number
  readonly max_x: number
  readonly max_y: number
  readonly width_px: number
  readonly height_px: number
  readonly pixel_count: number
}

export interface KnifeGuardFpsPoint {
  readonly x_px: number
  readonly y_px: number
}

export interface KnifeGuardNegativeSpaceViewMetric {
  readonly view_id: KnifeViewId
  /** Visible guard pixels used to derive the local bbox. */
  readonly guard_visible_pixel_count: KnifeGuardFpsMetricValue
  readonly guard_bbox: KnifeGuardFpsPixelBounds | 'NOT_COMPUTABLE'
  /** Rasterized convex-hull area; denominator for guard_negative_space_ratio. */
  readonly guard_envelope_pixel_count: KnifeGuardFpsMetricValue
  /** Number of 4-connected pure-background components inside the guard hull. */
  readonly background_component_count: KnifeGuardFpsMetricValue
  /** Pixels that are empty and have no Part-ID; other solid Parts are excluded. */
  readonly background_pixel_count: KnifeGuardFpsMetricValue
  readonly largest_background_component_pixel_count: KnifeGuardFpsMetricValue
  /** background_pixel_count / guard_envelope_pixel_count. This is not a true 3-D void. */
  readonly guard_negative_space_ratio: KnifeGuardFpsMetricValue
  readonly computability: KnifeGuardFpsComputability
}

export interface KnifeGuardNegativeSpaceMetrics {
  readonly basis: typeof KNIFE_GUARD_NEGATIVE_SPACE_BASIS
  readonly interpretation: typeof KNIFE_GUARD_NEGATIVE_SPACE_INTERPRETATION
  readonly is_visible_opening_proxy: true
  readonly views: readonly KnifeGuardNegativeSpaceViewMetric[]
}

export interface KnifeFpsHoldMetrics {
  readonly view_id: 'FPS_HOLD'
  readonly bbox_basis: typeof KNIFE_FPS_HOLD_BBOX_BASIS
  readonly tip_basis: typeof KNIFE_FPS_HOLD_TIP_BASIS
  readonly guard_occlusion_basis: typeof KNIFE_FPS_HOLD_GUARD_OCCLUSION_BASIS
  readonly asset_bbox: KnifeGuardFpsPixelBounds | 'NOT_COMPUTABLE'
  readonly asset_bbox_width_fraction: KnifeGuardFpsMetricValue
  readonly asset_bbox_height_fraction: KnifeGuardFpsMetricValue
  readonly tip_point: KnifeGuardFpsPoint | 'NOT_COMPUTABLE'
  /** Minimum distance from the projected tip point to any viewport edge. */
  readonly tip_safe_margin_px: KnifeGuardFpsMetricValue
  /** tip_safe_margin_px / min(frame_width, frame_height). */
  readonly tip_safe_margin_fraction: KnifeGuardFpsMetricValue
  readonly guard_projected_pixel_count: KnifeGuardFpsMetricValue
  readonly guard_visible_pixel_count: KnifeGuardFpsMetricValue
  readonly guard_occluded_by_other_part_pixel_count: KnifeGuardFpsMetricValue
  readonly guard_occlusion_ratio: KnifeGuardFpsMetricValue
  readonly computability: KnifeGuardFpsComputability
}

export interface KnifeGuardFpsMetrics {
  readonly schema_version: typeof KNIFE_GUARD_FPS_METRICS_SCHEMA
  readonly source_fingerprint: string
  readonly rig_fingerprint: string
  readonly view_ids: readonly KnifeViewId[]
  readonly frame_width: number
  readonly frame_height: number
  readonly guard_part_id: string | 'NOT_COMPUTABLE'
  readonly guard_negative_space: KnifeGuardNegativeSpaceMetrics
  readonly fps_hold: KnifeFpsHoldMetrics
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
  readonly status: KnifeGuardFpsMetricsStatus
  /** Browser-safe deterministic fingerprint; it is not a Runtime/CAS hash. */
  readonly deterministic_fingerprint: string
}

export interface KnifeGuardFpsMetricsInput {
  readonly compiled: CompiledKnifeScene
  readonly rig?: KnifeViewRig
}

export class KnifeGuardFpsMetricsError extends Error {
  constructor(message: string) {
    super(`KNIFE_GUARD_FPS_METRICS_INVALID: ${message}`)
    this.name = 'KnifeGuardFpsMetricsError'
  }
}

export function measureKnifeGuardFpsMetrics(
  compiled: CompiledKnifeScene,
  rig?: KnifeViewRig,
): KnifeGuardFpsMetrics
export function measureKnifeGuardFpsMetrics(
  input: KnifeGuardFpsMetricsInput,
): KnifeGuardFpsMetrics
export function measureKnifeGuardFpsMetrics(
  first: CompiledKnifeScene | KnifeGuardFpsMetricsInput,
  second?: KnifeViewRig,
): KnifeGuardFpsMetrics {
  if (arguments.length > 2) {
    throw new KnifeGuardFpsMetricsError('only compiled scene and optional fixed rig are accepted')
  }
  const { compiled, rig } = resolveInput(first, second)
  const effectiveRig = rig ?? createKnifeViewRig()

  // This existing evaluator performs the closed scene/rig validation and binds
  // all masks to the same source and fixed-rig fingerprints before this module
  // performs its smaller derived calculations.
  try {
    measureKnifePartVisibilityMetrics(compiled, effectiveRig)
  } catch (error) {
    const reason = error instanceof Error ? error.message : 'part visibility validation failed'
    throw new KnifeGuardFpsMetricsError(reason)
  }

  let evaluation: KnifeEightViewEvaluation
  try {
    evaluation = evaluateKnifeRig(compiled, effectiveRig)
  } catch (error) {
    const reason = error instanceof Error ? error.message : 'fixed-view evaluation failed'
    throw new KnifeGuardFpsMetricsError(reason)
  }
  validateEvaluationBinding(compiled, effectiveRig, evaluation)

  const guardIndices = compiled.parts
    .map((part, index) => (part.surface_role === 'guard' || part.assembly_primitive === 'guard' ? index : -1))
    .filter((index) => index >= 0)
  const guardIndex = guardIndices.length === 1 ? guardIndices[0] : undefined
  const guardPartId = guardIndex === undefined ? 'NOT_COMPUTABLE' : compiled.parts[guardIndex].part_id

  const negativeSpaceViews = evaluation.views.map((view) => measureNegativeSpaceView(view.view_id, view.mask, guardIndex))
  const fpsView = evaluation.views.find((view) => view.view_id === 'FPS_HOLD')
  if (!fpsView) throw new KnifeGuardFpsMetricsError('fixed rig is missing FPS_HOLD')
  const fpsHold = measureFpsHold(compiled, effectiveRig, fpsView, guardIndex)

  const negativeSpace = Object.freeze({
    basis: KNIFE_GUARD_NEGATIVE_SPACE_BASIS,
    interpretation: KNIFE_GUARD_NEGATIVE_SPACE_INTERPRETATION,
    is_visible_opening_proxy: true as const,
    views: Object.freeze(negativeSpaceViews),
  })
  const fingerprint = fingerprintMetrics(
    compiled.deterministic_fingerprint,
    effectiveRig.deterministic_fingerprint,
    effectiveRig.frame_width,
    effectiveRig.frame_height,
    guardPartId,
    negativeSpace,
    fpsHold,
  )

  return Object.freeze({
    schema_version: KNIFE_GUARD_FPS_METRICS_SCHEMA,
    source_fingerprint: compiled.deterministic_fingerprint,
    rig_fingerprint: effectiveRig.deterministic_fingerprint,
    view_ids: Object.freeze([...KNIFE_VIEW_IDS]),
    frame_width: effectiveRig.frame_width,
    frame_height: effectiveRig.frame_height,
    guard_part_id: guardPartId,
    guard_negative_space: negativeSpace,
    fps_hold: fpsHold,
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
    status: KNIFE_GUARD_FPS_METRICS_STATUS,
    deterministic_fingerprint: fingerprint,
  })
}

export const evaluateKnifeGuardFpsMetrics = measureKnifeGuardFpsMetrics
export const measureKnifeGuardMetrics = measureKnifeGuardFpsMetrics

function resolveInput(
  first: CompiledKnifeScene | KnifeGuardFpsMetricsInput,
  second: KnifeViewRig | undefined,
): { readonly compiled: CompiledKnifeScene; readonly rig: KnifeViewRig | undefined } {
  if (isRecord(first) && Object.prototype.hasOwnProperty.call(first, 'compiled')) {
    if (second !== undefined) throw new KnifeGuardFpsMetricsError('object input cannot be combined with a positional rig')
    const input = first as unknown as Record<string, unknown>
    assertExactKeys(input, ['compiled', 'rig'], 'input')
    if (!isRecord(input.compiled)) throw new KnifeGuardFpsMetricsError('input.compiled must be an object')
    if (input.rig !== undefined && !isRecord(input.rig)) throw new KnifeGuardFpsMetricsError('input.rig must be an object when present')
    return {
      compiled: input.compiled as CompiledKnifeScene,
      rig: input.rig as KnifeViewRig | undefined,
    }
  }
  if (second !== undefined && !isRecord(second)) throw new KnifeGuardFpsMetricsError('rig must be an object')
  return { compiled: first as CompiledKnifeScene, rig: second }
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
    throw new KnifeGuardFpsMetricsError('fixed-view evaluation is not bound to supplied source and rig')
  }
  for (let index = 0; index < evaluation.views.length; index += 1) {
    const view = evaluation.views[index]
    const expectedPixels = rig.frame_width * rig.frame_height
    if (view.view_id !== KNIFE_VIEW_IDS[index]
      || view.receipt.rig_fingerprint !== rig.deterministic_fingerprint
      || view.receipt.source_fingerprint !== compiled.deterministic_fingerprint
      || view.receipt.renderer_invoked !== false
      || view.receipt.quality_status !== 'NOT_RUN'
      || view.mask.width !== rig.frame_width
      || view.mask.height !== rig.frame_height
      || view.mask.pixels.length !== expectedPixels
      || view.mask.part_indices.length !== expectedPixels
      || view.mask.depth.length !== expectedPixels) {
      throw new KnifeGuardFpsMetricsError(`view ${KNIFE_VIEW_IDS[index]} mask binding is invalid`)
    }
    for (let pixel = 0; pixel < expectedPixels; pixel += 1) {
      if (view.mask.pixels[pixel] !== 0 && view.mask.pixels[pixel] !== 255) {
        throw new KnifeGuardFpsMetricsError(`view ${view.view_id} contains a non-binary mask pixel`)
      }
      if (view.mask.pixels[pixel] === 0 && view.mask.part_indices[pixel] !== 0xffff) {
        throw new KnifeGuardFpsMetricsError(`view ${view.view_id} has an uncovered pixel with a Part-ID`)
      }
      if (view.mask.pixels[pixel] !== 0 && !Number.isFinite(view.mask.depth[pixel])) {
        throw new KnifeGuardFpsMetricsError(`view ${view.view_id} has a covered pixel with non-finite depth`)
      }
      if (view.mask.pixels[pixel] !== 0 && view.mask.part_indices[pixel] >= compiled.parts.length) {
        throw new KnifeGuardFpsMetricsError(`view ${view.view_id} has an out-of-range Part-ID`)
      }
    }
  }
}

function measureNegativeSpaceView(
  viewId: KnifeViewId,
  mask: KnifeMaskResult,
  guardIndex: number | undefined,
): KnifeGuardNegativeSpaceViewMetric {
  if (guardIndex === undefined) return Object.freeze(notComputableNegativeSpace(viewId))
  const bounds = visiblePartBounds(mask, guardIndex)
  if (!bounds) return Object.freeze(notComputableNegativeSpace(viewId))

  const background = new Uint8Array(bounds.pixel_count)
  const guardPixels = visiblePartPoints(mask, guardIndex)
  const hull = convexHull(guardPixels)
  if (hull.length < 3) return Object.freeze(notComputableNegativeSpace(viewId))
  let backgroundPixelCount = 0
  let envelopePixelCount = 0
  for (let localY = 0; localY < bounds.height_px; localY += 1) {
    for (let localX = 0; localX < bounds.width_px; localX += 1) {
      const x = bounds.min_x + localX
      const y = bounds.min_y + localY
      const sourceIndex = y * mask.width + x
      const localIndex = localY * bounds.width_px + localX
      if (!pointInsideConvexHull(x + 0.5, y + 0.5, hull)) continue
      envelopePixelCount += 1
      // Only pure background enclosed by the visible guard's convex envelope
      // contributes. Other solid Parts remain excluded from the opening proxy.
      if (mask.pixels[sourceIndex] === 0 && mask.part_indices[sourceIndex] === 0xffff) {
        background[localIndex] = 1
        backgroundPixelCount += 1
      }
    }
  }
  if (envelopePixelCount === 0) return Object.freeze(notComputableNegativeSpace(viewId))
  const components = connectedComponents(background, bounds.width_px, bounds.height_px)
  const largest = components.length === 0 ? 0 : Math.max(...components)
  return Object.freeze({
    view_id: viewId,
    guard_visible_pixel_count: countVisiblePartPixels(mask, guardIndex),
    guard_bbox: bounds,
    guard_envelope_pixel_count: envelopePixelCount,
    background_component_count: components.length,
    background_pixel_count: backgroundPixelCount,
    largest_background_component_pixel_count: largest,
    guard_negative_space_ratio: backgroundPixelCount / envelopePixelCount,
    computability: 'COMPUTED',
  })
}

function notComputableNegativeSpace(viewId: KnifeViewId): KnifeGuardNegativeSpaceViewMetric {
  return {
    view_id: viewId,
    guard_visible_pixel_count: 'NOT_COMPUTABLE',
    guard_bbox: 'NOT_COMPUTABLE',
    guard_envelope_pixel_count: 'NOT_COMPUTABLE',
    background_component_count: 'NOT_COMPUTABLE',
    background_pixel_count: 'NOT_COMPUTABLE',
    largest_background_component_pixel_count: 'NOT_COMPUTABLE',
    guard_negative_space_ratio: 'NOT_COMPUTABLE',
    computability: 'NOT_COMPUTABLE',
  }
}

function measureFpsHold(
  compiled: CompiledKnifeScene,
  rig: KnifeViewRig,
  view: KnifeEightViewEvaluation['views'][number],
  guardIndex: number | undefined,
): KnifeFpsHoldMetrics {
  const assetBounds = occupiedBounds(view.mask)
  const assetWidth = assetBounds?.width_px ?? 'NOT_COMPUTABLE'
  const assetHeight = assetBounds?.height_px ?? 'NOT_COMPUTABLE'
  const assetWidthFraction: KnifeGuardFpsMetricValue = typeof assetWidth === 'number' ? assetWidth / rig.frame_width : 'NOT_COMPUTABLE'
  const assetHeightFraction: KnifeGuardFpsMetricValue = typeof assetHeight === 'number' ? assetHeight / rig.frame_height : 'NOT_COMPUTABLE'

  let tipPoint: KnifeGuardFpsPoint | 'NOT_COMPUTABLE' = 'NOT_COMPUTABLE'
  let tipMarginPx: KnifeGuardFpsMetricValue = 'NOT_COMPUTABLE'
  let tipMarginFraction: KnifeGuardFpsMetricValue = 'NOT_COMPUTABLE'
  const tipSections = compiled.sections.filter((section) => section.role === 'tip')
  if (tipSections.length === 1) {
    const camera = createKnifeViewCamera(rig, 'FPS_HOLD')
    const worldTip = new THREE.Vector3(...tipSections[0].center).applyMatrix4(compiled.group.matrixWorld)
    const projected = worldTip.project(camera)
    const x = (projected.x * 0.5 + 0.5) * rig.frame_width
    const y = (-projected.y * 0.5 + 0.5) * rig.frame_height
    if ([x, y].every((value) => Number.isFinite(value))) {
      tipPoint = Object.freeze({ x_px: x, y_px: y })
      tipMarginPx = Math.min(x, rig.frame_width - x, y, rig.frame_height - y)
      tipMarginFraction = tipMarginPx / Math.min(rig.frame_width, rig.frame_height)
    }
  }

  const guardAttribution = guardIndex === undefined
    ? {
        projected: 'NOT_COMPUTABLE' as const,
        visible: 'NOT_COMPUTABLE' as const,
        occluded: 'NOT_COMPUTABLE' as const,
        ratio: 'NOT_COMPUTABLE' as const,
        complete: false,
      }
    : measureGuardAttribution(view, guardIndex)
  const computableFlags = [assetBounds !== undefined, typeof tipMarginPx === 'number', guardAttribution.complete]
  const computability: KnifeGuardFpsComputability = computableFlags.every(Boolean)
    ? 'COMPUTED'
    : computableFlags.some(Boolean)
      ? 'PARTIAL'
      : 'NOT_COMPUTABLE'

  return Object.freeze({
    view_id: 'FPS_HOLD',
    bbox_basis: KNIFE_FPS_HOLD_BBOX_BASIS,
    tip_basis: KNIFE_FPS_HOLD_TIP_BASIS,
    guard_occlusion_basis: KNIFE_FPS_HOLD_GUARD_OCCLUSION_BASIS,
    asset_bbox: assetBounds ?? 'NOT_COMPUTABLE',
    asset_bbox_width_fraction: assetWidthFraction,
    asset_bbox_height_fraction: assetHeightFraction,
    tip_point: tipPoint,
    tip_safe_margin_px: tipMarginPx,
    tip_safe_margin_fraction: tipMarginFraction,
    guard_projected_pixel_count: guardAttribution.projected,
    guard_visible_pixel_count: guardAttribution.visible,
    guard_occluded_by_other_part_pixel_count: guardAttribution.occluded,
    guard_occlusion_ratio: guardAttribution.ratio,
    computability,
  })
}

function measureGuardAttribution(
  view: KnifeEightViewEvaluation['views'][number],
  guardIndex: number,
): {
  readonly projected: KnifeGuardFpsMetricValue
  readonly visible: KnifeGuardFpsMetricValue
  readonly occluded: KnifeGuardFpsMetricValue
  readonly ratio: KnifeGuardFpsMetricValue
  readonly complete: boolean
} {
  const projectedDepth = rasterizePartDepth(view.projection, guardIndex)
  let projected = 0
  let visible = 0
  let occluded = 0
  let unresolved = 0
  for (let index = 0; index < projectedDepth.length; index += 1) {
    if (!Number.isFinite(projectedDepth[index])) continue
    projected += 1
    if (view.mask.pixels[index] === 0) {
      unresolved += 1
    } else if (view.mask.part_indices[index] === guardIndex) {
      visible += 1
    } else {
      occluded += 1
    }
  }
  const complete = projected > 0 && unresolved === 0 && projected === visible + occluded
  if (!complete) {
    return { projected: projected > 0 ? projected : 'NOT_COMPUTABLE', visible: visible > 0 ? visible : 'NOT_COMPUTABLE', occluded: 'NOT_COMPUTABLE', ratio: 'NOT_COMPUTABLE', complete: false }
  }
  return {
    projected,
    visible,
    occluded,
    ratio: projected === 0 ? 'NOT_COMPUTABLE' : occluded / projected,
    complete: true,
  }
}

function rasterizePartDepth(
  projection: KnifeEightViewEvaluation['views'][number]['projection'],
  partIndex: number,
): Float32Array {
  const depth = new Float32Array(projection.rig.frame_width * projection.rig.frame_height)
  depth.fill(Number.POSITIVE_INFINITY)
  const width = projection.rig.frame_width
  const height = projection.rig.frame_height
  for (const triangle of projection.triangles) {
    if (triangle.part_index !== partIndex) continue
    const a = projection.vertices[triangle.a]
    const b = projection.vertices[triangle.b]
    const c = projection.vertices[triangle.c]
    const minX = Math.max(0, Math.floor(Math.min(a.x_px, b.x_px, c.x_px)))
    const maxX = Math.min(width - 1, Math.ceil(Math.max(a.x_px, b.x_px, c.x_px)))
    const minY = Math.max(0, Math.floor(Math.min(a.y_px, b.y_px, c.y_px)))
    const maxY = Math.min(height - 1, Math.ceil(Math.max(a.y_px, b.y_px, c.y_px)))
    if (minX > maxX || minY > maxY) continue
    const area = edgeFunction(a.x_px, a.y_px, b.x_px, b.y_px, c.x_px, c.y_px)
    if (Math.abs(area) <= Number.EPSILON) continue
    for (let y = minY; y <= maxY; y += 1) {
      for (let x = minX; x <= maxX; x += 1) {
        const sampleX = x + 0.5
        const sampleY = y + 0.5
        const w0 = edgeFunction(b.x_px, b.y_px, c.x_px, c.y_px, sampleX, sampleY)
        const w1 = edgeFunction(c.x_px, c.y_px, a.x_px, a.y_px, sampleX, sampleY)
        const w2 = edgeFunction(a.x_px, a.y_px, b.x_px, b.y_px, sampleX, sampleY)
        const inside = area > 0
          ? w0 >= 0 && w1 >= 0 && w2 >= 0
          : w0 <= 0 && w1 <= 0 && w2 <= 0
        if (!inside) continue
        const candidateDepth = (w0 * a.depth_ndc + w1 * b.depth_ndc + w2 * c.depth_ndc) / area
        const pixelIndex = y * width + x
        if (candidateDepth < depth[pixelIndex] - 1e-7) depth[pixelIndex] = candidateDepth
      }
    }
  }
  return depth
}

function visiblePartBounds(mask: KnifeMaskResult, partIndex: number): KnifeGuardFpsPixelBounds | undefined {
  let minX = mask.width
  let minY = mask.height
  let maxX = -1
  let maxY = -1
  for (let y = 0; y < mask.height; y += 1) {
    for (let x = 0; x < mask.width; x += 1) {
      const index = y * mask.width + x
      if (mask.pixels[index] !== 0 && mask.part_indices[index] === partIndex) {
        minX = Math.min(minX, x)
        maxX = Math.max(maxX, x)
        minY = Math.min(minY, y)
        maxY = Math.max(maxY, y)
      }
    }
  }
  return makeBounds(minX, minY, maxX, maxY)
}

function occupiedBounds(mask: KnifeMaskResult): KnifeGuardFpsPixelBounds | undefined {
  let minX = mask.width
  let minY = mask.height
  let maxX = -1
  let maxY = -1
  for (let y = 0; y < mask.height; y += 1) {
    for (let x = 0; x < mask.width; x += 1) {
      if (mask.pixels[y * mask.width + x] !== 0) {
        minX = Math.min(minX, x)
        maxX = Math.max(maxX, x)
        minY = Math.min(minY, y)
        maxY = Math.max(maxY, y)
      }
    }
  }
  return makeBounds(minX, minY, maxX, maxY)
}

function makeBounds(minX: number, minY: number, maxX: number, maxY: number): KnifeGuardFpsPixelBounds | undefined {
  if (maxX < minX || maxY < minY) return undefined
  const width = maxX - minX + 1
  const height = maxY - minY + 1
  return Object.freeze({
    min_x: minX,
    min_y: minY,
    max_x: maxX,
    max_y: maxY,
    width_px: width,
    height_px: height,
    pixel_count: width * height,
  })
}

function countVisiblePartPixels(mask: KnifeMaskResult, partIndex: number): number {
  let count = 0
  for (let index = 0; index < mask.pixels.length; index += 1) {
    if (mask.pixels[index] !== 0 && mask.part_indices[index] === partIndex) count += 1
  }
  return count
}

interface PixelPoint {
  readonly x: number
  readonly y: number
}

function visiblePartPoints(mask: KnifeMaskResult, partIndex: number): PixelPoint[] {
  const points: PixelPoint[] = []
  for (let y = 0; y < mask.height; y += 1) {
    for (let x = 0; x < mask.width; x += 1) {
      const index = y * mask.width + x
      if (mask.pixels[index] !== 0 && mask.part_indices[index] === partIndex) points.push({ x: x + 0.5, y: y + 0.5 })
    }
  }
  return points
}

function convexHull(points: readonly PixelPoint[]): PixelPoint[] {
  const sorted = [...points].sort((left, right) => left.x - right.x || left.y - right.y)
  const unique = sorted.filter((point, index) => index === 0 || point.x !== sorted[index - 1].x || point.y !== sorted[index - 1].y)
  if (unique.length <= 2) return unique
  const cross = (origin: PixelPoint, left: PixelPoint, right: PixelPoint): number =>
    (left.x - origin.x) * (right.y - origin.y) - (left.y - origin.y) * (right.x - origin.x)
  const lower: PixelPoint[] = []
  for (const point of unique) {
    while (lower.length >= 2 && cross(lower.at(-2)!, lower.at(-1)!, point) <= 0) lower.pop()
    lower.push(point)
  }
  const upper: PixelPoint[] = []
  for (let index = unique.length - 1; index >= 0; index -= 1) {
    const point = unique[index]
    while (upper.length >= 2 && cross(upper.at(-2)!, upper.at(-1)!, point) <= 0) upper.pop()
    upper.push(point)
  }
  lower.pop()
  upper.pop()
  return [...lower, ...upper]
}

function pointInsideConvexHull(x: number, y: number, hull: readonly PixelPoint[]): boolean {
  let sign = 0
  for (let index = 0; index < hull.length; index += 1) {
    const start = hull[index]
    const end = hull[(index + 1) % hull.length]
    const cross = (end.x - start.x) * (y - start.y) - (end.y - start.y) * (x - start.x)
    if (Math.abs(cross) <= 1e-9) continue
    const current = Math.sign(cross)
    if (sign === 0) sign = current
    else if (current !== sign) return false
  }
  return true
}

function connectedComponents(mask: Uint8Array, width: number, height: number): number[] {
  const components: number[] = []
  const visited = new Uint8Array(mask.length)
  const queue = new Int32Array(mask.length)
  for (let start = 0; start < mask.length; start += 1) {
    if (mask[start] === 0 || visited[start] !== 0) continue
    let head = 0
    let tail = 0
    queue[tail++] = start
    visited[start] = 1
    let size = 0
    while (head < tail) {
      const current = queue[head++]
      size += 1
      const x = current % width
      const y = Math.floor(current / width)
      const neighbors = [
        x > 0 ? current - 1 : -1,
        x + 1 < width ? current + 1 : -1,
        y > 0 ? current - width : -1,
        y + 1 < height ? current + width : -1,
      ]
      for (const next of neighbors) {
        if (next >= 0 && mask[next] !== 0 && visited[next] === 0) {
          visited[next] = 1
          queue[tail++] = next
        }
      }
    }
    components.push(size)
  }
  return components
}

function fingerprintMetrics(
  sourceFingerprint: string,
  rigFingerprint: string,
  frameWidth: number,
  frameHeight: number,
  guardPartId: string,
  negativeSpace: KnifeGuardNegativeSpaceMetrics,
  fpsHold: KnifeFpsHoldMetrics,
): string {
  const values: string[] = [
    KNIFE_GUARD_FPS_METRICS_SCHEMA,
    sourceFingerprint,
    rigFingerprint,
    `${frameWidth}x${frameHeight}`,
    guardPartId,
    negativeSpace.basis,
    negativeSpace.interpretation,
    negativeSpace.is_visible_opening_proxy ? '1' : '0',
  ]
  for (const view of negativeSpace.views) {
    values.push(view.view_id, `${view.guard_visible_pixel_count}`, boundsFingerprint(view.guard_bbox), `${view.guard_envelope_pixel_count}`, `${view.background_component_count}`, `${view.background_pixel_count}`, `${view.largest_background_component_pixel_count}`, `${view.guard_negative_space_ratio}`, view.computability)
  }
  values.push(
    fpsHold.view_id,
    boundsFingerprint(fpsHold.asset_bbox),
    `${fpsHold.asset_bbox_width_fraction}`,
    `${fpsHold.asset_bbox_height_fraction}`,
    pointFingerprint(fpsHold.tip_point),
    `${fpsHold.tip_safe_margin_px}`,
    `${fpsHold.tip_safe_margin_fraction}`,
    `${fpsHold.guard_projected_pixel_count}`,
    `${fpsHold.guard_visible_pixel_count}`,
    `${fpsHold.guard_occluded_by_other_part_pixel_count}`,
    `${fpsHold.guard_occlusion_ratio}`,
    fpsHold.computability,
  )
  return fnv1a64(values.join('|'))
}

function boundsFingerprint(bounds: KnifeGuardFpsPixelBounds | 'NOT_COMPUTABLE'): string {
  return bounds === 'NOT_COMPUTABLE'
    ? bounds
    : `${bounds.min_x},${bounds.min_y},${bounds.max_x},${bounds.max_y},${bounds.width_px},${bounds.height_px},${bounds.pixel_count}`
}

function pointFingerprint(point: KnifeGuardFpsPoint | 'NOT_COMPUTABLE'): string {
  return point === 'NOT_COMPUTABLE' ? point : `${canonicalNumber(point.x_px)},${canonicalNumber(point.y_px)}`
}

function assertExactKeys(value: Record<string, unknown>, allowed: readonly string[], context: string): void {
  const allowedSet = new Set(allowed)
  for (const key of Object.keys(value)) {
    if (!allowedSet.has(key)) throw new KnifeGuardFpsMetricsError(`${context} contains unsupported field ${key}`)
  }
}

function isRecord(value: unknown): value is Record<string, any> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function edgeFunction(ax: number, ay: number, bx: number, by: number, px: number, py: number): number {
  return (px - ax) * (by - ay) - (py - ay) * (bx - ax)
}

function canonicalNumber(value: number): string {
  if (!Number.isFinite(value)) return value === Number.POSITIVE_INFINITY ? 'INF' : 'NAN'
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
