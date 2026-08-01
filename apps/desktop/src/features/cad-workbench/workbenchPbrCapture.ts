/**
 * Transient evidence captured from the existing workbench renderer.
 *
 * This module deliberately has no Tauri, API, asset-version, or filesystem
 * dependency. A caller can hand the returned PNG bytes to a Rust-owned
 * pending inspection, but the capture itself cannot create a Snapshot,
 * QualityReport, export, or version.
 */

export const WORKBENCH_PBR_CAPTURE_SCHEMA = 'WorkbenchPbrVisualCapture@1' as const
export const WORKBENCH_PBR_RENDERER_ID = 'forgecad-workbench-pbr@1' as const
export const WORKBENCH_PBR_RENDER_MANIFEST_SHA256 = '024d7e8f707c75eafd12f22e9a5e9f9c5ab0fcbd1a6ce1a4de6726ace7b2451a' as const
export const WORKBENCH_PBR_VISUAL_ENVIRONMENT_ID = 'env_forgecad_room_studio_v2' as const
export const WORKBENCH_PBR_VISUAL_ENVIRONMENT_SHA256 = '0884e4f7b32c11ce94b4d406260f9ea89ca0c7933e0088d14e9eb89f382508a4' as const
export const WORKBENCH_PBR_CAPTURE_WIDTH_PX = 640 as const
export const WORKBENCH_PBR_CAPTURE_HEIGHT_PX = 640 as const
export const WORKBENCH_PBR_AUXILIARY_PASS_WIDTH_PX = 320 as const
export const WORKBENCH_PBR_AUXILIARY_PASS_HEIGHT_PX = 320 as const
export const WORKBENCH_PBR_AUXILIARY_CAPTURE_WIDTH_PX = 960 as const
export const WORKBENCH_PBR_AUXILIARY_CAPTURE_HEIGHT_PX = 640 as const
export const WORKBENCH_PBR_CAPTURE_EVENT = 'forgecad:workbench-pbr-capture@1'
export const WORKBENCH_PBR_CAMERA_EVENT = 'forgecad:workbench-pbr-camera@1'
export const WORKBENCH_PBR_LIGHT_EVENT = 'forgecad:workbench-pbr-light@1'

const SHA256_PATTERN = /^[a-f0-9]{64}$/i
const FIXED_VIEWS = [
  'turntable_000', 'turntable_045', 'turntable_090', 'turntable_135',
  'turntable_180', 'turntable_225', 'turntable_270', 'turntable_315',
] as const
const RESTORABLE_CAMERA_VIEWS = [
  ...FIXED_VIEWS,
  'iso', 'front', 'back', 'left', 'right', 'top', 'gripper_iso', 'gripper_front',
] as const
const CAPTURE_TIMEOUT_MS = 12_000

export type WorkbenchPbrCaptureView = typeof FIXED_VIEWS[number]
export type WorkbenchPbrConceptView = 'iso' | 'front' | 'side' | 'top'
export type WorkbenchPbrAuxiliaryPass = 'silhouette' | 'normal' | 'depth' | 'part_id' | 'material_id'
type WorkbenchPbrCameraView = typeof RESTORABLE_CAMERA_VIEWS[number]
export type WorkbenchPbrLightPreset = 'cad_neutral' | 'soft_studio' | 'concept_contrast'

/** A Rust-issued camera for exact reference-to-UV projection. */
export type WorkbenchPbrProjectionCameraBinding = {
  schemaVersion: 'ProjectionCameraBinding@1'
  algorithmId: 'forgecad.turntable_projection_camera'
  algorithmVersion: '1'
  candidateGlbSha256: string
  viewId: WorkbenchPbrCaptureView
  verticalFovMillidegrees: number
  frameTargetNdcMillionths: number
  sourceBoundsMeters: readonly [number, number, number]
  cameraPositionMeters: readonly [number, number, number]
  cameraTargetMeters: readonly [number, number, number]
  nearMeters: number
  farMeters: number
  worldToClipRowMajor: readonly number[]
  bindingSha256: string
}

