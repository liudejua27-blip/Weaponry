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

export function agentItemTypeLabel(itemType: AgentItemType): string {
  return ITEM_TYPE_LABELS[itemType]
}

export function agentItemPreview(item: AgentItem): string {
  const payload = item.payload
  if (typeof payload.message === 'string') return payload.message
  if (typeof payload.text === 'string') return payload.text
  return item.item_type
}

export function AgentStepItem({ item }: { item: AgentItem }) {
  return (
    <div className="agent-kernel-event" data-agent-item-type={item.item_type}>
      <span>{agentItemTypeLabel(item.item_type)}</span>
      <small>{agentItemPreview(item)}</small>
    </div>
  )
}
