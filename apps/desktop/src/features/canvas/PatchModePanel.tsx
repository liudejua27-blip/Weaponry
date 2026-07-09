import { type CSSProperties, type PointerEvent, useEffect, useMemo, useRef, useState } from 'react'
import type { ForgeApiClient } from '../../shared/api/forgeApi'
import type { AssetUploadResponse, CreateWeaponResponse, WeaponDetail, WeaponSummary } from '../../shared/types'

type Props = {
  api: ForgeApiClient
  activeWeaponId?: string
  activeVersionId?: string
  onWeaponSelected: (weaponId: string, versionId?: string) => void
  onVersionSelected: (versionId: string) => void
  onWeaponDetailLoaded: (detail: WeaponDetail) => void
  onJobCreated: (response: CreateWeaponResponse) => void
  onEventsReset: () => void
}

type PatchTarget = 'blade' | 'guard' | 'core' | 'rune' | 'glow' | 'material' | 'silhouette' | 'handle' | 'ornament' | 'whole_weapon'
type Strength = 'subtle' | 'medium' | 'strong'
type MaskTool = 'brush' | 'lasso'
type WeaponVersion = NonNullable<WeaponDetail['versions']>[number]
type VersionAsset = NonNullable<WeaponVersion['assets']>[number]

const targetOptions: Array<{ value: PatchTarget; label: string }> = [
  { value: 'blade', label: '刀身 / 剑身' },
  { value: 'guard', label: '护手' },
  { value: 'core', label: '宝石核心' },
  { value: 'rune', label: '纹样' },
  { value: 'glow', label: '光效' },
  { value: 'material', label: '材质' },
  { value: 'silhouette', label: '轮廓' },
  { value: 'handle', label: '握柄' },
  { value: 'ornament', label: '装饰件' },
  { value: 'whole_weapon', label: '整体' },
]

const preserveOptions = [
  { value: 'overall_silhouette', label: '整体剪影' },
  { value: 'chinese_motifs', label: '国风纹样' },
  { value: 'toon_outline', label: '3渲2描边' },
  { value: 'main_palette', label: '主色调' },
  { value: 'material_zones', label: '材质分区' },
  { value: 'weapon_identity', label: '武器识别度' },
]

