import * as THREE from 'three'

import {
  KNIFE_VIEW_IDS,
  createKnifeViewCamera,
  getKnifeView,
  type KnifeViewId,
  type KnifeViewRig,
} from './knife-view-evaluation.ts'
import type { CompiledKnifePart, CompiledKnifeScene } from './knife-scene-compiler.ts'
import { createKnifeCameraMatrixBinding, sha256Hex } from './knife-browser-capture.ts'

/**
 * The packaged preview worker has a deliberately smaller AOV contract than
 * the historical capture API.  These IDs are the only bytes emitted by the
 * fixed worker: one PNG per AOV for each of the eight fixed cameras.
 */
export const KNIFE_PREVIEW_WORKER_AOV_IDS = [
  'beauty',
  'alpha-silhouette',
  'semantic-id',
  'depth',
  'normal',
  'roughness-material-id',
] as const

export type KnifePreviewWorkerAovId = (typeof KNIFE_PREVIEW_WORKER_AOV_IDS)[number]

export const KNIFE_PREVIEW_WORKER_AOV_COUNT = 48 as const
export const KNIFE_PREVIEW_WORKER_AOV_MIME = 'image/png' as const
export const KNIFE_PREVIEW_WORKER_ROUGHNESS_ENCODING =
  'R=round(roughness*255);G/B=stable-material-zone-index-u16-big-endian@1' as const

export interface KnifePreviewWorkerPass {
  readonly aov_id: KnifePreviewWorkerAovId
  /** Semantic hash of the rendered PNG. This route has no separate semantic raster. */
  readonly sha256: string
  /** CAS object hash of the exact PNG bytes. */
  readonly object_sha256: string
  readonly bytes: number
  readonly mime: typeof KNIFE_PREVIEW_WORKER_AOV_MIME
}

export interface KnifePreviewWorkerView {
  readonly view_id: KnifeViewId
  readonly camera_sha256: string
  readonly width: 512
  readonly height: 512
  readonly passes: readonly KnifePreviewWorkerPass[]
}

export interface KnifePreviewWorkerPayload {
  readonly view_id: KnifeViewId
  readonly aov_id: KnifePreviewWorkerAovId
  readonly mime_type: typeof KNIFE_PREVIEW_WORKER_AOV_MIME
  readonly base64: string
}

export interface KnifePreviewWorkerCapture {
  readonly schema_version: 'WeaponryThreeJsPreviewWorkerCapture@1'
  readonly view_ids: readonly KnifeViewId[]
  readonly aov_ids: readonly KnifePreviewWorkerAovId[]
  readonly width: 512
  readonly height: 512
  readonly roughness_material_id_encoding: typeof KNIFE_PREVIEW_WORKER_ROUGHNESS_ENCODING
  readonly views: readonly KnifePreviewWorkerView[]
  readonly payloads: readonly KnifePreviewWorkerPayload[]
  readonly renderer_invoked: true
}

export interface KnifePreviewWorkerCaptureOptions {
  readonly renderer: THREE.WebGLRenderer
  readonly scene: THREE.Scene
  readonly compiled: CompiledKnifeScene
  readonly rig: KnifeViewRig
  readonly clear_color?: THREE.ColorRepresentation
  readonly capture_sink?: (viewId: KnifeViewId, aovId: KnifePreviewWorkerAovId, pngBytes: Uint8Array) => void
}

const MAX_PNG_BYTES = 4 * 1024 * 1024

/**
 * Render the fixed worker's six AOVs. The function only accepts a live
 * browser WebGLRenderer and reads bytes from its canvas; no manifest or view
 * list is treated as proof of a render.
 */
