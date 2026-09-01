import * as THREE from 'three'
import RAPIER from '@dimforge/rapier3d-compat'

import {
  createWeaponryAnimationClipSet,
  createWeaponryAnimationMixerAdapter,
  createWeaponryFrameBudgetMonitor,
  createWeaponryRapierPreviewBridge,
  createWeaponryRightHandGripBinding,
  createWeaponryRightHandTarget,
  createWeaponryThreeJsEngineFromRoot,
  loadWeaponryR8Delivery,
  setWeaponrySelectedPart,
  type WeaponryAction,
  type WeaponryAnimationMixerAdapter,
  type WeaponryAnimationMixerSnapshot,
  type WeaponryEngineSnapshot,
  type WeaponryFpsSocketBinding,
  type WeaponryFpsSocketBindingSnapshot,
  type WeaponryFrameBudgetSnapshot,
  type WeaponryRapierPreviewBridge,
  type WeaponryRapierPreviewModule,
  type WeaponryRapierPreviewSnapshot,
  type WeaponrySimulationState,
  type WeaponryThreeJsEngine,
} from '../../src/engine/index.ts'

import './styles.css'

type PresentationCamera = 'orbit' | 'inspect' | 'fps'
type RuntimeStatus = 'loading' | 'ready' | 'error' | 'context-lost' | 'recovering'

interface DeliveryManifest {
  readonly asset_id: string
  readonly asset_version: string
  readonly delivery_glb: { readonly path: string; readonly sha256: string; readonly bytes: number; readonly triangles: number; readonly draw_calls: number }
  readonly action_runtime: { readonly part_ids: readonly string[] }
  readonly dependency_lock?: { readonly three_version?: string }
}

const EXPECTED_ASSET_ID = 'Dragonfang Kukri'
const EXPECTED_ASSET_VERSION = 'r8'
const EXPECTED_GLB_PATH = 'dragonfang-kukri-r8-action-ready.glb'
const EXPECTED_THREE_VERSION = '0.185.1'

interface EngineProbeState {
  readonly schema_version: 'WeaponryThreeJsEngineProbe@1'
  readonly status: RuntimeStatus
  readonly asset_id: string | null
  readonly asset_version: string | null
  readonly delivery_glb_sha256: string | null
  readonly camera_mode: PresentationCamera
  readonly action: WeaponryAction
  /** Backward-compatible simulation projection; authored clip evidence is below. */
  readonly interaction_pose: WeaponryAction
  readonly interaction_pose_playing: boolean
  readonly interaction_pose_progress: number
  readonly pose_runtime: 'prototype'
  readonly animation_snapshot: WeaponryAnimationMixerSnapshot | null
  readonly fps_binding_snapshot: WeaponryFpsSocketBindingSnapshot | null
  readonly physics_snapshot: WeaponryRapierPreviewSnapshot | null
  readonly frame_budget: WeaponryFrameBudgetSnapshot
  readonly paused: boolean
  readonly selected_part_id: string | null
  readonly visible_part_ids: readonly string[]
  readonly exploded_amount: number
  readonly context_status: RuntimeStatus
  readonly context_recoveries: number
  readonly viewport: { readonly width: number; readonly height: number; readonly pixel_ratio: number }
  readonly frame: number
  readonly last_input: 'none' | 'pointer' | 'keyboard' | 'wheel' | 'resize'
  readonly last_key: string | null
  readonly engine_snapshot: WeaponryEngineSnapshot | null
  readonly error: string | null
}

declare global {
  interface Window {
    readonly __WPN_ENGINE_PROBE__: EngineProbeState
  }
}