export function PatchModePanel({
  api,
  activeWeaponId = '',
  activeVersionId = '',
  onWeaponSelected,
  onVersionSelected,
  onWeaponDetailLoaded,
  onJobCreated,
  onEventsReset,
}: Props) {
  const [weapons, setWeapons] = useState<WeaponSummary[]>([])
  const [selectedWeaponId, setSelectedWeaponId] = useState('')
  const [detail, setDetail] = useState<WeaponDetail | null>(null)
  const [selectedVersionId, setSelectedVersionId] = useState('')
  const [target, setTarget] = useState<PatchTarget>('core')
  const [instruction, setInstruction] = useState('把核心改成青蓝玉石雷纹能量核，保持高拟真国风神兵外观')
  const [preserve, setPreserve] = useState<string[]>(['overall_silhouette', 'chinese_motifs', 'toon_outline'])
  const [strength, setStrength] = useState<Strength>('medium')
  const [status, setStatus] = useState<'loading' | 'ready' | 'empty' | 'error' | 'submitting'>('loading')
  const [error, setError] = useState<string | null>(null)
  const [lastUpload, setLastUpload] = useState<AssetUploadResponse | null>(null)
  const [lastPatchJobId, setLastPatchJobId] = useState<string | null>(null)
  const [maskHasInk, setMaskHasInk] = useState(false)
  const [isDrawing, setIsDrawing] = useState(false)
  const [maskTool, setMaskTool] = useState<MaskTool>('brush')
  const [lassoPoints, setLassoPoints] = useState<Array<{ x: number; y: number }>>([])
  const [brushSize, setBrushSize] = useState(56)
  const [maskOpacity, setMaskOpacity] = useState(58)
  const [comparisonRatio, setComparisonRatio] = useState(50)
  const [undoDepth, setUndoDepth] = useState(0)
  const [redoDepth, setRedoDepth] = useState(0)
  const maskCanvasRef = useRef<HTMLCanvasElement | null>(null)
  const lastPointRef = useRef<{ x: number; y: number } | null>(null)
  const isDrawingRef = useRef(false)
  const lassoPointsRef = useRef<Array<{ x: number; y: number }>>([])
  const undoStackRef = useRef<ImageData[]>([])
  const redoStackRef = useRef<ImageData[]>([])

  useEffect(() => {
    let cancelled = false
    setStatus('loading')
    setError(null)
    api.listWeapons()
      .then((items) => {
        if (cancelled) return
        setWeapons(items)
        setSelectedWeaponId((current) => current || activeWeaponId || items[0]?.weapon_id || '')
        if (!activeWeaponId && items[0]) onWeaponSelected(items[0].weapon_id, items[0].current_version_id ?? undefined)
        setStatus(items.length ? 'ready' : 'empty')
      })
      .catch((caught) => {
        if (cancelled) return
        setStatus('error')
        setError(caught instanceof Error ? caught.message : '资产库加载失败')
      })
    return () => {
      cancelled = true
    }
  }, [api])

  useEffect(() => {
    if (activeWeaponId && activeWeaponId !== selectedWeaponId) setSelectedWeaponId(activeWeaponId)
  }, [activeWeaponId, selectedWeaponId])

  useEffect(() => {
    if (!selectedWeaponId) {
      setDetail(null)
      setSelectedVersionId('')
      return
    }
    let cancelled = false
    setError(null)
    api.getWeapon(selectedWeaponId)
      .then((nextDetail) => {
        if (cancelled) return
        setDetail(nextDetail)
        onWeaponDetailLoaded(nextDetail)
        const nextVersionId = activeVersionId && (nextDetail.versions ?? []).some((version) => version.version_id === activeVersionId)
          ? activeVersionId
          : nextDetail.current_version_id || (nextDetail.versions ?? []).at(-1)?.version_id || ''
        setSelectedVersionId(nextVersionId)
        if (nextVersionId) onVersionSelected(nextVersionId)
      })
      .catch((caught) => {
        if (cancelled) return
        setError(caught instanceof Error ? caught.message : '武器详情加载失败')
      })
    return () => {
      cancelled = true
    }
  }, [api, selectedWeaponId])

  useEffect(() => {
    if (activeVersionId && activeVersionId !== selectedVersionId) setSelectedVersionId(activeVersionId)
  }, [activeVersionId, selectedVersionId])

  const selectedVersion = useMemo(() => {
    const versions = detail?.versions ?? []
    return versions.find((version) => version.version_id === selectedVersionId) ?? versions.at(-1) ?? null
  }, [detail, selectedVersionId])

  const sourceImage = useMemo(() => {
    return findPatchableImage(selectedVersion)
  }, [selectedVersion])
  const sourceImageUrl = sourceImage ? api.getAssetFileUrl(sourceImage.asset_id) : null

  const parentVersion = useMemo(() => {
    if (!selectedVersion?.parent_version_id) return null
    return (detail?.versions ?? []).find((version) => version.version_id === selectedVersion.parent_version_id) ?? null
  }, [detail, selectedVersion])

  const comparison = useMemo(() => {
    const before = findPatchableImage(parentVersion)
    const after = selectedVersion?.version_type === 'patch' ? findPatchableImage(selectedVersion) : null
    if (!before || !after || before.asset_id === after.asset_id) return null
    return { before, after }
  }, [parentVersion, selectedVersion])

  useEffect(() => {
    const canvas = maskCanvasRef.current
    if (!sourceImage || !canvas) return
    const width = sourceImage.width ?? 1280
    const height = sourceImage.height ?? 720
    if (canvas.dataset.sourceAssetId === sourceImage.asset_id && canvas.width === width && canvas.height === height) return
    canvas.width = width
    canvas.height = height
    canvas.dataset.sourceAssetId = sourceImage.asset_id
    clearMaskCanvas(canvas)
    undoStackRef.current = []
    redoStackRef.current = []
    setMaskHasInk(false)
    syncHistoryState()
  })

  async function submitPatch() {
    if (!detail || !selectedVersion || !sourceImage) {
      setError('需要先选择一个带概念图的武器版本')
      return
    }
    setStatus('submitting')
    setError(null)
    setLastUpload(null)
    setLastPatchJobId(null)
    onEventsReset()
    try {
      const now = Date.now()
      const width = sourceImage.width ?? 1280
      const height = sourceImage.height ?? 720
      if (!maskCanvasRef.current || !maskHasInk) {
        setError('请先在源图上涂抹需要修改的区域')
        setStatus('ready')
        return
      }
      const selection = selectionPolygon(width, height, target)
      const mask = await api.uploadVersionAsset(detail.weapon_id, selectedVersion.version_id, {
        client_request_id: `desktop_patch_mask_${now}`,
        role: 'patch_mask',
        filename: `patch-mask-${now}.png`,
        mime_type: 'image/png',
        data_base64: canvasToPngBase64(maskCanvasRef.current),
        metadata: {
          source: 'desktop_patch_panel',
          target,
          tool: maskTool,
          brush_size_px: brushSize,
          non_manufacturing_asset: true,
        },
      })
      setLastUpload(mask)

      const manifest = {
        schema_version: 'PatchManifest@1',
        weapon_id: detail.weapon_id,
        source_asset_id: sourceImage.asset_id,
        source_image: sourceImage.logical_path,
        mask_asset_id: mask.asset_id,
        mask_image: mask.logical_path,
        selection: {
          tool: maskTool,
          polygon: selection,
        },
        instruction: {
          target,
          text: instruction,
        },
        preserve,
        strength,
        regenerate_3d: false,
        created_at: new Date().toISOString(),
      }
      const manifestUpload = await api.uploadVersionAsset(detail.weapon_id, selectedVersion.version_id, {
        client_request_id: `desktop_patch_manifest_${now}`,
        role: 'patch_manifest',
        filename: `patch-manifest-${now}.json`,
        mime_type: 'application/json',
        data_base64: bytesToBase64(new TextEncoder().encode(JSON.stringify(manifest))),
        metadata: {
          source: 'desktop_patch_panel',
          schema_version: 'PatchManifest@1',
          non_manufacturing_asset: true,
        },
      })

      const patch = await api.patchWeapon(detail.weapon_id, {
        client_request_id: `desktop_patch_job_${now}`,
        source_version_id: selectedVersion.version_id,
        source_image_asset_id: sourceImage.asset_id,
        mask_asset_id: mask.asset_id,
        patch_manifest_asset_id: manifestUpload.asset_id,
        target_area: target,
        instruction,
        preserve,
        strength,
        regenerate_3d: false,
        provider_id: 'mock_comfyui',
      })
      setLastPatchJobId(patch.job_id)
      onJobCreated(patch)
      const nextDetail = await api.getWeapon(detail.weapon_id)
      setDetail(nextDetail)
      onWeaponDetailLoaded(nextDetail)
      setSelectedVersionId(nextDetail.current_version_id || selectedVersion.version_id)
      if (nextDetail.current_version_id) onVersionSelected(nextDetail.current_version_id)
      setStatus('ready')
    } catch (caught) {
      setStatus('ready')
      setError(caught instanceof Error ? caught.message : '局部修改提交失败')
    }
  }

  function togglePreserve(value: string) {
    setPreserve((current) => current.includes(value) ? current.filter((item) => item !== value) : [...current, value])
  }

  async function activateVersion(versionId: string) {
    if (!detail) return
    setStatus('submitting')
    setError(null)
    try {
      const nextDetail = await api.activateVersion(detail.weapon_id, versionId)
      setDetail(nextDetail)
      onWeaponDetailLoaded(nextDetail)
      setSelectedVersionId(versionId)
      onVersionSelected(versionId)
      setStatus('ready')
    } catch (caught) {
      setStatus('ready')
      setError(caught instanceof Error ? caught.message : '版本切换失败')
    }
  }

  function retryFromParentVersion() {
    if (!parentVersion) return
    setSelectedVersionId(parentVersion.version_id)
    onVersionSelected(parentVersion.version_id)
    setLastUpload(null)
    setLastPatchJobId(null)
    setError(null)
  }

  function beginDraw(event: PointerEvent<HTMLCanvasElement>) {
    const canvas = maskCanvasRef.current
    if (!canvas) return
    const point = eventPoint(event, canvas)
    pushUndoSnapshot()
    redoStackRef.current = []
    syncHistoryState()
    isDrawingRef.current = true
    setIsDrawing(true)
    lastPointRef.current = point
    if (maskTool === 'brush') {
      drawMaskStroke(canvas, point, point, brushSize)
      setMaskHasInk(true)
    } else {
      lassoPointsRef.current = [point]
      setLassoPoints([point])
    }
    event.currentTarget.setPointerCapture(event.pointerId)
  }

  function moveDraw(event: PointerEvent<HTMLCanvasElement>) {
    const canvas = maskCanvasRef.current
    const previous = lastPointRef.current
    if (!canvas || !isDrawingRef.current || !previous) return
    const next = eventPoint(event, canvas)
    if (maskTool === 'brush') {
      drawMaskStroke(canvas, previous, next, brushSize)
    } else if (distance(previous, next) >= 6) {
      lassoPointsRef.current = [...lassoPointsRef.current, next]
      setLassoPoints(lassoPointsRef.current)
    }
    lastPointRef.current = next
  }

  function endDraw(event: PointerEvent<HTMLCanvasElement>) {
    const canvas = maskCanvasRef.current
    if (canvas && maskTool === 'lasso') {
      const points = lassoPointsRef.current
      if (points.length >= 3) {
        fillMaskPolygon(canvas, points)
        setMaskHasInk(true)
      }
      lassoPointsRef.current = []
      setLassoPoints([])
    }
    isDrawingRef.current = false
    setIsDrawing(false)
    lastPointRef.current = null
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
  }

  function clearMask() {
    const canvas = maskCanvasRef.current
    if (!canvas) return
    if (maskHasInk) {
      pushUndoSnapshot()
      redoStackRef.current = []
      syncHistoryState()
    }
    clearMaskCanvas(canvas)
    lassoPointsRef.current = []
    setLassoPoints([])
    setMaskHasInk(false)
  }

  function undoMask() {
    const canvas = maskCanvasRef.current
    const previous = undoStackRef.current.pop()
    if (!canvas || !previous) return
    const current = captureMaskSnapshot(canvas)
    if (current) redoStackRef.current.push(current)
    restoreMaskSnapshot(canvas, previous)
    setMaskHasInk(maskImageDataHasInk(previous))
    syncHistoryState()
  }

  function redoMask() {
    const canvas = maskCanvasRef.current
    const next = redoStackRef.current.pop()
    if (!canvas || !next) return
    const current = captureMaskSnapshot(canvas)
    if (current) undoStackRef.current.push(current)
    restoreMaskSnapshot(canvas, next)
    setMaskHasInk(maskImageDataHasInk(next))
    syncHistoryState()
  }

  function pushUndoSnapshot() {
    const canvas = maskCanvasRef.current
    const snapshot = canvas ? captureMaskSnapshot(canvas) : null
    if (!snapshot) return
    undoStackRef.current.push(snapshot)
    if (undoStackRef.current.length > 20) undoStackRef.current.shift()
  }

  function syncHistoryState() {
    setUndoDepth(undoStackRef.current.length)
    setRedoDepth(redoStackRef.current.length)
  }

  return (
    <section className="panel-section patch-panel">
      <div>
        <h1>Patch Mode</h1>
        <p className="muted">上传 mask 与 PatchManifest，生成新的概念图版本；输出仍限定为虚构 Unity 游戏美术资产。</p>
      </div>

      {status === 'loading' && <p className="muted">正在读取资产库...</p>}
      {status === 'empty' && <p className="muted">暂无可修改武器。先在 Forge 工作台创建一个 mock 武器。</p>}
      {status === 'error' && <p className="error">{error}</p>}

      {weapons.length > 0 && (
        <>
          <label htmlFor="patch-weapon">武器</label>
          <select
            id="patch-weapon"
            value={selectedWeaponId}
            onChange={(event) => {
              setSelectedWeaponId(event.target.value)
              onWeaponSelected(event.target.value)
            }}
          >
            {weapons.map((weapon) => (
              <option key={weapon.weapon_id} value={weapon.weapon_id}>
                {weapon.display_name} · {weapon.stage}
              </option>
            ))}
          </select>

          <label htmlFor="patch-version">源版本</label>
          <select
            id="patch-version"
            value={selectedVersionId}
            onChange={(event) => {
              setSelectedVersionId(event.target.value)
              onVersionSelected(event.target.value)
            }}
          >
            {(detail?.versions ?? []).map((version) => (
              <option key={version.version_id} value={version.version_id}>
                v{version.version_no} · {version.version_type} · {version.version_id}
              </option>
            ))}
          </select>

          <div className="source-summary">
            <strong>{sourceImage ? '源图已就绪' : '当前版本没有可 patch 的概念图'}</strong>
            {sourceImage && (
              <>
                <code>{sourceImage.asset_id}</code>
                <span>{sourceImage.role} · {sourceImage.width ?? '?'} x {sourceImage.height ?? '?'}</span>
              </>
            )}
          </div>

          {comparison && (
            <div className="comparison-panel">
              <div className="canvas-toolbar">
                <span className="muted">Patch 前后对比</span>
                <span className="muted">v{parentVersion?.version_no ?? '?'} {'->'} v{selectedVersion?.version_no ?? '?'}</span>
              </div>
              <div className="comparison-frame" style={{ '--compare-ratio': `${comparisonRatio}%` } as CSSProperties}>
                <img src={api.getAssetFileUrl(comparison.before.asset_id)} alt="Patch 前源图" draggable={false} />
                <div className="comparison-after">
                  <img src={api.getAssetFileUrl(comparison.after.asset_id)} alt="Patch 后结果图" draggable={false} />
                </div>
                <span className="comparison-label before">Before</span>
                <span className="comparison-label after">After</span>
                <span className="comparison-divider" />
              </div>
              <label className="comparison-control" htmlFor="patch-comparison">
                <span>对比位置 {comparisonRatio}%</span>
                <input
                  id="patch-comparison"
                  type="range"
                  min="5"
                  max="95"
                  step="1"
                  value={comparisonRatio}
                  onChange={(event) => setComparisonRatio(Number(event.target.value))}
                />
              </label>
              <div className="comparison-actions">
                <button
                  onClick={() => selectedVersion && activateVersion(selectedVersion.version_id)}
                  disabled={!selectedVersion || detail?.current_version_id === selectedVersion.version_id || status === 'submitting'}
                >
                  {detail?.current_version_id === selectedVersion?.version_id ? '已是当前版本' : '设为当前版本'}
                </button>
                <button
                  onClick={() => parentVersion && activateVersion(parentVersion.version_id)}
                  disabled={!parentVersion || status === 'submitting'}
                >
                  回到父版本
                </button>
                <button onClick={retryFromParentVersion} disabled={!parentVersion || status === 'submitting'}>
                  从父版本重试
                </button>
              </div>
            </div>
          )}

          <div className="canvas-toolbar">
            <span className="muted">{sourceImageUrl ? '在源图上标记要重绘的区域' : '等待源图'}</span>
            <div className="canvas-actions">
              <button onClick={undoMask} disabled={undoDepth === 0}>撤销</button>
              <button onClick={redoMask} disabled={redoDepth === 0}>重做</button>
              <button onClick={clearMask} disabled={!sourceImage || !maskHasInk}>清空 mask</button>
            </div>
          </div>

          <div className="tool-switch" aria-label="mask 工具">
            <button className={maskTool === 'brush' ? 'active' : ''} onClick={() => setMaskTool('brush')}>画笔</button>
            <button className={maskTool === 'lasso' ? 'active' : ''} onClick={() => setMaskTool('lasso')}>套索</button>
          </div>

          <div className="canvas-controls">
            <label htmlFor="brush-size">
              <span>画笔 {brushSize}px</span>
              <input
                id="brush-size"
                type="range"
                min="12"
                max="140"
                step="2"
                value={brushSize}
                onChange={(event) => setBrushSize(Number(event.target.value))}
              />
            </label>
            <label htmlFor="mask-opacity">
              <span>mask 透明度 {maskOpacity}%</span>
              <input
                id="mask-opacity"
                type="range"
                min="25"
                max="90"
                step="5"
                value={maskOpacity}
                onChange={(event) => setMaskOpacity(Number(event.target.value))}
              />
            </label>
          </div>

          <div className="mask-canvas-shell" aria-label="Patch mask 画布">
            {sourceImageUrl ? <img src={sourceImageUrl} alt="当前源概念图" draggable={false} /> : <div className="mask-placeholder" />}
            <canvas
              ref={maskCanvasRef}
              style={{ opacity: maskOpacity / 100 }}
              onPointerDown={beginDraw}
              onPointerMove={moveDraw}
              onPointerUp={endDraw}
              onPointerCancel={endDraw}
            />
            {lassoPoints.length >= 2 && (
              <svg className="lasso-preview" viewBox={`0 0 ${sourceImage?.width ?? 1280} ${sourceImage?.height ?? 720}`}>
                <polyline points={lassoPoints.map((point) => `${point.x},${point.y}`).join(' ')} />
              </svg>
            )}
            {!maskHasInk && <div className="mask-hint">标记区域会作为白色 mask 上传</div>}
          </div>

          <label htmlFor="patch-target">修改目标</label>
          <select id="patch-target" value={target} onChange={(event) => setTarget(event.target.value as PatchTarget)}>
            {targetOptions.map((option) => (
              <option key={option.value} value={option.value}>{option.label}</option>
            ))}
          </select>

          <label htmlFor="patch-instruction">修改描述</label>
          <textarea id="patch-instruction" rows={5} value={instruction} onChange={(event) => setInstruction(event.target.value)} />

          <div className="segmented-options" aria-label="保持不变">
            {preserveOptions.map((option) => (
              <label key={option.value}>
                <input type="checkbox" checked={preserve.includes(option.value)} onChange={() => togglePreserve(option.value)} />
                <span>{option.label}</span>
              </label>
            ))}
          </div>

          <label htmlFor="patch-strength">修改强度</label>
          <select id="patch-strength" value={strength} onChange={(event) => setStrength(event.target.value as Strength)}>
            <option value="subtle">轻微</option>
            <option value="medium">中等</option>
            <option value="strong">大幅</option>
          </select>

          <button className="primary" onClick={submitPatch} disabled={status === 'submitting' || !sourceImage || !maskHasInk || !instruction.trim()}>
            {status === 'submitting' ? '提交 Patch 中...' : '上传 mask 并生成 patch'}
          </button>

          {lastUpload && <p className="muted">最近上传 mask：<code>{lastUpload.asset_id}</code></p>}
          {lastPatchJobId && <p className="muted">最近 patch 任务：<code>{lastPatchJobId}</code></p>}
          {error && <p className="error">{error}</p>}
        </>
      )}
    </section>
  )
}

