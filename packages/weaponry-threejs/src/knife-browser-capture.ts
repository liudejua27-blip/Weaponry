import * as THREE from 'three'

import {
  KNIFE_VIEW_IDS,
  createKnifeViewCamera,
  createKnifeViewRig,
  getKnifeView,
  type KnifeViewId,
  type KnifeViewRig,
} from './knife-view-evaluation.ts'
import type { CompiledKnifePart, CompiledKnifeScene } from './knife-scene-compiler.ts'

/**
 * Browser-only capture contract for the knife route.
 *
 * The fixed-view software evaluator intentionally remains separate from this
 * module.  This module crosses the browser/WebGL boundary only when the
 * caller supplies a live WebGLRenderer and canvas.  It records PNG evidence
 * and binding data; it never decides visual, human, engine, or commercial
 * acceptance.
 */

export const KNIFE_CAPTURE_AOV_IDS = [
  'beauty',
  'silhouette',
  'depth',
  'normal',
  'part-id',
  'material-id',
  'wireframe',
] as const

export const KNIFE_OPTIONAL_CAPTURE_AOV_IDS = ['curvature', 'uv-stretch'] as const
export type KnifeCaptureAovId = (typeof KNIFE_CAPTURE_AOV_IDS | typeof KNIFE_OPTIONAL_CAPTURE_AOV_IDS)[number]
export type KnifeCaptureQualityStatus = 'NOT_RUN' | 'RENDERED_NOT_APPROVED'

const REQUIRED_VIEW_IDS = [...KNIFE_VIEW_IDS] as readonly KnifeViewId[]
const REQUIRED_AOV_IDS = [...KNIFE_CAPTURE_AOV_IDS] as readonly KnifeCaptureAovId[]
const OPTIONAL_AOV_IDS = [...KNIFE_OPTIONAL_CAPTURE_AOV_IDS] as readonly KnifeCaptureAovId[]
const SHA256_PATTERN = /^[a-f0-9]{64}$/
const FINGERPRINT_PATTERN = /^[a-f0-9]{16,128}$/
const ID_PATTERN = /^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/
const MAX_FRAME_DIMENSION = 2048
const MAX_PNG_BYTES = 64 * 1024 * 1024

export interface KnifeCameraMatrixBinding {
  readonly view_id: KnifeViewId
  readonly projection: 'orthographic' | 'perspective'
  readonly matrix_world: readonly number[]
  readonly matrix_world_inverse: readonly number[]
  readonly projection_matrix: readonly number[]
  readonly camera_fingerprint: string
}

export interface KnifePngCapture {
  readonly aov_id: KnifeCaptureAovId
  readonly mime_type: 'image/png'
  readonly width: number
  readonly height: number
  readonly png_sha256: string
  readonly png_size_bytes: number
}

export interface KnifeCaptureView {
  readonly view_id: KnifeViewId
  readonly camera: KnifeCameraMatrixBinding
  readonly aovs: readonly KnifePngCapture[]
}

/** Closed input to createKnifeCaptureManifest. */
export interface KnifeCaptureManifestDraft {
  readonly manifest_id: string
  readonly rig_id: 'knife-fixed-eight-view@1'
  readonly rig_fingerprint: string
  readonly rig_margin: number
  readonly program_fingerprint: string
  readonly scene_fingerprint: string
  readonly frame_width: number
  readonly frame_height: number
  readonly views: readonly KnifeCaptureView[]
}

export interface KnifeCaptureManifest {
  readonly schema_version: 'WeaponryThreeJsCaptureManifest@1'
  readonly manifest_id: string
  readonly rig_id: 'knife-fixed-eight-view@1'
  readonly rig_fingerprint: string
  readonly rig_margin: number
  readonly program_fingerprint: string
  readonly scene_fingerprint: string
  readonly frame_width: number
  readonly frame_height: number
  readonly view_ids: readonly KnifeViewId[]
  readonly aov_ids: readonly KnifeCaptureAovId[]
  readonly views: readonly KnifeCaptureView[]
  readonly renderer: 'browser-webgl@1'
  readonly capture_mode: 'browser-canvas-to-png@1'
  readonly renderer_invoked: true
  readonly render_status: 'RENDERED'
  readonly quality_status: 'RENDERED_NOT_APPROVED'
  readonly visual_status: 'NOT_RUN'
  readonly human_status: 'NOT_RUN'
  readonly engine_status: 'NOT_RUN'
  readonly commercial_status: 'NOT_RUN'
  readonly canonical_sha256: string
}

export interface KnifeBrowserCaptureReceipt {
  readonly schema_version: 'WeaponryThreeJsBrowserCaptureReceipt@1'
  readonly manifest_sha256: string
  readonly manifest_id: string
  readonly rig_fingerprint: string
  readonly rig_margin: number
  readonly program_fingerprint: string
  readonly scene_fingerprint: string
  readonly expected_view_count: 8
  readonly captured_view_count: number
  readonly expected_aov_count_per_view: 7
  readonly captured_aov_count: number
  readonly missing_capture_count: 0
  readonly renderer_invoked: true
  readonly render_status: 'RENDERED'
  readonly quality_status: 'RENDERED_NOT_APPROVED'
  readonly visual_status: 'NOT_RUN'
  readonly human_status: 'NOT_RUN'
  readonly engine_status: 'NOT_RUN'
  readonly commercial_status: 'NOT_RUN'
  readonly canonical_sha256: string
}

export interface KnifeBrowserCaptureResult {
  readonly manifest: KnifeCaptureManifest
  readonly receipt: KnifeBrowserCaptureReceipt
}

