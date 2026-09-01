import * as THREE from 'three'

import type { CompiledKnifeScene } from './knife-scene-compiler.ts'
import type { KnifeVec3 } from './knife-scene-program.ts'

export const KNIFE_VIEW_IDS = [
  'FRONT',
  'BACK',
  'TOP',
  'BOTTOM',
  'LEFT',
  'RIGHT',
  'REAR_THREE_QUARTER',
  'FPS_HOLD',
] as const

export type KnifeViewId = (typeof KNIFE_VIEW_IDS)[number]
export type KnifeProjectionType = 'orthographic' | 'perspective'

export interface KnifeViewDescriptor {
  readonly view_id: KnifeViewId
  readonly projection: KnifeProjectionType
  readonly position: KnifeVec3
  readonly target: KnifeVec3
  readonly up: KnifeVec3
  readonly near: number
  readonly far: number
  readonly ortho_height?: number
  readonly fov_degrees?: number
}

export interface KnifeViewRigOptions {
  readonly frame_width?: number
  readonly frame_height?: number
  readonly margin?: number
}

export const KNIFE_VIEW_CALIBRATION_SCHEMA = 'KnifeViewCalibration@1' as const
export const KNIFE_VIEW_CALIBRATION_RECEIPT_SCHEMA = 'KnifeViewCalibrationReceipt@1' as const
export const KNIFE_VIEW_CALIBRATION_ORIGIN = 'baseline-compiled-scene@1' as const
export const KNIFE_VIEW_CALIBRATION_POLICY = 'baseline-only-reuse@1' as const

/** A world-space, inclusive bound measured from the explicitly focused parts. */
export interface KnifeWorldAabb {
  readonly min: KnifeVec3
  readonly max: KnifeVec3
  readonly size: KnifeVec3
}

/**
 * Immutable camera calibration produced from one baseline compiled scene.
 * The source fingerprint and focus set bind the fit; candidates must reuse
 * this value through createKnifeViewRigFromCalibration rather than fitting
 * their own camera from candidate geometry.
 */
export interface KnifeViewCalibration {
  readonly schema_version: typeof KNIFE_VIEW_CALIBRATION_SCHEMA
  readonly calibration_origin: typeof KNIFE_VIEW_CALIBRATION_ORIGIN
  readonly fit_policy: typeof KNIFE_VIEW_CALIBRATION_POLICY
  readonly source_fingerprint: string
  readonly focus_part_ids: readonly string[]
  readonly world_aabb: KnifeWorldAabb
  readonly center: KnifeVec3
  /** Unpadded orthographic height; the rig margin is applied by the camera. */
  readonly ortho_height: number
  /** Maximum AABB depth span projected on any fixed-view camera axis. */
  readonly depth_span: number
  readonly frame_width: number
  readonly frame_height: number
  readonly margin: number
  readonly deterministic_fingerprint: string
}

export interface KnifeViewCalibrationReceipt {
  readonly schema_version: typeof KNIFE_VIEW_CALIBRATION_RECEIPT_SCHEMA
  readonly calibration_origin: typeof KNIFE_VIEW_CALIBRATION_ORIGIN
  readonly fit_policy: typeof KNIFE_VIEW_CALIBRATION_POLICY
  readonly calibration_fingerprint: string
  readonly source_fingerprint: string
  readonly focus_part_ids: readonly string[]
  readonly world_aabb: KnifeWorldAabb
  readonly center: KnifeVec3
  readonly ortho_height: number
  readonly depth_span: number
  readonly frame_width: number
  readonly frame_height: number
  readonly margin: number
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
  readonly deterministic_fingerprint: string
}

export interface KnifeViewCalibrationOptions extends KnifeViewRigOptions {
  /** Required. There is deliberately no automatic focus-part fallback. */
  readonly focus_part_ids: readonly string[]
}

export interface KnifeViewRig {
  readonly schema_version: 'KnifeFixedEightViewRig@1'
  readonly rig_id: 'knife-fixed-eight-view@1'
  readonly coordinate_convention: 'weapon-front-z-up-right-handed@1'
  readonly frame_width: number
  readonly frame_height: number
  readonly margin: number
  readonly views: readonly KnifeViewDescriptor[]
  readonly calibration?: KnifeViewCalibration
  readonly calibration_receipt?: KnifeViewCalibrationReceipt
  /** Browser-safe structural fingerprint, not a Runtime/CAS hash. */
  readonly deterministic_fingerprint: string
}

export interface KnifeCalibratedViewRig extends KnifeViewRig {
  readonly calibration: KnifeViewCalibration
  readonly calibration_receipt: KnifeViewCalibrationReceipt
}

export interface KnifeViewCalibrationResult {
  readonly calibration: KnifeViewCalibration
  readonly rig: KnifeCalibratedViewRig
  readonly receipt: KnifeViewCalibrationReceipt
}

export interface KnifeProjectedVertex {
  readonly x_px: number
  readonly y_px: number
  readonly depth_ndc: number
  readonly clip_visible: boolean
  /** Cohen-Sutherland-style homogeneous clip outcode; zero means in-frustum. */
  readonly clip_outcode: number
}

export interface KnifeProjectedTriangle {
  readonly a: number
  readonly b: number
  readonly c: number
  readonly part_index: number
  readonly material_index: number
  readonly part_id: string
  readonly material_zone_id: string
}

export interface KnifeProjectionReceipt {
  readonly schema_version: 'WeaponryThreeJsProjectionReceipt@1'
  readonly rig_schema_version: 'KnifeFixedEightViewRig@1'
  readonly rig_fingerprint: string
  readonly source_fingerprint: string
  readonly view_id: KnifeViewId
  readonly frame_width: number
  readonly frame_height: number
  readonly projection_type: KnifeProjectionType
  readonly projected_vertex_count: number
  readonly projected_triangle_count: number
  readonly clip_visible_vertex_count: number
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
  readonly deterministic_fingerprint: string
}

export interface KnifeProjectionResult {
  readonly schema_version: 'WeaponryThreeJsProjection@1'
  readonly rig: KnifeViewRig
  readonly view: KnifeViewDescriptor
  readonly part_ids: readonly string[]
  readonly material_zone_ids: readonly string[]
  readonly vertices: readonly KnifeProjectedVertex[]
  readonly triangles: readonly KnifeProjectedTriangle[]
  readonly receipt: KnifeProjectionReceipt
}

export interface KnifeMaskReceipt {
  readonly schema_version: 'WeaponryThreeJsMaskReceipt@1'
  readonly projection_fingerprint: string
  readonly view_id: KnifeViewId
  readonly frame_width: number
  readonly frame_height: number
  /**
   * The bounded software rasterizer performs same-plane frustum rejection and
   * bounds accepted samples to NDC depth. It is not a full polygon clipper.
   */
  readonly rasterizer: 'software-triangle-mask@2'
  readonly anti_aliasing: 'none'
  readonly covered_pixel_count: number
  readonly coverage_ratio: number
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
  readonly deterministic_fingerprint: string
}

