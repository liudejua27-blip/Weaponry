import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { RoomEnvironment } from 'three/examples/jsm/environments/RoomEnvironment.js'
import { TransformControls } from 'three/examples/jsm/controls/TransformControls.js'
import type { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import type { ModuleAssetRecord, ModuleGraphRecord, QualityFinding, Transform } from '../../shared/types'
import { buildShapeProgramPreview } from './shapeProgramPreview.js'
import type { ViewportMeasurementPoint } from './viewportMeasurementPresentation.js'

export type { ViewportMeasurementPoint } from './viewportMeasurementPresentation.js'

type CameraView = 'iso' | 'front' | 'top' | 'right'
type LightPreset = 'cad_neutral' | 'soft_studio' | 'concept_contrast'
type TransformTool = 'none' | 'translate' | 'rotate' | 'scale'
type BlockoutGlbKind =
  | 'compiled_agent_pbr'
  | 'compiled_agent_preview_pbr'
  | 'compiled_agent_production_pbr'
  | 'external_reference'
  | null
type Graph = NonNullable<ModuleGraphRecord>['graph']
const GLB_METERS_TO_WORKBENCH_MILLIMETERS = 1000
const BLOCKOUT_DISPLAY_DIAGONAL_MM = 520
const BLOCKOUT_FRAME_TARGET_NDC = 0.84
const DEFAULT_SCENE_FOG_NEAR_MM = 300
const DEFAULT_SCENE_FOG_FAR_MM = 820
const PBR_TEXTURE_ANISOTROPY_CAP = 4
// Must match ForgeCADVisualEnvironment@1 written into every current ShapeProgram
// GLB.  The viewport has one renderer/context; this profile only configures its
// existing RoomEnvironment/PMREM scene and never creates asset state.
const FORGECAD_STUDIO_MANIFEST = {
  schema_version: 'ForgeCADVisualEnvironment@1',
  environment_id: 'env_forgecad_room_studio_v1',
  environment_kind: 'procedural_studio',
  source: 'forgecad_builtin',
  license: 'not_applicable',
  color_workflow: 'linear_srgb',
  output_color_space: 'srgb',
  tone_mapping: 'aces_filmic',
  tone_mapping_exposure: 0.86,
  contact_shadows: true,
  pmrem: { near: 0.04, cube_size: 128 },
  cad_neutral_lighting: {
    background: '#0b1420',
    hemisphere: { sky: '#eef6ff', ground: '#111820', intensity: 1.45 },
    ambient: { color: '#8aa0b8', intensity: 0.24 },
    key: { color: '#f7fbff', intensity: 3.6, position: [150, 210, 160] as [number, number, number] },
    rim: { color: '#91b6d9', intensity: 0.95, position: [-160, 110, -120] as [number, number, number] },
    warm_rim: { color: '#ffd0b5', intensity: 0.28, position: [110, 20, -190] as [number, number, number] },
    floor: { kind: 'shadow_catcher', color: '#000000', opacity: 0.16, radius_ratio: 1.1 },
  },
  camera_views: {
    iso: { direction: [-0.9, 0.85, 1.55] as [number, number, number], distance_ratio: 0.98, fov_degrees: 38 },
  },
} as const
const FORGECAD_STUDIO_ENVIRONMENT_SHA256 = '291b13f7d1606bd3c180a3fb9850538f5d23208086d7f8488c2214fa59061042'
// GLB material data and its readback remain immutable.  This bounded table
// only tunes the existing RoomEnvironment reflection in the one workbench
// renderer, so clearcoat/aluminium highlights do not erase the authored base
// colour hierarchy.  Unknown/external material ids use the conservative
// default rather than being recoloured or assigned a ForgeCAD identity.
const FORGECAD_PBR_ENVIRONMENT_INTENSITY_BY_MATERIAL_ID: Readonly<Record<string, number>> = {
  mat_primary: 0.5,
  mat_aluminum: 0.42,
  mat_composite: 0.25,
  mat_rubber: 0.14,
  mat_dark_glass: 0.4,
  mat_emissive_blue: 0.2,
  mat_automotive_paint: 0.52,
}
const FORGECAD_PBR_DEFAULT_ENVIRONMENT_INTENSITY = 0.45
// Packaged arm MVP QA asks the mounted viewport to copy pixels through this
// same-realm event.  It is deliberately not a product API: the event carries
// no model/state data, is only listened to by the existing host, and its
// response is fulfilled immediately after this renderer has drawn a frame.
// This keeps preserveDrawingBuffer disabled for normal interactive rendering.
const FORGECAD_QA_VIEWPORT_CAPTURE_EVENT = 'forgecad:qa-capture-viewport@1'
let viewportRendererGeneration = 0
let activeViewportContexts = 0

type ModuleGraphViewportProps = {
  graphRecord: ModuleGraphRecord | null
  modules: ModuleAssetRecord[]
  cameraView: CameraView
  lightPreset: LightPreset
  showGrid: boolean
  wireframe: boolean
  xRay: boolean
  sectionEnabled: boolean
  sectionOffset: number
  selectedNodeId: string
  hiddenNodeIds: string[]
  focusNodeId: string | null
  qualityHighlightNodeIds: string[]
  qualityGeometryRefs: NonNullable<QualityFinding['geometry_refs']>
  blockoutGlbBase64: string | ArrayBuffer | null
  blockoutGlbKind: BlockoutGlbKind
  blockoutShapeProgram: Record<string, unknown> | null
  blockoutMaterialOverride: string | null
  referenceImage: {
    url: string
    evidenceId: string
    sourceObjectSha256: string
    referenceClass: 'single_image' | 'multi_view_contact_sheet'
  } | null
  onReferenceImageDisplayFailure: () => void
  selectedAgentPartId: string | null
  hiddenAgentPartIds: string[]
  isolatedAgentPartId: string | null
  lockedAgentPartIds: string[]
  showConnectors: boolean
  explodeFactor: number
  ghostPreview: boolean
  transformTool: TransformTool
  transformSpace: 'world' | 'local'
  snapEnabled: boolean
  measureEnabled: boolean
  getModuleFileUrl: (moduleId: string) => string
  onSelectNode: (nodeId: string) => void
  onDropModule: (nodeId: string, moduleId: string) => void
  onTransformCommit: (nodeId: string, transform: Transform) => void
  onMeasurePoint: (point: ViewportMeasurementPoint) => void
}

type ViewportRuntime = {
  scene: THREE.Scene
  camera: THREE.PerspectiveCamera
  renderer: THREE.WebGLRenderer
  controls: OrbitControls
  transformControls: TransformControls
  transformHelper: THREE.Object3D
  sectionPlane: THREE.Plane
  sectionHelper: THREE.PlaneHelper
  grid: THREE.GridHelper
  axes: THREE.AxesHelper
  displayFloor: THREE.Mesh<THREE.CircleGeometry, THREE.ShadowMaterial>
  moduleRoot: THREE.Group
  blockoutRoot: THREE.Group
  referenceImageRoot: THREE.Group
  qualityRoot: THREE.Group
  connectorGeometry: THREE.SphereGeometry
  connectorMaterials: { exclusive: THREE.MeshBasicMaterial; shared: THREE.MeshBasicMaterial }
  hemisphereLight: THREE.HemisphereLight
  ambientLight: THREE.AmbientLight
  keyLight: THREE.DirectionalLight
  rimLight: THREE.DirectionalLight
  warmRimLight: THREE.DirectionalLight
  pmremGenerator: THREE.PMREMGenerator
  studioEnvironment: THREE.WebGLRenderTarget
  studioEnvironmentScene: RoomEnvironment
  nodeObjects: Map<string, THREE.Group>
  moduleCache: Map<string, Promise<THREE.Group>>
  graph: Graph | null
  modulesById: Map<string, ModuleAssetRecord>
  activeBlockoutPreview: {
    source: THREE.Object3D
    displayScale: number
    displayDiagonalMm: number
    sourceBoundsMm: number[]
  } | null
  blockoutReplacementGeneration: number
  disposedBlockoutAssetCount: number
  scheduleRender: () => void
}

type QaViewportCapture = {
  width: number
  height: number
  pixels: Uint8Array
}

type QaViewportCaptureRequest = {
  viewport: HTMLElement
  resolve: (capture: QaViewportCapture) => void
  reject: (error: Error) => void
}

export function ModuleGraphViewport(props: ModuleGraphViewportProps) {
  const hostRef = useRef<HTMLDivElement | null>(null)
  const runtimeRef = useRef<ViewportRuntime | null>(null)
  const propsRef = useRef(props)
  propsRef.current = props
  const [loadState, setLoadState] = useState<'empty' | 'loading' | 'ready' | 'failed'>(
    props.graphRecord ? 'loading' : 'empty',
  )
  const [loadMessage, setLoadMessage] = useState('还没有 Agent 资产；在底部描述你想生成的模型。')
  const [blockoutLoadState, setBlockoutLoadState] = useState<'empty' | 'loading' | 'ready' | 'failed'>('empty')
  const [blockoutLoadMessage, setBlockoutLoadMessage] = useState('')
  const [blockoutPreviewPrimitiveKinds, setBlockoutPreviewPrimitiveKinds] = useState<string[]>([])
  const [blockoutRenderSource, setBlockoutRenderSource] = useState<'empty' | 'glb_pbr' | 'external_reference' | 'shape_program_fallback'>('empty')
  const [blockoutEmbeddedPbrMaterialCount, setBlockoutEmbeddedPbrMaterialCount] = useState(0)
  const [referenceImageLoadState, setReferenceImageLoadState] = useState<'empty' | 'loading' | 'ready' | 'failed'>('empty')
  const [referenceImageLoadMessage, setReferenceImageLoadMessage] = useState('')

  // Renderer, Scene and camera exist for the lifetime of the panel only. Selection,
  // overlay and wireframe updates below never destroy the WebGL context.
  useEffect(() => {
    const host = hostRef.current
    if (!host) return
    const neutralLighting = FORGECAD_STUDIO_MANIFEST.cad_neutral_lighting
    const scene = new THREE.Scene()
    scene.background = new THREE.Color(neutralLighting.background)
    scene.fog = new THREE.Fog(neutralLighting.background, DEFAULT_SCENE_FOG_NEAR_MM, DEFAULT_SCENE_FOG_FAR_MM)
    const camera = new THREE.PerspectiveCamera(FORGECAD_STUDIO_MANIFEST.camera_views.iso.fov_degrees, 1, 0.01, 100000)
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false })
    renderer.localClippingEnabled = true
    viewportRendererGeneration += 1
    activeViewportContexts += 1
    host.dataset.rendererGeneration = String(viewportRendererGeneration)
    host.dataset.activeWebglContexts = String(activeViewportContexts)
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    renderer.outputColorSpace = THREE.SRGBColorSpace
    renderer.toneMapping = THREE.ACESFilmicToneMapping
    renderer.toneMappingExposure = FORGECAD_STUDIO_MANIFEST.tone_mapping_exposure
    renderer.shadowMap.enabled = true
    renderer.shadowMap.type = THREE.PCFSoftShadowMap
    host.appendChild(renderer.domElement)
    const pmremGenerator = new THREE.PMREMGenerator(renderer)
    const studioEnvironmentScene = new RoomEnvironment()
    const studioEnvironment = pmremGenerator.fromScene(
      studioEnvironmentScene,
      FORGECAD_STUDIO_MANIFEST.pmrem.near,
      32,
      FORGECAD_STUDIO_MANIFEST.pmrem.cube_size,
    )
    scene.environment = studioEnvironment.texture
    scene.userData.forgecadVisualEnvironment = FORGECAD_STUDIO_MANIFEST.environment_id
    host.dataset.visualEnvironmentId = FORGECAD_STUDIO_MANIFEST.environment_id
    host.dataset.visualEnvironmentSha256 = FORGECAD_STUDIO_ENVIRONMENT_SHA256

    const controls = new OrbitControls(camera, renderer.domElement)
    controls.enableDamping = false
    controls.minDistance = 0.01
    controls.maxDistance = 100000
    const hemisphereLight = new THREE.HemisphereLight(
      neutralLighting.hemisphere.sky,
      neutralLighting.hemisphere.ground,
      neutralLighting.hemisphere.intensity,
    )
    scene.add(hemisphereLight)
    const ambientLight = new THREE.AmbientLight(neutralLighting.ambient.color, neutralLighting.ambient.intensity)
    scene.add(ambientLight)
    const keyLight = new THREE.DirectionalLight(neutralLighting.key.color, neutralLighting.key.intensity)
    keyLight.position.set(...neutralLighting.key.position)
    keyLight.castShadow = true
    keyLight.shadow.mapSize.set(2048, 2048)
    keyLight.shadow.camera.near = 1
    keyLight.shadow.camera.far = 900
    keyLight.shadow.camera.left = -360
    keyLight.shadow.camera.right = 360
    keyLight.shadow.camera.top = 360
    keyLight.shadow.camera.bottom = -360
    scene.add(keyLight)
    const rimLight = new THREE.DirectionalLight(neutralLighting.rim.color, neutralLighting.rim.intensity)
    rimLight.position.set(...neutralLighting.rim.position)
    scene.add(rimLight)
    const warmRimLight = new THREE.DirectionalLight(neutralLighting.warm_rim.color, neutralLighting.warm_rim.intensity)
    warmRimLight.position.set(...neutralLighting.warm_rim.position)
    scene.add(warmRimLight)
    const displayFloor = new THREE.Mesh(
      new THREE.CircleGeometry(1, 96),
      new THREE.ShadowMaterial({
        color: neutralLighting.floor.color,
        transparent: true,
        opacity: neutralLighting.floor.opacity,
      }),
    )
    displayFloor.rotation.x = -Math.PI / 2
    displayFloor.receiveShadow = true
    displayFloor.name = 'DisplayFloor'
    scene.add(displayFloor)
    const grid = new THREE.GridHelper(420, 42, '#28466a', '#223244')
    scene.add(grid)
    const axes = new THREE.AxesHelper(28)
    axes.name = 'ForgeCADCoordinateAxes'
    scene.add(axes)
    const sectionPlane = new THREE.Plane(new THREE.Vector3(1, 0, 0), 0)
    const sectionHelper = new THREE.PlaneHelper(sectionPlane, 180, '#f0b84b')
    sectionHelper.visible = false
    scene.add(sectionHelper)
    const transformControls = new TransformControls(camera, renderer.domElement)
    const transformHelper = transformControls.getHelper()
    transformHelper.visible = false
    scene.add(transformHelper)
    const moduleRoot = new THREE.Group()
    moduleRoot.name = 'ModuleGraphRoot'
    scene.add(moduleRoot)
    const blockoutRoot = new THREE.Group()
    blockoutRoot.name = 'AgentBlockoutPreviewRoot'
    scene.add(blockoutRoot)
    const referenceImageRoot = new THREE.Group()
    referenceImageRoot.name = 'ReadOnlyReferenceImageRoot'
    scene.add(referenceImageRoot)
    const qualityRoot = new THREE.Group()
    qualityRoot.name = 'QualityGeometryOverlay'
    scene.add(qualityRoot)
    const connectorGeometry = new THREE.SphereGeometry(1, 16, 12)
    const connectorMaterials = {
      exclusive: new THREE.MeshBasicMaterial({ color: '#42c8ff', depthTest: false, transparent: true, opacity: 0.9 }),
      shared: new THREE.MeshBasicMaterial({ color: '#f1b84b', depthTest: false, transparent: true, opacity: 0.9 }),
    }

    let frame = 0
    let pendingQaCapture: QaViewportCaptureRequest | null = null
    const render = () => {
      frame = 0
      renderer.render(scene, camera)
      // The default WebGL framebuffer is allowed to be cleared after
      // presentation because preserveDrawingBuffer stays false.  Read it in
      // this exact render callback instead of later from the QA module.
      const capture = pendingQaCapture
      if (capture) {
        pendingQaCapture = null
        try {
          const context = renderer.getContext()
          const width = context.drawingBufferWidth
          const height = context.drawingBufferHeight
          if (width < 320 || height < 240 || width * height > 8_400_000) {
            throw new Error('QA_V3_VIEWPORT_SCREENSHOT_CANVAS_INVALID')
          }
          const pixels = new Uint8Array(width * height * 4)
          context.readPixels(0, 0, width, height, context.RGBA, context.UNSIGNED_BYTE, pixels)
          if (context.getError() !== context.NO_ERROR) {
            throw new Error('QA_V3_VIEWPORT_SCREENSHOT_READBACK_FAILED')
          }
          capture.resolve({ width, height, pixels })
        } catch (error) {
          capture.reject(error instanceof Error ? error : new Error('QA_V3_VIEWPORT_SCREENSHOT_READBACK_FAILED'))
        }
      }
      host.dataset.rendererGeometries = String(renderer.info.memory.geometries)
      host.dataset.rendererTextures = String(renderer.info.memory.textures)
      host.dataset.rendererDrawCalls = String(renderer.info.render.calls)
      host.dataset.rendererTriangles = String(renderer.info.render.triangles)
      host.dataset.rendererLines = String(renderer.info.render.lines)
    }
    const scheduleRender = () => {
      if (!frame) frame = requestAnimationFrame(render)
    }
    const runtime: ViewportRuntime = {
      scene,
      camera,
      renderer,
      controls,
      transformControls,
      transformHelper,
      sectionPlane,
      sectionHelper,
      grid,
      axes,
      displayFloor,
      moduleRoot,
      blockoutRoot,
      referenceImageRoot,
      qualityRoot,
      connectorGeometry,
      connectorMaterials,
      hemisphereLight,
      ambientLight,
      keyLight,
      rimLight,
      warmRimLight,
      pmremGenerator,
      studioEnvironment,
      studioEnvironmentScene,
      nodeObjects: new Map(),
      moduleCache: new Map(),
      graph: null,
      modulesById: new Map(),
      activeBlockoutPreview: null,
      blockoutReplacementGeneration: 0,
      disposedBlockoutAssetCount: 0,
      scheduleRender,
    }
    recordAppliedVisualEnvironment(runtime, host)
    runtimeRef.current = runtime

    const onQaViewportCapture = (event: Event) => {
      const request = event instanceof CustomEvent
        ? event.detail as QaViewportCaptureRequest | undefined
        : undefined
      // A direct dispatch on the desired host means sibling workbenches never
      // contend for a capture.  Keep this guard for transient React trees and
      // future multi-window QA runs.
      if (!request || request.viewport !== host) return
      if (pendingQaCapture) {
        request.reject(new Error('QA_V3_VIEWPORT_SCREENSHOT_BUSY'))
        return
      }
      pendingQaCapture = request
      scheduleRender()
    }
    host.addEventListener(FORGECAD_QA_VIEWPORT_CAPTURE_EVENT, onQaViewportCapture)

    const onTransformDragging = (event: { value: unknown }) => {
      controls.enabled = event.value !== true
    }
    const onTransformObjectChange = () => runtime.scheduleRender()
    const onTransformCommit = () => {
      const object = transformControls.object
      const nodeId = object?.userData.nodeId
      if (!object || typeof nodeId !== 'string') return
      propsRef.current.onTransformCommit(nodeId, transformFromObject(object))
    }
    transformControls.addEventListener('dragging-changed', onTransformDragging)
    transformControls.addEventListener('objectChange', onTransformObjectChange)
    transformControls.addEventListener('mouseUp', onTransformCommit)

    const raycaster = new THREE.Raycaster()
    const pointer = new THREE.Vector2()
    const hitAtClientPoint = (clientX: number, clientY: number) => {
      const rect = renderer.domElement.getBoundingClientRect()
      pointer.x = ((clientX - rect.left) / Math.max(rect.width, 1)) * 2 - 1
      pointer.y = -((clientY - rect.top) / Math.max(rect.height, 1)) * 2 + 1
      raycaster.setFromCamera(pointer, camera)
      const hit = raycaster.intersectObjects([...moduleRoot.children, ...blockoutRoot.children], true)
        .find((candidate) => isMeshObject(candidate.object) && Boolean(candidate.face))
      if (!hit || !hit.face) return null
      const nodeId = typeof hit.object.userData.nodeId === 'string'
        ? hit.object.userData.nodeId
        : hit.object.userData.agentBlockout
          ? 'agent_blockout'
          : null
      if (!nodeId) return null
      const normal = hit.face.normal.clone().transformDirection(hit.object.matrixWorld)
      const point = hit.point.clone()
      // Production GLBs are visually fitted into the shared CAD camera.  The
      // fitting scale is presentation-only, so reverse it before passing the
      // point to the inspection layer; otherwise the two-click readout would
      // report the on-screen display size instead of the model's millimetres.
      const displayScale = hit.object.userData.agentBlockout
        ? runtime.activeBlockoutPreview?.displayScale ?? 1
        : 1
      if (displayScale > 0 && displayScale !== 1) point.multiplyScalar(1 / displayScale)
      return { nodeId, point, normal }
    }
    const selectAtPointer = (event: PointerEvent) => {
      const hit = hitAtClientPoint(event.clientX, event.clientY)
      if (!hit) return
      if (propsRef.current.measureEnabled) {
        propsRef.current.onMeasurePoint({
          nodeId: hit.nodeId,
          position: [hit.point.x, hit.point.y, hit.point.z],
          normal: [hit.normal.x, hit.normal.y, hit.normal.z],
        })
        return
      }
      propsRef.current.onSelectNode(hit.nodeId)
    }
    const allowModuleDrop = (event: DragEvent) => {
      if (event.dataTransfer?.types.includes('application/x-forgecad-module-id')) {
        event.preventDefault()
        event.dataTransfer.dropEffect = 'copy'
      }
    }
    const dropModule = (event: DragEvent) => {
      const moduleId = event.dataTransfer?.getData('application/x-forgecad-module-id')
        || event.dataTransfer?.getData('text/plain')
      if (!moduleId) return
      event.preventDefault()
      const nodeId = hitAtClientPoint(event.clientX, event.clientY)?.nodeId ?? propsRef.current.selectedNodeId
      if (nodeId) propsRef.current.onDropModule(nodeId, moduleId)
    }
    renderer.domElement.addEventListener('pointerdown', selectAtPointer)
    renderer.domElement.addEventListener('dragover', allowModuleDrop)
    renderer.domElement.addEventListener('drop', dropModule)
    controls.addEventListener('change', scheduleRender)
    const resize = () => {
      const width = host.clientWidth
      const height = host.clientHeight
      renderer.setSize(width, height, false)
      camera.aspect = Math.max(width, 1) / Math.max(height, 1)
      camera.updateProjectionMatrix()
      if (runtime.activeBlockoutPreview) {
        refreshActiveBlockoutFrame(runtime, propsRef.current.cameraView)
      } else {
        recordPresentationRuntimeFacts(runtime)
      }
      scheduleRender()
    }
    const observer = new ResizeObserver(resize)
    observer.observe(host)
    resize()

    return () => {
      cancelAnimationFrame(frame)
      host.removeEventListener(FORGECAD_QA_VIEWPORT_CAPTURE_EVENT, onQaViewportCapture)
      if (pendingQaCapture) {
        pendingQaCapture.reject(new Error('QA_V3_VIEWPORT_SCREENSHOT_VIEWPORT_DISPOSED'))
        pendingQaCapture = null
      }
      observer.disconnect()
      transformControls.removeEventListener('dragging-changed', onTransformDragging)
      transformControls.removeEventListener('objectChange', onTransformObjectChange)
      transformControls.removeEventListener('mouseUp', onTransformCommit)
      transformControls.detach()
      transformControls.dispose()
      controls.removeEventListener('change', scheduleRender)
      renderer.domElement.removeEventListener('pointerdown', selectAtPointer)
      renderer.domElement.removeEventListener('dragover', allowModuleDrop)
      renderer.domElement.removeEventListener('drop', dropModule)
      clearNodeObjects(runtime)
      clearObjectChildren(blockoutRoot)
      clearObjectChildren(referenceImageRoot)
      runtime.moduleCache.forEach((source) => { void source.then(disposeObject) })
      clearObjectChildren(qualityRoot)
      scene.environment = null
      disposeObject(studioEnvironmentScene)
      studioEnvironment.dispose()
      pmremGenerator.dispose()
      connectorGeometry.dispose()
      connectorMaterials.exclusive.dispose()
      connectorMaterials.shared.dispose()
      controls.dispose()
      disposeObject(scene)
      renderer.dispose()
      renderer.forceContextLoss()
      activeViewportContexts = Math.max(0, activeViewportContexts - 1)
      host.dataset.activeWebglContexts = String(activeViewportContexts)
      renderer.domElement.remove()
      runtimeRef.current = null
    }
  }, [])

  const referenceImageIdentity = props.referenceImage
    ? `${props.referenceImage.evidenceId}:${props.referenceImage.sourceObjectSha256}:${props.referenceImage.referenceClass}:${props.referenceImage.url}`
    : ''
  useEffect(() => {
    const runtime = runtimeRef.current
    const host = hostRef.current
    if (!runtime || !host) return
    const referenceImage = props.referenceImage
    clearObjectChildren(runtime.referenceImageRoot)
    runtime.referenceImageRoot.visible = false
    if (!referenceImage) {
      host.dataset.referenceDisplayMode = 'result'
      delete host.dataset.referenceEvidenceId
      delete host.dataset.referenceSourceObjectSha256
      delete host.dataset.referenceClass
      if (referenceImageLoadState !== 'failed') {
        setReferenceImageLoadState('empty')
        setReferenceImageLoadMessage('')
      }
      runtime.scheduleRender()
      return
    }

    let cancelled = false
    host.dataset.referenceDisplayMode = 'loading'
    host.dataset.referenceEvidenceId = referenceImage.evidenceId
    host.dataset.referenceSourceObjectSha256 = referenceImage.sourceObjectSha256
    host.dataset.referenceClass = referenceImage.referenceClass
    setReferenceImageLoadState('loading')
    setReferenceImageLoadMessage('正在把只读参考图片加载到同一个 3D 视口…')
    const textureLoader = new THREE.TextureLoader()
    textureLoader.load(referenceImage.url, (texture) => {
      if (cancelled) {
        texture.dispose()
        return
      }
      texture.colorSpace = THREE.SRGBColorSpace
      texture.generateMipmaps = true
      texture.minFilter = THREE.LinearMipmapLinearFilter
      texture.magFilter = THREE.LinearFilter
      const image = texture.image as { naturalWidth?: number; naturalHeight?: number; width?: number; height?: number }
      const pixelWidth = Math.max(image.naturalWidth ?? image.width ?? 1, 1)
      const pixelHeight = Math.max(image.naturalHeight ?? image.height ?? 1, 1)
      const scale = Math.min(360 / pixelWidth, 230 / pixelHeight)
      const width = pixelWidth * scale
      const height = pixelHeight * scale
      const imagePlane = new THREE.Mesh(
        new THREE.PlaneGeometry(width, height),
        new THREE.MeshBasicMaterial({ map: texture, toneMapped: false, side: THREE.DoubleSide }),
      )
      imagePlane.name = 'ReadOnlyReferenceImagePlane'
      imagePlane.position.y = -12
      const frame = new THREE.Mesh(
        new THREE.PlaneGeometry(width + 10, height + 48),
        new THREE.MeshBasicMaterial({ color: '#10243a', toneMapped: false, side: THREE.DoubleSide }),
      )
      frame.name = 'ReadOnlyReferenceImageFrame'
      frame.position.y = 4
      frame.position.z = -1
      const header = createReferenceImageHeaderPlane(width, height / 2 + 13, referenceImage.referenceClass)
      runtime.referenceImageRoot.add(frame, imagePlane, header)
      runtime.referenceImageRoot.visible = true
      runtime.blockoutRoot.visible = false
      runtime.moduleRoot.visible = false
      runtime.grid.visible = false
      runtime.axes.visible = false
      runtime.displayFloor.visible = false
      runtime.transformControls.detach()
      runtime.transformHelper.visible = false
      const bounds = new THREE.Box3().setFromObject(runtime.referenceImageRoot)
      frameBlockoutCameraToBounds(runtime, 'front', bounds)
      host.dataset.referenceDisplayMode = 'reference_image'
      setReferenceImageLoadState('ready')
      setReferenceImageLoadMessage('只读参考图片 · 同一 renderer 展示 · 不作为几何真值')
      runtime.scheduleRender()
    }, undefined, () => {
      if (cancelled) return
      clearObjectChildren(runtime.referenceImageRoot)
      runtime.referenceImageRoot.visible = false
      host.dataset.referenceDisplayMode = 'failed'
      setReferenceImageLoadState('failed')
      setReferenceImageLoadMessage('参考图片纹理加载失败；已安全返回当前结果。')
      runtime.scheduleRender()
      propsRef.current.onReferenceImageDisplayFailure()
    })
    return () => {
      cancelled = true
      clearObjectChildren(runtime.referenceImageRoot)
      runtime.referenceImageRoot.visible = false
    }
  // Identity fields are immutable for the lifetime of one saved evidence view.
  // Do not restart texture loading for unrelated workbench presentation renders.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [referenceImageIdentity])

  const graphHash = props.graphRecord?.graph_sha256 ?? ''
  useEffect(() => {
    const runtime = runtimeRef.current
    if (!runtime) return
    const graph = props.graphRecord?.graph ?? null
    runtime.graph = graph
    runtime.modulesById = new Map(props.modules.map((item) => [item.manifest.module_id, item]))
    if (!graph) {
      clearNodeObjects(runtime)
      setLoadState('empty')
      setLoadMessage('还没有 Agent 资产；在底部描述你想生成的模型。')
      if (runtime.activeBlockoutPreview) {
        refreshActiveBlockoutFrame(runtime, props.cameraView)
      } else {
        frameCamera(runtime.camera, runtime.controls, props.cameraView, new THREE.Vector3(), 240)
        recordPresentationRuntimeFacts(runtime)
      }
      runtime.scheduleRender()
      return
    }

    let cancelled = false
    setLoadState('loading')
    setLoadMessage('正在读取不可变 GLB 模块…')
    reconcileNodeObjects(runtime, graph)
    void import('three/examples/jsm/loaders/GLTFLoader.js')
      .then(({ GLTFLoader }) => Promise.all(
        graph.nodes
          .filter((node) => !runtime.nodeObjects.has(node.node_id))
          .map((node) => loadNode(runtime, GLTFLoader, node, props.getModuleFileUrl)),
      ))
      .then(() => {
        if (cancelled) return
        applyVisualState(runtime, propsRef.current)
        syncTransformControls(runtime, propsRef.current)
        if (runtime.activeBlockoutPreview) {
          refreshActiveBlockoutFrame(runtime, propsRef.current.cameraView)
        } else {
          frameVisibleObjects(runtime, propsRef.current)
          recordPresentationRuntimeFacts(runtime)
        }
        setLoadState('ready')
        setLoadMessage(`已加载 ${graph.nodes.length} 个真实 GLB 节点`)
        runtime.scheduleRender()
      })
      .catch((caught) => {
        if (cancelled) return
        setLoadState('failed')
        setLoadMessage(caught instanceof Error ? caught.message : String(caught))
        if (runtime.activeBlockoutPreview) {
          refreshActiveBlockoutFrame(runtime, propsRef.current.cameraView)
        } else {
          frameCamera(runtime.camera, runtime.controls, propsRef.current.cameraView, new THREE.Vector3(), 240)
          recordPresentationRuntimeFacts(runtime)
        }
        runtime.scheduleRender()
      })
    return () => { cancelled = true }
  }, [graphHash, props.getModuleFileUrl, props.modules])

  useEffect(() => {
    const runtime = runtimeRef.current
    if (!runtime) return
    restoreModuleGraphPresentation(runtime, propsRef.current)
    if (props.referenceImage) {
      runtime.moduleRoot.visible = false
      runtime.blockoutRoot.visible = false
      runtime.axes.visible = false
      runtime.grid.visible = false
      runtime.displayFloor.visible = false
      setBlockoutLoadState('empty')
      setBlockoutLoadMessage('')
      setBlockoutPreviewPrimitiveKinds([])
      setBlockoutRenderSource('empty')
      setBlockoutEmbeddedPbrMaterialCount(0)
      runtime.scheduleRender()
      return
    }
    if (!props.blockoutShapeProgram && !props.blockoutGlbBase64) {
      setBlockoutLoadState('empty')
      setBlockoutLoadMessage('')
      setBlockoutPreviewPrimitiveKinds([])
      setBlockoutRenderSource('empty')
      setBlockoutEmbeddedPbrMaterialCount(0)
      runtime.scheduleRender()
      return
    }
    setBlockoutLoadState('loading')
    let cancelled = false
    const attachPreview = (source: THREE.Object3D, message: string) => {
      if (cancelled) {
        disposeObject(source)
        return
      }
      runtime.blockoutRoot.add(source)
      runtime.blockoutRoot.visible = true
      runtime.axes.visible = false
      runtime.grid.visible = false
      setBlockoutPreviewPrimitiveKinds(Array.isArray(source.userData.forgecadPreviewPrimitiveKinds)
        ? source.userData.forgecadPreviewPrimitiveKinds.filter((item: unknown): item is string => typeof item === 'string')
        : [])
      setBlockoutEmbeddedPbrMaterialCount(
        typeof source.userData.forgecadEmbeddedPbrMaterialCount === 'number'
          ? source.userData.forgecadEmbeddedPbrMaterialCount
          : 0,
      )
      source.updateMatrixWorld(true)
      runtime.blockoutRoot.updateMatrixWorld(true)
      let bounds = new THREE.Box3().setFromObject(source)
      if (bounds.isEmpty()) throw new Error('导入模型没有可显示的网格输出')
      const sourceSize = bounds.getSize(new THREE.Vector3())
      const fitScale = Math.min(1, BLOCKOUT_DISPLAY_DIAGONAL_MM / Math.max(sourceSize.length(), 1))
      // GLBLoader has already converted source metres to workbench millimetres.
      // Multiply that scale instead of replacing it, otherwise a 1 m asset is
      // accidentally displayed as roughly 1 mm and the shadow catcher dwarfs it.
      source.scale.multiplyScalar(fitScale)
      runtime.moduleRoot.visible = false
      runtime.transformControls.detach()
      runtime.transformHelper.visible = false
      source.updateMatrixWorld(true)
      runtime.blockoutRoot.updateMatrixWorld(true)
      bounds = new THREE.Box3().setFromObject(source)
      // Normalize the preview into the existing CAD camera space. This keeps
      // the single viewport stable and avoids a camera jump while still
      // showing the complete generated silhouette.
      source.position.sub(bounds.getCenter(new THREE.Vector3()))
      source.updateMatrixWorld(true)
      runtime.blockoutRoot.updateMatrixWorld(true)
      bounds = new THREE.Box3().setFromObject(source)
      const framedCenter = bounds.getCenter(new THREE.Vector3())
      const framedSize = bounds.getSize(new THREE.Vector3())
      const framedDiagonal = Math.max(framedSize.length(), 1)
      runtime.displayFloor.position.set(
        framedCenter.x,
        bounds.min.y - Math.max(framedSize.y * 0.01, 1),
        framedCenter.z,
      )
      const horizontalRadius = Math.max(Math.hypot(framedSize.x, framedSize.z) * 0.5, 1)
      runtime.displayFloor.scale.setScalar(
        horizontalRadius * FORGECAD_STUDIO_MANIFEST.cad_neutral_lighting.floor.radius_ratio,
      )
      fitPreviewShadowCamera(runtime, framedSize)
      runtime.activeBlockoutPreview = {
        source,
        displayScale: fitScale,
        displayDiagonalMm: framedDiagonal,
        sourceBoundsMm: sourceSize.toArray(),
      }
      refreshActiveBlockoutFrame(runtime, propsRef.current.cameraView)
      setBlockoutLoadState('ready')
      setBlockoutLoadMessage(message)
      runtime.scheduleRender()
    }
    if (props.blockoutGlbBase64) {
      const blockoutGlbPayload = props.blockoutGlbBase64
      // The compiled Agent GLB is the only source that contains its exact
      // texture bytes, UV/tangent bindings and zone-to-material mapping.
      // Prefer it over the bounded ShapeProgram display adapter whenever both
      // are available; silently replacing it with parameter materials would
      // make the PBR/readback promise unverifiable in the workbench.
      const externalReference = props.blockoutGlbKind === 'external_reference'
      const productionConcept = props.blockoutGlbKind === 'compiled_agent_production_pbr'
      setBlockoutLoadMessage(
        externalReference
          ? '正在加载只读外部参考 GLB…'
          : productionConcept
            ? '正在加载生产概念工件档 PBR GLB…'
            : '正在加载同源轻量 PBR 预览…',
      )
      void import('three/examples/jsm/loaders/GLTFLoader.js')
        .then(({ GLTFLoader }) => new Promise<THREE.Object3D>((resolve, reject) => {
          const loader = new GLTFLoader()
          const glbPayload = typeof blockoutGlbPayload === 'string'
            ? base64ToArrayBuffer(blockoutGlbPayload)
            : blockoutGlbPayload.slice(0)
          loader.parse(glbPayload, '', (gltf) => {
            const source = gltf.scene ?? gltf.scenes[0]
            if (!source) {
              reject(new Error('导入 GLB 没有 scene'))
              return
            }
            source.scale.setScalar(GLB_METERS_TO_WORKBENCH_MILLIMETERS)
            source.traverse((child) => {
              if (isMeshObject(child)) {
                child.castShadow = true
                child.receiveShadow = true
                child.userData.agentBlockout = true
                child.material = Array.isArray(child.material)
                  ? child.material.map((material) => material.clone())
                  : child.material.clone()
                const materials = Array.isArray(child.material) ? child.material : [child.material]
                materials.forEach(applyForgecadPbrDisplayCalibration)
                const hasTransparentSurface = materials.some((material) => (
                  material.transparent
                  || material.opacity < 0.99
                  || (isPbrMaterial(material) && 'transmission' in material && material.transmission > 0)
                ))
                child.geometry.computeBoundingBox()
                const geometrySize = child.geometry.boundingBox?.getSize(new THREE.Vector3()).length() ?? 0
                const hasCompletePbrSurface = materials.length > 0 && materials.every(isCompletePbrMaterial)
                // Full PBR assets already carry normal/roughness detail. A
                // per-mesh line overlay doubles draw calls and gives the
                // result a toy/CAD-outline look, so keep it only for the
                // bounded parameter-material fallback.
                if (!hasCompletePbrSurface && !hasTransparentSurface && geometrySize >= 0.08) {
                  const edgeOverlay = new THREE.LineSegments(
                    new THREE.EdgesGeometry(child.geometry, 42),
                    new THREE.LineBasicMaterial({ color: '#d3dde6', transparent: true, opacity: 0.08 }),
                  )
                  edgeOverlay.name = 'ForgeCADAgentEdgeOverlay'
                  edgeOverlay.renderOrder = 3
                  edgeOverlay.userData.forgecadAgentEdgeOverlay = true
                  child.add(edgeOverlay)
                }
              }
            })
            applyAgentBlockoutVisualState(source, propsRef.current)
            const embeddedPbrMaterialCount = countEmbeddedPbrMaterials(source)
            if (!externalReference && embeddedPbrMaterialCount === 0) {
              reject(new Error(`同源 GLB 没有可用的完整 PBR 纹理材质，不能作为真实纹理预览显示 [${diagnosePbrMaterialGap(source)}]`))
              return
            }
            const targetPbrAnisotropy = configurePbrTextureSampling(source, runtime.renderer)
            source.userData.forgecadEmbeddedPbrMaterialCount = embeddedPbrMaterialCount
            source.userData.forgecadPbrTextureFacts = collectPbrTextureFacts(source, targetPbrAnisotropy)
            resolve(source)
          }, reject)
        }))
        .then((source) => {
          if (cancelled) {
            disposeObject(source)
            return
          }
          const hasEmbeddedPbr = Number(source.userData.forgecadEmbeddedPbrMaterialCount ?? 0) > 0
          setBlockoutRenderSource(hasEmbeddedPbr ? 'glb_pbr' : 'external_reference')
          attachPreview(
            source,
            externalReference
              ? hasEmbeddedPbr
                ? '外部参考 GLB 已按完整嵌入 PBR 加载（只读）'
                : '外部参考 GLB 已按只读来源加载；不声明完整 PBR'
              : '同源 PBR GLB 已加载',
          )
        })
        .catch((error) => {
          if (cancelled) return
          setBlockoutRenderSource('empty')
          setBlockoutEmbeddedPbrMaterialCount(0)
          restoreModuleGraphPresentation(runtime, propsRef.current)
          setBlockoutLoadState('failed')
          setBlockoutLoadMessage(error instanceof Error ? error.message : String(error))
          runtime.scheduleRender()
        })
    } else if (props.blockoutShapeProgram) {
      // A source GLB is unavailable only while a bounded program is being
      // prepared. This adapter is intentionally labelled as a display-only
      // fallback and is never presented as embedded-texture PBR output.
      setBlockoutLoadMessage('正在解释 Agent ShapeProgram（参数外观回退）…')
      try {
        setBlockoutRenderSource('shape_program_fallback')
        setBlockoutEmbeddedPbrMaterialCount(0)
        attachPreview(
          buildShapeProgramPreview(
            props.blockoutShapeProgram,
            {
              materialOverride: props.blockoutMaterialOverride,
              selectedAgentPartId: props.selectedAgentPartId,
              hiddenAgentPartIds: props.hiddenAgentPartIds,
              isolatedAgentPartId: props.isolatedAgentPartId,
              lockedAgentPartIds: props.lockedAgentPartIds,
            },
          ),
          'Agent ShapeProgram 参数外观预览已加载；等待同源 PBR GLB',
        )
      } catch (error) {
        setBlockoutRenderSource('empty')
        setBlockoutEmbeddedPbrMaterialCount(0)
        restoreModuleGraphPresentation(runtime, propsRef.current)
        setBlockoutLoadState('failed')
        setBlockoutLoadMessage(error instanceof Error ? error.message : String(error))
        runtime.scheduleRender()
      }
    }
    return () => {
      cancelled = true
      runtime.activeBlockoutPreview = null
    }
  }, [
    props.blockoutGlbBase64,
    props.blockoutGlbKind,
    props.blockoutShapeProgram,
    props.blockoutMaterialOverride,
    referenceImageIdentity,
  ])

  useEffect(() => {
    const runtime = runtimeRef.current
    if (!runtime) return
    runtime.blockoutRoot.traverse((child) => {
      if (isMeshObject(child) && child.userData.agentBlockout) applyAgentBlockoutMeshVisualState(child, props)
    })
    runtime.scheduleRender()
  }, [
    props.selectedAgentPartId,
    props.hiddenAgentPartIds,
    props.isolatedAgentPartId,
    props.lockedAgentPartIds,
    props.wireframe,
    props.xRay,
    props.ghostPreview,
  ])

  useEffect(() => {
    const runtime = runtimeRef.current
    if (!runtime) return
    if (propsRef.current.referenceImage) {
      runtime.moduleRoot.visible = false
      runtime.blockoutRoot.visible = false
      runtime.axes.visible = false
      runtime.grid.visible = false
      runtime.displayFloor.visible = false
      runtime.scheduleRender()
      return
    }
    applyVisualState(runtime, propsRef.current)
    syncTransformControls(runtime, propsRef.current)
    runtime.scheduleRender()
  }, [
    props.explodeFactor,
    props.ghostPreview,
    props.hiddenNodeIds,
    props.qualityHighlightNodeIds,
    props.selectedNodeId,
    props.showConnectors,
    props.showGrid,
    props.wireframe,
    props.xRay,
    props.lightPreset,
    props.sectionEnabled,
    props.sectionOffset,
    props.transformTool,
    props.transformSpace,
    props.snapEnabled,
    graphHash,
  ])

  useEffect(() => {
    const runtime = runtimeRef.current
    if (!runtime) return
    // Measuring intentionally freezes orbit for the two-click interaction.
    // It only switches the existing controls on the one renderer; no overlay
    // canvas or second scene is created.
    runtime.controls.enabled = !props.measureEnabled
    const host = runtime.renderer.domElement.parentElement
    if (host instanceof HTMLElement) host.dataset.measureEnabled = String(props.measureEnabled)
    runtime.scheduleRender()
  }, [props.measureEnabled])

  useEffect(() => {
    const runtime = runtimeRef.current
    if (!runtime) return
    clearObjectChildren(runtime.qualityRoot)
    runtime.qualityRoot.add(buildQualityOverlay(props.qualityGeometryRefs))
    runtime.scheduleRender()
  }, [props.qualityGeometryRefs])

  useEffect(() => {
    const runtime = runtimeRef.current
    if (!runtime) return
    if (runtime.activeBlockoutPreview) {
      refreshActiveBlockoutFrame(runtime, propsRef.current.cameraView)
    } else if (runtime.nodeObjects.size > 0) {
      frameVisibleObjects(runtime, propsRef.current)
      recordPresentationRuntimeFacts(runtime)
    } else {
      return
    }
    runtime.scheduleRender()
  }, [props.cameraView, props.focusNodeId, props.qualityHighlightNodeIds, graphHash])

  return (
    <div className="weapon-viewport-shell">
      <div
        className="weapon-viewport"
        ref={hostRef}
        aria-label="真实 ModuleGraph 三维视口"
        data-load-state={loadState}
        data-preview-mode={props.ghostPreview ? 'ghost' : 'committed'}
        data-camera-view={props.cameraView}
        data-light-preset={props.lightPreset}
        data-xray={props.xRay ? 'enabled' : 'disabled'}
        data-section={props.sectionEnabled ? 'enabled' : 'disabled'}
        data-section-offset={String(props.sectionOffset)}
        data-focus-node-id={props.focusNodeId ?? ''}
        data-quality-node-ids={props.qualityHighlightNodeIds.join(',')}
        data-quality-triangle-count={props.qualityGeometryRefs.reduce(
          (count, reference) => count + (reference.world_triangles_mm?.length ?? 0),
          0,
        )}
        data-blockout-preview={props.blockoutGlbBase64 ? 'ready' : 'empty'}
        data-blockout-load-state={blockoutLoadState}
        data-blockout-glb-kind={props.blockoutGlbKind ?? 'none'}
        data-blockout-render-source={blockoutRenderSource}
        data-blockout-embedded-pbr-material-count={String(blockoutEmbeddedPbrMaterialCount)}
        data-blockout-preview-primitives={blockoutPreviewPrimitiveKinds.join(',')}
        data-reference-display-mode={referenceImageLoadState === 'ready' ? 'reference_image' : referenceImageLoadState === 'loading' ? 'loading' : referenceImageLoadState === 'failed' ? 'failed' : 'result'}
        data-reference-image-load-state={referenceImageLoadState}
        data-reference-evidence-id={props.referenceImage?.evidenceId ?? ''}
        data-reference-source-object-sha256={props.referenceImage?.sourceObjectSha256 ?? ''}
        data-reference-class={props.referenceImage?.referenceClass ?? ''}
        data-agent-hidden-part-ids={props.hiddenAgentPartIds.join(',')}
        data-agent-isolated-part-id={props.isolatedAgentPartId ?? ''}
        data-agent-locked-part-ids={props.lockedAgentPartIds.join(',')}
        data-measure-enabled={String(props.measureEnabled)}
      />
      {referenceImageLoadState === 'empty' && loadState !== 'ready' && blockoutLoadState !== 'ready' && (
        <div className={`viewport-data-state ${loadState}`} role="status">
          <strong>{loadState === 'loading' ? '加载 3D 资产' : loadState === 'failed' ? 'GLB 无法显示' : '等待 Agent 生成'}</strong>
          <span>{loadMessage}</span>
        </div>
      )}
      {props.blockoutGlbBase64 && blockoutLoadState !== 'ready' && (
        <div className={`viewport-data-state blockout-${blockoutLoadState}`} role="status">
          <strong>{blockoutLoadState === 'loading' ? '加载 Agent blockout' : 'Agent blockout 无法显示'}</strong>
          <span>{blockoutLoadMessage}</span>
        </div>
      )}
      {props.referenceImage && referenceImageLoadState !== 'ready' && (
        <div className={`viewport-data-state reference-image-${referenceImageLoadState}`} role="status">
          <strong>{referenceImageLoadState === 'loading' ? '加载只读参考图片' : '参考图片无法显示'}</strong>
          <span>{referenceImageLoadMessage}</span>
        </div>
      )}
    </div>
  )
}

async function loadNode(
  runtime: ViewportRuntime,
  Loader: typeof GLTFLoader,
  node: Graph['nodes'][number],
  getModuleFileUrl: (moduleId: string) => string,
) {
  const source = await loadModuleSource(runtime, Loader, node.module_id, getModuleFileUrl(node.module_id))
  const object = new THREE.Group()
  object.name = node.node_id
  object.userData.nodeId = node.node_id
  object.userData.moduleId = node.module_id
  const assetScene = source.clone(true)
  assetScene.scale.setScalar(GLB_METERS_TO_WORKBENCH_MILLIMETERS)
  assetScene.traverse((child) => {
    child.userData.nodeId = node.node_id
    if (!(child instanceof THREE.Mesh)) return
    child.castShadow = true
    child.receiveShadow = true
    const materials = (Array.isArray(child.material) ? child.material : [child.material]).map((item) => {
      const material = item.clone()
      if (material instanceof THREE.MeshStandardMaterial || material instanceof THREE.MeshPhysicalMaterial) {
        const color = material.color
        const isSignalAccent = color.r > color.g * 1.45 && color.r > color.b * 1.35
        // The authored pack retains its source materials. In the workbench we
        // lift only the low-value non-accent surfaces into the same readable
        // graphite range as a physical CAD presentation, rather than hiding
        // hard-surface detail in near-black diffuse shading.
        const perceivedLuminance = color.r * 0.2126 + color.g * 0.7152 + color.b * 0.0722
        if (!isSignalAccent && perceivedLuminance < 0.24) {
          color.lerp(new THREE.Color('#52677b'), 0.38)
        }
        material.metalness = Math.max(material.metalness, isSignalAccent ? 0.38 : 0.7)
        material.roughness = Math.min(Math.max(material.roughness || 0.42, isSignalAccent ? 0.28 : 0.32), isSignalAccent ? 0.48 : 0.52)
        material.envMapIntensity = Math.max(material.envMapIntensity, 0.9)
      }
      return material
    })
    child.material = Array.isArray(child.material) ? materials : materials[0]
    // This is an overlay of the real mesh's own hard edges—not a replacement
    // illustration. It gives low-texture authored modules the legible panel
    // breaks expected in a precision CAD presentation.
    const edgeOverlay = new THREE.LineSegments(
      new THREE.EdgesGeometry(child.geometry, 28),
      new THREE.LineBasicMaterial({ color: '#07101a', transparent: true, opacity: 0.52 }),
    )
    edgeOverlay.name = 'ForgeCADEdgeOverlay'
    edgeOverlay.renderOrder = 4
    edgeOverlay.userData.forgecadEdgeOverlay = true
    child.add(edgeOverlay)
  })
  object.add(assetScene)
  const moduleRecord = runtime.modulesById.get(node.module_id)
  const markerRadius = Math.max(
    Math.max(...(moduleRecord?.manifest.bounds_mm ?? [10])) * 0.035,
    0.5,
  )
  for (const connector of moduleRecord?.manifest.connectors ?? []) {
    const marker = new THREE.Mesh(
      runtime.connectorGeometry,
      connector.exclusive === false ? runtime.connectorMaterials.shared : runtime.connectorMaterials.exclusive,
    )
    marker.scale.setScalar(markerRadius)
    const [cx = 0, cy = 0, cz = 0] = connector.transform.position
    marker.position.set(cx, cy, cz)
    marker.name = connector.connector_id
    marker.renderOrder = 10
    marker.userData.nodeId = node.node_id
    marker.userData.forgecadConnectorMarker = true
    object.add(marker)
  }
  runtime.nodeObjects.set(node.node_id, object)
  runtime.moduleRoot.add(object)
}

function loadModuleSource(
  runtime: ViewportRuntime,
  Loader: typeof GLTFLoader,
  moduleId: string,
  url: string,
): Promise<THREE.Group> {
  const cached = runtime.moduleCache.get(moduleId)
  if (cached) return cached
  const source = new Promise<THREE.Group>((resolve, reject) => {
    const loader = new Loader()
    loader.load(url, (gltf) => resolve(gltf.scene), undefined, reject)
  }).catch((error) => {
    runtime.moduleCache.delete(moduleId)
    throw error
  })
  runtime.moduleCache.set(moduleId, source)
  return source
}

function applyVisualState(runtime: ViewportRuntime, props: ModuleGraphViewportProps) {
  applyLightPreset(runtime, props.lightPreset)
  runtime.grid.visible = props.showGrid && runtime.moduleRoot.visible
  runtime.sectionPlane.constant = props.sectionOffset
  runtime.sectionHelper.visible = props.sectionEnabled
  runtime.renderer.clippingPlanes = props.sectionEnabled ? [runtime.sectionPlane] : []
  const graph = runtime.graph
  if (!graph) return
  const rootPosition = new THREE.Vector3(
    ...(graph.nodes.find((node) => node.node_id === graph.root_node_id)?.transform.position ?? [0, 0, 0]) as [number, number, number],
  )
  graph.nodes.forEach((node, nodeIndex) => {
    const object = runtime.nodeObjects.get(node.node_id)
    if (!object) return
    const [px = 0, py = 0, pz = 0] = node.transform.position
    const [rx = 0, ry = 0, rz = 0] = node.transform.rotation
    const [sx = 1, sy = 1, sz = 1] = node.transform.scale
    const mirrorAxis = node.mirror_axis ?? 'none'
    object.position.set(px, py, pz)
    if (props.explodeFactor > 0 && node.node_id !== graph.root_node_id) {
      const direction = object.position.clone().sub(rootPosition)
      if (direction.lengthSq() < 0.0001) direction.set(nodeIndex % 2 === 0 ? 1 : -1, nodeIndex % 3 === 0 ? 0.5 : 0, 0)
      const extent = Math.max(...(runtime.modulesById.get(node.module_id)?.manifest.bounds_mm ?? [50]))
      object.position.add(direction.normalize().multiplyScalar(extent * props.explodeFactor))
    }
    object.rotation.set(rx, ry, rz)
    object.scale.set(sx * (mirrorAxis === 'x' ? -1 : 1), sy * (mirrorAxis === 'y' ? -1 : 1), sz * (mirrorAxis === 'z' ? -1 : 1))
    object.visible = node.visible !== false && !props.hiddenNodeIds.includes(node.node_id)
    object.traverse((child) => {
      if (child.userData.forgecadConnectorMarker) {
        child.visible = props.showConnectors
        return
      }
      if (!(child instanceof THREE.Mesh)) return
      const materials = Array.isArray(child.material) ? child.material : [child.material]
      materials.forEach((material) => {
        if ('wireframe' in material) material.wireframe = props.wireframe
        if ('transparent' in material) {
          material.transparent = props.ghostPreview || props.xRay
          material.opacity = props.ghostPreview ? 0.58 : props.xRay ? 0.24 : 1
          material.depthWrite = !props.ghostPreview && !props.xRay
        }
        if (material instanceof THREE.MeshStandardMaterial || material instanceof THREE.MeshPhysicalMaterial) {
          const quality = props.qualityHighlightNodeIds.includes(node.node_id)
          material.emissive.set(quality ? '#b62424' : props.ghostPreview ? '#087ea8' : node.node_id === props.selectedNodeId ? '#1f64a8' : '#000000')
          material.emissiveIntensity = quality ? 0.72 : props.ghostPreview ? 0.48 : node.node_id === props.selectedNodeId ? 0.42 : 0
        }
      })
    })
  })
}

function applyAgentBlockoutVisualState(root: THREE.Object3D, props: ModuleGraphViewportProps): void {
  root.traverse((child) => {
    if (isMeshObject(child) && child.userData.agentBlockout) applyAgentBlockoutMeshVisualState(child, props)
  })
}

function countEmbeddedPbrMaterials(root: THREE.Object3D): number {
  const textureSetKeys = new Set<string>()
  root.traverse((child) => {
    if (!isMeshObject(child)) return
    for (const material of Array.isArray(child.material) ? child.material : [child.material]) {
      if (!isCompletePbrMaterial(material)) continue
      const declaredTextureSetId = typeof material.userData.forgecad_visual_texture_set_id === 'string'
        ? material.userData.forgecad_visual_texture_set_id
        : ''
      const declaredMaterialId = typeof material.userData.forgecad_texture_material_id === 'string'
        ? material.userData.forgecad_texture_material_id
        : ''
      textureSetKeys.add(declaredTextureSetId || declaredMaterialId || [
        material.map.uuid,
        material.metalnessMap.uuid,
        material.roughnessMap.uuid,
        material.normalMap.uuid,
        material.aoMap.uuid,
        material.emissiveMap.uuid,
      ].join(':'))
    }
  })
  return textureSetKeys.size
}

function diagnosePbrMaterialGap(root: THREE.Object3D): string {
  const materials: Array<THREE.MeshStandardMaterial | THREE.MeshPhysicalMaterial> = []
  root.traverse((child) => {
    if (!isMeshObject(child)) return
    for (const material of Array.isArray(child.material) ? child.material : [child.material]) {
      if (isPbrMaterial(material)) materials.push(material)
    }
  })
  if (materials.length === 0) return 'PBR_MATERIAL_TYPE_MISSING'
  for (const [field, code] of [
    ['map', 'PBR_BASE_COLOR_MAP_MISSING'],
    ['metalnessMap', 'PBR_METALNESS_MAP_MISSING'],
    ['roughnessMap', 'PBR_ROUGHNESS_MAP_MISSING'],
    ['normalMap', 'PBR_NORMAL_MAP_MISSING'],
    ['aoMap', 'PBR_AO_MAP_MISSING'],
    ['emissiveMap', 'PBR_EMISSIVE_MAP_MISSING'],
  ] as const) {
    if (materials.every((material) => !material[field])) return code
  }
  return 'PBR_TEXTURE_SET_FRAGMENTED'
}

type CompletePbrMaterial = (THREE.MeshStandardMaterial | THREE.MeshPhysicalMaterial) & {
  map: THREE.Texture
  metalnessMap: THREE.Texture
  roughnessMap: THREE.Texture
  normalMap: THREE.Texture
  aoMap: THREE.Texture
  emissiveMap: THREE.Texture
}

function isMeshObject(object: THREE.Object3D): object is THREE.Mesh {
  // Three.js documents the `isMesh` discriminator for runtime type checks.
  // It remains reliable when Vite places GLTFLoader and the workbench runtime
  // in separate chunks, whereas `instanceof` can fail across module copies.
  return (object as THREE.Mesh).isMesh === true
}

function isPbrMaterial(
  material: THREE.Material,
): material is THREE.MeshStandardMaterial | THREE.MeshPhysicalMaterial {
  const candidate = material as THREE.MeshStandardMaterial & { isMeshPhysicalMaterial?: boolean }
  return candidate.isMeshStandardMaterial === true || candidate.isMeshPhysicalMaterial === true
}

function applyForgecadPbrDisplayCalibration(material: THREE.Material): void {
  if (!isPbrMaterial(material)) return
  const materialId = typeof material.userData.forgecad_texture_material_id === 'string'
    ? material.userData.forgecad_texture_material_id
    : ''
  material.envMapIntensity = FORGECAD_PBR_ENVIRONMENT_INTENSITY_BY_MATERIAL_ID[materialId]
    ?? FORGECAD_PBR_DEFAULT_ENVIRONMENT_INTENSITY
}

function isCompletePbrMaterial(material: THREE.Material): material is CompletePbrMaterial {
  return (
    isPbrMaterial(material)
    && Boolean(material.map)
    && Boolean(material.metalnessMap)
    && Boolean(material.roughnessMap)
    && Boolean(material.normalMap)
    && Boolean(material.aoMap)
    && Boolean(material.emissiveMap)
  )
}

type BlockoutPbrTextureFacts = {
  uniqueTextureCount: number
  estimatedGpuBytes: number
  colorSpacesValid: boolean
  samplingValid: boolean
  minAnisotropy: number
  maxAnisotropy: number
}

type BlockoutFrameNdcFacts = {
  minX: number
  maxX: number
  minY: number
  maxY: number
  cameraDistanceMm: number
}

function refreshActiveBlockoutFrame(runtime: ViewportRuntime, view: CameraView): void {
  const active = runtime.activeBlockoutPreview
  if (!active) return
  active.source.updateMatrixWorld(true)
  runtime.blockoutRoot.updateMatrixWorld(true)
  const bounds = new THREE.Box3().setFromObject(active.source)
  if (bounds.isEmpty()) throw new Error('导入模型没有可显示的网格输出')
  const frameNdc = frameBlockoutCameraToBounds(runtime, view, bounds)
  recordBlockoutRuntimeFacts(runtime, active.source, {
    displayScale: active.displayScale,
    displayDiagonalMm: active.displayDiagonalMm,
    sourceBoundsMm: active.sourceBoundsMm,
    frameNdc,
  })
  const host = runtime.renderer.domElement.parentElement
  if (host instanceof HTMLElement) host.dataset.presentationSource = 'blockout'
  recordPresentationRuntimeFacts(runtime)
}

function restoreModuleGraphPresentation(runtime: ViewportRuntime, props: ModuleGraphViewportProps): void {
  runtime.activeBlockoutPreview = null
  const disposedCount = runtime.blockoutRoot.children.length
  clearObjectChildren(runtime.blockoutRoot)
  if (disposedCount > 0) runtime.disposedBlockoutAssetCount += disposedCount
  runtime.blockoutReplacementGeneration += 1
  runtime.blockoutRoot.visible = false
  runtime.moduleRoot.visible = true
  runtime.axes.visible = true
  runtime.displayFloor.visible = true
  if (runtime.scene.fog instanceof THREE.Fog) {
    runtime.scene.fog.near = DEFAULT_SCENE_FOG_NEAR_MM
    runtime.scene.fog.far = DEFAULT_SCENE_FOG_FAR_MM
  }
  applyVisualState(runtime, props)
  syncTransformControls(runtime, props)
  frameVisibleObjects(runtime, props)
  recordBlockoutRuntimeFacts(runtime, null)
  const host = runtime.renderer.domElement.parentElement
  if (host instanceof HTMLElement) {
    host.dataset.presentationSource = 'module_graph'
    host.dataset.blockoutReplacementGeneration = String(runtime.blockoutReplacementGeneration)
    host.dataset.disposedBlockoutAssetCount = String(runtime.disposedBlockoutAssetCount)
  }
  recordPresentationRuntimeFacts(runtime)
}

function recordPresentationRuntimeFacts(runtime: ViewportRuntime): void {
  const host = runtime.renderer.domElement.parentElement
  if (!(host instanceof HTMLElement)) return
  const shadowCamera = runtime.keyLight.shadow.camera
  host.dataset.presentationRuntimeFacts = canonicalJson({
    module_root_visible: runtime.moduleRoot.visible,
    blockout_root_visible: runtime.blockoutRoot.visible,
    axes_visible: runtime.axes.visible,
    grid_visible: runtime.grid.visible,
    transform_helper_visible: runtime.transformHelper.visible,
    display_floor: {
      position: runtime.displayFloor.position.toArray(),
      rotation: runtime.displayFloor.rotation.toArray().slice(0, 3),
      scale: runtime.displayFloor.scale.toArray(),
    },
    shadow_camera: {
      left: shadowCamera.left,
      right: shadowCamera.right,
      top: shadowCamera.top,
      bottom: shadowCamera.bottom,
      near: shadowCamera.near,
      far: shadowCamera.far,
    },
    camera: {
      position: runtime.camera.position.toArray(),
      target: runtime.controls.target.toArray(),
      aspect: runtime.camera.aspect,
      near: runtime.camera.near,
      far: runtime.camera.far,
    },
  })
}

function pbrTextureMaps(
  material: THREE.MeshStandardMaterial | THREE.MeshPhysicalMaterial,
): Array<[THREE.Texture | null, string]> {
  return [
    [material.map, THREE.SRGBColorSpace],
    [material.metalnessMap, THREE.NoColorSpace],
    [material.roughnessMap, THREE.NoColorSpace],
    [material.normalMap, THREE.NoColorSpace],
    [material.aoMap, THREE.NoColorSpace],
    [material.emissiveMap, THREE.SRGBColorSpace],
  ]
}

function configurePbrTextureSampling(root: THREE.Object3D, renderer: THREE.WebGLRenderer): number {
  const hardwareMaxAnisotropy = renderer.capabilities.getMaxAnisotropy()
  const targetAnisotropy = Math.min(
    PBR_TEXTURE_ANISOTROPY_CAP,
    Number.isFinite(hardwareMaxAnisotropy) ? Math.max(1, hardwareMaxAnisotropy) : 1,
  )
  const textures = new Set<THREE.Texture>()
  root.traverse((child) => {
    if (!isMeshObject(child)) return
    for (const material of Array.isArray(child.material) ? child.material : [child.material]) {
      if (!isPbrMaterial(material)) continue
      for (const [texture] of pbrTextureMaps(material)) {
        if (texture) textures.add(texture)
      }
    }
  })
  for (const texture of textures) {
    const samplingChanged = (
      texture.wrapS !== THREE.RepeatWrapping
      || texture.wrapT !== THREE.RepeatWrapping
      || texture.magFilter !== THREE.LinearFilter
      || texture.minFilter !== THREE.LinearMipmapLinearFilter
      || !texture.generateMipmaps
      || texture.anisotropy !== targetAnisotropy
    )
    texture.wrapS = THREE.RepeatWrapping
    texture.wrapT = THREE.RepeatWrapping
    texture.magFilter = THREE.LinearFilter
    texture.minFilter = THREE.LinearMipmapLinearFilter
    texture.generateMipmaps = true
    texture.anisotropy = targetAnisotropy
    if (samplingChanged) texture.needsUpdate = true
  }
  return targetAnisotropy
}

function collectPbrTextureFacts(
  root: THREE.Object3D,
  expectedAnisotropy: number,
): BlockoutPbrTextureFacts {
  const textures = new Set<THREE.Texture>()
  let colorSpacesValid = true
  let samplingValid = true
  root.traverse((child) => {
    if (!isMeshObject(child)) return
    for (const material of Array.isArray(child.material) ? child.material : [child.material]) {
      if (!isPbrMaterial(material)) continue
      for (const [texture, expectedColorSpace] of pbrTextureMaps(material)) {
        if (!texture) continue
        textures.add(texture)
        if (texture.colorSpace !== expectedColorSpace) colorSpacesValid = false
        if (
          texture.wrapS !== THREE.RepeatWrapping
          || texture.wrapT !== THREE.RepeatWrapping
          || texture.magFilter !== THREE.LinearFilter
          || texture.minFilter !== THREE.LinearMipmapLinearFilter
          || !texture.generateMipmaps
          || texture.anisotropy !== expectedAnisotropy
        ) {
          samplingValid = false
        }
      }
    }
  })
  let estimatedGpuBytes = 0
  for (const texture of textures) {
    const image = texture.image as { width?: number; height?: number } | undefined
    const width = Number(image?.width ?? 0)
    const height = Number(image?.height ?? 0)
    if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) continue
    // Conservative RGBA8 estimate including a complete mip chain. The source
    // PNG byte size is not a GPU budget and must not be substituted here.
    estimatedGpuBytes += Math.ceil(width * height * 4 * (4 / 3))
  }
  const anisotropyValues = [...textures].map((texture) => texture.anisotropy)
  return {
    uniqueTextureCount: textures.size,
    estimatedGpuBytes,
    colorSpacesValid,
    samplingValid,
    minAnisotropy: anisotropyValues.length ? Math.min(...anisotropyValues) : 0,
    maxAnisotropy: anisotropyValues.length ? Math.max(...anisotropyValues) : 0,
  }
}