export interface KnifeBrowserCaptureOptions {
  readonly renderer: THREE.WebGLRenderer
  readonly scene: THREE.Scene
  readonly compiled: CompiledKnifeScene
  readonly rig?: KnifeViewRig
  readonly manifest_id?: string
  readonly clear_color?: THREE.ColorRepresentation
  /**
   * Optional byte sink owned by the caller. The immutable manifest stores only
   * hashes and sizes; a browser exporter can persist the exact bytes without
   * making filesystem or download behavior part of the compiler.
   */
  readonly capture_sink?: (viewId: KnifeViewId, aovId: KnifeCaptureAovId, pngBytes: Uint8Array) => void
}

export class KnifeBrowserCaptureError extends Error {
  constructor(message: string) {
    super(`KNIFE_BROWSER_CAPTURE_INVALID: ${message}`)
    this.name = 'KnifeBrowserCaptureError'
  }
}

/** Materialize the exact camera matrix binding used by one capture view. */
export function createKnifeCameraMatrixBinding(rig: KnifeViewRig, viewId: KnifeViewId): KnifeCameraMatrixBinding {
  const view = getKnifeView(rig, viewId)
  return createCameraBinding(viewId, view.projection, createKnifeViewCamera(rig, viewId))
}

/**
 * Capture the exact eight views and seven required AOVs from a live browser
 * WebGLRenderer.  The caller owns the renderer/scene lifecycle; this method
 * only performs bounded render calls and restores renderer/material state.
 */
export function captureKnifeAovs(options: KnifeBrowserCaptureOptions): KnifeBrowserCaptureResult {
  if (!options || typeof options !== 'object') throw new KnifeBrowserCaptureError('capture options are required')
  const rig = options.rig
  if (!rig) throw new KnifeBrowserCaptureError('a fixed eight-view rig is required')
  validateCaptureRuntime(options.renderer, rig)
  validateSceneBinding(options.scene, options.compiled)
  options.scene.updateMatrixWorld(true)
  options.compiled.group.updateMatrixWorld(true)

  const renderer = options.renderer
  const frameWidth = renderer.domElement.width
  const frameHeight = renderer.domElement.height
  const programFingerprint = boundedFingerprint(options.compiled.deterministic_fingerprint, 'compiled program fingerprint')
  const sceneFingerprint = fingerprintScene(options.scene, options.compiled)
  const manifestViews: KnifeCaptureView[] = []
  const oldClearColor = renderer.getClearColor(new THREE.Color()).clone()
  const oldClearAlpha = renderer.getClearAlpha()
  const oldToneMapping = renderer.toneMapping
  const oldOutputColorSpace = renderer.outputColorSpace
  const oldBackground = options.scene.background
  const oldRenderTarget = renderer.getRenderTarget()
  const oldViewport = renderer.getViewport(new THREE.Vector4()).clone()
  const oldScissor = renderer.getScissor(new THREE.Vector4()).clone()
  const oldScissorTest = renderer.getScissorTest()
  const clearColor = options.clear_color ?? 0x000000

  try {
    renderer.setRenderTarget(null)
    renderer.setViewport(0, 0, frameWidth, frameHeight)
    renderer.setScissorTest(false)
    for (const viewId of REQUIRED_VIEW_IDS) {
      const view = getKnifeView(rig, viewId)
      const camera = createKnifeViewCamera(rig, viewId)
      camera.updateMatrixWorld(true)
      const cameraBinding = createCameraBinding(viewId, view.projection, camera)
      const captures: KnifePngCapture[] = []
      for (const aovId of REQUIRED_AOV_IDS) {
        const pngBytes = renderAovToPng(renderer, options.scene, options.compiled, camera, aovId, clearColor)
        options.capture_sink?.(viewId, aovId, pngBytes.slice())
        captures.push({
          aov_id: aovId,
          mime_type: 'image/png',
          width: frameWidth,
          height: frameHeight,
          png_sha256: sha256Hex(pngBytes),
          png_size_bytes: pngBytes.byteLength,
        })
      }
      manifestViews.push({ view_id: viewId, camera: cameraBinding, aovs: captures })
    }
  } finally {
    renderer.setRenderTarget(oldRenderTarget)
    renderer.setViewport(oldViewport)
    renderer.setScissor(oldScissor)
    renderer.setScissorTest(oldScissorTest)
    renderer.setClearColor(oldClearColor, oldClearAlpha)
    renderer.toneMapping = oldToneMapping
    renderer.outputColorSpace = oldOutputColorSpace
    options.scene.background = oldBackground
  }

  const manifestId = options.manifest_id ?? `capture-${sha256Hex(canonicalJson({ rig: rig.deterministic_fingerprint, program: programFingerprint, scene: sceneFingerprint })).slice(0, 24)}`
  return createKnifeCaptureManifest({
    manifest_id: manifestId,
    rig_id: rig.rig_id,
    rig_fingerprint: rig.deterministic_fingerprint,
    rig_margin: rig.margin,
    program_fingerprint: programFingerprint,
    scene_fingerprint: sceneFingerprint,
    frame_width: frameWidth,
    frame_height: frameHeight,
    views: manifestViews,
  }, rig)
}

