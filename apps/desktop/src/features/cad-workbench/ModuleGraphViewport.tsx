import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import type { ModuleGraphRecord } from '../../shared/types'

type CameraView = 'iso' | 'front' | 'top' | 'right'

type ModuleGraphViewportProps = {
  graphRecord: ModuleGraphRecord | null
  cameraView: CameraView
  showGrid: boolean
  wireframe: boolean
  selectedNodeId: string
  getModuleFileUrl: (moduleId: string) => string
  onSelectNode: (nodeId: string) => void
}

export function ModuleGraphViewport({
  graphRecord,
  cameraView,
  showGrid,
  wireframe,
  selectedNodeId,
  getModuleFileUrl,
  onSelectNode,
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

    let disposed = false
    const loader = new GLTFLoader()
    const loadNode = (node: NonNullable<typeof graph>['nodes'][number]) => new Promise<void>((resolve, reject) => {
      loader.load(
        getModuleFileUrl(node.module_id),
        (gltf) => {
          if (disposed) {
            disposeObject(gltf.scene)
            resolve()
            return
          }
          const object = gltf.scene
          object.name = node.node_id
          object.userData.nodeId = node.node_id
          const [px = 0, py = 0, pz = 0] = node.transform.position
          const [rx = 0, ry = 0, rz = 0] = node.transform.rotation
          const [sx = 1, sy = 1, sz = 1] = node.transform.scale
          object.position.set(px, py, pz)
          object.rotation.set(rx, ry, rz)
          object.scale.set(sx, sy, sz)
          object.visible = node.visible !== false
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
                clone.emissive.set(node.node_id === selectedNodeId ? '#1f64a8' : '#000000')
                clone.emissiveIntensity = node.node_id === selectedNodeId ? 0.42 : 0
              }
              return clone
            })
            child.material = Array.isArray(child.material) ? materials : materials[0]
          })
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
          const bounds = new THREE.Box3().setFromObject(moduleRoot)
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
    const selectAtPointer = (event: PointerEvent) => {
      const rect = renderer.domElement.getBoundingClientRect()
      pointer.x = ((event.clientX - rect.left) / Math.max(rect.width, 1)) * 2 - 1
      pointer.y = -((event.clientY - rect.top) / Math.max(rect.height, 1)) * 2 + 1
      raycaster.setFromCamera(pointer, camera)
      const hit = raycaster.intersectObjects(moduleRoot.children, true)[0]
      const nodeId = hit?.object.userData.nodeId
      if (typeof nodeId === 'string') onSelectNode(nodeId)
    }
    renderer.domElement.addEventListener('pointerdown', selectAtPointer)

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
      animationFrame = requestAnimationFrame(render)
    }
    render()

    return () => {
      disposed = true
      cancelAnimationFrame(animationFrame)
      observer.disconnect()
      renderer.domElement.removeEventListener('pointerdown', selectAtPointer)
      controls.dispose()
      disposeObject(scene)
      renderer.dispose()
      renderer.domElement.remove()
    }
  }, [cameraView, getModuleFileUrl, graphRecord, onSelectNode, selectedNodeId, showGrid, wireframe])

  return (
    <div className="weapon-viewport-shell">
      <div className="weapon-viewport" ref={hostRef} aria-label="真实 ModuleGraph 三维视口" />
      {loadState !== 'ready' && (
        <div className={`viewport-data-state ${loadState}`} role="status">
          <strong>{loadState === 'loading' ? '加载 ModuleGraph' : loadState === 'failed' ? 'GLB 无法显示' : '等待模块组合'}</strong>
          <span>{loadMessage}</span>
        </div>
      )}
    </div>
  )
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
  root.traverse((object) => {
    if (!(object instanceof THREE.Mesh || object instanceof THREE.LineSegments)) return
    object.geometry?.dispose()
    const materials = Array.isArray(object.material) ? object.material : [object.material]
    materials.forEach((material) => material?.dispose())
  })
}