function recordBlockoutRuntimeFacts(
  runtime: ViewportRuntime,
  source: THREE.Object3D | null,
  display?: {
    displayScale: number
    displayDiagonalMm: number
    sourceBoundsMm: number[]
    frameNdc: BlockoutFrameNdcFacts
  },
): void {
  const host = runtime.renderer.domElement.parentElement
  if (!(host instanceof HTMLElement)) return
  const facts = source?.userData.forgecadPbrTextureFacts as BlockoutPbrTextureFacts | undefined
  host.dataset.blockoutPbrTextureCount = String(facts?.uniqueTextureCount ?? 0)
  host.dataset.blockoutPbrEstimatedGpuBytes = String(facts?.estimatedGpuBytes ?? 0)
  host.dataset.blockoutPbrColorSpaces = facts ? (facts.colorSpacesValid ? 'valid' : 'invalid') : 'not_applicable'
  host.dataset.blockoutPbrSamplingValid = facts ? String(facts.samplingValid) : 'not_applicable'
  host.dataset.blockoutPbrMinAnisotropy = String(facts?.minAnisotropy ?? 0)
  host.dataset.blockoutPbrMaxAnisotropy = String(facts?.maxAnisotropy ?? 0)
  host.dataset.blockoutDisplayScale = String(display?.displayScale ?? 0)
  host.dataset.blockoutDisplayDiagonalMm = String(display?.displayDiagonalMm ?? 0)
  host.dataset.blockoutSourceBoundsMm = JSON.stringify(display?.sourceBoundsMm ?? [])
  host.dataset.blockoutFrameNdc = JSON.stringify(display?.frameNdc ?? {})
  host.dataset.blockoutFogNearMm = String(runtime.scene.fog instanceof THREE.Fog ? runtime.scene.fog.near : 0)
  host.dataset.blockoutFogFarMm = String(runtime.scene.fog instanceof THREE.Fog ? runtime.scene.fog.far : 0)
}