/** Build and validate a closed manifest from browser-produced PNG records. */
export function createKnifeCaptureManifest(
  draft: KnifeCaptureManifestDraft,
  expectedRig?: KnifeViewRig,
): KnifeBrowserCaptureResult {
  validateCaptureDraft(draft, expectedRig)
  const manifest: KnifeCaptureManifest = {
    schema_version: 'WeaponryThreeJsCaptureManifest@1',
    manifest_id: draft.manifest_id,
    rig_id: draft.rig_id,
    rig_fingerprint: draft.rig_fingerprint,
    rig_margin: draft.rig_margin,
    program_fingerprint: draft.program_fingerprint,
    scene_fingerprint: draft.scene_fingerprint,
    frame_width: draft.frame_width,
    frame_height: draft.frame_height,
    view_ids: [...REQUIRED_VIEW_IDS],
    aov_ids: [...draft.views[0].aovs.map((aov) => aov.aov_id)],
    views: draft.views.map((view) => ({
      view_id: view.view_id,
      camera: {
        view_id: view.camera.view_id,
        projection: view.camera.projection,
        matrix_world: [...view.camera.matrix_world],
        matrix_world_inverse: [...view.camera.matrix_world_inverse],
        projection_matrix: [...view.camera.projection_matrix],
        camera_fingerprint: view.camera.camera_fingerprint,
      },
      aovs: view.aovs.map((aov) => ({ ...aov })),
    })),
    renderer: 'browser-webgl@1',
    capture_mode: 'browser-canvas-to-png@1',
    renderer_invoked: true,
    render_status: 'RENDERED',
    quality_status: 'RENDERED_NOT_APPROVED',
    visual_status: 'NOT_RUN',
    human_status: 'NOT_RUN',
    engine_status: 'NOT_RUN',
    commercial_status: 'NOT_RUN',
    canonical_sha256: '',
  }
  const manifestWithHash = freezeDeep({ ...manifest, canonical_sha256: canonicalSha256(manifest) }) as KnifeCaptureManifest
  const receipt = createKnifeCaptureReceipt(manifestWithHash, expectedRig)
  return { manifest: manifestWithHash, receipt }
}

/** Verify a complete immutable browser capture manifest. */
export function validateKnifeCaptureManifest(manifest: KnifeCaptureManifest, expectedRig?: KnifeViewRig): string {
  exactKeys(manifest, [
    'schema_version',
    'manifest_id',
    'rig_id',
    'rig_fingerprint',
    'rig_margin',
    'program_fingerprint',
    'scene_fingerprint',
    'frame_width',
    'frame_height',
    'view_ids',
    'aov_ids',
    'views',
    'renderer',
    'capture_mode',
    'renderer_invoked',
    'render_status',
    'quality_status',
    'visual_status',
    'human_status',
    'engine_status',
    'commercial_status',
    'canonical_sha256',
  ], 'capture manifest')
  if (manifest.schema_version !== 'WeaponryThreeJsCaptureManifest@1') throw new KnifeBrowserCaptureError('unsupported manifest schema')
  if (!isBoundedId(manifest.manifest_id)) throw new KnifeBrowserCaptureError('manifest_id is not bounded')
  if (manifest.rig_id !== 'knife-fixed-eight-view@1') throw new KnifeBrowserCaptureError('rig_id is not the canonical fixed rig')
  boundedFingerprint(manifest.rig_fingerprint, 'manifest rig fingerprint')
  validateMargin(manifest.rig_margin, 'manifest rig margin')
  boundedFingerprint(manifest.program_fingerprint, 'manifest program fingerprint')
  boundedFingerprint(manifest.scene_fingerprint, 'manifest scene fingerprint')
  validateFrame(manifest.frame_width, manifest.frame_height)
  const boundRig = resolveExpectedCaptureRig(
    manifest.frame_width,
    manifest.frame_height,
    manifest.rig_margin,
    expectedRig,
  )
  if (manifest.rig_fingerprint !== boundRig.deterministic_fingerprint) throw new KnifeBrowserCaptureError('manifest rig fingerprint does not match the fixed camera rig')
  if (!Array.isArray(manifest.view_ids) || !Array.isArray(manifest.aov_ids) || !Array.isArray(manifest.views)) {
    throw new KnifeBrowserCaptureError('manifest view_ids, aov_ids, and views must be arrays')
  }
  if (!sameSequence(manifest.view_ids, REQUIRED_VIEW_IDS) || !validAovOrder(manifest.aov_ids)) {
    throw new KnifeBrowserCaptureError('manifest view or AOV order is not the closed required set')
  }
  const manifestAovIds = manifest.aov_ids
  if (manifest.views.length !== REQUIRED_VIEW_IDS.length) throw new KnifeBrowserCaptureError('manifest does not contain exactly eight views')
  for (let index = 0; index < manifest.views.length; index += 1) {
    validateCaptureView(manifest.views[index], REQUIRED_VIEW_IDS[index], manifest.frame_width, manifest.frame_height, manifestAovIds)
    const expectedCamera = createCameraBinding(REQUIRED_VIEW_IDS[index], getKnifeView(boundRig, REQUIRED_VIEW_IDS[index]).projection, createKnifeViewCamera(boundRig, REQUIRED_VIEW_IDS[index]))
    if (!sameMatrix(manifest.views[index].camera.matrix_world, expectedCamera.matrix_world) || !sameMatrix(manifest.views[index].camera.matrix_world_inverse, expectedCamera.matrix_world_inverse) || !sameMatrix(manifest.views[index].camera.projection_matrix, expectedCamera.projection_matrix) || manifest.views[index].camera.camera_fingerprint !== expectedCamera.camera_fingerprint) {
      throw new KnifeBrowserCaptureError(`camera fingerprint mismatch for ${manifest.views[index].view_id}`)
    }
  }
  if (manifest.renderer !== 'browser-webgl@1' || manifest.capture_mode !== 'browser-canvas-to-png@1') {
    throw new KnifeBrowserCaptureError('manifest does not prove browser canvas capture')
  }
  if (manifest.renderer_invoked !== true) throw new KnifeBrowserCaptureError('renderer_invoked must be true for a complete manifest')
  if (manifest.render_status !== 'RENDERED') throw new KnifeBrowserCaptureError('render status must be RENDERED for a complete manifest')
  if (manifest.quality_status !== 'RENDERED_NOT_APPROVED') throw new KnifeBrowserCaptureError('quality status cannot exceed RENDERED_NOT_APPROVED')
  if (manifest.visual_status !== 'NOT_RUN' || manifest.human_status !== 'NOT_RUN' || manifest.engine_status !== 'NOT_RUN' || manifest.commercial_status !== 'NOT_RUN') {
    throw new KnifeBrowserCaptureError('visual, human, engine, and commercial statuses must remain NOT_RUN')
  }
  if (!SHA256_PATTERN.test(manifest.canonical_sha256) || manifest.canonical_sha256 !== canonicalSha256(manifest)) {
    throw new KnifeBrowserCaptureError('manifest canonical SHA-256 does not match')
  }
  return manifest.canonical_sha256
}