const canvas = query<HTMLCanvasElement>('engine-canvas')
const stage = query<HTMLElement>('engine-stage')
const loading = query<HTMLElement>('engine-loading')
const statusNode = query<HTMLElement>('engine-status')
const statusLight = query<HTMLElement>('engine-status-light')
const selectedNode = query<HTMLElement>('engine-selected-part')
const selectedHintNode = query<HTMLElement>('engine-selected-hint')
const partsList = query<HTMLElement>('engine-parts-list')
const partsCount = query<HTMLElement>('engine-parts-count')
const explodeInput = query<HTMLInputElement>('engine-explode')
const explodeValue = query<HTMLOutputElement>('engine-explode-value')
const contextStatusNode = query<HTMLElement>('engine-context-status')
const resolutionNode = query<HTMLElement>('engine-resolution')
const frameNode = query<HTMLElement>('engine-frame-count')
const inputStatusNode = query<HTMLElement>('engine-input-status')
const animationStatusNode = query<HTMLElement>('engine-animation-status')
const socketStatusNode = query<HTMLElement>('engine-socket-status')
const physicsStatusNode = query<HTMLElement>('engine-physics-status')
const frameP95Node = query<HTMLElement>('engine-frame-p95')
const frameBudgetStatusNode = query<HTMLElement>('engine-frame-budget-status')

let runtimeStatus: RuntimeStatus = 'loading'
let contextStatus: RuntimeStatus = 'loading'
let contextRecoveries = 0
let viewport = { width: 0, height: 0, pixel_ratio: 1 }
let frame = 0
let lastInput: EngineProbeState['last_input'] = 'none'
let lastKey: string | null = null
let lastError: string | null = null
let presentationCamera: PresentationCamera = 'orbit'
let renderer: THREE.WebGLRenderer | null = null
let camera: THREE.PerspectiveCamera | null = null
let scene: THREE.Scene | null = null
let engine: WeaponryThreeJsEngine | null = null
let engineState: WeaponrySimulationState | null = null
let animation: WeaponryAnimationMixerAdapter | null = null
let fpsBinding: WeaponryFpsSocketBinding | null = null
let physics: WeaponryRapierPreviewBridge | null = null
let physicsSnapshot: WeaponryRapierPreviewSnapshot | null = null
let manifest: DeliveryManifest | null = null
let selectionHelper: THREE.Box3Helper | null = null
let lastTime = 0
let pointerDown = { x: 0, y: 0, moved: false, button: 0 }
let orbitYaw = 0.6
let orbitPitch = 0.12
let orbitDistance = 3.05
let paused = document.hidden || !document.hasFocus()
const frameBudget = createWeaponryFrameBudgetMonitor({ target_fps: 60, minimum_samples: 120, sample_capacity: 240 })

Object.defineProperty(window, '__WPN_ENGINE_PROBE__', {
  configurable: false,
  enumerable: false,
  get: () => makeProbe(),
})

void boot()