function applyAgentBlockoutMeshVisualState(mesh: THREE.Mesh, props: ModuleGraphViewportProps): void {
  const partRole = typeof mesh.userData.forgecad_part_role === 'string'
    ? mesh.userData.forgecad_part_role
    : typeof mesh.userData.partRole === 'string'
      ? mesh.userData.partRole
      : ''
  const matchesPart = (partId: string | null): boolean => Boolean(partId && partRole && partId.endsWith(`_${partRole}`))
  const selected = matchesPart(props.selectedAgentPartId)
  const hidden = props.hiddenAgentPartIds.some((partId) => matchesPart(partId))
  const isolatedAway = Boolean(props.isolatedAgentPartId && !matchesPart(props.isolatedAgentPartId))
  const locked = props.lockedAgentPartIds.some((partId) => matchesPart(partId))
  mesh.visible = !hidden && !isolatedAway
  mesh.userData.agentPartLocked = locked
  const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material]
  materials.forEach((material) => {
    if ('wireframe' in material) material.wireframe = props.wireframe
    if (!('transparent' in material) || !('opacity' in material) || !('depthWrite' in material)) return
    const baseline = material.userData.forgecadBlockoutVisualBaseline as {
      transparent: boolean
      opacity: number
      depthWrite: boolean
      emissive?: number
      emissiveIntensity?: number
    } | undefined
    const original = baseline ?? {
      transparent: material.transparent,
      opacity: material.opacity,
      depthWrite: material.depthWrite,
      ...(isPbrMaterial(material)
        ? { emissive: material.emissive.getHex(), emissiveIntensity: material.emissiveIntensity }
        : {}),
    }
    if (!baseline) material.userData.forgecadBlockoutVisualBaseline = original
    material.transparent = props.ghostPreview || props.xRay || original.transparent
    material.opacity = props.ghostPreview ? 0.58 : props.xRay ? 0.24 : original.opacity
    material.depthWrite = props.ghostPreview || props.xRay ? false : original.depthWrite
    if (isPbrMaterial(material)) {
      material.emissive.setHex(original.emissive ?? 0)
      material.emissiveIntensity = original.emissiveIntensity ?? 1
      if (selected) {
        material.emissive.set('#1f64a8')
        material.emissiveIntensity = 0.42
      } else if (locked) {
        material.emissive.set('#9b6e21')
        material.emissiveIntensity = 0.24
      }
    }
  })
}