export const validateKnifeBrowserCaptureManifest = validateKnifeCaptureManifest

/** Create the receipt only after the complete manifest has passed validation. */
export function createKnifeCaptureReceipt(
  manifest: KnifeCaptureManifest,
  expectedRig?: KnifeViewRig,
): KnifeBrowserCaptureReceipt {
  const manifestSha = validateKnifeCaptureManifest(manifest, expectedRig)
  const receipt: KnifeBrowserCaptureReceipt = {
    schema_version: 'WeaponryThreeJsBrowserCaptureReceipt@1',
    manifest_sha256: manifestSha,
    manifest_id: manifest.manifest_id,
    rig_fingerprint: manifest.rig_fingerprint,
    rig_margin: manifest.rig_margin,
    program_fingerprint: manifest.program_fingerprint,
    scene_fingerprint: manifest.scene_fingerprint,
    expected_view_count: 8,
    captured_view_count: manifest.views.length,
    expected_aov_count_per_view: 7,
    captured_aov_count: manifest.views.reduce((count, view) => count + view.aovs.length, 0),
    missing_capture_count: 0,
    renderer_invoked: true,
    render_status: 'RENDERED',
    quality_status: 'RENDERED_NOT_APPROVED',
    visual_status: 'NOT_RUN',
    human_status: 'NOT_RUN',
    engine_status: 'NOT_RUN',
    commercial_status: 'NOT_RUN',
    canonical_sha256: '',
  }
  return freezeDeep({ ...receipt, canonical_sha256: canonicalSha256(receipt) }) as KnifeBrowserCaptureReceipt
}

export function validateKnifeCaptureReceipt(receipt: KnifeBrowserCaptureReceipt): string {
  exactKeys(receipt, [
    'schema_version',
    'manifest_sha256',
    'manifest_id',
    'rig_fingerprint',
    'rig_margin',
    'program_fingerprint',
    'scene_fingerprint',
    'expected_view_count',
    'captured_view_count',
    'expected_aov_count_per_view',
    'captured_aov_count',
    'missing_capture_count',
    'renderer_invoked',
    'render_status',
    'quality_status',
    'visual_status',
    'human_status',
    'engine_status',
    'commercial_status',
    'canonical_sha256',
  ], 'capture receipt')
  if (receipt.schema_version !== 'WeaponryThreeJsBrowserCaptureReceipt@1') throw new KnifeBrowserCaptureError('unsupported capture receipt schema')
  if (!SHA256_PATTERN.test(receipt.manifest_sha256)) throw new KnifeBrowserCaptureError('receipt manifest SHA-256 is invalid')
  if (!isBoundedId(receipt.manifest_id)) throw new KnifeBrowserCaptureError('receipt manifest_id is invalid')
  boundedFingerprint(receipt.rig_fingerprint, 'receipt rig fingerprint')
  validateMargin(receipt.rig_margin, 'receipt rig margin')
  boundedFingerprint(receipt.program_fingerprint, 'receipt program fingerprint')
  boundedFingerprint(receipt.scene_fingerprint, 'receipt scene fingerprint')
  if (receipt.expected_view_count !== 8 || receipt.captured_view_count !== 8 || receipt.expected_aov_count_per_view !== 7 || receipt.captured_aov_count < 56 || receipt.captured_aov_count > 72 || receipt.missing_capture_count !== 0 || receipt.captured_aov_count % 8 !== 0) {
    throw new KnifeBrowserCaptureError('receipt counts do not prove complete 8x7 capture')
  }
  if (receipt.renderer_invoked !== true || receipt.render_status !== 'RENDERED' || receipt.quality_status !== 'RENDERED_NOT_APPROVED') throw new KnifeBrowserCaptureError('receipt status crossed the render approval boundary')
  if (receipt.visual_status !== 'NOT_RUN' || receipt.human_status !== 'NOT_RUN' || receipt.engine_status !== 'NOT_RUN' || receipt.commercial_status !== 'NOT_RUN') {
    throw new KnifeBrowserCaptureError('receipt non-render statuses must remain NOT_RUN')
  }
  if (!SHA256_PATTERN.test(receipt.canonical_sha256) || receipt.canonical_sha256 !== canonicalSha256(receipt)) {
    throw new KnifeBrowserCaptureError('receipt canonical SHA-256 does not match')
  }
  return receipt.canonical_sha256
}

