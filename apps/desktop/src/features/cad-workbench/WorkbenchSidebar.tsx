import type { AgentThreadSummary, ConceptProjectSummary } from '../../shared/types'
import { displayPartRole } from './partRoleLabels.js'
import { F026Icon } from './F026Icon.js'

type SidebarPart = {
  part_id: string
  role: string
  material_zone_ids: string[]
}

/**
 * F026's left rail is a pure projection of Rust-owned project, thread and
 * AssemblyGraph facts.  It intentionally owns no selection or persistence
 * state: callers must resolve every click through the existing Snapshot/API
 * lifecycle.
 */
export type WorkbenchSidebarProps = {
  projects: readonly ConceptProjectSummary[]
  activeProjectId: string | null
  threads: readonly AgentThreadSummary[]
  activeThreadId: string | null
  parts: readonly SidebarPart[]
  selectedPartId: string | null
  loading?: boolean
  onCreateProject: () => void
  onSelectProject: (projectId: string) => void
  onSelectThread: (threadId: string) => void
  onSelectPart: (partId: string) => void
}

export function WorkbenchSidebar({
  projects,
  activeProjectId,
  threads,
  activeThreadId,
  parts,
  selectedPartId,
  loading = false,
  onCreateProject,
  onSelectProject,
  onSelectThread,
  onSelectPart,
}: WorkbenchSidebarProps) {
  return (
    <aside className="f026-sidebar" aria-label="项目、对话记录与组件库">
      <div className="f026-sidebar-heading">
        <span>工作区</span>
        <button type="button" onClick={onCreateProject} disabled={loading} aria-label="新建设计">
          <F026Icon name="add" />
          <span>新建设计</span>
        </button>
      </div>

      <section className="f026-sidebar-section" aria-labelledby="f026-projects-heading">
        <div className="f026-sidebar-section-heading">
          <F026Icon name="project" />
          <span id="f026-projects-heading">项目</span>
        </div>
        <div className="f026-sidebar-list" aria-label="项目列表">
          {projects.length === 0 ? (
            <p className="f026-sidebar-empty">还没有设计项目。</p>
          ) : projects.map((project) => (
            <button
              key={project.project_id}
              type="button"
              className={project.project_id === activeProjectId ? 'active' : ''}
              aria-pressed={project.project_id === activeProjectId}
              onClick={() => onSelectProject(project.project_id)}
              disabled={loading}
            >
              <strong>{project.name}</strong>
              <small>{project.status}</small>
            </button>
          ))}
        </div>
      </section>

      <section className="f026-sidebar-section" aria-labelledby="f026-threads-heading">
        <div className="f026-sidebar-section-heading">
          <F026Icon name="thread" />
          <span id="f026-threads-heading">对话记录</span>
        </div>
        <div className="f026-sidebar-list" aria-label="对话记录">
          {threads.length === 0 ? (
            <p className="f026-sidebar-empty">当前项目还没有对话记录。</p>
          ) : threads.map((thread) => (
            <button
              key={thread.thread_id}
              type="button"
              className={thread.thread_id === activeThreadId ? 'active' : ''}
              aria-pressed={thread.thread_id === activeThreadId}
              onClick={() => onSelectThread(thread.thread_id)}
              disabled={loading}
            >
              <strong>{thread.title}</strong>
              <small>{thread.summary || thread.status}</small>
            </button>
          ))}
        </div>
      </section>

      <section className="f026-sidebar-section" aria-labelledby="f026-components-heading">
        <div className="f026-sidebar-section-heading">
          <F026Icon name="components" />
          <span id="f026-components-heading">组件库</span>
          <small>{parts.length}</small>
        </div>
        <div className="f026-sidebar-list" aria-label="当前设计组件">
          {parts.length === 0 ? (
            <p className="f026-sidebar-empty">生成模型后会显示可编辑组件。</p>
          ) : parts.map((part) => (
            <button
              key={part.part_id}
              type="button"
              className={part.part_id === selectedPartId ? 'active' : ''}
              aria-pressed={part.part_id === selectedPartId}
              onClick={() => onSelectPart(part.part_id)}
              disabled={loading}
            >
              <strong><F026Icon name="part" /> {displayPartRole(part.role)}</strong>
              <small>{part.material_zone_ids.length} 个材质区</small>
            </button>
          ))}
        </div>
      </section>
    </aside>
  )
}
