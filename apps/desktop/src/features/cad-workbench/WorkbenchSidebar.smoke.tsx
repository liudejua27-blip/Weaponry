import { isValidElement, type ReactElement, type ReactNode } from 'react'
import type { WorkbenchSidebarProps } from './WorkbenchSidebar.js'
import { WorkbenchSidebar } from './WorkbenchSidebar.js'

function assert(value: unknown, message: string): asserts value { if (!value) throw new Error(message) }
function text(node: ReactNode): string {
  if (node === null || node === undefined || typeof node === 'boolean') return ''
  if (typeof node === 'string' || typeof node === 'number') return String(node)
  if (Array.isArray(node)) return node.map(text).join(' ')
  if (!isValidElement(node)) return ''
  if (typeof node.type === 'function') return text((node.type as (props: unknown) => ReactNode)(node.props))
  return text((node.props as { children?: ReactNode }).children)
}
function hostButtons(node: ReactNode): ReactElement[] {
  if (node === null || node === undefined || typeof node === 'boolean') return []
  if (Array.isArray(node)) return node.flatMap(hostButtons)
  if (!isValidElement(node)) return []
  if (typeof node.type === 'function') return hostButtons((node.type as (props: unknown) => ReactNode)(node.props))
  return (node.type === 'button' ? [node] : []).concat(hostButtons((node.props as { children?: ReactNode }).children))
}

export function runWorkbenchSidebarSmoke(): void {
  const calls: string[] = []
  const props: WorkbenchSidebarProps = {
    projects: [{ project_id: 'project_a', profile_id: 'profile', domain_type: 'vehicle_concept', name: '冰原探索车', status: 'active', created_at: '2026-07-18T00:00:00Z', updated_at: '2026-07-18T00:00:00Z' }],
    activeProjectId: 'project_a',
    threads: [{ thread_id: 'thread_a', project_id: 'project_a', title: '探索车外观', status: 'idle', summary: '已生成当前模型。', provider_id: 'deterministic_rules', created_at: '2026-07-18T00:00:00Z', updated_at: '2026-07-18T00:00:00Z' }],
    activeThreadId: 'thread_a',
    parts: [{ part_id: 'part_body', role: 'body_shell', material_zone_ids: ['zone_body'] }],
    selectedPartId: 'part_body',
    onCreateProject: () => calls.push('create'),
    onSelectProject: (id) => calls.push(`project:${id}`),
    onSelectThread: (id) => calls.push(`thread:${id}`),
    onSelectPart: (id) => calls.push(`part:${id}`),
  }
  const output = WorkbenchSidebar(props)
  const rendered = text(output)
  assert(rendered.includes('新建设计') && rendered.includes('项目') && rendered.includes('对话记录') && rendered.includes('组件库'), 'sidebar must contain F026 left-rail sections')
  assert(rendered.includes('冰原探索车') && rendered.includes('探索车外观') && rendered.includes('车身外壳'), 'sidebar must project supplied project, thread and part facts')
  const buttons = hostButtons(output)
  const click = (label: string) => {
    const button = buttons.find((item) => text(item).includes(label))
    assert(button, `missing button: ${label}`)
    ;(button.props as { onClick?: () => void }).onClick?.()
  }
  click('新建设计'); click('冰原探索车'); click('探索车外观'); click('车身外壳')
  assert(calls.join(',') === 'create,project:project_a,thread:thread_a,part:part_body', 'sidebar must forward each user intent through supplied callbacks')
}