function validateCaptureRuntime(renderer: THREE.WebGLRenderer, rig: KnifeViewRig): void {
  if (!renderer || !renderer.domElement || typeof renderer.domElement.toDataURL !== 'function') {
    throw new KnifeBrowserCaptureError('a live browser canvas renderer is required')
  }
  const context = renderer.getContext()
  if (!context || typeof context.getContextAttributes !== 'function' || context.getContextAttributes()?.preserveDrawingBuffer !== true) {
    throw new KnifeBrowserCaptureError('renderer must use preserveDrawingBuffer=true for PNG capture')
  }
  validateFrame(renderer.domElement.width, renderer.domElement.height)
  if (renderer.domElement.width !== rig.frame_width || renderer.domElement.height !== rig.frame_height) {
    throw new KnifeBrowserCaptureError('renderer canvas dimensions do not match the fixed rig')
  }
  if (rig.views.length !== REQUIRED_VIEW_IDS.length || rig.views.some((view, index) => view.view_id !== REQUIRED_VIEW_IDS[index])) {
    throw new KnifeBrowserCaptureError('rig must contain the canonical eight views in order')
  }
}

function validateSceneBinding(scene: THREE.Scene, compiled: CompiledKnifeScene): void {
  if (!scene || !(scene instanceof THREE.Scene)) throw new KnifeBrowserCaptureError('a live THREE.Scene is required')
  if (!compiled || !compiled.group || !compiled.parts || compiled.parts.length === 0) {
    throw new KnifeBrowserCaptureError('a compiled knife scene with renderable parts is required')
  }
  let groupFound = false
  scene.traverse((object) => {
    if (object === compiled.group) groupFound = true
  })
  if (!groupFound) throw new KnifeBrowserCaptureError('compiled scene group is not attached to the render scene')
  for (const part of compiled.parts) {
    if (!part.mesh || part.mesh.parent !== compiled.group || part.geometry !== part.mesh.geometry) {
      throw new KnifeBrowserCaptureError(`compiled part ${part.part_id} is not bound to the render scene`)
    }
  }
}

function validateCaptureDraft(draft: KnifeCaptureManifestDraft, expectedRig?: KnifeViewRig): void {
  exactKeys(draft, [
    'manifest_id',
    'rig_id',
    'rig_fingerprint',
    'rig_margin',
    'program_fingerprint',
    'scene_fingerprint',
    'frame_width',
    'frame_height',
    'views',
  ], 'capture manifest draft')
  if (!isBoundedId(draft.manifest_id)) throw new KnifeBrowserCaptureError('draft manifest_id is invalid')
  if (draft.rig_id !== 'knife-fixed-eight-view@1') throw new KnifeBrowserCaptureError('draft rig_id is invalid')
  boundedFingerprint(draft.rig_fingerprint, 'draft rig fingerprint')
  validateMargin(draft.rig_margin, 'draft rig margin')
  boundedFingerprint(draft.program_fingerprint, 'draft program fingerprint')
  boundedFingerprint(draft.scene_fingerprint, 'draft scene fingerprint')
  validateFrame(draft.frame_width, draft.frame_height)
  const boundRig = resolveExpectedCaptureRig(
    draft.frame_width,
    draft.frame_height,
    draft.rig_margin,
    expectedRig,
  )
  if (draft.rig_fingerprint !== boundRig.deterministic_fingerprint) throw new KnifeBrowserCaptureError('draft rig fingerprint does not match the fixed camera rig')
  if (!Array.isArray(draft.views) || draft.views.length !== REQUIRED_VIEW_IDS.length) throw new KnifeBrowserCaptureError('draft must contain exactly eight views')
  const firstView = draft.views[0]
  if (!firstView || !Array.isArray(firstView.aovs)) throw new KnifeBrowserCaptureError('draft first view must contain AOVs')
  const draftAovIds: KnifeCaptureAovId[] = []
  for (let index = 0; index < firstView.aovs.length; index += 1) {
    const aov = firstView.aovs[index]
    exactKeys(aov, ['aov_id', 'mime_type', 'width', 'height', 'png_sha256', 'png_size_bytes'], `draft AOV ${index}`)
    draftAovIds.push((aov as KnifePngCapture).aov_id)
  }
  if (!validAovOrder(draftAovIds)) throw new KnifeBrowserCaptureError('draft AOV order does not contain the required seven AOVs')
  for (let index = 0; index < draft.views.length; index += 1) {
    validateCaptureView(draft.views[index], REQUIRED_VIEW_IDS[index], draft.frame_width, draft.frame_height, draftAovIds)
    const expectedCamera = createCameraBinding(REQUIRED_VIEW_IDS[index], getKnifeView(boundRig, REQUIRED_VIEW_IDS[index]).projection, createKnifeViewCamera(boundRig, REQUIRED_VIEW_IDS[index]))
    if (!sameMatrix(draft.views[index].camera.matrix_world, expectedCamera.matrix_world) || !sameMatrix(draft.views[index].camera.matrix_world_inverse, expectedCamera.matrix_world_inverse) || !sameMatrix(draft.views[index].camera.projection_matrix, expectedCamera.projection_matrix) || draft.views[index].camera.camera_fingerprint !== expectedCamera.camera_fingerprint) {
      throw new KnifeBrowserCaptureError(`draft camera fingerprint mismatch for ${draft.views[index].view_id}`)
    }
  }
}