async function boot(): Promise<void> {
  setStatus('loading', 'Loading delivery')
  try {
    // Static file URLs let Vite package the immutable r8 inputs with the demo
    // while keeping the checked-in delivery as the single source of truth.
    const manifestUrl = new URL('../../deliveries/dragonfang-r8/delivery-manifest.json', import.meta.url)
    const glbUrl = new URL('../../deliveries/dragonfang-r8/dragonfang-kukri-r8-action-ready.glb', import.meta.url)
    const manifestResponse = await fetch(manifestUrl)
    if (!manifestResponse.ok) throw new Error(`ENGINE_DEMO_MANIFEST_FETCH_FAILED: ${manifestResponse.status}`)
    manifest = validateDeliveryManifest(await manifestResponse.json())
    const glbResponse = await fetch(glbUrl)
    if (!glbResponse.ok) throw new Error(`ENGINE_DEMO_GLB_FETCH_FAILED: ${glbResponse.status} ${glbUrl.href}`)
    const glbBytes = await glbResponse.arrayBuffer()
    if (glbBytes.byteLength !== manifest.delivery_glb.bytes) throw new Error('ENGINE_DEMO_GLB_SIZE_MISMATCH')
    const actualGlbSha256 = await sha256Hex(glbBytes)
    if (actualGlbSha256 !== manifest.delivery_glb.sha256) throw new Error('ENGINE_DEMO_GLB_SHA256_MISMATCH')

    // The shared engine consumes caller-owned bytes and owns simulation/state
    // transitions. The r8 root is normalized before the bridge captures rest.
    const loaded = await loadWeaponryR8Delivery(glbBytes)
    const bounds = new THREE.Box3().setFromObject(loaded.root)
    const center = bounds.getCenter(new THREE.Vector3())
    const size = bounds.getSize(new THREE.Vector3())
    const largestDimension = Math.max(size.x, size.y, size.z) || 1
    loaded.root.position.sub(center)
    const normalizedScale = 1.8 / largestDimension
    loaded.root.scale.setScalar(normalizedScale)
    engine = createWeaponryThreeJsEngineFromRoot(loaded.root)
    engineState = engine.createState({ initial_camera: 'fps' })
    if (engine.part_ids.join('\n') !== manifest.action_runtime.part_ids.join('\n')) {
      throw new Error('ENGINE_DEMO_PART_COVERAGE_MISMATCH: shared engine and manifest disagree')
    }
    const clipSet = createWeaponryAnimationClipSet(loaded.root)
    animation = createWeaponryAnimationMixerAdapter(loaded.root, clipSet)
    fpsBinding = createWeaponryRightHandGripBinding(engine.bridge.controller)
    fpsBinding.bind(createWeaponryRightHandTarget())
    await RAPIER.init()
    physics = createWeaponryRapierPreviewBridge(
      RAPIER as unknown as WeaponryRapierPreviewModule,
      engine.collider_proxies,
      {
        part_ids: engine.part_ids,
        uniform_scale: normalizedScale,
        initial_transform: {
          translation: loaded.root.position,
          rotation: loaded.root.quaternion,
        },
      },
    )
    physicsSnapshot = physics.snapshot()

    scene = new THREE.Scene()
    scene.background = new THREE.Color('#080c12')
    camera = new THREE.PerspectiveCamera(42, 1, .01, 100)
    scene.add(camera)
    scene.add(new THREE.HemisphereLight(0xd8e2ef, 0x10151c, 1.65))
    const key = new THREE.DirectionalLight(0xffe4b3, 3.1)
    key.position.set(-2.4, 3.2, 4.5)
    scene.add(key)
    const rim = new THREE.DirectionalLight(0x8caed2, 2.15)
    rim.position.set(2.8, 1.2, -3.2)
    scene.add(rim)
    const ground = new THREE.GridHelper(8, 16, 0x293341, 0x151d27)
    ground.position.y = -.58
    ground.material.opacity = .25
    ground.material.transparent = true
    scene.add(ground)
    scene.add(loaded.root)
    selectionHelper = new THREE.Box3Helper(new THREE.Box3(), 0xd4a45e)
    selectionHelper.visible = false
    scene.add(selectionHelper)

    renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: false, preserveDrawingBuffer: true, powerPreference: 'high-performance' })
    renderer.outputColorSpace = THREE.SRGBColorSpace
    renderer.toneMapping = THREE.ACESFilmicToneMapping
    renderer.toneMappingExposure = 1.12
    renderer.setClearColor('#080c12', 1)
    canvas.addEventListener('webglcontextlost', onContextLost, false)
    canvas.addEventListener('webglcontextrestored', onContextRestored, false)

    renderParts(engine.part_ids)
    setStatus('ready', 'Ready · interactive')
    setContextStatus('ready')
    loading.classList.add('is-hidden')
    updateViewport('resize')
    animate(0)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    lastError = message
    setStatus('error', 'Delivery unavailable')
    setContextStatus('error')
    loading.classList.add('is-hidden')
    loading.textContent = message
  }
}

function animate(now: number): void {
  requestAnimationFrame(animate)
  if (!renderer || !scene || !camera || !engine || !engineState || !animation || !physics) return
  const delta = lastTime === 0 ? 16 : Math.min(80, now - lastTime)
  lastTime = now
  if (paused || contextStatus === 'context-lost') return
  const frameStarted = performance.now()
  engineState = engine.advance(engineState, delta)
  const simulationEnded = performance.now()
  if (presentationCamera === 'orbit') {
    engine.apply(engineState)
    updateOrbitCamera()
  } else {
    engine.apply(engineState, camera)
  }
  animation.advance(delta / 1_000)
  const animationEnded = performance.now()
  const rootWorldPosition = engine.bridge.root.getWorldPosition(new THREE.Vector3())
  const rootWorldRotation = engine.bridge.root.getWorldQuaternion(new THREE.Quaternion())
  physics.syncRootTransform({ translation: rootWorldPosition, rotation: rootWorldRotation })
  physicsSnapshot = physics.step()
  const physicsEnded = performance.now()
  updateSelectionHelper()
  renderer.render(scene, camera)
  const renderEnded = performance.now()
  frameBudget.record({
    simulation_ms: simulationEnded - frameStarted,
    animation_ms: animationEnded - simulationEnded,
    physics_ms: physicsEnded - animationEnded,
    render_ms: renderEnded - physicsEnded,
    total_ms: renderEnded - frameStarted,
  })
  frame += 1
  frameNode.textContent = String(frame)
  if (frame % 8 === 0) updateProbeUi()
}

