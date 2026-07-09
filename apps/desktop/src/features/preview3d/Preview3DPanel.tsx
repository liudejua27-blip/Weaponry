import { useEffect, useMemo, useRef, useState, type PointerEvent } from 'react'
import * as THREE from 'three'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import type { ForgeApiClient } from '../../shared/api/forgeApi'
import type { CreateWeaponResponse, WeaponDetail, WeaponSummary } from '../../shared/types'

type Props = {
  api: ForgeApiClient
  refreshKey?: string | null
  activeWeaponId?: string
  activeVersionId?: string
  onWeaponSelected: (weaponId: string, versionId?: string) => void
  onVersionSelected: (versionId: string) => void
  onWeaponDetailLoaded: (detail: WeaponDetail) => void
  onJobCreated: (response: CreateWeaponResponse) => void
  onEventsReset: () => void
}

type PreviewMode = 'solid' | 'toon' | 'wireframe'
type VersionAsset = NonNullable<NonNullable<WeaponDetail['versions']>[number]['assets']>[number]
type WeaponVersion = NonNullable<WeaponDetail['versions']>[number]
type SourceImageSelection = {
  version: WeaponVersion
  asset: VersionAsset
  isFallback: boolean
}
type HandoffAsset = {
  version: WeaponVersion
  asset: VersionAsset
  isFallback: boolean
}
type UnityHandoffState = {
  activeVersion: WeaponVersion | null
  rawGlb: HandoffAsset | null
  normalizedGlb: HandoffAsset | null
  optimizedGlb: HandoffAsset | null
  unityMaterial: HandoffAsset | null
  qualityReport: HandoffAsset | null
  unityExport: HandoffAsset | null
  previewGlb: HandoffAsset | null
  modelId: string | null
  modelStatus: string | null
  qualityStatus: string | null
  qualityReportFileId: string | null
  qualitySummary: ModelQualitySummary | null
  transformPolicy: ModelTransformPolicy | null
}
type ModelQualitySummary = {
  triangleCount: number | null
  meshCount: number | null
  primitiveCount: number | null
  vertexCount: number | null
  materialCount: number | null
  textureCount: number | null
  imageCount: number | null
  longestAxis: number | null
  boundsValid: boolean | null
  hasPbrMaterial: boolean | null
  center: number[] | null
  extents: number[] | null
}
type ModelTransformPolicy = {
  forwardAxis: string | null
  longAxis: string | null
  pivot: string | null
  fallbackPivot: string | null
  scalePolicy: string | null
}

type PreviewStatus = 'loading' | 'ready' | 'empty' | 'error'

