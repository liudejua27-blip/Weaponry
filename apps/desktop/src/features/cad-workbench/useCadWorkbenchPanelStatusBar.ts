import { useMemo } from 'react'
import type { ActiveDesignSnapshot } from '../../shared/types'
import { buildWorkbenchStatusBarPresentation } from './workbenchStatusBarPresentation'
import type { WorkbenchStatusBarPresentation } from './workbenchStatusBarPresentation'

type ProjectVersion = {
  version_id: string
  version_no: number | string
}

type UseCadWorkbenchPanelStatusBarInput = {
  conceptLoading: boolean
  conceptLegacyDetailsEnabled: boolean
  activeAgentAssetVersionVersionNo: number | null
  activeDesignSnapshot: ActiveDesignSnapshot | null
  conceptVersions: readonly ProjectVersion[] | null | undefined
  conceptVersionId: string | null
  conceptQualityStatus?: 'passed' | 'warning' | 'failed' | 'not_run' | 'unavailable'
  agentQualityStatus?: 'passed' | 'warning' | 'failed' | 'unavailable' | null
}

export function useCadWorkbenchPanelStatusBar({
  conceptLoading,
  conceptLegacyDetailsEnabled,
  activeAgentAssetVersionVersionNo,
  activeDesignSnapshot,
  conceptVersions,
  conceptVersionId,
  conceptQualityStatus,
  agentQualityStatus,
}: UseCadWorkbenchPanelStatusBarInput): WorkbenchStatusBarPresentation {
  const activeVersionSummary = useMemo(
    () => (conceptVersions ?? []).find((item) => item.version_id === conceptVersionId) ?? null,
    [conceptVersions, conceptVersionId],
  )

  return useMemo(
    () => buildWorkbenchStatusBarPresentation({
      conceptLoading,
      conceptLegacyDetailsEnabled,
      activeAgentAssetVersionVersionNo,
      activeDesignSnapshot,
      activeVersionSummary,
      conceptQualityStatus,
      agentQualityStatus,
    }),
    [
      conceptLoading,
      conceptLegacyDetailsEnabled,
      activeAgentAssetVersionVersionNo,
      activeDesignSnapshot,
      activeVersionSummary,
      conceptQualityStatus,
      agentQualityStatus,
    ],
  )
}
