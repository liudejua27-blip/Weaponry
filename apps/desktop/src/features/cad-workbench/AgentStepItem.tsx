import type { AgentItem, AgentItemType } from '../../shared/types'

const ITEM_TYPE_LABELS: Record<AgentItemType, string> = {
  user_message: '需求',
  assistant_message: '回复',
  plan: '理解',
  tool_call: '工具',
  tool_result: '结果',
  preview: '预览',
  approval_request: '确认',
  clarification: '确认',
  artifact: '产物',
}

const TOOL_STAGE_LABELS: Record<string, string> = {
  infer_product_domain: '识别设计对象',
  author_forge_visual_program: '生成受限设计稿',
  author_universal_asset: '理解对象并规划表示',
  patch_forge_visual_program: '应用局部修复',
  build_candidate_geometry: '构建候选模型',
  compile_readback_candidate: '核验 GLB 回读',
  render_candidate_views: '生成固定视图',
  evaluate_candidate: '执行质量与参考比对',
  prepare_candidate_preview: '准备确认前预览',
  plan_complete_concept: '整理设计方案',
}

type RecordValue = Record<string, unknown>

export type AgentProcessStep = {
  key: string
  stage: string
  tool: string | null
  status: AgentItem['status']
  inputEvidence: string | null
  duration: string | null
  failureCode: string | null
  repairCount: number | null
  detail: string | null
}

export function agentItemTypeLabel(itemType: AgentItemType): string {
  return ITEM_TYPE_LABELS[itemType]
}

export function agentItemPreview(item: AgentItem): string {
  const payload = item.payload
  if (typeof payload.message === 'string') return payload.message
  if (typeof payload.text === 'string') return payload.text
  return item.item_type
}

/**
 * Produces a compact audit trail from persisted Item rows. Deliberately only
 * whitelisted tool metadata and bounded proof counts are presented here:
 * Provider reasoning, prompt text, raw tool arguments, and raw results stay
 * out of the workbench UI.
 */
export function agentProcessSteps(items: readonly AgentItem[]): AgentProcessStep[] {
  const calls = new Map<string, AgentItem>()
  const completedCallIds = new Set<string>()
  const steps: AgentProcessStep[] = []
  for (const item of items) {
    const callId = safeString(item.payload.call_id)
    if (item.item_type === 'tool_call' && callId) {
      calls.set(callId, item)
      continue
    }
    if (item.item_type === 'tool_result') {
      const call = callId ? calls.get(callId) : undefined
      if (callId && call) completedCallIds.add(callId)
      const tool = safeString(item.payload.tool_name) ?? safeString(call?.payload.tool_name)
      steps.push({
        key: item.item_id,
        stage: tool ? (TOOL_STAGE_LABELS[tool] ?? '执行受限工具') : '完成受限工具步骤',
        tool,
        status: item.status,
        inputEvidence: inputEvidence(call?.payload),
        duration: formatDuration(call?.created_at, item.created_at),
        failureCode: failureCode(item.payload),
        repairCount: repairCount(item.payload),
        detail: item.status === 'completed' ? completedEvidence(item.payload) : null,
      })
      continue
    }
    if (item.item_type !== 'tool_call') {
      steps.push({
        key: item.item_id,
        stage: agentItemTypeLabel(item.item_type),
        tool: null,
        status: item.status,
        inputEvidence: null,
        duration: null,
        failureCode: null,
        repairCount: null,
        detail: safeItemMessage(item),
      })
    }
  }
  // A live tool call has no result row yet. Keep it in the audit trail so the
  // user can see the current bounded action rather than a silent gap.
  for (const [callId, call] of calls) {
    if (completedCallIds.has(callId)) continue
    const tool = safeString(call.payload.tool_name)
    steps.push({
      key: call.item_id,
      stage: tool ? (TOOL_STAGE_LABELS[tool] ?? '执行受限工具') : '执行受限工具步骤',
      tool,
      status: call.status,
      inputEvidence: inputEvidence(call.payload),
      duration: null,
      failureCode: null,
      repairCount: null,
      detail: null,
    })
  }
  return steps
}

function safeString(value: unknown): string | null {
  if (typeof value !== 'string') return null
  const normalized = value.trim()
  return normalized && normalized.length <= 120 ? normalized : null
}