export function Preview3DPanel({
  api,
  refreshKey,
  activeWeaponId = '',
  activeVersionId = '',
  onWeaponSelected,
  onVersionSelected,
  onWeaponDetailLoaded,
  onJobCreated,
  onEventsReset,
}: Props) {
  const mountRef = useRef<HTMLDivElement | null>(null)
  const rendererRef = useRef<THREE.WebGLRenderer | null>(null)
  const sceneRef = useRef<THREE.Scene | null>(null)
  const cameraRef = useRef<THREE.PerspectiveCamera | null>(null)
  const displayRigRef = useRef<THREE.Group | null>(null)
  const weaponSocketRef = useRef<THREE.Group | null>(null)
  const modelRef = useRef<THREE.Object3D | null>(null)
  const animationRef = useRef<number | null>(null)
  const dragRef = useRef<{ active: boolean; lastX: number }>({ active: false, lastX: 0 })
  const originalMaterialsRef = useRef(new Map<THREE.Mesh, THREE.Material | THREE.Material[]>())
  const [weapons, setWeapons] = useState<WeaponSummary[]>([])
  const [selectedWeaponId, setSelectedWeaponId] = useState('')
  const [detail, setDetail] = useState<WeaponDetail | null>(null)
  const [status, setStatus] = useState<PreviewStatus>('loading')
  const [mode, setMode] = useState<PreviewMode>('toon')
  const [error, setError] = useState<string | null>(null)
  const [loadedAssetId, setLoadedAssetId] = useState<string | null>(null)
  const [isGenerating3D, setIsGenerating3D] = useState(false)
  const [isExportingUnity, setIsExportingUnity] = useState(false)
  const [pending3DMessage, setPending3DMessage] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    setStatus('loading')
    setError(null)
    api.listWeapons()
      .then((items) => {
        if (cancelled) return
        setWeapons(items)
        const fallback = items.find((item) => item.current_model_id) ?? items[0]
        setSelectedWeaponId((current) => activeWeaponId || current || fallback?.weapon_id || '')
        if (!activeWeaponId && fallback) onWeaponSelected(fallback.weapon_id, fallback.current_version_id ?? undefined)
        setStatus(items.length ? 'ready' : 'empty')
      })
      .catch((caught) => {
        if (cancelled) return
        setError(caught instanceof Error ? caught.message : '3D 资产列表加载失败')
        setStatus('error')
      })
    return () => {
      cancelled = true
    }
  }, [api, refreshKey])

  useEffect(() => {
    if (activeWeaponId && activeWeaponId !== selectedWeaponId) setSelectedWeaponId(activeWeaponId)
  }, [activeWeaponId, selectedWeaponId])

  useEffect(() => {
    if (!selectedWeaponId) {
      setDetail(null)
      return
    }
    let cancelled = false
    setError(null)
    api.getWeapon(selectedWeaponId)
      .then((nextDetail) => {
        if (cancelled) return
        setDetail(nextDetail)
        onWeaponDetailLoaded(nextDetail)
        if (!activeVersionId && nextDetail.current_version_id) onVersionSelected(nextDetail.current_version_id)
      })
      .catch((caught) => {
        if (cancelled) return
        setError(caught instanceof Error ? caught.message : '3D 资产详情加载失败')
        setStatus('error')
      })
    return () => {
      cancelled = true
    }
  }, [api, refreshKey, selectedWeaponId])

  const handoff = useMemo(() => buildUnityHandoffState(detail, activeVersionId), [detail, activeVersionId])
  const glbAsset = handoff.previewGlb?.asset ?? null
  const unityMaterialAsset = handoff.unityMaterial?.asset ?? null
  const unityExportAsset = handoff.unityExport?.asset ?? null
  const sourceImage = useMemo(() => findCurrentSourceImage(detail, activeVersionId), [detail, activeVersionId])

  useEffect(() => {
    const mount = mountRef.current
    if (!mount) return

    const scene = new THREE.Scene()
    scene.background = new THREE.Color(0xf7f2e8)
    const camera = new THREE.PerspectiveCamera(36, 1, 0.01, 100)
    camera.position.set(1.9, 1.15, 3.1)
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false, preserveDrawingBuffer: true })
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
    renderer.outputColorSpace = THREE.SRGBColorSpace
    mount.appendChild(renderer.domElement)

    const keyLight = new THREE.DirectionalLight(0xfff3df, 2.35)
    keyLight.position.set(3, 4, 5)
    scene.add(keyLight)
    scene.add(new THREE.HemisphereLight(0xfff0d0, 0x302820, 1.35))
    const fillLight = new THREE.DirectionalLight(0x7da2ff, 0.8)
    fillLight.position.set(-3, 2, -2)
    scene.add(fillLight)
    const grid = new THREE.GridHelper(3.4, 14, 0xc8b58d, 0xe0d3bc)
    grid.position.y = -0.9
    scene.add(grid)
    const { root, weaponSocket } = createCharacterDisplayRig()
    scene.add(root)
    displayRigRef.current = root
    weaponSocketRef.current = weaponSocket

    sceneRef.current = scene
    cameraRef.current = camera
    rendererRef.current = renderer

    const resize = () => {
      const rect = mount.getBoundingClientRect()
      const width = Math.max(1, Math.floor(rect.width))
      const height = Math.max(1, Math.floor(rect.height))
      camera.aspect = width / height
      camera.updateProjectionMatrix()
      renderer.setSize(width, height, false)
    }
    resize()
    const observer = new ResizeObserver(resize)
    observer.observe(mount)

    const animate = () => {
      if (displayRigRef.current && !dragRef.current.active) displayRigRef.current.rotation.y += 0.0055
      renderer.render(scene, camera)
      animationRef.current = requestAnimationFrame(animate)
    }
    animate()

    return () => {
      observer.disconnect()
      if (animationRef.current) cancelAnimationFrame(animationRef.current)
      disposeCurrentModel()
      if (displayRigRef.current) {
        scene.remove(displayRigRef.current)
        disposeObject(displayRigRef.current)
      }
      renderer.dispose()
      renderer.forceContextLoss()
      renderer.domElement.remove()
      sceneRef.current = null
      cameraRef.current = null
      rendererRef.current = null
      displayRigRef.current = null
      weaponSocketRef.current = null
    }
  }, [])

  useEffect(() => {
    if (!glbAsset || !sceneRef.current || !cameraRef.current) {
      disposeCurrentModel()
      setLoadedAssetId(null)
      return
    }
    let cancelled = false
    const loader = new GLTFLoader()
    setError(null)
    setStatus('loading')
    loader.load(
      api.getAssetFileUrl(glbAsset.asset_id),
      (gltf) => {
        if (cancelled) {
          disposeObject(gltf.scene)
          return
        }
        disposeCurrentModel()
        const weaponGroup = prepareWeaponForCharacter(gltf.scene)
        modelRef.current = weaponGroup
        originalMaterialsRef.current.clear()
        weaponSocketRef.current?.add(weaponGroup)
        prepareModel(weaponGroup)
        applyPreviewMode(mode)
        resetCamera()
        setLoadedAssetId(glbAsset.asset_id)
        setStatus('ready')
      },
      undefined,
      (caught) => {
        if (cancelled) return
        setError(caught instanceof Error ? caught.message : 'GLB 加载失败')
        setStatus('error')
      }
    )
    return () => {
      cancelled = true
    }
  }, [api, glbAsset?.asset_id])

  useEffect(() => {
    applyPreviewMode(mode)
  }, [mode])

  function captureScreenshot() {
    const renderer = rendererRef.current
    if (!renderer) return
    const link = document.createElement('a')
    link.download = `${loadedAssetId || 'wushen-preview'}.png`
    link.href = renderer.domElement.toDataURL('image/png')
    link.click()
  }

  async function generateRough3D() {
    if (!detail || !sourceImage) {
      setError('需要先选择一个带概念图或 patch 图的武器版本')
      return
    }

    const now = Date.now()
    setIsGenerating3D(true)
    setPending3DMessage(null)
    setError(null)
    try {
      onEventsReset()
      const response = await api.generateRough3D(detail.weapon_id, {
        client_request_id: `desktop_generate3d_${now}`,
        source_version_id: sourceImage.version.version_id,
        source_image_asset_id: sourceImage.asset.asset_id,
        provider_id: 'mock_3d',
        target_format: 'glb',
        style: 'stylized_toon_weapon',
        orientation_policy: {
          forward_axis: '+Z',
          long_axis: '+Y',
          pivot: 'grip_center',
        },
        scale_policy: 'normalized_game_asset_scale',
        build_unity_export: true,
      })
      onJobCreated(response)
      if (!isAcceptedTerminal(response.status)) {
        setPending3DMessage('已提交 3D 任务，可离开此面板；完成后会写入当前武器版本。')
        return
      }
      const nextDetail = await api.getWeapon(detail.weapon_id)
      setDetail(nextDetail)
      onWeaponDetailLoaded(nextDetail)
      if (nextDetail.current_version_id) onVersionSelected(nextDetail.current_version_id)
      setPending3DMessage('后台完成，已生成 rough GLB。')
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : '3D 粗模生成失败')
    } finally {
      setIsGenerating3D(false)
    }
  }

  async function exportUnityPackage() {
    if (!detail?.current_model_id) {
      setError('需要先生成一个 3D 粗模，才能导出 Unity 包')
      return
    }

    const now = Date.now()
    setIsExportingUnity(true)
    setError(null)
    try {
      onEventsReset()
      const response = await api.exportUnityPackage(detail.weapon_id, {
        client_request_id: `desktop_export_unity_${now}`,
        model_id: detail.current_model_id,
        export_type: 'unity_glb',
        include_source_spec: true,
        include_quality_reports: true,
      })
      onJobCreated(response)
      const nextDetail = await api.getWeapon(detail.weapon_id)
      setDetail(nextDetail)
      onWeaponDetailLoaded(nextDetail)
      if (nextDetail.current_version_id) onVersionSelected(nextDetail.current_version_id)
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'Unity 导出包生成失败')
    } finally {
      setIsExportingUnity(false)
    }
  }

  return (
    <section className="preview-card preview3d-card">
      <div className="preview3d-header">
        <div>
          <h3>3D 展台预览</h3>
          <p className="muted">角色持武器站在展览台上，可拖拽 360 度查看。</p>
        </div>
        <span className={`preview3d-status ${status}`}>{status}</span>
      </div>

      <label className="preview3d-field">
        武器
        <select
          value={selectedWeaponId}
          onChange={(event) => {
            setSelectedWeaponId(event.target.value)
            const selected = weapons.find((weapon) => weapon.weapon_id === event.target.value)
            onWeaponSelected(event.target.value, selected?.current_version_id ?? undefined)
          }}
        >
          {weapons.map((weapon) => (
            <option key={weapon.weapon_id} value={weapon.weapon_id}>
              {weapon.display_name}
            </option>
          ))}
        </select>
      </label>

      <div className="preview3d-toolbar" aria-label="3D 预览工具">
        <button className={mode === 'toon' ? 'active' : ''} onClick={() => setMode('toon')}>toon</button>
        <button className={mode === 'solid' ? 'active' : ''} onClick={() => setMode('solid')}>solid</button>
        <button className={mode === 'wireframe' ? 'active' : ''} onClick={() => setMode('wireframe')}>wire</button>
        <button onClick={resetCamera} disabled={!loadedAssetId}>reset</button>
        <button onClick={captureScreenshot} disabled={!loadedAssetId}>shot</button>
      </div>

      <div className="preview3d-action-row">
        <button className="primary" onClick={generateRough3D} disabled={!sourceImage || isGenerating3D}>
          {isGenerating3D ? '生成中...' : '从当前图生成 3D'}
        </button>
        <span className="muted">
          {pending3DMessage
            ? pending3DMessage
            : sourceImage
            ? `${sourceImage.asset.role}${sourceImage.isFallback ? ' · 使用最近图像' : ''}`
            : '需要概念图或 patch 图'}
        </span>
      </div>
      <div className="preview3d-action-row">
        <button className="primary" onClick={exportUnityPackage} disabled={!detail?.current_model_id || isExportingUnity}>
          {isExportingUnity ? '导出中...' : '导出 Unity 包'}
        </button>
        <span className="muted">
          {unityExportAsset ? '已生成 Unity ZIP 快照' : detail?.current_model_id ? '包含 GLB / Material / Spec / Report' : '需要 3D 粗模'}
        </span>
      </div>

      <div
        className="preview3d-frame"
        ref={mountRef}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerCancel={handlePointerUp}
      >
        {!glbAsset && status !== 'loading' && <span className="preview3d-overlay">暂无 rough GLB，先展示角色展台</span>}
      </div>

      {glbAsset && (
        <div className="preview3d-meta">
          <span>GLB</span>
          <code>{glbAsset.asset_id}</code>
          <span>{formatBytes(glbAsset.byte_size)}</span>
        </div>
      )}
      {unityMaterialAsset && (
        <div className="preview3d-meta">
          <span>Unity Material</span>
          <code>{unityMaterialAsset.asset_id}</code>
        </div>
      )}
      {unityExportAsset && (
        <div className="preview3d-meta">
          <span>Unity Export ZIP</span>
          <code>{unityExportAsset.asset_id}</code>
          <a href={api.getAssetFileUrl(unityExportAsset.asset_id)} download>download</a>
        </div>
      )}
      <UnityHandoffCard api={api} handoff={handoff} />
      {error && <p className="error">{error}</p>}
    </section>
  )

  function disposeCurrentModel() {
    const socket = weaponSocketRef.current
    const model = modelRef.current
    if (socket && model) socket.remove(model)
    if (model) disposeObject(model)
    modelRef.current = null
    originalMaterialsRef.current.clear()
  }

  function resetCamera() {
    const camera = cameraRef.current
    const target = displayRigRef.current ?? modelRef.current
    if (!target || !camera) return
    const box = new THREE.Box3().setFromObject(target)
    const center = new THREE.Vector3()
    const size = new THREE.Vector3()
    box.getCenter(center)
    box.getSize(size)
    const radius = Math.max(size.x, size.y, size.z, 1.2)
    camera.position.set(center.x + radius * 1.05, center.y + radius * 0.6, center.z + radius * 1.95)
    camera.lookAt(center.x, center.y + 0.12, center.z)
    camera.near = Math.max(0.01, radius / 100)
    camera.far = Math.max(20, radius * 8)
    camera.updateProjectionMatrix()
  }

  function handlePointerDown(event: PointerEvent<HTMLDivElement>) {
    dragRef.current = { active: true, lastX: event.clientX }
    event.currentTarget.setPointerCapture(event.pointerId)
  }

  function handlePointerMove(event: PointerEvent<HTMLDivElement>) {
    if (!dragRef.current.active || !displayRigRef.current) return
    const delta = event.clientX - dragRef.current.lastX
    displayRigRef.current.rotation.y += delta * 0.01
    dragRef.current.lastX = event.clientX
  }

  function handlePointerUp(event: PointerEvent<HTMLDivElement>) {
    dragRef.current.active = false
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
  }

  function applyPreviewMode(nextMode: PreviewMode) {
    const model = modelRef.current
    if (!model) return
    model.traverse((object) => {
      if (!(object instanceof THREE.Mesh)) return
      if (!originalMaterialsRef.current.has(object)) {
        originalMaterialsRef.current.set(object, object.material)
      }
      if (nextMode === 'wireframe') {
        object.material = new THREE.MeshBasicMaterial({ color: 0x2a2520, wireframe: true })
      } else if (nextMode === 'toon') {
        object.material = new THREE.MeshToonMaterial({ color: 0xc64a32 })
      } else {
        object.material = cloneMaterial(originalMaterialsRef.current.get(object))
      }
    })
  }
}

