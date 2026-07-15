import type {
  AgentAssetChangeSet,
  AgentAssetVersion,
  AgentComponentCandidate,
  AgentPartEditOperation,
  ActiveDesignPartDisplay,
  AgentStructureSuggestion,
  SegmentAgentBlockoutResponse,
  ResolvedSemanticProportionOptions,
} from '../../shared/types'
import { AgentParameterControls } from './AgentParameterControls.js'
import { AgentSemanticProportionControls } from './AgentSemanticProportionControls.js'
import { displayPartRole, isJointPartRole } from './partRoleLabels.js'
import type { AgentBlockoutPreviewPresentation } from './agentBlockoutPreviewPresentation.js'

type AgentPart = AgentAssetVersion['parts'][number]
type PartDisplayAction = 'lock' | 'unlock' | 'hide' | 'show' | 'isolate' | 'clear_isolation' | 'show_all'

export type AgentSelectionCardProps = {
  segmentation: SegmentAgentBlockoutResponse
  agentAssetVersion: AgentAssetVersion | null
  activeAgentAssetVersion: AgentAssetVersion | null
  selectedPart: AgentPart | undefined
  selectedPartId: string | null
  partDisplay: ActiveDesignPartDisplay | null
  isSelectedPartLocked: boolean
  isExternalGlbReference: boolean
  isSnapshotActionPending: boolean
  agentAssetChangeSet: AgentAssetChangeSet | null
  agentComponentCandidates: AgentComponentCandidate[]
  agentStructureSuggestions: AgentStructureSuggestion[]
  structureSuggestionUnavailableMessage: string | null
  semanticProportions: ResolvedSemanticProportionOptions | null
  editAssistLoading: boolean
  blockoutPreviewPresentation: AgentBlockoutPreviewPresentation | null
  onSelectPart: (partId: string) => void | Promise<void>
  onCommitBlockout: () => void | Promise<void>
  onRegenerateBlockout: () => void | Promise<void>
  onPreviewEdit: (operation: AgentPartEditOperation, summary: string) => void | Promise<void>
  onSaveSelectedComponent: () => void | Promise<void>
  onReplaceComponent: (candidate: AgentComponentCandidate) => void | Promise<void>
  onPreviewStructureSuggestion: (suggestion: AgentStructureSuggestion) => void | Promise<void>
  onSetPartDisplay: (action: PartDisplayAction, partId?: string) => void | Promise<void>
  onInspectAsset: () => void | Promise<void>
  onRejectChange: () => void | Promise<void>
  onConfirmChange: () => void | Promise<void>
}

function operationId(prefix: string): string {
  return `${prefix}_${Date.now().toString(36)}`
}