function validateCaptureView(view: KnifeCaptureView, expectedViewId: KnifeViewId, width: number, height: number, expectedAovIds: readonly KnifeCaptureAovId[] = REQUIRED_AOV_IDS): void {
  exactKeys(view, ['view_id', 'camera', 'aovs'], `capture view ${expectedViewId}`)
  if (view.view_id !== expectedViewId) throw new KnifeBrowserCaptureError(`capture view order is not ${expectedViewId}`)
  exactKeys(view.camera, ['view_id', 'projection', 'matrix_world', 'matrix_world_inverse', 'projection_matrix', 'camera_fingerprint'], `camera ${expectedViewId}`)
  if (view.camera.view_id !== expectedViewId) throw new KnifeBrowserCaptureError(`camera view binding mismatch for ${expectedViewId}`)
  if (view.camera.projection !== 'orthographic' && view.camera.projection !== 'perspective') throw new KnifeBrowserCaptureError(`unsupported projection for ${expectedViewId}`)
  validateMatrix(view.camera.matrix_world, `camera ${expectedViewId}.matrix_world`)
  validateMatrix(view.camera.matrix_world_inverse, `camera ${expectedViewId}.matrix_world_inverse`)
  validateMatrix(view.camera.projection_matrix, `camera ${expectedViewId}.projection_matrix`)
  boundedFingerprint(view.camera.camera_fingerprint, `camera ${expectedViewId} fingerprint`)
  if (!Array.isArray(view.aovs) || view.aovs.length !== expectedAovIds.length) throw new KnifeBrowserCaptureError(`view ${expectedViewId} does not contain the closed AOV set`)
  for (let index = 0; index < view.aovs.length; index += 1) {
    const aov = view.aovs[index]
    exactKeys(aov, ['aov_id', 'mime_type', 'width', 'height', 'png_sha256', 'png_size_bytes'], `AOV ${expectedViewId}/${expectedAovIds[index]}`)
    if (aov.aov_id !== expectedAovIds[index]) throw new KnifeBrowserCaptureError(`AOV order is not closed for ${expectedViewId}`)
    if (aov.mime_type !== 'image/png') throw new KnifeBrowserCaptureError(`AOV ${expectedViewId}/${aov.aov_id} is not a PNG`)
    if (aov.width !== width || aov.height !== height) throw new KnifeBrowserCaptureError(`AOV ${expectedViewId}/${aov.aov_id} dimensions do not match the rig`)
    if (!SHA256_PATTERN.test(aov.png_sha256)) throw new KnifeBrowserCaptureError(`AOV ${expectedViewId}/${aov.aov_id} PNG SHA-256 is invalid`)
    if (!Number.isInteger(aov.png_size_bytes) || aov.png_size_bytes <= 0 || aov.png_size_bytes > MAX_PNG_BYTES) throw new KnifeBrowserCaptureError(`AOV ${expectedViewId}/${aov.aov_id} PNG size is invalid`)
  }
}

function validateFrame(width: number, height: number): void {
  if (!Number.isInteger(width) || !Number.isInteger(height) || width < 16 || height < 16 || width > MAX_FRAME_DIMENSION || height > MAX_FRAME_DIMENSION) {
    throw new KnifeBrowserCaptureError(`frame must be an integer in [16, ${MAX_FRAME_DIMENSION}]`)
  }
}

function validateMargin(value: number, label: string): void {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value >= 0.45) {
    throw new KnifeBrowserCaptureError(`${label} must be finite and in [0, 0.45)`)
  }
}

function createKnifeCaptureRig(width: number, height: number, margin: number): KnifeViewRig {
  try {
    return createKnifeViewRig({ frame_width: width, frame_height: height, margin })
  } catch (error) {
    throw new KnifeBrowserCaptureError(`fixed camera rig cannot be reconstructed: ${error instanceof Error ? error.message : String(error)}`)
  }
}

function resolveExpectedCaptureRig(
  width: number,
  height: number,
  margin: number,
  expectedRig?: KnifeViewRig,
): KnifeViewRig {
  if (expectedRig === undefined) return createKnifeCaptureRig(width, height, margin)
  if (expectedRig.frame_width !== width || expectedRig.frame_height !== height || expectedRig.margin !== margin) {
    throw new KnifeBrowserCaptureError('supplied fixed camera rig does not match capture dimensions or margin')
  }
  if (expectedRig.rig_id !== 'knife-fixed-eight-view@1'
    || expectedRig.views.length !== REQUIRED_VIEW_IDS.length
    || expectedRig.views.some((view, index) => view.view_id !== REQUIRED_VIEW_IDS[index])) {
    throw new KnifeBrowserCaptureError('supplied fixed camera rig does not contain the canonical eight views')
  }
  return expectedRig
}

function validateMatrix(matrix: readonly number[], label: string): void {
  if (!Array.isArray(matrix) || matrix.length !== 16 || matrix.some((value) => typeof value !== 'number' || !Number.isFinite(value))) {
    throw new KnifeBrowserCaptureError(`${label} must be a finite 4x4 matrix`)
  }
}

function sameMatrix(left: readonly number[], right: readonly number[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index] || (value === 0 && right[index] === 0))
}

function exactKeys(value: unknown, keys: readonly string[], label: string): void {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new KnifeBrowserCaptureError(`${label} must be an object`)
  const actual = Object.keys(value).sort()
  const expected = [...keys].sort()
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) throw new KnifeBrowserCaptureError(`${label} keys are not closed`)
}

function isBoundedId(value: unknown): value is string {
  return typeof value === 'string' && ID_PATTERN.test(value)
}

function boundedFingerprint(value: unknown, label: string): string {
  if (typeof value !== 'string' || !FINGERPRINT_PATTERN.test(value)) throw new KnifeBrowserCaptureError(`${label} is invalid`)
  return value
}

function sameSequence<T>(left: readonly T[], right: readonly T[]): boolean {
  return Array.isArray(left) && left.length === right.length && left.every((value, index) => value === right[index])
}

