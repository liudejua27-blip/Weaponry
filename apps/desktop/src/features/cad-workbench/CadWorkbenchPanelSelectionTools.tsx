import { memo } from 'react'
import { AgentSelectionCard, type AgentSelectionCardProps } from './AgentSelectionCard'
import { CadWorkbenchPanelMaterialOptions } from './cadWorkbenchPanelMaterialOptions'
import { displayPartRole } from './partRoleLabels.js'

type CadWorkbenchPanelMaterialOptionsProps = Parameters<typeof CadWorkbenchPanelMaterialOptions>[0]

type CadWorkbenchPanelSelectionToolsProps = {
  agentSelectionCardProps: AgentSelectionCardProps | null
  materialOptionsProps: CadWorkbenchPanelMaterialOptionsProps | null
  showSelectionTools: boolean
  showMaterialOptions: boolean
  expandResultDetails: boolean
  onOpenSurfaceAdornment: () => void
}

export const CadWorkbenchPanelSelectionTools = memo(function CadWorkbenchPanelSelectionTools({
  agentSelectionCardProps,
  materialOptionsProps,
  showSelectionTools,
  showMaterialOptions,
  expandResultDetails,
  onOpenSurfaceAdornment,
}: CadWorkbenchPanelSelectionToolsProps) {
  if (!showSelectionTools) {
    return null
  }

  if (!agentSelectionCardProps) {
    return null
  }

  return (
    <>
      {agentSelectionCardProps.selectedPart ? (
        <section className="f026-selected-component-summary" aria-label="当前选中组件">
          <div>
            <span>当前选中组件</span>
            <strong>{displayPartRole(agentSelectionCardProps.selectedPart.role)}</strong>
          </div>
          <small>可继续用自然语言修改，或展开查看部件操作。</small>
        </section>
      ) : null}
      <details className="f026-result-details" open={expandResultDetails}>
        <summary>{agentSelectionCardProps.selectedPart ? '展开部件操作' : '组件与继续编辑'}</summary>
        <AgentSelectionCard
          {...agentSelectionCardProps}
          onOpenSurfaceAdornment={onOpenSurfaceAdornment}
        />
      </details>
      {showMaterialOptions && materialOptionsProps ? (
        <CadWorkbenchPanelMaterialOptions {...materialOptionsProps} />
      ) : null}
    </>
  )
})
