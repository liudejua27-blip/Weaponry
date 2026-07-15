import {
  activeDesignCanSelectParts,
  activeDesignIsLegacyReadOnly,
  activeDesignMachineReducer,
  activeDesignSelectedPartId,
  activeDesignVersionId,
  initialActiveDesignMachineState,
} from './activeDesignMachine.js'

const agentSnapshot = {
  schema_version: 'ActiveDesignSnapshot@1' as const,
  project_id: 'prj_s005_agent',
  active_design: {
    source: 'agent_asset' as const,
    project_id: 'prj_s005_agent',
    asset_version_id: 'assetver_s005_agent',
    assembly_graph_id: 'mg_s005_agent',
  },
  selected_part_id: 'part_s005_body',
  preview: null,
  quality: null,
  export: {
    source: 'agent_asset' as const,
    project_id: 'prj_s005_agent',
    source_version_id: 'assetver_s005_agent',
  },
  revision: 3,
  updated_at: '2026-07-13T00:00:00+00:00',
}

const legacySnapshot = {
  schema_version: 'ActiveDesignSnapshot@1' as const,
  project_id: 'prj_s005_legacy',
  active_design: {
    source: 'legacy_concept_read_only' as const,
    project_id: 'prj_s005_legacy',
    legacy_version_id: 'ver_s005_legacy',
    module_graph_id: 'mg_s005_legacy',
  },
  selected_part_id: null,
  preview: null,
  quality: null,
  export: {
    source: 'legacy_concept_read_only' as const,
    project_id: 'prj_s005_legacy',
    source_version_id: 'ver_s005_legacy',
  },
  revision: 1,
  updated_at: '2026-07-13T00:00:00+00:00',
}

export function runActiveDesignMachineSmoke(): void {
  let state = activeDesignMachineReducer(initialActiveDesignMachineState, {
    type: 'open_project',
    projectId: 'prj_s005_agent',
  })
  state = activeDesignMachineReducer(state, { type: 'request_started', requestId: 1, operation: 'loading' })
  state = activeDesignMachineReducer(state, {
    type: 'snapshot_received',
    projectId: 'prj_s005_agent',
    requestId: 1,
    response: { data: agentSnapshot, etag: 'W/"active-design-3"' },
  })
  assert(state.snapshot === agentSnapshot, 'current Snapshot must be the only stored design result')
  assert(state.snapshotEtag === 'W/"active-design-3"', 'must retain server ETag with Snapshot')
  assert(activeDesignVersionId(state.snapshot) === 'assetver_s005_agent', 'must derive active Agent version from Snapshot')
  assert(activeDesignSelectedPartId(state.snapshot) === 'part_s005_body', 'must derive selected Part from Snapshot')
  assert(activeDesignCanSelectParts(state.snapshot), 'Agent Snapshot must allow part selection')
  assert(!activeDesignIsLegacyReadOnly(state.snapshot), 'Agent Snapshot must not be legacy read-only')

  state = activeDesignMachineReducer(state, { type: 'request_started', requestId: 2, operation: 'selecting' })
  state = activeDesignMachineReducer(state, { type: 'request_cancelled', requestId: 2 })
  assert(state.operation === 'idle', 'cancelled request must release the loading state')
  assert(state.snapshot === agentSnapshot, 'cancelled request must preserve the last verified Snapshot')

  state = activeDesignMachineReducer(state, { type: 'request_started', requestId: 3, operation: 'selecting' })
  const afterStaleResponse = activeDesignMachineReducer(state, {
    type: 'snapshot_received',
    projectId: 'prj_s005_agent',
    requestId: 2,
    response: { data: { ...agentSnapshot, revision: 2 }, etag: 'W/"active-design-2"' },
  })
  assert(afterStaleResponse === state, 'late response must never replace a newer request')
  state = activeDesignMachineReducer(state, {
    type: 'request_failed',
    requestId: 3,
    error: {
      kind: 'stale',
      message: '刷新后重试',
      shouldReloadSnapshot: true,
      assetChanged: false,
    },
  })
  assert(state.snapshot === agentSnapshot, 'failed requests must preserve the last verified Snapshot')
  assert(state.error?.shouldReloadSnapshot, 'stale error must request a Snapshot reload')

  state = activeDesignMachineReducer(state, { type: 'open_project', projectId: 'prj_s005_legacy' })
  state = activeDesignMachineReducer(state, { type: 'request_started', requestId: 4, operation: 'loading' })
  state = activeDesignMachineReducer(state, {
    type: 'snapshot_received',
    projectId: 'prj_s005_legacy',
    requestId: 4,
    response: { data: legacySnapshot, etag: 'W/"active-design-1"' },
  })
  assert(activeDesignVersionId(state.snapshot) === 'ver_s005_legacy', 'legacy version must remain separate from Agent version IDs')
  assert(!activeDesignCanSelectParts(state.snapshot), 'legacy Snapshot must not select Agent parts')
  assert(activeDesignIsLegacyReadOnly(state.snapshot), 'legacy Snapshot must remain read-only')
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
