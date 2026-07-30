type ViewportReadoutInput = {
  isPreviewActive: boolean
  hasActiveAgentAsset: boolean
  legacyDetailsEnabled: boolean
}

type MeasurementPromptInput = {
  hasMeasurementStart: boolean
  hasMeasurementEnd: boolean
}

export function buildViewportReadoutText({
  isPreviewActive,
  hasActiveAgentAsset,
  legacyDetailsEnabled,
}: ViewportReadoutInput): string {
  if (isPreviewActive) {
    return '正在预览 Agent 修改，尚未保存'
  }
  if (hasActiveAgentAsset) {
    return '当前视口绑定 Agent Snapshot'
  }
  return legacyDetailsEnabled
    ? '旧版 Graph 只读查看'
    : '等待 Agent 预览'
}

export function buildViewportMeasurementPrompt({
  hasMeasurementStart,
  hasMeasurementEnd,
}: MeasurementPromptInput): string {
  if (!hasMeasurementStart) return '点击模型设置起点'
  if (!hasMeasurementEnd) return '点击模型设置终点'
  return ''
}
