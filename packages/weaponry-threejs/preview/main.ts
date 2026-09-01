import * as THREE from 'three'

import dragonfangProgram from '../../../skills/weaponry-threejs-knife-studio/references/dragonfang-first-slice.json'
import dragonfangFullAssetCalibration from '../../../skills/weaponry-threejs-knife-studio/references/dragonfang-full-asset-calibration.json'
import {
  compileKnifeSceneProgram,
  captureKnifeAovs,
  createKnifeViewCamera,
  createKnifeViewRig,
  createKnifeViewRigFromCalibration,
  fingerprintKnifePreviewScene,
  getKnifeView,
  hashKnifeBrowserPreviewReceipt,
  makeKnifeBrowserCameraReceipt,
  overrideKnifePreviewUuid,
  parseKnifePreviewQuery,
  stableKnifePreviewUuid,
  type KnifeBrowserPreviewReceipt,
  type KnifeBrowserCaptureResult,
  type KnifeCaptureAovId,
  type KnifeBrowserViewReceipt,
  type KnifePreviewViewport,
  type KnifeSceneProgram,
  type KnifeViewCalibration,
} from '../src/index.ts'

declare global {
  interface Window {
    weaponryPreviewReceipt?: KnifeBrowserPreviewReceipt
    weaponryCaptureBundle?: KnifeBrowserExportBundle
  }
}

interface KnifeBrowserExportPayload {
  readonly view_id: string
  readonly aov_id: KnifeCaptureAovId
  readonly mime_type: 'image/png'
  readonly base64: string
}

interface KnifeBrowserExportBundle {
  readonly schema_version: 'WeaponryThreeJsBrowserExportBundle@1'
  readonly capture: KnifeBrowserCaptureResult
  readonly payloads: readonly KnifeBrowserExportPayload[]
}

const host = document.querySelector<HTMLDivElement>('#app')
const status = document.querySelector<HTMLSpanElement>('#status')
const receiptNode = document.querySelector<HTMLOutputElement>('#weaponry-preview-receipt')
const captureNode = document.querySelector<HTMLOutputElement>('#weaponry-capture-bundle')
if (!host) throw new Error('preview host is missing')

const request = parseKnifePreviewQuery(window.location.search)
const rig = request.framing === 'full-asset-baseline'
  ? createKnifeViewRigFromCalibration(dragonfangFullAssetCalibration as unknown as KnifeViewCalibration)
  : createKnifeViewRig(request.rig_options)
const compiled = compileKnifeSceneProgram(dragonfangProgram as unknown as KnifeSceneProgram)

const scene = new THREE.Scene()
scene.name = `weaponry-preview-scene:${compiled.deterministic_fingerprint}`
overrideKnifePreviewUuid(scene, stableKnifePreviewUuid(`scene:${compiled.deterministic_fingerprint}`))
scene.background = new THREE.Color('#080b10')
scene.userData = {
  schema_version: 'WeaponryThreeJsPreviewScene@1',
  source_fingerprint: compiled.deterministic_fingerprint,
  rig_id: rig.rig_id,
  rig_fingerprint: rig.deterministic_fingerprint,
  renderer_invoked: false,
  quality_status: 'NOT_RUN',
}
scene.add(compiled.group)

const hemisphere = new THREE.HemisphereLight(0xe8edf5, 0x101820, 1.45)
hemisphere.name = 'preview-light:hemisphere'
overrideKnifePreviewUuid(hemisphere, stableKnifePreviewUuid(`light:${compiled.deterministic_fingerprint}:hemisphere`))
scene.add(hemisphere)

const key = new THREE.DirectionalLight(0xfff0d8, 4.2)
key.name = 'preview-light:key'
key.position.set(-2.4, 3.0, 4.0)
overrideKnifePreviewUuid(key, stableKnifePreviewUuid(`light:${compiled.deterministic_fingerprint}:key`))
scene.add(key)

const rim = new THREE.DirectionalLight(0x8abfff, 2.1)
rim.name = 'preview-light:rim'
rim.position.set(2.8, -1.2, 2.4)
overrideKnifePreviewUuid(rim, stableKnifePreviewUuid(`light:${compiled.deterministic_fingerprint}:rim`))
scene.add(rim)

const viewIds = request.selected_view_ids
const tileWidth = rig.frame_width
const tileHeight = rig.frame_height
const columns = Math.max(1, Math.ceil(Math.sqrt(viewIds.length)))
const rows = Math.max(1, Math.ceil(viewIds.length / columns))
const renderWidth = tileWidth * columns
const renderHeight = tileHeight * rows

const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false, powerPreference: 'high-performance' })
renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
renderer.setSize(renderWidth, renderHeight, false)
renderer.outputColorSpace = THREE.SRGBColorSpace
renderer.toneMapping = THREE.ACESFilmicToneMapping
renderer.toneMappingExposure = 1.15
renderer.setClearColor('#080b10', 1)
renderer.setScissorTest(false)
renderer.clear(true, true, true)
renderer.setScissorTest(true)
host.append(renderer.domElement)

