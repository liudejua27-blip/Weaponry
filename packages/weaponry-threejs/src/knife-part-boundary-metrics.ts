import {
  createKnifeViewRig,
  evaluateKnifeRig,
  KNIFE_VIEW_IDS,
  type KnifeEightViewEvaluation,
  type KnifeViewId,
  type KnifeViewRig,
} from './knife-view-evaluation.ts'
import {
  measureKnifePartVisibilityMetrics,
} from './knife-part-visibility-metrics.ts'
import type { CompiledKnifePart, CompiledKnifeScene } from './knife-scene-compiler.ts'

/** Closed raster observations derived from the fixed CPU part-ID masks. */
export const KNIFE_PART_BOUNDARY_METRICS_SCHEMA = 'KnifePartBoundaryMetrics@1' as const
export const KNIFE_PART_BOUNDARY_METRICS_STATUS = 'MEASURED_NOT_REVIEWED' as const
export const KNIFE_PART_BOUNDARY_CONNECTIVITY = 'four-neighbor@1' as const
export const KNIFE_PART_BOUNDARY_NORMALIZATION = 'frame-diagonal-pixels@1' as const

export type KnifePartBoundaryMetricsStatus = typeof KNIFE_PART_BOUNDARY_METRICS_STATUS

/** Role-only relationship; it is not evidence that the meshes physically meet. */
export type KnifePartBoundaryRelation =
  | 'self'
  | 'blade-root-attachment'
  | 'blade-surface-adjacency'
  | 'blade-feature-attachment'
  | 'guard-grip-attachment'
  | 'guard-feature-attachment'
  | 'grip-pommel-attachment'
  | 'grip-feature-attachment'
  | 'pommel-feature-attachment'
  | 'semantic-part-pair'

export interface KnifePartBoundaryViewMetric {
  readonly view_id: KnifeViewId
  readonly visible_pixel_count: number
  /** Number of visible part pixels touching a different label/background/frame. */
  readonly boundary_pixel_count: number
  /** Four-neighbor boundary edges, including frame/background edges. */
  readonly boundary_edge_count: number
  /** boundary_edge_count / sqrt(frame_width² + frame_height²). */
  readonly boundary_length_normalized: number
  /** Four-neighbor connected components in this part's visible mask. */
  readonly connected_island_count: number
}

export interface KnifePartBoundaryPartMetric {
  readonly part_id: string
  readonly material_zone_id: string
  readonly views: readonly KnifePartBoundaryViewMetric[]
  readonly boundary_pixel_count: number
  readonly boundary_length_normalized: number
  readonly connected_island_count: number
  readonly status: KnifePartBoundaryMetricsStatus
}

export interface KnifePartBoundaryAdjacencyViewMetric {
  readonly view_id: KnifeViewId
  readonly contact_pixel_count: number
  readonly contact_edge_count: number
  /** Uncovered one-pixel cells aligned between the two semantic part masks. */
  readonly gap_pixel_count: number
  readonly gap_edge_count: number
  /** contact_edge_count / (contact_edge_count + gap_edge_count), or zero. */
  readonly contact_ratio: number
}

export interface KnifePartBoundaryAdjacencyCell {
  readonly part_id: string
  readonly neighbor_part_id: string
  readonly relation: KnifePartBoundaryRelation
  readonly views: readonly KnifePartBoundaryAdjacencyViewMetric[]
  readonly contact_pixel_count: number
  readonly contact_edge_count: number
  readonly gap_pixel_count: number
  readonly gap_edge_count: number
  readonly contact_ratio: number
}

