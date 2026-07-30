import { memo } from 'react'
import { AgentSelectionCard, type AgentSelectionCardProps } from './AgentSelectionCard'
import { CadWorkbenchPanelMaterialOptions } from './cadWorkbenchPanelMaterialOptions'

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
      <details className="f026-result-details" open={expandResultDetails}>
        <summary>组件与继续编辑</summary>
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