function findPatchableImage(version?: WeaponVersion | null): VersionAsset | null {
  const assets = [...(version?.assets ?? [])].reverse()
  return assets.find((asset) => asset.role === 'concept_patch') ?? assets.find((asset) => asset.role === 'concept_image') ?? null
}

function selectionPolygon(width: number, height: number, target: PatchTarget): Array<{ x: number; y: number }> {
  const presets: Record<PatchTarget, [number, number, number, number]> = {
    blade: [0.26, 0.28, 0.76, 0.5],
    guard: [0.18, 0.5, 0.4, 0.72],
    core: [0.56, 0.28, 0.72, 0.52],
    rune: [0.34, 0.34, 0.68, 0.47],
    glow: [0.5, 0.22, 0.8, 0.58],
    material: [0.3, 0.32, 0.74, 0.56],
    silhouette: [0.14, 0.2, 0.86, 0.78],
    handle: [0.12, 0.62, 0.34, 0.86],
    ornament: [0.18, 0.48, 0.5, 0.7],
    whole_weapon: [0.12, 0.18, 0.88, 0.82],
  }
  const [left, top, right, bottom] = presets[target]
  return [
    { x: Math.round(width * left), y: Math.round(height * top) },
    { x: Math.round(width * right), y: Math.round(height * top) },
    { x: Math.round(width * right), y: Math.round(height * bottom) },
    { x: Math.round(width * left), y: Math.round(height * bottom) },
  ]
}

