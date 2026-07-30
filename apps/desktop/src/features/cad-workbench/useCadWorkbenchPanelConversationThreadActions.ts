import { useCallback } from 'react'
import type { AgentItem, AgentThreadDetail } from '../../shared/types'
import { buildConversationThreadSummary } from './conversationThreadPresentation.js'
import type { AgentTurnPresentation } from './agentConversationState'

type UseCadWorkbenchPanelConversationThreadActionsInput = {
  projectId: string | null
  getConversationThread: (threadId: string) => Promise<AgentThreadDetail>
  startAgentConversationRequest: (projectId: string | null) => { requestId: number }
  isCurrentAgentConversationRequest: (projectId: string | null, requestId: number) => boolean
  receiveAgentTurn: (
    projectId: string | null,
    requestId: number,
    threadId: string,
    items: readonly AgentItem[],
    presentation: AgentTurnPresentation,
  ) => boolean
  setAssistantNote: (note: string) => void
  errorText: (value: unknown) => string
}

type UseCadWorkbenchPanelConversationThreadActionsOutput = {
  selectConversationThread: (threadId: string) => Promise<void>
}

export function useCadWorkbenchPanelConversationThreadActions({
  projectId,
  getConversationThread,
  startAgentConversationRequest,
  isCurrentAgentConversationRequest,
  receiveAgentTurn,
  setAssistantNote,
  errorText,
}: UseCadWorkbenchPanelConversationThreadActionsInput): UseCadWorkbenchPanelConversationThreadActionsOutput {
  const selectConversationThread = useCallback(async (threadId: string) => {
    const { requestId } = startAgentConversationRequest(projectId)
    try {
      const thread = await getConversationThread(threadId)
      if ((thread.project_id ?? null) !== projectId) {
        setAssistantNote('这个对话不属于当前项目，未切换工作台。')
        return
      }
      const threadSummary = buildConversationThreadSummary(thread)
      if (!receiveAgentTurn(projectId, requestId, thread.thread_id, threadSummary.items, threadSummary.presentation)) return
      setAssistantNote(threadSummary.assistantNote)
    } catch (caught) {
      if (!isCurrentAgentConversationRequest(projectId, requestId)) return
      setAssistantNote(`对话记录加载失败：${errorText(caught)}`)
    }
  }, [
    errorText,
    getConversationThread,
    isCurrentAgentConversationRequest,
    projectId,
    receiveAgentTurn,
    setAssistantNote,
    startAgentConversationRequest,
  ])

  return {
    selectConversationThread,
  }
}
