export type View = 'forge' | 'patch' | 'library' | 'jobs' | 'settings'

export type AppRoute =
  | { kind: 'none' }
  | { kind: 'view'; view: View }
  | { kind: 'job'; jobId: string }
  | { kind: 'weapon'; weaponId: string; versionId?: string }

export function parseHashRoute(): AppRoute {
  if (typeof window === 'undefined') return { kind: 'none' }
  const raw = window.location.hash.replace(/^#\/?/, '')
  const parts = raw.split('/').filter(Boolean).map((part) => {
    try {
      return decodeURIComponent(part)
    } catch {
      return part
    }
  })
  const [first, second, third, fourth] = parts
  if (first === 'jobs' && second) return { kind: 'job', jobId: second }
  if (first === 'weapons' && second) {
    if (third === 'versions' && fourth) {
      return { kind: 'weapon', weaponId: second, versionId: fourth }
    }
    return { kind: 'weapon', weaponId: second }
  }
  if (isView(first)) return { kind: 'view', view: first }
  return { kind: 'none' }
}

export function writeHashRoute(route: AppRoute): void {
  if (typeof window === 'undefined') return
  const next = routeKey(route) || '#/forge'
  if (window.location.hash !== next) window.location.hash = next
}

export function routeKey(route: AppRoute): string {
  switch (route.kind) {
    case 'view':
      return `#/${route.view}`
    case 'job':
      return `#/jobs/${encodeURIComponent(route.jobId)}`
    case 'weapon':
      return route.versionId
        ? `#/weapons/${encodeURIComponent(route.weaponId)}/versions/${encodeURIComponent(route.versionId)}`
        : `#/weapons/${encodeURIComponent(route.weaponId)}`
    default:
      return ''
  }
}

export function routeView(route: AppRoute): View | null {
  if (route.kind === 'view') return route.view
  if (route.kind === 'job') return 'jobs'
  if (route.kind === 'weapon') return 'library'
  return null
}

export function routeHasResource(route: AppRoute): boolean {
  return route.kind === 'job' || route.kind === 'weapon'
}

function isView(value: string | undefined): value is View {
  return value === 'forge'
    || value === 'patch'
    || value === 'library'
    || value === 'jobs'
    || value === 'settings'
}