function validAovOrder(value: readonly KnifeCaptureAovId[]): boolean {
  if (!Array.isArray(value) || value.length < REQUIRED_AOV_IDS.length || !sameSequence(value.slice(0, REQUIRED_AOV_IDS.length), REQUIRED_AOV_IDS)) return false
  const allowed = new Set<KnifeCaptureAovId>([...REQUIRED_AOV_IDS, ...OPTIONAL_AOV_IDS])
  if (value.some((aov) => !allowed.has(aov))) return false
  if (new Set(value).size !== value.length) return false
  const extras = value.slice(REQUIRED_AOV_IDS.length)
  return sameSequence(extras, [...extras].sort((left, right) => OPTIONAL_AOV_IDS.indexOf(left) - OPTIONAL_AOV_IDS.indexOf(right)))
}

function createCameraBinding(viewId: KnifeViewId, projection: 'orthographic' | 'perspective', camera: THREE.Camera): KnifeCameraMatrixBinding {
  const binding = {
    view_id: viewId,
    projection,
    matrix_world: camera.matrixWorld.toArray(),
    matrix_world_inverse: camera.matrixWorldInverse.toArray(),
    projection_matrix: camera.projectionMatrix.toArray(),
    camera_fingerprint: '',
  }
  return { ...binding, camera_fingerprint: cameraFingerprint(binding) }
}

function cameraFingerprint(binding: Omit<KnifeCameraMatrixBinding, 'camera_fingerprint'> | KnifeCameraMatrixBinding): string {
  return sha256Hex(canonicalJson({
    view_id: binding.view_id,
    projection: binding.projection,
    matrix_world: binding.matrix_world,
    matrix_world_inverse: binding.matrix_world_inverse,
    projection_matrix: binding.projection_matrix,
  }))
}

function fingerprintScene(scene: THREE.Scene, compiled: CompiledKnifeScene): string {
  const parts = compiled.parts.map((part) => ({
    part_id: part.part_id,
    material_zone_id: part.material_zone_id,
    triangle_count: triangleCount(part),
    matrix_world: part.mesh.matrixWorld.toArray(),
  }))
  return sha256Hex(canonicalJson({
    scene_name: scene.name,
    scene_uuid: scene.uuid,
    group_name: compiled.group.name,
    group_uuid: compiled.group.uuid,
    group_matrix_world: compiled.group.matrixWorld.toArray(),
    program_fingerprint: compiled.deterministic_fingerprint,
    triangle_count: compiled.triangle_count,
    parts,
  }))
}

function triangleCount(part: CompiledKnifePart): number {
  const index = part.geometry.getIndex()
  const count = index ? index.count : part.geometry.getAttribute('position')?.count ?? 0
  return Math.floor(count / 3)
}

function renderAovToPng(renderer: THREE.WebGLRenderer, scene: THREE.Scene, compiled: CompiledKnifeScene, camera: THREE.Camera, aov: KnifeCaptureAovId, clearColor: THREE.ColorRepresentation): Uint8Array {
  const originalMaterials = new Map<THREE.Mesh, THREE.Material | THREE.Material[]>()
  const temporaryMaterials: THREE.Material[] = []
  const oldToneMapping = renderer.toneMapping
  const oldOutputColorSpace = renderer.outputColorSpace
  const oldBackground = scene.background
  try {
    if (aov !== 'beauty') scene.background = new THREE.Color(clearColor)
    renderer.setRenderTarget(null)
    renderer.setClearColor(clearColor, 1)
    if (aov !== 'beauty') {
      renderer.toneMapping = THREE.NoToneMapping
      // Three.js does not accept NoColorSpace as a renderer output target;
      // LinearSRGBColorSpace keeps data AOV shader values free of a transfer
      // function while remaining a supported WebGL canvas color space.
      renderer.outputColorSpace = THREE.LinearSRGBColorSpace
      for (let index = 0; index < compiled.parts.length; index += 1) {
        const part = compiled.parts[index]
        const material = materialForAov(aov, part, index, compiled)
        originalMaterials.set(part.mesh, part.mesh.material)
        ;(part.mesh as unknown as THREE.Mesh).material = material
        temporaryMaterials.push(material)
      }
    }
    renderer.render(scene, camera)
    const dataUrl = renderer.domElement.toDataURL('image/png')
    return decodePngDataUrl(dataUrl)
  } finally {
    for (const [mesh, material] of originalMaterials) mesh.material = material
    for (const material of temporaryMaterials) material.dispose()
    renderer.toneMapping = oldToneMapping
    renderer.outputColorSpace = oldOutputColorSpace
    scene.background = oldBackground
  }
}

function materialForAov(aov: KnifeCaptureAovId, part: CompiledKnifePart, partIndex: number, compiled: CompiledKnifeScene): THREE.Material {
  if (aov === 'silhouette') return new THREE.MeshBasicMaterial({ color: 0xffffff, side: THREE.DoubleSide, toneMapped: false })
  if (aov === 'depth') return new THREE.MeshDepthMaterial({ depthPacking: THREE.BasicDepthPacking, side: THREE.DoubleSide })
  if (aov === 'normal') return new THREE.MeshNormalMaterial({ side: THREE.DoubleSide })
  if (aov === 'wireframe') return new THREE.MeshBasicMaterial({ color: 0xffffff, wireframe: true, side: THREE.DoubleSide, toneMapped: false })
  let colorIndex = partIndex + 1
  if (aov === 'material-id') {
    const zoneIds = [...new Set(compiled.parts.map((candidate) => candidate.material_zone_id))].sort()
    colorIndex = zoneIds.indexOf(part.material_zone_id) + 1
  }
  const [red, green, blue] = encodeIdColor(colorIndex)
  const color = new THREE.Color(red / 255, green / 255, blue / 255)
  return new THREE.MeshBasicMaterial({ color, side: THREE.DoubleSide, toneMapped: false })
}

