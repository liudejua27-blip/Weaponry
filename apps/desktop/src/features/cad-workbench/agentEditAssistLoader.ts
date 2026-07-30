import type { ForgeApi } from '../../shared/api/forgeApi'

type AgentEditAssistApi = Pick<
  ForgeApi,
  'listAgentComponentCandidates' |
  'listAgentStructureSuggestions' |
  'listAgentSemanticProportions'
>

type ComponentCandidates = Awaited<ReturnType<AgentEditAssistApi['listAgentComponentCandidates']>>
type StructureSuggestions = Awaited<ReturnType<AgentEditAssistApi['listAgentStructureSuggestions']>>
type SemanticProportions = Awaited<ReturnType<AgentEditAssistApi['listAgentSemanticProportions']>>

type AgentEditAssistRequestInput = {
  projectId: string
  assetVersionId: string
  partId: string
}

type AgentEditAssistCallbacks = {
  startAgentEditAssistRead: (projectId: string, assetVersionId: string, partId: string) => number | null
  receiveAgentEditAssistRead: (
    projectId: string,
    assetVersionId: string,
    partId: string,
    requestId: number,
    candidates: ComponentCandidates,
    structureSuggestions: StructureSuggestions,
    semanticProportions: SemanticProportions | null,
  ) => void
  failAgentEditAssistRead: (projectId: string, assetVersionId: string, partId: string, requestId: number) => void
}

export async function loadAgentEditAssist(
  api: AgentEditAssistApi,
  callbacks: AgentEditAssistCallbacks,
  input: AgentEditAssistRequestInput,
): Promise<void> {
  const {
    startAgentEditAssistRead,
    receiveAgentEditAssistRead,
    failAgentEditAssistRead,
  } = callbacks

  const { projectId, assetVersionId, partId } = input
  const requestId = startAgentEditAssistRead(projectId, assetVersionId, partId)
  if (requestId === null) return

  try {
    const [candidates, structure, semanticProportions] = await Promise.all([
      api.listAgentComponentCandidates(assetVersionId, partId),
      api.listAgentStructureSuggestions(assetVersionId),
      api.listAgentSemanticProportions(assetVersionId, partId).catch(() => null),
    ])
    receiveAgentEditAssistRead(projectId, assetVersionId, partId, requestId, candidates, structure, semanticProportions)
  } catch {
    failAgentEditAssistRead(projectId, assetVersionId, partId, requestId)
  }
}