function updateOrbitCamera(): void {
  if (!camera) return
  const target = new THREE.Vector3(0, 0, 0)
  const cosPitch = Math.cos(orbitPitch)
  camera.position.set(
    target.x + Math.sin(orbitYaw) * cosPitch * orbitDistance,
    target.y + Math.sin(orbitPitch) * orbitDistance,
    target.z + Math.cos(orbitYaw) * cosPitch * orbitDistance,
  )
  camera.fov = 32
  camera.near = .01
  camera.far = 100
  camera.lookAt(target)
  camera.updateProjectionMatrix()
}

function updateViewport(input: EngineProbeState['last_input'] = 'resize'): void {
  if (!renderer || !camera) return
  const width = Math.max(1, stage.clientWidth)
  const height = Math.max(1, stage.clientHeight)
  const pixelRatio = Math.min(window.devicePixelRatio || 1, 2)
  renderer.setPixelRatio(pixelRatio)
  renderer.setSize(width, height, false)
  camera.aspect = width / height
  camera.updateProjectionMatrix()
  viewport = { width, height, pixel_ratio: pixelRatio }
  resolutionNode.textContent = `${width} × ${height}`
  lastInput = input
  updateProbeUi()
}

function setPresentationCamera(next: PresentationCamera): void {
  presentationCamera = next
  if (engine && engineState) {
    if (next === 'inspect' && engineState.camera !== 'inspect') {
      engineState = engine.dispatch(engineState, 'inspect', engineState.time_ms)
      animation?.play('inspect')
    }
    if (next === 'fps' && engineState.camera !== 'fps') {
      engineState = engine.dispatch(engineState, 'idle', engineState.time_ms)
      animation?.play('idle')
    }
  }
  if (next === 'orbit') resetOrbitCamera()
  updateCameraButtons()
  lastInput = 'pointer'
  updateProbeUi()
}

function resetCamera(): void {
  setPresentationCamera('orbit')
  if (engine && engineState) engineState = setWeaponrySelectedPart(engineState, null)
  animation?.reset()
  updateSelectionUi()
  lastInput = 'keyboard'
  updateProbeUi()
}

function resetOrbitCamera(): void {
  orbitYaw = .6
  orbitPitch = .12
  orbitDistance = 3.05
}

function triggerAction(next: WeaponryAction): void {
  if (!engine || !engineState) return
  engineState = engine.dispatch(engineState, next, engineState.time_ms)
  animation?.play(engineState.interaction)
  presentationCamera = engineState.camera
  updateCameraButtons()
  document.querySelectorAll<HTMLElement>('[data-action]').forEach((button) => button.classList.toggle('is-active', button.dataset.action === next))
  lastInput = lastInput === 'keyboard' ? 'keyboard' : 'pointer'
  updateProbeUi()
}

function updateCameraButtons(): void {
  document.querySelectorAll<HTMLElement>('[data-camera]').forEach((button) => {
    button.classList.toggle('is-active', button.dataset.camera === presentationCamera)
  })
}

function setExploded(next: number): void {
  if (!engine || !engineState) return
  engineState = engine.setExploded(engineState, Math.min(1, Math.max(0, next)))
  const percent = Math.round(engineState.exploded * 100)
  explodeInput.value = String(percent)
  explodeValue.value = `${percent}%`
  updateProbeUi()
}

