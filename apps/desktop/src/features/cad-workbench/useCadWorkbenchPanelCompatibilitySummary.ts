import { useMemo } from 'react'
import { buildCadWorkbenchPanelCompatibilitySummary } from './cadWorkbenchPanelCompatibilitySummary'

type UseCadWorkbenchPanelCompatibilitySummaryInput = (
  Parameters<typeof buildCadWorkbenchPanelCompatibilitySummary>[0]
)

type UseCadWorkbenchPanelCompatibilitySummary = (
  ReturnType<typeof buildCadWorkbenchPanelCompatibilitySummary>
)

export function useCadWorkbenchPanelCompatibilitySummary({
  activeAssetSummary,
  directionSummary,
  fallbackPartCount,
  activeAssetVersionNo,
}: UseCadWorkbenchPanelCompatibilitySummaryInput): UseCadWorkbenchPanelCompatibilitySummary {
  return useMemo(
    () => buildCadWorkbenchPanelCompatibilitySummary({
      activeAssetSummary,
      directionSummary,
      fallbackPartCount,
      activeAssetVersionNo,
    }),
    [activeAssetSummary, activeAssetVersionNo, directionSummary, fallbackPartCount],
  )
}
