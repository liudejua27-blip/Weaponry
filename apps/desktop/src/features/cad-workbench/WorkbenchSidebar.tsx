import { ArrowRight, Cube } from '@phosphor-icons/react'
import type { ReactNode } from 'react'
import type { AgentThreadSummary, ConceptProjectSummary } from '../../shared/types'
import { F026Icon } from './F026Icon.js'

export type WorkbenchSidebarPart = {
  part_id: string
  role: string
  material_zone_ids: readonly string[]
}

export type WorkbenchSidebarProps = {
  projects: readonly ConceptProjectSummary[]
  activeProjectId: string | null
  threads?: readonly AgentThreadSummary[]
  activeThreadId?: string | null
  parts?: readonly WorkbenchSidebarPart[]
  selectedPartId?: string | null
  loading?: boolean
  onCreateProject: () => void
  onSelectProject: (projectId: string) => void
  onSelectThread?: (threadId: string) => void
  onSelectPart?: (partId: string) => void
  onTemplateSelect?: (template: string) => void
  onUploadReference?: () => void
  onOpenFromTemplatePrompt?: (template: string) => void
  onOpenSettings?: () => void
  onOpenHelp?: () => void
  compactMode?: boolean
  onToggle?: () => void
  onCollapse?: () => void
}

const TEMPLATE_LIST = [
  '写实动物外观',
  '角色与生物',
  '家具与产品',
  '建筑与环境',
  '游戏道具外观',
  '混合对象',
]

function pickProjectTitle(project: { name: string }) {
  return project.name || '未命名项目'
}

function formatRelativeTime(value: string) {
  const now = new Date()
  const valueDate = new Date(value)
  if (Number.isNaN(valueDate.getTime())) return '刚刚'

  const sameDay = (left: Date, right: Date): boolean => (
    left.getFullYear() === right.getFullYear()
    && left.getMonth() === right.getMonth()
    && left.getDate() === right.getDate()
  )
  const formatClock = (target: Date): string => target.toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
  })

  const diff = now.getTime() - valueDate.getTime()
  if (diff <= 0) return '刚刚'

  if (sameDay(now, valueDate)) {
    return `今天 ${formatClock(valueDate)}`
  }

  const diffHours = Math.floor(diff / (1000 * 60 * 60))
  if (diffHours < 1) {
    return `${Math.max(1, Math.floor(diff / (1000 * 60)))} 分钟前`
  }
  if (diffHours < 24) {
    return `今天 ${formatClock(valueDate)} · ${Math.max(1, diffHours)} 小时前`
  }
  if (diffHours < 48) {
    return `昨天 ${formatClock(valueDate)}`
  }

  const diffDays = Math.floor(diffHours / 24)
  if (diffDays < 7) {
    return `${diffDays} 天前`
  }

  return `${valueDate.getMonth() + 1}-${String(valueDate.getDate()).padStart(2, '0')} ${formatClock(valueDate)}`
}

function ProjectItem({
  project,
  isActive,
  className,
  onChoose,
  disabled,
}: {
  project: ConceptProjectSummary
  isActive: boolean
  className?: string
  onChoose: () => void
  disabled: boolean
}) {
  return (
    <button
      type="button"
      className={(
        ['f026-sidebar-project-button', className, isActive ? 'active' : null]
          .filter(Boolean) as string[]
      ).join(' ')}
      aria-pressed={isActive}
      onClick={onChoose}
      disabled={disabled}
      aria-label={`选择 ${pickProjectTitle(project)}`}
    >
      <span
        className="f026-sidebar-project-preview"
        data-project-domain={project.domain_type}
        aria-hidden="true"
      >
        <Cube size={18} weight="duotone" />
      </span>
      <span className="f026-sidebar-project-copy">
        <strong className="f026-sidebar-project-title">{pickProjectTitle(project)}</strong>
        <small className="f026-sidebar-project-meta">更新：{formatRelativeTime(project.updated_at)} · 点击打开</small>
      </span>
      {isActive ? <span className="f026-sidebar-project-arrow" aria-hidden="true"><ArrowRight size={13} /></span> : null}
    </button>
  )
}

function QuickActionButton({
  onClick,
  disabled,
  children,
}: {
  onClick: () => void
  disabled?: boolean
  children: ReactNode
}) {
  return (
    <button
      type="button"
      className="f026-sidebar-shortcut"
      onClick={onClick}
      disabled={disabled}
    >
      {children}
    </button>
  )
}

