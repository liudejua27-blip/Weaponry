import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { TransformControls } from 'three/examples/jsm/controls/TransformControls.js'
import type { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import type { ModuleAssetRecord, ModuleGraphRecord, QualityFinding, Transform } from '../../shared/types'

type CameraView = 'iso' | 'front' | 'top' | 'right'
type TransformTool = 'none' | 'translate' | 'rotate' | 'scale'
type Graph = NonNullable<ModuleGraphRecord>['graph']
export type ViewportMeasurementPoint = {
  nodeId: string
  position: [number, number, number]
  normal: [number, number, number]
}

const GLB_METERS_TO_WORKBENCH_MILLIMETERS = 1000
let viewportRendererGeneration = 0
let activeViewportContexts = 0

type ModuleGraphViewportProps = {
  graphRecord: ModuleGraphRecord | null
  modules: ModuleAssetRecord[]
  cameraView: CameraView
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
  displayFloor: THREE.Mesh<THREE.CircleGeometry, THREE.MeshStandardMaterial>
  moduleRoot: THREE.Group
  qualityRoot: THREE.Group
  connectorGeometry: THREE.SphereGeometry
  connectorMaterials: { exclusive: THREE.MeshBasicMaterial; shared: THREE.MeshBasicMaterial }
  nodeObjects: Map<string, THREE.Group>
  moduleCache: Map<string, Promise<THREE.Group>>
  graph: Graph | null
  modulesById: Map<string, ModuleAssetRecord>
  scheduleRender: () => void
}

export function ModuleGraphViewport(props: ModuleGraphViewportProps) {
  const hostRef = useRef<HTMLDivElement | null>(null)
  const runtimeRef = useRef<ViewportRuntime | null>(null)
  const propsRef = useRef(props)
  propsRef.current = props
  const [loadState, setLoadState] = useState<'empty' | 'loading' | 'ready' | 'failed'>(
    props.graphRecord ? 'loading' : 'empty',
  )
  const [loadMessage, setLoadMessage] = useState('当前版本尚未绑定 ModuleGraph')

  // Renderer, Scene and camera exist for the lifetime of the panel only. Selection,
  // overlay and wireframe updates below never destroy the WebGL context.
  useEffect(() => {
    const host = hostRef.current
    if (!host) return
    const scene = new THREE.Scene()
    scene.background = new THREE.Color('#08111c')
    scene.fog = new THREE.Fog('#08111c', 260, 720)
    const camera = new THREE.PerspectiveCamera(38, 1, 0.01, 100000)
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false })
    renderer.localClippingEnabled = true
    viewportRendererGeneration += 1
    activeViewportContexts += 1
    host.dataset.rendererGeneration = String(viewportRendererGeneration)
    host.dataset.activeWebglContexts = String(activeViewportContexts)
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    renderer.outputColorSpace = THREE.SRGBColorSpace
    renderer.toneMapping = THREE.ACESFilmicToneMapping
    renderer.toneMappingExposure = 1.18
    renderer.shadowMap.enabled = true
    renderer.shadowMap.type = THREE.PCFSoftShadowMap
    host.appendChild(renderer.domElement)

    const controls = new OrbitControls(camera, renderer.domElement)
    controls.enableDamping = false
    controls.minDistance = 0.01
    controls.maxDistance = 100000
    scene.add(new THREE.HemisphereLight('#c8ddff', '#07101a', 2.65))
    scene.add(new THREE.AmbientLight('#4f6686', 0.42))
    const keyLight = new THREE.DirectionalLight('#e8f2ff', 5.6)
    keyLight.position.set(120, 180, 140)
    keyLight.castShadow = true
    keyLight.shadow.mapSize.set(2048, 2048)
    keyLight.shadow.camera.near = 1
    keyLight.shadow.camera.far = 900
    keyLight.shadow.camera.left = -360
    keyLight.shadow.camera.right = 360
    keyLight.shadow.camera.top = 360
    keyLight.shadow.camera.bottom = -360
    scene.add(keyLight)
    const rimLight = new THREE.DirectionalLight('#4e9cff', 3.2)
    rimLight.position.set(-140, 80, -100)
    scene.add(rimLight)
    const warmRimLight = new THREE.DirectionalLight('#ff8a62', 1.4)
    warmRimLight.position.set(80, -20, -180)
    scene.add(warmRimLight)
    const displayFloor = new THREE.Mesh(
      new THREE.CircleGeometry(285, 96),
      new THREE.MeshStandardMaterial({ color: '#0a1420', metalness: 0.5, roughness: 0.7, transparent: true, opacity: 0.78 }),
    )
    displayFloor.rotation.x = -Math.PI / 2
    displayFloor.receiveShadow = true
    displayFloor.name = 'DisplayFloor'
    scene.add(displayFloor)
    const grid = new THREE.GridHelper(420, 42, '#2f66ff', '#263747')
    scene.add(grid)
    scene.add(new THREE.AxesHelper(28))
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
    const qualityRoot = new THREE.Group()
    qualityRoot.name = 'QualityGeometryOverlay'
    scene.add(qualityRoot)
    const connectorGeometry = new THREE.SphereGeometry(1, 16, 12)
    const connectorMaterials = {
      exclusive: new THREE.MeshBasicMaterial({ color: '#42c8ff', depthTest: false, transparent: true, opacity: 0.9 }),
      shared: new THREE.MeshBasicMaterial({ color: '#f1b84b', depthTest: false, transparent: true, opacity: 0.9 }),
    }

    let frame = 0
    const render = () => {
      frame = 0
      renderer.render(scene, camera)
      host.dataset.rendererGeometries = String(renderer.info.memory.geometries)
      host.dataset.rendererTextures = String(renderer.info.memory.textures)
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
      displayFloor,
      moduleRoot,
      qualityRoot,
      connectorGeometry,
      connectorMaterials,
      nodeObjects: new Map(),
      moduleCache: new Map(),
      graph: null,
      modulesById: new Map(),
      scheduleRender,
    }
    runtimeRef.current = runtime

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
      const hit = raycaster.intersectObjects(moduleRoot.children, true)[0]
      const nodeId = hit?.object.userData.nodeId
      if (typeof nodeId !== 'string' || !hit || !hit.face) return null
      const normal = hit.face.normal.clone().transformDirection(hit.object.matrixWorld)
      return { nodeId, point: hit.point, normal }
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
      camera.aspect = width / Math.max(height, 1)
      camera.updateProjectionMatrix()
      scheduleRender()
    }
    const observer = new ResizeObserver(resize)
    observer.observe(host)
    resize()

    return () => {
      cancelAnimationFrame(frame)
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
      runtime.moduleCache.forEach((source) => { void source.then(disposeObject) })
      clearObjectChildren(qualityRoot)
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
      setLoadMessage('当前版本尚未绑定 ModuleGraph')
      frameCamera(runtime.camera, runtime.controls, props.cameraView, new THREE.Vector3(), 240)
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
        frameVisibleObjects(runtime, propsRef.current)
        setLoadState('ready')
        setLoadMessage(`已加载 ${graph.nodes.length} 个真实 GLB 节点`)
        runtime.scheduleRender()
      })
      .catch((caught) => {
        if (cancelled) return
        setLoadState('failed')
        setLoadMessage(caught instanceof Error ? caught.message : String(caught))
        frameCamera(runtime.camera, runtime.controls, propsRef.current.cameraView, new THREE.Vector3(), 240)
        runtime.scheduleRender()
      })
    return () => { cancelled = true }
  }, [graphHash, props.getModuleFileUrl, props.modules])

  useEffect(() => {
    const runtime = runtimeRef.current
    if (!runtime) return
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
    clearObjectChildren(runtime.qualityRoot)
    runtime.qualityRoot.add(buildQualityOverlay(props.qualityGeometryRefs))
    runtime.scheduleRender()
  }, [props.qualityGeometryRefs])

  useEffect(() => {
    const runtime = runtimeRef.current
    if (!runtime || runtime.nodeObjects.size === 0) return
    frameVisibleObjects(runtime, propsRef.current)
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
        data-xray={props.xRay ? 'enabled' : 'disabled'}
        data-section={props.sectionEnabled ? 'enabled' : 'disabled'}
        data-section-offset={String(props.sectionOffset)}
        data-focus-node-id={props.focusNodeId ?? ''}
        data-quality-node-ids={props.qualityHighlightNodeIds.join(',')}
        data-quality-triangle-count={props.qualityGeometryRefs.reduce(
          (count, reference) => count + (reference.world_triangles_mm?.length ?? 0),
          0,
        )}
      />
      {loadState !== 'ready' && (
        <div className={`viewport-data-state ${loadState}`} role="status">
          <strong>{loadState === 'loading' ? '加载 ModuleGraph' : loadState === 'failed' ? 'GLB 无法显示' : '等待模块组合'}</strong>
          <span>{loadMessage}</span>
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
  runtime.grid.visible = props.showGrid
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
    frameCamera(runtime.camera, runtime.controls, props.cameraView, new THREE.Vector3(), 240)
    return
  }
  const center = bounds.getCenter(new THREE.Vector3())
  const size = bounds.getSize(new THREE.Vector3())
  runtime.displayFloor.position.set(center.x, bounds.min.y - Math.max(size.y * 0.13, 4), center.z)
  runtime.displayFloor.scale.setScalar(Math.max(size.length() / 190, 0.65))
  frameCamera(runtime.camera, runtime.controls, props.cameraView, center, Math.max(size.length(), 1))
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
  const distance = Math.max(size * 0.98, 1)
  const direction: Record<CameraView, THREE.Vector3> = {
    // In the exported Y-up coordinate system, positive Y is above the prop.
    // Keep a prominent depth component while opening the X/Y angle enough to
    // show the top rails and lower display grip on first launch.
    iso: new THREE.Vector3(0.9, 0.85, 1.55), front: new THREE.Vector3(0, 0.08, 1), top: new THREE.Vector3(0, 1, 0.001), right: new THREE.Vector3(1, 0.08, 0),
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