function findCurrentSourceImage(detail: WeaponDetail | null, preferredVersionId?: string): SourceImageSelection | null {
  if (!detail?.versions?.length) return null
  const current = detail.versions.find((version) => version.version_id === preferredVersionId)
    ?? detail.versions.find((version) => version.version_id === detail.current_version_id)
    ?? detail.versions.at(-1)
  const currentAsset = findConceptOrPatchAsset(current)
  if (current && currentAsset) return { version: current, asset: currentAsset, isFallback: false }

  for (const version of [...detail.versions].reverse()) {
    const asset = findConceptOrPatchAsset(version)
    if (asset) return { version, asset, isFallback: true }
  }
  return null
}

function findConceptOrPatchAsset(version: WeaponVersion | undefined): VersionAsset | null {
  const assets = version?.assets ?? []
  return assets.find((asset) => asset.role === 'concept_patch')
    ?? assets.find((asset) => asset.role === 'concept_image')
    ?? null
}

function buildUnityHandoffState(detail: WeaponDetail | null, preferredVersionId?: string): UnityHandoffState {
  const activeVersion = findActiveVersion(detail, preferredVersionId)
  const rawGlb = findHandoffAsset(detail, activeVersion, 'rough_raw_glb', 'model/gltf-binary')
  const normalizedGlb = findHandoffAsset(detail, activeVersion, 'rough_normalized_glb', 'model/gltf-binary')
  const optimizedGlb = findHandoffAsset(detail, activeVersion, 'rough_optimized_glb', 'model/gltf-binary')
  const unityMaterial = findHandoffAsset(detail, activeVersion, 'unity_material_json')
  const qualityReportAsset = findHandoffAsset(detail, activeVersion, 'quality_report')
  const unityExport = findHandoffAsset(detail, activeVersion, 'unity_export_package', 'application/zip')
  const currentModel = isRecord(detail?.current_model) ? detail.current_model : {}
  const orientationPolicy = isRecord(currentModel.orientation_policy) ? currentModel.orientation_policy : {}
  const qualityReport = isRecord(currentModel.quality_report) ? currentModel.quality_report : {}
  const qualityMetrics = isRecord(qualityReport.metrics) ? qualityReport.metrics : {}
  return {
    activeVersion,
    rawGlb,
    normalizedGlb,
    optimizedGlb,
    unityMaterial,
    qualityReport: qualityReportAsset,
    unityExport,
    previewGlb: optimizedGlb ?? normalizedGlb ?? rawGlb,
    modelId: stringOrNull(currentModel.model_id) ?? detail?.current_model_id ?? null,
    modelStatus: stringOrNull(currentModel.status),
    qualityStatus: stringOrNull(qualityReport.status),
    qualityReportFileId: stringOrNull(qualityReport.quality_report_file_id),
    qualitySummary: buildModelQualitySummary(qualityMetrics),
    transformPolicy: buildModelTransformPolicy(orientationPolicy),
  }
}

