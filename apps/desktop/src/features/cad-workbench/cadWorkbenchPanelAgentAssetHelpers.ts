import type { MultimodalAgentTurnContext } from './agentTurnSubmissionLoader'

const INSPECT_AGENT_ASSET_FALLBACK_FINDING = '请查看检查结果。'

type AgentAssetQualityReportFinding = {
  message?: string | null
}

type AgentAssetQualityReport = {
  status?: 'passed' | 'warning' | 'failed' | string
  triangle_count: number
  findings?: readonly AgentAssetQualityReportFinding[] | null
}

export function buildInspectAgentAssetNote(report: AgentAssetQualityReport): string {
  if (report.status === 'passed') {
    return `模型检查通过：${report.triangle_count.toLocaleString()} 三角形，部件层级和关节引用正常。`
  }

  const findingMessage = report.findings?.[0]?.message ?? INSPECT_AGENT_ASSET_FALLBACK_FINDING
  return `模型检查${report.status === 'warning' ? '有提示' : '未通过'}：${findingMessage}`
}

export function buildReferenceEvidenceVisionContext(
  submitAssistantInstructionWithText: (
    requestedText: string,
    clarificationDomainPackId?: string,
    multimodalContext?: MultimodalAgentTurnContext,
  ) => Promise<void>,
  input: {
    instruction: string
    activeAssetVersionId: string | null
    selectedPartId: string | null
    selectedMaterialZoneId: string | null
  },
) {
  return {
    instruction: input.instruction,
    activeAssetVersionId: input.activeAssetVersionId,
    selectedPartId: input.selectedPartId,
    selectedMaterialZoneId: input.selectedMaterialZoneId,
    onUseEvidence: ({ instruction, request, graph }: {
      instruction: string
      request: MultimodalAgentTurnContext['request']
      graph: MultimodalAgentTurnContext['graph']
    }) => {
      return submitAssistantInstructionWithText(instruction, undefined, {
        request,
        graph,
      })
    },
  }
}
