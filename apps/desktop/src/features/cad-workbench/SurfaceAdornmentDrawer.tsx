import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react'

export type SurfaceAdornmentKind = 'engraving' | 'panel_line' | 'texture' | 'streamline'
export type SurfaceAdornmentMotif = 'parallel' | 'radial' | 'hexagonal' | 'technical_mark'
export type SurfaceAdornmentIntensity = 'subtle' | 'balanced' | 'bold'
export type SurfaceAdornmentCoverage = 'center' | 'edge' | 'full' | 'symmetric'

export type SurfaceAdornmentTarget = {
  projectId: string
  assetVersionId: string
  partId: string
  partLabel: string
  materialZoneId: string
  materialZoneLabel: string
}

export type SurfaceAdornmentDraft = {
  kind: SurfaceAdornmentKind
  motif: SurfaceAdornmentMotif
  intensity: SurfaceAdornmentIntensity
  coverage: SurfaceAdornmentCoverage
}

export type SurfaceAdornmentPreviewResponse =
  | { status: 'preview_ready'; changeSetId: string; summary: string }
  | { status: 'activation_required'; message: string }
  | { status: 'unavailable' | 'failed'; message: string; errorCode?: string }

export type SurfaceAdornmentRetainResponse =
  | { status: 'retained'; summary: string }
  | { status: 'unavailable' | 'failed'; message: string }

export const surfaceAdornmentPreviewEndpoint = (assetVersionId: string) =>
  `/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}/surface-adornments:preview`
export const surfaceAdornmentConfirmEndpoint = (changeSetId: string) =>
  `/api/v1/agent/change-sets/${encodeURIComponent(changeSetId)}:confirm`
export const surfaceAdornmentRejectEndpoint = (changeSetId: string) =>
  `/api/v1/agent/change-sets/${encodeURIComponent(changeSetId)}:reject`

/**
 * This is deliberately a product-facing seam, not a generated API client.
 * A005 wiring must replace this adapter only after the server returns a real
 * ChangeSet preview and the renderer has received that preview. Until then,
 * callers return `unavailable`; the drawer never presents a fake preview.
 */
export type SurfaceAdornmentAdapter = {
  enable: () => Promise<{ status: 'enabled' } | { status: 'failed'; message: string }>
  preview: (target: SurfaceAdornmentTarget, draft: SurfaceAdornmentDraft) => Promise<SurfaceAdornmentPreviewResponse>
  retain: (changeSetId: string) => Promise<SurfaceAdornmentRetainResponse>
  cancel: (changeSetId: string) => Promise<void>
}

export const unavailableSurfaceAdornmentAdapter: SurfaceAdornmentAdapter = {
  async enable() {
    return { status: 'failed', message: '外观细节能力尚未连接。' }
  },
  async preview() {
    return {
      status: 'unavailable',
      message: '外观细节预览正在连接模型服务；当前不会创建修改或新版本。',
    }
  },
  async retain() {
    return {
      status: 'unavailable',
      message: '外观细节预览尚未就绪，当前没有可保留的修改。',
    }
  },
  async cancel() {},
}

export type SurfaceAdornmentDrawerProps = {
  open: boolean
  target: SurfaceAdornmentTarget | null
  disabledReason?: string | null
  adapter: SurfaceAdornmentAdapter
  onClose: () => void
  onMessage?: (message: string) => void
}

const KIND_OPTIONS: Array<{ value: SurfaceAdornmentKind; label: string }> = [
  { value: 'engraving', label: '浅雕刻感' },
  { value: 'panel_line', label: '面板分缝' },
  { value: 'texture', label: '微表面纹理' },
  { value: 'streamline', label: '流线点缀' },
]
const MOTIF_OPTIONS: Array<{ value: SurfaceAdornmentMotif; label: string }> = [
  { value: 'parallel', label: '平行条纹' },
  { value: 'radial', label: '放射纹样' },
  { value: 'hexagonal', label: '六边微纹' },
  { value: 'technical_mark', label: '技术标记' },
]
const INTENSITY_OPTIONS: Array<{ value: SurfaceAdornmentIntensity; label: string }> = [
  { value: 'subtle', label: '轻微' },
  { value: 'balanced', label: '适中' },
  { value: 'bold', label: '明显' },
]
const COVERAGE_OPTIONS: Array<{ value: SurfaceAdornmentCoverage; label: string }> = [
  { value: 'center', label: '中心区域' },
  { value: 'edge', label: '边缘区域' },
  { value: 'full', label: '整个材质区' },
  { value: 'symmetric', label: '对称双侧' },
]

const INITIAL_DRAFT: SurfaceAdornmentDraft = {
  kind: 'engraving', motif: 'parallel', intensity: 'subtle', coverage: 'center',
}

