import type { AgentPartEditOperation, ResolvedSemanticProportionOptions } from '../../shared/types.js'

export type AgentSemanticProportionControlsProps = {
  semanticProportions: ResolvedSemanticProportionOptions | null
  isLocked: boolean
  hasPendingChange: boolean
  loading: boolean
  onPreviewEdit: (operation: AgentPartEditOperation, summary: string) => void | Promise<void>
}

function operationId(recipeId: string): string {
  return `op_${recipeId.replace(/^proportion_/, 'ratio_')}_${Date.now().toString(36)}`
}

export function AgentSemanticProportionControls({
  semanticProportions,
  isLocked,
  hasPendingChange,
  loading,
  onPreviewEdit,
}: AgentSemanticProportionControlsProps) {
  const options = semanticProportions?.options ?? []
  return (
    <div className="agent-semantic-proportions" aria-label="外观比例配方">
      <div className="assistant-directions-heading">
        <span>外观比例配方</span>
        <small>领域语义 · 受限参数</small>
      </div>
      {loading && !semanticProportions && <small>正在读取真实模型的可用比例…</small>}
      {!loading && options.length === 0 && (
        <small>{semanticProportions?.unavailable_message ?? '当前部件没有可用的外观比例配方。'}</small>
      )}
      {options.length > 0 && (
        <div className="agent-semantic-proportion-list">
          {options.map((option) => (
            <button
              key={option.recipe_id}
              type="button"
              aria-label={`${option.display_name}；${option.description}`}
              title={`${option.description}；比例范围 ${option.min}–${option.max}，步长 ${option.step}`}
              disabled={isLocked || hasPendingChange}
              onClick={() => void onPreviewEdit({
                operation_id: operationId(option.recipe_id),
                op: 'set_part_parameter',
                part_id: semanticProportions!.part_id,
                path: option.path,
                value: option.target_value,
              }, `应用「${option.display_name}」外观比例配方`)}
            >
              <strong>{option.display_name}</strong>
              <small>{option.style_token.display_name} · {option.current_value} → {option.target_value} 倍</small>
            </button>
          ))}
        </div>
      )}
      <small className="agent-component-compatibility-note">只改变外观比例，不代表尺寸、结构、性能或制造结论。</small>
    </div>
  )
}