export interface KnifePartBoundaryMetrics {
  readonly schema_version: typeof KNIFE_PART_BOUNDARY_METRICS_SCHEMA
  readonly source_fingerprint: string
  readonly rig_fingerprint: string
  readonly view_ids: readonly KnifeViewId[]
  readonly frame_width: number
  readonly frame_height: number
  readonly connectivity: typeof KNIFE_PART_BOUNDARY_CONNECTIVITY
  readonly normalization: typeof KNIFE_PART_BOUNDARY_NORMALIZATION
  readonly parts: readonly KnifePartBoundaryPartMetric[]
  /** Row/column order is the compiled.parts order; diagonal cells are zero. */
  readonly adjacency_matrix: readonly (readonly KnifePartBoundaryAdjacencyCell[])[]
  /** Non-self cells with a known semantic role relationship. */
  readonly semantic_adjacencies: readonly KnifePartBoundaryAdjacencyCell[]
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
  readonly status: KnifePartBoundaryMetricsStatus
  /** Browser-safe deterministic fingerprint, not a Runtime/CAS hash. */
  readonly deterministic_fingerprint: string
}

export interface KnifePartBoundaryMetricsInput {
  readonly compiled: CompiledKnifeScene
  readonly rig?: KnifeViewRig
}

export class KnifePartBoundaryMetricsError extends Error {
  constructor(message: string) {
    super(`KNIFE_PART_BOUNDARY_METRICS_INVALID: ${message}`)
    this.name = 'KnifePartBoundaryMetricsError'
  }
}

export function measureKnifePartBoundaryMetrics(
  compiled: CompiledKnifeScene,
  rig?: KnifeViewRig,
): KnifePartBoundaryMetrics
export function measureKnifePartBoundaryMetrics(
  input: KnifePartBoundaryMetricsInput,
): KnifePartBoundaryMetrics
export function measureKnifePartBoundaryMetrics(
  first: CompiledKnifeScene | KnifePartBoundaryMetricsInput,
  second?: KnifeViewRig,
): KnifePartBoundaryMetrics {
  if (arguments.length > 2) {
    throw new KnifePartBoundaryMetricsError('only compiled scene and optional fixed rig are accepted')
  }

  const { compiled, rig } = resolveInput(first, second)
  const effectiveRig = rig ?? createKnifeViewRig()
  let evaluation: KnifeEightViewEvaluation
  try {
    // The existing visibility receipt owns the complete scene/mask/rig
    // validator. Its result is intentionally discarded; this module consumes
    // the same evaluateKnifeRig z-buffer rather than duplicating that gate.
    measureKnifePartVisibilityMetrics(compiled, effectiveRig)
    evaluation = evaluateKnifeRig(compiled, effectiveRig)
  } catch (error) {
    const reason = error instanceof Error ? error.message : 'fixed-view evaluation failed'
    throw new KnifePartBoundaryMetricsError(reason)
  }
  validateEvaluationBinding(compiled, effectiveRig, evaluation)

  const measured = evaluation.views.map((view) => measureView(view, compiled.parts.length, effectiveRig.frame_width, effectiveRig.frame_height))
  const parts = compiled.parts.map((part, partIndex) => buildPartMetric(part, measured, partIndex))
  const matrix = buildAdjacencyMatrix(compiled.parts, measured)
  const semanticAdjacencies = Object.freeze(matrix.flat().filter((cell) => cell.part_id !== cell.neighbor_part_id && cell.relation !== 'semantic-part-pair'))
  const frozenParts = Object.freeze(parts)
  const frozenMatrix = Object.freeze(matrix)
  const fingerprint = fingerprintMetrics(compiled, effectiveRig, frozenParts, frozenMatrix)

  return Object.freeze({
    schema_version: KNIFE_PART_BOUNDARY_METRICS_SCHEMA,
    source_fingerprint: compiled.deterministic_fingerprint,
    rig_fingerprint: effectiveRig.deterministic_fingerprint,
    view_ids: Object.freeze([...KNIFE_VIEW_IDS]),
    frame_width: effectiveRig.frame_width,
    frame_height: effectiveRig.frame_height,
    connectivity: KNIFE_PART_BOUNDARY_CONNECTIVITY,
    normalization: KNIFE_PART_BOUNDARY_NORMALIZATION,
    parts: frozenParts,
    adjacency_matrix: frozenMatrix,
    semantic_adjacencies: semanticAdjacencies,
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
    status: KNIFE_PART_BOUNDARY_METRICS_STATUS,
    deterministic_fingerprint: fingerprint,
  })
}

