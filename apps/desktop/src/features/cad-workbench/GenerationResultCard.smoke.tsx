import { isValidElement, type ReactElement, type ReactNode } from 'react'
import { GenerationResultCard } from './GenerationResultCard.js'

function assert(value: unknown, message: string): asserts value { if (!value) throw new Error(message) }
function text(node: ReactNode): string {
  if (node === null || node === undefined || typeof node === 'boolean') return ''
  if (typeof node === 'string' || typeof node === 'number') return String(node)
  if (Array.isArray(node)) return node.map(text).join(' ')
  if (!isValidElement(node)) return ''
  if (typeof node.type === 'function') return text((node.type as (props: unknown) => ReactNode)(node.props))
  return text((node.props as { children?: ReactNode }).children)
}
function buttons(node: ReactNode): ReactElement[] {
  if (node === null || node === undefined || typeof node === 'boolean') return []
  if (Array.isArray(node)) return node.flatMap(buttons)
  if (!isValidElement(node)) return []
  if (typeof node.type === 'function') return buttons((node.type as (props: unknown) => ReactNode)(node.props))
  return (node.type === 'button' ? [node] : []).concat(buttons((node.props as { children?: ReactNode }).children))
}

export function runGenerationResultCardSmoke(): void {
  const idle = GenerationResultCard({ state: 'idle' })
  assert(text(idle).includes('等待生成'), 'idle card must not invent a result')
  const processing = GenerationResultCard({ state: 'processing', detail: '正在检查 GLB。' })
  assert(text(processing).includes('正在生成当前模型') && text(processing).includes('正在检查 GLB。'), 'processing card must describe the current work')
  const failed = GenerationResultCard({ state: 'failed', error: 'GLB 回读失败。' })
  assert(text(failed).includes('本次生成未完成') && text(failed).includes('GLB 回读失败。'), 'failed card must expose the actual error')
  const calls: string[] = []
  const compatibility = GenerationResultCard({
    state: 'compatibility_result',
    summary: '由临时适配器构建的一个 3D 结果。',
    onContinueEditing: () => calls.push('compat-edit'),
  })
  const compatibilityText = text(compatibility)
  assert(
    compatibilityText.includes('当前临时结果')
      && compatibilityText.includes('尚未经过正式生成质量门')
      && !compatibilityText.includes('当前生成结果'),
    'compatibility result must not impersonate the V003 ready state',
  )
  const ready = GenerationResultCard({
    state: 'ready',
    summary: '紧凑探索车，已完成外观分件。',
    versionLabel: '可编辑资产 v3',
    onSave: () => calls.push('save'),
    onContinueEditing: () => calls.push('edit'),
  })
  const readyText = text(ready)
  assert(readyText.includes('当前生成结果') && readyText.includes('继续修改') && readyText.includes('保存'), 'ready card must expose the current result actions')
  assert(!/best|candidate|最佳|候选/i.test(readyText), 'single-result card must not expose candidate or ranking language')
  for (const button of buttons(ready)) {
    const label = text(button)
    ;(button.props as { onClick?: () => void }).onClick?.()
    assert(label.includes('继续修改') || label.includes('保存'), 'ready card may only expose save and continue-editing actions')
  }
  assert(calls.join(',') === 'edit,save', 'ready actions must remain callback-owned')
}
