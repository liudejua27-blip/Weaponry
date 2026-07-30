type CadWorkbenchPanelCompatibilitySummary = {
  compatibilityResultSummary: string
  compatibilityVersionLabel: string
}

type CadWorkbenchPanelCompatibilitySummaryInput = {
  activeAssetSummary: string | null | undefined
  directionSummary: string | null | undefined
  fallbackPartCount: number
  activeAssetVersionNo: number | null
}

const PREVIEW_VERSION_LABEL = '预览状态 · 确认前不会写入版本'

export function buildCadWorkbenchPanelCompatibilitySummary({
  activeAssetSummary,
  directionSummary,
  fallbackPartCount,
  activeAssetVersionNo,
}: CadWorkbenchPanelCompatibilitySummaryInput): CadWorkbenchPanelCompatibilitySummary {
  const compatibilityResultSummary = activeAssetSummary
    ?? directionSummary
    ?? `已生成 ${fallbackPartCount} 个可编辑组件。`

  const compatibilityVersionLabel = activeAssetVersionNo === null
    ? PREVIEW_VERSION_LABEL
    : `可编辑资产 v${activeAssetVersionNo}`

  return {
    compatibilityResultSummary,
    compatibilityVersionLabel,
  }
}