function selectPart(partId: string | null, source: 'pointer' | 'keyboard' = 'pointer'): void {
  if (!engine || !engineState) return
  if (partId && engineState.visible_parts[partId] === false) return
  engineState = setWeaponrySelectedPart(engineState, partId)
  lastInput = source
  updateSelectionUi()
  updateProbeUi()
}

function updateSelectionUi(): void {
  const selectedPartId = engineState?.selected_part_id ?? null
  document.querySelectorAll<HTMLElement>('[data-part-id]').forEach((row) => {
    row.classList.toggle('is-selected', row.dataset.partId === selectedPartId)
  })
  if (!selectedPartId) {
    selectedNode.textContent = 'Nothing selected'
    selectedHintNode.textContent = 'Pick a part in the viewport'
  } else {
    selectedNode.textContent = selectedPartId
    selectedHintNode.textContent = engineState?.visible_parts[selectedPartId] === false ? 'Hidden · stable part ID' : 'Visible · stable part ID'
  }
}

function updateSelectionHelper(): void {
  const selectedPartId = engineState?.selected_part_id ?? null
  const currentEngine = engine
  const root = currentEngine?.bridge.root
  if (!selectionHelper || !root || !selectedPartId || engineState?.visible_parts[selectedPartId] === false) {
    if (selectionHelper) selectionHelper.visible = false
    return
  }
  const pivot = currentEngine.bridge.controller.partPivots.get(selectedPartId)
  if (!pivot) return
  selectionHelper.box.copy(new THREE.Box3().setFromObject(pivot))
  selectionHelper.visible = true
}

function renderParts(partIds: readonly string[]): void {
  partsCount.textContent = `${partIds.length} / ${partIds.length}`
  partsList.replaceChildren()
  for (const partId of partIds) {
    const row = document.createElement('div')
    row.className = 'part-row'
    row.dataset.partId = partId
    row.setAttribute('role', 'listitem')
    row.tabIndex = 0
    const checkbox = document.createElement('input')
    checkbox.type = 'checkbox'
    checkbox.checked = true
    checkbox.dataset.testid = `part-visibility-${partId}`
    checkbox.setAttribute('aria-label', `Toggle ${partId}`)
    checkbox.addEventListener('click', (event) => event.stopPropagation())
    checkbox.addEventListener('change', () => {
      if (!engine || !engineState) return
      engineState = engine.setPartVisible(engineState, partId, checkbox.checked)
      row.classList.toggle('is-hidden', !checkbox.checked)
      if (!checkbox.checked && engineState.selected_part_id === partId) selectPart(null, 'pointer')
      updatePartCount()
      updateProbeUi()
    })
    const name = document.createElement('span')
    name.className = 'part-row-label'
    name.textContent = partId
    row.append(checkbox, name)
    row.addEventListener('click', () => selectPart(partId))
    row.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault()
        selectPart(partId, 'keyboard')
      }
    })
    partsList.append(row)
  }
  updateSelectionUi()
}

function updatePartCount(): void {
  if (!engineState || !engine) return
  const visible = engine.part_ids.filter((id) => engineState?.visible_parts[id] !== false).length
  partsCount.textContent = `${visible} / ${engine.part_ids.length}`
}

function pickAt(clientX: number, clientY: number): void {
  if (!renderer || !camera || !engine || !engineState) return
  const rect = canvas.getBoundingClientRect()
  const point = new THREE.Vector2(
    ((clientX - rect.left) / rect.width) * 2 - 1,
    -((clientY - rect.top) / rect.height) * 2 + 1,
  )
  const raycaster = new THREE.Raycaster()
  raycaster.setFromCamera(point, camera)
  const picked = engine.pick(engineState, raycaster)
  engineState = picked.hit ? picked.state : setWeaponrySelectedPart(picked.state, null)
  selectPart(engineState.selected_part_id, 'pointer')
}

function onContextLost(event: Event): void {
  event.preventDefault()
  setStatus('context-lost', 'Context lost · waiting for recovery')
  setContextStatus('context-lost')
}

