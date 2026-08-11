import { useEffect, useMemo, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import * as THREE from 'three'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'

type ViewerProject = {
  project?: { project_id?: string; name?: string }
  record?: { head_snapshot_id?: string | null }
  versions?: unknown[]
  candidates?: Array<{
    candidate?: { candidate_id?: string; state?: string; quality_hard_gate_passed?: boolean }
    artifact?: { artifact_id?: string; mime?: string; part_ids?: string[]; triangle_count?: number; validator_status?: string; uv_status?: string; tangent_status?: string } | null
    quality?: QualityReport | null
    reference?: { reference?: ReferenceEvidence } | null
  }>
  head_snapshot?: unknown
}

type ArtifactBytes = {
  artifact_id?: string
  candidate_id?: string
  mime?: string
  bytes_base64?: string
  sha256?: string
}

type ReferenceEvidence = {
  reference_id?: string
  project_id?: string
  object_sha256?: string
  mime?: string
  width?: number
  height?: number
}

type QualityReport = {
  reference_id?: string | null
  reference_sha256?: string | null
  render_set_hash?: string
  comparison_report_hash?: string
  visual_status?: string
  hard_gate_passed?: boolean
}

type ViewerVisualEvidence = {
  reference_id?: string
  render_set_hash?: string
  comparison_report_hash?: string | null
  quality_report_hash?: string
  quality_report?: QualityReport
  comparison_report?: {
    status?: string
    metrics?: Record<string, number>
  } | null
}

type RenderPass = {
  mime?: string
  png_base64?: string
  sha256?: string
  pass?: string
}

const AOV_PASSES = ['beauty', 'silhouette', 'depth', 'normal', 'ao', 'part-id', 'material-id', 'wireframe', 'uv-stretch'] as const
type AovPass = typeof AOV_PASSES[number]
type CompareMode = 'split' | 'overlay' | 'flicker'

type ViewerModel = {
  status: 'Ready' | 'Unavailable'
  retryable: boolean
  projects: ViewerProject[]
  code?: string
}

const EMPTY_MODEL: ViewerModel = { status: 'Unavailable', retryable: true, projects: [] }

export function RuntimeViewer() {
  const [model, setModel] = useState<ViewerModel>(EMPTY_MODEL)
  const [selectedPass, setSelectedPass] = useState<AovPass>('beauty')
  const [compareMode, setCompareMode] = useState<CompareMode>('split')
  const [evidence, setEvidence] = useState<ViewerVisualEvidence | null>(null)
  const [referenceImage, setReferenceImage] = useState<ArtifactBytes | null>(null)
  const [renderImage, setRenderImage] = useState<RenderPass | null>(null)
  const [flickerOn, setFlickerOn] = useState(true)
  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  const capabilities = useMemo(() => [
    ['入口', 'Codex → MCP stdio'],
    ['写入模型', 'preview → confirm → immutable snapshot'],
    ['Viewer 权限', '只读 Runtime read model'],
    ['当前阶段', 'MCP007 · geometry + GLB readback'],
  ], [])

  useEffect(() => {
    let active = true
    const refresh = async () => {
      try {
        const next = await invoke<ViewerModel>('viewer_read_model')
        if (active) setModel(next)
      } catch {
        if (active) setModel(EMPTY_MODEL)
      }
    }
    void refresh()
    const timer = window.setInterval(() => void refresh(), 2000)
    return () => {
      active = false
      window.clearInterval(timer)
    }
  }, [])

  const project = model.projects[0]
  const ready = model.status === 'Ready'
  const projectName = project?.project?.name ?? '暂无项目'
  const versionCount = project?.versions?.length ?? 0
  const latestCandidate = project?.candidates?.[0]
  const artifact = latestCandidate?.artifact
  const partCount = artifact?.part_ids?.length ?? 0
  const candidateId = latestCandidate?.candidate?.candidate_id
  const projectId = project?.project?.project_id
  const reference = latestCandidate?.reference?.reference
  const referenceId = evidence?.reference_id ?? reference?.reference_id ?? latestCandidate?.quality?.reference_id ?? undefined
  const renderSetHash = evidence?.render_set_hash ?? latestCandidate?.quality?.render_set_hash

  useEffect(() => {
    let active = true
    setEvidence(null)
    setReferenceImage(null)
    setRenderImage(null)
    if (!candidateId) return () => { active = false }
    void invoke<ViewerVisualEvidence>('viewer_visual_evidence', { candidateId }).then((next) => {
      if (active && next?.quality_report) setEvidence(next)
    }).catch(() => undefined)
    return () => { active = false }
  }, [candidateId])

  useEffect(() => {
    let active = true
    setReferenceImage(null)
    setRenderImage(null)
    if (!referenceId || !projectId || !renderSetHash) return () => { active = false }
    void Promise.all([
      invoke<ArtifactBytes>('viewer_reference_bytes', { referenceId, projectId }),
      invoke<RenderPass>('viewer_render_pass', { renderSetHash, pass: selectedPass }),
    ]).then(([referencePayload, renderPayload]) => {
      if (!active) return
      setReferenceImage(referencePayload)
      setRenderImage(renderPayload)
    }).catch(() => undefined)
    return () => { active = false }
  }, [referenceId, projectId, renderSetHash, selectedPass])

  useEffect(() => {
    if (compareMode !== 'flicker') {
      setFlickerOn(true)
      return undefined
    }
    const timer = window.setInterval(() => setFlickerOn((value) => !value), 500)
    return () => window.clearInterval(timer)
  }, [compareMode])

  useEffect(() => {
    const artifactId = artifact?.artifact_id
    const candidateId = latestCandidate?.candidate?.candidate_id
    const canvas = canvasRef.current
    if (!artifactId || !candidateId || !canvas) return
    let disposed = false
    const scene = new THREE.Scene()
    scene.background = new THREE.Color('#080d14')
    const camera = new THREE.PerspectiveCamera(32, 1, 0.01, 100)
    const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true })
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
    renderer.setSize(canvas.clientWidth || 640, canvas.clientHeight || 520, false)
    scene.add(new THREE.HemisphereLight('#f1f6ff', '#172536', 2.2))
    const key = new THREE.DirectionalLight('#ffd39c', 2.4)
    key.position.set(4, 5, 6)
    scene.add(key)
    const loader = new GLTFLoader()
    void invoke<ArtifactBytes>('viewer_artifact_bytes', { artifactId, candidateId }).then((payload) => {
      if (disposed || !payload.bytes_base64) return
      const binary = Uint8Array.from(atob(payload.bytes_base64), (character) => character.charCodeAt(0))
      loader.parse(binary.buffer, '', (gltf) => {
        if (disposed) return
        const root = gltf.scene
        scene.add(root)
        const box = new THREE.Box3().setFromObject(root)
        const size = box.getSize(new THREE.Vector3())
        const center = box.getCenter(new THREE.Vector3())
        const radius = Math.max(size.x, size.y, size.z, 0.1)
        camera.position.set(radius * 1.9, radius * 1.15, radius * 2.1)
        camera.lookAt(center)
        renderer.render(scene, camera)
      }, () => undefined)
    }).catch(() => undefined)
    return () => {
      disposed = true
      renderer.dispose()
      while (scene.children.length) scene.remove(scene.children[0])
    }
  }, [artifact?.artifact_id, latestCandidate?.candidate?.candidate_id])

  const referenceDataUrl = referenceImage?.bytes_base64
    ? `data:${referenceImage.mime ?? 'image/png'};base64,${referenceImage.bytes_base64}`
    : undefined
  const renderDataUrl = renderImage?.png_base64
    ? `data:${renderImage.mime ?? 'image/png'};base64,${renderImage.png_base64}`
    : undefined
  const comparisonMetrics = evidence?.comparison_report?.metrics ?? {}
  const visualStatus = evidence?.quality_report?.visual_status ?? latestCandidate?.quality?.visual_status ?? 'not-run'
  const hardGatePassed = evidence?.quality_report?.hard_gate_passed ?? latestCandidate?.candidate?.quality_hard_gate_passed ?? false
  const metricLabels: Array<[string, string]> = [
    ['silhouette_iou', 'Silhouette IoU'],
    ['boundary_f1_4px', 'Boundary F1'],
    ['bbox_edge_error', 'BBox edge error'],
    ['centroid_error', 'Centroid error'],
    ['landmark_coverage', 'Landmark coverage'],
  ]

  return <main className="runtime-shell">
    <header className="runtime-header">
      <div><p className="eyebrow">FORGECAD RUNTIME</p><h1>3D Runtime Viewer</h1><p className="subtitle">由 Codex 通过 MCP 调用；Viewer 只读取 Runtime 投影，不参与写入。</p></div>
      <div className={`status-pill ${ready ? '' : 'status-pill-muted'}`} role="status"><span className="status-dot" />{ready ? 'Runtime ready · read-only' : 'Runtime unavailable · Viewer mode'}</div>
    </header>
    <section className="runtime-grid" aria-label="ForgeCAD runtime viewer">
      <div className="viewport-card">
        <div className="viewport-toolbar"><span>ActiveDesignSnapshot</span><span className="toolbar-muted">{ready ? (project?.record?.head_snapshot_id ? '已读取当前快照' : '暂无已确认快照') : '等待 Runtime'}</span></div>
        <div className="viewport-stage" aria-label={artifact ? 'GLB artifact readback' : '3D viewport placeholder'}><div className="viewport-crosshair" aria-hidden="true" />{artifact ? <><canvas ref={canvasRef} className="glb-canvas" aria-label="Runtime GLB 3D preview" /><div className="viewport-message"><span className="viewport-icon">◇</span><strong>GLB readback 已连接</strong><span>{partCount} 个语义部件 · {artifact.triangle_count ?? 0} triangles · UV {artifact.uv_status ?? 'unknown'} · tangent {artifact.tangent_status ?? 'unknown'}</span><code>{artifact.artifact_id}</code></div></> : <div className="viewport-message"><span className="viewport-icon">◇</span><strong>等待 Codex 提交设计</strong><span>这里仅查看模型、材质、参考比较和版本状态。</span></div>}</div>
        <div className="viewport-footer"><span>Project: {projectName}</span><span>Versions: {versionCount}</span><span>Candidate: {latestCandidate?.candidate?.state ?? 'none'}</span></div>
        <section className="compare-panel" aria-label="Reference and fixed render comparison">
          <div className="compare-header">
            <div><p className="section-kicker">REFERENCE COMPARE</p><h2>固定视图证据</h2></div>
            <div className="compare-status"><span className={`quality-dot ${hardGatePassed ? 'quality-dot-pass' : ''}`} />{visualStatus}</div>
          </div>
          <div className="compare-toolbar">
            <div className="aov-tabs" role="tablist" aria-label="Render AOV passes">
              {AOV_PASSES.map((pass) => <button key={pass} type="button" className={`aov-tab ${selectedPass === pass ? 'aov-tab-active' : ''}`} aria-selected={selectedPass === pass} onClick={() => setSelectedPass(pass)}>{pass}</button>)}
            </div>
            <div className="mode-tabs" role="group" aria-label="Compare mode">
              {(['split', 'overlay', 'flicker'] as CompareMode[]).map((mode) => <button key={mode} type="button" className={`mode-tab ${compareMode === mode ? 'mode-tab-active' : ''}`} aria-pressed={compareMode === mode} onClick={() => setCompareMode(mode)}>{mode}</button>)}
            </div>
          </div>
          <div className={`compare-stage compare-${compareMode}`} aria-label={`${selectedPass} reference comparison`}>
            {referenceDataUrl && (compareMode === 'split' || compareMode === 'overlay' || (compareMode === 'flicker' && !flickerOn)) && <div className="compare-pane compare-reference"><span>REFERENCE</span><img src={referenceDataUrl} alt="Authorized reference" /></div>}
            {renderDataUrl && (compareMode === 'split' || compareMode === 'overlay' || (compareMode === 'flicker' && flickerOn)) && <div className="compare-pane compare-render"><span>{selectedPass.toUpperCase()}</span><img src={renderDataUrl} alt={`Fixed render ${selectedPass}`} /></div>}
            {!referenceDataUrl || !renderDataUrl ? <div className="compare-empty">等待 candidate-bound 参考图、RenderSet 和 {selectedPass} PNG</div> : null}
          </div>
          <div className="compare-footer"><span>Camera lock · 512×512 perspective</span><span>RenderSet: {renderSetHash ?? 'not-run'}</span><span>Reference: {referenceId ?? 'not-run'}</span></div>
        </section>
      </div>
      <aside className="runtime-panel">
        <section className="panel-section"><p className="section-kicker">CALL PATH</p><h2>Codex 是唯一外部 Agent</h2><p className="panel-copy">普通用户在 Codex 中对话并上传授权参考图。Codex 通过 MCP 工具提交类型化请求，ForgeCAD 不内置模型、聊天页或 API Key。</p></section>
        <section className="panel-section"><p className="section-kicker">LIVE CONTRACT</p><div className="capability-list">{capabilities.map(([label, value]) => <div className="capability-row" key={label}><span>{label}</span><strong>{value}</strong></div>)}</div></section>
        <section className="panel-section"><p className="section-kicker">QUALITY EVIDENCE</p><div className="quality-summary"><div><span>Visual status</span><strong>{visualStatus}</strong></div><div><span>Hard gate</span><strong>{hardGatePassed ? 'PASS' : 'NOT PASSED'}</strong></div>{metricLabels.map(([key, label]) => <div key={key}><span>{label}</span><strong>{typeof comparisonMetrics[key] === 'number' ? comparisonMetrics[key].toFixed(3) : '—'}</strong></div>)}</div></section>
        <section className="panel-section panel-note"><p className="section-kicker">MVP STATUS</p><p className="panel-copy">Viewer 通过受保护的本地 IPC 读取 Runtime 的候选、GLB bytes、版本和当前快照；Three.js 只创建临时 canvas scene，不写数据库、不改变 Runtime artifact。固定渲染证据和 PBR metadata 与 candidate hash 绑定。</p></section>
      </aside>
    </section>
  </main>
}