function asRecord(value: unknown): RecordValue | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value) ? value as RecordValue : null
}

function inputEvidence(payload: RecordValue | undefined): string | null {
  const argumentsValue = payload ? asRecord(payload.arguments) : null
  if (!argumentsValue) return null
  const dispositions = argumentsValue.evidence_dispositions
  if (Array.isArray(dispositions) && dispositions.length > 0) return `输入证据：已绑定 ${dispositions.length} 条视觉 claim`
  if (typeof argumentsValue.brief === 'string') return '输入证据：文字设计需求'
  if (asRecord(argumentsValue.patch)) return '输入证据：当前版本与受限修复目标'
  if (asRecord(argumentsValue.program)) return '输入证据：受限视觉程序草稿'
  return null
}

function resultValue(payload: RecordValue): RecordValue | null {
  const direct = asRecord(payload.result)
  if (direct) return direct
  const toolResult = asRecord(payload.tool_result)
  const validated = toolResult ? asRecord(toolResult.validated_output) : null
  return validated ? asRecord(validated.value) : null
}

function repairCount(payload: RecordValue): number | null {
  const result = resultValue(payload)
  const convergence = result ? asRecord(result.visual_convergence_report) : null
  const repair = result ? asRecord(result.repair) : null
  if (convergence?.schema_version !== 'VisualConvergenceReport@2') return null
  const count = convergence?.repair_attempt_count ?? repair?.repair_number
  return typeof count === 'number' && Number.isInteger(count) && count >= 0 && count <= 1 ? count : null
}

function completedEvidence(payload: RecordValue): string | null {
  const result = resultValue(payload)
  if (!result) return null
  const convergence = asRecord(result.visual_convergence_report)
  const fixedViews = convergence?.fixed_view_count
  if (typeof fixedViews === 'number' && Number.isInteger(fixedViews)) return `可核验证据：${fixedViews} 个固定视图已记录`
  if (result.hard_gate_passed === true) return '可核验证据：硬门已通过'
  if (result.candidate_only === true) return '可核验证据：候选模型已构建'
  return null
}

function failureCode(payload: RecordValue): string | null {
  const direct = safeErrorCode(payload.error_code)
  if (direct) return direct
  const toolResult = asRecord(payload.tool_result)
  return safeErrorCode(toolResult?.error_code)
}

function safeErrorCode(value: unknown): string | null {
  if (typeof value !== 'string' || value.length === 0 || value.length > 120) return null
  return /^[A-Z0-9_]+$/.test(value) ? value : null
}

function formatDuration(startedAt: string | undefined, completedAt: string): string | null {
  if (!startedAt) return null
  const start = Date.parse(startedAt)
  const end = Date.parse(completedAt)
  if (!Number.isFinite(start) || !Number.isFinite(end) || end < start) return null
  const milliseconds = end - start
  return milliseconds < 1_000 ? '< 1 秒' : `${(milliseconds / 1_000).toFixed(milliseconds < 10_000 ? 1 : 0)} 秒`
}

function safeItemMessage(item: AgentItem): string | null {
  if (item.item_type === 'plan') return agentItemPreview(item)
  return null
}

export function AgentStepItem({ step }: { step: AgentProcessStep }) {
  const statusLabel = step.status === 'pending'
    ? '进行中'
    : step.status === 'completed'
      ? '完成'
      : step.status === 'cancelled'
        ? '已取消'
        : '失败'
  return (
    <div className={`agent-kernel-event status-${step.status}`} data-agent-stage={step.stage}>
      <div className="agent-kernel-event-heading">
        <strong>{step.stage}</strong>
        <span>{statusLabel}</span>
      </div>
      <div className="agent-kernel-event-meta">
        {step.tool && <small>工具动作：{step.tool}</small>}
        {step.inputEvidence && <small>{step.inputEvidence}</small>}
        {step.duration && <small>耗时：{step.duration}</small>}
        {step.repairCount !== null && <small>修复次数：{step.repairCount}/2</small>}
        {step.failureCode && <small>失败原因：{step.failureCode}</small>}
        {step.detail && <small>{step.detail}</small>}
      </div>
    </div>
  )
}
