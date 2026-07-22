import { isValidElement, type ReactElement, type ReactNode } from 'react'
import { WorkbenchComposer, type WorkbenchComposerProps } from './WorkbenchComposer.js'

function assert(value: unknown, message: string): asserts value { if (!value) throw new Error(message) }
function text(node: ReactNode): string {
  if (node === null || node === undefined || typeof node === 'boolean') return ''
  if (typeof node === 'string' || typeof node === 'number') return String(node)
  if (Array.isArray(node)) return node.map(text).join(' ')
  if (!isValidElement(node)) return ''
  if (typeof node.type === 'function') return text((node.type as (props: unknown) => ReactNode)(node.props))
  return text((node.props as { children?: ReactNode }).children)
}
function find(node: ReactNode, predicate: (element: ReactElement) => boolean): ReactElement | undefined {
  if (node === null || node === undefined || typeof node === 'boolean') return undefined
  if (Array.isArray(node)) return node.map((child) => find(child, predicate)).find(Boolean)
  if (!isValidElement(node)) return undefined
  if (typeof node.type === 'function') return find((node.type as (props: unknown) => ReactNode)(node.props), predicate)
  return predicate(node) ? node : find((node.props as { children?: ReactNode }).children, predicate)
}

export function runWorkbenchComposerSmoke(): void {
  const calls: string[] = []
  const props: WorkbenchComposerProps = {
    value: '设计一个未来概念车',
    onChange: (value) => calls.push(`change:${value}`),
    onSend: () => calls.push('send'),
    onOpenStyle: () => calls.push('style'),
    onOpenMaterial: () => calls.push('material'),
    onOpenReference: () => calls.push('reference'),
    referenceImportCapability: 'reference_guided_rebuild',
    onOpenSurfaceAdornment: () => calls.push('adornment'),
    surfaceAdornmentDisabled: false,
  }
  const output = WorkbenchComposer(props)
  assert(text(output).includes('选择风格') && text(output).includes('选择材质') && text(output).includes('参考图 / GLB') && text(output).includes('添加外观细节'), 'plus menu must contain implemented F026 attachment actions')
  assert(text(output).includes('参考图与 GLB 可用于引导重建。'), 'reference entry must disclose the implemented R007 boundary')
  const textarea = find(output, (element) => element.type === 'textarea')
  const textareaProps = textarea?.props as { 'aria-label'?: string; placeholder?: string; onChange?: (event: { target: { value: string } }) => void; onKeyDown?: (event: { key: string; shiftKey: boolean; preventDefault: () => void }) => void } | undefined
  assert(textareaProps?.placeholder === '描述你想设计的 3D 概念模型…', 'composer must expose the natural-language input')
  textareaProps.onChange?.({ target: { value: '修改后的描述' } })
  assert(textareaProps?.['aria-label'] === '设计需求', 'composer must expose an accessible input label')
  textareaProps.onKeyDown?.({ key: 'Enter', shiftKey: false, preventDefault: () => calls.push('prevent') })
  assert(calls.join(',') === 'change:修改后的描述,prevent,send', 'Enter must send the non-empty request')
  const shiftCalls = [...calls]
  textareaProps.onKeyDown?.({ key: 'Enter', shiftKey: true, preventDefault: () => calls.push('unexpected') })
  assert(calls.join(',') === shiftCalls.join(','), 'Shift+Enter must preserve textarea newline behavior')
  const plus = find(output, (element) => (element.props as { 'aria-label'?: string })['aria-label'] === '添加风格、材质或参考')
  assert(plus, 'composer must provide a plus menu trigger')
  assert(plus.type === 'summary', 'composer must use a native details menu so it remains functional without duplicate state')
  const plusProps = plus.props as { 'aria-haspopup'?: string; 'aria-expanded'?: boolean; 'aria-controls'?: string }
  assert(plusProps['aria-haspopup'] === 'menu' && plusProps['aria-expanded'] === false && plusProps['aria-controls'] === 'f026-composer-actions', 'plus trigger must expose its menu relationship and expansion state')
  const menu = find(output, (element) => (element.props as { role?: string }).role === 'menu')
  assert(menu && typeof (menu.props as { onKeyDown?: unknown }).onKeyDown === 'function', 'plus menu must own keyboard navigation and Escape handling')
  const clickMenuItem = (label: string) => {
    const item = find(output, (element) => element.type === 'button' && text(element).includes(label))
    assert(item, `missing menu action: ${label}`)
    ;(item.props as { onClick?: (event: { currentTarget: { closest: () => null } }) => void }).onClick?.({ currentTarget: { closest: () => null } })
  }
  clickMenuItem('选择风格'); clickMenuItem('选择材质'); clickMenuItem('参考图 / GLB'); clickMenuItem('添加外观细节')
  assert(calls.join(',') === 'change:修改后的描述,prevent,send,style,material,reference,adornment', 'composer must forward input, send, and plus-menu actions')
}