export function SurfaceAdornmentDrawer({
  open,
  target,
  disabledReason = null,
  adapter,
  onClose,
  onMessage,
}: SurfaceAdornmentDrawerProps) {
  const [draft, setDraft] = useState<SurfaceAdornmentDraft>(INITIAL_DRAFT)
  const [status, setStatus] = useState<'editing' | 'processing' | 'activation_required' | 'preview_ready' | 'failed'>('editing')
  const [detail, setDetail] = useState('')
  const [errorCode, setErrorCode] = useState('')
  const changeSetIdRef = useRef<string | null>(null)
  const requestTokenRef = useRef(0)
  const closeButtonRef = useRef<HTMLButtonElement | null>(null)
  const dialogRef = useRef<HTMLElement | null>(null)
  const contextKey = target
    ? `${target.projectId}:${target.assetVersionId}:${target.partId}:${target.materialZoneId}`
    : 'unavailable'

  const reset = () => {
    requestTokenRef.current += 1
    changeSetIdRef.current = null
    setStatus('editing')
    setDetail('')
    setErrorCode('')
  }

  useEffect(() => {
    reset()
  // A project, asset, part, or material-zone switch invalidates any late reply.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [contextKey])

  useEffect(() => {
    if (!open) return
    window.requestAnimationFrame(() => closeButtonRef.current?.focus())
  }, [open])

  if (!open) return null
  const unavailable = !target || Boolean(disabledReason)
  const close = () => {
    const changeSetId = changeSetIdRef.current
    reset()
    if (changeSetId) void adapter.cancel(changeSetId)
    onClose()
  }
  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault()
      close()
      return
    }
    if (event.key !== 'Tab') return
    const dialog = dialogRef.current
    if (!dialog) return
    const controls = [...dialog.querySelectorAll<HTMLElement>('button:not(:disabled), select:not(:disabled)')]
    if (controls.length === 0) return
    const first = controls[0]
    const last = controls[controls.length - 1]
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last?.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first?.focus()
    }
  }
  const runPreview = async () => {
    if (!target || unavailable || status === 'processing') return
    const token = ++requestTokenRef.current
    changeSetIdRef.current = null
    setStatus('processing')
    setDetail('正在生成外观细节预览…')
    setErrorCode('')
    const response = await adapter.preview(target, draft).catch((caught): SurfaceAdornmentPreviewResponse => ({
      status: 'failed',
      message: caught instanceof Error ? caught.message : '外观细节预览失败；当前设计没有变化。',
      errorCode: typeof caught === 'object' && caught !== null && 'code' in caught && typeof caught.code === 'string'
        ? caught.code
        : 'SURFACE_ADORNMENT_ADAPTER_REJECTED',
    }))
    if (token !== requestTokenRef.current) return
    if (response.status !== 'preview_ready') {
      // Publish the diagnostic attribute before exposing the terminal failed
      // class.  The packaged WebView observer must never see a transient
      // failure node whose stable API/stage code still belongs to the previous
      // render.
      setErrorCode('errorCode' in response ? response.errorCode ?? '' : '')
      setStatus(response.status === 'activation_required' ? 'activation_required' : 'failed')
      setDetail(response.message)
      onMessage?.(response.message)
      return
    }
    changeSetIdRef.current = response.changeSetId
    setStatus('preview_ready')
    setDetail(response.summary)
    onMessage?.(response.summary)
  }
  const enableAndPreview = async () => {
    if (status !== 'activation_required') return
    const token = ++requestTokenRef.current
    setStatus('processing')
    setDetail('正在启用内置外观细节能力…')
    const response = await adapter.enable().catch((caught) => ({
      status: 'failed' as const,
      message: caught instanceof Error ? caught.message : '启用外观细节能力失败。',
    }))
    if (token !== requestTokenRef.current) return
    if (response.status !== 'enabled') {
      setStatus('failed')
      setDetail(response.message)
      onMessage?.(response.message)
      return
    }
    setStatus('editing')
    setDetail('外观细节能力已启用。请再次点击预览。')
    onMessage?.('外观细节能力已明确启用。')
  }
  const retain = async () => {
    const changeSetId = changeSetIdRef.current
    if (!changeSetId || status !== 'preview_ready') return
    const token = ++requestTokenRef.current
    setStatus('processing')
    setDetail('正在保留外观细节…')
    const response = await adapter.retain(changeSetId).catch((caught): SurfaceAdornmentRetainResponse => ({
      status: 'failed', message: caught instanceof Error ? caught.message : '保留外观细节失败；当前设计没有变化。',
    }))
    if (token !== requestTokenRef.current) return
    if (response.status !== 'retained') {
      setStatus('failed')
      setDetail(response.message)
      onMessage?.(response.message)
      return
    }
    changeSetIdRef.current = null
    setStatus('editing')
    setDetail(response.summary)
    onMessage?.(response.summary)
  }

  return (
    <section ref={dialogRef} className="surface-adornment-drawer" role="dialog" aria-modal="true" aria-label="添加外观细节" onKeyDown={onKeyDown}>
      <header>
        <div><strong>添加外观细节</strong><small>只细化外观，不提供制造或性能结论。</small></div>
        <button ref={closeButtonRef} type="button" aria-label="关闭添加外观细节" onClick={close}>关闭</button>
      </header>
      {target ? (
        <p className="surface-adornment-target"><strong>{target.partLabel}</strong><span>{target.materialZoneLabel}</span></p>
      ) : null}
      {unavailable ? (
        <p className="surface-adornment-status failed" role="status">{disabledReason ?? '请先保存设计、选择一个部件和材质区。'}</p>
      ) : (
        <>
          <div className="surface-adornment-fields">
            <ChoiceField label="细节类型" value={draft.kind} options={KIND_OPTIONS} disabled={status === 'processing'} onChange={(kind) => setDraft((value) => ({ ...value, kind }))} />
            <ChoiceField label="图案" value={draft.motif} options={MOTIF_OPTIONS} disabled={status === 'processing'} onChange={(motif) => setDraft((value) => ({ ...value, motif }))} />
            <ChoiceField label="强度" value={draft.intensity} options={INTENSITY_OPTIONS} disabled={status === 'processing'} onChange={(intensity) => setDraft((value) => ({ ...value, intensity }))} />
            <ChoiceField label="覆盖区域" value={draft.coverage} options={COVERAGE_OPTIONS} disabled={status === 'processing'} onChange={(coverage) => setDraft((value) => ({ ...value, coverage }))} />
          </div>
          <SurfaceAdornmentDesignSurface draft={draft} target={target} />
          {status === 'activation_required' ? (
            <button type="button" className="surface-adornment-primary" onClick={() => void enableAndPreview()}>启用外观细节能力</button>
          ) : status !== 'preview_ready' ? (
            <button type="button" className="surface-adornment-primary" disabled={status === 'processing'} onClick={() => void runPreview()}>{status === 'processing' ? '正在处理…' : '预览外观细节'}</button>
          ) : null}
          {status === 'preview_ready' && (
            <div className="surface-adornment-actions">
              <button type="button" onClick={close}>取消</button>
              <button type="button" className="surface-adornment-primary" onClick={() => void retain()}>保留</button>
            </div>
          )}
        </>
      )}
      {detail && <p className={`surface-adornment-status ${status === 'failed' ? 'failed' : ''}`} data-error-code={errorCode} role="status" aria-live="polite">{detail}</p>}
    </section>
  )
}