function canvasToPngBase64(canvas: HTMLCanvasElement): string {
  return canvas.toDataURL('image/png').split(',')[1] ?? ''
}

function eventPoint(event: PointerEvent<HTMLCanvasElement>, canvas: HTMLCanvasElement): { x: number; y: number } {
  const rect = canvas.getBoundingClientRect()
  return {
    x: Math.round(((event.clientX - rect.left) / rect.width) * canvas.width),
    y: Math.round(((event.clientY - rect.top) / rect.height) * canvas.height),
  }
}

function clearMaskCanvas(canvas: HTMLCanvasElement): void {
  const context = canvas.getContext('2d')
  if (!context) return
  context.globalCompositeOperation = 'source-over'
  context.fillStyle = '#000'
  context.fillRect(0, 0, canvas.width, canvas.height)
}

function fillMaskPolygon(canvas: HTMLCanvasElement, points: Array<{ x: number; y: number }>): void {
  const context = canvas.getContext('2d')
  if (!context || points.length < 3) return
  context.globalCompositeOperation = 'source-over'
  context.fillStyle = '#fff'
  context.beginPath()
  context.moveTo(points[0].x, points[0].y)
  for (const point of points.slice(1)) context.lineTo(point.x, point.y)
  context.closePath()
  context.fill()
}

