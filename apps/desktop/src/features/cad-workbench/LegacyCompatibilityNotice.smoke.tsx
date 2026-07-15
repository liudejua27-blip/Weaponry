import { isValidElement, type ReactNode } from 'react'
import { LegacyCompatibilityNotice } from './LegacyCompatibilityNotice.js'
import { getLegacyCompatibilityDisplay } from './legacyCompatibilityDisplay.js'

function collectText(node: ReactNode): string {
  if (node === null || node === undefined || typeof node === 'boolean') return ''
  if (typeof node === 'string' || typeof node === 'number') return String(node)
  if (Array.isArray(node)) return node.map(collectText).join(' ')
  if (!isValidElement(node)) return ''
  if (typeof node.type === 'function') return collectText((node.type as (props: unknown) => ReactNode)(node.props))
  return collectText((node.props as { children?: ReactNode }).children)
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

export function runLegacyCompatibilityNoticeSmoke(): void {
  const none = getLegacyCompatibilityDisplay(null, 'idle')
  assert(none.source === 'none' && !none.showRebuildGuidance, 'empty Snapshot must not show legacy guidance')

  const legacy = getLegacyCompatibilityDisplay({
    project_id: 'project-legacy',
    active_design: { source: 'legacy_concept_read_only', project_id: 'project-legacy', legacy_version_id: 'legacy-v1', module_graph_id: 'graph-v1' },
    export: { source: 'legacy_concept_read_only', project_id: 'project-legacy', source_version_id: 'legacy-v1' },
    revision: 1,
    updated_at: '2026-07-14T00:00:00Z',
  }, 'idle')
  assert(legacy.source === 'legacy_read_only' && legacy.isLegacyReadOnly && legacy.rebuildActionEnabled, 'legacy Snapshot must expose only read-only rebuild guidance')
  assert(!('revision' in legacy) && !('asset_version_id' in legacy), 'display model must not own Snapshot or asset-version truth')
  const legacyText = collectText(LegacyCompatibilityNotice({ display: legacy, onRequestLegacyAgentRebuild: () => undefined }))
  assert(legacyText.includes('旧版只读设计') && legacyText.includes('原设计会保留不变'), 'legacy guidance must explain the safe conversion boundary')

  const agent = getLegacyCompatibilityDisplay({
    project_id: 'project-agent',
    active_design: { source: 'agent_asset', project_id: 'project-agent', asset_version_id: 'asset-v1', assembly_graph_id: 'assembly-v1' },
    export: { source: 'agent_asset', project_id: 'project-agent', source_version_id: 'asset-v1' },
    revision: 2,
    updated_at: '2026-07-14T00:00:00Z',
  }, 'idle')
  assert(agent.source === 'agent_asset' && !agent.showRebuildGuidance, 'Agent Snapshot must not render legacy conversion guidance')
}
