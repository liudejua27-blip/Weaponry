import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import type { ModuleAssetRecord, ModuleGraphRecord, QualityFinding } from '../../shared/types'

type CameraView = 'iso' | 'front' | 'top' | 'right'

const GLB_METERS_TO_WORKBENCH_MILLIMETERS = 1000
let viewportRendererGeneration = 0
let activeViewportContexts = 0

type ModuleGraphViewportProps = {
  graphRecord: ModuleGraphRecord | null
  modules: ModuleAssetRecord[]
  cameraView: CameraView
  showGrid: boolean
  wireframe: boolean
  selectedNodeId: string
  hiddenNodeIds: string[]
  focusNodeId: string | null
  qualityHighlightNodeIds: string[]
  qualityGeometryRefs: NonNullable<QualityFinding['geometry_refs']>
  showConnectors: boolean
  explodeFactor: number
  getModuleFileUrl: (moduleId: string) => string
  onSelectNode: (nodeId: string) => void
  onDropModule: (nodeId: string, moduleId: string) => void
}

export function ModuleGraphViewport({
  graphRecord,
  modules,
  cameraView,
  showGrid,
  wireframe,
  selectedNodeId,
  hiddenNodeIds,
  focusNodeId,
  qualityHighlightNodeIds,
  qualityGeometryRefs,
  showConnectors,
  explodeFactor,
  getModuleFileUrl,
  onSelectNode,
  onDropModule,
}: ModuleGraphViewportProps) {
  const hostRef = useRef<HTMLDivElement | null>(null)
  const [loadState, setLoadState] = useState<'empty' | 'loading' | 'ready' | 'failed'>(
    graphRecord ? 'loading' : 'empty',
  )
  const [loadMessage, setLoadMessage] = useState('当前版本尚未绑定 ModuleGraph')

  useEffect(() => {
    const host = hostRef.current
    if (!host) return
    const graph = graphRecord?.graph
    setLoadState(graph ? 'loading' : 'empty')
    setLoadMessage(graph ? '正在读取不可变 GLB 模块…' : '当前版本尚未绑定 ModuleGraph')

    const scene = new THREE.Scene()
    scene.background = new THREE.Color('#101823')
    const camera = new THREE.PerspectiveCamera(38, 1, 0.01, 100000)
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false })
    viewportRendererGeneration += 1
    activeViewportContexts += 1
    host.dataset.rendererGeneration = String(viewportRendererGeneration)
    host.dataset.activeWebglContexts = String(activeViewportContexts)
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    renderer.outputColorSpace = THREE.SRGBColorSpace
    renderer.shadowMap.enabled = true
    host.appendChild(renderer.domElement)

    const controls = new OrbitControls(camera, renderer.domElement)
    controls.enableDamping = true
    controls.minDistance = 0.01
    controls.maxDistance = 100000

    scene.add(new THREE.HemisphereLight('#d9e7ff', '#17202a', 3.1))
    scene.add(new THREE.AmbientLight('#8aa2c4', 0.75))
    const keyLight = new THREE.DirectionalLight('#ffffff', 4.2)
    keyLight.position.set(120, 180, 140)
    keyLight.castShadow = true
    scene.add(keyLight)
    const rimLight = new THREE.DirectionalLight('#4895ff', 1.8)
    rimLight.position.set(-140, 80, -100)
    scene.add(rimLight)

    const grid = new THREE.GridHelper(420, 42, '#2f66ff', '#263747')
    grid.visible = showGrid
    scene.add(grid)
    scene.add(new THREE.AxesHelper(28))
    const moduleRoot = new THREE.Group()
    moduleRoot.name = 'ModuleGraphRoot'
    scene.add(moduleRoot)
    const qualityOverlay = buildQualityOverlay(qualityGeometryRefs)
    scene.add(qualityOverlay)

    let disposed = false
    const loader = new GLTFLoader()
    const rootPosition = new THREE.Vector3(
      ...(graph?.nodes.find((node) => node.node_id === graph.root_node_id)?.transform.position
        ?? [0, 0, 0]) as [number, number, number],
    )
    const loadNode = (node: NonNullable<typeof graph>['nodes'][number]) => new Promise<void>((resolve, reject) => {
      loader.load(
        getModuleFileUrl(node.module_id),
        (gltf) => {
          if (disposed) {
            disposeObject(gltf.scene)
            resolve()
            return
          }
          const object = new THREE.Group()
          const assetScene = gltf.scene
          assetScene.scale.setScalar(GLB_METERS_TO_WORKBENCH_MILLIMETERS)
          object.add(assetScene)
          const moduleRecord = modules.find(
            (item) => item.manifest.module_id === node.module_id,
          )
          object.name = node.node_id
          object.userData.nodeId = node.node_id
          const [px = 0, py = 0, pz = 0] = node.transform.position
          const [rx = 0, ry = 0, rz = 0] = node.transform.rotation
          const [sx = 1, sy = 1, sz = 1] = node.transform.scale
          const mirrorAxis = node.mirror_axis ?? 'none'
          const mirrorScale = {
            x: mirrorAxis === 'x' ? -1 : 1,
            y: mirrorAxis === 'y' ? -1 : 1,
            z: mirrorAxis === 'z' ? -1 : 1,
          }
          object.position.set(px, py, pz)
          if (explodeFactor > 0 && node.node_id !== graph?.root_node_id) {
            const direction = object.position.clone().sub(rootPosition)
            if (direction.lengthSq() < 0.0001) {
              const nodeIndex = graph?.nodes.findIndex((item) => item.node_id === node.node_id) ?? 1
              direction.set(nodeIndex % 2 === 0 ? 1 : -1, nodeIndex % 3 === 0 ? 0.5 : 0, 0)
            }
            const extent = Math.max(...(moduleRecord?.manifest.bounds_mm ?? [50]))
            object.position.add(direction.normalize().multiplyScalar(extent * explodeFactor))
          }
          object.rotation.set(rx, ry, rz)
          object.scale.set(
            sx * mirrorScale.x,
            sy * mirrorScale.y,
            sz * mirrorScale.z,
          )
          object.visible = node.visible !== false && !hiddenNodeIds.includes(node.node_id)
          object.traverse((child) => {
            child.userData.nodeId = node.node_id
            if (!(child instanceof THREE.Mesh)) return
            child.castShadow = true
            child.receiveShadow = true
            const sourceMaterials = Array.isArray(child.material) ? child.material : [child.material]
            const materials = sourceMaterials.map((material) => {
              const clone = material.clone()
              if ('wireframe' in clone) clone.wireframe = wireframe
              if (clone instanceof THREE.MeshStandardMaterial || clone instanceof THREE.MeshPhysicalMaterial) {
                const qualityHighlighted = qualityHighlightNodeIds.includes(node.node_id)
                clone.emissive.set(
                  qualityHighlighted
                    ? '#b62424'
                    : node.node_id === selectedNodeId
                    ? '#1f64a8'
                    : '#000000',
                )
                clone.emissiveIntensity = qualityHighlighted
                  ? 0.72
                  : node.node_id === selectedNodeId
                  ? 0.42
                  : 0
              }
              return clone
            })
            child.material = Array.isArray(child.material) ? materials : materials[0]
          })
          if (showConnectors) {
            const markerRadius = Math.max(
              ...(moduleRecord?.manifest.bounds_mm ?? [10]),
            ) * 0.035
            for (const connector of moduleRecord?.manifest.connectors ?? []) {
              const marker = new THREE.Mesh(
                new THREE.SphereGeometry(Math.max(markerRadius, 0.5), 16, 12),
                new THREE.MeshBasicMaterial({
                  color: connector.exclusive === false ? '#f1b84b' : '#42c8ff',
                  depthTest: false,
                  transparent: true,
                  opacity: 0.9,
                }),
              )
              const [cx = 0, cy = 0, cz = 0] = connector.transform.position
              marker.position.set(cx, cy, cz)
              marker.name = connector.connector_id
              marker.renderOrder = 10
              marker.userData.nodeId = node.node_id
              marker.userData.connectorId = connector.connector_id
              object.add(marker)
            }
          }
          moduleRoot.add(object)
          resolve()
        },
        undefined,
        reject,
      )
    })

    if (graph) {
      Promise.all(graph.nodes.map(loadNode))
        .then(() => {
          if (disposed) return
          const focusObjects = (
            qualityHighlightNodeIds.length
              ? qualityHighlightNodeIds
              : focusNodeId
              ? [focusNodeId]
              : []
          )
            .map((nodeId) => moduleRoot.getObjectByName(nodeId))
            .filter((item): item is THREE.Object3D => Boolean(item))
          let bounds = focusObjects.length
            ? focusObjects.reduce(
                (combined, item) => combined.union(new THREE.Box3().setFromObject(item)),
                new THREE.Box3(),
              )
            : new THREE.Box3().setFromObject(moduleRoot)
          if (bounds.isEmpty() && focusObjects.length) bounds = new THREE.Box3().setFromObject(moduleRoot)
          if (bounds.isEmpty()) {
            setLoadState('failed')
            setLoadMessage('ModuleGraph 已加载，但 GLB 中没有可显示网格')
            frameCamera(camera, controls, cameraView, new THREE.Vector3(), 240)
            return
          }
          const center = bounds.getCenter(new THREE.Vector3())
          const size = bounds.getSize(new THREE.Vector3())
          frameCamera(camera, controls, cameraView, center, Math.max(size.length(), 1))
          setLoadState('ready')
          setLoadMessage(`已加载 ${graph.nodes.length} 个真实 GLB 节点`)
        })
        .catch((caught) => {
          if (disposed) return
          setLoadState('failed')
          setLoadMessage(caught instanceof Error ? caught.message : String(caught))
          frameCamera(camera, controls, cameraView, new THREE.Vector3(), 240)
        })
    } else {
      frameCamera(camera, controls, cameraView, new THREE.Vector3(), 240)
    }

    const raycaster = new THREE.Raycaster()
    const pointer = new THREE.Vector2()
    const nodeAtClientPoint = (clientX: number, clientY: number) => {
      const rect = renderer.domElement.getBoundingClientRect()
      pointer.x = ((clientX - rect.left) / Math.max(rect.width, 1)) * 2 - 1
      pointer.y = -((clientY - rect.top) / Math.max(rect.height, 1)) * 2 + 1
      raycaster.setFromCamera(pointer, camera)
      const hit = raycaster.intersectObjects(moduleRoot.children, true)[0]
      const nodeId = hit?.object.userData.nodeId
      return typeof nodeId === 'string' ? nodeId : null
    }
    const selectAtPointer = (event: PointerEvent) => {
      const nodeId = nodeAtClientPoint(event.clientX, event.clientY)
      if (nodeId) onSelectNode(nodeId)
    }
    const allowModuleDrop = (event: DragEvent) => {
      if (event.dataTransfer?.types.includes('application/x-forgecad-module-id')) {
        event.preventDefault()
        if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy'
      }
    }
    const dropModule = (event: DragEvent) => {
      const moduleId = event.dataTransfer?.getData('application/x-forgecad-module-id')
        || event.dataTransfer?.getData('text/plain')
      if (!moduleId) return
      event.preventDefault()
      const nodeId = nodeAtClientPoint(event.clientX, event.clientY) ?? selectedNodeId
      if (nodeId) onDropModule(nodeId, moduleId)
    }
    renderer.domElement.addEventListener('pointerdown', selectAtPointer)
    renderer.domElement.addEventListener('dragover', allowModuleDrop)
    renderer.domElement.addEventListener('drop', dropModule)

    const resize = () => {
      const width = host.clientWidth
      const height = host.clientHeight
      renderer.setSize(width, height, false)
      camera.aspect = width / Math.max(height, 1)
      camera.updateProjectionMatrix()
    }
    const observer = new ResizeObserver(resize)
    observer.observe(host)
    resize()

    let animationFrame = 0
    const render = () => {
      controls.update()
      renderer.render(scene, camera)
      host.dataset.rendererGeometries = String(renderer.info.memory.geometries)
      host.dataset.rendererTextures = String(renderer.info.memory.textures)
      animationFrame = requestAnimationFrame(render)
    }
    render()

    return () => {
      disposed = true
      cancelAnimationFrame(animationFrame)
      observer.disconnect()
      renderer.domElement.removeEventListener('pointerdown', selectAtPointer)
      renderer.domElement.removeEventListener('dragover', allowModuleDrop)
      renderer.domElement.removeEventListener('drop', dropModule)
      controls.dispose()
      disposeObject(scene)
      renderer.dispose()
      renderer.forceContextLoss()
      activeViewportContexts = Math.max(0, activeViewportContexts - 1)
      host.dataset.activeWebglContexts = String(activeViewportContexts)
      renderer.domElement.remove()
    }
  }, [
    cameraView,
    explodeFactor,
    focusNodeId,
    getModuleFileUrl,
    graphRecord,
    hiddenNodeIds,
    modules,
    onSelectNode,
    onDropModule,
    selectedNodeId,
    qualityGeometryRefs,
    qualityHighlightNodeIds,
    showConnectors,
    showGrid,
    wireframe,
  ])

  return (
    <div className="weapon-viewport-shell">
      <div
        className="weapon-viewport"
        ref={hostRef}
        aria-label="真实 ModuleGraph 三维视口"
        data-load-state={loadState}
        data-focus-node-id={focusNodeId ?? ''}
        data-quality-node-ids={qualityHighlightNodeIds.join(',')}
        data-quality-triangle-count={qualityGeometryRefs.reduce(
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

function buildQualityOverlay(
  references: NonNullable<QualityFinding['geometry_refs']>,
): THREE.Group {
  const group = new THREE.Group()
  group.name = 'QualityGeometryOverlay'
  references.forEach((reference, referenceIndex) => {
    const positions: number[] = []
    for (const triangle of reference.world_triangles_mm ?? []) {
      if (triangle.length !== 3 || triangle.some((point) => point.length !== 3)) continue
      const [first, second, third] = triangle
      positions.push(
        ...first, ...second,
        ...second, ...third,
        ...third, ...first,
      )
    }
    if (!positions.length) return
    const geometry = new THREE.BufferGeometry()
    geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3))
    const material = new THREE.LineBasicMaterial({
      color: referenceIndex % 2 === 0 ? '#ff4d4d' : '#ffb347',
      depthTest: false,
      transparent: true,
      opacity: 0.98,
    })
    const lines = new THREE.LineSegments(geometry, material)
    lines.name = `QualityTriangles_${reference.node_id}`
    lines.renderOrder = 30
    group.add(lines)
  })
  return group
}

function frameCamera(
  camera: THREE.PerspectiveCamera,
  controls: OrbitControls,
  view: CameraView,
  center: THREE.Vector3,
  size: number,
) {
  const distance = Math.max(size * 1.45, 1)
  const direction: Record<CameraView, THREE.Vector3> = {
    iso: new THREE.Vector3(0.58, 0.48, 1),
    front: new THREE.Vector3(0, 0.08, 1),
    top: new THREE.Vector3(0, 1, 0.001),
    right: new THREE.Vector3(1, 0.08, 0),
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
      Object.values(material).forEach((value) => {
        if (value instanceof THREE.Texture) textures.add(value)
      })
      material.dispose()
    })
    if (object instanceof THREE.SkinnedMesh) object.skeleton.dispose()
  })
  textures.forEach((texture) => texture.dispose())
}
