import type { AgentComponentCandidate, AgentStructureSuggestionList } from '../../shared/types.js'
import {
  agentEditAssistPresentationReducer,
  initialAgentEditAssistPresentationState,
} from './agentEditAssistPresentationState.js'

const candidate = (assetVersionId: string, partId: string): AgentComponentCandidate => ({
  component: {
    component_id: `component-${partId}`,
    project_id: 'project-a', domain_pack_id: 'pack_robotic_arm_concept', role: 'joint_elbow', display_name: '可复用关节',
    source_asset_version_id: assetVersionId, source_part_id: partId, part_template: { part_id: partId, role: 'joint_elbow', position_mm: [0, 0, 0], size_mm: [1, 1, 1], material_zone_ids: [] },
    shape_operation: {}, created_at: '2026-07-14T00:00:00Z', updated_at: '2026-07-14T00:00:00Z',
  },
  compatibility: {
    component_id: `component-${partId}`, target_asset_version_id: assetVersionId, target_part_id: partId, eligible: true,
    source_quality_status: 'passed', reason_codes: ['same_domain_pack', 'same_role', 'component_active', 'source_quality_passed', 'target_connectors_preserved'],
  },
})

const structure = (assetVersionId: string, partId: string): AgentStructureSuggestionList => ({
  asset_version_id: assetVersionId,
  suggestions: [{ suggestion_id: `suggestion-${partId}`, kind: 'split_part', asset_version_id: assetVersionId, part_id: partId, affected_part_ids: [partId], source_facts: ['stable_role'], summary: '依据现有关系预览拆分' }],
})

export function runAgentEditAssistPresentationStateSmoke(): void {
  let state = agentEditAssistPresentationReducer(initialAgentEditAssistPresentationState, {
    type: 'open_context', projectId: 'project-a', assetVersionId: 'asset-a', selectedPartId: 'part-a', requestId: 1,
  })
  state = agentEditAssistPresentationReducer(state, {
    type: 'read_started', projectId: 'project-a', assetVersionId: 'asset-a', selectedPartId: 'part-a', requestId: 2,
  })
  state = agentEditAssistPresentationReducer(state, {
    type: 'read_received', projectId: 'project-a', assetVersionId: 'asset-a', selectedPartId: 'part-a', requestId: 2,
    componentCandidates: [candidate('asset-a', 'part-a'), candidate('asset-a', 'part-b'), candidate('asset-b', 'part-a')],
    structure: {
      ...structure('asset-a', 'part-a'),
      suggestions: [...(structure('asset-a', 'part-a').suggestions ?? []), ...(structure('asset-b', 'part-a').suggestions ?? [])],
    },
  })
  assert(
    state.componentCandidates.length === 1 && state.structureSuggestions.length === 1 && !state.loading,
    'read results must retain only candidates and suggestions for the current asset and selected part',
  )

  state = agentEditAssistPresentationReducer(state, {
    type: 'open_context', projectId: 'project-a', assetVersionId: 'asset-a', selectedPartId: 'part-b', requestId: 3,
  })
  assert(
    state.componentCandidates.length === 0 && state.structureSuggestions.length === 0 && state.structureSuggestionUnavailableMessage === null,
    'changing the selected part must clear previous edit-assist read results',
  )
  const lateRead = agentEditAssistPresentationReducer(state, {
    type: 'read_received', projectId: 'project-a', assetVersionId: 'asset-a', selectedPartId: 'part-a', requestId: 2,
    componentCandidates: [candidate('asset-a', 'part-a')], structure: structure('asset-a', 'part-a'),
  })
  assert(lateRead === state, 'late reads from a prior part must not overwrite the current selection context')

  state = agentEditAssistPresentationReducer(state, {
    type: 'read_started', projectId: 'project-a', assetVersionId: 'asset-a', selectedPartId: 'part-b', requestId: 4,
  })
  state = agentEditAssistPresentationReducer(state, {
    type: 'read_failed', projectId: 'project-a', assetVersionId: 'asset-a', selectedPartId: 'part-b', requestId: 4,
  })
  assert(
    state.structureSuggestionUnavailableMessage === '暂时无法读取结构建议；模型没有被修改。',
    'read failures must report only an unavailable presentation state, never fabricate a suggestion',
  )

  state = agentEditAssistPresentationReducer(state, {
    type: 'open_context', projectId: null, assetVersionId: null, selectedPartId: null, requestId: 5,
  })
  const fields = Object.keys(state).sort().join(',')
  assert(
    fields === 'assetVersionId,componentCandidates,latestRequestId,loading,projectId,selectedPartId,structureSuggestionUnavailableMessage,structureSuggestions',
    'edit-assist state must not contain Snapshot, quality, ChangeSet, export, component catalog, renderer, or asset-head truth',
  )
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