export type WorkbenchPbrViewportIdentity = {
  renderer_id: typeof WORKBENCH_PBR_RENDERER_ID
  visual_environment_id: string
  visual_environment_sha256: string
  render_manifest_sha256: typeof WORKBENCH_PBR_RENDER_MANIFEST_SHA256
  output_color_space: 'srgb'
  tone_mapping: 'aces_filmic'
  source_glb_sha256: string
}

export type WorkbenchPbrCapture = {
  schema_version: typeof WORKBENCH_PBR_CAPTURE_SCHEMA
  renderer: WorkbenchPbrViewportIdentity
  view_id: WorkbenchPbrCaptureView
  width: number
  height: number
  pixel_encoding: 'display_srgb'
  camera_pose_sha256: string
  projection_camera_binding_sha256: string | null
  png_sha256: string
  png_bytes: Uint8Array
  auxiliary: {
    pass_ids: readonly WorkbenchPbrAuxiliaryPass[]
    width: typeof WORKBENCH_PBR_AUXILIARY_CAPTURE_WIDTH_PX
    height: typeof WORKBENCH_PBR_AUXILIARY_CAPTURE_HEIGHT_PX
    png_sha256: string
    png_bytes: Uint8Array
  }
}

/**
 * Four user-facing concept images captured from the same mounted PBR canvas.
 * Unlike the Rust candidate receipt, this is a transient local presentation
 * value: it never creates a version, quality report, or export record.
 */
export type WorkbenchPbrConceptCapture = {
  schema_version: typeof WORKBENCH_PBR_CAPTURE_SCHEMA
  renderer: WorkbenchPbrViewportIdentity
  view_id: WorkbenchPbrConceptView
  width: number
  height: number
  pixel_encoding: 'display_srgb'
  camera_pose_sha256: string
  png_sha256: string
  png_bytes: Uint8Array
}

type ViewportPixels = {
  width: number
  height: number
  pixels: Uint8Array
  origin: 'top_left'
  cameraPoseSha256: string
  auxiliaryPixels: Uint8Array
  auxiliaryWidth: number
  auxiliaryHeight: number
  auxiliaryPassIds: readonly WorkbenchPbrAuxiliaryPass[]
}

type ViewportCaptureRequest = {
  viewport: HTMLElement
  resolve: (capture: ViewportPixels) => void
  reject: (error: Error) => void
}

type ViewportCameraRequest = {
  viewport: HTMLElement
  view: WorkbenchPbrCameraView
  projectionCameraBinding?: WorkbenchPbrProjectionCameraBinding
  resolve: () => void
  reject: (error: Error) => void
}

type ViewportLightRequest = {
  viewport: HTMLElement
  preset: WorkbenchPbrLightPreset
  resolve: () => void
  reject: (error: Error) => void
}

export function fixedWorkbenchPbrCaptureViews(): readonly WorkbenchPbrCaptureView[] {
  return FIXED_VIEWS
}

/** Validates that the mounted canvas is displaying the exact compiled PBR GLB. */
export function readWorkbenchPbrViewportIdentity(
  viewport: HTMLElement,
  expectedSourceGlbSha256: string,
): WorkbenchPbrViewportIdentity {
  if (!SHA256_PATTERN.test(expectedSourceGlbSha256)) {
    throw new Error('WORKBENCH_PBR_CAPTURE_SOURCE_SHA256_INVALID')
  }
  const data = viewport.dataset
  const visualEnvironmentSha256 = data.visualEnvironmentSha256
  const embeddedPbrMaterialCount = Number(data.blockoutEmbeddedPbrMaterialCount ?? 0)
  const pbrTextureCount = Number(data.blockoutPbrTextureCount ?? 0)
  if (
    data.pbrRendererId !== WORKBENCH_PBR_RENDERER_ID
    || data.blockoutLoadState !== 'ready'
    || data.blockoutRenderSource !== 'glb_pbr'
    || !data.blockoutGlbKind?.startsWith('compiled_agent_')
    || !Number.isInteger(embeddedPbrMaterialCount)
    || embeddedPbrMaterialCount < 1
    || !Number.isInteger(pbrTextureCount)
    || pbrTextureCount < 5
    || data.blockoutPbrColorSpaces !== 'valid'
    || data.blockoutPbrSamplingValid !== 'true'
    || data.blockoutGlbSha256?.toLowerCase() !== expectedSourceGlbSha256.toLowerCase()
    || data.outputColorSpace !== 'srgb'
    || data.toneMapping !== 'aces_filmic'
    || data.pbrRenderManifestSha256 !== WORKBENCH_PBR_RENDER_MANIFEST_SHA256
    || !SHA256_PATTERN.test(visualEnvironmentSha256 ?? '')
    || !data.visualEnvironmentId
  ) throw new Error('WORKBENCH_PBR_CAPTURE_VIEWPORT_LINEAGE_INVALID')
  return {
    renderer_id: WORKBENCH_PBR_RENDERER_ID,
    visual_environment_id: data.visualEnvironmentId,
    visual_environment_sha256: visualEnvironmentSha256!.toLowerCase(),
    render_manifest_sha256: WORKBENCH_PBR_RENDER_MANIFEST_SHA256,
    output_color_space: 'srgb',
    tone_mapping: 'aces_filmic',
    source_glb_sha256: expectedSourceGlbSha256.toLowerCase(),
  }
}