export const evaluateKnifePartBoundaryMetrics = measureKnifePartBoundaryMetrics
export const measureKnifePartBoundary = measureKnifePartBoundaryMetrics

interface MeasuredView {
  readonly view_id: KnifeViewId
  readonly frame_diagonal: number
  readonly boundary_pixels: readonly ReadonlySet<number>[]
  readonly boundary_edges: readonly number[]
  readonly islands: readonly number[]
  readonly visible_pixels: readonly number[]
  readonly adjacency: readonly (readonly Readonly<PairMeasurement>[])[]
}

interface PairMeasurement {
  contact_pixels: Set<number>
  contact_edges: number
  gap_pixels: Set<number>
  gap_edges: number
}

function resolveInput(
  first: CompiledKnifeScene | KnifePartBoundaryMetricsInput,
  second: KnifeViewRig | undefined,
): { readonly compiled: CompiledKnifeScene; readonly rig: KnifeViewRig | undefined } {
  const firstRecord = first as unknown as Record<string, any>
  if (isRecord(first) && Object.prototype.hasOwnProperty.call(firstRecord, 'compiled')) {
    if (second !== undefined) throw new KnifePartBoundaryMetricsError('object input cannot be combined with a positional rig')
    assertExactKeys(firstRecord, ['compiled', 'rig'], 'input')
    if (!isRecord(firstRecord.compiled)) throw new KnifePartBoundaryMetricsError('input.compiled must be an object')
    if (firstRecord.rig !== undefined && !isRecord(firstRecord.rig)) throw new KnifePartBoundaryMetricsError('input.rig must be an object when present')
    return { compiled: firstRecord.compiled as CompiledKnifeScene, rig: firstRecord.rig as KnifeViewRig | undefined }
  }
  if (second !== undefined && !isRecord(second)) throw new KnifePartBoundaryMetricsError('rig must be an object')
  return { compiled: first as CompiledKnifeScene, rig: second }
}

function validateEvaluationBinding(compiled: CompiledKnifeScene, rig: KnifeViewRig, evaluation: KnifeEightViewEvaluation): void {
  if (evaluation.rig !== rig
    || evaluation.receipt.rig_fingerprint !== rig.deterministic_fingerprint
    || evaluation.receipt.source_fingerprint !== compiled.deterministic_fingerprint
    || evaluation.receipt.renderer_invoked !== false
    || evaluation.receipt.quality_status !== 'NOT_RUN'
    || evaluation.views.length !== KNIFE_VIEW_IDS.length
    || evaluation.receipt.view_ids.join('|') !== KNIFE_VIEW_IDS.join('|')) {
    throw new KnifePartBoundaryMetricsError('fixed-view evaluation is not bound to the supplied scene and rig')
  }
  const expectedPixels = rig.frame_width * rig.frame_height
  for (let viewIndex = 0; viewIndex < evaluation.views.length; viewIndex += 1) {
    const view = evaluation.views[viewIndex]
    if (view.view_id !== KNIFE_VIEW_IDS[viewIndex]
      || view.mask.width !== rig.frame_width
      || view.mask.height !== rig.frame_height
      || view.mask.pixels.length !== expectedPixels
      || view.mask.part_indices.length !== expectedPixels
      || view.receipt.renderer_invoked !== false
      || view.receipt.quality_status !== 'NOT_RUN') {
      throw new KnifePartBoundaryMetricsError(`view ${KNIFE_VIEW_IDS[viewIndex]} mask binding is invalid`)
    }
  }
}

