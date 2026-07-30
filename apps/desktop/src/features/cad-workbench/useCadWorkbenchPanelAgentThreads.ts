import { useEffect, useState } from 'react'
import type { AgentThreadSummary } from '../../shared/types'
import { filterAgentThreadsForProject } from './filterAgentThreadsForProject'

type CadWorkbenchPanelThreadApi = {
  listAgentThreads: () => Promise<{ items: readonly AgentThreadSummary[] }>
}

type UseCadWorkbenchPanelAgentThreadsInput = {
  api: CadWorkbenchPanelThreadApi
  projectId: string | null
  activeThreadId: string | null
}

export function useCadWorkbenchPanelAgentThreads({
  api,
  projectId,
  activeThreadId,
}: UseCadWorkbenchPanelAgentThreadsInput): {
  agentThreads: AgentThreadSummary[]
  threadHistoryLoading: boolean
} {
  const [agentThreads, setAgentThreads] = useState<AgentThreadSummary[]>([])
  const [threadHistoryLoading, setThreadHistoryLoading] = useState(false)

  useEffect(() => {
    let cancelled = false
    setThreadHistoryLoading(true)
    void api.listAgentThreads()
      .then((response) => {
        if (cancelled) return
        setAgentThreads(filterAgentThreadsForProject(response.items, projectId))
      })
      .catch(() => {
        if (!cancelled) setAgentThreads([])
      })
      .finally(() => {
        if (!cancelled) setThreadHistoryLoading(false)
      })

    return () => {
      cancelled = true
    }
  }, [api, projectId, activeThreadId])

  return { agentThreads, threadHistoryLoading }
}