function findActiveVersion(detail: WeaponDetail | null, preferredVersionId?: string): WeaponVersion | null {
  if (!detail?.versions?.length) return null
  return detail.versions.find((version) => version.version_id === preferredVersionId)
    ?? detail.versions.find((version) => version.version_id === detail.current_version_id)
    ?? detail.versions.at(-1)
    ?? null
}

function findHandoffAsset(detail: WeaponDetail | null, activeVersion: WeaponVersion | null, role: string, mimeType?: string): HandoffAsset | null {
  const matches = (asset: VersionAsset) => asset.role === role && (!mimeType || asset.mime_type === mimeType)
  const activeAsset = (activeVersion?.assets ?? []).find(matches)
  if (activeVersion && activeAsset) return { version: activeVersion, asset: activeAsset, isFallback: false }
  for (const version of [...(detail?.versions ?? [])].reverse()) {
    const asset = (version.assets ?? []).find(matches)
    if (asset) return { version, asset, isFallback: true }
  }
  return null
}

function UnityHandoffCard({ api, handoff }: { api: ForgeApiClient; handoff: UnityHandoffState }) {
  const exportIsFallback = Boolean(handoff.unityExport?.isFallback)
  const hasExportInputs = Boolean(handoff.optimizedGlb && handoff.unityMaterial && handoff.qualityReport)
  return (
    <div className="unity-handoff-card">
      <header>
        <div>
          <strong>Unity 交接状态</strong>
          <small>当前只判断游戏资产交接完整度，不代表现实制造说明。</small>
        </div>
        <span className={handoff.unityExport && !exportIsFallback ? 'ready' : hasExportInputs ? 'warning' : 'missing'}>
          {handoff.unityExport && !exportIsFallback ? 'ZIP ready' : hasExportInputs ? '可导出' : '缺资产'}
        </span>
      </header>
      <div className="handoff-grid">
        <HandoffAssetRow label="Raw GLB" item={handoff.rawGlb} api={api} />
        <HandoffAssetRow label="Normalized GLB" item={handoff.normalizedGlb} api={api} />
        <HandoffAssetRow label="Optimized GLB" item={handoff.optimizedGlb} api={api} />
        <HandoffAssetRow label="Unity Material" item={handoff.unityMaterial} api={api} />
        <HandoffAssetRow label="Quality Report" item={handoff.qualityReport} api={api} extra={handoff.qualityStatus ?? handoff.qualityReportFileId ?? undefined} />
        <HandoffAssetRow label="Export ZIP" item={handoff.unityExport} api={api} />
      </div>
      <ModelQualityPanel summary={handoff.qualitySummary} />
      <ModelTransformPanel policy={handoff.transformPolicy} />
      <div className="handoff-warnings">
        <span>model {handoff.modelId ?? 'unknown'} · {handoff.modelStatus ?? 'unknown status'}</span>
        {handoff.previewGlb?.isFallback && <span>预览使用最近可用 GLB：v{handoff.previewGlb.version.version_no}</span>}
        {exportIsFallback && <span>ZIP 来自其他版本，当前模型可能需要重新导出。</span>}
        {!handoff.optimizedGlb && <span>缺少 optimized GLB，不能标记为 Unity 交接 ready。</span>}
      </div>
    </div>
  )
}