function measureView(
  evaluation: KnifeEightViewEvaluation['views'][number],
  partCount: number,
  width: number,
  height: number,
): MeasuredView {
  const pixelCount = width * height
  const boundaryPixels = Array.from({ length: partCount }, () => new Set<number>())
  const boundaryEdges = new Array<number>(partCount).fill(0)
  const visiblePixels = new Array<number>(partCount).fill(0)
  const islands = new Array<number>(partCount).fill(0)
  const visited = new Uint8Array(pixelCount)
  const queue = new Int32Array(pixelCount)
  const partAt = (index: number): number => evaluation.mask.pixels[index] === 0 ? -1 : evaluation.mask.part_indices[index]

  for (let index = 0; index < pixelCount; index += 1) {
    const partIndex = partAt(index)
    if (partIndex < 0) continue
    if (!Number.isInteger(partIndex) || partIndex < 0 || partIndex >= partCount) {
      throw new KnifePartBoundaryMetricsError(`view ${evaluation.view_id} has an invalid part index`)
    }
    visiblePixels[partIndex] += 1
    const x = index % width
    const y = Math.floor(index / width)
    for (const neighbor of boundaryNeighbors(index, x, y, width, height)) {
      if (neighbor < 0 || partAt(neighbor) !== partIndex) {
        boundaryPixels[partIndex].add(index)
        boundaryEdges[partIndex] += 1
      }
    }
    if (visited[index] !== 0) continue
    islands[partIndex] += 1
    let head = 0
    let tail = 1
    queue[0] = index
    visited[index] = 1
    while (head < tail) {
      const current = queue[head++]
      const currentX = current % width
      const currentY = Math.floor(current / width)
      for (const neighbor of neighbors(currentX, currentY, width, height)) {
        if (visited[neighbor] === 0 && partAt(neighbor) === partIndex) {
          visited[neighbor] = 1
          queue[tail++] = neighbor
        }
      }
    }
  }

  const adjacency = createPairMeasurements(partCount)
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const index = y * width + x
      const left = partAt(index)
      if (left < 0) continue
      for (const [neighbor, beyond] of [[x + 1 < width ? index + 1 : -1, x + 2 < width ? index + 2 : -1], [y + 1 < height ? index + width : -1, y + 2 < height ? index + width * 2 : -1]] as const) {
        if (neighbor < 0) continue
        const right = partAt(neighbor)
        if (right >= 0 && right !== left) {
          const pair = adjacency[Math.min(left, right)][Math.max(left, right)]
          pair.contact_edges += 1
          pair.contact_pixels.add(index)
          pair.contact_pixels.add(neighbor)
        } else if (right < 0 && beyond >= 0) {
          const beyondPart = partAt(beyond)
          if (beyondPart >= 0 && beyondPart !== left) {
            const pair = adjacency[Math.min(left, beyondPart)][Math.max(left, beyondPart)]
            pair.gap_edges += 1
            // This is the uncovered gap cell, not a geometric distance claim.
            pair.gap_pixels.add(neighbor)
          }
        }
      }
    }
  }

  const frameDiagonal = Math.hypot(width, height)
  return {
    view_id: evaluation.view_id,
    frame_diagonal: frameDiagonal,
    boundary_pixels: Object.freeze(boundaryPixels),
    boundary_edges: Object.freeze(boundaryEdges),
    islands: Object.freeze(islands),
    visible_pixels: Object.freeze(visiblePixels),
    adjacency: freezePairMeasurements(adjacency),
  }
}

function buildPartMetric(part: CompiledKnifePart, views: readonly MeasuredView[], partIndex: number): KnifePartBoundaryPartMetric {
  const frameDiagonal = views[0]?.frame_diagonal ?? 1
  const viewMetrics = views.map((view) => {
    const boundaryEdgeCount = view.boundary_edges[partIndex]
    return Object.freeze({
      view_id: view.view_id,
      visible_pixel_count: view.visible_pixels[partIndex],
      boundary_pixel_count: view.boundary_pixels[partIndex].size,
      boundary_edge_count: boundaryEdgeCount,
      boundary_length_normalized: boundaryEdgeCount / frameDiagonal,
      connected_island_count: view.islands[partIndex],
    })
  })
  return Object.freeze({
    part_id: part.part_id,
    material_zone_id: part.material_zone_id,
    views: Object.freeze(viewMetrics),
    boundary_pixel_count: viewMetrics.reduce((sum, view) => sum + view.boundary_pixel_count, 0),
    boundary_length_normalized: viewMetrics.reduce((sum, view) => sum + view.boundary_length_normalized, 0) / viewMetrics.length,
    connected_island_count: viewMetrics.reduce((sum, view) => sum + view.connected_island_count, 0),
    status: KNIFE_PART_BOUNDARY_METRICS_STATUS,
  })
}

