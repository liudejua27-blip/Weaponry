import type { ForgeApi } from '../../shared/api/forgeApi'
import type { MechanicalConceptPlan } from '../../shared/types'

type AgentDirectionPreviewApi = Pick<ForgeApi, 'buildAgentBlockout' | 'segmentAgentBlockout'>

type BuildBlockoutResult = Awaited<ReturnType<AgentDirectionPreviewApi['buildAgentBlockout']>>
type SegmentBlockoutResult = Awaited<ReturnType<AgentDirectionPreviewApi['segmentAgentBlockout']>>

type DirectionProfile = 'quick_sketch' | 'showcase'

type AgentDirectionPreviewCallbacks = {
  startDirectionPreview: (projectId: string | null, directionId: string, variationIndex: number) => number
  receiveBlockoutBuild: (
    projectId: string | null,
    requestId: number,
    glbBase64: BuildBlockoutResult['glb_base64'],
    shapeProgram: BuildBlockoutResult['shape_program'],
  ) => boolean
  clearAgentAssetWorkspace: () => void
  resetDirectionDraftSelections: () => void
  receiveSegmentation: (
    projectId: string | null,
    requestId: number,
    segmentation: SegmentBlockoutResult,
  ) => boolean
  failSegmentation: (projectId: string | null, requestId: number) => boolean
  isCurrentDirectionPreview: (projectId: string | null, requestId: number) => boolean
  failDirectionPreview: (projectId: string | null, requestId: number) => boolean
  setAssistantNote: (message: string) => void
}
const DIRECTION_PROFILE_LABELS: Record<DirectionProfile, string> = {
  quick_sketch: '快速草图',
  showcase: '展示模型',
}

export async function previewAgentDirection(
  api: AgentDirectionPreviewApi,
  callbacks: AgentDirectionPreviewCallbacks,
  projectId: string | null,
  directionId: string,
  variationIndex: number,
  requestedProfile: DirectionProfile,
  planOverride?: MechanicalConceptPlan,
): Promise<void> {
  const plan = planOverride ?? null
  if (!plan) return

  const requestId = callbacks.startDirectionPreview(projectId, directionId, variationIndex)
  callbacks.setAssistantNote('正在构建当前唯一展示结果…')

  try {
    const result = await api.buildAgentBlockout({
      client_request_id: `agent-blockout-${Date.now()}`,
      plan,
      direction_id: directionId,
      variation_index: variationIndex,
      presentation_profile: requestedProfile,
    })

    if (!callbacks.receiveBlockoutBuild(projectId, requestId, result.glb_base64, result.shape_program)) return

    callbacks.clearAgentAssetWorkspace()
    callbacks.resetDirectionDraftSelections()

    try {
      const segmentation = await api.segmentAgentBlockout({
        client_request_id: `agent-segment-${Date.now()}`,
        plan,
        direction_id: directionId,
        variant_id: result.variant_id,
        variation_index: result.variation_index,
        presentation_profile: result.presentation_profile,
        artifact_id: result.artifact_id,
      })
      if (!callbacks.receiveSegmentation(projectId, requestId, segmentation)) return
    } catch {
      if (!callbacks.failSegmentation(projectId, requestId)) return
    }

    if (!callbacks.isCurrentDirectionPreview(projectId, requestId)) return

    const profile = DIRECTION_PROFILE_LABELS[result.presentation_profile ?? requestedProfile]
    callbacks.setAssistantNote(`${profile}已生成 ${result.triangle_count.toLocaleString()} 个展示面；确认前不会写入正式版本。`)
  } catch {
    if (!callbacks.failDirectionPreview(projectId, requestId)) return
    callbacks.setAssistantNote('blockout 预览生成失败；当前设计仍未写入版本。')
  }
}