/**
 * A small, inspectable design surface for the A005 editor.  The SVG is a
 * two-dimensional description of the requested layer stack, not a CSS model
 * or a replacement for the server-owned GLB/PBR preview.  Preview and retain
 * continue through the adapter above, which lowers the constrained draft into
 * the Rust-owned SurfaceAdornmentProgram and Material Zone ChangeSet.
 */
export function SurfaceAdornmentDesignSurface({
  draft,
  target,
}: {
  draft: SurfaceAdornmentDraft
  target: SurfaceAdornmentTarget
}) {
  const clipId = useId().replace(/:/g, '')
  const layerTone = draft.kind === 'streamline'
    ? '#5cceff'
    : draft.kind === 'texture'
      ? '#9eaec3'
      : draft.kind === 'panel_line'
        ? '#b6d6ee'
        : '#73c5ff'
  const layerOpacity = draft.intensity === 'subtle' ? 0.48 : draft.intensity === 'balanced' ? 0.72 : 0.96
  const coverage = coverageBox(draft.coverage)

  return (
    <figure
      className="surface-adornment-design-surface"
      data-testid="surface-adornment-design-surface"
      data-surface-truth="editor_only"
      data-material-zone-id={target.materialZoneId}
      aria-label={`二维表面设计预览：${target.materialZoneLabel}`}
    >
      <div className="surface-adornment-design-surface-heading">
        <strong>二维表面设计</strong>
        <span>SVG 图层预览</span>
      </div>
      <svg viewBox="0 0 280 132" role="img" aria-label={`${target.partLabel}的${draft.motif}二维外观图层`}>
        <defs>
          <linearGradient id={`${clipId}-shell`} x1="0" x2="1" y1="0" y2="1">
            <stop offset="0" stopColor="#29465d" />
            <stop offset="0.52" stopColor="#152a3b" />
            <stop offset="1" stopColor="#0b1722" />
          </linearGradient>
          <clipPath id={`${clipId}-zone`}>
            <path d="M26 38 51 19h167l36 18-12 54-31 21H56L25 91Z" />
          </clipPath>
        </defs>
        <path className="surface-adornment-shell" d="M26 38 51 19h167l36 18-12 54-31 21H56L25 91Z" fill={`url(#${clipId}-shell)`} />
        <path className="surface-adornment-shell-edge" d="M26 38 51 19h167l36 18-12 54-31 21H56L25 91Z" />
        <g clipPath={`url(#${clipId}-zone)`} opacity={layerOpacity}>
          <rect x={coverage.x} y={coverage.y} width={coverage.width} height={coverage.height} fill="#0d2030" opacity="0.36" />
          <AdornmentMotif motif={draft.motif} kind={draft.kind} tone={layerTone} coverage={draft.coverage} />
        </g>
        <path className="surface-adornment-panel-inset" d="M53 44 71 32h130l25 13-11 43-22 13H68L50 87Z" />
        <circle cx="61" cy="49" r="3" fill="#8dd4ff" opacity="0.62" />
        <circle cx="219" cy="88" r="3" fill="#8dd4ff" opacity="0.48" />
      </svg>
      <figcaption>
        <span>目标：{target.materialZoneLabel}</span>
        <span>SVG 只编辑轮廓/图层；保留时才写入真实 PBR 与 GLB。</span>
      </figcaption>
    </figure>
  )
}