/**
 * Captures fixed exterior views from the single mounted Three.js renderer.
 * The bytes remain only in the returned value; no Tauri command or storage is
 * reached here. A later Rust session owns any validation and persistence.
 */
export async function captureWorkbenchPbrViews(input: {
  viewport: HTMLElement
  sourceGlbSha256: string
  views?: readonly WorkbenchPbrCaptureView[]
  lightPreset?: WorkbenchPbrLightPreset
  projectionCameraBindings?: readonly WorkbenchPbrProjectionCameraBinding[]
}): Promise<WorkbenchPbrCapture[]> {
  const identity = readWorkbenchPbrViewportIdentity(input.viewport, input.sourceGlbSha256)
  const views = input.views ?? FIXED_VIEWS
  if (views.length === 0 || views.some((view) => !isWorkbenchPbrCaptureView(view))) {
    throw new Error('WORKBENCH_PBR_CAPTURE_VIEWS_INVALID')
  }
  const bindingsByView = projectionBindingsByView(input.projectionCameraBindings, views, input.sourceGlbSha256)
  const priorCamera = input.viewport.dataset.cameraView
  const priorLight = input.viewport.dataset.lightPreset
  const captures: WorkbenchPbrCapture[] = []
  try {
    await setLight(input.viewport, input.lightPreset ?? 'soft_studio')
    for (const view of views) {
      const binding = bindingsByView.get(view) ?? null
      await setCamera(input.viewport, view, binding ?? undefined)
      const pixels = await requestViewportPixels(input.viewport)
      const pngBytes = await encodeDisplaySrgbPng(pixels)
      const auxiliaryPngBytes = await encodeDisplaySrgbPng({
        ...pixels,
        width: pixels.auxiliaryWidth,
        height: pixels.auxiliaryHeight,
        pixels: pixels.auxiliaryPixels,
      })
      captures.push({
        schema_version: WORKBENCH_PBR_CAPTURE_SCHEMA,
        renderer: identity,
        view_id: view,
        width: pixels.width,
        height: pixels.height,
        pixel_encoding: 'display_srgb',
        camera_pose_sha256: pixels.cameraPoseSha256,
        projection_camera_binding_sha256: binding?.bindingSha256 ?? null,
        png_sha256: await sha256Hex(pngBytes),
        png_bytes: pngBytes,
        auxiliary: {
          pass_ids: pixels.auxiliaryPassIds,
          width: WORKBENCH_PBR_AUXILIARY_CAPTURE_WIDTH_PX,
          height: WORKBENCH_PBR_AUXILIARY_CAPTURE_HEIGHT_PX,
          png_sha256: await sha256Hex(auxiliaryPngBytes),
          png_bytes: auxiliaryPngBytes,
        },
      })
    }
    return captures
  } finally {
    if (isWorkbenchPbrLightPreset(priorLight)) await setLight(input.viewport, priorLight).catch(() => undefined)
    if (isWorkbenchPbrCameraView(priorCamera)) await setCamera(input.viewport, priorCamera).catch(() => undefined)
  }
}

/**
 * Captures the four persistent-Agent concept views from the exact canvas the
 * user is looking at. This is intentionally separate from the eight-view
 * Rust candidate receipt: committed-asset presentation has no pending Turn,
 * so it must remain a read-only browser artifact while still using the real
 * GPU/PBR renderer rather than the Python diagnostic rasterizer.
 */