export function captureKnifePreviewWorkerAovs(options: KnifePreviewWorkerCaptureOptions): KnifePreviewWorkerCapture {
  if (!options || !options.renderer || !options.scene || !options.compiled || !options.rig) {
    throw new Error('KNIFE_PREVIEW_WORKER_INVALID: live renderer, scene, compiled scene and fixed rig are required')
  }
  if (options.rig.frame_width !== 512 || options.rig.frame_height !== 512 || options.rig.views.length !== KNIFE_VIEW_IDS.length) {
    throw new Error('KNIFE_PREVIEW_WORKER_INVALID: renderer must use the fixed 512x512 eight-view rig')
  }
  if (options.renderer.domElement.width !== 512 || options.renderer.domElement.height !== 512) {
    throw new Error('KNIFE_PREVIEW_WORKER_INVALID: renderer canvas must be exactly 512x512')
  }
  const context = options.renderer.getContext()
  if (!context || typeof context.getContextAttributes !== 'function' || !context.getContextAttributes()?.preserveDrawingBuffer) {
    throw new Error('KNIFE_PREVIEW_WORKER_INVALID: preserveDrawingBuffer is required for PNG capture')
  }
  if (!options.compiled.parts.length) throw new Error('KNIFE_PREVIEW_WORKER_INVALID: compiled scene has no parts')

  options.scene.updateMatrixWorld(true)
  options.compiled.group.updateMatrixWorld(true)
  const renderer = options.renderer
  const oldClearColor = renderer.getClearColor(new THREE.Color()).clone()
  const oldClearAlpha = renderer.getClearAlpha()
  const oldToneMapping = renderer.toneMapping
  const oldOutputColorSpace = renderer.outputColorSpace
  const oldBackground = options.scene.background
  const oldTarget = renderer.getRenderTarget()
  const oldViewport = renderer.getViewport(new THREE.Vector4()).clone()
  const oldScissor = renderer.getScissor(new THREE.Vector4()).clone()
  const oldScissorTest = renderer.getScissorTest()
  const clearColor = options.clear_color ?? 0x000000
  const payloads: KnifePreviewWorkerPayload[] = []
  const views: KnifePreviewWorkerView[] = []

  try {
    renderer.setRenderTarget(null)
    renderer.setViewport(0, 0, 512, 512)
    renderer.setScissorTest(false)
    for (const viewId of KNIFE_VIEW_IDS) {
      const view = getKnifeView(options.rig, viewId)
      const camera = createKnifeViewCamera(options.rig, viewId)
      camera.updateMatrixWorld(true)
      const camera_sha256 = createKnifeCameraMatrixBinding(options.rig, viewId).camera_fingerprint
      const passes: KnifePreviewWorkerPass[] = []
      for (const aovId of KNIFE_PREVIEW_WORKER_AOV_IDS) {
        const bytes = renderPreviewAov(renderer, options.scene, options.compiled, camera, aovId, clearColor)
        if (bytes.byteLength === 0 || bytes.byteLength > MAX_PNG_BYTES) {
          throw new Error(`KNIFE_PREVIEW_WORKER_INVALID: ${viewId}/${aovId} PNG size is outside the bounded range`)
        }
        options.capture_sink?.(viewId, aovId, bytes.slice())
        let binary = ''
        const chunkSize = 0x8000
        for (let offset = 0; offset < bytes.length; offset += chunkSize) {
          binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize))
        }
        payloads.push({ view_id: viewId, aov_id: aovId, mime_type: KNIFE_PREVIEW_WORKER_AOV_MIME, base64: btoa(binary) })
        const hash = sha256Hex(bytes)
        passes.push({ aov_id: aovId, sha256: hash, object_sha256: hash, bytes: bytes.byteLength, mime: KNIFE_PREVIEW_WORKER_AOV_MIME })
      }
      views.push({ view_id: viewId, camera_sha256, width: 512, height: 512, passes })
      // Keep the descriptor read from the canonical rig in the loop so a
      // changed rig cannot silently produce a result with the old ordering.
      if (view.view_id !== viewId) throw new Error(`KNIFE_PREVIEW_WORKER_INVALID: fixed view order drifted at ${viewId}`)
    }
  } finally {
    renderer.setRenderTarget(oldTarget)
    renderer.setViewport(oldViewport)
    renderer.setScissor(oldScissor)
    renderer.setScissorTest(oldScissorTest)
    renderer.setClearColor(oldClearColor, oldClearAlpha)
    renderer.toneMapping = oldToneMapping
    renderer.outputColorSpace = oldOutputColorSpace
    options.scene.background = oldBackground
  }

  if (views.length !== 8 || payloads.length !== KNIFE_PREVIEW_WORKER_AOV_COUNT) {
    throw new Error(`KNIFE_PREVIEW_WORKER_INVALID: captured ${views.length} views and ${payloads.length} PNGs`)
  }
  return {
    schema_version: 'WeaponryThreeJsPreviewWorkerCapture@1',
    view_ids: [...KNIFE_VIEW_IDS],
    aov_ids: [...KNIFE_PREVIEW_WORKER_AOV_IDS],
    width: 512,
    height: 512,
    roughness_material_id_encoding: KNIFE_PREVIEW_WORKER_ROUGHNESS_ENCODING,
    views,
    payloads,
    renderer_invoked: true,
  }
}

