import type { AgentAssetRenderSet } from '../../shared/types.js'
import {
  agentRenderPresentationReducer,
  initialAgentRenderPresentationState,
} from './agentRenderPresentationState.js'

const renderSet = (assetVersionId: string, sha256: string): AgentAssetRenderSet => ({
  schema_version: 'AgentAssetRenderSet@1',
  asset_version_id: assetVersionId,
  width: 512,
  height: 512,
  render_set_sha256: sha256,
  render_set_byte_size: 3,
  rendered_at: '2026-07-14T00:00:00Z',
  views: [{ asset_version_id: assetVersionId, view_id: 'iso', camera_view: 'iso', width: 512, height: 512, png_base64: 'png', sha256: 'view-hash', byte_size: 3 }],
})

export function runAgentRenderPresentationStateSmoke(): void {
  let state = agentRenderPresentationReducer(initialAgentRenderPresentationState, {
    type: 'open_context', projectId: 'project-a', assetVersionId: 'asset-a', requestId: 1,
  })
  state = agentRenderPresentationReducer(state, {
    type: 'render_started', projectId: 'project-a', assetVersionId: 'asset-a', requestId: 2,
  })
  const lateWrongAsset = agentRenderPresentationReducer(state, {
    type: 'render_received', projectId: 'project-a', assetVersionId: 'asset-b', requestId: 2, renderSet: renderSet('asset-b', 'hash-b'),
  })
  assert(lateWrongAsset === state, 'a render for another asset must not replace the current presentation')
  state = agentRenderPresentationReducer(state, {
    type: 'render_received', projectId: 'project-a', assetVersionId: 'asset-a', requestId: 2, renderSet: renderSet('asset-a', 'hash-a'),
  })
  assert(state.renderSet?.render_set_sha256 === 'hash-a' && !state.renderLoading, 'only the current asset render result may become visible')

  const stalePackageFingerprint = agentRenderPresentationReducer(state, {
    type: 'package_started', projectId: 'project-a', assetVersionId: 'asset-a', requestId: 3, renderSetSha256: 'old-hash',
  })
  assert(stalePackageFingerprint === state, 'a package request must use the currently displayed render fingerprint')
  state = agentRenderPresentationReducer(state, {
    type: 'package_started', projectId: 'project-a', assetVersionId: 'asset-a', requestId: 3, renderSetSha256: 'hash-a',
  })
  assert(state.renderPackageLoading, 'the current render fingerprint may start a package request')

  state = agentRenderPresentationReducer(state, {
    type: 'open_context', projectId: 'project-a', assetVersionId: 'asset-b', requestId: 4,
  })
  assert(
    state.renderSet === null && !state.renderLoading && !state.renderPackageLoading,
    'asset changes must clear prior concept images and all pending presentation requests',
  )
  const latePackage = agentRenderPresentationReducer(state, {
    type: 'package_finished', projectId: 'project-a', assetVersionId: 'asset-a', requestId: 3, renderSetSha256: 'hash-a',
  })
  assert(latePackage === state, 'late package results must not affect a newer Agent asset context')

  state = agentRenderPresentationReducer(state, {
    type: 'render_started', projectId: 'project-a', assetVersionId: 'asset-b', requestId: 5,
  })
  state = agentRenderPresentationReducer(state, { type: 'drawer_closed', requestId: 6 })
  const lateAfterClose = agentRenderPresentationReducer(state, {
    type: 'render_received', projectId: 'project-a', assetVersionId: 'asset-b', requestId: 5, renderSet: renderSet('asset-b', 'hash-b'),
  })
  assert(lateAfterClose === state && !state.renderLoading, 'closing the drawer must discard late render responses without changing a model or export')

  state = agentRenderPresentationReducer(state, {
    type: 'open_context', projectId: null, assetVersionId: null, requestId: 7,
  })
  const fields = Object.keys(state).sort().join(',')
  assert(
    fields === 'assetVersionId,latestRequestId,projectId,renderLoading,renderPackageLoading,renderSet',
    'presentation state must not contain Snapshot, quality, ChangeSet, export, renderer, image URL, or version-head truth',
  )
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