export async function captureWorkbenchPbrConceptViews(input: {
  viewport: HTMLElement
  sourceGlbSha256: string
  lightPreset?: WorkbenchPbrLightPreset
}): Promise<WorkbenchPbrConceptCapture[]> {
  const identity = readWorkbenchPbrViewportIdentity(input.viewport, input.sourceGlbSha256)
  const views: readonly WorkbenchPbrConceptView[] = ['iso', 'front', 'side', 'top']
  const priorCamera = input.viewport.dataset.cameraView
  const priorLight = input.viewport.dataset.lightPreset
  const captures: WorkbenchPbrConceptCapture[] = []
  try {
    await setLight(input.viewport, input.lightPreset ?? 'soft_studio')
    for (const view of views) {
      // The legacy Agent presentation names the lateral image "side", while
      // the one workbench camera registry exposes that pose as "right".
      await setCamera(input.viewport, view === 'side' ? 'right' : view)
      const pixels = await requestViewportPixels(input.viewport)
      const pngBytes = await encodeDisplaySrgbPng(pixels)
      captures.push({
        schema_version: WORKBENCH_PBR_CAPTURE_SCHEMA,
        renderer: identity,
        view_id: view,
        width: pixels.width,
        height: pixels.height,
        pixel_encoding: 'display_srgb',
        camera_pose_sha256: pixels.cameraPoseSha256,
        png_sha256: await sha256Hex(pngBytes),
        png_bytes: pngBytes,
      })
    }
    return captures
  } finally {
    if (isWorkbenchPbrLightPreset(priorLight)) await setLight(input.viewport, priorLight).catch(() => undefined)
    if (isWorkbenchPbrCameraView(priorCamera)) await setCamera(input.viewport, priorCamera).catch(() => undefined)
  }
}

export async function sha256Hex(bytes: Uint8Array | ArrayBuffer): Promise<string> {
  const buffer: ArrayBuffer = bytes instanceof Uint8Array
    ? new Uint8Array(bytes).buffer
    : bytes.slice(0)
  const digest = await crypto.subtle.digest('SHA-256', buffer)
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('')
}

function isWorkbenchPbrCaptureView(value: unknown): value is WorkbenchPbrCaptureView {
  return typeof value === 'string' && FIXED_VIEWS.includes(value as WorkbenchPbrCaptureView)
}

function isWorkbenchPbrCameraView(value: unknown): value is WorkbenchPbrCameraView {
  return typeof value === 'string' && RESTORABLE_CAMERA_VIEWS.includes(value as WorkbenchPbrCameraView)
}

function isWorkbenchPbrLightPreset(value: unknown): value is WorkbenchPbrLightPreset {
  return value === 'cad_neutral' || value === 'soft_studio' || value === 'concept_contrast'
}

async function setCamera(
  viewport: HTMLElement,
  view: WorkbenchPbrCameraView,
  projectionCameraBinding?: WorkbenchPbrProjectionCameraBinding,
): Promise<void> {
  await dispatchWithTimeout<void>(viewport, WORKBENCH_PBR_CAMERA_EVENT, { viewport, view, projectionCameraBinding })
  await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))
}

