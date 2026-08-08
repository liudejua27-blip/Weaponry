import { useMemo } from 'react'

export function RuntimeViewer() {
  const capabilities = useMemo(() => [
    ['入口', 'Codex → MCP stdio'],
    ['写入模型', 'preview → confirm → immutable snapshot'],
    ['渲染器', 'Viewer only（单一 WebGL 上下文）'],
    ['当前阶段', 'MCP001 · capability discovery'],
  ], [])

  return <main className="runtime-shell">
    <header className="runtime-header">
      <div><p className="eyebrow">FORGECAD RUNTIME</p><h1>3D Runtime Viewer</h1><p className="subtitle">由 Codex 通过 MCP 调用，工作台负责验证、保存和展示。</p></div>
      <div className="status-pill" role="status"><span className="status-dot" />Runtime ready · Viewer mode</div>
    </header>
    <section className="runtime-grid" aria-label="ForgeCAD runtime viewer">
      <div className="viewport-card">
        <div className="viewport-toolbar"><span>ActiveDesignSnapshot</span><span className="toolbar-muted">暂无已确认快照</span></div>
        <div className="viewport-stage" aria-label="3D viewport placeholder"><div className="viewport-crosshair" aria-hidden="true" /><div className="viewport-message"><span className="viewport-icon">◇</span><strong>等待 Codex 提交设计</strong><span>这里仅查看模型、材质、参考比较和版本状态。</span></div></div>
        <div className="viewport-footer"><span>Geometry: gated</span><span>Materials: gated</span><span>Quality: receipt required</span></div>
      </div>
      <aside className="runtime-panel">
        <section className="panel-section"><p className="section-kicker">CALL PATH</p><h2>Codex 是唯一外部 Agent</h2><p className="panel-copy">普通用户在 Codex 中对话并上传授权参考图。Codex 通过 MCP 工具提交类型化请求，ForgeCAD 不内置模型、聊天页或 API Key。</p></section>
        <section className="panel-section"><p className="section-kicker">LIVE CONTRACT</p><div className="capability-list">{capabilities.map(([label, value]) => <div className="capability-row" key={label}><span>{label}</span><strong>{value}</strong></div>)}</div></section>
        <section className="panel-section panel-note"><p className="section-kicker">NEXT</p><p className="panel-copy">MCP002 将接入本地项目读取与快照查询；几何、UV、PBR、纹理、材质、局部细节、爆炸图和视觉评审都必须经过独立 Skill/Recipe/Validator 质量门后才会开放。</p></section>
      </aside>
    </section>
  </main>
}