export interface KnifeMaskResult {
  readonly schema_version: 'WeaponryThreeJsMask@1'
  readonly width: number
  readonly height: number
  readonly pixels: Uint8Array
  readonly part_indices: Uint16Array
  readonly material_indices: Uint16Array
  readonly depth: Float32Array
  readonly receipt: KnifeMaskReceipt
}

export interface KnifeViewEvaluationReceipt {
  readonly schema_version: 'WeaponryThreeJsViewEvaluationReceipt@1'
  readonly rig_id: 'knife-fixed-eight-view@1'
  readonly rig_fingerprint: string
  readonly source_fingerprint: string
  readonly view_id: KnifeViewId
  readonly projection_fingerprint: string
  readonly mask_fingerprint: string
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
}

export interface KnifeViewEvaluation {
  readonly view_id: KnifeViewId
  readonly projection: KnifeProjectionResult
  readonly mask: KnifeMaskResult
  readonly receipt: KnifeViewEvaluationReceipt
}

export interface KnifeEightViewEvaluationReceipt {
  readonly schema_version: 'WeaponryThreeJsEightViewEvaluationReceipt@1'
  readonly rig_id: 'knife-fixed-eight-view@1'
  readonly rig_fingerprint: string
  readonly source_fingerprint: string
  readonly view_ids: readonly KnifeViewId[]
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
  readonly deterministic_fingerprint: string
}

export interface KnifeEightViewEvaluation {
  readonly rig: KnifeViewRig
  readonly views: readonly KnifeViewEvaluation[]
  readonly receipt: KnifeEightViewEvaluationReceipt
}

export class KnifeViewEvaluationError extends Error {
  constructor(message: string) {
    super(`KNIFE_VIEW_EVALUATION_INVALID: ${message}`)
    this.name = 'KnifeViewEvaluationError'
  }
}

const DEFAULT_FRAME_WIDTH = 256
const DEFAULT_FRAME_HEIGHT = 256
const DEFAULT_MARGIN = 0.08
const MIN_CALIBRATION_SPAN = 1e-4
const MAX_CALIBRATION_FOCUS_PARTS = 32
const MAX_CALIBRATION_SCENE_PARTS = 256
const MAX_CALIBRATION_VERTICES = 2_000_000
const MAX_CALIBRATION_TRIANGLES = 1_000_000
const MAX_CALIBRATION_COORDINATE = 1_000_000
const STABLE_ID_PATTERN = /^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/
const FINGERPRINT_PATTERN = /^[a-f0-9]{16,128}$/i

export function createKnifeViewRig(options: KnifeViewRigOptions = {}): KnifeViewRig {
  const frameWidth = boundedFrameDimension(options.frame_width ?? DEFAULT_FRAME_WIDTH, 'frame_width')
  const frameHeight = boundedFrameDimension(options.frame_height ?? DEFAULT_FRAME_HEIGHT, 'frame_height')
  const margin = options.margin ?? DEFAULT_MARGIN
  if (!Number.isFinite(margin) || margin < 0 || margin >= 0.45) {
    throw new KnifeViewEvaluationError('margin must be finite and in [0, 0.45)')
  }

  const views: readonly KnifeViewDescriptor[] = [
    orthographicView('FRONT', [0, 0, 3.5], [0, 0, 0], [0, 1, 0]),
    orthographicView('BACK', [0, 0, -3.5], [0, 0, 0], [0, 1, 0]),
    orthographicView('TOP', [0, 3.5, 0], [0, 0, 0], [0, 0, 1]),
    orthographicView('BOTTOM', [0, -3.5, 0], [0, 0, 0], [0, 0, 1]),
    orthographicView('LEFT', [-3.5, 0, 0], [0, 0, 0], [0, 1, 0]),
    orthographicView('RIGHT', [3.5, 0, 0], [0, 0, 0], [0, 1, 0]),
    orthographicView('REAR_THREE_QUARTER', [2.2, 1.15, -2.6], [0, 0, 0], [0, 1, 0]),
    perspectiveView('FPS_HOLD', [0, -0.2, 2.8], [0.18, 0, 0], [0, 1, 0]),
  ].map((view) => Object.freeze({
    ...view,
    position: Object.freeze([...view.position]) as unknown as KnifeVec3,
    target: Object.freeze([...view.target]) as unknown as KnifeVec3,
    up: Object.freeze([...view.up]) as unknown as KnifeVec3,
  }))

  const rig = {
    schema_version: 'KnifeFixedEightViewRig@1' as const,
    rig_id: 'knife-fixed-eight-view@1' as const,
    coordinate_convention: 'weapon-front-z-up-right-handed@1' as const,
    frame_width: frameWidth,
    frame_height: frameHeight,
    margin,
    views: Object.freeze(views),
    deterministic_fingerprint: '',
  }
  const fingerprint = hashRig(rig)
  return Object.freeze({ ...rig, deterministic_fingerprint: fingerprint })
}

export const createKnifeFixedEightViewRig = createKnifeViewRig

/**
 * Fit the fixed camera set once from a baseline compiled scene.  The focus
 * list is mandatory: omission never falls back to all parts or to a guessed
 * blade subset.  The returned calibration and receipt are deeply frozen.
 */
export function calibrateKnifeViewRig(
  compiled: CompiledKnifeScene,
  options: KnifeViewCalibrationOptions,
): KnifeViewCalibrationResult
export function calibrateKnifeViewRig(
  compiled: CompiledKnifeScene,
  focusPartIds: readonly string[],
  options?: KnifeViewRigOptions,
): KnifeViewCalibrationResult
export function calibrateKnifeViewRig(
  compiled: CompiledKnifeScene,
  optionsOrFocus: KnifeViewCalibrationOptions | readonly string[],
  legacyOptions: KnifeViewRigOptions = {},
): KnifeViewCalibrationResult {
  const options: KnifeViewCalibrationOptions = Array.isArray(optionsOrFocus)
    ? { ...legacyOptions, focus_part_ids: optionsOrFocus }
    : optionsOrFocus as KnifeViewCalibrationOptions
  if (!options || typeof options !== 'object' || Array.isArray(options)) {
    throw new KnifeViewEvaluationError('calibration options with explicit focus_part_ids are required')
  }
  validateCalibrationOptions(options)
  const baseRig = createKnifeViewRig({
    frame_width: options.frame_width,
    frame_height: options.frame_height,
    margin: options.margin,
  })
  const focusPartIds = normalizeCalibrationFocusPartIds(compiled, options.focus_part_ids)
  const worldAabb = computeFocusWorldAabb(compiled, focusPartIds)
  const metrics = measureCalibrationMetrics(baseRig, worldAabb)
  const calibration = freezeCalibration({
    schema_version: KNIFE_VIEW_CALIBRATION_SCHEMA,
    calibration_origin: KNIFE_VIEW_CALIBRATION_ORIGIN,
    fit_policy: KNIFE_VIEW_CALIBRATION_POLICY,
    source_fingerprint: compiled.deterministic_fingerprint,
    focus_part_ids: focusPartIds,
    world_aabb: worldAabb,
    center: centerOfAabb(worldAabb),
    ortho_height: metrics.ortho_height,
    depth_span: metrics.depth_span,
    frame_width: baseRig.frame_width,
    frame_height: baseRig.frame_height,
    margin: baseRig.margin,
    deterministic_fingerprint: '',
  })
  const finalizedCalibration = freezeCalibration({
    ...calibration,
    deterministic_fingerprint: hashCalibration(calibration),
  })
  const receipt = makeCalibrationReceipt(finalizedCalibration)
  const rig = buildCalibratedViewRig(baseRig, finalizedCalibration, receipt)
  return Object.freeze({ calibration: finalizedCalibration, rig, receipt })
}

