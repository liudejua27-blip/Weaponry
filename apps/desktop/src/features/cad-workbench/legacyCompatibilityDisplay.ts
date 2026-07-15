import type { ActiveDesignSnapshot } from '../../shared/types.js'

/**
 * Read-only presentation model for historical Concept data. It intentionally
 * exposes neither version IDs nor conversion authority: the parent keeps the
 * Snapshot, ETag and conversion command as the only state-changing boundary.
 */
export type LegacyCompatibilityDisplay = {
  source: 'none' | 'legacy_read_only' | 'agent_asset'
  isLegacyReadOnly: boolean
  showRebuildGuidance: boolean
  rebuildActionEnabled: boolean
}

export function getLegacyCompatibilityDisplay(
  snapshot: ActiveDesignSnapshot | null,
  activeDesignOperation: string,
): LegacyCompatibilityDisplay {
  if (!snapshot) {
    return {
      source: 'none',
      isLegacyReadOnly: false,
      showRebuildGuidance: false,
      rebuildActionEnabled: false,
    }
  }
  if (snapshot.active_design.source === 'legacy_concept_read_only') {
    return {
      source: 'legacy_read_only',
      isLegacyReadOnly: true,
      showRebuildGuidance: true,
      rebuildActionEnabled: activeDesignOperation === 'idle',
    }
  }
  return {
    source: 'agent_asset',
    isLegacyReadOnly: false,
    showRebuildGuidance: false,
    rebuildActionEnabled: false,
  }
}
