import type { AgentAssetVersion, AgentPartEditOperation, EditableParameterBinding } from '../../shared/types'

type AgentPart = AgentAssetVersion['parts'][number]

export type AgentParameterControlsProps = {
  agentAssetVersion: AgentAssetVersion
  selectedPart: AgentPart
  isLocked: boolean
  hasPendingChange: boolean
  onPreviewEdit: (operation: AgentPartEditOperation, summary: string) => void | Promise<void>
}

type TransformAxis = 'x' | 'y' | 'z'
type TransformKind = 'position' | 'scale'

const AXIS_INDEX: Record<TransformAxis, number> = { x: 0, y: 1, z: 2 }

function operationId(parameterId: string): string {
  return `op_parameter_${parameterId}_${Date.now().toString(36)}`
}

function bindingTarget(path: EditableParameterBinding['path']): { kind: TransformKind; axis: TransformAxis } {
  const [, kind, axis] = path.split('.') as ['transform', TransformKind, TransformAxis]
  return { kind, axis }
}

function currentBindingValue(
  assetVersion: AgentAssetVersion,
  partId: string,
  binding: EditableParameterBinding,
): number {
  const graph = assetVersion.assembly_graph
  const parts = Array.isArray(graph.parts) ? graph.parts : []
  const part = parts.find((item): item is Record<string, unknown> => (
    typeof item === 'object' && item !== null && item.part_id === partId
  ))
  const transform = part?.transform
  if (typeof transform !== 'object' || transform === null) return binding.default
  const { kind, axis } = bindingTarget(binding.path)
  const values = (transform as Record<string, unknown>)[kind]
  const value = Array.isArray(values) ? values[AXIS_INDEX[axis]] : undefined
  return typeof value === 'number' && Number.isFinite(value) ? value : binding.default
}

function nextBindingValue(binding: EditableParameterBinding, current: number, direction: -1 | 1): number | null {
  const precision = Math.max(decimalPlaces(binding.step), decimalPlaces(binding.min), decimalPlaces(binding.max), 6)
  const candidate = Number((current + direction * binding.step).toFixed(precision))
  if (candidate < binding.min - 1e-9 || candidate > binding.max + 1e-9) return null
  return candidate
}

function decimalPlaces(value: number): number {
  const fraction = String(value).split('.')[1]
  return fraction?.length ?? 0
}

function formatValue(value: number): string {
  return Number.isInteger(value) ? String(value) : String(Number(value.toFixed(4)))
}

function unitLabel(unit: EditableParameterBinding['unit']): string {
  return unit === 'ratio' ? '比例（ratio）' : 'mm'
}

function adjustmentSummary(binding: EditableParameterBinding, value: number): string {
  return `将${binding.display_name}调整为 ${formatValue(value)} ${unitLabel(binding.unit)}`
}

/**
 * Renders only server-declared, bounded parameter controls.  Values are
 * derived from the immutable active AssetVersion (or its declared default),
 * so no local draft can become a second source of design truth.
 */
export function AgentParameterControls({
  agentAssetVersion,
  selectedPart,
  isLocked,
  hasPendingChange,
  onPreviewEdit,
}: AgentParameterControlsProps) {
  const bindings = selectedPart.editable_parameter_bindings ?? []
  const controlsDisabled = isLocked || hasPendingChange

  if (bindings.length === 0) {
    return (
      <p className="agent-parameter-unavailable" aria-label="部件参数不可编辑">
        这个部件暂不支持单独调整比例；请选择其他部件或让 Agent 继续细化。
      </p>
    )
  }

  return (
    <section className="agent-parameter-controls" aria-label="部件可调参数">
      <div className="agent-parameter-controls-heading">
        <strong>可调参数</strong>
        <small>{isLocked ? '已锁定，不能修改' : '每次只调整一个已声明的步长'}</small>
      </div>
      {bindings.map((binding) => {
        const current = currentBindingValue(agentAssetVersion, selectedPart.part_id, binding)
        const decrease = nextBindingValue(binding, current, -1)
        const increase = nextBindingValue(binding, current, 1)
        const label = unitLabel(binding.unit)
        return (
          <div className="agent-parameter-control" key={binding.parameter_id} data-parameter-id={binding.parameter_id}>
            <div>
              <strong>{binding.display_name}</strong>
              <span>{formatValue(current)} {label}</span>
              <small>范围 {formatValue(binding.min)}–{formatValue(binding.max)} {label} · 每次 {formatValue(binding.step)} {label}</small>
            </div>
            <div className="agent-parameter-stepper">
              <button
                type="button"
                aria-label={`减小 ${binding.display_name}`}
                disabled={controlsDisabled || decrease === null}
                onClick={() => {
                  if (decrease === null) return
                  void onPreviewEdit({
                    operation_id: operationId(binding.parameter_id),
                    op: 'set_part_parameter',
                    part_id: selectedPart.part_id,
                    path: binding.path,
                    value: decrease,
                  }, adjustmentSummary(binding, decrease))
                }}
              >减少</button>
              <button
                type="button"
                aria-label={`增大 ${binding.display_name}`}
                disabled={controlsDisabled || increase === null}
                onClick={() => {
                  if (increase === null) return
                  void onPreviewEdit({
                    operation_id: operationId(binding.parameter_id),
                    op: 'set_part_parameter',
                    part_id: selectedPart.part_id,
                    path: binding.path,
                    value: increase,
                  }, adjustmentSummary(binding, increase))
                }}
              >增加</button>
            </div>
          </div>
        )
      })}
    </section>
  )
}