function onContextRestored(): void {
  contextRecoveries += 1
  setStatus('recovering', 'Context restored · recovering')
  setContextStatus('recovering')
  window.setTimeout(() => {
    if (!renderer) return
    updateViewport('resize')
    setStatus('ready', 'Ready · context recovered')
    setContextStatus('ready')
  }, 80)
}

function recoverContext(): void {
  if (!renderer) return
  if (contextStatus !== 'context-lost') {
    setStatus('ready', 'Ready · context healthy')
    setContextStatus('ready')
    return
  }
  const extension = renderer.getContext().getExtension('WEBGL_lose_context') as { restoreContext?: () => void } | null
  if (extension?.restoreContext) {
    setStatus('recovering', 'Requesting context recovery')
    setContextStatus('recovering')
    extension.restoreContext()
  } else {
    onContextRestored()
  }
}

function setStatus(next: RuntimeStatus, text: string): void {
  runtimeStatus = next
  statusNode.textContent = text
  statusLight.className = `status-light ${next === 'ready' ? 'is-ready' : next === 'error' || next === 'context-lost' ? 'is-error' : ''}`
  updateProbeUi()
}

function setContextStatus(next: RuntimeStatus): void {
  contextStatus = next
  contextStatusNode.textContent = next === 'context-lost' ? 'lost' : next
  contextStatusNode.className = next === 'ready' ? 'is-ok' : next === 'error' || next === 'context-lost' ? 'is-error' : 'is-warn'
  updateProbeUi()
}

function updateProbeUi(): void {
  updateSelectionUi()
  updatePartCount()
  if (engineState) {
    const percent = Math.round(engineState.exploded * 100)
    explodeInput.value = String(percent)
    explodeValue.value = `${percent}%`
    const action = engineState.interaction
    document.querySelectorAll<HTMLElement>('[data-action]').forEach((button) => button.classList.toggle('is-active', button.dataset.action === action))
  }
  const animationSnapshot = animation?.snapshot() ?? null
  const bindingSnapshot = fpsBinding?.snapshot() ?? null
  const budgetSnapshot = frameBudget.snapshot()
  animationStatusNode.textContent = animationSnapshot ? `${animationSnapshot.action} · ${animationSnapshot.status}` : 'waiting'
  socketStatusNode.textContent = bindingSnapshot?.status.toLowerCase() ?? 'waiting'
  physicsStatusNode.textContent = physicsSnapshot ? `${physicsSnapshot.collider_count} colliders` : 'waiting'
  frameP95Node.textContent = budgetSnapshot.sample_count === 0 ? '—' : `${budgetSnapshot.total_ms.p95.toFixed(2)} ms`
  frameBudgetStatusNode.textContent = budgetSnapshot.status === 'WARMING_UP'
    ? `warming ${budgetSnapshot.sample_count}/${budgetSnapshot.minimum_samples}`
    : budgetSnapshot.status === 'MEASURED_WITHIN_BUDGET' ? 'within 60 fps CPU budget' : 'over 60 fps CPU budget'
  frameBudgetStatusNode.className = budgetSnapshot.status === 'MEASURED_WITHIN_BUDGET' ? 'is-ok' : budgetSnapshot.status === 'MEASURED_OVER_BUDGET' ? 'is-warn' : ''
}

function makeProbe(): EngineProbeState {
  const snapshot = engine && engineState ? engine.snapshot(engineState) : null
  const action = snapshot?.interaction ?? 'idle'
  const visiblePartIds = snapshot ? snapshot.part_ids.filter((id) => snapshot.visible_parts[id] !== false) : []
  return freezeProbe({
    schema_version: 'WeaponryThreeJsEngineProbe@1',
    status: runtimeStatus,
    asset_id: manifest?.asset_id ?? null,
    asset_version: manifest?.asset_version ?? null,
    delivery_glb_sha256: manifest?.delivery_glb.sha256 ?? null,
    camera_mode: presentationCamera,
    action,
    interaction_pose: action,
    interaction_pose_playing: action !== 'idle' && action !== 'inspect' && (snapshot?.action_progress ?? 0) < 1,
    interaction_pose_progress: snapshot?.action_progress ?? 0,
    pose_runtime: 'prototype',
    animation_snapshot: animation?.snapshot() ?? null,
    fps_binding_snapshot: fpsBinding?.snapshot() ?? null,
    physics_snapshot: physicsSnapshot,
    frame_budget: frameBudget.snapshot(),
    paused,
    selected_part_id: snapshot?.selected_part_id ?? null,
    visible_part_ids: visiblePartIds,
    exploded_amount: snapshot?.exploded ?? 0,
    context_status: contextStatus,
    context_recoveries: contextRecoveries,
    viewport,
    frame,
    last_input: lastInput,
    last_key: lastKey,
    engine_snapshot: snapshot,
    error: lastError,
  })
}