export function AgentSelectionCard({
  segmentation,
  agentAssetVersion,
  activeAgentAssetVersion,
  selectedPart,
  selectedPartId,
  partDisplay,
  isSelectedPartLocked,
  isExternalGlbReference,
  isSnapshotActionPending,
  agentAssetChangeSet,
  agentComponentCandidates,
  agentStructureSuggestions,
  structureSuggestionUnavailableMessage,
  semanticProportions,
  editAssistLoading,
  blockoutPreviewPresentation,
  onSelectPart,
  onCommitBlockout,
  onRegenerateBlockout,
  onPreviewEdit,
  onSaveSelectedComponent,
  onReplaceComponent,
  onPreviewStructureSuggestion,
  onSetPartDisplay,
  onInspectAsset,
  onRejectChange,
  onConfirmChange,
}: AgentSelectionCardProps) {
  const eligibleComponentCandidates = agentComponentCandidates.filter((candidate) => candidate.compatibility.eligible)
  const selectedStructureSuggestions = selectedPart
    ? agentStructureSuggestions.filter((suggestion) => suggestion.part_id === selectedPart.part_id || suggestion.target_part_id === selectedPart.part_id)
    : []
  // The candidate card may render before the server-owned Snapshot finishes
  // hydrating after a restart. Keep all durable asset actions unavailable until
  // the visible asset is confirmed as the active Snapshot asset.
  const persistedActionsDisabled = Boolean(agentAssetChangeSet) || isSnapshotActionPending || !activeAgentAssetVersion
  const displayHasHiddenParts = Boolean((partDisplay?.hidden_part_ids ?? []).length)
  const isolatedPartId = partDisplay?.isolated_part_id ?? null
  return (
    <div
      className="agent-segmentation-candidates"
      aria-label="分件候选"
      aria-live="polite"
      data-agent-asset-version-id={agentAssetVersion?.asset_version_id ?? undefined}
      data-active-agent-asset-version-id={activeAgentAssetVersion?.asset_version_id ?? undefined}
      data-selected-part-id={selectedPartId ?? undefined}
      data-selected-part-available={selectedPart ? 'true' : 'false'}
      data-external-glb-reference={isExternalGlbReference ? 'true' : 'false'}
    >
      <div className="assistant-directions-heading">
        <span>分件候选</span>
        <small>{isExternalGlbReference ? `导入参考模型 v${agentAssetVersion?.version_no}` : agentAssetVersion ? `可编辑资产 v${agentAssetVersion.version_no}` : '预览状态 · 未写入版本'}</small>
      </div>
      <p>{isExternalGlbReference ? '导入模型已通过 GLB 安全检查，当前作为参考显示；请让 Agent 重建后再进行部件级编辑。' : blockoutPreviewPresentation ? `${blockoutPreviewPresentation.title} · ${blockoutPreviewPresentation.detail}` : `Agent 已按领域角色拆出 ${segmentation.parts.length} 个可编辑候选部件。`}</p>
      <div className="agent-segmentation-list">
        {segmentation.parts.map((part) => {
          const currentPart = agentAssetVersion?.parts.find((item) => item.part_id === part.part_id) ?? part
          const isVisible = !(partDisplay?.hidden_part_ids ?? []).includes(part.part_id)
            && (isolatedPartId === null || isolatedPartId === part.part_id)
          return (
          <button
            key={part.part_id}
            type="button"
            className={selectedPartId === part.part_id ? 'active' : ''}
            aria-label={`选择部件 ${displayPartRole(part.role)}`}
            aria-pressed={selectedPartId === part.part_id}
            disabled={!isVisible}
            onClick={() => void onSelectPart(part.part_id)}
          >
            <strong>{displayPartRole(part.role)}</strong>
            <small>{isVisible ? `${part.material_zone_ids.length} 个材质区 · ${currentPart.editable_parameter_bindings?.length ? '可调整' : '暂不可调'}` : isolatedPartId ? '单独查看时暂不显示' : '已隐藏'}</small>
          </button>
          )
        })}
      </div>
      {agentAssetVersion && !isExternalGlbReference && (displayHasHiddenParts || isolatedPartId) && (
        <div className="agent-part-display-summary" aria-label="部件显示状态">
          <small>{isolatedPartId ? '正在单独查看一个部件。' : `已隐藏 ${partDisplay?.hidden_part_ids?.length ?? 0} 个部件。`}</small>
          {isolatedPartId && <button type="button" onClick={() => void onSetPartDisplay('clear_isolation')} disabled={persistedActionsDisabled}>结束单独查看</button>}
          <button type="button" onClick={() => void onSetPartDisplay('show_all')} disabled={persistedActionsDisabled}>显示所有部件</button>
        </div>
      )}
      {!agentAssetVersion && (
        <div className="agent-blockout-preview-actions" aria-label="预览外观动作">
          <small>当前第 {(segmentation.variation_index ?? 0) + 1} / 3 版 · 仍是预览，不影响已保存设计</small>
          <button
            type="button"
            aria-label="换一版外观"
            onClick={() => void onRegenerateBlockout()}
            disabled={Boolean(agentAssetChangeSet)}
          >
            换一版外观
          </button>
          <button
            type="button"
            className="agent-asset-commit"
            aria-label="保存为可编辑模型"
            onClick={() => void onCommitBlockout()}
            disabled={Boolean(agentAssetChangeSet)}
          >
            保存为可编辑模型
          </button>
        </div>
      )}
      {agentAssetVersion && selectedPart && !isExternalGlbReference && (
        <div className="agent-part-actions" aria-label="部件级调整">
          <div className="assistant-directions-heading">
            <span>已选中：{displayPartRole(selectedPart.role)}</span>
            <small>{isSelectedPartLocked ? '已锁定，不能修改' : '修改只作用于此部件'}</small>
          </div>
          <div className="agent-part-action-row" aria-label="部件显示与保护">
            <button
              type="button"
              aria-label={isSelectedPartLocked ? '解除部件锁定' : '锁定部件'}
              onClick={() => void onSetPartDisplay(isSelectedPartLocked ? 'unlock' : 'lock', selectedPart.part_id)}
              disabled={persistedActionsDisabled}
            >{isSelectedPartLocked ? '解除锁定' : '锁定此部件'}</button>
            <button
              type="button"
              aria-label="隐藏当前部件"
              onClick={() => void onSetPartDisplay('hide', selectedPart.part_id)}
              disabled={persistedActionsDisabled}
            >隐藏此部件</button>
            <button
              type="button"
              aria-label={isolatedPartId === selectedPart.part_id ? '结束单独查看' : '只看当前部件'}
              onClick={() => void onSetPartDisplay(
                isolatedPartId === selectedPart.part_id ? 'clear_isolation' : 'isolate',
                isolatedPartId === selectedPart.part_id ? undefined : selectedPart.part_id,
              )}
              disabled={persistedActionsDisabled}
            >{isolatedPartId === selectedPart.part_id ? '结束单独查看' : '只看这个部件'}</button>
          </div>
          <AgentParameterControls
            agentAssetVersion={agentAssetVersion}
            selectedPart={selectedPart}
            isLocked={isSelectedPartLocked}
            hasPendingChange={persistedActionsDisabled}
            onPreviewEdit={onPreviewEdit}
          />
          <AgentSemanticProportionControls
            semanticProportions={semanticProportions}
            isLocked={isSelectedPartLocked}
            hasPendingChange={persistedActionsDisabled}
            loading={editAssistLoading}
            onPreviewEdit={onPreviewEdit}
          />
          <div className="agent-part-action-row">
            <button
              type="button"
              aria-label="换成拉丝铝"
              onClick={() => void onPreviewEdit({
                operation_id: operationId('op_aluminum'),
                op: 'apply_material_preset',
                part_id: selectedPart.part_id,
                material_id: 'mat_aluminum',
              }, '换成拉丝铝视觉材质')}
              disabled={persistedActionsDisabled || isSelectedPartLocked}
            >换成拉丝铝</button>
            {isJointPartRole(selectedPart.role) && (
              <button
                type="button"
                aria-label="关节左转 15°"
                onClick={() => void onPreviewEdit({
                  operation_id: operationId('op_joint'),
                  op: 'set_joint_pose',
                  part_id: selectedPart.part_id,
                  transform: { rotation: [0, 0, 0.26] },
                }, '关节向左转 15°')}
                disabled={persistedActionsDisabled || isSelectedPartLocked}
              >关节左转 15°</button>
            )}
          </div>
          <div className="agent-part-action-row">
            <button type="button" onClick={() => void onSaveSelectedComponent()} disabled={persistedActionsDisabled || isSelectedPartLocked} aria-label="保存为可复用部件">
              保存为可复用部件
            </button>
            {eligibleComponentCandidates.slice(0, 3).map((candidate) => (
              <span key={candidate.component.component_id} className="agent-component-candidate">
                <button
                  type="button"
                  aria-label={`替换：${candidate.component.display_name}；${componentCompatibilitySummary(candidate)}`}
                  onClick={() => void onReplaceComponent(candidate)}
                  disabled={persistedActionsDisabled || isSelectedPartLocked}
                  title={`${candidate.component.description || '来自当前项目的 Agent 部件库'}；${componentCompatibilitySummary(candidate)}`}
                >
                  替换：{candidate.component.display_name}
                </button>
                <small className="agent-component-compatibility-note">{componentCompatibilitySummary(candidate)}</small>
              </span>
            ))}
            {agentComponentCandidates.length > 0 && eligibleComponentCandidates.length === 0 && (
              <small className="agent-component-compatibility-note">当前没有可替换部件：需要同领域、同部件类型，且来源模型已通过检查。</small>
            )}
            {editAssistLoading && <small className="agent-component-compatibility-note">正在读取可替换部件…</small>}
          </div>
          <div className="agent-structure-suggestions" aria-label="拆分或合并建议">
            <small>Agent 只会依据当前已知的部件关系提出建议，不会猜测切割线或工程结构。</small>
            {selectedStructureSuggestions.map((suggestion) => (
              <span key={suggestion.suggestion_id} className="agent-component-candidate">
                <button
                  type="button"
                  aria-label={`${suggestion.kind === 'split_part' ? '预览拆分部件' : '预览合并部件'}：${suggestion.summary}`}
                  onClick={() => void onPreviewStructureSuggestion(suggestion)}
                  disabled={persistedActionsDisabled || isSelectedPartLocked}
                >
                  {suggestion.kind === 'split_part' ? '预览拆分' : '预览合并'}
                </button>
                <small className="agent-component-compatibility-note">{suggestion.summary}</small>
              </span>
            ))}
            {selectedStructureSuggestions.length === 0 && (
              <small className="agent-component-compatibility-note">{structureSuggestionUnavailableMessage ?? '当前部件暂不能建议拆分或合并。'}</small>
            )}
            {editAssistLoading && <small className="agent-component-compatibility-note">正在读取结构建议…</small>}
          </div>
        </div>
      )}
      {activeAgentAssetVersion && (
        <button type="button" className="agent-asset-quality" aria-label="检查这个模型" onClick={() => void onInspectAsset()} disabled={persistedActionsDisabled}>
          检查这个模型
        </button>
      )}
      {agentAssetChangeSet?.preview && (
        <div className="agent-asset-change-review" aria-label="可编辑资产修改预览">
          <strong>{agentAssetChangeSet.summary}</strong>
          <small>这是临时预览，确认后才创建下一个资产版本。</small>
          <div className="agent-part-action-row">
            <button type="button" onClick={() => void onRejectChange()} aria-label="取消修改">取消修改</button>
            <button type="button" className="confirm" onClick={() => void onConfirmChange()} aria-label="保留并创建新版本">保留并创建新版本</button>
          </div>
        </div>
      )}
    </div>
  )
}

function componentCompatibilitySummary(candidate: AgentComponentCandidate): string {
  const quality = candidate.compatibility.source_quality_status === 'passed'
    ? '来源检查通过'
    : candidate.compatibility.source_quality_status === 'warning'
      ? '来源检查有提示'
      : '来源模型尚未可用'
  return `${quality}；保留当前连接位置`
}