function applyLightPreset(runtime: ViewportRuntime, preset: LightPreset) {
  const recordEnvironment = () => {
    const host = runtime.renderer.domElement.parentElement
    if (host instanceof HTMLElement) recordAppliedVisualEnvironment(runtime, host)
  }
  if (preset === 'soft_studio') {
    runtime.keyLight.color.set('#e8f2ff')
    runtime.keyLight.intensity = 4.2
    runtime.keyLight.position.set(100, 150, 120)
    runtime.rimLight.color.set('#6ea9d9')
    runtime.rimLight.intensity = 1.8
    runtime.rimLight.position.set(-110, 70, -80)
    runtime.warmRimLight.color.set('#ffad86')
    runtime.warmRimLight.intensity = 0.4
    runtime.warmRimLight.position.set(70, -10, -150)
    recordEnvironment()
    return
  }
  if (preset === 'concept_contrast') {
    runtime.keyLight.color.set('#ffffff')
    runtime.keyLight.intensity = 7
    runtime.keyLight.position.set(150, 210, 100)
    runtime.rimLight.color.set('#3d8dff')
    runtime.rimLight.intensity = 4
    runtime.rimLight.position.set(-170, 100, -130)
    runtime.warmRimLight.color.set('#ff724e')
    runtime.warmRimLight.intensity = 2.5
    runtime.warmRimLight.position.set(100, -30, -210)
    recordEnvironment()
    return
  }
  const neutral = FORGECAD_STUDIO_MANIFEST.cad_neutral_lighting
  runtime.keyLight.color.set(neutral.key.color)
  runtime.keyLight.intensity = neutral.key.intensity
  runtime.keyLight.position.set(...neutral.key.position)
  runtime.rimLight.color.set(neutral.rim.color)
  runtime.rimLight.intensity = neutral.rim.intensity
  runtime.rimLight.position.set(...neutral.rim.position)
  runtime.warmRimLight.color.set(neutral.warm_rim.color)
  runtime.warmRimLight.intensity = neutral.warm_rim.intensity
  runtime.warmRimLight.position.set(...neutral.warm_rim.position)
  recordEnvironment()
}