/**
 * Recreate a calibrated rig without accepting a scene.  This is intentionally
 * a separate API so a later candidate can only reuse the baseline fit and
 * cannot silently refit its cameras from candidate geometry.
 */
export function createKnifeViewRigFromCalibration(
  calibration: KnifeViewCalibration,
): KnifeCalibratedViewRig {
  validateKnifeViewCalibration(calibration)
  const stableCalibration = freezeCalibration(calibration)
  const baseRig = createKnifeViewRig({
    frame_width: stableCalibration.frame_width,
    frame_height: stableCalibration.frame_height,
    margin: stableCalibration.margin,
  })
  const receipt = makeCalibrationReceipt(stableCalibration)
  return buildCalibratedViewRig(baseRig, stableCalibration, receipt)
}

/** Alias kept explicit for callers that read the result as a calibrated rig. */
export const createKnifeCalibratedViewRig = createKnifeViewRigFromCalibration

/** Validate a calibration before it is persisted or reused by a candidate. */
export function validateKnifeViewCalibration(calibration: KnifeViewCalibration): string {
  if (!calibration || typeof calibration !== 'object' || Array.isArray(calibration)) {
    throw new KnifeViewEvaluationError('calibration must be an object')
  }
  exactCalibrationKeys(calibration)
  if (calibration.schema_version !== KNIFE_VIEW_CALIBRATION_SCHEMA) {
    throw new KnifeViewEvaluationError(`calibration schema must be ${KNIFE_VIEW_CALIBRATION_SCHEMA}`)
  }
  if (calibration.calibration_origin !== KNIFE_VIEW_CALIBRATION_ORIGIN) {
    throw new KnifeViewEvaluationError('calibration origin must be baseline-compiled-scene@1')
  }
  if (calibration.fit_policy !== KNIFE_VIEW_CALIBRATION_POLICY) {
    throw new KnifeViewEvaluationError('calibration fit policy must be baseline-only-reuse@1')
  }
  if (!isStableFingerprint(calibration.source_fingerprint)) {
    throw new KnifeViewEvaluationError('calibration source fingerprint is invalid')
  }
  validateCalibrationFocusIds(calibration.focus_part_ids)
  validateWorldAabb(calibration.world_aabb)
  validateVec3(calibration.center, 'calibration center')
  const expectedCenter = centerOfAabb(calibration.world_aabb)
  if (!sameVec3(calibration.center, expectedCenter)) {
    throw new KnifeViewEvaluationError('calibration center does not match world AABB')
  }
  validateCalibrationNumber(calibration.ortho_height, 'calibration ortho_height')
  validateCalibrationNumber(calibration.depth_span, 'calibration depth_span')
  boundedFrameDimension(calibration.frame_width, 'calibration frame_width')
  boundedFrameDimension(calibration.frame_height, 'calibration frame_height')
  if (!Number.isFinite(calibration.margin) || calibration.margin < 0 || calibration.margin >= 0.45) {
    throw new KnifeViewEvaluationError('calibration margin must be finite and in [0, 0.45)')
  }
  if (!isStableFingerprint(calibration.deterministic_fingerprint)) {
    throw new KnifeViewEvaluationError('calibration deterministic fingerprint is invalid')
  }
  if (calibration.deterministic_fingerprint !== hashCalibration(calibration)) {
    throw new KnifeViewEvaluationError('calibration deterministic fingerprint does not match its fields')
  }
  return calibration.deterministic_fingerprint
}

function normalizeCalibrationFocusPartIds(
  compiled: CompiledKnifeScene,
  focusPartIds: readonly string[] | undefined,
): readonly string[] {
  validateCompiledSceneForCalibration(compiled)
  if (!Array.isArray(focusPartIds) || focusPartIds.length < 1 || focusPartIds.length > MAX_CALIBRATION_FOCUS_PARTS) {
    throw new KnifeViewEvaluationError(
      `focus_part_ids must explicitly contain one to ${MAX_CALIBRATION_FOCUS_PARTS} parts; automatic focus is disabled`,
    )
  }
  const available = new Set<string>()
  for (const part of compiled.parts) {
    if (available.has(part.part_id)) throw new KnifeViewEvaluationError(`compiled scene has duplicate part ID ${part.part_id}`)
    available.add(part.part_id)
  }
  const selected = new Set<string>()
  for (const partId of focusPartIds) {
    if (!isStablePartId(partId)) throw new KnifeViewEvaluationError('focus_part_ids contains an invalid stable part ID')
    if (selected.has(partId)) throw new KnifeViewEvaluationError(`focus_part_ids contains duplicate part ID ${partId}`)
    if (!available.has(partId)) throw new KnifeViewEvaluationError(`focus part ID ${partId} does not exist in the compiled scene`)
    selected.add(partId)
  }
  return Object.freeze([...selected].sort(compareStableId))
}

function validateCalibrationOptions(options: KnifeViewCalibrationOptions): void {
  const allowed = ['focus_part_ids', 'frame_width', 'frame_height', 'margin']
  const actual = Object.keys(options).sort()
  if (actual.some((key) => !allowed.includes(key))) {
    throw new KnifeViewEvaluationError('calibration options contain an unsupported field')
  }
}

function validateCompiledSceneForCalibration(compiled: CompiledKnifeScene): void {
  if (!compiled || typeof compiled !== 'object' || !Array.isArray(compiled.parts)) {
    throw new KnifeViewEvaluationError('baseline compiled scene is required for calibration')
  }
  if (!compiled.group || typeof compiled.group.updateMatrixWorld !== 'function') {
    throw new KnifeViewEvaluationError('baseline compiled scene has no valid Object3D group')
  }
  if (compiled.parts.length < 1 || compiled.parts.length > MAX_CALIBRATION_SCENE_PARTS) {
    throw new KnifeViewEvaluationError(`compiled scene parts exceed calibration budget ${MAX_CALIBRATION_SCENE_PARTS}`)
  }
  if (!Number.isInteger(compiled.triangle_count) || compiled.triangle_count < 1 || compiled.triangle_count > MAX_CALIBRATION_TRIANGLES) {
    throw new KnifeViewEvaluationError(`compiled scene triangle count exceeds calibration budget ${MAX_CALIBRATION_TRIANGLES}`)
  }
  if (!isStableFingerprint(compiled.deterministic_fingerprint)) {
    throw new KnifeViewEvaluationError('compiled scene fingerprint is invalid')
  }
}