function AdornmentMotif({
  motif,
  kind,
  tone,
  coverage,
}: {
  motif: SurfaceAdornmentMotif
  kind: SurfaceAdornmentKind
  tone: string
  coverage: SurfaceAdornmentCoverage
}) {
  const strokeWidth = kind === 'engraving' ? 1.3 : kind === 'panel_line' ? 1.7 : 1.1
  const dashArray = kind === 'texture' ? '1.8 3.1' : undefined
  if (motif === 'parallel') {
    return <g stroke={tone} strokeWidth={strokeWidth} strokeDasharray={dashArray} fill="none">
      {[0, 1, 2, 3, 4, 5].map((index) => <path key={index} d={`M38 ${44 + index * 8} 235 ${24 + index * 8}`} />)}
    </g>
  }
  if (motif === 'radial') {
    return <g stroke={tone} strokeWidth={strokeWidth} strokeDasharray={dashArray} fill="none">
      {[0, 1, 2, 3, 4, 5, 6].map((index) => {
        const angle = (Math.PI * 2 * index) / 7
        const x = 142 + Math.cos(angle) * 102
        const y = 66 + Math.sin(angle) * 48
        return <path key={index} d={`M142 66 ${x.toFixed(1)} ${y.toFixed(1)}`} />
      })}
      <circle cx="142" cy="66" r="10" stroke={tone} />
    </g>
  }
  if (motif === 'hexagonal') {
    const cells = [
      [86, 47], [110, 47], [134, 47], [98, 67], [122, 67], [146, 67], [110, 87], [134, 87],
    ] as const
    return <g stroke={tone} strokeWidth={strokeWidth} strokeDasharray={dashArray} fill="none">
      {cells.map(([x, y]) => <path key={`${x}-${y}`} d={`M${x} ${y - 7}l7 4v7l-7 4-7-4v-7Z`} />)}
    </g>
  }
  return <g stroke={tone} strokeWidth={strokeWidth} strokeDasharray={dashArray} fill="none">
    <path d="M66 83 91 52h76l23 15-22 25H99Z" />
    <path d="M80 68h91M126 44v48M156 49l20 18-18 20" />
    <circle cx="87" cy="68" r="4" /><circle cx="178" cy="68" r="4" />
    {coverage === 'symmetric' && <path d="M51 52 74 67 51 82M233 52 210 67 233 82" />}
  </g>
}

function coverageBox(coverage: SurfaceAdornmentCoverage): { x: number; y: number; width: number; height: number } {
  if (coverage === 'edge') return { x: 28, y: 22, width: 224, height: 88 }
  if (coverage === 'full') return { x: 24, y: 18, width: 232, height: 96 }
  if (coverage === 'symmetric') return { x: 42, y: 30, width: 198, height: 74 }
  return { x: 68, y: 35, width: 150, height: 62 }
}

function ChoiceField<T extends string>({
  label, value, options, disabled, onChange,
}: {
  label: string
  value: T
  options: Array<{ value: T; label: string }>
  disabled: boolean
  onChange: (value: T) => void
}) {
  return (
    <label>
      <span>{label}</span>
      <select value={value} disabled={disabled} onChange={(event) => onChange(event.target.value as T)}>
        {options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
      </select>
    </label>
  )
}
