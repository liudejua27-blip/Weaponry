import {
  ArrowsClockwise,
  Check,
  ClockCounterClockwise,
  Export,
  FolderOpen,
  Sparkle,
} from '@phosphor-icons/react'
import type { ReactElement } from 'react'

export type GlobalActionState = {
  canUndo: boolean
  canRedo: boolean
  canImport: boolean
  importingGlb: boolean
  importLabel: string
}

type CadWorkbenchPanelGlobalActionsProps = {
  actions: GlobalActionState
  onUndo: () => void
  onRedo: () => void
  onImport: () => void
  onCheck: () => void
  onExport: () => void
  onOpenAdvanced?: () => void
  canCheck?: boolean
  showAdvancedActions?: boolean
}

export function CadWorkbenchPanelGlobalActions({
  actions,
  onUndo,
  onRedo,
  onImport,
  onCheck,
  onExport,
  onOpenAdvanced,
  canCheck = false,
  showAdvancedActions = true,
}: CadWorkbenchPanelGlobalActionsProps): ReactElement {
  return (
    <div className="cad-global-actions" aria-label="工作区工具">
      {!showAdvancedActions && onOpenAdvanced ? (
        <button
          type="button"
          className="text-action"
          data-qa-action="open-advanced-settings"
          onClick={onOpenAdvanced}
        >
          <Sparkle size={15} /> 进阶模式
        </button>
      ) : null}
      <button type="button" className="text-action" onClick={onExport} aria-label="下载模型">
        <Export size={16} /> 下载
      </button>
      <button
        type="button"
        className="text-action"
        onClick={onImport}
        disabled={!actions.canImport || actions.importingGlb}
        title="导入参考模型"
      ><FolderOpen size={16} /> 导入参考</button>
      {showAdvancedActions && canCheck ? (
        <button
          type="button"
          className="text-action"
          onClick={onCheck}
          title="检查当前资产质量与可用性"
        ><Check size={16} /> 质量检查</button>
      ) : null}
      {showAdvancedActions ? (
        <>
          <details className="cad-global-actions-advanced" aria-label="高级工具">
            <summary className="text-action">更多工具</summary>
            <div className="cad-advanced-action-group">
              <button
                type="button"
                className="text-action"
                onClick={onUndo}
                disabled={!actions.canUndo}
                title="回到上一个模型状态"
              ><ClockCounterClockwise size={16} /> 上一步</button>
              <button
                type="button"
                className="text-action"
                onClick={onRedo}
                disabled={!actions.canRedo}
                title="恢复下一个模型状态"
              ><ArrowsClockwise size={16} /> 恢复</button>
            </div>
          </details>
        </>
      ) : null}
    </div>
  )
}
