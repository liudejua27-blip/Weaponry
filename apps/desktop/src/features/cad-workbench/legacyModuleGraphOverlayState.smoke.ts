import {
  createLegacyModuleGraphOverlayContextKey,
  initialLegacyModuleGraphOverlayState,
  legacyModuleGraphOverlayReducer,
} from './legacyModuleGraphOverlayState.js'

export function runLegacyModuleGraphOverlayStateSmoke(): void {
  const projectAGraphA = createLegacyModuleGraphOverlayContextKey('project-a', 'graph-a')
  const projectAGraphB = createLegacyModuleGraphOverlayContextKey('project-a', 'graph-b')
  const projectBGraphA = createLegacyModuleGraphOverlayContextKey('project-b', 'graph-a')
  assert(projectAGraphA && projectAGraphA !== projectAGraphB && projectAGraphA !== projectBGraphA, 'overlay must be isolated by project and graph')

  let state = legacyModuleGraphOverlayReducer(initialLegacyModuleGraphOverlayState, {
    type: 'open_context', contextKey: projectAGraphA, defaultHiddenNodeIds: ['storage', 'storage'],
  })
  state = legacyModuleGraphOverlayReducer(state, { type: 'reconcile_nodes', nodeIds: ['root', 'body', 'storage'] })
  state = legacyModuleGraphOverlayReducer(state, { type: 'toggle_hidden_node', nodeId: 'body' })
  state = legacyModuleGraphOverlayReducer(state, { type: 'set_focus_node', nodeId: 'body' })
  state = legacyModuleGraphOverlayReducer(state, {
    type: 'set_quality_overlay',
    nodeIds: ['body', 'missing'],
    geometryRefs: [
      { node_id: 'body', triangle_indices: [1, 2, 3] },
      { node_id: 'missing', triangle_indices: [4, 5, 6] },
    ],
  })
  state = legacyModuleGraphOverlayReducer(state, { type: 'reconcile_nodes', nodeIds: ['root', 'body', 'storage'] })
  state = legacyModuleGraphOverlayReducer(state, { type: 'record_thumbnail_failure', moduleId: 'module-body' })
  assert(
    state.hiddenNodeIds.join(',') === 'storage,body'
      && state.focusNodeId === 'body'
      && state.qualityHighlightNodeIds.join(',') === 'body'
      && state.qualityGeometryRefs.length === 1
      && state.thumbnailFailureModuleIds.join(',') === 'module-body',
    'legacy overlays must retain only valid graph references and transient thumbnail failures',
  )

  state = legacyModuleGraphOverlayReducer(state, { type: 'reconcile_nodes', nodeIds: ['root', 'storage'] })
  assert(
    state.hiddenNodeIds.join(',') === 'storage'
      && state.focusNodeId === null
      && state.qualityHighlightNodeIds.length === 0
      && state.qualityGeometryRefs.length === 0,
    'stale graph nodes must be removed from each viewport overlay without changing a quality result',
  )

  state = legacyModuleGraphOverlayReducer(state, { type: 'clear_quality_overlay' })
  state = legacyModuleGraphOverlayReducer(state, {
    type: 'open_context', contextKey: projectAGraphB, defaultHiddenNodeIds: ['storage'],
  })
  assert(
    state.hiddenNodeIds.join(',') === 'storage'
      && state.focusNodeId === null
      && state.qualityHighlightNodeIds.length === 0
      && state.qualityGeometryRefs.length === 0
      && state.thumbnailFailureModuleIds.length === 0,
    'switching a legacy graph must clear transient focus, quality overlays, and thumbnail failures',
  )

  state = legacyModuleGraphOverlayReducer(state, {
    type: 'open_context', contextKey: null, defaultHiddenNodeIds: [],
  })
  state = legacyModuleGraphOverlayReducer(state, { type: 'toggle_hidden_node', nodeId: 'agent-part' })
  assert(
    state.contextKey === null && state.hiddenNodeIds.length === 0 && state.focusNodeId === null,
    'Agent source must not receive legacy hidden-node or focus display state',
  )

  const fields = Object.keys(state).sort().join(',')
  assert(
    fields === 'contextKey,focusNodeId,hiddenNodeIds,qualityGeometryRefs,qualityHighlightNodeIds,thumbnailFailureModuleIds',
    'overlay state must not hold Snapshot, version, quality result, export, Agent parts, camera, or renderer state',
  )
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
