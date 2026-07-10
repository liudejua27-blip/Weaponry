import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  parseHashRoute,
  routeView,
  writeHashRoute,
  type AppRoute,
  type View,
} from './routing'

export function useAppRouting() {
  const initialRoute = useMemo(() => parseHashRoute(), [])
  const [view, setView] = useState<View>(() => routeView(initialRoute) ?? 'forge')
  const [pendingRoute, setPendingRoute] = useState<AppRoute>(initialRoute)

  useEffect(() => {
    const syncRoute = () => {
      const route = parseHashRoute()
      setPendingRoute(route)
      const nextView = routeView(route)
      if (nextView) setView(nextView)
    }
    window.addEventListener('hashchange', syncRoute)
    return () => window.removeEventListener('hashchange', syncRoute)
  }, [])

  const navigateToView = useCallback((nextView: View) => {
    writeHashRoute({ kind: 'view', view: nextView })
    setView(nextView)
  }, [])

  return {
    view,
    setView,
    pendingRoute,
    navigateToView,
  }
}
