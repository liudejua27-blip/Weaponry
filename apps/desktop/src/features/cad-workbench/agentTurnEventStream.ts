import type { AgentEvent, AgentItem } from '../../shared/types'
import type { AgentTurnPresentation } from './agentConversationState'

export function latestKernelSequence(items: readonly AgentItem[]): number {
  let afterSequence = 0
  for (let index = 0; index < items.length; index += 1) {
    const sequence = items[index].sequence
    if (sequence > afterSequence) afterSequence = sequence
  }
  return afterSequence
}

type AgentTurnEventCollectorOptions = {
  projectId: string | null
  requestId: number
  threadId: string
  isCurrentRequest: (projectId: string | null, requestId: number) => boolean
  setActiveProviderTurnId: (value: string | null) => void
  parseAgentTurnPresentation: (items: readonly AgentItem[], requestText: string) => AgentTurnPresentation
  receiveAgentTurn: (
    projectId: string | null,
    requestId: number,
    threadId: string,
    items: readonly AgentItem[],
    presentation: AgentTurnPresentation,
  ) => boolean
  message: string
}

export function createAgentTurnEventCollector({
  existingKernelItems,
  projectId,
  requestId,
  threadId,
  isCurrentRequest,
  setActiveProviderTurnId,
  parseAgentTurnPresentation,
  receiveAgentTurn,
  message,
}: AgentTurnEventCollectorOptions & { existingKernelItems: readonly AgentItem[] }) {
  const orderedItems: AgentItem[] = [...existingKernelItems].sort((left, right) => left.sequence - right.sequence)
  const afterSequence = latestKernelSequence(existingKernelItems)
  let itemCount = orderedItems.length

  function upsertBySequence(nextItem: AgentItem): void {
    let low = 0
    let high = itemCount
    while (low < high) {
      const mid = (low + high) >>> 1
      if (orderedItems[mid].sequence < nextItem.sequence) {
        low = mid + 1
      } else {
        high = mid
      }
    }

    if (low < itemCount && orderedItems[low].sequence === nextItem.sequence) {
      orderedItems[low] = nextItem
      return
    }

    orderedItems.splice(low, 0, nextItem)
    itemCount += 1
  }

  function onEvent(event: AgentEvent): void {
    if (!isCurrentRequest(projectId, requestId)) return
    const nextItem = event.item
    upsertBySequence(nextItem)
    setActiveProviderTurnId(event.turn_id)
    receiveAgentTurn(
      projectId,
      requestId,
      threadId,
      orderedItems,
      parseAgentTurnPresentation(orderedItems, message),
    )
  }

  return {
    afterSequence,
    onEvent,
  }
}