function computeFocusWorldAabb(
  compiled: CompiledKnifeScene,
  focusPartIds: readonly string[],
): KnifeWorldAabb {
  compiled.group.updateMatrixWorld(true)
  const partsById = new Map(compiled.parts.map((part) => [part.part_id, part]))
  const min = new THREE.Vector3(Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY)
  const max = new THREE.Vector3(Number.NEGATIVE_INFINITY, Number.NEGATIVE_INFINITY, Number.NEGATIVE_INFINITY)
  let vertexBudget = 0
  let triangleBudget = 0

  for (const partId of focusPartIds) {
    const part = partsById.get(partId)
    if (!part) throw new KnifeViewEvaluationError(`focus part ID ${partId} does not exist in the compiled scene`)
    if (!(part.geometry instanceof THREE.BufferGeometry) || !part.mesh || typeof part.mesh.updateWorldMatrix !== 'function') {
      throw new KnifeViewEvaluationError(`focus part ${partId} has no valid BufferGeometry/Object3D binding`)
    }
    const position = part.geometry.getAttribute('position')
    if (!position || position.itemSize !== 3 || !Number.isInteger(position.count) || position.count < 3) {
      throw new KnifeViewEvaluationError(`focus part ${partId} has an invalid position attribute`)
    }
    if (position.count > MAX_CALIBRATION_VERTICES - vertexBudget) {
      throw new KnifeViewEvaluationError(`focus geometry exceeds vertex calibration budget ${MAX_CALIBRATION_VERTICES}`)
    }
    const index = part.geometry.getIndex()
    const indexCount = index ? index.count : position.count
    if (!Number.isInteger(indexCount) || indexCount < 3 || indexCount % 3 !== 0) {
      throw new KnifeViewEvaluationError(`focus part ${partId} has an invalid triangle index count`)
    }
    const partTriangles = indexCount / 3
    if (!Number.isSafeInteger(partTriangles) || partTriangles > MAX_CALIBRATION_TRIANGLES - triangleBudget) {
      throw new KnifeViewEvaluationError(`focus geometry exceeds triangle calibration budget ${MAX_CALIBRATION_TRIANGLES}`)
    }
    if (index) {
      for (let indexOffset = 0; indexOffset < index.count; indexOffset += 1) {
        const vertexIndex = index.getX(indexOffset)
        if (!Number.isInteger(vertexIndex) || vertexIndex < 0 || vertexIndex >= position.count) {
          throw new KnifeViewEvaluationError(`focus part ${partId} has an out-of-range triangle index`)
        }
      }
    }
    vertexBudget += position.count
    triangleBudget += partTriangles

    part.mesh.updateWorldMatrix(true, false)
    if (part.mesh.matrixWorld.elements.some((value) => !Number.isFinite(value))) {
      throw new KnifeViewEvaluationError(`focus part ${partId} has a non-finite world transform`)
    }
    const world = new THREE.Vector3()
    for (let vertexIndex = 0; vertexIndex < position.count; vertexIndex += 1) {
      world.fromBufferAttribute(position, vertexIndex).applyMatrix4(part.mesh.matrixWorld)
      if (!Number.isFinite(world.x) || !Number.isFinite(world.y) || !Number.isFinite(world.z)
        || Math.abs(world.x) > MAX_CALIBRATION_COORDINATE
        || Math.abs(world.y) > MAX_CALIBRATION_COORDINATE
        || Math.abs(world.z) > MAX_CALIBRATION_COORDINATE) {
        throw new KnifeViewEvaluationError(`focus part ${partId} has a non-finite or out-of-budget world vertex`)
      }
      min.min(world)
      max.max(world)
    }
  }

  if (![min.x, min.y, min.z, max.x, max.y, max.z].every(Number.isFinite)) {
    throw new KnifeViewEvaluationError('focus parts produced an empty world AABB')
  }
  return freezeWorldAabb({
    min: [min.x, min.y, min.z],
    max: [max.x, max.y, max.z],
    size: [max.x - min.x, max.y - min.y, max.z - min.z],
  })
}

function measureCalibrationMetrics(
  baseRig: KnifeViewRig,
  worldAabb: KnifeWorldAabb,
): { readonly ortho_height: number; readonly depth_span: number } {
  const corners = aabbCorners(worldAabb)
  const center = new THREE.Vector3(...centerOfAabb(worldAabb))
  const aspect = baseRig.frame_width / baseRig.frame_height
  let orthoHeight = MIN_CALIBRATION_SPAN
  let depthSpan = MIN_CALIBRATION_SPAN

  for (const view of baseRig.views) {
    const camera = createKnifeViewCamera(baseRig, view.view_id)
    const right = new THREE.Vector3().setFromMatrixColumn(camera.matrixWorld, 0).normalize()
    const up = new THREE.Vector3().setFromMatrixColumn(camera.matrixWorld, 1).normalize()
    const backwards = new THREE.Vector3().setFromMatrixColumn(camera.matrixWorld, 2).normalize()
    if (![right.x, right.y, right.z, up.x, up.y, up.z, backwards.x, backwards.y, backwards.z].every(Number.isFinite)) {
      throw new KnifeViewEvaluationError(`view ${view.view_id} produced a non-finite calibration basis`)
    }

    let minHorizontal = Number.POSITIVE_INFINITY
    let maxHorizontal = Number.NEGATIVE_INFINITY
    let minVertical = Number.POSITIVE_INFINITY
    let maxVertical = Number.NEGATIVE_INFINITY
    let minDepth = Number.POSITIVE_INFINITY
    let maxDepth = Number.NEGATIVE_INFINITY
    for (const corner of corners) {
      const relative = corner.clone().sub(center)
      const horizontal = relative.dot(right)
      const vertical = relative.dot(up)
      const depth = relative.dot(backwards)
      minHorizontal = Math.min(minHorizontal, horizontal)
      maxHorizontal = Math.max(maxHorizontal, horizontal)
      minVertical = Math.min(minVertical, vertical)
      maxVertical = Math.max(maxVertical, vertical)
      minDepth = Math.min(minDepth, depth)
      maxDepth = Math.max(maxDepth, depth)
    }
    const horizontalSpan = Math.max(MIN_CALIBRATION_SPAN, maxHorizontal - minHorizontal)
    const verticalSpan = Math.max(MIN_CALIBRATION_SPAN, maxVertical - minVertical)
    const viewDepthSpan = Math.max(MIN_CALIBRATION_SPAN, maxDepth - minDepth)
    depthSpan = Math.max(depthSpan, viewDepthSpan)
    if (view.projection === 'orthographic') {
      orthoHeight = Math.max(orthoHeight, verticalSpan, horizontalSpan / aspect)
    }
  }

  if (!Number.isFinite(orthoHeight) || !Number.isFinite(depthSpan)
    || orthoHeight > MAX_CALIBRATION_COORDINATE * 4 || depthSpan > MAX_CALIBRATION_COORDINATE * 4) {
    throw new KnifeViewEvaluationError('calibration fit metrics are non-finite or exceed budget')
  }
  return { ortho_height: orthoHeight, depth_span: depthSpan }
}

