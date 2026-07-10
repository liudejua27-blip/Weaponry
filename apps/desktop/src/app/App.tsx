import { Suspense, lazy } from 'react'
import { LegacyWorkbench } from './LegacyWorkbench'
import { useLegacyAppController } from './useLegacyAppController'

const CadWorkbenchPanel = lazy(() => import('../features/cad-workbench/CadWorkbenchPanel').then(
  (module) => ({ default: module.CadWorkbenchPanel }),
))

export function App() {
  const controller = useLegacyAppController()

  if (controller.view === 'cad') {
    return (
      <Suspense fallback={<div className="panel-section"><p className="muted">正在加载 CAD 工作台...</p></div>}>
        <CadWorkbenchPanel onOpenLegacy={() => controller.navigateToView('forge')} />
      </Suspense>
    )
  }

  return <LegacyWorkbench controller={controller} />
}
