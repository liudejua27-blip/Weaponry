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
  }>
  head_snapshot?: unknown
}

type ArtifactBytes = {
  artifact_id?: string
  candidate_id?: string
  bytes_base64?: string
  sha256?: string
}

type ViewerModel = {
  status: 'Ready' | 'Unavailable'
  retryable: boolean
  projects: ViewerProject[]
  code?: string
}

const EMPTY_MODEL: ViewerModel = { status: 'Unavailable', retryable: true, projects: [] }

export function RuntimeViewer() {
  const [model, setModel] = useState<ViewerModel>(EMPTY_MODEL)
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
      </div>
      <aside className="runtime-panel">
        <section className="panel-section"><p className="section-kicker">CALL PATH</p><h2>Codex 是唯一外部 Agent</h2><p className="panel-copy">普通用户在 Codex 中对话并上传授权参考图。Codex 通过 MCP 工具提交类型化请求，ForgeCAD 不内置模型、聊天页或 API Key。</p></section>
        <section className="panel-section"><p className="section-kicker">LIVE CONTRACT</p><div className="capability-list">{capabilities.map(([label, value]) => <div className="capability-row" key={label}><span>{label}</span><strong>{value}</strong></div>)}</div></section>
        <section className="panel-section panel-note"><p className="section-kicker">MVP STATUS</p><p className="panel-copy">Viewer 通过受保护的本地 IPC 读取 Runtime 的候选、GLB bytes、版本和当前快照；Three.js 只创建临时 canvas scene，不写数据库、不改变 Runtime artifact。固定渲染证据和 PBR metadata 与 candidate hash 绑定。</p></section>
      </aside>
    </section>
  </main>
}