function buildCalibratedViewRig(
  baseRig: KnifeViewRig,
  calibration: KnifeViewCalibration,
  receipt: KnifeViewCalibrationReceipt,
): KnifeCalibratedViewRig {
  const views = baseRig.views.map((view) => {
    const offset = new THREE.Vector3(...view.position).sub(new THREE.Vector3(...view.target))
    const distance = offset.length()
    const depthPadding = Math.max(MIN_CALIBRATION_SPAN, calibration.depth_span * 0.02)
    const near = Math.max(0.001, Math.min(view.near, distance - calibration.depth_span * 0.5 - depthPadding))
    const far = Math.max(view.far, distance + calibration.depth_span * 0.5 + depthPadding)
    const calibrated: KnifeViewDescriptor = {
      ...view,
      position: freezeVec3(new THREE.Vector3(...calibration.center).add(offset)),
      target: freezeVec3(calibration.center),
      near,
      far,
      ...(view.projection === 'orthographic' ? { ortho_height: calibration.ortho_height } : {}),
    }
    return Object.freeze(calibrated)
  })
  const draft: KnifeCalibratedViewRig = {
    ...baseRig,
    views: Object.freeze(views),
    calibration,
    calibration_receipt: receipt,
    deterministic_fingerprint: '',
  }
  return Object.freeze({ ...draft, deterministic_fingerprint: hashRig(draft) })
}

function makeCalibrationReceipt(calibration: KnifeViewCalibration): KnifeViewCalibrationReceipt {
  const receiptFingerprint = fnv1a64([
    KNIFE_VIEW_CALIBRATION_RECEIPT_SCHEMA,
    calibration.calibration_origin,
    calibration.fit_policy,
    calibration.deterministic_fingerprint,
    calibration.source_fingerprint,
    calibration.focus_part_ids.join(','),
    ...calibration.world_aabb.min.map(canonicalNumber),
    ...calibration.world_aabb.max.map(canonicalNumber),
    ...calibration.world_aabb.size.map(canonicalNumber),
    ...calibration.center.map(canonicalNumber),
    canonicalNumber(calibration.ortho_height),
    canonicalNumber(calibration.depth_span),
    `${calibration.frame_width}x${calibration.frame_height}`,
    canonicalNumber(calibration.margin),
    'renderer_invoked=false',
    'quality_status=NOT_RUN',
  ].join('|'))
  return Object.freeze({
    schema_version: KNIFE_VIEW_CALIBRATION_RECEIPT_SCHEMA,
    calibration_origin: calibration.calibration_origin,
    fit_policy: calibration.fit_policy,
    calibration_fingerprint: calibration.deterministic_fingerprint,
    source_fingerprint: calibration.source_fingerprint,
    focus_part_ids: calibration.focus_part_ids,
    world_aabb: calibration.world_aabb,
    center: calibration.center,
    ortho_height: calibration.ortho_height,
    depth_span: calibration.depth_span,
    frame_width: calibration.frame_width,
    frame_height: calibration.frame_height,
    margin: calibration.margin,
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
    deterministic_fingerprint: receiptFingerprint,
  })
}

function hashCalibration(calibration: Omit<KnifeViewCalibration, 'deterministic_fingerprint'> | KnifeViewCalibration): string {
  return fnv1a64([
    calibration.schema_version,
    calibration.calibration_origin,
    calibration.fit_policy,
    calibration.source_fingerprint,
    calibration.focus_part_ids.join(','),
    ...calibration.world_aabb.min.map(canonicalNumber),
    ...calibration.world_aabb.max.map(canonicalNumber),
    ...calibration.world_aabb.size.map(canonicalNumber),
    ...calibration.center.map(canonicalNumber),
    canonicalNumber(calibration.ortho_height),
    canonicalNumber(calibration.depth_span),
    `${calibration.frame_width}x${calibration.frame_height}`,
    canonicalNumber(calibration.margin),
  ].join('|'))
}

function freezeCalibration(calibration: KnifeViewCalibration): KnifeViewCalibration {
  return Object.freeze({
    ...calibration,
    focus_part_ids: Object.freeze([...calibration.focus_part_ids]),
    world_aabb: freezeWorldAabb(calibration.world_aabb),
    center: freezeVec3(calibration.center),
  })
}

function freezeWorldAabb(worldAabb: KnifeWorldAabb): KnifeWorldAabb {
  return Object.freeze({
    min: freezeVec3(worldAabb.min),
    max: freezeVec3(worldAabb.max),
    size: freezeVec3(worldAabb.size),
  })
}

function aabbCorners(worldAabb: KnifeWorldAabb): readonly THREE.Vector3[] {
  const [minX, minY, minZ] = worldAabb.min
  const [maxX, maxY, maxZ] = worldAabb.max
  return Object.freeze([
    new THREE.Vector3(minX, minY, minZ),
    new THREE.Vector3(minX, minY, maxZ),
    new THREE.Vector3(minX, maxY, minZ),
    new THREE.Vector3(minX, maxY, maxZ),
    new THREE.Vector3(maxX, minY, minZ),
    new THREE.Vector3(maxX, minY, maxZ),
    new THREE.Vector3(maxX, maxY, minZ),
    new THREE.Vector3(maxX, maxY, maxZ),
  ])
}

function validateWorldAabb(worldAabb: KnifeWorldAabb): void {
  if (!worldAabb || typeof worldAabb !== 'object' || Array.isArray(worldAabb)) {
    throw new KnifeViewEvaluationError('calibration world_aabb must be an object')
  }
  const keys = Object.keys(worldAabb).sort()
  if (keys.join('|') !== 'max|min|size') throw new KnifeViewEvaluationError('calibration world_aabb fields are not closed')
  validateVec3(worldAabb.min, 'calibration world_aabb.min')
  validateVec3(worldAabb.max, 'calibration world_aabb.max')
  validateVec3(worldAabb.size, 'calibration world_aabb.size')
  const expectedSize: KnifeVec3 = [
    worldAabb.max[0] - worldAabb.min[0],
    worldAabb.max[1] - worldAabb.min[1],
    worldAabb.max[2] - worldAabb.min[2],
  ]
  if (worldAabb.min.some((value, index) => value > worldAabb.max[index]) || !sameVec3(worldAabb.size, expectedSize)) {
    throw new KnifeViewEvaluationError('calibration world_aabb bounds are inconsistent')
  }
}

