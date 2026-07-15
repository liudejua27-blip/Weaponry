import { Check, WarningCircle } from '@phosphor-icons/react'
import type { RefObject } from 'react'
import type { AgentAssetQualityReport, AgentAssetChangeSet, AgentAssetVersion, QualityFinding } from '../../shared/types'

export type QualityDrawerProps = {
  agentAssetActive: boolean
  activeAgentAssetVersion: AgentAssetVersion | null
  agentQualityReport: AgentAssetQualityReport | null
  agentAssetChangeSet: AgentAssetChangeSet | null
  graphReady: boolean
  legacyVersionReady: boolean
  legacyQualityStatus?: 'passed' | 'warning' | 'failed' | 'not_run'
  legacyFindings: QualityFinding[]
  loading: boolean
  drawerRef?: RefObject<HTMLElement | null>
  onClose: () => void
  onFocusLegacyFinding: (finding: QualityFinding) => void
  onInspectAgentAsset: () => void
  onRunLegacyInspection: () => void
}

function qualityLabel(status?: QualityDrawerProps['legacyQualityStatus'] | AgentAssetQualityReport['status']): string {
  if (status === 'passed') return '通过'
  if (status === 'warning') return '需复核'
  if (status === 'failed') return '未通过'
  return '未检查'
}

export function QualityDrawer({
  agentAssetActive,
  activeAgentAssetVersion,
  agentQualityReport,
  agentAssetChangeSet,
  graphReady,
  legacyVersionReady,
  legacyQualityStatus,
  legacyFindings,
  loading,
  drawerRef,
  onClose,
  onFocusLegacyFinding,
  onInspectAgentAsset,
  onRunLegacyInspection,
}: QualityDrawerProps) {
  const status = agentAssetActive ? agentQualityReport?.status : legacyQualityStatus
  return (
    <div className="workbench-overlay" role="presentation" onMouseDown={onClose}>
      <section ref={drawerRef} className="workbench-drawer quality-drawer" role="dialog" aria-modal="true" aria-labelledby="forgecad-quality-drawer-title" data-forgecad-drawer="quality" tabIndex={-1} onMouseDown={(event) => event.stopPropagation()}>
        <div className="drawer-heading">
          <div><span id="forgecad-quality-drawer-title">模型检查</span><strong>{qualityLabel(status)}</strong></div>
          <button type="button" data-dialog-initial-focus="true" onClick={onClose} aria-label="关闭模型检查">×</button>
        </div>
        <p>这里仅在需要时显示模型连接和几何质量信息，不会干扰设计过程。</p>
        <div className="quality-overview">
          <div className="dfm-row"><span>展示组件已加载</span><strong>{graphReady ? '已就绪' : '未准备'}</strong>{graphReady ? <Check size={15} weight="bold" /> : <WarningCircle size={15} />}</div>
          <div className="dfm-row"><span>当前版本</span><strong>{agentAssetActive ? (activeAgentAssetVersion ? '活动 Agent 资产' : '同步中') : legacyVersionReady ? '可保存' : '未创建'}</strong>{(agentAssetActive ? Boolean(activeAgentAssetVersion) : legacyVersionReady) ? <Check size={15} weight="bold" /> : <WarningCircle size={15} />}</div>
          <div className="dfm-row"><span>几何检查</span><strong>{qualityLabel(status)}</strong>{status === 'passed' ? <Check size={15} weight="bold" /> : <WarningCircle size={15} />}</div>
        </div>
        {agentAssetActive && (agentQualityReport?.findings ?? []).slice(0, 3).map((finding) => (
          <div className={`quality-finding ${finding.severity}`} key={finding.check_id}>
            <strong>{finding.check_id}</strong><span>{finding.message}</span>
            {finding.part_ids?.length ? <small>涉及部件：{finding.part_ids.join('、')}</small> : null}
          </div>
        ))}
        {!agentAssetActive && legacyFindings.slice(0, 3).map((finding) => (
          <button type="button" className={`quality-finding ${finding.severity}`} key={finding.finding_id} onClick={() => onFocusLegacyFinding(finding)}>
            <strong>{finding.check_id}</strong><span>{finding.message}</span>
          </button>
        ))}
        <button
          type="button"
          className="drawer-primary-action"
          disabled={agentAssetActive ? !activeAgentAssetVersion || Boolean(agentAssetChangeSet) : loading || !legacyVersionReady}
          onClick={agentAssetActive ? onInspectAgentAsset : onRunLegacyInspection}
        >
          {agentAssetActive ? '检查当前 Agent 资产' : loading ? '检查中…' : '运行模型检查'}
        </button>
      </section>
    </div>
  )
}