function buildAdjacencyMatrix(parts: readonly CompiledKnifePart[], views: readonly MeasuredView[]): readonly (readonly KnifePartBoundaryAdjacencyCell[])[] {
  const matrix = parts.map((part, row) => parts.map((neighbor, column) => {
    if (row === column) return emptyAdjacencyCell(part.part_id, neighbor.part_id, 'self', views)
    const measurements = views.map((view) => {
      const pair = view.adjacency[Math.min(row, column)][Math.max(row, column)]
      const contactEdges = pair.contact_edges
      const gapEdges = pair.gap_edges
      return Object.freeze({
        view_id: view.view_id,
        contact_pixel_count: pair.contact_pixels.size,
        contact_edge_count: contactEdges,
        gap_pixel_count: pair.gap_pixels.size,
        gap_edge_count: gapEdges,
        contact_ratio: contactEdges + gapEdges === 0 ? 0 : contactEdges / (contactEdges + gapEdges),
      })
    })
    const relation = semanticRelation(parts[row], parts[column])
    return Object.freeze({
      part_id: part.part_id,
      neighbor_part_id: neighbor.part_id,
      relation,
      views: Object.freeze(measurements),
      contact_pixel_count: measurements.reduce((sum, metric) => sum + metric.contact_pixel_count, 0),
      contact_edge_count: measurements.reduce((sum, metric) => sum + metric.contact_edge_count, 0),
      gap_pixel_count: measurements.reduce((sum, metric) => sum + metric.gap_pixel_count, 0),
      gap_edge_count: measurements.reduce((sum, metric) => sum + metric.gap_edge_count, 0),
      contact_ratio: ratio(
        measurements.reduce((sum, metric) => sum + metric.contact_edge_count, 0),
        measurements.reduce((sum, metric) => sum + metric.gap_edge_count, 0),
      ),
    })
  }))
  return Object.freeze(matrix.map((row) => Object.freeze(row)))
}

function emptyAdjacencyCell(partId: string, neighborPartId: string, relation: KnifePartBoundaryRelation, views: readonly MeasuredView[]): KnifePartBoundaryAdjacencyCell {
  return Object.freeze({
    part_id: partId,
    neighbor_part_id: neighborPartId,
    relation,
    views: Object.freeze(views.map((view) => Object.freeze({
      view_id: view.view_id,
      contact_pixel_count: 0,
      contact_edge_count: 0,
      gap_pixel_count: 0,
      gap_edge_count: 0,
      contact_ratio: 0,
    }))),
    contact_pixel_count: 0,
    contact_edge_count: 0,
    gap_pixel_count: 0,
    gap_edge_count: 0,
    contact_ratio: 0,
  })
}

function semanticRelation(left: CompiledKnifePart, right: CompiledKnifePart): KnifePartBoundaryRelation {
  const roles = [left.surface_role, right.surface_role]
  const has = (role: CompiledKnifePart['surface_role']): boolean => roles.includes(role)
  const isBlade = (role: CompiledKnifePart['surface_role']): boolean => role === 'blade-body' || role === 'cutting-edge'
  const isFeature = (role: CompiledKnifePart['surface_role']): boolean => role === 'fastener' || role === 'gem' || role === 'relief'
  if (isBlade(left.surface_role) && isBlade(right.surface_role)) return 'blade-surface-adjacency'
  if (has('guard') && (isBlade(left.surface_role) || isBlade(right.surface_role))) return 'blade-root-attachment'
  if (has('guard') && isFeature(left.surface_role) || has('guard') && isFeature(right.surface_role)) return 'guard-feature-attachment'
  if (has('grip') && has('guard')) return 'guard-grip-attachment'
  if (has('grip') && has('pommel')) return 'grip-pommel-attachment'
  if (has('grip') && (isFeature(left.surface_role) || isFeature(right.surface_role))) return 'grip-feature-attachment'
  if (has('pommel') && (isFeature(left.surface_role) || isFeature(right.surface_role))) return 'pommel-feature-attachment'
  if (isBlade(left.surface_role) && isFeature(right.surface_role) || isBlade(right.surface_role) && isFeature(left.surface_role)) return 'blade-feature-attachment'
  return 'semantic-part-pair'
}