function validateCalibrationFocusIds(focusPartIds: readonly string[]): void {
  if (!Array.isArray(focusPartIds) || focusPartIds.length < 1 || focusPartIds.length > MAX_CALIBRATION_FOCUS_PARTS) {
    throw new KnifeViewEvaluationError('calibration focus_part_ids are outside the bounded range')
  }
  const seen = new Set<string>()
  for (let index = 0; index < focusPartIds.length; index += 1) {
    const partId = focusPartIds[index]
    if (!isStablePartId(partId) || seen.has(partId)) throw new KnifeViewEvaluationError('calibration focus_part_ids must be stable and unique')
    if (index > 0 && compareStableId(focusPartIds[index - 1], partId) >= 0) {
      throw new KnifeViewEvaluationError('calibration focus_part_ids must use canonical order')
    }
    seen.add(partId)
  }
}

function validateVec3(value: KnifeVec3, label: string): void {
  if (!Array.isArray(value) || value.length !== 3 || value.some((coordinate) => !Number.isFinite(coordinate) || Math.abs(coordinate) > MAX_CALIBRATION_COORDINATE)) {
    throw new KnifeViewEvaluationError(`${label} must contain three finite bounded values`)
  }
}

function validateCalibrationNumber(value: number, label: string): void {
  if (!Number.isFinite(value) || value < MIN_CALIBRATION_SPAN || value > MAX_CALIBRATION_COORDINATE * 4) {
    throw new KnifeViewEvaluationError(`${label} is outside the bounded calibration range`)
  }
}

function exactCalibrationKeys(calibration: KnifeViewCalibration): void {
  const expected = [
    'calibration_origin',
    'center',
    'depth_span',
    'deterministic_fingerprint',
    'fit_policy',
    'focus_part_ids',
    'frame_height',
    'frame_width',
    'margin',
    'ortho_height',
    'schema_version',
    'source_fingerprint',
    'world_aabb',
  ]
  const actual = Object.keys(calibration).sort()
  if (actual.join('|') !== expected.sort().join('|')) {
    throw new KnifeViewEvaluationError('calibration fields are not closed')
  }
}

function centerOfAabb(worldAabb: KnifeWorldAabb): KnifeVec3 {
  return [
    (worldAabb.min[0] + worldAabb.max[0]) * 0.5,
    (worldAabb.min[1] + worldAabb.max[1]) * 0.5,
    (worldAabb.min[2] + worldAabb.max[2]) * 0.5,
  ]
}

function freezeVec3(value: KnifeVec3 | THREE.Vector3): KnifeVec3 {
  const values = value instanceof THREE.Vector3 ? [value.x, value.y, value.z] : [value[0], value[1], value[2]]
  return Object.freeze(values) as unknown as KnifeVec3
}

function sameVec3(left: KnifeVec3, right: KnifeVec3): boolean {
  return left.every((value, index) => Math.abs(value - right[index]) <= 1e-9)
}

function isStablePartId(value: unknown): value is string {
  return typeof value === 'string' && STABLE_ID_PATTERN.test(value)
}

function isStableFingerprint(value: unknown): value is string {
  return typeof value === 'string' && FINGERPRINT_PATTERN.test(value)
}

function compareStableId(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0
}

export function getKnifeView(rig: KnifeViewRig, viewId: KnifeViewId): KnifeViewDescriptor {
  validateRig(rig)
  const view = rig.views.find((candidate) => candidate.view_id === viewId)
  if (!view) throw new KnifeViewEvaluationError(`view ${viewId} is not in the fixed rig`)
  return view
}

export function createKnifeViewCamera(rig: KnifeViewRig, viewId: KnifeViewId): THREE.Camera {
  const view = getKnifeView(rig, viewId)
  const aspect = rig.frame_width / rig.frame_height
  const effectiveOrthoHeight = view.ortho_height ? view.ortho_height * (1 + rig.margin * 2) : undefined
  const camera = view.projection === 'orthographic'
    ? new THREE.OrthographicCamera(
        -(effectiveOrthoHeight as number) * aspect * 0.5,
        (effectiveOrthoHeight as number) * aspect * 0.5,
        (effectiveOrthoHeight as number) * 0.5,
        -(effectiveOrthoHeight as number) * 0.5,
        view.near,
        view.far,
      )
    : new THREE.PerspectiveCamera(view.fov_degrees as number, aspect, view.near, view.far)

  camera.name = `knife-camera:${view.view_id}`
  overrideUuid(camera, stableUuid(`camera:${rig.deterministic_fingerprint}:${view.view_id}`))
  camera.position.fromArray(view.position)
  camera.up.fromArray(view.up)
  camera.lookAt(new THREE.Vector3(...view.target))
  camera.updateProjectionMatrix()
  camera.updateMatrixWorld(true)
  camera.userData = {
    schema_version: 'KnifeFixedEightViewRig@1',
    rig_id: rig.rig_id,
    rig_fingerprint: rig.deterministic_fingerprint,
    view_id: view.view_id,
    projection: view.projection,
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
  }
  return camera
}

