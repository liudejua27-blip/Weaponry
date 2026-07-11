import type { ReactNode } from 'react'
import type { ServiceStatus } from './providers/RuntimeProvider'
import type { View } from './routing'

type AppShellProps = {
  view: View
  title: string
  subtitle: string
  serviceStatus: ServiceStatus
  serviceLabel: string
  onNavigate: (view: View) => void
  children: ReactNode
}

const NAV_ITEMS: Array<{ view: View; label: string }> = [
  { view: 'cad', label: 'CAD 工作台' },
  { view: 'forge', label: 'Forge 工作台' },
  { view: 'patch', label: 'Patch Mode' },
  { view: 'library', label: '资产库' },
  { view: 'jobs', label: '任务中心' },
  { view: 'settings', label: '设置' },
]

export function AppShell({
  view,
  title,
  subtitle,
  serviceStatus,
  serviceLabel,
  onNavigate,
  children,
}: AppShellProps) {
  return (
    <div className="app-shell">
      <aside className="nav-rail" aria-label="主导航">
        <div className="brand">
          <span className="brand-mark">武</span>
          <span>武神 Forge</span>
        </div>
        {NAV_ITEMS.map((item) => (
          <button
            key={item.view}
            className={view === item.view ? 'active' : ''}
            onClick={() => onNavigate(item.view)}
          >
            {item.label}
          </button>
        ))}
      </aside>

      <main className="workspace">
        <header className="top-bar">
          <div>
            <strong>{title}</strong>
            <span>{subtitle}</span>
          </div>
          <div className={`status-pill ${serviceStatus}`}>{serviceLabel}</div>
        </header>
        {children}
      </main>
    </div>
  )
}
