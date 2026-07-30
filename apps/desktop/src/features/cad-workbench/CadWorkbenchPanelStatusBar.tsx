import type { WorkbenchStatusBarPresentation } from './workbenchStatusBarPresentation'
import type { ReactElement } from 'react'

type CadWorkbenchPanelStatusBarProps = {
  workbenchStatusBar: WorkbenchStatusBarPresentation
  showCompactSidebar: boolean
}

const BEGINNER_MODE_HINT = '零基础模式：先输入一句话开始生成，再打开高级设置补充参数。'

export function CadWorkbenchPanelStatusBar({
  workbenchStatusBar,
  showCompactSidebar,
}: CadWorkbenchPanelStatusBarProps): ReactElement {
  const statusBarClassName = showCompactSidebar
    ? 'cad-status-bar is-beginner'
    : 'cad-status-bar'

  return (
    <footer className={statusBarClassName} role="status" aria-live="polite" aria-label="工作台状态">
      {showCompactSidebar ? (
        <>
          <span>{BEGINNER_MODE_HINT}</span>
          <span className="status-spacer" />
          <span>{workbenchStatusBar.assistantStateText}</span>
        </>
      ) : (
        <>
          <span>{workbenchStatusBar.assistantStateText}</span>
          <span>{workbenchStatusBar.assetStateText}</span>
          <span>版本：{workbenchStatusBar.versionText}</span>
          <span>单位：mm</span>
          <span className="status-spacer" />
          <span>{workbenchStatusBar.qualityText}</span>
        </>
      )}
    </footer>
  )
}