export function projectKnifeScene(compiled: CompiledKnifeScene, rig: KnifeViewRig, viewId: KnifeViewId): KnifeProjectionResult {
  validateRig(rig)
  const view = getKnifeView(rig, viewId)
  const camera = createKnifeViewCamera(rig, viewId)
  compiled.group.updateMatrixWorld(true)

  const partIds = compiled.parts.map((part) => part.part_id)
  const materialZoneIds = [...new Set(compiled.parts.map((part) => part.material_zone_id))].sort()
  const materialIndexById = new Map(materialZoneIds.map((id, index) => [id, index]))
  const vertices: KnifeProjectedVertex[] = []
  const triangles: KnifeProjectedTriangle[] = []

  for (let partIndex = 0; partIndex < compiled.parts.length; partIndex += 1) {
    const part = compiled.parts[partIndex]
    const position = part.geometry.getAttribute('position')
    if (!position || position.itemSize !== 3) throw new KnifeViewEvaluationError(`part ${part.part_id} has no position attribute`)
    part.mesh.updateWorldMatrix(true, false)
    const firstVertex = vertices.length
    for (let vertexIndex = 0; vertexIndex < position.count; vertexIndex += 1) {
      const world = new THREE.Vector3().fromBufferAttribute(position, vertexIndex).applyMatrix4(part.mesh.matrixWorld)
      const clip = new THREE.Vector4(world.x, world.y, world.z, 1)
        .applyMatrix4(camera.matrixWorldInverse)
        .applyMatrix4(camera.projectionMatrix)
      const finiteClip = Number.isFinite(clip.x) && Number.isFinite(clip.y) && Number.isFinite(clip.z) && Number.isFinite(clip.w)
      const safeW = finiteClip && Math.abs(clip.w) > Number.EPSILON ? clip.w : Number.NaN
      const ndcX = clip.x / safeW
      const ndcY = clip.y / safeW
      const ndcZ = clip.z / safeW
      let clipOutcode = 0
      if (!finiteClip || !Number.isFinite(ndcX) || !Number.isFinite(ndcY) || !Number.isFinite(ndcZ) || clip.w <= 0) {
        clipOutcode = 0x40
      } else {
        if (clip.x < -clip.w) clipOutcode |= 0x01
        if (clip.x > clip.w) clipOutcode |= 0x02
        if (clip.y < -clip.w) clipOutcode |= 0x04
        if (clip.y > clip.w) clipOutcode |= 0x08
        if (clip.z < -clip.w) clipOutcode |= 0x10
        if (clip.z > clip.w) clipOutcode |= 0x20
      }
      vertices.push({
        x_px: (ndcX * 0.5 + 0.5) * rig.frame_width,
        y_px: (-ndcY * 0.5 + 0.5) * rig.frame_height,
        depth_ndc: ndcZ,
        clip_visible: clipOutcode === 0,
        clip_outcode: clipOutcode,
      })
    }

    const index = part.geometry.getIndex()
    const indexCount = index ? index.count : position.count
    if (indexCount % 3 !== 0) throw new KnifeViewEvaluationError(`part ${part.part_id} index count is not divisible by three`)
    for (let offset = 0; offset < indexCount; offset += 3) {
      const a = firstVertex + (index ? index.getX(offset) : offset)
      const b = firstVertex + (index ? index.getX(offset + 1) : offset + 1)
      const c = firstVertex + (index ? index.getX(offset + 2) : offset + 2)
      if (a >= vertices.length || b >= vertices.length || c >= vertices.length) {
        throw new KnifeViewEvaluationError(`part ${part.part_id} has an out-of-range triangle index`)
      }
      triangles.push({
        a,
        b,
        c,
        part_index: partIndex,
        material_index: materialIndexById.get(part.material_zone_id) as number,
        part_id: part.part_id,
        material_zone_id: part.material_zone_id,
      })
    }
  }

  const projectionFingerprint = hashProjection(rig, view, compiled.deterministic_fingerprint, vertices, triangles)
  const receipt: KnifeProjectionReceipt = {
    schema_version: 'WeaponryThreeJsProjectionReceipt@1',
    rig_schema_version: rig.schema_version,
    rig_fingerprint: rig.deterministic_fingerprint,
    source_fingerprint: compiled.deterministic_fingerprint,
    view_id: view.view_id,
    frame_width: rig.frame_width,
    frame_height: rig.frame_height,
    projection_type: view.projection,
    projected_vertex_count: vertices.length,
    projected_triangle_count: triangles.length,
    clip_visible_vertex_count: vertices.filter((vertex) => vertex.clip_visible).length,
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
    deterministic_fingerprint: projectionFingerprint,
  }
  return {
    schema_version: 'WeaponryThreeJsProjection@1',
    rig,
    view,
    part_ids: partIds,
    material_zone_ids: materialZoneIds,
    vertices,
    triangles,
    receipt,
  }
}

export function rasterizeKnifeMask(projection: KnifeProjectionResult): KnifeMaskResult {
  const width = projection.rig.frame_width
  const height = projection.rig.frame_height
  const pixelCount = width * height
  const pixels = new Uint8Array(pixelCount)
  const partIndices = new Uint16Array(pixelCount)
  const materialIndices = new Uint16Array(pixelCount)
  const depth = new Float32Array(pixelCount)
  partIndices.fill(0xffff)
  materialIndices.fill(0xffff)
  depth.fill(Number.POSITIVE_INFINITY)

  for (const triangle of projection.triangles) {
    const a = projection.vertices[triangle.a]
    const b = projection.vertices[triangle.b]
    const c = projection.vertices[triangle.c]
    // Reject only when all three vertices lie beyond the same clip plane.
    // Vertices may all be outside different planes while their triangle still
    // crosses the viewport, so an "all invisible" test would be incorrect.
    if ((a.clip_outcode & b.clip_outcode & c.clip_outcode) !== 0) continue
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
        const barycentricScale = 1 / area
        const candidateDepth = (w0 * a.depth_ndc + w1 * b.depth_ndc + w2 * c.depth_ndc) * barycentricScale
        if (!Number.isFinite(candidateDepth) || candidateDepth < -1 || candidateDepth > 1) continue
        const pixelIndex = y * width + x
        if (candidateDepth < depth[pixelIndex] - 1e-7) {
          depth[pixelIndex] = candidateDepth
          pixels[pixelIndex] = 255
          partIndices[pixelIndex] = triangle.part_index
          materialIndices[pixelIndex] = triangle.material_index
        }
      }
    }
  }

  const coveredPixelCount = pixels.reduce((count, pixel) => count + (pixel === 0 ? 0 : 1), 0)
  const maskFingerprint = hashMask(projection.receipt.deterministic_fingerprint, pixels, partIndices, materialIndices, depth)
  const receipt: KnifeMaskReceipt = {
    schema_version: 'WeaponryThreeJsMaskReceipt@1',
    projection_fingerprint: projection.receipt.deterministic_fingerprint,
    view_id: projection.view.view_id,
    frame_width: width,
    frame_height: height,
    rasterizer: 'software-triangle-mask@2',
    anti_aliasing: 'none',
    covered_pixel_count: coveredPixelCount,
    coverage_ratio: coveredPixelCount / pixelCount,
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
    deterministic_fingerprint: maskFingerprint,
  }
  return {
    schema_version: 'WeaponryThreeJsMask@1',
    width,
    height,
    pixels,
    part_indices: partIndices,
    material_indices: materialIndices,
    depth,
    receipt,
  }
}

export const generateKnifeMask = rasterizeKnifeMask

export function evaluateKnifeView(compiled: CompiledKnifeScene, rig: KnifeViewRig, viewId: KnifeViewId): KnifeViewEvaluation {
  const projection = projectKnifeScene(compiled, rig, viewId)
  const mask = rasterizeKnifeMask(projection)
  return {
    view_id: viewId,
    projection,
    mask,
    receipt: {
      schema_version: 'WeaponryThreeJsViewEvaluationReceipt@1',
      rig_id: rig.rig_id,
      rig_fingerprint: rig.deterministic_fingerprint,
      source_fingerprint: compiled.deterministic_fingerprint,
      view_id: viewId,
      projection_fingerprint: projection.receipt.deterministic_fingerprint,
      mask_fingerprint: mask.receipt.deterministic_fingerprint,
      renderer_invoked: false,
      quality_status: 'NOT_RUN',
    },
  }
}