function freezeProbe(value: EngineProbeState): EngineProbeState {
  return Object.freeze({
    ...value,
    visible_part_ids: Object.freeze([...value.visible_part_ids]),
    viewport: Object.freeze({ ...value.viewport }),
    engine_snapshot: value.engine_snapshot ? Object.freeze({ ...value.engine_snapshot }) : null,
    animation_snapshot: value.animation_snapshot ? Object.freeze({ ...value.animation_snapshot }) : null,
    fps_binding_snapshot: value.fps_binding_snapshot ? Object.freeze({ ...value.fps_binding_snapshot }) : null,
    physics_snapshot: value.physics_snapshot ? Object.freeze({ ...value.physics_snapshot }) : null,
    frame_budget: Object.freeze({ ...value.frame_budget }),
  })
}

function query<T extends HTMLElement>(id: string): T {
  const node = document.querySelector<T>(`[data-testid="${id}"]`) ?? document.getElementById(id) as T | null
  if (!node) throw new Error(`ENGINE_DEMO_ELEMENT_MISSING: ${id}`)
  return node
}

function validateDeliveryManifest(value: unknown): DeliveryManifest {
  if (!value || typeof value !== 'object') throw new Error('ENGINE_DEMO_MANIFEST_INVALID: object required')
  const candidate = value as Partial<DeliveryManifest>
  if (candidate.asset_id !== EXPECTED_ASSET_ID || candidate.asset_version !== EXPECTED_ASSET_VERSION) {
    throw new Error('ENGINE_DEMO_MANIFEST_IDENTITY_MISMATCH')
  }
  if (!candidate.delivery_glb || candidate.delivery_glb.path !== EXPECTED_GLB_PATH) {
    throw new Error('ENGINE_DEMO_MANIFEST_GLB_PATH_MISMATCH')
  }
  if (!/^[a-f0-9]{64}$/.test(candidate.delivery_glb.sha256)
    || !Number.isInteger(candidate.delivery_glb.bytes) || candidate.delivery_glb.bytes <= 0) {
    throw new Error('ENGINE_DEMO_MANIFEST_GLB_IDENTITY_INVALID')
  }
  if (!candidate.action_runtime || !Array.isArray(candidate.action_runtime.part_ids)
    || candidate.action_runtime.part_ids.length !== 13
    || new Set(candidate.action_runtime.part_ids).size !== candidate.action_runtime.part_ids.length) {
    throw new Error('ENGINE_DEMO_MANIFEST_PARTS_INVALID')
  }
  if (candidate.dependency_lock?.three_version !== EXPECTED_THREE_VERSION) {
    throw new Error('ENGINE_DEMO_THREE_VERSION_MISMATCH')
  }
  return candidate as DeliveryManifest
}

async function sha256Hex(bytes: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', bytes)
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('')
}