function ModelTransformPanel({ policy }: { policy: ModelTransformPolicy | null }) {
  if (!policy) {
    return (
      <div className="model-transform-panel empty">
        <strong>Unity 轴向 / 尺度</strong>
        <span>暂无 orientation policy</span>
      </div>
    )
  }
  return (
    <div className="model-transform-panel">
      <strong>Unity 轴向 / 尺度</strong>
      <div className="model-transform-grid">
        <MetricCell label="Forward" value={policy.forwardAxis ?? 'missing'} />
        <MetricCell label="Long Axis" value={policy.longAxis ?? 'missing'} />
        <MetricCell label="Pivot" value={policy.pivot ?? 'missing'} />
        <MetricCell label="Scale" value={policy.scalePolicy ?? 'missing'} />
      </div>
      <small>fallback pivot {policy.fallbackPivot ?? 'bounding_box_center'} · game asset relative scale only</small>
    </div>
  )
}

function ModelQualityPanel({ summary }: { summary: ModelQualitySummary | null }) {
  if (!summary) {
    return (
      <div className="model-quality-panel empty">
        <strong>模型质量证据</strong>
        <span>暂无 parsed GLB metrics</span>
      </div>
    )
  }
  return (
    <div className="model-quality-panel">
      <div className="model-quality-header">
        <strong>模型质量证据</strong>
        <span className={summary.boundsValid === false ? 'missing' : 'ready'}>
          {summary.boundsValid === false ? 'Bounds invalid' : 'Bounds ready'}
        </span>
      </div>
      <div className="model-quality-grid">
        <MetricCell label="Triangles" value={formatMetric(summary.triangleCount)} />
        <MetricCell label="Meshes" value={formatMetric(summary.meshCount)} />
        <MetricCell label="Vertices" value={formatMetric(summary.vertexCount)} />
        <MetricCell label="Materials" value={formatMetric(summary.materialCount)} />
        <MetricCell label="Textures" value={formatMetric(summary.textureCount)} />
        <MetricCell label="Longest Axis" value={summary.longestAxis === null ? 'missing' : trimNumber(summary.longestAxis)} />
      </div>
      <small>
        primitives {formatMetric(summary.primitiveCount)} · images {formatMetric(summary.imageCount)} · PBR {summary.hasPbrMaterial ? 'yes' : 'no'} · center {formatVector(summary.center)} · extents {formatVector(summary.extents)}
      </small>
    </div>
  )
}

