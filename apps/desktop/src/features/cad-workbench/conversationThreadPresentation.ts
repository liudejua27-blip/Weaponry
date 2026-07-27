import type { AgentThreadDetail, AgentItem } from '../../shared/types'
import { parseAgentTurnPresentation, type AgentTurnPresentation } from './agentConversationState'
import { flattenConversationHistoryItems } from './conversationHistoryItems.js'

type ThreadLike = Pick<AgentThreadDetail, 'title' | 'turns'>

type ConversationThreadSummary = {
  assistantNote: string
  items: AgentItem[]
  presentation: AgentTurnPresentation
}

export function buildConversationThreadSummary(thread: ThreadLike): ConversationThreadSummary {
  const lastTurn = thread.turns.at(-1)
  const items = flattenConversationHistoryItems(thread.turns)
  const presentation = lastTurn
    ? parseAgentTurnPresentation(lastTurn.items, lastTurn.request_text)
    : { clarification: null, plan: null }

  return {
    assistantNote: lastTurn
      ? `已打开“${thread.title}”；当前 3D 与 Snapshot 保持不变。`
      : `已打开“${thread.title}”；这个对话还没有消息。`,
    items,
    presentation,
  }
}
