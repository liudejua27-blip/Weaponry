import type { AgentThreadSummary } from '../../shared/types'

export function filterAgentThreadsForProject(
  threads: readonly AgentThreadSummary[] | null | undefined,
  projectId: string | null,
): AgentThreadSummary[] {
  if (!projectId || !threads || threads.length === 0) return []

  const result: AgentThreadSummary[] = []
  for (let index = 0; index < threads.length; index += 1) {
    const thread = threads[index]
    if (thread?.project_id === projectId) {
      result.push(thread)
    }
  }

  return result
}