function projectionBindingsByView(
  bindings: readonly WorkbenchPbrProjectionCameraBinding[] | undefined,
  views: readonly WorkbenchPbrCaptureView[],
  sourceGlbSha256: string,
): ReadonlyMap<WorkbenchPbrCaptureView, WorkbenchPbrProjectionCameraBinding> {
  if (!bindings) return new Map()
  if (bindings.length !== views.length) throw new Error('WORKBENCH_PBR_CAPTURE_CAMERA_BINDINGS_INCOMPLETE')
  const byView = new Map<WorkbenchPbrCaptureView, WorkbenchPbrProjectionCameraBinding>()
  for (const binding of bindings) {
    if (
      binding.schemaVersion !== 'ProjectionCameraBinding@1'
      || binding.algorithmId !== 'forgecad.turntable_projection_camera'
      || binding.algorithmVersion !== '1'
      || !isWorkbenchPbrCaptureView(binding.viewId)
      || binding.candidateGlbSha256.toLowerCase() !== sourceGlbSha256.toLowerCase()
      || binding.verticalFovMillidegrees !== 38000
      || binding.frameTargetNdcMillionths !== 840000
      || !SHA256_PATTERN.test(binding.bindingSha256)
      || binding.worldToClipRowMajor.length !== 16
      || !binding.worldToClipRowMajor.every(Number.isFinite)
      || ![...binding.sourceBoundsMeters, ...binding.cameraPositionMeters, ...binding.cameraTargetMeters, binding.nearMeters, binding.farMeters].every(Number.isFinite)
      || binding.nearMeters <= 0
      || binding.farMeters <= binding.nearMeters
      || byView.has(binding.viewId)
    ) throw new Error('WORKBENCH_PBR_CAPTURE_CAMERA_BINDING_INVALID')
    byView.set(binding.viewId, binding)
  }
  if (views.some((view) => !byView.has(view))) throw new Error('WORKBENCH_PBR_CAPTURE_CAMERA_BINDINGS_INCOMPLETE')
  return byView
}

async function setLight(viewport: HTMLElement, preset: WorkbenchPbrLightPreset): Promise<void> {
  await dispatchWithTimeout<void>(viewport, WORKBENCH_PBR_LIGHT_EVENT, { viewport, preset })
}

function requestViewportPixels(viewport: HTMLElement): Promise<ViewportPixels> {
  return dispatchWithTimeout<ViewportPixels>(viewport, WORKBENCH_PBR_CAPTURE_EVENT, { viewport })
    .then((capture) => {
      if (
        capture.width !== WORKBENCH_PBR_CAPTURE_WIDTH_PX
        || capture.height !== WORKBENCH_PBR_CAPTURE_HEIGHT_PX
        || capture.pixels.byteLength !== capture.width * capture.height * 4
        || capture.origin !== 'top_left'
        || !SHA256_PATTERN.test(capture.cameraPoseSha256)
        || capture.auxiliaryWidth !== WORKBENCH_PBR_AUXILIARY_CAPTURE_WIDTH_PX
        || capture.auxiliaryHeight !== WORKBENCH_PBR_AUXILIARY_CAPTURE_HEIGHT_PX
        || capture.auxiliaryPixels.byteLength !== capture.auxiliaryWidth * capture.auxiliaryHeight * 4
        || capture.auxiliaryPassIds.join(',') !== 'silhouette,normal,depth,part_id,material_id'
      ) throw new Error('WORKBENCH_PBR_CAPTURE_PIXELS_INVALID')
      return capture
    })
}

function dispatchWithTimeout<T>(
  viewport: HTMLElement,
  eventName: string,
  request: Record<string, unknown>,
): Promise<T> {
  return new Promise((resolve, reject) => {
    let settled = false
    const finish = (callback: () => void) => {
      if (settled) return
      settled = true
      window.clearTimeout(timeout)
      callback()
    }
    const timeout = window.setTimeout(() => finish(() => reject(new Error('WORKBENCH_PBR_CAPTURE_TIMEOUT'))), CAPTURE_TIMEOUT_MS)
    viewport.dispatchEvent(new CustomEvent(eventName, {
      detail: {
        ...request,
        resolve: (value: T) => finish(() => resolve(value)),
        reject: (error: unknown) => finish(() => reject(error instanceof Error ? error : new Error('WORKBENCH_PBR_CAPTURE_FAILED'))),
      },
    }))
  })
}

async function encodeDisplaySrgbPng(capture: ViewportPixels): Promise<Uint8Array> {
  const canvas = document.createElement('canvas')
  canvas.width = capture.width
  canvas.height = capture.height
  const context = canvas.getContext('2d', { willReadFrequently: true })
  if (!context) throw new Error('WORKBENCH_PBR_CAPTURE_PNG_CONTEXT_UNAVAILABLE')
  const image = context.createImageData(capture.width, capture.height)
  image.data.set(capture.pixels)
  context.putImageData(image, 0, 0)
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'))
  if (!blob || blob.size === 0) throw new Error('WORKBENCH_PBR_CAPTURE_PNG_UNAVAILABLE')
  return new Uint8Array(await blob.arrayBuffer())
}