function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  if (value && typeof value === 'object') {
    return `{${Object.entries(value as Record<string, unknown>)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${canonicalJson(item)}`)
      .join(',')}}`
  }
  return JSON.stringify(value) ?? 'null'
}

function colorHex(color: THREE.Color): string {
  return `#${color.getHexString(THREE.SRGBColorSpace)}`
}

function recordAppliedVisualEnvironment(runtime: ViewportRuntime, host: HTMLElement): void {
  const floorMaterial = runtime.displayFloor.material
  const manifest = {
    schema_version: FORGECAD_STUDIO_MANIFEST.schema_version,
    environment_id: FORGECAD_STUDIO_MANIFEST.environment_id,
    environment_kind: FORGECAD_STUDIO_MANIFEST.environment_kind,
    source: FORGECAD_STUDIO_MANIFEST.source,
    license: FORGECAD_STUDIO_MANIFEST.license,
    color_workflow: FORGECAD_STUDIO_MANIFEST.color_workflow,
    output_color_space: runtime.renderer.outputColorSpace === THREE.SRGBColorSpace ? 'srgb' : 'unsupported',
    tone_mapping: runtime.renderer.toneMapping === THREE.ACESFilmicToneMapping ? 'aces_filmic' : 'unsupported',
    tone_mapping_exposure: runtime.renderer.toneMappingExposure,
    contact_shadows: runtime.renderer.shadowMap.enabled && runtime.keyLight.castShadow && runtime.displayFloor.receiveShadow,
    pmrem: FORGECAD_STUDIO_MANIFEST.pmrem,
    cad_neutral_lighting: {
      background: runtime.scene.background instanceof THREE.Color ? colorHex(runtime.scene.background) : 'unsupported',
      hemisphere: {
        sky: colorHex(runtime.hemisphereLight.color),
        ground: colorHex(runtime.hemisphereLight.groundColor),
        intensity: runtime.hemisphereLight.intensity,
      },
      ambient: { color: colorHex(runtime.ambientLight.color), intensity: runtime.ambientLight.intensity },
      key: { color: colorHex(runtime.keyLight.color), intensity: runtime.keyLight.intensity, position: runtime.keyLight.position.toArray() },
      rim: { color: colorHex(runtime.rimLight.color), intensity: runtime.rimLight.intensity, position: runtime.rimLight.position.toArray() },
      warm_rim: {
        color: colorHex(runtime.warmRimLight.color),
        intensity: runtime.warmRimLight.intensity,
        position: runtime.warmRimLight.position.toArray(),
      },
      floor: {
        kind: FORGECAD_STUDIO_MANIFEST.cad_neutral_lighting.floor.kind,
        color: colorHex(floorMaterial.color),
        opacity: floorMaterial.opacity,
        radius_ratio: FORGECAD_STUDIO_MANIFEST.cad_neutral_lighting.floor.radius_ratio,
      },
    },
    camera_views: {
      iso: {
        direction: FORGECAD_STUDIO_MANIFEST.camera_views.iso.direction,
        distance_ratio: FORGECAD_STUDIO_MANIFEST.camera_views.iso.distance_ratio,
        fov_degrees: runtime.camera.fov,
      },
    },
  }
  host.dataset.visualEnvironmentRecipe = canonicalJson(manifest)
}