export function evaluateKnifeRig(compiled: CompiledKnifeScene, rig: KnifeViewRig = createKnifeViewRig()): KnifeEightViewEvaluation {
  validateRig(rig)
  const views = KNIFE_VIEW_IDS.map((viewId) => evaluateKnifeView(compiled, rig, viewId))
  return {
    rig,
    views,
    receipt: {
      schema_version: 'WeaponryThreeJsEightViewEvaluationReceipt@1',
      rig_id: rig.rig_id,
      rig_fingerprint: rig.deterministic_fingerprint,
      source_fingerprint: compiled.deterministic_fingerprint,
      view_ids: [...KNIFE_VIEW_IDS],
      renderer_invoked: false,
      quality_status: 'NOT_RUN',
      deterministic_fingerprint: fnv1a64(views.map((view) => `${view.view_id}:${view.receipt.projection_fingerprint}:${view.receipt.mask_fingerprint}`).join('|')),
    },
  }
}

function orthographicView(viewId: Exclude<KnifeViewId, 'FPS_HOLD'>, position: KnifeVec3, target: KnifeVec3, up: KnifeVec3): KnifeViewDescriptor {
  return {
    view_id: viewId,
    projection: 'orthographic',
    position,
    target,
    up,
    near: 0.01,
    far: 50,
    ortho_height: 2.8,
  }
}

function perspectiveView(viewId: 'FPS_HOLD', position: KnifeVec3, target: KnifeVec3, up: KnifeVec3): KnifeViewDescriptor {
  return {
    view_id: viewId,
    projection: 'perspective',
    position,
    target,
    up,
    near: 0.05,
    far: 20,
    fov_degrees: 36,
  }
}

function validateRig(rig: KnifeViewRig): void {
  if (!rig || rig.schema_version !== 'KnifeFixedEightViewRig@1' || rig.rig_id !== 'knife-fixed-eight-view@1') {
    throw new KnifeViewEvaluationError('unsupported fixed-view rig')
  }
  if ((rig.calibration === undefined) !== (rig.calibration_receipt === undefined)) {
    throw new KnifeViewEvaluationError('calibrated rig must carry both calibration and calibration receipt')
  }
  if (rig.calibration !== undefined) {
    validateKnifeViewCalibration(rig.calibration)
    if (rig.calibration_receipt?.schema_version !== KNIFE_VIEW_CALIBRATION_RECEIPT_SCHEMA
      || rig.calibration_receipt.calibration_fingerprint !== rig.calibration.deterministic_fingerprint
      || rig.deterministic_fingerprint !== hashRig(rig)) {
      throw new KnifeViewEvaluationError('calibrated rig fingerprint or receipt binding is invalid')
    }
  }
  if (rig.views.length !== KNIFE_VIEW_IDS.length || rig.views.some((view, index) => view.view_id !== KNIFE_VIEW_IDS[index])) {
    throw new KnifeViewEvaluationError('rig must contain the canonical eight views in order')
  }
  if (!Number.isInteger(rig.frame_width) || !Number.isInteger(rig.frame_height) || rig.frame_width <= 0 || rig.frame_height <= 0) {
    throw new KnifeViewEvaluationError('rig frame dimensions must be positive integers')
  }
  for (const view of rig.views) {
    for (const vector of [view.position, view.target, view.up]) {
      for (const value of vector) {
        if (typeof value !== 'number' || !Number.isFinite(value)) {
          throw new KnifeViewEvaluationError(`view ${view.view_id} has a non-finite camera vector`)
        }
      }
    }
    if (!Number.isFinite(view.near) || !Number.isFinite(view.far) || view.near <= 0 || view.far <= view.near) {
      throw new KnifeViewEvaluationError(`view ${view.view_id} has an invalid clip range`)
    }
    if (view.projection === 'orthographic' && (!Number.isFinite(view.ortho_height) || (view.ortho_height as number) <= 0)) {
      throw new KnifeViewEvaluationError(`view ${view.view_id} has an invalid orthographic height`)
    }
    if (view.projection === 'perspective' && (!Number.isFinite(view.fov_degrees) || (view.fov_degrees as number) <= 0 || (view.fov_degrees as number) >= 179)) {
      throw new KnifeViewEvaluationError(`view ${view.view_id} has an invalid perspective field of view`)
    }
  }
}

function boundedFrameDimension(value: number, name: string): number {
  if (!Number.isInteger(value) || value < 16 || value > 2048) throw new KnifeViewEvaluationError(`${name} must be an integer in [16, 2048]`)
  return value
}

function edgeFunction(ax: number, ay: number, bx: number, by: number, px: number, py: number): number {
  return (px - ax) * (by - ay) - (py - ay) * (bx - ax)
}

function hashRig(rig: Pick<KnifeViewRig, 'schema_version' | 'rig_id' | 'coordinate_convention' | 'frame_width' | 'frame_height' | 'margin' | 'views' | 'calibration'>): string {
  const values = [rig.schema_version, rig.rig_id, rig.coordinate_convention, `${rig.frame_width}x${rig.frame_height}`, canonicalNumber(rig.margin)]
  if (rig.calibration !== undefined) values.push('calibration', rig.calibration.deterministic_fingerprint)
  for (const view of rig.views) values.push(view.view_id, view.projection, view.position.join(','), view.target.join(','), view.up.join(','), canonicalNumber(view.near), canonicalNumber(view.far), canonicalNumber(view.ortho_height ?? 0), canonicalNumber(view.fov_degrees ?? 0))
  return fnv1a64(values.join('|'))
}

function hashProjection(
  rig: KnifeViewRig,
  view: KnifeViewDescriptor,
  sourceFingerprint: string,
  vertices: readonly KnifeProjectedVertex[],
  triangles: readonly KnifeProjectedTriangle[],
): string {
  const values = [rig.deterministic_fingerprint, sourceFingerprint, view.view_id]
  for (const vertex of vertices) values.push(
    canonicalNumber(vertex.x_px),
    canonicalNumber(vertex.y_px),
    canonicalNumber(vertex.depth_ndc),
    vertex.clip_visible ? '1' : '0',
    String(vertex.clip_outcode),
  )
  for (const triangle of triangles) values.push(`${triangle.a},${triangle.b},${triangle.c}`, `${triangle.part_index}:${triangle.material_index}`)
  return fnv1a64(values.join('|'))
}

function hashMask(projectionFingerprint: string, pixels: Uint8Array, parts: Uint16Array, materials: Uint16Array, depths: Float32Array): string {
  let value = `${projectionFingerprint}|`
  for (let index = 0; index < pixels.length; index += 1) value += `${pixels[index]},${parts[index]},${materials[index]},${canonicalNumber(depths[index])};`
  return fnv1a64(value)
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

function stableUuid(value: string): string {
  const raw = `${fnv1a64(`${value}:0`)}${fnv1a64(`${value}:1`)}${fnv1a64(`${value}:2`)}${fnv1a64(`${value}:3`)}`
  return `${raw.slice(0, 8)}-${raw.slice(8, 12)}-${raw.slice(12, 16)}-${raw.slice(16, 20)}-${raw.slice(20, 32)}`
}

function overrideUuid(object: { readonly uuid: string }, uuid: string): void {
  Object.defineProperty(object, 'uuid', { configurable: true, enumerable: true, value: uuid, writable: true })
}