function neighbors(x: number, y: number, width: number, height: number): number[] {
  const result: number[] = []
  if (x > 0) result.push(y * width + x - 1)
  if (x + 1 < width) result.push(y * width + x + 1)
  if (y > 0) result.push((y - 1) * width + x)
  if (y + 1 < height) result.push((y + 1) * width + x)
  return result
}

function boundaryNeighbors(index: number, x: number, y: number, width: number, height: number): number[] {
  return [
    x > 0 ? index - 1 : -1,
    x + 1 < width ? index + 1 : -1,
    y > 0 ? index - width : -1,
    y + 1 < height ? index + width : -1,
  ]
}

function createPairMeasurements(partCount: number): PairMeasurement[][] {
  return Array.from({ length: partCount }, () => Array.from({ length: partCount }, () => ({
    contact_pixels: new Set<number>(),
    contact_edges: 0,
    gap_pixels: new Set<number>(),
    gap_edges: 0,
  })))
}

function freezePairMeasurements(value: PairMeasurement[][]): readonly (readonly Readonly<PairMeasurement>[])[] {
  return Object.freeze(value.map((row) => Object.freeze(row.map((pair) => Object.freeze(pair)))))
}

function fingerprintMetrics(
  compiled: CompiledKnifeScene,
  rig: KnifeViewRig,
  parts: readonly KnifePartBoundaryPartMetric[],
  matrix: readonly (readonly KnifePartBoundaryAdjacencyCell[])[],
): string {
  const values = [KNIFE_PART_BOUNDARY_METRICS_SCHEMA, compiled.deterministic_fingerprint, rig.deterministic_fingerprint, `${rig.frame_width}x${rig.frame_height}`, KNIFE_PART_BOUNDARY_CONNECTIVITY, KNIFE_PART_BOUNDARY_NORMALIZATION]
  for (const part of parts) {
    values.push(part.part_id, part.material_zone_id, `${part.boundary_pixel_count}`, canonicalNumber(part.boundary_length_normalized), `${part.connected_island_count}`, part.status)
    for (const view of part.views) values.push(view.view_id, `${view.visible_pixel_count}`, `${view.boundary_pixel_count}`, `${view.boundary_edge_count}`, canonicalNumber(view.boundary_length_normalized), `${view.connected_island_count}`)
  }
  for (const row of matrix) for (const cell of row) {
    values.push(cell.part_id, cell.neighbor_part_id, cell.relation, `${cell.contact_pixel_count}`, `${cell.contact_edge_count}`, `${cell.gap_pixel_count}`, `${cell.gap_edge_count}`, canonicalNumber(cell.contact_ratio))
    for (const view of cell.views) values.push(view.view_id, `${view.contact_pixel_count}`, `${view.contact_edge_count}`, `${view.gap_pixel_count}`, `${view.gap_edge_count}`, canonicalNumber(view.contact_ratio))
  }
  return fnv1a64(values.join('|'))
}

function ratio(numerator: number, denominator: number): number {
  const total = numerator + denominator
  return total === 0 ? 0 : numerator / total
}

function assertExactKeys(value: Record<string, unknown>, allowed: readonly string[], context: string): void {
  const allowedSet = new Set(allowed)
  for (const key of Object.keys(value)) if (!allowedSet.has(key)) throw new KnifePartBoundaryMetricsError(`${context} contains unsupported field ${key}`)
}

function isRecord(value: unknown): value is Record<string, any> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
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