function syncTransformControls(runtime: ViewportRuntime, props: ModuleGraphViewportProps) {
  const transformControls = runtime.transformControls
  const selected = runtime.nodeObjects.get(props.selectedNodeId)
  const selectedNode = runtime.graph?.nodes.find((node) => node.node_id === props.selectedNodeId)
  const canTransform = Boolean(
    selected
      && selectedNode
      && !selectedNode.locked
      && selectedNode.node_id !== runtime.graph?.root_node_id
      && props.transformTool !== 'none'
      && !props.ghostPreview
      && props.explodeFactor === 0,
  )
  if (!canTransform || !selected) {
    transformControls.detach()
    runtime.transformHelper.visible = false
    return
  }
  runtime.transformHelper.visible = true
  if (props.transformTool === 'none') return
  transformControls.setMode(props.transformTool)
  transformControls.setSpace(props.transformSpace)
  transformControls.setTranslationSnap(props.snapEnabled ? 1 : null)
  transformControls.setRotationSnap(props.snapEnabled ? Math.PI / 12 : null)
  transformControls.setScaleSnap(props.snapEnabled ? 0.05 : null)
  if (transformControls.object !== selected) transformControls.attach(selected)
}

function transformFromObject(object: THREE.Object3D): Transform {
  return {
    position: [object.position.x, object.position.y, object.position.z],
    rotation: [object.rotation.x, object.rotation.y, object.rotation.z],
    // Mirroring is stored separately on the ModuleGraph node; a transform edit must
    // preserve that invariant rather than serializing a negative scale into the contract.
    scale: [Math.abs(object.scale.x), Math.abs(object.scale.y), Math.abs(object.scale.z)],
  }
}