window.addEventListener('resize', () => updateViewport('resize'))
window.addEventListener('blur', () => {
  paused = true
  inputStatusNode.textContent = 'paused'
  updateProbeUi()
})
window.addEventListener('focus', () => {
  paused = document.hidden
  lastTime = 0
  inputStatusNode.textContent = paused ? 'paused' : 'ready'
  updateProbeUi()
})
document.addEventListener('visibilitychange', () => {
  paused = document.hidden
  lastTime = 0
  inputStatusNode.textContent = paused ? 'paused' : 'ready'
  updateProbeUi()
})
window.addEventListener('keydown', (event) => {
  lastKey = event.key
  lastInput = 'keyboard'
  const key = event.key.toLowerCase()
  if (engine && engineState) {
    const mapped = engine.input(engineState, { code: event.code, phase: 'pressed', repeat: event.repeat })
    if (mapped !== engineState) {
      engineState = mapped
      animation?.play(engineState.interaction)
      presentationCamera = engineState.camera
      updateCameraButtons()
      updateProbeUi()
      return
    }
  }
  if (key === '1') triggerAction('idle')
  else if (key === '2') triggerAction('light')
  else if (key === '3') triggerAction('heavy')
  else if (key === 'i') triggerAction('inspect')
  else if (key === 's' || key === 'q') triggerAction('sheath')
  else if (key === 'r') resetCamera()
  else if (key === 'x') setExploded((engineState?.exploded ?? 0) > .01 ? 0 : .6)
  else if (key === 'escape') selectPart(null, 'keyboard')
  else if (key === 'arrowleft') orbitYaw -= .08
  else if (key === 'arrowright') orbitYaw += .08
  else if (key === 'arrowup') orbitPitch = Math.min(.9, orbitPitch + .05)
  else if (key === 'arrowdown') orbitPitch = Math.max(-.9, orbitPitch - .05)
  updateProbeUi()
})

canvas.addEventListener('pointerdown', (event) => {
  pointerDown = { x: event.clientX, y: event.clientY, moved: false, button: event.button }
  canvas.setPointerCapture(event.pointerId)
  canvas.classList.add('is-dragging')
  lastInput = 'pointer'
})
canvas.addEventListener('pointermove', (event) => {
  if (!canvas.hasPointerCapture(event.pointerId)) return
  if (presentationCamera !== 'orbit') return
  const dx = event.clientX - pointerDown.x
  const dy = event.clientY - pointerDown.y
  if (Math.abs(dx) + Math.abs(dy) < 2) return
  pointerDown.moved = true
  orbitYaw -= dx * .008
  orbitPitch = Math.max(-1.05, Math.min(1.05, orbitPitch + dy * .006))
  pointerDown.x = event.clientX
  pointerDown.y = event.clientY
  lastInput = 'pointer'
  inputStatusNode.textContent = 'orbiting'
  updateProbeUi()
})
canvas.addEventListener('pointerup', (event) => {
  canvas.releasePointerCapture(event.pointerId)
  canvas.classList.remove('is-dragging')
  inputStatusNode.textContent = 'ready'
  if (!pointerDown.moved && presentationCamera === 'fps' && pointerDown.button === 0) {
    triggerAction('light')
  } else if (!pointerDown.moved) {
    pickAt(event.clientX, event.clientY)
  }
  lastInput = 'pointer'
  updateProbeUi()
})
canvas.addEventListener('pointercancel', () => {
  canvas.classList.remove('is-dragging')
  inputStatusNode.textContent = 'ready'
})
canvas.addEventListener('wheel', (event) => {
  event.preventDefault()
  orbitDistance = Math.max(1.35, Math.min(5.5, orbitDistance * Math.exp(event.deltaY * .001)))
  lastInput = 'wheel'
  inputStatusNode.textContent = 'zoom'
  window.setTimeout(() => { inputStatusNode.textContent = 'ready' }, 180)
  updateProbeUi()
}, { passive: false })
canvas.addEventListener('contextmenu', (event) => {
  event.preventDefault()
  if (presentationCamera === 'fps') triggerAction('heavy')
})

document.querySelectorAll<HTMLElement>('[data-camera]').forEach((button) => {
  button.addEventListener('click', () => setPresentationCamera(button.dataset.camera as PresentationCamera))
})
document.querySelectorAll<HTMLElement>('[data-action]').forEach((button) => {
  button.addEventListener('click', () => triggerAction(button.dataset.action as WeaponryAction))
})
query<HTMLButtonElement>('camera-reset').addEventListener('click', resetCamera)
query<HTMLButtonElement>('engine-context-recover').addEventListener('click', recoverContext)
explodeInput.addEventListener('input', () => setExploded(Number(explodeInput.value) / 100))
window.addEventListener('pagehide', () => {
  animation?.dispose()
  physics?.dispose()
  renderer?.dispose()
})