function renderPreviewAov(
  renderer: THREE.WebGLRenderer,
  scene: THREE.Scene,
  compiled: CompiledKnifeScene,
  camera: THREE.Camera,
  aov: KnifePreviewWorkerAovId,
  clearColor: THREE.ColorRepresentation,
): Uint8Array {
  const originalMaterials = new Map<THREE.Mesh, THREE.Material | THREE.Material[]>()
  const temporaryMaterials: THREE.Material[] = []
  const oldToneMapping = renderer.toneMapping
  const oldOutputColorSpace = renderer.outputColorSpace
  const oldBackground = scene.background
  try {
    scene.background = aov === 'beauty' ? oldBackground : new THREE.Color(clearColor)
    renderer.setRenderTarget(null)
    renderer.setClearColor(clearColor, 1)
    if (aov !== 'beauty') {
      renderer.toneMapping = THREE.NoToneMapping
      renderer.outputColorSpace = THREE.LinearSRGBColorSpace
      const materials = materialForPreviewAov(aov, compiled)
      for (let index = 0; index < compiled.parts.length; index += 1) {
        const part = compiled.parts[index]
        const material = materials[index]
        originalMaterials.set(part.mesh, part.mesh.material)
        ;(part.mesh as unknown as THREE.Mesh).material = material
        temporaryMaterials.push(material)
      }
    }
    renderer.render(scene, camera)
    const dataUrl = renderer.domElement.toDataURL('image/png')
    return decodePng(dataUrl)
  } finally {
    for (const [mesh, material] of originalMaterials) mesh.material = material
    for (const material of temporaryMaterials) material.dispose()
    renderer.toneMapping = oldToneMapping
    renderer.outputColorSpace = oldOutputColorSpace
    scene.background = oldBackground
  }
}

function materialForPreviewAov(aov: Exclude<KnifePreviewWorkerAovId, 'beauty'>, compiled: CompiledKnifeScene): THREE.Material[] {
  const sortedParts = [...compiled.parts].sort((left, right) => left.part_id.localeCompare(right.part_id))
  const partIndices = new Map(sortedParts.map((part, index) => [part.part_id, index + 1]))
  const zoneIds = [...new Set(compiled.parts.map((part) => part.material_zone_id))].sort()
  const zoneIndices = new Map(zoneIds.map((zone, index) => [zone, index + 1]))
  return compiled.parts.map((part) => {
    if (aov === 'alpha-silhouette') return new THREE.MeshBasicMaterial({ color: 0xffffff, side: THREE.DoubleSide, toneMapped: false })
    if (aov === 'depth') return new THREE.MeshDepthMaterial({ depthPacking: THREE.BasicDepthPacking, side: THREE.DoubleSide })
    if (aov === 'normal') return new THREE.MeshNormalMaterial({ side: THREE.DoubleSide })
    if (aov === 'semantic-id') return idMaterial(partIndices.get(part.part_id) ?? 1)
    const materialIndex = zoneIndices.get(part.material_zone_id) ?? 1
    const red = Math.round(Math.min(1, Math.max(0, part.material.roughness)) * 255)
    const green = (materialIndex >>> 8) & 0xff
    const blue = materialIndex & 0xff
    return new THREE.MeshBasicMaterial({ color: new THREE.Color(red / 255, green / 255, blue / 255), side: THREE.DoubleSide, toneMapped: false })
  })
}

function idMaterial(index: number): THREE.MeshBasicMaterial {
  const bounded = Math.max(1, Math.min(0xffffff, index))
  return new THREE.MeshBasicMaterial({
    color: new THREE.Color(((bounded >>> 16) & 0xff) / 255, ((bounded >>> 8) & 0xff) / 255, (bounded & 0xff) / 255),
    side: THREE.DoubleSide,
    toneMapped: false,
  })
}

function decodePng(dataUrl: string): Uint8Array {
  if (!dataUrl.startsWith('data:image/png;base64,')) throw new Error('KNIFE_PREVIEW_WORKER_INVALID: canvas did not return PNG')
  const decoded = atob(dataUrl.slice('data:image/png;base64,'.length))
  const bytes = new Uint8Array(decoded.length)
  for (let index = 0; index < decoded.length; index += 1) bytes[index] = decoded.charCodeAt(index)
  if (bytes.byteLength < 8 || bytes[0] !== 137 || bytes[1] !== 80 || bytes[2] !== 78 || bytes[3] !== 71) {
    throw new Error('KNIFE_PREVIEW_WORKER_INVALID: canvas result is not a PNG')
  }
  return bytes
}
