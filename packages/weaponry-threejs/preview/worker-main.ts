import * as THREE from 'three'

import {
  compileKnifeSceneProgram,
  captureKnifePreviewWorkerAovs,
  type KnifePreviewWorkerCapture,
  createKnifeViewRig,
  type KnifeSceneProgram,
} from '../src/index.ts'

/**
 * Static browser entry for the packaged preview worker.
 *
 * The host injects one closed program before loading this module.  The page
 * owns the WebGL context and is the only place where PNG bytes are produced;
 * the Node launcher only transports the resulting closed capture bundle.
 */

declare global {
  interface Window {
    __WPN_PREVIEW_PROGRAM__?: KnifeSceneProgram
    __WPN_THREEJS_PREVIEW_RESULT__?: KnifeBrowserPreviewWorkerResult
    __WPN_THREEJS_PREVIEW_ERROR__?: string
    __WPN_THREEJS_PREVIEW_PROGRESS__?: string
  }
}

interface KnifeBrowserPreviewWorkerResult {
  readonly schema_version: 'WeaponryThreeJsBrowserPreviewWorkerResult@1'
  readonly renderer: 'THREE.WebGLRenderer'
  readonly three_revision: string
  readonly capture: KnifePreviewWorkerCapture
  readonly view_count: 8
  readonly aov_count_per_view: 6
  readonly png_count: 48
  readonly renderer_invoked: true
  readonly roughness_material_id_encoding: string
  readonly visual_status: 'NOT_RUN'
  readonly human_status: 'NOT_RUN'
  readonly engine_status: 'NOT_RUN'
  readonly commercial_status: 'NOT_RUN'
}

const program = window.__WPN_PREVIEW_PROGRAM__
window.__WPN_THREEJS_PREVIEW_PROGRESS__ = 'module-loaded'

try {
  if (!program) throw new Error('packaged preview program was not injected')
  if (THREE.REVISION !== '185') throw new Error(`fixed Three.js revision drifted: ${THREE.REVISION}`)

  const compiled = compileKnifeSceneProgram(program)
  window.__WPN_THREEJS_PREVIEW_PROGRESS__ = 'scene-compiled'
  const rig = createKnifeViewRig({ frame_width: 512, frame_height: 512, margin: 0.08 })
  const scene = new THREE.Scene()
  scene.name = `weaponry-threejs-packaged-preview:${compiled.deterministic_fingerprint}`
  scene.background = new THREE.Color(0x080b10)
  scene.add(compiled.group)

  const hemisphere = new THREE.HemisphereLight(0xe8edf5, 0x101820, 1.45)
  hemisphere.position.set(0, 1, 0)
  scene.add(hemisphere)
  const key = new THREE.DirectionalLight(0xfff0d8, 4.2)
  key.position.set(-2.4, 3, 4)
  scene.add(key)
  const rim = new THREE.DirectionalLight(0x8abfff, 2.1)
  rim.position.set(2.8, -1.2, 2.4)
  scene.add(rim)
  scene.updateMatrixWorld(true)

  const renderer = new THREE.WebGLRenderer({
    antialias: true,
    alpha: false,
    preserveDrawingBuffer: true,
    powerPreference: 'high-performance',
  })
  renderer.setPixelRatio(1)
  renderer.setSize(rig.frame_width, rig.frame_height, false)
  renderer.outputColorSpace = THREE.SRGBColorSpace
  renderer.toneMapping = THREE.ACESFilmicToneMapping
  renderer.toneMappingExposure = 1.15
  renderer.setClearColor(0x080b10, 1)
  document.body.append(renderer.domElement)

  window.__WPN_THREEJS_PREVIEW_PROGRESS__ = 'capture-started'
  const capture = captureKnifePreviewWorkerAovs({
    renderer,
    scene,
    compiled,
    rig,
    clear_color: 0x000000,
  })
  window.__WPN_THREEJS_PREVIEW_PROGRESS__ = 'capture-complete'

  renderer.dispose()
  if (capture.payloads.length !== 48) throw new Error(`packaged preview captured ${capture.payloads.length} PNGs, expected 48`)
  window.__WPN_THREEJS_PREVIEW_RESULT__ = {
    schema_version: 'WeaponryThreeJsBrowserPreviewWorkerResult@1',
    renderer: 'THREE.WebGLRenderer',
    three_revision: '0.185.1',
    capture,
    view_count: 8,
    aov_count_per_view: 6,
    png_count: 48,
    renderer_invoked: true,
    roughness_material_id_encoding: capture.roughness_material_id_encoding,
    visual_status: 'NOT_RUN',
    human_status: 'NOT_RUN',
    engine_status: 'NOT_RUN',
    commercial_status: 'NOT_RUN',
  }
} catch (error) {
  window.__WPN_THREEJS_PREVIEW_ERROR__ = error instanceof Error ? error.message : String(error)
}