const viewReceipts: KnifeBrowserViewReceipt[] = []
for (let index = 0; index < viewIds.length; index += 1) {
  const viewId = viewIds[index]
  const camera = createKnifeViewCamera(rig, viewId)
  const column = index % columns
  const row = Math.floor(index / columns)
  // WebGL viewports use a bottom-left origin; the page grid is top-to-bottom.
  const viewport: KnifePreviewViewport = {
    x: column * tileWidth,
    y: renderHeight - (row + 1) * tileHeight,
    width: tileWidth,
    height: tileHeight,
  }
  renderer.setViewport(viewport.x, viewport.y, viewport.width, viewport.height)
  renderer.setScissor(viewport.x, viewport.y, viewport.width, viewport.height)
  renderer.render(scene, camera)
  const cameraReceipt = makeKnifeBrowserCameraReceipt(camera, getKnifeView(rig, viewId), viewport)
  viewReceipts.push({
    view_id: viewId,
    viewport: Object.freeze({ ...viewport }),
    camera: cameraReceipt,
    render_status: 'RENDERED',
    renderer_invoked: true,
    quality_status: 'NOT_RUN',
  })
}
renderer.setScissorTest(false)
renderer.setViewport(0, 0, renderWidth, renderHeight)

if (request.capture_aovs) {
  const payloads: KnifeBrowserExportPayload[] = []
  const captureRenderer = new THREE.WebGLRenderer({
    antialias: true,
    alpha: false,
    preserveDrawingBuffer: true,
    powerPreference: 'high-performance',
  })
  captureRenderer.setPixelRatio(1)
  captureRenderer.setSize(rig.frame_width, rig.frame_height, false)
  captureRenderer.outputColorSpace = THREE.SRGBColorSpace
  captureRenderer.toneMapping = THREE.ACESFilmicToneMapping
  captureRenderer.toneMappingExposure = 1.15
  const capture = captureKnifeAovs({
    renderer: captureRenderer,
    scene,
    compiled,
    rig,
    capture_sink: (viewId, aovId, bytes) => {
      payloads.push({
        view_id: viewId,
        aov_id: aovId,
        mime_type: 'image/png',
        base64: bytesToBase64(bytes),
      })
    },
  })
  captureRenderer.dispose()
  const bundle: KnifeBrowserExportBundle = Object.freeze({
    schema_version: 'WeaponryThreeJsBrowserExportBundle@1',
    capture,
    payloads: Object.freeze(payloads),
  })
  window.weaponryCaptureBundle = bundle
  if (captureNode) captureNode.textContent = JSON.stringify(bundle)
}

scene.userData.renderer_invoked = true
scene.updateMatrixWorld(true)
const sceneFingerprint = fingerprintKnifePreviewScene(scene, compiled.deterministic_fingerprint)
const stableViews = Object.freeze(viewReceipts)
const renderTarget = Object.freeze({
  width: renderWidth,
  height: renderHeight,
  pixel_ratio: renderer.getPixelRatio(),
})
const receiptFingerprint = hashKnifeBrowserPreviewReceipt(
  compiled.deterministic_fingerprint,
  sceneFingerprint,
  rig.deterministic_fingerprint,
  stableViews,
)

const captureReadyReceipt: KnifeBrowserPreviewReceipt = Object.freeze({
  schema_version: 'WeaponryThreeJsBrowserPreviewReceipt@2',
  route: 'weaponry-threejs-knife-preview@1',
  asset_id: dragonfangProgram.asset_id,
  source_fingerprint: compiled.deterministic_fingerprint,
  scene_fingerprint: sceneFingerprint,
  rig_schema_version: rig.schema_version,
  rig_id: rig.rig_id,
  rig_fingerprint: rig.deterministic_fingerprint,
  manifest: request.manifest,
  selected_view_ids: Object.freeze([...viewIds]),
  views: stableViews,
  render_target: renderTarget,
  renderer: {
    type: 'THREE.WebGLRenderer' as const,
    three_revision: THREE.REVISION,
    antialias: true as const,
  },
  renderer_invoked: true,
  network_policy: 'bundled-static-only@1',
  external_network_used: false,
  render_status: 'RENDERED',
  capture_status: 'CAPTURE_READY',
  capture_ready: true,
  settled: false,
  visual_status: 'NOT_REVIEWED',
  quality_status: 'NOT_RUN',
  deterministic_fingerprint: receiptFingerprint,
})
publishReceipt(captureReadyReceipt)
if (status) {
  status.textContent = `${viewIds.join(' · ')} · capture-ready · quality NOT_RUN`
}

if (request.capture_mode === 'settled') {
  window.requestAnimationFrame(() => {
    const settledReceipt: KnifeBrowserPreviewReceipt = Object.freeze({
      ...captureReadyReceipt,
      capture_status: 'SETTLED',
      settled: true,
    })
    publishReceipt(settledReceipt)
    if (status) status.textContent = `${viewIds.join(' · ')} · settled · quality NOT_RUN`
  })
}

function publishReceipt(receipt: KnifeBrowserPreviewReceipt): void {
  window.weaponryPreviewReceipt = receipt
  if (receiptNode) receiptNode.textContent = JSON.stringify(receipt)
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = ''
  const chunkSize = 0x8000
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize))
  }
  return btoa(binary)
}