function encodeIdColor(value: number): [number, number, number] {
  const bounded = Math.max(1, Math.min(0xffffff, value))
  return [(bounded >>> 16) & 0xff, (bounded >>> 8) & 0xff, bounded & 0xff]
}

function decodePngDataUrl(dataUrl: string): Uint8Array {
  if (!dataUrl.startsWith('data:image/png;base64,')) throw new KnifeBrowserCaptureError('canvas did not return a PNG data URL')
  const encoded = dataUrl.slice('data:image/png;base64,'.length)
  if (typeof atob !== 'function') throw new KnifeBrowserCaptureError('browser atob is unavailable for PNG capture')
  const decoded = atob(encoded)
  const bytes = new Uint8Array(decoded.length)
  for (let index = 0; index < decoded.length; index += 1) bytes[index] = decoded.charCodeAt(index)
  if (bytes.byteLength <= 0 || bytes.byteLength > MAX_PNG_BYTES) throw new KnifeBrowserCaptureError('captured PNG size is outside the bounded range')
  const signature = [137, 80, 78, 71, 13, 10, 26, 10]
  if (bytes.byteLength < signature.length || signature.some((value, index) => bytes[index] !== value)) {
    throw new KnifeBrowserCaptureError('captured data URL is not a PNG byte stream')
  }
  return bytes
}

function canonicalSha256(value: object): string {
  return sha256Hex(canonicalJson({ ...(value as Record<string, unknown>), canonical_sha256: '' }))
}

function freezeDeep<T>(value: T): T {
  if (!value || typeof value !== 'object' || Object.isFrozen(value)) return value
  for (const child of Object.values(value as Record<string, unknown>)) freezeDeep(child)
  return Object.freeze(value)
}

function canonicalJson(value: unknown): string {
  if (value === null) return 'null'
  if (typeof value === 'string') return JSON.stringify(value)
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new KnifeBrowserCaptureError('canonical JSON cannot contain a non-finite number')
    return Object.is(value, -0) ? '0' : JSON.stringify(value)
  }
  if (Array.isArray(value)) return `[${value.map((item) => canonicalJson(item)).join(',')}]`
  if (typeof value === 'object') {
    const record = value as Record<string, unknown>
    return `{${Object.keys(record).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`).join(',')}}`
  }
  throw new KnifeBrowserCaptureError('canonical JSON cannot contain undefined or a function')
}

/** Small browser-safe SHA-256 implementation for PNG and closed receipts. */
export function sha256Hex(input: string | Uint8Array): string {
  const bytes = typeof input === 'string' ? new TextEncoder().encode(input) : input
  const bitLength = bytes.byteLength * 8
  const paddedLength = Math.ceil((bytes.byteLength + 9) / 64) * 64
  const padded = new Uint8Array(paddedLength)
  padded.set(bytes)
  padded[bytes.byteLength] = 0x80
  const view = new DataView(padded.buffer)
  view.setUint32(paddedLength - 8, Math.floor(bitLength / 0x100000000), false)
  view.setUint32(paddedLength - 4, bitLength >>> 0, false)

  const constants = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
  ]
  let h0 = 0x6a09e667
  let h1 = 0xbb67ae85
  let h2 = 0x3c6ef372
  let h3 = 0xa54ff53a
  let h4 = 0x510e527f
  let h5 = 0x9b05688c
  let h6 = 0x1f83d9ab
  let h7 = 0x5be0cd19
  const words = new Uint32Array(64)
  for (let offset = 0; offset < paddedLength; offset += 64) {
    for (let index = 0; index < 16; index += 1) words[index] = view.getUint32(offset + index * 4, false)
    for (let index = 16; index < 64; index += 1) {
      const value = words[index - 15]
      const sigma0 = ((value >>> 7) | (value << 25)) ^ ((value >>> 18) | (value << 14)) ^ (value >>> 3)
      const next = words[index - 2]
      const sigma1 = ((next >>> 17) | (next << 15)) ^ ((next >>> 19) | (next << 13)) ^ (next >>> 10)
      words[index] = (words[index - 16] + sigma0 + words[index - 7] + sigma1) >>> 0
    }
    let a = h0; let b = h1; let c = h2; let d = h3; let e = h4; let f = h5; let g = h6; let h = h7
    for (let index = 0; index < 64; index += 1) {
      const sigma1 = ((e >>> 6) | (e << 26)) ^ ((e >>> 11) | (e << 21)) ^ ((e >>> 25) | (e << 7))
      const choice = (e & f) ^ (~e & g)
      const temp1 = (h + sigma1 + choice + constants[index] + words[index]) >>> 0
      const sigma0 = ((a >>> 2) | (a << 30)) ^ ((a >>> 13) | (a << 19)) ^ ((a >>> 22) | (a << 10))
      const majority = (a & b) ^ (a & c) ^ (b & c)
      const temp2 = (sigma0 + majority) >>> 0
      h = g; g = f; f = e; e = (d + temp1) >>> 0; d = c; c = b; b = a; a = (temp1 + temp2) >>> 0
    }
    h0 = (h0 + a) >>> 0; h1 = (h1 + b) >>> 0; h2 = (h2 + c) >>> 0; h3 = (h3 + d) >>> 0
    h4 = (h4 + e) >>> 0; h5 = (h5 + f) >>> 0; h6 = (h6 + g) >>> 0; h7 = (h7 + h) >>> 0
  }
  return [h0, h1, h2, h3, h4, h5, h6, h7].map((value) => value.toString(16).padStart(8, '0')).join('')
}