function frameVisibleObjects(runtime: ViewportRuntime, props: ModuleGraphViewportProps) {
  const targets = (props.qualityHighlightNodeIds.length ? props.qualityHighlightNodeIds : props.focusNodeId ? [props.focusNodeId] : [])
    .map((nodeId) => runtime.nodeObjects.get(nodeId))
    .filter((item): item is THREE.Group => item !== undefined)
  let bounds = targets.length
    ? targets.reduce((combined, item) => combined.union(new THREE.Box3().setFromObject(item)), new THREE.Box3())
    : new THREE.Box3().setFromObject(runtime.moduleRoot)
  if (bounds.isEmpty() && targets.length) bounds = new THREE.Box3().setFromObject(runtime.moduleRoot)
  if (bounds.isEmpty()) {
    runtime.displayFloor.position.set(0, -1, 0)
    runtime.displayFloor.scale.setScalar(
      240 * 0.5 * FORGECAD_STUDIO_MANIFEST.cad_neutral_lighting.floor.radius_ratio,
    )
    fitPreviewShadowCamera(runtime, new THREE.Vector3(240, 240, 240))
    frameCamera(runtime.camera, runtime.controls, props.cameraView, new THREE.Vector3(), 240)
    return
  }
  const center = bounds.getCenter(new THREE.Vector3())
  const size = bounds.getSize(new THREE.Vector3())
  runtime.displayFloor.position.set(center.x, bounds.min.y - Math.max(size.y * 0.01, 1), center.z)
  runtime.displayFloor.scale.setScalar(
    Math.max(Math.hypot(size.x, size.z) * 0.5, 1)
      * FORGECAD_STUDIO_MANIFEST.cad_neutral_lighting.floor.radius_ratio,
  )
  fitPreviewShadowCamera(runtime, size)
  frameCamera(runtime.camera, runtime.controls, props.cameraView, center, Math.max(size.length(), 1))
}

