import { F026Icon } from './F026Icon.js'

type ReadyGenerationResultCardProps = {
  state: 'ready'
  summary: string
  versionLabel?: string
  onSave?: () => void
  onContinueEditing: () => void
}
type CompatibilityGenerationResultCardProps = {
  state: 'compatibility_result'
  summary: string
  versionLabel?: string
  onSave?: () => void
  onContinueEditing: () => void
}
type IdleGenerationResultCardProps = {
  state: 'idle'
}
type ProcessingGenerationResultCardProps = {
  state: 'processing'
  detail?: string
}
type FailedGenerationResultCardProps = {
  state: 'failed'
  error: string
  onRetry?: () => void
}

/**
 * F026's single-result card intentionally describes only the current Turn
 * output.  It has no candidate count, ranking, or "best" claim.
 */
export type GenerationResultCardProps =
  | ReadyGenerationResultCardProps
  | CompatibilityGenerationResultCardProps
  | IdleGenerationResultCardProps
  | ProcessingGenerationResultCardProps
  | FailedGenerationResultCardProps

export function GenerationResultCard(props: GenerationResultCardProps) {
  if (props.state === 'idle') {
    return (
      <section className="f026-generation-result" data-generation-state="idle" aria-label="生成结果">
        <F026Icon name="waiting" className="f026-generation-result-icon" />
        <div><strong>等待生成</strong><p>发送设计需求后，结果会显示在这里。</p></div>
      </section>
    )
  }
  if (props.state === 'processing') {
    return (
      <section className="f026-generation-result" data-generation-state="processing" aria-live="polite" aria-label="正在生成模型">
        <F026Icon name="loading" className="f026-generation-result-icon f026-spin" />
        <div><strong>正在生成当前模型</strong><p>{props.detail ?? 'Agent 正在构建并检查 3D 结果。'}</p></div>
      </section>
    )
  }
  if (props.state === 'failed') {
    return (
      <section className="f026-generation-result" data-generation-state="failed" role="alert" aria-label="生成失败">
        <F026Icon name="failure" className="f026-generation-result-icon" />
        <div><strong>本次生成未完成</strong><p>{props.error}</p></div>
        {props.onRetry && <button type="button" onClick={props.onRetry}>重试</button>}
      </section>
    )
  }
  const isCompatibilityResult = props.state === 'compatibility_result'
  return (
    <section
      className="f026-generation-result"
      data-generation-state={props.state}
      aria-label={isCompatibilityResult ? '当前临时结果' : '当前生成结果'}
    >
      <F026Icon name="success" className="f026-generation-result-icon" />
      <div>
        <strong>{isCompatibilityResult ? '当前临时结果' : '当前生成结果'}</strong>
        <p>{props.summary}</p>
        <small>{isCompatibilityResult
          ? `${props.versionLabel ?? '预览状态 · 确认前不会写入版本'} · 尚未经过正式生成质量门`
          : (props.versionLabel ?? '正式生成质量门已通过')}</small>
      </div>
      <div className="f026-generation-result-actions">
        <button type="button" onClick={props.onContinueEditing}><F026Icon name="edit" /> 继续修改</button>
        {props.onSave && <button type="button" onClick={props.onSave} aria-label="保存为可编辑模型"><F026Icon name="save" /> 确认保存</button>}
      </div>
    </section>
  )
}
