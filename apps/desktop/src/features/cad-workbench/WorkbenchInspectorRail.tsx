import { Cube, SlidersHorizontal } from '@phosphor-icons/react'
import type { ReactNode } from 'react'
import type {
  AgentAssetQualityReport,
  AgentAssetVersion,
  ConceptVersionDetail,
  ModuleGraphRecord,
  QualityRunRecord,
} from '../../shared/types'
import { displayPartRole } from './partRoleLabels.js'

type LegacyNode = ModuleGraphRecord['graph']['nodes'][number]

export type WorkbenchInspectorRailProps = {
  mode: 'agent' | 'legacy' | 'empty'
  agentAssetVersion: AgentAssetVersion | null
  agentQualityReport: AgentAssetQualityReport | null
  selectedAgentPartId: string | null
  materialEditor: ReactNode
  legacyDetailsOpen: boolean
  legacyVersion: ConceptVersionDetail | null
  legacyGraph: ModuleGraphRecord | null
  legacyQualityRun: QualityRunRecord | null
  selectedLegacyNode: LegacyNode | null
  onCloseLegacyDetails: () => void
  onSelectLegacyNode: (nodeId: string) => void
}

/**
 * The Agent inspector and legacy compatibility surface are mutually exclusive.
 * Legacy values are presentation-only and expose no mutation or export command.
 */
export function WorkbenchInspectorRail({
  mode,
  agentAssetVersion,
  agentQualityReport,
  selectedAgentPartId,
  materialEditor,
  legacyDetailsOpen,
  legacyVersion,
  legacyGraph,
  legacyQualityRun,
  selectedLegacyNode,
  onCloseLegacyDetails,
  onSelectLegacyNode,
}: WorkbenchInspectorRailProps) {
  if (mode === 'agent') {
    const selectedPart = agentAssetVersion?.parts.find((part) => part.part_id === selectedAgentPartId) ?? null
    return (
      <aside className="cad-right-rail" data-testid="agent-asset-inspector">
        <section className="cad-panel properties-panel">
          <div className="cad-panel-title"><span><SlidersHorizontal size={16} /> 当前 Agent 资产</span></div>
          <div className="agent-inspector-summary">
            <strong>{agentAssetVersion ? `Agent v${agentAssetVersion.version_no}` : '正在同步资产'}</strong>
            <span>{selectedPart ? `已选：${displayPartRole(selectedPart.role)}` : '从分件列表选择一个部件继续编辑'}</span>
            <small>检查：{qualityLabel(agentQualityReport?.status)} · 导出只使用当前 Snapshot 绑定的 GLB</small>
          </div>
          {materialEditor}
        </section>
      </aside>
    )
  }

  if (mode === 'legacy') {
    if (!legacyDetailsOpen) return null
    return (
      <aside className="cad-right-rail" data-testid="legacy-readonly-boundary">
        <section className="cad-panel properties-panel">
          <div className="cad-panel-title"><span><Cube size={16} /> 旧版只读兼容</span></div>
          <div className="legacy-readonly-surface" aria-label="旧版只读 Graph Inspector">
              <div className="legacy-readonly-heading">
                <div><strong>Graph Inspector · 只读</strong><span>不能修改 Agent Snapshot、质量或导出身份</span></div>
                <button type="button" onClick={onCloseLegacyDetails}>关闭</button>
              </div>
              <label className="wide-field"><span>旧版版本</span><input value={legacyVersion ? `v${legacyVersion.version_no}` : '未读取'} readOnly /></label>
              <label className="wide-field"><span>Graph 状态</span><input value={legacyGraph?.validation_status ?? '未读取'} readOnly /></label>
              <label className="wide-field"><span>Graph 节点</span><input value={String(legacyGraph?.graph.nodes.length ?? 0)} readOnly /></label>
              <div className="legacy-readonly-node-list" aria-label="旧版 Graph 节点列表">
                {(legacyGraph?.graph.nodes ?? []).map((node) => (
                  <button
                    key={node.node_id}
                    type="button"
                    className={selectedLegacyNode?.node_id === node.node_id ? 'active' : ''}
                    onClick={() => onSelectLegacyNode(node.node_id)}
                  >
                    <strong>{node.node_id}</strong><span>{node.module_id}</span>
                  </button>
                ))}
                {!legacyGraph && <span>没有读取到旧版 ModuleGraph。</span>}
              </div>
              <div className="property-divider" />
              <strong className="legacy-readonly-section-title">旧参数 · 只读</strong>
              <ReadOnlyNumber label="整体长度" value={legacyVersion?.spec.proportions.overall_length_mm} unit="mm" />
              <ReadOnlyNumber label="主体高度" value={legacyVersion?.spec.proportions.body_height_mm} unit="mm" />
              <ReadOnlyNumber label="握持角度" value={legacyVersion?.spec.proportions.grip_angle_deg} unit="°" />
              <ReadOnlyNumber label="细节密度" value={legacyVersion ? Math.round(legacyVersion.spec.style.detail_density * 100) : undefined} unit="%" />
              <div className="property-divider" />
              <strong className="legacy-readonly-section-title">历史检查 · 只读</strong>
              <span>{qualityLabel(legacyQualityRun?.report.status)}</span>
              <div className="property-divider" />
              <strong className="legacy-readonly-section-title">旧导出格式 · 兼容记录</strong>
              <span>SOURCE ZIP · OBJ · PNG · MP4</span>
              <small>此处不创建新导出。Agent 下载抽屉始终只提供当前资产的 GLB 与概念图。</small>
          </div>
        </section>
      </aside>
    )
  }

  return (
    <aside className="cad-right-rail" data-testid="empty-agent-inspector">
      <section className="cad-panel properties-panel">
        <div className="cad-panel-title"><span><SlidersHorizontal size={16} /> 当前设计</span></div>
        <div className="agent-inspector-summary">
          <strong>尚未保存 Agent 资产</strong>
          <span>完成 Brief 和预览后，可保存为受 Snapshot 管理的可编辑资产。</span>
        </div>
      </section>
    </aside>
  )
}

function ReadOnlyNumber({ label, value, unit }: { label: string; value?: number; unit: string }) {
  return (
    <label className="property-number">
      <span>{label}</span>
      <input value={typeof value === 'number' ? value : '—'} readOnly />
      <small>{unit}</small>
    </label>
  )
}

function qualityLabel(status?: 'passed' | 'warning' | 'failed' | 'not_run' | 'unavailable'): string {
  if (status === 'passed') return '通过'
  if (status === 'warning') return '需复核'
  if (status === 'failed') return '未通过'
  return '未检查'
}
