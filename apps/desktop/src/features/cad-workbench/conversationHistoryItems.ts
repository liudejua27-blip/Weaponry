import type { AgentItem } from '../../shared/types'

type ThreadLike = {
  items: readonly AgentItem[]
}

export function flattenConversationHistoryItems(turns: readonly ThreadLike[]): AgentItem[] {
  let totalItems = 0
  for (let turnIndex = 0; turnIndex < turns.length; turnIndex += 1) {
    const turnItems = turns[turnIndex].items
    totalItems += turnItems.length
  }
  if (totalItems === 0) return []

  const items: AgentItem[] = new Array(totalItems)
  let itemIndex = 0
  for (let turnIndex = 0; turnIndex < turns.length; turnIndex += 1) {
    const turnItems = turns[turnIndex].items
    for (let index = 0; index < turnItems.length; index += 1) {
      items[itemIndex] = turnItems[index]
      itemIndex += 1
    }
  }
  items.sort((left, right) => left.sequence - right.sequence)
  return items
}
