import { Check, WarningCircle } from '@phosphor-icons/react'
import type { RefObject } from 'react'
import type { AgentAssetQualityReport, AgentAssetChangeSet, AgentAssetVersion } from '../../shared/types'

export type QualityDrawerProps = {
  activeAgentAssetVersion: AgentAssetVersion | null
  agentQualityReport: AgentAssetQualityReport | null
  agentAssetChangeSet: AgentAssetChangeSet | null
  drawerRef?: RefObject<HTMLElement | null>
  onClose: () => void
  onInspectAgentAsset: () => void
}

function qualityLabel(status?: AgentAssetQualityReport['status']): string {
  if (status === 'passed') return '通过'
  if (status === 'warning') return '需复核'
  if (status === 'failed') return '未通过'
  return '未检查'
}

export function QualityDrawer({
  activeAgentAssetVersion,
  agentQualityReport,
  agentAssetChangeSet,
  drawerRef,
  onClose,
  onInspectAgentAsset,
}: QualityDrawerProps) {
  const status = agentQualityReport?.status
  return (
    <div className="workbench-overlay" role="presentation" onMouseDown={onClose}>
      <section ref={drawerRef} className="workbench-drawer quality-drawer" role="dialog" aria-modal="true" aria-labelledby="forgecad-quality-drawer-title" data-forgecad-drawer="quality" tabIndex={-1} onMouseDown={(event) => event.stopPropagation()}>
        <div className="drawer-heading">
          <div><span id="forgecad-quality-drawer-title">模型检查</span><strong>{qualityLabel(status)}</strong></div>
          <button type="button" data-dialog-initial-focus="true" onClick={onClose} aria-label="关闭模型检查">×</button>
        </div>
        <p>这里仅在需要时显示模型连接和几何质量信息，不会干扰设计过程。</p>
        <div className="quality-overview">
          <div className="dfm-row"><span>当前版本</span><strong>{activeAgentAssetVersion ? '活动 Agent 资产' : '未准备'}</strong>{activeAgentAssetVersion ? <Check size={15} weight="bold" /> : <WarningCircle size={15} />}</div>
          <div className="dfm-row"><span>几何检查</span><strong>{qualityLabel(status)}</strong>{status === 'passed' ? <Check size={15} weight="bold" /> : <WarningCircle size={15} />}</div>
        </div>
        {(agentQualityReport?.findings ?? []).slice(0, 3).map((finding) => (
          <div className={`quality-finding ${finding.severity}`} key={finding.check_id}>
            <strong>{finding.check_id}</strong><span>{finding.message}</span>
            {finding.part_ids?.length ? <small>涉及部件：{finding.part_ids.join('、')}</small> : null}
          </div>
        ))}
        <button
          type="button"
          className="drawer-primary-action"
          disabled={!activeAgentAssetVersion || Boolean(agentAssetChangeSet)}
          onClick={onInspectAgentAsset}
        >
          检查当前 Agent 资产
        </button>
      </section>
    </div>
  )
}
