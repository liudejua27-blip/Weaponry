import { useState } from 'react'
import type { VisualGenerationState } from './useVisualGeneration.js'

export function VisualGenerationCard(props: {
  state: VisualGenerationState
  onConfigure: (key: string) => Promise<unknown>
  onCancel: () => void
  onResume: () => void
  onRetry: () => void
}) {
  const [key, setKey] = useState('')
  const [saving, setSaving] = useState(false)
  const [setupError, setSetupError] = useState('')

  if (props.state.status === 'idle') return null

  if (props.state.status === 'needs_configuration') {
    const save = async () => {
      if (!key.trim() || saving) return
      setSaving(true)
      setSetupError('')
      try {
        await props.onConfigure(key)
        setKey('')
      } catch (caught) {
        setSetupError(caught instanceof Error ? caught.message : String(caught))
      } finally {
        setSaving(false)
      }
    }
    return (
      <section className="visual-generation-card" aria-label="配置视觉生成服务">
        <strong>连接视觉生成服务</strong>
        <p>{props.state.detail}</p>
        <label>
          <span>FAL API Key</span>
          <input
            type="password"
            value={key}
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => setKey(event.target.value)}
            placeholder="仅保存到本机私密文件"
          />
        </label>
        <small>不会写入项目、日志或 Git，也不会触发 macOS 钥匙串弹窗。保存本身不发起收费请求。</small>
        {setupError && <p role="alert">{setupError}</p>}
        <button type="button" onClick={() => void save()} disabled={!key.trim() || saving}>
          {saving ? '正在保存…' : '安全保存'}
        </button>
      </section>
    )
  }

  if (props.state.status === 'generating') {
    return (
      <section className="visual-generation-card" aria-live="polite">
        <strong>正在生成唯一 3D 结果</strong>
        <p>{props.state.detail}</p>
        <button type="button" onClick={props.onCancel}>取消</button>
      </section>
    )
  }

  if (props.state.status === 'recoverable') {
    return (
      <section className="visual-generation-card" aria-live="polite">
        <strong>发现未完成的远程任务</strong>
        <p>{props.state.detail}</p>
        <div className="visual-generation-card__actions">
          <button type="button" onClick={props.onResume}>继续任务</button>
          <button type="button" onClick={props.onCancel}>取消远程任务</button>
        </div>
      </section>
    )
  }

  if (props.state.status === 'failed') {
    return (
      <section className="visual-generation-card" role="alert">
        <strong>视觉生成未完成</strong>
        <p>{props.state.detail}</p>
        <button type="button" onClick={props.onRetry}>重试</button>
      </section>
    )
  }

  return (
    <section className="visual-generation-card">
      <strong>唯一神经 3D 候选已生成</strong>
      <p>{props.state.result.brief.visual_summary}</p>
      <dl>
        <div><dt>三角形</dt><dd>{props.state.result.inspection.triangle_count.toLocaleString()}</dd></div>
        <div><dt>网格</dt><dd>{props.state.result.inspection.mesh_count}</dd></div>
        <div><dt>材质</dt><dd>{props.state.result.inspection.material_count}</dd></div>
        <div><dt>PBR 通道</dt><dd>{props.state.result.inspection.pbr_channels.length}</dd></div>
      </dl>
      <small>当前只是通过 Rust 结构 readback 的未保存候选；八视角视觉门与正式资产包仍需完成。</small>
    </section>
  )
}
