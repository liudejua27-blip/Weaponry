import { initialAgentBlockoutDisplayState, type AgentBlockoutDisplayState } from './agentBlockoutDisplayState.js'
import { selectAgentBlockoutPreviewPresentation } from './agentBlockoutPreviewPresentation.js'

function state(overrides: Partial<AgentBlockoutDisplayState>): AgentBlockoutDisplayState {
  return { ...initialAgentBlockoutDisplayState, projectId: 'project-smoke', directionId: 'direction_smoke', ...overrides }
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

export function runAgentBlockoutPreviewPresentationSmoke(): void {
  assert(selectAgentBlockoutPreviewPresentation(initialAgentBlockoutDisplayState) === null, 'an idle workbench must not invent a preview message')

  const working = selectAgentBlockoutPreviewPresentation(state({ directionPreviewLoading: true }))
  assert(working?.tone === 'working' && working.title.includes('完整外观') && working.detail.includes('已保存设计'), 'building must use a plain-language no-write explanation')

  const ready = selectAgentBlockoutPreviewPresentation(state({ segmentation: {
    artifact_id: 'artifact_smoke', plan_id: 'plan_smoke', direction_id: 'direction_smoke', domain_pack_id: 'pack_vehicle_concept',
    segmentation_status: 'candidate', parts: [], assembly_graph: { schema_version: 'AssemblyGraph@1', roots: [], parts: [], joints: [] },
  } }))
  assert(ready?.tone === 'ready' && ready.detail.includes('保存为可编辑模型') && ready.detail.includes('换一版外观'), 'candidate readiness must give exactly the two beginner actions')

  const segmentationUnavailable = selectAgentBlockoutPreviewPresentation(state({ previewError: 'segmentation_failed' }))
  assert(segmentationUnavailable?.tone === 'notice' && segmentationUnavailable.detail.includes('重新选择方向') && !segmentationUnavailable.detail.includes('segmentation'), 'candidate failure must remain non-technical')

  const failed = selectAgentBlockoutPreviewPresentation(state({ previewError: 'blockout_failed' }))
  assert(failed?.tone === 'error' && failed.detail.includes('没有变化') && !failed.detail.includes('error'), 'generation failure must state safety and next actions without an error code')
}