export function WorkbenchSidebar({
  projects,
  activeProjectId,
  loading = false,
  onCreateProject,
  onSelectProject,
  onTemplateSelect,
  onOpenFromTemplatePrompt,
  onOpenSettings,
  onOpenHelp,
  compactMode = false,
  onToggle,
  onCollapse,
}: WorkbenchSidebarProps) {
  const showTemplateTools = typeof onTemplateSelect === 'function' || typeof onOpenFromTemplatePrompt === 'function'
  const showSettingLinks = typeof onOpenSettings === 'function' || typeof onOpenHelp === 'function'

  const recentProjects = [...projects].sort((left, right) => (
    new Date(right.updated_at).getTime() - new Date(left.updated_at).getTime()
  ))
  return (
    <aside className={`f026-sidebar ${compactMode ? 'f026-compact' : ''}`} aria-label="我的设计">
      <div className="f026-sidebar-heading">
        <div>
          <span className="forge-sidebar-logo" aria-hidden="true">✦</span>
          <h2>我的设计</h2>
          {onCollapse ? (
            <button
              type="button"
              className="f026-sidebar-collapse"
              onClick={onCollapse}
              aria-label="收起我的设计侧栏"
              title="收起我的设计侧栏"
            >
              <ArrowRight size={14} aria-hidden="true" />
            </button>
          ) : null}
        </div>
        <button
          type="button"
          className="f026-sidebar-create"
          onClick={onCreateProject}
          disabled={loading}
          aria-label="新建设计"
        >
          <span aria-hidden="true">+ 新建设计</span>
          <span className="visually-hidden">开始设计</span>
        </button>
      </div>

      <details className={`f026-sidebar-section f026-sidebar-foldout ${compactMode ? '' : 'is-open'}`} open={!compactMode}>
        <summary className="f026-sidebar-section-heading">
          <span id="f026-projects-heading">最近作品</span>
          <small>{recentProjects.length}</small>
        </summary>
        <div className="f026-sidebar-foldout-body">
          <div className="f026-sidebar-list" aria-label="项目列表">
          {recentProjects.length === 0 ? (
          <p className="f026-sidebar-empty">还没有项目，先点“开始设计”创建第一个。</p>
          ) : recentProjects.slice(0, 12).map((project) => (
            <ProjectItem
              key={project.project_id}
              project={project}
              isActive={project.project_id === activeProjectId}
              disabled={loading}
              onChoose={() => {
                onSelectProject(project.project_id)
                onToggle?.()
              }}
              className="f026-sidebar-project-item"
            />
          ))}
          </div>
        </div>
      </details>

      {showTemplateTools ? (
        <details className={`f026-sidebar-section f026-sidebar-foldout ${compactMode ? '' : 'is-open'}`} open={!compactMode}>
          <summary className="f026-sidebar-section-heading">
            <F026Icon name="part" />
            <span id="f026-template-heading">示例</span>
            <small>{TEMPLATE_LIST.length}</small>
          </summary>
          <div className="f026-sidebar-foldout-body">
            <div className="f026-sidebar-list" aria-label="模板列表">
              {TEMPLATE_LIST.map((template) => (
                <button
                  key={template}
                  type="button"
                  disabled={loading}
                  onClick={() => {
                    onTemplateSelect?.(template)
                    onOpenFromTemplatePrompt?.(template)
                    onToggle?.()
                  }}
                  className="f026-sidebar-template-item"
                >
                  <Cube size={14} aria-hidden="true" />
                  <span>{template}</span>
                  <ArrowRight size={12} aria-hidden="true" className="f026-sidebar-template-arrow" />
                  </button>
              ))}
            </div>
          </div>
        </details>
      ) : null}

      {showSettingLinks ? (
        <section className="f026-sidebar-section f026-sidebar-footer">
        <div className="f026-sidebar-section-heading">
          <F026Icon name="edit" />
          <span>设置</span>
        </div>
        <div className="f026-sidebar-shortcuts">
            <QuickActionButton
              onClick={() => {
              onOpenSettings?.()
                onToggle?.()
              }}
              disabled={loading || typeof onOpenSettings !== 'function'}
            >
              <F026Icon name="style" />
              <span>设置</span>
            </QuickActionButton>
            <QuickActionButton
              onClick={() => {
                onOpenHelp?.()
                onToggle?.()
              }}
              disabled={typeof onOpenHelp !== 'function'}
            >
              <F026Icon name="reference" />
              <span>帮助</span>
            </QuickActionButton>
          </div>
        </section>
      ) : null}
    </aside>
  )
}