function distance(first: { x: number; y: number }, second: { x: number; y: number }): number {
  return Math.hypot(first.x - second.x, first.y - second.y)
}

function captureMaskSnapshot(canvas: HTMLCanvasElement): ImageData | null {
  const context = canvas.getContext('2d')
  if (!context) return null
  return context.getImageData(0, 0, canvas.width, canvas.height)
}

function restoreMaskSnapshot(canvas: HTMLCanvasElement, snapshot: ImageData): void {
  const context = canvas.getContext('2d')
  if (!context) return
  context.putImageData(snapshot, 0, 0)
}

function maskImageDataHasInk(snapshot: ImageData): boolean {
  const data = snapshot.data
  for (let index = 0; index < data.length; index += 4) {
    if (data[index] > 8 || data[index + 1] > 8 || data[index + 2] > 8) return true
  }
  return false
}

function drawMaskStroke(canvas: HTMLCanvasElement, start: { x: number; y: number }, end: { x: number; y: number }, brushSize: number): void {
  const context = canvas.getContext('2d')
  if (!context) return
  context.globalCompositeOperation = 'source-over'
  context.lineWidth = brushSize
  context.lineCap = 'round'
  context.lineJoin = 'round'
  context.strokeStyle = '#fff'
  context.fillStyle = '#fff'
  context.beginPath()
  context.moveTo(start.x, start.y)
  context.lineTo(end.x, end.y)
  context.stroke()
  context.beginPath()
  context.arc(end.x, end.y, brushSize / 2, 0, Math.PI * 2)
  context.fill()
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = ''
  const chunkSize = 0x8000
  for (let index = 0; index < bytes.length; index += chunkSize) {
    binary += String.fromCharCode(...bytes.slice(index, index + chunkSize))
  }
  return btoa(binary)
}