function fitPreviewShadowCamera(runtime: ViewportRuntime, size: THREE.Vector3): void {
  const extent = Math.max(size.x, size.y, size.z, 1) * 0.72
  const shadowCamera = runtime.keyLight.shadow.camera
  shadowCamera.left = -extent
  shadowCamera.right = extent
  shadowCamera.top = extent
  shadowCamera.bottom = -extent
  shadowCamera.near = 1
  shadowCamera.far = Math.max(runtime.keyLight.position.length() + size.length() * 2, 900)
  shadowCamera.updateProjectionMatrix()
}

function base64ToArrayBuffer(value: string): ArrayBuffer {
  const binary = window.atob(value)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index)
  return bytes.buffer
}

function buildQualityOverlay(references: NonNullable<QualityFinding['geometry_refs']>): THREE.Group {
  const group = new THREE.Group()
  references.forEach((reference, referenceIndex) => {
    const positions: number[] = []
    for (const triangle of reference.world_triangles_mm ?? []) {
      if (triangle.length !== 3 || triangle.some((point) => point.length !== 3)) continue
      const [first, second, third] = triangle
      positions.push(...first, ...second, ...second, ...third, ...third, ...first)
    }
    if (!positions.length) return
    const geometry = new THREE.BufferGeometry()
    geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3))
    const material = new THREE.LineBasicMaterial({ color: referenceIndex % 2 === 0 ? '#ff4d4d' : '#ffb347', depthTest: false, transparent: true, opacity: 0.98 })
    const lines = new THREE.LineSegments(geometry, material)
    lines.renderOrder = 30
    group.add(lines)
  })
  return group
}

function clearNodeObjects(runtime: ViewportRuntime) {
  runtime.nodeObjects.forEach((object) => {
    runtime.moduleRoot.remove(object)
    disposeNodeInstance(object)
  })
  runtime.nodeObjects.clear()
}

function reconcileNodeObjects(runtime: ViewportRuntime, graph: Graph) {
  const expectedModules = new Map(graph.nodes.map((node) => [node.node_id, node.module_id]))
  runtime.nodeObjects.forEach((object, nodeId) => {
    if (expectedModules.get(nodeId) === object.userData.moduleId) return
    runtime.moduleRoot.remove(object)
    disposeNodeInstance(object)
    runtime.nodeObjects.delete(nodeId)
  })
}

function createReferenceImageHeaderPlane(
  width: number,
  y: number,
  referenceClass: 'single_image' | 'multi_view_contact_sheet',
): THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial> {
  const canvas = document.createElement('canvas')
  canvas.width = 1024
  canvas.height = 112
  const context = canvas.getContext('2d')
  if (!context) throw new Error('只读参考图片标签无法创建')
  context.clearRect(0, 0, canvas.width, canvas.height)
  context.fillStyle = '#10243a'
  context.fillRect(0, 0, canvas.width, canvas.height)
  context.fillStyle = '#55b9ff'
  context.font = '600 42px system-ui, sans-serif'
  context.textBaseline = 'middle'
  context.fillText('只读参考图', 30, 56)
  context.fillStyle = '#b5c8db'
  context.font = '30px system-ui, sans-serif'
  context.textAlign = 'right'
  context.fillText(referenceClass === 'multi_view_contact_sheet' ? '多视图联系表 · 非几何真值' : '单图线索 · 非几何真值', 994, 56)
  const texture = new THREE.CanvasTexture(canvas)
  texture.colorSpace = THREE.SRGBColorSpace
  texture.minFilter = THREE.LinearFilter
  texture.magFilter = THREE.LinearFilter
  const header = new THREE.Mesh(
    new THREE.PlaneGeometry(width, Math.min(width * (canvas.height / canvas.width), 40)),
    new THREE.MeshBasicMaterial({ map: texture, transparent: true, toneMapped: false, side: THREE.DoubleSide }),
  )
  header.name = 'ReadOnlyReferenceImageHeader'
  header.position.set(0, y, 0.1)
  return header
}

function clearObjectChildren(root: THREE.Object3D) {
  for (const child of [...root.children]) {
    root.remove(child)
    disposeObject(child)
  }
}

function disposeNodeInstance(root: THREE.Object3D) {
  const materials = new Set<THREE.Material>()
  root.traverse((object) => {
    if (object instanceof THREE.Mesh) {
      if (object.userData.forgecadConnectorMarker) return
      const values = Array.isArray(object.material) ? object.material : [object.material]
      values.forEach((item) => materials.add(item))
    }
  })
  materials.forEach((material) => material.dispose())
}

function frameCamera(camera: THREE.PerspectiveCamera, controls: OrbitControls, view: CameraView, center: THREE.Vector3, size: number) {
  // Keep the assembled concept prominent like a CAD presentation viewport;
  // the old distance left too much empty grid around compact module packs.
  const distance = Math.max(size * FORGECAD_STUDIO_MANIFEST.camera_views.iso.distance_ratio, 1)
  const direction: Record<CameraView, THREE.Vector3> = {
    // In the exported Y-up coordinate system, positive Y is above the prop.
    // Keep a prominent depth component while opening the X/Y angle enough to
    // show the top rails and lower display grip on first launch.
    iso: new THREE.Vector3(...FORGECAD_STUDIO_MANIFEST.camera_views.iso.direction), front: new THREE.Vector3(0, 0.08, 1), top: new THREE.Vector3(0, 1, 0.001), right: new THREE.Vector3(1, 0.08, 0),
  }
  camera.position.copy(center).add(direction[view].normalize().multiplyScalar(distance))
  camera.near = Math.max(distance / 1000, 0.001)
  camera.far = Math.max(distance * 20, 100)
  camera.updateProjectionMatrix()
  controls.target.copy(center)
  controls.minDistance = Math.max(size * 0.05, 0.01)
  controls.maxDistance = Math.max(size * 10, 10)
  controls.update()
}

function frameBlockoutCameraToBounds(
  runtime: ViewportRuntime,
  view: CameraView,
  bounds: THREE.Box3,
): BlockoutFrameNdcFacts {
  const { camera, controls } = runtime
  const center = bounds.getCenter(new THREE.Vector3())
  const size = bounds.getSize(new THREE.Vector3())
  const corners = boxCorners(bounds)
  const direction: Record<CameraView, THREE.Vector3> = {
    iso: new THREE.Vector3(...FORGECAD_STUDIO_MANIFEST.camera_views.iso.direction),
    front: new THREE.Vector3(0, 0.08, 1),
    top: new THREE.Vector3(0, 1, 0.001),
    right: new THREE.Vector3(1, 0.08, 0),
  }
  controls.target.copy(center)
  camera.position.copy(center).add(direction[view].normalize())
  controls.update()
  camera.updateMatrixWorld(true)

  // Use the actual OrbitControls camera basis so near-vertical views and the
  // live viewport aspect ratio participate in the fit. A diagonal-only
  // distance can pass loading checks while clipping a wide vehicle or tall arm.
  const right = new THREE.Vector3().setFromMatrixColumn(camera.matrixWorld, 0).normalize()
  const up = new THREE.Vector3().setFromMatrixColumn(camera.matrixWorld, 1).normalize()
  const backward = new THREE.Vector3().setFromMatrixColumn(camera.matrixWorld, 2).normalize()
  const tangentY = Math.tan(THREE.MathUtils.degToRad(camera.fov) / 2)
  const tangentX = tangentY * Math.max(camera.aspect, 0.01)
  let distance = 1
  for (const corner of corners) {
    const delta = corner.clone().sub(center)
    const widthDistance = Math.abs(delta.dot(right)) / (BLOCKOUT_FRAME_TARGET_NDC * tangentX)
    const heightDistance = Math.abs(delta.dot(up)) / (BLOCKOUT_FRAME_TARGET_NDC * tangentY)
    distance = Math.max(distance, delta.dot(backward) + Math.max(widthDistance, heightDistance))
  }
  distance *= 1.01
  camera.position.copy(center).add(backward.multiplyScalar(distance))
  camera.near = Math.max(distance / 1000, 0.001)
  camera.far = Math.max(distance * 20, 100)
  camera.updateProjectionMatrix()
  controls.minDistance = Math.max(size.length() * 0.05, 0.01)
  controls.maxDistance = Math.max(size.length() * 10, 10)
  controls.update()
  camera.updateMatrixWorld(true)
  if (runtime.scene.fog instanceof THREE.Fog) {
    // Keep the entire object ahead of the studio haze. The old fixed fog
    // depth made a correctly framed wide/tall model almost black.
    runtime.scene.fog.near = Math.max(300, distance + size.length())
    runtime.scene.fog.far = runtime.scene.fog.near + Math.max(520, distance * 1.5)
  }

  const projected = corners.map((corner) => corner.clone().project(camera))
  return {
    minX: Math.min(...projected.map((point) => point.x)),
    maxX: Math.max(...projected.map((point) => point.x)),
    minY: Math.min(...projected.map((point) => point.y)),
    maxY: Math.max(...projected.map((point) => point.y)),
    cameraDistanceMm: distance,
  }
}

function boxCorners(bounds: THREE.Box3): THREE.Vector3[] {
  return [
    new THREE.Vector3(bounds.min.x, bounds.min.y, bounds.min.z),
    new THREE.Vector3(bounds.min.x, bounds.min.y, bounds.max.z),
    new THREE.Vector3(bounds.min.x, bounds.max.y, bounds.min.z),
    new THREE.Vector3(bounds.min.x, bounds.max.y, bounds.max.z),
    new THREE.Vector3(bounds.max.x, bounds.min.y, bounds.min.z),
    new THREE.Vector3(bounds.max.x, bounds.min.y, bounds.max.z),
    new THREE.Vector3(bounds.max.x, bounds.max.y, bounds.min.z),
    new THREE.Vector3(bounds.max.x, bounds.max.y, bounds.max.z),
  ]
}

function disposeObject(root: THREE.Object3D) {
  const textures = new Set<THREE.Texture>()
  root.traverse((object) => {
    if (!(object instanceof THREE.Mesh || object instanceof THREE.LineSegments)) return
    object.geometry?.dispose()
    const materials = Array.isArray(object.material) ? object.material : [object.material]
    materials.forEach((material) => {
      if (!material) return
      Object.values(material).forEach((value) => { if (value instanceof THREE.Texture) textures.add(value) })
      material.dispose()
    })
    if (object instanceof THREE.SkinnedMesh) object.skeleton.dispose()
  })
  textures.forEach((texture) => texture.dispose())
}