function MetricCell({ label, value }: { label: string; value: string }) {
  return (
    <span>
      <small>{label}</small>
      <strong>{value}</strong>
    </span>
  )
}

function HandoffAssetRow({ label, item, api, extra }: { label: string; item: HandoffAsset | null; api: ForgeApiClient; extra?: string }) {
  return (
    <div className={`handoff-row ${item ? item.isFallback ? 'fallback' : 'ready' : 'missing'}`}>
      <span>{label}</span>
      {item ? (
        <>
          <code>{item.asset.asset_id}</code>
          <small>v{item.version.version_no} · {formatBytes(item.asset.byte_size)}{item.isFallback ? ' · 最近可用' : ''}{extra ? ` · ${extra}` : ''}</small>
          <a href={api.getAssetFileUrl(item.asset.asset_id)} download>file</a>
        </>
      ) : (
        <small>missing</small>
      )}
    </div>
  )
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function stringOrNull(value: unknown): string | null {
  return typeof value === 'string' && value.length ? value : null
}

function numberOrNull(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value
  if (typeof value === 'string' && value.trim()) {
    const parsed = Number(value)
    if (Number.isFinite(parsed)) return parsed
  }
  return null
}

function booleanOrNull(value: unknown): boolean | null {
  return typeof value === 'boolean' ? value : null
}

function numberArrayOrNull(value: unknown): number[] | null {
  if (!Array.isArray(value)) return null
  const items = value.map(numberOrNull)
  return items.every((item): item is number => item !== null) ? items : null
}

function buildModelQualitySummary(metrics: Record<string, unknown>): ModelQualitySummary | null {
  if (!Object.keys(metrics).length) return null
  return {
    triangleCount: numberOrNull(metrics.triangle_count),
    meshCount: numberOrNull(metrics.mesh_count),
    primitiveCount: numberOrNull(metrics.primitive_count),
    vertexCount: numberOrNull(metrics.vertex_count),
    materialCount: numberOrNull(metrics.material_count),
    textureCount: numberOrNull(metrics.texture_count),
    imageCount: numberOrNull(metrics.image_count),
    longestAxis: numberOrNull(metrics.longest_axis),
    boundsValid: booleanOrNull(metrics.bounds_valid),
    hasPbrMaterial: booleanOrNull(metrics.has_pbr_material),
    center: numberArrayOrNull(metrics.center),
    extents: numberArrayOrNull(metrics.extents),
  }
}

function buildModelTransformPolicy(policy: Record<string, unknown>): ModelTransformPolicy | null {
  if (!Object.keys(policy).length) return null
  return {
    forwardAxis: stringOrNull(policy.forward_axis),
    longAxis: stringOrNull(policy.long_axis),
    pivot: stringOrNull(policy.pivot),
    fallbackPivot: stringOrNull(policy.fallback_pivot),
    scalePolicy: stringOrNull(policy.scale_policy),
  }
}

function formatMetric(value: number | null) {
  return value === null ? 'missing' : Math.round(value).toLocaleString('en-US')
}

function trimNumber(value: number) {
  return Number.isInteger(value) ? String(value) : value.toFixed(3).replace(/\.?0+$/, '')
}

function formatVector(values: number[] | null) {
  if (!values?.length) return 'missing'
  return values.map(trimNumber).join(', ')
}

function isAcceptedTerminal(status: CreateWeaponResponse['status']) {
  return ['succeeded', 'failed', 'cancelled', 'partial_succeeded'].includes(status)
}

function createCharacterDisplayRig(): { root: THREE.Group; weaponSocket: THREE.Group } {
  const root = new THREE.Group()
  root.name = 'WushenDisplayRig'

  const pedestal = new THREE.Mesh(
    new THREE.CylinderGeometry(1.02, 1.16, 0.22, 48),
    new THREE.MeshStandardMaterial({ color: 0x786a58, roughness: 0.68, metalness: 0.08 })
  )
  pedestal.position.y = -0.82
  root.add(pedestal)

  const topPlate = new THREE.Mesh(
    new THREE.CylinderGeometry(0.92, 0.92, 0.045, 48),
    new THREE.MeshToonMaterial({ color: 0xd7c3a2 })
  )
  topPlate.position.y = -0.68
  root.add(topPlate)

  const bodyMaterial = new THREE.MeshToonMaterial({ color: 0x8db55a })
  const bellyMaterial = new THREE.MeshToonMaterial({ color: 0xf2d9aa })
  const accentMaterial = new THREE.MeshToonMaterial({ color: 0x47583f })
  const eyeMaterial = new THREE.MeshBasicMaterial({ color: 0x151515 })

  const body = new THREE.Mesh(new THREE.SphereGeometry(0.32, 24, 18), bodyMaterial)
  body.scale.set(0.92, 1.24, 0.72)
  body.position.set(0, -0.18, 0)
  root.add(body)

  const belly = new THREE.Mesh(new THREE.SphereGeometry(0.2, 18, 12), bellyMaterial)
  belly.scale.set(1, 1.18, 0.22)
  belly.position.set(0, -0.18, 0.215)
  root.add(belly)

  const head = new THREE.Mesh(new THREE.SphereGeometry(0.26, 24, 18), bodyMaterial)
  head.scale.set(1.08, 0.92, 0.88)
  head.position.set(0, 0.29, 0.02)
  root.add(head)

  const leftEye = new THREE.Mesh(new THREE.SphereGeometry(0.035, 12, 8), eyeMaterial)
  leftEye.position.set(-0.095, 0.34, 0.235)
  root.add(leftEye)
  const rightEye = leftEye.clone()
  rightEye.position.x = 0.095
  root.add(rightEye)

  const leftLeg = roundedLimb(0.07, 0.2, bodyMaterial)
  leftLeg.position.set(-0.14, -0.62, 0.03)
  leftLeg.rotation.z = -0.16
  root.add(leftLeg)
  const rightLeg = roundedLimb(0.07, 0.2, bodyMaterial)
  rightLeg.position.set(0.14, -0.62, 0.03)
  rightLeg.rotation.z = 0.16
  root.add(rightLeg)

  const leftArm = roundedLimb(0.052, 0.42, bodyMaterial)
  leftArm.position.set(-0.36, -0.16, 0.08)
  leftArm.rotation.z = -0.95
  leftArm.rotation.x = -0.12
  root.add(leftArm)

  const rightUpperArm = roundedLimb(0.052, 0.32, bodyMaterial)
  rightUpperArm.position.set(0.29, -0.05, 0.08)
  rightUpperArm.rotation.z = -0.58
  rightUpperArm.rotation.x = -0.18
  root.add(rightUpperArm)

  const rightForearm = roundedLimb(0.046, 0.34, bodyMaterial)
  rightForearm.position.set(0.45, 0.08, 0.12)
  rightForearm.rotation.z = -0.98
  rightForearm.rotation.x = -0.2
  root.add(rightForearm)

  const hand = new THREE.Mesh(new THREE.SphereGeometry(0.075, 16, 10), accentMaterial)
  hand.position.set(0.56, 0.2, 0.15)
  root.add(hand)

  const weaponSocket = new THREE.Group()
  weaponSocket.name = 'WeaponGripSocket'
  weaponSocket.position.set(0.56, 0.2, 0.15)
  weaponSocket.rotation.set(0.16, -0.12, -0.72)
  root.add(weaponSocket)

  return { root, weaponSocket }
}

function roundedLimb(radius: number, length: number, material: THREE.Material): THREE.Group {
  const group = new THREE.Group()
  const cylinder = new THREE.Mesh(new THREE.CylinderGeometry(radius, radius, length, 16), material)
  cylinder.rotation.z = Math.PI / 2
  group.add(cylinder)
  const start = new THREE.Mesh(new THREE.SphereGeometry(radius, 16, 8), material)
  start.position.x = -length / 2
  group.add(start)
  const end = start.clone()
  end.position.x = length / 2
  group.add(end)
  return group
}

function prepareWeaponForCharacter(model: THREE.Object3D): THREE.Group {
  const group = new THREE.Group()
  const box = new THREE.Box3().setFromObject(model)
  const center = new THREE.Vector3()
  const size = new THREE.Vector3()
  box.getCenter(center)
  box.getSize(size)
  const longAxis = Math.max(size.x, size.y, size.z, 0.001)
  const scale = 1.18 / longAxis
  model.position.sub(center)
  group.add(model)
  group.scale.setScalar(scale)
  group.position.set(0.02, 0, 0)
  return group
}

function prepareModel(model: THREE.Object3D) {
  model.traverse((object) => {
    if (object instanceof THREE.Mesh) {
      object.castShadow = true
      object.receiveShadow = true
      if (object.geometry && !object.geometry.attributes.normal) object.geometry.computeVertexNormals()
    }
  })
}

function disposeObject(object: THREE.Object3D) {
  object.traverse((child) => {
    if (child instanceof THREE.Mesh) {
      child.geometry?.dispose()
      disposeMaterial(child.material)
    }
  })
}

function disposeMaterial(material: THREE.Material | THREE.Material[]) {
  if (Array.isArray(material)) {
    material.forEach(disposeMaterial)
    return
  }
  material.dispose()
}

function cloneMaterial(material: THREE.Material | THREE.Material[] | undefined): THREE.Material | THREE.Material[] {
  if (!material) return new THREE.MeshStandardMaterial({ color: 0xb84832, roughness: 0.58, metalness: 0.2 })
  if (Array.isArray(material)) return material.map((item) => item.clone())
  return material.clone()
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`
  return `${(value / 1024 / 1024).toFixed(1)} MB`
}
