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
    onTemplateSelect: (template) => calls.push(`template:${template}`),
    onOpenSettings: () => calls.push('settings'),
    onOpenHelp: () => calls.push('help'),
  }
  const output = WorkbenchSidebar(props)
  const rendered = text(output)
  assert(rendered.includes('我的设计') && rendered.includes('最近作品') && rendered.includes('示例') && rendered.includes('设置') && rendered.includes('帮助'), 'sidebar must contain the redesigned left rail sections')
  assert(rendered.includes('冰原探索车') && rendered.includes('写实动物外观') && rendered.includes('游戏道具外观'), 'sidebar should expose recent projects and category-open examples')
  assert(rendered.includes('设置') && rendered.includes('帮助'), 'sidebar should expose settings and help entry')
  const buttons = hostButtons(output)
  const click = (label: string) => {
    const button = buttons.find((item) => text(item).includes(label))
    assert(button, `missing button: ${label}`)
    ;(button.props as { onClick?: () => void }).onClick?.()
  }
  click('开始设计'); click('冰原探索车')
  click('设置'); click('帮助')
  assert(calls.join(',') === 'create,project:project_a,settings,help', 'sidebar must forward exposed intents through supplied callbacks')
}
